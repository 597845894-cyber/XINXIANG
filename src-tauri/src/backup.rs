use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
    path::Path,
};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use getrandom::fill;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    security::key_protection::{DpapiCurrentUserProtector, MasterKeyManager},
    storage::database::EncryptedDatabase,
};

const MAGIC: &[u8; 12] = b"XINXIANGBK01";
const FORMAT_VERSION: u8 = 1;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 12;
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Debug)]
pub enum BackupError {
    Invalid,
    PasswordOrIntegrity,
    Incompatible,
    InsufficientSpace,
    TargetExists,
    Io,
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid | Self::PasswordOrIntegrity => "BACKUP_PASSWORD_OR_FILE_INVALID",
            Self::Incompatible => "BACKUP_VERSION_UNSUPPORTED",
            Self::InsufficientSpace => "BACKUP_INSUFFICIENT_SPACE",
            Self::TargetExists => "BACKUP_TARGET_EXISTS",
            Self::Io => "BACKUP_STORAGE_FAILED",
        })
    }
}

impl std::error::Error for BackupError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub format_version: u8,
    pub created_at: String,
    pub notice_count: u64,
    pub task_count: u64,
    pub attachment_count: u64,
    pub byte_size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    format_version: u8,
    created_at: String,
    app_version: String,
    summary: BackupSummary,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFile {
    path: String,
    byte_size: u64,
    sha256: String,
}

pub fn create(
    app_data_directory: &Path,
    target: &Path,
    password: &str,
    summary: BackupSummary,
) -> Result<BackupSummary, BackupError> {
    if password.chars().count() < 8 || target.exists() {
        return Err(if target.exists() {
            BackupError::TargetExists
        } else {
            BackupError::Invalid
        });
    }
    let parent = target.parent().ok_or(BackupError::Io)?;
    fs::create_dir_all(parent).map_err(|_| BackupError::Io)?;
    let mut entries = Vec::new();
    entries.push(read_entry(app_data_directory, "inbox.db")?);
    entries.push(read_entry(app_data_directory, "security/master-key.json")?);
    let attachments = app_data_directory.join("attachments");
    if attachments.exists() {
        for entry in fs::read_dir(&attachments).map_err(|_| BackupError::Io)? {
            let entry = entry.map_err(|_| BackupError::Io)?;
            if entry.file_type().map_err(|_| BackupError::Io)?.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".enc") {
                    entries.push(read_entry(
                        app_data_directory,
                        &format!("attachments/{name}"),
                    )?);
                }
            }
        }
    }
    let mut summary = summary;
    summary.byte_size = entries.iter().map(|entry| entry.data.len() as u64).sum();
    let manifest_files = entries
        .iter()
        .map(|entry| ManifestFile {
            path: entry.path.clone(),
            byte_size: entry.data.len() as u64,
            sha256: digest_hex(&entry.data),
        })
        .collect();
    let manifest = BackupManifest {
        format_version: FORMAT_VERSION,
        created_at: summary.created_at.clone(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        summary,
        files: manifest_files,
    };
    let mut archive = vec![ArchiveEntry {
        path: "manifest.json".to_owned(),
        data: serde_json::to_vec(&manifest).map_err(|_| BackupError::Invalid)?,
    }];
    archive.append(&mut entries);
    let plaintext = encode_archive(&archive)?;
    if plaintext.len() > MAX_PAYLOAD_BYTES {
        return Err(BackupError::Invalid);
    }
    let bytes = encrypt_container(&plaintext, password)?;
    let staging = target.with_extension(format!("{}.staging", Uuid::new_v4()));
    write_synced(&staging, &bytes)?;
    fs::rename(&staging, target).map_err(|_| BackupError::Io)?;
    Ok(manifest.summary)
}

pub fn inspect(path: &Path, password: &str) -> Result<BackupSummary, BackupError> {
    let archive = read_archive(path, password)?;
    Ok(manifest_from(&archive)?.summary)
}

pub fn restore(
    app_data_directory: &Path,
    path: &Path,
    password: &str,
) -> Result<BackupSummary, BackupError> {
    let archive = read_archive(path, password)?;
    let manifest = manifest_from(&archive)?;
    let required_bytes = archive
        .values()
        .map(|value| value.len() as u64)
        .sum::<u64>();
    let available = fs2::available_space(app_data_directory).map_err(|_| BackupError::Io)?;
    if available
        < required_bytes
            .saturating_mul(2)
            .saturating_add(5 * 1024 * 1024)
    {
        return Err(BackupError::InsufficientSpace);
    }
    let parent = app_data_directory.parent().ok_or(BackupError::Io)?;
    let staging = parent.join(format!(".xinxiang-restore-{}", Uuid::new_v4()));
    let previous = parent.join(format!(".xinxiang-previous-{}", Uuid::new_v4()));
    let result = restore_to_staging(&staging, &archive)
        .and_then(|_| verify_staged_data(&staging))
        .and_then(|_| replace_data(app_data_directory, &staging, &previous));
    if result.is_err() {
        let _ = remove_path(&staging);
    }
    result?;
    let _ = remove_path(&previous);
    Ok(manifest.summary)
}

fn read_archive(path: &Path, password: &str) -> Result<BTreeMap<String, Vec<u8>>, BackupError> {
    let bytes = fs::read(path).map_err(|_| BackupError::Io)?;
    let plaintext = decrypt_container(&bytes, password)?;
    decode_archive(&plaintext)
}

fn read_entry(root: &Path, relative: &str) -> Result<ArchiveEntry, BackupError> {
    validate_relative_path(relative)?;
    Ok(ArchiveEntry {
        path: relative.to_owned(),
        data: fs::read(root.join(relative)).map_err(|_| BackupError::Io)?,
    })
}

fn manifest_from(archive: &BTreeMap<String, Vec<u8>>) -> Result<BackupManifest, BackupError> {
    let bytes = archive.get("manifest.json").ok_or(BackupError::Invalid)?;
    let manifest: BackupManifest =
        serde_json::from_slice(bytes).map_err(|_| BackupError::Invalid)?;
    if manifest.format_version != FORMAT_VERSION
        || !archive.contains_key("inbox.db")
        || !archive.contains_key("security/master-key.json")
    {
        return Err(BackupError::Incompatible);
    }
    for file in &manifest.files {
        let data = archive.get(&file.path).ok_or(BackupError::Invalid)?;
        if data.len() as u64 != file.byte_size || digest_hex(data) != file.sha256 {
            return Err(BackupError::PasswordOrIntegrity);
        }
    }
    Ok(manifest)
}

fn restore_to_staging(
    staging: &Path,
    archive: &BTreeMap<String, Vec<u8>>,
) -> Result<(), BackupError> {
    fs::create_dir_all(staging).map_err(|_| BackupError::Io)?;
    for (relative, data) in archive {
        if relative == "manifest.json" {
            continue;
        }
        validate_relative_path(relative)?;
        let target = staging.join(relative);
        if !target.starts_with(staging) {
            return Err(BackupError::Invalid);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| BackupError::Io)?;
        }
        write_synced(&target, data)?;
    }
    Ok(())
}

fn verify_staged_data(staging: &Path) -> Result<(), BackupError> {
    let manager = MasterKeyManager::new(
        staging.join("security").join("master-key.json"),
        DpapiCurrentUserProtector,
    );
    let key = manager.load_or_create().map_err(|_| BackupError::Invalid)?;
    let database = EncryptedDatabase::open(&staging.join("inbox.db"), &key)
        .map_err(|_| BackupError::Invalid)?;
    database
        .schema_version()
        .map_err(|_| BackupError::Invalid)?;
    Ok(())
}

fn replace_data(
    app_data_directory: &Path,
    staging: &Path,
    previous: &Path,
) -> Result<(), BackupError> {
    fs::create_dir_all(app_data_directory).map_err(|_| BackupError::Io)?;
    fs::create_dir_all(previous).map_err(|_| BackupError::Io)?;
    let names = ["inbox.db", "attachments", "security"];
    let mut moved = Vec::new();
    for name in names {
        let active = app_data_directory.join(name);
        if active.exists() {
            fs::rename(&active, previous.join(name)).map_err(|_| BackupError::Io)?;
            moved.push(name);
        }
    }
    for name in names {
        let incoming = staging.join(name);
        if incoming.exists() && fs::rename(&incoming, app_data_directory.join(name)).is_err() {
            for restore_name in names {
                let current = app_data_directory.join(restore_name);
                let old = previous.join(restore_name);
                let _ = remove_path(&current);
                if old.exists() {
                    let _ = fs::rename(old, current);
                }
            }
            return Err(BackupError::Io);
        }
    }
    let _ = moved;
    remove_path(staging)
}

fn encrypt_container(plaintext: &[u8], password: &str) -> Result<Vec<u8>, BackupError> {
    let mut salt = [0_u8; SALT_BYTES];
    let mut nonce = [0_u8; NONCE_BYTES];
    fill(&mut salt).map_err(|_| BackupError::Io)?;
    fill(&mut nonce).map_err(|_| BackupError::Io)?;
    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| BackupError::Invalid)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| BackupError::Invalid)?;
    let mut result =
        Vec::with_capacity(MAGIC.len() + 1 + SALT_BYTES + NONCE_BYTES + ciphertext.len());
    result.extend_from_slice(MAGIC);
    result.push(FORMAT_VERSION);
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

fn decrypt_container(bytes: &[u8], password: &str) -> Result<Vec<u8>, BackupError> {
    let header = MAGIC.len() + 1 + SALT_BYTES + NONCE_BYTES;
    if bytes.len() <= header
        || &bytes[..MAGIC.len()] != MAGIC
        || bytes[MAGIC.len()] != FORMAT_VERSION
    {
        return Err(BackupError::Invalid);
    }
    let salt_start = MAGIC.len() + 1;
    let nonce_start = salt_start + SALT_BYTES;
    let key = derive_key(password, &bytes[salt_start..nonce_start])?;
    let cipher = Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| BackupError::Invalid)?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&bytes[nonce_start..header]),
            &bytes[header..],
        )
        .map_err(|_| BackupError::PasswordOrIntegrity)?;
    if plaintext.len() > MAX_PAYLOAD_BYTES {
        return Err(BackupError::Invalid);
    }
    Ok(plaintext)
}

fn derive_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, BackupError> {
    if password.chars().count() < 8 || salt.len() != SALT_BYTES {
        return Err(BackupError::Invalid);
    }
    let parameters = Params::new(19_456, 2, 1, Some(32)).map_err(|_| BackupError::Invalid)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters);
    let mut key = Zeroizing::new([0_u8; 32]);
    argon
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|_| BackupError::Invalid)?;
    Ok(key)
}

#[derive(Debug)]
struct ArchiveEntry {
    path: String,
    data: Vec<u8>,
}

fn encode_archive(entries: &[ArchiveEntry]) -> Result<Vec<u8>, BackupError> {
    let mut result = Vec::new();
    result.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in entries {
        validate_relative_path(&entry.path)?;
        let path = entry.path.as_bytes();
        let length = u16::try_from(path.len()).map_err(|_| BackupError::Invalid)?;
        result.extend_from_slice(&length.to_le_bytes());
        result.extend_from_slice(path);
        result.extend_from_slice(&(entry.data.len() as u64).to_le_bytes());
        result.extend_from_slice(&entry.data);
    }
    Ok(result)
}

fn decode_archive(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, BackupError> {
    let mut cursor = Cursor::new(bytes);
    let count = read_u32(&mut cursor)?;
    if count == 0 || count > 10_000 {
        return Err(BackupError::Invalid);
    }
    let mut entries = BTreeMap::new();
    for _ in 0..count {
        let path_length = read_u16(&mut cursor)? as usize;
        let mut path = vec![0_u8; path_length];
        cursor
            .read_exact(&mut path)
            .map_err(|_| BackupError::Invalid)?;
        let path = String::from_utf8(path).map_err(|_| BackupError::Invalid)?;
        validate_relative_path(&path)?;
        let length = read_u64(&mut cursor)? as usize;
        if length > MAX_PAYLOAD_BYTES || length > bytes.len() {
            return Err(BackupError::Invalid);
        }
        let mut data = vec![0_u8; length];
        cursor
            .read_exact(&mut data)
            .map_err(|_| BackupError::Invalid)?;
        if entries.insert(path, data).is_some() {
            return Err(BackupError::Invalid);
        }
    }
    if cursor.position() as usize != bytes.len() {
        return Err(BackupError::Invalid);
    }
    Ok(entries)
}

fn validate_relative_path(path: &str) -> Result<(), BackupError> {
    if path.is_empty()
        || Path::new(path).is_absolute()
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path.contains('\\')
    {
        return Err(BackupError::Invalid);
    }
    Ok(())
}

fn read_u16(cursor: &mut Cursor<&[u8]>) -> Result<u16, BackupError> {
    let mut bytes = [0_u8; 2];
    cursor
        .read_exact(&mut bytes)
        .map_err(|_| BackupError::Invalid)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(cursor: &mut Cursor<&[u8]>) -> Result<u32, BackupError> {
    let mut bytes = [0_u8; 4];
    cursor
        .read_exact(&mut bytes)
        .map_err(|_| BackupError::Invalid)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(cursor: &mut Cursor<&[u8]>) -> Result<u64, BackupError> {
    let mut bytes = [0_u8; 8];
    cursor
        .read_exact(&mut bytes)
        .map_err(|_| BackupError::Invalid)?;
    Ok(u64::from_le_bytes(bytes))
}

fn write_synced(path: &Path, data: &[u8]) -> Result<(), BackupError> {
    let mut file = fs::File::create(path).map_err(|_| BackupError::Io)?;
    file.write_all(data).map_err(|_| BackupError::Io)?;
    file.sync_all().map_err(|_| BackupError::Io)
}

fn remove_path(path: &Path) -> Result<(), BackupError> {
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|_| BackupError::Io)
    } else if path.exists() {
        fs::remove_file(path).map_err(|_| BackupError::Io)
    } else {
        Ok(())
    }
}

fn digest_hex(value: &[u8]) -> String {
    let hash = Sha256::digest(value);
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        decode_archive, decrypt_container, encode_archive, encrypt_container, ArchiveEntry,
    };

    #[test]
    fn encrypted_container_hides_plaintext_and_rejects_wrong_password() {
        let plaintext = "通知正文：明天 17:00 前提交材料".as_bytes();
        let container = encrypt_container(plaintext, "correct-passphrase").unwrap();

        assert!(!container
            .windows(plaintext.len())
            .any(|part| part == plaintext));
        assert_eq!(
            decrypt_container(&container, "correct-passphrase").unwrap(),
            plaintext
        );
        assert!(decrypt_container(&container, "incorrect-passphrase").is_err());
    }

    #[test]
    fn modified_container_and_unsafe_archive_paths_are_rejected() {
        let mut container = encrypt_container(b"protected", "correct-passphrase").unwrap();
        let last = container.len() - 1;
        container[last] ^= 0x01;
        assert!(decrypt_container(&container, "correct-passphrase").is_err());

        let archive = encode_archive(&[ArchiveEntry {
            path: "../outside".to_owned(),
            data: b"blocked".to_vec(),
        }]);
        assert!(archive.is_err());
    }

    #[test]
    fn archive_round_trip_requires_each_entry_to_be_complete() {
        let archive = encode_archive(&[ArchiveEntry {
            path: "security/master-key.json".to_owned(),
            data: b"metadata".to_vec(),
        }])
        .unwrap();
        assert_eq!(
            decode_archive(&archive)
                .unwrap()
                .get("security/master-key.json")
                .unwrap(),
            b"metadata"
        );
        assert!(decode_archive(&archive[..archive.len() - 1]).is_err());
    }
}

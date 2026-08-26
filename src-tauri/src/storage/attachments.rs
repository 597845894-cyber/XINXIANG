use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use aes_gcm::{
    aead::{AeadInPlace, KeyInit},
    Aes256Gcm, Nonce,
};
use getrandom::fill;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::security::key_protection::MasterKey;

const FORMAT_VERSION: u8 = 1;
const CHUNK_BYTES: usize = 64 * 1024;
const NONCE_BYTES: usize = 12;
const TAG_BYTES: usize = 16;

#[derive(Debug)]
pub enum AttachmentError {
    Io,
    Encryption,
    Integrity,
    InvalidFormat,
}

impl std::fmt::Display for AttachmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::Io => "ATTACHMENT_IO_FAILED",
            Self::Encryption => "ATTACHMENT_ENCRYPTION_FAILED",
            Self::Integrity | Self::InvalidFormat => "ATTACHMENT_CONTENT_UNAVAILABLE",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for AttachmentError {}

#[derive(Debug, Clone)]
pub struct EncryptedAttachmentMetadata {
    pub encrypted_data_key: Vec<u8>,
    pub key_nonce: [u8; NONCE_BYTES],
    pub content_nonce: [u8; NONCE_BYTES],
    pub ciphertext_checksum: [u8; 32],
}

pub struct AttachmentStore {
    root: PathBuf,
}

impl AttachmentStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn encrypted_path(&self, attachment_id: &str) -> PathBuf {
        self.root.join(format!("{attachment_id}.enc"))
    }

    pub fn write_from_reader(
        &self,
        attachment_id: &str,
        input: &mut impl Read,
        master_key: &MasterKey,
    ) -> Result<EncryptedAttachmentMetadata, AttachmentError> {
        fs::create_dir_all(&self.root).map_err(|_| AttachmentError::Io)?;
        let target = self.encrypted_path(attachment_id);
        let staging = target.with_extension("enc.staging");
        let _ = fs::remove_file(&staging);

        let result = self.write_staging(attachment_id, &staging, input, master_key);
        match result {
            Ok(metadata) => {
                if fs::rename(&staging, &target).is_err() {
                    let _ = fs::remove_file(&staging);
                    return Err(AttachmentError::Io);
                }
                Ok(metadata)
            }
            Err(error) => {
                let _ = fs::remove_file(&staging);
                Err(error)
            }
        }
    }

    pub fn read_to_writer(
        &self,
        attachment_id: &str,
        metadata: &EncryptedAttachmentMetadata,
        output: &mut impl Write,
        master_key: &MasterKey,
    ) -> Result<(), AttachmentError> {
        let data_key = unwrap_data_key(attachment_id, metadata, master_key)?;
        let cipher =
            Aes256Gcm::new_from_slice(&data_key).map_err(|_| AttachmentError::Encryption)?;
        let mut file =
            File::open(self.encrypted_path(attachment_id)).map_err(|_| AttachmentError::Io)?;
        let mut checksum = Sha256::new();
        let mut header = [0_u8; 2];
        file.read_exact(&mut header)
            .map_err(|_| AttachmentError::Integrity)?;
        if header != [FORMAT_VERSION, NONCE_BYTES as u8] {
            return Err(AttachmentError::InvalidFormat);
        }

        let mut chunk_index = 0_u64;
        loop {
            let mut length_bytes = [0_u8; 4];
            match file.read_exact(&mut length_bytes) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(_) => return Err(AttachmentError::Integrity),
            }
            let encrypted_length = u32::from_le_bytes(length_bytes) as usize;
            if encrypted_length < TAG_BYTES || encrypted_length > CHUNK_BYTES + TAG_BYTES {
                return Err(AttachmentError::InvalidFormat);
            }
            let mut encrypted_chunk = vec![0_u8; encrypted_length];
            file.read_exact(&mut encrypted_chunk)
                .map_err(|_| AttachmentError::Integrity)?;
            checksum.update(&length_bytes);
            checksum.update(&encrypted_chunk);
            let nonce = nonce_for_chunk(metadata.content_nonce, chunk_index);
            cipher
                .decrypt_in_place(
                    Nonce::from_slice(&nonce),
                    &chunk_aad(attachment_id, chunk_index),
                    &mut encrypted_chunk,
                )
                .map_err(|_| AttachmentError::Integrity)?;
            output
                .write_all(&encrypted_chunk)
                .map_err(|_| AttachmentError::Io)?;
            chunk_index += 1;
        }

        if checksum.finalize().as_slice() != metadata.ciphertext_checksum {
            return Err(AttachmentError::Integrity);
        }
        Ok(())
    }

    fn write_staging(
        &self,
        attachment_id: &str,
        staging: &Path,
        input: &mut impl Read,
        master_key: &MasterKey,
    ) -> Result<EncryptedAttachmentMetadata, AttachmentError> {
        let mut data_key = Zeroizing::new([0_u8; 32]);
        fill(data_key.as_mut()).map_err(|_| AttachmentError::Encryption)?;
        let mut content_nonce = [0_u8; NONCE_BYTES];
        let mut key_nonce = [0_u8; NONCE_BYTES];
        fill(&mut content_nonce).map_err(|_| AttachmentError::Encryption)?;
        fill(&mut key_nonce).map_err(|_| AttachmentError::Encryption)?;

        let key_cipher = Aes256Gcm::new_from_slice(master_key.expose())
            .map_err(|_| AttachmentError::Encryption)?;
        let mut encrypted_data_key = data_key.to_vec();
        key_cipher
            .encrypt_in_place(
                Nonce::from_slice(&key_nonce),
                attachment_id.as_bytes(),
                &mut encrypted_data_key,
            )
            .map_err(|_| AttachmentError::Encryption)?;

        let content_cipher = Aes256Gcm::new_from_slice(data_key.as_ref())
            .map_err(|_| AttachmentError::Encryption)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(staging)
            .map_err(|_| AttachmentError::Io)?;
        file.write_all(&[FORMAT_VERSION, NONCE_BYTES as u8])
            .map_err(|_| AttachmentError::Io)?;

        let mut checksum = Sha256::new();
        let mut buffer = vec![0_u8; CHUNK_BYTES];
        let mut chunk_index = 0_u64;
        loop {
            let bytes_read = input.read(&mut buffer).map_err(|_| AttachmentError::Io)?;
            if bytes_read == 0 {
                break;
            }
            let mut chunk = buffer[..bytes_read].to_vec();
            let nonce = nonce_for_chunk(content_nonce, chunk_index);
            content_cipher
                .encrypt_in_place(
                    Nonce::from_slice(&nonce),
                    &chunk_aad(attachment_id, chunk_index),
                    &mut chunk,
                )
                .map_err(|_| AttachmentError::Encryption)?;
            let length = u32::try_from(chunk.len())
                .map_err(|_| AttachmentError::Encryption)?
                .to_le_bytes();
            file.write_all(&length)
                .and_then(|_| file.write_all(&chunk))
                .map_err(|_| AttachmentError::Io)?;
            checksum.update(length);
            checksum.update(&chunk);
            chunk_index += 1;
        }
        file.sync_all().map_err(|_| AttachmentError::Io)?;

        Ok(EncryptedAttachmentMetadata {
            encrypted_data_key,
            key_nonce,
            content_nonce,
            ciphertext_checksum: checksum.finalize().into(),
        })
    }
}

fn unwrap_data_key(
    attachment_id: &str,
    metadata: &EncryptedAttachmentMetadata,
    master_key: &MasterKey,
) -> Result<Zeroizing<Vec<u8>>, AttachmentError> {
    let cipher =
        Aes256Gcm::new_from_slice(master_key.expose()).map_err(|_| AttachmentError::Encryption)?;
    let mut data_key = metadata.encrypted_data_key.clone();
    cipher
        .decrypt_in_place(
            Nonce::from_slice(&metadata.key_nonce),
            attachment_id.as_bytes(),
            &mut data_key,
        )
        .map_err(|_| AttachmentError::Integrity)?;
    if data_key.len() != 32 {
        return Err(AttachmentError::InvalidFormat);
    }
    Ok(Zeroizing::new(data_key))
}

fn nonce_for_chunk(mut base_nonce: [u8; NONCE_BYTES], chunk_index: u64) -> [u8; NONCE_BYTES] {
    let index_bytes = chunk_index.to_be_bytes();
    for (offset, byte) in index_bytes.iter().enumerate() {
        base_nonce[NONCE_BYTES - index_bytes.len() + offset] ^= byte;
    }
    base_nonce
}

fn chunk_aad(attachment_id: &str, chunk_index: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(attachment_id.len() + 8);
    aad.extend_from_slice(attachment_id.as_bytes());
    aad.extend_from_slice(&chunk_index.to_be_bytes());
    aad
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{AttachmentError, AttachmentStore};
    use crate::security::key_protection::MasterKey;
    use tempfile::tempdir;

    fn test_key() -> MasterKey {
        MasterKey::from_bytes(&[13; 32]).unwrap()
    }

    #[test]
    fn encrypts_large_attachments_without_leaving_plaintext_on_disk() {
        let directory = tempdir().unwrap();
        let store = AttachmentStore::new(directory.path());
        let plaintext = vec![b'A'; 160_000];
        let metadata = store
            .write_from_reader("asset-1", &mut Cursor::new(&plaintext), &test_key())
            .unwrap();

        let encrypted = std::fs::read(store.encrypted_path("asset-1")).unwrap();
        assert!(!encrypted
            .windows(24)
            .any(|window| window == b"AAAAAAAAAAAAAAAAAAAAAAAA"));
        let mut restored = Vec::new();
        store
            .read_to_writer("asset-1", &metadata, &mut restored, &test_key())
            .unwrap();
        assert_eq!(restored, plaintext);
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let directory = tempdir().unwrap();
        let store = AttachmentStore::new(directory.path());
        let metadata = store
            .write_from_reader(
                "asset-2",
                &mut Cursor::new(b"private source image"),
                &test_key(),
            )
            .unwrap();
        let path = store.encrypted_path("asset-2");
        let mut encrypted = std::fs::read(&path).unwrap();
        let last_index = encrypted.len() - 1;
        encrypted[last_index] ^= 1;
        std::fs::write(path, encrypted).unwrap();

        let result = store.read_to_writer("asset-2", &metadata, &mut Vec::new(), &test_key());
        assert!(matches!(result, Err(AttachmentError::Integrity)));
    }

    #[test]
    fn removes_staging_file_when_encryption_cannot_read_input() {
        struct BrokenReader;
        impl std::io::Read for BrokenReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("synthetic read failure"))
            }
        }

        let directory = tempdir().unwrap();
        let store = AttachmentStore::new(directory.path());
        assert!(store
            .write_from_reader("asset-3", &mut BrokenReader, &test_key())
            .is_err());
        assert!(!store
            .encrypted_path("asset-3")
            .with_extension("enc.staging")
            .exists());
    }

    #[test]
    fn removes_staging_file_when_atomic_publish_fails() {
        let directory = tempdir().unwrap();
        let store = AttachmentStore::new(directory.path());
        std::fs::create_dir(store.encrypted_path("asset-4")).unwrap();

        assert!(store
            .write_from_reader(
                "asset-4",
                &mut Cursor::new(b"private source image"),
                &test_key(),
            )
            .is_err());
        assert!(!store
            .encrypted_path("asset-4")
            .with_extension("enc.staging")
            .exists());
    }
}

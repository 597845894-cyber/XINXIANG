use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

const MASTER_KEY_BYTES: usize = 32;
const KEY_ENVELOPE_VERSION: u16 = 1;

#[derive(Debug)]
pub enum KeyProtectionError {
    Io,
    InvalidEnvelope,
    ProtectionFailed,
    UnlockFailed,
    RotationFailed,
    UnsupportedPlatform,
}

impl std::fmt::Display for KeyProtectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::Io => "KEY_IO_FAILED",
            Self::InvalidEnvelope => "KEY_ENVELOPE_INVALID",
            Self::ProtectionFailed => "KEY_PROTECTION_FAILED",
            Self::UnlockFailed => "KEY_UNLOCK_FAILED",
            Self::RotationFailed => "KEY_ROTATION_FAILED",
            Self::UnsupportedPlatform => "KEY_PLATFORM_UNSUPPORTED",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for KeyProtectionError {}

#[derive(Zeroize)]
#[zeroize(drop)]
pub struct MasterKey([u8; MASTER_KEY_BYTES]);

impl MasterKey {
    pub fn generate() -> Result<Self, KeyProtectionError> {
        let mut bytes = [0_u8; MASTER_KEY_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| KeyProtectionError::ProtectionFailed)?;
        Ok(Self(bytes))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KeyProtectionError> {
        let value: [u8; MASTER_KEY_BYTES] = bytes
            .try_into()
            .map_err(|_| KeyProtectionError::InvalidEnvelope)?;
        Ok(Self(value))
    }

    pub fn expose(&self) -> &[u8; MASTER_KEY_BYTES] {
        &self.0
    }
}

pub trait UserDataProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, KeyProtectionError>;
    fn unprotect(&self, protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeyProtectionError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DpapiCurrentUserProtector;

#[cfg(windows)]
impl UserDataProtector for DpapiCurrentUserProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, KeyProtectionError> {
        use windows_sys::Win32::Security::Cryptography::{
            CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        };

        let input = CRYPT_INTEGER_BLOB {
            cbData: plaintext
                .len()
                .try_into()
                .map_err(|_| KeyProtectionError::ProtectionFailed)?,
            pbData: plaintext.as_ptr().cast_mut(),
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        let succeeded = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if succeeded == 0 {
            return Err(KeyProtectionError::ProtectionFailed);
        }
        copy_and_free_blob(output).ok_or(KeyProtectionError::ProtectionFailed)
    }

    fn unprotect(&self, protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeyProtectionError> {
        use windows_sys::Win32::Security::Cryptography::{
            CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        };

        let input = CRYPT_INTEGER_BLOB {
            cbData: protected
                .len()
                .try_into()
                .map_err(|_| KeyProtectionError::UnlockFailed)?,
            pbData: protected.as_ptr().cast_mut(),
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        let succeeded = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if succeeded == 0 {
            return Err(KeyProtectionError::UnlockFailed);
        }
        copy_and_free_blob(output)
            .map(Zeroizing::new)
            .ok_or(KeyProtectionError::UnlockFailed)
    }
}

#[cfg(windows)]
fn copy_and_free_blob(
    blob: windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB,
) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;

    if blob.pbData.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec() };
    unsafe {
        LocalFree(blob.pbData.cast());
    }
    Some(bytes)
}

#[cfg(not(windows))]
impl UserDataProtector for DpapiCurrentUserProtector {
    fn protect(&self, _plaintext: &[u8]) -> Result<Vec<u8>, KeyProtectionError> {
        Err(KeyProtectionError::UnsupportedPlatform)
    }

    fn unprotect(&self, _protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeyProtectionError> {
        Err(KeyProtectionError::UnsupportedPlatform)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyEnvelopeV1 {
    schema_version: u16,
    protection: String,
    protected_key: Vec<u8>,
}

pub struct MasterKeyManager<P> {
    path: PathBuf,
    protector: P,
}

impl<P: UserDataProtector> MasterKeyManager<P> {
    pub fn new(path: impl Into<PathBuf>, protector: P) -> Self {
        Self {
            path: path.into(),
            protector,
        }
    }

    pub fn load_or_create(&self) -> Result<MasterKey, KeyProtectionError> {
        if self.path.exists() {
            return self.unlock();
        }
        let key = MasterKey::generate()?;
        let envelope = self.envelope_for(&key)?;
        write_new_envelope(&self.path, &envelope)?;
        Ok(key)
    }

    pub fn unlock(&self) -> Result<MasterKey, KeyProtectionError> {
        let envelope: KeyEnvelopeV1 =
            serde_json::from_slice(&fs::read(&self.path).map_err(|_| KeyProtectionError::Io)?)
                .map_err(|_| KeyProtectionError::InvalidEnvelope)?;
        if envelope.schema_version != KEY_ENVELOPE_VERSION
            || envelope.protection != "windows-dpapi-current-user"
        {
            return Err(KeyProtectionError::InvalidEnvelope);
        }
        let plaintext = self.protector.unprotect(&envelope.protected_key)?;
        MasterKey::from_bytes(&plaintext)
    }

    pub fn rotate<F>(&self, reencrypt: F) -> Result<MasterKey, KeyProtectionError>
    where
        F: FnOnce(&MasterKey, &MasterKey) -> Result<(), KeyProtectionError>,
    {
        let current = self.unlock()?;
        let next = MasterKey::generate()?;
        let envelope = self.envelope_for(&next)?;
        reencrypt(&current, &next).map_err(|_| KeyProtectionError::RotationFailed)?;
        replace_envelope(&self.path, &envelope)?;
        Ok(next)
    }

    fn envelope_for(&self, key: &MasterKey) -> Result<KeyEnvelopeV1, KeyProtectionError> {
        Ok(KeyEnvelopeV1 {
            schema_version: KEY_ENVELOPE_VERSION,
            protection: "windows-dpapi-current-user".to_owned(),
            protected_key: self.protector.protect(key.expose())?,
        })
    }
}

fn serialize_envelope(envelope: &KeyEnvelopeV1) -> Result<Vec<u8>, KeyProtectionError> {
    serde_json::to_vec(envelope).map_err(|_| KeyProtectionError::InvalidEnvelope)
}

fn write_new_envelope(path: &Path, envelope: &KeyEnvelopeV1) -> Result<(), KeyProtectionError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| KeyProtectionError::Io)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| KeyProtectionError::Io)?;
    file.write_all(&serialize_envelope(envelope)?)
        .and_then(|_| file.sync_all())
        .map_err(|_| KeyProtectionError::Io)
}

fn replace_envelope(path: &Path, envelope: &KeyEnvelopeV1) -> Result<(), KeyProtectionError> {
    let staging = path.with_extension("key.next");
    let backup = path.with_extension("key.previous");
    let _ = fs::remove_file(&staging);
    let _ = fs::remove_file(&backup);
    write_new_envelope(&staging, envelope)?;
    fs::rename(path, &backup).map_err(|_| KeyProtectionError::Io)?;
    if fs::rename(&staging, path).is_err() {
        let _ = fs::rename(&backup, path);
        let _ = fs::remove_file(&staging);
        return Err(KeyProtectionError::Io);
    }
    fs::remove_file(backup).map_err(|_| KeyProtectionError::Io)
}

#[cfg(test)]
mod tests {
    use super::{
        DpapiCurrentUserProtector, KeyProtectionError, MasterKeyManager, UserDataProtector,
    };
    use tempfile::tempdir;
    use zeroize::Zeroizing;

    #[derive(Clone)]
    struct ContextProtector(u8);

    impl UserDataProtector for ContextProtector {
        fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, KeyProtectionError> {
            let mut protected = vec![self.0];
            protected.extend(plaintext.iter().map(|value| value ^ self.0));
            Ok(protected)
        }

        fn unprotect(&self, protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeyProtectionError> {
            if protected.first().copied() != Some(self.0) {
                return Err(KeyProtectionError::UnlockFailed);
            }
            Ok(Zeroizing::new(
                protected[1..].iter().map(|value| value ^ self.0).collect(),
            ))
        }
    }

    #[test]
    fn creates_unlocks_and_rotates_a_master_key() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("master-key.json");
        let manager = MasterKeyManager::new(&path, ContextProtector(17));
        let first = manager.load_or_create().unwrap();
        assert_eq!(manager.unlock().unwrap().expose(), first.expose());

        let rotated = manager
            .rotate(|old, new| {
                assert_ne!(old.expose(), new.expose());
                Ok(())
            })
            .unwrap();
        assert_eq!(manager.unlock().unwrap().expose(), rotated.expose());
    }

    #[test]
    fn copied_key_material_cannot_be_unlocked_by_another_user_context() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("master-key.json");
        MasterKeyManager::new(&path, ContextProtector(17))
            .load_or_create()
            .unwrap();

        let other_user = MasterKeyManager::new(&path, ContextProtector(42));
        assert!(matches!(
            other_user.unlock(),
            Err(KeyProtectionError::UnlockFailed)
        ));
    }

    #[test]
    fn failed_reencryption_keeps_the_existing_wrapped_key() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("master-key.json");
        let manager = MasterKeyManager::new(&path, ContextProtector(17));
        let first = manager.load_or_create().unwrap();
        let result = manager.rotate(|_, _| Err(KeyProtectionError::RotationFailed));

        assert!(matches!(result, Err(KeyProtectionError::RotationFailed)));
        assert_eq!(manager.unlock().unwrap().expose(), first.expose());
    }

    #[cfg(windows)]
    #[test]
    fn windows_dpapi_current_user_round_trip_hides_plaintext() {
        let plaintext = b"xinxiang-dpapi-test-key-material";
        let protected = DpapiCurrentUserProtector.protect(plaintext).unwrap();
        assert_ne!(protected, plaintext);
        assert_eq!(
            DpapiCurrentUserProtector
                .unprotect(&protected)
                .unwrap()
                .as_slice(),
            plaintext
        );
    }
}

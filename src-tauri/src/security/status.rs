use std::path::Path;

use crate::{contracts::SecurityStatusV1, storage::database::EncryptedDatabase};

use super::key_protection::{DpapiCurrentUserProtector, MasterKeyManager};

#[derive(Debug)]
pub enum SecurityStatusError {
    Key,
    Database,
}

impl std::fmt::Display for SecurityStatusError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LOCAL_SECURITY_CHECK_FAILED")
    }
}

impl std::error::Error for SecurityStatusError {}

pub fn verify_local_security(
    app_data_directory: &Path,
) -> Result<SecurityStatusV1, SecurityStatusError> {
    let key_path = app_data_directory.join("security").join("master-key.json");
    let manager = MasterKeyManager::new(key_path, DpapiCurrentUserProtector);
    let master_key = manager
        .load_or_create()
        .map_err(|_| SecurityStatusError::Key)?;
    EncryptedDatabase::open(&app_data_directory.join("inbox.db"), &master_key)
        .map_err(|_| SecurityStatusError::Database)?;
    Ok(SecurityStatusV1::verified())
}

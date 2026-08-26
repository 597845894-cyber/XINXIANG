use std::path::Path;

use crate::{contracts::SecurityStatusV1, storage::database::EncryptedDatabase};

use super::key_protection::{DpapiCurrentUserProtector, MasterKeyManager};

pub fn verify_local_security(app_data_directory: &Path) -> Result<SecurityStatusV1, ()> {
    let key_path = app_data_directory.join("security").join("master-key.json");
    let manager = MasterKeyManager::new(key_path, DpapiCurrentUserProtector);
    let master_key = manager.load_or_create().map_err(|_| ())?;
    EncryptedDatabase::open(&app_data_directory.join("inbox.db"), &master_key).map_err(|_| ())?;
    Ok(SecurityStatusV1::verified())
}

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

pub const SELECTED_MODELS_MANIFEST: &str =
    include_str!("../resources/models/selected-models.lock.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelectionManifestV1 {
    pub schema_version: u16,
    pub selection_id: String,
    pub components: Vec<ModelComponentV1>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelComponentV1 {
    pub resource_id: String,
    pub kind: String,
    pub files: Vec<ModelFileV1>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFileV1 {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelResourceState {
    Available,
    Missing,
    Corrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelResourceIssueV1 {
    pub resource_id: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelResourceStatusV1 {
    pub schema_version: u16,
    pub selection_id: String,
    pub state: ModelResourceState,
    pub issues: Vec<ModelResourceIssueV1>,
    pub recovery_action: Option<String>,
    pub network_required: bool,
}

pub fn selected_manifest() -> Result<ModelSelectionManifestV1, String> {
    let manifest: ModelSelectionManifestV1 = serde_json::from_str(SELECTED_MODELS_MANIFEST)
        .map_err(|error| format!("MODEL_MANIFEST_INVALID: {error}"))?;
    if manifest.schema_version != 1 {
        return Err("MODEL_MANIFEST_VERSION_UNSUPPORTED".to_owned());
    }
    for component in &manifest.components {
        if !matches!(component.kind.as_str(), "ocr" | "semantic" | "runtime") {
            return Err("MODEL_MANIFEST_COMPONENT_KIND_INVALID".to_owned());
        }
        for file in &component.files {
            validate_relative_path(&file.path)?;
            if file.sha256.len() != 64 || !file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err("MODEL_MANIFEST_HASH_INVALID".to_owned());
            }
        }
    }
    Ok(manifest)
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("MODEL_MANIFEST_PATH_INVALID".to_owned());
    }
    Ok(())
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn inspect_resources(
    root: &Path,
    manifest: &ModelSelectionManifestV1,
) -> ModelResourceStatusV1 {
    let mut issues = Vec::new();
    let mut has_corrupt_file = false;
    for component in &manifest.components {
        for expected in &component.files {
            let path = root.join(&expected.path);
            let metadata = match fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() => metadata,
                _ => {
                    issues.push(ModelResourceIssueV1 {
                        resource_id: component.resource_id.clone(),
                        path: expected.path.clone(),
                        reason: "missing".to_owned(),
                    });
                    continue;
                }
            };
            let digest = sha256_file(&path).ok();
            if metadata.len() != expected.size
                || digest.as_deref() != Some(expected.sha256.to_ascii_lowercase().as_str())
            {
                has_corrupt_file = true;
                issues.push(ModelResourceIssueV1 {
                    resource_id: component.resource_id.clone(),
                    path: expected.path.clone(),
                    reason: "size-or-sha256-mismatch".to_owned(),
                });
            }
        }
    }
    let state = if issues.is_empty() {
        ModelResourceState::Available
    } else if has_corrupt_file {
        ModelResourceState::Corrupt
    } else {
        ModelResourceState::Missing
    };
    ModelResourceStatusV1 {
        schema_version: manifest.schema_version,
        selection_id: manifest.selection_id.clone(),
        state,
        issues,
        recovery_action: (state != ModelResourceState::Available)
            .then(|| "请选择包含完整模型资源的本地文件夹重新安装；此操作不需要联网。".to_owned()),
        network_required: false,
    }
}

fn copy_expected_files(
    source: &Path,
    destination: &Path,
    manifest: &ModelSelectionManifestV1,
) -> Result<(), String> {
    for component in &manifest.components {
        for expected in &component.files {
            let source_file = source.join(&expected.path);
            let destination_file = destination.join(&expected.path);
            if let Some(parent) = destination_file.parent() {
                fs::create_dir_all(parent).map_err(|_| "MODEL_INSTALL_CREATE_FAILED")?;
            }
            fs::copy(source_file, destination_file).map_err(|_| "MODEL_INSTALL_COPY_FAILED")?;
        }
    }
    Ok(())
}

pub fn install_resources(
    source: &Path,
    target: &Path,
    manifest: &ModelSelectionManifestV1,
) -> Result<ModelResourceStatusV1, String> {
    let source_status = inspect_resources(source, manifest);
    if source_status.state != ModelResourceState::Available {
        return Err("MODEL_INSTALL_SOURCE_INVALID".to_owned());
    }
    let parent = target
        .parent()
        .ok_or_else(|| "MODEL_INSTALL_TARGET_INVALID".to_owned())?;
    fs::create_dir_all(parent).map_err(|_| "MODEL_INSTALL_CREATE_FAILED".to_owned())?;
    let staging = parent.join(format!(".{}.installing", manifest.selection_id));
    let backup = parent.join(format!(".{}.previous", manifest.selection_id));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|_| "MODEL_INSTALL_CLEANUP_FAILED".to_owned())?;
    }
    if backup.exists() {
        fs::remove_dir_all(&backup).map_err(|_| "MODEL_INSTALL_CLEANUP_FAILED".to_owned())?;
    }
    fs::create_dir_all(&staging).map_err(|_| "MODEL_INSTALL_CREATE_FAILED".to_owned())?;
    if let Err(error) = copy_expected_files(source, &staging, manifest) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let staged_status = inspect_resources(&staging, manifest);
    if staged_status.state != ModelResourceState::Available {
        let _ = fs::remove_dir_all(&staging);
        return Err("MODEL_INSTALL_VERIFICATION_FAILED".to_owned());
    }
    if target.exists() {
        fs::rename(target, &backup).map_err(|_| "MODEL_INSTALL_SWAP_FAILED".to_owned())?;
    }
    if fs::rename(&staging, target).is_err() {
        if backup.exists() {
            let _ = fs::rename(&backup, target);
        }
        return Err("MODEL_INSTALL_SWAP_FAILED".to_owned());
    }
    if backup.exists() {
        fs::remove_dir_all(backup).map_err(|_| "MODEL_INSTALL_CLEANUP_FAILED".to_owned())?;
    }
    Ok(inspect_resources(target, manifest))
}

pub fn install_root(app_data_dir: &Path, selection_id: &str) -> PathBuf {
    app_data_dir.join("model-resources").join(selection_id)
}

#[cfg(test)]
mod tests {
    use super::{
        inspect_resources, install_resources, selected_manifest, ModelComponentV1, ModelFileV1,
        ModelResourceState, ModelSelectionManifestV1,
    };
    use sha2::{Digest, Sha256};
    use std::fs;
    use tempfile::tempdir;

    fn fixture_manifest(content: &[u8]) -> ModelSelectionManifestV1 {
        ModelSelectionManifestV1 {
            schema_version: 1,
            selection_id: "fixture-v1".to_owned(),
            components: vec![ModelComponentV1 {
                resource_id: "fixture.model".to_owned(),
                kind: "semantic".to_owned(),
                files: vec![ModelFileV1 {
                    path: "semantic/model.gguf".to_owned(),
                    size: content.len() as u64,
                    sha256: format!("{:x}", Sha256::digest(content)),
                }],
            }],
        }
    }

    #[test]
    fn distinguishes_missing_corrupt_and_available_resources() {
        let directory = tempdir().unwrap();
        let content = b"offline-model-fixture";
        let manifest = fixture_manifest(content);
        let missing = inspect_resources(directory.path(), &manifest);
        assert_eq!(missing.state, ModelResourceState::Missing);
        assert!(!missing.network_required);
        let model = directory.path().join("semantic/model.gguf");
        fs::create_dir_all(model.parent().unwrap()).unwrap();
        fs::write(&model, b"tampered").unwrap();
        assert_eq!(
            inspect_resources(directory.path(), &manifest).state,
            ModelResourceState::Corrupt
        );
        fs::write(model, content).unwrap();
        assert_eq!(
            inspect_resources(directory.path(), &manifest).state,
            ModelResourceState::Available
        );
    }

    #[test]
    fn installs_a_verified_local_resource_without_network_access() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("source");
        let target = directory.path().join("installed/fixture-v1");
        let content = b"offline-model-fixture";
        let manifest = fixture_manifest(content);
        fs::create_dir_all(source.join("semantic")).unwrap();
        fs::write(source.join("semantic/model.gguf"), content).unwrap();
        let status = install_resources(&source, &target, &manifest).unwrap();
        assert_eq!(status.state, ModelResourceState::Available);
        assert!(!status.network_required);
        assert_eq!(
            fs::read(target.join("semantic/model.gguf")).unwrap(),
            content
        );
    }

    #[test]
    fn rejects_an_incomplete_source_without_changing_an_existing_install() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("source");
        let target = directory.path().join("installed/fixture-v1");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("keep.txt"), b"existing").unwrap();
        let manifest = fixture_manifest(b"offline-model-fixture");
        assert_eq!(
            install_resources(&source, &target, &manifest).unwrap_err(),
            "MODEL_INSTALL_SOURCE_INVALID"
        );
        assert_eq!(fs::read(target.join("keep.txt")).unwrap(), b"existing");
    }

    #[test]
    fn selected_manifest_is_versioned_and_offline_recovery_is_explicit() {
        let directory = tempdir().unwrap();
        let manifest = selected_manifest().unwrap();
        let status = inspect_resources(directory.path(), &manifest);
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.components.len(), 3);
        assert_eq!(status.state, ModelResourceState::Missing);
        assert!(status.recovery_action.unwrap().contains("不需要联网"));
        assert!(!status.network_required);
    }
}

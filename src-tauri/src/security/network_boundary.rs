#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdaterState {
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicComponentVersion {
    ApplicationV1,
    ModelManifestV1,
}

pub struct OptionalUpdaterBoundary;

impl OptionalUpdaterBoundary {
    pub fn state(&self) -> UpdaterState {
        UpdaterState::Disabled
    }

    pub fn allows_metadata(&self, _component: PublicComponentVersion) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{OptionalUpdaterBoundary, PublicComponentVersion, UpdaterState};

    #[test]
    fn updater_is_disabled_and_accepts_no_business_metadata() {
        let boundary = OptionalUpdaterBoundary;
        assert_eq!(boundary.state(), UpdaterState::Disabled);
        assert!(!boundary.allows_metadata(PublicComponentVersion::ApplicationV1));
        assert!(!boundary.allows_metadata(PublicComponentVersion::ModelManifestV1));
    }

    #[test]
    fn business_core_has_no_direct_network_client_or_updater_access() {
        let manifest =
            fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
        for forbidden in [
            "req".to_owned() + "west",
            "u".to_owned() + "req",
            "hy".to_owned() + "per",
        ] {
            assert!(!manifest.contains(&forbidden));
        }

        let source_root = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        assert_no_direct_network_calls(std::path::Path::new(source_root));
    }

    fn assert_no_direct_network_calls(directory: &std::path::Path) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                assert_no_direct_network_calls(&path);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
                continue;
            }
            let source = fs::read_to_string(path).unwrap();
            for forbidden in [
                "Tcp".to_owned() + "Stream",
                "Web".to_owned() + "Socket",
                "http".to_owned() + "://",
                "https".to_owned() + "://",
            ] {
                assert!(!source.contains(&forbidden));
            }
        }
    }
}

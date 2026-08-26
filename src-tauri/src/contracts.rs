use serde::{Deserialize, Serialize};

pub const CONTRACT_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppRouteId {
    Inbox,
    QuickImport,
    Review,
    Tasks,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandName {
    GetAppBootstrap,
    GetSecurityStatus,
    GetModelResourceStatus,
    InstallModelResources,
    OpenQuickImport,
    QuitApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventName {
    QuickImportRequested,
    WindowHiddenToTray,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteDescriptorV1 {
    pub id: AppRouteId,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppBootstrapV1 {
    pub contract_version: u16,
    pub app_version: String,
    pub routes: Vec<RouteDescriptorV1>,
    pub commands: Vec<CommandName>,
    pub events: Vec<EventName>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityStatusV1 {
    pub schema_version: u16,
    pub master_key: String,
    pub database: String,
    pub attachments: String,
    pub business_networking: String,
    pub updater: String,
}

impl SecurityStatusV1 {
    pub fn verified() -> Self {
        Self {
            schema_version: CONTRACT_VERSION,
            master_key: "currentWindowsUserProtected".to_owned(),
            database: "sqlCipherVerified".to_owned(),
            attachments: "aes256Gcm".to_owned(),
            business_networking: "blocked".to_owned(),
            updater: "disabled".to_owned(),
        }
    }
}

impl AppBootstrapV1 {
    pub fn current() -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            routes: vec![
                RouteDescriptorV1 {
                    id: AppRouteId::Inbox,
                    label: "收件箱".to_owned(),
                },
                RouteDescriptorV1 {
                    id: AppRouteId::QuickImport,
                    label: "快速导入".to_owned(),
                },
                RouteDescriptorV1 {
                    id: AppRouteId::Review,
                    label: "任务核对".to_owned(),
                },
                RouteDescriptorV1 {
                    id: AppRouteId::Tasks,
                    label: "任务表".to_owned(),
                },
                RouteDescriptorV1 {
                    id: AppRouteId::Settings,
                    label: "设置".to_owned(),
                },
            ],
            commands: vec![
                CommandName::GetAppBootstrap,
                CommandName::GetSecurityStatus,
                CommandName::GetModelResourceStatus,
                CommandName::InstallModelResources,
                CommandName::OpenQuickImport,
                CommandName::QuitApp,
            ],
            events: vec![
                EventName::QuickImportRequested,
                EventName::WindowHiddenToTray,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppBootstrapV1;

    #[test]
    fn serialization_matches_the_shared_v1_fixture() {
        let fixture = include_str!("../../contracts/v1/app-bootstrap.json");
        let fixture_value: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let rust_value = serde_json::to_value(AppBootstrapV1::current()).unwrap();

        assert_eq!(rust_value, fixture_value);
    }

    #[test]
    fn shared_v1_fixture_deserializes_in_rust() {
        let fixture = include_str!("../../contracts/v1/app-bootstrap.json");
        let contract: AppBootstrapV1 = serde_json::from_str(fixture).unwrap();

        assert_eq!(contract, AppBootstrapV1::current());
    }
}

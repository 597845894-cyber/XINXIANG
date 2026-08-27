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
    ImportTextNotice,
    ImportImageNotice,
    ListNotices,
    GetNoticeDetail,
    GetNoticeImagePreview,
    UpdateNoticePublishedTime,
    SetNoticeState,
    AnalyzeNotice,
    CancelAnalysis,
    ListReviewCandidates,
    EditTaskCandidate,
    ConfirmTaskCandidate,
    IgnoreTaskCandidate,
    MergeTaskCandidates,
    SplitTaskCandidate,
    ListTasks,
    CreateManualTask,
    UpdateTask,
    SetTaskState,
    GetTaskHistory,
    SuggestNoticeRelations,
    ListNoticeRelations,
    ResolveNoticeRelation,
    OpenQuickImport,
    QuitApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventName {
    QuickImportRequested,
    WindowHiddenToTray,
    RelativeDateRecalculationRequested,
    AnalysisProgress,
    AnalysisCompleted,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoticeStateV1 {
    PendingAnalysis,
    PendingReview,
    PartiallyProcessed,
    Processed,
    InformationOnly,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeSummaryV1 {
    pub id: String,
    pub source_kind: String,
    pub inbox_state: NoticeStateV1,
    pub published_at: String,
    pub published_time_source: String,
    pub published_time_candidate: Option<String>,
    pub published_time_candidate_source: Option<String>,
    pub created_at: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAssetInfoV1 {
    pub id: String,
    pub media_type: String,
    pub byte_size: usize,
    pub pixel_width: Option<u32>,
    pub pixel_height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeDetailV1 {
    #[serde(flatten)]
    pub summary: NoticeSummaryV1,
    pub original_text: Option<String>,
    pub source_asset: Option<SourceAssetInfoV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePreviewV1 {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgressV1 {
    pub notice_id: String,
    pub stage: String,
    pub progress_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateViewV1 {
    pub id: String,
    pub notice_id: String,
    pub analysis_revision_id: String,
    pub state: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskViewV1 {
    pub id: String,
    pub notice_id: Option<String>,
    pub state: String,
    pub payload: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeRelationViewV1 {
    pub id: String,
    pub notice_id: String,
    pub related_notice_id: String,
    pub relation_type: String,
    pub relation_state: String,
    pub evidence: serde_json::Value,
    pub created_at: String,
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
                CommandName::ImportTextNotice,
                CommandName::ImportImageNotice,
                CommandName::ListNotices,
                CommandName::GetNoticeDetail,
                CommandName::GetNoticeImagePreview,
                CommandName::UpdateNoticePublishedTime,
                CommandName::SetNoticeState,
                CommandName::AnalyzeNotice,
                CommandName::CancelAnalysis,
                CommandName::ListReviewCandidates,
                CommandName::EditTaskCandidate,
                CommandName::ConfirmTaskCandidate,
                CommandName::IgnoreTaskCandidate,
                CommandName::MergeTaskCandidates,
                CommandName::SplitTaskCandidate,
                CommandName::ListTasks,
                CommandName::CreateManualTask,
                CommandName::UpdateTask,
                CommandName::SetTaskState,
                CommandName::GetTaskHistory,
                CommandName::SuggestNoticeRelations,
                CommandName::ListNoticeRelations,
                CommandName::ResolveNoticeRelation,
                CommandName::OpenQuickImport,
                CommandName::QuitApp,
            ],
            events: vec![
                EventName::QuickImportRequested,
                EventName::WindowHiddenToTray,
                EventName::RelativeDateRecalculationRequested,
                EventName::AnalysisProgress,
                EventName::AnalysisCompleted,
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

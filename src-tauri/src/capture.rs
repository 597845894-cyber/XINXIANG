use std::{io::Cursor, path::Path};

use serde::Serialize;
use uuid::Uuid;

use crate::{
    contracts::{
        CandidateViewV1, ImagePreviewV1, NoticeDetailV1, NoticeRelationViewV1, NoticeStateV1,
        NoticeSummaryV1, ReminderViewV1, SourceAssetInfoV1, TaskViewV1,
    },
    security::key_protection::{DpapiCurrentUserProtector, MasterKeyManager},
    storage::{
        attachments::AttachmentStore,
        database::EncryptedDatabase,
        repository::{
            CandidateState, NewCandidate, NoticeDetail, NoticeRelationRecord, NoticeRepository,
            NoticeState, NoticeSummary, PublishedTimeCandidate, ReminderRecord, SourceAsset,
            TaskState,
        },
    },
};

const MAX_IMAGE_BYTES: usize = 15 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 40_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageValidationV1 {
    pub media_type: String,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

#[derive(Debug)]
pub enum CaptureError {
    EmptyText,
    InvalidPublishedTime,
    ImageTooLarge,
    UnsupportedImage,
    InvalidImage,
    Storage,
    MissingNotice,
    InvalidPayload,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::EmptyText => "NOTICE_TEXT_REQUIRED",
            Self::InvalidPublishedTime => "NOTICE_PUBLISHED_TIME_INVALID",
            Self::ImageTooLarge => "NOTICE_IMAGE_TOO_LARGE",
            Self::UnsupportedImage => "NOTICE_IMAGE_UNSUPPORTED",
            Self::InvalidImage => "NOTICE_IMAGE_INVALID",
            Self::Storage => "NOTICE_STORAGE_FAILED",
            Self::MissingNotice => "NOTICE_NOT_FOUND",
            Self::InvalidPayload => "TASK_PAYLOAD_INVALID",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for CaptureError {}

pub fn import_text(
    app_data_directory: &Path,
    original_text: String,
    published_at: String,
) -> Result<NoticeSummaryV1, CaptureError> {
    if original_text.trim().is_empty() {
        return Err(CaptureError::EmptyText);
    }
    validate_published_time(&published_at)?;
    let database = open_database(app_data_directory)?;
    let repository = NoticeRepository::new(database.connection());
    let notice_id = new_id();
    repository
        .create_text_notice(&notice_id, &original_text, &published_at)
        .map_err(|_| CaptureError::Storage)?;
    summary_for(&repository, &notice_id)
}

pub fn validate_image(
    bytes: &[u8],
    declared_media_type: Option<&str>,
) -> Result<ImageValidationV1, CaptureError> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(CaptureError::ImageTooLarge);
    }
    if bytes.is_empty() {
        return Err(CaptureError::InvalidImage);
    }
    let (media_type, width, height) = image_dimensions(bytes)?;
    if let Some(declared) = declared_media_type.filter(|value| !value.is_empty()) {
        if declared != media_type {
            return Err(CaptureError::UnsupportedImage);
        }
    }
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err(CaptureError::InvalidImage);
    }
    Ok(ImageValidationV1 {
        media_type: media_type.to_owned(),
        pixel_width: width,
        pixel_height: height,
    })
}

pub fn import_image(
    app_data_directory: &Path,
    bytes: Vec<u8>,
    declared_media_type: Option<String>,
    published_at: String,
) -> Result<NoticeSummaryV1, CaptureError> {
    validate_published_time(&published_at)?;
    let validation = validate_image(&bytes, declared_media_type.as_deref())?;
    let time_candidate = extract_published_time_candidate(&bytes);
    let byte_size = bytes.len();
    let master_key = load_key(app_data_directory)?;
    let database = EncryptedDatabase::open(&app_data_directory.join("inbox.db"), &master_key)
        .map_err(|_| CaptureError::Storage)?;
    let notice_id = new_id();
    let asset_id = new_id();
    let store = AttachmentStore::new(app_data_directory.join("attachments"));
    let metadata = store
        .write_from_reader(&asset_id, &mut Cursor::new(bytes), &master_key)
        .map_err(|_| CaptureError::Storage)?;
    let asset = SourceAsset {
        id: asset_id.clone(),
        media_type: validation.media_type,
        encrypted_file_name: format!("{asset_id}.enc"),
        metadata,
        byte_size,
        pixel_width: Some(validation.pixel_width),
        pixel_height: Some(validation.pixel_height),
    };
    let repository = NoticeRepository::new(database.connection());
    if repository
        .create_image_notice(&notice_id, &published_at, time_candidate.as_ref(), &asset)
        .is_err()
    {
        store.remove(&asset_id);
        return Err(CaptureError::Storage);
    }
    summary_for(&repository, &notice_id)
}

pub fn list_notices(
    app_data_directory: &Path,
    state: Option<NoticeStateV1>,
) -> Result<Vec<NoticeSummaryV1>, CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .list_notices(state.map(from_contract_state))
        .map(|notices| notices.into_iter().map(summary_to_contract).collect())
        .map_err(|_| CaptureError::Storage)
}

pub fn notice_detail(
    app_data_directory: &Path,
    notice_id: &str,
) -> Result<NoticeDetailV1, CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .notice_detail(notice_id)
        .map(detail_to_contract)
        .map_err(map_repository_error)
}

pub fn image_preview(
    app_data_directory: &Path,
    notice_id: &str,
) -> Result<ImagePreviewV1, CaptureError> {
    let master_key = load_key(app_data_directory)?;
    let database = EncryptedDatabase::open(&app_data_directory.join("inbox.db"), &master_key)
        .map_err(|_| CaptureError::Storage)?;
    let detail = NoticeRepository::new(database.connection())
        .notice_detail(notice_id)
        .map_err(map_repository_error)?;
    let asset = detail.source_asset.ok_or(CaptureError::MissingNotice)?;
    let mut bytes = Vec::with_capacity(asset.byte_size);
    AttachmentStore::new(app_data_directory.join("attachments"))
        .read_to_writer(&asset.id, &asset.metadata, &mut bytes, &master_key)
        .map_err(|_| CaptureError::Storage)?;
    Ok(ImagePreviewV1 {
        media_type: asset.media_type,
        bytes,
    })
}

pub fn update_published_time(
    app_data_directory: &Path,
    notice_id: &str,
    published_at: String,
) -> Result<(), CaptureError> {
    validate_published_time(&published_at)?;
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .update_published_time(notice_id, &published_at)
        .map_err(map_repository_error)
}

pub fn set_notice_state(
    app_data_directory: &Path,
    notice_id: &str,
    state: NoticeStateV1,
) -> Result<(), CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .set_notice_state(notice_id, from_contract_state(state))
        .map_err(map_repository_error)
}

#[derive(Debug, Clone)]
pub enum AnalysisInput {
    Text(String),
    Image { bytes: Vec<u8>, media_type: String },
}

pub fn analysis_input(
    app_data_directory: &Path,
    notice_id: &str,
) -> Result<(AnalysisInput, String), CaptureError> {
    let master_key = load_key(app_data_directory)?;
    let database = EncryptedDatabase::open(&app_data_directory.join("inbox.db"), &master_key)
        .map_err(|_| CaptureError::Storage)?;
    let detail = NoticeRepository::new(database.connection())
        .notice_detail(notice_id)
        .map_err(map_repository_error)?;
    let published_at = detail.summary.published_at.clone();
    if let Some(text) = detail.original_text {
        return Ok((AnalysisInput::Text(text), published_at));
    }
    let asset = detail.source_asset.ok_or(CaptureError::MissingNotice)?;
    let mut bytes = Vec::with_capacity(asset.byte_size);
    AttachmentStore::new(app_data_directory.join("attachments"))
        .read_to_writer(&asset.id, &asset.metadata, &mut bytes, &master_key)
        .map_err(|_| CaptureError::Storage)?;
    Ok((
        AnalysisInput::Image {
            bytes,
            media_type: asset.media_type,
        },
        published_at,
    ))
}

fn open_database(app_data_directory: &Path) -> Result<EncryptedDatabase, CaptureError> {
    let master_key = load_key(app_data_directory)?;
    EncryptedDatabase::open(&app_data_directory.join("inbox.db"), &master_key)
        .map_err(|_| CaptureError::Storage)
}

pub fn open_database_for_analysis(
    app_data_directory: &Path,
) -> Result<EncryptedDatabase, CaptureError> {
    open_database(app_data_directory)
}

pub fn list_review_candidates(
    app_data_directory: &Path,
) -> Result<Vec<CandidateViewV1>, CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .list_candidates(Some(CandidateState::Pending))
        .map(|candidates| {
            candidates
                .into_iter()
                .filter_map(candidate_to_contract)
                .collect()
        })
        .map_err(|_| CaptureError::Storage)
}

pub fn edit_task_candidate(
    app_data_directory: &Path,
    candidate_id: &str,
    payload: serde_json::Value,
) -> Result<(), CaptureError> {
    let payload = validated_payload(payload)?;
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .edit_candidate(candidate_id, &payload)
        .map_err(map_repository_error)
}

pub fn confirm_task_candidate(
    app_data_directory: &Path,
    candidate_id: &str,
    payload: serde_json::Value,
) -> Result<TaskViewV1, CaptureError> {
    let payload = validated_payload(payload)?;
    let task_id = new_id();
    let database = open_database(app_data_directory)?;
    let repository = NoticeRepository::new(database.connection());
    repository
        .confirm_candidate(candidate_id, &task_id, &new_id(), &payload)
        .map_err(map_repository_error)?;
    repository
        .list_tasks()
        .map_err(map_repository_error)?
        .into_iter()
        .find(|task| task.id == task_id)
        .and_then(task_to_contract)
        .ok_or(CaptureError::Storage)
}

pub fn ignore_task_candidate(
    app_data_directory: &Path,
    candidate_id: &str,
) -> Result<(), CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .set_candidate_state(candidate_id, CandidateState::Ignored)
        .map_err(map_repository_error)
}

pub fn merge_task_candidates(
    app_data_directory: &Path,
    target_id: &str,
    source_ids: &[String],
    payload: serde_json::Value,
) -> Result<(), CaptureError> {
    let payload = validated_payload(payload)?;
    let database = open_database(app_data_directory)?;
    let sources = source_ids.iter().map(String::as_str).collect::<Vec<_>>();
    NoticeRepository::new(database.connection())
        .merge_candidates(target_id, &sources, &payload)
        .map_err(map_repository_error)
}

pub fn split_task_candidate(
    app_data_directory: &Path,
    candidate_id: &str,
    payloads: Vec<serde_json::Value>,
) -> Result<(), CaptureError> {
    if payloads.len() < 2 {
        return Err(CaptureError::Storage);
    }
    let serialized = payloads
        .into_iter()
        .map(validated_payload)
        .collect::<Result<Vec<_>, _>>()?;
    let ids = (0..serialized.len()).map(|_| new_id()).collect::<Vec<_>>();
    let candidates = ids
        .iter()
        .zip(serialized.iter())
        .map(|(id, payload)| NewCandidate { id, payload })
        .collect::<Vec<_>>();
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .split_candidate(candidate_id, &candidates)
        .map_err(map_repository_error)
}

pub fn list_tasks(app_data_directory: &Path) -> Result<Vec<TaskViewV1>, CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .list_tasks()
        .map(|tasks| tasks.into_iter().filter_map(task_to_contract).collect())
        .map_err(|_| CaptureError::Storage)
}

pub fn create_manual_task(
    app_data_directory: &Path,
    payload: serde_json::Value,
) -> Result<TaskViewV1, CaptureError> {
    let payload = validated_payload(payload)?;
    let task_id = new_id();
    let database = open_database(app_data_directory)?;
    let repository = NoticeRepository::new(database.connection());
    repository
        .create_manual_task(&task_id, &new_id(), &payload)
        .map_err(map_repository_error)?;
    repository
        .list_tasks()
        .map_err(map_repository_error)?
        .into_iter()
        .find(|task| task.id == task_id)
        .and_then(task_to_contract)
        .ok_or(CaptureError::Storage)
}

pub fn update_task(
    app_data_directory: &Path,
    task_id: &str,
    payload: serde_json::Value,
) -> Result<(), CaptureError> {
    let payload = validated_payload(payload)?;
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .transition_task(task_id, TaskState::Todo, &payload, &new_id())
        .map_err(map_repository_error)
}

pub fn set_task_state(
    app_data_directory: &Path,
    task_id: &str,
    state: &str,
    payload: serde_json::Value,
) -> Result<(), CaptureError> {
    let payload = validated_payload(payload)?;
    let state = match state {
        "todo" => TaskState::Todo,
        "completed" => TaskState::Completed,
        "cancelled" => TaskState::Cancelled,
        _ => return Err(CaptureError::Storage),
    };
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .transition_task(task_id, state, &payload, &new_id())
        .map_err(map_repository_error)
}

pub fn list_reminders(
    app_data_directory: &Path,
    task_id: Option<&str>,
) -> Result<Vec<ReminderViewV1>, CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .list_reminders(task_id)
        .map_err(map_repository_error)
        .map(|rows| rows.into_iter().map(reminder_to_contract).collect())
}

pub fn upsert_reminder(
    app_data_directory: &Path,
    task_id: &str,
    reminder_id: String,
    scheduled_at: String,
    idempotency_key: String,
) -> Result<ReminderViewV1, CaptureError> {
    validate_published_time(&scheduled_at)?;
    let record = ReminderRecord {
        id: reminder_id,
        task_id: task_id.to_owned(),
        scheduled_at,
        reminder_state: "pending".to_owned(),
        idempotency_key,
        created_at: String::new(),
    };
    let database = open_database(app_data_directory)?;
    let repository = NoticeRepository::new(database.connection());
    repository
        .upsert_reminder(&record)
        .map_err(map_repository_error)?;
    repository
        .list_reminders(Some(task_id))
        .map_err(map_repository_error)?
        .into_iter()
        .find(|item| item.id == record.id || item.idempotency_key == record.idempotency_key)
        .map(reminder_to_contract)
        .ok_or(CaptureError::Storage)
}

pub fn delete_reminder(app_data_directory: &Path, reminder_id: &str) -> Result<(), CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .delete_reminder(reminder_id)
        .map_err(map_repository_error)
}

pub fn claim_due_reminders(app_data_directory: &Path) -> Result<Vec<ReminderRecord>, CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .claim_due_reminders()
        .map_err(map_repository_error)
}

pub fn task_history(
    app_data_directory: &Path,
    task_id: &str,
) -> Result<Vec<serde_json::Value>, CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .task_history(task_id)
        .map_err(map_repository_error)?
        .into_iter()
        .map(|payload| serde_json::from_slice(&payload).map_err(|_| CaptureError::Storage))
        .collect()
}

pub fn suggest_notice_relations(
    app_data_directory: &Path,
    notice_id: &str,
) -> Result<Vec<NoticeRelationViewV1>, CaptureError> {
    let database = open_database(app_data_directory)?;
    let repository = NoticeRepository::new(database.connection());
    let candidates = repository
        .list_candidates(None)
        .map_err(map_repository_error)?;
    let titles = candidates
        .iter()
        .filter_map(|candidate| {
            payload_title(&candidate.payload).map(|title| (candidate.notice_id.as_str(), title))
        })
        .collect::<Vec<_>>();
    for (other_notice_id, title) in titles.iter().filter(|(id, _)| *id == notice_id) {
        let _ = other_notice_id;
        let normalized = normalize_key(title);
        for (comparison_notice_id, comparison_title) in
            titles.iter().filter(|(id, _)| *id != notice_id)
        {
            if normalized == normalize_key(comparison_title) {
                let evidence = serde_json::to_vec(&serde_json::json!({
                    "matchedTitle": title,
                    "otherTitle": comparison_title,
                    "reason": "normalizedTitle",
                }))
                .map_err(|_| CaptureError::Storage)?;
                let relation = NoticeRelationRecord {
                    id: new_id(),
                    notice_id: notice_id.to_owned(),
                    related_notice_id: (*comparison_notice_id).to_owned(),
                    relation_type: "duplicate".to_owned(),
                    relation_state: "suggested".to_owned(),
                    evidence,
                    created_at: String::new(),
                };
                let _ = repository.create_relation(&relation);
            }
        }
    }
    list_notice_relations(app_data_directory, Some(notice_id))
}

pub fn list_notice_relations(
    app_data_directory: &Path,
    notice_id: Option<&str>,
) -> Result<Vec<NoticeRelationViewV1>, CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .list_relations(notice_id)
        .map(|relations| {
            relations
                .into_iter()
                .filter_map(relation_to_contract)
                .collect()
        })
        .map_err(|_| CaptureError::Storage)
}

pub fn resolve_notice_relation(
    app_data_directory: &Path,
    relation_id: &str,
    accepted: bool,
) -> Result<(), CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .set_relation_state(relation_id, if accepted { "accepted" } else { "rejected" })
        .map_err(map_repository_error)
}

fn validated_payload(payload: serde_json::Value) -> Result<Vec<u8>, CaptureError> {
    if !payload.is_object()
        || payload
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .is_none()
    {
        return Err(CaptureError::InvalidPayload);
    }
    serde_json::to_vec(&payload).map_err(|_| CaptureError::InvalidPayload)
}

fn candidate_to_contract(
    candidate: crate::storage::repository::CandidateRecord,
) -> Option<CandidateViewV1> {
    let payload = serde_json::from_slice(&candidate.payload).ok()?;
    Some(CandidateViewV1 {
        id: candidate.id,
        notice_id: candidate.notice_id,
        analysis_revision_id: candidate.analysis_revision_id,
        state: candidate.state.as_db().to_owned(),
        payload,
        created_at: candidate.created_at,
    })
}

fn task_to_contract(task: crate::storage::repository::TaskRecord) -> Option<TaskViewV1> {
    let payload = serde_json::from_slice(&task.payload).ok()?;
    Some(TaskViewV1 {
        id: task.id,
        notice_id: task.notice_id,
        state: task.state.as_db().to_owned(),
        payload,
        created_at: task.created_at,
        updated_at: task.updated_at,
    })
}

fn reminder_to_contract(reminder: ReminderRecord) -> ReminderViewV1 {
    ReminderViewV1 {
        id: reminder.id,
        task_id: reminder.task_id,
        scheduled_at: reminder.scheduled_at,
        reminder_state: reminder.reminder_state,
        idempotency_key: reminder.idempotency_key,
        created_at: reminder.created_at,
    }
}

fn relation_to_contract(relation: NoticeRelationRecord) -> Option<NoticeRelationViewV1> {
    let evidence =
        serde_json::from_slice(&relation.evidence).unwrap_or_else(|_| serde_json::json!({}));
    Some(NoticeRelationViewV1 {
        id: relation.id,
        notice_id: relation.notice_id,
        related_notice_id: relation.related_notice_id,
        relation_type: relation.relation_type,
        relation_state: relation.relation_state,
        evidence,
        created_at: relation.created_at,
    })
}

fn payload_title(payload: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(payload)
        .ok()?
        .get("title")?
        .as_str()
        .map(str::to_owned)
}

fn normalize_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_whitespace() && !matches!(character, '，' | ',' | '。' | '.')
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn load_key(
    app_data_directory: &Path,
) -> Result<crate::security::key_protection::MasterKey, CaptureError> {
    MasterKeyManager::new(
        app_data_directory.join("security").join("master-key.json"),
        DpapiCurrentUserProtector,
    )
    .load_or_create()
    .map_err(|_| CaptureError::Storage)
}

fn validate_published_time(value: &str) -> Result<(), CaptureError> {
    if value.trim().is_empty() || !value.contains('T') {
        return Err(CaptureError::InvalidPublishedTime);
    }
    Ok(())
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn summary_for(
    repository: &NoticeRepository<'_>,
    notice_id: &str,
) -> Result<NoticeSummaryV1, CaptureError> {
    repository
        .notice_detail(notice_id)
        .map(|detail| summary_to_contract(detail.summary))
        .map_err(map_repository_error)
}

fn map_repository_error(error: crate::storage::repository::RepositoryError) -> CaptureError {
    match error {
        crate::storage::repository::RepositoryError::MissingNotice => CaptureError::MissingNotice,
        _ => CaptureError::Storage,
    }
}

fn from_contract_state(state: NoticeStateV1) -> NoticeState {
    match state {
        NoticeStateV1::PendingAnalysis => NoticeState::PendingAnalysis,
        NoticeStateV1::PendingReview => NoticeState::PendingReview,
        NoticeStateV1::PartiallyProcessed => NoticeState::PartiallyProcessed,
        NoticeStateV1::Processed => NoticeState::Processed,
        NoticeStateV1::InformationOnly => NoticeState::InformationOnly,
        NoticeStateV1::Failed => NoticeState::Failed,
    }
}

fn to_contract_state(state: NoticeState) -> NoticeStateV1 {
    match state {
        NoticeState::PendingAnalysis => NoticeStateV1::PendingAnalysis,
        NoticeState::PendingReview => NoticeStateV1::PendingReview,
        NoticeState::PartiallyProcessed => NoticeStateV1::PartiallyProcessed,
        NoticeState::Processed => NoticeStateV1::Processed,
        NoticeState::InformationOnly => NoticeStateV1::InformationOnly,
        NoticeState::Failed => NoticeStateV1::Failed,
    }
}

fn summary_to_contract(summary: NoticeSummary) -> NoticeSummaryV1 {
    NoticeSummaryV1 {
        id: summary.id,
        source_kind: summary.source_kind,
        inbox_state: to_contract_state(summary.inbox_state),
        published_at: summary.published_at,
        published_time_source: summary.published_time_source,
        published_time_candidate: summary.published_time_candidate,
        published_time_candidate_source: summary.published_time_candidate_source,
        created_at: summary.created_at,
        excerpt: summary.excerpt,
    }
}

fn extract_published_time_candidate(bytes: &[u8]) -> Option<PublishedTimeCandidate> {
    let mut index = 0;
    while index < bytes.len() {
        if let Some(candidate) = parse_timestamp_at(bytes, index) {
            return Some(PublishedTimeCandidate {
                published_at: candidate,
                source: if bytes[index..].starts_with(b"20") && bytes.get(index + 4) == Some(&b':')
                {
                    "embeddedMetadata".to_owned()
                } else {
                    "embeddedText".to_owned()
                },
            });
        }
        index += 1;
    }
    None
}

fn parse_timestamp_at(bytes: &[u8], index: usize) -> Option<String> {
    let digits = |offset: usize, count: usize| -> Option<u32> {
        let end = index.checked_add(offset + count)?;
        let slice = bytes.get(index + offset..end)?;
        if !slice.iter().all(u8::is_ascii_digit) {
            return None;
        }
        slice.iter().try_fold(0_u32, |value, digit| {
            value.checked_mul(10)?.checked_add(u32::from(digit - b'0'))
        })
    };
    let year = digits(0, 4)?;
    if !(2000..=2100).contains(&year) {
        return None;
    }
    let separator = |offset: usize, expected: u8| bytes.get(index + offset) == Some(&expected);
    let month_day_separator = if separator(4, b':') || separator(4, b'-') || separator(4, b'/') {
        bytes[index + 4]
    } else {
        return None;
    };
    let month = digits(5, 2)?;
    if !separator(7, month_day_separator) {
        return None;
    }
    let day = digits(8, 2)?;
    let time_offset = if separator(10, b' ') || separator(10, b'T') {
        11
    } else {
        return None;
    };
    if !((1..=12).contains(&month) && (1..=31).contains(&day)) {
        return None;
    }
    let hour = digits(time_offset, 2)?;
    if bytes.get(index + time_offset + 2) != Some(&b':') {
        return None;
    }
    let minute = digits(time_offset + 3, 2)?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:00Z"
    ))
}

fn detail_to_contract(detail: NoticeDetail) -> NoticeDetailV1 {
    let source_asset = detail.source_asset.map(|asset| SourceAssetInfoV1 {
        id: asset.id,
        media_type: asset.media_type,
        byte_size: asset.byte_size,
        pixel_width: asset.pixel_width,
        pixel_height: asset.pixel_height,
    });
    NoticeDetailV1 {
        summary: summary_to_contract(detail.summary),
        original_text: detail.original_text,
        source_asset,
    }
}

fn image_dimensions(bytes: &[u8]) -> Result<(&'static str, u32, u32), CaptureError> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        && bytes.len() >= 45
        && &bytes[12..16] == b"IHDR"
        && bytes.ends_with(b"IEND\xaeB`\x82")
    {
        return Ok((
            "image/png",
            u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
            u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
        ));
    }
    if bytes.starts_with(&[0xff, 0xd8]) && bytes.ends_with(&[0xff, 0xd9]) {
        return jpeg_dimensions(bytes).map(|(width, height)| ("image/jpeg", width, height));
    }
    if bytes.len() >= 30 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return webp_dimensions(bytes).map(|(width, height)| ("image/webp", width, height));
    }
    Err(CaptureError::UnsupportedImage)
}

fn jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32), CaptureError> {
    let mut offset = 2;
    while offset + 9 <= bytes.len() {
        if bytes[offset] != 0xff {
            return Err(CaptureError::InvalidImage);
        }
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        if offset >= bytes.len() {
            break;
        }
        let marker = bytes[offset];
        offset += 1;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        if offset + 2 > bytes.len() {
            return Err(CaptureError::InvalidImage);
        }
        let length = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        if length < 2 || offset + length > bytes.len() {
            return Err(CaptureError::InvalidImage);
        }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if length < 8 {
                return Err(CaptureError::InvalidImage);
            }
            return Ok((
                u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]) as u32,
                u16::from_be_bytes([bytes[offset + 3], bytes[offset + 4]]) as u32,
            ));
        }
        offset += length;
    }
    Err(CaptureError::InvalidImage)
}

fn webp_dimensions(bytes: &[u8]) -> Result<(u32, u32), CaptureError> {
    match &bytes[12..16] {
        b"VP8X" if bytes.len() >= 30 => Ok((
            1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]),
            1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]),
        )),
        b"VP8 " if bytes.len() >= 30 && bytes[23..26] == [0x9d, 0x01, 0x2a] => Ok((
            u16::from_le_bytes([bytes[26], bytes[27]]) as u32 & 0x3fff,
            u16::from_le_bytes([bytes[28], bytes[29]]) as u32 & 0x3fff,
        )),
        b"VP8L" if bytes.len() >= 25 && bytes[20] == 0x2f => {
            let bits = u32::from_le_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]);
            Ok(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        _ => Err(CaptureError::InvalidImage),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        image_preview, import_image, import_text, list_notices, notice_detail, validate_image,
        CaptureError,
    };
    use crate::contracts::NoticeStateV1;
    use tempfile::tempdir;

    fn png() -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        png.extend([0, 0, 0, 2, 0, 0, 0, 3]);
        png.extend([0; 13]);
        png.extend(b"IEND\xaeB`\x82");
        png
    }

    fn png_with_embedded_time() -> Vec<u8> {
        let mut image = png();
        let insertion = image.len() - 12;
        image.splice(insertion..insertion, b"2026-08-28 17:30".iter().copied());
        image
    }

    #[test]
    fn validates_a_png_without_reading_from_disk() {
        let result = validate_image(&png(), Some("image/png")).unwrap();
        assert_eq!((result.pixel_width, result.pixel_height), (2, 3));
    }

    #[test]
    fn rejects_unsupported_or_mismatched_images() {
        assert!(matches!(
            validate_image(b"not an image", None),
            Err(CaptureError::UnsupportedImage)
        ));
        assert!(matches!(
            validate_image(&png(), Some("image/jpeg")),
            Err(CaptureError::UnsupportedImage)
        ));
    }

    #[test]
    fn rejects_blank_text_and_preserves_duplicate_text_submissions() {
        let directory = tempdir().unwrap();
        assert!(matches!(
            import_text(
                directory.path(),
                " \n\t ".to_owned(),
                "2026-08-27T09:00:00Z".to_owned(),
            ),
            Err(CaptureError::EmptyText)
        ));

        let first = import_text(
            directory.path(),
            "同一条通知".to_owned(),
            "2026-08-27T09:00:00Z".to_owned(),
        )
        .unwrap();
        let second = import_text(
            directory.path(),
            "同一条通知".to_owned(),
            "2026-08-27T09:00:00Z".to_owned(),
        )
        .unwrap();
        let notices = list_notices(directory.path(), Some(NoticeStateV1::PendingAnalysis)).unwrap();

        assert_ne!(first.id, second.id);
        assert_eq!(notices.len(), 2);
        assert_eq!(
            notice_detail(directory.path(), &first.id)
                .unwrap()
                .original_text
                .as_deref(),
            Some("同一条通知")
        );
    }

    #[test]
    fn stores_images_encrypted_and_only_decrypts_for_the_requested_preview() {
        let directory = tempdir().unwrap();
        let original = png();
        let notice = import_image(
            directory.path(),
            original.clone(),
            Some("image/png".to_owned()),
            "2026-08-27T09:00:00Z".to_owned(),
        )
        .unwrap();
        let detail = notice_detail(directory.path(), &notice.id).unwrap();
        let asset = detail.source_asset.unwrap();
        let encrypted = std::fs::read(
            directory
                .path()
                .join("attachments")
                .join(format!("{}.enc", asset.id)),
        )
        .unwrap();

        assert_ne!(encrypted, original);
        assert_eq!(
            image_preview(directory.path(), &notice.id).unwrap().bytes,
            original
        );
    }

    #[test]
    fn extracts_embedded_image_time_as_a_candidate_without_running_ocr() {
        let directory = tempdir().unwrap();
        let notice = import_image(
            directory.path(),
            png_with_embedded_time(),
            Some("image/png".to_owned()),
            "2026-08-27T09:00:00Z".to_owned(),
        )
        .unwrap();
        let detail = notice_detail(directory.path(), &notice.id).unwrap();

        assert_eq!(
            detail.summary.published_time_candidate.as_deref(),
            Some("2026-08-28T17:30:00Z")
        );
        assert_eq!(
            detail.summary.published_time_candidate_source.as_deref(),
            Some("embeddedText")
        );
    }

    #[test]
    fn rejects_invalid_images_before_creating_an_attachment() {
        let directory = tempdir().unwrap();
        assert!(matches!(
            import_image(
                directory.path(),
                b"not an image".to_vec(),
                Some("image/png".to_owned()),
                "2026-08-27T09:00:00Z".to_owned(),
            ),
            Err(CaptureError::UnsupportedImage)
        ));
        assert!(!directory.path().join("attachments").exists());
    }
}

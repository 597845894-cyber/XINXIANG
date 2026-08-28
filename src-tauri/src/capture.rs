use std::path::Path;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    backup::{self, BackupSummary},
    contracts::{
        AnalysisRevisionViewV1, BackupSummaryV1, CandidateViewV1, NoticeDetailV1,
        NoticeRelationViewV1, NoticeStateV1, NoticeSummaryV1, ReminderViewV1, SourceAssetInfoV1,
        TaskRevisionViewV1, TaskViewV1,
    },
    security::key_protection::{DpapiCurrentUserProtector, MasterKeyManager},
    storage::{
        attachments::AttachmentStore,
        database::EncryptedDatabase,
        repository::{
            CandidateState, NewCandidate, NoticeDetail, NoticeRelationRecord, NoticeRepository,
            NoticeState, NoticeSummary, ReminderRecord, TaskState,
        },
    },
};

#[derive(Debug)]
pub enum CaptureError {
    EmptyText,
    InvalidPublishedTime,
    ImageImportNotSupported,
    Storage,
    MissingNotice,
    InvalidPayload,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::EmptyText => "NOTICE_TEXT_REQUIRED",
            Self::InvalidPublishedTime => "NOTICE_PUBLISHED_TIME_INVALID",
            Self::ImageImportNotSupported => "NOTICE_IMAGE_IMPORT_NOT_SUPPORTED",
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

pub fn import_image(
    _app_data_directory: &Path,
    _bytes: Vec<u8>,
    _declared_media_type: Option<String>,
    _published_at: String,
) -> Result<NoticeSummaryV1, CaptureError> {
    Err(CaptureError::ImageImportNotSupported)
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

pub fn list_analysis_revisions(
    app_data_directory: &Path,
    notice_id: &str,
) -> Result<Vec<AnalysisRevisionViewV1>, CaptureError> {
    let database = open_database(app_data_directory)?;
    let repository = NoticeRepository::new(database.connection());
    let revisions = repository
        .list_analysis_revisions(notice_id)
        .map_err(map_repository_error)?;
    let candidates = repository
        .list_candidates(None)
        .map_err(map_repository_error)?;
    Ok(revisions
        .into_iter()
        .map(|revision| AnalysisRevisionViewV1 {
            candidates: candidates
                .iter()
                .filter(|candidate| candidate.analysis_revision_id == revision.id)
                .filter_map(|candidate| serde_json::from_slice(&candidate.payload).ok())
                .collect(),
            id: revision.id,
            revision_number: revision.revision_number,
            classifier_version: revision.classifier_version,
            ocr_text: revision.ocr_text,
            created_at: revision.created_at,
        })
        .collect())
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
    Err(CaptureError::ImageImportNotSupported)
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

pub fn create_backup(
    app_data_directory: &Path,
    target_path: &Path,
    password: &str,
) -> Result<BackupSummaryV1, CaptureError> {
    let database = open_database(app_data_directory)?;
    database
        .connection()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|_| CaptureError::Storage)?;
    let summary = backup_summary(database.connection())?;
    drop(database);
    backup::create(app_data_directory, target_path, password, summary)
        .map(backup_to_contract)
        .map_err(|_| CaptureError::Storage)
}

pub fn inspect_backup(path: &Path, password: &str) -> Result<BackupSummaryV1, CaptureError> {
    backup::inspect(path, password)
        .map(backup_to_contract)
        .map_err(|_| CaptureError::Storage)
}

pub fn restore_backup(
    app_data_directory: &Path,
    path: &Path,
    password: &str,
) -> Result<BackupSummaryV1, CaptureError> {
    backup::restore(app_data_directory, path, password)
        .map(backup_to_contract)
        .map_err(|_| CaptureError::Storage)
}

pub fn delete_notice_cascade(
    app_data_directory: &Path,
    notice_id: &str,
) -> Result<(), CaptureError> {
    let database = open_database(app_data_directory)?;
    let asset_ids = NoticeRepository::new(database.connection())
        .delete_notice_cascade(notice_id)
        .map_err(map_repository_error)?;
    let store = AttachmentStore::new(app_data_directory.join("attachments"));
    for asset_id in asset_ids {
        store.remove(&asset_id).map_err(|_| CaptureError::Storage)?;
    }
    Ok(())
}

pub fn delete_notice_keep_tasks(
    app_data_directory: &Path,
    notice_id: &str,
) -> Result<(), CaptureError> {
    let database = open_database(app_data_directory)?;
    let asset_ids = NoticeRepository::new(database.connection())
        .detach_notice_keep_tasks(notice_id)
        .map_err(map_repository_error)?;
    let store = AttachmentStore::new(app_data_directory.join("attachments"));
    for asset_id in asset_ids {
        store.remove(&asset_id).map_err(|_| CaptureError::Storage)?;
    }
    Ok(())
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
) -> Result<Vec<TaskRevisionViewV1>, CaptureError> {
    let database = open_database(app_data_directory)?;
    NoticeRepository::new(database.connection())
        .task_history(task_id)
        .map_err(map_repository_error)?
        .into_iter()
        .map(|revision| {
            Ok(TaskRevisionViewV1 {
                id: revision.id,
                revision_number: revision.revision_number,
                source_candidate_id: revision.source_candidate_id,
                analysis_revision_id: revision.analysis_revision_id,
                payload: serde_json::from_slice(&revision.payload)
                    .map_err(|_| CaptureError::Storage)?,
                created_at: revision.created_at,
            })
        })
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
    let candidate_contexts = candidates
        .iter()
        .filter_map(|candidate| {
            let payload = serde_json::from_slice::<serde_json::Value>(&candidate.payload).ok()?;
            let title = payload.get("title")?.as_str()?.to_owned();
            let source_text = repository
                .notice_detail(&candidate.notice_id)
                .ok()
                .and_then(|detail| detail.original_text)
                .unwrap_or_default();
            Some((
                candidate.id.as_str(),
                candidate.notice_id.as_str(),
                title,
                source_text,
                payload,
            ))
        })
        .collect::<Vec<_>>();
    for (candidate_id, _, title, source_text, payload) in candidate_contexts
        .iter()
        .filter(|(_, id, _, _, _)| *id == notice_id)
    {
        let normalized = normalize_key(title);
        for (
            comparison_candidate_id,
            comparison_notice_id,
            comparison_title,
            comparison_source_text,
            comparison_payload,
        ) in candidate_contexts
            .iter()
            .filter(|(_, id, _, _, _)| *id != notice_id)
        {
            let comparison_normalized = normalize_key(comparison_title);
            let match_evidence = relation_match_evidence(
                title,
                source_text,
                payload,
                comparison_title,
                comparison_source_text,
                comparison_payload,
            );
            if let Some(match_evidence) = match_evidence {
                let relation_type =
                    relation_type(title, source_text, normalized == comparison_normalized);
                let evidence = serde_json::to_vec(&serde_json::json!({
                    "matchedTitle": title,
                    "otherTitle": comparison_title,
                    "reason": match_evidence.reason,
                    "normalizedHash": normalized_hash(title),
                    "relatedNormalizedHash": normalized_hash(comparison_title),
                    "normalizedContentHash": normalized_hash(source_text),
                    "relatedNormalizedContentHash": normalized_hash(comparison_source_text),
                    "textSimilarity": match_evidence.text_similarity,
                    "fieldMatches": match_evidence.field_matches,
                    "proposedCandidateId": candidate_id,
                    "existingCandidateId": comparison_candidate_id,
                    "proposedPayload": payload,
                    "existingPayload": comparison_payload,
                }))
                .map_err(|_| CaptureError::Storage)?;
                let relation = NoticeRelationRecord {
                    id: new_id(),
                    notice_id: notice_id.to_owned(),
                    related_notice_id: (*comparison_notice_id).to_owned(),
                    relation_type: relation_type.to_owned(),
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
    let repository = NoticeRepository::new(database.connection());
    if !accepted {
        return repository
            .set_relation_state(relation_id, "rejected")
            .map_err(map_repository_error);
    }
    let relation = repository
        .relation_by_id(relation_id)
        .map_err(map_repository_error)?;
    let proposed_payload = serde_json::from_slice::<serde_json::Value>(&relation.evidence)
        .ok()
        .and_then(|evidence| evidence.get("proposedPayload").cloned())
        .map(validated_payload)
        .transpose()?;
    let existing_candidate_id = serde_json::from_slice::<serde_json::Value>(&relation.evidence)
        .ok()
        .and_then(|evidence| {
            evidence
                .get("existingCandidateId")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
    repository
        .accept_relation(
            relation_id,
            proposed_payload.as_deref(),
            existing_candidate_id.as_deref(),
        )
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
        source_removed_at: task.source_removed_at,
    })
}

fn backup_summary(connection: &rusqlite::Connection) -> Result<BackupSummary, CaptureError> {
    let count = |table: &str| -> Result<u64, CaptureError> {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(|_| CaptureError::Storage)
    };
    let created_at = connection
        .query_row("SELECT datetime('now')", [], |row| row.get(0))
        .map_err(|_| CaptureError::Storage)?;
    Ok(BackupSummary {
        format_version: 1,
        created_at,
        notice_count: count("notices")?,
        task_count: count("tasks")?,
        attachment_count: count("source_assets")?,
        byte_size: 0,
    })
}

fn backup_to_contract(summary: BackupSummary) -> BackupSummaryV1 {
    BackupSummaryV1 {
        format_version: summary.format_version,
        created_at: summary.created_at,
        notice_count: summary.notice_count,
        task_count: summary.task_count,
        attachment_count: summary.attachment_count,
        byte_size: summary.byte_size,
    }
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

fn relation_type(title: &str, source_text: &str, exact_title_match: bool) -> &'static str {
    if contains_any(title, source_text, &["取消", "停止", "作废"]) {
        "cancel"
    } else if contains_any(title, source_text, &["改期", "延期", "调整", "变更"]) {
        "reschedule"
    } else if exact_title_match {
        "duplicate"
    } else {
        "supplement"
    }
}

struct RelationMatchEvidence {
    reason: &'static str,
    text_similarity: f32,
    field_matches: Vec<&'static str>,
}

fn relation_match_evidence(
    title: &str,
    source_text: &str,
    payload: &serde_json::Value,
    comparison_title: &str,
    comparison_source_text: &str,
    comparison_payload: &serde_json::Value,
) -> Option<RelationMatchEvidence> {
    let left_title = normalize_key(title);
    let right_title = normalize_key(comparison_title);
    if left_title == right_title {
        return Some(RelationMatchEvidence {
            reason: "normalizedHash",
            text_similarity: 1.0,
            field_matches: matching_structured_fields(payload, comparison_payload),
        });
    }

    let text_similarity = character_similarity(
        &format!("{title} {source_text}"),
        &format!("{comparison_title} {comparison_source_text}"),
    );
    let field_matches = matching_structured_fields(payload, comparison_payload);
    let related_titles = left_title.len() >= 4
        && right_title.len() >= 4
        && (left_title.contains(&right_title) || right_title.contains(&left_title));
    if related_titles
        || text_similarity >= 0.42
        || (text_similarity >= 0.24 && !field_matches.is_empty())
    {
        Some(RelationMatchEvidence {
            reason: if !field_matches.is_empty() {
                "structuredFieldMatch"
            } else {
                "textSimilarity"
            },
            text_similarity,
            field_matches,
        })
    } else {
        None
    }
}

fn matching_structured_fields(
    payload: &serde_json::Value,
    comparison_payload: &serde_json::Value,
) -> Vec<&'static str> {
    ["location", "submissionUrl", "audience", "required"]
        .into_iter()
        .filter(|field| {
            let left = payload.get(*field);
            let right = comparison_payload.get(*field);
            left.is_some() && left == right && !left.is_some_and(serde_json::Value::is_null)
        })
        .collect()
}

fn character_similarity(left: &str, right: &str) -> f32 {
    let left = normalize_key(left);
    let right = normalize_key(right);
    let left_characters = left.chars().collect::<std::collections::BTreeSet<_>>();
    let right_characters = right.chars().collect::<std::collections::BTreeSet<_>>();
    let union = left_characters.union(&right_characters).count();
    if union == 0 {
        return 0.0;
    }
    left_characters.intersection(&right_characters).count() as f32 / union as f32
}

fn contains_any(title: &str, source_text: &str, keywords: &[&str]) -> bool {
    keywords
        .iter()
        .any(|keyword| title.contains(keyword) || source_text.contains(keyword))
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

fn normalized_hash(value: &str) -> String {
    let digest = Sha256::digest(normalize_key(value).as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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

#[cfg(test)]
mod tests {
    use super::{
        import_image, import_text, list_notices, notice_detail, relation_match_evidence,
        relation_type, CaptureError,
    };
    use crate::contracts::NoticeStateV1;
    use tempfile::tempdir;

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
    fn relation_matching_uses_hash_text_and_structured_fields_without_matching_unrelated_notices() {
        let exact = relation_match_evidence(
            "综合测评报名",
            "请在本周内完成综合测评报名。",
            &serde_json::json!({ "location": "学生中心", "required": true }),
            "综合测评报名",
            "请在本周内完成综合测评报名。",
            &serde_json::json!({ "location": "学生中心", "required": true }),
        )
        .unwrap();
        assert_eq!(exact.reason, "normalizedHash");
        assert!(exact.field_matches.contains(&"location"));

        let changed = relation_match_evidence(
            "关于综合测评报名",
            "综合测评报名截止时间调整至下周五。",
            &serde_json::json!({ "audience": "2026级" }),
            "综合测评报名",
            "请同学们完成综合测评报名。",
            &serde_json::json!({ "audience": "2026级" }),
        )
        .unwrap();
        assert_eq!(changed.reason, "structuredFieldMatch");
        assert_eq!(
            relation_type(
                "关于综合测评报名",
                "综合测评报名截止时间调整至下周五。",
                false
            ),
            "reschedule"
        );

        assert!(relation_match_evidence(
            "宿舍晚归检查",
            "今晚开展宿舍晚归检查。",
            &serde_json::json!({}),
            "食堂满意度问卷",
            "请填写食堂满意度问卷。",
            &serde_json::json!({}),
        )
        .is_none());
    }

    #[test]
    fn rejects_image_import_without_creating_notices_or_attachments() {
        let directory = tempdir().unwrap();
        assert!(matches!(
            import_image(
                directory.path(),
                b"image bytes must never be inspected".to_vec(),
                Some("image/png".to_owned()),
                "2026-08-27T09:00:00Z".to_owned(),
            ),
            Err(CaptureError::ImageImportNotSupported)
        ));
        assert!(!directory.path().join("attachments").exists());
        assert!(!directory.path().join("inbox.db").exists());
    }
}

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::storage::attachments::EncryptedAttachmentMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeState {
    PendingAnalysis,
    PendingReview,
    PartiallyProcessed,
    Processed,
    InformationOnly,
    Failed,
}

impl NoticeState {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::PendingAnalysis => "pendingAnalysis",
            Self::PendingReview => "pendingReview",
            Self::PartiallyProcessed => "partiallyProcessed",
            Self::Processed => "processed",
            Self::InformationOnly => "informationOnly",
            Self::Failed => "failed",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "pendingAnalysis" => Some(Self::PendingAnalysis),
            "pendingReview" => Some(Self::PendingReview),
            "partiallyProcessed" => Some(Self::PartiallyProcessed),
            "processed" => Some(Self::Processed),
            "informationOnly" => Some(Self::InformationOnly),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateState {
    Pending,
    Confirmed,
    Ignored,
    Merged,
}

impl CandidateState {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Ignored => "ignored",
            Self::Merged => "merged",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "confirmed" => Some(Self::Confirmed),
            "ignored" => Some(Self::Ignored),
            "merged" => Some(Self::Merged),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Todo,
    Completed,
    Cancelled,
}

impl TaskState {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "todo" => Some(Self::Todo),
            "completed" => Some(Self::Completed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum RepositoryError {
    Database,
    InvalidState,
    MissingCandidate,
    MissingNotice,
    MissingTask,
    MissingRelation,
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::Database => "REPOSITORY_DATABASE_FAILED",
            Self::InvalidState => "REPOSITORY_INVALID_STATE",
            Self::MissingCandidate => "REPOSITORY_CANDIDATE_MISSING",
            Self::MissingNotice => "REPOSITORY_NOTICE_MISSING",
            Self::MissingTask => "REPOSITORY_TASK_MISSING",
            Self::MissingRelation => "REPOSITORY_RELATION_MISSING",
        };
        formatter.write_str(code)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeSummary {
    pub id: String,
    pub source_kind: String,
    pub inbox_state: NoticeState,
    pub published_at: String,
    pub published_time_source: String,
    pub published_time_candidate: Option<String>,
    pub published_time_candidate_source: Option<String>,
    pub created_at: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeDetail {
    pub summary: NoticeSummary,
    pub original_text: Option<String>,
    pub source_asset: Option<SourceAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedTimeCandidate {
    pub published_at: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAsset {
    pub id: String,
    pub media_type: String,
    pub encrypted_file_name: String,
    pub metadata: EncryptedAttachmentMetadata,
    pub byte_size: usize,
    pub pixel_width: Option<u32>,
    pub pixel_height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateRecord {
    pub id: String,
    pub notice_id: String,
    pub analysis_revision_id: String,
    pub state: CandidateState,
    pub payload: Vec<u8>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub id: String,
    pub notice_id: Option<String>,
    pub state: TaskState,
    pub current_revision_id: Option<String>,
    pub payload: Vec<u8>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeRelationRecord {
    pub id: String,
    pub notice_id: String,
    pub related_notice_id: String,
    pub relation_type: String,
    pub relation_state: String,
    pub evidence: Vec<u8>,
    pub created_at: String,
}

impl std::error::Error for RepositoryError {}

pub struct NoticeRepository<'connection> {
    connection: &'connection Connection,
}

impl<'connection> NoticeRepository<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn create_notice(&self, notice_id: &str, source_kind: &str) -> Result<(), RepositoryError> {
        self.connection
            .execute(
                "INSERT INTO notices (id, source_kind, inbox_state, created_at, updated_at)
                 VALUES (?1, ?2, ?3, datetime('now'), datetime('now'))",
                params![notice_id, source_kind, NoticeState::PendingAnalysis.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        Ok(())
    }

    pub fn save_analysis_revision(
        &self,
        notice_id: &str,
        analysis_revision_id: &str,
        classifier_version: &str,
        candidates: &[NewCandidate<'_>],
    ) -> Result<(), RepositoryError> {
        self.save_analysis_revision_full(
            notice_id,
            analysis_revision_id,
            classifier_version,
            None,
            candidates,
        )
    }

    pub fn save_analysis_revision_full(
        &self,
        notice_id: &str,
        analysis_revision_id: &str,
        classifier_version: &str,
        ocr_text: Option<&str>,
        candidates: &[NewCandidate<'_>],
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| RepositoryError::Database)?;
        let revision_number: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(revision_number), 0) + 1 FROM analysis_revisions WHERE notice_id = ?1",
                [notice_id],
                |row| row.get(0),
            )
            .map_err(|_| RepositoryError::Database)?;
        let notice_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM notices WHERE id = ?1)",
                [notice_id],
                |row| row.get(0),
            )
            .map_err(|_| RepositoryError::Database)?;
        if !notice_exists {
            return Err(RepositoryError::InvalidState);
        }
        transaction
            .execute(
                "INSERT INTO analysis_revisions (
                    id, notice_id, revision_number, analysis_state, ocr_text, classifier_version, created_at
                 ) VALUES (?1, ?2, ?3, 'complete', ?4, ?5, datetime('now'))",
                params![
                    analysis_revision_id,
                    notice_id,
                    revision_number,
                    ocr_text.map(str::as_bytes),
                    classifier_version
                ],
            )
            .map_err(|_| RepositoryError::Database)?;
        for candidate in candidates {
            transaction
                .execute(
                    "INSERT INTO task_candidates (
                        id, analysis_revision_id, candidate_state, structured_payload, created_at
                     ) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                    params![
                        candidate.id,
                        analysis_revision_id,
                        CandidateState::Pending.as_db(),
                        candidate.payload
                    ],
                )
                .map_err(|_| RepositoryError::Database)?;
        }
        let next_state = if candidates.is_empty() {
            NoticeState::InformationOnly
        } else {
            NoticeState::PendingReview
        };
        transaction
            .execute(
                "UPDATE notices SET inbox_state = ?2, updated_at = datetime('now') WHERE id = ?1",
                params![notice_id, next_state.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }

    pub fn mark_analysis_failed(&self, notice_id: &str) -> Result<(), RepositoryError> {
        self.connection
            .execute(
                "UPDATE notices SET inbox_state = ?2, updated_at = datetime('now') WHERE id = ?1",
                params![notice_id, NoticeState::Failed.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        Ok(())
    }

    pub fn confirm_candidate(
        &self,
        candidate_id: &str,
        task_id: &str,
        task_revision_id: &str,
        payload: &[u8],
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| RepositoryError::Database)?;
        let candidate = candidate_for_confirmation(&transaction, candidate_id)?;
        transaction
            .execute(
                "INSERT INTO tasks (id, notice_id, task_state, current_revision_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'))",
                params![
                    task_id,
                    candidate.notice_id,
                    TaskState::Todo.as_db(),
                    task_revision_id
                ],
            )
            .map_err(|_| RepositoryError::Database)?;
        transaction
            .execute(
                "INSERT INTO task_revisions (
                    id, task_id, revision_number, source_candidate_id, payload, created_at
                 ) VALUES (?1, ?2, 1, ?3, ?4, datetime('now'))",
                params![task_revision_id, task_id, candidate_id, payload],
            )
            .map_err(|_| RepositoryError::Database)?;
        transaction
            .execute(
                "UPDATE task_candidates SET candidate_state = ?2 WHERE id = ?1",
                params![candidate_id, CandidateState::Confirmed.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        let pending_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM task_candidates candidate
                 INNER JOIN analysis_revisions revision ON revision.id = candidate.analysis_revision_id
                 WHERE revision.notice_id = ?1 AND candidate.candidate_state = 'pending'",
                [&candidate.notice_id],
                |row| row.get(0),
            )
            .map_err(|_| RepositoryError::Database)?;
        let next_state = if pending_count == 0 {
            NoticeState::Processed
        } else {
            NoticeState::PartiallyProcessed
        };
        transaction
            .execute(
                "UPDATE notices SET inbox_state = ?2, updated_at = datetime('now') WHERE id = ?1",
                params![candidate.notice_id, next_state.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        insert_audit(
            &transaction,
            "candidate",
            candidate_id,
            "confirmed",
            Some(payload),
        )?;
        insert_audit(&transaction, "task", task_id, "created", Some(payload))?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }

    pub fn list_candidates(
        &self,
        state: Option<CandidateState>,
    ) -> Result<Vec<CandidateRecord>, RepositoryError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT candidate.id, revision.notice_id, candidate.analysis_revision_id,
                        candidate.candidate_state, candidate.structured_payload, candidate.created_at
                 FROM task_candidates candidate
                 INNER JOIN analysis_revisions revision ON revision.id = candidate.analysis_revision_id
                 WHERE (?1 IS NULL OR candidate.candidate_state = ?1)
                 ORDER BY candidate.created_at DESC",
            )
            .map_err(|_| RepositoryError::Database)?;
        let rows = statement
            .query_map([state.map(CandidateState::as_db)], |row| {
                let state_value: String = row.get(3)?;
                Ok(CandidateRecord {
                    id: row.get(0)?,
                    notice_id: row.get(1)?,
                    analysis_revision_id: row.get(2)?,
                    state: CandidateState::from_db(&state_value)
                        .ok_or(rusqlite::Error::InvalidQuery)?,
                    payload: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|_| RepositoryError::Database)?;
        rows.map(|row| row.map_err(|_| RepositoryError::Database))
            .collect()
    }

    pub fn edit_candidate(
        &self,
        candidate_id: &str,
        payload: &[u8],
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| RepositoryError::Database)?;
        let changed = transaction
            .execute(
                "UPDATE task_candidates SET structured_payload = ?2 WHERE id = ?1 AND candidate_state = 'pending'",
                params![candidate_id, payload],
            )
            .map_err(|_| RepositoryError::Database)?;
        if changed == 0 {
            return Err(RepositoryError::InvalidState);
        }
        insert_audit(
            &transaction,
            "candidate",
            candidate_id,
            "edited",
            Some(payload),
        )?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }

    pub fn set_candidate_state(
        &self,
        candidate_id: &str,
        state: CandidateState,
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| RepositoryError::Database)?;
        let notice_id: String = transaction
            .query_row(
                "SELECT revision.notice_id FROM task_candidates candidate
                 INNER JOIN analysis_revisions revision ON revision.id = candidate.analysis_revision_id
                 WHERE candidate.id = ?1",
                [candidate_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| RepositoryError::Database)?
            .ok_or(RepositoryError::MissingCandidate)?;
        let changed = transaction
            .execute(
                "UPDATE task_candidates SET candidate_state = ?2 WHERE id = ?1 AND candidate_state = 'pending'",
                params![candidate_id, state.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        if changed == 0 {
            return Err(RepositoryError::InvalidState);
        }
        let pending: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM task_candidates candidate
                 INNER JOIN analysis_revisions revision ON revision.id = candidate.analysis_revision_id
                 WHERE revision.notice_id = ?1 AND candidate.candidate_state = 'pending'",
                [&notice_id],
                |row| row.get(0),
            )
            .map_err(|_| RepositoryError::Database)?;
        let next_notice_state = if pending == 0 {
            NoticeState::Processed
        } else if state == CandidateState::Ignored || state == CandidateState::Merged {
            NoticeState::PartiallyProcessed
        } else {
            NoticeState::PendingReview
        };
        transaction
            .execute(
                "UPDATE notices SET inbox_state = ?2, updated_at = datetime('now') WHERE id = ?1",
                params![notice_id, next_notice_state.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        insert_audit(&transaction, "candidate", candidate_id, state.as_db(), None)?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }

    pub fn merge_candidates(
        &self,
        target_id: &str,
        source_ids: &[&str],
        payload: &[u8],
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| RepositoryError::Database)?;
        let target_state: String = transaction
            .query_row(
                "SELECT candidate_state FROM task_candidates WHERE id = ?1",
                [target_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| RepositoryError::Database)?
            .ok_or(RepositoryError::MissingCandidate)?;
        if target_state != CandidateState::Pending.as_db()
            || source_ids.iter().any(|id| *id == target_id)
        {
            return Err(RepositoryError::InvalidState);
        }
        transaction.execute("UPDATE task_candidates SET structured_payload = ?2 WHERE id = ?1 AND candidate_state = 'pending'", params![target_id, payload]).map_err(|_| RepositoryError::Database)?;
        for source_id in source_ids {
            let changed = transaction.execute("UPDATE task_candidates SET candidate_state = 'merged' WHERE id = ?1 AND candidate_state = 'pending'", [source_id]).map_err(|_| RepositoryError::Database)?;
            if changed == 0 {
                return Err(RepositoryError::InvalidState);
            }
            insert_audit(
                &transaction,
                "candidate",
                source_id,
                "merged",
                Some(target_id.as_bytes()),
            )?;
        }
        insert_audit(
            &transaction,
            "candidate",
            target_id,
            "mergeTargetEdited",
            Some(payload),
        )?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }

    pub fn split_candidate(
        &self,
        candidate_id: &str,
        splits: &[NewCandidate<'_>],
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| RepositoryError::Database)?;
        let revision_id: String = transaction
            .query_row("SELECT analysis_revision_id FROM task_candidates WHERE id = ?1 AND candidate_state = 'pending'", [candidate_id], |row| row.get(0))
            .optional().map_err(|_| RepositoryError::Database)?.ok_or(RepositoryError::InvalidState)?;
        if splits.len() < 2 {
            return Err(RepositoryError::InvalidState);
        }
        for split in splits {
            transaction.execute("INSERT INTO task_candidates (id, analysis_revision_id, candidate_state, structured_payload, created_at) VALUES (?1, ?2, 'pending', ?3, datetime('now'))", params![split.id, revision_id, split.payload]).map_err(|_| RepositoryError::Database)?;
        }
        transaction
            .execute(
                "UPDATE task_candidates SET candidate_state = 'merged' WHERE id = ?1",
                [candidate_id],
            )
            .map_err(|_| RepositoryError::Database)?;
        insert_audit(&transaction, "candidate", candidate_id, "split", None)?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }

    pub fn list_tasks(&self) -> Result<Vec<TaskRecord>, RepositoryError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT task.id, task.notice_id, task.task_state, task.current_revision_id,
                    revision.payload, task.created_at, task.updated_at
             FROM tasks task
             LEFT JOIN task_revisions revision ON revision.id = task.current_revision_id
             ORDER BY task.updated_at DESC",
            )
            .map_err(|_| RepositoryError::Database)?;
        let rows = statement
            .query_map([], |row| {
                let state_value: String = row.get(2)?;
                Ok(TaskRecord {
                    id: row.get(0)?,
                    notice_id: row.get(1)?,
                    state: TaskState::from_db(&state_value).ok_or(rusqlite::Error::InvalidQuery)?,
                    current_revision_id: row.get(3)?,
                    payload: row.get(4)?.unwrap_or_default(),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|_| RepositoryError::Database)?;
        rows.map(|row| row.map_err(|_| RepositoryError::Database))
            .collect()
    }

    pub fn transition_task(
        &self,
        task_id: &str,
        state: TaskState,
        payload: &[u8],
        revision_id: &str,
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| RepositoryError::Database)?;
        let current_revision: Option<String> = transaction
            .query_row(
                "SELECT current_revision_id FROM tasks WHERE id = ?1",
                [task_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| RepositoryError::Database)?
            .flatten();
        if current_revision.is_none() {
            return Err(RepositoryError::MissingTask);
        }
        let revision_number: i64 = transaction.query_row("SELECT COALESCE(MAX(revision_number), 0) + 1 FROM task_revisions WHERE task_id = ?1", [task_id], |row| row.get(0)).map_err(|_| RepositoryError::Database)?;
        transaction.execute("INSERT INTO task_revisions (id, task_id, revision_number, payload, created_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))", params![revision_id, task_id, revision_number, payload]).map_err(|_| RepositoryError::Database)?;
        transaction.execute("UPDATE tasks SET task_state = ?2, current_revision_id = ?3, updated_at = datetime('now') WHERE id = ?1", params![task_id, state.as_db(), revision_id]).map_err(|_| RepositoryError::Database)?;
        insert_audit(&transaction, "task", task_id, state.as_db(), Some(payload))?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }

    pub fn create_manual_task(
        &self,
        task_id: &str,
        task_revision_id: &str,
        payload: &[u8],
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| RepositoryError::Database)?;
        transaction
            .execute(
                "INSERT INTO tasks (id, task_state, current_revision_id, created_at, updated_at)
             VALUES (?1, 'todo', ?2, datetime('now'), datetime('now'))",
                params![task_id, task_revision_id],
            )
            .map_err(|_| RepositoryError::Database)?;
        transaction
            .execute(
                "INSERT INTO task_revisions (id, task_id, revision_number, payload, created_at)
             VALUES (?1, ?2, 1, ?3, datetime('now'))",
                params![task_revision_id, task_id, payload],
            )
            .map_err(|_| RepositoryError::Database)?;
        insert_audit(
            &transaction,
            "task",
            task_id,
            "createdManually",
            Some(payload),
        )?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }

    pub fn task_history(&self, task_id: &str) -> Result<Vec<Vec<u8>>, RepositoryError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT payload FROM task_revisions WHERE task_id = ?1 ORDER BY revision_number",
            )
            .map_err(|_| RepositoryError::Database)?;
        let rows = statement
            .query_map([task_id], |row| row.get(0))
            .map_err(|_| RepositoryError::Database)?;
        rows.map(|row| row.map_err(|_| RepositoryError::Database))
            .collect()
    }

    pub fn create_relation(&self, relation: &NoticeRelationRecord) -> Result<(), RepositoryError> {
        self.connection.execute("INSERT INTO notice_relations (id, notice_id, related_notice_id, relation_type, relation_state, evidence, created_at) VALUES (?1, ?2, ?3, ?4, 'suggested', ?5, datetime('now'))", params![relation.id, relation.notice_id, relation.related_notice_id, relation.relation_type, relation.evidence]).map_err(|_| RepositoryError::Database)?;
        Ok(())
    }

    pub fn list_relations(
        &self,
        notice_id: Option<&str>,
    ) -> Result<Vec<NoticeRelationRecord>, RepositoryError> {
        let mut statement = self.connection.prepare("SELECT id, notice_id, related_notice_id, relation_type, relation_state, COALESCE(evidence, X''), created_at FROM notice_relations WHERE (?1 IS NULL OR notice_id = ?1) ORDER BY created_at DESC").map_err(|_| RepositoryError::Database)?;
        let rows = statement
            .query_map([notice_id], |row| {
                Ok(NoticeRelationRecord {
                    id: row.get(0)?,
                    notice_id: row.get(1)?,
                    related_notice_id: row.get(2)?,
                    relation_type: row.get(3)?,
                    relation_state: row.get(4)?,
                    evidence: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|_| RepositoryError::Database)?;
        rows.map(|row| row.map_err(|_| RepositoryError::Database))
            .collect()
    }

    pub fn set_relation_state(
        &self,
        relation_id: &str,
        state: &str,
    ) -> Result<(), RepositoryError> {
        let changed = self
            .connection
            .execute(
                "UPDATE notice_relations SET relation_state = ?2 WHERE id = ?1",
                params![relation_id, state],
            )
            .map_err(|_| RepositoryError::Database)?;
        if changed == 0 {
            return Err(RepositoryError::MissingRelation);
        }
        Ok(())
    }

    pub fn create_text_notice(
        &self,
        notice_id: &str,
        original_text: &str,
        published_at: &str,
    ) -> Result<(), RepositoryError> {
        self.connection
            .execute(
                "INSERT INTO notices (
                    id, source_kind, original_text, published_at, published_time_source,
                    inbox_state, created_at, updated_at
                 ) VALUES (?1, 'text', ?2, ?3, 'importTimeTentative', ?4, datetime('now'), datetime('now'))",
                params![notice_id, original_text.as_bytes(), published_at, NoticeState::PendingAnalysis.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        Ok(())
    }

    pub fn create_image_notice(
        &self,
        notice_id: &str,
        published_at: &str,
        candidate: Option<&PublishedTimeCandidate>,
        asset: &SourceAsset,
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| RepositoryError::Database)?;
        transaction
            .execute(
                "INSERT INTO notices (
                    id, source_kind, published_at, published_time_source,
                    published_time_candidate, published_time_candidate_source,
                    inbox_state, created_at, updated_at
                 ) VALUES (?1, 'image', ?2, 'importTimeTentative', ?4, ?5, ?3, datetime('now'), datetime('now'))",
                params![
                    notice_id,
                    published_at,
                    NoticeState::PendingAnalysis.as_db(),
                    candidate.map(|value| value.published_at.as_str()),
                    candidate.map(|value| value.source.as_str()),
                ],
            )
            .map_err(|_| RepositoryError::Database)?;
        transaction
            .execute(
                "INSERT INTO source_assets (
                    id, notice_id, media_type, encrypted_file_name, encrypted_data_key,
                    key_nonce, content_nonce, ciphertext_checksum, byte_size, pixel_width,
                    pixel_height, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
                params![
                    asset.id,
                    notice_id,
                    asset.media_type,
                    asset.encrypted_file_name,
                    asset.metadata.encrypted_data_key,
                    asset.metadata.key_nonce.as_slice(),
                    asset.metadata.content_nonce.as_slice(),
                    asset.metadata.ciphertext_checksum.as_slice(),
                    asset.byte_size as i64,
                    asset.pixel_width.map(i64::from),
                    asset.pixel_height.map(i64::from),
                ],
            )
            .map_err(|_| RepositoryError::Database)?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }

    pub fn list_notices(
        &self,
        state: Option<NoticeState>,
    ) -> Result<Vec<NoticeSummary>, RepositoryError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, source_kind, inbox_state, COALESCE(published_at, created_at),
                        published_time_source, published_time_candidate,
                        published_time_candidate_source, created_at,
                        COALESCE(CAST(original_text AS TEXT), '')
                 FROM notices
                 WHERE (?1 IS NULL OR inbox_state = ?1)
                 ORDER BY created_at DESC",
            )
            .map_err(|_| RepositoryError::Database)?;
        let rows = statement
            .query_map([state.map(NoticeState::as_db)], |row| {
                let state_value: String = row.get(2)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    state_value,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(|_| RepositoryError::Database)?;
        rows.map(|row| {
            let (
                id,
                source_kind,
                state_value,
                published_at,
                published_time_source,
                published_time_candidate,
                published_time_candidate_source,
                created_at,
                original_text,
            ) = row.map_err(|_| RepositoryError::Database)?;
            let inbox_state =
                NoticeState::from_db(&state_value).ok_or(RepositoryError::Database)?;
            Ok(NoticeSummary {
                id,
                source_kind,
                inbox_state,
                published_at,
                published_time_source,
                published_time_candidate,
                published_time_candidate_source,
                created_at,
                excerpt: original_text.chars().take(96).collect(),
            })
        })
        .collect()
    }

    pub fn notice_detail(&self, notice_id: &str) -> Result<NoticeDetail, RepositoryError> {
        let row = self
            .connection
            .query_row(
                "SELECT id, source_kind, inbox_state, COALESCE(published_at, created_at),
                        published_time_source, published_time_candidate,
                        published_time_candidate_source, created_at, COALESCE(CAST(original_text AS TEXT), '')
                 FROM notices WHERE id = ?1",
                [notice_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| RepositoryError::Database)?
            .ok_or(RepositoryError::MissingNotice)?;
        let state = NoticeState::from_db(&row.2).ok_or(RepositoryError::Database)?;
        let summary = NoticeSummary {
            id: row.0,
            source_kind: row.1,
            inbox_state: state,
            published_at: row.3,
            published_time_source: row.4,
            published_time_candidate: row.5,
            published_time_candidate_source: row.6,
            created_at: row.7,
            excerpt: row.8.chars().take(96).collect(),
        };
        let original_text = if summary.source_kind == "text" {
            Some(row.8)
        } else {
            None
        };
        let source_asset = self.source_asset_for_notice(&summary.id)?;
        Ok(NoticeDetail {
            summary,
            original_text,
            source_asset,
        })
    }

    pub fn update_published_time(
        &self,
        notice_id: &str,
        published_at: &str,
    ) -> Result<(), RepositoryError> {
        let changed = self.connection.execute(
            "UPDATE notices SET published_at = ?2, published_time_source = 'userConfirmed', published_time_candidate = NULL, published_time_candidate_source = NULL, updated_at = datetime('now') WHERE id = ?1",
            params![notice_id, published_at],
        ).map_err(|_| RepositoryError::Database)?;
        if changed == 0 {
            return Err(RepositoryError::MissingNotice);
        }
        Ok(())
    }

    pub fn set_notice_state(
        &self,
        notice_id: &str,
        state: NoticeState,
    ) -> Result<(), RepositoryError> {
        let changed = self
            .connection
            .execute(
                "UPDATE notices SET inbox_state = ?2, updated_at = datetime('now') WHERE id = ?1",
                params![notice_id, state.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        if changed == 0 {
            return Err(RepositoryError::MissingNotice);
        }
        Ok(())
    }

    fn source_asset_for_notice(
        &self,
        notice_id: &str,
    ) -> Result<Option<SourceAsset>, RepositoryError> {
        self.connection.query_row(
            "SELECT id, media_type, encrypted_file_name, encrypted_data_key, key_nonce, content_nonce,
                    ciphertext_checksum, byte_size, pixel_width, pixel_height
             FROM source_assets WHERE notice_id = ?1 ORDER BY created_at ASC LIMIT 1",
            [notice_id],
            |row| {
                let key_nonce = to_nonce(row.get::<_, Vec<u8>>(4)?)?;
                let content_nonce = to_nonce(row.get::<_, Vec<u8>>(5)?)?;
                let checksum = to_checksum(row.get::<_, Vec<u8>>(6)?)?;
                Ok(SourceAsset {
                    id: row.get(0)?, media_type: row.get(1)?, encrypted_file_name: row.get(2)?,
                    metadata: EncryptedAttachmentMetadata { encrypted_data_key: row.get(3)?, key_nonce, content_nonce, ciphertext_checksum: checksum },
                    byte_size: row.get::<_, i64>(7)? as usize,
                    pixel_width: row.get::<_, Option<i64>>(8)?.map(|value| value as u32),
                    pixel_height: row.get::<_, Option<i64>>(9)?.map(|value| value as u32),
                })
            },
        ).optional().map_err(|_| RepositoryError::Database)
    }
}

fn to_nonce(value: Vec<u8>) -> rusqlite::Result<[u8; 12]> {
    value.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn to_checksum(value: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    value.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}

pub struct NewCandidate<'value> {
    pub id: &'value str,
    pub payload: &'value [u8],
}

fn insert_audit(
    transaction: &Transaction<'_>,
    entity_type: &str,
    entity_id: &str,
    action: &str,
    payload: Option<&[u8]>,
) -> Result<(), RepositoryError> {
    transaction
        .execute(
            "INSERT INTO audit_events (id, entity_type, entity_id, action, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            params![
                uuid::Uuid::new_v4().to_string(),
                entity_type,
                entity_id,
                action,
                payload,
            ],
        )
        .map_err(|_| RepositoryError::Database)?;
    Ok(())
}

struct CandidateForConfirmation {
    notice_id: String,
}

fn candidate_for_confirmation(
    transaction: &Transaction<'_>,
    candidate_id: &str,
) -> Result<CandidateForConfirmation, RepositoryError> {
    let result = transaction.query_row(
        "SELECT revision.notice_id, candidate.candidate_state
         FROM task_candidates candidate
         INNER JOIN analysis_revisions revision ON revision.id = candidate.analysis_revision_id
         WHERE candidate.id = ?1",
        [candidate_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    );
    match result {
        Ok((notice_id, state)) if state == CandidateState::Pending.as_db() => {
            Ok(CandidateForConfirmation { notice_id })
        }
        Ok(_) => Err(RepositoryError::InvalidState),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(RepositoryError::MissingCandidate),
        Err(_) => Err(RepositoryError::Database),
    }
}

#[cfg(test)]
mod tests {
    use super::{NewCandidate, NoticeRepository, NoticeState, RepositoryError};
    use crate::{security::key_protection::MasterKey, storage::database::EncryptedDatabase};
    use tempfile::tempdir;

    fn database() -> EncryptedDatabase {
        let directory = tempdir().unwrap();
        let path = directory.keep().join("inbox.db");
        EncryptedDatabase::open(&path, &MasterKey::from_bytes(&[3; 32]).unwrap()).unwrap()
    }

    #[test]
    fn confirming_a_candidate_creates_a_complete_task_atomically() {
        let database = database();
        let repository = NoticeRepository::new(database.connection());
        repository.create_notice("notice-1", "text").unwrap();
        repository
            .save_analysis_revision(
                "notice-1",
                "analysis-1",
                "rule-v1",
                &[NewCandidate {
                    id: "candidate-1",
                    payload: b"{}",
                }],
            )
            .unwrap();
        repository
            .confirm_candidate("candidate-1", "task-1", "task-revision-1", b"{}")
            .unwrap();

        let task_count: i64 = database
            .connection()
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        let notice_state: String = database
            .connection()
            .query_row(
                "SELECT inbox_state FROM notices WHERE id = 'notice-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(task_count, 1);
        assert_eq!(notice_state, NoticeState::Processed.as_db());
    }

    #[test]
    fn failed_confirmation_rolls_back_without_a_partial_task() {
        let database = database();
        let repository = NoticeRepository::new(database.connection());
        repository.create_notice("notice-2", "text").unwrap();
        repository
            .save_analysis_revision(
                "notice-2",
                "analysis-2",
                "rule-v1",
                &[NewCandidate {
                    id: "candidate-2",
                    payload: b"{}",
                }],
            )
            .unwrap();

        database
            .connection()
            .execute(
                "INSERT INTO tasks (id, task_state, created_at, updated_at) VALUES ('task-2', 'todo', datetime('now'), datetime('now'))",
                [],
            )
            .unwrap();

        let error = repository.confirm_candidate("candidate-2", "task-2", "task-revision-2", b"{}");
        assert!(matches!(error, Err(RepositoryError::Database)));
        let task_count: i64 = database
            .connection()
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        let candidate_state: String = database
            .connection()
            .query_row(
                "SELECT candidate_state FROM task_candidates WHERE id = 'candidate-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(task_count, 1);
        assert_eq!(candidate_state, "pending");
    }

    #[test]
    fn reanalysis_creates_a_new_revision_without_overwriting_the_previous_one() {
        let database = database();
        let repository = NoticeRepository::new(database.connection());
        repository.create_notice("notice-revision", "text").unwrap();
        repository
            .save_analysis_revision_full(
                "notice-revision",
                "analysis-revision-1",
                "rule-v1",
                Some("第一版 OCR"),
                &[NewCandidate {
                    id: "candidate-revision-1",
                    payload: b"{}",
                }],
            )
            .unwrap();
        repository
            .save_analysis_revision_full(
                "notice-revision",
                "analysis-revision-2",
                "rule-v2",
                Some("第二版 OCR"),
                &[NewCandidate {
                    id: "candidate-revision-2",
                    payload: b"{}",
                }],
            )
            .unwrap();

        let revisions: Vec<(i64, String)> = database
            .connection()
            .prepare(
                "SELECT revision_number, CAST(ocr_text AS TEXT)
                 FROM analysis_revisions WHERE notice_id = 'notice-revision'
                 ORDER BY revision_number",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            revisions,
            vec![(1, "第一版 OCR".to_owned()), (2, "第二版 OCR".to_owned())]
        );
    }

    #[test]
    fn preserves_each_original_text_submission_and_supports_state_filters() {
        let database = database();
        let repository = NoticeRepository::new(database.connection());
        repository
            .create_text_notice("notice-a", "同一条通知", "2026-08-27T09:00:00Z")
            .unwrap();
        repository
            .create_text_notice("notice-b", "同一条通知", "2026-08-27T09:00:00Z")
            .unwrap();
        repository
            .set_notice_state("notice-b", NoticeState::InformationOnly)
            .unwrap();

        let pending = repository
            .list_notices(Some(NoticeState::PendingAnalysis))
            .unwrap();
        let information_only = repository
            .list_notices(Some(NoticeState::InformationOnly))
            .unwrap();
        let detail = repository.notice_detail("notice-a").unwrap();

        assert_eq!(pending.len(), 1);
        assert_eq!(information_only.len(), 1);
        assert_eq!(detail.original_text.as_deref(), Some("同一条通知"));
        assert_eq!(detail.summary.published_time_source, "importTimeTentative");
    }

    #[test]
    fn confirming_published_time_does_not_change_the_original_text() {
        let database = database();
        let repository = NoticeRepository::new(database.connection());
        repository
            .create_text_notice("notice-time", "保留原文", "2026-08-27T09:00:00Z")
            .unwrap();
        repository
            .update_published_time("notice-time", "2026-08-28T09:00:00Z")
            .unwrap();
        let detail = repository.notice_detail("notice-time").unwrap();

        assert_eq!(detail.original_text.as_deref(), Some("保留原文"));
        assert_eq!(detail.summary.published_at, "2026-08-28T09:00:00Z");
        assert_eq!(detail.summary.published_time_source, "userConfirmed");
    }
}

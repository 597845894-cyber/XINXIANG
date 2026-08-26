use rusqlite::{params, Connection, Transaction};

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
    fn as_db(self) -> &'static str {
        match self {
            Self::PendingAnalysis => "pendingAnalysis",
            Self::PendingReview => "pendingReview",
            Self::PartiallyProcessed => "partiallyProcessed",
            Self::Processed => "processed",
            Self::InformationOnly => "informationOnly",
            Self::Failed => "failed",
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
    fn as_db(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Ignored => "ignored",
            Self::Merged => "merged",
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
    fn as_db(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug)]
pub enum RepositoryError {
    Database,
    InvalidState,
    MissingCandidate,
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::Database => "REPOSITORY_DATABASE_FAILED",
            Self::InvalidState => "REPOSITORY_INVALID_STATE",
            Self::MissingCandidate => "REPOSITORY_CANDIDATE_MISSING",
        };
        formatter.write_str(code)
    }
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
                    id, notice_id, revision_number, analysis_state, classifier_version, created_at
                 ) VALUES (?1, ?2, ?3, 'complete', ?4, datetime('now'))",
                params![
                    analysis_revision_id,
                    notice_id,
                    revision_number,
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
        transaction
            .execute(
                "UPDATE notices SET inbox_state = ?2, updated_at = datetime('now') WHERE id = ?1",
                params![candidate.notice_id, NoticeState::Processed.as_db()],
            )
            .map_err(|_| RepositoryError::Database)?;
        transaction.commit().map_err(|_| RepositoryError::Database)
    }
}

pub struct NewCandidate<'value> {
    pub id: &'value str,
    pub payload: &'value [u8],
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
}

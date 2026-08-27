use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::security::key_protection::MasterKey;

const CURRENT_SCHEMA_VERSION: i64 = 7;

#[derive(Debug)]
pub enum DatabaseError {
    Open,
    CipherUnavailable,
    Migration,
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::Open => "DATABASE_OPEN_FAILED",
            Self::CipherUnavailable => "DATABASE_CIPHER_UNAVAILABLE",
            Self::Migration => "DATABASE_MIGRATION_FAILED",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for DatabaseError {}

pub struct EncryptedDatabase {
    connection: Connection,
}

impl EncryptedDatabase {
    pub fn open(path: &Path, master_key: &MasterKey) -> Result<Self, DatabaseError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )
        .map_err(|_| DatabaseError::Open)?;

        configure_connection(&connection, master_key)?;
        apply_migrations(&connection)?;
        Ok(Self { connection })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn schema_version(&self) -> Result<i64, DatabaseError> {
        self.connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|_| DatabaseError::Migration)
    }
}

fn configure_connection(
    connection: &Connection,
    master_key: &MasterKey,
) -> Result<(), DatabaseError> {
    let key_hex = master_key
        .expose()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    connection
        .execute_batch(&format!(
            "PRAGMA key = \"x'{key_hex}'\";\
             PRAGMA foreign_keys = ON;\
             PRAGMA journal_mode = WAL;\
             PRAGMA secure_delete = ON;"
        ))
        .map_err(|_| DatabaseError::Open)?;

    let cipher_version: String = connection
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .map_err(|_| DatabaseError::CipherUnavailable)?;
    if cipher_version.trim().is_empty() {
        return Err(DatabaseError::CipherUnavailable);
    }
    Ok(())
}

fn apply_migrations(connection: &Connection) -> Result<(), DatabaseError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )
        .map_err(|_| DatabaseError::Migration)?;

    for version in 1..=CURRENT_SCHEMA_VERSION {
        let is_applied: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |row| row.get(0),
            )
            .map_err(|_| DatabaseError::Migration)?;
        if is_applied {
            continue;
        }

        let transaction = connection
            .unchecked_transaction()
            .map_err(|_| DatabaseError::Migration)?;
        match version {
            1 => create_initial_schema(&transaction)?,
            2 => create_schema_indexes(&transaction)?,
            3 => add_attachment_content_nonce(&transaction)?,
            4 => add_notice_capture_fields(&transaction)?,
            5 => add_published_time_candidates(&transaction)?,
            6 => add_task_management_audit(&transaction)?,
            7 => add_task_source_removal_status(&transaction)?,
            _ => return Err(DatabaseError::Migration),
        }
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, datetime('now'))",
                [version],
            )
            .map_err(|_| DatabaseError::Migration)?;
        transaction.commit().map_err(|_| DatabaseError::Migration)?;
    }
    Ok(())
}

fn create_initial_schema(transaction: &rusqlite::Transaction<'_>) -> Result<(), DatabaseError> {
    transaction
        .execute_batch(
            "CREATE TABLE notices (
                id TEXT PRIMARY KEY,
                source_kind TEXT NOT NULL,
                published_at TEXT,
                inbox_state TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE source_assets (
                id TEXT PRIMARY KEY,
                notice_id TEXT NOT NULL REFERENCES notices(id) ON DELETE CASCADE,
                media_type TEXT NOT NULL,
                encrypted_file_name TEXT NOT NULL,
                encrypted_data_key BLOB NOT NULL,
                nonce BLOB NOT NULL,
                ciphertext_checksum BLOB NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE analysis_revisions (
                id TEXT PRIMARY KEY,
                notice_id TEXT NOT NULL REFERENCES notices(id) ON DELETE CASCADE,
                revision_number INTEGER NOT NULL,
                analysis_state TEXT NOT NULL,
                ocr_text BLOB,
                classifier_version TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(notice_id, revision_number)
            );
            CREATE TABLE task_candidates (
                id TEXT PRIMARY KEY,
                analysis_revision_id TEXT NOT NULL REFERENCES analysis_revisions(id) ON DELETE CASCADE,
                candidate_state TEXT NOT NULL,
                structured_payload BLOB NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE tasks (
                id TEXT PRIMARY KEY,
                notice_id TEXT REFERENCES notices(id) ON DELETE SET NULL,
                task_state TEXT NOT NULL,
                current_revision_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE task_revisions (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                revision_number INTEGER NOT NULL,
                source_candidate_id TEXT REFERENCES task_candidates(id) ON DELETE SET NULL,
                payload BLOB NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(task_id, revision_number)
            );
            CREATE TABLE reminders (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                scheduled_at TEXT NOT NULL,
                reminder_state TEXT NOT NULL,
                idempotency_key TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
            );
            CREATE TABLE notice_relations (
                id TEXT PRIMARY KEY,
                notice_id TEXT NOT NULL REFERENCES notices(id) ON DELETE CASCADE,
                related_notice_id TEXT NOT NULL REFERENCES notices(id) ON DELETE CASCADE,
                relation_type TEXT NOT NULL,
                relation_state TEXT NOT NULL,
                created_at TEXT NOT NULL,
                CHECK(notice_id <> related_notice_id),
                UNIQUE(notice_id, related_notice_id, relation_type)
            );",
        )
        .map_err(|_| DatabaseError::Migration)
}

fn create_schema_indexes(transaction: &rusqlite::Transaction<'_>) -> Result<(), DatabaseError> {
    transaction
        .execute_batch(
            "CREATE INDEX source_assets_by_notice ON source_assets(notice_id);
             CREATE INDEX analysis_revisions_by_notice ON analysis_revisions(notice_id, revision_number);
             CREATE INDEX task_candidates_by_revision ON task_candidates(analysis_revision_id);
             CREATE INDEX reminders_by_task_time ON reminders(task_id, scheduled_at);
             CREATE INDEX notice_relations_by_notice ON notice_relations(notice_id);",
        )
        .map_err(|_| DatabaseError::Migration)
}

fn add_attachment_content_nonce(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<(), DatabaseError> {
    transaction
        .execute_batch(
            "ALTER TABLE source_assets RENAME COLUMN nonce TO key_nonce;
             ALTER TABLE source_assets ADD COLUMN content_nonce BLOB NOT NULL DEFAULT X'';",
        )
        .map_err(|_| DatabaseError::Migration)
}

fn add_notice_capture_fields(transaction: &rusqlite::Transaction<'_>) -> Result<(), DatabaseError> {
    transaction
        .execute_batch(
            "ALTER TABLE notices ADD COLUMN original_text BLOB;
             ALTER TABLE notices ADD COLUMN published_time_source TEXT NOT NULL DEFAULT 'importTimeTentative';
             ALTER TABLE source_assets ADD COLUMN byte_size INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE source_assets ADD COLUMN pixel_width INTEGER;
             ALTER TABLE source_assets ADD COLUMN pixel_height INTEGER;
             CREATE INDEX notices_by_state_updated ON notices(inbox_state, updated_at DESC);",
        )
        .map_err(|_| DatabaseError::Migration)
}

fn add_published_time_candidates(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<(), DatabaseError> {
    transaction
        .execute_batch(
            "ALTER TABLE notices ADD COLUMN published_time_candidate TEXT;
             ALTER TABLE notices ADD COLUMN published_time_candidate_source TEXT;",
        )
        .map_err(|_| DatabaseError::Migration)
}

fn add_task_management_audit(transaction: &rusqlite::Transaction<'_>) -> Result<(), DatabaseError> {
    transaction
        .execute_batch(
            "CREATE TABLE audit_events (
                id TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                action TEXT NOT NULL,
                payload BLOB,
                created_at TEXT NOT NULL
            );
            CREATE INDEX audit_events_by_entity ON audit_events(entity_type, entity_id, created_at);
            CREATE INDEX task_revisions_by_task ON task_revisions(task_id, revision_number);
            CREATE INDEX tasks_by_state ON tasks(task_state, updated_at DESC);
            ALTER TABLE notice_relations ADD COLUMN evidence BLOB;",
        )
        .map_err(|_| DatabaseError::Migration)
}

fn add_task_source_removal_status(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<(), DatabaseError> {
    transaction
        .execute_batch(
            "ALTER TABLE tasks ADD COLUMN source_removed_at TEXT;
             CREATE INDEX tasks_by_source_removal ON tasks(source_removed_at);",
        )
        .map_err(|_| DatabaseError::Migration)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_migrations, configure_connection, EncryptedDatabase, CURRENT_SCHEMA_VERSION,
    };
    use crate::security::key_protection::MasterKey;
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn test_key() -> MasterKey {
        MasterKey::from_bytes(&[7; 32]).unwrap()
    }

    #[test]
    fn creates_an_encrypted_database_with_all_domain_tables() {
        let directory = tempdir().unwrap();
        let database =
            EncryptedDatabase::open(&directory.path().join("inbox.db"), &test_key()).unwrap();
        let table_count: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                    'notices', 'source_assets', 'analysis_revisions', 'task_candidates',
                    'tasks', 'task_revisions', 'reminders', 'notice_relations'
                )",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(table_count, 8);
        assert_eq!(database.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        let cipher_version: String = database
            .connection()
            .query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .unwrap();
        assert!(!cipher_version.is_empty());
    }

    #[test]
    fn applies_the_next_migration_to_an_existing_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("inbox.db");
        let key = test_key();
        let connection = Connection::open(&path).unwrap();
        configure_connection(&connection, &key).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
                 INSERT INTO schema_migrations (version, applied_at) VALUES (1, '2026-01-01T00:00:00Z');
                 CREATE TABLE notices (id TEXT PRIMARY KEY, source_kind TEXT NOT NULL, published_at TEXT, inbox_state TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
                 CREATE TABLE source_assets (id TEXT PRIMARY KEY, notice_id TEXT NOT NULL REFERENCES notices(id) ON DELETE CASCADE, media_type TEXT NOT NULL, encrypted_file_name TEXT NOT NULL, encrypted_data_key BLOB NOT NULL, nonce BLOB NOT NULL, ciphertext_checksum BLOB NOT NULL, created_at TEXT NOT NULL);
                 CREATE TABLE analysis_revisions (id TEXT PRIMARY KEY, notice_id TEXT NOT NULL REFERENCES notices(id) ON DELETE CASCADE, revision_number INTEGER NOT NULL, analysis_state TEXT NOT NULL, ocr_text BLOB, classifier_version TEXT NOT NULL, created_at TEXT NOT NULL, UNIQUE(notice_id, revision_number));
                 CREATE TABLE task_candidates (id TEXT PRIMARY KEY, analysis_revision_id TEXT NOT NULL REFERENCES analysis_revisions(id) ON DELETE CASCADE, candidate_state TEXT NOT NULL, structured_payload BLOB NOT NULL, created_at TEXT NOT NULL);
                 CREATE TABLE tasks (id TEXT PRIMARY KEY, notice_id TEXT REFERENCES notices(id) ON DELETE SET NULL, task_state TEXT NOT NULL, current_revision_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
                 CREATE TABLE task_revisions (id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE, revision_number INTEGER NOT NULL, source_candidate_id TEXT REFERENCES task_candidates(id) ON DELETE SET NULL, payload BLOB NOT NULL, created_at TEXT NOT NULL, UNIQUE(task_id, revision_number));
                 CREATE TABLE reminders (id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE, scheduled_at TEXT NOT NULL, reminder_state TEXT NOT NULL, idempotency_key TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
                 CREATE TABLE notice_relations (id TEXT PRIMARY KEY, notice_id TEXT NOT NULL REFERENCES notices(id) ON DELETE CASCADE, related_notice_id TEXT NOT NULL REFERENCES notices(id) ON DELETE CASCADE, relation_type TEXT NOT NULL, relation_state TEXT NOT NULL, created_at TEXT NOT NULL, CHECK(notice_id <> related_notice_id), UNIQUE(notice_id, related_notice_id, relation_type));",
            )
            .unwrap();
        apply_migrations(&connection).unwrap();
        let version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
        let index_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'reminders_by_task_time')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(index_exists);
        let attachment_nonce_columns: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('source_assets')
                 WHERE name IN ('key_nonce', 'content_nonce')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(attachment_nonce_columns, 2);
        let capture_columns: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('notices')
                 WHERE name IN ('original_text', 'published_time_source')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(capture_columns, 2);
        let candidate_columns: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('notices')
                 WHERE name IN ('published_time_candidate', 'published_time_candidate_source')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(candidate_columns, 2);
    }
}

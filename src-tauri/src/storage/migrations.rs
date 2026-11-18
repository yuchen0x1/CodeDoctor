use crate::error::AppError;
use rusqlite::Connection;

pub const MIGRATION_V1: &str = include_str!("../../migrations/0001_initial.sql");
pub const MIGRATION_V2: &str = include_str!("../../migrations/0002_scan_cache.sql");

pub fn run(conn: &mut Connection) -> Result<(), AppError> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let transaction = conn.transaction()?;
    let metadata_exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_metadata')",
        [],
        |row| row.get(0),
    )?;
    let current_version = if metadata_exists {
        transaction.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_metadata",
            [],
            |row| row.get::<_, i64>(0),
        )?
    } else {
        0
    };

    if current_version < 1 {
        transaction.execute_batch(MIGRATION_V1)?;
        transaction.execute(
            "INSERT INTO schema_metadata (version, applied_at) VALUES (1, datetime('now'))",
            [],
        )?;
    }
    if current_version < 2 {
        transaction.execute_batch(MIGRATION_V2)?;
        transaction.execute(
            "INSERT INTO schema_metadata (version, applied_at) VALUES (2, datetime('now'))",
            [],
        )?;
    }

    transaction.commit()?;
    Ok(())
}

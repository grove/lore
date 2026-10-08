use rusqlite::Connection;

/// Apply the immutable historical-evidence schema. Future migrations must be
/// added as separate versioned SQL files, not edits to a released migration.
pub const SCHEMA_V1: &str = include_str!("../migrations/0001_knowledge.sql");

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if version == 0 {
        conn.execute_batch(SCHEMA_V1)?;
    }
    Ok(())
}

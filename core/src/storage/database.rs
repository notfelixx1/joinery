//! Local SQLite storage: settings + undo journal.
//!
//! The database lives in the OS app-data directory. It never leaves the
//! machine — Joinery has no network features at all.

use std::fs;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS undo_journal (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    tool       TEXT NOT NULL,
    payload    TEXT NOT NULL,
    undone     INTEGER NOT NULL DEFAULT 0
);
";

/// Open (or create) the database file and apply the schema.
pub fn open(path: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    migrate(&conn)?;
    Ok(conn)
}

/// In-place migrations. Each one is idempotent and ignores its own
/// "already exists" error.
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    // Databases created before the durable journal miss the committed
    // column; every pre-existing row counts as fully applied.
    let _ = conn.execute(
        "ALTER TABLE undo_journal ADD COLUMN committed INTEGER NOT NULL DEFAULT 1",
        [],
    );
    Ok(())
}

/// In-memory database, used by tests.
#[cfg(test)]
pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Read one setting. Storage errors surface as `Err` — a broken
/// database must not silently look like "no setting". Callers choose
/// their own fallback for the error case.
pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip() {
        let conn = open_in_memory().expect("db");
        assert_eq!(get_setting(&conn, "theme").expect("get"), None);
        set_setting(&conn, "theme", "dark").expect("set");
        assert_eq!(
            get_setting(&conn, "theme").expect("get"),
            Some("dark".to_string())
        );
        set_setting(&conn, "theme", "light").expect("upsert");
        assert_eq!(
            get_setting(&conn, "theme").expect("get"),
            Some("light".to_string())
        );
    }

    #[test]
    fn open_creates_file_and_schema() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("nested").join("joinery.db");
        let conn = open(&db_path).expect("open");
        // Schema is applied — inserts must succeed.
        set_setting(&conn, "language", "ru").expect("set");
        assert_eq!(
            get_setting(&conn, "language").expect("get"),
            Some("ru".to_string())
        );
        assert!(db_path.exists());
    }
}

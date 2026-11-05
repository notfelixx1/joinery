//! Undo journal.
//!
//! Every destructive-but-reversible operation is recorded in the local
//! SQLite database before the UI reports success. Undo replays the
//! stored records backwards; entries already undone are skipped.

use std::fs;
use std::io;
use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::core::renamer::RenameRecord;

/// Journal row as shown in the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub id: i64,
    pub tool: String,
    pub created_at: String,
    pub op_count: usize,
    pub undone: bool,
}

/// Persist a list of records; returns the journal id.
pub fn save_journal(
    conn: &Connection,
    tool: &str,
    records: &[RenameRecord],
) -> rusqlite::Result<i64> {
    let payload = serde_json::to_string(records).expect("serialize journal payload");
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO undo_journal (created_at, tool, payload, undone) VALUES (?1, ?2, ?3, 0)",
        params![now, tool, payload],
    )?;
    Ok(conn.last_insert_rowid())
}

/// List journal entries, newest first.
pub fn list_journals(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<JournalEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, tool, created_at, payload, undone
         FROM undo_journal
         ORDER BY id DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        let payload: String = row.get(3)?;
        let op_count = serde_json::from_str::<Vec<RenameRecord>>(&payload)
            .map(|records| records.len())
            .unwrap_or(0);
        Ok(JournalEntry {
            id: row.get(0)?,
            tool: row.get(1)?,
            created_at: row.get(2)?,
            op_count,
            undone: row.get::<_, i64>(4)? != 0,
        })
    })?;
    rows.collect()
}

/// Load the records of one journal and mark it as not-yet-undone.
pub fn load_records(conn: &Connection, journal_id: i64) -> Option<Vec<RenameRecord>> {
    let payload: Option<String> = conn
        .query_row(
            "SELECT payload FROM undo_journal WHERE id = ?1 AND undone = 0",
            params![journal_id],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten();
    payload.and_then(|p| serde_json::from_str(&p).ok())
}

/// Mark a journal entry as undone.
pub fn mark_undone(conn: &Connection, journal_id: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE undo_journal SET undone = 1 WHERE id = ?1",
        params![journal_id],
    )
}

/// Replay records in reverse: move every `to` back to its `from`.
/// Files that disappeared or were recreated meanwhile are skipped, not
/// overwritten.
pub fn undo_records(records: &[RenameRecord]) -> io::Result<usize> {
    let mut undone = 0;
    for record in records.iter().rev() {
        if record.to.exists() && !record.from.exists() {
            fs::rename(&record.to, &record.from)?;
            undone += 1;
        }
    }
    Ok(undone)
}

/// Delete paths into the system trash. Returns per-path results.
pub fn delete_to_trash(paths: &[PathBuf]) -> Vec<(PathBuf, bool, Option<String>)> {
    paths
        .iter()
        .map(|path| match trash::delete(path) {
            Ok(()) => (path.clone(), true, None),
            Err(err) => (path.clone(), false, Some(err.to_string())),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn memory_db() -> Connection {
        crate::storage::database::open_in_memory().expect("db")
    }

    #[test]
    fn journal_roundtrip() {
        let conn = memory_db();
        let records = vec![
            RenameRecord {
                from: PathBuf::from("/tmp/a.txt"),
                to: PathBuf::from("/tmp/b.txt"),
            },
            RenameRecord {
                from: PathBuf::from("/tmp/c.txt"),
                to: PathBuf::from("/tmp/d.txt"),
            },
        ];
        let id = save_journal(&conn, "renamer", &records).expect("save");
        assert!(id > 0);

        let list = list_journals(&conn, 10).expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].op_count, 2);
        assert!(!list[0].undone);
        assert_eq!(list[0].tool, "renamer");

        let loaded = load_records(&conn, id).expect("load");
        assert_eq!(loaded.len(), 2);

        mark_undone(&conn, id).expect("mark");
        let list = list_journals(&conn, 10).expect("list");
        assert!(list[0].undone);
        assert!(
            load_records(&conn, id).is_none(),
            "undone journals are not re-undoable"
        );
    }

    #[test]
    fn undo_records_skips_conflicts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        File::create(&a).expect("create a");
        fs::rename(&a, &b).expect("rename");

        // Someone recreates a.txt in the meantime → undo must skip.
        File::create(&a).expect("recreate a");

        let records = vec![RenameRecord {
            from: a.clone(),
            to: b.clone(),
        }];
        let undone = undo_records(&records).expect("undo");
        assert_eq!(undone, 0);
        assert!(a.exists() && b.exists());
    }
}

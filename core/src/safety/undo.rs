//! Undo journal.
//!
//! Every destructive-but-reversible operation is journaled as a durable
//! intent BEFORE the first file is touched. The row starts uncommitted;
//! after the operation succeeded it is committed and becomes undoable.
//! A crash in between leaves a pending row that [`recover_pending`]
//! settles at the next startup, so applied renames never lose their
//! undo path and never-applied intents never show up as undoable.
//!
//! Undo replays the stored records backwards; files that disappeared
//! or were recreated meanwhile are skipped, never overwritten, and one
//! failing record never stops the rest.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

/// Persist the intent to apply `records` before any file is touched.
/// The row starts uncommitted; [`mark_committed`] confirms it after
/// the operation succeeded. Returns the journal id.
pub fn save_intent(
    conn: &Connection,
    tool: &str,
    records: &[RenameRecord],
) -> rusqlite::Result<i64> {
    let payload = serde_json::to_string(records).expect("serialize journal payload");
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO undo_journal (created_at, tool, payload, undone, committed)
         VALUES (?1, ?2, ?3, 0, 0)",
        params![now, tool, payload],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Mark a journal as fully applied — undoable from now on.
pub fn mark_committed(conn: &Connection, journal_id: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE undo_journal SET committed = 1 WHERE id = ?1",
        params![journal_id],
    )
}

/// Drop a pending intent whose apply touched nothing.
pub fn delete_journal(conn: &Connection, journal_id: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM undo_journal WHERE id = ?1 AND committed = 0",
        params![journal_id],
    )
}

/// List committed journals, newest first.
pub fn list_journals(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<JournalEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, tool, created_at, payload, undone
         FROM undo_journal
         WHERE committed = 1
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

/// Load the records of one committed, not-yet-undone journal.
///
/// Storage errors surface as `Err` — a broken database must never look
/// like "nothing to undo".
pub fn load_records(
    conn: &Connection,
    journal_id: i64,
) -> rusqlite::Result<Option<Vec<RenameRecord>>> {
    let payload: Option<String> = conn
        .query_row(
            "SELECT payload FROM undo_journal WHERE id = ?1 AND undone = 0 AND committed = 1",
            params![journal_id],
            |row| row.get(0),
        )
        .optional()?;
    match payload {
        Some(p) => serde_json::from_str(&p).map(Some).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        }),
        None => Ok(None),
    }
}

/// Mark a journal entry as undone.
pub fn mark_undone(conn: &Connection, journal_id: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE undo_journal SET undone = 1 WHERE id = ?1",
        params![journal_id],
    )
}

/// Resolve uncommitted journals left by a crash between the intent and
/// the confirmation. No file operations — bookkeeping only:
/// - at least one rename applied → the journal becomes undoable;
/// - nothing applied → the intent is dropped.
pub fn recover_pending(conn: &Connection) -> rusqlite::Result<usize> {
    let rows: Vec<(i64, String)> = {
        let mut stmt = conn.prepare("SELECT id, payload FROM undo_journal WHERE committed = 0")?;
        let mapped = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        mapped.collect::<Result<_, _>>()?
    };
    let mut resolved = 0;
    for (id, payload) in rows {
        let records: Vec<RenameRecord> = match serde_json::from_str(&payload) {
            Ok(records) => records,
            Err(_) => continue, // corrupted payload: leave for manual inspection
        };
        let applied = records.iter().any(|r| {
            if r.from.parent() == r.to.parent()
                && crate::safety::fs::is_case_only_move(&r.from, &r.to)
            {
                // A case-only rename keeps the old name resolvable
                // forever — it is still the same file — so the generic
                // probe below would always report "not applied" and
                // drop the undo path. The real directory entries
                // decide instead: did the file under exactly the new
                // case appear?
                match (r.from.parent(), r.to.file_name()) {
                    (Some(parent), Some(name)) => parent_has_exact_entry(parent, name),
                    _ => false,
                }
            } else {
                r.to.exists() && !r.from.exists()
            }
        });
        if applied {
            mark_committed(conn, id)?;
        } else {
            conn.execute("DELETE FROM undo_journal WHERE id = ?1", params![id])?;
        }
        resolved += 1;
    }
    Ok(resolved)
}

/// Directory listing lookup that is case-sensitive everywhere (unlike
/// name resolution on Windows/macOS): `OsStr` compares byte-wise.
fn parent_has_exact_entry(parent: &Path, name: &std::ffi::OsStr) -> bool {
    match fs::read_dir(parent) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .any(|e| e.file_name() == name),
        Err(_) => false,
    }
}

/// Replay records in reverse: move every `to` back to its `from`.
/// Files that disappeared or were recreated meanwhile are skipped, not
/// overwritten, and one failing record never stops the rest.
pub fn undo_records(records: &[RenameRecord]) -> io::Result<usize> {
    let mut undone = 0;
    for record in records.iter().rev() {
        // Undo only ever swaps names inside one folder; a record whose
        // sides live in different folders is forged or corrupted.
        if record.to.parent() != record.from.parent() {
            continue;
        }
        // No-replace move: a file recreated under the original name is
        // never clobbered, a vanished file just skips.
        match crate::safety::fs::move_noreplace(&record.to, &record.from) {
            Ok(true) => undone += 1,
            Ok(false) | Err(_) => continue,
        }
    }
    Ok(undone)
}

/// Delete files into the system trash, re-checking each one first: a
/// file whose size no longer matches the scan is skipped, so a stale
/// scan cannot trash different content. Returns per-path results.
pub fn delete_to_trash(items: &[(PathBuf, u64)]) -> Vec<(PathBuf, bool, Option<String>)> {
    items
        .iter()
        .map(
            |(path, expected_size)| match fs::metadata(path).map(|m| m.len()) {
                Ok(size) if size == *expected_size => match trash::delete(path) {
                    Ok(()) => (path.clone(), true, None),
                    Err(err) => (path.clone(), false, Some(err.to_string())),
                },
                Ok(_) => (
                    path.clone(),
                    false,
                    Some("file changed since the scan".to_string()),
                ),
                Err(err) => (path.clone(), false, Some(err.to_string())),
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
        let id = save_intent(&conn, "renamer", &records).expect("save");
        assert!(id > 0);

        // Pending journals are invisible to listing and undo.
        let list = list_journals(&conn, 10).expect("list");
        assert!(list.is_empty());
        assert!(load_records(&conn, id).expect("load").is_none());

        mark_committed(&conn, id).expect("commit");
        let list = list_journals(&conn, 10).expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].op_count, 2);
        assert!(!list[0].undone);
        assert_eq!(list[0].tool, "renamer");

        let loaded = load_records(&conn, id).expect("load");
        assert_eq!(loaded.as_ref().map(|r| r.len()), Some(2));

        mark_undone(&conn, id).expect("mark");
        let list = list_journals(&conn, 10).expect("list");
        assert!(list[0].undone);
        assert!(
            load_records(&conn, id).expect("load").is_none(),
            "undone journals are not re-undoable"
        );
    }

    #[test]
    fn recovery_resolves_pending_journals() {
        let conn = memory_db();
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&b, b"moved").expect("write");

        // The rename already happened on disk: the intent becomes
        // undoable instead of vanishing.
        let applied_id = save_intent(
            &conn,
            "renamer",
            &[RenameRecord {
                from: a.clone(),
                to: b.clone(),
            }],
        )
        .expect("save");
        // Nothing was ever touched: the stale intent is dropped.
        save_intent(
            &conn,
            "renamer",
            &[RenameRecord {
                from: dir.path().join("x.txt"),
                to: dir.path().join("y.txt"),
            }],
        )
        .expect("save");

        let resolved = recover_pending(&conn).expect("recover");
        assert_eq!(resolved, 2);
        let list = list_journals(&conn, 10).expect("list");
        assert_eq!(list.len(), 1, "only the applied journal stays");
        assert_eq!(list[0].id, applied_id);
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

    #[test]
    fn undo_continues_after_a_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        let c = dir.path().join("c.txt");
        let d = dir.path().join("d.txt");
        File::create(&a).expect("create a");
        fs::rename(&a, &b).expect("rename a -> b");
        File::create(&c).expect("create c");
        fs::rename(&c, &d).expect("rename c -> d");

        // The b -> a record was already undone by hand; undo must still
        // restore d -> c instead of aborting on the missing file.
        fs::rename(&b, &a).expect("manual undo");
        let records = vec![
            RenameRecord {
                from: a.clone(),
                to: b.clone(),
            },
            RenameRecord {
                from: c.clone(),
                to: d.clone(),
            },
        ];
        let undone = undo_records(&records).expect("undo");
        assert_eq!(undone, 1);
        assert!(a.exists() && !b.exists());
        assert!(c.exists() && !d.exists());
    }

    #[test]
    fn undo_rejects_cross_folder_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let other = tempfile::tempdir().expect("other");
        let from = dir.path().join("b.txt");
        let to = other.path().join("a.txt");
        fs::write(&from, b"one").expect("write");
        fs::write(&to, b"two").expect("write");

        // A record moving a file across folders cannot come from the
        // renamer — undo must not perform it.
        let records = vec![RenameRecord {
            from: from.clone(),
            to: to.clone(),
        }];
        let undone = undo_records(&records).expect("undo");
        assert_eq!(undone, 0);
        assert!(from.exists() && to.exists());
    }

    #[test]
    fn delete_to_trash_skips_changed_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a.txt");
        fs::write(&a, b"current content").expect("write");

        // The scan recorded a different size: the file must be skipped,
        // not trashed.
        let results = delete_to_trash(&[(a.clone(), 999)]);
        assert_eq!(results.len(), 1);
        assert!(!results[0].1, "stale entries must not be trashed");
        assert!(a.exists());
    }

    // A crash between a case-only rename and its confirmation: name
    // resolution cannot tell "renamed" apart on case-insensitive file
    // systems ("case.jpg" still resolves), so recovery must look at the
    // real directory entries and keep the journal undoable.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn recovery_sees_a_case_only_rename_as_applied() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("case.jpg");
        fs::write(&src, b"x").expect("write");
        let target = dir.path().join("case.JPG");

        let conn = memory_db();
        let id = save_intent(
            &conn,
            "renamer",
            &[RenameRecord {
                from: src.clone(),
                to: target.clone(),
            }],
        )
        .expect("intent");

        // Then the rename happens and the process dies.
        fs::rename(&src, &target).expect("case rename");

        let resolved = recover_pending(&conn).expect("recover");
        assert_eq!(resolved, 1);

        // The journal survived as undoable, not dropped.
        let records = load_records(&conn, id).expect("read").expect("committed");
        assert_eq!(records.len(), 1);

        // And undo works: back to the original case.
        let undone = undo_records(&records).expect("undo");
        assert_eq!(undone, 1);
        let names: Vec<String> = fs::read_dir(dir.path())
            .expect("read")
            .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["case.jpg".to_string()]);
    }

    // The negative side: without the rename the intent is still dropped.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn recovery_drops_a_case_only_intent_that_never_applied() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("case.jpg");
        fs::write(&src, b"x").expect("write");
        let target = dir.path().join("case.JPG");

        let conn = memory_db();
        let id = save_intent(
            &conn,
            "renamer",
            &[RenameRecord {
                from: src.clone(),
                to: target.clone(),
            }],
        )
        .expect("intent");

        // No rename happened before the crash.
        let resolved = recover_pending(&conn).expect("recover");
        assert_eq!(resolved, 1);
        assert!(load_records(&conn, id).expect("read").is_none());
        assert!(src.exists(), "the file was never touched");
    }
}

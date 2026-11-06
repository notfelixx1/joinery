//! History page: the undo journal listing and its undo actions.

use iced::Task;

use joinery_tools::safety::undo;

use super::flatten;
use super::App;

#[derive(Debug, Clone)]
pub enum Msg {
    /// Undo one journal entry from the History page.
    Journal(i64),
    JournalReady {
        id: i64,
        total: usize,
        res: Result<usize, String>,
    },
}

/// Reload the undo journal listing for the History page.
pub(crate) fn reload(app: &mut App) {
    match app.db.as_ref() {
        Some(conn) => {
            app.history = undo::list_journals(conn, 50).unwrap_or_default();
            app.history_note = None;
        }
        None => app.history.clear(),
    }
}

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::Journal(id) => {
            let Some(conn) = app.db.as_ref() else {
                return Task::none();
            };
            let records = match undo::load_records(conn, id) {
                Ok(Some(records)) => records,
                Ok(None) => {
                    reload(app);
                    return Task::none();
                }
                Err(err) => {
                    app.history_note = Some(format!("undo journal read failed: {err}"));
                    return Task::none();
                }
            };
            let total = records.len();
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    undo::undo_records(&records).map_err(|e| e.to_string())
                }),
                move |res| Msg::JournalReady {
                    id,
                    total,
                    res: flatten(res),
                },
            )
        }
        Msg::JournalReady { id, total, res } => {
            match &res {
                Ok(undone) => {
                    if *undone == total {
                        if let Some(conn) = app.db.as_mut() {
                            let _ = undo::mark_undone(conn, id);
                        }
                    }
                }
                Err(err) => app.history_note = Some(err.clone()),
            }
            reload(app);
            Task::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use joinery_tools::safety::undo;

    use super::super::test_app;
    use super::*;

    fn entry() -> undo::JournalEntry {
        undo::JournalEntry {
            id: 1,
            tool: "renamer".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            op_count: 3,
            undone: false,
        }
    }

    #[test]
    fn reload_without_db_clears_the_listing() {
        let mut app = test_app();
        app.history = vec![entry()];

        reload(&mut app);

        assert!(app.history.is_empty());
    }

    #[test]
    fn journal_without_db_is_a_noop() {
        let mut app = test_app();
        app.history = vec![entry()];

        let _ = update(&mut app, Msg::Journal(1));

        // No storage — nothing breaks, nothing is "undone", the listing
        // stays as it is.
        assert!(app.history_note.is_none());
        assert_eq!(app.history.len(), 1);
    }
}

//! Batch Renamer: state, messages and the update loop.
//!
//! The safety flow mirrors `joinery_tools::core::renamer`: plan first,
//! write a durable undo intent before any file is touched, then apply.

use std::path::PathBuf;

use iced::Task;

use joinery_tools::core::renamer;
use joinery_tools::safety::undo;

use crate::i18n::{fmt, tr};

use super::App;
use super::{flatten, pick_folder_task, push_recent};

#[derive(Debug, Default)]
pub struct RenamerState {
    pub folder: Option<PathBuf>,
    pub find: String,
    pub replace: String,
    pub prefix: String,
    pub suffix: String,
    pub new_ext: String,
    pub pattern: String,
    pub include_subfolders: bool,
    pub allow_overwrite: bool,
    pub plan: Option<Vec<renamer::RenamePlanItem>>,
    pub ready_count: usize,
    pub has_conflicts: bool,
    pub busy: bool,
    pub outcome: Option<(usize, usize)>,
    pub journal: Option<i64>,
    pub note: Option<String>,
    pub error: Option<String>,
}

impl RenamerState {
    fn options(&self) -> renamer::RenameOptions {
        renamer::RenameOptions {
            find: self.find.clone(),
            replace: self.replace.clone(),
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            new_extension: self.new_ext.clone(),
            pattern: self.pattern.clone(),
            include_subfolders: self.include_subfolders,
        }
    }

    fn reset_result(&mut self) {
        self.plan = None;
        self.outcome = None;
        self.journal = None;
        self.note = None;
        self.error = None;
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    ChooseFolder,
    Folder(Option<String>),
    Find(String),
    Replace(String),
    Prefix(String),
    Suffix(String),
    NewExt(String),
    Pattern(String),
    Subfolders(bool),
    AllowOverwrite(bool),
    Preview,
    PlanReady(Result<Vec<renamer::RenamePlanItem>, String>),
    Apply,
    ApplyReady(Result<renamer::ApplyOutcome, String>),
    Undo,
    UndoReady {
        total: usize,
        res: Result<usize, String>,
    },
}

/// The records this apply step will journal: every Ready item and
/// every case-only rename. Journaling and applying must use exactly
/// the same filter, so nothing is renamed without an undo path.
fn apply_records(items: &[renamer::RenamePlanItem]) -> Vec<renamer::RenameRecord> {
    items
        .iter()
        .filter(|i| {
            matches!(
                i.status,
                renamer::RenameStatus::Ready | renamer::RenameStatus::CaseOnly
            )
        })
        .map(|i| renamer::RenameRecord {
            from: i.from.clone(),
            to: i.to.clone(),
        })
        .collect()
}

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::ChooseFolder => {
            app.ren.reset_result();
            pick_folder_task(Msg::Folder)
        }
        Msg::Folder(path) => {
            app.ren.folder = path.map(PathBuf::from);
            if let Some(p) = app.ren.folder.as_ref() {
                push_recent(&mut app.db, &mut app.ren_recent, "recent.renamer", p);
            }
            Task::none()
        }
        Msg::Find(v) => {
            app.ren.find = v;
            Task::none()
        }
        Msg::Replace(v) => {
            app.ren.replace = v;
            Task::none()
        }
        Msg::Prefix(v) => {
            app.ren.prefix = v;
            Task::none()
        }
        Msg::Suffix(v) => {
            app.ren.suffix = v;
            Task::none()
        }
        Msg::NewExt(v) => {
            app.ren.new_ext = v;
            Task::none()
        }
        Msg::Pattern(v) => {
            app.ren.pattern = v;
            Task::none()
        }
        Msg::Subfolders(v) => {
            app.ren.include_subfolders = v;
            Task::none()
        }
        Msg::AllowOverwrite(v) => {
            app.ren.allow_overwrite = v;
            Task::none()
        }
        Msg::Preview => {
            let Some(root) = app.ren.folder.clone() else {
                return Task::none();
            };
            let options = app.ren.options();
            app.ren.busy = true;
            app.ren.reset_result();
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    renamer::plan_rename(&root, &options).map_err(|e| e.to_string())
                }),
                |res| Msg::PlanReady(flatten(res)),
            )
        }
        Msg::PlanReady(res) => {
            app.ren.busy = false;
            match res {
                Ok(plan) => {
                    app.ren.ready_count = plan
                        .iter()
                        .filter(|i| {
                            matches!(
                                i.status,
                                renamer::RenameStatus::Ready | renamer::RenameStatus::CaseOnly
                            )
                        })
                        .count();
                    app.ren.has_conflicts = plan
                        .iter()
                        .any(|i| i.status == renamer::RenameStatus::BatchDuplicate);
                    app.ren.plan = Some(plan);
                }
                Err(err) => app.ren.error = Some(err),
            }
            Task::none()
        }
        Msg::Apply => {
            let Some(root) = app.ren.folder.clone() else {
                return Task::none();
            };
            let Some(items) = app.ren.plan.clone() else {
                return Task::none();
            };
            let allow = app.ren.allow_overwrite;

            // Durable intent BEFORE any file is touched: if the process
            // dies mid-apply, startup recovery settles the journal. A
            // journal write failure refuses the apply — files are never
            // renamed without an undo path.
            let records = apply_records(&items);
            match app.db.as_mut() {
                Some(conn) => match undo::save_intent(conn, "renamer", &records) {
                    Ok(id) => app.ren.journal = Some(id),
                    Err(err) => {
                        app.ren.error = Some(format!("cannot write the undo journal: {err}"));
                        return Task::none();
                    }
                },
                None => {
                    app.ren.error = Some("undo journal unavailable (no database)".to_string());
                    return Task::none();
                }
            }

            app.ren.busy = true;
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    renamer::apply_plan(&root, &items, allow).map_err(|e| e.to_string())
                }),
                |res| Msg::ApplyReady(flatten(res)),
            )
        }
        Msg::ApplyReady(res) => {
            app.ren.busy = false;
            match res {
                Ok(outcome) => {
                    if let (Some(conn), Some(id)) = (app.db.as_mut(), app.ren.journal) {
                        if outcome.applied.is_empty() {
                            // Nothing was touched — drop the intent.
                            let _ = undo::delete_journal(conn, id);
                            app.ren.journal = None;
                        } else {
                            let _ = undo::mark_committed(conn, id);
                        }
                    }
                    app.ren.outcome = Some((outcome.applied.len(), outcome.skipped.len()));
                    app.ren.plan = None;
                    app.ren.ready_count = 0;
                    app.ren.has_conflicts = false;
                }
                Err(err) => app.ren.error = Some(err),
            }
            Task::none()
        }
        Msg::Undo => {
            let Some(id) = app.ren.journal else {
                return Task::none();
            };
            let Some(conn) = app.db.as_ref() else {
                return Task::none();
            };
            let records = match undo::load_records(conn, id) {
                Ok(records) => records,
                Err(err) => {
                    app.ren.error = Some(format!("undo journal read failed: {err}"));
                    return Task::none();
                }
            };
            let Some(records) = records else {
                app.ren.journal = None;
                app.ren.note = Some("journal entry not found".to_string());
                return Task::none();
            };
            let total = records.len();
            app.ren.busy = true;
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    undo::undo_records(&records).map_err(|e| e.to_string())
                }),
                move |res| Msg::UndoReady {
                    total,
                    res: flatten(res),
                },
            )
        }
        Msg::UndoReady { total, res } => {
            app.ren.busy = false;
            if let Some(id) = app.ren.journal.take() {
                // A fully undone journal is closed; a partial one keeps
                // its records for a future attempt.
                if matches!(&res, Ok(undone) if *undone == total) {
                    if let Some(conn) = app.db.as_mut() {
                        let _ = undo::mark_undone(conn, id);
                    }
                }
            }
            match res {
                Ok(undone) => {
                    let template = tr(app.lang, "renamer.undoDone");
                    app.ren.note = Some(fmt(
                        template,
                        &[("n", undone.to_string()), ("total", total.to_string())],
                    ));
                }
                Err(err) => app.ren.error = Some(err),
            }
            Task::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use joinery_tools::core::renamer::{
        ApplyOutcome, RenamePlanItem, RenameRecord, RenameStatus, SkippedRename,
    };
    use joinery_tools::safety::undo;
    use joinery_tools::storage::database;

    use super::super::test_app;
    use super::*;

    fn item(status: RenameStatus) -> RenamePlanItem {
        RenamePlanItem {
            from: PathBuf::from("one.jpg"),
            to: PathBuf::from("two.jpg"),
            from_name: "one.jpg".to_string(),
            to_name: "two.jpg".to_string(),
            status,
            reason: None,
        }
    }

    fn record() -> RenameRecord {
        RenameRecord {
            from: PathBuf::from("one.jpg"),
            to: PathBuf::from("two.jpg"),
        }
    }

    fn db() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = database::open(&dir.path().join("test.db")).unwrap();
        (dir, conn)
    }

    #[test]
    fn plan_ready_counts_only_renameable_items() {
        let mut app = test_app();
        let plan = vec![
            item(RenameStatus::Ready),
            item(RenameStatus::Ready),
            item(RenameStatus::NoChange),
            item(RenameStatus::BatchDuplicate),
        ];

        let _ = update(&mut app, Msg::PlanReady(Ok(plan)));

        assert_eq!(app.ren.ready_count, 2);
        assert!(app.ren.has_conflicts);
        assert_eq!(app.ren.plan.as_ref().unwrap().len(), 4);
        assert!(!app.ren.busy);
    }

    #[test]
    fn case_only_counts_as_ready_and_reaches_the_journal() {
        let plan = vec![
            item(RenameStatus::Ready),
            item(RenameStatus::CaseOnly),
            item(RenameStatus::TargetExists),
            item(RenameStatus::NoChange),
        ];
        let items = plan.clone();
        let mut app = test_app();

        let _ = update(&mut app, Msg::PlanReady(Ok(plan)));

        // A case-only rename is applicable, so it counts as ready.
        assert_eq!(app.ren.ready_count, 2);
        assert!(!app.ren.has_conflicts);

        // And the undo journal covers it like any other rename.
        assert_eq!(apply_records(&items).len(), 2);
    }

    #[test]
    fn apply_without_database_refuses() {
        let mut app = test_app();
        app.ren.folder = Some(PathBuf::from("anything"));
        app.ren.plan = Some(vec![item(RenameStatus::Ready)]);
        app.ren.ready_count = 1;

        let _ = update(&mut app, Msg::Apply);

        // No undo journal — no rename. The preview must survive so the
        // user can still act on it.
        assert!(app.ren.error.as_deref().unwrap().contains("undo journal"));
        assert!(app.ren.plan.is_some());
        assert!(app.ren.journal.is_none());
        assert!(!app.ren.busy);
    }

    #[test]
    fn apply_ready_nothing_applied_drops_the_intent() {
        let mut app = test_app();
        let (_dir, conn) = db();
        let id = undo::save_intent(&conn, "renamer", &[record()]).unwrap();
        app.db = Some(conn);
        app.ren.journal = Some(id);

        let _ = update(
            &mut app,
            Msg::ApplyReady(Ok(ApplyOutcome {
                applied: Vec::new(),
                skipped: vec![SkippedRename {
                    from: PathBuf::from("a"),
                    to: PathBuf::from("b"),
                    reason: "missing".to_string(),
                }],
            })),
        );

        assert!(app.ren.journal.is_none());
        // Nothing was applied → the intent is gone, not undoable.
        let conn = app.db.as_ref().unwrap();
        assert!(undo::load_records(conn, id).unwrap().is_none());
    }

    #[test]
    fn apply_ready_marks_the_journal_committed() {
        let mut app = test_app();
        let (_dir, conn) = db();
        let id = undo::save_intent(&conn, "renamer", &[record()]).unwrap();
        app.db = Some(conn);
        app.ren.journal = Some(id);

        let _ = update(
            &mut app,
            Msg::ApplyReady(Ok(ApplyOutcome {
                applied: vec![record()],
                skipped: Vec::new(),
            })),
        );

        assert_eq!(app.ren.outcome, Some((1, 0)));
        let conn = app.db.as_ref().unwrap();
        // Now undoable: the committed records are readable again.
        let records = undo::load_records(conn, id).unwrap().unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn undo_full_closes_the_journal() {
        let mut app = test_app();
        let (_dir, conn) = db();
        let records = vec![record(), record()];
        let id = undo::save_intent(&conn, "renamer", &records).unwrap();
        undo::mark_committed(&conn, id).unwrap();
        app.db = Some(conn);
        app.ren.journal = Some(id);

        let _ = update(
            &mut app,
            Msg::UndoReady {
                total: 2,
                res: Ok(2),
            },
        );

        assert!(app.ren.journal.is_none()); // UI dropped its handle
        assert!(app.ren.note.as_deref().unwrap().contains("2"));
        let conn = app.db.as_ref().unwrap();
        assert!(undo::load_records(conn, id).unwrap().is_none()); // closed
    }

    #[test]
    fn undo_partial_keeps_the_records_for_another_attempt() {
        let mut app = test_app();
        let (_dir, conn) = db();
        let records = vec![record(), record()];
        let id = undo::save_intent(&conn, "renamer", &records).unwrap();
        undo::mark_committed(&conn, id).unwrap();
        app.db = Some(conn);
        app.ren.journal = Some(id);

        let _ = update(
            &mut app,
            Msg::UndoReady {
                total: 2,
                res: Ok(1),
            },
        );

        assert!(app.ren.note.as_deref().unwrap().contains("1"));
        let conn = app.db.as_ref().unwrap();
        let records = undo::load_records(conn, id).unwrap().unwrap();
        assert_eq!(records.len(), 2);
    }
}

//! Duplicate Finder: state, messages, update logic and the background
//! scan subscription.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use iced::futures::SinkExt;
use iced::{Subscription, Task};

use joinery_tools::core::duplicates;
use joinery_tools::safety::undo;

use crate::i18n::{fmt, tr};

use super::App;
use super::{flatten, pick_folder_task, push_recent};

#[derive(Debug, Default)]
pub struct DuplicatesState {
    pub folder: Option<PathBuf>,
    pub min_size: String,
    pub scanning: bool,
    pub scanned: u64,
    pub scan_id: u64,
    pub result: Option<duplicates::ScanResult>,
    pub selected: HashMap<String, bool>,
    pub note: Option<String>,
    pub error: Option<String>,
}

impl DuplicatesState {
    pub fn min_size_parsed(&self) -> u64 {
        self.min_size.trim().parse().unwrap_or(1)
    }

    fn start_scan(&mut self) -> Task<Msg> {
        if self.folder.is_none() {
            return Task::none();
        }
        self.scanning = true;
        self.scan_id += 1;
        self.scanned = 0;
        self.result = None;
        self.selected.clear();
        self.error = None;
        Task::none()
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    ChooseFolder,
    Folder(Option<String>),
    MinSize(String),
    Scan,
    Progress(u64),
    Done(Result<duplicates::ScanResult, String>),
    Toggle(String, bool),
    Delete,
    DeleteReady(Result<Vec<(PathBuf, bool, Option<String>)>, String>),
}

/// The files the user marked for deletion, in the order they are
/// displayed. Only checked files qualify and one file is never
/// touched twice, even if a malformed scan lists it twice.
fn delete_targets(
    result: &duplicates::ScanResult,
    selected: &HashMap<String, bool>,
) -> Vec<(PathBuf, u64)> {
    let mut targets: Vec<(PathBuf, u64)> = result
        .groups
        .iter()
        .flat_map(|g| g.files.iter())
        .filter(|f| {
            selected
                .get(&f.path.to_string_lossy().to_string())
                .copied()
                .unwrap_or(false)
        })
        .map(|f| (f.path.clone(), f.size))
        .collect();
    let mut seen: HashSet<String> = HashSet::new();
    targets.retain(|(path, _)| seen.insert(path.to_string_lossy().to_string()));
    targets
}

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::ChooseFolder => {
            app.dup.result = None;
            app.dup.note = None;
            app.dup.error = None;
            pick_folder_task(Msg::Folder)
        }
        Msg::Folder(path) => {
            app.dup.folder = path.map(PathBuf::from);
            if let Some(p) = app.dup.folder.as_ref() {
                push_recent(&mut app.db, &mut app.dup_recent, "recent.duplicates", p);
            }
            Task::none()
        }
        Msg::MinSize(v) => {
            app.dup.min_size = v;
            Task::none()
        }
        Msg::Scan => app.dup.start_scan(),
        Msg::Progress(n) => {
            app.dup.scanned = n;
            Task::none()
        }
        Msg::Done(res) => {
            app.dup.scanning = false;
            match res {
                Ok(result) => {
                    let mut selected = HashMap::new();
                    for group in &result.groups {
                        for (index, file) in group.files.iter().enumerate() {
                            // keep the first copy of every group by default
                            selected.insert(file.path.to_string_lossy().to_string(), index > 0);
                        }
                    }
                    app.dup.selected = selected;
                    app.dup.result = Some(result);
                }
                Err(err) => app.dup.error = Some(err),
            }
            Task::none()
        }
        Msg::Toggle(path, value) => {
            app.dup.selected.insert(path, value);
            Task::none()
        }
        Msg::Delete => {
            let targets = {
                let Some(result) = &app.dup.result else {
                    return Task::none();
                };
                delete_targets(result, &app.dup.selected)
            };
            if targets.is_empty() {
                return Task::none();
            }
            let count = targets.len();
            let confirm_text = fmt(tr(app.lang, "dup.confirm"), &[("n", count.to_string())]);
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    let confirmed = rfd::MessageDialog::new()
                        .set_title("Joinery Tools")
                        .set_description(&confirm_text)
                        .set_buttons(rfd::MessageButtons::OkCancel)
                        .show();
                    if matches!(
                        confirmed,
                        rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes
                    ) {
                        Ok(undo::delete_to_trash(&targets))
                    } else {
                        Err("cancelled".to_string())
                    }
                }),
                |res| Msg::DeleteReady(flatten(res)),
            )
        }
        Msg::DeleteReady(res) => match res {
            Ok(results) => {
                let deleted = results.iter().filter(|(_, ok, _)| *ok).count();
                let failed = results.len() - deleted;
                let template = tr(app.lang, "dup.deleted");
                app.dup.note = Some(fmt(
                    template,
                    &[
                        ("deleted", deleted.to_string()),
                        ("failed", failed.to_string()),
                    ],
                ));
                app.dup.start_scan()
            }
            Err(err) if err == "cancelled" => Task::none(),
            Err(err) => {
                app.dup.error = Some(err);
                Task::none()
            }
        },
    }
}

/// Background duplicate scan: runs while `dup.scanning` is set and is
/// keyed by `scan_id`, so starting a new scan cancels the old one.
pub fn scan_subscription(app: &App) -> Subscription<Msg> {
    if !app.dup.scanning {
        return Subscription::none();
    }
    let scan_id = app.dup.scan_id;
    let root = app.dup.folder.clone().unwrap_or_default();
    let min_size = app.dup.min_size_parsed();

    Subscription::run_with_id(
        ("dup-scan", scan_id),
        iced::stream::channel::<Msg, _>(100, move |mut out| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u64>();
            let handle = tokio::task::spawn_blocking(move || {
                duplicates::find_duplicates(&root, min_size, &mut |n| {
                    let _ = tx.send(n);
                })
            });
            while let Some(n) = rx.recv().await {
                let _ = out.send(Msg::Progress(n)).await;
            }
            let result = handle
                .await
                .unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())));
            let _ = out.send(Msg::Done(result.map_err(|e| e.to_string()))).await;
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::super::test_app;
    use super::*;
    use joinery_tools::core::duplicates::{DuplicateFile, DuplicateGroup};

    fn file(path: &str, size: u64) -> DuplicateFile {
        DuplicateFile {
            path: PathBuf::from(path),
            size,
        }
    }

    fn result() -> duplicates::ScanResult {
        duplicates::ScanResult {
            groups: vec![
                DuplicateGroup {
                    hash: "h1".to_string(),
                    size: 12,
                    files: vec![file("keep.png", 12), file("copy.png", 12)],
                },
                DuplicateGroup {
                    hash: "h2".to_string(),
                    size: 4,
                    files: vec![file("two.dat", 4), file("two copy.dat", 4)],
                },
            ],
            stats: duplicates::ScanStats {
                scanned_files: 4,
                skipped_files: 0,
                duplicate_groups: 2,
                wasted_bytes: 16,
            },
        }
    }

    #[test]
    fn done_selects_every_copy_except_the_first() {
        let mut app = test_app();
        app.dup.folder = Some(PathBuf::from("x"));

        let _ = update(&mut app, Msg::Done(Ok(result())));

        assert!(!app.dup.scanning);
        let selected = &app.dup.selected;
        assert_eq!(selected.get("keep.png"), Some(&false));
        assert_eq!(selected.get("copy.png"), Some(&true));
        assert_eq!(selected.get("two.dat"), Some(&false));
        assert_eq!(selected.get("two copy.dat"), Some(&true));
    }

    #[test]
    fn start_scan_guards_and_resets() {
        let mut app = test_app();
        let _ = app.dup.start_scan();
        assert!(!app.dup.scanning); // no folder — no scan

        app.dup.scan_id = 7;
        app.dup.folder = Some(PathBuf::from("x"));
        app.dup.result = Some(result());
        app.dup.selected.insert("keep.png".into(), true);
        app.dup.error = Some("old".into());

        let _ = app.dup.start_scan();
        assert!(app.dup.scanning);
        assert_eq!(app.dup.scan_id, 8);
        assert_eq!(app.dup.scanned, 0);
        assert!(app.dup.result.is_none());
        assert!(app.dup.selected.is_empty());
        assert!(app.dup.error.is_none());
    }

    #[test]
    fn delete_message_without_selection_refuses() {
        let mut app = test_app();
        app.dup.folder = Some(PathBuf::from("x"));
        app.dup.result = Some(result());

        let _ = update(&mut app, Msg::Delete);

        // Nothing marked for deletion → the confirmation dialog never
        // starts and nothing looks busy.
        assert!(app.dup.error.is_none());
        assert!(!app.dup.scanning);
        assert!(app.dup.result.is_some());
    }

    #[test]
    fn delete_marks_only_the_checked_files() {
        let r = result();
        let mut selected = HashMap::new();
        selected.insert("copy.png".to_string(), true);
        selected.insert("two copy.dat".to_string(), true);
        selected.insert("keep.png".to_string(), false); // explicitly kept

        let targets = delete_targets(&r, &selected);

        assert_eq!(
            targets,
            vec![
                (PathBuf::from("copy.png"), 12),
                (PathBuf::from("two copy.dat"), 4),
            ]
        );
    }

    #[test]
    fn delete_never_touches_one_file_twice() {
        let mut r = result();
        // Same path as in the first group — the trash step must never
        // touch one file twice.
        r.groups[1].files.push(file("copy.png", 12));

        let mut selected = HashMap::new();
        selected.insert("copy.png".to_string(), true);

        let targets = delete_targets(&r, &selected);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, PathBuf::from("copy.png"));
    }
}

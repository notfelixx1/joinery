//! Checksum tool: state, messages and the update loop.

use std::path::PathBuf;

use iced::futures::SinkExt;
use iced::{Subscription, Task};

use joinery_tools::core::checksum as core_checksum;

use super::App;

#[derive(Debug, Clone)]
pub struct ChecksumItem {
    pub path: PathBuf,
    pub hash: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct ChecksumState {
    pub paths: Vec<PathBuf>,
    pub algorithm: core_checksum::Algorithm,
    pub busy: bool,
    /// Bumped on every Run — keys the streaming subscription, so a new
    /// run cancels the previous one.
    pub scan_id: u64,
    pub results: Option<Vec<ChecksumItem>>,
    pub expected: String,
    /// Hash of the last copied result — drives the transient
    /// "Copied" label; cleared by `Msg::CopyClear` after a moment.
    pub copied: Option<String>,
    pub error: Option<String>,
}

impl Default for ChecksumState {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            algorithm: core_checksum::Algorithm::Sha256,
            busy: false,
            scan_id: 0,
            results: None,
            expected: String::new(),
            copied: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    ChooseFiles,
    Files(Vec<String>),
    Algorithm(core_checksum::Algorithm),
    Run,
    /// `done` of `total` files hashed — drives the top-bar progress.
    Progress(usize, usize),
    Done(Vec<ChecksumItem>),
    /// Toggle the auto-copy-to-clipboard setting (single-file results).
    Autocopy(bool),
    Expected(String),
    Copy(String),
    CopyClear,
}

/// Native multi-file picker running off the main thread.
fn pick_files_task() -> Task<Msg> {
    Task::perform(
        tokio::task::spawn_blocking(move || {
            rfd::FileDialog::new()
                .pick_files()
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<String>>()
        }),
        |res| Msg::Files(res.unwrap_or_default()),
    )
}

/// Streaming hash job: mirrors the duplicates scan pattern — one
/// message per finished file drives the top-bar progress, the final
/// message delivers every result.
pub fn run_subscription(app: &App) -> Subscription<Msg> {
    if !app.cks.busy {
        return Subscription::none();
    }
    let scan_id = app.cks.scan_id;
    let paths = app.cks.paths.clone();
    let algorithm = app.cks.algorithm;

    Subscription::run_with_id(
        ("cks-hash", scan_id),
        iced::stream::channel::<Msg, _>(64, move |mut out| async move {
            let total = paths.len();
            let mut items = Vec::with_capacity(total);
            for (done, path) in paths.iter().enumerate() {
                let item = match core_checksum::hash_file(path, algorithm) {
                    Ok(hash) => ChecksumItem {
                        path: path.clone(),
                        hash: Some(hash),
                        error: None,
                    },
                    Err(err) => ChecksumItem {
                        path: path.clone(),
                        hash: None,
                        error: Some(err.to_string()),
                    },
                };
                items.push(item);
                let _ = out.send(Msg::Progress(done + 1, total)).await;
            }
            let _ = out.send(Msg::Done(items)).await;
        }),
    )
}

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::ChooseFiles => {
            app.cks.results = None;
            app.cks.error = None;
            pick_files_task()
        }
        Msg::Files(paths) => {
            app.cks.paths = paths.into_iter().map(PathBuf::from).collect();
            Task::none()
        }
        Msg::Algorithm(algorithm) => {
            app.cks.algorithm = algorithm;
            Task::none()
        }
        Msg::Run => {
            if app.cks.paths.is_empty() {
                return Task::none();
            }
            app.cks.busy = true;
            app.cks.scan_id += 1;
            app.cks.error = None;
            app.progress = Some(0.0);
            Task::none()
        }
        Msg::Progress(done, total) => {
            if total > 0 {
                app.progress = Some(done as f32 / total as f32);
            }
            Task::none()
        }
        Msg::Done(items) => {
            app.cks.busy = false;
            app.progress = None;
            // Auto-copy: one file — its hash goes straight to the
            // clipboard and reuses the transient "Copied" label.
            let auto = app.cks_autocopy
                && items.len() == 1
                && items.first().and_then(|i| i.hash.clone()).is_some();
            app.cks.results = Some(items);
            if auto {
                if let Some(hash) = app
                    .cks
                    .results
                    .as_ref()
                    .and_then(|r| r.first())
                    .and_then(|i| i.hash.clone())
                {
                    app.cks.copied = Some(hash.clone());
                    return iced::clipboard::write::<Msg>(hash).chain(Task::perform(
                        tokio::time::sleep(std::time::Duration::from_millis(1500)),
                        |_| Msg::CopyClear,
                    ));
                }
            }
            Task::none()
        }
        Msg::Autocopy(v) => {
            app.cks_autocopy = v;
            super::persist_settings(&app.db, "cks_autocopy", if v { "1" } else { "0" });
            Task::none()
        }
        Msg::Expected(v) => {
            app.cks.expected = v;
            Task::none()
        }
        Msg::Copy(hash) => {
            app.cks.copied = Some(hash.clone());
            Task::batch([
                iced::clipboard::write::<Msg>(hash),
                Task::perform(
                    tokio::time::sleep(std::time::Duration::from_millis(1500)),
                    |_| Msg::CopyClear,
                ),
            ])
        }
        Msg::CopyClear => {
            app.cks.copied = None;
            Task::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_app;
    use super::*;

    #[test]
    fn files_message_converts_strings_to_paths() {
        let mut app = test_app();
        app.cks.results = Some(Vec::new());

        let _ = update(
            &mut app,
            Msg::Files(vec!["a.txt".to_string(), "b.bin".to_string()]),
        );

        assert_eq!(app.cks.paths[1], PathBuf::from("b.bin"));
        // Picking new files does not wipe the old results by itself —
        // that is ChooseFiles' job before the dialog even opens.
        assert!(app.cks.results.is_some());
    }
}

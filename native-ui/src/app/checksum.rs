//! Checksum tool: state, messages and the update loop.

use std::path::PathBuf;

use iced::Task;

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
    pub results: Option<Vec<ChecksumItem>>,
    pub expected: String,
    pub error: Option<String>,
}

impl Default for ChecksumState {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            algorithm: core_checksum::Algorithm::Sha256,
            busy: false,
            results: None,
            expected: String::new(),
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
    Done(Vec<ChecksumItem>),
    Expected(String),
    Copy(String),
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
            let paths = app.cks.paths.clone();
            if paths.is_empty() {
                return Task::none();
            }
            let algorithm = app.cks.algorithm;
            app.cks.busy = true;
            app.cks.error = None;
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    paths
                        .into_iter()
                        .map(|path| match core_checksum::hash_file(&path, algorithm) {
                            Ok(hash) => ChecksumItem {
                                path,
                                hash: Some(hash),
                                error: None,
                            },
                            Err(err) => ChecksumItem {
                                path,
                                hash: None,
                                error: Some(err.to_string()),
                            },
                        })
                        .collect::<Vec<_>>()
                }),
                |res| Msg::Done(res.unwrap_or_default()),
            )
        }
        Msg::Done(items) => {
            app.cks.busy = false;
            app.cks.results = Some(items);
            Task::none()
        }
        Msg::Expected(v) => {
            app.cks.expected = v;
            Task::none()
        }
        Msg::Copy(hash) => iced::clipboard::write::<Msg>(hash),
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

//! Folder Analyzer: state, messages and the update loop.

use std::path::PathBuf;

use iced::Task;

use joinery_tools::core::analyzer as core_analyzer;

use super::App;
use super::{flatten, pick_folder_task, push_recent};

#[derive(Debug, Default)]
pub struct AnalyzerState {
    pub folder: Option<PathBuf>,
    pub busy: bool,
    pub analysis: Option<core_analyzer::FolderAnalysis>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ChooseFolder,
    Folder(Option<String>),
    Run,
    Done(Result<core_analyzer::FolderAnalysis, String>),
}

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::ChooseFolder => {
            app.ana.analysis = None;
            app.ana.error = None;
            pick_folder_task(Msg::Folder)
        }
        Msg::Folder(path) => {
            app.ana.folder = path.map(PathBuf::from);
            if let Some(p) = app.ana.folder.as_ref() {
                push_recent(&mut app.db, &mut app.ana_recent, "recent.analyzer", p);
            }
            Task::none()
        }
        Msg::Run => {
            let Some(root) = app.ana.folder.clone() else {
                return Task::none();
            };
            app.ana.busy = true;
            app.ana.error = None;
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    core_analyzer::analyze_folder(&root).map_err(|e| e.to_string())
                }),
                |res| Msg::Done(flatten(res)),
            )
        }
        Msg::Done(res) => {
            app.ana.busy = false;
            match res {
                Ok(analysis) => app.ana.analysis = Some(analysis),
                Err(err) => app.ana.error = Some(err),
            }
            Task::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_app;
    use super::*;

    #[test]
    fn folder_message_sets_the_folder_but_no_recents_without_db() {
        let mut app = test_app();

        let _ = update(&mut app, Msg::Folder(Some("C:/tmp".to_string())));

        assert_eq!(
            app.ana
                .folder
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            Some("C:/tmp".to_string())
        );
        // No storage — no recents, and no panic either.
        assert!(app.ana_recent.is_empty());
    }
}

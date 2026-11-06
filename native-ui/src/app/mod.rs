//! Application state, messages and the update loop.
//!
//! Everything heavy delegates to `joinery_tools::core` / `::safety` —
//! exactly the same functions the Tauri commands call. No IPC here:
//! the native frontend talks to the core in-process.
//!
//! Global state (navigation, theme, language, storage) and the dispatch
//! live here; every tool owns its slice of the app in a sibling module
//! (`<tool>::State`, `<tool>::Msg`, `<tool>::update`).

pub mod analyzer;
pub mod checksum;
pub mod diff;
pub mod duplicates;
pub mod history;
pub mod jsonyaml;
pub mod renamer;

use std::path::PathBuf;

use iced::{Subscription, Task};
use rusqlite::Connection;

use joinery_tools::safety::undo;
use joinery_tools::storage::database;

use crate::i18n::Lang;

use analyzer::AnalyzerState;
use checksum::ChecksumState;
use diff::DiffState;
use duplicates::DuplicatesState;
use jsonyaml::JsonYamlState;
use renamer::RenamerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Files,
    Developer,
    History,
    System,
    Support,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTool {
    Renamer,
    Duplicates,
    Analyzer,
    Checksum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevTool {
    JsonYaml,
    Diff,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Nav(Page),
    FileTool(FileTool),
    DevTool(DevTool),
    ToggleTheme,
    ToggleLang,
    OpenUrl(&'static str),

    Renamer(renamer::Msg),
    Duplicates(duplicates::Msg),
    Analyzer(analyzer::Msg),
    Checksum(checksum::Msg),
    JsonYaml(jsonyaml::Msg),
    Diff(diff::Msg),
    History(history::Msg),
}

pub struct App {
    pub page: Page,
    pub file_tool: FileTool,
    pub dev_tool: DevTool,
    pub dark: bool,
    pub lang: Lang,
    pub db: Option<Connection>,

    pub history: Vec<undo::JournalEntry>,
    pub history_note: Option<String>,
    pub ren_recent: Vec<PathBuf>,
    pub dup_recent: Vec<PathBuf>,
    pub ana_recent: Vec<PathBuf>,

    pub ren: RenamerState,
    pub dup: DuplicatesState,
    pub ana: AnalyzerState,
    pub cks: ChecksumState,
    pub jy: JsonYamlState,
    pub diff: DiffState,
}

impl App {
    pub fn new() -> (App, Task<Msg>) {
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dev.joinery.tools")
            .join("joinery.db");
        let db = database::open(&db_path).ok();

        // Settle journals left pending by a crash between the intent
        // and the confirmation — bookkeeping only, no file operations.
        if let Some(conn) = &db {
            let _ = undo::recover_pending(conn);
        }

        let mut dark = false;
        let mut lang = Lang::En;
        let (ren_recent, dup_recent, ana_recent) = match &db {
            Some(conn) => (
                database::recent_list(conn, "recent.renamer"),
                database::recent_list(conn, "recent.duplicates"),
                database::recent_list(conn, "recent.analyzer"),
            ),
            None => (Vec::new(), Vec::new(), Vec::new()),
        };
        if let Some(conn) = &db {
            // Settings are cosmetic — fall back to the defaults when
            // the storage errors out.
            let theme = database::get_setting(conn, "theme").unwrap_or(None);
            dark = theme.as_deref() == Some("dark");
            if let Some(v) = database::get_setting(conn, "lang").unwrap_or(None) {
                lang = Lang::from_str(&v);
            }
        }

        (
            App {
                page: Page::Home,
                file_tool: FileTool::Renamer,
                dev_tool: DevTool::JsonYaml,
                dark,
                lang,
                db,
                history: Vec::new(),
                history_note: None,
                ren_recent,
                dup_recent,
                ana_recent,
                ren: RenamerState::default(),
                dup: DuplicatesState {
                    min_size: "1".to_string(),
                    ..DuplicatesState::default()
                },
                ana: AnalyzerState::default(),
                cks: ChecksumState::default(),
                jy: JsonYamlState::default(),
                diff: DiffState::default(),
            },
            Task::none(),
        )
    }

    pub fn iced_theme(&self) -> iced::Theme {
        iced::Theme::custom("Joinery".to_string(), crate::palette::palette(self.dark))
    }
}

pub fn subscription(app: &App) -> Subscription<Msg> {
    duplicates::scan_subscription(app).map(Msg::Duplicates)
}

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::Nav(page) => {
            app.page = page;
            if page == Page::History {
                history::reload(app);
            }
            Task::none()
        }
        Msg::FileTool(tool) => {
            app.file_tool = tool;
            Task::none()
        }
        Msg::DevTool(tool) => {
            app.dev_tool = tool;
            Task::none()
        }
        Msg::ToggleTheme => {
            app.dark = !app.dark;
            persist_settings(&app.db, "theme", if app.dark { "dark" } else { "light" });
            Task::none()
        }
        Msg::ToggleLang => {
            app.lang = app.lang.toggle();
            persist_settings(
                &app.db,
                "lang",
                match app.lang {
                    Lang::En => "en",
                    Lang::Ru => "ru",
                },
            );
            Task::none()
        }
        Msg::OpenUrl(url) => {
            let _ = open::that(url);
            Task::none()
        }
        Msg::Renamer(msg) => renamer::update(app, msg).map(Msg::Renamer),
        Msg::Duplicates(msg) => duplicates::update(app, msg).map(Msg::Duplicates),
        Msg::Analyzer(msg) => analyzer::update(app, msg).map(Msg::Analyzer),
        Msg::Checksum(msg) => checksum::update(app, msg).map(Msg::Checksum),
        Msg::JsonYaml(msg) => jsonyaml::update(app, msg).map(Msg::JsonYaml),
        Msg::Diff(msg) => diff::update(app, msg).map(Msg::Diff),
        Msg::History(msg) => history::update(app, msg).map(Msg::History),
    }
}

// ---------- shared helpers ----------

/// Flatten a `spawn_blocking` join result around a core `Result<_, String>`.
pub(crate) fn flatten<T>(
    res: Result<Result<T, String>, tokio::task::JoinError>,
) -> Result<T, String> {
    match res {
        Ok(inner) => inner,
        Err(err) => Err(format!("internal error: {err}")),
    }
}

/// Native folder picker running off the main thread.
pub(crate) fn pick_folder_task<M: Send + 'static>(map: fn(Option<String>) -> M) -> Task<M> {
    Task::perform(
        tokio::task::spawn_blocking(move || {
            rfd::FileDialog::new()
                .pick_folder()
                .map(|p| p.to_string_lossy().to_string())
        }),
        move |res| map(res.unwrap_or(None)),
    )
}

/// Settings are cosmetic — storage errors are intentionally swallowed.
pub(crate) fn persist_settings(db: &Option<Connection>, key: &str, value: &str) {
    if let Some(conn) = db {
        let _ = database::set_setting(conn, key, value);
    }
}

/// Remember a picked folder in the per-tool recent list.
pub(crate) fn push_recent(
    db: &mut Option<Connection>,
    field: &mut Vec<PathBuf>,
    key: &str,
    path: &std::path::Path,
) {
    if let Some(conn) = db.as_mut() {
        let _ = database::recent_push(conn, key, path, 5);
        *field = database::recent_list(conn, key);
    }
}

// ---------- tests ----------

/// A plain `App` for update-loop tests: no storage, no dialogs, no
/// rendering. Tests that need storage replace `app.db` themselves.
#[cfg(test)]
pub(crate) fn test_app() -> App {
    App {
        page: Page::Home,
        file_tool: FileTool::Renamer,
        dev_tool: DevTool::JsonYaml,
        dark: false,
        lang: Lang::En,
        db: None,
        history: Vec::new(),
        history_note: None,
        ren_recent: Vec::new(),
        dup_recent: Vec::new(),
        ana_recent: Vec::new(),
        ren: RenamerState::default(),
        dup: DuplicatesState::default(),
        ana: AnalyzerState::default(),
        cks: ChecksumState::default(),
        jy: JsonYamlState::default(),
        diff: DiffState::default(),
    }
}

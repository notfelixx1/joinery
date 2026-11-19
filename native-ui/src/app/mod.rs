//! Application state, messages and the update loop.
//!
//! Everything heavy delegates to `joinery_tools::core` / `::safety` —
//! the native frontend talks to the core in-process: no IPC, no
//! bundled browser, no network.
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
    /// Expand/collapse a sidebar section; opening also navigates there.
    ToggleNav(Page),
    /// One animation frame of the section slide-out reveal.
    AnimTick,
    FileTool(FileTool),
    DevTool(DevTool),
    ToggleTheme,
    SetLang(Lang),
    OpenUrl(&'static str),

    /// Window close interception: geometry is saved, then the window
    /// actually closes.
    CloseRequested(iced::window::Id),
    CloseMax(iced::window::Id, bool),
    ClosePos(iced::window::Id, Option<iced::Point>),
    CloseSize(iced::window::Id, iced::Size),

    Renamer(renamer::Msg),
    Duplicates(duplicates::Msg),
    Analyzer(analyzer::Msg),
    Checksum(checksum::Msg),
    JsonYaml(jsonyaml::Msg),
    Diff(diff::Msg),
    History(history::Msg),
}

/// The slide-out animation of a sidebar section: `t` runs 0.0 → 1.0
/// while the sub-items of `page` move in (`opening`) or out.
#[derive(Debug, PartialEq)]
pub struct NavReveal {
    pub page: Page,
    pub opening: bool,
    pub t: f32,
}

pub struct App {
    pub page: Page,
    pub file_tool: FileTool,
    pub dev_tool: DevTool,
    pub dark: bool,
    pub lang: Lang,
    pub db: Option<Connection>,
    /// The currently expanded sidebar section, if any.
    pub nav_open: Option<Page>,
    /// The section reveal animation in flight, if any.
    pub nav_reveal: Option<NavReveal>,
    /// Top-bar progress fraction while a long operation runs.
    pub progress: Option<f32>,
    /// Auto-copy the checksum hash to the clipboard (saved setting).
    pub cks_autocopy: bool,
    /// Window position captured on the close request, handed to the
    /// size step before both are persisted.
    pub close_pos: Option<iced::Point>,

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

/// Command-line options: `joinery --renamer --folder "D:\photos" --dark`.
#[derive(Debug, Default)]
pub struct Options {
    pub page: Option<Page>,
    pub file_tool: Option<FileTool>,
    pub dev_tool: Option<DevTool>,
    pub dark: Option<bool>,
    pub lang: Option<Lang>,
    pub folder: Option<PathBuf>,
}

const HELP: &str = "\
Joinery Tools — one app. Everyday tools. No nonsense.

Usage:
  joinery [OPTIONS]

Open directly on a tool:
  --renamer       Batch Renamer
  --duplicates    Duplicate Finder
  --analyzer      Folder Analyzer
  --checksum      Checksum
  --jsonyaml      JSON / YAML Validator
  --diff          Text Diff
  --history       History
  --system        System
  --support       Support

Options:
  --dark | --light    Theme (overrides the saved one)
  --lang en|ru|uk|es|de|fr|pt|it    Interface language
  --folder <path>     Preselect a folder for the file tool
  --help              Show this text
";

impl Options {
    /// Parse the command line. Unknown arguments are ignored, so the
    /// exe stays launchable from any shortcut.
    pub fn from_env() -> Self {
        let mut opts = Self::default();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--renamer" => opts.file_tool = Some(FileTool::Renamer),
                "--duplicates" => opts.file_tool = Some(FileTool::Duplicates),
                "--analyzer" => opts.file_tool = Some(FileTool::Analyzer),
                "--checksum" => opts.file_tool = Some(FileTool::Checksum),
                "--jsonyaml" => opts.dev_tool = Some(DevTool::JsonYaml),
                "--diff" => opts.dev_tool = Some(DevTool::Diff),
                "--history" => opts.page = Some(Page::History),
                "--home" => opts.page = Some(Page::Home),
                "--system" => opts.page = Some(Page::System),
                "--support" => opts.page = Some(Page::Support),
                "--dark" => opts.dark = Some(true),
                "--light" => opts.dark = Some(false),
                "--lang" => {
                    if let Some(v) = args.next() {
                        opts.lang = Some(Lang::from_str(&v));
                    }
                }
                "--folder" => {
                    if let Some(v) = args.next() {
                        opts.folder = Some(PathBuf::from(v));
                    }
                }
                "--help" | "-h" => {
                    println!("{HELP}");
                    std::process::exit(0);
                }
                _ => {}
            }
        }
        // A tool flag implies its page unless one was named explicitly.
        if opts.page.is_none() {
            if opts.file_tool.is_some() {
                opts.page = Some(Page::Files);
            } else if opts.dev_tool.is_some() {
                opts.page = Some(Page::Developer);
            }
        }
        opts
    }
}

/// Where the app database lives — shared with main for the early
/// window-geometry read.
pub fn settings_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dev.joinery.tools")
        .join("joinery.db")
}

/// Stable text codes for the persisted session settings.
pub(crate) fn page_code(page: Page) -> &'static str {
    match page {
        Page::Home => "home",
        Page::Files => "files",
        Page::Developer => "developer",
        Page::History => "history",
        Page::System => "system",
        Page::Support => "support",
    }
}

pub(crate) fn page_from_code(v: &str) -> Option<Page> {
    match v {
        "home" => Some(Page::Home),
        "files" => Some(Page::Files),
        "developer" => Some(Page::Developer),
        "history" => Some(Page::History),
        "system" => Some(Page::System),
        "support" => Some(Page::Support),
        _ => None,
    }
}

pub(crate) fn file_tool_code(tool: FileTool) -> &'static str {
    match tool {
        FileTool::Renamer => "renamer",
        FileTool::Duplicates => "duplicates",
        FileTool::Analyzer => "analyzer",
        FileTool::Checksum => "checksum",
    }
}

pub(crate) fn file_tool_from_code(v: &str) -> Option<FileTool> {
    match v {
        "renamer" => Some(FileTool::Renamer),
        "duplicates" => Some(FileTool::Duplicates),
        "analyzer" => Some(FileTool::Analyzer),
        "checksum" => Some(FileTool::Checksum),
        _ => None,
    }
}

pub(crate) fn dev_tool_code(tool: DevTool) -> &'static str {
    match tool {
        DevTool::JsonYaml => "jsonyaml",
        DevTool::Diff => "diff",
    }
}

pub(crate) fn dev_tool_from_code(v: &str) -> Option<DevTool> {
    match v {
        "jsonyaml" => Some(DevTool::JsonYaml),
        "diff" => Some(DevTool::Diff),
        _ => None,
    }
}

/// Indeterminate shimmer for the top-bar progress, driven by the
/// per-file scan counter.
pub(crate) fn scan_pulse(n: u64) -> f32 {
    0.15 + 0.6 * ((n % 24) as f32 / 24.0)
}

impl App {
    pub fn new(options: Options) -> (App, Task<Msg>) {
        let db = database::open(&settings_db_path()).ok();

        // Settle journals left pending by a crash between the intent
        // and the confirmation — bookkeeping only, no file operations.
        if let Some(conn) = &db {
            let _ = undo::recover_pending(conn);
        }

        let mut dark = false;
        let mut lang = Lang::En;
        let mut saved_page: Option<Page> = None;
        let mut saved_file_tool: Option<FileTool> = None;
        let mut saved_dev_tool: Option<DevTool> = None;
        let mut saved_nav_open: Option<Page> = None;
        let mut cks_autocopy = true;
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
            saved_page = database::get_setting(conn, "page")
                .unwrap_or(None)
                .and_then(|v| page_from_code(&v));
            saved_file_tool = database::get_setting(conn, "file_tool")
                .unwrap_or(None)
                .and_then(|v| file_tool_from_code(&v));
            saved_dev_tool = database::get_setting(conn, "dev_tool")
                .unwrap_or(None)
                .and_then(|v| dev_tool_from_code(&v));
            saved_nav_open = database::get_setting(conn, "nav_open")
                .unwrap_or(None)
                .and_then(|v| page_from_code(&v))
                .filter(|p| matches!(p, Page::Files | Page::Developer));
            cks_autocopy = database::get_setting(conn, "cks_autocopy")
                .unwrap_or(None)
                .map(|v| v != "0")
                .unwrap_or(true);
        }

        // Command-line overrides win over the saved settings.
        dark = options.dark.unwrap_or(dark);
        lang = options.lang.unwrap_or(lang);

        // "Open where you left off": the last session comes back
        // unless the command line points somewhere specific.
        let page = options.page.unwrap_or(saved_page.unwrap_or(Page::Home));
        let nav_open = match options.page {
            Some(p @ (Page::Files | Page::Developer)) => Some(p),
            Some(_) => saved_nav_open,
            None => saved_nav_open.or(match page {
                Page::Files | Page::Developer => Some(page),
                _ => None,
            }),
        };
        let mut app = App {
            page,
            file_tool: options
                .file_tool
                .unwrap_or(saved_file_tool.unwrap_or(FileTool::Renamer)),
            dev_tool: options
                .dev_tool
                .unwrap_or(saved_dev_tool.unwrap_or(DevTool::JsonYaml)),
            cks_autocopy,
            nav_open,
            progress: None,
            close_pos: None,
            dark,
            lang,
            db,
            nav_reveal: None,
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
        };

        // A folder from the command line preselects the active file tool.
        if let Some(folder) = options.folder {
            match app.file_tool {
                FileTool::Renamer => app.ren.folder = Some(folder.clone()),
                FileTool::Duplicates => app.dup.folder = Some(folder.clone()),
                FileTool::Analyzer => app.ana.folder = Some(folder.clone()),
                FileTool::Checksum => app.cks.paths = vec![folder],
            }
        }

        (app, Task::none())
    }

    pub fn iced_theme(&self) -> iced::Theme {
        iced::Theme::custom("Joinery".to_string(), crate::palette::palette(self.dark))
    }
}

pub fn subscription(app: &App) -> Subscription<Msg> {
    // The section reveal animates with a 60 fps tick for as long as it
    // is in flight; nothing runs when the sidebar is at rest.
    let reveal = if app.nav_reveal.is_some() {
        iced::time::every(std::time::Duration::from_millis(16)).map(|_| Msg::AnimTick)
    } else {
        Subscription::none()
    };
    Subscription::batch([
        reveal,
        duplicates::scan_subscription(app).map(Msg::Duplicates),
        checksum::run_subscription(app).map(Msg::Checksum),
        iced::window::close_requests().map(Msg::CloseRequested),
    ])
}

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::Nav(page) => {
            app.page = page;
            persist_settings(&app.db, "page", page_code(page));
            if page == Page::History {
                history::reload(app);
            }
            Task::none()
        }
        Msg::ToggleNav(page) => {
            // Opening a section also navigates to it; closing only
            // collapses the group.
            let opening = app.nav_open != Some(page);
            if opening {
                app.page = page;
            }
            app.nav_open = if opening { Some(page) } else { None };
            persist_settings(&app.db, "nav_open", page_code(page));
            persist_settings(&app.db, "page", page_code(app.page));
            app.nav_reveal = Some(NavReveal {
                page,
                opening,
                t: 0.0,
            });
            Task::none()
        }
        Msg::AnimTick => {
            if let Some(reveal) = &mut app.nav_reveal {
                reveal.t += 0.12;
                if reveal.t >= 1.0 {
                    app.nav_reveal = None;
                }
            }
            Task::none()
        }
        Msg::FileTool(tool) => {
            app.file_tool = tool;
            app.page = Page::Files;
            app.nav_open = Some(Page::Files);
            persist_settings(&app.db, "file_tool", file_tool_code(tool));
            persist_settings(&app.db, "page", page_code(Page::Files));
            Task::none()
        }
        Msg::DevTool(tool) => {
            app.dev_tool = tool;
            app.page = Page::Developer;
            app.nav_open = Some(Page::Developer);
            persist_settings(&app.db, "dev_tool", dev_tool_code(tool));
            persist_settings(&app.db, "page", page_code(Page::Developer));
            Task::none()
        }
        Msg::ToggleTheme => {
            app.dark = !app.dark;
            persist_settings(&app.db, "theme", if app.dark { "dark" } else { "light" });
            Task::none()
        }
        Msg::SetLang(lang) => {
            app.lang = lang;
            persist_settings(&app.db, "lang", lang.code());
            Task::none()
        }
        Msg::CloseRequested(id) => {
            iced::window::get_maximized(id).map(move |maximized| Msg::CloseMax(id, maximized))
        }
        Msg::CloseMax(id, maximized) => {
            if maximized {
                // Keep the last normal-sized geometry; do not persist
                // the maximized window's full-monitor rectangle.
                iced::window::close(id)
            } else {
                iced::window::get_position(id).map(move |pos| Msg::ClosePos(id, pos))
            }
        }
        Msg::ClosePos(id, pos) => {
            app.close_pos = pos;
            iced::window::get_size(id).map(move |size| Msg::CloseSize(id, size))
        }
        Msg::CloseSize(id, size) => {
            if let (Some(pos), Some(conn)) = (app.close_pos, &app.db) {
                if size.width >= 300.0 && size.height >= 200.0 {
                    let _ = database::set_setting(conn, "win_x", &pos.x.to_string());
                    let _ = database::set_setting(conn, "win_y", &pos.y.to_string());
                    let _ = database::set_setting(conn, "win_w", &size.width.to_string());
                    let _ = database::set_setting(conn, "win_h", &size.height.to_string());
                }
            }
            app.close_pos = None;
            iced::window::close(id)
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
        nav_open: None,
        nav_reveal: None,
        progress: None,
        cks_autocopy: true,
        close_pos: None,
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

#[cfg(test)]
mod nav_tests {
    use super::*;

    #[test]
    fn toggling_a_closed_section_opens_and_navigates() {
        let mut app = test_app();
        assert_eq!(app.nav_open, None);

        let _ = update(&mut app, Msg::ToggleNav(Page::Files));

        assert_eq!(app.nav_open, Some(Page::Files));
        assert_eq!(app.page, Page::Files);
        let reveal = app.nav_reveal.expect("reveal in flight");
        assert_eq!(reveal.page, Page::Files);
        assert!(reveal.opening);
    }

    #[test]
    fn toggling_an_open_section_collapses_without_navigating() {
        let mut app = test_app();
        app.page = Page::Home;
        app.nav_open = Some(Page::Files);

        let _ = update(&mut app, Msg::ToggleNav(Page::Files));

        assert_eq!(app.nav_open, None);
        assert_eq!(app.page, Page::Home, "closing must not navigate");
        let reveal = app.nav_reveal.expect("reveal in flight");
        assert!(!reveal.opening);
    }

    #[test]
    fn switching_sections_moves_the_open_group() {
        let mut app = test_app();
        app.nav_open = Some(Page::Files);

        let _ = update(&mut app, Msg::ToggleNav(Page::Developer));

        assert_eq!(app.nav_open, Some(Page::Developer));
        assert_eq!(app.page, Page::Developer);
    }

    #[test]
    fn animation_ticks_finish_and_clear_the_reveal() {
        let mut app = test_app();
        let _ = update(&mut app, Msg::ToggleNav(Page::Files));

        for _ in 0..20 {
            let _ = update(&mut app, Msg::AnimTick);
        }

        assert_eq!(app.nav_reveal, None, "the reveal must settle");
        assert_eq!(app.nav_open, Some(Page::Files));

        // An idle tick with nothing in flight must be a no-op.
        let _ = update(&mut app, Msg::AnimTick);
        assert_eq!(app.nav_reveal, None);
    }

    #[test]
    fn picking_a_sub_item_jumps_straight_to_the_tool() {
        let mut app = test_app();
        app.page = Page::Home;

        let _ = update(&mut app, Msg::FileTool(FileTool::Checksum));

        assert_eq!(app.page, Page::Files);
        assert_eq!(app.file_tool, FileTool::Checksum);

        let _ = update(&mut app, Msg::DevTool(DevTool::Diff));
        assert_eq!(app.page, Page::Developer);
        assert_eq!(app.dev_tool, DevTool::Diff);
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;

    #[test]
    fn session_codes_round_trip() {
        for page in [
            Page::Home,
            Page::Files,
            Page::Developer,
            Page::History,
            Page::System,
            Page::Support,
        ] {
            assert_eq!(page_from_code(page_code(page)), Some(page));
        }
        for tool in [
            FileTool::Renamer,
            FileTool::Duplicates,
            FileTool::Analyzer,
            FileTool::Checksum,
        ] {
            assert_eq!(file_tool_from_code(file_tool_code(tool)), Some(tool));
        }
        for tool in [DevTool::JsonYaml, DevTool::Diff] {
            assert_eq!(dev_tool_from_code(dev_tool_code(tool)), Some(tool));
        }
        assert_eq!(page_from_code("bogus"), None);
        assert_eq!(file_tool_from_code("bogus"), None);
        assert_eq!(dev_tool_from_code("bogus"), None);
    }

    #[test]
    fn scan_pulse_stays_in_bounds() {
        for n in 0..100u64 {
            let f = scan_pulse(n);
            assert!(
                (0.0..=1.0).contains(&f),
                "pulse {f} out of bounds for n={n}"
            );
        }
    }

    #[test]
    fn autocopy_toggle_applies() {
        let mut app = test_app();
        let _ = update(&mut app, Msg::Checksum(checksum::Msg::Autocopy(false)));
        assert!(!app.cks_autocopy);
        let _ = update(&mut app, Msg::Checksum(checksum::Msg::Autocopy(true)));
        assert!(app.cks_autocopy);
    }

    #[test]
    fn picking_a_tool_reopens_its_section() {
        let mut app = test_app();
        app.nav_open = None;
        let _ = update(&mut app, Msg::FileTool(FileTool::Checksum));
        assert_eq!(app.nav_open, Some(Page::Files));
        let _ = update(&mut app, Msg::DevTool(DevTool::Diff));
        assert_eq!(app.nav_open, Some(Page::Developer));
    }
}

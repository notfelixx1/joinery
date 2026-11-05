//! Application state, messages and the update loop.
//!
//! Everything heavy delegates to `joinery_tools::core` / `::safety` —
//! exactly the same functions the Tauri commands call. No IPC here:
//! the native frontend talks to the core in-process.

use std::collections::HashMap;
use std::path::PathBuf;

use iced::futures::SinkExt;
use iced::widget::text_editor;
use iced::{Subscription, Task};
use rusqlite::Connection;

use joinery_tools::core::{analyzer, checksum, duplicates, renamer, text_diff, validators};
use joinery_tools::safety::undo;
use joinery_tools::storage::database;

use crate::i18n::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Files,
    Developer,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JyKind {
    Json,
    Yaml,
}

impl JyKind {
    fn label(self) -> &'static str {
        match self {
            JyKind::Json => "JSON",
            JyKind::Yaml => "YAML",
        }
    }
}

impl std::fmt::Display for JyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

// ---------- per-tool state ----------

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
}

#[derive(Debug, Default)]
pub struct AnalyzerState {
    pub folder: Option<PathBuf>,
    pub busy: bool,
    pub analysis: Option<analyzer::FolderAnalysis>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChecksumItem {
    pub path: PathBuf,
    pub hash: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct ChecksumState {
    pub paths: Vec<PathBuf>,
    pub algorithm: checksum::Algorithm,
    pub busy: bool,
    pub results: Option<Vec<ChecksumItem>>,
    pub expected: String,
    pub copied: Option<String>,
    pub error: Option<String>,
}

impl Default for ChecksumState {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            algorithm: checksum::Algorithm::Sha256,
            busy: false,
            results: None,
            expected: String::new(),
            copied: None,
            error: None,
        }
    }
}

#[derive(Debug)]
pub struct JsonYamlState {
    pub kind: JyKind,
    pub content: text_editor::Content,
    pub busy: bool,
    pub result: Option<validators::ValidationResult>,
    pub error: Option<String>,
}

impl Default for JsonYamlState {
    fn default() -> Self {
        Self {
            kind: JyKind::Json,
            content: text_editor::Content::new(),
            busy: false,
            result: None,
            error: None,
        }
    }
}

#[derive(Debug)]
pub struct DiffState {
    pub old_text: text_editor::Content,
    pub new_text: text_editor::Content,
    pub busy: bool,
    pub result: Option<text_diff::DiffResult>,
    pub error: Option<String>,
    /// `true` → load file into the old pane, `false` → the new pane.
    pub loading_target: Option<bool>,
}

impl Default for DiffState {
    fn default() -> Self {
        Self {
            old_text: text_editor::Content::new(),
            new_text: text_editor::Content::new(),
            busy: false,
            result: None,
            error: None,
            loading_target: None,
        }
    }
}

// ---------- messages ----------

#[derive(Debug, Clone)]
pub enum Msg {
    Nav(Page),
    FileTool(FileTool),
    DevTool(DevTool),
    ToggleTheme,
    ToggleLang,

    RenChooseFolder,
    RenFolder(Option<String>),
    RenFind(String),
    RenReplace(String),
    RenPrefix(String),
    RenSuffix(String),
    RenNewExt(String),
    RenPattern(String),
    RenSubfolders(bool),
    RenAllowOverwrite(bool),
    RenPreview,
    RenPlanReady(Result<Vec<renamer::RenamePlanItem>, String>),
    RenApply,
    RenApplyReady(Result<renamer::ApplyOutcome, String>),
    RenUndo,
    RenUndoReady {
        total: usize,
        res: Result<usize, String>,
    },

    DupChooseFolder,
    DupFolder(Option<String>),
    DupMinSize(String),
    DupScan,
    DupProgress(u64),
    DupDone(Result<duplicates::ScanResult, String>),
    DupToggle(String, bool),
    DupDelete,
    DupDeleteReady(Result<Vec<(PathBuf, bool, Option<String>)>, String>),

    AnaChooseFolder,
    AnaFolder(Option<String>),
    AnaRun,
    AnaDone(Result<analyzer::FolderAnalysis, String>),

    CksChooseFiles,
    CksFiles(Vec<String>),
    CksAlgorithm(checksum::Algorithm),
    CksRun,
    CksDone(Vec<ChecksumItem>),
    CksExpected(String),
    CksCopy(String),
    Copied,

    JyKind(JyKind),
    JyEdit(text_editor::Action),
    JyExample,
    JyClear,
    JyValidate,
    JyResult(Result<validators::ValidationResult, String>),
    JyReformat(bool),
    JyFormatReady(Result<Result<String, validators::FormatError>, String>),

    DiffEdit(bool, text_editor::Action),
    DiffLoad(bool),
    DiffFileReady {
        into_old: bool,
        res: Result<String, String>,
    },
    DiffRun,
    DiffDone(Result<text_diff::DiffResult, String>),
    DiffClear,
}

fn flatten<T>(res: Result<Result<T, String>, tokio::task::JoinError>) -> Result<T, String> {
    match res {
        Ok(inner) => inner,
        Err(err) => Err(format!("internal error: {err}")),
    }
}

fn pick_folder_task(map: fn(Option<String>) -> Msg) -> Task<Msg> {
    Task::perform(
        tokio::task::spawn_blocking(move || {
            rfd::FileDialog::new()
                .pick_folder()
                .map(|p| p.to_string_lossy().to_string())
        }),
        move |res| map(res.unwrap_or(None)),
    )
}

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
        |res| Msg::CksFiles(res.unwrap_or_default()),
    )
}

fn pick_file_task(into_old: bool) -> Task<Msg> {
    Task::perform(
        tokio::task::spawn_blocking(move || rfd::FileDialog::new().pick_file()),
        move |res| {
            let into_old = into_old;
            let path = res.unwrap_or(None);
            Msg::DiffFileReady {
                into_old,
                res: match path {
                    Some(p) => read_text_file(&p),
                    None => Err(String::new()),
                },
            }
        },
    )
}

fn read_text_file(path: &std::path::Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a regular file".to_string());
    }
    const CAP: usize = 5 * 1024 * 1024;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let take = CAP.min(bytes.len());
    Ok(String::from_utf8_lossy(&bytes[..take]).to_string())
}

// ---------- app ----------

pub struct App {
    pub page: Page,
    pub file_tool: FileTool,
    pub dev_tool: DevTool,
    pub dark: bool,
    pub lang: Lang,
    pub db: Option<Connection>,

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

    fn persist(&self, key: &str, value: &str) {
        if let Some(conn) = &self.db {
            let _ = database::set_setting(conn, key, value);
        }
    }

    fn start_dup_scan(&mut self) -> Task<Msg> {
        if self.dup.folder.is_none() {
            return Task::none();
        }
        self.dup.scanning = true;
        self.dup.scan_id += 1;
        self.dup.scanned = 0;
        self.dup.result = None;
        self.dup.selected.clear();
        self.dup.error = None;
        Task::none()
    }
}

pub fn subscription(app: &App) -> Subscription<Msg> {
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
                let _ = out.send(Msg::DupProgress(n)).await;
            }
            let result = handle
                .await
                .unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())));
            let _ = out
                .send(Msg::DupDone(result.map_err(|e| e.to_string())))
                .await;
        }),
    )
}

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::Nav(page) => {
            app.page = page;
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
            app.persist("theme", if app.dark { "dark" } else { "light" });
            Task::none()
        }
        Msg::ToggleLang => {
            app.lang = app.lang.toggle();
            app.persist(
                "lang",
                match app.lang {
                    Lang::En => "en",
                    Lang::Ru => "ru",
                },
            );
            Task::none()
        }

        // ---------- Batch Renamer ----------
        Msg::RenChooseFolder => {
            app.ren.reset_result();
            pick_folder_task(Msg::RenFolder)
        }
        Msg::RenFolder(path) => {
            app.ren.folder = path.map(PathBuf::from);
            Task::none()
        }
        Msg::RenFind(v) => {
            app.ren.find = v;
            Task::none()
        }
        Msg::RenReplace(v) => {
            app.ren.replace = v;
            Task::none()
        }
        Msg::RenPrefix(v) => {
            app.ren.prefix = v;
            Task::none()
        }
        Msg::RenSuffix(v) => {
            app.ren.suffix = v;
            Task::none()
        }
        Msg::RenNewExt(v) => {
            app.ren.new_ext = v;
            Task::none()
        }
        Msg::RenPattern(v) => {
            app.ren.pattern = v;
            Task::none()
        }
        Msg::RenSubfolders(v) => {
            app.ren.include_subfolders = v;
            Task::none()
        }
        Msg::RenAllowOverwrite(v) => {
            app.ren.allow_overwrite = v;
            Task::none()
        }
        Msg::RenPreview => {
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
                |res| Msg::RenPlanReady(flatten(res)),
            )
        }
        Msg::RenPlanReady(res) => {
            app.ren.busy = false;
            match res {
                Ok(plan) => {
                    app.ren.ready_count = plan
                        .iter()
                        .filter(|i| i.status == renamer::RenameStatus::Ready)
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
        Msg::RenApply => {
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
            let records: Vec<renamer::RenameRecord> = items
                .iter()
                .filter(|i| i.status == renamer::RenameStatus::Ready)
                .map(|i| renamer::RenameRecord {
                    from: i.from.clone(),
                    to: i.to.clone(),
                })
                .collect();
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
                |res| Msg::RenApplyReady(flatten(res)),
            )
        }
        Msg::RenApplyReady(res) => {
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
        Msg::RenUndo => {
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
                move |res| Msg::RenUndoReady {
                    total,
                    res: flatten(res),
                },
            )
        }
        Msg::RenUndoReady { total, res, .. } => {
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
                    let template = crate::i18n::tr(app.lang, "renamer.undoDone");
                    app.ren.note = Some(crate::i18n::fmt(
                        template,
                        &[("n", undone.to_string()), ("total", total.to_string())],
                    ));
                }
                Err(err) => app.ren.error = Some(err),
            }
            Task::none()
        }

        // ---------- Duplicate Finder ----------
        Msg::DupChooseFolder => {
            app.dup.result = None;
            app.dup.note = None;
            app.dup.error = None;
            pick_folder_task(Msg::DupFolder)
        }
        Msg::DupFolder(path) => {
            app.dup.folder = path.map(PathBuf::from);
            Task::none()
        }
        Msg::DupMinSize(v) => {
            app.dup.min_size = v;
            Task::none()
        }
        Msg::DupScan => app.start_dup_scan(),
        Msg::DupProgress(n) => {
            app.dup.scanned = n;
            Task::none()
        }
        Msg::DupDone(res) => {
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
        Msg::DupToggle(path, value) => {
            app.dup.selected.insert(path, value);
            Task::none()
        }
        Msg::DupDelete => {
            let Some(result) = &app.dup.result else {
                return Task::none();
            };
            let targets: Vec<(PathBuf, u64)> = result
                .groups
                .iter()
                .flat_map(|g| g.files.iter())
                .filter(|f| {
                    app.dup
                        .selected
                        .get(&f.path.to_string_lossy().to_string())
                        .copied()
                        .unwrap_or(false)
                })
                .map(|f| (f.path.clone(), f.size))
                .collect();
            if targets.is_empty() {
                return Task::none();
            }
            let count = targets.len();
            let confirm_text = crate::i18n::fmt(
                crate::i18n::tr(app.lang, "dup.confirm"),
                &[("n", count.to_string())],
            );
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
                |res| Msg::DupDeleteReady(flatten(res)),
            )
        }
        Msg::DupDeleteReady(res) => match res {
            Ok(results) => {
                let deleted = results.iter().filter(|(_, ok, _)| *ok).count();
                let failed = results.len() - deleted;
                let template = crate::i18n::tr(app.lang, "dup.deleted");
                app.dup.note = Some(crate::i18n::fmt(
                    template,
                    &[
                        ("deleted", deleted.to_string()),
                        ("failed", failed.to_string()),
                    ],
                ));
                app.start_dup_scan()
            }
            Err(err) if err == "cancelled" => Task::none(),
            Err(err) => {
                app.dup.error = Some(err);
                Task::none()
            }
        },

        // ---------- Folder Analyzer ----------
        Msg::AnaChooseFolder => {
            app.ana.analysis = None;
            app.ana.error = None;
            pick_folder_task(Msg::AnaFolder)
        }
        Msg::AnaFolder(path) => {
            app.ana.folder = path.map(PathBuf::from);
            Task::none()
        }
        Msg::AnaRun => {
            let Some(root) = app.ana.folder.clone() else {
                return Task::none();
            };
            app.ana.busy = true;
            app.ana.error = None;
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    analyzer::analyze_folder(&root).map_err(|e| e.to_string())
                }),
                |res| Msg::AnaDone(flatten(res)),
            )
        }
        Msg::AnaDone(res) => {
            app.ana.busy = false;
            match res {
                Ok(analysis) => app.ana.analysis = Some(analysis),
                Err(err) => app.ana.error = Some(err),
            }
            Task::none()
        }

        // ---------- Checksum ----------
        Msg::CksChooseFiles => {
            app.cks.results = None;
            app.cks.error = None;
            pick_files_task()
        }
        Msg::CksFiles(paths) => {
            app.cks.paths = paths.into_iter().map(PathBuf::from).collect();
            Task::none()
        }
        Msg::CksAlgorithm(algorithm) => {
            app.cks.algorithm = algorithm;
            Task::none()
        }
        Msg::CksRun => {
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
                        .map(|path| match checksum::hash_file(&path, algorithm) {
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
                |res| Msg::CksDone(res.unwrap_or_default()),
            )
        }
        Msg::CksDone(items) => {
            app.cks.busy = false;
            app.cks.results = Some(items);
            Task::none()
        }
        Msg::CksExpected(v) => {
            app.cks.expected = v;
            Task::none()
        }
        Msg::CksCopy(hash) => {
            app.cks.copied = Some(hash.clone());
            iced::clipboard::write::<Msg>(hash).map(|_| Msg::Copied)
        }
        Msg::Copied => Task::none(),

        // ---------- JSON / YAML ----------
        Msg::JyKind(kind) => {
            app.jy.kind = kind;
            app.jy.result = None;
            app.jy.error = None;
            Task::none()
        }
        Msg::JyEdit(action) => {
            app.jy.content.perform(action);
            Task::none()
        }
        Msg::JyExample => {
            let sample = match app.jy.kind {
                JyKind::Json => "{\n  \"name\": \"joinery\",\n  \"tools\": [\"renamer\", \"diff\"],\n  \"offline\": true\n}",
                JyKind::Yaml => "name: joinery\ntools:\n  - renamer\n  - diff\noffline: true\n",
            };
            app.jy.content = text_editor::Content::with_text(sample);
            app.jy.result = None;
            app.jy.error = None;
            Task::none()
        }
        Msg::JyClear => {
            app.jy.content = text_editor::Content::new();
            app.jy.result = None;
            app.jy.error = None;
            Task::none()
        }
        Msg::JyValidate => {
            let text = app.jy.content.text().to_string();
            if text.trim().is_empty() {
                return Task::none();
            }
            let kind = app.jy.kind;
            app.jy.busy = true;
            app.jy.error = None;
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    Ok(match kind {
                        JyKind::Json => validators::validate_json(&text),
                        JyKind::Yaml => validators::validate_yaml(&text),
                    })
                }),
                |res| Msg::JyResult(flatten(res)),
            )
        }
        Msg::JyResult(res) => {
            app.jy.busy = false;
            match res {
                Ok(result) => app.jy.result = Some(result),
                Err(err) => app.jy.error = Some(err),
            }
            Task::none()
        }
        Msg::JyReformat(minify) => {
            if app.jy.kind != JyKind::Json {
                return Task::none();
            }
            let text = app.jy.content.text().to_string();
            if text.trim().is_empty() {
                return Task::none();
            }
            app.jy.busy = true;
            app.jy.error = None;
            Task::perform(
                tokio::task::spawn_blocking(move || Ok(validators::format_json(&text, minify))),
                |res| Msg::JyFormatReady(flatten(res)),
            )
        }
        Msg::JyFormatReady(res) => {
            app.jy.busy = false;
            match res {
                Ok(Ok(formatted)) => {
                    app.jy.content = text_editor::Content::with_text(&formatted);
                    app.jy.result = Some(validators::ValidationResult {
                        kind: validators::DocKind::Json,
                        valid: true,
                        error: None,
                        formatted: Some(formatted),
                    });
                }
                Ok(Err(err)) => {
                    app.jy.result = Some(validators::ValidationResult {
                        kind: validators::DocKind::Json,
                        valid: false,
                        error: Some(err),
                        formatted: None,
                    });
                }
                Err(err) => app.jy.error = Some(err),
            }
            Task::none()
        }

        // ---------- Text Diff ----------
        Msg::DiffEdit(is_old, action) => {
            if is_old {
                app.diff.old_text.perform(action);
            } else {
                app.diff.new_text.perform(action);
            }
            Task::none()
        }
        Msg::DiffLoad(into_old) => {
            app.diff.loading_target = Some(into_old);
            pick_file_task(into_old)
        }
        Msg::DiffFileReady { into_old, res } => {
            app.diff.loading_target = None;
            match res {
                Ok(text) => {
                    let content = text_editor::Content::with_text(&text);
                    if into_old {
                        app.diff.old_text = content;
                    } else {
                        app.diff.new_text = content;
                    }
                    app.diff.result = None;
                    app.diff.error = None;
                }
                Err(err) if !err.is_empty() => app.diff.error = Some(err),
                Err(_) => {}
            }
            Task::none()
        }
        Msg::DiffRun => {
            let old = app.diff.old_text.text().to_string();
            let new = app.diff.new_text.text().to_string();
            app.diff.busy = true;
            app.diff.error = None;
            Task::perform(
                tokio::task::spawn_blocking(move || Ok(text_diff::diff_texts(&old, &new))),
                |res| Msg::DiffDone(flatten(res)),
            )
        }
        Msg::DiffDone(res) => {
            app.diff.busy = false;
            match res {
                Ok(result) => app.diff.result = Some(result),
                Err(err) => app.diff.error = Some(err),
            }
            Task::none()
        }
        Msg::DiffClear => {
            app.diff.old_text = text_editor::Content::new();
            app.diff.new_text = text_editor::Content::new();
            app.diff.result = None;
            app.diff.error = None;
            Task::none()
        }
    }
}

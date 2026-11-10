//! Text Diff: state, messages and the update loop.

use iced::widget::text_editor;
use iced::Task;

use joinery_tools::core::text_diff;

use super::flatten;
use super::App;

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

#[derive(Debug, Clone)]
pub enum Msg {
    Edit(bool, text_editor::Action),
    Load(bool),
    FileReady {
        into_old: bool,
        res: Result<String, String>,
    },
    Run,
    Done(Result<text_diff::DiffResult, String>),
    Clear,
}

/// Native file picker; the picked file is read off the main thread and
/// delivered as `FileReady`.
fn pick_file_task(into_old: bool) -> Task<Msg> {
    Task::perform(
        tokio::task::spawn_blocking(move || rfd::FileDialog::new().pick_file()),
        move |res| {
            let into_old = into_old;
            let path = res.unwrap_or(None);
            Msg::FileReady {
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

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::Edit(is_old, action) => {
            if is_old {
                app.diff.old_text.perform(action);
            } else {
                app.diff.new_text.perform(action);
            }
            Task::none()
        }
        Msg::Load(into_old) => {
            app.diff.loading_target = Some(into_old);
            pick_file_task(into_old)
        }
        Msg::FileReady { into_old, res } => {
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
        Msg::Run => {
            let old = app.diff.old_text.text().to_string();
            let new = app.diff.new_text.text().to_string();
            let lang = app.lang;
            app.diff.busy = true;
            app.diff.error = None;
            Task::perform(
                tokio::task::spawn_blocking(move || {
                    // Giant inputs get a confirmation first: the result
                    // rows are heavy to build and display.
                    let lines = old.lines().count() + new.lines().count();
                    if lines > 200_000 {
                        let confirm_text = crate::i18n::fmt(
                            crate::i18n::tr(lang, "diff.giantConfirm"),
                            &[("n", lines.to_string())],
                        );
                        let confirmed = rfd::MessageDialog::new()
                            .set_title("Joinery Tools")
                            .set_description(&confirm_text)
                            .set_buttons(rfd::MessageButtons::OkCancel)
                            .show();
                        if !matches!(
                            confirmed,
                            rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes
                        ) {
                            return Err("cancelled".to_string());
                        }
                    }
                    Ok(text_diff::diff_texts(&old, &new))
                }),
                |res| Msg::Done(flatten(res)),
            )
        }
        Msg::Done(res) => {
            app.diff.busy = false;
            match res {
                Ok(result) => app.diff.result = Some(result),
                Err(err) if err == "cancelled" => {}
                Err(err) => app.diff.error = Some(err),
            }
            Task::none()
        }
        Msg::Clear => {
            app.diff.old_text = text_editor::Content::new();
            app.diff.new_text = text_editor::Content::new();
            app.diff.result = None;
            app.diff.error = None;
            Task::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_app;
    use super::*;

    // `text_editor::Content::text()` appends a trailing newline to any
    // buffer — even an empty one renders as "\n". This helper keeps the
    // assertion intent obvious.
    fn text(c: &text_editor::Content) -> String {
        c.text().trim_end_matches('\n').to_string()
    }

    #[test]
    fn file_ready_loads_into_the_requested_pane() {
        let mut app = test_app();
        app.diff.loading_target = Some(true);

        let _ = update(
            &mut app,
            Msg::FileReady {
                into_old: true,
                res: Ok("hello".to_string()),
            },
        );

        assert_eq!(text(&app.diff.old_text), "hello");
        assert!(text(&app.diff.new_text).is_empty());
        assert!(app.diff.loading_target.is_none());
        assert!(app.diff.error.is_none());
    }

    #[test]
    fn blank_file_pick_is_ignored() {
        let mut app = test_app();

        let _ = update(
            &mut app,
            Msg::FileReady {
                into_old: false,
                res: Err(String::new()),
            },
        );

        // An empty error means "nothing picked" — not a failure.
        assert!(app.diff.error.is_none());
        assert!(text(&app.diff.new_text).is_empty());
    }

    #[test]
    fn done_stores_the_result() {
        let mut app = test_app();
        app.diff.busy = true;

        let _ = update(&mut app, Msg::Done(Ok(text_diff::diff_texts("a", "b"))));

        assert!(!app.diff.busy);
        assert!(app.diff.result.is_some());
    }

    #[test]
    fn clear_resets_everything() {
        let mut app = test_app();
        app.diff.old_text = text_editor::Content::with_text("old");
        app.diff.new_text = text_editor::Content::with_text("new");
        app.diff.result = Some(text_diff::diff_texts("old", "new"));
        app.diff.error = Some("e".to_string());

        let _ = update(&mut app, Msg::Clear);

        assert!(text(&app.diff.old_text).is_empty());
        assert!(text(&app.diff.new_text).is_empty());
        assert!(app.diff.result.is_none());
        assert!(app.diff.error.is_none());
    }
}

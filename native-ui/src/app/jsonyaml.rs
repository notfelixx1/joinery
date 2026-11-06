//! JSON / YAML validator: state, messages and the update loop.

use iced::widget::text_editor;
use iced::Task;

use joinery_tools::core::validators;

use super::flatten;
use super::App;

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

#[derive(Debug, Clone)]
pub enum Msg {
    Kind(JyKind),
    Edit(text_editor::Action),
    Example,
    Clear,
    Validate,
    Result(Result<validators::ValidationResult, String>),
    Reformat(bool),
    FormatReady(Result<Result<String, validators::FormatError>, String>),
}

pub fn update(app: &mut App, msg: Msg) -> Task<Msg> {
    match msg {
        Msg::Kind(kind) => {
            app.jy.kind = kind;
            app.jy.result = None;
            app.jy.error = None;
            Task::none()
        }
        Msg::Edit(action) => {
            app.jy.content.perform(action);
            Task::none()
        }
        Msg::Example => {
            let sample = match app.jy.kind {
                JyKind::Json => "{\n  \"name\": \"joinery\",\n  \"tools\": [\"renamer\", \"diff\"],\n  \"offline\": true\n}",
                JyKind::Yaml => "name: joinery\ntools:\n  - renamer\n  - diff\noffline: true\n",
            };
            app.jy.content = text_editor::Content::with_text(sample);
            app.jy.result = None;
            app.jy.error = None;
            Task::none()
        }
        Msg::Clear => {
            app.jy.content = text_editor::Content::new();
            app.jy.result = None;
            app.jy.error = None;
            Task::none()
        }
        Msg::Validate => {
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
                |res| Msg::Result(flatten(res)),
            )
        }
        Msg::Result(res) => {
            app.jy.busy = false;
            match res {
                Ok(result) => app.jy.result = Some(result),
                Err(err) => app.jy.error = Some(err),
            }
            Task::none()
        }
        Msg::Reformat(minify) => {
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
                |res| Msg::FormatReady(flatten(res)),
            )
        }
        Msg::FormatReady(res) => {
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
    }
}

#[cfg(test)]
mod tests {
    use joinery_tools::core::validators;

    use super::super::test_app;
    use super::*;

    #[test]
    fn switching_kind_clears_stale_results() {
        let mut app = test_app();
        app.jy.result = Some(validators::ValidationResult {
            kind: validators::DocKind::Json,
            valid: true,
            error: None,
            formatted: Some("{}".to_string()),
        });
        app.jy.error = Some("old".to_string());

        let _ = update(&mut app, Msg::Kind(JyKind::Yaml));

        assert_eq!(app.jy.kind, JyKind::Yaml);
        assert!(app.jy.result.is_none());
        assert!(app.jy.error.is_none());
    }

    #[test]
    fn blank_content_is_not_submitted() {
        let mut app = test_app();
        app.jy.content = text_editor::Content::with_text("   \n\t");

        let _ = update(&mut app, Msg::Validate);

        assert!(!app.jy.busy);
        assert!(app.jy.result.is_none());
        assert!(app.jy.error.is_none());
    }

    #[test]
    fn format_ready_error_marks_the_result_invalid() {
        let mut app = test_app();
        let err = validators::FormatError {
            message: "bad".to_string(),
            line: Some(1),
            column: Some(2),
        };

        let _ = update(&mut app, Msg::FormatReady(Ok(Err(err))));

        let result = app.jy.result.as_ref().unwrap();
        assert!(!result.valid);
        assert!(app.jy.error.is_none());
    }

    #[test]
    fn kind_labels() {
        assert_eq!(JyKind::Json.to_string(), "JSON");
        assert_eq!(JyKind::Yaml.to_string(), "YAML");
    }
}

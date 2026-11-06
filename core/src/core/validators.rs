//! JSON and YAML validation / formatting.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml_ng::Value as YamlValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    Json,
    Yaml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub kind: DocKind,
    pub valid: bool,
    pub error: Option<FormatError>,
    /// Pretty-printed document when valid, `None` otherwise.
    pub formatted: Option<String>,
}

/// Validate a JSON document and return its pretty form when valid.
pub fn validate_json(text: &str) -> ValidationResult {
    match serde_json::from_str::<JsonValue>(text) {
        Ok(value) => ValidationResult {
            kind: DocKind::Json,
            valid: true,
            error: None,
            formatted: serde_json::to_string_pretty(&value).ok(),
        },
        Err(err) => ValidationResult {
            kind: DocKind::Json,
            valid: false,
            error: Some(FormatError {
                message: err.to_string(),
                line: Some(err.line()),
                column: Some(err.column()),
            }),
            formatted: None,
        },
    }
}

/// Validate a YAML document and return its canonical form when valid.
pub fn validate_yaml(text: &str) -> ValidationResult {
    match serde_yaml_ng::from_str::<YamlValue>(text) {
        Ok(value) => ValidationResult {
            kind: DocKind::Yaml,
            valid: true,
            error: None,
            formatted: serde_yaml_ng::to_string(&value).ok(),
        },
        Err(err) => {
            let (line, column) = err
                .location()
                .map(|loc| (Some(loc.line()), Some(loc.column())))
                .unwrap_or((None, None));
            ValidationResult {
                kind: DocKind::Yaml,
                valid: false,
                error: Some(FormatError {
                    message: err.to_string(),
                    line,
                    column,
                }),
                formatted: None,
            }
        }
    }
}

/// Format (pretty-print or minify) a JSON document.
pub fn format_json(text: &str, minify: bool) -> Result<String, FormatError> {
    let value: JsonValue = serde_json::from_str(text).map_err(|err| FormatError {
        message: err.to_string(),
        line: Some(err.line()),
        column: Some(err.column()),
    })?;
    if minify {
        serde_json::to_string(&value)
    } else {
        serde_json::to_string_pretty(&value)
    }
    .map_err(|err| FormatError {
        message: err.to_string(),
        line: None,
        column: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_json_is_pretty_printed() {
        let result = validate_json(r#"{"a":1,"b":[2,3]}"#);
        assert!(result.valid);
        assert_eq!(
            result.formatted.as_deref(),
            Some("{\n  \"a\": 1,\n  \"b\": [\n    2,\n    3\n  ]\n}")
        );
    }

    #[test]
    fn invalid_json_reports_position() {
        let result = validate_json("{\n  \"a\": 1,\n}");
        assert!(!result.valid);
        let error = result.error.expect("error");
        assert_eq!(error.line, Some(3));
        assert!(error.column.is_some());
    }

    #[test]
    fn valid_yaml_is_canonicalized() {
        let result = validate_yaml("a: 1\nb:\n  - x\n  - y\n");
        assert!(result.valid, "unexpected error: {:?}", result.error);
        let formatted = result.formatted.expect("formatted");
        assert!(formatted.contains("a: 1"));
        assert!(formatted.contains("- x"));
    }

    #[test]
    fn invalid_yaml_reports_error() {
        let result = validate_yaml("a: [1, 2\nb: c\n");
        assert!(!result.valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn format_json_minify() {
        let out = format_json("{ \"a\" : 1 , \"b\" : 2 }", true).expect("formatted");
        assert_eq!(out, r#"{"a":1,"b":2}"#);
    }

    #[test]
    fn format_json_invalid_is_error() {
        assert!(format_json("{ nope", false).is_err());
    }
}

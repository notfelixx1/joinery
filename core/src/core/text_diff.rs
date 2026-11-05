//! Line-based text comparison built on the `similar` crate.

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffKind {
    Equal,
    Add,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffRow {
    pub kind: DiffKind,
    /// 1-based line number in the old text, when applicable.
    pub old_line: Option<usize>,
    /// 1-based line number in the new text, when applicable.
    pub new_line: Option<usize>,
    /// Line content, newline stripped. A trailing newline in the input is
    /// preserved as an empty final row marker via `eol_missing`.
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiffStats {
    pub added: usize,
    pub removed: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    pub rows: Vec<DiffRow>,
    pub stats: DiffStats,
}

/// Compare two texts line by line.
pub fn diff_texts(old_text: &str, new_text: &str) -> DiffResult {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut rows = Vec::new();
    let mut stats = DiffStats::default();

    for change in diff.iter_all_changes() {
        let text = change.value().trim_end_matches(['\n', '\r']);
        match change.tag() {
            ChangeTag::Equal => {
                stats.unchanged += 1;
                rows.push(DiffRow {
                    kind: DiffKind::Equal,
                    old_line: change.old_index().map(|i| i + 1),
                    new_line: change.new_index().map(|i| i + 1),
                    text: text.to_string(),
                });
            }
            ChangeTag::Delete => {
                stats.removed += 1;
                rows.push(DiffRow {
                    kind: DiffKind::Delete,
                    old_line: change.old_index().map(|i| i + 1),
                    new_line: None,
                    text: text.to_string(),
                });
            }
            ChangeTag::Insert => {
                stats.added += 1;
                rows.push(DiffRow {
                    kind: DiffKind::Add,
                    old_line: None,
                    new_line: change.new_index().map(|i| i + 1),
                    text: text.to_string(),
                });
            }
        }
    }

    DiffResult { rows, stats }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_texts_have_no_changes() {
        let result = diff_texts("a\nb\nc\n", "a\nb\nc\n");
        assert_eq!(result.stats.added, 0);
        assert_eq!(result.stats.removed, 0);
        assert_eq!(result.stats.unchanged, 3);
    }

    #[test]
    fn detects_additions_and_deletions() {
        let old_text = "one\ntwo\nthree\n";
        let new_text = "one\ndos\nthree\nfour\n";
        let result = diff_texts(old_text, new_text);
        assert!(result.stats.added >= 2, "expect additions, got {:?}", result.stats);
        assert!(result.stats.removed >= 1, "expect removals, got {:?}", result.stats);

        let added: Vec<&DiffRow> = result.rows.iter().filter(|r| r.kind == DiffKind::Add).collect();
        let deleted: Vec<&DiffRow> = result
            .rows
            .iter()
            .filter(|r| r.kind == DiffKind::Delete)
            .collect();
        assert!(added.iter().any(|r| r.text == "four"));
        assert!(deleted.iter().any(|r| r.text == "two"));
    }

    #[test]
    fn empty_inputs() {
        let result = diff_texts("", "line\n");
        assert_eq!(result.stats.added, 1);
        let result = diff_texts("line\n", "");
        assert_eq!(result.stats.removed, 1);
    }

    #[test]
    fn line_numbers_are_one_based() {
        let result = diff_texts("a\nb\n", "a\nc\n");
        let changed: Vec<&DiffRow> = result
            .rows
            .iter()
            .filter(|r| r.kind != DiffKind::Equal)
            .collect();
        for row in changed {
            if row.kind == DiffKind::Delete {
                assert_eq!(row.old_line, Some(2));
            } else {
                assert_eq!(row.new_line, Some(2));
            }
        }
    }
}

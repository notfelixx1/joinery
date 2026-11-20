//! Interface translations.
//!
//! Dictionaries live in `native-ui/i18n/<code>.json` — flat
//! `"key": "translated text"` maps (placeholders in the user's sense:
//! every UI string is addressed by a stable key like `nav.support`
//! and resolved to the language's text at render time). Variables
//! inside a value, such as `{n}`, are substituted afterwards by
//! [`fmt`]. The files are embedded into the binary at compile time
//! and parsed once on first use; missing keys fall back to English.

use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    Ru,
    Uk,
    Es,
    De,
    Fr,
    Pt,
    It,
}

impl Lang {
    /// Every supported language, in picker order.
    pub const ALL: [Lang; 8] = [
        Lang::En,
        Lang::Ru,
        Lang::Uk,
        Lang::Es,
        Lang::De,
        Lang::Fr,
        Lang::Pt,
        Lang::It,
    ];

    /// Lowercase code used on the CLI, in SQLite and in the file names.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::Uk => "uk",
            Lang::Es => "es",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Pt => "pt",
            Lang::It => "it",
        }
    }

    /// The language's own name, for the picker.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Русский",
            Lang::Uk => "Українська",
            Lang::Es => "Español",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::Pt => "Português",
            Lang::It => "Italiano",
        }
    }

    pub fn from_str(value: &str) -> Lang {
        match value {
            "ru" => Lang::Ru,
            "uk" => Lang::Uk,
            "es" => Lang::Es,
            "de" => Lang::De,
            "fr" => Lang::Fr,
            "pt" => Lang::Pt,
            "it" => Lang::It,
            _ => Lang::En,
        }
    }
}

/// The language's own name — also what the pick_list renders.
impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.native_name())
    }
}

/// `(language, embedded dictionary file)`.
const DICTIONARIES: &[(Lang, &str)] = &[
    (Lang::En, include_str!("../i18n/en.json")),
    (Lang::Ru, include_str!("../i18n/ru.json")),
    (Lang::Uk, include_str!("../i18n/uk.json")),
    (Lang::Es, include_str!("../i18n/es.json")),
    (Lang::De, include_str!("../i18n/de.json")),
    (Lang::Fr, include_str!("../i18n/fr.json")),
    (Lang::Pt, include_str!("../i18n/pt.json")),
    (Lang::It, include_str!("../i18n/it.json")),
];

type Table = HashMap<&'static str, &'static str>;

/// Parse every dictionary once and leak it — the strings live for the
/// whole process, so `tr` can hand out `&'static str` without cloning.
fn tables() -> &'static HashMap<Lang, Table> {
    static TABLES: OnceLock<HashMap<Lang, Table>> = OnceLock::new();
    TABLES.get_or_init(|| {
        DICTIONARIES
            .iter()
            .map(|(lang, json)| {
                let parsed: HashMap<String, String> = serde_json::from_str(json)
                    .unwrap_or_else(|err| panic!("i18n/{}.json is invalid: {err}", lang.code()));
                let leaked = parsed
                    .into_iter()
                    .map(|(k, v)| {
                        // `Box::leak` yields `&mut str`; the immutable
                        // bindings below reborrow it as `&'static str`.
                        let key: &'static str = Box::leak(k.into_boxed_str());
                        let value: &'static str = Box::leak(v.into_boxed_str());
                        (key, value)
                    })
                    .collect();
                (*lang, leaked)
            })
            .collect()
    })
}

/// Translate a key for the given language. Keys missing from a
/// dictionary fall back to English; unknown keys render as empty.
pub fn tr(lang: Lang, key: &str) -> &'static str {
    let tables = tables();
    tables
        .get(&lang)
        .and_then(|table| table.get(key))
        .copied()
        .or_else(|| tables[&Lang::En].get(key).copied())
        .unwrap_or("")
}

/// Substitute `{name}` variables in a translated template.
pub fn fmt(template: &str, vars: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (name, value) in vars {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `{token}` placeholders used by `fmt`, extracted in order.
    fn placeholders(value: &str) -> Vec<String> {
        let bytes: Vec<char> = value.chars().collect();
        let mut found = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == '{' {
                if let Some(end) = bytes[i..].iter().position(|c| *c == '}') {
                    found.push(bytes[i + 1..i + end].iter().collect());
                    i += end;
                }
            }
            i += 1;
        }
        found
    }

    fn en() -> &'static Table {
        &tables()[&Lang::En]
    }

    #[test]
    fn every_language_covers_exactly_the_english_keys() {
        let en_keys: Vec<_> = en().keys().copied().collect();
        assert_eq!(en_keys.len(), 145, "English dictionary size");
        for lang in Lang::ALL {
            let table = &tables()[&lang];
            for key in &en_keys {
                assert!(
                    table.contains_key(key),
                    "{} misses the key {key}",
                    lang.code()
                );
            }
            for key in table.keys() {
                assert!(
                    en().contains_key(key),
                    "{} has the extra key {key}",
                    lang.code()
                );
            }
        }
    }

    #[test]
    fn no_dictionary_value_is_empty() {
        for lang in Lang::ALL {
            for (key, value) in &tables()[&lang] {
                assert!(!value.trim().is_empty(), "{}: {key} is empty", lang.code());
            }
        }
    }

    #[test]
    fn placeholders_match_english_in_every_language() {
        // fmt() substitutes named variables — a language may reorder
        // them, but none may be lost or invented.
        for lang in Lang::ALL {
            for (key, en_value) in en() {
                let translated = tables()[&lang].get(key).expect("key parity");
                let mut expected = placeholders(en_value);
                let mut actual = placeholders(translated);
                expected.sort();
                actual.sort();
                assert_eq!(
                    expected,
                    actual,
                    "{}: {key} placeholders differ",
                    lang.code()
                );
            }
        }
    }

    #[test]
    fn language_codes_round_trip() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_str(lang.code()), lang);
            assert_eq!(Lang::from_str(lang.native_name()), Lang::En);
        }
        assert_eq!(Lang::from_str("unknown"), Lang::En);
    }

    #[test]
    fn translation_and_formatting_work() {
        assert_eq!(tr(Lang::En, "nav.home"), "Home");
        assert_eq!(tr(Lang::Ru, "nav.home"), "Главная");
        assert_eq!(tr(Lang::De, "nav.home"), "Start");
        assert_eq!(tr(Lang::En, "no.such.key"), "");
        let filled = fmt(tr(Lang::En, "history.files"), &[("n", "3".to_string())]);
        assert_eq!(filled, "3 files");
    }
}

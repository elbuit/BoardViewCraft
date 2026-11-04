//! Internationalization (i18n) module for BoardViewCraft following TSV catalog conventions.
//! Supports English, Spanish, Catalan, French, German, and Italian.

use std::collections::HashMap;
use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    Spanish,
    Catalan,
    French,
    German,
    Italian,
}

impl Language {
    pub const ALL: [Language; 6] = [
        Language::English,
        Language::Spanish,
        Language::Catalan,
        Language::French,
        Language::German,
        Language::Italian,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Español",
            Language::Catalan => "Català",
            Language::French => "Français",
            Language::German => "Deutsch",
            Language::Italian => "Italiano",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
            Language::Catalan => "ca",
            Language::French => "fr",
            Language::German => "de",
            Language::Italian => "it",
        }
    }

    /// A language by its exact code (case-insensitive), e.g. `es`.
    pub fn from_code(code: &str) -> Option<Language> {
        Language::ALL.into_iter().find(|l| l.code().eq_ignore_ascii_case(code))
    }

    /// A language by its serialized variant name (`"Spanish"`), as written by version 1
    /// preference files.
    fn from_variant_name(name: &str) -> Option<Language> {
        Language::ALL.into_iter().find(|l| format!("{l:?}").eq_ignore_ascii_case(name))
    }
}

/// The `language` preference value that follows the system locale.
pub const AUTO: &str = "auto";

/// The user's language preference: follow the system locale, or a fixed language.
///
/// Serialized as `"auto"` or the language variant name (`"Spanish"`); language codes (`"es"`)
/// are accepted too. Anything unknown (e.g. a language from a newer version) means Auto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum LanguagePref {
    #[default]
    Auto,
    Explicit(Language),
}

impl LanguagePref {
    /// The language the UI should use for this preference.
    pub fn resolve(self) -> Language {
        match self {
            LanguagePref::Auto => system_lang(),
            LanguagePref::Explicit(lang) => lang,
        }
    }
}

impl From<String> for LanguagePref {
    fn from(value: String) -> Self {
        Language::from_code(&value)
            .or_else(|| Language::from_variant_name(&value))
            .map_or(LanguagePref::Auto, LanguagePref::Explicit)
    }
}

impl From<LanguagePref> for String {
    fn from(pref: LanguagePref) -> Self {
        match pref {
            LanguagePref::Auto => AUTO.to_string(),
            LanguagePref::Explicit(lang) => format!("{lang:?}"),
        }
    }
}

/// Candidate language codes for a locale tag, most specific first: `ca_ES.UTF-8@valencia` →
/// `ca-es`, `ca`.
fn candidates(tag: &str) -> Vec<String> {
    let base = tag.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_ascii_lowercase();
    let parts: Vec<&str> = base.split('-').filter(|p| !p.is_empty()).collect();
    (1..=parts.len()).rev().map(|n| parts[..n].join("-")).collect()
}

/// The supported language for a locale tag such as `es_ES.UTF-8` or `es-ES`; `None` if the
/// language isn't supported. `C`/`POSIX` mean English.
pub fn lang_from_tag(tag: &str) -> Option<Language> {
    let cands = candidates(tag);
    if matches!(cands.first().map(String::as_str), Some("c" | "posix")) {
        return Some(Language::English);
    }
    cands.iter().find_map(|c| Language::from_code(c))
}

/// The system language (cached). English when it can't be determined.
pub fn system_lang() -> Language {
    // Unit tests assert English labels whatever the developer's locale is.
    if cfg!(test) {
        return Language::English;
    }
    static SYSTEM: OnceLock<Language> = OnceLock::new();
    *SYSTEM.get_or_init(detect_system_lang)
}

fn detect_system_lang() -> Language {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(l) = std::env::var(var).ok().and_then(|v| language_override(&v)) {
            return l;
        }
    }
    // Apps started from the Finder don't inherit LANG: use the macOS preferred-languages list.
    // The absolute path keeps a `defaults` earlier on PATH from running; any failure means English.
    #[cfg(target_os = "macos")]
    if let Ok(out) = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLanguages"]).output()
        && out.status.success()
        && let Some(l) = first_supported(&String::from_utf8_lossy(&out.stdout))
    {
        return l;
    }
    #[cfg(target_os = "windows")]
    if let Some(l) = windows_ui_language().as_deref().and_then(first_supported) {
        return l;
    }
    Language::English
}

/// A named locale can override the display language. On Windows, the neutral C/POSIX locale
/// inherited from a launcher says nothing about the user's UI language.
fn language_override(tag: &str) -> Option<Language> {
    let tag = tag.trim();
    if cfg!(target_os = "windows") && matches!(candidates(tag).first().map(String::as_str), Some("c" | "posix")) {
        return None;
    }
    lang_from_tag(tag)
}

/// The Windows display languages in preference order, one tag per line. `sys-locale` asks
/// Windows directly (`GetUserPreferredUILanguages`), so no process is started.
#[cfg(target_os = "windows")]
fn windows_ui_language() -> Option<String> {
    let tags: Vec<String> = sys_locale::get_locales().collect();
    (!tags.is_empty()).then(|| tags.join("\n"))
}

/// The first supported language in a macOS `defaults read` list or a Windows UI-language list.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
fn first_supported(list: &str) -> Option<Language> {
    list.split(['(', ')', ',', '"', '\n']).map(str::trim).filter(|s| !s.is_empty()).find_map(lang_from_tag)
}

type Catalog = HashMap<&'static str, &'static str>;

struct CatalogEntry {
    code: &'static str,
    source: &'static str,
    catalog: OnceLock<Catalog>,
}

static CATALOGS: [CatalogEntry; 6] = [
    CatalogEntry {
        code: "en",
        source: include_str!("i18n/en.tsv"),
        catalog: OnceLock::new(),
    },
    CatalogEntry {
        code: "es",
        source: include_str!("i18n/es.tsv"),
        catalog: OnceLock::new(),
    },
    CatalogEntry {
        code: "ca",
        source: include_str!("i18n/ca.tsv"),
        catalog: OnceLock::new(),
    },
    CatalogEntry {
        code: "fr",
        source: include_str!("i18n/fr.tsv"),
        catalog: OnceLock::new(),
    },
    CatalogEntry {
        code: "de",
        source: include_str!("i18n/de.tsv"),
        catalog: OnceLock::new(),
    },
    CatalogEntry {
        code: "it",
        source: include_str!("i18n/it.tsv"),
        catalog: OnceLock::new(),
    },
];

fn parse_catalog(source: &'static str) -> Catalog {
    let mut map = HashMap::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&'static str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let key = parts[1].trim();
            let val = parts[2].trim();
            map.insert(key, val);
        } else if parts.len() == 2 {
            let key = parts[0].trim();
            let val = parts[1].trim();
            map.insert(key, val);
        }
    }
    map
}

fn get_catalog(lang_code: &str) -> &'static Catalog {
    let entry = CATALOGS
        .iter()
        .find(|c| c.code == lang_code)
        .unwrap_or(&CATALOGS[0]);
    entry.catalog.get_or_init(|| parse_catalog(entry.source))
}

/// Translates a given key into the target language.
pub fn tr(lang: Language, key: &'static str) -> &'static str {
    let cat = get_catalog(lang.code());
    cat.get(key).copied().unwrap_or(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i18n_catalogs_loaded() {
        assert_eq!(tr(Language::English, "app.title"), "BoardViewCraft - Pure Rust Board Viewer");
        assert_eq!(tr(Language::Spanish, "app.title"), "BoardViewCraft - Visor de Placas en Rust");
        assert_eq!(tr(Language::Catalan, "app.title"), "BoardViewCraft - Visor de Plaques en Rust");
        assert_eq!(tr(Language::French, "app.title"), "BoardViewCraft - Visionneuse de circuits imprimés en Rust");
        assert_eq!(tr(Language::German, "app.title"), "BoardViewCraft - Board-Viewer in reinem Rust");
        assert_eq!(tr(Language::Italian, "app.title"), "BoardViewCraft - Visualizzatore di circuiti stampati in Rust");
    }

    #[test]
    fn test_every_catalog_has_all_keys() {
        let en_cat = get_catalog("en");
        assert_eq!(en_cat.len(), 189);
        for lang in &["es", "ca", "fr", "de", "it"] {
            let cat = get_catalog(lang);
            assert_eq!(cat.len(), 189, "Catalog {lang} missing entries");
            for key in en_cat.keys() {
                assert!(cat.contains_key(key), "Catalog {lang} missing key: {key}");
            }
        }
    }

    #[test]
    fn every_shortcut_action_has_an_english_label() {
        let en_cat = get_catalog("en");
        for action in crate::keymap::Action::ALL {
            assert!(en_cat.contains_key(action.i18n_key()), "missing {}", action.i18n_key());
        }
    }

    #[test]
    fn test_lang_from_tag_maps_posix_locales() {
        assert_eq!(lang_from_tag("es_ES.UTF-8"), Some(Language::Spanish));
        assert_eq!(lang_from_tag("ca_ES@valencia"), Some(Language::Catalan));
        assert_eq!(lang_from_tag("fr-CA"), Some(Language::French));
        assert_eq!(lang_from_tag("de_DE"), Some(Language::German));
        assert_eq!(lang_from_tag("it"), Some(Language::Italian));
        assert_eq!(lang_from_tag("en_GB.UTF-8"), Some(Language::English));
        assert_eq!(lang_from_tag("C"), Some(Language::English));
        assert_eq!(lang_from_tag("C.UTF-8"), Some(Language::English));
        assert_eq!(lang_from_tag("POSIX"), Some(Language::English));
        assert_eq!(lang_from_tag("pt_BR.UTF-8"), None);
        assert_eq!(lang_from_tag("xx_YY"), None);
        assert_eq!(lang_from_tag(""), None);
    }

    #[test]
    fn test_candidates_most_specific_first() {
        assert_eq!(candidates("ca_ES.UTF-8@valencia"), vec!["ca-es".to_string(), "ca".to_string()]);
        assert!(candidates("").is_empty());
    }

    #[test]
    fn test_first_supported_reads_macos_language_list() {
        let list = "(\n    \"pt-BR\",\n    \"de-DE\",\n    \"en-US\"\n)\n";
        assert_eq!(first_supported(list), Some(Language::German));
        assert_eq!(first_supported("(\n    \"ja-JP\"\n)"), None);
    }

    #[test]
    fn test_language_from_code() {
        for l in Language::ALL {
            assert_eq!(Language::from_code(l.code()), Some(l));
        }
        assert_eq!(Language::from_code("ES"), Some(Language::Spanish));
        assert_eq!(Language::from_code("pt"), None);
    }

    #[test]
    fn test_from_pref_auto_and_explicit() {
        // `system_lang()` is pinned to English under `cfg(test)`.
        let from_pref = |pref: &str| LanguagePref::from(pref.to_string()).resolve();
        assert_eq!(from_pref(AUTO), Language::English);
        assert_eq!(from_pref("unknown"), Language::English);
        assert_eq!(from_pref("es"), Language::Spanish);
        assert_eq!(from_pref("Italian"), Language::Italian);
        assert_eq!(LanguagePref::Auto.resolve(), Language::English);
        assert_eq!(LanguagePref::Explicit(Language::Catalan).resolve(), Language::Catalan);
    }

    #[test]
    fn test_language_pref_serde() {
        let parse = |json: &str| serde_json::from_str::<LanguagePref>(json).unwrap();
        assert_eq!(parse("\"auto\""), LanguagePref::Auto);
        assert_eq!(parse("\"English\""), LanguagePref::Explicit(Language::English));
        assert_eq!(parse("\"Spanish\""), LanguagePref::Explicit(Language::Spanish));
        assert_eq!(parse("\"de\""), LanguagePref::Explicit(Language::German));
        assert_eq!(parse("\"Klingon\""), LanguagePref::Auto);
        for pref in [LanguagePref::Auto, LanguagePref::Explicit(Language::French)] {
            let json = serde_json::to_string(&pref).unwrap();
            assert_eq!(parse(&json), pref);
        }
        assert_eq!(serde_json::to_string(&LanguagePref::Auto).unwrap(), "\"auto\"");
        assert_eq!(LanguagePref::default(), LanguagePref::Auto);
    }
}

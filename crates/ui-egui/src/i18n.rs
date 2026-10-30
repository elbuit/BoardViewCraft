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
        assert_eq!(en_cat.len(), 128);
        for lang in &["es", "ca", "fr", "de", "it"] {
            let cat = get_catalog(lang);
            assert_eq!(cat.len(), 128, "Catalog {lang} missing entries");
            for key in en_cat.keys() {
                assert!(cat.contains_key(key), "Catalog {lang} missing key: {key}");
            }
        }
    }
}

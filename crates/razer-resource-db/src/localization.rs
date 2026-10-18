//! Installed locale dictionaries are loaded once before views are constructed.
use anyhow::{Context as _, ensure};
use rusqlite::Connection;
use std::{collections::BTreeMap, sync::OnceLock};

/// Validated immutable words. Locale selection remains with the UI language owner.
pub struct DictionarySnapshot {
    languages: BTreeMap<String, BTreeMap<String, String>>,
}

impl DictionarySnapshot {
    /// Exact dictionary lookup; callers apply their source language fallback policy.
    pub fn get(&self, locale: &str, key: &str) -> Option<&str> {
        self.languages.get(locale)?.get(key).map(String::as_str)
    }

    pub fn locales(&self) -> impl Iterator<Item = &str> {
        self.languages.keys().map(String::as_str)
    }
}

pub fn snapshot() -> anyhow::Result<&'static DictionarySnapshot> {
    static SNAPSHOT: OnceLock<Result<DictionarySnapshot, String>> = OnceLock::new();
    SNAPSHOT
        .get_or_init(|| super::read_catalog(read).map_err(|error| format!("{error:#}")))
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

fn read(connection: &Connection) -> anyhow::Result<DictionarySnapshot> {
    let mut languages = BTreeMap::new();
    let mut locales = connection.prepare("SELECT id, position FROM ui_locale ORDER BY position")?;
    let mut entries = connection.prepare(
        "SELECT key, value, position FROM ui_translation WHERE locale=? ORDER BY position",
    )?;
    let mut locale_rows = locales.query([])?;
    while let Some(row) = locale_rows.next()? {
        let locale: String = row.get(0)?;
        let position: usize = row.get(1)?;
        ensure!(
            position == languages.len() && position < 128,
            "Invalid installed locale order"
        );
        ensure!(
            !locale.is_empty()
                && locale.len() <= 64
                && locale.split('-').all(|part| !part.is_empty()
                    && part.bytes().all(|byte| byte.is_ascii_alphanumeric())),
            "Invalid installed locale identity"
        );
        let mut words = BTreeMap::new();
        let mut word_rows = entries.query([&locale])?;
        while let Some(row) = word_rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            let position: usize = row.get(2)?;
            ensure!(
                position == words.len() && position < 1_000_000,
                "Invalid installed translation order"
            );
            ensure!(
                !key.is_empty() && key.len() <= 4096 && value.len() <= 1024 * 1024,
                "Invalid installed translation length"
            );
            ensure!(
                words.insert(key, value).is_none(),
                "Duplicate installed translation key"
            );
        }
        ensure!(!words.is_empty(), "Installed locale has no translations");
        ensure!(
            languages.insert(locale, words).is_none(),
            "Duplicate installed locale"
        );
    }
    ensure!(
        !languages
            .get("en")
            .context("Missing installed fallback locale")?
            .is_empty(),
        "Installed fallback locale is empty"
    );
    Ok(DictionarySnapshot { languages })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE ui_locale(id TEXT, position INTEGER);
             CREATE TABLE ui_translation(locale TEXT, position INTEGER, key TEXT, value TEXT);
             INSERT INTO ui_locale VALUES ('en',0),('zh-CN',1);
             INSERT INTO ui_translation VALUES
               ('en',0,'RESET','RESET'), ('en',1,'SOURCE.BODY','first\nsecond'),
               ('zh-CN',0,'RESET','重置'), ('zh-CN',1,'EMPTY','');",
            )
            .unwrap();
        connection
    }

    #[test]
    fn exact_snapshot_keeps_namespaces_newlines_empty_and_identical_values() {
        let words = read(&fixture()).unwrap();
        assert_eq!(words.get("en", "RESET"), Some("RESET"));
        assert_eq!(words.get("en", "SOURCE.BODY"), Some("first\nsecond"));
        assert_eq!(words.get("zh-CN", "EMPTY"), Some(""));
        assert_eq!(words.get("zh-CN", "SOURCE.BODY"), None);
        assert_eq!(words.get("unknown", "RESET"), None);
        assert_eq!(words.locales().collect::<Vec<_>>(), ["en", "zh-CN"]);
    }

    #[test]
    fn missing_fallback_or_noncontiguous_and_duplicate_rows_are_rejected() {
        for statement in [
            "DELETE FROM ui_locale WHERE id='en'",
            "UPDATE ui_locale SET position=3 WHERE id='zh-CN'",
            "UPDATE ui_translation SET position=3 WHERE locale='en' AND position=1",
            "INSERT INTO ui_translation VALUES ('en',2,'RESET','other')",
            "DELETE FROM ui_translation WHERE locale='en'",
        ] {
            let connection = fixture();
            connection.execute_batch(statement).unwrap();
            assert!(read(&connection).is_err(), "{statement}");
        }
    }
}

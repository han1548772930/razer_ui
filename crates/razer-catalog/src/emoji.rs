//! Source-ordered emoji profiles decoded once from the packaged resource database.
use anyhow::{Context as _, ensure};
use std::{collections::BTreeMap, sync::OnceLock};

/// One source search entry. The name is the source's English search text.
#[derive(Debug)]
pub struct EmojiSearchEntry {
    emoji: String,
    name: String,
}

impl EmojiSearchEntry {
    pub fn emoji(&self) -> &str {
        &self.emoji
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Immutable source order over shared entries, including source duplicates.
pub struct EmojiSearchTable {
    entries: Vec<EmojiSearchEntry>,
    order: Vec<usize>,
}

impl EmojiSearchTable {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &EmojiSearchEntry> + ExactSizeIterator {
        self.order.iter().map(|&ix| &self.entries[ix])
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatasetId {
    Mapping,
    MappingWithoutExclusions,
    Macro,
}

impl DatasetId {
    const ALL: [Self; 3] = [Self::Mapping, Self::MappingWithoutExclusions, Self::Macro];

    fn name(self) -> &'static str {
        match self {
            Self::Mapping => "mapping",
            Self::MappingWithoutExclusions => "mapping_without_exclusions",
            Self::Macro => "macro",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Mapping => 0,
            Self::MappingWithoutExclusions => 1,
            Self::Macro => 2,
        }
    }
}

pub struct EmojiGroup {
    values: Vec<String>,
    label: Option<String>,
    name: Option<String>,
}

impl EmojiGroup {
    pub fn values(&self) -> &[String] {
        &self.values
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

pub struct EmojiTab {
    emoji: String,
    name: String,
    position: Option<usize>,
}

impl EmojiTab {
    pub fn emoji(&self) -> &str {
        &self.emoji
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn position(&self) -> Option<usize> {
        self.position
    }
}

/// Profiles retain their own labels, category order and Windows exclusion rule.
pub struct EmojiDataset {
    groups: Vec<EmojiGroup>,
    tabs: Vec<EmojiTab>,
    variants: BTreeMap<String, Vec<String>>,
    exclusions: Vec<String>,
}

impl EmojiDataset {
    pub fn groups(&self) -> &[EmojiGroup] {
        &self.groups
    }
    pub fn tabs(&self) -> &[EmojiTab] {
        &self.tabs
    }
    pub fn variants(&self) -> &BTreeMap<String, Vec<String>> {
        &self.variants
    }
    pub fn exclusions(&self) -> &[String] {
        &self.exclusions
    }
}

struct Catalog {
    search: EmojiSearchTable,
    datasets: Vec<EmojiDataset>,
}

fn catalog() -> anyhow::Result<&'static Catalog> {
    static CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        razer_resource_db::read_catalog(|connection| {
            let mut entries = Vec::new();
            let mut statement = connection.prepare("SELECT id, emoji, name FROM emoji_entry ORDER BY id")?;
            for row in statement.query_map([], |row| Ok((row.get::<_, usize>(0)?, EmojiSearchEntry {
                emoji: row.get(1)?, name: row.get(2)?,
            })))? {
                let (index, entry) = row?;
                ensure!(index == entries.len(), "Emoji entry IDs are not contiguous");
                entries.push(entry);
            }
            let mut order = Vec::new();
            let mut statement = connection.prepare("SELECT position, entry_id FROM emoji_search ORDER BY position")?;
            for row in statement.query_map([], |row| Ok((row.get::<_, usize>(0)?, row.get::<_, usize>(1)?)))? {
                let (position, entry) = row?;
                ensure!(position == order.len() && entry < entries.len(), "Invalid emoji search sequence");
                order.push(entry);
            }
            ensure!(!order.is_empty(), "Emoji search vocabulary is empty");
            let mut datasets = Vec::new();
            for id in DatasetId::ALL {
                let (variant_set, exclusion_set, rule): (u32, u32, String) = connection.query_row(
                    "SELECT variant_set, exclusion_set, exclusion_rule FROM emoji_dataset WHERE id=?1", [id.name()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
                let macro_profile = id == DatasetId::Macro;
                ensure!(rule == if macro_profile { "windows_11" } else { "without_windows_10" }, "Invalid emoji exclusion branch");
                let mut groups = Vec::new();
                let mut statement = connection.prepare(
                    "SELECT position, values_id, label, name FROM emoji_group WHERE dataset_id=?1 ORDER BY position",
                )?;
                for row in statement.query_map([id.name()], |row| Ok((row.get::<_, usize>(0)?, row.get::<_, u32>(1)?,
                    row.get::<_, Option<String>>(2)?, row.get::<_, Option<String>>(3)?)))? {
                    let (position, values_id, label, name) = row?;
                    ensure!(position == groups.len(), "Emoji group positions are not contiguous");
                    ensure!(label.is_some() == macro_profile && name.is_some() == macro_profile, "Invalid emoji group label branch");
                    let mut values = Vec::new();
                    let mut items = connection.prepare("SELECT position, value FROM emoji_group_value WHERE list_id=?1 ORDER BY position")?;
                    for item in items.query_map([values_id], |row| Ok((row.get::<_, usize>(0)?, row.get::<_, String>(1)?)))? {
                        let (position, item) = item?;
                        ensure!(position == values.len(), "Invalid emoji group value order");
                        values.push(item);
                    }
                    groups.push(EmojiGroup { values, label, name });
                }
                let mut tabs = Vec::new();
                let mut statement = connection.prepare(
                    "SELECT position, emoji, name, target_position FROM emoji_tab WHERE dataset_id=?1 ORDER BY position",
                )?;
                for row in statement.query_map([id.name()], |row| Ok((row.get::<_, usize>(0)?, EmojiTab {
                    emoji: row.get(1)?, name: row.get(2)?, position: row.get(3)?,
                })))? {
                    let (position, tab) = row?;
                    ensure!(position == tabs.len() && tab.position.is_some() == macro_profile, "Invalid emoji tab branch");
                    if let Some(target) = tab.position { ensure!(target < groups.len(), "Invalid emoji tab target"); }
                    tabs.push(tab);
                }
                ensure!(!groups.is_empty() && groups.len() == tabs.len(), "Invalid emoji category count");
                let mut variants: BTreeMap<String, Vec<String>> = BTreeMap::new();
                let mut statement = connection.prepare("SELECT base, position, value FROM emoji_variant WHERE set_id=?1 ORDER BY base, position")?;
                for row in statement.query_map([variant_set], |row| Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?, row.get::<_, String>(2)?)))? {
                    let (base, position, value) = row?;
                    let items = variants.entry(base).or_default();
                    ensure!(position == items.len(), "Invalid emoji variant order");
                    items.push(value);
                }
                let mut exclusions = Vec::new();
                let mut statement = connection.prepare("SELECT position, value FROM emoji_exclusion WHERE set_id=?1 ORDER BY position")?;
                for row in statement.query_map([exclusion_set], |row| Ok((row.get::<_, usize>(0)?, row.get::<_, String>(1)?)))? {
                    let (position, value) = row?;
                    ensure!(position == exclusions.len(), "Invalid emoji exclusion order");
                    exclusions.push(value);
                }
                datasets.push(EmojiDataset { groups, tabs, variants, exclusions });
            }
            Ok(Catalog { search: EmojiSearchTable { entries, order }, datasets })
        }).map_err(|error| format!("{error:#}"))
    }).as_ref().map_err(|error| anyhow::anyhow!("{error}")).context("Cannot read packaged emoji profiles")
}

/// Startup validates the complete catalog before any infallible UI constructors run.
pub fn validate() -> anyhow::Result<()> {
    catalog()?;
    Ok(())
}

pub fn search() -> &'static EmojiSearchTable {
    &catalog()
        .expect("packaged emoji catalog must validate before UI construction")
        .search
}

pub fn dataset(id: DatasetId) -> &'static EmojiDataset {
    &catalog()
        .expect("packaged emoji catalog must validate before UI construction")
        .datasets[id.index()]
}

#[cfg(test)]
mod tests {
    use super::{DatasetId, dataset, search};
    use std::collections::BTreeSet;

    #[test]
    fn source_order_retains_duplicates_and_zwj_sequences() {
        let table = search();
        assert_eq!(table.len(), 1936);
        assert_eq!(table.iter().next().unwrap().emoji(), "😀");
        assert_eq!(table.iter().next().unwrap().name(), "Grinning Face");
        assert_eq!(
            table
                .iter()
                .filter(|entry| entry.emoji() == "🕴" && entry.name() == "Person in Suit Levitating")
                .count(),
            2
        );
        assert!(table.iter().any(|entry| entry.emoji() == "🧑🏻‍🤝‍🧑🏼"));
        assert!(
            table
                .iter()
                .any(|entry| entry.emoji().is_empty() && entry.name() == "Man")
        );
        assert_eq!(
            table
                .iter()
                .map(|entry| (entry.emoji(), entry.name()))
                .collect::<BTreeSet<_>>()
                .len(),
            1857
        );
    }

    #[test]
    fn repeated_source_entries_share_storage() {
        let entries = search()
            .iter()
            .filter(|entry| entry.emoji() == "🕴" && entry.name() == "Person in Suit Levitating")
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 2);
        assert!(std::ptr::eq(entries[0], entries[1]));
        assert!(std::ptr::eq(search(), search()));
    }

    #[test]
    fn profile_specific_categories_tabs_and_exclusions_are_retained() {
        let mapping = dataset(DatasetId::Mapping);
        let alternate = dataset(DatasetId::MappingWithoutExclusions);
        let macros = dataset(DatasetId::Macro);
        assert_eq!(mapping.groups().len(), 8);
        assert_eq!(mapping.exclusions().len(), 5);
        assert!(alternate.exclusions().is_empty());
        assert_eq!(mapping.tabs()[0].name(), "Smileys");
        assert_eq!(macros.tabs()[0].name(), "TEXT_EMOJI_SMILEYS");
        assert_eq!(macros.tabs()[0].position(), Some(0));
        assert_eq!(macros.groups()[0].label(), Some("SMILEYS"));
        assert_ne!(mapping.groups()[2].values(), macros.groups()[2].values());
        assert_eq!(mapping.variants(), macros.variants());
    }
}

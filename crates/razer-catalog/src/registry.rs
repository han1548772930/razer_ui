//! Immutable product navigation loaded once from the installed resource catalog.
use anyhow::{Context as _, ensure};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProductPageId {
    product_id: u32,
    key: &'static str,
}
impl ProductPageId {
    pub fn product_id(self) -> u32 {
        self.product_id
    }
    pub fn key(self) -> &'static str {
        self.key
    }
}

/// A localization key or literal; it never identifies a renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProductPageKind(&'static str);
impl ProductPageKind {
    pub fn key(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductPageRole {
    Page,
    Help,
    StandaloneMode,
}

#[derive(Debug)]
pub struct ProductPage {
    id: ProductPageId,
    kind: ProductPageKind,
    role: ProductPageRole,
    help_selector: usize,
}
impl ProductPage {
    pub fn id(&self) -> ProductPageId {
        self.id
    }
    pub fn kind(&self) -> ProductPageKind {
        self.kind
    }
    pub fn label(&self) -> String {
        razer_i18n::t(self.kind.0)
    }
    pub fn role(&self) -> ProductPageRole {
        self.role
    }
    /// Existing help records use this selector for content and local-scope filtering.
    /// It is independent of source provenance and runtime compatibility checks.
    pub fn offset(&self) -> usize {
        self.help_selector
    }
}

#[derive(Debug)]
pub struct ProductNavigation {
    key: &'static str,
    display_mode: &'static str,
    primary: bool,
    pages: Vec<ProductPage>,
}
impl ProductNavigation {
    pub fn key(&self) -> &'static str {
        self.key
    }
    pub fn display_mode(&self) -> &'static str {
        self.display_mode
    }
    pub fn is_primary(&self) -> bool {
        self.primary
    }
    pub fn pages(&'static self) -> &'static [ProductPage] {
        &self.pages
    }
    pub fn page(&'static self, id: ProductPageId) -> Option<&'static ProductPage> {
        self.pages.iter().find(|page| page.id == id)
    }
}

#[derive(Debug)]
pub struct RegisteredProduct {
    id: u32,
    name: &'static str,
    categories: Vec<&'static str>,
    edition_ids: &'static [u32],
    navigations: Vec<ProductNavigation>,
}
impl RegisteredProduct {
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn categories(&'static self) -> &'static [&'static str] {
        &self.categories
    }
    pub fn edition_ids(&self) -> &'static [u32] {
        self.edition_ids
    }
    pub fn navigations(&'static self) -> &'static [ProductNavigation] {
        &self.navigations
    }
    pub fn primary_navigation(&'static self) -> Option<&'static ProductNavigation> {
        self.navigations
            .iter()
            .find(|navigation| navigation.primary)
    }
    pub fn page(&'static self, id: ProductPageId) -> Option<&'static ProductPage> {
        if id.product_id != self.id {
            return None;
        }
        self.navigations
            .iter()
            .find_map(|navigation| navigation.page(id))
    }
}

struct StoredPage {
    key: String,
    kind: String,
    role: ProductPageRole,
    help_selector: usize,
}
struct StoredNavigation {
    key: String,
    display_mode: String,
    primary: bool,
    pages: Vec<StoredPage>,
}
struct StoredProduct {
    id: u32,
    name: String,
    categories: Vec<String>,
    edition_ids: Vec<u32>,
    navigations: Vec<StoredNavigation>,
}

fn load() -> anyhow::Result<Vec<StoredProduct>> {
    razer_resource_db::read_catalog(|connection| {
        let mut statement = connection.prepare("SELECT id, name FROM ui_product ORDER BY id")?;
        let mut products = statement
            .query_map([], |row| {
                Ok(StoredProduct {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    categories: Vec::new(),
                    edition_ids: Vec::new(),
                    navigations: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        ensure!(!products.is_empty(), "Installed product directory is empty");
        ensure!(
            products.windows(2).all(|rows| rows[0].id < rows[1].id),
            "Duplicate product identity"
        );

        let mut statement = connection.prepare("SELECT product_id, position, value FROM ui_product_category ORDER BY product_id, position")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let pid: u32 = row.get(0)?;
            let position: usize = row.get(1)?;
            let value: String = row.get(2)?;
            let index = products
                .binary_search_by_key(&pid, |product| product.id)
                .ok()
                .context("Category has no product")?;
            let categories = &mut products[index].categories;
            ensure!(
                position == categories.len() && !value.is_empty() && !categories.contains(&value),
                "Invalid product category order or identity"
            );
            categories.push(value);
        }
        let mut statement = connection.prepare("SELECT product_id, position, edition_id FROM ui_product_edition ORDER BY product_id, position")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let pid: u32 = row.get(0)?;
            let position: usize = row.get(1)?;
            let edition: u32 = row.get(2)?;
            let index = products
                .binary_search_by_key(&pid, |product| product.id)
                .ok()
                .context("Edition has no product")?;
            let editions = &mut products[index].edition_ids;
            ensure!(
                position == editions.len() && !editions.contains(&edition),
                "Invalid product edition order or identity"
            );
            editions.push(edition);
        }
        let mut statement = connection.prepare("SELECT product_id, position, key, display_mode, is_primary FROM ui_navigation ORDER BY product_id, position")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let pid: u32 = row.get(0)?;
            let position: usize = row.get(1)?;
            let key: String = row.get(2)?;
            let display_mode: String = row.get(3)?;
            let primary: u8 = row.get(4)?;
            let index = products
                .binary_search_by_key(&pid, |product| product.id)
                .ok()
                .context("Navigation has no product")?;
            let navigations = &mut products[index].navigations;
            ensure!(
                position == navigations.len()
                    && !key.is_empty()
                    && !display_mode.is_empty()
                    && primary <= 1,
                "Invalid product navigation fields or order"
            );
            navigations.push(StoredNavigation {
                key,
                display_mode,
                primary: primary == 1,
                pages: Vec::new(),
            });
        }
        let mut statement = connection.prepare("SELECT navigation.product_id, navigation.position, page.navigation_key, page.position, page.key, page.kind, page.role, page.help_selector FROM ui_page AS page JOIN ui_navigation AS navigation ON navigation.key=page.navigation_key ORDER BY navigation.product_id, navigation.position, page.position")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let pid: u32 = row.get(0)?;
            let navigation_position: usize = row.get(1)?;
            let navigation_key: String = row.get(2)?;
            let position: usize = row.get(3)?;
            let key: String = row.get(4)?;
            let kind: String = row.get(5)?;
            let role: String = row.get(6)?;
            let help_selector = row.get(7)?;
            let index = products
                .binary_search_by_key(&pid, |product| product.id)
                .ok()
                .context("Page has no product")?;
            let navigation = products[index]
                .navigations
                .get_mut(navigation_position)
                .context("Page has no navigation")?;
            ensure!(
                navigation.key == navigation_key
                    && position == navigation.pages.len()
                    && !key.is_empty()
                    && !kind.is_empty(),
                "Invalid page identity or order"
            );
            let role = match role.as_str() {
                "Page" => ProductPageRole::Page,
                "Help" => ProductPageRole::Help,
                "StandaloneMode" => ProductPageRole::StandaloneMode,
                _ => anyhow::bail!("Unknown product page role: {role}"),
            };
            navigation.pages.push(StoredPage {
                key,
                kind,
                role,
                help_selector,
            });
        }
        for product in &products {
            ensure!(
                product.id > 0
                    && !product.name.is_empty()
                    && !product.categories.is_empty()
                    && !product.edition_ids.is_empty(),
                "Product directory has incomplete identity fields"
            );
            ensure!(
                product
                    .navigations
                    .iter()
                    .filter(|navigation| navigation.primary)
                    .count()
                    == 1,
                "Product has no unique primary navigation"
            );
            ensure!(
                product
                    .navigations
                    .iter()
                    .all(|navigation| !navigation.pages.is_empty()),
                "Product navigation has no pages"
            );
        }
        Ok(products)
    })
}

fn snapshot() -> anyhow::Result<&'static [RegisteredProduct]> {
    // Stored rows own every string for the process lifetime. Public descriptors
    // borrow these rows once, preserving Copy IDs without leaking allocations.
    static ROWS: OnceLock<Result<Vec<StoredProduct>, String>> = OnceLock::new();
    static SNAPSHOT: OnceLock<Vec<RegisteredProduct>> = OnceLock::new();
    let rows = ROWS
        .get_or_init(|| load().map_err(|error| format!("{error:#}")))
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(SNAPSHOT.get_or_init(|| {
        rows.iter()
            .map(|product| RegisteredProduct {
                id: product.id,
                name: &product.name,
                categories: product.categories.iter().map(String::as_str).collect(),
                edition_ids: &product.edition_ids,
                navigations: product
                    .navigations
                    .iter()
                    .map(|navigation| ProductNavigation {
                        key: &navigation.key,
                        display_mode: &navigation.display_mode,
                        primary: navigation.primary,
                        pages: navigation
                            .pages
                            .iter()
                            .map(|page| ProductPage {
                                id: ProductPageId {
                                    product_id: product.id,
                                    key: &page.key,
                                },
                                kind: ProductPageKind(&page.kind),
                                role: page.role,
                                help_selector: page.help_selector,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect()
    }))
}

/// Validate and retain all product branches before constructing any window.
pub fn initialize_registry() -> anyhow::Result<()> {
    snapshot().map(|_| ())
}
pub fn registry() -> &'static [RegisteredProduct] {
    snapshot().expect("Installed product directory must be initialized before use")
}
pub fn registered(pid: u32) -> Option<&'static RegisteredProduct> {
    let products = registry();
    products
        .binary_search_by_key(&pid, |product| product.id)
        .ok()
        .map(|index| &products[index])
}

#[cfg(test)]
mod tests {
    use super::{ProductPageRole, initialize_registry, registered, registry};
    #[test]
    fn identities_and_connection_specific_pages_survive_database_loading() {
        initialize_registry().unwrap();
        assert_eq!(registry().len(), 331);
        assert!(registered(162).is_none());
        assert_eq!(registered(163).unwrap().name(), "Razer Cobra");
        assert_eq!(registered(1318).unwrap().categories(), &["AUDIO"]);
        assert_eq!(registered(3886).unwrap().edition_ids(), &[0]);
        let product = registered(756).unwrap();
        assert_eq!(product.name(), "Razer Reclusa Mini 65%");
        let primary = product.primary_navigation().unwrap();
        assert_eq!(
            primary
                .pages()
                .iter()
                .map(|page| page.kind().key())
                .collect::<Vec<_>>(),
            [
                "TAB_CUSTOMIZE",
                "ACTUATION",
                "TAB_LIGHTING",
                "TAB_POWER",
                "TAB_CALIBRATION",
                "HELP"
            ]
        );
        let ble = product
            .navigations()
            .iter()
            .find(|navigation| navigation.key().starts_with("product-756-bluetooth-"))
            .unwrap();
        assert!(
            !ble.pages()
                .iter()
                .any(|page| page.kind().key() == "TAB_CALIBRATION")
        );
        assert_eq!(
            ble.pages()[0].id().key(),
            "synapse--products--756--ui--TAB_CUSTOMIZE--337399"
        );
        assert_eq!(ble.pages().last().unwrap().role(), ProductPageRole::Help);
        assert_eq!(primary.pages().last().unwrap().offset(), 337_238);
        assert_eq!(ble.pages().last().unwrap().offset(), 337_754);
        assert_eq!(
            product
                .navigations()
                .iter()
                .find(|navigation| navigation.display_mode() == "chromaApp")
                .unwrap()
                .pages()[0]
                .kind()
                .key(),
            "TAB_LIGHTING"
        );
        for product in registry() {
            for navigation in product.navigations() {
                for page in navigation.pages() {
                    assert!(std::ptr::eq(product.page(page.id()).unwrap(), page));
                }
            }
        }
        let address = registry().as_ptr();
        initialize_registry().unwrap();
        assert_eq!(address, registry().as_ptr());
    }
}

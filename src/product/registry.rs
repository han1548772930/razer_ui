//! Source identities are independent of native adapters. A shared label never
//! licenses sharing another product's page, controls, defaults, or side effects.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ProductPageId {
    product_id: u32,
    key: &'static str,
}
impl ProductPageId {
    pub(crate) fn product_id(self) -> u32 {
        self.product_id
    }
    pub(crate) fn key(self) -> &'static str {
        self.key
    }
}

/// Original localization key/literal, preserved even when two keys translate
/// to the same title. This is a navigation name, never a renderer identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ProductPageKind(&'static str);
impl ProductPageKind {
    pub(crate) fn key(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductPageRole {
    Page,
    Help,
    StandaloneMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdapterStatus {
    /// A source route is audited; there is no claim its native UI is complete.
    SourceAudited,
    /// The previously audited workspace has an adapter; conditional UI and
    /// native service completion remain separately audited.
    ExistingAdapter,
}

#[derive(Debug)]
pub(crate) struct ProductPage {
    id: ProductPageId,
    kind: ProductPageKind,
    role: ProductPageRole,
    source_id: Option<i64>,
    component_expression: Option<&'static str>,
    component_kind: &'static str,
    extra_class: Option<&'static str>,
    offset: usize,
    adapter_status: AdapterStatus,
}
impl ProductPage {
    pub(crate) fn id(&self) -> ProductPageId {
        self.id
    }
    pub(crate) fn kind(&self) -> ProductPageKind {
        self.kind
    }
    pub(crate) fn label(&self) -> String {
        crate::i18n::t(self.kind.0)
    }
    pub(crate) fn role(&self) -> ProductPageRole {
        self.role
    }
    pub(crate) fn source_id(&self) -> Option<i64> {
        self.source_id
    }
    pub(crate) fn component_expression(&self) -> Option<&'static str> {
        self.component_expression
    }
    pub(crate) fn component_kind(&self) -> &'static str {
        self.component_kind
    }
    pub(crate) fn extra_class(&self) -> Option<&'static str> {
        self.extra_class
    }
    pub(crate) fn offset(&self) -> usize {
        self.offset
    }
    pub(crate) fn adapter_status(&self) -> AdapterStatus {
        self.adapter_status
    }
}

#[derive(Debug)]
pub(crate) struct ProductNavigation {
    key: &'static str,
    owner: &'static str,
    display_mode: &'static str,
    source: &'static str,
    source_sha256: &'static str,
    offset: usize,
    primary: bool,
    /// Exact root-selection expression or maintained inventory reachability.
    reachability: &'static str,
    pages: &'static [ProductPage],
}
impl ProductNavigation {
    pub(crate) fn key(&self) -> &'static str {
        self.key
    }
    pub(crate) fn owner(&self) -> &'static str {
        self.owner
    }
    pub(crate) fn display_mode(&self) -> &'static str {
        self.display_mode
    }
    pub(crate) fn source(&self) -> &'static str {
        self.source
    }
    pub(crate) fn source_sha256(&self) -> &'static str {
        self.source_sha256
    }
    pub(crate) fn offset(&self) -> usize {
        self.offset
    }
    pub(crate) fn is_primary(&self) -> bool {
        self.primary
    }
    pub(crate) fn reachability(&self) -> &'static str {
        self.reachability
    }
    pub(crate) fn pages(&self) -> &'static [ProductPage] {
        self.pages
    }
    pub(crate) fn page(&self, id: ProductPageId) -> Option<&'static ProductPage> {
        self.pages.iter().find(|page| page.id == id)
    }
}

#[derive(Debug)]
pub(crate) struct RegisteredProduct {
    id: u32,
    name: &'static str,
    categories: &'static [&'static str],
    edition_ids: &'static [u32],
    navigations: &'static [ProductNavigation],
}
impl RegisteredProduct {
    pub(crate) fn id(&self) -> u32 {
        self.id
    }
    pub(crate) fn name(&self) -> &'static str {
        self.name
    }
    pub(crate) fn categories(&self) -> &'static [&'static str] {
        self.categories
    }
    pub(crate) fn edition_ids(&self) -> &'static [u32] {
        self.edition_ids
    }
    pub(crate) fn navigations(&self) -> &'static [ProductNavigation] {
        self.navigations
    }
    pub(crate) fn primary_navigation(&self) -> Option<&'static ProductNavigation> {
        self.navigations.iter().find(|nav| nav.primary)
    }
    pub(crate) fn page(&self, id: ProductPageId) -> Option<&'static ProductPage> {
        self.navigations.iter().find_map(|nav| nav.page(id))
    }
}

pub(crate) fn registry() -> &'static [RegisteredProduct] {
    GENERATED
}
pub(crate) fn registered(pid: u32) -> Option<&'static RegisteredProduct> {
    GENERATED
        .binary_search_by_key(&pid, |product| product.id)
        .ok()
        .map(|ix| &GENERATED[ix])
}

const GENERATED: &[RegisteredProduct] = include!("registry_data.rs");

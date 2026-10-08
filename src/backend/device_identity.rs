//! Current host HD/_checkUSBDetail identity projection, without hardware state.
//! See docs/re/device-identity-current-contract.md for source boundaries.
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Ids {
    One(u32),
    Many(Vec<u32>),
}
impl Ids {
    /// Host HD accepts a scalar or an array; no string/number coercion.
    fn contains(&self, raw: u32) -> bool {
        match self {
            Self::One(value) => *value == raw,
            Self::Many(values) => values.contains(&raw),
        }
    }
    /// Middleware he uses strict scalar equality, not host HD.
    fn scalar_equals(&self, raw: u32) -> bool {
        matches!(self, Self::One(value) if *value == raw)
    }
}

/// Per-product `DeviceInfo` facts declared by the product's own middleware bundle.
///
/// Generated for every product by `tools/audit-middleware-device-bindings.cjs` and
/// projected by `tools/prepare-discovery-catalog.py`; no product id is special-cased.
/// The original runtime reads `claimInterface`/`dongleId`/`bleId`/`category` from here.
#[derive(Debug, Deserialize)]
struct MiddlewareFacts {
    #[serde(default)]
    dongle_id: Option<u32>,
    #[serde(default)]
    ble_id: Option<u32>,
    #[serde(default)]
    claim_interface: Option<u8>,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogProduct {
    product_id: u32,
    #[serde(default)]
    rep_id: Option<u32>,
    #[serde(default)]
    dongle_id: Option<Ids>,
    #[serde(default)]
    ble_id: Option<Ids>,
    #[serde(default)]
    xbox_id: Option<Ids>,
    #[serde(default)]
    wired_id: Option<Ids>,
    #[serde(default)]
    ps_mode_ids: Option<Vec<u32>>,
    #[serde(default)]
    monitor_port_ids: Option<Vec<u32>>,
    #[serde(default)]
    is_not_chroma_device: bool,
    #[serde(default)]
    middleware: Option<MiddlewareFacts>,
}
impl CatalogProduct {
    fn route(&self) -> u32 {
        self.rep_id.filter(|id| *id != 0).unwrap_or(self.product_id)
    }
    fn middleware_dongle(&self) -> Option<u32> {
        self.middleware.as_ref().and_then(|facts| facts.dongle_id)
    }
    fn middleware_ble(&self) -> Option<u32> {
        self.middleware.as_ref().and_then(|facts| facts.ble_id)
    }
    fn matches(&self, raw: u32) -> bool {
        self.product_id == raw
            || self.rep_id == Some(raw)
            || contains(&self.dongle_id, raw)
            || contains(&self.ble_id, raw)
            || contains(&self.xbox_id, raw)
            || contains(&self.wired_id, raw)
            || self.middleware_dongle() == Some(raw)
            || self.middleware_ble() == Some(raw)
            || self
                .ps_mode_ids
                .as_ref()
                .is_some_and(|ids| ids.contains(&raw))
    }
    fn identity(&self, raw: u32, catalog_index: usize) -> ProductIdentity {
        ProductIdentity {
            product_id: self.route(),
            real_product_id: raw,
            source_product_id: self.product_id,
            catalog_index,
            is_dongle: contains(&self.dongle_id, raw) || self.middleware_dongle() == Some(raw),
            is_ble: contains(&self.ble_id, raw) || self.middleware_ble() == Some(raw),
            is_xbox: contains(&self.xbox_id, raw),
            is_playstation: self
                .ps_mode_ids
                .as_ref()
                .is_some_and(|ids| ids.contains(&raw)),
            is_monitor: false,
            is_not_chroma_device: self.is_not_chroma_device,
        }
    }
}
fn contains(ids: &Option<Ids>, raw: u32) -> bool {
    ids.as_ref().is_some_and(|ids| ids.contains(raw))
}
fn catalog() -> &'static [CatalogProduct] {
    static CATALOG: OnceLock<Vec<CatalogProduct>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("discovery_catalog.json"))
            .expect("statically validated complete current identity catalog")
    })
}

/// Catalog facts only. No edition, layout, serial, online or readiness is inferred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductIdentity {
    pub(crate) product_id: u32,
    pub(crate) real_product_id: u32,
    pub(crate) source_product_id: u32,
    /// Preserve source row identity even when two rows route to the same product.
    pub(crate) catalog_index: usize,
    pub(crate) is_dongle: bool,
    pub(crate) is_ble: bool,
    pub(crate) is_xbox: bool,
    pub(crate) is_playstation: bool,
    pub(crate) is_monitor: bool,
    pub(crate) is_not_chroma_device: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IdentityLookup {
    Unmatched {
        raw_product_id: u32,
    },
    Unique(ProductIdentity),
    Ambiguous {
        raw_product_id: u32,
        candidates: Vec<ProductIdentity>,
    },
}
impl IdentityLookup {
    pub(crate) fn raw_product_id(&self) -> u32 {
        match self {
            Self::Unmatched { raw_product_id } | Self::Ambiguous { raw_product_id, .. } => {
                *raw_product_id
            }
            Self::Unique(identity) => identity.real_product_id,
        }
    }
    pub(crate) fn candidates(&self) -> &[ProductIdentity] {
        match self {
            Self::Unmatched { .. } => &[],
            Self::Unique(identity) => std::slice::from_ref(identity),
            Self::Ambiguous { candidates, .. } => candidates,
        }
    }
}
fn classify(raw_product_id: u32, mut candidates: Vec<ProductIdentity>) -> IdentityLookup {
    match candidates.len() {
        0 => IdentityLookup::Unmatched { raw_product_id },
        1 => IdentityLookup::Unique(candidates.remove(0)),
        _ => IdentityLookup::Ambiguous {
            raw_product_id,
            candidates,
        },
    }
}

/// The available-device branch for an actual Razer USB/HID PID, not a preview ID.
/// Callers own vendor/container validation and observations. Monitor ports require
/// their separate verified serial-bearing source; generic HID serials are not it.
pub(crate) fn lookup(raw_pid: u32) -> IdentityLookup {
    if raw_pid == 1325 {
        return classify(raw_pid, vec![]); // Current EARLY_IGNORE_PID branch.
    }
    let matches: Vec<_> = catalog()
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.matches(raw_pid))
        .collect();
    if matches.len() == 1 && matches[0].1.monitor_port_ids.is_some() {
        return classify(raw_pid, vec![]); // Host single monitorPortIds unsupported.
    }
    let multiple = matches.len() > 1;
    classify(
        raw_pid,
        matches
            .into_iter()
            .map(|(index, entry)| {
                let mut identity = entry.identity(raw_pid, index);
                if multiple {
                    // _checkUSBDetail's multi branch emits each catalog candidate with
                    // isDongle=true; it does not resolve which physical product it is.
                    identity.is_dongle = true;
                    identity.is_ble = false;
                    identity.is_xbox = false;
                    identity.is_playstation = false;
                }
                identity
            })
            .collect(),
    )
}

/// Current 34340/he: dongleId === queried productId. Preserve all candidates
/// rather than silently selecting Array.find's first row or filtering siblings.
/// `he` compares the product's own `DeviceInfo.dongleId`, which the generated
/// middleware facts carry for products the dashboard catalog omits.
pub(crate) fn lookup_receiver_peer(raw_pid: u32, match_product_id: bool) -> IdentityLookup {
    classify(
        raw_pid,
        catalog()
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                (match_product_id && entry.product_id == raw_pid)
                    || entry
                        .dongle_id
                        .as_ref()
                        .is_some_and(|ids| ids.scalar_equals(raw_pid))
                    || entry.middleware_dongle() == Some(raw_pid)
            })
            .map(|(index, entry)| {
                let mut identity = entry.identity(raw_pid, index);
                identity.product_id = entry.product_id; // he does not apply repId.
                identity.is_dongle = true;
                identity
            })
            .collect(),
    )
}

/// Source-declared HID interface number for a product, when its middleware
/// `DeviceInfo` declares one. Consumers must not hardcode an interface per product.
pub(crate) fn claim_interface(product_id: u32) -> Option<u8> {
    catalog()
        .iter()
        .find(|entry| entry.product_id == product_id)
        .and_then(|entry| entry.middleware.as_ref())
        .and_then(|facts| facts.claim_interface)
}

/// Source-declared middleware category (`ACCESSORY`, `MOUSE`, `KEYBOARD`, ...).
pub(crate) fn middleware_category(product_id: u32) -> Option<&'static str> {
    catalog()
        .iter()
        .find(|entry| entry.product_id == product_id)
        .and_then(|entry| entry.middleware.as_ref())
        .and_then(|facts| facts.category.as_deref())
}

/// Only for the source monitor observation branch, after its serial query.
/// This does not validate the serial or turn a generic HID serial into that query.
pub(crate) fn lookup_monitor_port(raw_pid: u32, observed_monitor_serial: &str) -> IdentityLookup {
    if observed_monitor_serial.trim().is_empty() {
        return classify(raw_pid, vec![]);
    }
    classify(
        raw_pid,
        catalog()
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry
                    .monitor_port_ids
                    .as_ref()
                    .is_some_and(|ids| ids.contains(&raw_pid))
            })
            .map(|(index, entry)| {
                let mut identity = entry.identity(raw_pid, index);
                identity.is_monitor = true;
                identity
            })
            .collect(),
    )
}

//! Current host HD/_checkUSBDetail identity projection, without hardware state.
//! See docs/re/device-identity-current-contract.md for source boundaries.
#[cfg(test)]
use anyhow::{Context as _, ensure};
use serde::Deserialize;
#[cfg(test)]
use std::collections::BTreeSet;
use std::sync::OnceLock;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum Ids {
    One(u32),
    Many(Vec<u32>),
}
impl Ids {
    #[cfg(test)]
    fn validate(&self) -> anyhow::Result<()> {
        let valid = |id: &u32| *id > 0 && *id <= u32::from(u16::MAX);
        ensure!(
            match self {
                Self::One(id) => valid(id),
                Self::Many(ids) => !ids.is_empty() && ids.iter().all(valid),
            },
            "Identity transport IDs must be nonzero USB product IDs"
        );
        Ok(())
    }

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

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum DeviceCategory {
    Accessory,
    Audio,
    Broadcaster,
    Case,
    Chair,
    Egpu,
    Gamepad,
    Hue,
    Iot,
    Keyboard,
    Keypad,
    Monitor,
    Mouse,
    Mousemat,
    Mouseplusmat,
    System,
}

impl DeviceCategory {
    fn name(self) -> &'static str {
        match self {
            Self::Accessory => "ACCESSORY",
            Self::Audio => "AUDIO",
            Self::Broadcaster => "BROADCASTER",
            Self::Case => "CASE",
            Self::Chair => "CHAIR",
            Self::Egpu => "EGPU",
            Self::Gamepad => "GAMEPAD",
            Self::Hue => "HUE",
            Self::Iot => "IOT",
            Self::Keyboard => "KEYBOARD",
            Self::Keypad => "KEYPAD",
            Self::Monitor => "MONITOR",
            Self::Mouse => "MOUSE",
            Self::Mousemat => "MOUSEMAT",
            Self::Mouseplusmat => "MOUSEPLUSMAT",
            Self::System => "SYSTEM",
        }
    }
}

/// Per-product `DeviceInfo` facts declared by the product's own middleware bundle.
///
/// The fixed table carries source `claimInterface`/`dongleId`/`bleId`/`category`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MiddlewareFacts {
    #[serde(default)]
    pub(crate) dongle_id: Option<u32>,
    #[serde(default)]
    pub(crate) ble_id: Option<u32>,
    #[serde(default)]
    pub(crate) claim_interface: Option<u8>,
    #[serde(default)]
    pub(crate) category: Option<DeviceCategory>,
    #[serde(default, rename = "receipt")]
    pub(crate) _receipt: Option<serde::de::IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogProduct {
    pub(crate) product_id: u32,
    #[serde(default)]
    pub(crate) rep_id: Option<u32>,
    #[serde(default)]
    pub(crate) dongle_id: Option<Ids>,
    #[serde(default)]
    pub(crate) ble_id: Option<Ids>,
    #[serde(default)]
    pub(crate) xbox_id: Option<Ids>,
    #[serde(default)]
    pub(crate) wired_id: Option<Ids>,
    #[serde(default)]
    pub(crate) ps_mode_ids: Option<Vec<u32>>,
    #[serde(default)]
    pub(crate) monitor_port_ids: Option<Vec<u32>>,
    #[serde(default)]
    pub(crate) is_not_chroma_device: bool,
    #[serde(default)]
    pub(crate) middleware: Option<MiddlewareFacts>,
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
    CATALOG.get_or_init(crate::device_identity_catalog::products)
}

#[cfg(test)]
fn decode_catalog(json: &str) -> anyhow::Result<Vec<CatalogProduct>> {
    let products: Vec<CatalogProduct> =
        serde_json::from_str(json).context("Invalid identity catalog")?;
    ensure!(!products.is_empty(), "Identity catalog is empty");
    let mut seen = BTreeSet::new();
    for product in &products {
        ensure!(
            product.product_id > 0 && product.product_id <= u32::from(u16::MAX),
            "Identity product ID is invalid"
        );
        ensure!(
            seen.insert(product.product_id),
            "Identity catalog repeats a source product"
        );
        ensure!(
            product.rep_id.is_none_or(|id| id <= u32::from(u16::MAX)),
            "Identity route is outside USB product IDs"
        );
        for ids in [
            &product.dongle_id,
            &product.ble_id,
            &product.xbox_id,
            &product.wired_id,
        ]
        .into_iter()
        .flatten()
        {
            ids.validate()?;
        }
        for ids in [&product.ps_mode_ids, &product.monitor_port_ids]
            .into_iter()
            .flatten()
        {
            ensure!(
                !ids.is_empty() && ids.iter().all(|id| *id > 0 && *id <= u32::from(u16::MAX)),
                "Identity mode IDs are invalid"
            );
        }
        if let Some(facts) = &product.middleware {
            ensure!(
                facts
                    .dongle_id
                    .into_iter()
                    .chain(facts.ble_id)
                    .all(|id| id <= u32::from(u16::MAX)),
                "Middleware transport ID is invalid"
            );
        }
    }
    Ok(products)
}

/// Catalog facts only. No edition, layout, serial, online or readiness is inferred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductIdentity {
    pub product_id: u32,
    pub real_product_id: u32,
    pub source_product_id: u32,
    /// Preserve source row identity even when two rows route to the same product.
    pub catalog_index: usize,
    pub is_dongle: bool,
    pub is_ble: bool,
    pub is_xbox: bool,
    pub is_playstation: bool,
    pub is_monitor: bool,
    pub is_not_chroma_device: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityLookup {
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
    pub fn raw_product_id(&self) -> u32 {
        match self {
            Self::Unmatched { raw_product_id } | Self::Ambiguous { raw_product_id, .. } => {
                *raw_product_id
            }
            Self::Unique(identity) => identity.real_product_id,
        }
    }
    pub fn candidates(&self) -> &[ProductIdentity] {
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
pub fn lookup(raw_pid: u32) -> IdentityLookup {
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
pub fn lookup_receiver_peer(raw_pid: u32, match_product_id: bool) -> IdentityLookup {
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
pub fn claim_interface(product_id: u32) -> Option<u8> {
    catalog()
        .iter()
        .find(|entry| entry.product_id == product_id)
        .and_then(|entry| entry.middleware.as_ref())
        .and_then(|facts| facts.claim_interface)
}

/// Source-declared middleware category (`ACCESSORY`, `MOUSE`, `KEYBOARD`, ...).
pub fn middleware_category(product_id: u32) -> Option<&'static str> {
    catalog()
        .iter()
        .find(|entry| entry.product_id == product_id)
        .and_then(|entry| entry.middleware.as_ref())
        .and_then(|facts| facts.category.map(DeviceCategory::name))
}

/// Only for the source monitor observation branch, after its serial query.
/// This does not validate the serial or turn a generic HID serial into that query.
pub fn lookup_monitor_port(raw_pid: u32, observed_monitor_serial: &str) -> IdentityLookup {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_catalog_rejects_unknown_categories_invalid_ids_and_duplicate_rows() {
        for source in [
            r#"[{"product_id":756,"middleware":{"category":"UNKNOWN"}}]"#,
            r#"[{"product_id":756,"dongle_id":[]}]"#,
            r#"[{"product_id":756,"ble_id":70000}]"#,
            r#"[{"product_id":756},{"product_id":756}]"#,
            r#"[{"product_id":0}]"#,
        ] {
            assert!(decode_catalog(source).is_err(), "{source}");
        }
        // Shared transport aliases remain ambiguous by design; only repeated
        // source rows are invalid, not product branches sharing a radio.
        let catalog = decode_catalog(
            r#"[
            {"product_id":600,"dongle_id":625},
            {"product_id":601,"dongle_id":625}
        ]"#,
        )
        .unwrap();
        assert_eq!(catalog.len(), 2);
    }

    #[test]
    fn current756_routes_wired_dongle_and_ble_without_inventing_state() {
        for (raw, dongle, ble) in [(756, false, false), (757, true, false), (758, false, true)] {
            let IdentityLookup::Unique(identity) = lookup(raw) else {
                panic!("current756 transport {raw} did not resolve uniquely");
            };
            assert_eq!(identity.product_id, 756);
            assert_eq!(identity.source_product_id, 756);
            assert_eq!(identity.real_product_id, raw);
            assert_eq!(identity.is_dongle, dongle);
            assert_eq!(identity.is_ble, ble);
        }
        assert_eq!(claim_interface(756), Some(3));
        assert_eq!(middleware_category(756), Some("KEYBOARD"));
    }

    #[test]
    fn current756_receiver_peer_matches_its_original_radio_or_logical_id() {
        for raw in [756, 757] {
            let IdentityLookup::Unique(identity) = lookup_receiver_peer(raw, true) else {
                panic!("current756 peer {raw} did not resolve uniquely");
            };
            assert_eq!(identity.product_id, 756);
            assert_eq!(identity.real_product_id, raw);
            assert!(identity.is_dongle);
        }
        assert!(matches!(
            lookup_receiver_peer(756, false),
            IdentityLookup::Unmatched { .. }
        ));
    }
}

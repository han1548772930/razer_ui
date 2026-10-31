//! Source-declared discovery candidates and independent UI/middleware identities.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ids {
    One(u32),
    Many(&'static [u32]),
}

impl Ids {
    /// Host HD accepts a scalar or an array, with strict numeric equality.
    fn contains(self, raw: u32) -> bool {
        match self {
            Self::One(value) => value == raw,
            Self::Many(values) => values.contains(&raw),
        }
    }

    /// The receiver consumer compares catalog dongleId with scalar equality.
    fn scalar_equals(self, raw: u32) -> bool {
        matches!(self, Self::One(value) if value == raw)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Fixed DeviceInfo declarations. These fields do not establish a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceProductIdentity {
    pub product_id: u32,
    pub dongle_id: Option<u32>,
    pub ble_id: Option<u32>,
    pub vendor_id: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalInterface {
    pub(crate) physical_product_id: u32,
    pub(crate) interface: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClaimInterface {
    Uniform(u8),
    ByProduct(&'static [PhysicalInterface]),
}

impl ClaimInterface {
    fn for_physical_product(self, physical_product_id: u32) -> Option<u8> {
        match self {
            Self::Uniform(interface) => Some(interface),
            Self::ByProduct(branches) => branches
                .iter()
                .find(|branch| branch.physical_product_id == physical_product_id)
                .map(|branch| branch.interface),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeviceInfoDefinition {
    pub(crate) identity: SourceProductIdentity,
    pub(crate) claim_interface: Option<ClaimInterface>,
    pub(crate) category: DeviceCategory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProductDefinitions {
    pub(crate) mounted_product_id: u32,
    /// None retains a verified unavailable official endpoint, never a default.
    pub(crate) ui: Option<DeviceInfoDefinition>,
    pub(crate) middleware: Option<DeviceInfoDefinition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CatalogProduct {
    pub(crate) product_id: u32,
    pub(crate) rep_id: Option<u32>,
    pub(crate) dongle_id: Option<Ids>,
    pub(crate) ble_id: Option<Ids>,
    pub(crate) xbox_id: Option<Ids>,
    pub(crate) wired_id: Option<Ids>,
    pub(crate) ps_mode_ids: Option<&'static [u32]>,
    pub(crate) monitor_port_ids: Option<&'static [u32]>,
    pub(crate) is_not_chroma_device: bool,
}

impl CatalogProduct {
    fn route(&self) -> u32 {
        self.rep_id.filter(|id| *id != 0).unwrap_or(self.product_id)
    }

    fn matches(&self, raw: u32) -> bool {
        self.product_id == raw
            || self.rep_id == Some(raw)
            || contains(self.dongle_id, raw)
            || contains(self.ble_id, raw)
            || contains(self.xbox_id, raw)
            || contains(self.wired_id, raw)
            || self.ps_mode_ids.is_some_and(|ids| ids.contains(&raw))
    }

    fn identity(&self, raw: u32, catalog_index: usize) -> ProductIdentity {
        ProductIdentity {
            product_id: self.route(),
            real_product_id: raw,
            source_product_id: self.product_id,
            catalog_index,
            is_dongle: contains(self.dongle_id, raw),
            is_ble: contains(self.ble_id, raw),
            is_xbox: contains(self.xbox_id, raw),
            is_playstation: self.ps_mode_ids.is_some_and(|ids| ids.contains(&raw)),
            is_monitor: false,
            is_not_chroma_device: self.is_not_chroma_device,
        }
    }
}

fn contains(ids: Option<Ids>, raw: u32) -> bool {
    ids.is_some_and(|ids| ids.contains(raw))
}

fn catalog() -> &'static [CatalogProduct] {
    crate::device_identity_catalog::PRODUCTS
}

fn definitions(mounted_product_id: u32) -> Option<&'static ProductDefinitions> {
    crate::device_identity_catalog::DEFINITIONS
        .binary_search_by_key(&mounted_product_id, |entry| entry.mounted_product_id)
        .ok()
        .map(|index| &crate::device_identity_catalog::DEFINITIONS[index])
}

/// Identity declared by the mounted UI, including endpoints declaring aliases.
pub fn ui_product_identity(mounted_product_id: u32) -> Option<SourceProductIdentity> {
    definitions(mounted_product_id)?
        .ui
        .map(|info| info.identity)
}

/// Identity declared by the mounted middleware, independent of host candidates.
pub fn middleware_product_identity(mounted_product_id: u32) -> Option<SourceProductIdentity> {
    definitions(mounted_product_id)?
        .middleware
        .map(|info| info.identity)
}

/// Catalog facts only. Edition, layout, serial, online and readiness remain separate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductIdentity {
    pub product_id: u32,
    pub real_product_id: u32,
    pub source_product_id: u32,
    /// Preserve the original catalog row, including repeated source products.
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

/// Available-device classification for a Razer USB/HID PID. Callers own vendor,
/// container and connection observations. Monitor serial queries are separate.
pub fn lookup(raw_pid: u32) -> IdentityLookup {
    lookup_catalog(
        raw_pid,
        catalog(),
        crate::device_identity_catalog::IN_DEVELOPMENT,
    )
}

fn lookup_catalog(
    raw_pid: u32,
    products: &[CatalogProduct],
    in_development: &[u32],
) -> IdentityLookup {
    if raw_pid == 1325 {
        return classify(raw_pid, vec![]);
    }
    let matches: Vec<_> = products
        .iter()
        .enumerate()
        .filter(|(_, entry)| !in_development.contains(&entry.product_id) && entry.matches(raw_pid))
        .collect();
    if matches.len() == 1 && matches[0].1.monitor_port_ids.is_some() {
        return classify(raw_pid, vec![]);
    }
    let multiple = matches.len() > 1;
    classify(
        raw_pid,
        matches
            .into_iter()
            .map(|(index, entry)| {
                let mut identity = entry.identity(raw_pid, index);
                if multiple {
                    // Host _checkUSBDetail maps every row in source order.
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

/// The receiver status consumer uses the first catalog scalar dongleId match;
/// its product-ID branch is selected by the receiver's verified protocol owner.
pub fn lookup_receiver_peer(raw_pid: u32, match_product_id: bool) -> IdentityLookup {
    lookup_receiver_catalog(raw_pid, match_product_id, catalog())
}

fn lookup_receiver_catalog(
    raw_pid: u32,
    match_product_id: bool,
    products: &[CatalogProduct],
) -> IdentityLookup {
    let candidate = products.iter().enumerate().find(|(_, entry)| {
        (match_product_id && entry.product_id == raw_pid)
            || entry
                .dongle_id
                .is_some_and(|ids| ids.scalar_equals(raw_pid))
    });
    match candidate {
        Some((index, entry)) => {
            let mut identity = entry.identity(raw_pid, index);
            identity.product_id = entry.product_id;
            identity.is_dongle = true;
            IdentityLookup::Unique(identity)
        }
        None => IdentityLookup::Unmatched {
            raw_product_id: raw_pid,
        },
    }
}

/// Declared middleware interface when the physical PID equals the mounted PID.
pub fn claim_interface(product_id: u32) -> Option<u8> {
    claim_interface_for(product_id, product_id)
}

/// Resolve the actual physical branch of DeviceInfo.claimInterface. An absent
/// declaration or unlisted object key has no inferred interface.
pub fn claim_interface_for(mounted_product_id: u32, physical_product_id: u32) -> Option<u8> {
    definitions(mounted_product_id)?
        .middleware?
        .claim_interface?
        .for_physical_product(physical_product_id)
}

pub fn middleware_category(mounted_product_id: u32) -> Option<&'static str> {
    definitions(mounted_product_id)?
        .middleware
        .map(|facts| facts.category.name())
}

/// Only for the source monitor branch after its serial query. A generic HID
/// serial does not establish the monitor-specific observation.
pub fn lookup_monitor_port(raw_pid: u32, observed_monitor_serial: &str) -> IdentityLookup {
    lookup_monitor_catalog(
        raw_pid,
        observed_monitor_serial,
        catalog(),
        crate::device_identity_catalog::IN_DEVELOPMENT,
    )
}

fn lookup_monitor_catalog(
    raw_pid: u32,
    observed_monitor_serial: &str,
    products: &[CatalogProduct],
    in_development: &[u32],
) -> IdentityLookup {
    if observed_monitor_serial.is_empty() || raw_pid == 1325 {
        return classify(raw_pid, vec![]);
    }
    let matched = products.iter().enumerate().find(|(_, entry)| {
        !in_development.contains(&entry.product_id)
            && entry
                .monitor_port_ids
                .is_some_and(|ids| ids.contains(&raw_pid))
    });
    match matched {
        Some((index, entry)) => {
            let mut identity = entry.identity(raw_pid, index);
            identity.is_monitor = true;
            IdentityLookup::Unique(identity)
        }
        None => IdentityLookup::Unmatched {
            raw_product_id: raw_pid,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn product(product_id: u32) -> CatalogProduct {
        CatalogProduct {
            product_id,
            rep_id: None,
            dongle_id: None,
            ble_id: None,
            xbox_id: None,
            wired_id: None,
            ps_mode_ids: None,
            monitor_port_ids: None,
            is_not_chroma_device: false,
        }
    }

    fn unique(result: IdentityLookup) -> ProductIdentity {
        let IdentityLookup::Unique(identity) = result else {
            panic!("source identity was not unique: {result:?}");
        };
        identity
    }

    #[test]
    fn host_repeated_rows_are_distinct_and_keep_the_observed_raw_pid() {
        let rows = [
            CatalogProduct {
                dongle_id: Some(Ids::One(692)),
                ..product(691)
            },
            product(691),
        ];
        let result = lookup_catalog(691, &rows, &[]);
        assert!(matches!(result, IdentityLookup::Ambiguous { .. }));
        assert_eq!(result.candidates().len(), 2);
        for (index, identity) in result.candidates().iter().enumerate() {
            assert_eq!(identity.catalog_index, index);
            assert_eq!(identity.real_product_id, 691);
            assert_eq!(identity.product_id, 691);
            assert!(identity.is_dongle);
        }
        assert_eq!(
            unique(lookup_receiver_catalog(691, true, &rows)).catalog_index,
            0
        );
        assert_eq!(unique(lookup_catalog(692, &rows, &[])).product_id, 691);
    }

    #[test]
    fn host_alias_arrays_and_receiver_scalar_first_match_have_distinct_semantics() {
        let rows = [
            CatalogProduct {
                dongle_id: Some(Ids::Many(&[625, 626])),
                ..product(600)
            },
            CatalogProduct {
                dongle_id: Some(Ids::One(625)),
                ..product(601)
            },
            CatalogProduct {
                dongle_id: Some(Ids::One(625)),
                ..product(602)
            },
        ];
        assert_eq!(lookup_catalog(625, &rows, &[]).candidates().len(), 3);
        assert_eq!(
            unique(lookup_receiver_catalog(625, false, &rows)).product_id,
            601
        );
        assert!(matches!(
            lookup_receiver_catalog(626, false, &rows),
            IdentityLookup::Unmatched { .. }
        ));
        assert_eq!(
            unique(lookup_catalog(625, &rows, &[600, 601])).catalog_index,
            2
        );
    }

    #[test]
    fn host_mode_flags_and_rep_route_follow_only_catalog_fields() {
        let rows = [CatalogProduct {
            rep_id: Some(701),
            dongle_id: Some(Ids::One(702)),
            ble_id: Some(Ids::Many(&[703])),
            xbox_id: Some(Ids::One(704)),
            wired_id: Some(Ids::One(705)),
            ps_mode_ids: Some(&[706]),
            is_not_chroma_device: true,
            ..product(700)
        }];
        for raw in 700..=706 {
            let actual = unique(lookup_catalog(raw, &rows, &[]));
            assert_eq!(actual.product_id, 701);
            assert_eq!(actual.real_product_id, raw);
            assert_eq!(actual.is_dongle, raw == 702);
            assert_eq!(actual.is_ble, raw == 703);
            assert_eq!(actual.is_xbox, raw == 704);
            assert_eq!(actual.is_playstation, raw == 706);
            assert!(actual.is_not_chroma_device);
        }
    }

    #[test]
    fn monitor_requires_its_serial_branch_and_takes_the_first_port_match() {
        let rows = [
            CatalogProduct {
                monitor_port_ids: Some(&[200]),
                ..product(100)
            },
            CatalogProduct {
                monitor_port_ids: Some(&[200]),
                ..product(101)
            },
        ];
        assert!(matches!(
            lookup_catalog(100, &rows, &[]),
            IdentityLookup::Unmatched { .. }
        ));
        assert!(matches!(
            lookup_monitor_catalog(200, "", &rows, &[]),
            IdentityLookup::Unmatched { .. }
        ));
        let identity = unique(lookup_monitor_catalog(200, "monitor serial", &rows, &[]));
        assert_eq!(identity.product_id, 100);
        assert!(identity.is_monitor);
        assert_eq!(
            unique(lookup_monitor_catalog(200, "monitor serial", &rows, &[100])).product_id,
            101
        );
    }

    #[test]
    fn current_105_and_3886_source_aliases_do_not_create_discovery_candidates() {
        assert_eq!(unique(lookup(105)).product_id, 105);
        assert!(!unique(lookup(105)).is_dongle);
        assert_eq!(middleware_product_identity(105).unwrap().product_id, 104);
        assert_eq!(ui_product_identity(105).unwrap().product_id, 104);
        assert_eq!(unique(lookup(3884)).product_id, 3884);
        assert!(matches!(lookup(3886), IdentityLookup::Unmatched { .. }));
        assert_eq!(
            middleware_product_identity(3886).unwrap().dongle_id,
            Some(3884)
        );
        assert_eq!(unique(lookup_receiver_peer(3884, false)).product_id, 3884);
        assert_eq!(middleware_category(105), Some("MOUSEPLUSMAT"));
        assert_eq!(middleware_category(1318), Some("AUDIO"));
    }

    #[test]
    fn claim_interfaces_preserve_physical_branches_and_absent_declarations() {
        assert_eq!(claim_interface(756), Some(3));
        for (mounted, physical, expected) in [
            (574, 2308, 0),
            (574, 574, 2),
            (691, 691, 3),
            (691, 692, 2),
            (1383, 1383, 5),
            (1383, 1384, 4),
            (2595, 2593, 0),
            (2595, 2595, 2),
        ] {
            assert_eq!(claim_interface_for(mounted, physical), Some(expected));
        }
        assert_eq!(claim_interface_for(691, 693), None);
        assert_eq!(claim_interface_for(1383, 1385), None);
        assert_eq!(claim_interface(100), None);
        assert_eq!(claim_interface(680), None);
        assert_eq!(ui_product_identity(680), None);
        assert_eq!(middleware_category(680), None);
        assert_eq!(claim_interface(65535), None);
    }

    #[test]
    fn current756_routes_wired_dongle_and_ble_without_inventing_state() {
        for (raw, dongle, ble) in [(756, false, false), (757, true, false), (758, false, true)] {
            let identity = unique(lookup(raw));
            assert_eq!(identity.product_id, 756);
            assert_eq!(identity.source_product_id, 756);
            assert_eq!(identity.real_product_id, raw);
            assert_eq!(identity.is_dongle, dongle);
            assert_eq!(identity.is_ble, ble);
        }
        assert_eq!(middleware_category(756), Some("KEYBOARD"));
        for raw in [756, 757] {
            let identity = unique(lookup_receiver_peer(raw, true));
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

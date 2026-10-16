//! Source-derived host SVGs for window controls that must work offline.

pub(super) const ICONS: [(&str, &[u8]); 4] = [
    (
        "synapse/host-minimize.svg",
        include_bytes!("../../../assets/synapse/shared/window-controls/minimize.svg"),
    ),
    (
        "synapse/host-maximize.svg",
        include_bytes!("../../../assets/synapse/shared/window-controls/maximize.svg"),
    ),
    (
        "synapse/host-restore.svg",
        include_bytes!("../../../assets/synapse/shared/window-controls/restore.svg"),
    ),
    (
        "synapse/host-close.svg",
        include_bytes!("../../../assets/synapse/shared/window-controls/close.svg"),
    ),
];

pub(super) fn load(key: &str) -> Option<&'static [u8]> {
    ICONS
        .iter()
        .find_map(|(name, bytes)| (*name == key).then_some(*bytes))
}

#[cfg(test)]
mod tests {
    use gpui_kit::AssetSource as _;
    use sha2::{Digest as _, Sha256};

    #[test]
    fn controls_load_without_workers_credentials_or_downloads() {
        for (key, bytes) in super::ICONS {
            assert!(super::load(key).is_some());
            assert_eq!(
                crate::SynapseAssets.load(key).unwrap().unwrap().as_ref(),
                bytes
            );
            let svg = std::str::from_utf8(bytes).unwrap();
            assert!(svg.starts_with("<svg"));
            assert!(svg.contains("#999"));
            let (_, length, checksum) = crate::remote::identity(key).unwrap();
            assert_eq!(bytes.len() as u64, length);
            assert_eq!(format!("{:x}", Sha256::digest(bytes)), checksum);
        }
        assert!(super::load("synapse/product-182.png").is_none());
    }
}

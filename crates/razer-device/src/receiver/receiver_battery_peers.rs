//! Battery-monitor peer identity and native LED geometry from the current
//! product manifests. Physical filenames and declared device IDs are distinct.
use super::QuickEffect;
use crate::lighting_engine::LedConfiguration;

#[path = "receiver_battery_peers_generated.rs"]
mod generated;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerCategory {
    Mouse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerDeviceFactory {
    RzDevice25,
    RzDevice25DualLinkMouse,
    RzDevice30,
    RzDevice25Krakoff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerExitEffect {
    Spectrum,
    Static { color: [u8; 3] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerProfileEffect {
    Spectrum,
    Static { color: [u8; 3] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct PeerBatteryEffect {
    pub minimum: u8,
    pub maximum: u8,
    pub effect_id: u16,
    pub color: u32,
}
impl PeerBatteryEffect {
    pub fn new(minimum: u8, maximum: u8, effect_id: u16, color: u32) -> anyhow::Result<Self> {
        anyhow::ensure!(
            minimum <= maximum && maximum <= 100,
            "Battery effect interval exceeds its source percentage range"
        );
        anyhow::ensure!(
            color <= 0xff_ffff,
            "Battery effect color exceeds its source RGB range"
        );
        Ok(Self {
            minimum,
            maximum,
            effect_id,
            color,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct PeerBatteryEffects {
    pub full: PeerBatteryEffect,
    pub high: PeerBatteryEffect,
    pub mid: PeerBatteryEffect,
    pub low: PeerBatteryEffect,
    pub critical: PeerBatteryEffect,
}

impl PeerBatteryEffects {
    pub fn new(
        full: PeerBatteryEffect,
        high: PeerBatteryEffect,
        mid: PeerBatteryEffect,
        low: PeerBatteryEffect,
        critical: PeerBatteryEffect,
    ) -> anyhow::Result<Self> {
        let effects = [full, high, mid, low, critical];
        for percentage in 0..=100 {
            anyhow::ensure!(
                effects
                    .iter()
                    .filter(|effect| (effect.minimum..=effect.maximum).contains(&percentage))
                    .count()
                    == 1,
                "Battery effect intervals must cover each percentage once"
            );
        }
        Ok(Self {
            full,
            high,
            mid,
            low,
            critical,
        })
    }

    pub fn for_level(&self, battery_level: u8) -> anyhow::Result<&PeerBatteryEffect> {
        anyhow::ensure!(
            battery_level <= 100,
            "Battery percentage exceeds its source range"
        );
        [&self.full, &self.high, &self.mid, &self.low, &self.critical]
            .into_iter()
            .find(|effect| (effect.minimum..=effect.maximum).contains(&battery_level))
            .ok_or_else(|| anyhow::anyhow!("Battery percentage has no source effect interval"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct PeerSupport {
    pub source_product_id: u16,
    pub manifest_product_id: u16,
    pub edition: u8,
    pub vendor_id: u16,
    pub wired_product_id: u16,
    pub dongle_product_id: u16,
    pub ble_product_id: Option<u16>,
    pub category: PeerCategory,
    pub rz_device_name: PeerDeviceFactory,
    pub claim_interface: u8,
    pub max_retry_in: u8,
    pub max_retry_out: u8,
    pub sleep_time_between_in_ms: u16,
    pub sleep_time_between_out_ms: u16,
    pub sleep_time_between_out_in_ms: u16,
    pub support_battery_monitoring: bool,
    pub support_docking_station: bool,
    pub support_docking_v2: bool,
    pub is_dual_link: bool,
    pub ignore_set_device_mode_timeout_error: bool,
    pub quick_effect_on_exit: PeerExitEffect,
    pub default_profile_effect: PeerProfileEffect,
    pub skip_regions_for_charging_effect: bool,
    pub use_parent_container_id_to_set_device_handle: bool,
    pub create_preview_engine_main_device: Option<bool>,
    pub send_chroma_frames_to_dock: Option<bool>,
}
impl PeerSupport {
    pub fn for_product(source_product_id: u16, edition: u8) -> anyhow::Result<Self> {
        peer_support(source_product_id, edition)
    }

    pub fn default_quick_effect(&self) -> QuickEffect {
        let (selected_effect_id, selected_effect_setting) = match self.default_profile_effect {
            PeerProfileEffect::Spectrum => (3, None),
            PeerProfileEffect::Static {
                color: [red, green, blue],
            } => (
                1,
                Some(serde_json::Map::from_iter([(
                    "color1".into(),
                    serde_json::Value::String(format!("#{red:02x}{green:02x}{blue:02x}")),
                )])),
            ),
        };
        QuickEffect {
            selected_effect_id,
            selected_effect_setting,
            region_id: None,
            extensions: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct PeerManifest {
    pub support: PeerSupport,
    pub led: LedConfiguration,
    pub battery_level_effects: Option<PeerBatteryEffects>,
    pub charging_effects: Option<PeerBatteryEffects>,
}
impl PeerManifest {
    pub fn for_product(source_product_id: u16, edition: u8, layout: u8) -> anyhow::Result<Self> {
        peer_manifest(source_product_id, edition, layout)
    }
}

pub fn peer_identities() -> &'static [(u16, u8)] {
    generated::IDENTITIES
}

pub fn peer_support(source_product_id: u16, edition: u8) -> anyhow::Result<PeerSupport> {
    generated::support(source_product_id, edition).ok_or_else(|| {
        anyhow::anyhow!(
            "Battery-monitor peer has no current manifest for product {source_product_id}, edition {edition}"
        )
    })
}

pub fn peer_manifest(
    source_product_id: u16,
    edition: u8,
    layout: u8,
) -> anyhow::Result<PeerManifest> {
    let support = peer_support(source_product_id, edition)?;
    // DeviceInfo.getLedConfig tries the exact edition/layout first. Its
    // non-keyboard fallback uses the same edition, layout 0 and then layout 1.
    let led = generated::led_configuration(source_product_id, edition, layout)
        .or_else(|| generated::led_configuration(source_product_id, edition, 0))
        .or_else(|| generated::led_configuration(source_product_id, edition, 1))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Battery-monitor peer has no current LED geometry for product {source_product_id}, edition {edition}, layout {layout}"
            )
        })?;
    Ok(PeerManifest {
        support,
        led,
        battery_level_effects: generated::battery_level_effects(source_product_id, edition),
        charging_effects: generated::charging_effects(source_product_id, edition),
    })
}

#[cfg(test)]
#[path = "receiver_battery_peers_tests.rs"]
mod tests;

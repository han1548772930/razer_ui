//! Fixed local receiver defaults. JSON exists only at document boundaries.
use crate::receiver_document::{
    BrightnessData, DeviceDocument, EffectsData, Field, LightingData, ProfileData,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingMaster {
    pub product_id: u16,
    pub dongle_id: u16,
    pub ble_id: u16,
    pub claim_interface: u8,
    #[serde(
        rename = "supports8KHzPollingRate",
        skip_serializing_if = "Option::is_none"
    )]
    pub supports_eight_khz: Option<bool>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Brightness {
    pub is_enabled: bool,
    pub value: u8,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwitchOffLighting {
    pub is_display_on: bool,
    pub is_idle_enabled: bool,
    pub idle_minutes: u16,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuickEffects {
    pub selected_effect_id: u16,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileDocument<'a> {
    pub guid: &'a str,
    pub name: &'a str,
    pub brightness: Brightness,
    pub switch_off_lighting: SwitchOffLighting,
    pub quick_effects: QuickEffects,
    pub mappings: &'static [()],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_pair_two_devices: Option<bool>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct ReportIds {
    #[serde(rename = "4")]
    pub key_report: &'static str,
    #[serde(rename = "5")]
    pub hardware_event: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlobalDocument {
    pub version: u64,
    pub vendor_id: u16,
    pub product_id: u32,
    pub category: &'static str,
    #[serde(rename = "reportIDs")]
    pub report_ids: ReportIds,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_pair_two_devices: Option<bool>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResetDefaults {
    pub product_id: u32,
    pub edition_id: u32,
    pub device_name: &'static str,
    pub schema_version: u32,
    pub default_global: GlobalDocument,
    pub default_profile: ProfileDocument<'static>,
}

impl ResetDefaults {
    pub fn profile(&self, guid: &str, name: &str) -> ProfileData {
        let source = self.default_profile;
        ProfileData {
            guid: Field::set(guid.to_owned()),
            name: Field::set(name.to_owned()),
            brightness: Field::set(BrightnessData {
                is_enabled: Field::set(source.brightness.is_enabled),
                value: Field::set(source.brightness.value),
                ..BrightnessData::default()
            }),
            switch_off_lighting: Field::set(LightingData {
                is_display_on: Field::set(source.switch_off_lighting.is_display_on),
                is_idle_enabled: Field::set(source.switch_off_lighting.is_idle_enabled),
                idle_minutes: Field::set(source.switch_off_lighting.idle_minutes),
                ..LightingData::default()
            }),
            quick_effects: Field::set(EffectsData {
                selected_effect_id: Field::set(source.quick_effects.selected_effect_id),
                ..EffectsData::default()
            }),
            mappings: Field::set(Vec::new()),
            can_pair_two_devices: source
                .can_pair_two_devices
                .map_or(Field::Missing, Field::set),
            ..ProfileData::default()
        }
    }

    pub fn global_document(&self) -> DeviceDocument {
        let source = self.default_global;
        DeviceDocument {
            version: Field::set(source.version),
            product_id: Field::set(source.product_id),
            vendor_id: Field::set(source.vendor_id),
            category: Field::set(source.category.to_owned()),
            report_ids: Field::set(self.report_ids()),
            can_pair_two_devices: source
                .can_pair_two_devices
                .map_or(Field::Missing, Field::set),
            ..DeviceDocument::default()
        }
    }

    pub fn report_ids(&self) -> std::collections::BTreeMap<String, String> {
        let source = self.default_global.report_ids;
        [
            ("4".to_owned(), source.key_report.to_owned()),
            ("5".to_owned(), source.hardware_event.to_owned()),
        ]
        .into_iter()
        .collect()
    }
}

//! Complete current182/current190 profile contract and source-owned defaults.
//! Defaults describe a local profile; they never establish a device observation.
use crate::mouse_dpi_stages::{DpiStage, DpiStagesDraft};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

type Extensions = BTreeMap<String, serde_json::Value>;

fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Stages {
    enable: bool,
    active: usize,
    count: usize,
    stages: Vec<Stage>,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Stage {
    x: u16,
    y: u16,
    independent: bool,
    visible: bool,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}

impl Stages {
    pub fn draft(&self) -> Result<DpiStagesDraft> {
        let value = self.clone();
        ensure!(
            value.stages.len() == 5 && (1..=5).contains(&value.active),
            "invalid current host DPI row selection"
        );
        ensure!(
            (1..=5).contains(&value.count)
                && value.stages.iter().filter(|s| s.visible).count() == value.count,
            "invalid current host DPI visibility/count"
        );
        ensure!(
            value.stages.iter().all(|s| s.x > 0 && s.y > 0),
            "invalid current host DPI value"
        );
        Ok(DpiStagesDraft {
            enabled: value.enable,
            active_stage: value.active,
            stages: value
                .stages
                .into_iter()
                .map(|stage| DpiStage {
                    x: stage.x,
                    y: stage.y,
                    independent: stage.independent,
                    visible: stage.visible,
                })
                .collect(),
        })
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    guid: String,
    name: String,
    dpi: Dpi,
    mappings: Vec<Mapping>,
    #[serde(rename = "dpiStages")]
    stages: Stages,
    low_power_mode: u8,
    polling_rate: u32,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    led_power_settings: Option<LedPower>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    polling_rate_wireless: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    quick_effects: Option<QuickEffects>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    in_game_polling_rate: Option<InGamePollingRate>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    selected_sensitivity_matcher: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    rotation: Option<Rotation>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    indicator_led_configuration: Option<Vec<IndicatorLed>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    dynamic_sensitivity: Option<DynamicSensitivity>,
    #[serde(flatten)]
    extensions: Extensions,
}

impl Profile {
    pub fn guid(&self) -> &str {
        &self.guid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn source_settings(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(self)?)
    }

    pub fn stages(&self) -> Result<DpiStagesDraft> {
        self.stages.draft()
    }

    pub fn validate_for(&self, product_id: u32) -> Result<()> {
        ensure!(
            matches!(product_id, 182 | 190),
            "current mouse profile product is not verified"
        );
        ensure!(!self.guid.is_empty(), "host mouse profile GUID is empty");
        let stages = self.stages()?;
        crate::mouse_current_dpi::pack_for(product_id, &stages)?;
        let cap = crate::mouse_current_dpi::capability(product_id)
            .context("host mouse profile has no current DPI capability")?;
        ensure!(
            [self.dpi.x, self.dpi.y].iter().all(|dpi| {
                *dpi >= u32::from(cap.minimum()) && *dpi <= u32::from(cap.maximum())
            }),
            "host mouse current DPI is outside the product range"
        );
        ensure!(
            self.low_power_mode <= 100,
            "host mouse low-power threshold exceeds 100"
        );
        ensure!(
            self.polling_rate_wireless.is_some(),
            "host mouse wireless polling rate is absent"
        );
        for rate in [Some(self.polling_rate), self.polling_rate_wireless]
            .into_iter()
            .flatten()
        {
            validate_polling_rate(product_id, rate)?;
        }
        for mapping in &self.mappings {
            ensure!(
                !mapping.input_id.is_empty() && !mapping.output_type.is_empty(),
                "host mouse mapping identity is empty"
            );
        }
        if product_id == 182 {
            ensure!(
                self.led_power_settings.is_some(),
                "host mouse LED power settings are absent"
            );
        } else {
            ensure!(
                self.quick_effects.is_some()
                    && self.in_game_polling_rate.is_some()
                    && self.selected_sensitivity_matcher.is_some()
                    && self.rotation.is_some()
                    && self.indicator_led_configuration.is_some()
                    && self.dynamic_sensitivity.is_some(),
                "host current190 profile is missing required settings"
            );
        }
        if let Some(polling) = &self.in_game_polling_rate {
            ensure!(
                polling.state <= 1,
                "host mouse in-game polling state is unknown"
            );
            validate_polling_rate(product_id, polling.wired)?;
            validate_polling_rate(product_id, polling.wireless)?;
        }
        if let Some(rotation) = &self.rotation {
            crate::mouse_rotation::Setting {
                enabled: rotation.is_enabled,
                angle_degrees: rotation.value,
            }
            .validate()?;
        }
        if let Some(leds) = &self.indicator_led_configuration {
            let indexes = leds.iter().map(|led| led.index).collect::<BTreeSet<_>>();
            ensure!(
                leds.len() == 3 && indexes == BTreeSet::from([0, 1, 2]),
                "host mouse LED indexes are invalid"
            );
        }
        if let Some(dynamic) = &self.dynamic_sensitivity {
            let mode = crate::mouse_dynamic::mode(dynamic.mode)
                .context("host mouse dynamic-sensitivity mode is unknown")?;
            ensure!(
                dynamic.state <= 1,
                "host mouse dynamic-sensitivity state is unknown"
            );
            crate::mouse_dynamic::Setting::new(
                dynamic.state == 1,
                mode,
                dynamic.custom_sensor_accelerations.clone(),
            )?;
        }
        Ok(())
    }
}

fn validate_polling_rate(product_id: u32, hz: u32) -> Result<()> {
    ensure!(
        matches!(hz, 125 | 250 | 500 | 1000)
            || product_id == 190 && matches!(hz, 2000 | 4000 | 8000),
        "host mouse polling rate is unknown"
    );
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Dpi {
    x: u32,
    y: u32,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
enum InputType {
    MouseInput,
    DKMInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Mapping {
    #[serde(rename = "inputID")]
    input_id: String,
    input_type: InputType,
    output_type: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    is_hyper_shift: Option<bool>,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LedPower {
    idle_state: u8,
    idle_value: u32,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuickEffects {
    selected_effect_id: u32,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct InGamePollingRate {
    state: u8,
    wired: u32,
    wireless: u32,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Rotation {
    is_enabled: bool,
    value: i8,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IndicatorLed {
    index: u8,
    led_status: crate::mouse_indicator_led::Mode,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DynamicSensitivity {
    mode: u8,
    state: u8,
    custom_sensor_accelerations: Vec<f32>,
    template_id: u32,
    #[serde(flatten)]
    extensions: Extensions,
}

/// Source getDefaultProfile constructor after edition selection and identity
/// assignment. The caller owns UUID generation and the observed computer name.
pub fn new_profile(product_id: u32, edition_id: u32, guid: &str, name: &str) -> Result<Profile> {
    let mut profile = template(product_id, edition_id)?;
    profile.guid = guid.to_owned();
    profile.name = name.to_owned();
    profile.validate_for(product_id)?;
    Ok(profile)
}

pub fn source_device_name(product_id: u32) -> Result<&'static str> {
    match product_id {
        182 => Ok("Razer DeathAdder V3 Pro"),
        190 => Ok("Razer DeathAdder V4 Pro"),
        _ => anyhow::bail!("current mouse source device name is not verified"),
    }
}

pub(crate) fn template_182() -> Profile {
    template(182, 0).expect("current182 source edition zero")
}

fn template(product_id: u32, edition_id: u32) -> Result<Profile> {
    ensure!(
        match product_id {
            182 => matches!(edition_id, 0 | 128 | 129 | 130 | 132),
            190 => matches!(edition_id, 0 | 128 | 130 | 131 | 132 | 133),
            _ => false,
        },
        "current mouse default profile edition is not present in the source table"
    );
    let current190 = product_id == 190;
    Ok(Profile {
        guid: String::new(),
        name: String::new(),
        dpi: Dpi {
            x: 800,
            y: 800,
            extensions: Extensions::new(),
        },
        mappings: Vec::new(),
        stages: Stages {
            enable: true,
            active: 3,
            count: 5,
            stages: [400, 800, 1600, 3200, 6400]
                .map(|dpi| Stage {
                    x: dpi,
                    y: dpi,
                    independent: false,
                    visible: true,
                    extensions: Extensions::new(),
                })
                .into(),
            extensions: Extensions::new(),
        },
        low_power_mode: 30,
        polling_rate: 1000,
        polling_rate_wireless: Some(1000),
        led_power_settings: (!current190).then_some(LedPower {
            idle_state: 1,
            idle_value: 60,
            extensions: Extensions::new(),
        }),
        quick_effects: current190.then_some(QuickEffects {
            selected_effect_id: 3,
            extensions: Extensions::new(),
        }),
        in_game_polling_rate: current190.then_some(InGamePollingRate {
            state: 0,
            wired: 1000,
            wireless: 1000,
            extensions: Extensions::new(),
        }),
        selected_sensitivity_matcher: current190.then(String::new),
        rotation: current190.then_some(Rotation {
            is_enabled: false,
            value: 0,
            extensions: Extensions::new(),
        }),
        indicator_led_configuration: current190.then(|| {
            vec![
                IndicatorLed {
                    index: 0,
                    led_status: crate::mouse_indicator_led::Mode::ConnectionQuality,
                    extensions: Extensions::new(),
                },
                IndicatorLed {
                    index: 1,
                    led_status: crate::mouse_indicator_led::Mode::BatteryLevel,
                    extensions: Extensions::new(),
                },
                IndicatorLed {
                    index: 2,
                    led_status: crate::mouse_indicator_led::Mode::PollingRate,
                    extensions: Extensions::new(),
                },
            ]
        }),
        dynamic_sensitivity: current190.then_some(DynamicSensitivity {
            mode: 0,
            state: 0,
            custom_sensor_accelerations: Vec::new(),
            template_id: 0,
            extensions: Extensions::new(),
        }),
        extensions: Extensions::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current190_default_matches_the_complete_source_profile_without_hardware_claims() {
        let profile = new_profile(190, 0, "source-guid", "PC-Default").unwrap();
        assert_eq!(
            profile.source_settings().unwrap(),
            serde_json::json!({
                "guid":"source-guid","name":"PC-Default","dpi":{"x":800,"y":800},
                "quickEffects":{"selectedEffectId":3},"mappings":[],
                "dpiStages":{"stages":[
                    {"x":400,"y":400,"independent":false,"visible":true},
                    {"x":800,"y":800,"independent":false,"visible":true},
                    {"x":1600,"y":1600,"independent":false,"visible":true},
                    {"x":3200,"y":3200,"independent":false,"visible":true},
                    {"x":6400,"y":6400,"independent":false,"visible":true}
                ],"active":3,"enable":true,"count":5},
                "inGamePollingRate":{"state":0,"wired":1000,"wireless":1000},
                "pollingRate":1000,"pollingRateWireless":1000,"lowPowerMode":30,
                "selectedSensitivityMatcher":"","rotation":{"isEnabled":false,"value":0},
                "indicatorLedConfiguration":[{"ledStatus":2,"index":0},{"ledStatus":1,"index":1},{"ledStatus":3,"index":2}],
                "dynamicSensitivity":{"mode":0,"state":0,"customSensorAccelerations":[],"templateId":0}
            })
        );
        assert_eq!(profile.stages().unwrap().active_stage, 3);
    }

    #[test]
    fn current_default_editions_follow_each_actual_source_table() {
        for edition in [0, 128, 129, 130, 132] {
            assert!(new_profile(182, edition, "guid", "name").is_ok());
        }
        assert!(new_profile(182, 131, "guid", "name").is_err());
        for edition in [0, 128, 130, 131, 132, 133] {
            assert!(new_profile(190, edition, "guid", "name").is_ok());
        }
        assert!(new_profile(190, 129, "guid", "name").is_err());
        assert!(new_profile(170, 0, "guid", "name").is_err());
    }
}

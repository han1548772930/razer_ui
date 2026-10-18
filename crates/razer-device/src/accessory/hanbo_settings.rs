//! Local Hanbo settings are independent of native command acknowledgements.
use crate::hanbo_cooler::{
    CoolerChannel, CoolerCommand, FAN_PORT_ID, FAN_PORT_NAME, PUMP_PORT_ID, PUMP_PORT_NAME,
    PortThermal, ThermalProfile,
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const TEMPERATURE_UNIT_FIELD: &str = "isCelsius";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DefaultPort {
    id: u32,
    name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    brightness: Option<PortBrightness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thermal: Option<PortThermal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quick_effects: Option<PortEffect>,
}

#[derive(Serialize)]
struct PortBrightness {
    value: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PortEffect {
    selected_effect_id: u8,
    effect_settings: [EffectSetting; 1],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectSetting {
    selected_effect_id: u8,
    selected_effect_setting: EffectColor,
}

#[derive(Serialize)]
struct EffectColor {
    color1: &'static str,
}

/// The current MW DEFAULTPROFILE has two thermal and four lighting ports.
/// Serialization happens only when constructing a local profile document.
fn source_ports() -> [DefaultPort; 6] {
    let thermal = ThermalProfile::default();
    let lighting = |id, name, color1| DefaultPort {
        id,
        name,
        brightness: None,
        thermal: None,
        quick_effects: Some(PortEffect {
            selected_effect_id: 3,
            effect_settings: [EffectSetting {
                selected_effect_id: 1,
                selected_effect_setting: EffectColor { color1 },
            }],
        }),
    };
    [
        DefaultPort {
            id: PUMP_PORT_ID,
            name: PUMP_PORT_NAME,
            brightness: Some(PortBrightness { value: 50 }),
            thermal: Some(thermal.pump),
            quick_effects: None,
        },
        DefaultPort {
            id: FAN_PORT_ID,
            name: FAN_PORT_NAME,
            brightness: Some(PortBrightness { value: 50 }),
            thermal: Some(thermal.fans),
            quick_effects: None,
        },
        lighting(4, "Pump Cap", "#c00000"),
        lighting(2_147_483_649, "Fan 1", "#0000ff"),
        lighting(2_147_483_650, "Fan 2", "#00ff00"),
        lighting(2_147_483_651, "Fan 3", "#00ff00"),
    ]
}

pub fn default_ports() -> Value {
    serde_json::to_value(source_ports()).expect("source Hanbo port constants are serializable")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileBrightness {
    is_enabled: bool,
    value: u8,
    is_global_brightness: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileEffect {
    selected_effect_id: u8,
    is_global_quick_effect: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LightingIdle {
    is_display_on: bool,
    is_idle_enabled: bool,
    idle_minutes: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DefaultProfile {
    brightness: ProfileBrightness,
    quick_effects: ProfileEffect,
    switch_off_lighting: LightingIdle,
    ports: [DefaultPort; 6],
    mappings: [(); 0],
}

/// Source DEFAULTPROFILE settings; the profile owner supplies its GUID/name.
pub fn default_profile_settings() -> Value {
    serde_json::to_value(DefaultProfile {
        brightness: ProfileBrightness {
            is_enabled: true,
            value: 100,
            is_global_brightness: true,
        },
        quick_effects: ProfileEffect {
            selected_effect_id: 3,
            is_global_quick_effect: true,
        },
        switch_off_lighting: LightingIdle {
            is_display_on: false,
            is_idle_enabled: false,
            idle_minutes: 1,
        },
        ports: source_ports(),
        mappings: [],
    })
    .expect("source Hanbo default profile is serializable")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileRestore {
    All,
    Changes,
}

impl ProfileRestore {
    pub fn commands(
        self,
        previous: Option<&Value>,
        current: Option<&Value>,
    ) -> Result<Vec<CoolerCommand>> {
        match self {
            Self::All => restore_commands(current),
            Self::Changes => crate::hanbo_profile_restore::commands(previous, current),
        }
    }
}

pub fn device_fields(product_id: u32) -> &'static [&'static str] {
    if product_id == crate::hanbo_cooler::PRODUCT_ID {
        &[TEMPERATURE_UNIT_FIELD]
    } else {
        &[]
    }
}

/// A committed source interaction, captured before any later page edits.
#[derive(Clone, Debug, PartialEq)]
pub enum LocalChange {
    Thermal {
        channel: CoolerChannel,
        thermal: PortThermal,
    },
    TemperatureUnit {
        celsius: bool,
    },
}

#[derive(Deserialize)]
struct PortIdentity {
    name: String,
    thermal: Option<PortThermal>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredPort {
    id: u32,
    name: String,
    #[serde(default)]
    is_active: Option<bool>,
    thermal: Option<PortThermal>,
}

#[derive(PartialEq)]
struct ThermalPort {
    channel: CoolerChannel,
    thermal: PortThermal,
}

fn thermal_ports(profile: Option<&Value>) -> Result<Vec<ThermalPort>> {
    let Some(profile) = profile else {
        return Ok(Vec::new());
    };
    let Some(ports) = profile
        .as_object()
        .context("Hanbo profile settings must be an object")?
        .get("ports")
    else {
        return Ok(Vec::new());
    };
    let ports: Vec<StoredPort> = serde_json::from_value(ports.clone())
        .context("Hanbo stored ports must have typed identities and thermal settings")?;
    let mut identifiers = std::collections::BTreeSet::new();
    for port in &ports {
        ensure!(identifiers.insert(port.id), "Hanbo port ID is duplicated");
    }
    let mut result = Vec::new();
    // MW runtimeData.ports supplies the merge and dispatch order; saved array
    // order and the visual order of the two charts do not reorder native work.
    for (id, name, channel) in [
        (PUMP_PORT_ID, PUMP_PORT_NAME, CoolerChannel::Pump),
        (FAN_PORT_ID, FAN_PORT_NAME, CoolerChannel::Fans),
    ] {
        let Some(port) = ports.iter().find(|port| port.id == id) else {
            continue;
        };
        if port.is_active == Some(false) {
            continue;
        }
        let Some(thermal) = &port.thermal else {
            continue;
        };
        ensure!(port.name == name, "Hanbo thermal port identity differs");
        ensure!(
            ThermalProfile::default().restore_port(channel, thermal.clone()),
            "Hanbo stored thermal curve is invalid"
        );
        result.push(ThermalPort {
            channel,
            thermal: thermal.clone(),
        });
    }
    Ok(result)
}

/// One source restore pass merges saved ports into the stable runtime order.
pub fn restore_commands(profile: Option<&Value>) -> Result<Vec<CoolerCommand>> {
    thermal_ports(profile)?
        .into_iter()
        .map(|port| CoolerCommand::for_port(port.channel, &port.thermal))
        .collect()
}

impl LocalChange {
    /// MW `co` calls `Mn` after each of its two `Pf` calls. A thermal diff
    /// selects its channel; an absent diff restores every active thermal port.
    /// The source NORMAL queues preserve both passes, including equal cleanup.
    pub fn commands_after_save(
        &self,
        previous: Option<&Value>,
        current: Option<&Value>,
    ) -> Result<Vec<CoolerCommand>> {
        let Self::Thermal { channel, .. } = self else {
            return Ok(Vec::new());
        };
        let previous = thermal_ports(previous)?;
        let current = thermal_ports(current)?;
        let mut commands = Vec::new();
        for port in &current {
            if previous == current || port.channel == *channel {
                commands.push(CoolerCommand::for_port(port.channel, &port.thermal)?);
            }
        }
        commands.extend(commands.clone());
        Ok(commands)
    }

    pub fn apply(&self, profile: &mut Option<Value>, device: &mut Option<Value>) -> Result<()> {
        match self {
            Self::TemperatureUnit { celsius } => {
                let target = device
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .context("Hanbo device settings must be an object")?;
                target.insert(TEMPERATURE_UNIT_FIELD.into(), Value::Bool(*celsius));
            }
            Self::Thermal { channel, thermal } => {
                let mut validation = ThermalProfile::default();
                ensure!(
                    validation.restore_port(*channel, thermal.clone()),
                    "Hanbo local thermal curve is invalid"
                );
                let target = profile
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .context("Hanbo profile settings must be an object")?;
                let ports = target
                    .entry("ports")
                    .or_insert_with(default_ports)
                    .as_array_mut()
                    .context("Hanbo profile ports must be an array")?;
                let mut matching = None;
                for (index, port) in ports.iter().enumerate() {
                    let identity: PortIdentity = serde_json::from_value(port.clone())
                        .context("Hanbo local port identity is invalid")?;
                    let selected = identity.thermal.is_some()
                        && match channel {
                            CoolerChannel::Fans => identity.name == FAN_PORT_NAME,
                            CoolerChannel::Pump => identity.name != FAN_PORT_NAME,
                        };
                    if selected {
                        ensure!(matching.is_none(), "Hanbo thermal channel is ambiguous");
                        matching = Some(index);
                    }
                }
                if let Some(index) = matching {
                    let current = ports[index]
                        .as_object_mut()
                        .context("Hanbo thermal port must be an object")?
                        .get_mut("thermal")
                        .and_then(Value::as_object_mut)
                        .context("Hanbo thermal settings must be an object")?;
                    current.insert(
                        "advancedSetting".into(),
                        serde_json::to_value(&thermal.advanced_setting)?,
                    );
                    let indicator = current
                        .get_mut("activeThermalIndicator")
                        .and_then(Value::as_object_mut)
                        .context("Hanbo active thermal indicator must be an object")?;
                    indicator.insert(
                        "modeSelected".into(),
                        serde_json::to_value(thermal.active_thermal_indicator.mode_selected)?,
                    );
                } else {
                    let (id, name) = match channel {
                        CoolerChannel::Fans => (FAN_PORT_ID, FAN_PORT_NAME),
                        CoolerChannel::Pump => (PUMP_PORT_ID, PUMP_PORT_NAME),
                    };
                    ports.push(json!({"id": id, "name": name, "thermal": thermal}));
                }
            }
        }
        Ok(())
    }
}

/// Absence retains the source UI's initialization; malformed stored data is an
/// error, not an observed or silently substituted device setting.
pub fn temperature_unit(settings: Option<&Value>) -> Result<Option<bool>> {
    let Some(settings) = settings else {
        return Ok(None);
    };
    let settings = settings
        .as_object()
        .context("Hanbo device settings must be an object")?;
    settings
        .get(TEMPERATURE_UNIT_FIELD)
        .map(|value| value.as_bool().context("Hanbo isCelsius must be a boolean"))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_default_ports_keep_source_order_and_lighting() {
        let ports = default_ports();
        let ports = ports.as_array().unwrap();
        assert_eq!(
            ports
                .iter()
                .map(|port| port["id"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            [2, 1, 4, 2_147_483_649, 2_147_483_650, 2_147_483_651]
        );
        assert_eq!(ports[0]["brightness"]["value"], 50);
        let fan_curve: Vec<f64> =
            serde_json::from_value(ports[1]["thermal"]["advancedSetting"].clone()).unwrap();
        assert_eq!(fan_curve, [20., 30., 40., 50., 60., 80., 100.]);
        for (port, color) in ports[2..]
            .iter()
            .zip(["#c00000", "#0000ff", "#00ff00", "#00ff00"])
        {
            assert_eq!(port["quickEffects"]["selectedEffectId"], 3);
            assert_eq!(
                port["quickEffects"]["effectSettings"][0]["selectedEffectSetting"]["color1"],
                color
            );
        }
    }

    #[test]
    fn changed_port_dispatches_twice_and_equal_cleanup_restores_in_runtime_order() {
        let previous = Some(json!({"ports":default_ports()}));
        let mut current = previous.clone();
        let mut thermal = ThermalProfile::default().fans;
        thermal.active_thermal_indicator.mode_selected = crate::hanbo_cooler::ThermalMode::Quiet;
        let change = LocalChange::Thermal {
            channel: CoolerChannel::Fans,
            thermal,
        };
        change.apply(&mut current, &mut None).unwrap();
        assert_eq!(
            change
                .commands_after_save(previous.as_ref(), current.as_ref())
                .unwrap(),
            [
                CoolerCommand::FanParameter { mode: 1, speed: 20 },
                CoolerCommand::FanParameter { mode: 1, speed: 20 }
            ]
        );
        assert_eq!(
            change
                .commands_after_save(current.as_ref(), current.as_ref())
                .unwrap(),
            [
                CoolerCommand::PumpParameter { mode: 2, speed: 50 },
                CoolerCommand::FanParameter { mode: 1, speed: 20 },
                CoolerCommand::PumpParameter { mode: 2, speed: 50 },
                CoolerCommand::FanParameter { mode: 1, speed: 20 },
            ]
        );
        assert!(
            LocalChange::TemperatureUnit { celsius: true }
                .commands_after_save(None, None)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn restore_merges_by_stable_port_id_and_rejects_ambiguous_data() {
        assert!(restore_commands(None).unwrap().is_empty());
        assert!(restore_commands(Some(&json!({}))).unwrap().is_empty());
        let mut ports = default_ports();
        ports.as_array_mut().unwrap().reverse();
        assert_eq!(
            restore_commands(Some(&json!({"ports":ports}))).unwrap(),
            [
                CoolerCommand::PumpParameter { mode: 2, speed: 50 },
                CoolerCommand::FanParameter { mode: 2, speed: 50 },
            ]
        );
        assert!(
            restore_commands(Some(&json!({"ports":[
                {"id":1,"name":"Fans"}, {"id":1,"name":"Fans"}
            ]})))
            .is_err()
        );
    }

    #[test]
    fn independent_channel_saves_preserve_other_ports_and_extension_fields() {
        let mut profile = Some(json!({"guid":"profile-a","ports":[
            {"id":2,"name":"Pump Cap","lighting":{"effect":"static"},"thermal":ThermalProfile::default().pump},
            {"id":1,"name":"Fans","lighting":{"effect":"wave"},"thermal":ThermalProfile::default().fans},
            {"id":3,"name":"Logo","lighting":{"effect":"static"}}
        ]}));
        let mut device = Some(json!({"isCelsius":true,"extension":7}));
        profile.as_mut().unwrap()["ports"][0]["thermal"]["extension"] = json!({"keep":true});
        profile.as_mut().unwrap()["ports"][0]["thermal"]["activeThermalIndicator"]["extension"] =
            json!(11);
        let mut fan = ThermalProfile::default().fans;
        fan.active_thermal_indicator.mode_selected = crate::hanbo_cooler::ThermalMode::Performance;
        LocalChange::Thermal {
            channel: CoolerChannel::Fans,
            thermal: fan.clone(),
        }
        .apply(&mut profile, &mut device)
        .unwrap();
        let mut pump = ThermalProfile::default().pump;
        pump.advanced_setting = vec![10., 25., 50., 75., 100.];
        LocalChange::Thermal {
            channel: CoolerChannel::Pump,
            thermal: pump.clone(),
        }
        .apply(&mut profile, &mut device)
        .unwrap();
        let profile = profile.unwrap();
        assert_eq!(
            serde_json::from_value::<PortThermal>(profile["ports"][0]["thermal"].clone()).unwrap(),
            pump
        );
        assert_eq!(
            profile["ports"][0]["thermal"]["extension"],
            json!({"keep":true})
        );
        assert_eq!(
            profile["ports"][0]["thermal"]["activeThermalIndicator"]["extension"],
            11
        );
        assert_eq!(
            profile["ports"][1]["thermal"],
            serde_json::to_value(fan).unwrap()
        );
        assert_eq!(profile["ports"][0]["lighting"]["effect"], "static");
        assert_eq!(profile["ports"][1]["lighting"]["effect"], "wave");
        assert_eq!(
            profile["ports"][2],
            json!({"id":3,"name":"Logo","lighting":{"effect":"static"}})
        );
        assert_eq!(device.unwrap(), json!({"isCelsius":true,"extension":7}));
    }

    #[test]
    fn unit_is_device_scoped_and_preserves_profile_drafts() {
        let mut profile = Some(json!({"ports":[],"newerDraft":123}));
        let mut device = Some(json!({"unknown":{"keep":true}}));
        LocalChange::TemperatureUnit { celsius: false }
            .apply(&mut profile, &mut device)
            .unwrap();
        assert_eq!(temperature_unit(device.as_ref()).unwrap(), Some(false));
        assert_eq!(profile.unwrap(), json!({"ports":[],"newerDraft":123}));
        assert_eq!(device.unwrap()["unknown"], json!({"keep":true}));
        assert_eq!(temperature_unit(None).unwrap(), None);
        assert!(temperature_unit(Some(&json!({"isCelsius":"false"}))).is_err());
    }

    #[test]
    fn malformed_or_ambiguous_data_and_invalid_curves_remain_errors() {
        let thermal = ThermalProfile::default().fans;
        let change = LocalChange::Thermal {
            channel: CoolerChannel::Fans,
            thermal: thermal.clone(),
        };
        for value in [
            json!([]),
            json!({"ports":{}}),
            json!({"ports":[{"thermal":thermal}]}),
            json!({"ports":[{"name":"Fans","thermal":thermal},{"name":"Fans","thermal":thermal}]}),
        ] {
            assert!(change.apply(&mut Some(value), &mut None).is_err());
        }
        let mut invalid = thermal;
        invalid.advanced_setting[0] = f64::NAN;
        assert!(
            LocalChange::Thermal {
                channel: CoolerChannel::Fans,
                thermal: invalid
            }
            .apply(&mut None, &mut None)
            .is_err()
        );
    }
}

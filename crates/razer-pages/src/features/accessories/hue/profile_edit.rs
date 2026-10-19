use serde_json::{Value, json};

/// Source action payloads for the selected Hue profile.
#[derive(Clone, Debug)]
pub enum HueChanged {
    Brightness {
        is_enabled: bool,
        value: u8,
        is_global_brightness: Option<bool>,
    },
    QuickEffect {
        selected_effect: u32,
        setting: Value,
    },
    PortBrightness {
        region_id: u32,
        value: u8,
    },
}

impl HueChanged {
    pub(crate) fn apply(&self, profile: &mut Value) -> Result<bool, &'static str> {
        let fields = profile
            .as_object_mut()
            .ok_or("Hue source profile is not an object")?;
        match self {
            Self::Brightness {
                is_enabled,
                value,
                is_global_brightness,
            } => {
                let Some(brightness) = fields.get_mut("brightness") else {
                    return Ok(false);
                };
                let brightness = brightness
                    .as_object_mut()
                    .ok_or("Hue source brightness is not an object")?;
                brightness.insert("isEnabled".into(), json!(is_enabled));
                brightness.insert("value".into(), json!(value));
                if let Some(global) = is_global_brightness {
                    brightness.insert("isGlobalBrightness".into(), json!(global));
                }
            }
            Self::QuickEffect {
                selected_effect,
                setting,
            } => {
                if !setting.is_object() {
                    return Err("Hue quick-effect action setting is not an object");
                }
                // Current 30525 Rt creates this field only for ON_SET_EFFECTS.
                let effects = fields.entry("quickEffects").or_insert_with(|| json!({}));
                let effects = effects
                    .as_object_mut()
                    .ok_or("Hue source quick effects is not an object")?;
                effects.insert("selectedEffectId".into(), json!(selected_effect));
                effects.insert("selectedEffectSetting".into(), setting.clone());
                effects.insert("regionId".into(), json!(0));
            }
            Self::PortBrightness { region_id, value } => {
                let Some(ports) = fields.get_mut("ports") else {
                    return Ok(false);
                };
                let ports = ports
                    .as_array_mut()
                    .ok_or("Hue source ports is not an array")?;
                if let Some(port) = ports
                    .iter_mut()
                    .find(|port| port.get("id").and_then(Value::as_u64) == Some(*region_id as u64))
                {
                    port.as_object_mut()
                        .ok_or("Hue source port is not an object")?
                        .insert("brightness".into(), json!({"value": value}));
                } else {
                    // Current UI Op appends the supplied field object, without
                    // materializing an absent port identity or other settings.
                    ports.push(json!({"brightness": {"value": value}}));
                }
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_and_ports_do_not_materialize_absent_source_fields() {
        let mut profile = json!({"guid": "actual", "extension": {"keep": 7}});
        let before = profile.clone();
        for edit in [
            HueChanged::Brightness {
                is_enabled: true,
                value: 73,
                is_global_brightness: Some(false),
            },
            HueChanged::PortBrightness {
                region_id: 17,
                value: 64,
            },
        ] {
            assert!(!edit.apply(&mut profile).unwrap());
            assert_eq!(profile, before);
        }
        assert!(
            HueChanged::PortBrightness {
                region_id: 17,
                value: 64
            }
            .apply(&mut Value::Null)
            .is_err()
        );
    }

    #[test]
    fn explicit_effect_action_creates_only_its_field_and_preserves_extensions() {
        let mut profile = json!({"guid": "actual", "extension": 7});
        assert!(
            HueChanged::QuickEffect {
                selected_effect: 1,
                setting: json!({"color1": "#112233"}),
            }
            .apply(&mut profile)
            .unwrap()
        );
        assert_eq!(
            profile,
            json!({"guid": "actual", "extension": 7,
                "quickEffects": {"selectedEffectId": 1, "selectedEffectSetting": {"color1": "#112233"}, "regionId": 0}})
        );
        assert!(profile.get("brightness").is_none());
        assert!(profile.get("ports").is_none());
    }

    #[test]
    fn source_brightness_extensions_and_port_order_survive_independent_actions() {
        let mut profile = json!({
            "brightness": {"value": 21, "isEnabled": false, "isGlobalBrightness": true, "extension": 4},
            "ports": [{"id": 42, "brightness": {"value": 12}}, {"id": 17, "name": "Actual"}]
        });
        HueChanged::Brightness {
            is_enabled: true,
            value: 73,
            is_global_brightness: None,
        }
        .apply(&mut profile)
        .unwrap();
        HueChanged::PortBrightness {
            region_id: 17,
            value: 64,
        }
        .apply(&mut profile)
        .unwrap();
        assert_eq!(
            profile,
            json!({"brightness": {"value": 73, "isEnabled": true, "isGlobalBrightness": true, "extension": 4},
                "ports": [{"id": 42, "brightness": {"value": 12}}, {"id": 17, "name": "Actual", "brightness": {"value": 64}}]})
        );
        HueChanged::PortBrightness {
            region_id: 99,
            value: 38,
        }
        .apply(&mut profile)
        .unwrap();
        assert_eq!(profile["ports"][2], json!({"brightness": {"value": 38}}));
    }

    #[test]
    fn malformed_source_and_action_fields_are_preserved_on_failure() {
        for mut profile in [
            json!({"quickEffects": false}),
            json!({"brightness": false}),
            json!({"ports": false}),
        ] {
            let before = profile.clone();
            let edit = if profile.get("quickEffects").is_some() {
                HueChanged::QuickEffect {
                    selected_effect: 3,
                    setting: json!({}),
                }
            } else if profile.get("brightness").is_some() {
                HueChanged::Brightness {
                    is_enabled: true,
                    value: 73,
                    is_global_brightness: None,
                }
            } else {
                HueChanged::PortBrightness {
                    region_id: 17,
                    value: 64,
                }
            };
            assert!(edit.apply(&mut profile).is_err());
            assert_eq!(profile, before);
        }
    }
}

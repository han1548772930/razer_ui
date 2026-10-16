//! Current hardware parsers and84910 notification handlers. Keep original bytes
//! in Event; these values are observations, never device-write acknowledgements.
use super::{Event, Protocol, capability};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatteryStatus {
    level: u8,
    charging_status: &'static str,
}
impl BatteryStatus {
    fn from_packet(level: u8, status: u8) -> Self {
        Self {
            level: (u16::from(level) * 100 / 255) as u8,
            // Current84910 explicitly falls back to NoCharge_BatteryFull for
            // unknown enum values. The original raw code remains in Event.
            charging_status: match status {
                1 => "Charging",
                2 => "off",
                _ => "NoCharge_BatteryFull",
            },
        }
    }
    pub fn level(&self) -> u8 {
        self.level
    }
    pub fn charging_status(&self) -> &'static str {
        self.charging_status
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lighting {
    effect_id: u8,
    flags: u8,
    rate: u8,
    color: [u8; 3],
    color_count: Option<u8>,
}
impl Lighting {
    pub fn effect_id(&self) -> u8 {
        self.effect_id
    }
    pub fn flags(&self) -> u8 {
        self.flags
    }
    pub fn rate(&self) -> u8 {
        self.rate
    }
    pub fn color(&self) -> [u8; 3] {
        self.color
    }
    pub fn color_count(&self) -> Option<u8> {
        self.color_count
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MappingInput {
    /// Standard parsers look up BUTTON_M<number> in the product mapping table.
    MButtonNumber(u8),
    /// Record05 looks up the entire firmware ID in that same table.
    FirmwareId(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notification {
    LightingEffect(Lighting),
    LightingAnimationSpeed(Lighting),
    LightingColor(Lighting),
    Brightness { percent: u8 },
    Haptics { strength: u8 },
    Mapping { input: MappingInput, assignment: u8 },
    ProfileSlot { slot: u8 },
    Battery(BatteryStatus),
    Wireless { connected: bool },
    Socd { mode: u8 },
    RestoreDefaults { mode: u8 },
}

impl Event {
    /// Decode only the complete current product frame. Metadata and payload must
    /// agree, including when the Event crossed the local service IPC boundary.
    pub fn notification(&self, product_id: u32) -> Option<Notification> {
        let source = capability(product_id)?;
        let Self::Notification {
            event_id,
            record_id,
            buffer,
        } = self
        else {
            return None;
        };
        if buffer.first() != Some(record_id) {
            return None;
        }
        if source.protocol == Protocol::Record05 {
            if *record_id != 5 || buffer.len() != 16 || buffer[1] != *event_id {
                return None;
            }
            return match event_id {
                1 => Some(Notification::ProfileSlot { slot: buffer[2] }),
                32 => Some(Notification::Haptics {
                    strength: buffer[2],
                }),
                49 if product_id == 2676 => Some(Notification::Battery(
                    BatteryStatus::from_packet(buffer[2], buffer[3]),
                )),
                97 => Some(Notification::Mapping {
                    input: MappingInput::FirmwareId(buffer[2]),
                    assignment: buffer[3],
                }),
                _ => None,
            };
        }
        let arcade = source.protocol == Protocol::PlaystationArcade;
        let playstation = source.protocol == Protocol::Playstation;
        if *record_id == 0
            || if playstation {
                buffer.len() < 64
            } else {
                buffer.len() != 64
            }
            || buffer[if arcade { 8 } else { 9 }] != *event_id
        {
            return None;
        }
        let value_offset = if arcade { 9 } else { 10 };
        match event_id {
            1 | 2 | 4 => {
                let lighting = Lighting {
                    effect_id: buffer[value_offset],
                    flags: buffer[value_offset + 1],
                    rate: buffer[value_offset + 2],
                    color: [buffer[13], buffer[14], buffer[15]],
                    color_count: arcade.then_some(buffer[12]),
                };
                Some(match event_id {
                    1 => Notification::LightingEffect(lighting),
                    2 => Notification::LightingAnimationSpeed(lighting),
                    _ => Notification::LightingColor(lighting),
                })
            }
            3 => Some(Notification::Brightness {
                // Kitsune50462 uses floor;85275/15706/33807 use ceil.
                percent: ((u16::from(buffer[16]) * 100 + if arcade { 0 } else { 254 }) / 255) as u8,
            }),
            5 if !arcade => Some(Notification::Haptics {
                strength: buffer[10],
            }),
            6 if !arcade => Some(Notification::Mapping {
                input: MappingInput::MButtonNumber(buffer[10]),
                assignment: buffer[11],
            }),
            7 if !arcade => Some(Notification::ProfileSlot { slot: buffer[10] }),
            8 if !arcade => Some(Notification::Battery(BatteryStatus::from_packet(
                buffer[10], buffer[11],
            ))),
            9 if !arcade && !playstation => Some(Notification::Wireless {
                connected: buffer[10] == 3,
            }),
            10 if playstation => Some(Notification::Wireless {
                // Source33807 translates RF1 into49065.CONNECT_EVENT3.
                connected: buffer[10] == 1,
            }),
            13 if arcade || playstation => Some(Notification::Socd {
                mode: buffer[value_offset],
            }),
            14 if arcade || playstation => Some(Notification::RestoreDefaults {
                mode: buffer[value_offset],
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MappingInput, Notification};
    use crate::controller_input::{Event, Parser, capability};
    use serde_json::json;

    fn event(product: u32, id: u8, value: u8, next: u8) -> Event {
        let record05 = matches!(product, 2676 | 2684);
        let mut frame = vec![0; if record05 { 16 } else { 64 }];
        frame[0] = if record05 { 5 } else { 1 };
        let offset = if record05 {
            1
        } else if product == 4115 {
            8
        } else {
            9
        };
        frame[offset] = id;
        frame[offset + 1] = value;
        frame[offset + 2] = next;
        Parser::new(product, product)
            .unwrap()
            .parse(&json!(frame))
            .unwrap()
            .remove(0)
    }

    #[test]
    fn controller_input_capability_includes_all_nine_products_without_adding_trigger_pages() {
        for pid in [2629, 2636, 2647, 2650, 2676, 2684, 4115, 4133, 4144] {
            assert!(capability(pid).unwrap().accepts(pid));
            assert!(!capability(pid).unwrap().accepts(9999));
            assert_eq!(
                capability(pid).unwrap().is_battery_supported(),
                matches!(pid, 2636 | 2647 | 2676 | 4133 | 4144)
            );
        }
        assert_eq!(Parser::new(2650, 2650).unwrap().interval_ms(), Some(60));
        assert_eq!(Parser::new(4115, 4115).unwrap().interval_ms(), None);
        assert!(crate::controller_triggers::capability(2650).is_none());
        assert!(crate::controller_triggers::capability(4115).is_none());
    }

    #[test]
    fn battery_notifications_keep_source_floor_and_charging_enum_and_product_branches() {
        for product in [2636, 2647, 2676, 4133, 4144] {
            for (raw, percent) in [(0, 0), (1, 0), (128, 50), (254, 99), (255, 100)] {
                for (status, expected) in [
                    (0, "NoCharge_BatteryFull"),
                    (1, "Charging"),
                    (2, "off"),
                    (255, "NoCharge_BatteryFull"),
                ] {
                    let event = event(product, if product == 2676 { 49 } else { 8 }, raw, status);
                    let Some(Notification::Battery(reading)) = event.notification(product) else {
                        panic!("battery event lost")
                    };
                    assert_eq!(reading.level(), percent);
                    assert_eq!(reading.charging_status(), expected);
                }
            }
        }
        assert!(event(2684, 49, 128, 1).notification(2684).is_none());
        assert!(event(4115, 8, 128, 1).notification(4115).is_none());
    }

    #[test]
    fn arcade_prefix_offsets_and_brightness_rounding_differ_from_gamepads() {
        for (product, expected) in [(2636, 1), (4115, 0)] {
            let mut frame = vec![0; 64];
            frame[0] = 1;
            frame[if product == 4115 { 8 } else { 9 }] = 3;
            frame[16] = 1;
            if product == 4115 {
                frame.insert(0, 0);
            }
            let event = Parser::new(product, product)
                .unwrap()
                .parse(&json!(frame))
                .unwrap()
                .remove(0);
            assert_eq!(
                event.notification(product),
                Some(Notification::Brightness { percent: expected })
            );
        }
    }

    #[test]
    fn profile_mapping_and_rf_values_are_decoded_by_product_protocol() {
        assert_eq!(
            event(2636, 7, 3, 0).notification(2636),
            Some(Notification::ProfileSlot { slot: 3 })
        );
        assert_eq!(
            event(2676, 1, 5, 0).notification(2676),
            Some(Notification::ProfileSlot { slot: 5 })
        );
        assert_eq!(
            event(2636, 6, 2, 17).notification(2636),
            Some(Notification::Mapping {
                input: MappingInput::MButtonNumber(2),
                assignment: 17
            })
        );
        assert_eq!(
            event(2684, 97, 20, 1).notification(2684),
            Some(Notification::Mapping {
                input: MappingInput::FirmwareId(20),
                assignment: 1
            })
        );
        for (product, id, connected_value) in [(2636, 9, 3), (4144, 10, 1)] {
            assert_eq!(
                event(product, id, connected_value, 0).notification(product),
                Some(Notification::Wireless { connected: true })
            );
            assert_eq!(
                event(product, id, 0, 0).notification(product),
                Some(Notification::Wireless { connected: false })
            );
        }
        assert!(event(4133, 9, 3, 0).notification(4133).is_none());
    }

    #[test]
    fn mismatched_ipc_metadata_and_incomplete_frames_cannot_become_observations() {
        let mut input = event(2636, 8, 255, 1);
        if let Event::Notification { event_id, .. } = &mut input {
            *event_id = 7;
        }
        assert!(input.notification(2636).is_none());
        let mut input = event(2676, 49, 255, 1);
        if let Event::Notification { buffer, .. } = &mut input {
            buffer.pop();
        }
        assert!(input.notification(2676).is_none());
        assert!(event(2636, 255, 0, 0).notification(2636).is_none());
        assert!(event(2636, 8, 255, 1).notification(9999).is_none());
    }
}

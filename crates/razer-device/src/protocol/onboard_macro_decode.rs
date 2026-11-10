//! Source OBM macro item decoding, including its stateful mouse records and
//! the narrowly defined missing-final-key-up firmware compensation.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Item {
    Keyboard {
        hid: u16,
        scancode: Option<u32>,
        key_name: Option<String>,
        flag: u8,
    },
    Mouse {
        id: Option<u32>,
        data: Option<i64>,
    },
    Delay {
        milliseconds: f64,
    },
    Cursor {
        x: f64,
        y: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stop {
    UnsupportedOpcode {
        offset: usize,
        opcode: u8,
    },
    TruncatedItem {
        offset: usize,
        expected: usize,
        available: usize,
    },
    MissingBytes {
        offset: usize,
        remaining: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Decoded {
    pub items: Vec<Item>,
    pub consumed_bytes: usize,
    pub declared_bytes: u32,
    pub repaired_key_up: Option<u8>,
    pub stopped: Option<Stop>,
}

fn keyboard(product: u32, hid: u8, flag: u8) -> Result<Item> {
    let keys = crate::onboard_macro_codec::keyboard_keys(product)?;
    let key_name = match hid {
        102 => Some("KEY_POWER"),
        103 => Some("KEY_NUMPAD_EQUAL"),
        133 => Some("KEY_NUMPAD_COMMA"),
        140 => Some("KEY_NUMPAD_JPCOMMA"),
        144 => Some("KEY_HANGEUL"),
        145 => Some("KEY_HANJA"),
        146 => Some("KEY_KATAKANA"),
        147 => Some("KEY_HIRAGANA"),
        148 => Some("KEY_ZENKAKU_HANKAKU"),
        231 => Some("KEY_RIGHT_GUI"),
        // Directional-keypad diagonals exist in the worker table but are not
        // entries in the firmware macro HID-to-name table.
        232..=235 => None,
        _ => keys
            .iter()
            .find(|key| key.hid.and_then(|id| id.parse::<u16>().ok()) == Some(u16::from(hid)))
            .and_then(|key| key.input_id),
    }
    .map(str::to_owned);
    Ok(Item::Keyboard {
        hid: u16::from(hid),
        scancode: crate::keyboard_magnetic_mapping::macro_scancode(u16::from(hid)),
        key_name,
        flag,
    })
}

fn mouse(id: Option<u32>) -> Item {
    Item::Mouse { id, data: None }
}

pub fn keyboard_items(product: u32, bytes: &[u8], declared_bytes: u32) -> Result<Decoded> {
    ensure!(
        crate::keyboard_profile_inventory::supports(product),
        "Macro decoder product has no current keyboard OBM source"
    );
    let declared = usize::try_from(declared_bytes)?;
    ensure!(
        bytes.len() <= declared,
        "Macro bytes exceed the declared source size"
    );
    let mut result = Decoded {
        items: Vec::new(),
        consumed_bytes: 0,
        declared_bytes,
        repaired_key_up: None,
        stopped: None,
    };
    let mut held_keys = Vec::new();
    let mut held_mouse = 0u8;
    while result.consumed_bytes < declared {
        let offset = result.consumed_bytes;
        let remaining = &bytes[offset..];
        if remaining.is_empty() {
            if offset + 2 == declared && held_keys.len() == 1 {
                let hid = held_keys[0];
                result.items.push(keyboard(product, hid, 1)?);
                result.repaired_key_up = Some(hid);
            } else {
                result.stopped = Some(Stop::MissingBytes {
                    offset,
                    remaining: declared - offset,
                });
            }
            break;
        }
        let size = match remaining[0] {
            1 | 2 | 8 | 10 | 17 => 2,
            18 => 3,
            19 => 4,
            20 | 21 => 5,
            opcode => {
                result.stopped = Some(Stop::UnsupportedOpcode { offset, opcode });
                break;
            }
        };
        if remaining.len() < size {
            result.stopped = Some(Stop::TruncatedItem {
                offset,
                expected: size,
                available: remaining.len(),
            });
            break;
        }
        let mut prepend = None;
        let item = match remaining[0] {
            opcode @ (1 | 2) => {
                let hid = remaining[1];
                if opcode == 1 {
                    held_keys.push(hid);
                } else if let Some(position) = held_keys.iter().position(|key| *key == hid) {
                    held_keys.remove(position);
                }
                keyboard(product, hid, u8::from(opcode == 2))?
            }
            8 => {
                let next = remaining[1];
                let mut pressed = None;
                for (mask, down, up) in [
                    (1, 1, 2),
                    (2, 4, 8),
                    (4, 16, 32),
                    (8, 64, 128),
                    (16, 256, 512),
                ] {
                    if next & mask != 0 {
                        if held_mouse & mask == 0 {
                            pressed = Some(down);
                            // The source merges the entire new byte here, so
                            // later newly pressed bits do not emit another DOWN.
                            held_mouse |= next;
                        }
                    } else if held_mouse & mask != 0 {
                        prepend = Some(up);
                        held_mouse &= !mask;
                    }
                }
                if pressed.is_none() && prepend.is_some() {
                    pressed = prepend.take();
                }
                mouse(pressed)
            }
            10 => Item::Mouse {
                id: Some(1024),
                data: match remaining[1] {
                    1 => Some(120),
                    255 => Some(65416),
                    _ => None,
                },
            },
            17 => Item::Delay {
                milliseconds: f64::from(remaining[1]),
            },
            18 => Item::Delay {
                milliseconds: f64::from(u16::from_be_bytes(remaining[1..3].try_into()?)),
            },
            19 => Item::Delay {
                milliseconds: f64::from(u32::from_be_bytes([
                    0,
                    remaining[1],
                    remaining[2],
                    remaining[3],
                ])),
            },
            20 => Item::Delay {
                milliseconds: f64::from(i32::from_be_bytes(remaining[1..5].try_into()?)),
            },
            21 => Item::Cursor {
                x: f64::from(u16::from_be_bytes(remaining[1..3].try_into()?)),
                y: f64::from(u16::from_be_bytes(remaining[3..5].try_into()?)),
            },
            _ => unreachable!(),
        };
        if let Some(id) = prepend {
            result.items.push(mouse(Some(id)));
        }
        result.items.push(item);
        // The original checks the complete remaining byte slice, rather than
        // releasing every held button whenever the stream happens to end.
        if remaining.len() == 2 && held_mouse != 0 {
            if let Some((_, id)) = [(1, 2), (2, 8), (4, 32), (8, 128), (16, 512)]
                .into_iter()
                .find(|(mask, _)| held_mouse & mask != 0)
            {
                result.items.push(mouse(Some(id)));
            }
            held_mouse = 0;
        }
        result.consumed_bytes += size;
    }
    Ok(result)
}

/// The worker compares only these fields. Mouse wheel data and key labels do
/// not participate; a cursor differs only when both coordinates differ.
pub fn differs(user: &[Item], onboard: &[Item]) -> bool {
    user.len() != onboard.len()
        || user
            .iter()
            .zip(onboard)
            .any(|(user, onboard)| match (user, onboard) {
                (
                    Item::Keyboard {
                        hid: a,
                        scancode: sa,
                        flag: fa,
                        ..
                    },
                    Item::Keyboard {
                        hid: b,
                        scancode: sb,
                        flag: fb,
                        ..
                    },
                ) => (a != b && sa != sb) || fa != fb,
                (Item::Mouse { id: a, .. }, Item::Mouse { id: b, .. }) => a != b,
                (Item::Delay { milliseconds: a }, Item::Delay { milliseconds: b }) => a != b,
                (Item::Cursor { x: ax, y: ay }, Item::Cursor { x: bx, y: by }) => {
                    ax != bx && ay != by
                }
                _ => true,
            })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_mouse_state_preserves_suppressed_edges_and_last_record_release() {
        let result = keyboard_items(739, &[8, 3, 8, 0, 8, 1, 1, 4], 10).unwrap();
        assert_eq!(
            result.items,
            vec![
                mouse(Some(1)),
                mouse(Some(8)),
                mouse(Some(1)),
                Item::Keyboard {
                    hid: 4,
                    scancode: Some(30),
                    key_name: Some("KEY_A".into()),
                    flag: 0
                },
                mouse(Some(2)),
                Item::Keyboard {
                    hid: 4,
                    scancode: Some(30),
                    key_name: Some("KEY_A".into()),
                    flag: 1
                },
            ]
        );
        assert_eq!(result.repaired_key_up, Some(4));
        assert_eq!(result.consumed_bytes, 8);
        assert!(result.stopped.is_none());
    }

    #[test]
    fn four_byte_delay_is_signed_and_unknown_wheel_data_stays_absent() {
        let result = keyboard_items(
            747,
            &[20, 255, 255, 255, 255, 10, 3, 21, 128, 1, 255, 254],
            12,
        )
        .unwrap();
        assert_eq!(
            result.items,
            [
                Item::Delay { milliseconds: -1.0 },
                Item::Mouse {
                    id: Some(1024),
                    data: None
                },
                Item::Cursor {
                    x: 32769.0,
                    y: 65534.0
                },
            ]
        );
        assert!(result.stopped.is_none());
    }

    #[test]
    fn missing_bytes_only_repair_one_outstanding_final_key_up() {
        let incomplete = keyboard_items(679, &[1, 4, 1, 5], 6).unwrap();
        assert!(incomplete.repaired_key_up.is_none());
        assert_eq!(
            incomplete.stopped,
            Some(Stop::MissingBytes {
                offset: 4,
                remaining: 2
            })
        );
        let unknown = keyboard_items(679, &[1, 4, 3, 0, 2, 4], 6).unwrap();
        assert_eq!(
            unknown.stopped,
            Some(Stop::UnsupportedOpcode {
                offset: 2,
                opcode: 3
            })
        );
        let truncated = keyboard_items(679, &[21, 1], 2).unwrap();
        assert_eq!(
            truncated.stopped,
            Some(Stop::TruncatedItem {
                offset: 0,
                expected: 5,
                available: 2
            })
        );
    }

    #[test]
    fn source_difference_uses_conjunctions_and_ignores_mouse_wheel_payload() {
        let key = |hid, scancode, flag| Item::Keyboard {
            hid,
            scancode,
            flag,
            key_name: None,
        };
        assert!(!differs(&[key(50, Some(43), 0)], &[key(49, Some(43), 0)]));
        assert!(!differs(&[key(4, Some(30), 0)], &[key(4, Some(31), 0)]));
        assert!(differs(&[key(4, Some(30), 0)], &[key(5, Some(48), 0)]));
        assert!(differs(&[key(4, Some(30), 0)], &[key(4, Some(30), 1)]));
        assert!(!differs(
            &[Item::Mouse {
                id: Some(1024),
                data: Some(120)
            }],
            &[Item::Mouse {
                id: Some(1024),
                data: Some(65416)
            }]
        ));
        assert!(!differs(
            &[Item::Cursor { x: 1.0, y: 2.0 }],
            &[Item::Cursor { x: 1.0, y: 9.0 }]
        ));
        assert!(differs(
            &[Item::Cursor { x: 1.0, y: 2.0 }],
            &[Item::Cursor { x: 8.0, y: 9.0 }]
        ));
        assert!(differs(
            &[Item::Delay { milliseconds: 1.5 }],
            &[Item::Delay { milliseconds: 1.0 }]
        ));
        assert!(differs(&[], &[Item::Delay { milliseconds: 1.0 }]));
    }

    #[test]
    fn firmware_name_table_keeps_its_extra_keys_and_excludes_worker_diagonals() {
        let result =
            keyboard_items(739, &[1, 148, 2, 148, 1, 231, 2, 231, 1, 233, 2, 233], 12).unwrap();
        assert!(
            matches!(&result.items[0], Item::Keyboard { key_name: Some(name), .. } if name == "KEY_ZENKAKU_HANKAKU")
        );
        assert!(
            matches!(&result.items[2], Item::Keyboard { key_name: Some(name), .. } if name == "KEY_RIGHT_GUI")
        );
        assert!(matches!(
            &result.items[4],
            Item::Keyboard {
                key_name: None,
                scancode: None,
                ..
            }
        ));
    }
}

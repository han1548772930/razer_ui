//! The source picker has sixteen UI slots; profile writes omit empty slots.
use super::CustomColorSlots;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
struct PaletteColor {
    source: String,
    rgb: Option<[u8; 3]>,
}
impl TryFrom<String> for PaletteColor {
    type Error = String;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        let rgb = if source == "#111" {
            None
        } else {
            let digits = source
                .strip_prefix('#')
                .filter(|digits| {
                    matches!(digits.len(), 3 | 6)
                        && digits.bytes().all(|digit| digit.is_ascii_hexdigit())
                })
                .ok_or("Invalid custom RGB color")?;
            let value = u32::from_str_radix(digits, 16).expect("validated hexadecimal color");
            Some(if digits.len() == 3 {
                [
                    ((value >> 8) as u8 & 15) * 17,
                    ((value >> 4) as u8 & 15) * 17,
                    (value as u8 & 15) * 17,
                ]
            } else {
                [(value >> 16) as u8, (value >> 8) as u8, value as u8]
            })
        };
        Ok(Self { source, rgb })
    }
}
impl From<PaletteColor> for String {
    fn from(color: PaletteColor) -> Self {
        color.source
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SourceCustomColor {
    color: PaletteColor,
    #[serde(skip_serializing_if = "Option::is_none")]
    picked: Option<bool>,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}
impl SourceCustomColor {
    fn empty() -> Self {
        Self {
            color: PaletteColor {
                source: "#111".into(),
                rgb: None,
            },
            picked: Some(false),
            extensions: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CustomColorEdit {
    Set { slot: usize, color: [u8; 3] },
    Delete { slot: usize },
}

/// Active-profile palette data and external extension fields. This is separate
/// from the application's shared palette and any device write acknowledgement.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(try_from = "Vec<SourceCustomColor>", into = "Vec<SourceCustomColor>")]
pub struct ProfileCustomColors {
    entries: Vec<SourceCustomColor>,
    filter_empty: bool,
}
impl TryFrom<Vec<SourceCustomColor>> for ProfileCustomColors {
    type Error = String;

    fn try_from(entries: Vec<SourceCustomColor>) -> Result<Self, Self::Error> {
        if entries.len() > 16 {
            return Err("Custom palette exceeds the source picker's sixteen slots".into());
        }
        Ok(Self {
            entries,
            filter_empty: false,
        })
    }
}
impl From<ProfileCustomColors> for Vec<SourceCustomColor> {
    fn from(mut colors: ProfileCustomColors) -> Self {
        if colors.filter_empty {
            colors.entries.retain(|entry| entry.color.rgb.is_some());
        }
        colors.entries
    }
}
impl ProfileCustomColors {
    pub fn from_slots(slots: CustomColorSlots) -> Self {
        let entries = slots
            .into_iter()
            .map(|color| match color {
                Some(rgb) => SourceCustomColor {
                    color: PaletteColor {
                        source: format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]),
                        rgb: Some(rgb),
                    },
                    picked: Some(true),
                    extensions: BTreeMap::new(),
                },
                None => SourceCustomColor::empty(),
            })
            .collect();
        Self {
            entries,
            filter_empty: false,
        }
    }

    pub fn slots(&self) -> CustomColorSlots {
        std::array::from_fn(|index| self.entries.get(index).and_then(|entry| entry.color.rgb))
    }

    /// UI messages contain all sixteen records. Middleware persistence can
    /// filter empty records without discarding source record metadata here.
    pub fn slot_records(&self) -> impl Serialize {
        let mut entries = self.entries.clone();
        entries.resize_with(16, SourceCustomColor::empty);
        entries
    }

    /// Current taskMakerSetCustomColorsSetting filters the #111 empty sentinel
    /// before saving. The UI retains its slot objects, including extensions,
    /// while an edit replaces the color or a deletion shifts those objects.
    pub fn apply_edit(&mut self, edit: CustomColorEdit) -> Result<(), String> {
        let slot = match edit {
            CustomColorEdit::Set { slot, .. } | CustomColorEdit::Delete { slot } => slot,
        };
        if slot >= 16 {
            return Err("Custom palette slot exceeds the source picker".into());
        }
        self.entries.resize_with(16, SourceCustomColor::empty);
        for entry in &mut self.entries {
            entry.picked = Some(entry.color.rgb.is_some());
        }
        match edit {
            CustomColorEdit::Set { color: rgb, .. } => {
                let entry = &mut self.entries[slot];
                entry.color = PaletteColor {
                    source: format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]),
                    rgb: Some(rgb),
                };
                entry.picked = Some(true);
            }
            CustomColorEdit::Delete { .. } => {
                self.entries.remove(slot);
                self.entries.push(SourceCustomColor::empty());
            }
        }
        self.filter_empty = true;
        Ok(())
    }

    /// Current synchronization compares the source color strings, preserving
    /// the existing records and appending colors absent from the initial set.
    pub fn merge_synchronized(&mut self, incoming: Self) -> Result<(), String> {
        let mut current: Vec<SourceCustomColor> = self.clone().into();
        let known = current
            .iter()
            .map(|entry| entry.color.source.as_str())
            .collect::<BTreeSet<_>>();
        let incoming: Vec<SourceCustomColor> = incoming.into();
        let added = incoming
            .into_iter()
            .filter(|entry| !known.contains(entry.color.source.as_str()))
            .collect::<Vec<_>>();
        if current.len() + added.len() > 16 {
            return Err("Synchronized custom palette exceeds sixteen slots".into());
        }
        current.extend(added);
        self.entries = current;
        self.filter_empty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{CustomColorEdit, ProfileCustomColors};
    use serde_json::json;

    #[test]
    fn profile_palette_keeps_ui_slots_separate_from_filtered_persistence() {
        let mut palette: ProfileCustomColors = serde_json::from_value(json!([
            {"color":"#AABBCC","picked":false,"record":1},
            {"color":"#111","picked":false},
            {"color":"#aabbcc","picked":true,"record":2}
        ]))
        .unwrap();
        let slots = palette.slots();
        assert_eq!(slots[0], Some([170, 187, 204]));
        assert_eq!(slots[1], None);
        assert_eq!(slots[2], slots[0]);
        palette
            .apply_edit(CustomColorEdit::Set {
                slot: 2,
                color: [17, 34, 51],
            })
            .unwrap();
        assert_eq!(palette.slots()[0], slots[0]);
        assert_eq!(palette.slots()[1], None);
        assert_eq!(palette.slots()[2], Some([17, 34, 51]));
        assert_eq!(
            serde_json::to_value(palette).unwrap(),
            json!([
                {"color":"#AABBCC","picked":true,"record":1},
                {"color":"#112233","picked":true,"record":2}
            ])
        );
    }

    #[test]
    fn deleting_duplicate_colors_shifts_the_source_records_without_reassigning_metadata() {
        let mut palette: ProfileCustomColors = serde_json::from_value(json!([
            {"color":"#112233","picked":true,"record":1},
            {"color":"#112233","picked":true,"record":2}
        ]))
        .unwrap();
        palette
            .apply_edit(CustomColorEdit::Delete { slot: 0 })
            .unwrap();
        assert_eq!(palette.slots()[0], Some([17, 34, 51]));
        assert_eq!(palette.slots()[1], None);
        assert_eq!(
            serde_json::to_value(&palette).unwrap(),
            json!([
                {"color":"#112233","picked":true,"record":2}
            ])
        );
        let before = serde_json::to_value(&palette).unwrap();
        assert!(
            palette
                .apply_edit(CustomColorEdit::Delete { slot: 16 })
                .is_err()
        );
        assert_eq!(serde_json::to_value(palette).unwrap(), before);
    }

    #[test]
    fn synchronized_palette_uses_source_identity_and_rejects_overflow_atomically() {
        let mut palette: ProfileCustomColors = serde_json::from_value(json!([
            {"color":"#aabbcc","picked":false,"extension":7}
        ]))
        .unwrap();
        palette
            .merge_synchronized(
                serde_json::from_value(json!([
                    {"color":"#aabbcc","picked":true},
                    {"color":"#AABBCC","picked":true}
                ]))
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(&palette).unwrap(),
            json!([
                {"color":"#aabbcc","picked":false,"extension":7},
                {"color":"#AABBCC","picked":true}
            ])
        );
        let before = serde_json::to_value(&palette).unwrap();
        let incoming = serde_json::from_value(json!(vec![json!({"color":"#112233"}); 16])).unwrap();
        assert!(palette.merge_synchronized(incoming).is_err());
        assert_eq!(serde_json::to_value(palette).unwrap(), before);
    }

    #[test]
    fn malformed_profile_palette_is_explicit() {
        for invalid in [
            json!([{}]),
            json!([{"color":"#12"}]),
            json!([{"color":"#112233","picked":"true"}]),
            json!(vec![json!({"color":"#112233"}); 17]),
        ] {
            assert!(serde_json::from_value::<ProfileCustomColors>(invalid).is_err());
        }
    }

    #[test]
    fn ui_slot_messages_preserve_shorthand_duplicate_records_and_the_empty_slots() {
        let mut palette: ProfileCustomColors = serde_json::from_value(json!([
            {"color":"#abc","picked":true,"record":1},
            {"color":"#aabbcc","picked":true,"record":2}
        ]))
        .unwrap();
        assert_eq!(palette.slots()[0], Some([170, 187, 204]));
        palette
            .apply_edit(CustomColorEdit::Delete { slot: 0 })
            .unwrap();
        let records = serde_json::to_value(palette.slot_records()).unwrap();
        assert_eq!(records.as_array().unwrap().len(), 16);
        assert_eq!(
            records[0],
            json!({"color":"#aabbcc","picked":true,"record":2})
        );
        assert_eq!(records[1], json!({"color":"#111","picked":false}));
        assert_eq!(
            serde_json::to_value(palette).unwrap(),
            json!([
                {"color":"#aabbcc","picked":true,"record":2}
            ])
        );
    }
}

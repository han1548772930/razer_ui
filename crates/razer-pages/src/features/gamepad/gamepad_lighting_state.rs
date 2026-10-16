//! Current 2636 useOm / controllerLightingEffectReducer and MW 70784.
//! UI selection stays optimistic; protocol results are not stored as settings.
use anyhow::ensure;
use razer_device::controller_lighting::{ControllerEffect, ObservedEffect, SpectrumRate};
use serde::{Deserialize, Serialize};

pub(super) const PRESET_COLORS: [[u8; 3]; 8] = [
    [0, 255, 0],
    [68, 215, 182],
    [0, 25, 255],
    [127, 0, 255],
    [255, 0, 0],
    [250, 100, 0],
    [247, 181, 0],
    [255, 255, 255],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "u8", into = "u8")]
pub(super) enum EffectId {
    Off,
    Static,
    Spectrum,
}
impl TryFrom<u8> for EffectId {
    type Error = anyhow::Error;
    fn try_from(id: u8) -> anyhow::Result<Self> {
        match id {
            0 => Ok(Self::Off),
            1 => Ok(Self::Static),
            3 => Ok(Self::Spectrum),
            _ => anyhow::bail!("Invalid source controller effect identifier"),
        }
    }
}
impl From<EffectId> for u8 {
    fn from(id: EffectId) -> Self {
        match id {
            EffectId::Off => 0,
            EffectId::Static => 1,
            EffectId::Spectrum => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum ColorMode {
    Preset,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub(super) struct HexColor(pub(super) [u8; 3]);
impl TryFrom<String> for HexColor {
    type Error = anyhow::Error;
    fn try_from(text: String) -> anyhow::Result<Self> {
        let digits = text
            .strip_prefix('#')
            .ok_or_else(|| anyhow::anyhow!("Controller color requires a hexadecimal prefix"))?;
        ensure!(
            digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid controller RGB color"
        );
        let value = u32::from_str_radix(digits, 16)?;
        Ok(Self([(value >> 16) as u8, (value >> 8) as u8, value as u8]))
    }
}
impl From<HexColor> for String {
    fn from(color: HexColor) -> Self {
        format!("#{:02x}{:02x}{:02x}", color.0[0], color.0[1], color.0[2])
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ColorEntry {
    pub(super) identity: u64,
    pub(super) index: u8,
    pub(super) value: HexColor,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum SourceColor {
    Indexed { value: HexColor, index: u8 },
    Plain(HexColor),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceSettings {
    effect_id: EffectId,
    mode: ColorMode,
    index: u8,
    color_list: Vec<SourceColor>,
    rate: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "SourceSettings", into = "SourceSettings")]
pub(super) struct ControllerLightingSettings {
    pub(super) effect_id: EffectId,
    pub(super) mode: ColorMode,
    pub(super) index: u8,
    pub(super) color_list: Vec<ColorEntry>,
    pub(super) rate: u8,
    next_color_identity: u64,
}
impl Default for ControllerLightingSettings {
    fn default() -> Self {
        Self {
            effect_id: EffectId::Spectrum,
            mode: ColorMode::Preset,
            index: 0,
            rate: 2,
            color_list: Self::presets(),
            next_color_identity: 8,
        }
    }
}
impl TryFrom<SourceSettings> for ControllerLightingSettings {
    type Error = anyhow::Error;
    fn try_from(source: SourceSettings) -> anyhow::Result<Self> {
        ensure!(
            source.color_list.len() <= 16 && source.index < 16 && source.rate <= 3,
            "Saved controller lighting exceeds its source limits"
        );
        let mut seen = [false; 16];
        let color_list = source
            .color_list
            .into_iter()
            .enumerate()
            .map(|(identity, color)| {
                let (index, value) = match color {
                    SourceColor::Plain(value) => (identity as u8, value),
                    SourceColor::Indexed { value, index } => (index, value),
                };
                ensure!(
                    index < 16 && !seen[usize::from(index)],
                    "Invalid or duplicate controller palette index"
                );
                seen[usize::from(index)] = true;
                Ok(ColorEntry {
                    identity: identity as u64,
                    index,
                    value,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Self {
            effect_id: source.effect_id,
            mode: source.mode,
            index: source.index,
            next_color_identity: color_list.len() as u64,
            color_list,
            rate: source.rate,
        })
    }
}
impl From<ControllerLightingSettings> for SourceSettings {
    fn from(settings: ControllerLightingSettings) -> Self {
        Self {
            effect_id: settings.effect_id,
            mode: settings.mode,
            index: settings.index,
            color_list: settings
                .color_list
                .into_iter()
                .map(|color| SourceColor::Indexed {
                    value: color.value,
                    index: color.index,
                })
                .collect(),
            rate: settings.rate,
        }
    }
}
impl ControllerLightingSettings {
    fn presets() -> Vec<ColorEntry> {
        PRESET_COLORS
            .into_iter()
            .enumerate()
            .map(|(index, color)| ColorEntry {
                identity: index as u64,
                index: index as u8,
                value: HexColor(color),
            })
            .collect()
    }
    pub(super) fn protocol_effect(&self) -> anyhow::Result<ControllerEffect> {
        let effect = match self.effect_id {
            EffectId::Off => ControllerEffect::Off,
            EffectId::Spectrum => ControllerEffect::Spectrum {
                rate: SpectrumRate::try_from(self.rate)?,
            },
            EffectId::Static => ControllerEffect::Static {
                index: self.index,
                custom_colors: self
                    .color_list
                    .iter()
                    .filter(|color| color.index >= 8)
                    .map(|color| color.value.0)
                    .collect(),
            },
        };
        effect.validate()?;
        Ok(effect)
    }
    pub(super) fn select(&mut self, identity: u64) {
        if let Some(color) = self
            .color_list
            .iter()
            .find(|color| color.identity == identity)
        {
            self.index = color.index;
            // Current useOm intentionally marks preset swatch clicks CUSTOM too.
            self.mode = ColorMode::Custom;
        }
    }
    pub(super) fn append(&mut self, color: [u8; 3]) -> anyhow::Result<()> {
        ensure!(
            self.color_list.len() < 16,
            "Controller custom palette is full"
        );
        self.index = self.color_list.len() as u8;
        self.color_list.push(ColorEntry {
            identity: self.next_color_identity,
            index: self.index,
            value: HexColor(color),
        });
        self.next_color_identity = self.next_color_identity.wrapping_add(1);
        Ok(())
    }
    pub(super) fn delete(&mut self, identity: u64) -> anyhow::Result<()> {
        let color = self
            .color_list
            .iter()
            .find(|color| color.identity == identity)
            .ok_or_else(|| anyhow::anyhow!("Controller palette entry no longer exists"))?;
        ensure!(
            color.index >= 8,
            "Controller preset colors cannot be deleted"
        );
        let deleted = color.index;
        self.color_list.retain(|color| color.identity != identity);
        if deleted == self.index {
            self.index = self.color_list.len().saturating_sub(1) as u8;
        } else if deleted < self.index {
            self.index -= 1;
        }
        for (index, color) in self.color_list.iter_mut().enumerate() {
            color.index = index as u8;
        }
        Ok(())
    }
    pub(super) fn observe(&mut self, observed: &ObservedEffect) -> anyhow::Result<()> {
        match observed.effect_id {
            0 => self.effect_id = EffectId::Off,
            3 => {
                self.effect_id = EffectId::Spectrum;
                self.rate = match observed.rate {
                    1 => 3,
                    0 => 2,
                    2 => 1,
                    _ => 0,
                };
            }
            13 => {
                ensure!(
                    observed.flags < 16,
                    "Device returned an invalid controller color index"
                );
                self.effect_id = EffectId::Static;
                self.index = observed.flags;
                self.mode = if self.index > 8 {
                    ColorMode::Custom
                } else {
                    ColorMode::Preset
                };
                if observed.color_count != 0 && !observed.color_bytes.is_empty() {
                    ensure!(
                        observed.color_bytes.len() % 3 == 0 && observed.color_bytes.len() <= 24,
                        "Device returned an invalid controller custom palette"
                    );
                    self.color_list = Self::presets();
                    self.next_color_identity = 8;
                    for color in observed.color_bytes.chunks_exact(3) {
                        let index = self.color_list.len() as u8;
                        self.color_list.push(ColorEntry {
                            identity: self.next_color_identity,
                            index,
                            value: HexColor([color[0], color[1], color[2]]),
                        });
                        self.next_color_identity += 1;
                    }
                }
            }
            _ => anyhow::bail!("Device returned an unsupported controller effect"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorMode, ControllerLightingSettings, EffectId, PRESET_COLORS};
    use razer_device::controller_lighting::{ControllerEffect, ObservedEffect};
    #[test]
    fn controller_palette_edits_preserve_mode_selection_and_wire_order() {
        let mut state = ControllerLightingSettings::default();
        assert_eq!(
            state
                .color_list
                .iter()
                .map(|color| color.value.0)
                .collect::<Vec<_>>(),
            PRESET_COLORS
        );
        state.effect_id = EffectId::Static;
        state.append([1, 2, 3]).unwrap();
        assert_eq!(state.mode, ColorMode::Preset);
        state.append([4, 5, 6]).unwrap();
        let selected = state.color_list[9].identity;
        let first_custom = state.color_list[8].identity;
        state.delete(first_custom).unwrap();
        assert_eq!(state.index, 8);
        assert_eq!(state.color_list[8].identity, selected);
        assert_eq!(
            state.protocol_effect().unwrap(),
            ControllerEffect::Static {
                index: 8,
                custom_colors: vec![[4, 5, 6]]
            }
        );
        state.select(state.color_list[0].identity);
        assert_eq!(state.mode, ColorMode::Custom);
        state.select(selected);
        state.delete(selected).unwrap();
        assert_eq!(state.index, 7);
    }
    #[test]
    fn controller_readback_preserves_the_source_index_eight_mode_boundary() {
        let mut state = ControllerLightingSettings::default();
        state
            .observe(&ObservedEffect {
                profile_slot: 1,
                region_id: 0,
                effect_id: 13,
                flags: 8,
                rate: 0,
                color_count: 1,
                color_bytes: vec![0x12, 0x34, 0x56],
            })
            .unwrap();
        assert_eq!(state.mode, ColorMode::Preset);
        assert_eq!(state.index, 8);
        assert_eq!(state.color_list[8].value.0, [0x12, 0x34, 0x56]);
    }
    #[test]
    fn malformed_saved_palettes_are_rejected_and_plain_source_colors_are_normalized() {
        let mut value = serde_json::to_value(ControllerLightingSettings::default()).unwrap();
        value["colorList"][0] = serde_json::json!("#00ff00");
        let restored: ControllerLightingSettings = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(restored.color_list[0].value.0, [0, 255, 0]);
        value["colorList"][1]["index"] = serde_json::json!(0);
        assert!(serde_json::from_value::<ControllerLightingSettings>(value).is_err());
    }
}

//! Audited per-profile data. Legacy DeviceFeatures remains serialized for migration.
//! These local values are drafts, never evidence of a successful hardware write.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub(super) frequency: u32,
    pub(super) decibel: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EqKind {
    Audio,
    Mic,
}

impl EqKind {
    pub(super) fn presets(self) -> &'static [&'static str] {
        match self {
            Self::Audio => &[
                "default",
                "amplified",
                "vocals",
                "bassboost",
                "enhancedclarity",
                "custom",
            ],
            Self::Mic => &["default", "boost", "broadcast", "conference", "custom"],
        }
    }
    fn values(self, preset: &str) -> [i8; 10] {
        match (self, preset) {
            (Self::Audio, "amplified") => [-2, 2, 4, 5, 5, 5, 5, 5, 3, -2],
            (Self::Audio, "vocals") => [3, 3, 3, 1, -2, -2, -2, 1, 3, 4],
            (Self::Audio, "bassboost") => [5, 5, 5, 3, 0, 0, 0, 2, 2, 0],
            (Self::Audio, "enhancedclarity") => [-5, -4, -2, -1, 0, 0, 0, 2, 2, 2],
            (Self::Mic, "boost") => [0, 0, 2, 4, 5, 5, 5, 5, 2, 1],
            (Self::Mic, "broadcast") => [-2, -1, 1, -1, 0, 0, 0, 2, 1, 1],
            (Self::Mic, "conference") => [-5, -5, -5, -3, 1, 0, 3, 2, 1, 0],
            _ => [0; 10],
        }
    }
    fn bands(self, preset: &str) -> Vec<Band> {
        // 777 module 7816 builds BOTH preset collections from audioBandFrequency.
        // Custom device bands keep their own frequencies. See docs/screens/09-mic.md.
        [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
            .into_iter()
            .zip(self.values(preset))
            .map(|(frequency, decibel)| Band { frequency, decibel })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqSettings {
    pub(super) preset: String,
    pub(super) bands: Vec<Band>,
    pub(super) custom: Vec<Band>,
}
impl Default for EqSettings {
    fn default() -> Self {
        let bands = EqKind::Audio.bands("default");
        Self {
            preset: "default".into(),
            custom: bands.clone(),
            bands,
        }
    }
}
impl EqSettings {
    pub(super) fn select(&mut self, kind: EqKind, preset: &str) {
        if !kind.presets().contains(&preset) {
            return;
        }
        self.preset = preset.into();
        self.bands = if preset == "custom" {
            self.custom.clone()
        } else {
            kind.bands(preset)
        };
    }
    pub(super) fn edit(&mut self, frequency: u32, value: i8) {
        if let Some(band) = self.bands.iter_mut().find(|b| b.frequency == frequency) {
            band.decibel = value.clamp(-5, 5);
            self.preset = "custom".into();
            self.custom = self.bands.clone();
        }
    }
    pub(super) fn reset(&mut self, kind: EqKind) {
        self.preset = "custom".into();
        self.bands = kind.bands("default");
        self.custom = self.bands.clone();
    }
    fn normalize(&mut self, kind: EqKind) {
        if self.bands.is_empty() {
            self.bands = kind.bands("default");
        }
        let mut seen = std::collections::BTreeSet::new();
        self.bands
            .retain(|b| b.frequency > 0 && seen.insert(b.frequency));
        seen.clear();
        self.custom
            .retain(|b| b.frequency > 0 && seen.insert(b.frequency));
        if self.bands.is_empty() {
            self.bands = kind.bands("default");
        }
        for b in self.bands.iter_mut().chain(self.custom.iter_mut()) {
            b.decibel = b.decibel.clamp(-5, 5);
        }
        if self.custom.is_empty() {
            self.custom = kind.bands("default");
        }
        if !kind.presets().contains(&self.preset.as_str()) {
            self.preset = "custom".into();
            self.custom = self.bands.clone();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SmartTracking {
    pub(super) asymmetric: bool,
    pub(super) tracking: u8,
    pub(super) lift: u8,
    pub(super) landing: u8,
}
impl Default for SmartTracking {
    fn default() -> Self {
        Self {
            asymmetric: false,
            tracking: 1,
            lift: 2,
            landing: 1,
        }
    }
}
impl SmartTracking {
    pub(super) fn set_lift(&mut self, value: u8) {
        self.lift = value.clamp(2, 26);
        self.landing = self.landing.clamp(1, self.lift - 1);
    }
    pub(super) fn set_landing(&mut self, value: u8) {
        self.landing = value.clamp(1, 25);
        self.lift = self.lift.clamp(self.landing + 1, 26);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sensitivity {
    pub(super) stages: Vec<[u32; 2]>,
    pub(super) active: usize,
    pub(super) visible: bool,
    pub(super) independent: bool,
    // Stable slot identities travel with their values when reordered. Empty in
    // older local files; normalize migrates their global XY setting once.
    #[serde(default)]
    pub(super) slots: Vec<SensitivitySlot>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct SensitivitySlot {
    pub(super) id: u8,
    pub(super) enabled: bool,
    pub(super) independent: bool,
}
impl Default for Sensitivity {
    fn default() -> Self {
        Self {
            stages: vec![[400; 2], [800; 2], [1600; 2], [3200; 2], [6400; 2]],
            active: 2,
            visible: true,
            independent: false,
            slots: (1..=5)
                .map(|id| SensitivitySlot {
                    id,
                    enabled: true,
                    independent: false,
                })
                .collect(),
        }
    }
}
impl Sensitivity {
    pub(super) fn normalize(&mut self) {
        if self.stages.is_empty() {
            *self = Self::default();
        }
        self.stages.truncate(5);
        let old_count = self.stages.len();
        for index in old_count..5 {
            self.stages.push(Self::default().stages[index]);
        }
        let old_slots = std::mem::take(&mut self.slots);
        let mut used = std::collections::BTreeSet::new();
        self.slots = (0..5)
            .map(|index| {
                let old = old_slots.get(index);
                let id = old
                    .map(|s| s.id)
                    .filter(|id| (1..=5).contains(id) && !used.contains(id))
                    .unwrap_or_else(|| (1..=5).find(|id| !used.contains(id)).unwrap());
                used.insert(id);
                SensitivitySlot {
                    id,
                    enabled: old.map_or(index < old_count, |s| s.enabled),
                    independent: old
                        .map_or(index < old_count && self.independent, |s| s.independent),
                }
            })
            .collect();
        // The source UI always keeps at least two slots enabled. Stage mode
        // off only hides the other rows; it never discards their values.
        for index in 0..5 {
            if self.enabled_count() >= 2 {
                break;
            }
            self.slots[index].enabled = true;
        }
        self.active = self.active.min(4);
        if !self.slots[self.active].enabled {
            self.active = (self.active + 1..5)
                .chain(0..self.active)
                .find(|index| self.slots[*index].enabled)
                .unwrap();
        }
        for (values, slot) in self.stages.iter_mut().zip(&self.slots) {
            for value in values.iter_mut() {
                *value = (*value).clamp(100, 30000).div_ceil(50) * 50;
            }
            if !slot.independent {
                values[1] = values[0];
            }
        }
        self.independent = self.slots[self.active].independent;
    }
    pub(super) fn enabled_count(&self) -> usize {
        self.slots.iter().filter(|s| s.enabled).count()
    }
    pub(super) fn editable_slot(&self, id: u8) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.id == id)
            .filter(|index| self.slots[*index].enabled && (self.visible || *index == self.active))
    }
    pub(super) fn select_stage(&mut self, index: usize) {
        if self.slots.is_empty() {
            self.normalize();
        }
        if (self.visible || index == self.active)
            && self.slots.get(index).is_some_and(|s| s.enabled)
        {
            self.active = index;
            self.independent = self.slots[index].independent;
        }
    }
    pub(super) fn set_enabled(&mut self, id: u8, enabled: bool) {
        let Some(index) = self.slots.iter().position(|s| s.id == id) else {
            return;
        };
        if !self.visible || (!enabled && self.slots[index].enabled && self.enabled_count() <= 2) {
            return;
        }
        self.slots[index].enabled = enabled;
        if !enabled && self.active == index {
            let next = (index + 1..5)
                .chain(0..index)
                .find(|i| self.slots[*i].enabled)
                .unwrap();
            self.select_stage(next);
        }
    }
    pub(super) fn set_slot_axis(&mut self, id: u8, axis: usize, value: u32) {
        let Some(index) = self.editable_slot(id) else {
            return;
        };
        if axis > 1 || (axis == 1 && !self.slots[index].independent) {
            return;
        }
        // 182 stepper module 4230 parseInput rounds upward to stepValue.
        let value = value.clamp(100, 30000).div_ceil(50) * 50;
        self.stages[index][axis] = value;
        if !self.slots[index].independent {
            self.stages[index][1] = value;
        }
        self.select_stage(index);
    }
    pub(super) fn link_slot(&mut self, id: u8, independent: bool) {
        let Some(index) = self.editable_slot(id) else {
            return;
        };
        self.slots[index].independent = independent;
        if !independent {
            self.stages[index][1] = self.stages[index][0];
        }
        self.independent = self.slots[self.active].independent;
    }
    pub(super) fn move_slot(&mut self, id: u8, target: u8) {
        if !self.visible || id == target {
            return;
        }
        let Some(from) = self.slots.iter().position(|s| s.id == id) else {
            return;
        };
        let Some(to) = self.slots.iter().position(|s| s.id == target) else {
            return;
        };
        let active_id = self.slots[self.active].id;
        let slot = self.slots.remove(from);
        let values = self.stages.remove(from);
        self.slots.insert(to, slot);
        self.stages.insert(to, values);
        self.active = self.slots.iter().position(|s| s.id == active_id).unwrap();
        self.independent = self.slots[self.active].independent;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum Effect {
    Ambient,
    AudioMeter,
    Breathing,
    Fire,
    Reactive,
    Ripple,
    Spectrum,
    Starlight,
    Static,
    Tidal,
    Wave,
    Wheel,
}
impl From<Effect> for u8 {
    fn from(effect: Effect) -> u8 {
        match effect {
            Effect::Static => 1,
            Effect::Breathing => 2,
            Effect::Spectrum => 3,
            Effect::Wave => 4,
            Effect::Reactive => 5,
            Effect::Ripple => 6,
            Effect::Starlight => 7,
            Effect::Fire => 8,
            Effect::Ambient => 11,
            Effect::AudioMeter => 12,
            Effect::Wheel => 13,
            Effect::Tidal => 19,
        }
    }
}
impl TryFrom<u8> for Effect {
    type Error = String;
    fn try_from(id: u8) -> Result<Self, Self::Error> {
        Ok(match id {
            1 => Self::Static,
            2 => Self::Breathing,
            3 => Self::Spectrum,
            4 => Self::Wave,
            5 => Self::Reactive,
            6 => Self::Ripple,
            7 => Self::Starlight,
            8 => Self::Fire,
            11 => Self::Ambient,
            12 => Self::AudioMeter,
            13 => Self::Wheel,
            19 => Self::Tidal,
            _ => return Err(format!("Unknown product effect ID {id}")),
        })
    }
}
impl Effect {
    pub(super) fn list(pid: u32, is_ble: bool, use_hardware_effect: bool) -> &'static [Self] {
        use Effect::*;
        match (pid, is_ble && use_hardware_effect) {
            (653, true) => &[
                Breathing, Reactive, Spectrum, Starlight, Static, Tidal, Wave,
            ],
            (653, false) => &[
                Ambient, AudioMeter, Breathing, Fire, Reactive, Ripple, Spectrum, Starlight,
                Static, Tidal, Wave, Wheel,
            ],
            (777, true) => &[
                AudioMeter, Breathing, Reactive, Ripple, Spectrum, Starlight, Static, Wave,
            ],
            (777, false) => &[AudioMeter, Breathing, Spectrum, Static],
            (3073 | 3074 | 3078, _) => &[AudioMeter, Breathing, Reactive, Spectrum, Static],
            (3072 | 3076 | 3077 | 3080, _) => &[
                AudioMeter, Breathing, Reactive, Spectrum, Static, Tidal, Wave,
            ],
            _ => &[],
        }
    }
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Ambient => "ambient",
            Self::AudioMeter => "audio_meter",
            Self::Breathing => "breathing",
            Self::Fire => "fire",
            Self::Reactive => "reactive",
            Self::Ripple => "ripple",
            Self::Spectrum => "spectrum",
            Self::Starlight => "starlight",
            Self::Static => "static",
            Self::Tidal => "tidal",
            Self::Wave => "wave",
            Self::Wheel => "wheel",
        }
    }
    pub(super) fn label(self) -> String {
        crate::i18n::t_or(
            &self.id().to_uppercase(),
            match self {
                Self::Ambient => "环境感知",
                Self::AudioMeter => "音频计",
                Self::Breathing => "呼吸",
                Self::Fire => "火焰",
                Self::Reactive => "响应",
                Self::Ripple => "涟漪",
                Self::Spectrum => "光谱循环",
                Self::Starlight => "星光",
                Self::Static => "静态",
                Self::Tidal => "潮汐",
                Self::Wave => "波浪",
                Self::Wheel => "转轮",
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lighting {
    pub(super) enabled: bool,
    pub(super) brightness: u8,
    pub(super) effect: Effect,
    pub(super) display_off: bool,
    pub(super) idle_enabled: bool,
    pub(super) idle_minutes: u8,
    pub(super) advanced: bool,
    // A cache per effect prevents losing an effect's parameters when previewing another.
    pub(super) colors: BTreeMap<String, [u8; 3]>,
    pub(super) parameters: BTreeMap<u8, EffectParameters>,
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            enabled: true,
            brightness: 100,
            effect: Effect::Spectrum,
            display_off: false,
            idle_enabled: false,
            idle_minutes: 5,
            advanced: false,
            colors: BTreeMap::new(),
            parameters: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct EffectParameters {
    pub(super) color1: Option<[u8; 3]>,
    pub(super) color2: Option<[u8; 3]>,
    pub(super) random: bool,
    pub(super) direction: u8,
    pub(super) duration: u8,
    pub(super) color_boost: f32,
    pub(super) screen: String,
}
/// 653 DU / Stepper 44230 rounds upward to a quarter after clamping.
pub(super) fn normalize_color_boost(value: f32) -> f32 {
    if value.is_finite() {
        (value.clamp(0.25, 4.) * 4.).ceil() / 4.
    } else {
        0.25
    }
}

impl EffectParameters {
    fn for_effect(effect: Effect) -> Self {
        Self {
            color1: Some([0, 255, 0]),
            color2: if effect == Effect::Tidal {
                Some([0, 0, 255])
            } else {
                None
            },
            random: false,
            direction: if effect == Effect::Wave { 2 } else { 1 },
            duration: 2,
            color_boost: 1.,
            screen: "full".into(),
        }
    }
}
impl Lighting {
    pub(super) fn params(&self) -> EffectParameters {
        self.parameters
            .get(&u8::from(self.effect))
            .cloned()
            .unwrap_or_else(|| {
                let mut value = EffectParameters::for_effect(self.effect);
                if let Some(color) = self.colors.get(self.effect.id()) {
                    value.color1 = Some(*color);
                }
                value
            })
    }
    pub(super) fn params_mut(&mut self) -> &mut EffectParameters {
        let value = self.params();
        self.parameters.entry(self.effect.into()).or_insert(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct DialMode {
    pub(super) uid: String,
    pub(super) enabled: bool,
    #[serde(default)]
    pub(super) name: String,
    #[serde(default)]
    pub(super) is_custom: bool,
    #[serde(default = "default_dial_color")]
    pub(super) color: [u8; 3],
    #[serde(default)]
    pub(super) mappings: BTreeMap<String, String>,
}

fn default_dial_color() -> [u8; 3] {
    [254, 237, 3]
}

impl DialMode {
    pub(super) fn is_switch_applications(&self) -> bool {
        // Em's y() compares the displayed source name, without excluding
        // custom modes. A custom mode named exactly "Switch Applications"
        // therefore follows the same Alt+Tab restriction. The canonical id
        // branch also recognizes presets in older local profiles.
        self.name == "Switch Applications"
            || (!self.is_custom
                && (self.uid == "SWITCH_APPLICATIONS" || self.name == "SWITCH_APPLICATIONS"))
    }

    pub(super) fn label(&self) -> String {
        let name = if self.name.is_empty() {
            &self.uid
        } else {
            &self.name
        };
        if self.is_custom {
            name.to_owned()
        } else {
            crate::i18n::t_or(name, name)
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keyboard {
    pub(super) gaming: bool,
    pub(super) in_game: bool,
    pub(super) disable_alt_tab: bool,
    pub(super) disable_alt_f4: bool,
    pub(super) snap_tap: bool,
    pub(super) snap_keys: [String; 2],
    /// Empty in legacy profiles; `snap_keys` remains the compatible first pair.
    pub(super) snap_pairs: Vec<[String; 2]>,
    pub(super) dial_modes: Vec<DialMode>,
    pub(super) dial_active: String,
}
impl Default for Keyboard {
    fn default() -> Self {
        Self {
            gaming: false,
            in_game: false,
            disable_alt_tab: false,
            disable_alt_f4: false,
            snap_tap: false,
            snap_keys: ["KEY_A".into(), "KEY_D".into()],
            snap_pairs: Vec::new(),
            dial_modes: [
                "KEYBOARD_BRIGHTNESS",
                "WINDOWS_ZOOM",
                "SWITCH_APPLICATIONS",
                "TRACK_JOGGING",
                "TRACK_SELECTOR",
                "VERTICAL_SCROLLING",
                "HORIZONTAL_SCROLLING",
                "SWITCH_BROWSER_TABS",
            ]
            .into_iter()
            .enumerate()
            .map(|(index, uid)| DialMode {
                uid: uid.into(),
                enabled: index < 4,
                name: uid.into(),
                is_custom: false,
                color: [
                    [0, 255, 0],
                    [255, 0, 0],
                    [0, 0, 255],
                    [254, 237, 3],
                    [0, 255, 255],
                    [255, 0, 255],
                    [255, 255, 255],
                    [253, 134, 17],
                ][index],
                mappings: BTreeMap::new(),
            })
            .collect(),
            dial_active: "KEYBOARD_BRIGHTNESS".into(),
        }
    }
}
impl Keyboard {
    pub(super) fn normalize(&mut self) {
        let defaults = Self::default();
        for mode in &mut self.dial_modes {
            // Legacy local profiles stored only uid/enabled for the presets.
            if !mode.is_custom && mode.name.is_empty() {
                if let Some(default) = defaults
                    .dial_modes
                    .iter()
                    .find(|default| default.uid == mode.uid)
                {
                    mode.name = default.name.clone();
                    mode.color = default.color;
                }
            }
        }
        if !self.dial_modes.iter().any(|mode| !mode.is_custom) {
            self.dial_modes.extend(defaults.dial_modes.clone());
        }
        self.apply_dial_game_mode();
        // The original game-mode effect can disable the only enabled preset.
        // Preserve complete source-mode sets in that state; only repair old
        // incomplete local mode lists here.
        let complete_presets = defaults.dial_modes.iter().all(|default| {
            self.dial_modes.iter().any(|mode| {
                !mode.is_custom && (mode.uid == default.uid || mode.name == default.name)
            })
        });
        if !self
            .dial_modes
            .iter()
            .any(|mode| !mode.is_custom && mode.enabled)
            && !complete_presets
        {
            let alt_tab_disabled = self.disable_alt_tab;
            if let Some(mode) = self.dial_modes.iter_mut().find(|mode| {
                !mode.is_custom && !(alt_tab_disabled && mode.is_switch_applications())
            }) {
                mode.enabled = true;
            } else {
                self.dial_modes.push(defaults.dial_modes[0].clone());
            }
        }
        if !self.dial_selection_valid() {
            self.dial_active = self
                .dial_modes
                .iter()
                .find(|mode| mode.enabled)
                .or_else(|| {
                    self.dial_modes
                        .iter()
                        .find(|mode| mode.is_switch_applications())
                })
                .or_else(|| self.dial_modes.first())
                .map(|mode| mode.uid.clone())
                .unwrap_or_default();
        }
        let pairs = self.snap_key_pairs();
        let mut seen = std::collections::BTreeSet::new();
        let pairs = pairs
            .into_iter()
            .filter(|pair| {
                pair[0] != pair[1]
                    && pair
                        .iter()
                        .all(|key| canonical_snap_key(key).is_some() && !seen.contains(key))
                    && {
                        seen.extend(pair.iter().cloned());
                        true
                    }
            })
            .take(4)
            .collect::<Vec<_>>();
        self.snap_pairs = if pairs.is_empty() {
            vec![defaults.snap_keys]
        } else {
            pairs
        };
        self.snap_keys = self.snap_pairs[0].clone();
    }

    pub(super) fn snap_key_pairs(&self) -> Vec<[String; 2]> {
        let pairs = if self.snap_pairs.is_empty() {
            vec![self.snap_keys.clone()]
        } else {
            self.snap_pairs.clone()
        };
        pairs
            .into_iter()
            .map(|pair| pair.map(|key| canonical_snap_key(&key).unwrap_or(key)))
            .collect()
    }

    pub(super) fn set_snap_key(&mut self, index: usize, value: &str) {
        let Some(mut pair) = self.snap_key_pairs().first().cloned() else {
            return;
        };
        if index < 2 {
            pair[index] = value.into();
            self.set_snap_pair(0, pair);
        }
    }

    pub(super) fn set_snap_pair(&mut self, index: usize, pair: [String; 2]) -> bool {
        let mut pairs = self.snap_key_pairs();
        if index > pairs.len() || index >= 4 {
            return false;
        }
        let [Some(left), Some(right)] = pair.map(|key| canonical_snap_key(&key)) else {
            return false;
        };
        if left == right
            || pairs.iter().enumerate().any(|(ix, pair)| {
                ix != index && pair.iter().any(|key| *key == left || *key == right)
            })
        {
            return false;
        }
        if index == pairs.len() {
            pairs.push([left, right]);
        } else {
            pairs[index] = [left, right];
        }
        self.snap_keys = pairs[0].clone();
        self.snap_pairs = pairs;
        true
    }

    pub(super) fn remove_snap_pair(&mut self, index: usize) {
        let mut pairs = self.snap_key_pairs();
        if index > 0 && index < pairs.len() {
            pairs.remove(index);
            self.snap_pairs = pairs;
        }
    }

    pub(super) fn dial_enabled_change_allowed(&self, uid: &str, enabled: bool) -> bool {
        self.dial_modes
            .iter()
            .find(|mode| mode.uid == uid)
            .is_some_and(|mode| {
                !self.dial_blocked_by_game_mode(mode)
                    && (enabled
                        || !mode.enabled
                        // Em applies f() && isEnabled to every row. f() is
                        // true when exactly one default mode remains enabled,
                        // so enabled custom switches are locked in that case
                        // too. Zero defaults is possible after the game-mode
                        // effect and does not impose this UI restriction.
                        || self
                            .dial_modes
                            .iter()
                            .filter(|mode| mode.enabled && !mode.is_custom)
                            .count()
                            != 1)
            })
    }

    pub(super) fn dial_blocked_by_game_mode(&self, mode: &DialMode) -> bool {
        self.disable_alt_tab && mode.is_switch_applications()
    }

    pub(super) fn set_disable_alt_tab(&mut self, disabled: bool) {
        self.disable_alt_tab = disabled;
        self.apply_dial_game_mode();
    }

    fn apply_dial_game_mode(&mut self) {
        if self.disable_alt_tab {
            for mode in &mut self.dial_modes {
                if mode.is_switch_applications() {
                    // Em calls updateModeProperty with notJumpNext: true.
                    // Clearing the Alt+Tab restriction does not re-enable it.
                    mode.enabled = false;
                }
            }
        }
    }

    pub(super) fn dial_selection_valid(&self) -> bool {
        // notJumpNext retains a disabled current mode. An empty cycle can
        // retain a custom mode too; subsequently enabling another mode does
        // not select it. Valid persisted identity therefore means existence,
        // while select_dial separately enforces whether a new choice is usable.
        self.dial_modes
            .iter()
            .any(|mode| mode.uid == self.dial_active)
    }
    pub(super) fn enable_dial(&mut self, uid: &str, enabled: bool) {
        if !self.dial_enabled_change_allowed(uid, enabled) {
            return;
        }
        if let Some(mode) = self.dial_modes.iter_mut().find(|m| m.uid == uid) {
            mode.enabled = enabled;
        }
        if self.dial_active == uid && !enabled {
            self.dial_active = self.next_enabled_dial(uid).unwrap_or_else(|| uid.into());
        }
    }

    fn next_enabled_dial(&self, uid: &str) -> Option<String> {
        let start = self.dial_modes.iter().position(|mode| mode.uid == uid)?;
        (1..=self.dial_modes.len())
            .map(|offset| &self.dial_modes[(start + offset) % self.dial_modes.len()])
            .find(|mode| mode.enabled && !self.dial_blocked_by_game_mode(mode))
            .map(|mode| mode.uid.clone())
    }

    pub(super) fn select_dial(&mut self, uid: &str) {
        if self
            .dial_modes
            .iter()
            .any(|mode| mode.uid == uid && mode.enabled && !self.dial_blocked_by_game_mode(mode))
        {
            self.dial_active = uid.into();
        }
    }

    pub(super) fn add_dial(&mut self) -> Option<String> {
        if self.dial_modes.iter().filter(|mode| mode.is_custom).count() >= 100 {
            return None;
        }
        static NEXT_DIAL_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let serial = NEXT_DIAL_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let uid = format!("local-dial-{epoch}-{serial}");
        let base = crate::i18n::t_or("CUSTOM_MODE", "Custom mode");
        let name = std::iter::once(base.clone())
            .chain((1..).map(|ix| format!("{base} {ix}")))
            .find(|name| !self.dial_modes.iter().any(|mode| mode.label() == *name))?;
        self.dial_modes.insert(
            0,
            DialMode {
                uid: uid.clone(),
                enabled: true,
                name,
                is_custom: true,
                color: default_dial_color(),
                mappings: ["ScrollRight", "ScrollLeft"]
                    .into_iter()
                    .map(|input| (input.into(), "disable".into()))
                    .collect(),
            },
        );
        Some(uid)
    }

    pub(super) fn rename_dial(&mut self, uid: &str, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || name.encode_utf16().count() > 40 {
            return false;
        }
        let Some(mode) = self
            .dial_modes
            .iter_mut()
            .find(|mode| mode.uid == uid && mode.is_custom)
        else {
            return false;
        };
        mode.name = name.into();
        true
    }

    pub(super) fn delete_dial(&mut self, uid: &str) {
        if !self
            .dial_modes
            .iter()
            .any(|mode| mode.uid == uid && mode.is_custom)
        {
            return;
        }
        if self.dial_active == uid {
            self.dial_active = self.next_enabled_dial(uid).unwrap_or_else(|| {
                // Game mode can leave no enabled preset. Deleting a custom
                // mode must still keep a surviving identity in the document.
                self.dial_modes
                    .iter()
                    .find(|mode| mode.uid != uid)
                    .map(|mode| mode.uid.clone())
                    .unwrap_or_default()
            });
        }
        self.dial_modes.retain(|mode| mode.uid != uid);
    }

    pub(super) fn reset_dial(&mut self) {
        let mut defaults = Self::default();
        for mode in &mut defaults.dial_modes {
            if let Some(previous) = self.dial_modes.iter().find(|previous| {
                !previous.is_custom && (previous.name == mode.name || previous.uid == mode.uid)
            }) {
                mode.uid = previous.uid.clone();
            }
        }
        defaults.dial_active = defaults.dial_modes[0].uid.clone();
        self.dial_modes = defaults.dial_modes;
        self.dial_active = defaults.dial_active;
        self.apply_dial_game_mode();
    }
    pub(super) fn move_dial(&mut self, uid: &str, delta: isize) {
        if let Some(index) = self.dial_modes.iter().position(|m| m.uid == uid) {
            let target = index as isize + delta;
            if target >= 0 && target < (self.dial_modes.len() as isize) {
                self.dial_modes.swap(index, target as usize);
            }
        }
    }
}

pub(super) fn canonical_snap_key(key: &str) -> Option<String> {
    const KEYS: &[(&str, &str, &str)] = include!("mapping_keys.rs");
    let key = key.trim().to_ascii_uppercase();
    let id = if key.starts_with("KEY_") {
        key
    } else {
        format!("KEY_{key}")
    };
    if [
        "KEY_APPLICATION",
        "KEY_LEFT_GUI",
        "KEY_WINDOWS",
        "KEY_FN",
        "DKM_F6",
        "DKM_D2",
    ]
    .contains(&id.as_str())
    {
        return None;
    }
    KEYS.iter()
        .any(|(_, candidate, _)| *candidate == id)
        .then_some(id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkedGame {
    pub(super) name: String,
    pub(super) executable: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileSettings {
    pub(super) audio: EqSettings,
    pub(super) mic: EqSettings,
    pub(super) playback_enabled: bool,
    pub(super) volume: u8,
    pub(super) tracking: SmartTracking,
    pub(super) sensitivity: Sensitivity,
    pub(super) polling: u32,
    pub(super) idle_minutes: u8,
    pub(super) power_enabled: bool,
    pub(super) low_power: u8,
    pub(super) lighting: Lighting,
    pub(super) keyboard: Keyboard,
    pub(super) bindings: BTreeMap<String, String>,
    pub(super) hypershift_bindings: BTreeMap<String, String>,
    pub(super) linked_games: Vec<LinkedGame>,
}
impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            audio: EqSettings::default(),
            mic: EqSettings::default(),
            playback_enabled: true,
            volume: 70,
            tracking: SmartTracking::default(),
            sensitivity: Sensitivity::default(),
            polling: 1000,
            idle_minutes: 5,
            power_enabled: true,
            low_power: 5,
            lighting: Lighting::default(),
            keyboard: Keyboard::default(),
            bindings: BTreeMap::new(),
            hypershift_bindings: BTreeMap::new(),
            linked_games: vec![],
        }
    }
}
impl ProfileSettings {
    pub(crate) fn for_product(pid: u32) -> Self {
        let mut settings = Self::default();
        if let Some(product) = crate::product::audited_mouse_mat(pid) {
            settings.lighting.brightness = product.brightness();
            // Mouse mat modules retain idle data, but only render display-off.
            settings.lighting.idle_minutes = 1;
        }
        settings.normalize(pid);
        settings
    }

    pub(crate) fn from_legacy(
        device: &crate::model::Device,
        profile: &crate::model::Profile,
    ) -> Self {
        let mut settings = Self::for_product(device.product_id);
        if let Some(stages) = &profile.dpi_stages {
            settings.sensitivity.stages = stages.stages.iter().map(|s| [s.x, s.y]).collect();
            settings.sensitivity.visible = stages.enable;
            settings.sensitivity.slots = stages
                .stages
                .iter()
                .take(5)
                .enumerate()
                .map(|(index, stage)| SensitivitySlot {
                    id: index as u8 + 1,
                    enabled: true,
                    independent: stage.x != stage.y,
                })
                .collect();
        }
        // Device-wide old values can only be attributed to the active profile.
        if profile.id == device.active_profile {
            settings.polling = device.features.performance.polling_rate.hz();
            if let Some(power) = &device.features.power {
                settings.idle_minutes = power.sleep_after_min.min(255) as u8;
                settings.power_enabled = power.sleep_after_min != 0;
            }
            if let Some(zone) = device.features.lighting.first() {
                use crate::domain::LightingEffect as Old;
                let effect = match zone.effect {
                    Old::Static => Some(Effect::Static),
                    Old::SpectrumCycling => Some(Effect::Spectrum),
                    Old::AudioMeter => Some(Effect::AudioMeter),
                    Old::Breathing => Some(Effect::Breathing),
                    Old::Reactive => Some(Effect::Reactive),
                    Old::Ripple => Some(Effect::Ripple),
                    Old::Starlight => Some(Effect::Starlight),
                    Old::Wave => Some(Effect::Wave),
                    _ => None,
                };
                settings.lighting.brightness = zone.brightness;
                settings.lighting.enabled = zone.effect != Old::Off;
                if let Some(effect) = effect {
                    settings.lighting.effect = effect;
                    settings
                        .lighting
                        .colors
                        .insert(effect.id().into(), zone.color);
                }
            }
            if let Some(sound) = &device.features.sound {
                settings.volume = sound.volume;
                if sound.equalizer.bands.len() == 10 {
                    for (band, value) in settings.audio.bands.iter_mut().zip(&sound.equalizer.bands)
                    {
                        band.decibel = (*value).clamp(-5, 5);
                    }
                    settings.audio.custom = settings.audio.bands.clone();
                    settings.audio.preset = "custom".into();
                }
            }
            settings.bindings = device
                .dkm_keys
                .iter()
                .map(|key| (key.input_id.clone(), key.button_key.clone()))
                .collect();
        }
        settings.normalize(device.product_id);
        settings
    }
    pub(crate) fn normalize(&mut self, pid: u32) {
        self.audio.normalize(EqKind::Audio);
        self.mic.normalize(EqKind::Mic);
        self.volume = self.volume.min(100);
        self.tracking.tracking = self.tracking.tracking.clamp(1, 3);
        self.tracking.set_lift(self.tracking.lift);
        self.sensitivity.normalize();
        self.keyboard.normalize();
        self.idle_minutes = self.idle_minutes.clamp(
            if pid == 777 { 5 } else { 1 },
            if pid == 777 { 60 } else { 15 },
        );
        self.low_power = (self.low_power.clamp(5, 100) / 5) * 5;
        self.lighting.brightness = self.lighting.brightness.min(100);
        let effects = Effect::list(if pid == 9001 { 653 } else { pid }, false, false);
        if !effects.is_empty() && !effects.contains(&self.lighting.effect) {
            self.lighting.effect = Effect::Spectrum;
        }
        self.lighting.idle_minutes = self.lighting.idle_minutes.clamp(1, 15);
        let wave_direction =
            crate::product::audited_mouse_mat(pid).and_then(|product| product.wave_direction());
        if let Some(direction) = wave_direction {
            // Quick-effect selection applies the product's direction default
            // only when that effect has no cached setting yet.
            self.lighting
                .parameters
                .entry(Effect::Wave.into())
                .or_insert_with(|| {
                    let mut params = EffectParameters::for_effect(Effect::Wave);
                    params.direction = direction.default_value();
                    params
                });
        }
        for (id, params) in &mut self.lighting.parameters {
            params.duration = params.duration.clamp(1, 3);
            params.color_boost = normalize_color_boost(params.color_boost);
            params.direction = if *id == 19 {
                params.direction.min(1)
            } else if *id == u8::from(Effect::Wave)
                && wave_direction
                    == Some(crate::product::MouseMatWaveDirection::ClockwiseCounterclockwise)
            {
                if matches!(params.direction, 11 | 12) {
                    params.direction
                } else {
                    12
                }
            } else {
                params.direction.clamp(1, 2)
            };
            if !["full", "left", "top", "right", "bottom"].contains(&params.screen.as_str()) {
                params.screen = "full".into();
            }
        }
    }
    pub(super) fn eq(&self, kind: EqKind) -> &EqSettings {
        match kind {
            EqKind::Audio => &self.audio,
            EqKind::Mic => &self.mic,
        }
    }
    pub(super) fn eq_mut(&mut self, kind: EqKind) -> &mut EqSettings {
        match kind {
            EqKind::Audio => &mut self.audio,
            EqKind::Mic => &mut self.mic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mic_edits_and_reset_do_not_modify_audio_or_lose_custom_frequencies() {
        let mut state = ProfileSettings::default();
        state.audio.select(EqKind::Audio, "bassboost");
        let audio = state.audio.clone();
        state.mic.bands = vec![Band {
            frequency: 750,
            decibel: 0,
        }];
        state.mic.edit(750, 12);
        state.mic.select(EqKind::Mic, "boost");
        state.mic.select(EqKind::Mic, "custom");
        assert_eq!(
            state.mic.bands,
            vec![Band {
                frequency: 750,
                decibel: 5
            }]
        );
        state.mic.reset(EqKind::Mic);
        assert_eq!(state.mic.preset, "custom");
        assert!(state.mic.bands.iter().all(|b| b.decibel == 0));
        assert_eq!(audio, state.audio);
    }
    #[test]
    fn asymmetric_boundaries_remain_strict_for_every_input() {
        for lift in 0..=30 {
            for landing in 0..=30 {
                let mut state = SmartTracking::default();
                state.set_landing(landing);
                state.set_lift(lift);
                assert!((1..=25).contains(&state.landing));
                assert!((2..=26).contains(&state.lift));
                assert!(state.landing < state.lift);
                state.set_landing(landing);
                assert!(state.landing < state.lift);
            }
        }
    }
    #[test]
    fn dpi_edits_target_active_stage_and_link_both_axes() {
        let mut state = Sensitivity::default();
        state.set_slot_axis(3, 0, 101);
        assert_eq!(state.stages[2], [150, 150]);
        state.set_slot_axis(3, 0, 1628);
        assert_eq!(state.stages[2], [1650, 1650]);
        assert_eq!(state.stages[0], [400, 400]);
        state.link_slot(3, true);
        state.set_slot_axis(3, 1, 100000);
        assert_eq!(state.stages[2], [1650, 30000]);
        state.link_slot(3, false);
        assert_eq!(state.stages[2], [1650, 1650]);
    }
    #[test]
    fn dpi_slots_keep_independent_axes_and_values_when_disabled_and_reordered() {
        let mut state = Sensitivity::default();
        state.normalize();
        state.link_slot(1, true);
        state.set_slot_axis(1, 1, 1200);
        state.link_slot(3, true);
        state.set_slot_axis(3, 1, 3000);
        state.link_slot(3, false);
        assert_eq!(state.stages[0], [400, 1200]);
        assert_eq!(state.stages[2], [1600, 1600]);
        state.set_enabled(3, false);
        assert_eq!(state.slots[state.active].id, 4);
        state.move_slot(1, 5);
        assert_eq!(state.slots[state.active].id, 4);
        let moved = state.slots.iter().position(|s| s.id == 1).unwrap();
        assert_eq!(state.stages[moved], [400, 1200]);
        assert!(state.slots[moved].independent);
        let hidden = state.slots.iter().position(|s| s.id == 3).unwrap();
        assert_eq!(state.stages[hidden], [1600, 1600]);
        state.set_enabled(3, true);
        assert_eq!(state.stages[hidden], [1600, 1600]);
        state.normalize();
        let encoded = serde_json::to_string(&state).unwrap();
        let mut restored: Sensitivity = serde_json::from_str(&encoded).unwrap();
        restored.normalize();
        assert_eq!(restored, state);
    }
    #[test]
    fn dpi_slots_migrate_old_files_and_keep_two_enabled_slots() {
        let mut state: Sensitivity = serde_json::from_str(
            r#"{"stages":[[500,900],[1700,1800]],"active":1,"visible":true,"independent":true}"#,
        )
        .unwrap();
        state.normalize();
        assert_eq!(state.stages.len(), 5);
        assert_eq!(state.enabled_count(), 2);
        state.set_enabled(2, false);
        assert!(state.slots[1].enabled);
        assert_eq!(state.stages[1], [1700, 1800]);
        state.set_enabled(5, true);
        state.set_enabled(2, false);
        assert_eq!(state.slots[state.active].id, 5);
        state.visible = false;
        let previous = state.clone();
        state.move_slot(1, 5);
        state.set_enabled(1, false);
        assert_eq!(previous, state);
    }
    #[test]
    fn dpi_defaults_are_editable_and_legacy_xy_migration_only_runs_once() {
        let mut fresh = Sensitivity::default();
        assert_eq!(fresh.enabled_count(), 5);
        fresh.set_slot_axis(5, 0, 1234);
        assert_eq!(fresh.stages[4], [1250, 1250]);
        assert_eq!(fresh.active, 4);

        let mut legacy: Sensitivity = serde_json::from_str(
            r#"{"stages":[[500,900],[1700,1800]],"active":1,"visible":true,"independent":true}"#,
        )
        .unwrap();
        assert!(legacy.slots.is_empty());
        legacy.normalize();
        assert!(legacy.slots[0].independent);
        assert!(legacy.slots[1].independent);
        assert!(!legacy.slots[2].independent);
        legacy.link_slot(2, false);
        let snapshot = legacy.clone();
        legacy.normalize();
        assert_eq!(legacy, snapshot);
        assert_eq!(legacy.stages[0], [500, 900]);
        assert_eq!(legacy.stages[1], [1700, 1700]);
    }
    #[test]
    fn disabled_or_hidden_dpi_slots_reject_delayed_control_edits() {
        let mut state = Sensitivity::default();
        state.link_slot(1, true);
        state.set_slot_axis(1, 1, 1200);
        state.set_enabled(1, false);
        let disabled = state.clone();
        state.set_slot_axis(1, 0, 30000);
        state.set_slot_axis(1, 1, 30000);
        state.link_slot(1, false);
        state.select_stage(0);
        assert_eq!(state, disabled);

        state.visible = false;
        let hidden = state.clone();
        state.set_slot_axis(5, 0, 30000);
        state.link_slot(5, true);
        state.select_stage(4);
        state.set_enabled(5, false);
        state.move_slot(5, 2);
        assert_eq!(state, hidden);
        let active_id = state.slots[state.active].id;
        state.set_slot_axis(active_id, 0, 2777);
        assert_eq!(state.stages[state.active], [2800, 2800]);
    }
    #[test]
    fn disabling_current_dpi_slot_wraps_and_reordering_keeps_its_identity() {
        let mut state = Sensitivity::default();
        state.select_stage(4);
        state.set_enabled(5, false);
        assert_eq!(state.slots[state.active].id, 1);
        state.move_slot(1, 5);
        assert_eq!(state.slots[state.active].id, 1);
        assert_eq!(state.active, 4);
        state.set_enabled(1, false);
        assert_eq!(state.slots[state.active].id, 2);
        state.set_enabled(3, false);
        assert_eq!(state.enabled_count(), 2);
        let at_minimum = state.clone();
        state.set_enabled(2, false);
        assert_eq!(state, at_minimum);
        state.set_slot_axis(2, 2, 999);
        state.set_slot_axis(2, 1, 999);
        state.set_slot_axis(255, 0, 999);
        state.link_slot(255, true);
        state.move_slot(255, 2);
        assert_eq!(state, at_minimum);
    }
    #[test]
    fn legacy_device_dpi_values_migrate_each_slots_xy_and_stage_count() {
        let mut device = crate::model::measured_devices().remove(0);
        let profile = &mut device.profiles[0];
        profile.dpi_stages = Some(crate::model::DpiStages {
            enable: false,
            stages: vec![
                crate::model::DpiStage { x: 500, y: 900 },
                crate::model::DpiStage { x: 1800, y: 1800 },
            ],
        });
        let settings = ProfileSettings::from_legacy(&device, &device.profiles[0]);
        let state = &settings.sensitivity;
        assert!(!state.visible);
        assert_eq!(state.stages.len(), 5);
        assert_eq!(state.enabled_count(), 2);
        assert_eq!(state.stages[0], [500, 900]);
        assert_eq!(state.stages[1], [1800, 1800]);
        assert!(state.slots[0].independent);
        assert!(!state.slots[1].independent);
        assert!(state.slots[state.active].enabled);
        let mut restored: ProfileSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        restored.normalize(device.product_id);
        assert_eq!(restored, settings);
    }
    #[test]
    fn ble_alone_does_not_select_hardware_effects() {
        assert_eq!(
            Effect::list(777, true, false),
            Effect::list(777, false, false)
        );
        assert_eq!(Effect::list(653, true, false).len(), 12);
        assert_eq!(Effect::list(653, true, true).len(), 7);
        let ids: std::collections::BTreeSet<_> = Effect::list(653, false, false)
            .iter()
            .map(|e| e.id())
            .collect();
        assert_eq!(ids.len(), 12);
    }
    #[test]
    fn effect_ids_and_parameters_round_trip_without_cross_effect_aliases() {
        let mut lighting = Lighting::default();
        lighting.effect = Effect::Tidal;
        lighting.params_mut().direction = 0;
        lighting.effect = Effect::Static;
        lighting.params_mut().color1 = Some([12, 34, 56]);
        let encoded = serde_json::to_value(&lighting).unwrap();
        assert_eq!(encoded["effect"], 1);
        let mut restored: Lighting = serde_json::from_value(encoded).unwrap();
        assert_eq!(restored.params().color1, Some([12, 34, 56]));
        restored.effect = Effect::Tidal;
        assert_eq!(restored.params().direction, 0);
        assert_eq!(restored.params().color1, Some([0, 255, 0]));
        assert_eq!(u8::from(Effect::Tidal), 19);
        assert_eq!(u8::from(Effect::Fire), 8);
    }

    #[test]
    fn mouse_mat_wave_directions_survive_profile_round_trip_and_normalization() {
        for (pid, direction) in [(3072, 11), (3076, 12), (3077, 11), (3080, 1)] {
            let mut settings = ProfileSettings::for_product(pid);
            settings.lighting.effect = Effect::Wave;
            settings.lighting.params_mut().direction = direction;
            settings.lighting.brightness = 37;
            let mut restored: ProfileSettings =
                serde_json::from_value(serde_json::to_value(&settings).unwrap()).unwrap();
            restored.normalize(pid);
            assert_eq!(restored, settings);

            // Opening another effect must retain the product-specific Wave value.
            restored.lighting.effect = Effect::Tidal;
            restored.lighting.params_mut().direction = 0;
            restored.normalize(pid);
            restored.lighting.effect = Effect::Wave;
            assert_eq!(restored.lighting.params().direction, direction);
        }
        let mut invalid = ProfileSettings::for_product(3076);
        invalid.lighting.effect = Effect::Wave;
        invalid.lighting.params_mut().direction = 2;
        invalid.normalize(3076);
        assert_eq!(invalid.lighting.params().direction, 12);
    }

    #[test]
    fn mouse_mat_legacy_migration_uses_source_defaults_then_preserves_saved_values() {
        let mut device = crate::model::measured_devices().remove(0);
        device.product_id = 3076;
        device.features.lighting.clear();
        let defaults = ProfileSettings::from_legacy(&device, &device.profiles[0]);
        assert_eq!(defaults.lighting.brightness, 66);
        assert_eq!(defaults.lighting.idle_minutes, 1);
        assert_eq!(defaults.lighting.effect, Effect::Spectrum);
        assert!(defaults.lighting.enabled);
        assert!(!defaults.lighting.display_off);
        assert!(!defaults.lighting.idle_enabled);

        let mut saved = defaults;
        saved.lighting.brightness = 23;
        saved.lighting.display_off = true;
        saved.normalize(device.product_id);
        assert_eq!(saved.lighting.brightness, 23);
        assert!(saved.lighting.display_off);

        saved.lighting.effect = Effect::Wave;
        saved.normalize(3073);
        assert_eq!(saved.lighting.effect, Effect::Spectrum);
        assert_eq!(saved.lighting.brightness, 23);
        assert_eq!(ProfileSettings::for_product(3073).lighting.brightness, 100);
    }
}

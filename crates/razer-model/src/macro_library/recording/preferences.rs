//! Co's editor defaults and G's independently persisted recordProfile fields.
use crate::macro_library::{MacroType, MovementMode};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Seconds(f64);

impl Eq for Seconds {}

impl Seconds {
    pub const ZERO: Self = Self(0.);

    pub fn new(value: f64) -> Result<Self, String> {
        if value.is_finite() && value >= 0. {
            Ok(Self(value))
        } else {
            Err("Macro recording delay must be finite and nonnegative".into())
        }
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Seconds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelayMode {
    #[default]
    Recorded,
    Fixed,
    Randomized,
    None,
}

impl DelayMode {
    pub fn from_setting(setting: u8) -> Result<Self, String> {
        match setting {
            0 => Ok(Self::Recorded),
            1 => Ok(Self::Fixed),
            2 => Ok(Self::Randomized),
            3 => Ok(Self::None),
            _ => Err("Unknown macro recording delay mode".into()),
        }
    }

    pub fn setting(self) -> u8 {
        match self {
            Self::Recorded => 0,
            Self::Fixed => 1,
            Self::Randomized => 2,
            Self::None => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub start_delay: u8,
    pub fixed: Seconds,
    pub random: [Seconds; 2],
    pub movement: MovementMode,
    pub macro_type: MacroType,
    pub delay_mode: DelayMode,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            start_delay: 0,
            fixed: Seconds(0.2),
            random: [Seconds(0.2), Seconds(0.5)],
            movement: MovementMode::None,
            macro_type: MacroType::Standard,
            delay_mode: DelayMode::Recorded,
        }
    }
}

impl Preferences {
    /// sc restores each persisted zero delay from Co before mounting the
    /// editor. Live preference updates do not perform this startup conversion.
    pub fn restored(mut self) -> Self {
        let defaults = Self::default();
        if self.fixed == Seconds::ZERO {
            self.fixed = defaults.fixed;
        }
        for (value, default) in self.random.iter_mut().zip(defaults.random) {
            if *value == Seconds::ZERO {
                *value = default;
            }
        }
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.start_delay > 5 {
            return Err("Macro recording start delay exceeds five seconds".into());
        }
        if self.random.iter().any(|value| value.value() > 5.) {
            return Err("Macro random delay exceeds five seconds".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macro_library::MacroLibraryFile;
    use serde_json::json;

    #[test]
    fn source_defaults_migrate_only_missing_editor_preferences_and_roundtrip_accepted_values() {
        let old: MacroLibraryFile = serde_json::from_value(json!({
            "next_id":1,"entries":[],"current":null,"tutorial":"complete"
        }))
        .unwrap();
        assert!(old.recording_preferences.is_none());
        let mut updated = old.clone();
        updated.recording_preferences = Some(Preferences {
            start_delay: 5,
            fixed: Seconds::new(0.25).unwrap(),
            random: [Seconds::new(0.1).unwrap(), Seconds::new(0.5).unwrap()],
            movement: MovementMode::Relative,
            macro_type: MacroType::Sequence,
            delay_mode: DelayMode::Randomized,
        });
        updated.validate().unwrap();
        assert!(!updated.same_document(&old));
        let decoded: MacroLibraryFile =
            serde_json::from_slice(&serde_json::to_vec(&updated).unwrap()).unwrap();
        assert!(decoded.same_document(&updated));
    }

    #[test]
    fn invalid_delays_and_preferences_are_explicit_boundary_errors() {
        for value in [-1., f64::INFINITY, f64::NAN] {
            assert!(Seconds::new(value).is_err());
        }
        let mut profile = Preferences::default();
        profile.start_delay = 6;
        assert!(profile.validate().is_err());
        profile.start_delay = 0;
        profile.random = [Seconds::new(6.).unwrap(), Seconds::ZERO];
        assert!(profile.validate().is_err());
        let mut serialized = serde_json::to_value(Preferences::default()).unwrap();
        serialized["fixed"] = json!("invalid");
        assert!(serde_json::from_value::<Preferences>(serialized).is_err());
    }

    #[test]
    fn startup_restores_zero_delays_independently_and_preserves_nonzero_preferences() {
        let defaults = Preferences::default();
        assert_eq!(defaults.fixed.value(), 0.2);
        assert_eq!(defaults.random.map(Seconds::value), [0.2, 0.5]);
        let live = Preferences {
            start_delay: 4,
            fixed: Seconds::ZERO,
            random: [Seconds::ZERO, Seconds::new(0.1).unwrap()],
            movement: MovementMode::Relative,
            macro_type: MacroType::Sequence,
            delay_mode: DelayMode::Randomized,
        };
        live.validate().unwrap();
        let decoded: Preferences =
            serde_json::from_value(serde_json::to_value(&live).unwrap()).unwrap();
        assert_eq!(decoded, live);
        let restored = decoded.restored();
        assert_eq!(restored.fixed.value(), 0.2);
        assert_eq!(restored.random.map(Seconds::value), [0.2, 0.1]);
        assert_eq!(restored.start_delay, 4);
        assert_eq!(restored.movement, MovementMode::Relative);
        assert_eq!(restored.macro_type, MacroType::Sequence);
        assert_eq!(restored.delay_mode, DelayMode::Randomized);
        restored.validate().unwrap();
        assert_eq!(restored.clone().restored(), restored);
        let restored = Preferences {
            fixed: Seconds::new(0.125).unwrap(),
            random: [Seconds::new(0.05).unwrap(), Seconds::ZERO],
            ..defaults
        }
        .restored();
        assert_eq!(restored.fixed.value(), 0.125);
        assert_eq!(restored.random.map(Seconds::value), [0.05, 0.5]);
    }
}

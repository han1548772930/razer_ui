//! Typed UI flags at the application's localStorage boundary. Legacy files
//! are read only for migration and remain available as recovery copies.
use anyhow::Context as _;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flag {
    KeyboardCalibrationIntro,
    MagneticCalibrationSkipPrompt,
    HueTutorial,
    ArmoryIntroduction,
    DisplayNotifications,
}

impl Flag {
    pub const ALL: [Self; 5] = [
        Self::KeyboardCalibrationIntro,
        Self::MagneticCalibrationSkipPrompt,
        Self::HueTutorial,
        Self::ArmoryIntroduction,
        Self::DisplayNotifications,
    ];

    pub const fn default_value(self) -> bool {
        !matches!(self, Self::MagneticCalibrationSkipPrompt)
    }

    pub const fn initial_value(self) -> bool {
        // 720 notification component starts unchecked, then reads on mount.
        !matches!(self, Self::DisplayNotifications) && self.default_value()
    }

    pub const fn publish_on_observe(self) -> bool {
        matches!(self, Self::DisplayNotifications)
    }

    fn key(self) -> &'static str {
        match self {
            Self::KeyboardCalibrationIntro => "showNotificationBannerCalibration",
            Self::MagneticCalibrationSkipPrompt => "MagneticSwitchCalibrationPrompt",
            Self::HueTutorial => "isShowTutorialHue",
            Self::ArmoryIntroduction => "isShowArmoryIntroductionBanner",
            Self::DisplayNotifications => crate::notification_preferences::DISPLAY_KEY,
        }
    }

    fn initialize_missing(self) -> bool {
        // Current 740/746 module 63183 initializes its missing key on mount.
        matches!(self, Self::KeyboardCalibrationIntro)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Current,
    MigratedLegacy,
    Initialized,
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    value: Option<bool>,
    origin: Origin,
}
impl Observation {
    pub const fn current(value: bool) -> Self {
        Self {
            value: Some(value),
            origin: Origin::Current,
        }
    }
    pub const fn missing() -> Self {
        Self {
            value: None,
            origin: Origin::Missing,
        }
    }
    pub const fn value(self) -> Option<bool> {
        self.value
    }
    pub const fn origin(self) -> Origin {
        self.origin
    }
}

fn read_current(path: &Path, flag: Flag) -> anyhow::Result<Option<bool>> {
    crate::source_local::get_raw(path, flag.key())?
        .map(|raw| {
            serde_json::from_str::<bool>(&raw)
                .with_context(|| format!("invalid {:?} UI preference", flag))
        })
        .transpose()
}

fn validate(value: serde_json::Value, flag: Flag) -> anyhow::Result<bool> {
    value
        .as_bool()
        .with_context(|| format!("invalid {:?} UI preference", flag))
}

fn insert_missing(path: &Path, flag: Flag, value: bool) -> anyhow::Result<bool> {
    crate::source_local::update(path, flag.key(), |old| {
        if let Some(old) = old {
            validate(old, flag)?;
            Ok(None)
        } else {
            Ok(Some(serde_json::Value::Bool(value)))
        }
    })
}

pub(crate) fn load_at(path: &Path, flag: Flag) -> anyhow::Result<Observation> {
    if let Some(value) = read_current(path, flag)? {
        return Ok(Observation {
            value: Some(value),
            origin: Origin::Current,
        });
    }
    if flag == Flag::DisplayNotifications {
        // Only the workspace's retired explicit choice migrates for this key.
        // The retained state owner runs the source writer after observation.
        return Ok(Observation::missing());
    }
    let legacy = path.with_file_name(format!("{}.json", flag.key()));
    let previous = crate::read_document(&legacy, |text| {
        serde_json::from_str::<bool>(text)
            .with_context(|| format!("invalid legacy {:?} UI preference", flag))
    })?;
    let (value, origin) = match previous {
        Some(value) => (value, Origin::MigratedLegacy),
        None if flag.initialize_missing() => (flag.default_value(), Origin::Initialized),
        None => {
            return Ok(Observation {
                value: None,
                origin: Origin::Missing,
            });
        }
    };
    let inserted = insert_missing(path, flag, value)?;
    Ok(Observation {
        value: read_current(path, flag)?,
        origin: if inserted { origin } else { Origin::Current },
    })
}

fn save_at(path: &Path, flag: Flag, value: bool) -> anyhow::Result<()> {
    if flag == Flag::DisplayNotifications {
        return crate::notification_preferences::save_at(path, value);
    }
    crate::source_local::update(path, flag.key(), |previous| {
        if let Some(previous) = previous {
            validate(previous, flag)?;
        }
        Ok(Some(serde_json::Value::Bool(value)))
    })?;
    Ok(())
}

/// Call from the retained preference worker, never from a UI callback.
pub fn load(flag: Flag) -> anyhow::Result<Observation> {
    load_at(&crate::source_local::path()?, flag)
}

/// Atomic publication preserves unrelated raw entries and rejects malformed
/// existing values. Success acknowledges only this local preference file.
pub fn save(flag: Flag, value: bool) -> anyhow::Result<()> {
    save_at(&crate::source_local::path()?, flag, value)
}

#[cfg(test)]
#[path = "ui_preferences/tests.rs"]
mod tests;

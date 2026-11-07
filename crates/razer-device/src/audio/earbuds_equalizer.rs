//! Firmware EQ submissions are local configuration and acknowledged writes,
//! never independently observed EQ: the mounted feature has no getter.
use crate::earbuds_protocol40::{CommandHeader, Protocol40Session, Sleeper};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Game,
    Music,
    Movie,
    Custom,
    Voice,
}

impl Preset {
    pub fn key(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Music => "music",
            Self::Movie => "movie",
            Self::Custom => "custom",
            Self::Voice => "voice",
        }
    }
    fn index(self, product_id: u32) -> u8 {
        match self {
            Self::Game => 1,
            Self::Music => 2,
            Self::Movie => 3,
            Self::Custom => 31,
            Self::Voice if product_id != 2689 => 4,
            Self::Voice => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Standard,
    Esports,
}
impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Esports => "esports",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    pub mode: Mode,
    pub preset: Preset,
    pub bands: [i8; 10],
}
impl Setting {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.bands.iter().all(|level| (-6..=6).contains(level)),
            "Earbud EQ band is outside -6..6"
        );
        Ok(())
    }
}

pub fn apply<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    product_id: u32,
    setting: Setting,
    validate: &dyn Fn() -> Result<()>,
) -> Result<()> {
    ensure!(
        crate::earbuds_protocol40::supports_auto_pause(product_id),
        "Product has no current earbud firmware EQ"
    );
    setting.validate()?;
    session
        .send_command(
            CommandHeader::new([4, 24, 0]),
            &[1, 1, 0, setting.preset.index(product_id)],
            validate,
        )
        .context("Earbud EQ preset command was not confirmed")?;
    let mut payload = [0; 12];
    payload[..2].copy_from_slice(&[1, 1]);
    for (byte, level) in payload[2..].iter_mut().zip(setting.bands) {
        *byte = if level < 0 {
            0x80 | level.unsigned_abs()
        } else {
            level as u8
        };
    }
    session
        .send_command(CommandHeader::new([12, 24, 1]), &payload, validate)
        .context("Earbud EQ preset was acknowledged; band command did not complete")?;
    Ok(())
}

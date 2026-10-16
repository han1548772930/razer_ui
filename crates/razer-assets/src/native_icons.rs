//! Installation-critical native rasters, shared by the tray and resource loader.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NativeIcon {
    Notification16,
    Application,
    Notification24,
    Notification32,
    Notification40,
    Notification48,
    Notification64,
    SettingsDark,
    SettingsLight,
}

impl NativeIcon {
    const ALL: [Self; 9] = [
        Self::Notification16,
        Self::Application,
        Self::Notification24,
        Self::Notification32,
        Self::Notification40,
        Self::Notification48,
        Self::Notification64,
        Self::SettingsDark,
        Self::SettingsLight,
    ];

    pub fn for_notification_size(requested: u32) -> Self {
        Self::ALL[..7]
            .iter()
            .copied()
            .find(|icon| icon.pixels() >= requested)
            .unwrap_or(Self::Notification64)
    }

    pub const fn pixels(self) -> u32 {
        match self {
            Self::Notification16 => 16,
            Self::Application => 20,
            Self::Notification24 => 24,
            Self::Notification32 => 32,
            Self::Notification40 => 40,
            Self::Notification48 => 48,
            Self::Notification64 => 64,
            Self::SettingsDark | Self::SettingsLight => 14,
        }
    }

    fn filename(self) -> &'static str {
        match self {
            Self::Notification16 => "a64b8a64b721fb74-tray-native.rgba",
            Self::Application => "f979021796dfd342-tray-app.rgba",
            Self::Notification24 => "81851a9004a803d3-tray-icon-24.rgba",
            Self::Notification32 => "71b354f660adb5db-tray-icon.rgba",
            Self::Notification40 => "f8fd980e3bec82fe-tray-icon-40.rgba",
            Self::Notification48 => "5e5df98271550aca-tray-icon-48.rgba",
            Self::Notification64 => "afb1350c23ce2416-tray-icon-64.rgba",
            Self::SettingsDark => "7747f51f284ded65-tray-gear-dark.rgba",
            Self::SettingsLight => "33892a0b56b91958-tray-gear-light.rgba",
        }
    }

    pub fn rgba(self) -> &'static [u8] {
        match self {
            Self::Notification16 => include_bytes!(
                "../../../assets/synapse/shared/pending-source-audit/a64b8a64b721fb74-tray-native.rgba"
            ),
            Self::Application => include_bytes!(
                "../../../assets/synapse/shared/pending-source-audit/f979021796dfd342-tray-app.rgba"
            ),
            Self::Notification24 => include_bytes!(
                "../../../assets/synapse/shared/pending-source-audit/81851a9004a803d3-tray-icon-24.rgba"
            ),
            Self::Notification32 => include_bytes!(
                "../../../assets/synapse/shared/pending-source-audit/71b354f660adb5db-tray-icon.rgba"
            ),
            Self::Notification40 => include_bytes!(
                "../../../assets/synapse/shared/pending-source-audit/f8fd980e3bec82fe-tray-icon-40.rgba"
            ),
            Self::Notification48 => include_bytes!(
                "../../../assets/synapse/shared/pending-source-audit/5e5df98271550aca-tray-icon-48.rgba"
            ),
            Self::Notification64 => include_bytes!(
                "../../../assets/synapse/shared/pending-source-audit/afb1350c23ce2416-tray-icon-64.rgba"
            ),
            Self::SettingsDark => include_bytes!(
                "../../../assets/synapse/shared/pending-source-audit/7747f51f284ded65-tray-gear-dark.rgba"
            ),
            Self::SettingsLight => include_bytes!(
                "../../../assets/synapse/shared/pending-source-audit/33892a0b56b91958-tray-gear-light.rgba"
            ),
        }
    }
}

pub(super) fn embedded(path: &str) -> Option<&'static [u8]> {
    let filename = path.strip_prefix("assets/synapse/shared/pending-source-audit/")?;
    NativeIcon::ALL
        .into_iter()
        .find(|icon| icon.filename() == filename)
        .map(NativeIcon::rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_rasters_keep_dimensions_and_notification_selection() {
        for icon in NativeIcon::ALL {
            assert_eq!(
                icon.rgba().len(),
                (icon.pixels() * icon.pixels() * 4) as usize
            );
        }
        for (requested, expected) in [
            (0, 16),
            (16, 16),
            (17, 20),
            (25, 32),
            (48, 48),
            (49, 64),
            (100, 64),
        ] {
            assert_eq!(
                NativeIcon::for_notification_size(requested).pixels(),
                expected
            );
        }
        assert!(embedded("assets/unknown.rgba").is_none());
    }
}

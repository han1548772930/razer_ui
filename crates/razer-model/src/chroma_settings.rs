//! Local Chroma preferences. These values never acknowledge host or device writes.
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppFilter {
    #[default]
    All,
    Apps,
    Games,
}
impl AppFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Apps, Self::Games];

    pub const fn id(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Apps => "apps",
            Self::Games => "games",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|filter| filter.id() == id)
    }

    pub const fn position(self) -> usize {
        match self {
            Self::All => 0,
            Self::Apps => 1,
            Self::Games => 2,
        }
    }
}
impl<'de> Deserialize<'de> for AppFilter {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            Name(String),
            LegacyPosition(u64),
        }
        match Stored::deserialize(deserializer)? {
            Stored::Name(name) => Self::from_id(&name)
                .ok_or_else(|| serde::de::Error::custom("unknown Chroma application filter")),
            Stored::LegacyPosition(0) => Ok(Self::All),
            Stored::LegacyPosition(1) => Ok(Self::Apps),
            Stored::LegacyPosition(2) => Ok(Self::Games),
            Stored::LegacyPosition(_) => Err(serde::de::Error::custom(
                "invalid legacy Chroma application filter position",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashboardGroup {
    ApplyEffects,
    Devices,
}
impl DashboardGroup {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ApplyEffects => "chroma-apply-effects",
            Self::Devices => "chroma-device-group",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsDraft {
    auto_start: Option<bool>,
    start_minimized: Option<bool>,
    notifications: Option<bool>,
    dynamic_lighting: Option<bool>,
    tutorials: BTreeMap<String, bool>,
}
impl SettingsDraft {
    /// Current Settings useHs initializes both startup editors to true.
    pub fn auto_start_enabled(&self) -> bool {
        self.auto_start.unwrap_or(true)
    }
    pub fn start_minimized_enabled(&self) -> bool {
        self.start_minimized.unwrap_or(true)
    }
    /// An absent source displayNotifications key is enabled; false survives.
    pub fn notifications_enabled(&self) -> bool {
        self.notifications.unwrap_or(true)
    }
    /// Current useDe initializes the local mode editor to false.
    pub fn dynamic_lighting_enabled(&self) -> bool {
        self.dynamic_lighting.unwrap_or(false)
    }
    pub fn tutorial_enabled(&self, key: &str) -> bool {
        self.tutorials.get(key) == Some(&true)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DashboardPreferences {
    introduction: bool,
    onboard: bool,
    auto_prioritize: bool,
    show_disabled: bool,
    filter: AppFilter,
    collapsed: BTreeSet<String>,
}
impl Default for DashboardPreferences {
    fn default() -> Self {
        // Current Dashboard Se/useJ defaults and missing tutorial-key behavior.
        Self {
            introduction: true,
            onboard: true,
            auto_prioritize: false,
            show_disabled: true,
            filter: AppFilter::All,
            collapsed: BTreeSet::new(),
        }
    }
}
impl DashboardPreferences {
    pub fn introduction_enabled(&self) -> bool {
        self.introduction
    }
    pub fn onboard_enabled(&self) -> bool {
        self.onboard
    }
    pub fn auto_prioritize_enabled(&self) -> bool {
        self.auto_prioritize
    }
    pub fn show_disabled(&self) -> bool {
        self.show_disabled
    }
    pub fn filter(&self) -> AppFilter {
        self.filter
    }
    pub fn is_collapsed(&self, group: DashboardGroup) -> bool {
        self.collapsed.contains(group.id())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    settings: SettingsDraft,
    dashboard: DashboardPreferences,
}
impl Preferences {
    pub fn from_parts(settings: SettingsDraft, dashboard: DashboardPreferences) -> Self {
        Self {
            settings,
            dashboard,
        }
    }
    pub fn settings(&self) -> &SettingsDraft {
        &self.settings
    }
    pub fn dashboard(&self) -> &DashboardPreferences {
        &self.dashboard
    }
    pub fn apply(&mut self, change: &Change) {
        match change {
            Change::AutoStart(value) => self.settings.auto_start = Some(*value),
            Change::StartMinimized(value) => self.settings.start_minimized = Some(*value),
            Change::Notifications(value) => self.settings.notifications = Some(*value),
            Change::DynamicLighting(value) => self.settings.dynamic_lighting = Some(*value),
            Change::ResetTutorials(keys) => {
                for key in keys {
                    self.settings.tutorials.insert(key.clone(), true);
                }
            }
            Change::Introduction(value) => self.dashboard.introduction = *value,
            Change::Onboard(value) => self.dashboard.onboard = *value,
            Change::AutoPrioritize(value) => self.dashboard.auto_prioritize = *value,
            Change::ShowDisabled(value) => self.dashboard.show_disabled = *value,
            Change::Filter(value) => self.dashboard.filter = *value,
            Change::GroupCollapsed { group, collapsed } => {
                if *collapsed {
                    self.dashboard.collapsed.insert(group.id().into());
                } else {
                    self.dashboard.collapsed.remove(group.id());
                }
            }
        }
    }
}

/// Idempotent assignments let a later accepted save include unacknowledged edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    AutoStart(bool),
    StartMinimized(bool),
    Notifications(bool),
    DynamicLighting(bool),
    ResetTutorials(Vec<String>),
    Introduction(bool),
    Onboard(bool),
    AutoPrioritize(bool),
    ShowDisabled(bool),
    Filter(AppFilter),
    GroupCollapsed {
        group: DashboardGroup,
        collapsed: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_defaults_preserve_explicit_false_choices() {
        let settings: SettingsDraft = serde_json::from_str("{}").unwrap();
        assert!(settings.auto_start_enabled());
        assert!(settings.start_minimized_enabled());
        assert!(settings.notifications_enabled());
        assert!(!settings.dynamic_lighting_enabled());
        let settings: SettingsDraft =
            serde_json::from_str(r#"{"notifications":false,"auto_start":false}"#).unwrap();
        assert!(!settings.notifications_enabled());
        assert!(!settings.auto_start_enabled());
    }

    #[test]
    fn filter_migration_uses_domain_identity_and_rejects_unknown_positions() {
        for (position, name) in [(0, "all"), (1, "apps"), (2, "games")] {
            let value: AppFilter = serde_json::from_str(&position.to_string()).unwrap();
            assert_eq!(value.position(), position);
            assert_eq!(
                serde_json::to_string(&value).unwrap(),
                format!("\"{name}\"")
            );
        }
        assert!(serde_json::from_str::<AppFilter>("3").is_err());
        assert!(serde_json::from_str::<DashboardPreferences>(r#"{"onboard":null}"#).is_err());
    }

    #[test]
    fn repeated_assignments_keep_dynamic_lighting_and_group_state() {
        let mut values = Preferences::default();
        for change in [
            Change::DynamicLighting(true),
            Change::GroupCollapsed {
                group: DashboardGroup::Devices,
                collapsed: true,
            },
        ] {
            values.apply(&change);
            let first = values.clone();
            values.apply(&change);
            assert_eq!(values, first);
        }
        assert!(values.settings().dynamic_lighting_enabled());
        assert!(values.dashboard().is_collapsed(DashboardGroup::Devices));
    }
}

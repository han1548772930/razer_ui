//! Current controller MW brightness runtime transitions. These decisions do not
//! change the persisted brightness, and a decision is not a device write receipt.
use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub is_shutdown: bool,
    pub is_display_on: bool,
    pub is_idle_enabled: bool,
    pub idle_minutes: u8,
}
impl Settings {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            (1..=15).contains(&self.idle_minutes),
            "Controller idle threshold exceeds its source limits"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Brightness {
    pub is_enabled: bool,
    pub value: u8,
}
impl Brightness {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.value <= 100,
            "Controller brightness exceeds 100 percent"
        );
        Ok(())
    }
    pub fn wire_value(&self) -> u8 {
        if self.is_enabled { self.value } else { 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    DisplayOff,
    DisplayOn,
    DisplayDimmed,
    Idle { milliseconds: u32 },
    Shutdown,
}

#[derive(Clone, Debug)]
pub struct Policy {
    settings: Settings,
    brightness: Brightness,
    off_by_runtime: bool,
    system_idle_handled: bool,
}
impl Policy {
    pub fn new(settings: Settings, brightness: Brightness) -> anyhow::Result<Self> {
        settings.validate()?;
        brightness.validate()?;
        Ok(Self {
            settings,
            brightness,
            off_by_runtime: false,
            system_idle_handled: false,
        })
    }
    pub fn update(&mut self, settings: Settings, brightness: Brightness) -> anyhow::Result<()> {
        settings.validate()?;
        brightness.validate()?;
        self.settings = settings;
        self.brightness = brightness;
        Ok(())
    }
    pub fn brightness(&self) -> &Brightness {
        &self.brightness
    }
    pub fn apply(&mut self, event: Event) -> Option<u8> {
        if event == Event::Shutdown {
            return self.settings.is_shutdown.then_some(0);
        }
        if !self.brightness.is_enabled {
            return None;
        }
        if self.off_by_runtime {
            let restore = match event {
                Event::DisplayOn if self.settings.is_display_on => true,
                Event::Idle { milliseconds }
                    if self.settings.is_idle_enabled
                        && milliseconds <= u32::from(self.settings.idle_minutes) * 60_000
                        && self.system_idle_handled =>
                {
                    self.system_idle_handled = false;
                    true
                }
                _ => false,
            };
            if restore {
                self.off_by_runtime = false;
                return Some(self.brightness.wire_value());
            }
            return None;
        }
        let off = match event {
            Event::DisplayOff => self.settings.is_display_on,
            Event::Idle { milliseconds }
                if self.settings.is_idle_enabled
                    && milliseconds >= u32::from(self.settings.idle_minutes) * 60_000
                    && !self.system_idle_handled =>
            {
                self.system_idle_handled = true;
                true
            }
            _ => false,
        };
        if off {
            self.off_by_runtime = true;
            Some(0)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> Policy {
        Policy::new(
            Settings {
                is_shutdown: true,
                is_display_on: true,
                is_idle_enabled: true,
                idle_minutes: 5,
            },
            Brightness {
                is_enabled: true,
                value: 73,
            },
        )
        .unwrap()
    }
    #[test]
    fn controller_power_policy_runtime_off_restores_saved_brightness_without_mutation() {
        let mut policy = policy();
        assert_eq!(policy.apply(Event::DisplayDimmed), None);
        assert_eq!(policy.apply(Event::DisplayOff), Some(0));
        assert_eq!(policy.apply(Event::DisplayOff), None);
        assert_eq!(policy.apply(Event::DisplayOn), Some(73));
        assert_eq!(policy.brightness().value, 73);
    }
    #[test]
    fn controller_power_policy_idle_boundaries_match_source_and_setting_changes_do_not_restore() {
        let mut policy = policy();
        assert_eq!(
            policy.apply(Event::Idle {
                milliseconds: 299_999
            }),
            None
        );
        assert_eq!(
            policy.apply(Event::Idle {
                milliseconds: 300_000
            }),
            Some(0)
        );
        assert_eq!(
            policy.apply(Event::Idle {
                milliseconds: 300_001
            }),
            None
        );
        assert_eq!(
            policy.apply(Event::Idle {
                milliseconds: 300_000
            }),
            Some(73)
        );
        assert_eq!(policy.apply(Event::DisplayOff), Some(0));
        policy
            .update(
                Settings {
                    is_display_on: false,
                    ..policy.settings.clone()
                },
                policy.brightness.clone(),
            )
            .unwrap();
        assert_eq!(policy.apply(Event::DisplayOn), None);
        assert_eq!(policy.apply(Event::Shutdown), Some(0));
    }
    #[test]
    fn controller_power_policy_disabled_brightness_and_unsubscribed_events_do_not_write() {
        let mut policy = policy();
        policy
            .update(
                Settings {
                    is_shutdown: false,
                    is_display_on: false,
                    is_idle_enabled: false,
                    idle_minutes: 1,
                },
                Brightness {
                    is_enabled: false,
                    value: 73,
                },
            )
            .unwrap();
        assert_eq!(policy.apply(Event::DisplayOff), None);
        assert_eq!(
            policy.apply(Event::Idle {
                milliseconds: u32::MAX
            }),
            None
        );
        assert_eq!(policy.apply(Event::Shutdown), None);
    }
}

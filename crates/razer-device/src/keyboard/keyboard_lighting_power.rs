//! Current single-mode keyboard brightness event policy. Requested output,
//! persisted profile fields and independently observed device values stay separate.
use super::{BrightnessIntent, BrightnessSubmission};
use crate::keyboard_current_profile::{Brightness, LightingOff};
use anyhow::ensure;

pub fn validate(settings: &LightingOff, brightness: &Brightness) -> anyhow::Result<()> {
    ensure!(
        (1..=15).contains(&settings.idle_minutes),
        "Keyboard idle duration is outside 1..15 minutes"
    );
    ensure!(
        brightness.value <= 100,
        "Keyboard brightness is outside 0..100"
    );
    Ok(())
}

pub fn supports_resume_probe(product_id: u32) -> bool {
    crate::keyboard_metadata::hid_capability(product_id, product_id).is_some()
        && !matches!(product_id, 721 | 739 | 752 | 756)
}

pub fn supports_transport(product_id: u32, ble: bool) -> bool {
    crate::keyboard_metadata::hid_capability(product_id, product_id).is_some()
        && !(product_id == 756 && ble)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recorder {
    Free,
    Software,
    Otfm,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    DisplayOff,
    DisplayOn,
    DisplayDimmed,
    Idle { milliseconds: u32 },
    RecorderStarted(Recorder),
    RecorderStopped(Recorder),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    OffByRuntime,
}

#[derive(Clone, Debug)]
pub struct Policy {
    settings: LightingOff,
    brightness: Brightness,
    recorder: Recorder,
    phase: Phase,
    system_idle_handled: bool,
    display_subscribed: bool,
    idle_subscribed: bool,
    requested_off: Option<bool>,
}

impl Policy {
    /// Loading a local policy starts no device write.
    pub fn new(
        settings: LightingOff,
        brightness: Brightness,
        recorder: Recorder,
    ) -> anyhow::Result<Self> {
        validate(&settings, &brightness)?;
        let enabled = brightness.is_enabled;
        Ok(Self {
            display_subscribed: enabled && settings.is_display_on,
            idle_subscribed: enabled && recorder == Recorder::Free && settings.is_idle_enabled,
            settings,
            brightness,
            recorder,
            phase: Phase::Idle,
            system_idle_handled: false,
            requested_off: None,
        })
    }

    pub fn display_subscribed(&self) -> bool {
        self.display_subscribed
    }
    pub fn recorder(&self) -> Recorder {
        self.recorder
    }

    pub fn restore_local_brightness(&mut self, brightness: Brightness) -> anyhow::Result<()> {
        validate(&self.settings, &brightness)?;
        self.brightness = brightness;
        Ok(())
    }

    /// Source ON_SWITCH_OFF_SETTING commits context after local persistence.
    /// Its offByRuntime branch can restore output without changing that FSM phase.
    pub fn configure(
        &mut self,
        settings: LightingOff,
    ) -> anyhow::Result<Option<BrightnessSubmission>> {
        validate(&settings, &self.brightness)?;
        self.display_subscribed = settings.is_display_on && self.brightness.is_enabled;
        self.idle_subscribed = settings.is_idle_enabled
            && self.brightness.is_enabled
            && self.recorder == Recorder::Free;
        let restore = self.phase == Phase::OffByRuntime
            && self.recorder == Recorder::Free
            && self.brightness.is_enabled
            && !settings.is_display_on
            && !settings.is_idle_enabled;
        self.settings = settings;
        Ok(if restore { self.output(true) } else { None })
    }

    /// A user edit updates the source profile even while the recorder owns
    /// hardware. Only the free-recorder branch admits the physical submission.
    pub fn submitted(&mut self, submission: BrightnessSubmission) -> anyhow::Result<bool> {
        submission.validate()?;
        self.brightness.value = submission.value();
        self.brightness.is_enabled = submission.enabled();
        if submission.intent() == BrightnessIntent::Enabled {
            let registered = self.recorder == Recorder::Free && self.phase == Phase::Idle;
            self.display_subscribed =
                registered && submission.enabled() && self.settings.is_display_on;
            self.idle_subscribed =
                registered && submission.enabled() && self.settings.is_idle_enabled;
        }
        if self.recorder != Recorder::Free {
            return Ok(false);
        }
        self.requested_off = Some(submission.effective_percent() == 0);
        Ok(true)
    }

    fn output(&mut self, enabled: bool) -> Option<BrightnessSubmission> {
        let off = !enabled || self.brightness.value == 0;
        if self.requested_off == Some(off) {
            return None;
        }
        self.requested_off = Some(off);
        Some(
            BrightnessSubmission::new(self.brightness.value, enabled, BrightnessIntent::Value)
                .expect("Validated profile brightness"),
        )
    }

    pub fn event(&mut self, event: Event) -> Option<BrightnessSubmission> {
        match event {
            Event::RecorderStarted(mode) if mode != Recorder::Free => {
                self.recorder = mode;
                self.phase = Phase::Idle;
                self.display_subscribed = self.brightness.is_enabled && self.settings.is_display_on;
                self.idle_subscribed = false;
                return None;
            }
            Event::RecorderStopped(mode)
                if self.recorder == Recorder::Unknown
                    || self.recorder == mode && mode != Recorder::Free =>
            {
                self.recorder = Recorder::Free;
                self.phase = Phase::Idle;
                // Actual dispatchers execute an empty maker here. The graph's
                // restore/registration descriptions do not write hardware or
                // reinstall the idle subscription removed by recorder start.
                return None;
            }
            Event::RecorderStarted(_) | Event::RecorderStopped(_) => return None,
            _ if self.recorder != Recorder::Free => return None,
            _ => {}
        }
        let threshold = self.settings.idle_minutes * 60_000;
        if self.phase == Phase::OffByRuntime {
            let restore = match event {
                Event::DisplayOn if self.display_subscribed => true,
                Event::Idle { milliseconds }
                    if self.idle_subscribed
                        && milliseconds <= threshold
                        && self.system_idle_handled =>
                {
                    self.system_idle_handled = false;
                    true
                }
                _ => false,
            };
            if restore {
                self.phase = Phase::Idle;
                return self.output(true);
            }
            return None;
        }
        let off = match event {
            Event::DisplayOff => self.display_subscribed && self.brightness.is_enabled,
            Event::Idle { milliseconds }
                if self.idle_subscribed
                    && milliseconds >= threshold
                    && !self.system_idle_handled =>
            {
                self.system_idle_handled = true;
                true
            }
            _ => false,
        };
        if off {
            self.phase = Phase::OffByRuntime;
            self.output(false)
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
            LightingOff {
                is_display_on: true,
                is_idle_enabled: true,
                idle_minutes: 5,
                extensions: Default::default(),
            },
            Brightness {
                is_enabled: true,
                value: 73,
                extensions: Default::default(),
            },
            Recorder::Free,
        )
        .unwrap()
    }
    #[test]
    fn keyboard_power_loading_and_dimmed_events_never_write() {
        let mut policy = policy();
        assert_eq!(policy.event(Event::DisplayDimmed), None);
        assert_eq!(
            policy.event(Event::Idle {
                milliseconds: 299_999
            }),
            None
        );
    }
    #[test]
    fn keyboard_power_display_and_idle_restore_profile_without_mutation() {
        let mut policy = policy();
        assert_eq!(
            policy.event(Event::DisplayOff).unwrap().effective_percent(),
            0
        );
        assert_eq!(policy.event(Event::DisplayOff), None);
        assert_eq!(
            policy.event(Event::DisplayOn).unwrap().effective_percent(),
            73
        );
        assert_eq!(
            policy
                .event(Event::Idle {
                    milliseconds: 300_000
                })
                .unwrap()
                .effective_percent(),
            0
        );
        assert_eq!(
            policy.event(Event::Idle {
                milliseconds: 300_001
            }),
            None
        );
        assert_eq!(
            policy
                .event(Event::Idle {
                    milliseconds: 300_000
                })
                .unwrap()
                .effective_percent(),
            73
        );
        assert!(policy.brightness.is_enabled);
        assert_eq!(policy.brightness.value, 73);
    }

    #[test]
    fn keyboard_power_events_execute_real_packet_policy_and_readback() {
        use crate::keyboard_control_test_transport::{Mock, reply, sequence};
        let mut cap = crate::keyboard_settings::capability(600).unwrap().clone();
        cap.transport.max_retry_in = 1;
        cap.transport.max_retry_out = 1;
        cap.transport.sleep_between_out_ms = 0;
        cap.transport.sleep_between_out_in_ms = 0;
        cap.transport.sleep_between_in_ms = 0;
        let device = Mock::new([
            reply(&[1, 0, 0]),
            reply(&[1, 0, 0, 0, 0]),
            reply(&[1, 1, 0]),
            reply(&[1, 0, 186]),
            reply(&[1, 0, 0, 0, 0]),
            reply(&[1, 1, 186]),
        ]);
        let mut policy = policy();
        let mut next = sequence();
        let off = policy.event(Event::DisplayOff).unwrap();
        let off_reply =
            crate::keyboard_settings::apply_brightness(&device, &cap, off, &mut next, || Ok(()))
                .unwrap();
        assert_eq!(off_reply.reading().unwrap().percent, 0);
        let on = policy.event(Event::DisplayOn).unwrap();
        let on_reply =
            crate::keyboard_settings::apply_brightness(&device, &cap, on, &mut next, || Ok(()))
                .unwrap();
        assert_eq!(on_reply.reading().unwrap().percent, 73);
        let sent = device.sent();
        assert_eq!(sent.len(), 6);
        assert_eq!(&sent[0][6..12], &[3, 15, 4, 1, 0, 0]);
        assert_eq!(&sent[3][6..12], &[3, 15, 4, 1, 0, 186]);
        assert_eq!(
            (policy.brightness.value, policy.brightness.is_enabled),
            (73, true)
        );
    }
    #[test]
    fn keyboard_power_clearing_all_conditions_restores_but_retains_source_phase() {
        let mut policy = policy();
        policy.event(Event::DisplayOff).unwrap();
        let mut setting = policy.settings.clone();
        setting.is_display_on = false;
        setting.is_idle_enabled = false;
        assert_eq!(
            policy
                .configure(setting)
                .unwrap()
                .unwrap()
                .effective_percent(),
            73
        );
        assert_eq!(policy.phase, Phase::OffByRuntime);
        assert_eq!(policy.event(Event::DisplayOn), None);
    }
    #[test]
    fn keyboard_power_recorder_blocks_runtime_and_user_hardware_until_matching_stop() {
        let mut policy = policy();
        policy.event(Event::RecorderStarted(Recorder::Software));
        assert_eq!(policy.event(Event::DisplayOff), None);
        assert_eq!(
            policy.event(Event::Idle {
                milliseconds: u32::MAX
            }),
            None
        );
        assert!(
            !policy
                .submitted(BrightnessSubmission::new(42, true, BrightnessIntent::Value).unwrap())
                .unwrap()
        );
        assert_eq!(policy.event(Event::RecorderStopped(Recorder::Otfm)), None);
        assert_eq!(
            policy.event(Event::RecorderStopped(Recorder::Software)),
            None
        );
        assert_eq!(policy.recorder(), Recorder::Free);
        assert_eq!(
            policy.event(Event::Idle {
                milliseconds: 600_000
            }),
            None
        );
        policy.configure(policy.settings.clone()).unwrap();
        assert_eq!(
            policy
                .event(Event::Idle {
                    milliseconds: 600_000
                })
                .unwrap()
                .effective_percent(),
            0
        );
        assert_eq!(
            policy
                .event(Event::Idle { milliseconds: 0 })
                .unwrap()
                .effective_percent(),
            42
        );
    }
    #[test]
    fn keyboard_power_request_cache_is_not_a_measured_value() {
        let mut policy = policy();
        policy.event(Event::DisplayOff).unwrap();
        policy
            .submitted(BrightnessSubmission::new(60, true, BrightnessIntent::Value).unwrap())
            .unwrap();
        assert_eq!(policy.event(Event::DisplayOn), None);
        assert_eq!(policy.phase, Phase::Idle);
    }

    #[test]
    fn keyboard_power_macro_stop_has_no_extra_set_or_idle_registration() {
        for mode in [Recorder::Software, Recorder::Otfm, Recorder::Unknown] {
            let mut policy = policy();
            policy.event(Event::RecorderStarted(mode));
            let before = policy.requested_off;
            assert_eq!(
                policy.event(Event::RecorderStopped(Recorder::Software)),
                None
            );
            assert_eq!(policy.event(Event::RecorderStopped(Recorder::Otfm)), None);
            assert_eq!(policy.requested_off, before);
            assert_eq!(
                policy.event(Event::Idle {
                    milliseconds: 600_000
                }),
                None
            );
        }
    }

    #[test]
    fn keyboard_power_enable_in_runtime_off_uses_missing_registration_defaults() {
        let mut policy = policy();
        policy.event(Event::DisplayOff).unwrap();
        policy
            .submitted(BrightnessSubmission::new(73, false, BrightnessIntent::Enabled).unwrap())
            .unwrap();
        assert!(!policy.display_subscribed());
        assert_eq!(policy.event(Event::DisplayOn), None);
        assert_eq!(policy.event(Event::Idle { milliseconds: 0 }), None);
    }
    #[test]
    fn keyboard_power_real_resume_branch_is_not_inferred_by_family_name() {
        for product in [721, 739, 752, 756] {
            assert!(!supports_resume_probe(product));
        }
        for product in [585, 600, 631, 691, 708, 719] {
            assert!(supports_resume_probe(product));
        }
        assert!(!supports_resume_probe(0));
        assert!(!supports_transport(756, true));
        assert!(supports_transport(756, false));
        assert!(supports_transport(708, true));
    }
}

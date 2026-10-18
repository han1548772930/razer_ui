//! KeyboardExecutorWin timers (12f312/12f3ce/12fb30/12ff70).
use super::Keyboard;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardTiming {
    pub initial_ms: u32,
    pub repeat_ms: u32,
}

impl KeyboardTiming {
    /// Failed SPI reads retain the native constructor's 250/30 ms values.
    pub fn from_system_parameters(delay: Option<u32>, speed: Option<u32>) -> Self {
        Self {
            initial_ms: delay.map_or(250, |value| value.wrapping_mul(250).wrapping_add(250)),
            repeat_ms: speed.map_or(30, |value| {
                (1000.0 / (f64::from(value) * 0.887 + 2.5)) as u32
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Identity {
    scancode: u16,
    flags: u16,
    unit: Option<u16>,
}
impl From<&Keyboard> for Identity {
    fn from(key: &Keyboard) -> Self {
        Self {
            scancode: key.scancode,
            flags: key.flag & !1,
            unit: key.unit,
        }
    }
}

struct AutoRelease {
    key: Keyboard,
    remaining_ms: i32,
}
struct Repeat {
    deadline_us: u64,
    interval_ms: u32,
}

pub struct KeyboardExecutor {
    timing: KeyboardTiming,
    pressed: BTreeSet<Identity>,
    active: Option<Keyboard>,
    repeat: Option<Repeat>,
    smart_stop_ms: Option<i32>,
    releases: BTreeMap<Identity, AutoRelease>,
    release_check_us: Option<u64>,
}

impl KeyboardExecutor {
    pub fn new(timing: KeyboardTiming) -> Self {
        Self {
            timing,
            pressed: BTreeSet::new(),
            active: None,
            repeat: None,
            smart_stop_ms: None,
            releases: BTreeMap::new(),
            release_check_us: None,
        }
    }

    pub fn execute(
        &mut self,
        key: Keyboard,
        smart_stop: bool,
        now_us: u64,
        send: &mut impl FnMut(&Keyboard) -> Result<(), String>,
    ) -> Result<(), String> {
        let identity = Identity::from(&key);
        if key.flag & 1 == 0 {
            if let Some(milliseconds) = key.release_after_ms {
                self.releases.insert(
                    identity,
                    AutoRelease {
                        key: key.clone(),
                        remaining_ms: milliseconds as i32,
                    },
                );
            }
            self.pressed.insert(identity);
            let initial = key.delay_ms.unwrap_or(self.timing.initial_ms);
            self.smart_stop_ms = smart_stop.then(|| {
                self.timing
                    .initial_ms
                    .max(initial)
                    .wrapping_add(self.timing.repeat_ms.wrapping_mul(2)) as i32
            });
            let result = if self.active.as_ref() == Some(&key) {
                Ok(())
            } else {
                send(&key).map(|()| {
                    self.active = Some(key);
                    self.schedule(now_us, initial);
                })
            };
            if self.release_check_us.is_none() && !self.releases.is_empty() {
                self.release_check_us = Some(now_us.saturating_add(250_000));
            }
            result
        } else if self.pressed.remove(&identity) {
            self.releases.remove(&identity);
            let result = send(&key);
            if let Some(active) = &self.active {
                if Identity::from(active) == identity {
                    self.active = None;
                    self.repeat = None;
                    self.smart_stop_ms = None;
                } else {
                    self.schedule(now_us, active.delay_ms.unwrap_or(self.timing.initial_ms));
                }
            }
            result
        } else {
            Ok(())
        }
    }

    pub fn next_deadline_us(&self) -> Option<u64> {
        self.repeat
            .as_ref()
            .map(|repeat| repeat.deadline_us)
            .into_iter()
            .chain(self.release_check_us)
            .min()
    }

    /// Process due owned timers once. Delayed service dispatch does not invent
    /// missed repeats or decrement the 250 ms tracker multiple times.
    pub fn poll(
        &mut self,
        now_us: u64,
        send: &mut impl FnMut(&Keyboard) -> Result<(), String>,
    ) -> Vec<String> {
        let mut errors = Vec::new();
        let release_due = self
            .release_check_us
            .is_some_and(|deadline| deadline <= now_us);
        let repeat_due = self
            .repeat
            .as_ref()
            .is_some_and(|repeat| repeat.deadline_us <= now_us);
        let release_first = release_due
            && (!repeat_due
                || self.release_check_us <= self.repeat.as_ref().map(|repeat| repeat.deadline_us));
        if release_first {
            self.check_releases(now_us, send, &mut errors);
        }
        if self
            .repeat
            .as_ref()
            .is_some_and(|repeat| repeat.deadline_us <= now_us)
        {
            let repeat = self.repeat.take().unwrap();
            let stopped = self.smart_stop_ms.as_mut().is_some_and(|remaining| {
                *remaining = remaining.wrapping_sub(repeat.interval_ms as i32);
                *remaining <= 0
            });
            if !stopped && let Some(key) = self.active.clone() {
                match send(&key) {
                    Ok(()) => self.schedule(now_us, key.delay_ms.unwrap_or(self.timing.repeat_ms)),
                    Err(error) => errors.push(error),
                }
            }
        }
        if release_due && !release_first {
            self.check_releases(now_us, send, &mut errors);
        }
        errors
    }

    /// Called after the mapping owner has sent its held-key releases. Timers
    /// cannot survive endpoint cleanup or submit into a replacement session.
    pub fn stop(&mut self) {
        self.repeat = None;
        self.release_check_us = None;
        self.active = None;
        self.releases.clear();
        self.pressed.clear();
        self.smart_stop_ms = None;
    }

    fn schedule(&mut self, now_us: u64, interval_ms: u32) {
        self.repeat = Some(Repeat {
            deadline_us: now_us.saturating_add(u64::from(interval_ms) * 1000),
            interval_ms,
        });
    }

    fn check_releases(
        &mut self,
        now_us: u64,
        send: &mut impl FnMut(&Keyboard) -> Result<(), String>,
        errors: &mut Vec<String>,
    ) {
        let mut expired = Vec::new();
        self.releases.retain(|_, release| {
            release.remaining_ms = release.remaining_ms.wrapping_sub(250);
            if release.remaining_ms < 0 {
                let mut key = release.key.clone();
                key.flag |= 1;
                expired.push(key);
                false
            } else {
                true
            }
        });
        for key in expired {
            if let Err(error) = self.execute(key, false, now_us, send) {
                errors.push(error);
            }
        }
        self.release_check_us = (!self.releases.is_empty()).then(|| now_us.saturating_add(250_000));
    }
}

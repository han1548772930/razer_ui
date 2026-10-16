//! Current 3594 qM and 3595 tU exposure effects, retained in the current
//! camera-local-pages complete owners. `initialized` is the mounted-effect
//! flag, independent of device observations and transport acknowledgements.
use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct State {
    last_assertion: Option<Instant>,
}

impl State {
    /// lodash throttle(1000, {trailing:false}) belongs to the mounted
    /// exposure component. Each eligible field shares that single throttle.
    pub(super) fn reassert(&mut self, product_id: u32, key: &str, now: Instant) -> bool {
        let name = key.rsplit(':').next().unwrap_or(key);
        if !matches!(product_id, 3594 | 3595)
            || !(matches!(name, "iso" | "shutter" | "metering" | "compensation")
                || product_id == 3595 && name == "focus-lighting")
            || self.last_assertion.is_some_and(|previous| {
                now.saturating_duration_since(previous) < Duration::from_secs(1)
            })
        {
            return false;
        }
        self.last_assertion = Some(now);
        true
    }
}

pub(super) fn apply_metering_effect(
    profile: &mut Value,
    previous: Option<&Value>,
    initialized: bool,
) -> bool {
    let metering = profile.pointer("/camera/autoExposure/metering");
    if !initialized || metering == previous || metering.and_then(Value::as_u64) != Some(5) {
        return false;
    }
    let Some(compensation) = profile.pointer_mut("/camera/autoExposure/compensation") else {
        return false;
    };
    if !compensation.as_f64().is_some_and(|value| value < -1.) {
        return false;
    }
    *compensation = Value::from(-1);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> Value {
        serde_json::json!({"camera":{"autoExposure":{"metering":5,"compensation":-2.5}}})
    }

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn face_transition_dispatches_compensation_after_mount_without_changing_range() {
        let mut value = profile();
        assert!(apply_metering_effect(&mut value, Some(&0.into()), true));
        assert_eq!(value["camera"]["autoExposure"]["compensation"], -1);
    }

    #[test]
    fn initial_mount_and_compensation_only_changes_do_not_run_metering_effect() {
        let mut value = profile();
        assert!(!apply_metering_effect(&mut value, None, false));
        assert!(!apply_metering_effect(&mut value, Some(&5.into()), true));
        assert_eq!(value["camera"]["autoExposure"]["compensation"], -2.5);
    }

    #[test]
    fn camera_exposure_reassertion_is_shared_leading_only_and_owned_by_mount() {
        let mut state = State::default();
        let now = Instant::now();
        assert!(state.reassert(3595, "3595:metering", now));
        assert!(!state.reassert(
            3595,
            "3595:focus-lighting",
            now + Duration::from_millis(999)
        ));
        assert!(state.reassert(3595, "3595:focus-lighting", now + Duration::from_secs(1)));
        assert!(!state.reassert(3595, "3595:iso", now + Duration::from_millis(1001)));
        assert!(!state.reassert(3596, "3596:metering", now + Duration::from_secs(2)));
        assert!(!state.reassert(3594, "3594:focus-lighting", now + Duration::from_secs(2)));
        assert!(!state.reassert(3594, "3594:auto-exposure", now + Duration::from_secs(2)));
        // Returning to CAMERA mounts a fresh memoized throttle.
        state = State::default();
        assert!(state.reassert(3594, "3594:shutter", now));
    }
}

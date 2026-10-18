//! Computed Xbox output callbacks (759D0/81D6E/81FB6/82220).
//! These samples are not device readback or driver completion receipts.
use crate::{
    mapping_native_observation::{Delivery, State as Interval},
    mapping_native_output::GamepadOutputState,
};

#[derive(Default)]
pub struct State {
    mode: Option<Mode>,
}
enum Mode {
    Events { counter: u32 },
    Interval(Interval<GamepadOutputState>),
}
impl State {
    pub fn register_events(&mut self, skip: u32) -> bool {
        if self.mode.is_some() {
            return false;
        }
        // 759D0 stores zero at +512 (threshold), and the supplied skip at
        // +516 (counter). 81D6E increments +516 and compares it with +512.
        self.mode = Some(Mode::Events { counter: skip });
        true
    }
    pub fn register_interval(&mut self, milliseconds: u32) -> bool {
        if self.mode.is_some() {
            return false;
        }
        let mut stream = Interval::default();
        stream.register_interval(milliseconds);
        self.mode = Some(Mode::Interval(stream));
        true
    }
    pub fn observe(
        &mut self,
        previous: GamepadOutputState,
        current: GamepadOutputState,
        tick: u64,
    ) -> Option<Delivery<GamepadOutputState>> {
        if previous == current {
            return None;
        }
        match self.mode.as_mut()? {
            Mode::Events { counter } => {
                let next = counter.wrapping_add(1);
                let overflow = next > 0;
                *counter = 0;
                (overflow || forced(previous, current)).then(|| Delivery {
                    events: vec![current],
                    time_tick: tick,
                })
            }
            Mode::Interval(stream) => stream.observe(0, false, tick, || current),
        }
    }
    pub fn poll(&mut self, tick: u64) -> Vec<Delivery<GamepadOutputState>> {
        match &mut self.mode {
            Some(Mode::Interval(stream)) => stream.poll(tick),
            _ => Vec::new(),
        }
    }
    pub fn next_deadline(&self) -> Option<u64> {
        match &self.mode {
            Some(Mode::Interval(stream)) => stream.next_deadline(),
            _ => None,
        }
    }
}
fn forced(previous: GamepadOutputState, current: GamepadOutputState) -> bool {
    (!previous.buttons & current.buttons) != 0
        || [
            (previous.left_trigger, current.left_trigger),
            (previous.right_trigger, current.right_trigger),
        ]
        .into_iter()
        .any(|(old, new)| old != new && matches!(new, 0 | 255))
        || previous
            .left_thumb
            .into_iter()
            .chain(previous.right_thumb)
            .zip(current.left_thumb.into_iter().chain(current.right_thumb))
            .any(|(old, new)| old != new && matches!(new, i16::MIN | i16::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(value: u8) -> GamepadOutputState {
        GamepadOutputState {
            left_trigger: value,
            ..Default::default()
        }
    }
    #[test]
    fn native_skip_initializes_counter_and_unchanged_output_has_no_callback() {
        let mut stream = State::default();
        assert!(stream.observe(state(0), state(1), 0).is_none());
        assert!(stream.register_events(50));
        assert!(!stream.register_events(u32::MAX));
        assert!(!stream.register_interval(5));
        assert!(stream.observe(state(0), state(0), 0).is_none());
        for n in 1..5 {
            assert_eq!(
                stream
                    .observe(state(n - 1), state(n), n.into())
                    .unwrap()
                    .events,
                vec![state(n)]
            );
        }
    }
    #[test]
    fn wrapped_counter_forces_press_and_extremes_but_not_release_or_midrange() {
        let zero = GamepadOutputState::default();
        let pressed = GamepadOutputState { buttons: 1, ..zero };
        for (old, new, want) in [
            (state(0), state(1), false),
            (pressed, zero, false),
            (zero, pressed, true),
            (state(1), state(0), true),
            (state(1), state(255), true),
            (
                zero,
                GamepadOutputState {
                    right_thumb: [i16::MIN, i16::MAX],
                    ..zero
                },
                true,
            ),
        ] {
            let mut stream = State::default();
            stream.register_events(u32::MAX);
            assert_eq!(stream.observe(old, new, 1).is_some(), want);
            assert!(stream.observe(state(2), state(3), 2).is_some());
        }
    }
    #[test]
    fn interval_keeps_samples_and_schedule_origin_and_zero_interval() {
        let mut stream = State::default();
        stream.register_interval(5);
        assert_eq!(
            stream.observe(state(0), state(1), 100_000).unwrap().events,
            vec![state(1)]
        );
        assert!(stream.observe(state(1), state(2), 102_000).is_none());
        assert!(stream.observe(state(2), state(3), 103_000).is_none());
        assert_eq!(stream.next_deadline(), Some(105_000));
        assert!(stream.poll(104_999).is_empty());
        assert_eq!(
            stream.poll(105_000),
            vec![Delivery {
                events: vec![state(2), state(3)],
                time_tick: 105_000
            }]
        );
        assert!(stream.observe(state(3), state(4), 106_000).is_none());
        assert_eq!(stream.next_deadline(), Some(107_000));
        let mut zero = State::default();
        zero.register_interval(0);
        assert_eq!(
            zero.observe(state(0), state(1), 1).unwrap().events,
            vec![state(1)]
        );
    }
    #[test]
    fn capacity_flush_and_wrapping_interval_follow_native_order() {
        let mut stream = State::default();
        stream.register_interval(5);
        for n in 0..10_000 {
            assert!(
                stream
                    .observe(state((n % 2) as u8), state((1 - n % 2) as u8), 1_000)
                    .is_none()
            );
        }
        assert_eq!(
            stream
                .observe(state(0), state(1), 1_000)
                .unwrap()
                .events
                .len(),
            10_001
        );
        assert!(stream.observe(state(1), state(2), 2_000).is_none());
        assert_eq!(stream.poll(5_000)[0].events, vec![state(2)]);
        assert!(stream.poll(6_000).is_empty());
        let mut wrapped = State::default();
        wrapped.register_interval(u32::MAX);
        wrapped.observe(state(0), state(1), 1);
        assert_eq!(
            wrapped.next_deadline(),
            Some(u64::from(u32::MAX.wrapping_mul(1000)))
        );
    }
}

//! PID747 hardware controller samples and its 15 ms UI delivery throttle.
use serde::{Deserialize, Serialize};

/// MW41243 forwards these thousandth-unit values without the separate
/// controller action's UI multiplication by 1000.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SampleFields")]
pub struct Sample {
    left_thumb_x: i16,
    left_thumb_y: i16,
    right_thumb_x: i16,
    right_thumb_y: i16,
    left_trigger: u16,
    right_trigger: u16,
}

#[derive(Deserialize)]
struct SampleFields {
    left_thumb_x: i16,
    left_thumb_y: i16,
    right_thumb_x: i16,
    right_thumb_y: i16,
    left_trigger: u16,
    right_trigger: u16,
}

impl TryFrom<SampleFields> for Sample {
    type Error = &'static str;

    fn try_from(fields: SampleFields) -> Result<Self, Self::Error> {
        if ![
            fields.left_thumb_x,
            fields.left_thumb_y,
            fields.right_thumb_x,
            fields.right_thumb_y,
        ]
        .into_iter()
        .all(|value| (-1000..=1000).contains(&value))
            || fields.left_trigger > 1000
            || fields.right_trigger > 1000
        {
            return Err("Keypad controller sample exceeds thousandth-unit bounds");
        }
        Ok(Self {
            left_thumb_x: fields.left_thumb_x,
            left_thumb_y: fields.left_thumb_y,
            right_thumb_x: fields.right_thumb_x,
            right_thumb_y: fields.right_thumb_y,
            left_trigger: fields.left_trigger,
            right_trigger: fields.right_trigger,
        })
    }
}

impl Sample {
    pub fn left_thumb_x(self) -> i16 {
        self.left_thumb_x
    }
    pub fn left_thumb_y(self) -> i16 {
        self.left_thumb_y
    }
    pub fn right_thumb_x(self) -> i16 {
        self.right_thumb_x
    }
    pub fn right_thumb_y(self) -> i16 {
        self.right_thumb_y
    }
    pub fn left_trigger(self) -> u16 {
        self.left_trigger
    }
    pub fn right_trigger(self) -> u16 {
        self.right_trigger
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControllerReport {
    pub sample: Sample,
    pub button_event: u8,
    pub dpad_event: u8,
}

// Protocol25KeypadParser module47561 requires exactly 15 bytes and report 0.
// Its five words are unsigned LE values biased by 32768, including the
// shared trigger axis. A normal signed-i16 interpretation has a different zero.
pub fn decode_report(bytes: &[u8]) -> Option<ControllerReport> {
    let [
        0,
        lx0,
        lx1,
        ly0,
        ly1,
        rx0,
        rx1,
        ry0,
        ry1,
        tr0,
        tr1,
        buttons,
        dpad,
        _,
        _,
    ] = *bytes
    else {
        return None;
    };
    let axis = |lo, hi| i32::from(u16::from_le_bytes([lo, hi])) - 32768;
    let trigger = axis(tr0, tr1);
    Some(ControllerReport {
        sample: Sample {
            left_thumb_x: normalize(axis(lx0, lx1)),
            left_thumb_y: normalize(axis(ly0, ly1)),
            right_thumb_x: normalize(axis(rx0, rx1)),
            right_thumb_y: normalize(axis(ry0, ry1)),
            left_trigger: normalize(trigger.max(0)) as u16,
            right_trigger: normalize((-trigger).max(0)) as u16,
        },
        button_event: buttons,
        dpad_event: dpad,
    })
}

// Math.round(axis / 32767 * 1000) rounds ties toward +infinity. The odd
// denominator prevents a half-integer tie for every integral source axis.
fn normalize(axis: i32) -> i16 {
    (axis * 1000 + 16383).div_euclid(32767) as i16
}

const WAIT_MICROSECONDS: u64 = 15_000;

/// The leading/trailing throttle in MW41243 uses maxWait equal to wait.
/// All times use the host event clock's microsecond unit.
#[derive(Default)]
pub struct Throttle {
    last_call: Option<u64>,
    last_invoke: u64,
    next_due: Option<u64>,
    pending: Option<Sample>,
}

impl Throttle {
    fn should_invoke(&self, now: u64) -> bool {
        self.last_call.is_none_or(|last| {
            now < last
                || now - last >= WAIT_MICROSECONDS
                || now.saturating_sub(self.last_invoke) >= WAIT_MICROSECONDS
        })
    }

    fn invoke(&mut self, now: u64) -> Option<Sample> {
        self.last_invoke = now;
        self.pending.take()
    }

    pub fn observe(&mut self, sample: Sample, now: u64) -> Option<Sample> {
        let now = now / 1000 * 1000;
        let invoke = self.should_invoke(now);
        self.pending = Some(sample);
        self.last_call = Some(now);
        if invoke {
            self.next_due = Some(now.saturating_add(WAIT_MICROSECONDS));
            return self.invoke(now);
        }
        if self.next_due.is_none() {
            self.next_due = Some(now.saturating_add(WAIT_MICROSECONDS));
        }
        None
    }

    pub fn poll(&mut self, now: u64) -> Option<Sample> {
        let now = now / 1000 * 1000;
        if !self.next_due.is_some_and(|deadline| deadline <= now) {
            return None;
        }
        if self.should_invoke(now) {
            self.next_due = None;
            return self.pending.is_some().then(|| self.invoke(now)).flatten();
        }
        let since_call = now.saturating_sub(self.last_call.unwrap());
        let since_invoke = now.saturating_sub(self.last_invoke);
        let remaining = (WAIT_MICROSECONDS - since_call).min(WAIT_MICROSECONDS - since_invoke);
        self.next_due = Some(now.saturating_add(remaining));
        None
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.next_due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(raw: u16) -> Sample {
        let [lo, hi] = raw.to_le_bytes();
        decode_report(&[0, lo, hi, 0, 128, 0, 128, 0, 128, 0, 128, 0, 0, 0, 0])
            .unwrap()
            .sample
    }

    #[test]
    fn keypad_report_uses_source_bias_endianness_trigger_split_and_thousandths() {
        let report = decode_report(&[
            0, 0, 0, 0, 128, 255, 255, 255, 127, 0, 192, 16, 28, 170, 187,
        ])
        .unwrap();
        assert_eq!(report.sample.left_thumb_x(), -1000);
        assert_eq!(report.sample.left_thumb_y(), 0);
        assert_eq!(report.sample.right_thumb_x(), 1000);
        assert_eq!(report.sample.right_thumb_y(), 0);
        assert_eq!(report.sample.left_trigger(), 500);
        assert_eq!(report.sample.right_trigger(), 0);
        assert_eq!((report.button_event, report.dpad_event), (16, 28));
        let reverse =
            decode_report(&[0, 255, 127, 1, 128, 0, 192, 0, 64, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(reverse.sample.left_thumb_x(), 0);
        assert_eq!(reverse.sample.left_thumb_y(), 0);
        assert_eq!(reverse.sample.right_thumb_x(), 500);
        assert_eq!(reverse.sample.right_thumb_y(), -500);
        assert_eq!(reverse.sample.left_trigger(), 0);
        assert_eq!(reverse.sample.right_trigger(), 1000);
    }

    #[test]
    fn keypad_exact_length_and_report_id_match_the_parser_branch() {
        for bytes in [&[][..], &[0; 14], &[0; 16], &[1; 15]] {
            assert!(decode_report(bytes).is_none());
        }
    }

    #[test]
    fn keypad_integer_normalization_matches_source_math_round_for_all_axis_words() {
        for raw in 0..=u16::MAX {
            let axis = i32::from(raw) - 32768;
            let expected = (f64::from(axis) / 32767.0 * 1000.0 + 0.5).floor() as i16;
            assert_eq!(normalize(axis), expected, "axis {axis}");
        }
        assert_eq!(normalize(32768), 1000);
    }

    #[test]
    fn keypad_throttle_delivers_leading_then_only_latest_trailing_sample() {
        let mut throttle = Throttle::default();
        assert_eq!(throttle.observe(sample(0), 0), Some(sample(0)));
        assert!(throttle.poll(15_000).is_none());
        assert!(throttle.next_deadline().is_none());
        assert_eq!(throttle.observe(sample(1), 20_000), Some(sample(1)));
        assert!(throttle.observe(sample(2), 22_000).is_none());
        assert!(throttle.observe(sample(65535), 30_000).is_none());
        assert!(throttle.poll(34_999).is_none());
        assert_eq!(throttle.poll(35_000), Some(sample(65535)));
        assert!(throttle.poll(90_000).is_none());
    }

    #[test]
    fn keypad_throttle_max_wait_and_following_burst_follow_debounce_state() {
        let mut throttle = Throttle::default();
        assert_eq!(throttle.observe(sample(0), 0), Some(sample(0)));
        assert!(throttle.observe(sample(32768), 10_000).is_none());
        assert_eq!(throttle.poll(15_000), Some(sample(32768)));
        assert!(throttle.observe(sample(65535), 16_000).is_none());
        assert_eq!(throttle.next_deadline(), Some(31_000));
        assert_eq!(throttle.observe(sample(0), 30_000), Some(sample(0)));
        assert_eq!(throttle.next_deadline(), Some(45_000));
        assert!(throttle.poll(45_000).is_none());
    }

    #[test]
    fn keypad_throttle_uses_source_integer_millisecond_clock() {
        let mut throttle = Throttle::default();
        assert_eq!(throttle.observe(sample(0), 999), Some(sample(0)));
        assert!(throttle.observe(sample(65535), 14_999).is_none());
        assert_eq!(throttle.next_deadline(), Some(15_000));
        assert_eq!(throttle.poll(15_001), Some(sample(65535)));
    }
}

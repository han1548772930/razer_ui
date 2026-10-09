//! Local layout edits never establish physical port, power or detection facts.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Segment {
    pub(super) id: u64,
    pub(super) value: u32,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PortDraft {
    pub(super) name: String,
    pub(super) is_strip_mode: bool,
    pub(super) strip: Vec<Segment>,
    pub(super) fan: Vec<Segment>,
    pub(super) dismissed: bool,
}
impl PortDraft {
    pub(super) fn segments(&self) -> &[Segment] {
        if self.is_strip_mode {
            &self.strip
        } else {
            &self.fan
        }
    }
    pub(super) fn segments_mut(&mut self) -> &mut Vec<Segment> {
        if self.is_strip_mode {
            &mut self.strip
        } else {
            &mut self.fan
        }
    }
    pub(super) fn total(&self) -> u32 {
        self.segments().iter().map(|segment| segment.value).sum()
    }
    pub(super) fn valid(&self, minimum: u32, fan_counts: &[u32]) -> bool {
        let mut ids = BTreeSet::new();
        !self.name.is_empty()
            && self.name.encode_utf16().count() <= 32
            && (1..=4).contains(&self.strip.len())
            && (1..=14).contains(&self.fan.len())
            && self
                .strip
                .iter()
                .all(|s| (minimum..=1000).contains(&s.value) && ids.insert(s.id))
            && self
                .fan
                .iter()
                .all(|s| fan_counts.contains(&s.value) && ids.insert(s.id))
    }
    pub(super) fn maximum(&self, id: u64, maximum: u32) -> u32 {
        // In a four-sided strip, the fourth side cannot exceed the second.
        if self.is_strip_mode && self.strip.len() == 4 && self.strip[3].id == id {
            maximum.min(self.strip[1].value)
        } else {
            maximum
        }
    }
    pub(super) fn set_leds(&mut self, id: u64, value: u32, minimum: u32, maximum: u32) {
        let value = value.clamp(minimum, self.maximum(id, maximum).max(minimum));
        if self.is_strip_mode && self.strip.len() == 4 && self.strip[1].id == id {
            let second = self.strip[1].value;
            if second >= value && second == self.strip[3].value {
                self.strip[3].value = value;
            }
        }
        if let Some(segment) = self
            .segments_mut()
            .iter_mut()
            .find(|segment| segment.id == id)
        {
            segment.value = value;
        }
        self.dismissed = false;
    }
    pub(super) fn add(&mut self, minimum: u32) {
        let id = self
            .strip
            .iter()
            .chain(&self.fan)
            .map(|s| s.id)
            .max()
            .unwrap_or(0)
            + 1;
        let total = self.total();
        let strip = self.is_strip_mode;
        let segments = self.segments_mut();
        if strip {
            let values = distribute(total, segments.len() + 1, minimum);
            for (segment, value) in segments.iter_mut().zip(&values) {
                segment.value = *value;
            }
            segments.push(Segment {
                id,
                value: *values.last().expect("nonempty distribution"),
            });
        } else {
            segments.push(Segment { id, value: 20 });
        }
        self.dismissed = false;
    }
}

// Exact recursive ceiling distribution from nH (3871) / jh (778). Adding
// bends may increase total when the minimum leaves too few LEDs to divide.
fn distribute(total: u32, count: usize, minimum: u32) -> Vec<u32> {
    if total < count as u32 {
        return vec![minimum; count];
    }
    if count == 1 {
        return vec![total];
    }
    let next = total.div_ceil(count as u32);
    let mut values = vec![next.max(minimum)];
    values.extend(distribute(total - next, count - 1, minimum));
    values
}

#[derive(Clone)]
pub(super) struct PortObservation {
    pub(super) id: u32,
    pub(super) active: bool,
    pub(super) detected_leds: u32,
    pub(super) maximum_leds: u32,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub(super) enum Status {
    #[default]
    Unavailable,
    Ready,
    Refreshing,
    NoDevices,
    NoPower,
    Protection,
    LedLimit,
}
impl Status {
    pub(super) fn ports_visible(self) -> bool {
        matches!(self, Self::Ready | Self::NoDevices | Self::LedLimit)
    }
}

//! Local layout drafts. Detection, power and connection observations are never serialized.
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Segment {
    pub(super) id: u32,
    pub(super) value: u32,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Port {
    pub(super) id: u32,
    pub(super) name: String,
    pub(super) strip_mode: bool,
    pub(super) strip: Vec<Segment>,
    pub(super) fan: Vec<Segment>,
    #[serde(skip)]
    pub(super) dismissed: bool,
}
impl Port {
    pub(super) fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            strip_mode: true,
            strip: vec![Segment { id: 0, value: 40 }],
            fan: vec![Segment { id: 0, value: 20 }],
            dismissed: false,
        }
    }
    pub(super) fn segments(&self) -> &[Segment] {
        if self.strip_mode {
            &self.strip
        } else {
            &self.fan
        }
    }
    pub(super) fn segments_mut(&mut self) -> &mut Vec<Segment> {
        if self.strip_mode {
            &mut self.strip
        } else {
            &mut self.fan
        }
    }
    pub(super) fn total(&self) -> u32 {
        self.segments().iter().map(|s| s.value).sum()
    }
    pub(super) fn set_leds(&mut self, segment_id: u32, value: u32, min: u32, max: u32) {
        let strip = self.strip_mode;
        let segments = self.segments_mut();
        let mut value = value.clamp(min, max.max(min));
        // Four-sided layout: the fourth side must not exceed the second side.
        if strip && segments.len() == 4 {
            if segments[3].id == segment_id {
                value = value.min(segments[1].value);
            }
            if segments[1].id == segment_id
                && segments[1].value == segments[3].value
                && value <= segments[1].value
            {
                segments[3].value = value;
            }
        }
        if let Some(segment) = segments.iter_mut().find(|s| s.id == segment_id) {
            segment.value = value;
        }
        self.dismissed = false;
    }
    pub(super) fn add(&mut self, minimum: u32, next_id: u32) {
        if !self.strip_mode {
            self.fan.push(Segment {
                id: next_id,
                value: 20,
            });
        } else {
            let total = self.total();
            let count = self.strip.len() + 1;
            let mut remaining = total;
            let mut values = Vec::new();
            for ix in 0..count {
                let value = remaining.div_ceil((count - ix) as u32);
                values.push(value.max(minimum));
                remaining = remaining.saturating_sub(value);
            }
            for (segment, value) in self.strip.iter_mut().zip(&values) {
                segment.value = *value;
            }
            self.strip.push(Segment {
                id: next_id,
                value: values[count - 1],
            });
        }
        self.dismissed = false;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Observation {
    Unavailable,
    Ready,
    Detecting,
    Empty,
    Standby,
    Mobile,
    Bluetooth,
    DcRequired,
    Protection,
    LedLimit,
}
impl Observation {
    pub(super) fn power_on(self) -> bool {
        self != Self::Standby
    }
    pub(super) fn ports_visible(self) -> bool {
        matches!(self, Self::Ready | Self::LedLimit)
    }
}

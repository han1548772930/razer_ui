//! Current MW hardware-event parsers 85275/15706/93804/33807/50462 and the 84910
//! button-event throttle. Input is the native mapping callback JSON buffer,
//! after its original device-specific conversion; it is never arbitrary HID.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[path = "controller_notification.rs"]
mod notification;
pub use notification::{BatteryStatus, Lighting, MappingInput, Notification};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    left_thumbstick_x: f64,
    left_thumbstick_y: f64,
    right_thumbstick_x: f64,
    right_thumbstick_y: f64,
    left_trigger: f64,
    right_trigger: f64,
    button_event: u16,
    dpad_event: u8,
}
impl Sample {
    pub fn triggers(&self) -> [f64; 2] {
        [self.left_trigger, self.right_trigger]
    }
    pub fn thumbsticks(&self) -> [f64; 4] {
        [
            self.left_thumbstick_x,
            self.left_thumbstick_y,
            self.right_thumbstick_x,
            self.right_thumbstick_y,
        ]
    }
    pub fn buttons(&self) -> u16 {
        self.button_event
    }
    pub fn dpad(&self) -> u8 {
        self.dpad_event
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Buttons {
        sample: Sample,
    },
    /// Preserve the complete source notification for its product reducer.
    Notification {
        event_id: u8,
        record_id: u8,
        buffer: Vec<u8>,
    },
    Unsupported {
        buffer: Vec<u8>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Protocol {
    Standard,
    EightK,
    Record05,
    Playstation,
    PlaystationArcade,
}

/// Mounted hardware-event capability, independent from a Triggers page.
/// Products2650 and4115 register input even though they mount no Triggers page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capability {
    product_id: u32,
    protocol: Protocol,
}

pub fn capability(product_id: u32) -> Option<Capability> {
    let protocol = match product_id {
        2629 | 2636 => Protocol::Standard,
        2647 | 2650 => Protocol::EightK,
        2676 | 2684 => Protocol::Record05,
        4133 | 4144 => Protocol::Playstation,
        4115 => Protocol::PlaystationArcade,
        _ => return None,
    };
    Some(Capability {
        product_id,
        protocol,
    })
}

impl Capability {
    pub fn accepts(self, physical_id: u32) -> bool {
        match self.product_id {
            2629 | 2636 | 2650 | 4115 => physical_id == self.product_id,
            2647 => matches!(physical_id, 2647 | 2649),
            2676 => matches!(physical_id, 2676 | 2677 | 2680 | 2681),
            2684 => matches!(physical_id, 2684 | 2685),
            4133 => matches!(physical_id, 4133 | 4135 | 4136),
            4144 => matches!(physical_id, 4144 | 4146 | 4147),
            _ => false,
        }
    }

    pub fn is_battery_supported(self) -> bool {
        matches!(self.product_id, 2636 | 2647 | 2676 | 4133 | 4144)
    }
}

pub struct Parser {
    protocol: Protocol,
    physical_product_id: u32,
    previous: Option<Vec<u8>>,
}
impl Parser {
    pub fn new(product_id: u32, physical_product_id: u32) -> anyhow::Result<Self> {
        let source = capability(product_id)
            .context("Controller hardware-event product has no current source capability")?;
        ensure!(
            source.accepts(physical_product_id),
            "Controller hardware-event product identity is not a current source variant"
        );
        Ok(Self {
            protocol: source.protocol,
            physical_product_id,
            previous: None,
        })
    }
    pub fn interval_ms(&self) -> Option<u32> {
        matches!(self.protocol, Protocol::EightK | Protocol::Record05).then_some(60)
    }
    pub fn parse(&mut self, buffer: &Value) -> anyhow::Result<Vec<Event>> {
        let input = buffer
            .as_array()
            .context("Native controller callback buffer is not an array")?;
        let mut events = Vec::new();
        if input.first().is_some_and(Value::is_array) {
            ensure!(
                input.len() <= 10001,
                "Native controller callback batch exceeds producer limit"
            );
            // 90372.onhardwareeventarray dispatches only these two parsers.
            ensure!(
                self.interval_ms().is_some(),
                "Unexpected batched callback for controller protocol"
            );
            for (index, frame) in input.iter().enumerate() {
                self.parse_buffer(
                    bytes(frame)?,
                    index == 0 || index + 1 == input.len(),
                    &mut events,
                );
            }
        } else {
            self.parse_buffer(
                bytes(buffer)?,
                self.protocol == Protocol::Record05,
                &mut events,
            );
        }
        Ok(events)
    }
    fn pc_mode(&self) -> bool {
        matches!(self.physical_product_id, 2677 | 2685 | 2681)
    }
    fn parse_buffer(&mut self, buffer: Vec<u8>, forced: bool, events: &mut Vec<Event>) {
        if self.protocol == Protocol::Record05 {
            let length = match buffer.first() {
                Some(0) => {
                    if self.pc_mode() {
                        16
                    } else {
                        15
                    }
                }
                Some(5) => 16,
                _ => 0,
            };
            if length > 0
                && buffer.len() > length
                && buffer.len().is_multiple_of(length)
                && buffer.chunks(length).all(|frame| frame[0] == buffer[0])
            {
                let count = buffer.len() / length;
                for (index, frame) in buffer.chunks(length).enumerate() {
                    self.parse_frame(frame, index == 0 || index + 1 == count, events);
                }
                return;
            }
        }
        self.parse_frame(&buffer, forced, events);
    }
    fn parse_frame(&mut self, buffer: &[u8], forced: bool, events: &mut Vec<Event>) {
        // Current50462 strips this one native prefix before selecting the report.
        // It must not remove a leading zero from an ordinary15-byte input frame.
        let buffer = if self.protocol == Protocol::PlaystationArcade
            && buffer.len() > 15
            && buffer[0] == 0
            && buffer[1] != 0
        {
            &buffer[1..]
        } else {
            buffer
        };
        let pc = self.protocol == Protocol::Record05 && self.pc_mode();
        if buffer.first() == Some(&0) && buffer.len() == if pc { 16 } else { 15 } {
            if matches!(self.protocol, Protocol::EightK | Protocol::Record05) {
                if !forced
                    && self.previous.as_ref().is_some_and(|previous| {
                        previous[11] == buffer[11] && (!pc || previous[12] == buffer[12])
                    })
                {
                    return;
                }
                self.previous = Some(buffer.to_vec());
            }
            let axis = |offset| {
                (i32::from(u16::from_le_bytes([buffer[offset], buffer[offset + 1]])) - 32768) as f64
                    / 32767.
            };
            let [left_trigger, right_trigger] = if pc {
                let value = buffer[10];
                if value >= 128 {
                    [(value - 128) as f64 / 127., 0.]
                } else {
                    [0., (127 - value) as f64 / 127.]
                }
            } else {
                let value = i32::from(u16::from_le_bytes([buffer[9], buffer[10]])) - 32768;
                [
                    if value < 0 { 0. } else { value as f64 / 32767. },
                    if value > 0 { 0. } else { value as f64 / -32767. },
                ]
            };
            let button_event =
                u16::from(buffer[11]) | if pc { u16::from(buffer[12] & 3) << 8 } else { 0 };
            let dpad_event = if pc {
                if (1..=8).contains(&buffer[13]) {
                    4 * buffer[13]
                } else {
                    0
                }
            } else {
                buffer[12]
            };
            events.push(Event::Buttons {
                sample: Sample {
                    left_thumbstick_x: axis(1),
                    left_thumbstick_y: axis(3),
                    right_thumbstick_x: axis(5),
                    right_thumbstick_y: axis(7),
                    left_trigger,
                    right_trigger,
                    button_event,
                    dpad_event,
                },
            });
        } else if self.protocol == Protocol::Record05
            && buffer.first() == Some(&5)
            && buffer.len() == 16
        {
            events.push(Event::Notification {
                event_id: buffer[1],
                record_id: 5,
                buffer: buffer.to_vec(),
            });
        } else if self.protocol != Protocol::Record05
            && buffer.first() != Some(&0)
            && if self.protocol == Protocol::Playstation {
                buffer.len() >= 64
            } else {
                buffer.len() == 64
            }
        {
            events.push(Event::Notification {
                event_id: buffer[if self.protocol == Protocol::PlaystationArcade {
                    8
                } else {
                    9
                }],
                record_id: buffer[0],
                buffer: buffer.to_vec(),
            });
        } else {
            events.push(Event::Unsupported {
                buffer: buffer.to_vec(),
            });
        }
    }
}
fn bytes(value: &Value) -> anyhow::Result<Vec<u8>> {
    let values = value
        .as_array()
        .context("Native controller report is not an array")?;
    ensure!(
        values.len() <= 160016,
        "Native controller report is oversized"
    );
    values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .context("Native controller report contains a non-byte value")
        })
        .collect()
}

/// Source sends immediately after33ms since last send; otherwise it replaces
/// a trailing33ms timer on every event. A clock value is supplied for pure tests.
#[derive(Default)]
pub struct Throttle {
    last_sent: Option<u64>,
    pending: Option<(u64, Sample)>,
}
impl Throttle {
    pub fn observe(&mut self, sample: Sample, now_ms: u64) -> Option<Sample> {
        self.pending = None;
        if self
            .last_sent
            .is_none_or(|last| now_ms.saturating_sub(last) >= 33)
        {
            self.last_sent = Some(now_ms);
            Some(sample)
        } else {
            self.pending = Some((now_ms.saturating_add(33), sample));
            None
        }
    }
    pub fn flush(&mut self, now_ms: u64) -> Option<Sample> {
        if self
            .pending
            .as_ref()
            .is_some_and(|(deadline, _)| now_ms >= *deadline)
        {
            self.last_sent = Some(now_ms);
            self.pending.take().map(|(_, sample)| sample)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, Parser, Throttle};
    use serde_json::json;
    fn frame(pc: bool, trigger: u16, buttons: u8) -> Vec<u8> {
        let mut frame = vec![0; if pc { 16 } else { 15 }];
        for offset in [1, 3, 5, 7] {
            frame[offset + 1] = 128;
        }
        if pc {
            frame[10] = trigger as u8;
        } else {
            frame[9..11].copy_from_slice(&trigger.to_le_bytes());
        }
        frame[11] = buttons;
        frame
    }
    fn samples(events: Vec<Event>) -> Vec<super::Sample> {
        events
            .into_iter()
            .filter_map(|event| match event {
                Event::Buttons { sample } => Some(sample),
                _ => None,
            })
            .collect()
    }
    #[test]
    fn actual_callback_formats_preserve_product_specific_trigger_and_button_values() {
        for (product, physical, pc) in [
            (2629, 2629, false),
            (2636, 2636, false),
            (2647, 2649, false),
            (2650, 2650, false),
            (2676, 2677, true),
            (2676, 2681, true),
            (2684, 2685, true),
            (4115, 4115, false),
            (4133, 4135, false),
            (4144, 4146, false),
        ] {
            let mut parser = Parser::new(product, physical).unwrap();
            let mut data = frame(pc, if pc { 255 } else { 65535 }, 16);
            if pc {
                data[12] = 3;
                data[13] = 8;
            }
            let sample = samples(parser.parse(&json!(data)).unwrap()).remove(0);
            assert_eq!(sample.triggers(), [1., 0.]);
            assert_eq!(sample.thumbsticks(), [0.; 4]);
            assert_eq!(sample.buttons(), if pc { 784 } else { 16 });
            assert_eq!(sample.dpad(), if pc { 32 } else { 0 });
        }
        assert!(Parser::new(2629, 2677).is_err());
    }
    #[test]
    fn batches_force_endpoints_and_record05_splits_only_consistent_frame_ids() {
        let mut parser = Parser::new(2647, 2647).unwrap();
        let frames = [
            frame(false, 32768, 0),
            frame(false, 40000, 0),
            frame(false, 65535, 0),
        ];
        let data = samples(parser.parse(&json!(frames)).unwrap());
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].triggers(), [0., -0.]);
        assert_eq!(data[1].triggers(), [1., 0.]);
        let mut parser = Parser::new(2676, 2677).unwrap();
        let joined = [
            frame(true, 128, 0),
            frame(true, 200, 0),
            frame(true, 255, 0),
        ]
        .concat();
        assert_eq!(samples(parser.parse(&json!(joined)).unwrap()).len(), 2);
        let mut broken = joined;
        broken[16] = 5;
        assert!(matches!(
            parser.parse(&json!(broken)).unwrap().as_slice(),
            [Event::Unsupported { .. }]
        ));
        assert!(parser.parse(&json!([0, 256])).is_err());
    }
    #[test]
    fn notifications_cannot_become_neutral_analog_samples() {
        let mut parser = Parser::new(2676, 2677).unwrap();
        let mut data = vec![0; 16];
        data[0] = 5;
        data[1] = 49;
        assert!(matches!(
            parser.parse(&json!(data)).unwrap().as_slice(),
            [Event::Notification { event_id: 49, .. }]
        ));
        assert!(samples(parser.parse(&json!(vec![0; 14])).unwrap()).is_empty());
    }
    #[test]
    fn throttle_keeps_leading_sample_and_replaces_source_trailing_timer() {
        let mut parser = Parser::new(2629, 2629).unwrap();
        let a = samples(parser.parse(&json!(frame(false, 32768, 0))).unwrap()).remove(0);
        let b = samples(parser.parse(&json!(frame(false, 65535, 0))).unwrap()).remove(0);
        let mut throttle = Throttle::default();
        assert_eq!(throttle.observe(a.clone(), 0), Some(a.clone()));
        assert!(throttle.observe(a, 1).is_none());
        assert!(throttle.observe(b.clone(), 2).is_none());
        assert!(throttle.flush(34).is_none());
        assert_eq!(throttle.flush(35), Some(b));
        assert!(throttle.flush(100).is_none());
    }
}

//! Validated native callbacks retained until the source postRecording consumer.
use super::{Decoder, Options, RecordingGeometry, StopCause, number, word};
use serde_json::Value;

#[derive(Clone)]
pub struct Callback {
    pub(super) received_at_ms: Option<u64>,
    pub(super) kind: CallbackKind,
    pub(super) source: Value,
}

#[derive(Clone)]
pub(super) enum CallbackKind {
    Started,
    Items { tick: u64, inputs: Vec<Input> },
}

#[derive(Clone)]
pub(super) struct Input {
    pub(super) kind: InputKind,
    pub(super) time: f64,
    pub(super) source: Value,
}

#[derive(Clone, Copy)]
pub(super) enum InputKind {
    Key { down: bool, key: NativeKey },
    Movement { x: f64, y: f64 },
    Mouse { action: MouseAction, data: i64 },
}

#[derive(Clone, Copy)]
pub(super) struct NativeKey {
    pub(super) virtual_key: u16,
    pub(super) scan_code: u16,
    pub(super) flags: u16,
}

#[derive(Clone, Copy)]
pub(super) enum MouseAction {
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
    MiddleDown,
    MiddleUp,
    ExtraDown,
    ExtraUp,
    Wheel,
    HorizontalWheel,
}

impl Input {
    fn parse(source: &Value) -> Result<Self, String> {
        let action = source
            .get("action")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                source
                    .get("action")
                    .and_then(Value::as_i64)
                    .map(|action| action.to_string())
            })
            .ok_or("录制事件缺少 action")?;
        let time = number(source, "time")?;
        let kind = match action.as_str() {
            "keydown" | "keyup" => InputKind::Key {
                down: action == "keydown",
                key: NativeKey {
                    virtual_key: word(source, "vkCode")?,
                    scan_code: word(source, "scanCode")?,
                    flags: word(source, "flags")?,
                },
            },
            "mousemove" => InputKind::Movement {
                x: number(source, "x")?,
                y: number(source, "y")?,
            },
            action => {
                let action = match action {
                    "lbuttondown" => MouseAction::LeftDown,
                    "lbuttonup" => MouseAction::LeftUp,
                    "rbuttondown" => MouseAction::RightDown,
                    "rbuttonup" => MouseAction::RightUp,
                    "mbuttondown" => MouseAction::MiddleDown,
                    "mbuttonup" => MouseAction::MiddleUp,
                    "523" => MouseAction::ExtraDown,
                    "524" => MouseAction::ExtraUp,
                    "mousewheel" => MouseAction::Wheel,
                    "mousehwheel" => MouseAction::HorizontalWheel,
                    _ => return Err(format!("不支持的原生录制事件：{action}")),
                };
                let data = match source.get("data") {
                    None => 0,
                    Some(value) => value.as_i64().ok_or("录制鼠标 data 不是整数")?,
                };
                InputKind::Mouse { action, data }
            }
        };
        Ok(Self {
            kind,
            time,
            source: source.clone(),
        })
    }

    pub(super) fn key(&self) -> Option<NativeKey> {
        match self.kind {
            InputKind::Key { key, .. } => Some(key),
            _ => None,
        }
    }

    pub(super) fn is_movement(&self) -> bool {
        matches!(self.kind, InputKind::Movement { .. })
    }
}

impl Callback {
    pub fn input_count(&self) -> usize {
        match &self.kind {
            CallbackKind::Started => 0,
            CallbackKind::Items { inputs, .. } => inputs.len(),
        }
    }

    pub fn parse(source: &Value) -> Result<Option<Self>, String> {
        let kind = match source.get("kind").and_then(Value::as_str) {
            Some("started") => CallbackKind::Started,
            Some("item") => {
                let raw = source
                    .get("event")
                    .and_then(Value::as_array)
                    .filter(|items| !items.is_empty())
                    .ok_or("录制事件数组为空或格式错误")?;
                let inputs = raw
                    .iter()
                    .map(Input::parse)
                    .collect::<Result<Vec<_>, _>>()?;
                if inputs[..inputs.len() - 1]
                    .iter()
                    .any(|input| !input.is_movement())
                {
                    return Err("录制事件批次包含非鼠标轨迹前缀".into());
                }
                let tick = source
                    .get("time_tick")
                    .and_then(Value::as_u64)
                    .ok_or("录制事件缺少 time_tick")?
                    .checked_add(u64::from(inputs.len() > 1))
                    .ok_or("录制事件 time_tick 溢出")?;
                CallbackKind::Items { tick, inputs }
            }
            _ => return Ok(None),
        };
        let received_at_ms = match source.get("received_at_ms") {
            None => None,
            Some(value) => Some(
                value
                    .as_u64()
                    .ok_or("录制回调 received_at_ms 不是无符号整数")?,
            ),
        };
        Ok(Some(Self {
            received_at_ms,
            kind,
            source: source.clone(),
        }))
    }

    pub(super) fn final_input(&self) -> Option<&Input> {
        match &self.kind {
            CallbackKind::Items { inputs, .. } => inputs.last(),
            _ => None,
        }
    }

    pub(super) fn only_final_input(&self) -> Self {
        let mut callback = self.clone();
        if let CallbackKind::Items { inputs, .. } = &mut callback.kind {
            let input = inputs.pop().expect("validated nonempty native callback");
            // This is the retained native file envelope, not the typed decoder state.
            callback.source["event"] = Value::Array(vec![input.source.clone()]);
            *inputs = vec![input];
        }
        callback
    }
}

#[derive(Default)]
pub struct Capture {
    callbacks: Vec<Callback>,
}

impl Capture {
    pub fn push(&mut self, callback: Callback) {
        self.callbacks.push(callback);
    }

    pub fn process(
        self,
        options: &Options,
        geometry: RecordingGeometry,
        stop: Option<StopCause>,
    ) -> Result<Vec<super::ActionItem>, String> {
        self.process_cancelable(options, Some(geometry), stop, || false)?
            .ok_or_else(|| "录制后处理已取消".into())
    }

    pub fn process_cancelable(
        self,
        options: &Options,
        geometry: Option<RecordingGeometry>,
        stop: Option<StopCause>,
        cancelled: impl Fn() -> bool,
    ) -> Result<Option<Vec<super::ActionItem>>, String> {
        let mut decoder = Decoder::new(options);
        if let Some(geometry) = geometry {
            decoder.set_geometry(geometry);
        }
        for callback in &self.callbacks {
            if cancelled() {
                return Ok(None);
            }
            decoder.push_callback(callback)?;
        }
        if cancelled() {
            return Ok(None);
        }
        let rows = decoder.finish(stop)?;
        Ok((!cancelled()).then_some(rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macro_library::{ActionKind, MacroType};
    use serde_json::json;

    fn options(kind: MacroType) -> Options {
        Options {
            macro_type: kind,
            phase: None,
            delay_mode: 3,
            fixed: 0.,
            random: [0., 0.],
            next_pair: 41,
            movement_mode: 0,
            shortcut: vec![],
            finalization_macro_type: kind,
            finalization_delay_mode: 3,
        }
    }

    fn geometry() -> RecordingGeometry {
        RecordingGeometry {
            monitors_before: json!([]),
            monitors_after: json!([]),
            screen_before: json!({}),
            screen_after: json!({}),
        }
    }

    fn capture() -> Capture {
        let mut capture = Capture::default();
        for event in [
            json!({"kind":"started","received_at_ms":100}),
            json!({"kind":"item","received_at_ms":110,"time_tick":1,"event":[
                {"action":"keydown","vkCode":65,"scanCode":30,"flags":0,"time":1000}]}),
            json!({"kind":"item","received_at_ms":120,"time_tick":2,"event":[
                {"action":"keyup","vkCode":65,"scanCode":30,"flags":128,"time":2000}]}),
        ] {
            capture.push(Callback::parse(&event).unwrap().unwrap());
        }
        capture
    }

    #[test]
    fn captured_callbacks_use_the_consumer_macro_type_phase_and_pair_base() {
        let rows = capture()
            .process(&options(MacroType::Sequence), geometry(), None)
            .unwrap();
        assert_eq!(rows.len(), 1);
        let key = rows[0].keyboard.as_ref().unwrap();
        assert_eq!(
            (key.pair_id, key.makecode, key.state),
            (Some(41), Some(65), None)
        );
        assert_eq!(rows[0].recorded_input.as_ref().unwrap()["time_tick"], 2);

        for phase in 0..3 {
            let mut current = options(MacroType::Phased);
            current.phase = Some(phase);
            let rows = capture().process(&current, geometry(), None).unwrap();
            assert_eq!(rows.len(), 2);
            assert!(rows.iter().all(|row| row.phase == Some(phase)));
            assert_eq!(
                rows[0].keyboard.as_ref().unwrap().pair_id,
                rows[1].keyboard.as_ref().unwrap().pair_id
            );
        }
    }

    #[test]
    fn postprocessing_reads_current_fixed_and_random_delay_without_replacing_raw_input() {
        let mut current = options(MacroType::Standard);
        current.delay_mode = 1;
        current.fixed = 0.25;
        let rows = capture().process(&current, geometry(), None).unwrap();
        assert_eq!(
            rows.iter()
                .filter(|row| row.kind == ActionKind::Delay)
                .map(|row| row.value.as_str())
                .collect::<Vec<_>>(),
            ["0.250", "0.250"]
        );
        assert_eq!(
            rows[1].recorded_input.as_ref().unwrap()["event"][0]["time"],
            1000
        );
        current.delay_mode = 2;
        current.random = [0.125, 0.75];
        let rows = capture().process(&current, geometry(), None).unwrap();
        assert!(
            rows.iter()
                .filter(|row| row.kind == ActionKind::Delay)
                .all(|row| {
                    row.state == "randomized"
                        && row.number_min == "0.125"
                        && row.number_max == "0.75"
                })
        );
    }

    #[test]
    fn cancellation_retains_no_partially_decoded_output() {
        let calls = std::cell::Cell::new(0);
        let rows = capture()
            .process_cancelable(
                &options(MacroType::Standard),
                Some(geometry()),
                None,
                || {
                    calls.set(calls.get() + 1);
                    calls.get() == 3
                },
            )
            .unwrap();
        assert!(rows.is_none());
    }

    #[test]
    fn callback_boundary_rejects_invalid_prefix_time_and_tick() {
        for value in [
            json!({"kind":"item","time_tick":1,"event":[]}),
            json!({"kind":"item","time_tick":1,"event":[{"action":"lbuttondown","time":1},{"action":"lbuttonup","time":1}]}),
            json!({"kind":"item","time_tick":u64::MAX,"event":[{"action":"mousemove","x":1,"y":2,"time":1},{"action":"lbuttonup","time":1}]}),
            json!({"kind":"item","time_tick":1,"event":[{"action":"lbuttondown"}]}),
        ] {
            assert!(Callback::parse(&value).is_err(), "{value}");
        }
    }

    #[test]
    fn preview_failure_does_not_consume_callbacks_needed_by_the_current_postprocessor() {
        let callback = Callback::parse(&json!({
            "kind":"item","time_tick":1,"received_at_ms":1,"event":[
                {"action":"keyup","vkCode":231,"scanCode":0,"flags":128,"time":1000}
            ]
        }))
        .unwrap()
        .unwrap();
        let mut preview = Decoder::for_preview(&options(MacroType::Standard));
        assert!(preview.push_callback(&callback).is_err());
        let mut captured = Capture::default();
        captured.push(callback);
        let rows = captured
            .process(&options(MacroType::Sequence), geometry(), None)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].keyboard.as_ref().unwrap().makecode, Some(231));
    }

    #[test]
    fn current_record_profile_controls_stop_filtering_and_first_delay_separately_from_rows() {
        let mut recorded = options(MacroType::Standard);
        recorded.delay_mode = 1;
        recorded.fixed = 0.25;
        recorded.finalization_delay_mode = 0;
        let rows = capture().process(&recorded, geometry(), None).unwrap();
        assert_eq!(rows[0].value, "0.010");
        assert_eq!(rows[2].value, "0.250");

        for (finalization, expected) in [(MacroType::Standard, 1), (MacroType::Sequence, 2)] {
            let mut recorded = options(MacroType::Standard);
            recorded.finalization_macro_type = finalization;
            let mut capture = Capture::default();
            for (tick, action) in [(1, "lbuttondown"), (2, "lbuttonup")] {
                capture.push(
                    Callback::parse(&json!({"kind":"item","time_tick":tick,
                    "event":[{"action":action,"time":1000}]}))
                    .unwrap()
                    .unwrap(),
                );
            }
            let rows = capture
                .process(&recorded, geometry(), Some(StopCause::Button))
                .unwrap();
            assert_eq!(rows.len(), expected);
        }
    }
}

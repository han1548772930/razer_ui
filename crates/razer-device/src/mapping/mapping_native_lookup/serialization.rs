use super::{Input, InputKind};
use serde_json::{Value, json};

impl Input {
    /// Current native virtual +40 notification serialization. Selection-only
    /// metadata and interrupts do not appear in this input object.
    pub fn to_native_json(&self) -> Value {
        let mut value = match &self.kind {
            InputKind::Keyboard { scancode, flag } => {
                json!({"type":"keyboard","scancode":scancode,"flag":flag,"extraInformation":0,"modifiers":self.modifiers as i32})
            }
            InputKind::AnalogKey { scancode, flag } => {
                json!({"type":"analogKey","scancode":scancode,"flag":flag,"modifiers":self.modifiers as i32})
            }
            InputKind::Mouse { id, data } => {
                json!({"type":"mouse","id":id,"data":data,"extraInformation":0,"modifiers":self.modifiers as i32})
            }
            InputKind::AnalogStick { axis, flag } => {
                json!({"type":"analogStick","axis":(["left","right","up","down","invalid"].get(usize::from(*axis)).copied().unwrap_or("invalid")),"flag":flag})
            }
            InputKind::External { key, flag } => {
                json!({"type":"external","key":*key as i32,"flag":*flag as i32,"modifiers":self.modifiers as i32})
            }
            InputKind::RazerKey { key, flag } => {
                json!({"type":"razerKey","key":*key as i32,"flag":*flag as i32,"modifiers":self.modifiers as i32})
            }
            InputKind::StreamController { key, flag } => {
                json!({"type":"streamController","key":*key as i32,"flag":*flag as i32})
            }
            InputKind::ControlPodButton { source, data } => {
                json!({"type":"controlPod","id":if *source {"sourceButton"} else {"topButton"},"data":data})
            }
            InputKind::ControlPodDial { clockwise } => {
                json!({"type":"controlPod","id":"dial","rotation":if *clockwise {"clockwise"} else {"anticlockwise"},"repeat":self.repeat})
            }
            InputKind::CoolingPad { id, byte1, byte2 } => {
                json!({"type":"coolingPad","id":id,"byte1":byte1,"byte2":byte2})
            }
            InputKind::RazerEvent(data) => {
                json!({"type":"razerEvent","events":data.iter().map(|v|*v as u8).collect::<Vec<_>>()})
            }
        };
        if matches!(
            self.kind,
            InputKind::Keyboard { .. } | InputKind::Mouse { .. }
        ) {
            if let Some(unit) = self.unit_id {
                value["unitId"] = unit.into();
            }
        }
        if matches!(self.kind, InputKind::Keyboard { .. }) {
            if let Some(delay) = self.delay_ms {
                value["delayInMs"] = (delay as i32).into();
            }
            if let Some(delay) = self.release_after_ms {
                value["releaseAfterMs"] = (delay as i32).into();
            }
        }
        if matches!(self.kind, InputKind::RazerKey { .. }) {
            if let Some(report) = self.report_id {
                value["reportId"] = report.into();
            }
        }
        value
    }

    pub fn to_native_string(&self) -> String {
        self.to_native_json().to_string()
    }

    /// Source virtual +64 identity, also the device-mode map ordering key.
    pub fn stateless_id(&self) -> String {
        let mut id = match &self.kind {
            InputKind::Keyboard { scancode, flag } => format!("t21s{scancode}f{}", flag & !1),
            InputKind::AnalogKey { scancode, flag } => format!("t2s{scancode}f{}", flag & !1),
            InputKind::Mouse { id, data } => {
                let packed = u32::from(*id) | (u32::from(*data) << 16);
                if matches!(id, 1024 | 2048) {
                    format!("t27i{id}d{data}")
                } else {
                    let mask = match packed {
                        1 | 2 => 3,
                        4 | 8 => 12,
                        16 | 32 => 48,
                        64 | 128 => 192,
                        256 | 512 => 768,
                        65536 | 131072 => 196608,
                        262144 | 524288 => 786432,
                        _ => 0,
                    };
                    format!("t27i{mask}")
                }
            }
            InputKind::AnalogStick { axis, .. } => format!("t3a{axis}"),
            InputKind::External { key, flag } => {
                format!("17:{}", key.wrapping_shl(16).wrapping_add(flag & !1))
            }
            InputKind::RazerKey { key, .. } => format!("t35k{key}"),
            InputKind::StreamController { key, .. } => format!("t36k{key}"),
            InputKind::ControlPodButton { source, data } => {
                format!("t9i{}d{data}", u8::from(!source))
            }
            InputKind::ControlPodDial { clockwise } => format!("t9i2r{}", u8::from(!clockwise)),
            InputKind::CoolingPad { id, byte1, byte2 } => format!("t10i{id}b{byte1}b{byte2}"),
            InputKind::RazerEvent(data) => format!(
                "t34{}",
                data.iter()
                    .map(|v| format!(":{}", *v as u8))
                    .collect::<String>()
            ),
        };
        if matches!(
            self.kind,
            InputKind::Keyboard { .. } | InputKind::Mouse { .. }
        ) {
            if let Some(unit) = self.unit_id {
                id.push_str(&format!("u{unit}"));
            }
        }
        id
    }
}

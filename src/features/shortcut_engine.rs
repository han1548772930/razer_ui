//! Source-backed native global-shortcut encoding; this module performs no I/O.
//! Frontend 2280: modules 42358 (inputs/outputs), 629 (keys), 74488 (launch),
//! 19019 (hash). Frontend 55/module 59007 adds native input variants before
//! submission. Frontend main: modules 5371 (stable JSON), 32937 (MD5).
use super::{
    shortcuts::{Shortcut, ShortcutOutput, validate_shortcuts},
    workspace::normalized_website,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const KEYS: &[(&str, u32, u32)] = include!("shortcut_engine_keys.rs");

/// Produces only the native `{mappings, hash}` object. Unsupported actions fail
/// the entire request, preserving the caller's existing engine configuration.
pub(crate) fn encode_shortcuts(shortcuts: &[Shortcut]) -> Result<Value, String> {
    validate_shortcuts(shortcuts)?;
    let mut mappings = Vec::with_capacity(shortcuts.len() * 2);
    let mut aliases = Vec::new();
    let mut inputs = BTreeSet::new();
    for shortcut in shortcuts {
        let (down_input, up_input) = encode_input(shortcut)?;
        if !inputs.insert(stable_json(&down_input)?) {
            return Err("快捷键使用了相同的原生输入，请修改重复的按键组合。".into());
        }
        let (down_output, up_output) = encode_output(shortcut.output())
            .map_err(|error| format!("快捷键 {}：{error}", shortcut.id()))?;
        // 59007 adds these mappings after the ordinary inputs. 7282/Oe
        // registers the corresponding DKM records through 42358/S_.
        let alias = match shortcut.input() {
            "KEY_BACKSPACE" => Some(251),
            "KEY_B" => Some(252),
            "KEY_SPACEBAR" => Some(253),
            _ => None,
        };
        if let Some(alias) = alias {
            for (flag, output) in [(0, &down_output), (1, &up_output)] {
                aliases.push(json!({
                    "input":{"type":"razerKey", "key":alias, "hypershift":shortcut.hypershift(),
                        "flag":flag, "modifiers":down_input["modifiers"]},
                    "output":output,
                }));
            }
        }
        mappings.push(json!({"input": down_input, "output": down_output}));
        mappings.push(json!({"input": up_input, "output": up_output}));
    }
    mappings.extend(aliases);
    let variants: Vec<_> = [(0, 4), (43981, 8)]
        .into_iter()
        .flat_map(|(unit, report)| {
            mappings.iter().cloned().map(move |mut mapping| {
                if mapping["input"]["type"] == "razerKey" {
                    mapping["input"]["reportId"] = report.into();
                } else {
                    mapping["input"]["unitId"] = unit.into();
                }
                mapping
            })
        })
        .collect();
    mappings.extend(variants);
    let mut engine = json!({"mappings": mappings});
    engine["hash"] = engine_hash(&engine)?.into();
    Ok(engine)
}

fn encode_input(shortcut: &Shortcut) -> Result<(Value, Value), String> {
    // Module 42358/Qe. New GPUI recordings use generic modifiers because its
    // modifier booleans do not expose the physical side. Legacy keys keep their
    // side-specific masks. These are not RegisterHotKey's Win32 flags.
    let mut modifiers = 0_u32;
    for modifier in shortcut.modifiers() {
        modifiers |= match modifier.as_str() {
            "ALT" => 1,
            "CTRL" => 2,
            "SHIFT" => 4,
            "GUI" => 8,
            "KEY_LEFT_ALT" => 256,
            "KEY_LEFT_CTRL" => 512,
            "KEY_LEFT_SHIFT" => 1024,
            "KEY_LEFT_GUI" => 2048,
            "KEY_RIGHT_ALT" => 16,
            "KEY_RIGHT_CTRL" => 32,
            "KEY_RIGHT_SHIFT" => 64,
            "KEY_RIGHT_GUI" => 128,
            _ => return Err(format!("尚未支持此原生辅助键：{modifier}")),
        };
    }
    let hypershift = shortcut.hypershift();
    let mouse = match shortcut.input() {
        "RightClick" => Some((4, 8)),
        "ScrollButton" => Some((16, 32)),
        "Button4" => Some((64, 128)),
        "Button5" => Some((256, 512)),
        _ => None,
    };
    if let Some((down, up)) = mouse {
        // rt() replaces the object with {down,up}; therefore data is undefined
        // and omitted from JSON for these mouse buttons. It must not become 0.
        return Ok((
            json!({"type":"mouse", "id":down, "hypershift":hypershift, "modifiers":modifiers}),
            json!({"type":"mouse", "id":up, "hypershift":hypershift, "modifiers":modifiers}),
        ));
    }
    let Some((_, scancode, flag)) = KEYS.iter().find(|(id, _, _)| *id == shortcut.input()) else {
        return Err(format!("缺少此按键的原生扫描码：{}", shortcut.input()));
    };
    Ok((
        json!({"type":"keyboard", "scancode":scancode, "hypershift":hypershift, "flag":flag, "modifiers":modifiers}),
        json!({"type":"keyboard", "scancode":scancode, "hypershift":hypershift, "flag":flag+1, "modifiers":modifiers}),
    ))
}

fn disabled() -> Value {
    json!({"type":"disabled"})
}
fn key(scancode: u32, flag: u32) -> Value {
    json!({"type":"keyboard", "scancode":scancode, "flag":flag})
}
fn delay(ms: u32) -> Value {
    json!({"type":"delay", "ms":ms})
}
fn multi(outputs: Vec<Value>) -> Value {
    json!({"type":"multi", "outputs":outputs})
}
fn tap(scancode: u32) -> Value {
    multi(vec![key(scancode, 2), delay(10), key(scancode, 3)])
}

fn encode_output(output: &ShortcutOutput) -> Result<(Value, Value), String> {
    match output {
        ShortcutOutput::Program { target } => {
            // Original non-exe launch uses cmd /c. Support its directly launched
            // exe branch only until that separate shell branch is implemented.
            if !std::path::Path::new(target).is_absolute()
                || !target.to_ascii_lowercase().ends_with(".exe")
                || target.contains('"')
                || target.chars().any(char::is_control)
            {
                return Err("原生引擎暂仅支持绝对路径的 .exe 程序。".into());
            }
            Ok((
                json!({"type":"launch", "path":target, "startHidden":false, "pathToCheck":target}),
                disabled(),
            ))
        }
        ShortcutOutput::Website { target } => {
            if target.chars().any(char::is_control) {
                return Err("网站地址包含控制字符。".into());
            }
            let target = normalized_website(target).ok_or("网站地址无效。")?;
            // The proven native branch embeds the URL in cmd /c start. Reject
            // quote, expansion and control syntax rather than execute it.
            if target.contains(['"', '%', '!', '^']) || target.chars().any(char::is_control) {
                return Err("此网址包含当前原生启动方式无法安全传递的字符。".into());
            }
            Ok((
                json!({"type":"launch", "path":format!("cmd /c start \"\" \"{target}\""), "startHidden":true}),
                disabled(),
            ))
        }
        ShortcutOutput::Text { text } => Ok((
            json!({"type":"clipboard", "id":"text", "text":text}),
            disabled(),
        )),
        ShortcutOutput::Multimedia { action } => encode_media(action),
        ShortcutOutput::Windows { action } => encode_windows(action),
    }
}

fn encode_media(action: &str) -> Result<(Value, Value), String> {
    let scan = match action {
        "VolumeDown" => Some(46),
        "VolumeUp" => Some(48),
        "PrevTrack" => Some(16),
        "NextTrack" => Some(25),
        _ => None,
    };
    if let Some(scan) = scan {
        return Ok((key(scan, 2), key(scan, 3)));
    }
    let down = match action {
        "MuteVolume" => tap(32),
        "Play" => tap(34),
        "MuteMic" => json!({"repeat":1, "type":"audio", "id":"mic", "mute":2}),
        "MuteAll" => json!({"repeat":1, "type":"audio", "id":"all", "mute":2}),
        "MicVolumeDown" => json!({"repeat":1, "type":"audio", "id":"micVolumeDown"}),
        "MicVolumeUp" => json!({"repeat":1, "type":"audio", "id":"micVolumeUp"}),
        _ => return Err(format!("尚未支持此原生多媒体操作：{action}")),
    };
    Ok((down, disabled()))
}

fn encode_windows(action: &str) -> Result<(Value, Value), String> {
    // These branches precede the source's default-Turbo lookup result.
    let down = match action {
        "SwitchApps" => {
            return Ok((
                multi(vec![key(56, 0), delay(10), key(15, 0)]),
                multi(vec![key(15, 1), delay(10), key(56, 1)]),
            ));
        }
        "DisplayBrightnessUp" | "DisplayBrightnessDown" => {
            return Ok((
                json!({"type":"display", "id":if action == "DisplayBrightnessUp" {"driverBrightnessUp"} else {"driverBrightnessDown"}}),
                json!({"type":"display", "id":"driverBrightnessStop"}),
            ));
        }
        "Mail" => tap(108),
        "ThisPC" => tap(107),
        "Refresh" => tap(103),
        // 42358.generateDefaultTurbos / 51480: these actions refer to separately
        // registered event collections by GUID. A mapping alone is incomplete.
        "PowerUserMenu" | "ShowDesktop" | "CycleApps" | "CloseApp" | "Cut" | "Copy" | "Paste"
        | "WindowsZoomIn" | "WindowsZoomOut" | "OfficeZoomIn" | "OfficeZoomOut" => {
            return Err(format!(
                "{action} 需要原引擎已注册的默认 Turbo，当前尚未接入。"
            ));
        }
        // Source fe selects a turbo for LockComputer in global-shortcut mode,
        // but generateDefaultTurbos does not provide its GUID. Never emit an
        // unresolved reference or substitute a different Win+L implementation.
        "LockComputer" => return Err("尚未取得锁定计算机操作所需的原生 Turbo 身份。".into()),
        _ => {
            let path = match action {
                "Calculator" => "calc",
                "MSPaint" => "mspaint",
                "Notepad" => "notepad",
                "Snipping_Tool" => "snippingtool",
                "LaunchTaskManager" => "cmd /c start taskmgr",
                "MSCopilot" => "cmd /c start ms-copilot:",
                "User_Directory" => "cmd /c start explorer %USERPROFILE%",
                "File_Explorer" => "explorer",
                _ => return Err(format!("尚未支持此原生 Windows 操作：{action}")),
            };
            let mut value = json!({"type":"launch", "path":path});
            if path.starts_with("cmd") {
                value["startHidden"] = true.into();
            }
            value
        }
    };
    Ok((down, disabled()))
}

// json-stable-stringify sorts object keys by JavaScript UTF-16 comparison;
// array order is significant. Do not depend on serde_json's preserve_order flag.
fn stable_json(value: &Value) -> Result<String, String> {
    match value {
        Value::Object(object) => {
            let mut keys: Vec<_> = object.keys().collect();
            keys.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
            let fields = keys
                .into_iter()
                .map(|key| {
                    Ok(format!(
                        "{}:{}",
                        serde_json::to_string(key).map_err(|error| error.to_string())?,
                        stable_json(&object[key])?
                    ))
                })
                .collect::<Result<Vec<String>, String>>()?;
            Ok(format!("{{{}}}", fields.join(",")))
        }
        Value::Array(values) => Ok(format!(
            "[{}]",
            values
                .iter()
                .map(stable_json)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        )),
        _ => serde_json::to_string(value).map_err(|error| error.to_string()),
    }
}

fn engine_hash(engine: &Value) -> Result<String, String> {
    let mut object = engine.clone();
    let fields = object.as_object_mut().ok_or("原生映射不是 JSON 对象。")?;
    fields.remove("hash");
    fields.remove("gamemode");
    let canonical = stable_json(&object)?.replace('<', "\\u003C");
    Ok(format!("{:x}", md5::compute(canonical.as_bytes())))
}

#[cfg(test)]
#[path = "shortcut_engine_tests.rs"]
mod tests;

//! Source-backed native global-shortcut encoding; this module performs no I/O.
//! Frontend 2280: modules 42358 (inputs/outputs), 629 (keys), 74488 (launch),
//! 19019 (hash). Frontend 55/module 59007 adds native input variants before
//! submission. Frontend main: modules 5371 (stable JSON), 32937 (MD5).
use super::{Shortcut, ShortcutOutput, validate_shortcuts};
use crate::macro_library::{EntryKind, MacroLibraryFile, MacroType};
use crate::mapping::normalized_website;
use razer_device::{
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::{canonical_json, source_hash},
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Produces only the native `{mappings, hash}` object. Unsupported actions fail
/// the entire request, preserving the caller's existing engine configuration.
pub fn encode_shortcuts(shortcuts: &[Shortcut]) -> Result<Value, String> {
    encode_with_context(shortcuts, &mut empty_context(), None)
}

fn empty_context() -> EngineContext {
    EngineContext {
        default_turbos: Some(Vec::new()),
        synapse_turbos: Some(Vec::new()),
        is_global_shortcuts: true,
        ..Default::default()
    }
}

pub(super) fn encode_with_context(
    shortcuts: &[Shortcut],
    context: &mut EngineContext,
    library: Option<&MacroLibraryFile>,
) -> Result<Value, String> {
    validate_shortcuts(shortcuts)?;
    let mut mappings = Vec::with_capacity(shortcuts.len() * 2);
    let mut aliases = Vec::new();
    let mut inputs = BTreeSet::new();
    for shortcut in shortcuts {
        let (down_input, up_input) = encode_input(shortcut)?;
        if !inputs.insert(canonical_json(&down_input).map_err(|error| error.to_string())?) {
            return Err("快捷键使用了相同的原生输入，请修改重复的按键组合。".into());
        }
        let (down_output, up_output) =
            encode_output_with_context(shortcut.output(), context, library)
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
    engine["hash"] = source_hash(&engine)
        .map_err(|error| error.to_string())?
        .into();
    Ok(engine)
}

fn encode_input(shortcut: &Shortcut) -> Result<(Value, Value), String> {
    MappingEngine::global_shortcuts()
        .encode_shortcut_input(
            shortcut.input(),
            shortcut.modifiers(),
            shortcut.hypershift(),
        )
        .map(|input| input.into_parts())
        .map_err(|error| format!("快捷键 {}：{error}", shortcut.id()))
}

fn disabled() -> Value {
    json!({"type":"disabled"})
}
#[cfg(test)]
fn encode_output(output: &ShortcutOutput) -> Result<(Value, Value), String> {
    encode_output_with_context(output, &mut empty_context(), None)
}

fn encode_output_with_context(
    output: &ShortcutOutput,
    context: &mut EngineContext,
    library: Option<&MacroLibraryFile>,
) -> Result<(Value, Value), String> {
    match output {
        ShortcutOutput::Program { target } => {
            if !std::path::Path::new(target).is_absolute()
                || target.contains('"')
                || target.chars().any(char::is_control)
            {
                return Err("请选择有效的绝对程序路径。".into());
            }
            if !target.to_ascii_lowercase().ends_with(".exe")
                && target.contains(['%', '!', '^', '&', '|', '<', '>'])
            {
                return Err("此文件路径包含原生命令启动方式无法安全传递的字符。".into());
            }
            source_outputs("launchGroup", json!({"path":target}), context)
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
        ShortcutOutput::Multimedia { action } => source_outputs(
            "multimediaGroup",
            json!({"multimediaAssignment":action}),
            context,
        ),
        ShortcutOutput::Windows { action } => encode_windows_with_context(action, context),
        ShortcutOutput::Macro {
            macro_id,
            playback,
            repeat_count,
            ..
        } => {
            let library = library.ok_or("快捷键编译未提供本地宏文档。")?;
            let entry = library
                .entries
                .iter()
                .find(|entry| entry.id == *macro_id && entry.kind == EntryKind::Macro)
                .ok_or("快捷键引用的本地宏已删除。")?;
            let choices = match entry.macro_type {
                MacroType::Standard => &super::source_shortcuts().macro_playback,
                MacroType::Sequence => &super::source_shortcuts().macro_sequence_playback,
                MacroType::Phased => &super::source_shortcuts().macro_phased_playback,
            };
            if !choices.iter().any(|choice| choice.id == *playback) {
                return Err("快捷键播放选项不适用于当前宏类型。".into());
            }
            let guid = entry
                .xml_guid
                .as_deref()
                .ok_or("快捷键引用的宏尚无持久身份。")?;
            source_outputs(
                "macroGroup",
                json!({"guid":guid,"macroPlaybackOption":playback,"repeatCount":repeat_count}),
                context,
            )
        }
    }
}

fn source_outputs(
    group: &str,
    value: Value,
    context: &mut EngineContext,
) -> Result<(Value, Value), String> {
    // A normal key selects the ordinary two-edge branch. The shared factory
    // owns output flags/timing; Dashboard inputs are decorated independently.
    let mut record = json!({"inputID":"KEY_A","inputType":"KeyInput","outputType":group});
    record[group] = value;
    let mappings = MappingEngine::global_shortcuts()
        .record(&record, context)
        .map_err(|e| e.to_string())?;
    if mappings.len() != 2 {
        return Err("Source shortcut output did not produce both input edges".into());
    }
    let output = |mapping: &Value| {
        mapping
            .get("output")
            .cloned()
            .ok_or_else(|| "Source shortcut mapping has no output".to_owned())
    };
    Ok((output(&mappings[0])?, output(&mappings[1])?))
}

#[cfg(test)]
fn encode_media(action: &str) -> Result<(Value, Value), String> {
    source_outputs(
        "multimediaGroup",
        json!({"multimediaAssignment":action}),
        &mut empty_context(),
    )
}

#[cfg(test)]
fn encode_windows(action: &str) -> Result<(Value, Value), String> {
    encode_windows_with_context(action, &mut empty_context())
}

fn encode_windows_with_context(
    action: &str,
    context: &mut EngineContext,
) -> Result<(Value, Value), String> {
    if matches!(
        action,
        "PowerUserMenu"
            | "ShowDesktop"
            | "CycleApps"
            | "CloseApp"
            | "Cut"
            | "Copy"
            | "Paste"
            | "WindowsZoomIn"
            | "WindowsZoomOut"
            | "OfficeZoomIn"
            | "OfficeZoomOut"
            | "LockComputer"
    ) && !context.default_turbos.as_ref().is_some_and(|turbos| {
        turbos.iter().any(|turbo| {
            turbo.get("name").and_then(Value::as_str) == Some(action)
                && turbo.get("guid").and_then(Value::as_str).is_some()
        })
    }) {
        return Err(format!("{action} 缺少原生默认 Turbo 身份。"));
    }
    source_outputs(
        "win8ShortcutsGroup",
        json!({"windowsShortcutAssignment":action}),
        context,
    )
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;

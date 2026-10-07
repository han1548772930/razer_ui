//! Current main Mn/hn v4 interchange. XML is data, never executable content.
use super::super::{ActionItem, ActionKind, Entry, EntryKind};
use crate::features::macro_library::{KeyboardEvent, MacroType, MouseEvent, MouseMovement};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|n| n.has_tag_name(name))
}
fn value(node: Node<'_, '_>, name: &str) -> String {
    // XML text/CDATA is validated against Mn's additional X pass below.
    child(node, name)
        .map(|field| {
            field
                .children()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .collect()
        })
        .unwrap_or_default()
}
fn source_html_decode(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}
fn required<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Result<Node<'a, 'input>, String> {
    child(node, name).ok_or_else(|| format!("宏 XML 缺少 {name}。"))
}
fn number(node: Node<'_, '_>, name: &str) -> Result<f64, String> {
    value(node, name)
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("宏 XML 的 {name} 数值无效。"))
}
fn integer(node: Node<'_, '_>, name: &str, max: u16) -> Result<Option<u16>, String> {
    let text = value(node, name);
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<u16>()
        .ok()
        .filter(|n| *n <= max)
        .map(Some)
        .ok_or_else(|| format!("宏 XML 的 {name} 整数无效。"))
}
fn mode(name: &str) -> Option<u8> {
    Some(match name {
        "" | "none" => 0,
        "absolute" => 1,
        "foreground" => 2,
        "relative" => 3,
        _ => return None,
    })
}
fn mode_name(mode: u8) -> &'static str {
    match mode {
        1 => "absolute",
        2 => "foreground",
        3 => "relative",
        _ => "none",
    }
}

pub(super) fn decode(text: &str) -> Result<Entry, String> {
    // roxmltree defaults to disallowing DTD; entities cannot read files/network.
    let doc = Document::parse(text).map_err(|e| format!("宏 XML 无效：{e}"))?;
    // Mn applies X recursively after XML parsing. For literal entity text,
    // that transformation loses content. Do not silently claim equivalent
    // import with different semantics or accept a lossy conversion.
    for node in doc
        .descendants()
        .filter(|n| n.is_element() && n.children().all(|c| !c.is_element()))
    {
        let raw: String = node
            .children()
            .filter(|n| n.is_text())
            .filter_map(|n| n.text())
            .collect();
        if source_html_decode(&raw) != raw {
            return Err(format!(
                "宏 XML 的 {} 包含字面 HTML 实体；当前原版导入会再次解码并改变文本。此兼容性尚未解决，文件未导入。",
                node.tag_name().name()
            ));
        }
    }
    let root = doc.root_element();
    if !root.has_tag_name("Macro") {
        return Err("文件不是 Macro XML。".into());
    }
    if value(root, "Version") != "4" {
        return Err(
            "此文件使用旧版宏格式；当前文件导入支持 Synapse 4 XML。原文件和宏列表未更改。".into(),
        );
    }
    let name = value(root, "Name");
    if name.trim().is_empty() {
        return Err("宏 XML 的名称为空。".into());
    }
    let delay = integer(root, "DelaySetting", 3)?.unwrap_or(0) as u8;
    let mut movement_mode =
        mode(&value(root, "MouseMoveType")).ok_or("宏 XML 的鼠标移动模式无效。")?;
    let events = required(root, "MacroEvents")?;
    let nodes = events
        .children()
        .filter(|node| node.has_tag_name("MacroEvent"))
        .collect::<Vec<_>>();
    if nodes
        .first()
        .is_none_or(|node| value(*node, "Type") != "actionBar")
        || nodes
            .iter()
            .skip(1)
            .any(|node| value(*node, "Type") == "actionBar")
    {
        return Err("宏 XML 缺少首个 actionBar 元数据或包含重复元数据，未导入。".into());
    }
    if nodes.len() > 100_000 {
        return Err("宏 XML 超出 100,000 个动作限制。".into());
    }
    for event in &nodes {
        if value(*event, "Type") == "actionBar" {
            if let Some(profile) = child(*event, "recordProfile") {
                if let Some(mode) = integer(profile, "mmtSetting", 3)?.filter(|n| *n > 0) {
                    movement_mode = mode as u8;
                }
            }
        }
    }
    let mut actions = Vec::new();
    for (index, event) in nodes.into_iter().enumerate() {
        let kind = match value(event, "Type").as_str() {
            "actionBar" => continue,
            "0" => ActionKind::Delay,
            "1" => ActionKind::Keyboard,
            "2" | "3" => ActionKind::Mouse,
            "4" => ActionKind::Text,
            "5" => ActionKind::Command,
            "6" => ActionKind::Loop,
            "7" => ActionKind::Macro,
            "10" => ActionKind::Launch,
            other => {
                return Err(format!(
                    "第 {} 个动作使用尚未支持的 XML 类型 {other}；未导入，避免丢失动作。",
                    index + 1
                ));
            }
        };
        let mut item = ActionItem::new(kind);
        // hn drops phase, so there is no verified lossless import contract.
        if child(event, "phase").is_some() {
            return Err(
                "当前原版 XML 导入会丢失动作阶段；阶段宏兼容性尚未完成，文件未导入。".into(),
            );
        }
        match kind {
            ActionKind::Delay => {
                let field = required(event, "Number")?;
                if child(field, "min").is_some() || child(field, "max").is_some() {
                    return Err("当前原版 XML 导入会把随机延迟范围转换为无效数值；此兼容性尚未完成，文件未导入。".into());
                } else {
                    let n = number(event, "Number")?;
                    if !(0. ..=99_999.999).contains(&n) {
                        return Err("延迟值超出有效范围。".into());
                    }
                    item.value = format!("{n:.3}");
                }
            }
            ActionKind::Keyboard => {
                let key = required(event, "KeyEvent")?;
                let state = integer(key, "State", 3)?.map(|n| n as u8);
                item.keyboard = Some(KeyboardEvent {
                    pair_id: None,
                    makecode: integer(key, "Makecode", u16::MAX)?,
                    state,
                    flag: state,
                    key_type: None,
                });
            }
            ActionKind::Mouse if value(event, "Type") == "3" => {
                if movement_mode == 0 {
                    return Err("鼠标轨迹缺少移动模式，未推测屏幕坐标。".into());
                }
                let mouse = required(event, "MouseEvent")?;
                let mut buffer = Vec::new();
                let mut elapsed = 0.;
                for point in mouse.children().filter(|n| n.has_tag_name("Buffer")) {
                    let time = number(point, "time")?;
                    if time < 0. {
                        return Err("鼠标轨迹时间不能为负。".into());
                    }
                    elapsed += time;
                    buffer.push(json!({"x":number(point,"x")?,"y":number(point,"y")?,"time":time}));
                }
                if buffer.is_empty() || !elapsed.is_finite() {
                    return Err("鼠标轨迹为空或时间溢出。".into());
                }
                item.value = format!("{:.3}", elapsed / 1000.);
                item.mouse_movement = Some(MouseMovement {
                    imported_xml: true,
                    mode: movement_mode,
                    buffer,
                    geometry: None,
                });
            }
            ActionKind::Mouse => {
                let mouse = required(event, "MouseEvent")?;
                item.mouse = Some(MouseEvent {
                    pair_id: None,
                    button: integer(mouse, "MouseButton", 9)?.map(|n| n as u8),
                    state: integer(mouse, "State", 1)?.map(|n| n as u8),
                });
            }
            ActionKind::Text => item.value = value(event, "Text"),
            ActionKind::Command => item.value = value(event, "TextCommand"),
            ActionKind::Loop => {
                let count = value(event, "Number")
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=99_999).contains(n))
                    .ok_or("循环次数无效。")?;
                item.value = count.to_string();
                let loop_event = required(event, "LoopEvent")?;
                item.state = if integer(loop_event, "State", 1)? == Some(1) {
                    "end"
                } else {
                    "start"
                }
                .into();
            }
            ActionKind::Macro => {
                let guid = value(event, "guid");
                item.xml_macro_guid = (!guid.is_empty()).then_some(guid);
            }
            ActionKind::Launch => {
                item.state = match integer(event, "RadioIndex", 1)? {
                    Some(0) => "program",
                    Some(1) => "website",
                    _ => "",
                }
                .into();
                item.value = value(event, "Content0");
                item.secondary_value = value(event, "Content1");
            }
        }
        actions.push(item);
    }
    // hn reconstructs keyboard/mouse pairs from row order. Keep that lookup,
    // including its literal zero/nonzero keyboard state behavior.
    let mut next_pair = 1;
    for index in 0..actions.len() {
        let kind = actions[index].kind;
        let down = match kind {
            ActionKind::Keyboard => actions[index]
                .keyboard
                .as_ref()
                .is_some_and(|k| k.state == Some(0)),
            ActionKind::Mouse => actions[index]
                .mouse
                .as_ref()
                .is_some_and(|m| m.state == Some(0)),
            _ => false,
        };
        if !down {
            continue;
        }
        let end = ((index + 1)..actions.len()).find(|&n| match kind {
            ActionKind::Keyboard => actions[n]
                .keyboard
                .as_ref()
                .is_some_and(|k| k.state.is_some_and(|s| s != 0)),
            ActionKind::Mouse => actions[n]
                .mouse
                .as_ref()
                .is_some_and(|m| m.state == Some(1)),
            _ => false,
        });
        for n in std::iter::once(index).chain(end) {
            if let Some(key) = &mut actions[n].keyboard {
                key.pair_id = Some(next_pair);
            }
            if let Some(mouse) = &mut actions[n].mouse {
                mouse.pair_id = Some(next_pair);
            }
        }
        next_pair += 1;
    }
    // hn sets Loop.Id=null and does not rebuild it. Do not infer pairs from
    // order or fabricate identities that the import converter never supplies.
    Ok(Entry {
        id: 1,
        name,
        kind: EntryKind::Macro,
        parent: None,
        open: false,
        actions,
        macro_type: MacroType::Standard,
        active_phase: None,
        record_delay: delay,
        xml_guid: None,
        xml_mouse_mode: movement_mode,
    })
}

fn finite(value: &str, field: &str) -> Result<Value, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .map(|_| Value::String(value.to_owned()))
        .ok_or_else(|| format!("无法导出：{field} 数值无效。"))
}

pub(super) fn encode(entry: &Entry, entries: &[Entry]) -> Result<String, String> {
    let modes = entry
        .actions
        .iter()
        .filter_map(|a| a.mouse_movement.as_ref().map(|m| m.mode))
        .collect::<std::collections::HashSet<_>>();
    if modes.len() > 1 {
        return Err("一个 XML 宏只能保存一种鼠标移动模式；当前草稿包含多个模式。".into());
    }
    let mode = modes.iter().next().copied().unwrap_or(entry.xml_mouse_mode);
    let mut events = vec![
        json!({"Type":"actionBar","recordProfile":if mode>0 { json!({"mmtSetting":mode}) } else { json!({}) },"selected":false}),
    ];
    let macros = entries
        .iter()
        .filter(|e| e.kind == EntryKind::Macro)
        .collect::<Vec<_>>();
    for item in &entry.actions {
        let mut event = match item.kind {
            ActionKind::Delay if item.state == "randomized" => {
                json!({"Type":0,"Number":{"min":finite(&item.number_min,"min")?,"max":finite(&item.number_max,"max")?},"selected":false})
            }
            ActionKind::Delay => {
                json!({"Type":0,"Number":finite(&item.value,"Number")?,"selected":false})
            }
            ActionKind::Keyboard => {
                let key = item.keyboard.as_ref();
                if key.is_none() && !item.value.is_empty() {
                    return Err("旧本地按键行缺少键码，请重新选择按键后导出。".into());
                }
                json!({"Type":1,"Id":key.and_then(|k|k.pair_id),"KeyEvent":{"Makecode":key.and_then(|k|k.makecode),"State":key.and_then(|k|k.state)},"flag":key.and_then(|k|k.flag),"selected":false,"isPairing":false})
            }
            ActionKind::Mouse if item.mouse_movement.is_some() => {
                let movement = item.mouse_movement.as_ref().unwrap();
                json!({"Type":3,"Id":null,"Number":finite(&item.value,"Number")?,"MouseEvent":{"Buffer":movement.buffer},"selected":false})
            }
            ActionKind::Mouse => {
                let mouse = item.mouse.as_ref();
                if mouse.is_none() && !item.value.is_empty() {
                    return Err("旧本地鼠标行缺少按键数据，请重新选择鼠标功能后导出。".into());
                }
                json!({"Type":2,"Id":mouse.and_then(|m|m.pair_id),"MouseEvent":{"MouseButton":mouse.and_then(|m|m.button),"State":mouse.and_then(|m|m.state)},"selected":false,"isPairing":false})
            }
            ActionKind::Text => json!({"Type":4,"Text":item.value,"selected":false}),
            ActionKind::Command => json!({"Type":5,"TextCommand":item.value,"selected":false}),
            ActionKind::Loop => {
                json!({"Type":6,"Id":item.loop_pair_id,"Number":finite(&item.value,"Number")?,"LoopEvent":{"State":u8::from(item.state=="end")},"selected":false,"isPairing":false})
            }
            ActionKind::Macro => {
                let target = macros.iter().position(|e| Some(e.id) == item.macro_id);
                let guid = target
                    .and_then(|i| macros[i].xml_guid.as_deref())
                    .or(item.xml_macro_guid.as_deref());
                if item.macro_id.is_some() && guid.is_none() {
                    return Err("嵌套宏目标已删除，请重新选择目标后导出。".into());
                }
                json!({"Type":7,"MPIndex":target,"guid":guid,"selected":false})
            }
            ActionKind::Launch => {
                json!({"Type":10,"RadioIndex":match item.state.as_str(){"program"=>Some(0),"website"=>Some(1),_=>None},"Content0":item.value,"Content1":item.secondary_value,"selected":false})
            }
        };
        if let Some(phase) = item.phase {
            event["phase"] = json!(phase);
        }
        events.push(event);
    }
    let macro_doc = json!({"Name":entry.name,"Guid":entry.xml_guid,"Version":4,"DelaySetting":entry.record_delay,"MouseMoveType":mode_name(mode),"MacroEvents":{"MacroEvent":events}});
    let mut out = String::new();
    element(&mut out, "Macro", &macro_doc, 0)?;
    if out.len() > super::MAX_BYTES {
        return Err("宏 XML 超出 16 MiB 导出限制。".into());
    }
    Ok(out)
}

fn escape(text: &str) -> Result<String, String> {
    if source_html_decode(text) != text {
        return Err("文本包含字面 HTML 实体；当前原版 XML 导入会再次解码并改变文本。尚不能保真交换，文件未导出。".into());
    }
    if text.chars().any(|c| {
        !matches!(c, '\t' | '\n' | '\r') && (c < ' ' || matches!(c, '\u{fffe}' | '\u{ffff}'))
    }) {
        return Err("宏文本包含 XML 1.0 不支持的字符；文件未导出。".into());
    }
    Ok(text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;"))
}
fn element(out: &mut String, name: &str, value: &Value, depth: usize) -> Result<(), String> {
    if let Value::Array(items) = value {
        for item in items {
            element(out, name, item, depth)?;
        }
        return Ok(());
    }
    let pad = "   ".repeat(depth);
    out.push_str(&format!("{pad}<{name}"));
    match value {
        Value::Null => out.push_str("/>\n"),
        Value::Object(fields) if fields.is_empty() => out.push_str("/>\n"),
        Value::Object(fields) => {
            out.push_str(">\n");
            for (field, value) in fields {
                element(out, field, value, depth + 1)?;
            }
            out.push_str(&format!("{pad}</{name}>\n"));
        }
        _ => {
            let text = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            out.push('>');
            out.push_str(&escape(&text)?);
            out.push_str(&format!("</{name}>\n"));
        }
    }
    Ok(())
}

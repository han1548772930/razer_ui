//! Local shortcut documents, source choices and strict registration/storage validation.
use crate::mapping::{MEDIA, WINDOWS, canonical_key, key_label, normalized_website};
use razer_catalog::emoji::EmojiSearchTable;
use razer_i18n as i18n;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
pub const MODIFIERS: &[(&str, &str)] = &[
    ("CTRL", "Ctrl"),
    ("ALT", "Alt"),
    ("SHIFT", "Shift"),
    ("GUI", "Win"),
];
#[derive(Deserialize)]
#[non_exhaustive]
pub struct SourceShortcutChoice {
    pub id: String,
    pub content: String,
}
#[non_exhaustive]
pub struct SourceShortcutData {
    pub functions: Vec<String>,
    pub key_names: std::collections::BTreeMap<String, String>,
    pub media: Vec<SourceShortcutChoice>,
    pub windows: Vec<SourceShortcutChoice>,
    pub macro_playback: Vec<SourceShortcutChoice>,
    pub macro_sequence_playback: Vec<SourceShortcutChoice>,
    pub macro_phased_playback: Vec<SourceShortcutChoice>,
    pub profile_actions: Vec<SourceShortcutChoice>,
    pub sensitivity_actions: Vec<SourceShortcutChoice>,
    pub excluded_keys: Vec<String>,
    pub emoji: SourceEmojiData,
}
#[non_exhaustive]
pub struct SourceEmojiData {
    pub groups: Vec<SourceEmojiGroup>,
    pub tabs: Vec<SourceEmojiTab>,
    pub search: &'static EmojiSearchTable,
    pub variants: std::collections::BTreeMap<String, Vec<String>>,
    pub excluded_without_windows_10: Vec<String>,
}
impl SourceEmojiData {
    fn from_catalog(id: razer_catalog::emoji::DatasetId) -> Self {
        let source = razer_catalog::emoji::dataset(id);
        Self {
            groups: source
                .groups()
                .iter()
                .map(|group| SourceEmojiGroup {
                    values: group.values().to_vec(),
                })
                .collect(),
            tabs: source
                .tabs()
                .iter()
                .map(|tab| SourceEmojiTab {
                    emo: tab.emoji().to_owned(),
                    name: tab.name().to_owned(),
                })
                .collect(),
            search: razer_catalog::emoji::search(),
            variants: source.variants().clone(),
            excluded_without_windows_10: source.exclusions().to_vec(),
        }
    }
}
#[derive(Deserialize)]
#[non_exhaustive]
pub struct SourceEmojiGroup {
    pub values: Vec<String>,
}
#[derive(Deserialize)]
#[non_exhaustive]
pub struct SourceEmojiTab {
    pub emo: String,
    pub name: String,
}
pub fn source_shortcuts() -> &'static SourceShortcutData {
    static DATA: OnceLock<SourceShortcutData> = OnceLock::new();
    DATA.get_or_init(|| read_shortcuts_current_catalog())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Shortcut {
    pub id: String,
    pub input: String,
    pub modifiers: Vec<String>,
    pub hypershift: bool,
    pub output: ShortcutOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShortcutOutput {
    Program {
        target: String,
    },
    Website {
        target: String,
    },
    Multimedia {
        action: String,
    },
    Windows {
        action: String,
    },
    Text {
        text: String,
    },
    /// Persistent identity in this workspace's local Macro library. This is
    /// deliberately not a native Synapse GUID or an executable engine output.
    Macro {
        macro_id: u64,
        name: String,
        playback: String,
        repeat_count: u8,
    },
}

impl ShortcutOutput {
    pub fn category(&self) -> &'static str {
        match self {
            Self::Program { .. } | Self::Website { .. } => "LAUNCH_PROGRAM",
            Self::Multimedia { .. } => "MULTIMEDIA",
            Self::Windows { .. } => "WINDOWS_SHORTCUT",
            Self::Text { .. } => "TEXT_FUNCTION",
            Self::Macro { .. } => "MACRO",
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Program { .. } => "program",
            Self::Website { .. } => "website",
            Self::Multimedia { .. } => "multimedia",
            Self::Windows { .. } => "windows",
            Self::Text { .. } => "text",
            Self::Macro { .. } => "macro",
        }
    }
    pub fn value(&self) -> &str {
        match self {
            Self::Program { target } | Self::Website { target } => target,
            Self::Multimedia { action } | Self::Windows { action } => action,
            Self::Text { text } => text,
            Self::Macro { name, .. } => name,
        }
    }
    pub fn from_kind(kind: &str) -> Self {
        match kind {
            "website" => Self::Website {
                target: String::new(),
            },
            "multimedia" => Self::Multimedia {
                action: MEDIA[0].0.into(),
            },
            "windows" => Self::Windows {
                action: WINDOWS[0].0.into(),
            },
            "text" => Self::Text {
                text: String::new(),
            },
            "macro" => Self::Macro {
                macro_id: 0,
                name: String::new(),
                playback: "Once".into(),
                repeat_count: 2,
            },
            _ => Self::Program {
                target: String::new(),
            },
        }
    }
    pub fn label(&self) -> String {
        let table = match self {
            Self::Multimedia { .. } => MEDIA,
            Self::Windows { .. } => WINDOWS,
            _ => &[],
        };
        table
            .iter()
            .find(|(id, _)| *id == self.value())
            .map(|(_, key)| i18n::t(key))
            .unwrap_or_else(|| self.value().into())
    }
    pub fn error(&self) -> Option<&'static str> {
        match self {
            Self::Program { target } if target.trim().is_empty() => Some("请选择程序文件。"),
            Self::Website { target } if normalized_website(target).is_none() => {
                Some("请输入有效的网站地址。")
            }
            Self::Text { text } if text.is_empty() || text.encode_utf16().count() > 250 => {
                Some("请输入 1–250 个字符的文本。")
            }
            Self::Multimedia { action } if !MEDIA.iter().any(|(id, _)| id == action) => {
                Some("请选择有效的多媒体操作。")
            }
            Self::Windows { action } if !WINDOWS.iter().any(|(id, _)| id == action) => {
                Some("请选择有效的 Windows 操作。")
            }
            Self::Macro {
                macro_id,
                name,
                playback,
                repeat_count,
            } if *macro_id == 0
                || name.trim().is_empty()
                || playback == "NTimes" && !(1..=99).contains(repeat_count)
                || !source_shortcuts()
                    .macro_playback
                    .iter()
                    .chain(&source_shortcuts().macro_sequence_playback)
                    .chain(&source_shortcuts().macro_phased_playback)
                    .any(|item| item.id == *playback) =>
            {
                Some("宏引用或播放选项无效。")
            }
            _ => None,
        }
    }
}

// Accessors for the retained pure native encoder (no transport is connected).
#[allow(dead_code)]
impl Shortcut {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn modifiers(&self) -> &[String] {
        &self.modifiers
    }
    pub fn hypershift(&self) -> bool {
        self.hypershift
    }
    pub fn output(&self) -> &ShortcutOutput {
        &self.output
    }
}
impl Shortcut {
    pub fn chord(&self) -> String {
        self.modifiers
            .iter()
            .map(|key| modifier_label(key))
            .chain(self.hypershift.then(|| "Hypershift".into()))
            .chain((!self.input.is_empty()).then(|| input_label(&self.input)))
            .collect::<Vec<_>>()
            .join(" + ")
    }
    pub fn same_chord(&self, other: &Self) -> bool {
        self.input == other.input
            && self.modifiers == other.modifiers
            && self.hypershift == other.hypershift
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("快捷键缺少身份。".into());
        }
        if self.input.is_empty()
            || !(canonical_key(&self.input).is_some()
                || MOUSE_INPUTS.iter().any(|(id, _)| *id == self.input))
            || self.input.contains("SHIFT")
            || self.input.ends_with("CTRL")
            || self.input.ends_with("ALT")
            || self.input.ends_with("GUI")
        {
            return Err("请录制一个按键或选择鼠标输入。".into());
        }
        let families = self
            .modifiers
            .iter()
            .map(|key| modifier_family(key))
            .collect::<Option<Vec<_>>>()
            .ok_or("辅助键无效。")?;
        if families.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("辅助键无效或重复。".into());
        }
        if let Some(error) = self.output.error() {
            return Err(error.into());
        }
        Ok(())
    }
}

pub fn modifier_family(key: &str) -> Option<usize> {
    match key {
        "CTRL" | "KEY_LEFT_CTRL" | "KEY_RIGHT_CTRL" => Some(0),
        "ALT" | "KEY_LEFT_ALT" | "KEY_RIGHT_ALT" => Some(1),
        "SHIFT" | "KEY_LEFT_SHIFT" | "KEY_RIGHT_SHIFT" => Some(2),
        "GUI" | "KEY_LEFT_GUI" | "KEY_RIGHT_GUI" => Some(3),
        _ => None,
    }
}

pub fn validate_shortcuts(items: &[Shortcut]) -> Result<(), String> {
    for (ix, item) in items.iter().enumerate() {
        item.validate()?;
        if items[..ix]
            .iter()
            .any(|prior| prior.id == item.id || prior.same_chord(item))
        {
            return Err("快捷键身份或按键组合重复。".into());
        }
    }
    Ok(())
}

pub fn validate_stored_shortcuts(items: &[Shortcut]) -> Result<(), String> {
    for (index, item) in items.iter().enumerate() {
        if item.id.is_empty() || items[..index].iter().any(|other| other.id == item.id) {
            return Err("快捷键身份无效或重复。".into());
        }
        if item.input.is_empty() {
            if !item.modifiers.is_empty() {
                return Err("尚未录制按键的快捷键不能包含辅助键。".into());
            }
            if let Some(error) = item.output.error() {
                return Err(error.into());
            }
        } else {
            item.validate()?;
        }
    }
    Ok(())
}

pub const MOUSE_INPUTS: &[(&str, &str)] = &[
    ("ScrollButton", "SCROLL_CLICK"),
    ("RightClick", "RIGHT_CLICK"),
    ("Button4", "MOUSE_BUTTON_4"),
    ("Button5", "MOUSE_BUTTON_5"),
];

impl Shortcut {
    pub fn new(
        id: String,
        input: String,
        modifiers: Vec<String>,
        hypershift: bool,
        output: ShortcutOutput,
    ) -> Self {
        Self {
            id,
            input,
            modifiers,
            hypershift,
            output,
        }
    }
}
// Generated catalog reader: razer-pages/shortcuts_current.
fn read_shortcuts_current_catalog() -> SourceShortcutData {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SourceShortcutData> {
        let value = match r.tag()? {
            0 => SourceShortcutData {
                functions: r.child(decode_1)?,
                key_names: r.child(decode_3)?,
                media: r.child(decode_4)?,
                windows: r.child(decode_4)?,
                macro_playback: r.child(decode_4)?,
                macro_sequence_playback: r.child(decode_4)?,
                macro_phased_playback: r.child(decode_4)?,
                profile_actions: r.child(decode_4)?,
                sensitivity_actions: r.child(decode_4)?,
                excluded_keys: r.child(decode_1)?,
                emoji: r.child(decode_6)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_2, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SourceShortcutChoice>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SourceShortcutChoice> {
        let value = match r.tag()? {
            0 => SourceShortcutChoice {
                id: r.child(decode_2)?,
                content: r.child(decode_2)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SourceEmojiData> {
        let value = match r.tag()? {
            0 => SourceEmojiData::from_catalog(razer_catalog::emoji::DatasetId::Mapping),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_SHORTCUTS_CURRENT,
        0,
        "540f9e8fbf9cd9f3da2b4b36b61cbfa65a439a1d90404068b61e4fa79060b84a",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod shortcuts_current_catalog_tests {
    #[test]
    fn packaged_catalog_shortcuts_current_decodes() {
        let _ = super::read_shortcuts_current_catalog();
    }
}
// End generated catalog reader: razer-pages/shortcuts_current.

fn modifier_label(key: &str) -> String {
    let Some(family) = modifier_family(key) else {
        return key_label(key);
    };
    // 46114/i names left modifiers Ctrl/Alt/Shift without an invented prefix.
    // GPUI's side-independent aliases use that source left-key spelling.
    let source_key = if key.starts_with("KEY_") {
        key.to_owned()
    } else {
        format!("KEY_LEFT_{}", MODIFIERS[family].0)
    };
    source_shortcuts()
        .key_names
        .get(&source_key)
        .cloned()
        .unwrap_or_else(|| key_label(key))
}

fn input_label(input: &str) -> String {
    MOUSE_INPUTS
        .iter()
        .find(|(id, _)| *id == input)
        .map(|(_, label)| i18n::t(label))
        .or_else(|| source_shortcuts().key_names.get(input).cloned())
        .unwrap_or_else(|| key_label(input))
}

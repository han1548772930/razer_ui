//! Current 4608/94608: Oe → Te → be rows beside ie's shared mapping tree.
//! Runtime registration is separate from storing a user's configuration.
use super::{
    controls::Choice,
    workspace::{MEDIA, WINDOWS, canonical_key, key_label, normalized_website},
};
use crate::ui::source_alert::{AlertAction, AlertPlacement, SourceAlert};
use crate::{i18n, ui::surface};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::{Deserialize, Serialize};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct SourceShortcutChoice {
    id: String,
    content: String,
}
#[derive(Deserialize)]
struct SourceShortcutData {
    functions: Vec<String>,
    key_names: std::collections::BTreeMap<String, String>,
    media: Vec<SourceShortcutChoice>,
    windows: Vec<SourceShortcutChoice>,
    macro_playback: Vec<SourceShortcutChoice>,
    macro_sequence_playback: Vec<SourceShortcutChoice>,
    macro_phased_playback: Vec<SourceShortcutChoice>,
    profile_actions: Vec<SourceShortcutChoice>,
    sensitivity_actions: Vec<SourceShortcutChoice>,
    excluded_keys: Vec<String>,
    emoji: SourceEmojiData,
}
#[derive(Deserialize)]
struct SourceEmojiData {
    groups: Vec<SourceEmojiGroup>,
    tabs: Vec<SourceEmojiTab>,
    search: Vec<SourceEmojiSearch>,
    variants: std::collections::BTreeMap<String, Vec<String>>,
    excluded_without_windows_10: Vec<String>,
}
#[derive(Deserialize)]
struct SourceEmojiGroup {
    values: Vec<String>,
}
#[derive(Deserialize)]
struct SourceEmojiTab {
    emo: String,
    name: String,
}
#[derive(Deserialize)]
struct SourceEmojiSearch {
    emo: String,
    name: String,
}
fn source_shortcuts() -> &'static SourceShortcutData {
    static DATA: OnceLock<SourceShortcutData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("shortcuts_current.json"))
            .expect("current shortcuts literals")
    })
}

/// HTML textarea maxlength counts UTF-16 units. Trim only the inserted span,
/// keeping the existing suffix when a paste or IME replacement exceeds 250.
pub(crate) fn limit_shortcut_text(
    previous: &str,
    current: &str,
) -> Option<(std::ops::Range<usize>, String)> {
    if current.encode_utf16().count() <= 250 {
        return None;
    }
    let prefix = previous
        .chars()
        .zip(current.chars())
        .take_while(|(a, b)| a == b)
        .map(|(_, ch)| ch.len_utf8())
        .sum::<usize>();
    let suffix = previous[prefix..]
        .chars()
        .rev()
        .zip(current[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(_, ch)| ch.len_utf8())
        .sum::<usize>();
    let end = current.len() - suffix;
    let mut remaining = 250_usize.saturating_sub(
        current[..prefix].encode_utf16().count() + current[end..].encode_utf16().count(),
    );
    let mut accepted = prefix;
    for ch in current[prefix..end].chars() {
        if ch.len_utf16() > remaining {
            break;
        }
        remaining -= ch.len_utf16();
        accepted += ch.len_utf8();
    }
    Some((
        accepted..end,
        format!("{}{}", &current[..accepted], &current[end..]),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Shortcut {
    id: String,
    input: String,
    modifiers: Vec<String>,
    hypershift: bool,
    output: ShortcutOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ShortcutOutput {
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
    fn category(&self) -> &'static str {
        match self {
            Self::Program { .. } | Self::Website { .. } => "LAUNCH_PROGRAM",
            Self::Multimedia { .. } => "MULTIMEDIA",
            Self::Windows { .. } => "WINDOWS_SHORTCUT",
            Self::Text { .. } => "TEXT_FUNCTION",
            Self::Macro { .. } => "MACRO",
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::Program { .. } => "program",
            Self::Website { .. } => "website",
            Self::Multimedia { .. } => "multimedia",
            Self::Windows { .. } => "windows",
            Self::Text { .. } => "text",
            Self::Macro { .. } => "macro",
        }
    }
    fn value(&self) -> &str {
        match self {
            Self::Program { target } | Self::Website { target } => target,
            Self::Multimedia { action } | Self::Windows { action } => action,
            Self::Text { text } => text,
            Self::Macro { name, .. } => name,
        }
    }
    fn from_kind(kind: &str) -> Self {
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
    fn label(&self) -> String {
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
    fn error(&self) -> Option<&'static str> {
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
    pub(crate) fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn input(&self) -> &str {
        &self.input
    }
    pub(crate) fn modifiers(&self) -> &[String] {
        &self.modifiers
    }
    pub(crate) fn hypershift(&self) -> bool {
        self.hypershift
    }
    pub(crate) fn output(&self) -> &ShortcutOutput {
        &self.output
    }
}
impl Shortcut {
    fn chord(&self) -> String {
        self.modifiers
            .iter()
            .map(|key| modifier_label(key))
            .chain(self.hypershift.then(|| "Hypershift".into()))
            .chain((!self.input.is_empty()).then(|| input_label(&self.input)))
            .collect::<Vec<_>>()
            .join(" + ")
    }
    fn same_chord(&self, other: &Self) -> bool {
        self.input == other.input
            && self.modifiers == other.modifiers
            && self.hypershift == other.hypershift
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
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

const MODIFIERS: &[(&str, &str)] = &[
    ("CTRL", "Ctrl"),
    ("ALT", "Alt"),
    ("SHIFT", "Shift"),
    ("GUI", "Win"),
];
fn modifier_family(key: &str) -> Option<usize> {
    match key {
        "CTRL" | "KEY_LEFT_CTRL" | "KEY_RIGHT_CTRL" => Some(0),
        "ALT" | "KEY_LEFT_ALT" | "KEY_RIGHT_ALT" => Some(1),
        "SHIFT" | "KEY_LEFT_SHIFT" | "KEY_RIGHT_SHIFT" => Some(2),
        "GUI" | "KEY_LEFT_GUI" | "KEY_RIGHT_GUI" => Some(3),
        _ => None,
    }
}
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
fn default_modifiers(value: &mut Shortcut) {
    // Current 94608/Te.updateMapping calls getModifiers for an empty
    // recording. GPUI supplies side-independent modifier booleans.
    if value.modifiers.is_empty() {
        value.modifiers = vec!["CTRL".into(), "SHIFT".into()];
    }
}
/// Strict native registration contract, retained with the pure encoder.
#[allow(dead_code)]
pub(crate) fn validate_shortcuts(items: &[Shortcut]) -> Result<(), String> {
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
/// Oe saves output mappings before be records an input. Conflicts remain visible
/// as Te warnings; they are never accepted by validate_shortcuts/encode_shortcuts.
pub(crate) fn validate_stored_shortcuts(items: &[Shortcut]) -> Result<(), String> {
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
const MOUSE_INPUTS: &[(&str, &str)] = &[
    ("ScrollButton", "SCROLL_CLICK"),
    ("RightClick", "RIGHT_CLICK"),
    ("Button4", "MOUSE_BUTTON_4"),
    ("Button5", "MOUSE_BUTTON_5"),
];
fn input_label(input: &str) -> String {
    MOUSE_INPUTS
        .iter()
        .find(|(id, _)| *id == input)
        .map(|(_, label)| i18n::t(label))
        .or_else(|| source_shortcuts().key_names.get(input).cloned())
        .unwrap_or_else(|| key_label(input))
}
fn kinds() -> Vec<Choice> {
    [
        ("program", "启动程序"),
        ("website", "打开网站"),
        ("multimedia", "多媒体"),
        ("windows", "Windows 快捷方式"),
        ("text", "文本功能"),
    ]
    .into_iter()
    .map(|(id, label)| Choice::new(id, label))
    .chain(std::iter::once(Choice::new("macro", i18n::t("MACRO"))))
    .collect()
}
fn action_choices(kind: &str) -> Vec<Choice> {
    let choices: &[SourceShortcutChoice] = match kind {
        "multimedia" => &source_shortcuts().media,
        "windows" => &source_shortcuts().windows,
        _ => &[],
    };
    choices
        .iter()
        .map(|item| Choice::new(item.id.clone(), i18n::t(&item.content)))
        .collect()
}

struct Draft {
    value: Shortcut,
    original: Option<Shortcut>,
}
pub(crate) struct Shortcuts {
    items: Vec<Shortcut>,
    saved: Vec<Shortcut>,
    draft: Option<Draft>,
    recording: bool,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    kind: Entity<SelectState<Vec<Choice>>>,
    action: Entity<SelectState<Vec<Choice>>>,
    mouse: Entity<SelectState<Vec<Choice>>>,
    target: Entity<InputState>,
    paragraph: Entity<TextareaState>,
    subscriptions: Vec<Subscription>,
    source_alert: Option<Entity<SourceAlert>>,
    delete_confirmation: Option<String>,
    delete_focus: FocusHandle,
    list_focus: FocusHandle,
    delete_return_focus: Option<FocusHandle>,
    draft_generation: u64,
    mapping_category: String,
    mapping_expanded: bool,
    menu_for: Option<String>,
    menu_button_bounds: Bounds<Pixels>,
    recording_id: Option<String>,
    recording_ready: Option<Instant>,
    recording_focus: FocusHandle,
    empty_device: Entity<SelectState<Vec<Choice>>>,
    empty_catalog: Entity<SelectState<Vec<Choice>>>,
    playback: Entity<SelectState<Vec<Choice>>>,
    macro_catalog: Vec<MacroCatalogItem>,
    macro_selector: Entity<SelectState<Vec<Choice>>>,
    macro_repeat: Entity<crate::ui::stepper::Stepper>,
    macro_repeat_draft: String,
    launch_program: String,
    launch_website: String,
    emoji_open: bool,
    emoji_search: Entity<InputState>,
    emoji_category: usize,
    emoji_cursor: usize,
    emoji_tab_focus: FocusHandle,
    emoji_item_focus: FocusHandle,
    emoji_scroll: ScrollHandle,
    emoji_last_scroll: Pixels,
    emoji_button_bounds: Bounds<Pixels>,
    emoji_text_bounds: Bounds<Pixels>,
    emoji_toolbar_bounds: Bounds<Pixels>,
    emoji_hover: Option<(usize, usize)>,
    emoji_search_generation: u64,
    emoji_resetting: bool,
}
pub(crate) struct ShortcutsChanged;
impl EventEmitter<ShortcutsChanged> for Shortcuts {}
pub(crate) enum ShortcutsOpenModule {
    Macro,
    ChromaStudio,
    CharacterMap,
}
impl EventEmitter<ShortcutsOpenModule> for Shortcuts {}

impl Shortcuts {
    pub(crate) fn new(items: Vec<Shortcut>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let kind = cx.new(|cx| SelectState::new(kinds(), Some(IndexPath::new(0)), window, cx));
        let action = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let mouse = cx.new(|cx| {
            SelectState::new(
                MOUSE_INPUTS
                    .iter()
                    .map(|(id, label)| Choice::new(*id, i18n::t(label)))
                    .collect::<Vec<_>>(),
                None,
                window,
                cx,
            )
        });
        let target = cx.new(|cx| InputState::new(window, cx));
        let paragraph =
            cx.new(|cx| TextareaState::new(window, cx).placeholder(i18n::t("ENTER_TEXT")));
        let emoji_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(i18n::t("TEXT_SEARCH_EMOJI")));
        let empty_device = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let empty_catalog = cx.new(|cx| {
            SelectState::new(
                vec![Choice::new("empty", " ")],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let playback = cx.new(|cx| {
            SelectState::new(
                source_shortcuts()
                    .macro_playback
                    .iter()
                    .map(|item| Choice::new(item.id.clone(), i18n::t(&item.content)))
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let macro_selector = cx.new(|cx| {
            SelectState::new(
                vec![Choice::new("empty", " ")],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let macro_repeat = cx.new(|cx| {
            crate::ui::stepper::Stepper::new(
                "shortcut-macro-repeat",
                2.,
                (1., 99., 1.),
                false,
                false,
                Some(2),
                window,
                cx,
            )
            .in_custom_keymapping()
            .live_update()
        });
        let mut this = Self {
            saved: items.clone(),
            items,
            draft: None,
            recording: false,
            focus: cx.focus_handle(),
            return_focus: None,
            kind,
            action,
            mouse,
            target,
            paragraph,
            subscriptions: vec![],
            source_alert: None,
            delete_confirmation: None,
            delete_focus: cx.focus_handle(),
            list_focus: cx.focus_handle(),
            delete_return_focus: None,
            draft_generation: 0,
            mapping_category: "SWITCH_DEVICE_PROFILE".into(),
            mapping_expanded: false,
            menu_for: None,
            menu_button_bounds: Bounds::default(),
            recording_id: None,
            recording_ready: None,
            recording_focus: cx.focus_handle(),
            empty_device,
            empty_catalog,
            playback,
            macro_catalog: vec![],
            macro_selector,
            macro_repeat,
            macro_repeat_draft: "2".into(),
            launch_program: String::new(),
            launch_website: String::new(),
            emoji_open: false,
            emoji_search,
            emoji_category: 0,
            emoji_cursor: 0,
            emoji_tab_focus: cx.focus_handle(),
            emoji_item_focus: cx.focus_handle(),
            emoji_scroll: ScrollHandle::new(),
            emoji_last_scroll: px(0.),
            emoji_button_bounds: Bounds::default(),
            emoji_text_bounds: Bounds::default(),
            emoji_toolbar_bounds: Bounds::default(),
            emoji_hover: None,
            emoji_search_generation: 0,
            emoji_resetting: false,
        };
        this.subscriptions.push(cx.subscribe_in(
            &this.emoji_search,
            window,
            |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.emoji_search_changed(window, cx);
                }
            },
        ));
        this.subscriptions.push(
            cx.subscribe_in(&this.kind, window, |this, _, event, w, cx| {
                if let SelectEvent::Confirm(Some(kind)) = event {
                    if let Some(draft) = &mut this.draft {
                        if draft.value.output.kind() != kind {
                            draft.value.output = ShortcutOutput::from_kind(kind);
                            this.draft_generation = this.draft_generation.wrapping_add(1);
                        }
                    }
                    this.sync(w, cx);
                    this.changed(cx);
                }
            }),
        );
        this.subscriptions
            .push(cx.subscribe(&this.action, |this, _, event, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.set_output_value(value.clone());
                    this.changed(cx);
                }
            }));
        this.subscriptions
            .push(cx.subscribe(&this.mouse, |this, _, event, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Some(draft) = &mut this.draft {
                        draft.value.input = value.clone();
                        default_modifiers(&mut draft.value);
                    }
                    this.recording = false;
                    this.changed(cx);
                }
            }));
        this.subscriptions
            .push(cx.subscribe(&this.target, |this, input, event, cx| {
                if matches!(event, InputEvent::Change)
                    && this
                        .draft
                        .as_ref()
                        .is_some_and(|draft| draft.value.output.kind() == "website")
                {
                    this.set_output_value(input.read(cx).value().to_string());
                    this.changed(cx);
                }
            }));
        this.subscriptions.push(cx.subscribe_in(
            &this.paragraph,
            window,
            |this, input, event, window, cx| {
                if matches!(event, InputEvent::Change)
                    && this
                        .draft
                        .as_ref()
                        .is_some_and(|draft| draft.value.output.kind() == "text")
                {
                    let value = input.read(cx).value().to_string();
                    let previous = this
                        .draft
                        .as_ref()
                        .map(|draft| draft.value.output.value())
                        .unwrap_or("");
                    let value =
                        if let Some((range, limited)) = limit_shortcut_text(previous, &value) {
                            input.update(cx, |input, cx| {
                                let caret = range.start;
                                input.set_selected_range(range, cx);
                                input.replace("", window, cx);
                                input.set_selected_range(caret..caret, cx);
                            });
                            limited
                        } else {
                            value
                        };
                    this.set_output_value(value);
                    this.changed(cx);
                }
            },
        ));
        this.subscriptions.push(cx.on_focus_out(
            &this.recording_focus,
            window,
            |this, _, _, cx| {
                this.recording = false;
                this.recording_id = None;
                this.recording_ready = None;
                cx.notify();
            },
        ));
        this.subscribe_macro_controls(window, cx);
        this
    }
    fn set_output_value(&mut self, value: String) {
        if let Some(draft) = &mut self.draft {
            match &mut draft.value.output {
                ShortcutOutput::Program { target } => {
                    *target = value.clone();
                    self.launch_program = value;
                }
                ShortcutOutput::Website { target } => {
                    *target = value.clone();
                    self.launch_website = value;
                }
                ShortcutOutput::Multimedia { action } | ShortcutOutput::Windows { action } => {
                    *action = value
                }
                ShortcutOutput::Text { text } => *text = value,
                ShortcutOutput::Macro { .. } => {}
            }
        }
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(ShortcutsChanged);
        cx.notify();
    }
    pub(crate) fn snapshot(&self) -> Vec<Shortcut> {
        self.items.clone()
    }
    pub(crate) fn saved_snapshot(&self) -> Vec<Shortcut> {
        self.saved.clone()
    }
    pub(crate) fn mark_saved(&mut self, snapshot: Vec<Shortcut>, cx: &mut Context<Self>) {
        self.saved = snapshot;
        cx.notify();
    }
    #[cfg(test)]
    pub(crate) fn dirty(&self) -> bool {
        self.committed_pending() || self.draft_dirty()
    }
    pub(crate) fn committed_pending(&self) -> bool {
        self.items != self.saved
    }
    pub(crate) fn draft_dirty(&self) -> bool {
        self.draft.as_ref().is_some_and(|draft| {
            draft
                .original
                .as_ref()
                .is_none_or(|original| original != &draft.value)
        })
    }
    fn error(&self) -> Option<String> {
        let draft = self.draft.as_ref()?;
        if let ShortcutOutput::Macro {
            macro_id, playback, ..
        } = &draft.value.output
        {
            if !self.macro_catalog.iter().any(|item| item.id == *macro_id) {
                return Some(i18n::t("CREATE_MACRO_MSG"));
            }
            if !self
                .selected_macro_playback()
                .iter()
                .any(|item| item.id == *playback)
            {
                return Some("宏引用或播放选项无效。".into());
            }
        }
        if matches!(&draft.value.output, ShortcutOutput::Macro { playback, .. } if playback == "NTimes")
            && !self
                .macro_repeat_draft
                .parse::<u8>()
                .is_ok_and(|value| (1..=99).contains(&value))
        {
            return Some("重复次数必须为 1–99。".into());
        }
        if self.mapping_category != draft.value.output.category() {
            return Some(i18n::t(match self.mapping_category.as_str() {
                "MACRO" => "CREATE_MACRO_MSG",
                "SWITCH_LIGHTING" => "CONFIGURE_CHROMA_STUDIO_MSG",
                _ => "DEVICE_NOT_CONNECTED_MSG",
            }));
        }
        draft.value.output.error().map(str::to_string)
    }
    pub(crate) fn valid(&self) -> bool {
        self.error().is_none()
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &self.draft else {
            return;
        };
        let kind = draft.value.output.kind().to_string();
        let value = draft.value.output.value().to_string();
        let input = draft.value.input.clone();
        if let ShortcutOutput::Macro { repeat_count, .. } = &draft.value.output {
            self.macro_repeat_draft = repeat_count.to_string();
            self.macro_repeat.update(cx, |stepper, cx| {
                stepper.reset_value(f64::from(*repeat_count), window, cx)
            });
        }
        self.kind
            .update(cx, |s, cx| s.set_selected_value(&kind, window, cx));
        self.action.update(cx, |s, cx| {
            s.set_items(action_choices(&kind), window, cx);
            s.set_selected_value(&value, window, cx);
        });
        self.mouse
            .update(cx, |s, cx| s.set_selected_value(&input, window, cx));
        self.target.update(cx, |s, cx| {
            s.set_value(self.launch_website.clone(), window, cx)
        });
        self.paragraph
            .update(cx, |s, cx| s.set_value(value, window, cx));
        self.sync_macro_controls(window, cx);
    }
    fn begin(
        &mut self,
        source: Option<String>,
        duplicate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !duplicate
            && self
                .draft
                .as_ref()
                .is_some_and(|draft| source.as_ref() == Some(&draft.value.id))
        {
            self.menu_for = None;
            if self
                .draft
                .as_ref()
                .is_some_and(|draft| draft.original.is_none())
            {
                self.discard_editor(window, cx);
            } else {
                cx.notify();
            }
            return;
        }
        // A new editor cannot share its save confirmation with a stale row popup.
        self.delete_confirmation = None;
        self.delete_return_focus = None;
        self.menu_for = None;
        self.reset_emoji_mapping(window, cx);
        self.draft_generation = self.draft_generation.wrapping_add(1);
        let original = source.and_then(|id| self.items.iter().find(|item| item.id == id).cloned());
        let mut value = original.clone().unwrap_or_else(|| Shortcut {
            id: String::new(),
            input: String::new(),
            modifiers: vec![],
            hypershift: false,
            output: ShortcutOutput::from_kind("program"),
        });
        if duplicate || original.is_none() {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            value.id = format!("shortcut-{seed}");
            value.input.clear();
            value.modifiers.clear();
        }
        if duplicate {
            self.discard_editor(window, cx);
            self.items.push(value);
            self.changed(cx);
            return;
        }
        self.mapping_category = if original.is_some() {
            value.output.category()
        } else {
            "SWITCH_DEVICE_PROFILE"
        }
        .into();
        self.mapping_expanded = false;
        self.launch_program = if value.output.kind() == "program" {
            value.output.value().into()
        } else {
            String::new()
        };
        self.launch_website = if value.output.kind() == "website" {
            value.output.value().into()
        } else {
            String::new()
        };
        self.return_focus = window.focused(cx);
        self.draft = Some(Draft {
            value,
            original: if duplicate { None } else { original },
        });
        self.recording = false;
        self.recording_id = None;
        self.recording_ready = None;
        self.sync(window, cx);
        window.focus(&self.focus, cx);
        self.changed(cx);
    }
    pub(crate) fn discard_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.emoji_open = false;
        self.emoji_hover = None;
        self.draft_generation = self.draft_generation.wrapping_add(1);
        let restore = self.focus.contains_focused(window, cx);
        self.draft = None;
        self.recording = false;
        self.recording_id = None;
        self.recording_ready = None;
        if let Some(focus) = self.return_focus.take()
            && restore
        {
            window.focus(&focus, cx);
        }
        self.changed(cx);
    }
    pub(crate) fn dismiss_for_navigation(&mut self, cx: &mut Context<Self>) {
        self.emoji_open = false;
        self.emoji_hover = None;
        self.delete_confirmation = None;
        self.delete_return_focus = None;
        self.draft_generation = self.draft_generation.wrapping_add(1);
        self.draft = None;
        self.recording = false;
        self.recording_id = None;
        self.recording_ready = None;
        self.menu_for = None;
        self.return_focus = None;
        self.changed(cx);
    }
    pub(crate) fn prepare_save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.error().is_some() {
            return false;
        }
        if let Some(mut draft) = self.draft.take() {
            // Output mapping save precedes the independent row input recorder.
            if !draft.value.input.is_empty() {
                default_modifiers(&mut draft.value);
            }
            if let ShortcutOutput::Website { target } = &mut draft.value.output {
                *target = normalized_website(target).expect("validated website");
            }
            if let ShortcutOutput::Macro {
                playback,
                repeat_count,
                ..
            } = &mut draft.value.output
            {
                // 82508 getMappingData always uses 2 outside NTimes, including
                // mappings reloaded from a local file with a different count.
                if playback != "NTimes" {
                    *repeat_count = 2;
                }
            }
            if let Some(item) = self.items.iter_mut().find(|item| item.id == draft.value.id) {
                *item = draft.value;
            } else {
                self.items.push(draft.value);
            }
            self.discard_editor(window, cx);
        }
        true
    }
    fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.draft_dirty() {
            self.discard_editor(window, cx);
            return;
        }
        let save = cx.entity().downgrade();
        let discard = save.clone();
        self.source_alert = Some(SourceAlert::open(
            i18n::t("SAVE_REMAPPED_BUTTON_HEADER"),
            format!(
                "{}\n\n{}",
                i18n::t("SAVE_REMAPPED_BUTTON_MSG1"),
                i18n::t("SAVE_REMAPPED_BUTTON_MSG2")
            ),
            "shortcut-keep-editing",
            vec![
                AlertAction::new(
                    "shortcut-discard",
                    i18n::t("DONT_SAVE"),
                    move |window, cx| {
                        let _ = discard.update(cx, |this, cx| this.discard_editor(window, cx));
                    },
                ),
                AlertAction::new(
                    "shortcut-save-and-close",
                    i18n::t("SAVE"),
                    move |window, cx| {
                        let _ = save.update(cx, |this, cx| {
                            this.prepare_save(window, cx);
                        });
                    },
                )
                .primary()
                .disabled(!self.valid()),
            ],
            AlertPlacement::AboveCenter,
            window,
            cx,
        ));
        cx.notify();
    }
    fn delete(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if !self.items.iter().any(|item| item.id == id) {
            return;
        }
        self.delete_confirmation = Some(id);
        self.delete_return_focus = window.focused(cx);
        window.focus(&self.delete_focus, cx);
        cx.notify();
    }
    fn dismiss_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_confirmation = None;
        if let Some(focus) = self.delete_return_focus.take() {
            window.focus(&focus, cx);
        }
        cx.notify();
    }
    fn delete_popover(&self, item: &Shortcut, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let confirm_owner = owner.clone();
        let id = item.id.clone();
        let chord = item.chord();
        // 7282 .shortcut_item .profile-del is anchored to the row, not the
        // window. A zero-size trigger supplies its original left274/top53 point.
        gpui_kit::base::Popover::new(SharedString::from(format!("shortcut-delete-popup-{id}")))
            .absolute()
            .left(surface::css(274.))
            .top(surface::css(53.))
            .size_0()
            .open(true)
            .track_focus(&self.delete_focus)
            .trigger_with(|_, _, _| div().size_0().into_any_element())
            .on_open_change(move |open, window, cx| {
                if !open {
                    let _ = owner.update(cx, |page, cx| page.dismiss_delete(window, cx));
                }
            })
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let owner = confirm_owner.clone();
                let id = id.clone();
                let danger = crate::ui::theme::ProfileAlertColors::new().danger();
                v_flex()
                    .id("shortcut-delete-confirmation")
                    .test_support()
                    .role(Role::Dialog)
                    .aria_label(format!("{}：{chord}", i18n::t("DELETE_SHORTCUT")))
                    .w(surface::css(300.))
                    .p(surface::css(20.))
                    .items_center()
                    .rounded(surface::css(3.))
                    .border_1()
                    .border_color(danger)
                    .bg(cx.theme().group_box)
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .shadow(vec![BoxShadow {
                        color: cx.theme().title_bar.opacity(0.2),
                        offset: point(Pixels::ZERO, cx.theme().font_size * (6. / 16.)),
                        blur_radius: cx.theme().font_size * (10. / 16.),
                        spread_radius: Pixels::ZERO,
                        inset: false,
                    }])
                    .child(
                        div()
                            .text_center()
                            .text_color(danger)
                            .mb(surface::css(10.))
                            .child(i18n::t("DELETE_SHORTCUT")),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_center()
                            .whitespace_normal()
                            .mb(surface::css(10.))
                            .child(i18n::t("DELETE_SHORTCUT_MESSAGE")),
                    )
                    .child(
                        Button::new("shortcut-delete-confirm")
                            .label(i18n::t("REMOVE"))
                            .h(surface::css(27.))
                            .min_w(surface::css(90.))
                            .px(surface::css(5.))
                            .py_0()
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .rounded(cx.theme().font_size * (3. / 16.))
                            .border_1()
                            .border_color(cx.theme().title_bar.opacity(0.3))
                            .custom(
                                button::ButtonCustomVariant::new(cx)
                                    .color(danger)
                                    .foreground(cx.theme().primary_foreground)
                                    .hover(danger.opacity(0.8))
                                    .active(danger.opacity(0.6)),
                            )
                            .on_click(move |_, window, cx| {
                                let _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                                let _ = owner.update(cx, |page, cx| {
                                    page.delete_confirmation = None;
                                    page.delete_return_focus = None;
                                    page.items.retain(|item| item.id != id);
                                    page.discard_editor(window, cx);
                                    window.focus(&page.list_focus, cx);
                                    page.changed(cx);
                                });
                            }),
                    )
                    .into_any_element()
            })
            .into_any_element()
    }
    fn start_recording(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.discard_editor(window, cx);
        self.menu_for = None;
        self.recording_id = Some(id);
        self.recording = true;
        // Current be registers browser and native input listeners after 100 ms.
        self.recording_ready = Some(Instant::now() + Duration::from_millis(100));
        window.focus(&self.recording_focus, cx);
        cx.notify();
    }
    fn record_input(&mut self, input: String, modifiers: Modifiers, cx: &mut Context<Self>) {
        if !self.recording || self.recording_ready.is_none_or(|at| Instant::now() < at) {
            return;
        }
        let Some(id) = &self.recording_id else {
            return;
        };
        if let Some(item) = self.items.iter_mut().find(|item| &item.id == id) {
            item.input = input;
            // GPUI exposes modifier families, not physical left/right keys.
            // Current be ignores the Windows key and gets Hypershift from the
            // unavailable native device-mode event. Do not fabricate either.
            item.modifiers = [modifiers.control, modifiers.alt, modifiers.shift]
                .into_iter()
                .zip(MODIFIERS)
                .filter(|(on, _)| *on)
                .map(|(_, (id, _))| (*id).into())
                .collect();
            item.hypershift = false;
            default_modifiers(item);
        }
        self.changed(cx);
    }
    fn capture(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.recording {
            return;
        }
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        let Some(key) = canonical_key(&event.keystroke.key) else {
            return;
        };
        if modifier_family(&key).is_some() || source_shortcuts().excluded_keys.contains(&key) {
            return;
        }
        self.record_input(key, event.keystroke.modifiers, cx);
    }
    fn select_category(&mut self, category: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.mapping_category == category {
            return;
        }
        let kind = match category.as_str() {
            "TEXT_FUNCTION" => Some("text"),
            "LAUNCH_PROGRAM" => Some("program"),
            "MULTIMEDIA" => Some("multimedia"),
            "WINDOWS_SHORTCUT" => Some("windows"),
            "MACRO" => Some("macro"),
            _ => None,
        };
        if let (Some(kind), Some(draft)) = (kind, &mut self.draft) {
            draft.value.output = ShortcutOutput::from_kind(kind);
        }
        if category == "LAUNCH_PROGRAM" {
            self.launch_program.clear();
            self.launch_website.clear();
        }
        self.mapping_category = category;
        if self.mapping_category == "MACRO" {
            self.choose_default_macro();
        }
        self.reset_emoji_mapping(window, cx);
        self.mapping_expanded = false;
        self.draft_generation = self.draft_generation.wrapping_add(1);
        self.sync(window, cx);
        self.changed(cx);
    }
    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &self.draft else {
            return;
        };
        let id = draft.value.id.clone();
        self.draft_generation = self.draft_generation.wrapping_add(1);
        let generation = self.draft_generation;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择程序".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picker.await
                && let Some(path) = paths.first()
            {
                let value = path.to_string_lossy().into_owned();
                let _ = this.update_in(cx, |this, w, cx| {
                    if this.draft_generation == generation
                        && this.draft.as_ref().is_some_and(|draft| {
                            draft.value.id == id && draft.value.output.kind() == "program"
                        })
                    {
                        this.set_output_value(value);
                        this.sync(w, cx);
                        this.changed(cx);
                    }
                });
            }
        })
        .detach();
    }
}

include!("shortcuts_view.rs");
include!("shortcuts_mapping.rs");
include!("shortcuts_text.rs");
include!("shortcuts_macro.rs");

#[cfg(test)]
#[path = "shortcuts_tests.rs"]
mod tests;

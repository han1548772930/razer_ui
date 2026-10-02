//! Local global-shortcut editing follows frontend 7282/Oe and its mapping kinds.
//! Runtime registration is separate from storing a user's configuration.
use super::{
    controls::Choice,
    workspace::{MEDIA, WINDOWS, canonical_key, key_label, normalized_website},
};
use crate::ui::source_alert::{AlertAction, AlertPlacement, SourceAlert};
use crate::{i18n, ui::surface};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::{Deserialize, Serialize};

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
    Program { target: String },
    Website { target: String },
    Multimedia { action: String },
    Windows { action: String },
    Text { text: String },
}

impl ShortcutOutput {
    fn kind(&self) -> &'static str {
        match self {
            Self::Program { .. } => "program",
            Self::Website { .. } => "website",
            Self::Multimedia { .. } => "multimedia",
            Self::Windows { .. } => "windows",
            Self::Text { .. } => "text",
        }
    }
    fn value(&self) -> &str {
        match self {
            Self::Program { target } | Self::Website { target } => target,
            Self::Multimedia { action } | Self::Windows { action } => action,
            Self::Text { text } => text,
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
            _ => None,
        }
    }
}

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
    let label = MODIFIERS[family].1;
    if key.starts_with("KEY_LEFT_") {
        format!("左 {label}")
    } else if key.starts_with("KEY_RIGHT_") {
        format!("右 {label}")
    } else {
        label.into()
    }
}
fn default_modifiers(value: &mut Shortcut) {
    // Original frontend 7282/updateMapping calls getModifiers for an empty
    // recording. GPUI supplies side-independent modifier booleans.
    if value.modifiers.is_empty() {
        value.modifiers = vec!["CTRL".into(), "SHIFT".into()];
    }
}
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
const MOUSE_INPUTS: &[(&str, &str)] = &[
    ("ScrollButton", "滚轮单击"),
    ("RightClick", "右键单击"),
    ("Button4", "鼠标按钮 4"),
    ("Button5", "鼠标按钮 5"),
];
fn input_label(input: &str) -> String {
    MOUSE_INPUTS
        .iter()
        .find(|(id, _)| *id == input)
        .map(|(_, label)| (*label).into())
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
    .collect()
}
fn action_choices(kind: &str) -> Vec<Choice> {
    match kind {
        "multimedia" => MEDIA,
        "windows" => WINDOWS,
        _ => &[],
    }
    .iter()
    .map(|(id, key)| Choice::new(*id, i18n::t(key)))
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
    encoding_status: Option<Result<usize, String>>,
}
pub(crate) struct ShortcutsChanged;
impl EventEmitter<ShortcutsChanged> for Shortcuts {}

impl Shortcuts {
    pub(crate) fn new(items: Vec<Shortcut>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let kind = cx.new(|cx| SelectState::new(kinds(), Some(IndexPath::new(0)), window, cx));
        let action = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let mouse = cx.new(|cx| {
            SelectState::new(
                MOUSE_INPUTS
                    .iter()
                    .map(|(id, label)| Choice::new(*id, *label))
                    .collect::<Vec<_>>(),
                None,
                window,
                cx,
            )
        });
        let target = cx.new(|cx| InputState::new(window, cx));
        let paragraph = cx.new(|cx| TextareaState::new(window, cx));
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
            encoding_status: None,
        };
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
                if matches!(event, InputEvent::Change) {
                    this.set_output_value(input.read(cx).value().to_string());
                    this.changed(cx);
                }
            }));
        this.subscriptions
            .push(cx.subscribe(&this.paragraph, |this, input, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.set_output_value(input.read(cx).value().to_string());
                    this.changed(cx);
                }
            }));
        this.subscriptions
            .push(cx.on_focus_out(&this.focus, window, |this, _, _, cx| {
                this.recording = false;
                cx.notify();
            }));
        this
    }
    fn set_output_value(&mut self, value: String) {
        if let Some(draft) = &mut self.draft {
            match &mut draft.value.output {
                ShortcutOutput::Program { target } | ShortcutOutput::Website { target } => {
                    *target = value
                }
                ShortcutOutput::Multimedia { action } | ShortcutOutput::Windows { action } => {
                    *action = value
                }
                ShortcutOutput::Text { text } => *text = value,
            }
        }
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.encoding_status = None;
        cx.emit(ShortcutsChanged);
        cx.notify();
    }
    fn check_encoding(&mut self, cx: &mut Context<Self>) {
        if self.draft.is_some() {
            return;
        }
        self.encoding_status =
            Some(super::shortcut_engine::encode_shortcuts(&self.items).map(|_| self.items.len()));
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
        let mut value = draft.value.clone();
        default_modifiers(&mut value);
        if let Err(error) = value.validate() {
            return Some(error);
        }
        if self
            .items
            .iter()
            .any(|item| item.id != value.id && item.same_chord(&value))
        {
            return Some("此按键组合已经分配给另一个全局快捷键。".into());
        }
        None
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
        self.kind
            .update(cx, |s, cx| s.set_selected_value(&kind, window, cx));
        self.action.update(cx, |s, cx| {
            s.set_items(action_choices(&kind), window, cx);
            s.set_selected_value(&value, window, cx);
        });
        self.mouse
            .update(cx, |s, cx| s.set_selected_value(&input, window, cx));
        self.target
            .update(cx, |s, cx| s.set_value(value.clone(), window, cx));
        self.paragraph
            .update(cx, |s, cx| s.set_value(value, window, cx));
    }
    fn begin(
        &mut self,
        source: Option<String>,
        duplicate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A new editor cannot share its save confirmation with a stale row popup.
        self.delete_confirmation = None;
        self.delete_return_focus = None;
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
        self.return_focus = window.focused(cx);
        self.draft = Some(Draft {
            value,
            original: if duplicate { None } else { original },
        });
        self.recording = false;
        self.sync(window, cx);
        window.focus(&self.focus, cx);
        self.changed(cx);
    }
    pub(crate) fn discard_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft_generation = self.draft_generation.wrapping_add(1);
        let restore = self.focus.contains_focused(window, cx);
        self.draft = None;
        self.recording = false;
        if let Some(focus) = self.return_focus.take()
            && restore
        {
            window.focus(&focus, cx);
        }
        self.changed(cx);
    }
    pub(crate) fn dismiss_for_navigation(&mut self, cx: &mut Context<Self>) {
        self.delete_confirmation = None;
        self.delete_return_focus = None;
        self.draft_generation = self.draft_generation.wrapping_add(1);
        self.draft = None;
        self.recording = false;
        self.return_focus = None;
        self.changed(cx);
    }
    pub(crate) fn prepare_save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.error().is_some() {
            return false;
        }
        if let Some(mut draft) = self.draft.take() {
            default_modifiers(&mut draft.value);
            if let ShortcutOutput::Website { target } = &mut draft.value.output {
                *target = normalized_website(target).expect("validated website");
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
    pub(crate) fn discard_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.items = self.saved.clone();
        self.discard_editor(window, cx);
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
        if self.draft.is_some() || !self.items.iter().any(|item| item.id == id) {
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
                                    window.focus(&page.list_focus, cx);
                                    page.changed(cx);
                                });
                            }),
                    )
                    .into_any_element()
            })
            .into_any_element()
    }
    fn capture(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.recording {
            return;
        }
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        if event.keystroke.key == "escape" {
            self.recording = false;
            cx.notify();
            return;
        }
        let Some(key) = canonical_key(&event.keystroke.key) else {
            return;
        };
        if modifier_family(&key).is_some() {
            return;
        }
        let m = event.keystroke.modifiers;
        if let Some(draft) = &mut self.draft {
            draft.value.input = key;
            draft.value.modifiers = [m.control, m.alt, m.shift, m.platform]
                .into_iter()
                .zip(MODIFIERS)
                .filter(|(on, _)| *on)
                .map(|(_, (id, _))| (*id).into())
                .collect();
            default_modifiers(&mut draft.value);
        }
        self.recording = false;
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
    fn editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let draft = self.draft.as_ref().expect("open editor");
        let kind = draft.value.output.kind();
        let target = match kind {
            "text" => div()
                .id("shortcut-text")
                .test_support()
                .w_full()
                .child(
                    Textarea::new(&self.paragraph)
                        .w_full()
                        .h(surface::css(120.)),
                )
                .into_any_element(),
            "multimedia" | "windows" => surface::select(&self.action)
                .id("shortcut-action")
                .items(action_choices(kind))
                .w_full()
                .into_any_element(),
            _ => v_flex()
                .gap_2()
                .child(Input::new(&self.target).id("shortcut-target").w_full())
                .when(kind == "program", |s| {
                    s.child(
                        Button::new("shortcut-browse")
                            .label("浏览…")
                            .outline()
                            .on_click(cx.listener(|s, _, w, cx| s.browse(w, cx))),
                    )
                })
                .into_any_element(),
        };
        v_flex()
            .id("shortcut-editor")
            .test_support()
            .track_focus(&self.focus)
            .tab_group()
            .capture_key_down(cx.listener(|s, e, w, cx| s.capture(e, w, cx)))
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key == "escape" && !s.recording {
                    s.close_editor(w, cx);
                    cx.stop_propagation();
                }
            }))
            .w(surface::css(292.))
            .min_w(surface::css(292.))
            .p(surface::css(20.))
            .gap(surface::css(12.))
            .bg(cx.theme().group_box)
            .border_1()
            .border_color(cx.theme().border)
            .child(
                h_flex().justify_between().child("全局快捷键映射").child(
                    Button::new("shortcut-close")
                        .ghost()
                        .label("×")
                        .accessibility_label("关闭快捷键编辑器")
                        .on_click(cx.listener(|s, _, w, cx| s.close_editor(w, cx))),
                ),
            )
            .child(div().child("按键组合"))
            .child(
                Button::new("shortcut-record")
                    .outline()
                    .w_full()
                    .label(if self.recording {
                        "请按下组合键…".into()
                    } else if draft.value.input.is_empty() {
                        "录制快捷键".into()
                    } else {
                        draft.value.chord()
                    })
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.recording = !s.recording;
                        w.focus(&s.focus, cx);
                        cx.notify();
                    })),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .children(MODIFIERS.iter().map(|(id, label)| {
                        let id = *id;
                        let family = modifier_family(id);
                        Checkbox::new(SharedString::from(format!("shortcut-modifier-{id}")))
                            .label(*label)
                            .checked(
                                draft
                                    .value
                                    .modifiers
                                    .iter()
                                    .any(|v| modifier_family(v) == family),
                            )
                            .on_click(cx.listener(move |s, checked, _, cx| {
                                if let Some(draft) = &mut s.draft {
                                    draft
                                        .value
                                        .modifiers
                                        .retain(|m| modifier_family(m) != family);
                                    if *checked {
                                        draft.value.modifiers.push(id.into());
                                    }
                                    draft.value.modifiers.sort_by_key(|m| modifier_family(m));
                                }
                                s.changed(cx);
                            }))
                    })),
            )
            .child(
                Checkbox::new("shortcut-hypershift")
                    .label("Razer Hypershift")
                    .checked(draft.value.hypershift)
                    .on_click(cx.listener(|s, value, _, cx| {
                        if let Some(draft) = &mut s.draft {
                            draft.value.hypershift = *value;
                        }
                        s.changed(cx);
                    })),
            )
            .child(div().child("或选择鼠标输入"))
            .child(
                surface::select(&self.mouse)
                    .id("shortcut-mouse")
                    .items(
                        MOUSE_INPUTS
                            .iter()
                            .map(|(id, label)| Choice::new(*id, *label))
                            .collect(),
                    )
                    .w_full(),
            )
            .child(div().child("分配操作"))
            .child(
                surface::select(&self.kind)
                    .id("shortcut-kind")
                    .items(kinds())
                    .w_full(),
            )
            .child(target)
            .child(surface::note("宏、跨设备和 Chroma 操作需要相应服务。", cx))
            .when_some(self.error(), |s, error| {
                s.child(
                    div()
                        .id("shortcut-error")
                        .test_support()
                        .aria_label(error.clone())
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("shortcut-cancel")
                            .label("取消")
                            .on_click(cx.listener(|s, _, w, cx| s.close_editor(w, cx))),
                    )
                    .child(
                        Button::new("shortcut-apply")
                            .label("保存快捷键")
                            .primary()
                            .disabled(!self.valid())
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.prepare_save(w, cx);
                            })),
                    ),
            )
            .into_any_element()
    }
}
impl Render for Shortcuts {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editing = self.draft.is_some();
        h_flex()
            .id("global-shortcuts-workspace")
            .test_support()
            .track_focus(&self.list_focus)
            .items_start()
            .gap(surface::css(20.))
            .flex_wrap()
            .w_full()
            .child(
                surface::panel("全局快捷键", cx)
                    .id("global-shortcuts")
                    .relative()
                    .w(surface::css(600.))
                    .max_w_full()
                    .pt(surface::css(20.))
                    .px(surface::css(10.))
                    .pb(surface::css(30.))
                    .gap_0()
                    .child(
                        surface::asset_button(
                            "shortcut-add-icon",
                            "synapse/dashboard-add.svg",
                            "添加快捷键",
                            cx,
                        )
                        .absolute()
                        .top(surface::css(20.))
                        .right(surface::css(20.))
                        .size(surface::css(18.))
                        .disabled(editing)
                        .on_click(cx.listener(|s, _, w, cx| s.begin(None, false, w, cx))),
                    )
                    .child(
                        div()
                            .px(surface::css(10.))
                            .mb(surface::css(20.))
                            .child("为快捷键分配操作，并在不同应用程序中使用。"),
                    )
                    .children(self.items.iter().map(|item| {
                        let edit = item.id.clone();
                        let duplicate = edit.clone();
                        let delete = edit.clone();
                        h_flex()
                            .id(SharedString::from(format!("shortcut-row-{}", item.id)))
                            .test_support()
                            .relative()
                            .min_h(surface::css(70.))
                            .px(surface::css(20.))
                            .py(surface::css(8.))
                            .gap_3()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                Button::new(SharedString::from(format!(
                                    "shortcut-edit-{}",
                                    item.id
                                )))
                                .ghost()
                                .flex_1()
                                .justify_start()
                                .label(format!("{}  ·  {}", item.chord(), item.output.label()))
                                .disabled(editing)
                                .on_click(cx.listener(
                                    move |s, _, w, cx| s.begin(Some(edit.clone()), false, w, cx),
                                )),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "shortcut-duplicate-{}",
                                    item.id
                                )))
                                .ghost()
                                .label("复制")
                                .disabled(editing)
                                .on_click(cx.listener(
                                    move |s, _, w, cx| {
                                        s.begin(Some(duplicate.clone()), true, w, cx)
                                    },
                                )),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "shortcut-delete-{}",
                                    item.id
                                )))
                                .ghost()
                                .label("删除")
                                .disabled(editing)
                                .on_click(
                                    cx.listener(move |s, _, w, cx| s.delete(delete.clone(), w, cx)),
                                ),
                            )
                            .when(
                                self.delete_confirmation.as_deref() == Some(item.id.as_str()),
                                |row| row.child(self.delete_popover(item, cx)),
                            )
                    }))
                    .child(
                        Button::new("shortcut-add-card")
                            .ghost()
                            .label("添加快捷键")
                            .w_full()
                            .h(surface::css(70.))
                            .px(surface::css(20.))
                            .py(surface::css(8.))
                            .mt(surface::css(17.))
                            .mb(surface::css(25.))
                            .border_2()
                            .border_dashed()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().font_size * (5. / 16.))
                            .disabled(editing)
                            .on_click(cx.listener(|s, _, w, cx| s.begin(None, false, w, cx))),
                    )
                    .child(surface::note("快捷键配置保存在本机。", cx))
                    .child(
                        h_flex()
                            .gap_3()
                            .mt(surface::css(12.))
                            .child(
                                Button::new("shortcut-check")
                                    .outline()
                                    .label("检查配置")
                                    .disabled(editing)
                                    .on_click(cx.listener(|s, _, _, cx| s.check_encoding(cx))),
                            )
                            .child(
                                Button::new("shortcut-engine-apply")
                                    .label("应用到引擎")
                                    .disabled(true),
                            ),
                    )
                    .when_some(self.encoding_status.clone(), |s, result| {
                        let message = match &result {
                            Ok(count) => format!("{count} 个快捷键的配置检查通过。"),
                            Err(error) => format!("配置检查失败：{error}"),
                        };
                        s.child(
                            div()
                                .id("shortcut-encoding-status")
                                .test_support()
                                .aria_label(message.clone())
                                .mt(surface::css(8.))
                                .text_color(if result.is_ok() {
                                    cx.theme().foreground
                                } else {
                                    cx.theme().danger
                                })
                                .child(message),
                        )
                    })
                    .child(surface::note(
                        "暂时无法读取引擎中的现有快捷键，尚不能应用。",
                        cx,
                    ))
                    .when(self.dirty(), |s| {
                        s.child(
                            Button::new("shortcuts-discard-all")
                                .outline()
                                .label("丢弃快捷键更改")
                                .on_click(cx.listener(|s, _, w, cx| s.discard_all(w, cx))),
                        )
                    }),
            )
            .when(editing, |s| s.child(self.editor(cx)))
            .when_some(self.source_alert.clone(), |s, alert| s.child(alert))
    }
}

#[cfg(test)]
#[path = "shortcuts_tests.rs"]
mod tests;

//! 653 Tm/Em command modes and Ym Snap Tap capture. Profile data stays in
//! Keyboard; this owner retains only native controls and an uncommitted pair.
use super::{
    controls::{Choice, Choices},
    lighting_color::LightingColorPicker,
    settings::{DialMode, canonical_snap_key},
    workspace::{Continue, DeviceWorkspace},
};
use crate::{
    i18n,
    nav::Tab,
    ui::{
        surface::{self, SynapseSwitch},
        theme::ProfileAlertColors,
    },
};
use gpui_kit::base::{Button as BaseButton, Popover, PopoverState, TestSupportExt as _};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    color_picker::{ColorPickerEvent, ColorPickerState},
    input::{Input, InputEvent, InputState},
    radio::Radio,
    select::{Select, SelectEvent, SelectState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::collections::BTreeMap;

const KEY_NAMES: &[(&str, &str, &str)] = include!("mapping_keys.rs");

struct SnapCapture {
    pair: usize,
    key: usize,
    keys: [String; 2],
    return_focus: Option<FocusHandle>,
}

struct DialColorControl {
    state: Entity<ColorPickerState>,
    _subscription: Subscription,
}

#[derive(Clone, PartialEq, Eq)]
enum DialConfirmationAction {
    Delete(String),
    Reset,
}

/// A popup belongs to the device/profile that opened it, including when a
/// profile switch or a restored store arrives before the confirmation click.
#[derive(Clone, PartialEq, Eq)]
struct DialConfirmation {
    device: String,
    profile: String,
    action: DialConfirmationAction,
}

impl DialConfirmation {
    fn current(&self, workspace: &DeviceWorkspace) -> bool {
        self.device == workspace.identity()
            && self.profile == workspace.device().active_profile
            && workspace.page == Tab::Customize
            && match &self.action {
                DialConfirmationAction::Delete(uid) => workspace
                    .settings()
                    .keyboard
                    .dial_modes
                    .iter()
                    .any(|mode| mode.uid == *uid && mode.is_custom),
                DialConfirmationAction::Reset => true,
            }
    }
}

pub(super) struct KeyboardControls {
    name: Entity<InputState>,
    name_uid: Option<String>,
    dial_renaming: bool,
    dial_expanded: Option<String>,
    colors: BTreeMap<String, DialColorControl>,
    color_device: String,
    profile: String,
    dial_confirmation: Option<DialConfirmation>,
    dial_confirmation_focus: FocusHandle,
    snap_focus: FocusHandle,
    snap_choice: Entity<Choices>,
    choosing_key: bool,
    capture: Option<SnapCapture>,
    snap_message: Option<&'static str>,
    subscriptions: Vec<Subscription>,
}

impl KeyboardControls {
    pub(super) fn new(window: &mut Window, cx: &mut Context<DeviceWorkspace>) -> Self {
        Self {
            name: cx
                .new(|cx| InputState::new(window, cx).placeholder("模式名称（最多 40 个字符）")),
            name_uid: None,
            dial_renaming: false,
            dial_expanded: None,
            colors: BTreeMap::new(),
            color_device: String::new(),
            profile: String::new(),
            dial_confirmation: None,
            dial_confirmation_focus: cx.focus_handle(),
            snap_focus: cx.focus_handle(),
            snap_choice: cx.new(|cx| {
                SelectState::new(Vec::<Choice>::new(), None, window, cx).searchable(true)
            }),
            choosing_key: false,
            capture: None,
            snap_message: None,
            subscriptions: Vec::new(),
        }
    }
}

fn key_name(id: &str) -> String {
    KEY_NAMES
        .iter()
        .find(|(_, key, _)| *key == id)
        .map(|(_, _, name)| (*name).to_owned())
        .unwrap_or_else(|| id.to_owned())
}

fn pressed_key(key: &str) -> Option<String> {
    let id = match key.to_ascii_lowercase().as_str() {
        "space" | " " => "KEY_SPACEBAR",
        "enter" | "return" => "KEY_ENTER",
        "tab" => "KEY_TAB",
        "backspace" => "KEY_BACKSPACE",
        "escape" | "esc" => "KEY_ESC",
        "up" => "KEY_UP_ARROW",
        "down" => "KEY_DOWN_ARROW",
        "left" => "KEY_LEFT_ARROW",
        "right" => "KEY_RIGHT_ARROW",
        "pageup" | "page-up" => "KEY_PAGE_UP",
        "pagedown" | "page-down" => "KEY_PAGE_DOWN",
        "capslock" | "caps-lock" => "KEY_CAPS_LOCK",
        "numlock" | "num-lock" => "KEY_NUMPAD_NUM_LOCK",
        "scrolllock" | "scroll-lock" => "KEY_SCROLL_LOCK",
        "printscreen" | "print-screen" => "KEY_PRINT_SCREEN",
        "ctrl" | "control" => "KEY_LEFT_CTRL",
        "shift" => "KEY_LEFT_SHIFT",
        "alt" => "KEY_LEFT_ALT",
        "-" | "_" => "KEY_HYPEN",
        "=" | "+" => "KEY_EQUAL",
        "[" | "{" => "KEY_OPEN_SQUARE_BRACKET",
        "]" | "}" => "KEY_CLOSE_SQUARE_BRACKET",
        "\\" | "|" => "KEY_BACKSLASH",
        ";" | ":" => "KEY_SEMICOLON",
        "'" | "\"" => "KEY_APOSTROPHE",
        "`" | "~" => "KEY_TILDE",
        "," | "<" => "KEY_COMMA",
        "." | ">" => "KEY_PERIOD",
        "/" | "?" => "KEY_SLASH",
        _ => {
            return canonical_snap_key(key).or_else(|| {
                KEY_NAMES
                    .iter()
                    .find(|(_, _, label)| label.eq_ignore_ascii_case(key))
                    .and_then(|(_, id, _)| canonical_snap_key(id))
            });
        }
    };
    canonical_snap_key(id)
}

#[derive(Clone)]
struct DialDrag {
    uid: String,
    profile: String,
    label: String,
}

impl Render for DialDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .bg(cx.theme().group_box)
            .child(self.label.clone())
    }
}

impl DeviceWorkspace {
    pub(super) fn install_keyboard_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.keyboard_controls.subscriptions.push(cx.subscribe_in(
            &self.keyboard_controls.name,
            window,
            |this, _, event, window, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.commit_dial_name(window, cx);
                }
            },
        ));
        self.keyboard_controls.subscriptions.push(cx.on_focus_out(
            &self.keyboard_controls.snap_focus,
            window,
            |this, _, _, cx| {
                // Clicking another task cancels an incomplete pair without
                // moving focus away from the newly chosen control.
                if !this.keyboard_controls.choosing_key {
                    this.keyboard_controls.capture = None;
                    this.keyboard_controls.snap_message = None;
                    cx.notify();
                }
            },
        ));
        self.keyboard_controls.subscriptions.push(cx.subscribe_in(
            &self.keyboard_controls.snap_choice,
            window,
            |this, _, event, window, cx| {
                let SelectEvent::Confirm(Some(input)) = event else {
                    return;
                };
                if !this.keyboard_controls.choosing_key
                    || !this
                        .snap_choice_items()
                        .iter()
                        .any(|item| item.id() == input)
                {
                    return;
                }
                this.keyboard_controls.choosing_key = false;
                window.close_dialog(cx);
                this.capture_snap_input(input, window, cx);
                if this.keyboard_controls.capture.is_some() {
                    window.focus(&this.keyboard_controls.snap_focus, cx);
                }
            },
        ));
    }

    pub(super) fn sync_keyboard_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_dial_color_controls(window, cx);
        if self
            .keyboard_controls
            .dial_confirmation
            .as_ref()
            .is_some_and(|confirmation| !confirmation.current(self))
        {
            self.keyboard_controls.dial_confirmation = None;
        }
        if self.keyboard_controls.profile != self.device().active_profile {
            self.keyboard_controls.profile = self.device().active_profile.clone();
            self.keyboard_controls.capture = None;
            self.keyboard_controls.choosing_key = false;
            self.keyboard_controls.snap_message = None;
            self.dial_highlight = None;
        }
        if !self.settings().keyboard.snap_tap {
            self.keyboard_controls.capture = None;
        }
        let highlighted = self.dial_highlight.as_deref().and_then(|uid| {
            self.settings()
                .keyboard
                .dial_modes
                .iter()
                .find(|mode| mode.uid == uid)
                .cloned()
        });
        if self.dial_highlight.is_some() && highlighted.is_none() {
            self.dial_highlight = None;
        }
        let uid = highlighted.as_ref().map(|mode| mode.uid.clone());
        let changed = uid != self.keyboard_controls.name_uid;
        if changed || self.page != Tab::Customize {
            self.keyboard_controls.dial_renaming = false;
            self.keyboard_controls.dial_expanded = None;
        } else if highlighted.as_ref().is_none_or(|mode| !mode.enabled) {
            self.keyboard_controls.dial_expanded = None;
        }
        self.keyboard_controls.name_uid = uid.clone();
        if let Some(mode) = &highlighted {
            let name = mode.label();
            let input = &self.keyboard_controls.name;
            if (changed || !input.focus_handle(cx).is_focused(window))
                && input.read(cx).value().as_str() != name
            {
                input.update(cx, |input, cx| input.set_value(name, window, cx));
            }
        }
    }

    fn sync_dial_color_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let device = self.identity();
        let profile = self.device().active_profile.clone();
        let modes = if self.pid() == 653 {
            self.settings().keyboard.dial_modes.clone()
        } else {
            Vec::new()
        };
        let owner_changed = self.keyboard_controls.color_device != device
            || self.keyboard_controls.profile != profile;
        self.keyboard_controls.colors.retain(|uid, control| {
            let keep = !owner_changed && modes.iter().any(|mode| mode.uid == *uid);
            if !keep || self.page != Tab::Customize {
                control
                    .state
                    .update(cx, |picker, cx| picker.set_open(false, cx));
            }
            keep
        });
        self.keyboard_controls.color_device = device.clone();
        for mode in modes {
            // Em mounts one JM/$M picker in every standard-mode row. Keeping
            // its state keyed by uid avoids rebinding an open popup to another
            // mode when the highlighted row or the row order changes.
            let control = self
                .keyboard_controls
                .colors
                .entry(mode.uid.clone())
                .or_insert_with(|| {
                    let state = cx.new(|cx| ColorPickerState::new(window, cx));
                    let uid = mode.uid.clone();
                    let device = device.clone();
                    let profile = profile.clone();
                    let subscription =
                        cx.subscribe_in(&state, window, move |this, _, event, window, cx| {
                            let ColorPickerEvent::Change(Some(color)) = event else {
                                return;
                            };
                            if this.identity() != device
                                || this.device().active_profile != profile
                                || this.page != Tab::Customize
                                || !this
                                    .settings()
                                    .keyboard
                                    .dial_modes
                                    .iter()
                                    .any(|mode| mode.uid == uid)
                            {
                                return;
                            }
                            let color = Rgba::from(*color);
                            let color = [color.r, color.g, color.b]
                                .map(|value| (value * 255.).round() as u8);
                            this.edit(window, cx, |settings| {
                                if let Some(mode) = settings
                                    .keyboard
                                    .dial_modes
                                    .iter_mut()
                                    .find(|mode| mode.uid == uid)
                                {
                                    mode.color = color;
                                }
                            });
                        });
                    DialColorControl {
                        state,
                        _subscription: subscription,
                    }
                });
            let color: Hsla = rgb(u32::from_be_bytes([
                0,
                mode.color[0],
                mode.color[1],
                mode.color[2],
            ]))
            .into();
            if control.state.read(cx).value() != Some(color) {
                control
                    .state
                    .update(cx, |picker, cx| picker.set_value(color, window, cx));
            }
        }
    }

    fn commit_dial_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.keyboard_controls.dial_renaming {
            return;
        }
        self.keyboard_controls.dial_renaming = false;
        let Some(uid) = self.keyboard_controls.name_uid.clone() else {
            return;
        };
        let value = self.keyboard_controls.name.read(cx).value().to_string();
        self.edit(window, cx, |settings| {
            settings.keyboard.rename_dial(&uid, &value);
        });
        if let Some(mode) = self
            .settings()
            .keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == uid)
        {
            let name = mode.label();
            self.keyboard_controls
                .name
                .update(cx, |input, cx| input.set_value(name, window, cx));
        }
    }

    fn highlight_dial(&mut self, uid: String, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_dial_name(window, cx);
        self.dial_highlight = Some(uid);
        self.sync_keyboard_controls(window, cx);
        cx.notify();
    }

    fn begin_dial_rename(&mut self, uid: String, window: &mut Window, cx: &mut Context<Self>) {
        self.highlight_dial(uid.clone(), window, cx);
        if self
            .settings()
            .keyboard
            .dial_modes
            .iter()
            .any(|mode| mode.uid == uid && mode.is_custom && mode.enabled)
        {
            self.keyboard_controls.dial_renaming = true;
            self.keyboard_controls
                .name
                .focus_handle(cx)
                .focus(window, cx);
            cx.notify();
        }
    }

    pub(super) fn start_snap_capture(
        &mut self,
        pair: usize,
        key: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.settings().keyboard.snap_tap || key > 1 {
            return;
        }
        let pairs = self.settings().keyboard.snap_key_pairs();
        let Some(keys) = pairs.get(pair).cloned() else {
            return;
        };
        self.keyboard_controls.capture = Some(SnapCapture {
            pair,
            key,
            keys,
            return_focus: window.focused(cx),
        });
        self.keyboard_controls.snap_message = None;
        window.focus(&self.keyboard_controls.snap_focus, cx);
        cx.notify();
    }

    pub(super) fn start_add_snap_pair(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pairs = self.settings().keyboard.snap_key_pairs();
        if !self.settings().keyboard.snap_tap || pairs.len() >= 4 {
            return;
        }
        self.keyboard_controls.capture = Some(SnapCapture {
            pair: pairs.len(),
            key: 0,
            keys: [String::new(), String::new()],
            return_focus: window.focused(cx),
        });
        self.keyboard_controls.snap_message = None;
        window.focus(&self.keyboard_controls.snap_focus, cx);
        cx.notify();
    }

    pub(super) fn cancel_snap_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.keyboard_controls.choosing_key = false;
        if let Some(capture) = self.keyboard_controls.capture.take() {
            if self
                .keyboard_controls
                .snap_focus
                .contains_focused(window, cx)
            {
                if let Some(focus) = capture.return_focus {
                    window.focus(&focus, cx);
                }
            }
        }
        self.keyboard_controls.snap_message = None;
    }

    pub(super) fn capture_snap_input(
        &mut self,
        input: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(capture) = self.keyboard_controls.capture.as_ref() else {
            return false;
        };
        let pair_ix = capture.pair;
        let key_ix = capture.key;
        let valid = canonical_snap_key(input).is_some()
            && super::customize_drawer::keyboard_mapping_input(self.device().layout_id, input)
                .is_some();
        let duplicate = self
            .settings()
            .keyboard
            .snap_key_pairs()
            .iter()
            .enumerate()
            .any(|(ix, pair)| ix != pair_ix && pair.iter().any(|key| key == input))
            || capture.keys[1 - key_ix] == input;
        if !valid || duplicate {
            self.keyboard_controls.snap_message = Some(if duplicate {
                "每个按键只能用于一组 Snap Tap，且一组中的两个按键必须不同。"
            } else {
                "此按键不能用于 Snap Tap。"
            });
            cx.notify();
            return true;
        }
        let capture = self.keyboard_controls.capture.as_mut().unwrap();
        capture.keys[key_ix] = input.into();
        self.keyboard_controls.snap_message = None;
        if key_ix == 0 {
            capture.key = 1;
        } else {
            let capture = self.keyboard_controls.capture.take().unwrap();
            self.edit(window, cx, |settings| {
                settings.keyboard.set_snap_pair(capture.pair, capture.keys);
            });
            if let Some(focus) = capture.return_focus {
                window.focus(&focus, cx);
            }
        }
        cx.notify();
        true
    }

    fn capture_snap_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.keyboard_controls.capture.is_none() {
            return;
        }
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        if event.keystroke.modifiers.platform {
            return;
        }
        if event.keystroke.modifiers.alt && event.keystroke.key == "down" {
            self.open_snap_key_choice(window, cx);
            return;
        }
        if event.keystroke.key == "escape" {
            self.cancel_snap_capture(window, cx);
            cx.notify();
            return;
        }
        if let Some(key) = pressed_key(&event.keystroke.key) {
            self.capture_snap_input(&key, window, cx);
        } else {
            self.keyboard_controls.snap_message = Some("此按键不能用于 Snap Tap。");
            cx.notify();
        }
    }

    fn snap_choice_items(&self) -> Vec<Choice> {
        let Some(capture) = &self.keyboard_controls.capture else {
            return Vec::new();
        };
        let pairs = self.settings().keyboard.snap_key_pairs();
        KEY_NAMES
            .iter()
            .filter_map(|(_, id, _)| {
                let input =
                    super::customize_drawer::keyboard_mapping_input(self.device().layout_id, id)?;
                if !input.is_enabled
                    || input.disabled
                    || canonical_snap_key(id).is_none()
                    || capture.keys[1 - capture.key] == *id
                    || pairs
                        .iter()
                        .enumerate()
                        .any(|(ix, pair)| ix != capture.pair && pair.iter().any(|key| key == id))
                {
                    return None;
                }
                Some(Choice::new(*id, key_name(id)))
            })
            .collect()
    }

    fn open_snap_key_choice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Native fallback for keys the host cannot distinguish in key events.
        // 653 Ym records keyboard/inputredirect events and has no layout modal.
        if self.keyboard_controls.capture.is_none() {
            return;
        }
        let items = self.snap_choice_items();
        self.keyboard_controls.snap_choice.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            state.set_selected_index(None, window, cx);
        });
        self.keyboard_controls.choosing_key = true;
        let picker = self.keyboard_controls.snap_choice.clone();
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let entity = entity.clone();
            let cancel = entity.clone();
            dialog
                .title("选择布局按键")
                .child(
                    Select::new(&picker)
                        .id("snap-layout-key")
                        .placeholder("按名称查找按键")
                        .w_full(),
                )
                .on_close(move |_, window, cx| {
                    entity.update(cx, |this, cx| {
                        this.keyboard_controls.choosing_key = false;
                        if this.keyboard_controls.capture.is_some() {
                            window.focus(&this.keyboard_controls.snap_focus, cx);
                        }
                    });
                })
                .footer(h_flex().justify_end().child(
                    Button::new("snap-layout-cancel").label("取消").on_click(
                        move |_, window, cx| {
                            window.close_dialog(cx);
                            cancel.update(cx, |this, cx| {
                                this.keyboard_controls.choosing_key = false;
                                if this.keyboard_controls.capture.is_some() {
                                    window.focus(&this.keyboard_controls.snap_focus, cx);
                                }
                            });
                        },
                    ),
                ))
        });
    }

    pub(super) fn snap_tap_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.settings().keyboard.snap_tap;
        let mut pairs = self.settings().keyboard.snap_key_pairs();
        let capture = self.keyboard_controls.capture.as_ref();
        if let Some(capture) = capture {
            if capture.pair == pairs.len() {
                pairs.push(capture.keys.clone());
            } else if let Some(pair) = pairs.get_mut(capture.pair) {
                *pair = capture.keys.clone();
            }
        }
        let has_copilot = ["DKM_D2", "DKM_F6"].into_iter().any(|key| {
            super::customize_drawer::keyboard_mapping_input(self.device().layout_id, key).is_some()
        });
        surface::panel_with_control(
            i18n::t("SNAP_TAP_HEADER"),
            h_flex()
                .items_center()
                .gap(surface::css(10.))
                .child(
                    SynapseSwitch::new("snap-tap-enabled")
                        .accessibility_label(i18n::t("SNAP_TAP_HEADER"))
                        .checked(enabled)
                        .on_change(cx.listener(|this, value, window, cx| {
                            this.cancel_snap_capture(window, cx);
                            this.edit(window, cx, |settings| settings.keyboard.snap_tap = *value);
                        })),
                )
                .child(
                    surface::asset_button(
                        "snap-tap-help",
                        "synapse/help-default.svg",
                        "Snap Tap 使用说明",
                        cx,
                    )
                    .tooltip(i18n::t(if has_copilot {
                        "SNAP_TAP_TOOLTIP_COPILOT"
                    } else {
                        "SNAP_TAP_TOOLTIP_MENU"
                    })),
                ),
            cx,
        )
        .id("snap-tap-panel")
        .test_support()
        .track_focus(&self.keyboard_controls.snap_focus)
        .capture_key_down(
            cx.listener(|this, event, window, cx| this.capture_snap_key(event, window, cx)),
        )
        .child(div().child(i18n::t("SNAP_TAP_DESC")))
        .child(
            h_flex()
                .justify_between()
                .items_start()
                .child(
                    v_flex().children(pairs.iter().enumerate().map(|(pair_ix, pair)| {
                        h_flex()
                            .gap(surface::css(10.))
                            .mb(surface::css(10.))
                            .items_center()
                            .children(pair.iter().enumerate().map(|(key_ix, key)| {
                                let recording = capture.is_some_and(|capture| {
                                    capture.pair == pair_ix && capture.key == key_ix
                                });
                                Button::new(SharedString::from(format!(
                                    "snap-key-{pair_ix}-{key_ix}"
                                )))
                                .outline()
                                .label(if recording {
                                    "请按键…".into()
                                } else if key.is_empty() {
                                    "待设置".into()
                                } else {
                                    key_name(key)
                                })
                                .accessibility_label(format!(
                                    "Snap Tap 第 {} 组按键 {}",
                                    pair_ix + 1,
                                    key_ix + 1
                                ))
                                .selected(recording)
                                .disabled(!enabled)
                                .min_w(surface::css(64.))
                                .h(surface::css(44.))
                                .px(surface::css(10.))
                                .py_0()
                                .border(surface::css(2.))
                                .border_color(
                                    if recording && self.keyboard_controls.snap_message.is_some() {
                                        cx.theme().warning
                                    } else if recording {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().foreground
                                    },
                                )
                                .rounded(cx.theme().font_size * (4. / 16.))
                                .text_size(surface::css(11.))
                                .custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().transparent)
                                        .foreground(cx.theme().button_foreground)
                                        .hover(cx.theme().transparent)
                                        .active(cx.theme().transparent),
                                )
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        if this.keyboard_controls.capture.is_none() {
                                            this.continue_with(
                                                Continue::SnapCapture {
                                                    pair: pair_ix,
                                                    key: key_ix,
                                                },
                                                window,
                                                cx,
                                            );
                                        }
                                    },
                                ))
                            }))
                            .when(pair_ix > 0, |row| {
                                row.child(
                                    Button::new(SharedString::from(format!(
                                        "snap-delete-{pair_ix}"
                                    )))
                                    .ghost()
                                    .icon(gpui_kit::assets::IconName::Trash)
                                    .accessibility_label("删除按键组")
                                    .disabled(!enabled || capture.is_some())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.edit(window, cx, |settings| {
                                            settings.keyboard.remove_snap_pair(pair_ix)
                                        });
                                    })),
                                )
                            })
                    })),
                )
                .child(
                    BaseButton::new("snap-add-pair")
                        .child("+")
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(cx.theme().foreground)
                        .accessibility_label(i18n::t("SNAP_TAP_ADD_BUTTON_TOOLTIP"))
                        .tooltip(|window, cx| {
                            Tooltip::new(i18n::t("SNAP_TAP_ADD_BUTTON_TOOLTIP")).build(window, cx)
                        })
                        .styles(|s| s.disabled(|s| s.opacity(0.3)))
                        .w(surface::css(64.))
                        .h(surface::css(44.))
                        .p_0()
                        .border(surface::css(2.))
                        .border_color(cx.theme().foreground)
                        .when(enabled && pairs.len() < 4 && capture.is_none(), |button| {
                            button.hover(|style| {
                                style
                                    .border_color(cx.theme().primary)
                                    .text_color(cx.theme().primary)
                            })
                        })
                        .rounded(cx.theme().font_size * (5. / 16.))
                        .focus_visible(|s| s.border_color(cx.theme().primary))
                        .text_size(surface::css(20.))
                        .disabled(!enabled || pairs.len() >= 4 || capture.is_some())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.continue_with(Continue::AddSnapPair, window, cx)
                        })),
                ),
        )
        .when(capture.is_some(), |panel| {
            panel.child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("snap-cancel-capture")
                            .ghost()
                            .label("取消")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel_snap_capture(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("snap-choose-key")
                            .outline()
                            .label("选择布局按键…")
                            .tooltip("Alt+↓")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_snap_key_choice(window, cx)
                            })),
                    ),
            )
        })
        .when(capture.is_none(), |panel| {
            panel.child(surface::note(i18n::t("SNAP_TAP_INTRODUCE_TEXT"), cx))
        })
        .when(capture.is_some(), |panel| {
            panel.child(surface::note(
                "按下要使用的按键；Esc 取消，Alt+↓ 选择布局中的按键。",
                cx,
            ))
        })
        .when_some(self.keyboard_controls.snap_message, |panel, message| {
            panel.child(
                div()
                    .id("snap-tap-message")
                    .test_support()
                    .aria_label(message)
                    .text_color(cx.theme().warning)
                    .child(message),
            )
        })
        .into_any_element()
    }

    pub(super) fn command_dial_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let keyboard = &self.settings().keyboard;
        let at_limit = keyboard
            .dial_modes
            .iter()
            .filter(|mode| mode.is_custom)
            .count()
            >= 100;
        surface::panel_with_control(
            crate::i18n::t_or("COMMAND_DIAL", "命令旋钮"),
            h_flex()
                .flex_1()
                .justify_end()
                .items_center()
                .gap(surface::css(20.))
                .child(
                    dial_icon_button(
                        "dial-add",
                        if at_limit {
                            "synapse/dial-add-disabled.svg"
                        } else {
                            "synapse/dial-add.svg"
                        },
                        "synapse/dial-add-hover.svg",
                        i18n::t("ADD_NEW_MODE"),
                    )
                    .disabled(at_limit)
                    .on_click(cx.listener(|this, _, window, cx| {
                        let mut uid = None;
                        this.edit(window, cx, |settings| uid = settings.keyboard.add_dial());
                        if let Some(uid) = uid {
                            this.highlight_dial(uid, window, cx);
                        }
                    })),
                )
                .child(self.dial_confirmation_popover(DialConfirmationAction::Reset, cx)),
            cx,
        )
        .relative()
        .child(
            dial_icon_button(
                "dial-help",
                "synapse/help-default.svg",
                "synapse/help-hover.svg",
                i18n::t("COMMAND_DIAL"),
            )
            .absolute()
            .top(surface::css(10.))
            .right(surface::css(10.))
            .size(surface::css(14.))
            .tooltip(|window, cx| {
                Tooltip::element(|_, _| {
                    v_flex()
                        .id("dial-help-content")
                        .test_support()
                        .w(surface::css(278.))
                        .gap(surface::css(17.))
                        .child(i18n::t("COMMAND_DIAL_USAGE_1"))
                        .child(
                            v_flex().pl(surface::css(20.)).children(
                                ["COMMAND_DIAL_USAGE_2", "COMMAND_DIAL_USAGE_3"]
                                    .into_iter()
                                    .map(|key| {
                                        h_flex()
                                            .items_start()
                                            .gap(surface::css(5.))
                                            .child("•")
                                            .child(div().flex_1().child(i18n::t(key)))
                                    }),
                            ),
                        )
                        .child(i18n::t("COMMAND_DIAL_USAGE_4"))
                })
                .m_0()
                .px(surface::css(10.))
                .py(surface::css(8.))
                .bg(cx.theme().title_bar)
                .rounded_none()
                .shadow_none()
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .build(window, cx)
            }),
        )
        .child(crate::i18n::t("SUB_CONTENT_COMMAND_DIAL"))
        .children(
            keyboard
                .dial_modes
                .iter()
                .map(|mode| self.dial_mode_row(mode, cx)),
        )
        .into_any_element()
    }

    fn dial_mode_row(&self, mode: &DialMode, cx: &mut Context<Self>) -> AnyElement {
        let uid = mode.uid.clone();
        let blocked = self.settings().keyboard.dial_blocked_by_game_mode(mode);
        let selected = self.settings().keyboard.dial_active == uid;
        let highlighted = self.dial_highlight.as_deref() == Some(uid.as_str());
        let color = rgb(u32::from_be_bytes([
            0,
            mode.color[0],
            mode.color[1],
            mode.color[2],
        ]));
        let drag = DialDrag {
            uid: uid.clone(),
            profile: self.device().active_profile.clone(),
            label: mode.label(),
        };
        let drop_uid = uid.clone();
        let mut row = v_flex()
            .id(SharedString::from(format!("dial-row-{uid}")))
            .group("dial-row")
            .test_support()
            .bg(cx.theme().background)
            .my(surface::css(5.))
            .rounded(surface::css(5.))
            .border_1()
            .border_color(if highlighted {
                cx.theme().primary
            } else {
                cx.theme().transparent
            })
            .hover(|style| {
                style.border_color(if highlighted {
                    cx.theme().primary
                } else {
                    crate::ui::theme::CommandDialColors.hover_border()
                })
            })
            .on_drop(cx.listener(move |this, drag: &DialDrag, window, cx| {
                if drag.profile == this.device().active_profile {
                    this.edit(window, cx, |settings| {
                        let modes = &mut settings.keyboard.dial_modes;
                        if let (Some(from), Some(to)) = (
                            modes.iter().position(|mode| mode.uid == drag.uid),
                            modes.iter().position(|mode| mode.uid == drop_uid),
                        ) {
                            modes.swap(from, to);
                        }
                    });
                }
            }))
            .child(
                h_flex()
                    .min_h(surface::css(60.))
                    .px(surface::css(20.))
                    .gap(surface::css(6.))
                    .items_center()
                    .child(
                        Radio::new(SharedString::from(format!("dial-selected-{uid}")))
                            .label("")
                            .accessibility_label(format!("使用 {}", mode.label()))
                            .checked(selected)
                            .disabled(!mode.enabled || blocked)
                            .on_change(cx.listener({
                                let uid = uid.clone();
                                move |this, _, window, cx| {
                                    this.edit(window, cx, |settings| {
                                        settings.keyboard.select_dial(&uid)
                                    });
                                }
                            })),
                    )
                    .when_some(self.keyboard_controls.colors.get(&uid), |row, control| {
                        row.child(
                            div().mr(surface::css(20.)).flex_shrink_0().child(
                                LightingColorPicker::new(
                                    &control.state,
                                    format!("{} 模式指示灯颜色", mode.label()),
                                )
                                .dial_trigger(true)
                                // JM receives no hideNoColor prop here,
                                // but .mode .standard-mode .preset.no-color
                                // hides this cell in the final 653 CSS.
                                .allow_none(false),
                            ),
                        )
                    })
                    .child(
                        if highlighted && self.keyboard_controls.dial_renaming && mode.is_custom {
                            Input::new(&self.keyboard_controls.name)
                                .id("dial-name")
                                .w(surface::css(221.))
                                .h(surface::css(27.))
                                .disabled(!mode.enabled)
                                .into_any_element()
                        } else {
                            Button::new(SharedString::from(format!("dial-mode-{uid}")))
                                .ghost()
                                .label(mode.label())
                                .selected(highlighted)
                                .flex_1()
                                .min_w_0()
                                .justify_start()
                                .px_0()
                                .custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().transparent)
                                        .foreground(cx.theme().foreground)
                                        .hover(cx.theme().transparent)
                                        .active(cx.theme().transparent),
                                )
                                .on_click(cx.listener({
                                    let uid = uid.clone();
                                    move |this, _, window, cx| {
                                        this.begin_dial_rename(uid.clone(), window, cx)
                                    }
                                }))
                                .into_any_element()
                        },
                    )
                    .when(mode.is_custom && mode.enabled, |row| {
                        row.child(
                            dial_icon_button(
                                SharedString::from(format!("dial-expand-{uid}")),
                                "synapse/dial-more.svg",
                                "synapse/dial-more-hover.svg",
                                "旋转方向的按键分配",
                            )
                            .on_click(cx.listener({
                                let uid = uid.clone();
                                move |this, _, window, cx| {
                                    let expanded =
                                        this.keyboard_controls.dial_expanded.as_ref() == Some(&uid);
                                    this.highlight_dial(uid.clone(), window, cx);
                                    this.keyboard_controls.dial_expanded =
                                        (!expanded).then(|| uid.clone());
                                    cx.notify();
                                }
                            })),
                        )
                    })
                    .when(mode.is_custom, |row| {
                        row.child(self.dial_confirmation_popover(
                            DialConfirmationAction::Delete(uid.clone()),
                            cx,
                        ))
                    })
                    .child(
                        SynapseSwitch::new(SharedString::from(format!("dial-enabled-{uid}")))
                            .accessibility_label(format!("启用 {}", mode.label()))
                            .checked(mode.enabled)
                            .disabled(
                                !self
                                    .settings()
                                    .keyboard
                                    .dial_enabled_change_allowed(&uid, !mode.enabled),
                            )
                            .on_change(cx.listener({
                                let uid = uid.clone();
                                move |this, enabled, window, cx| {
                                    this.edit(window, cx, |settings| {
                                        settings.keyboard.enable_dial(&uid, *enabled)
                                    });
                                }
                            })),
                    )
                    .child(
                        gpui_kit::base::Button::new(SharedString::from(format!("dial-drag-{uid}")))
                            .accessibility_label("拖动排序模式，或按上、下方向键调整顺序")
                            .w(surface::css(8.))
                            .h(surface::css(20.))
                            .p_0()
                            .child(img("synapse/dpi-draggable.svg").size_full())
                            .on_key_down(cx.listener({
                                let uid = uid.clone();
                                move |this, event: &KeyDownEvent, window, cx| {
                                    let offset = match event.keystroke.key.as_str() {
                                        "up" => -1,
                                        "down" => 1,
                                        _ => return,
                                    };
                                    cx.stop_propagation();
                                    this.edit(window, cx, |settings| {
                                        settings.keyboard.move_dial(&uid, offset)
                                    });
                                }
                            }))
                            .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone())),
                    ),
            );
        if blocked {
            row = row.child(
                surface::note("禁用 Alt+Tab 时，此模式不可用。", cx)
                    .px(surface::css(20.))
                    .pb(surface::css(10.)),
            );
        }
        if highlighted
            && mode.is_custom
            && self.keyboard_controls.dial_expanded.as_ref() == Some(&uid)
        {
            row = row.child(
                v_flex()
                    .py(surface::css(10.))
                    .mx(surface::css(20.))
                    .min_h(surface::css(113.))
                    .border_t_1()
                    .border_color(crate::ui::theme::CommandDialColors.detail_border())
                    .gap_2()
                    .when(mode.is_custom, |detail| {
                        detail.child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    img("synapse/keyboard-653-digital-dial.png")
                                        .size(surface::css(64.))
                                        .rounded_full()
                                        .border(surface::css(3.))
                                        .border_color(color),
                                )
                                .child(
                                    v_flex().flex_1().gap_2().children(
                                        [("ScrollRight", "顺时针"), ("ScrollLeft", "逆时针")]
                                            .into_iter()
                                            .map(|(input, direction)| {
                                                let uid = uid.clone();
                                                let value = mode
                                                    .mappings
                                                    .get(input)
                                                    .map(String::as_str)
                                                    .unwrap_or("disable");
                                                Button::new(SharedString::from(format!(
                                                    "dial-mapping-{uid}-{input}"
                                                )))
                                                .outline()
                                                .label(format!(
                                                    "{direction} · {}",
                                                    self.mapping_summary(value).1
                                                ))
                                                .disabled(!mode.enabled)
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.continue_with(
                                                            Continue::DialInput {
                                                                mode_uid: uid.clone(),
                                                                input: input.into(),
                                                            },
                                                            window,
                                                            cx,
                                                        )
                                                    },
                                                ))
                                            }),
                                    ),
                                ),
                        )
                    }),
            );
        }
        row.into_any_element()
    }

    fn dial_confirmation_popover(
        &self,
        action: DialConfirmationAction,
        cx: &mut Context<Self>,
    ) -> Popover {
        let confirmation = DialConfirmation {
            device: self.identity(),
            profile: self.device().active_profile.clone(),
            action,
        };
        let reset = matches!(confirmation.action, DialConfirmationAction::Reset);
        let id = match &confirmation.action {
            DialConfirmationAction::Delete(uid) => format!("dial-delete-{uid}"),
            DialConfirmationAction::Reset => "dial-reset".to_owned(),
        };
        let focus = self.keyboard_controls.dial_confirmation_focus.clone();
        let workspace = cx.entity().downgrade();
        let open_workspace = workspace.clone();
        let target = confirmation.clone();
        Popover::new(SharedString::from(format!("{id}-popover")))
            .anchor(Anchor::TopRight)
            // 653: reset top:42px; delete top:26px. Both source icons are 20px.
            .offset(cx.theme().font_size * (if reset { 22. } else { 6. } / 16.))
            .size(surface::css(20.))
            .flex_shrink_0()
            .when(!reset, |popup| popup.self_end())
            .open(self.keyboard_controls.dial_confirmation.as_ref() == Some(&confirmation))
            .track_focus(&focus)
            .trigger_with(move |_, _, _| {
                let label = i18n::t(if reset {
                    "RESET_COMMAND_DIAL"
                } else {
                    "REMOVE"
                });
                let tooltip = label.clone();
                let (asset, hover) = if reset {
                    ("synapse/dial-reset.svg", "synapse/dial-reset-hover.svg")
                } else {
                    ("synapse/dial-delete.svg", "synapse/dial-delete-hover.svg")
                };
                BaseButton::new(SharedString::from(id))
                    .accessibility_label(label)
                    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                    .group("dial-confirm-trigger")
                    .relative()
                    .size(surface::css(20.))
                    .p_0()
                    .rounded_none()
                    .child(img(asset).size_full())
                    .child(
                        img(hover)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .opacity(0.)
                            .group_hover("dial-confirm-trigger", |style| style.opacity(1.)),
                    )
                    .into_any_element()
            })
            .on_open_change(move |open, _, cx| {
                _ = open_workspace.update(cx, |workspace, cx| {
                    if *open && target.current(workspace) {
                        workspace.keyboard_controls.dial_confirmation = Some(target.clone());
                    } else if workspace.keyboard_controls.dial_confirmation.as_ref()
                        == Some(&target)
                    {
                        workspace.keyboard_controls.dial_confirmation = None;
                    }
                    cx.notify();
                });
            })
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                // The source delete box is left:-141px; translateX(-50%).
                // At its 300px width that puts its right edge 11px before the
                // right edge of the 20px trigger. Keep that measured offset in
                // the popup bounds so Base can still clamp it to the viewport.
                div()
                    .when(!reset, |content| content.pr(surface::css(11.)))
                    .child(dial_confirmation_content(
                        confirmation,
                        popup,
                        workspace,
                        focus,
                        cx,
                    ))
            })
    }
}

fn dial_icon_button(
    id: impl Into<ElementId>,
    asset: &'static str,
    hover: &'static str,
    label: impl Into<SharedString>,
) -> BaseButton {
    let label = label.into();
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
        .group("dial-icon")
        .relative()
        .size(surface::css(20.))
        .flex_shrink_0()
        .p_0()
        .rounded_none()
        .child(img(asset).size_full())
        .child(
            img(hover)
                .absolute()
                .inset_0()
                .size_full()
                .opacity(0.)
                .group_hover("dial-icon", |style| style.opacity(1.)),
        )
}

fn dial_confirmation_content(
    confirmation: DialConfirmation,
    popup: WeakEntity<PopoverState>,
    workspace: WeakEntity<DeviceWorkspace>,
    focus: FocusHandle,
    cx: &App,
) -> impl IntoElement {
    let reset = matches!(confirmation.action, DialConfirmationAction::Reset);
    let (frame_id, button_id, title_key, message_key, button_key) = if reset {
        (
            "dial-reset-confirmation",
            "dial-reset-confirm",
            "RESET_COMMAND_DIAL",
            "RESET_COMMAND_DIAL_DISCRIPTION",
            "RESET",
        )
    } else {
        (
            "dial-delete-confirmation",
            "dial-delete-confirm",
            "REMOVE_CUSTOM_MODE",
            "REMOVE_CUSTOM_MODE_DISCRIPTION",
            "REMOVE",
        )
    };
    let danger = ProfileAlertColors::new().danger();
    let button_color = if reset { danger } else { cx.theme().button };
    let title = i18n::t(title_key).to_uppercase();
    v_flex()
        .id(frame_id)
        .test_support()
        .role(Role::Dialog)
        .aria_label(title.clone())
        .w(surface::css(300.))
        .p(surface::css(20.))
        .items_center()
        .rounded(surface::css(3.))
        .border_1()
        .border_color(if reset { danger } else { cx.theme().warning })
        .bg(cx.theme().popover)
        .shadow(vec![BoxShadow {
            color: cx.theme().title_bar.opacity(0.2),
            offset: point(Pixels::ZERO, cx.theme().font_size * (6. / 16.)),
            blur_radius: cx.theme().font_size * (10. / 16.),
            spread_radius: Pixels::ZERO,
            inset: false,
        }])
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .text_color(cx.theme().foreground)
        .text_center()
        .child(
            div()
                .w_full()
                .mb(surface::css(10.))
                // Fs renders del-title-normal. The orange override only
                // targets del-title, so both actual source titles stay red.
                .text_color(danger)
                .font_weight(FontWeight::NORMAL)
                .whitespace_normal()
                .child(title),
        )
        .child(
            div()
                .w_full()
                .mb(surface::css(10.))
                .whitespace_normal()
                .child(i18n::t(message_key)),
        )
        .child(
            Button::new(button_id)
                .label(i18n::t(button_key))
                .track_focus(&focus)
                .h(surface::css(27.))
                .min_w(surface::css(90.))
                .px(surface::css(5.))
                .py(surface::css(4.))
                .text_size(surface::css(12.))
                .line_height(surface::css(14.))
                .rounded(cx.theme().font_size * (3. / 16.))
                .border_1()
                .border_color(cx.theme().title_bar.opacity(0.3))
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(button_color)
                        .foreground(cx.theme().primary_foreground)
                        .hover(button_color.opacity(0.8))
                        .active(button_color.opacity(0.6)),
                )
                .on_click(move |_, window, cx| {
                    let owner = workspace.upgrade().filter(|workspace| {
                        let value = workspace.read(cx);
                        value.keyboard_controls.dial_confirmation.as_ref() == Some(&confirmation)
                            && confirmation.current(value)
                    });
                    _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                    if let Some(owner) = owner {
                        owner.update(cx, |workspace, cx| {
                            let next = match &confirmation.action {
                                DialConfirmationAction::Delete(uid) => {
                                    Continue::DeleteDial(uid.clone())
                                }
                                DialConfirmationAction::Reset => Continue::ResetDial,
                            };
                            workspace.continue_with(next, window, cx);
                        });
                    }
                }),
        )
}

#[cfg(test)]
#[path = "keyboard_controls_tests.rs"]
mod tests;

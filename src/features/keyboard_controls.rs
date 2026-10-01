//! 653 Tm/Em command modes and Ym Snap Tap capture. Profile data stays in
//! Keyboard; this owner retains only native controls and an uncommitted pair.
use super::{
    controls::{Choice, Choices},
    settings::{DialMode, canonical_snap_key},
    workspace::{Continue, DeviceWorkspace},
};
use crate::ui::surface::{self, SynapseSwitch};
use gpui_kit::base::{Popover, TestSupportExt as _};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    input::{Input, InputEvent, InputState},
    radio::Radio,
    select::{Select, SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

const KEY_NAMES: &[(&str, &str, &str)] = include!("mapping_keys.rs");

struct SnapCapture {
    pair: usize,
    key: usize,
    keys: [String; 2],
    return_focus: Option<FocusHandle>,
}

pub(super) struct KeyboardControls {
    name: Entity<InputState>,
    name_uid: Option<String>,
    color: Entity<ColorPickerState>,
    color_uid: Option<String>,
    profile: String,
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
            color: cx.new(|cx| ColorPickerState::new(window, cx)),
            color_uid: None,
            profile: String::new(),
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
        self.keyboard_controls.subscriptions.push(cx.subscribe_in(
            &self.keyboard_controls.color,
            window,
            |this, _, event, window, cx| {
                let ColorPickerEvent::Change(Some(color)) = event else {
                    return;
                };
                let Some(uid) = this.keyboard_controls.color_uid.clone() else {
                    return;
                };
                let color = Rgba::from(*color);
                let color = [color.r, color.g, color.b].map(|value| (value * 255.).round() as u8);
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
        self.keyboard_controls.name_uid = uid.clone();
        if let Some(mode) = &highlighted {
            let name = mode.label();
            let input = &self.keyboard_controls.name;
            if (changed || !input.focus_handle(cx).is_focused(window))
                && input.read(cx).value().as_str() != name
            {
                input.update(cx, |input, cx| input.set_value(name, window, cx));
            }
            self.keyboard_controls.color.update(cx, |picker, cx| {
                if changed {
                    picker.set_open(false, cx);
                }
                picker.set_value(
                    rgb(u32::from_be_bytes([
                        0,
                        mode.color[0],
                        mode.color[1],
                        mode.color[2],
                    ])),
                    window,
                    cx,
                );
            });
        }
        self.keyboard_controls.color_uid = uid;
    }

    fn commit_dial_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        surface::panel("Snap Tap", cx)
            .id("snap-tap-panel")
            .test_support()
            .track_focus(&self.keyboard_controls.snap_focus)
            .capture_key_down(
                cx.listener(|this, event, window, cx| this.capture_snap_key(event, window, cx)),
            )
            .child(
                SynapseSwitch::new("snap-tap-enabled")
                    .label("Snap Tap")
                    .checked(enabled)
                    .on_change(cx.listener(|this, value, window, cx| {
                        this.cancel_snap_capture(window, cx);
                        this.edit(window, cx, |settings| settings.keyboard.snap_tap = *value);
                    })),
            )
            .child(surface::note(
                "同时按住两个按键时，优先使用最后按下的按键。",
                cx,
            ))
            .children(pairs.iter().enumerate().map(|(pair_ix, pair)| {
                h_flex()
                    .gap(surface::css(10.))
                    .items_center()
                    .children(pair.iter().enumerate().map(|(key_ix, key)| {
                        let recording = capture.is_some_and(|capture| {
                            capture.pair == pair_ix && capture.key == key_ix
                        });
                        Button::new(SharedString::from(format!("snap-key-{pair_ix}-{key_ix}")))
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
                            .disabled(!enabled || capture.is_some())
                            .min_w(surface::css(64.))
                            .h(surface::css(44.))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.continue_with(
                                    Continue::SnapCapture {
                                        pair: pair_ix,
                                        key: key_ix,
                                    },
                                    window,
                                    cx,
                                );
                            }))
                    }))
                    .when(pair_ix > 0, |row| {
                        row.child(
                            Button::new(SharedString::from(format!("snap-delete-{pair_ix}")))
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
            }))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("snap-add-pair")
                            .outline()
                            .label("添加按键组…")
                            .disabled(!enabled || pairs.len() >= 4 || capture.is_some())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.continue_with(Continue::AddSnapPair, window, cx)
                            })),
                    )
                    .when(capture.is_some(), |row| {
                        row.child(
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
                        )
                    }),
            )
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
                        .child(surface::note(message, cx)),
                )
            })
            .into_any_element()
    }

    pub(super) fn command_dial_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let keyboard = &self.settings().keyboard;
        surface::panel_with_control(
            crate::i18n::t_or("COMMAND_DIAL", "命令旋钮"),
            Popover::new("dial-help-popover")
                .trigger_with(|_, _, cx| {
                    surface::asset_button(
                        "dial-help",
                        "synapse/help-default.svg",
                        "命令旋钮使用说明",
                        cx,
                    )
                    .into_any_element()
                })
                .content(|_, _, cx| {
                    v_flex()
                        .id("dial-help-content")
                        .test_support()
                        .w(surface::css(340.))
                        .p(surface::css(20.))
                        .gap(surface::css(14.))
                        .bg(cx.theme().popover)
                        .border_1()
                        .border_color(cx.theme().border)
                        .text_size(surface::css(14.))
                        .child(
                            img("synapse/keyboard-653-dial.png")
                                .size(surface::css(80.))
                                .self_center()
                                .object_fit(ObjectFit::Contain),
                        )
                        .child(crate::i18n::t("COMMAND_DIAL_USAGE_1"))
                        .child(crate::i18n::t("COMMAND_DIAL_USAGE_2"))
                        .child(crate::i18n::t("COMMAND_DIAL_USAGE_3"))
                        .child(
                            h_flex()
                                .gap(surface::css(10.))
                                .items_start()
                                .child(
                                    img("synapse/keyboard-653-dial-mapping.svg")
                                        .size(surface::css(16.))
                                        .flex_shrink_0(),
                                )
                                .child(
                                    div().flex_1().child(crate::i18n::t("COMMAND_DIAL_USAGE_4")),
                                ),
                        )
                        .into_any_element()
                }),
            cx,
        )
        .child(crate::i18n::t("SUB_CONTENT_COMMAND_DIAL"))
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("dial-add")
                        .outline()
                        .label("添加模式")
                        .disabled(
                            keyboard
                                .dial_modes
                                .iter()
                                .filter(|mode| mode.is_custom)
                                .count()
                                >= 100,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            let mut uid = None;
                            this.edit(window, cx, |settings| uid = settings.keyboard.add_dial());
                            if let Some(uid) = uid {
                                this.highlight_dial(uid, window, cx);
                            }
                        })),
                )
                .child(
                    Button::new("dial-reset")
                        .ghost()
                        .label("重置模式…")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.confirm_dial_reset(window, cx)),
                        ),
                ),
        )
        .child(surface::note(
            "勾选当前模式，点击名称查看设置。拖动或使用箭头调整顺序。",
            cx,
        ))
        .children(
            keyboard
                .dial_modes
                .iter()
                .enumerate()
                .map(|(ix, mode)| self.dial_mode_row(mode, ix, cx)),
        )
        .into_any_element()
    }

    fn dial_mode_row(&self, mode: &DialMode, ix: usize, cx: &mut Context<Self>) -> AnyElement {
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
        let count = self.settings().keyboard.dial_modes.len();
        let drop_uid = uid.clone();
        let mut row = v_flex()
            .id(SharedString::from(format!("dial-row-{uid}")))
            .test_support()
            .bg(if highlighted {
                cx.theme().group_box
            } else {
                cx.theme().transparent
            })
            .border_b_1()
            .border_color(cx.theme().border)
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
                    .child(
                        div()
                            .size(surface::css(12.))
                            .rounded_full()
                            .bg(color)
                            .flex_shrink_0(),
                    )
                    .child(
                        Button::new(SharedString::from(format!("dial-mode-{uid}")))
                            .ghost()
                            .label(mode.label())
                            .selected(highlighted)
                            .flex_1()
                            .min_w_0()
                            .on_click(cx.listener({
                                let uid = uid.clone();
                                move |this, _, window, cx| {
                                    this.highlight_dial(uid.clone(), window, cx)
                                }
                            })),
                    )
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
                        Button::new(SharedString::from(format!("dial-up-{uid}")))
                            .ghost()
                            .icon(gpui_kit::assets::IconName::ArrowUp)
                            .accessibility_label("上移模式")
                            .disabled(ix == 0)
                            .size(surface::css(24.))
                            .p_0()
                            .on_click(cx.listener({
                                let uid = uid.clone();
                                move |this, _, window, cx| {
                                    this.edit(window, cx, |settings| {
                                        settings.keyboard.move_dial(&uid, -1)
                                    })
                                }
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("dial-down-{uid}")))
                            .ghost()
                            .icon(gpui_kit::assets::IconName::ArrowDown)
                            .accessibility_label("下移模式")
                            .disabled(ix + 1 == count)
                            .size(surface::css(24.))
                            .p_0()
                            .on_click(cx.listener({
                                let uid = uid.clone();
                                move |this, _, window, cx| {
                                    this.edit(window, cx, |settings| {
                                        settings.keyboard.move_dial(&uid, 1)
                                    })
                                }
                            })),
                    )
                    .child(
                        gpui_kit::base::Button::new(SharedString::from(format!("dial-drag-{uid}")))
                            .accessibility_label("拖动排序模式")
                            .size(surface::css(24.))
                            .child("⠿")
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
        if highlighted {
            row = row.child(
                v_flex()
                    .px(surface::css(20.))
                    .py(surface::css(10.))
                    .gap_2()
                    .when(mode.is_custom, |detail| {
                        detail.child(
                            Input::new(&self.keyboard_controls.name)
                                .id("dial-name")
                                .disabled(!mode.enabled),
                        )
                    })
                    .child(
                        ColorPicker::new(&self.keyboard_controls.color)
                            .accessibility_label("模式指示灯颜色"),
                    )
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
                    })
                    .when(mode.is_custom, |detail| {
                        detail.child(
                            Button::new(SharedString::from(format!("dial-delete-{uid}")))
                                .ghost()
                                .label("删除模式…")
                                .on_click(cx.listener({
                                    let uid = uid.clone();
                                    move |this, _, window, cx| {
                                        this.confirm_dial_delete(uid.clone(), window, cx)
                                    }
                                })),
                        )
                    }),
            );
        }
        row.into_any_element()
    }

    fn confirm_dial_delete(&self, uid: String, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let entity = entity.clone();
            let uid = uid.clone();
            dialog
                .title("删除自定义模式？")
                .child("此模式的两个方向映射也会被删除。")
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("dial-delete-cancel")
                                .label("取消")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(Button::new("dial-delete-confirm").label("删除").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.continue_with(
                                        Continue::DeleteDial(uid.clone()),
                                        window,
                                        cx,
                                    )
                                });
                            },
                        )),
                )
        });
    }

    fn confirm_dial_reset(&self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let entity = entity.clone();
            dialog
                .title("重置 Command Dial？")
                .child("恢复八个预设模式，并移除自定义模式。")
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("dial-reset-cancel")
                                .label("取消")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(Button::new("dial-reset-confirm").label("重置").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.continue_with(Continue::ResetDial, window, cx)
                                });
                            },
                        )),
                )
        });
    }
}

#[cfg(test)]
#[path = "keyboard_controls_tests.rs"]
mod tests;

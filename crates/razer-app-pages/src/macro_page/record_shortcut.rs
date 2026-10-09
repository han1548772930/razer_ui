//! Current 58190 Lr -> Rr browser-keycode branch. No global registration or
//! Razer input redirect: this captures only the focused native Macro window.
use super::keyboard::{RawKey, shortcut_name};
use super::keyboard_windows::Capture;
use super::*;
use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};

#[derive(Deserialize)]
struct Key {
    #[serde(rename = "inputID")]
    input_id: String,
    #[serde(rename = "keyCode")]
    code: String,
}
#[derive(Deserialize)]
struct Data {
    keys: Vec<Key>,
    modifiers: Vec<String>,
}
fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("record_options_data.json"))
            .expect("current ordered Macro shortcut keys")
    })
}

pub struct ShortcutUi {
    focus: FocusHandle,
    generation: u64,
    capture: Option<Capture>,
    listening: bool,
    selected: Vec<String>,
    // JS Map preserves first insertion order; a sorted map would reorder the
    // shortcut. Browser keyCodes intentionally select the first matching row.
    pressed_modifiers: Vec<String>,
    released_modifiers: HashSet<String>,
    combination_key: Option<String>,
    hovered: bool,
}
impl ShortcutUi {
    pub fn new(focus: FocusHandle) -> Self {
        Self {
            focus,
            generation: 0,
            capture: None,
            listening: false,
            selected: Vec::new(),
            pressed_modifiers: Vec::new(),
            released_modifiers: HashSet::new(),
            combination_key: None,
            hovered: false,
        }
    }
    pub fn stop(&mut self) {
        self.capture = None; // Drops the current-thread/window hook immediately.
        self.listening = false;
        self.generation = self.generation.wrapping_add(1);
        self.clear_pending();
    }
    fn clear_pending(&mut self) {
        self.pressed_modifiers.clear();
        self.released_modifiers.clear();
        self.combination_key = None;
    }
    pub fn reset(&mut self) {
        // Rr's remove handler only clears inputIds; it neither exits capture nor
        // discards currently pressed modifiers.
        self.selected.clear();
    }
    pub fn display(&self) -> String {
        self.selected
            .iter()
            .map(|key| shortcut_name(key))
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join(" + ")
    }
    fn start(&mut self, window: &mut Window, cx: &mut App) -> bool {
        if self.listening || !window.is_window_active() {
            return false;
        }
        let _ = data(); // Parse before installing the native callback.
        let Some(capture) = Capture::start_stream(window) else {
            return false;
        };
        self.pressed_modifiers.clear();
        self.released_modifiers.clear();
        self.combination_key = None;
        self.generation = self.generation.wrapping_add(1);
        self.capture = Some(capture);
        self.listening = true;
        self.focus.focus(window, cx);
        true
    }
    fn poll(&mut self, window: &Window) -> bool {
        if !self.listening {
            return false;
        }
        if !window.is_window_active() {
            // Rr keeps `s` (listening) while window blur clears `l` (active).
            // Release the local native hook while inactive; discard incomplete
            // modifiers, since their releases may occur in another window.
            if self.capture.take().is_some() {
                self.clear_pending();
                return true;
            }
            return false;
        }
        if !self.focus.is_focused(window) {
            self.stop();
            return true;
        }
        if self.capture.is_none() {
            let Some(capture) = Capture::start_stream(window) else {
                self.stop();
                return true;
            };
            self.capture = Some(capture);
        }
        let Some(events) = self.capture.as_mut().and_then(Capture::take_all) else {
            // Overflow cannot commit a partial combination or a lost release.
            self.stop();
            return true;
        };
        let previous = self.selected.clone();
        for event in events {
            self.accept(event);
        }
        previous != self.selected
    }
    fn accept(&mut self, event: RawKey) {
        // Match Rr L/v, including its asymmetric 92 -> 91 keydown coercion.
        // Rr's browser path does not inspect scanCode or key location.
        let code = if event.down && event.code == 92 {
            91
        } else {
            event.code
        };
        let Some(key) = data()
            .keys
            .iter()
            .find(|key| key.code.parse::<u16>().ok() == Some(code))
        else {
            return;
        };
        let id = key.input_id.as_str();
        if matches!(id, "KEY_F12" | "KEY_LEFT_GUI") {
            return;
        }
        let modifier = data().modifiers.iter().any(|key| key == id);
        if event.down {
            if modifier && !self.pressed_modifiers.iter().any(|key| key == id) {
                self.pressed_modifiers.push(id.to_owned());
            }
        } else if modifier {
            self.released_modifiers.insert(id.to_owned());
            if !self.pressed_modifiers.is_empty()
                && self.pressed_modifiers.len() == self.released_modifiers.len()
            {
                if self.combination_key.take().is_none() {
                    self.selected = self.pressed_modifiers.clone();
                }
                self.pressed_modifiers.clear();
                self.released_modifiers.clear();
            }
        } else if self.combination_key.is_none() {
            let mut selected = self.pressed_modifiers.clone();
            if !selected.is_empty() {
                self.combination_key = Some(id.to_owned());
            }
            selected.push(id.to_owned());
            self.selected = selected;
        }
    }
}

impl MacroPage {
    fn start_record_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.record_ui.open {
            return;
        }
        if !self.record_ui.shortcut.start(window, cx) {
            return;
        }
        let generation = self.record_ui.shortcut.generation;
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let keep = this
                    .update_in(cx, |this, window, cx| {
                        if !this.record_ui.open
                            || this.record_ui.shortcut.generation != generation
                            || !this.record_ui.shortcut.listening
                        {
                            return false;
                        }
                        if this.record_ui.shortcut.poll(window) {
                            cx.notify();
                        }
                        this.record_ui.shortcut.listening
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    pub fn record_shortcut_control(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ui = &self.record_ui.shortcut;
        let listening = ui.listening;
        let empty = ui.selected.is_empty();
        let border: Hsla = motion::transition(
            "macro-record-shortcut-border",
            rgb(if listening || ui.hovered {
                0x44d62c
            } else {
                0x5d5d5d
            })
            .into(),
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        div()
            .id("macro-record-shortcut")
            .test_support()
            .aria_label(format!(
                "本地录制快捷键：{}；{}",
                if empty {
                    tr("TEXT_SHORTCUT_CONTENT")
                } else {
                    ui.display()
                },
                if listening {
                    "正在捕获"
                } else {
                    "未捕获"
                }
            ))
            .relative()
            .w_full()
            .min_h(css(27.))
            .mt(css(5.))
            .p(css(5.))
            .border_1()
            .border_color(border)
            .cursor_pointer()
            .text_size(css(13.5))
            .line_height(css(17.))
            .text_color(rgb(0xcccccc))
            .track_focus(&ui.focus)
            .on_hover(cx.listener(|this, value, _, cx| {
                this.record_ui.shortcut.hovered = *value;
                cx.notify();
            }))
            .on_click(cx.listener(|this, _, window, cx| this.start_record_shortcut(window, cx)))
            // Source listens for window click, not the enclosing menu's
            // document mousedown. Mouse-up outside is the native click exit.
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.record_ui.shortcut.listening {
                        this.record_ui.shortcut.stop();
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .when(empty, |text| text.text_color(rgb(0x707070)))
                    .child(if empty {
                        tr("TEXT_SHORTCUT_CONTENT")
                    } else {
                        ui.display()
                    }),
            )
            .when(!empty, |field| {
                field.child(
                    div()
                        .id("macro-record-shortcut-clear")
                        .test_support()
                        .aria_label("清除本地录制快捷键")
                        .absolute()
                        .right_0()
                        .top_0()
                        .size(css(25.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(img("synapse/shortcuts-search-clear.svg").size(css(20.)))
                        .on_mouse_down(MouseButton::Left, |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.record_ui.shortcut.reset();
                            cx.stop_propagation();
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }
}

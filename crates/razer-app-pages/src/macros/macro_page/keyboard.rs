//! Current Macro 58190 Nn/yn/pn and 25572 O/C; see the keyboard source audit.
use super::*;
use razer_pages::features::macro_library::KeyboardEvent;
use serde::Deserialize;
use std::{collections::HashMap, sync::OnceLock, time::Instant};

#[derive(Deserialize)]
struct Key {
    name: String,
    #[serde(rename = "inputID")]
    input_id: Option<String>,
    #[serde(rename = "keyCode")]
    code: String,
    #[serde(rename = "virtualKey")]
    virtual_key: Option<String>,
    #[serde(rename = "type")]
    key_type: String,
    flag: u8,
    #[serde(rename = "outputFlag")]
    output_flag: Option<u8>,
}
impl Key {
    fn flag(&self) -> u8 {
        // yn uses JS `outputFlag || flag`, including right Shift's zero.
        self.output_flag
            .filter(|flag| *flag != 0)
            .unwrap_or(self.flag)
    }
}
#[derive(Deserialize)]
struct Data {
    keys: Vec<Key>,
    names: KeyNames,
    extended: HashMap<String, String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum KeyName {
    Text(String),
    ByDevice(KeyNameByDevice),
}
#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
struct KeyNameByDevice {
    keyboard: String,
    mouse: String,
    default: String,
}
impl KeyName {
    fn default_text(&self) -> &str {
        match self {
            Self::Text(value) => value,
            Self::ByDevice(value) => &value.default,
        }
    }
}
type KeyNames = HashMap<String, HashMap<String, KeyName>>;
fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_catalog())
}

pub(super) fn recording_shortcut_key(input: &str) -> Result<recording_decode::ShortcutKey, String> {
    let key = data()
        .keys
        .iter()
        .find(|key| key.input_id.as_deref() == Some(input))
        .ok_or_else(|| format!("Unknown macro shortcut key: {input}"))?;
    // Current 62141.S supplies native release identities; Ao/Nr retain the
    // recorder table's keyCode for post-recording chord removal.
    let virtual_key = match input {
        "KEY_LEFT_ALT" => 164,
        "KEY_LEFT_CTRL" => 162,
        "KEY_LEFT_SHIFT" => 160,
        "KEY_RIGHT_ALT" => 165,
        "KEY_RIGHT_CTRL" => 163,
        "KEY_RIGHT_SHIFT" => 258,
        _ => key
            .virtual_key
            .as_deref()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| format!("Macro shortcut has no native key: {input}"))?,
    };
    Ok(recording_decode::ShortcutKey {
        virtual_key,
        key_code: key
            .code
            .parse()
            .map_err(|_| format!("Invalid macro shortcut keyCode: {input}"))?,
    })
}

pub use razer_widgets::keyboard_capture::RawKey;

pub(super) fn shortcut_capture_input(event: &RawKey) -> Option<&'static str> {
    let code = if event.down() && event.code() == 92 {
        91
    } else {
        event.code()
    };
    let right_location = match event.code() {
        16 => event.scan() == 0x36,
        17 | 18 => event.extended(),
        92 => true,
        _ => false,
    };
    // Ss selects the flag-2 table row for DOM_KEY_LOCATION_RIGHT on both
    // keyboard edges; the Windows message retains the required physical bits.
    data()
        .keys
        .iter()
        .find(|key| {
            key.code.parse::<u16>().ok() == Some(code) && (!right_location || key.flag == 2)
        })
        .and_then(|key| key.input_id.as_deref())
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn physical_location_selects_left_and_right_modifiers_on_both_edges() {
        for down in [true, false] {
            for (code, scan, extended, expected) in [
                (16, 0x2a, false, "KEY_LEFT_SHIFT"),
                (16, 0x36, false, "KEY_RIGHT_SHIFT"),
                (17, 0x1d, false, "KEY_LEFT_CTRL"),
                (17, 0x1d, true, "KEY_RIGHT_CTRL"),
                (18, 0x38, false, "KEY_LEFT_ALT"),
                (18, 0x38, true, "KEY_RIGHT_ALT"),
                (65, 0x1e, false, "KEY_A"),
            ] {
                let event = RawKey::new(code, scan, extended, down, Instant::now());
                assert_eq!(shortcut_capture_input(&event), Some(expected));
            }
        }
    }
}

fn captured_key(event: &RawKey) -> Option<&'static Key> {
    let code = if event.down() && event.code() == 92 {
        91
    } else {
        event.code()
    };
    let input_id = if code == 13 && !event.extended() {
        Some("KEY_ENTER")
    } else if code == 16 && event.scan() == 0x36 {
        Some("KEY_RIGHT_SHIFT")
    } else {
        None
    };
    data()
        .keys
        .iter()
        .find(|key| match input_id {
            Some(id) => key.input_id.as_deref() == Some(id),
            None => key.code.parse::<u16>().ok() == Some(code),
        })
        .filter(|key| key.input_id.is_some())
}
pub fn accepts(event: &RawKey) -> bool {
    captured_key(event).is_some()
}

fn resolved_key(event: &KeyboardEvent) -> Option<&'static Key> {
    let code = event.makecode.filter(|code| *code != 0)?;
    let flag = event.state.unwrap_or(0) / 2 * 2;
    let matches = |key: &&Key| {
        (key.code.parse::<u16>().ok() == Some(code)
            || key
                .virtual_key
                .as_deref()
                .and_then(|s| s.parse::<u16>().ok())
                == Some(code))
            && key.flag() == flag
    };
    data()
        .keys
        .iter()
        .find(|key| matches(key) && event.key_type.as_deref() == Some(key.key_type.as_str()))
        .or_else(|| data().keys.iter().find(matches))
        .or_else(|| {
            let id = data().extended.get(&code.to_string())?;
            data()
                .keys
                .iter()
                .find(|key| key.input_id.as_ref() == Some(id))
        })
}

fn display(key: &Key, editing: bool) -> String {
    let Some(id) = key.input_id.as_deref() else {
        return key.name.clone();
    };
    // The Razer systemKeyboardLayout service is not connected. yn explicitly
    // falls back to UnitedStates; pn passes undefined and F uses default.
    let table = &data().names[if editing { "default" } else { "1" }];
    let value = table.get(id).or_else(|| data().names["default"].get(id));
    value.map(KeyName::default_text).unwrap_or(id).to_string()
}

/// ShortcutKey Rr calls the same layout-name resolver. An unobserved system
/// keyboard layout uses its source default table rather than guessing a layout.
pub fn shortcut_name(input_id: &str) -> String {
    data().names["default"]
        .get(input_id)
        .map(KeyName::default_text)
        .unwrap_or(input_id)
        .to_owned()
}

struct Session {
    document: u64,
    index: usize,
    baseline: ActionItem,
    selected: Option<&'static Key>,
    pending_at: Option<Instant>,
    capture: super::keyboard_windows::Capture,
}
pub struct KeyboardUi {
    focus: FocusHandle,
    generation: u64,
    session: Option<Session>,
    task: Option<Task<()>>,
}
impl KeyboardUi {
    pub fn new(focus: FocusHandle) -> Self {
        Self {
            focus,
            generation: 0,
            session: None,
            task: None,
        }
    }
}

impl MacroPage {
    /// yn's editable key input disables the host's global shortcuts until its
    /// blur/exit path, independently of the recording state machine.
    pub fn keyboard_shortcut_capture_active(&self) -> bool {
        self.keyboard_ui.session.is_some()
    }

    pub fn open_keyboard_editor(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_pending_edits(window, cx);
        let Some(document) = self.current.filter(|id| self.actions_for == Some(*id)) else {
            return;
        };
        let Some(baseline) = self
            .actions
            .get(index)
            .filter(|item| item.kind == ActionKind::Keyboard)
            .cloned()
        else {
            return;
        };
        // Materialize the fixed key table before installing the native callback.
        let _ = data();
        let Some(capture) = super::keyboard_windows::Capture::start(window) else {
            return;
        };
        self.choice_action = None;
        self.keyboard_ui.generation = self.keyboard_ui.generation.wrapping_add(1);
        let generation = self.keyboard_ui.generation;
        self.keyboard_ui.session = Some(Session {
            document,
            index,
            baseline,
            selected: None,
            pending_at: None,
            capture,
        });
        self.keyboard_ui.focus.focus(window, cx);
        self.keyboard_ui.task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let keep = this
                    .update_in(cx, |this, window, cx| {
                        if this.keyboard_ui.generation != generation
                            || this.keyboard_ui.session.is_none()
                        {
                            return false;
                        }
                        if !this.keyboard_ui.focus.is_focused(window) {
                            this.finish_keyboard_editor();
                            cx.notify();
                            return false;
                        }
                        let changed = this.poll_keyboard_capture();
                        let due = this
                            .keyboard_ui
                            .session
                            .as_ref()
                            .and_then(|s| s.pending_at)
                            .is_some_and(|at| at.elapsed() >= Duration::from_millis(150));
                        if due {
                            this.commit_keyboard_capture();
                        }
                        if changed || due {
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn poll_keyboard_capture(&mut self) -> bool {
        let Some(session) = &mut self.keyboard_ui.session else {
            return false;
        };
        let Some(event) = session.capture.take() else {
            return false;
        };
        let Some(key) = captured_key(&event) else {
            return false;
        };
        session.selected = Some(key);
        session.pending_at = Some(event.at());
        true
    }

    fn commit_keyboard_capture(&mut self) {
        let Some(session) = &mut self.keyboard_ui.session else {
            return;
        };
        if session.pending_at.take().is_none() {
            return;
        }
        if self.current != Some(session.document)
            || self.actions_for != self.current
            || self.actions.get(session.index) != Some(&session.baseline)
        {
            return;
        }
        let Some(key) = session.selected else { return };
        let mut event = session.baseline.keyboard.clone().unwrap_or(KeyboardEvent {
            pair_id: None,
            makecode: None,
            state: None,
            flag: None,
            key_type: None,
        });
        let partner = event
            .pair_id
            .filter(|_| event.state.is_some())
            .and_then(|id| {
                self.actions.iter().enumerate().position(|(i, item)| {
                    i != session.index
                        && item.kind == ActionKind::Keyboard
                        && item
                            .keyboard
                            .as_ref()
                            .is_some_and(|key| key.pair_id == Some(id))
                })
            });
        event.makecode = key.code.parse().ok();
        event.key_type = Some(key.key_type.clone());
        // 25572.C leaves null-state Sequence and orphaned row flags untouched.
        if partner.is_some() {
            let flag = key.flag() + event.flag.unwrap_or(0) % 2;
            event.flag = Some(flag);
            event.state = Some(flag);
        }
        let mut next = self.actions.clone();
        next[session.index].value.clear();
        next[session.index].keyboard = Some(event.clone());
        if let Some(partner) = partner {
            let other = next[partner]
                .keyboard
                .as_mut()
                .expect("matched keyboard partner");
            other.makecode = event.makecode;
            other.key_type = event.key_type.clone();
            other.state = event.state.map(|flag| flag ^ 1);
            other.flag = other.state;
            next[partner].value.clear();
        }
        if next != self.actions {
            self.actions_dirty = true;
            if let Some(execution) = &mut self.execution {
                execution.invalidate();
            }
            self.undo.push(std::mem::replace(&mut self.actions, next));
            self.redo.clear();
        }
        session.baseline = self.actions[session.index].clone();
        self.rebuild_action_execution();
    }

    pub fn finish_keyboard_editor(&mut self) {
        self.poll_keyboard_capture();
        self.commit_keyboard_capture();
        self.cancel_keyboard_editor();
    }

    pub(super) fn cancel_keyboard_editor(&mut self) {
        self.keyboard_ui.session = None; // RAII unhooks on every exit path.
        self.keyboard_ui.task = None;
        self.keyboard_ui.generation = self.keyboard_ui.generation.wrapping_add(1);
    }
    pub fn keyboard_pending(&self) -> bool {
        self.keyboard_ui
            .session
            .as_ref()
            .is_some_and(|s| s.pending_at.is_some())
    }

    pub fn keyboard_value_editor(
        &self,
        index: usize,
        item: &ActionItem,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let session = self.keyboard_ui.session.as_ref();
        let editing = session.is_some_and(|s| s.index == index);
        let pending = session
            .filter(|s| {
                s.index == index
                    || s.baseline
                        .keyboard
                        .as_ref()
                        .and_then(|k| k.pair_id)
                        .is_some_and(|id| {
                            item.keyboard
                                .as_ref()
                                .is_some_and(|key| key.pair_id == Some(id))
                        })
            })
            .and_then(|s| s.selected);
        let key = pending.or_else(|| item.keyboard.as_ref().and_then(resolved_key));
        let label = key.map(|key| display(key, editing)).unwrap_or_else(|| {
            if editing {
                String::new()
            } else if let Some(code) = item
                .keyboard
                .as_ref()
                .and_then(|k| k.makecode)
                .filter(|code| *code != 0)
            {
                code.to_string()
            } else if !item.value.is_empty() && item.value != "TEXT_NO_KEY_SET" {
                item.value.clone()
            } else {
                tr("TEXT_NO_KEY_SET")
            }
        });
        if editing {
            let capture = div()
                .id(("macro-keyboard-capture", index))
                .relative()
                .w(css(160.))
                .min_h(css(27.))
                .p(css(5.))
                .bg(rgb(0x111111))
                .border_1()
                .border_color(rgb(0x44d62c))
                .text_size(css(13.5))
                .line_height(css(17.))
                .text_color(rgb(0xcccccc))
                .cursor_pointer()
                .track_focus(&self.keyboard_ui.focus)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| cx.stop_propagation())
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.finish_keyboard_editor();
                    cx.notify();
                }))
                .child(label)
                .when(
                    key.and_then(|key| key.input_id.as_ref()).is_some(),
                    |editor| {
                        editor.child(
                            div()
                                .id(("macro-keyboard-clear", index))
                                .flex()
                                .items_center()
                                .justify_center()
                                .absolute()
                                .right_0()
                                .top_0()
                                .size(css(25.))
                                .p_0()
                                .child(img("synapse/shortcuts-search-clear.svg").size(css(20.)))
                                // Source yn throws before changing state on empty ID.
                                // Preserve the no-change result without a native panic.
                                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                                    window.prevent_default();
                                    cx.stop_propagation();
                                })
                                .on_click(|_, _, cx| cx.stop_propagation()),
                        )
                    },
                );
            // InputKey_kbf fixes the wrapper to 160 x 27; its >div override
            // removes the otherwise inherited 5px top margin from the capture.
            return div()
                .w(css(160.))
                .h(css(27.))
                .bg(rgb(0x111111))
                .child(capture)
                .into_any_element();
        }
        div()
            .id(("macro-keyboard-label", index))
            .text_size(css(14.))
            .line_height(css(17.))
            .text_color(rgb(if key.and_then(|k| k.input_id.as_ref()).is_some() {
                0xcccccc
            } else {
                0x707070
            }))
            .hover(|style| style.text_color(rgb(0x44d62c)))
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.open_keyboard_editor(index, window, cx);
            }))
            .into_any_element()
    }
}

// Generated catalog reader: razer-app-pages/keyboard_data.
fn read_keyboard_catalog() -> Data {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Data> {
        let value = match r.tag()? {
            0 => Data {
                keys: r.child(decode_1)?,
                names: r.child(decode_7)?,
                extended: r.child(decode_11)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Key>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Key> {
        let value = match r.tag()? {
            0 => Key {
                name: r.child(decode_3)?,
                input_id: r.child(decode_4)?,
                code: r.child(decode_3)?,
                virtual_key: r.child(decode_4)?,
                key_type: r.child(decode_3)?,
                flag: r.child(decode_5)?,
                output_flag: None,
            },
            1 => Key {
                name: r.child(decode_3)?,
                input_id: r.child(decode_4)?,
                code: r.child(decode_3)?,
                virtual_key: r.child(decode_4)?,
                key_type: r.child(decode_3)?,
                flag: r.child(decode_5)?,
                output_flag: r.child(decode_6)?,
            },
            2 => Key {
                name: r.child(decode_3)?,
                input_id: None,
                code: r.child(decode_3)?,
                virtual_key: r.child(decode_4)?,
                key_type: r.child(decode_3)?,
                flag: r.child(decode_5)?,
                output_flag: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_3)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u8> {
        let value = match r.tag()? {
            0 => r.u8()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u8>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_5)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::HashMap<String, std::collections::HashMap<String, KeyName>>>
    {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_8)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::HashMap<String, KeyName>> {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_9)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<KeyName> {
        let value = match r.tag()? {
            0 => KeyName::Text(r.child(decode_3)?),
            1 => KeyName::ByDevice(r.child(decode_10)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<KeyNameByDevice> {
        let value = match r.tag()? {
            0 => KeyNameByDevice {
                keyboard: r.child(decode_3)?,
                mouse: r.child(decode_3)?,
                default: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_APP_PAGES_KEYBOARD_DATA,
        0,
        "fb7d8829ec647cda4c12fe55bc5e4484748a718e46bd6339302ced844a04571d",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_data_decodes() {
        let _ = super::read_keyboard_catalog();
    }
}
// End generated catalog reader: razer-app-pages/keyboard_data.

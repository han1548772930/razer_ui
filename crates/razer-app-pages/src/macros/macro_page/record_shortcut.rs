//! Current 58190 Lr -> Rr focused browser capture and 25572.Y -> br global
//! registration. Native registration and event polling belong to the service.
use super::keyboard::{RawKey, shortcut_name};
use super::keyboard_windows::Capture;
use super::*;
use razer_service::mapping::hotkeys::{self, Watch, WatchUpdate};
use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};

const RECORD_ARGUMENT: &str = "macro_record_shortcut";

fn modifier_bit(input: &str) -> Option<u32> {
    match input {
        "KEY_RIGHT_ALT" => Some(16),
        "KEY_RIGHT_CTRL" => Some(32),
        "KEY_RIGHT_SHIFT" => Some(64),
        "KEY_LEFT_ALT" => Some(256),
        "KEY_LEFT_CTRL" => Some(512),
        "KEY_LEFT_SHIFT" => Some(1024),
        "KEY_LEFT_GUI" => Some(2048),
        _ => None,
    }
}
fn registration(selected: &[String]) -> Result<Option<hotkeys::Shortcut>, String> {
    let Some(input) = selected
        .iter()
        .find(|input| modifier_bit(input).is_none())
        .or_else(|| selected.last())
    else {
        return Ok(None);
    };
    let virtual_key = data()
        .keys
        .iter()
        .find(|key| key.input_id == *input)
        .and_then(|key| key.code.parse().ok())
        .ok_or_else(|| format!("Unknown macro shortcut: {input}"))?;
    let modifiers = selected
        .iter()
        .filter_map(|input| modifier_bit(input))
        .fold(0, |bits, value| bits | value);
    Ok(Some(hotkeys::Shortcut {
        virtual_key,
        modifiers,
        argument: RECORD_ARGUMENT.into(),
    }))
}

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
    DATA.get_or_init(|| read_record_shortcut_catalog())
}

pub struct ShortcutUi {
    focus: FocusHandle,
    generation: u64,
    capture: Option<Capture>,
    listening: bool,
    pub(super) selected: Vec<String>,
    // JS Map preserves the first insertion order of each physical modifier.
    pressed_modifiers: Vec<String>,
    released_modifiers: HashSet<String>,
    combination_key: Option<String>,
    hovered: bool,
    task: Option<Task<()>>,
    watch: Option<Watch>,
    watch_task: Option<Task<()>>,
    watch_generation: u64,
    watch_active: bool,
    watch_closing: bool,
    registered: Option<hotkeys::Shortcut>,
    pub(super) registration_error: Option<String>,
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
            task: None,
            watch: None,
            watch_task: None,
            watch_generation: 0,
            watch_active: false,
            watch_closing: false,
            registered: None,
            registration_error: None,
        }
    }
    pub fn stop(&mut self) {
        self.capture = None; // Drops the current-thread/window hook immediately.
        self.task = None;
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
    pub fn recording_keys(&self) -> Result<Vec<recording_decode::ShortcutKey>, String> {
        self.selected
            .iter()
            .map(|input| super::keyboard::recording_shortcut_key(input))
            .collect()
    }
    pub fn global_shortcuts_suspended(&self) -> bool {
        self.listening && self.capture.is_some()
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
        let resumed = self.capture.is_none();
        if resumed {
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
        resumed || previous != self.selected
    }
    fn accept(&mut self, event: RawKey) {
        let Some(id) = super::keyboard::shortcut_capture_input(&event) else {
            return;
        };
        if !data().keys.iter().any(|key| key.input_id == id) {
            return;
        }
        if matches!(id, "KEY_F12" | "KEY_LEFT_GUI") {
            return;
        }
        let modifier = data().modifiers.iter().any(|key| key == id);
        if event.down() {
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
    pub fn resume_recording_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_record_shortcut(window, cx);
    }
    pub fn recording_shutdown_pending(&self) -> bool {
        self.recording_busy() || self.record_ui.shortcut.watch_closing
    }
    pub fn deactivate_recording(&mut self, cx: &mut Context<Self>) {
        let shortcut = &mut self.record_ui.shortcut;
        shortcut.stop();
        shortcut.watch_active = false;
        shortcut.watch_generation = shortcut.watch_generation.wrapping_add(1);
        shortcut.registered = None;
        if let Some(watch) = &shortcut.watch {
            shortcut.watch_closing = true;
            watch.close();
        }
        self.cancel_recording(cx);
    }
    pub(super) fn activate_record_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.record_ui.shortcut.watch_active = true;
        if self.record_ui.shortcut.watch.is_some() || self.record_ui.shortcut.selected.is_empty() {
            return;
        }
        let (watch, updates) = match Watch::start(Duration::from_millis(400)) {
            Ok(value) => value,
            Err(error) => {
                self.record_ui.shortcut.registration_error = Some(error);
                cx.notify();
                return;
            }
        };
        self.record_ui.shortcut.watch = Some(watch);
        self.update_record_shortcut();
        self.record_ui.shortcut.watch_task = Some(cx.spawn_in(window, async move |this, cx| {
            while let Ok(update) = updates.recv().await {
                let closed = matches!(update, WatchUpdate::Closed(_));
                if this
                    .update_in(cx, |this, window, cx| {
                        match update {
                            WatchUpdate::Registered { generation, result }
                                if this.record_ui.shortcut.watch_generation == generation =>
                            {
                                match result {
                                    Ok(shortcut) => {
                                        this.record_ui.shortcut.registered = shortcut;
                                        this.record_ui.shortcut.registration_error = None;
                                    }
                                    Err(error) => {
                                        this.record_ui.shortcut.registration_error = Some(error)
                                    }
                                }
                            }
                            WatchUpdate::Triggered { generation, event }
                                if this.record_ui.shortcut.watch_active
                                    && this.record_ui.shortcut.watch_generation == generation
                                    && this.record_ui.shortcut.registered.as_ref()
                                        == Some(&event.shortcut) =>
                            {
                                this.toggle_recording_from_shortcut(window, cx);
                            }
                            WatchUpdate::Closed(result) => {
                                this.record_ui.shortcut.watch = None;
                                this.record_ui.shortcut.registered = None;
                                let resume = this.record_ui.shortcut.watch_closing
                                    && this.record_ui.shortcut.watch_active;
                                this.record_ui.shortcut.watch_closing = false;
                                if let Err(error) = result {
                                    this.record_ui.shortcut.registration_error = Some(error);
                                } else if resume {
                                    this.activate_record_shortcut(window, cx);
                                }
                            }
                            _ => {}
                        }
                        cx.notify();
                    })
                    .is_err()
                    || closed
                {
                    return;
                }
            }
        }));
    }
    fn update_record_shortcut(&mut self) {
        let shortcut = &mut self.record_ui.shortcut;
        shortcut.watch_generation = shortcut.watch_generation.wrapping_add(1);
        shortcut.registered = None;
        if let Some(watch) = &shortcut.watch {
            if !shortcut.watch_closing {
                if let Err(error) = registration(&shortcut.selected)
                    .and_then(|value| watch.replace(shortcut.watch_generation, value))
                {
                    shortcut.registration_error = Some(error);
                }
            }
        }
    }
    pub(super) fn adopt_record_shortcut(
        &mut self,
        selected: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.record_ui.shortcut.selected == selected {
            return;
        }
        self.record_ui.shortcut.selected = selected;
        self.update_record_shortcut();
        if self.record_ui.shortcut.watch_active && self.record_ui.shortcut.watch.is_none() {
            self.activate_record_shortcut(window, cx);
        }
    }
    fn publish_record_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.publish_library(cx);
        if self.record_ui.shortcut.watch.is_none() {
            self.activate_record_shortcut(window, cx);
        } else {
            self.update_record_shortcut();
        }
    }
    /// Rr retains listening across blur but only suppresses shortcuts while
    /// its input capture is active; countdown/recording alone do not.
    pub fn record_shortcut_capture_active(&self) -> bool {
        self.record_ui.shortcut.global_shortcuts_suspended()
    }

    fn start_record_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.record_ui.open {
            return;
        }
        if !self.record_ui.shortcut.start(window, cx) {
            return;
        }
        let generation = self.record_ui.shortcut.generation;
        self.record_ui.shortcut.task = Some(cx.spawn_in(window, async move |this, cx| {
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
                        let selected = this.record_ui.shortcut.selected.clone();
                        if this.record_ui.shortcut.poll(window) {
                            if selected != this.record_ui.shortcut.selected {
                                this.publish_record_shortcut(window, cx);
                            }
                            cx.notify();
                        }
                        this.record_ui.shortcut.listening
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
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
                if listening { "正在捕获" } else { "未捕获" }
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
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.record_ui.shortcut.reset();
                            this.publish_record_shortcut(window, cx);
                            cx.stop_propagation();
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }
}

// Generated catalog reader: razer-app-pages/record_shortcut_data.
fn read_record_shortcut_catalog() -> Data {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Data> {
        let value = match r.tag()? {
            0 => Data {
                keys: r.child(decode_1)?,
                modifiers: r.child(decode_4)?,
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
                input_id: r.child(decode_3)?,
                code: r.child(decode_3)?,
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
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_APP_PAGES_RECORD_SHORTCUT_DATA,
        0,
        "39485c086ccf38241a7b1104314968d1fcc4ff5519fa06ed4b2655a4eeac2611",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod record_shortcut_data_catalog_tests {
    #[test]
    fn packaged_catalog_record_shortcut_data_decodes() {
        let _ = super::read_record_shortcut_catalog();
    }
}
// End generated catalog reader: razer-app-pages/record_shortcut_data.

#[cfg(test)]
mod registration_tests {
    use super::*;
    use std::prelude::v1::test;
    #[test]
    fn registration_keeps_browser_keycode_and_source_modifier_order() {
        let chord = registration(&[
            "KEY_LEFT_CTRL".into(),
            "KEY_RIGHT_ALT".into(),
            "KEY_A".into(),
        ])
        .unwrap()
        .unwrap();
        assert_eq!((chord.virtual_key, chord.modifiers), (65, 528));
        assert_eq!(chord.argument, "macro_record_shortcut");
        let chord = registration(&["KEY_RIGHT_CTRL".into(), "KEY_LEFT_SHIFT".into()])
            .unwrap()
            .unwrap();
        assert_eq!((chord.virtual_key, chord.modifiers), (16, 1056));
        assert!(registration(&[]).unwrap().is_none());
        assert!(registration(&["KEY_UNKNOWN".into()]).is_err());
    }
    #[test]
    fn native_release_and_postprocessing_use_different_key_domains() {
        let left = super::super::keyboard::recording_shortcut_key("KEY_LEFT_CTRL").unwrap();
        assert_eq!(
            left,
            recording_decode::ShortcutKey {
                virtual_key: 162,
                key_code: 17
            }
        );
        let right = super::super::keyboard::recording_shortcut_key("KEY_RIGHT_CTRL").unwrap();
        assert_eq!(
            right,
            recording_decode::ShortcutKey {
                virtual_key: 163,
                key_code: 17
            }
        );
    }
}

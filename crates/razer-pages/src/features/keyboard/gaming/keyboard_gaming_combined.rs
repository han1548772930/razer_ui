//! Current GameMode.useQ, its nine mounted products and fixed two-row recorder.
//! A local edit is independent of the ON_SET_GAME_MODE_COMBINATION_KEYS result.
use super::*;
use gpui_kit::base::Button as BaseButton;
use razer_widgets::keyboard_capture::Capture;
use razer_widgets::theme::KeyboardAnalogControlColors as Colors;
use serde::Serialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LOCAL: &str = "_gamingCombosLocalV1";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct Combo {
    id: u32,
    enabled: bool,
    key1: String,
    key2: String,
}

fn defaults() -> Vec<Combo> {
    [1, 2]
        .map(|id| Combo {
            id,
            enabled: false,
            key1: String::new(),
            key2: String::new(),
        })
        .into()
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Message {
    #[default]
    Introduction,
    Duplicate,
    DefaultShortcut,
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    First,
    Second,
}

pub(super) struct State {
    combos: Vec<Combo>,
    local: bool,
    target: Option<(u32, Part)>,
    message: Message,
    focus: Option<FocusHandle>,
    return_focus: Option<FocusHandle>,
    capture: Option<Capture>,
    native_capture: Option<razer_service::mapping::key_capture::Session>,
    task: Option<Task<()>>,
    generation: u64,
    resume_single_snap: bool,
    key_name_sleep: String,
    sleep_initialized: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            combos: defaults(),
            local: false,
            target: None,
            message: Message::Introduction,
            focus: None,
            return_focus: None,
            capture: None,
            native_capture: None,
            task: None,
            generation: 0,
            resume_single_snap: false,
            key_name_sleep: String::new(),
            sleep_initialized: false,
        }
    }
}

impl State {
    pub(super) fn cancel(&mut self) {
        self.capture = None;
        self.native_capture = None;
        self.task = None;
        self.generation = self.generation.wrapping_add(1);
        self.target = None;
        self.return_focus = None;
        self.resume_single_snap = false;
    }
}

fn normalized(key: &str) -> &str {
    match key {
        "KEY_LEFT_ALT" | "KEY_RIGHT_ALT" => "KEY_ALT",
        key => key,
    }
}

fn pair<'a>(key1: &'a str, key2: &'a str) -> Option<[&'a str; 2]> {
    if key1.is_empty() || key2.is_empty() {
        return None;
    }
    let mut pair = [normalized(key1), normalized(key2)];
    pair.sort_unstable();
    Some(pair)
}

fn validate(
    combos: &[Combo],
    id: u32,
    part: Part,
    key: &str,
    in_game: bool,
) -> Result<Option<Part>, Message> {
    if key == "KEY_APPLICATION" || key == "KEY_LEFT_GUI" && !in_game {
        return Ok(None);
    }
    let Some(combo) = combos.iter().find(|combo| combo.id == id) else {
        return Ok(None);
    };
    let part = if part == Part::Second && combo.key1.is_empty() {
        Part::First
    } else {
        part
    };
    if part == Part::First && combo.key2 == key || part == Part::Second && combo.key1 == key {
        return Err(Message::Duplicate);
    }
    let first = if part == Part::First { key } else { &combo.key1 };
    let second = if part == Part::Second { key } else { &combo.key2 };
    // Current default-shortcut guard is directional; do not add a symmetric
    // FN+F10 or Alt shortcut rule absent from the actual function body.
    if !in_game && (first == "KEY_LEFT_GUI" || second == "KEY_LEFT_GUI")
        || first == "KEY_APPLICATION"
        || second == "KEY_APPLICATION"
        || first == "KEY_FN" && second == "KEY_F10"
        || matches!(first, "KEY_LEFT_ALT" | "KEY_RIGHT_ALT")
            && matches!(second, "KEY_TAB" | "KEY_F4" | "KEY_SPACEBAR")
    {
        return Err(Message::DefaultShortcut);
    }
    if !first.is_empty()
        && second.is_empty()
        && combos.iter().any(|other| {
            other.id != id && normalized(&other.key1) == normalized(first) && other.key2.is_empty()
        })
    {
        return Err(Message::Duplicate);
    }
    if pair(first, second).is_some_and(|keys| {
        combos
            .iter()
            .any(|other| other.id != id && pair(&other.key1, &other.key2) == Some(keys))
    }) {
        return Err(Message::Duplicate);
    }
    Ok(Some(part))
}

impl KeyboardProductWorkspace {
    pub(super) fn gaming_fn_sleep_control(
        &self,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if self.spec.config["DeviceInfo"]["isComposeF4"] != true {
            return None;
        }
        let name = &self.gaming_combined.key_name_sleep;
        if name.is_empty() {
            return None;
        }
        Some(self.toggle(
            "/gamingMode/isFnSleepEnabled",
            t("DISABLE_FN_SLEEP").replace("{{keyName}}", name),
            enabled,
            cx,
        ))
    }

    pub(super) fn refresh_gaming_fn_sleep(&mut self, cx: &mut Context<Self>) {
        if self.spec.config["DeviceInfo"]["isComposeF4"] != true
            || self.page != "TAB_CUSTOMIZE"
            || self.customize_factory_root()
        {
            return;
        }
        let button = self
            .mapping_buttons()
            .iter()
            .map(|button| self.mapping_active_button(button))
            .find(|button| button["assignmentValue"] == "Sleep");
        let Some(button) = button else {
            return;
        };
        let key = self
            .interrupt_recording_targets()
            .iter()
            .find(|key| key["inputID"] == button["inputID"]);
        let Some(key) = key else {
            return;
        };
        let (Some(scan), Some(name)) = (
            key["scancode"].as_u64(),
            button["defaultValue"]
                .as_str()
                .or_else(|| key["name"].as_str()),
        ) else {
            return;
        };
        let name = name.to_owned();
        let post = self.gaming_combined.sleep_initialized;
        self.gaming_combined.sleep_initialized = true;
        self.gaming_combined.key_name_sleep = name;
        if self.draft["gamingMode"]["scancode"].as_u64() != Some(scan) {
            self.draft["gamingMode"]["scancode"] = json!(scan);
            if post {
                self.request_source_gaming_mode(cx);
            }
        }
        cx.notify();
    }

    pub(super) fn gaming_combos_snapshot(&self, snapshot: &mut Value) {
        if self.spec.custom_gaming_combinations() && self.gaming_combined.local {
            snapshot[LOCAL] = serde_json::to_value(&self.gaming_combined.combos).unwrap();
        }
    }

    pub(super) fn restore_gaming_combos(&mut self) {
        self.gaming_combined.cancel();
        self.gaming_combined.combos = defaults();
        self.gaming_combined.local = false;
        self.gaming_combined.message = Message::Introduction;
        if !self.spec.custom_gaming_combinations() {
            return;
        }
        let source = self
            .draft
            .get(LOCAL)
            .or_else(|| self.draft.pointer("/gamingMode/customGameMode/combos"))
            .or_else(|| self.draft.pointer("/gamingMode/combos"));
        if let Some(source) = source {
            match serde_json::from_value::<Vec<Combo>>(source.clone()) {
                Ok(combos) => {
                    self.gaming_combined.combos = combos;
                    self.gaming_combined.local = self.draft.get(LOCAL).is_some();
                }
                Err(error) => eprintln!(
                    "[keyboard-game-mode] Combo observation could not be decoded: {error}"
                ),
            }
        }
    }

    /// Current MW profile reducer supplies gamingMode/customGameMode/combos.
    /// This is an observation entrypoint, never a setter acknowledgement.
    pub fn observe_gaming_mode(&mut self, gaming_mode: Value, cx: &mut Context<Self>) {
        if !gaming_mode.is_object() {
            return;
        }
        let combos = gaming_mode
            .pointer("/customGameMode/combos")
            .or_else(|| gaming_mode.get("combos"));
        if let Some(combos) = combos {
            match serde_json::from_value::<Vec<Combo>>(combos.clone()) {
                Ok(combos) => self.gaming_combined.combos = combos,
                Err(error) => eprintln!(
                    "[keyboard-game-mode] Combo observation could not be decoded: {error}"
                ),
            }
        }
        self.draft["gamingMode"] = gaming_mode;
        cx.notify();
    }

    fn source_gaming_mode_payload(&self) -> Value {
        let gaming = &self.draft["gamingMode"];
        json!({
            "state":gaming["state"].as_u64().unwrap_or(0),
            "enableInGame":gaming["enableInGame"].as_bool().unwrap_or(false),
            "isWindowsKeyDisabled":gaming["isWindowsKeyDisabled"].as_bool().unwrap_or(false),
            "isAltTabDisabled":gaming["isAltTabDisabled"].as_bool().unwrap_or(false),
            "isAltF4Disabled":gaming["isAltF4Disabled"].as_bool().unwrap_or(false),
            "isAltSpaceDisabled":gaming["isAltSpaceDisabled"].as_bool().unwrap_or(false),
            "isGameOn":gaming["isGameOn"].as_bool().unwrap_or(false),
            "isFnSleepEnabled":gaming["isFnSleepEnabled"].as_bool().unwrap_or(false),
            "scancode":gaming["scancode"].as_u64().unwrap_or(0),
            "combos":self.gaming_combined.combos,
        })
    }

    pub(super) fn request_source_gaming_mode(&mut self, cx: &mut Context<Self>) {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.request_actuation_message(json!({"type":"ON_SET_GAME_MODE","timerTick":tick,
            "payload":{"MACRO_RECORDER_LOCK_NAME":"RzGlobalLock_macro_recorder","MACRO_RECORDER_WINDOW_STORAGE_NAME":"RzWinStorage_macro_recorder","gamingMode":self.source_gaming_mode_payload()}}),cx);
    }

    fn submit_gaming_combos(&mut self, cx: &mut Context<Self>) {
        self.gaming_combined.local = true;
        let payload = self.source_gaming_mode_payload();
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.request_actuation_message(
            json!({"type":"ON_SET_GAME_MODE_COMBINATION_KEYS","timerTick":tick,"payload":payload}),
            cx,
        );
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }

    pub(super) fn gaming_combined_keys(
        &self,
        enabled: bool,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let state = &self.gaming_combined;
        v_flex().id("keyboard-game-combined-keys")
            .when_some(state.focus.as_ref(), |panel,focus| panel.track_focus(focus))
            .gap(surface::css(15.)).opacity(if enabled { 1. } else { 0.3 })
            .child(div().mb(surface::css(5.)).child(t("COMBINED_KEY_TITLE")))
            .children(state.combos.iter().map(|combo| {
                let id = combo.id;
                let active = combo.enabled;
                let unavailable = disabled || !enabled;
                h_flex().items_center().gap(surface::css(16.))
                    .child(div().w(surface::css(60.)).flex_shrink_0().child(
                        Checkbox::new(SharedString::from(format!("keyboard-game-combo-enabled-{id}")))
                            .label(t("COMBINED_KEY").replace("{{index}}",&id.to_string()))
                            .checked(active).disabled(unavailable)
                            .on_change(cx.listener(move |this,value,window,cx| {
                                this.finish_gaming_recording(true,window,cx);
                                if let Some(combo) = this.gaming_combined.combos.iter_mut().find(|combo|combo.id == id) { combo.enabled = *value; }
                                this.submit_gaming_combos(cx);
                            }))))
                    .child(h_flex().w_full().items_center().gap(surface::css(10.)).ml(surface::css(85.))
                        .children([(Part::First,&combo.key1),(Part::Second,&combo.key2)].map(|(part,key)| {
                            let recording = state.target == Some((id,part));
                            let warning = recording && state.message == Message::Duplicate;
                            let color = if warning { Colors::warning() } else { Colors::accent() };
                            let label = if key.is_empty() { String::new() } else { self.interrupt_recording_key_name(key) };
                            let text = div().mx(surface::css(1.)).px(surface::css(if recording && label.is_empty() { 10. } else { 5. })).h(surface::css(20.)).rounded(surface::css(3.)).text_size(surface::css(12.)).child(label.clone());
                            let text = if recording {
                                text.with_animation(SharedString::from(format!("keyboard-game-combo-blink-{id}-{}-{}-{warning}", part as u8, state.generation)),Animation::new(Duration::from_secs(1)).repeat(),move |text,delta| {
                                    let opacity = (delta * 2. - 1.).abs();
                                    text.bg(color.opacity(opacity)).text_color(Colors::assignment_text().blend(Colors::selected_text().opacity(opacity)))
                                }).into_any_element()
                            } else { text.into_any_element() };
                            let button = BaseButton::new(SharedString::from(format!("keyboard-game-combo-key-{id}-{}",part as u8)))
                                .accessibility_label(format!("{} {}: {label}",t("COMBINED_KEY").replace("{{index}}",&id.to_string()),part as u8 + 1))
                                .disabled(unavailable || !active).min_w(surface::css(30.)).h(surface::css(30.)).border_1().border_color(if recording { color } else { Colors::record_border() }).rounded(surface::css(4.))
                                .text_color(Colors::assignment_text())
                                .when(!recording && !unavailable && active,|button|button.hover(|button|button.border_color(Colors::record_hover_border())))
                                .child(text).on_click(cx.listener(move |this,_,window,cx|this.start_gaming_recording(id,part,window,cx)));
                            if recording {
                                button.with_animation(SharedString::from(format!("keyboard-game-combo-border-blink-{id}-{}-{}-{warning}",part as u8,state.generation)),Animation::new(Duration::from_secs(1)).repeat(),move |button,delta| {
                                    button.border_color(Colors::record_border().blend(color.opacity((delta*2.-1.).abs())))
                                }).into_any_element()
                            } else { button.into_any_element() }
                        }))
                        .when(!combo.key1.is_empty() || !combo.key2.is_empty(), |pair|pair.child(
                            BaseButton::new(SharedString::from(format!("keyboard-game-combo-delete-{id}")))
                                .disabled(unavailable || !active).accessibility_label(t("DELETE_KEY"))
                                .size(surface::css(20.)).ml(surface::css(5.)).text_color(Colors::muted())
                                .hover(|button|button.text_color(Colors::combo_delete_hover()))
                                .child(razer_assets::resource_svg().path("synapse/keyboard-key-delete-active.svg").size_full())
                                .on_click(cx.listener(move |this,_,window,cx| {
                                    this.finish_gaming_recording(false,window,cx);
                                    if let Some(combo) = this.gaming_combined.combos.iter_mut().find(|combo|combo.id == id) { combo.key1.clear();combo.key2.clear();combo.enabled = false; }
                                    this.gaming_combined.message = Message::Introduction;
                                    this.submit_gaming_combos(cx);
                                })))))
                    .into_any_element()
            }))
            .when(state.message != Message::Introduction, |panel|panel.child(div().text_color(Colors::warning()).child(t("COMBINED_KEY_WARNING_MESSAGE"))))
            .on_mouse_down_out(cx.listener(|this,_,window,cx|this.finish_gaming_recording(true,window,cx)))
            .on_mouse_down(MouseButton::Left,cx.listener(|this,_,window,cx|this.finish_gaming_recording(true,window,cx)))
            .on_key_up(cx.listener(|this,event: &KeyUpEvent,window,cx| {
                if this.gaming_combined.target.is_none() { return; }
                window.prevent_default();cx.stop_propagation();
                if this.gaming_combined.capture.is_none() {
                    if let Some(key) = this.gpui_single_key_target(event.keystroke.key.as_str()) { this.accept_gaming_key(&key,window,cx); }
                    else { eprintln!("[keyboard-game-combo] This platform does not expose the physical key/location for {:?}",event.keystroke.key); }
                }
            }))
            .into_any_element()
    }

    fn start_gaming_recording(
        &mut self,
        id: u32,
        part: Part,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.gaming_combined.target.is_some()
            || self.customize_controls_disabled()
            || self.draft["gamingMode"]["state"].as_u64().unwrap_or(0) == 0
        {
            return;
        }
        let Some(combo) = self
            .gaming_combined
            .combos
            .iter()
            .find(|combo| combo.id == id && combo.enabled)
        else {
            return;
        };
        let part = if part == Part::Second && combo.key1.is_empty() {
            Part::First
        } else {
            part
        };
        self.stop_interrupt_recording(window, cx);
        let resume = self.suspend_single_snap_capture(cx);
        let state = &mut self.gaming_combined;
        state.target = Some((id, part));
        state.message = Message::Introduction;
        state.return_focus = window.focused(cx);
        let focus = state.focus.get_or_insert_with(|| cx.focus_handle());
        focus.focus(window, cx);
        state.resume_single_snap = resume;
        state.capture = Capture::start_stream(window);
        match razer_service::mapping::key_capture::Session::start() {
            Ok(owner) => state.native_capture = Some(owner),
            Err(error) => eprintln!("[keyboard-game-combo] Capture startup failed: {error:#}"),
        }
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        state.task = Some(cx.spawn_in(window,async move |this,cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let keep = this.update_in(cx,|this,window,cx| {
                    if this.gaming_combined.generation != generation || this.gaming_combined.target.is_none() { return false; }
                    if this.page != "TAB_CUSTOMIZE" {
                        this.finish_gaming_recording(false,window,cx);
                        return false;
                    }
                    if this.customize_controls_disabled() || this.draft["gamingMode"]["state"].as_u64().unwrap_or(0) == 0 {
                        this.finish_gaming_recording(true,window,cx);
                        return false;
                    }
                    if !window.is_window_active() || !this.gaming_combined.focus.as_ref().is_some_and(|focus|focus.is_focused(window)) {
                        this.finish_gaming_recording(false,window,cx);
                        return false;
                    }
                    let events = if let Some(capture) = this.gaming_combined.capture.as_mut() {
                        let Some(events) = capture.take_all() else {
                            eprintln!("[keyboard-game-combo] Input capture overflowed; canceled recording");
                            this.finish_gaming_recording(false,window,cx);
                            return false;
                        };
                        events
                    } else { Vec::new() };
                    for event in events {
                        if event.down() || this.gaming_combined.target.is_none() { continue; }
                        if let Some(key) = analog_controls::browser_target(this.interrupt_recording_targets(),&event) { this.accept_gaming_key(&key,window,cx); }
                    }
                    let events = this.gaming_combined.native_capture.as_ref().map(|owner|owner.drain()).unwrap_or_default();
                    for reply in events {
                        use razer_service::mapping::key_capture::Reply;
                        match reply {
                            Reply::Mode(_) => {},
                            Reply::Ready(receipt) => {
                                if !receipt.unresolved_devices().is_empty() { eprintln!("[keyboard-game-combo] Unresolved endpoints: {:?}",receipt.unresolved_devices()); }
                            }
                            Reply::Failed(error) => { eprintln!("[keyboard-game-combo] {error}");this.gaming_combined.native_capture = None; }
                            Reply::Events(events) => for event in events {
                                if this.gaming_combined.target.is_none() { break; }
                                let input = &event["input"];
                                let parsed = input.as_str().and_then(|value|serde_json::from_str::<Value>(value).ok());
                                if let Some(key) = analog_controls::redirect_target(this.interrupt_recording_targets(),this.spec.config["DKMKEYS"].as_array().map(Vec::as_slice).unwrap_or_default(),this.analog_gamepad.layout,parsed.as_ref().unwrap_or(input)) { this.accept_gaming_key(&key,window,cx); }
                            },
                        }
                    }
                    this.gaming_combined.target.is_some()
                }).unwrap_or(false);
                if !keep { break; }
            }
        }));
        cx.notify();
    }

    fn accept_gaming_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, part)) = self.gaming_combined.target else {
            return;
        };
        let in_game = self.draft["gamingMode"]["enableInGame"]
            .as_bool()
            .unwrap_or(false);
        let effective = match validate(&self.gaming_combined.combos, id, part, key, in_game) {
            Ok(Some(part)) => part,
            Ok(None) => return,
            Err(message) => {
                self.gaming_combined.message = message;
                cx.notify();
                return;
            }
        };
        let Some(combo) = self
            .gaming_combined
            .combos
            .iter_mut()
            .find(|combo| combo.id == id && combo.enabled)
        else {
            return;
        };
        if effective == Part::First {
            combo.key1 = key.into();
        } else {
            combo.key2 = key.into();
        }
        self.gaming_combined.message = Message::Introduction;
        self.submit_gaming_combos(cx);
        if part == Part::First && effective == Part::First {
            self.gaming_combined.target = Some((id, Part::Second));
        } else {
            self.finish_gaming_recording(false, window, cx);
        }
        cx.notify();
    }

    pub(super) fn finish_gaming_recording(
        &mut self,
        reset_message: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if reset_message {
            self.gaming_combined.message = Message::Introduction;
            cx.notify();
        }
        if self.gaming_combined.target.is_none() {
            return;
        }
        // Current useQ stores TRef before setE, but its [E] effect cleanup
        // clears TRef on READY -> KEY1 and KEY1 -> KEY2 (38904,38968..38971).
        // Outside/blur/disabled callbacks therefore retain accepted keys;
        // an extra write-back of a Rust snapshot would invent rollback.
        let resume = self.gaming_combined.resume_single_snap;
        let focus = self.gaming_combined.return_focus.take();
        self.gaming_combined.cancel();
        self.resume_single_snap_capture(resume, cx);
        if let Some(focus) = focus {
            focus.focus(window, cx);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn disabled_rows_still_take_part_in_alt_pair_duplicate_validation() {
        let mut combos = defaults();
        combos[0].key1 = "KEY_LEFT_ALT".into();
        combos[0].key2 = "KEY_A".into();
        combos[1].key1 = "KEY_A".into();
        assert!(matches!(
            validate(&combos, 2, Part::Second, "KEY_RIGHT_ALT", false),
            Err(Message::Duplicate)
        ));
    }

    #[::core::prelude::v1::test]
    fn source_shortcut_guard_is_directional_and_windows_requires_in_game() {
        let mut combos = defaults();
        combos[0].key1 = "KEY_FN".into();
        assert!(matches!(
            validate(&combos, 1, Part::Second, "KEY_F10", false),
            Err(Message::DefaultShortcut)
        ));
        combos[0].key1 = "KEY_F10".into();
        assert!(matches!(
            validate(&combos, 1, Part::Second, "KEY_FN", false),
            Ok(Some(Part::Second))
        ));
        assert!(matches!(
            validate(&combos, 2, Part::First, "KEY_LEFT_GUI", false),
            Ok(None)
        ));
        assert!(matches!(
            validate(&combos, 2, Part::First, "KEY_LEFT_GUI", true),
            Ok(Some(Part::First))
        ));
    }
}

//! Current 742/756 Customize reducers and mounted analog input controls.
//! UI intents are the original BroadcastChannel messages; observations enter
//! through the corresponding MW reducer actions, never through synthetic ACKs.
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::keyboard_capture::{Capture, RawKey};
use razer_widgets::theme::KeyboardAnalogControlColors as Colors;
use serde::Serialize;
use std::time::Duration;

const LOCAL: &str = "_analogControlsLocalV1";

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Interrupt {
    is_enabled: bool,
    interrupts: Vec<String>,
    interruption_key: String,
}
impl Default for Interrupt {
    fn default() -> Self {
        Self {
            is_enabled: false,
            interrupts: vec!["KEY_ANY".into()],
            interruption_key: "any".into(),
        }
    }
}
impl Interrupt {
    /// useAn's effect runs after enabled/mode changes. Disabled mode selection
    /// only changes the reducer's interruptionKey; it emits no device write.
    fn effect_keys(&mut self) -> bool {
        if !self.is_enabled {
            return false;
        }
        if self.interruption_key == "dedicated" {
            self.interrupts.retain(|key| key != "KEY_ANY");
        } else {
            self.interrupts = vec!["KEY_ANY".into()];
        }
        true
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Configuration {
    is_enable_keyboard_keys: bool,
    is_gamepad_override: bool,
    bottom_deadzone: Value,
    interrupt: Interrupt,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            is_enable_keyboard_keys: true,
            is_gamepad_override: true,
            bottom_deadzone: json!({"value":"medium"}),
            interrupt: Interrupt::default(),
        }
    }
}

#[derive(Default)]
pub(super) struct State {
    configuration: Configuration,
    local: bool,
    recording: bool,
    focus: Option<FocusHandle>,
    return_focus: Option<FocusHandle>,
    capture: Option<Capture>,
    native_capture: Option<razer_service::mapping::key_capture::Session>,
    capture_task: Option<Task<()>>,
    capture_generation: u64,
    toggle_task: Option<Task<()>>,
    single_snap_enabled: bool,
    resume_single_snap: bool,
}
impl State {
    fn stop_capture(&mut self) {
        self.recording = false;
        self.capture = None;
        self.native_capture = None;
        self.capture_task = None;
        self.capture_generation = self.capture_generation.wrapping_add(1);
        self.return_focus = None;
        self.resume_single_snap = false;
    }

    pub(super) fn cancel(&mut self) {
        self.stop_capture();
        self.toggle_task = None;
    }
}

#[derive(Clone)]
pub enum KeyboardAnalogControlsObservation {
    EnableKeyboardKeys(bool),
    GamepadOverride(bool),
    BottomDeadzone(Value),
    InterruptKeys(Value),
    SingleSnapTap(bool),
}

impl KeyboardProductSpec {
    pub(super) fn analog_customize_layout(&self) -> bool {
        self.analog_gamepad_layout() || self.modern_analog_controls()
    }

    pub(super) fn modern_analog_controls(&self) -> bool {
        // Current Customize callers, including 728's analog controls without
        // custom gaming combinations, mount all four modern analog panels.
        matches!(
            self.product_id,
            719 | 720 | 721 | 728 | 740 | 741 | 742 | 746 | 756
        )
    }

    pub(super) fn custom_gaming_combinations(&self) -> bool {
        // Each current mounted caller explicitly passes
        // supportCustomCombinedKeys:true. 739 is an ordinary keyboard.
        matches!(
            self.product_id,
            719 | 720 | 721 | 739 | 740 | 741 | 742 | 746 | 756
        )
    }

    pub(super) fn gaming_shortcut(&self) -> Option<&'static str> {
        match self.product_id {
            721 | 739 | 740 | 756 => Some("O"),
            719 | 720 | 728 | 741 | 742 | 746 => Some("F10"),
            _ => None,
        }
    }
}

impl KeyboardProductWorkspace {
    pub fn observe_analog_controls(
        &mut self,
        observation: KeyboardAnalogControlsObservation,
        cx: &mut Context<Self>,
    ) {
        if !self.spec.modern_analog_controls() {
            return;
        }
        let configuration = &mut self.analog_controls.configuration;
        match observation {
            KeyboardAnalogControlsObservation::EnableKeyboardKeys(value) => {
                configuration.is_enable_keyboard_keys = value;
            }
            KeyboardAnalogControlsObservation::GamepadOverride(value) => {
                configuration.is_gamepad_override = value;
            }
            KeyboardAnalogControlsObservation::BottomDeadzone(value) => {
                configuration.bottom_deadzone = value;
            }
            KeyboardAnalogControlsObservation::InterruptKeys(value) => {
                // MW_SET_INTERRUPT_KEYS_TO_UI consumes these exact fields and
                // supplies "any" only when interruptionKey is absent.
                if let (Some(enabled), Some(keys)) =
                    (value["isEnabled"].as_bool(), value["interrupts"].as_array())
                {
                    configuration.interrupt = Interrupt {
                        is_enabled: enabled,
                        interrupts: keys
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect(),
                        interruption_key: value["interruptionKey"].as_str().unwrap_or("any").into(),
                    };
                }
            }
            KeyboardAnalogControlsObservation::SingleSnapTap(enabled) => {
                self.analog_controls.single_snap_enabled = enabled;
            }
        }
        cx.notify();
    }

    pub(super) fn analog_controls_snapshot(&self, snapshot: &mut Value) {
        if self.spec.modern_analog_controls() && self.analog_controls.local {
            snapshot[LOCAL] = serde_json::to_value(&self.analog_controls.configuration).unwrap();
        }
    }

    pub(super) fn restore_analog_controls(&mut self) {
        self.analog_controls.cancel();
        self.analog_controls.configuration = Configuration::default();
        self.analog_controls.local = false;
        if !self.spec.modern_analog_controls() {
            return;
        }
        if let Some(saved) = self
            .draft
            .get(LOCAL)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
        {
            self.analog_controls.configuration = saved;
            self.analog_controls.local = true;
        }
        if matches!(self.spec.product_id, 740 | 746 | 756) {
            self.restore_latest_analog_configuration();
        }
    }

    fn restore_latest_analog_configuration(&mut self) {
        let configuration = &mut self.analog_controls.configuration;
        if let Some(enabled) = self.draft["isEnableKeyboardKeys"].as_bool() {
            configuration.is_enable_keyboard_keys = enabled;
        }
        if let Some(enabled) = self.draft["isGamepadOverride"].as_bool() {
            configuration.is_gamepad_override = enabled;
        }
        if self.draft["bottomDeadzone"].is_object() {
            configuration.bottom_deadzone = self.draft["bottomDeadzone"].clone();
        }
        if let Ok(interrupt) =
            serde_json::from_value::<Interrupt>(self.draft["interruptKey"].clone())
        {
            configuration.interrupt = interrupt;
        }
        self.analog_controls.single_snap_enabled =
            self.draft["singleKeySnapTap"]["isEnabled"] == true;
    }
    pub(super) fn observe_latest_analog_controls(&mut self, cx: &mut Context<Self>) {
        self.restore_latest_analog_configuration();
        cx.notify();
    }

    fn analog_control_changed(&mut self, cx: &mut Context<Self>) {
        self.analog_controls.local = true;
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }

    pub(super) fn analog_deadzone_panel(&self, cx: &Context<Self>) -> AnyElement {
        let deadzone = &self.analog_controls.configuration.bottom_deadzone;
        let selected = deadzone["value"].as_str().or_else(|| deadzone.as_str());
        // Na disables only the gaming/interrupt and pairing/polling wrappers
        // on BLE; this mounted widget is outside those wrappers.
        let disabled = self.customize_factory_root();
        surface::panel_with_help(
            t("SWITCH_DEAD_ZONE_HEADER"),
            surface::help_control("keyboard-switch-deadzone-help", t("SWITCH_DEAD_ZONE_TOOLTIP")),
            cx,
        )
        .mb_0()
        .child(div().mb(surface::css(20.)).child(t("SWITCH_DEAD_ZONE_DESC")))
        .child(
            h_flex()
                .w_auto()
                .h(surface::css(36.))
                .border_1()
                .border_color(Colors::border())
                .rounded(surface::css(36.))
                .children([("responsive", "RESPONSIVE"), ("medium", "MEDIUM_MODE"), ("stable", "STABLE")].map(|(value,label)| {
                    analog_choice(format!("keyboard-switch-deadzone-{value}"), t(label), selected.is_some_and(|current| current.eq_ignore_ascii_case(value)), disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let current = &this.analog_controls.configuration.bottom_deadzone;
                            if current["value"].as_str().or_else(|| current.as_str()).is_some_and(|current| current.eq_ignore_ascii_case(value)) {
                                return;
                            }
                            this.analog_controls.configuration.bottom_deadzone = json!({"value":value});
                            this.analog_controls.local = true;
                            this.request_actuation_message(json!({"type":"ON_SET_BOTTOM_DEADZONE","payload":{"bottomDeadzone":{"value":value}}}), cx);
                            this.analog_control_changed(cx);
                        }))
                })),
        )
        .into_any_element()
    }

    pub(super) fn analog_gamepad_input_panel(&self, cx: &Context<Self>) -> AnyElement {
        let configuration = &self.analog_controls.configuration;
        let enabled = configuration.is_enable_keyboard_keys;
        let override_mapping = configuration.is_gamepad_override;
        let disabled = self.customize_factory_root();
        surface::panel_with_help(
            t("GAMEPAD_INPUT_CUSTOMIZATION"),
            surface::help_control(
                "keyboard-gamepad-input-help",
                t(if matches!(self.spec.product_id, 721 | 740 | 746 | 756) { "GAMEPAD_INPUT_CUSTOMIZATION_TOOLTIPS_SNAP_FLEX_WARNING" } else { "GAMEPAD_INPUT_CUSTOMIZATION_TOOLTIPS" }),
            ),
            cx,
        )
        .mb_0()
        .child(h_flex().justify_between()
            .child(t("KEYBOARD_INPUT_PASSTHROUGH"))
            .child(surface::SynapseSwitch::new("keyboard-gamepad-passthrough")
                .checked(enabled).disabled(disabled)
                .on_change(cx.listener(move |this, _, _, cx| {
                    this.analog_controls.configuration.is_enable_keyboard_keys = !enabled;
                    this.analog_controls.local = true;
                    this.request_actuation_message(json!({"type":"ON_SET_ENABLE_KEYBOARD_KEYS","payload":{"isEnableKeyboardKeys":!enabled}}), cx);
                    this.analog_control_changed(cx);
                }))))
        .child(div().pb(surface::css(24.)).text_color(Colors::muted()).child(t("KEYBOARD_INPUT_PASSTHROUGH_DESC")))
        .child(h_flex().justify_between()
            .child(t("GAMEPAD_MAPPING_OVERRIDE"))
            .child(surface::SynapseSwitch::new("keyboard-gamepad-override")
                .checked(override_mapping).disabled(disabled || !enabled)
                .on_change(cx.listener(move |this, _, _, cx| {
                    this.analog_controls.configuration.is_gamepad_override = !override_mapping;
                    this.analog_controls.local = true;
                    this.request_actuation_message(json!({"type":"ON_SET_GAMEPAD_OVERRIDE_KEYS","payload":{"isGamepadOverride":!override_mapping}}), cx);
                    this.analog_control_changed(cx);
                }))))
        .child(div().text_color(Colors::muted()).child(t("GAMEPAD_MAPPING_OVERRIDE_DESC")))
        .into_any_element()
    }

    fn request_interrupt_configuration(&mut self, cx: &mut Context<Self>) {
        self.analog_controls.local = true;
        let payload = serde_json::to_value(&self.analog_controls.configuration.interrupt).unwrap();
        self.request_actuation_message(
            json!({"type":"ON_SET_INTERRUPT_KEYS","payload":payload}),
            cx,
        );
    }

    fn select_interrupt_mode(
        &mut self,
        mode: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .analog_controls
            .configuration
            .interrupt
            .interruption_key
            == mode
        {
            return;
        }
        self.stop_interrupt_recording(window, cx);
        self.analog_controls
            .configuration
            .interrupt
            .interruption_key = mode.into();
        if self.analog_controls.configuration.interrupt.effect_keys() {
            self.request_interrupt_configuration(cx);
        }
        self.analog_control_changed(cx);
    }

    fn schedule_interrupt_toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let enabled = self.analog_controls.configuration.interrupt.is_enabled;
        self.analog_controls.toggle_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.stop_interrupt_recording(window, cx);
                this.analog_controls.configuration.interrupt.is_enabled = !enabled;
                this.request_interrupt_configuration(cx);
                if this.analog_controls.configuration.interrupt.effect_keys() {
                    this.request_interrupt_configuration(cx);
                }
                this.analog_control_changed(cx);
            });
        }));
    }

    pub(super) fn analog_interrupt_panel(&self, cx: &Context<Self>) -> AnyElement {
        let state = &self.analog_controls;
        let interrupt = &state.configuration.interrupt;
        let dedicated = interrupt.interruption_key == "dedicated";
        let disabled = !interrupt.is_enabled || self.customize_controls_disabled();
        surface::panel_with_title_switch(
            t("INTERRUPT_MACROS_TITLE"),
            surface::SynapseSwitch::new("keyboard-interrupt-macros-enabled")
                .checked(interrupt.is_enabled)
                .disabled(self.customize_controls_disabled())
                .on_change(
                    cx.listener(|this, _, window, cx| this.schedule_interrupt_toggle(window, cx)),
                ),
            surface::help_control(
                "keyboard-interrupt-macros-help",
                t("INTERRUPT_MACROS_TOOLTIPS"),
            ),
            cx,
        )
        .mb_0()
        .child(
            div()
                .mb(surface::css(20.))
                .text_color(Colors::muted())
                .child(t(if dedicated {
                    "INTERRUPTION_DESC"
                } else {
                    "INTERRUPTION_DESC1"
                })),
        )
        .child(
            v_flex()
                .opacity(if disabled { 0.3 } else { 1. })
                .child(
                    h_flex()
                        .w_auto()
                        .h(surface::css(36.))
                        .mb(surface::css(10.))
                        .border_1()
                        .border_color(Colors::border())
                        .rounded(surface::css(36.))
                        .children([("any", "ANY"), ("dedicated", "DEDICATED")].map(
                            |(value, label)| {
                                analog_choice(
                                    format!("keyboard-interrupt-mode-{value}"),
                                    t(label),
                                    interrupt.interruption_key.eq_ignore_ascii_case(value),
                                    disabled,
                                )
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.select_interrupt_mode(value, window, cx)
                                    },
                                ))
                            },
                        )),
                )
                .when(dedicated, |panel| {
                    panel.child(self.interrupt_key_list(disabled, cx))
                })
                .child(div().text_color(Colors::muted()).child(t(if dedicated {
                    "INTERRUPTION_DESC_DEDICATED"
                } else {
                    "INTERRUPTION_DESC_ANY"
                })))
                .when(dedicated && interrupt.interrupts.is_empty(), |panel| {
                    panel.child(
                        h_flex()
                            .gap(surface::css(5.))
                            .mt(surface::css(20.))
                            .text_color(Colors::warning())
                            .child(
                                img("synapse/keyboard-interrupt-warning.svg")
                                    .w(surface::css(20.))
                                    .h(surface::css(27.)),
                            )
                            .child(t("INTERRUPTION_KEY_EMPTY_WARNING_DESC")),
                    )
                }),
        )
        .into_any_element()
    }

    fn interrupt_key_list(&self, disabled: bool, cx: &Context<Self>) -> AnyElement {
        let state = &self.analog_controls;
        let keys = &state.configuration.interrupt.interrupts;
        h_flex().gap(surface::css(10.)).my(surface::css(20.)).p(surface::css(10.)).bg(Colors::record_surface()).rounded(surface::css(6.))
            .children(keys.iter().filter(|key| key.as_str() != "KEY_ANY").map(|key| {
                let input = key.clone();
                let group = SharedString::from(format!("keyboard-interrupt-key-{key}"));
                div().id(group.clone()).group(group.clone()).relative().flex().items_center().justify_center()
                    .h(surface::css(30.)).min_w(surface::css(30.)).px(surface::css(10.)).mx(surface::css(2.))
                    .text_size(surface::css(12.)).text_color(Colors::muted()).border_1().border_color(Colors::muted()).rounded(surface::css(3.))
                    .when(!disabled, |key| key.hover(|key|key.border_color(Colors::muted_hover())))
                    .child(div().h(surface::css(23.)).flex().items_center()
                        .when(!disabled, |label|label.group_hover(group.clone(),|label|label.text_color(Colors::muted_hover())))
                        .child(self.interrupt_recording_key_name(key)))
                    .child(div().absolute().inset_0().flex().items_center().justify_center().child(
                        BaseButton::new(SharedString::from(format!("keyboard-interrupt-delete-{key}")))
                            .disabled(disabled)
                            .accessibility_label(format!("{}: {}",t("DELETE_KEY"),self.interrupt_recording_key_name(key)))
                            .w(surface::css(13.)).h(surface::css(17.)).opacity(0.).text_color(Colors::delete_hover())
                            .when(!disabled, |button|button.group_hover(group,|button|button.opacity(1.)))
                            .focus_visible(|button|button.opacity(1.))
                            .child(razer_assets::resource_svg().path("synapse/keyboard-key-delete-active.svg").size_full())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.analog_controls.configuration.interrupt.interrupts.retain(|key| key != &input);
                                this.request_interrupt_configuration(cx);
                                this.analog_control_changed(cx);
                            }))))
                    .tooltip(move |window,cx| gpui_kit::component::tooltip::Tooltip::new(t("DELETE_KEY")).build(window,cx))
            }))
            .when(state.recording, |list| list.child(
                div().id("keyboard-interrupt-recording-input").when_some(state.focus.as_ref(), |input,focus| input.track_focus(focus))
                    .w(surface::css(30.)).h(surface::css(30.)).border_1().border_color(Colors::accent()).rounded(surface::css(3.))
                    .on_key_down(cx.listener(|this,event: &KeyDownEvent,window,cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                        // Windows capture preserves the physical VK/location;
                        // GPUI's fallback accepts only unambiguous source keys.
                        if this.analog_controls.capture.is_none() {
                            if let Some(key) = this.gpui_single_key_target(event.keystroke.key.as_str()) {
                                this.accept_interrupt_key(&key,window,cx);
                            } else {
                                eprintln!("[keyboard-interrupt-capture] This platform does not expose the physical key/location for {:?}",event.keystroke.key);
                            }
                        }
                    }))
                    .on_mouse_down_out(cx.listener(|this,_,window,cx| this.stop_interrupt_recording(window,cx))),
            ))
            .when(!state.recording && keys.iter().filter(|key| key.as_str() != "KEY_ANY").count() < 4, |list| list.child(
                BaseButton::new("keyboard-interrupt-add-key")
                    .disabled(disabled).accessibility_label(t("ADD_NEW"))
                    .h(surface::css(30.)).min_w(surface::css(30.)).px(surface::css(10.)).mx(surface::css(2.))
                    .border_1().border_color(Colors::muted()).rounded(surface::css(3.)).text_color(Colors::muted())
                    .when(!disabled,|button|button.hover(|button|button.border_color(Colors::accent()).text_color(Colors::accent())))
                    .child("+")
                    .tooltip({ let label = t(if keys.is_empty() { "ADD_NEW" } else { "ADD_ANOTHER_DEDICATED" });
                        move |window,cx|gpui_kit::component::tooltip::Tooltip::new(label.clone()).build(window,cx) })
                    .on_click(cx.listener(|this,_,window,cx| this.start_interrupt_recording(window,cx))),
            ))
            .into_any_element()
    }

    fn start_interrupt_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.analog_controls.recording
            || !self.analog_controls.configuration.interrupt.is_enabled
            || self.customize_controls_disabled()
        {
            return;
        }
        self.finish_gaming_recording(false, window, cx);
        let state = &mut self.analog_controls;
        if state
            .configuration
            .interrupt
            .interrupts
            .iter()
            .filter(|key| key.as_str() != "KEY_ANY")
            .count()
            >= 4
        {
            return;
        }
        state.return_focus = window.focused(cx);
        let focus = state.focus.get_or_insert_with(|| cx.focus_handle());
        focus.focus(window, cx);
        state.recording = true;
        state.capture_generation = state.capture_generation.wrapping_add(1);
        state.capture = Capture::start_stream(window);
        match razer_service::mapping::key_capture::Session::start() {
            Ok(owner) => state.native_capture = Some(owner),
            Err(error) => eprintln!("[keyboard-interrupt-capture] Startup: {error:#}"),
        }
        let generation = state.capture_generation;
        state.resume_single_snap = state.single_snap_enabled;
        if state.resume_single_snap {
            state.single_snap_enabled = false;
            self.request_actuation_message(
                json!({"type":"ON_TOGGLE_SINGLE_KEY_SNAP_TAP","payload":{"isEnabled":false}}),
                cx,
            );
        }
        self.analog_controls.capture_task = Some(cx.spawn_in(window,async move |this,cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let keep = this.update_in(cx,|this,window,cx| {
                    if this.analog_controls.capture_generation != generation || !this.analog_controls.recording { return false; }
                    if this.page != "TAB_CUSTOMIZE" || !window.is_window_active() || !this.analog_controls.focus.as_ref().is_some_and(|focus| focus.is_focused(window)) {
                        this.stop_interrupt_recording(window,cx);
                        return false;
                    }
                    let events = if let Some(capture) = this.analog_controls.capture.as_mut() {
                        let Some(events) = capture.take_all() else {
                            eprintln!("[keyboard-interrupt-capture] Input capture overflowed; canceled recording");
                            this.stop_interrupt_recording(window,cx);
                            return false;
                        };
                        events
                    } else { Vec::new() };
                    let native = this.analog_controls.native_capture.as_ref().map(|owner| owner.drain()).unwrap_or_default();
                    for event in events {
                        if !this.analog_controls.recording { break; }
                        if let Some(key) = this.interrupt_raw_target(&event) {
                            this.accept_interrupt_key(&key,window,cx);
                        }
                    }
                    for reply in native {
                        use razer_service::mapping::key_capture::Reply;
                        match reply {
                            Reply::Mode(_) => {},
                            Reply::Ready(receipt) => {
                                if !receipt.unresolved_devices().is_empty() {
                                    eprintln!("[keyboard-interrupt-capture] Unresolved endpoints: {:?}",receipt.unresolved_devices());
                                }
                            }
                            Reply::Failed(error) => {
                                eprintln!("[keyboard-interrupt-capture] {error}");
                                this.analog_controls.native_capture = None;
                            }
                            Reply::Events(events) => for event in events {
                                if !this.analog_controls.recording { break; }
                                let input = &event["input"];
                                let parsed = input.as_str().and_then(|value| serde_json::from_str::<Value>(value).ok());
                                this.consume_interrupt_redirect(parsed.as_ref().unwrap_or(input),window,cx);
                            },
                        }
                    }
                    this.analog_controls.recording
                }).unwrap_or(false);
                if !keep { break; }
            }
        }));
        cx.notify();
    }

    fn interrupt_raw_target(&self, event: &RawKey) -> Option<String> {
        // Source handleKeyDown consumes the browser key down, while
        // onInputRedirectEvent below consumes the odd (release) flag.
        if !event.down() {
            return None;
        }
        browser_target(self.interrupt_recording_targets(), event)
    }

    fn consume_interrupt_redirect(
        &mut self,
        input: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = redirect_target(
            self.interrupt_recording_targets(),
            self.spec.config["DKMKEYS"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default(),
            self.analog_gamepad.layout,
            input,
        );
        if let Some(key) = key {
            self.accept_interrupt_key(&key, window, cx);
        }
    }

    pub(super) fn gpui_single_key_target(&self, key: &str) -> Option<String> {
        let code = match key {
            "escape" => 27,
            "backspace" => 8,
            "tab" => 9,
            "enter" | "return" => 13,
            "space" => 32,
            "pageup" => 33,
            "pagedown" => 34,
            "end" => 35,
            "home" => 36,
            "left" => 37,
            "up" => 38,
            "right" => 39,
            "down" => 40,
            "insert" => 45,
            "delete" => 46,
            key if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() => {
                u32::from(key.as_bytes()[0].to_ascii_uppercase())
            }
            key if key.starts_with('f') => {
                key[1..]
                    .parse::<u32>()
                    .ok()
                    .filter(|value| (1..=24).contains(value))?
                    + 111
            }
            _ => return None,
        };
        let mut candidates = self.interrupt_recording_targets().iter().filter(|target| {
            target["keyCode"] == code.to_string() && target["inputID"].is_string()
        });
        let target = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        target["inputID"].as_str().map(str::to_owned)
    }

    fn accept_interrupt_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let keys = &self.analog_controls.configuration.interrupt.interrupts;
        if !self.analog_controls.recording
            || keys.len() >= 4
            || keys.iter().any(|existing| existing == key)
        {
            return;
        }
        if self
            .draft
            .pointer(&self.mapping_path())
            .and_then(Value::as_array)
            .is_some_and(|mappings| {
                mappings.iter().any(|mapping| {
                    mapping["inputID"] == key
                        && mapping["inputType"] == "AnalogInput"
                        && matches!(
                            mapping["mapping"][0]["outputType"].as_str(),
                            Some("dynamicKeyStrokeGroup" | "macroGroup")
                        )
                })
            })
        {
            return;
        }
        self.analog_controls
            .configuration
            .interrupt
            .interrupts
            .retain(|key| key != "KEY_ANY");
        self.analog_controls
            .configuration
            .interrupt
            .interrupts
            .push(key.into());
        self.request_interrupt_configuration(cx);
        self.analog_control_changed(cx);
        self.stop_interrupt_recording(window, cx);
    }

    pub(super) fn stop_interrupt_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.analog_controls;
        let resume = state.resume_single_snap;
        let focus = state.return_focus.take();
        let recording = state.recording;
        state.stop_capture();
        if resume {
            state.single_snap_enabled = true;
            self.request_actuation_message(
                json!({"type":"ON_TOGGLE_SINGLE_KEY_SNAP_TAP","payload":{"isEnabled":true}}),
                cx,
            );
        }
        if recording {
            if let Some(focus) = focus {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    pub(super) fn leave_analog_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_interrupt_recording(window, cx);
        // useAn cancels its debounced switch when the mounted widget exits.
        self.analog_controls.toggle_task = None;
    }

    pub(super) fn suspend_single_snap_capture(&mut self, cx: &mut Context<Self>) -> bool {
        let enabled = self.analog_controls.single_snap_enabled;
        if enabled {
            self.analog_controls.single_snap_enabled = false;
            self.request_actuation_message(
                json!({"type":"ON_TOGGLE_SINGLE_KEY_SNAP_TAP","payload":{"isEnabled":false}}),
                cx,
            );
        }
        enabled
    }

    pub(super) fn resume_single_snap_capture(&mut self, resume: bool, cx: &mut Context<Self>) {
        if resume {
            self.analog_controls.single_snap_enabled = true;
            self.request_actuation_message(
                json!({"type":"ON_TOGGLE_SINGLE_KEY_SNAP_TAP","payload":{"isEnabled":true}}),
                cx,
            );
        }
    }
}

fn source_flag(key: &Value) -> Option<u64> {
    key.get("outputFlag")
        .and_then(Value::as_u64)
        .or_else(|| key["flag"].as_u64())
}

pub(super) fn browser_target(targets: &[Value], event: &RawKey) -> Option<String> {
    let alias = match event.code() {
        0x91 => Some("KEY_SCROLL_LOCK"),
        0x13 => Some("KEY_PAUSE"),
        0x90 => Some("KEY_NUMPAD_NUM_LOCK"),
        _ => None,
    };
    let flag = if event.extended() { 2 } else { 0 };
    let target = if let Some(alias) = alias {
        targets.iter().find(|key| key["inputID"] == alias)
    } else {
        // Tj filters by browser keyCode before considering location. This
        // avoids confusing a VK_BACKSLASH with the shared scan43 pound row.
        targets
            .iter()
            .filter(|key| key["keyCode"] == event.code().to_string())
            .find(|key| {
                key["scancode"].as_u64() == Some(u64::from(event.scan()))
                    && source_flag(key) == Some(flag)
            })
            .or_else(|| {
                targets
                    .iter()
                    .find(|key| key["keyCode"] == event.code().to_string())
            })
    };
    target
        .and_then(|key| key["inputID"].as_str())
        .map(str::to_owned)
}

pub(super) fn redirect_target(
    targets: &[Value],
    dkm: &[Value],
    layout: Option<u32>,
    input: &Value,
) -> Option<String> {
    if !matches!(
        input["type"].as_str(),
        Some("keyboard" | "analogKey" | "razerKey")
    ) || input["flag"].as_u64().is_none_or(|flag| flag % 2 == 0)
    {
        return None;
    }
    // Current 69937.B8W and 46114.Fo/uH; this is the native redirect
    // non-US-pound conversion, independent of mapping-recording exclusions.
    let backslash = input["scancode"] == 43
        && layout.is_some_and(|layout| [1, 2, 5, 8, 9, 11, 19, 20, 21].contains(&layout));
    let target = targets
        .iter()
        .find(|key| {
            if backslash {
                return key["HID"] == "49";
            }
            key["scancode"] == input["scancode"]
                && source_flag(key).is_some_and(|flag| {
                    input["flag"]
                        .as_u64()
                        .is_some_and(|input| flag == input || flag + 1 == input)
                })
        })
        .or_else(|| {
            if input["type"] != "razerKey" {
                return None;
            }
            let input_key = input["key"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| input["key"].to_string());
            // Preserve source strict equality: e.key === t.key.toString().
            // Current numeric DKMKEYS do not satisfy that source branch.
            dkm.iter()
                .find(|key| key["key"].as_str() == Some(input_key.as_str()))
        });
    target
        .and_then(|key| key["inputID"].as_str())
        .map(str::to_owned)
}

fn analog_choice(id: String, label: String, selected: bool, disabled: bool) -> BaseButton {
    BaseButton::new(SharedString::from(id))
        .selected(selected)
        .disabled(disabled)
        .h(surface::css(26.))
        .mx(surface::css(5.))
        .my(surface::css(4.3))
        .px(surface::css(10.))
        .rounded(surface::css(26.))
        .text_size(surface::css(14.))
        .text_color(Colors::text())
        .styles(|styles| {
            styles.selected(|style| {
                style
                    .bg(Colors::accent())
                    .text_color(Colors::selected_text())
            })
        })
        .child(label)
}

#[cfg(test)]
mod tests {
    use super::{Configuration, Interrupt, redirect_target};
    use serde_json::json;

    #[::core::prelude::v1::test]
    fn redirect_uses_release_flag_layout_conversion_and_strict_dkm_key_equality() {
        let keys = vec![
            json!({"inputID":"KEY_LEFT_ALT","scancode":56,"outputFlag":0,"flag":8,"HID":"226"}),
            json!({"inputID":"KEY_RIGHT_ALT","scancode":56,"outputFlag":2,"flag":8,"HID":"230"}),
            json!({"inputID":"KEY_BACKSLASH","scancode":43,"outputFlag":0,"HID":"49"}),
        ];
        assert_eq!(
            redirect_target(
                &keys,
                &[],
                None,
                &json!({"type":"analogKey","scancode":56,"flag":3})
            )
            .as_deref(),
            Some("KEY_RIGHT_ALT")
        );
        assert!(
            redirect_target(
                &keys,
                &[],
                None,
                &json!({"type":"keyboard","scancode":56,"flag":2})
            )
            .is_none()
        );
        assert_eq!(
            redirect_target(
                &keys,
                &[],
                Some(1),
                &json!({"type":"keyboard","scancode":43,"flag":3})
            )
            .as_deref(),
            Some("KEY_BACKSLASH")
        );
        let packet = json!({"type":"razerKey","scancode":999,"flag":1,"key":42});
        assert!(
            redirect_target(
                &keys,
                &[json!({"inputID":"DKM_F1","key":42})],
                None,
                &packet
            )
            .is_none()
        );
        assert_eq!(
            redirect_target(
                &keys,
                &[json!({"inputID":"DKM_F1","key":"42"})],
                None,
                &packet
            )
            .as_deref(),
            Some("DKM_F1")
        );
    }
    #[::core::prelude::v1::test]
    fn disabled_mode_selection_keeps_keys_until_enable_effect() {
        let mut state = Interrupt::default();
        state.interruption_key = "dedicated".into();
        assert!(!state.effect_keys());
        assert_eq!(state.interrupts, ["KEY_ANY"]);
        state.is_enabled = true;
        assert!(state.effect_keys());
        assert!(state.interrupts.is_empty());
        state.interrupts.push("KEY_A".into());
        state.interruption_key = "any".into();
        assert!(state.effect_keys());
        assert_eq!(state.interrupts, ["KEY_ANY"]);
    }
    #[::core::prelude::v1::test]
    fn local_restore_keeps_disabled_override_value_and_dedicated_keys() {
        let mut saved = Configuration::default();
        saved.is_enable_keyboard_keys = false;
        saved.interrupt.interruption_key = "dedicated".into();
        saved.interrupt.interrupts = vec!["KEY_A".into(), "KEY_D".into()];
        let restored: Configuration =
            serde_json::from_value(serde_json::to_value(saved).unwrap()).unwrap();
        assert!(!restored.is_enable_keyboard_keys);
        assert!(restored.is_gamepad_override);
        assert_eq!(restored.interrupt.interrupts, ["KEY_A", "KEY_D"]);
    }
}

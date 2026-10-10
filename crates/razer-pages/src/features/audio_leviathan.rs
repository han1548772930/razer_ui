//! Current 1352 LG source page. Source reducer defaults are presentation only;
//! real device observations and transport capabilities are supplied separately.
use super::AudioProductWorkspace;
use gpui_kit::component::{
    ActiveTheme, WindowExt as _, h_flex,
    slider::{SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n::t;
use razer_widgets::{source_slider::SourceSlider, surface};

fn source_help(id: &'static str, text: String) -> AnyElement {
    div()
        .absolute()
        .right(surface::css(10.))
        .top(surface::css(10.))
        .child(surface::help_control(id, text))
        .into_any_element()
}

#[derive(Default)]
struct DisabledPointer {
    hovered: bool,
    position: Point<Pixels>,
}
fn disabled_card(
    id: &'static str,
    content: AnyElement,
    disabled: bool,
    tip: String,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let state = window.use_keyed_state((ElementId::from(id), "disabled-pointer"), cx, |_, _| {
        DisabledPointer::default()
    });
    let current = state.read(cx);
    let shown = disabled && current.hovered;
    let position = current.position + point(px(0.), window.rem_size() * (20. / 16.));
    div()
        .id(id)
        .relative()
        .child(content)
        .when(disabled, |wrapper| {
            wrapper.child(
                div()
                    .id((ElementId::from(id), "overlay"))
                    .occlude()
                    .absolute()
                    .top(surface::css(10.))
                    .bottom(surface::css(10.))
                    .left(surface::css(10.))
                    .right(surface::css(10.))
                    .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                        state.hovered = *hovered;
                        cx.notify();
                    }))
                    .on_mouse_move(window.listener_for(
                        &state,
                        |state, event: &MouseMoveEvent, _, cx| {
                            state.position = event.position;
                            cx.notify();
                        },
                    )),
            )
        })
        .when(shown, |wrapper| {
            wrapper.child(
                deferred(
                    gpui_kit::base::Positioner::corner(Anchor::TopLeft, position)
                        .margin(px(0.))
                        .child(
                            div()
                                .w(surface::css(300.))
                                .px(surface::css(10.))
                                .py(surface::css(8.))
                                .bg(rgb(0x111111))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .text_color(rgb(0xcccccc))
                                .font_family("Roboto")
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .child(tip),
                        ),
                )
                .with_priority(200),
            )
        })
        .into_any_element()
}
fn condition_panel(title: String, corner: AnyElement, disabled: bool, cx: &App) -> Div {
    let opacity = if disabled { 0.3 } else { 1. };
    v_flex()
        .relative()
        .w_full()
        .flex_shrink_0()
        .my(surface::css(10.))
        .py(surface::css(30.))
        .px(surface::css(40.))
        .bg(rgb(0x111111))
        .rounded(surface::css(5.))
        .text_size(surface::css(14.))
        .child(
            h_flex().justify_between().opacity(opacity).child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .text_color(cx.theme().primary)
                    .mb(surface::css(20.))
                    .child(title.to_uppercase()),
            ),
        )
        .child(div().opacity(opacity).child(corner))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeviathanOperation {
    Subwoofer { level: u8, is_enabled: bool },
    Output { headset: bool },
    AudioMode { mode: u8 },
    BluetoothInput,
}
impl LeviathanOperation {
    /// Current reducer-to-middleware message names and payload keys. These
    /// are service message semantics, not inferred HID report bytes.
    pub fn source_message(self) -> (&'static str, serde_json::Value) {
        match self {
            Self::Subwoofer { level, is_enabled } => (
                "ON_SET_SUBWOOFER",
                serde_json::json!({"subWoofer":{"level":level,"isEnabled":is_enabled}}),
            ),
            Self::Output { headset } => (
                "ON_SWITCH_AUDIO_OUTPUT_SOURCE",
                serde_json::json!({"audioOutputSource":if headset { "HEADSET" } else { "SOUNDBAR" }}),
            ),
            Self::AudioMode { mode } => (
                "ON_SET_AUDIO_MODE",
                serde_json::json!({"audioModeSelected":mode}),
            ),
            Self::BluetoothInput => (
                "ON_SET_INPUT_SOURCE",
                serde_json::json!({"inputSource":"bluetooth"}),
            ),
        }
    }
}
#[derive(Clone, Debug)]
pub struct LeviathanRequest {
    pub generation: u64,
    pub operation: LeviathanOperation,
}
#[derive(Clone, Debug)]
pub struct LeviathanObservation {
    pub subwoofer_level: u8,
    pub subwoofer_enabled: bool,
    pub headset_output: bool,
    pub headset_connected: bool,
    pub audio_mode: u8,
    pub camera_tracking: bool,
}
pub(super) struct State {
    // fr/vr/Hr defaults at source offsets 4098631..4098872.
    visible: LeviathanObservation,
    observed: bool,
    transport_available: bool,
    generation: u64,
    pending: Option<LeviathanRequest>,
    input_popup: bool,
    windows_11: bool,
    bass: Option<Entity<SliderState>>,
    bass_timer: Option<Task<()>>,
}
impl State {
    pub(super) fn new(pid: u32, _window: &mut Window, cx: &mut App) -> Self {
        Self {
            visible: LeviathanObservation {
                subwoofer_level: 4,
                subwoofer_enabled: true,
                headset_output: false,
                headset_connected: false,
                audio_mode: 0,
                camera_tracking: false,
            },
            observed: false,
            transport_available: false,
            generation: 0,
            pending: None,
            input_popup: false,
            windows_11: razer_platform::system::is_windows_11(),
            bass: (pid == 1352).then(|| {
                cx.new(|_| {
                    SliderState::new()
                        .min(1.)
                        .max(7.)
                        .step(1.)
                        .default_value(4.)
                })
            }),
            bass_timer: None,
        }
    }
}
impl EventEmitter<LeviathanRequest> for AudioProductWorkspace {}
impl AudioProductWorkspace {
    pub(super) fn dismiss_leviathan_page(&mut self) {
        self.leviathan.input_popup = false;
        self.leviathan.bass_timer = None;
        self.leviathan.pending = None;
        self.leviathan.generation = self.leviathan.generation.wrapping_add(1);
    }
    pub fn leviathan_has_observation(&self) -> bool {
        self.leviathan.observed
    }
    pub fn leviathan_request_current(&self, request: &LeviathanRequest) -> bool {
        self.spec.product_id == 1352
            && self.page == "TAB_SOUND"
            && self.leviathan.pending.as_ref().is_some_and(|pending| {
                pending.generation == request.generation && pending.operation == request.operation
            })
    }
    pub fn cancel_leviathan(&mut self, cx: &mut Context<Self>) {
        self.dismiss_leviathan_page();
        self.leviathan.transport_available = false;
        self.leviathan.observed = false;
        cx.notify();
    }
    pub(super) fn subscribe_leviathan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bass) = self.leviathan.bass.clone() else {
            return;
        };
        self.subscriptions.push(
            cx.subscribe_in(&bass, window, |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    let level = value.start().round().clamp(1., 7.) as u8;
                    this.leviathan.visible.subwoofer_level = level;
                    this.leviathan.bass_timer = Some(cx.spawn_in(window, async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(300))
                            .await;
                        let _ = this.update_in(cx, |this, window, cx| {
                            this.request_leviathan(
                                LeviathanOperation::Subwoofer {
                                    level,
                                    is_enabled: this.leviathan.visible.subwoofer_enabled,
                                },
                                window,
                                cx,
                            );
                        });
                    }));
                    cx.notify();
                }
            }),
        );
    }
    /// Transport adapters must explicitly opt in. Source defaults/local drafts
    /// do not grant real-device mutation capability.
    pub fn set_leviathan_transport_available(&mut self, available: bool, cx: &mut Context<Self>) {
        self.leviathan.transport_available = available;
        cx.notify();
    }
    pub fn observe_leviathan(
        &mut self,
        value: LeviathanObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 1352
            || !(1..=7).contains(&value.subwoofer_level)
            || value.audio_mode > 3
        {
            return;
        }
        self.leviathan.bass_timer = None;
        self.leviathan.visible = value;
        self.leviathan.observed = true;
        if let Some(bass) = &self.leviathan.bass {
            self.syncing = true;
            bass.update(cx, |bass, cx| {
                bass.set_value(
                    f32::from(self.leviathan.visible.subwoofer_level),
                    window,
                    cx,
                )
            });
            self.syncing = false;
        }
        cx.notify();
    }
    pub fn complete_leviathan(
        &mut self,
        generation: u64,
        result: Result<LeviathanObservation, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .leviathan
            .pending
            .as_ref()
            .is_some_and(|pending| pending.generation == generation)
        {
            return;
        }
        self.leviathan.pending = None;
        match result {
            Ok(observation) => self.observe_leviathan(observation, window, cx),
            Err(error) => window.push_notification(error, cx),
        }
        cx.notify();
    }
    fn request_leviathan(
        &mut self,
        operation: LeviathanOperation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.leviathan.transport_available {
            window.push_notification("Leviathan V2 Pro 设备提交服务尚未接入；未写入设备", cx);
            return;
        }
        match operation {
            LeviathanOperation::Subwoofer { level, is_enabled } => {
                self.leviathan.visible.subwoofer_level = level;
                self.leviathan.visible.subwoofer_enabled = is_enabled;
            }
            LeviathanOperation::Output { headset } => {
                self.leviathan.visible.headset_output = headset
            }
            LeviathanOperation::AudioMode { mode } => self.leviathan.visible.audio_mode = mode,
            LeviathanOperation::BluetoothInput => {}
        }
        self.leviathan.generation = self.leviathan.generation.wrapping_add(1);
        let request = LeviathanRequest {
            generation: self.leviathan.generation,
            operation,
        };
        self.leviathan.pending = Some(request.clone());
        cx.emit(request);
        cx.notify();
    }
    pub(super) fn leviathan_sound_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .w_full()
            .child(
                div()
                    .relative()
                    .flex()
                    .justify_center()
                    .items_center()
                    .w_full()
                    .h(surface::css(250.))
                    .max_w(surface::css(1220.))
                    .min_w(surface::css(1024.))
                    .mx_auto()
                    .my(surface::css(10.))
                    .child(surface::dot_background(cx))
                    .child(
                        img(if window.scale_factor() > 1. {
                            "synapse/audio-1352-product-3x.png"
                        } else {
                            "synapse/audio-1352-product-1x.png"
                        })
                        .relative()
                        .w(surface::css(325.))
                        .h(surface::css(250.))
                        .object_fit(ObjectFit::Contain),
                    ),
            )
            .child(
                surface::page_columns()
                    .child(surface::page_column(
                        v_flex()
                            .child(self.source_volume_panel(cx))
                            .child(self.leviathan_subwoofer(window, cx))
                            .child(self.leviathan_output(window, cx))
                            .child(self.leviathan_input(cx)),
                    ))
                    .child(surface::page_column(
                        v_flex()
                            .child(self.leviathan_modes(window, cx))
                            .child(self.leviathan_properties(cx)),
                    )),
            )
            .into_any_element()
    }
    fn leviathan_subwoofer(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.leviathan.visible.headset_output;
        let level = self.leviathan.visible.subwoofer_level;
        let body = div()
            .relative()
            .child(
                SourceSlider::new(
                    self.leviathan.bass.as_ref().expect("1352 bass slider"),
                    f32::from(level - 1) / 6.,
                )
                .tip(Some(level.to_string()))
                .interaction_enabled(!disabled),
            )
            .child(
                h_flex()
                    .absolute()
                    .bottom(surface::css(-2.))
                    .w_full()
                    .justify_between()
                    .child(t("LESS_BASS").to_uppercase())
                    .child(t("MORE_BASS").to_uppercase()),
            );
        let panel = condition_panel(
            t("SUBWOOFER_LEVEL"),
            source_help("leviathan-bass-help", t("SUBWOOFER_DESC")),
            disabled,
            cx,
        )
        .relative()
        .child(body.opacity(if disabled { 0.3 } else { 1. }))
        .into_any_element();
        disabled_card(
            "leviathan-bass-card",
            panel,
            disabled,
            "Toggle to Soundbar output to enable this setting.".into(),
            window,
            cx,
        )
    }
    fn leviathan_output(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let connected = self.leviathan.visible.headset_connected;
        let current = self.leviathan.visible.headset_output;
        let tabs = h_flex()
            .self_start()
            .h(surface::css(36.))
            .p(surface::css(4.))
            .rounded(surface::css(18.))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .gap(surface::css(5.))
            .children(
                [(false, "SOUNDBAR"), (true, "HEADSET")].map(|(headset, label)| {
                    gpui_kit::base::Button::new(("leviathan-output", u64::from(headset)))
                        .px(surface::css(10.))
                        .py(surface::css(5.))
                        .rounded(surface::css(13.))
                        .text_size(surface::css(14.))
                        .child(t(label))
                        .when(current == headset, |button| {
                            button.bg(cx.theme().primary).text_color(rgb(0x111111))
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.leviathan.visible.headset_connected {
                                this.request_leviathan(
                                    LeviathanOperation::Output { headset },
                                    window,
                                    cx,
                                );
                            }
                        }))
                }),
            );
        let panel = condition_panel(
            t("SOUNDBAR_HEADSET_TITLE"),
            source_help("leviathan-output-help", t("SOUNDBAR_HEADSET_TOOLTIP")),
            !connected,
            cx,
        )
        .relative()
        .child(
            v_flex()
                .opacity(if connected { 1. } else { 0.3 })
                .gap(surface::css(20.))
                .child(t("SOUNDBAR_HEADSET_DESC"))
                .child(tabs)
                .child(
                    h_flex()
                        .child(t("SOUNDBAR_HEADSET_TIP"))
                        .child(" ( ")
                        .child(
                            img("synapse/audio-1352-input-icon.svg")
                                .w(surface::css(21.))
                                .h(surface::css(12.)),
                        )
                        .child(" )."),
                ),
        )
        .into_any_element();
        disabled_card(
            "leviathan-output-card",
            panel,
            !connected,
            t("SOUNDBAR_HEADSET_DISABLED_TIP"),
            window,
            cx,
        )
    }
    fn leviathan_input(&self, cx: &mut Context<Self>) -> AnyElement {
        surface::panel_with_control(
            t("INPUT_SOURCE"),
            source_help("leviathan-input-help", t("INPUT_SOURCE_TIP2")),
            cx,
        )
        .relative()
        .child(
            gpui_kit::base::Button::new("leviathan-switch-bluetooth")
                .self_start()
                .h(surface::css(27.))
                .px(surface::css(16.))
                .min_w(surface::css(90.))
                .rounded(surface::css(3.))
                .bg(rgb(0x707070))
                .text_color(rgb(0xffffff))
                .text_size(surface::css(12.))
                .child(t("SWITCH_TO_BLE").to_uppercase())
                .hover(|style| style.bg(rgba(0xffffff4d)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.leviathan.input_popup = true;
                    cx.notify();
                })),
        )
        .child(
            h_flex()
                .my(surface::css(20.))
                .child(t("SWITCH_TO_BLE_DESC_1"))
                .child(
                    img("synapse/audio-1352-input-icon.svg")
                        .w(surface::css(21.))
                        .h(surface::css(12.)),
                )
                .child(" )."),
        )
        .when(self.leviathan.input_popup, |panel| {
            panel.child(self.leviathan_input_popup(cx))
        })
        .into_any_element()
    }
    fn leviathan_input_popup(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("leviathan-input-confirm")
            .absolute()
            .top(relative(-1.28))
            .w(surface::css(360.))
            .p(surface::css(20.))
            .gap(surface::css(10.))
            .items_center()
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0xfd8611))
            .rounded(surface::css(5.))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.leviathan.input_popup = false;
                cx.notify();
            }))
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .child(img("synapse/audio-1352-warning.svg").size(surface::css(20.)))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(16.))
                            .text_color(rgb(0xfd8611))
                            .child(t("INPUT_CONFIRM_POPUP_HEADER").to_uppercase()),
                    ),
            )
            .child(
                div()
                    .text_center()
                    .line_height(surface::css(17.))
                    .child(t("INPUT_CONFIRM_POPUP_CONTENT")),
            )
            .child(
                div()
                    .text_center()
                    .text_color(rgb(0x999999))
                    .line_height(surface::css(17.))
                    .child(t("INPUT_CONFIRM_POPUP_TIP")),
            )
            .child(
                gpui_kit::base::Button::new("leviathan-confirm-bluetooth")
                    .mt(surface::css(10.))
                    .px(surface::css(16.))
                    .py(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(rgb(0x707070))
                    .text_color(rgb(0xffffff))
                    .text_size(surface::css(12.))
                    .child(t("SWITCH_TO_BLE").to_uppercase())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.leviathan.input_popup = false;
                        this.request_leviathan(LeviathanOperation::BluetoothInput, window, cx);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
    fn leviathan_modes(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.leviathan.visible;
        let labels = [
            ("", "STEREO"),
            ("THX_SPATIAL_AUDIO", "VIRTUAL_HEADSET"),
            ("THX_SPATIAL_AUDIO", "VIRTUAL_SPEAKER"),
            ("", "ROOM_FILL"),
        ];
        let descriptions = [
            "AUDIO_MODE_DESC",
            "AUDIO_MODE_DESC2",
            "AUDIO_MODE_DESC3",
            "AUDIO_MODE_DESC4",
        ];
        let mode = usize::from(state.audio_mode.min(3));
        let camera_color = if matches!(mode, 1 | 2) {
            if state.camera_tracking {
                rgb(0x44d62c)
            } else {
                rgb(0xfd8611)
            }
        } else {
            rgb(0xcccccc)
        };
        let camera_tip = if matches!(mode, 1 | 2) {
            if state.camera_tracking {
                "AUDIO_MODE_CAMERA_ENABLE_TOLLTIP"
            } else {
                "AUDIO_MODE_CAMERA_ENABLE_TOLLTIP1"
            }
        } else {
            "AUDIO_MODE_CAMERA_DISABLE_TOLLTIP"
        };
        let opacity = if state.headset_output { 0.3 } else { 1. };
        let panel = condition_panel(
            t("AUDIO_MODES"),
            div()
                .id("leviathan-modes-help")
                .group("leviathan-modes-help")
                .absolute()
                .right(surface::css(10.))
                .top(surface::css(10.))
                .size(surface::css(14.))
                .bg(rgb(0x4a4a4a))
                .rounded(surface::css(7.))
                .child(img("synapse/audio-1352-help.svg").size_full())
                .child(
                    v_flex()
                        .absolute()
                        .right_0()
                        .top(surface::css(24.))
                        .w(surface::css(300.))
                        .px(surface::css(10.))
                        .py(surface::css(8.))
                        .gap(surface::css(16.))
                        .items_center()
                        .bg(rgb(0x000000))
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .invisible()
                        .group_hover("leviathan-modes-help", |style| style.visible())
                        .child(
                            div()
                                .text_size(surface::css(14.))
                                .line_height(surface::css(18.))
                                .child(t("AUDIO_MODE_TOOLTIP")),
                        )
                        .child(
                            img("synapse/audio-1352-mode-tip.svg")
                                .w(surface::css(262.))
                                .h(surface::css(207.)),
                        ),
                )
                .into_any_element(),
            state.headset_output,
            cx,
        )
        .relative()
        .child(
            h_flex()
                .opacity(opacity)
                .gap(surface::css(12.))
                .mb(surface::css(20.))
                .items_stretch()
                .children(labels.into_iter().enumerate().map(|(id, (title, body))| {
                    gpui_kit::base::Button::new(("leviathan-mode", id as u64))
                        .flex()
                        .flex_col()
                        .justify_center()
                        .items_center()
                        .px(surface::css(20.))
                        .py(surface::css(6.))
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(if id == mode {
                            rgb(0x44d62c)
                        } else {
                            rgb(0x5d5d5d)
                        })
                        .bg(if id == mode {
                            rgb(0x292929)
                        } else {
                            rgb(0x111111)
                        })
                        .child(
                            div()
                                .text_size(surface::css(10.))
                                .line_height(surface::css(12.))
                                .mb(surface::css(2.))
                                .child(if title.is_empty() {
                                    String::new()
                                } else {
                                    t(title).to_uppercase()
                                }),
                        )
                        .child(
                            div()
                                .text_size(surface::css(12.))
                                .child(t(body).to_uppercase()),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if !this.leviathan.visible.headset_output {
                                this.request_leviathan(
                                    LeviathanOperation::AudioMode { mode: id as u8 },
                                    window,
                                    cx,
                                );
                            }
                        }))
                })),
        )
        .child(
            v_flex()
                .opacity(opacity)
                .mb(surface::css(20.))
                .min_h(surface::css(56.))
                .child(
                    h_flex()
                        .items_center()
                        .child(
                            div()
                                .id("leviathan-camera-status")
                                .group("leviathan-camera-status")
                                .relative()
                                .size(surface::css(36.))
                                .mr(surface::css(10.))
                                .rounded_full()
                                .bg(rgb(0x222222))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    img("synapse/audio-1352-camera.svg")
                                        .w(surface::css(18.))
                                        .h(surface::css(20.)),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left(surface::css(6.))
                                        .top(surface::css(6.))
                                        .size(surface::css(4.))
                                        .rounded_full()
                                        .bg(camera_color),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .top(surface::css(40.))
                                        .w(surface::css(300.))
                                        .px(surface::css(10.))
                                        .py(surface::css(8.))
                                        .bg(rgb(0x111111))
                                        .border_1()
                                        .border_color(rgb(0x5d5d5d))
                                        .line_height(surface::css(17.))
                                        .invisible()
                                        .group_hover("leviathan-camera-status", |style| {
                                            style.visible()
                                        })
                                        .child(t(camera_tip))
                                        .when(matches!(mode, 1 | 2), |tip| {
                                            tip.child(
                                                div().child(t("AUDIO_MODE_CAMERA_ENABLE_TOLLTIP2")),
                                            )
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .line_height(surface::css(17.))
                                .child(t(descriptions[mode])),
                        ),
                )
                .when(matches!(mode, 1 | 2), |body| {
                    body.child(
                        div()
                            .pl(surface::css(46.))
                            .pt(surface::css(3.))
                            .text_color(rgb(0x999999))
                            .child(t(if mode == 1 {
                                "AUDIO_MODE_TIP1"
                            } else {
                                "AUDIO_MODE_TIP2"
                            })),
                    )
                }),
        )
        .child(
            gpui_kit::base::Button::new("leviathan-demo-play")
                .opacity(opacity)
                .relative()
                .w_full()
                .mt(surface::css(20.))
                .mb(surface::css(30.))
                .child(
                    img("synapse/audio-1352-demo.png")
                        .w_full()
                        .object_fit(ObjectFit::Contain),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .size(surface::css(100.))
                                .rounded_full()
                                .border_3()
                                .border_color(rgb(0xffffff))
                                .bg(rgba(0x000000cc))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(img("synapse/audio-1352-play.svg").size(surface::css(48.))),
                        ),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    if !this.leviathan.visible.headset_output {
                        window.push_notification("音频演示播放服务尚未接入", cx);
                    }
                })),
        )
        .into_any_element();
        disabled_card(
            "leviathan-modes-card",
            panel,
            state.headset_output,
            t("SOUNDBAR_DISABLED_TIP"),
            window,
            cx,
        )
    }
    fn leviathan_properties(&self, cx: &mut Context<Self>) -> AnyElement {
        // LG mounts $U with no isTHX prop, so only the Windows audio link exists.
        surface::panel_with_control(
            t("SOUND_PROPERTIES"),
            source_help("leviathan-properties-help", t("SOUND_PROPERTIES_TOOLTIP")),
            cx,
        )
        .relative()
        .child(
            h_flex()
                .items_center()
                .child(
                    img(if self.leviathan.windows_11 {
                        "synapse/audio-1352-windows-11.svg"
                    } else {
                        "synapse/audio-1352-windows.svg"
                    })
                    .size(surface::css(44.))
                    .mr(surface::css(20.)),
                )
                .child(
                    gpui_kit::base::Button::new("leviathan-open-sound-properties")
                        .group("leviathan-properties-link")
                        .h(surface::css(44.))
                        .line_height(surface::css(44.))
                        .flex()
                        .items_center()
                        .text_size(surface::css(14.))
                        .underline()
                        .hover(|style| style.text_color(rgb(0x44d62c)))
                        .child(t("OPEN_WINDOW_PROPERTIES_SOUND"))
                        .child(
                            div()
                                .relative()
                                .size(surface::css(20.))
                                .ml(surface::css(4.))
                                .child(
                                    img("synapse/audio-1352-properties-external.svg").size_full(),
                                )
                                .child(
                                    img("synapse/audio-1352-properties-external-active.svg")
                                        .absolute()
                                        .inset_0()
                                        .size_full()
                                        .opacity(0.)
                                        .group_hover("leviathan-properties-link", |style| {
                                            style.opacity(1.)
                                        }),
                                ),
                        )
                        .on_click(|_, window, cx| {
                            if let Err(error) = razer_platform::system::open(
                                razer_platform::system::Properties::Sound,
                            ) {
                                window.push_notification(format!("无法打开声音属性：{error}"), cx);
                            }
                        }),
                ),
        )
        .into_any_element()
    }
}

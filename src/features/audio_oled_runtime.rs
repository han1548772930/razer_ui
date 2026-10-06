//! Current 1383 runtime reducers and their mounted conditional UI.
//! Observations are external facts. Requests and the source reducer's immediate
//! transitions never fabricate a device reply, a download, or completion.
use super::*;
use std::time::Duration;
#[path = "audio_oled_runtime_theme.rs"]
mod theme;
use theme::RuntimeColors as C;

#[derive(Clone, Debug)]
// Constructed by a real transport adapter; this UI never manufactures replies.
#[allow(dead_code)]
pub(crate) enum OledRuntimeObservation {
    Connection {
        is_ble: bool,
        is_dongle: bool,
    },
    Loading {
        target: String,
        kind: String,
        total_items: u64,
        current_item: u64,
        progress: f32,
    },
    Error {
        payload: Option<Value>,
    },
    Language {
        value: u8,
        changed: u64,
    },
}
#[derive(Clone, Debug)]
pub(crate) struct OledRuntimeRequested {
    message_type: &'static str,
    payload: Option<Value>,
}
// The forwarded request is consumed by a real transport adapter when present.
#[allow(dead_code)]
impl OledRuntimeRequested {
    pub(crate) fn message_type(&self) -> &'static str {
        self.message_type
    }
    pub(crate) fn payload(&self) -> Option<&Value> {
        self.payload.as_ref()
    }
}
impl EventEmitter<OledRuntimeRequested> for AudioProductWorkspace {}

#[derive(Deserialize)]
struct RuntimeSpec {
    connection: Connection,
    language: Language,
    loading: Loading,
    labels: BTreeMap<String, String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Connection {
    is_ble: bool,
    is_dongle: bool,
}
#[derive(Clone, Deserialize)]
struct Language {
    value: u8,
    changed: u64,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Loading {
    target: String,
    #[serde(rename = "type")]
    kind: String,
    total_items: u64,
    current_item: u64,
    progress: f32,
}
fn spec() -> &'static RuntimeSpec {
    static SPEC: OnceLock<RuntimeSpec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("audio_oled_runtime_data.json"))
            .expect("validated 1383 runtime defaults")
    })
}
fn text(symbol: &str) -> String {
    t(&spec().labels[symbol])
}
pub(super) fn ble_text() -> String {
    text("XVF")
}

pub(super) struct OledRuntimeState {
    connection: Connection,
    language: Language,
    loading: Loading,
    error: Option<Value>,
    reset_error_visible: bool,
    confirm_language_cancel: bool,
    progress_motion: ProgressMotion,
}
impl OledRuntimeState {
    pub(super) fn new() -> Self {
        Self {
            connection: spec().connection.clone(),
            language: spec().language.clone(),
            loading: spec().loading.clone(),
            error: None,
            reset_error_visible: false,
            confirm_language_cancel: false,
            progress_motion: ProgressMotion::new(),
        }
    }
    fn reset_loading(&mut self) {
        self.loading = Loading {
            target: "none".into(),
            kind: "none".into(),
            total_items: 0,
            current_item: 0,
            progress: 0.,
        };
        self.confirm_language_cancel = false;
        self.progress_motion = ProgressMotion::new();
    }
    fn reset_context(&self) -> bool {
        self.loading.kind == "progress" && self.loading.target == "reset"
    }
}

impl AudioProductWorkspace {
    /// Feed only an actual transport/connection observation into this state.
    pub(crate) fn observe_oled_runtime(
        &mut self,
        observation: OledRuntimeObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(home) = self.oled_home.as_mut() else {
            return;
        };
        let state = &mut home.runtime;
        match observation {
            OledRuntimeObservation::Connection { is_ble, is_dongle } => {
                state.connection = Connection { is_ble, is_dongle }
            }
            OledRuntimeObservation::Loading {
                target,
                kind,
                total_items,
                current_item,
                progress,
            } => {
                if !progress.is_finite() {
                    return;
                }
                let was_progress = state.loading.kind == "progress";
                if !was_progress {
                    state.progress_motion = ProgressMotion::new();
                }
                state.progress_motion.set(progress);
                if kind != "progress" || target != "language" {
                    state.confirm_language_cancel = false;
                }
                state.loading = Loading {
                    target,
                    kind,
                    total_items,
                    current_item,
                    progress,
                };
            }
            OledRuntimeObservation::Error { payload } => {
                // `if (!t) return null`: null/false/zero/empty string do not open vx.
                state.error = payload.filter(js_truthy);
                state.reset_error_visible = state.error.is_some();
            }
            OledRuntimeObservation::Language { value, changed } => {
                let old_changed = state.language.changed;
                state.language = Language { value, changed };
                self.draft["device"]["oledLanguage"] = json!(value);
                self.staged.remove("/device/oledLanguage");
                // Bv's mounted effect runs only for changed > 1 and selected=6.
                if changed != old_changed
                    && changed > 1
                    && self.draft["oledHome"]["home"]["selected"] == 6
                {
                    let language = decode_language(value);
                    let info = self.draft["oledHome"]["system"]["info"].clone();
                    if let Some(slides) = self.draft["oledHome"]["system"]["slides"].as_array_mut()
                    {
                        for slide in slides {
                            for side in ["left", "right"] {
                                let tag = &mut slide[side];
                                if tag.is_null() {
                                    continue;
                                }
                                let label = if language == 1 {
                                    rust_i18n::t!(
                                        tag["deviceLabelKey"].as_str().unwrap_or(""),
                                        locale = "zh-CN"
                                    )
                                    .to_string()
                                } else {
                                    info[tag["id"].as_str().unwrap_or("")]["label"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_owned()
                                };
                                tag["deviceLabel"] = json!(label);
                            }
                        }
                    }
                    cx.emit(AudioProductChanged);
                }
                self.sync_oled_runtime_select(window, cx);
            }
        }
        cx.notify();
    }
    pub(crate) fn request_oled_runtime_data(&self, cx: &mut Context<Self>) {
        if self.oled_home.is_some() {
            request(cx, "ON_SET_OLED_DATA_TO_UI", None);
        }
    }
    pub(in crate::features::audio_products) fn oled_is_ble(&self) -> bool {
        self.oled_home
            .as_ref()
            .is_some_and(|home| home.runtime.connection.is_ble)
    }
    pub(in crate::features::audio_products) fn oled_is_loading(&self) -> bool {
        self.oled_home
            .as_ref()
            .is_some_and(|home| home.runtime.loading.kind == "progress")
    }
    pub(in crate::features::audio_products) fn oled_language_values(&self) -> (u8, u8, u8) {
        let raw = self
            .oled_home
            .as_ref()
            .map_or(0, |home| home.runtime.language.value);
        let staged = self
            .staged
            .get("/device/oledLanguage")
            .and_then(Value::as_u64)
            .map_or(raw, |v| v as u8);
        (raw, staged, decode_language(staged))
    }
    /// Parent calls this at the end of its general sync; encoded raw language
    /// (>=127) must select `255 & ~raw`, while remaining eligible for Apply.
    pub(in crate::features::audio_products) fn sync_oled_runtime_select(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.oled_home.is_none() {
            return;
        }
        let value = self.oled_language_values().2.to_string();
        let syncing = self.syncing;
        self.syncing = true;
        if let Some(select) = self.selects.get("/device/oledLanguage") {
            select.update(cx, |state, cx| state.set_selected_value(&value, window, cx));
        }
        self.syncing = syncing;
    }
    pub(in crate::features::audio_products) fn apply_oled_language(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.oled_is_ble() || self.oled_is_loading() {
            return;
        }
        let (_, _, value) = self.oled_language_values();
        if let Some(home) = &mut self.oled_home {
            home.runtime.language.value = value;
        }
        self.staged.remove("/device/oledLanguage");
        // SET_OLED_LANGUAGE itself commits this local reducer value. It does
        // not increment changed or fabricate loading/completion feedback.
        self.edit("/device/oledLanguage", json!(value), window, cx);
        request(
            cx,
            "ON_SET_OLED_LANGUAGE",
            Some(json!({"oledLanguage":value})),
        );
    }
    fn cancel_oled_download(&mut self, cx: &mut Context<Self>) {
        if let Some(home) = &mut self.oled_home {
            home.runtime.reset_loading();
        }
        request(cx, "ON_SET_CANCEL_OLED_DOWNLOAD_PROCESS", None);
        cx.notify();
    }
    fn oled_error_action(&mut self, retry: bool, cx: &mut Context<Self>) {
        let Some(state) = self.oled_home.as_ref().map(|home| &home.runtime) else {
            return;
        };
        let Some(payload) = state.error.clone() else {
            return;
        };
        let reset = state.reset_context();
        if reset && retry {
            let data = &self.draft["oledHome"];
            let mut emote = data["emote"].clone();
            emote["imageData"] = emote["src"].clone();
            let mut banner = data["banner"].clone();
            banner["imageState"] = json!(u8::from(banner["image"]["enabled"] == true));
            banner["imagePos"] = json!(u8::from(banner["image"]["position"] != "top"));
            banner["imageIndex"] = json!(0);
            let mut media = data["media"].clone();
            media["mediaState"] = json!(u8::from(media["info"]["enabled"] == true));
            media["visualizerState"] = json!(u8::from(media["visualizer"]["enabled"] == true));
            let mut system = data["system"].clone();
            system["interval"] = system["timeBetweenSlides"].clone();
            system["tempFormat"] = system["temperatureUnit"].clone();
            request(
                cx,
                "ON_RESET_OLED",
                Some(
                    json!({"animation":data["animation"],"image":data["image"],"emote":emote,"banner":banner,"media":media,"system":system}),
                ),
            );
            if let Some(home) = &mut self.oled_home {
                home.runtime.reset_error_visible = false;
            }
        } else {
            if reset {
                self.cancel_oled_download(cx);
            }
            if let Some(home) = &mut self.oled_home {
                home.runtime.error = None;
                home.runtime.reset_error_visible = false;
            }
            request(
                cx,
                if retry {
                    "ON_SET_OLED_HOME_SCREEN_DISPLAY_ERROR_RETRY"
                } else {
                    "ON_SET_OLED_HOME_SCREEN_DISPLAY_ERROR_REVERT"
                },
                Some(payload),
            );
        }
        cx.notify();
    }
    /// Local layers occupy the OLED body, matching the source's absolute
    /// backdrops. No invented Escape/backdrop-dismiss control is attached.
    pub(in crate::features::audio_products) fn oled_runtime_layers(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(state) = self.oled_home.as_ref().map(|home| &home.runtime) else {
            return vec![];
        };
        let mut layers = vec![];
        if !state.connection.is_dongle {
            layers.push(
                deferred(warning(
                    window,
                    "1383-oled-wired",
                    "QAd",
                    div().child(text("l2t")).into_any_element(),
                    400.,
                    Some(117.),
                    None,
                ))
                .with_priority(200)
                .into_any_element(),
            );
        }
        if state.loading.kind == "simple" {
            layers.push(
                deferred(
                    div()
                        .absolute()
                        .inset_0()
                        .m(surface::css(-20.))
                        .bg(C::backdrop())
                        .occlude()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .mt(surface::css(40.))
                                .size(surface::css(100.))
                                .with_animation(
                                    "1383-oled-loading-ring",
                                    Animation::new(Duration::from_secs(2)).repeat(),
                                    |el, phase| el.child(spinner(phase)),
                                ),
                        ),
                )
                .with_priority(100)
                .into_any_element(),
            );
        }
        if state.loading.kind == "progress"
            && matches!(
                state.loading.target.as_str(),
                "animation" | "image" | "reset" | "language"
            )
        {
            layers.push(
                deferred(self.oled_progress(window, cx))
                    .with_priority(100)
                    .into_any_element(),
            );
        }
        if state.error.is_some() && (!state.reset_context() || state.reset_error_visible) {
            let footer = h_flex()
                .gap(surface::css(5.))
                .justify_center()
                .items_center()
                .child(
                    runtime_button("1383-oled-revert", "B3A", false, true)
                        .on_click(cx.listener(|this, _, _, cx| this.oled_error_action(false, cx))),
                )
                .child(
                    runtime_button("1383-oled-retry", "p6C", true, false)
                        .on_click(cx.listener(|this, _, _, cx| this.oled_error_action(true, cx))),
                );
            let body = v_flex()
                .child(text("sUR"))
                .child(div().h(surface::css(17.)))
                .child(text("ZV_"));
            layers.push(
                deferred(warning(
                    window,
                    "1383-oled-error",
                    "JQR",
                    body.into_any_element(),
                    330.,
                    Some(200.),
                    Some(footer.into_any_element()),
                ))
                .with_priority(200)
                .into_any_element(),
            );
        }
        if state.confirm_language_cancel
            && state.loading.kind == "progress"
            && state.loading.target == "language"
        {
            let footer = h_flex()
                .gap(surface::css(5.))
                .justify_center()
                .items_center()
                .child(
                    runtime_button("1383-oled-language-cancel", "bOp", false, false).on_click(
                        cx.listener(|this, _, _, cx| {
                            if let Some(home) = &mut this.oled_home {
                                home.runtime.confirm_language_cancel = false;
                            }
                            request(cx, "ON_CANCEL_OLED_LANGUAGE_UPDATE", None);
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    runtime_button("1383-oled-language-continue", "Jf5", true, true).on_click(
                        cx.listener(|this, _, _, cx| {
                            if let Some(home) = &mut this.oled_home {
                                home.runtime.confirm_language_cancel = false;
                            }
                            cx.notify();
                        }),
                    ),
                );
            layers.push(
                deferred(warning(
                    window,
                    "1383-oled-language-impact",
                    "QWf",
                    div().child(text("$PN")).into_any_element(),
                    330.,
                    None,
                    Some(footer.into_any_element()),
                ))
                .with_priority(200)
                .into_any_element(),
            );
        }
        layers
    }
    fn oled_progress(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.oled_home.as_ref().unwrap().runtime;
        let loading = &state.loading;
        let language = loading.target == "language";
        let (progress, animating) = state.progress_motion.value();
        if animating {
            window.request_animation_frame();
        }
        let cancel = runtime_button("1383-oled-progress-cancel", "tus", false, false)
            .w(surface::css(100.))
            .p(surface::css(5.))
            .mx_auto()
            .disabled(loading.target == "reset")
            .styles(|style| style.disabled(|style| style.opacity(0.3)))
            .on_click(cx.listener(move |this, _, _, cx| {
                if language {
                    if let Some(home) = &mut this.oled_home {
                        home.runtime.confirm_language_cancel = true;
                    }
                    cx.notify();
                } else {
                    this.cancel_oled_download(cx);
                }
            }));
        let body = runtime_body(window)
            .w(surface::css(400.))
            .min_h(surface::css(260.))
            .border_color(C::border())
            .text_center()
            .child(
                div()
                    .text_size(surface::css(16.))
                    .child(text(if language { "_Hj" } else { "JQR" }).to_uppercase()),
            )
            .child(text("kM7"))
            .child(text("dt6"))
            .child(div().h(surface::css(17.)))
            .child(
                h_flex()
                    .justify_between()
                    .child(format!("{}...", text("R42")))
                    .child(format!("{}%", loading.progress)),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(surface::css(5.))
                    .rounded(surface::css(4.))
                    .overflow_hidden()
                    .bg(C::progress_track())
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .h_full()
                            .w(relative(progress / 100.))
                            .rounded(surface::css(15.))
                            .bg(C::green()),
                    ),
            )
            .when(!language, |body| {
                body.child(
                    div().text_right().child(
                        text("stg")
                            .replace("{{currentItem}}", &loading.current_item.to_string())
                            .replace("{{totalItems}}", &loading.total_items.to_string()),
                    ),
                )
            })
            .child(cancel);
        div()
            .absolute()
            .top(surface::css(10.))
            .left_0()
            .w_full()
            .flex()
            .justify_center()
            .child(
                div()
                    .w(surface::css(400.))
                    .min_h(surface::css(260.))
                    .bg(C::backdrop())
                    .occlude()
                    .child(body),
            )
            .into_any_element()
    }
}

fn request(
    cx: &mut Context<AudioProductWorkspace>,
    message_type: &'static str,
    payload: Option<Value>,
) {
    cx.emit(OledRuntimeRequested {
        message_type,
        payload,
    });
}
fn decode_language(value: u8) -> u8 {
    if value < 127 { value } else { !value }
}
fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.),
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}
fn runtime_body(window: &Window) -> Div {
    v_flex()
        .flex_shrink_0()
        .p(surface::css(20.))
        .gap(surface::css(10.))
        .justify_center()
        .bg(C::body())
        .border_1()
        .rounded(surface::css(3.))
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .text_color(C::text())
        .shadow(vec![BoxShadow {
            color: C::shadow(),
            offset: point(px(0.), window.rem_size() * (6. / 16.)),
            blur_radius: window.rem_size() * (10. / 16.),
            spread_radius: px(0.),
            inset: false,
        }])
}
fn warning(
    window: &Window,
    id: &'static str,
    title: &str,
    body: AnyElement,
    width: f32,
    height: Option<f32>,
    footer: Option<AnyElement>,
) -> AnyElement {
    let panel = runtime_body(window)
        .mt(surface::css(100.))
        .w(surface::css(width))
        .border_color(C::warning())
        .when_some(height, |el, height| el.h(surface::css(height)))
        .child(
            h_flex()
                .items_center()
                .justify_center()
                .gap(surface::css(5.))
                .text_size(surface::css(16.))
                .text_color(C::warning())
                .child(
                    img("synapse/audio-oled-runtime-warning.svg")
                        .w(surface::css(20.))
                        .h(surface::css(27.)),
                )
                .child(text(title).to_uppercase()),
        )
        .child(div().mb(surface::css(5.)).text_center().child(body))
        .when_some(footer, |el, footer| el.child(footer));
    div()
        .id(id)
        .absolute()
        .inset_0()
        .mt(surface::css(-12.))
        .mx(surface::css(-20.))
        .bg(C::backdrop())
        .occlude()
        .flex()
        .justify_center()
        .items_start()
        .child(panel)
        .into_any_element()
}
fn runtime_button(id: &'static str, title: &str, confirm: bool, wide: bool) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(text(title))
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .rounded(surface::css(3.))
        .font_family("Roboto")
        .text_size(surface::css(12.))
        .text_color(if confirm {
            C::confirm_text()
        } else {
            C::white()
        })
        .bg(if confirm { C::green() } else { C::button() })
        .active(|style| style.opacity(0.3))
        .when(confirm, |el| {
            el.border_1().border_color(C::confirm_border())
        })
        .when(wide, |el| el.w(surface::css(150.)))
        .child(text(title).to_uppercase())
}

/// k's decreasing-progress path finishes the previous item, resets to zero
/// with a 1ms transition, then advances with the CSS 300ms linear transition.
struct ProgressMotion {
    from: f32,
    to: f32,
    start: Instant,
    wrap: bool,
    finish: f32,
}
impl ProgressMotion {
    fn new() -> Self {
        Self {
            from: 0.,
            to: 0.,
            start: Instant::now(),
            wrap: false,
            finish: 0.,
        }
    }
    fn set(&mut self, value: f32) {
        if value == self.to {
            return;
        }
        self.from = self.value().0;
        self.wrap = value < self.to;
        self.finish = if self.to < 100. { 0.3 } else { 0. };
        self.to = value;
        self.start = Instant::now();
    }
    fn value(&self) -> (f32, bool) {
        let elapsed = self.start.elapsed().as_secs_f32();
        if self.wrap {
            if elapsed < self.finish {
                return (self.from + (100. - self.from) * elapsed / self.finish, true);
            }
            if elapsed < self.finish + 0.001 {
                return (100. * (1. - (elapsed - self.finish) / 0.001), true);
            }
            let progress = (elapsed - self.finish - 0.001) / 0.3;
            (self.to * progress.clamp(0., 1.), progress < 1.)
        } else {
            let progress = (elapsed / 0.3).clamp(0., 1.);
            (
                self.from + (self.to - self.from) * progress,
                progress < 1. && self.from != self.to,
            )
        }
    }
}
fn spinner(phase: f32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            // Qv's independent current SVG: r30, stroke10, square foreground caps;
            // linear keyTimes 0/.5/1, rotate0/180/720 and dash10%/50%/10%, 2s.
            let scale = f32::from(bounds.size.width) / 100.;
            let radius = 30. * scale;
            let center = bounds.center();
            let rotation = if phase <= 0.5 {
                phase * 360.
            } else {
                180. + (phase - 0.5) * 1080.
            };
            let fraction = if phase <= 0.5 {
                0.1 + phase * 0.8
            } else {
                0.5 - (phase - 0.5) * 0.8
            };
            for (start, length, alpha) in [
                (0., std::f32::consts::TAU, 0.3),
                (rotation.to_radians(), fraction * std::f32::consts::TAU, 1.),
            ] {
                let project =
                    |angle: f32| center + point(px(angle.cos() * radius), px(angle.sin() * radius));
                let mut path = PathBuilder::stroke(px(10. * scale));
                path.move_to(project(start));
                path.arc_to(
                    point(px(radius), px(radius)),
                    px(0.),
                    false,
                    true,
                    project(start + length / 2.),
                );
                path.arc_to(
                    point(px(radius), px(radius)),
                    px(0.),
                    false,
                    true,
                    project(start + length),
                );
                if let Ok(path) = path.build() {
                    window.paint_path(path, C::green().opacity(alpha));
                }
                if alpha == 1. {
                    for (angle, direction) in [(start, -1.), (start + length, 1.)] {
                        let endpoint = project(angle);
                        let normal =
                            point(px(angle.cos() * 5. * scale), px(angle.sin() * 5. * scale));
                        let extension = point(
                            px(-angle.sin() * 5. * scale * direction),
                            px(angle.cos() * 5. * scale * direction),
                        );
                        let mut cap = PathBuilder::fill();
                        cap.move_to(endpoint + normal);
                        cap.line_to(endpoint - normal);
                        cap.line_to(endpoint - normal + extension);
                        cap.line_to(endpoint + normal + extension);
                        cap.close();
                        if let Ok(cap) = cap.build() {
                            window.paint_path(cap, C::green());
                        }
                    }
                }
            }
        },
    )
    .size(surface::css(100.))
}

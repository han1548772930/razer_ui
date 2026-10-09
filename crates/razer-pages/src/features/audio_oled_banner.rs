//! Current 1383 `Ug` / `Hg`: retained Banner draft and four source keyframes.
use super::*;
use gpui_kit::base::Radio;
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use razer_widgets::scroll::SourceScrollable as _;
use std::time::Duration;
#[path = "audio_oled_banner_theme.rs"]
mod theme;
use theme::Colors;

#[derive(Deserialize)]
struct Named {
    name: String,
}
#[derive(Deserialize)]
struct ImagePreset {
    id: String,
    src: String,
    asset: String,
}
#[derive(Deserialize)]
struct Limits {
    horizontal_width: f32,
    vertical_with_image: f32,
    vertical_without_image: f32,
}
#[derive(Deserialize)]
struct Motion {
    horizontal_base_ms: f32,
    horizontal_width_ms: f32,
    vertical_ms: f32,
    restart_ms: u64,
}
#[derive(Deserialize)]
struct TextareaMetrics {
    outer_width: f32,
    outer_height: f32,
    padding: f32,
}
#[derive(Deserialize)]
struct Spec {
    labels: BTreeMap<String, String>,
    fonts: Vec<Named>,
    sizes: Vec<Named>,
    defaults: Value,
    images: Vec<ImagePreset>,
    icons: BTreeMap<String, String>,
    limits: Limits,
    motion: Motion,
    textarea: TextareaMetrics,
    placeholder: String,
}
fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("audio_oled_banner_data.json"))
            .expect("audited current 1383 Banner data")
    })
}
fn label(symbol: &str) -> String {
    t(&spec().labels[symbol])
}

pub(super) fn default_value() -> Value {
    let mut value = spec().defaults.clone();
    // Local presentation state: Hg's effect measures only when value/scroll changes.
    value["local_duration_ms"] = json!(0);
    value
}
pub(super) fn normalize(value: &mut Value) {
    if !value.is_object() {
        *value = default_value();
        return;
    }
    for key in ["image", "text"] {
        if !value[key].is_object() {
            value[key] = spec().defaults[key].clone();
        }
    }
    if !value["image"]["enabled"].is_boolean() {
        value["image"]["enabled"] = json!(true);
    }
    if !matches!(value["image"]["position"].as_str(), Some("top" | "bottom")) {
        value["image"]["position"] = json!("top");
    }
    let index = spec()
        .images
        .iter()
        .position(|i| Some(i.id.as_str()) == value["image"]["selected"]["id"].as_str())
        .or_else(|| {
            value["image"]["selectedIdx"]
                .as_u64()
                .map(|v| v as usize)
                .filter(|&v| v < spec().images.len())
        })
        .unwrap_or(0);
    let selected = &spec().images[index];
    value["image"]["selected"] = json!({"id":selected.id,"src":selected.src});
    value["image"]["selectedIdx"] = json!(index);
    if !value["text"]["value"].is_string() {
        value["text"]["value"] = spec().defaults["text"]["value"].clone();
    }
    if !matches!(
        value["text"]["scroll"].as_str(),
        Some("left" | "right" | "up" | "down")
    ) {
        value["text"]["scroll"] = spec().defaults["text"]["scroll"].clone();
    }
    if !value["text"]["style"].is_object() {
        value["text"]["style"] = spec().defaults["text"]["style"].clone();
    }
    for (key, choices) in [("fontFamily", &spec().fonts), ("fontSize", &spec().sizes)] {
        if !choices
            .iter()
            .any(|n| Some(n.name.as_str()) == value["text"]["style"][key]["name"].as_str())
        {
            value["text"]["style"][key] = spec().defaults["text"]["style"][key].clone();
        }
    }
    for (key, allowed) in [
        ("fontWeight", "bold"),
        ("fontStyle", "italic"),
        ("textDecoration", "underline"),
    ] {
        if value["text"]["style"][key].as_str() != Some(allowed) {
            value["text"]["style"][key] = Value::Null;
        }
    }
    if !value["local_duration_ms"]
        .as_f64()
        .is_some_and(|v| v.is_finite() && v >= 0.)
    {
        value["local_duration_ms"] = json!(0);
    }
    if !value["imageData"].is_string() {
        value["imageData"] = json!("");
    }
}

struct TextGeometry {
    font: Font,
    size: f32,
    line_height: f32,
    width: f32,
    height: f32,
    viewport: f32,
    horizontal: bool,
    text: String,
}
fn geometry(value: &Value, window: &Window) -> TextGeometry {
    let style = &value["text"]["style"];
    let size = style["fontSize"]["name"]
        .as_str()
        .unwrap_or("20px")
        .trim_end_matches("px")
        .parse::<f32>()
        .expect("normalized source font size");
    let mut font = window.text_style().font();
    font.family = style["fontFamily"]["name"]
        .as_str()
        .unwrap_or("RazerF5")
        .to_owned()
        .into();
    font.weight = if style["fontWeight"] == "bold" {
        FontWeight::BOLD
    } else {
        FontWeight::NORMAL
    };
    font.style = if style["fontStyle"] == "italic" {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };
    let scale = f32::from(window.rem_size()) / 16.;
    let font_size = window.rem_size() * (size / 16.);
    let font_id = window.text_system().resolve_font(&font);
    // CSS leaves line-height normal. Use actual native font metrics, never a
    // guessed fixed ratio. Chromium's UA/line-gap parity is separately noted.
    let line_height = f32::from(
        window.text_system().ascent(font_id, font_size)
            + window.text_system().descent(font_id, font_size),
    ) / scale;
    let horizontal = matches!(value["text"]["scroll"].as_str(), Some("left" | "right"));
    let original = value["text"]["value"].as_str().unwrap_or_default();
    let text = if horizontal {
        original.replace('\n', " ")
    } else {
        original.to_owned()
    };
    let width = text
        .split('\n')
        .map(|line| {
            let run = TextRun {
                len: line.len(),
                font: font.clone(),
                color: Colors::text(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(
                window
                    .text_system()
                    .shape_line(line.to_owned().into(), font_size, &[run], None)
                    .width,
            ) / scale
        })
        .fold(0_f32, f32::max);
    let viewport = if value["image"]["enabled"] == true {
        44.
    } else {
        64.
    };
    let height = (text.split('\n').count() as f32 * line_height).max(viewport);
    TextGeometry {
        font,
        size,
        line_height,
        width: (width + if horizontal { 6. } else { 0. }).max(232.),
        height,
        viewport,
        horizontal,
        text,
    }
}
/// Mount-time Hg measurement for each separately retained preview instance.
pub(super) fn measure_duration_ms(value: &Value, window: &Window) -> f32 {
    spec().motion.horizontal_base_ms
        + spec().motion.horizontal_width_ms * geometry(value, window).width.round()
}
fn duration_ms(value: &Value, geometry: &TextGeometry) -> f32 {
    if geometry.horizontal {
        value["local_duration_ms"]
            .as_f64()
            .filter(|v| *v > 0.)
            .map(|v| v as f32)
            .unwrap_or(
                spec().motion.horizontal_base_ms
                    + spec().motion.horizontal_width_ms * geometry.width.round(),
            )
    } else {
        spec().motion.vertical_ms
    }
}
fn frame(phase: f32, extent: f32, positive: bool) -> (f32, f32) {
    let direction = if positive { 1. } else { -1. };
    let offset = if phase <= 0.5 {
        phase * 2.
    } else if phase < 0.51 {
        1. - (phase - 0.5) * 200.
    } else {
        -(1. - (phase - 0.51) / 0.49)
    } * extent
        * direction;
    let opacity = if !(0.49..0.52).contains(&phase) {
        1.
    } else if phase <= 0.5 {
        1. - (phase - 0.49) * 100.
    } else if phase < 0.51 {
        0.
    } else {
        (phase - 0.51) * 100.
    };
    (offset, opacity.clamp(0., 1.))
}
pub(super) fn preview(value: &Value, elapsed: f32, window: &Window) -> AnyElement {
    let geometry = geometry(value, window);
    let phase = (elapsed.max(0.) * 1000. / duration_ms(value, &geometry)).fract();
    let positive = matches!(value["text"]["scroll"].as_str(), Some("right" | "down"));
    let (offset, opacity) = frame(
        phase,
        if geometry.horizontal {
            geometry.width
        } else {
            geometry.height + 5.
        },
        positive,
    );
    let mut lines = v_flex()
        .flex_shrink_0()
        .w(surface::css(if geometry.horizontal {
            geometry.width
        } else {
            232.
        }))
        .h(surface::css(if geometry.horizontal {
            geometry.viewport
        } else {
            geometry.height
        }))
        .font_family(geometry.font.family.clone())
        .font_weight(geometry.font.weight)
        .when(geometry.font.style == FontStyle::Italic, |e| e.italic())
        .when(
            value["text"]["style"]["textDecoration"] == "underline",
            |e| e.underline(),
        )
        .text_size(surface::css(geometry.size))
        .line_height(surface::css(geometry.line_height))
        .text_color(Colors::text())
        .bg(Colors::black())
        .whitespace_nowrap();
    if geometry.horizontal {
        lines = lines.justify_center().px(surface::css(3.));
    }
    for line in geometry.text.split('\n') {
        lines = lines.child(
            div()
                .h(surface::css(geometry.line_height))
                .flex_shrink_0()
                .child(line.to_owned()),
        );
    }
    let content = div()
        .relative()
        .w(surface::css(232.))
        .h(surface::css(geometry.viewport))
        .flex_shrink_0()
        .overflow_hidden()
        .child(
            lines
                .absolute()
                .left(surface::css(if geometry.horizontal { offset } else { 0. }))
                .top(surface::css(if geometry.horizontal { 0. } else { offset }))
                .opacity(opacity),
        );
    let image = spec()
        .images
        .iter()
        .find(|i| Some(i.id.as_str()) == value["image"]["selected"]["id"].as_str())
        .unwrap_or(&spec().images[0]);
    let banner = || {
        img(image.asset.clone())
            .w(surface::css(232.))
            .h(surface::css(20.))
            .flex_shrink_0()
    };
    v_flex()
        .w(surface::css(232.))
        .h(surface::css(64.))
        .bg(Colors::black())
        .when(
            value["image"]["enabled"] == true && value["image"]["position"] == "top",
            |e| e.child(banner()),
        )
        .child(content)
        .when(
            value["image"]["enabled"] == true && value["image"]["position"] == "bottom",
            |e| e.child(banner()),
        )
        .into_any_element()
}

pub(super) struct BannerEditor {
    owner: WeakEntity<AudioProductWorkspace>,
    draft: Value,
    dialog: dialog::DialogState,
    text: Entity<TextareaState>,
    font: Entity<SelectState<Vec<Choice>>>,
    size: Entity<SelectState<Vec<Choice>>>,
    warning: bool,
    paused: bool,
    epoch: Instant,
    carried: f32,
    syncing: bool,
    _subscriptions: Vec<Subscription>,
}
impl BannerEditor {
    pub(super) fn open(
        owner: WeakEntity<AudioProductWorkspace>,
        mut draft: Value,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        normalize(&mut draft);
        // The editor mounts a separate Hg; its initial effect always remeasures.
        draft["local_duration_ms"] = json!(measure_duration_ms(&draft, window));
        cx.new(|cx: &mut Context<Self>| {
            let text = cx.new(|cx| {
                TextareaState::new(window, cx)
                    .default_value(
                        draft["text"]["value"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                    )
                    .placeholder(spec().placeholder.clone())
            });
            let font =
                cx.new(|cx| SelectState::new(Self::choices(&spec().fonts), None, window, cx));
            let size =
                cx.new(|cx| SelectState::new(Self::choices(&spec().sizes), None, window, cx));
            let mut this = Self {
                owner,
                draft,
                dialog: dialog::DialogState::new(window, cx),
                text,
                font,
                size,
                warning: false,
                paused: false,
                epoch: Instant::now(),
                carried: 0.,
                syncing: false,
                _subscriptions: Vec::new(),
            };
            this._subscriptions.push(cx.subscribe_in(
                &this.text,
                window,
                |this: &mut Self,
                 input: &Entity<TextareaState>,
                 event: &InputEvent,
                 window,
                 cx: &mut Context<Self>| {
                    if this.syncing || !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let value = input.read(cx).value().to_string();
                    if this.draft["text"]["value"] == value {
                        return;
                    }
                    this.warning = false;
                    this.draft["text"]["value"] = json!(value);
                    this.enforce_limit(window, cx);
                    this.measure_duration(window);
                    this.restart(true);
                    cx.notify();
                },
            ));
            for (state, key) in [
                (this.font.clone(), "fontFamily"),
                (this.size.clone(), "fontSize"),
            ] {
                this._subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this: &mut Self, _, event, window, cx| {
                        if this.syncing {
                            return;
                        }
                        if let SelectEvent::Confirm(Some(value)) = event {
                            this.draft["text"]["style"][key] = json!({"name":value});
                            this.enforce_limit(window, cx);
                            cx.notify();
                        }
                    },
                ));
            }
            this.sync_controls(window, cx);
            this
        })
    }
    fn choices(values: &[Named]) -> Vec<Choice> {
        values
            .iter()
            .map(|n| Choice::new(n.name.clone(), n.name.clone()))
            .collect()
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog.close(window, cx);
        cx.notify();
    }
    fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        self.text.update(cx, |input, cx| {
            input.set_value(
                self.draft["text"]["value"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                window,
                cx,
            )
        });
        for (state, key) in [(&self.font, "fontFamily"), (&self.size, "fontSize")] {
            let value = self.draft["text"]["style"][key]["name"]
                .as_str()
                .expect("normalized Banner choice")
                .to_owned();
            state.update(cx, |select, cx| {
                select.set_selected_value(&value, window, cx)
            });
        }
        self.syncing = false;
    }
    fn measure_duration(&mut self, window: &Window) {
        self.draft["local_duration_ms"] = json!(measure_duration_ms(&self.draft, window));
    }
    fn enforce_limit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let original = self.draft["text"]["value"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        loop {
            let measured = geometry(&self.draft, window);
            let limit = if self.draft["image"]["enabled"] == true {
                spec().limits.vertical_with_image
            } else {
                spec().limits.vertical_without_image
            };
            if !(if measured.horizontal {
                measured.width.round() > spec().limits.horizontal_width
            } else {
                measured.height.round() > limit
            }) {
                break;
            }
            let mut utf16 = self.draft["text"]["value"]
                .as_str()
                .unwrap_or_default()
                .encode_utf16()
                .collect::<Vec<_>>();
            if utf16.pop().is_none() {
                break;
            }
            // Match slice(0,-1) and the displayed replacement for an isolated
            // UTF-16 surrogate; Rust JSON strings cannot retain lone surrogates.
            self.draft["text"]["value"] = json!(String::from_utf16_lossy(&utf16));
            self.warning = true;
        }
        if self.draft["text"]["value"] != original {
            self.measure_duration(window);
            self.sync_controls(window, cx);
        }
    }
    fn animation_elapsed(&self) -> f32 {
        self.carried
            + if self.paused {
                0.
            } else {
                Instant::now()
                    .checked_duration_since(self.epoch)
                    .map_or(0., |d| d.as_secs_f32())
            }
    }
    fn restart(&mut self, delay: bool) {
        self.carried = 0.;
        self.epoch = Instant::now()
            + Duration::from_millis(if delay { spec().motion.restart_ms } else { 0 });
    }
    fn set_scroll(&mut self, scroll: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft["text"]["scroll"] == scroll {
            return;
        }
        self.draft["text"]["scroll"] = json!(scroll);
        self.enforce_limit(window, cx);
        self.measure_duration(window);
        self.restart(false);
        cx.notify();
    }
    fn toggle_style(
        &mut self,
        key: &'static str,
        value: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft["text"]["style"][key] = if self.draft["text"]["style"][key] == value {
            Value::Null
        } else {
            json!(value)
        };
        self.enforce_limit(window, cx);
        cx.notify();
    }
    fn change_size(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let index = spec()
            .sizes
            .iter()
            .position(|n| {
                Some(n.name.as_str()) == self.draft["text"]["style"]["fontSize"]["name"].as_str()
            })
            .unwrap_or(8);
        let next = index
            .saturating_add_signed(delta)
            .min(spec().sizes.len() - 1);
        if next == index {
            return;
        }
        self.draft["text"]["style"]["fontSize"] = json!({"name":spec().sizes[next].name});
        self.enforce_limit(window, cx);
        self.sync_controls(window, cx);
        cx.notify();
    }
    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let old = self.draft["text"].clone();
        let duration = self.draft["local_duration_ms"].clone();
        self.draft = default_value();
        self.draft["local_duration_ms"] = duration;
        if old["value"] != self.draft["text"]["value"]
            || old["scroll"] != self.draft["text"]["scroll"]
        {
            self.measure_duration(window);
        }
        if old["scroll"] != self.draft["text"]["scroll"] {
            self.restart(false);
        }
        self.enforce_limit(window, cx);
        self.sync_controls(window, cx);
        cx.notify();
    }
    fn icon(id: &'static str) -> AnyElement {
        let (w, h) = match id {
            "restart" | "pause" => (24., 24.),
            "increase" | "decrease" => (27., 27.),
            "bold" => (12., 14.),
            "italic" => (13., 14.),
            "underline" => (14., 18.),
            _ => (20., 20.),
        };
        img(spec().icons[id].clone())
            .w(surface::css(w))
            .h(surface::css(h))
            .into_any_element()
    }
    fn option(id: &'static str, selected: bool) -> BaseButton {
        BaseButton::new(format!("1383-banner-{id}"))
            .accessibility_label(id)
            .w(surface::css(45.))
            .h(surface::css(27.))
            .border_1()
            .border_color(if selected {
                Colors::selected()
            } else {
                Colors::border()
            })
            .rounded(surface::css(3.))
            .bg(if selected {
                Colors::selected_surface()
            } else {
                Colors::input()
            })
            .flex()
            .items_center()
            .justify_center()
            .hover(|s| s.border_color(Colors::selected()))
            .child(Self::icon(id))
    }
    fn position(
        &self,
        id: &'static str,
        symbol: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.draft["image"]["position"] == id;
        let progress = motion::transition(
            (id, "1383-banner-radio"),
            if selected { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        let owner = cx.entity().downgrade();
        Radio::new(id)
            .checked(selected)
            .disabled(self.draft["image"]["enabled"] != true)
            .accessibility_label(label(symbol))
            .flex()
            .items_center()
            .line_height(surface::css(20.))
            .text_color(Colors::text())
            .child(
                div()
                    .size(surface::css(20.))
                    .border_1()
                    .border_color(Colors::radio_border())
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(surface::css(10. * progress))
                            .rounded_full()
                            .bg(Colors::selected())
                            .opacity(progress),
                    ),
            )
            .child(div().ml(surface::css(10.)).child(label(symbol)))
            .on_change(move |_, _, _, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.draft["image"]["position"] = json!(id);
                    cx.notify();
                });
            })
            .into_any_element()
    }
    fn config(title: String, body: AnyElement) -> AnyElement {
        v_flex()
            .gap(surface::css(10.))
            .child(
                div()
                    .text_size(surface::css(12.))
                    .child(title.to_uppercase()),
            )
            .child(body)
            .into_any_element()
    }
}
impl Render for BannerEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.dialog.is_open() {
            return div().into_any_element();
        }
        if !self.paused {
            window.request_animation_frame();
        }
        let image_enabled = self.draft["image"]["enabled"] == true;
        let preview_block = v_flex()
            .w(surface::css(236.))
            .flex_shrink_0()
            .child(
                div()
                    .h(surface::css(19.))
                    .p(surface::css(2.))
                    .child(label("$yX").to_uppercase()),
            )
            .child(
                div()
                    .w(surface::css(234.))
                    .h(surface::css(66.))
                    .m(surface::css(1.))
                    .border_1()
                    .border_color(Colors::border())
                    .child(preview(&self.draft, self.animation_elapsed(), window)),
            );
        let playback = v_flex()
            .flex_1()
            .child(
                h_flex()
                    .mb(surface::css(20.))
                    .child(
                        BaseButton::new("1383-banner-restart")
                            .accessibility_label("Restart preview")
                            .mr(surface::css(10.))
                            .child(Self::icon("restart"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.restart(true);
                                cx.notify();
                            })),
                    )
                    .child(
                        BaseButton::new("1383-banner-pause")
                            .accessibility_label("Pause preview")
                            .mr(surface::css(10.))
                            .rounded(surface::css(3.))
                            .when(self.paused, |b| b.bg(Colors::black()))
                            .child(Self::icon("pause"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.carried = this.animation_elapsed();
                                this.paused = !this.paused;
                                self::resume_epoch(this);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                BaseButton::new("1383-banner-reset")
                    .accessibility_label(label("Ufo"))
                    .underline()
                    .text_color(Colors::text())
                    .hover(|s| s.text_color(Colors::selected()))
                    .child(label("Ufo"))
                    .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
            );
        let mut images = v_flex().gap(surface::css(10.));
        for pair in spec().images.chunks(2) {
            let mut row = h_flex().gap(surface::css(10.));
            for image in pair {
                let selected =
                    Some(image.id.as_str()) == self.draft["image"]["selected"]["id"].as_str();
                let id = image.id.clone();
                row = row.child(
                    BaseButton::new(format!("1383-banner-image-{id}"))
                        .accessibility_label(id.clone())
                        .disabled(!image_enabled)
                        .w(surface::css(236.))
                        .h(surface::css(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(surface::css(if selected { 236. } else { 234. }))
                                .h(surface::css(if selected { 24. } else { 22. }))
                                .border(surface::css(if selected { 2. } else { 1. }))
                                .border_color(if selected {
                                    Colors::selected()
                                } else {
                                    Colors::border()
                                })
                                .child(
                                    img(image.asset.clone())
                                        .w(surface::css(232.))
                                        .h(surface::css(20.)),
                                ),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let index = spec()
                                .images
                                .iter()
                                .position(|i| i.id == id)
                                .expect("source banner image");
                            let image = &spec().images[index];
                            this.draft["image"]["selected"] =
                                json!({"id":image.id,"src":image.src});
                            this.draft["image"]["selectedIdx"] = json!(index);
                            cx.notify();
                        })),
                );
            }
            images = images.child(row);
        }
        let positions = v_flex()
            .gap(surface::css(10.))
            .child(
                div()
                    .text_size(surface::css(12.))
                    .child(label("MV7").to_uppercase()),
            )
            .child(self.position("top", "wv5", window, cx))
            .child(self.position("bottom", "_NK", window, cx));
        let image_block = v_flex()
            .mt(surface::css(25.))
            .mb(surface::css(30.))
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .mb(surface::css(5.))
                    .child(label("wLi").to_uppercase())
                    .child(
                        surface::SynapseSwitch::new("1383-banner-image-switch")
                            .accessibility_label(label("wLi"))
                            .checked(image_enabled)
                            .on_change(cx.listener(|this, _, _, cx| {
                                this.draft["image"]["enabled"] =
                                    json!(this.draft["image"]["enabled"] != true);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                h_flex()
                    .gap(surface::css(20.))
                    .when(!image_enabled, |e| e.opacity(0.3))
                    .child(images)
                    .child(positions),
            );
        let style = &self.draft["text"]["style"];
        let measured = geometry(&self.draft, window);
        let focused = self.text.read(cx).focus_handle(cx).is_focused(window);
        let text = Textarea::new(&self.text)
            .appearance(false)
            .bordered(false)
            .aria_label(label("gc6"))
            .w(surface::css(spec().textarea.outer_width))
            .h(surface::css(spec().textarea.outer_height))
            .p(surface::css(spec().textarea.padding))
            .rounded_none()
            .border_1()
            .border_color(if focused {
                Colors::selected()
            } else {
                Colors::border()
            })
            .bg(Colors::input())
            .text_color(Colors::text())
            .font_family(measured.font.family.clone())
            .font_weight(measured.font.weight)
            .text_size(surface::css(measured.size))
            .line_height(surface::css(measured.line_height))
            .when(measured.font.style == FontStyle::Italic, |e| e.italic())
            .when(style["textDecoration"] == "underline", |e| e.underline());
        let text_block = v_flex()
            .flex_shrink_0()
            .w(surface::css(spec().textarea.outer_width))
            .child(
                div()
                    .mb(surface::css(5.))
                    .child(label("gc6").to_uppercase()),
            )
            .child(text)
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .text_size(surface::css(12.))
                    .child(
                        div().text_color(Colors::muted()).child(
                            label("Xpn").replace(
                                "{{length}}",
                                &self.draft["text"]["value"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .encode_utf16()
                                    .count()
                                    .to_string(),
                            ),
                        ),
                    )
                    .when(self.warning, |e| {
                        e.child(div().text_color(Colors::warning()).child(label("erU")))
                    }),
            );
        let fonts = surface::select(&self.font)
            .id("1383-banner-font")
            .items(Self::choices(&spec().fonts))
            .accessibility_label(label("gv6"))
            .w(surface::css(166.))
            .into_any_element();
        let size = h_flex()
            .gap(surface::css(5.))
            .child(
                surface::select(&self.size)
                    .id("1383-banner-size")
                    .items(Self::choices(&spec().sizes))
                    .accessibility_label(label("E_v"))
                    .w(surface::css(100.)),
            )
            .child(
                BaseButton::new("1383-banner-increase")
                    .accessibility_label("Increase font size")
                    .text_color(Colors::text())
                    .hover(|s| s.text_color(Colors::selected()))
                    .child(
                        svg()
                            .path(spec().icons["increase"].clone())
                            .size(surface::css(27.)),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.change_size(1, window, cx))),
            )
            .child(
                BaseButton::new("1383-banner-decrease")
                    .accessibility_label("Decrease font size")
                    .text_color(Colors::text())
                    .hover(|s| s.text_color(Colors::selected()))
                    .child(
                        svg()
                            .path(spec().icons["decrease"].clone())
                            .size(surface::css(27.)),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.change_size(-1, window, cx))),
            );
        let styles = h_flex()
            .gap(surface::css(5.))
            .child(
                Self::option("bold", style["fontWeight"] == "bold").on_click(cx.listener(
                    |this, _, window, cx| this.toggle_style("fontWeight", "bold", window, cx),
                )),
            )
            .child(
                Self::option("italic", style["fontStyle"] == "italic").on_click(cx.listener(
                    |this, _, window, cx| this.toggle_style("fontStyle", "italic", window, cx),
                )),
            )
            .child(
                Self::option("underline", style["textDecoration"] == "underline").on_click(
                    cx.listener(|this, _, window, cx| {
                        this.toggle_style("textDecoration", "underline", window, cx)
                    }),
                ),
            );
        let mut scroll = h_flex().gap(surface::css(5.));
        for id in ["left", "right", "up", "down"] {
            scroll = scroll.child(
                Self::option(id, self.draft["text"]["scroll"] == id).on_click(
                    cx.listener(move |this, _, window, cx| this.set_scroll(id, window, cx)),
                ),
            );
        }
        let controls = h_flex()
            .gap(surface::css(20.))
            .child(
                v_flex()
                    .child(Self::config(label("gv6"), fonts))
                    .child(Self::config(label("WAD"), styles.into_any_element())),
            )
            .child(
                v_flex()
                    .child(Self::config(label("E_v"), size.into_any_element()))
                    .child(Self::config(label("sn9"), scroll.into_any_element())),
            );
        let body = v_flex()
            .id("1383-banner-scroll")
            .min_h_0()
            .flex_1()
            .pt(surface::css(20.))
            .px(surface::css(25.))
            .pb(surface::css(97.))
            .scrollable_y()
            .text_color(Colors::text())
            .text_size(surface::css(14.))
            .child(
                h_flex()
                    .items_end()
                    .gap(surface::css(20.))
                    .child(div().flex_1())
                    .child(preview_block)
                    .child(playback),
            )
            .child(image_block)
            .child(
                h_flex()
                    .gap(surface::css(20.))
                    .mb(surface::css(30.))
                    .child(text_block)
                    .child(controls),
            )
            .child(
                div()
                    .mt(surface::css(-20.))
                    .text_color(Colors::muted())
                    .child(label("BZ0")),
            )
            .into_any_element();
        let cancel = dialog::action("1383-banner-cancel", label("bOp"), false)
            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .into_any_element();
        let apply = dialog::action("1383-banner-apply", label("pJk"), true)
            .on_click(cx.listener(|this, _, window, cx| {
                let mut value = this.draft.clone();
                // The local editor publishes settings; a real device transport owns
                // html2canvas-compatible raster encoding. Never retain a stale bitmap.
                value["imageData"] = json!("");
                let _ = this.owner.update(cx, |owner, cx| {
                    let previous = &owner.draft["oledHome"]["banner"];
                    value["local_duration_ms"] = if previous["text"]["value"]
                        == value["text"]["value"]
                        && previous["text"]["scroll"] == value["text"]["scroll"]
                    {
                        // The Home Hg stays mounted. Font/size/style-only edits
                        // retain its own duration, independent of the editor Hg.
                        previous["local_duration_ms"]
                            .as_f64()
                            .filter(|v| *v > 0.)
                            .map_or_else(
                                || json!(measure_duration_ms(previous, window)),
                                |v| json!(v),
                            )
                    } else {
                        json!(measure_duration_ms(&value, window))
                    };
                    owner.draft["oledHome"]["banner"] = value;
                    cx.emit(AudioProductChanged);
                    cx.notify();
                });
                this.close(window, cx);
            }))
            .into_any_element();
        self.dialog.render(
            "1383-banner",
            label("ADx"),
            body,
            dialog::footer(cancel, apply),
            window,
            cx,
            Self::close,
        )
    }
}
fn resume_epoch(this: &mut BannerEditor) {
    this.epoch = this.epoch.max(Instant::now());
}

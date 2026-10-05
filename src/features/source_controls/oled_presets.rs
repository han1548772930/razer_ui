//! Local preset selection follows OLED wt/Vt. The dialog owns an isolated
//! draft; only Apply changes the device-owned OLED settings.
use super::*;
use crate::{i18n::t, ui::theme::OledColors};
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::radio::Radio;
use serde::Serialize;
use std::sync::Arc;

#[path = "oled_crop.rs"]
mod crop;
#[path = "oled_home_cards.rs"]
mod home_cards;
use crop::{CROP_HEIGHT, CROP_TOP, CropCanvas, HEIGHT, WIDTH};

#[derive(Clone, Copy)]
enum PresetKind {
    Animation,
    Image,
}

const OLED_MEDIA_VISUALIZERS: [&str; 3] = [
    "synapse/oled-home-visualizer-1.webp",
    "synapse/oled-home-visualizer-2.webp",
    "synapse/oled-home-visualizer-3.webp",
];

/// The source `Os` editor keeps media information and visualizer settings in
/// an isolated draft until Apply.  These values only describe the local OLED
/// preview; audio metadata and hardware transfer remain outside this UI.
#[derive(Clone)]
struct OledMediaDraft {
    info_enabled: bool,
    info_position: String,
    visualizer_enabled: bool,
    visualizer_index: usize,
}

impl OledMediaDraft {
    fn from_value(value: &Value) -> Self {
        let info = value.get("info");
        let visualizer = value.get("visualizer");
        Self {
            info_enabled: info
                .and_then(|value| value.get("enabled"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            info_position: info
                .and_then(|value| value.get("selected"))
                .and_then(Value::as_str)
                .unwrap_or("top")
                .to_owned(),
            visualizer_enabled: visualizer
                .and_then(|value| value.get("enabled"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            visualizer_index: visualizer
                .and_then(|value| value.get("selected"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(2) as usize,
        }
    }

    fn value(&self) -> Value {
        serde_json::json!({
            "info": {"enabled": self.info_enabled, "selected": self.info_position},
            "visualizer": {"enabled": self.visualizer_enabled, "selected": self.visualizer_index},
        })
    }

    fn preview_content(&self) -> AnyElement {
        let mut preview = v_flex()
            .w(surface::css(232.))
            .h(surface::css(64.))
            .bg(OledColors::screen())
            .items_center()
            .justify_center()
            .overflow_hidden();
        let media_info = || {
            div()
                .text_color(gpui_kit::rgb(0xcccccc))
                .text_size(surface::css(14.))
                .child("(Track Title - Artiste Name)")
                .into_any_element()
        };
        if self.info_enabled && self.info_position == "top" {
            preview = preview.child(media_info());
        }
        if self.visualizer_enabled {
            preview = preview.child(
                img(OLED_MEDIA_VISUALIZERS[self.visualizer_index])
                    .id(SharedString::from(format!(
                        "oled-media-preview-{}",
                        OLED_MEDIA_VISUALIZERS[self.visualizer_index]
                    )))
                    .w(surface::css(232.))
                    .h(surface::css(44.))
                    .object_fit(ObjectFit::Fill),
            );
        }
        if self.info_enabled && self.info_position == "bottom" {
            preview = preview.child(media_info());
        }
        preview.into_any_element()
    }

    fn preview(&self) -> AnyElement {
        div()
            .w(surface::css(236.))
            .h(surface::css(68.))
            .p(surface::css(1.))
            .child(
                div()
                    .w(surface::css(234.))
                    .h(surface::css(66.))
                    .border_1()
                    .border_color(OledColors::border())
                    .child(self.preview_content()),
            )
            .into_any_element()
    }
}

struct OledMediaEditor {
    draft: OledMediaDraft,
}

impl Render for OledMediaEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let info_enabled = self.draft.info_enabled;
        let visualizer_enabled = self.draft.visualizer_enabled;
        let info_position = self.draft.info_position.clone();
        let selected_visualizer = self.draft.visualizer_index;
        v_flex()
            .w_full()
            .text_color(gpui_kit::rgb(0xcccccc))
            .child(
                h_flex()
                    .items_end()
                    .gap(surface::css(20.))
                    .child(div().flex_1())
                    .child(
                        v_flex()
                            .w(surface::css(236.))
                            .child(
                                div()
                                    .h(surface::css(19.))
                                    .p(surface::css(2.))
                                    .text_size(surface::css(14.))
                                    .child(t("PREVIEW").to_uppercase()),
                            )
                            .child(self.draft.preview()),
                    )
                    .child(
                        div().flex_1().child(
                            div()
                                .id("oled-media-reset")
                                .text_size(surface::css(14.))
                                .underline()
                                .cursor_pointer()
                                .hover(|s| s.text_color(gpui_kit::rgb(0x44d62c)))
                                .child(t("RESET_BTN"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.draft = OledMediaDraft {
                                        info_enabled: true,
                                        info_position: "top".into(),
                                        visualizer_enabled: true,
                                        visualizer_index: 0,
                                    };
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .child(
                v_flex()
                    .gap(surface::css(10.))
                    .mt(surface::css(20.))
                    .mb(surface::css(40.))
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .items_center()
                            .child(
                                div()
                                    .text_size(surface::css(14.))
                                    .child(t("CUSTOMIZE_MEDIA_INFO_LABEL").to_uppercase()),
                            )
                            .child(
                                surface::SynapseSwitch::new("oled-media-info-enabled")
                                    .checked(info_enabled)
                                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                                        if !*value {
                                            this.draft.visualizer_enabled = true;
                                        }
                                        this.draft.info_enabled = *value;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(surface::note(t("CUSTOMIZE_MEDIA_INFO_DESC"), cx))
                    .child(
                        h_flex().gap(surface::css(18.)).children(
                            [("top", "TOP"), ("bottom", "BOTTOM")].into_iter().map(
                                |(value, label)| {
                                    Radio::new(SharedString::from(format!(
                                        "oled-media-info-position-{value}"
                                    )))
                                    .label(t(label))
                                    .checked(info_position == value)
                                    .disabled(!info_enabled)
                                    .on_change(cx.listener(move |this, _, _, cx| {
                                        this.draft.info_position = value.to_owned();
                                        cx.notify();
                                    }))
                                },
                            ),
                        ),
                    ),
            )
            .child(
                v_flex()
                    .gap(surface::css(10.))
                    .mb(surface::css(40.))
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .items_center()
                            .child(
                                div()
                                    .text_size(surface::css(14.))
                                    .child(t("CUSTOMIZE_MEDIA_VISUALIZER_LABEL").to_uppercase()),
                            )
                            .child(
                                surface::SynapseSwitch::new("oled-media-visualizer-enabled")
                                    .checked(visualizer_enabled)
                                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                                        if !*value {
                                            this.draft.info_enabled = true;
                                        }
                                        this.draft.visualizer_enabled = *value;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(surface::note(t("CUSTOMIZE_MEDIA_VISUALIZER_DESC"), cx))
                    .child(
                        h_flex().gap(surface::css(5.)).children(
                            OLED_MEDIA_VISUALIZERS.into_iter().enumerate().map(
                                |(index, source)| {
                                    let selected = selected_visualizer == index;
                                    gpui_kit::base::Button::new(SharedString::from(format!(
                                        "oled-media-visualizer-{index}"
                                    )))
                                    .accessibility_label(format!(
                                        "{} {}",
                                        t("CUSTOMIZE_MEDIA_VISUALIZER_LABEL"),
                                        index + 1,
                                    ))
                                    .disabled(!visualizer_enabled)
                                    .relative()
                                    .w(surface::css(236.))
                                    .h(surface::css(48.))
                                    .p(surface::css(1.))
                                    .border_1()
                                    .border_color(if selected {
                                        if visualizer_enabled {
                                            gpui_kit::rgb(0x44d62c).into()
                                        } else {
                                            gpui_kit::rgba(0x2cd62c4d).into()
                                        }
                                    } else {
                                        OledColors::border()
                                    })
                                    .when(selected, |button| button.border_2().p_0())
                                    .child(
                                        img(source)
                                            .id(SharedString::from(format!(
                                                "oled-media-image-{source}"
                                            )))
                                            .w(surface::css(232.))
                                            .h(surface::css(44.))
                                            .object_fit(ObjectFit::Fill),
                                    )
                                    .when(!visualizer_enabled, |button| {
                                        button.child(
                                            div()
                                                .absolute()
                                                .left_0()
                                                .top_0()
                                                .w(surface::css(232.))
                                                .h(surface::css(44.))
                                                .bg(OledColors::border())
                                                .opacity(0.3),
                                        )
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.draft.visualizer_enabled {
                                            this.draft.visualizer_index = index;
                                            cx.notify();
                                        }
                                    }))
                                },
                            ),
                        ),
                    ),
            )
    }
}
impl PresetKind {
    fn key(self) -> &'static str {
        match self {
            Self::Animation => "animation",
            Self::Image => "image",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Animation => "OLED_HOME_SCREEN_DISPLAY_TITLE_ANIMATION",
            Self::Image => "OLED_HOME_SCREEN_DISPLAY_TITLE_IMAGE",
        }
    }
    fn editor_title(self) -> &'static str {
        match self {
            Self::Animation => "CUSTOMIZE_MODAL_ANIMATION_TITLE",
            Self::Image => "CUSTOMIZE_MODAL_IMAGE_TITLE",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Animation => "CUSTOMIZE_ANIMATION_DESC",
            Self::Image => "CUSTOMIZE_IMAGE_DESC",
        }
    }
    fn asset(self, ix: usize) -> SharedString {
        let ext = match self {
            Self::Animation => "webp",
            Self::Image => "png",
        };
        format!("synapse/oled-home-{}-{}.{ext}", self.key(), ix + 1).into()
    }

    fn accepts_data_url(self, source: &str) -> bool {
        let prefixes: &[&str] = match self {
            Self::Animation => &["data:image/gif;base64,"],
            Self::Image => &[
                "data:image/png;base64,",
                "data:image/jpg;base64,",
                "data:image/jpeg;base64,",
                "data:image/bmp;base64,",
            ],
        };
        prefixes.iter().any(|prefix| {
            source
                .strip_prefix(*prefix)
                .is_some_and(|payload| !payload.is_empty())
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Preset {
    id: String,
    custom: bool,
    enabled: bool,
    /// Local data URL staged by the editor. The native source stores the
    /// processed payload beside the custom flag; it is never sent by this UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    src: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    /// Native preview metadata for the original imported data URL. This is
    /// deliberately separate from the source's processed `src` payload: GIF
    /// processing and device transfer are not connected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    local_crop: Option<CropPlacement>,
}

#[derive(Clone, Deserialize, Serialize)]
struct CropPlacement {
    /// Legacy centred-preview magnification, used only without canvas data.
    zoom: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    canvas: Option<CropCanvas>,
}

impl CropPlacement {
    fn normalized(mut self) -> Self {
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(1., 10.)
        } else {
            1.
        };
        self.canvas = self.canvas.and_then(CropCanvas::normalized);
        self
    }
}

/// The fixed crop box and the saved preset preview use the same placement.
/// Keeping the original data URL plus local geometry makes the local preview
/// reversible without claiming to have encoded a source-compatible payload.
fn cropped_preview(
    id: SharedString,
    source: SharedString,
    crop: Option<&CropPlacement>,
) -> AnyElement {
    cropped_preview_scaled(id, source, crop, 1.)
}

/// jt/Ht scale the already cropped preview around its centre on hover.
fn cropped_preview_scaled(
    id: SharedString,
    source: SharedString,
    crop: Option<&CropPlacement>,
    scale: f32,
) -> AnyElement {
    // GPUI only retains animated frame state for an image with an ElementId.
    // Include content identity so replacing a slot starts its new GIF at zero.
    let image_id: SharedString = format!("{id}:{}", gpui_kit::hash(&source)).into();
    let zoom = crop.map_or(1., |crop| crop.clone().normalized().zoom);
    let canvas = crop
        .and_then(|crop| crop.canvas)
        .and_then(CropCanvas::normalized);
    let (left, top, width, height) = canvas.map_or(
        (
            (WIDTH - WIDTH * zoom) / 2.,
            (CROP_HEIGHT - CROP_HEIGHT * zoom) / 2.,
            WIDTH * zoom,
            CROP_HEIGHT * zoom,
        ),
        |canvas| {
            (
                canvas.left(),
                canvas.top() - CROP_TOP,
                canvas.width(),
                canvas.height(),
            )
        },
    );
    div()
        .relative()
        .w(surface::css(232.))
        .h(surface::css(64.))
        .flex_shrink_0()
        .bg(OledColors::screen())
        .overflow_hidden()
        .child(
            img(crop::preview_source(image_id.clone(), source))
                .id(image_id)
                .absolute()
                .left(surface::css((left - WIDTH / 2.) * scale + WIDTH / 2.))
                .top(surface::css(
                    (top - CROP_HEIGHT / 2.) * scale + CROP_HEIGHT / 2.,
                ))
                .w(surface::css(width * scale))
                .h(surface::css(height * scale))
                .grayscale(true)
                .object_fit(if canvas.is_some() {
                    ObjectFit::Fill
                } else {
                    ObjectFit::Contain
                }),
        )
        .into_any_element()
}
#[derive(Clone, Deserialize, Serialize)]
struct PresetSelection {
    #[serde(rename = "selectedIdx")]
    selected_ix: usize,
    list: Vec<Preset>,
}
impl PresetSelection {
    fn normalize(&mut self) {
        // Older local drafts may contain no enabled item or an invalid index.
        if !self.list.iter().any(|item| item.enabled) {
            if let Some(first) = self.list.first_mut() {
                first.enabled = true;
            }
        }
        if !self.list.get(self.selected_ix).is_some_and(|p| p.enabled) {
            self.selected_ix = self.list.iter().position(|p| p.enabled).unwrap_or(0);
        }
    }
    fn set_enabled(&mut self, ix: usize, enabled: bool) {
        if !enabled && self.list.iter().filter(|p| p.enabled).count() == 1 {
            return;
        }
        if let Some(item) = self.list.get_mut(ix) {
            item.enabled = enabled;
            self.normalize();
        }
    }
}

fn restore_preset_selection(target: &mut PresetSelection, saved: Option<&Value>, kind: PresetKind) {
    let Some(saved_list) = saved
        .and_then(|value| value.get("list"))
        .and_then(Value::as_array)
    else {
        return;
    };
    target.selected_ix = saved
        .and_then(|value| value.get("selectedIdx"))
        .and_then(Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
        .unwrap_or(target.selected_ix);
    for (index, item) in target.list.iter_mut().enumerate() {
        let Some(saved_item) = saved_list.get(index) else {
            continue;
        };
        if saved_item.get("id").and_then(Value::as_str) != Some(item.id.as_str()) {
            continue;
        }
        if let Some(enabled) = saved_item.get("enabled").and_then(Value::as_bool) {
            item.enabled = enabled;
        }
        let source = saved_item
            .get("src")
            .and_then(Value::as_str)
            .filter(|source| kind.accepts_data_url(source));
        item.custom =
            saved_item.get("custom").and_then(Value::as_bool) == Some(true) && source.is_some();
        if item.custom {
            item.src = source.map(str::to_owned);
            item.size = saved_item.get("size").and_then(Value::as_u64);
            item.local_crop = saved_item
                .get("local_crop")
                .and_then(|crop| serde_json::from_value::<CropPlacement>(crop.clone()).ok())
                .map(CropPlacement::normalized);
        }
    }
    target.normalize();
}

struct PresetEditor {
    kind: PresetKind,
    selection: PresetSelection,
    import_generation: u64,
    hovered: Option<usize>,
}

/// Local crop draft used by the source's upload flow. The original cropper
/// fixes the OLED canvas to 232x64 (aspect 3.625), keeps the crop box fixed,
/// and exposes zoom/reset controls. We preserve that surface and stage the
/// selected data URL locally; transport and GIF frame processing remain a
/// service boundary.
struct CropDraft {
    source: SharedString,
    preview: Arc<RenderImage>,
    index: usize,
    canvas: CropCanvas,
    initial_canvas: CropCanvas,
    drag_position: Option<Point<Pixels>>,
    focus: FocusHandle,
    zoom_level: u8,
    zoom_slider: Entity<SliderState>,
    _subscriptions: Vec<Subscription>,
    kind: PresetKind,
}

impl CropDraft {
    fn new(
        source: SharedString,
        preview: Arc<RenderImage>,
        index: usize,
        kind: PresetKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let zoom_slider = cx.new(|_| {
            SliderState::new()
                .min(1.)
                .max(10.)
                .step(1.)
                .default_value(1.)
        });
        let subscription = cx.subscribe_in(&zoom_slider, window, |this, _, event, _, cx| {
            let SliderEvent::Change(value) = event else {
                return;
            };
            let next = value.start().round().clamp(1., 10.) as u8;
            if next != this.zoom_level {
                // OLED `xe` passes directional values to Cropper.zoom; the
                // slider position is not an absolute magnification factor.
                let delta = crop_slider_delta(this.zoom_level, next);
                this.canvas.zoom_by(delta);
                this.zoom_level = next;
                cx.notify();
            }
        });
        let dimensions = preview.size(0);
        let canvas = CropCanvas::new(dimensions.width.0 as f32, dimensions.height.0 as f32);
        Self {
            source,
            preview,
            index,
            canvas,
            initial_canvas: canvas,
            drag_position: None,
            focus: cx.focus_handle().tab_stop(true),
            zoom_level: 1,
            zoom_slider,
            _subscriptions: vec![subscription],
            kind,
        }
    }

    fn step_zoom(&mut self, increment: bool, window: &mut Window, cx: &mut Context<Self>) {
        if (increment && self.zoom_level == 10) || (!increment && self.zoom_level == 1) {
            return;
        }
        self.zoom_level = if increment {
            self.zoom_level + 1
        } else {
            self.zoom_level - 1
        };
        self.canvas.zoom_by(if increment { 0.1 } else { -0.1 });
        let level = self.zoom_level as f32;
        self.zoom_slider
            .update(cx, |slider, cx| slider.set_value(level, window, cx));
        cx.notify();
    }

    fn drag_canvas(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(previous) = self.drag_position else {
            return;
        };
        if !event.dragging() {
            self.drag_position = None;
        } else {
            let scale = f32::from(surface::css(1.).to_pixels(window.rem_size()));
            self.canvas.move_by(
                f32::from(event.position.x - previous.x) / scale,
                f32::from(event.position.y - previous.y) / scale,
            );
            // Cropper updates pointer start coordinates after every event,
            // including a move clamped against a canvas boundary.
            self.drag_position = Some(event.position);
        }
        cx.notify();
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag_position.take().is_some() {
            cx.notify();
        }
    }
}

fn crop_slider_delta(previous: u8, next: u8) -> f32 {
    if next < previous {
        (next as f32 - 11.) / 10.
    } else if next > previous {
        next as f32 / 10.
    } else {
        0.
    }
}

fn crop_frame(focused: bool, cx: &App) -> AnyElement {
    let accent = cx.theme().primary;
    div()
        .absolute()
        .left_0()
        .top_0()
        .size_full()
        .child(
            div()
                .absolute()
                .left(surface::css(-1.))
                .top(surface::css(CROP_TOP - 1.))
                .w(surface::css(WIDTH + 2.))
                .h(surface::css(CROP_HEIGHT + 2.))
                .border_1()
                .border_dashed()
                .border_color(accent),
        )
        .children(
            [(false, false), (true, false), (false, true), (true, true)]
                .into_iter()
                .map(|(right, bottom)| {
                    div()
                        .absolute()
                        .w(surface::css(14.))
                        .h(surface::css(14.))
                        .left(surface::css(if right { WIDTH + 3. - 14. } else { -3. }))
                        .top(surface::css(if bottom {
                            CROP_TOP + CROP_HEIGHT + 3. - 14.
                        } else {
                            CROP_TOP - 3.
                        }))
                        .border_color(accent)
                        .when(right, |view| view.border_r(surface::css(3.)))
                        .when(!right, |view| view.border_l(surface::css(3.)))
                        .when(bottom, |view| view.border_b(surface::css(3.)))
                        .when(!bottom, |view| view.border_t(surface::css(3.)))
                }),
        )
        .when(focused, |view| {
            view.child(
                div()
                    .absolute()
                    .left(surface::css(-5.))
                    .top(surface::css(-5.))
                    .w(surface::css(WIDTH + 10.))
                    .h(surface::css(HEIGHT + 10.))
                    .border_1()
                    .border_color(cx.theme().ring),
            )
        })
        .into_any_element()
}

impl Render for CropDraft {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let canvas = self.canvas;
        let pointer = cx.weak_entity();
        let level = self.zoom_level;
        v_flex()
            .gap(surface::css(10.))
            .child(
                h_flex().justify_center().child(
                    div()
                        .id("oled-crop-canvas")
                        .role(Role::Group)
                        .aria_label(t("ANIMATION_CROPPER_TITLE"))
                        .aria_keyshortcuts("ArrowUp ArrowDown ArrowLeft ArrowRight")
                        .track_focus(&self.focus)
                        .relative()
                        .w(surface::css(WIDTH))
                        .h(surface::css(HEIGHT))
                        .cursor(if self.drag_position.is_some() {
                            CursorStyle::ClosedHand
                        } else {
                            CursorStyle::OpenHand
                        })
                        .child(
                            div()
                                .size_full()
                                .relative()
                                .overflow_hidden()
                                .child(
                                    img(self.preview.clone())
                                        .id(("oled-crop-image", self.preview.id.0))
                                        .absolute()
                                        .left(surface::css(canvas.left()))
                                        .top(surface::css(canvas.top()))
                                        .w(surface::css(canvas.width()))
                                        .h(surface::css(canvas.height()))
                                        .object_fit(ObjectFit::Fill)
                                        .grayscale(true),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .top_0()
                                        .w_full()
                                        .h(surface::css(CROP_TOP))
                                        .bg(OledColors::screen())
                                        .opacity(0.5),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .bottom_0()
                                        .w_full()
                                        .h(surface::css(CROP_TOP))
                                        .bg(OledColors::screen())
                                        .opacity(0.5),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .top(surface::css(CROP_TOP))
                                        .w_full()
                                        .h(surface::css(CROP_HEIGHT))
                                        .bg(OledColors::crop_face())
                                        .opacity(0.1),
                                ),
                        )
                        .child(crop_frame(self.focus.is_focused(window), cx))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                this.focus.focus(window, cx);
                                this.drag_position = Some(event.position);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.end_drag(cx)),
                        )
                        .on_mouse_up_out(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.end_drag(cx)),
                        )
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            let delta = match event.keystroke.key.as_str() {
                                "left" => (-1., 0.),
                                "right" => (1., 0.),
                                "up" => (0., -1.),
                                "down" => (0., 1.),
                                _ => return,
                            };
                            let step = if event.keystroke.modifiers.shift {
                                10.
                            } else {
                                1.
                            };
                            this.canvas.move_by(delta.0 * step, delta.1 * step);
                            window.prevent_default();
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .child(
                            gpui_kit::canvas(
                                |_, _, _| {},
                                move |_, _, window, cx| {
                                    let Some(entity) = pointer.upgrade() else {
                                        return;
                                    };
                                    if entity.read(cx).drag_position.is_none() {
                                        return;
                                    }
                                    let motion = entity.clone();
                                    window.on_mouse_event(
                                        move |event: &MouseMoveEvent, phase, window, cx| {
                                            if phase.capture() {
                                                motion.update(cx, |this, cx| {
                                                    this.drag_canvas(event, window, cx)
                                                });
                                            }
                                        },
                                    );
                                    window.on_mouse_event(
                                        move |event: &MouseUpEvent, phase, _, cx| {
                                            if phase.capture() && event.button == MouseButton::Left
                                            {
                                                entity.update(cx, |this, cx| this.end_drag(cx));
                                            }
                                        },
                                    );
                                },
                            )
                            .absolute()
                            .size_0(),
                        ),
                ),
            )
            .child(
                h_flex()
                    .justify_center()
                    .items_center()
                    .child(
                        Button::new("oled-crop-zoom-out")
                            .label("−")
                            .accessibility_label(t("ZOOM_OUT"))
                            .outline()
                            .disabled(level == 1)
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.step_zoom(false, window, cx)
                                }),
                            ),
                    )
                    .child(
                        div()
                            .w(surface::css(150.))
                            .mx(surface::css(10.))
                            .mt(surface::css(4.))
                            .child(Slider::new(&self.zoom_slider)),
                    )
                    .child(
                        Button::new("oled-crop-zoom-in")
                            .label("+")
                            .accessibility_label(t("ZOOM_IN"))
                            .outline()
                            .disabled(level == 10)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.step_zoom(true, window, cx)),
                            ),
                    ),
            )
            .child(
                h_flex().justify_center().child(
                    Button::new("oled-crop-reset")
                        .label(t("RESET"))
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.canvas = this.initial_canvas;
                            this.drag_position = None;
                            this.zoom_level = 1;
                            this.zoom_slider
                                .update(cx, |slider, cx| slider.set_value(1., window, cx));
                            cx.notify();
                        })),
                ),
            )
            .when(matches!(self.kind, PresetKind::Animation), |view| {
                view.child(
                    div()
                        .text_size(surface::css(12.))
                        .line_height(surface::css(15.))
                        .text_color(OledColors::muted())
                        .child(t("ANIMATION_CROPPER_INFO")),
                )
            })
    }
}

impl Render for PresetEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let last_enabled = self.selection.list.iter().filter(|p| p.enabled).count() == 1;
        v_flex()
            .w_full()
            .text_color(rgb(0xcccccc))
            .text_size(surface::css(14.))
            .child(t(self.kind.description()))
            .child(
                div()
                    .grid()
                    .grid_cols(3)
                    .gap(surface::css(20.))
                    .mt(surface::css(30.))
                    .children(self.selection.list.iter().enumerate().map(|(ix, preset)| {
                        let selected = self.selection.selected_ix == ix;
                        let enabled = preset.enabled;
                        let id = preset.id.clone();
                        let custom = preset.custom;
                        let hovered = self.hovered == Some(ix);
                        let strong_border = enabled && (selected || hovered);
                        let source = preset
                            .src
                            .clone()
                            .unwrap_or_else(|| self.kind.asset(ix).to_string());
                        div()
                            .w(surface::css(236.))
                            .h(surface::css(68.))
                            .flex_shrink_0()
                            .child(
                                div()
                                    .id(SharedString::from(format!("preset-{id}")))
                                    .relative()
                                    .w(surface::css(if strong_border { 236. } else { 234. }))
                                    .h(surface::css(if strong_border { 68. } else { 66. }))
                                    .m(surface::css(if selected || hovered { 0. } else { 1. }))
                                    .border_1()
                                    .when(strong_border, |card| card.border_2())
                                    .border_color(if !enabled {
                                        rgba(0x5f5f5f4d)
                                    } else if selected {
                                        rgba(0x44d62cff)
                                    } else if hovered {
                                        rgba(0x44d62c4d)
                                    } else {
                                        rgba(0x5f5f5fff)
                                    })
                                    .bg(OledColors::screen())
                                    .overflow_hidden()
                                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                        if *hovered {
                                            this.hovered = Some(ix);
                                        } else if this.hovered == Some(ix) {
                                            this.hovered = None;
                                        }
                                        cx.notify();
                                    }))
                                    .child(
                                        div().when(!enabled, |preview| preview.opacity(0.1)).child(
                                            cropped_preview_scaled(
                                                format!("oled-editor-{id}").into(),
                                                SharedString::from(source),
                                                preset.local_crop.as_ref(),
                                                if hovered { 1.1 } else { 1. },
                                            ),
                                        ),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.selection.list[ix].enabled {
                                            this.selection.selected_ix = ix;
                                            cx.notify();
                                        }
                                    }))
                                    .when(hovered, |card| {
                                        card.child(
                                            v_flex()
                                                .absolute()
                                                .left_0()
                                                .top_0()
                                                .w(surface::css(232.))
                                                .h(surface::css(64.))
                                                .p(surface::css(2.))
                                                .justify_between()
                                                .bg(rgba(0x00000080))
                                                .child(
                                                    h_flex()
                                                        .justify_between()
                                                        .items_start()
                                                        .child(
                                                            div().m(surface::css(2.)).child(
                                                                surface::SynapseSwitch::new(
                                                                    SharedString::from(format!(
                                                                        "enable-{id}"
                                                                    )),
                                                                )
                                                                .accessibility_label(format!(
                                                                    "{} {}",
                                                                    t(self.kind.title()),
                                                                    ix + 1
                                                                ))
                                                                .checked(enabled)
                                                                .disabled(last_enabled && enabled)
                                                                .on_change(cx.listener(
                                                                    move |this, enabled, _, cx| {
                                                                        cx.stop_propagation();
                                                                        this.selection.set_enabled(
                                                                            ix, *enabled,
                                                                        );
                                                                        cx.notify();
                                                                    },
                                                                )),
                                                            ),
                                                        )
                                                        .when(enabled, |row| {
                                                            row.child(
                                                                preset_icon_button(
                                                                    format!("import-{id}"),
                                                                    "REPLACE",
                                                                    "synapse/oled-691-replace.svg",
                                                                )
                                                                .on_click(cx.listener(
                                                                    move |this, _, window, cx| {
                                                                        this.import_custom(
                                                                            ix, window, cx,
                                                                        );
                                                                    },
                                                                )),
                                                            )
                                                        }),
                                                )
                                                .when(custom && enabled, |layer| {
                                                    layer.child(
                                                        preset_icon_button(
                                                            format!("reset-{id}"),
                                                            "RESET_BTN",
                                                            "synapse/oled-691-reset.svg",
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                if let Some(item) = this
                                                                    .selection
                                                                    .list
                                                                    .get_mut(ix)
                                                                    .filter(|item| item.enabled)
                                                                {
                                                                    item.custom = false;
                                                                    item.src = None;
                                                                    item.size = None;
                                                                    item.local_crop = None;
                                                                }
                                                                cx.notify();
                                                            },
                                                        )),
                                                    )
                                                }),
                                        )
                                    }),
                            )
                    })),
            )
    }
}

fn preset_icon_button(
    id: String,
    label: &'static str,
    asset: &'static str,
) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(SharedString::from(id))
        .accessibility_label(t(label))
        .tooltip(move |window, cx| tooltip::Tooltip::new(t(label)).build(window, cx))
        .w(surface::css(28.))
        .h(surface::css(27.))
        .m(surface::css(2.))
        .p_0()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(surface::css(5.))
        .border_1()
        .border_color(rgb(0xffffff))
        .bg(rgb(0))
        .text_color(rgb(0xcccccc))
        .hover(|s| s.border_color(rgb(0x44d62c)).text_color(rgb(0x44d62c)))
        .child(svg().path(asset).size(surface::css(20.)))
}

impl PresetEditor {
    fn import_custom(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.import_generation = self.import_generation.wrapping_add(1);
        let generation = self.import_generation;
        let prompt = match self.kind {
            PresetKind::Animation => "Choose a GIF animation".into(),
            PresetKind::Image => "Choose an OLED image".into(),
        };
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(prompt),
        });
        let kind = self.kind;
        let renderer = cx.svg_renderer();
        let parent = cx.weak_entity();
        cx.spawn_in(window, async move |_, cx| {
            let path = match picker.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let Some(path) = path else {
                return;
            };
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let allowed = match kind {
                PresetKind::Animation => extension == "gif",
                PresetKind::Image => {
                    matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "bmp")
                }
            };
            if !allowed {
                return;
            }
            // Disk I/O and base64 encoding can be substantial for imported
            // animations. Only the completed draft returns to the UI thread.
            let Ok(imported) = cx
                .background_spawn(async move { crop::load_preset(&path, &extension, renderer) })
                .await
            else {
                return;
            };
            _ = parent.update_in(cx, |this, window, cx| {
                // A later file selection supersedes an earlier, slower read.
                if this.import_generation != generation {
                    return;
                }
                if this
                    .selection
                    .list
                    .get(index)
                    .is_some_and(|item| item.enabled)
                {
                    this.open_crop(index, kind, imported, window, cx);
                }
            });
        })
        .detach();
    }

    fn open_crop(
        &mut self,
        index: usize,
        kind: PresetKind,
        imported: crop::ImportedPreset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (source, size, preview) = imported.into_parts();
        let crop = cx.new(|cx| CropDraft::new(source.into(), preview, index, kind, window, cx));
        let parent = cx.weak_entity();
        let width = surface::css(640.).to_pixels(window.rem_size());
        window.open_dialog(cx, move |dialog, _, _| {
            let crop_for_apply = crop.clone();
            let parent = parent.clone();
            dialog
                .title(t("ANIMATION_CROPPER_TITLE"))
                .width(width)
                .child(crop.clone())
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("oled-crop-cancel")
                                .label(t("CANCEL"))
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("oled-crop-apply")
                                .label(t("CROPPER_CROP_BTN"))
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let source = crop_for_apply.read(cx).source.clone();
                                    let index = crop_for_apply.read(cx).index;
                                    let placement = CropPlacement {
                                        zoom: 1.,
                                        canvas: Some(crop_for_apply.read(cx).canvas),
                                    }
                                    .normalized();
                                    let _ = parent.update(cx, |this, cx| {
                                        if let Some(item) = this
                                            .selection
                                            .list
                                            .get_mut(index)
                                            .filter(|item| item.enabled)
                                        {
                                            item.custom = true;
                                            item.src = Some(source.to_string());
                                            item.size = Some(size);
                                            item.local_crop = Some(placement);
                                        }
                                        cx.notify();
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }
}

/// Tiny dependency-free base64 encoder for local preview data URLs.
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let value = (a << 16) | (b << 8) | c;
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

impl SourceControls {
    /// `merge_known` intentionally ignores extra keys, while preset defaults
    /// have no imported payload. Restore the whitelisted OLED custom fields
    /// against the current source's fixed preset IDs instead of allowing a
    /// saved object to replace the product schema.
    pub(super) fn restore_oled_custom_presets(&mut self, saved: &Value) {
        if self.spec.product_id != 691 {
            return;
        }
        for kind in [PresetKind::Animation, PresetKind::Image] {
            let mut selection: PresetSelection =
                serde_json::from_value(self.spec.profile["oled"][kind.key()].clone())
                    .expect("validated OLED preset defaults");
            restore_preset_selection(
                &mut selection,
                saved.pointer(&format!("/oled/{}", kind.key())),
                kind,
            );
            self.draft["oled"][kind.key()] =
                serde_json::to_value(selection).expect("OLED selection serializes");
        }
    }

    fn preset_selection(&self, kind: PresetKind) -> PresetSelection {
        let mut selection: PresetSelection =
            serde_json::from_value(self.draft["oled"][kind.key()].clone())
                .expect("validated OLED preset selection");
        selection.normalize();
        selection
    }

    fn open_oled_media(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_ble
            || self.page != "OLED"
            || self.draft.pointer("/oled/homeScreenDisplay/enabled") != Some(&Value::Bool(true))
        {
            return;
        }
        let editor = cx.new(|_| OledMediaEditor {
            draft: OledMediaDraft::from_value(
                self.draft.pointer("/oled/media").unwrap_or(&Value::Null),
            ),
        });
        let editor_for_apply = editor.clone();
        let parent = cx.weak_entity();
        let width = surface::css(850.).to_pixels(window.rem_size());
        window.open_dialog(cx, move |dialog, _, _| {
            let editor_for_apply = editor_for_apply.clone();
            let parent = parent.clone();
            dialog
                .title(t("CUSTOMIZE_MODAL_AUDIO_METER_TITLE"))
                .width(width)
                .child(editor.clone())
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("oled-media-cancel")
                                .label(t("CANCEL"))
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("oled-media-apply")
                                .label(t("APPLY"))
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let media = editor_for_apply.read(cx).draft.value();
                                    let _ = parent.update(cx, |this, cx| {
                                        if !this.is_ble
                                            && this.page == "OLED"
                                            && this.draft.pointer("/oled/homeScreenDisplay/enabled")
                                                == Some(&Value::Bool(true))
                                        {
                                            this.draft["oled"]["media"] = media;
                                            cx.emit(SourceControlsChanged);
                                            cx.notify();
                                        }
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    fn open_oled_presets(&mut self, kind: PresetKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_ble
            || self.draft.pointer("/oled/homeScreenDisplay/enabled") != Some(&Value::Bool(true))
        {
            return;
        }
        let selection = self.preset_selection(kind);
        let editor = cx.new(|_| PresetEditor {
            kind,
            selection,
            import_generation: 0,
            hovered: None,
        });
        let parent = cx.weak_entity();
        let width = surface::css(800.).to_pixels(window.rem_size());
        window.open_dialog(cx, move |dialog, _, _| {
            let editor_for_apply = editor.clone();
            let parent = parent.clone();
            dialog
                .title(t(kind.editor_title()))
                .width(width)
                .child(editor.clone())
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("oled-preset-cancel")
                                .label(t("CANCEL"))
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("oled-preset-apply")
                                .label(t("APPLY"))
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let selection = editor_for_apply.read(cx).selection.clone();
                                    let _ = parent.update(cx, |this, cx| {
                                        if !this.is_ble
                                            && this.page == "OLED"
                                            && this.draft.pointer("/oled/homeScreenDisplay/enabled")
                                                == Some(&Value::Bool(true))
                                        {
                                            this.draft["oled"][kind.key()] =
                                                serde_json::to_value(selection)
                                                    .expect("OLED selection serializes");
                                            cx.emit(SourceControlsChanged);
                                            cx.notify();
                                        }
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{CropPlacement, Preset, PresetKind, PresetSelection, restore_preset_selection};

    fn selection() -> PresetSelection {
        PresetSelection {
            selected_ix: 1,
            list: (0..3)
                .map(|ix| Preset {
                    id: format!("image-{}", ix + 1),
                    custom: false,
                    enabled: true,
                    src: None,
                    size: None,
                    local_crop: None,
                })
                .collect(),
        }
    }

    #[test]
    fn disabling_selected_preset_moves_to_first_enabled() {
        let mut draft = selection();
        draft.set_enabled(1, false);
        assert_eq!(draft.selected_ix, 0);
        draft.set_enabled(0, false);
        assert_eq!(draft.selected_ix, 2);
        draft.set_enabled(2, false);
        assert!(draft.list[2].enabled);
        assert_eq!(draft.selected_ix, 2);
    }

    #[test]
    fn editing_clone_does_not_change_committed_selection() {
        let committed = selection();
        let mut draft = committed.clone();
        draft.set_enabled(1, false);
        assert_eq!(committed.selected_ix, 1);
        assert!(committed.list.iter().all(|p| p.enabled));
    }

    #[test]
    fn malformed_saved_selection_recovers_enabled_item() {
        let mut draft = selection();
        draft.selected_ix = usize::MAX;
        draft.list.iter_mut().for_each(|p| p.enabled = false);
        draft.normalize();
        assert_eq!(draft.selected_ix, 0);
        assert!(draft.list[0].enabled);
        draft.set_enabled(2, true);
        assert_eq!(draft.selected_ix, 0);
        assert_eq!(serde_json::to_value(&draft).unwrap()["selectedIdx"], 0);
    }

    #[test]
    fn imported_crop_survives_schema_merge_and_saved_draft_restore() {
        let defaults = selection();
        let mut saved_selection = defaults.clone();
        saved_selection.selected_ix = 2;
        let imported = &mut saved_selection.list[2];
        imported.custom = true;
        imported.src = Some("data:image/png;base64,iVBORw0KGgo=".into());
        imported.size = Some(8);
        imported.local_crop = Some(CropPlacement {
            zoom: 1.8,
            canvas: None,
        });
        saved_selection.set_enabled(1, false);
        let saved = serde_json::to_value(saved_selection).unwrap();

        // Exercise the actual failure: generic schema merge cannot create the
        // optional payload keys absent from current source preset defaults.
        let mut merged = serde_json::to_value(&defaults).unwrap();
        super::super::merge_known(&mut merged, &saved);
        assert!(merged["list"][2].get("src").is_none());
        let mut restored: PresetSelection = serde_json::from_value(merged).unwrap();
        restore_preset_selection(&mut restored, Some(&saved), PresetKind::Image);

        assert_eq!(restored.selected_ix, 2);
        assert!(!restored.list[1].enabled);
        let imported = &restored.list[2];
        assert!(imported.custom);
        assert_eq!(
            imported.src.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
        assert_eq!(imported.size, Some(8));
        assert_eq!(imported.local_crop.as_ref().unwrap().zoom, 1.8);
    }

    #[test]
    fn malformed_custom_record_keeps_current_product_preset_schema() {
        let mut restored = selection();
        let saved = serde_json::json!({
            "selectedIdx": 90,
            "list": [
                {"id": "animation-1", "enabled": false, "custom": true,
                    "src": "data:image/gif;base64,R0lGODlh", "size": 6},
                {"id": "image-2", "enabled": false, "custom": true,
                    "src": "https://example.invalid/image.png", "size": 8},
                {"id": "image-3", "enabled": false, "custom": true,
                    "src": "data:image/png;base64,iVBORw0KGgo=", "size": 8,
                    "local_crop": {"zoom": 200}},
            ],
        });
        restore_preset_selection(&mut restored, Some(&saved), PresetKind::Image);
        assert_eq!(restored.list.len(), 3);
        assert_eq!(restored.list[0].id, "image-1");
        assert!(restored.list[0].enabled);
        assert!(!restored.list[0].custom);
        assert!(!restored.list[1].custom);
        assert!(restored.list[1].src.is_none());
        assert_eq!(restored.list[2].local_crop.as_ref().unwrap().zoom, 10.);
        assert_eq!(restored.selected_ix, 0);
    }
}

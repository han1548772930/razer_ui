//! Local preset selection follows OLED wt/Vt. The dialog owns an isolated
//! draft; only Apply changes the device-owned OLED settings.
use super::*;
use crate::{i18n::t, ui::theme::OledColors};
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::radio::Radio;
use serde::Serialize;
use std::path::Path;

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

    fn preview(&self) -> AnyElement {
        let mut preview = v_flex()
            .w(surface::css(234.))
            .h(surface::css(66.))
            .bg(OledColors::screen())
            .border_1()
            .border_color(OledColors::border())
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
                    .w(surface::css(232.))
                    .h(surface::css(44.))
                    .object_fit(ObjectFit::Fill),
            );
        }
        if self.info_enabled && self.info_position == "bottom" {
            preview = preview.child(media_info());
        }
        div()
            .w(surface::css(236.))
            .h(surface::css(68.))
            .p(surface::css(1.))
            .child(preview)
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
    zoom: f32,
}

impl CropPlacement {
    fn normalized(mut self) -> Self {
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(1., 10.)
        } else {
            1.
        };
        self
    }
}

/// The fixed crop box and the saved preset preview use the same placement.
/// Keeping the original data URL plus local geometry makes the local preview
/// reversible without claiming to have encoded a source-compatible payload.
fn cropped_preview(source: SharedString, crop: Option<&CropPlacement>) -> AnyElement {
    let zoom = crop.map_or(1., |crop| crop.clone().normalized().zoom);
    div()
        .relative()
        .w(surface::css(232.))
        .h(surface::css(64.))
        .flex_shrink_0()
        .bg(OledColors::screen())
        .overflow_hidden()
        .child(
            img(source)
                .absolute()
                .left(surface::css((232. - 232. * zoom) / 2.))
                .top(surface::css((64. - 64. * zoom) / 2.))
                .w(surface::css(232. * zoom))
                .h(surface::css(64. * zoom))
                .object_fit(ObjectFit::Contain),
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
}

/// Local crop draft used by the source's upload flow. The original cropper
/// fixes the OLED canvas to 232x64 (aspect 3.625), keeps the crop box fixed,
/// and exposes zoom/reset controls. We preserve that surface and stage the
/// selected data URL locally; transport and GIF frame processing remain a
/// service boundary.
struct CropDraft {
    source: SharedString,
    index: usize,
    zoom: f32,
    kind: PresetKind,
}

impl CropDraft {
    fn zoom_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.zoom = (self.zoom + delta).clamp(1., 10.);
        cx.notify();
    }
}

impl Render for CropDraft {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = self.source.clone();
        let zoom = self.zoom;
        v_flex()
            .gap(surface::css(10.))
            .child(
                h_flex()
                    .w(surface::css(580.))
                    .h(surface::css(190.))
                    .bg(OledColors::screen())
                    .border_1()
                    .border_color(OledColors::border())
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .child(
                        div()
                            .border_1()
                            .border_color(OledColors::border())
                            .child(cropped_preview(source, Some(&CropPlacement { zoom }))),
                    ),
            )
            .child(
                h_flex()
                    .gap(surface::css(8.))
                    .items_center()
                    .child(
                        Button::new("oled-crop-zoom-out")
                            .label("−")
                            .outline()
                            .disabled(zoom <= 1.)
                            .on_click(cx.listener(|this, _, _, cx| this.zoom_by(-0.1, cx))),
                    )
                    .child(
                        div()
                            .w(surface::css(58.))
                            .text_center()
                            .text_size(surface::css(12.))
                            .child(format!("{:.1}×", zoom)),
                    )
                    .child(
                        Button::new("oled-crop-zoom-in")
                            .label("+")
                            .outline()
                            .disabled(zoom >= 10.)
                            .on_click(cx.listener(|this, _, _, cx| this.zoom_by(0.1, cx))),
                    )
                    .child(
                        Button::new("oled-crop-reset")
                            .label(t("RESET"))
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.zoom = 1.;
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
        h_flex()
            .w_full()
            .items_start()
            .flex_wrap()
            .gap(surface::css(20.))
            .children(self.selection.list.iter().enumerate().map(|(ix, preset)| {
                let selected = self.selection.selected_ix == ix;
                let enabled = preset.enabled;
                let id = preset.id.clone();
                let custom = preset.custom;
                let source = preset
                    .src
                    .clone()
                    .unwrap_or_else(|| self.kind.asset(ix).to_string());
                v_flex()
                    .w(surface::css(236.))
                    .gap_1()
                    .child(
                        gpui_kit::base::Button::new(SharedString::from(format!("select-{id}")))
                            .accessibility_label(format!("{} {}", t(self.kind.title()), ix + 1))
                            .disabled(!enabled)
                            .w(surface::css(236.))
                            .h(surface::css(68.))
                            .border_2()
                            .border_color(if selected {
                                cx.theme().primary
                            } else {
                                OledColors::border()
                            })
                            .bg(OledColors::screen())
                            .hover(|s| s.border_color(cx.theme().primary))
                            .focus_visible(|s| s.border_color(cx.theme().primary))
                            .child(div().when(!enabled, |preview| preview.opacity(0.1)).child(
                                cropped_preview(
                                    SharedString::from(source),
                                    preset.local_crop.as_ref(),
                                ),
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.selection.list[ix].enabled {
                                    this.selection.selected_ix = ix;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        h_flex()
                            .gap(surface::css(6.))
                            .child(
                                Button::new(SharedString::from(format!("import-{id}")))
                                    .label(t("IMPORT"))
                                    .outline()
                                    .disabled(!enabled)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.import_custom(ix, window, cx);
                                    })),
                            )
                            .when(custom && enabled, |row| {
                                row.child(
                                    Button::new(SharedString::from(format!("reset-{id}")))
                                        .label(t("RESET"))
                                        .outline()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if let Some(item) = this.selection.list.get_mut(ix) {
                                                item.custom = false;
                                                item.src = None;
                                                item.size = None;
                                                item.local_crop = None;
                                            }
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .child(
                        surface::SynapseSwitch::new(SharedString::from(format!("enable-{id}")))
                            .label(format!("{} {}", t(self.kind.title()), ix + 1))
                            .checked(enabled)
                            .disabled(last_enabled && enabled)
                            .on_change(cx.listener(move |this, enabled, _, cx| {
                                this.selection.set_enabled(ix, *enabled);
                                cx.notify();
                            })),
                    )
            }))
    }
}

impl PresetEditor {
    fn import_custom(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
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
        let parent = cx.weak_entity();
        cx.spawn_in(window, async move |_, cx| {
            let result = match picker.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next().and_then(|path| {
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
                    allowed.then(|| load_data_url(&path, &extension))
                }),
                _ => None,
            };
            let Some(Ok((source, size))) = result else {
                return;
            };
            _ = parent.update_in(cx, |this, window, cx| {
                if this
                    .selection
                    .list
                    .get(index)
                    .is_some_and(|item| item.enabled)
                {
                    this.open_crop(index, kind, source, size, window, cx);
                }
            });
        })
        .detach();
    }

    fn open_crop(
        &mut self,
        index: usize,
        kind: PresetKind,
        source: String,
        size: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let crop = cx.new(|_| CropDraft {
            source: source.clone().into(),
            index,
            zoom: 1.,
            kind,
        });
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
                                        zoom: crop_for_apply.read(cx).zoom,
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

fn load_data_url(path: &Path, extension: &str) -> Result<(String, u64), std::io::Error> {
    let bytes = std::fs::read(path)?;
    let mime = match extension {
        "gif" => "image/gif",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    };
    Ok((
        format!("data:{mime};base64,{}", base64_encode(&bytes)),
        bytes.len() as u64,
    ))
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

    pub(super) fn render_oled_presets(&self, disabled: bool, cx: &mut Context<Self>) -> AnyElement {
        let selected = self
            .draft
            .pointer("/oled/homeScreenDisplay/selected")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let cards =
            h_flex()
                .gap(surface::css(10.))
                .flex_wrap()
                .when(selected <= 1, |view| {
                    view.children([PresetKind::Animation, PresetKind::Image].into_iter().map(
                        |kind| {
                            let selection = self.preset_selection(kind);
                            let source = selection
                                .list
                                .get(selection.selected_ix)
                                .and_then(|preset| preset.src.clone())
                                .unwrap_or_else(|| kind.asset(selection.selected_ix).to_string());
                            let crop = selection
                                .list
                                .get(selection.selected_ix)
                                .and_then(|preset| preset.local_crop.as_ref());
                            v_flex()
                                .w(surface::css(236.))
                                .gap_1()
                                .when(disabled, |view| view.opacity(0.2))
                                .child(div().text_size(surface::css(14.)).child(t(kind.title())))
                                .child(
                                    h_flex()
                                        .w(surface::css(236.))
                                        .h(surface::css(68.))
                                        .bg(OledColors::screen())
                                        .border_2()
                                        .border_color(OledColors::border())
                                        .items_center()
                                        .justify_center()
                                        .child(cropped_preview(SharedString::from(source), crop)),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "oled-edit-{}",
                                        kind.key()
                                    )))
                                    .label(t("EDIT"))
                                    .outline()
                                    .disabled(disabled || self.is_ble)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_oled_presets(kind, window, cx);
                                    })),
                                )
                        },
                    ))
                });
        v_flex()
            .gap(surface::css(10.))
            .child(cards)
            .child(self.render_home_mode_branch(selected, disabled, cx))
            .into_any_element()
    }

    /// Static counterparts of the source's emote, banner, media, system and
    /// keyboard branches. Their samples are presentation-only; the device
    /// service is not connected, so no telemetry or OLED payload is inferred.
    fn render_home_mode_branch(
        &self,
        selected: u64,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let unavailable = "Preview only; OLED content service is not connected.";
        let black_preview = |label: &str| {
            v_flex()
                .w(surface::css(530.))
                .h(surface::css(120.))
                .bg(OledColors::screen())
                .border_1()
                .border_color(OledColors::border())
                .items_center()
                .justify_center()
                .gap(surface::css(6.))
                .child(
                    div()
                        .text_size(surface::css(15.))
                        .text_color(OledColors::muted())
                        .child(label.to_owned()),
                )
                .child(
                    div()
                        .text_size(surface::css(12.))
                        .text_color(OledColors::muted())
                        .child(unavailable),
                )
        };
        let mut panel = v_flex().gap(surface::css(8.));
        match selected {
            4 => {
                panel = panel
                    .child(div().text_size(surface::css(14.)).child("Emote"))
                    .child(black_preview("Emote preview"))
                    .child(
                        Button::new("oled-emote-edit")
                            .label(t("EDIT"))
                            .outline()
                            .disabled(disabled || self.is_ble)
                            .on_click(cx.listener(|_, _, window, cx| {
                                window.open_dialog(cx, |dialog, _, cx| {
                                    dialog
                                        .title("Emote editor")
                                        .child(surface::note(
                                            "Emote catalog service is unavailable; no emote is selected.",
                                            cx,
                                        ))
                                });
                            })),
                    );
            }
            2 => {
                panel = panel
                    .child(div().text_size(surface::css(14.)).child("Banner"))
                    .child(black_preview("Banner preview"))
                    .child(
                        h_flex()
                            .gap(surface::css(8.))
                            .children(["Top", "Bottom"].into_iter().map(|position| {
                                Button::new(SharedString::from(format!("oled-banner-{position}")))
                                    .label(position)
                                    .outline()
                                    .disabled(disabled || self.is_ble)
                            })),
                    )
                    .child(
                        Button::new("oled-banner-edit")
                            .label(t("EDIT"))
                            .outline()
                            .disabled(disabled || self.is_ble)
                            .on_click(cx.listener(|_, _, window, cx| {
                                window.open_dialog(cx, |dialog, _, cx| {
                                    dialog
                                        .title("Banner editor")
                                        .child(surface::note(
                                            "Banner image and text editing are staged locally; host transfer is unavailable.",
                                            cx,
                                        ))
                                });
                            })),
                    );
            }
            5 => {
                let media = OledMediaDraft::from_value(
                    self.draft.pointer("/oled/media").unwrap_or(&Value::Null),
                );
                panel = panel
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .child(t("OLED_HOME_SCREEN_DISPLAY_TITLE_AUDIO_METER")),
                    )
                    .child(media.preview())
                    .child(
                        Button::new("oled-media-edit")
                            .label(t("EDIT"))
                            .outline()
                            .disabled(disabled || self.is_ble)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_oled_media(window, cx);
                            })),
                    )
                    .child(surface::note(
                        "Media metadata and visualizer values are preview-only until the audio service connects.",
                        cx,
                    ));
            }
            6 => {
                panel = panel
                    .child(div().text_size(surface::css(14.)).child("System information"))
                    .child(black_preview("System information preview"))
                    .child(surface::note(
                        "Battery, temperature and date samples are intentionally hidden; live system information is unavailable.",
                        cx,
                    ));
            }
            3 => {
                panel = panel
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .child("Keyboard information"),
                    )
                    .child(black_preview("Keyboard information"));
            }
            _ => {}
        }
        panel.into_any_element()
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
        let editor = cx.new(|_| PresetEditor { kind, selection });
        let parent = cx.weak_entity();
        let width = surface::css(800.).to_pixels(window.rem_size());
        window.open_dialog(cx, move |dialog, _, _| {
            let editor_for_apply = editor.clone();
            let parent = parent.clone();
            dialog
                .title(t(kind.title()))
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
        imported.local_crop = Some(CropPlacement { zoom: 1.8 });
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

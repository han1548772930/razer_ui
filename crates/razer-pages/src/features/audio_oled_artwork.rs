//! Current 1383 au/Su/Vu: isolated local artwork drafts and source preset lists.
use super::dialog::DialogState;
use super::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use razer_widgets::scroll::SourceScrollable as _;
#[path = "audio_oled_artwork_theme.rs"]
mod artwork_theme;
use artwork_theme::Colors as ArtworkColors;
// Reuse local decoding/cache and Cropper viewMode-0 arithmetic, after checking
// 1383 rh's 232x190, 232x64 fixed crop and minCanvasHeight:64 props separately.
#[path = "audio_oled_artwork_crop.rs"]
mod crop_editor;
#[path = "source_controls/oled_crop.rs"]
mod media_decode;

#[derive(Deserialize)]
struct Artwork {
    id: String,
    name: String,
    asset: String,
}
#[derive(Deserialize)]
struct ArtworkSpec {
    labels: BTreeMap<String, String>,
    animations: Vec<Artwork>,
    images: Vec<Artwork>,
    emotes: Vec<Artwork>,
    default_emote: String,
}
fn artwork_spec() -> &'static ArtworkSpec {
    static SPEC: OnceLock<ArtworkSpec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("audio_oled_artwork_data.json"))
            .expect("validated current 1383 artwork")
    })
}
fn artwork_label(symbol: &str) -> String {
    t(&artwork_spec().labels[symbol])
}
fn mode_key(mode: u32) -> &'static str {
    match mode {
        0 => "animation",
        1 => "image",
        4 => "emote",
        _ => unreachable!("artwork mode"),
    }
}
fn presets(mode: u32) -> &'static [Artwork] {
    match mode {
        0 => &artwork_spec().animations,
        1 => &artwork_spec().images,
        4 => &artwork_spec().emotes,
        _ => &[],
    }
}
fn emote_value(item: &Artwork) -> Value {
    json!({"src":{"id":item.id,"name":item.name,"src":item.asset},"isDataMatch":true})
}

pub(super) fn default_value(mode: u32) -> Value {
    if mode == 4 {
        return emote_value(
            artwork_spec()
                .emotes
                .iter()
                .find(|item| item.id == artwork_spec().default_emote)
                .expect("source default emote"),
        );
    }
    json!({"selectedIdx":0,"list":presets(mode).iter().map(|item| json!({"id":item.id,"src":item.asset,"custom":false,"enabled":true,"size":0,"local_crop":media_decode::CropCanvas::new(232.,64.)})).collect::<Vec<_>>()})
}

pub(super) fn normalize(mode: u32, value: &mut Value) {
    if mode == 4 {
        let matched = value["isDataMatch"] != false;
        if let Some(item) = presets(mode)
            .iter()
            .find(|item| value["src"]["id"] == item.id)
        {
            *value = emote_value(item);
        } else {
            *value = default_value(mode);
        }
        value["isDataMatch"] = json!(matched);
        return;
    }
    let original = value.clone();
    *value = default_value(mode);
    for (index, item) in value["list"]
        .as_array_mut()
        .expect("default list")
        .iter_mut()
        .enumerate()
    {
        if let Some(enabled) = original["list"][index]["enabled"].as_bool() {
            item["enabled"] = json!(enabled);
        }
        let saved = &original["list"][index];
        // Zero preserves the source's absent-size falsiness through merge_known.
        // Only the source reset callback's known preset size is restored.
        if saved["custom"] != true && saved["size"] == if mode == 0 { 60 } else { 1 } {
            item["size"] = saved["size"].clone();
        }
        if saved["custom"] == true
            && saved["src"].as_str().is_some_and(|src| {
                [
                    "data:image/gif;base64,",
                    "data:image/png;base64,",
                    "data:image/jpeg;base64,",
                    "data:image/bmp;base64,",
                ]
                .iter()
                .any(|prefix| src.starts_with(prefix))
            })
        {
            if let Some(canvas) =
                serde_json::from_value::<media_decode::CropCanvas>(saved["local_crop"].clone())
                    .ok()
                    .and_then(media_decode::CropCanvas::normalized)
            {
                item["custom"] = json!(true);
                item["src"] = saved["src"].clone();
                item["local_crop"] = json!(canvas);
            }
        }
    }
    if !value["list"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["enabled"] == true)
    {
        value["list"][0]["enabled"] = json!(true);
    }
    let selected = original["selectedIdx"]
        .as_u64()
        .map(|n| n as usize)
        .filter(|index| {
            value["list"]
                .get(*index)
                .is_some_and(|item| item["enabled"] == true)
        })
        .unwrap_or_else(|| {
            value["list"]
                .as_array()
                .unwrap()
                .iter()
                .position(|item| item["enabled"] == true)
                .unwrap()
        });
    value["selectedIdx"] = json!(selected);
}

pub(super) fn preview(mode: u32, value: &Value) -> AnyElement {
    if mode == 4 && value["isDataMatch"] == false {
        return v_flex()
            .w(surface::css(232.))
            .h(surface::css(64.))
            .items_center()
            .justify_center()
            .bg(ArtworkColors::black())
            .text_size(surface::css(14.))
            .text_center()
            .child(
                img("synapse/audio-oled-artwork-not-found.svg")
                    .w(surface::css(24.))
                    .h(surface::css(20.)),
            )
            .child(artwork_label("avz"))
            .into_any_element();
    }
    if mode != 4 {
        if let Some(item) = value["selectedIdx"]
            .as_u64()
            .and_then(|index| value["list"].get(index as usize))
        {
            return preset_preview(item, false, true);
        }
    }
    let asset = if mode == 4 {
        value["src"]["src"].as_str()
    } else {
        value["selectedIdx"]
            .as_u64()
            .and_then(|n| value["list"][n as usize]["src"].as_str())
    };
    let mut view = div()
        .relative()
        .w(surface::css(232.))
        .h(surface::css(64.))
        .overflow_hidden()
        .bg(if mode == 4 {
            ArtworkColors::emote()
        } else {
            ArtworkColors::black()
        });
    if let Some(asset) = asset {
        view = view.child(
            img(SharedString::from(asset.to_owned()))
                .absolute()
                .left(surface::css(if mode == 4 { -11. } else { 0. }))
                .w(surface::css(232.))
                .h(surface::css(64.))
                .object_fit(ObjectFit::Fill),
        );
    }
    view.into_any_element()
}

fn preset_preview(item: &Value, hovered: bool, enabled: bool) -> AnyElement {
    let zoom = if hovered { 1.1 } else { 1. };
    let canvas = serde_json::from_value::<media_decode::CropCanvas>(item["local_crop"].clone())
        .ok()
        .and_then(media_decode::CropCanvas::normalized)
        .unwrap_or_else(|| media_decode::CropCanvas::new(232., 64.));
    let src = SharedString::from(item["src"].as_str().unwrap_or_default().to_owned());
    let id = SharedString::from(format!(
        "1383-artwork-preview-{}",
        item["id"].as_str().unwrap_or_default()
    ));
    div()
        .relative()
        .w(surface::css(232.))
        .h(surface::css(64.))
        .overflow_hidden()
        .when(enabled, |view| view.bg(ArtworkColors::black()))
        .child(
            img(media_decode::preview_source(id, src))
                .absolute()
                .w(surface::css(canvas.width() * zoom))
                .h(surface::css(canvas.height() * zoom))
                .left(surface::css(canvas.left() * zoom - 232. * (zoom - 1.) / 2.))
                .top(surface::css(
                    (canvas.top() - media_decode::CROP_TOP) * zoom - 64. * (zoom - 1.) / 2.,
                ))
                .object_fit(ObjectFit::Fill)
                .grayscale(true)
                .opacity(if enabled { 1. } else { 0.1 }),
        )
        .into_any_element()
}

pub(super) struct ArtworkEditor {
    owner: WeakEntity<AudioProductWorkspace>,
    mode: u32,
    draft: Value,
    dialog: DialogState,
    search: Entity<InputState>,
    _search_subscription: Subscription,
    crop: Option<crop_editor::CropState>,
    import_generation: u64,
}
impl ArtworkEditor {
    pub(super) fn open(
        owner: WeakEntity<AudioProductWorkspace>,
        mode: u32,
        mut draft: Value,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        normalize(mode, &mut draft);
        cx.new(|cx| {
            let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
            let subscription = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
            Self {
                owner,
                mode,
                draft,
                dialog: DialogState::new(window, cx).max_width(800.),
                search,
                _search_subscription: subscription,
                crop: None,
                import_generation: 0,
            }
        })
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import_generation = self.import_generation.wrapping_add(1);
        self.dialog.close(window, cx);
        cx.notify();
    }
    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.draft.clone();
        let key = mode_key(self.mode);
        let _ = self.owner.update(cx, |owner, cx| {
            owner.draft["oledHome"][key] = value;
            cx.emit(AudioProductChanged);
            cx.notify();
        });
        self.close(window, cx);
    }
    fn toggle(&mut self, index: usize, cx: &mut Context<Self>) {
        let list = self.draft["list"]
            .as_array_mut()
            .expect("normalized preset list");
        let enabled_count = list.iter().filter(|item| item["enabled"] == true).count();
        let enabled = list[index]["enabled"] == true;
        if enabled && enabled_count == 1 {
            return;
        }
        list[index]["enabled"] = json!(!enabled);
        if enabled && self.draft["selectedIdx"] == index {
            let selected = self.draft["list"]
                .as_array()
                .unwrap()
                .iter()
                .position(|item| item["enabled"] == true)
                .unwrap();
            self.draft["selectedIdx"] = json!(selected);
        }
        cx.notify();
    }
    fn reset(&mut self, index: usize, cx: &mut Context<Self>) {
        let item = &presets(self.mode)[index];
        self.draft["list"][index]["src"] = json!(item.asset);
        self.draft["list"][index]["custom"] = json!(false);
        self.draft["list"][index]["local_crop"] = json!(media_decode::CropCanvas::new(232., 64.));
        self.draft["list"][index]["size"] = json!(if self.mode == 0 { 60 } else { 1 });
        cx.notify();
    }
    fn preset_tile(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let item = &presets(self.mode)[index];
        let enabled = self.draft["list"][index]["enabled"] == true;
        let selected = self.draft["selectedIdx"] == index;
        let custom = self.draft["list"][index]["custom"] == true;
        let enabled_count = self.draft["list"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["enabled"] == true)
            .count();
        let hover = window.use_keyed_state(
            (ElementId::from(item.id.clone()), "artwork-hover"),
            cx,
            |_, _| false,
        );
        let hovered = *hover.read(cx);
        let border = if !enabled {
            ArtworkColors::disabled_border()
        } else if selected {
            ArtworkColors::selected()
        } else if hovered {
            ArtworkColors::hover()
        } else {
            ArtworkColors::border()
        };
        // Source content-box 232×64 + border/margin remains 236×68.
        let overlay = v_flex()
            .absolute()
            .inset_0()
            .p(surface::css(2.))
            .justify_between()
            .bg(ArtworkColors::backdrop())
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .id(SharedString::from(format!("{}-switch", item.id)))
                            .m(surface::css(2.))
                            .on_click(|_, _, cx| cx.stop_propagation())
                            .child(
                                surface::SynapseSwitch::new(SharedString::from(format!(
                                    "{}-enabled",
                                    item.id
                                )))
                                .accessibility_label(item.name.clone())
                                .checked(enabled)
                                .disabled(enabled && enabled_count == 1)
                                .on_change(
                                    cx.listener(move |this, _, _, cx| this.toggle(index, cx)),
                                ),
                            ),
                    )
                    .when(enabled, |view| {
                        let button = artwork_icon("replace", &item.id).on_click(cx.listener(
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.import_custom(index, window, cx);
                            },
                        ));
                        view.child(super::tooltip::artwork(
                            format!("1383-artwork-replace-tip-{}", item.id),
                            artwork_label("djP"),
                            button,
                            window,
                            cx,
                        ))
                    }),
            )
            .when(custom && enabled, |view| {
                let button =
                    artwork_icon("reset", &item.id).on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.reset(index, cx);
                    }));
                view.child(super::tooltip::artwork(
                    format!("1383-artwork-reset-tip-{}", item.id),
                    artwork_label("VLI"),
                    button,
                    window,
                    cx,
                ))
            });
        let content = div()
            .relative()
            .w(surface::css(232.))
            .h(surface::css(64.))
            .when(enabled, |view| view.bg(ArtworkColors::black()))
            .child(preset_preview(&self.draft["list"][index], hovered, enabled))
            .when(hovered, |view| view.child(overlay));
        let tile = BaseButton::new(SharedString::from(format!("1383-artwork-{}", item.id)))
            .accessibility_label(item.name.clone())
            .p_0()
            .relative()
            .w(surface::css(236.))
            .h(surface::css(68.))
            .on_hover(window.listener_for(&hover, |state, value, _, cx| {
                *state = *value;
                cx.notify();
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                if enabled {
                    this.draft["selectedIdx"] = json!(index);
                    cx.notify();
                }
            }))
            .child(
                div()
                    .absolute()
                    .inset(surface::css(if selected || hovered { 0. } else { 1. }))
                    .when(!enabled && hovered, |view| {
                        view.w(surface::css(234.)).h(surface::css(66.))
                    })
                    .border_color(border)
                    .map(|view| {
                        if enabled && (selected || hovered) {
                            view.border_2()
                        } else {
                            view.border_1()
                        }
                    })
                    .overflow_hidden()
                    .child(content),
            );
        tile.into_any_element()
    }
    fn emote_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let filter = |text: &str| {
            text.chars()
                .filter(|ch| !matches!(ch, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'))
                .collect::<String>()
                .to_lowercase()
        };
        let query = filter(self.search.read(cx).value().as_ref());
        let mut tiles = div()
            .id("1383-emote-list")
            .flex_1()
            .min_h_0()
            .grid()
            .grid_cols(5)
            .gap_y(surface::css(15.));
        for item in &artwork_spec().emotes {
            if !filter(&item.name).contains(&query) {
                continue;
            }
            let selected = self.draft["src"]["id"] == item.id && self.draft["isDataMatch"] == true;
            let hover = window.use_keyed_state(
                (ElementId::from(item.id.clone()), "emote-hover"),
                cx,
                |_, _| false,
            );
            let hovered = *hover.read(cx);
            let id = item.id.clone();
            tiles = tiles.child(
                BaseButton::new(SharedString::from(format!("1383-emote-{}", item.id)))
                    .p_0()
                    .w(surface::css(132.))
                    .h(surface::css(65.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(surface::css(5.))
                    .accessibility_label(item.name.clone())
                    .on_hover(window.listener_for(&hover, |state, value, _, cx| {
                        *state = *value;
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(item) = artwork_spec().emotes.iter().find(|item| item.id == id)
                        {
                            this.draft = emote_value(item);
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .w(surface::css(132.))
                            .h(surface::css(46.))
                            .flex_shrink_0()
                            .map(|view| {
                                if selected {
                                    view.border_2()
                                } else {
                                    view.border_1().p(surface::css(1.))
                                }
                            })
                            .bg(ArtworkColors::emote())
                            .border_color(if selected {
                                ArtworkColors::selected()
                            } else if hovered {
                                ArtworkColors::hover()
                            } else {
                                ArtworkColors::emote_border()
                            })
                            .child(
                                img(SharedString::from(item.asset.clone()))
                                    .w(surface::css(116.))
                                    .h(surface::css(42.))
                                    .object_fit(ObjectFit::Contain),
                            ),
                    )
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .font_weight(FontWeight::NORMAL)
                            .text_center()
                            .child(item.name.clone()),
                    ),
            );
        }
        v_flex()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .w(surface::css(290.))
                    .p(surface::css(3.))
                    .mb(surface::css(20.))
                    .border_1()
                    .border_color(ArtworkColors::emote_border())
                    .bg(ArtworkColors::search())
                    .child(
                        Input::new(&self.search)
                            .appearance(false)
                            .h(surface::css(20.))
                            .flex_1()
                            .min_w_0()
                            .p_0()
                            .w_full()
                            .border_0()
                            .bg(ArtworkColors::search())
                            .text_color(ArtworkColors::white()),
                    )
                    .child(img("synapse/audio-oled-artwork-search.svg").size(surface::css(20.))),
            )
            .child(tiles.scrollable_y())
            .into_any_element()
    }
}
fn artwork_icon(kind: &'static str, id: &str) -> BaseButton {
    BaseButton::new(SharedString::from(format!("1383-artwork-{kind}-{id}")))
        .accessibility_label(artwork_label(if kind == "reset" { "VLI" } else { "djP" }))
        .p_0()
        // The tooltip's source target wrapper owns the 2px outer margin.
        .m_0()
        .w(surface::css(28.))
        .h(surface::css(27.))
        .rounded(surface::css(5.))
        .border_1()
        .border_color(ArtworkColors::white())
        .bg(ArtworkColors::black())
        .text_color(ArtworkColors::text())
        .hover(|view| {
            view.border_color(ArtworkColors::selected())
                .text_color(ArtworkColors::selected())
        })
        .child(
            gpui_kit::component::Icon::default()
                .path(SharedString::from(format!(
                    "synapse/audio-oled-artwork-{kind}.svg"
                )))
                .size(surface::css(20.)),
        )
}
impl Render for ArtworkEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if self.mode == 4 {
            self.emote_body(window, cx)
        } else {
            let mut grid = div()
                .grid()
                .grid_cols(3)
                .gap(surface::css(20.))
                .mt(surface::css(30.));
            for index in 0..presets(self.mode).len() {
                grid = grid.child(self.preset_tile(index, window, cx));
            }
            let mut body = v_flex()
                .id("1383-artwork-presets")
                .flex_1()
                .min_h_0()
                .scrollable_y()
                .child(artwork_label(if self.mode == 0 { "PYB" } else { "Qg_" }))
                .child(grid);
            if self.mode == 0 {
                let frames = self.draft["list"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|item| item["size"].as_u64())
                    .sum::<u64>();
                if frames > 0 {
                    let seconds = (frames as f64 * 1.4).ceil() as u64;
                    let minutes = (seconds as f64 / 60.).round() as u64;
                    let text = artwork_label(if minutes > 0 { "pc9" } else { "Qj6" }).replace(
                        "{{time}}",
                        &if minutes > 0 { minutes } else { seconds }.to_string(),
                    );
                    body = body.child(
                        h_flex()
                            .items_center()
                            .justify_center()
                            .gap(surface::css(5.))
                            .child(
                                img("synapse/audio-oled-artwork-info.svg").size(surface::css(24.)),
                            )
                            .child(text),
                    );
                }
            }
            body.into_any_element()
        };
        let body = v_flex()
            .flex_1()
            .min_h_0()
            .pt(surface::css(20.))
            .px(surface::css(25.))
            .pb(surface::css(97.))
            .child(body)
            .into_any_element();
        if self.crop.is_some() {
            let body = self.render_crop(window, cx);
            let footer = dialog::footer(
                dialog::action("1383-artwork-crop-cancel", artwork_label("bOp"), false)
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_crop(cx)))
                    .into_any_element(),
                // Current oh checks applyBtnName === "Crop", but rh passes
                // "CROPPER_CROP_BTN"; the actual rendered label remains APPLY.
                dialog::action("1383-artwork-crop-apply", artwork_label("pJk"), true)
                    .on_click(cx.listener(|this, _, _, cx| this.apply_crop(cx)))
                    .into_any_element(),
            );
            return self.dialog.render(
                "1383-artwork",
                artwork_label(if self.mode == 0 { "scg" } else { "jJm" }),
                body,
                footer,
                window,
                cx,
                Self::close,
            );
        }
        let footer = dialog::footer(
            dialog::action("1383-artwork-cancel", artwork_label("bOp"), false)
                .on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))
                .into_any_element(),
            dialog::action("1383-artwork-apply", artwork_label("pJk"), true)
                .disabled(self.mode == 4 && self.draft["isDataMatch"] != true)
                .when(
                    self.mode == 4 && self.draft["isDataMatch"] != true,
                    |button| button.opacity(0.3),
                )
                .on_click(cx.listener(|this, _, window, cx| this.apply(window, cx)))
                .into_any_element(),
        );
        self.dialog.render(
            "1383-artwork",
            artwork_label(match self.mode {
                0 => "scg",
                1 => "jJm",
                _ => "Qt7",
            }),
            body,
            footer,
            window,
            cx,
            Self::close,
        )
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
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

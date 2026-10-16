//! 1383 `_v / gv / hv / uv / xv / vv`; source data are local preview fixtures.
//! No service telemetry or SET_OLED_DISPLAY_SYSTEM_INFO acknowledgement is forged.
use super::dialog::{self, DialogState};
use super::*;
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::base::{Button as BaseButton, ElementExt as _, PopoverState};
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::source_tooltip::SourceTooltip;
use std::{cell::Cell, rc::Rc, time::Duration};
#[path = "audio_oled_system_menu.rs"]
mod menu;
#[path = "audio_oled_system_theme.rs"]
mod theme;
use theme::SystemColors as C;

#[derive(Deserialize)]
struct SystemSpec {
    #[serde(rename = "default")]
    defaults: Value,
    tags: Vec<Value>,
    slide_titles: Vec<Value>,
    intervals: Vec<u64>,
    date_formats: Vec<Value>,
    labels: BTreeMap<String, String>,
    dots: Vec<String>,
    batteries: Vec<String>,
    icons: BTreeMap<String, String>,
}
fn spec() -> &'static SystemSpec {
    static SPEC: OnceLock<SystemSpec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("audio_oled_system_data.json"))
            .expect("validated current 1383 System Info data")
    })
}
fn text(symbol: &str) -> String {
    t(&spec().labels[symbol])
}
fn tag(id: &str) -> Option<&'static Value> {
    spec().tags.iter().find(|t| t["id"] == id)
}
fn tag_name(value: &Value) -> String {
    t(value["label"].as_str().expect("source tag label"))
}
fn canonical_tag(id: &str) -> Option<Value> {
    let mut value = tag(id)?.clone();
    value.as_object_mut()?.remove("previewAsset");
    Some(value)
}
pub(super) fn default_value() -> Value {
    spec().defaults.clone()
}
pub(super) fn normalize(value: &mut Value) {
    let old = value.clone();
    *value = default_value();
    for (field, allowed) in [
        ("timeBetweenSlides", spec().intervals.as_slice()),
        ("selectedSlideIdx", &[0, 1, 2]),
        ("timeFormat", &[0, 1]),
        ("dateFormat", &[0, 1, 2]),
    ] {
        if let Some(v) = old[field].as_u64().filter(|v| allowed.contains(v)) {
            value[field] = json!(v);
        }
    }
    if let Some(v) = old["temperatureUnit"]
        .as_str()
        .filter(|v| matches!(*v, "celsius" | "fahrenheit"))
    {
        value["temperatureUnit"] = json!(v);
    }
    if let Some(slides) = old["slides"].as_array().filter(|slides| slides.len() == 3) {
        for (index, slide) in slides.iter().enumerate() {
            for side in ["left", "right"] {
                value["slides"][index][side] = slide[side]["id"]
                    .as_str()
                    .and_then(canonical_tag)
                    .unwrap_or(Value::Null);
            }
        }
    }
}
/// Home display filters empty slides before its source interval advances.
pub(super) fn preview(value: &Value, elapsed: f32, oled_language: u64) -> AnyElement {
    let slides = value["slides"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter(|s| !s["left"].is_null() || !s["right"].is_null())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let interval = value["timeBetweenSlides"]
        .as_u64()
        .filter(|v| spec().intervals.contains(v))
        .unwrap_or(3);
    let selected = if slides.is_empty() {
        None
    } else {
        Some(slides[(elapsed.max(0.) / interval as f32) as usize % slides.len()])
    };
    preview_slide(selected, oled_language)
}
fn preview_slide(slide: Option<&Value>, oled_language: u64) -> AnyElement {
    h_flex()
        .w(surface::css(232.))
        .h(surface::css(64.))
        .justify_around()
        .text_color(C::white())
        .text_size(surface::css(20.))
        .children(["left", "right"].into_iter().filter_map(|side| {
            let selected = slide?.get(side)?;
            let id = selected["id"].as_str()?;
            let source_tag = tag(id)?;
            let info = &spec().defaults["info"][id];
            let label = if oled_language == 1 {
                razer_i18n::t_locale(source_tag["deviceLabel"].as_str().unwrap_or(""), "zh-CN")
            } else {
                info["label"].as_str().unwrap_or("").to_owned()
            };
            let number = info["value"].as_u64();
            let value = number.map_or_else(
                || info["value"].as_str().unwrap_or("").to_owned(),
                |value| format!("{value}%"),
            );
            let battery = number.map(|v| {
                if v > 75 {
                    4
                } else if v > 50 {
                    3
                } else if v > 25 {
                    2
                } else if v > 5 {
                    1
                } else {
                    0
                }
            });
            Some(
                v_flex()
                    .w(relative(0.5))
                    .text_center()
                    .items_center()
                    .child(
                        h_flex()
                            .justify_center()
                            .when_some(battery, |row, index| {
                                row.child(
                                    img(SharedString::from(spec().batteries[index].clone()))
                                        .size(surface::css(20.)),
                                )
                            })
                            .child(value),
                    )
                    .when(!info["label"].as_str().unwrap_or("").is_empty(), |cell| {
                        cell.child(
                            div()
                                .text_size(surface::css(14.))
                                .child(label.to_uppercase()),
                        )
                    }),
            )
        }))
        .into_any_element()
}

#[derive(Clone)]
struct TagDrag {
    id: String,
}
struct DragPreview {
    asset: String,
    width: f32,
    height: f32,
}
impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        img(SharedString::from(self.asset.clone()))
            .w(surface::css(self.width))
            .h(surface::css(self.height))
    }
}
#[derive(Deserialize)]
struct SystemAssetSizes {
    sizes: BTreeMap<String, [f32; 2]>,
}
fn preview_size(asset: &str) -> [f32; 2] {
    static SIZES: OnceLock<SystemAssetSizes> = OnceLock::new();
    SIZES
        .get_or_init(|| {
            serde_json::from_str(include_str!("audio_oled_system_asset_sizes.json"))
                .expect("source drag-preview sizes")
        })
        .sizes[asset]
}
pub(super) struct SystemEditor {
    owner: WeakEntity<AudioProductWorkspace>,
    draft: Value,
    dialog: DialogState,
    menu: Entity<PopoverState>,
    date: Entity<SelectState<Vec<Choice>>>,
    _subscription: Subscription,
    open_menu: Option<(usize, &'static str)>,
    dragging: Option<String>,
    oled_language: u64,
}
impl SystemEditor {
    pub(super) fn open(
        owner: WeakEntity<AudioProductWorkspace>,
        mut draft: Value,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        normalize(&mut draft);
        let oled_language = owner
            .upgrade()
            .map(|owner| {
                owner.read(cx).draft["device"]["oledLanguage"]
                    .as_u64()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let date: Entity<SelectState<Vec<Choice>>> = cx.new(|cx| {
            SelectState::new(
                spec()
                    .date_formats
                    .iter()
                    .map(|d| Choice::new(d["id"].to_string(), d["name"].as_str().unwrap_or("")))
                    .collect::<Vec<Choice>>(),
                None,
                window,
                cx,
            )
        });
        date.update(cx, |date, cx| {
            date.set_selected_value(&draft["dateFormat"].to_string(), window, cx)
        });
        let menu = cx.new(|cx| PopoverState::new(false, cx));
        cx.new(|cx| {
            let subscription =
                cx.subscribe_in(&date, window, |this: &mut Self, _, event, _, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event {
                        if let Ok(value) = value.parse::<u64>() {
                            if value < 3 {
                                this.draft["dateFormat"] = json!(value);
                                cx.notify();
                            }
                        }
                    }
                });
            Self {
                owner,
                draft,
                dialog: DialogState::new(window, cx),
                menu,
                date,
                _subscription: subscription,
                open_menu: None,
                dragging: None,
                oled_language,
            }
        })
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_menu = None;
        self.menu
            .update(cx, |menu, cx| menu.sync_open(false, window, cx));
        self.dialog.close(window, cx);
        cx.notify();
    }
    fn select_slide(&mut self, index: usize, cx: &mut Context<Self>) {
        self.draft["selectedSlideIdx"] = json!(index);
        cx.notify();
    }
    fn reset(&mut self, cx: &mut Context<Self>) {
        // _v Reset intentionally leaves the date and time format states intact.
        for field in [
            "slides",
            "selectedSlideIdx",
            "timeBetweenSlides",
            "temperatureUnit",
        ] {
            self.draft[field] = spec().defaults[field].clone();
        }
        self.open_menu = None;
        cx.notify();
    }
    fn set_slot(
        &mut self,
        index: usize,
        side: &'static str,
        id: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        self.draft["slides"][index][side] = id.and_then(canonical_tag).unwrap_or(Value::Null);
        self.open_menu = None;
        self.dragging = None;
        cx.notify();
    }
    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.empty() {
            return;
        }
        let draft = self.draft.clone();
        let _ = self.owner.update(cx, |owner, cx| {
            owner.draft["oledHome"]["system"] = draft;
            cx.emit(AudioProductChanged);
            cx.notify();
        });
        self.close(window, cx);
    }
    fn empty(&self) -> bool {
        self.draft["slides"].as_array().is_none_or(|slides| {
            slides
                .iter()
                .all(|slide| slide["left"].is_null() && slide["right"].is_null())
        })
    }
    fn palette_tag(
        &self,
        tag: &'static Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = tag["id"].as_str().expect("source tag id");
        let hovered = window.use_keyed_state(
            (ElementId::from(format!("1383-system-tag-{id}")), "hover"),
            cx,
            |_, _| false,
        );
        let is_dragging = self.dragging.as_deref() == Some(id) && cx.has_active_drag();
        let tip = local_tip(
            format!("1383-system-tag-tip-{id}"),
            *hovered.read(cx) && !is_dragging,
            29.,
            text("eWG"),
            window,
            cx,
        );
        let owner = cx.entity().downgrade();
        let payload = TagDrag { id: id.to_owned() };
        let asset = tag["previewAsset"]
            .as_str()
            .expect("source preview asset")
            .to_owned();
        div()
            .id(SharedString::from(format!("1383-system-tag-{id}")))
            .relative()
            .bg(C::tag())
            .rounded(surface::css(50.))
            .text_size(surface::css(11.))
            .mr(surface::css(10.))
            .py(surface::css(5.))
            .px(surface::css(6.))
            .cursor(CursorStyle::OpenHand)
            .when(is_dragging, |el| el.opacity(0.4))
            .on_hover(window.listener_for(&hovered, |state, hovered, _, cx| {
                *state = *hovered;
                cx.notify();
            }))
            .child(tag_name(tag).to_uppercase())
            .child(tip)
            .on_drag(payload, move |drag, _, _, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.dragging = Some(drag.id.clone());
                    this.open_menu = None;
                    cx.notify();
                });
                let [width, height] = preview_size(&asset);
                cx.new(|_| DragPreview {
                    asset: asset.clone(),
                    width,
                    height,
                })
            })
            .into_any_element()
    }
    fn slot(
        &self,
        index: usize,
        side: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = &self.draft["slides"][index][side];
        let selected = current["id"].as_str().and_then(tag);
        let id = SharedString::from(format!("1383-system-slot-{index}-{side}"));
        let hover =
            window.use_keyed_state((ElementId::from(id.clone()), "hover"), cx, |_, _| false);
        let hovered = *hover.read(cx);
        let mut slot = div()
            .id(id.clone())
            .relative()
            .w(surface::css(128.))
            .h(surface::css(38.))
            .flex()
            .items_center()
            .justify_center()
            .when(side == "left", |slot| {
                slot.border_r_1().border_dashed().border_color(C::border())
            })
            .on_hover(window.listener_for(&hover, |state, hovered, _, cx| {
                *state = *hovered;
                cx.notify();
            }));
        if let Some(selected) = selected {
            slot = slot.child(
                div()
                    .bg(C::tag())
                    .border_1()
                    .border_color(C::border())
                    .rounded(surface::css(50.))
                    .px(surface::css(6.))
                    .py(surface::css(5.))
                    .text_size(surface::css(11.))
                    .text_color(C::selected())
                    .when(hovered, |el| el.opacity(0.3))
                    .child(tag_name(selected).to_uppercase()),
            );
            if hovered {
                let delete_hover = window.use_keyed_state(
                    (ElementId::from(id.clone()), "delete-hover"),
                    cx,
                    |_, _| false,
                );
                let show = *delete_hover.read(cx);
                slot = slot.child(
                    BaseButton::new((ElementId::from(id.clone()), "delete"))
                        .accessibility_label(text("SJi"))
                        .absolute()
                        .size(surface::css(24.))
                        .p_0()
                        .items_start()
                        .justify_start()
                        .bg(C::surface())
                        .on_hover(window.listener_for(&delete_hover, |state, hovered, _, cx| {
                            *state = *hovered;
                            cx.notify();
                        }))
                        .child(
                            svg()
                                .path(SharedString::from(spec().icons["delete"].clone()))
                                .size(surface::css(20.))
                                .text_color(C::delete()),
                        )
                        .when(show, |el| {
                            el.child(razer_widgets::attribute_tip::attribute_tip(
                                (ElementId::from(id.clone()), "delete-tip"),
                                text("SJi"),
                            ))
                        })
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.set_slot(index, side, None, cx)),
                        ),
                );
            }
        } else {
            let menu_open = self.open_menu == Some((index, side));
            let dragging = self.dragging.is_some() && cx.has_active_drag();
            let trigger = BaseButton::new((ElementId::from(id.clone()), "trigger"))
                .accessibility_label(text("EyM"))
                .size_full()
                .p_0()
                .child(
                    svg()
                        .path(SharedString::from(spec().icons["add"].clone()))
                        .size(surface::css(20.))
                        .text_color(if menu_open { C::selected() } else { C::text() }),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_menu = Some((index, side));
                    cx.notify();
                }));
            slot = slot
                .child(
                    div()
                        .id((ElementId::from(id.clone()), "drop"))
                        .size_full()
                        .when(dragging, |el| el.bg(C::selected()).opacity(0.5))
                        .drag_over::<TagDrag>(|s, _, _, _| s.bg(C::selected()).opacity(1.))
                        .on_drop(cx.listener(move |this, drag: &TagDrag, _, cx| {
                            this.set_slot(index, side, Some(&drag.id), cx)
                        }))
                        .child(trigger),
                )
                .child(local_tip(
                    (ElementId::from(id), "add-tip"),
                    hovered,
                    40.,
                    text("EyM"),
                    window,
                    cx,
                ));
        }
        slot.into_any_element()
    }
    fn slide(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.draft["selectedSlideIdx"] == index;
        let mask = Rc::new(Cell::new(window.content_mask()));
        v_flex()
            .relative()
            .w(surface::css(255.))
            .h(surface::css(63.))
            .border_1()
            .border_dashed()
            .border_color(if selected { C::selected() } else { C::border() })
            .on_prepaint({
                let mask = mask.clone();
                move |_, window, _| mask.set(window.content_mask())
            })
            .child(
                BaseButton::new(SharedString::from(format!("1383-system-slide-{index}")))
                    .accessibility_label(t(spec().slide_titles[index]["title"].as_str().unwrap()))
                    .w_full()
                    .h(surface::css(24.))
                    .border_b_1()
                    .border_dashed()
                    .border_color(C::border())
                    .bg(C::tag())
                    .text_size(surface::css(12.))
                    .line_height(surface::css(24.))
                    .text_color(C::text())
                    .hover(|s| s.text_color(C::selected()))
                    .child(t(spec().slide_titles[index]["title"].as_str().unwrap()).to_uppercase())
                    .on_click(cx.listener(move |this, _, _, cx| this.select_slide(index, cx))),
            )
            .child(
                h_flex()
                    .h(surface::css(38.))
                    .child(self.slot(index, "left", window, cx))
                    .child(self.slot(index, "right", window, cx)),
            )
            .when_some(
                self.open_menu
                    .filter(|(slide, _)| *slide == index)
                    .map(|(_, side)| side),
                |slide, side| {
                    // hv is a direct child of gv, not a popup below the slot.
                    // Its absolute layout participates in the scroll owner's
                    // overflow; only z-index paint ordering is deferred.
                    slide.child(
                        deferred(menu::ClippedMenu::new(
                            self.menu_options(index, side, window, cx),
                            mask,
                        ))
                        .with_priority(100),
                    )
                },
            )
            .into_any_element()
    }
    fn menu_options(
        &self,
        index: usize,
        side: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let focus = self.menu.read(cx).focus_handle(cx);
        let menu_state = self.menu.clone();
        v_flex()
            .id(SharedString::from(format!(
                "1383-system-menu-{index}-{side}"
            )))
            .role(Role::Menu)
            .occlude()
            .tab_group()
            .track_focus(&focus)
            .key_context("Popover")
            .on_action(window.listener_for(&self.menu, PopoverState::on_action_cancel))
            .on_mouse_down_out(move |_, window, cx| {
                menu_state.update(cx, |state, cx| state.dismiss(window, cx));
            })
            .absolute()
            .top(surface::css(61.))
            .when(side == "left", |menu| menu.left_0())
            .when(side == "right", |menu| menu.right_0())
            .w(surface::css(if side == "left" { 126. } else { 129. }))
            .bg(C::black())
            .border_1()
            .border_color(C::border())
            .children(spec().tags.iter().map(|tag| {
                let tag_id = tag["id"].as_str().expect("source tag id");
                BaseButton::new(SharedString::from(format!(
                    "1383-system-option-{index}-{side}-{tag_id}"
                )))
                .role(Role::MenuItem)
                .accessibility_label(tag_name(tag))
                .w_full()
                .justify_start()
                .px(surface::css(6.))
                .py(surface::css(5.))
                .text_size(surface::css(14.))
                .text_color(C::text())
                .hover(|s| s.bg(C::option_hover()))
                .child(capitalize(&tag_name(tag)))
                .on_click(
                    cx.listener(move |this, _, _, cx| this.set_slot(index, side, Some(tag_id), cx)),
                )
            }))
            .into_any_element()
    }
    fn config_title(&self, id: &'static str, label: &str, tip: Option<&str>) -> AnyElement {
        h_flex()
            .gap(surface::css(10.))
            .child(text(label).to_uppercase())
            .when_some(tip, |row, tip| row.child(system_help(id, text(tip))))
            .into_any_element()
    }
    fn pills(
        &self,
        id: &'static str,
        field: &'static str,
        options: [(Value, &'static str); 2],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        h_flex()
            .w(surface::css(111.))
            .h(surface::css(32.))
            .p(surface::css(5.))
            .justify_between()
            .bg(C::panel())
            .border_1()
            .border_color(C::border())
            .rounded(surface::css(36.))
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (value, label))| {
                        let selected = self.draft[field] == value;
                        BaseButton::new((id, index))
                            .accessibility_label(label)
                            .w(surface::css(48.))
                            .h(surface::css(22.))
                            .rounded(surface::css(26.))
                            .bg(if selected { C::selected() } else { C::panel() })
                            .text_color(if selected { C::black() } else { C::white() })
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.draft[field] = value.clone();
                                cx.notify();
                            }))
                    }),
            )
            .into_any_element()
    }
}
impl Render for SystemEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        self.menu.update(cx, |menu, cx| {
            menu.set_on_open_change(Some(Rc::new(move |open, _, cx| {
                if !open {
                    let _ = owner.update(cx, |this, cx| {
                        this.open_menu = None;
                        cx.notify();
                    });
                }
            })));
            menu.sync_open(self.open_menu.is_some(), window, cx);
        });
        if !self.dialog.is_open() {
            return div().into_any_element();
        }
        if !cx.has_active_drag() {
            self.dragging = None;
        }
        let selected = self.draft["selectedSlideIdx"].as_u64().unwrap_or(0).min(2) as usize;
        let preview = v_flex()
            .col_start(2)
            .w(surface::css(236.))
            .child(
                div()
                    .h(surface::css(19.))
                    .p(surface::css(2.))
                    .child(text("$yX").to_uppercase()),
            )
            .child(
                v_flex()
                    .w(surface::css(234.))
                    .h(surface::css(66.))
                    .m(surface::css(1.))
                    .border_1()
                    .border_color(C::border())
                    .bg(C::black())
                    .justify_center()
                    .child(preview_slide(
                        self.draft["slides"].get(selected),
                        self.oled_language,
                    )),
            );
        let preview_block = div()
            .grid()
            .grid_cols(3)
            .gap_x(surface::css(20.))
            .items_end()
            .child(preview)
            .child(
                div().child(
                    BaseButton::new("1383-system-reset")
                        .accessibility_label(text("VLI"))
                        .underline()
                        .text_color(C::text())
                        .hover(|s| s.text_color(C::selected()))
                        .child(text("VLI"))
                        .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                ),
            )
            .child(
                h_flex()
                    .col_start(2)
                    .justify_center()
                    .gap(surface::css(10.))
                    .mt(surface::css(5.))
                    .children((0..3).map(|index| {
                        BaseButton::new(("1383-system-preview-dot", index))
                            .accessibility_label(t(spec().slide_titles[index]["title"]
                                .as_str()
                                .unwrap()))
                            .size(surface::css(6.))
                            .p_0()
                            .child(
                                img(SharedString::from(
                                    spec().dots[if selected == index { 0 } else { 1 }].clone(),
                                ))
                                .size_full(),
                            )
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.select_slide(index, cx)),
                            )
                    })),
            );
        let tags = h_flex().relative().children(
            spec()
                .tags
                .iter()
                .map(|tag| self.palette_tag(tag, window, cx)),
        );
        let slides = h_flex()
            .justify_between()
            .mt(surface::css(12.))
            .children((0..3).map(|index| self.slide(index, window, cx)));
        let interval = self.draft["timeBetweenSlides"].as_u64().unwrap_or(3);
        let interval_buttons = h_flex().children(spec().intervals.iter().map(|&seconds| {
            BaseButton::new(SharedString::from(format!(
                "1383-system-interval-{seconds}"
            )))
            .accessibility_label(seconds.to_string())
            .w(surface::css(48.))
            .px(surface::css(16.))
            .py(surface::css(7.))
            .mr(surface::css(10.))
            .rounded(surface::css(3.))
            .bg(C::panel())
            .border_1()
            .border_color(if interval == seconds {
                C::selected()
            } else {
                C::border()
            })
            .child(seconds.to_string())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.draft["timeBetweenSlides"] = json!(seconds);
                cx.notify();
            }))
        }));
        let configs = h_flex()
            .items_start()
            .gap(surface::css(40.))
            .child(
                v_flex()
                    .child(self.config_title("1383-system-interval-help", "RtR", Some("OLp")))
                    .child(line_break())
                    .child(interval_buttons),
            )
            .child(
                v_flex()
                    .child(self.config_title("1383-system-temperature-help", "tI_", Some("qu5")))
                    .child(line_break())
                    .child(self.pills(
                        "1383-system-temperature",
                        "temperatureUnit",
                        [(json!("fahrenheit"), "°F"), (json!("celsius"), "°C")],
                        cx,
                    )),
            )
            .child(
                v_flex()
                    .child(self.config_title("1383-system-date", "hIg", None))
                    .child(line_break())
                    .child(surface::select(&self.date).w(surface::css(110.))),
            )
            .child(
                v_flex()
                    .child(self.config_title("1383-system-time", "ylX", None))
                    .child(line_break())
                    .child(self.pills(
                        "1383-system-time",
                        "timeFormat",
                        [(json!(0), "12H"), (json!(1), "24H")],
                        cx,
                    )),
            );
        let body = v_flex()
            .id("1383-system-body")
            .min_h_0()
            .flex_1()
            .text_size(surface::css(14.))
            .text_color(C::text())
            .pt(surface::css(20.))
            .px(surface::css(25.))
            .pb(surface::css(97.))
            .scrollable_y()
            .child(preview_block)
            .child(line_break())
            .child(line_break())
            .child(self.config_title("1383-system-tags-help", "OcK", Some("btK")))
            .child(line_break())
            .child(tags)
            .child(slides)
            .child(line_break())
            .child(line_break())
            .child(configs)
            .child(line_break())
            .into_any_element();
        let footer = dialog::footer(
            dialog::action("1383-system-cancel", text("bOp"), false)
                .on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))
                .into_any_element(),
            dialog::action("1383-system-apply", text("pJk"), true)
                .disabled(self.empty())
                .styles(|s| s.disabled(|s| s.opacity(0.3)))
                .on_click(cx.listener(|this, _, window, cx| this.apply(window, cx)))
                .into_any_element(),
        );
        self.dialog.render(
            "1383-system-dialog",
            text("G9b"),
            body,
            footer,
            window,
            cx,
            Self::close,
        )
    }
}
fn line_break() -> Div {
    div().h(surface::css(17.)).flex_shrink_0()
}
fn capitalize(text: &str) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}
fn local_tip(
    id: impl Into<ElementId>,
    hovered: bool,
    top: f32,
    text: String,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id = id.into();
    let opacity = motion::transition(
        (id.clone(), "opacity"),
        if hovered { 1_f32 } else { 0. },
        Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
        window,
        cx,
    );
    div()
        .id(id)
        .absolute()
        .left_0()
        .top(surface::css(top))
        .w(surface::css(300.))
        .px(surface::css(10.))
        .py(surface::css(8.))
        .bg(C::panel())
        .border_1()
        .border_color(C::border())
        .text_color(C::text())
        .text_size(surface::css(14.))
        .whitespace_normal()
        .opacity(opacity)
        .when(!hovered, |el| el.invisible())
        .child(text)
        .into_any_element()
}
fn system_help(id: &'static str, text: String) -> AnyElement {
    SourceTooltip::new(id, text.clone(), 210.)
        .hovered_priority(9999)
        .trigger(move |hovered, window, cx| {
            let color = motion::transition(
                (id, "help-color"),
                super::theme::CssColor(if hovered { C::help_hover() } else { C::help() }.into()),
                Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
                window,
                cx,
            );
            BaseButton::new(id)
                .accessibility_label(text)
                .size(surface::css(14.))
                .p_0()
                .rounded_full()
                .bg(color.0)
                .child(img(SharedString::from(spec().icons["help"].clone())).size_full())
                .into_any_element()
        })
        .into_any_element()
}

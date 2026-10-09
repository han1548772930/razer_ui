//! Current 9388 / 19388 Gamer Room marketing, grouped devices and tutorial.
//! Device observations enter only through explicit source subDevices payloads.
use crate::iot_popup;
use gpui_kit::base::{
    Button as BaseButton,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_widgets::surface;
use razer_widgets::theme::MainPageColors;
use razer_widgets::tutorial::TutorialIndicator;
use razer_widgets::tutorial::tutorial_button;
use std::{cell::Cell, rc::Rc, time::Duration};

#[path = "gamer_room_devices.rs"]
mod devices;
#[path = "gamer_room_hotspot.rs"]
mod hotspot;
use hotspot::HotspotPulse;
#[path = "gamer_room_presentation.rs"]
mod presentation;
use presentation::{CollapseArrow, GroupContent, GroupHelp, SourceLayer};

#[cfg(test)]
#[path = "gamer_room_tutorial_tests.rs"]
mod tutorial_tests;

struct Product {
    name: &'static str,
    description: &'static str,
    image: &'static str,
    url: &'static str,
}
const PRODUCTS: &[Product] = &[
    Product {
        name: "AETHER_LIGHT_BULBS",
        description: "AETHER_LIGHT_BULBS_DES",
        image: "synapse/gr-bulb.png",
        url: "https://www.razer.com/gamer-room-lights/razer-aether-light-bulb",
    },
    Product {
        name: "AETHER_LIGHT_STRIP",
        description: "AETHER_LIGHT_STRIP_DES",
        image: "synapse/gr-strip.png",
        url: "https://www.razer.com/gamer-room-lights/razer-aether-light-strip",
    },
    Product {
        name: "AETHER_LAMP_PRO",
        description: "AETHER_LAMP_PRO_DES_1",
        image: "synapse/gr-lamp-pro.png",
        url: "https://www.razer.com/gamer-room-lights/razer-aether-lamp-pro",
    },
    Product {
        name: "AETHER_LAMP",
        description: "AETHER_LAMP_PRO_DES_2",
        image: "synapse/gr-lamp.png",
        url: "https://www.razer.com/gamer-room-lights/razer-aether-lamp",
    },
];

fn product_info(index: usize, cx: &App) -> AnyElement {
    let item = &PRODUCTS[index];
    let content = v_flex()
        .min_w_0()
        .gap(surface::css(16.))
        .items_center()
        .child(
            img(item.image)
                .w(surface::css(250.))
                .max_w_full()
                .h(surface::css(140.))
                .object_fit(ObjectFit::Contain),
        )
        .child(
            div()
                .font_family("RazerF5")
                .text_size(surface::css(18.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .text_center()
                .child(i18n::t(item.name).to_uppercase()),
        )
        .child(
            div()
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_center()
                .whitespace_normal()
                .when(index >= 2, |view| view.min_w(surface::css(284.)))
                .child(i18n::t(item.description)),
        )
        .text_color(cx.theme().foreground);
    gpui_kit::base::Link::new(SharedString::from(format!("gr-product-{}", item.name)))
        .href(item.url)
        .accessibility_label(i18n::t(item.name))
        .open_with(|url, _, _, cx| cx.open_url(url))
        .cursor_pointer()
        .focus_visible(|view| view.bg(cx.theme().secondary_hover))
        .child(content)
        .into_any_element()
}

pub struct GamerRoomPage {
    collapsed: [bool; 2],
    hovered_product: Option<usize>,
    devices: Vec<devices::Item>,
    banner_visible: bool,
    device_popup: Option<String>,
    device_popup_bounds: Rc<Cell<Bounds<Pixels>>>,
    tour_step: Option<usize>,
    stored_seen: Option<bool>,
    add_dialog: Option<Entity<iot_popup::IotPopup>>,
    add_dialog_subscription: Option<Subscription>,
}
pub enum GamerRoomEvent {
    TutorialCompleted,
    OpenDevice {
        product_id: u32,
        serial_number: String,
        device_container_id: String,
    },
    DeviceCommand {
        product_id: u32,
        device_container_id: String,
        action: &'static str,
        payload: serde_json::Value,
    },
}
impl EventEmitter<GamerRoomEvent> for GamerRoomPage {}
impl GamerRoomPage {
    pub fn new() -> Self {
        Self {
            collapsed: [false; 2],
            hovered_product: None,
            devices: vec![],
            banner_visible: true,
            device_popup: None,
            device_popup_bounds: Rc::new(Cell::new(Bounds::default())),
            tour_step: None,
            stored_seen: None,
            add_dialog: None,
            add_dialog_subscription: None,
        }
    }
    pub fn set_tutorial_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        if self.stored_seen == Some(seen) {
            return;
        }
        self.stored_seen = Some(seen);
        self.tour_step = if seen { None } else { Some(0) };
        cx.notify();
    }
    pub fn reset_tutorial(&mut self, cx: &mut Context<Self>) {
        self.stored_seen = Some(false);
        self.tour_step = Some(0);
        cx.notify();
    }
    fn complete_tutorial(&mut self, cx: &mut Context<Self>) {
        self.tour_step = None;
        self.stored_seen = Some(true);
        cx.emit(GamerRoomEvent::TutorialCompleted);
        cx.notify();
    }
    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_add_kind(iot_popup::DeviceKind::GamerRoom, window, cx);
    }
    fn open_add_kind(
        &mut self,
        kind: iot_popup::DeviceKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .add_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.read(cx).is_open())
        {
            return;
        }
        let view = iot_popup::open(kind, window, cx);
        // Background marketing popups do not belong to the modal's focus or
        // pointer interaction. Keep the tutorial step for after it closes.
        self.hovered_product = None;
        self.add_dialog_subscription =
            Some(cx.subscribe(&view, |this, _, _: &DismissEvent, cx| {
                this.add_dialog = None;
                this.add_dialog_subscription = None;
                cx.notify();
            }));
        self.add_dialog = Some(view);
        cx.notify();
    }
    fn banner(&self, wide: bool, cx: &mut Context<Self>) -> AnyElement {
        let colors = MainPageColors;
        div()
            .id("gamer-room-banner")
            .test_support()
            .relative()
            .w_full()
            .min_w_0()
            .h(surface::css(if wide { 930. } else { 531. }))
            .when(wide, |view| view.w(surface::css(2500.)))
            .rounded(cx.theme().font_size * (5. / 16.))
            .overflow_hidden()
            .child(
                img("synapse/gr-background.png")
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            // GPUI gradients have two stops. Adjacent bands reproduce the
            // original .fade's four stops without baking text into the image.
            .children(
                [
                    (0., 0.1563, 217. / 255., 217. / 255.),
                    (0.1563, 0.4323, 217. / 255., 0.4),
                    (0.4323, 0.714, 0.4, 26. / 255.),
                    (0.714, 1., 26. / 255., 1.),
                ]
                .into_iter()
                .map(|(start, end, from, to)| {
                    div()
                        .absolute()
                        .left_0()
                        .w_full()
                        .top(relative(start))
                        .h(relative(end - start))
                        .bg(linear_gradient(
                            180.,
                            linear_color_stop(colors.banner_shade().opacity(from), 0.),
                            linear_color_stop(colors.banner_shade().opacity(to), 1.),
                        ))
                }),
            )
            .child(
                v_flex()
                    .absolute()
                    .top(surface::css(26.))
                    .left(surface::css(20.))
                    .right(surface::css(20.))
                    .gap(surface::css(16.))
                    .items_center()
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(24.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().primary)
                            .text_center()
                            .child(i18n::t("RAZER_GAMER_ROOM")),
                    )
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(16.))
                            .text_color(colors.banner_heading())
                            .text_center()
                            .child(i18n::t("RAZER_GAMER_ROOM_TILE")),
                    )
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .text_center()
                            .whitespace_normal()
                            .child(i18n::t("RAZER_GAMER_ROOM_DES")),
                    ),
            )
            .child(
                h_flex()
                    .absolute()
                    .bottom(surface::css(35.))
                    .w_full()
                    .justify_center()
                    .child(
                        gpui_kit::base::Link::new("gamer-room-learn")
                            .href("https://www.razer.com/pc/gamer-room")
                            .accessibility_label(i18n::t("LEARN_MORE"))
                            .open_with(|url, _, _, cx| cx.open_url(url))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .text_color(rgb(0xcccccc))
                            .underline()
                            .hover(|view| view.text_color(rgb(0x44d62c)))
                            .child(i18n::t("LEARN_MORE"))
                            .child(
                                Icon::default()
                                    .path("synapse/gr-external-link.svg")
                                    .w(surface::css(21.))
                                    .h(surface::css(20.)),
                            ),
                    ),
            )
            .children(
                [(0, 0.10, 0.37), (1, 0.39, 0.68), (2, 0.15, 0.64)]
                    .into_iter()
                    .map(|(index, x, y)| {
                        h_flex()
                            .absolute()
                            .top(relative(y))
                            .when(index == 2, |view| view.right(relative(x)))
                            .when(index != 2, |view| view.left(relative(x)))
                            .when(index != 2, |view| {
                                view.child(
                                    SourceLayer::new(
                                        HotspotPulse::new(if index == 0 {
                                            "gr-pulse-bulb"
                                        } else {
                                            "gr-pulse-strip"
                                        })
                                        .mr(surface::css(-10.)),
                                        0.,
                                        0.,
                                    )
                                    .priority(1),
                                )
                            })
                            .child(
                                BaseButton::new(SharedString::from(format!(
                                    "gr-hotspot-{}",
                                    PRODUCTS[index].name
                                )))
                                .accessibility_label(i18n::t(PRODUCTS[index].name))
                                .w_auto()
                                .h_auto()
                                .flex_shrink_0()
                                .px(surface::css(8.))
                                .py(surface::css(8.))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .whitespace_nowrap()
                                .rounded(surface::css(5.))
                                .bg(colors.banner_shade().opacity(0.5))
                                .text_color(cx.theme().foreground)
                                .shadow(vec![BoxShadow {
                                    color: colors.banner_heading().opacity(0.2),
                                    offset: point(px(0.), px(0.)),
                                    blur_radius: surface::css(15.).to_pixels(cx.theme().font_size),
                                    spread_radius: px(0.),
                                    inset: false,
                                }])
                                .focus_visible(|style| style.bg(colors.banner_shade().opacity(0.7)))
                                .child(i18n::t(PRODUCTS[index].name))
                                .on_hover(cx.listener(
                                    move |this, hovered: &bool, _, cx| {
                                        if *hovered && this.hovered_product != Some(index) {
                                            this.hovered_product = Some(index);
                                            cx.notify();
                                        }
                                    },
                                )),
                            )
                            .when(index == 2, |view| {
                                view.child(
                                    SourceLayer::new(
                                        HotspotPulse::new("gr-pulse-lamp").ml(surface::css(-10.)),
                                        0.,
                                        0.,
                                    )
                                    .priority(1),
                                )
                            })
                            .child(self.product_overlay(index, cx))
                    }),
            )
            .into_any_element()
    }
    fn product_overlay(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        if self.hovered_product != Some(index) {
            return div().into_any_element();
        }
        SourceLayer::new(
            self.product_panel(index, cx),
            if index == 2 { -0.865 } else { -0.235 },
            -0.5,
        )
        .priority(2)
        .into_any_element()
    }
    fn product_panel(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .id("gr-marketing-panel")
            .test_support()
            .absolute()
            .left(relative(0.5))
            .top(relative(0.5))
            .min_h(surface::css(312.))
            .min_w(surface::css(400.))
            .py(surface::css(36.))
            .px(surface::css(24.))
            .gap(surface::css(36.))
            .justify_center()
            .bg(MainPageColors.banner_shade().opacity(0.5))
            .rounded(cx.theme().font_size * (5. / 16.))
            .child(product_info(index, cx))
            .when(index == 2, |view| view.child(product_info(3, cx)))
            // Current o clears hoveredItem only on leaving this container.
            // A hotspot click does not pin the marketing panel.
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if !*hovered && this.hovered_product == Some(index) {
                    this.hovered_product = None;
                    cx.notify();
                }
            }))
            .into_any_element()
    }
    fn group(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let (title, tip, description) = if index == 0 {
            (
                "SYNAPSE_OVERRIDE_HEADER",
                "SYNAPSE_OVERRIDE_TIP",
                "SYNAPSE_OVERRIDE_MSG",
            )
        } else {
            (
                "CONTROLLED_BY_GAMER_ROOM",
                "SMART_HOME_TOOLTIP",
                "CONTROLLED_BY_GAMER_ROOM_MSG",
            )
        };
        v_flex()
            .id(SharedString::from(format!("gr-group-{index}")))
            .test_support()
            .relative()
            .w_full()
            // 9388 uses `(ie.boxGroup, { zIndex })`: the declared 30px style
            // is discarded, leaving the actual `.dashboard .box-group` margin.
            .my(surface::css(10.))
            .child(
                h_flex()
                    .group(if index == 0 {
                        "gr-collapse-0"
                    } else {
                        "gr-collapse-1"
                    })
                    .w_full()
                    .text_color(rgb(0xcccccc))
                    .hover(|style| style.text_color(rgb(0xffffff)))
                    .child(
                        BaseButton::new(SharedString::from(format!("gr-group-toggle-{index}")))
                            .accessibility_label(i18n::t(title))
                            .aria_expanded(!self.collapsed[index])
                            .self_start()
                            .h(surface::css(18.))
                            .justify_start()
                            .p_0()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(18.))
                            .focus_visible(|style| style.bg(cx.theme().secondary_hover))
                            .child(
                                h_flex()
                                    .gap(surface::css(10.))
                                    .child(CollapseArrow {
                                        id: if index == 0 {
                                            "gr-collapse-0"
                                        } else {
                                            "gr-collapse-1"
                                        },
                                        collapsed: self.collapsed[index],
                                    })
                                    .child(i18n::t(title)),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.collapsed[index] = !this.collapsed[index];
                                cx.notify();
                            })),
                    )
                    .child(GroupHelp {
                        id: if index == 0 { "gr-help-0" } else { "gr-help-1" },
                        tip,
                    }),
            )
            .child(GroupContent {
                id: if index == 0 {
                    "gr-content-0"
                } else {
                    "gr-content-1"
                },
                collapsed: self.collapsed[index],
                child: self.group_items(index, description, cx),
            })
            .into_any_element()
    }
    fn tutorial_overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Pe is a zero-height sibling immediately before the first box-group,
        // inside je's column. Base Popup always snaps to the window's edges;
        // retain Base's focus/dismissal state without changing this containing
        // block or detaching the tutorial from the page's scroll region.
        let state = window.use_keyed_state("gamer-room-tutorial-state", cx, |_, cx| {
            gpui_kit::base::PopoverState::new(false, cx)
        });
        let open = self.tour_step.is_some() && self.add_dialog.is_none();
        let on_open_change = Rc::new(cx.listener(|this, open: &bool, _, cx| {
            if !*open && this.tour_step.is_some() {
                this.complete_tutorial(cx);
            }
        }));
        state.update(cx, |state, cx| {
            state.set_on_open_change(Some(on_open_change));
            state.sync_open(open, window, cx);
        });
        if !open {
            return div().into_any_element();
        }
        let step = self.tour_step.unwrap_or(0);
        let focus = state.focus_handle(cx);
        div()
            .id("gamer-room-tutorial-wrapper")
            .test_support()
            .absolute()
            .left_0()
            .top_0()
            .w_full()
            .h_0()
            .child(
                div()
                    .id("gamer-room-tutorial-surface")
                    .absolute()
                    .left(surface::css(if step == 0 { 220. } else { 213. }))
                    .top(surface::css(if step == 0 { 22. } else { -25. }))
                    .role(Role::Dialog)
                    .aria_label(i18n::t("GAMER_ROOM_TUTORIAL_HEADER"))
                    .block_mouse_except_scroll()
                    .tab_group()
                    .track_focus(&focus)
                    .key_context("Popover")
                    .on_action(
                        window.listener_for(&state, gpui_kit::base::PopoverState::on_action_cancel),
                    )
                    .child(self.tour(step, window, cx)),
            )
            .into_any_element()
    }
    fn group_placeholder(
        &self,
        index: usize,
        description: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = v_flex()
            .w_full()
            .min_w_0()
            .items_center()
            .justify_center()
            .gap(surface::css(8.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_center()
            .whitespace_normal()
            .text_color(MainPageColors.empty_group_text())
            .child(
                div()
                    .w_full()
                    .whitespace_normal()
                    .child(i18n::t(description)),
            )
            .when(index == 0, |view| {
                view.child(
                    div()
                        .w_full()
                        .whitespace_normal()
                        .underline()
                        .child(i18n::t("ADD_NEW_DEVICE")),
                )
            });
        if index == 0 {
            BaseButton::new("gamer-room-add")
                .accessibility_label(i18n::t("ADD_OTHER_WIFI_DEVICE"))
                .self_start()
                .w(surface::css(186.))
                .h(surface::css(176.))
                .flex_shrink_0()
                .p(surface::css(15.))
                .border_2()
                .border_dashed()
                .border_color(cx.theme().border)
                .rounded(cx.theme().font_size * (5. / 16.))
                .focus_visible(|style| style.border_color(cx.theme().primary))
                .child(content)
                .on_click(cx.listener(|this, _, w, cx| this.open_add(w, cx)))
                .into_any_element()
        } else {
            div()
                .w(surface::css(186.))
                .h(surface::css(176.))
                .p(surface::css(15.))
                .flex()
                .items_center()
                .justify_center()
                .border_2()
                .border_dashed()
                .border_color(cx.theme().border)
                .rounded(cx.theme().font_size * (5. / 16.))
                .child(content)
                .into_any_element()
        }
    }
    fn tour(&self, step: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let texts = [
            "GAMER_ROOM_TUTORIAL_HEADER_DESC_1",
            "GAMER_ROOM_TUTORIAL_HEADER_DESC_2",
        ];
        let videos = [
            "synapse/tutorial-gamer-room-1.webp",
            "synapse/tutorial-gamer-room-2.webp",
        ];
        v_flex()
            .id("gamer-room-tour")
            .test_support()
            .relative()
            .w(surface::css(290.))
            .bg(cx.theme().group_box)
            .border_1()
            .border_color(MainPageColors.tutorial_accent())
            .rounded(cx.theme().font_size * (5. / 16.))
            .child(
                TutorialIndicator::new("gamer-room-tutorial-indicator")
                    .absolute()
                    .left(relative(if step == 0 { -0.09 } else { -0.1 }))
                    .ml(surface::css(-18.))
                    .when(step == 0, |view| view.top_0())
                    .when(step != 0, |view| {
                        view.top(relative(0.5)).mt(surface::css(-18.))
                    }),
            )
            .child(
                gpui_kit::base::Button::new("gr-tour-skip")
                    .accessibility_label(i18n::t("TUTORIAL_SKIP"))
                    .absolute()
                    .top(surface::css(15.))
                    .right(surface::css(20.))
                    .p_0()
                    .text_size(surface::css(12.))
                    .line_height(relative(1.5))
                    .text_color(cx.theme().foreground)
                    .underline()
                    .hover(|style| style.text_color(cx.theme().primary))
                    .focus_visible(|style| style.text_color(cx.theme().primary))
                    .child(i18n::t("TUTORIAL_SKIP").to_uppercase())
                    .on_click(cx.listener(|this, _, _, cx| this.complete_tutorial(cx))),
            )
            .child(
                div().id("gamer-room-tutorial-content").w_full().child(
                    v_flex()
                        .pt(surface::css(40.))
                        .px(surface::css(20.))
                        .pb(surface::css(20.))
                        .child(razer_widgets::tutorial_media::clip(
                            videos[step],
                            i18n::t("GAMER_ROOM_TUTORIAL_HEADER"),
                            250. / 190.,
                        ))
                        .child(
                            div()
                                .text_size(surface::css(16.))
                                .text_color(MainPageColors.tutorial_accent())
                                .mb(surface::css(5.))
                                .child(i18n::t("GAMER_ROOM_TUTORIAL_HEADER")),
                        )
                        .child(
                            div()
                                .text_size(surface::css(14.))
                                .whitespace_normal()
                                .child(i18n::t(texts[step])),
                        )
                        .child(
                            h_flex()
                                .mt(surface::css(20.))
                                .justify_center()
                                .gap(surface::css(10.))
                                .child(
                                    tutorial_button(
                                        "gr-tour-back",
                                        i18n::t("BACK"),
                                        false,
                                        step == 0,
                                        window,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.tour_step = Some(0);
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    tutorial_button(
                                        "gr-tour-next",
                                        i18n::t(if step == 0 { "NEXT" } else { "DONE" }),
                                        true,
                                        false,
                                        window,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            if step == 0 {
                                                this.tour_step = Some(1);
                                                cx.notify();
                                            } else {
                                                this.complete_tutorial(cx);
                                            }
                                        },
                                    )),
                                ),
                        )
                        .child(
                            div()
                                .mt(surface::css(10.))
                                .text_size(surface::css(12.))
                                .text_center()
                                .child(format!("{} / 2", step + 1)),
                        ),
                ),
            )
            .into_any_element()
    }
}
impl Render for GamerRoomPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport_width =
            f32::from(window.viewport_size().width) * 16. / f32::from(window.rem_size()).max(1.);
        let wide = viewport_width >= 2560.;
        let groups_max_width = if wide {
            2500.
        } else if viewport_width <= 600. {
            290.
        } else if viewport_width < 1280. {
            910.
        } else {
            1220.
        };
        v_flex()
            .id("gamer-room")
            .test_support()
            .w_full()
            .min_w_0()
            .when(self.banner_visible, |view| {
                view.child(self.banner(wide, cx))
            })
            .child(
                v_flex()
                    .id("gamer-room-groups")
                    .test_support()
                    .relative()
                    .w_full()
                    .min_w(surface::css(if viewport_width <= 600. { 0. } else { 620. }))
                    .max_w(surface::css(groups_max_width))
                    .mx_auto()
                    .mb(surface::css(50.))
                    .child(self.group(0, cx))
                    .child(self.group(1, cx))
                    // Paint last to retain Pe's z-index above both groups. Its
                    // zero-height origin still precedes the first 10px margin.
                    .child(
                        SourceLayer::new(self.tutorial_overlay(window, cx), 0., 0.).priority(1050),
                    ),
            )
            .when_some(self.add_dialog.clone(), |view, dialog| view.child(dialog))
    }
}

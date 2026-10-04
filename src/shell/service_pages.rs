//! Main frontend 9388/19388, IotPopupRoot/28256 and 6505/44442.
//! Browsing and navigation are local. Native discovery/install results are never
//! synthesized from clicks, timers, or the catalogue of supported products.
use super::{app_picker::PickerModule, iot_popup};
use crate::ui::scroll::SourceScrollable as _;
use crate::{
    i18n,
    model::Device,
    ui::{
        surface,
        theme::MainPageColors,
        tutorial::{TutorialIndicator, tutorial_button},
    },
};
use gpui_kit::base::{
    Button as BaseButton,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{collections::BTreeSet, rc::Rc, time::Duration};

#[path = "module_preview.rs"]
mod module_preview;

#[cfg(test)]
#[path = "gamer_room_tutorial_tests.rs"]
mod tutorial_tests;

#[cfg(test)]
#[path = "service_button_tests.rs"]
mod button_tests;

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

pub(super) fn source_link(
    id: &'static str,
    label: impl Into<SharedString>,
    url: &'static str,
    cx: &App,
) -> gpui_kit::base::Link {
    let label = label.into();
    gpui_kit::base::Link::new(id)
        .href(url)
        .accessibility_label(label.clone())
        .open_with(|url, _, _, cx| cx.open_url(url))
        .cursor_pointer()
        .text_size(surface::css(14.))
        .text_color(cx.theme().foreground)
        .underline()
        .hover(|view| view.text_color(cx.theme().primary))
        .focus_visible(|view| {
            view.bg(cx.theme().secondary_hover)
                .text_color(cx.theme().primary)
        })
        .child(label)
}

/// 6505's `.item-action.btn`: max-content width, 90px minimum, 27px border box.
/// Base owns activation; a direct text child keeps the source 12px measurement
/// independent of Component Button's full-size label slot and default type size.
pub(super) fn module_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    disabled: bool,
    cx: &App,
) -> ModuleAction {
    let id = id.into();
    let label = label.into().to_uppercase();
    let button = BaseButton::new(id.clone())
        .accessibility_label(label.clone())
        .disabled(disabled)
        .w_auto()
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .flex_shrink_0()
        .px(surface::css(16.))
        .pt(surface::css(7.))
        .pb(surface::css(6.))
        .border_1()
        .border_color(crate::ui::theme::PaletteColors.swatch_border())
        .rounded(surface::css(2.))
        .text_size(surface::css(12.))
        .line_height(surface::css(12.))
        .whitespace_nowrap()
        .bg(if primary {
            cx.theme().primary
        } else {
            MainPageColors.module_action_gray()
        })
        .text_color(if primary {
            MainPageColors.banner_shade()
        } else {
            MainPageColors.banner_heading()
        })
        .cursor_default()
        .when(!disabled, |button| {
            button.focus_visible(|style| style.border_color(cx.theme().foreground))
        })
        .child(label);
    ModuleAction {
        id,
        button,
        disabled,
    }
}

/// Keeps the source opacity transition on the whole button, including its label
/// and border. Rendering supplies Window only for retained presentation state;
/// command activation, keyboard handling and focus remain owned by Base Button.
#[derive(IntoElement)]
pub(super) struct ModuleAction {
    id: ElementId,
    button: BaseButton,
    disabled: bool,
}

impl ModuleAction {
    pub(super) fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.button = self.button.on_click(handler);
        self
    }
}

impl Styled for ModuleAction {
    fn style(&mut self) -> &mut StyleRefinement {
        self.button.style()
    }
}

impl InteractiveElement for ModuleAction {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.button.interactivity()
    }
}

impl StatefulInteractiveElement for ModuleAction {}

#[derive(Default)]
struct ModuleActionInteraction {
    hovered: bool,
    pressed: bool,
}

impl RenderOnce for ModuleAction {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled = self.disabled;
        let state = window.use_keyed_state(
            (self.id.clone(), "module-button-interaction"),
            cx,
            |_, _| ModuleActionInteraction::default(),
        );
        let interaction = state.read(cx);
        let target = if disabled {
            0.3
        } else if interaction.pressed && interaction.hovered {
            0.6
        } else if interaction.hovered {
            0.8
        } else {
            1.
        };
        let opacity = motion::transition(
            (self.id, "module-button-opacity"),
            target,
            Transition::new(Duration::from_millis(200)).easing(Easing::EaseOut),
            window,
            cx,
        );
        self.button
            .opacity(opacity)
            .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                state.hovered = *hovered;
                if !hovered {
                    state.pressed = false;
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, move |state, _, _, cx| {
                    state.pressed = !disabled;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
    }
}

/// The source's underlined `.info-text.link` is a text-sized in-app command.
pub(super) fn module_detail_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> BaseButton {
    let label = label.into();
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .w_auto()
        .h_auto()
        .flex_shrink_0()
        .p_0()
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .whitespace_nowrap()
        .text_color(MainPageColors.card_caption())
        .underline()
        .cursor_default()
        .hover(|style| style.text_color(cx.theme().primary))
        .active(|style| style.text_color(cx.theme().primary.opacity(0.7)))
        .focus_visible(|style| style.text_color(cx.theme().primary))
        .child(label)
}

fn product_info(index: usize, cx: &App) -> AnyElement {
    let item = &PRODUCTS[index];
    let content = v_flex()
        .w(surface::css(284.))
        .max_w_full()
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
                .text_center()
                .child(i18n::t(item.name).to_uppercase()),
        )
        .child(
            div()
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_center()
                .whitespace_normal()
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

pub(super) struct GamerRoomPage {
    collapsed: [bool; 2],
    hovered_product: Option<usize>,
    active_product: Option<usize>,
    tour_step: Option<usize>,
    stored_seen: Option<bool>,
    add_dialog: Option<Entity<iot_popup::IotPopup>>,
    add_dialog_subscription: Option<Subscription>,
}
pub(super) enum GamerRoomEvent {
    TutorialCompleted,
}
impl EventEmitter<GamerRoomEvent> for GamerRoomPage {}
impl GamerRoomPage {
    pub(super) fn new() -> Self {
        Self {
            collapsed: [false; 2],
            hovered_product: None,
            active_product: None,
            tour_step: None,
            stored_seen: None,
            add_dialog: None,
            add_dialog_subscription: None,
        }
    }
    pub(super) fn set_tutorial_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        if self.stored_seen == Some(seen) {
            return;
        }
        self.stored_seen = Some(seen);
        self.tour_step = if seen { None } else { Some(0) };
        cx.notify();
    }
    pub(super) fn reset_tutorial(&mut self, cx: &mut Context<Self>) {
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
        self.active_product = None;
        self.add_dialog_subscription =
            Some(cx.subscribe(&view, |this, _, _: &DismissEvent, cx| {
                this.add_dialog = None;
                this.add_dialog_subscription = None;
                cx.notify();
            }));
        self.add_dialog = Some(view);
        cx.notify();
    }
    fn open_product(&mut self, index: usize, _: &mut Window, cx: &mut Context<Self>) {
        self.hovered_product = None;
        self.active_product = Some(index);
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
                    .child(source_link(
                        "gamer-room-learn",
                        i18n::t("LEARN_MORE"),
                        "https://www.razer.com/pc/gamer-room",
                        cx,
                    )),
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
                                    img("synapse/gr-hotspot.svg")
                                        .size(surface::css(40.))
                                        .mr(surface::css(-10.)),
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
                                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                    if *hovered && this.hovered_product != Some(index) {
                                        this.hovered_product = Some(index);
                                        cx.notify();
                                    }
                                }))
                                .on_click(cx.listener(
                                    move |this, _, w, cx| this.open_product(index, w, cx),
                                )),
                            )
                            .when(index == 2, |view| {
                                view.child(
                                    img("synapse/gr-hotspot.svg")
                                        .size(surface::css(40.))
                                        .ml(surface::css(-10.)),
                                )
                            })
                            .child(self.product_overlay(index, cx))
                    }),
            )
            .into_any_element()
    }
    fn product_overlay(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let width = if index == 2 { 652. } else { 400. };
        let pinned = self.active_product == Some(index);
        let owner = cx.entity().downgrade();
        div()
            .absolute()
            .left(relative(0.5))
            .top(relative(0.5))
            .ml(surface::css(
                -width * if index == 2 { 0.865 } else { 0.235 },
            ))
            .child(if pinned {
                gpui_kit::base::Popover::new(SharedString::from(format!(
                    "gr-marketing-popover-{}",
                    PRODUCTS[index].name
                )))
                .anchor(Anchor::LeftCenter)
                .offset(px(0.))
                .open(true)
                .trigger_with(|_, _, _| div().size_0().into_any_element())
                .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                    if !*open {
                        this.active_product = None;
                        cx.notify();
                    }
                }))
                .content(move |_, _, cx| {
                    owner
                        .update(cx, |this, cx| this.product_panel(index, cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .into_any_element()
            } else if self.active_product.is_none() && self.hovered_product == Some(index) {
                gpui_kit::base::Popup::new(
                    SharedString::from(format!("gr-marketing-hover-{}", PRODUCTS[index].name)),
                    div().size_0(),
                )
                .anchor(Anchor::LeftCenter)
                .offset(px(0.))
                .content(self.product_panel(index, cx))
                .into_any_element()
            } else {
                div().into_any_element()
            })
            .into_any_element()
    }
    fn product_panel(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .id("gr-marketing-panel")
            .test_support()
            .min_h(surface::css(312.))
            .w(surface::css(if index == 2 { 652. } else { 400. }))
            .py(surface::css(36.))
            .px(surface::css(24.))
            .gap(surface::css(36.))
            .justify_center()
            .bg(MainPageColors.banner_shade().opacity(0.5))
            .rounded(cx.theme().font_size * (5. / 16.))
            .child(product_info(index, cx))
            .when(index == 2, |view| view.child(product_info(3, cx)))
            // The deferred Popup occludes the banner and its hotspot. The
            // source closes on leaving this panel, not on leaving the banner.
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
                "CONTROLLED_GAMER_ROOM_APP_TIP",
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
                BaseButton::new(SharedString::from(format!("gr-group-toggle-{index}")))
                    .accessibility_label(i18n::t(title))
                    .aria_expanded(!self.collapsed[index])
                    .self_start()
                    .h(surface::css(18.))
                    .justify_start()
                    .p_0()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(18.))
                    .text_color(cx.theme().foreground)
                    .hover(|style| style.text_color(MainPageColors.banner_heading()))
                    .focus_visible(|style| style.bg(cx.theme().secondary_hover))
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                Icon::default()
                                    .path("synapse/expand.svg")
                                    .size(surface::css(10.))
                                    .transform(Transformation::rotate(radians(
                                        if self.collapsed[index] {
                                            -std::f32::consts::FRAC_PI_2
                                        } else {
                                            0.
                                        },
                                    ))),
                            )
                            .child(i18n::t(title)),
                    )
                    .tooltip(move |window, cx| Tooltip::new(i18n::t(tip)).build(window, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.collapsed[index] = !this.collapsed[index];
                        cx.notify();
                    })),
            )
            .when(!self.collapsed[index], |view| {
                view.child(v_flex().mt(surface::css(10.)).child(self.group_placeholder(
                    index,
                    description,
                    cx,
                )))
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
                        .child(crate::ui::tutorial_media::clip(
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
            .child(self.banner(wide, cx))
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
                    .child(self.tutorial_overlay(window, cx)),
            )
            .when_some(self.add_dialog.clone(), |view, dialog| view.child(dialog))
    }
}

struct Module {
    id: &'static str,
    /// 原版模块盒的盒名（Dashboard 模块 54693 的常量，`showModules` 的 switch 用它分派）。
    box_name: &'static str,
    /// 点击该盒聚焦的窗口名与地址（Dashboard 模块 54420 的窗口登记表）。
    window: &'static str,
    url: &'static str,
    title: &'static str,
    icon: &'static str,
    image: Option<&'static str>,
    description: &'static str,
    link: Option<&'static str>,
    /// A compiled page is available independently of an external module install.
    native_page: Option<ModulePage>,
}
/// 已在本仓库实现的对应窗口；原版点击模块盒只是聚焦同名窗口。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModulePage {
    Picker(PickerModule),
    /// Alexa is registered by the current Dashboard as the named `alexa`
    /// application window (`policy=3`, `/synapse/alexa/`).  Keep it separate
    /// from the host tab route used by Settings previews.
    Alexa,
    IntroductionTour,
    Macro,
    Armory,
    Profiles,
    Feedback,
}
// 6505/44442 ne te are a static catalogue. No installed/available state is
// inferred from these records; that state belongs to the installer service.
// 盒名、窗口名与地址来自当前 Dashboard：模块 54420 登记具名窗口，7861 的
// `showModules` 把盒名映射到窗口名（`focusTab(windowName)`），未安装时先打开
// `/installer/#type=module&id=<id>&location=<path>` 再聚焦窗口。运行时盒子的顺序
// 由宿主的 group items 决定，本地不臆造顺序。收据见 docs/re/display-window-contract.md
// 与 docs/re/macro-app-current-audit.md。
const MODULES: &[Module] = &[
    Module {
        id: "alexa",
        box_name: "DASHBOARD_ALEXA",
        window: "alexa",
        url: "/synapse/alexa/",
        title: "Alexa",
        icon: "synapse/module-alexa.svg",
        image: Some("synapse/module-alexa.png"),
        description: "对于所有支持 Chroma 幻彩的设备，Amazon Alexa 模块将完整的 Alexa Voice Service 集成到 Synapse 雷云中。需要有效的麦克风和 Amazon Alexa 账户。",
        link: Some("https://www.razer.com/chroma/alexa"),
        native_page: Some(ModulePage::Alexa),
    },
    Module {
        id: "macro",
        box_name: "MACRO",
        window: "macro",
        url: "/synapse/macro/",
        title: "宏",
        icon: "synapse/module-macro.svg",
        image: Some("synapse/module-macro.png"),
        description: "Independent Macro application window; local editor chrome and draft actions are available, while macro transport and device binding remain service boundaries.",
        link: None,
        // 原版此盒聚焦名为 `macro` 的窗口（`/synapse/macro/`）；本地已实现该窗口的
        // 外框与两个导航标签，宏服务与功能面板仍未接入。
        native_page: Some(ModulePage::Macro),
    },
    Module {
        id: "linked-games",
        box_name: "LINKED_GAMES",
        window: "profiles",
        url: "/synapse/profiles/",
        title: "已关联的游戏",
        icon: "synapse/module-linked-games.svg",
        image: None,
        description: "Independent Profiles application window; games and devices views, local profile associations and local game tiles are available. Service scanning remains deferred.",
        link: None,
        // 模块表：`linkedGames → windowName:"profiles", url:"/synapse/profiles/"`，
        // 打开参数 `policy=3,shouldFocus=1,tab_visible=1`（同窗口 + 聚焦 + 标签可见）。
        native_page: Some(ModulePage::Profiles),
    },
    Module {
        id: "feedback",
        box_name: "FEEDBACK",
        window: "feedback-synapse",
        // 模块表原文：`/feedback/?app=synapse&path=${encodeURIComponent("/synapse/dashboard")}`。
        url: "/feedback/?app=synapse&path=%2Fsynapse%2Fdashboard",
        title: "反馈",
        icon: "synapse/module-feedback.svg",
        image: None,
        description: "Current feedback-synapse form; local validation and drafts are available while submission and log export remain unavailable without the service.",
        link: None,
        native_page: Some(ModulePage::Feedback),
    },
    Module {
        id: "armory",
        box_name: "DASHBOARD_WORKSHOP",
        window: "armory",
        url: "/synapse/armory/",
        title: "工坊",
        icon: "synapse/module-armory.svg",
        image: None,
        description: "Independent Armory application window; source navigation, banner and local filter/sort surfaces are available. Workshop content service remains deferred.",
        link: None,
        // 原版此盒打开 `armory` 窗口（`/synapse/armory/`）；本地已实现该窗口的
        // 外框与四个导航标签，资料分享服务仍未接入。
        native_page: Some(ModulePage::Armory),
    },
    Module {
        id: "profile-migration",
        box_name: "PROFILE_MIGRATION",
        window: "syn3-profile-migration",
        url: "/profile-migration/",
        title: "配置文件迁移",
        icon: "synapse/module-profile-migration.svg",
        image: None,
        description: "把 Synapse 3 的配置文件迁移到当前版本。原版点击此盒聚焦 syn3-profile-migration 窗口，本地已实现该页面。",
        link: None,
        native_page: Some(ModulePage::Picker(PickerModule::ProfileMigration)),
    },
    Module {
        id: "tour",
        box_name: "TOUR",
        window: "synapse-introduction",
        url: "/synapse/introduction-tour/",
        title: "介绍导览",
        // 导览应用自己的图标（与 host_tabs.rs 里 Tour 标签页同一份资源）。
        icon: "synapse/tour-app-icon.svg",
        image: None,
        description: "原版点击此盒聚焦 synapse-introduction 窗口，本地已实现 Synapse 介绍导览。",
        link: None,
        native_page: Some(ModulePage::IntroductionTour),
    },
];
pub(super) struct ModuleCatalog {
    expanded: BTreeSet<&'static str>,
    details: Option<Entity<DeviceDetails>>,
}
pub(super) enum ModuleCatalogEvent {
    OpenModule(ModulePage),
    FirmwareUpdate {
        device: Option<Device>,
        preview: bool,
    },
}
impl EventEmitter<ModuleCatalogEvent> for ModuleCatalog {}
impl ModuleCatalog {
    pub(super) fn new() -> Self {
        Self {
            expanded: BTreeSet::new(),
            details: None,
        }
    }
    /// Explicit UI samples: no installer transport or device mutation is involved.
    pub(super) fn open_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        module_preview::open(cx.entity().downgrade(), window, cx);
    }
    pub(super) fn open_device_details(
        &mut self,
        device: Device,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.details.is_some() {
            return;
        }
        let owner = cx.entity().downgrade();
        let return_focus = window.focused(cx);
        let details = cx.new(|cx| DeviceDetails {
            device,
            focus: cx.focus_handle(),
            return_focus,
            owner,
        });
        window.focus(&details.read(cx).focus.clone(), cx);
        self.details = Some(details);
        cx.notify();
    }
    fn module_row(&self, item: &'static Module, cx: &mut Context<Self>) -> AnyElement {
        let expanded = self.expanded.contains(item.id);
        v_flex()
            .id(SharedString::from(format!("catalog-{}", item.id)))
            .test_support()
            .w_full()
            .child(
                h_flex()
                    .h(surface::css(80.))
                    .pl(surface::css(20.))
                    .pr(surface::css(30.))
                    .mb(surface::css(1.))
                    .bg(cx.theme().group_box)
                    .child(img(item.icon).size(surface::css(40.)))
                    .child(
                        div()
                            .ml(surface::css(10.))
                            .w(surface::css(500.))
                            .flex_shrink_0()
                            .text_size(surface::css(16.))
                            .text_ellipsis()
                            .child(item.title),
                    )
                    .child(div().flex_1().min_w_0().flex().items_center().when(
                        item.image.is_some(),
                        |view| {
                            view.child(
                                module_detail_action(
                                    SharedString::from(format!("module-details-{}", item.id)),
                                    i18n::t(if expanded {
                                        "CLOSE"
                                    } else {
                                        "MORE_INFORMATION"
                                    }),
                                    cx,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if !this.expanded.remove(item.id) {
                                            this.expanded.insert(item.id);
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                        },
                    ))
                    .when_some(item.native_page, |view, module| {
                        view.child(
                            module_action(
                                SharedString::from(format!("module-open-{}", item.id)),
                                "打开",
                                false,
                                false,
                                cx,
                            )
                            .ml(surface::css(30.))
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(ModuleCatalogEvent::OpenModule(module));
                                },
                            )),
                        )
                    })
                    .when(item.native_page.is_none(), |view| {
                        view.child(
                            div()
                                .id(SharedString::from(format!(
                                    "module-source-{}",
                                    item.id
                                )))
                                .text_size(surface::css(14.))
                                .text_color(cx.theme().muted_foreground)
                                // 原版此盒的盒名与它聚焦的窗口来自当前 Dashboard；
                                // 本地没有该窗口时只显示事实，不假装已安装。
                                .child(format!(
                                    "安装状态未读取；原版盒 {} 打开 {}（{}）",
                                    item.box_name, item.window, item.url
                                )),
                        )
                        .child(
                            module_action(
                                SharedString::from(format!("module-install-{}", item.id)),
                                "安装",
                                true,
                                true,
                                cx,
                            )
                            .ml(surface::css(30.))
                            .tooltip(move |window, cx| {
                                Tooltip::new(format!(
                                    "此版本尚未接入模块安装服务；原版未安装时先打开 /installer/#type=module&id={}&location={}",
                                    item.id,
                                    item.url.trim_start_matches('/')
                                ))
                                .build(window, cx)
                            }),
                        )
                    }),
            )
            .when(expanded && item.image.is_some(), |view| {
                view.child(
                    h_flex()
                        .items_start()
                        .gap(surface::css(20.))
                        .min_h(surface::css(202.))
                        .p(surface::css(20.))
                        .bg(MainPageColors.detail_surface())
                        .when_some(item.image, |view, image| {
                            view.child(
                                img(image)
                                    .w(surface::css(288.))
                                    .h(surface::css(162.))
                                    .flex_shrink_0()
                                    .object_fit(ObjectFit::Contain),
                            )
                        })
                        .child(
                            v_flex()
                                .w(surface::css(592.))
                                .gap(surface::css(20.))
                                .child(div().whitespace_normal().child(item.description))
                                .when_some(item.link, |view, url| {
                                    view.child(source_link(
                                        "module-alexa-learn",
                                        "了解更多",
                                        url,
                                        cx,
                                    ))
                                })
                                .when(item.native_page.is_none(), |view| {
                                    view.child(surface::note(
                                        "安装包大小、版本和更新状态尚未读取。",
                                        cx,
                                    ))
                                }),
                        ),
                )
            })
            .into_any_element()
    }
}
impl Render for ModuleCatalog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("module-catalog")
            .test_support()
            .w_full()
            .mt(surface::css(30.))
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(24.))
                    .text_color(cx.theme().primary)
                    .mb(surface::css(10.))
                    .child("模块目录"),
            )
            .child(
                div()
                    .mb(surface::css(10.))
                    .child(surface::note("外部模块的安装与更新状态尚未读取。", cx)),
            )
            .children(MODULES.iter().map(|item| self.module_row(item, cx)))
            .when_some(self.details.clone(), |view, details| view.child(details))
    }
}

/// This is a local snapshot inspector, not the source installer's remove dialog.
/// Its shell follows the source's general `.modal-content` composition.
struct DeviceDetails {
    device: Device,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    owner: WeakEntity<ModuleCatalog>,
}
impl DeviceDetails {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(focus) = self.return_focus.take() {
            window.focus(&focus, cx);
        }
        let _ = self.owner.update(cx, |catalog, cx| {
            catalog.details = None;
            cx.notify();
        });
    }
}
impl Render for DeviceDetails {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let device = &self.device;
        let mut values = vec![
            ("设备型号", device.product_id.to_string()),
            ("序列号", device.serial_number.clone()),
            (
                "固件版本（快照）",
                device.firmware_info.current_fw_version.clone(),
            ),
            ("本地配置数量", device.profiles.len().to_string()),
        ];
        if let Some(version) = &device.firmware_info.current_dock_fw_version {
            values.push(("接收器版本（快照）", version.clone()));
        }
        let panel = v_flex()
            .id("module-device-details")
            .test_support()
            .relative()
            .occlude()
            .w((window.rem_size() * 25.)
                .min((window.viewport_size().width - window.rem_size() * (30. / 16.)).max(px(0.))))
            .max_h((window.viewport_size().height - window.rem_size() * (60. / 16.)).max(px(0.)))
            .px(surface::css(30.))
            .py(surface::css(20.))
            .bg(cx.theme().group_box)
            .border_1()
            .border_color(cx.theme().primary)
            .rounded(cx.theme().font_size * (5. / 16.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .child(
                div()
                    .text_size(surface::css(16.))
                    .font_weight(FontWeight::NORMAL)
                    .text_center()
                    .mb(surface::css(20.))
                    .child(device.display_name()),
            )
            .child(
                v_flex()
                    .id("device-details-scroll")
                    .min_h_0()
                    .scrollable_y()
                    .gap(surface::css(14.))
                    .children(values.into_iter().map(|(label, value)| {
                        h_flex()
                            .items_start()
                            .gap(surface::css(12.))
                            .child(div().w(surface::css(138.)).flex_shrink_0().child(label))
                            .child(div().flex_1().min_w_0().whitespace_normal().child(
                                if value.is_empty() {
                                    "未提供".into()
                                } else {
                                    value
                                },
                            ))
                    }))
                    .child(surface::note(
                        "未读取此设备的原生安装、连接或固件更新状态。",
                        cx,
                    )),
            )
            .child(
                h_flex()
                    .justify_center()
                    .flex_wrap()
                    .gap(surface::css(10.))
                    .mt(surface::css(20.))
                    .child(
                        Button::new("module-device-update")
                            .label("固件更新")
                            .w(surface::css(90.))
                            .h(surface::css(27.))
                            .text_size(surface::css(12.))
                            .tooltip("打开固件更新界面；设备服务尚未接入")
                            .on_click(cx.listener(|this, _, window, cx| {
                                let device = this.device.clone();
                                this.close(window, cx);
                                let _ = this.owner.update(cx, |_, cx| {
                                    cx.emit(ModuleCatalogEvent::FirmwareUpdate {
                                        device: Some(device),
                                        preview: false,
                                    });
                                });
                            })),
                    )
                    .child(
                        Button::new("module-device-remove")
                            .label("移除设备")
                            .w(surface::css(90.))
                            .h(surface::css(27.))
                            .text_size(surface::css(12.))
                            .disabled(true)
                            .tooltip("尚未读取原生安装及可移除状态"),
                    )
                    .child(
                        Button::new("module-device-close")
                            .label("关闭")
                            .w(surface::css(90.))
                            .h(surface::css(27.))
                            .text_size(surface::css(12.))
                            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                    ),
            )
            .child(
                Button::new("module-device-close-icon")
                    .ghost()
                    .absolute()
                    .top_0()
                    .right_0()
                    .size(surface::css(36.))
                    .p_0()
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(cx.theme().transparent)
                            .hover(MainPageColors.banner_heading().opacity(0.1))
                            .active(MainPageColors.banner_shade().opacity(0.3)),
                    )
                    .accessibility_label("关闭设备详情")
                    .child(img("synapse/calibration-close.svg").size(surface::css(20.)))
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
            );
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .size_full()
                    .bg(MainPageColors.banner_shade().opacity(0.7)),
            )
            .popup(panel)
    }
}

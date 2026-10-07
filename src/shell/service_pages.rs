//! Main frontend 9388/19388, IotPopupRoot/28256 and 6505/44442.
//! Browsing and navigation are local. Native discovery/install results are never
//! synthesized from clicks, timers, or the catalogue of supported products.
use super::app_picker::PickerModule;
use crate::features::module_service::{self as service, ModuleServiceSnapshot, Record};
use crate::{
    i18n,
    model::Device,
    ui::{surface, theme::MainPageColors},
};
use gpui_kit::base::{
    Button as BaseButton,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
    time::Duration,
};

#[path = "module_preview.rs"]
mod module_preview;

#[path = "gamer_room.rs"]
mod gamer_room;
pub(super) use gamer_room::{GamerRoomEvent, GamerRoomPage};

#[cfg(test)]
#[path = "service_button_tests.rs"]
mod button_tests;

#[cfg(test)]
#[path = "service_pages_tests.rs"]
mod page_tests;

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

struct Module {
    id: &'static str,
    box_name: &'static str,
    icon: &'static str,
    image: Option<&'static str>,
    link: Option<&'static str>,
    native_page: Option<ModulePage>,
}
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
// Local module destinations. Devices & Modules membership and localized text
// come from its own current 6505/44442 ne/te/ee table, not this broader list.
// No native installed state is inferred from a compiled local destination.
// 盒名、窗口名与地址来自当前 Dashboard：模块 54420 登记具名窗口，7861 的
// `showModules` maps each box to a named window (`focusTab(windowName)`). Source
// modules with `policy=3,tab_visible=1` are visible host tabs. Locally implemented
// pages open their matching tab directly; only unimplemented applications retain an
// installation state. Runtime box order comes from host group items. Evidence:
// docs/re/display-window-contract.md and docs/re/macro-ui-current.md.
const MODULES: &[Module] = &[
    Module {
        id: "alexa",
        box_name: "DASHBOARD_ALEXA",
        icon: "synapse/module-alexa.svg",
        image: Some("synapse/module-alexa.png"),
        link: Some("https://www.razer.com/chroma/alexa"),
        native_page: Some(ModulePage::Alexa),
    },
    Module {
        id: "macro",
        box_name: "MACRO",
        icon: "synapse/module-macro.svg",
        image: Some("synapse/module-macro.png"),
        link: None,
        // 原版此盒聚焦名为 `macro` 的窗口（`/synapse/macro/`）；本地已实现该窗口的
        // 外框与两个导航标签，宏服务与功能面板仍未接入。
        native_page: Some(ModulePage::Macro),
    },
    Module {
        id: "linked-games",
        box_name: "LINKED_GAMES",
        icon: "synapse/module-linked-games.svg",
        image: None,
        link: None,
        // 模块表：`linkedGames → windowName:"profiles", url:"/synapse/profiles/"`，
        // 打开参数 `policy=3,shouldFocus=1,tab_visible=1`（同窗口 + 聚焦 + 标签可见）。
        native_page: Some(ModulePage::Profiles),
    },
    Module {
        id: "feedback",
        box_name: "FEEDBACK",
        // 模块表原文：`/feedback/?app=synapse&path=${encodeURIComponent("/synapse/dashboard")}`。
        icon: "synapse/module-feedback.svg",
        image: None,
        link: None,
        native_page: Some(ModulePage::Feedback),
    },
    Module {
        id: "armory",
        box_name: "DASHBOARD_WORKSHOP",
        icon: "synapse/module-armory.svg",
        image: None,
        link: None,
        // 原版此盒打开 `armory` 窗口（`/synapse/armory/`）；本地已实现该窗口的
        // 外框与四个导航标签，资料分享服务仍未接入。
        native_page: Some(ModulePage::Armory),
    },
    Module {
        id: "profile-migration",
        box_name: "PROFILE_MIGRATION",
        icon: "synapse/module-profile-migration.svg",
        image: None,
        link: None,
        native_page: Some(ModulePage::Picker(PickerModule::ProfileMigration)),
    },
    Module {
        id: "tour",
        box_name: "TOUR",
        // 导览应用自己的图标（与 host_tabs.rs 里 Tour 标签页同一份资源）。
        icon: "synapse/tour-app-icon.svg",
        image: None,
        link: None,
        native_page: Some(ModulePage::IntroductionTour),
    },
];
include!("devices_modules_catalog.rs");
include!("module_service_rows.rs");
include!("module_service_remove.rs");

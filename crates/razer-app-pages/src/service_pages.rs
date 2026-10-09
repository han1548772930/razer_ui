//! Main frontend 9388/19388, IotPopupRoot/28256 and 6505/44442.
//! Browsing and navigation are local. Native discovery/install results are never
//! synthesized from clicks, timers, or the catalogue of supported products.
use super::app_picker::PickerModule;
use gpui_kit::base::{
    Button as BaseButton,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_pages::features::module_service as service;
use razer_pages::features::module_service::ModuleServiceSnapshot;
use razer_pages::features::module_service::Record;
use razer_widgets::surface;
use razer_widgets::theme::MainPageColors;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
    time::Duration,
};

#[path = "module_preview.rs"]
mod module_preview;

#[path = "gamer_room.rs"]
mod gamer_room;
pub use gamer_room::{GamerRoomEvent, GamerRoomPage};

#[cfg(test)]
#[path = "service_button_tests.rs"]
mod button_tests;

#[cfg(test)]
#[path = "service_pages_tests.rs"]
mod page_tests;

use razer_widgets::module_button::{ModuleAction, module_action, module_detail_action};

struct Module {
    id: &'static str,
    box_name: &'static str,
    icon: &'static str,
    image: Option<&'static str>,
    link: Option<&'static str>,
    native_page: Option<ModulePage>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulePage {
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

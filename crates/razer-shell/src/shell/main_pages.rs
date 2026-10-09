//! Host navigation bindings for independent dashboard presentation.
use super::{AppShell, Location};
use gpui_kit::{component::*, prelude::FluentBuilder as _, *};
use razer_dashboard::*;
use razer_i18n as i18n;
use razer_widgets::surface;
#[path = "main_pages/dashboard_cards.rs"]
mod dashboard_cards;
impl AppShell {
    pub(super) fn modules_page(&self, _: &mut Context<Self>) -> AnyElement {
        self.module_catalog.clone().into_any_element()
    }

    pub(super) fn gamer_room_page(&self, _: &mut Context<Self>) -> AnyElement {
        self.gamer_room.clone().into_any_element()
    }

    pub(super) fn shortcuts_page(&self, _: &mut Context<Self>) -> AnyElement {
        self.shortcuts.clone().into_any_element()
    }
}

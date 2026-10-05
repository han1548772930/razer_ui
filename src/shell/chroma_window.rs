//! Independent `policy=5` Chroma application window.
//!
//! The Dashboard contract names this window `chroma-app` and keeps it outside
//! the Synapse host tab strip. The page itself is shared with the local
//! Chroma Studio module view, while this wrapper supplies the OS-window
//! boundary and forwards settings navigation to the host shell.
use super::{
    AppShell, Location, Tab,
    chroma_page::{ChromaPage, ChromaPageEvent},
    display_window::{CHROMA_APP_WINDOW_ICON_PATH, DisplayMode},
};
use crate::features::ProductWorkspace;
use gpui_kit::*;

pub(super) struct ChromaWindow {
    page: Entity<ChromaPage>,
}

impl ChromaWindow {
    pub(super) fn new(
        owner: WeakEntity<AppShell>,
        devices: Vec<Entity<ProductWorkspace>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_window_title("Chroma");
        // gpui's portable WindowOptions accepts decoded image data rather
        // than the host's path flag; retain the audited path as the source
        // contract while the native title bar supplies the window chrome.
        let _icon_path = CHROMA_APP_WINDOW_ICON_PATH;
        let page = cx.new(|cx| ChromaPage::new(window, cx));
        page.update(cx, |page, cx| page.set_devices(devices, cx));
        cx.subscribe_in(
            &page,
            window,
            move |_, _, event: &ChromaPageEvent, _, cx| match event {
                ChromaPageEvent::OpenSettings => {
                    let _ = owner.update_in(cx, |shell, window, cx| {
                        shell.navigate(Location::Main(Tab::Setting), window, cx);
                    });
                }
                ChromaPageEvent::OpenTour(kind) => {
                    let _ = owner.update_in(cx, |shell, window, cx| {
                        shell.navigate(Location::Tour(*kind), window, cx);
                    });
                }
            },
        )
        .detach();
        Self { page }
    }
}

impl Render for ChromaWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(SharedString::from(format!(
                "display-mode-{}",
                DisplayMode::ChromaApp.key()
            )))
            .size_full()
            .child(self.page.clone())
    }
}

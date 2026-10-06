//! Independent `policy=5` Chroma application window.
//!
//! The Dashboard contract names this window `chroma-app` and keeps it outside
//! the Synapse host tab strip. Dashboard and Studio are separate roots in
//! the current host's Chroma subtab manager.
use super::{
    AppShell, Location, Tab,
    chroma_page::{ChromaPage, ChromaPageEvent},
    chroma_studio_window::StudioSession,
    display_window::{CHROMA_APP_WINDOW_ICON_PATH, DisplayMode},
    host_tabs,
};
use crate::features::ProductWorkspace;
use crate::ui::{surface::css, theme::HostColors};
use gpui_kit::{component::*, *};

pub(super) struct ChromaWindow {
    page: Entity<ChromaPage>,
    studio: Option<Entity<StudioSession>>,
    active_studio: bool,
    window: AnyWindowHandle,
    owner: WeakEntity<AppShell>,
    page_subscription: Option<Subscription>,
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
        let mut this = Self {
            page,
            studio: None,
            active_studio: false,
            window: window.window_handle(),
            owner,
            page_subscription: None,
        };
        this.bind_window(window, cx);
        this
    }
    pub(super) fn bind_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window = window.window_handle();
        let owner = self.owner.clone();
        // Host navigation uses the shell's main-window handle, never
        // subscribe_in's retained association (ensure_window uses or_insert).
        self.page_subscription = Some(cx.subscribe(
            &self.page,
            move |_, _, event: &ChromaPageEvent, cx| match event {
                ChromaPageEvent::OpenStudio => {
                    let owner = owner.clone();
                    cx.defer(move |cx| {
                        let _ = owner.update(cx, |shell, cx| shell.open_chroma_studio(cx));
                    });
                }
                ChromaPageEvent::OpenSettings => {
                    let Ok(handle) = owner.update(cx, |shell, _| shell.main_window) else {
                        return;
                    };
                    let owner = owner.clone();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            #[cfg(target_os = "windows")]
                            super::tray::native::show(window);
                            window.activate_window();
                            let _ = owner.update(cx, |shell, cx| {
                                shell.navigate(Location::Main(Tab::Setting), window, cx)
                            });
                        });
                    });
                }
                ChromaPageEvent::OpenTour(kind) => {
                    let Ok(handle) = owner.update(cx, |shell, _| shell.main_window) else {
                        return;
                    };
                    let owner = owner.clone();
                    let kind = *kind;
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            #[cfg(target_os = "windows")]
                            super::tray::native::show(window);
                            window.activate_window();
                            let _ = owner.update(cx, |shell, cx| {
                                shell.navigate(Location::Tour(kind), window, cx)
                            });
                        });
                    });
                }
            },
        ));
    }
    pub(super) fn open_studio(&mut self, studio: Entity<StudioSession>, cx: &mut Context<Self>) {
        self.studio = Some(studio.clone());
        self.active_studio = true;
        let handle = self.window;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                studio.update(cx, |studio, cx| studio.focus(window, cx))
            });
        });
        cx.notify();
    }
    fn tab(&self, studio: bool, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let label = if studio {
            crate::i18n::t("CHROMA_STUDIO")
        } else {
            "CHROMA".into()
        }
        .to_uppercase();
        let active = self.active_studio == studio;
        let id = if studio {
            "chroma-studio"
        } else {
            "chroma-app"
        };
        let mut frame = host_tabs::tab_frame(id.into(), active)
            .w(css(host_tabs::tab_width(&label, window)))
            .child(
                gpui_kit::base::Button::new(id)
                    .accessibility_label(label.clone())
                    .flex()
                    .items_center()
                    .size_full()
                    .rounded_t(css(5.))
                    .px(css(12.))
                    .py(css(6.))
                    .justify_start()
                    .text_size(css(12.))
                    .font_weight(FontWeight::LIGHT)
                    .bg(if active {
                        HostColors::surface()
                    } else {
                        HostColors::background()
                    })
                    .text_color(if active {
                        HostColors::active_text()
                    } else {
                        HostColors::inactive_text()
                    })
                    .focus_visible(|s| s.border_1().border_color(cx.theme().primary))
                    .child(
                        img(if studio {
                            "synapse/host-chroma-studio-favicon.svg"
                        } else {
                            "synapse/host-app-chroma.svg"
                        })
                        .size(css(20.))
                        .flex_shrink_0(),
                    )
                    .child(div().ml(css(10.)).text_ellipsis().child(label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.active_studio = studio;
                        if studio {
                            if let Some(studio) = this.studio.clone() {
                                studio.update(cx, |studio, cx| studio.focus(window, cx));
                            }
                        } else {
                            this.page.update(cx, |page, cx| page.focus(window, cx));
                        }
                        cx.notify();
                    })),
            );
        if studio {
            frame = frame.child(
                gpui_kit::base::Button::new("close-chroma-studio-tab")
                    .accessibility_label(crate::i18n::t("CLOSE"))
                    .absolute()
                    .right(css(5.))
                    .top(css(5.))
                    .size(css(24.))
                    .opacity(0.)
                    .group_hover(SharedString::from(id), |s| s.opacity(1.))
                    .focus_visible(|s| s.opacity(1.).border_1().border_color(cx.theme().primary))
                    .child(
                        img(if active {
                            "synapse/host-close_active_tab.svg"
                        } else {
                            "synapse/host-close-original.svg"
                        })
                        .size_full(),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.studio = None;
                        this.active_studio = false;
                        this.page.update(cx, |page, cx| page.focus(window, cx));
                        cx.notify();
                    })),
            );
        }
        frame.into_any_element()
    }
}

impl Render for ChromaWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = host_tabs::titlebar_frame(false)
            .child(div().w(css(8.)))
            .child(self.tab(false, window, cx));
        if self.studio.is_some() {
            bar = bar
                .child(div().w(css(4.)))
                .child(self.tab(true, window, cx));
        }
        bar = bar
            .child(div().flex_1())
            .child(
                host_tabs::window_button(
                    "chroma-minimize",
                    "synapse/host-minimize.svg",
                    "Minimize",
                )
                .on_click(|_, window, _| window.minimize_window()),
            )
            .child(
                host_tabs::maximize_button(window.is_maximized()).on_click(|_, window, _| {
                    let _ = host_tabs::host_window::toggle_maximize(window);
                }),
            )
            .child(
                host_tabs::window_button("chroma-close", "synapse/host-close.svg", "Close")
                    .on_click(|_, window, _| window.remove_window()),
            );
        let root = if self.active_studio {
            self.studio
                .as_ref()
                .map(|studio| studio.clone().into_any_element())
                .unwrap_or_else(|| self.page.clone().into_any_element())
        } else {
            self.page.clone().into_any_element()
        };
        v_flex()
            .id(SharedString::from(format!(
                "display-mode-{}",
                DisplayMode::ChromaApp.key()
            )))
            .size_full()
            .child(bar)
            .child(div().flex_1().min_h_0().child(root))
    }
}

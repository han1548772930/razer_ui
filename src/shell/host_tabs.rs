//! Electron TabUI: tab lifetime is separate from retained device configuration.
use super::{AppShell, Location, Tab};
use crate::ui::{surface, theme::HostColors};
use gpui_kit::base::{
    Button,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

actions!(
    host_tabs,
    [
        CloseTab,
        ReopenTab,
        NextTab,
        PreviousTab,
        MoveTabLeft,
        MoveTabRight
    ]
);

#[cfg(test)]
#[path = "host_tabs_tests.rs"]
mod tests;

#[path = "host_tab_drag.rs"]
mod drag;

#[path = "host_window.rs"]
mod host_window;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HostTab {
    Device(String),
    Tour(super::TourKind),
    Alexa,
    FirmwareUpdate,
}
impl HostTab {
    fn location(&self) -> Location {
        match self {
            Self::Device(key) => Location::Device(key.clone()),
            Self::Tour(kind) => Location::Tour(*kind),
            Self::Alexa => Location::Alexa,
            Self::FirmwareUpdate => Location::FirmwareUpdate,
        }
    }
    fn id(&self) -> SharedString {
        match self {
            Self::Device(key) => format!("host-{key}").into(),
            Self::Tour(kind) => kind.id().into(),
            Self::Alexa => "host-alexa".into(),
            Self::FirmwareUpdate => "host-firmware-update".into(),
        }
    }
    fn from_location(location: &Location) -> Option<Self> {
        match location {
            Location::Device(key) => Some(Self::Device(key.clone())),
            Location::Tour(kind) => Some(Self::Tour(*kind)),
            Location::Alexa => Some(Self::Alexa),
            Location::FirmwareUpdate => Some(Self::FirmwareUpdate),
            _ => None,
        }
    }
}

struct OpenTab {
    tab: HostTab,
    focus: FocusHandle,
    close_focus: FocusHandle,
}

pub(super) struct HostTabs {
    open: Vec<OpenTab>,
    closed: Vec<HostTab>,
    main_location: Location,
    main_focus: FocusHandle,
    pub(super) focus: FocusHandle,
    scroll: ScrollHandle,
    drag: Option<drag::TabDrag>,
    drag_subscription: Option<Subscription>,
    saved_order: Vec<String>,
    order_changed: bool,
}
impl HostTabs {
    pub(super) fn new(cx: &mut App) -> Self {
        cx.bind_keys([
            KeyBinding::new("ctrl-w", CloseTab, Some("AppShell")),
            KeyBinding::new("ctrl-shift-t", ReopenTab, Some("AppShell")),
            KeyBinding::new("ctrl-tab", NextTab, Some("AppShell")),
            KeyBinding::new("ctrl-shift-tab", PreviousTab, Some("AppShell")),
            KeyBinding::new("ctrl-shift-pageup", MoveTabLeft, Some("AppShell")),
            KeyBinding::new("ctrl-shift-pagedown", MoveTabRight, Some("AppShell")),
        ]);
        Self {
            open: vec![],
            closed: vec![],
            main_location: Location::Main(Tab::Home),
            main_focus: cx.focus_handle().tab_stop(true),
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            drag: None,
            drag_subscription: None,
            saved_order: vec![],
            order_changed: false,
        }
    }
    pub(super) fn open(&mut self, tab: HostTab, cx: &mut App) {
        self.closed.retain(|closed| *closed != tab);
        if !self.open.iter().any(|entry| entry.tab == tab) {
            self.open.push(OpenTab {
                tab,
                focus: cx.focus_handle().tab_stop(true),
                close_focus: cx.focus_handle().tab_stop(true),
            });
        }
    }
    pub(super) fn visit(&mut self, location: &Location, cx: &mut App) {
        if let Some(tab) = HostTab::from_location(location) {
            self.open(tab.clone(), cx);
            if let Some(ix) = self.open.iter().position(|entry| entry.tab == tab) {
                self.scroll.scroll_to_item(ix);
            }
        } else {
            self.main_location = location.clone();
        }
    }
    pub(super) fn focus_location(&self, location: &Location, window: &mut Window, cx: &mut App) {
        let focus = self
            .open
            .iter()
            .find(|entry| entry.tab.location() == *location)
            .map(|entry| &entry.focus)
            .unwrap_or(&self.main_focus);
        focus.focus(window, cx);
    }
    pub(super) fn restore_order(&mut self, order: &[String]) {
        self.open.sort_by_key(|entry| {
            order
                .iter()
                .position(|id| id == entry.tab.id().as_ref())
                .unwrap_or(usize::MAX)
        });
        self.saved_order = self.order();
        self.order_changed = false;
    }
    pub(super) fn order(&self) -> Vec<String> {
        self.open
            .iter()
            .map(|entry| entry.tab.id().to_string())
            .collect()
    }
    pub(super) fn order_pending(&self) -> bool {
        self.order_changed && self.order() != self.saved_order
    }
    pub(super) fn mark_order_saved(&mut self, order: Vec<String>) {
        if self.order() == order {
            self.order_changed = false;
        }
        self.saved_order = order;
    }
    pub(super) fn reveal_active(&self, location: &Location) {
        if let Some(ix) = self
            .open
            .iter()
            .position(|entry| entry.tab.location() == *location)
        {
            self.scroll.scroll_to_item(ix);
        }
    }
    fn remove(&mut self, tab: &HostTab, active: &Location) -> Option<Location> {
        let ix = self.open.iter().position(|entry| entry.tab == *tab)?;
        if self.drag.as_ref().is_some_and(|drag| drag.tab == *tab) {
            self.drag = None;
            self.drag_subscription = None;
        }
        self.open.remove(ix);
        self.closed.retain(|closed| closed != tab);
        self.closed.push(tab.clone());
        if *active != tab.location() {
            return None;
        }
        Some(
            self.open
                .get(ix)
                .or_else(|| self.open.last())
                .map(|entry| entry.tab.location())
                .unwrap_or_else(|| self.main_location.clone()),
        )
    }
}

impl AppShell {
    pub(super) fn close_host_tab(
        &mut self,
        tab: HostTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if tab == HostTab::FirmwareUpdate {
            if let Some((page, _)) = &self.firmware_update {
                if !page.update(cx, |page, cx| page.allow_close(window, cx)) {
                    self.navigate(Location::FirmwareUpdate, window, cx);
                    return;
                }
            }
        }
        let Some(entry) = self.host_tabs.open.iter().find(|entry| entry.tab == tab) else {
            return;
        };
        let restore_focus = self.location == tab.location()
            || entry.focus.is_focused(window)
            || entry.close_focus.is_focused(window);
        let next = self.host_tabs.remove(&tab, &self.location);
        if let HostTab::Device(key) = &tab {
            if let Some(device) = self
                .devices
                .iter()
                .find(|device| device.read(cx).identity() == *key)
            {
                device.update(cx, |device, cx| device.dismiss_profile_dialog(window, cx));
            }
        }
        // Closing hides a retained workspace. Drafts, profiles and shell exit/save
        // checks keep their owner; closing a host tab must never discard edits.
        if let Some(next) = next {
            self.navigate_now(next.clone(), None, window, cx);
        }
        if restore_focus {
            self.host_tabs.focus_location(&self.location, window, cx);
        }
        let closed = tab.location();
        self.history_index = self
            .history
            .iter()
            .take(self.history_index + 1)
            .filter(|location| **location != closed)
            .count()
            .saturating_sub(1);
        self.history.retain(|location| *location != closed);
        if let HostTab::Tour(kind) = tab {
            self.introduction_tours.remove(&kind);
        }
        if tab == HostTab::Alexa {
            self.alexa = None;
            self.alexa_subscription = None;
        }
        if tab == HostTab::FirmwareUpdate {
            self.firmware_update = None;
        }
        cx.notify();
    }
    pub(super) fn close_current_host_tab(
        &mut self,
        _: &CloseTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window
            .context_stack()
            .iter()
            .any(|context| context.contains("Dialog"))
        {
            return;
        }
        if let Some(tab) = HostTab::from_location(&self.location) {
            self.close_host_tab(tab, window, cx);
        }
    }
    pub(super) fn reopen_host_tab(
        &mut self,
        _: &ReopenTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window
            .context_stack()
            .iter()
            .any(|context| context.contains("Dialog"))
        {
            return;
        }
        // Consume only when navigation commits: Keep Editing in the Shortcuts
        // guard must not lose the last-closed tab.
        if let Some(tab) = self
            .host_tabs
            .closed
            .iter()
            .rev()
            .find(|tab| !self.host_tabs.open.iter().any(|entry| entry.tab == **tab))
            .cloned()
        {
            self.navigate(tab.location(), window, cx);
        }
    }
    fn cycle_host_tab(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if window
            .context_stack()
            .iter()
            .any(|context| context.contains("Dialog"))
        {
            return;
        }
        let current = self
            .host_tabs
            .open
            .iter()
            .position(|entry| entry.tab.location() == self.location)
            .map(|ix| ix + 1)
            .unwrap_or(0);
        let next =
            (current as isize + delta).rem_euclid(self.host_tabs.open.len() as isize + 1) as usize;
        let location = if next == 0 {
            self.host_tabs.main_location.clone()
        } else {
            self.host_tabs.open[next - 1].tab.location()
        };
        self.navigate(location, window, cx);
    }
    pub(super) fn next_host_tab(
        &mut self,
        _: &NextTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cycle_host_tab(1, window, cx);
    }
    pub(super) fn previous_host_tab(
        &mut self,
        _: &PreviousTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cycle_host_tab(-1, window, cx);
    }

    pub(super) fn title_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // App.B and shared module 57881 select the Chinese host title only for zh-CN.
        let main_label = if crate::i18n::locale().eq_ignore_ascii_case("zh-cn") {
            "雷云"
        } else {
            "SYNAPSE"
        };
        let main_width = tab_width(main_label, window);
        let mut left = 0.;
        let tabs = self
            .host_tabs
            .open
            .iter()
            .map(|entry| {
                let label = match &entry.tab {
                    HostTab::Device(key) => self
                        .devices
                        .iter()
                        .find(|device| device.read(cx).identity() == *key)
                        .map(|device| device.read(cx).device().display_name())
                        .unwrap_or_default(),
                    HostTab::Tour(kind) => kind.title(),
                    HostTab::Alexa => "Alexa".into(),
                    HostTab::FirmwareUpdate => "固件更新".into(),
                };
                let label = label.to_uppercase();
                let width = tab_width(&label, window);
                // TabUI removes the closed tab immediately, then transitions remaining
                // tabs' left coordinates for .2s ease. Do not fade or shrink the content.
                let dragging = self
                    .host_tabs
                    .drag
                    .as_ref()
                    .filter(|drag| drag.tab == entry.tab);
                let animated = motion::transition(
                    (entry.tab.id(), "left"),
                    dragging.map_or(left, |drag| drag.left),
                    Transition::new(if dragging.is_some() {
                        Duration::ZERO
                    } else {
                        Duration::from_millis(200)
                    })
                    .easing(Easing::Ease),
                    window,
                    cx,
                );
                let view = self
                    .host_tab(
                        Some(&entry.tab),
                        label,
                        &entry.focus,
                        Some(&entry.close_focus),
                        width > 186.,
                        cx,
                    )
                    .w(surface::css(width))
                    .left(surface::css(animated - left));
                left += width + 4.;
                if dragging.is_some() {
                    deferred(view).priority(20).into_any_element()
                } else {
                    view.into_any_element()
                }
            })
            .collect::<Vec<_>>();
        let viewport = f32::from(window.viewport_size().width) * 16. / f32::from(window.rem_size());
        // TabUI.calculateTabWrapperWidth reserves a window-size-dependent drag lane.
        let reserved = ((viewport + 1. - 600.) / 16.5 + 10.).floor().min(100.);
        let available = viewport - 144. - main_width - 10. - reserved;
        let content_width = (left - 4.).max(0.);
        let overflow = content_width > available;
        let max_offset = window.rem_size() * ((content_width - available + 40.).max(0.) / 16.);
        titlebar_frame(self.host_tabs.drag.is_some())
            .on_drag_move(cx.listener(Self::move_host_tab_drag))
            .child(
                self.host_tab(
                    None,
                    main_label.into(),
                    &self.host_tabs.main_focus,
                    None,
                    false,
                    cx,
                )
                .ml(surface::css(8.))
                .w(surface::css(main_width)),
            )
            .when(overflow, |bar| {
                bar.child(self.tab_scroll_button(false, max_offset, cx))
            })
            .child(
                h_flex()
                    .id("host-tabs-scroll")
                    .ml(surface::css(4.))
                    .gap(surface::css(4.))
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .items_end()
                    .overflow_x_scroll()
                    .track_scroll(&self.host_tabs.scroll)
                    .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
                    .children(tabs),
            )
            .when(overflow, |bar| {
                bar.child(self.tab_scroll_button(true, max_offset, cx))
            })
            .child(
                div()
                    .w(surface::css((reserved - 2.).max(0.)))
                    .h_full()
                    .flex_shrink_0(),
            )
            .child(
                window_button("window-minimize", "synapse/host-minimize.svg", "最小化")
                    .on_click(|_, window, _| window.minimize_window()),
            )
            .child(
                maximize_button(window.is_maximized()).on_click(|_, window, cx| {
                    if let Err(error) = host_window::toggle_maximize(window) {
                        eprintln!("无法切换窗口最大化状态: {error}");
                    }
                    cx.stop_propagation();
                }),
            )
            .child(
                window_button("window-close", "synapse/host-close.svg", "关闭窗口")
                    .on_click(cx.listener(|this, _, window, cx| this.request_exit(window, cx))),
            )
            .into_any_element()
    }

    fn host_tab(
        &self,
        tab: Option<&HostTab>,
        label: String,
        focus: &FocusHandle,
        close_focus: Option<&FocusHandle>,
        overflow: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let active = tab
            .map(|tab| tab.location() == self.location)
            .unwrap_or(matches!(
                self.location,
                Location::Main(_) | Location::Pairing
            ));
        let id: SharedString = tab.map(HostTab::id).unwrap_or_else(|| "host-main".into());
        let location = tab
            .map(HostTab::location)
            .unwrap_or_else(|| self.host_tabs.main_location.clone());
        let pointer_location = location.clone();
        let close = tab.cloned();
        let middle_close = close.clone();
        let group = id.clone();
        let icon = match tab {
            Some(HostTab::Tour(_)) => "synapse/tour-app-icon.svg",
            Some(HostTab::Alexa) => "synapse/module-alexa.svg",
            Some(HostTab::FirmwareUpdate) => "synapse/synapse.svg",
            Some(HostTab::Device(key)) => match self
                .devices
                .iter()
                .find(|device| device.read(cx).identity() == *key)
                .map(|device| device.read(cx).device().product_id)
            {
                Some(653) => "synapse/host-category-keyboard.svg",
                Some(777) => "synapse/host-category-audio.svg",
                Some(pid) if crate::product::audited_mouse_mat(pid).is_some() => {
                    "synapse/host-category-mousemat.svg"
                }
                _ => "synapse/host-category-mouse.svg",
            },
            None => "synapse/synapse.svg",
        };
        tab_frame(id.clone(), active)
            .when_some(
                tab.map(|_| self.host_tabs.scroll.clone()),
                |frame, scroll| {
                    // Win32 no-drag requires an occluding hitbox, which also blocks
                    // its ancestor's wheel listener. Forward to the same native
                    // scroll handle, retaining GPUI's axis choice and layout clamp.
                    frame.on_scroll_wheel(move |event, window, _| {
                        scroll_tabs(&scroll, event, window);
                    })
                },
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(move |this, _, window, cx| {
                    if let Some(tab) = &middle_close {
                        this.close_host_tab(tab.clone(), window, cx);
                    }
                    cx.stop_propagation();
                }),
            )
            .child(
                Button::new(id)
                    .track_focus(focus)
                    .accessibility_label(label.clone())
                    .selected(active)
                    .when_some(tab.cloned(), |button, tab| {
                        let owner = cx.entity().downgrade();
                        button.on_drag(drag::DragTab(tab), move |payload, offset, window, cx| {
                            let preview = cx.new(|_| drag::TabDragPreview);
                            let _ = owner.update(cx, |shell, cx| {
                                shell.start_host_tab_drag(
                                    payload.0.clone(),
                                    offset,
                                    &preview,
                                    window,
                                    cx,
                                );
                            });
                            preview
                        })
                    })
                    .size_full()
                    .rounded_t(surface::css(5.))
                    .px(surface::css(12.))
                    .py(surface::css(6.))
                    .justify_start()
                    .overflow_hidden()
                    .text_size(surface::css(12.))
                    .font_weight(FontWeight::LIGHT)
                    .bg(if active {
                        HostColors::surface()
                    } else {
                        HostColors::background()
                    })
                    .when(!active, |button| {
                        button.group_hover(group.clone(), |style| style.bg(tab_gradient(false)))
                    })
                    .active(|style| style.bg(HostColors::surface()))
                    .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
                    .text_color(if active {
                        HostColors::active_text()
                    } else {
                        HostColors::inactive_text()
                    })
                    .child(img(icon).size(surface::css(20.)).flex_shrink_0())
                    .child(
                        div()
                            .ml(surface::css(10.))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .child(label.clone()),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.navigate(pointer_location.clone(), window, cx);
                        }),
                    )
                    .on_click(cx.listener(move |this, event, window, cx| {
                        // TabUI activates on mousedown. Keep Base's Enter/Space
                        // and touch activation without repeating the mouse intent.
                        if !matches!(event, ClickEvent::Mouse(_)) {
                            this.navigate(location.clone(), window, cx);
                        }
                    })),
            )
            .when(overflow, |view| {
                let end = if active {
                    HostColors::surface()
                } else {
                    HostColors::background()
                };
                view.child(
                    div()
                        .absolute()
                        .right(surface::css(1.))
                        .top(surface::css(1.))
                        .w(surface::css(45.))
                        .h(surface::css(33.))
                        .rounded_tr(surface::css(5.))
                        .bg(linear_gradient(
                            90.,
                            linear_color_stop(HostColors::surface().opacity(0.), 0.),
                            linear_color_stop(end, 0.75),
                        ))
                        .when(!active, |mask| {
                            mask.group_hover(group.clone(), |style| style.opacity(0.))
                        }),
                )
            })
            .when_some(
                close.filter(|_| self.host_tabs.drag.is_none()),
                |view, tab| {
                    let close_group: SharedString = format!("{}-close", tab.id()).into();
                    view.child(
                        div()
                            .absolute()
                            .right(surface::css(1.))
                            .top(surface::css(1.))
                            .bottom_0()
                            .w(surface::css(30.))
                            .rounded_tr(surface::css(5.))
                            .bg(if active {
                                HostColors::surface().into()
                            } else {
                                tab_gradient(false)
                            })
                            .opacity(0.)
                            .group_hover(group.clone(), |style| style.opacity(1.)),
                    )
                    .child(
                        Button::new(close_group.clone())
                            .when_some(close_focus, |button, focus| button.track_focus(focus))
                            .group(close_group.clone())
                            .accessibility_label(format!("{} · {label}", crate::i18n::t("CLOSE")))
                            .absolute()
                            .right(surface::css(5.))
                            .top(surface::css(5.))
                            .size(surface::css(24.))
                            .opacity(0.)
                            .group_hover(group, |style| style.opacity(1.))
                            .focus_visible(|style| {
                                style
                                    .opacity(1.)
                                    .border_1()
                                    .border_color(cx.theme().primary)
                            })
                            .child(
                                img(if active {
                                    "synapse/host-close_active_tab.svg"
                                } else {
                                    "synapse/host-close-original.svg"
                                })
                                .size_full(),
                            )
                            .child(
                                img("synapse/host-close_active_tab_hover.svg")
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .opacity(0.)
                                    .group_hover(close_group.clone(), |style| style.opacity(1.)),
                            )
                            .child(
                                img("synapse/host-close_pressed.svg")
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .opacity(0.)
                                    .group_active(close_group, |style| style.opacity(1.)),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close_host_tab(tab.clone(), window, cx);
                            })),
                    )
                },
            )
    }

    fn tab_scroll_button(&self, right: bool, max_offset: Pixels, cx: &mut Context<Self>) -> Button {
        let scroll = self.host_tabs.scroll.clone();
        let disabled = if right {
            -scroll.offset().x >= max_offset
        } else {
            scroll.offset().x >= px(0.)
        };
        let id = if right {
            "host-tabs-right"
        } else {
            "host-tabs-left"
        };
        let (normal, hover, active, inactive) = if right {
            (
                "synapse/host-right-arrow.svg",
                "synapse/host-right-arrow-hover.svg",
                "synapse/host-right-arrow-active.svg",
                "synapse/host-right-arrow-disabled.svg",
            )
        } else {
            (
                "synapse/host-left-arrow.svg",
                "synapse/host-left-arrow-hover.svg",
                "synapse/host-left-arrow-active.svg",
                "synapse/host-left-arrow-disabled.svg",
            )
        };
        Button::new(id)
            .occlude()
            .group(id)
            .relative()
            .disabled(disabled)
            .accessibility_label(if right {
                "向右滚动页签"
            } else {
                "向左滚动页签"
            })
            .size(surface::css(20.))
            .mb(surface::css(10.))
            .flex_shrink_0()
            .child(img(if disabled { inactive } else { normal }).size_full())
            .when(!disabled, |button| {
                button
                    .child(
                        img(hover)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .opacity(0.)
                            .group_hover(id, |s| s.opacity(1.)),
                    )
                    .child(
                        img(active)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .opacity(0.)
                            .group_active(id, |s| s.opacity(1.)),
                    )
            })
            .on_click(cx.listener(move |_, _, _, cx| {
                let indices = 0..scroll.children_count();
                let candidate = if right {
                    indices.into_iter().find(|ix| {
                        scroll.bounds_for_item(*ix).is_some_and(|bounds| {
                            bounds.right() + scroll.offset().x > scroll.bounds().right()
                        })
                    })
                } else {
                    indices.into_iter().rev().find(|ix| {
                        scroll.bounds_for_item(*ix).is_some_and(|bounds| {
                            bounds.left() + scroll.offset().x < scroll.bounds().left()
                        })
                    })
                };
                if let Some(ix) = candidate {
                    scroll.scroll_to_item(ix);
                }
                cx.notify();
            }))
    }
}

fn tab_width(label: &str, window: &Window) -> f32 {
    let mut font = window.text_style().font();
    font.weight = FontWeight::LIGHT;
    let width = window
        .text_system()
        .shape_line(
            label.to_owned().into(),
            window.rem_size() * 0.75,
            &[TextRun {
                len: label.len(),
                font,
                color: HostColors::active_text(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        )
        .width;
    (f32::from(width) * 16. / f32::from(window.rem_size()) + 56.).clamp(90., 240.)
}

fn tab_gradient(active: bool) -> Background {
    linear_gradient(
        0.,
        linear_color_stop(HostColors::surface(), 0.2),
        linear_color_stop(
            if active {
                HostColors::accent()
            } else {
                HostColors::hover()
            },
            1.,
        ),
    )
    .into()
}

fn scroll_tabs(scroll: &ScrollHandle, event: &ScrollWheelEvent, window: &mut Window) {
    let delta = event.delta.pixel_delta(window.line_height());
    let x = if delta.x.is_zero() { delta.y } else { delta.x };
    scroll.set_offset(scroll.offset() + point(x, px(0.)));
    window.refresh();
}

fn titlebar_frame(dragging_tab: bool) -> Stateful<Div> {
    // Electron `.etabs-tabs` and the wrapper's ::after are native drag areas.
    // Include all empty space and the strip above the tabs. Interactive children
    // occlude this hitbox; stopping event bubbling alone does not affect Win32's
    // WM_NCHITTEST. Disable native movement during a tab reorder, as in TabUI.
    h_flex()
        .id("host-titlebar")
        .h(surface::css(42.))
        .flex_shrink_0()
        .bg(HostColors::background())
        .items_end()
        .when(!dragging_tab, |bar| {
            bar.window_control_area(WindowControlArea::Drag)
        })
}

fn tab_frame(id: SharedString, active: bool) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("{id}-frame")))
        .group(id)
        .occlude()
        .relative()
        // 20px icon + 6px vertical padding on each side + 1px outer padding.
        .h(surface::css(34.))
        .flex_shrink_0()
        .min_w(surface::css(90.))
        .max_w(surface::css(240.))
        .p(surface::css(1.))
        .rounded_t(surface::css(5.))
        .bg(if active {
            tab_gradient(true)
        } else {
            HostColors::background().into()
        })
        .when(!active, |view| {
            view.hover(|style| style.bg(tab_gradient(false)))
        })
}

fn maximize_button(maximized: bool) -> Button {
    window_button(
        "window-maximize",
        if maximized {
            "synapse/host-restore.svg"
        } else {
            "synapse/host-maximize.svg"
        },
        if maximized {
            "还原窗口"
        } else {
            "最大化窗口"
        },
    )
}

fn window_button(id: &'static str, asset: &'static str, label: &'static str) -> Button {
    // Electron index.css centers each background at its intrinsic size. Only
    // minimize/maximize include a 48x32 canvas; close/restore are tight glyphs.
    let (width, height) = match asset {
        "synapse/host-close.svg" => (12.7, 12.7),
        "synapse/host-restore.svg" => (13., 13.),
        _ => (48., 32.),
    };
    Button::new(id)
        .occlude()
        .accessibility_label(label)
        .w(surface::css(48.))
        .h(surface::css(42.))
        .flex_shrink_0()
        .hover(|style| style.bg(HostColors::surface()))
        .focus_visible(|style| style.bg(HostColors::surface()))
        .child(
            div()
                .id("window-control-image")
                .test_support()
                .w(surface::css(width))
                .h(surface::css(height))
                .flex_shrink_0()
                .child(img(asset).size_full().object_fit(ObjectFit::Contain)),
        )
}

//! Native 4130 MultiDevicePairing: retained request lifecycle and original cards.
//! No service is started on render, and no local operation invents a device.
#[path = "pairing_state.rs"]
mod state;

use crate::{i18n, resources, ui::surface};
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde_json::Value;
use state::{DeviceCard, Lane, PairingDevice, PairingRequest, PairingState, PairingStatus, Ticket};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

/// `master` contains the validated owner for external unpair requests, including
/// its deviceContainerId when supplied. An adapter must use a bounded worker.
/// This interface does not infer HID reports or reuse the
/// unrelated mapping-engine shortcut ABI.
pub(super) trait PairingTransport: Send + Sync {
    fn request(
        &self,
        master: Value,
        operation: &'static str,
        payload: Value,
        external: bool,
    ) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;
}

pub(super) enum PairingPageEvent {
    Back,
}

#[derive(Clone)]
enum Confirmation {
    Pair {
        device: PairingDevice,
        external: bool,
    },
    Unpair {
        device: PairingDevice,
        external: bool,
    },
    Continue713 {
        device: PairingDevice,
    },
}
impl Confirmation {
    fn device(&self) -> &PairingDevice {
        match self {
            Self::Pair { device, .. }
            | Self::Unpair { device, .. }
            | Self::Continue713 { device } => device,
        }
    }
}

pub(super) struct PairingPage {
    state: PairingState,
    transport: Option<Arc<dyn PairingTransport>>,
    request_task: Option<Task<()>>,
    watchdog: Option<Task<()>>,
    recovery: Option<Task<()>>,
    external_fallback: Option<Task<()>>,
    confirmation: Option<Confirmation>,
    confirm_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    hovered_badge: Option<String>,
}
impl EventEmitter<PairingPageEvent> for PairingPage {}

impl PairingPage {
    pub(super) fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            state: PairingState::default(),
            transport: None,
            request_task: None,
            watchdog: None,
            recovery: None,
            external_fallback: None,
            confirmation: None,
            confirm_focus: cx.focus_handle(),
            return_focus: None,
            hovered_badge: None,
        }
    }

    /// Only feed actual service metadata. Preview devices and local snapshots
    /// must not be presented as detected pairing candidates.
    pub(super) fn replace_masters(
        &mut self,
        masters: Value,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let previous = self.state.generation();
        self.state.replace_masters(masters)?;
        if self.state.generation() != previous {
            self.clear_tasks();
            self.confirmation = None;
        }
        cx.notify();
        Ok(())
    }

    pub(super) fn set_external_devices(
        &mut self,
        devices: Value,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.state.set_external(devices)?;
        cx.notify();
        Ok(())
    }

    pub(super) fn set_transport(
        &mut self,
        transport: Option<Arc<dyn PairingTransport>>,
        cx: &mut Context<Self>,
    ) {
        self.deactivate(cx);
        self.transport = transport;
        cx.notify();
    }

    /// Invalidate all request IDs before releasing tasks. A response from an old
    /// page/receiver cannot update a reopened pairing page.
    pub(super) fn deactivate(&mut self, cx: &mut Context<Self>) {
        if self.state.busy() {
            self.cancel_transport(cx);
        }
        self.state.invalidate();
        self.clear_tasks();
        self.confirmation = None;
        self.return_focus = None;
        self.hovered_badge = None;
        cx.notify();
    }

    fn clear_tasks(&mut self) {
        self.request_task.take();
        self.watchdog.take();
        self.recovery.take();
        self.external_fallback.take();
    }

    fn cancel_transport(&self, cx: &Context<Self>) {
        if let (Some(transport), Some((master, external))) =
            (self.transport.clone(), self.state.cancellation_context())
        {
            cx.background_executor()
                .spawn(async move {
                    let _ = transport
                        .request(master, "DUALLINK_CANCEL", serde_json::json!({}), external)
                        .await;
                })
                .detach();
        }
    }

    fn execute(&mut self, request: PairingRequest, window: &mut Window, cx: &mut Context<Self>) {
        let Some(transport) = self.transport.clone() else {
            self.state.set_error(match request {
                PairingRequest::Scan(_) => "无法扫描设备：无线配对服务尚未接通。",
                PairingRequest::ReadBindings => "无法读取已配对设备：无线配对服务尚未接通。",
                PairingRequest::Bind(_) => "无法配对设备：无线配对服务尚未接通。",
                PairingRequest::Unbind { .. } | PairingRequest::UnbindProduct(_) => {
                    "无法解除配对：无线配对服务尚未接通。"
                }
                PairingRequest::Cancel => "无法确认取消结果：无线配对服务尚未接通。",
            });
            cx.notify();
            return;
        };
        let Some(master) = self.state.context() else {
            self.state
                .set_error("尚未读取可用的无线配对设备，请先选择设备。");
            cx.notify();
            return;
        };
        if let PairingRequest::Bind(device) = &request {
            if device.requires_continuation() {
                self.show_confirmation(
                    Confirmation::Continue713 {
                        device: device.clone(),
                    },
                    window,
                    cx,
                );
                return;
            }
        }
        self.dispatch(request, master, transport, window, cx);
    }

    fn dispatch(
        &mut self,
        request: PairingRequest,
        master: Value,
        transport: Arc<dyn PairingTransport>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let master = match request.routing_context(master) {
            Ok(master) => master,
            Err(error) => {
                self.state.set_error(error);
                cx.notify();
                return;
            }
        };
        // External requests send the dongle ID, then the product ID either on
        // acknowledgement or after two seconds. The overall wait stays <= 10s.
        let timeout = if matches!(request, PairingRequest::UnbindProduct(_)) {
            8
        } else {
            10
        };
        let ticket = match self.state.begin(request) {
            Ok(ticket) => ticket,
            Err(error) => {
                self.state.set_error(error);
                cx.notify();
                return;
            }
        };
        self.clear_tasks();
        let operation = ticket.request().operation();
        let payload = ticket.request().payload(self.state.dual());
        let external = ticket.request().external();
        let response_ticket = ticket.clone();
        self.request_task = Some(cx.spawn_in(window, async move |page, cx| {
            let response = cx
                .background_executor()
                .spawn(async move {
                    transport
                        .request(master, operation, payload, external)
                        .await
                })
                .await;
            let _ = page.update_in(cx, |page, window, cx| {
                page.receive(&response_ticket, response, window, cx)
            });
        }));
        if ticket.request().fallback().is_some() {
            let fallback_ticket = ticket.clone();
            self.external_fallback = Some(cx.spawn_in(window, async move |page, cx| {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                let _ = page.update_in(cx, |page, window, cx| {
                    if let Some(next) = page.state.external_fallback(&fallback_ticket) {
                        page.execute(next, window, cx);
                    }
                });
            }));
        }
        self.watchdog = Some(cx.spawn_in(window, async move |page, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(timeout))
                .await;
            let _ = page.update_in(cx, |page, window, cx| {
                if page.state.current(&ticket) {
                    page.cancel_transport(cx);
                    page.request_task.take();
                    page.receive(
                        &ticket,
                        Err("等待配对服务响应超时，请检查设备连接后重试。".into()),
                        window,
                        cx,
                    );
                }
            });
        }));
        cx.notify();
    }

    fn receive(
        &mut self,
        ticket: &Ticket,
        response: Result<Value, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.state.current(ticket) {
            return;
        }
        self.watchdog.take();
        self.external_fallback.take();
        let next = self.state.receive(ticket, response);
        let lane = ticket.lane();
        let status = self.state.status(lane);
        if matches!(
            status,
            PairingStatus::BindError | PairingStatus::UnbindError
        ) {
            let generation = self.state.generation();
            self.recovery = Some(cx.spawn_in(window, async move |page, cx| {
                cx.background_executor().timer(Duration::from_secs(4)).await;
                let _ = page.update_in(cx, |page, _, cx| {
                    page.state.recover(generation, lane, status);
                    cx.notify();
                });
            }));
        }
        cx.notify();
        if let Some(next) = next {
            self.execute(next, window, cx);
        }
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_transport(cx);
        self.state.invalidate();
        self.clear_tasks();
        self.confirmation = None;
        self.execute(PairingRequest::Cancel, window, cx);
    }

    fn show_confirmation(
        &mut self,
        confirmation: Confirmation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.return_focus = window.focused(cx);
        self.state.select(confirmation.device().key());
        self.confirmation = Some(confirmation);
        window.focus(&self.confirm_focus, cx);
        cx.notify();
    }

    fn close_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirmation = None;
        if let Some(focus) = self.return_focus.take() {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(confirmation) = self.confirmation.clone() else {
            return;
        };
        self.close_confirmation(window, cx);
        match confirmation {
            Confirmation::Pair { device, external } => {
                let request = self.state.pair_request(device, external);
                self.execute(request, window, cx);
            }
            Confirmation::Unpair { device, external } => {
                self.execute(PairingRequest::Unbind { device, external }, window, cx)
            }
            Confirmation::Continue713 { device } => {
                if let (Some(master), Some(transport)) =
                    (self.state.context(), self.transport.clone())
                {
                    self.dispatch(PairingRequest::Bind(device), master, transport, window, cx);
                } else {
                    self.execute(PairingRequest::Bind(device), window, cx);
                }
            }
        }
    }

    fn header(&self, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .items_start()
            .gap(surface::css(60.))
            .p(surface::css(30.))
            .px(surface::css(40.))
            .mb(surface::css(20.))
            .rounded(surface::css(5.))
            .bg(cx.theme().button_primary_foreground.opacity(0.2))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(surface::css(16.))
                            .text_color(cx.theme().primary)
                            .mb(surface::css(12.))
                            .child(i18n::t("MULTI_DEVICE_PAIRING")),
                    )
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(21.))
                            .mb(surface::css(16.))
                            .whitespace_normal()
                            .child(i18n::t("PAIRING_UNPAIR_WARNING")),
                    )
                    .child(
                        link::Link::new("pairing-compatible-devices")
                            .href("https://www.razer.com/technology/razer-hyperspeed-wireless")
                            .child(
                                h_flex()
                                    .gap(surface::css(6.))
                                    .child(i18n::t("VIEW_COMPATIBLE_DEVICES"))
                                    .child(
                                        img("synapse/external-link.svg").size(surface::css(14.)),
                                    ),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .mb(surface::css(16.))
                            .child(i18n::t("PAIRING_CONDITIONS_TITLE")),
                    )
                    .children(
                        [
                            "PAIRING_CONDITION_HYPERSPEED_MODE",
                            "PAIRING_CONDITION_NO_USB",
                            "PAIRING_CONDITION_NO_DONGLE",
                            "PAIRING_CONDITION_PROXIMITY",
                            "PAIRING_CONDITION_KEEP_ACTIVE",
                        ]
                        .map(|key| {
                            h_flex()
                                .items_start()
                                .ml(surface::css(8.))
                                .gap(surface::css(8.))
                                .child("•")
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .line_height(surface::css(19.6))
                                        .whitespace_normal()
                                        .child(i18n::t(key)),
                                )
                        }),
                    ),
            )
            .into_any_element()
    }

    fn device_card(&self, card: DeviceCard, cx: &mut Context<Self>) -> AnyElement {
        let device = card.device().clone();
        let key = device.key().to_string();
        let paired = card.paired();
        let external = card.external();
        let status = card.status();
        let busy = matches!(
            status,
            PairingStatus::Binding
                | PairingStatus::CardUnbinding
                | PairingStatus::Unbinding
                | PairingStatus::Scanning
        );
        let hovered = self.hovered_badge.as_deref() == Some(key.as_str());
        let action_device = device.clone();
        let hover_key = key.clone();
        let selected = self.state.selected() == Some(key.as_str());
        let mut content = v_flex()
            .id(SharedString::from(format!("pairing-card-{key}")))
            .test_support()
            .relative()
            .w(surface::css(290.))
            .min_h(surface::css(220.))
            .p(surface::css(10.))
            .rounded(surface::css(5.))
            .border_1()
            .border_color(if selected {
                cx.theme().primary
            } else {
                cx.theme().transparent
            })
            .bg(cx.theme().button_primary_foreground.opacity(0.3));
        if busy {
            content = content.child(
                h_flex()
                    .h(surface::css(20.))
                    .gap(surface::css(6.))
                    .child(spinner::Spinner::new().small())
                    .child(i18n::t(status.key())),
            );
        } else {
            let foreground = if paired {
                if hovered {
                    cx.theme().warning
                } else {
                    cx.theme().primary
                }
            } else {
                cx.theme().muted_foreground
            };
            content = content.child(
                Button::new(SharedString::from(format!("pairing-action-{key}")))
                    .h(surface::css(20.))
                    .px(surface::css(5.))
                    .rounded_full()
                    .text_size(surface::css(12.))
                    .border_1()
                    .border_color(foreground)
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(cx.theme().transparent)
                            .foreground(foreground)
                            .hover(cx.theme().muted.opacity(0.1)),
                    )
                    .disabled(self.state.busy() || self.confirmation.is_some())
                    .accessibility_label(i18n::t(if paired { "UNPAIR" } else { "PAIR" }))
                    .when(paired, |button| {
                        button.child(
                            img(if hovered {
                                "synapse/pairing-unpair.svg"
                            } else {
                                "synapse/pairing-paired.svg"
                            })
                            .size(surface::css(16.)),
                        )
                    })
                    .child(i18n::t(if paired && !hovered {
                        "PAIRED"
                    } else if paired {
                        "UNPAIR"
                    } else {
                        "PAIR"
                    }))
                    .on_hover(cx.listener(move |page, hovered: &bool, _, cx| {
                        page.hovered_badge = hovered.then(|| hover_key.clone());
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |page, _, window, cx| {
                        page.show_confirmation(
                            if paired {
                                Confirmation::Unpair {
                                    device: action_device.clone(),
                                    external,
                                }
                            } else {
                                Confirmation::Pair {
                                    device: action_device.clone(),
                                    external,
                                }
                            },
                            window,
                            cx,
                        );
                    })),
            );
        }
        content = content
            .child(
                div()
                    .h(surface::css(140.))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(device.disconnected(), |element| element.opacity(0.45))
                    .child(product_image(&device, cx)),
            )
            .child(
                div()
                    .w_full()
                    .min_h(surface::css(50.))
                    .text_center()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(19.6))
                    .whitespace_normal()
                    .child(device.name(&i18n::locale())),
            )
            .when(external, |element| {
                element.child(surface::note("此设备来自其他接收器的已配对记录", cx))
            })
            .when(
                matches!(
                    status,
                    PairingStatus::BindError | PairingStatus::UnbindError
                ),
                |element| {
                    element.child(
                        div()
                            .text_color(cx.theme().warning)
                            .child(status_text(status)),
                    )
                },
            );
        if self
            .confirmation
            .as_ref()
            .is_some_and(|confirmation| confirmation.device().key() == key)
        {
            content = content.child(self.confirmation_view(cx));
        }
        content.into_any_element()
    }

    fn confirmation_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let confirmation = self
            .confirmation
            .as_ref()
            .expect("confirmation only renders while open");
        let unpair = matches!(confirmation, Confirmation::Unpair { .. });
        let continuation = matches!(confirmation, Confirmation::Continue713 { .. });
        let title = if unpair {
            "PAIRING_UNPAIR_DEVICE_TITLE"
        } else {
            "PAIRING_PAIR_DEVICE_TITLE"
        };
        let warnings = if unpair {
            vec![
                "PAIRING_UNPAIR_WARNING_MESSAGE",
                "PAIRING_UNPAIR_NEED_OTHER_DEVICE",
            ]
        } else {
            vec!["PAIRING_PAIR_NEW_DEVICE_WARNING"]
        };
        div()
            .id("pairing-confirmation")
            .test_support()
            .absolute()
            .inset_0()
            .rounded(surface::css(5.))
            .bg(cx.theme().popover)
            .flex()
            .items_start()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|page, _, window, cx| page.close_confirmation(window, cx)),
            )
            .child(
                v_flex()
                    .id("pairing-confirm-dialog")
                    .role(Role::Dialog)
                    .track_focus(&self.confirm_focus)
                    .w(surface::css(230.))
                    .p(surface::css(20.))
                    .min_h(surface::css(220.))
                    .border_1()
                    .border_color(cx.theme().warning)
                    .rounded(surface::css(5.))
                    .bg(cx.theme().background)
                    .text_center()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_key_down(cx.listener(|page, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            page.close_confirmation(window, cx);
                            cx.stop_propagation();
                        } else if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            page.confirm(window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .mb(surface::css(10.))
                            .child(i18n::t(title)),
                    )
                    .children(warnings.into_iter().map(|key| {
                        div()
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.4))
                            .whitespace_normal()
                            .mb(surface::css(10.))
                            .child(i18n::t(key))
                    }))
                    .when(continuation, |element| {
                        element.child(
                            div()
                                .text_size(surface::css(12.))
                                .whitespace_normal()
                                .child(i18n::t("CONTINUE_PAIRING")),
                        )
                    })
                    .child(
                        Button::new("pairing-confirm-continue")
                            .label(i18n::t("BUTTON_TEXT_CONTINUE"))
                            .h(surface::css(27.))
                            .w(surface::css(90.))
                            .mt(surface::css(10.))
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(cx.theme().warning)
                                    .foreground(cx.theme().button_primary_foreground),
                            )
                            .on_click(cx.listener(|page, _, window, cx| page.confirm(window, cx))),
                    )
                    .focus_trap("pairing-confirm-trap", &self.confirm_focus),
            )
            .into_any_element()
    }
}

impl Render for PairingPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cards = self.state.cards();
        let empty = cards.is_empty();
        let busy = self.state.busy();
        v_flex()
            .id("multi-device-pairing")
            .test_support()
            .w_full()
            .min_h_0()
            .pt(surface::css(10.))
            .px(surface::css(10.))
            .pb(surface::css(20.))
            .text_size(surface::css(14.))
            .text_color(cx.theme().foreground)
            .child(self.header(cx))
            .child(
                h_flex()
                    .gap(surface::css(12.))
                    .mb(surface::css(16.))
                    .flex_wrap()
                    .child(
                        Button::new("pairing-back")
                            .ghost()
                            .label("返回")
                            .on_click(cx.listener(|page, _, _, cx| {
                                page.deactivate(cx);
                                cx.emit(PairingPageEvent::Back);
                            })),
                    )
                    .child(
                        Button::new("pairing-refresh")
                            .label("读取已配对设备")
                            .disabled(busy)
                            .on_click(cx.listener(|page, _, window, cx| {
                                page.execute(PairingRequest::ReadBindings, window, cx)
                            })),
                    )
                    .child(
                        Button::new("pairing-scan-primary")
                            .label(if self.state.dual() {
                                "扫描键盘"
                            } else {
                                "扫描设备"
                            })
                            .primary()
                            .disabled(busy || self.confirmation.is_some())
                            .on_click(cx.listener(|page, _, window, cx| {
                                page.execute(PairingRequest::Scan(Lane::Primary), window, cx)
                            })),
                    )
                    .when(self.state.dual(), |element| {
                        element.child(
                            Button::new("pairing-scan-secondary")
                                .label("扫描鼠标")
                                .disabled(busy || self.confirmation.is_some())
                                .on_click(cx.listener(|page, _, window, cx| {
                                    page.execute(PairingRequest::Scan(Lane::Secondary), window, cx)
                                })),
                        )
                    })
                    .when(busy, |element| {
                        element.child(
                            Button::new("pairing-cancel")
                                .label(i18n::t("CANCEL"))
                                .on_click(
                                    cx.listener(|page, _, window, cx| page.cancel(window, cx)),
                                ),
                        )
                    }),
            )
            .when(!self.state.masters().is_empty(), |element| {
                element.child(
                    h_flex()
                        .gap(surface::css(10.))
                        .mb(surface::css(16.))
                        .flex_wrap()
                        .children(self.state.masters().iter().map(|device| {
                            let key = device.key().to_string();
                            Button::new(SharedString::from(format!("pairing-master-{key}")))
                                .label(device.name(&i18n::locale()))
                                .disabled(busy)
                                .selected(
                                    self.state
                                        .master()
                                        .is_some_and(|master| master.key() == key),
                                )
                                .on_click(cx.listener(move |page, _, window, cx| {
                                    page.clear_tasks();
                                    if let Err(error) = page.state.select_master(&key) {
                                        page.state.set_error(error);
                                    } else {
                                        page.execute(PairingRequest::ReadBindings, window, cx);
                                    }
                                    cx.notify();
                                }))
                        })),
                )
            })
            .when(self.state.master().is_none(), |element| {
                element.child(
                    div()
                        .id("pairing-receiver-status")
                        .test_support()
                        .mb(surface::css(12.))
                        .child("尚未读取可用于无线配对的设备。"),
                )
            })
            .when_some(self.state.error().map(str::to_string), |element, error| {
                element.child(
                    div()
                        .id("pairing-error")
                        .test_support()
                        .mb(surface::css(16.))
                        .text_color(cx.theme().warning)
                        .whitespace_normal()
                        .child(error),
                )
            })
            .child(
                div()
                    .mb(surface::css(10.))
                    .child(i18n::t("PAIRING_SELECT_DEVICE")),
            )
            .child(
                h_flex()
                    .items_start()
                    .flex_wrap()
                    .gap(surface::css(20.))
                    .children(cards.into_iter().map(|card| self.device_card(card, cx)))
                    .when(empty, |element| element.child(empty_card(busy, cx))),
            )
            .when(self.state.master().is_some(), |element| {
                element.child(
                    h_flex()
                        .mt(surface::css(20.))
                        .gap(surface::css(20.))
                        .child(
                            div()
                                .id("pairing-primary-status")
                                .test_support()
                                .child(status_text(self.state.status(Lane::Primary))),
                        )
                        .when(self.state.dual(), |element| {
                            element.child(
                                div()
                                    .id("pairing-secondary-status")
                                    .test_support()
                                    .child(status_text(self.state.status(Lane::Secondary))),
                            )
                        }),
                )
            })
    }
}

fn status_text(status: PairingStatus) -> String {
    i18n::t(status.key())
}

fn product_image(device: &PairingDevice, cx: &App) -> AnyElement {
    if let Some(asset) = resources::device_image(
        device.product_id(),
        device.edition_id(),
        1,
        resources::DeviceImage::Product,
    ) {
        return img(asset)
            .max_w_full()
            .max_h_full()
            .object_fit(ObjectFit::Contain)
            .into_any_element();
    }
    div()
        .size_full()
        .rounded(surface::css(4.))
        .bg(cx.theme().button_primary_foreground.opacity(0.4))
        .flex()
        .items_center()
        .justify_center()
        .child(
            img(if device.category() == "KEYBOARD" {
                "synapse/pairing-keyboard.svg"
            } else {
                "synapse/pairing-mouse.svg"
            })
            .size(surface::css(80.))
            .opacity(0.3),
        )
        .into_any_element()
}

fn empty_card(busy: bool, cx: &App) -> AnyElement {
    if busy {
        return v_flex()
            .id("pairing-loading")
            .test_support()
            .w(surface::css(290.))
            .h(surface::css(220.))
            .p(surface::css(20.))
            .rounded(surface::css(5.))
            .gap(surface::css(10.))
            .bg(cx.theme().button_primary_foreground.opacity(0.3))
            .child(
                div()
                    .w(surface::css(76.))
                    .h(surface::css(20.))
                    .rounded_full()
                    .bg(cx.theme().muted)
                    .text_size(surface::css(12.))
                    .text_center()
                    .child(i18n::t("LOADING")),
            )
            .child(
                skeleton::Skeleton::new()
                    .w(surface::css(248.))
                    .h(surface::css(99.)),
            )
            .child(
                skeleton::Skeleton::new()
                    .w(surface::css(248.))
                    .h(surface::css(16.)),
            )
            .child(
                skeleton::Skeleton::new()
                    .w(surface::css(248.))
                    .h(surface::css(16.)),
            )
            .into_any_element();
    }
    v_flex()
        .id("pairing-empty")
        .test_support()
        .w(surface::css(290.))
        .h(surface::css(220.))
        .p(surface::css(36.))
        .rounded(surface::css(5.))
        .border_2()
        .border_dashed()
        .border_color(cx.theme().muted_foreground.opacity(0.65))
        .justify_center()
        .items_center()
        .gap(surface::css(16.))
        .child(
            div()
                .text_center()
                .line_height(surface::css(17.))
                .whitespace_normal()
                .child(i18n::t("PAIRING_DEVICE_NOT_FOUND")),
        )
        .into_any_element()
}

#[cfg(test)]
#[path = "pairing_tests.rs"]
mod tests;

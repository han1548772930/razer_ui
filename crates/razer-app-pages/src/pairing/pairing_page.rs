//! Current standalone multipairing: actual `xs -> Ns -> Ds -> Ss -> cs` UI.
//! No service is started on render, and no local operation invents a device.
#[path = "pairing_state.rs"]
mod state;

use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_assets as resources;
use razer_i18n as i18n;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::surface;
use razer_widgets::theme::PairingColors;
use serde_json::Value;
use state::{DeviceCard, Lane, PairingDevice, PairingRequest, PairingState, PairingStatus, Ticket};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

pub struct ProductPairingTarget<'a> {
    product_id: u32,
    container_id: &'a str,
    device_info: &'a Value,
}

impl<'a> ProductPairingTarget<'a> {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }

    pub fn container_id(&self) -> &str {
        self.container_id
    }

    pub fn device_info(&self) -> &'a Value {
        self.device_info
    }
}

pub fn product_pairing_target(device_info: &Value) -> Option<ProductPairingTarget<'_>> {
    let product_id = u32::try_from(device_info.get("productId")?.as_u64()?)
        .ok()
        .filter(|product_id| *product_id > 0)?;
    let container_id = device_info
        .get("deviceContainerId")
        .or_else(|| device_info.get("containerId"))?
        .as_str()
        .filter(|value| !value.trim().is_empty())?;
    Some(ProductPairingTarget {
        product_id,
        container_id,
        device_info,
    })
}

/// `master` contains the validated owner for external unpair requests, including
/// its deviceContainerId when supplied. An adapter must use a bounded worker.
/// This interface does not infer HID reports or reuse the
/// unrelated mapping-engine shortcut ABI.
pub trait PairingTransport: Send + Sync {
    fn request(
        &self,
        master: Value,
        operation: &'static str,
        payload: Value,
        external: bool,
    ) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;
}

pub enum PairingPageEvent {
    Back,
    /// Dashboard 7861 `hi`：设备盒被按下时要打开该产品的
    /// `displayMode=multiDevicePairing` 窗口，负载是该设备记录本身。
    OpenProductWindow(Value),
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
}
impl Confirmation {
    fn device(&self) -> &PairingDevice {
        match self {
            Self::Pair { device, .. } | Self::Unpair { device, .. } => device,
        }
    }
}

pub struct PairingPage {
    state: PairingState,
    transport: Option<Arc<dyn PairingTransport>>,
    request_task: [Option<Task<()>>; 2],
    watchdog: [Option<Task<()>>; 2],
    recovery: [Option<Task<()>>; 2],
    external_fallback: [Option<Task<()>>; 2],
    external_deadline: [Option<Instant>; 2],
    confirmation: Option<Confirmation>,
    confirm_focus: FocusHandle,
    confirm_dialog_focus: FocusHandle,
    continuation_awaiting: bool,
    card_scope_focus: FocusHandle,
    hovered_badge: Option<String>,
    empty_settled: bool,
}
impl EventEmitter<PairingPageEvent> for PairingPage {}

impl PairingPage {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            state: PairingState::default(),
            transport: None,
            request_task: [None, None],
            watchdog: [None, None],
            recovery: [None, None],
            external_fallback: [None, None],
            external_deadline: [None, None],
            confirmation: None,
            confirm_focus: cx.focus_handle(),
            confirm_dialog_focus: cx.focus_handle(),
            continuation_awaiting: false,
            card_scope_focus: cx.focus_handle(),
            hovered_badge: None,
            empty_settled: false,
        }
    }

    /// Only feed actual service metadata. Preview devices and local snapshots
    /// must not be presented as detected pairing candidates.
    pub fn replace_masters(
        &mut self,
        masters: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let previous = self.state.generation();
        let cancellation = self
            .state
            .busy()
            .then(|| self.state.cancellation_context())
            .flatten();
        self.state.replace_masters(masters)?;
        if self.state.generation() != previous {
            if let Some((master, external)) = cancellation {
                self.cancel_context(master, external, cx);
            }
            self.clear_tasks();
            self.close_confirmation(window, cx);
            self.hovered_badge = None;
        }
        cx.notify();
        Ok(())
    }

    pub fn set_external_devices(
        &mut self,
        devices: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let previous = self.state.generation();
        let cancellation = self
            .state
            .busy()
            .then(|| self.state.cancellation_context())
            .flatten();
        self.state.set_external(devices)?;
        if self.state.generation() != previous {
            if let Some((master, external)) = cancellation {
                self.cancel_context(master, external, cx);
            }
            self.clear_tasks();
        }
        if self.confirmation.as_ref().is_some_and(|confirmation| {
            !self
                .state
                .cards()
                .iter()
                .any(|card| card.device() == confirmation.device())
        }) {
            self.close_confirmation(window, cx);
        }
        cx.notify();
        Ok(())
    }

    /// 产品配对窗口传入的 `allMasters`（180 `BG` 的 `multiDevicePairingInit`
    /// 负载）。只有真实的 DUALLINK 记录才应传入；解析失败时沿用页面原有错误提示，
    /// 不会把非法负载当成已配对设备。
    pub fn apply_all_masters(
        &mut self,
        masters: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.set_external_devices(masters, window, cx) {
            self.state.set_error(error);
            cx.notify();
        }
    }

    /// Consume the real product root's `multiDevicePairingInit.deviceInfo`.
    /// Without an observed transport the original initial `loading` remains.
    pub fn apply_device_info(
        &mut self,
        device_info: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.state.busy() {
            self.cancel_transport(cx);
        }
        self.clear_tasks();
        self.close_confirmation(window, cx);
        self.state.initialize_device_info(device_info)?;
        self.empty_settled = false;
        self.activate(window, cx);
        cx.notify();
        Ok(())
    }

    pub fn set_transport(
        &mut self,
        transport: Option<Arc<dyn PairingTransport>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_confirmation(window, cx);
        self.deactivate(cx);
        self.transport = transport;
        self.activate(window, cx);
        cx.notify();
    }

    pub fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.await_bindings();
        if self.state.master().is_some() && self.transport.is_some() {
            if self.state.busy() {
                self.cancel_transport(cx);
            }
            self.state.invalidate();
            self.clear_tasks();
            self.execute(PairingRequest::ReadBindings, window, cx);
        }
    }

    /// Invalidate all request IDs before releasing tasks. A response from an old
    /// page/receiver cannot update a reopened pairing page.
    pub fn deactivate(&mut self, cx: &mut Context<Self>) {
        if self.state.busy() {
            self.cancel_transport(cx);
        }
        self.state.invalidate();
        self.state.await_bindings();
        self.clear_tasks();
        self.confirmation = None;
        self.hovered_badge = None;
        cx.notify();
    }

    fn clear_tasks(&mut self) {
        self.continuation_awaiting = false;
        self.external_deadline = [None, None];
        self.clear_request_tasks();
        for task in &mut self.recovery {
            task.take();
        }
    }

    fn clear_request_tasks(&mut self) {
        for lane in [Lane::Primary, Lane::Secondary] {
            self.clear_lane_tasks(lane);
        }
    }

    fn clear_lane_tasks(&mut self, lane: Lane) {
        self.request_task[lane.index()].take();
        self.watchdog[lane.index()].take();
        self.external_fallback[lane.index()].take();
    }

    fn cancel_transport(&self, cx: &Context<Self>) {
        if let Some((master, external)) = self.state.cancellation_context() {
            self.cancel_context(master, external, cx);
        }
    }

    fn cancel_context(&self, master: Value, external: bool, cx: &Context<Self>) {
        if let Some(transport) = self.transport.clone() {
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
                PairingRequest::Unbind { .. }
                | PairingRequest::UnbindProduct(_)
                | PairingRequest::ReclaimExternal(_) => "无法解除配对：无线配对服务尚未接通。",
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
                // Current standalone `Ns` only raises continuePairing state.
                // The actual bootstrap mounts no DualLinkWarning component.
                // Keep Binding pending until a real continuation observation.
                match self.state.begin(request) {
                    Ok(_) => self.continuation_awaiting = true,
                    Err(error) => self.state.set_error(error),
                }
                cx.notify();
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
        let ticket = match self.state.begin(request) {
            Ok(ticket) => ticket,
            Err(error) => {
                self.state.set_error(error);
                cx.notify();
                return;
            }
        };
        self.clear_lane_tasks(ticket.lane());
        let deadline = match ticket.request() {
            PairingRequest::Unbind { external: true, .. } => {
                let deadline = Instant::now() + Duration::from_secs(10);
                self.external_deadline[ticket.lane().index()] = Some(deadline);
                Some(deadline)
            }
            PairingRequest::UnbindProduct(_) => self.external_deadline[ticket.lane().index()],
            _ => {
                self.external_deadline[ticket.lane().index()] = None;
                None
            }
        };
        // The original scan handler calls `Ce` before submitting discovery.
        // It cancels recovery regardless of which category is scanned.
        if matches!(ticket.request(), PairingRequest::Scan(_)) {
            for recovery in &mut self.recovery {
                recovery.take();
            }
        }
        let operation = ticket.request().operation();
        let payload = ticket.request().payload(self.state.dual());
        let external = ticket.request().external();
        let response_ticket = ticket.clone();
        self.request_task[ticket.lane().index()] =
            Some(cx.spawn_in(window, async move |page, cx| {
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
        if ticket.request().fallback().is_some()
            || matches!(ticket.request(), PairingRequest::ReclaimExternal(_))
        {
            let fallback_ticket = ticket.clone();
            self.external_fallback[ticket.lane().index()] =
                Some(cx.spawn_in(window, async move |page, cx| {
                    cx.background_executor().timer(Duration::from_secs(2)).await;
                    let _ = page.update_in(cx, |page, window, cx| {
                        if let Some(next) = page.state.external_fallback(&fallback_ticket) {
                            page.execute(next, window, cx);
                        }
                    });
                }));
        }
        if let Some(deadline) = deadline {
            // Only `Ds.ee` owns a ten-second cleanup. Ordinary `Ns` requests
            // have no UI watchdog and never send an invented timeout CANCEL.
            let lane_index = ticket.lane().index();
            self.watchdog[lane_index] = Some(cx.spawn_in(window, async move |page, cx| {
                cx.background_executor()
                    .timer(deadline.saturating_duration_since(Instant::now()))
                    .await;
                let _ = page.update_in(cx, |page, _, cx| {
                    if page.state.current(&ticket) {
                        page.clear_lane_tasks(ticket.lane());
                        page.state.expire_external(&ticket);
                        page.external_deadline[ticket.lane().index()] = None;
                        cx.notify();
                    }
                });
            }));
        }
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
        let next = self.state.receive(ticket, response);
        if self.state.current(ticket) {
            // Source ignores unrelated external responses, missing bind
            // devices and reclaim errors while keeping its listener/timer.
            cx.notify();
            return;
        }
        self.watchdog[ticket.lane().index()].take();
        self.external_fallback[ticket.lane().index()].take();
        let recovery = self.state.recovery_request();
        if self.state.clears_recovery() {
            for recovery in &mut self.recovery {
                recovery.take();
            }
        }
        if let Some(next) = next {
            // `Ns` invokes `Is` (and scan's `Ce`) before scheduling the
            // binding-error timeout. Preserve that ordering across categories.
            self.execute(next, window, cx);
        }
        if let Some((lane, status)) = recovery.filter(|_| !self.state.clears_recovery()) {
            let generation = self.state.generation();
            self.recovery[lane.index()] = Some(cx.spawn_in(window, async move |page, cx| {
                cx.background_executor().timer(Duration::from_secs(4)).await;
                let _ = page.update_in(cx, |page, _, cx| {
                    page.state.recover(generation, lane, status);
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Current `Ns` sends CANCEL and immediately refreshes BIND_INFO. CANCEL
        // has no response handler, so waiting for an acknowledgement strands it.
        self.cancel_transport(cx);
        self.state.invalidate();
        self.clear_tasks();
        self.close_confirmation(window, cx);
        self.execute(PairingRequest::ReadBindings, window, cx);
    }

    pub fn observe_continue_pairing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.continuation_awaiting {
            return;
        }
        if let (Some(master), Some(transport)) = (self.state.context(), self.transport.clone()) {
            if let Some(request) = self.state.continue_713() {
                self.continuation_awaiting = false;
                self.dispatch(request, master, transport, window, cx);
            }
        }
    }

    pub fn observe_cancel_pairing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel(window, cx);
    }

    fn show_confirmation(
        &mut self,
        confirmation: Confirmation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .state
            .lane_busy(self.state.lane_for(confirmation.device()))
            || !self
                .state
                .cards()
                .iter()
                .any(|card| card.device() == confirmation.device() && card.actionable())
        {
            self.state
                .set_error("所选设备已不可操作，请等待当前操作完成或重新扫描。");
            cx.notify();
            return;
        }
        self.confirmation = Some(confirmation);
        window.focus(&self.confirm_focus, cx);
        cx.notify();
    }

    fn close_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirmation = None;
        // Original `ss` unmounts the focused button without focusing its
        // trigger. Remove the orphaned native focus, preserving that behavior.
        if self.confirm_focus.is_focused(window) {
            window.blur(cx);
        }
        cx.notify();
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(confirmation) = self.confirmation.clone() else {
            return;
        };
        self.close_confirmation(window, cx);
        if self
            .state
            .lane_busy(self.state.lane_for(confirmation.device()))
            || !self
                .state
                .cards()
                .iter()
                .any(|card| card.device() == confirmation.device() && card.actionable())
        {
            self.state
                .set_error("设备信息已变化，请重新选择并确认配对操作。");
            cx.notify();
            return;
        }
        match confirmation {
            Confirmation::Pair { device, external } => {
                match self.state.pair_request(device, external) {
                    Ok(request) => self.execute(request, window, cx),
                    Err(error) => {
                        self.state.set_error(error);
                        cx.notify();
                    }
                }
            }
            Confirmation::Unpair { device, external } => {
                self.execute(PairingRequest::Unbind { device, external }, window, cx)
            }
        }
    }

    fn header(&self, _cx: &Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .items_start()
            .gap(surface::css(60.))
            .p(surface::css(30.))
            .px(surface::css(40.))
            .mb(surface::css(20.))
            .rounded(surface::css(5.))
            .bg(PairingColors.header_surface())
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(surface::css(16.))
                            .text_color(PairingColors.paired())
                            .mb(surface::css(12.))
                            .font_weight(FontWeight::NORMAL)
                            .child(i18n::t("MULTI_DEVICE_PAIRING").to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(21.))
                            .text_color(PairingColors.body_text())
                            .mb(surface::css(16.))
                            .whitespace_normal()
                            .child(i18n::t("PAIRING_UNPAIR_WARNING")),
                    )
                    .child(
                        div()
                            .id("pairing-compatible-devices")
                            .cursor_pointer()
                            .text_color(PairingColors.body_text())
                            .hover(|style| style.text_color(PairingColors.paired()))
                            .on_click(|_, _, cx| {
                                cx.open_url(
                                    "https://www.razer.com/technology/razer-hyperspeed-wireless",
                                );
                            })
                            .child(
                                h_flex()
                                    .gap(surface::css(6.))
                                    .child(
                                        div().underline().child(i18n::t("VIEW_COMPATIBLE_DEVICES")),
                                    )
                                    .child(
                                        img("synapse/pairing-compatible.svg")
                                            .size(surface::css(14.)),
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
        let paired = card.paired()
            || (card.status() == PairingStatus::UnbindError
                && !card.external()
                && !device.disconnected());
        let actionable = card.actionable();
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
        let mut content = v_flex()
            .id(SharedString::from(format!("pairing-card-{key}")))
            .test_support()
            .relative()
            .w(surface::css(290.))
            .min_h(surface::css(220.))
            .p(surface::css(10.))
            .rounded(surface::css(5.))
            .bg(PairingColors.card_surface());
        if !busy && actionable {
            let foreground = if paired {
                if hovered {
                    PairingColors.unpair()
                } else {
                    PairingColors.paired()
                }
            } else {
                PairingColors.unpaired()
            };
            content = content.child(
                Button::new(SharedString::from(format!("pairing-action-{key}")))
                    .xsmall()
                    .absolute()
                    .left(surface::css(10.))
                    .top(surface::css(10.))
                    .h(surface::css(20.))
                    .py(surface::css(2.))
                    .px(surface::css(8.))
                    .when(paired, |button| {
                        button
                            .min_w(surface::css(66.))
                            .pl(surface::css(2.))
                            .pr(surface::css(5.))
                    })
                    .rounded_full()
                    .text_size(surface::css(12.))
                    .font_semibold()
                    .border_1()
                    .border_color(foreground)
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(cx.theme().transparent)
                            .foreground(foreground)
                            .hover(cx.theme().muted.opacity(0.1)),
                    )
                    .disabled(!actionable || self.confirmation.is_some())
                    .accessibility_label(i18n::t(if paired { "UNPAIR" } else { "PAIR" }))
                    .child(
                        h_flex()
                            .gap(surface::css(4.))
                            .text_size(surface::css(12.))
                            .when(paired, |row| {
                                row.child(
                                    img(if hovered {
                                        "synapse/pairing-unpair.svg"
                                    } else {
                                        "synapse/pairing-paired.svg"
                                    })
                                    .size(surface::css(16.)),
                                )
                            })
                            .child(sentence_case(&i18n::t(if paired && !hovered {
                                "PAIRED"
                            } else if paired {
                                "UNPAIR"
                            } else {
                                "PAIR"
                            }))),
                    )
                    .on_hover(cx.listener(move |page, hovered: &bool, _, cx| {
                        page.hovered_badge = (actionable && *hovered).then(|| hover_key.clone());
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
                    .relative()
                    .h(surface::css(140.))
                    .min_h(surface::css(140.))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(device.dimmed(), |element| element.opacity(0.45))
                    .child(product_image(&device, cx))
                    .when(busy, |image| {
                        let unbinding = matches!(
                            status,
                            PairingStatus::CardUnbinding | PairingStatus::Unbinding
                        );
                        let color = if unbinding {
                            PairingColors.unpair()
                        } else {
                            PairingColors.paired()
                        };
                        image.child(
                            h_flex()
                                .absolute()
                                .top_0()
                                .left_0()
                                .h(surface::css(20.))
                                .p(surface::css(2.))
                                .min_w(surface::css(if unbinding { 92. } else { 76. }))
                                .border_1()
                                .border_color(color)
                                .rounded_full()
                                .gap(surface::css(6.))
                                .text_size(surface::css(12.))
                                .font_semibold()
                                .text_color(color)
                                .child(spinner::Spinner::new().small().color(color))
                                .child(sentence_case(&i18n::t(if unbinding {
                                    "UNPAIRING"
                                } else {
                                    "PAIRING"
                                }))),
                        )
                    }),
            )
            .child(
                div()
                    .w_full()
                    .min_h(surface::css(50.))
                    .text_center()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(19.6))
                    .text_color(PairingColors.device_name())
                    .mb(surface::css(2.))
                    .whitespace_normal()
                    .when(device.dimmed(), |element| element.opacity(0.45))
                    .child(device.name(&i18n::locale()).to_uppercase()),
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
            .occlude()
            .rounded(surface::css(5.))
            .bg(PairingColors.inline_backdrop())
            .flex()
            .items_start()
            .justify_center()
            .on_click(cx.listener(|page, _, window, cx| page.close_confirmation(window, cx)))
            .child(
                v_flex()
                    .id("pairing-confirm-dialog")
                    .test_support()
                    .role(Role::Dialog)
                    .w(surface::css(230.))
                    .p(surface::css(20.))
                    .border_1()
                    .border_color(PairingColors.unpair())
                    .rounded(surface::css(5.))
                    .bg(PairingColors.dialog_surface())
                    .justify_between()
                    .text_center()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .on_key_down(cx.listener(|page, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            page.close_confirmation(window, cx);
                            cx.stop_propagation();
                        } else if event.keystroke.key == "tab" {
                            window.focus(&page.confirm_focus, cx);
                            cx.stop_propagation();
                        } else if !page.confirm_focus.is_focused(window)
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            page.confirm(window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .text_color(PairingColors.body_text())
                            .mb(surface::css(10.))
                            .child(i18n::t(title).to_uppercase()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap(surface::css(10.))
                            .text_size(surface::css(12.))
                            .text_color(PairingColors.unpaired())
                            .line_height(surface::css(14.4))
                            .children(
                                warnings
                                    .into_iter()
                                    .map(|key| div().whitespace_normal().child(i18n::t(key))),
                            ),
                    )
                    .child(
                        h_flex().justify_center().child(
                            BaseButton::new("pairing-confirm-continue")
                                .track_focus(&self.confirm_focus)
                                .accessibility_label(i18n::t("BUTTON_TEXT_CONTINUE").to_uppercase())
                                .child(i18n::t("BUTTON_TEXT_CONTINUE").to_uppercase())
                                .text_size(surface::css(12.))
                                .relative()
                                .h(surface::css(27.))
                                .w(surface::css(90.))
                                .p_0()
                                .rounded(cx.theme().font_size * (3. / 16.))
                                .mt(surface::css(10.))
                                .bg(PairingColors.unpair())
                                .text_color(PairingColors.button_text())
                                .hover(|style| {
                                    style
                                        .bg(PairingColors.button_hover())
                                        .top(surface::css(-1.))
                                })
                                .active(|style| style.top_0())
                                .on_click(
                                    cx.listener(|page, _, window, cx| page.confirm(window, cx)),
                                ),
                        ),
                    )
                    .focus_trap("pairing-confirm-trap", &self.confirm_dialog_focus),
            )
            .into_any_element()
    }
}

impl Render for PairingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.state.status(Lane::Primary) == PairingStatus::Initializing {
            return div()
                .id("pairing-initial-loading")
                .test_support()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child("loading")
                .into_any_element();
        }
        let cards = self.state.cards();
        let empty = cards.is_empty();
        let scanning = self.state.status(Lane::Primary) == PairingStatus::Scanning
            || self.state.status(Lane::Secondary) == PairingStatus::Scanning;
        let settled = [Lane::Primary, Lane::Secondary].iter().any(|lane| {
            matches!(
                self.state.status(*lane),
                PairingStatus::Ready
                    | PairingStatus::ScanResults
                    | PairingStatus::Bound
                    | PairingStatus::Unbound
                    | PairingStatus::BindError
                    | PairingStatus::UnbindError
            )
        });
        // `Ss.P.current`: the first settled empty render retains the skeleton.
        let skeleton = empty && (scanning || !settled || !self.empty_settled);
        if empty && !scanning && settled {
            self.empty_settled = true;
        }
        v_flex()
            .id("multi-device-pairing")
            .test_support()
            .w_full()
            .min_h_0()
            .pt(surface::css(10.))
            .px(surface::css(10.))
            .pb(surface::css(20.))
            .text_size(surface::css(14.))
            .text_color(PairingColors.body_text())
            .child(self.header(cx))
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
                    .when(empty, |element| element.child(empty_card(skeleton, cx)))
                    .focus_trap("pairing-card-actions", &self.card_scope_focus),
            )
            .into_any_element()
    }
}

fn sentence_case(label: &str) -> String {
    let lower = label.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn product_image(device: &PairingDevice, cx: &App) -> AnyElement {
    if let Some(asset) = resources::pairing_product_image(
        device.product_id(),
        device.edition_id(),
        if device.category() == "KEYBOARD" { 1 } else { 0 },
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
            .p(surface::css(10.))
            .rounded(surface::css(5.))
            .items_center()
            .bg(PairingColors.card_surface())
            .child(
                div().w_full().mb(surface::css(16.)).child(
                    div()
                        .w(surface::css(76.))
                        .h(surface::css(20.))
                        .rounded_full()
                        .border_1()
                        .border_color(cx.theme().muted_foreground)
                        .text_color(cx.theme().muted_foreground)
                        .py(surface::css(2.))
                        .px(surface::css(8.))
                        .text_size(surface::css(12.))
                        .font_semibold()
                        .line_height(surface::css(14.))
                        .child(format!("{}...", i18n::t("LOADING"))),
                ),
            )
            .child(
                skeleton::Skeleton::new()
                    .w(surface::css(248.))
                    .h(surface::css(99.))
                    .rounded(surface::css(3.))
                    .bg(cx.theme().button_foreground.opacity(0.03))
                    .mb(surface::css(16.)),
            )
            .child(
                skeleton::Skeleton::new()
                    .w(surface::css(248.))
                    .h(surface::css(16.))
                    .rounded(surface::css(3.))
                    .bg(cx.theme().button_foreground.opacity(0.03))
                    .mb(surface::css(8.)),
            )
            .child(
                skeleton::Skeleton::new()
                    .w(surface::css(248.))
                    .h(surface::css(16.))
                    .rounded(surface::css(3.))
                    .bg(cx.theme().button_foreground.opacity(0.03))
                    .mb(surface::css(8.)),
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
        .border_color(PairingColors.empty_border())
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

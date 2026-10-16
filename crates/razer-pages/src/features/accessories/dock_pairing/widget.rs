//! PID756 current1874/useOn. The mounted widget owns reads and its legacy modal.
//! Cache master classification is source presentation, never a hardware binding.
use super::*;
use serde_json::{Value, json};
use std::time::Duration;

pub struct DualLinkWidget {
    device: Device,
    device_context: context::DeviceContext,
    state: PairingState,
    active: bool,
    bluetooth: bool,
    blocked_by_receiver: bool,
    master_is_dual: bool,
    paired_cache: Value,
    query_sessions: BTreeMap<String, u64>,
    mount_task: Option<Task<()>>,
    modal: Option<Entity<DockDialog>>,
    modal_subscriptions: Vec<Subscription>,
}
impl EventEmitter<DockPairingEvent> for DualLinkWidget {}

impl DualLinkWidget {
    pub fn new(device: &Device) -> Self {
        Self {
            device: device.clone(),
            device_context: Default::default(),
            state: Default::default(),
            active: false,
            bluetooth: false,
            blocked_by_receiver: false,
            master_is_dual: false,
            paired_cache: Value::Null,
            query_sessions: Default::default(),
            mount_task: None,
            modal: None,
            modal_subscriptions: Vec::new(),
        }
    }
    pub fn set_bluetooth(&mut self, bluetooth: bool, cx: &mut Context<Self>) {
        if self.bluetooth != bluetooth {
            self.bluetooth = bluetooth;
            cx.notify();
        }
    }
    pub fn set_pairing_cache(&mut self, records: Value, cx: &mut Context<Self>) {
        if self.paired_cache != records {
            self.paired_cache = records;
            self.refresh_master_classification();
            cx.notify();
        }
    }
    fn refresh_master_classification(&mut self) {
        // MW ge reads q's logical productId and its cached master; it does not
        // claim the record is a live binding. UI Oe._5 reads that master's
        // source DeviceInfo.canPairTwoDevices from the installed local catalog.
        let rows: Vec<_> = match &self.paired_cache {
            Value::Object(rows) => rows.values().collect(),
            Value::Array(rows) => rows.iter().collect(),
            _ => Vec::new(),
        };
        self.master_is_dual = rows
            .iter()
            .find(|row| row["productId"].as_u64() == Some(u64::from(self.device.product_id)))
            .and_then(|row| row["master"]["productId"].as_u64())
            .and_then(|id| u32::try_from(id).ok())
            .is_some_and(razer_device::receiver_pairing::master_can_pair_two_devices);
    }
    pub fn set_device_context(&mut self, devices: &[Device], cx: &mut Context<Self>) {
        let context = context::DeviceContext::from_devices(devices);
        let mut changed = self.device_context != context;
        if let Some(device) = devices.iter().find(|device| {
            device.product_id == self.device.product_id
                && device.device_container_id == self.device.device_container_id
                && device.serial_number == self.device.serial_number
        }) {
            changed |= self.device.real_product_id != device.real_product_id
                || self.device.dashboard.connection_observation
                    != device.dashboard.connection_observation;
            self.device = device.clone();
        }
        // useOn's o only matches valid164/179 with the current product's
        // container; a persisted identity alone never enters validDevices.
        self.blocked_by_receiver = devices.iter().any(|device| {
            matches!(device.product_id, 164 | 179)
                && context
                    .valid_devices()
                    .iter()
                    .any(|(id, edition)| *id == device.product_id && *edition == device.edition_id)
                && !self.device.device_container_id.is_empty()
                && device.device_container_id == self.device.device_container_id
        });
        self.device_context = context;
        if changed {
            self.begin_read(cx);
            cx.notify();
        }
    }
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active == active {
            return;
        }
        self.active = active;
        self.mount_task = None;
        if active {
            self.begin_read(cx);
            self.mount_task = Some(cx.spawn(async move |owner, cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let _ = owner.update(cx, |this, cx| {
                    if this.active {
                        for kind in ["DUALLINK_CONNECT_TYPE", "DUALLINK_DEVICE_INFO"] {
                            let event = DockPairingEvent::request(kind, json!({}));
                            this.query_sessions.insert(kind.into(), event.session());
                            cx.emit(event);
                        }
                    }
                });
            }));
        } else {
            self.query_sessions.clear();
            if let Some(modal) = self.modal.take() {
                let handle = modal.read(cx).window_handle();
                self.modal_subscriptions.clear();
                cx.spawn(async move |owner, cx| {
                    let _ = handle.update(cx, |_, window, cx| {
                        let event = modal.update(cx, |modal, cx| modal.finish_close(window, cx));
                        if let Some(event) = event {
                            let _ = owner.update(cx, |_, cx| cx.emit(event));
                        }
                    });
                })
                .detach();
            }
            if self.state.active.is_some() {
                cx.emit(self.state.cancel());
            }
        }
        cx.notify();
    }
    fn begin_read(&mut self, cx: &mut Context<Self>) {
        if self.active && self.modal.is_none() {
            cx.emit(self.state.request("DUALLINK_BIND_INFO", json!({})));
        }
    }
    fn disabled(&self) -> bool {
        self.bluetooth
            || self.blocked_by_receiver
            || !multi::is_dongle(&self.device)
            || self.master_is_dual
    }
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(modal) = self.modal.take() {
            let cancel = modal.update(cx, |modal, cx| modal.finish_close(window, cx));
            self.modal_subscriptions.clear();
            if let Some(event) = cancel {
                cx.emit(event);
            }
        }
        self.modal_subscriptions.clear();
        cx.notify();
    }
    pub fn observe_pairing(
        &mut self,
        observation: DockPairingObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if matches!(
            observation.kind(),
            "DUALLINK_CONNECT_TYPE" | "DUALLINK_DEVICE_INFO"
        ) {
            if self.query_sessions.get(observation.kind()) != Some(&observation.session()) {
                return false;
            }
            let kind = observation.kind().to_owned();
            self.query_sessions.remove(&kind);
            if let Some(Ok(payload)) = observation.into_result() {
                if kind == "DUALLINK_DEVICE_INFO" {
                    self.master_is_dual = payload["master"]["productId"]
                        .as_u64()
                        .and_then(|id| u32::try_from(id).ok())
                        .is_some_and(razer_device::receiver_pairing::master_can_pair_two_devices);
                }
            }
            cx.notify();
            return true;
        }
        if let Some(modal) = &self.modal {
            let state = modal.update(cx, |modal, cx| {
                modal
                    .observe_pairing(observation.clone(), cx)
                    .then(|| modal.state.clone())
            });
            if let Some(mut state) = state {
                state.active = None;
                state.pending = None;
                state.recovery = None;
                state.success_close = None;
                self.state = state;
                cx.notify();
                return true;
            }
        }
        let (changed, event, _) = self.state.observe(observation, false);
        if changed {
            if let Some(event) = event {
                cx.emit(event);
            }
            cx.notify();
        }
        changed
    }
    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled() || !self.active {
            return;
        }
        self.dismiss(window, cx);
        if self.state.active.is_some() {
            cx.emit(self.state.cancel());
        }
        let modal = DockDialog::open(
            spec(self.device.product_id),
            self.device.edition_id,
            self.device.layout_id,
            self.device
                .product_name
                .get(&i18n::locale().to_lowercase())
                .to_owned(),
            self.state.clone(),
            false,
            window,
            cx,
        );
        modal.update(cx, |modal, cx| {
            modal.set_owner(&self.device, cx);
        });
        self.modal_subscriptions = vec![
            cx.subscribe(&modal, |_, _, event: &DockPairingEvent, cx| {
                cx.emit(event.clone())
            }),
            cx.subscribe(&modal, |this, _, _: &dialog::DockDialogClosed, cx| {
                this.modal = None;
                this.modal_subscriptions.clear();
                this.begin_read(cx);
                cx.notify();
            }),
        ];
        modal.update(cx, |modal, cx| modal.begin_read(cx));
        self.modal = Some(modal);
        cx.notify();
    }
}
impl Render for DualLinkWidget {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let spec = spec(self.device.product_id);
        let disabled = self.disabled();
        let panel = surface::panel_with_control(
            spec.text("HYPER_SPEED_MULTI_DEVICE_PAIRING"),
            surface::asset_button(
                "keyboard-duallink-tips",
                "synapse/help-default.svg",
                spec.text("HYPER_SPEED_DUALINK_PROPERTIES_TOOLTIP"),
                cx,
            ),
            cx,
        )
        .child(
            h_flex()
                .items_center()
                .gap(surface::css(20.))
                .child(img(spec.asset("icon-multideviceparing")).size(surface::css(44.)))
                .child(
                    gpui_kit::base::Button::new("keyboard-duallink-open")
                        .disabled(disabled)
                        .accessibility_label(spec.text("OPEN_PAIRING_UTILITY"))
                        .h(surface::css(44.))
                        .text_size(surface::css(14.))
                        .underline()
                        .text_color(cx.theme().foreground)
                        .when(disabled, |button| button.opacity(0.45))
                        .when(!disabled, |button| {
                            button.hover(|style| style.text_color(cx.theme().primary))
                        })
                        .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
                        .child(spec.text("OPEN_PAIRING_UTILITY"))
                        .on_click(cx.listener(|this, _, window, cx| this.open(window, cx))),
                ),
        )
        .when(disabled, |panel| {
            panel.child(
                div()
                    .mt(surface::css(20.))
                    .text_size(surface::css(14.))
                    .text_color(cx.theme().foreground)
                    .child(spec.text("ENABLE_LAUNCH_PARING_UTILITY_INFO")),
            )
        });
        div().child(panel).children(self.modal.clone())
    }
}

//! PID 207's actually mounted legacy km -> qM/wm/ym/Ym/Vm/Bm/dm graph.
//! All transport results enter through observe_hyperflux_pairing_response.
use super::*;
use std::time::Duration;

#[path = "hyperflux_207_pairing_ui.rs"]
mod ui;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Status {
    #[default]
    Loading,
    Loaded,
    Scanning,
    Scanned,
    Pairing,
    Paired,
    PairFailed,
    Unpairing,
    JustUnpair,
    Unpaired,
    UnpairFailed,
    JustPaired,
}
#[derive(Default)]
pub(super) struct Pairing {
    open: bool,
    status: [Status; 2],
    bind_info: Vec<Value>,
    scanned_info: Vec<Value>,
    selected_ix: usize,
    warning: bool,
    failure_reset: Option<Task<()>>,
}
impl Pairing {
    fn scans(&self, slot: usize) -> Vec<&Value> {
        self.scanned_info
            .iter()
            .filter(|d| d["category"] == category(slot))
            .collect()
    }
    fn can_modify(&self, slot: usize) -> bool {
        let other = 1 - slot;
        !matches!(
            self.status[other],
            Status::Pairing | Status::Scanning | Status::JustUnpair | Status::Scanned
        ) || self.status[other] == Status::Scanned && self.scans(other).is_empty()
    }
    fn clear_if_idle(&mut self) {
        // wm invokes vm only when BOTH category states are Loaded or Paired.
        if self
            .status
            .iter()
            .all(|s| matches!(s, Status::Loaded | Status::Paired))
        {
            self.failure_reset = None;
        }
    }
}
fn category(slot: usize) -> &'static str {
    if slot == 0 { "KEYBOARD" } else { "MOUSE" }
}
fn tick() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
impl SourceControls {
    pub(super) fn pairing_refresh_bind_info(&self, cx: &mut Context<Self>) {
        self.pairing_message("DUALLINK_BIND_INFO", serde_json::json!({}), cx);
    }
    pub(super) fn mount_legacy_pairing_queries(&mut self, cx: &mut Context<Self>) {
        if self.hyperflux.feature {
            return;
        }
        // km has both the validDevices effect and the independent mount effect.
        self.pairing_refresh_bind_info(cx);
        self.pairing_refresh_bind_info(cx);
        let generation = self.hyperflux.generation;
        self.hyperflux.pairing_queries = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let _ = this.update(cx, |this, cx| {
                if this.hyperflux.generation != generation
                    || !this.hyperflux.mounted
                    || this.hyperflux.feature
                {
                    return;
                }
                this.pairing_message("DUALLINK_CONNECT_TYPE", serde_json::json!({}), cx);
                this.pairing_message("DUALLINK_DEVICE_INFO", serde_json::json!({}), cx);
            });
        }));
    }
    fn pairing_message(&self, kind: &str, payload: Value, cx: &mut Context<Self>) {
        self.hyperflux_request(
            HyperfluxOperation::SourceMessage(
                serde_json::json!({"type":kind,"payload":payload,"timerTick":tick()}),
            ),
            cx,
        );
    }
    pub(super) fn open_hyperflux_pairing(&mut self, cx: &mut Context<Self>) {
        if self.hyperflux.feature {
            self.hyperflux_request(
                HyperfluxOperation::OpenPairing {
                    new_utility: true,
                    serial_number: self.hyperflux.serial.clone(),
                },
                cx,
            );
            return;
        }
        self.hyperflux.pairing = Pairing {
            open: true,
            bind_info: self.hyperflux.paired_info.clone(),
            ..Default::default()
        };
        self.pairing_message("DUALLINK_BIND_INFO", serde_json::json!({}), cx);
        cx.notify();
    }
    fn close_pairing(&mut self, cancel_transport: bool, cx: &mut Context<Self>) {
        self.hyperflux.pairing.failure_reset = None;
        self.hyperflux.pairing.open = false;
        self.hyperflux.pairing.warning = false;
        if cancel_transport {
            self.pairing_message("DUALLINK_CANCEL", serde_json::json!({}), cx);
        }
        cx.notify();
    }
    fn scan_pairing(&mut self, slot: usize, cx: &mut Context<Self>) {
        if !self.hyperflux.pairing.can_modify(slot) {
            return;
        }
        self.hyperflux.pairing.failure_reset = None;
        self.hyperflux.pairing.status[slot] = Status::Scanning;
        self.pairing_message(
            "DUALLINK_SCAN_DEVICE",
            serde_json::json!({"status":1,"category":category(slot)}),
            cx,
        );
        cx.notify();
    }
    fn bind_pairing(
        &mut self,
        slot: usize,
        warning_on_713: bool,
        device: Value,
        cx: &mut Context<Self>,
    ) {
        self.hyperflux.pairing.status[slot] = Status::Pairing;
        if warning_on_713 && device["dongleId"] == 713 {
            self.hyperflux.pairing.warning = true;
        } else {
            self.hyperflux.pairing.warning = false;
            self.pairing_message(
                "DUALLINK_BIND_DEVICE",
                serde_json::json!({"mode":1,"device":device}),
                cx,
            );
        }
        cx.notify();
    }
    fn bind_selected_pairing(&mut self, slot: usize, cx: &mut Context<Self>) {
        // ym uses the UNFILTERED scanedInfo[selectedIndex], also for status2.
        // Preserve the source's cross-category indexing instead of correcting it.
        if let Some(device) = self
            .hyperflux
            .pairing
            .scanned_info
            .get(self.hyperflux.pairing.selected_ix)
            .cloned()
        {
            self.bind_pairing(slot, true, device, cx);
        }
    }
    fn confirm_pairing_warning(&mut self, keep_pairing: bool, cx: &mut Context<Self>) {
        self.hyperflux.pairing.warning = false;
        if keep_pairing {
            if let Some(device) = self
                .hyperflux
                .pairing
                .scanned_info
                .iter()
                .find(|d| d["dongleId"] == 713)
                .cloned()
            {
                self.pairing_message(
                    "DUALLINK_BIND_DEVICE",
                    serde_json::json!({"mode":1,"device":device}),
                    cx,
                );
            }
        } else {
            self.pairing_message("DUALLINK_CANCEL", serde_json::json!({}), cx);
            self.pairing_message("DUALLINK_BIND_INFO", serde_json::json!({}), cx);
            self.close_pairing(false, cx);
        }
        cx.notify();
    }
    fn unbind_pairing(&mut self, slot: usize, device: &Value, cx: &mut Context<Self>) {
        let Some(id) = device
            .get("dongleId")
            .filter(|id| truthy(id))
            .or_else(|| device.get("productId"))
            .cloned()
        else {
            return;
        };
        for device in &mut self.hyperflux.pairing.bind_info {
            if device["productId"] == id {
                device["connected"] = Value::from(0);
            }
        }
        self.hyperflux.pairing.status[slot] = Status::JustUnpair;
        self.pairing_message(
            "DUALLINK_UNBIND_DEVICE",
            serde_json::json!({"productId":id,"category":category(slot)}),
            cx,
        );
        cx.notify();
    }
    fn reset_pairing_failure(&mut self, slot: Option<usize>, cx: &mut Context<Self>) {
        let generation = self.hyperflux.generation;
        self.hyperflux.pairing.failure_reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.hyperflux.generation != generation || !this.hyperflux.pairing.open {
                    return;
                }
                if let Some(slot) = slot {
                    this.hyperflux.pairing.status[slot] = Status::Paired;
                } else {
                    this.hyperflux.pairing.status = [Status::Loaded; 2];
                    this.hyperflux.pairing.bind_info.clear();
                    this.hyperflux.pairing.scanned_info.clear();
                }
                cx.notify();
            });
        }));
    }
    /// Actual MW_RESPONSE_UI.msgData; requests never manufacture this response.
    pub fn observe_hyperflux_pairing_response(
        &mut self,
        message: &Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 207 || !self.hyperflux.mounted {
            return false;
        }
        let Some(kind) = message["type"].as_str() else {
            return false;
        };
        if !matches!(
            kind,
            "DUALLINK_BIND_INFO"
                | "DUALLINK_SCAN_DEVICE"
                | "DUALLINK_BIND_DEVICE"
                | "DUALLINK_UNBIND_DEVICE"
        ) {
            return false;
        }
        let payload = &message["payload"];
        let failed = truthy(&message["error"]);
        // km independently keeps the Redux pairedInfo current; wm is mounted
        // only while its dialog is open. Failed native results are not imported.
        if !failed {
            match kind {
                "DUALLINK_BIND_INFO" => {
                    let Some(devices) = payload.as_array() else {
                        return false;
                    };
                    self.hyperflux.paired_info = devices.clone();
                }
                "DUALLINK_BIND_DEVICE" => {
                    let device = &payload["device"];
                    if !device.is_object() {
                        return false;
                    }
                    if !self
                        .hyperflux
                        .paired_info
                        .iter()
                        .any(|d| d["dongleId"] == device["dongleId"])
                    {
                        self.hyperflux
                            .paired_info
                            .retain(|d| d["category"] != device["category"]);
                        self.hyperflux.paired_info.push(device.clone());
                    }
                }
                "DUALLINK_UNBIND_DEVICE" => self
                    .hyperflux
                    .paired_info
                    .retain(|d| d["dongleId"] != payload["productId"]),
                _ => {}
            }
        }
        if !self.hyperflux.pairing.open {
            cx.notify();
            return true;
        }
        let states = self.hyperflux.pairing.status;
        if failed {
            match kind {
                "DUALLINK_BIND_INFO" => {
                    self.hyperflux.pairing.status = [Status::Loaded; 2];
                    self.hyperflux.pairing.bind_info.clear();
                    self.hyperflux.pairing.scanned_info.clear();
                }
                "DUALLINK_SCAN_DEVICE" => {
                    if let Some(slot) = states.iter().position(|s| *s == Status::Scanning) {
                        self.hyperflux.pairing.status[slot] = Status::Scanned;
                        self.hyperflux.pairing.scanned_info.clear();
                    }
                }
                "DUALLINK_BIND_DEVICE" => {
                    if let Some(slot) = states.iter().position(|s| *s == Status::Pairing) {
                        self.hyperflux.pairing.status[slot] = Status::PairFailed;
                    }
                    self.reset_pairing_failure(None, cx);
                }
                "DUALLINK_UNBIND_DEVICE" => {
                    if let Some(slot) = states.iter().position(|s| *s == Status::JustUnpair) {
                        self.hyperflux.pairing.status[slot] = Status::UnpairFailed;
                        // gm restores connected only for payload.status, not status2.
                        if slot == 0 {
                            for d in &mut self.hyperflux.pairing.bind_info {
                                d["connected"] = Value::from(2);
                            }
                        }
                        self.reset_pairing_failure(Some(slot), cx);
                    }
                }
                _ => {}
            }
        } else {
            match kind {
                "DUALLINK_BIND_INFO" => {
                    let devices = payload.as_array().unwrap().clone();
                    self.hyperflux.pairing.status = std::array::from_fn(|slot| {
                        if devices.iter().any(|d| d["category"] == category(slot)) {
                            Status::Paired
                        } else {
                            Status::Loaded
                        }
                    });
                    self.hyperflux.pairing.bind_info = devices;
                    self.hyperflux.pairing.scanned_info.clear();
                }
                "DUALLINK_SCAN_DEVICE" => {
                    let Some(devices) = payload.as_array() else {
                        return false;
                    };
                    self.hyperflux.pairing.scanned_info = devices.clone();
                    if devices.len() == 1 {
                        if let Some(slot) = states.iter().enumerate().find_map(|(slot, s)| {
                            (*s == Status::Scanning && devices[0]["category"] == category(slot))
                                .then_some(slot)
                        }) {
                            // wm's automatic MOUSE branch does not call nm(true).
                            self.bind_pairing(slot, slot == 0, devices[0].clone(), cx);
                        } else {
                            for (slot, state) in states.iter().enumerate() {
                                if *state == Status::Scanning {
                                    self.hyperflux.pairing.status[slot] = Status::Scanned;
                                }
                            }
                        }
                    } else {
                        for (slot, state) in states.iter().enumerate() {
                            if *state == Status::Scanning {
                                self.hyperflux.pairing.status[slot] = Status::Scanned;
                            }
                        }
                    }
                }
                "DUALLINK_BIND_DEVICE" => {
                    let device = payload["device"].clone();
                    let slot = if device["category"] == "KEYBOARD" { 0 } else { 1 };
                    // wm uses the latest connected pairedInfo prop, not its local bindInfo.
                    let mut devices = self.hyperflux.paired_info.clone();
                    devices.retain(|d| d["category"] != device["category"]);
                    devices.push(device);
                    self.hyperflux.pairing.bind_info = devices.clone();
                    self.hyperflux.paired_info = devices;
                    self.hyperflux.pairing.status[slot] = Status::JustPaired;
                    self.hyperflux.pairing.selected_ix = 0;
                    // Source close checks pre-response T/I; preserve that timing.
                    if states
                        .iter()
                        .all(|s| matches!(s, Status::Paired | Status::JustPaired))
                    {
                        self.close_pairing(false, cx);
                    }
                }
                "DUALLINK_UNBIND_DEVICE" => {
                    if let Some(slot) = states.iter().position(|s| *s == Status::JustUnpair) {
                        self.hyperflux.pairing.status[slot] = Status::Unpaired;
                    }
                    self.hyperflux.pairing.bind_info = self.hyperflux.paired_info.clone();
                    self.hyperflux.pairing.selected_ix = 0;
                }
                _ => {}
            }
        }
        self.hyperflux.pairing.clear_if_idle();
        cx.notify();
        true
    }
    fn pairing_master(&self) -> Option<Value> {
        let all = &self.hyperflux.connected_device_info;
        let mut device = all
            .iter()
            .find(|d| d["productId"] == 207 && d["editionId"] == self.hyperflux.edition)
            .or_else(|| all.iter().find(|d| d["productId"] == 207))?
            .clone();
        device["dongleId"] = Value::from(207);
        device["editionId"] = Value::from(self.hyperflux.edition);
        device["valid"] = Value::Bool(true);
        device["connected"] = Value::from(2);
        Some(device)
    }
    fn pairing_slave(&self, slot: usize) -> Value {
        let mut device=self.hyperflux.pairing.bind_info.iter().find(|d|d["category"]==category(slot)).cloned()
            .unwrap_or_else(||serde_json::json!({"name":{"en":""},"productName":"","category":category(slot),"productId":0,"editionId":0,"layoutId":0,"valid":false,"connected":0}));
        if truthy(&device["productId"]) {
            device["valid"] = Value::Bool(true);
            device["connected"] = Value::from(2);
            if device["dongleId"] == 144 {
                device["layoutId"] = Value::from("3");
            }
        }
        let status = self.hyperflux.pairing.status[slot];
        if (matches!(status, Status::Scanning | Status::Pairing)
            || status == Status::Scanned && self.hyperflux.pairing.scans(slot).len() > 1)
            && device["connected"] != 2
        {
            device["connected"] = Value::from(1);
        }
        device
    }
}

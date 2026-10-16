//! Current standalone multipairing: Ns/Ds/Ss, DUALLINK_* state and cards.
//! Device state enters through observed transport responses.
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PairingStatus {
    Initializing = 0,
    Ready = 1,
    Scanning = 2,
    ScanResults = 3,
    Binding = 4,
    Bound = 5,
    BindError = 6,
    CardUnbinding = 7,
    Unbinding = 8,
    Unbound = 9,
    UnbindError = 10,
    BindConfirmed = 11,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Primary,
    Secondary,
}
impl Lane {
    pub fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::Secondary => 1,
        }
    }
    pub fn category(self) -> &'static str {
        match self {
            Self::Primary => "KEYBOARD",
            Self::Secondary => "MOUSE",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PairingDevice {
    raw: Value,
    key: String,
    product_id: u32,
}
impl PairingDevice {
    pub fn parse(raw: Value) -> Result<Self, String> {
        let product_id = number(raw.get("productId"))
            .filter(|value| *value > 0)
            .ok_or("设备信息缺少有效产品编号")?;
        let identity = ["serialNumber", "deviceContainerId", "dongleId"]
            .into_iter()
            .find_map(|field| {
                string(raw.get(field)).filter(|value| !value.is_empty() && value != "0")
            })
            .unwrap_or_default();
        Ok(Self {
            key: format!("{product_id}:{identity}"),
            raw,
            product_id,
        })
    }
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn edition_id(&self) -> u32 {
        number(self.raw.get("editionId")).unwrap_or(0)
    }
    pub fn category(&self) -> &str {
        self.raw
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("")
    }
    pub fn name(&self, language: &str) -> String {
        // `cs.L`: current language, then English, then empty. A missing name
        // neither rejects the whole response nor invents another locale.
        self.raw
            .get("productName")
            .and_then(Value::as_object)
            .and_then(|names| names.get(language).or_else(|| names.get("en")))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }
    pub fn serial(&self) -> Option<&str> {
        self.raw
            .get("serialNumber")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.trim().is_empty())
    }
    pub fn connected(&self) -> bool {
        self.connection() == Some(true) && !self.sleeping()
    }
    /// Runtime power data affects presentation; it does not erase a binding.
    pub fn dimmed(&self) -> bool {
        self.sleeping() || self.connection() == Some(false)
    }
    fn sleeping(&self) -> bool {
        let power = self
            .raw
            .get("devicePowerState")
            .and_then(Value::as_str)
            .unwrap_or("");
        let charging = self
            .raw
            .get("powerStatus")
            .and_then(|power| power.get("chargingStatus"))
            .or_else(|| self.raw.get("chargingStatus"))
            .and_then(Value::as_str)
            .unwrap_or("");
        ["off", "standby", "sleep"]
            .iter()
            .any(|state| power.eq_ignore_ascii_case(state))
            || charging.eq_ignore_ascii_case("off")
    }
    fn connection(&self) -> Option<bool> {
        if let Some(value) = self.raw.get("connected").filter(|value| !value.is_null()) {
            return value
                .as_bool()
                .or_else(|| number(Some(value)).map(|value| matches!(value, 1 | 2)));
        }
        self.raw.get("status").and_then(|value| {
            number(Some(value)).map(|value| value == 1).or_else(|| {
                value
                    .as_str()
                    .map(|value| value.eq_ignore_ascii_case("connected"))
            })
        })
    }
    pub fn disconnected(&self) -> bool {
        self.raw.get("connected") == Some(&Value::Bool(false))
            || number(self.raw.get("connected")) == Some(0)
    }
    pub fn requires_continuation(&self) -> bool {
        number(self.raw.get("dongleId")) == Some(713)
    }
    fn unpair_id(&self) -> Value {
        json!(
            number(self.raw.get("dongleId"))
                .filter(|id| *id > 0)
                .unwrap_or(self.product_id)
        )
    }
    fn master_context_changed(&self, previous: &Self) -> bool {
        [
            "category",
            "canPairTwoDevices",
            "deviceContainerId",
            "dongleId",
            "connected",
            "devicePowerState",
        ]
        .iter()
        .any(|field| self.raw.get(*field) != previous.raw.get(*field))
    }
    fn set_disconnected(&mut self) {
        self.raw["connected"] = json!(0);
    }
    fn owner(&self) -> Result<Value, String> {
        let mut owner = self
            .raw
            .get("master")
            .filter(|owner| owner.is_object())
            .cloned()
            .ok_or("外部配对记录缺少所属接收器，无法确认解除配对的目标")?;
        if number(owner.get("productId"))
            .filter(|id| *id > 0)
            .is_none()
        {
            return Err("外部配对记录缺少有效的所属接收器编号".into());
        }
        if let Some(container) = self.raw.get("deviceContainerId") {
            owner["deviceContainerId"] = container.clone();
        }
        Ok(owner)
    }
}

fn number(value: Option<&Value>) -> Option<u32> {
    let value = value?;
    value
        .as_u64()
        .and_then(|value| value.try_into().ok())
        .or_else(|| value.as_str()?.parse().ok())
}
fn string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}
/// The binding mapper compares String(id ?? ""), without trimming strings.
fn source_id_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}
fn source_response_id_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}
fn devices(value: Value) -> Result<Vec<PairingDevice>, String> {
    let Value::Array(values) = value else {
        return Err("配对服务返回的设备列表格式无效".into());
    };
    let mut result = Vec::new();
    for value in values {
        let device = PairingDevice::parse(value)?;
        if !result
            .iter()
            .any(|known: &PairingDevice| known.key() == device.key())
        {
            result.push(device);
        }
    }
    Ok(result)
}

#[derive(Clone, Debug)]
pub enum PairingRequest {
    ReadBindings,
    Scan(Lane),
    Bind(PairingDevice),
    Unbind {
        device: PairingDevice,
        external: bool,
    },
    /// The second external-owner request uses the product ID after the dongle ID.
    UnbindProduct(PairingDevice),
    /// Reclaim sends one unbind, then scans after its acknowledgement or 2 s.
    ReclaimExternal(PairingDevice),
}
impl PairingRequest {
    pub fn operation(&self) -> &'static str {
        match self {
            Self::ReadBindings => "DUALLINK_BIND_INFO",
            Self::Scan(_) => "DUALLINK_SCAN_DEVICE",
            Self::Bind(_) => "DUALLINK_BIND_DEVICE",
            Self::Unbind { .. } | Self::UnbindProduct(_) | Self::ReclaimExternal(_) => {
                "DUALLINK_UNBIND_DEVICE"
            }
        }
    }
    pub fn payload(&self, dual: bool) -> Value {
        match self {
            Self::ReadBindings => json!({}),
            Self::Scan(lane) => json!({"status": 1, "category": dual.then(|| lane.category())}),
            Self::Bind(device) => json!({"mode": 1, "device": device.raw}),
            Self::Unbind { device, .. } | Self::ReclaimExternal(device) => json!({
                "productId": device.unpair_id(),
                "category": dual.then(|| device.category()),
            }),
            Self::UnbindProduct(device) => json!({
                "productId": device.product_id(),
                "category": dual.then(|| device.category()),
            }),
        }
    }
    pub fn external(&self) -> bool {
        matches!(
            self,
            Self::Unbind { external: true, .. } | Self::UnbindProduct(_) | Self::ReclaimExternal(_)
        )
    }
    pub fn routing_context(&self, master: Value) -> Result<Value, String> {
        match self {
            Self::Unbind {
                device,
                external: true,
            }
            | Self::UnbindProduct(device)
            | Self::ReclaimExternal(device) => Ok(device.owner().unwrap_or(master)),
            _ => Ok(master),
        }
    }
    pub fn fallback(&self) -> Option<Self> {
        match self {
            Self::Unbind {
                device,
                external: true,
            } if number(Some(&device.unpair_id())) != Some(device.product_id()) => {
                Some(Self::UnbindProduct(device.clone()))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ticket {
    id: u64,
    generation: u64,
    request: PairingRequest,
    lane: Lane,
}
impl Ticket {
    pub fn request(&self) -> &PairingRequest {
        &self.request
    }
    pub fn lane(&self) -> Lane {
        self.lane
    }
}

#[derive(Clone)]
pub struct DeviceCard {
    device: PairingDevice,
    status: PairingStatus,
    external: bool,
}
impl DeviceCard {
    pub fn device(&self) -> &PairingDevice {
        &self.device
    }
    pub fn status(&self) -> PairingStatus {
        self.status
    }
    pub fn external(&self) -> bool {
        self.external
    }
    pub fn paired(&self) -> bool {
        matches!(
            self.status,
            PairingStatus::Bound | PairingStatus::BindConfirmed
        )
    }
    pub fn actionable(&self) -> bool {
        // An external record that is no longer connected is not a scan result.
        !self.external || self.device.connected()
    }
}

pub struct PairingState {
    statuses: [PairingStatus; 2],
    masters: Vec<PairingDevice>,
    master: Option<String>,
    dual: bool,
    bound: Vec<PairingDevice>,
    scanned: Vec<PairingDevice>,
    external: Vec<PairingDevice>,
    active_devices: [Option<String>; 2],
    replacement: [Option<PairingDevice>; 2],
    pending: [Option<Ticket>; 2],
    generation: u64,
    next_request: u64,
    auto_paired: [bool; 2],
    recovery_request: Option<(Lane, PairingStatus)>,
    error: Option<String>,
}
impl Default for PairingState {
    fn default() -> Self {
        Self {
            statuses: [PairingStatus::Initializing; 2],
            masters: Vec::new(),
            master: None,
            dual: false,
            bound: Vec::new(),
            scanned: Vec::new(),
            external: Vec::new(),
            active_devices: [None, None],
            replacement: [None, None],
            pending: [None, None],
            generation: 0,
            next_request: 0,
            auto_paired: [false; 2],
            recovery_request: None,
            error: None,
        }
    }
}
impl PairingState {
    /// Current `xs` / `multiDevicePairingInit` owner metadata. This is the
    /// source URL/message record, not an enumerated or paired device.
    pub fn initialize_device_info(&mut self, raw: Value) -> Result<(), String> {
        let product_id = number(raw.get("productId"))
            .filter(|value| *value > 0)
            .ok_or("Pairing initialization has no valid productId")?;
        self.invalidate();
        let key = format!("{product_id}:owner");
        self.dual = raw.get("canPairTwoDevices") == Some(&Value::Bool(true));
        self.masters = vec![PairingDevice {
            raw,
            key: key.clone(),
            product_id,
        }];
        self.master = Some(key);
        self.bound.clear();
        self.scanned.clear();
        self.auto_paired = [false; 2];
        self.statuses = [PairingStatus::Initializing; 2];
        self.error = None;
        Ok(())
    }
    pub fn status(&self, lane: Lane) -> PairingStatus {
        self.statuses[lane.index()]
    }
    pub fn await_bindings(&mut self) {
        self.statuses = [PairingStatus::Initializing; 2];
    }
    pub fn continue_713(&mut self) -> Option<PairingRequest> {
        let pending_lane = self.pending.iter().position(|ticket| {
            matches!(ticket.as_ref().map(|ticket| &ticket.request), Some(PairingRequest::Bind(device)) if device.requires_continuation())
        })?;
        // `Ns`'s continuePairing effect uses the first 713 scan entry,
        // including when the selected card was a different 713 entry.
        let device = self
            .scanned
            .iter()
            .find(|device| device.requires_continuation())?
            .clone();
        self.pending[pending_lane] = None;
        Some(PairingRequest::Bind(device))
    }
    pub fn dual(&self) -> bool {
        self.dual
    }
    pub fn busy(&self) -> bool {
        self.pending.iter().any(Option::is_some)
    }
    pub fn lane_busy(&self, lane: Lane) -> bool {
        self.pending[lane.index()].is_some()
            || self.pending[Lane::Primary.index()]
                .as_ref()
                .is_some_and(|ticket| matches!(ticket.request, PairingRequest::ReadBindings))
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    #[cfg(test)]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn recovery_request(&self) -> Option<(Lane, PairingStatus)> {
        self.recovery_request
    }
    pub fn clears_recovery(&self) -> bool {
        // `Ns` calls `Ce` during rendering only for X/$ in both slots.
        self.statuses
            .iter()
            .all(|status| matches!(status, PairingStatus::Ready | PairingStatus::Bound))
    }
    pub fn master(&self) -> Option<&PairingDevice> {
        self.master
            .as_ref()
            .and_then(|key| self.masters.iter().find(|device| device.key() == key))
    }
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }
    pub fn replace_masters(&mut self, value: Value) -> Result<(), String> {
        let masters = devices(value)?;
        if masters.iter().any(|device| device.serial().is_none()) {
            return Err("接收器列表缺少序列号，无法确认设备身份".into());
        }
        let previous = self.master.clone();
        let previous_device = self.master().cloned();
        self.masters = masters;
        if !self
            .masters
            .iter()
            .any(|device| Some(device.key()) == previous.as_deref())
        {
            self.invalidate();
            self.master = None;
            self.dual = false;
            self.bound.clear();
            self.scanned.clear();
            self.statuses = [PairingStatus::Ready; 2];
            self.error = previous.map(|_| "当前配对设备已断开，请重新选择设备。".into());
        } else if let Some(current) = self.master().cloned() {
            if previous_device
                .as_ref()
                .is_some_and(|previous| current.master_context_changed(previous))
            {
                self.invalidate();
                self.master = None;
                self.dual = false;
                self.bound.clear();
                self.scanned.clear();
                self.statuses = [PairingStatus::Ready; 2];
                self.error =
                    Some("当前配对设备的连接或接收器信息已变化，请重新选择并读取设备。".into());
            }
        }
        Ok(())
    }
    pub fn set_external(&mut self, value: Value) -> Result<(), String> {
        let external = devices(value)?;
        let invalidated = self.pending.iter().flatten().any(|ticket| {
            let device = match &ticket.request {
                PairingRequest::Unbind {
                    device,
                    external: true,
                }
                | PairingRequest::UnbindProduct(device)
                | PairingRequest::ReclaimExternal(device) => device,
                _ => return false,
            };
            !external.contains(device)
        });
        self.external = external;
        if invalidated {
            self.invalidate();
            self.error = Some("外部设备的配对记录已变化，请重新扫描后重试。".into());
        }
        Ok(())
    }
    pub fn context(&self) -> Option<Value> {
        self.master().map(|device| device.raw.clone())
    }
    pub fn cancellation_context(&self) -> Option<(Value, bool)> {
        let master = self.context()?;
        match self.pending.iter().flatten().next() {
            Some(ticket) => Some((
                ticket.request.routing_context(master).ok()?,
                ticket.request.external(),
            )),
            None => Some((master, false)),
        }
    }
    pub fn lane_for(&self, device: &PairingDevice) -> Lane {
        if self.dual && device.category() == "MOUSE" {
            Lane::Secondary
        } else {
            Lane::Primary
        }
    }
    fn eligible(&self, device: &PairingDevice) -> bool {
        if !matches!(device.category(), "MOUSE" | "KEYBOARD") {
            return false;
        }
        let Some(master) = self.master() else {
            return false;
        };
        device.product_id() != master.product_id()
            && (self.dual || master.category() != device.category())
    }
    pub fn cards(&self) -> Vec<DeviceCard> {
        let mut cards = Vec::new();
        for device in &self.bound {
            if !self.eligible(device)
                || cards
                    .iter()
                    .any(|card: &DeviceCard| card.device.product_id() == device.product_id())
            {
                continue;
            }
            let state = self.status(self.lane_for(device));
            // Current `Ss.Z` projects every bound entry from its category
            // status; it never borrows a selected scanned card's BindError.
            let status = if matches!(
                state,
                PairingStatus::Unbinding | PairingStatus::CardUnbinding
            ) {
                PairingStatus::CardUnbinding
            } else if state == PairingStatus::Unbound || device.disconnected() {
                PairingStatus::Unbound
            } else {
                PairingStatus::Bound
            };
            cards.push(DeviceCard {
                device: device.clone(),
                status,
                external: false,
            });
        }
        for (values, external) in [(&self.scanned, false), (&self.external, true)] {
            for device in values {
                if !self.eligible(device)
                    || cards
                        .iter()
                        .any(|card| card.device.product_id() == device.product_id())
                {
                    continue;
                }
                let status = if external {
                    if self.pending.iter().flatten().any(|ticket| {
                        matches!(&ticket.request,
                            PairingRequest::Unbind { device: active, external: true }
                            | PairingRequest::UnbindProduct(active) if active.key() == device.key())
                    }) {
                        PairingStatus::CardUnbinding
                    } else {
                        PairingStatus::Unbound
                    }
                } else if self.status(self.lane_for(device)) == PairingStatus::Scanning {
                    PairingStatus::Scanning
                } else if self.active_devices[self.lane_for(device).index()].as_deref()
                    == Some(device.key())
                {
                    match self.status(self.lane_for(device)) {
                        PairingStatus::Unbinding => PairingStatus::CardUnbinding,
                        value @ (PairingStatus::Binding
                        | PairingStatus::Scanning
                        | PairingStatus::BindError
                        | PairingStatus::UnbindError) => value,
                        _ => PairingStatus::Unbound,
                    }
                } else {
                    PairingStatus::Unbound
                };
                cards.push(DeviceCard {
                    device: device.clone(),
                    status,
                    external,
                });
            }
        }
        cards
    }
    pub fn pair_request(
        &mut self,
        device: PairingDevice,
        external: bool,
    ) -> Result<PairingRequest, String> {
        let lane = self.lane_for(&device);
        if self.lane_busy(lane) {
            return Err("请等待当前配对操作完成或取消操作".into());
        }
        if !self.cards().iter().any(|card| {
            card.device.key() == device.key() && card.external == external && card.actionable()
        }) {
            return Err("所选设备已不可用于配对，请重新扫描后重试".into());
        }
        if external && device.connected() {
            self.replacement[lane.index()] = Some(device.clone());
            Ok(PairingRequest::ReclaimExternal(device))
        } else if let Some(previous) = self
            .bound
            .iter()
            .find(|previous| self.lane_for(previous) == lane && previous.key() != device.key())
            .cloned()
        {
            self.replacement[lane.index()] = Some(device);
            Ok(PairingRequest::Unbind {
                device: previous,
                external: false,
            })
        } else {
            self.replacement[lane.index()] = None;
            Ok(PairingRequest::Bind(device))
        }
    }
    pub fn begin(&mut self, request: PairingRequest) -> Result<Ticket, String> {
        if self.master().is_none() {
            return Err("尚未读取可用的无线配对设备".into());
        }
        let lane = match &request {
            PairingRequest::Scan(lane) => *lane,
            PairingRequest::Bind(device)
            | PairingRequest::Unbind { device, .. }
            | PairingRequest::UnbindProduct(device)
            | PairingRequest::ReclaimExternal(device) => self.lane_for(device),
            _ => Lane::Primary,
        };
        if self.lane_busy(lane) || matches!(request, PairingRequest::ReadBindings) && self.busy() {
            return Err("请等待当前配对操作完成或取消操作".into());
        }
        if lane == Lane::Secondary && !self.dual {
            return Err("当前设备不支持第二配对通道".into());
        }
        if let PairingRequest::Bind(device)
        | PairingRequest::Unbind { device, .. }
        | PairingRequest::UnbindProduct(device)
        | PairingRequest::ReclaimExternal(device) = &request
        {
            if !self.cards().iter().any(|card| card.device == *device) {
                return Err("所选设备已不在有效配对列表中".into());
            }
            if !request.external() {
                self.active_devices[lane.index()] = Some(device.key().into());
            }
        }
        if matches!(request, PairingRequest::Scan(_)) {
            self.active_devices[lane.index()] = None;
        } else if matches!(request, PairingRequest::ReadBindings) {
            self.active_devices = [None, None];
        }
        if matches!(request, PairingRequest::ReadBindings) && self.dual {
            self.statuses[Lane::Secondary.index()] = PairingStatus::Initializing;
        }
        let next_status = match request {
            PairingRequest::ReadBindings => PairingStatus::Initializing,
            PairingRequest::Scan(_) => PairingStatus::Scanning,
            PairingRequest::Bind(_) => PairingStatus::Binding,
            PairingRequest::Unbind { .. }
            | PairingRequest::UnbindProduct(_)
            | PairingRequest::ReclaimExternal(_) => PairingStatus::Unbinding,
        };
        // `Ds` external unpair/reclaim listeners are independent of `Ns`'s
        // keyboard/mouse status reducers. Only their own card marker changes.
        if !request.external() {
            self.statuses[lane.index()] = next_status;
        }
        if let PairingRequest::Unbind {
            device,
            external: false,
        } = &request
        {
            let id = number(Some(&device.unpair_id()));
            for previous in &mut self.bound {
                if number(previous.raw.get("productId")) == id
                    || number(previous.raw.get("dongleId")) == id
                {
                    // `Ns.M` changes the UI binding record before sending.
                    // This is an optimistic UI flag, not a successful write.
                    previous.set_disconnected();
                }
            }
        }
        self.next_request = self.next_request.wrapping_add(1);
        let ticket = Ticket {
            id: self.next_request,
            generation: self.generation,
            request,
            lane,
        };
        self.error = None;
        self.pending[lane.index()] = Some(ticket.clone());
        Ok(ticket)
    }
    pub fn current(&self, ticket: &Ticket) -> bool {
        self.generation == ticket.generation
            && self.pending[ticket.lane.index()]
                .as_ref()
                .is_some_and(|pending| pending.id == ticket.id)
    }
    pub fn receive(
        &mut self,
        ticket: &Ticket,
        result: Result<Value, String>,
    ) -> Option<PairingRequest> {
        if !self.current(ticket) {
            return None;
        }
        self.recovery_request = None;
        match (&ticket.request, &result) {
            (PairingRequest::Bind(_), Ok(value))
                if value.get("device").is_none_or(Value::is_null) =>
            {
                return None;
            }
            (PairingRequest::ReclaimExternal(_), Err(_)) => return None,
            (
                PairingRequest::Unbind { external: true, .. } | PairingRequest::UnbindProduct(_),
                Ok(value),
            ) if value.get("productId").map(source_id_string)
                != Some(source_id_string(
                    &ticket.request.payload(self.dual)["productId"],
                )) =>
            {
                return None;
            }
            _ => {}
        }
        self.pending[ticket.lane.index()] = None;
        if ticket.request.external() && result.is_err() {
            // `Ds.ee` closes its listener/marker on error; no `Ns` error
            // status or automatic cancel/recovery is dispatched.
            self.error = result.err();
            return None;
        }
        match result.and_then(|value| self.accept(ticket, value)) {
            Ok(next) => next,
            Err(error) => {
                self.error = Some(error);
                // `Ns` broadcasts have no category correlation on errors.
                // The first active primary status wins, even when the Rust
                // transport ticket belongs to the other category.
                let active_status = match ticket.request {
                    PairingRequest::Scan(_) => PairingStatus::Scanning,
                    PairingRequest::Bind(_) => PairingStatus::Binding,
                    PairingRequest::Unbind { .. } => PairingStatus::Unbinding,
                    _ => PairingStatus::Initializing,
                };
                let response_lane = [Lane::Primary, Lane::Secondary]
                    .into_iter()
                    .find(|lane| self.status(*lane) == active_status);
                if response_lane.is_none()
                    && !matches!(ticket.request, PairingRequest::ReadBindings)
                {
                    if matches!(ticket.request, PairingRequest::Bind(_)) {
                        // Original binding failure always schedules its
                        // global reset, even without a currently Binding slot.
                        self.recovery_request = Some((Lane::Primary, PairingStatus::BindError));
                    }
                    return None;
                }
                let response_lane = response_lane.unwrap_or(Lane::Primary);
                let error_status = match ticket.request {
                    PairingRequest::ReadBindings => {
                        self.bound.clear();
                        self.scanned.clear();
                        self.statuses = [PairingStatus::Ready; 2];
                        PairingStatus::Ready
                    }
                    PairingRequest::Scan(_) => {
                        if self.dual {
                            self.scanned
                                .retain(|device| device.category() != response_lane.category());
                        } else {
                            self.scanned.clear();
                        }
                        PairingStatus::ScanResults
                    }
                    PairingRequest::Bind(_) => PairingStatus::BindError,
                    PairingRequest::Unbind { .. }
                    | PairingRequest::UnbindProduct(_)
                    | PairingRequest::ReclaimExternal(_) => PairingStatus::UnbindError,
                };
                self.statuses[response_lane.index()] = error_status;
                if matches!(
                    error_status,
                    PairingStatus::BindError | PairingStatus::UnbindError
                ) {
                    self.recovery_request = Some((response_lane, error_status));
                }
                self.replacement[response_lane.index()] = None;
                // Current `Ns` invokes `Is` after a keyboard scan/bind error.
                (self.dual
                    && response_lane == Lane::Primary
                    && !matches!(
                        self.status(Lane::Secondary),
                        PairingStatus::Scanning | PairingStatus::Binding
                    )
                    && matches!(
                        ticket.request,
                        PairingRequest::Scan(_) | PairingRequest::Bind(_)
                    ))
                .then_some(PairingRequest::Scan(Lane::Secondary))
            }
        }
    }
    fn accept(&mut self, ticket: &Ticket, value: Value) -> Result<Option<PairingRequest>, String> {
        match &ticket.request {
            PairingRequest::ReadBindings => {
                // `Ns` accepts a non-array successful payload as an empty list.
                let values = devices(if value.is_array() { value } else { json!([]) })?;
                self.bound = values;
                self.scanned.clear();
                self.restore_statuses();
                Ok(Some(PairingRequest::Scan(Lane::Primary)))
            }
            PairingRequest::Scan(lane) => {
                let values = devices(value)?;
                let was_primary_scanning = self.status(Lane::Primary) == PairingStatus::Scanning;
                let response_lane = [Lane::Primary, Lane::Secondary]
                    .into_iter()
                    .find(|lane| self.status(*lane) == PairingStatus::Scanning);
                if self.dual && response_lane.is_some() {
                    self.scanned
                        .retain(|device| device.category() != response_lane.unwrap().category());
                } else {
                    self.scanned.clear();
                }
                self.scanned.extend(values);
                for status in &mut self.statuses {
                    if *status == PairingStatus::Scanning {
                        *status = PairingStatus::ScanResults;
                    }
                }
                let lane = response_lane.as_ref().unwrap_or(lane);
                if let Some(target) = self.replacement[lane.index()].take() {
                    if let Some(device) = self
                        .scanned
                        .iter()
                        .find(|device| device.product_id() == target.product_id())
                        .cloned()
                    {
                        return self.pair_request(device, false).map(Some);
                    }
                    self.error =
                        Some("解除原配对后尚未扫描到目标设备，请保持设备唤醒并重试。".into());
                    // The current `Ds` target lookup compares productId only.
                    return Ok(None);
                }
                let lane_has_binding = self
                    .bound
                    .iter()
                    .any(|device| self.lane_for(device) == *lane);
                // `Ss` auto-selects each dual category at ScanResults. Its
                // single-category branch requires Ready; a completed scan
                // is ScanResults and must offer an explicit Pair action.
                if self.dual
                    && !self.statuses.contains(&PairingStatus::Binding)
                    && !self.auto_paired[lane.index()]
                    && !lane_has_binding
                {
                    if let Some(device) = self
                        .scanned
                        .iter()
                        .find(|device| self.lane_for(device) == *lane && self.eligible(device))
                        .cloned()
                    {
                        let request = self.pair_request(device, false)?;
                        self.auto_paired[lane.index()] = true;
                        return Ok(Some(request));
                    }
                }
                if self.dual
                    && was_primary_scanning
                    && !matches!(
                        self.status(Lane::Secondary),
                        PairingStatus::Scanning | PairingStatus::Binding
                    )
                    && (lane_has_binding
                        || !self
                            .scanned
                            .iter()
                            .any(|device| device.category() == "KEYBOARD"))
                {
                    return Ok(Some(PairingRequest::Scan(Lane::Secondary)));
                }
                Ok(None)
            }
            PairingRequest::Bind(_) => {
                // Current `Ns` ignores a success message with no device.
                let Some(raw) = value.get("device").filter(|raw| !raw.is_null()) else {
                    return Ok(None);
                };
                let device = PairingDevice::parse(raw.clone())?;
                let confirmed_lane = if self.dual && device.category() != "KEYBOARD" {
                    Lane::Secondary
                } else {
                    Lane::Primary
                };
                self.bound
                    .retain(|previous| previous.category() != device.category());
                self.bound.push(device);
                self.statuses[confirmed_lane.index()] = PairingStatus::BindConfirmed;
                self.active_devices[confirmed_lane.index()] = None;
                if self.dual
                    && confirmed_lane == Lane::Primary
                    && !matches!(
                        self.status(Lane::Secondary),
                        PairingStatus::Scanning | PairingStatus::Binding
                    )
                {
                    Ok(Some(PairingRequest::Scan(Lane::Secondary)))
                } else {
                    Ok(None)
                }
            }
            PairingRequest::Unbind {
                external: false, ..
            } => {
                // `Ns` uses the first Unbinding status, independently of the
                // payload category. Missing/different IDs still acknowledge
                // that status; only records matching the response ID change.
                if self.status(Lane::Primary) == PairingStatus::Unbinding {
                    self.statuses[Lane::Primary.index()] = PairingStatus::Unbound;
                } else if self.status(Lane::Secondary) == PairingStatus::Unbinding {
                    self.statuses[Lane::Secondary.index()] = PairingStatus::Unbound;
                }
                if let Some(id) = value.get("productId") {
                    let id = source_response_id_string(id);
                    for previous in &mut self.bound {
                        if source_id_string(previous.raw.get("productId").unwrap_or(&Value::Null))
                            == id
                            || source_id_string(
                                previous.raw.get("dongleId").unwrap_or(&Value::Null),
                            ) == id
                        {
                            previous.set_disconnected();
                        }
                    }
                }
                if let Some(target) = self.replacement[ticket.lane.index()].take() {
                    Ok(Some(PairingRequest::Bind(target)))
                } else {
                    Ok(None)
                }
            }
            PairingRequest::ReclaimExternal(_) => {
                // `Ds.te` accepts any successful UNBIND message on the
                // owner's response channel and then scans the target lane.
                Ok(Some(PairingRequest::Scan(ticket.lane)))
            }
            PairingRequest::Unbind {
                device,
                external: true,
            }
            | PairingRequest::UnbindProduct(device) => {
                let id = value.get("productId").ok_or("解除配对响应缺少产品编号")?;
                let expected = ticket.request.payload(self.dual)["productId"].clone();
                if number(Some(id)) != number(Some(&expected)) {
                    return Err("解除配对响应与所选设备不一致".into());
                }
                if let Some(next) = ticket.request.fallback() {
                    return Ok(Some(next));
                }
                let external = ticket.request.external();
                for previous in if external {
                    &mut self.external
                } else {
                    &mut self.bound
                } {
                    if previous.key() == device.key() {
                        previous.set_disconnected();
                    }
                }
                Ok(None)
            }
        }
    }
    pub fn recover(&mut self, generation: u64, lane: Lane, status: PairingStatus) {
        if self.generation != generation {
            return;
        }
        if status == PairingStatus::BindError {
            // `Ns`'s four-second SET_BINDINFO resets both statuses and all
            // binding/scan records, even if the other lane changed meanwhile.
            self.statuses = [PairingStatus::Ready; 2];
            self.bound.clear();
            self.scanned.clear();
            self.active_devices = [None, None];
            self.error = None;
            return;
        }
        match status {
            PairingStatus::UnbindError => {
                self.statuses[lane.index()] = PairingStatus::Bound;
            }
            _ => return,
        }
        if !self.busy() {
            self.error = None;
        }
        self.active_devices[lane.index()] = None;
    }
    /// A timed fallback starts another real request; it never marks an unpair as successful.
    pub fn external_fallback(&mut self, ticket: &Ticket) -> Option<PairingRequest> {
        if !self.current(ticket) {
            return None;
        }
        let next = if matches!(ticket.request, PairingRequest::ReclaimExternal(_)) {
            // Current `Ds.te` resumes scanning after 2 s without an ack. This
            // starts discovery only: the external binding remains unconfirmed.
            PairingRequest::Scan(ticket.lane)
        } else {
            ticket.request.fallback()?
        };
        self.pending[ticket.lane.index()] = None;
        Some(next)
    }
    /// `Ds.ee`'s ten-second listener cleanup changes no device or lane status.
    pub fn expire_external(&mut self, ticket: &Ticket) {
        if self.current(ticket)
            && matches!(
                ticket.request,
                PairingRequest::Unbind { external: true, .. } | PairingRequest::UnbindProduct(_)
            )
        {
            self.pending[ticket.lane.index()] = None;
        }
    }
    fn restored_status(&self, lane: Lane) -> PairingStatus {
        if self
            .bound
            .iter()
            .any(|device| self.lane_for(device) == lane)
        {
            PairingStatus::Bound
        } else {
            PairingStatus::Ready
        }
    }
    fn restore_statuses(&mut self) {
        for lane in [Lane::Primary, Lane::Secondary] {
            self.statuses[lane.index()] = self.restored_status(lane);
        }
    }
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = [None, None];
        self.replacement = [None, None];
        self.recovery_request = None;
        self.active_devices = [None, None];
        self.restore_statuses();
    }
}

//! 4130: Ge..et, DUALLINK_* payloads and category-specific list projection.
//! Success enters only through a matching, validated transport response.
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

impl PairingStatus {
    pub fn key(self) -> &'static str {
        match self {
            Self::Initializing => "LOADING",
            Self::Ready => "PAIR",
            Self::Scanning => "SCANNING",
            Self::ScanResults => "DUALLINK_SCAN_DEVICE_FOUND",
            Self::Binding => "PAIRING",
            Self::Bound | Self::BindConfirmed => "PAIRED",
            Self::BindError => "PAIRING_FAILED",
            Self::CardUnbinding | Self::Unbinding => "UNPAIRING",
            Self::Unbound => "PAIR",
            Self::UnbindError => "UNPAIRING_FAILED",
        }
    }
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
        let valid_name = match raw.get("productName") {
            Some(Value::String(name)) => !name.trim().is_empty(),
            Some(Value::Object(names)) => names
                .values()
                .any(|name| name.as_str().is_some_and(|name| !name.trim().is_empty())),
            _ => false,
        };
        if !valid_name {
            return Err("设备信息缺少产品名称".into());
        }
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
        match self.raw.get("productName") {
            Some(Value::String(name)) => name.clone(),
            Some(Value::Object(names)) => names
                .iter()
                .filter(|(_, value)| value.as_str().is_some_and(|name| !name.trim().is_empty()))
                .min_by_key(|(key, _)| {
                    if key.eq_ignore_ascii_case(language) {
                        0
                    } else if key.eq_ignore_ascii_case("en") {
                        1
                    } else {
                        2
                    }
                })
                .and_then(|(_, value)| value.as_str())
                .unwrap_or_default()
                .to_string(),
            _ => String::new(),
        }
    }
    pub fn serial(&self) -> Option<&str> {
        self.raw
            .get("serialNumber")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.trim().is_empty())
    }
    /// Dashboard 7861 `hi` 的 guard：`productId` 与 `deviceContainerId` 同时存在
    /// 才打开该产品的配对窗口。返回窗口层需要的身份字段，缺一即为 `None`。
    pub fn open_window_payload(&self) -> Option<Value> {
        let container =
            string(self.raw.get("deviceContainerId")).filter(|value| !value.is_empty())?;
        let product = number(self.raw.get("productId")).filter(|value| *value > 0)?;
        let mut payload = json!({
            "productId": product,
            "deviceContainerId": container,
            "serialNumber": self.serial(),
        });
        // Dashboard 7861 forwards these values when it builds the
        // `displayMode=multiDevicePairing` product URL. Keep the fields on the
        // opener payload so the native window can consume the same source
        // metadata instead of reducing it to only the two identity guards.
        if let Some(category) = self.raw.get("category").and_then(Value::as_str) {
            payload["category"] = json!(category);
        }
        for field in ["canPairTwoDevices", "isProductivity"] {
            if let Some(value) = self.raw.get(field).filter(|value| value.is_boolean()) {
                payload[field] = value.clone();
            }
        }
        if let Some(name) = self.raw.get("deviceName").and_then(Value::as_str) {
            if !name.trim().is_empty() {
                payload["deviceName"] = json!(name);
            }
        }
        if let Some(lang) = self.raw.get("lang").and_then(Value::as_str) {
            if !lang.trim().is_empty() {
                payload["lang"] = json!(lang);
            }
        }
        if let Some(masters) = self.raw.get("allMasters") {
            payload["allMasters"] = masters.clone();
        }
        Some(payload)
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
        number(self.raw.get("connected")) == Some(0)
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
    fn identity(&self) -> Option<(&'static str, String)> {
        ["serialNumber", "deviceContainerId", "dongleId"]
            .into_iter()
            .find_map(|field| {
                string(self.raw.get(field))
                    .filter(|value| !value.is_empty() && value != "0")
                    .map(|value| (field, value))
            })
    }
    fn matches_identity(&self, target: &Self) -> bool {
        target
            .identity()
            .is_some_and(|(field, identity)| string(self.raw.get(field)) == Some(identity))
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
    fn same_unit(&self, target: &Self) -> bool {
        self.product_id() == target.product_id()
            && self.category() == target.category()
            && self.matches_identity(target)
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
            | Self::ReclaimExternal(device) => device.owner(),
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
    replacement: Option<PairingDevice>,
    pending: Option<Ticket>,
    generation: u64,
    next_request: u64,
    auto_paired: [bool; 2],
    error: Option<String>,
}
impl Default for PairingState {
    fn default() -> Self {
        Self {
            statuses: [PairingStatus::Ready; 2],
            masters: Vec::new(),
            master: None,
            dual: false,
            bound: Vec::new(),
            scanned: Vec::new(),
            external: Vec::new(),
            active_devices: [None, None],
            replacement: None,
            pending: None,
            generation: 0,
            next_request: 0,
            auto_paired: [false; 2],
            error: None,
        }
    }
}
impl PairingState {
    pub fn status(&self, lane: Lane) -> PairingStatus {
        self.statuses[lane.index()]
    }
    pub fn dual(&self) -> bool {
        self.dual
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn masters(&self) -> &[PairingDevice] {
        &self.masters
    }
    pub fn master(&self) -> Option<&PairingDevice> {
        self.master
            .as_ref()
            .and_then(|key| self.masters.iter().find(|device| device.key() == key))
    }
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }
    #[allow(dead_code)] // Used by the service metadata seam once a DUALLINK adapter is connected.
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
    pub fn select_master(&mut self, key: &str) -> Result<(), String> {
        let device = self
            .masters
            .iter()
            .find(|device| device.key() == key)
            .ok_or("所选接收器已不可用")?;
        let dual = device.raw.get("canPairTwoDevices") == Some(&Value::Bool(true));
        self.invalidate();
        self.master = Some(key.into());
        self.dual = dual;
        self.auto_paired = [false; 2];
        self.bound.clear();
        self.scanned.clear();
        self.statuses = [PairingStatus::Ready; 2];
        self.error = None;
        Ok(())
    }
    #[allow(dead_code)] // Service-owned allMasters/duallink records; never filled from previews.
    pub fn set_external(&mut self, value: Value) -> Result<(), String> {
        let external = devices(value)?;
        let invalidated = self.pending.as_ref().is_some_and(|ticket| {
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
        match self.pending.as_ref() {
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
            let active =
                self.active_devices[self.lane_for(device).index()].as_deref() == Some(device.key());
            let status =
                if active && matches!(state, PairingStatus::Binding | PairingStatus::BindError) {
                    state
                } else if active
                    && matches!(
                        state,
                        PairingStatus::Unbinding | PairingStatus::CardUnbinding
                    )
                {
                    PairingStatus::CardUnbinding
                } else if active && state == PairingStatus::UnbindError {
                    PairingStatus::UnbindError
                } else if device.disconnected() {
                    PairingStatus::Unbound
                } else if state == PairingStatus::BindConfirmed {
                    PairingStatus::BindConfirmed
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
                let status = if self.status(self.lane_for(device)) == PairingStatus::Scanning {
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
        if self.busy() {
            return Err("请等待当前配对操作完成或取消操作".into());
        }
        if !self.cards().iter().any(|card| {
            card.device.key() == device.key() && card.external == external && card.actionable()
        }) {
            return Err("所选设备已不可用于配对，请重新扫描后重试".into());
        }
        if device.identity().is_none() {
            return Err("扫描设备缺少可验证的设备标识，请重新扫描后重试".into());
        }
        if external {
            device.owner()?;
        }
        let lane = self.lane_for(&device);
        if external && device.connected() {
            self.replacement = Some(device.clone());
            Ok(PairingRequest::ReclaimExternal(device))
        } else if let Some(previous) = self
            .bound
            .iter()
            .find(|previous| {
                self.lane_for(previous) == lane
                    && !previous.disconnected()
                    && previous.key() != device.key()
            })
            .cloned()
        {
            self.replacement = Some(device);
            Ok(PairingRequest::Unbind {
                device: previous,
                external: false,
            })
        } else {
            self.replacement = None;
            Ok(PairingRequest::Bind(device))
        }
    }
    pub fn begin(&mut self, request: PairingRequest) -> Result<Ticket, String> {
        if self.busy() {
            return Err("请等待当前配对操作完成或取消操作".into());
        }
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
        if lane == Lane::Secondary && !self.dual {
            return Err("当前设备不支持第二配对通道".into());
        }
        if let PairingRequest::Bind(device) = &request {
            if device.identity().is_none() {
                return Err("扫描设备缺少可验证的设备标识，请重新扫描后重试".into());
            }
        }
        if let PairingRequest::Bind(device)
        | PairingRequest::Unbind { device, .. }
        | PairingRequest::UnbindProduct(device)
        | PairingRequest::ReclaimExternal(device) = &request
        {
            if !self.cards().iter().any(|card| card.device == *device) {
                return Err("所选设备已不在有效配对列表中".into());
            }
            self.active_devices[lane.index()] = Some(device.key().into());
        }
        if matches!(request, PairingRequest::Scan(_)) {
            self.active_devices[lane.index()] = None;
        } else if matches!(request, PairingRequest::ReadBindings) {
            self.active_devices = [None, None];
        }
        if matches!(request, PairingRequest::ReadBindings) && self.dual {
            self.statuses[Lane::Secondary.index()] = PairingStatus::Initializing;
        }
        self.statuses[lane.index()] = match request {
            PairingRequest::ReadBindings => PairingStatus::Initializing,
            PairingRequest::Scan(_) => PairingStatus::Scanning,
            PairingRequest::Bind(_) => PairingStatus::Binding,
            PairingRequest::Unbind { .. }
            | PairingRequest::UnbindProduct(_)
            | PairingRequest::ReclaimExternal(_) => PairingStatus::Unbinding,
        };
        self.next_request = self.next_request.wrapping_add(1);
        let ticket = Ticket {
            id: self.next_request,
            generation: self.generation,
            request,
            lane,
        };
        self.error = None;
        self.pending = Some(ticket.clone());
        Ok(ticket)
    }
    pub fn current(&self, ticket: &Ticket) -> bool {
        self.generation == ticket.generation
            && self
                .pending
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
        self.pending = None;
        match result.and_then(|value| self.accept(ticket, value)) {
            Ok(next) => next,
            Err(error) => {
                self.error = Some(error);
                self.statuses[ticket.lane.index()] = match ticket.request {
                    PairingRequest::ReadBindings => {
                        self.bound.clear();
                        self.scanned.clear();
                        self.statuses = [PairingStatus::Ready; 2];
                        PairingStatus::Ready
                    }
                    PairingRequest::Scan(_) => {
                        if self.dual {
                            self.scanned
                                .retain(|device| device.category() != ticket.lane.category());
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
                self.replacement = None;
                // 4130 pi() still visits the mouse lane after a keyboard scan
                // or bind error. Failure in one category must not strand the other.
                (self.dual
                    && ticket.lane == Lane::Primary
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
                let values = devices(value)?;
                self.bound = values;
                self.scanned.clear();
                self.restore_statuses();
                Ok(Some(PairingRequest::Scan(Lane::Primary)))
            }
            PairingRequest::Scan(lane) => {
                let values = devices(value)?;
                if self.dual
                    && values
                        .iter()
                        .any(|device| device.category() != lane.category())
                {
                    return Err("扫描响应与所请求的设备通道不一致".into());
                }
                if self.dual {
                    self.scanned
                        .retain(|device| device.category() != lane.category());
                } else {
                    self.scanned.clear();
                }
                self.scanned.extend(values);
                self.statuses[lane.index()] = PairingStatus::ScanResults;
                if let Some(target) = self.replacement.take() {
                    if let Some(device) = self
                        .scanned
                        .iter()
                        .find(|device| device.same_unit(&target))
                        .cloned()
                    {
                        return self.pair_request(device, false).map(Some);
                    }
                    self.error =
                        Some("解除原配对后尚未扫描到目标设备，请保持设备唤醒并重试。".into());
                    // Never auto-pair a different unit after reclaiming a named device.
                    return Ok(None);
                }
                let lane_has_binding = self
                    .bound
                    .iter()
                    .any(|device| self.lane_for(device) == *lane);
                if !self.auto_paired[lane.index()] && !lane_has_binding {
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
                if self.dual && *lane == Lane::Primary {
                    return Ok(Some(PairingRequest::Scan(Lane::Secondary)));
                }
                Ok(None)
            }
            PairingRequest::Bind(target) => {
                let device = PairingDevice::parse(
                    value.get("device").cloned().ok_or("配对响应缺少设备信息")?,
                )?;
                if device.product_id() != target.product_id()
                    || device.category() != target.category()
                    || !device.matches_identity(target)
                {
                    return Err("配对响应与所选设备不一致".into());
                }
                self.bound
                    .retain(|previous| previous.category() != device.category());
                self.bound.push(device);
                self.statuses[ticket.lane.index()] = PairingStatus::BindConfirmed;
                self.active_devices[ticket.lane.index()] = None;
                if self.dual && ticket.lane == Lane::Primary {
                    Ok(Some(PairingRequest::Scan(Lane::Secondary)))
                } else {
                    Ok(None)
                }
            }
            PairingRequest::Unbind { device, .. }
            | PairingRequest::UnbindProduct(device)
            | PairingRequest::ReclaimExternal(device) => {
                let id = value.get("productId").ok_or("解除配对响应缺少产品编号")?;
                let expected = ticket.request.payload(self.dual)["productId"].clone();
                if number(Some(id)) != number(Some(&expected)) {
                    return Err("解除配对响应与所选设备不一致".into());
                }
                if self.dual
                    && value
                        .get("category")
                        .and_then(Value::as_str)
                        .is_some_and(|category| category != device.category())
                {
                    return Err("解除配对响应与所选设备通道不一致".into());
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
                self.statuses[ticket.lane.index()] = PairingStatus::Unbound;
                if self.replacement.is_some() {
                    if external {
                        Ok(Some(PairingRequest::Scan(ticket.lane)))
                    } else {
                        let target = self.replacement.take().expect("replacement checked above");
                        self.pair_request(target, false).map(Some)
                    }
                } else {
                    Ok(None)
                }
            }
        }
    }
    pub fn recover(&mut self, generation: u64, lane: Lane, status: PairingStatus) {
        if self.generation != generation
            || self
                .pending
                .as_ref()
                .is_some_and(|ticket| ticket.lane == lane)
            || self.status(lane) != status
        {
            return;
        }
        match status {
            PairingStatus::BindError => {
                self.statuses[lane.index()] = PairingStatus::Ready;
                if self.dual {
                    self.bound
                        .retain(|device| device.category() != lane.category());
                    self.scanned
                        .retain(|device| device.category() != lane.category());
                } else {
                    self.bound.clear();
                    self.scanned.clear();
                }
            }
            PairingStatus::UnbindError => {
                self.statuses[lane.index()] = self.restored_status(lane);
            }
            _ => return,
        }
        if self.pending.is_none() {
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
            // Source Y() resumes scanning after 2 s even without an ack. This
            // starts discovery only: the external binding remains unconfirmed.
            PairingRequest::Scan(ticket.lane)
        } else {
            ticket.request.fallback()?
        };
        self.pending = None;
        Some(next)
    }
    fn restored_status(&self, lane: Lane) -> PairingStatus {
        if self
            .bound
            .iter()
            .any(|device| self.lane_for(device) == lane && !device.disconnected())
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
        self.pending = None;
        self.replacement = None;
        self.active_devices = [None, None];
        self.restore_statuses();
    }
}

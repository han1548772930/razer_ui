//! Current 164/241 DUALLINK UI boundary. No DLL command is issued here.
use super::{Lane, PairingState, Peer, Status};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug)]
pub struct DockPairingEvent {
    session: u64,
    kind: String,
    payload: serde_json::Value,
}
impl DockPairingEvent {
    pub fn session(&self) -> u64 {
        self.session
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn payload(&self) -> &serde_json::Value {
        &self.payload
    }
    fn lane(&self) -> Lane {
        if self.payload["category"] == "KEYBOARD"
            || self.payload["device"]["category"] == "KEYBOARD"
        {
            Lane::Keyboard
        } else {
            Lane::Mouse
        }
    }
}

#[derive(Clone, Debug)]
pub struct DockPairingObservation {
    session: u64,
    kind: String,
    result: Option<Result<serde_json::Value, String>>,
}
impl DockPairingObservation {
    /// Supply only an actual middleware result/error, using its request token.
    pub fn result(
        session: u64,
        kind: impl Into<String>,
        result: Result<serde_json::Value, String>,
    ) -> Self {
        Self {
            session,
            kind: kind.into(),
            result: Some(result),
        }
    }
    /// A publisher's acknowledgement; creating a local intent is not progress.
    pub fn progress(session: u64, kind: impl Into<String>) -> Self {
        Self {
            session,
            kind: kind.into(),
            result: None,
        }
    }
}

fn peer(payload: serde_json::Value, dual: bool) -> Result<Peer, String> {
    let number = |key: &str| payload[key].as_u64().and_then(|v| u32::try_from(v).ok());
    let product_id = number("productId")
        .filter(|id| *id != 0)
        .ok_or("配对结果缺少产品 ID。")?;
    let lane = match payload["category"].as_str() {
        Some("KEYBOARD") => Lane::Keyboard,
        Some("MOUSE") => Lane::Mouse,
        None if !dual => Lane::Mouse,
        _ => return Err("配对结果缺少有效的设备类别。".into()),
    };
    let locale = razer_i18n::locale();
    let name = ["productName", "name"]
        .into_iter()
        .find_map(|field| {
            let names = payload[field].as_object()?;
            names
                .iter()
                .find(|(lang, _)| lang.eq_ignore_ascii_case(&locale))
                .map(|(_, value)| value)
                .or_else(|| names.get("en"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("PID {product_id}"));
    Ok(Peer {
        id: format!(
            "{product_id}:{:?}:{:?}:{}",
            number("editionId"),
            number("layoutId"),
            payload["serialNumber"].as_str().unwrap_or("")
        ),
        name,
        product_id,
        dongle_id: number("dongleId"),
        edition: number("editionId"),
        layout: number("layoutId"),
        lane,
        payload: Some(payload),
    })
}

impl PairingState {
    pub(super) fn request(&mut self, kind: &str, payload: serde_json::Value) -> DockPairingEvent {
        static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);
        let event = DockPairingEvent {
            session: NEXT_OPERATION.fetch_add(1, Ordering::Relaxed),
            kind: kind.into(),
            payload,
        };
        self.pending = Some(event.clone());
        self.active = Some(event.clone());
        event
    }
    pub(super) fn cancel(&mut self) -> DockPairingEvent {
        let event = self.request("DUALLINK_CANCEL", serde_json::json!({}));
        self.pending = None;
        self.active = None;
        event
    }
    pub(super) fn observe(
        &mut self,
        observation: DockPairingObservation,
        dual: bool,
    ) -> (bool, Option<DockPairingEvent>, Option<String>) {
        let Some(active) = self
            .active
            .clone()
            .filter(|event| event.session == observation.session && event.kind == observation.kind)
        else {
            return (false, None, None);
        };
        let lane = active.lane();
        let Some(result) = observation.result else {
            let status = match active.kind.as_str() {
                "DUALLINK_SCAN_DEVICE" => Status::Scanning,
                "DUALLINK_BIND_DEVICE" => Status::Pairing,
                "DUALLINK_UNBIND_DEVICE" => Status::Unpairing,
                _ => return (false, None, None),
            };
            self.pending = None;
            self.channel_mut(lane).status = status;
            return (true, None, None);
        };
        self.pending = None;
        self.active = None;
        let applied = result.and_then(|payload| self.apply_result(&active, payload, dual));
        match applied {
            Ok(event) => (true, event, None),
            Err(error) => {
                match active.kind.as_str() {
                    "DUALLINK_BIND_INFO" => {
                        self.channels = Default::default();
                        for channel in &mut self.channels {
                            channel.status = Status::Ready;
                        }
                    }
                    "DUALLINK_SCAN_DEVICE" => {
                        let channel = self.channel_mut(lane);
                        channel.status = Status::Scanned;
                        channel.candidates.clear();
                        channel.selected = None;
                    }
                    "DUALLINK_BIND_DEVICE" => self.channel_mut(lane).status = Status::PairFailed,
                    "DUALLINK_UNBIND_DEVICE" => {
                        self.channel_mut(lane).status = Status::UnpairFailed
                    }
                    _ => (),
                }
                (true, None, Some(error))
            }
        }
    }
    fn apply_result(
        &mut self,
        request: &DockPairingEvent,
        payload: serde_json::Value,
        dual: bool,
    ) -> Result<Option<DockPairingEvent>, String> {
        let lane = request.lane();
        match request.kind.as_str() {
            "DUALLINK_BIND_INFO" | "DUALLINK_SCAN_DEVICE" => {
                let peers = payload
                    .as_array()
                    .ok_or("配对结果不是设备列表。")?
                    .iter()
                    .cloned()
                    .map(|value| peer(value, dual))
                    .collect::<Result<Vec<_>, _>>()?;
                if request.kind == "DUALLINK_BIND_INFO" {
                    if peers.len() > if dual { 2 } else { 1 }
                        || peers.iter().filter(|p| p.lane == Lane::Keyboard).count() > 1
                        || peers.iter().filter(|p| p.lane == Lane::Mouse).count() > 1
                    {
                        return Err("配对结果包含重复或过多设备。".into());
                    }
                    self.channels = Default::default();
                    for channel in &mut self.channels {
                        channel.status = Status::Ready;
                    }
                    for peer in peers {
                        let channel = self.channel_mut(peer.lane);
                        channel.status = Status::Paired;
                        channel.peer = Some(peer);
                    }
                } else {
                    if peers.iter().any(|peer| peer.lane != lane) {
                        return Err("扫描结果的设备类别与请求不符。".into());
                    }
                    let channel = self.channel_mut(lane);
                    channel.status = Status::Scanned;
                    channel.selected = peers.first().map(|peer| peer.id.clone());
                    channel.candidates = peers;
                    if channel.candidates.len() == 1 {
                        let payload = channel.candidates[0]
                            .payload
                            .clone()
                            .expect("observed candidate");
                        return Ok(Some(self.request(
                            "DUALLINK_BIND_DEVICE",
                            serde_json::json!({"mode":1,"device":payload}),
                        )));
                    }
                }
            }
            "DUALLINK_BIND_DEVICE" => {
                let peer = peer(
                    payload
                        .get("device")
                        .cloned()
                        .ok_or("配对结果缺少设备信息。")?,
                    dual,
                )?;
                if peer.lane != lane {
                    return Err("配对结果的设备类别与请求不符。".into());
                }
                let channel = self.channel_mut(lane);
                channel.status = if dual {
                    Status::JustPaired
                } else {
                    Status::Paired
                };
                channel.peer = Some(peer);
                channel.candidates.clear();
                channel.selected = None;
            }
            "DUALLINK_UNBIND_DEVICE" => {
                let channel = self.channel_mut(lane);
                *channel = Default::default();
                channel.status = Status::Unpaired;
            }
            _ => return Err("未知配对响应。".into()),
        }
        Ok(None)
    }
}

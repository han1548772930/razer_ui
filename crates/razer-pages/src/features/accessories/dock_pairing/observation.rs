//! Current 164/241 DUALLINK UI boundary. No DLL command is issued here.
use super::{
    Lane, PairingState, Peer, Status,
    state::{FailureRecovery, SuccessClose},
};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug)]
pub struct DockPairingEvent {
    session: u64,
    kind: String,
    payload: serde_json::Value,
    lane: Lane,
}
impl DockPairingEvent {
    /// A caller-owned source request. Transport must validate kind and payload.
    pub fn request(kind: impl Into<String>, payload: serde_json::Value) -> Self {
        static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);
        let lane =
            if payload["category"] == "KEYBOARD" || payload["device"]["category"] == "KEYBOARD" {
                Lane::Keyboard
            } else {
                Lane::Mouse
            };
        Self {
            session: NEXT_OPERATION.fetch_add(1, Ordering::Relaxed),
            kind: kind.into(),
            payload,
            lane,
        }
    }
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
        self.lane
    }
}

#[derive(Clone, Debug)]
pub struct DockPairingObservation {
    session: u64,
    kind: String,
    result: Option<Result<serde_json::Value, String>>,
    continuation: bool,
}
impl DockPairingObservation {
    pub fn session(&self) -> u64 {
        self.session
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn result_ref(&self) -> Option<&Result<serde_json::Value, String>> {
        self.result.as_ref()
    }
    pub(super) fn into_result(self) -> Option<Result<serde_json::Value, String>> {
        self.result
    }
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
            continuation: false,
        }
    }
    /// Current192 Te may broadcast its setter error and then payload:{} for
    /// the same request when the recovery getter rejects. Keep that caller
    /// alive until its second source response; this does not retry a command.
    pub fn intermediate_result(
        session: u64,
        kind: impl Into<String>,
        result: Result<serde_json::Value, String>,
    ) -> Self {
        Self {
            session,
            kind: kind.into(),
            result: Some(result),
            continuation: true,
        }
    }
    /// A publisher's acknowledgement of the already presented request progress.
    pub fn progress(session: u64, kind: impl Into<String>) -> Self {
        Self {
            session,
            kind: kind.into(),
            result: None,
            continuation: false,
        }
    }
}

fn peer(payload: serde_json::Value, dual: bool) -> Result<Peer, String> {
    let number = |key: &str| payload[key].as_u64().and_then(|v| u32::try_from(v).ok());
    let product_id = number("productId")
        .filter(|id| !dual || *id != 0)
        .ok_or("配对结果缺少产品 ID。")?;
    let dongle_id = number("dongleId");
    if product_id == 0 && !dongle_id.is_some_and(|id| id > 0 && id < u16::MAX as u32) {
        return Err("未知配对候选缺少真实 radio ID。".into());
    }
    // Current756 useCn's single-device branch presents all scanedInfo in its
    // only status channel. Category still belongs to the untouched payload;
    // only the two-device tool uses it to select a UI column.
    let lane = if dual {
        match payload["category"].as_str() {
            Some("KEYBOARD") => Lane::Keyboard,
            Some("MOUSE") => Lane::Mouse,
            _ => return Err("配对结果缺少有效的设备类别。".into()),
        }
    } else {
        Lane::Mouse
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
            "{product_id}:{dongle_id:?}:{:?}:{:?}:{}",
            number("editionId"),
            number("layoutId"),
            payload["serialNumber"].as_str().unwrap_or("")
        ),
        name,
        product_id,
        dongle_id,
        edition: number("editionId"),
        layout: number("layoutId"),
        lane,
        payload: Some(payload),
    })
}

impl PairingState {
    pub(super) fn request(&mut self, kind: &str, payload: serde_json::Value) -> DockPairingEvent {
        let event = DockPairingEvent::request(kind, payload);
        self.retain_request(event)
    }
    pub(super) fn request_in_lane(
        &mut self,
        kind: &str,
        payload: serde_json::Value,
        lane: Lane,
    ) -> DockPairingEvent {
        let mut event = DockPairingEvent::request(kind, payload);
        event.lane = lane;
        self.retain_request(event)
    }
    fn retain_request(&mut self, event: DockPairingEvent) -> DockPairingEvent {
        self.recovery = None;
        self.success_close = None;
        // Current scan/bind/unPair callers set progress before Qe/ie posts.
        // No binding or peer is changed until its matching result arrives.
        let status = match event.kind() {
            "DUALLINK_SCAN_DEVICE" => Some(Status::Scanning),
            "DUALLINK_BIND_DEVICE" => Some(Status::Pairing),
            "DUALLINK_UNBIND_DEVICE" => Some(Status::Unpairing),
            _ => None,
        };
        if let Some(status) = status {
            self.channel_mut(event.lane()).status = status;
        }
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
        if !observation.continuation {
            self.active = None;
        }
        let applied = result.and_then(|payload| self.apply_result(&active, payload, dual));
        match applied {
            Ok(event) => (true, event, None),
            Err(error) => {
                match active.kind.as_str() {
                    "DUALLINK_BIND_INFO" => {
                        self.scanned.clear();
                        self.channels = Default::default();
                        for channel in &mut self.channels {
                            channel.status = Status::Ready;
                        }
                    }
                    "DUALLINK_SCAN_DEVICE" => {
                        self.scanned.clear();
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
                let status = self.channel(lane).status;
                if matches!(status, Status::PairFailed | Status::UnpairFailed) {
                    self.recovery = Some(FailureRecovery {
                        session: active.session,
                        lane,
                        status,
                    });
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
                    self.scanned.clear();
                    for channel in &mut self.channels {
                        channel.status = Status::Ready;
                    }
                    for peer in peers {
                        let channel = self.channel_mut(peer.lane);
                        channel.status = Status::Paired;
                        channel.peer = Some(peer);
                    }
                } else {
                    // Current 241 ds retains all scanedInfo; as filters it for
                    // each column. Auto-bind requires the FULL response to have
                    // one candidate, as well as matching the active category.
                    let auto_bind = peers.len() == 1 && (!dual || peers[0].lane == lane);
                    self.scanned = peers;
                    for candidate_lane in [Lane::Keyboard, Lane::Mouse] {
                        let candidates: Vec<_> = self
                            .scanned
                            .iter()
                            .filter(|peer| !dual || peer.lane == candidate_lane)
                            .cloned()
                            .collect();
                        let channel = self.channel_mut(candidate_lane);
                        channel.selected = candidates.first().map(|peer| peer.id.clone());
                        channel.candidates = candidates;
                    }
                    let channel = self.channel_mut(lane);
                    channel.status = Status::Scanned;
                    if auto_bind {
                        let payload = channel.candidates[0]
                            .payload
                            .clone()
                            .expect("observed candidate");
                        return Ok(Some(self.request_in_lane(
                            "DUALLINK_BIND_DEVICE",
                            serde_json::json!({"mode":1,"device":payload}),
                            lane,
                        )));
                    }
                }
            }
            "DUALLINK_BIND_DEVICE" => {
                if payload["hardware_confirmed"] == true && payload["metadata_complete"] == false {
                    return Err(format!(
                        "硬件已确认配对，设备信息读取失败：{}",
                        payload["metadata_error"]
                            .as_str()
                            .unwrap_or("配对后的设备信息未完整读取")
                    ));
                }
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
                // ds tests the reducer's existing statuses before dispatching
                // JUST_PAIRED. Keep that source condition, including its dual
                // lane behavior; the single-device utility always closes.
                let close = !dual
                    || self.channels.iter().all(|channel| {
                        matches!(channel.status, Status::Paired | Status::JustPaired)
                    });
                let channel = self.channel_mut(lane);
                channel.status = if dual { Status::JustPaired } else { Status::Paired };
                channel.peer = Some(peer);
                channel.candidates.clear();
                channel.selected = None;
                if close {
                    self.success_close = Some(SuccessClose {
                        session: request.session,
                        statuses: self.channels.each_ref().map(|channel| channel.status),
                    });
                }
            }
            "DUALLINK_UNBIND_DEVICE" => {
                let channel = self.channel_mut(lane);
                *channel = Default::default();
                channel.status = Status::Unpaired;
                // 164 closes after unbind; 241 stays open for another pairing.
                if !dual {
                    self.success_close = Some(SuccessClose {
                        session: request.session,
                        statuses: self.channels.each_ref().map(|channel| channel.status),
                    });
                }
            }
            _ => return Err("未知配对响应。".into()),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Current 241 module3746 ds/as: full scan length gates auto-bind;
    // per-column lists are filtered afterward. Fixtures exercise mixed scans.
    fn keyboard() -> serde_json::Value {
        json!({"productId":716,"dongleId":713,"category":"KEYBOARD","editionId":128,"productName":{"en":"Keyboard"}})
    }
    fn mouse() -> serde_json::Value {
        json!({"productId":182,"dongleId":183,"category":"MOUSE","editionId":0,"productName":{"en":"Mouse"}})
    }

    fn unknown(dongle_id: u16) -> serde_json::Value {
        json!({"productId":0,"dongleId":dongle_id,"category":"","editionId":129,"layoutId":0,"productName":{"en":format!("unknown-{dongle_id}")}})
    }

    #[::core::prelude::v1::test]
    fn confirmed_pair_with_failed_metadata_does_not_publish_peer_or_close_success() {
        let mut state = PairingState::default();
        let bind = state.request("DUALLINK_BIND_DEVICE", json!({"device":mouse()}));
        let (accepted, next, error) = state.observe(
            DockPairingObservation::result(
                bind.session(),
                bind.kind(),
                Ok(json!({"hardware_confirmed":true,
                "metadata_complete":false,"metadata_error":"getEdition failed","device":null})),
            ),
            true,
        );
        assert!(accepted && next.is_none());
        assert_eq!(
            error.as_deref(),
            Some("硬件已确认配对，设备信息读取失败：getEdition failed")
        );
        assert!(state.channel(Lane::Mouse).peer.is_none());
        assert!(state.success_close.is_none());
        assert_eq!(state.channel(Lane::Mouse).status, Status::PairFailed);
    }

    #[::core::prelude::v1::test]
    fn source192_failed_recovery_query_keeps_both_source_responses_for_same_request() {
        let mut state = PairingState::default();
        let read = state.request("DUALLINK_BIND_INFO", json!({}));
        state.observe(
            DockPairingObservation::result(read.session(), read.kind(), Ok(json!([mouse()]))),
            true,
        );
        let unbind = state.request(
            "DUALLINK_UNBIND_DEVICE",
            state.unpair_payload(Lane::Mouse, true).unwrap(),
        );
        let (accepted, _, error) = state.observe(
            DockPairingObservation::intermediate_result(
                unbind.session(),
                unbind.kind(),
                Err("original setter rejection".into()),
            ),
            true,
        );
        assert!(accepted && error.is_some());
        assert_eq!(state.channel(Lane::Mouse).status, Status::UnpairFailed);
        assert!(state.active.is_some());
        let (accepted, _, error) = state.observe(
            DockPairingObservation::result(unbind.session(), unbind.kind(), Ok(json!({}))),
            true,
        );
        assert!(accepted && error.is_none());
        assert_eq!(state.channel(Lane::Mouse).status, Status::Unpaired);
        assert!(state.active.is_none());
        let (accepted, _, _) = state.observe(
            DockPairingObservation::result(unbind.session(), unbind.kind(), Ok(json!({}))),
            true,
        );
        assert!(!accepted);
    }

    #[::core::prelude::v1::test]
    fn source756_unknown_single_scan_auto_binds_its_unmodified_radio_payload() {
        let mut state = PairingState::default();
        let scan = state.request(
            "DUALLINK_SCAN_DEVICE",
            PairingState::scan_payload(Lane::Mouse, false),
        );
        let raw = unknown(60000);
        let (accepted, next, error) = state.observe(
            DockPairingObservation::result(scan.session(), scan.kind(), Ok(json!([raw.clone()]))),
            false,
        );
        assert!(accepted && error.is_none());
        let bind = next.expect("756 preserves and auto-binds a real unknown singleton");
        assert_eq!(bind.payload()["device"], raw);
        assert_eq!(state.channel(Lane::Mouse).status, Status::Pairing);
        assert!(state.channel(Lane::Mouse).peer.is_none());
        assert!(
            state
                .observe(
                    DockPairingObservation::progress(bind.session(), bind.kind()),
                    false
                )
                .0
        );
        // The catalog-miss source callback supplies no result. Progress remains
        // pending until the user cancels; a Rust diagnostic cannot create a peer.
        assert_eq!(state.active.as_ref().unwrap().session(), bind.session());
        let canceled = state.cancel();
        assert_eq!(canceled.kind(), "DUALLINK_CANCEL");
        assert!(
            !state
                .observe(
                    DockPairingObservation::result(
                        bind.session(),
                        bind.kind(),
                        Ok(json!({"device":raw}))
                    ),
                    false
                )
                .0
        );
        assert!(state.channel(Lane::Mouse).peer.is_none());
    }

    #[::core::prelude::v1::test]
    fn source756_multiple_unknown_radios_keep_independent_selection_ids() {
        let mut state = PairingState::default();
        let scan = state.request(
            "DUALLINK_SCAN_DEVICE",
            PairingState::scan_payload(Lane::Mouse, false),
        );
        let (accepted, next, error) = state.observe(
            DockPairingObservation::result(
                scan.session(),
                scan.kind(),
                Ok(json!([unknown(60000), unknown(60001)])),
            ),
            false,
        );
        assert!(accepted && next.is_none() && error.is_none());
        let candidates = &state.channel(Lane::Mouse).candidates;
        assert_eq!(candidates.len(), 2);
        assert_ne!(candidates[0].id, candidates[1].id);
        let second = candidates[1].id.clone();
        state.channel_mut(Lane::Mouse).selected = Some(second);
        assert_eq!(state.selected(Lane::Mouse).unwrap().dongle_id, Some(60001));
    }

    #[::core::prelude::v1::test]
    fn source756_single_tool_keeps_catalog_categories_in_the_only_ui_channel() {
        for raw in [
            keyboard(),
            json!({"productId":0,"dongleId":241,"category":"ACCESSORY","editionId":0}),
        ] {
            let mut state = PairingState::default();
            let scan = state.request(
                "DUALLINK_SCAN_DEVICE",
                PairingState::scan_payload(Lane::Mouse, false),
            );
            let (_, next, error) = state.observe(
                DockPairingObservation::result(
                    scan.session(),
                    scan.kind(),
                    Ok(json!([raw.clone()])),
                ),
                false,
            );
            assert!(error.is_none());
            let bind = next.unwrap();
            assert_eq!(bind.payload()["device"], raw);
            assert_eq!(bind.lane(), Lane::Mouse);
            assert_eq!(state.channel(Lane::Mouse).status, Status::Pairing);
            assert_eq!(state.channel(Lane::Keyboard).status, Status::Loading);
            let device = json!({"productId":0,"dongleId":bind.payload()["device"]["dongleId"],"category":"MOUSE","editionId":0,"layoutId":0});
            let (_, _, error) = state.observe(
                DockPairingObservation::result(
                    bind.session(),
                    bind.kind(),
                    Ok(json!({"device":device})),
                ),
                false,
            );
            assert!(error.is_none());
            assert_eq!(state.channel(Lane::Mouse).status, Status::Paired);
        }
    }

    #[::core::prelude::v1::test]
    fn source756_unregistered_pair_product_zero_is_accepted_only_for_single_valid_radio() {
        let mut state = PairingState::default();
        let raw = json!({"productId":181,"dongleId":180,"category":"MOUSE","editionId":128});
        let bind = state.request("DUALLINK_BIND_DEVICE", json!({"mode":1,"device":raw}));
        let device =
            json!({"productId":0,"dongleId":180,"category":"MOUSE","editionId":5,"layoutId":6});
        let (accepted, next, error) = state.observe(
            DockPairingObservation::result(
                bind.session(),
                bind.kind(),
                Ok(json!({"device":device.clone()})),
            ),
            false,
        );
        assert!(accepted && next.is_none() && error.is_none());
        let paired = state.channel(Lane::Mouse).peer.as_ref().unwrap();
        assert_eq!(paired.product_id, 0);
        assert_eq!(paired.dongle_id, Some(180));
        assert_eq!(paired.edition, Some(5));
        assert_eq!(paired.layout, Some(6));
        assert!(state.success_close.is_some());
        assert!(peer(device, true).is_err());
        assert!(peer(json!({"productId":0,"category":""}), false).is_err());
        assert!(peer(json!({"productId":0,"dongleId":65535,"category":""}), false).is_err());
        assert!(peer(json!({"productId":182,"dongleId":183,"category":""}), true).is_err());
    }
    #[::core::prelude::v1::test]
    fn mixed_scan_does_not_auto_pair_a_filtered_singleton() {
        let mut state = PairingState::default();
        let request = state.request(
            "DUALLINK_SCAN_DEVICE",
            PairingState::scan_payload(Lane::Keyboard, true),
        );
        let (accepted, next, error) = state.observe(
            DockPairingObservation::result(
                request.session(),
                request.kind(),
                Ok(json!([keyboard(), mouse()])),
            ),
            true,
        );
        assert!(accepted);
        assert!(error.is_none());
        assert!(next.is_none());
        assert_eq!(state.scanned.len(), 2);
        assert_eq!(
            state.channel(Lane::Keyboard).candidates[0].dongle_id,
            Some(713)
        );
        assert_eq!(
            state.channel(Lane::Mouse).candidates[0].dongle_id,
            Some(183)
        );
    }
    #[::core::prelude::v1::test]
    fn opposite_category_is_an_empty_lane_not_a_failed_scan() {
        let mut state = PairingState::default();
        let request = state.request(
            "DUALLINK_SCAN_DEVICE",
            PairingState::scan_payload(Lane::Keyboard, true),
        );
        let (_, next, error) = state.observe(
            DockPairingObservation::result(request.session(), request.kind(), Ok(json!([mouse()]))),
            true,
        );
        assert!(next.is_none());
        assert!(error.is_none());
        assert!(state.channel(Lane::Keyboard).candidates.is_empty());
        assert_eq!(state.channel(Lane::Keyboard).status, Status::Scanned);
        assert_eq!(state.scanned.len(), 1);
    }
    #[::core::prelude::v1::test]
    fn one_matching_candidate_submits_its_raw_dongle_and_canceled_reply_is_ignored() {
        let mut state = PairingState::default();
        let request = state.request(
            "DUALLINK_SCAN_DEVICE",
            PairingState::scan_payload(Lane::Keyboard, true),
        );
        let (_, next, error) = state.observe(
            DockPairingObservation::result(
                request.session(),
                request.kind(),
                Ok(json!([keyboard()])),
            ),
            true,
        );
        assert!(error.is_none());
        let pair = next.expect("source singleton creates bind intent");
        assert_eq!(state.channel(Lane::Keyboard).status, Status::Pairing);
        assert!(state.channel(Lane::Keyboard).peer.is_none());
        assert_eq!(pair.kind(), "DUALLINK_BIND_DEVICE");
        assert_eq!(pair.payload()["device"]["dongleId"], 713);
        assert_eq!(pair.payload()["device"]["productId"], 716);
        state.cancel();
        let (accepted, _, _) = state.observe(
            DockPairingObservation::result(
                pair.session(),
                pair.kind(),
                Ok(json!({"device":keyboard()})),
            ),
            true,
        );
        assert!(!accepted);
        assert!(state.channel(Lane::Keyboard).peer.is_none());
    }

    #[::core::prelude::v1::test]
    fn request_progress_and_failed_unbind_keep_the_observed_peer() {
        let mut state = PairingState::default();
        let read = state.request("DUALLINK_BIND_INFO", json!({}));
        state.observe(
            DockPairingObservation::result(read.session(), read.kind(), Ok(json!([mouse()]))),
            true,
        );
        let peer = state.channel(Lane::Mouse).peer.clone().unwrap();
        let unbind = state.request(
            "DUALLINK_UNBIND_DEVICE",
            state.unpair_payload(Lane::Mouse, true).unwrap(),
        );
        assert_eq!(state.channel(Lane::Mouse).status, Status::Unpairing);
        assert_eq!(
            state.channel(Lane::Mouse).peer.as_ref().unwrap().id,
            peer.id
        );
        state.observe(
            DockPairingObservation::result(unbind.session(), unbind.kind(), Err("failure".into())),
            true,
        );
        let ticket = state.recovery.unwrap();
        assert_eq!(ticket.delay(), std::time::Duration::from_secs(4));
        assert!(state.recover_failure(ticket));
        assert_eq!(state.channel(Lane::Mouse).status, Status::Paired);
        assert_eq!(
            state.channel(Lane::Mouse).peer.as_ref().unwrap().id,
            peer.id
        );
        assert!(!state.recover_failure(ticket));
    }

    #[::core::prelude::v1::test]
    fn bind_failure_resets_both_source_lanes_and_old_timer_cannot_reset_retry() {
        let mut state = PairingState::default();
        let read = state.request("DUALLINK_BIND_INFO", json!({}));
        state.observe(
            DockPairingObservation::result(read.session(), read.kind(), Ok(json!([mouse()]))),
            true,
        );
        let bind = state.request(
            "DUALLINK_BIND_DEVICE",
            json!({"mode":1,"device":keyboard()}),
        );
        assert_eq!(state.channel(Lane::Keyboard).status, Status::Pairing);
        assert!(state.channel(Lane::Keyboard).peer.is_none());
        state.observe(
            DockPairingObservation::result(bind.session(), bind.kind(), Err("failure".into())),
            true,
        );
        let ticket = state.recovery.unwrap();
        let mut recovered = state.clone();
        assert!(recovered.recover_failure(ticket));
        assert!(recovered.peers().is_empty());
        assert!(
            recovered
                .channels
                .iter()
                .all(|channel| channel.status == Status::Ready)
        );
        state.request(
            "DUALLINK_SCAN_DEVICE",
            PairingState::scan_payload(Lane::Keyboard, true),
        );
        assert!(!state.recover_failure(ticket));
        assert!(state.channel(Lane::Mouse).peer.is_some());
        assert_eq!(state.channel(Lane::Keyboard).status, Status::Scanning);
    }

    #[::core::prelude::v1::test]
    fn single_utility_closes_only_after_result_and_dual_unbind_stays_open() {
        let mut state = PairingState::default();
        let bind = state.request("DUALLINK_BIND_DEVICE", json!({"mode":1,"device":mouse()}));
        assert!(state.success_close.is_none());
        state.observe(
            DockPairingObservation::result(
                bind.session(),
                bind.kind(),
                Ok(json!({"device":mouse()})),
            ),
            false,
        );
        let ticket = state.success_close.unwrap();
        assert_eq!(ticket.delay(), std::time::Duration::from_secs(1));
        assert!(state.can_close_success(ticket));
        state.cancel();
        assert!(!state.can_close_success(ticket));
        let unbind = state.request(
            "DUALLINK_UNBIND_DEVICE",
            state.unpair_payload(Lane::Mouse, true).unwrap(),
        );
        state.observe(
            DockPairingObservation::result(
                unbind.session(),
                unbind.kind(),
                Ok(json!({"productId":183})),
            ),
            true,
        );
        assert_eq!(state.channel(Lane::Mouse).status, Status::Unpaired);
        assert!(state.success_close.is_none());
    }
}

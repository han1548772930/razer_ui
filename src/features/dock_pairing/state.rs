//! Runtime-only pairing state. Requests never manufacture a service response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lane {
    Keyboard,
    Mouse,
}
impl Lane {
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Keyboard => "KEYBOARD",
            Self::Mouse => "MOUSE",
        }
    }
    pub(super) fn ix(self) -> usize {
        match self {
            Self::Keyboard => 0,
            Self::Mouse => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Status {
    #[default]
    Loading,
    Ready,
    Scanning,
    Scanned,
    Pairing,
    Paired,
    PairFailed,
    ConfirmUnpair,
    Unpairing,
    Unpaired,
    UnpairFailed,
    JustPaired,
}
#[derive(Clone, Debug)]
pub(super) struct Peer {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) product_id: u32,
    pub(super) dongle_id: Option<u32>,
    pub(super) edition: u32,
    pub(super) layout: u32,
    pub(super) lane: Lane,
}
#[derive(Clone, Default)]
pub(super) struct Channel {
    pub(super) status: Status,
    pub(super) peer: Option<Peer>,
    pub(super) candidates: Vec<Peer>,
    pub(super) selected: Option<String>,
}
impl Channel {
    fn blocks_other(&self) -> bool {
        matches!(
            self.status,
            Status::Scanning | Status::Pairing | Status::Unpairing
        ) || self.status == Status::Scanned && !self.candidates.is_empty()
    }
}
#[derive(Clone, Default)]
pub(super) struct PairingState {
    pub(super) channels: [Channel; 2],
}
impl PairingState {
    pub(super) fn channel(&self, lane: Lane) -> &Channel {
        &self.channels[lane.ix()]
    }
    pub(super) fn channel_mut(&mut self, lane: Lane) -> &mut Channel {
        &mut self.channels[lane.ix()]
    }
    pub(super) fn modifiable(&self, lane: Lane, dual: bool) -> bool {
        !dual || !self.channels[1 - lane.ix()].blocks_other()
    }
    pub(super) fn peers(&self) -> Vec<&Peer> {
        self.channels
            .iter()
            .filter_map(|c| c.peer.as_ref())
            .collect()
    }
    pub(super) fn selected(&self, lane: Lane) -> Option<&Peer> {
        let channel = self.channel(lane);
        channel
            .candidates
            .iter()
            .find(|peer| Some(&peer.id) == channel.selected.as_ref() && peer.lane == lane)
    }
    pub(super) fn scan_payload(lane: Lane, dual: bool) -> serde_json::Value {
        if dual {
            serde_json::json!({"status":1,"category":lane.key()})
        } else {
            serde_json::json!({"status":1})
        }
    }
    pub(super) fn unpair_payload(&self, lane: Lane, dual: bool) -> Option<serde_json::Value> {
        let peer = self.channel(lane).peer.as_ref()?;
        let mut payload =
            serde_json::json!({"productId":peer.dongle_id.unwrap_or(peer.product_id)});
        if dual {
            payload["category"] = serde_json::json!(lane.key());
        }
        Some(payload)
    }
}

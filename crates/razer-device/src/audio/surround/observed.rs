//! Service-owned observations; no stored preference is substituted for a missing state.
use super::{Product, protocol::*, wire::Payload};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Observations {
    pub manager: Option<WasapiManagerState>,
    pub streams: Option<StreamStates>,
    pub channel_maps: Option<ChannelMapsState>,
    pub equalizer: Option<EqState>,
    pub delay: Option<DelayState>,
    pub endpoints: Option<EndpointsState>,
    pub volumes: BTreeMap<String, EndpointVolumeState>,
    pub front_ctc: Option<FrontCtcState>,
    pub side_ctc: Option<SideCtcState>,
    pub rear_ctc: Option<RearCtcState>,
    pub front_virtualizer: Option<FrontVirtualizerState>,
    pub side_virtualizer: Option<SideVirtualizerState>,
    pub rear_virtualizer: Option<RearVirtualizerState>,
    pub spatial: Option<SpatialGlobalSettingsState>,
    pub bass: Option<BassManagementState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Update { Changed, Duplicate, Stale }

fn commit<T: PartialEq>(slot: &mut Option<T>, incoming: T, header: impl Fn(&T) -> Option<&StateHeader>) -> Result<Update> {
    let revision = header(&incoming).context("Incoming state header is absent")?.revision;
    if let Some(current) = slot {
        let previous = header(current).context("Retained state header is absent")?.revision;
        if revision < previous { return Ok(Update::Stale); }
        if revision == previous {
            ensure!(*current == incoming, "Routing state changed without a revision change");
            return Ok(Update::Duplicate);
        }
    }
    *slot = Some(incoming);
    Ok(Update::Changed)
}

impl Observations {
    pub fn apply(&mut self, payload: Payload) -> Result<Update> {
        payload.validate()?;
        match payload {
            Payload::WasapiManagerState(v) => commit(&mut self.manager, v, |v| v.state_header.as_ref()),
            Payload::StreamStates(v) => commit(&mut self.streams, v, |v| v.state_header.as_ref()),
            Payload::ChannelMapsState(v) => commit(&mut self.channel_maps, v, |v| v.state_header.as_ref()),
            Payload::EqState(v) => commit(&mut self.equalizer, v, |v| v.state_header.as_ref()),
            Payload::DelayState(v) => commit(&mut self.delay, v, |v| v.state_header.as_ref()),
            Payload::EndpointsState(v) => commit(&mut self.endpoints, v, |v| v.state_header.as_ref()),
            Payload::EndpointVolumeState(v) => {
                let id = v.state_header.as_ref().context("Volume header is absent")?.id.clone();
                let mut slot = self.volumes.get(&id).cloned();
                let update = commit(&mut slot, v, |v| v.state_header.as_ref())?;
                if let Some(state) = slot { self.volumes.insert(id, state); }
                Ok(update)
            }
            Payload::FrontCtcState(v) => commit(&mut self.front_ctc, v, |v| v.state_header.as_ref()),
            Payload::SideCtcState(v) => commit(&mut self.side_ctc, v, |v| v.state_header.as_ref()),
            Payload::RearCtcState(v) => commit(&mut self.rear_ctc, v, |v| v.state_header.as_ref()),
            Payload::FrontVirtualizerState(v) => commit(&mut self.front_virtualizer, v, |v| v.state_header.as_ref()),
            Payload::SideVirtualizerState(v) => commit(&mut self.side_virtualizer, v, |v| v.state_header.as_ref()),
            Payload::RearVirtualizerState(v) => commit(&mut self.rear_virtualizer, v, |v| v.state_header.as_ref()),
            Payload::SpatialGlobalSettingsState(v) => commit(&mut self.spatial, v, |v| v.state_header.as_ref()),
            Payload::BassManagementState(v) => commit(&mut self.bass, v, |v| v.state_header.as_ref()),
            _ => anyhow::bail!("Routing message is not a state observation"),
        }
    }
    pub fn ready(&self, product: Product) -> bool {
        let base = self.manager.is_some() && self.streams.is_some() && self.channel_maps.is_some()
            && self.equalizer.is_some() && self.delay.is_some() && self.endpoints.is_some();
        base && (product == Product::Clio || (self.front_ctc.is_some() && self.side_ctc.is_some()
            && self.rear_ctc.is_some() && self.front_virtualizer.is_some() && self.side_virtualizer.is_some()
            && self.rear_virtualizer.is_some() && self.spatial.is_some() && self.bass.is_some()))
    }
    pub fn endpoint(&self, id: &str) -> Option<&EndpointProperties> {
        self.endpoints.as_ref()?.endpoints_properties.iter().find(|e| e.endpoint_id == id)
    }
    pub fn endpoint_with_role(&self, role: u32) -> Result<&EndpointProperties> {
        let mut found = self.endpoints.as_ref().context("Routing endpoints are unobserved")?
            .endpoints_properties.iter().filter(|e| e.thx_endpoint_id == role);
        let endpoint = found.next().context("Routing endpoint role is not present")?;
        ensure!(found.next().is_none(), "Routing endpoint role is ambiguous");
        Ok(endpoint)
    }
}

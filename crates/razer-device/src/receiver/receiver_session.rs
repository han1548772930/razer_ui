//! A current DualLink caller retains its own namespace on a physical master.
//! The adapter owns one handle and lock across both peer observations and I/O.
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    receiver_capabilities::{self, ReceiverCapability},
};
use anyhow::{Context as _, ensure};
use serde::Serialize;

pub struct ReceiverSession {
    transport: DeviceReadCapability,
    receiver: ReceiverCapability,
    peer_product_id: u32,
    peer_alias: Option<u32>,
}

#[derive(Serialize)]
#[non_exhaustive]
pub struct Observation<T> {
    pub reading: T,
    pub receiver_before: Vec<(u16, u8)>,
    pub receiver_after: Vec<(u16, u8)>,
}

impl ReceiverSession {
    pub fn mouse(
        product_id: u32,
        physical_product_id: u32,
        peer_product_id: u32,
    ) -> anyhow::Result<Self> {
        let source = crate::device_reads::capability(product_id)
            .context("Mouse source transport is absent")?;
        Self::mouse_with_source(source, physical_product_id, peer_product_id)
    }

    pub(crate) fn mouse_with_source(
        source: &DeviceReadCapability,
        physical_product_id: u32,
        peer_product_id: u32,
    ) -> anyhow::Result<Self> {
        ensure!(
            source
                .physical_transports_by_pid
                .get(&peer_product_id.to_string())
                .is_some_and(
                    |branch| branch.is_duallink && branch.source_class == "rzDevice25DualLinkMouse"
                ),
            "Mouse peer is not a current radio factory identity"
        );
        let receiver = receiver_capabilities::capability(u16::try_from(physical_product_id)?)
            .context("Physical master source is absent")?;
        Self::bind(
            source.for_duallink_master(physical_product_id, receiver.claim_interface)?,
            receiver,
            peer_product_id,
        )
    }

    pub fn keyboard(
        product_id: u32,
        physical_product_id: u32,
        peer_product_id: u32,
    ) -> anyhow::Result<Self> {
        let settings = crate::keyboard_settings::capability(product_id)
            .context("Keyboard brightness source is absent")?;
        // These rows capture the complete mounted DualLink factory, including
        // 756's interface 3 sender (the metadata helper's selector is separate).
        let source = receiver_capabilities::all()
            .iter()
            .find(|row| {
                u32::from(row.source_product_id) == product_id
                    && row.source_class == "rzDevice25DualLinkKeyboard"
            })
            .context("Keyboard has no current DualLink factory")?;
        ensure!(
            peer_product_id == u32::from(source.product_id)
                || (source.peer_match_product_id && peer_product_id == product_id),
            "Keyboard peer differs from its current factory"
        );
        let receiver = receiver_capabilities::capability(u16::try_from(physical_product_id)?)
            .context("Physical master source is absent")?;
        let mut transport = settings.transport.clone();
        transport.source_class = source.source_class.clone();
        transport.direct_pids = vec![physical_product_id];
        transport.claim_interface = receiver.claim_interface;
        transport.report_id = receiver.claim_interface;
        transport.transaction_prefix = source.transaction_prefix;
        transport.transaction_modulus = source.transaction_modulus;
        transport.max_retry_in = source.max_retry_in;
        transport.max_retry_out = source.max_retry_out;
        transport.sleep_between_out_ms = source.sleep_between_out_ms;
        transport.sleep_between_out_in_ms = source.sleep_between_out_in_ms;
        transport.sleep_between_in_ms = source.sleep_between_in_ms;
        let mut session = Self::bind(transport, receiver, peer_product_id)?;
        if source.peer_match_product_id {
            session.peer_alias = Some(if peer_product_id == product_id {
                u32::from(source.product_id)
            } else {
                product_id
            });
        }
        Ok(session)
    }

    fn bind(
        transport: DeviceReadCapability,
        physical: &ReceiverCapability,
        peer_product_id: u32,
    ) -> anyhow::Result<Self> {
        ensure!(
            physical.vendor_id == transport.vendor_id
                && physical.report_bytes == transport.report_bytes
                && physical.command == [80, 0, 191]
                && matches!(
                    physical.source_class.as_str(),
                    "rzDevice25DualLinkKeyboard"
                        | "rzDevice25DualLinkMouse"
                        | "rzDevice25Linker"
                        | "rzDevice25LinkerMultiDevices"
                        | "rzDevice25LinkerUma"
                ),
            "Master query layout differs from the source caller"
        );
        let mut receiver = physical.clone();
        receiver.source_product_id = u16::try_from(transport.product_id)?;
        receiver.source_class = transport.source_class.clone();
        receiver.report_id = transport.report_id;
        receiver.transaction_prefix = transport.transaction_prefix;
        receiver.transaction_modulus = transport.transaction_modulus;
        receiver.max_retry_in = transport.max_retry_in;
        receiver.max_retry_out = transport.max_retry_out;
        receiver.sleep_between_out_ms = transport.sleep_between_out_ms;
        receiver.sleep_between_out_in_ms = transport.sleep_between_out_in_ms;
        receiver.sleep_between_in_ms = transport.sleep_between_in_ms;
        receiver.startup_retry = None;
        Ok(Self {
            transport,
            receiver,
            peer_product_id,
            peer_alias: None,
        })
    }

    pub fn transport(&self) -> &DeviceReadCapability {
        &self.transport
    }

    pub fn validate_peer(&self, peers: &[(u16, u8)]) -> anyhow::Result<()> {
        ensure!(
            self.peer_alias
                .is_none_or(|alias| !peers.iter().any(|(pid, _)| u32::from(*pid) == alias)),
            "Receiver reported both identities of the same logical keyboard"
        );
        let mut matches = peers
            .iter()
            .filter(|(pid, _)| u32::from(*pid) == self.peer_product_id);
        let peer = matches
            .next()
            .context("Receiver did not report the requested peer")?;
        ensure!(
            peer.1 == 1 && matches.next().is_none(),
            "Receiver peer is offline or ambiguous"
        );
        Ok(())
    }

    pub fn observe<T, N, V>(
        &self,
        device: &dyn FeatureTransport,
        mut next: N,
        validate: V,
        operation: impl FnOnce(&mut N, &V) -> anyhow::Result<T>,
    ) -> anyhow::Result<Observation<T>>
    where
        N: FnMut() -> anyhow::Result<u8>,
        V: Fn() -> anyhow::Result<()>,
    {
        validate()?;
        let before =
            crate::device_query::read_receiver(device, &self.receiver, next()?, &validate)?;
        self.validate_peer(&before)?;
        validate()?;
        let reading = operation(&mut next, &validate)?;
        validate()?;
        let after = crate::device_query::read_receiver(device, &self.receiver, next()?, &validate)?;
        self.validate_peer(&after)?;
        validate()?;
        Ok(Observation {
            reading,
            receiver_before: before,
            receiver_after: after,
        })
    }
}

#[cfg(test)]
#[path = "receiver_session_tests.rs"]
mod tests;

/// Only a source-proved radio sender bound to a known master may bypass the
/// ordinary physical factory selector. No descriptor or arbitrary PID grants it.
pub(crate) fn is_bound_master(transport: &DeviceReadCapability, keyboard: bool) -> bool {
    let [physical] = transport.direct_pids.as_slice() else {
        return false;
    };
    let candidates: Vec<u32> = if keyboard {
        receiver_capabilities::all()
            .iter()
            .filter(|row| {
                u32::from(row.source_product_id) == transport.product_id
                    && row.source_class == "rzDevice25DualLinkKeyboard"
            })
            .map(|row| u32::from(row.product_id))
            .collect()
    } else {
        crate::device_reads::capability(transport.product_id)
            .map(|source| {
                source
                    .physical_transports_by_pid
                    .iter()
                    .filter(|(_, branch)| branch.is_duallink)
                    .filter_map(|(pid, _)| pid.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    };
    candidates.into_iter().any(|peer| {
        let session = if keyboard {
            ReceiverSession::keyboard(transport.product_id, *physical, peer)
        } else {
            ReceiverSession::mouse(transport.product_id, *physical, peer)
        };
        session.is_ok_and(|session| {
            let source = session.transport();
            transport.source_class == source.source_class
                && transport.vendor_id == source.vendor_id
                && transport.report_bytes == source.report_bytes
                && transport.claim_interface == source.claim_interface
                && transport.report_id == source.report_id
                && transport.transaction_prefix == source.transaction_prefix
                && transport.transaction_modulus == source.transaction_modulus
                && transport.claim_interfaces_by_pid.is_empty()
                && transport.physical_transports_by_pid.is_empty()
        })
    })
}

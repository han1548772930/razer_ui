//! PID182's current basic getters and Ne -> Ze/QC polling receiver chain.
//! A physical master is not the mouse's logical dongle identity. The mouse
//! caller retains transaction namespace 0 even on a Linker master (namespace
//! 0xe0), and chooses high-speed polling only for source UMA/ZIA masters.
use crate::{
    backend::FeatureTransport,
    device_query,
    device_reads::{self, DeviceReadCapability, DeviceReadKind, DeviceReadValue, ReadCommand},
    receiver_capabilities::{self, ReceiverCapability},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

pub const PRODUCT_ID: u32 = 182;
pub const PEER_PRODUCT_ID: u32 = 183;

pub struct ReceiverRead {
    transport: DeviceReadCapability,
    receiver: ReceiverCapability,
    command: ReadCommand,
}

#[derive(Serialize)]
pub struct Observation<T = DeviceReadValue> {
    pub reading: T,
    pub receiver_before: Vec<(u16, u8)>,
    pub receiver_after: Vec<(u16, u8)>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PollingWriteResult {
    pub requested_hz: u32,
    pub effective_hz: u32,
    pub previous_hz: u32,
    pub observed_hz: u32,
    pub acknowledgement: Option<Vec<u8>>,
    pub changed: bool,
}

impl ReceiverRead {
    pub fn new(
        product_id: u32,
        physical_product_id: u32,
        peer_product_id: Option<u32>,
        kind: DeviceReadKind,
    ) -> anyhow::Result<Self> {
        ensure!(
            product_id == PRODUCT_ID && peer_product_id == Some(PEER_PRODUCT_ID),
            "Receiver read requires the current PID182/peer183 caller"
        );
        let mut receiver = receiver_capabilities::capability(u16::try_from(physical_product_id)?)
            .context("Physical master has no current receiver capability")?
            .clone();
        let mut transport = device_reads::capability(PRODUCT_ID)
            .context("PID182 source transport is absent")?
            .clone();
        ensure!(
            receiver.vendor_id == transport.vendor_id && receiver.report_bytes == 91,
            "Receiver transport differs from the current mouse factory"
        );
        // Factory $ selects e.master.dongleId and its claimInterface. He then
        // restores rzDevice.productId=183 while device.productId stays physical.
        // Both getMultipleDeviceWirelessConnectionStatusV2 and the polling GET
        // use the inherited DualLinkMouse sender, not the master's own sender.
        transport.claim_interface = receiver.claim_interface;
        transport.direct_pids = vec![physical_product_id];
        transport.claim_interfaces_by_pid.clear();
        transport.physical_transports_by_pid.clear();
        // UsbRzDeviceAction prepends claimInterface when this caller omits a
        // reportId in its IPC payload. This can be nonzero for a shared master.
        transport.report_id = receiver.claim_interface;
        receiver.source_product_id = PRODUCT_ID as u16;
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

        let mut command = transport
            .queries
            .iter()
            .find(|command| command.name == kind)
            .context("PID182 requested getter is absent")?
            .clone();
        // Current highSpeedPollingRate.yes has no isHyperpollingDevice or
        // isHyperpollingWireless flags. QC's UMA(179)/ZIA(164) master list is
        // therefore the only high-speed branch, after an online peer check.
        if kind == DeviceReadKind::Polling && matches!(physical_product_id, 164 | 179) {
            command.method = "getUSBHighSpeedPollingRate".into();
            command.command = [2, 0, 192];
            transport.polling_codes = [
                ("RATE_8000Hz", 1),
                ("RATE_4000Hz", 2),
                ("RATE_2000Hz", 4),
                ("RATE_1000Hz", 8),
                ("RATE_500Hz", 16),
                ("RATE_250Hz", 32),
                ("RATE_125Hz", 64),
            ]
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect();
            transport.polling_default_hz = Some(8000);
        }
        Ok(Self {
            transport,
            receiver,
            command,
        })
    }

    pub fn transport(&self) -> &DeviceReadCapability {
        &self.transport
    }

    pub fn method(&self) -> &str {
        &self.command.method
    }

    /// The adapter owns one physical handle and its lock for this entire call.
    /// Association changes during GET invalidate the observation; a saved
    /// pairing, profile value or failed status read cannot supply an answer.
    pub fn read(
        &self,
        device: &dyn FeatureTransport,
        next_transaction: impl FnMut() -> anyhow::Result<u8>,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Observation> {
        self.observe(device, next_transaction, validate, |next, validate| {
            device_query::read_device(device, &self.transport, &self.command, next()?, validate)
        })
    }

    /// Current85191.ot: cap ordinary masters to1000, pre-read/skip, setter
    /// ACK, abort check, real getter comparison. No OBM slot is implied.
    pub fn apply_polling(
        &self,
        device: &dyn FeatureTransport,
        hz: u32,
        next_transaction: impl FnMut() -> anyhow::Result<u8>,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Observation<PollingWriteResult>> {
        ensure!(
            self.command.name == DeviceReadKind::Polling,
            "Polling writer requires its source getter"
        );
        ensure!(
            [125, 250, 500, 1000, 2000, 4000, 8000].contains(&hz),
            "Polling rate is outside source choices"
        );
        let high_speed = self.command.command == [2, 0, 192];
        let effective_hz = if high_speed { hz } else { hz.min(1000) };
        let code = *self
            .transport
            .polling_codes
            .get(&format!("RATE_{effective_hz}Hz"))
            .context("Polling setter has no source code for this rate")?;
        self.observe(device, next_transaction, validate, |next, validate| {
            let get = |transaction| -> anyhow::Result<u32> {
                match device_query::read_device(
                    device,
                    &self.transport,
                    &self.command,
                    transaction,
                    validate,
                )? {
                    DeviceReadValue::Polling { hz } => Ok(hz),
                    _ => anyhow::bail!("Polling getter returned another value kind"),
                }
            };
            let previous_hz = get(next()?)?;
            validate()?;
            if previous_hz == effective_hz {
                return Ok(PollingWriteResult {
                    requested_hz: hz,
                    effective_hz,
                    previous_hz,
                    observed_hz: previous_hz,
                    acknowledgement: None,
                    changed: false,
                });
            }
            let acknowledgement = crate::feature_exchange::exchange(
                device,
                &self.transport,
                if high_speed { [2, 0, 64] } else { [2, 0, 14] },
                &[1, code],
                next()?,
                validate,
            )?;
            validate()?;
            let observed_hz = get(next()?)?;
            ensure!(
                observed_hz == effective_hz,
                "Polling setter acknowledged but getter differs"
            );
            validate()?;
            Ok(PollingWriteResult {
                requested_hz: hz,
                effective_hz,
                previous_hz,
                observed_hz,
                acknowledgement: Some(acknowledgement),
                changed: true,
            })
        })
    }

    pub fn read_dpi_stages(
        &self,
        device: &dyn FeatureTransport,
        next_transaction: impl FnMut() -> anyhow::Result<u8>,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Observation<crate::mouse_dpi_stages::DpiStagesReading>> {
        let settings = self.dpi_stages_capability()?;
        self.observe(device, next_transaction, validate, |next, validate| {
            crate::mouse_dpi_stages::read_current(device, &settings, next, validate)
        })
    }

    /// The mounted PID182 power-saving task (85191.Rt) calls the same inherited
    /// setTimeToSleep/getTimeToSleep methods on its DualLink mouse. The master
    /// changes framing only; no profile selector or preliminary idle GET exists.
    pub fn apply_idle(
        &self,
        device: &dyn FeatureTransport,
        raw_time: u16,
        next_transaction: impl FnMut() -> anyhow::Result<u8>,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Observation<crate::device_writes::DeviceWriteResult>> {
        use crate::device_writes::{self, DeviceWriteSetting};
        ensure!(
            self.command.name == DeviceReadKind::Idle,
            "Idle writer requires its source getter"
        );
        let setting = DeviceWriteSetting::Idle { raw_time };
        device_writes::prepare(
            device_writes::capability(PRODUCT_ID).context("PID182 idle setter is absent")?,
            &setting,
        )?;
        self.observe(device, next_transaction, validate, |next, validate| {
            device_writes::apply(device, &self.transport, &setting, next, validate)
        })
    }

    pub fn apply_dpi_stages(
        &self,
        device: &dyn FeatureTransport,
        draft: &crate::mouse_dpi_stages::DpiStagesDraft,
        next_transaction: impl FnMut() -> anyhow::Result<u8>,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Observation<crate::mouse_dpi_stages::DpiStagesWriteResult>> {
        let settings = self.dpi_stages_capability()?;
        // Reject malformed drafts before querying or mutating the receiver.
        crate::mouse_dpi_stages::pack(&settings, draft)?;
        self.observe(device, next_transaction, validate, |next, validate| {
            crate::mouse_dpi_stages::apply_current(device, &settings, draft, next, validate)
        })
    }

    pub fn current_dpi(
        &self,
        device: &dyn FeatureTransport,
        change: Option<&crate::mouse_current_dpi::DpiChange>,
        next_transaction: impl FnMut() -> anyhow::Result<u8>,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Observation<crate::mouse_current_dpi::DpiReport>> {
        if let Some(change) = change {
            crate::mouse_current_dpi::validate_change_for(PRODUCT_ID, change)?;
        }
        self.observe(device, next_transaction, validate, |next, validate| {
            crate::mouse_current_dpi::execute_receiver(
                &self.transport,
                device,
                change,
                next,
                validate,
            )
        })
    }

    fn dpi_stages_capability(
        &self,
    ) -> anyhow::Result<crate::mouse_dpi_stages::MouseDpiStagesCapability> {
        let mut settings = crate::mouse_dpi_stages::capability(PRODUCT_ID)
            .context("PID182 source DPI table capability is absent")?
            .clone();
        settings.transport = self.transport.clone();
        Ok(settings)
    }

    fn observe<T, N, V>(
        &self,
        device: &dyn FeatureTransport,
        mut next_transaction: N,
        validate: V,
        operation: impl FnOnce(&mut N, &V) -> anyhow::Result<T>,
    ) -> anyhow::Result<Observation<T>>
    where
        N: FnMut() -> anyhow::Result<u8>,
        V: Fn() -> anyhow::Result<()>,
    {
        validate()?;
        let receiver_before =
            device_query::read_receiver(device, &self.receiver, next_transaction()?, &validate)?;
        validate_peer(&receiver_before)?;
        let reading = operation(&mut next_transaction, &validate)?;
        let receiver_after =
            device_query::read_receiver(device, &self.receiver, next_transaction()?, &validate)?;
        validate_peer(&receiver_after)?;
        validate()?;
        Ok(Observation {
            reading,
            receiver_before,
            receiver_after,
        })
    }
}

fn validate_peer(peers: &[(u16, u8)]) -> anyhow::Result<()> {
    let mut matching = peers
        .iter()
        .filter(|(pid, _)| u32::from(*pid) == PEER_PRODUCT_ID);
    let peer = matching.next().context("接收器未报告 PID183 无线设备")?;
    ensure!(
        matching.next().is_none(),
        "接收器重复报告 PID183，关联不唯一"
    );
    ensure!(peer.1 == 1, "接收器没有确认 PID183 无线设备当前在线");
    Ok(())
}

#[cfg(test)]
#[path = "mouse_receiver_polling_tests.rs"]
mod tests;

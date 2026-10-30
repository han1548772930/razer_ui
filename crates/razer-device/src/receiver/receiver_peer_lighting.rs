//! Current battery-monitor peer factories use source Feature commands. Native
//! fallback frames are ordinary base25 rows; Krakoff targets only its dock.
use super::{PeerDeviceFactory, PeerExitEffect, PeerSupport};
use crate::{
    backend::{FeatureBatch, FeatureBatchFailure, FeatureTransport},
    lighting_driver::HardwareEffect,
    lighting_engine::EngineFrame,
    receiver_feature_channel::{Channel, WriteProgress, exchange_channel},
};
use anyhow::{Context as _, ensure};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Edition {
    pub layout: u8,
    pub edition: u8,
    pub firmware: u8,
}
impl Edition {
    pub fn new(layout: u8, edition: u8, firmware: u8) -> Self {
        Self {
            layout,
            edition,
            firmware,
        }
    }
}

pub struct PeerHid {
    support: PeerSupport,
    dock_transaction: u8,
    mouse_transaction: u8,
    acknowledged: u32,
    restoration_required: bool,
    custom_selected: bool,
}
impl PeerHid {
    pub fn new(support: PeerSupport) -> anyhow::Result<Self> {
        ensure!(
            support.rz_device_name != PeerDeviceFactory::RzDevice30,
            "BLE peer is not a Feature transport"
        );
        Ok(Self {
            support,
            dock_transaction: 0,
            mouse_transaction: 0,
            acknowledged: 0,
            restoration_required: false,
            custom_selected: false,
        })
    }
    pub fn restoration_required(&self) -> bool {
        self.restoration_required
    }
    pub fn custom_selected(&self) -> bool {
        self.custom_selected
    }
    fn channel(&self, mouse: bool) -> Channel {
        let outgoing = if self.support.rz_device_name == PeerDeviceFactory::RzDevice25 {
            0
        } else {
            self.support.claim_interface
        };
        Channel {
            source_scope: (self.support.rz_device_name == PeerDeviceFactory::RzDevice25).then_some(
                crate::backend::SourceCommandScope::Protocol25 {
                    topology: crate::backend::Protocol25Topology::Normal,
                },
            ),
            report_id: outgoing,
            get_report_id: self.support.claim_interface,
            transaction_prefix: if self.support.rz_device_name
                == PeerDeviceFactory::RzDevice25Krakoff
                && !mouse
            {
                224
            } else {
                0
            },
            transaction_modulus: 31,
            max_retry_in: self.support.max_retry_in,
            max_retry_out: self.support.max_retry_out,
            sleep_between_out_ms: u64::from(self.support.sleep_time_between_out_ms),
            sleep_between_out_in_ms: u64::from(self.support.sleep_time_between_out_in_ms),
            sleep_between_in_ms: u64::from(self.support.sleep_time_between_in_ms),
            ignore_set_device_mode_timeout_error: self.support.ignore_set_device_mode_timeout_error,
        }
    }
    fn next(&mut self, mouse: bool) -> u8 {
        let prefix = self.channel(mouse).transaction_prefix;
        let counter = if mouse {
            &mut self.mouse_transaction
        } else {
            &mut self.dock_transaction
        };
        let transaction = prefix | *counter;
        *counter = (*counter + 1) % 31;
        transaction
    }
    fn command(
        &mut self,
        device: &dyn FeatureTransport,
        command: [u8; 3],
        payload: &[u8],
        mouse: bool,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<u8>> {
        let transaction = self.next(mouse);
        let mut progress = WriteProgress::default();
        let result = exchange_channel(
            device,
            &self.channel(mouse),
            command,
            payload,
            transaction,
            validate,
            &mut progress,
        );
        self.acknowledged += u32::from(progress.acknowledged);
        result
    }
    pub fn edition(
        &mut self,
        device: &dyn FeatureTransport,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Edition> {
        let mut last = None;
        for attempt in 0..3 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            validate()?;
            match self.command(device, [3, 0, 134], &[], false, validate) {
                Ok(data) => {
                    ensure!(data.len() >= 3, "Peer edition reply is truncated");
                    return Ok(Edition {
                        layout: data[0],
                        edition: data[1],
                        firmware: data[2],
                    });
                }
                Err(error) if crate::backend::exchange_finalization_failed(&error) => {
                    return Err(error);
                }
                Err(error) => last = Some(error),
            }
        }
        Err(last.context("Peer edition query has no attempt")?)
    }
    pub fn battery(
        &mut self,
        device: &dyn FeatureTransport,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u8> {
        let data = self.command(device, [2, 7, 128], &[0], false, validate)?;
        ensure!(data.len() >= 2, "Peer battery reply is truncated");
        ensure!(
            data[0] == 0,
            "Peer battery reply belongs to another battery ID"
        );
        Ok((u16::from(data[1]) * 100 / 255) as u8)
    }
    pub fn on_dock(
        &mut self,
        device: &dyn FeatureTransport,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<bool> {
        let data = self.command(device, [1, 0, 187], &[], false, validate)?;
        Ok(*data
            .first()
            .context("Peer docking-charge reply is truncated")?
            == 1)
    }
    fn both(
        &mut self,
        device: &dyn FeatureTransport,
        command: [u8; 3],
        payload: &[u8],
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u32> {
        let previous = self.acknowledged;
        self.command(device, command, payload, false, validate)?;
        if self.support.rz_device_name == PeerDeviceFactory::RzDevice25Krakoff {
            self.command(device, command, payload, true, validate)?;
        }
        Ok(self.acknowledged - previous)
    }
    pub fn select_custom(
        &mut self,
        device: &dyn FeatureTransport,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u32> {
        // A mode write can succeed before the custom-effect transaction fails.
        // Retain its restoration obligation at the transport owner.
        self.restoration_required = true;
        let mut acknowledged = self.both(device, [2, 0, 4], &[3, 0], validate)?;
        acknowledged += self.both(
            device,
            [6, 15, 2],
            &[0, 0, HardwareEffect::CustomFrame.code(), 0, 0, 0],
            validate,
        )?;
        self.custom_selected = true;
        Ok(acknowledged)
    }
    pub fn restore(
        &mut self,
        device: &dyn FeatureTransport,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u32> {
        let mut acknowledged = self.both(device, [2, 0, 4], &[0, 0], validate)?;
        let payload = match self.support.quick_effect_on_exit {
            PeerExitEffect::Spectrum => vec![1, 0, HardwareEffect::Spectrum.code(), 0, 0, 0],
            PeerExitEffect::Static { color } => {
                // NEW_CHROMA_EFFECT_ID.Static is 1 at the hardware boundary;
                // the native engine's Static effect uses a separate ID of 6.
                vec![
                    1,
                    0,
                    HardwareEffect::Static.code(),
                    0,
                    0,
                    1,
                    color[0],
                    color[1],
                    color[2],
                ]
            }
        };
        acknowledged += self.both(device, [payload.len() as u8, 15, 2], &payload, validate)?;
        self.restoration_required = false;
        self.custom_selected = false;
        Ok(acknowledged)
    }
    pub fn send_frame(
        &mut self,
        device: &dyn FeatureTransport,
        frame: &EngineFrame,
        handle: u32,
        rows: u8,
        columns: u8,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u32> {
        ensure!(
            frame.device_handle == handle
                && frame.num_row == rows
                && frame.num_col == columns
                && frame.frame.len() == usize::from(rows) * usize::from(columns) * 3
                && columns > 0
                && columns <= 25,
            "Peer native frame identity or geometry differs"
        );
        let mut batch_receipt = None;
        crate::feature_exchange::with_validated_exchange(
            device,
            crate::backend::ExchangeOptions::batch(),
            validate,
            |device, _| {
                let mut batch = FeatureBatch::new();
                for row in 0..rows {
                    let mut payload = vec![0, frame.region_id, row, 0, columns - 1];
                    let start = usize::from(row) * usize::from(columns) * 3;
                    payload
                        .extend_from_slice(&frame.frame[start..start + usize::from(columns) * 3]);
                    // setChromaFrame's checkUSBTransferIn defaults false. A successful
                    // write is a submission receipt, not a fabricated Feature ACK.
                    let transaction = self.next(false);
                    let mut report = [0u8; 91];
                    report[0] = self.channel(false).report_id;
                    report[2] = transaction;
                    report[6..9].copy_from_slice(&[payload.len() as u8, 15, 3]);
                    report[9..9 + payload.len()].copy_from_slice(&payload);
                    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
                    batch.push_report(&report)?;
                    if row + 1 < rows {
                        batch.push_sleep(1);
                    }
                }
                let receipt = device.send_feature_batch(&batch)?;
                let submitted = receipt.progress().reports_submitted();
                let complete = receipt.raw_return() != 0 && submitted == usize::from(rows);
                batch_receipt = Some(receipt);
                ensure!(
                    complete,
                    "Peer native lighting batch did not submit every row"
                );
                Ok(submitted as u32)
            },
        )
        .map_err(|error| {
            let error = match batch_receipt {
                Some(receipt) => receipt.into_failure(error),
                None => error,
            };
            let reports_submitted = FeatureBatchFailure::from_error(&error)
                .map_or(0, |failure| failure.progress.reports_submitted() as u32);
            error.context(crate::ReceiverFrameSubmissionFailure { reports_submitted })
        })
    }
}

#[cfg(test)]
#[path = "receiver_peer_lighting_tests.rs"]
mod tests;

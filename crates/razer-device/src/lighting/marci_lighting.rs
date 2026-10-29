//! Current 3942 Marci Output frames and its separate Protocol25 control channel.
use crate::{
    backend::{FeatureTransport, HidNode},
    lighting_driver::{HardwareEffect, HardwareQuickEffect},
    lighting_engine::EngineFrame,
    receiver_lighting::QuickEffect,
};
use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};

pub const PRODUCT_ID: u16 = 3942;
pub const OUTPUT_BYTES: usize = 781;
pub const LED_COUNT: usize = 176;
pub const RGB_BYTES: usize = LED_COUNT * 3;
pub const DEFINED_BYTES: usize = 7 + RGB_BYTES;
pub const NATIVE_TAIL_BYTES: usize = OUTPUT_BYTES - DEFINED_BYTES;
const HEADER: [u8; 7] = [23, 0, 132, 0, 1, 0, 87];

pub fn manifest_exit_effect() -> HardwareQuickEffect {
    HardwareQuickEffect {
        effect: HardwareEffect::Wave,
        flags: 0,
        rate: 40,
        colors: Vec::new(),
    }
}
/// The 3942 manifest syncs UI IDs 1..8 on exit and maps the others to UI Wave
/// while retaining their current settings. This target is distinct from a
/// hardware observation and is sent only during the source cleanup sequence.
pub fn exit_effect_for_quick(effect: &QuickEffect) -> anyhow::Result<HardwareQuickEffect> {
    effect.hardware_effect_as(if (1..=8).contains(&effect.source_id()) {
        effect.source_id()
    } else {
        4
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame([u8; RGB_BYTES]);
impl Frame {
    /// The source connects its Output collection with this exact blank query.
    pub fn connection_probe() -> Self {
        Self([0; RGB_BYTES])
    }
    pub fn from_engine(frame: &EngineFrame, device_handle: u32) -> anyhow::Result<Self> {
        ensure!(
            device_handle != 0 && frame.device_handle == device_handle,
            "Marci frame belongs to a different renderer device"
        );
        ensure!(
            frame.region_id == 0 && frame.num_row == 1 && frame.num_col == 176,
            "Marci frame differs from the current 1 by 176 source matrix"
        );
        Ok(Self(frame.frame.as_slice().try_into().map_err(|_| {
            anyhow::anyhow!("Marci RGB payload must contain 528 bytes")
        })?))
    }
    /// Source JS constructs Uint8Array(781). Its zero tail is a JS allocation
    /// property, not a defined native protocol padding value.
    pub fn javascript_report(&self) -> [u8; OUTPUT_BYTES] {
        let mut report = [0; OUTPUT_BYTES];
        self.copy_defined(&mut report);
        report
    }
    /// Native full-frame comparison must supply all captured opaque tail bytes.
    /// Rust never reads uninitialized memory or synthesizes native tail values.
    pub fn native_report(&self, retained_tail: &[u8; NATIVE_TAIL_BYTES]) -> [u8; OUTPUT_BYTES] {
        let mut report = [0; OUTPUT_BYTES];
        self.copy_defined(&mut report);
        report[DEFINED_BYTES..].copy_from_slice(retained_tail);
        report
    }
    fn copy_defined(&self, report: &mut [u8; OUTPUT_BYTES]) {
        report[..7].copy_from_slice(&HEADER);
        report[7..DEFINED_BYTES].copy_from_slice(&self.0);
    }
}

pub fn validate_output_node(node: &HidNode) -> anyhow::Result<()> {
    validate_node(node)?;
    ensure!(
        node.usage_page == 0xff14 && node.usage == 1,
        "Marci Output collection is not usage ff14/1"
    );
    Ok(())
}
pub fn validate_control_node(node: &HidNode) -> anyhow::Result<()> {
    validate_node(node)?;
    ensure!(
        node.usage_page == 12 && node.usage == 1,
        "Marci control collection is not usage 12/1"
    );
    Ok(())
}
fn validate_node(node: &HidNode) -> anyhow::Result<()> {
    ensure!(
        node.vendor_id == 0x1532 && node.product_id == PRODUCT_ID && !node.path.is_empty(),
        "Marci collection belongs to another physical product"
    );
    ensure!(
        node.interface_number == 2 || cfg!(windows) && node.interface_number == -1,
        "Marci collection is not source interface 2"
    );
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OutputReceipt {
    pub submitted_bytes: usize,
    pub transferred_bytes: usize,
    pub complete: bool,
}

#[derive(Debug)]
pub struct OutputReleaseFailure {
    pub receipt: OutputReceipt,
}

impl std::fmt::Display for OutputReleaseFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Marci Output transferred {} of {} bytes; HID exchange release failed",
            self.receipt.transferred_bytes, self.receipt.submitted_bytes
        )
    }
}

impl std::error::Error for OutputReleaseFailure {}
/// The counted Output path reports actual platform completion. No device ACK
/// exists on this channel. Official HID.node's immediate-success -1 branch is
/// retained as a comparison difference, not invented by this Rust adapter.
pub fn submit_javascript_frame(
    transport: &dyn FeatureTransport,
    frame: &Frame,
) -> anyhow::Result<OutputReceipt> {
    validate_output_node(
        transport
            .hid_node()
            .ok_or_else(|| anyhow::anyhow!("Marci Output identity is unobserved"))?,
    )?;
    ensure!(
        transport.report_lengths()?.output.values().max().copied() == Some(OUTPUT_BYTES),
        "Marci Output descriptor does not expose 781 bytes"
    );
    let lease = transport.begin_exchange(crate::backend::ExchangeOptions::plain())?;
    let result = (|| {
        let transport = lease.transport();
        let count = transport.write_output_counted(&frame.javascript_report())?;
        ensure!(
            count <= OUTPUT_BYTES,
            "Marci Output returned an excessive byte count"
        );
        Ok(OutputReceipt {
            submitted_bytes: OUTPUT_BYTES,
            transferred_bytes: count,
            complete: count == OUTPUT_BYTES,
        })
    })();
    match result {
        Ok(receipt) => match lease.finish() {
            Ok(()) => Ok(receipt),
            Err(error) => Err(error.context(OutputReleaseFailure { receipt })),
        },
        Err(error) => crate::backend::finish_exchange(lease, Err(error)),
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Gates {
    pub paused: bool,
    pub driver_enabled: bool,
    pub host_frames: bool,
    pub lighting_off: bool,
}
impl Gates {
    pub const fn running() -> Self {
        Self {
            paused: false,
            driver_enabled: true,
            host_frames: true,
            lighting_off: false,
        }
    }
    pub fn forwards(self) -> bool {
        !self.paused && self.driver_enabled && self.host_frames && !self.lighting_off
    }
}

/// Engine mode 2 replaces, gathers and clears the latest frame for each handle.
/// Logical time is supplied by the owner; missed ticks never replay a FIFO.
pub struct LatestFrames {
    generation: u64,
    next_push_ms: u64,
    pending: BTreeMap<u32, EngineFrame>,
    gates: Gates,
    stopped: bool,
}
impl LatestFrames {
    pub fn new(generation: u64, now_ms: u64) -> Self {
        Self {
            generation,
            next_push_ms: now_ms.saturating_add(10),
            pending: BTreeMap::new(),
            gates: Gates::running(),
            stopped: false,
        }
    }
    pub fn replace(&mut self, generation: u64, frame: EngineFrame) -> anyhow::Result<()> {
        ensure!(
            !self.stopped && generation == self.generation && frame.device_handle != 0,
            "Marci frame owner was stopped or replaced"
        );
        self.pending.insert(frame.device_handle, frame);
        Ok(())
    }
    pub fn change_generation(&mut self, generation: u64) -> anyhow::Result<()> {
        ensure!(
            !self.stopped && generation > self.generation,
            "Marci update generation is stale"
        );
        self.generation = generation;
        self.pending.clear();
        Ok(())
    }
    pub fn set_gates(&mut self, gates: Gates) {
        self.gates = gates;
    }
    pub fn due(&mut self, now_ms: u64) -> Vec<EngineFrame> {
        if self.stopped || now_ms < self.next_push_ms {
            return Vec::new();
        }
        self.next_push_ms = now_ms.saturating_add(40);
        let latest = std::mem::take(&mut self.pending);
        if self.gates.forwards() {
            latest.into_values().collect()
        } else {
            Vec::new()
        }
    }
    pub fn stop(&mut self) {
        self.stopped = true;
        self.pending.clear();
    }
    pub fn until_due_ms(&self, now_ms: u64) -> u64 {
        self.next_push_ms.saturating_sub(now_ms)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Operation {
    Apply {
        generation: u64,
        effect: QuickEffect,
    },
    SetGates {
        generation: u64,
        gates: Gates,
    },
    Poll,
    Stop,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Status {
    pub owner_id: String,
    pub device_container_id: String,
    pub generation: u64,
    pub frames_submitted: u64,
    pub last_output: Option<OutputReceipt>,
    pub connection_output: Option<OutputReceipt>,
    pub screen: Option<crate::lighting_engine::renderer::ScreenCaptureStatus>,
    pub control_acknowledged: u32,
    pub stopped: bool,
    pub renderer_released: bool,
    pub transports_released: bool,
    pub exit_required: bool,
    pub normal_mode_confirmed: bool,
    pub exit_effect_confirmed: bool,
    pub planned_exit_effect: HardwareQuickEffect,
    pub errors: Vec<String>,
    pub cleanup_errors: Vec<String>,
}
impl Status {
    pub fn starting(owner_id: String, device_container_id: String, generation: u64) -> Self {
        Self {
            owner_id,
            device_container_id,
            generation,
            frames_submitted: 0,
            last_output: None,
            connection_output: None,
            screen: None,
            control_acknowledged: 0,
            stopped: false,
            renderer_released: true,
            transports_released: true,
            exit_required: false,
            normal_mode_confirmed: false,
            exit_effect_confirmed: false,
            planned_exit_effect: manifest_exit_effect(),
            errors: Vec::new(),
            cleanup_errors: Vec::new(),
        }
    }
    pub fn cleanup_completed(&self) -> bool {
        self.stopped
            && self.renderer_released
            && self.transports_released
            && self.screen.as_ref().is_none_or(|screen| screen.released)
            && (!self.exit_required || self.normal_mode_confirmed && self.exit_effect_confirmed)
            && self.cleanup_errors.is_empty()
    }
}

/// A control owner has its own transaction sequence; Output frames never advance it.
#[derive(Default)]
pub struct Control {
    transaction: u8,
}
impl Control {
    fn request(&mut self, command: [u8; 3], payload: &[u8]) -> anyhow::Result<[u8; 91]> {
        self.transaction = if self.transaction == 31 {
            1
        } else {
            self.transaction + 1
        };
        crate::controller_feature::report(16, self.transaction, command, payload)
    }
    /// Marci's exact 20 OUT / 30 IN, 40ms OUT-to-IN and 5ms later IN retry chain.
    pub fn exchange(
        &mut self,
        transport: &dyn FeatureTransport,
        command: [u8; 3],
        payload: &[u8],
        wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<u8>> {
        validate_control_node(
            transport
                .hid_node()
                .ok_or_else(|| anyhow::anyhow!("Marci control identity is unobserved"))?,
        )?;
        ensure!(
            transport.report_lengths()?.feature.values().max().copied() == Some(91),
            "Marci Feature descriptor differs from 91 bytes"
        );
        let request = self.request(command, payload)?;
        wait(Duration::ZERO)?;
        crate::feature_exchange::with_feature_reply(
            transport,
            crate::backend::ExchangeOptions::plain(),
            command,
            request[2],
            |transport, _| {
                let mut error = String::from("No Marci control response");
                for _ in 0..20 {
                    wait(Duration::ZERO)?;
                    if let Err(failure) = transport.send_feature_at_collection_max(&request) {
                        error = format!("{failure:#}");
                        continue;
                    }
                    wait(Duration::from_millis(40))?;
                    let mut resend = false;
                    for attempt in 0..30 {
                        if attempt > 0 {
                            wait(Duration::from_millis(5))?;
                        }
                        let mut response = [0; 91];
                        response[0] = 16;
                        let count = match transport.get_feature_at_collection_max(&mut response) {
                            Ok(n) => n,
                            Err(failure) => {
                                error = format!("{failure:#}");
                                break;
                            }
                        };
                        ensure!(count == 91, "Marci control response is truncated");
                        if response[2] != request[2] || response[7] != request[7] {
                            error = "Marci response transaction/class differs".into();
                            resend = true;
                            break;
                        }
                        if response[8] != request[8] {
                            error = "Marci response command differs".into();
                            resend = true;
                            continue;
                        }
                        match response[1] {
                            2 => {
                                ensure!(
                                    response[6] <= 80,
                                    "Marci response payload exceeds the report"
                                );
                                return Ok(response[9..9 + usize::from(response[6])].to_vec());
                            }
                            1 | 4 => {
                                error =
                                    format!("Marci control busy/timeout status {}", response[1]);
                                resend = true;
                            }
                            0 | 3 => {
                                error = format!("Marci control retry status {}", response[1]);
                                resend = true;
                                break;
                            }
                            status => bail!("Marci control rejected with status {status}"),
                        }
                    }
                    if !resend {
                        break;
                    }
                }
                bail!("Marci control was not confirmed: {error}")
            },
        )
    }
    pub fn firmware(
        &mut self,
        transport: &dyn FeatureTransport,
        wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let bytes = self.exchange(transport, [4, 0, 135], &[], wait)?;
        ensure!(bytes.len() >= 4, "Marci firmware observation is truncated");
        Ok(())
    }
    /// The source helper GETs before SET and skips a confirmed matching mode.
    pub fn mode(
        &mut self,
        transport: &dyn FeatureTransport,
        mode: u8,
        wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<bool> {
        ensure!(
            matches!(mode, 0 | 3),
            "Marci mode is neither normal nor driver"
        );
        let observed = self.exchange(transport, [2, 0, 132], &[], wait)?;
        ensure!(observed.len() >= 2, "Marci device mode is unobserved");
        if observed[0] == mode {
            return Ok(false);
        }
        self.exchange(transport, [2, 0, 4], &[mode, 0], wait)?;
        Ok(true)
    }
    /// Manifest effect 4/direction "up" reaches Uint8Array as flag 0; the
    /// converter supplies rate 40. This is an exit target, not an old snapshot.
    pub fn exit_wave(
        &mut self,
        transport: &dyn FeatureTransport,
        wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.exit_effect(transport, &manifest_exit_effect(), wait)?;
        Ok(())
    }
    pub fn exit_effect(
        &mut self,
        transport: &dyn FeatureTransport,
        target: &HardwareQuickEffect,
        wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let payload = target.protocol25_payload(1, 0)?;
        self.exchange(transport, [payload.len() as u8, 15, 2], &payload, wait)?;
        Ok(())
    }
}

#[path = "marci_geometry.rs"]
mod geometry;
pub use geometry::led_configuration;

#[cfg(test)]
#[path = "marci_lighting_tests.rs"]
mod tests;

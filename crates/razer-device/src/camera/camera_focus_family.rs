//! Independently audited 1.0.35.0 autofocus private commands for PID 3595/3596.
//! These products scan topology nodes and use different GUIDs/selectors and
//! FDAF payloads from PID 3594. Platform I/O belongs to the injected transport.
use crate::camera_extensions::CameraKsCallOutcome;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const CAMERA_FOCUS_MAX_TOPOLOGY_NODES: u32 = 256;
pub const CAMERA_FOCUS_DELAY_MS: u32 = 10;
pub const CAMERA_FOCUS_BASE_PROPERTY_SET: [u8; 16] = [
    0xd0, 0x9e, 0xe4, 0x23, 0x78, 0x11, 0x31, 0x4f, 0xae, 0x52, 0xd2, 0xfb, 0x8a, 0x8d, 0x3b, 0x48,
];
pub const CAMERA_FOCUS_X_PROPERTY_SET: [u8; 16] = [
    0xc3, 0x85, 0xb8, 0x0f, 0xc2, 0x68, 0x47, 0x45, 0x90, 0xf7, 0x8f, 0x47, 0x57, 0x9d, 0x95, 0xfc,
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraFamilyFocusRequest {
    SetFdafMode {
        mode: i32,
        light_condition: i32,
    },
    SetAutoFocusLowSensitivity {
        value: i32,
    },
    /// Base/X use this same audited topology-scanning ISP transport.
    SetAutoframing {
        mode: i32,
    },
    SetAutoframingSpeedAndSize {
        speed: i32,
        size: i32,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraFamilyFocusCommand {
    pub bytes: [u8; 8],
    pub native_rva: u32,
}
impl CameraFamilyFocusRequest {
    pub fn command(self, product_id: u32) -> Result<CameraFamilyFocusCommand> {
        ensure!(
            matches!(product_id, 3595 | 3596),
            "camera family focus product has no current private audit"
        );
        Ok(match self {
            Self::SetFdafMode {
                mode,
                light_condition,
            } if product_id == 3595 => CameraFamilyFocusCommand {
                bytes: [0xc0, 0x0a, 1, mode as u8, 0, 0, 0, light_condition as u8],
                native_rva: 0x4da20,
            },
            Self::SetFdafMode { mode, .. } => CameraFamilyFocusCommand {
                // The native export narrows to u8 before the X method compares
                // with 1 and serializes a four-byte Windows BOOL at bytes 2..6.
                bytes: [0x2e, 0, u8::from(mode as u8 != 1), 0, 0, 0, 0, 0],
                native_rva: 0x53c80,
            },
            Self::SetAutoFocusLowSensitivity { value } => CameraFamilyFocusCommand {
                bytes: [0xff, 0x06, value as u8, 0, 0, 0, 0, 0],
                native_rva: 0x4d600,
            },
            Self::SetAutoframing { mode } if product_id == 3595 => CameraFamilyFocusCommand {
                bytes: [1, 0, 1, mode as u8, 0, 0, 0, 0],
                native_rva: 0x4e380,
            },
            Self::SetAutoframing { mode } => CameraFamilyFocusCommand {
                bytes: [0x2b, 0, mode as u8, 0, 0, 0, 0, 0],
                native_rva: 0x54fd0,
            },
            Self::SetAutoframingSpeedAndSize { speed, size } => CameraFamilyFocusCommand {
                bytes: [0xc0, 0x0e, 0x0d, speed as u8, size as u8, 0, 0, 0],
                native_rva: 0x4e570,
            },
        })
    }
}
pub fn camera_family_focus_header(product_id: u32, node: u32, is_set: bool) -> Result<[u8; 32]> {
    let (guid, property) = match product_id {
        3595 => (
            CAMERA_FOCUS_BASE_PROPERTY_SET,
            if is_set { 1u32 } else { 2 },
        ),
        3596 => (CAMERA_FOCUS_X_PROPERTY_SET, 5),
        _ => anyhow::bail!("camera family focus product has no current private audit"),
    };
    let mut header = [0; 32];
    header[..16].copy_from_slice(&guid);
    header[16..20].copy_from_slice(&property.to_le_bytes());
    header[20..24]
        .copy_from_slice(&(if is_set { 0x1000_0002u32 } else { 0x1000_0001 }).to_le_bytes());
    header[24..28].copy_from_slice(&node.to_le_bytes());
    // Native leaves the reserved DWORD uninitialized; Rust initializes it.
    Ok(header)
}
pub trait CameraFamilyFocusTransport {
    /// QI IKsTopologyInfo and GetNumNodes (slot 8). Keep topology until cleanup.
    fn begin_topology(&mut self) -> Result<u32>;
    /// CreateNodeInstance (slot 10) for this candidate; adopt failure outputs.
    fn begin_node(&mut self, node: u32) -> Result<()>;
    fn ks_property(&mut self, header: &[u8; 32], data: &mut [u8; 8])
    -> Result<CameraKsCallOutcome>;
    fn end_node(&mut self);
    fn end_topology(&mut self);
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> {
        std::thread::sleep(std::time::Duration::from_millis(milliseconds.into()));
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraFamilyFocusPhase {
    Target,
    Validation,
    Topology,
    Set,
    Wait,
    Get,
    Decode,
    Exhausted,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraFamilyFocusFailure {
    pub phase: CameraFamilyFocusPhase,
    pub node_id: Option<u32>,
    pub message: String,
}
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraFamilyFocusAttempt {
    pub node_id: u32,
    pub opened: bool,
    pub open_error: Option<String>,
    pub set_attempted: bool,
    pub set_outcome: Option<CameraKsCallOutcome>,
    pub get_attempted: bool,
    pub get_outcome: Option<CameraKsCallOutcome>,
    pub response: Option<[u8; 8]>,
    pub acknowledged: bool,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraFamilyFocusProgress {
    pub product_id: u32,
    pub request: CameraFamilyFocusRequest,
    pub command: Option<CameraFamilyFocusCommand>,
    pub topology_nodes: Option<u32>,
    /// Preserve every node's SET/GET attempt, including earlier writes whose
    /// response was unavailable. A later accepted node cannot erase them.
    pub attempts: Vec<CameraFamilyFocusAttempt>,
    pub completed: bool,
    pub failure: Option<CameraFamilyFocusFailure>,
}
impl CameraFamilyFocusProgress {
    pub fn failed(
        product_id: u32,
        request: CameraFamilyFocusRequest,
        phase: CameraFamilyFocusPhase,
        message: String,
    ) -> Self {
        Self {
            product_id,
            request,
            command: request.command(product_id).ok(),
            topology_nodes: None,
            attempts: vec![],
            completed: false,
            failure: Some(CameraFamilyFocusFailure {
                phase,
                node_id: None,
                message,
            }),
        }
    }
}
pub fn execute_camera_family_focus(
    transport: &mut impl CameraFamilyFocusTransport,
    product_id: u32,
    request: CameraFamilyFocusRequest,
    validate: &impl Fn() -> Result<()>,
) -> CameraFamilyFocusProgress {
    use CameraFamilyFocusPhase::*;
    let command = match request.command(product_id) {
        Ok(command) => command,
        Err(e) => {
            return CameraFamilyFocusProgress::failed(product_id, request, Target, e.to_string());
        }
    };
    let exchange = execute_camera_family_isp(transport, product_id, command.bytes, validate);
    CameraFamilyFocusProgress {
        product_id,
        request,
        command: Some(command),
        topology_nodes: exchange.topology_nodes,
        attempts: exchange.attempts,
        completed: exchange.completed,
        failure: exchange.failure,
    }
}

/// Shared base/X ISP lifecycle. Exposure and focus use the same independently
/// audited native +344 transport; only command encoding/decoding differs.
#[derive(Default)]
pub(crate) struct CameraFamilyIspProgress {
    pub topology_nodes: Option<u32>,
    pub attempts: Vec<CameraFamilyFocusAttempt>,
    pub completed: bool,
    pub failure: Option<CameraFamilyFocusFailure>,
}
pub(crate) fn execute_camera_family_isp(
    transport: &mut impl CameraFamilyFocusTransport,
    product_id: u32,
    bytes: [u8; 8],
    validate: &impl Fn() -> Result<()>,
) -> CameraFamilyIspProgress {
    use CameraFamilyFocusPhase::*;
    let mut progress = CameraFamilyIspProgress::default();
    let failure = |phase, node_id, e: anyhow::Error| CameraFamilyFocusFailure {
        phase,
        node_id,
        message: format!("{e:#}"),
    };
    if let Err(e) = validate() {
        progress.failure = Some(failure(Validation, None, e));
        return progress;
    }
    let result = (|| -> std::result::Result<(), CameraFamilyFocusFailure> {
        let nodes = transport
            .begin_topology()
            .map_err(|e| failure(Topology, None, e))?;
        progress.topology_nodes = Some(nodes);
        ensure_topology_count(nodes).map_err(|e| failure(Topology, None, e))?;
        for node in 0..nodes {
            validate().map_err(|e| failure(Validation, Some(node), e))?;
            let mut attempt = CameraFamilyFocusAttempt {
                node_id: node,
                ..Default::default()
            };
            match transport.begin_node(node) {
                Ok(()) => attempt.opened = true,
                Err(e) => attempt.open_error = Some(format!("{e:#}")),
            }
            if !attempt.opened {
                transport.end_node();
                progress.attempts.push(attempt);
                continue;
            }
            // All exits release this IKsControl before the retained topology.
            let outcome = (|| -> std::result::Result<bool, CameraFamilyFocusFailure> {
                validate().map_err(|e| failure(Validation, Some(node), e))?;
                let mut packet = bytes;
                attempt.set_attempted = true;
                let set = transport
                    .ks_property(
                        &camera_family_focus_header(product_id, node, true).unwrap(),
                        &mut packet,
                    )
                    .map_err(|e| failure(Set, Some(node), e))?;
                attempt.set_outcome = Some(set);
                // Native continues after failed SET. Reject it before GET to
                // avoid accepting a stale response as the requested change.
                if set.hresult < 0 {
                    return Err(failure(
                        Set,
                        Some(node),
                        anyhow::anyhow!("camera family SET failed: 0x{:08x}", set.hresult as u32),
                    ));
                }
                validate().map_err(|e| failure(Validation, Some(node), e))?;
                transport
                    .wait_ms(CAMERA_FOCUS_DELAY_MS)
                    .map_err(|e| failure(Wait, Some(node), e))?;
                validate().map_err(|e| failure(Validation, Some(node), e))?;
                let mut response = [0; 8];
                response[0] = 0xff;
                attempt.get_attempted = true;
                let get = transport.ks_property(
                    &camera_family_focus_header(product_id, node, false).unwrap(),
                    &mut response,
                );
                attempt.response = Some(response);
                let get = get.map_err(|e| failure(Get, Some(node), e))?;
                attempt.get_outcome = Some(get);
                // A non-S_OK node is skipped by both independently audited
                // native loops. Retain that potentially applied SET receipt.
                if get.hresult != 0 {
                    return Ok(false);
                }
                if get.bytes_returned != 8 {
                    return Err(failure(
                        Get,
                        Some(node),
                        anyhow::anyhow!(
                            "camera family GET returned {} bytes; expected 8",
                            get.bytes_returned
                        ),
                    ));
                }
                if response[0] != 0 {
                    return Err(failure(
                        Decode,
                        Some(node),
                        anyhow::anyhow!(
                            "camera family device rejected command: status 0x{:02x}",
                            response[0]
                        ),
                    ));
                }
                attempt.acknowledged = true;
                validate().map_err(|e| failure(Validation, Some(node), e))?;
                Ok(true)
            })();
            transport.end_node();
            progress.attempts.push(attempt);
            if outcome? {
                return Ok(());
            }
        }
        Err(failure(
            Exhausted,
            None,
            anyhow::anyhow!("no camera topology node acknowledged the private ISP command"),
        ))
    })();
    // Cleanup is also required after a failed topology/QI/count attempt.
    transport.end_topology();
    match result {
        Ok(()) => progress.completed = true,
        Err(e) => progress.failure = Some(e),
    }
    progress
}
fn ensure_topology_count(nodes: u32) -> Result<()> {
    ensure!(
        (1..=CAMERA_FOCUS_MAX_TOPOLOGY_NODES).contains(&nodes),
        "camera topology node count is outside the bounded audit adapter: {nodes}"
    );
    Ok(())
}
fn set_ok(a: &CameraFamilyFocusAttempt) -> bool {
    a.set_attempted && a.set_outcome.is_some_and(|o| o.hresult >= 0)
}
fn get_ok(a: &CameraFamilyFocusAttempt) -> bool {
    a.get_attempted
        && a.get_outcome
            .is_some_and(|o| o.hresult == 0 && o.bytes_returned == 8)
        && a.response.is_some()
}
fn can_scan_next(a: &CameraFamilyFocusAttempt) -> bool {
    !a.acknowledged && (!a.opened || (set_ok(a) && a.get_outcome.is_some_and(|o| o.hresult != 0)))
}
/// Check product-specific packets and every node receipt before publication.
/// This only parses returned data; it cannot invoke an OS adapter.
pub fn validate_camera_family_focus_progress(
    product_id: u32,
    request: CameraFamilyFocusRequest,
    progress: &CameraFamilyFocusProgress,
) -> Result<()> {
    ensure!(
        progress.product_id == product_id && progress.request == request,
        "family focus receipt identity mismatch"
    );
    match request.command(product_id) {
        Ok(command) => ensure!(
            progress.command == Some(command),
            "family focus packet/native metadata mismatch"
        ),
        Err(_) => ensure!(
            progress.command.is_none()
                && progress
                    .failure
                    .as_ref()
                    .is_some_and(|f| f.phase == CameraFamilyFocusPhase::Target),
            "family focus unsupported command was not rejected before transport"
        ),
    }
    validate_camera_family_isp_progress(
        progress.topology_nodes,
        &progress.attempts,
        progress.completed,
        progress.failure.as_ref(),
    )
}

pub(crate) fn validate_camera_family_isp_progress(
    topology_nodes: Option<u32>,
    attempts: &[CameraFamilyFocusAttempt],
    completed: bool,
    failure: Option<&CameraFamilyFocusFailure>,
) -> Result<()> {
    use CameraFamilyFocusPhase::*;
    if let Some(nodes) = topology_nodes {
        if failure.as_ref().is_none_or(|f| f.phase != Topology) {
            ensure_topology_count(nodes)?;
        }
        ensure!(
            attempts.len() as u32 <= nodes
                && attempts.len() <= CAMERA_FOCUS_MAX_TOPOLOGY_NODES as usize,
            "extra family topology attempts"
        );
    } else {
        ensure!(
            attempts.is_empty(),
            "family attempts without topology count"
        );
    }
    for (i, a) in attempts.iter().enumerate() {
        ensure!(
            a.node_id == i as u32,
            "family node traversal is out of order"
        );
        ensure!(
            a.opened == a.open_error.is_none()
                && a.open_error.as_ref().is_none_or(|e| !e.is_empty()),
            "family open result is inconsistent"
        );
        ensure!(
            !a.set_attempted || a.opened,
            "family SET before successful node open"
        );
        ensure!(
            a.set_outcome.is_none() || a.set_attempted,
            "family SET result without attempt"
        );
        ensure!(
            !a.get_attempted || set_ok(a),
            "family GET before successful SET"
        );
        ensure!(
            a.get_outcome.is_none() || a.get_attempted,
            "family GET result without attempt"
        );
        ensure!(
            a.response.is_some() == a.get_attempted,
            "family GET must retain raw response"
        );
        if a.acknowledged {
            ensure!(
                set_ok(a) && get_ok(a) && a.response.unwrap()[0] == 0,
                "family ACK has rejected or incomplete exchange"
            );
        }
        if i + 1 < attempts.len() {
            ensure!(
                can_scan_next(a),
                "family traversal follows a terminal node result"
            );
        }
    }
    if completed {
        ensure!(
            failure.is_none() && attempts.last().is_some_and(|a| a.acknowledged),
            "family completion lacks acknowledgement"
        );
        return Ok(());
    }
    let f = failure
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("incomplete family receipt has no failure"))?;
    ensure!(!f.message.is_empty(), "family failure has no message");
    let last = attempts.last();
    let phase_ok = match f.phase {
        Target => topology_nodes.is_none() && last.is_none() && f.node_id.is_none(),
        Topology => {
            last.is_none()
                && f.node_id.is_none()
                && topology_nodes.is_none_or(|n| ensure_topology_count(n).is_err())
        }
        Exhausted => {
            f.node_id.is_none()
                && topology_nodes.is_some_and(|n| n == attempts.len() as u32)
                && attempts.iter().all(can_scan_next)
        }
        Validation => {
            if let Some(node) = f.node_id {
                topology_nodes.is_some_and(|n| node < n)
                    && (last.is_some_and(|a| {
                        a.node_id == node
                            && a.opened
                            && ((!a.set_attempted && !a.get_attempted)
                                || (set_ok(a) && !a.get_attempted)
                                || a.acknowledged)
                    }) || (node == attempts.len() as u32 && last.is_none_or(can_scan_next)))
            } else {
                topology_nodes.is_none() && last.is_none()
            }
        }
        Set => last.is_some_and(|a| {
            f.node_id == Some(a.node_id)
                && a.set_attempted
                && !a.get_attempted
                && !a.acknowledged
                && a.set_outcome.is_none_or(|o| o.hresult < 0)
        }),
        Wait => last.is_some_and(|a| {
            f.node_id == Some(a.node_id) && set_ok(a) && !a.get_attempted && !a.acknowledged
        }),
        Get => last.is_some_and(|a| {
            f.node_id == Some(a.node_id)
                && set_ok(a)
                && a.get_attempted
                && !a.acknowledged
                && a.get_outcome
                    .is_none_or(|o| o.hresult == 0 && o.bytes_returned != 8)
        }),
        Decode => last.is_some_and(|a| {
            f.node_id == Some(a.node_id)
                && set_ok(a)
                && get_ok(a)
                && !a.acknowledged
                && a.response.unwrap()[0] != 0
        }),
    };
    ensure!(
        phase_ok,
        "family failure phase does not match retained node attempts"
    );
    Ok(())
}

#[cfg(test)]
#[path = "camera_focus_family_tests.rs"]
mod tests;

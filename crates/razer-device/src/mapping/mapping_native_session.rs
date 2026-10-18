//! Explicit mapping-driver routing and lifecycle.
//!
//! The current mapping DLL has two distinct paths which are easy to confuse:
//! HID input-monitor discovery resolves a supplied interface path, while the
//! filter-driver probe enumerates an interface class and matches its device
//! container and `pid_XXXX` path component.  This module keeps that identity
//! decision separate from the Windows handle adapter and from the output
//! encoding modules.
//!
//! `NativeSessionTransport::submit` returning an id means only that the
//! operation was accepted by the queue.  A later `poll` completion is required
//! before the operation is considered successful.  The initial `0x88883018`
//! request is a persistent input stream, not a connection handshake: a handle
//! with an accepted read is connected while the read is still pending.

use std::{collections::BTreeMap, error::Error, fmt, time::Duration};

use crate::mapping_native_output::DRIVER_SEND_INPUT_IOCTL;
#[path = "mapping_native_session/hooks.rs"]
pub mod hooks;

/// RVA `0x1101D0` arms this read when the handle opens. RVA `0x114850`
/// continuously resubmits it using alternate 304-byte buffers.
pub const DRIVER_READ_INPUT_IOCTL: u32 = 0x8888_3018;
pub const DRIVER_READ_INPUT_OUTPUT_BYTES: usize = 304;
pub const RAZER_VENDOR_ID: u16 = 0x1532;
pub const MAX_CONNECT_RETRIES: u8 = 10;

/// GUID bytes recovered from the current binary at RVA `0x383518`; the
/// filter wrapper at `0x11A6EE` passes this class to `0x119DE6`.  The HID
/// class used by `0x11A702` is a separate interface class.
pub const FILTER_DRIVER_INTERFACE_CLASS: NativeGuid = NativeGuid::new(
    0xe3be_005d,
    0xd130,
    0x4910,
    [0x88, 0xff, 0x09, 0xae, 0x02, 0xf6, 0x80, 0xe9],
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl NativeGuid {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self {
            data1,
            data2,
            data3,
            data4,
        }
    }

    pub const fn is_nil(self) -> bool {
        self.data1 == 0 && self.data2 == 0 && self.data3 == 0 && u64::from_le_bytes(self.data4) == 0
    }
}

/// All identity/capability metadata must come from the requesting device's
/// current evidence. SetupDi observes path and container, not product kind
/// or a capability bitmap. This query never invents those observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeDiscoveryQuery {
    pub container: NativeGuid,
    pub product: NativeProductIdentity,
    pub verified_capabilities: NativeCapabilitySet,
}

impl NativeDiscoveryQuery {
    pub fn validate(&self) -> Result<(), NativeRouteError> {
        if self.container.is_nil() {
            Err(NativeRouteError::InvalidContainer)
        } else {
            Ok(())
        }
    }

    /// Container equality and the lowercased `pid_XXXX` path component are
    /// the native selection criteria. Return every matching collection in
    /// enumeration order; selecting an unrelated first HID path is invalid.
    pub fn route_for(
        &self,
        class: NativeGuid,
        container: NativeGuid,
        path: &str,
    ) -> Result<Option<NativeRoute>, NativeRouteError> {
        self.validate()?;
        if class != FILTER_DRIVER_INTERFACE_CLASS || container != self.container {
            return Ok(None);
        }
        let path = path.to_ascii_lowercase();
        let Some(offset) = path.find("pid_") else {
            return Ok(None);
        };
        let Some(hex) = path.get(offset + 4..offset + 8) else {
            return Ok(None);
        };
        if !hex.as_bytes().iter().all(u8::is_ascii_hexdigit)
            || u16::from_str_radix(hex, 16).ok() != Some(self.product.product_id())
        {
            return Ok(None);
        }
        Ok(Some(NativeRoute::windows_discovered(
            NativeDriverPath::new(path)?,
            self.product,
            self.verified_capabilities,
            container,
        )))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeDiscoveryFailure {
    pub interface_index: u32,
    pub operation: &'static str,
    pub win32_code: Option<u32>,
}

/// Partial interface failures are retained separately. A nonempty route list
/// does not imply that every interface was enumerated successfully.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeDiscoveryReport {
    pub routes: Vec<NativeRoute>,
    pub failures: Vec<NativeDiscoveryFailure>,
    pub enumeration_complete: bool,
}

/// A path copied from the native `path_` field.  The source calls
/// `CreateFileA`, so non-ASCII text, NULs and empty paths are rejected before
/// a platform adapter can touch the operating system.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeDriverPath(String);

impl NativeDriverPath {
    pub fn new(path: impl AsRef<str>) -> Result<Self, NativeRouteError> {
        let path = path.as_ref();
        if path.is_empty() || !path.is_ascii() || path.contains('\0') {
            return Err(NativeRouteError::InvalidDriverPath);
        }
        Ok(Self(path.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<&str> for NativeDriverPath {
    type Error = NativeRouteError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for NativeDriverPath {
    type Error = NativeRouteError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeDeviceKind {
    Keyboard,
    Mouse,
    Joystick,
    Gamepad,
    Display,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeProductIdentity {
    vendor_id: u16,
    product_id: u16,
    kind: NativeDeviceKind,
}

impl NativeProductIdentity {
    pub const fn new(vendor_id: u16, product_id: u16, kind: NativeDeviceKind) -> Self {
        Self {
            vendor_id,
            product_id,
            kind,
        }
    }

    pub const fn razer(product_id: u16, kind: NativeDeviceKind) -> Self {
        Self::new(RAZER_VENDOR_ID, product_id, kind)
    }

    pub const fn vendor_id(self) -> u16 {
        self.vendor_id
    }

    pub const fn product_id(self) -> u16 {
        self.product_id
    }

    pub const fn kind(self) -> NativeDeviceKind {
        self.kind
    }
}

/// Capability bits describe observed native routes.  They are deliberately
/// independent of product catalog names; a route must carry the bits needed
/// by the caller instead of inheriting support from a neighbouring PID.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeCapability {
    KeyboardInput = 1 << 0,
    MouseInput = 1 << 1,
    DisplayInput = 1 << 2,
    JoystickOutput = 1 << 3,
    GamepadOutput = 1 << 4,
    AnalogOutput = 1 << 5,
    Rotation = 1 << 6,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NativeCapabilitySet(u32);

impl NativeCapabilitySet {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn single(capability: NativeCapability) -> Self {
        Self(capability as u32)
    }

    pub const fn contains(self, capability: NativeCapability) -> bool {
        self.0 & capability as u32 != 0
    }

    pub const fn contains_all(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    pub const fn missing(self, required: Self) -> Self {
        Self(required.0 & !self.0)
    }

    pub const fn with(self, capability: NativeCapability) -> Self {
        Self(self.0 | capability as u32)
    }
}

impl From<NativeCapability> for NativeCapabilitySet {
    fn from(value: NativeCapability) -> Self {
        Self::single(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeRoutePlatform {
    Windows,
}

/// A route is complete only when the path, HID identity and observed
/// capabilities came from the same discovery record.  The constructor does
/// not select a path from a list and never falls back to the first HID path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeRoute {
    path: NativeDriverPath,
    product: NativeProductIdentity,
    capabilities: NativeCapabilitySet,
    platform: NativeRoutePlatform,
    container: Option<NativeGuid>,
}

impl NativeRoute {
    pub fn windows(
        path: NativeDriverPath,
        product: NativeProductIdentity,
        capabilities: NativeCapabilitySet,
    ) -> Self {
        Self {
            path,
            product,
            capabilities,
            platform: NativeRoutePlatform::Windows,
            container: None,
        }
    }

    pub fn windows_discovered(
        path: NativeDriverPath,
        product: NativeProductIdentity,
        capabilities: NativeCapabilitySet,
        container: NativeGuid,
    ) -> Self {
        Self {
            container: Some(container),
            ..Self::windows(path, product, capabilities)
        }
    }

    pub const fn container(&self) -> Option<NativeGuid> {
        self.container
    }

    pub fn path(&self) -> &NativeDriverPath {
        &self.path
    }

    pub const fn product(&self) -> NativeProductIdentity {
        self.product
    }

    pub const fn capabilities(&self) -> NativeCapabilitySet {
        self.capabilities
    }

    pub const fn platform(&self) -> NativeRoutePlatform {
        self.platform
    }

    pub fn validate(
        &self,
        expected_product: NativeProductIdentity,
        required_capabilities: NativeCapabilitySet,
    ) -> Result<(), NativeRouteError> {
        if self.product.vendor_id() != expected_product.vendor_id() {
            return Err(NativeRouteError::VendorMismatch {
                expected: expected_product.vendor_id(),
                observed: self.product.vendor_id(),
            });
        }
        if self.product != expected_product {
            return Err(NativeRouteError::ProductMismatch {
                expected: expected_product,
                observed: self.product,
            });
        }
        let missing = self.capabilities.missing(required_capabilities);
        if missing.bits() != 0 {
            return Err(NativeRouteError::MissingCapabilities {
                required: required_capabilities,
                observed: self.capabilities,
                missing,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeRouteError {
    InvalidDriverPath,
    InvalidContainer,
    VendorMismatch {
        expected: u16,
        observed: u16,
    },
    ProductMismatch {
        expected: NativeProductIdentity,
        observed: NativeProductIdentity,
    },
    MissingCapabilities {
        required: NativeCapabilitySet,
        observed: NativeCapabilitySet,
        missing: NativeCapabilitySet,
    },
}

impl fmt::Display for NativeRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDriverPath => {
                f.write_str("mapping driver path must be nonempty ASCII without NUL")
            }
            Self::InvalidContainer => {
                f.write_str("mapping driver discovery requires a nonzero device container GUID")
            }
            Self::VendorMismatch { expected, observed } => {
                write!(
                    f,
                    "mapping route vendor {observed:#06x} does not match {expected:#06x}"
                )
            }
            Self::ProductMismatch { expected, observed } => write!(
                f,
                "mapping route product identity {observed:?} does not match {expected:?}"
            ),
            Self::MissingCapabilities {
                required,
                observed,
                missing,
            } => write!(
                f,
                "mapping route lacks capability bits {:#x} (required {:#x}, observed {:#x})",
                missing.bits(),
                required.bits(),
                observed.bits()
            ),
        }
    }
}

impl Error for NativeRouteError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeSubmissionId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeOperation {
    ReadInput {
        ioctl: u32,
        output_bytes: usize,
    },
    Ioctl {
        ioctl: u32,
        input: Vec<u8>,
        output_bytes: usize,
    },
}

impl NativeOperation {
    pub const fn read_input() -> Self {
        Self::ReadInput {
            ioctl: DRIVER_READ_INPUT_IOCTL,
            output_bytes: DRIVER_READ_INPUT_OUTPUT_BYTES,
        }
    }

    pub fn ioctl(ioctl: u32, input: impl Into<Vec<u8>>, output_bytes: usize) -> Self {
        Self::Ioctl {
            ioctl,
            input: input.into(),
            output_bytes,
        }
    }

    pub const fn ioctl_code(&self) -> u32 {
        match self {
            Self::ReadInput { ioctl, .. } | Self::Ioctl { ioctl, .. } => *ioctl,
        }
    }

    pub const fn output_bytes(&self) -> usize {
        match self {
            Self::ReadInput { output_bytes, .. } | Self::Ioctl { output_bytes, .. } => {
                *output_bytes
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeTransportError {
    UnsupportedPlatform,
    InvalidPath,
    InvalidBufferLength {
        input_bytes: usize,
        output_bytes: usize,
    },
    Closed,
    Open {
        code: u32,
    },
    Submit {
        code: u32,
    },
    Poll {
        code: u32,
    },
    Cancel {
        code: u32,
    },
    Close {
        code: u32,
    },
    UnknownSubmission(NativeSubmissionId),
}

impl fmt::Display for NativeTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                f.write_str("mapping driver is unavailable on this platform")
            }
            Self::InvalidPath => f.write_str("mapping driver path is invalid"),
            Self::InvalidBufferLength {
                input_bytes,
                output_bytes,
            } => write!(
                f,
                "mapping driver buffer lengths exceed the native ABI: input {input_bytes}, output {output_bytes}"
            ),
            Self::Closed => f.write_str("mapping driver transport is closed"),
            Self::Open { code } => write!(f, "mapping driver open failed with Win32 error {code}"),
            Self::Submit { code } => {
                write!(f, "mapping driver submit failed with Win32 error {code}")
            }
            Self::Poll { code } => write!(
                f,
                "mapping driver completion failed with Win32 error {code}"
            ),
            Self::Cancel { code } => write!(
                f,
                "mapping driver cancellation failed with Win32 error {code}"
            ),
            Self::Close { code } => {
                write!(f, "mapping driver close failed with Win32 error {code}")
            }
            Self::UnknownSubmission(id) => write!(f, "unknown mapping driver submission {}", id.0),
        }
    }
}

impl Error for NativeTransportError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeCompletion {
    Pending,
    Completed(Result<Vec<u8>, NativeTransportError>),
}

/// A completed read is returned even when queuing the next read fails. The
/// caller can dispatch its actual bytes and inspect the independent rearm
/// outcome without claiming another event or a successful device write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeInputPoll {
    Pending,
    Completed {
        output: Vec<u8>,
        rearmed: Result<NativeSubmissionId, NativeTransportError>,
    },
    Failed(NativeTransportError),
}

/// OS adapters own handles and buffers.  In particular, an accepted
/// submission must retain its input/output storage until `Completed` is
/// observed, including after `cancel`.
pub trait NativeSessionTransport {
    fn open(&mut self, path: &NativeDriverPath) -> Result<(), NativeTransportError>;
    fn submit(
        &mut self,
        operation: NativeOperation,
    ) -> Result<NativeSubmissionId, NativeTransportError>;
    fn poll(
        &mut self,
        id: NativeSubmissionId,
        wait: Duration,
    ) -> Result<NativeCompletion, NativeTransportError>;
    fn cancel(&mut self, id: NativeSubmissionId) -> Result<(), NativeTransportError>;
    fn close(&mut self) -> Result<(), NativeTransportError>;
}

pub struct UnsupportedNativeSessionTransport;

impl NativeSessionTransport for UnsupportedNativeSessionTransport {
    fn open(&mut self, _path: &NativeDriverPath) -> Result<(), NativeTransportError> {
        Err(NativeTransportError::UnsupportedPlatform)
    }
    fn submit(
        &mut self,
        _operation: NativeOperation,
    ) -> Result<NativeSubmissionId, NativeTransportError> {
        Err(NativeTransportError::UnsupportedPlatform)
    }
    fn poll(
        &mut self,
        _id: NativeSubmissionId,
        _wait: Duration,
    ) -> Result<NativeCompletion, NativeTransportError> {
        Err(NativeTransportError::UnsupportedPlatform)
    }
    fn cancel(&mut self, _id: NativeSubmissionId) -> Result<(), NativeTransportError> {
        Err(NativeTransportError::UnsupportedPlatform)
    }
    fn close(&mut self) -> Result<(), NativeTransportError> {
        Err(NativeTransportError::UnsupportedPlatform)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSubmission {
    id: NativeSubmissionId,
    operation: NativeOperation,
}

impl NativeSubmission {
    pub const fn id(&self) -> NativeSubmissionId {
        self.id
    }

    pub fn operation(&self) -> &NativeOperation {
        &self.operation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSessionState {
    Discovered,
    Opening { attempt: u8 },
    Connected,
    Disconnecting,
    Closed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeRetryPlan {
    pub attempt: u8,
    pub delay: Duration,
    pub exhausted: bool,
}

/// Discovery (`0xACBC0`) has a different retry policy from opening an
/// already discovered handle (`0x1101D0`). Its counter starts at one, with
/// delays 4s, 8s, then 10s, and the tenth failed probe is terminal.
pub fn native_filter_probe_retry_plan(attempt: u8) -> NativeRetryPlan {
    NativeRetryPlan {
        attempt,
        delay: Duration::from_secs((2u64 << attempt.min(3)).min(10)),
        exhausted: attempt >= 10,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeSessionError {
    Route(NativeRouteError),
    Transport(NativeTransportError),
    InvalidState {
        state: NativeSessionState,
        operation: &'static str,
    },
    UnknownSubmission(NativeSubmissionId),
    RetryExhausted,
    CleanupRequired,
}

impl fmt::Display for NativeSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Route(error) => error.fmt(f),
            Self::Transport(error) => error.fmt(f),
            Self::InvalidState { state, operation } => {
                write!(
                    f,
                    "cannot {operation} while mapping session is in {state:?} state"
                )
            }
            Self::UnknownSubmission(id) => write!(f, "unknown mapping session submission {}", id.0),
            Self::RetryExhausted => f.write_str("mapping driver connection retry limit reached"),
            Self::CleanupRequired => f.write_str(
                "mapping driver retry requires the previous handle and operations to be closed",
            ),
        }
    }
}

impl Error for NativeSessionError {}

impl From<NativeTransportError> for NativeSessionError {
    fn from(value: NativeTransportError) -> Self {
        Self::Transport(value)
    }
}

impl From<NativeRouteError> for NativeSessionError {
    fn from(value: NativeRouteError) -> Self {
        Self::Route(value)
    }
}

#[derive(Clone, Debug)]
struct PendingOperation {
    cancel_requested: bool,
    cancel_error: Option<NativeTransportError>,
}

/// A route-bound lifecycle controller.  It owns no Windows handle and can be
/// driven by a pure simulated transport in tests.
pub struct NativeSession<T> {
    route: NativeRoute,
    expected_product: NativeProductIdentity,
    required_capabilities: NativeCapabilitySet,
    transport: T,
    state: NativeSessionState,
    pending: BTreeMap<NativeSubmissionId, PendingOperation>,
    input_stream: Option<NativeSubmissionId>,
    handle_open: bool,
    retry_count: u8,
    retry_plan: Option<NativeRetryPlan>,
    last_error: Option<NativeSessionError>,
    cleanup_errors: Vec<NativeTransportError>,
}

impl<T: NativeSessionTransport> NativeSession<T> {
    pub fn new(
        route: NativeRoute,
        expected_product: NativeProductIdentity,
        required_capabilities: NativeCapabilitySet,
        transport: T,
    ) -> Result<Self, NativeSessionError> {
        route.validate(expected_product, required_capabilities)?;
        Ok(Self {
            route,
            expected_product,
            required_capabilities,
            transport,
            state: NativeSessionState::Discovered,
            pending: BTreeMap::new(),
            input_stream: None,
            handle_open: false,
            retry_count: 0,
            retry_plan: None,
            last_error: None,
            cleanup_errors: Vec::new(),
        })
    }

    pub fn route(&self) -> &NativeRoute {
        &self.route
    }

    /// Access the adapter for event-loop integration.  The adapter remains
    /// responsible for retaining native buffers until final completion.
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub const fn expected_product(&self) -> NativeProductIdentity {
        self.expected_product
    }

    pub const fn required_capabilities(&self) -> NativeCapabilitySet {
        self.required_capabilities
    }

    pub const fn state(&self) -> NativeSessionState {
        self.state
    }

    pub fn last_error(&self) -> Option<&NativeSessionError> {
        self.last_error.as_ref()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn retry_plan(&self) -> Option<&NativeRetryPlan> {
        self.retry_plan.as_ref()
    }

    pub fn input_stream_submission(&self) -> Option<NativeSubmissionId> {
        self.input_stream
    }

    pub fn cleanup_errors(&self) -> &[NativeTransportError] {
        &self.cleanup_errors
    }

    /// Open the exact discovered path and arm the persistent read stream.
    /// Connected means a handle and accepted stream, not a received input
    /// event or a device acknowledgement. Reads can remain pending forever
    /// while independent output operations are submitted.
    pub fn open(&mut self) -> Result<NativeSubmission, NativeSessionError> {
        if self.state != NativeSessionState::Discovered {
            return Err(NativeSessionError::InvalidState {
                state: self.state,
                operation: "open",
            });
        }
        let attempt = self.retry_count.saturating_add(1);
        self.state = NativeSessionState::Opening { attempt };
        if let Err(error) = self.transport.open(self.route.path()) {
            return Err(self.fail_connection(attempt, error.into()));
        }
        self.handle_open = true;
        let operation = NativeOperation::read_input();
        let id = match self.transport.submit(operation.clone()) {
            Ok(id) => id,
            Err(error) => {
                // No request was accepted, so close the synchronously opened
                // handle before exposing the connection failure.
                match self.transport.close() {
                    Ok(()) => self.handle_open = false,
                    Err(error) => self.cleanup_errors.push(error),
                }
                return Err(self.fail_connection(attempt, error.into()));
            }
        };
        self.input_stream = Some(id);
        let submission = NativeSubmission { id, operation };
        self.pending.insert(
            id,
            PendingOperation {
                cancel_requested: false,
                cancel_error: None,
            },
        );
        self.retry_plan = None;
        self.retry_count = 0;
        self.last_error = None;
        self.state = NativeSessionState::Connected;
        Ok(submission)
    }

    /// Retry is explicit.  A failed open never becomes connected merely
    /// because a delay elapsed; the caller must invoke this method and then
    /// successfully open the handle and arm a fresh input read.
    pub fn retry(&mut self) -> Result<NativeSubmission, NativeSessionError> {
        if self.state != NativeSessionState::Failed {
            return Err(NativeSessionError::InvalidState {
                state: self.state,
                operation: "retry",
            });
        }
        if self.handle_open || !self.pending.is_empty() {
            return Err(NativeSessionError::CleanupRequired);
        }
        let Some(plan) = self.retry_plan.clone() else {
            return Err(NativeSessionError::RetryExhausted);
        };
        if plan.exhausted {
            return Err(NativeSessionError::RetryExhausted);
        }
        self.state = NativeSessionState::Discovered;
        self.open()
    }

    /// Observe a stream event and rearm the next read before returning the
    /// completed bytes, as in `0x114850`. Pending reads leave the handle
    /// connected. A terminal stream error permits explicit disconnect and
    /// does not manufacture a connection retry or event.
    pub fn poll_input(&mut self, wait: Duration) -> Result<NativeInputPoll, NativeSessionError> {
        if self.state != NativeSessionState::Connected {
            return Err(NativeSessionError::InvalidState {
                state: self.state,
                operation: "poll_input",
            });
        }
        let id = self.input_stream.ok_or(NativeSessionError::InvalidState {
            state: self.state,
            operation: "poll_input",
        })?;
        match self.poll_operation(id, wait)? {
            NativeCompletion::Pending => Ok(NativeInputPoll::Pending),
            NativeCompletion::Completed(Ok(output)) => {
                let rearmed = self.transport.submit(NativeOperation::read_input());
                match &rearmed {
                    Ok(id) => {
                        self.input_stream = Some(*id);
                        self.pending.insert(
                            *id,
                            PendingOperation {
                                cancel_requested: false,
                                cancel_error: None,
                            },
                        );
                    }
                    Err(error) => self.fail_stream(error.clone()),
                }
                Ok(NativeInputPoll::Completed { output, rearmed })
            }
            NativeCompletion::Completed(Err(error)) => {
                self.fail_stream(error.clone());
                Ok(NativeInputPoll::Failed(error))
            }
        }
    }

    pub fn submit_ioctl(
        &mut self,
        ioctl: u32,
        input: impl Into<Vec<u8>>,
        output_bytes: usize,
    ) -> Result<NativeSubmission, NativeSessionError> {
        if self.state != NativeSessionState::Connected {
            return Err(NativeSessionError::InvalidState {
                state: self.state,
                operation: "submit",
            });
        }
        self.submit_operation(NativeOperation::ioctl(ioctl, input, output_bytes))
    }

    pub fn submit_input_report(
        &mut self,
        report: crate::mapping_native_output::DriverInputReport,
    ) -> Result<NativeSubmission, NativeSessionError> {
        self.submit_ioctl(DRIVER_SEND_INPUT_IOCTL, report.bytes().to_vec(), 0)
    }

    fn submit_operation(
        &mut self,
        operation: NativeOperation,
    ) -> Result<NativeSubmission, NativeSessionError> {
        let id = self.transport.submit(operation.clone())?;
        let submission = NativeSubmission { id, operation };
        self.pending.insert(
            id,
            PendingOperation {
                cancel_requested: false,
                cancel_error: None,
            },
        );
        Ok(submission)
    }

    pub fn poll_submission(
        &mut self,
        id: NativeSubmissionId,
        wait: Duration,
    ) -> Result<NativeCompletion, NativeSessionError> {
        if self.input_stream == Some(id) {
            return Err(NativeSessionError::InvalidState {
                state: self.state,
                operation: "poll the persistent input read using poll_input",
            });
        }
        self.poll_operation(id, wait)
    }

    fn poll_operation(
        &mut self,
        id: NativeSubmissionId,
        wait: Duration,
    ) -> Result<NativeCompletion, NativeSessionError> {
        if !self.pending.contains_key(&id) {
            return Err(NativeSessionError::UnknownSubmission(id));
        }
        // Keep the entry in `pending` when the adapter only reports an
        // observation error.  The kernel request still owns its buffers.
        let completion = self.transport.poll(id, wait)?;
        if matches!(completion, NativeCompletion::Completed(_)) {
            self.pending.remove(&id);
            if self.input_stream == Some(id) {
                self.input_stream = None;
            }
        }
        Ok(completion)
    }

    pub fn cancel_submission(&mut self, id: NativeSubmissionId) -> Result<(), NativeSessionError> {
        let pending = self
            .pending
            .get_mut(&id)
            .ok_or(NativeSessionError::UnknownSubmission(id))?;
        if pending.cancel_requested {
            return Ok(());
        }
        if let Err(error) = self.transport.cancel(id) {
            pending.cancel_error = Some(error.clone());
            self.cleanup_errors.push(error.clone());
            return Err(NativeSessionError::Transport(error));
        }
        pending.cancel_requested = true;
        Ok(())
    }

    pub fn begin_disconnect(&mut self) -> Result<(), NativeSessionError> {
        match self.state {
            NativeSessionState::Connected
            | NativeSessionState::Failed
            | NativeSessionState::Discovered => {
                self.state = NativeSessionState::Disconnecting;
                let ids: Vec<_> = self.pending.keys().copied().collect();
                for id in ids {
                    let _ = self.cancel_submission(id);
                }
                Ok(())
            }
            NativeSessionState::Disconnecting => Ok(()),
            state => Err(NativeSessionError::InvalidState {
                state,
                operation: "disconnect",
            }),
        }
    }

    /// Drain every accepted operation before closing the transport.  A
    /// `Pending` result keeps the session in `Disconnecting`; no close is
    /// attempted while outstanding kernel operations remain.
    pub fn poll_disconnect(
        &mut self,
        wait: Duration,
    ) -> Result<NativeCompletion, NativeSessionError> {
        if self.state != NativeSessionState::Disconnecting {
            return Err(NativeSessionError::InvalidState {
                state: self.state,
                operation: "poll_disconnect",
            });
        }
        let ids: Vec<_> = self.pending.keys().copied().collect();
        for id in ids {
            let canceled = self.pending[&id].cancel_requested;
            if let NativeCompletion::Completed(Err(error)) = self.poll_operation(id, wait)? {
                if !canceled || error != (NativeTransportError::Poll { code: 995 }) {
                    self.cleanup_errors.push(error);
                }
            }
        }
        if !self.pending.is_empty() {
            return Ok(NativeCompletion::Pending);
        }
        if self.handle_open {
            self.transport.close()?;
            self.handle_open = false;
        }
        self.state = NativeSessionState::Closed;
        Ok(NativeCompletion::Completed(
            match self.cleanup_errors.first() {
                Some(error) => Err(error.clone()),
                None => Ok(Vec::new()),
            },
        ))
    }

    fn fail_stream(&mut self, error: NativeTransportError) {
        self.state = NativeSessionState::Failed;
        self.input_stream = None;
        self.retry_plan = None;
        self.last_error = Some(error.into());
    }

    fn fail_connection(&mut self, attempt: u8, error: NativeSessionError) -> NativeSessionError {
        self.state = NativeSessionState::Failed;
        self.input_stream = None;
        self.last_error = Some(error.clone());
        self.retry_count = attempt;
        let exhausted = attempt > MAX_CONNECT_RETRIES;
        let seconds = 1u64.checked_shl(attempt.min(63) as u32).unwrap_or(u64::MAX);
        self.retry_plan = Some(NativeRetryPlan {
            attempt,
            delay: Duration::from_secs(seconds),
            exhausted,
        });
        error
    }
}

#[cfg(test)]
#[path = "mapping_native_session_tests.rs"]
mod tests;

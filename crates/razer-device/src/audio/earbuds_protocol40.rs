//! PROTOCOL40_EARBUDS (`rzDevice40`, library 0.0.340) Feature Report exchange
//! and the two statically closed controls that use it: audio auto-pause
//! (2657/2660/2664/2668/2689) and PID2657 ANC/Ambient/Reduce.
//!
//! Source: `.analysis/results/frontend/audio-mw-audit.json` sections
//! `auto_pause_chain`, `auto_pause_ui_consumers`, `auto_pause_hid_chain` and
//! `headset_2657_anc_chain`. The request is Report ID 2 plus a 63-byte body;
//! the reply is consumed only after transaction/class/id and status checks.
//! A sent byte count is never treated as a device acknowledgement.
use crate::backend::{ExchangeLease, ExchangeOptions, FeatureTransport};
use anyhow::{Context as _, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const REPORT_ID: u8 = 2;
/// `reportLength`; the protocol body is one byte shorter.
pub const REPORT_LENGTH: usize = 64;
const BODY_LENGTH: usize = REPORT_LENGTH - 1;
const MAX_RETRY_OUT: usize = 20;
const MAX_RETRY_IN: usize = 10;
const SLEEP_OUT_TO_IN: Duration = Duration::from_millis(100);
const SLEEP_BETWEEN_IN: Duration = Duration::from_millis(50);
/// Profile ID passed by every audited earbuds helper call.
pub const SOURCE_PROFILE_ID: u8 = 1;

/// Products whose current MW registers `audioAutoPauseControlFeature`.
pub const AUTO_PAUSE_PRODUCTS: [u32; 5] = [2657, 2660, 2664, 2668, 2689];
/// Only PID2657 registers `activeNoiseCancellation{supportsANCLevel:true}`.
pub const ANC_PRODUCT: u32 = 2657;

pub fn supports_auto_pause(product_id: u32) -> bool {
    AUTO_PAUSE_PRODUCTS.contains(&product_id)
}

pub fn supports_anc(product_id: u32) -> bool {
    product_id == ANC_PRODUCT
}

/// `ne.KL` device types used for the transaction high bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransactionDeviceType {
    Mouse = 0,
    AudioHeadsetSpeakerMic = 3,
    DockDongle = 7,
}

/// `_getTransactionId`: wraps at 31 back to 0 before incrementing, so the
/// low five bits cycle 1..=31.
#[derive(Debug, Default)]
pub struct TransactionCounter {
    current: u8,
}

impl TransactionCounter {
    pub fn next(&mut self, device_type: TransactionDeviceType) -> u8 {
        if self.current == 31 {
            self.current = 0;
        }
        self.current += 1;
        self.current | ((device_type as u8) << 5)
    }
}

/// A source command header `[declared size, class, id]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandHeader {
    pub declared_size: u8,
    pub class: u8,
    pub id: u8,
}

impl CommandHeader {
    pub const fn new(bytes: [u8; 3]) -> Self {
        Self {
            declared_size: bytes[0],
            class: bytes[1],
            id: bytes[2],
        }
    }
}

pub const GET_AUTO_PAUSE: CommandHeader = CommandHeader::new([1, 24, 158]);
pub const SET_AUTO_PAUSE: CommandHeader = CommandHeader::new([2, 24, 30]);
pub const GET_ANC: CommandHeader = CommandHeader::new([1, 24, 134]);
pub const SET_ANC: CommandHeader = CommandHeader::new([3, 24, 6]);

/// `_createDataSend` with the default `packetSizeSet=-1`, prefixed by the
/// Report ID that Electron sends with the 63-byte body.
pub fn encode_request(
    header: CommandHeader,
    params: &[u8],
    transaction: u8,
) -> Result<[u8; REPORT_LENGTH]> {
    if params.len() > BODY_LENGTH - 8 {
        bail!("PROTOCOL40 parameters exceed the 63-byte body");
    }
    let mut body = [0u8; BODY_LENGTH];
    body[1] = transaction;
    // Size bytes come from header[0]; values above 255 do not occur here.
    body[5] = header.declared_size;
    body[6] = header.class;
    body[7] = header.id;
    body[8..8 + params.len()].copy_from_slice(params);
    // `_calculateChecksum` XORs indices 2..88; past the body it reads
    // undefined, which coerces to 0, so only 2..63 contribute.
    let checksum = body[2..].iter().fold(0u8, |acc, byte| acc ^ byte);
    body[BODY_LENGTH - 2] = checksum;
    body[BODY_LENGTH - 1] = 0;
    let mut report = [0u8; REPORT_LENGTH];
    report[0] = REPORT_ID;
    report[1..].copy_from_slice(&body);
    Ok(report)
}

/// `_getUSBTransferInResult` status byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyStatus {
    NoResponse,
    Busy,
    Success,
    Failed,
    Timeout,
    NotSupported,
    Unknown(u8),
}

impl ReplyStatus {
    fn from_byte(byte: u8) -> Self {
        match byte {
            0 => Self::NoResponse,
            1 => Self::Busy,
            2 => Self::Success,
            3 => Self::Failed,
            4 => Self::Timeout,
            5 => Self::NotSupported,
            other => Self::Unknown(other),
        }
    }
}

/// Successful reply payload: `rawData.slice(8, 8 + l[5])`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum ExchangeError {
    Status {
        class: u8,
        id: u8,
        status: ReplyStatus,
    },
    NoMatchingReply {
        class: u8,
        id: u8,
    },
    ShortSend {
        received: usize,
    },
}

impl std::fmt::Display for ExchangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status { class, id, status } => {
                write!(f, "PROTOCOL40 command {class}/{id} failed: {status:?}")
            }
            Self::NoMatchingReply { class, id } => {
                write!(
                    f,
                    "PROTOCOL40 command {class}/{id} received no matching reply"
                )
            }
            Self::ShortSend { received } => {
                write!(
                    f,
                    "PROTOCOL40 send returned {received} bytes, expected {REPORT_LENGTH}"
                )
            }
        }
    }
}

impl std::error::Error for ExchangeError {}

/// Injected so mock-transport tests run without wall-clock sleeps.
pub trait Sleeper {
    fn sleep(&self, duration: Duration);
}

pub struct ThreadSleeper;

impl Sleeper for ThreadSleeper {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// One source `sendCommand` cycle per admitted lease. `validate` carries the
/// caller's device identity and generation: a stale or canceled owner is
/// rejected before admission, across the exchange and before the result is
/// returned.
pub struct Protocol40Session<'a, S: Sleeper = ThreadSleeper> {
    transport: &'a dyn FeatureTransport,
    transactions: TransactionCounter,
    next_transaction: Option<&'a mut dyn FnMut(TransactionDeviceType) -> Result<u8>>,
    device_type: TransactionDeviceType,
    sleeper: S,
}

impl<'a> Protocol40Session<'a, ThreadSleeper> {
    pub fn new(transport: &'a dyn FeatureTransport) -> Self {
        Self::with_sleeper(transport, ThreadSleeper)
    }
}

impl<'a, S: Sleeper> Protocol40Session<'a, S> {
    /// Auto-pause and ANC use the instance type `AudioHeadsetSpeakerMic`.
    pub fn with_sleeper(transport: &'a dyn FeatureTransport, sleeper: S) -> Self {
        Self {
            transport,
            transactions: TransactionCounter::default(),
            next_transaction: None,
            device_type: TransactionDeviceType::AudioHeadsetSpeakerMic,
            sleeper,
        }
    }

    /// Production retains the source counter across calls on one device owner.
    pub fn with_transaction_source(
        transport: &'a dyn FeatureTransport,
        sleeper: S,
        next: &'a mut dyn FnMut(TransactionDeviceType) -> Result<u8>,
    ) -> Self {
        let mut session = Self::with_sleeper(transport, sleeper);
        session.next_transaction = Some(next);
        session
    }

    pub fn send_command(
        &mut self,
        header: CommandHeader,
        params: &[u8],
        validate: &dyn Fn() -> Result<()>,
    ) -> Result<Reply> {
        self.send_command_as(header, params, self.device_type, validate)
    }

    pub fn send_command_as(
        &mut self,
        header: CommandHeader,
        params: &[u8],
        device_type: TransactionDeviceType,
        validate: &dyn Fn() -> Result<()>,
    ) -> Result<Reply> {
        self.send_command_inner(header, params, device_type, validate, None)
    }

    pub(crate) fn send_command_observing(
        &mut self,
        header: CommandHeader,
        params: &[u8],
        validate: &dyn Fn() -> Result<()>,
        observe_send: &dyn Fn(&[u8]),
    ) -> Result<Reply> {
        self.send_command_inner(
            header,
            params,
            self.device_type,
            validate,
            Some(observe_send),
        )
    }

    fn send_command_inner(
        &mut self,
        header: CommandHeader,
        params: &[u8],
        device_type: TransactionDeviceType,
        validate: &dyn Fn() -> Result<()>,
        observe_send: Option<&dyn Fn(&[u8])>,
    ) -> Result<Reply> {
        let transaction = match &mut self.next_transaction {
            Some(next) => next(device_type)?,
            None => self.transactions.next(device_type),
        };
        let request = encode_request(header, params, transaction)?;
        validate().map_err(|error| {
            crate::backend::interrupt_exchange(
                error,
                crate::backend::ExchangeInterruption::Validation,
            )
        })?;
        let lease = self
            .transport
            .begin_exchange_validated(ExchangeOptions::plain(), std::rc::Rc::new(validate))
            .map_err(|error| {
                crate::backend::interrupt_exchange(
                    error,
                    crate::backend::ExchangeInterruption::Admission,
                )
            })?;
        let lease = if let Some(observe) = observe_send {
            crate::backend::checked_exchange_observing_send(
                lease,
                validate,
                validate,
                Some(REPORT_LENGTH),
                observe,
            )
        } else {
            lease
        };
        let result = exchange(
            lease.transport(),
            lease.as_ref(),
            &self.sleeper,
            &request,
            header,
        )
        .map(|reply| reply.data);
        crate::feature_exchange::finish_feature_reply(
            lease,
            result,
            validate(),
            [header.declared_size, header.class, header.id],
            transaction,
        )
        .map(|data| Reply { data })
    }
}

fn exchange<S: Sleeper>(
    transport: &dyn FeatureTransport,
    lease: &dyn ExchangeLease,
    sleeper: &S,
    request: &[u8; REPORT_LENGTH],
    header: CommandHeader,
) -> Result<Reply> {
    let mut last: Result<Reply> = Err(ExchangeError::NoMatchingReply {
        class: header.class,
        id: header.id,
    }
    .into());
    // The source builds the packet once; every OUT retry resends the same
    // bytes, including the transaction ID.
    for attempt in 0..MAX_RETRY_OUT {
        if attempt > 0 {
            lease.retry_release()?;
        }
        let sent = transport
            .send_feature_counted(request)
            .context("PROTOCOL40 Feature Report send")?;
        if sent != REPORT_LENGTH {
            last = Err(ExchangeError::ShortSend { received: sent }.into());
            continue;
        }
        sleeper.sleep(SLEEP_OUT_TO_IN);
        let (result, resend) = read_reply(transport, sleeper, request, header);
        match result {
            Ok(reply) => return Ok(reply),
            Err(error) if !resend => return Err(error),
            Err(error) => last = Err(error),
        }
    }
    last
}

/// Returns the IN outcome and the source `resendOutCommand` flag.
fn read_reply<S: Sleeper>(
    transport: &dyn FeatureTransport,
    sleeper: &S,
    request: &[u8; REPORT_LENGTH],
    header: CommandHeader,
) -> (Result<Reply>, bool) {
    let sent = &request[1..];
    let mut resend = false;
    let mut outcome: Result<Reply> = Err(ExchangeError::NoMatchingReply {
        class: header.class,
        id: header.id,
    }
    .into());
    for attempt in 0..MAX_RETRY_IN {
        if attempt > 0 {
            sleeper.sleep(SLEEP_BETWEEN_IN);
        }
        let mut report = [0u8; REPORT_LENGTH];
        report[0] = REPORT_ID;
        let count = match transport.get_feature(&mut report) {
            Ok(count) => count,
            Err(error) => {
                let accepted = crate::backend::native_read_bytes(&error)
                    .and_then(|bytes| success_reply(bytes, request));
                let error = if let Some(reply) = accepted {
                    crate::feature_exchange::CompletedFeatureExchange::retain(
                        error,
                        [header.declared_size, header.class, header.id],
                        request[2],
                        reply.data,
                    )
                } else {
                    error
                };
                return (Err(error.context("PROTOCOL40 Feature Report read")), resend);
            }
        };
        // The Host strips the Report ID; an empty read becomes a zero array.
        let mut body = [0u8; BODY_LENGTH];
        if count > 1 {
            let end = count.min(REPORT_LENGTH);
            body[..end - 1].copy_from_slice(&report[1..end]);
        }
        if body[1] != sent[1] {
            resend = true;
            // retryTransactionIdMismatch is enabled for these products.
            continue;
        }
        if body[6] != sent[6] || body[7] != sent[7] {
            return (outcome, true);
        }
        let status = ReplyStatus::from_byte(body[0]);
        let error = |status| -> Result<Reply> {
            Err(ExchangeError::Status {
                class: header.class,
                id: header.id,
                status,
            }
            .into())
        };
        match status {
            ReplyStatus::Success => {
                let accepted = report
                    .get(..count)
                    .and_then(|bytes| success_reply(bytes, request));
                if accepted.is_none() {
                    return (
                        Err(anyhow!(
                            "PROTOCOL40 success payload is not backed by the received report bytes"
                        )),
                        false,
                    );
                }
                return (Ok(accepted.expect("checked successful reply")), false);
            }
            ReplyStatus::Busy => {
                resend = true;
                outcome = error(status);
            }
            ReplyStatus::NoResponse | ReplyStatus::Failed | ReplyStatus::Timeout => {
                return (error(status), true);
            }
            ReplyStatus::NotSupported | ReplyStatus::Unknown(_) => return (error(status), false),
        }
    }
    (outcome, resend)
}

fn success_reply(report: &[u8], request: &[u8; REPORT_LENGTH]) -> Option<Reply> {
    if !(9..=REPORT_LENGTH).contains(&report.len())
        || report[0] != REPORT_ID
        || report[1] != 2
        || report[2] != request[2]
        || report[7..9] != request[7..9]
    {
        return None;
    }
    let length = usize::from(report[6]);
    if length > BODY_LENGTH - 8 || report.len() < 9 + length {
        return None;
    }
    Some(Reply {
        data: report[9..9 + length].to_vec(),
    })
}

// ---------------------------------------------------------------- auto-pause

/// Successful `getAudioAutoPauseStatus(1)` / `setAudioAutoPauseStatus` reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoPauseReply {
    pub profile_id: u8,
    pub enabled: bool,
}

fn parse_profile_state(reply: &Reply) -> Result<(u8, u8)> {
    match reply.data.as_slice() {
        [profile, state, ..] => Ok((*profile, *state)),
        _ => Err(anyhow!("PROTOCOL40 reply is shorter than profile/state")),
    }
}

impl<S: Sleeper> Protocol40Session<'_, S> {
    pub fn get_auto_pause(&mut self, validate: &dyn Fn() -> Result<()>) -> Result<AutoPauseReply> {
        let reply = self.send_command(GET_AUTO_PAUSE, &[SOURCE_PROFILE_ID], validate)?;
        let (profile_id, state) = parse_profile_state(&reply)?;
        Ok(AutoPauseReply {
            profile_id,
            enabled: decode_toggle(state)?,
        })
    }

    /// The source setter consumes only the status; it does not read back.
    pub fn set_auto_pause(
        &mut self,
        enabled: bool,
        validate: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        self.send_command(
            SET_AUTO_PAUSE,
            &[SOURCE_PROFILE_ID, u8::from(enabled)],
            validate,
        )
        .map(|_| ())
    }
}

/// Device observation for a boolean earbud control. Failure never establishes
/// an observed off value, regardless of the source facade's boolean fallback.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum ToggleObservation {
    #[default]
    Unobserved,
    Observed(bool),
    Unknown(u8),
    ReadFailed(String),
}

#[derive(Debug)]
struct UnknownToggle(u8);
impl std::fmt::Display for UnknownToggle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Unknown earbud boolean state {}", self.0)
    }
}
impl std::error::Error for UnknownToggle {}

pub(crate) fn decode_toggle(value: u8) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(UnknownToggle(value).into()),
    }
}

impl ToggleObservation {
    pub(crate) fn from_read_error(error: &anyhow::Error) -> Self {
        match error.downcast_ref::<UnknownToggle>() {
            Some(UnknownToggle(value)) => Self::Unknown(*value),
            None => Self::ReadFailed(format!("{error:#}")),
        }
    }

    /// Diagnostic text accompanies the typed state; unknown bytes remain distinct
    /// from failed transport and cannot establish an observed boolean value.
    pub fn diagnostic(&self) -> Option<String> {
        match self {
            Self::Unknown(value) => Some(UnknownToggle(*value).to_string()),
            Self::ReadFailed(error) => Some(error.clone()),
            Self::Unobserved | Self::Observed(_) => None,
        }
    }
}

pub type AutoPauseObservation = ToggleObservation;

impl AutoPauseObservation {
    /// Successful restore reads compare the observed boolean with local intent.
    /// Failed reads are rejected before reaching this comparison.
    fn source_compare_value(&self) -> bool {
        matches!(self, Self::Observed(true))
    }
}

/// Factory default and missing-field fallback in `updateStorage` and
/// `restoreDeviceData`: `{isEnabled:true}`.
pub const AUTO_PAUSE_PROFILE_DEFAULT: bool = true;

/// `audioAutoPauseControl` in the selected local profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutoPauseProfileField {
    pub is_enabled: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum WriteOutcome {
    Skipped,
    Confirmed,
    /// The source setter returned false; UI is still updated from the profile.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ToggleRestore {
    pub observation: AutoPauseObservation,
    pub write: WriteOutcome,
    /// `MW_UPDATE_AUDIO_AUTO_PAUSE_CONTROL_TO_UI` payload: the local value.
    pub published_local: bool,
}

pub type AutoPauseRestore = ToggleRestore;

/// `restoreDeviceData`: read, compare, write only on difference, then publish
/// the local profile value regardless of the setter result.
pub fn restore_auto_pause<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    field: AutoPauseProfileField,
    validate: &dyn Fn() -> Result<()>,
) -> Result<AutoPauseRestore> {
    let local = field.is_enabled.unwrap_or(AUTO_PAUSE_PROFILE_DEFAULT);
    let observation = match session.get_auto_pause(validate) {
        Ok(reply) => AutoPauseObservation::Observed(reply.enabled),
        Err(error) => {
            reject_if_stale(validate, &error)?;
            AutoPauseObservation::from_read_error(&error)
        }
    };
    if let Some(error) = observation.diagnostic() {
        return Ok(AutoPauseRestore {
            observation: observation.clone(),
            write: WriteOutcome::Failed(format!(
                "Restore was not submitted because the read failed: {error}"
            )),
            published_local: local,
        });
    }
    let write = if observation.source_compare_value() == local {
        WriteOutcome::Skipped
    } else {
        match session.set_auto_pause(local, validate) {
            Ok(()) => WriteOutcome::Confirmed,
            Err(error) => {
                reject_if_stale(validate, &error)?;
                WriteOutcome::Failed(format!("{error:#}"))
            }
        }
    };
    validate()?;
    Ok(AutoPauseRestore {
        observation,
        write,
        published_local: local,
    })
}

/// A device switch or cancellation aborts the whole chain instead of being
/// recorded as an ordinary device failure.
fn reject_if_stale(validate: &dyn Fn() -> Result<()>, error: &anyhow::Error) -> Result<()> {
    validate().map_err(|stale| stale.context(format!("{error:#}")))
}

// ---------------------------------------------------------------------- ANC

/// Page modes `ANC 10` / `Ambient 255` (`ancStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum AncMode {
    Anc,
    Ambient,
}

impl AncMode {
    pub fn ui_status(self) -> u8 {
        match self {
            Self::Anc => 10,
            Self::Ambient => 255,
        }
    }
}

/// Reduce toggles the level between 1 and 10.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum AncLevel {
    Reduced = 1,
    Full = 10,
}

/// `{isEnabled, status, level}` as held by the feature and profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AncSetting {
    pub enabled: bool,
    pub mode: AncMode,
    pub level: AncLevel,
}

/// Local default profile `true/ANC10/level10`. This is local state only:
/// current 2657 sets no `initFromDevice`, so it is never a device reading.
pub const ANC_PROFILE_DEFAULT: AncSetting = AncSetting {
    enabled: true,
    mode: AncMode::Anc,
    level: AncLevel::Full,
};

/// UI reducer initial state `false/ANC10/level10`.
pub const ANC_UI_INITIAL: AncSetting = AncSetting {
    enabled: false,
    ..ANC_PROFILE_DEFAULT
};

impl AncSetting {
    /// Firmware state: disabled 0, ANC 1, Ambient 80.
    pub fn firmware_state(self) -> u8 {
        match (self.enabled, self.mode) {
            (false, _) => 0,
            (true, AncMode::Anc) => 1,
            (true, AncMode::Ambient) => 80,
        }
    }

    pub fn set_params(self) -> [u8; 6] {
        // Allocated with header[2] = 6 bytes; the tail stays zero.
        [
            SOURCE_PROFILE_ID,
            self.firmware_state(),
            self.level as u8,
            0,
            0,
            0,
        ]
    }
}

/// Partial ANC payload from the page; the mounted reducer merges it into the
/// previous state and posts the full triple.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AncChange {
    pub enabled: Option<bool>,
    pub mode: Option<AncMode>,
    pub level: Option<AncLevel>,
}

/// The feature's local `runtimeState` skip cache starts from its source default.
/// The independent owner advances it after a confirmed write; it is not a
/// device observation, especially when the initial default causes a skip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AncRuntimeState(pub AncSetting);

impl Default for AncRuntimeState {
    fn default() -> Self {
        Self(ANC_PROFILE_DEFAULT)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum AncWrite {
    /// Equal to runtimeState: the source skips I/O (restore's force flag is
    /// not consumed).
    Skipped,
    /// The device returned SUCCESS for the write.
    Acknowledged,
    /// Transport or business response failure. Runtime is unchanged so an
    /// identical retry is not skipped.
    Failed(String),
}

pub fn set_anc<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    runtime: &mut AncRuntimeState,
    requested: AncSetting,
    validate: &dyn Fn() -> Result<()>,
) -> Result<AncWrite> {
    let cache = crate::earbuds_anc::Cache::from_runtime(*runtime);
    let result = cache.execute(session, ANC_PRODUCT, requested, validate, validate);
    *runtime = cache.runtime();
    result.map(|report| report.write)
}

/// Hardware notification `[5,33,state]`: only the status byte. The profile
/// keeps its level; runtimeState is not updated. Missing and unknown states
/// cannot overwrite the local profile or become an observed disabled value.
pub fn apply_anc_notification(profile: AncSetting, record: &[u8]) -> Option<AncSetting> {
    if record.get(0) != Some(&5) || record.get(1) != Some(&33) {
        return None;
    }
    let state = *record.get(2)?;
    if !matches!(state, 0 | 1 | 80) {
        return None;
    }
    Some(AncSetting {
        enabled: state != 0,
        mode: if state == 80 {
            AncMode::Ambient
        } else {
            AncMode::Anc
        },
        level: profile.level,
    })
}

/// `minimumFWVersionSupportReduceANC = 1.4.6.0`.
pub const REDUCE_MIN_FIRMWARE: &str = "1.4.6.0";

/// Source `U5(minimum, current)` result: `true` hides/blocks Reduce.
/// Equal complete strings → false. Otherwise semver-coerced comparison where
/// an identical coerced core still blocks (object identity, not equality).
/// Missing/uncoercible current version → false (Reduce not hidden).
pub fn reduce_blocked(current: Option<&str>) -> bool {
    firmware_blocked(REDUCE_MIN_FIRMWARE, current)
}

/// Shared source U5 comparator used by the current earbuds' UI gates.
pub fn firmware_blocked(minimum: &str, current: Option<&str>) -> bool {
    let Some(current) = current else { return false };
    if current == minimum {
        return false;
    }
    let Some(current_core) = coerce_core(current) else {
        return false;
    };
    let Some(minimum_core) = coerce_core(minimum) else {
        return false;
    };
    current_core <= minimum_core
}

/// `semver.coerce`: first `major[.minor[.patch]]` digits run.
fn coerce_core(text: &str) -> Option<(u64, u64, u64)> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let mut parts = text[start..].split('.').map(|part| {
        part.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
    });
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

#[cfg(test)]
#[path = "earbuds_protocol40_tests.rs"]
mod tests;

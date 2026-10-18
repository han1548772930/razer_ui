//! Current THX V4 JSON semantics and six BlackShark spatial submission chains.
//!
//! Source: `docs/re/evidence/thx-source-current.json`. Native property packing:
//! `rea-thx-v4-static-current.json`, SetProperty RVA 0x17de80, inference 0x17dc30,
//! GetProperty 0x17d4d0. This is an OS-independent logical protocol; the external
//! driver/service transport uses the source-audited local RPC/HID adapters.
//! No vendor DLL is loaded here.
//!
//! Source local draft updates precede native submission. A native bool result,
//! getter observation, and acknowledged in-memory state are separate. Getter
//! verification and checks between operations are Rust integration policy;
//! the source checks task cancellation after execute and does no extra getter.

use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::VecDeque;

#[cfg(test)]
#[path = "thx_v4_tests.rs"]
mod tests;

#[path = "thx_v4_lifecycle.rs"]
mod lifecycle;
pub use lifecycle::{
    ThxLifecycleReceipt, ThxRenderSetupOutcome, ThxRenderSetupStatus, ThxRenderSetupTrigger,
};

#[path = "thx_v4_shutdown.rs"]
mod shutdown;
pub use shutdown::{ThxRenderShutdownOutcome, ThxRenderShutdownStatus};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThxPropertyType {
    Bool,
    Int,
    Float,
    String,
    StringArray,
    FloatArray,
    None,
    Explicit(String),
}
impl ThxPropertyType {
    pub fn source_name(&self) -> &str {
        match self {
            Self::Bool => "Bool",
            Self::Int => "Int",
            Self::Float => "Float",
            Self::String => "String",
            Self::StringArray => "StringArray",
            Self::FloatArray => "FloatArray",
            Self::None => "None",
            Self::Explicit(name) => name,
        }
    }
}

/// The recovered helper inspects the first array element, then handles native
/// JSON types 3/4/5/7. Native unsigned (6) has no switch case. Non-negative
/// integer JSON tokens parse as unsigned in the native parser and map to None.
/// Source numeric setters use explicit Float where required; do not normalize
/// None to Int or validate every array element as if the original did so.
pub fn infer_property_type(value: &Value) -> ThxPropertyType {
    let array = value.is_array();
    let inspected = if let Value::Array(values) = value {
        values.first().unwrap_or(value)
    } else {
        value
    };
    match inspected {
        Value::Bool(_) => ThxPropertyType::Bool,
        Value::Number(number) if number.is_f64() => {
            if array {
                ThxPropertyType::FloatArray
            } else {
                ThxPropertyType::Float
            }
        }
        Value::Number(number) if number.as_i64().is_some_and(|n| n < 0) => ThxPropertyType::Int,
        Value::String(_) => {
            if array {
                ThxPropertyType::StringArray
            } else {
                ThxPropertyType::String
            }
        }
        _ => ThxPropertyType::None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThxDeviceIdentity {
    pub product_id: String,
    pub endpoint_id: String,
}
impl ThxDeviceIdentity {
    pub fn new(product_id: impl Into<String>, endpoint_id: impl Into<String>) -> Result<Self> {
        let result = Self {
            product_id: product_id.into(),
            endpoint_id: endpoint_id.into(),
        };
        ensure!(!result.product_id.is_empty(), "THX productId is empty");
        // Rust boundary validation: do not silently truncate an owned string
        // when a later Windows adapter converts it to the observed C-string ABI.
        ensure!(
            !result.product_id.contains('\0') && !result.endpoint_id.contains('\0'),
            "THX identity contains an embedded NUL"
        );
        Ok(result)
    }
    /// The current JS wrapper always supplies endpointId, defaulting to "".
    /// productId is the configured THX string, not the numeric USB product ID.
    pub fn wire_json(&self) -> Value {
        json!({"productId": self.product_id, "endpointId": self.endpoint_id})
    }
    pub fn cache_key(&self) -> String {
        if self.endpoint_id.is_empty() {
            self.product_id.clone()
        } else {
            format!("{}_{}", self.product_id, self.endpoint_id)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThxPropertyRequest {
    pub id: String,
    pub value: Value,
    pub property_type: ThxPropertyType,
}
impl ThxPropertyRequest {
    pub fn new(id: impl Into<String>, value: Value, explicit_type: Option<&str>) -> Result<Self> {
        let id = id.into();
        ensure!(
            !id.contains('\0'),
            "THX property id contains an embedded NUL"
        );
        let property_type = match explicit_type.filter(|name| !name.is_empty()) {
            Some(name) => {
                ensure!(
                    !name.contains('\0'),
                    "THX property type contains an embedded NUL"
                );
                // Native strlen counts UTF-8 bytes and accepts lengths <100.
                ensure!(name.len() < 100, "THX property type is 100 bytes or longer");
                ThxPropertyType::Explicit(name.to_owned())
            }
            None => infer_property_type(&value),
        };
        Ok(Self {
            id,
            value,
            property_type,
        })
    }
    pub fn parse(
        id: impl Into<String>,
        value_json: &str,
        explicit_type: Option<&str>,
    ) -> Result<Self> {
        let value =
            serde_json::from_str(value_json).context("THX property value is invalid JSON")?;
        Self::new(id, value, explicit_type)
    }
    /// The logical object passed to the native vtable method. It is not proof
    /// of the external COM/RPC/driver message encoding.
    pub fn wire_json(&self) -> Value {
        json!({"id": self.id, "value": self.value, "type": self.property_type.source_name()})
    }
    pub fn boolean(id: &str, enabled: bool) -> Self {
        Self {
            id: id.into(),
            value: json!(enabled),
            property_type: ThxPropertyType::Bool,
        }
    }
}

/// GetProperty returns only the contained value as JSON; a missing native
/// property or missing `value` field yields NULL, not an invented default.
pub fn decode_native_property_reply(reply: Option<&str>) -> Result<Option<Value>> {
    reply
        .map(|text| serde_json::from_str(text).context("THX getter returned invalid JSON"))
        .transpose()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ThxV4Capability {
    pub product_id: u32,
    pub thx_product_id: &'static str,
    pub endpoint_id: &'static str,
    pub direct_transport_available: bool,
}
const CAPABILITIES: [ThxV4Capability; 6] = [
    ThxV4Capability {
        product_id: 1398,
        thx_product_id: "razer_blacksharkv3pro_pc",
        endpoint_id: "headphones_game",
        direct_transport_available: cfg!(windows),
    },
    ThxV4Capability {
        product_id: 1401,
        thx_product_id: "razer_blacksharkv3_pc",
        endpoint_id: "headphones_game",
        direct_transport_available: cfg!(windows),
    },
    ThxV4Capability {
        product_id: 2638,
        thx_product_id: "razer_blacksharkv3pro_xbox",
        endpoint_id: "headphones_game",
        direct_transport_available: cfg!(windows),
    },
    ThxV4Capability {
        product_id: 2641,
        thx_product_id: "razer_blacksharkv3_xbox",
        endpoint_id: "headphones_game",
        direct_transport_available: cfg!(windows),
    },
    ThxV4Capability {
        product_id: 4124,
        thx_product_id: "razer_blacksharkv3pro_ps",
        endpoint_id: "headphones_game",
        direct_transport_available: cfg!(windows),
    },
    ThxV4Capability {
        product_id: 4126,
        thx_product_id: "razer_blacksharkv3_ps",
        endpoint_id: "headphones_game",
        direct_transport_available: cfg!(windows),
    },
];
pub fn capability(product_id: u32) -> Option<&'static ThxV4Capability> {
    CAPABILITIES.iter().find(|cap| cap.product_id == product_id)
}

/// Logical calls across the THX/firmware/cloud EQ service boundaries. The
/// firmware/cloud operations are existing source service calls, not fabricated
/// THX DLL exports or guessed HID reports. A real adapter must implement each.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ThxOperation {
    AddDevice,
    RemoveDevice,
    CheckStatus,
    SetProperty(ThxPropertyRequest),
    GetProperty(String),
    SetFirmwarePresetEqEnabled(bool),
    RestoreCloudEqStereo {
        presets: Vec<Value>,
        selected_index: Option<Value>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ThxReply {
    NativeBool(bool),
    /// Rust binding allocation/removal; never a claimed vendor DLL result.
    LocalBinding(bool),
    /// A complete successful local GetVersion RPC, independent of its text.
    /// This Rust readiness policy differs from native GetState(true)==2.
    LocalReady(bool),
    /// Real SetProperty Empty frame + END_STREAM + grpc-status 0.
    RpcAcknowledged,
    /// A complete nonzero gRPC status, distinct from a DLL bool and from an
    /// incomplete/uncertain transport error. It never acknowledges a write.
    RpcRejected {
        code: u32,
        message: Option<String>,
    },
    Property(Option<Value>),
    FirmwareStatus(String),
    CloudEqBool(bool),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThxTransportKind {
    Unavailable,
    Simulated,
    Direct,
}

/// A response arrived before a subsequent identity/cancellation check failed.
/// The reply is diagnostic evidence only; the caller still receives an error
/// and must not continue writes or treat the previous scope as current.
#[derive(Debug)]
pub struct ThxExchangeInterrupted {
    pub reply: ThxReply,
    pub error: String,
}
impl std::fmt::Display for ThxExchangeInterrupted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "THX response received before guard failed: {}",
            self.error
        )
    }
}
impl std::error::Error for ThxExchangeInterrupted {}
pub trait ThxV4Transport {
    fn kind(&self) -> ThxTransportKind;
    fn exchange(
        &mut self,
        identity: &ThxDeviceIdentity,
        operation: &ThxOperation,
    ) -> Result<ThxReply>;
}

pub struct UnavailableThxV4Transport;
impl ThxV4Transport for UnavailableThxV4Transport {
    fn kind(&self) -> ThxTransportKind {
        ThxTransportKind::Unavailable
    }
    fn exchange(&mut self, _: &ThxDeviceIdentity, _: &ThxOperation) -> Result<ThxReply> {
        bail!("THX V4 external driver/service transport is not yet implemented")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThxLocalDraft {
    pub is_enabled: bool,
    pub timer_tick: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpatialTask {
    pub device: ThxDeviceIdentity,
    pub generation: u64,
    pub timer_tick: u64,
    pub requested_enabled: bool,
    pub cloud_presets: Vec<Value>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThxObservationScope {
    pub device: ThxDeviceIdentity,
    pub generation: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThxV4State {
    pub device: ThxDeviceIdentity,
    /// Source optimistic local state only. No device save is implied.
    pub local_draft: Option<ThxLocalDraft>,
    /// The before-hook state is updated before the spatial native setter.
    pub valkyrie_spatial_enabled: bool,
    /// Source-style cache changes after an accepted spatial setter. Extra Rust
    /// getters do not overwrite it or produce the source UI refresh event.
    pub runtime_spatial_enabled: Option<bool>,
    /// Actual getter observation, independent of the submitted/source cache.
    pub observed_spatial_enabled: Option<bool>,
    /// Only set after true setter + matching getter + current identity/generation.
    /// This is acknowledged memory state, not a successful disk/device save.
    pub acknowledged_spatial_enabled: Option<bool>,
    pub last_error: Option<String>,
    pub generation: u64,
    pub ready: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpatialStatus {
    Verified,
    Observed,
    NativeRejected,
    RpcRejected,
    TransportFailed,
    ReadbackFailed,
    ReadbackMismatch,
    Canceled,
    Stale,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpatialOutcome {
    pub status: SpatialStatus,
    pub transport_kind: ThxTransportKind,
    pub generation: u64,
    pub timer_tick: u64,
    pub native_acknowledged: bool,
    pub write_acknowledgement: Option<ThxWriteAcknowledgement>,
    pub readback: Option<bool>,
    pub operations: Vec<ThxOperation>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThxWriteAcknowledgement {
    NativeBool,
    LocalGrpcStatus,
}
impl SpatialOutcome {
    fn new(task: &SpatialTask, transport_kind: ThxTransportKind) -> Self {
        Self {
            status: SpatialStatus::TransportFailed,
            transport_kind,
            generation: task.generation,
            timer_tick: task.timer_tick,
            native_acknowledged: false,
            write_acknowledgement: None,
            readback: None,
            operations: vec![],
            warnings: vec![],
            error: None,
        }
    }
}

pub struct ThxV4Controller<T> {
    transport: T,
    capability: &'static ThxV4Capability,
    state: ThxV4State,
}
impl<T: ThxV4Transport> ThxV4Controller<T> {
    pub fn new(product_id: u32, transport: T) -> Result<Self> {
        let capability = capability(product_id)
            .context("Current THX V4 spatial chain not audited for this owner")?;
        let device = ThxDeviceIdentity::new(capability.thx_product_id, capability.endpoint_id)?;
        Ok(Self {
            transport,
            capability,
            state: ThxV4State {
                device,
                local_draft: None,
                valkyrie_spatial_enabled: false,
                runtime_spatial_enabled: None,
                observed_spatial_enabled: None,
                acknowledged_spatial_enabled: None,
                last_error: None,
                generation: 0,
                ready: false,
            },
        })
    }
    pub fn state(&self) -> &ThxV4State {
        &self.state
    }
    pub fn capability(&self) -> &ThxV4Capability {
        self.capability
    }
    pub fn transport(&self) -> &T {
        &self.transport
    }
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
    /// DeviceReset changes only Valkyrie's before-hook state from the newly
    /// persisted profile. It does not acknowledge a THX or firmware setter.
    pub fn reset_blackshark_local_profile(&mut self) -> Result<()> {
        ensure!(
            self.capability.product_id == crate::blackshark_reset::PRODUCT_ID,
            "Local Help reset is audited only for 1398"
        );
        self.state.valkyrie_spatial_enabled = crate::blackshark_reset::DEFAULT_SPATIAL_ENABLED;
        Ok(())
    }
    fn require_transport(&self) -> Result<()> {
        match self.transport.kind() {
            ThxTransportKind::Simulated => Ok(()),
            ThxTransportKind::Direct if self.capability.direct_transport_available => Ok(()),
            _ => bail!(
                "THX V4 direct external transport remains unavailable; a typed plan is not device support"
            ),
        }
    }

    /// Bind without applying source render initialization settings. Source
    /// AddDevice is followed by at most three CheckStatus attempts. The
    /// scheduler receives the original 500ms retry interval; tests supply a
    /// virtual scheduler and perform no wall-clock/OS/device operations.
    pub fn bind(&mut self, mut wait_retry: impl FnMut(u64) -> Result<()>) -> Result<()> {
        self.require_transport()?;
        self.state.ready = false;
        match self
            .transport
            .exchange(&self.state.device, &ThxOperation::AddDevice)?
        {
            ThxReply::NativeBool(true) | ThxReply::LocalBinding(true) => {}
            _ => bail!("THX AddDevice did not return true"),
        }
        for _ in 0..3 {
            match self
                .transport
                .exchange(&self.state.device, &ThxOperation::CheckStatus)?
            {
                ThxReply::NativeBool(true) | ThxReply::LocalReady(true) => {
                    self.state.ready = true;
                    return Ok(());
                }
                ThxReply::NativeBool(false) | ThxReply::LocalReady(false) => wait_retry(500)?,
                _ => bail!("THX CheckStatus returned the wrong response type"),
            }
        }
        bail!("THX CheckStatus did not become ready after three attempts")
    }

    /// Original local draft stage. It runs before device access and is retained
    /// on native failure. Disk persistence belongs to the caller's local store.
    pub fn prepare_spatial(
        &mut self,
        enabled: bool,
        cloud_presets: Vec<Value>,
        timer_tick: u64,
    ) -> SpatialTask {
        self.state.generation = self.state.generation.wrapping_add(1);
        self.state.local_draft = Some(ThxLocalDraft {
            is_enabled: enabled,
            timer_tick,
        });
        self.state.last_error = None;
        SpatialTask {
            device: self.state.device.clone(),
            generation: self.state.generation,
            timer_tick,
            requested_enabled: enabled,
            cloud_presets,
        }
    }
    pub fn invalidate(&mut self) {
        self.state.generation = self.state.generation.wrapping_add(1);
        self.state.ready = false;
    }
    pub fn cancel(&mut self) {
        self.state.generation = self.state.generation.wrapping_add(1);
    }
    pub fn observation_scope(&self) -> ThxObservationScope {
        ThxObservationScope {
            device: self.state.device.clone(),
            generation: self.state.generation,
        }
    }
    fn current(&self, task: &SpatialTask) -> bool {
        self.state.device == task.device
            && self.state.generation == task.generation
            && self.state.local_draft.as_ref().is_some_and(|draft| {
                draft.timer_tick == task.timer_tick && draft.is_enabled == task.requested_enabled
            })
    }
    fn guarded_exchange(
        &mut self,
        task: &SpatialTask,
        operation: ThxOperation,
        validate: &mut impl FnMut() -> Result<()>,
        outcome: &mut SpatialOutcome,
    ) -> Result<ThxReply> {
        if !self.current(task) {
            outcome.status = SpatialStatus::Stale;
            bail!("THX task identity/generation is stale");
        }
        if let Err(error) = validate() {
            outcome.status = SpatialStatus::Canceled;
            return Err(error);
        }
        outcome.operations.push(operation.clone());
        let reply = self.transport.exchange(&task.device, &operation)?;
        if !self.current(task) {
            outcome.status = SpatialStatus::Stale;
            bail!("THX task became stale during transport");
        }
        if let Err(error) = validate() {
            outcome.status = SpatialStatus::Canceled;
            return Err(error);
        }
        Ok(reply)
    }
    fn native_set(
        &mut self,
        task: &SpatialTask,
        id: &str,
        enabled: bool,
        validate: &mut impl FnMut() -> Result<()>,
        outcome: &mut SpatialOutcome,
    ) -> Result<ThxWriteAcknowledgement> {
        let reply = self.guarded_exchange(
            task,
            ThxOperation::SetProperty(ThxPropertyRequest::boolean(id, enabled)),
            validate,
            outcome,
        )?;
        match reply {
            ThxReply::NativeBool(true) => Ok(ThxWriteAcknowledgement::NativeBool),
            ThxReply::RpcAcknowledged => Ok(ThxWriteAcknowledgement::LocalGrpcStatus),
            ThxReply::NativeBool(false) => {
                outcome.status = SpatialStatus::NativeRejected;
                bail!("THX SetProperty({id}) returned false");
            }
            ThxReply::RpcRejected { code, message } => {
                outcome.status = SpatialStatus::RpcRejected;
                bail!("THX SetProperty({id}) RPC rejected: {code}, {message:?}");
            }
            _ => bail!("THX SetProperty({id}) returned the wrong response type"),
        }
    }
    fn firmware_eq(
        &mut self,
        task: &SpatialTask,
        enabled: bool,
        validate: &mut impl FnMut() -> Result<()>,
        outcome: &mut SpatialOutcome,
    ) -> Result<()> {
        let reply = self.guarded_exchange(
            task,
            ThxOperation::SetFirmwarePresetEqEnabled(enabled),
            validate,
            outcome,
        )?;
        match reply {
            ThxReply::FirmwareStatus(status) => {
                // This failure is log-only in the source before-hook. Do not
                // silently replace it with a new fail-stop source behavior.
                if status != "SUCCESS" {
                    outcome.warnings.push(format!(
                        "Source continued after firmware EQ status {status}"
                    ));
                }
                Ok(())
            }
            _ => bail!("Firmware EQ returned the wrong response type"),
        }
    }
    fn cloud_eq(
        &mut self,
        task: &SpatialTask,
        selected_index: Option<Value>,
        validate: &mut impl FnMut() -> Result<()>,
        outcome: &mut SpatialOutcome,
    ) -> Result<()> {
        let reply = self.guarded_exchange(
            task,
            ThxOperation::RestoreCloudEqStereo {
                presets: task.cloud_presets.clone(),
                selected_index,
            },
            validate,
            outcome,
        )?;
        match reply {
            ThxReply::CloudEqBool(success) => {
                // The source awaits this function but does not check its bool.
                if !success {
                    outcome
                        .warnings
                        .push("Source continued after Cloud EQ restore returned false".into());
                }
                Ok(())
            }
            _ => bail!("Cloud EQ restore returned the wrong response type"),
        }
    }
    fn run_spatial(
        &mut self,
        task: &SpatialTask,
        validate: &mut impl FnMut() -> Result<()>,
        outcome: &mut SpatialOutcome,
    ) -> Result<()> {
        if let Err(error) = self.require_transport() {
            outcome.status = SpatialStatus::Unavailable;
            return Err(error);
        }
        if !self.current(task) {
            outcome.status = SpatialStatus::Stale;
            bail!("THX task is stale");
        }
        ensure!(self.state.ready, "THX device binding is not ready");
        let enabled = task.requested_enabled;
        let changed = self.state.valkyrie_spatial_enabled != enabled;
        // Generic hookExecute invokes the Valkyrie before-hook before the
        // original spatial setter. Same-state requests still write FW/THX EQ.
        if enabled {
            self.firmware_eq(task, false, validate, outcome)?;
            self.native_set(task, "eq_enabled", true, validate, outcome)?;
            if changed {
                if let Some(active) = task
                    .cloud_presets
                    .iter()
                    .find(|preset| preset["currentActive"] == json!(true))
                {
                    self.cloud_eq(task, active.get("index").cloned(), validate, outcome)?;
                } else {
                    outcome.warnings.push(
                        "Source continued because no active Cloud EQ preset was found".into(),
                    );
                }
            }
        } else {
            self.native_set(task, "eq_enabled", false, validate, outcome)?;
            self.firmware_eq(task, true, validate, outcome)?;
            if changed {
                self.cloud_eq(task, None, validate, outcome)?;
            }
        }
        self.state.valkyrie_spatial_enabled = enabled;
        let acknowledgement =
            self.native_set(task, "spatial_enabled", enabled, validate, outcome)?;
        outcome.write_acknowledgement = Some(acknowledgement);
        outcome.native_acknowledged = acknowledgement == ThxWriteAcknowledgement::NativeBool;
        self.state.runtime_spatial_enabled = Some(enabled);
        // Additional Rust read-back validation; the original execute setter
        // updates its cache after true and performs no getter here.
        outcome.status = SpatialStatus::ReadbackFailed;
        let reply = self.guarded_exchange(
            task,
            ThxOperation::GetProperty("spatial_enabled".into()),
            validate,
            outcome,
        )?;
        let observed = match reply {
            ThxReply::Property(Some(value)) => value
                .as_bool()
                .context("THX spatial_enabled getter was not Bool")?,
            ThxReply::Property(None) => bail!("THX spatial_enabled getter returned no value"),
            _ => bail!("THX getter returned the wrong response type"),
        };
        self.state.observed_spatial_enabled = Some(observed);
        outcome.readback = Some(observed);
        if observed != enabled {
            outcome.status = SpatialStatus::ReadbackMismatch;
            bail!("THX spatial read-back differs from requested value");
        }
        self.state.acknowledged_spatial_enabled = Some(observed);
        outcome.status = SpatialStatus::Verified;
        Ok(())
    }
    pub fn execute_spatial(
        &mut self,
        task: &SpatialTask,
        mut validate: impl FnMut() -> Result<()>,
    ) -> SpatialOutcome {
        let mut outcome = SpatialOutcome::new(task, self.transport.kind());
        if let Err(error) = self.run_spatial(task, &mut validate, &mut outcome) {
            let message = format!("{error:#}");
            // A late/stale result must not overwrite the newer task's state.
            if self.current(task) {
                self.state.last_error = Some(message.clone());
            }
            outcome.error = Some(message);
        }
        outcome
    }

    /// An unsolicited getter refresh is an observation, not a setter/save ack.
    pub fn refresh_spatial(
        &mut self,
        scope: &ThxObservationScope,
        timer_tick: u64,
        mut validate: impl FnMut() -> Result<()>,
    ) -> SpatialOutcome {
        // A refresh captures identity/generation without changing local drafts.
        let task = SpatialTask {
            device: scope.device.clone(),
            generation: scope.generation,
            timer_tick,
            requested_enabled: false,
            cloud_presets: vec![],
        };
        let mut outcome = SpatialOutcome::new(&task, self.transport.kind());
        let result = (|| -> Result<()> {
            if let Err(error) = self.require_transport() {
                outcome.status = SpatialStatus::Unavailable;
                return Err(error);
            }
            outcome.status = SpatialStatus::ReadbackFailed;
            let current = |state: &ThxV4State| {
                state.device == scope.device && state.generation == scope.generation
            };
            if !current(&self.state) {
                outcome.status = SpatialStatus::Stale;
                bail!("THX refresh identity/generation is stale");
            }
            ensure!(self.state.ready, "THX device binding is not ready");
            if let Err(error) = validate() {
                outcome.status = SpatialStatus::Canceled;
                return Err(error);
            }
            let operation = ThxOperation::GetProperty("spatial_enabled".into());
            outcome.operations.push(operation.clone());
            let reply = self.transport.exchange(&scope.device, &operation)?;
            if !current(&self.state) {
                outcome.status = SpatialStatus::Stale;
                bail!("THX refresh became stale during transport");
            }
            if let Err(error) = validate() {
                outcome.status = SpatialStatus::Canceled;
                return Err(error);
            }
            match reply {
                ThxReply::Property(Some(value)) => {
                    let observed = value
                        .as_bool()
                        .context("THX spatial_enabled getter was not Bool")?;
                    self.state.observed_spatial_enabled = Some(observed);
                    // The service's initial getter seeds its changed-state hook
                    // as explicit Rust policy, not a source runtimeCache write.
                    self.state.valkyrie_spatial_enabled = observed;
                    outcome.readback = Some(observed);
                    outcome.status = SpatialStatus::Observed;
                    Ok(())
                }
                _ => bail!("THX getter returned no Bool observation"),
            }
        })();
        if let Err(error) = result {
            outcome.error = Some(format!("{error:#}"));
        }
        outcome
    }
    pub fn remove(&mut self) -> Result<()> {
        self.invalidate();
        self.require_transport()?;
        match self
            .transport
            .exchange(&self.state.device, &ThxOperation::RemoveDevice)?
        {
            ThxReply::NativeBool(true) | ThxReply::LocalBinding(true) => Ok(()),
            _ => bail!("THX RemoveDevice did not return true"),
        }
    }
}

/// Explicitly scripted simulator: an unconfigured operation is an error. Its
/// replies are synthetic test fixtures and cannot be used as real device ack.
pub struct SimulatedThxV4Transport {
    script: VecDeque<(ThxOperation, std::result::Result<ThxReply, String>)>,
    pub exchanges: Vec<(ThxDeviceIdentity, ThxOperation)>,
}
impl SimulatedThxV4Transport {
    pub fn new(script: impl IntoIterator<Item = (ThxOperation, ThxReply)>) -> Self {
        Self {
            script: script
                .into_iter()
                .map(|(op, reply)| (op, Ok(reply)))
                .collect(),
            exchanges: vec![],
        }
    }
    pub fn failure(&mut self, operation: ThxOperation, message: impl Into<String>) {
        self.script.push_back((operation, Err(message.into())));
    }
    pub fn remaining(&self) -> usize {
        self.script.len()
    }
}
impl ThxV4Transport for SimulatedThxV4Transport {
    fn kind(&self) -> ThxTransportKind {
        ThxTransportKind::Simulated
    }
    fn exchange(
        &mut self,
        identity: &ThxDeviceIdentity,
        operation: &ThxOperation,
    ) -> Result<ThxReply> {
        self.exchanges.push((identity.clone(), operation.clone()));
        let (expected, reply) = self
            .script
            .pop_front()
            .context("Unscripted simulated THX operation")?;
        ensure!(
            &expected == operation,
            "Simulated THX request differs: expected {expected:?}; got {operation:?}"
        );
        reply.map_err(|message| anyhow::anyhow!(message))
    }
}

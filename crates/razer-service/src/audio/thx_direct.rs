//! THX local RPC + retained BlackShark Output/Interrupt HID collection.
//! Binding/readiness are Rust state; successful RPC is never a vendor DLL ACK.
use crate::thx_rpc::Cancellation;
use anyhow::{Result, bail, ensure};
use razer_device::{
    thx_blackshark_firmware::{
        BlackSharkFirmwareFailureKind, BlackSharkFirmwareIdentity, BlackSharkFirmwareSession,
    },
    thx_submission::ThxFirmwareProgress,
    thx_v4::{
        ThxDeviceIdentity, ThxExchangeInterrupted, ThxOperation, ThxReply, ThxTransportKind,
        ThxV4Transport, capability,
    },
    thx_v4_proto::{CodecError, Request, Response},
    thx_v4_value::{property_from_request, value_from_property},
};
use serde_json::Value;
use std::time::Duration;

pub const SOCKET_URI_PREFIX: &str = "unix:C:/ProgramData/thx/";
pub const ENDPOINT_ID: &str = "headphones_game";
pub fn socket_uri(identity: &ThxDeviceIdentity) -> String {
    format!(
        "{SOCKET_URI_PREFIX}{}/thx_properties.ipc",
        identity.product_id
    )
}

/// Production exclusively uses the local AF_UNIX stream; tests inject typed
/// scripted responses or memory HTTP/2 streams.
pub trait ThxLocalRpc {
    fn kind(&self) -> ThxTransportKind;
    fn request(&mut self, identity: &ThxDeviceIdentity, request: Request) -> Result<Response>;
}
pub struct LocalRpc {
    pub deadline: Duration,
    pub cancellation: Cancellation,
    pub scope: razer_device::thx_submission::ThxSubmissionScope,
}
impl ThxLocalRpc for LocalRpc {
    fn kind(&self) -> ThxTransportKind {
        ThxTransportKind::Direct
    }
    fn request(&mut self, identity: &ThxDeviceIdentity, request: Request) -> Result<Response> {
        self.cancellation.check()?;
        // RPC addresses only product/endpoint. Raw roller HID jobs have an
        // exact container and must not inherit this separate RPC restriction.
        crate::thx::resolve(&self.scope)?;
        #[cfg(windows)]
        {
            let result = crate::thx_rpc::unary_local(
                &socket_uri(identity),
                &request,
                self.deadline,
                &self.cancellation,
            );
            if let Err(error) = crate::thx::resolve(&self.scope) {
                if matches!(&result, Ok(Response::SetProperty)) {
                    return Err(ThxExchangeInterrupted {
                        reply: ThxReply::RpcAcknowledged,
                        error: format!("{error:#}"),
                    }
                    .into());
                }
                return Err(error);
            }
            result
        }
        #[cfg(not(windows))]
        {
            let _ = (identity, request);
            bail!("THX local AF_UNIX adapter is unavailable on this platform")
        }
    }
}
type Validate = Box<dyn FnMut(&BlackSharkFirmwareIdentity) -> Result<()>>;
type Wait = Box<dyn FnMut(u64) -> Result<()>>;
pub struct DirectThxTransport<R> {
    rpc: R,
    firmware: BlackSharkFirmwareSession,
    validate: Validate,
    wait: Wait,
    route: ThxDeviceIdentity,
    binding: Option<ThxDeviceIdentity>,
    progress: Vec<ThxFirmwareProgress>,
}
impl<R: ThxLocalRpc> DirectThxTransport<R> {
    pub fn new(
        rpc: R,
        firmware: BlackSharkFirmwareSession,
        validate: Validate,
        wait: Wait,
    ) -> Result<Self> {
        let cap = capability(firmware.identity().owner_product_id)
            .ok_or_else(|| anyhow::anyhow!("THX owner lacks a current source audit"))?;
        let route = ThxDeviceIdentity::new(cap.thx_product_id, cap.endpoint_id)?;
        Ok(Self {
            rpc,
            firmware,
            validate,
            wait,
            route,
            binding: None,
            progress: vec![],
        })
    }
    pub fn firmware_progress(&self) -> &[ThxFirmwareProgress] {
        &self.progress
    }
    pub fn exchange_hid(
        &mut self,
        command: razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommand,
    ) -> Result<razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommandProgress> {
        use razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommand;
        ensure!(
            matches!(
                command,
                BlackSharkFirmwareCommand::GetFirmwareVersions
                    | BlackSharkFirmwareCommand::GetRollerFunction
                    | BlackSharkFirmwareCommand::SetRollerFunction(_)
            ),
            "Expected a firmware-version or roller command"
        );
        // No AddDevice/CheckStatus/THX binding: the current feature owns a
        // direct general-protocol operation on this retained HID collection.
        let progress = if command == BlackSharkFirmwareCommand::GetFirmwareVersions
            && self.firmware.identity().connection()?
                == razer_device::thx_blackshark_firmware::BlackSharkFirmwareConnection::Dongle
        {
            // AudioValkyrieDongle -> AudioBillieT1Dongle uses the explicit
            // sendCommandToDongle path for getExtendedFirmwareVersion.
            self.firmware
                .exchange_to_dongle(command, &mut self.validate, &mut self.wait)
        } else {
            self.firmware
                .exchange(command, &mut self.validate, &mut self.wait)
        };
        self.progress
            .push(ThxFirmwareProgress::Command(progress.clone()));
        Ok(progress)
    }
    pub fn bound(&self) -> bool {
        self.binding.is_some()
    }
    pub fn rpc(&self) -> &R {
        &self.rpc
    }
    fn check(&mut self, identity: &ThxDeviceIdentity) -> Result<()> {
        ensure!(
            *identity == self.route,
            "THX product/endpoint differs from the audited retained HID owner"
        );
        (self.validate)(self.firmware.identity())
    }
    fn request(&mut self, identity: &ThxDeviceIdentity, request: Request) -> Result<Response> {
        self.check(identity)?;
        ensure!(
            self.binding.as_ref() == Some(identity),
            "THX local binding is missing"
        );
        let reply = self.rpc.request(identity, request);
        if let Err(error) = self.check(identity) {
            let received = match &reply {
                Ok(Response::SetProperty) => Some(ThxReply::RpcAcknowledged),
                Err(error) => match error.downcast_ref::<CodecError>() {
                    Some(CodecError::GrpcStatus { code, message }) => Some(ThxReply::RpcRejected {
                        code: *code,
                        message: message.clone(),
                    }),
                    _ => None,
                },
                _ => None,
            };
            if let Some(reply) = received {
                return Err(ThxExchangeInterrupted {
                    reply,
                    error: format!("{error:#}"),
                }
                .into());
            }
            return Err(error);
        }
        reply
    }
    fn value(id: &str, property: &razer_device::thx_v4_proto::Property) -> Result<Value> {
        ensure!(
            property.id == id,
            "THX property id differs from the request"
        );
        if matches!(id, "spatial_enabled" | "eq_enabled") {
            // Native compatibility defaults cannot prove an observed Bool.
            Ok(Value::Bool(property.observed_bool(id)?))
        } else {
            value_from_property(property).map_err(Into::into)
        }
    }
}
fn exceptional(
    failure: Option<&razer_device::thx_blackshark_firmware::BlackSharkFirmwareFailure>,
) -> Result<()> {
    if let Some(failure) = failure {
        if matches!(
            failure.kind,
            BlackSharkFirmwareFailureKind::Validation
                | BlackSharkFirmwareFailureKind::Scheduling
                | BlackSharkFirmwareFailureKind::Transport
        ) {
            bail!("BlackShark {:?}: {}", failure.kind, failure.message);
        }
    }
    Ok(())
}
impl<R: ThxLocalRpc> ThxV4Transport for DirectThxTransport<R> {
    fn kind(&self) -> ThxTransportKind {
        self.rpc.kind()
    }
    fn exchange(
        &mut self,
        identity: &ThxDeviceIdentity,
        operation: &ThxOperation,
    ) -> Result<ThxReply> {
        if *operation == ThxOperation::RemoveDevice {
            ensure!(*identity == self.route, "THX removal identity differs");
            // No socket task survives unary(). Clear our actual owned binding,
            // even after cancel/disconnect. This is never a native ACK.
            return Ok(ThxReply::LocalBinding(
                self.binding.take().as_ref() == Some(identity),
            ));
        }
        self.check(identity)?;
        match operation {
            ThxOperation::AddDevice => {
                // Native AddDevice RVA 0x17B770 returns true for an existing
                // same-key map entry. It does not recreate/verify the channel.
                if self.binding.as_ref() == Some(identity) {
                    return Ok(ThxReply::LocalBinding(true));
                }
                ensure!(self.binding.is_none(), "THX local binding differs");
                self.binding = Some(identity.clone());
                Ok(ThxReply::LocalBinding(true))
            }
            ThxOperation::CheckStatus => match self.request(identity, Request::GetVersion)? {
                // Native readiness is GetState(true)==READY (2), with a
                // diagnostic GetVersion. Empty version text is valid; Rust
                // requires completed HTTP/2/RPC instead of testing text.
                Response::GetVersion { .. } => Ok(ThxReply::LocalReady(true)),
                _ => bail!("THX GetVersion response type differs"),
            },
            ThxOperation::SetProperty(request) => match self.request(
                identity,
                Request::SetProperty {
                    property: Some(property_from_request(request)?),
                    endpoint_id: Some(identity.endpoint_id.clone()),
                },
            ) {
                Ok(Response::SetProperty) => Ok(ThxReply::RpcAcknowledged),
                Ok(_) => bail!("THX SetProperty response type differs"),
                Err(error) => {
                    // Native RVA 0x15B730 tests the final grpc::Status code:
                    // nonzero becomes bool false, which initThxSettings logs
                    // and continues. Keep the actual RPC status distinct. A
                    // missing/partial reply, timeout, cancel, validation or
                    // codec error remains exceptional with unknown side effects.
                    if let Some(CodecError::GrpcStatus { code, message }) =
                        error.downcast_ref::<CodecError>()
                    {
                        Ok(ThxReply::RpcRejected {
                            code: *code,
                            message: message.clone(),
                        })
                    } else {
                        Err(error)
                    }
                }
            },
            ThxOperation::GetProperty(id) => match self.request(
                identity,
                Request::GetProperty {
                    id: id.clone(),
                    endpoint_id: Some(identity.endpoint_id.clone()),
                },
            )? {
                Response::GetProperty(property) => {
                    Ok(ThxReply::Property(Some(Self::value(id, &property)?)))
                }
                _ => bail!("THX GetProperty response type differs"),
            },
            ThxOperation::SetFirmwarePresetEqEnabled(enabled) => {
                ensure!(
                    self.binding.as_ref() == Some(identity),
                    "THX local binding is missing"
                );
                let result = self.firmware.set_preset_eq_enabled(
                    *enabled,
                    &mut self.validate,
                    &mut self.wait,
                );
                let status = result.source_status().to_owned();
                let failure = result.failure.clone();
                self.progress.push(ThxFirmwareProgress::Command(result));
                exceptional(failure.as_ref())?;
                self.check(identity)?;
                Ok(ThxReply::FirmwareStatus(status))
            }
            ThxOperation::RestoreCloudEqStereo {
                presets,
                selected_index,
            } => {
                ensure!(
                    self.binding.as_ref() == Some(identity),
                    "THX local binding is missing"
                );
                let result = self.firmware.restore_local_eq(
                    presets,
                    selected_index.as_ref(),
                    &mut self.validate,
                    &mut self.wait,
                );
                let succeeded = result.succeeded();
                let failure = result.failure.clone();
                self.progress.push(ThxFirmwareProgress::LocalEq(result));
                exceptional(failure.as_ref())?;
                self.check(identity)?;
                Ok(ThxReply::CloudEqBool(succeeded))
            }
            ThxOperation::RemoveDevice => unreachable!(),
        }
    }
}

#[cfg(test)]
#[path = "thx_direct_tests.rs"]
mod tests;

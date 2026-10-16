//! Current Razer camera controls exposed by `RzNative_GenericCamera`.
//!
//! The protocol layer is deliberately independent of Windows COM.  The native
//! DLL uses DirectShow `IAMVideoProcAmp` and `IAMCameraControl` for the two
//! standard groups, then dispatches the remaining controls through a private
//! camera object.  This module keeps those two transports separate so a
//! Windows adapter can implement the standard properties without pretending
//! that the private extension ABI is portable.
//!
//! Property IDs, HRESULT handling and export RVAs are reviewed against the
//! indexed original inputs and complete native capture. Evidence digests are
//! analysis metadata, separate from runtime device capability checks.

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const KIYO_V2_PRO_PRODUCT_ID: u32 = 3594;
pub const KIYO_V2_PRODUCT_ID: u32 = 3595;
pub const KIYO_V2_X_PRODUCT_ID: u32 = 3596;

/// A caller-selected DirectShow symbolic link, never a camera name or substring.
/// Each product has its own current manifest and native-property audit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraTarget {
    pub product_id: u32,
    pub device_path: String,
}

impl CameraTarget {
    pub fn new(product_id: u32, device_path: String) -> Result<Self> {
        let target = Self {
            product_id,
            device_path,
        };
        target.validate()?;
        Ok(target)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(
                self.product_id,
                KIYO_V2_PRO_PRODUCT_ID | KIYO_V2_PRODUCT_ID | KIYO_V2_X_PRODUCT_ID
            ),
            "camera product has no current native property audit"
        );
        ensure!(
            !self.device_path.contains('\0') && self.device_path.len() <= 32_768,
            "camera DevicePath is invalid"
        );
        let path = self.device_path.to_ascii_lowercase();
        let parts: Vec<_> = path.split('#').collect();
        ensure!(
            parts.len() >= 4
                && parts[0] == r"\\?\usb"
                && !parts[2].is_empty()
                && !parts[3].is_empty(),
            "camera target must be a complete USB DirectShow DevicePath"
        );
        let hardware: Vec<_> = parts[1].split('&').collect();
        ensure!(
            hardware
                .iter()
                .filter(|s| s.starts_with("vid_"))
                .copied()
                .eq(["vid_1532"])
                && hardware
                    .iter()
                    .filter(|s| s.starts_with("pid_"))
                    .copied()
                    .eq([format!("pid_{:04x}", self.product_id).as_str()]),
            "camera DevicePath does not identify the audited Razer product"
        );
        Ok(())
    }
}

/// DirectShow Video ProcAmp properties used by the current native wrapper.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum VideoProcAmpProperty {
    Brightness,
    Contrast,
    Hue,
    Saturation,
    Sharpness,
    Gamma,
    ColorEnable,
    WhiteBalance,
    BacklightCompensation,
    Gain,
    AntiFlicker,
}

impl VideoProcAmpProperty {
    /// Property values passed to `IAMVideoProcAmp::Get/Set` by the DLL.
    pub const fn id(self) -> u32 {
        match self {
            Self::Brightness => 0,
            Self::Contrast => 1,
            Self::Hue => 2,
            Self::Saturation => 3,
            Self::Sharpness => 4,
            Self::Gamma => 5,
            Self::ColorEnable => 6,
            Self::WhiteBalance => 7,
            Self::BacklightCompensation => 8,
            Self::Gain => 9,
            // The current source intentionally uses 0x0d rather than the
            // usual public enum value for PowerLineFrequency.
            Self::AntiFlicker => 0x0d,
        }
    }

    pub const fn export_names(self) -> (&'static str, &'static str) {
        match self {
            Self::Brightness => ("GetBrightness", "SetBrightness"),
            Self::Contrast => ("GetContrast", "SetContrast"),
            Self::Hue => ("GetHue", "SetHue"),
            Self::Saturation => ("GetSaturation", "SetSaturation"),
            Self::Sharpness => ("GetSharpness", "SetSharpness"),
            Self::Gamma => ("GetGamma", "SetGamma"),
            Self::ColorEnable => ("GetColorEnable", "SetColorEnable"),
            Self::WhiteBalance => ("GetWhiteBalance", "SetWhiteBalance"),
            Self::BacklightCompensation => ("GetBacklightCompensation", "SetBacklightCompensation"),
            Self::Gain => ("GetGain", "SetGain"),
            Self::AntiFlicker => ("GetAntiFlicker", "SetAntiFlicker"),
        }
    }
}

/// DirectShow camera control properties used by the current native wrapper.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum CameraControlProperty {
    Pan,
    Tilt,
    Roll,
    Zoom,
    Exposure,
    Iris,
    Focus,
    Flash,
    LowLightCompensation,
}

impl CameraControlProperty {
    pub const fn id(self) -> u32 {
        match self {
            Self::Pan => 0,
            Self::Tilt => 1,
            Self::Roll => 2,
            Self::Zoom => 3,
            // These signed LONG values are intentionally represented as i32.
            Self::Exposure => 4,
            Self::Iris => 5,
            Self::Focus => 6,
            Self::Flash => 7,
            Self::LowLightCompensation => 0x13,
        }
    }

    pub const fn export_names(self) -> (&'static str, &'static str) {
        match self {
            Self::Pan => ("GetCameraPan", "SetCameraPan"),
            Self::Tilt => ("GetCameraTilt", "SetCameraTilt"),
            Self::Roll => ("GetCameraRoll", "SetCameraRoll"),
            Self::Zoom => ("GetCameraZoom", "SetCameraZoom"),
            Self::Exposure => ("GetCameraExposure", "SetCameraExposure"),
            Self::Iris => ("GetCameraIris", "SetCameraIris"),
            Self::Focus => ("GetCameraFocus", "SetCameraFocus"),
            Self::Flash => ("GetCameraFlash", "SetCameraFlash"),
            Self::LowLightCompensation => (
                "GetCameraLowLightCompensation",
                "SetCameraLowLightCompensation",
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum StandardCameraProperty {
    VideoProcAmp(VideoProcAmpProperty),
    CameraControl(CameraControlProperty),
}

impl StandardCameraProperty {
    pub const ALL: [Self; 20] = [
        Self::VideoProcAmp(VideoProcAmpProperty::Brightness),
        Self::VideoProcAmp(VideoProcAmpProperty::Contrast),
        Self::VideoProcAmp(VideoProcAmpProperty::Hue),
        Self::VideoProcAmp(VideoProcAmpProperty::Saturation),
        Self::VideoProcAmp(VideoProcAmpProperty::Sharpness),
        Self::VideoProcAmp(VideoProcAmpProperty::Gamma),
        Self::VideoProcAmp(VideoProcAmpProperty::ColorEnable),
        Self::VideoProcAmp(VideoProcAmpProperty::WhiteBalance),
        Self::VideoProcAmp(VideoProcAmpProperty::BacklightCompensation),
        Self::VideoProcAmp(VideoProcAmpProperty::Gain),
        Self::VideoProcAmp(VideoProcAmpProperty::AntiFlicker),
        Self::CameraControl(CameraControlProperty::Pan),
        Self::CameraControl(CameraControlProperty::Tilt),
        Self::CameraControl(CameraControlProperty::Roll),
        Self::CameraControl(CameraControlProperty::Zoom),
        Self::CameraControl(CameraControlProperty::Exposure),
        Self::CameraControl(CameraControlProperty::Iris),
        Self::CameraControl(CameraControlProperty::Focus),
        Self::CameraControl(CameraControlProperty::Flash),
        Self::CameraControl(CameraControlProperty::LowLightCompensation),
    ];

    pub const fn id(self) -> u32 {
        match self {
            Self::VideoProcAmp(property) => property.id(),
            Self::CameraControl(property) => property.id(),
        }
    }

    pub const fn export_names(self) -> (&'static str, &'static str) {
        match self {
            Self::VideoProcAmp(property) => property.export_names(),
            Self::CameraControl(property) => property.export_names(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct CameraLongObservation {
    pub value: i32,
    /// Preserve flags such as 3 rather than silently rewriting them as manual.
    /// The original JSON nevertheless reports isAuto only for flags == 1.
    pub native_flags: i32,
    pub native_hresult: i32,
}

impl CameraLongObservation {
    pub fn from_native(value: i32, native_flags: i32, native_hresult: i32) -> Result<Self> {
        ensure!(
            native_hresult >= 0,
            "camera Get failed: 0x{:08x}",
            native_hresult as u32
        );
        Ok(Self {
            value,
            native_flags,
            native_hresult,
        })
    }

    pub const fn is_auto(self) -> bool {
        self.native_flags == 1
    }

    pub fn source_response(self) -> serde_json::Value {
        serde_json::json!({"value": self.value, "isAuto": self.is_auto()})
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct CameraSetting {
    pub value: i32,
    pub is_auto: bool,
}

impl CameraSetting {
    pub const fn native_flags(self) -> u32 {
        if self.is_auto { 1 } else { 2 }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct CameraWriteAck {
    pub accepted: bool,
    pub native_hresult: i32,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CameraSetPhase {
    QueryInterface,
    #[default]
    Set,
}
/// A real native setter path returned false, either before Set (failed QI) or
/// from Set itself. Source compound runners can continue without an ACK.
#[derive(Debug)]
pub struct CameraSetRejection {
    pub native_hresult: i32,
    pub phase: CameraSetPhase,
}
impl std::fmt::Display for CameraSetRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "camera {:?} failed: 0x{:08x}",
            self.phase, self.native_hresult as u32
        )
    }
}
impl std::error::Error for CameraSetRejection {}

impl CameraWriteAck {
    pub fn from_hresult(native_hresult: i32) -> Result<Self> {
        if native_hresult < 0 {
            return Err(CameraSetRejection {
                native_hresult,
                phase: CameraSetPhase::Set,
            }
            .into());
        }
        Ok(Self {
            accepted: true,
            native_hresult,
        })
    }
}

/// A Windows adapter implements these methods with the exact COM calls seen
/// in the current PE.  No HRESULT failure may be converted into an invented
/// zero observation.
pub trait DirectShowCameraTransport {
    fn get_video_proc_amp(&mut self, property_id: u32) -> Result<CameraLongObservation>;
    fn set_video_proc_amp(
        &mut self,
        property_id: u32,
        value: i32,
        is_auto: bool,
    ) -> Result<CameraWriteAck>;
    fn get_camera_control(&mut self, property_id: u32) -> Result<CameraLongObservation>;
    fn set_camera_control(
        &mut self,
        property_id: u32,
        value: i32,
        is_auto: bool,
    ) -> Result<CameraWriteAck>;
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StandardCameraRead {
    pub property: StandardCameraProperty,
    pub observation: CameraLongObservation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StandardCameraWrite {
    pub property: StandardCameraProperty,
    pub requested: CameraSetting,
    pub acknowledgement: CameraWriteAck,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StandardCameraSetting {
    pub property: StandardCameraProperty,
    pub setting: CameraSetting,
}

/// There is no atomic multi-property transaction or automatic rollback in the
/// observed native API. Keep every ACK when a later property/cancellation fails.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraWriteProgress {
    pub acknowledged: Vec<StandardCameraWrite>,
    /// An errored transport call may already have changed the device.
    pub unconfirmed_request: Option<StandardCameraSetting>,
    pub completed: bool,
    pub error: Option<String>,
}

/// Typed standard-property session.  It does not cache local drafts and does
/// not claim that a write was persisted beyond the adapter's acknowledgement.
pub struct CameraSession<T> {
    transport: T,
    target: CameraTarget,
}

impl<T> CameraSession<T> {
    pub fn open(transport: T, target: CameraTarget) -> Result<Self> {
        target.validate()?;
        Ok(Self { transport, target })
    }

    pub fn target(&self) -> &CameraTarget {
        &self.target
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

impl<T: DirectShowCameraTransport> CameraSession<T> {
    pub fn read(
        &mut self,
        property: StandardCameraProperty,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<StandardCameraRead> {
        self.target.validate()?;
        validate()?;
        let observation = match property {
            StandardCameraProperty::VideoProcAmp(_) => {
                self.transport.get_video_proc_amp(property.id())?
            }
            StandardCameraProperty::CameraControl(_) => {
                self.transport.get_camera_control(property.id())?
            }
        };
        ensure!(
            observation.native_hresult >= 0,
            "camera transport returned a failed Get as an observation"
        );
        validate()?;
        Ok(StandardCameraRead {
            property,
            observation,
        })
    }

    pub fn write(
        &mut self,
        property: StandardCameraProperty,
        value: i32,
        is_auto: bool,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<StandardCameraWrite> {
        self.target.validate()?;
        validate()?;
        self.submit(property, CameraSetting { value, is_auto })
    }

    fn submit(
        &mut self,
        property: StandardCameraProperty,
        setting: CameraSetting,
    ) -> Result<StandardCameraWrite> {
        let CameraSetting { value, is_auto } = setting;
        let acknowledgement = match property {
            StandardCameraProperty::VideoProcAmp(_) => {
                self.transport
                    .set_video_proc_amp(property.id(), value, is_auto)?
            }
            StandardCameraProperty::CameraControl(_) => {
                self.transport
                    .set_camera_control(property.id(), value, is_auto)?
            }
        };
        ensure!(
            acknowledgement.accepted && acknowledgement.native_hresult >= 0,
            "camera standard property setter was rejected"
        );
        Ok(StandardCameraWrite {
            property,
            requested: setting,
            acknowledgement,
        })
    }

    pub fn write_batch(
        &mut self,
        settings: &[StandardCameraSetting],
        validate: &impl Fn() -> Result<()>,
    ) -> CameraWriteProgress {
        let mut progress = CameraWriteProgress::default();
        let outcome = (|| -> Result<()> {
            self.target.validate()?;
            ensure!(
                !settings.is_empty() && settings.len() <= StandardCameraProperty::ALL.len(),
                "camera property batch is empty or too large"
            );
            let mut seen = std::collections::HashSet::new();
            ensure!(
                settings.iter().all(|s| seen.insert(s.property)),
                "camera batch repeats a property"
            );
            for requested in settings {
                validate()?;
                progress.unconfirmed_request = Some(*requested);
                let write = self.submit(requested.property, requested.setting)?;
                progress.acknowledged.push(write);
                progress.unconfirmed_request = None;
                validate()?;
            }
            Ok(())
        })();
        match outcome {
            Ok(()) => progress.completed = true,
            Err(error) => progress.error = Some(format!("{error:#}")),
        }
        progress
    }
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;

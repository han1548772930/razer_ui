//! Direct camera service entry points. No vendor DLL is loaded.
//! Only the COM/SetupDi adapter is Windows-specific.

#[path = "camera/commands.rs"]
pub mod commands;
#[path = "camera/lifecycle.rs"]
pub mod lifecycle;
#[path = "camera/serial.rs"]
mod serial;
pub use serial::{CameraInitializationFailure, CameraSerialSession};

use anyhow::Result;
#[cfg(windows)]
use razer_device::camera::CameraSession;
use razer_device::camera::{
    CameraTarget, CameraWriteProgress, StandardCameraProperty, StandardCameraRead,
    StandardCameraSetting, StandardCameraWrite,
};

#[cfg(windows)]
#[path = "runtime/windows/camera.rs"]
mod windows;

#[cfg(windows)]
#[path = "runtime/windows/camera_extensions.rs"]
mod extensions;
#[cfg(windows)]
#[path = "runtime/windows/camera_focus_family.rs"]
mod focus_family;
#[cfg(windows)]
#[path = "runtime/windows/camera_identity.rs"]
mod identity;
#[cfg(windows)]
#[path = "runtime/windows/camera_lock.rs"]
pub(crate) mod lock;

use razer_device::camera_submission::{
    CameraCommand, CameraCommandOutcome, CameraCommandTransport, CameraScope,
};

pub fn discover(scope: &CameraScope) -> Result<CameraTarget> {
    scope.validate()?;
    #[cfg(windows)]
    {
        windows::discover(scope)
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("camera discovery requires the audited Windows DirectShow adapter")
    }
}
pub fn validate_live_scope(target: &CameraTarget, scope: &CameraScope) -> Result<()> {
    target.validate()?;
    scope.validate()?;
    anyhow::ensure!(
        target.product_id == scope.product_id,
        "camera target/scope mismatch"
    );
    #[cfg(windows)]
    {
        anyhow::ensure!(
            identity::container_id(&target.device_path)?.eq_ignore_ascii_case(&scope.container_id),
            "camera ContainerId changed"
        );
        Ok(())
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("camera identity adapter is unavailable on this platform")
    }
}

pub(super) struct ScopedCameraSession {
    inner: CameraDeviceSession,
    target: CameraTarget,
    scope: CameraScope,
}

impl ScopedCameraSession {
    pub(super) fn open_mode(scope: CameraScope, properties: bool) -> Result<Self> {
        let target = discover(&scope)?;
        let inner = CameraDeviceSession::open_mode(target.clone(), properties)?;
        validate_live_scope(&target, &scope)?;
        Ok(Self {
            inner,
            target,
            scope,
        })
    }
}
impl CameraCommandTransport for ScopedCameraSession {
    fn execute(
        &mut self,
        command: &CameraCommand,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<CameraCommandOutcome> {
        let target = &self.target;
        let scope = &self.scope;
        let check = || {
            validate()?;
            validate_live_scope(target, scope)
        };
        check()?;
        Ok(match command {
            CameraCommand::ShowProperties => {
                self.inner.show_properties(&check)?;
                CameraCommandOutcome::PropertiesClosed
            }
            CameraCommand::StandardRead(property) => {
                #[cfg(windows)]
                let _native = lock::acquire(&lock::native_name(scope), &check)?;
                CameraCommandOutcome::Read(self.inner.read(*property, &check)?)
            }
            CameraCommand::StandardWrite(setting) => {
                #[cfg(windows)]
                let _native = lock::acquire(&lock::native_name(scope), &check)?;
                match self.inner.write(
                    setting.property,
                    setting.setting.value,
                    setting.setting.is_auto,
                    &check,
                ) {
                    Ok(write) => CameraCommandOutcome::Write(write),
                    Err(error) => {
                        match error.downcast_ref::<razer_device::camera::CameraSetRejection>() {
                            Some(rejected) => CameraCommandOutcome::RejectedWrite {
                                requested: *setting,
                                native_hresult: rejected.native_hresult,
                                phase: rejected.phase,
                            },
                            None => return Err(error),
                        }
                    }
                }
            }
            CameraCommand::Extension(request) => CameraCommandOutcome::Extension(
                self.inner.extension_scoped(*request, scope, &check),
            ),
            CameraCommand::FamilyFocus(request) => CameraCommandOutcome::FamilyFocus(
                self.inner.family_focus_scoped(*request, scope, &check),
            ),
            CameraCommand::FamilyExposure(request) => CameraCommandOutcome::FamilyExposure(
                self.inner.family_exposure_scoped(*request, scope, &check),
            ),
            CameraCommand::FamilyProcessing(request) => CameraCommandOutcome::FamilyProcessing(
                self.inner.family_processing_scoped(*request, scope, &check),
            ),
        })
    }
}

/// All calls and apartment cleanup stay on the opening thread.
#[cfg(windows)]
pub struct CameraDeviceSession {
    session: CameraSession<windows::CameraComSession>,
    serial: razer_device::camera_serial::CameraSerialState,
}

#[cfg(not(windows))]
pub struct CameraDeviceSession {
    product_id: u32,
}

impl CameraDeviceSession {
    pub fn open(target: CameraTarget) -> Result<Self> {
        Self::open_mode(target, false)
    }
    fn open_mode(target: CameraTarget, properties: bool) -> Result<Self> {
        target.validate()?;
        #[cfg(windows)]
        {
            let transport = windows::CameraComSession::open_mode(&target, properties)?;
            let serial = razer_device::camera_serial::CameraSerialState::new(target.product_id)?;
            Ok(Self {
                session: CameraSession::open(transport, target)?,
                serial,
            })
        }
        #[cfg(not(windows))]
        anyhow::bail!(
            "current camera adapter requires Windows DirectShow; this platform has no audited UVC adapter"
        )
    }

    fn serial_scoped(
        &mut self,
        scope: &CameraScope,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<razer_device::camera_serial::CameraSerialProgress> {
        #[cfg(windows)]
        {
            if self.session.target().product_id == razer_device::camera::KIYO_V2_PRO_PRODUCT_ID {
                let mut transport =
                    extensions::CameraKsSession::new(self.session.transport_mut(), scope, validate);
                Ok(self.serial.read_pro(&mut transport, validate))
            } else {
                let mut transport = focus_family::CameraFamilyKsSession::new(
                    self.session.transport_mut(),
                    scope,
                    validate,
                );
                Ok(self.serial.read_family(&mut transport, validate))
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (scope, validate);
            anyhow::bail!("camera serial observation requires Windows DirectShow")
        }
    }

    pub fn show_properties(&mut self, validate: &impl Fn() -> Result<()>) -> Result<()> {
        #[cfg(windows)]
        {
            self.session.transport_mut().show_properties(validate)
        }
        #[cfg(not(windows))]
        {
            let _ = validate;
            anyhow::bail!("Windows camera property pages require DirectShow");
        }
    }

    pub fn read(
        &mut self,
        property: StandardCameraProperty,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<StandardCameraRead> {
        #[cfg(windows)]
        {
            self.session.read(property, validate)
        }
        #[cfg(not(windows))]
        {
            let _ = (property, validate);
            anyhow::bail!("camera adapter is unavailable on this platform")
        }
    }

    pub fn write(
        &mut self,
        property: StandardCameraProperty,
        value: i32,
        is_auto: bool,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<StandardCameraWrite> {
        #[cfg(windows)]
        {
            self.session.write(property, value, is_auto, validate)
        }
        #[cfg(not(windows))]
        {
            let _ = (property, value, is_auto, validate);
            anyhow::bail!("camera adapter is unavailable on this platform")
        }
    }

    pub fn write_batch(
        &mut self,
        settings: &[StandardCameraSetting],
        validate: &impl Fn() -> Result<()>,
    ) -> CameraWriteProgress {
        #[cfg(windows)]
        {
            self.session.write_batch(settings, validate)
        }
        #[cfg(not(windows))]
        {
            let _ = (settings, validate);
            CameraWriteProgress {
                error: Some("camera adapter is unavailable on this platform".into()),
                ..Default::default()
            }
        }
    }

    fn extension_scoped(
        &mut self,
        request: razer_device::camera_extensions::CameraExtensionRequest,
        scope: &CameraScope,
        validate: &impl Fn() -> Result<()>,
    ) -> razer_device::camera_extensions::CameraExtensionProgress {
        #[cfg(windows)]
        {
            let product_id = self.session.target().product_id;
            let mut transport =
                extensions::CameraKsSession::new(self.session.transport_mut(), scope, validate);
            razer_device::camera_extensions::execute_camera_extension(
                &mut transport,
                product_id,
                request,
                validate,
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (validate, scope);
            razer_device::camera_extensions::CameraExtensionProgress::failed(
                request,
                razer_device::camera_extensions::CameraExtensionPhase::Target,
                "camera private KS adapter is unavailable on this platform".into(),
            )
        }
    }

    fn family_focus_scoped(
        &mut self,
        request: razer_device::camera_focus_family::CameraFamilyFocusRequest,
        scope: &CameraScope,
        validate: &impl Fn() -> Result<()>,
    ) -> razer_device::camera_focus_family::CameraFamilyFocusProgress {
        #[cfg(windows)]
        {
            let product_id = self.session.target().product_id;
            let mut transport = focus_family::CameraFamilyKsSession::new(
                self.session.transport_mut(),
                scope,
                validate,
            );
            razer_device::camera_focus_family::execute_camera_family_focus(
                &mut transport,
                product_id,
                request,
                validate,
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (validate, scope);
            razer_device::camera_focus_family::CameraFamilyFocusProgress::failed(
                self.product_id,
                request,
                razer_device::camera_focus_family::CameraFamilyFocusPhase::Target,
                "camera family KS adapter is unavailable on this platform".into(),
            )
        }
    }

    fn family_exposure_scoped(
        &mut self,
        request: razer_device::camera_exposure_family::CameraFamilyExposureRequest,
        scope: &CameraScope,
        validate: &impl Fn() -> Result<()>,
    ) -> razer_device::camera_exposure_family::CameraFamilyExposureProgress {
        #[cfg(windows)]
        {
            let product_id = self.session.target().product_id;
            // Focus and exposure dispatch through the same native +344
            // base/X ISP slot, with the entire scan borrowed exclusively.
            let mut transport = focus_family::CameraFamilyKsSession::new(
                self.session.transport_mut(),
                scope,
                validate,
            );
            razer_device::camera_exposure_family::execute_camera_family_exposure(
                &mut transport,
                product_id,
                request,
                validate,
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (validate, scope);
            razer_device::camera_exposure_family::CameraFamilyExposureProgress::failed(
                self.product_id,
                request,
                razer_device::camera_exposure_family::CameraFamilyExposurePhase::Target,
                "camera family KS adapter is unavailable on this platform".into(),
            )
        }
    }

    fn family_processing_scoped(
        &mut self,
        request: razer_device::camera_processing_family::CameraFamilyProcessingRequest,
        scope: &CameraScope,
        validate: &impl Fn() -> Result<()>,
    ) -> razer_device::camera_processing_family::CameraFamilyProcessingProgress {
        #[cfg(windows)]
        {
            let product_id = self.session.target().product_id;
            let mut transport = focus_family::CameraFamilyKsSession::new(
                self.session.transport_mut(),
                scope,
                validate,
            );
            razer_device::camera_processing_family::execute_camera_family_processing(
                &mut transport,
                product_id,
                request,
                validate,
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (validate, scope);
            razer_device::camera_processing_family::CameraFamilyProcessingProgress::failed(
                self.product_id,
                request,
                razer_device::camera_focus_family::CameraFamilyFocusPhase::Target,
                "camera family KS adapter is unavailable on this platform".into(),
            )
        }
    }
}

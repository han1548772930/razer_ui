//! Connection-owned GenericCamera initialization and serial observation.
use super::{CameraScope, ScopedCameraSession, validate_live_scope};
use anyhow::Result;
use razer_device::camera_serial::CameraSerialProgress;
use std::{fmt, time::Duration};

/// A physical connection retains its filter and observed serial on one COM
/// worker. Page changes neither recreate the native cache nor move the filter.
pub struct CameraSerialSession {
    inner: ScopedCameraSession,
}

impl CameraSerialSession {
    pub fn open(scope: CameraScope) -> Result<Self> {
        Ok(Self { inner: ScopedCameraSession::open_mode(scope, false)? })
    }

    pub fn initialize(scope: CameraScope, validate: &impl Fn() -> Result<()>) -> Result<Self> {
        scope.validate()?;
        initialize_with(
            || Self::open(scope.clone()),
            validate,
            |delay| {
                validate()?;
                std::thread::sleep(delay);
                validate()
            },
        ).map_err(anyhow::Error::new)
    }

    pub fn read(&mut self, validate: &impl Fn() -> Result<()>) -> Result<CameraSerialProgress> {
        let target = &self.inner.target;
        let scope = &self.inner.scope;
        let check = || {
            validate()?;
            validate_live_scope(target, scope)
        };
        check()?;
        self.inner.inner.serial_scoped(scope, &check)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OpenFailure {
    wrapper_attempt: u8,
    native_attempt: u8,
    error: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraInitializationFailure {
    attempts: Vec<OpenFailure>,
    interruption: Option<String>,
}

impl CameraInitializationFailure {
    pub fn attempts(&self) -> usize {
        self.attempts.len()
    }

    pub fn interruption(&self) -> Option<&str> {
        self.interruption.as_deref()
    }
}

impl fmt::Display for CameraInitializationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Camera initialization failed after {} open attempts", self.attempts.len())?;
        for attempt in &self.attempts {
            write!(formatter, "; wrapper {} / native {}: {}", attempt.wrapper_attempt, attempt.native_attempt, attempt.error)?;
        }
        if let Some(error) = &self.interruption {
            write!(formatter, "; interrupted: {error}")?;
        }
        Ok(())
    }
}
impl std::error::Error for CameraInitializationFailure {}

fn initialize_with<T>(
    mut open: impl FnMut() -> Result<T>,
    validate: &impl Fn() -> Result<()>,
    mut wait: impl FnMut(Duration) -> Result<()>,
) -> std::result::Result<T, CameraInitializationFailure> {
    let mut failure = CameraInitializationFailure { attempts: Vec::new(), interruption: None };
    let result = (|| -> Result<Option<T>> {
        // rzCamKimberly.init retries DeviceInitialize ten times with 100 ms
        // after false. Each native 577e0/57640 call tries thirty binds with
        // 1000 ms after failure, including both loops' final failed attempt.
        for wrapper_attempt in 1..=10 {
            for native_attempt in 1..=30 {
                validate()?;
                match open() {
                    Ok(session) => {
                        validate()?;
                        return Ok(Some(session));
                    }
                    Err(error) => failure.attempts.push(OpenFailure {
                        wrapper_attempt,
                        native_attempt,
                        error: format!("{error:#}"),
                    }),
                }
                validate()?;
                wait(Duration::from_millis(1000))?;
            }
            validate()?;
            wait(Duration::from_millis(100))?;
        }
        Ok(None)
    })();
    match result {
        Ok(Some(session)) => Ok(session),
        Ok(None) => Err(failure),
        Err(error) => {
            failure.interruption = Some(format!("{error:#}"));
            Err(failure)
        }
    }
}

#[cfg(test)]
#[path = "serial_tests.rs"]
mod tests;

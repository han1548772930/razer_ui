use super::super::mapping_native_input::WindowsOsInputTransport;
use anyhow::{Result, ensure};
use razer_device::{
    macro_runtime::{Keyboard, MovementMode},
    mapping_native_input::{
        InputReportOrigin, KeyboardInput, MouseInput, NativeInput, NativeInputExecution,
        OsInputExecution, encode_mouse,
    },
    mapping_native_output::{CompletionPoll, MappingOutputTransport, ReportDelivery},
};
use std::{
    collections::BTreeMap,
    mem::{MaybeUninit, size_of},
    sync::Arc,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum HeldIdentity {
    Keyboard {
        unit: u16,
        scancode: u16,
        flags: u16,
    },
    Mouse {
        unit: u16,
        buttons: u32,
    },
}

fn release_identity(input: NativeInput) -> Option<(HeldIdentity, bool, NativeInput)> {
    match input {
        NativeInput::Keyboard(key) => Some((
            HeldIdentity::Keyboard {
                unit: key.unit_id,
                scancode: key.scancode,
                flags: key.flag & !1,
            },
            key.is_released(),
            NativeInput::Keyboard(key.released()),
        )),
        NativeInput::Mouse(mouse) if mouse.is_pressed() || mouse.is_released() => Some((
            HeldIdentity::Mouse {
                unit: mouse.unit_id,
                buttons: mouse.pressed().packed_buttons(),
            },
            mouse.is_released(),
            NativeInput::Mouse(mouse.released()),
        )),
        _ => None,
    }
}
use windows_sys::Win32::{
    Foundation::GetLastError,
    UI::Input::KeyboardAndMouse::{INPUT, INPUT_MOUSE, SendInput},
};

#[derive(Clone, Debug)]
pub enum Receipt {
    Driver(NativeInputExecution),
    OperatingSystem(OsInputExecution),
    Movement { x: i32, y: i32, driver: bool },
}

impl std::fmt::Display for Receipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Driver(execution) => write!(formatter, "driver {execution:?}"),
            Self::OperatingSystem(execution) => write!(formatter, "OS {execution:?}"),
            Self::Movement { x, y, driver } => {
                write!(formatter, "movement x={x} y={y} driver={driver}")
            }
        }
    }
}

pub(super) struct Input {
    driver: Option<super::driver::Output>,
    pub last_receipt: Option<Receipt>,
    pub completed: u64,
    relative: Option<(u64, i32, i32)>,
    cancellation: Arc<super::Cancellation>,
    pub cancellable: bool,
    acceleration: super::acceleration::Owner,
    lease: Option<super::acceleration::Lease>,
    pub operation_errors: Vec<String>,
    held: BTreeMap<HeldIdentity, NativeInput>,
    /// Clipboard modifier restoration transfers those keys back to the user.
    pub retain_held_outputs: bool,
}

impl Input {
    pub fn open(
        route: Option<&super::driver::Route>,
        cancellation: Arc<super::Cancellation>,
        acceleration: super::acceleration::Owner,
    ) -> Result<Self> {
        Ok(Self {
            driver: route.map(super::driver::Route::output),
            last_receipt: None,
            completed: 0,
            relative: None,
            cancellation,
            cancellable: true,
            acceleration,
            lease: None,
            operation_errors: Vec::new(),
            held: BTreeMap::new(),
            retain_held_outputs: true,
        })
    }

    pub fn has_driver(&self) -> bool {
        self.driver.is_some()
    }

    pub fn keyboard(&mut self, key: &Keyboard) -> Result<(), String> {
        self.send(NativeInput::Keyboard(KeyboardInput {
            unit_id: key.unit.unwrap_or(0),
            scancode: key.scancode,
            flag: key.flag,
            modifiers: key.modifiers,
            reserved: 0,
        }))
        .map_err(|error| format!("{error:#}"))
    }

    pub fn send(&mut self, input: NativeInput) -> Result<()> {
        let release = release_identity(input);
        if let Some((identity, true, _)) = release {
            // Each source break is attempted once. A failed break is retained
            // as a failure receipt, not silently retried during endpoint close.
            self.held.remove(&identity);
        }
        let mut os = WindowsOsInputTransport;
        let accepted = if let Some(driver) = &mut self.driver {
            let mut execution = NativeInputExecution::submit_with_os_fallback(
                driver,
                &mut os,
                &[input],
                Duration::ZERO,
            );
            let mut cancelled = false;
            while execution.is_pending() {
                if !cancelled && self.cancellable && self.cancellation.is_cancelled() {
                    execution.cancel_pending(driver, Duration::ZERO);
                    cancelled = true;
                }
                execution.poll_with_os_fallback(driver, &mut os, Duration::from_millis(5));
            }
            let accepted = execution.reports.iter().all(|report| {
                report.delivery == ReportDelivery::Completed
                    || matches!(report.origin, InputReportOrigin::Primary { .. })
                        && report.os_fallback.as_ref().is_some_and(|result| {
                            result.as_ref().is_ok_and(OsInputExecution::os_accepted)
                        })
            });
            self.last_receipt = Some(Receipt::Driver(execution));
            accepted
        } else {
            let execution = OsInputExecution::submit(&mut os, input)?;
            let accepted = execution.os_accepted();
            self.last_receipt = Some(Receipt::OperatingSystem(execution));
            accepted
        };
        ensure!(accepted, "Input output failed: {:?}", self.last_receipt);
        if self.retain_held_outputs
            && let Some((identity, false, release)) = release
        {
            self.held.entry(identity).or_insert(release);
        }
        self.completed = self.completed.saturating_add(1);
        Ok(())
    }

    pub fn movement(&mut self, mode: MovementMode, x: i32, y: i32, epoch: u64) -> Result<()> {
        let (x, y, absolute) = match mode {
            MovementMode::None => anyhow::bail!("Invalid mouseMoveType none"),
            MovementMode::Absolute => (x, y, true),
            MovementMode::Relative => {
                if self.lease.is_none() {
                    let (lease, error) = self.acceleration.acquire();
                    self.lease = Some(lease);
                    if let Some(error) = error {
                        self.operation_errors.push(error);
                    }
                }
                let previous = self.relative.replace((epoch, x, y));
                let Some((old_epoch, old_x, old_y)) =
                    previous.filter(|previous| previous.0 == epoch)
                else {
                    return Ok(());
                };
                let _ = old_epoch;
                (x.wrapping_sub(old_x), y.wrapping_sub(old_y), false)
            }
            // 12c68f stores the current type-28 event in owner+128 before
            // 130dc6 tests that field. The macro path therefore reuses the
            // constructor's zero viewport (126f1c); SSE cvttps2dq produces
            // INT_MIN for both non-finite coordinates. Do not substitute a
            // newly queried foreground rectangle for this source behavior.
            MovementMode::Foreground => (i32::MIN, i32::MIN, true),
        };
        if let Some(driver) = &mut self.driver {
            let report = encode_mouse(MouseInput {
                flags: u16::from(absolute),
                movement: [x, y],
                ..Default::default()
            });
            let id = driver.submit(report)?;
            let mut cancelled = false;
            loop {
                if !cancelled && self.cancellable && self.cancellation.is_cancelled() {
                    driver.cancel(id)?;
                    cancelled = true;
                }
                match driver.poll_completion(id, Duration::from_millis(5))? {
                    CompletionPoll::Pending => {}
                    CompletionPoll::Completed(result) => {
                        result?;
                        break;
                    }
                }
            }
        } else {
            let mut input = MaybeUninit::<INPUT>::zeroed();
            unsafe {
                let native = input.as_mut_ptr();
                (*native).r#type = INPUT_MOUSE;
                (*native).Anonymous.mi.dx = x;
                (*native).Anonymous.mi.dy = y;
                (*native).Anonymous.mi.dwFlags = if absolute { 0x8001 } else { 1 };
            }
            let inserted = unsafe { SendInput(1, input.as_ptr(), size_of::<INPUT>() as i32) };
            ensure!(
                inserted == 1,
                "Mouse movement SendInput inserted {inserted}: {}",
                unsafe { GetLastError() }
            );
        }
        self.completed = self.completed.saturating_add(1);
        self.last_receipt = Some(Receipt::Movement {
            x,
            y,
            driver: self.driver.is_some(),
        });
        Ok(())
    }

    pub fn release_held(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        self.cancellable = false;
        for release in std::mem::take(&mut self.held).into_values() {
            if let Err(error) = self.send(release) {
                errors.push(format!("Release held output: {error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        if let Err(error) = self.release_held() {
            errors.push(format!("{error:#}"));
        }
        if let Some(driver) = &mut self.driver {
            if let Err(error) = driver.close() {
                errors.push(error.to_string());
            }
        }
        self.driver = None;
        if let Some(mut lease) = self.lease.take() {
            if let Err(error) = lease.release() {
                errors.push(error);
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("Mapping input cleanup failed: {error:#}");
        }
    }
}

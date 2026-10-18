//! Shared relative-movement lease, current 5e200/17d4aa/17d566.
use std::sync::{Arc, Mutex};
use windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;

#[derive(Default)]
struct State {
    leases: usize,
    original: Option<i32>,
}

#[derive(Clone, Default)]
pub(super) struct Owner(Arc<Mutex<State>>);
pub(super) struct Lease {
    owner: Owner,
    released: bool,
}

trait Settings {
    fn read(&mut self) -> Result<[i32; 3], String>;
    fn write(&mut self, value: [i32; 3]) -> Result<(), String>;
}
struct WindowsSettings;
impl Settings for WindowsSettings {
    fn read(&mut self) -> Result<[i32; 3], String> {
        let mut value = [0i32; 3];
        if unsafe { SystemParametersInfoW(3, 0, value.as_mut_ptr().cast(), 0) } != 0 {
            Ok(value)
        } else {
            Err(format!("SPI_GETMOUSE: {}", std::io::Error::last_os_error()))
        }
    }
    fn write(&mut self, mut value: [i32; 3]) -> Result<(), String> {
        if unsafe { SystemParametersInfoW(4, 0, value.as_mut_ptr().cast(), 2) } != 0 {
            Ok(())
        } else {
            Err(format!("SPI_SETMOUSE: {}", std::io::Error::last_os_error()))
        }
    }
}
impl State {
    fn acquire(&mut self, settings: &mut impl Settings) -> Result<(), String> {
        let first = self.leases == 0;
        self.leases += 1;
        if first {
            let mut value = settings.read()?;
            self.original = Some(value[2]);
            if value[2] != 0 {
                value[2] = 0;
                settings.write(value)?;
            }
        }
        Ok(())
    }
    fn release(&mut self, settings: &mut impl Settings) -> Result<(), String> {
        assert!(self.leases != 0);
        self.leases -= 1;
        if self.leases == 0 {
            let mut value = settings.read()?;
            if let Some(original) = self.original.take() {
                if value[2] != original {
                    value[2] = original;
                    settings.write(value)?;
                }
            }
        }
        Ok(())
    }
}
impl Owner {
    pub fn acquire(&self) -> (Lease, Option<String>) {
        let result = self
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .acquire(&mut WindowsSettings);
        (
            Lease {
                owner: self.clone(),
                released: false,
            },
            result.err(),
        )
    }
}
impl Lease {
    pub fn release(&mut self) -> Result<(), String> {
        if self.released {
            return Ok(());
        }
        self.released = true;
        self.owner
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .release(&mut WindowsSettings)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Err(error) = self.release() {
            eprintln!("Relative mouse cleanup: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        value: [i32; 3],
        writes: Vec<[i32; 3]>,
        fail_read: bool,
    }
    impl Settings for Fake {
        fn read(&mut self) -> Result<[i32; 3], String> {
            if self.fail_read {
                Err("read failed".into())
            } else {
                Ok(self.value)
            }
        }
        fn write(&mut self, value: [i32; 3]) -> Result<(), String> {
            self.value = value;
            self.writes.push(value);
            Ok(())
        }
    }
    #[test]
    fn overlapping_relative_runs_restore_once_and_keep_current_thresholds() {
        let mut settings = Fake {
            value: [6, 10, 2],
            writes: Vec::new(),
            fail_read: false,
        };
        let mut state = State::default();
        state.acquire(&mut settings).unwrap();
        state.acquire(&mut settings).unwrap();
        assert_eq!(settings.writes, [[6, 10, 0]]);
        settings.value = [7, 11, 0];
        state.release(&mut settings).unwrap();
        assert_eq!(settings.writes.len(), 1);
        state.release(&mut settings).unwrap();
        assert_eq!(settings.writes, [[6, 10, 0], [7, 11, 2]]);
    }
    #[test]
    fn failed_acquisition_does_not_invent_restore_value() {
        let mut settings = Fake {
            value: [6, 10, 2],
            writes: Vec::new(),
            fail_read: true,
        };
        let mut state = State::default();
        assert!(state.acquire(&mut settings).is_err());
        settings.fail_read = false;
        state.release(&mut settings).unwrap();
        assert!(settings.writes.is_empty());
    }
}

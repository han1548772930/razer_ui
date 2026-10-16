//! Focused window keyboard events for browser keyCode/location consumers.
//! The Windows adapter observes the owning UI thread's message queue; it never
//! redirects input, consumes another window's keys or installs a global hook.
use std::thread::{self, ThreadId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardEvent {
    key_code: u32,
    location: u8,
    physical_code: u32,
    key_up: bool,
    held: bool,
}
impl KeyboardEvent {
    /// Pure decoding of WM_KEYDOWN/UP and WM_SYSKEYDOWN/UP. The message lParam
    /// preserves scan code, extended side, previous key state and key release.
    pub fn from_windows_message(message: u32, virtual_key: u32, lparam: usize) -> Option<Self> {
        let key_up = match message {
            0x100 | 0x104 => false,
            0x101 | 0x105 => true,
            _ => return None,
        };
        let scan = ((lparam >> 16) & 0xff) as u32;
        let extended = lparam & (1 << 24) != 0;
        let key_code = match virtual_key {
            0xa0 | 0xa1 => 16,
            0xa2 | 0xa3 => 17,
            0xa4 | 0xa5 => 18,
            value => value,
        };
        let location = match virtual_key {
            0xa1 | 0xa3 | 0xa5 | 0x5c => 2,
            0xa0 | 0xa2 | 0xa4 | 0x5b => 1,
            16 => {
                if scan == 0x36 {
                    2
                } else {
                    1
                }
            }
            17 | 18 => {
                if extended {
                    2
                } else {
                    1
                }
            }
            0x60..=0x6f | 0x90 => 3,
            13 if extended => 3,
            0x21..=0x28 | 0x2d | 0x2e if !extended && (0x47..=0x53).contains(&scan) => 3,
            _ => 0,
        };
        Some(Self {
            key_code,
            location,
            physical_code: scan * 2 + u32::from(extended),
            key_up,
            held: !key_up && lparam & (1 << 30) != 0,
        })
    }
    pub fn key_code(&self) -> u32 {
        self.key_code
    }
    pub fn location(&self) -> u8 {
        self.location
    }
    pub fn physical_code(&self) -> u32 {
        self.physical_code
    }
    pub fn key_up(&self) -> bool {
        self.key_up
    }
    pub fn held(&self) -> bool {
        self.held
    }
}

pub struct KeyboardCapture {
    owner: ThreadId,
    #[cfg(windows)]
    hook: crate::platform::windows::keyboard_capture::Hook,
}
impl KeyboardCapture {
    pub fn start_on_message_thread() -> anyhow::Result<Self> {
        #[cfg(windows)]
        return Ok(Self {
            owner: thread::current().id(),
            hook: crate::platform::windows::keyboard_capture::Hook::start()?,
        });
        #[cfg(not(windows))]
        anyhow::bail!("Focused raw keyboard capture is unavailable on this platform");
    }
    pub fn drain_on_message_thread(&mut self) -> anyhow::Result<Vec<KeyboardEvent>> {
        anyhow::ensure!(
            self.owner == thread::current().id(),
            "Keyboard capture left its UI message thread"
        );
        #[cfg(windows)]
        return self.hook.drain();
        #[cfg(not(windows))]
        anyhow::bail!("Focused raw keyboard capture is unavailable on this platform");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decode(message: u32, key: u32, scan: usize, extended: bool, repeat: bool) -> KeyboardEvent {
        KeyboardEvent::from_windows_message(
            message,
            key,
            (scan << 16) | (usize::from(extended) << 24) | (usize::from(repeat) << 30),
        )
        .unwrap()
    }
    #[test]
    fn focused_key_capture_preserves_sides_keyup_and_repeat() {
        let left = decode(0x100, 17, 0x1d, false, false);
        let right = decode(0x100, 17, 0x1d, true, false);
        assert_eq!((left.key_code(), left.location()), (17, 1));
        assert_eq!((right.key_code(), right.location()), (17, 2));
        assert_ne!(left.physical_code(), right.physical_code());
        assert!(decode(0x100, 65, 0x1e, false, true).held());
        assert!(!decode(0x101, 65, 0x1e, false, true).held());
        assert!(decode(0x105, 18, 0x38, true, false).key_up());
        assert_eq!(decode(0x100, 16, 0x36, false, false).location(), 2);
        assert!(KeyboardEvent::from_windows_message(0x102, 65, 0).is_none());
    }
    #[test]
    fn focused_key_capture_keeps_print_caps_and_numpad_identity() {
        assert_eq!(decode(0x101, 44, 0x37, true, false).key_code(), 44);
        assert_eq!(decode(0x100, 20, 0x3a, false, false).key_code(), 20);
        assert_eq!(
            (
                decode(0x100, 97, 0x4f, false, false).key_code(),
                decode(0x100, 97, 0x4f, false, false).location()
            ),
            (97, 3)
        );
        assert_eq!(decode(0x100, 13, 0x1c, true, false).location(), 3);
        assert_eq!(decode(0x100, 35, 0x4f, false, false).location(), 3);
        assert_eq!(decode(0x100, 35, 0x4f, true, false).location(), 0);
    }
}

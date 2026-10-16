//! Win32 adapter for the mapping DLL's failed-driver input fallback.
//! No vendor DLL is loaded. Calling `send_one` performs a real OS input action;
//! verification of this adapter is compile-only, with simulated transports in
//! the shared crate covering the failure and continuation branches.

use razer_device::mapping_native_input::{OsInput, OsInputError, OsInputTransport};
use std::mem::{MaybeUninit, size_of};
use windows_sys::Win32::{
    Foundation::GetLastError,
    UI::Input::KeyboardAndMouse::{INPUT, INPUT_KEYBOARD, INPUT_MOUSE, SendInput},
};

pub struct WindowsOsInputTransport;

impl OsInputTransport for WindowsOsInputTransport {
    fn send_one(&mut self, input: OsInput) -> Result<(), OsInputError> {
        // RVA 0x11C510 zeroes all INPUT bytes before setting only these fields.
        // In particular, it never copies driver modifiers, mouse movement,
        // time, virtual keys or extra information into the fallback request.
        // Fill fields in-place in a zeroed allocation so unused union bytes
        // and ABI padding also remain zero, matching the recovered memset.
        let mut native = MaybeUninit::<INPUT>::zeroed();
        unsafe {
            let native = native.as_mut_ptr();
            match input {
                OsInput::Keyboard { scancode, flags } => {
                    (*native).r#type = INPUT_KEYBOARD;
                    (*native).Anonymous.ki.wScan = scancode;
                    (*native).Anonymous.ki.dwFlags = flags;
                }
                OsInput::Mouse { flags, data } => {
                    (*native).r#type = INPUT_MOUSE;
                    (*native).Anonymous.mi.mouseData = data;
                    (*native).Anonymous.mi.dwFlags = flags;
                }
            }
        }
        // The current x64 DLL passes 40; size_of supplies the target's Win32
        // INPUT ABI instead of hard-coding a size unsuitable for Windows x86.
        let inserted = unsafe { SendInput(1, native.as_ptr(), size_of::<INPUT>() as i32) };
        if inserted == 1 {
            Ok(())
        } else {
            Err(OsInputError::SendInput {
                inserted,
                win32_code: unsafe { GetLastError() },
            })
        }
    }
}

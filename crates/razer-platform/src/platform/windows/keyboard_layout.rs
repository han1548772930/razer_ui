//! IDA RVA 0x70b10 -> 0x70aa0, current SysUtilsNative.dll.
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayoutNameA;

pub(crate) fn get() -> i32 {
    // The original initializes the buffer and result to zero and ignores the
    // GetKeyboardLayoutNameA BOOL. It queries this thread, not the foreground
    // thread or a saved user/profile setting. Keep that source distinction.
    let mut klid = [0u8; 64];
    unsafe { GetKeyboardLayoutNameA(klid.as_mut_ptr()) };
    crate::keyboard_layout::parse_klid(&klid)
}

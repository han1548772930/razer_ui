//! DLL-free SysUtilsNative wheel-scroll settings.
//!
//! Current IDA evidence: SysUtilsNative.dll, getWheelScrollLines RVA 0x77f10
//! and setWheelScrollLines RVA 0x77f60. This is the Windows per-user scroll
//! setting, not a Razer HID request. See sysutils-wheel-scroll-current-evidence.json.

/// Read the original signed FFI value, preserving the page-scroll sentinel -1.
/// The source initializes zero and ignores the Windows query's BOOL result.
pub fn get() -> anyhow::Result<i32> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::wheel_scroll::get()
    }
    #[cfg(not(target_os = "windows"))]
    anyhow::bail!("Windows wheel-scroll settings are unsupported on this platform")
}

/// Write the exact i32 argument's UINT bit pattern and return the original BOOL.
/// The original uses fWinIni=0: no added persistence or settings broadcast.
pub fn set(lines: i32) -> anyhow::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::wheel_scroll::set(lines)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = lines;
        anyhow::bail!("Windows wheel-scroll settings are unsupported on this platform")
    }
}

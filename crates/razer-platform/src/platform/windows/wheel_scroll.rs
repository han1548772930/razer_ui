//! Windows SPI implementation of current SysUtilsNative wheel-scroll exports.

pub(crate) fn get() -> anyhow::Result<i32> {
    let mut lines = 0u32;
    // The live, writable UINT buffer matches the original stack variable.
    // No library initialization, callback, handle or allocation is needed.
    unsafe { SystemParametersInfoW(0x68, 0, (&mut lines as *mut u32).cast(), 0) };
    Ok(lines as i32)
}

pub(crate) fn set(lines: i32) -> anyhow::Result<bool> {
    // This action changes the user setting only when the caller requests it.
    let result = unsafe { SystemParametersInfoW(0x69, lines as u32, std::ptr::null_mut(), 0) };
    Ok(result != 0)
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SystemParametersInfoW(
        ui_action: u32,
        ui_param: u32,
        pv_param: *mut std::ffi::c_void,
        f_win_ini: u32,
    ) -> i32;
}

//! SysUtilsNative current 01223bf: getMonitorInfo718c0, callback70d50,
//! getVirtualScreenRect71ba0. The adapter reads the same local Win32 sources.
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub monitors: Value,
    pub screen: Value,
}

#[cfg(not(windows))]
pub fn snapshot() -> anyhow::Result<Snapshot> {
    anyhow::bail!("Macro monitor geometry is not implemented for this operating system")
}

#[cfg(windows)]
pub fn snapshot() -> anyhow::Result<Snapshot> {
    use anyhow::ensure;
    use serde_json::json;
    use std::{
        mem::size_of,
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{
            DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, HDC,
            HMONITOR, MONITORINFO,
        },
        UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    };
    struct Capture {
        names: Vec<String>,
        monitors: Vec<Value>,
        bounds: Option<[i32; 4]>,
        error: Option<String>,
    }
    unsafe extern "system" fn monitor(
        h: HMONITOR,
        _: HDC,
        _: *mut RECT,
        data: LPARAM,
    ) -> windows_sys::core::BOOL {
        // The OS invokes synchronously while Capture remains owned by snapshot.
        let capture = unsafe { &mut *(data as *mut Capture) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> anyhow::Result<()> {
                let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
                info.cbSize = size_of::<MONITORINFO>() as u32;
                ensure!(
                    unsafe { GetMonitorInfoW(h, &mut info) } != 0,
                    "GetMonitorInfoW failed: {}",
                    std::io::Error::last_os_error()
                );
                let (mut x, mut y) = (0, 0);
                let hr = unsafe { GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut x, &mut y) };
                ensure!(
                    hr >= 0 && x > 0 && y > 0,
                    "GetDpiForMonitor failed: 0x{hr:08x}"
                );
                let r = info.rcMonitor;
                let b = capture
                    .bounds
                    .get_or_insert([r.left, r.top, r.right, r.bottom]);
                b[0] = b[0].min(r.left);
                b[1] = b[1].min(r.top);
                b[2] = b[2].max(r.right);
                b[3] = b[3].max(r.bottom);
                let rect =
                    |r: RECT| json!({"left":r.left,"top":r.top,"right":r.right,"bottom":r.bottom});
                capture.monitors.push(json!({"deviceName":capture.names.get(capture.monitors.len()),
                "dpiScaleX":f64::from(x)/96.,"dpiScaleY":f64::from(y)/96.,"isPrimaryMonitor":info.dwFlags==1,
                "MonitorRect":rect(r),"WorkRect":rect(info.rcWork)}));
                Ok(())
            },
        ));
        match result {
            Ok(Ok(())) => 1,
            Ok(Err(error)) => {
                capture.error = Some(error.to_string());
                0
            }
            Err(_) => {
                capture.error = Some("Monitor enumeration callback failed".into());
                0
            }
        }
    }
    let mut capture = Capture {
        names: Vec::new(),
        monitors: Vec::new(),
        bounds: None,
        error: None,
    };
    let mut index = 0;
    loop {
        let mut device = DISPLAY_DEVICEW::default();
        device.cb = size_of::<DISPLAY_DEVICEW>() as u32;
        if unsafe { EnumDisplayDevicesW(null(), index, &mut device, 0) } == 0 {
            break;
        }
        let length = device
            .DeviceName
            .iter()
            .position(|v| *v == 0)
            .unwrap_or(device.DeviceName.len());
        capture
            .names
            .push(String::from_utf16_lossy(&device.DeviceName[..length]));
        let mut child = DISPLAY_DEVICEW::default();
        child.cb = size_of::<DISPLAY_DEVICEW>() as u32;
        unsafe { EnumDisplayDevicesW(device.DeviceName.as_ptr(), 0, &mut child, 0) };
        if child.DeviceName[0] == 0 {
            break;
        }
        index += 1;
    }
    let success = unsafe {
        EnumDisplayMonitors(
            null_mut(),
            null(),
            Some(monitor),
            &mut capture as *mut Capture as LPARAM,
        )
    };
    if let Some(error) = capture.error {
        anyhow::bail!(error);
    }
    ensure!(
        success != 0,
        "EnumDisplayMonitors failed: {}",
        std::io::Error::last_os_error()
    );
    let [left, top, right, bottom] = capture
        .bounds
        .ok_or_else(|| anyhow::anyhow!("No local monitor was observed"))?;
    Ok(Snapshot {
        monitors: capture.monitors.into(),
        screen: json!({"MonitorRect":{"left":left,"top":top,"width":i64::from(right)-i64::from(left),"height":i64::from(bottom)-i64::from(top)}}),
    })
}

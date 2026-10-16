//! Windows SCM adapter for current RzPowerTool service semantics.

pub(crate) fn start(service_name: &str) -> anyhow::Result<u32> {
    Ok(control(service_name, Operation::Start))
}

pub(crate) fn stop(service_name: &str) -> anyhow::Result<u32> {
    Ok(control(service_name, Operation::Stop))
}

const SUPPORTED_SERVICES: [&str; 12] = [
    "Razer Game Manager Service 3",
    "Razer Chroma SDK Server",
    "Razer Chroma SDK Service",
    "Razer Chroma Stream Server",
    "Razer Synapse Service",
    "VSSrv",
    "Audiosrv",
    "XTU3SERVICE",
    "HapticService",
    "RazerExperienceService",
    "Carol Routing Service",
    "razer_elevation_service",
];

#[derive(Clone, Copy)]
enum Operation {
    Start,
    Stop,
}

fn control(service_name: &str, operation: Operation) -> u32 {
    // Only the top-level source DoStart/DoStop functions apply this gate.
    // The SCM supplies dependency names to recursive handle functions.
    if !SUPPORTED_SERVICES
        .iter()
        .any(|name| name.eq_ignore_ascii_case(service_name))
    {
        return 0;
    }
    let name = service_name
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let Some(manager) =
        Handle::new(unsafe { OpenSCManagerW(std::ptr::null(), std::ptr::null(), 0xF003F) })
    else {
        return 0;
    };
    u32::from(control_handle(&manager, name.as_ptr(), operation))
}

fn control_handle(manager: &Handle, name: *const u16, operation: Operation) -> bool {
    if name.is_null() {
        return false;
    }
    let access = match operation {
        Operation::Start => 0xF01FF,
        Operation::Stop => 0x2C,
    };
    // Name is from the live caller UTF16 buffer or a live SCM enumeration.
    let Some(service) = Handle::new(unsafe { OpenServiceW(manager.0, name, access) }) else {
        return false;
    };
    match operation {
        Operation::Start => {
            if !start_handle(&service) {
                return false;
            }
            // Source ignores dependency enumeration and child return values.
            let _ = control_dependents(manager, &service, operation);
            true
        }
        Operation::Stop => {
            let _ = control_dependents(manager, &service, operation);
            stop_handle(&service)
        }
    }
}

fn query_status(service: &Handle, status: &mut ServiceStatusProcess) -> bool {
    let mut needed = 0u32;
    // A full nine-DWORD buffer safely backs the source's 0x24-byte query.
    unsafe {
        QueryServiceStatusEx(
            service.0,
            0,
            (status as *mut ServiceStatusProcess).cast(),
            std::mem::size_of::<ServiceStatusProcess>() as u32,
            &mut needed,
        ) != 0
    }
}

fn polling_delay(wait_hint: u32) -> u32 {
    (wait_hint / 10).clamp(1000, 10000)
}

fn elapsed_since(started: u64) -> u64 {
    // The original subtracts unsigned GetTickCount64 values.
    unsafe { GetTickCount64().wrapping_sub(started) }
}

fn start_handle(service: &Handle) -> bool {
    let mut status = ServiceStatusProcess::default();
    if !query_status(service, &mut status) {
        return false;
    }
    // The source treats every state other than stopped/stop-pending as already
    // running and returns true, including paused/start-pending/invalid states.
    if !matches!(status.current_state, 1 | 3) {
        return true;
    }
    let mut started = unsafe { GetTickCount64() };
    let mut checkpoint = status.check_point;
    while status.current_state == 3 {
        unsafe { Sleep(polling_delay(status.wait_hint)) };
        if !query_status(service, &mut status) {
            return false;
        }
        if status.check_point > checkpoint {
            started = unsafe { GetTickCount64() };
            checkpoint = status.check_point;
        } else if elapsed_since(started) > u64::from(status.wait_hint) {
            // This check occurs before the next state test in the original.
            return false;
        }
    }
    if unsafe { StartServiceW(service.0, 0, std::ptr::null()) } == 0 {
        return false;
    }
    if !query_status(service, &mut status) {
        return false;
    }
    started = unsafe { GetTickCount64() };
    checkpoint = status.check_point;
    while status.current_state == 2 {
        let delay = polling_delay(status.wait_hint);
        unsafe { Sleep(delay) };
        if !query_status(service, &mut status) {
            // Source breaks and inspects the same buffer on this failure.
            break;
        }
        if status.check_point > checkpoint {
            started = unsafe { GetTickCount64() };
            checkpoint = status.check_point;
        } else if elapsed_since(started) > u64::from(30 * delay) {
            break;
        }
    }
    status.current_state == 4
}

fn stop_handle(service: &Handle) -> bool {
    // Source's fixed timeout baseline precedes the very first query; it is
    // never reset by checkpoint changes or before issuing ControlService.
    let started = unsafe { GetTickCount64() };
    let mut status = ServiceStatusProcess::default();
    if !query_status(service, &mut status) {
        return false;
    }
    if status.current_state == 1 {
        return true;
    }
    while status.current_state == 3 {
        unsafe { Sleep(polling_delay(status.wait_hint)) };
        if !query_status(service, &mut status) {
            return false;
        }
        if status.current_state == 1 {
            return true;
        }
        if elapsed_since(started) > 30000 {
            return false;
        }
    }
    // ControlService writes seven DWORDs; our larger status buffer has the
    // identical prefix and retains enough room for the following 36-byte query.
    if unsafe {
        ControlService(
            service.0,
            1,
            (&mut status as *mut ServiceStatusProcess).cast(),
        )
    } == 0
    {
        return false;
    }
    while status.current_state != 1 {
        // Unlike pending-state polling, the post-control source sleeps the
        // entire waitHint without clamping; timeout is evaluated afterwards.
        unsafe { Sleep(status.wait_hint) };
        if !query_status(service, &mut status) {
            return false;
        }
        if status.current_state == 1 {
            break;
        }
        if elapsed_since(started) > 30000 {
            return false;
        }
    }
    true
}

fn control_dependents(manager: &Handle, service: &Handle, operation: Operation) -> bool {
    let state_filter = match operation {
        Operation::Start => 3, // SERVICE_STATE_ALL
        Operation::Stop => 1,  // SERVICE_ACTIVE
    };
    let mut needed = 0u32;
    let mut count = 0u32;
    // The first call probes the necessary bytes, not a list count allocation.
    if unsafe {
        EnumDependentServicesW(
            service.0,
            state_filter,
            std::ptr::null_mut(),
            0,
            &mut needed,
            &mut count,
        )
    } != 0
    {
        return true;
    }
    if unsafe { GetLastError() } != 234 {
        // ERROR_MORE_DATA
        return false;
    }
    let size = needed;
    // Windows process heap guarantees the required structure alignment.
    let Some(buffer) = HeapBuffer::new(unsafe { HeapAlloc(GetProcessHeap(), 8, size as usize) })
    else {
        return false;
    };
    if unsafe {
        EnumDependentServicesW(
            service.0,
            state_filter,
            buffer.0.cast(),
            size,
            &mut needed,
            &mut count,
        )
    } != 0
    {
        for index in 0..count {
            // Successful enumeration guarantees `count` live records and
            // NUL-terminated names inside the buffer until it is freed.
            let record = unsafe { &*buffer.0.cast::<EnumServiceStatus>().add(index as usize) };
            let _ = control_handle(manager, record.service_name, operation);
        }
    }
    // Source returns true even if the second enumeration or children failed.
    // The RAII guard frees the allocation after all recursive calls complete.
    true
}

struct HeapBuffer(*mut std::ffi::c_void);

impl HeapBuffer {
    fn new(raw: *mut std::ffi::c_void) -> Option<Self> {
        (!raw.is_null()).then_some(Self(raw))
    }
}

impl Drop for HeapBuffer {
    fn drop(&mut self) {
        unsafe { HeapFree(GetProcessHeap(), 0, self.0) };
    }
}

#[repr(C)]
struct EnumServiceStatus {
    service_name: *const u16,
    display_name: *const u16,
    status: [u32; 7],
}

/// Return the helper's exit-code semantics: 0 on unsupported service/API failure,
/// 1 for stopped, 3 for stop-pending, and 4 for every other queried state.
/// In particular, the original also collapses paused/start-pending to 4.
pub(crate) fn query(service_name: &str) -> anyhow::Result<u32> {
    // The original service allowlist uses its default-C wide comparison,
    // which folds ASCII A-Z only. No arbitrary service is queried.
    if !SUPPORTED_SERVICES
        .iter()
        .any(|name| name.eq_ignore_ascii_case(service_name))
    {
        return Ok(0);
    }
    let name = service_name
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // Exact access masks from the current helper, including the original
    // failure when this caller cannot acquire these rights.
    let Some(manager) =
        Handle::new(unsafe { OpenSCManagerW(std::ptr::null(), std::ptr::null(), 0xF003F) })
    else {
        return Ok(0);
    };
    let Some(service) = Handle::new(unsafe { OpenServiceW(manager.0, name.as_ptr(), 0xF01FF) })
    else {
        return Ok(0);
    };
    let mut status = ServiceStatusProcess::default();
    let mut needed = 0u32;
    // SC_STATUS_PROCESS_INFO is zero; nine DWORDs match the 0x24-byte
    // source buffer. The API initializes fields before they are consumed.
    let queried = unsafe {
        QueryServiceStatusEx(
            service.0,
            0,
            (&mut status as *mut ServiceStatusProcess).cast(),
            std::mem::size_of::<ServiceStatusProcess>() as u32,
            &mut needed,
        )
    };
    if queried == 0 {
        return Ok(0);
    }
    Ok(match status.current_state {
        1 | 3 => status.current_state,
        _ => 4,
    })
}

struct Handle(*mut std::ffi::c_void);

impl Handle {
    fn new(raw: *mut std::ffi::c_void) -> Option<Self> {
        (!raw.is_null()).then_some(Self(raw))
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // RAII closes service first, then manager, on all success/failure paths.
        unsafe { CloseServiceHandle(self.0) };
    }
}

#[repr(C)]
#[derive(Default)]
struct ServiceStatusProcess {
    service_type: u32,
    current_state: u32,
    controls_accepted: u32,
    win32_exit_code: u32,
    service_specific_exit_code: u32,
    check_point: u32,
    wait_hint: u32,
    process_id: u32,
    service_flags: u32,
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenSCManagerW(
        machine: *const u16,
        database: *const u16,
        access: u32,
    ) -> *mut std::ffi::c_void;
    fn OpenServiceW(
        manager: *mut std::ffi::c_void,
        name: *const u16,
        access: u32,
    ) -> *mut std::ffi::c_void;
    fn QueryServiceStatusEx(
        service: *mut std::ffi::c_void,
        info_level: i32,
        buffer: *mut u8,
        size: u32,
        needed: *mut u32,
    ) -> i32;
    fn CloseServiceHandle(service: *mut std::ffi::c_void) -> i32;
    fn StartServiceW(
        service: *mut std::ffi::c_void,
        count: u32,
        arguments: *const *const u16,
    ) -> i32;
    fn ControlService(service: *mut std::ffi::c_void, control: u32, status: *mut u8) -> i32;
    fn EnumDependentServicesW(
        service: *mut std::ffi::c_void,
        state: u32,
        buffer: *mut EnumServiceStatus,
        size: u32,
        needed: *mut u32,
        returned: *mut u32,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetTickCount64() -> u64;
    fn Sleep(milliseconds: u32);
    fn GetLastError() -> u32;
    fn GetProcessHeap() -> *mut std::ffi::c_void;
    fn HeapAlloc(heap: *mut std::ffi::c_void, flags: u32, size: usize) -> *mut std::ffi::c_void;
    fn HeapFree(heap: *mut std::ffi::c_void, flags: u32, memory: *mut std::ffi::c_void) -> i32;
}

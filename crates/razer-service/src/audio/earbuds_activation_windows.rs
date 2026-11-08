//! Current SimpleService elevation selection and local COM launch contract.
use anyhow::{Context as _, Result};
use std::{
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::atomic::{AtomicU32, Ordering},
};
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            CoCreateInstance, CoInitializeEx, CoUninitialize, DISPATCH_FLAGS, DISPPARAMS,
            EXCEPINFO, IConnectionPoint, IConnectionPointContainer, IDispatch, IDispatch_Impl,
            IDispatch_Vtbl, ITypeInfo,
        },
        Variant::VARIANT,
    },
    core::{BSTR, GUID, HRESULT, IUnknown, Interface, PCWSTR, implement, interface},
};

const CLASS: GUID = GUID::from_u128(0x0edeaf3c_d36e_4e7e_9467_900b977dc4ff);
const EVENTS: GUID = GUID::from_u128(0xc6237515_dd5f_4acc_b6f0_f45569adf965);

#[interface("bfe24d59-6568-4179-8ae5-d9d53869a3e3")]
unsafe trait Elevator: IDispatch {
    unsafe fn launch_process(
        &self,
        path: *const u16,
        parameters: *const u16,
        job: u32,
        exit: *mut i32,
    ) -> HRESULT;
}

#[interface("c6237515-dd5f-4acc-b6f0-f45569adf965")]
unsafe trait ElevatorEvents: IDispatch {}

#[implement(ElevatorEvents)]
struct Sink {
    events: AtomicU32,
}
impl ElevatorEvents_Impl for Sink_Impl {}
impl IDispatch_Impl for Sink_Impl {
    fn GetTypeInfoCount(&self) -> windows::core::Result<u32> {
        Ok(0)
    }
    fn GetTypeInfo(&self, _: u32, _: u32) -> windows::core::Result<ITypeInfo> {
        Err(HRESULT(0x80004001u32 as i32).into())
    }
    fn GetIDsOfNames(
        &self,
        _: *const GUID,
        _: *const PCWSTR,
        _: u32,
        _: u32,
        _: *mut i32,
    ) -> windows::core::Result<()> {
        Err(HRESULT(0x80020006u32 as i32).into())
    }
    fn Invoke(
        &self,
        member: i32,
        _: *const GUID,
        _: u32,
        _: DISPATCH_FLAGS,
        parameters: *const DISPPARAMS,
        _: *mut VARIANT,
        _: *mut EXCEPINFO,
        _: *mut u32,
    ) -> windows::core::Result<()> {
        if member != 1 {
            return Err(HRESULT(0x80020003u32 as i32).into());
        }
        // The server retains and supplies the dispatch arguments for this call.
        if parameters.is_null() || unsafe { (*parameters).cArgs } != 3 {
            return Err(HRESULT(0x8002000eu32 as i32).into());
        }
        self.events.fetch_add(1, Ordering::Relaxed);
        // The Sound caller consumes the method completion, not app-service broadcasts.
        Ok(())
    }
}

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
struct Subscription {
    point: IConnectionPoint,
    cookie: Option<u32>,
}
impl Subscription {
    fn close(&mut self) -> windows::core::Result<()> {
        if let Some(cookie) = self.cookie.take() {
            // Keep unregistration failure separate from the already completed launch.
            unsafe { self.point.Unadvise(cookie) }
        } else {
            Ok(())
        }
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        // COM retains the reference-counted sink until Unadvise/server release.
        let _ = self.close();
    }
}

pub(super) struct Completion {
    pub(super) exit_code: i32,
    pub(super) cleanup_error: Option<String>,
}

pub(super) fn launch(path: &str, arguments: &str) -> Result<Completion> {
    // This synchronous RPC has no source cancellation contract. Its background
    // owner waits for completion and releases everything on this same thread.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok() }?;
    let _apartment = Apartment;
    let client: Elevator = unsafe { CoCreateInstance(&CLASS, None, CLSCTX_LOCAL_SERVER) }?;
    let container: IConnectionPointContainer = client.cast()?;
    let point = unsafe { container.FindConnectionPoint(&EVENTS) }?;
    let sink: ElevatorEvents = Sink {
        events: AtomicU32::new(0),
    }
    .into();
    let unknown: IUnknown = sink.cast()?;
    let cookie = unsafe { point.Advise(&unknown) }?;
    let mut subscription = Subscription {
        point,
        cookie: Some(cookie),
    };
    static JOB: AtomicU32 = AtomicU32::new(0);
    let job = JOB.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    let path = BSTR::from(path);
    let arguments = BSTR::from(arguments);
    let mut exit = -1;
    // The current typelib specifies two input BSTRs, UI4 job and INT out at slot56.
    let result = unsafe {
        client
            .launch_process(path.as_ptr(), arguments.as_ptr(), job, &mut exit)
            .ok()
    };
    let cleanup_error = subscription.close().err().map(|error| error.to_string());
    result?;
    Ok(Completion {
        exit_code: exit,
        cleanup_error,
    })
}

fn manifest_requires_elevation(xml: &str) -> bool {
    roxmltree::Document::parse(xml)
        .ok()
        .is_some_and(|document| {
            document.descendants().any(|node| {
                node.is_element()
                    && node.tag_name().name() == "requestedExecutionLevel"
                    && matches!(
                        node.attribute("level"),
                        Some("requireAdministrator" | "highestAvailable")
                    )
            })
        })
}

pub(super) fn required(path: &Path) -> Result<bool> {
    use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
    use windows_sys::Win32::System::LibraryLoader::{
        FindResourceW, LOAD_LIBRARY_AS_DATAFILE, LoadLibraryExW, LoadResource, LockResource,
        SizeofResource,
    };
    struct Resource(HMODULE);
    impl Drop for Resource {
        fn drop(&mut self) {
            unsafe { FreeLibrary(self.0) };
        }
    }
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // DATAFILE never executes entry points or resolves executable imports.
    let module = unsafe {
        LoadLibraryExW(
            name.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_AS_DATAFILE,
        )
    };
    if module.is_null() {
        return Ok(false);
    }
    let module = Resource(module);
    let resource = unsafe { FindResourceW(module.0, 1usize as *const u16, 24usize as *const u16) };
    if resource.is_null() {
        return Ok(false);
    }
    let size = unsafe { SizeofResource(module.0, resource) } as usize;
    let loaded = unsafe { LoadResource(module.0, resource) };
    if loaded.is_null() || size == 0 {
        return Ok(false);
    }
    let data = unsafe { LockResource(loaded) };
    if data.is_null() {
        return Ok(false);
    }
    // Resource bytes stay valid while the datafile module is owned above.
    let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };
    let xml = std::str::from_utf8(bytes).context("Application manifest is not UTF-8")?;
    Ok(manifest_requires_elevation(
        xml.trim_start_matches('﻿').trim_end_matches('\0'),
    ))
}

#[cfg(test)]
mod tests {
    use super::manifest_requires_elevation;
    #[test]
    fn only_explicit_source_execution_levels_select_elevation() {
        for (level, expected) in [
            ("asInvoker", false),
            ("requireAdministrator", true),
            ("highestAvailable", true),
            ("unknown", false),
        ] {
            assert_eq!(
                manifest_requires_elevation(&format!(
                    "<assembly><requestedExecutionLevel level='{level}'/></assembly>"
                )),
                expected
            );
        }
        assert!(!manifest_requires_elevation("broken"));
        assert!(!manifest_requires_elevation("<assembly/>"));
    }
}

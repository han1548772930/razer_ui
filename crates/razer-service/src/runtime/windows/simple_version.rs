//! Local version query from simple_service RVA 0x28200 / 0x2dc9a.
//! The first three fields are source build constants; only Elevation is a
//! live COM observation. The service worker owns the apartment for this call.
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;
use std::{ffi::c_void, marker::PhantomData, ptr, rc::Rc};
use windows::core::{BSTR, HRESULT, IUnknown, IUnknown_Vtbl, Interface};
use windows_sys::{
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize},
    core::GUID,
};

const ELEVATION_CLASS: GUID = GUID::from_u128(0x0edeaf3c_d36e_4e7e_9467_900b977dc4ff);
const ELEVATION_INTERFACE: GUID = GUID::from_u128(0xbfe24d59_6568_4179_8ae5_d9d53869a3e3);

#[derive(Debug, Serialize)]
pub(super) struct Versions {
    #[serde(rename = "Simple Service Version")]
    simple: &'static str,
    #[serde(rename = "Apps Service Version")]
    apps: &'static str,
    #[serde(rename = "Audio Service Version")]
    audio: &'static str,
    #[serde(rename = "Elevation Service Version")]
    elevation: ElevationVersion,
}

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum ElevationVersion {
    Observed { value: String },
    Failed { error: String },
}

impl Versions {
    fn from_observation(elevation: Result<String>) -> Self {
        Self {
            simple: "1.2.2.10",
            apps: "1.2.2.4",
            audio: "1.2.1.16",
            elevation: match elevation {
                Ok(value) => ElevationVersion::Observed { value },
                Err(error) => ElevationVersion::Failed {
                    error: format!("{error:#}"),
                },
            },
        }
    }
}

struct Apartment(PhantomData<Rc<()>>);

impl Apartment {
    fn open() -> Result<Self> {
        // RVA 0x760c0 uses STA | DISABLE_OLE1DDE; successful S_FALSE also
        // acquires one initialization count, released by this thread's Drop.
        let hr = unsafe { CoInitializeEx(ptr::null(), 6) };
        ensure!(
            hr >= 0,
            "Elevation COM initialization failed: 0x{:08x}",
            hr as u32
        );
        Ok(Self(PhantomData))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // The interface and returned BSTR are released before this owner.
        unsafe { CoUninitialize() };
    }
}

#[repr(C)]
struct ElevationVtable {
    unknown: IUnknown_Vtbl,
    // Only the observed slot 8 is callable here. No signature is inferred
    // for the five preceding interface methods.
    preceding_methods: [*const c_void; 5],
    get_version: unsafe extern "system" fn(*mut c_void, *mut *const u16) -> HRESULT,
}

struct ElevationService(IUnknown);

impl ElevationService {
    /// `pointer` must be the owned output of CoCreateInstance for
    /// ELEVATION_INTERFACE, with the source-verified slot 8 ABI.
    unsafe fn from_output(hr: i32, pointer: *mut c_void) -> Result<Self> {
        // Adopt before checking HRESULT so a non-null failure output is also
        // released. Null outputs never enter IUnknown's non-null owner.
        let service = (!pointer.is_null()).then(|| Self(unsafe { IUnknown::from_raw(pointer) }));
        ensure!(
            hr >= 0,
            "Failed to connect to Elevation Service: 0x{:08x}",
            hr as u32
        );
        service.context("Elevation Service returned a null interface")
    }

    fn version(&self) -> Result<String> {
        // CoCreateInstance requested the exact IID above. Its full captured
        // caller invokes vtable +64 on x64 with HRESULT(this, BSTR*).
        let vtable = unsafe { &**(self.0.as_raw() as *const *const ElevationVtable) };
        let mut raw = ptr::null();
        let hr = unsafe { (vtable.get_version)(self.0.as_raw(), &mut raw) };
        // BSTR owns SysFreeString on every outcome, before interface Release.
        let version = unsafe { BSTR::from_raw(raw) };
        ensure!(
            hr.is_ok(),
            "Failed to get Elevation Service version: 0x{:08x}",
            hr.0 as u32
        );
        ensure!(!raw.is_null(), "Elevation Service returned a null version");
        // The original builds a NUL-terminated wstring. Bound that scan to
        // the BSTR allocation length, preserving its embedded-NUL behavior.
        let end = version
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(version.len());
        let version = String::from_utf16(&version[..end])
            .context("Elevation Service returned invalid UTF-16")?;
        ensure!(
            !version.is_empty(),
            "Elevation Service returned an empty version"
        );
        Ok(version)
    }
}

pub(super) fn query() -> Result<Versions> {
    // Source-build metadata remains available even when the independently
    // observed service is absent; failure is never an empty observed version.
    Ok(Versions::from_observation(query_elevation()))
}

fn query_elevation() -> Result<String> {
    let _apartment = Apartment::open()?;
    let mut pointer = ptr::null_mut();
    // RVA 0x2dc9a uses CLSCTX_LOCAL_SERVER=4, with no elevation moniker.
    let hr = unsafe {
        CoCreateInstance(
            &ELEVATION_CLASS,
            ptr::null_mut(),
            4,
            &ELEVATION_INTERFACE,
            &mut pointer,
        )
    };
    let service = unsafe { ElevationService::from_output(hr, pointer) }?;
    service.version()
}

#[cfg(test)]
#[path = "simple_version_tests.rs"]
mod tests;

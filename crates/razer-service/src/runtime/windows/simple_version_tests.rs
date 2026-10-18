use super::*;
use std::cell::Cell;

#[derive(Default, Clone)]
struct Calls {
    reads: Rc<Cell<u32>>,
    releases: Rc<Cell<u32>>,
}

#[repr(C)]
struct FakeService {
    vtable: *const ElevationVtable,
    table_owner: Box<ElevationVtable>,
    calls: Calls,
    outcome: HRESULT,
    version: Option<Vec<u16>>,
}

unsafe extern "system" fn query_interface(
    _: *mut c_void,
    _: *const windows::core::GUID,
    output: *mut *mut c_void,
) -> HRESULT {
    unsafe { *output = ptr::null_mut() };
    HRESULT(0x80004002u32 as i32)
}

unsafe extern "system" fn add_ref(_: *mut c_void) -> u32 {
    panic!("the version query must not clone its COM owner")
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let service = unsafe { Box::from_raw(this.cast::<FakeService>()) };
    service.calls.releases.set(service.calls.releases.get() + 1);
    0
}

unsafe extern "system" fn get_version(this: *mut c_void, output: *mut *const u16) -> HRESULT {
    let service = unsafe { &*this.cast::<FakeService>() };
    service.calls.reads.set(service.calls.reads.get() + 1);
    unsafe {
        *output = service
            .version
            .as_ref()
            .map(|text| BSTR::from_wide(text).into_raw())
            .unwrap_or(ptr::null());
    }
    service.outcome
}

fn create(
    creation_result: i32,
    version_result: i32,
    version: Option<Vec<u16>>,
) -> (Result<ElevationService>, Calls) {
    let table_owner = Box::new(ElevationVtable {
        unknown: IUnknown_Vtbl {
            QueryInterface: query_interface,
            AddRef: add_ref,
            Release: release,
        },
        preceding_methods: [ptr::null(); 5],
        get_version,
    });
    let calls = Calls::default();
    let service = Box::new(FakeService {
        vtable: &*table_owner,
        table_owner,
        calls: calls.clone(),
        outcome: HRESULT(version_result),
        version,
    });
    (
        unsafe { ElevationService::from_output(creation_result, Box::into_raw(service).cast()) },
        calls,
    )
}

#[test]
fn observed_version_uses_source_slot_and_live_com_output() {
    assert_eq!(
        std::mem::offset_of!(ElevationVtable, get_version),
        8 * std::mem::size_of::<*const c_void>()
    );
    let (service, calls) = create(0, 1, Some("9.8.7.6".encode_utf16().collect()));
    let service = service.unwrap();
    let report = Versions::from_observation(service.version());
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        serde_json::json!({
            "Simple Service Version": "1.2.2.10",
            "Apps Service Version": "1.2.2.4",
            "Audio Service Version": "1.2.1.16",
            "Elevation Service Version": {"state":"observed", "value":"9.8.7.6"}
        })
    );
    assert_eq!(calls.reads.get(), 1);
    assert_eq!(calls.releases.get(), 0);
    drop(service);
    assert_eq!(calls.releases.get(), 1);
}

#[test]
fn failed_creation_releases_nonnull_output_without_calling_it() {
    let (result, calls) = create(0x80004005u32 as i32, 0, None);
    assert!(result.err().unwrap().to_string().contains("80004005"));
    assert_eq!(calls.reads.get(), 0);
    assert_eq!(calls.releases.get(), 1);
    let result = unsafe { ElevationService::from_output(0, ptr::null_mut()) };
    assert!(result.err().unwrap().to_string().contains("null interface"));
}

#[test]
fn failure_null_empty_and_invalid_versions_never_become_success() {
    for (hr, text, expected) in [
        (0x80004005u32 as i32, Some(vec![b'1' as u16]), "80004005"),
        (0, None, "null version"),
        (0, Some(vec![0]), "empty version"),
        (0, Some(vec![0xd800]), "invalid UTF-16"),
    ] {
        let (service, calls) = create(0, hr, text);
        let service = service.unwrap();
        assert!(
            service
                .version()
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
        drop(service);
        assert_eq!(calls.reads.get(), 1);
        assert_eq!(calls.releases.get(), 1);
    }
}

#[test]
fn bstr_conversion_preserves_source_nul_termination() {
    let (service, calls) = create(0, 0, Some(vec![b'1' as u16, 0, b'2' as u16]));
    let service = service.unwrap();
    assert_eq!(service.version().unwrap(), "1");
    drop(service);
    assert_eq!(calls.releases.get(), 1);
}

#[test]
fn failed_live_service_keeps_source_build_metadata_and_explicit_failure() {
    let report = Versions::from_observation(Err(anyhow::anyhow!("COM unavailable")));
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        serde_json::json!({
            "Simple Service Version":"1.2.2.10",
            "Apps Service Version":"1.2.2.4",
            "Audio Service Version":"1.2.1.16",
            "Elevation Service Version":{"state":"failed", "error":"COM unavailable"}
        })
    );
}

#[test]
#[ignore = "requires an audited installed local COM service; use tools/workspace.py"]
fn connected_service_version_read_only() {
    use super::super::{Initialize, NativeRuntime, Query, callback0, callback3};
    use crate::dll::EngineLibrary;
    use std::mem::ManuallyDrop;

    let path = std::env::var_os("RAZER_VERIFIED_SIMPLE_SERVICE")
        .expect("the maintained verifier must supply the audited source library");
    let report = query().unwrap();
    let mut original = NativeRuntime::new();
    // This original library is used only by this opt-in comparison test. Its
    // process retains the module through shutdown and callback completion.
    original.simple = Some(ManuallyDrop::new(
        EngineLibrary::load(std::path::Path::new(&path)).unwrap(),
    ));
    let initialize = original.simple();
    let source = initialize.and_then(|()| {
        let call: Query = unsafe { original.simple_symbol("simpleGetVersionInfo")? };
        let reply = original.call_source(|| unsafe { call(callback3) })?;
        Ok(serde_json::json!({
            "success":reply.success, "reason":reply.reason,
            "value":reply.value.as_deref().map(serde_json::from_str::<serde_json::Value>).transpose()?
        }))
    });
    let cleanup = if original.simple_initialized && !original.poisoned {
        (|| -> Result<()> {
            let shutdown: Initialize = unsafe { original.simple_symbol("simpleServiceShutdown")? };
            original.call(|| unsafe { shutdown(callback0) })?;
            original.simple_initialized = false;
            Ok(())
        })()
    } else {
        Err(anyhow::anyhow!(
            "original lifecycle did not reach a callable initialized state"
        ))
    };
    let source = source.map_err(|error| format!("{error:#}"));
    let cleanup = cleanup.map_err(|error| format!("{error:#}"));
    let receipt = serde_json::json!({"rust":report, "original":source, "cleanup":cleanup});
    println!("RAZER_VERSION_RESULT {receipt}");
    assert!(cleanup.is_ok(), "original shutdown must complete");
    let source = source.unwrap();
    assert_eq!(source["success"], true);
    let ElevationVersion::Observed { value } = &report.elevation else {
        panic!("live COM observation failed");
    };
    assert_eq!(source["value"]["Elevation Service Version"], *value);
    assert_eq!(source["value"]["Simple Service Version"], report.simple);
    assert_eq!(source["value"]["Apps Service Version"], report.apps);
    assert_eq!(source["value"]["Audio Service Version"], report.audio);
}

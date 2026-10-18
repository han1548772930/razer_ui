//! Worker-only no-argument Blade report queries and their native cleanup.
use anyhow::{Context as _, ensure};
use razer_device::system_info::{Data, Query, Response, layout};

trait Calls {
    fn read(&mut self, query: Query) -> anyhow::Result<Vec<u8>>;
    fn terminate(&mut self) -> anyhow::Result<()>;
    fn terminate_when_exit(&mut self) -> anyhow::Result<()>;
}

fn guarded<T>(stage: &str, operation: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    // Rust unwinds still pass through both release stages. This does not catch
    // native access violations, aborts or process termination.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).map_err(|payload| {
        let detail = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic");
        anyhow::anyhow!("Native system-info {stage} panicked: {detail}")
    })?
}

fn query_with(calls: &mut impl Calls, product_id: u32, query: Query) -> anyhow::Result<Response> {
    let result = guarded("read", || {
        calls
            .read(query)
            .and_then(|bytes| Data::decode(query, &bytes))
    });
    // In all four current binaries Terminate only logs. The host's registered
    // TerminateWhenExit performs the separate window/color-library cleanup.
    let mut errors = Vec::new();
    if let Err(error) = guarded("Terminate", || calls.terminate()) {
        errors.push(format!("Terminate: {error:#}"));
    }
    if let Err(error) = guarded("TerminateWhenExit", || calls.terminate_when_exit()) {
        errors.push(format!("TerminateWhenExit: {error:#}"));
    }
    match (result, errors.is_empty()) {
        (Ok(data), true) => Ok(Response { product_id, data }),
        (Err(error), true) => Err(error),
        (Ok(_), false) => anyhow::bail!("Native system-info cleanup failed: {}", errors.join("; ")),
        (Err(error), false) => {
            Err(error.context(format!("Native cleanup also failed: {}", errors.join("; "))))
        }
    }
}

pub fn query(product_id: u32, query: Query) -> anyhow::Result<Response> {
    ensure!(
        layout(product_id).is_some(),
        "Product has no current Blade report caller"
    );
    platform::query(product_id, query)
}

#[cfg(windows)]
mod platform {
    use super::*;
    use crate::dll::EngineLibrary;
    use razer_catalog::native_library;
    use std::{
        ffi::{CStr, c_char, c_void},
        mem::ManuallyDrop,
    };

    type GetText = unsafe extern "C" fn() -> *mut c_char;
    type Release = unsafe extern "C" fn(*mut c_void);
    type Terminate = unsafe extern "C" fn() -> bool;

    struct NativeText {
        raw: *mut c_char,
        release: Release,
    }
    impl Drop for NativeText {
        fn drop(&mut self) {
            // The current ffiMain.readCString consumer pairs every non-null
            // pointer from these getters with the same module's FreeMalloc.
            unsafe { (self.release)(self.raw.cast()) };
        }
    }

    struct NativeCalls {
        _library: ManuallyDrop<EngineLibrary>,
        get: GetText,
        release: Release,
        terminate: Terminate,
        exit: Terminate,
    }
    impl Calls for NativeCalls {
        fn read(&mut self, _: Query) -> anyhow::Result<Vec<u8>> {
            // No Initialize/notification registration is part of either
            // no-argument getter. Their buffers are allocated, NUL-terminated
            // strings in the current native producers, not borrowed storage.
            read_text(self.get, self.release)
        }
        fn terminate(&mut self) -> anyhow::Result<()> {
            ensure!(
                unsafe { (self.terminate)() },
                "Native Terminate returned false"
            );
            Ok(())
        }
        fn terminate_when_exit(&mut self) -> anyhow::Result<()> {
            ensure!(
                unsafe { (self.exit)() },
                "Native TerminateWhenExit returned false"
            );
            Ok(())
        }
    }
    fn read_text(get: GetText, release: Release) -> anyhow::Result<Vec<u8>> {
        let raw = unsafe { get() };
        ensure!(
            !raw.is_null(),
            "Native system-info getter returned a null pointer"
        );
        let text = NativeText { raw, release };
        let bytes = unsafe { CStr::from_ptr(text.raw) }.to_bytes();
        ensure!(
            bytes.len() <= razer_device::system_info::MAXIMUM_REPORT_BYTES,
            "Native system report exceeds its size limit"
        );
        std::str::from_utf8(bytes).context("Native system report is not UTF-8")?;
        Ok(bytes.to_vec())
    }

    pub(super) fn query(product_id: u32, query: Query) -> anyhow::Result<Response> {
        let library = native_library::find("blade2Native")
            .context("Current Blade native resource is missing")?;
        ensure!(
            library.products.contains(&product_id),
            "Native resource does not own this product"
        );
        for (name, returns, args) in [
            (query.export(), "pointer", &[][..]),
            ("FreeMalloc", "void", &["pointer"][..]),
            ("Terminate", "bool", &[][..]),
            ("TerminateWhenExit", "bool", &[][..]),
        ] {
            let function = library
                .function(name)
                .with_context(|| format!("Current Blade source omits {name}"))?;
            ensure!(
                function.returns == returns
                    && function
                        .args
                        .iter()
                        .map(String::as_str)
                        .eq(args.iter().copied()),
                "Blade {name} ABI differs from the current caller"
            );
        }
        let (resource, path) =
            razer_platform::native_paths::resource_candidates(library, Some(product_id))
                .into_iter()
                .find(|(_, path)| path.is_file())
                .context("Installed local Blade native library was not found")?;
        ensure!(
            resource.machine == 0x8664 && size_of::<usize>() == 8,
            "Blade report adapter requires the current AMD64 ABI"
        );
        for name in [
            query.export(),
            "FreeMalloc",
            "Terminate",
            "TerminateWhenExit",
        ] {
            ensure!(
                resource.exports.iter().any(|export| export == name),
                "Current Blade resource evidence omits {name}"
            );
        }
        // Source hashes identify analysis inputs, not runtime compatibility.
        // Actual installed symbols and the loader's architecture checks apply.
        // Keep the native module mapped until the exclusive worker exits;
        // TerminateWhenExit does not prove that every native thread has joined.
        let loaded = ManuallyDrop::new(EngineLibrary::load(&path)?);
        let mut calls = NativeCalls {
            get: unsafe { loaded.func(query.export()) }
                .context("Installed Blade getter is missing")?,
            release: unsafe { loaded.func("FreeMalloc") }
                .context("Installed Blade allocator release is missing")?,
            terminate: unsafe { loaded.func("Terminate") }
                .context("Installed Blade Terminate is missing")?,
            exit: unsafe { loaded.func("TerminateWhenExit") }
                .context("Installed Blade exit cleanup is missing")?,
            _library: loaded,
        };
        query_with(&mut calls, product_id, query)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{
            ffi::CString,
            sync::atomic::{AtomicUsize, Ordering},
        };
        static RELEASED: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn invalid_utf8() -> *mut c_char {
            CString::new(vec![0xff]).unwrap().into_raw()
        }
        unsafe extern "C" fn null() -> *mut c_char {
            std::ptr::null_mut()
        }
        unsafe extern "C" fn valid() -> *mut c_char {
            CString::new(r#"{"Display":null}"#).unwrap().into_raw()
        }
        unsafe extern "C" fn release(raw: *mut c_void) {
            drop(unsafe { CString::from_raw(raw.cast()) });
            RELEASED.fetch_add(1, Ordering::AcqRel);
        }
        #[test]
        fn help_system_info_native_pointer_is_released_on_utf8_failure_and_success() {
            assert!(read_text(invalid_utf8, release).is_err());
            assert_eq!(RELEASED.load(Ordering::Acquire), 1);
            assert_eq!(read_text(valid, release).unwrap(), br#"{"Display":null}"#);
            assert_eq!(RELEASED.load(Ordering::Acquire), 2);
            assert!(read_text(null, release).is_err());
            assert_eq!(RELEASED.load(Ordering::Acquire), 2);
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub(super) fn query(_: u32, _: Query) -> anyhow::Result<Response> {
        anyhow::bail!("Local Blade native system reports require the Windows adapter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        bytes: Vec<u8>,
        events: Vec<&'static str>,
        fail_read: bool,
        fail_cleanup: bool,
    }
    impl Calls for Fake {
        fn read(&mut self, _: Query) -> anyhow::Result<Vec<u8>> {
            self.events.push("read");
            ensure!(!self.fail_read, "native read failed");
            Ok(self.bytes.clone())
        }
        fn terminate(&mut self) -> anyhow::Result<()> {
            self.events.push("terminate");
            ensure!(!self.fail_cleanup, "native session cleanup failed");
            Ok(())
        }
        fn terminate_when_exit(&mut self) -> anyhow::Result<()> {
            self.events.push("exit");
            ensure!(!self.fail_cleanup, "native process cleanup failed");
            Ok(())
        }
    }
    #[test]
    fn help_system_info_keeps_actual_report_and_cleans_every_result_branch() {
        for (bytes, fail_read, succeeds) in [
            (
                br#"{"Display":null,"CPU":"Real processor"}"#.to_vec(),
                false,
                true,
            ),
            (b"broken".to_vec(), false, false),
            (Vec::new(), true, false),
        ] {
            let mut fake = Fake {
                bytes,
                events: Vec::new(),
                fail_read,
                fail_cleanup: false,
            };
            let result = query_with(&mut fake, 737, Query::Display);
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(fake.events, ["read", "terminate", "exit"]);
            if let Ok(response) = result {
                let Data::Display(details) = response.data else {
                    panic!()
                };
                assert_eq!(details.cpu.as_deref(), Some("Real processor"));
            }
        }
    }
    #[test]
    fn help_system_info_retains_query_and_both_cleanup_errors() {
        let mut fake = Fake {
            bytes: Vec::new(),
            events: Vec::new(),
            fail_read: true,
            fail_cleanup: true,
        };
        let error = format!("{:#}", query_with(&mut fake, 737, Query::Full).unwrap_err());
        for expected in [
            "native read failed",
            "native session cleanup failed",
            "native process cleanup failed",
        ] {
            assert!(error.contains(expected));
        }
        assert_eq!(fake.events, ["read", "terminate", "exit"]);
    }
    #[test]
    fn help_system_info_rust_unwind_does_not_skip_either_cleanup_stage() {
        struct Panics(Vec<&'static str>);
        impl Calls for Panics {
            fn read(&mut self, _: Query) -> anyhow::Result<Vec<u8>> {
                self.0.push("read");
                panic!("read panic")
            }
            fn terminate(&mut self) -> anyhow::Result<()> {
                self.0.push("terminate");
                panic!("terminate panic")
            }
            fn terminate_when_exit(&mut self) -> anyhow::Result<()> {
                self.0.push("exit");
                panic!("exit panic")
            }
        }
        let mut calls = Panics(Vec::new());
        let error = format!(
            "{:#}",
            query_with(&mut calls, 737, Query::Display).unwrap_err()
        );
        assert_eq!(calls.0, ["read", "terminate", "exit"]);
        for expected in ["read panic", "terminate panic", "exit panic"] {
            assert!(error.contains(expected));
        }
    }
}

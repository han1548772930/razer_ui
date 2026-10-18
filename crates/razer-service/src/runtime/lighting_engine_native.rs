//! Current RzLightingEngineApi JSON ABI, used only on the retained lighting
//! worker. Returned allocations are released by their originating DLL.
use anyhow::{Context as _, ensure};
use razer_device::lighting_engine::{EngineEffect, EngineFrame, LedConfiguration};
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString, c_char, c_void};

type JsonApi = unsafe extern "C" fn(*const c_char) -> *mut c_char;
type JsonApiNoReturn = unsafe extern "C" fn(*const c_char);
type ReadString = unsafe extern "C" fn() -> *mut c_char;
type Free = unsafe extern "C" fn(*mut c_void);
type OperatingMode = unsafe extern "C" fn(i32);

pub(in crate::runtime) struct Engine {
    api: JsonApi,
    no_return: JsonApiNoReturn,
    poll: ReadString,
    free: Free,
    _library: crate::dll::EngineLibrary,
}

#[derive(Serialize)]
struct Request<T: Serialize> {
    #[serde(rename = "Action")]
    action: u32,
    #[serde(flatten)]
    payload: T,
}

#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "Status")]
    status: i64,
    device_handle: Option<u32>,
    engine_handle: Option<u32>,
    effect_handle: Option<u32>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct Event {
    #[serde(rename = "eventId")]
    event_id: u32,
    device_handle: Option<u32>,
    #[serde(rename = "regionId")]
    region: Option<u8>,
    #[serde(rename = "numRow")]
    rows: Option<u8>,
    #[serde(rename = "numCol")]
    columns: Option<u8>,
    frame: Option<Vec<u8>>,
}

impl Engine {
    pub(in crate::runtime) fn open() -> anyhow::Result<Self> {
        let candidates =
            razer_platform::native_paths::native_library_candidates("RzLightingEngineApi", None)
                .context("Current lighting-engine installation binding is absent")?;
        let files = candidates
            .into_iter()
            .map(|(_, path)| path)
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        ensure!(
            files.len() == 1,
            "Current installed lighting-engine DLL is absent or ambiguous"
        );
        let library = crate::dll::EngineLibrary::load(&files[0])?;
        // SAFETY: current 2198 FFI table and native 0x33490/0x33b60/0x32d90/
        // 0x340c0/0x32d00 exports establish these x64 C signatures. The library
        // remains owned by this same worker through all calls and cleanup.
        let (api, no_return, poll, free, operating_mode, version): (
            JsonApi,
            JsonApiNoReturn,
            ReadString,
            Free,
            OperatingMode,
            ReadString,
        ) = unsafe {
            (
                library
                    .func("RzLightingApi")
                    .context("Missing RzLightingApi")?,
                library
                    .func("RzLightingApiNoReturn")
                    .context("Missing RzLightingApiNoReturn")?,
                library.func("PollEvents").context("Missing PollEvents")?,
                library.func("FreeMalloc").context("Missing FreeMalloc")?,
                library
                    .func("SetOperatingMode")
                    .context("Missing SetOperatingMode")?,
                library
                    .func("GetDLLVersion")
                    .context("Missing GetDLLVersion")?,
            )
        };
        // SAFETY: owned library exports have the verified signatures above.
        let observed_version = unsafe { owned_string(version(), free)? };
        ensure!(
            observed_version == "4.0.55.0",
            "Installed lighting-engine ABI version has no current static capture: {observed_version}"
        );
        // Poll mode is the native/source 4.0.3 fallback. It keeps frame delivery
        // on this worker and avoids callbacks outliving the worker/DLL.
        unsafe { operating_mode(1) };
        Ok(Self {
            api,
            no_return,
            poll,
            free,
            _library: library,
        })
    }

    fn request<T: Serialize>(&self, action: u32, payload: T) -> anyhow::Result<Reply> {
        let text = CString::new(serde_json::to_vec(&Request { action, payload })?)?;
        // SAFETY: request is a live NUL-terminated UTF-8 string. The export
        // returns an owned NUL-terminated allocation freed even on parse error.
        let reply = unsafe { owned_string((self.api)(text.as_ptr()), self.free)? };
        let reply: Reply =
            serde_json::from_str(&reply).context("Malformed native lighting reply")?;
        ensure!(
            reply.status == 0,
            "Native lighting action {action} failed ({:#x}): {}",
            reply.status,
            reply.error.as_deref().unwrap_or("No native error text")
        );
        Ok(reply)
    }

    pub(in crate::runtime) fn create_device(
        &mut self,
        config: &LedConfiguration,
    ) -> anyhow::Result<u32> {
        #[derive(Serialize)]
        struct Payload<'a> {
            config: &'a LedConfiguration,
        }
        self.request(1, Payload { config })?
            .device_handle
            .context("Native create-device reply omitted its handle")
    }
    pub(in crate::runtime) fn create_engine(&mut self) -> anyhow::Result<u32> {
        #[derive(Serialize)]
        struct Payload {
            fps: u8,
            r#type: &'static str,
        }
        self.request(
            3,
            Payload {
                fps: 25,
                r#type: "Basic",
            },
        )?
        .engine_handle
        .context("Native create-engine reply omitted its handle")
    }
    pub(in crate::runtime) fn add_device(
        &mut self,
        engine_handle: u32,
        device_handle: u32,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Payload {
            engine_handle: u32,
            device_handle: u32,
            region: u8,
            orientation: u8,
        }
        self.request(
            17,
            Payload {
                engine_handle,
                device_handle,
                region: 0,
                orientation: 0,
            },
        )?;
        Ok(())
    }
    pub(in crate::runtime) fn add_effect(
        &mut self,
        engine_handle: u32,
        effect: &EngineEffect,
    ) -> anyhow::Result<u32> {
        #[derive(Serialize)]
        struct Payload<'a> {
            engine_handle: u32,
            #[serde(flatten)]
            effect: &'a EngineEffect,
        }
        self.request(
            33,
            Payload {
                engine_handle,
                effect,
            },
        )?
        .effect_handle
        .context("Native add-effect reply omitted its handle")
    }
    pub(in crate::runtime) fn remove_effect(
        &mut self,
        engine_handle: u32,
        effect_handle: u32,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Payload {
            engine_handle: u32,
            effect_handle: u32,
        }
        self.request(
            34,
            Payload {
                engine_handle,
                effect_handle,
            },
        )?;
        Ok(())
    }
    pub(in crate::runtime) fn enable(
        &mut self,
        engine_handle: u32,
        enabled: bool,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Payload {
            engine_handle: u32,
            enable: u8,
            device_handle: u32,
            region: u8,
            #[serde(rename = "clearFrame")]
            clear_frame: u8,
        }
        self.request(
            49,
            Payload {
                engine_handle,
                enable: u8::from(enabled),
                device_handle: 0,
                region: 0,
                clear_frame: 1,
            },
        )?;
        Ok(())
    }
    pub(in crate::runtime) fn input(&mut self, device_handle: u32) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Input {
            data: u8,
            #[serde(rename = "extraInformation")]
            extra_information: u8,
            id: u8,
            modifiers: u8,
            r#type: &'static str,
        }
        #[derive(Serialize)]
        struct Payload {
            device_handle: u32,
            #[serde(rename = "InputData")]
            input: Input,
        }
        let request = Request {
            action: 61445,
            payload: Payload {
                device_handle,
                input: Input {
                    data: 0,
                    extra_information: 0,
                    id: 1,
                    modifiers: 0,
                    r#type: "mouse",
                },
            },
        };
        let text = CString::new(serde_json::to_vec(&request)?)?;
        // SAFETY: same verified char* ABI as RzLightingApi; source 23059 uses
        // this no-return export, so there is no invented native/device ACK.
        unsafe { (self.no_return)(text.as_ptr()) };
        Ok(())
    }
    pub(in crate::runtime) fn frames(&mut self) -> anyhow::Result<Vec<EngineFrame>> {
        // SAFETY: PollEvents owns the returned allocation; FreeMalloc is paired.
        let text = unsafe { owned_string((self.poll)(), self.free)? };
        let events: Vec<Event> =
            serde_json::from_str(&text).context("Malformed native lighting event batch")?;
        ensure!(
            events.len() <= 1024,
            "Native lighting batch exceeds its worker bound"
        );
        events
            .into_iter()
            .filter(|event| event.event_id == 2)
            .map(|event| {
                Ok(EngineFrame {
                    device_handle: event
                        .device_handle
                        .context("Native render frame omitted device_handle")?,
                    region_id: event
                        .region
                        .context("Native render frame omitted regionId")?,
                    num_row: event.rows.context("Native render frame omitted numRow")?,
                    num_col: event
                        .columns
                        .context("Native render frame omitted numCol")?,
                    frame: event
                        .frame
                        .context("Native render frame omitted RGB data")?,
                })
            })
            .collect()
    }
    pub(in crate::runtime) fn destroy_engine(&mut self, engine_handle: u32) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Payload {
            engine_handle: u32,
        }
        self.request(4, Payload { engine_handle })?;
        Ok(())
    }
    pub(in crate::runtime) fn destroy_device(&mut self, device_handle: u32) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Payload {
            device_handle: u32,
        }
        self.request(2, Payload { device_handle })?;
        Ok(())
    }
    pub(in crate::runtime) fn terminate(&mut self) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Payload {}
        self.request(65520, Payload {})?;
        Ok(())
    }
}

unsafe fn owned_string(pointer: *mut c_char, free: Free) -> anyhow::Result<String> {
    ensure!(
        !pointer.is_null(),
        "Native lighting returned a null allocation"
    );
    struct Allocation {
        pointer: *mut c_char,
        free: Free,
    }
    impl Drop for Allocation {
        fn drop(&mut self) {
            unsafe { (self.free)(self.pointer.cast()) };
        }
    }
    let allocation = Allocation { pointer, free };
    // SAFETY: the verified exports return NUL-terminated strings. Keep the
    // allocation alive through copying and release using the same library.
    let bytes = unsafe { CStr::from_ptr(allocation.pointer) }.to_bytes();
    ensure!(
        bytes.len() <= 1024 * 1024,
        "Native lighting response exceeds its worker bound"
    );
    Ok(std::str::from_utf8(bytes)?.to_owned())
}

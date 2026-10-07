//! Generic opaque-handle access to the pinned official HID.node C exports.
//! No product IDs, protocol commands, receiver mappings or N-API calls live here.
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::{
    ffi::{CString, c_char, c_int, c_void},
    fs::OpenOptions,
    io::Write as _,
    path::PathBuf,
    sync::OnceLock,
};
const NATIVE_BYTES: &[u8] = include_bytes!("../../assets/native/razer-hid-0.0.31.node");
const NATIVE_SHA256: &str = "f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f";

// AMD64 disassembly of the pinned original exports establishes RCX opaque
// hid_device*, RDX report*, R8 size_t; EAX is signed int. open_path consumes
// RCX char*, returns an opaque pointer; close consumes that pointer only.
// See receiver-native-hid-current-evidence.json, not just their export names.
type OpenPath = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type SendFeature = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> c_int;
type GetFeature = unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> c_int;
type Close = unsafe extern "C" fn(*mut c_void);

struct NativeApi {
    _library: libloading::Library,
    path: PathBuf,
    open: OpenPath,
    send: SendFeature,
    get: GetFeature,
    close: Close,
}

fn native_api() -> anyhow::Result<&'static NativeApi> {
    static API: OnceLock<Result<NativeApi, String>> = OnceLock::new();
    API.get_or_init(|| load_native().map_err(|error| format!("{error:#}")))
        .as_ref()
        .map_err(|error| anyhow::anyhow!(error.clone()))
}

fn load_native() -> anyhow::Result<NativeApi> {
    ensure!(
        cfg!(target_arch = "x86_64"),
        "当前官方 HID 原生模块仅核验了 AMD64 ABI"
    );
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("缺少 LOCALAPPDATA，无法准备原生 HID 模块"))?;
    let directory = root.join("RazerUi").join("native").join(NATIVE_SHA256);
    std::fs::create_dir_all(&directory).context("无法准备原生 HID 模块目录")?;
    let path = directory.join("HID.node");
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(NATIVE_BYTES)
                .context("无法准备原生 HID 模块文件")?;
            file.sync_all().context("无法保存原生 HID 模块文件")?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("无法创建原生 HID 模块文件"),
    }
    ensure!(
        std::fs::read(&path)?.as_slice() == NATIVE_BYTES,
        "原生 HID 模块与已核验的官方文件不一致"
    );
    // Loading executes the original DLL entry point; this function is only
    // reachable inside the isolated worker. The parent kills a stuck worker.
    // C HID exports do not invoke N-API registration or node.exe delay imports.
    let library: libloading::Library =
        unsafe { libloading::os::windows::Library::load_with_flags(&path, 0x100 | 0x800) }
            .context("无法加载当前官方 HID 原生模块")?
            .into();
    // SAFETY: exact pinned AMD64 binary, verified C-export ABI, retained library.
    let (open, send, get, close) = unsafe {
        (
            *library.get::<OpenPath>(b"hid_open_path\0")?,
            *library.get::<SendFeature>(b"hid_send_feature_report\0")?,
            *library.get::<GetFeature>(b"hid_get_feature_report\0")?,
            *library.get::<Close>(b"hid_close\0")?,
        )
    };
    Ok(NativeApi {
        _library: library,
        path,
        open,
        send,
        get,
        close,
    })
}

pub(super) struct Device<'a> {
    raw: *mut c_void,
    api: &'a NativeApi,
}
impl Drop for Device<'_> {
    fn drop(&mut self) {
        // SAFETY: non-null handle returned by this exact library, closed once.
        unsafe { (self.api.close)(self.raw) };
    }
}

pub(super) fn open(path: &str) -> anyhow::Result<Device<'static>> {
    let api = native_api()?;
    let c_path = CString::new(path)?;
    // SAFETY: the worker validated this current SetupAPI path before calling;
    // a NUL-terminated string is passed to the verified C ABI.
    let raw = unsafe { (api.open)(c_path.as_ptr()) };
    ensure!(!raw.is_null(), "原生 HID 无法打开设备接口");
    Ok(Device { raw, api })
}

impl Device<'_> {
    pub(super) fn send_feature(&self, report: &[u8]) -> anyhow::Result<usize> {
        // SAFETY: live opaque handle, valid immutable buffer and exact length.
        let count = unsafe { (self.api.send)(self.raw, report.as_ptr(), report.len()) };
        ensure!(count >= 0, "原生 HID 发送 Feature 查询失败（返回 {count}）");
        Ok(count as usize)
    }

    pub(super) fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        // SAFETY: live opaque handle, exclusive mutable buffer and exact length.
        // The pinned export returns actual transferred bytes plus ReportID.
        let count = unsafe { (self.api.get)(self.raw, report.as_mut_ptr(), report.len()) };
        ensure!(count >= 0, "原生 HID 读取 Feature 响应失败（返回 {count}）");
        Ok(count as usize)
    }

    pub(super) fn metadata(&self) -> Value {
        json!({"transport":"node-rz-hid", "native_version":"0.0.31",
            "native_library":self.api.path, "native_sha256":NATIVE_SHA256})
    }
}

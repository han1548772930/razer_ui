//! Generic opaque-handle access to the pinned official HID.node C exports.
//! No product IDs, protocol commands, receiver mappings or N-API calls live here.
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
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
type Enumerate = unsafe extern "C" fn(u16, u16) -> *mut HidDeviceInfo;
type FreeEnumeration = unsafe extern "C" fn(*mut HidDeviceInfo);

// The pinned AMD64 node allocates 0x50-byte records. Fields used here are
// established by hid_enumerate's stores and hid_free_enumeration's loads.
#[repr(C)]
struct HidDeviceInfo {
    path: *mut c_char,
    vendor_id: u16,
    product_id: u16,
    serial_number: *mut u16,
    release_number: u16,
    _padding: [u8; 6],
    manufacturer_string: *mut u16,
    product_string: *mut u16,
    _owned_string_0: *mut u16,
    _owned_string_1: *mut u16,
    usage_page: u16,
    usage: u16,
    interface_number: i32,
    next: *mut HidDeviceInfo,
}

pub(super) struct EnumeratedDevice {
    pub(super) path: String,
    pub(super) vendor_id: u16,
    pub(super) product_id: u16,
    pub(super) interface_number: i32,
    pub(super) serial_number: Option<String>,
    pub(super) manufacturer: Option<String>,
    pub(super) product: Option<String>,
    pub(super) usage_page: u16,
    pub(super) usage: u16,
}

struct NativeApi {
    _library: libloading::Library,
    path: PathBuf,
    open: OpenPath,
    send: SendFeature,
    get: GetFeature,
    close: Close,
    enumerate: Enumerate,
    free_enumeration: FreeEnumeration,
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
    let (open, send, get, close, enumerate, free_enumeration) = unsafe {
        (
            *library.get::<OpenPath>(b"hid_open_path\0")?,
            *library.get::<SendFeature>(b"hid_send_feature_report\0")?,
            *library.get::<GetFeature>(b"hid_get_feature_report\0")?,
            *library.get::<Close>(b"hid_close\0")?,
            *library.get::<Enumerate>(b"hid_enumerate\0")?,
            *library.get::<FreeEnumeration>(b"hid_free_enumeration\0")?,
        )
    };
    Ok(NativeApi {
        _library: library,
        path,
        open,
        send,
        get,
        close,
        enumerate,
        free_enumeration,
    })
}

unsafe fn copy_wide(value: *const u16) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let mut length = 0usize;
    while length < 4096 && unsafe { *value.add(length) } != 0 {
        length += 1;
    }
    if length == 4096 {
        return None;
    }
    let slice = unsafe { std::slice::from_raw_parts(value, length) };
    Some(String::from_utf16_lossy(slice))
}

pub(super) fn enumerate() -> anyhow::Result<Vec<EnumeratedDevice>> {
    let api = native_api()?;
    // Zero filters request the full current HID enumeration, matching
    // node-rz-hid's HID.devices() behavior.
    let mut current = unsafe { (api.enumerate)(0, 0) };
    let head = current;
    let mut devices = Vec::new();
    let result = (|| {
        while !current.is_null() {
            // SAFETY: current points into the native list until it is freed.
            let item = unsafe { &*current };
            ensure!(!item.path.is_null(), "原生 HID 枚举返回了空设备路径");
            // SAFETY: hid_enumerate owns a NUL-terminated ANSI path/string.
            let path = unsafe { CStr::from_ptr(item.path) }
                .to_string_lossy()
                .into_owned();
            devices.push(EnumeratedDevice {
                path,
                vendor_id: item.vendor_id,
                product_id: item.product_id,
                interface_number: item.interface_number,
                // SAFETY: these are native-owned UTF-16 strings, copied before
                // hid_free_enumeration releases their storage.
                serial_number: unsafe { copy_wide(item.serial_number) },
                manufacturer: unsafe { copy_wide(item.manufacturer_string) },
                product: unsafe { copy_wide(item.product_string) },
                usage_page: item.usage_page,
                usage: item.usage,
            });
            current = item.next;
        }
        Ok(())
    })();
    // SAFETY: head is the exact list returned by this library; its exported
    // destructor frees each owned path/string/node exactly once.
    unsafe { (api.free_enumeration)(head) };
    result?;
    Ok(devices)
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

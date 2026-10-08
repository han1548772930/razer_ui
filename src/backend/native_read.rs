//! Worker-only queries with current source calls, sessions and PE evidence.
use super::dll::EngineLibrary;
use super::native_library::{self, NativeLibrary, NativeResource};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::{
    ffi::{CStr, CString, c_char, c_long, c_void},
    mem::ManuallyDrop,
    path::PathBuf,
};

type GetPointer = unsafe extern "C" fn(*const c_char) -> *mut c_char;
type GetBool = unsafe extern "C" fn(*const c_char) -> bool;
type GetLong = unsafe extern "C" fn(*const c_char) -> c_long;
type GetInt = unsafe extern "C" fn(*const c_char) -> i32;
type GetUint = unsafe extern "C" fn(*const c_char) -> u32;
type GetFloat = unsafe extern "C" fn(*const c_char) -> f32;
type GetDouble = unsafe extern "C" fn(*const c_char) -> f64;
type FreeMalloc = unsafe extern "C" fn(*mut c_void);

/// MD5 compares local bytes with the prepared resource; its official SHA-256
/// and PE identity were independently checked by the static preparation tool.
pub(crate) fn verified_resource<'a>(
    library: &'a NativeLibrary,
    product_id: Option<u32>,
    required: &[&str],
) -> anyhow::Result<(&'a NativeResource, PathBuf)> {
    let candidates = native_library::resource_candidates(library, product_id);
    ensure!(
        !candidates.is_empty(),
        "{} 缺少该产品的当前官方 DLL 资源证据，请指定 productId 或补齐源码",
        library.id
    );
    let (resource, path) = candidates
        .into_iter()
        .find(|(_, path)| path.is_file())
        .with_context(|| format!("{} 的当前官方 DLL 路径不存在（未安装该组件）", library.id))?;
    ensure!(
        resource.machine == 0x8664 && size_of::<usize>() == 8,
        "仅接入已静态核实的 AMD64 DLL"
    );
    for name in required {
        ensure!(
            resource.exports.iter().any(|export| export == name),
            "当前官方 {} 未导出 {name}，源码声明不能替代实际导出",
            resource.file
        );
    }
    let bytes =
        std::fs::read(&path).with_context(|| format!("无法读取 DLL 文件 {}", path.display()))?;
    ensure!(
        bytes.len() == resource.bytes && format!("{:x}", md5::compute(&bytes)) == resource.md5,
        "{} 与已静态核实的当前官方 DLL 字节不一致",
        path.display()
    );
    Ok((resource, path))
}

pub(crate) fn getter(
    library_id: &str,
    export: &str,
    device_id: &str,
    product_id: Option<u32>,
    mut validate_target: impl FnMut() -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    let library = native_library::find(library_id).context("原生库不在当前源码清单里")?;
    ensure!(
        !super::blocks_load(&library.id),
        "{} 的加载生命周期尚未核实，未接入读取",
        library.id
    );
    let declared = library
        .function(export)
        .context("没有唯一、已核实的导出声明")?;
    ensure!(
        declared.args == ["string"],
        "{export} 尚不属于已接入的单 ContainerId 查询"
    );
    let call = declared
        .calls
        .iter()
        .find(|call| {
            call.method.starts_with("get")
                && ["this.containerId", "this.deviceContainerId"].contains(&call.argument.as_str())
        })
        .context("尚未核实以真实 ContainerId 调用的原始 getter")?;
    let session = library
        .sessions
        .iter()
        .find(|session| {
            session.path == call.receipt.path
                && session.class_offset == call.class_offset
                && session.argument == call.argument
        })
        .context("初始化/释放调用链尚未核实")?;
    ensure!(
        matches!(
            declared.returns.as_str(),
            "pointer" | "char*" | "bool" | "long" | "int" | "uint" | "float" | "double"
        ),
        "{export} 的返回类型 {} 尚未接入",
        declared.returns
    );
    let free = library
        .function("FreeMalloc")
        .context("当前绑定表缺少 FreeMalloc")?;
    ensure!(
        free.returns == "void" && free.args == ["pointer"],
        "FreeMalloc 的原始签名不匹配"
    );
    let (resource, path) = verified_resource(
        library,
        product_id,
        &[
            export,
            &session.initialize,
            &session.terminate,
            "FreeMalloc",
        ],
    )?;
    let argument = CString::new(device_id).context("ContainerId 含 NUL")?;
    validate_target()?;
    // Native background-thread lifetime has not been proved; retain the module
    // until the isolated worker exits, as the host retains its FFI handlers.
    let loaded = ManuallyDrop::new(EngineLibrary::load(&path)?);
    let initialize: GetBool =
        unsafe { loaded.func(&session.initialize) }.context("缺少初始化导出")?;
    let terminate: GetBool =
        unsafe { loaded.func(&session.terminate) }.context("缺少会话释放导出")?;
    let release: FreeMalloc = unsafe { loaded.func("FreeMalloc") }.context("缺少字符串释放导出")?;
    let result: anyhow::Result<Value> = (|| {
        ensure!(
            session.initialize_attempts > 0 && session.initialize_attempts <= 10,
            "源码初始化计划无效"
        );
        let mut initialized = false;
        for attempt in 0..session.initialize_attempts {
            initialized = unsafe { initialize(argument.as_ptr()) };
            if initialized {
                break;
            }
            if attempt + 1 < session.initialize_attempts {
                std::thread::sleep(std::time::Duration::from_millis(session.retry_delay_ms));
            }
        }
        ensure!(
            initialized,
            "{} 未接受设备初始化，未执行 getter",
            session.initialize
        );
        validate_target()?;
        let value = match declared.returns.as_str() {
            "pointer" | "char*" => {
                let function: GetPointer =
                    unsafe { loaded.func(export) }.context("缺少查询导出")?;
                let raw = unsafe { function(argument.as_ptr()) };
                ensure!(!raw.is_null(), "{export} 返回空指针");
                let text = unsafe { CStr::from_ptr(raw) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { release(raw.cast()) };
                ensure!(!text.trim().is_empty(), "{export} 返回空结果");
                if call.decoder == "json" {
                    let value: Value =
                        serde_json::from_str(&text).context("原始 getter 返回的 JSON 无效")?;
                    ensure!(!value.is_null(), "{export} 返回 JSON null");
                    value
                } else {
                    Value::String(text)
                }
            }
            "bool" => {
                let function: GetBool = unsafe { loaded.func(export) }.context("缺少查询导出")?;
                json!(unsafe { function(argument.as_ptr()) })
            }
            "long" => {
                let function: GetLong = unsafe { loaded.func(export) }.context("缺少查询导出")?;
                json!(unsafe { function(argument.as_ptr()) })
            }
            "int" => {
                let function: GetInt = unsafe { loaded.func(export) }.context("缺少查询导出")?;
                json!(unsafe { function(argument.as_ptr()) })
            }
            "uint" => {
                let function: GetUint = unsafe { loaded.func(export) }.context("缺少查询导出")?;
                json!(unsafe { function(argument.as_ptr()) })
            }
            "float" => {
                let function: GetFloat = unsafe { loaded.func(export) }.context("缺少查询导出")?;
                let value = unsafe { function(argument.as_ptr()) };
                ensure!(value.is_finite(), "{export} 返回非有限数值");
                json!(value)
            }
            "double" => {
                let function: GetDouble = unsafe { loaded.func(export) }.context("缺少查询导出")?;
                let value = unsafe { function(argument.as_ptr()) };
                ensure!(value.is_finite(), "{export} 返回非有限数值");
                json!(value)
            }
            _ => unreachable!("return type checked before load"),
        };
        validate_target()?;
        Ok(
            json!({"library":library.id,"path":path,"source_sha256":resource.sha256,"export":export,
            "device_id":device_id,"product_id":product_id,"value":value,"declared_returns":declared.returns,
            "source_call":call.receipt}),
        )
    })();
    // The original wrapper terminates a configured session even if its device
    // initialization did not succeed. Preserve both query and cleanup errors.
    let terminated = unsafe { terminate(argument.as_ptr()) };
    match (result, terminated) {
        (Ok(value), true) => Ok(value),
        (Err(error), true) => Err(error),
        (Ok(_), false) => anyhow::bail!("{} 返回失败，读取会话未完整结束", session.terminate),
        (Err(error), false) => Err(error.context(format!("{} 同时返回失败", session.terminate))),
    }
}

/// Static eligibility only. The product's manifest must own a matching PE
/// export, and the same source class must provide its container session.
pub(crate) fn readable_exports(library: &NativeLibrary, product_id: u32) -> Vec<&str> {
    if super::blocks_load(&library.id) {
        return Vec::new();
    }
    library
        .declared_functions
        .iter()
        .filter(|function| {
            function.args == ["string"]
                && matches!(
                    function.returns.as_str(),
                    "pointer" | "char*" | "bool" | "long" | "int" | "uint" | "float" | "double"
                )
                && library.resources.iter().any(|resource| {
                    resource.machine == 0x8664
                        && resource
                            .bindings
                            .iter()
                            .any(|binding| binding.product_id == Some(product_id))
                        && resource.exports.contains(&function.name)
                })
                && function.calls.iter().any(|call| {
                    call.method.starts_with("get")
                        && ["this.containerId", "this.deviceContainerId"]
                            .contains(&call.argument.as_str())
                        && library.sessions.iter().any(|session| {
                            session.path == call.receipt.path
                                && session.class_offset == call.class_offset
                                && session.argument == call.argument
                        })
                })
        })
        .map(|function| function.name.as_str())
        .collect()
}

/// Each getter is an independent bounded read session; never execute the
/// original product bootstrap, which can start streams and write configuration.
pub(crate) fn snapshot(
    library_id: &str,
    device_id: &str,
    product_id: u32,
    mut validate_target: impl FnMut() -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    let library = native_library::find(library_id).context("原生库不在当前源码清单里")?;
    let exports = readable_exports(library, product_id);
    ensure!(
        !exports.is_empty(),
        "当前产品尚无已核实的 ContainerId 查询链，不能返回成功空集合"
    );
    validate_target()?;
    let mut fields = serde_json::Map::new();
    let mut errors = serde_json::Map::new();
    for export in exports {
        match getter(
            library_id,
            export,
            device_id,
            Some(product_id),
            &mut validate_target,
        ) {
            Ok(value) => {
                fields.insert(export.to_owned(), value);
            }
            Err(error) => {
                errors.insert(export.to_owned(), Value::String(format!("{error:#}")));
            }
        }
    }
    validate_target()?;
    Ok(
        json!({"library":library.id,"product_id":product_id,"device_container_id":device_id,
        "status":if errors.is_empty(){"received"}else{"partial"},
        "scope":"source_verified_container_getters_only","complete_profile_read":false,
        "fields":fields,"errors":errors}),
    )
}

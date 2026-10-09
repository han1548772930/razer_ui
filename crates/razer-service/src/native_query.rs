//! Worker-only version queries; source binding, allocator and PE must agree.
use super::{dll::EngineLibrary, native_read::verified_resource};
use anyhow::{Context as _, ensure};
use razer_catalog::native_library;
use serde_json::{Value, json};
use std::{
    ffi::{CStr, c_char, c_void},
    mem::ManuallyDrop,
};

type VersionFn = unsafe extern "C" fn() -> *mut c_char;
type ReleaseFn = unsafe extern "C" fn(*mut c_void);

pub fn version(library_id: &str, product_id: Option<u32>) -> anyhow::Result<Value> {
    let library = native_library::find(library_id).context("原生库不在当前源码清单里")?;
    ensure!(
        !razer_catalog::engines::blocks_load(&library.id),
        "{} 的加载生命周期尚未核实，未接入读取",
        library.id
    );
    let declared = [
        "GetDLLVersion",
        "GetDllVersion",
        "GetLibVersion",
        "getSDKVersion",
    ]
    .iter()
    .find_map(|name| library.function(name))
    .context("当前源码没有唯一的无参版本导出声明")?;
    ensure!(
        declared.args.is_empty(),
        "版本导出的实际声明带参数，尚未接入"
    );
    // ffiMain.readCString releases pointer results with FreeMalloc; the current
    // lighting driver's separate getDllVersion wrapper calls FreeString.
    let release_name = match declared.returns.as_str() {
        "pointer" => "FreeMalloc",
        "char*" if library.id.eq_ignore_ascii_case("lighting_driver") => "FreeString",
        _ => anyhow::bail!("版本返回类型/所有权尚未核实：{}", declared.returns),
    };
    let release_declared = library
        .function(release_name)
        .context("当前源码没有对应的释放导出")?;
    ensure!(
        release_declared.returns == "void" && release_declared.args == ["pointer"],
        "释放导出的原始签名不匹配"
    );
    let (resource, path) = verified_resource(library, product_id, &[&declared.name, release_name])?;
    let loaded = ManuallyDrop::new(EngineLibrary::load(&path)?);
    let get: VersionFn = unsafe { loaded.func(&declared.name) }.context("缺少版本查询导出")?;
    let release: ReleaseFn = unsafe { loaded.func(release_name) }.context("缺少字符串释放导出")?;
    let raw = unsafe { get() };
    ensure!(!raw.is_null(), "版本查询返回空指针");
    let text = unsafe { CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { release(raw.cast()) };
    ensure!(!text.trim().is_empty(), "版本查询返回空结果");
    Ok(
        json!({"library":library.id,"product_id":product_id,"path":path,"export":declared.name,
        "release_export":release_name,"version":text,"source_sha256":resource.sha256}),
    )
}

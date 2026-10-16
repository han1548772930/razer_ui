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

/// IDA/Hex-Rays at RVA 0x1ae10 shows that the current RzAudioUtil version
/// export only formats the constants 1, 0, 3, 1, allocates strlen+1 bytes and
/// copies the NUL-terminated result. Keep this deterministic local semantic
/// independent of vendor DLL loading. Product 1401's separate 1.0.1.1 binary
/// has the same ownership behavior and its own constants and IDA evidence.
fn source_rzaudioutil_version(product_id: Option<u32>) -> anyhow::Result<Value> {
    const CURRENT_PRODUCTS: &[u32] = &[1398, 1422, 1427, 1446, 2638, 2641, 4124, 4126];
    let product =
        product_id.context("RzAudioUtil GetDLLVersion 需要当前 middleware 的 product_id")?;
    let (version, sha256) = if product == 1401 {
        (
            "1.0.1.1",
            "012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11",
        )
    } else {
        ensure!(
            CURRENT_PRODUCTS.contains(&product),
            "RzAudioUtil GetDLLVersion 的当前 IDA 语义未覆盖 product_id={product}"
        );
        (
            "1.0.3.1",
            "9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf",
        )
    };
    Ok(json!({
        "library": "RzAudioUtil",
        "product_id": product_id,
        "path": null,
        "export": "GetDLLVersion",
        "release_export": "FreeMalloc",
        "version": version,
        "source_sha256": sha256,
        "transport": "source-derived",
        "vendor_dll_loaded": false,
        "evidence": "docs/re/rzaudioutil-dll-version-current.md"
    }))
}

pub fn version(library_id: &str, product_id: Option<u32>) -> anyhow::Result<Value> {
    let library = native_library::find(library_id).context("原生库不在当前源码清单里")?;
    ensure!(
        !razer_catalog::engines::blocks_load(&library.id),
        "{} 的加载生命周期尚未核实，未接入读取",
        library.id
    );
    if library.id.eq_ignore_ascii_case("RzAudioUtil") {
        return source_rzaudioutil_version(product_id);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rzaudioutil_current_product_bindings_use_ida_version_semantics() {
        for product in [1398, 1422, 1427, 1446, 2638, 2641, 4124, 4126] {
            let response = version("RzAudioUtil", Some(product)).unwrap();
            assert_eq!(response["version"], "1.0.3.1");
            assert_eq!(response["vendor_dll_loaded"], false);
            assert!(response["path"].is_null());
            assert_eq!(
                response["source_sha256"],
                "9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf"
            );
        }
        let response = version("RzAudioUtil", Some(1401)).unwrap();
        assert_eq!(response["version"], "1.0.1.1");
        assert_eq!(response["vendor_dll_loaded"], false);
        assert_eq!(
            response["source_sha256"],
            "012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11"
        );
    }

    #[test]
    fn rzaudioutil_missing_or_unproved_product_fails_before_native_loading() {
        assert!(version("RzAudioUtil", None).is_err());
        assert!(version("RzAudioUtil", Some(182)).is_err());
        assert!(version("RzAudioUtil", Some(u32::MAX)).is_err());
    }
}

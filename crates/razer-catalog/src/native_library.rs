//! Generated inventory of every native library the current sources declare.
//!
//! The asset is produced by `tools/generate-native-library-inventory.cjs` from
//! the reverse-engineered host wrappers and product middleware `ConfigureFFI`
//! payloads. Nothing here is product-specific: every id, file name, path rule
//! and declared function comes from that asset, so a new library or a changed
//! binding table is picked up by regenerating, never by editing Rust.
//!
//! Declaring a function proves only that the vendor JavaScript passes it to
//! `ffi-napi-rz.Library`. It does not prove the export exists, that the ABI is
//! correct, or that calling it is safe. The current scope therefore exposes the
//! declaration plus read-only path resolution; mutation/lifecycle entries stay
//! declared-but-deferred and must not be reported as applied.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

/// What kind of native artifact this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    Engine,
    Device,
    LightingDriver,
    Iot,
    Template,
}

/// Where the original loads the file from, relative to the user data or the
/// product-files root. Derived from the `userDataDir + "\\Apps\\..."` literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRule {
    /// `CommonDLL\<file>` under `%ProgramFiles%\Razer\RazerAppEngine\app-<ver>`.
    CommonDll,
    /// `%LOCALAPPDATA%\Razer\RazerAppEngine\User Data\Apps\Synapse\<file>`.
    AppsSynapse,
    /// `...\Apps\Synapse\<product_id>\<file>`.
    AppsSynapseProduct,
    /// `...\Apps\Common\<file>`.
    AppsCommon,
    /// `...\Apps\Common\<product_id>\RzNative_<hex pid>.dll`.
    AppsCommonProduct,
}

impl PathRule {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "common_dll" => Some(Self::CommonDll),
            "apps_synapse" => Some(Self::AppsSynapse),
            "apps_synapse_product" => Some(Self::AppsSynapseProduct),
            "apps_common" => Some(Self::AppsCommon),
            "apps_common_product" => Some(Self::AppsCommonProduct),
            _ => None,
        }
    }
}

/// One entry of an `apiObj` binding table.
#[derive(Debug, Clone, Deserialize)]
pub struct NativeFunction {
    pub name: String,
    pub returns: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub calls: Vec<NativeCall>,
}

/// Exact action object and argument expression from the current wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct NativeCall {
    pub argument: String,
    pub method: String,
    pub decoder: String,
    pub class_offset: usize,
    pub receipt: Receipt,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeSession {
    pub argument: String,
    pub initialize: String,
    pub terminate: String,
    pub class_offset: usize,
    pub path: String,
    pub initialize_attempts: usize,
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeResourceBinding {
    pub product_id: Option<u32>,
    pub install_relative_path: String,
}

/// Manifest-pinned bytes and statically inspected direct exports.
#[derive(Debug, Clone, Deserialize)]
pub struct NativeResource {
    pub file: String,
    pub bytes: usize,
    pub sha256: String,
    pub md5: String,
    pub machine: u16,
    pub exports: Vec<String>,
    pub bindings: Vec<NativeResourceBinding>,
}

/// Static provenance for a declaration or a default path.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Receipt {
    pub path: String,
    pub offset: usize,
    pub end: usize,
    pub sha256: String,
    #[serde(default)]
    pub source: String,
}

/// A native library as the current sources declare it.
#[derive(Debug, Clone, Deserialize)]
pub struct NativeLibrary {
    pub id: String,
    pub kind: LibraryKind,
    pub channel: String,
    pub file_names: Vec<String>,
    pub path_rules: Vec<String>,
    #[serde(default)]
    pub products: Vec<u32>,
    #[serde(default)]
    pub declared_functions: Vec<NativeFunction>,
    #[serde(default)]
    pub receipts: Vec<Receipt>,
    #[serde(default)]
    pub resources: Vec<NativeResource>,
    #[serde(default)]
    pub sessions: Vec<NativeSession>,
}

impl NativeLibrary {
    /// Parsed path rules, in the order the generator emitted them.
    pub fn rules(&self) -> Vec<PathRule> {
        self.path_rules
            .iter()
            .filter_map(|rule| PathRule::parse(rule))
            .collect()
    }

    /// Look up one declared function by name.
    pub fn function(&self, name: &str) -> Option<&NativeFunction> {
        self.declared_functions
            .iter()
            .find(|function| function.name == name)
    }
}

#[derive(Deserialize)]
struct Asset {
    schema_version: u8,
    libraries: Vec<NativeLibrary>,
}

fn catalog() -> &'static [NativeLibrary] {
    static CATALOG: OnceLock<Vec<NativeLibrary>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let asset: Asset = serde_json::from_str(include_str!(
            "../../../assets/data/native-library-inventory.json"
        ))
        .expect("statically generated native library inventory must be valid");
        assert_eq!(asset.schema_version, 1);
        asset.libraries
    })
}

/// Every declared native library, in generated order.
pub fn all() -> &'static [NativeLibrary] {
    catalog()
}

/// Find a library by generated id (for example `mapping_engine`, `RzNative_0542`).
pub fn find(id: &str) -> Option<&'static NativeLibrary> {
    catalog()
        .iter()
        .find(|library| library.id.eq_ignore_ascii_case(id))
}

/// Libraries whose generated product list contains `product_id`.
///
/// The product list comes from the middleware directories the library's default
/// path was written in, so it links an observed device to the libraries its own
/// product app declares. An empty result means no declared library for that
/// product; it is not evidence that the product has no native component.
pub fn libraries_for_product(product_id: u32) -> Vec<&'static NativeLibrary> {
    catalog()
        .iter()
        .filter(|library| library.products.contains(&product_id))
        .collect()
}

/// Libraries that declare at least one function whose name matches `needle`.
pub fn with_function(needle: &str) -> Vec<&'static NativeLibrary> {
    catalog()
        .iter()
        .filter(|library| library.function(needle).is_some())
        .collect()
}

/// Total declared functions across every library.
pub fn declared_function_count() -> usize {
    catalog()
        .iter()
        .map(|library| library.declared_functions.len())
        .sum()
}

/// Resolve one library's file candidates for a product, plus the rules that
/// produced them. Paths are returned unverified: the caller must still check the
/// file exists and must never treat presence as a verified ABI.
pub fn candidate_paths(
    library: &NativeLibrary,
    product_id: Option<u32>,
    programs_root: &std::path::Path,
    user_data_root: &std::path::Path,
) -> Vec<(PathRule, PathBuf)> {
    let mut paths = Vec::new();
    let apps_root = user_data_root.join("Apps");
    for resource in &library.resources {
        for binding in &resource.bindings {
            if product_id.is_some() && binding.product_id != product_id {
                continue;
            }
            let name = &binding.install_relative_path;
            let (rule, path) = if let Some(file) = name.strip_prefix("CommonDLL/") {
                (
                    PathRule::CommonDll,
                    programs_root.join("CommonDLL").join(file),
                )
            } else if name.to_ascii_lowercase().starts_with("common/") {
                (
                    if name.split('/').count() > 2 {
                        PathRule::AppsCommonProduct
                    } else {
                        PathRule::AppsCommon
                    },
                    apps_root.join(name),
                )
            } else if name.to_ascii_lowercase().starts_with("synapse/") {
                (
                    if name.split('/').count() > 2 {
                        PathRule::AppsSynapseProduct
                    } else {
                        PathRule::AppsSynapse
                    },
                    apps_root.join(name),
                )
            } else {
                continue;
            };
            if !paths.iter().any(|(_, existing)| existing == &path) {
                paths.push((rule, path));
            }
        }
    }
    if !paths.is_empty() {
        return paths;
    }
    for rule in library.rules() {
        let file = library.file_names.first();
        match (rule, file) {
            (PathRule::CommonDll, Some(file)) => {
                paths.push((rule, programs_root.join("CommonDLL").join(file)));
            }
            (PathRule::AppsSynapse, Some(file)) => {
                paths.push((rule, apps_root.join("Synapse").join(file)));
            }
            (PathRule::AppsSynapseProduct, Some(file)) => {
                if let Some(product) = product_id {
                    paths.push((
                        rule,
                        apps_root
                            .join("Synapse")
                            .join(product.to_string())
                            .join(file),
                    ));
                }
            }
            (PathRule::AppsCommon, Some(file)) => {
                paths.push((rule, apps_root.join("Common").join(file)));
            }
            (PathRule::AppsCommonProduct, _) => {
                // `RzNative_<hex product id>.dll` under Apps\Common\<product>\.
                if let Some(product) = product_id {
                    for name in &library.file_names {
                        if name.ends_with(".dll") && !name.contains('<') {
                            paths.push((
                                rule,
                                apps_root
                                    .join("Common")
                                    .join(product.to_string())
                                    .join(name),
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    paths
}

pub fn readable_exports(library: &NativeLibrary, product_id: u32) -> Vec<&str> {
    if super::engines::blocks_load(&library.id) {
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

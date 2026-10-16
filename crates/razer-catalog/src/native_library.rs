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
//! correct, or that calling it is safe. Reads, writes and lifecycle behavior are
//! all in scope, but this inventory only exposes declarations and path facts.
//! Actual implementations require source-proved ABI/protocol and caller chains;
//! an inventory entry alone must never be reported as an applied operation.
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
    /// Current lighting-engine app declares RzLightingEngineApi separately
    /// from its lighting_driver resource and transport channel.
    LightingEngine,
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
    /// Absent when the current loader reads installedResources.filePath.
    pub install_relative_path: Option<String>,
    #[serde(default)]
    pub install_path_template: Option<String>,
    #[serde(default)]
    pub fallback_relative_path: Option<String>,
}

impl NativeResourceBinding {
    /// A static candidate, never a claim that the runtime resource was observed.
    pub fn static_relative_path(&self) -> Option<&str> {
        self.install_relative_path
            .as_deref()
            .or(self.fallback_relative_path.as_deref())
    }
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
        let asset: Asset = read_native_library_inventory_catalog();
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
            let Some(name) = binding.static_relative_path() else {
                continue;
            };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_dynamic_lighting_bindings_parse_without_fabricating_an_install_path() {
        for (id, fallback, kind) in [
            (
                "lighting_driver",
                "Synapse/lighting_driver.dll",
                LibraryKind::LightingDriver,
            ),
            (
                "RzLightingEngineApi",
                "Synapse/RzLightingEngineApi.dll",
                LibraryKind::LightingEngine,
            ),
        ] {
            let library = find(id).expect("current lighting source resource");
            assert_eq!(library.kind, kind);
            let binding = &library.resources[0].bindings[0];
            assert!(binding.install_relative_path.is_none());
            assert!(
                binding
                    .install_path_template
                    .as_deref()
                    .unwrap()
                    .contains("installedResources")
            );
            assert_eq!(binding.static_relative_path(), Some(fallback));
            let candidates = candidate_paths(
                library,
                None,
                std::path::Path::new("programs"),
                std::path::Path::new("userData"),
            );
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].0, PathRule::AppsSynapse);
            assert_eq!(
                candidates[0].1,
                std::path::Path::new("userData").join("Apps").join(fallback)
            );
        }
    }
}

// Generated catalog reader: razer-catalog/native_library_inventory.
fn read_native_library_inventory_catalog() -> Asset {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Asset> {
        let value = match r.tag()? {
            0 => Asset {
                schema_version: r.child(decode_1)?,
                libraries: r.child(decode_2)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u8> {
        let value = match r.tag()? {
            0 => r.u8()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<NativeLibrary>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<NativeLibrary> {
        let value = match r.tag()? {
            0 => NativeLibrary {
                id: r.child(decode_4)?,
                kind: r.child(decode_5)?,
                channel: r.child(decode_4)?,
                file_names: r.child(decode_6)?,
                path_rules: r.child(decode_6)?,
                products: r.child(decode_7)?,
                declared_functions: r.child(decode_9)?,
                receipts: r.child(decode_10)?,
                resources: r.child(decode_11)?,
                sessions: r.child(decode_19)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<LibraryKind> {
        let value = match r.tag()? {
            0 => LibraryKind::Device,
            1 => LibraryKind::LightingDriver,
            2 => LibraryKind::Engine,
            3 => LibraryKind::LightingEngine,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<u32>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_8)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<NativeFunction>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.sequence(decode_20)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Receipt>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.sequence(decode_23)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<NativeResource>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_12)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<NativeResource> {
        let value = match r.tag()? {
            0 => NativeResource {
                file: r.child(decode_4)?,
                bytes: r.child(decode_13)?,
                sha256: r.child(decode_4)?,
                md5: r.child(decode_4)?,
                machine: r.child(decode_14)?,
                exports: r.child(decode_6)?,
                bindings: r.child(decode_15)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<usize> {
        let value = match r.tag()? {
            0 => r.usize()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u16> {
        let value = match r.tag()? {
            0 => r.u16()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<NativeResourceBinding>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_16)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_16(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<NativeResourceBinding> {
        let value = match r.tag()? {
            0 => NativeResourceBinding {
                product_id: r.child(decode_17)?,
                install_relative_path: r.child(decode_18)?,
                install_path_template: None,
                fallback_relative_path: None,
            },
            1 => NativeResourceBinding {
                product_id: None,
                install_relative_path: None,
                install_path_template: r.child(decode_18)?,
                fallback_relative_path: r.child(decode_18)?,
            },
            2 => NativeResourceBinding {
                product_id: None,
                install_relative_path: r.child(decode_18)?,
                install_path_template: None,
                fallback_relative_path: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_17(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u32>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_8)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_18(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_4)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_19(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<NativeSession>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.sequence(decode_24)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_20(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<NativeFunction> {
        let value = match r.tag()? {
            0 => NativeFunction {
                name: r.child(decode_4)?,
                returns: r.child(decode_4)?,
                args: r.child(decode_6)?,
                calls: r.child(decode_21)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_21(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<NativeCall>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_22)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_22(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<NativeCall> {
        let value = match r.tag()? {
            0 => NativeCall {
                argument: r.child(decode_4)?,
                method: r.child(decode_4)?,
                decoder: r.child(decode_4)?,
                class_offset: r.child(decode_13)?,
                receipt: r.child(decode_23)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_23(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Receipt> {
        let value = match r.tag()? {
            0 => Receipt {
                path: r.child(decode_4)?,
                offset: r.child(decode_13)?,
                end: r.child(decode_13)?,
                sha256: r.child(decode_4)?,
                source: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_24(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<NativeSession> {
        let value = match r.tag()? {
            0 => NativeSession {
                argument: r.child(decode_4)?,
                initialize: r.child(decode_4)?,
                terminate: r.child(decode_4)?,
                class_offset: r.child(decode_13)?,
                path: r.child(decode_4)?,
                initialize_attempts: r.child(decode_13)?,
                retry_delay_ms: r.child(decode_25)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_25(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u64> {
        let value = match r.tag()? {
            0 => r.u64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_CATALOG_NATIVE_LIBRARY_INVENTORY,
        0,
        "9032c9a732c8c0f74b317d477b31906e742466f9a2b55f230cf3f3509d12db76",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod native_library_inventory_catalog_tests {
    #[test]
    fn packaged_catalog_native_library_inventory_decodes() {
        let _ = super::read_native_library_inventory_catalog();
    }
}
// End generated catalog reader: razer-catalog/native_library_inventory.

//! Installed resource catalog and independently owned, on-demand resource I/O.
//! Clients read memory and enqueue work; only the runtime accesses disk or HTTP.
mod cache;
mod catalog;
mod credentials;
mod runtime;
mod transport;

pub use catalog::{identity, list};
pub use runtime::{ResourceChanged, ResourceClient, ResourceFont, ResourceRuntime};

#[derive(Debug)]
pub struct ResourcePending;
impl std::fmt::Display for ResourcePending {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("资源正在加载")
    }
}
impl std::error::Error for ResourcePending {}

pub fn is_resource_pending(error: &anyhow::Error) -> bool {
    error.is::<ResourcePending>()
}

pub fn validate() -> anyhow::Result<()> {
    catalog::manifest().map(|_| ())
}

/// Blocking, verified inspection for CPU resource checks. Never queues downloads.
/// This API is absent from normal product builds.
#[cfg(feature = "cache-inspection")]
pub fn inspect_cached(key: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let catalog = catalog::manifest()?;
    let cache = cache::Cache::alongside(&std::env::current_exe()?)?;
    if let Some(&ix) = catalog.keys.get(key) {
        let entry = &catalog.entries[ix];
        return cache
            .read(&entry.file)?
            .map(|bytes| catalog::select_view(entry, &bytes))
            .transpose();
    }
    let Some(&ix) = catalog.paths.get(key) else {
        return Ok(None);
    };
    cache.read(&catalog.files[ix])
}

/// Opt-in, path-only diagnostics. Never log URLs, headers or credentials.
pub fn trace(stage: &str, path: &str, detail: impl std::fmt::Display) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("RAZER_RESOURCE_TRACE").is_some()) {
        eprintln!("[resource] {stage} {path} {detail}");
    }
}

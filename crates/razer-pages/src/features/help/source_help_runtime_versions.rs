//! Current Help runtime versions. Storage reads are observations, not UI builds.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[path = "source_help_version_storage.rs"]
mod storage;

#[derive(Clone, Debug)]
pub struct HelpRuntimeVersionsRequest {
    owner: EntityId,
    generation: u64,
    product_id: u32,
    page_offset: usize,
    edition_id: u32,
    layout_id: u32,
    serial_number: String,
    container_id: String,
    cancelled: Arc<AtomicBool>,
}
impl HelpRuntimeVersionsRequest {
    pub fn owner(&self) -> EntityId {
        self.owner
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn container_id(&self) -> &str {
        &self.container_id
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }
    pub fn same_mount(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.generation == other.generation
            && self.product_id == other.product_id
            && self.page_offset == other.page_offset
            && self.edition_id == other.edition_id
            && self.layout_id == other.layout_id
            && self.serial_number == other.serial_number
            && self.container_id == other.container_id
            && Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HelpRuntimeVersionStatus {
    Unobserved,
    Loading,
    Missing,
    Observed(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpRuntimeVersionsStatus {
    pub middleware: HelpRuntimeVersionStatus,
    pub application: HelpRuntimeVersionStatus,
}
impl HelpRuntimeVersionsStatus {
    pub fn from_storage(
        product_id: u32,
        container_id: &str,
        middleware: Result<Option<String>, String>,
        apps: Result<Option<String>, String>,
    ) -> Self {
        Self {
            middleware: storage::middleware_version(product_id, container_id, middleware),
            application: storage::application_version(apps),
        }
    }
}

/// The inputs are actual host WindowStorage and localStorage observations.
/// Each field keeps its own read failure or absence; neither is a UI build.
#[derive(Debug)]
pub struct HelpRuntimeVersionsObservation {
    request: HelpRuntimeVersionsRequest,
    status: HelpRuntimeVersionsStatus,
}
impl HelpRuntimeVersionsObservation {
    pub fn from_status(
        request: HelpRuntimeVersionsRequest,
        status: HelpRuntimeVersionsStatus,
    ) -> Self {
        Self { request, status }
    }
    pub fn from_storage(
        request: HelpRuntimeVersionsRequest,
        middleware: Result<Option<String>, String>,
        apps: Result<Option<String>, String>,
    ) -> Self {
        let status = HelpRuntimeVersionsStatus::from_storage(
            request.product_id,
            &request.container_id,
            middleware,
            apps,
        );
        Self { request, status }
    }
    pub fn failed(request: HelpRuntimeVersionsRequest, error: String) -> Self {
        Self {
            request,
            status: HelpRuntimeVersionsStatus {
                middleware: HelpRuntimeVersionStatus::Failed(error.clone()),
                application: HelpRuntimeVersionStatus::Failed(error),
            },
        }
    }
    pub fn status(&self) -> &HelpRuntimeVersionsStatus {
        &self.status
    }
}

pub(super) struct State {
    owner: EntityId,
    generation: u64,
    mounted: bool,
    cancelled: Arc<AtomicBool>,
    status: HelpRuntimeVersionsStatus,
    mw_version: Option<String>,
    synapse_version: Option<String>,
}
impl State {
    pub(super) fn new(owner: EntityId) -> Self {
        Self {
            owner,
            generation: 0,
            mounted: false,
            cancelled: Arc::new(AtomicBool::new(true)),
            status: HelpRuntimeVersionsStatus {
                middleware: HelpRuntimeVersionStatus::Unobserved,
                application: HelpRuntimeVersionStatus::Unobserved,
            },
            mw_version: None,
            synapse_version: None,
        }
    }
    pub(super) fn unmount(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.mounted = false;
        self.generation = self.generation.wrapping_add(1);
        self.status.middleware = HelpRuntimeVersionStatus::Unobserved;
        self.status.application = HelpRuntimeVersionStatus::Unobserved;
        self.mw_version = None;
        self.synapse_version = None;
    }
    fn mount(&mut self) {
        self.mounted = true;
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.status.middleware = HelpRuntimeVersionStatus::Loading;
        self.status.application = HelpRuntimeVersionStatus::Loading;
    }
    fn observe(&mut self, status: HelpRuntimeVersionsStatus) {
        // Help updates truthy successful values only. A subsequent missing or
        // failed read keeps the mounted source display and its own read status.
        if let HelpRuntimeVersionStatus::Observed(version) = &status.middleware {
            self.mw_version = Some(version.clone());
        }
        if let HelpRuntimeVersionStatus::Observed(version) = &status.application {
            self.synapse_version = Some(version.clone());
        }
        self.status = status;
    }
    pub(super) fn rows(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [
            ("MW_VERSION", self.mw_version.as_deref()),
            ("SYNAPSE_VERSION", self.synapse_version.as_deref()),
        ]
        .into_iter()
        .filter_map(|(label, value)| value.map(|value| (label, value)))
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

#[derive(Deserialize)]
struct Supported {
    product_id: u32,
    offset: usize,
    retains_serial: bool,
}
fn records() -> &'static [Supported] {
    static SUPPORTED: OnceLock<Vec<Supported>> = OnceLock::new();
    SUPPORTED.get_or_init(|| read_source_help_runtime_versions_catalog())
}
impl EventEmitter<HelpRuntimeVersionsRequest> for SourceHelp {}
impl SourceHelp {
    pub(super) fn retains_mounted_serial(&self) -> bool {
        self.page_offset.is_some_and(|offset| {
            records().iter().any(|row| {
                row.product_id == self.device.product_id
                    && row.offset == offset
                    && row.retains_serial
            })
        }) && self.metadata().is_some_and(|(_, page)| !page.camo)
    }
    pub(in crate::features) fn runtime_versions_request(
        &self,
    ) -> Option<HelpRuntimeVersionsRequest> {
        self.page_offset?;
        let (_, page) = self.metadata()?;
        (page.view_more && self.runtime_versions.mounted).then(|| HelpRuntimeVersionsRequest {
            owner: self.runtime_versions.owner,
            generation: self.runtime_versions.generation,
            product_id: self.device.product_id,
            page_offset: page.offset,
            edition_id: self.device.edition_id,
            layout_id: self.device.layout_id,
            serial_number: self.device.serial_number.clone(),
            container_id: self.device.device_container_id.clone(),
            cancelled: self.runtime_versions.cancelled.clone(),
        })
    }
    pub(in crate::features) fn accepts_runtime_versions(
        &self,
        request: &HelpRuntimeVersionsRequest,
    ) -> bool {
        !request.is_cancelled()
            && self
                .runtime_versions_request()
                .is_some_and(|current| current.same_mount(request))
    }
    pub(in crate::features) fn runtime_versions_status(&self) -> &HelpRuntimeVersionsStatus {
        &self.runtime_versions.status
    }
    pub(in crate::features) fn mount_runtime_versions(&mut self, cx: &mut Context<Self>) {
        self.runtime_versions.unmount();
        if self.page_offset.is_some() && self.metadata().is_some_and(|(_, page)| page.view_more) {
            self.runtime_versions.mount();
            if let Some(request) = self.runtime_versions_request() {
                cx.emit(request);
            }
        }
        cx.notify();
    }
    pub(in crate::features) fn observe_runtime_versions(
        &mut self,
        observation: HelpRuntimeVersionsObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_runtime_versions(&observation.request) {
            return false;
        }
        self.runtime_versions.observe(observation.status);
        cx.notify();
        true
    }
}

#[cfg(test)]
#[path = "source_help_runtime_versions_tests.rs"]
mod tests;

// Generated catalog reader: razer-pages/source_help_runtime_versions_data.
fn read_source_help_runtime_versions_catalog() -> Vec<Supported> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Supported>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Supported> {
        let value = match r.tag()? {
            0 => Supported {
                product_id: r.child(decode_2)?,
                offset: r.child(decode_3)?,
                retains_serial: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<usize> {
        let value = match r.tag()? {
            0 => r.usize()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_SOURCE_HELP_RUNTIME_VERSIONS_DATA,
        0,
        "d4f2e0520a719394f5edd1bb89b1b44198294c107449e5143eb2e3450a6724c4",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod source_help_runtime_versions_data_catalog_tests {
    #[test]
    fn packaged_catalog_source_help_runtime_versions_data_decodes() {
        let _ = super::read_source_help_runtime_versions_catalog();
    }
}
// End generated catalog reader: razer-pages/source_help_runtime_versions_data.

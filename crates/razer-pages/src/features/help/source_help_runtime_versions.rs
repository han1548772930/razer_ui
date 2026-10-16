//! Current Help runtime versions. Storage reads are observations, not UI builds.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelpRuntimeVersionsRequest {
    owner: EntityId,
    generation: u64,
}

/// `mw_version` is the result of that product's source storage selection:
/// container/product matching, or the first entry in the older six branches.
/// `apps` is the original localStorage `apps` JSON, not a Cargo version.
pub struct HelpRuntimeVersionsObservation {
    request: HelpRuntimeVersionsRequest,
    mw_version: Option<String>,
    apps: Option<String>,
}
impl HelpRuntimeVersionsObservation {
    pub fn new(request: HelpRuntimeVersionsRequest) -> Self {
        Self {
            request,
            mw_version: None,
            apps: None,
        }
    }
    pub fn mw_version(mut self, version: impl Into<String>) -> Self {
        self.mw_version = Some(version.into());
        self
    }
    pub fn apps(mut self, json: impl Into<String>) -> Self {
        self.apps = Some(json.into());
        self
    }
}

pub(super) struct State {
    owner: EntityId,
    generation: u64,
    mw_version: Option<String>,
    synapse_version: Option<String>,
}
impl State {
    pub(super) fn new(owner: EntityId) -> Self {
        Self {
            owner,
            generation: 0,
            mw_version: None,
            synapse_version: None,
        }
    }
    pub(super) fn unmount(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.mw_version = None;
        self.synapse_version = None;
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

fn synapse_version(apps: &str) -> Option<String> {
    let apps: serde_json::Value = serde_json::from_str(apps).ok()?;
    let app = apps
        .as_array()?
        .iter()
        .find(|app| app["name"] == "synapse")?;
    // A non-string version throws in the source .split(), caught without a
    // state update. The first matching app wins even if its version is missing.
    let version = app["version"]
        .as_str()
        .filter(|version| !version.is_empty())?;
    Some(
        std::iter::once("4")
            .chain(version.split('.').skip(1))
            .collect::<Vec<_>>()
            .join("."),
    )
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
pub(super) fn supported(product_id: u32, offset: usize) -> bool {
    records()
        .iter()
        .any(|row| row.product_id == product_id && row.offset == offset)
}
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
        if !supported(self.device.product_id, self.page_offset?) {
            return None;
        }
        self.metadata()?
            .1
            .view_more
            .then_some(HelpRuntimeVersionsRequest {
                owner: self.runtime_versions.owner,
                generation: self.runtime_versions.generation,
            })
    }
    pub(in crate::features) fn observe_runtime_versions(
        &mut self,
        observation: HelpRuntimeVersionsObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.runtime_versions_request() != Some(observation.request) {
            return false;
        }
        // The source only updates truthy successful results. Missing/failed
        // reads do not clear a prior result within the same mounted component.
        if let Some(version) = observation.mw_version.filter(|version| !version.is_empty()) {
            self.runtime_versions.mw_version = Some(version);
        }
        if let Some(version) = observation.apps.as_deref().and_then(synapse_version) {
            self.runtime_versions.synapse_version = Some(version);
        }
        cx.notify();
        true
    }
}

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

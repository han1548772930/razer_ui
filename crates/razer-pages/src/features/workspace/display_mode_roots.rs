//! Root-level `displayMode` branches audited from the current product bundles.
//!
//! Each product bundle selects these roots in the same ternary chain that reads
//! `searchParams.get("displayMode")`; the branches are recorded in
//! [display mode audit](../../docs/re/shell-workspace-current.md) and regenerated into
//! `display-mode-roots.json` by `tools/generate-display-mode-roots.cjs`. The
//! table only carries product ids, so a branch is never claimed for a product
//! whose bundle was not scanned.
use std::sync::OnceLock;

#[derive(serde::Deserialize)]
struct DisplayModeRoots {
    modes: DisplayModeModes,
}

#[derive(serde::Deserialize)]
struct DisplayModeModes {
    armory: Vec<u32>,
    #[serde(rename = "chromaApp")]
    chroma_app: Vec<u32>,
    #[serde(rename = "macro")]
    macro_root: Vec<u32>,
    #[serde(rename = "multiDevicePairing")]
    multi_device_pairing: Vec<u32>,
}

/// The four `displayMode` values the current product bundles compare against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayModeRoot {
    Armory,
    ChromaApp,
    Macro,
    MultiDevicePairing,
}

impl DisplayModeRoot {
    /// The literal the bundle compares `displayMode` with.
    pub fn key(self) -> &'static str {
        match self {
            Self::Armory => "armory",
            Self::ChromaApp => "chromaApp",
            Self::Macro => "macro",
            Self::MultiDevicePairing => "multiDevicePairing",
        }
    }

    fn products(self) -> &'static [u32] {
        let modes = roots();
        match self {
            Self::Armory => &modes.armory,
            Self::ChromaApp => &modes.chroma_app,
            Self::Macro => &modes.macro_root,
            Self::MultiDevicePairing => &modes.multi_device_pairing,
        }
    }
}

fn roots() -> &'static DisplayModeModes {
    static ROOTS: OnceLock<DisplayModeModes> = OnceLock::new();
    ROOTS.get_or_init(|| read_display_mode_roots_catalog().modes)
}

impl DisplayModeRoot {
    /// Audit helper: every mode the current bundles compare against.
    #[cfg(test)]
    pub fn all() -> [Self; 4] {
        [
            Self::Armory,
            Self::ChromaApp,
            Self::Macro,
            Self::MultiDevicePairing,
        ]
    }
}

/// Whether the current bundle for `product_id` mounts this root-level branch.
pub fn has_root_branch(mode: DisplayModeRoot, product_id: u32) -> bool {
    mode.products().binary_search(&product_id).is_ok()
}

#[cfg(test)]
impl DisplayModeRoot {
    /// Size of the generated table for this mode.
    pub fn products_len(self) -> usize {
        self.products().len()
    }

    /// `has_root_branch` binary-searches, so the table has to stay ordered.
    pub fn products_sorted(self) -> bool {
        self.products().windows(2).all(|pair| pair[0] < pair[1])
    }
}

#[cfg(test)]
#[path = "display_mode_roots_tests.rs"]
mod tests;

// Generated catalog reader: razer-pages/display_mode_roots.
fn read_display_mode_roots_catalog() -> DisplayModeRoots {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<DisplayModeRoots> {
        let value = match r.tag()? {
            0 => DisplayModeRoots {
                modes: r.child(decode_1)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<DisplayModeModes> {
        let value = match r.tag()? {
            0 => DisplayModeModes {
                armory: r.child(decode_2)?,
                chroma_app: r.child(decode_2)?,
                macro_root: r.child(decode_2)?,
                multi_device_pairing: r.child(decode_2)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<u32>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_DISPLAY_MODE_ROOTS,
        0,
        "f40a1335d30112a4a83864ad9e398cfc18a3ffecbd168f9b5ae7913b8e3c0c1c",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_display_mode_roots_decodes() {
    let _ = read_display_mode_roots_catalog();
}
// End generated catalog reader: razer-pages/display_mode_roots.

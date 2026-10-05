//! Root-level `displayMode` branches audited from the current product bundles.
//!
//! Each product bundle selects these roots in the same ternary chain that reads
//! `searchParams.get("displayMode")`; the branches are recorded in
//! [display mode audit](../../docs/re/display-mode-audit.md) and regenerated into
//! `display-mode-roots.json` by `tools/generate-display-mode-roots.cjs`. The
//! table only carries product ids, so a branch is never claimed for a product
//! whose bundle was not scanned.
use std::sync::OnceLock;

const DISPLAY_MODE_ROOTS: &str = include_str!("display-mode-roots.json");

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
pub(crate) enum DisplayModeRoot {
    Armory,
    ChromaApp,
    Macro,
    MultiDevicePairing,
}

impl DisplayModeRoot {
    /// The literal the bundle compares `displayMode` with.
    pub(crate) fn key(self) -> &'static str {
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
    ROOTS.get_or_init(|| {
        serde_json::from_str::<DisplayModeRoots>(DISPLAY_MODE_ROOTS)
            .expect("validated display mode root table")
            .modes
    })
}

impl DisplayModeRoot {
    /// Audit helper: every mode the current bundles compare against.
    #[cfg(test)]
    pub(crate) fn all() -> [Self; 4] {
        [
            Self::Armory,
            Self::ChromaApp,
            Self::Macro,
            Self::MultiDevicePairing,
        ]
    }
}

/// Whether the current bundle for `product_id` mounts this root-level branch.
pub(crate) fn has_root_branch(mode: DisplayModeRoot, product_id: u32) -> bool {
    mode.products().binary_search(&product_id).is_ok()
}

#[cfg(test)]
impl DisplayModeRoot {
    /// Size of the generated table for this mode.
    pub(crate) fn products_len(self) -> usize {
        self.products().len()
    }

    /// `has_root_branch` binary-searches, so the table has to stay ordered.
    pub(crate) fn products_sorted(self) -> bool {
        self.products().windows(2).all(|pair| pair[0] < pair[1])
    }
}

#[cfg(test)]
#[path = "display_mode_roots_tests.rs"]
mod tests;

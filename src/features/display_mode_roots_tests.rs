//! Receipts for the generated `displayMode` root table.
//!
//! The table is produced by `tools/generate-display-mode-roots.cjs` from the
//! audited static scan, so these checks fail when either side drifts.
use super::{DisplayModeRoot, has_root_branch};

#[test]
fn root_table_matches_the_audited_counts() {
    // Counts from docs/re/display-mode-audit.json (root branches per mode).
    for (mode, expected) in [
        (DisplayModeRoot::Armory, 226usize),
        (DisplayModeRoot::ChromaApp, 212),
        (DisplayModeRoot::Macro, 174),
        (DisplayModeRoot::MultiDevicePairing, 30),
    ] {
        assert_eq!(mode.products_len(), expected, "{} root count", mode.key());
        assert!(
            mode.products_sorted(),
            "{} table must stay sorted",
            mode.key()
        );
    }
}

#[test]
fn membership_follows_the_product_bundle() {
    assert!(has_root_branch(DisplayModeRoot::ChromaApp, 653));
    assert!(has_root_branch(DisplayModeRoot::Armory, 653));
    // 3080 (Firefly V2 Pro) only carries the macro root in the current bundle.
    assert!(!has_root_branch(DisplayModeRoot::ChromaApp, 3080));
    assert!(!has_root_branch(DisplayModeRoot::Armory, 3080));
    assert!(has_root_branch(DisplayModeRoot::Macro, 3080));
    // 691 (BlackWidow V4 Pro 75%) has no displayMode branch at all.
    assert!(
        DisplayModeRoot::all()
            .into_iter()
            .all(|mode| !has_root_branch(mode, 691))
    );
    // 182 carries the pairing root; 221 is registered but has no such branch.
    assert!(has_root_branch(DisplayModeRoot::MultiDevicePairing, 182));
    assert!(!has_root_branch(DisplayModeRoot::MultiDevicePairing, 221));
}

//! PID756 supplies its source-specific active rows to the shared compiler.
use super::*;

pub(super) fn gamepad_rows(profile: &Value) -> anyhow::Result<Option<Vec<Value>>> {
    crate::keyboard_firmware_mapping::gamepad_rows(
        profile,
        active_mappings(profile)?,
        source()["full_mapping_list"]
            .as_array()
            .context("756 source FULLMAPPINGLIST is absent")?,
    )
}

pub(super) use crate::keyboard_firmware_mapping::snap_tap;

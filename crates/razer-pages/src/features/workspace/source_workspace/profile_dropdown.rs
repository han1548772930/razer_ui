//! Current679/756 profile datasets keep local identity independent of OBM.
//! Current679 UI createProfileDataSet groups only matching observed GUIDs;
//! its local names use module35380.Ye before that grouping.
use super::{Choice, Device};
use razer_device::keyboard_profile_inventory::{ProfileMetadataObservation, ProfileSlot};
use razer_widgets::synapse_select::SelectOptionDecoration;
use std::collections::BTreeMap;

fn source_groups(product_id: u32) -> bool {
    matches!(product_id, 679 | 756)
}

fn observed_slots(slots: Option<&[ProfileSlot]>) -> BTreeMap<&str, u8> {
    slots
        .into_iter()
        .flatten()
        .filter_map(|slot| match &slot.metadata {
            ProfileMetadataObservation::Observed { identity, .. } => {
                Some((identity.guid.as_str(), slot.slot_id))
            }
            ProfileMetadataObservation::SourceSkippedFirstSlot => None,
        })
        .collect()
}

pub(super) fn choices(
    device: &Device,
    slots: Option<&[ProfileSlot]>,
) -> anyhow::Result<Vec<Choice>> {
    if !source_groups(device.product_id) {
        return Ok(device
            .profiles
            .iter()
            .map(|profile| Choice::new(&profile.id, &profile.name))
            .collect());
    }
    if let Some(slots) = slots {
        razer_device::keyboard_profile_inventory::validate_slots(device.product_id, slots)?;
    }
    let locale = razer_platform::system::collation_locale()?;
    let order = razer_device::source_string::ProfileNameOrder::try_new(&locale)?;
    let mut local = device.profiles.iter().collect::<Vec<_>>();
    local.sort_by(|left, right| order.compare(&left.name, &right.name));
    let slots = observed_slots(slots);
    // Stable sorting preserves Ye order within the local group and at equal
    // slot IDs. A slot's last repeated GUID is the source Map's retained row.
    local.sort_by_key(|profile| slots.get(profile.guid.as_str()).copied());
    local.sort_by_key(|profile| !slots.contains_key(profile.guid.as_str()));
    Ok(local
        .into_iter()
        .map(|profile| Choice::new(&profile.id, &profile.name))
        .collect())
}

pub(super) fn decorations(
    device: &Device,
    slots: Option<&[ProfileSlot]>,
    choices: &[Choice],
) -> BTreeMap<String, SelectOptionDecoration> {
    if !source_groups(device.product_id) {
        return BTreeMap::new();
    }
    let slots = observed_slots(slots);
    let mut saw_onboard = false;
    let mut saw_local = false;
    let mut result = BTreeMap::new();
    for choice in choices {
        let Some(profile) = device.profiles.iter().find(|profile| profile.id == choice.id()) else {
            continue;
        };
        let mut decoration = SelectOptionDecoration::new();
        if let Some(slot) = slots.get(profile.guid.as_str()) {
            saw_onboard = true;
            let image = match slot {
                1 | 2 => Some("synapse/profile-slot-factory.svg"),
                3 => Some("synapse/profile-slot-1.svg"),
                4 => Some("synapse/profile-slot-2.svg"),
                5 => Some("synapse/profile-slot-3.svg"),
                6 => Some("synapse/profile-slot-4.svg"),
                _ => None,
            };
            if let Some(image) = image {
                decoration = decoration.prefix_image(image, 10., 10.);
            }
        } else if !saw_local {
            saw_local = true;
            decoration = decoration.separator_before(saw_onboard);
        }
        let tip = match profile.guid.as_str() {
            "af06d371-861f-4b98-8d78-bfaef5cfdebf" => Some("PRESET_PROFILES_MINI_CONTENT_1_HEAD"),
            "d363ba48-79a8-422c-b405-29a07eff7e8e" => Some("PRESET_PROFILES_MINI_CONTENT_3_HEAD"),
            "2c481cef-62e0-4fe6-85a1-4d134c279c23" => Some("PRESET_PROFILES_MINI_CONTENT_4_HEAD"),
            "474c90e3-7eab-4aa0-8030-34c43cbc707a" => Some("PRESET_PROFILES_MINI_CONTENT_5_HEAD"),
            "177a20d5-f30a-4a52-923e-4fadb7db8392" => Some("PRESET_PROFILES_MINI_CONTENT_6_HEAD"),
            _ => None,
        };
        if let Some(tip) = tip {
            decoration = decoration.tooltip_icon("synapse/profile-preset-info.svg", razer_i18n::t(tip));
        }
        result.insert(profile.id.clone(), decoration);
    }
    result
}

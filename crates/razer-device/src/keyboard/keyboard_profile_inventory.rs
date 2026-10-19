//! Current keyboard OBM identity reads. The caller owns the physical session,
//! transaction counter, cancellation guard and serialization with other I/O.
use crate::{
    backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange,
    keyboard_metadata,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

/// Current product registrations that actually mount `isObm` over HID. Merely
/// shipping the shared OBM chunk does not establish that a product mounts it.
const OBM_PRODUCTS: &[u32] = &[
    550, 551, 552, 577, 579, 590, 599, 600, 602, 614, 617, 619, 620, 642, 647, 653, 658, 661, 664,
    677, 678, 679, 688, 691, 697, 708, 716, 717, 719, 720, 721, 724, 727, 728, 739, 740, 741, 742,
    746, 747, 752, 756,
];

pub fn supports(product_id: u32) -> bool {
    OBM_PRODUCTS.contains(&product_id)
}

fn analog_v3(product_id: u32) -> bool {
    matches!(
        product_id,
        678 | 679 | 688 | 719 | 720 | 721 | 728 | 740 | 741 | 742 | 746 | 747 | 756
    )
}

fn validate_capability(cap: &DeviceReadCapability) -> anyhow::Result<()> {
    ensure!(
        supports(cap.product_id) && keyboard_metadata::capability(cap.product_id).is_some(),
        "Keyboard product does not mount the current HID OBM reader"
    );
    ensure!(
        matches!(
            cap.source_class.as_str(),
            "rzDevice25" | "rzDevice25DualLinkKeyboard" | "rzDevice25Oled"
        ),
        "Keyboard OBM read transport is not a verified HID factory"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuidStatus {
    Recognized,
    Unrecognized,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileIdentity {
    pub guid: String,
    pub guid_status: GuidStatus,
    pub name: String,
    pub owner: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MappingHashObservation {
    Unobserved,
    Observed([u8; 6]),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileMetadataObservation {
    /// The ordinary engine skips metadata for its first returned list entry.
    /// Its ID is observed; its name, owner and GUID remain unobserved.
    SourceSkippedFirstSlot,
    Observed {
        identity: ProfileIdentity,
        mapping_hash: MappingHashObservation,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSlot {
    pub slot_id: u8,
    pub metadata: ProfileMetadataObservation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileInventory {
    pub max_profiles_supported: u8,
    pub declared_profile_count: u8,
    pub slots: Vec<ProfileSlot>,
}

impl ProfileInventory {
    pub fn validate(&self, product_id: u32) -> anyhow::Result<()> {
        ensure!(supports(product_id), "Keyboard does not mount HID OBM");
        ensure!(
            self.slots.len() <= 79 && self.slots.len() == usize::from(self.declared_profile_count),
            "Keyboard profile inventory count differs"
        );
        let mut ids = std::collections::BTreeSet::new();
        for (index, slot) in self.slots.iter().enumerate() {
            ensure!(
                slot.slot_id > 0 && ids.insert(slot.slot_id),
                "Keyboard profile inventory IDs are zero or repeated"
            );
            match &slot.metadata {
                ProfileMetadataObservation::SourceSkippedFirstSlot => ensure!(
                    index == 0 && !analog_v3(product_id),
                    "Keyboard profile metadata was skipped outside its source branch"
                ),
                ProfileMetadataObservation::Observed {
                    identity,
                    mapping_hash,
                } => {
                    ensure!(
                        index != 0 || analog_v3(product_id),
                        "Keyboard first-slot metadata is not observed by this source"
                    );
                    ensure!(
                        identity.guid.len() == 36
                            && identity.guid.bytes().enumerate().all(|(index, byte)| {
                                if matches!(index, 8 | 13 | 18 | 23) {
                                    byte == b'-'
                                } else {
                                    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
                                }
                            })
                            && identity.guid_status == guid_status(&identity.guid),
                        "Keyboard metadata GUID/status differs from its decoded bytes"
                    );
                    ensure!(
                        matches!(mapping_hash, MappingHashObservation::Observed(_))
                            == matches!(product_id, 747 | 756),
                        "Keyboard mapping hash observation differs from its metadata schema"
                    );
                }
            }
        }
        Ok(())
    }
}

/// A separate device observation. The source's inventory loader does not call
/// this getter, and a returned slot is not a local/UI active-profile GUID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveProfileReading {
    pub slot_id: u8,
}

pub(crate) fn guid_text(bytes: &[u8; 16]) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        u32::from_le_bytes(bytes[..4].try_into().expect("GUID field length")),
        u16::from_le_bytes(bytes[4..6].try_into().expect("GUID field length")),
        u16::from_le_bytes(bytes[6..8].try_into().expect("GUID field length")),
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn guid_status(guid: &str) -> GuidStatus {
    if guid.contains("ffff") || guid.contains("0000") {
        GuidStatus::Unrecognized
    } else {
        GuidStatus::Recognized
    }
}

/// `getProfileNameStructure`: mixed-endian GUID, UTF-8 name and a separate
/// byte-string owner. A missing NUL follows JS `slice(0, -1)` exactly.
pub fn decode_identity(bytes: &[u8]) -> anyhow::Result<ProfileIdentity> {
    ensure!(
        bytes.len() == 250,
        "Keyboard profile metadata is incomplete"
    );
    let guid = guid_text(bytes[..16].try_into()?);
    let name = &bytes[16..116];
    let length = name.iter().position(|byte| *byte == 0).unwrap_or(99);
    let owner = bytes[116..180]
        .iter()
        .copied()
        .map(char::from)
        .collect::<String>();
    Ok(ProfileIdentity {
        guid_status: guid_status(&guid),
        guid,
        name: String::from_utf8_lossy(&name[..length]).into_owned(),
        owner: owner.trim_matches('\0').to_owned(),
    })
}

/// Shared with the latest keyboard restore inventory. The ten attempts and
/// doubled IN timing belong to `getProfileNameStructure`, not a generic retry
/// policy. Cancellation never enters another attempt.
pub(crate) fn read_raw_metadata_hid(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    slot_id: u8,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<[u8; 250]> {
    ensure!(slot_id > 0, "Keyboard metadata slot ID is zero");
    let mut extended = cap.clone();
    extended.sleep_between_out_in_ms = extended
        .sleep_between_out_in_ms
        .checked_mul(2)
        .context("Keyboard metadata delay exceeds its range")?;
    extended.sleep_between_in_ms = extended
        .sleep_between_in_ms
        .max(extended.sleep_between_out_in_ms);
    let mut last_error = None;
    for attempt in 0..10 {
        validate()?;
        let mut bytes = Vec::with_capacity(250);
        for offset in (0u16..250).step_by(64) {
            let length = usize::from((250 - offset).min(64));
            let [high, low] = offset.to_be_bytes();
            let chunk = feature_exchange::exchange_with_report_data_size(
                device,
                &extended,
                [80, 5, 136],
                &[slot_id, high, low, 0, 0],
                next()?,
                (length + 5) as u8,
                validate,
            );
            match chunk {
                Ok(chunk) => {
                    ensure!(
                        chunk.len() >= length + 5
                            && chunk[0] == slot_id
                            && chunk[1..3] == [high, low],
                        "Keyboard profile metadata identity/length differs"
                    );
                    bytes.extend_from_slice(&chunk[5..5 + length]);
                }
                Err(error) => {
                    validate()?;
                    last_error = Some(error);
                    bytes.clear();
                    break;
                }
            }
        }
        if bytes.len() == 250 {
            let bytes: [u8; 250] = bytes.try_into().expect("Complete metadata length");
            let guid = guid_text(bytes[..16].try_into()?);
            if guid_status(&guid) == GuidStatus::Recognized || attempt == 9 {
                validate()?;
                return Ok(bytes);
            }
            // The source returns its final actual result even when the GUID
            // remains unknown. It must not turn into a local/default profile.
            last_error = None;
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Keyboard metadata was not observed")))
        .context("Keyboard profile metadata reads exhausted")
}

pub fn read_hid(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<ProfileInventory> {
    validate_capability(cap)?;
    validate()?;
    let maximum = feature_exchange::exchange(device, cap, [1, 5, 138], &[], next()?, validate)?;
    let max_profiles_supported = *maximum
        .first()
        .context("Keyboard maximum profile count was not returned")?;
    let count = feature_exchange::exchange(device, cap, [1, 5, 128], &[], next()?, validate)?;
    let declared_profile_count = *count
        .first()
        .context("Keyboard profile count was not returned")?;
    let list = feature_exchange::exchange(device, cap, [80, 5, 129], &[], next()?, validate)?;
    let listed_count = usize::from(*list.first().context("Keyboard profile list is empty")?);
    ensure!(
        listed_count <= 79
            && list.len() > listed_count
            && listed_count == usize::from(declared_profile_count),
        "Keyboard profile count/list length differs"
    );
    let ids = &list[1..1 + listed_count];
    ensure!(
        ids.iter().all(|id| *id > 0)
            && ids.iter().collect::<std::collections::BTreeSet<_>>().len() == ids.len(),
        "Keyboard profile slot IDs are zero or repeated"
    );
    let mut slots = Vec::with_capacity(ids.len());
    for (index, &slot_id) in ids.iter().enumerate() {
        let metadata = if index == 0 && !analog_v3(cap.product_id) {
            ProfileMetadataObservation::SourceSkippedFirstSlot
        } else {
            let bytes = read_raw_metadata_hid(device, cap, slot_id, &mut next, validate)?;
            ProfileMetadataObservation::Observed {
                identity: decode_identity(&bytes)?,
                mapping_hash: if matches!(cap.product_id, 747 | 756) {
                    MappingHashObservation::Observed(bytes[244..250].try_into()?)
                } else {
                    MappingHashObservation::Unobserved
                },
            }
        };
        slots.push(ProfileSlot { slot_id, metadata });
    }
    validate()?;
    let inventory = ProfileInventory {
        max_profiles_supported,
        declared_profile_count,
        slots,
    };
    inventory.validate(cap.product_id)?;
    Ok(inventory)
}

pub fn read_active_hid(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<ActiveProfileReading> {
    validate_capability(cap)?;
    let bytes = feature_exchange::exchange(device, cap, [1, 5, 132], &[], next()?, validate)?;
    let slot_id = *bytes
        .first()
        .context("Keyboard active profile ID was not returned")?;
    validate()?;
    Ok(ActiveProfileReading { slot_id })
}

#[cfg(test)]
#[path = "keyboard_profile_inventory_tests.rs"]
mod tests;

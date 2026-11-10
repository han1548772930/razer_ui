//! Current keypad keymap storage. The caller owns the physical transport,
//! transaction sequence, cancellation guard and connection lifetime.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

pub(crate) const SET_TARGET: [u8; 3] = [1, 2, 55];
const GET_TARGET: [u8; 3] = [1, 2, 183];
const GET_INVENTORY: [u8; 3] = [1, 2, 178];
const GET_ACTIVE: [u8; 3] = [2, 2, 182];
const SET_ACTIVE: [u8; 3] = [2, 2, 54];
const ADD: [u8; 3] = [2, 2, 51];
const REMOVE: [u8; 3] = [2, 2, 52];
const GET_NAME: [u8; 3] = [80, 2, 181];
const SET_NAME: [u8; 3] = [80, 2, 53];
const NAME_BYTES: usize = 120;
const NAME_CHUNK: usize = 64;
const NAME_ATTEMPTS: usize = 10;

pub(crate) fn validate_capability(cap: &DeviceReadCapability) -> Result<()> {
    ensure!(
        cap.product_id == 747
            && cap.source_class == "rzDevice25"
            && cap.report_bytes == 91
            && cap.report_id == 0
            && cap.transaction_prefix == 0
            && cap.transaction_modulus == 31,
        "Keymap storage requires the current keypad Feature transport"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AddressFields")]
pub struct Address {
    profile_id: u8,
    keymap_id: u8,
}

#[derive(Deserialize)]
struct AddressFields {
    profile_id: u8,
    keymap_id: u8,
}

impl TryFrom<AddressFields> for Address {
    type Error = anyhow::Error;
    fn try_from(fields: AddressFields) -> Result<Self> {
        Self::new(fields.profile_id, fields.keymap_id)
    }
}

impl Address {
    pub fn new(profile_id: u8, keymap_id: u8) -> Result<Self> {
        let address = Self {
            profile_id,
            keymap_id,
        };
        address.validate()?;
        Ok(address)
    }

    pub fn profile_id(self) -> u8 {
        self.profile_id
    }
    pub fn keymap_id(self) -> u8 {
        self.keymap_id
    }

    fn validate(self) -> Result<()> {
        ensure!(
            self.profile_id > 0 && self.keymap_id > 0,
            "Keymap address is zero"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    profile_id: u8,
    total_enabled: u8,
    keymap_ids: Vec<u8>,
}

impl Inventory {
    pub fn profile_id(&self) -> u8 {
        self.profile_id
    }
    pub fn total_enabled(&self) -> u8 {
        self.total_enabled
    }
    pub fn keymap_ids(&self) -> &[u8] {
        &self.keymap_ids
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Name {
    address: Address,
    guid: String,
    name: String,
    raw: Vec<u8>,
}

impl Name {
    pub fn address(&self) -> Address {
        self.address
    }
    pub fn guid(&self) -> &str {
        &self.guid
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    Add,
    Remove,
    Activate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NameWriteProgress {
    Attempted { offset: usize, length: usize },
    Acknowledged { offset: usize, data: Vec<u8> },
}

pub fn read_inventory(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    profile_id: u8,
    transaction: u8,
    guard: &impl Fn() -> Result<()>,
) -> Result<Inventory> {
    validate_capability(cap)?;
    ensure!(profile_id > 0, "Keymap profile ID is zero");
    let data = feature_exchange::exchange(
        device,
        cap,
        GET_INVENTORY,
        &[profile_id],
        transaction,
        guard,
    )?;
    ensure!(
        data.len() >= 2 && data[0] == profile_id,
        "Keymap inventory response has a different profile or is truncated"
    );
    let total_enabled = data[1];
    let end = data.len().min(2 + usize::from(total_enabled));
    // The source keeps a short nonempty list; it does not invent the missing IDs.
    Ok(Inventory {
        profile_id,
        total_enabled,
        keymap_ids: data[2..end].to_vec(),
    })
}

pub fn read_active(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    profile_id: u8,
    transaction: u8,
    guard: &impl Fn() -> Result<()>,
) -> Result<u8> {
    validate_capability(cap)?;
    ensure!(profile_id > 0, "Keymap profile ID is zero");
    let data = feature_exchange::exchange(
        device,
        cap,
        GET_ACTIVE,
        &[profile_id, 0],
        transaction,
        guard,
    )?;
    ensure!(
        data.len() >= 2 && data[0] == profile_id,
        "Active keymap response has a different profile or is truncated"
    );
    Ok(data[1])
}

pub fn read_target(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    transaction: u8,
    guard: &impl Fn() -> Result<()>,
) -> Result<u8> {
    validate_capability(cap)?;
    let data = feature_exchange::exchange(device, cap, GET_TARGET, &[], transaction, guard)?;
    data.first()
        .copied()
        .context("Target keymap response is truncated")
}

pub(crate) fn select_target_observing_send(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    keymap_id: u8,
    transaction: u8,
    guard: &impl Fn() -> Result<()>,
    sent: &mut impl FnMut(),
) -> Result<Vec<u8>> {
    validate_capability(cap)?;
    ensure!(keymap_id > 0, "Target keymap ID is zero");
    feature_exchange::exchange_with_report_data_size_observing_send(
        device,
        cap,
        SET_TARGET,
        &[keymap_id],
        transaction,
        1,
        guard,
        sent,
    )
}

/// A successful response acknowledges the requested operation. Its data is
/// retained verbatim and does not become an independent active-keymap read.
pub fn update(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    address: Address,
    operation: Update,
    transaction: u8,
    guard: &impl Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    validate_capability(cap)?;
    address.validate()?;
    let command = match operation {
        Update::Add => ADD,
        Update::Remove => REMOVE,
        Update::Activate => SET_ACTIVE,
    };
    feature_exchange::exchange(
        device,
        cap,
        command,
        &[address.profile_id, address.keymap_id],
        transaction,
        guard,
    )
}

fn metadata_capability(cap: &DeviceReadCapability) -> Result<DeviceReadCapability> {
    validate_capability(cap)?;
    let extended = cap
        .sleep_between_out_in_ms
        .checked_mul(2)
        .context("Keymap extended delay overflow")?;
    let mut result = cap.clone();
    result.sleep_between_out_in_ms = result.sleep_between_out_in_ms.max(extended);
    result.sleep_between_in_ms = result.sleep_between_in_ms.max(extended);
    Ok(result)
}

fn retryable(error: &anyhow::Error, guard: &impl Fn() -> Result<()>) -> Result<bool> {
    if crate::backend::exchange_interrupted(error) {
        return Ok(false);
    }
    guard()?;
    Ok(true)
}

pub(crate) fn next_transaction(
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
) -> Result<u8> {
    guard().and_then(|_| next()).map_err(|error| {
        crate::backend::interrupt_exchange(error, crate::backend::ExchangeInterruption::Validation)
    })
}

pub fn read_name(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    address: Address,
    mut next: impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
) -> Result<Name> {
    address.validate()?;
    let cap = metadata_capability(cap)?;
    let mut last_error = anyhow::anyhow!("Keymap name was not read");
    let mut last_observed = None;
    for _ in 0..NAME_ATTEMPTS {
        let attempt = (|| {
            let mut total = NAME_BYTES;
            let mut offset = 0;
            let mut bytes = Vec::with_capacity(total);
            while offset < total {
                let count = NAME_CHUNK.min(total - offset);
                let request = [
                    address.profile_id,
                    address.keymap_id,
                    offset as u8,
                    total as u8,
                ];
                let data = feature_exchange::exchange_with_report_data_size(
                    device,
                    &cap,
                    GET_NAME,
                    &request,
                    next_transaction(&mut next, guard)?,
                    (count + 4) as u8,
                    guard,
                )?;
                ensure!(
                    data.len() >= 4 && data[..3] == request[..3],
                    "Keymap name response has a different address or offset"
                );
                if data[3] > 0 {
                    total = NAME_BYTES.min(usize::from(data[3]));
                }
                bytes.extend_from_slice(&data[4..data.len().min(count + 4)]);
                offset += count;
            }
            ensure!(
                total >= 16 && bytes.len() >= total,
                "Keymap name content is truncated"
            );
            bytes.truncate(total);
            let guid = crate::onboard_macro::guid_text(&bytes[..16])?;
            let name = crate::onboard_macro::source_text(&bytes[16..bytes.len().min(116)], false);
            Ok(Name {
                address,
                guid,
                name,
                raw: bytes,
            })
        })();
        match attempt {
            Ok(name) if !name.name.is_empty() => return Ok(name),
            Ok(name) => last_observed = Some(name),
            Err(error) => {
                if !retryable(&error, guard)? {
                    return Err(error);
                }
                last_error = error;
            }
        }
    }
    // Source keeps a completed empty-name reading across later failed retries.
    // It belongs to this attempt's device session, never local profile data.
    last_observed.ok_or(last_error)
}

pub fn name_bytes(guid: &str, name: &str) -> Result<[u8; NAME_BYTES]> {
    let mut bytes = [0; NAME_BYTES];
    // Current keypad preset IDs include short, sometimes odd-length hex
    // identities. convertGuidToFWByte removes hyphens and zero-fills absent
    // nibbles before applying the ordinary mixed-endian GUID field encoding.
    let hex: String = guid.chars().filter(|value| *value != '-').collect();
    ensure!(
        !hex.is_empty() && hex.len() <= 32 && hex.bytes().all(|value| value.is_ascii_hexdigit()),
        "Keymap GUID is not a nonempty source hexadecimal identity"
    );
    let hex = format!("{hex:0<32}");
    let canonical = format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    );
    bytes[..16].copy_from_slice(&crate::onboard_macro::guid_bytes(&canonical)?);
    let length = name.len().min(99);
    bytes[16..16 + length].copy_from_slice(&name.as_bytes()[..length]);
    Ok(bytes)
}

/// Writes one complete source name structure. Cancellation or failure can
/// follow acknowledged chunks; the caller retains that physical write scope.
pub fn write_name(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    address: Address,
    bytes: &[u8; NAME_BYTES],
    mut next: impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
    progress: &mut impl FnMut(NameWriteProgress),
) -> Result<()> {
    address.validate()?;
    let cap = metadata_capability(cap)?;
    let mut failure = None;
    for _ in 0..NAME_ATTEMPTS {
        let attempt = (|| {
            for offset in (0..NAME_BYTES).step_by(NAME_CHUNK) {
                let end = NAME_BYTES.min(offset + NAME_CHUNK);
                let mut request = vec![
                    address.profile_id,
                    address.keymap_id,
                    offset as u8,
                    NAME_BYTES as u8,
                ];
                request.extend_from_slice(&bytes[offset..end]);
                let result = feature_exchange::exchange_with_report_data_size_observing_send(
                    device,
                    &cap,
                    SET_NAME,
                    &request,
                    next_transaction(&mut next, guard)?,
                    request.len() as u8,
                    guard,
                    &mut || {
                        progress(NameWriteProgress::Attempted {
                            offset,
                            length: end - offset,
                        })
                    },
                );
                if let Some(data) = feature_exchange::acknowledged_reply(&result) {
                    progress(NameWriteProgress::Acknowledged {
                        offset,
                        data: data.to_vec(),
                    });
                }
                result?;
            }
            Ok(())
        })();
        match attempt {
            Ok(()) => return Ok(()),
            Err(error) => {
                if !retryable(&error, guard)? {
                    return Err(error);
                }
                failure = Some(error);
            }
        }
    }
    Err(failure.context("Keymap name write had no completed attempt")?)
}

#[cfg(test)]
#[path = "keyboard_onboard_keymap_tests.rs"]
mod tests;

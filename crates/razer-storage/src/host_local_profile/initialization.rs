//! Current MW77846.w/N/A and MW60763.w/b/T serial-cache mutation. The
//! subsequent device restoration belongs to the connection/task owner.
use super::{
    InitializationOrigin, LocalProfiles, Ownership, Profile, SerialInitialization, SourceSelection,
};
use anyhow::{Context as _, Result, ensure};
use razer_model::model::Device;

#[derive(Clone, Copy, Debug)]
pub struct SerialInitializationContext<'a> {
    pub edition_id: u32,
    pub computer_name: &'a str,
    pub device_name: Option<&'a str>,
    pub new_guid: &'a str,
}

/// A checked local-cache mutation prepared from an actual successful read.
/// Applying it changes local profile fields only. The source's device restore
/// tasks and their responses remain separate from this receipt.
#[derive(Clone, Debug)]
pub struct SerialInitializationPlan {
    catalog: LocalProfiles,
    reason: SerialInitialization,
}

impl SerialInitializationPlan {
    pub fn prepare(
        source: &LocalProfiles,
        context: SerialInitializationContext<'_>,
    ) -> Result<Self> {
        let reason = match source.selection() {
            SourceSelection::NeedsSerialInitialization { reason } => *reason,
            SourceSelection::SerialMetadata { .. } => {
                anyhow::bail!("serial already has a selected source profile")
            }
        };
        ensure!(
            valid_uuid_v4(context.new_guid),
            "serial initialization requires a UUID v4"
        );
        let base = format!("{}-Default", context.computer_name);
        let mut name = base.clone();
        // Source AM/W$ probes the base and suffixes 1..99, then returns 100.
        for suffix in 1..=100 {
            if !source
                .profiles()
                .iter()
                .any(|profile| profile.name() == name)
            {
                break;
            }
            name = format!("{base} {suffix}");
        }
        let profile = razer_device::mouse_current_profile::new_profile(
            source.product_id(),
            context.edition_id,
            context.new_guid,
            &name,
        )?;
        let device_name =
            context
                .device_name
                .unwrap_or(razer_device::mouse_current_profile::source_device_name(
                    source.product_id(),
                )?);
        let (baseline_sequence, baseline_sha256) = source.revision();
        let origin = InitializationOrigin {
            product_id: source.product_id(),
            serial: source.serial().to_owned(),
            guid: context.new_guid.to_owned(),
            edition_id: context.edition_id,
            device_name: device_name.to_owned(),
            reason,
            baseline_sequence,
            baseline_sha256: baseline_sha256.to_owned(),
            extensions: std::collections::BTreeMap::new(),
        };
        Ok(Self {
            catalog: source.with_initialized(profile, origin, device_name)?,
            reason,
        })
    }

    pub fn reason(&self) -> SerialInitialization {
        self.reason
    }

    pub fn catalog(&self) -> &LocalProfiles {
        &self.catalog
    }

    pub fn new_profile(&self) -> &Profile {
        self.catalog
            .active_profile()
            .expect("checked serial initialization profile")
    }

    pub fn apply(&self, device: &mut Device, baseline: &Device) -> Result<bool> {
        super::import(device, baseline, &self.catalog)
    }
}

fn valid_uuid_v4(guid: &str) -> bool {
    guid.len() == 36
        && guid.bytes().enumerate().all(|(index, value)| match index {
            8 | 13 | 18 | 23 => value == b'-',
            14 => value == b'4',
            19 => matches!(value.to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b'),
            _ => value.is_ascii_hexdigit(),
        })
}

/// The vendor database remains read-only. Re-observing unchanged bytes must
/// retain a locally initialized profile instead of creating another one.
pub(super) fn retain_initialized(
    source: &LocalProfiles,
    device: &Device,
    ownership: &Ownership,
) -> Result<Option<LocalProfiles>> {
    let initialized = ownership
        .profiles
        .iter()
        .filter(|(_, entry)| entry.serial == source.serial())
        .filter_map(|(id, entry)| entry.initialized.as_ref().map(|origin| (id, origin)))
        .collect::<Vec<_>>();
    ensure!(
        initialized.len() <= 1,
        "serial has multiple local initialization owners"
    );
    let Some((id, origin)) = initialized.first().copied() else {
        return Ok(None);
    };
    if let Some(prepared) = source
        .profiles()
        .iter()
        .find_map(|profile| source.initialized(profile))
    {
        ensure!(
            prepared.product_id == origin.product_id
                && prepared.serial == origin.serial
                && prepared.guid == origin.guid
                && prepared.edition_id == origin.edition_id
                && prepared.reason == origin.reason
                && prepared.baseline_sha256 == origin.baseline_sha256,
            "serial was initialized by another local operation"
        );
        return Ok(None);
    }
    if source
        .profiles()
        .iter()
        .any(|profile| profile.guid() == origin.guid)
    {
        return Ok(None);
    }
    ensure!(
        matches!(
            source.selection(),
            SourceSelection::NeedsSerialInitialization { .. }
        ) && source.revision().1 == origin.baseline_sha256,
        "source mouse profile revision changed after local serial initialization"
    );
    let local = device
        .profiles
        .iter()
        .find(|profile| profile.id == *id && profile.guid == origin.guid)
        .context("locally initialized serial profile was removed or replaced")?;
    let profile: Profile = serde_json::from_value(
        local
            .source_settings
            .clone()
            .context("locally initialized profile settings are absent")?,
    )
    .context("locally initialized mouse profile settings are malformed")?;
    ensure!(
        profile.guid() == origin.guid,
        "locally initialized profile settings have another GUID"
    );
    Ok(Some(source.with_initialized(
        profile,
        origin.clone(),
        &origin.device_name,
    )?))
}

//! Pure connection epochs for explicit THX render setup.
//!
//! Callers supply identities from real, unique live routes. A discovery scan
//! is not a render-ready acknowledgement, a profile restore, or a device write.
//! An unknown scan retains the seen set so that a later unchanged scan cannot
//! silently retry initialization whose writes may already have reached a device.
use crate::{
    thx_blackshark_firmware::blackshark_firmware_route, thx_submission::ThxSubmissionScope,
};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

/// A full observed physical route, independent of page/profile metadata.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThxConnectionIdentity {
    pub product_id: u32,
    /// Preserve the observed spelling; do not invent or fold serial identities.
    pub serial_number: String,
    /// OS ContainerId comparison is ASCII case-insensitive.
    pub container_id: String,
    pub physical_product_id: u16,
    /// Preserve the raw HID collection bytes rather than selecting by PID alone.
    pub hid_path: Vec<u8>,
}

impl ThxConnectionIdentity {
    pub fn new(
        product_id: u32,
        serial_number: impl Into<String>,
        container_id: impl Into<String>,
        physical_product_id: u16,
        hid_path: impl Into<Vec<u8>>,
    ) -> Result<Self> {
        let identity = Self {
            product_id,
            serial_number: serial_number.into(),
            container_id: container_id.into().to_ascii_lowercase(),
            physical_product_id,
            hid_path: hid_path.into(),
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn validate(&self) -> Result<()> {
        // Reuse the existing supported-owner, ContainerId and text boundaries.
        // The empty profile is intentional: this is a physical connection key.
        ThxSubmissionScope {
            product_id: self.product_id,
            container_id: self.container_id.clone(),
            serial_number: self.serial_number.clone(),
            profile_id: String::new(),
            generation: 0,
        }
        .validate()?;
        ensure!(
            self.container_id == self.container_id.to_ascii_lowercase(),
            "THX connection ContainerId is not normalized"
        );
        blackshark_firmware_route(self.product_id, self.physical_product_id)?;
        // Rust resource/string boundary, not a claimed vendor HID path limit.
        ensure!(
            !self.hid_path.is_empty() && self.hid_path.len() <= 4096 && !self.hid_path.contains(&0),
            "THX connection HID collection path is empty, too large or contains NUL"
        );
        Ok(())
    }
}

/// A new observed connection; this does not assert that render setup succeeded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThxConnectionEpoch {
    pub identity: ThxConnectionIdentity,
    pub epoch: u64,
}

#[derive(Debug, Default)]
pub struct ThxRenderDiscoveryTracker {
    connections: BTreeMap<ThxConnectionIdentity, u64>,
    last_epoch: u64,
}

impl ThxRenderDiscoveryTracker {
    /// Return only new confirmed identities, each with a never-reused epoch.
    ///
    /// `None` is unknown/incomplete discovery and leaves all known keys intact.
    /// `Some` is the complete confirmed eligible set: truly absent keys expire,
    /// duplicates collapse, and unchanged keys retain their epochs. Callers must
    /// build validated identities and classify incomplete/ambiguous scans as
    /// unknown rather than publishing a guessed empty or partial eligible set.
    ///
    /// Cancelling or failing initialization does not call this method with an
    /// invented absence. Retry policy belongs to the explicit lifecycle caller.
    pub fn observe(
        &mut self,
        identities: Option<&[ThxConnectionIdentity]>,
    ) -> Vec<ThxConnectionEpoch> {
        let Some(identities) = identities else {
            return Vec::new();
        };
        let incoming = identities.iter().cloned().collect::<BTreeSet<_>>();
        self.connections
            .retain(|identity, _| incoming.contains(identity));
        let mut added = Vec::new();
        for identity in incoming {
            if self.connections.contains_key(&identity) {
                continue;
            }
            // Fail closed rather than wrapping into an earlier connection.
            self.last_epoch = self
                .last_epoch
                .checked_add(1)
                .expect("THX connection epoch exhausted");
            self.connections.insert(identity.clone(), self.last_epoch);
            added.push(ThxConnectionEpoch {
                identity,
                epoch: self.last_epoch,
            });
        }
        added
    }

    pub fn contains(&self, identity: &ThxConnectionIdentity) -> bool {
        self.connections.contains_key(identity)
    }

    pub fn current_epoch(&self, identity: &ThxConnectionIdentity) -> Option<ThxConnectionEpoch> {
        self.connections
            .get(identity)
            .map(|&epoch| ThxConnectionEpoch {
                identity: identity.clone(),
                epoch,
            })
    }
}

#[cfg(test)]
#[path = "thx_lifecycle_discovery_tests.rs"]
mod tests;

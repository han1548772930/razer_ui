//! Correlated rotation operations on the shared physical mapping-driver owner.
use anyhow::ensure;
use razer_device::mapping_native_rotation::RotationReply;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTarget {
    product_id: u32,
    container_id: String,
}
impl NativeTarget {
    pub fn new(product_id: u32, container_id: String) -> anyhow::Result<Self> {
        let target = Self {
            product_id,
            container_id,
        };
        target.validate()?;
        Ok(target)
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.product_id > 0 && self.product_id <= u16::MAX.into(),
            "Rotation physical product ID is invalid"
        );
        let container = uuid::Uuid::parse_str(self.container_id.trim_matches(['{', '}']))?;
        ensure!(
            !container.is_nil(),
            "Rotation requires an observed device container"
        );
        Ok(())
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn container_id(&self) -> &str {
        &self.container_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeRegistration {
    owner_guid: String,
    registered: bool,
}
impl NativeRegistration {
    pub fn new(owner_guid: String, registered: bool) -> Self {
        Self {
            owner_guid,
            registered,
        }
    }
    pub fn matches(&self, owner_guid: &str, registered: bool) -> bool {
        self.owner_guid == owner_guid && self.registered == registered
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reply", rename_all = "snake_case")]
pub enum NativeProgress {
    Pending,
    Completed(RotationReply),
    Failed { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeUpdate {
    owner_guid: String,
    progress: NativeProgress,
}
impl NativeUpdate {
    pub fn new(owner_guid: String, progress: NativeProgress) -> Self {
        Self {
            owner_guid,
            progress,
        }
    }
    pub fn into_progress(self, owner_guid: &str) -> anyhow::Result<NativeProgress> {
        ensure!(
            self.owner_guid == owner_guid,
            "Rotation operation owner changed"
        );
        if let NativeProgress::Failed { reason } = &self.progress {
            ensure!(!reason.is_empty(), "Rotation failure reason is missing");
        }
        Ok(self.progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rotation_wire_requires_observed_identity_and_correlated_completion() {
        for container in [
            "",
            "not-a-container",
            "00000000-0000-0000-0000-000000000000",
        ] {
            assert!(NativeTarget::new(196, container.into()).is_err());
        }
        let valid = "b63bfcb4-c1ee-4b2b-83f3-90fc06229af0";
        assert!(NativeTarget::new(65536, valid.into()).is_err());
        let target = NativeTarget::new(196, valid.into()).unwrap();
        assert_eq!(
            serde_json::to_value(&target).unwrap(),
            json!({"product_id":196,"container_id":valid})
        );
        let pending: NativeUpdate =
            serde_json::from_value(json!({"owner_guid":"current","progress":{"state":"pending"}}))
                .unwrap();
        assert_eq!(
            pending.clone().into_progress("current").unwrap(),
            NativeProgress::Pending
        );
        assert!(pending.into_progress("previous").is_err());
        assert!(
            serde_json::from_value::<NativeUpdate>(
                json!({"owner_guid":"current","progress":{"state":"completed"}})
            )
            .is_err()
        );
        assert!(serde_json::from_value::<NativeUpdate>(json!({"owner_guid":"current"})).is_err());
        assert!(!NativeRegistration::new("old".into(), true).matches("current", true));
        assert!(!NativeRegistration::new("current".into(), false).matches("current", true));
        let failed: NativeUpdate = serde_json::from_value(json!({
            "owner_guid":"current", "progress":{"state":"failed","reply":{"reason":"driver rejected SET"}}
        })).unwrap();
        assert_eq!(
            failed.into_progress("current").unwrap(),
            NativeProgress::Failed {
                reason: "driver rejected SET".into()
            }
        );
        let missing: NativeUpdate = serde_json::from_value(json!({
            "owner_guid":"current", "progress":{"state":"failed","reply":{"reason":""}}
        }))
        .unwrap();
        assert!(missing.into_progress("current").is_err());
    }
}

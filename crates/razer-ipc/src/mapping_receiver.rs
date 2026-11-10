//! A single physical mapping registration may serve multiple logical products.
//! Pairing and document projection stay with the caller that owns the group.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    physical_product_id: u32,
    device_container_id: String,
    members: Vec<Member>,
}

/// A caller's observed route claim. The platform owner verifies it against
/// current physical enumeration and, for radio routes, a live receiver query.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    product_id: u32,
    route: Route,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Route {
    Direct,
    Peer { product_id: u32 },
}

impl Member {
    pub fn direct(product_id: u32) -> Self {
        Self {
            product_id,
            route: Route::Direct,
        }
    }
    pub fn peer(product_id: u32, peer_product_id: u32) -> Self {
        Self {
            product_id,
            route: Route::Peer {
                product_id: peer_product_id,
            },
        }
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn source_product_id(&self) -> u32 {
        match razer_device::mapping_document::MappingProduct::for_product_id(self.product_id) {
            Some(product) => product.source_product_id(),
            None => self.product_id,
        }
    }
    pub fn route(&self) -> Route {
        self.route
    }
}

impl Target {
    pub fn new(
        physical_product_id: u32,
        device_container_id: String,
        members: Vec<Member>,
    ) -> anyhow::Result<Self> {
        let target = Self {
            physical_product_id,
            device_container_id,
            members,
        };
        target.validate()?;
        Ok(target)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (1..=u32::from(u16::MAX)).contains(&self.physical_product_id),
            "Mapping receiver physical identity is invalid"
        );
        anyhow::ensure!(
            !self.device_container_id.is_empty() && !self.device_container_id.contains('\0'),
            "Mapping receiver container is absent"
        );
        let mut products = BTreeSet::new();
        anyhow::ensure!(
            !self.members.is_empty()
                && self.members.iter().all(|member| {
                    let valid = |product| (1..=u32::from(u16::MAX)).contains(&product);
                    valid(member.product_id)
                        && products.insert(member.product_id)
                        && match member.route {
                            Route::Direct => true,
                            Route::Peer { product_id } => valid(product_id),
                        }
                }),
            "Mapping receiver members are absent, duplicated, or invalid"
        );
        Ok(())
    }

    pub fn physical_product_id(&self) -> u32 {
        self.physical_product_id
    }
    pub fn device_container_id(&self) -> &str {
        &self.device_container_id
    }
    pub fn members(&self) -> &[Member] {
        &self.members
    }
    pub fn contains_product(&self, product_id: u32) -> bool {
        self.members
            .iter()
            .any(|member| member.product_id == product_id)
    }
    pub fn contains_source_product(&self, product_id: u32) -> bool {
        self.members
            .iter()
            .any(|member| member.source_product_id() == product_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn physical_identity_and_members_are_distinct_and_required() {
        let valid: Target = serde_json::from_value(json!({
            "physical_product_id":191,
            "device_container_id":"receiver",
            "members":[{"product_id":190,"route":{"kind":"direct"}}]
        }))
        .unwrap();
        valid.validate().unwrap();
        assert_eq!(valid.physical_product_id(), 191);
        assert!(valid.contains_product(190));
        assert!(!valid.contains_product(191));
        for members in [
            vec![],
            vec![Member::direct(190), Member::direct(190)],
            vec![Member::direct(0)],
            vec![Member::peer(190, 65536)],
        ] {
            assert!(Target::new(191, "receiver".into(), members).is_err());
        }
        assert!(Target::new(191, String::new(), vec![Member::direct(190)]).is_err());
        assert!(
            serde_json::from_value::<Target>(
                json!({"physical_product_id":191,"members":[{"product_id":190}]})
            )
            .is_err()
        );
    }
}

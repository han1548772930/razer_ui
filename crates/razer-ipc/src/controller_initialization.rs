//! Identity-checked controller root import response boundary.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::HidNode,
    controller_initialization::{InitializationPlan, InitializationResult},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct InitializationResponse {
    pub node: HidNode,
    pub product_id: u32,
    pub plan: InitializationPlan,
    pub result: InitializationResult,
    pub transport_metadata: Value,
}
impl InitializationResponse {
    pub fn new(
        node: HidNode,
        product_id: u32,
        plan: InitializationPlan,
        result: InitializationResult,
        transport_metadata: Value,
    ) -> anyhow::Result<Self> {
        let source = razer_device::controller_sleep::capability(product_id)
            .context("No current controller importer")?;
        ensure!(
            node.vendor_id == 5426
                && source.accepts(u32::from(node.product_id))
                && !node.path.is_empty(),
            "Controller import collection belongs to another product"
        );
        result.validate_for(product_id, &plan)?;
        Ok(Self {
            node,
            product_id,
            plan,
            result,
            transport_metadata,
        })
    }
    pub fn decode(
        value: Value,
        node: &HidNode,
        product_id: u32,
        plan: &InitializationPlan,
    ) -> anyhow::Result<Self> {
        let reply: Self = serde_json::from_value(value)?;
        ensure!(
            &reply.node == node && reply.product_id == product_id && &reply.plan == plan,
            "Controller initialization response identity differs"
        );
        Self::new(
            reply.node,
            reply.product_id,
            reply.plan,
            reply.result,
            reply.transport_metadata,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::controller_initialization::{Read, RootField};
    fn node() -> HidNode {
        HidNode {
            path: vec![255, 0, 34, 92, 128],
            vendor_id: 5426,
            product_id: 2676,
            usage_page: 1,
            usage: 5,
            interface_number: 1,
            release_number: 1,
            serial_number: Some("controller".into()),
            manufacturer: None,
            product: None,
        }
    }
    #[test]
    fn controller_initialization_ipc_keeps_generic_connection_results_separate_from_cached_engine_evidence()
     {
        use razer_device::controller_initialization::{
            ConnectionFeaturesResult, ProfileDirectory, ProfileName, SocdObservation, SocdRead,
        };
        let directory = ProfileDirectory {
            button_ids: vec![16],
            profile_ids: vec![3],
            active_profile: 3,
            number_of_profiles: 1,
            names: vec![ProfileName {
                slot: 3,
                name: Read::Observed {
                    value: "Real three".into(),
                },
            }],
        };
        let plan = InitializationPlan::connection_features(Some(directory.clone()), vec![3]);
        let result = InitializationResult {
            connection_probe: Read::Unobserved,
            power_saving: Read::Unobserved,
            controller_mode: Read::Unobserved,
            directory: Read::Unobserved,
            profiles: vec![],
            canceled: false,
            connection_features: Some(ConnectionFeaturesResult {
                refreshed_directory: Read::Unobserved,
                socd: vec![SocdRead {
                    slot: 3,
                    result: Read::Observed {
                        value: SocdObservation { raw_option: 255 },
                    },
                }],
                canceled: false,
            }),
        };
        let value = serde_json::to_value(
            InitializationResponse::new(
                node(),
                2676,
                plan.clone(),
                result,
                serde_json::json!({"simulated":true}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(InitializationResponse::decode(value.clone(), &node(), 2676, &plan).is_ok());
        for (pointer, changed) in [
            (
                "/result/connection_features/socd/0/slot",
                serde_json::json!(1),
            ),
            ("/result/connection_features/socd", serde_json::json!([])),
            (
                "/result/connection_features/refreshed_directory",
                serde_json::json!({"status":"observed","value":directory}),
            ),
            (
                "/result/connection_features/canceled",
                serde_json::json!(true),
            ),
            (
                "/result/directory",
                serde_json::json!({"status":"observed","value":directory}),
            ),
            (
                "/result/connection_features/socd/0/result",
                serde_json::json!({"status":"unobserved"}),
            ),
        ] {
            let mut forged = value.clone();
            *forged.pointer_mut(pointer).unwrap() = changed;
            assert!(
                InitializationResponse::decode(forged, &node(), 2676, &plan).is_err(),
                "{pointer}"
            );
        }
    }
    #[test]
    fn controller_initialization_ipc_rejects_changed_identity_plan_and_unobserved_success() {
        let plan = InitializationPlan {
            roots: vec![RootField::PowerSaving],
            profiles: vec![],
            new_profile_fields: vec![],
            connection_features: None,
        };
        let result = InitializationResult {
            connection_probe: Read::Unobserved,
            power_saving: Read::Failed {
                error: "real getter failed".into(),
            },
            controller_mode: Read::Unobserved,
            directory: Read::Failed {
                error: "real directory failed".into(),
            },
            profiles: vec![],
            canceled: false,
            connection_features: None,
        };
        let value = serde_json::to_value(
            InitializationResponse::new(
                node(),
                2676,
                plan.clone(),
                result,
                serde_json::json!({"simulated":true}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(InitializationResponse::decode(value.clone(), &node(), 2676, &plan).is_ok());
        for (pointer, changed) in [
            ("/node/path", serde_json::json!([1])),
            ("/product_id", serde_json::json!(2636)),
            ("/plan/roots", serde_json::json!([])),
            (
                "/result/power_saving",
                serde_json::json!({"status":"unobserved"}),
            ),
            (
                "/result/directory",
                serde_json::json!({"status":"unobserved"}),
            ),
        ] {
            let mut forged = value.clone();
            *forged.pointer_mut(pointer).unwrap() = changed;
            assert!(
                InitializationResponse::decode(forged, &node(), 2676, &plan).is_err(),
                "{pointer}"
            );
        }
        let mut missing = value;
        missing["result"]
            .as_object_mut()
            .unwrap()
            .remove("directory");
        assert!(InitializationResponse::decode(missing, &node(), 2676, &plan).is_err());
    }
    #[test]
    fn controller_initialization_ipc_revalidates_constructor_invariants_inside_typed_profile_values()
     {
        use razer_device::controller_initialization::{
            ProfileDirectory, ProfileField, ProfileName, ProfileObservation, ProfileRead,
            ProfileValue,
        };
        use razer_device::controller_triggers::AnalogTrigger;
        let plan = InitializationPlan {
            roots: vec![],
            profiles: vec![],
            new_profile_fields: vec![ProfileField::ControllerAnalogActuation],
            connection_features: None,
        };
        let result = InitializationResult {
            connection_probe: Read::Unobserved,
            power_saving: Read::Unobserved,
            controller_mode: Read::Unobserved,
            canceled: false,
            connection_features: None,
            directory: Read::Observed {
                value: ProfileDirectory {
                    button_ids: vec![6],
                    profile_ids: vec![3],
                    active_profile: 3,
                    number_of_profiles: 1,
                    names: vec![ProfileName {
                        slot: 3,
                        name: Read::Observed {
                            value: "Real profile".into(),
                        },
                    }],
                },
            },
            profiles: vec![ProfileObservation {
                slot: 3,
                fields: vec![ProfileRead {
                    field: ProfileField::ControllerAnalogActuation,
                    result: Read::Observed {
                        value: ProfileValue::Analog {
                            values: vec![
                                AnalogTrigger::new(1, 0, 100, true).unwrap(),
                                AnalogTrigger::new(2, 0, 100, true).unwrap(),
                            ],
                        },
                    },
                }],
            }],
        };
        let value = serde_json::to_value(
            InitializationResponse::new(
                node(),
                2676,
                plan.clone(),
                result,
                serde_json::json!({"simulated":true}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(InitializationResponse::decode(value.clone(), &node(), 2676, &plan).is_ok());
        for (pointer, changed) in [
            (
                "/result/profiles/0/fields/0/result/value/values/0/endRange",
                serde_json::json!(101),
            ),
            (
                "/result/profiles/0/fields/0/result/value/values/0/startRange",
                serde_json::json!(100),
            ),
            (
                "/result/profiles/0/fields/0/result/value/values/0/controllerId",
                serde_json::json!(3),
            ),
        ] {
            let mut forged = value.clone();
            *forged.pointer_mut(pointer).unwrap() = changed;
            assert!(
                InitializationResponse::decode(forged, &node(), 2676, &plan).is_err(),
                "{pointer}"
            );
        }
        let mut forged = value;
        forged
            .pointer_mut("/result/profiles/0/fields/0/result/value/values")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(InitializationResponse::decode(forged, &node(), 2676, &plan).is_err());
    }
}

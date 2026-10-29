//! Connection-scoped OBM initialization and its later DPI edit associations.
use anyhow::ensure;
use razer_device::{
    mouse_current_dpi::{DpiChange, DpiReport, ble::initialization},
    mouse_runtime_artwork::WindowsBleTarget,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Profiles {
    ReadIdentity,
    Initialize {
        request: initialization::Request,
    },
    Cached {
        slots: initialization::ObservedSlots,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub connection_owner: String,
    pub generation: u64,
    pub profiles: Profiles,
}

impl Context {
    pub fn initializes(&self) -> bool {
        matches!(self.profiles, Profiles::Initialize { .. })
    }

    pub fn validate(&self, target: &WindowsBleTarget) -> anyhow::Result<()> {
        ensure!(
            !self.connection_owner.is_empty()
                && self.generation != 0
                && razer_device::mouse_current_dpi::ble::accepts(
                    target.product_id,
                    target.physical_product_id,
                ),
            "BLE DPI context has no connection owner or generation"
        );
        match &self.profiles {
            Profiles::ReadIdentity => Ok(()),
            Profiles::Initialize { request } => request.validate(),
            Profiles::Cached { slots } => {
                slots.validate_target(target, &self.connection_owner, self.generation)
            }
        }
    }

    pub fn validate_request(
        &self,
        target: &WindowsBleTarget,
        change: Option<&DpiChange>,
    ) -> anyhow::Result<()> {
        self.validate(target)?;
        ensure!(
            !matches!(self.profiles, Profiles::ReadIdentity) || change.is_none(),
            "BLE DPI identity observation cannot also modify settings"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub report: Option<DpiReport>,
    pub serial_number: Option<String>,
    pub initialization: Option<initialization::Report>,
    pub observed_slots: Option<initialization::ObservedSlots>,
    pub error: Option<String>,
    pub cleanup_error: Option<String>,
}

impl Outcome {
    pub fn from_report(report: DpiReport) -> Self {
        Self {
            report: Some(report),
            serial_number: None,
            initialization: None,
            observed_slots: None,
            error: None,
            cleanup_error: None,
        }
    }

    pub fn result(&self) -> Result<DpiReport, String> {
        match (&self.report, &self.error) {
            (Some(report), None) => Ok(report.clone()),
            (_, Some(error)) => Err(error.clone()),
            (None, None) => Err("BLE DPI task has no observed result".to_owned()),
        }
    }

    pub fn identity_result(&self) -> Result<&str, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        self.serial_number
            .as_deref()
            .ok_or_else(|| "BLE serial is unobserved".to_owned())
    }

    /// Validate the connection and the complete initialization result before
    /// publishing the projected associations to the retained owner.
    pub fn validate_context(
        &self,
        target: &WindowsBleTarget,
        context: Option<&Context>,
        change: Option<&DpiChange>,
    ) -> anyhow::Result<()> {
        let identity_read =
            context.is_some_and(|context| matches!(context.profiles, Profiles::ReadIdentity));
        ensure!(
            self.error.as_ref().is_none_or(|error| !error.is_empty())
                && self
                    .cleanup_error
                    .as_ref()
                    .is_none_or(|error| !error.is_empty() && self.error.is_some())
                && if identity_read {
                    self.report.is_none()
                        && self.serial_number.as_ref().is_none_or(|serial| {
                            !serial.is_empty()
                                && serial.bytes().all(|byte| (32..127).contains(&byte))
                        })
                        && (self.serial_number.is_some() || self.error.is_some())
                } else {
                    self.serial_number.is_none() && (self.report.is_some() || self.error.is_some())
                },
            "BLE DPI outcome omits its result or failure"
        );
        match context {
            None => ensure!(
                self.initialization.is_none() && self.observed_slots.is_none(),
                "BLE DPI reply published unrequested initialization"
            ),
            Some(context) => {
                context.validate_request(target, change)?;
                match &context.profiles {
                    Profiles::ReadIdentity => ensure!(
                        self.report.is_none()
                            && self.initialization.is_none()
                            && self.observed_slots.is_none(),
                        "BLE DPI identity read contains settings or initialization results"
                    ),
                    Profiles::Initialize { request } => {
                        let report = self.initialization.as_ref().ok_or_else(|| {
                            anyhow::anyhow!("BLE DPI reply omits initialization progress")
                        })?;
                        report.validate_reply(request)?;
                        if report.completed {
                            let slots = report.observed_slots(
                                target.clone(),
                                context.connection_owner.clone(),
                                context.generation,
                            )?;
                            ensure!(
                                self.observed_slots.as_ref() == Some(&slots),
                                "BLE DPI reply changed projected profile associations"
                            );
                        } else {
                            ensure!(
                                self.error.is_some() && self.observed_slots.is_none(),
                                "Incomplete BLE initialization published a DPI result or cache"
                            );
                        }
                    }
                    Profiles::Cached { slots } => ensure!(
                        self.initialization.is_none()
                            && self.observed_slots.as_ref() == Some(slots),
                        "BLE DPI reply replaced the connection's cached associations"
                    ),
                }
            }
        }
        if let Some(report) = &self.report
            && change.is_some_and(|change| change.stages.is_some() && change.profile_guid.is_some())
        {
            let slots = self.observed_slots.as_ref().ok_or_else(|| {
                anyhow::anyhow!("BLE DPI profile edit has no initialized associations")
            })?;
            ensure!(
                report.profile_slots == slots.profiles
                    || report.write_errors.stages().is_some() && report.profile_slots.is_empty(),
                "BLE DPI edit used associations from another connection"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn target() -> WindowsBleTarget {
        serde_json::from_value(json!({
            "product_id":226,"physical_product_id":228,"device_container_id":"container",
            "hid_path":"hid-ble","device_instance_id":"hid-instance",
            "parent_instance_id":"ble-instance","mac_address":12
        }))
        .unwrap()
    }

    #[test]
    fn current_dpi_ble_cached_context_requires_the_full_connection_identity() {
        let target = target();
        let slots = initialization::ObservedSlots {
            target: target.clone(),
            connection_owner: "workspace-42".into(),
            generation: 7,
            profiles: vec![],
        };
        let context = Context {
            connection_owner: slots.connection_owner.clone(),
            generation: slots.generation,
            profiles: Profiles::Cached { slots },
        };
        context.validate(&target).unwrap();
        for (path, replacement) in [
            ("/connection_owner", json!("replacement")),
            ("/generation", json!(8)),
            (
                "/profiles/slots/target/device_instance_id",
                json!("new-instance"),
            ),
            ("/profiles/slots/target/physical_product_id", json!(227)),
        ] {
            let mut value = serde_json::to_value(&context).unwrap();
            *value.pointer_mut(path).unwrap() = replacement;
            let changed: Context = serde_json::from_value(value).unwrap();
            assert!(changed.validate(&target).is_err(), "{path}");
        }
    }

    #[test]
    fn current_dpi_ble_context_decodes_an_explicit_mode_and_rejects_mixed_sources() {
        let bytes = br#"{"connection_owner":"workspace-42","generation":7,"profiles":{"kind":"initialize","request":{"profiles":[{"guid":"actual-local-guid","name":"Local","obm_slot_ids":[],"dpi_stages":null}],"active_profile_guid":"actual-local-guid","owner":"observed-owner"}}}"#;
        let context: Context = serde_json::from_slice(bytes).unwrap();
        context.validate(&target()).unwrap();
        assert!(context.initializes());
        assert_eq!(serde_json::to_vec(&context).unwrap(), bytes);
        let mut mixed: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        mixed["profiles"]["slots"] = json!({});
        assert!(serde_json::from_value::<Context>(mixed).is_err());
    }

    #[test]
    fn current_dpi_ble_identity_cleanup_failure_retains_serial_without_authorizing_initialization()
    {
        let context = Context {
            connection_owner: "workspace-42".into(),
            generation: 7,
            profiles: Profiles::ReadIdentity,
        };
        let mut outcome = Outcome {
            report: None,
            serial_number: Some("actual-serial".into()),
            initialization: None,
            observed_slots: None,
            error: Some("CCCD removal failed".into()),
            cleanup_error: Some("CCCD removal failed".into()),
        };
        outcome
            .validate_context(&target(), Some(&context), None)
            .unwrap();
        assert!(outcome.identity_result().unwrap_err().contains("CCCD"));
        assert_eq!(outcome.serial_number.as_deref(), Some("actual-serial"));
        outcome.error = None;
        assert!(
            outcome
                .validate_context(&target(), Some(&context), None)
                .is_err()
        );
        outcome.cleanup_error = None;
        outcome
            .validate_context(&target(), Some(&context), None)
            .unwrap();
        assert_eq!(outcome.identity_result().unwrap(), "actual-serial");
    }
}

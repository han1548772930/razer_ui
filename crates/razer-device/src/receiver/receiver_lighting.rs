//! Primary Linker lighting from the current dock manifests and lighting engine.
//! Native frame submission and Feature acknowledgements are separate receipts.
use crate::{
    backend::FeatureTransport,
    lighting_driver::{FrameWriter, NewChroma, Registration, RgbFrame},
    lighting_engine::{EngineFrame, LedConfiguration, LedMapping},
    receiver_capabilities::ReceiverCapability,
    receiver_feature_channel::{WriteProgress, exchange},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[path = "receiver_battery_monitor.rs"]
pub mod battery_monitor;
#[path = "receiver_battery_peers.rs"]
mod battery_peers;
#[path = "receiver_peer_lighting.rs"]
pub mod peer_protocol;
#[path = "receiver_quick_effect.rs"]
mod quick_effect;
pub use battery_peers::{
    PeerBatteryEffect, PeerBatteryEffects, PeerCategory, PeerDeviceFactory, PeerExitEffect,
    PeerManifest, PeerProfileEffect, PeerSupport, peer_identities, peer_manifest, peer_support,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuickEffect {
    selected_effect_id: u16,
    #[serde(
        default,
        deserialize_with = "deserialize_settings",
        skip_serializing_if = "Option::is_none"
    )]
    selected_effect_setting: Option<serde_json::Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    region_id: Option<u8>,
    #[serde(flatten)]
    extensions: std::collections::BTreeMap<String, Value>,
}

fn deserialize_settings<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<serde_json::Map<String, Value>>, D::Error> {
    serde_json::Map::deserialize(deserializer).map(Some)
}

impl QuickEffect {
    pub fn parse(value: &Value) -> anyhow::Result<Self> {
        ensure!(
            value
                .get("selectedEffectSetting")
                .is_none_or(Value::is_object),
            "Dock selected-effect settings must be an object when present"
        );
        let effect: Self = serde_json::from_value(value.clone())?;
        effect.validate_region()?;
        Ok(effect)
    }

    fn validate_region(&self) -> anyhow::Result<()> {
        ensure!(
            self.region_id.is_none_or(|region| region == 0),
            "Dock quick effect does not belong to the manifest region"
        );
        Ok(())
    }

    pub fn source_id(&self) -> u16 {
        self.selected_effect_id
    }

    pub fn same_ui_effect(&self, other: &Self) -> bool {
        // applyRegionQuickEffect compares selected ID and selected settings;
        // missing selectedEffectSetting is normalized to the source's {}.
        self.selected_effect_id == other.selected_effect_id
            && self
                .selected_effect_setting
                .as_ref()
                .is_none_or(|v| v.is_empty())
                == other
                    .selected_effect_setting
                    .as_ref()
                    .is_none_or(|v| v.is_empty())
            && match (
                &self.selected_effect_setting,
                &other.selected_effect_setting,
            ) {
                (Some(left), Some(right)) => left == right,
                (Some(value), None) | (None, Some(value)) => value.is_empty(),
                (None, None) => true,
            }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DockMouseSelection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_id: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dongle_id: Option<u16>,
    #[serde(flatten)]
    pub extensions: std::collections::BTreeMap<String, Value>,
}

impl DockMouseSelection {
    pub fn parse(value: Option<Value>) -> anyhow::Result<Option<Self>> {
        value
            .map(serde_json::from_value)
            .transpose()
            .map_err(Into::into)
    }

    pub fn includes(&self, product_id: u16) -> bool {
        let selected = self.product_id.filter(|id| *id != 0);
        let dongle = self.dongle_id.filter(|id| *id != 0);
        (selected.is_none() && dongle.is_none())
            || selected == Some(product_id)
            || dongle == Some(product_id)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Configuration {
    pub product_id: u16,
    pub edition_id: u8,
    pub effect: QuickEffect,
    pub selected_docking_mouse: Option<DockMouseSelection>,
}

impl Configuration {
    pub fn validate(&self) -> anyhow::Result<()> {
        led_configuration(self.product_id, self.edition_id)?;
        self.effect.validate_region()?;
        let mounted = match self.product_id {
            164 => &[1, 2, 3, 4, 5, 7, 12, 14, 19][..],
            241 => &[1, 2, 3, 4, 8, 12, 14, 19][..],
            _ => unreachable!("validated dock manifest"),
        };
        ensure!(
            mounted.contains(&self.effect.source_id()),
            "Quick effect is absent from the mounted dock page"
        );
        if self.effect.source_id() != 14 {
            self.effect.engine_effect()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Status {
    pub operation_id: String,
    pub product_id: u16,
    pub path: String,
    pub device_container_id: String,
    pub effect: QuickEffect,
    pub software: crate::lighting_engine::SoftwareStatus,
    pub source_effect_unchanged: bool,
    pub selected_docking_mouse: Option<DockMouseSelection>,
    pub application: ApplicationReceipt,
    pub monitoring: battery_monitor::MonitorStatus,
    pub restoration: RestorationDisposition,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RestorationDisposition {
    NotRequired,
    AwaitingAcknowledgements,
    Acknowledged { writes: u32 },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChromaAttributeRequest {
    pub worker_id: uuid::Uuid,
    pub id: u64,
    pub pid: u16,
    pub attribute: u8,
    pub value: u8,
}
impl ChromaAttributeRequest {
    pub fn charging(worker_id: uuid::Uuid, id: u64, pid: u16) -> anyhow::Result<Self> {
        ensure!(
            !worker_id.is_nil() && id > 0 && pid > 0,
            "Chroma attribute request identity is invalid"
        );
        Ok(Self {
            worker_id,
            id,
            pid,
            attribute: 2,
            value: 0,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChromaAttributeReceipt {
    NotSent { reason: String },
    Replied { response_json: String },
    Failed { message: String },
    Canceled,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChromaAttributeExchange {
    pub request: ChromaAttributeRequest,
    pub receipt: ChromaAttributeReceipt,
}
impl ChromaAttributeExchange {
    pub fn new(request: ChromaAttributeRequest, receipt: ChromaAttributeReceipt) -> Self {
        Self { request, receipt }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplicationReceipt {
    AwaitingFrame {
        generation: u64,
    },
    AwaitingLocalService {
        generation: u64,
    },
    SubmittedFrame {
        generation: u64,
    },
    SourceNoWrite {
        generation: u64,
        reason: battery_monitor::NoWriteReason,
    },
}

impl Status {
    pub fn starting(
        operation_id: String,
        path: String,
        device_container_id: String,
        configuration: &Configuration,
    ) -> Self {
        Self {
            operation_id,
            path,
            device_container_id,
            product_id: configuration.product_id,
            effect: configuration.effect.clone(),
            software: crate::lighting_engine::SoftwareStatus::starting(1),
            source_effect_unchanged: false,
            selected_docking_mouse: configuration.selected_docking_mouse.clone(),
            application: ApplicationReceipt::AwaitingFrame { generation: 1 },
            monitoring: Default::default(),
            restoration: RestorationDisposition::NotRequired,
        }
    }
    pub fn restoration_complete(&self) -> bool {
        match self.restoration {
            RestorationDisposition::NotRequired => true,
            RestorationDisposition::AwaitingAcknowledgements => false,
            RestorationDisposition::Acknowledged { writes } => {
                writes > 0 && self.software.restore_acknowledged
            }
        }
    }
    pub fn ensure_applied(&self, configuration: &Configuration) -> anyhow::Result<()> {
        use crate::lighting_engine::SoftwarePhase;
        ensure!(
            self.product_id == configuration.product_id
                && self.effect.same_ui_effect(&configuration.effect)
                && self.selected_docking_mouse == configuration.selected_docking_mouse,
            "Dock lighting receipt belongs to a different profile effect"
        );
        ensure!(
            self.software.phase == SoftwarePhase::Running
                && self.software.errors.is_empty()
                && self.monitoring.pending_chroma_attribute.is_none()
                && match self.application {
                    ApplicationReceipt::SubmittedFrame { generation } =>
                        generation == self.software.generation && self.software.frames_submitted > 0,
                    ApplicationReceipt::SourceNoWrite { generation, .. } =>
                        generation == self.software.generation
                            && configuration.effect.source_id() == 14
                            && self.monitoring.observations > 0
                            && self.monitoring.monitored_peers > 0,
                    ApplicationReceipt::AwaitingFrame { .. }
                    | ApplicationReceipt::AwaitingLocalService { .. } => false,
                },
            "Dock software lighting has no submitted-frame receipt"
        );
        Ok(())
    }
}

pub fn led_configuration(product_id: u16, edition: u8) -> anyhow::Result<LedConfiguration> {
    ensure!(
        matches!(product_id, 164 | 241) && edition == 0,
        "Dock lighting has no current manifest for this product/edition"
    );
    let indices = match product_id {
        164 => [5, 6, 4, 7, 3, 0, 2, 1],
        241 => [6, 7, 5, 0, 4, 1, 3, 2],
        _ => unreachable!("validated manifest product"),
    };
    let positions = [
        [0, 1],
        [1, 0],
        [1, 2],
        [2, 0],
        [2, 2],
        [3, 0],
        [3, 2],
        [4, 1],
    ];
    Ok(LedConfiguration {
        version: 2,
        vendor_id: 5426,
        product_id,
        edition,
        layout: 0,
        matrix_max_row: 5,
        matrix_max_col: 3,
        device_max_row: 1,
        device_max_col: 8,
        led_width: 40,
        led_height: 40,
        led_width_offset: 0,
        led_height_offset: 0,
        max_region: 0,
        region_max_row: 0,
        region_max_col: 0,
        led_matrix: positions
            .into_iter()
            .zip(indices)
            .map(|(matrix_pos, column)| LedMapping {
                matrix_pos,
                device_pos: [0, column],
            })
            .collect(),
        led_regions: Vec::new(),
        led_input_map: Vec::new(),
    })
}

pub struct LightingHid {
    capability: ReceiverCapability,
    frames: NewChroma,
    restoration_required: bool,
}

impl LightingHid {
    pub fn new(capability: &ReceiverCapability) -> anyhow::Result<Self> {
        led_configuration(capability.product_id, 0)?;
        Ok(Self {
            capability: capability.clone(),
            frames: NewChroma::default(),
            restoration_required: false,
        })
    }

    pub fn restoration_required(&self) -> bool {
        self.restoration_required
    }

    pub fn select_custom(
        &mut self,
        transport: &dyn FeatureTransport,
        mut next: impl FnMut() -> anyhow::Result<u8>,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u32> {
        self.restoration_required = true;
        exchange(
            transport,
            &self.capability,
            [2, 0, 4],
            &[3, 0],
            next()?,
            validate,
            &mut WriteProgress::default(),
        )?;
        exchange(
            transport,
            &self.capability,
            [6, 15, 2],
            &[0, 0, 8, 0, 0, 0],
            next()?,
            validate,
            &mut WriteProgress::default(),
        )?;
        Ok(2)
    }

    pub fn restore_on_exit(
        &mut self,
        transport: &dyn FeatureTransport,
        battery_level: bool,
        mut next: impl FnMut() -> anyhow::Result<u8>,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u32> {
        // configureDevicesDefaultData first restores normal device mode. The
        // manifest restores Spectrum, or hardware BatteryLevel for UI 14.
        exchange(
            transport,
            &self.capability,
            [2, 0, 4],
            &[0, 0],
            next()?,
            validate,
            &mut WriteProgress::default(),
        )?;
        let effect = if battery_level { 13 } else { 3 };
        exchange(
            transport,
            &self.capability,
            [6, 15, 2],
            &[1, 0, effect, 0, 0, 0],
            next()?,
            validate,
            &mut WriteProgress::default(),
        )?;
        self.restoration_required = false;
        Ok(2)
    }

    pub fn send_frame(
        &mut self,
        transport: &dyn FeatureTransport,
        frame: &EngineFrame,
        handle: u32,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        ensure!(
            frame.device_handle == handle
                && frame.region_id == 0
                && frame.num_row == 1
                && frame.num_col == 8
                && frame.frame.len() == 24,
            "Dock native frame identity or geometry differs"
        );
        struct Writer<'a, V> {
            transport: &'a dyn FeatureTransport,
            validate: &'a V,
            submitted: &'a std::cell::Cell<u32>,
        }
        impl<V: Fn() -> anyhow::Result<()>> FrameWriter for Writer<'_, V> {
            fn write_report(&mut self, report: &[u8; 91]) -> anyhow::Result<()> {
                (self.validate)()?;
                self.transport.send_feature_at_collection_max(report)?;
                self.submitted.set(self.submitted.get() + 1);
                (self.validate)()
            }
            fn wait_between_rows(&mut self, _: u8) -> anyhow::Result<()> {
                anyhow::bail!("Dock manifest contains one physical LED row")
            }
        }
        let submitted = std::cell::Cell::new(0);
        crate::feature_exchange::with_validated_exchange(
            transport,
            crate::backend::ExchangeOptions::batch(),
            validate,
            |transport, _| {
                self.frames
                    .write_frame(
                        Registration {
                            category: 7,
                            report_id: 0,
                            row_delay_ms: 1,
                        },
                        RgbFrame {
                            first_row: 0,
                            last_row: 0,
                            first_column: 0,
                            last_column: 7,
                            region_id: 0,
                            profile_id: 0,
                            rgb: &frame.frame,
                        },
                        &mut Writer {
                            transport,
                            validate,
                            submitted: &submitted,
                        },
                    )
                    .context("Dock native lighting frame was not submitted")
            },
        )
        .map_err(|error| {
            error.context(crate::ReceiverFrameSubmissionFailure {
                reports_submitted: submitted.get(),
            })
        })
    }
}

#[cfg(test)]
#[path = "receiver_lighting_tests.rs"]
mod tests;

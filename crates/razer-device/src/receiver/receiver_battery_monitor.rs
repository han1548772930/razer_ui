//! Dock monitoring owns percentages and charging overlays. UI BatteryLevel
//! (14, global effect 193) is never submitted to the native effect engine.
use super::{DockMouseSelection, PeerSupport};
use crate::lighting_engine::{EngineEffect, SoftwareEffect};
use serde::{Deserialize, Serialize};

pub const MONITOR_INTERVAL: std::time::Duration = std::time::Duration::from_secs(20 * 60);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NoWriteReason {
    NoEligiblePeer,
    NoPoweredEligiblePeer,
    NoBatteryObservation,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MonitorStatus {
    pub observations: u64,
    pub monitored_peers: usize,
    pub eligible_peers: usize,
    pub battery_reads: u64,
    pub charging: bool,
    pub attached_product_id: Option<u16>,
    pub diagnostics: Vec<String>,
    pub cleanup_errors: Vec<String>,
    pub chroma_sdk: ChromaSdkDisposition,
    pub pending_chroma_attribute: Option<super::ChromaAttributeRequest>,
    pub last_chroma_attribute: Option<super::ChromaAttributeExchange>,
}

impl MonitorStatus {
    pub fn accept_chroma_attribute(
        &mut self,
        worker_id: uuid::Uuid,
        id: u64,
        receipt: super::ChromaAttributeReceipt,
    ) -> anyhow::Result<()> {
        let request = self
            .pending_chroma_attribute
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Dock has no pending Chroma attribute request"))?;
        anyhow::ensure!(
            request.worker_id == worker_id && request.id == id,
            "Chroma attribute receipt belongs to another worker or request"
        );
        let request = self.pending_chroma_attribute.take().unwrap();
        self.last_chroma_attribute = Some(super::ChromaAttributeExchange::new(request, receipt));
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChromaSdkDisposition {
    // The source SDK wrapper returns before a handoff when it has not been
    // initialized. This runtime owns no SDK registered device handles.
    #[default]
    NotInitialized,
}

pub fn selected(selection: Option<&DockMouseSelection>, support: &PeerSupport) -> bool {
    support.support_battery_monitoring
        && selection.is_none_or(|selection| selection.includes(support.source_product_id))
}

pub fn eligible_v2(
    support: &PeerSupport,
    same_instance: bool,
    is_ble: bool,
    charging: bool,
) -> bool {
    support.support_docking_v2 && (same_instance || is_ble && charging)
}

pub fn battery_color(level: u8) -> anyhow::Result<[u8; 3]> {
    anyhow::ensure!(level <= 100, "Battery percentage exceeds the source range");
    Ok(match level {
        0..=24 => [255, 0, 0],
        25..=49 => [255, 69, 0],
        50..=74 => [255, 255, 0],
        75..=100 => [0, 255, 0],
        _ => unreachable!("validated battery percentage"),
    })
}

pub fn effect(level: u8, charging: bool) -> anyhow::Result<EngineEffect> {
    let color = battery_color(level)?;
    Ok(if charging && level < 100 {
        SoftwareEffect::Breathing {
            color: Some(color),
            color2: None,
            random: false,
            duration: None,
        }
        .engine_effect()
    } else {
        SoftwareEffect::Static { color }.engine_effect()
    })
}

pub fn no_device_effect() -> EngineEffect {
    SoftwareEffect::Static { color: [255, 0, 0] }.engine_effect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareObservation {
    Charging {
        connected: bool,
        product_id: u16,
        edition: u16,
    },
    Wireless {
        state: u8,
        product_id: Option<u16>,
    },
    Power {
        on: bool,
    },
    Battery {
        percentage: u8,
        charging: u8,
        power: u8,
    },
}

pub fn hardware_observation(bytes: &[u8]) -> anyhow::Result<Option<HardwareObservation>> {
    // Protocol25Parser consumes the actual report byte zero as recordId.
    if !bytes.first().is_some_and(|record| matches!(record, 5 | 9)) {
        return Ok(None);
    }
    let Some(event) = bytes.get(1).copied() else {
        anyhow::bail!("Lighting hardware notification has no event ID");
    };
    let required = match event {
        59 => 7,
        53 => 6,
        49 => 6,
        9 => 4,
        12 => 3,
        _ => return Ok(None),
    };
    anyhow::ensure!(
        bytes.len() >= required,
        "Lighting hardware notification is truncated"
    );
    Ok(Some(match event {
        59 => HardwareObservation::Charging {
            connected: bytes[2] != 0,
            product_id: u16::from_be_bytes([bytes[3], bytes[4]]),
            edition: u16::from_be_bytes([bytes[5], bytes[6]]),
        },
        53 => HardwareObservation::Wireless {
            state: bytes[2],
            product_id: Some(u16::from_be_bytes([bytes[4], bytes[5]])),
        },
        9 => HardwareObservation::Wireless {
            state: bytes[2],
            product_id: None,
        },
        49 => HardwareObservation::Battery {
            percentage: (u16::from(bytes[2]) * 100 / 255) as u8,
            charging: bytes[3],
            power: bytes[4],
        },
        12 => HardwareObservation::Power { on: bytes[2] != 0 },
        _ => unreachable!("validated hardware event"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receiver_lighting::{ChromaAttributeReceipt, ChromaAttributeRequest};

    #[test]
    fn chroma_receipt_rejects_previous_worker_and_serial_without_consuming_current_call() {
        let current_worker = uuid::Uuid::from_u128(2);
        let previous_worker = uuid::Uuid::from_u128(1);
        let request = ChromaAttributeRequest::charging(current_worker, 1, 215).unwrap();
        let mut status = MonitorStatus::default();
        status.pending_chroma_attribute = Some(request.clone());
        for (worker, id) in [(previous_worker, 1), (current_worker, 2)] {
            assert!(
                status
                    .accept_chroma_attribute(worker, id, ChromaAttributeReceipt::Canceled)
                    .is_err()
            );
            assert_eq!(status.pending_chroma_attribute.as_ref(), Some(&request));
            assert!(status.last_chroma_attribute.is_none());
        }
        status
            .accept_chroma_attribute(current_worker, 1, ChromaAttributeReceipt::Canceled)
            .unwrap();
        assert!(status.pending_chroma_attribute.is_none());
        assert_eq!(
            status.last_chroma_attribute.as_ref().unwrap().request,
            request
        );
        assert!(
            status
                .accept_chroma_attribute(current_worker, 1, ChromaAttributeReceipt::Canceled)
                .is_err()
        );
        assert!(ChromaAttributeRequest::charging(uuid::Uuid::nil(), 1, 215).is_err());
    }

    #[test]
    fn chroma_source_continues_after_every_receipt_without_inventing_device_writes() {
        let worker = uuid::Uuid::from_u128(3);
        let mut status = MonitorStatus::default();
        for (index, receipt) in [
            ChromaAttributeReceipt::NotSent {
                reason: "socket unavailable".into(),
            },
            ChromaAttributeReceipt::Failed {
                message: "token timed out".into(),
            },
            ChromaAttributeReceipt::Replied {
                response_json:
                    r#"{"token":{"id":1,"param":{"pid":215,"attribute":2,"value":0}},"result":0}"#
                        .into(),
            },
            ChromaAttributeReceipt::Canceled,
        ]
        .into_iter()
        .enumerate()
        {
            let request = ChromaAttributeRequest::charging(worker, index as u64 + 1, 215).unwrap();
            status.pending_chroma_attribute = Some(request.clone());
            status
                .accept_chroma_attribute(worker, request.id, receipt.clone())
                .unwrap();
            assert!(status.pending_chroma_attribute.is_none());
            assert_eq!(
                status.last_chroma_attribute.as_ref().unwrap().request,
                request
            );
            assert_eq!(
                status.last_chroma_attribute.as_ref().unwrap().receipt,
                receipt
            );
            assert_eq!(status.battery_reads, 0);
            assert_eq!(status.observations, 0);
        }
    }

    #[test]
    fn typed_thresholds_preserve_source_boundaries_and_charging_engine() {
        for (level, color) in [
            (0, [255, 0, 0]),
            (24, [255, 0, 0]),
            (25, [255, 69, 0]),
            (49, [255, 69, 0]),
            (50, [255, 255, 0]),
            (74, [255, 255, 0]),
            (75, [0, 255, 0]),
            (99, [0, 255, 0]),
            (100, [0, 255, 0]),
        ] {
            assert_eq!(battery_color(level).unwrap(), color);
            assert_eq!(effect(level, false).unwrap().effect, 6);
            assert_eq!(
                effect(level, true).unwrap().effect,
                if level == 100 { 6 } else { 5 }
            );
        }
        assert!(effect(101, true).is_err());
        assert_eq!(no_device_effect().parameters.color, Some(255));
    }
    #[test]
    fn monitor_selection_and_v2_eligibility_keep_real_no_write_branches() {
        let radio = super::super::peer_support(215, 128).unwrap();
        let wired = super::super::peer_support(214, 128).unwrap();
        let chosen = DockMouseSelection {
            product_id: Some(214),
            dongle_id: Some(215),
            ..Default::default()
        };
        assert!(selected(Some(&chosen), &radio));
        assert!(selected(Some(&chosen), &wired));
        assert!(eligible_v2(&radio, true, false, false));
        assert!(!eligible_v2(&radio, false, false, true));
        assert!(!eligible_v2(&wired, true, false, true));
        // A second dock observes the same monitor catalog. Only the owner's
        // actual connection identity may deliver that radio peer's frames.
        let peers = [
            ("dock-a", radio),
            ("dock-b", super::super::peer_support(227, 0).unwrap()),
        ];
        for dock in ["dock-a", "dock-b"] {
            let assigned = peers
                .iter()
                .filter(|(instance, support)| eligible_v2(support, *instance == dock, false, true))
                .map(|(instance, _)| *instance)
                .collect::<Vec<_>>();
            assert_eq!(assigned, vec![dock]);
        }
    }
    #[test]
    fn hardware_events_retain_real_pid_edition_and_reject_truncation() {
        assert_eq!(
            hardware_observation(&[5, 59, 1, 0, 215, 0, 128]).unwrap(),
            Some(HardwareObservation::Charging {
                connected: true,
                product_id: 215,
                edition: 128
            })
        );
        assert_eq!(
            hardware_observation(&[5, 49, 128, 1, 3, 2]).unwrap(),
            Some(HardwareObservation::Battery {
                percentage: 50,
                charging: 1,
                power: 3
            })
        );
        assert!(hardware_observation(&[5, 59, 1]).is_err());
        assert_eq!(hardware_observation(&[7, 59, 1]).unwrap(), None);
        assert_eq!(
            hardware_observation(&[9, 53, 3, 2, 0, 215]).unwrap(),
            Some(HardwareObservation::Wireless {
                state: 3,
                product_id: Some(215)
            })
        );
    }
}

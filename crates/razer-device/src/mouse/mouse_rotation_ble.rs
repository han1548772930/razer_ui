//! PID226's physical BLE228 factory uses rzDevice30 profile in header byte6.
//! A successful setter ACK is distinct from the separately requested getter.
use super::{Reading, Report, Setting};
use crate::mouse_runtime_artwork::WindowsBleTarget;
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

pub const GET: [u8; 3] = [0, 15, 148];
pub const SET: [u8; 3] = [2, 15, 20];
pub const PROFILE: u8 = 1;
pub const EDITION: [u8; 3] = [0, 1, 134];

pub fn accepts(product_id: u32, physical_product_id: u32) -> bool {
    product_id == 226 && physical_product_id == 228
}

pub fn target_from_hid(
    product_id: u32,
    physical_product_id: u32,
    container: &str,
    observation: &serde_json::Value,
) -> anyhow::Result<WindowsBleTarget> {
    ensure!(
        accepts(product_id, physical_product_id),
        "Rotation BLE physical factory differs"
    );
    WindowsBleTarget::from_hid(product_id, physical_product_id, container, observation)
}

pub fn frame(
    transaction: u8,
    command: [u8; 3],
    profile: u8,
    payload: &[u8],
) -> anyhow::Result<Vec<u8>> {
    ensure!(transaction < 31, "Rotation BLE transaction is invalid");
    ensure!(
        matches!((command, profile), (GET | SET, PROFILE) | (EDITION, 0)),
        "Rotation BLE command/profile differs"
    );
    ensure!(
        payload.len() == usize::from(command[0]),
        "Rotation BLE payload length differs"
    );
    let mut bytes = vec![
        transaction,
        command[0],
        0,
        0,
        command[1],
        command[2],
        profile,
        0,
    ];
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledgement {
    pub command: [u8; 3],
    pub profile: u8,
    pub transaction: u8,
    pub payload: Vec<u8>,
    pub data: Vec<u8>,
}
impl Acknowledgement {
    fn validate(&self, command: [u8; 3], profile: u8, payload: &[u8]) -> anyhow::Result<()> {
        ensure!(
            self.command == command && self.profile == profile && self.payload == payload,
            "Rotation BLE acknowledgement belongs to another request"
        );
        frame(self.transaction, self.command, self.profile, &self.payload)?;
        Ok(())
    }
}

pub trait Transport {
    /// Return only after the actual transaction/status response is assembled.
    fn exchange(
        &mut self,
        command: [u8; 3],
        profile: u8,
        payload: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Acknowledgement>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteReport {
    pub attempted_writes: usize,
    pub acknowledgement: Option<Acknowledgement>,
}
impl WriteReport {
    pub(super) fn validate(&self, requested: &Setting, failed: bool) -> anyhow::Result<()> {
        requested.validate()?;
        ensure!(
            self.attempted_writes <= 1,
            "Rotation BLE attempted-write count differs"
        );
        if let Some(reply) = &self.acknowledgement {
            ensure!(
                self.attempted_writes == 1,
                "Rotation BLE acknowledgement has no attempted write"
            );
            reply.validate(
                SET,
                PROFILE,
                &[u8::from(requested.enabled), requested.angle_degrees as u8],
            )?;
        }
        ensure!(
            failed || self.attempted_writes == 1 && self.acknowledgement.is_some(),
            "Rotation BLE setter has no actual acknowledgement"
        );
        Ok(())
    }
}

pub fn execute(
    product_id: u32,
    physical_product_id: u32,
    transport: &mut dyn Transport,
    setting: Option<&Setting>,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    ensure!(
        accepts(product_id, physical_product_id),
        "Rotation BLE product identity differs"
    );
    validate()?;
    let result = if let Some(setting) = setting {
        setting.validate()?;
        let payload = [u8::from(setting.enabled), setting.angle_degrees as u8];
        let mut delivery = WriteReport::default();
        let result = (|| {
            validate()?;
            delivery.attempted_writes = 1;
            let reply = transport.exchange(SET, PROFILE, &payload, validate)?;
            reply.validate(SET, PROFILE, &payload)?;
            delivery.acknowledgement = Some(reply);
            validate()
        })();
        Report::BleWrite {
            requested: *setting,
            delivery,
            failure: result.err().map(|error| format!("{error:#}")),
        }
    } else {
        let reply = transport.exchange(GET, PROFILE, &[], validate)?;
        reply.validate(GET, PROFILE, &[])?;
        validate()?;
        let raw_state = *reply
            .data
            .first()
            .context("Rotation BLE state is unobserved")?;
        let raw_angle = *reply
            .data
            .get(1)
            .context("Rotation BLE angle is unobserved")?;
        Report::Read {
            reading: Reading {
                raw_state,
                raw_angle,
            },
        }
    };
    result.validate_reply(setting)?;
    result.validate_transport(super::TransportKind::Ble)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    struct Script {
        data: Vec<u8>,
        frames: Vec<Vec<u8>>,
        fail: bool,
        wrong_profile: bool,
    }
    impl Transport for Script {
        fn exchange(
            &mut self,
            command: [u8; 3],
            profile: u8,
            payload: &[u8],
            validate: &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<Acknowledgement> {
            validate()?;
            let transaction = 30;
            self.frames
                .push(frame(transaction, command, profile, payload)?);
            ensure!(!self.fail, "device rejected command");
            Ok(Acknowledgement {
                command,
                profile: if self.wrong_profile { 0 } else { profile },
                transaction,
                payload: payload.to_vec(),
                data: self.data.clone(),
            })
        }
    }
    fn io(data: Vec<u8>) -> Script {
        Script {
            data,
            frames: vec![],
            fail: false,
            wrong_profile: false,
        }
    }

    #[test]
    fn rotation_ble_set_uses_header_profile_and_ack_without_implicit_get() {
        for (enabled, angle, raw) in [(true, -44, 212), (false, 44, 44)] {
            let mut io = io(vec![]);
            let setting = Setting {
                enabled,
                angle_degrees: angle,
            };
            let result = execute(226, 228, &mut io, Some(&setting), &|| Ok(())).unwrap();
            assert!(result.source_acknowledged());
            assert_eq!(
                io.frames,
                [vec![30, 2, 0, 0, 15, 20, 1, 0, u8::from(enabled), raw]]
            );
            assert!(
                result
                    .validate_transport(super::super::TransportKind::Feature)
                    .is_err()
            );
        }
        assert_eq!(
            frame(0, EDITION, 0, &[]).unwrap(),
            [0, 0, 0, 0, 1, 134, 0, 0]
        );
        assert!(frame(31, SET, 1, &[1, 0]).is_err());
        assert!(frame(1, SET, 0, &[1, 0]).is_err());
        assert!(frame(1, SET, 1, &[1]).is_err());
    }

    #[test]
    fn rotation_ble_get_preserves_unknown_values_and_rejects_missing_bytes() {
        let mut io = io(vec![254, 200]);
        let Report::Read { reading } = execute(226, 228, &mut io, None, &|| Ok(())).unwrap() else {
            panic!("getter kind")
        };
        assert_eq!((reading.raw_state, reading.raw_angle), (254, 200));
        assert_eq!((reading.enabled(), reading.angle_degrees()), (None, None));
        assert_eq!(io.frames, [vec![30, 0, 0, 0, 15, 148, 1, 0]]);
        for data in [vec![], vec![1]] {
            assert!(execute(226, 228, &mut self::io(data), None, &|| Ok(())).is_err());
        }
    }

    #[test]
    fn rotation_ble_failed_or_mismatched_setter_never_confirms_and_cancel_stops_write() {
        let setting = Setting {
            enabled: true,
            angle_degrees: -1,
        };
        for wrong_profile in [false, true] {
            let mut io = io(vec![]);
            io.fail = !wrong_profile;
            io.wrong_profile = wrong_profile;
            let result = execute(226, 228, &mut io, Some(&setting), &|| Ok(())).unwrap();
            assert!(!result.source_acknowledged());
            assert!(result.failure_reason().is_some());
        }
        let mut io = io(vec![]);
        let canceled = Cell::new(false);
        let validate = || {
            ensure!(!canceled.replace(true), "owner canceled");
            Ok(())
        };
        let result = execute(226, 228, &mut io, Some(&setting), &validate).unwrap();
        assert!(!result.source_acknowledged());
        assert!(io.frames.is_empty());
        assert!(execute(226, 227, &mut io, None, &|| Ok(())).is_err());
        assert!(execute(222, 228, &mut io, None, &|| Ok(())).is_err());
    }

    #[test]
    fn rotation_ble_actual_fragments_keep_transaction_status_and_payload_boundaries() {
        use crate::mouse_runtime_artwork::{BleCommandStatus, BleResponse};
        let mut response = BleResponse::new(4);
        assert_eq!(response.push(&[3, 2, 0, 0, 15, 148, 1, 2]).unwrap(), None);
        assert_eq!(response.push(&[4, 2, 0, 0, 15, 148, 1, 2]).unwrap(), None);
        assert_eq!(response.push(&[1]).unwrap(), None);
        assert_eq!(response.push(&[212]).unwrap(), Some(vec![1, 212]));
        assert_eq!(
            BleResponse::new(5)
                .push(&[5, 0, 0, 0, 15, 20, 1, 2])
                .unwrap(),
            Some(vec![])
        );
        for status in [0, 1, 3, 4, 5] {
            let error = BleResponse::new(6)
                .push(&[6, 0, 0, 0, 15, 20, 1, status])
                .unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<BleCommandStatus>()
                    .unwrap()
                    .retryable(),
                matches!(status, 1 | 3)
            );
        }
        assert!(BleResponse::new(7).push(&[7, 0, 0]).is_err());
    }
}

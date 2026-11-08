//! Source setup/unload mode commands for keyboards without an OBM startup branch.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    Initialize,
    Release,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Target {
    Collection {
        node: crate::backend::HidNode,
        product_id: u32,
        peer_product_id: Option<u32>,
    },
    WindowsContainer(crate::device_reads::DeviceReadTarget),
    WindowsBle(crate::mouse_runtime_artwork::WindowsBleTarget),
}
impl Target {
    pub fn product_id(&self) -> u32 {
        match self {
            Self::Collection { product_id, .. } => *product_id,
            Self::WindowsContainer(target) => target.product_id,
            Self::WindowsBle(target) => target.product_id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    pub mode: u8,
    pub parameter: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub product_id: u32,
    pub operation: Operation,
    pub before: Option<Reading>,
    pub acknowledgement: Option<Reading>,
}

pub fn supported(product_id: u32) -> bool {
    matches!(
        product_id,
        515 | 521
            | 529
            | 534
            | 542
            | 545
            | 554
            | 555
            | 556
            | 565
            | 567
            | 574
            | 575
            | 580
            | 585
            | 591
            | 592
            | 593
            | 605
            | 606
            | 631
            | 654
            | 655
            | 659
            | 660
            | 673
            | 674
            | 675
            | 689
            | 2595
            | 2596
    )
}

pub fn execute(
    product_id: u32,
    operation: Operation,
    mut exchange: impl FnMut([u8; 3], &[u8]) -> Result<Vec<u8>>,
) -> Result<Report> {
    ensure!(
        supported(product_id),
        "Keyboard mode requires the reviewed non-OBM setup branch"
    );
    let decode = |bytes: Vec<u8>| -> Result<Reading> {
        ensure!(bytes.len() >= 2, "Keyboard mode response is truncated");
        Ok(Reading {
            mode: bytes[0],
            parameter: bytes[1],
        })
    };
    let before = if operation == Operation::Initialize {
        Some(decode(exchange([2, 0, 132], &[])?)?)
    } else {
        None
    };
    let acknowledgement = if operation == Operation::Release || before.is_some_and(|r| r.mode != 3)
    {
        let desired = if operation == Operation::Initialize { 3 } else { 0 };
        Some(decode(exchange([2, 0, 4], &[desired, 0])?)?)
    } else {
        None
    };
    Ok(Report {
        product_id,
        operation,
        before,
        acknowledgement,
    })
}

pub fn execute_hid(
    device: &dyn FeatureTransport,
    capability: &DeviceReadCapability,
    operation: Operation,
    mut transaction: impl FnMut() -> Result<u8>,
    validate: &impl Fn() -> Result<()>,
) -> Result<Report> {
    execute(capability.product_id, operation, |command, payload| {
        validate()?;
        feature_exchange::exchange(
            device,
            capability,
            command,
            payload,
            transaction()?,
            validate,
        )
    })
}

pub fn ble_frame(transaction: u8, command: [u8; 3], payload: &[u8]) -> Result<Vec<u8>> {
    let ble_command = match command {
        [2, 0, 132] => {
            ensure!(payload.is_empty(), "Mode getter has a payload");
            [0, 1, 130]
        }
        [2, 0, 4] => {
            ensure!(payload.len() == 2, "Mode setter pair is incomplete");
            [2, 1, 2]
        }
        _ => anyhow::bail!("Unexpected keyboard mode command"),
    };
    crate::keyboard_settings::ble::frame(transaction, ble_command, 0, 0, payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initialization_skips_driver_and_release_is_unconditional() {
        for product in [515, 529, 574, 585, 631, 2595] {
            assert!(supported(product));
            let mut calls = Vec::new();
            let report = execute(product, Operation::Initialize, |cmd, data| {
                calls.push((cmd, data.to_vec()));
                Ok(vec![3, 9])
            })
            .unwrap();
            assert_eq!(calls, vec![([2, 0, 132], vec![])]);
            assert_eq!(
                report.before,
                Some(Reading {
                    mode: 3,
                    parameter: 9
                })
            );
            assert!(report.acknowledgement.is_none());
            calls.clear();
            execute(product, Operation::Release, |cmd, data| {
                calls.push((cmd, data.to_vec()));
                Ok(vec![0, 0])
            })
            .unwrap();
            assert_eq!(calls, vec![([2, 0, 4], vec![0, 0])]);
        }
        assert!(!supported(756));
        assert!(!supported(679));
    }
    #[test]
    fn non_driver_modes_write_and_read_failure_does_not_write() {
        let mut calls = Vec::new();
        let report = execute(515, Operation::Initialize, |cmd, data| {
            calls.push((cmd, data.to_vec()));
            Ok(if cmd[2] == 132 { vec![255, 7] } else { vec![3, 0] })
        })
        .unwrap();
        assert_eq!(calls, vec![([2, 0, 132], vec![]), ([2, 0, 4], vec![3, 0])]);
        assert_eq!(report.before.unwrap().mode, 255);
        let mut count = 0;
        assert!(
            execute(515, Operation::Initialize, |_, _| {
                count += 1;
                Ok(vec![0])
            })
            .is_err()
        );
        assert_eq!(count, 1);
        assert!(execute(515, Operation::Initialize, |_, _| anyhow::bail!("canceled")).is_err());
    }
    #[test]
    fn physical_reports_follow_source_commands_and_ble_encoding() {
        use crate::keyboard_control_test_transport::{Mock, reply, sequence};
        for pid in [
            515, 521, 529, 534, 542, 545, 554, 555, 556, 565, 567, 574, 575, 580, 585, 591, 592,
            593, 605, 606, 631, 654, 655, 659, 660, 673, 674, 675, 689, 2595, 2596,
        ] {
            assert!(supported(pid), "PID{pid}");
            let mut cap = crate::keyboard_metadata::capability(pid)
                .or_else(|| crate::keyboard_metadata::radio_capability(pid))
                .unwrap()
                .clone();
            cap.max_retry_in = 1;
            cap.max_retry_out = 1;
            cap.sleep_between_in_ms = 0;
            cap.sleep_between_out_ms = 0;
            cap.sleep_between_out_in_ms = 0;
            let device = Mock::new([reply(&[0, 7]), reply(&[3, 0])]);
            let result =
                execute_hid(&device, &cap, Operation::Initialize, sequence(), &|| Ok(())).unwrap();
            assert_eq!(
                result.before,
                Some(Reading {
                    mode: 0,
                    parameter: 7
                })
            );
            let sent = device.sent();
            assert_eq!(&sent[0][6..9], &[2, 0, 132]);
            assert_eq!(&sent[1][6..11], &[2, 0, 4, 3, 0]);
        }
        assert_eq!(
            ble_frame(7, [2, 0, 132], &[]).unwrap(),
            [7, 0, 0, 0, 1, 130, 0, 0]
        );
        assert_eq!(
            ble_frame(8, [2, 0, 4], &[3, 0]).unwrap(),
            [8, 2, 0, 0, 1, 2, 0, 0, 3, 0]
        );
        assert!(ble_frame(31, [2, 0, 4], &[3, 0]).is_err());
    }
}

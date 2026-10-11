//! Current rzDevice25SagePC modules 98773/87969 + current host protocol25.
//! Report ID 10, 90-byte middleware packet, 91-byte host Feature report.
use super::{
    backend::FeatureTransport,
    controller_calibration::{
        RawAnalogInput, TriggerCalibrationRange, TriggerCalibrationTransport, TriggerPart,
    },
};
use anyhow::{bail, ensure};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibrationCommand {
    RawInput,
    GetRange(TriggerPart),
    SetRange(TriggerPart, TriggerCalibrationRange),
}
impl CalibrationCommand {
    fn header(self) -> [u8; 3] {
        match self {
            Self::RawInput => [80, 12, 152],
            Self::GetRange(_) => [51, 12, 153],
            Self::SetRange(..) => [51, 12, 25],
        }
    }
}

pub fn feature_report(command: CalibrationCommand, transaction: u8) -> anyhow::Result<[u8; 91]> {
    ensure!(transaction < 31, "Invalid SagePC transaction");
    let header = command.header();
    let mut report = [0; 91];
    report[0] = 10;
    report[2] = transaction;
    report[6..9].copy_from_slice(&header);
    match command {
        // Vendor passes the same header as both command and dataArrayIn.
        CalibrationCommand::RawInput => report[9..12].copy_from_slice(&header),
        CalibrationCommand::GetRange(part) => report[9] = part.analog_mask(),
        CalibrationCommand::SetRange(part, range) => {
            report[9] = part.analog_mask();
            let offset = if part == TriggerPart::Left { 25 } else { 29 };
            report[9 + offset..11 + offset].copy_from_slice(&range.min.to_be_bytes());
            report[11 + offset..13 + offset].copy_from_slice(&range.max.to_be_bytes());
        }
    }
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    Ok(report)
}

#[derive(Debug, PartialEq, Eq)]
pub enum CalibrationReply {
    Busy,
    Resend,
    Rejected(u8),
    Data(Vec<u8>),
}
pub fn decode_feature(
    report: &[u8],
    command: CalibrationCommand,
    transaction: u8,
) -> anyhow::Result<CalibrationReply> {
    ensure!(
        report.len() == 91 && report[0] == 10,
        "Truncated or wrong SagePC Feature report"
    );
    let packet = &report[1..];
    let header = command.header();
    if packet[1] != transaction || packet[6] != header[1] || packet[7] != header[2] {
        return Ok(CalibrationReply::Resend);
    }
    match packet[0] {
        1 => Ok(CalibrationReply::Busy),
        0 | 3 | 4 => Ok(CalibrationReply::Resend),
        2 => {
            let size = usize::from(packet[5]);
            ensure!(size <= 80, "Invalid calibration packet size");
            Ok(CalibrationReply::Data(packet[8..8 + size].to_vec()))
        }
        other => Ok(CalibrationReply::Rejected(other)),
    }
}

/// Exact SagePC retries. Caller must hold the source's per-device lock for
/// each exchange, retain observed identity and cancel on loss of that identity.
/// The wait callback provides timing plus deadline/cancellation checks.
pub struct CalibrationProtocolSession<
    'a,
    T: FeatureTransport + ?Sized,
    W: FnMut(Duration) -> anyhow::Result<()>,
> {
    transport: &'a T,
    wait: W,
    transaction: u8,
}
impl<'a, T: FeatureTransport + ?Sized, W: FnMut(Duration) -> anyhow::Result<()>>
    CalibrationProtocolSession<'a, T, W>
{
    pub fn new(transport: &'a T, wait: W) -> Self {
        Self {
            transport,
            wait,
            transaction: 0,
        }
    }
    fn exchange(&mut self, command: CalibrationCommand) -> anyhow::Result<Vec<u8>> {
        // _getTransactionId returns transactionId++ after resetting 31 to 0.
        // The constructor starts at zero: 0..30, then zero again.
        let transaction = self.transaction;
        self.transaction = (self.transaction + 1) % 31;
        let report = feature_report(command, transaction)?;
        let mut last_error = String::from("No calibration response");
        for _ in 0..20 {
            // Host sendFeatureReport provides the source OUT delay. The Rust
            // adapter supplies that same delay here through the wait callback.
            (self.wait)(Duration::from_millis(30))?;
            if let Err(error) = self.transport.send_feature(&report) {
                last_error = error.to_string();
                continue;
            }
            (self.wait)(Duration::from_millis(30))?;
            for retry in 0..10 {
                if retry > 0 {
                    (self.wait)(Duration::from_millis(30))?;
                }
                let mut incoming = [0; 91];
                incoming[0] = 10;
                let length = match self.transport.get_feature(&mut incoming) {
                    Ok(length) => length,
                    Err(error) => {
                        last_error = error.to_string();
                        break;
                    }
                };
                ensure!(
                    length == incoming.len(),
                    "SagePC calibration response length mismatch"
                );
                match decode_feature(&incoming, command, transaction)? {
                    CalibrationReply::Busy => last_error = "Device busy".into(),
                    CalibrationReply::Resend => {
                        last_error = "Calibration response requires resend".into();
                        break;
                    }
                    CalibrationReply::Rejected(status) => {
                        bail!("Calibration command rejected with status {status}")
                    }
                    CalibrationReply::Data(data) => return Ok(data),
                }
            }
        }
        bail!("Calibration retries exhausted: {last_error}")
    }
}
impl<T: FeatureTransport + ?Sized, W: FnMut(Duration) -> anyhow::Result<()>>
    TriggerCalibrationTransport for CalibrationProtocolSession<'_, T, W>
{
    fn raw_analog_input(&mut self) -> anyhow::Result<RawAnalogInput> {
        let data = self.exchange(CalibrationCommand::RawInput)?;
        // Source parser only checks >0 before reading six words. Rust rejects
        // truncated input explicitly rather than manufacturing zero fields.
        ensure!(data.len() >= 12, "Raw analog input is truncated");
        let word = |offset| f64::from(u16::from_be_bytes([data[offset], data[offset + 1]]));
        Ok(RawAnalogInput {
            lx: word(0),
            ly: word(2),
            rx: word(4),
            ry: word(6),
            lt: word(8),
            rt: word(10),
        })
    }
    fn set_trigger_calibration(
        &mut self,
        part: TriggerPart,
        range: TriggerCalibrationRange,
    ) -> anyhow::Result<()> {
        self.exchange(CalibrationCommand::SetRange(part, range))?;
        Ok(())
    }
    fn trigger_calibration(
        &mut self,
        part: TriggerPart,
    ) -> anyhow::Result<TriggerCalibrationRange> {
        let data = self.exchange(CalibrationCommand::GetRange(part))?;
        ensure!(
            data.len() == 33,
            "Calibration readback must contain 33 source bytes"
        );
        // Source does not compare returned analogId. Range verification above
        // the protocol layer compares the two selected fields exactly.
        let offset = if part == TriggerPart::Left { 25 } else { 29 };
        Ok(TriggerCalibrationRange {
            min: u16::from_be_bytes([data[offset], data[offset + 1]]),
            max: u16::from_be_bytes([data[offset + 2], data[offset + 3]]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque};
    struct Transport {
        sends: RefCell<Vec<Vec<u8>>>,
        responses: RefCell<VecDeque<Vec<u8>>>,
    }
    impl FeatureTransport for Transport {
        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            self.sends.borrow_mut().push(report.to_vec());
            Ok(())
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            let bytes = self
                .responses
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("Fixture responses exhausted"))?;
            report[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::json!({"fixture":true})
        }
    }
    fn response(command: CalibrationCommand, transaction: u8, status: u8, data: &[u8]) -> Vec<u8> {
        let mut report = feature_report(command, transaction).unwrap().to_vec();
        report[1] = status;
        report[6] = data.len() as u8;
        report[9..9 + data.len()].copy_from_slice(data);
        report
    }
    #[test]
    fn raw_query_retains_original_header_payload_and_report_id() {
        let report = feature_report(CalibrationCommand::RawInput, 1).unwrap();
        assert_eq!(report[0], 10);
        assert_eq!(&report[6..9], &[80, 12, 152]);
        assert_eq!(&report[9..12], &[80, 12, 152]);
        assert_eq!(report[89], report[3..89].iter().fold(0, |s, b| s ^ b));
    }
    #[test]
    fn right_trigger_range_is_big_endian_at_source_offsets() {
        let report = feature_report(
            CalibrationCommand::SetRange(
                TriggerPart::Right,
                TriggerCalibrationRange {
                    min: 0x123,
                    max: 0x789,
                },
            ),
            30,
        )
        .unwrap();
        assert_eq!(report[9], 8);
        assert_eq!(&report[38..42], &[1, 0x23, 7, 0x89]);
        assert!(report[10..38].iter().all(|b| *b == 0));
        assert_eq!(&report[6..9], &[51, 12, 25]);
    }
    #[test]
    fn busy_mismatch_rejection_and_actual_data_are_distinct() {
        let command = CalibrationCommand::GetRange(TriggerPart::Left);
        let mut report = feature_report(command, 3).unwrap();
        report[1] = 1;
        assert_eq!(
            decode_feature(&report, command, 3).unwrap(),
            CalibrationReply::Busy
        );
        report[1] = 2;
        report[2] = 4;
        assert_eq!(
            decode_feature(&report, command, 3).unwrap(),
            CalibrationReply::Resend
        );
        report[2] = 3;
        report[1] = 5;
        assert_eq!(
            decode_feature(&report, command, 3).unwrap(),
            CalibrationReply::Rejected(5)
        );
        report[1] = 2;
        report[6] = 33;
        assert!(
            matches!(decode_feature(&report,command,3).unwrap(),CalibrationReply::Data(d) if d.len()==33)
        );
    }
    #[test]
    fn simulated_busy_then_stale_reply_resends_the_original_transaction() {
        let command = CalibrationCommand::RawInput;
        let data = [0, 1, 0, 2, 0, 3, 0, 4, 1, 44, 7, 208];
        let io = Transport {
            sends: RefCell::new(vec![]),
            responses: RefCell::new(
                [
                    response(command, 0, 1, &[]),
                    response(command, 1, 2, &data),
                    response(command, 0, 2, &data),
                ]
                .into(),
            ),
        };
        let mut waits = vec![];
        let mut session = CalibrationProtocolSession::new(&io, |delay| {
            waits.push(delay);
            Ok(())
        });
        let actual = session.raw_analog_input().unwrap();
        assert_eq!(actual.lt, 300.);
        assert_eq!(actual.rt, 2000.);
        drop(session);
        let sends = io.sends.borrow();
        assert_eq!(sends.len(), 2);
        assert_eq!(sends[0], sends[1]);
        assert_eq!(waits, vec![Duration::from_millis(30); 5]);
    }
    #[test]
    fn unsupported_command_is_not_retried_and_truncated_data_is_not_padded() {
        for (status, data) in [(5, vec![]), (2, vec![0, 1])] {
            let io = Transport {
                sends: RefCell::new(vec![]),
                responses: RefCell::new(
                    [response(CalibrationCommand::RawInput, 0, status, &data)].into(),
                ),
            };
            let mut session = CalibrationProtocolSession::new(&io, |_| Ok(()));
            assert!(session.raw_analog_input().is_err());
            assert_eq!(io.sends.borrow().len(), 1);
        }
    }
    #[test]
    fn transaction_starts_at_zero_and_wraps_after_thirty() {
        let command = CalibrationCommand::RawInput;
        let io = Transport {
            sends: RefCell::new(vec![]),
            responses: RefCell::new(
                (0..33)
                    .map(|i| response(command, i % 31, 2, &[0; 12]))
                    .collect(),
            ),
        };
        let mut session = CalibrationProtocolSession::new(&io, |_| Ok(()));
        for _ in 0..33 {
            session.raw_analog_input().unwrap();
        }
        let actual = io
            .sends
            .borrow()
            .iter()
            .map(|report| report[2])
            .collect::<Vec<_>>();
        assert_eq!(actual, (0..33).map(|i| i % 31).collect::<Vec<_>>());
        assert!(feature_report(command, 31).is_err());
    }
}

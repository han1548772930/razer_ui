//! Scripted Feature transport for the keyboard control tests. No HID device is
//! opened; responses are independently supplied by each test.
use crate::backend::FeatureTransport;
use anyhow::Context as _;
use std::{collections::VecDeque, sync::Mutex};

#[derive(Clone)]
pub struct Reply {
    pub status: u8,
    pub data: Vec<u8>,
    pub count: usize,
    pub report_id: u8,
    pub declared_length: Option<u8>,
    pub wrong_transaction: bool,
    pub wrong_command: bool,
}

pub fn reply(data: &[u8]) -> Reply {
    Reply {
        status: 2,
        data: data.to_vec(),
        count: 91,
        report_id: 0,
        declared_length: None,
        wrong_transaction: false,
        wrong_command: false,
    }
}

pub struct Mock {
    sent: Mutex<Vec<Vec<u8>>>,
    replies: Mutex<VecDeque<Reply>>,
    send_errors_remaining: Mutex<usize>,
}

impl Mock {
    pub fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
        Self {
            sent: Mutex::new(Vec::new()),
            replies: Mutex::new(replies.into_iter().collect()),
            send_errors_remaining: Mutex::new(0),
        }
    }

    pub fn fail_sends(self, count: usize) -> Self {
        *self.send_errors_remaining.lock().unwrap() = count;
        self
    }

    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.lock().unwrap().clone()
    }
}

impl FeatureTransport for Mock {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(report.to_vec());
        let mut remaining = self.send_errors_remaining.lock().unwrap();
        if *remaining != 0 {
            *remaining -= 1;
            anyhow::bail!("scripted send failure");
        }
        Ok(())
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .context("scripted reply exhausted")?;
        let sent = self.sent.lock().unwrap().last().unwrap().clone();
        report.fill(0);
        report[0] = reply.report_id;
        report[1] = reply.status;
        report[2] = sent[2] ^ u8::from(reply.wrong_transaction);
        report[6] = reply.declared_length.unwrap_or(reply.data.len() as u8);
        report[7..9].copy_from_slice(&sent[7..9]);
        report[8] ^= u8::from(reply.wrong_command);
        report[9..9 + reply.data.len()].copy_from_slice(&reply.data);
        Ok(reply.count)
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"transport": "scripted_keyboard_control"})
    }
}

pub fn sequence() -> impl FnMut() -> anyhow::Result<u8> {
    let mut sequence = 0;
    move || {
        let current = sequence;
        sequence = (sequence + 1) % 31;
        Ok(current)
    }
}

pub fn assert_report_integrity(reports: &[Vec<u8>]) {
    assert_report_integrity_with_id(reports, 0);
}

pub fn assert_report_integrity_with_id(reports: &[Vec<u8>], report_id: u8) {
    for report in reports {
        assert_eq!(report.len(), 91);
        assert_eq!(report[0], report_id);
        assert_eq!(
            report[89],
            report[3..89].iter().fold(0, |crc, byte| crc ^ byte)
        );
        assert_eq!(report[90], 0);
    }
}

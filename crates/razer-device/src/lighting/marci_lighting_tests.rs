use super::*;
use crate::backend::ReportLengths;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

fn frame(handle: u32, value: u8) -> EngineFrame {
    EngineFrame {
        device_handle: handle,
        region_id: 0,
        num_row: 1,
        num_col: 176,
        frame: vec![value; 528],
    }
}
fn node(control: bool) -> HidNode {
    HidNode {
        path: b"marci-simulated-collection".to_vec(),
        vendor_id: 5426,
        product_id: 3942,
        usage_page: if control { 12 } else { 0xff14 },
        usage: 1,
        interface_number: 2,
        release_number: 0,
        serial_number: None,
        manufacturer: None,
        product: None,
    }
}
struct Fake {
    node: HidNode,
    sent: RefCell<Vec<Vec<u8>>>,
    replies: RefCell<VecDeque<(u8, Vec<u8>, Option<u8>)>>,
    count: Cell<usize>,
    feature_len: Cell<usize>,
    output_len: Cell<usize>,
    read_len: Cell<usize>,
    read_count: Cell<usize>,
    failed: Cell<bool>,
}
impl Fake {
    fn new(control: bool) -> Self {
        Self {
            node: node(control),
            sent: RefCell::new(Vec::new()),
            replies: RefCell::new(VecDeque::new()),
            count: Cell::new(781),
            feature_len: Cell::new(91),
            output_len: Cell::new(781),
            read_len: Cell::new(91),
            read_count: Cell::new(0),
            failed: Cell::new(false),
        }
    }
    fn reply(&self, status: u8, data: &[u8], command: Option<u8>) {
        self.replies
            .borrow_mut()
            .push_back((status, data.to_vec(), command));
    }
}
impl FeatureTransport for Fake {
    fn hid_node(&self) -> Option<&HidNode> {
        Some(&self.node)
    }
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        ensure!(!self.failed.get(), "simulated send failure");
        self.sent.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.read_count.set(self.read_count.get() + 1);
        let (status, data, command) = self
            .replies
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("no simulated reply"))?;
        let sent = self.sent.borrow();
        let sent = sent.last().unwrap();
        report.copy_from_slice(sent);
        report[1] = status;
        report[6] = data.len() as u8;
        report[8] = command.unwrap_or(sent[8]);
        report[9..9 + data.len()].copy_from_slice(&data);
        Ok(self.read_len.get())
    }
    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        ensure!(!self.failed.get(), "simulated output failure");
        self.sent.borrow_mut().push(report.to_vec());
        Ok(self.count.get())
    }
    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        Ok(ReportLengths {
            feature: [(16, self.feature_len.get())].into(),
            output: [(23, self.output_len.get())].into(),
            ..Default::default()
        })
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}

#[test]
fn marci_frame_preserves_defined_bytes_and_explicit_native_tail() {
    let source = EngineFrame {
        frame: (0..528).map(|i| i as u8).collect(),
        ..frame(7, 0)
    };
    let f = Frame::from_engine(&source, 7).unwrap();
    let js = f.javascript_report();
    assert_eq!(&js[..7], &[23, 0, 132, 0, 1, 0, 87]);
    assert_eq!(&js[7..535], source.frame.as_slice());
    assert_eq!(&js[535..], &[0; 246]);
    let tail = [0xa5; 246];
    let native = f.native_report(&tail);
    assert_eq!(&native[..535], &js[..535]);
    assert_eq!(&native[535..], &tail);
    for bad in [
        EngineFrame {
            frame: vec![0; 527],
            ..source.clone()
        },
        EngineFrame {
            num_col: 175,
            ..source.clone()
        },
        EngineFrame {
            region_id: 1,
            ..source.clone()
        },
        EngineFrame {
            num_row: 2,
            ..source.clone()
        },
        EngineFrame {
            device_handle: 8,
            ..source
        },
    ] {
        assert!(Frame::from_engine(&bad, 7).is_err());
    }
    assert!(Frame::from_engine(&frame(0, 0), 0).is_err());
}

#[test]
fn marci_output_reports_actual_count_without_a_device_ack() {
    let mut hid = Fake::new(false);
    let f = Frame::from_engine(&frame(1, 42), 1).unwrap();
    assert!(submit_javascript_frame(&hid, &f).unwrap().complete);
    hid.count.set(780);
    assert_eq!(
        submit_javascript_frame(&hid, &f).unwrap(),
        OutputReceipt {
            submitted_bytes: 781,
            transferred_bytes: 780,
            complete: false
        }
    );
    hid.count.set(782);
    assert!(submit_javascript_frame(&hid, &f).is_err());
    hid.count.set(781);
    hid.output_len.set(91);
    assert!(submit_javascript_frame(&hid, &f).is_err());
    hid.output_len.set(781);
    hid.node.usage_page = 12;
    assert!(submit_javascript_frame(&hid, &f).is_err());
    hid.node = node(false);
    hid.node.product_id = 4115;
    assert!(submit_javascript_frame(&hid, &f).is_err());
    hid.node = node(false);
    hid.failed.set(true);
    assert!(submit_javascript_frame(&hid, &f).is_err());
}

#[test]
fn marci_latest_frames_replace_per_handle_and_drop_gated_ticks() {
    let mut latest = LatestFrames::new(1, 100);
    latest.replace(1, frame(7, 1)).unwrap();
    latest.replace(1, frame(7, 2)).unwrap();
    latest.replace(1, frame(8, 3)).unwrap();
    assert!(latest.due(109).is_empty());
    let values = latest.due(110);
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].frame, vec![2; 528]);
    assert_eq!(values[1].frame, vec![3; 528]);
    latest.replace(1, frame(7, 4)).unwrap();
    latest.set_gates(Gates {
        paused: true,
        ..Gates::running()
    });
    assert!(latest.due(150).is_empty());
    latest.set_gates(Gates::running());
    assert!(latest.due(190).is_empty());
    latest.replace(1, frame(7, 5)).unwrap();
    latest.change_generation(2).unwrap();
    assert!(latest.replace(1, frame(7, 6)).is_err());
    assert!(latest.due(230).is_empty());
    assert!(latest.change_generation(2).is_err());
    latest.stop();
    assert!(latest.replace(2, frame(7, 7)).is_err());
    assert!(latest.due(500).is_empty());
    for gates in [
        Gates {
            driver_enabled: false,
            ..Gates::running()
        },
        Gates {
            host_frames: false,
            ..Gates::running()
        },
        Gates {
            lighting_off: true,
            ..Gates::running()
        },
    ] {
        assert!(!gates.forwards());
    }
}

#[test]
fn marci_mode_reads_before_write_and_exit_uses_manifest_up_conversion() {
    let hid = Fake::new(true);
    let mut control = Control::default();
    hid.reply(2, &[3, 0], None);
    assert!(!control.mode(&hid, 3, &mut |_| Ok(())).unwrap());
    assert_eq!(hid.sent.borrow().len(), 1);
    hid.reply(2, &[3, 0], None);
    hid.reply(2, &[], None);
    assert!(control.mode(&hid, 0, &mut |_| Ok(())).unwrap());
    hid.reply(2, &[], None);
    control.exit_wave(&hid, &mut |_| Ok(())).unwrap();
    let sent = hid.sent.borrow();
    assert_eq!(&sent[0][6..9], &[2, 0, 132]);
    assert_eq!(&sent[2][6..11], &[2, 0, 4, 0, 0]);
    assert_eq!(&sent[3][6..15], &[6, 15, 2, 1, 0, 4, 0, 40, 0]);
    assert_eq!(sent[3][89], 0x26);
    assert_eq!((sent[0][0], sent[0][2], sent[3][2]), (16, 1, 4));
}

#[test]
fn marci_control_busy_timeout_and_other_command_retry_in_before_resend_out() {
    let hid = Fake::new(true);
    let mut control = Control::default();
    let mut waits = Vec::new();
    hid.reply(1, &[], None);
    hid.reply(2, &[1, 2, 3, 4], None);
    control
        .firmware(&hid, &mut |d| {
            waits.push(d.as_millis());
            Ok(())
        })
        .unwrap();
    assert_eq!(waits, vec![0, 40, 5]);
    assert_eq!(hid.sent.borrow().len(), 1);
    hid.reply(4, &[], None);
    hid.reply(2, &[1, 2, 3, 4], None);
    control.firmware(&hid, &mut |_| Ok(())).unwrap();
    assert_eq!(hid.sent.borrow().len(), 2);
    hid.reply(2, &[], Some(4));
    hid.reply(2, &[1, 2, 3, 4], None);
    control.firmware(&hid, &mut |_| Ok(())).unwrap();
    assert_eq!(hid.sent.borrow().len(), 3);
    hid.reply(5, &[], None);
    assert!(control.firmware(&hid, &mut |_| Ok(())).is_err());
    assert_eq!(hid.sent.borrow().len(), 4);
}

#[test]
fn marci_control_rejects_truncation_wrong_channel_and_cancellation() {
    let mut hid = Fake::new(true);
    hid.reply(2, &[1, 2, 3, 4], None);
    hid.read_len.set(90);
    assert!(Control::default().firmware(&hid, &mut |_| Ok(())).is_err());
    hid.read_len.set(91);
    hid.feature_len.set(90);
    assert!(Control::default().firmware(&hid, &mut |_| Ok(())).is_err());
    hid.feature_len.set(91);
    hid.node.usage_page = 0xff14;
    assert!(Control::default().firmware(&hid, &mut |_| Ok(())).is_err());
    hid.node = node(true);
    hid.sent.borrow_mut().clear();
    assert!(
        Control::default()
            .firmware(&hid, &mut |_| bail!("canceled"))
            .is_err()
    );
    assert!(hid.sent.borrow().is_empty());
}

#[test]
fn marci_control_has_twenty_out_thirty_in_retry_bounds_and_wraps_transaction_31() {
    let hid = Fake::new(true);
    for _ in 0..600 {
        hid.reply(1, &[], None);
    }
    let error = Control::default()
        .firmware(&hid, &mut |_| Ok(()))
        .unwrap_err();
    assert!(error.to_string().contains("busy/timeout"));
    assert_eq!(hid.sent.borrow().len(), 20);
    assert_eq!(hid.read_count.get(), 600);
    let hid = Fake::new(true);
    let mut owner = Control::default();
    for _ in 0..33 {
        hid.reply(2, &[1, 0, 0, 0], None);
        owner.firmware(&hid, &mut |_| Ok(())).unwrap();
    }
    let transactions = hid
        .sent
        .borrow()
        .iter()
        .map(|report| report[2])
        .collect::<Vec<_>>();
    assert_eq!(&transactions[..31], &(1..=31).collect::<Vec<_>>());
    assert_eq!(&transactions[31..], &[1, 2]);
}

#[test]
fn marci_hardware_exit_sync_keeps_source_rgb_flags_rates_and_fallback_settings() {
    let cases = [
        (
            1,
            serde_json::json!({"color1":"#112233"}),
            vec![1, 0, 1, 0, 0, 1, 17, 34, 51],
        ),
        (
            1,
            serde_json::json!({"color1":"#000000"}),
            vec![1, 0, 1, 0, 0, 0],
        ),
        (
            2,
            serde_json::json!({"color1":"no-color","color2":"#112233"}),
            vec![1, 0, 2, 1, 0, 1, 17, 34, 51],
        ),
        (
            2,
            serde_json::json!({"isRandom":true}),
            vec![1, 0, 2, 3, 0, 0],
        ),
        (3, serde_json::json!({}), vec![1, 0, 3, 0, 0, 0]),
        (
            4,
            serde_json::json!({"direction":1,"speed":1}),
            vec![1, 0, 4, 2, 75, 0],
        ),
        (
            5,
            serde_json::json!({"color1":"#112233","duration":2}),
            vec![1, 0, 5, 0, 2, 1, 17, 34, 51],
        ),
        (
            6,
            serde_json::json!({"color1":"#112233"}),
            vec![1, 0, 6, 0, 40, 1, 17, 34, 51],
        ),
        (
            7,
            serde_json::json!({"color1":"#112233","color2":"#445566","duration":3}),
            vec![1, 0, 7, 2, 3, 2, 17, 34, 51, 68, 85, 102],
        ),
        (
            7,
            serde_json::json!({"isRandom":true}),
            vec![1, 0, 7, 3, 0, 0],
        ),
        (8, serde_json::json!({}), vec![1, 0, 9, 0, 90, 1, 255, 0, 0]),
        (
            11,
            serde_json::json!({"screen":"top"}),
            vec![1, 0, 4, 1, 40, 0],
        ),
        (12, serde_json::json!({"flow":1}), vec![1, 0, 4, 1, 40, 0]),
        (
            13,
            serde_json::json!({"direction":2,"speed":3}),
            vec![1, 0, 4, 1, 15, 0],
        ),
        (
            19,
            serde_json::json!({"direction":0}),
            vec![1, 0, 4, 1, 40, 0],
        ),
    ];
    for (id, settings, expected) in cases {
        let effect = QuickEffect::parse(&serde_json::json!({
            "selectedEffectId":id,"selectedEffectSetting":settings
        }))
        .unwrap();
        let target = exit_effect_for_quick(&effect).unwrap();
        assert_eq!(
            target.protocol25_payload(1, 0).unwrap(),
            expected,
            "UI {id}"
        );
    }
    let hid = Fake::new(true);
    hid.reply(2, &[], None);
    let effect = QuickEffect::parse(&serde_json::json!({
        "selectedEffectId":1,"selectedEffectSetting":{"color1":"#112233"}
    }))
    .unwrap();
    Control::default()
        .exit_effect(&hid, &exit_effect_for_quick(&effect).unwrap(), &mut |_| {
            Ok(())
        })
        .unwrap();
    assert_eq!(
        &hid.sent.borrow()[0][6..18],
        &[9, 15, 2, 1, 0, 1, 0, 0, 1, 17, 34, 51]
    );
}

#[test]
fn marci_manifest_has_sixty_mappings_without_filling_other_report_columns() {
    let config = led_configuration();
    assert_eq!(config.led_matrix.len(), 60);
    assert_eq!(
        (
            config.matrix_max_row,
            config.matrix_max_col,
            config.device_max_row,
            config.device_max_col
        ),
        (31, 32, 1, 176)
    );
    assert_eq!(
        (
            config.led_matrix[0].matrix_pos,
            config.led_matrix[0].device_pos
        ),
        ([29, 14], [0, 88])
    );
    assert_eq!(
        (
            config.led_matrix[29].matrix_pos,
            config.led_matrix[29].device_pos
        ),
        ([0, 15], [0, 117])
    );
    assert_eq!(
        (
            config.led_matrix[30].matrix_pos,
            config.led_matrix[30].device_pos
        ),
        ([29, 17], [0, 0])
    );
    assert_eq!(
        (
            config.led_matrix[59].matrix_pos,
            config.led_matrix[59].device_pos
        ),
        ([0, 16], [0, 29])
    );
    assert!(config.led_input_map.is_empty());
    assert!(config.led_regions.is_empty());
}

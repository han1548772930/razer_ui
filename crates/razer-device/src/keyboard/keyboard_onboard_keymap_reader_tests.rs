use super::*;
use crate::keyboard_control_test_transport::{
    Mock, Reply, assert_report_integrity, reply, sequence,
};

const KEYS: [u8; 30] = [
    1, 2, 3, 4, 5, 6, 16, 17, 18, 19, 20, 21, 24, 30, 31, 32, 33, 34, 35, 44, 46, 47, 48, 49, 50,
    58, 59, 61, 217, 218,
];

fn capability() -> DeviceReadCapability {
    let mut cap = crate::keyboard_metadata::capability(747).unwrap().clone();
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap
}

fn initial_replies() -> Vec<Reply> {
    let mut bytes = [0; 120];
    bytes[..16].copy_from_slice(&[
        0x44, 0x33, 0x22, 0x11, 0x66, 0x55, 0x88, 0x77, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0,
    ]);
    bytes[16..20].copy_from_slice(b"read");
    let mut first = vec![2, 3, 0, 120];
    first.extend_from_slice(&bytes[..64]);
    let mut second = vec![2, 3, 64, 120];
    second.extend_from_slice(&bytes[64..]);
    vec![
        reply(&[2, 1, 3]),
        reply(&[2, 3]),
        reply(&[3]),
        reply(&first),
        reply(&second),
    ]
}

fn mode_replies(mode: u8, threshold: u16) -> Vec<Reply> {
    let mut replies = vec![reply(&[3])];
    for keys in KEYS.chunks(8) {
        let mut bytes = vec![2];
        for key in keys {
            bytes.extend_from_slice(&[*key, mode, 2, 2, 0, *key, 0, 0, 0]);
        }
        replies.push(reply(&bytes));
    }
    for keys in KEYS.chunks(13) {
        let mut bytes = vec![0, 2];
        for key in keys {
            bytes.extend_from_slice(&[*key, 1]);
            bytes.extend_from_slice(&threshold.to_be_bytes());
            bytes.extend_from_slice(&(threshold + 1).to_be_bytes());
        }
        replies.push(reply(&bytes));
    }
    for keys in KEYS.chunks(15) {
        let mut bytes = vec![0, 2];
        for key in keys {
            bytes.push(*key);
            bytes.extend_from_slice(&(threshold + 1).to_be_bytes());
            bytes.extend_from_slice(&(threshold + 2).to_be_bytes());
        }
        replies.push(reply(&bytes));
    }
    replies
}

#[test]
fn keypad_profile_reads_both_modes_and_analog_batches_without_extra_target_writes() {
    let mut replies = initial_replies();
    replies.extend(mode_replies(0, 1000));
    replies.extend(mode_replies(1, 2000));
    let device = Mock::new(replies);
    let reading = read(&device, &capability(), 2, 1, sequence(), &|| Ok(())).unwrap();
    assert_eq!(reading.inventory.observed().unwrap().keymap_ids(), [3]);
    assert_eq!(reading.active_keymap.observed(), Some(&3));
    assert_eq!(reading.selections.len(), 3);
    assert!(
        reading
            .selections
            .iter()
            .all(|selection| selection.attempted
                && selection.acknowledgement.as_deref() == Some(&[3]))
    );
    assert_eq!(
        reading.selections[1].phase,
        SelectionPhase::Assignments(Mode::Normal)
    );
    assert_eq!(
        reading.selections[2].phase,
        SelectionPhase::Assignments(Mode::Hypershift)
    );
    let keymap = &reading.keymaps[0];
    assert_eq!(keymap.name.observed().unwrap().name(), "read");
    assert_eq!(
        keymap.passes[0].actuation[0].reading.observed().unwrap()[0].threshold_low,
        1002
    );
    assert_eq!(
        keymap.passes[1].actuation[0].reading.observed().unwrap()[0].threshold_low,
        2002
    );
    let commands = device
        .sent()
        .iter()
        .map(|bytes| (bytes[7], bytes[8]))
        .collect::<Vec<_>>();
    let mut expected = vec![(2, 178), (2, 182), (2, 55), (2, 181), (2, 181)];
    for _ in 0..2 {
        expected.push((2, 55));
        expected.extend([(2, 158); 4]);
        expected.extend([(2, 154); 3]);
        expected.extend([(2, 153); 2]);
    }
    assert_eq!(commands, expected);
    assert_report_integrity(&device.sent());
}

#[test]
fn keypad_target_failure_retains_prior_observations_and_does_not_read_another_scope() {
    let mut failed = reply(&[]);
    failed.status = 5;
    let device = Mock::new([reply(&[2, 1, 3]), reply(&[2, 3]), failed]);
    let failure = read(&device, &capability(), 2, 1, sequence(), &|| Ok(())).unwrap_err();
    let reading = failure.snapshot();
    assert_eq!(reading.inventory.observed().unwrap().keymap_ids(), [3]);
    assert_eq!(reading.active_keymap.observed(), Some(&3));
    assert!(reading.selections[0].attempted);
    assert!(reading.selections[0].acknowledgement.is_none());
    assert!(matches!(reading.keymaps[0].name, Reading::Unobserved));
    assert!(reading.keymaps[0].passes.is_empty());
    assert_eq!(device.sent().len(), 3);
}

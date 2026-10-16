use super::*;
use crate::keyboard_control_test_transport::{Mock, assert_report_integrity, reply, sequence};
fn cap() -> DeviceReadCapability {
    let mut cap = capability(740).unwrap();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap
}
#[test]
fn magnetic_bottom_enters_test_mode_and_stop_restores_driver_mode() {
    let device = Mock::new([reply(&[]), reply(&[])]);
    let report = execute(&device, &cap(), &Operation::Bottom(31), sequence(), &|| {
        Ok(())
    })
    .unwrap();
    assert!(report.completed);
    assert_eq!(report.messages[0].payload, true);
    let sent = device.sent();
    assert_eq!(&sent[0][6..11], &[2, 0, 4, 2, 0]);
    assert_eq!(&sent[1][6..10], &[1, 254, 46, 31]);
    assert_report_integrity(&sent);
    let device = Mock::new([reply(&[])]);
    let report = execute(&device, &cap(), &Operation::Stop, sequence(), &|| Ok(())).unwrap();
    assert_eq!(
        report.messages[0].payload,
        json!({"status":"stop","isSuccess":true})
    );
    assert_eq!(&device.sent()[0][6..11], &[2, 0, 4, 3, 0]);
}
#[test]
fn getter_uses_source_second_byte_without_an_invented_echo_check() {
    let device = Mock::new([reply(&[99, 0])]);
    let report = execute(
        &device,
        &cap(),
        &Operation::VerifyTop(31),
        sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(report.verified_top);
    assert_eq!(&device.sent()[0][6..11], &[2, 254, 173, 31, 0]);
    assert_eq!(device.sent().len(), 1);
    for bytes in [&[31, 1][..], &[31][..], &[][..]] {
        let device = Mock::new([reply(bytes), reply(&[])]);
        let report = execute(
            &device,
            &cap(),
            &Operation::VerifyBottom(31),
            sequence(),
            &|| Ok(()),
        )
        .unwrap();
        assert!(!report.verified_top);
        assert_eq!(
            report.messages.last().unwrap().payload,
            json!({"direction":"bottom","isSuccess":false})
        );
        assert_eq!(&device.sent()[1][6..11], &[2, 0, 4, 3, 0]);
    }
}
#[test]
fn source_key_alias_layout_and_record_types_are_preserved() {
    for pid in [740, 746] {
        assert_eq!(key_id(pid, "KEY_FN", false).unwrap(), 59);
        assert_eq!(key_id(pid, "KEY_BACKSLASH", true).unwrap(), 42);
        assert_eq!(
            key_id(pid, "KEY_WINDOWS", false).unwrap(),
            key_id(pid, "KEY_LEFT_GUI", false).unwrap()
        );
        assert_eq!(key_record(pid, "KEY_A").unwrap(), ("KEY_A", "Alphanumeric"));
        assert_eq!(key_record(pid, "KEY_FN").unwrap(), ("KEY_FN", "KeyInput"));
    }
    assert!(key_id(679, "KEY_A", false).is_err());
    let device = Mock::new([]);
    let report = execute(&device, &cap(), &Operation::Bottom(31), sequence(), &|| {
        anyhow::bail!("canceled")
    })
    .unwrap();
    assert!(!report.completed);
    assert!(report.error.is_some());
    assert!(device.sent().is_empty());
}

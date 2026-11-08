//! Pure enumerator fixtures: no HID, SetupDi, local socket or service calls.
use super::*;

#[test]
fn local_eq_monitor_always_finishes_and_preserves_both_failures() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Events<'a> {
        active: &'a AtomicBool,
        calls: Vec<&'static str>,
        fail_begin: bool,
        fail_read: bool,
        fail_close: bool,
    }
    impl razer_device::receiver_pairing::HardwareEvents for Events<'_> {
        fn begin(&mut self) -> Result<()> {
            self.calls.push("begin");
            ensure!(!self.fail_begin, "begin failure");
            Ok(())
        }
        fn receive(&mut self, _: Duration) -> Result<Option<Vec<u8>>> {
            self.calls.push("receive");
            ensure!(!self.fail_read, "read failure");
            self.active.store(false, Ordering::Release);
            Ok(None)
        }
        fn finish(&mut self) -> Result<()> {
            self.calls.push("finish");
            ensure!(!self.fail_close, "close failure");
            Ok(())
        }
    }
    for (begin, read, close) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, true, true),
    ] {
        let active = AtomicBool::new(true);
        let mut events = Events {
            active: &active,
            calls: vec![],
            fail_begin: begin,
            fail_read: read,
            fail_close: close,
        };
        let result = monitor_eq_events(&mut events, &active, |_| panic!("no report expected"));
        assert_eq!(events.calls.last(), Some(&"finish"));
        assert_eq!(result.is_err(), begin || read || close);
        if read && close {
            let text = result.unwrap_err().to_string();
            assert!(text.contains("read failure") && text.contains("close failure"));
        }
    }
}

#[test]
fn local_eq_monitor_report_failure_and_cancellation_release_the_subscription() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Events<'a> {
        active: &'a AtomicBool,
        calls: Vec<&'static str>,
        cancel_on_receive: bool,
        fail_close: bool,
        malformed: bool,
    }
    impl razer_device::receiver_pairing::HardwareEvents for Events<'_> {
        fn begin(&mut self) -> Result<()> {
            self.calls.push("begin");
            Ok(())
        }
        fn receive(&mut self, _: Duration) -> Result<Option<Vec<u8>>> {
            self.calls.push("receive");
            assert_eq!(self.calls.len(), 2, "failed delivery must stop listening");
            if self.cancel_on_receive {
                self.active.store(false, Ordering::Release);
            }
            let mut raw = vec![0; 64];
            raw[0] = 2;
            raw[10] = 96;
            raw[11] = 2;
            raw[13] = if self.malformed { 9 } else { 4 };
            Ok(Some(raw))
        }
        fn finish(&mut self) -> Result<()> {
            self.calls.push("finish");
            ensure!(!self.fail_close, "close failure");
            Ok(())
        }
    }
    for (initially_active, cancel_on_receive, malformed, fail_close) in [
        (true, false, false, false),
        (true, false, false, true),
        (true, false, true, true),
        (true, true, false, false),
        (false, false, false, false),
    ] {
        let active = AtomicBool::new(initially_active);
        let mut events = Events {
            active: &active,
            calls: vec![],
            cancel_on_receive,
            fail_close,
            malformed,
        };
        let mut publications = 0;
        let result = monitor_eq_events(&mut events, &active, |event| {
            publications += 1;
            assert_eq!(
                event,
                razer_device::thx_submission::local_eq::Notification::Selected { index: 4 }
            );
            anyhow::bail!("publication failure")
        });
        assert_eq!(events.calls.last(), Some(&"finish"));
        if !initially_active {
            assert_eq!(events.calls, ["finish"]);
        } else {
            assert_eq!(events.calls, ["begin", "receive", "finish"]);
        }
        if initially_active && !cancel_on_receive {
            let error = format!("{:#}", result.unwrap_err());
            assert_eq!(publications, usize::from(!malformed));
            assert!(error.contains(if malformed {
                "slot is out of range"
            } else {
                "publication failure"
            }));
            if fail_close {
                assert!(error.contains("close failure"));
            }
        } else {
            assert_eq!(publications, 0);
            assert!(result.is_ok());
        }
    }
}

fn scope() -> ThxSubmissionScope {
    ThxSubmissionScope {
        product_id: 1398,
        container_id: "container-a".into(),
        serial_number: "serial-a".into(),
        profile_id: "local-profile".into(),
        generation: 7,
    }
}
fn node(pid: u16, path: &str, serial: Option<&str>) -> HidNode {
    HidNode {
        path: path.as_bytes().to_vec(),
        vendor_id: 0x1532,
        product_id: pid,
        usage_page: 0xff00,
        usage: 1,
        interface_number: 5,
        release_number: 1,
        serial_number: serial.map(str::to_owned),
        manufacturer: None,
        product: None,
    }
}
#[test]
fn thx_collection_selects_exact_wired_serial_and_case_insensitive_container() {
    let exact = node(1398, "fixture-a", Some("serial-a"));
    let mut wrong_interface = exact.clone();
    wrong_interface.interface_number = 4;
    let unrelated = node(1401, "fixture-other-product", None);
    assert_eq!(
        resolve_candidates(
            &scope(),
            &[wrong_interface, unrelated, exact.clone()],
            |n| {
                assert_eq!(n.path, b"fixture-a");
                Ok("CONTAINER-A".into())
            }
        )
        .unwrap(),
        exact
    );
}
#[test]
fn thx_collection_refuses_wired_serial_mismatch() {
    let nodes = [node(1398, "fixture-a", Some("different-serial"))];
    let error = resolve_candidates(&scope(), &nodes, |_| Ok("container-a".into())).unwrap_err();
    assert!(error.to_string().contains("serial differs"));
}
#[test]
fn thx_collection_dongle_serial_does_not_impersonate_owner_serial() {
    let exact = node(1399, "fixture-dongle", Some("receiver-serial"));
    assert_eq!(
        resolve_candidates(&scope(), &[exact.clone()], |_| Ok("container-a".into())).unwrap(),
        exact
    );
}
#[test]
fn thx_collection_duplicate_paths_and_same_container_collections_are_ambiguous() {
    let exact = node(1398, "fixture-a", Some("serial-a"));
    for second in [exact.clone(), node(1399, "fixture-dongle", None)] {
        let error = resolve_candidates(&scope(), &[exact.clone(), second], |_| {
            Ok("container-a".into())
        })
        .unwrap_err();
        assert!(error.to_string().contains("found 2"));
    }
}
#[test]
fn thx_collection_second_container_prevents_ambiguous_product_rpc() {
    let nodes = [node(1398, "a", Some("serial-a")), node(1399, "b", None)];
    let error = resolve_candidates(&scope(), &nodes, |n| {
        Ok(if n.path == b"a" {
            "container-a"
        } else {
            "container-b"
        }
        .into())
    })
    .unwrap_err();
    assert!(error.to_string().contains("product/endpoint"));
}

#[test]
fn roller_exact_container_is_independent_of_ambiguous_product_rpc() {
    let expected = node(1398, "a", Some("serial-a"));
    let nodes = [expected.clone(), node(1399, "b", None)];
    let resolved = resolve_candidates_for(&scope(), &nodes, false, |n| {
        Ok(if n.path == b"a" {
            "container-a"
        } else {
            "container-b"
        }
        .into())
    })
    .unwrap();
    assert_eq!(resolved, expected);
}
#[test]
fn thx_collection_incomplete_candidate_identity_cannot_hide_another_container() {
    let nodes = [node(1398, "a", Some("serial-a")), node(1399, "b", None)];
    let error = resolve_candidates(&scope(), &nodes, |n| {
        if n.path == b"a" {
            Ok("container-a".into())
        } else {
            anyhow::bail!("synthetic identity query failure")
        }
    })
    .unwrap_err();
    assert!(error.to_string().contains("identity incomplete"));
}

//! Pure enumerator fixtures: no HID, SetupDi, local socket or service calls.
use super::*;

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

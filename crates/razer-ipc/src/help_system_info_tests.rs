use super::*;
use razer_device::system_info::Query;

#[test]
fn help_system_info_request_preserves_the_fixed_internal_wire_contract() {
    let bytes = br#"{"operation":"help_system_info","product_id":737,"query":"full"}"#;
    let request: ServiceRequest = serde_json::from_slice(bytes).unwrap();
    assert!(matches!(
        request,
        ServiceRequest::HelpSystemInfo {
            product_id: 737,
            query: Query::Full
        }
    ));
    assert_eq!(serde_json::to_vec(&request).unwrap(), bytes);
    assert!(request.synchronous_device_task().unwrap().is_none());
    for invalid in [
        br#"{"operation":"help_system_info","query":"full"}"#.as_slice(),
        br#"{"operation":"help_system_info","product_id":737,"query":"guess"}"#,
    ] {
        assert!(serde_json::from_slice::<ServiceRequest>(invalid).is_err());
    }
}

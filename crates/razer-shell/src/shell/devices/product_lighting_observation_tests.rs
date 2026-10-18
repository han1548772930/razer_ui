use super::*;

#[::core::prelude::v1::test]
fn mouse_host_retains_independent_values_when_profile_and_installation_fail() {
    let readback = product_chroma::Readback {
        snapshot: Err("invalid profile".into()),
        dynamic_lighting: Ok(Some(true)),
        module_installed: Ok(Some(false)),
        installation: Err("installation failed".into()),
    };
    assert_eq!(
        mouse_host(&readback),
        json!({
            "isDynamicLighting": true,
            "chromaInstalledModule": false,
        })
    );
    assert!(readback.result().is_err());
}

#[::core::prelude::v1::test]
fn accepted_brightness_keeps_exit_pending_until_its_worker_releases_the_receipt() {
    let mut state = State::default();
    let (observation_id, observation) = state.accept(false, false).unwrap();
    drop(observation);
    assert!(!state.pending());

    // The profile write has completed and the caller requests exit before
    // device submission. Its accepted write still enters the retained owner.
    let (brightness_id, brightness) = state.accept(true, true).unwrap();
    assert!(brightness_id > observation_id);
    assert!(state.pending());
    assert!(state.accept(true, false).is_none());
    assert!(state.pending());

    // Background shutdown stops admission, but a still-owned receipt cannot
    // make its in-flight cleanup disappear from the exit decision.
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.pending());
    assert!(state.accept(false, true).is_none());
    assert!(state.accept(true, true).is_none());
    drop(brightness);
    assert!(!state.pending());
}

#[::core::prelude::v1::test]
fn concurrent_brightness_receipts_release_only_their_own_exit_work() {
    let mut state = State::default();
    let (first_id, first) = state.accept(false, true).unwrap();
    let (second_id, second) = state.accept(true, true).unwrap();
    assert_ne!(first_id, second_id);
    drop(first);
    assert!(state.pending());
    drop(second);
    assert!(!state.pending());
}

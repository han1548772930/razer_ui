use super::{CYCLE_ARTWORK, CycleSelection, CycleState, CycleTarget};

#[test]
fn cycle_hover_mounts_artwork_and_leave_restores_the_source_endpoint() {
    let mut cycle = CycleState::default();
    assert!(!cycle.shown);
    assert_eq!(cycle.target.coordinates(), (214., 169.));
    cycle.hover(7);
    assert!(cycle.shown);
    assert_eq!(cycle.target.coordinates(), (186., 173.));

    cycle.leave(CycleSelection::Empty);
    assert_eq!(cycle, CycleState::default());
    cycle.hover(4);
    assert_eq!(cycle, CycleState::default());
}

#[test]
fn selected_cycle_survives_pointer_leave_until_its_editor_is_cleared() {
    let mut cycle = CycleState::default();
    cycle.hover(7);
    cycle.selection_changed(CycleSelection::Empty, CycleSelection::Cycle, true);
    cycle.leave(CycleSelection::Cycle);
    assert!(cycle.shown);
    assert_eq!(cycle.target, CycleTarget::Magnified);

    cycle.hover(4);
    cycle.leave(CycleSelection::Cycle);
    assert!(cycle.shown);
    cycle.selection_changed(CycleSelection::Cycle, CycleSelection::Empty, false);
    assert_eq!(cycle, CycleState::default());
}

#[test]
fn selecting_cycle_from_the_panel_does_not_invent_a_hover_event() {
    let mut cycle = CycleState::default();
    cycle.selection_changed(CycleSelection::Empty, CycleSelection::Cycle, false);
    assert_eq!(cycle, CycleState::default());

    // CO.componentDidUpdate restores only the same-cycle update branch.
    cycle.selection_changed(CycleSelection::Cycle, CycleSelection::Cycle, false);
    assert!(cycle.shown);
    assert_eq!(cycle.target, CycleTarget::Magnified);
}

#[test]
fn immediate_image_selection_keeps_the_source_hidden_magnified_coordinates() {
    let mut cycle = CycleState::default();
    cycle.hover(7);
    cycle.selection_changed(CycleSelection::Cycle, CycleSelection::Other, true);
    assert!(!cycle.shown);
    assert_eq!(cycle.target, CycleTarget::Magnified);
    cycle.leave(CycleSelection::Other);
    assert_eq!(cycle, CycleState::default());
}

#[test]
fn accepting_a_deferred_selection_does_not_replay_the_immediate_click_branch() {
    let mut cycle = CycleState::default();
    cycle.hover(7);
    cycle.selection_changed(CycleSelection::Cycle, CycleSelection::Other, false);
    assert!(cycle.shown);
    assert_eq!(cycle.target, CycleTarget::Magnified);
    cycle.leave(CycleSelection::Other);
    assert_eq!(cycle, CycleState::default());
}

#[test]
fn current_inline_cycle_artwork_has_a_packaged_runtime_alias() {
    let resources = razer_assets::list_resources("synapse/mouse-customize/")
        .expect("packaged mouse artwork catalog");
    assert!(
        resources
            .iter()
            .any(|resource| resource.as_ref() == CYCLE_ARTWORK)
    );
}

use super::HoverState;

#[test]
fn dpi_row_hover_initial_mount_has_no_actions_or_xy_tooltip() {
    let hover = HoverState::default();
    for index in 0..5 {
        assert!(!hover.row_is_hovered(index));
        assert!(!hover.xy_is_hovered(index));
    }
}

#[test]
fn dpi_row_hover_only_the_current_row_can_show_its_xy_tooltip() {
    let mut hover = HoverState::default();
    assert!(!hover.set_xy(2, true));
    assert!(hover.set_row(2, true));
    assert!(!hover.set_xy(1, true));
    assert!(hover.set_xy(2, true));
    assert!(hover.row_is_hovered(2));
    assert!(hover.xy_is_hovered(2));
    assert!(!hover.row_is_hovered(1));
    assert!(!hover.xy_is_hovered(1));
}

#[test]
fn dpi_row_hover_late_leave_from_the_previous_row_preserves_current_actions() {
    let mut hover = HoverState::default();
    hover.set_row(0, true);
    hover.set_xy(0, true);
    assert!(hover.set_row(3, true));
    assert!(hover.set_xy(3, true));
    assert!(!hover.set_row(0, false));
    assert!(!hover.set_xy(0, false));
    assert!(hover.row_is_hovered(3));
    assert!(hover.xy_is_hovered(3));
}

#[test]
fn dpi_row_hover_changing_rows_clears_the_previous_tooltip() {
    let mut hover = HoverState::default();
    hover.set_row(1, true);
    hover.set_xy(1, true);
    assert!(hover.set_row(4, true));
    assert!(!hover.row_is_hovered(1));
    assert!(!hover.xy_is_hovered(1));
    assert!(!hover.xy_is_hovered(4));
    assert!(hover.row_is_hovered(4));
}

#[test]
fn dpi_row_hover_leaving_xy_preserves_the_row_actions() {
    let mut hover = HoverState::default();
    hover.set_row(2, true);
    hover.set_xy(2, true);
    assert!(!hover.set_row(2, true));
    assert!(hover.xy_is_hovered(2));
    assert!(hover.set_xy(2, false));
    assert!(hover.row_is_hovered(2));
    assert!(!hover.xy_is_hovered(2));
}

#[test]
fn dpi_row_hover_leaving_the_row_clears_actions_and_tooltip() {
    let mut hover = HoverState::default();
    hover.set_row(2, true);
    hover.set_xy(2, true);
    assert!(hover.set_row(2, false));
    assert_eq!(hover, HoverState::default());
    assert!(!hover.set_xy(2, true));
}

#[test]
fn dpi_row_hover_session_reset_and_drag_start_release_all_hover_state() {
    let mut hover = HoverState::default();
    hover.set_row(2, true);
    hover.set_xy(2, true);
    hover.clear();
    assert_eq!(hover, HoverState::default());
    assert!(!hover.set_xy(2, false));
    assert!(!hover.set_row(2, false));
    assert!(hover.set_row(2, true));
    assert!(!hover.xy_is_hovered(2));
}

use super::*;

#[::core::prelude::v1::test]
fn collapsed_function_rows_keep_their_icon_in_the_leading_slot() {
    for active in [false, true] {
        let mut button = function_button("DEFAULT", active);
        let style = button.style();
        assert_eq!(style.justify_content, Some(JustifyContent::Start));
        assert_eq!(style.flex_shrink, Some(0.));
        assert_eq!(style.size.height, Some(surface::css(40.).into()));
    }
}

#[::core::prelude::v1::test]
fn image_click_uses_the_source_button_side_without_fabricating_an_anchor() {
    let root = Bounds::new(point(px(1.), px(126.)), size(px(1430.), px(680.)));
    assert_eq!(
        popup_left(root, px(1.), PopupSide::Left, false, false),
        px(94.)
    );
    assert_eq!(
        popup_left(root, px(1.), PopupSide::Right, false, false),
        px(1036.)
    );
}

#[::core::prelude::v1::test]
fn panel_origin_and_image_origin_keep_their_distinct_source_rules() {
    let root = Bounds::new(point(px(18.), px(110.)), size(px(1280.), px(650.)));
    assert_eq!(
        popup_left(root, px(1.), PopupSide::Left, true, false),
        px(151.)
    );
    assert_eq!(
        popup_left(root, px(1.), PopupSide::Right, true, false),
        px(228.)
    );
    for side in [PopupSide::Left, PopupSide::Right] {
        assert_eq!(popup_left(root, px(1.), side, true, true), px(248.));
    }
}

#[::core::prelude::v1::test]
fn source_class_placement_scales_without_an_invented_viewport_clamp() {
    let root = Bounds::new(point(px(50.), px(100.)), size(px(1500.), px(850.)));
    assert_eq!(
        popup_left(root, px(1.25), PopupSide::Left, false, false),
        px(22.5)
    );
    let narrow = Bounds::new(point(px(25.), px(100.)), size(px(900.), px(650.)));
    assert_eq!(
        popup_left(narrow, px(1.), PopupSide::Left, false, false),
        px(-147.)
    );
}

#[::core::prelude::v1::test]
fn cycle_behavior_uses_the_button_key_while_selection_keeps_the_input_id() {
    let cycle = json!({
        "buttonKey": "CycleUpSensitivityStages",
        "inputID": "DKM_SB_03",
        "buttonPos": "left"
    });
    assert!(is_cycle_button(&cycle));
    assert_eq!(PopupSide::from_button(Some(&cycle)), PopupSide::Left);
    let other = json!({"buttonKey":"ScrollUp","inputID":"ScrollUp","buttonPos":"right"});
    assert!(!is_cycle_button(&other));
    assert_eq!(PopupSide::from_button(Some(&other)), PopupSide::Right);
    assert_eq!(PopupSide::from_button(None), PopupSide::Left);
}

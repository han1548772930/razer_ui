use super::labels::{LabelTip, label_tip, scroll_mode_phase};
use super::{MouseProductWorkspace, read_mouse_customize_current_catalog, source_surface};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};
use serde_json::json;

#[::core::prelude::v1::test]
fn mouse_label_tip_priority_and_scroll_animation_follow_current_source() {
    let mut button = json!({
        "isRemappedDisabled":true,"assignmentValue":"LEFT_CLICK",
        "assignment":"INTERDEVICE","isTooltipVisible":true,"toolTip":"DESCRIPTION"
    });
    assert_eq!(label_tip(&button, true), Some(LabelTip::RemappingBlocked));
    button["isRemappedDisabled"] = json!(false);
    assert_eq!(
        label_tip(&button, true),
        Some(LabelTip::PrimaryClickBlocked)
    );
    assert_eq!(label_tip(&button, false), Some(LabelTip::InterDevice));
    button["assignmentValue"] = json!("SCROLL_MODE_SWITCH");
    assert_eq!(label_tip(&button, false), Some(LabelTip::ScrollMode));
    button["assignment"] = json!("MOUSE");
    button["assignmentValue"] = json!("RIGHT_CLICK");
    assert_eq!(label_tip(&button, true), Some(LabelTip::Text));
    button["isTooltipVisible"] = json!(false);
    assert_eq!(label_tip(&button, false), None);
    for phase in [0., 0.25, 0.469] {
        assert_eq!(scroll_mode_phase(phase), 0.);
    }
    for phase in [0.5, 0.7, 0.969] {
        assert_eq!(scroll_mode_phase(phase), 1.);
    }
    assert!(scroll_mode_phase(0.485) > 0.5);
    assert!(scroll_mode_phase(0.985) < 0.5);
    assert_eq!(scroll_mode_phase(1.), 0.);
}

#[::core::prelude::v1::test]
fn every_cycle_branch_keeps_its_current_product_and_edition_selector() {
    let surfaces = read_mouse_customize_current_catalog();
    let branches = surfaces
        .iter()
        .filter(|surface| surface.cycle.is_some())
        .collect::<Vec<_>>();
    assert_eq!(branches.len(), 31);
    for surface in branches {
        let cycle = surface.cycle.as_ref().unwrap();
        assert!(
            !surface.cycle_images.is_empty(),
            "PID{}",
            surface.product_id
        );
        for image in &surface.cycle_images {
            let selected = surface.cycle_artwork(image.edition).unwrap();
            assert_eq!(
                selected.edition,
                cycle.fixed_edition.unwrap_or(image.edition)
            );
            assert!(selected.asset.starts_with("synapse/mouse-customize/"));
        }
        if cycle.fixed_edition.is_none() && !cycle.edition_zero_fallback {
            assert!(
                surface.cycle_artwork(u32::MAX).is_none(),
                "PID{}",
                surface.product_id
            );
        }
    }
    let viper = source_surface(229, 0).unwrap();
    for edition in [0, 128, 129, 130, 131, 132, 133, 134, u32::MAX] {
        assert_eq!(
            viper.cycle_artwork(edition).unwrap().asset,
            "synapse/mouse-customize/8bd13c4f3a94a3834d2a05c6.svg"
        );
    }
}

#[gpui_kit::test]
fn every_cycle_branch_mounts_only_for_hover_and_clears_on_leaving_page(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let surfaces = read_mouse_customize_current_catalog();
    for surface in surfaces.iter().filter(|surface| surface.cycle.is_some()) {
        let cycle = surface.cycle.as_ref().unwrap();
        let mut page = None;
        let handle = cx.open_window(size(px(1450.), px(1000.)), |window, cx| {
            let owner = cx.new(|cx| MouseProductWorkspace::new(surface.product_id, window, cx));
            page = Some(owner.clone());
            Root::new(owner, window, cx)
        });
        let owner = page.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(
                window.try_find("mouse-current-cycle-image").is_none(),
                "PID{} idle",
                surface.product_id
            );
            let before = owner.read(cx).snapshot();
            owner.update(cx, |owner, cx| {
                owner.hover_current_customize(cycle.index, true);
                cx.notify();
            });
            window.render_frame(cx);
            let image = window.find("mouse-current-cycle-image").bounds();
            let scale = f32::from(window.rem_size()) / 16.;
            assert_eq!(
                image.size,
                size(px(cycle.width * scale), px(cycle.height * scale)),
                "PID{}",
                surface.product_id
            );
            assert_eq!(owner.read(cx).snapshot(), before);
            owner.update(cx, |owner, cx| {
                owner.set_page("TAB_PERFORMANCE", window, cx);
            });
            window.render_frame(cx);
            assert!(window.try_find("mouse-current-cycle-image").is_none());
            assert!(owner.read(cx).customize_cycle.is_none());
            assert!(owner.read(cx).customize_label_hover.is_none());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn viper_v4_hover_mounts_source_cutout_and_selected_button_retains_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut page = None;
    let handle = cx.open_window(size(px(1450.), px(1000.)), |window, cx| {
        let owner = cx.new(|cx| MouseProductWorkspace::new(229, window, cx));
        page = Some(owner.clone());
        Root::new(owner, window, cx)
    });
    let owner = page.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let front = window.find("mouse-current-front-image").bounds();
        window.hover("mouse-229-input-DKM_SB_03", cx);
        let cutout = window.find("mouse-current-cycle-image").bounds();
        assert_eq!(cutout.size, size(px(102.), px(102.)));
        assert!((cutout.left() - front.left() - px(113.45)).abs() <= px(1.));
        assert!((cutout.top() - front.top() - px(115.2)).abs() <= px(1.));
        assert_eq!(window.find("mouse-current-front-image").bounds(), front);
        owner.update(cx, |owner, cx| {
            owner.set_current_customize_input(Some("DKM_SB_03".into()), true);
            owner.hover_current_customize(7, false);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("mouse-current-cycle-image").is_some());
        owner.update(cx, |owner, cx| {
            owner.set_current_customize_input(None, false);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("mouse-current-cycle-image").is_none());
    })
    .unwrap();
}

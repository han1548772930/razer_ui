use super::{
    Assignment, Category, PREFIX, assign_key_choice, canonical_key, categories,
    categories_for_layout, normalized_number, normalized_website, update_key_modifiers,
};
use crate::features::workspace::{Continue, DeviceWorkspace};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, ScrollDelta, TestAppContext, point, px, size};
use std::time::Duration;

#[gpui_kit::test]
fn mapping_categories_animate_over_a_stationary_body_and_scroll_in_a_short_window(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let handle = cx.open_window(size(px(1280.), px(440.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(crate::model::measured_devices().remove(0), true, window, cx)
        });
        Root::new(view, window, cx)
    });
    let body = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("customize-drawer-toggle", cx);
            window.click("drawer-input-RightButton", cx);
            let body = window.find("mapping-body").bounds();
            assert_eq!(
                window.find("mapping-categories").bounds().size.width,
                px(40.)
            );
            cx.set_reduce_motion(false);
            window.hover("mapping-category-default", cx);
            window.render_frame(cx);
            assert_eq!(
                window.find("mapping-categories").bounds().size.width,
                px(40.)
            );
            body
        })
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let width = window.find("mapping-categories").bounds().size.width;
        assert!(width > px(40.) && width < px(250.));
        assert_eq!(window.find("mapping-body").bounds(), body);
        window.hover("mapping-close", cx);
        window.render_frame(cx);
        assert!((window.find("mapping-categories").bounds().size.width - width).abs() < px(1.));
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("mapping-categories").bounds().size.width,
            px(40.)
        );
        window.hover("mapping-category-default", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let rail = window.find("mapping-categories").bounds();
        assert_eq!(rail.size.width, px(250.));
        let first_top = window.find("mapping-category-default").bounds().top();
        assert!(window.find("mapping-category-disable").bounds().bottom() > rail.bottom());
        window.scroll(
            "mapping-categories",
            ScrollDelta::Pixels(point(px(0.), px(-2000.))),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find("mapping-category-default").bounds().top() < first_top);
        assert!(window.find("mapping-category-disable").bounds().bottom() <= rail.bottom());
        assert_eq!(window.find("mapping-body").bounds(), body);
        window.scroll(
            "mapping-categories",
            ScrollDelta::Pixels(point(px(0.), px(2000.))),
            cx,
        );
        window.render_frame(cx);
        assert_eq!(
            window.find("mapping-category-default").bounds().top(),
            first_top
        );
    })
    .unwrap();
}

fn move_mapping_selection(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    control: &'static str,
    steps: i32,
) {
    cx.update_window(handle, |_, window, cx| {
        window.within(control).click("input", cx);
        for _ in 0..steps.unsigned_abs() {
            window.press(if steps > 0 { "down" } else { "up" }, cx);
        }
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[test]
fn versioned_assignments_preserve_unicode_multiline_and_legacy_bindings() {
    let text = Assignment::Text {
        text: "中文\n😀\"\\".into(),
    };
    assert_eq!(Assignment::decode(&text.encode()), text);
    assert!(text.encode().starts_with(PREFIX));
    assert_eq!(
        Assignment::decode("keyboard:Ctrl+C"),
        Assignment::Keyboard {
            key: "KEY_C".into(),
            modifiers: vec!["KEY_LEFT_CTRL".into()],
            turbo: None,
        }
    );
    assert_eq!(
        Assignment::decode("RightButton"),
        Assignment::Mouse {
            action: "Menu".into(),
            turbo: None
        }
    );
    assert_eq!(
        Assignment::decode("unknown-service-action").encode(),
        "unknown-service-action"
    );
    assert_eq!(
        Assignment::decode("local-mapping:v9:opaque").encode(),
        "local-mapping:v9:opaque"
    );
    assert_eq!(canonical_key("space").as_deref(), Some("KEY_SPACEBAR"));
    assert_eq!(canonical_key("F24").as_deref(), Some("KEY_F24"));
    assert_eq!(canonical_key("F25"), None);
}

#[test]
fn device_input_and_layer_filter_categories() {
    let scroll = categories(182, "ScrollUp", false);
    assert_eq!(
        scroll,
        vec![
            Category::Default,
            Category::Keyboard,
            Category::Mouse,
            Category::Macro,
            Category::Multimedia,
            Category::Disable
        ]
    );
    assert!(categories(182, "Button4", false).contains(&Category::Sensitivity));
    assert!(!categories(182, "CycleUpSensitivityStages", false).contains(&Category::Hypershift));
    assert!(!categories(182, "Button4", true).contains(&Category::Hypershift));
    assert!(!categories(653, "KEY_A", false).contains(&Category::Sensitivity));
    assert!(categories(653, "not-an-input", false).is_empty());
    assert!(categories(777, "Button4", false).is_empty());
    assert!(categories(653, "KEY_F9", true).is_empty());
    assert!(categories(653, "KEY_APPLICATION", true).is_empty());
    assert!(!categories(653, "KEY_F9", false).is_empty());
    assert!(categories_for_layout(653, 999, "KEY_A", false).is_empty());
}

#[test]
fn symbol_choices_preserve_physical_keys_and_required_shift() {
    let mut assignment = Assignment::Keyboard {
        key: "KEY_C".into(),
        modifiers: vec!["KEY_LEFT_CTRL".into()],
        turbo: None,
    };
    assign_key_choice(&mut assignment, "symbol:+", &["KEY_LEFT_CTRL".into()]);
    assert_eq!(
        assignment,
        Assignment::Keyboard {
            key: "KEY_EQUAL".into(),
            modifiers: vec!["KEY_LEFT_CTRL".into(), "KEY_LEFT_SHIFT".into()],
            turbo: None,
        }
    );
    assert_eq!(Assignment::decode("keyboard:Ctrl++"), assignment);
    assign_key_choice(&mut assignment, "KEY_EQUAL", &["KEY_LEFT_CTRL".into()]);
    assert_eq!(
        assignment,
        Assignment::Keyboard {
            key: "KEY_EQUAL".into(),
            modifiers: vec!["KEY_LEFT_CTRL".into()],
            turbo: None,
        }
    );
    assert_eq!(
        canonical_key("Num Enter").as_deref(),
        Some("KEY_NUMPAD_ENTER")
    );
    let future = r#"local-mapping:v1:{"kind":"text","text":"kept","future_option":true}"#;
    assert_eq!(Assignment::decode(future).encode(), future);
}

#[test]
fn shifted_symbols_keep_required_shift_when_optional_modifiers_are_cleared() {
    let mut assignment = Assignment::Keyboard {
        key: "KEY_EQUAL".into(),
        modifiers: vec!["KEY_LEFT_CTRL".into(), "KEY_RIGHT_SHIFT".into()],
        turbo: None,
    };
    update_key_modifiers(&mut assignment, Some("symbol:+"), &[]);
    assert_eq!(
        assignment,
        Assignment::Keyboard {
            key: "KEY_EQUAL".into(),
            modifiers: vec!["KEY_LEFT_SHIFT".into()],
            turbo: None,
        }
    );
    update_key_modifiers(
        &mut assignment,
        Some("symbol:+"),
        &["KEY_RIGHT_SHIFT".into()],
    );
    assert!(matches!(&assignment, Assignment::Keyboard { modifiers, .. }
        if modifiers == &["KEY_RIGHT_SHIFT"]));
    // Clearing optional modifiers on a physical '=' or a recorded chord must
    // not add Shift merely because the same key can also produce '+'.
    for selected in [Some("KEY_EQUAL"), None] {
        update_key_modifiers(&mut assignment, selected, &[]);
        assert!(matches!(&assignment, Assignment::Keyboard { modifiers, .. }
            if modifiers.is_empty()));
    }
}

#[test]
fn changing_symbols_retains_explicit_shift_but_drops_implicit_shift() {
    for (optional, expected) in [
        (vec![], vec!["KEY_LEFT_SHIFT".to_string()]),
        (
            vec!["KEY_RIGHT_SHIFT".into()],
            vec!["KEY_RIGHT_SHIFT".into()],
        ),
    ] {
        let mut assignment = Category::Keyboard.initial();
        assign_key_choice(&mut assignment, "symbol:+", &optional);
        assert!(
            matches!(&assignment, Assignment::Keyboard { key, modifiers, .. }
            if key == "KEY_EQUAL" && *modifiers == expected)
        );
        assign_key_choice(&mut assignment, "symbol:~", &optional);
        assert!(
            matches!(&assignment, Assignment::Keyboard { key, modifiers, .. }
            if key == "KEY_TILDE" && *modifiers == expected)
        );
        assign_key_choice(&mut assignment, "KEY_EQUAL", &optional);
        assert!(
            matches!(&assignment, Assignment::Keyboard { key, modifiers, .. }
            if key == "KEY_EQUAL" && *modifiers == optional)
        );
    }
}

#[gpui_kit::test]
fn symbol_dropdown_and_modifier_buttons_preserve_the_saved_chord(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1100.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("keyboard-input-KEY_A", cx);
        window.click("mapping-category-keyboard", cx);
        assert_eq!(window.find("mapping-apply").disabled(), Some(true));
        window.click("mapping-apply", cx);
        assert!(window.try_find("mapping-overlay").is_some());
    })
    .unwrap();
    cx.run_until_parked();

    // The source order is Recording, Alphanumeric, …, Symbols; '+' follows
    // the initial '`' by fifteen rows. Arrow input also scrolls the real menu.
    move_mapping_selection(cx, handle.into(), "mapping-key-group", 6);
    move_mapping_selection(cx, handle.into(), "mapping-action", 15);
    cx.update_window(handle.into(), |_, window, cx| {
        let workspace = view.read(cx);
        assert_eq!(
            workspace.mapping_symbol_choice(cx).as_deref(),
            Some("symbol:+")
        );
        assert!(workspace.mapping_optional_modifiers.is_empty());
        assert!(
            matches!(workspace.mapping_action(), Some(Assignment::Keyboard { key, modifiers, .. })
            if key == "KEY_EQUAL" && modifiers == ["KEY_LEFT_SHIFT"])
        );
        window.click("mapping-include-modifiers", cx);
        window.click("mapping-modifier-KEY_RIGHT_SHIFT", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(
            matches!(view.read(cx).mapping_action(), Some(Assignment::Keyboard { modifiers, .. })
            if modifiers == ["KEY_RIGHT_SHIFT"])
        );
        window.click("mapping-modifier-KEY_RIGHT_SHIFT", cx);
        assert!(view.read(cx).mapping_optional_modifiers.is_empty());
        assert!(
            matches!(view.read(cx).mapping_action(), Some(Assignment::Keyboard { modifiers, .. })
            if modifiers == ["KEY_LEFT_SHIFT"])
        );
        window.click("mapping-modifier-KEY_RIGHT_SHIFT", cx);
    })
    .unwrap();
    cx.run_until_parked();

    move_mapping_selection(cx, handle.into(), "mapping-action", -2);
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            view.read(cx).mapping_symbol_choice(cx).as_deref(),
            Some("KEY_EQUAL")
        );
        assert!(
            matches!(view.read(cx).mapping_action(), Some(Assignment::Keyboard { modifiers, .. })
            if modifiers == ["KEY_RIGHT_SHIFT"])
        );
        window.click("mapping-turbo", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).mapping_symbol_choice(cx).as_deref(),
            Some("KEY_EQUAL")
        );
    });

    // Switching dropdown groups keeps the user's optional modifiers.
    move_mapping_selection(cx, handle.into(), "mapping-key-group", -5);
    cx.update(|cx| {
        assert!(matches!(view.read(cx).mapping_action(), Some(Assignment::Keyboard { key, modifiers, .. })
            if key == "KEY_A" && modifiers == ["KEY_RIGHT_SHIFT"]));
    });
    move_mapping_selection(cx, handle.into(), "mapping-key-group", 5);
    move_mapping_selection(cx, handle.into(), "mapping-action", 15);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("mapping-include-modifiers", cx);
        window.click("mapping-apply", cx);
        assert!(window.try_find("mapping-overlay").is_none());
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            Assignment::decode(&view.read(cx).settings().bindings["KEY_A"]),
            Assignment::Keyboard {
                key: "KEY_EQUAL".into(),
                modifiers: vec!["KEY_LEFT_SHIFT".into()],
                turbo: Some(7),
            }
        );
    });
}

#[test]
fn keyboard_categories_follow_every_bundled_layout_and_hidden_dial_inputs() {
    for layout in 1..=18 {
        if crate::resources::keyboard_source_for_layout(layout).is_none() {
            assert!(categories_for_layout(653, layout, "KEY_A", false).is_empty());
            continue;
        }
        assert!(categories_for_layout(653, layout, "KEY_A", false).contains(&Category::Keyboard));
        assert!(
            categories_for_layout(653, layout, "ScrollUp", false).contains(&Category::Brightness)
        );
        assert!(!categories_for_layout(653, layout, "ScrollUp", false).contains(&Category::Text));
        for input in [
            "KEY_F9",
            "KEY_F10",
            "KEY_F11",
            "KEY_F12",
            "KEY_PAUSE",
            "KEY_APPLICATION",
        ] {
            assert!(categories_for_layout(653, layout, input, true).is_empty());
        }
        for input in ["DIAL_CLICK", "DKM_KB_LEFTKNOB", "ScrollLeft", "ScrollRight"] {
            assert!(categories_for_layout(653, layout, input, false).is_empty());
        }
    }
}

#[gpui_kit::test]
fn mouse_scroll_and_bottom_button_offer_only_supported_mapping_controls(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(crate::model::measured_devices().remove(0), true, window, cx)
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("mouse-input-ScrollUp", cx);
        for absent in [
            "mapping-category-sensitivity",
            "mapping-category-profile",
            "mapping-category-text",
            "mapping-category-hypershift",
        ] {
            assert!(window.try_find(absent).is_none());
        }
        window.click("mapping-category-mouse", cx);
        assert!(window.try_find("mapping-turbo").is_none());
        window.click("mapping-close", cx);
        window.click("mapping-discard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("customize-drawer-toggle", cx);
        window.click("drawer-input-CycleUpSensitivityStages", cx);
        assert!(window.try_find("mapping-category-hypershift").is_none());
        window.click("mapping-category-sensitivity", cx);
        assert!(window.try_find("mapping-dpi-x").is_none());
        assert!(view.read(cx).mapping_valid());
        assert!(
            matches!(view.read(cx).mapping_action(), Some(Assignment::Sensitivity { action, .. })
            if action == "DPI_Up")
        );
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn closing_or_saving_a_mapping_restores_focus_to_its_input(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(crate::model::measured_devices().remove(0), true, window, cx)
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("mouse-input-RightButton", cx);
        assert!(window.try_find("mapping-overlay").is_some());
        window.press("escape", cx);
        assert!(window.try_find("mapping-overlay").is_none());
        assert_eq!(window.find("mouse-input-RightButton").focused(), Some(true));
        window.click("mouse-input-RightButton", cx);
        window.click("mapping-apply", cx);
        assert!(window.try_find("mapping-overlay").is_none());
        assert_eq!(window.find("mouse-input-RightButton").focused(), Some(true));
        window.click("customize-drawer-toggle", cx);
        window.click("drawer-input-RightButton", cx);
        window.press("escape", cx);
        assert_eq!(
            window.find("drawer-input-RightButton").focused(),
            Some(true)
        );
    })
    .unwrap();
    cx.run_until_parked();
}

#[test]
fn committed_numeric_input_restores_or_clamps_and_websites_get_a_scheme() {
    assert_eq!(normalized_number("", false, 1600), 1600);
    assert_eq!(normalized_number("101", false, 800), 150);
    assert_eq!(normalized_number("99999", false, 800), 30000);
    assert_eq!(normalized_number("0", true, 7), 1);
    assert_eq!(normalized_number("21", true, 7), 20);
    assert_eq!(
        normalized_website("www.razer.com"),
        Some("https://www.razer.com".into())
    );
    assert_eq!(
        normalized_website("HTTP://localhost:8080/path"),
        Some("http://localhost:8080/path".into())
    );
    for invalid in [
        "http://",
        "https://.",
        "javascript:alert(1)",
        "a b.com",
        "https://host..com",
    ] {
        assert_eq!(normalized_website(invalid), None);
    }
}

#[gpui_kit::test]
fn source_capabilities_and_legacy_reverts_preserve_profile_drafts(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(crate::model::measured_devices().remove(0), true, window, cx)
        });
        view.update(cx, |workspace, cx| {
            workspace
                .settings_mut()
                .bindings
                .insert("Button4".into(), "keyboard:Ctrl+C".into());
            workspace.open_mapping("Button4".into(), window, cx);
            workspace.update_mapping(cx, |action| *action = Assignment::Disable);
            workspace.update_mapping(cx, |action| *action = Assignment::decode("keyboard:Ctrl+C"));
            assert!(!workspace.mapping_dirty());
            assert_eq!(workspace.mapping.as_ref().unwrap().value, "keyboard:Ctrl+C");

            workspace.hypershift = true;
            workspace.open_mapping("Button4".into(), window, cx);
            workspace.select_mapping_category(Category::Sensitivity, window, cx);
            let options = workspace.mapping_options(&workspace.mapping_action().unwrap());
            assert!(options.iter().any(|option| option.id() == "DPI_Clutch"));
            assert!(!options.iter().any(|option| option.id() == "DPI_OnTheFly"));
            workspace.hypershift = false;
            workspace.open_mapping("CycleUpSensitivityStages".into(), window, cx);
            workspace.select_mapping_category(Category::Sensitivity, window, cx);
            let options = workspace.mapping_options(&workspace.mapping_action().unwrap());
            assert!(
                !options
                    .iter()
                    .any(|option| matches!(option.id(), "DPI_Clutch" | "DPI_OnTheFly"))
            );

            workspace.mapping = None;
            workspace
                .settings_mut()
                .bindings
                .insert("Button4".into(), "future:opaque".into());
            workspace.open_mapping("Button4".into(), window, cx);
            assert!(
                workspace.mapping_valid(),
                "unchanged saved mappings do not block unrelated profile edits"
            );
            assert!(
                workspace.mapping_error().is_some(),
                "the editor must not claim support for opaque data"
            );

            let mut other = workspace.device.profiles[0].clone();
            other.id = "second-profile".into();
            workspace.device.profiles.push(other);
            workspace.select_mapping_category(Category::Profile, window, cx);
            assert_eq!(
                workspace
                    .mapping_options(&workspace.mapping_action().unwrap())
                    .len(),
                1
            );
            let own = format!("profile:{}", workspace.device.active_profile);
            workspace.update_mapping(cx, |action| *action = Assignment::Profile { action: own });
            assert!(
                !workspace.mapping_valid(),
                "a specific profile must differ from the active one"
            );
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn empty_operation_ids_cannot_be_saved_as_assignments(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        view.update(cx, |workspace, cx| {
            let mut other = workspace.device.profiles[0].clone();
            other.id = "second-profile".into();
            workspace.device.profiles.push(other);
            for assignment in [
                Assignment::Mouse {
                    action: String::new(),
                    turbo: Some(7),
                },
                Assignment::Multimedia {
                    action: String::new(),
                },
                Assignment::Brightness {
                    action: String::new(),
                },
                Assignment::Windows {
                    action: String::new(),
                },
                Assignment::Profile {
                    action: String::new(),
                },
                Assignment::Launch {
                    mode: String::new(),
                    target: "app.exe".into(),
                },
            ] {
                workspace.open_mapping("KEY_A".into(), window, cx);
                workspace.update_mapping(cx, |action| *action = assignment);
                assert!(!workspace.mapping_valid());
                assert!(!workspace.commit_mapping(cx));
                assert!(!workspace.settings().bindings.contains_key("KEY_A"));
            }
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn recorded_chord_obeys_save_discard_and_layer_continuations(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1100.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("keyboard-input-KEY_A", cx);
        window.click("mapping-category-keyboard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            !view.read(cx).mapping_valid(),
            "category alone is not a key"
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("mapping-record", cx);
        window.press("ctrl-k", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view.read(cx).mapping_valid());
        assert!(view.read(cx).mapping_dirty());
    });
    move_mapping_selection(cx, handle.into(), "mapping-key-group", 0);
    cx.update(|cx| {
        assert!(matches!(view.read(cx).mapping_action(), Some(Assignment::Keyboard { key, modifiers, .. })
            if key == "KEY_K" && modifiers == ["KEY_LEFT_CTRL"]));
    });
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.continue_with(Continue::Layer(true), window, cx)
        });
        window.render_frame(cx);
        window.click("mapping-keep-editing", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(!view.read(cx).hypershift));
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.continue_with(Continue::Layer(true), window, cx)
        });
        window.render_frame(cx);
        window.click("mapping-save", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert!(workspace.hypershift);
        assert_eq!(
            Assignment::decode(&workspace.settings().bindings["KEY_A"]),
            Assignment::Keyboard {
                key: "KEY_K".into(),
                modifiers: vec!["KEY_LEFT_CTRL".into()],
                turbo: None,
            }
        );
        assert!(
            !workspace
                .settings()
                .hypershift_bindings
                .contains_key("KEY_A")
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.continue_with(Continue::Input("KEY_A".into()), window, cx)
        });
        window.render_frame(cx);
        window.click("mapping-category-keyboard", cx);
        window.click("mapping-close", cx);
        window.click("mapping-discard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert!(workspace.mapping.is_none());
        assert!(workspace.settings().hypershift_bindings.is_empty());
    });
}

#[gpui_kit::test]
fn invalid_values_cannot_commit_and_opening_legacy_does_not_rewrite_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(crate::model::measured_devices().remove(0), true, window, cx)
        });
        view.update(cx, |workspace, cx| {
            workspace
                .settings_mut()
                .bindings
                .insert("Button4".into(), "keyboard:Ctrl+C".into());
            workspace.open_mapping("Button4".into(), window, cx);
            assert!(!workspace.mapping_dirty());
            assert_eq!(workspace.mapping.as_ref().unwrap().value, "keyboard:Ctrl+C");
            workspace.select_mapping_category(Category::Keyboard, window, cx);
            assert!(!workspace.mapping_dirty());
            workspace.update_mapping(cx, |a| {
                *a = Assignment::Mouse {
                    action: "Click".into(),
                    turbo: Some(21),
                }
            });
            assert!(!workspace.mapping_valid());
            assert!(!workspace.commit_mapping(cx));
            assert_eq!(workspace.settings().bindings["Button4"], "keyboard:Ctrl+C");
            workspace.update_mapping(cx, |a| {
                *a = Assignment::Mouse {
                    action: "Click".into(),
                    turbo: None,
                }
            });
            assert!(workspace.commit_mapping(cx));
            assert!(workspace.mapping_input_enabled("LeftButton"));
            workspace.open_mapping("LeftButton".into(), window, cx);
            workspace.update_mapping(cx, |a| *a = Assignment::Disable);
            assert!(workspace.commit_mapping(cx));
            workspace.open_mapping("Button4".into(), window, cx);
            workspace.update_mapping(cx, |a| *a = Assignment::Disable);
            assert!(
                !workspace.mapping_valid(),
                "last primary click cannot be removed"
            );
            assert!(!workspace.commit_mapping(cx));
            workspace.update_mapping(cx, |a| {
                *a = Assignment::Text {
                    text: "😀".repeat(126),
                }
            });
            assert!(!workspace.mapping_valid());
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
}

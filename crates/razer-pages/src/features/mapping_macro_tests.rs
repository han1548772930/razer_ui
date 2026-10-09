use super::{Assignment, DeviceWorkspace, EntryKind, MacroLibraryFile, MacroType, WorkspaceEvent};
use crate::features::macro_library::{Entry, Tutorial};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, TestAppContext, px, size};

fn library(entries: &[(u64, &str, MacroType)]) -> MacroLibraryFile {
    MacroLibraryFile {
        next_id: 20,
        entries: entries
            .iter()
            .map(|(id, name, macro_type)| Entry {
                id: *id,
                name: (*name).into(),
                kind: EntryKind::Macro,
                parent: None,
                open: false,
                actions: vec![],
                macro_type: *macro_type,
                active_phase: None,
                record_delay: 0,
                xml_guid: None,
                xml_mouse_mode: 0,
            })
            .collect(),
        current: entries.first().map(|entry| entry.0),
        tutorial: Tutorial::Complete,
    }
}

fn selection(cx: &mut TestAppContext, handle: AnyWindowHandle, control: &'static str, steps: i32) {
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

#[gpui_kit::test]
fn mouse_macro_picker_repeat_save_and_discard_use_local_document_identity(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let file = library(&[(7, "A", MacroType::Standard), (9, "B", MacroType::Sequence)]);
    let handle = cx.open_window(size(px(1280.), px(1100.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(
                razer_model::model::measured_devices().remove(0),
                true,
                window,
                cx,
            )
        });
        view.update(cx, |view, cx| {
            view.set_mapping_macro_library(&file, window, cx)
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("mouse-input-RightButton", cx);
        window.click("mapping-category-macro", cx);
        assert_eq!(
            view.read(cx)
                .mapping_macro_choices(MacroType::Standard)
                .iter()
                .map(|choice| choice.id.as_str())
                .collect::<Vec<_>>(),
            [
                "Once",
                "NTimes",
                "ContinuousToggle",
                "ContinuousHeld",
                "Queue"
            ]
        );
        assert!(window.try_find("mapping-macro").is_some());
        assert!(view.read(cx).mapping_valid()); // Empty action lists are selectable macros.
    })
    .unwrap();
    selection(cx, handle.into(), "mapping-macro-playback", 1);
    cx.update_window(handle.into(), |_, window, cx| {
        window.within("mapping-macro-repeat").click("input", cx);
        window.press("secondary-a", cx);
        window.input("37", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(matches!(
            view.read(cx).mapping_action(),
            Some(Assignment::Macro {
                macro_id: 7,
                repeat_count: 37,
                ..
            })
        ));
        window.click("mapping-apply", cx);
        assert!(window.try_find("mapping-overlay").is_none());
        let saved = view.read(cx).settings().bindings["RightButton"].clone();
        assert_eq!(
            Assignment::decode(&saved),
            Assignment::Macro {
                macro_id: 7,
                name: "A".into(),
                playback: "NTimes".into(),
                repeat_count: 37,
            }
        );
        assert!(!saved.contains("guid"));
        window.click("mouse-input-RightButton", cx);
    })
    .unwrap();
    selection(cx, handle.into(), "mapping-macro-selector", 1);
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(matches!(view.read(cx).mapping_action(), Some(Assignment::Macro { macro_id: 9, ref playback, repeat_count: 2, .. }) if playback == "Sequence"));
        window.click("mapping-close", cx);
        window.click("mapping-discard", cx);
        assert!(matches!(Assignment::decode(&view.read(cx).settings().bindings["RightButton"]), Assignment::Macro { macro_id: 7, repeat_count: 37, .. }));
    }).unwrap();
}

#[gpui_kit::test]
fn macro_catalog_arrival_delete_repair_and_type_change_preserve_existing_bindings(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1100.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(
                razer_model::model::measured_devices().remove(0),
                true,
                window,
                cx,
            )
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    let opened = std::rc::Rc::new(std::cell::Cell::new(false));
    let observe = opened.clone();
    let _subscription = view.update(cx, |_, cx| {
        cx.subscribe(&view, move |_, _, event: &WorkspaceEvent, _| {
            if matches!(event, WorkspaceEvent::OpenMacro) {
                observe.set(true);
            }
        })
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("mouse-input-RightButton", cx);
        window.click("mapping-category-macro", cx);
        assert_eq!(window.find("mapping-apply").disabled(), Some(true));
        window.click("mapping-configure-macros", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(opened.get());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |view, cx| {
            view.set_mapping_macro_library(
                &library(&[(7, "A", MacroType::Standard), (9, "B", MacroType::Standard)]),
                window,
                cx,
            )
        });
        assert!(matches!(
            view.read(cx).mapping_action(),
            Some(Assignment::Macro { macro_id: 7, .. })
        ));
        window.click("mapping-apply", cx);
        window.click("mouse-input-RightButton", cx);
        assert!(!view.read(cx).mapping_dirty());
        let original = view.read(cx).mapping.as_ref().unwrap().value.clone();
        view.update(cx, |view, cx| {
            view.set_mapping_macro_library(&library(&[(9, "B", MacroType::Standard)]), window, cx)
        });
        assert_eq!(view.read(cx).mapping.as_ref().unwrap().value, original);
        assert!(!view.read(cx).mapping_dirty());
        assert_eq!(window.find("mapping-apply").disabled(), Some(true));
        assert!(view.read(cx).mapping_error().unwrap().contains("已不存在"));
    })
    .unwrap();
    // A single remaining candidate stays actionable to repair a dangling ID.
    selection(cx, handle.into(), "mapping-macro-selector", 1);
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(matches!(
            view.read(cx).mapping_action(),
            Some(Assignment::Macro { macro_id: 9, .. })
        ));
        window.click("mapping-apply", cx);
        window.click("mouse-input-RightButton", cx);
        let original = view.read(cx).mapping.as_ref().unwrap().value.clone();
        view.update(cx, |view, cx| {
            view.set_mapping_macro_library(
                &library(&[(9, "Renamed", MacroType::Phased)]),
                window,
                cx,
            )
        });
        assert_eq!(view.read(cx).mapping.as_ref().unwrap().value, original);
        assert!(!view.read(cx).mapping_dirty());
        assert_eq!(view.read(cx).mapping_summary(&original).1, "Renamed");
        assert_eq!(window.find("mapping-apply").disabled(), Some(true));
    })
    .unwrap();
    selection(cx, handle.into(), "mapping-macro-playback", 1);
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(matches!(view.read(cx).mapping_action(), Some(Assignment::Macro { ref playback, repeat_count: 2, .. }) if playback == "Phased"));
        window.click("mapping-apply", cx);
    }).unwrap();
}

#[gpui_kit::test]
fn keyboard_macro_raw_repeat_blocks_save_and_scroll_eligibility_survives_selection(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let file = library(&[(7, "A", MacroType::Standard), (9, "B", MacroType::Standard)]);
    let handle = cx.open_window(size(px(1280.), px(1100.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(razer_model::demo::demo_keyboard(), true, window, cx));
        view.update(cx, |view, cx| {
            view.set_mapping_macro_library(&file, window, cx)
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("keyboard-input-KEY_A", cx);
        window.click("mapping-category-macro", cx);
    })
    .unwrap();
    selection(cx, handle.into(), "mapping-macro-playback", 1);
    cx.update_window(handle.into(), |_, window, cx| {
        window.within("mapping-macro-repeat").click("input", cx);
        window.press("secondary-a", cx);
        window.press("backspace", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(matches!(
            view.read(cx).mapping_action(),
            Some(Assignment::Macro {
                repeat_count: 0,
                ..
            })
        ));
        assert!(!view.read(cx).mapping_valid());
        assert_eq!(window.find("mapping-apply").disabled(), Some(true));
        view.update(cx, |view, cx| {
            view.set_mapping_macro_library(&file, window, cx)
        });
        assert!(!view.read(cx).mapping_valid());
        window.input("99", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("mapping-apply", cx);
        assert!(matches!(
            Assignment::decode(&view.read(cx).settings().bindings["KEY_A"]),
            Assignment::Macro {
                repeat_count: 99,
                ..
            }
        ));
        view.update(cx, |view, cx| {
            view.open_mapping("ScrollUp".into(), window, cx)
        });
        window.click("mapping-category-macro", cx);
        assert_eq!(
            view.read(cx)
                .mapping_macro_choices(MacroType::Standard)
                .iter()
                .map(|choice| choice.id.as_str())
                .collect::<Vec<_>>(),
            ["Once", "NTimes", "Queue"]
        );
    })
    .unwrap();
    selection(cx, handle.into(), "mapping-macro-selector", 1);
    selection(cx, handle.into(), "mapping-macro-playback", 2);
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(matches!(view.read(cx).mapping_action(), Some(Assignment::Macro { ref playback, repeat_count: 2, .. }) if playback == "Queue"));
        window.click("mapping-apply", cx);
        assert!(matches!(Assignment::decode(&view.read(cx).settings().bindings["ScrollUp"]), Assignment::Macro { ref playback, repeat_count: 2, .. } if playback == "Queue"));
    }).unwrap();
}

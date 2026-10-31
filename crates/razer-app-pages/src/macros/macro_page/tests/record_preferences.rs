use super::*;
use razer_model::macro_library::recording::{Preferences, Seconds};

#[gpui_kit::test]
fn recording_help_opens_and_closes_with_its_retained_focus_handle(cx: &mut TestAppContext) {
    let (page, _, handle) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| page.toggle_record_options(window, cx));
        window.render_frame(cx);
        let focus = page.read(cx).record_ui.help_focus[0].clone();
        focus.focus(window, cx);
        window.render_frame(cx);
        assert!(
            window
                .try_find(("macro-record-help-content", 0usize))
                .is_some()
        );
        window.blur(cx);
        window.render_frame(cx);
        assert!(
            window
                .try_find(("macro-record-help-content", 0usize))
                .is_none()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn restored_delays_apply_only_when_the_library_is_loaded(cx: &mut TestAppContext) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    file.recording_preferences = Some(Preferences {
        fixed: Seconds::ZERO,
        random: [Seconds::ZERO, Seconds::new(0.1).unwrap()],
        ..Preferences::default()
    });
    let (page, library, handle) = open(cx, file);
    cx.update(|cx| {
        assert_eq!(page.read(cx).record_preferences().fixed.value(), 0.2);
        assert_eq!(
            page.read(cx)
                .record_preferences()
                .random
                .map(Seconds::value),
            [0.2, 0.1]
        );
        assert!(library.read(cx).pending());
    });
    library.update(cx, |library, cx| {
        let mut file = library.snapshot();
        let preferences = file.recording_preferences.as_mut().unwrap();
        preferences.fixed = Seconds::ZERO;
        preferences.random = [Seconds::ZERO; 2];
        library.replace(file, cx);
    });
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(page.read(cx).record_preferences().fixed, Seconds::ZERO);
        assert_eq!(
            page.read(cx).record_preferences().random,
            [Seconds::ZERO; 2]
        );
        page.update(cx, |page, cx| page.reload_library(window, cx));
        assert_eq!(page.read(cx).record_preferences().fixed.value(), 0.2);
        assert_eq!(
            page.read(cx)
                .record_preferences()
                .random
                .map(Seconds::value),
            [0.2, 0.5]
        );
        assert_eq!(
            library.read(cx).snapshot().recording_preferences,
            Some(page.read(cx).record_preferences())
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn fixed_delay_debounces_the_latest_valid_input_and_keeps_it_when_cleared(cx: &mut TestAppContext) {
    let (page, library, handle) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| page.toggle_record_options(window, cx));
        window.render_frame(cx);
        window.click(("macro-record-delay", 1usize), cx);
        window.click("macro-record-fixed", cx);
        window.press("secondary-a", cx);
        window.input("1", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("secondary-a", cx);
        window.input("2", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(page.read(cx).record_preferences().fixed.value(), 0.2));
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(page.read(cx).record_preferences().fixed.value(), 2.);
        assert_eq!(
            library
                .read(cx)
                .snapshot()
                .recording_preferences
                .unwrap()
                .fixed
                .value(),
            2.
        );
        window.press("secondary-a", cx);
        window.input("3", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("secondary-a", cx);
        window.press("backspace", cx);
        assert_eq!(window.find("macro-record-fixed").value(), Some(""));
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(page.read(cx).record_preferences().fixed.value(), 3.);
        window.render_frame(cx);
        assert_eq!(window.find("macro-record-fixed").value(), Some("3"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn unrelated_preferences_preserve_an_incomplete_fixed_delay_draft(cx: &mut TestAppContext) {
    let (page, library, handle) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| page.toggle_record_options(window, cx));
        window.render_frame(cx);
        window.click(("macro-record-delay", 1usize), cx);
        window.click("macro-record-fixed", cx);
        window.press("secondary-a", cx);
        window.input("4.", cx);
    })
    .unwrap();
    library.update(cx, |library, cx| {
        let mut file = library.snapshot();
        file.recording_preferences.as_mut().unwrap().start_delay = 2;
        library.replace(file, cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(page.read(cx).record_preferences().fixed.value(), 0.2);
        window.render_frame(cx);
        assert_eq!(window.find("macro-record-fixed").value(), Some("4."));
    })
    .unwrap();
}

#[gpui_kit::test]
fn randomized_delay_changes_on_blur_and_finishes_after_settings_close(cx: &mut TestAppContext) {
    let (page, library, handle) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| page.toggle_record_options(window, cx));
        window.render_frame(cx);
        window.click(("macro-record-delay", 2usize), cx);
        window.click("macro-record-min", cx);
        window.press("secondary-a", cx);
        window.input("2", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            page.read(cx)
                .record_preferences()
                .random
                .map(Seconds::value),
            [0.2, 0.5]
        );
        window.click("macro-record-max", cx);
        window.render_frame(cx);
        assert_eq!(window.find("macro-record-max").value(), Some("3"));
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            page.read(cx)
                .record_preferences()
                .random
                .map(Seconds::value),
            [2., 3.]
        );
        window.press("secondary-a", cx);
        window.input("2", cx);
        window.blur(cx);
        page.update(cx, |page, cx| page.toggle_record_options(window, cx));
        assert!(!page.read(cx).record_ui.open);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            page.read(cx)
                .record_preferences()
                .random
                .map(Seconds::value),
            [2., 2.]
        );
        assert_eq!(
            library
                .read(cx)
                .snapshot()
                .recording_preferences
                .unwrap()
                .random
                .map(Seconds::value),
            [2., 2.]
        );
    });
}

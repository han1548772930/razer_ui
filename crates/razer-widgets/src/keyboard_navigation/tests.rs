use super::{Button, GuideState, KeyboardNavigation, guide_language, init, open_guide};
use gpui_kit::base::Root;
use gpui_kit::component::Theme;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext as _, Context, FocusHandle, IntoElement, ParentElement, Render, TestAppContext,
    Window, px, size,
};
use razer_state::ui_preferences::{InitialTabObservation, InitialTabRequest};

#[test]
fn guide_language_follows_its_source_prefix_and_chinese_script_rules() {
    for (input, expected) in [
        ("", "en"),
        ("EN-gb", "en"),
        ("de-AT", "de"),
        ("es-MX", "es"),
        ("fr-CA", "fr"),
        ("ja-JP", "ja"),
        ("ko-KR", "kr"),
        ("kr", "kr"),
        ("pt-PT", "pt-BR"),
        ("pt-BR", "pt-BR"),
        ("ru-RU", "ru"),
        ("zh", "zh-CN"),
        ("zh-Hans", "zh-CN"),
        ("zh-SG", "zh-CN"),
        ("zh-HK", "zh-TW"),
        ("zh-MO", "zh-TW"),
        ("zh-Hant", "zh-TW"),
        ("zh-TW", "zh-TW"),
        ("KR", "en"),
        ("unknown", "en"),
    ] {
        assert_eq!(guide_language(input), expected, "{input}");
    }
}

#[gpui_kit::test]
fn initial_tab_completion_requires_the_requesting_window_and_current_request(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        let first = cx.new(|_| ());
        let second = cx.new(|_| ());
        let request = InitialTabRequest::new(first.entity_id(), 1);
        let other = InitialTabRequest::new(second.entity_id(), 1);
        let old = InitialTabRequest::new(first.entity_id(), 0);
        let first_tab = Ok(InitialTabObservation::First {
            persistence: Ok(()),
        });
        let mut state = GuideState {
            pending_tab: Some(request),
            ..GuideState::default()
        };
        assert!(!state.observe_tab(other, &first_tab));
        assert!(!state.observe_tab(old, &first_tab));
        assert_eq!(state.pending_tab, Some(request));
        assert!(state.observe_tab(request, &first_tab));
        assert!(!state.observe_tab(request, &first_tab));

        state.pending_tab = Some(request);
        assert!(!state.observe_tab(request, &Err("read failed".into())));
        state.pending_tab = Some(request);
        assert!(!state.observe_tab(request, &Ok(InitialTabObservation::Present)));
        state.pending_tab = Some(request);
        assert!(state.observe_tab(
            request,
            &Ok(InitialTabObservation::First {
                persistence: Err("write failed".into())
            })
        ));
        state.pending_tab = Some(request);
        state.close();
        assert!(
            !state.observe_tab(request, &first_tab),
            "dismissal cancels late presentation while the accepted write can finish"
        );
    });
}

#[test]
fn closing_or_reopening_invalidates_queued_focus_and_open_is_idempotent() {
    let mut state = GuideState::default();
    let first = state.open().unwrap();
    assert!(state.accepts_focus(first));
    assert_eq!(state.open(), None);
    state.close();
    assert!(!state.accepts_focus(first));
    let second = state.open().unwrap();
    assert!(!state.accepts_focus(first));
    assert!(state.accepts_focus(second));
}

struct Background(FocusHandle);
impl Render for Background {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Button::new("background-button")
            .track_focus(&self.0)
            .child("Background")
    }
}

#[gpui_kit::test]
fn guide_closes_through_keyboard_and_pointer_with_window_owned_focus(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
        init(cx);
    });
    let handle = cx.open_window(size(px(900.), px(800.)), |window, cx| {
        let view = cx.new(|cx| Background(cx.focus_handle()));
        view.read(cx).0.focus(window, cx);
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        open_guide(window, cx);
        window.render_frame(cx);
        let owner = Root::read(window, cx)
            .plugin::<KeyboardNavigation>()
            .unwrap();
        assert!(owner.read(cx).guide.open);
        assert!(owner.read(cx).close_focus.is_focused(window));
        window.click("keyboard-guide-dialog", cx);
        assert!(
            owner.read(cx).guide.open,
            "clicking the panel must not dismiss the backdrop"
        );
        window.press("escape", cx);
        assert!(!owner.read(cx).guide.open);
        assert!(
            window.focused(cx).is_none(),
            "the source does not restore trigger focus"
        );
        open_guide(window, cx);
        window.render_frame(cx);
        window.click("keyboard-guide-close", cx);
        assert!(!owner.read(cx).guide.open);
    })
    .unwrap();
}

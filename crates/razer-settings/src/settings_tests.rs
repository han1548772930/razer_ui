use super::{super::runtime_page::RuntimePanel, SettingsPage};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, TestAppContext, WindowHandle, px, size};
use razer_state::AppPreferences;

fn open(
    cx: &mut TestAppContext,
    mut preferences: AppPreferences,
) -> (Entity<SettingsPage>, WindowHandle<Root>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    preferences.language = razer_state::LANGUAGES
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(&razer_i18n::locale()))
        .map(|(code, _)| (*code).to_string())
        .unwrap_or_else(|| "en".into());
    let mut page = None;
    let window = cx.open_window(size(px(1500.), px(1600.)), |window, cx| {
        let runtime = cx.new(|_| RuntimePanel::new());
        let entity = cx.new(|cx| SettingsPage::new(preferences, runtime, window, cx));
        page = Some(entity.clone());
        Root::new(entity, window, cx)
    });
    (page.unwrap(), window)
}

#[gpui_kit::test]
fn source_settings_commands_keep_minimum_width_and_height_after_reset(cx: &mut TestAppContext) {
    let (page, handle) = open(cx, AppPreferences::default());
    let reset_count = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = reset_count.clone();
    let subscription = page.update(cx, |_, cx| {
        cx.subscribe(&page, move |_, _, event, _| {
            if matches!(event, super::SettingsEvent::ResetTutorials) {
                count.set(count.get() + 1);
            }
        })
    });
    for width in [1500., 1080.] {
        cx.simulate_window_resize(handle.into(), size(px(width), px(1600.)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let scale = f32::from(window.rem_size()) / 16.;
            for id in ["settings-reset-tutorials", "settings-migration"] {
                let bounds = window.find(id).bounds();
                assert!(bounds.size.width >= px(100. * scale));
                assert!((bounds.size.height - px(27. * scale)).abs() < px(0.1));
            }
        })
        .unwrap();
    }
    cx.update_window(handle.into(), |_, window, cx| {
        let before = window.find("settings-reset-tutorials").bounds().size;
        window.click("settings-reset-tutorials", cx);
        window.render_frame(cx);
        window.click("settings-reset-tutorials", cx);
        assert_eq!(
            window.find("settings-reset-tutorials").bounds().size,
            before
        );
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(reset_count.get(), 1, "disabled reset must not emit again");
    drop(subscription);
}

struct SourceButtonFixture {
    disabled: bool,
    calls: usize,
    opacity: std::rc::Rc<std::cell::Cell<f32>>,
}
impl gpui_kit::Render for SourceButtonFixture {
    fn render(
        &mut self,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::base::TestSupportExt as _;
        use gpui_kit::{InteractiveElement as _, ParentElement as _, Styled as _};
        let mut button = super::settings_button("source-reset", "Reset", self.disabled, window, cx)
            .on_click(cx.listener(|this, _, _, cx| {
                this.calls += 1;
                cx.notify();
            }));
        self.opacity.set(button.style().opacity.unwrap_or(1.));
        gpui_kit::component::v_flex()
            .items_start()
            .gap(px(20.))
            .child(
                // A constrained row must not compress a source button below 100px.
                gpui_kit::component::h_flex()
                    .w(px(160.))
                    .child(button)
                    .child(gpui_kit::div().w(px(120.)).flex_shrink_0()),
            )
            .child(super::settings_button(
                "source-long",
                "Launch profile migration",
                false,
                window,
                cx,
            ))
            .child(
                gpui_kit::div()
                    .id("source-button-outside")
                    .test_support()
                    .size(px(40.)),
            )
    }
}

#[gpui_kit::test]
fn settings_source_button_grows_for_text_and_uses_source_opacity_states(cx: &mut TestAppContext) {
    use gpui_kit::{InputEvent as _, MouseButton, MouseDownEvent, MouseUpEvent};
    use std::time::Duration;
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let opacity = std::rc::Rc::new(std::cell::Cell::new(0.));
    let mut owner = None;
    let handle = cx.open_window(size(px(640.), px(400.)), |window, cx| {
        let view = cx.new(|_| SourceButtonFixture {
            disabled: false,
            calls: 0,
            opacity: opacity.clone(),
        });
        owner = Some(view.clone());
        Root::new(view, window, cx)
    });
    let owner = owner.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("source-reset").bounds().size,
            size(px(100.), px(27.))
        );
        assert!(window.find("source-long").bounds().size.width > px(100.));
        assert_eq!(window.find("source-long").bounds().size.height, px(27.));
        window.hover("source-reset", cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    assert!(
        opacity.get() > 0.8 && opacity.get() < 1.,
        "hover must transition, not jump"
    );
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!((opacity.get() - 0.8).abs() < 0.001);
        let position = window.find("source-reset").bounds().center();
        window.dispatch_event(
            MouseDownEvent {
                position,
                button: MouseButton::Left,
                click_count: 1,
                first_mouse: false,
                modifiers: Default::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!((opacity.get() - 0.6).abs() < 0.001);
        let position = window.find("source-button-outside").bounds().center();
        window.hover("source-button-outside", cx);
        window.dispatch_event(
            MouseUpEvent {
                position,
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Default::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
        assert_eq!(
            owner.read(cx).calls,
            0,
            "release outside cancels the action"
        );
        owner.update(cx, |this, cx| {
            this.disabled = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("source-reset", cx);
        assert_eq!(owner.read(cx).calls, 0);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!((opacity.get() - 0.3).abs() < 0.001);
        assert_eq!(
            window.find("source-reset").bounds().size,
            size(px(100.), px(27.))
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn recommendation_reset_clears_products_and_ownership_but_preserves_categories(
    cx: &mut TestAppContext,
) {
    let preferences = AppPreferences {
        ignored_categories: vec!["mouse".into()],
        ignored_products: vec!["182".into()],
        owned_products: vec!["653".into()],
        ..Default::default()
    };
    let (page, handle) = open(cx, preferences);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-reset-categories", cx);
        let values = page.read(cx).snapshot();
        assert!(values.ignored_products.is_empty());
        assert!(values.owned_products.is_empty());
        assert_eq!(values.ignored_categories, ["mouse"]);
        window.click("settings-recommendations", cx);
        let disabled_values = page.read(cx).snapshot();
        window.click("settings-category-mouse", cx);
        window.click("settings-reset-categories", cx);
        assert_eq!(page.read(cx).snapshot(), disabled_values);
        window.click("settings-recommendations", cx);
        assert_eq!(page.read(cx).snapshot().ignored_categories, ["mouse"]);
        assert!(window.try_find("settings-discard").is_none());
        assert!(window.try_find("settings-save").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn settings_changes_emit_immediately_and_preserve_edits_during_persistence(
    cx: &mut TestAppContext,
) {
    use super::SettingsEvent;
    use gpui_kit::Context;
    use std::{cell::Cell, rc::Rc};
    let (page, handle) = open(cx, AppPreferences::default());
    let changes = Rc::new(Cell::new(0));
    let count = changes.clone();
    let subscription = page.update(cx, |_, cx: &mut Context<SettingsPage>| {
        cx.subscribe(&page, move |_, _, event, _| {
            if matches!(event, SettingsEvent::Changed) {
                count.set(count.get() + 1);
            }
        })
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("settings-save").is_none());
        assert!(window.try_find("settings-discard").is_none());
        window.click("settings-notifications", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(changes.get(), 1);
    let captured = cx.update(|cx| page.read(cx).snapshot());
    assert!(!captured.notifications);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("settings-notifications", cx);
        page.update(cx, |page, cx| page.tutorial_seen(true, cx));
        page.update(cx, |page, cx| page.dashboard_tutorial_seen(true, cx));
        page.update(cx, |page, cx| page.mark_saved(captured, cx));
        let latest = page.read(cx).snapshot();
        assert!(latest.notifications);
        assert!(latest.gamer_room_tutorial_seen);
        assert!(latest.dashboard_tutorial_seen);
        assert!(
            page.read(cx).dirty(),
            "older completion must leave newer preferences pending"
        );
        page.update(cx, |page, cx| page.mark_saved(latest, cx));
        assert!(!page.read(cx).dirty());
        window.click("settings-tab-connection", cx);
        assert!(window.try_find("settings-save").is_none());
    })
    .unwrap();
    drop(subscription);
}

#[gpui_kit::test]
fn registered_product_preview_requests_selected_variant_without_installation_or_dialogs(
    cx: &mut TestAppContext,
) {
    use std::{cell::RefCell, rc::Rc};

    let (page, handle) = open(cx, AppPreferences::default());
    let requests = Rc::new(RefCell::new(Vec::new()));
    let observed = requests.clone();
    let subscription = page.update(cx, |_, cx| {
        cx.subscribe(&page, move |_, _, event, _| {
            if let super::SettingsEvent::PreviewVariant(pid, edition, layout) = event {
                observed.borrow_mut().push((*pid, *edition, *layout));
            }
        })
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-tab-connection", cx);
    })
    .unwrap();

    let expected = cx.update(|cx| {
        let page = page.read(cx);
        let selected = |state: &Entity<
            gpui_kit::component::select::SelectState<Vec<razer_pages::features::Choice>>,
        >| {
            state
                .read(cx)
                .selected_value()
                .unwrap()
                .parse::<u32>()
                .unwrap()
        };
        (
            selected(&page.preview_product),
            selected(&page.preview_edition),
            selected(&page.preview_layout),
        )
    });
    assert!(razer_catalog::registered(expected.0).is_some());
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("preview-product-select").is_some());
        window.click("preview-registered-product", cx);
        assert!(window.try_find("dialog").is_none());
        assert!(!page.read(cx).dirty(), "navigation must not save settings");
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(*requests.borrow(), [expected]);

    // The registry-driven command retains the native button keyboard contract.
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.find("preview-registered-product").focused(),
            Some(true)
        );
        window.press("enter", cx);
        assert!(window.try_find("dialog").is_none());
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(*requests.borrow(), [expected, expected]);
    drop(subscription);
}

use super::{
    AppPicker, AppPickerCatalog, AppPickerEvent, PickerApp, PickerDevice, PickerModule,
    PickerTarget, preview_catalog, source_title_case,
};
use gpui_kit::component::{Root, Theme, h_flex};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, ScrollDelta, Styled,
    Subscription, TestAppContext, Window, div, point, px, size,
};

#[test]
fn current_app_picker_geometry_keeps_icons_and_two_line_titles_intrinsic_in_three_columns() {
    let mut icon = super::tile_icon_slot();
    assert_eq!(icon.style().size.width, Some(super::css(40.).into()));
    assert_eq!(icon.style().size.height, Some(super::css(40.).into()));
    assert_eq!(icon.style().flex_shrink, Some(0.));
    let mut slot = super::tile_grid_slot("source-tile".into());
    assert_eq!(
        slot.style().flex_basis,
        Some(gpui_kit::relative(0.33).into())
    );
    assert_eq!(slot.style().size.height, Some(super::css(96.).into()));
    assert_eq!(slot.style().min_size.width, Some(gpui_kit::px(0.).into()));
    for expanded in [false, true] {
        let mut title = super::tile_title(true, expanded);
        let style = title.style();
        assert_eq!(style.size.width, Some(super::css(110.).into()));
        assert_eq!(style.min_size.width, Some(super::css(110.).into()));
        assert_eq!(style.flex_shrink, Some(0.));
        assert_eq!(style.text.white_space, Some(gpui_kit::WhiteSpace::Normal));
        assert_eq!(style.text.line_clamp, (!expanded).then_some(2));
        assert_eq!(style.text.text_overflow.is_some(), !expanded);
        assert_eq!(
            style.size.height,
            (!expanded).then_some(super::css(33.).into())
        );
        if expanded {
            assert!(style.overflow.x.is_none() && style.overflow.y.is_none());
        } else {
            assert_eq!(style.overflow.x, Some(gpui_kit::Overflow::Hidden));
            assert_eq!(style.overflow.y, Some(gpui_kit::Overflow::Hidden));
        }
    }
}

#[test]
fn bundled_pages_open_without_external_installation_or_download_state() {
    let catalog = AppPickerCatalog::new(PickerApp::Synapse)
        .bundled_modules([
            PickerModule::ProfileMigration,
            PickerModule::LinkedGames,
            PickerModule::Macro,
        ])
        .uninstalling_modules(["syn3-profile-migration"])
        .module_order(["profile-migration"]);
    assert!(catalog.status_unknown());
    assert!(catalog.installed_modules.is_none());
    assert!(catalog.native_apps.is_none());
    let sections = catalog.sections();
    let modules = sections
        .iter()
        .find(|section| section.id == "modules")
        .unwrap();
    assert_eq!(modules.items.len(), 3);
    assert!(
        modules
            .items
            .iter()
            .all(|item| item.reason.is_none() && !item.busy)
    );
    assert!(modules.items.iter().any(|item| matches!(
        &item.target,
        super::PickerRequest::Open(PickerTarget::Module(PickerModule::ProfileMigration))
    )));
    assert!(modules.items.iter().any(|item| matches!(
        &item.target,
        super::PickerRequest::Open(PickerTarget::Module(PickerModule::LinkedGames))
    )));
    assert!(!sections.iter().any(|section| section.id == "recommended"));
}

#[test]
fn local_page_capabilities_never_become_installation_evidence() {
    let unknown = AppPickerCatalog::new(PickerApp::Synapse)
        .launchable_modules([PickerModule::ProfileMigration])
        .launchable_apps([PickerApp::Chroma]);
    assert!(unknown.status_unknown());
    assert!(unknown.installed_modules.is_none());
    assert!(unknown.native_apps.is_none());
    assert_eq!(unknown.modules(), vec![PickerModule::ProfileMigration]);
    assert_eq!(unknown.other_apps(), vec![PickerApp::Chroma]);
    assert!(
        unknown
            .sections()
            .iter()
            .flat_map(|section| &section.items)
            .all(|item| item.reason.is_none()
                && !item.busy
                && matches!(&item.target, super::PickerRequest::Open(_)))
    );
    let storage_only = unknown
        .clone()
        .installed_modules(["syn3-profile-migration", "chroma-app"]);
    assert_eq!(storage_only.modules(), vec![PickerModule::ProfileMigration]);
    assert_eq!(storage_only.other_apps(), vec![PickerApp::Chroma]);
    assert!(
        !storage_only
            .sections()
            .iter()
            .any(|section| section.id == "recommended")
    );
    let confirmed = storage_only.native_apps([PickerApp::Chroma]);
    assert!(!confirmed.status_unknown());
    assert_eq!(confirmed.other_apps(), vec![PickerApp::Chroma]);
    assert!(
        !confirmed
            .sections()
            .iter()
            .any(|section| section.id == "recommended")
    );
    let absent = AppPickerCatalog::new(PickerApp::Synapse)
        .installed_modules(std::iter::empty::<String>())
        .native_apps([]);
    assert!(absent.sections().is_empty());
}

#[test]
fn source_module_order_preserves_local_modules_and_excludes_online_modules() {
    let catalog = AppPickerCatalog::new(PickerApp::Synapse)
        .installed_modules([
            "linkedGames",
            "macro",
            "alexa",
            "feedback",
            "armory",
            "syn3-profile-migration",
        ])
        .uninstalling_modules(["macro"])
        .module_order([
            "armory",
            "alexa",
            "feedback",
            "syn3-profile-migration",
            "linked-games",
        ]);
    assert_eq!(
        catalog.modules(),
        vec![PickerModule::ProfileMigration, PickerModule::LinkedGames]
    );
    let subset = catalog.clone().module_order(["syn3-profile-migration"]);
    assert_eq!(subset.modules(), vec![PickerModule::ProfileMigration]);
    let stale = catalog.module_order(["removed-module"]);
    assert_eq!(
        stale.modules(),
        vec![PickerModule::LinkedGames, PickerModule::ProfileMigration]
    );
    let chroma =
        AppPickerCatalog::new(PickerApp::Chroma).chroma_modules(["audio-visualizer", "sensa-hd"]);
    assert_eq!(
        chroma.modules(),
        vec![PickerModule::AudioVisualizer, PickerModule::SensaHd]
    );
}

#[test]
fn device_conditions_and_partial_host_order_preserve_real_visible_devices() {
    let device =
        |pid, id| PickerDevice::new(pid, id, "Device", "synapse/module-synapse.svg").ready(true);
    let catalog = AppPickerCatalog::new(PickerApp::Synapse).devices(vec![
        device(182, "first").section_position(2),
        device(653, "second").section_position(1),
        device(769, "philips-hue"),
        device(182, "off").powered_off(true),
        device(653, "failed").mixer_failed(true),
        device(777, "pending").ready(false),
    ]);
    let ids = |catalog: &AppPickerCatalog| {
        catalog
            .ordered_devices()
            .iter()
            .map(|device| device.container_id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&catalog), vec!["second", "first"]);
    assert_eq!(
        ids(&catalog.clone().device_order(["182-slot", "653-slot"])),
        vec!["first", "second"]
    );
    assert_eq!(
        ids(&catalog.device_order(["999-stale", "653-slot"])),
        vec!["second", "first"]
    );
    assert!(
        AppPickerCatalog::new(PickerApp::Chroma)
            .devices(vec![device(182, "first")])
            .ordered_devices()
            .is_empty()
    );
    assert_eq!(
        source_title_case("  SENSA HD · CHROMA 控制室  "),
        "Sensa HD · Chroma 控制室"
    );
}

struct PickerFixture {
    picker: Entity<AppPicker>,
    requests: Vec<AppPickerEvent>,
    _subscription: Subscription,
}
impl Render for PickerFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(h_flex().w_full().justify_end().child(self.picker.clone()))
    }
}
fn fixture(
    catalog: AppPickerCatalog,
    window: &mut Window,
    cx: &mut Context<PickerFixture>,
) -> PickerFixture {
    let picker = cx.new(|cx| AppPicker::new(window, cx));
    picker.update(cx, |picker, cx| picker.set_catalog(catalog, window, cx));
    let subscription = cx.subscribe(&picker, |this, _, event: &AppPickerEvent, cx| {
        this.requests.push(event.clone());
        cx.notify();
    });
    PickerFixture {
        picker,
        requests: vec![],
        _subscription: subscription,
    }
}

#[gpui_kit::test]
fn picker_keeps_three_columns_and_routes_enabled_tiles_with_focus_restoration(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(520.)), |window, cx| {
        let catalog = AppPickerCatalog::new(PickerApp::Synapse)
            .installed_modules(["linkedGames", "macro", "syn3-profile-migration", "armory"])
            .native_apps([])
            .launchable_modules([
                PickerModule::LinkedGames,
                PickerModule::ProfileMigration,
                PickerModule::AddWifi,
            ]);
        let fixture = cx.new(|cx| fixture(catalog, window, cx));
        view = Some(fixture.clone());
        Root::new(fixture, window, cx)
    });
    let view = view.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("app-picker-trigger", cx);
        let popup = window.find("app-picker-popup").bounds();
        assert_eq!(popup.size.width, px(410.));
        assert_eq!(
            popup.top(),
            window.find("app-picker-trigger").bounds().top() + px(40.)
        );
        let first = window.find("app-picker-module-linkedGames-slot").bounds();
        let second = window
            .find("app-picker-module-add-wifi-device-slot")
            .bounds();
        let third = window.find("app-picker-module-macro-slot").bounds();
        let fourth = window
            .find("app-picker-module-syn3-profile-migration-slot")
            .bounds();
        assert_eq!(first.top(), second.top());
        assert_eq!(first.top(), third.top());
        assert!(first.left() < second.left() && second.left() < third.left());
        assert_eq!(fourth.top() - first.top(), px(96.));
        window.click("app-picker-module-macro", cx);
        assert!(view.read(cx).requests.is_empty());
        assert!(view.read(cx).picker.read(cx).popup.read(cx).is_open());
        window.click_at(
            "app-picker-module-syn3-profile-migration",
            point(px(10.), px(94.)),
            cx,
        );
        assert_eq!(
            view.read(cx).requests,
            vec![AppPickerEvent::Open(PickerTarget::Module(
                PickerModule::ProfileMigration
            ))]
        );
        assert!(window.try_find("app-picker-popup").is_none());
        assert_eq!(window.find("app-picker-trigger").focused(), Some(true));
        window.press("enter", cx);
        assert!(window.try_find("app-picker-popup").is_some());
        assert_eq!(
            window.find("app-picker-module-linkedGames").focused(),
            Some(true)
        );
        window.press("tab", cx);
        assert_eq!(
            window
                .find("app-picker-module-syn3-profile-migration")
                .focused(),
            Some(true)
        );
        window.press("space", cx);
        assert_eq!(view.read(cx).requests.len(), 2);
        assert_eq!(window.find("app-picker-trigger").focused(), Some(true));
        window.press("enter", cx);
        window.press("escape", cx);
        assert!(window.try_find("app-picker-popup").is_none());
        assert_eq!(window.find("app-picker-trigger").focused(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn picker_short_window_scroll_and_live_capability_change_do_not_emit_installs(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(20.));
    });
    let mut view = None;
    let handle = cx.open_window(size(px(760.), px(340.)), |window, cx| {
        let catalog = preview_catalog("installing").unread_features(false);
        let fixture = cx.new(|cx| fixture(catalog, window, cx));
        view = Some(fixture.clone());
        Root::new(fixture, window, cx)
    });
    let view = view.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("app-picker-trigger", cx);
        let popup = window.find("app-picker-popup").bounds();
        assert!(popup.size.height <= px(240.));
        assert!(popup.bottom() <= px(340.));
        window.scroll(
            "app-picker-popup",
            ScrollDelta::Pixels(point(px(0.), px(-1400.))),
            cx,
        );
        assert!(window.try_find("app-picker-recommend-chroma-app").is_none());
        assert!(view.read(cx).requests.is_empty());
        assert!(view.read(cx).picker.read(cx).popup.read(cx).is_open());
        let picker = view.read(cx).picker.clone();
        picker.update(cx, |picker, cx| {
            picker.set_catalog(AppPickerCatalog::new(PickerApp::Synapse), window, cx)
        });
        window.render_frame(cx);
        assert!(window.try_find("app-picker-recommend-chroma-app").is_none());
        assert!(window.try_find("app-picker-unknown").is_some());
        window.press("escape", cx);
        assert!(window.try_find("app-picker-popup").is_none());
        assert_eq!(window.find("app-picker-trigger").focused(), Some(true));
    })
    .unwrap();
}

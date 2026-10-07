use super::{MouseProductWorkspace, PropertiesSpec, source_spec};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};

#[gpui_kit::test]
fn performance_uses_registered_properties_capability_and_preserves_local_profile(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let specs: Vec<PropertiesSpec> =
        serde_json::from_str(include_str!("mouse_properties_data.json")).unwrap();
    let registered = specs
        .first()
        .expect("audited properties capability")
        .product_id;
    let unknown = crate::product::registry()
        .iter()
        .map(|product| product.id())
        .find(|pid| super::super::source_product(*pid).is_some() && source_spec(*pid).is_none())
        .expect("mouse without completed properties audit");
    for product_id in [registered, unknown] {
        let mut owner = None;
        let handle = cx.open_window(size(px(1450.), px(1200.)), |window, cx| {
            let page = cx.new(|cx| MouseProductWorkspace::new(product_id, window, cx));
            page.update(cx, |page, cx| page.set_page("TAB_PERFORMANCE", window, cx));
            owner = Some(page.clone());
            Root::new(page, window, cx)
        });
        let owner = owner.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let before = owner.read(cx).snapshot();
            if let Some(spec) = source_spec(product_id) {
                let action = window.find("mouse-properties");
                assert_eq!(action.label(), Some(crate::i18n::t(&spec.action).as_str()));
                let scale = f32::from(window.rem_size()) / 16.;
                assert_eq!(action.bounds().size.height, px(spec.line_height * scale));
                window.hover("mouse-properties-help", cx);
                assert_eq!(
                    window.find("mouse-properties-help").label(),
                    Some(crate::i18n::t(&spec.tooltip).as_str())
                );
            } else {
                assert!(window.try_find("mouse-properties").is_none());
                assert!(window.try_find("mouse-properties-help").is_none());
            }
            assert_eq!(owner.read(cx).snapshot(), before);
        })
        .unwrap();
    }
}

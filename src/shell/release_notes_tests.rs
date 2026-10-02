use super::{open, sections};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};

#[test]
fn first_post_preserves_heading_sections_and_rule_boundaries() {
    let result = sections(
        "ignored<p>&nbsp;</p><h2>A</h2><p>first</p><h2>B</h2><ul><li>second</li></ul><HR /><h2>C</h2><p>third</p>",
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].subsections.len(), 2);
    assert_eq!(result[1].subsections.len(), 1);
    assert!(result[0].subsections[1].1.contains("<ul>"));
    assert!(result[1].subsections[0].1.starts_with("<h2>C</h2>"));
    assert!(sections("<p>No heading</p><hr><h2>Unclosed").is_empty());
}

#[gpui_kit::test]
fn release_notes_preview_keeps_source_geometry_and_can_close(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut notes = None;
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let view = open(window, cx);
        notes = Some(view.clone());
        Root::new(view, window, cx)
    });
    let notes = notes.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let panel = window.find("release-notes-panel").bounds();
        assert!((f32::from(panel.size.width) - 850.).abs() < 1.);
        assert!((f32::from(panel.size.height) - 794.).abs() < 1.);
        assert!((f32::from(panel.bottom()) - 900.).abs() < 1.);
        assert!(notes.read(cx).preview.is_none());
        window.click("release-notes-preview", cx);
        assert_eq!(notes.read(cx).preview.as_ref().unwrap().len(), 4);
        window.click("release-notes-close", cx);
        assert!(window.try_find("release-notes-panel").is_none());
        assert!(notes.read(cx).preview.is_none());
    })
    .unwrap();
}

use super::*;

fn shaped(text: &str, width: f32, glyphs: &[(usize, f32, f32, u32)]) -> ShapedLine {
    let mut line = ShapedLine::default();
    line.text = text.into();
    *std::ops::DerefMut::deref_mut(&mut line) = Arc::new(LineLayout {
        font_size: px(14.),
        width: px(width),
        ascent: px(11.),
        descent: px(3.),
        runs: vec![ShapedRun {
            font_id: FontId(7),
            glyphs: glyphs
                .iter()
                .map(|&(index, x, y, id)| ShapedGlyph {
                    id: GlyphId(id),
                    position: point(px(x), px(y)),
                    index,
                    is_emoji: false,
                })
                .collect(),
        }],
        len: text.len(),
    });
    line
}

fn positions(line: &ShapedLine) -> Vec<(usize, f32, f32, u32)> {
    line.runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .map(|glyph| {
            (
                glyph.index,
                f32::from(glyph.position.x),
                f32::from(glyph.position.y),
                glyph.id.0,
            )
        })
        .collect()
}

#[::core::prelude::v1::test]
fn source_text_tracking_keeps_shaping_and_the_shared_cache_immutable() {
    let line = shaped(
        "AVi",
        24.,
        &[(0, 0., 0., 1), (1, 8., 0., 2), (2, 17., 0., 3)],
    );
    let before = positions(&line);
    let rows = layout_rows(&line, px(0.5), None);
    assert_eq!(
        positions(&rows[0]),
        [(0, 0., 0., 1), (1, 8.5, 0., 2), (2, 18., 0., 3)]
    );
    // CSS inline tracking contributes an advance after each typographic unit,
    // including the final unit, without scaling any glyph or kerning pair.
    assert_eq!(rows[0].width(), px(25.5));
    assert_eq!(rows[0].runs[0].font_id, FontId(7));
    assert_eq!(rows[0].ascent, px(11.));
    assert_eq!(rows[0].descent, px(3.));
    assert_eq!(positions(&line), before);
    assert_eq!(line.width(), px(24.));
}

#[::core::prelude::v1::test]
fn source_text_combining_and_zwj_graphemes_keep_all_glyph_offsets() {
    let accent = shaped(
        "e\u{301}x",
        20.,
        &[(0, 0., 0., 1), (1, 4., -3., 2), (3, 10., 0., 3)],
    );
    let rows = layout_rows(&accent, px(0.5), None);
    assert_eq!(
        positions(&rows[0]),
        [(0, 0., 0., 1), (1, 4., -3., 2), (3, 10.5, 0., 3)]
    );
    assert_eq!(rows[0].width(), px(21.));

    let emoji = shaped(
        "👩\u{200d}💻x",
        30.,
        &[
            (0, 0., 0., 4),
            (4, 1., 0., 5),
            (7, 0., 0., 6),
            (11, 20., 0., 3),
        ],
    );
    let rows = layout_rows(&emoji, px(0.5), Some(px(21.)));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].text.as_ref(), "👩\u{200d}💻");
    assert_eq!(
        positions(&rows[0]),
        [(0, 0., 0., 4), (4, 1., 0., 5), (7, 0., 0., 6)]
    );
    assert_eq!(rows[1].text.as_ref(), "x");
}

#[::core::prelude::v1::test]
fn source_text_required_clusters_and_cursive_forms_are_never_split() {
    let required = shaped(
        "ffi x",
        32.,
        &[(0, 0., 0., 1), (3, 18., 0., 2), (4, 22., 0., 3)],
    );
    let rows = layout_rows(&required, px(0.5), Some(px(15.)));
    assert_eq!(rows[0].text.as_ref(), "ffi ");
    assert_eq!(rows[0].runs[0].glyphs[0].id, GlyphId(1));

    let arabic = shaped("اب", 20., &[(2, 0., 0., 4), (0, 10., 0., 5)]);
    let rows = layout_rows(&arabic, px(0.5), None);
    assert_eq!(positions(&rows[0]), positions(&arabic));
    assert_eq!(rows[0].width(), px(20.));
}

#[::core::prelude::v1::test]
fn source_text_tracking_preserves_nonmonotonic_bidi_indices() {
    let line = shaped(
        "אבג",
        30.,
        &[(4, 0., 0., 3), (2, 10., 0., 2), (0, 20., 0., 1)],
    );
    let rows = layout_rows(&line, px(0.5), None);
    assert_eq!(
        positions(&rows[0]),
        [(4, 0., 0., 3), (2, 10.5, 0., 2), (0, 21., 0., 1)]
    );
    assert_eq!(rows[0].width(), px(31.5));
}

#[::core::prelude::v1::test]
fn source_text_unicode_wrapping_uses_tracked_advances_and_cjk_punctuation() {
    let text = "甲（乙）丙";
    let line = shaped(
        text,
        50.,
        &[
            (0, 0., 0., 1),
            (3, 10., 0., 2),
            (6, 20., 0., 3),
            (9, 30., 0., 4),
            (12, 40., 0., 5),
        ],
    );
    let rows = layout_rows(&line, px(0.5), Some(px(21.)));
    let words: Vec<_> = rows.iter().map(|row| row.text.as_ref()).collect();
    assert_eq!(words, ["甲", "（乙）", "丙"]);
    assert_eq!(rows[1].width(), px(31.5));
    assert_eq!(
        positions(&rows[1]),
        [(0, 0., 0., 2), (3, 10.5, 0., 3), (6, 21., 0., 4)]
    );

    let latin = shaped(
        "a b",
        30.,
        &[(0, 0., 0., 1), (1, 10., 0., 2), (2, 20., 0., 3)],
    );
    assert_eq!(layout_rows(&latin, px(0.), Some(px(30.))).len(), 1);
    assert_eq!(layout_rows(&latin, px(0.5), Some(px(30.))).len(), 2);
    let narrow = layout_rows(&latin, px(0.5), Some(px(15.)));
    assert_eq!(narrow[0].text.as_ref(), "a ");
    assert_eq!(narrow[0].width(), px(10.5));
}

#[::core::prelude::v1::test]
fn source_text_features_disable_only_optional_ligatures() {
    let original = FontFeatures(Arc::new(vec![
        ("calt".into(), 1),
        ("rlig".into(), 1),
        ("liga".into(), 1),
    ]));
    let features = tracking_features(&original);
    assert_eq!(
        features.tag_value_list(),
        [
            ("calt".into(), 1),
            ("rlig".into(), 1),
            ("liga".into(), 0),
            ("clig".into(), 0)
        ]
    );
    assert_eq!(original.tag_value_list().last().unwrap().1, 1);
}

#[::core::prelude::v1::test]
fn source_text_accessibility_keeps_the_complete_localized_text_and_stable_id() {
    let label = SourceText::new("label", "风扇 👩\u{200d}💻").letter_spacing(0.5);
    assert_eq!(label.id(), Some("label".into()));
    assert_eq!(label.a11y_role(), Some(accesskit::Role::Label));
    let mut node = accesskit::Node::new(accesskit::Role::Label);
    label.write_a11y_info(&mut node);
    assert_eq!(node.value(), Some("风扇 👩\u{200d}💻"));
}

#[::core::prelude::v1::test]
fn source_text_preserves_initial_glyph_bearing_and_scales_source_pixels() {
    let line = shaped("AV", 20., &[(0, -1., 0., 1), (1, 9., 0., 2)]);
    let rows = layout_rows(&line, source_spacing(0.5, px(24.)), None);
    assert_eq!(positions(&rows[0]), [(0, -1., 0., 1), (1, 9.75, 0., 2)]);
    assert_eq!(rows[0].width(), px(21.5));
    assert_eq!(source_spacing(0.1, px(32.)), px(0.2));
    let untracked = layout_rows(&line, px(0.), None);
    assert_eq!(positions(&untracked[0]), positions(&line));
    assert_eq!(untracked[0].width(), line.width());
}

#[::core::prelude::v1::test]
fn source_text_empty_and_glyphless_lines_retain_their_original_layout() {
    for text in ["", "\u{200b}"] {
        let line = shaped(text, 0., &[]);
        let rows = layout_rows(&line, px(0.5), Some(px(10.)));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text.as_ref(), text);
        assert_eq!(rows[0].width(), Pixels::ZERO);
        assert!(positions(&rows[0]).is_empty());
    }
}

#[::core::prelude::v1::test]
fn source_text_mixed_rtl_wrapping_rebases_each_selected_shaping_run() {
    let line = shaped(
        "a \u{5d0}\u{5d1} b",
        60.,
        &[
            (0, 0., 0., 1),
            (1, 10., 0., 2),
            (4, 20., 0., 4),
            (2, 30., 0., 3),
            (6, 40., 0., 2),
            (7, 50., 0., 5),
        ],
    );
    let rows = layout_rows(&line, px(0.5), Some(px(32.)));
    let text: Vec<_> = rows.iter().map(|row| row.text.as_ref()).collect();
    assert_eq!(text, ["a ", "\u{5d0}\u{5d1} ", "b"]);
    assert_eq!(
        positions(&rows[1]),
        [(2, 0., 0., 4), (0, 10.5, 0., 3), (4, 21., 0., 2)]
    );
    assert_eq!(rows[1].width(), px(21.));
    assert_eq!(positions(&rows[2]), [(0, 0., 0., 5)]);
}

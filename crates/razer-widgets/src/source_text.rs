//! Source CSS tracking on whole shaped labels. The inherited font is resolved
//! by GPUI's text system; this element owns no font acquisition or background work.
use gpui_kit::*;
use std::{cell::RefCell, collections::BTreeMap, ops::Range, rc::Rc, sync::Arc};
use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

/// A source label with CSS letter spacing, inherited font/color/alignment, and
/// Unicode line wrapping. Its stable identity exposes the complete text to
/// accessibility. Source pixel lengths scale with the product's 16px rem basis.
pub struct SourceText {
    id: ElementId,
    text: SharedString,
    letter_spacing: f32,
}

impl SourceText {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            letter_spacing: 0.,
        }
    }

    pub fn letter_spacing(mut self, source_pixels: f32) -> Self {
        assert!(
            source_pixels.is_finite(),
            "Non-finite source letter spacing"
        );
        self.letter_spacing = source_pixels;
        self
    }
}

impl IntoElement for SourceText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Per-layout measurement shared with Taffy's measurement closure. It never
/// outlives the element's layout and contains no mutable framework cache data.
#[doc(hidden)]
#[derive(Default)]
pub struct SourceTextLayout {
    rows: Vec<ShapedLine>,
    line_height: Pixels,
    align: TextAlign,
}

impl Element for SourceText {
    type RequestLayoutState = Rc<RefCell<SourceTextLayout>>;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn a11y_role(&self) -> Option<accesskit::Role> {
        Some(accesskit::Role::Label)
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        node.set_value(self.text.to_string());
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.pixel_snap(
            style
                .line_height
                .to_pixels(font_size.into(), window.rem_size()),
        );
        let spacing = source_spacing(self.letter_spacing, window.rem_size());
        let paragraphs: Vec<_> = self
            .text
            .split('\n')
            .map(|text| {
                let mut run = style.to_run(text.len());
                if spacing != Pixels::ZERO {
                    run.font.features = tracking_features(&run.font.features);
                }
                window
                    .text_system()
                    .shape_line(SharedString::from(text), font_size, &[run], None)
            })
            .collect();
        let state = Rc::new(RefCell::new(SourceTextLayout {
            line_height,
            align: style.text_align,
            ..Default::default()
        }));
        let measured = state.clone();
        let layout_id =
            window.request_measured_layout(Style::default(), move |known, available, _, _| {
                let wrap_width = (style.white_space == WhiteSpace::Normal)
                    .then(|| {
                        known.width.or(match available.width {
                            AvailableSpace::Definite(width) => Some(width),
                            _ => None,
                        })
                    })
                    .flatten();
                let mut rows = Vec::new();
                for paragraph in &paragraphs {
                    rows.extend(layout_rows(paragraph, spacing, wrap_width));
                }
                if let Some(limit) = style.line_clamp {
                    rows.truncate(limit);
                }
                let width = rows.iter().map(ShapedLine::width).fold(px(0.), Pixels::max);
                let height = line_height * rows.len();
                measured.borrow_mut().rows = rows;
                size(width, height)
            });
        (layout_id, state)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let state = state.borrow();
        let mut origin = bounds.origin;
        for row in &state.rows {
            row.paint(
                origin,
                state.line_height,
                state.align,
                Some(bounds.size.width),
                window,
                cx,
            )
            .expect("Source text glyph painting failed");
            origin.y += state.line_height;
        }
    }
}

fn tracking_features(features: &FontFeatures) -> FontFeatures {
    let mut tags = features.tag_value_list().to_vec();
    tags.retain(|(tag, _)| tag != "liga" && tag != "clig");
    // Optional Latin ligatures must not consume tracking boundaries. Required
    // shaping, contextual forms and combining glyphs remain with the shaper.
    tags.extend([("liga".into(), 0), ("clig".into(), 0)]);
    FontFeatures(Arc::new(tags))
}

fn source_spacing(source_pixels: f32, rem_size: Pixels) -> Pixels {
    px(source_pixels * f32::from(rem_size) / 16.)
}

struct Cluster {
    bytes: Range<usize>,
    x: Pixels,
    origin: Pixels,
    advance: Pixels,
    spacing: Pixels,
}

fn accepts_tracking(text: &str) -> bool {
    // CSS Text cursive tracking must not separate connected script forms.
    !text.chars().any(|ch| {
        matches!(
            ch.script(),
            Script::Arabic
                | Script::Syriac
                | Script::Nko
                | Script::Manichaean
                | Script::Psalter_Pahlavi
                | Script::Mandaic
                | Script::Mongolian
                | Script::Phags_Pa
                | Script::Devanagari
                | Script::Bengali
                | Script::Gurmukhi
                | Script::Modi
                | Script::Sharada
                | Script::Syloti_Nagri
                | Script::Tirhuta
                | Script::Ogham
        )
    })
}

fn clusters(line: &ShapedLine, spacing: Pixels) -> Vec<Cluster> {
    let graphemes: Vec<_> = line.text.grapheme_indices(true).map(|(at, _)| at).collect();
    if graphemes.is_empty() {
        return Vec::new();
    }
    let mut starts = BTreeMap::new();
    for glyph in line.runs.iter().flat_map(|run| &run.glyphs) {
        let at = graphemes.partition_point(|start| *start <= glyph.index);
        let start = graphemes[at.saturating_sub(1)];
        starts
            .entry(start)
            .and_modify(|x: &mut Pixels| *x = (*x).min(glyph.position.x))
            .or_insert(glyph.position.x);
    }
    let mut clusters: Vec<_> = starts
        .iter()
        .map(|(&start, &x)| {
            let end = starts
                .range(start + 1..)
                .next()
                .map_or(line.text.len(), |(&end, _)| end);
            Cluster {
                bytes: start..end,
                x,
                origin: x,
                advance: Pixels::ZERO,
                spacing: if accepts_tracking(&line.text[start..end]) {
                    spacing
                } else {
                    Pixels::ZERO
                },
            }
        })
        .collect();
    // Font runs and bidi runs need not have monotonically increasing byte
    // indices. Tracking advances in visual order, while wrapping stays logical.
    let mut visual: Vec<_> = (0..clusters.len()).collect();
    visual.sort_by(|&left, &right| clusters[left].x.partial_cmp(&clusters[right].x).unwrap());
    for (position, &index) in visual.iter().enumerate() {
        // The first glyph can have a nonzero shaping offset. The paragraph
        // still starts at zero; preserve that bearing and the original width.
        if position == 0 {
            clusters[index].origin = Pixels::ZERO;
        }
        let end = visual
            .get(position + 1)
            .map_or(line.width(), |&next| clusters[next].x);
        clusters[index].advance = (end - clusters[index].origin).max(Pixels::ZERO);
    }
    clusters
}

fn row_ranges(text: &str, clusters: &[Cluster], width: Option<Pixels>) -> Vec<Range<usize>> {
    let Some(width) = width else {
        return vec![0..clusters.len()];
    };
    if clusters.is_empty() {
        return vec![0..0];
    }
    let breaks: BTreeMap<_, _> = linebreaks(text).collect();
    let mut rows = Vec::new();
    let mut start = 0;
    while start < clusters.len() {
        let mut advance = px(0.);
        let mut visible_advance = px(0.);
        let mut last_fit = None;
        let mut end = clusters.len();
        for (index, cluster) in clusters.iter().enumerate().skip(start) {
            advance += cluster.advance + cluster.spacing;
            if !collapses_at_line_edge(&text[cluster.bytes.clone()]) {
                visible_advance = advance;
            }
            let Some(opportunity) = breaks.get(&cluster.bytes.end) else {
                continue;
            };
            if visible_advance > width {
                // A word/required cluster wider than the box overflows intact;
                // a Unicode-forbidden break must not strand CJK punctuation.
                end = last_fit.unwrap_or(index + 1);
                break;
            }
            last_fit = Some(index + 1);
            if *opportunity == BreakOpportunity::Mandatory {
                end = index + 1;
                break;
            }
        }
        rows.push(start..end);
        start = end;
    }
    rows
}

fn collapses_at_line_edge(text: &str) -> bool {
    text.chars()
        .all(|ch| matches!(ch, ' ' | '\t' | '\r' | '\u{c}'))
}

fn layout_rows(line: &ShapedLine, spacing: Pixels, width: Option<Pixels>) -> Vec<ShapedLine> {
    let clusters = clusters(line, spacing);
    row_ranges(&line.text, &clusters, width)
        .into_iter()
        .map(|range| {
            let selected = &clusters[range];
            if selected.is_empty() {
                return line.clone();
            }
            let first_byte = selected.first().unwrap().bytes.start;
            let last_byte = selected.last().unwrap().bytes.end;
            let mut visual: Vec<_> = selected.iter().collect();
            visual.sort_by(|left, right| left.x.partial_cmp(&right.x).unwrap());
            let mut pen = px(0.);
            let mut offsets = BTreeMap::new();
            for cluster in visual {
                offsets.insert(cluster.bytes.start, pen - cluster.origin);
                pen += cluster.advance + cluster.spacing;
            }
            // Source normal whitespace hangs outside a wrapped line's inline
            // width. Keep its bytes for accessibility and glyph indexing.
            let trailing_space: Pixels = selected
                .iter()
                .rev()
                .take_while(|cluster| collapses_at_line_edge(&line.text[cluster.bytes.clone()]))
                .map(|cluster| cluster.advance + cluster.spacing)
                .sum();
            let runs = line
                .runs
                .iter()
                .filter_map(|run| {
                    let glyphs: Vec<_> = run
                        .glyphs
                        .iter()
                        .filter(|glyph| (first_byte..last_byte).contains(&glyph.index))
                        .map(|glyph| {
                            let cluster = selected
                                .iter()
                                .find(|cluster| cluster.bytes.contains(&glyph.index))
                                .expect("Shaped glyph belongs to a source cluster");
                            let mut glyph = glyph.clone();
                            glyph.position.x += offsets[&cluster.bytes.start];
                            glyph.index -= first_byte;
                            glyph
                        })
                        .collect();
                    (!glyphs.is_empty()).then_some(ShapedRun {
                        font_id: run.font_id,
                        glyphs,
                    })
                })
                .collect();
            let mut row = line.clone();
            // Replace our Arc. Mutating the cached Arc would corrupt every
            // ordinary text element sharing the framework's shaped line.
            *std::ops::DerefMut::deref_mut(&mut row) = Arc::new(LineLayout {
                font_size: line.font_size,
                width: (pen - trailing_space).max(px(0.)),
                ascent: line.ascent,
                descent: line.descent,
                runs,
                len: last_byte - first_byte,
            });
            row.text = SharedString::from(&line.text[first_byte..last_byte]);
            row
        })
        .collect()
}

#[cfg(test)]
mod tests;

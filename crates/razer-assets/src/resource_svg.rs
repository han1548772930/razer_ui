//! Monochrome SVG presentation waits for the same resource owner as images.
use gpui_kit::{
    App, InteractiveElement, Interactivity, IntoElement, RenderOnce, SharedString,
    StyleRefinement, Styled, Svg, Transformation, Window, svg,
};

#[derive(IntoElement)]
pub struct ResourceSvg {
    svg: Svg,
    path: Option<SharedString>,
}

pub fn resource_svg() -> ResourceSvg {
    ResourceSvg { svg: svg(), path: None }
}

impl ResourceSvg {
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_transformation(mut self, transformation: Transformation) -> Self {
        self.svg = self.svg.with_transformation(transformation);
        self
    }
}

impl Styled for ResourceSvg {
    fn style(&mut self) -> &mut StyleRefinement { self.svg.style() }
}

impl InteractiveElement for ResourceSvg {
    fn interactivity(&mut self) -> &mut Interactivity { self.svg.interactivity() }
}

impl RenderOnce for ResourceSvg {
    fn render(mut self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        if let Some(path) = self.path {
            // Availability only reads memory and queues the shared I/O owner.
            // Its completion refreshes windows. Preserve pending geometry but
            // do not pass a temporary ResourcePending error to GPUI paint_svg.
            // Actual failures still follow GPUI's error reporting and the
            // resource owner's retained failure/retry state.
            if !matches!(crate::resource_runtime::available(&path, cx), Ok(false)) {
                self.svg = self.svg.path(path);
            }
        }
        self.svg
    }
}

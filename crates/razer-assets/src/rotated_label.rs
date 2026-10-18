//! Retained, cancellable outlines for source canvas text rotated by -90 degrees.
//! Font acquisition uses the resource runtime; font discovery and shaping run
//! on the background executor. The normal SVG path renderer owns painting.
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_storage::file_transfer::{Operation, Transfer};
use std::sync::{Arc, LazyLock, Mutex};

const ROBOTO_REGULAR: &str = "synapse/Roboto-Regular.ttf";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelGeometry {
    font_size: f32,
    viewport: Size<f32>,
    anchor: Point<f32>,
}

impl LabelGeometry {
    /// Geometry is in the source canvas's unscaled coordinate space. The
    /// anchor has canvas `textAlign=center` and `textBaseline=top` semantics.
    pub fn new(font_size: f32, viewport: Size<f32>, anchor: Point<f32>) -> Self {
        Self {
            font_size,
            viewport,
            anchor,
        }
    }

    fn validate(self) -> Result<(), String> {
        if !self.font_size.is_finite()
            || self.font_size <= 0.
            || !self.viewport.width.is_finite()
            || self.viewport.width <= 0.
            || !self.viewport.height.is_finite()
            || self.viewport.height <= 0.
            || !self.anchor.x.is_finite()
            || !self.anchor.y.is_finite()
        {
            return Err("Invalid rotated-label geometry".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LabelRequest {
    text: SharedString,
    locale: SharedString,
    geometry: LabelGeometry,
}

impl LabelRequest {
    /// Current source canvas labels use Roboto normal, weight 400. Characters
    /// absent from that face use installed local fonts on the background task.
    pub fn new(
        text: impl Into<SharedString>,
        locale: impl Into<SharedString>,
        geometry: LabelGeometry,
    ) -> Self {
        Self {
            text: text.into(),
            locale: locale.into(),
            geometry,
        }
    }

    fn svg(&self) -> String {
        let geometry = self.geometry;
        let text = self
            .text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}"><text transform="translate({} {}) rotate(-90)" x="0" y="0" font-family="Roboto" font-weight="400" font-size="{}" text-anchor="middle" dominant-baseline="text-before-edge">{text}</text></svg>"#,
            geometry.viewport.width,
            geometry.viewport.height,
            geometry.viewport.width,
            geometry.viewport.height,
            geometry.anchor.x,
            geometry.anchor.y,
            geometry.font_size,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabelStatus {
    WaitingForFont,
    Loading,
    Ready,
    Failed(String),
}

#[derive(Clone, PartialEq)]
struct LabelIdentity {
    request: LabelRequest,
    /// The actual verified resource bytes participate in request identity.
    font: Arc<[u8]>,
}

/// Retain this entity with the page or chart which requests the label. Render
/// it inside the source-sized label rectangle; color and scale are inherited.
pub struct LabelState {
    request: LabelRequest,
    identity: Option<LabelIdentity>,
    outline: Option<Arc<[u8]>>,
    status: LabelStatus,
    transfer: Transfer,
    task: Option<Task<()>>,
    _resources: Subscription,
}

impl LabelState {
    pub fn new(request: LabelRequest, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let resources = crate::observe_resources(cx, |this: &mut Self, event, cx| {
                if crate::resource_identity(ROBOTO_REGULAR)
                    .is_some_and(|(path, _, _)| path == event.path())
                {
                    this.load_font(cx);
                }
            });
            let mut this = Self {
                request,
                identity: None,
                outline: None,
                status: LabelStatus::WaitingForFont,
                transfer: Transfer::new(),
                task: None,
                _resources: resources,
            };
            this.load_font(cx);
            this
        })
    }

    pub fn status(&self) -> &LabelStatus {
        &self.status
    }

    /// Retry after the shared resource retry command has reopened failed files.
    /// An identical ready or in-flight identity is reused.
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        self.load_font(cx);
    }

    pub fn set_request(&mut self, request: LabelRequest, cx: &mut Context<Self>) {
        if self.request == request {
            return;
        }
        self.cancel();
        self.request = request;
        self.identity = None;
        self.outline = None;
        self.load_font(cx);
    }

    fn cancel(&mut self) {
        self.transfer.cancel();
        self.task.take();
    }

    fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        self.cancel();
        self.outline = None;
        self.status = LabelStatus::Failed(error);
        cx.notify();
    }

    fn load_font(&mut self, cx: &mut Context<Self>) {
        let bytes = match crate::load_path(ROBOTO_REGULAR, cx) {
            Ok(Some(bytes)) => Arc::<[u8]>::from(bytes.into_owned()),
            Ok(None) => {
                self.fail(format!("Missing source font: {ROBOTO_REGULAR}"), cx);
                return;
            }
            Err(error) if crate::is_resource_pending(&error) => {
                self.status = LabelStatus::WaitingForFont;
                cx.notify();
                return;
            }
            Err(error) => {
                self.fail(format!("{ROBOTO_REGULAR}: {error:#}"), cx);
                return;
            }
        };
        let identity = LabelIdentity {
            request: self.request.clone(),
            font: bytes,
        };
        if self.identity.as_ref() == Some(&identity)
            && matches!(self.status, LabelStatus::Loading | LabelStatus::Ready)
        {
            return;
        }
        self.cancel();
        self.transfer = Transfer::new();
        let operation = match self.transfer.accept() {
            Ok(operation) => operation,
            Err(error) => {
                self.fail(error, cx);
                return;
            }
        };
        self.identity = Some(identity.clone());
        self.outline = None;
        self.status = LabelStatus::Loading;
        let transfer = self.transfer.clone();
        let background_identity = identity.clone();
        let worker = cx.background_executor().spawn(async move {
            operation.check()?;
            let fonts = system_fonts();
            operation.check()?;
            outline(&background_identity, fonts, &operation)
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |this, cx| {
                if !this.transfer.is_same(&transfer)
                    || this.identity.as_ref() != Some(&identity)
                    || transfer.check().is_err()
                {
                    return;
                }
                this.task = None;
                match result {
                    Ok(outline) => {
                        this.outline = Some(outline);
                        this.status = LabelStatus::Ready;
                    }
                    Err(error) => {
                        this.outline = None;
                        this.status = LabelStatus::Failed(error);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl Drop for LabelState {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Render for LabelState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .when_some(self.outline.as_ref(), |view, bytes| {
                view.child(svg().data(bytes).size_full())
            })
    }
}

/// An immutable platform-font snapshot, initialized only by the background
/// shaping task. No application-owned resource or page lifetime is retained.
fn system_fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: LazyLock<Arc<usvg::fontdb::Database>> = LazyLock::new(|| {
        let mut fonts = usvg::fontdb::Database::new();
        fonts.load_system_fonts();
        Arc::new(fonts)
    });
    FONTS.clone()
}

fn outline(
    identity: &LabelIdentity,
    fonts: Arc<usvg::fontdb::Database>,
    operation: &Operation,
) -> Result<Arc<[u8]>, String> {
    operation.check()?;
    identity.request.geometry.validate()?;
    let mut fonts = (*fonts).clone();
    let ids = fonts.load_font_source(usvg::fontdb::Source::Binary(Arc::new(
        identity.font.to_vec(),
    )));
    let font = ids
        .first()
        .copied()
        .ok_or_else(|| "Invalid source Roboto font".to_owned())?;
    operation.check()?;
    let missing = Arc::new(Mutex::new(None));
    let missing_glyph = missing.clone();
    let fallback = usvg::FontResolver::default_fallback_selector();
    let options = usvg::Options {
        fontdb: Arc::new(fonts),
        languages: vec![identity.request.locale.to_string()],
        font_resolver: usvg::FontResolver {
            select_font: Box::new(move |_, _| Some(font)),
            select_fallback: Box::new(move |character, used, database| {
                let selected = fallback(character, used, database);
                if selected.is_none() {
                    *missing_glyph.lock().expect("glyph diagnostics lock") = Some(character);
                }
                selected
            }),
        },
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(identity.request.svg().as_bytes(), &options)
        .map_err(|error| format!("Rotated label: {error}"))?;
    operation.check()?;
    if let Some(character) = *missing.lock().expect("glyph diagnostics lock") {
        return Err(format!(
            "No local font for label character U+{:04X}",
            character as u32
        ));
    }
    if !identity.request.text.trim().is_empty() && tree.root().children().is_empty() {
        return Err("Rotated label has no glyph outlines".into());
    }
    let output = tree.to_string(&usvg::WriteOptions::default());
    operation.check()?;
    Ok(Arc::from(output.into_bytes()))
}

#[cfg(test)]
mod tests;

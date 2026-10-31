use super::{SourcePage, focus};
use gpui_kit::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Document {
    pub(super) id: ElementId,
    pub(super) page: SourcePage,
}

pub(super) struct MountedDocument {
    pub(super) document: Document,
    pub(super) bounds: Option<Bounds<Pixels>>,
}

/// Match an official WebContents or iframe boundary without adding a layout box.
pub fn document_scope(
    id: impl Into<ElementId>,
    page: SourcePage,
    child: impl IntoElement,
) -> DocumentScope {
    DocumentScope {
        child: child.into_any_element(),
        document: Document {
            id: id.into(),
            page,
        },
        mount: true,
    }
}

pub(super) fn overlay(document: Document, child: impl IntoElement) -> DocumentScope {
    DocumentScope {
        child: child.into_any_element(),
        document,
        mount: false,
    }
}

pub struct DocumentScope {
    child: AnyElement,
    document: Document,
    mount: bool,
}

struct DocumentGuard {
    frame: focus::SharedFrame,
    previous: Option<Document>,
}

impl DocumentGuard {
    fn enter(document: Document, window: &Window, cx: &App) -> Option<Self> {
        let frame = focus::frame(window, cx)?;
        let previous = frame.borrow_mut().current_document.replace(document);
        Some(Self { frame, previous })
    }
}

impl Drop for DocumentGuard {
    fn drop(&mut self) {
        self.frame.borrow_mut().current_document = self.previous.take();
    }
}

impl IntoElement for DocumentScope {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for DocumentScope {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        if let Some(frame) = focus::frame(window, cx) {
            let mut state = frame.borrow_mut();
            if self.mount {
                if !state
                    .documents
                    .iter()
                    .any(|mounted| mounted.document == self.document)
                {
                    state.documents.push(MountedDocument {
                        document: self.document.clone(),
                        bounds: None,
                    });
                }
            } else if !state
                .documents
                .iter()
                .any(|mounted| mounted.document == self.document)
            {
                // A guide belongs to its source document, including during a tab switch.
                self.child = Empty.into_any_element();
            }
        }
        let _guard = DocumentGuard::enter(self.document.clone(), window, cx);
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.mount
            && let Some(frame) = focus::frame(window, cx)
        {
            if let Some(mounted) = frame
                .borrow_mut()
                .documents
                .iter_mut()
                .find(|mounted| mounted.document == self.document)
            {
                mounted.bounds = Some(bounds);
            }
        }
        let _guard = DocumentGuard::enter(self.document.clone(), window, cx);
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let _guard = DocumentGuard::enter(self.document.clone(), window, cx);
        self.child.paint(window, cx);
    }
}

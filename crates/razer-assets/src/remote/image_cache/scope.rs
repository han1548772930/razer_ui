//! Deferred drawing does not retain GPUI's image-cache stack. Keep the resource
//! cache with the child through layout, prepaint and paint without adding a box.
use super::resource_image_cache;
use gpui_kit::{
    AnyElement, AnyImageCache, App, Bounds, ContentMask, Deferred, Element, ElementId,
    GlobalElementId, ImageCacheProvider as _, InspectorElementId, IntoElement, LayoutId, Pixels,
    Point, Window,
};

pub struct ResourceScope {
    child: AnyElement,
}

/// Preserve requested-image loading across a portal or a deferred drawing pass.
/// The child retains its original layout node, bounds and positioning policy.
pub fn resource_scope(child: impl IntoElement) -> ResourceScope {
    ResourceScope {
        child: child.into_any_element(),
    }
}

/// Use the normal deferred layer while retaining its resource-loading owner.
pub fn deferred_resource(child: impl IntoElement) -> Deferred {
    gpui_kit::deferred(resource_scope(child))
}

/// A resource-scoped child retained by a custom element through its drawing phases.
/// Wrapping happens before layout; scheduling must not create an unlaid-out wrapper.
pub struct ResourceLayer {
    child: AnyElement,
}

impl ResourceLayer {
    pub fn new(child: impl IntoElement) -> Self {
        Self {
            child: resource_scope(child).into_any_element(),
        }
    }

    pub fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        self.child.request_layout(window, cx)
    }

    pub fn prepaint(&mut self, window: &mut Window, cx: &mut App) {
        self.child.prepaint(window, cx);
    }

    pub fn paint(&mut self, window: &mut Window, cx: &mut App) {
        self.child.paint(window, cx);
    }

    /// Consume the laid-out child during prepaint, as required by GPUI.
    pub fn schedule(
        self,
        window: &mut Window,
        absolute_offset: Point<Pixels>,
        priority: usize,
        content_mask: Option<ContentMask<Pixels>>,
    ) {
        window.defer_draw(self.child, absolute_offset, priority, content_mask);
    }
}

impl IntoElement for ResourceScope {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ResourceScope {
    type RequestLayoutState = AnyImageCache;
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
    ) -> (LayoutId, AnyImageCache) {
        let cache = resource_image_cache().provide(window, cx);
        let layout = window.with_image_cache(Some(cache.clone()), |window| {
            self.child.request_layout(window, cx)
        });
        (layout, cache)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        cache: &mut AnyImageCache,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_image_cache(Some(cache.clone()), |window| {
            self.child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        cache: &mut AnyImageCache,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_image_cache(Some(cache.clone()), |window| {
            self.child.paint(window, cx);
        });
    }
}

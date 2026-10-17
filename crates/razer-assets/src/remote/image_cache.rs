//! Wait for verified bytes before GPUI starts decoding, so pending is not a cached failure.
use gpui_kit::{
    AnyImageCache, App, AppContext as _, Entity, Global, ImageCache, ImageCacheError,
    ImageCacheItem, ImageCacheProvider, RenderImage, Resource, Subscription, Window, WindowId,
};
use std::{collections::HashMap, sync::Arc};

/// Owns one image cache for each open window. Element-scoped caches are not
/// sufficient here: deferred overlays and the root surface are separate GPUI
/// element trees, but they still belong to the same window and must share
/// single-flight loads and decoded images.
struct WindowResourceCaches {
    by_window: HashMap<WindowId, Entity<ResourceImageCache>>,
    _window_closed: Subscription,
}

impl Global for WindowResourceCaches {}

impl WindowResourceCaches {
    fn install(cx: &mut App) {
        let window_closed = cx.on_window_closed(|cx, window_id| {
            if cx.has_global::<Self>() {
                cx.global_mut::<Self>().by_window.remove(&window_id);
            }
        });
        cx.set_global(Self {
            by_window: HashMap::new(),
            _window_closed: window_closed,
        });
    }

    fn for_window(window: &mut Window, cx: &mut App) -> Entity<ResourceImageCache> {
        if !cx.has_global::<Self>() {
            Self::install(cx);
        }
        let window_id = window.window_handle().window_id();
        if let Some(cache) = cx.global::<Self>().by_window.get(&window_id) {
            return cache.clone();
        }
        let cache = ResourceImageCache::new(cx);
        cx.global_mut::<Self>()
            .by_window
            .insert(window_id, cache.clone());
        cache
    }
}

mod scope;
pub use scope::{ResourceLayer, ResourceScope, deferred_resource, resource_scope};

struct ResourceImageCache {
    images: HashMap<Resource, ImageCacheItem>,
    generation: u64,
}
impl ResourceImageCache {
    fn new(cx: &mut App) -> Entity<Self> {
        let cache = cx.new(|_| Self {
            images: HashMap::new(),
            generation: super::resource_generation(),
        });
        cx.observe_release(&cache, |cache, cx| {
            for (_, item) in cache.images.drain() {
                if let Some(Ok(image)) = item.get() {
                    cx.drop_image(image, None);
                }
            }
        })
        .detach();
        cache
    }

    fn refresh_generation(&mut self) {
        let generation = super::resource_generation();
        if self.generation == generation {
            return;
        }
        self.generation = generation;
        // GPUI's ImageCacheItem keeps a completed Err forever. Remote resource
        // completion/retry changes the bytes visible to AssetSource, so discard
        // only failed decodes at that boundary and let the next frame retry.
        self.images
            .retain(|_, item| !matches!(item.get(), Some(Err(_))));
    }
}
impl ImageCache for ResourceImageCache {
    fn load(
        &mut self,
        source: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        self.refresh_generation();
        if let Some(item) = self.images.get_mut(source) {
            return item.use_image(window);
        }
        if let Resource::Embedded(path) = source {
            match super::available(path) {
                Ok(false) => return None,
                Err(error) => return Some(Err(ImageCacheError::Asset(error.to_string().into()))),
                Ok(true) => {}
            }
        }
        self.images
            .entry(source.clone())
            .or_insert_with(|| ImageCacheItem::new(source, cx))
            .use_image(window)
    }
}

pub struct ResourceImageCacheProvider;
impl ImageCacheProvider for ResourceImageCacheProvider {
    fn provide(&mut self, window: &mut Window, cx: &mut App) -> AnyImageCache {
        WindowResourceCaches::for_window(window, cx).into()
    }
}

/// Place once around each window's content. Requests follow actual rendered images.
pub fn resource_image_cache() -> ResourceImageCacheProvider {
    ResourceImageCacheProvider
}

/// Decode through the same window owner used by rendered `img` elements.
/// Pending resource bytes stay pending without entering GPUI's asset cache.
pub fn load_resource_image(
    source: Resource,
    window: &mut Window,
    cx: &mut App,
) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
    let cache: AnyImageCache = WindowResourceCaches::for_window(window, cx).into();
    cache.load(&source, window, cx)
}

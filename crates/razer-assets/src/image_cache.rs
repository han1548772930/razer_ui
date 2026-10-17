//! Wait for verified bytes before GPUI starts decoding, so pending is not a cached failure.
use gpui_kit::{
    AnyImageCache, App, AppContext as _, Entity, Global, ImageCache, ImageCacheError,
    ImageCacheItem, ImageCacheProvider, RenderImage, Resource, Subscription, Window, WindowId,
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

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
}
impl ResourceImageCache {
    fn new(cx: &mut App) -> Entity<Self> {
        let cache = cx.new(|_| Self {
            images: HashMap::new(),
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

    fn retry_failed(&mut self) {
        self.images
            .retain(|_, item| !matches!(item.get(), Some(Err(_))));
    }

    fn errors(&self) -> impl Iterator<Item = String> + '_ {
        self.images.iter().filter_map(|(source, item)| {
            let Some(Err(error)) = item.get() else {
                return None;
            };
            let path = match source {
                Resource::Embedded(path) => path.to_string(),
                Resource::Path(path) => path.display().to_string(),
                Resource::Uri(uri) => uri.to_string(),
            };
            Some(format!("{path}: {error}"))
        })
    }
}
impl ImageCache for ResourceImageCache {
    fn load(
        &mut self,
        source: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        if let Some(item) = self.images.get_mut(source) {
            return item.use_image(window);
        }
        if let Resource::Embedded(path) = source {
            match available(path, cx) {
                Ok(false) => {
                    razer_resources::trace("image", path, "pending bytes");
                    return None;
                }
                Err(error) => {
                    razer_resources::trace("image", path, format!("unavailable {error:#}"));
                    return Some(Err(ImageCacheError::Asset(error.to_string().into())));
                }
                Ok(true) => razer_resources::trace("image", path, "decode started"),
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

pub(super) fn resource_image_errors(cx: &App) -> BTreeSet<String> {
    if !cx.has_global::<WindowResourceCaches>() {
        return BTreeSet::new();
    }
    cx.global::<WindowResourceCaches>()
        .by_window
        .values()
        .flat_map(|cache| cache.read(cx).errors())
        .collect()
}

pub(super) fn retry_resource_images(cx: &mut App) {
    if !cx.has_global::<WindowResourceCaches>() {
        return;
    }
    let caches = cx
        .global::<WindowResourceCaches>()
        .by_window
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for cache in caches {
        cache.update(cx, |cache, _| cache.retry_failed());
    }
}

#[cfg(test)]
mod tests;

fn available(path: &str, cx: &App) -> anyhow::Result<bool> {
    crate::resource_runtime::available(path, cx)
}

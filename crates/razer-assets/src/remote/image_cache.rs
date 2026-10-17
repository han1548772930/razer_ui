//! Wait for verified bytes before GPUI starts decoding, so pending is not a cached failure.
use gpui_kit::{
    AnyImageCache, App, AppContext as _, Entity, ImageCache, ImageCacheError, ImageCacheItem,
    ImageCacheProvider, RenderImage, Resource, Window,
};
use std::{collections::HashMap, sync::Arc};

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
        window
            .with_global_id("runtime-resource-images".into(), |id, window| {
                window.with_element_state::<Entity<ResourceImageCache>, _>(id, |cache, _| {
                    let cache = cache.unwrap_or_else(|| ResourceImageCache::new(cx));
                    (cache.clone(), cache)
                })
            })
            .into()
    }
}

/// Place once around each window's content. Requests follow actual rendered images.
pub fn resource_image_cache() -> ResourceImageCacheProvider {
    ResourceImageCacheProvider
}

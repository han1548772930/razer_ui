//! Wait for verified bytes before GPUI starts decoding, so pending is not a cached failure.
use gpui_kit::{
    AnyImageCache, App, AppContext as _, Context, Entity, Global, Image, ImageCache,
    ImageCacheError, ImageCacheItem, ImageCacheProvider, ImageFormat, ImageSource, RenderImage,
    Resource, SharedString, Subscription, Task, Window, WindowId,
};
use razer_storage::file_transfer::Transfer;
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
    lifecycle: DecoderLifecycle,
    _window_closed: Subscription,
}

#[derive(Default)]
enum DecoderLifecycle {
    #[default]
    Running,
    Stopped,
}

impl DecoderLifecycle {
    fn stop(&mut self) {
        *self = Self::Stopped;
    }

    fn check(&self) -> Result<(), ImageCacheError> {
        match self {
            Self::Running => Ok(()),
            Self::Stopped => Err(ImageCacheError::Asset(
                "Resource image cache is stopping".into(),
            )),
        }
    }
}

fn ensure_decoders_running(cx: &App) -> Result<(), ImageCacheError> {
    if let Some(caches) = cx.try_global::<WindowResourceCaches>() {
        caches.lifecycle.check()?;
    }
    Ok(())
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
            lifecycle: DecoderLifecycle::default(),
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
mod filters;
pub use filters::ImageFilter;

#[derive(Clone, PartialEq, Eq, Hash)]
struct FilteredIdentity {
    bytes: Arc<[u8]>,
    filters: Arc<[ImageFilter]>,
}

struct FilteredEntry {
    path: SharedString,
    result: Option<Result<Arc<RenderImage>, ImageCacheError>>,
    transfer: Transfer,
    task: Option<Task<()>>,
    last_used: u64,
}

impl Drop for FilteredEntry {
    fn drop(&mut self) {
        self.transfer.cancel();
        self.task.take();
    }
}

struct ResourceImageCache {
    images: HashMap<Resource, ImageCacheItem>,
    filtered: HashMap<FilteredIdentity, FilteredEntry>,
    filter_clock: u64,
}
impl ResourceImageCache {
    fn new(cx: &mut App) -> Entity<Self> {
        let cache = cx.new(|_| Self {
            images: HashMap::new(),
            filtered: HashMap::new(),
            filter_clock: 0,
        });
        cx.observe_release(&cache, |cache, cx| cache.clear(cx))
            .detach();
        cache
    }

    fn retry_failed(&mut self) {
        self.images
            .retain(|_, item| !matches!(item.get(), Some(Err(_))));
        self.filtered
            .retain(|_, entry| !matches!(entry.result, Some(Err(_))));
    }

    fn clear_filtered(&mut self, cx: &mut App) {
        discard_image_entries(
            &mut self.filtered,
            |entry| {
                entry
                    .result
                    .as_ref()
                    .and_then(|result| result.as_ref().ok())
                    .cloned()
            },
            |image| cx.drop_image(image, None),
        );
    }

    fn clear(&mut self, cx: &mut App) {
        discard_image_entries(
            &mut self.images,
            |entry| entry.get().and_then(Result::ok),
            |image| cx.drop_image(image, None),
        );
        self.clear_filtered(cx);
    }

    fn load_filtered(
        &mut self,
        path: SharedString,
        identity: FilteredIdentity,
        cx: &mut Context<Self>,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        if let Err(error) = ensure_decoders_running(cx) {
            return Some(Err(error));
        }
        self.filter_clock = self.filter_clock.wrapping_add(1);
        if let Some(entry) = self.filtered.get_mut(&identity) {
            entry.last_used = self.filter_clock;
            return entry.result.clone();
        }
        // Source filter transitions can produce intermediate variants. Keep
        // bounded per-window ownership while retaining recently used variants.
        if self.filtered.len() >= 64 {
            let oldest = self
                .filtered
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(identity, _)| identity.clone());
            if let Some(oldest) = oldest {
                if let Some(entry) = self.filtered.remove(&oldest) {
                    if let Some(Ok(image)) = entry.result.as_ref() {
                        cx.drop_image(image.clone(), None);
                    }
                }
            }
        }
        let transfer = Transfer::new();
        let operation = match transfer.accept() {
            Ok(operation) => operation,
            Err(error) => return Some(Err(ImageCacheError::Asset(error.into()))),
        };
        let renderer = cx.svg_renderer();
        let job_identity = identity.clone();
        let worker = cx.background_executor().spawn(async move {
            let check = || operation.check().map_err(std::io::Error::other);
            check()?;
            let decoded = Image::from_bytes(ImageFormat::Svg, job_identity.bytes.to_vec())
                .to_image_data(renderer)
                .map_err(std::io::Error::other)?;
            check()?;
            if decoded.frame_count() != 1 {
                return Err(std::io::Error::other("Filtered SVG must have one frame"));
            }
            let size = decoded.size(0);
            let width = u32::try_from(size.width.0)
                .ok()
                .filter(|width| *width > 0)
                .ok_or_else(|| std::io::Error::other("Invalid filtered SVG width"))?;
            let height = u32::try_from(size.height.0)
                .ok()
                .filter(|height| *height > 0)
                .ok_or_else(|| std::io::Error::other("Invalid filtered SVG height"))?;
            let mut pixels = decoded
                .as_bytes(0)
                .ok_or_else(|| std::io::Error::other("Filtered SVG has no pixels"))?
                .to_vec();
            // Chunking bounds cancellation latency for larger source SVGs.
            for chunk in pixels.chunks_mut(16 * 1024) {
                check()?;
                filters::filter_bgra(chunk, &job_identity.filters)
                    .map_err(std::io::Error::other)?;
            }
            // RenderImage expects BGRA despite image::Frame's RGBA container.
            let buffer = image::RgbaImage::from_raw(width, height, pixels)
                .ok_or_else(|| std::io::Error::other("Filtered SVG dimensions differ"))?;
            check()?;
            Ok(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
        });
        let completion_identity = identity.clone();
        let completion_transfer = transfer.clone();
        let task = cx.spawn(async move |cache, cx| {
            let result = worker.await.map_err(Into::into);
            let _ = cache.update(cx, |cache, cx| {
                let Some(entry) = cache.filtered.get_mut(&completion_identity) else {
                    return;
                };
                if !entry.transfer.is_same(&completion_transfer)
                    || completion_transfer.check().is_err()
                {
                    return;
                }
                entry.task = None;
                entry.result = Some(result);
                cx.refresh_windows();
            });
        });
        self.filtered.insert(
            identity,
            FilteredEntry {
                path,
                result: None,
                transfer,
                task: Some(task),
                last_used: self.filter_clock,
            },
        );
        None
    }

    fn errors(&self) -> impl Iterator<Item = String> + '_ {
        self.images
            .iter()
            .filter_map(|(source, item)| {
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
            .chain(self.filtered.values().filter_map(|entry| {
                let Some(Err(error)) = entry.result.as_ref() else {
                    return None;
                };
                Some(format!("{} (CSS filter): {error}", entry.path))
            }))
    }
}
impl ImageCache for ResourceImageCache {
    fn load(
        &mut self,
        source: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        if let Err(error) = ensure_decoders_running(cx) {
            return Some(Err(error));
        }
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

/// Apply source CSS to an SVG through the same window cache and verified byte
/// acquisition path as ordinary images. Pixel work runs on the background
/// executor; the complete bytes and ordered filter sequence identify a variant.
pub fn filtered_resource_svg(
    path: impl Into<SharedString>,
    filters: impl AsRef<[ImageFilter]>,
) -> ImageSource {
    let path = path.into();
    let filters = Arc::<[ImageFilter]>::from(filters.as_ref());
    ImageSource::Custom(Arc::new(move |window, cx| {
        if let Err(error) = ensure_decoders_running(cx) {
            return Some(Err(error));
        }
        match available(&path, cx) {
            Ok(false) => return None,
            Err(error) => return Some(Err(ImageCacheError::Asset(error.to_string().into()))),
            Ok(true) => {}
        }
        let bytes = match crate::load_path(&path, cx) {
            Ok(Some(bytes)) => Arc::<[u8]>::from(bytes.into_owned()),
            Ok(None) => {
                return Some(Err(ImageCacheError::Asset(
                    format!("Missing filtered SVG: {path}").into(),
                )));
            }
            Err(error) => return Some(Err(ImageCacheError::Asset(error.to_string().into()))),
        };
        let identity = FilteredIdentity {
            bytes,
            filters: filters.clone(),
        };
        let cache = WindowResourceCaches::for_window(window, cx);
        cache.update(cx, |cache, cx| {
            cache.load_filtered(path.clone(), identity, cx)
        })
    }))
}

/// Stop decoder work before the shell drains accepted local I/O on exit.
pub(super) fn stop_resource_images(cx: &mut App) {
    if !cx.has_global::<WindowResourceCaches>() {
        WindowResourceCaches::install(cx);
    }
    let caches = cx
        .global::<WindowResourceCaches>()
        .by_window
        .values()
        .cloned()
        .collect::<Vec<_>>();
    cx.global_mut::<WindowResourceCaches>().lifecycle.stop();
    for cache in caches {
        cache.update(cx, |cache, cx| cache.clear(cx));
    }
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

#[cfg(test)]
mod lifecycle_tests;

/// Drop the retained task and cached result together, releasing an uploaded
/// image before its entry disappears. Both normal and filtered loads use this.
fn discard_image_entries<K, E, I>(
    entries: &mut HashMap<K, E>,
    loaded: impl Fn(&E) -> Option<I>,
    mut release: impl FnMut(I),
) {
    for (_, entry) in entries.drain() {
        if let Some(image) = loaded(&entry) {
            release(image);
        }
    }
}

fn available(path: &str, cx: &App) -> anyhow::Result<bool> {
    crate::resource_runtime::available(path, cx)
}

#[cfg(test)]
mod filtered_identity_tests {
    use super::*;

    #[test]
    fn css_filters_cache_identity_uses_actual_bytes_and_ordered_sequence() {
        let identity = |bytes: &[u8], filters: &[ImageFilter]| FilteredIdentity {
            bytes: bytes.into(),
            filters: filters.into(),
        };
        let a = identity(
            b"actual SVG A",
            &[ImageFilter::Brightness(120), ImageFilter::Invert(20)],
        );
        let same = identity(
            b"actual SVG A",
            &[ImageFilter::Brightness(120), ImageFilter::Invert(20)],
        );
        let changed_bytes = identity(
            b"actual SVG B",
            &[ImageFilter::Brightness(120), ImageFilter::Invert(20)],
        );
        let changed_order = identity(
            b"actual SVG A",
            &[ImageFilter::Invert(20), ImageFilter::Brightness(120)],
        );
        let changed_amount = identity(
            b"actual SVG A",
            &[ImageFilter::Brightness(100), ImageFilter::Invert(20)],
        );
        let mut identities = std::collections::HashSet::new();
        assert!(identities.insert(a));
        assert!(!identities.insert(same));
        assert!(identities.insert(changed_bytes));
        assert!(identities.insert(changed_order));
        assert!(identities.insert(changed_amount));
    }
}

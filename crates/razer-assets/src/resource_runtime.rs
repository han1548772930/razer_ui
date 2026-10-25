//! GPUI bridge for the application-owned resource runtime.
use gpui_kit::{
    App, AppContext as _, BorrowAppContext as _, Context, Entity, EventEmitter, Global,
    Subscription, Task,
};
pub use razer_resources::ResourceChanged;
use razer_resources::{ResourceClient, ResourceFont, ResourceRuntime};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

struct FontRegistration {
    bytes: Arc<[u8]>,
    result: Result<(), String>,
}

#[derive(Default)]
struct FontRegistrations(BTreeMap<String, FontRegistration>);

impl FontRegistrations {
    fn register(
        &mut self,
        font: ResourceFont,
        retry: bool,
        install: impl FnOnce(&[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        if let Some(previous) = self.0.get(&font.path) {
            if previous.bytes == font.bytes && (previous.result.is_ok() || !retry) {
                return previous.result.clone();
            }
        }
        let result = install(&font.bytes);
        self.0.insert(
            font.path,
            FontRegistration {
                bytes: font.bytes,
                result: result.clone(),
            },
        );
        result
    }

    fn errors(&self) -> impl Iterator<Item = String> + '_ {
        self.0.iter().filter_map(|(path, font)| {
            font.result
                .as_ref()
                .err()
                .map(|error| format!("{path}：{error}"))
        })
    }
}

/// Presentation diagnostics also cover failures before a worker can emit an event.
#[derive(Default)]
struct AvailabilityErrors(BTreeMap<String, String>);
impl Global for AvailabilityErrors {}
impl AvailabilityErrors {
    fn observe(&mut self, path: &str, error: Option<String>) -> bool {
        match error {
            Some(error) if self.0.get(path) != Some(&error) => {
                self.0.insert(path.to_owned(), error);
                true
            }
            Some(_) => false,
            None => {
                self.0.remove(path);
                false
            }
        }
    }
}

pub struct ResourceEvents;
impl EventEmitter<ResourceChanged> for ResourceEvents {}
struct Events(Entity<ResourceEvents>);
impl Global for Events {}
fn events(cx: &mut App) -> Entity<ResourceEvents> {
    if !cx.has_global::<Events>() {
        let entity = cx.new(|_| ResourceEvents);
        cx.set_global(Events(entity));
    }
    cx.global::<Events>().0.clone()
}

/// Retain in the view which owns resource-derived presentation.
pub fn observe_resources<T: 'static>(
    cx: &mut Context<T>,
    callback: impl Fn(&mut T, &ResourceChanged, &mut Context<T>) + 'static,
) -> Subscription {
    let events = events(cx);
    cx.subscribe(&events, move |owner, _, event, cx| {
        callback(owner, event, cx)
    })
}

struct Runtime {
    owner: Option<ResourceRuntime>,
    client: ResourceClient,
    fonts: FontRegistrations,
    results: Option<Task<()>>,
    _quit: Subscription,
}
impl Global for Runtime {}

/// Attach the explicitly constructed service to this application only.
pub fn start_resources(owner: ResourceRuntime, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(
        !cx.has_global::<Runtime>(),
        "Resource runtime is already installed"
    );
    let changes = owner.changes();
    let client = owner.client();
    let event_owner = events(cx);
    let result_client = client.clone();
    let results = cx.spawn(async move |cx| {
        while let Ok(changed) = changes.recv().await {
            cx.update(|cx| {
                if changed.error().is_none() && cx.has_global::<AvailabilityErrors>() {
                    cx.global_mut::<AvailabilityErrors>()
                        .observe(changed.path(), None);
                }
                if changed.error().is_none() && result_client.is_font(changed.path()) {
                    if let Ok(Some(bytes)) = result_client.load(changed.path()) {
                        let font = ResourceFont::new(changed.path().to_owned(), bytes.into());
                        if let Err(error) = register_font(font, false, cx) {
                            log::warn!("{}：字体加载失败：{error}", changed.path());
                        }
                    }
                }
                event_owner.update(cx, |_, cx| cx.emit(changed));
                cx.refresh_windows();
            });
        }
    });
    // The shell drains this owner before requesting platform quit. This
    // callback is only a fallback because the framework bounds quit handlers.
    let quit = cx.on_app_quit(|cx| {
        let owner = take_resource_runtime_for_shutdown(cx);
        let cleanup = cx.background_executor().spawn(async move {
            if let Some(mut owner) = owner {
                if let Err(error) = owner.shutdown() {
                    eprintln!("资源清理失败：{error:#}");
                }
            }
        });
        async move { cleanup.await }
    });
    cx.set_global(Runtime {
        owner: Some(owner),
        client,
        fonts: FontRegistrations::default(),
        results: Some(results),
        _quit: quit,
    });
    cx.set_global(AvailabilityErrors::default());
    Ok(())
}

/// Transfer the application owner to its retained background shutdown task.
/// Requests and presentation notifications stop immediately; the caller must
/// finish `ResourceRuntime::shutdown` before completing application exit.
pub fn take_resource_runtime_for_shutdown(cx: &mut App) -> Option<ResourceRuntime> {
    crate::image_cache::stop_resource_images(cx);
    if !cx.has_global::<Runtime>() {
        return None;
    }
    let runtime = cx.global_mut::<Runtime>();
    let owner = runtime.owner.take()?;
    owner.begin_shutdown();
    runtime.results.take();
    Some(owner)
}

pub fn register_fonts(cx: &mut App) -> anyhow::Result<()> {
    register_available_fonts(false, cx)
}

fn register_font(font: ResourceFont, retry: bool, cx: &mut App) -> Result<(), String> {
    cx.update_global::<Runtime, _>(|runtime, cx| {
        runtime.fonts.register(font, retry, |bytes| {
            cx.text_system()
                .add_fonts(vec![Cow::Owned(bytes.to_vec())])
                .map_err(|error| format!("{error:#}"))
        })
    })
}

fn register_available_fonts(retry: bool, cx: &mut App) -> anyhow::Result<()> {
    let fonts = cx
        .try_global::<Runtime>()
        .ok_or_else(|| anyhow::anyhow!("Resource runtime is not installed"))?
        .client
        .fonts()?;
    let mut errors = Vec::new();
    for font in fonts {
        let path = font.path.clone();
        if let Err(error) = register_font(font, retry, cx) {
            errors.push(format!("{path}：{error}"));
        }
    }
    anyhow::ensure!(errors.is_empty(), "{}", errors.join("\n"));
    Ok(())
}

pub fn resource_errors(cx: &App) -> Vec<String> {
    let mut errors = crate::image_cache::resource_image_errors(cx);
    if let Some(availability) = cx.try_global::<AvailabilityErrors>() {
        errors.extend(
            availability
                .0
                .iter()
                .map(|(path, error)| format!("{path}：{error}")),
        );
    }
    if let Some(runtime) = cx.try_global::<Runtime>() {
        errors.extend(runtime.client.errors());
        errors.extend(runtime.fonts.errors());
    } else {
        errors.insert("Resource runtime is not installed".into());
    }
    errors.into_iter().collect()
}
pub fn retry_resources(cx: &mut App) -> anyhow::Result<()> {
    cx.try_global::<Runtime>()
        .ok_or_else(|| anyhow::anyhow!("Resource runtime is not installed"))?
        .client
        .retry()?;
    crate::image_cache::retry_resource_images(cx);
    if cx.has_global::<AvailabilityErrors>() {
        cx.global_mut::<AvailabilityErrors>().0.clear();
    }
    // A verified file can be ready even though the platform rejected its font.
    // Reuse those bytes and retry only failed registrations; no download is needed.
    let fonts = register_available_fonts(true, cx);
    cx.refresh_windows();
    fonts
}

pub(crate) fn available(path: &str, cx: &mut App) -> anyhow::Result<bool> {
    let result = query_availability(path, cx);
    let path = crate::resource_identity(path).map_or(path, |(path, _, _)| path);
    if !cx.has_global::<AvailabilityErrors>() {
        cx.set_global(AvailabilityErrors::default());
    }
    let error = result.as_ref().err().map(|error| format!("{error:#}"));
    if cx
        .global_mut::<AvailabilityErrors>()
        .observe(path, error.clone())
    {
        if let Some(error) = error {
            log::warn!(target: "razer_assets::resources", "unavailable {path}: {error}");
        }
    }
    result
}

fn query_availability(path: &str, cx: &App) -> anyhow::Result<bool> {
    if crate::embedded_asset(path).is_some() {
        return Ok(true);
    }
    if let Some(runtime) = cx.try_global::<Runtime>() {
        return runtime.client.available(path);
    }
    anyhow::ensure!(
        crate::resource_identity(path).is_none(),
        "Resource runtime is not installed"
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{AvailabilityErrors, FontRegistrations};
    use razer_resources::ResourceFont;
    use std::{cell::Cell, sync::Arc};

    fn font(path: &str, bytes: &[u8]) -> ResourceFont {
        ResourceFont::new(path.to_owned(), Arc::from(bytes))
    }

    #[test]
    fn font_registration_failure_remains_visible_until_explicit_retry_succeeds() {
        let mut fonts = FontRegistrations::default();
        let calls = Cell::new(0);
        let path = "assets/fonts/source-regular.ttf";
        assert_eq!(
            fonts.register(font(path, b"verified font"), false, |bytes| {
                assert_eq!(bytes, b"verified font");
                calls.set(calls.get() + 1);
                Err("platform font collection failed".into())
            }),
            Err("platform font collection failed".into())
        );
        assert_eq!(
            fonts.errors().collect::<Vec<_>>(),
            [format!("{path}：platform font collection failed")]
        );
        // Repeated resource notifications cannot turn a failed registration into success.
        assert!(
            fonts
                .register(font(path, b"verified font"), false, |_| {
                    panic!("failed fonts require an explicit retry")
                })
                .is_err()
        );
        fonts
            .register(font(path, b"verified font"), true, |bytes| {
                assert_eq!(bytes, b"verified font");
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap();
        assert_eq!(calls.get(), 2);
        assert!(fonts.errors().next().is_none());
        fonts
            .register(font(path, b"verified font"), true, |_| {
                panic!("a retry must not reinstall a successful font")
            })
            .unwrap();
    }

    #[test]
    fn retrying_a_failed_font_preserves_other_fonts_and_actual_failure_details() {
        let mut fonts = FontRegistrations::default();
        let regular = "assets/fonts/source-regular.ttf";
        let bold = "assets/fonts/source-bold.ttf";
        fonts
            .register(font(regular, b"regular"), false, |_| Ok(()))
            .unwrap();
        assert!(
            fonts
                .register(font(bold, b"bold"), false, |_| Err("first failure".into()))
                .is_err()
        );
        assert!(
            fonts
                .register(font(bold, b"bold"), true, |_| Err("retry failure".into()))
                .is_err()
        );
        assert_eq!(
            fonts.errors().collect::<Vec<_>>(),
            [format!("{bold}：retry failure")]
        );
        fonts
            .register(font(regular, b"regular"), true, |_| {
                panic!("another font's retry must keep a successful registration")
            })
            .unwrap();
    }

    #[test]
    fn font_registration_identity_compares_actual_bytes_with_the_source_path() {
        let mut fonts = FontRegistrations::default();
        let path = "assets/fonts/source.ttf";
        fonts
            .register(font(path, b"first font"), false, |_| Ok(()))
            .unwrap();
        let installed = Cell::new(false);
        assert!(
            fonts
                .register(font(path, b"replacement font"), false, |bytes| {
                    assert_eq!(bytes, b"replacement font");
                    installed.set(true);
                    Err("replacement rejected".into())
                })
                .is_err()
        );
        assert!(installed.get());
        assert_eq!(
            fonts.errors().collect::<Vec<_>>(),
            [format!("{path}：replacement rejected")]
        );
        fonts
            .register(
                font("assets/fonts/other.ttf", b"replacement font"),
                false,
                |_| Ok(()),
            )
            .unwrap();
        assert_eq!(fonts.errors().count(), 1);
    }

    #[test]
    fn resource_availability_errors_report_transitions_and_remain_queryable() {
        let mut errors = AvailabilityErrors::default();
        let path = "synapse/test.svg";
        assert!(errors.observe(path, Some("Resource runtime is not installed".into())));
        assert!(!errors.observe(path, Some("Resource runtime is not installed".into())));
        assert_eq!(errors.0[path], "Resource runtime is not installed");
        assert!(errors.observe(path, Some("HTTP 403".into())));
        assert!(!errors.observe(path, Some("HTTP 403".into())));
        assert_eq!(errors.0[path], "HTTP 403");
        assert!(!errors.observe(path, None));
        assert!(errors.0.is_empty());
        assert!(errors.observe(path, Some("HTTP 403".into())));
    }
}

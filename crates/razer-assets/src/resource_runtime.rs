//! GPUI bridge for the application-owned resource runtime.
use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Global, Subscription, Task};
pub use razer_resources::ResourceChanged;
use razer_resources::{ResourceClient, ResourceRuntime};
use std::borrow::Cow;

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
                if changed.error().is_none() && result_client.is_font(changed.path()) {
                    if let Ok(Some(bytes)) = result_client.load(changed.path()) {
                        if let Err(error) = cx.text_system().add_fonts(vec![Cow::Owned(bytes)]) {
                            eprintln!("字体加载失败：{error}");
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
        results: Some(results),
        _quit: quit,
    });
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

pub fn register_fonts(cx: &App) -> anyhow::Result<()> {
    let fonts = cx
        .try_global::<Runtime>()
        .ok_or_else(|| anyhow::anyhow!("Resource runtime is not installed"))?
        .client
        .fonts()?;
    if !fonts.is_empty() {
        cx.text_system()
            .add_fonts(fonts.into_iter().map(Cow::Owned).collect())?;
    }
    Ok(())
}

pub fn resource_errors(cx: &App) -> Vec<String> {
    let mut errors = crate::image_cache::resource_image_errors(cx);
    if let Some(runtime) = cx.try_global::<Runtime>() {
        errors.extend(runtime.client.errors());
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
    cx.refresh_windows();
    Ok(())
}

pub(crate) fn available(path: &str, cx: &App) -> anyhow::Result<bool> {
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

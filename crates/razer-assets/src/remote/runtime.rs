use super::{Cache, GENERATION, ResourceFile, manifest};
use anyhow::Context as _;
use futures::{StreamExt as _, stream};
use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Global, Subscription, Task};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock, atomic::Ordering},
    time::Duration,
};

const MAX_CONCURRENT_DOWNLOADS: usize = 4;
const MAX_REQUEST_BATCH: usize = 64;
const REQUEST_BATCH_WINDOW: Duration = Duration::from_millis(8);

enum FileState {
    Pending,
    Ready(Arc<[u8]>),
    Failed(String),
}

pub(super) struct Service {
    cache: Cache,
    states: Mutex<BTreeMap<String, FileState>>,
    sender: async_channel::Sender<ResourceFile>,
    receiver: async_channel::Receiver<ResourceFile>,
}

pub(super) fn service() -> anyhow::Result<&'static Arc<Service>> {
    static SERVICE: OnceLock<Result<Arc<Service>, String>> = OnceLock::new();
    SERVICE
        .get_or_init(|| {
            let initialize = || -> anyhow::Result<Arc<Service>> {
                let (sender, receiver) = async_channel::bounded(manifest()?.files.len());
                Ok(Arc::new(Service {
                    cache: Cache::alongside(&std::env::current_exe()?)?,
                    states: Mutex::new(BTreeMap::new()),
                    sender,
                    receiver,
                }))
            };
            initialize().map_err(|error| format!("{error:#}"))
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

impl Service {
    fn collect_batch(&self, first: ResourceFile) -> Vec<ResourceFile> {
        let mut batch = vec![first];
        while batch.len() < MAX_REQUEST_BATCH {
            let Ok(file) = self.receiver.try_recv() else { break };
            batch.push(file);
        }
        batch
    }

    async fn resolve<F: Future<Output = anyhow::Result<Vec<u8>>>>(
        &self,
        file: &ResourceFile,
        download: impl FnOnce() -> F,
    ) -> anyhow::Result<Arc<[u8]>> {
        anyhow::ensure!(!self.receiver.is_closed(), "资源加载已取消");
        if let Some(bytes) = self.cache.read(file)? {
            return Ok(bytes.into());
        }
        let bytes = download().await?;
        anyhow::ensure!(!self.receiver.is_closed(), "资源加载已取消");
        self.cache.publish(file, &bytes)?;
        Ok(bytes.into())
    }

    fn stop(&self) {
        self.receiver.close();
        while self.receiver.try_recv().is_ok() {}
        if let Ok(mut states) = self.states.lock() {
            for state in states.values_mut() {
                if matches!(state, FileState::Pending) {
                    *state = FileState::Failed("资源加载已取消".into());
                }
            }
        }
    }

    pub(super) fn read_or_request(&self, file: &ResourceFile) -> anyhow::Result<Option<Arc<[u8]>>> {
        if let Some(bytes) = super::embedded(&file.path) {
            return Ok(Some(bytes.into()));
        }
        let mut states = self
            .states
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource state lock is poisoned"))?;
        if let Some(FileState::Ready(bytes)) = states.get(&file.path) {
            return Ok(Some(bytes.clone()));
        }
        if let Some(FileState::Failed(error)) = states.get(&file.path) {
            anyhow::bail!("{error}");
        }
        if !matches!(states.get(&file.path), Some(FileState::Pending)) {
            states.insert(file.path.clone(), FileState::Pending);
            if let Err(error) = self.sender.try_send(file.clone()) {
                states.insert(
                    file.path.clone(),
                    FileState::Failed("资源加载队列不可用".into()),
                );
                return Err(error).context("Resource loader stopped");
            }
        }
        Ok(None)
    }

    fn complete(&self, file: &ResourceFile, result: &Result<Arc<[u8]>, String>) {
        if self.receiver.is_closed() {
            return;
        }
        if let Ok(mut states) = self.states.lock() {
            states.insert(
                file.path.clone(),
                match result {
                    Ok(bytes) => FileState::Ready(bytes.clone()),
                    Err(error) => FileState::Failed(error.clone()),
                },
            );
        }
    }

    fn retry(&self) -> anyhow::Result<()> {
        let manifest = manifest()?;
        let mut states = self
            .states
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource state lock is poisoned"))?;
        for (path, state) in states.iter_mut() {
            if matches!(state, FileState::Failed(_)) {
                if let Some(&ix) = manifest.paths.get(path) {
                    self.sender
                        .try_send(manifest.files[ix].clone())
                        .context("Resource loader stopped")?;
                    *state = FileState::Pending;
                }
            }
        }
        Ok(())
    }

    fn errors(&self) -> Vec<String> {
        self.states
            .lock()
            .map(|states| {
                states
                    .iter()
                    .filter_map(|(path, state)| {
                        if let FileState::Failed(error) = state {
                            Some(format!("{path}：{error}"))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Notification for a completed request, including failures. Device state is untouched.
#[derive(Clone)]
pub struct ResourceChanged {
    path: Arc<str>,
    error: Option<Arc<str>>,
}
impl ResourceChanged {
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
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

/// Retain the subscription in the view that owns resource-derived presentation.
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
    service: Arc<Service>,
    _loader: Task<()>,
    _results: Task<()>,
}
impl Global for Runtime {}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.service.stop();
    }
}

/// Start bounded workers for files requested by mounted views and shared dependencies.
pub fn start_resources(cx: &mut App) -> anyhow::Result<()> {
    if cx.has_global::<Runtime>() {
        return Ok(());
    }
    let service = service()?.clone();
    let client = Arc::new(Mutex::new(
        None::<Arc<dyn gpui_kit::http_client::HttpClient>>,
    ));
    let (finished, results) = async_channel::unbounded::<(ResourceFile, Result<(), String>)>();
    let executor = cx.background_executor().clone();
    let loader = {
        let service = service.clone();
        let requests = stream::unfold(service.clone(), move |service| {
            let executor = executor.clone();
            async move {
                let first = service.receiver.recv().await.ok()?;
                executor.timer(REQUEST_BATCH_WINDOW).await;
                let batch = service.collect_batch(first);
                Some((stream::iter(batch), service))
            }
        })
        .flatten();
        cx.background_executor().spawn(async move {
            requests
                .for_each_concurrent(MAX_CONCURRENT_DOWNLOADS, move |file| {
                    let service = service.clone();
                    let client = client.clone();
                    let finished = finished.clone();
                    async move {
                        let result = service
                            .resolve(&file, || async {
                                let client = {
                                    let mut client = client.lock().map_err(|_| {
                                        anyhow::anyhow!("Resource HTTP client lock is poisoned")
                                    })?;
                                    if client.is_none() {
                                        *client = Some(super::transport::Client::new()?);
                                    }
                                    client.as_ref().expect("HTTP client initialized").clone()
                                };
                                super::transport::download(client, &file).await
                            })
                            .await
                            .map_err(|error| format!("{error:#}"));
                        service.complete(&file, &result);
                        let _ = finished.send((file, result.map(|_| ()))).await;
                    }
                })
                .await;
        })
    };
    let events = events(cx);
    let results = cx.spawn(async move |cx| {
        while let Ok((file, result)) = results.recv().await {
            let changed = ResourceChanged {
                path: file.path.into(),
                error: result.err().map(Into::into),
            };
            let _ = cx.update(|cx| {
                GENERATION.fetch_add(1, Ordering::AcqRel);
                if changed.error.is_none()
                    && manifest().is_ok_and(|manifest| {
                        manifest
                            .fonts
                            .iter()
                            .any(|font| font.path == changed.path())
                    })
                {
                    if let Ok(Some(bytes)) = super::load_path(changed.path()) {
                        if let Err(error) = cx
                            .text_system()
                            .add_fonts(vec![std::borrow::Cow::Owned(bytes)])
                        {
                            eprintln!("字体加载失败：{error}");
                        }
                    }
                }
                events.update(cx, |_, cx| cx.emit(changed));
                cx.refresh_windows();
            });
        }
    });
    cx.set_global(Runtime {
        service,
        _loader: loader,
        _results: results,
    });
    Ok(())
}

pub fn resource_errors() -> Vec<String> {
    service()
        .map(|service| service.errors())
        .unwrap_or_else(|error| vec![error.to_string()])
}
pub fn retry_resources() -> anyhow::Result<()> {
    service()?.retry()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{executor::block_on, task::noop_waker};
    use std::{
        pin::pin,
        task::{Context as TaskContext, Poll},
    };

    fn test_service(root: std::path::PathBuf) -> Service {
        let (sender, receiver) = async_channel::bounded(256);
        Service {
            cache: Cache { root },
            states: Mutex::new(BTreeMap::new()),
            sender,
            receiver,
        }
    }

    #[test]
    fn opened_shared_and_product_resources_batch_without_waiting_for_io() {
        let directory = super::super::tests::TemporaryDirectory::new();
        let service = test_service(directory.0.clone());
        let shared =
            super::super::tests::file("assets/synapse/shared/settings/icon.svg", b"shared");
        let product = super::super::tests::file("assets/synapse/products/182/icon.svg", b"product");
        let unopened =
            super::super::tests::file("assets/synapse/products/214/icon.svg", b"unopened");
        for file in [&shared, &product, &shared] {
            assert!(service.read_or_request(file).unwrap().is_none());
        }
        let batch = service.collect_batch(service.receiver.try_recv().unwrap());
        assert_eq!(
            batch
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            [&shared.path, &product.path]
        );
        let (release, response) = async_channel::bounded(1);
        let mut loading = pin!(service.resolve(&shared, || async {
            response.recv().await?;
            Ok(b"shared".to_vec())
        }));
        let waker = noop_waker();
        let mut context = TaskContext::from_waker(&waker);
        assert!(matches!(loading.as_mut().poll(&mut context), Poll::Pending));
        assert!(service.read_or_request(&shared).unwrap().is_none());
        assert!(service.read_or_request(&product).unwrap().is_none());
        assert!(service.receiver.is_empty());
        assert!(!service.states.lock().unwrap().contains_key(&unopened.path));
        release.try_send(()).unwrap();
        let bytes = block_on(loading).unwrap();
        service.complete(&shared, &Ok(bytes.clone()));
        assert!(Arc::ptr_eq(
            &service.read_or_request(&shared).unwrap().unwrap(),
            &bytes
        ));
        let cached = block_on(service.resolve(&shared, || async {
            panic!("A cache hit must not initialize HTTP, read credentials or download");
        }))
        .unwrap();
        assert_eq!(cached.as_ref(), b"shared");
        assert!(!service.cache.path(&product).unwrap().exists());
        assert!(!service.cache.path(&unopened).unwrap().exists());
    }

    #[test]
    fn cancellation_before_download_completion_never_publishes_late_bytes() {
        let directory = super::super::tests::TemporaryDirectory::new();
        let service = test_service(directory.0.clone());
        let file = super::super::tests::file("assets/synapse/shared/help/clip.gif", b"clip");
        let (release, response) = async_channel::bounded(1);
        let mut loading = pin!(service.resolve(&file, || async {
            response.recv().await?;
            Ok(b"clip".to_vec())
        }));
        let waker = noop_waker();
        assert!(
            loading
                .as_mut()
                .poll(&mut TaskContext::from_waker(&waker))
                .is_pending()
        );
        service.stop();
        release.try_send(()).unwrap();
        assert!(block_on(loading).unwrap_err().to_string().contains("取消"));
        assert!(!service.cache.path(&file).unwrap().exists());
    }

    #[test]
    fn request_batches_remain_bounded_and_keep_remaining_files_queued() {
        let service = test_service(std::path::PathBuf::from("unused-batch-test-cache"));
        for index in 0..MAX_REQUEST_BATCH + 3 {
            let file =
                super::super::tests::file(&format!("assets/synapse/shared/{index}.svg"), b"icon");
            assert!(service.read_or_request(&file).unwrap().is_none());
        }
        let first = service.receiver.try_recv().unwrap();
        assert_eq!(service.collect_batch(first).len(), MAX_REQUEST_BATCH);
        assert_eq!(service.receiver.len(), 3);
        assert!(!service.cache.root.exists());
    }

    #[test]
    fn pending_requests_deduplicate_and_failures_requeue_on_retry() {
        let (sender, receiver) = async_channel::bounded(8);
        let service = Service {
            cache: Cache {
                root: std::path::PathBuf::from("unused-resource-test-cache"),
            },
            states: Mutex::new(BTreeMap::new()),
            sender,
            receiver,
        };
        let file = &manifest().unwrap().fonts[0];
        assert!(service.read_or_request(file).unwrap().is_none());
        assert!(service.read_or_request(file).unwrap().is_none());
        assert_eq!(service.receiver.len(), 1);
        assert_eq!(service.receiver.try_recv().unwrap().path, file.path);
        service.complete(file, &Err("offline".into()));
        assert!(service.read_or_request(file).is_err());
        assert_eq!(service.errors().len(), 1);
        service.retry().unwrap();
        assert!(service.errors().is_empty());
        assert_eq!(service.receiver.len(), 1);
        let bytes: Arc<[u8]> = Arc::from(b"cached bytes".as_slice());
        service.complete(file, &Ok(bytes.clone()));
        assert_eq!(
            service.read_or_request(file).unwrap().unwrap().as_ref(),
            bytes.as_ref()
        );
        assert_eq!(service.receiver.len(), 1);
    }

    #[test]
    fn stopping_owner_cancels_pending_requests_and_rejects_late_completion() {
        let (sender, receiver) = async_channel::bounded(8);
        let service = Service {
            cache: Cache {
                root: std::path::PathBuf::from("unused-cancel-test-cache"),
            },
            states: Mutex::new(BTreeMap::new()),
            sender,
            receiver,
        };
        let file = &manifest().unwrap().fonts[0];
        assert!(service.read_or_request(file).unwrap().is_none());
        service.stop();
        assert_eq!(service.receiver.len(), 0);
        service.complete(file, &Ok(Arc::from(b"late completion".as_slice())));
        assert!(
            service
                .read_or_request(file)
                .unwrap_err()
                .to_string()
                .contains("取消")
        );
        assert!(service.retry().is_err());
        assert!(!service.cache.root.exists());
    }
}

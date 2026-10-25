use crate::{
    ResourcePending,
    cache::Cache,
    catalog::{ResourceFile, manifest},
    trace,
};
use anyhow::Context as _;
use futures::{StreamExt as _, future::BoxFuture, stream};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
    thread::JoinHandle,
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

#[derive(Default, PartialEq, Eq)]
enum Lifecycle {
    #[default]
    Running,
    Closing,
    Stopped,
}

#[derive(Default)]
struct State {
    lifecycle: Lifecycle,
    failure: Option<String>,
    files: BTreeMap<String, FileState>,
}

struct Service {
    cache: Cache,
    state: Mutex<State>,
    publication: Mutex<()>,
    sender: async_channel::Sender<ResourceFile>,
    receiver: async_channel::Receiver<ResourceFile>,
    cancelled: async_channel::Sender<()>,
}

/// A cloneable, memory-only client. It cannot keep the worker alive after its owner closes.
#[derive(Clone)]
pub struct ResourceClient(Arc<Service>);

/// One verified font file, retaining its catalog identity for presentation errors and retries.
#[non_exhaustive]
pub struct ResourceFont {
    pub path: String,
    pub bytes: Arc<[u8]>,
}

impl ResourceFont {
    pub fn new(path: String, bytes: Arc<[u8]>) -> Self {
        Self { path, bytes }
    }
}

/// Completion of one requested file, including a real failure. No device state is implied.
#[derive(Clone, Debug)]
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

/// Owns one queue and I/O thread. Closing cancels active futures and joins all work.
pub struct ResourceRuntime {
    client: ResourceClient,
    changes: async_channel::Receiver<ResourceChanged>,
    worker: Option<JoinHandle<()>>,
}

pub(crate) trait Downloader: Send + 'static {
    fn fetch(&mut self, file: ResourceFile) -> BoxFuture<'static, anyhow::Result<Vec<u8>>>;
}

impl ResourceRuntime {
    pub fn start() -> anyhow::Result<Self> {
        Self::with_downloader(
            Cache::alongside(&std::env::current_exe()?)?,
            manifest()?.files.len(),
            crate::transport::HttpDownloader::default(),
        )
    }

    fn with_downloader(
        cache: Cache,
        capacity: usize,
        mut downloader: impl Downloader,
    ) -> anyhow::Result<Self> {
        let (sender, receiver) = async_channel::bounded(capacity.max(1));
        let (cancelled, cancellation) = async_channel::bounded(1);
        let (finished, changes) = async_channel::unbounded();
        let client = ResourceClient(Arc::new(Service {
            cache,
            state: Mutex::new(State::default()),
            publication: Mutex::new(()),
            sender,
            receiver,
            cancelled,
        }));
        let service = client.0.clone();
        let worker = std::thread::Builder::new()
            .name("resource-io".into())
            .spawn(move || {
                let run = || -> anyhow::Result<()> {
                    let executor = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    executor.block_on(async {
                        let requests = stream::unfold(service.clone(), |service| async move {
                            let first = service.receiver.recv().await.ok()?;
                            tokio::time::sleep(REQUEST_BATCH_WINDOW).await;
                            let batch = service.collect_batch(first);
                            Some((stream::iter(batch), service))
                        })
                        .flatten();
                        let work = requests
                            .map(|file| {
                                // Client/TLS initialization and credential lookup occur only on cache misses.
                                let service = service.clone();
                                let finished = finished.clone();
                                let download = &mut downloader;
                                let cached = service.cache.read(&file);
                                let cache_hit = matches!(cached, Ok(Some(_)));
                                let future = match cached {
                                    Ok(Some(bytes)) => {
                                        trace("cache-hit", &file.path, bytes.len());
                                        Box::pin(async { Ok(bytes) })
                                            as BoxFuture<'static, anyhow::Result<Vec<u8>>>
                                    }
                                    Ok(None) => download.fetch(file.clone()),
                                    Err(error) => Box::pin(async { Err(error) }),
                                };
                                async move {
                                    let result = future
                                        .await
                                        .and_then(|bytes| {
                                            service.ensure_running()?;
                                            if !cache_hit {
                                                service.cache.publish(&file, &bytes, || {
                                                    service.publication_permit()
                                                })?;
                                            }
                                            Ok(Arc::<[u8]>::from(bytes))
                                        })
                                        .map_err(|error| format!("{error:#}"));
                                    if let Some(changed) = service.complete(&file, result) {
                                        let _ = finished.send(changed).await;
                                    }
                                }
                            })
                            .buffer_unordered(MAX_CONCURRENT_DOWNLOADS)
                            .for_each(|()| async {});
                        tokio::select! {
                            _ = cancellation.recv() => {},
                            _ = work => {},
                        }
                    });
                    Ok(())
                };
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        service.fail_worker(format!("资源加载器无法启动：{error:#}"), &finished);
                    }
                    Err(panic) => {
                        service.fail_worker("资源加载器意外停止".into(), &finished);
                        std::panic::resume_unwind(panic);
                    }
                }
            })
            .context("Cannot start resource I/O worker")?;
        Ok(Self {
            client,
            changes,
            worker: Some(worker),
        })
    }

    pub fn client(&self) -> ResourceClient {
        self.client.clone()
    }

    /// One consumer bridges completions to its presentation system.
    pub fn changes(&self) -> async_channel::Receiver<ResourceChanged> {
        self.changes.clone()
    }

    /// Stop accepting requests and cancel asynchronous work without waiting for
    /// disk I/O or an in-progress cache commit. The owner must still call
    /// `shutdown` off the presentation thread and await its completion.
    pub fn begin_shutdown(&self) {
        self.client.0.begin_shutdown();
    }

    pub fn shutdown(&mut self) -> anyhow::Result<()> {
        self.client.0.stop();
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Resource I/O worker panicked"))?;
        }
        Ok(())
    }
}
impl Drop for ResourceRuntime {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("{error}");
        }
    }
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

    fn ensure_running(&self) -> anyhow::Result<()> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource state lock is poisoned"))?;
        anyhow::ensure!(
            state.lifecycle == Lifecycle::Running,
            "{}",
            state.failure.as_deref().unwrap_or("资源加载已取消")
        );
        Ok(())
    }

    fn begin_shutdown(&self) {
        {
            // This gate does not wait for publication, which can be in a
            // synchronous filesystem operation. Closing is an intent; stopped
            // is confirmed only after the commit permit has been released.
            if let Ok(mut state) = self.state.lock() {
                if state.lifecycle == Lifecycle::Running {
                    state.lifecycle = Lifecycle::Closing;
                }
                for value in state.files.values_mut() {
                    if matches!(value, FileState::Pending) {
                        *value = FileState::Failed("资源加载已取消".into());
                    }
                }
            }
        }
        self.sender.close();
        while self.receiver.try_recv().is_ok() {}
        self.cancelled.close();
    }

    fn stop(&self) {
        self.begin_shutdown();
        // A commit already holding the permit finishes before stopped is
        // published. New commits have been rejected by the closing gate.
        let _publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Ok(mut state) = self.state.lock() {
            state.lifecycle = Lifecycle::Stopped;
        }
    }

    fn publication_permit(&self) -> anyhow::Result<MutexGuard<'_, ()>> {
        let permit = self
            .publication
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource publication lock is poisoned"))?;
        self.ensure_running()?;
        Ok(permit)
    }

    fn read_or_request(&self, file: &ResourceFile) -> anyhow::Result<Option<Arc<[u8]>>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource state lock is poisoned"))?;
        anyhow::ensure!(
            state.lifecycle == Lifecycle::Running,
            "{}",
            state.failure.as_deref().unwrap_or("资源加载已取消")
        );
        match state.files.get(&file.path) {
            Some(FileState::Ready(bytes)) => return Ok(Some(bytes.clone())),
            Some(FileState::Failed(error)) => anyhow::bail!("{error}"),
            Some(FileState::Pending) => return Ok(None),
            None => {}
        }
        self.sender
            .try_send(file.clone())
            .context("资源加载队列不可用")?;
        trace("request", &file.path, "queued");
        state.files.insert(file.path.clone(), FileState::Pending);
        Ok(None)
    }

    fn complete(
        &self,
        file: &ResourceFile,
        result: Result<Arc<[u8]>, String>,
    ) -> Option<ResourceChanged> {
        let mut state = self.state.lock().ok()?;
        if state.lifecycle != Lifecycle::Running {
            return None;
        }
        if let Err(error) = &result {
            trace("failed", &file.path, error);
        }
        let changed = ResourceChanged {
            path: file.path.clone().into(),
            error: result.as_ref().err().map(|error| error.as_str().into()),
        };
        state.files.insert(
            file.path.clone(),
            match result {
                Ok(bytes) => FileState::Ready(bytes),
                Err(error) => FileState::Failed(error),
            },
        );
        Some(changed)
    }

    fn fail_worker(&self, error: String, finished: &async_channel::Sender<ResourceChanged>) {
        let _publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Ok(mut state) = self.state.lock() {
            state.lifecycle = Lifecycle::Stopped;
            state.failure = Some(error.clone());
            for (path, value) in &mut state.files {
                if matches!(value, FileState::Pending) {
                    *value = FileState::Failed(error.clone());
                    let _ = finished.try_send(ResourceChanged {
                        path: path.as_str().into(),
                        error: Some(error.as_str().into()),
                    });
                }
            }
        }
        self.sender.close();
        while self.receiver.try_recv().is_ok() {}
        self.cancelled.close();
    }
}

impl ResourceClient {
    /// Load either a catalog alias (including sprite selection) or an original file path.
    pub fn load(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let catalog = manifest()?;
        if let Some(&ix) = catalog.keys.get(key) {
            let entry = &catalog.entries[ix];
            let bytes = self
                .0
                .read_or_request(&entry.file)?
                .ok_or(ResourcePending)?;
            return crate::catalog::select_view(entry, &bytes).map(Some);
        }
        let Some(&ix) = catalog.paths.get(key) else {
            return Ok(None);
        };
        Ok(Some(
            self.0
                .read_or_request(&catalog.files[ix])?
                .ok_or(ResourcePending)?
                .to_vec(),
        ))
    }

    pub fn fonts(&self) -> anyhow::Result<Vec<ResourceFont>> {
        let mut ready = Vec::new();
        for font in &manifest()?.fonts {
            if let Some(bytes) = self.0.read_or_request(font)? {
                ready.push(ResourceFont {
                    path: font.path.clone(),
                    bytes,
                });
            }
        }
        Ok(ready)
    }

    pub fn available(&self, key: &str) -> anyhow::Result<bool> {
        let catalog = manifest()?;
        let file = catalog
            .keys
            .get(key)
            .map(|&ix| &catalog.entries[ix].file)
            .or_else(|| catalog.paths.get(key).map(|&ix| &catalog.files[ix]));
        file.map_or(Ok(true), |file| {
            self.0.read_or_request(file).map(|bytes| bytes.is_some())
        })
    }

    pub fn is_font(&self, path: &str) -> bool {
        manifest().is_ok_and(|catalog| catalog.fonts.iter().any(|font| font.path == path))
    }

    pub fn retry(&self) -> anyhow::Result<()> {
        let catalog = manifest()?;
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Resource state lock is poisoned"))?;
        anyhow::ensure!(
            state.lifecycle == Lifecycle::Running,
            "{}",
            state.failure.as_deref().unwrap_or("资源加载已取消")
        );
        for (path, value) in &mut state.files {
            if matches!(value, FileState::Failed(_)) {
                let file = &catalog.files[*catalog
                    .paths
                    .get(path)
                    .context("Requested resource is absent from catalog")?];
                self.0
                    .sender
                    .try_send(file.clone())
                    .context("资源加载队列不可用")?;
                *value = FileState::Pending;
            }
        }
        Ok(())
    }

    pub fn errors(&self) -> Vec<String> {
        match self.0.state.lock() {
            Ok(state) => state
                .failure
                .clone()
                .into_iter()
                .chain(state.files.iter().filter_map(|(path, value)| {
                    if let FileState::Failed(error) = value {
                        Some(format!("{path}：{error}"))
                    } else {
                        None
                    }
                }))
                .collect(),
            Err(_) => vec!["Resource state lock is poisoned".into()],
        }
    }
}

#[cfg(test)]
mod tests;

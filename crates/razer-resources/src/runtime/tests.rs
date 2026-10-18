use super::*;
use crate::cache::tests::{TemporaryDirectory, file};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

struct Request {
    file: ResourceFile,
    reply: async_channel::Sender<anyhow::Result<Vec<u8>>>,
}
struct ControlledDownload {
    requests: mpsc::Sender<Request>,
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}
struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
impl Downloader for ControlledDownload {
    fn fetch(&mut self, file: ResourceFile) -> BoxFuture<'static, anyhow::Result<Vec<u8>>> {
        let active = self.active.clone();
        let peak = self.peak.clone();
        let requests = self.requests.clone();
        Box::pin(async move {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            let _active = Active(active);
            let (reply, response) = async_channel::bounded(1);
            requests.send(Request { file, reply })?;
            response.recv().await?
        })
    }
}

struct Harness {
    runtime: ResourceRuntime,
    requests: mpsc::Receiver<Request>,
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}
impl Harness {
    fn new(root: &std::path::Path) -> Self {
        let (sender, requests) = mpsc::channel();
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let runtime = ResourceRuntime::with_downloader(
            Cache { root: root.into() },
            256,
            ControlledDownload {
                requests: sender,
                active: active.clone(),
                peak: peak.clone(),
            },
        )
        .unwrap();
        Self {
            runtime,
            requests,
            active,
            peak,
        }
    }
    fn request(&self) -> Request {
        self.requests
            .recv_timeout(Duration::from_secs(5))
            .expect("worker must start the explicitly queued request")
    }
    fn changed(&self) -> ResourceChanged {
        self.runtime.changes.recv_blocking().unwrap()
    }
}

#[test]
fn opened_shared_and_product_files_deduplicate_without_prefetching_other_pages() {
    let directory = TemporaryDirectory::new();
    let harness = Harness::new(&directory.0);
    let client = harness.runtime.client();
    let shared = file("assets/synapse/shared/settings/icon.svg", b"shared");
    let product = file("assets/synapse/products/182/icon.svg", b"product");
    let unopened = file("assets/synapse/products/214/icon.svg", b"unopened");
    for file in [&shared, &product, &shared] {
        assert!(client.0.read_or_request(file).unwrap().is_none());
    }
    let one = harness.request();
    let two = harness.request();
    assert_eq!(
        [one.file.path.as_str(), two.file.path.as_str()],
        [&shared.path, &product.path]
    );
    assert!(client.0.read_or_request(&shared).unwrap().is_none());
    assert!(harness.requests.try_recv().is_err());
    one.reply.send_blocking(Ok(b"shared".to_vec())).unwrap();
    two.reply.send_blocking(Ok(b"product".to_vec())).unwrap();
    assert!(harness.changed().error().is_none());
    assert!(harness.changed().error().is_none());
    assert_eq!(
        client.0.read_or_request(&shared).unwrap().unwrap().as_ref(),
        b"shared"
    );
    assert!(
        !client
            .0
            .state
            .lock()
            .unwrap()
            .files
            .contains_key(&unopened.path)
    );
    assert!(!client.0.cache.path(&unopened).unwrap().exists());
}

#[test]
fn shutdown_cancels_active_work_joins_it_and_a_new_owner_reuses_only_verified_cache() {
    let directory = TemporaryDirectory::new();
    let mut old = Harness::new(&directory.0);
    let old_client = old.runtime.client();
    let cached = file("assets/synapse/shared/help/icon.svg", b"cached");
    old_client.0.read_or_request(&cached).unwrap();
    old.request()
        .reply
        .send_blocking(Ok(b"cached".to_vec()))
        .unwrap();
    assert!(old.changed().error().is_none());
    let pending = file("assets/synapse/products/214/clip.gif", b"late");
    old_client.0.read_or_request(&pending).unwrap();
    let late = old.request();
    old.runtime.shutdown().unwrap();
    assert_eq!(
        old.active.load(Ordering::SeqCst),
        0,
        "shutdown must join cancellation and release in-flight downloads"
    );
    assert!(late.reply.send_blocking(Ok(b"late".to_vec())).is_err());
    assert!(!old_client.0.cache.path(&pending).unwrap().exists());
    assert!(old_client.0.read_or_request(&cached).is_err());
    assert!(old_client.retry().is_err());
    assert!(old.runtime.changes.is_closed());
    let fresh = Harness::new(&directory.0);
    let client = fresh.runtime.client();
    assert!(client.0.read_or_request(&cached).unwrap().is_none());
    assert_eq!(fresh.changed().path(), cached.path);
    assert!(
        fresh.requests.try_recv().is_err(),
        "cache hits must not initialize HTTP or look up credentials"
    );
    assert_eq!(
        client.0.read_or_request(&cached).unwrap().unwrap().as_ref(),
        b"cached"
    );
    assert!(old_client.0.read_or_request(&cached).is_err());
}

#[test]
fn shutdown_intent_returns_before_a_commit_finishes_then_the_owner_joins_all_work() {
    let directory = TemporaryDirectory::new();
    let harness = Harness::new(&directory.0);
    let client = harness.runtime.client();
    let pending = file("assets/synapse/shared/pending.svg", b"late");
    client.0.read_or_request(&pending).unwrap();
    let late = harness.request();

    let (committing, commit_started) = mpsc::channel();
    let (release, may_finish) = mpsc::channel();
    let committer = {
        let service = client.0.clone();
        std::thread::spawn(move || {
            let _permit = service.publication_permit().unwrap();
            committing.send(()).unwrap();
            may_finish.recv().unwrap();
        })
    };
    commit_started.recv_timeout(Duration::from_secs(5)).unwrap();

    let (transferred, owner_ready) = mpsc::channel();
    let runtime = harness.runtime;
    let closing = std::thread::spawn(move || {
        runtime.begin_shutdown();
        transferred.send(runtime).unwrap();
    });
    let owner = owner_ready.recv_timeout(Duration::from_secs(5));
    // Release the simulated slow commit before asserting, including on failure.
    // Shutdown intent must return while this permit is still held.
    release.send(()).unwrap();
    committer.join().unwrap();
    closing.join().unwrap();
    let mut runtime = owner.expect("Closing must not wait for synchronous cache publication");

    assert!(client.0.read_or_request(&pending).is_err());
    assert!(
        client
            .0
            .read_or_request(&file("assets/synapse/shared/new.svg", b"new"))
            .is_err()
    );
    assert!(client.retry().is_err());
    assert!(client.0.receiver.is_empty());
    runtime.begin_shutdown();
    runtime.shutdown().unwrap();
    assert_eq!(harness.active.load(Ordering::SeqCst), 0);
    assert!(late.reply.send_blocking(Ok(b"late".to_vec())).is_err());
    assert!(!client.0.cache.path(&pending).unwrap().exists());
    assert!(runtime.changes.is_closed());
    assert!(runtime.changes.try_recv().is_err());
}

#[test]
fn independent_owners_do_not_share_results_or_cancellation() {
    let directory = TemporaryDirectory::new();
    let mut first = Harness::new(&directory.0.join("first"));
    let second = Harness::new(&directory.0.join("second"));
    let file = file("assets/synapse/shared/about/logo.svg", b"logo");
    first.runtime.client().0.read_or_request(&file).unwrap();
    second.runtime.client().0.read_or_request(&file).unwrap();
    let late = first.request();
    let current = second.request();
    first.runtime.shutdown().unwrap();
    assert!(late.reply.send_blocking(Ok(b"logo".to_vec())).is_err());
    current.reply.send_blocking(Ok(b"logo".to_vec())).unwrap();
    assert_eq!(second.changed().path(), file.path);
    assert!(
        second
            .runtime
            .client()
            .0
            .read_or_request(&file)
            .unwrap()
            .is_some()
    );
    assert!(!first.runtime.client().0.cache.path(&file).unwrap().exists());
}

#[test]
fn failure_stays_explicit_and_retry_requeues_only_previously_requested_files() {
    let directory = TemporaryDirectory::new();
    let harness = Harness::new(&directory.0);
    let client = harness.runtime.client();
    let file = &manifest().unwrap().fonts[0];
    client.0.read_or_request(file).unwrap();
    harness
        .request()
        .reply
        .send_blocking(Err(anyhow::anyhow!("offline")))
        .unwrap();
    assert_eq!(harness.changed().error(), Some("offline"));
    assert!(client.0.read_or_request(file).is_err());
    assert_eq!(client.errors().len(), 1);
    client.retry().unwrap();
    assert!(client.errors().is_empty());
    assert_eq!(harness.request().file.path, file.path);
    assert_eq!(client.0.state.lock().unwrap().files.len(), 1);
}

#[test]
fn downloads_remain_bounded_and_cancel_drops_all_in_flight_futures() {
    let directory = TemporaryDirectory::new();
    let mut harness = Harness::new(&directory.0);
    for ix in 0..MAX_REQUEST_BATCH + 3 {
        harness
            .runtime
            .client()
            .0
            .read_or_request(&file(&format!("assets/synapse/shared/{ix}.svg"), b"icon"))
            .unwrap();
    }
    let first: Vec<_> = (0..MAX_CONCURRENT_DOWNLOADS)
        .map(|_| harness.request())
        .collect();
    assert_eq!(
        harness.active.load(Ordering::SeqCst),
        MAX_CONCURRENT_DOWNLOADS
    );
    assert_eq!(
        harness.peak.load(Ordering::SeqCst),
        MAX_CONCURRENT_DOWNLOADS
    );
    assert!(harness.requests.try_recv().is_err());
    harness.runtime.shutdown().unwrap();
    assert_eq!(harness.active.load(Ordering::SeqCst), 0);
    assert!(first.into_iter().all(|request| request.reply.is_closed()));
    assert!(harness.runtime.client().0.receiver.is_empty());
    assert!(std::fs::read_dir(&directory.0).unwrap().next().is_none());
}

#[test]
fn batch_limit_leaves_remaining_requests_queued() {
    let (sender, receiver) = async_channel::bounded(256);
    let (cancelled, _) = async_channel::bounded(1);
    let service = Service {
        cache: Cache {
            root: "unused-test-cache".into(),
        },
        state: Mutex::new(State::default()),
        publication: Mutex::new(()),
        sender,
        receiver,
        cancelled,
    };
    for ix in 0..MAX_REQUEST_BATCH + 3 {
        service
            .read_or_request(&file(&format!("assets/synapse/shared/{ix}.svg"), b"icon"))
            .unwrap();
    }
    assert_eq!(
        service
            .collect_batch(service.receiver.try_recv().unwrap())
            .len(),
        MAX_REQUEST_BATCH
    );
    assert_eq!(service.receiver.len(), 3);
}

#[test]
fn registered_product_alias_transitions_from_pending_to_verified_bytes() {
    let directory = TemporaryDirectory::new();
    let harness = Harness::new(&directory.0);
    let client = harness.runtime.client();
    const ALIAS: &str = "synapse/mouse-customize/8bd13c4f3a94a3834d2a05c6.svg";
    const SVG: &[u8] = include_bytes!("../../testdata/cycle-229.svg");
    assert!(crate::is_resource_pending(&client.load(ALIAS).unwrap_err()));
    assert!(crate::is_resource_pending(&client.load(ALIAS).unwrap_err()));
    let queued = harness.request();
    assert_eq!(
        queued.file.path,
        "assets/synapse/shared/products/8bd13c4f3a94a383-8bd13c4f3a94a3834d2a05c6.svg"
    );
    assert_eq!(queued.file.bytes, SVG.len() as u64);
    assert!(crate::cache::verified(&queued.file, SVG));
    queued.reply.send_blocking(Ok(SVG.to_vec())).unwrap();
    assert!(harness.changed().error().is_none());
    assert_eq!(client.load(ALIAS).unwrap().unwrap(), SVG);
    assert!(client.load("synapse/unknown-alias.svg").unwrap().is_none());
    assert!(harness.requests.try_recv().is_err());
}

#[test]
fn corrupt_transport_results_never_become_ready_or_cached() {
    let directory = TemporaryDirectory::new();
    let harness = Harness::new(&directory.0);
    let client = harness.runtime.client();
    let file = file("assets/synapse/shared/logo.svg", b"valid");
    client.0.read_or_request(&file).unwrap();
    harness
        .request()
        .reply
        .send_blocking(Ok(b"wrong".to_vec()))
        .unwrap();
    assert!(harness.changed().error().unwrap().contains("SHA-256"));
    assert!(client.0.read_or_request(&file).is_err());
    assert!(!client.0.cache.path(&file).unwrap().exists());
}

#[test]
fn worker_failure_closes_requests_and_preserves_the_failure_for_clients() {
    struct PanickingDownload;
    impl Downloader for PanickingDownload {
        fn fetch(&mut self, _: ResourceFile) -> BoxFuture<'static, anyhow::Result<Vec<u8>>> {
            panic!("controlled resource worker failure");
        }
    }
    let directory = TemporaryDirectory::new();
    let mut runtime = ResourceRuntime::with_downloader(
        Cache {
            root: directory.0.clone(),
        },
        2,
        PanickingDownload,
    )
    .unwrap();
    let client = runtime.client();
    let file = file("assets/synapse/shared/logo.svg", b"valid");
    assert!(client.0.read_or_request(&file).unwrap().is_none());
    let changed = runtime.changes.recv_blocking().unwrap();
    assert_eq!(changed.path(), file.path);
    assert_eq!(changed.error(), Some("资源加载器意外停止"));
    assert_eq!(
        client.0.read_or_request(&file).unwrap_err().to_string(),
        "资源加载器意外停止"
    );
    assert!(client.retry().is_err());
    assert!(client.0.receiver.is_empty());
    assert!(
        runtime
            .shutdown()
            .unwrap_err()
            .to_string()
            .contains("panicked")
    );
    assert!(runtime.changes.is_closed());
}

#[test]
fn stopping_after_staging_prevents_commit_and_cleans_only_the_pending_file() {
    let directory = TemporaryDirectory::new();
    let mut harness = Harness::new(&directory.0);
    let service = harness.runtime.client().0;
    let entry = file("assets/synapse/shared/logo.svg", b"valid");
    let path = service.cache.path(&entry).unwrap();
    let (staged, staging_complete) = mpsc::channel();
    let (resume, commit_allowed) = mpsc::channel();
    let publisher = {
        let service = service.clone();
        let entry = entry.clone();
        std::thread::spawn(move || {
            service.cache.publish(&entry, b"valid", || {
                staged.send(()).unwrap();
                commit_allowed.recv().unwrap();
                service.publication_permit()
            })
        })
    };
    staging_complete
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    assert!(!path.exists());
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1
    );
    harness.runtime.shutdown().unwrap();
    resume.send(()).unwrap();
    let error = publisher.join().unwrap().unwrap_err();
    assert!(format!("{error:#}").contains("资源加载已取消"));
    assert!(
        !path.exists(),
        "stopped sessions cannot publish staged bytes"
    );
    assert!(
        std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .next()
            .is_none()
    );
}

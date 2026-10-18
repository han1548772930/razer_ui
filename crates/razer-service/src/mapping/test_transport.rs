use super::*;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

pub(super) const TIMEOUT: Duration = Duration::from_secs(3);
pub(super) type Trace = Arc<Mutex<Vec<ServiceRequest>>>;

pub(super) struct Mock {
    handler: Box<dyn FnMut(ServiceRequest) -> anyhow::Result<Value> + Send>,
    trace: Trace,
    dropped: mpsc::Sender<()>,
    on_drop: Option<Box<dyn FnOnce() + Send>>,
}

impl Mock {
    pub(super) fn new(
        handler: impl FnMut(ServiceRequest) -> anyhow::Result<Value> + Send + 'static,
    ) -> (Self, Trace, mpsc::Receiver<()>) {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let (dropped, cleanup) = mpsc::channel();
        (
            Self {
                handler: Box::new(handler),
                trace: trace.clone(),
                dropped,
                on_drop: None,
            },
            trace,
            cleanup,
        )
    }

    pub(super) fn on_drop(mut self, callback: impl FnOnce() + Send + 'static) -> Self {
        self.on_drop = Some(Box::new(callback));
        self
    }
}

impl Transport for Mock {
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        self.trace.lock().unwrap().push(request.clone());
        (self.handler)(request)
    }

    fn is_stopped(&self) -> bool {
        false
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(callback) = self.on_drop.take() {
            callback();
        }
        self.dropped.send(()).unwrap();
    }
}

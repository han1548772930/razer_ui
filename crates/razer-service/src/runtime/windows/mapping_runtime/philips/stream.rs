//! The hardware HID stream is distinct from filter-driver input notifications.
//! Only actual protocol reports produce calibration samples.
use anyhow::{Result, ensure};
use razer_device::{
    mouse_philips::measurement::{Input, parse_report},
    receiver_pairing::HardwareEvents,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) struct Owner {
    stop: Arc<AtomicBool>,
    events: mpsc::Receiver<Result<(Input, u64), String>>,
    overflow: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<()>>>,
    cleanup_error: Option<String>,
}
impl Owner {
    pub fn start(product: u16, container: String, began: Instant) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let overflow = Arc::new(AtomicBool::new(false));
        let failed = overflow.clone();
        let (events, receiver) = mpsc::sync_channel(8192);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("razer-philips-input".into())
            .spawn(move || {
                let mut stream =
                    super::super::super::receiver_events::Events::new(product, &container);
                if let Err(error) = stream.begin() {
                    let cleanup = stream.finish();
                    let message =
                        format!("Philips hardware input: {error:#}; cleanup: {cleanup:?}");
                    let _ = ready.send(Err(message.clone()));
                    anyhow::bail!(message);
                }
                if ready.send(Ok(())).is_err() {
                    return stream.finish();
                }
                let result = (|| -> Result<()> {
                    while !signal.load(Ordering::Acquire) {
                        if let Some(bytes) = stream.receive(Duration::from_millis(2))? {
                            if let Some(input) = parse_report(&bytes)? {
                                if events
                                    .try_send(Ok((input, began.elapsed().as_millis() as u64)))
                                    .is_err()
                                {
                                    failed.store(true, Ordering::Release);
                                    anyhow::bail!("Philips hardware observation queue overflowed");
                                }
                            }
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = &result {
                    let _ = events.try_send(Err(format!("{error:#}")));
                }
                let cleanup = stream.finish();
                // Observation failures are delivered to the state machine; teardown
                // has an independent result so it cannot conceal a released handle.
                cleanup
            })?;
        match initialized.recv() {
            Ok(Ok(())) => Ok(Self {
                stop,
                events: receiver,
                overflow,
                worker: Some(worker),
                cleanup_error: None,
            }),
            result => {
                stop.store(true, Ordering::Release);
                let error = match result {
                    Ok(Err(error)) => error,
                    Err(error) => format!("Philips hardware initialization disconnected: {error}"),
                    Ok(Ok(())) => unreachable!(),
                };
                let cleanup = worker
                    .join()
                    .map_err(|_| anyhow::anyhow!("Philips input worker panicked"))
                    .and_then(|result| result);
                match cleanup {
                    Ok(()) => anyhow::bail!(error),
                    Err(cleanup) => anyhow::bail!("{error}; input cleanup: {cleanup:#}"),
                }
            }
        }
    }
    pub fn events(&self) -> Result<Vec<(Input, u64)>> {
        ensure!(
            !self.overflow.load(Ordering::Acquire),
            "Philips samples are incomplete after queue overflow"
        );
        let mut events = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(Ok(event)) => events.push(event),
                Ok(Err(error)) => anyhow::bail!(error),
                Err(mpsc::TryRecvError::Empty) => return Ok(events),
                Err(mpsc::TryRecvError::Disconnected) => {
                    anyhow::bail!("Philips hardware input stopped")
                }
            }
        }
    }
    pub fn stop(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let result = worker
                .join()
                .map_err(|_| anyhow::anyhow!("Philips input worker panicked"))
                .and_then(|value| value);
            if let Err(error) = result {
                self.cleanup_error = Some(format!("{error:#}"));
            }
        }
        if let Some(error) = &self.cleanup_error {
            anyhow::bail!("{error}");
        }
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("Philips input cleanup: {error:#}");
        }
    }
}

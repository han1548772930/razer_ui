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
        Self::start_with_events(began, move || {
            super::super::super::receiver_events::Events::new(product, &container)
        })
    }

    fn start_with_events<E: HardwareEvents + 'static>(
        began: Instant,
        create: impl FnOnce() -> E + Send + 'static,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let overflow = Arc::new(AtomicBool::new(false));
        let failed = overflow.clone();
        let (events, receiver) = mpsc::sync_channel(8192);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("razer-philips-input".into())
            .spawn(move || {
                let mut stream = create();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct Events {
        start_error: bool,
        cleanup_error: bool,
        cleanup_count: Arc<AtomicUsize>,
        requests: mpsc::Receiver<Result<Option<Vec<u8>>>>,
    }

    impl HardwareEvents for Events {
        fn begin(&mut self) -> Result<()> {
            ensure!(!self.start_error, "partial initialization failed");
            Ok(())
        }

        fn receive(&mut self, _: Duration) -> Result<Option<Vec<u8>>> {
            self.requests.recv().unwrap_or(Ok(None))
        }

        fn finish(&mut self) -> Result<()> {
            self.cleanup_count.fetch_add(1, Ordering::SeqCst);
            ensure!(!self.cleanup_error, "release failed");
            Ok(())
        }
    }

    #[test]
    fn partial_initialization_finishes_before_failure_returns() {
        for cleanup_error in [false, true] {
            let cleanup_count = Arc::new(AtomicUsize::new(0));
            let observed = cleanup_count.clone();
            let (_, requests) = mpsc::channel();
            let result = Owner::start_with_events(Instant::now(), move || Events {
                start_error: true,
                cleanup_error,
                cleanup_count,
                requests,
            });
            let error = match result {
                Ok(_) => panic!("startup must fail"),
                Err(error) => format!("{error:#}"),
            };
            assert!(error.contains("partial initialization failed"));
            assert_eq!(error.contains("release failed"), cleanup_error);
            assert_eq!(observed.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn initialization_disconnect_joins_the_panicked_worker() {
        let error = match Owner::start_with_events::<Events>(Instant::now(), || {
            panic!("stream construction failed")
        }) {
            Ok(_) => panic!("startup must fail"),
            Err(error) => format!("{error:#}"),
        };
        assert!(error.contains("initialization disconnected"));
        assert!(error.contains("worker panicked"));
    }

    #[test]
    fn stopping_joins_once_and_retains_cleanup_failure() {
        for cleanup_error in [false, true] {
            let cleanup_count = Arc::new(AtomicUsize::new(0));
            let observed = cleanup_count.clone();
            let (sender, requests) = mpsc::channel();
            let mut owner = Owner::start_with_events(Instant::now(), move || Events {
                start_error: false,
                cleanup_error,
                cleanup_count,
                requests,
            })
            .unwrap();
            owner.stop.store(true, Ordering::Release);
            drop(sender);
            for _ in 0..2 {
                let result = owner.stop();
                assert_eq!(result.is_err(), cleanup_error);
                if let Err(error) = result {
                    assert!(format!("{error:#}").contains("release failed"));
                }
            }
            assert_eq!(observed.load(Ordering::SeqCst), 1);
            drop(owner);
            assert_eq!(observed.load(Ordering::SeqCst), 1);
        }
    }
}

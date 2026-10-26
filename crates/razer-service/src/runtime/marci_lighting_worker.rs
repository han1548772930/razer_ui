use super::*;
use crate::runtime::{
    lighting_engine,
    lighting_engine_session::{EngineAdapter, Session},
    native,
};
use razer_device::{
    backend::{FeatureTransport, HidBackend, HidNode, ReportLengths},
    marci_lighting::{
        Control, Frame, LatestFrames, exit_effect_for_quick, led_configuration,
        manifest_exit_effect, submit_javascript_frame,
    },
};
use razer_hid::with_backend;
use std::time::Instant;

struct Checked<'a> {
    inner: &'a dyn FeatureTransport,
    validate: &'a dyn Fn() -> anyhow::Result<()>,
    validate_after: &'a dyn Fn() -> anyhow::Result<()>,
}
impl FeatureTransport for Checked<'_> {
    fn hid_node(&self) -> Option<&HidNode> {
        self.inner.hid_node()
    }
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        (self.validate)()?;
        self.inner.send_feature_at_collection_max(report)?;
        (self.validate_after)()
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        (self.validate)()?;
        let count = self.inner.get_feature_at_collection_max(report)?;
        (self.validate_after)()?;
        Ok(count)
    }
    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        (self.validate)()?;
        let count = self.inner.write_output_counted(report)?;
        (self.validate_after)()?;
        Ok(count)
    }
    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        self.inner.report_lengths()
    }
    fn metadata(&self) -> serde_json::Value {
        self.inner.metadata()
    }
}

pub(super) fn run(
    container: &str,
    generation: u64,
    effect: &QuickEffect,
    receiver: &mpsc::Receiver<Message>,
    canceled: &AtomicBool,
    status: &Mutex<Status>,
    response: mpsc::SyncSender<Result<Status, String>>,
) {
    let mut response = Some(response);
    let result = run_owned(
        container,
        generation,
        effect,
        receiver,
        canceled,
        status,
        &mut response,
    );
    if let Err(error) = &result {
        if !error.is::<Canceled>() {
            update(status, |s| s.errors.push(format!("{error:#}")));
        }
    }
    update(status, |s| s.stopped = true);
    if let Some(response) = response {
        let _ = response.send(
            result
                .map(|_| snapshot(status))
                .map_err(|e| format!("{e:#}"))
                .and_then(|s| s),
        );
    }
}

#[derive(Debug)]
struct Canceled;
impl std::fmt::Display for Canceled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Marci owner canceled")
    }
}
impl std::error::Error for Canceled {}
fn check(canceled: &AtomicBool) -> anyhow::Result<()> {
    if canceled.load(Ordering::Acquire) {
        return Err(Canceled.into());
    }
    Ok(())
}

fn run_owned(
    container: &str,
    generation: u64,
    effect: &QuickEffect,
    receiver: &mpsc::Receiver<Message>,
    canceled: &AtomicBool,
    status: &Mutex<Status>,
    response: &mut Option<mpsc::SyncSender<Result<Status, String>>>,
) -> anyhow::Result<()> {
    check(canceled)?;
    let channels = native::marci_channels(container)?;
    let validate_identity = || channels.validate(container);
    validate_identity()?;
    let output = with_backend(|backend| backend.open(&channels.output))?;
    update(status, |s| s.transports_released = false);
    let control_transport = match with_backend(|backend| backend.open(&channels.control)) {
        Ok(transport) => transport,
        Err(error) => {
            drop(output);
            update(status, |s| s.transports_released = true);
            return Err(error);
        }
    };
    let clock = Instant::now();
    run_connected(
        Connection {
            output,
            control_transport,
            validate_identity: &validate_identity,
        },
        Input {
            generation,
            effect,
            receiver,
            canceled,
            status,
            response,
        },
        lighting_engine::Engine::open,
        &thread::sleep,
        &|| clock.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
    )
}

struct Connection<'a> {
    output: Box<dyn FeatureTransport>,
    control_transport: Box<dyn FeatureTransport>,
    validate_identity: &'a dyn Fn() -> anyhow::Result<()>,
}
struct Input<'a> {
    generation: u64,
    effect: &'a QuickEffect,
    receiver: &'a mpsc::Receiver<Message>,
    canceled: &'a AtomicBool,
    status: &'a Mutex<Status>,
    response: &'a mut Option<mpsc::SyncSender<Result<Status, String>>>,
}

fn run_connected<E: EngineAdapter>(
    connection: Connection<'_>,
    input: Input<'_>,
    open_engine: impl FnOnce() -> anyhow::Result<E>,
    sleep: &impl Fn(Duration),
    now_ms: &impl Fn() -> u64,
) -> anyhow::Result<()> {
    let Connection {
        output,
        control_transport,
        validate_identity,
    } = connection;
    let Input {
        generation,
        effect,
        receiver,
        canceled,
        status,
        response,
    } = input;
    update(status, |s| s.transports_released = false);
    let mut control = Control::default();
    let mut session: Option<Session<E>> = None;
    let mut touched = false;
    let mut exit_target = manifest_exit_effect();
    let running = || {
        check(canceled)?;
        validate_identity()
    };
    let result = (|| -> anyhow::Result<()> {
        let checked_control = Checked {
            inner: control_transport.as_ref(),
            validate: &running,
            validate_after: validate_identity,
        };
        let checked_output = Checked {
            inner: output.as_ref(),
            validate: &running,
            validate_after: validate_identity,
        };
        let wait = |duration| {
            running()?;
            sleep(duration);
            running()
        };
        running()?;
        touched = true;
        update(status, |s| s.exit_required = true);
        let mut probe_failure = None;
        for _ in 0..3 {
            match submit_javascript_frame(&checked_output, &Frame::connection_probe()) {
                Ok(receipt) => {
                    let complete = receipt.complete;
                    update(status, |s| s.connection_output = Some(receipt));
                    if complete {
                        probe_failure = None;
                        break;
                    }
                    probe_failure =
                        Some(anyhow::anyhow!("Marci Output connection probe was short"));
                }
                Err(error) => {
                    if error.is::<Canceled>() {
                        return Err(error);
                    }
                    probe_failure = Some(error);
                }
            }
        }
        if let Some(error) = probe_failure {
            return Err(error);
        }
        control.firmware(&checked_control, &mut |d| wait(d))?;
        // The source's CustomFrame effect returns without Feature I/O. No ACK
        // is invented for that branch. Mode is observed before a possible SET.
        if control.mode(&checked_control, 3, &mut |d| wait(d))? {
            update(status, |s| s.control_acknowledged += 1);
        }
        exit_target = exit_effect_for_quick(effect)?;
        update(status, |s| s.planned_exit_effect = exit_target.clone());
        session = Some(Session::new(open_engine()?));
        update(status, |s| s.renderer_released = false);
        let session = session.as_mut().unwrap();
        session.initialize_effect(
            &led_configuration(),
            &effect.engine_effect_for_geometry(&led_configuration())?,
        )?;
        update(status, |s| s.screen = session.engine.screen_status());
        let device_handle = session
            .device_handle
            .context("Marci renderer has no device handle")?;
        let mut latest = LatestFrames::new(generation, now_ms());
        let mut active_generation = generation;
        if let Some(response) = response.take() {
            let _ = response.send(snapshot(status));
        }
        loop {
            check(canceled)?;
            let now = now_ms();
            let timeout = Duration::from_millis(latest.until_due_ms(now).min(40));
            match receiver.recv_timeout(timeout) {
                Ok(Message::Apply {
                    generation,
                    effect,
                    reply,
                }) => {
                    let result = (|| -> anyhow::Result<()> {
                        check(canceled)?;
                        validate_effect(&effect)?;
                        exit_target = exit_effect_for_quick(&effect)?;
                        update(status, |s| s.planned_exit_effect = exit_target.clone());
                        latest.change_generation(generation)?;
                        session.update_effect(
                            &effect.engine_effect_for_geometry(&led_configuration())?,
                        )?;
                        active_generation = generation;
                        update(status, |s| s.generation = generation);
                        Ok(())
                    })();
                    let failed = result.is_err();
                    let message = result.as_ref().map(|_| ()).map_err(|e| format!("{e:#}"));
                    let _ = reply.send(message.and_then(|_| snapshot(status)));
                    if failed {
                        result?;
                    }
                }
                Ok(Message::Gates {
                    generation,
                    gates,
                    reply,
                }) => {
                    let result = (|| -> anyhow::Result<()> {
                        check(canceled)?;
                        ensure!(
                            generation == active_generation,
                            "Marci gate belongs to another generation"
                        );
                        session.suspend(!gates.forwards())?;
                        latest.set_gates(gates);
                        Ok(())
                    })();
                    let message = result.as_ref().map(|_| ()).map_err(|e| format!("{e:#}"));
                    let _ = reply.send(message.and_then(|_| snapshot(status)));
                    result?;
                }
                Ok(Message::Wake) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            check(canceled)?;
            let frames = session.engine.frames();
            update(status, |s| s.screen = session.engine.screen_status());
            for frame in frames? {
                latest.replace(active_generation, frame)?;
            }
            let now = now_ms();
            for frame in latest.due(now) {
                let frame = Frame::from_engine(&frame, device_handle)?;
                let receipt = submit_javascript_frame(&checked_output, &frame)?;
                let complete = receipt.complete;
                update(status, |s| {
                    s.frames_submitted += 1;
                    s.last_output = Some(receipt);
                });
                ensure!(
                    complete,
                    "Marci Output transfer was short; no device ACK exists on this channel"
                );
            }
        }
        latest.stop();
        Ok(())
    })();
    // No frame producer survives this point. Cleanup ignores the cancellation
    // token but still requires the retained physical instance before every I/O.
    if let Some(mut session) = session.take() {
        let errors = session.cleanup();
        update(status, |s| {
            s.screen = session.engine.screen_status();
            s.renderer_released = errors.is_empty();
            s.cleanup_errors.extend(errors);
        });
    } else {
        update(status, |s| s.renderer_released = true);
    }
    if touched {
        let deadline = Instant::now() + Duration::from_secs(90);
        let cleanup_guard = || {
            ensure!(
                Instant::now() < deadline,
                "Marci exit restoration timed out"
            );
            validate_identity()
        };
        let checked = Checked {
            inner: control_transport.as_ref(),
            validate: &cleanup_guard,
            validate_after: &cleanup_guard,
        };
        let wait = |duration| {
            cleanup_guard()?;
            sleep(duration);
            cleanup_guard()
        };
        match control.mode(&checked, 0, &mut |d| wait(d)) {
            Ok(written) => update(status, |s| {
                s.normal_mode_confirmed = true;
                s.control_acknowledged += u32::from(written);
            }),
            Err(error) => update(status, |s| {
                s.cleanup_errors
                    .push(format!("Marci normal mode: {error:#}"))
            }),
        }
        match control.exit_effect(&checked, &exit_target, &mut |d| wait(d)) {
            Ok(()) => update(status, |s| {
                s.exit_effect_confirmed = true;
                s.control_acknowledged += 1;
            }),
            Err(error) => update(status, |s| {
                s.cleanup_errors
                    .push(format!("Marci exit effect: {error:#}"))
            }),
        }
    }
    drop(control_transport);
    drop(output);
    update(status, |s| s.transports_released = true);
    result
}

#[cfg(test)]
#[path = "marci_lighting_worker_tests.rs"]
mod tests;

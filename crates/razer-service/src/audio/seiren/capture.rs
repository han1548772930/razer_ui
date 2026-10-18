//! Source EQ6007 and UI7405 capture observations; cancellation retains final drain.
use super::{Owner, Session, guarded, shutdown};
use anyhow::{Context as _, ensure};
use razer_device::{
    seiren_audio_capture::{CaptureFrame, CapturePoll},
    seiren_microphone::CaptureOptions,
};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::{
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

pub struct Request {
    owner: Owner,
    options: CaptureOptions,
}
impl Request {
    pub fn new(
        session_id: u64,
        product_id: u32,
        options: CaptureOptions,
        canceled: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        options.validate()?;
        Ok(Self {
            owner: Owner::new(session_id, product_id, canceled)?,
            options,
        })
    }
}

pub enum Update {
    Frame(CaptureFrame),
    Finished(Option<String>),
}

pub fn spawn(request: Request) -> anyhow::Result<(Session, Receiver<Update>)> {
    let (updates, received) = mpsc::channel();
    let canceled = request.owner.canceled.clone();
    let session = Session::spawn("razer-seiren-analyser", canceled, move || {
        let result = guarded(|| {
            if request.owner.is_canceled() {
                return Ok(());
            }
            let mut client = ServiceClient::spawn()?;
            run_with_dispatch(
                &request,
                &updates,
                |request| client.request(request),
                || thread::sleep(Duration::from_millis(16)),
            )
        });
        let _ = updates.send(Update::Finished(
            result.err().map(|error| format!("{error:#}")),
        ));
    })?;
    Ok((session, received))
}

fn decode(value: serde_json::Value, request: &Request) -> anyhow::Result<CapturePoll> {
    let poll: CapturePoll = serde_json::from_value(value)
        .context("Seiren audio capture service reply has an invalid shape")?;
    ensure!(
        poll.session_id == request.owner.session_id && poll.product_id == request.owner.product_id,
        "Seiren audio capture reply differs from its page owner"
    );
    ensure!(
        poll.frame.is_none() || poll.started,
        "Seiren audio samples arrived before capture initialization"
    );
    if let Some(frame) = &poll.frame {
        ensure!(
            frame.capture_sample_rate > 0,
            "Seiren audio capture rate is zero"
        );
        ensure!(
            frame.output_sample_rate.is_none_or(|rate| rate > 0),
            "Seiren default output sample rate is zero"
        );
        ensure!(
            frame.mono_pcm.iter().all(|sample| sample.is_finite())
                && frame
                    .microphone_levels
                    .iter()
                    .flatten()
                    .all(|level| level.is_finite())
                && frame
                    .headphone_levels
                    .iter()
                    .flatten()
                    .all(|level| level.is_finite()),
            "Seiren audio capture reply contains non-finite observations"
        );
    }
    Ok(poll)
}

fn run_with_dispatch(
    request: &Request,
    updates: &Sender<Update>,
    mut dispatch: impl FnMut(ServiceRequest) -> anyhow::Result<serde_json::Value>,
    mut wait: impl FnMut(),
) -> anyhow::Result<()> {
    let owner = &request.owner;
    // A failing or malformed Start can still have created a native capture.
    let mut dispatched = false;
    let result = guarded(|| {
        if owner.is_canceled() {
            return Ok(());
        }
        dispatched = true;
        let started = decode(
            dispatch(ServiceRequest::SeirenAudioCaptureStart {
                session_id: owner.session_id,
                product_id: owner.product_id,
                options: request.options.clone(),
            })?,
            request,
        )?;
        if let Some(error) = started.error {
            anyhow::bail!("Seiren audio capture initialization failed: {error}");
        }
        ensure!(
            !started.terminal || owner.is_canceled(),
            "Seiren audio capture ended during initialization"
        );
        if let Some(frame) = started.frame {
            if !owner.is_canceled() {
                updates
                    .send(Update::Frame(frame))
                    .context("Seiren audio page owner was released")?;
            }
        }
        loop {
            if owner.is_canceled() {
                break;
            }
            wait();
            if owner.is_canceled() {
                break;
            }
            let reply = decode(
                dispatch(ServiceRequest::SeirenAudioCapturePoll {
                    session_id: owner.session_id,
                })?,
                request,
            )?;
            if reply.terminal {
                dispatched = false;
            }
            if let Some(error) = reply.error {
                anyhow::bail!("Seiren audio capture failed: {error}");
            }
            if !owner.is_canceled() {
                if let Some(frame) = reply.frame {
                    updates
                        .send(Update::Frame(frame))
                        .context("Seiren audio page owner was released")?;
                }
            }
            if reply.terminal {
                ensure!(
                    owner.is_canceled(),
                    "Seiren audio capture stopped without page cancellation"
                );
                break;
            }
        }
        Ok(())
    });
    let stopped = if dispatched {
        guarded(|| {
            let reply = decode(
                dispatch(ServiceRequest::SeirenAudioCaptureStop {
                    session_id: owner.session_id,
                })?,
                request,
            )?;
            ensure!(
                reply.terminal,
                "Seiren audio capture Stop did not confirm cleanup"
            );
            // Recording owns its final PCM until the page's generation check.
            if let Some(frame) = reply.frame {
                let _ = updates.send(Update::Frame(frame));
            }
            if let Some(error) = reply.error {
                anyhow::bail!("Seiren audio capture cleanup failed: {error}");
            }
            Ok(())
        })
    } else {
        Ok(())
    };
    let shutdown = shutdown(dispatch);
    let errors = [result, stopped, shutdown]
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(())
}

#[cfg(test)]
mod tests;

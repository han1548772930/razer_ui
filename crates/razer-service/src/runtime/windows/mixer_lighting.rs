//! RzNative 0x368c0 selects four bound input peaks, left*100 truncated;
//! 0x36ca0 sends Feature7/64, command0f13, ten attempts with 10ms delays.
use super::{mixer_driver, mixer_endpoint, mixer_meter};
use anyhow::{Context as _, ensure};
use razer_device::{
    audio_mixer::{MixerChannel, MixerControl, MixerSession, MixerTarget, MixerValue},
    audio_mixer_local::lighting,
    backend::HidNode,
};
use razer_hid::with_backend;
use serde_json::Value;
use std::time::{Duration, Instant};

pub(super) fn sample(node: &HidNode, sources: [u8; 4]) -> anyhow::Result<Value> {
    ensure!(
        sources.iter().all(|s| *s <= 6),
        "Mixer fader input source outside enum"
    );
    let proof = mixer_driver::observe_node(node, 1342)?;
    let container = proof["device_container_id"]
        .as_str()
        .context("Mixer lighting container absent")?;
    let _apartment = mixer_meter::Apartment::open()?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(10),
            "Mixer lighting sample deadline exceeded"
        );
        let current = mixer_driver::observe_node(node, 1342)?;
        ensure!(
            current["device_container_id"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case(container)),
            "Mixer lighting container changed"
        );
        Ok(())
    };
    let mut raw = [None; 7];
    let device = with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
    let session = MixerSession::new(device.as_ref(), &validate)?;
    for source in sources {
        if raw[source as usize].is_some() {
            continue;
        }
        let peak = match source {
            0 | 4 | 5 => {
                let control = match source {
                    0 => MixerControl::MicrophonePeak,
                    4 => MixerControl::ConsolePeak,
                    _ => MixerControl::LineInPeak,
                };
                let MixerValue::Peak { left, .. } = session.read(&MixerTarget {
                    control,
                    channel: MixerChannel::Both,
                    band: None,
                })?
                else {
                    anyhow::bail!("Mixer input peak reply type invalid")
                };
                left
            }
            _ => mixer_endpoint::source_peak(
                container,
                0,
                match source {
                    1 => 3,
                    2 => 1,
                    3 => 7,
                    6 => 16,
                    _ => unreachable!(),
                },
            )?,
        };
        ensure!(
            peak.is_finite() && (0. ..=1.).contains(&peak),
            "Mixer input peak outside source range"
        );
        raw[source as usize] = Some(peak);
    }
    serde_json::to_value(lighting::submit_sample(
        device.as_ref(),
        sources,
        raw,
        container.to_owned(),
        &validate,
    )?)
    .map_err(Into::into)
}

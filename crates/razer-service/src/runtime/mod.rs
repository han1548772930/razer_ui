//! Headless service composition. Protocols and clients are owned by razer-ipc.
use razer_ipc::{FRAME_PREFIX, RequestEnvelope, ResponseEnvelope, ServiceRequest, read_frame};
use std::io::Write;
#[path = "camera_controller.rs"]
mod camera_controller;
#[path = "controller_calibration.rs"]
mod controller_calibration;
#[path = "controller_initialization.rs"]
mod controller_initialization;
#[path = "controller_lighting.rs"]
mod controller_lighting;
#[path = "controller_sleep.rs"]
mod controller_sleep;
mod controller_socd;
#[path = "controller_triggers.rs"]
mod controller_triggers;
mod feature_submission;
mod hid_broker;
#[path = "keyboard_submission_controller.rs"]
mod keyboard_submission_controller;
#[path = "keyboard_wristrest_controller.rs"]
mod keyboard_wristrest_controller;
mod leviathan_events;
#[cfg(windows)]
mod lighting_engine;
mod lighting_engine_session;
mod marci_lighting;
#[path = "mouse_indicator_led_controller.rs"]
mod mouse_indicator_led_controller;
mod mouse_philips_controller;
#[path = "mouse_runtime_artwork_controller.rs"]
mod mouse_runtime_artwork_controller;
mod mouse_tracking_controller;
#[cfg(windows)]
#[path = "windows/native.rs"]
mod native;
#[path = "portable.rs"]
mod portable;
mod receiver_settings;
#[path = "seiren_audio_capture_controller.rs"]
mod seiren_audio_capture_controller;
mod session;
mod shutdown;
#[path = "thx_controller.rs"]
mod thx_controller;

#[derive(Clone, Copy)]
enum KeyboardRead<'a> {
    Metadata,
    Serial,
    ProfileInventory,
    OnboardProfiles(&'a razer_device::keyboard_onboard_profiles::ReadOptions),
}

impl KeyboardRead<'_> {
    fn deadline(self) -> std::time::Duration {
        std::time::Duration::from_secs(match self {
            Self::ProfileInventory | Self::OnboardProfiles(_) => 120,
            Self::Metadata | Self::Serial => 20,
        })
    }
}

pub(crate) fn mapping_client() -> anyhow::Result<crate::mapping::owner::Client> {
    #[cfg(windows)]
    {
        native::mapping_client()
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Native mapping execution is unavailable on this platform")
    }
}

fn write_response(response: &ResponseEnvelope) -> anyhow::Result<()> {
    let json = serde_json::to_string(response)?;
    let output = std::io::stdout();
    let mut output = output.lock();
    writeln!(output, "{FRAME_PREFIX}{json}")?;
    output.flush()?;
    Ok(())
}

/// Legacy stdio hosts connect to the same broker as normal service clients.
/// This entrypoint must be selected before initializing GPUI.
pub fn run_worker() -> i32 {
    let result = (|| -> anyhow::Result<()> {
        let executable = std::env::current_exe()?;
        let endpoint = razer_ipc::hid_broker::BrokerEndpoint::for_installation(&executable)?;
        let mut client = razer_ipc::hid_broker::BrokerClient::connect(
            &endpoint,
            None,
            std::time::Duration::from_secs(225),
        )?;
        let operation = (|| -> anyhow::Result<()> {
            let input = std::io::stdin();
            let mut input = input.lock();
            while let Some(bytes) = read_frame(&mut input)? {
                let envelope: RequestEnvelope = serde_json::from_slice(&bytes)?;
                let closing = matches!(envelope.request, ServiceRequest::Shutdown);
                let response =
                    client.request(envelope, &|| false, std::time::Duration::from_secs(305))?;
                write_response(&response)?;
                if closing {
                    break;
                }
            }
            Ok(())
        })();
        let cleanup = client
            .close_session()
            .and_then(|receipt| receipt.validate());
        match (operation, cleanup) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => {
                Err(error.context(format!("Service session cleanup also failed: {cleanup:#}")))
            }
        }
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Service connection: {error:#}");
            2
        }
    }
}

/// Dedicated shared process entrypoint; no UI or official business DLL starts.
pub fn run_hid_broker() -> i32 {
    match hid_broker::run() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("HID broker: {error:#}");
            2
        }
    }
}

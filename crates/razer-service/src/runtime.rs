//! Worker dispatch only. Wire contract and client are owned by razer-ipc.
use razer_ipc::{FRAME_PREFIX, RequestEnvelope, ResponseEnvelope, ServiceRequest, read_frame};
use std::{io::Write, time::Duration};
#[cfg(windows)]
#[path = "runtime_native.rs"]
mod native;
#[path = "runtime_portable.rs"]
mod portable;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(4);

/// `main` must dispatch `--service-worker` here before initializing GPUI, then
/// exit with this status. Never invoke this function from a UI callback.
pub fn run_worker() -> i32 {
    #[cfg(windows)]
    let mut runtime = native::NativeRuntime::new();
    let mut portable = portable::PortableRuntime::default();
    let mut host_storage = razer_storage::host::HostStorage::default();
    let input = std::io::stdin();
    let mut input = input.lock();
    loop {
        let envelope = match read_frame(&mut input) {
            Ok(Some(line)) => match serde_json::from_slice::<RequestEnvelope>(&line) {
                Ok(envelope) => envelope,
                Err(_) => return 2,
            },
            Ok(None) => return 0,
            Err(_) => return 2,
        };
        let shutdown = matches!(envelope.request, ServiceRequest::Shutdown);
        let result = match envelope.request {
            ServiceRequest::HostStorageView { view } => {
                host_storage.register_view(view);
                Ok(serde_json::json!({"registered": true}))
            }
            ServiceRequest::HostStorageClose { view_id } => {
                host_storage.close_view(view_id);
                Ok(serde_json::json!({"closed": true}))
            }
            ServiceRequest::HostStorageCall { call } => host_storage
                .call(call)
                .and_then(|result| serde_json::to_value(result).map_err(Into::into)),
            ServiceRequest::HostStorageEvents { view_id } => host_storage
                .drain_events(view_id)
                .and_then(|events| serde_json::to_value(events).map_err(Into::into)),
            request @ (ServiceRequest::AudioDevices
            | ServiceRequest::AudioEndpoints { .. }
            | ServiceRequest::HidNodes
            | ServiceRequest::HidNodeRead { .. }
            | ServiceRequest::HidNodeWrite { .. }
            | ServiceRequest::HidNodeMixerRead { .. }
            | ServiceRequest::HidNodeMixerWrite { .. }
            | ServiceRequest::HidNodeMixerRouteRead { .. }
            | ServiceRequest::HidNodeMixerRouteWrite { .. }
            | ServiceRequest::HidNodeMixerRestartStreams { .. }
            | ServiceRequest::HidNodeReceiverStatus { .. }) => portable.request(request),
            request => {
                #[cfg(windows)]
                {
                    runtime.request(request)
                }
                #[cfg(not(windows))]
                {
                    match request {
                        ServiceRequest::Shutdown => Ok(serde_json::json!({"stopped":true})),
                        _ => Err(anyhow::anyhow!(
                            "此操作需要尚未移植的平台服务或设备身份适配"
                        )),
                    }
                }
            }
        };
        #[cfg(windows)]
        let fatal = runtime.is_poisoned();
        #[cfg(not(windows))]
        let fatal = false;
        let response = match result {
            Ok(data) => ResponseEnvelope {
                id: envelope.id,
                data: Some(data),
                error: None,
                fatal,
            },
            Err(error) => ResponseEnvelope {
                id: envelope.id,
                data: None,
                error: Some(format!("{error:#}")),
                fatal,
            },
        };
        let Ok(json) = serde_json::to_string(&response) else {
            return 2;
        };
        let output = std::io::stdout();
        let mut output = output.lock();
        if writeln!(output, "{FRAME_PREFIX}{json}")
            .and_then(|_| output.flush())
            .is_err()
        {
            return 2;
        }
        if fatal || shutdown {
            return if fatal { 3 } else { 0 };
        }
    }
}

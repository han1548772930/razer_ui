use super::{DpiReport, MouseCurrentDpiRequested, ObservedDevice, Ordering, direct};
use anyhow::ensure;
use razer_device::mouse_current_dpi::ble::initialization;
use razer_ipc::mouse_current_dpi_ble::{Context as BleContext, Outcome, Profiles};
use razer_ipc::{ServiceClient, ServiceRequest};

#[derive(Clone)]
pub(super) struct BlePlan {
    pub connection_owner: String,
    pub generation: u64,
    pub cached: Option<initialization::ObservedSlots>,
    pub local: razer_model::model::Device,
}

// Current226 factory70035.KK is the local guest owner used by tn/ON_INIT.
const LOCAL_GUEST_OBM_OWNER: &str =
    "5ed8944a85a9763fd315852f448cb7de36c5e928e13b3be427f98f7dc455f141";

fn local_profiles_for_serial(
    device: &razer_model::model::Device,
    serial: &str,
) -> anyhow::Result<Option<initialization::Request>> {
    razer_storage::host_local_profile::ble226::from_device(device, serial, LOCAL_GUEST_OBM_OWNER)
}

pub(in crate::shell) struct Completion {
    result: Result<DpiReport, String>,
    local_profile: Option<Result<Option<razer_storage::host_local_profile::LocalProfiles>, String>>,
    ble: Option<Outcome>,
    ble_catalog: Option<razer_storage::host_local_profile::ble226::Catalog>,
    ble_serial: Option<String>,
    shutdown_error: Option<String>,
    termination_error: Option<String>,
    destruction_error: Option<String>,
}

impl Completion {
    pub(super) fn ble_catalog(
        &self,
    ) -> Option<(&str, &razer_storage::host_local_profile::ble226::Catalog)> {
        Some((self.ble_serial.as_deref()?, self.ble_catalog.as_ref()?))
    }
    pub(super) fn observed_slots(&self) -> Option<&initialization::ObservedSlots> {
        let ble = self.ble.as_ref()?;
        (self.result.is_ok() && ble.error.is_none() && self.cleanup_error().is_none())
            .then_some(ble.observed_slots.as_ref())
            .flatten()
    }

    pub(super) fn initialization_error(&self) -> Option<&str> {
        self.ble.as_ref()?.initialization.as_ref()?.error.as_deref()
    }
    pub(super) fn local_profile(
        &self,
    ) -> Option<Result<Option<razer_storage::host_local_profile::LocalProfiles>, String>> {
        self.local_profile.clone()
    }
    #[cfg(test)]
    pub(super) fn report(&self) -> Option<&DpiReport> {
        self.result.as_ref().ok()
    }

    pub(super) fn result(&self) -> Result<DpiReport, String> {
        let result = self.result.clone();
        if let Some(cleanup) = self.cleanup_error() {
            return Err(match result {
                Err(error) => format!("{error}；{cleanup}"),
                Ok(_) => cleanup,
            });
        }
        result.map(|mut report| {
            if let Some(serial) = &self.ble_serial {
                report.serial_number = Some(serial.clone());
            }
            report
        })
    }

    pub(super) fn actual_report(&self) -> Option<DpiReport> {
        let mut report = self
            .result
            .as_ref()
            .ok()
            .or_else(|| self.ble.as_ref()?.report.as_ref())?
            .clone();
        if let Some(serial) = &self.ble_serial {
            report.serial_number = Some(serial.clone());
        }
        Some(report)
    }

    pub(super) fn cleanup_error(&self) -> Option<String> {
        let errors = [
            self.ble
                .as_ref()
                .and_then(|outcome| outcome.cleanup_error.as_ref())
                .map(|error| format!("GATT 会话释放失败：{error}")),
            self.shutdown_error
                .as_ref()
                .map(|error| format!("关闭失败：{error}")),
            self.termination_error
                .as_ref()
                .map(|error| format!("进程回收失败：{error}")),
            self.destruction_error
                .as_ref()
                .map(|error| format!("通信所有者释放失败：{error}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        (!errors.is_empty()).then(|| errors.join("；"))
    }

    pub(super) fn message(&self, request: &MouseCurrentDpiRequested) -> String {
        let result = self.result();
        let mut message = match &result {
            Err(error) => format!("DPI 设备状态尚未确认：{error}"),
            Ok(_)
                if request
                    .change()
                    .is_some_and(|change| change.is_identity_read()) =>
            {
                "已读取设备序列号，正在载入对应配置。".into()
            }
            Ok(report) if request.is_read() => match report.active {
                Some(pair) if pair.x == pair.y => {
                    format!("设备当前 DPI：{}", pair.x)
                }
                Some(pair) => format!("设备当前 DPI：X {} / Y {}", pair.x, pair.y),
                None if report.read_errors.active().is_some() => {
                    format!(
                        "设备当前 DPI 读取失败：{}",
                        report.read_errors.active().unwrap()
                    )
                }
                None => "已读取设备 DPI 档位。".into(),
            },
            Ok(report) if !report.verified => {
                let errors = [report.write_errors.stages(), report.write_errors.active()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("；");
                format!("DPI 设备写入尚未确认：{errors}")
            }
            Ok(_) => "DPI 设备写入已应答。".into(),
        };
        if let Ok(report) = &result {
            if let Some(error) = report.read_errors.stages() {
                message.push_str(&format!("；DPI 档位读取失败：{error}"));
            }
        }
        message
    }
}

trait Transport {
    fn execute(&mut self) -> anyhow::Result<DpiReport>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn finish_termination(&mut self) -> anyhow::Result<()>;
    fn outcome(&self) -> Option<Outcome> {
        None
    }
    fn catalog(&self) -> Option<razer_storage::host_local_profile::ble226::Catalog> {
        None
    }
    fn observed_serial(&self) -> Option<String> {
        None
    }
}

struct NativeTransport {
    client: ServiceClient,
    route: ObservedDevice,
    request: MouseCurrentDpiRequested,
    ble_plan: Option<BlePlan>,
    ble_outcome: Option<Outcome>,
    ble_catalog: Option<razer_storage::host_local_profile::ble226::Catalog>,
    ble_serial: Option<String>,
}

impl Transport for NativeTransport {
    fn execute(&mut self) -> anyhow::Result<DpiReport> {
        let context = if let Some(plan) = &self.ble_plan {
            if let Some(slots) = &plan.cached {
                Some(BleContext {
                    connection_owner: plan.connection_owner.clone(),
                    generation: plan.generation,
                    profiles: Profiles::Cached {
                        slots: slots.clone(),
                    },
                })
            } else {
                let identity = direct::mouse_current_dpi::request_with_context(
                    &mut self.client,
                    &self.route,
                    None,
                    Some(BleContext {
                        connection_owner: plan.connection_owner.clone(),
                        generation: plan.generation,
                        profiles: Profiles::ReadIdentity,
                    }),
                )?;
                self.ble_outcome = Some(identity.clone());
                let serial = identity.identity_result().map_err(anyhow::Error::msg)?;
                self.ble_serial = Some(serial.to_owned());
                let request = match local_profiles_for_serial(&plan.local, serial)? {
                    Some(request) => Some(request),
                    None => {
                        let root = razer_platform::native_paths::host_local_storage_directory()?;
                        let catalog =
                            razer_storage::host_local_profile::ble226::read(&root, serial)?;
                        let request = catalog
                            .as_ref()
                            .map(|catalog| catalog.initialization_request(LOCAL_GUEST_OBM_OWNER))
                            .transpose()?
                            .flatten();
                        self.ble_catalog = catalog;
                        request
                    }
                };
                request.map(|request| BleContext {
                    connection_owner: plan.connection_owner.clone(),
                    generation: plan.generation,
                    profiles: Profiles::Initialize { request },
                })
            }
        } else {
            None
        };
        let outcome = direct::mouse_current_dpi::request_with_context(
            &mut self.client,
            &self.route,
            self.request.change().cloned(),
            context,
        )?;
        let result = outcome.result().map_err(anyhow::Error::msg);
        self.ble_outcome = Some(outcome);
        result
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        if self.client.is_stopped() {
            ensure!(
                self.client.shutdown_confirmation().is_some(),
                "DPI 通信进程已终止，未取得关闭回执"
            );
            return Ok(());
        }
        self.client.request(ServiceRequest::Shutdown).map(|_| ())
    }
    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
    fn outcome(&self) -> Option<Outcome> {
        self.ble_outcome.clone()
    }
    fn catalog(&self) -> Option<razer_storage::host_local_profile::ble226::Catalog> {
        self.ble_catalog.clone()
    }
    fn observed_serial(&self) -> Option<String> {
        self.ble_serial.clone()
    }
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| "DPI 通信所有者发生异常".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

fn check(valid: &std::sync::atomic::AtomicBool) -> anyhow::Result<()> {
    ensure!(
        valid.load(Ordering::Acquire),
        "DPI 请求范围已取消；已发送的设置仍可能生效"
    );
    Ok(())
}

pub(super) fn run(
    route: ObservedDevice,
    request: MouseCurrentDpiRequested,
    serial: String,
    ble_plan: Option<BlePlan>,
) -> Completion {
    let valid = request.guard();
    let product_id = route.product_id();
    let read_profile =
        request.is_read() && razer_storage::host_local_profile::supported(product_id);
    let identity_read = request
        .change()
        .is_some_and(|change| change.is_identity_read());
    let mut completion = run_with(&valid, || {
        let mut client = ServiceClient::spawn()?;
        client.set_request_guard(valid.clone());
        Ok(NativeTransport {
            client,
            route,
            request,
            ble_plan,
            ble_outcome: None,
            ble_catalog: None,
            ble_serial: None,
        })
    });
    if read_profile {
        observe_local_profile(&mut completion, &valid, identity_read, &serial, |serial| {
            let root = razer_platform::native_paths::host_local_storage_directory()?;
            razer_storage::host_local_profile::read(&root, product_id, serial)
        });
    }
    completion
}
fn observe_local_profile(
    completion: &mut Completion,
    valid: &std::sync::atomic::AtomicBool,
    identity_read: bool,
    previous_serial: &str,
    read: impl FnOnce(&str) -> anyhow::Result<Option<razer_storage::host_local_profile::LocalProfiles>>,
) {
    completion.local_profile = Some(guarded(|| {
        check(valid)?;
        let observed_serial = completion
            .result
            .as_ref()
            .ok()
            .and_then(|r| r.serial_number.as_deref());
        ensure!(
            !identity_read || observed_serial.is_some(),
            "local profile identity was not observed on this connection"
        );
        let profile = read(observed_serial.unwrap_or(previous_serial))?;
        check(valid)?;
        Ok(profile)
    }));
}

fn run_with<T: Transport>(
    valid: &std::sync::atomic::AtomicBool,
    factory: impl FnOnce() -> anyhow::Result<T>,
) -> Completion {
    let mut transport = match guarded(|| {
        check(valid)?;
        factory()
    }) {
        Ok(transport) => transport,
        Err(error) => {
            return Completion {
                result: Err(error),
                local_profile: None,
                ble: None,
                ble_catalog: None,
                ble_serial: None,
                shutdown_error: None,
                termination_error: None,
                destruction_error: None,
            };
        }
    };
    let result = guarded(|| {
        check(valid)?;
        let report = transport.execute()?;
        check(valid)?;
        Ok(report)
    });
    let ble = transport.outcome();
    let ble_catalog = transport.catalog();
    let ble_serial = transport.observed_serial();
    let shutdown_error = guarded(|| transport.shutdown()).err();
    let termination_error = guarded(|| transport.finish_termination()).err();
    let destruction_error = guarded(|| {
        drop(transport);
        Ok(())
    })
    .err();
    let result = result.and_then(|report| {
        guarded(|| {
            check(valid)?;
            Ok(report)
        })
    });
    Completion {
        result,
        local_profile: None,
        ble,
        ble_catalog,
        ble_serial,
        shutdown_error,
        termination_error,
        destruction_error,
    }
}

#[cfg(test)]
#[path = "mouse_current_dpi_worker_tests.rs"]
mod tests;

use super::{ObservedDevice, Ordering, PollingField, Request, direct};
use anyhow::{Context as _, ensure};
use razer_device::{device_reads::DeviceReadValue, device_writes::DeviceWriteSetting};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::sync::atomic::AtomicBool;

pub(super) struct Completion {
    result: Result<u32, String>,
    shutdown_error: Option<String>,
    termination_error: Option<String>,
    destruction_error: Option<String>,
}

impl Completion {
    pub(super) fn observed(&self) -> Option<u32> {
        self.result.as_ref().ok().copied()
    }

    pub(super) fn cleanup_error(&self) -> Option<String> {
        let errors = [
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

    pub(super) fn message(&self) -> String {
        let mut message = match &self.result {
            Ok(observed) => format!("设备已回读确认回报率：{observed} Hz。"),
            Err(error) => format!("回报率未确认：{error}。请重新读取设备状态。"),
        };
        if let Some(error) = self.cleanup_error() {
            message.push_str(&format!(" 通信清理：{error}"));
        }
        message
    }

    fn failed(error: String) -> Self {
        Self {
            result: Err(error),
            shutdown_error: None,
            termination_error: None,
            destruction_error: None,
        }
    }
}

trait Transport {
    fn execute(&mut self) -> anyhow::Result<u32>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn finish_termination(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    client: ServiceClient,
    route: ObservedDevice,
    request: Request,
}

impl Transport for NativeTransport {
    fn execute(&mut self) -> anyhow::Result<u32> {
        let observed = if let Some(change) = &self.request.change {
            let report =
                direct::mouse_current_polling::request(&mut self.client, &self.route, change)?;
            match &report.runtime().observed {
                Some(DeviceReadValue::Polling { hz }) => *hz,
                _ => anyhow::bail!("回报率读回没有实际任务频率"),
            }
        } else if self.request.field == PollingField::Wireless {
            direct::mouse_receiver_polling::write(
                &mut self.client,
                &self.route,
                self.request.device_hz,
            )?
            .observed_hz
        } else {
            let cap = razer_device::device_reads::capability(self.route.product_id())
                .context("产品回报率读取能力尚未核实")?;
            match direct::resolve(&mut self.client, &self.route, cap)?.write(
                &mut self.client,
                DeviceWriteSetting::Polling {
                    hz: self.request.device_hz,
                },
            )? {
                DeviceReadValue::Polling { hz } => hz,
                _ => anyhow::bail!("回报率读回没有实际频率"),
            }
        };
        ensure!(
            observed == self.request.device_hz,
            "回报率读回与实际任务频率不一致"
        );
        Ok(observed)
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.client.is_stopped(),
            "回报率通信进程已终止，未取得关闭回执"
        );
        self.client.request(ServiceRequest::Shutdown).map(|_| ())
    }

    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| "回报率通信所有者发生异常".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

fn check(valid: &AtomicBool) -> anyhow::Result<()> {
    ensure!(
        valid.load(Ordering::Acquire),
        "回报率请求范围已取消；已发送的设置仍可能生效"
    );
    Ok(())
}

pub(super) fn run(route: ObservedDevice, request: Request) -> Completion {
    let valid = request.valid.clone();
    run_with(&valid, || {
        let mut client = ServiceClient::spawn()?;
        client.set_request_guard(valid.clone());
        Ok(NativeTransport {
            client,
            route,
            request,
        })
    })
}

fn run_with<T: Transport>(
    valid: &AtomicBool,
    factory: impl FnOnce() -> anyhow::Result<T>,
) -> Completion {
    let mut transport = match guarded(|| {
        check(valid)?;
        factory()
    }) {
        Ok(transport) => transport,
        Err(error) => return Completion::failed(error),
    };
    let result = guarded(|| {
        check(valid)?;
        let observed = transport.execute()?;
        check(valid)?;
        Ok(observed)
    });
    let shutdown_error = guarded(|| transport.shutdown()).err();
    let termination_error = guarded(|| transport.finish_termination()).err();
    let destruction_error = guarded(|| {
        drop(transport);
        Ok(())
    })
    .err();
    let result = result.and_then(|observed| {
        guarded(|| {
            check(valid)?;
            Ok(observed)
        })
    });
    Completion {
        result,
        shutdown_error,
        termination_error,
        destruction_error,
    }
}

#[cfg(test)]
#[path = "mouse_polling_worker_tests.rs"]
mod tests;

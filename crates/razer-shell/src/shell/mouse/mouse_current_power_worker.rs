use super::{ObservedDevice, Ordering, Request, direct};
use anyhow::{Context as _, ensure};
use razer_device::{
    device_reads::DeviceReadValue, device_writes::DeviceWriteSetting, mouse_current_power::Report,
};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::sync::atomic::AtomicBool;

#[derive(Debug)]
enum Observation {
    Current(Report),
    Idle(u16),
}

pub(super) struct Completion {
    result: Result<Observation, String>,
    shutdown_error: Option<String>,
    termination_error: Option<String>,
    destruction_error: Option<String>,
}

impl Completion {
    pub(super) fn current_report(&self) -> Option<Report> {
        match self.result.as_ref().ok()? {
            Observation::Current(report) => Some(report.clone()),
            Observation::Idle(_) => None,
        }
    }

    pub(super) fn idle_seconds(&self) -> Option<u16> {
        match self.result.as_ref().ok()? {
            Observation::Idle(seconds) => Some(*seconds),
            Observation::Current(_) => None,
        }
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

    pub(super) fn message(&self, request: &Request, product_id: u32) -> String {
        let mut message = match (request, &self.result) {
            (Request::Idle { minutes, .. }, Ok(Observation::Idle(_))) => {
                format!("设备已回读确认休眠时间：{minutes} 分钟。")
            }
            (Request::Idle { .. }, Err(error)) => {
                format!("休眠设置未确认：{error}。当前选择已保留，请重新读取设备状态。")
            }
            (Request::Current(request), Ok(Observation::Current(_))) if request.is_read() => {
                "已读取设备省电设置。".into()
            }
            (Request::Current(request), Ok(Observation::Current(_)))
                if !request.requires_local_persistence(product_id) =>
            {
                "设备省电设置已确认。".into()
            }
            (Request::Current(_), Ok(Observation::Current(_))) => {
                "电源设置已保存到本机，设备设置已确认。".into()
            }
            (Request::Current(request), Err(error)) if request.is_read() => {
                format!("设备省电设置读取失败：{error}")
            }
            (Request::Current(request), Err(error))
                if !request.requires_local_persistence(product_id) =>
            {
                format!("设备省电设置写入尚未确认：{error}；当前编辑值保留。")
            }
            (Request::Current(_), Err(error)) => {
                format!("电源设置设备写入尚未确认：{error}；本地设置保留。")
            }
            _ => "设备电源应答类型与当前请求不一致。".into(),
        };
        if let Some(error) = self.cleanup_error() {
            message.push_str(&format!(" 通信清理：{error}"));
        }
        message
    }
}

trait Transport {
    fn execute(&mut self) -> anyhow::Result<Observation>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn finish_termination(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    client: ServiceClient,
    route: ObservedDevice,
    request: Request,
}

impl Transport for NativeTransport {
    fn execute(&mut self) -> anyhow::Result<Observation> {
        match &self.request {
            Request::Current(request) => {
                let route = direct::mouse_current_power::resolve(&mut self.client, &self.route)?;
                let change = request
                    .change()
                    .cloned()
                    .context("Power request has no change")?;
                route
                    .mouse_current_power(&mut self.client, change)
                    .map(Observation::Current)
            }
            Request::Idle { minutes, .. } => {
                let raw_time = u16::from(*minutes) * 60;
                let observed = if self.route.peer_product_id().is_some() {
                    direct::mouse_receiver_polling::write_idle(
                        &mut self.client,
                        &self.route,
                        raw_time,
                    )?
                } else {
                    let cap = razer_device::device_reads::capability(self.route.product_id())
                        .context("产品读取能力未核实")?;
                    match direct::resolve(&mut self.client, &self.route, cap)?
                        .write(&mut self.client, DeviceWriteSetting::Idle { raw_time })?
                    {
                        DeviceReadValue::Idle { raw_time } => raw_time,
                        _ => anyhow::bail!("设备回读没有休眠时间"),
                    }
                };
                ensure!(observed == raw_time, "设备回读与休眠设置不一致");
                Ok(Observation::Idle(observed))
            }
        }
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.client.is_stopped(),
            "电源通信进程已终止，未取得关闭回执"
        );
        self.client.request(ServiceRequest::Shutdown).map(|_| ())
    }

    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| "电源通信所有者发生异常".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

fn check(valid: &AtomicBool) -> anyhow::Result<()> {
    ensure!(
        valid.load(Ordering::Acquire),
        "电源请求范围已取消；已发送的设置仍可能生效"
    );
    Ok(())
}

pub(super) fn run(route: ObservedDevice, request: Request) -> Completion {
    let valid = request.guard();
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
        Err(error) => {
            return Completion {
                result: Err(error),
                shutdown_error: None,
                termination_error: None,
                destruction_error: None,
            };
        }
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
#[path = "mouse_current_power_worker_tests.rs"]
mod tests;

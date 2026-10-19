//! Stage readback and communication cleanup retain independent outcomes.
use super::runtime::Request;
use anyhow::ensure;
use razer_device::mouse_dpi_stages::DpiStagesReading;
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_ipc::{ServiceClient, ServiceRequest};

pub(super) struct Completion {
    result: Result<DpiStagesReading, String>,
    shutdown_error: Option<String>,
    termination_error: Option<String>,
    destruction_error: Option<String>,
}

impl Completion {
    pub(super) fn reading(&self) -> Option<&DpiStagesReading> {
        self.result.as_ref().ok()
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

    pub(super) fn message(&self, write: bool, current: bool) -> String {
        let mut message = if !current {
            "DPI 配置或连接已变化，旧设备阶段表结果已丢弃。".into()
        } else {
            match &self.result {
                Ok(_) if write => {
                    "设备已回读确认当前 DPI 阶段表；此确认不代表板载配置槽已保存。".into()
                }
                Ok(_) => "已读取设备 DPI 阶段表；隐藏阶段和独立 X/Y 设置保留本地配置。".into(),
                Err(error) => format!("DPI 阶段表尚未确认：{error}。请重新读取设备状态。"),
            }
        };
        if let Some(error) = self.cleanup_error() {
            message.push_str(&format!(" 通信清理：{error}"));
        }
        message
    }
}

trait Transport {
    fn execute(&mut self) -> anyhow::Result<DpiStagesReading>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn finish_termination(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    client: ServiceClient,
    route: ObservedDevice,
    request: Request,
}

impl Transport for NativeTransport {
    fn execute(&mut self) -> anyhow::Result<DpiStagesReading> {
        check(&|| self.request.is_valid())?;
        self.client.set_request_guard(self.request.valid.clone());
        direct::request_mouse_dpi_stages(&mut self.client, &self.route, self.request.draft.clone())
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.client.is_stopped(),
            "DPI 阶段表通信进程已终止，未取得关闭回执"
        );
        #[derive(serde::Deserialize)]
        struct Receipt {
            stopped: bool,
        }
        let receipt: Receipt =
            serde_json::from_value(self.client.request(ServiceRequest::Shutdown)?)?;
        ensure!(receipt.stopped, "DPI 阶段表通信进程未确认关闭");
        Ok(())
    }

    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| "DPI 阶段表通信所有者发生异常".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

fn check(valid: &impl Fn() -> bool) -> anyhow::Result<()> {
    ensure!(valid(), "DPI 阶段表请求已取消；已发送的设置仍可能生效");
    Ok(())
}

pub(super) fn run(route: ObservedDevice, request: Request) -> Completion {
    run_with(
        || request.is_valid(),
        || {
            Ok(NativeTransport {
                client: ServiceClient::spawn()?,
                route,
                request: request.clone(),
            })
        },
    )
}

fn run_with<T: Transport>(
    valid: impl Fn() -> bool,
    factory: impl FnOnce() -> anyhow::Result<T>,
) -> Completion {
    let mut transport = match guarded(|| {
        check(&valid)?;
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
        check(&valid)?;
        let reading = transport.execute()?;
        check(&valid)?;
        Ok(reading)
    });
    let shutdown_error = guarded(|| transport.shutdown()).err();
    let termination_error = guarded(|| transport.finish_termination()).err();
    let destruction_error = guarded(|| {
        drop(transport);
        Ok(())
    })
    .err();
    let result = result.and_then(|reading| {
        guarded(|| {
            check(&valid)?;
            Ok(reading)
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
#[path = "mouse_dpi_stages_worker_tests.rs"]
mod tests;

//! PID70 setter acknowledgement and communication cleanup are separate receipts.
use anyhow::ensure;
use razer_device::device_writes::DeviceWriteAck;
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::mouse_products::MouseLegacyDpiRequested;

pub(super) struct Completion {
    result: Result<DeviceWriteAck, String>,
    shutdown_error: Option<String>,
    termination_error: Option<String>,
    destruction_error: Option<String>,
}

impl Completion {
    pub(super) fn acknowledged(&self) -> bool {
        self.result.is_ok()
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

    pub(super) fn message(&self, context: &str, current: bool, x: u16, y: u16) -> String {
        let mut message = if !current {
            format!("{context}；配置或连接已变化，旧设备应答已丢弃。")
        } else {
            match &self.result {
                Ok(_) => format!("{context}，设备已应答 X {x} / Y {y} 设置。"),
                Err(error) => format!("{context}，设备设置未取得应答：{error}"),
            }
        };
        if let Some(error) = self.cleanup_error() {
            message.push_str(&format!(" 通信清理：{error}"));
        }
        message
    }
}

trait Transport {
    fn execute(&mut self) -> anyhow::Result<DeviceWriteAck>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn finish_termination(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    client: ServiceClient,
    route: ObservedDevice,
    request: MouseLegacyDpiRequested,
    x: u16,
    y: u16,
}

impl Transport for NativeTransport {
    fn execute(&mut self) -> anyhow::Result<DeviceWriteAck> {
        check(&|| self.request.is_valid())?;
        let cap = razer_device::mouse_legacy_polling::read_capability();
        self.client.set_request_guard(self.request.guard());
        let direct = direct::resolve(&mut self.client, &self.route, cap)?;
        check(&|| self.request.is_valid())?;
        direct.submit_legacy_dpi(&mut self.client, self.x, self.y)
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.client.is_stopped(),
            "DPI 通信进程已终止，未取得关闭回执"
        );
        #[derive(serde::Deserialize)]
        struct Receipt {
            stopped: bool,
        }
        let receipt: Receipt =
            serde_json::from_value(self.client.request(ServiceRequest::Shutdown)?)?;
        ensure!(receipt.stopped, "DPI 通信进程未确认关闭");
        Ok(())
    }

    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| "DPI 通信所有者发生异常".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

fn check(valid: &impl Fn() -> bool) -> anyhow::Result<()> {
    ensure!(valid(), "DPI 请求范围已取消；已发送的设置仍可能生效");
    Ok(())
}

pub(super) fn run(
    route: ObservedDevice,
    request: MouseLegacyDpiRequested,
    x: u16,
    y: u16,
) -> Completion {
    run_with(
        || request.is_valid(),
        || {
            Ok(NativeTransport {
                client: ServiceClient::spawn()?,
                route,
                request: request.clone(),
                x,
                y,
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
        let acknowledgement = transport.execute()?;
        check(&valid)?;
        Ok(acknowledgement)
    });
    let shutdown_error = guarded(|| transport.shutdown()).err();
    let termination_error = guarded(|| transport.finish_termination()).err();
    let destruction_error = guarded(|| {
        drop(transport);
        Ok(())
    })
    .err();
    let result = result.and_then(|acknowledgement| {
        guarded(|| {
            check(&valid)?;
            Ok(acknowledgement)
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
#[path = "mouse_dpi_70_worker_tests.rs"]
mod tests;

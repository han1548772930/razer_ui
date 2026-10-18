use super::{DpiReport, MouseCurrentDpiRequested, ObservedDevice, Ordering, direct};
use anyhow::ensure;
use razer_ipc::{ServiceClient, ServiceRequest};

pub(in crate::shell) struct Completion {
    result: Result<DpiReport, String>,
    shutdown_error: Option<String>,
    termination_error: Option<String>,
    destruction_error: Option<String>,
}

impl Completion {
    pub(super) fn report(&self) -> Option<&DpiReport> {
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

    pub(super) fn message(&self, request: &MouseCurrentDpiRequested) -> String {
        let message = match &self.result {
            Err(error) => format!("DPI 设备状态尚未确认：{error}；本地配置保留。"),
            Ok(_)
                if request
                    .change()
                    .is_some_and(|change| change.is_identity_read()) =>
            {
                "已读取设备序列号，正在载入对应配置。".into()
            }
            Ok(report) if request.is_read() => match report.active {
                Some(pair) if pair.x == pair.y => {
                    format!("设备当前 DPI：{}；页面显示本机配置。", pair.x)
                }
                Some(pair) => format!(
                    "设备当前 DPI：X {} / Y {}；页面显示本机配置。",
                    pair.x, pair.y
                ),
                None => "已读取设备 DPI 档位；页面显示本机配置。".into(),
            },
            Ok(_) => "DPI 已保存到本机，设备写入已应答。".into(),
        };
        match self.cleanup_error() {
            Some(error) => format!("{message} 通信清理：{error}"),
            None => message,
        }
    }
}

trait Transport {
    fn execute(&mut self) -> anyhow::Result<DpiReport>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn finish_termination(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    client: ServiceClient,
    route: ObservedDevice,
    request: MouseCurrentDpiRequested,
}

impl Transport for NativeTransport {
    fn execute(&mut self) -> anyhow::Result<DpiReport> {
        direct::mouse_current_dpi::request(
            &mut self.client,
            &self.route,
            self.request.change().cloned(),
        )
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.client.is_stopped(),
            "DPI 通信进程已终止，未取得关闭回执"
        );
        self.client.request(ServiceRequest::Shutdown).map(|_| ())
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

fn check(valid: &std::sync::atomic::AtomicBool) -> anyhow::Result<()> {
    ensure!(
        valid.load(Ordering::Acquire),
        "DPI 请求范围已取消；已发送的设置仍可能生效"
    );
    Ok(())
}

pub(super) fn run(route: ObservedDevice, request: MouseCurrentDpiRequested) -> Completion {
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
        shutdown_error,
        termination_error,
        destruction_error,
    }
}

#[cfg(test)]
#[path = "mouse_current_dpi_worker_tests.rs"]
mod tests;

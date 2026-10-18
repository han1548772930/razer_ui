//! One accepted brightness operation owns its service through shutdown,
//! process/pipe termination and destruction before delivering a result.
use anyhow::ensure;
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(in crate::shell) struct Completion {
    result: Result<u8, String>,
    shutdown_error: Option<String>,
    termination_error: Option<String>,
    destruction_error: Option<String>,
}

impl Completion {
    pub(in crate::shell) fn observed(&self) -> Option<u8> {
        self.result.as_ref().ok().copied()
    }

    pub(in crate::shell) fn cleanup_error(&self) -> Option<String> {
        let errors = [
            self.shutdown_error
                .as_ref()
                .map(|error| format!("通信进程关闭失败：{error}")),
            self.termination_error
                .as_ref()
                .map(|error| format!("通信进程回收失败：{error}")),
            self.destruction_error
                .as_ref()
                .map(|error| format!("通信所有者释放失败：{error}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        (!errors.is_empty()).then(|| errors.join("；"))
    }

    pub(in crate::shell) fn error(&self) -> Option<String> {
        let errors = self
            .result
            .as_ref()
            .err()
            .cloned()
            .into_iter()
            .chain(self.cleanup_error())
            .collect::<Vec<_>>();
        (!errors.is_empty()).then(|| errors.join("；"))
    }

    pub(super) fn failed(error: String) -> Self {
        Self {
            result: Err(error),
            shutdown_error: None,
            termination_error: None,
            destruction_error: None,
        }
    }
}

trait Transport {
    fn brightness(&mut self, percent: Option<u8>) -> anyhow::Result<u8>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn finish_termination(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    client: ServiceClient,
    route: ObservedDevice,
}

impl Transport for NativeTransport {
    fn brightness(&mut self, percent: Option<u8>) -> anyhow::Result<u8> {
        direct::request_keyboard_brightness(&mut self.client, &self.route, percent)
            .map(|reading| reading.percent)
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.client.is_stopped(),
            "通信进程已终止，未取得服务关闭回执"
        );
        self.client.request(ServiceRequest::Shutdown).map(|_| ())
    }

    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| "Keyboard brightness owner panicked".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

fn check(canceled: &AtomicBool) -> anyhow::Result<()> {
    ensure!(
        !canceled.load(Ordering::Acquire),
        "键盘亮度请求范围已取消；已发送的设置仍可能生效"
    );
    Ok(())
}

pub(super) fn run(
    route: ObservedDevice,
    percent: Option<u8>,
    canceled: Arc<AtomicBool>,
) -> Completion {
    let signal = canceled.clone();
    run_with(percent, &canceled, move || {
        let mut client = ServiceClient::spawn()?;
        client.set_cancellation_guard(signal);
        Ok(NativeTransport { client, route })
    })
}

fn run_with<T: Transport>(
    percent: Option<u8>,
    canceled: &AtomicBool,
    factory: impl FnOnce() -> anyhow::Result<T>,
) -> Completion {
    let mut transport = match guarded(|| {
        check(canceled)?;
        factory()
    }) {
        Ok(transport) => transport,
        Err(error) => return Completion::failed(error),
    };
    let result = guarded(|| {
        check(canceled)?;
        let observed = transport.brightness(percent)?;
        check(canceled)?;
        Ok(observed)
    });
    // Cancellation revokes ordinary request admission, while ServiceClient
    // still admits Shutdown. A terminated client has no shutdown receipt;
    // termination is checked independently and both failures remain visible.
    let shutdown_error = guarded(|| transport.shutdown()).err();
    let termination_error = guarded(|| transport.finish_termination()).err();
    let destruction_error = guarded(|| {
        drop(transport);
        Ok(())
    })
    .err();
    let result = result.and_then(|observed| {
        guarded(|| {
            check(canceled)?;
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
#[path = "keyboard_brightness_worker_tests.rs"]
mod tests;

//! Application root owns bootstrap and a connection lease across all windows.
use razer_ipc::hid_broker::{BrokerBootstrap, BrokerClient};
use std::time::Duration;

pub(super) struct Runtime {
    connection: BrokerClient,
    bootstrap: Option<BrokerBootstrap>,
}

impl Runtime {
    pub(super) fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let executable = razer_ipc::service_executable()?;
        let (connection, bootstrap) =
            razer_ipc::hid_broker::connect_or_start(&executable, Duration::from_secs(15))?;
        Ok(Self {
            connection,
            bootstrap,
        })
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // Application::run has returned. This root lease is independent of the
        // operation sessions and never requests termination of another owner.
        if let Err(error) = self.connection.close_session() {
            log::error!("根设备服务会话清理未确认：{error:#}");
        }
        if let Some(bootstrap) = self.bootstrap.as_mut() {
            match bootstrap.finish(Duration::from_secs(3)) {
                Ok(receipt) if !receipt.status.success() => {
                    log::error!("设备服务以失败状态退出：{}", receipt.status);
                }
                Ok(_) => {}
                Err(error) => {
                    // Bootstrap Drop transfers the live process and stderr
                    // drain to its reaper, preserving other owners' sessions.
                    log::error!("设备服务仍由在途工作或其他会话持有：{error:#}");
                }
            }
        }
    }
}

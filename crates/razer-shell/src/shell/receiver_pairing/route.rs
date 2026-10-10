//! Command collection selection. Shared protocol does not use Windows APIs.
use razer_ipc::ServiceClient;

#[cfg(windows)]
mod windows;

pub(super) fn command_path(
    client: &mut ServiceClient,
    product_id: u32,
    container: &str,
) -> anyhow::Result<String> {
    #[cfg(windows)]
    {
        windows::command_path(client, product_id, container)
    }
    #[cfg(not(windows))]
    {
        let _ = (client, product_id, container);
        anyhow::bail!("配对硬件事件的物理接收器拓扑尚未在此平台实现")
    }
}

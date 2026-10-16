//! SetupAPI identity proof for hidapi's unknown (-1) interface number.
//! Never rewrite HidNode fields or infer interface from Report ID/VID/PID.
use super::{hid, receiver};
use anyhow::{Context as _, ensure};
use razer_device::backend::HidNode;
use serde_json::{Value, json};

pub(super) fn proof(node: &HidNode) -> anyhow::Result<Value> {
    ensure!(node.vendor_id == 5426, "校准 HID vendor 不符");
    let path = std::str::from_utf8(&node.path).context("校准 Windows HID 路径不是 UTF-8")?;
    ensure!(
        path.is_ascii()
            && !path.contains('\0')
            && path.to_ascii_lowercase().starts_with(r"\\?\hid#"),
        "校准 Windows HID 路径无效"
    );
    let snapshot = hid::enumerate_for_product(node.vendor_id, node.product_id)?;
    ensure!(snapshot["complete"] == true, "校准 Windows HID 枚举未完成");
    let matches = snapshot["interfaces"]
        .as_array()
        .context("校准 Windows HID 接口缺失")?
        .iter()
        .filter(|item| {
            item["path"]
                .as_str()
                .is_some_and(|current| current.eq_ignore_ascii_case(path))
        })
        .collect::<Vec<_>>();
    let [observed] = matches.as_slice() else {
        anyhow::bail!("校准路径没有唯一的实际 Windows 接口");
    };
    ensure!(
        observed["vendor_id"] == node.vendor_id
            && observed["product_id"] == node.product_id
            && observed["claim_interface"] == 1
            && observed["feature_report_bytes"] == 91,
        "实际 Windows 接口不是源 claim1/Feature91"
    );
    let container = observed["device_container_id"]
        .as_str()
        .context("校准接口缺少 ContainerId")?;
    ensure!(
        receiver::valid_container(container),
        "校准实际 ContainerId 无效"
    );
    let instance = observed["device_instance_id"]
        .as_str()
        .filter(|value| !value.is_empty())
        .context("校准接口缺少实际实例身份")?;
    Ok(
        json!({"path":path.to_ascii_lowercase(),"device_container_id":container.to_ascii_lowercase(),
        "device_instance_id":instance.to_ascii_lowercase(),"vendor_id":node.vendor_id,
        "product_id":node.product_id,"claim_interface":1,"feature_report_bytes":91}),
    )
}

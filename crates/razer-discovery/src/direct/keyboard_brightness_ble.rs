use super::keyboard_metadata;
use crate::discovery::ObservedDevice;
use anyhow::{Context as _, ensure};
use razer_device::{
    keyboard_settings::{
        BrightnessSubmission, KeyboardBrightness,
        ble::{Acknowledgement, Report},
    },
    mouse_runtime_artwork::WindowsBleTarget,
};
use razer_ipc::{ServiceClient, ServiceRequest};
use serde::Deserialize;

#[derive(Deserialize)]
enum SourceClass {
    #[serde(rename = "rzDevice30")]
    Protocol30,
}
#[derive(Deserialize)]
enum Transport {
    #[serde(rename = "windows_gatt")]
    WindowsGatt,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    target: WindowsBleTarget,
    source_class: SourceClass,
    transport: Transport,
    connection_edition: Acknowledgement,
    report: Report,
}

pub(super) fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    submission: Option<BrightnessSubmission>,
) -> anyhow::Result<KeyboardBrightness> {
    let keyboard_metadata::Route::Ble(target) = keyboard_metadata::resolve(client, observed)?
    else {
        anyhow::bail!("Keyboard brightness resolved a different physical transport");
    };
    let reply = client.request(ServiceRequest::BleKeyboardBrightness {
        target: target.clone(),
        submission,
    })?;
    decode(reply, &target, submission)
}

fn decode(
    reply: serde_json::Value,
    target: &WindowsBleTarget,
    submission: Option<BrightnessSubmission>,
) -> anyhow::Result<KeyboardBrightness> {
    let reply: Reply = serde_json::from_value(reply)?;
    ensure!(
        reply.target == *target,
        "Keyboard BLE brightness target changed"
    );
    let (SourceClass::Protocol30, Transport::WindowsGatt) = (reply.source_class, reply.transport);
    ensure!(
        reply
            .connection_edition
            .matches(razer_device::keyboard_settings::ble::EDITION, 0, 0)
            && reply.connection_edition.data().len() >= 2,
        "Keyboard BLE connection edition is absent"
    );
    reply.report.check_request(target.product_id, submission)?;
    reply
        .report
        .reading()
        .cloned()
        .context("Keyboard BLE brightness was not observed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn target() -> WindowsBleTarget {
        serde_json::from_value(json!({
            "product_id":600, "physical_product_id":626,
            "device_container_id":"{10000000-0000-0000-0000-000000000001}",
            "hid_path":"simulated-keyboard-ble-626",
            "device_instance_id":"simulated-hid-instance",
            "parent_instance_id":"simulated-ble-parent", "mac_address":0x102030405060u64
        }))
        .unwrap()
    }

    fn reading_reply(target: &WindowsBleTarget) -> serde_json::Value {
        json!({"target":target,"source_class":"rzDevice30","transport":"windows_gatt",
            "connection_edition":{"transaction":0,"command":[0,1,134],"profile":0,"selector":0,"data":[1,2]},
            "report":{"product_id":600,"requested":null,"confirmed_enabled":null,
                "acknowledgements":[
                    {"transaction":1,"command":[0,16,128],"profile":0,"selector":1,"data":[1]},
                    {"transaction":2,"command":[0,16,133],"profile":1,"selector":1,"data":[127]}],
                "reading":{"profile_id":1,"region_id":1,"raw_value":127,"percent":50}}})
    }

    #[test]
    fn keyboard_ble_brightness_reply_preserves_actual_reading() {
        let target = target();
        let reading = decode(reading_reply(&target), &target, None).unwrap();
        assert_eq!((reading.raw_value, reading.percent), (127, 50));
    }

    #[test]
    fn keyboard_ble_brightness_reply_rejects_stale_target_and_unconfirmed_data() {
        let target = target();
        for (path, value) in [
            ("/target/mac_address", json!(1)),
            ("/connection_edition/command", json!([0, 16, 133])),
            ("/report/reading/percent", json!(51)),
            ("/report/confirmed_enabled", json!(true)),
            ("/report/acknowledgements/1/selector", json!(2)),
            ("/report/acknowledgements/1/data", json!([])),
        ] {
            let mut reply = reading_reply(&target);
            *reply.pointer_mut(path).unwrap() = value;
            assert!(decode(reply, &target, None).is_err(), "{path}");
        }
    }
}

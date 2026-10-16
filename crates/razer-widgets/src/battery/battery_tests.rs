//! 电量档位与原版状态机的逐值对照（只编译、不运行）。
//!
//! 依据：`.ref/devices/104/static/js/main.9b94769e.js` 的 `Sa`／`qe`。
// 只取需要的符号：`use super::*` 会把父模块的 `gpui_kit::*` 一并带进来，
// 其中的 `test` 会遮蔽内置的 `#[test]` 属性（展开递归超限）。
use super::{badge, bucket};

/// 分档与原版 `Sa` 的公式逐值对齐（含 10 与 10..20 两个特例分支）。
#[test]
fn bucket_matches_source() {
    assert_eq!(bucket(0), 0);
    assert_eq!(bucket(1), 0);
    assert_eq!(bucket(9), 0);
    assert_eq!(bucket(10), 10);
    assert_eq!(bucket(11), 20);
    assert_eq!(bucket(19), 20);
    assert_eq!(bucket(20), 20);
    assert_eq!(bucket(47), 40);
    assert_eq!(bucket(99), 90);
    assert_eq!(bucket(100), 100);
}

/// `chargingStatus` 的每个分支都映到原版类名与图标。
#[test]
fn status_mapping_matches_source() {
    let off = badge(50, "off");
    assert_eq!(off.class, "batt batt-off");
    assert_eq!(off.icon, "synapse/battery-off.svg");

    let charging = badge(50, "Charging");
    assert_eq!(charging.class, "batt charging");
    assert_eq!(charging.icon, "synapse/battery-charging.svg");

    // Lower-level payloads can use all-caps enum values; they map to the same
    // source badge instead of falling through to the default 100% icon.
    let charging_upper = badge(50, "CHARGING");
    assert_eq!(charging_upper.class, "batt charging");
    let off_upper = badge(50, "OFF");
    assert_eq!(off_upper.class, "batt batt-off");

    let full = badge(99, "Charging");
    assert_eq!(full.class, "batt charging100");
    assert_eq!(full.icon, "synapse/battery-charging-100.svg");

    let idle = badge(47, "NoCharge_BatteryFull");
    assert_eq!(idle.class, "batt batt-40");
    assert_eq!(idle.icon, "synapse/battery-40.svg");

    // `pV.PAUSED_CHARGING` 的字面量是 `ReachChargingLimit`，图标取自暂停档片段。
    let paused = badge(80, "ReachChargingLimit");
    assert_eq!(paused.class, "batt batt-80 paused");
    assert_eq!(paused.icon, "synapse/battery-paused-80.svg");

    let disconnected = badge(0, "NoCharge_BatteryFull");
    assert_eq!(disconnected.class, "batt batt-disconnected");
    assert_eq!(disconnected.icon, "synapse/battery-disconnected.svg");

    let warning = badge(50, "batt-warning");
    assert_eq!(warning.class, "batt batt-warning");
    assert_eq!(warning.icon, "synapse/battery-error.svg");
    assert!(warning.warning);

    // default 分支：原版取 `Sa(100)`。
    let unknown = badge(50, "something-new");
    assert_eq!(unknown.class, "batt batt-100");
    assert_eq!(unknown.icon, "synapse/battery-100.svg");
}

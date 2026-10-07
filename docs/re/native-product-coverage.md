# 当前原生页面覆盖审计

本报告按实际工作区分派和生成数据统计。`partial_native` 只表示已有部分原生内容，不代表界面、交互或硬件行为已完整还原。没有产品被标记为全部完成。

注册 331 个产品入口；331 个有部分主页面内容；主导航共 1419 页。

| 页面状态 | 数量 |
| --- | ---: |
| partial_native | 1392 |
| partial_native_reaudited | 27 |

原有十个适配器按页复核：27 页已确认本地路由与源码依据（`partial_native_reaudited`），0 页仍记为 `existing_partial_not_reaudited_here`。复核结果仍是部分内容，不等于视觉一致。独立模式另列，未计作主页面实现。

## 尚无主页面内容的产品

无；这仍不表示所有产品已经完成。

## 完全待接入的主页面

| 产品 ID | 产品 | 页面 |
| --- | --- | --- |

## 仍需完成

- Renderer/descriptor presence is not visual equivalence, complete behavior, or hardware support.
- Mouse/keyboard custom mappings, alternate layouts, advanced actions and some conditional interactions remain partial.
- Camera preview, enumeration, framing presets, overlays and hardware commands remain incomplete.
- Audio demo pages and complex mappings remain incomplete; DSP and haptics are local drafts.
- Accessory port discovery, pairing workflows, lighting color parameters and Hue discovery remain incomplete.
- Accessory source bodies and preview fixtures do not prove full navigation, profile-menu, animation or modal parity.
- System controls do not apply hardware settings or fabricate temperature, fan RPM, SKU or display modes.
- Help retains per-page source conditions; unavailable firmware/reset/system services stay unavailable.
- Independent displayMode branches are registered as evidence but not automatically exposed by the primary workspace.
- Legacy adapter pages marked `partial_native_reaudited` have a verified local route and source basis; that is still partial content, not visual parity.
- No application, build, tests, installer, downloaded JavaScript or DLL was executed for this audit.

## 不包含在设备页计数中的界面缺口

- 托盘：账户/通知/Widgets 已有 UI；真实发布者、多应用与动态高度见 [当前托盘契约](tray-ui-current.md)。
- OLED：卡片和编辑器已部分接入；语言下载、设备传输及其余条件见 [当前 OLED 契约](oled-ui-current.md)。
- 宿主服务：独立 Settings 窗口已有本地实现；账户、目录、固件/重置等真实服务与界面完成分开核实。
- 跨平台：公共托盘菜单及界面已与 Windows 适配拆分，macOS/Linux 的系统注册适配仍未实现。

逐产品页面身份、实际路由和输入 SHA-256 见 [机器可读记录](native-product-coverage.json)。

重新生成：`python -X utf8 tools/audit-native-product-coverage.py`。该命令只解析本地数据和 Rust 源码。

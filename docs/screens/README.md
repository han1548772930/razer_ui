# 当前界面契约索引

按当前源码和实际实现查阅下表。每份契约只证明它明确核实的范围；路由、描述符、资源存在和编译通过均不等于整页完成。已修复条目以契约记录为准，源码和相关实现未变化时不重复修复。

当前 Dashboard 与宿主版本依据见[来源版本](../re/20-current-source-version.md)和[宿主包审计](../re/current-host-version-audit.md)。工作顺序、只读观察、本地编辑与 DLL 写回边界见[路线图](../re/ui-readonly-first-roadmap.md)。

| 界面主题 | 当前契约及核实范围 |
| --- | --- |
| 应用壳、页签与系统窗口 | [宿主窗口策略](../re/host-window-policy-current-audit.md)、[窗口打开契约](../re/display-window-contract.md)、[设备导航](../re/device-tabs-audit.md) |
| 工具栏、保存和退出 | [当前托盘/关闭](../re/tray-ui-current.md)、[未保存项源收据](../re/save-close-current-source.json) |
| 自定义、按键与映射 | [鼠标/键盘当前核实](../re/mouse-keyboard-current-audit.md)、[快捷键与宏契约](../re/shortcuts-macro-current-contract.md)；具体能力不能跨产品套用 |
| DPI、滚轮与鼠标属性 | [鼠标 UI 当前契约](../re/mouse-ui-current.md)；182 既有页面见适配器覆盖边界 |
| 配对与接收器 | [接收器当前 UI](../re/receiver-ui-current.md)、[发现/读取契约](../re/dll-readonly-inventory.md) |
| 校准与 Smart Tracking | [Smart Tracking](../re/smart-tracking-current-audit.md)、[键盘校准](../re/keyboard-calibration-ui-current.md) |
| 设备电源 | [鼠标/键盘能力](../re/mouse-keyboard-current-audit.md)、[音频产品能力](../re/audio-products-native.md)；原有适配器的字段级行为仍须按对应产品源码核实 |
| 设备灯光与鼠标垫 | [原有适配器覆盖边界](../re/legacy-adapter-page-reaudit.md)、[当前资源清单](../../assets/synapse/manifest.json)、[产品内容布局](../re/product-content-current-audit.md) |
| Sound / Mic | [当前音频能力与证据生成](../re/audio-products-native.md)、[原有适配器覆盖边界](../re/legacy-adapter-page-reaudit.md)；不能将别的产品 EQ 或默认字段移植给 777 |
| Dashboard | [卡片状态](../re/dashboard-ui-current.md)、[设备身份契约](../re/device-identity-current-contract.md) |
| Devices & Modules / 应用选择器 | [模块服务行](../re/module-service-ui-current.md)、[正式入口与预览隔离](../re/service-preview-current-audit.md)、[应用菜单当前下载收据](../../.ref/applications/rz-app-menu/source.json) |
| Gamer Room / Wi-Fi 添加 | [Gamer Room](../re/gamer-room-ui-current.md)、[正式入口与预览隔离](../re/service-preview-current-audit.md)；真实扫描、配网和安装不能由预览页面冒充 |
| 设备 Help / 注册 | [产品注册](../re/product-registration-audit.md)、[原有 Help 适配器范围](../re/legacy-adapter-page-reaudit.md)、[当前 Help 实现](../../crates/razer-pages/src/features/help_page.rs) |
| Settings | [当前设置审计](../re/settings-current-audit.md)；服务连接和本地预览是明确分开的项目功能 |
| Profile Migration | [当前迁移入口/普通页边界](../re/profile-migration-header-audit.md)、[独立当前源核验](../re/profile-migration-current-source.json) |
| 账户菜单 | [托盘/账户关闭边界](../re/tray-ui-current.md)、[当前菜单原始下载收据](../../.ref/rz-user-profile-menu/source.json)、[当前菜单实现](../../crates/razer-shell/src/shell/account_menu.rs) |
| Introduction Tour | [具名窗口路由](../re/display-window-contract.md)、[教程媒体源/输出收据](../../assets/synapse/tutorial-media-manifest.json)、[当前教程实现](../../crates/razer-app-pages/src/introduction_tour.rs) |
| Alexa | [正式页和预览隔离](../re/service-preview-current-audit.md)、[独立下拉契约](../re/alexa-dropdown-audit.md)、[当前源语言收据](../re/alexa-source-locales.json) |
| Chroma Studio 属性 | [Studio 当前属性契约](../re/studio-properties-current.md) |
| 宏编辑器 | [宏 UI 当前契约](../re/macro-ui-current.md) |

原有 182、653、777 与七款鼠标垫仍有独立适配器。[适配器覆盖审计](../re/legacy-adapter-page-reaudit.md)只核实本地路由、页面可达性和当前数据存在，不提供页面内部逐字段、视觉或硬件行为的完整证明。上述链接保持这个范围，不把旧逐页说明换个标题继续当作已审计结论。

原始产品配置、页面组件和媒体字节继续保存在当前 `.ref/devices/`、`.ref/applications/` 及其维护的提取结果里。教程的逐帧尺寸/延时、产品图的 edition/layout/用途与源哈希、Alexa 原字典、应用菜单和账户菜单下载回执均保留在表中；清理过期说明不删除这些证据。

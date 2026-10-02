# 逐页规格索引

页面是否存在，以产品根 `navs` 和实际组件树为准。以下链接包含交互、资源和当前实现差异。

索引覆盖当前已取得源码；原版并非只有三个产品模块，完整范围说明见[产品模块清单](../re/15-product-module-inventory.md)。

| 文档 | 适用范围 |
|---|---|
| [00 应用壳层](00-app-shell.md) | Electron 标签栏、前端工具栏、导航、profile、内容布局 |
| [01 自定义](01-customize.md) | 182 鼠标与 653 键盘；按键图、映射、Hypershift、游戏模式、Snap Tap、Command Dial |
| [02 性能](02-performance.md) | 182 独立页面；653 的回报率仅作为 Customize 中的关联功能 |
| [03 配对](03-pairing.md) | 182 独立 multiDevicePairing 模式及 frontend 共享配对状态机 |
| [04 校准](04-calibration.md) | 182 Smart Tracking |
| [05 电源](05-power.md) | 182 / 777，范围和开关行为不同 |
| [07 灯光](07-lighting.md) | 653 / 777，灯效列表和附属卡片不同 |
| [08 声音](08-sound.md) | 777 音量、Windows 属性、audioEq |
| [09 麦克风](09-mic.md) | 777 micEq、原包频率数据矛盾；当前入口只有 EQ |
| [10 主前端页面](10-main-frontend-pages.md) | Dashboard、Gamer Room、Devices & Modules、Global Shortcuts、Settings 边界 |
| [11 设备帮助](11-help.md) | 182 / 653 / 777 帮助路由、产品支持、序列号、版本和注册；恢复出厂服务边界 |
| [12 设置](12-settings.md) | 独立 Settings 的 Synapse / General 页面、偏好保存及服务连接扩展 |
| [13 配置迁移](13-profile-migration.md) | Settings打开的独立迁移应用、分组卡片、选择与状态弹层；真实扫描/转换边界 |
| [14 Wi-Fi 添加](14-iot-add.md) | Key Light／Gamer Room 分支、网络与设备列表、返回与密码清理、显式状态预览 |
| [15 账户菜单](15-account-menu.md) | 访客菜单的原版外观、焦点、退出保护及账户服务边界 |

产品导航顺序：

- 182：Customize → Performance → Power → Calibration；HELP；Pairing 为独立模式。
- 653：Customize → Lighting；HELP。
- 777：Sound → Mic → Lighting → Power；HELP。

文中数值首先表示原始 CSS px 或原始业务数值。后续 GPUI Kit 实现统一换算缩放，并通过领域事件更新状态；不要把页面文档中的 React 符号直接设计成 Rust 公共 API。

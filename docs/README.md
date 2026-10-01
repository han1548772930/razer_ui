# Razer Synapse 4 原版审计与 GPUI Kit 对照文档

本目录以当前 `.ref` 中的原版 Electron / React 代码为依据，重新核对页面、交互、数据和资源。更新日期：2026-10-01。当前 Rust UI 只作为实现对照对象，不作为原版规格的证据。

建议按以下顺序阅读：

1. [总规格](RAZER-SYNAPSE-UI-SPEC.md)：实际页面路由、设备能力和整体约束。
2. [逐页规格](screens/README.md)：组件树、条件、数值范围、交互及资源。
3. [源码审计依据](re/00-source-audit.md)：版本、入口定位方法、证据可信度与旧文档更正。
4. [当前实现差异](re/03-implementation-gap.md)：原版与 Rust 的具体差异、优先级和验收条件。
5. [资源索引](re/04-resource-index.md)：实际资源引用、原始名称到构建文件的映射、下载缺口。
6. [GPUI Kit 实现映射](re/05-gpui-kit-mapping.md)：按项目 skills 和 0.7.0 源码确认的组件、状态和资源接入要求。
7. [IPC / API 清单](re/01-ipc-api-surface.md)、[灯光协议](re/02-lighting-actions.md)：宿主边界与协议层，不代表当前 UI 已接通硬件。
8. [UI 样式复核与资源同步](re/06-style-source-audit.md)：本轮按原版 JS/CSS 修复的导航、鼠标叠层、开关、EQ 和资源生成链，不使用旧截图作为依据。
9. [全部页面与附属界面覆盖](re/07-page-coverage.md)：主应用及产品的 14 个普通页面实例、帮助、配对、Profile、抽屉和教程，并记录动态图片、下拉框与仍未接入的部分。
10. [映射编辑器审计](re/09-mapping-editor.md)：分类、录制、物理键与修饰键、输入能力、旧值保留和本地保存边界。
11. [运行时接入](re/10-runtime-integration.md)：已证实的 FFI 签名、后台 worker、HID 元数据、服务查询和原快捷键读取协议缺口。
12. [映射警告审计](re/11-mapping-warnings.md)：Windows 登录提示和 Quick Remapping 冲突的真实触发条件及当前适用范围。
13. [全局快捷键引擎编码](re/12-global-shortcut-encoding.md)：原输入/输出、设备变体、DKM 别名、hash，以及未注册 Turbo 的输出限制。

文中 `[JS]` 表示实际渲染或事件代码，`[CONFIG]` 表示静态产品配置，`[CSS]` 表示样式基线，`[RUST 基线]` 表示重构前实现，`[建议]` 表示后续实现方案。静态能力、运行时条件和已经验证的设备行为必须分开理解。

文档审计后已接入 Rust 设备工作区、本地 Profile 导入导出/关联、Snap Tap 多键对、自定义 Command Dial、全局快捷键编辑/编码和设置中的独立服务读取面板。当前编译入口、配置保存、资源、回归源码和剩余差异统一维护在[重构状态](re/03-implementation-gap.md)。本轮仅做 `cargo check`，不运行 build、测试、应用、worker 或 DLL；动态安装状态、设备回读、固件及真实交互仍未运行验证。

# razer_ui

使用 Rust + GPUI Kit 重建 Razer Synapse 4 的原生 UI。

## 先读规范

唯一的 UI 规格来源是 [`docs/RAZER-SYNAPSE-UI-SPEC.md`](docs/RAZER-SYNAPSE-UI-SPEC.md)。它基于 `.ref/` 中保存的雷蛇原始前端、产品模块、CSS、语言包和 Electron 桥接代码，包含：

- Dashboard、应用设置和独立设备页的层级关系。
- 产品模块驱动的页面能力和设备差异。
- 顶栏、widget、产品图片区、Dashboard 卡片的布局。
- 原版颜色、字号、圆角、按钮、开关、下拉、滑块和浮层状态。
- 182 鼠标的自定义、性能、电源与校准，653 键盘的自定义与灯光，以及 777 耳机的声音、麦克风、灯光与电源。
- 配对、帮助、Profile 与映射等附属界面的实际入口和能力条件。
- 目标模块边界、异步状态和验收清单。

逐界面规格见 [`docs/screens/README.md`](docs/screens/README.md)，每个页面分别记录设备归属、卡片分区、CSS 类名、尺寸、状态和原始文案 key。

当前已接入设备工作区、本地 Profile 管理/导入导出/关联程序、输入抽屉和分类映射编辑器、16 种键盘布局的精确命中、五槽 DPI 步进与排序、四组 Snap Tap、自定义 Command Dial，以及全局快捷键编辑和原引擎编码。

设置中的服务面板在点击连接后，通过后台 worker 独立读取 HID 接口元数据、版本和音频列表；接口尚未合并为完整设备工作区。设备参数写入/回读、宏/跨设备/Chroma 等服务仍未接入；原快捷键配置缺少已证实的读取 ABI，整表替换提交入口保持禁用。本地保存不表示设备或原生服务已经应用设置。当前代码、资源计数和验证状态见[重构状态](docs/re/03-implementation-gap.md)。

本轮仅做 `cargo check --locked --all-targets`，不执行 build、测试、应用、worker 或 DLL；测试源码的编译通过不代表交互和硬件验收。

底层契约：

- [`docs/re/01-ipc-api-surface.md`](docs/re/01-ipc-api-surface.md)
- [`docs/re/02-lighting-actions.md`](docs/re/02-lighting-actions.md)
- [运行时接入与服务边界](docs/re/10-runtime-integration.md)
- [映射警告的真实触发条件](docs/re/11-mapping-warnings.md)
- [全局快捷键原生引擎编码](docs/re/12-global-shortcut-encoding.md)

## 运行

```bash
cargo run
```

常用参数：`--probe`、`--selftest`、`--demo-keyboard`、`--tab <名字>`、`--select <productId>`。

## 原始参考

```text
.ref/frontend/                 主前端
.ref/devices/182/              DeathAdder V3 Pro
.ref/devices/653/              BlackWidow V4 Pro
.ref/devices/777/              Kraken BT Sanrio
.ref/synapse-asar/             Electron 壳和设备桥
.ref/tools/                    抓取、扫描、CSS 审计工具
```

项目目标不是复制 Electron，而是使用原生 Rust UI 实现同样的产品能力和交互模型。

# razer_ui

使用 Rust + GPUI Kit 重建 Razer Synapse 4 的原生 UI。

## 先读规范

唯一的 UI 规格来源是 [`docs/RAZER-SYNAPSE-UI-SPEC.md`](docs/RAZER-SYNAPSE-UI-SPEC.md)。它基于 `.ref/` 中保存的雷蛇原始前端、产品模块、CSS、语言包和 Electron 桥接代码，包含：

- Dashboard、应用设置和独立设备页的层级关系。
- 产品模块驱动的页面能力和设备差异。
- 顶栏、widget、产品图片区、Dashboard 卡片的布局。
- 原版颜色、字号、圆角、按钮、开关、下拉、滑块和浮层状态。
- 自定义、宏、性能、灯光、电源、滚动、配对、校准、音频、EQ、麦克风、OLED 和触觉反馈功能。
- 目标模块边界、异步状态和验收清单。

逐界面规格见 [`docs/screens/README.md`](docs/screens/README.md)，每个页面分别记录设备归属、卡片分区、CSS 类名、尺寸、状态和原始文案 key。

底层契约：

- [`docs/re/01-ipc-api-surface.md`](docs/re/01-ipc-api-surface.md)
- [`docs/re/02-lighting-actions.md`](docs/re/02-lighting-actions.md)

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

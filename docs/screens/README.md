# 逐界面原版规格

这些文件只记录雷蛇原始前端的页面事实：页面归属、页面顺序、布局分区、CSS 类名、尺寸、颜色、状态和原始文案 key；不记录当前 Rust 项目的完成度。

## 先读

| 文档 | 范围 |
|---|---|
| [00-app-shell.md](00-app-shell.md) | Dashboard、应用设置、产品窗口、顶栏和全局弹窗 |
| [01-customize.md](01-customize.md) | 自定义、按键绑定、Hypershift、动作编辑器 |
| [02-performance.md](02-performance.md) | DPI、轮询率、传感器、键盘性能 |
| [03-pairing.md](03-pairing.md) | 无线接收器、配对、扫描和解绑 |
| [04-calibration.md](04-calibration.md) | 表面校准、摇杆校准、灵敏度匹配 |
| [05-power.md](05-power.md) | 电池、充电、睡眠和省电 |
| [06-scrolling.md](06-scrolling.md) | 滚轮模式、阻尼、触觉和高分辨率滚动 |
| [07-lighting.md](07-lighting.md) | Chroma 效果、区域、颜色、速度和亮度 |
| [08-sound.md](08-sound.md) | 音量、音频模式、THX、EQ 和输出 |
| [09-mic.md](09-mic.md) | 麦克风、监听、降噪、录音和 XLR |
| [10-main-frontend-pages.md](10-main-frontend-pages.md) | 主前端的首页、设置和扩展页面能力 |

## 设备页面矩阵

| 设备 | productId | 页面顺序 |
|---|---:|---|
| Razer DeathAdder V3 Pro | `182` | 自定义、性能、正在配对、校准、电源、滚动 |
| BlackWidow V4 Pro | `653` | 自定义、性能、灯光、电源、滚动 |
| RAZER KRAKEN BT SANRIO LIMITED EDITION | `777` | 自定义、灯光、校准、电源、声音、麦克风 |

## 每个页面文档的固定结构

1. 出现设备和模块声明顺序。
2. 产品页共用骨架和该页自己的卡片排列。
3. 产品差异：同一页面在不同 productId 下的分区、控件和能力。
4. CSS 类名、布局、颜色、字号、圆角、状态和过渡。
5. 语言包中的真实功能 key、中文原文和英文原文。

## 证据

- 产品模块：`.ref/devices/<productId>/`
- 主前端：`.ref/frontend/`
- 设备模块页面生成器：`.ref/tools/gen-screen-docs.js`
- CSS 审计工具：`.ref/tools/visual-system.js`、`.ref/tools/audit-css.js`

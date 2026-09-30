# 逐界面原版规格

这些文件只记录雷蛇原始前端的页面事实：页面归属、页面顺序、布局分区、CSS 类名、尺寸、颜色、状态和原始文案来源；不记录当前 Rust 项目的完成度，也不把未验证能力写成已实现功能。

## 证据优先级

1. `.ref/devices/<productId>/static/js` 与 `.ref/devices/<productId>/static/css`：产品页面最终依据。
2. `.ref/frontend/static/js` 与 `.ref/frontend/static/css`：主前端 Dashboard、产品顶栏、Gamer Room、固件更新和全局入口依据。
3. `.ref/frontend/locales/`、`trans-*.chunk.js`：文案 key 和本地化文本依据。
4. `.ref/frontend/manifest.json`、`asset-manifest.json`：入口 bundle、构建版本和应用壳信息。
5. `.ref/tools/`：只作为审计辅助，不能覆盖实际 bundle 中的组件和 CSS 证据。

截图、当前 Rust 文件名、旧版 MD 或通用产品经验都不能推翻原始 bundle。没有 JavaScript/CSS/locale/manifest 证据的能力必须标记为“未验证”，不能继续保留为确定布局或功能。

## 文档索引

| 文档 | 范围 |
|---|---|
| [00-app-shell.md](00-app-shell.md) | Electron 外层标签栏、产品顶栏、Dashboard、产品窗口和全局弹窗 |
| [01-customize.md](01-customize.md) | 自定义、按键绑定、Hypershift、动作编辑器 |
| [02-performance.md](02-performance.md) | DPI、轮询率、传感器和键盘性能 |
| [03-pairing.md](03-pairing.md) | 无线接收器、配对、扫描和解绑 |
| [04-calibration.md](04-calibration.md) | 182 鼠标表面校准、校准中/完成/失败状态；其他设备变体需重新核对 bundle |
| [05-power.md](05-power.md) | 电池、充电、睡眠和省电 |
| [06-scrolling.md](06-scrolling.md) | 182/653 已证实的滚轮模式、阶段选择、阻尼/触觉分支 |
| [07-lighting.md](07-lighting.md) | Chroma 效果、资源状态和设备灯光分支 |
| [08-sound.md](08-sound.md) | 777 音频页面及其已证实的 EQ 组件 |
| [09-mic.md](09-mic.md) | 777 麦克风页面及其已证实的组件 |
| [10-main-frontend-pages.md](10-main-frontend-pages.md) | 主前端壳层、四个顶层入口、设备 Dashboard、Gamer Room 和固件更新 |

## 产品页面矩阵

| 设备 | productId | 当前由产品 bundle 证实的页面顺序 |
|---|---:|---|
| Razer DeathAdder V3 Pro | `182` | 自定义、性能、配对、校准、电源、滚动 |
| BlackWidow V4 Pro | `653` | 自定义、性能、灯光、电源、滚动 |
| RAZER KRAKEN BT SANRIO LIMITED EDITION | `777` | 自定义、灯光、校准、电源、声音、麦克风 |

该矩阵只描述已审计产品模块的入口顺序；页面内部的控件、状态和颜色以对应文档及产品源码为准。主前端的 locale key 不能扩大产品能力。

## 每个产品页面文档的固定结构

1. 出现设备和模块的源码条件与顺序。
2. 产品页面共用骨架和该页自己的卡片排列。
3. 不同 `productId` 的分区、控件、能力 flag 和隐藏条件。
4. CSS 类名、布局、尺寸、颜色、字体、圆角、状态和过渡。
5. locale 中可定位的功能 key、中文原文和英文原文；无法定位的文案不补写。
6. 明确列出未验证项，避免实现把推断内容做成可点击功能。

## 维护规则

- 修改前先检查 `.ref` 的真实文件名和入口关系。
- 只保留能回溯到源码的布局和功能；错误、过时或仅由截图推断的段落直接删除或改为未验证。
- 不恢复已经删除的旧页面文件、旧截图或 `src/pages/...` 实现路径。
- 文档变化完成后至少执行 `git diff --check`，并确认没有修改本次范围之外的 Markdown、Rust 或资源文件。

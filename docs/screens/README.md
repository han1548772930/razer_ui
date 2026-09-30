# 各界面文档

每个界面一份，写清它的**布局**（分区与布局类名）与**功能项**（雷云真实文案）。
全部取自雷云自己的设备模块与语言包，未做臆测。

## 先读这两份

| 文档 | 内容 |
|---|---|
| **[00-app-shell.md](00-app-shell.md)** | **整体骨架**：应用外壳 / 顶栏三区 / Dashboard / 应用设置 / 设备页两级模型，**以及本项目当前偏差清单** |
| **[00-visual-system.md](00-visual-system.md)** | 视觉规范：从设备模块 CSS 全量提取的颜色 / 圆角 / 字号 / 边框 / 阴影 / 过渡 / 各状态 |

重新生成：

```
node .ref/tools/gen-screen-docs.js                    # 本目录各界面文档 + 本索引
node .ref/tools/visual-system.js .ref/devices/777     # 视觉规范
# 00-app-shell.md 综合多份证据手写，用下面两条核对：
node .ref/tools/audit-css.js ".ref/frontend/static/css/55.a5b041a2.chunk.css" main-container nav-tabs side-navigation
node .ref/tools/grep-css.js  ".ref/frontend/static/css/4130.6bdf8dd0.chunk.css" "DeviceCard_deviceCard" 4
```

| 界面 | key | 出现设备数 | 功能项 | 本项目实现 |
|---|---|---:|---:|---|
| [自定义](01-customize.md) | `TAB_CUSTOMIZE` | 3 | 192 | src/pages/customize.rs |
| [性能](02-performance.md) | `TAB_PERFORMANCE` | 2 | 52 | src/pages/performance.rs |
| [正在配对](03-pairing.md) | `TAB_PAIRING` | 1 | 68 | src/pages/pairing.rs |
| [校准](04-calibration.md) | `TAB_CALIBRATION` | 2 | 74 | src/pages/calibration.rs |
| [电源](05-power.md) | `TAB_POWER` | 3 | 90 | src/pages/power.rs |
| [滚动](06-scrolling.md) | `TAB_SCROLLING` | 2 | 55 | src/pages/calibration.rs |
| [灯光](07-lighting.md) | `TAB_LIGHTING` | 2 | 131 | src/pages/lighting.rs |
| [声音](08-sound.md) | `TAB_SOUND` | 1 | 264 | src/pages/calibration.rs |
| [麦克风](09-mic.md) | `TAB_MIC` | 1 | 106 | src/pages/calibration.rs |

## 按设备看标签页

标签页是**按设备下发**的：每个产品一份独立 React 应用，各自声明自己显示哪些页。

| 设备 | productId | 类型 | 标签页（模块内声明顺序） | 页数 |
|---|---|---|---|---:|
| Razer DeathAdder V3 Pro | 182 | 鼠标 | 自定义 · 性能 · 正在配对 · 校准 · 电源 · 滚动 | 6 |
| Blackwidow V4 Pro | 653 | 键盘 | 自定义 · 性能 · 灯光 · 电源 · 滚动 | 5 |
| RAZER KRAKEN BT SANRIO LIMITED EDITION | 777 | 耳机 | 自定义 · 灯光 · 校准 · 电源 · 声音 · 麦克风 | 6 |

## 证据来源

- **布局**：设备模块 CSS 里的语义类名（`.ref/devices/<id>/`），哈希类名已剔除
- **文案**：雷云语言包（`locales/zh-CN.json`），中文为原文
- **设备归属**：设备模块 `main.js` 里 `TAB_*` 常量的声明块
- **设备模块清单**：`.ref/notes/razer-products*.txt`（共 200 个）

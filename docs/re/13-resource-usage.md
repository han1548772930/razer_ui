# 资源使用与未接入清单

本页区分原始文件、导入资源、注册资源和实际页面消费者。仅依据文件名搜索无法判断是否使用：产品图按 product/edition/layout 动态解析，映射图标、灯效方向和 DPI 图标也由状态拼接路径。`embedded.rs` 能加载一个文件，不代表界面已展示它。

统计快照：2026-10-01，新取得 Settings 前端资源之前。后续新接入的资源在文末更新，不能将本快照当作整个工作区持续不变的文件总数。

## 原始资源的范围

| 范围 | 文件数 | 内容 |
| --- | ---: | --- |
| `.ref` 全部 | 9872 | 原页面、Electron ASAR、依赖、源码映射、日志及提取工具等 |
| `.ref/frontend` | 307 | 前端文件，其中 static/media 154 项 |
| `.ref/devices/182` | 529 | 鼠标文件，其中 static/media 408 项 |
| `.ref/devices/653` | 664 | 键盘文件，其中 static/media 449 项 |
| `.ref/devices/777` | 425 | 耳机文件，其中 static/media 376 项 |
| `.ref/synapse-asar` | 7641 | Electron 及依赖，包含 2844 个 `.map`、3278 个 `.js`；不是 7641 张界面图片 |
| `.ref/notes` / `.ref/tools` / 原 `.ref/settings` | 243 / 62 / 1 | 记录、提取工具及原设置数据 |

四个 UI `static/media` 目录合计 **1387 个文件**：1283 SVG、102 AVIF、2 WAV。按 SHA-256 去重后为 **500 份内容**。单目录分别为：

| 目录 | SVG | AVIF | WAV | 合计 | 独立内容 |
| --- | ---: | ---: | ---: | ---: | ---: |
| frontend | 146 | 8 | 0 | 154 | 153 |
| 182 | 382 | 26 | 0 | 408 | 403 |
| 653 | 397 | 52 | 0 | 449 | 444 |
| 777 | 358 | 16 | 2 | 376 | 372 |

大量通用 SVG 同时出现在三个设备包。已从 653 导入 `install_chroma` 时，不需要再将 frontend/777 的相同文件各接一次。Webpack JS/CSS、字体和开发依赖也不能作为“还缺一个图片界面”的数量。

## 已导入 160 项的消费链

[manifest.json](../../assets/synapse/manifest.json) 是提取子集，含 **67 PNG、69 SVG、4 TTF、20 JSON**。来源有 137 个不同路径；其中 128 个是四 UI media 目录的原始文件，其余为 JS/CSS 提取、字体转换和 Dashboard 来源。不是全部原始资源。

| 状态 | 数量 | 消费入口 |
| --- | ---: | --- |
| 当前界面直接引用图像，包括 hover/active 分支 | 35 | shell、device_pages、profile、keyboard_controls、surface 等 |
| 动态映射/方向/DPI 图像 | 32 | `mapping_editor` 15 分类、`device_pages` 12 方向状态、`sensitivity` 5 阶段 |
| 动态产品与底部图像，扣除直接引用 | 28 | `resources::resolve_device_image` → `surface::product_image`、Customize 设备图 |
| 字体 | 4 | `main` → `resources::register_fonts` |
| 当前运行期读取的布局 JSON | 17 | 16 份 Customize `groupList` → `keyboard-sources.rs`，以及 `keyboard-653-layouts.json` |
| 已导入但没有当前界面消费者的图像 | 41 | 下表；包括分辨率备用、状态图层和未接入图标 |
| 保留作提取/追溯的 JSON | 3 | deviceconfig、旧 keys、customize-layouts 汇总 |
| 合计 | **160** | 116 项有当前消费者，44 项无当前消费者 |

这里的“消费者”表示代码路径可达，不表示用户已打开全部页面或所有 edition/layout 都有已连接硬件。未运行程序或原包来推断显示结果。

### 已用图像明细

- 直接引用 35 项：`calibration-close{,-active}`、`chroma-sync`、`dashboard-182.png`、`dashboard-add`、`dpi-draggable`、`drawer{,-active,-close}`、`eq-reset{,-hover,-active}`、`expand`、`external-link`、`help-{active,default,hover,less,more}`、`history-{back,forward}`、`install-chroma.png`、`keyboard-653-dial.png`、`palette-none`、`profile{,-more}`、`sensitivity-xy{,-active,-disabled}`、`settings`、`stepper-{down,up}`、`stream-777.png`、`synapse`、`windows-11`。无扩展名项目均为 SVG。
- 动态 32 项：`mapping-{brightness,default,keyboard,mouse,sensitivity,macro,interdevice,profile,hypershift,launch,multimedia,windows,text,disable}.svg` 与 `mapping-lighting.png`；`direction-{left,right,cw,ccw,out,in}{,-active}.svg`；`stage-{1,2,3,4,5}.svg`。不能将这些文件因缺少完整字符串字面量而判为未用。
- `product-images.rs` 的 **56 行是身份/部件映射记录，不是 56 个独立且全部已展示的图像**。当前 Product/MouseBottom 的调用会选择 28 份独立图像；支持 edition 与 16 种键盘布局。`KeyboardWrist`/`KeyboardDial` 虽在表中，但尚无界面调用，`Streamer` 当前由页面直接引用。
- `keyboard-653-layouts.json` 中 1901 个精确形状用于绘制和命中；16 份 groupList 中的 1965 个完整输入用于抽屉，包括每布局 4 个无几何拨轮输入。JSON 不是闲置图片。

### 没有当前图像消费者的 41 项

| 分组 | 数量 | 文件与原用途 | 判定 |
| --- | ---: | --- | --- |
| 腕托/多媒体滚轮图层 | 4 | `keyboard-653-wrist.png`、`keyboard-653-wrist-white.png`、`keyboard-653-roller.png`、`keyboard-653-roller-white.png` | 已有 resolver 映射但没有页面调用；原 `km` 依赖连接/输入状态。不能直接常驻显示并宣称连接了腕托 |
| 182 低分辨率备用 | 9 | `product-182-*-1x.*.png` | 3 张底部 + 6 张正面；当前主动选择 3x，不是缺 9 个页面 |
| 653 低分辨率/通用备用 | 20 | `product-653-*-1x.*.png` 18 张，以及 `product-653-prd.4221e2bc.png`、`product-653-prd.5fbaa05f.png` | 当前使用 layout 的 3x 图片；通用 prd 与布局图并存 |
| 777 低分辨率备用 | 1 | `product-777-prd-1x.5cd1e5ea.png` | 当前选 3x |
| Command Dial 说明图 | 1 | `keyboard-653-dial-mapping.svg` | 原 `.knob-item .icon-info .tip .des-3`，未接当前说明入口 |
| Chroma Studio 图标 | 1 | `chroma-studio.svg` | 高级灯效页面还未使用，当前只有 install_chroma 插图 |
| 系统旧图标 | 1 | `windows.svg` | 当前系统入口使用原 `windows-11.svg` |
| 按钮 hover 状态 | 2 | `expand-hover.svg`、`dashboard-add-hover.svg` | 导入但没有状态消费者；当前基本图标已经显示 |
| Dashboard 拖动 | 1 | `dashboard-drag.svg` | 当前仅 DPI 用同源 `dpi-draggable.svg`；Dashboard 别名没有消费者 |
| 旧 Chroma 区域图 | 1 | `keyboard-653-layout-1.svg` | 来自 965 chunk 的 Chroma 区域，不能替换 Customize 精确命中图；Studio 编辑器尚未实现 |

以上文件均在 `assets/synapse/`。不建议为提高“使用率”去渲染备用资源或删除原始出处。

### 172 个 assets 文件为何不等于 160

另外 12 个文件：4 个自有窗口按钮 SVG、原 Dashboard PNG、2 份 `assets/locale` JSON、3 份生成 Rust 表、manifest 及 product-image-map。4 个窗口 SVG 未接入；当前 title bar 用 Kit 的 Minus/Square/X 图标。Dashboard PNG 是转换来源；Rust 表是加载/解析链；manifest/product-image-map 是追溯表。实际 i18n 宏读取 `locales/`，`assets/locale/` 两份是来源副本，不能算额外的运行期翻译表。

## 未导入原资源与页面缺口的关系

| 原资源组 | 原页面/证据 | 当前结论 |
| --- | --- | --- |
| 653 `digital-dial.1e701318.avif` | main `Em` 的 `.custom-mode .digital-dial`；CSS 为 64×64、3px 彩色圆边 | 自定义模式已能编辑，但使用了另一张 `dial-3x-1-2x`；应换成原插图 |
| `icon_obm*`、`profile_1…4`、`profile_factory` | 653 共享 Profile 的 `renderOBM`；653 hasOBM 受 BLE 能力控制，182/777 明确 false | 板载入口/槽位展示未完整接入。可补说明入口；真实槽位值、同步中/成功/错误必须来自读取结果 |
| 三种 `hyperspeed-dongle*.avif`、配对图标 | frontend 配对入口与设备共用资源 | 配对交互属于独立流程，不是每个设备包各缺三页；真实扫描/配对状态仍需服务 |
| `card_icon`、`installation_cover_placeholder`、`discover-more-razer-apps`、`install_sensahd` | Modules/安装/发现应用的公用资源，多个包重复 | 对应应用目录/安装状态和 Sensa HD 入口，应按实际页面功能接入，不按副本数计量 |
| `icon_audio_profile_*`、`icon_audio_enhancement_thx` | 777 音频/EQ/增强功能 | 部分本地控件已存在而图标未使用；是否需要当前产品分支必须同时查调用链，不能仅按文件名增加功能 |
| 777 其他 `prd-*`、stream 1x/2x | 通用媒体集合 | 当前 777 webpack product context 的已解析分支使用导入的图，不能把所有 prd 文件都擅自当作该耳机 edition |

来源：[04 原资源索引](04-resource-index.md)、[07 页面覆盖](07-page-coverage.md)、[Customize 调用链](../screens/01-customize.md)、[资源 resolver](../../src/resources.rs)、[资源生成器](../../tools/prepare-resources.py)。本审计未执行原 JS、应用、测试或 DLL。

## 本轮接入后的更新

进行中：Command Dial 原插图/说明入口、已有设备页资源接入及新 Settings 来源将另列实际增量。上文为接入前的可复核快照。

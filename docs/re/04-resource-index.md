# 资源索引：实际引用、变体和本地缺口

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](20-current-source-version.md)为准。

## Rust 已打包资源（重构后补充）

2026-10-02 复核：当前 [manifest.json](../../assets/synapse/manifest.json) 包含 **234 条源/输出记录**（94 PNG、116 SVG、4 TTF、20 JSON），其中 **191 项有当前生产代码消费者**，43 项没有当前消费者。源文件来自 `.ref` 原始包、原 JS/CSS 的静态内嵌数据，以及原代码指向的精确下载 URL。每条记录保留源路径及源/输出 SHA-256；位图另有像素尺寸和实际色彩模式。下文原始库存表保留此前快照，Settings、Gamer Room、Dashboard、配对及映射选中态的当前消费者见[资源使用审计](13-resource-usage.md)。同步依据是原版代码、CSS 与资源引用，旧截图不作为依据；本轮样式使用方式见 [UI 样式复核](06-style-source-audit.md)。

[prepare-resources.py](../../tools/prepare-resources.py) 从同一份记录生成清单和 [embedded.rs](../../assets/synapse/embedded.rs)，[resources.rs](../../src/resources.rs) 的 `SynapseAssets` 直接包含该嵌入表，并保留 GPUI Kit 通用资源回退。资源已编译进程序，运行时无需读取 `.ref`；重新生成也不会覆盖原始 `.ref`。转换依赖见 [requirements-resources.txt](../../tools/requirements-resources.txt)。

当前打包范围与接入状态：

- 182 edition 0、128–132 的正面与底部 DPI 图，653 edition 0/128 的 layout 1–12、15–18 及 edition 129/130 的 layout 1 图，777 edition 0 产品图均已按原 Webpack 请求打包。`device_image(pid, edition_id, layout_id, part)` 优先选择精确变体；依据原 `wp/pM` 的 catch 分支，缺 edition 请求时回退到 edition 0 的相同 layout。182/777 忽略 layout。653 的原 `km.render` 调用 `MM/pM` 时明确传入 `layoutId: this.props.layoutId || 1`，因此旧配置缺字段形成的 layout 0 按默认 layout 1 处理。演示 PID 9001 映射到 653，演示设备明确声明 layout 1。
- 对 999 等没有已知图片的非零键盘 layout，本项目额外显示 layout 1 通用预览：先取原 edition 的 layout 1，再取 edition 0/layout 1；该行为是本项目对未识别布局的补充，原 `pM` 没有这条回退。`resolve_device_image(...)` 返回实际 `asset/edition_id/layout_id` 与 `is_fallback_preview`：默认 0→1 不是未知预览，非零未知布局回退则标为预览。UI 对全部 16 种已知布局使用各自命中区域；预览图不启用按键命中或映射，也不声明设备的真实布局。
- 原请求的 1x/3x 位图均保留并转为透明 PNG；运行时选 3x。653 腕托与 roller 白色分支只对应 edition 130；API 的 `KeyboardDial` 当前指 roller。Command Dial 自定义模式使用原 `Em` 的 `keyboard-653-digital-dial.png`；说明入口使用独立圆盘 `keyboard-653-dial.png` 与原指示灯图标。腕托/roller 的显示仍需要真实连接或输入状态，资源准备不代表页面伪造该状态。
- Dashboard 独立使用原 `PluginImages/*_dashboard3x.avif` 图片，已从精确 URL 下载 182 的 6 个 edition、653 的 34 个 edition/layout 组合及 777 标准图，共 41 个身份、19 张去重 PNG。[dashboard-image-map.json](../../assets/synapse/dashboard-image-map.json) 保留 URL、原 AVIF 与输出哈希；[dashboard-182.png](../../assets/synapse/dashboard-182.png) 也由本次原 AVIF 重新转换。[历史下载文件](../../assets/mouse-182-dashboard3x.png) 仅保留作追溯。`dashboard_image` 对键盘 layout 0 使用 1，未知明确 edition/layout 返回无图；主页面及配对页已接入，不使用 Customize 图片替代。
- 653 Chroma 区域 SVG/配置、16 种 Customize 布局的原始字段及 1901 个精确 path/circle/rect 形状；每布局另保留 4 个无几何拨轮子输入。Command Dial mapping 和 Chroma Studio 图标保留原 SVG。Chroma 区域与 Customize 命中几何分别取自原代码，不能互换。Bezier 控制点由[几何解析器](../../tools/keyboard_geometry.py)保留，绘制和命中使用同一数据。
- Sound/Mic EQ Reset 的 default/hover/active 三态 SVG，历史左右箭头、设置、Profile 与 Synapse 标志已同步并被界面引用；这些 SVG 直接复制原内容。
- 帮助图标源 `icon_help.377359c3.svg` 是通过 `#default/#hover/#active` 选择视口的 sprite。生成器保留原路径，为三个输出分别设置源 `<view>` 对应的根 `viewBox` 和 26×26 尺寸，界面按悬停/按下状态切换，避免把整张精灵作为单个图标渲染。
- 抽屉 normal/active 两态及原 `icon_closepanel` 已接入输入列表；14 枚原 `icon_config_*` 分类 SVG 与原 CSS 内嵌灯光 PNG 各自的默认/选中态用于映射分类，映射面板关闭按钮使用独立的 `mapping-close.svg`。宏/跨设备/Chroma 服务尚不可用，分类资源存在不表示服务已完成。
- 帮助页外链图标直接同步原 SVG，更多/更少箭头从 182 main 的内联 `Ed/qR` 提取原 path，保留20×20视口及currentColor，见[帮助规格](../screens/11-help.md)。
- 主前端 Add 默认/hover、Dashboard 拖动、通用展开箭头默认/hover 均已同步原 SVG。主前端 `55` CSS 与设备目录使用同名文件，此处直接同步 `.ref/devices/182` 的文件：`.box_img_2:before` 使用 18×18 Add；Dashboard collapse 的箭头显示 10px，原版拖动图在 22×19 容器内旋转 90°。当前拖动图尚无消费者。Select 使用同一默认展开箭头，展开时旋转 180°；默认箭头为 `#999`，hover 版为 `#fff`。
- 182 Performance 的 Windows/Windows 11 图标、Sensitivity XY 默认/active/disabled 三态及五个阶段红/绿/蓝/青/黄三角已同步。Windows 图来自 `.img-text .windows/.windows-11` 的媒体引用，原显示 44×44、右边距 20；不是 `um` 中的内嵌 SVG，`um → Nm` 实际只渲染对应 CSS 类。
- Smart Tracking 介绍关闭图标的白/绿两态已同步；原 CSS 的关闭区域为 36×36，图标显示 20px，hover/active 共用绿色。它属于介绍提示，不表示存在表面扫描流程。
- Lighting 的 `install_chroma.85d4bc96.avif` 转为原尺寸 520×180 的 `install-chroma.png`；同步图标、左右/顺逆时针/向内向外方向的普通及 active SVG、调色板 None SVG 共 15 项已纳入生成和嵌入表。目标名称与当前 Lighting 页面引用一致。
- Roboto Regular/Medium/Bold、RazerF5 Regular 从原 WOFF2 转为 TTF，并在启动时注册。设备媒体目录没有这些字体，继续使用上面的 `synapse-asar` 字体源；转换保留源字体时间戳，使输出可复现。

[product-image-map.json](../../assets/synapse/product-image-map.json) 保存从原 Webpack context 字面量及媒体导出模块提取的 **98 个请求**，每个请求记录原 main/chunk 路径、模块 ID 和 SHA-256；[product-images.rs](../../assets/synapse/product-images.rs) 生成 **56 条产品/edition/layout/部件解析项**。相同源文件只转换一次；不会用相似文件名猜测不存在的变体，也不把预览回退伪装成原 Webpack 请求。当前 Customize 和声卡产品区域已通过此 resolver 选择图片；653 已知 layout 的图片、命中几何、抽屉与编辑器能力均按同一布局解析。未知 layout 只显示通用预览。

重新生成资源的顺序是 `node tools/extract-keyboard.cjs`、`.work/resource-env/Scripts/python.exe tools/prepare-resources.py`；后者使用现有资源环境，离线转换已下载的源文件。配对 SVG 的静态提取出处另存 `assets/synapse/pairing-manifest.json`，生成器核对源/输出哈希后并入总表。校验使用仅依赖 Python 标准库的 [validate-resources.py](../../tools/validate-resources.py)，检查源/输出 SHA-256、PNG 文件头与尺寸、SVG 根元素及帮助视口、嵌入表完整性、每个原请求的模块与媒体导出证据、Dashboard 下载映射、Rust 索引一致性和键盘输入身份。本轮资源增量已通过静态提取/转换重新生成并校验：234 条资源记录、98 个原请求、56 条 Customize 解析项、41 个 Dashboard 身份、16 布局 / 1901 个命中形状；不执行原 JS、测试、应用或 DLL。此前 `node tools/extract-keyboard.cjs --check` 的结果仍适用于本轮未变更的原布局输入。

默认布局更正后没有重新生成媒体文件；验证器额外核对原 `layoutId || 1` 字面量、四个已知 edition 的 layout 1 预览资源及原请求表没有伪造 layout 0/999，复验通过。源码可通过 `node .work/audit-query.cjs 653 pM 'km#render'` 复查：653 main 中 pM 的 UTF-16 范围为 7811915–7815102，km class 为 7867751–7874954。

**几何更正：**965 chunk 的 SVG_PRODUCT / DEVICECONFIG 是 Chroma 区域图（960×360）。Customize 使用 730×340，布局 1 的 groupList 来自 653 main 模块 21368，其余布局由模块 30387 的 `me` 按模块 13254 的 `rE` 枚举选择。[extract-keyboard.cjs](../../tools/extract-keyboard.cjs) 只提取 AST 字面量；[布局来源索引](../../assets/synapse/keyboard-653-customize-layouts.json) 保存 16 个布局的模块和范围，[完整命中集合](../../assets/synapse/keyboard-653-layouts.json) 保存稳定 inputID、能力列表和路径。[布局 1 原始字段](../../assets/synapse/keyboard-653-customize.json) 与 [布局 1 的 118 个形状](../../assets/synapse/keyboard-653-keys.json) 是兼容保留的子集，不能代指所有布局。不能把 Chroma SVG 直接叠在 Customize 产品图上。

| 布局编号 | 每布局命中形状数 | 每布局完整输入数 |
|---|---|---|
| 1、2、5、8、9、11 | 118 | 122 |
| 3、4、6、7、10、15、16、17、18 | 119 | 123 |
| 12（日语） | 122 | 126 |
| 合计 16 布局 | 1901 | 1965 |

上表计数按布局相加，同名 inputID 在不同布局中分别保留；1965 不是单个键盘的按键数。完整输入数比命中形状数每布局多 4，供抽屉和映射编辑器使用。

## 1. 如何使用本表

审计日期：2026-09-30。以设备 Webpack context、模块中的 media 引用、CSS、asset-manifest 和文件存在性核对。**原始请求名与实际文件名可能不同**：例如 PNG 请求构建为 AVIF。表中的 module ID 和 chunk 可回到原始 JS 查证。

“文件在包内”“在当前页面实际渲染”“已被 Rust 引用”是三个不同结论。这里为已审计页面列出所需资源与变体，另把共有/未启用资源单独说明，不因图标存在而新增功能页。

所有路径相对本文。缺失文件只写代码路径，不生成伪有效链接。同名候选不等于已经恢复原路径；需要实际补齐并核对内容后才能更新状态。

## 2. 下载情况

| 范围 | 当前 static/media | 非 source-map 清单缺失 | 缺失 source maps |
|---|---|---|---|
| frontend | 154：146 SVG、8 AVIF | 273；189 在其它目录有同名候选、84 在 `.ref` 和 `assets` 中未找到 | 123 |
| 182 | 408：382 SVG、26 AVIF | sw.html | 99 |
| 653 | 449：397 SVG、52 AVIF | sw.html | 128 |
| 777 | 376：358 SVG、16 AVIF、2 WAV | sw.html | 21 |

计数对 asset-manifest 的路径去重、排除 .map。设备已声明的 JS/CSS/media 均在本地，缺 sw.html 不等于缺产品图片；source map 是调试材料，不能计为可见图片缺失。主前端缺口明细在本文末尾。

动态产品 PluginImages、网络营销数据、安装后下载资源不必全部出现在上述 manifest，不能由“设备 manifest 完整”推出所有远程资源完整。

## 3. 字体和窗口图标

原字体目录：`.ref/host-4.0.827/electron/assets/fonts/`。Roboto 用于正文/控件；部分主应用组标题使用 RazerF5。RazerF5-Ligth 是文件的实际拼写，不应按英文纠正路径。WOFF 和 WOFF2 都存在。

| 字体 | WOFF | WOFF2 |
|---|---|---|
| RazerF5-Bold | [RazerF5-Bold.woff](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-Bold.woff) | [RazerF5-Bold.woff2](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-Bold.woff2) |
| RazerF5-BoldItalic | [RazerF5-BoldItalic.woff](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-BoldItalic.woff) | [RazerF5-BoldItalic.woff2](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-BoldItalic.woff2) |
| RazerF5-Ligth | [RazerF5-Ligth.woff](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-Ligth.woff) | [RazerF5-Ligth.woff2](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-Ligth.woff2) |
| RazerF5-RegItalic | [RazerF5-RegItalic.woff](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-RegItalic.woff) | [RazerF5-RegItalic.woff2](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-RegItalic.woff2) |
| RazerF5-Regular | [RazerF5-Regular.woff](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-Regular.woff) | [RazerF5-Regular.woff2](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-Regular.woff2) |
| RazerF5-SemiBold | [RazerF5-SemiBold.woff](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-SemiBold.woff) | [RazerF5-SemiBold.woff2](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-SemiBold.woff2) |
| RazerF5-Thin | [RazerF5-Thin.woff](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-Thin.woff) | [RazerF5-Thin.woff2](../../.ref/host-4.0.827/electron/assets/fonts/RazerF5-Thin.woff2) |
| Roboto-Bold | [Roboto-Bold.woff](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Bold.woff) | [Roboto-Bold.woff2](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Bold.woff2) |
| Roboto-BoldItalic | [Roboto-BoldItalic.woff](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-BoldItalic.woff) | [Roboto-BoldItalic.woff2](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-BoldItalic.woff2) |
| Roboto-Italic | [Roboto-Italic.woff](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Italic.woff) | [Roboto-Italic.woff2](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Italic.woff2) |
| Roboto-Light | [Roboto-Light.woff](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Light.woff) | [Roboto-Light.woff2](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Light.woff2) |
| Roboto-LightItalic | [Roboto-LightItalic.woff](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-LightItalic.woff) | [Roboto-LightItalic.woff2](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-LightItalic.woff2) |
| Roboto-Medium | [Roboto-Medium.woff](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Medium.woff) | [Roboto-Medium.woff2](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Medium.woff2) |
| Roboto-MediumItalic | [Roboto-MediumItalic.woff](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-MediumItalic.woff) | [Roboto-MediumItalic.woff2](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-MediumItalic.woff2) |
| Roboto-Regular | [Roboto-Regular.woff](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Regular.woff) | [Roboto-Regular.woff2](../../.ref/host-4.0.827/electron/assets/fonts/Roboto-Regular.woff2) |

字体样式入口：[Roboto.css](../../.ref/host-4.0.827/electron/assets/style/Roboto.css)。当前 Rust 已通过 `resources::register_fonts` 注册上述四份 TTF，不能仅靠设置 `font_family` 判断其它字重也已接入；GPUI 格式兼容性见 [GPUI Kit 映射](05-gpui-kit-mapping.md)。

窗口/标签图标完整状态表见 [壳层资源](../screens/00-app-shell.md)：minimize、maximize、restore、窗口 close、活动标签 close、左右箭头的 normal/hover/active/disabled、wordmark、spinner。不要把窗口关闭图和映射面板关闭图混用。

## 4. 产品图片请求映射

下面保留原版 request；重复 context 对相同 request/模块/文件的记录合并。1x/3x 决定清晰度，布局尺寸由 CSS 控制。共享包里的其它产品图不能默认用于当前产品。

### 182：鼠标正面、底部按钮及 edition

用于 Customize 的 zp / 底部 DPI 分支。不同 edition 复用同一底部图也属于实际映射，不能根据文件名强行生成不存在的新文件。

| 原始 request | 模块 / 文件 | 实际资源（存在） |
|---|---|---|
| `./182_0/img_prods/litat1-profile-button-1x.png` | 3712 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-1x.ba4ac8e1.avif](../../.ref/devices/182/static/media/litat1-profile-button-1x.ba4ac8e1.avif) |
| `./182_0/img_prods/litat1-profile-button-3x.png` | 4986 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-3x.99ffa7e1.avif](../../.ref/devices/182/static/media/litat1-profile-button-3x.99ffa7e1.avif) |
| `./182_0/img_prods/prd-1x.png` | 48 / [48.7e4eb4db.chunk.js](../../.ref/devices/182/static/js/48.7e4eb4db.chunk.js) | [prd-1x.cb2eedcf.avif](../../.ref/devices/182/static/media/prd-1x.cb2eedcf.avif) |
| `./182_0/img_prods/prd-3x.png` | 8922 / [8922.8bdb77be.chunk.js](../../.ref/devices/182/static/js/8922.8bdb77be.chunk.js) | [prd-3x.674b18f0.avif](../../.ref/devices/182/static/media/prd-3x.674b18f0.avif) |
| `./182_128/img_prods/litat1-profile-button-1x.png` | 1477 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-1x.51af3998.avif](../../.ref/devices/182/static/media/litat1-profile-button-1x.51af3998.avif) |
| `./182_128/img_prods/litat1-profile-button-3x.png` | 5179 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-3x.19382cc9.avif](../../.ref/devices/182/static/media/litat1-profile-button-3x.19382cc9.avif) |
| `./182_128/img_prods/prd-1x.png` | 3125 / [3125.b2af78bc.chunk.js](../../.ref/devices/182/static/js/3125.b2af78bc.chunk.js) | [prd-1x.a8c0cadd.avif](../../.ref/devices/182/static/media/prd-1x.a8c0cadd.avif) |
| `./182_128/img_prods/prd-3x.png` | 7915 / [7915.50aec7db.chunk.js](../../.ref/devices/182/static/js/7915.50aec7db.chunk.js) | [prd-3x.fa151ada.avif](../../.ref/devices/182/static/media/prd-3x.fa151ada.avif) |
| `./182_129/img_prods/litat1-profile-button-1x.png` | 8110 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-1x.fc69b02f.avif](../../.ref/devices/182/static/media/litat1-profile-button-1x.fc69b02f.avif) |
| `./182_129/img_prods/litat1-profile-button-3x.png` | 8100 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-3x.d3439e26.avif](../../.ref/devices/182/static/media/litat1-profile-button-3x.d3439e26.avif) |
| `./182_129/img_prods/prd-1x.png` | 6378 / [6378.75912685.chunk.js](../../.ref/devices/182/static/js/6378.75912685.chunk.js) | [prd-1x.bbacb213.avif](../../.ref/devices/182/static/media/prd-1x.bbacb213.avif) |
| `./182_129/img_prods/prd-3x.png` | 2560 / [2560.ac5d57b2.chunk.js](../../.ref/devices/182/static/js/2560.ac5d57b2.chunk.js) | [prd-3x.d6a100a5.avif](../../.ref/devices/182/static/media/prd-3x.d6a100a5.avif) |
| `./182_130/img_prods/litat1-profile-button-1x.png` | 3452 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-1x.ba4ac8e1.avif](../../.ref/devices/182/static/media/litat1-profile-button-1x.ba4ac8e1.avif) |
| `./182_130/img_prods/litat1-profile-button-3x.png` | 3094 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-3x.99ffa7e1.avif](../../.ref/devices/182/static/media/litat1-profile-button-3x.99ffa7e1.avif) |
| `./182_130/img_prods/prd-1x.png` | 1588 / [1588.0cd516df.chunk.js](../../.ref/devices/182/static/js/1588.0cd516df.chunk.js) | [prd-1x.d9ca8c5e.avif](../../.ref/devices/182/static/media/prd-1x.d9ca8c5e.avif) |
| `./182_130/img_prods/prd-3x.png` | 8238 / [8238.54f5819c.chunk.js](../../.ref/devices/182/static/js/8238.54f5819c.chunk.js) | [prd-3x.d9ca8c5e.avif](../../.ref/devices/182/static/media/prd-3x.d9ca8c5e.avif) |
| `./182_131/img_prods/litat1-profile-button-1x.png` | 4555 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-1x.ba4ac8e1.avif](../../.ref/devices/182/static/media/litat1-profile-button-1x.ba4ac8e1.avif) |
| `./182_131/img_prods/litat1-profile-button-3x.png` | 7349 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-3x.99ffa7e1.avif](../../.ref/devices/182/static/media/litat1-profile-button-3x.99ffa7e1.avif) |
| `./182_131/img_prods/prd-1x.png` | 4663 / [4663.07e4df00.chunk.js](../../.ref/devices/182/static/js/4663.07e4df00.chunk.js) | [prd-1x.43d579a8.avif](../../.ref/devices/182/static/media/prd-1x.43d579a8.avif) |
| `./182_131/img_prods/prd-3x.png` | 6577 / [6577.87d0f9cf.chunk.js](../../.ref/devices/182/static/js/6577.87d0f9cf.chunk.js) | [prd-3x.43d579a8.avif](../../.ref/devices/182/static/media/prd-3x.43d579a8.avif) |
| `./182_132/img_prods/litat1-profile-button-1x.png` | 7254 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-1x.ba4ac8e1.avif](../../.ref/devices/182/static/media/litat1-profile-button-1x.ba4ac8e1.avif) |
| `./182_132/img_prods/litat1-profile-button-3x.png` | 4460 / [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [litat1-profile-button-3x.99ffa7e1.avif](../../.ref/devices/182/static/media/litat1-profile-button-3x.99ffa7e1.avif) |
| `./182_132/img_prods/prd-1x.png` | 6530 / [6530.c3315102.chunk.js](../../.ref/devices/182/static/js/6530.c3315102.chunk.js) | [prd-1x.02f9ee63.avif](../../.ref/devices/182/static/media/prd-1x.02f9ee63.avif) |
| `./182_132/img_prods/prd-3x.png` | 3736 / [3736.4fb94caa.chunk.js](../../.ref/devices/182/static/js/3736.4fb94caa.chunk.js) | [prd-3x.f43aa7e1.avif](../../.ref/devices/182/static/media/prd-3x.f43aa7e1.avif) |

### 653：键盘 layout、white、腕托、滚轮、Command Dial

layout 为 1–12、15–18；edition 130 为 white 分支。图片要与下一节的几何/配置采用相同布局。无 PID 前缀的条目是该产品上下文导出的附属资源。

| 原始 request | 模块 / 文件 | 实际资源（存在） |
|---|---|---|
| `./653_0/img_prods/1-1x.png` | 95858 / [5858.593ed23c.chunk.js](../../.ref/devices/653/static/js/5858.593ed23c.chunk.js) | [1-1x.ab26aeb7.avif](../../.ref/devices/653/static/media/1-1x.ab26aeb7.avif) |
| `./653_0/img_prods/1-3x.png` | 60136 / [136.af63c2d3.chunk.js](../../.ref/devices/653/static/js/136.af63c2d3.chunk.js) | [1-3x.8d525695.avif](../../.ref/devices/653/static/media/1-3x.8d525695.avif) |
| `./653_0/img_prods/2-1x.png` | 41547 / [1547.176ec3d7.chunk.js](../../.ref/devices/653/static/js/1547.176ec3d7.chunk.js) | [2-1x.cfe79c6f.avif](../../.ref/devices/653/static/media/2-1x.cfe79c6f.avif) |
| `./653_0/img_prods/2-3x.png` | 71445 / [1445.b281a7b0.chunk.js](../../.ref/devices/653/static/js/1445.b281a7b0.chunk.js) | [2-3x.fb8dcb10.avif](../../.ref/devices/653/static/media/2-3x.fb8dcb10.avif) |
| `./653_0/img_prods/3-1x.png` | 44764 / [4764.9669d50d.chunk.js](../../.ref/devices/653/static/js/4764.9669d50d.chunk.js) | [3-1x.52fa5004.avif](../../.ref/devices/653/static/media/3-1x.52fa5004.avif) |
| `./653_0/img_prods/3-3x.png` | 28902 / [8902.0144cdf9.chunk.js](../../.ref/devices/653/static/js/8902.0144cdf9.chunk.js) | [3-3x.64d7b830.avif](../../.ref/devices/653/static/media/3-3x.64d7b830.avif) |
| `./653_0/img_prods/4-1x.png` | 94077 / [8839.8cf5ba7f.chunk.js](../../.ref/devices/653/static/js/8839.8cf5ba7f.chunk.js) | [4-1x.03001c8e.avif](../../.ref/devices/653/static/media/4-1x.03001c8e.avif) |
| `./653_0/img_prods/4-3x.png` | 14227 / [4227.463b329e.chunk.js](../../.ref/devices/653/static/js/4227.463b329e.chunk.js) | [4-3x.5433580e.avif](../../.ref/devices/653/static/media/4-3x.5433580e.avif) |
| `./653_0/img_prods/5-1x.png` | 76774 / [6774.525e824b.chunk.js](../../.ref/devices/653/static/js/6774.525e824b.chunk.js) | [5-1x.31d3238f.avif](../../.ref/devices/653/static/media/5-1x.31d3238f.avif) |
| `./653_0/img_prods/5-3x.png` | 94524 / [4524.1539d9e4.chunk.js](../../.ref/devices/653/static/js/4524.1539d9e4.chunk.js) | [5-3x.268ad7c5.avif](../../.ref/devices/653/static/media/5-3x.268ad7c5.avif) |
| `./653_0/img_prods/6-1x.png` | 96287 / [6287.fea6e5c8.chunk.js](../../.ref/devices/653/static/js/6287.fea6e5c8.chunk.js) | [6-1x.af2072c7.avif](../../.ref/devices/653/static/media/6-1x.af2072c7.avif) |
| `./653_0/img_prods/6-3x.png` | 90153 / [153.c0c74f63.chunk.js](../../.ref/devices/653/static/js/153.c0c74f63.chunk.js) | [6-3x.f8148117.avif](../../.ref/devices/653/static/media/6-3x.f8148117.avif) |
| `./653_0/img_prods/7-1x.png` | 41168 / [8787.a40711ef.chunk.js](../../.ref/devices/653/static/js/8787.a40711ef.chunk.js) | [7-1x.71ca569c.avif](../../.ref/devices/653/static/media/7-1x.71ca569c.avif) |
| `./653_0/img_prods/7-3x.png` | 19034 / [9034.0b7cf5d7.chunk.js](../../.ref/devices/653/static/js/9034.0b7cf5d7.chunk.js) | [7-3x.9da95ab4.avif](../../.ref/devices/653/static/media/7-3x.9da95ab4.avif) |
| `./653_0/img_prods/8-1x.png` | 79985 / [9985.862311e0.chunk.js](../../.ref/devices/653/static/js/9985.862311e0.chunk.js) | [8-1x.e90184f0.avif](../../.ref/devices/653/static/media/8-1x.e90184f0.avif) |
| `./653_0/img_prods/8-3x.png` | 81447 / [1447.398f4775.chunk.js](../../.ref/devices/653/static/js/1447.398f4775.chunk.js) | [8-3x.372ad914.avif](../../.ref/devices/653/static/media/8-3x.372ad914.avif) |
| `./653_0/img_prods/9-1x.png` | 29274 / [9274.66bacf55.chunk.js](../../.ref/devices/653/static/js/9274.66bacf55.chunk.js) | [9-1x.c1e63484.avif](../../.ref/devices/653/static/media/9-1x.c1e63484.avif) |
| `./653_0/img_prods/9-3x.png` | 25008 / [5008.7f30ee66.chunk.js](../../.ref/devices/653/static/js/5008.7f30ee66.chunk.js) | [9-3x.e379204e.avif](../../.ref/devices/653/static/media/9-3x.e379204e.avif) |
| `./653_0/img_prods/10-1x.png` | 82692 / [2692.7df07c84.chunk.js](../../.ref/devices/653/static/js/2692.7df07c84.chunk.js) | [10-1x.f4ae459c.avif](../../.ref/devices/653/static/media/10-1x.f4ae459c.avif) |
| `./653_0/img_prods/10-3x.png` | 49966 / [9966.d8fbc183.chunk.js](../../.ref/devices/653/static/js/9966.d8fbc183.chunk.js) | [10-3x.3f217b26.avif](../../.ref/devices/653/static/media/10-3x.3f217b26.avif) |
| `./653_0/img_prods/11-1x.png` | 69171 / [9171.1cf3cebb.chunk.js](../../.ref/devices/653/static/js/9171.1cf3cebb.chunk.js) | [11-1x.f9260b13.avif](../../.ref/devices/653/static/media/11-1x.f9260b13.avif) |
| `./653_0/img_prods/11-3x.png` | 92701 / [2701.071ae757.chunk.js](../../.ref/devices/653/static/js/2701.071ae757.chunk.js) | [11-3x.dfee759e.avif](../../.ref/devices/653/static/media/11-3x.dfee759e.avif) |
| `./653_0/img_prods/12-1x.png` | 85722 / [5722.02b17057.chunk.js](../../.ref/devices/653/static/js/5722.02b17057.chunk.js) | [12-1x.6c82872f.avif](../../.ref/devices/653/static/media/12-1x.6c82872f.avif) |
| `./653_0/img_prods/12-3x.png` | 7856 / [7856.907e2157.chunk.js](../../.ref/devices/653/static/js/7856.907e2157.chunk.js) | [12-3x.97e2fc23.avif](../../.ref/devices/653/static/media/12-3x.97e2fc23.avif) |
| `./653_0/img_prods/15-1x.png` | 24039 / [4039.8b4557e0.chunk.js](../../.ref/devices/653/static/js/4039.8b4557e0.chunk.js) | [15-1x.04bce260.avif](../../.ref/devices/653/static/media/15-1x.04bce260.avif) |
| `./653_0/img_prods/15-3x.png` | 33489 / [3489.1101f94b.chunk.js](../../.ref/devices/653/static/js/3489.1101f94b.chunk.js) | [15-3x.2d501bd7.avif](../../.ref/devices/653/static/media/15-3x.2d501bd7.avif) |
| `./653_0/img_prods/16-1x.png` | 17358 / [7358.88503455.chunk.js](../../.ref/devices/653/static/js/7358.88503455.chunk.js) | [16-1x.7296aa8c.avif](../../.ref/devices/653/static/media/16-1x.7296aa8c.avif) |
| `./653_0/img_prods/16-3x.png` | 66916 / [6916.aabb1b48.chunk.js](../../.ref/devices/653/static/js/6916.aabb1b48.chunk.js) | [16-3x.099bf2be.avif](../../.ref/devices/653/static/media/16-3x.099bf2be.avif) |
| `./653_0/img_prods/17-1x.png` | 95909 / [5909.ca5d8299.chunk.js](../../.ref/devices/653/static/js/5909.ca5d8299.chunk.js) | [17-1x.e408c5a0.avif](../../.ref/devices/653/static/media/17-1x.e408c5a0.avif) |
| `./653_0/img_prods/17-3x.png` | 20699 / [699.1455c195.chunk.js](../../.ref/devices/653/static/js/699.1455c195.chunk.js) | [17-3x.024b2318.avif](../../.ref/devices/653/static/media/17-3x.024b2318.avif) |
| `./653_0/img_prods/18-1x.png` | 70031 / [2412.1332569f.chunk.js](../../.ref/devices/653/static/js/2412.1332569f.chunk.js) | [18-1x.4006827f.avif](../../.ref/devices/653/static/media/18-1x.4006827f.avif) |
| `./653_0/img_prods/18-3x.png` | 88454 / [8454.9dae2729.chunk.js](../../.ref/devices/653/static/js/8454.9dae2729.chunk.js) | [18-3x.4f188968.avif](../../.ref/devices/653/static/media/18-3x.4f188968.avif) |
| `./653_0/img_prods/prd.png` | 2649 / [2649.6577f913.chunk.js](../../.ref/devices/653/static/js/2649.6577f913.chunk.js) | [prd.4221e2bc.avif](../../.ref/devices/653/static/media/prd.4221e2bc.avif) |
| `./653_128/img_prods/1-1x.png` | 34475 / [4475.d44136e6.chunk.js](../../.ref/devices/653/static/js/4475.d44136e6.chunk.js) | [1-1x.ab26aeb7.avif](../../.ref/devices/653/static/media/1-1x.ab26aeb7.avif) |
| `./653_128/img_prods/1-3x.png` | 29845 / [9845.c1e290b9.chunk.js](../../.ref/devices/653/static/js/9845.c1e290b9.chunk.js) | [1-3x.8d525695.avif](../../.ref/devices/653/static/media/1-3x.8d525695.avif) |
| `./653_128/img_prods/2-1x.png` | 74162 / [4162.96c1d7e9.chunk.js](../../.ref/devices/653/static/js/4162.96c1d7e9.chunk.js) | [2-1x.93ae7bf6.avif](../../.ref/devices/653/static/media/2-1x.93ae7bf6.avif) |
| `./653_128/img_prods/2-3x.png` | 47848 / [7848.c0d04910.chunk.js](../../.ref/devices/653/static/js/7848.c0d04910.chunk.js) | [2-3x.64d7b830.avif](../../.ref/devices/653/static/media/2-3x.64d7b830.avif) |
| `./653_128/img_prods/3-1x.png` | 56297 / [6297.762254d1.chunk.js](../../.ref/devices/653/static/js/6297.762254d1.chunk.js) | [3-1x.52fa5004.avif](../../.ref/devices/653/static/media/3-1x.52fa5004.avif) |
| `./653_128/img_prods/3-3x.png` | 83391 / [3391.95a45e72.chunk.js](../../.ref/devices/653/static/js/3391.95a45e72.chunk.js) | [3-3x.64d7b830.avif](../../.ref/devices/653/static/media/3-3x.64d7b830.avif) |
| `./653_128/img_prods/4-1x.png` | 57040 / [7040.adb12d77.chunk.js](../../.ref/devices/653/static/js/7040.adb12d77.chunk.js) | [4-1x.03001c8e.avif](../../.ref/devices/653/static/media/4-1x.03001c8e.avif) |
| `./653_128/img_prods/4-3x.png` | 26458 / [6458.2a99ea51.chunk.js](../../.ref/devices/653/static/js/6458.2a99ea51.chunk.js) | [4-3x.5433580e.avif](../../.ref/devices/653/static/media/4-3x.5433580e.avif) |
| `./653_128/img_prods/5-1x.png` | 75199 / [5199.018dd35e.chunk.js](../../.ref/devices/653/static/js/5199.018dd35e.chunk.js) | [5-1x.31d3238f.avif](../../.ref/devices/653/static/media/5-1x.31d3238f.avif) |
| `./653_128/img_prods/5-3x.png` | 64457 / [4457.c1a399f0.chunk.js](../../.ref/devices/653/static/js/4457.c1a399f0.chunk.js) | [5-3x.268ad7c5.avif](../../.ref/devices/653/static/media/5-3x.268ad7c5.avif) |
| `./653_128/img_prods/6-1x.png` | 20454 / [454.a3efa97c.chunk.js](../../.ref/devices/653/static/js/454.a3efa97c.chunk.js) | [6-1x.af2072c7.avif](../../.ref/devices/653/static/media/6-1x.af2072c7.avif) |
| `./653_128/img_prods/6-3x.png` | 24412 / [4412.d8cca8cd.chunk.js](../../.ref/devices/653/static/js/4412.d8cca8cd.chunk.js) | [6-3x.f8148117.avif](../../.ref/devices/653/static/media/6-3x.f8148117.avif) |
| `./653_128/img_prods/7-1x.png` | 9949 / [9949.0e36ea13.chunk.js](../../.ref/devices/653/static/js/9949.0e36ea13.chunk.js) | [7-1x.71ca569c.avif](../../.ref/devices/653/static/media/7-1x.71ca569c.avif) |
| `./653_128/img_prods/7-3x.png` | 70067 / [67.3184fddb.chunk.js](../../.ref/devices/653/static/js/67.3184fddb.chunk.js) | [7-3x.9da95ab4.avif](../../.ref/devices/653/static/media/7-3x.9da95ab4.avif) |
| `./653_128/img_prods/8-1x.png` | 81540 / [1540.d7430497.chunk.js](../../.ref/devices/653/static/js/1540.d7430497.chunk.js) | [8-1x.e90184f0.avif](../../.ref/devices/653/static/media/8-1x.e90184f0.avif) |
| `./653_128/img_prods/8-3x.png` | 31982 / [1982.4a63b747.chunk.js](../../.ref/devices/653/static/js/1982.4a63b747.chunk.js) | [8-3x.372ad914.avif](../../.ref/devices/653/static/media/8-3x.372ad914.avif) |
| `./653_128/img_prods/9-1x.png` | 87347 / [7347.2b3feef9.chunk.js](../../.ref/devices/653/static/js/7347.2b3feef9.chunk.js) | [9-1x.c1e63484.avif](../../.ref/devices/653/static/media/9-1x.c1e63484.avif) |
| `./653_128/img_prods/9-3x.png` | 67197 / [7197.7a7e9b15.chunk.js](../../.ref/devices/653/static/js/7197.7a7e9b15.chunk.js) | [9-3x.e379204e.avif](../../.ref/devices/653/static/media/9-3x.e379204e.avif) |
| `./653_128/img_prods/10-1x.png` | 10927 / [927.87022876.chunk.js](../../.ref/devices/653/static/js/927.87022876.chunk.js) | [10-1x.f4ae459c.avif](../../.ref/devices/653/static/media/10-1x.f4ae459c.avif) |
| `./653_128/img_prods/10-3x.png` | 64121 / [4121.c0a3f108.chunk.js](../../.ref/devices/653/static/js/4121.c0a3f108.chunk.js) | [10-3x.3f217b26.avif](../../.ref/devices/653/static/media/10-3x.3f217b26.avif) |
| `./653_128/img_prods/11-1x.png` | 31168 / [1168.d1473202.chunk.js](../../.ref/devices/653/static/js/1168.d1473202.chunk.js) | [11-1x.f9260b13.avif](../../.ref/devices/653/static/media/11-1x.f9260b13.avif) |
| `./653_128/img_prods/11-3x.png` | 47018 / [7018.b4878e5a.chunk.js](../../.ref/devices/653/static/js/7018.b4878e5a.chunk.js) | [11-3x.dfee759e.avif](../../.ref/devices/653/static/media/11-3x.dfee759e.avif) |
| `./653_128/img_prods/12-1x.png` | 84077 / [4077.2d6bab44.chunk.js](../../.ref/devices/653/static/js/4077.2d6bab44.chunk.js) | [12-1x.6c82872f.avif](../../.ref/devices/653/static/media/12-1x.6c82872f.avif) |
| `./653_128/img_prods/12-3x.png` | 59683 / [9683.961958a2.chunk.js](../../.ref/devices/653/static/js/9683.961958a2.chunk.js) | [12-3x.97e2fc23.avif](../../.ref/devices/653/static/media/12-3x.97e2fc23.avif) |
| `./653_128/img_prods/15-1x.png` | 93388 / [3388.8b5ea3bf.chunk.js](../../.ref/devices/653/static/js/3388.8b5ea3bf.chunk.js) | [15-1x.04bce260.avif](../../.ref/devices/653/static/media/15-1x.04bce260.avif) |
| `./653_128/img_prods/15-3x.png` | 98006 / [8006.b04e5b96.chunk.js](../../.ref/devices/653/static/js/8006.b04e5b96.chunk.js) | [15-3x.2d501bd7.avif](../../.ref/devices/653/static/media/15-3x.2d501bd7.avif) |
| `./653_128/img_prods/16-1x.png` | 99033 / [9033.6775f3df.chunk.js](../../.ref/devices/653/static/js/9033.6775f3df.chunk.js) | [16-1x.7296aa8c.avif](../../.ref/devices/653/static/media/16-1x.7296aa8c.avif) |
| `./653_128/img_prods/16-3x.png` | 60335 / [335.de71a4da.chunk.js](../../.ref/devices/653/static/js/335.de71a4da.chunk.js) | [16-3x.099bf2be.avif](../../.ref/devices/653/static/media/16-3x.099bf2be.avif) |
| `./653_128/img_prods/17-1x.png` | 34210 / [4210.d8f31915.chunk.js](../../.ref/devices/653/static/js/4210.d8f31915.chunk.js) | [17-1x.e408c5a0.avif](../../.ref/devices/653/static/media/17-1x.e408c5a0.avif) |
| `./653_128/img_prods/17-3x.png` | 11416 / [1416.c7139311.chunk.js](../../.ref/devices/653/static/js/1416.c7139311.chunk.js) | [17-3x.024b2318.avif](../../.ref/devices/653/static/media/17-3x.024b2318.avif) |
| `./653_128/img_prods/18-1x.png` | 74583 / [4583.32901d69.chunk.js](../../.ref/devices/653/static/js/4583.32901d69.chunk.js) | [18-1x.4006827f.avif](../../.ref/devices/653/static/media/18-1x.4006827f.avif) |
| `./653_128/img_prods/18-3x.png` | 40417 / [417.b96d7ec8.chunk.js](../../.ref/devices/653/static/js/417.b96d7ec8.chunk.js) | [18-3x.4f188968.avif](../../.ref/devices/653/static/media/18-3x.4f188968.avif) |
| `./653_128/img_prods/prd.png` | 22974 / [2974.9c5d5114.chunk.js](../../.ref/devices/653/static/js/2974.9c5d5114.chunk.js) | [prd.4221e2bc.avif](../../.ref/devices/653/static/media/prd.4221e2bc.avif) |
| `./653_129/img_prods/1-1x.png` | 83700 / [3700.e633a9d5.chunk.js](../../.ref/devices/653/static/js/3700.e633a9d5.chunk.js) | [1-1x.ab26aeb7.avif](../../.ref/devices/653/static/media/1-1x.ab26aeb7.avif) |
| `./653_129/img_prods/1-3x.png` | 3710 / [3710.e3e9e66f.chunk.js](../../.ref/devices/653/static/js/3710.e3e9e66f.chunk.js) | [1-3x.8d525695.avif](../../.ref/devices/653/static/media/1-3x.8d525695.avif) |
| `./653_129/img_prods/prd.png` | 10075 / [75.f144d8a3.chunk.js](../../.ref/devices/653/static/js/75.f144d8a3.chunk.js) | [prd.4221e2bc.avif](../../.ref/devices/653/static/media/prd.4221e2bc.avif) |
| `./653_130/img_prods/1-1x.png` | 33582 / [3582.01519583.chunk.js](../../.ref/devices/653/static/js/3582.01519583.chunk.js) | [1-1x.9d96d6ee.avif](../../.ref/devices/653/static/media/1-1x.9d96d6ee.avif) |
| `./653_130/img_prods/1-3x.png` | 96932 / [6932.0bd01882.chunk.js](../../.ref/devices/653/static/js/6932.0bd01882.chunk.js) | [1-3x.5fbaa05f.avif](../../.ref/devices/653/static/media/1-3x.5fbaa05f.avif) |
| `./653_130/img_prods/prd.png` | 49357 / [9357.495b3f1c.chunk.js](../../.ref/devices/653/static/js/9357.495b3f1c.chunk.js) | [prd.5fbaa05f.avif](../../.ref/devices/653/static/media/prd.5fbaa05f.avif) |
| `./dial-3x-1-2x.png` | 835 / [835.c5fdd848.chunk.js](../../.ref/devices/653/static/js/835.c5fdd848.chunk.js) | [dial-3x-1-2x.a7526df5.avif](../../.ref/devices/653/static/media/dial-3x-1-2x.a7526df5.avif) |
| `./icon_scrolldown_a.svg` | 68427 / [8427.1ba147c9.chunk.js](../../.ref/devices/653/static/js/8427.1ba147c9.chunk.js) | [icon_scrolldown_a.e0e1f5ba.svg](../../.ref/devices/653/static/media/icon_scrolldown_a.e0e1f5ba.svg) |
| `./icon_scrolldown.svg` | 32792 / [411.d29d3bd6.chunk.js](../../.ref/devices/653/static/js/411.d29d3bd6.chunk.js) | [icon_scrolldown.63625b62.svg](../../.ref/devices/653/static/media/icon_scrolldown.63625b62.svg) |
| `./icon_scrollup_a.svg` | 65472 / [5472.d55066f2.chunk.js](../../.ref/devices/653/static/js/5472.d55066f2.chunk.js) | [icon_scrollup_a.890fc04d.svg](../../.ref/devices/653/static/media/icon_scrollup_a.890fc04d.svg) |
| `./icon_scrollup.svg` | 95492 / [5492.184553b9.chunk.js](../../.ref/devices/653/static/js/5492.184553b9.chunk.js) | [icon_scrollup.d58ba5fc.svg](../../.ref/devices/653/static/media/icon_scrollup.d58ba5fc.svg) |
| `./roller-3x.png` | 8770 / [main.7b71cce5.js](../../.ref/devices/653/static/js/main.7b71cce5.js) | [roller-3x.7fe981b9.avif](../../.ref/devices/653/static/media/roller-3x.7fe981b9.avif) |
| `./roller-white-3x.png` | 9378 / [main.7b71cce5.js](../../.ref/devices/653/static/js/main.7b71cce5.js) | [roller-white-3x.7863b66d.avif](../../.ref/devices/653/static/media/roller-white-3x.7863b66d.avif) |
| `./wrist-without-gap-white.png` | 40567 / [567.701d3063.chunk.js](../../.ref/devices/653/static/js/567.701d3063.chunk.js) | [wrist-without-gap-white.f54a214f.avif](../../.ref/devices/653/static/media/wrist-without-gap-white.f54a214f.avif) |
| `./wrist-without-gap.png` | 27415 / [7415.89c4dd0e.chunk.js](../../.ref/devices/653/static/js/7415.89c4dd0e.chunk.js) | [wrist-without-gap.efc0505f.avif](../../.ref/devices/653/static/media/wrist-without-gap.efc0505f.avif) |

### 777：耳机 Sound 产品图

当前 777_0 的产品图在 Sound 使用，Mic 当前树没有产品图。bundle 中另有 1328/1332 变体，属于共有产品上下文，不代表新增 777 页面。

| 原始 request | 模块 / 文件 | 实际资源（存在） |
|---|---|---|
| `./777_0/img_prods/prd-1x.png` | 5298 / [298.252992aa.chunk.js](../../.ref/devices/777/static/js/298.252992aa.chunk.js) | [prd-1x.5cd1e5ea.avif](../../.ref/devices/777/static/media/prd-1x.5cd1e5ea.avif) |
| `./777_0/img_prods/prd-3x.png` | 4024 / [24.2b0388fd.chunk.js](../../.ref/devices/777/static/js/24.2b0388fd.chunk.js) | [prd-3x.ede8800a.avif](../../.ref/devices/777/static/media/prd-3x.ede8800a.avif) |

## 5. 内嵌设备几何与布局配置

这些 request 导向 JavaScript 模块，不应继续在 media 中查找同名 SVG。653 layout 1 的模块 10965 导出 **SVG_PRODUCT** 和 **DEVICECONFIG**，SVG viewBox 为 `0 0 960 360`，含按键 ID 和选择分组。需要保留交互几何，不能整图栅格化后丢掉命中区。

各产品 chromaSVG 上下文如下；表证明模块入口存在，具体字段以模块导出为准，不保证所有产品都使用相同 SVG schema。

| 182 原始 request | 模块 ID | 本地 JS |
|---|---|---|
| `./182_0/chromaSVG/0.js` | 1113 | [1113.5918a324.chunk.js](../../.ref/devices/182/static/js/1113.5918a324.chunk.js) |
| `./182_128/chromaSVG/0.js` | 904 | [904.863372f5.chunk.js](../../.ref/devices/182/static/js/904.863372f5.chunk.js) |
| `./182_129/chromaSVG/0.js` | 3831 | [3831.388052ec.chunk.js](../../.ref/devices/182/static/js/3831.388052ec.chunk.js) |
| `./182_132/chromaSVG/0.js` | 5003 | [5003.d82bdd61.chunk.js](../../.ref/devices/182/static/js/5003.d82bdd61.chunk.js) |

| 653 原始 request | 模块 ID | 本地 JS |
|---|---|---|
| `./653_0/chromaSVG/1.js` | 10965 | [965.8f16fe94.chunk.js](../../.ref/devices/653/static/js/965.8f16fe94.chunk.js) |
| `./653_0/chromaSVG/2.js` | 29388 | [9388.87365b87.chunk.js](../../.ref/devices/653/static/js/9388.87365b87.chunk.js) |
| `./653_0/chromaSVG/3.js` | 79123 | [9123.45daab1a.chunk.js](../../.ref/devices/653/static/js/9123.45daab1a.chunk.js) |
| `./653_0/chromaSVG/4.js` | 75434 | [5434.7de765f0.chunk.js](../../.ref/devices/653/static/js/5434.7de765f0.chunk.js) |
| `./653_0/chromaSVG/5.js` | 88217 | [8217.b162456b.chunk.js](../../.ref/devices/653/static/js/8217.b162456b.chunk.js) |
| `./653_0/chromaSVG/6.js` | 70816 | [816.1c8e79d7.chunk.js](../../.ref/devices/653/static/js/816.1c8e79d7.chunk.js) |
| `./653_0/chromaSVG/7.js` | 60535 | [535.4ac2b91a.chunk.js](../../.ref/devices/653/static/js/535.4ac2b91a.chunk.js) |
| `./653_0/chromaSVG/8.js` | 44094 | [4094.1915e60f.chunk.js](../../.ref/devices/653/static/js/4094.1915e60f.chunk.js) |
| `./653_0/chromaSVG/9.js` | 70365 | [365.ecce9cca.chunk.js](../../.ref/devices/653/static/js/365.ecce9cca.chunk.js) |
| `./653_0/chromaSVG/10.js` | 21625 | [1625.5d64aca9.chunk.js](../../.ref/devices/653/static/js/1625.5d64aca9.chunk.js) |
| `./653_0/chromaSVG/11.js` | 8842 | [8842.814a5d2e.chunk.js](../../.ref/devices/653/static/js/8842.814a5d2e.chunk.js) |
| `./653_0/chromaSVG/12.js` | 93943 | [3943.3d4aa5d4.chunk.js](../../.ref/devices/653/static/js/3943.3d4aa5d4.chunk.js) |
| `./653_0/chromaSVG/15.js` | 35414 | [5414.93ca2590.chunk.js](../../.ref/devices/653/static/js/5414.93ca2590.chunk.js) |
| `./653_0/chromaSVG/16.js` | 12531 | [2531.21b59e51.chunk.js](../../.ref/devices/653/static/js/2531.21b59e51.chunk.js) |
| `./653_0/chromaSVG/17.js` | 62796 | [2796.0e6d6bda.chunk.js](../../.ref/devices/653/static/js/2796.0e6d6bda.chunk.js) |
| `./653_0/chromaSVG/18.js` | 60641 | [641.448acf38.chunk.js](../../.ref/devices/653/static/js/641.448acf38.chunk.js) |
| `./653_128/chromaSVG/1.js` | 60464 | [464.c97ca79b.chunk.js](../../.ref/devices/653/static/js/464.c97ca79b.chunk.js) |
| `./653_128/chromaSVG/2.js` | 10825 | [825.1be87025.chunk.js](../../.ref/devices/653/static/js/825.1be87025.chunk.js) |
| `./653_128/chromaSVG/3.js` | 40314 | [7933.b7565252.chunk.js](../../.ref/devices/653/static/js/7933.b7565252.chunk.js) |
| `./653_128/chromaSVG/4.js` | 1475 | [1475.96cd6ef6.chunk.js](../../.ref/devices/653/static/js/1475.96cd6ef6.chunk.js) |
| `./653_128/chromaSVG/5.js` | 19836 | [9836.0fc3a53a.chunk.js](../../.ref/devices/653/static/js/9836.0fc3a53a.chunk.js) |
| `./653_128/chromaSVG/6.js` | 26789 | [6789.c06cd900.chunk.js](../../.ref/devices/653/static/js/6789.c06cd900.chunk.js) |
| `./653_128/chromaSVG/7.js` | 96070 | [6070.8fefc507.chunk.js](../../.ref/devices/653/static/js/6070.8fefc507.chunk.js) |
| `./653_128/chromaSVG/8.js` | 89007 | [9007.17a15931.chunk.js](../../.ref/devices/653/static/js/9007.17a15931.chunk.js) |
| `./653_128/chromaSVG/9.js` | 45080 | [5080.662e1731.chunk.js](../../.ref/devices/653/static/js/5080.662e1731.chunk.js) |
| `./653_128/chromaSVG/10.js` | 7202 | [7202.38bf2ab0.chunk.js](../../.ref/devices/653/static/js/7202.38bf2ab0.chunk.js) |
| `./653_128/chromaSVG/11.js` | 75089 | [5089.3d7b9f8b.chunk.js](../../.ref/devices/653/static/js/5089.3d7b9f8b.chunk.js) |
| `./653_128/chromaSVG/12.js` | 77112 | [7112.996f046d.chunk.js](../../.ref/devices/653/static/js/7112.996f046d.chunk.js) |
| `./653_128/chromaSVG/15.js` | 92845 | [2845.c3d1bdb9.chunk.js](../../.ref/devices/653/static/js/2845.c3d1bdb9.chunk.js) |
| `./653_128/chromaSVG/16.js` | 44964 | [4964.7c068871.chunk.js](../../.ref/devices/653/static/js/4964.7c068871.chunk.js) |
| `./653_128/chromaSVG/17.js` | 66987 | [6987.19f1c8c2.chunk.js](../../.ref/devices/653/static/js/6987.19f1c8c2.chunk.js) |
| `./653_128/chromaSVG/18.js` | 72346 | [2346.263f4886.chunk.js](../../.ref/devices/653/static/js/2346.263f4886.chunk.js) |
| `./653_129/chromaSVG/1.js` | 90095 | [95.d6f3f13c.chunk.js](../../.ref/devices/653/static/js/95.d6f3f13c.chunk.js) |
| `./653_130/chromaSVG/1.js` | 58641 | [8641.a0b19656.chunk.js](../../.ref/devices/653/static/js/8641.a0b19656.chunk.js) |

| 777 原始 request | 模块 ID | 本地 JS |
|---|---|---|
| `./777_0/chromaSVG/0.js` | 1027 | [27.ee79b122.chunk.js](../../.ref/devices/777/static/js/27.ee79b122.chunk.js) |

## 6. 页面附属图和状态图标

以下资源家族用于已确认组件或相邻的共享控件状态。状态图标的“在包中可用”不代表每个产品都展示所有功能，是否挂载仍按逐页 render 树。

| 产品 | 用途 / 条件 | 本地文件 |
|---|---|---|
| 182 | 校准提示关闭 / hover | [icon_close_green.45f61360.svg](../../.ref/devices/182/static/media/icon_close_green.45f61360.svg)<br>[icon_close_white.8ab462b8.svg](../../.ref/devices/182/static/media/icon_close_white.8ab462b8.svg) |
| 182 | 普通映射与 Hypershift 图标 | [icon_config_default_a.0946ac79.svg](../../.ref/devices/182/static/media/icon_config_default_a.0946ac79.svg)<br>[icon_config_default.46d77571.svg](../../.ref/devices/182/static/media/icon_config_default.46d77571.svg)<br>[icon_config_disable_a.6b71a2b4.svg](../../.ref/devices/182/static/media/icon_config_disable_a.6b71a2b4.svg)<br>[icon_config_disable.52df003a.svg](../../.ref/devices/182/static/media/icon_config_disable.52df003a.svg)<br>[icon_config_hypershift_a.33a591f6.svg](../../.ref/devices/182/static/media/icon_config_hypershift_a.33a591f6.svg)<br>[icon_config_hypershift.6c83f48a.svg](../../.ref/devices/182/static/media/icon_config_hypershift.6c83f48a.svg)<br>[icon_config_keyboard_a.7051c99b.svg](../../.ref/devices/182/static/media/icon_config_keyboard_a.7051c99b.svg)<br>[icon_config_keyboard.08cddd8c.svg](../../.ref/devices/182/static/media/icon_config_keyboard.08cddd8c.svg)<br>[icon_config_macro_a.27e913c3.svg](../../.ref/devices/182/static/media/icon_config_macro_a.27e913c3.svg)<br>[icon_config_macro.243cd8e5.svg](../../.ref/devices/182/static/media/icon_config_macro.243cd8e5.svg)<br>[icon_config_mouse_a.e80d52da.svg](../../.ref/devices/182/static/media/icon_config_mouse_a.e80d52da.svg)<br>[icon_config_mouse_scrolling_a.c975098f.svg](../../.ref/devices/182/static/media/icon_config_mouse_scrolling_a.c975098f.svg)<br>[icon_config_mouse_scrolling.fd95e298.svg](../../.ref/devices/182/static/media/icon_config_mouse_scrolling.fd95e298.svg)<br>[icon_config_mouse_sensitivity_a.7f79e597.svg](../../.ref/devices/182/static/media/icon_config_mouse_sensitivity_a.7f79e597.svg)<br>[icon_config_mouse_sensitivity.cd4e347f.svg](../../.ref/devices/182/static/media/icon_config_mouse_sensitivity.cd4e347f.svg)<br>[icon_config_mouse.d7500793.svg](../../.ref/devices/182/static/media/icon_config_mouse.d7500793.svg) |
| 182 | Profile / 板载状态候选 | [profile_1.0aee3fb0.svg](../../.ref/devices/182/static/media/profile_1.0aee3fb0.svg)<br>[profile_2.beccee7f.svg](../../.ref/devices/182/static/media/profile_2.beccee7f.svg)<br>[profile_3.f868618e.svg](../../.ref/devices/182/static/media/profile_3.f868618e.svg)<br>[profile_4.d96e73a1.svg](../../.ref/devices/182/static/media/profile_4.d96e73a1.svg)<br>[profile_factory.ee33907e.svg](../../.ref/devices/182/static/media/profile_factory.ee33907e.svg)<br>[profile-default.f608d82c.svg](../../.ref/devices/182/static/media/profile-default.f608d82c.svg)<br>[profile-spinner.157bab13.svg](../../.ref/devices/182/static/media/profile-spinner.157bab13.svg)<br>[profile-unsupported.671bbbc6.svg](../../.ref/devices/182/static/media/profile-unsupported.671bbbc6.svg) |
| 182 | 电池状态候选（按 powerStatus 使用） | [icon_battery_0.d600fc0a.svg](../../.ref/devices/182/static/media/icon_battery_0.d600fc0a.svg)<br>[icon_battery_10.5a0a21e9.svg](../../.ref/devices/182/static/media/icon_battery_10.5a0a21e9.svg)<br>[icon_battery_100.b00b88f9.svg](../../.ref/devices/182/static/media/icon_battery_100.b00b88f9.svg)<br>[icon_battery_50.b36451e7.svg](../../.ref/devices/182/static/media/icon_battery_50.b36451e7.svg)<br>[icon_battery_charging_100.d01170a4.svg](../../.ref/devices/182/static/media/icon_battery_charging_100.d01170a4.svg)<br>[icon_battery_charging.99e70522.svg](../../.ref/devices/182/static/media/icon_battery_charging.99e70522.svg)<br>[icon_battery_disconnected.e5f74ae3.svg](../../.ref/devices/182/static/media/icon_battery_disconnected.e5f74ae3.svg)<br>[icon_battery_error.44052666.svg](../../.ref/devices/182/static/media/icon_battery_error.44052666.svg)<br>[icon_battery_paused.087ee1a5.svg](../../.ref/devices/182/static/media/icon_battery_paused.087ee1a5.svg) |
| 653 | Command Dial / Snap Tap / 游戏模式相关候选 | [dial-mapping.36cf0d1f.svg](../../.ref/devices/653/static/media/dial-mapping.36cf0d1f.svg)<br>[icon_config_multidial_a.aff8cb37.svg](../../.ref/devices/653/static/media/icon_config_multidial_a.aff8cb37.svg)<br>[icon_config_multidial.50aa8a22.svg](../../.ref/devices/653/static/media/icon_config_multidial.50aa8a22.svg) |
| 653 | 快速/高级灯光和颜色 | [chroma_studio.55db6875.svg](../../.ref/devices/653/static/media/chroma_studio.55db6875.svg)<br>[chroma_sync_v3_static.c8ddf315.svg](../../.ref/devices/653/static/media/chroma_sync_v3_static.c8ddf315.svg)<br>[clockwise-gray.8b2c0b15.svg](../../.ref/devices/653/static/media/clockwise-gray.8b2c0b15.svg)<br>[clockwise.eca45a25.svg](../../.ref/devices/653/static/media/clockwise.eca45a25.svg)<br>[color_picker_hover.c1c1c043.svg](../../.ref/devices/653/static/media/color_picker_hover.c1c1c043.svg)<br>[color_picker.b2c4481d.svg](../../.ref/devices/653/static/media/color_picker.b2c4481d.svg)<br>[counter-clockwise-gray.a5d7275a.svg](../../.ref/devices/653/static/media/counter-clockwise-gray.a5d7275a.svg)<br>[counter-clockwise.93ff0031.svg](../../.ref/devices/653/static/media/counter-clockwise.93ff0031.svg)<br>[fast.43db44c4.svg](../../.ref/devices/653/static/media/fast.43db44c4.svg)<br>[slow.8b9c0df7.svg](../../.ref/devices/653/static/media/slow.8b9c0df7.svg) |
| 777 | Sound / Mic Reset 三态 | [eq_reset_active.37c570d3.svg](../../.ref/devices/777/static/media/eq_reset_active.37c570d3.svg)<br>[eq_reset_hover.186df33c.svg](../../.ref/devices/777/static/media/eq_reset_hover.186df33c.svg)<br>[eq_reset.e0c3c09c.svg](../../.ref/devices/777/static/media/eq_reset.e0c3c09c.svg) |
| 777 | Stream Reactive Lighting 插图 | [stream-1x.b197a2c3.avif](../../.ref/devices/777/static/media/stream-1x.b197a2c3.avif)<br>[stream-2x.c872cf03.avif](../../.ref/devices/777/static/media/stream-2x.c872cf03.avif)<br>[stream-3x.e6572c2c.avif](../../.ref/devices/777/static/media/stream-3x.e6572c2c.avif) |
| 777 | Chroma / 颜色编辑 | [chroma_studio.55db6875.svg](../../.ref/devices/777/static/media/chroma_studio.55db6875.svg)<br>[chroma_sync_v3_static.c8ddf315.svg](../../.ref/devices/777/static/media/chroma_sync_v3_static.c8ddf315.svg)<br>[color_picker_hover.c1c1c043.svg](../../.ref/devices/777/static/media/color_picker_hover.c1c1c043.svg)<br>[color_picker.b2c4481d.svg](../../.ref/devices/777/static/media/color_picker.b2c4481d.svg) |

内嵌 SVG 组件也属于资源：182 Performance 的 Windows 鼠标属性 um、777 Sound 的系统属性 nM、设备页帮助/状态组件，都应沿 main 中的实际组件定位，不能因为没有独立 svg 文件就用无关图标代替。

CSS 生成的内容包括 dot-bg 点阵、控件轨道/滑块、部分三角/边框和 EQ 刻度布局。鼠标映射连线由画布绘制。它们不需要下载同名图片，但需要正确的几何和样式实现。

## 7. 已有但不是当前路由功能的资源

- 182 的 mouse_calibration_1/2、complete/error 在共有包内；实际 Calibration 为 Smart Tracking，不能由这些图追加表面扫描流程。
- 777 的 Helicopter_test_stereo_widening_demo / Helicopter_test_surround_demo WAV 由产品配置导出，但 VM/kM 未挂载测试/校准按钮。
- audio_profile、XLR、SensaHD、控制器和其它产品 SVG 大量来自共享组件；不能据此给这三个产品增加页面。
- 777 包内 1328/1332 产品上下文并不改变 777 的根导航。

| 范围 | 示例本地资源 | 结论 |
|---|---|---|
| 182 | [mouse_calibration_complete.0fcb4d53.svg](../../.ref/devices/182/static/media/mouse_calibration_complete.0fcb4d53.svg)<br>[mouse_calibration_error.a227b74c.svg](../../.ref/devices/182/static/media/mouse_calibration_error.a227b74c.svg) | 文件存在；不是本次 LD 路由的完成/失败画面 |
| 777 | [Helicopter_test_stereo_widening_demo.70d74fa5.wav](../../.ref/devices/777/static/media/Helicopter_test_stereo_widening_demo.70d74fa5.wav)<br>[Helicopter_test_surround_demo.48b345ad.wav](../../.ref/devices/777/static/media/Helicopter_test_surround_demo.48b345ad.wav) | 音频存在；当前 Sound/Mic 不提供对应测试按钮 |

## 8. 语言资源

产品翻译 context 位于各产品 `./{pid}_{edition}/translations/*.js`；组件先使用 locale key，再由 language reducer/翻译函数解析。主前端完整中文/英文包位于 `.ref/applications/synapse/dashboard/locales`；当前项目运行期语言数据位于 `locales/`。

下面给出当前产品的 en / zh-CN / index 入口，避免只从共有词汇表推断页面：

| 产品 | request | 模块 / chunk |
|---|---|---|
| 182 | `./182_0/translations/en.js` | 1040 / [6331.b92d9fc5.chunk.js](../../.ref/devices/182/static/js/6331.b92d9fc5.chunk.js) |
| 182 | `./182_0/translations/index.js` | 6331 / [6331.b92d9fc5.chunk.js](../../.ref/devices/182/static/js/6331.b92d9fc5.chunk.js) |
| 182 | `./182_0/translations/zh-CN.js` | 8115 / [8115.806ba232.chunk.js](../../.ref/devices/182/static/js/8115.806ba232.chunk.js) |
| 182 | `./182_128/translations/en.js` | 4365 / [8752.3e3ca2f4.chunk.js](../../.ref/devices/182/static/js/8752.3e3ca2f4.chunk.js) |
| 182 | `./182_128/translations/index.js` | 8752 / [8752.3e3ca2f4.chunk.js](../../.ref/devices/182/static/js/8752.3e3ca2f4.chunk.js) |
| 182 | `./182_128/translations/zh-CN.js` | 5984 / [8752.3e3ca2f4.chunk.js](../../.ref/devices/182/static/js/8752.3e3ca2f4.chunk.js) |
| 182 | `./182_129/translations/en.js` | 2698 / [9521.3bfa8a10.chunk.js](../../.ref/devices/182/static/js/9521.3bfa8a10.chunk.js) |
| 182 | `./182_129/translations/index.js` | 9521 / [9521.3bfa8a10.chunk.js](../../.ref/devices/182/static/js/9521.3bfa8a10.chunk.js) |
| 182 | `./182_129/translations/zh-CN.js` | 3129 / [9521.3bfa8a10.chunk.js](../../.ref/devices/182/static/js/9521.3bfa8a10.chunk.js) |
| 182 | `./182_132/translations/en.js` | 8990 / [8990.42172803.chunk.js](../../.ref/devices/182/static/js/8990.42172803.chunk.js) |
| 182 | `./182_132/translations/index.js` | 5157 / [5157.9754ff82.chunk.js](../../.ref/devices/182/static/js/5157.9754ff82.chunk.js) |
| 182 | `./182_132/translations/zh-CN.js` | 2005 / [5157.9754ff82.chunk.js](../../.ref/devices/182/static/js/5157.9754ff82.chunk.js) |
| 777 | `./777_0/translations/en.js` | 4361 / [742.9889722d.chunk.js](../../.ref/devices/777/static/js/742.9889722d.chunk.js) |
| 777 | `./777_0/translations/index.js` | 7517 / [517.403091f6.chunk.js](../../.ref/devices/777/static/js/517.403091f6.chunk.js) |
| 777 | `./777_0/translations/zh-CN.js` | 8093 / [93.04d6851c.chunk.js](../../.ref/devices/777/static/js/93.04d6851c.chunk.js) |

翻译 key 作为显示来源，不作为领域 ID。新增/切换 locale 不能改变 preset、按键、profile 或路由标识。

## 9. 当前 Rust 实际接入情况


| 本地文件 | 字节 | 当前使用 |
|---|---|---|
| [assets/mouse-182-dashboard3x.png](../../assets/mouse-182-dashboard3x.png) | 80271 | 保留的历史下载；当前 Dashboard 已从可追溯 PluginImages AVIF 重新转换，见 [资源使用清单](13-resource-usage.md) |
| [assets/window-close.svg](../../assets/window-close.svg) | 258 | 未接入；当前标题栏使用 Kit 窗口按钮 |
| [assets/window-maximize.svg](../../assets/window-maximize.svg) | 465 | 未接入；当前标题栏使用 Kit 窗口按钮 |
| [assets/window-minimize.svg](../../assets/window-minimize.svg) | 353 | 未接入；当前标题栏使用 Kit 窗口按钮 |
| [assets/window-restore.svg](../../assets/window-restore.svg) | 228 | 未接入；当前标题栏使用 Kit 窗口按钮 |
| [locales/en.json](../../locales/en.json) | 371475 | 当前运行期英文语言数据；不代表产品路由/功能证据 |
| [locales/zh-CN.json](../../locales/zh-CN.json) | 363785 | 当前运行期中文语言数据；不代表产品路由/功能证据 |
| [assets/synapse/stream-777.png](../../assets/synapse/stream-777.png) | 23398 | 777 Lighting 的 Stream Reactive Lighting 插图 |
| [assets/synapse/eq-reset.svg](../../assets/synapse/eq-reset.svg) | 565 | 777 Sound/Mic Reset 的默认图标；hover/pressed 变体也已打包 |
| [assets/synapse/keyboard-653-dial-mapping.svg](../../assets/synapse/keyboard-653-dial-mapping.svg) | 388 | 653 Customize Command Dial mapping affordance |
| [assets/synapse/chroma-studio.svg](../../assets/synapse/chroma-studio.svg) | 7051 | 653 高级灯光不可用状态的原版入口图标 |
| [assets/synapse/keyboard-653-layout-1.svg](../../assets/synapse/keyboard-653-layout-1.svg) | 171724 | 保留的 653 Chroma 区域；当前无渲染消费者，不用于 Customize 命中几何 |

当前运行入口通过 [resources.rs](../../src/resources.rs) 的 `SynapseAssets` 按资源键加载 PNG/SVG，并通过 `include_bytes!` 固定进程序；产品图、键盘 730×340 命中几何、Chroma 960×360 区域图不混用。当前 `.ref` 被 gitignore 排除，最终发布不依赖开发机上的 `.ref` 绝对路径；资源来源和哈希保留在 manifest。

GPUI Kit 的解码和 WOFF 注册能力需要按目标构建核对；锁文件存在 AVIF 编码依赖不代表能解码所有原图。当前生成器已将所用 AVIF 转为 PNG、四份 WOFF2 转为 TTF，并保留源文件/哈希、输出格式、尺寸、透明度和用途。

## 10. 主前端缺失资源明细

原目录基准为 `.ref/applications/synapse/dashboard/`。下表完整列出 asset-manifest 中非 map 缺失路径。84 项在本轮检索的 `.ref` 和 `assets` 中没有同名文件；189 项有同名候选，仍未恢复到原目录。来源 manifest：[asset-manifest.json](../../.ref/applications/synapse/dashboard/asset-manifest.json)。

### 10.1 未找到同名候选（84）


| 原引用路径（缺失） |
|---|
| `static/media/Gamer Room Dashboard Tutorial 1.080f80fb.mp4` |
| `static/media/Gamer Room Dashboard Tutorial 2.b4e338ae.mp4` |
| `static/media/Icon-Rectangle.71d513d1.svg` |
| `static/media/Synapse Dashboard Tutorial.4d4e2f9c.mp4` |
| `static/media/add_wifi_device.60d9fd5b.svg` |
| `static/media/aether-bulb-marketing-image.2048a07b.avif` |
| `static/media/aether-lamp-pro-marketing-image.57b787df.avif` |
| `static/media/aether-strip-marketing-image.797da4bf.avif` |
| `static/media/alexa.110b43c3.avif` |
| `static/media/animated_startup_dotted_scaling.d5d9ac9c.svg` |
| `static/media/app-gamer-room-icon.01c86f10.svg` |
| `static/media/app-streaming-icon.6be479a2.svg` |
| `static/media/arcade-controller-disable.ce6508f3.svg` |
| `static/media/arcade-controller-fw-update-disable.3348302b.svg` |
| `static/media/big_synapse_4.69474e71.svg` |
| `static/media/check-icon-normal.921ce9cb.svg` |
| `static/media/controller-disable.a11aa934.svg` |
| `static/media/controller_icon.18ddf9a7.svg` |
| `static/media/dongle-pairing.62f44d13.svg` |
| `static/media/earbuds-disable.6f9e24da.svg` |
| `static/media/gamer-room-bg-image@3x.ed0ae6ff.avif` |
| `static/media/gamer_room_hotspot_animation.53dd5566.svg` |
| `static/media/headset-disable.6cb91cff.svg` |
| `static/media/icon_add_key_light_outline.2bc5f2e7.svg` |
| `static/media/icon_add_smart_home_device_outline.01f8b64d.svg` |
| `static/media/icon_arrow_short_right.5a823bac.svg` |
| `static/media/icon_config_perfect180.53ef2a6c.svg` |
| `static/media/icon_config_perfect180_a.9b62bbd2.svg` |
| `static/media/icon_default.170f7482.svg` |
| `static/media/icon_device_accessories.af262686.svg` |
| `static/media/icon_device_chromahdk.c23d595f.svg` |
| `static/media/icon_device_headphone.0e80d85a.svg` |
| `static/media/icon_device_keyboard.e2c33997.svg` |
| `static/media/icon_device_mat.3752fc04.svg` |
| `static/media/icon_device_mouse.9d31d656.svg` |
| `static/media/icon_device_power_state_off.6b8c9694.svg` |
| `static/media/icon_external_link_active.69855296.svg` |
| `static/media/icon_external_link_green.f2d03392.svg` |
| `static/media/icon_external_link_white.168bdcbe.svg` |
| `static/media/icon_game_priority_warning.64842e1b.svg` |
| `static/media/icon_gamer_room_app.4c75473f.svg` |
| `static/media/icon_mobile_download.858c66c9.svg` |
| `static/media/icon_more_horizontal-5.72cdcb2c.svg` |
| `static/media/icon_new_iot_device_tooltip.990363a0.svg` |
| `static/media/icon_not_found.98525e61.svg` |
| `static/media/icon_open_to_link.845ace35.svg` |
| `static/media/icon_pen_edit.2d7a4ded.svg` |
| `static/media/icon_refresh-2.de191fed.svg` |
| `static/media/icon_refresh_3.4e3a6824.svg` |
| `static/media/icon_success_gray.14655ce6.svg` |
| `static/media/icon_wifi_add.5fd71806.svg` |
| `static/media/icon_wifi_error.a355ac62.svg` |
| `static/media/indeterminate-bar.c53ed439.svg` |
| `static/media/introduction_background.f3dc376c.avif` |
| `static/media/logo_alexa.b9b62050.svg` |
| `static/media/logo_armory_exchange.089029fd.svg` |
| `static/media/logo_armory_workshop.9cfc256a.svg` |
| `static/media/logo_chromaconnect.676a8f14.svg` |
| `static/media/logo_legacy_devices.13321b8b.svg` |
| `static/media/logo_synapse_4_animnated.036c5094.svg` |
| `static/media/logo_tour.c14123f7.svg` |
| `static/media/logo_unsupported_devices.557ccedb.svg` |
| `static/media/macro.4afae13e.avif` |
| `static/media/new_logo_synapse_4_animnated.5eb54d89.svg` |
| `static/media/placeholder_game_icon.17a60ecb.svg` |
| `static/media/ps-controller-disable.a16ccaff.avif` |
| `static/media/ps-icon.54354df0.svg` |
| `static/media/qr-code-gamer-room.58677794.svg` |
| `static/media/qr-code-key-light.f91e9fbe.svg` |
| `static/media/qr-code.e47e48d3.svg` |
| `static/media/razer_controller_setup_for_xbox.c9fccd0a.svg` |
| `static/media/razer_xbox_headset.aaea54b6.svg` |
| `static/media/see_password.123e5e09.svg` |
| `static/media/service_gold_and_silver.12f2d9c9.avif` |
| `static/media/services_community_placeholder.50541baa.avif` |
| `static/media/services_store_placeholder.9f5d3c9a.avif` |
| `static/media/services_support_placeholder.62c592fb.avif` |
| `static/media/skeleton-device-list-animation.9fcf73cc.svg` |
| `static/media/split_arrow.61fe5435.svg` |
| `static/media/warning.95a829f6.svg` |
| `static/media/windows_dynamic_lighting_active_icon.07524369.svg` |
| `static/media/windows_dynamic_lighting_icon.00836c88.svg` |
| `static/media/xbox-icon.11f09412.svg` |
| `sw.html` |

### 10.2 其它目录存在同名候选（189）

每项给出一个确切候选路径，并列出候选总数；同名 hash 文件可用于继续核对，但不能据文件名代替原始内容比对。

| 原引用路径（缺失） | 本地候选 | 候选数 |
|---|---|---|
| `static/media/alexa.b9b62050.svg` | [.ref/devices/182/static/media/alexa.b9b62050.svg](../../.ref/devices/182/static/media/alexa.b9b62050.svg) | 3 |
| `static/media/blue.c01cdc30.svg` | [.ref/devices/182/static/media/blue.c01cdc30.svg](../../.ref/devices/182/static/media/blue.c01cdc30.svg) | 3 |
| `static/media/circle.90ad9590.svg` | [.ref/devices/182/static/media/circle.90ad9590.svg](../../.ref/devices/182/static/media/circle.90ad9590.svg) | 3 |
| `static/media/close_button.5144c49d.svg` | [.ref/devices/182/static/media/close_button.5144c49d.svg](../../.ref/devices/182/static/media/close_button.5144c49d.svg) | 3 |
| `static/media/cloud-circle-gray.fe220500.svg` | [.ref/devices/182/static/media/cloud-circle-gray.fe220500.svg](../../.ref/devices/182/static/media/cloud-circle-gray.fe220500.svg) | 3 |
| `static/media/cloud-yellow.3957bb55.svg` | [.ref/devices/182/static/media/cloud-yellow.3957bb55.svg](../../.ref/devices/182/static/media/cloud-yellow.3957bb55.svg) | 3 |
| `static/media/common-windows-11.d477cadb.svg` | [.ref/devices/182/static/media/common-windows-11.d477cadb.svg](../../.ref/devices/182/static/media/common-windows-11.d477cadb.svg) | 3 |
| `static/media/computer-circle-gray.262fc6ea.svg` | [.ref/devices/182/static/media/computer-circle-gray.262fc6ea.svg](../../.ref/devices/182/static/media/computer-circle-gray.262fc6ea.svg) | 3 |
| `static/media/cyan.02647853.svg` | [.ref/devices/182/static/media/cyan.02647853.svg](../../.ref/devices/182/static/media/cyan.02647853.svg) | 3 |
| `static/media/dial-mapping.36cf0d1f.svg` | [.ref/devices/182/static/media/dial-mapping.36cf0d1f.svg](../../.ref/devices/182/static/media/dial-mapping.36cf0d1f.svg) | 3 |
| `static/media/disable_palette.03c60895.svg` | [.ref/devices/182/static/media/disable_palette.03c60895.svg](../../.ref/devices/182/static/media/disable_palette.03c60895.svg) | 3 |
| `static/media/eq_reset.e0c3c09c.svg` | [.ref/devices/182/static/media/eq_reset.e0c3c09c.svg](../../.ref/devices/182/static/media/eq_reset.e0c3c09c.svg) | 3 |
| `static/media/eq_reset_active.37c570d3.svg` | [.ref/devices/182/static/media/eq_reset_active.37c570d3.svg](../../.ref/devices/182/static/media/eq_reset_active.37c570d3.svg) | 3 |
| `static/media/eq_reset_hover.186df33c.svg` | [.ref/devices/182/static/media/eq_reset_hover.186df33c.svg](../../.ref/devices/182/static/media/eq_reset_hover.186df33c.svg) | 3 |
| `static/media/exclamation-circle-gray.73839f52.svg` | [.ref/devices/182/static/media/exclamation-circle-gray.73839f52.svg](../../.ref/devices/182/static/media/exclamation-circle-gray.73839f52.svg) | 3 |
| `static/media/fast.43db44c4.svg` | [.ref/devices/182/static/media/fast.43db44c4.svg](../../.ref/devices/182/static/media/fast.43db44c4.svg) | 3 |
| `static/media/green.9abd2946.svg` | [.ref/devices/182/static/media/green.9abd2946.svg](../../.ref/devices/182/static/media/green.9abd2946.svg) | 3 |
| `static/media/icon-progress_spinner.e93a0af0.svg` | [.ref/devices/182/static/media/icon-progress_spinner.e93a0af0.svg](../../.ref/devices/182/static/media/icon-progress_spinner.e93a0af0.svg) | 2 |
| `static/media/icon_actuation_point.bbadbed4.svg` | [.ref/devices/182/static/media/icon_actuation_point.bbadbed4.svg](../../.ref/devices/182/static/media/icon_actuation_point.bbadbed4.svg) | 3 |
| `static/media/icon_add.c95a8d74.svg` | [.ref/devices/182/static/media/icon_add.c95a8d74.svg](../../.ref/devices/182/static/media/icon_add.c95a8d74.svg) | 3 |
| `static/media/icon_add_w.0fc3f789.svg` | [.ref/devices/182/static/media/icon_add_w.0fc3f789.svg](../../.ref/devices/182/static/media/icon_add_w.0fc3f789.svg) | 3 |
| `static/media/icon_addon_error.5a00577f.svg` | [.ref/devices/182/static/media/icon_addon_error.5a00577f.svg](../../.ref/devices/182/static/media/icon_addon_error.5a00577f.svg) | 3 |
| `static/media/icon_app.b648a5e5.svg` | [.ref/devices/182/static/media/icon_app.b648a5e5.svg](../../.ref/devices/182/static/media/icon_app.b648a5e5.svg) | 3 |
| `static/media/icon_arrow_left_thin.e6d37c55.svg` | [.ref/devices/182/static/media/icon_arrow_left_thin.e6d37c55.svg](../../.ref/devices/182/static/media/icon_arrow_left_thin.e6d37c55.svg) | 3 |
| `static/media/icon_arrow_right_thin.bef8ca32.svg` | [.ref/devices/182/static/media/icon_arrow_right_thin.bef8ca32.svg](../../.ref/devices/182/static/media/icon_arrow_right_thin.bef8ca32.svg) | 3 |
| `static/media/icon_audio_enhancement_thx.e501de27.svg` | [.ref/devices/182/static/media/icon_audio_enhancement_thx.e501de27.svg](../../.ref/devices/182/static/media/icon_audio_enhancement_thx.e501de27.svg) | 3 |
| `static/media/icon_audio_profile_movie.ab5bcd10.svg` | [.ref/devices/182/static/media/icon_audio_profile_movie.ab5bcd10.svg](../../.ref/devices/182/static/media/icon_audio_profile_movie.ab5bcd10.svg) | 3 |
| `static/media/icon_back_arrow.b39e4841.svg` | [.ref/devices/182/static/media/icon_back_arrow.b39e4841.svg](../../.ref/devices/182/static/media/icon_back_arrow.b39e4841.svg) | 3 |
| `static/media/icon_battery_0.d600fc0a.svg` | [.ref/devices/182/static/media/icon_battery_0.d600fc0a.svg](../../.ref/devices/182/static/media/icon_battery_0.d600fc0a.svg) | 3 |
| `static/media/icon_battery_10.5a0a21e9.svg` | [.ref/devices/182/static/media/icon_battery_10.5a0a21e9.svg](../../.ref/devices/182/static/media/icon_battery_10.5a0a21e9.svg) | 3 |
| `static/media/icon_battery_100.b00b88f9.svg` | [.ref/devices/182/static/media/icon_battery_100.b00b88f9.svg](../../.ref/devices/182/static/media/icon_battery_100.b00b88f9.svg) | 3 |
| `static/media/icon_battery_20.5573858b.svg` | [.ref/devices/182/static/media/icon_battery_20.5573858b.svg](../../.ref/devices/182/static/media/icon_battery_20.5573858b.svg) | 3 |
| `static/media/icon_battery_30.2ea266e0.svg` | [.ref/devices/182/static/media/icon_battery_30.2ea266e0.svg](../../.ref/devices/182/static/media/icon_battery_30.2ea266e0.svg) | 3 |
| `static/media/icon_battery_40.425e62c7.svg` | [.ref/devices/182/static/media/icon_battery_40.425e62c7.svg](../../.ref/devices/182/static/media/icon_battery_40.425e62c7.svg) | 3 |
| `static/media/icon_battery_50.b36451e7.svg` | [.ref/devices/182/static/media/icon_battery_50.b36451e7.svg](../../.ref/devices/182/static/media/icon_battery_50.b36451e7.svg) | 3 |
| `static/media/icon_battery_60.ca9e0c58.svg` | [.ref/devices/182/static/media/icon_battery_60.ca9e0c58.svg](../../.ref/devices/182/static/media/icon_battery_60.ca9e0c58.svg) | 3 |
| `static/media/icon_battery_70.73ff3b3b.svg` | [.ref/devices/182/static/media/icon_battery_70.73ff3b3b.svg](../../.ref/devices/182/static/media/icon_battery_70.73ff3b3b.svg) | 3 |
| `static/media/icon_battery_80.cabac91c.svg` | [.ref/devices/182/static/media/icon_battery_80.cabac91c.svg](../../.ref/devices/182/static/media/icon_battery_80.cabac91c.svg) | 3 |
| `static/media/icon_battery_90.f59d2a3d.svg` | [.ref/devices/182/static/media/icon_battery_90.f59d2a3d.svg](../../.ref/devices/182/static/media/icon_battery_90.f59d2a3d.svg) | 3 |
| `static/media/icon_camera.0eefa3e2.svg` | [.ref/devices/182/static/media/icon_camera.0eefa3e2.svg](../../.ref/devices/182/static/media/icon_camera.0eefa3e2.svg) | 3 |
| `static/media/icon_camera.green.97c86790.svg` | [.ref/devices/182/static/media/icon_camera.green.97c86790.svg](../../.ref/devices/182/static/media/icon_camera.green.97c86790.svg) | 3 |
| `static/media/icon_category_keyboard.e681b4fe.svg` | [.ref/devices/182/static/media/icon_category_keyboard.e681b4fe.svg](../../.ref/devices/182/static/media/icon_category_keyboard.e681b4fe.svg) | 2 |
| `static/media/icon_close.55fe41f1.svg` | [.ref/devices/182/static/media/icon_close.55fe41f1.svg](../../.ref/devices/182/static/media/icon_close.55fe41f1.svg) | 3 |
| `static/media/icon_close_enclosed.6056b667.svg` | [.ref/devices/182/static/media/icon_close_enclosed.6056b667.svg](../../.ref/devices/182/static/media/icon_close_enclosed.6056b667.svg) | 3 |
| `static/media/icon_close_enclosed_a.83aff4bb.svg` | [.ref/devices/182/static/media/icon_close_enclosed_a.83aff4bb.svg](../../.ref/devices/182/static/media/icon_close_enclosed_a.83aff4bb.svg) | 3 |
| `static/media/icon_close_green.45f61360.svg` | [.ref/devices/182/static/media/icon_close_green.45f61360.svg](../../.ref/devices/182/static/media/icon_close_green.45f61360.svg) | 1 |
| `static/media/icon_close_white.8ab462b8.svg` | [.ref/devices/182/static/media/icon_close_white.8ab462b8.svg](../../.ref/devices/182/static/media/icon_close_white.8ab462b8.svg) | 3 |
| `static/media/icon_closepanel.86903958.svg` | [.ref/devices/182/static/media/icon_closepanel.86903958.svg](../../.ref/devices/182/static/media/icon_closepanel.86903958.svg) | 3 |
| `static/media/icon_cone.7bf8041f.svg` | [.ref/devices/182/static/media/icon_cone.7bf8041f.svg](../../.ref/devices/182/static/media/icon_cone.7bf8041f.svg) | 3 |
| `static/media/icon_config_brightness.7a95c958.svg` | [.ref/devices/182/static/media/icon_config_brightness.7a95c958.svg](../../.ref/devices/182/static/media/icon_config_brightness.7a95c958.svg) | 3 |
| `static/media/icon_config_brightness_a.570b75be.svg` | [.ref/devices/182/static/media/icon_config_brightness_a.570b75be.svg](../../.ref/devices/182/static/media/icon_config_brightness_a.570b75be.svg) | 3 |
| `static/media/icon_config_default.46d77571.svg` | [.ref/devices/182/static/media/icon_config_default.46d77571.svg](../../.ref/devices/182/static/media/icon_config_default.46d77571.svg) | 3 |
| `static/media/icon_config_default_a.0946ac79.svg` | [.ref/devices/182/static/media/icon_config_default_a.0946ac79.svg](../../.ref/devices/182/static/media/icon_config_default_a.0946ac79.svg) | 3 |
| `static/media/icon_config_disable.52df003a.svg` | [.ref/devices/182/static/media/icon_config_disable.52df003a.svg](../../.ref/devices/182/static/media/icon_config_disable.52df003a.svg) | 3 |
| `static/media/icon_config_disable_a.6b71a2b4.svg` | [.ref/devices/182/static/media/icon_config_disable_a.6b71a2b4.svg](../../.ref/devices/182/static/media/icon_config_disable_a.6b71a2b4.svg) | 3 |
| `static/media/icon_config_hypershift.6c83f48a.svg` | [.ref/devices/182/static/media/icon_config_hypershift.6c83f48a.svg](../../.ref/devices/182/static/media/icon_config_hypershift.6c83f48a.svg) | 3 |
| `static/media/icon_config_hypershift_a.33a591f6.svg` | [.ref/devices/182/static/media/icon_config_hypershift_a.33a591f6.svg](../../.ref/devices/182/static/media/icon_config_hypershift_a.33a591f6.svg) | 3 |
| `static/media/icon_config_interdevice.9b1fc6e9.svg` | [.ref/devices/182/static/media/icon_config_interdevice.9b1fc6e9.svg](../../.ref/devices/182/static/media/icon_config_interdevice.9b1fc6e9.svg) | 3 |
| `static/media/icon_config_interdevice_a.87c34f05.svg` | [.ref/devices/182/static/media/icon_config_interdevice_a.87c34f05.svg](../../.ref/devices/182/static/media/icon_config_interdevice_a.87c34f05.svg) | 3 |
| `static/media/icon_config_joystick.f5f05356.svg` | [.ref/devices/182/static/media/icon_config_joystick.f5f05356.svg](../../.ref/devices/182/static/media/icon_config_joystick.f5f05356.svg) | 3 |
| `static/media/icon_config_joystick_a.76f1ea9f.svg` | [.ref/devices/182/static/media/icon_config_joystick_a.76f1ea9f.svg](../../.ref/devices/182/static/media/icon_config_joystick_a.76f1ea9f.svg) | 3 |
| `static/media/icon_config_keyboard.08cddd8c.svg` | [.ref/devices/182/static/media/icon_config_keyboard.08cddd8c.svg](../../.ref/devices/182/static/media/icon_config_keyboard.08cddd8c.svg) | 3 |
| `static/media/icon_config_keyboard_a.7051c99b.svg` | [.ref/devices/182/static/media/icon_config_keyboard_a.7051c99b.svg](../../.ref/devices/182/static/media/icon_config_keyboard_a.7051c99b.svg) | 3 |
| `static/media/icon_config_keymap.cb5ba1f0.svg` | [.ref/devices/182/static/media/icon_config_keymap.cb5ba1f0.svg](../../.ref/devices/182/static/media/icon_config_keymap.cb5ba1f0.svg) | 3 |
| `static/media/icon_config_keymap_a.e27c970d.svg` | [.ref/devices/182/static/media/icon_config_keymap_a.e27c970d.svg](../../.ref/devices/182/static/media/icon_config_keymap_a.e27c970d.svg) | 3 |
| `static/media/icon_config_launch.3b800ccf.svg` | [.ref/devices/182/static/media/icon_config_launch.3b800ccf.svg](../../.ref/devices/182/static/media/icon_config_launch.3b800ccf.svg) | 3 |
| `static/media/icon_config_launch_a.52c03b9d.svg` | [.ref/devices/182/static/media/icon_config_launch_a.52c03b9d.svg](../../.ref/devices/182/static/media/icon_config_launch_a.52c03b9d.svg) | 3 |
| `static/media/icon_config_macro.243cd8e5.svg` | [.ref/devices/182/static/media/icon_config_macro.243cd8e5.svg](../../.ref/devices/182/static/media/icon_config_macro.243cd8e5.svg) | 3 |
| `static/media/icon_config_macro_a.27e913c3.svg` | [.ref/devices/182/static/media/icon_config_macro_a.27e913c3.svg](../../.ref/devices/182/static/media/icon_config_macro_a.27e913c3.svg) | 3 |
| `static/media/icon_config_mouse.d7500793.svg` | [.ref/devices/182/static/media/icon_config_mouse.d7500793.svg](../../.ref/devices/182/static/media/icon_config_mouse.d7500793.svg) | 3 |
| `static/media/icon_config_mouse_a.e80d52da.svg` | [.ref/devices/182/static/media/icon_config_mouse_a.e80d52da.svg](../../.ref/devices/182/static/media/icon_config_mouse_a.e80d52da.svg) | 3 |
| `static/media/icon_config_mouse_scrolling.fd95e298.svg` | [.ref/devices/182/static/media/icon_config_mouse_scrolling.fd95e298.svg](../../.ref/devices/182/static/media/icon_config_mouse_scrolling.fd95e298.svg) | 3 |
| `static/media/icon_config_mouse_scrolling_a.c975098f.svg` | [.ref/devices/182/static/media/icon_config_mouse_scrolling_a.c975098f.svg](../../.ref/devices/182/static/media/icon_config_mouse_scrolling_a.c975098f.svg) | 3 |
| `static/media/icon_config_multidial.50aa8a22.svg` | [.ref/devices/182/static/media/icon_config_multidial.50aa8a22.svg](../../.ref/devices/182/static/media/icon_config_multidial.50aa8a22.svg) | 3 |
| `static/media/icon_config_multidial_a.aff8cb37.svg` | [.ref/devices/182/static/media/icon_config_multidial_a.aff8cb37.svg](../../.ref/devices/182/static/media/icon_config_multidial_a.aff8cb37.svg) | 3 |
| `static/media/icon_config_multidial_white.c56d5b56.svg` | [.ref/devices/182/static/media/icon_config_multidial_white.c56d5b56.svg](../../.ref/devices/182/static/media/icon_config_multidial_white.c56d5b56.svg) | 3 |
| `static/media/icon_config_multimedia.800d8c90.svg` | [.ref/devices/182/static/media/icon_config_multimedia.800d8c90.svg](../../.ref/devices/182/static/media/icon_config_multimedia.800d8c90.svg) | 3 |
| `static/media/icon_config_multimedia_a.b2da1c02.svg` | [.ref/devices/182/static/media/icon_config_multimedia_a.b2da1c02.svg](../../.ref/devices/182/static/media/icon_config_multimedia_a.b2da1c02.svg) | 3 |
| `static/media/icon_config_switch_device_profile.88b9f2aa.svg` | [.ref/devices/182/static/media/icon_config_switch_device_profile.88b9f2aa.svg](../../.ref/devices/182/static/media/icon_config_switch_device_profile.88b9f2aa.svg) | 3 |
| `static/media/icon_config_switch_device_profile_a.942d360a.svg` | [.ref/devices/182/static/media/icon_config_switch_device_profile_a.942d360a.svg](../../.ref/devices/182/static/media/icon_config_switch_device_profile_a.942d360a.svg) | 3 |
| `static/media/icon_config_text.3259fd59.svg` | [.ref/devices/182/static/media/icon_config_text.3259fd59.svg](../../.ref/devices/182/static/media/icon_config_text.3259fd59.svg) | 3 |
| `static/media/icon_config_text_a.a7d11e49.svg` | [.ref/devices/182/static/media/icon_config_text_a.a7d11e49.svg](../../.ref/devices/182/static/media/icon_config_text_a.a7d11e49.svg) | 3 |
| `static/media/icon_config_windows_shortcut.580f7d02.svg` | [.ref/devices/182/static/media/icon_config_windows_shortcut.580f7d02.svg](../../.ref/devices/182/static/media/icon_config_windows_shortcut.580f7d02.svg) | 3 |
| `static/media/icon_config_windows_shortcut_a.3751588d.svg` | [.ref/devices/182/static/media/icon_config_windows_shortcut_a.3751588d.svg](../../.ref/devices/182/static/media/icon_config_windows_shortcut_a.3751588d.svg) | 3 |
| `static/media/icon_direction_ccw.e34b12f6.svg` | [.ref/devices/182/static/media/icon_direction_ccw.e34b12f6.svg](../../.ref/devices/182/static/media/icon_direction_ccw.e34b12f6.svg) | 3 |
| `static/media/icon_direction_ccw_1.3dffaadc.svg` | [.ref/devices/182/static/media/icon_direction_ccw_1.3dffaadc.svg](../../.ref/devices/182/static/media/icon_direction_ccw_1.3dffaadc.svg) | 3 |
| `static/media/icon_direction_cw.075c1730.svg` | [.ref/devices/182/static/media/icon_direction_cw.075c1730.svg](../../.ref/devices/182/static/media/icon_direction_cw.075c1730.svg) | 3 |
| `static/media/icon_direction_cw_1.0d8f9301.svg` | [.ref/devices/182/static/media/icon_direction_cw_1.0d8f9301.svg](../../.ref/devices/182/static/media/icon_direction_cw_1.0d8f9301.svg) | 3 |
| `static/media/icon_direction_down.531edd48.svg` | [.ref/devices/182/static/media/icon_direction_down.531edd48.svg](../../.ref/devices/182/static/media/icon_direction_down.531edd48.svg) | 3 |
| `static/media/icon_direction_down_999.56bcfa23.svg` | [.ref/devices/182/static/media/icon_direction_down_999.56bcfa23.svg](../../.ref/devices/182/static/media/icon_direction_down_999.56bcfa23.svg) | 3 |
| `static/media/icon_direction_inward.29c0ed8a.svg` | [.ref/devices/182/static/media/icon_direction_inward.29c0ed8a.svg](../../.ref/devices/182/static/media/icon_direction_inward.29c0ed8a.svg) | 3 |
| `static/media/icon_direction_inward_999.29221403.svg` | [.ref/devices/182/static/media/icon_direction_inward_999.29221403.svg](../../.ref/devices/182/static/media/icon_direction_inward_999.29221403.svg) | 3 |
| `static/media/icon_direction_left.cc325d65.svg` | [.ref/devices/182/static/media/icon_direction_left.cc325d65.svg](../../.ref/devices/182/static/media/icon_direction_left.cc325d65.svg) | 3 |
| `static/media/icon_direction_left_999.3397283a.svg` | [.ref/devices/182/static/media/icon_direction_left_999.3397283a.svg](../../.ref/devices/182/static/media/icon_direction_left_999.3397283a.svg) | 3 |
| `static/media/icon_direction_outward.7c388a12.svg` | [.ref/devices/182/static/media/icon_direction_outward.7c388a12.svg](../../.ref/devices/182/static/media/icon_direction_outward.7c388a12.svg) | 3 |
| `static/media/icon_direction_outward_999.4cd91b14.svg` | [.ref/devices/182/static/media/icon_direction_outward_999.4cd91b14.svg](../../.ref/devices/182/static/media/icon_direction_outward_999.4cd91b14.svg) | 3 |
| `static/media/icon_direction_right.70dccfc5.svg` | [.ref/devices/182/static/media/icon_direction_right.70dccfc5.svg](../../.ref/devices/182/static/media/icon_direction_right.70dccfc5.svg) | 3 |
| `static/media/icon_direction_right_999.8c251efa.svg` | [.ref/devices/182/static/media/icon_direction_right_999.8c251efa.svg](../../.ref/devices/182/static/media/icon_direction_right_999.8c251efa.svg) | 3 |
| `static/media/icon_direction_up.f4029a80.svg` | [.ref/devices/182/static/media/icon_direction_up.f4029a80.svg](../../.ref/devices/182/static/media/icon_direction_up.f4029a80.svg) | 3 |
| `static/media/icon_direction_up_999.2c24f25f.svg` | [.ref/devices/182/static/media/icon_direction_up_999.2c24f25f.svg](../../.ref/devices/182/static/media/icon_direction_up_999.2c24f25f.svg) | 3 |
| `static/media/icon_download.1e6d735a.svg` | [.ref/devices/182/static/media/icon_download.1e6d735a.svg](../../.ref/devices/182/static/media/icon_download.1e6d735a.svg) | 3 |
| `static/media/icon_emoji.07a71bf8.svg` | [.ref/devices/182/static/media/icon_emoji.07a71bf8.svg](../../.ref/devices/182/static/media/icon_emoji.07a71bf8.svg) | 3 |
| `static/media/icon_emoji_active.b1a4efad.svg` | [.ref/devices/182/static/media/icon_emoji_active.b1a4efad.svg](../../.ref/devices/182/static/media/icon_emoji_active.b1a4efad.svg) | 3 |
| `static/media/icon_expand.55a47b0c.svg` | [.ref/devices/182/static/media/icon_expand.55a47b0c.svg](../../.ref/devices/182/static/media/icon_expand.55a47b0c.svg) | 3 |
| `static/media/icon_expand_l.3b8f6b34.svg` | [.ref/devices/182/static/media/icon_expand_l.3b8f6b34.svg](../../.ref/devices/182/static/media/icon_expand_l.3b8f6b34.svg) | 3 |
| `static/media/icon_folder.e5ceb5b6.svg` | [.ref/devices/182/static/media/icon_folder.e5ceb5b6.svg](../../.ref/devices/182/static/media/icon_folder.e5ceb5b6.svg) | 3 |
| `static/media/icon_lock.8f4a479b.svg` | [.ref/devices/182/static/media/icon_lock.8f4a479b.svg](../../.ref/devices/182/static/media/icon_lock.8f4a479b.svg) | 3 |
| `static/media/icon_macro.1d733729.svg` | [.ref/devices/182/static/media/icon_macro.1d733729.svg](../../.ref/devices/182/static/media/icon_macro.1d733729.svg) | 3 |
| `static/media/icon_marching_ants_connected_left_to_right.d11787b8.svg` | [.ref/devices/182/static/media/icon_marching_ants_connected_left_to_right.d11787b8.svg](../../.ref/devices/182/static/media/icon_marching_ants_connected_left_to_right.d11787b8.svg) | 2 |
| `static/media/icon_marching_ants_connected_right_to_left.325e3d12.svg` | [.ref/devices/182/static/media/icon_marching_ants_connected_right_to_left.325e3d12.svg](../../.ref/devices/182/static/media/icon_marching_ants_connected_right_to_left.325e3d12.svg) | 2 |
| `static/media/icon_marching_ants_connecting_left_to_right.c8c3af5a.svg` | [.ref/devices/182/static/media/icon_marching_ants_connecting_left_to_right.c8c3af5a.svg](../../.ref/devices/182/static/media/icon_marching_ants_connecting_left_to_right.c8c3af5a.svg) | 2 |
| `static/media/icon_marching_ants_connecting_right_to_left.e9be0b6f.svg` | [.ref/devices/182/static/media/icon_marching_ants_connecting_right_to_left.e9be0b6f.svg](../../.ref/devices/182/static/media/icon_marching_ants_connecting_right_to_left.e9be0b6f.svg) | 2 |
| `static/media/icon_marching_ants_master_line.294a8460.svg` | [.ref/devices/182/static/media/icon_marching_ants_master_line.294a8460.svg](../../.ref/devices/182/static/media/icon_marching_ants_master_line.294a8460.svg) | 2 |
| `static/media/icon_more.fb688d78.svg` | [.ref/devices/182/static/media/icon_more.fb688d78.svg](../../.ref/devices/182/static/media/icon_more.fb688d78.svg) | 3 |
| `static/media/icon_more_70.a6ba9f0c.svg` | [.ref/devices/182/static/media/icon_more_70.a6ba9f0c.svg](../../.ref/devices/182/static/media/icon_more_70.a6ba9f0c.svg) | 1 |
| `static/media/icon_more_active.8e04906f.svg` | [.ref/devices/182/static/media/icon_more_active.8e04906f.svg](../../.ref/devices/182/static/media/icon_more_active.8e04906f.svg) | 3 |
| `static/media/icon_more_default.eb7284c1.svg` | [.ref/devices/182/static/media/icon_more_default.eb7284c1.svg](../../.ref/devices/182/static/media/icon_more_default.eb7284c1.svg) | 3 |
| `static/media/icon_more_g.32e1e984.svg` | [.ref/devices/182/static/media/icon_more_g.32e1e984.svg](../../.ref/devices/182/static/media/icon_more_g.32e1e984.svg) | 3 |
| `static/media/icon_more_horizontal.72cdcb2c.svg` | [.ref/devices/182/static/media/icon_more_horizontal.72cdcb2c.svg](../../.ref/devices/182/static/media/icon_more_horizontal.72cdcb2c.svg) | 3 |
| `static/media/icon_obm.888208d6.svg` | [.ref/devices/182/static/media/icon_obm.888208d6.svg](../../.ref/devices/182/static/media/icon_obm.888208d6.svg) | 3 |
| `static/media/icon_obm_2.512f2e40.svg` | [.ref/devices/182/static/media/icon_obm_2.512f2e40.svg](../../.ref/devices/182/static/media/icon_obm_2.512f2e40.svg) | 3 |
| `static/media/icon_obm_3.4350a917.svg` | [.ref/devices/182/static/media/icon_obm_3.4350a917.svg](../../.ref/devices/182/static/media/icon_obm_3.4350a917.svg) | 3 |
| `static/media/icon_obm_4.379b007d.svg` | [.ref/devices/182/static/media/icon_obm_4.379b007d.svg](../../.ref/devices/182/static/media/icon_obm_4.379b007d.svg) | 3 |
| `static/media/icon_obm_error.7fee0731.svg` | [.ref/devices/182/static/media/icon_obm_error.7fee0731.svg](../../.ref/devices/182/static/media/icon_obm_error.7fee0731.svg) | 3 |
| `static/media/icon_obm_na.5e6daed8.svg` | [.ref/devices/182/static/media/icon_obm_na.5e6daed8.svg](../../.ref/devices/182/static/media/icon_obm_na.5e6daed8.svg) | 3 |
| `static/media/icon_obm_overlay_error.36277c8e.svg` | [.ref/devices/182/static/media/icon_obm_overlay_error.36277c8e.svg](../../.ref/devices/182/static/media/icon_obm_overlay_error.36277c8e.svg) | 3 |
| `static/media/icon_refresh-1.856d6176.svg` | [.ref/devices/182/static/media/icon_refresh-1.856d6176.svg](../../.ref/devices/182/static/media/icon_refresh-1.856d6176.svg) | 3 |
| `static/media/icon_refresh-1.green.5d467aff.svg` | [.ref/devices/182/static/media/icon_refresh-1.green.5d467aff.svg](../../.ref/devices/182/static/media/icon_refresh-1.green.5d467aff.svg) | 3 |
| `static/media/icon_refresh.80aa16c3.svg` | [.ref/devices/182/static/media/icon_refresh.80aa16c3.svg](../../.ref/devices/182/static/media/icon_refresh.80aa16c3.svg) | 3 |
| `static/media/icon_refresh_white.80aa16c3.svg` | [.ref/devices/182/static/media/icon_refresh_white.80aa16c3.svg](../../.ref/devices/182/static/media/icon_refresh_white.80aa16c3.svg) | 3 |
| `static/media/icon_release.45f570dc.svg` | [.ref/devices/182/static/media/icon_release.45f570dc.svg](../../.ref/devices/182/static/media/icon_release.45f570dc.svg) | 3 |
| `static/media/icon_search.b84dee08.svg` | [.ref/devices/182/static/media/icon_search.b84dee08.svg](../../.ref/devices/182/static/media/icon_search.b84dee08.svg) | 3 |
| `static/media/icon_search.green.f5e350b7.svg` | [.ref/devices/182/static/media/icon_search.green.f5e350b7.svg](../../.ref/devices/182/static/media/icon_search.green.f5e350b7.svg) | 3 |
| `static/media/icon_sensitivity_xy.b9eb5286.svg` | [.ref/devices/182/static/media/icon_sensitivity_xy.b9eb5286.svg](../../.ref/devices/182/static/media/icon_sensitivity_xy.b9eb5286.svg) | 3 |
| `static/media/icon_sensitivity_xy_active.39d46352.svg` | [.ref/devices/182/static/media/icon_sensitivity_xy_active.39d46352.svg](../../.ref/devices/182/static/media/icon_sensitivity_xy_active.39d46352.svg) | 3 |
| `static/media/icon_sensitivity_xy_disabled.3a6ee34e.svg` | [.ref/devices/182/static/media/icon_sensitivity_xy_disabled.3a6ee34e.svg](../../.ref/devices/182/static/media/icon_sensitivity_xy_disabled.3a6ee34e.svg) | 3 |
| `static/media/icon_sidepanel.e53fef93.svg` | [.ref/devices/182/static/media/icon_sidepanel.e53fef93.svg](../../.ref/devices/182/static/media/icon_sidepanel.e53fef93.svg) | 3 |
| `static/media/icon_sidepanel_a.90d67a6e.svg` | [.ref/devices/182/static/media/icon_sidepanel_a.90d67a6e.svg](../../.ref/devices/182/static/media/icon_sidepanel_a.90d67a6e.svg) | 3 |
| `static/media/icon_small_blur_cone.9c492dd7.svg` | [.ref/devices/182/static/media/icon_small_blur_cone.9c492dd7.svg](../../.ref/devices/182/static/media/icon_small_blur_cone.9c492dd7.svg) | 2 |
| `static/media/icon_small_cone.8ec07c8a.svg` | [.ref/devices/182/static/media/icon_small_cone.8ec07c8a.svg](../../.ref/devices/182/static/media/icon_small_cone.8ec07c8a.svg) | 2 |
| `static/media/icon_sync.c57b3cca.svg` | [.ref/devices/182/static/media/icon_sync.c57b3cca.svg](../../.ref/devices/182/static/media/icon_sync.c57b3cca.svg) | 3 |
| `static/media/icon_upload.3e232198.svg` | [.ref/devices/182/static/media/icon_upload.3e232198.svg](../../.ref/devices/182/static/media/icon_upload.3e232198.svg) | 2 |
| `static/media/icon_upload_in_black.4230f4d3.svg` | [.ref/devices/182/static/media/icon_upload_in_black.4230f4d3.svg](../../.ref/devices/182/static/media/icon_upload_in_black.4230f4d3.svg) | 2 |
| `static/media/icon_warning.6c0cd78b.svg` | [.ref/devices/182/static/media/icon_warning.6c0cd78b.svg](../../.ref/devices/182/static/media/icon_warning.6c0cd78b.svg) | 3 |
| `static/media/info-icon.769264c1.svg` | [.ref/devices/182/static/media/info-icon.769264c1.svg](../../.ref/devices/182/static/media/info-icon.769264c1.svg) | 3 |
| `static/media/instant.c9629055.svg` | [.ref/devices/182/static/media/instant.c9629055.svg](../../.ref/devices/182/static/media/instant.c9629055.svg) | 3 |
| `static/media/nav_back_arrow.0203563a.svg` | [.ref/devices/182/static/media/nav_back_arrow.0203563a.svg](../../.ref/devices/182/static/media/nav_back_arrow.0203563a.svg) | 3 |
| `static/media/nav_fwd_arrow.8ceda865.svg` | [.ref/devices/182/static/media/nav_fwd_arrow.8ceda865.svg](../../.ref/devices/182/static/media/nav_fwd_arrow.8ceda865.svg) | 3 |
| `static/media/path.0086a00e.svg` | [.ref/devices/182/static/media/path.0086a00e.svg](../../.ref/devices/182/static/media/path.0086a00e.svg) | 3 |
| `static/media/plus.a386742c.svg` | [.ref/devices/182/static/media/plus.a386742c.svg](../../.ref/devices/182/static/media/plus.a386742c.svg) | 3 |
| `static/media/profile-default.f608d82c.svg` | [.ref/devices/182/static/media/profile-default.f608d82c.svg](../../.ref/devices/182/static/media/profile-default.f608d82c.svg) | 3 |
| `static/media/profile-unsupported.671bbbc6.svg` | [.ref/devices/182/static/media/profile-unsupported.671bbbc6.svg](../../.ref/devices/182/static/media/profile-unsupported.671bbbc6.svg) | 3 |
| `static/media/ps-btn-create.9005c933.svg` | [.ref/devices/182/static/media/ps-btn-create.9005c933.svg](../../.ref/devices/182/static/media/ps-btn-create.9005c933.svg) | 3 |
| `static/media/ps-btn-options.8ba682f4.svg` | [.ref/devices/182/static/media/ps-btn-options.8ba682f4.svg](../../.ref/devices/182/static/media/ps-btn-options.8ba682f4.svg) | 3 |
| `static/media/ps-circle.2f4a4594.svg` | [.ref/devices/182/static/media/ps-circle.2f4a4594.svg](../../.ref/devices/182/static/media/ps-circle.2f4a4594.svg) | 3 |
| `static/media/ps-square.1bc237d1.svg` | [.ref/devices/182/static/media/ps-square.1bc237d1.svg](../../.ref/devices/182/static/media/ps-square.1bc237d1.svg) | 3 |
| `static/media/ps-triangle.f73de746.svg` | [.ref/devices/182/static/media/ps-triangle.f73de746.svg](../../.ref/devices/182/static/media/ps-triangle.f73de746.svg) | 3 |
| `static/media/red.99a70e7d.svg` | [.ref/devices/182/static/media/red.99a70e7d.svg](../../.ref/devices/182/static/media/red.99a70e7d.svg) | 3 |
| `static/media/require_macro_module_icon.20694b5c.svg` | [.ref/devices/182/static/media/require_macro_module_icon.20694b5c.svg](../../.ref/devices/182/static/media/require_macro_module_icon.20694b5c.svg) | 3 |
| `static/media/require_synapse_icon.d5e656b0.svg` | [.ref/devices/182/static/media/require_synapse_icon.d5e656b0.svg](../../.ref/devices/182/static/media/require_synapse_icon.d5e656b0.svg) | 3 |
| `static/media/save-white.783e6aa4.svg` | [.ref/devices/182/static/media/save-white.783e6aa4.svg](../../.ref/devices/182/static/media/save-white.783e6aa4.svg) | 3 |
| `static/media/scrollmode_switch_bg.cd01ecda.svg` | [.ref/devices/182/static/media/scrollmode_switch_bg.cd01ecda.svg](../../.ref/devices/182/static/media/scrollmode_switch_bg.cd01ecda.svg) | 3 |
| `static/media/scrollmode_switch_top.5d132bac.svg` | [.ref/devices/182/static/media/scrollmode_switch_top.5d132bac.svg](../../.ref/devices/182/static/media/scrollmode_switch_top.5d132bac.svg) | 3 |
| `static/media/slow.8b9c0df7.svg` | [.ref/devices/182/static/media/slow.8b9c0df7.svg](../../.ref/devices/182/static/media/slow.8b9c0df7.svg) | 3 |
| `static/media/snap_tap_icon.fdbdd616.svg` | [.ref/devices/182/static/media/snap_tap_icon.fdbdd616.svg](../../.ref/devices/182/static/media/snap_tap_icon.fdbdd616.svg) | 3 |
| `static/media/standard.cbf1362d.svg` | [.ref/devices/182/static/media/standard.cbf1362d.svg](../../.ref/devices/182/static/media/standard.cbf1362d.svg) | 3 |
| `static/media/star.1d11a1b8.svg` | [.ref/devices/182/static/media/star.1d11a1b8.svg](../../.ref/devices/182/static/media/star.1d11a1b8.svg) | 3 |
| `static/media/stepper_down.349f755c.svg` | [.ref/devices/182/static/media/stepper_down.349f755c.svg](../../.ref/devices/182/static/media/stepper_down.349f755c.svg) | 3 |
| `static/media/stepper_up.dcb04520.svg` | [.ref/devices/182/static/media/stepper_up.dcb04520.svg](../../.ref/devices/182/static/media/stepper_up.dcb04520.svg) | 3 |
| `static/media/synapse_profile_migration.c3622288.svg` | [.ref/devices/182/static/media/synapse_profile_migration.c3622288.svg](../../.ref/devices/182/static/media/synapse_profile_migration.c3622288.svg) | 3 |
| `static/media/thumbs_down.1581c2be.svg` | [.ref/devices/182/static/media/thumbs_down.1581c2be.svg](../../.ref/devices/182/static/media/thumbs_down.1581c2be.svg) | 3 |
| `static/media/thumbs_up.550d225b.svg` | [.ref/devices/182/static/media/thumbs_up.550d225b.svg](../../.ref/devices/182/static/media/thumbs_up.550d225b.svg) | 3 |
| `static/media/thx_logo.2a283c30.svg` | [.ref/devices/182/static/media/thx_logo.2a283c30.svg](../../.ref/devices/182/static/media/thx_logo.2a283c30.svg) | 3 |
| `static/media/tooltip_exclamationmark.cc8fb226.svg` | [.ref/devices/182/static/media/tooltip_exclamationmark.cc8fb226.svg](../../.ref/devices/182/static/media/tooltip_exclamationmark.cc8fb226.svg) | 3 |
| `static/media/user.53ceca5f.svg` | [.ref/devices/182/static/media/user.53ceca5f.svg](../../.ref/devices/182/static/media/user.53ceca5f.svg) | 3 |
| `static/media/window-big-icon.0ec8a269.svg` | [.ref/devices/182/static/media/window-big-icon.0ec8a269.svg](../../.ref/devices/182/static/media/window-big-icon.0ec8a269.svg) | 3 |
| `static/media/windows_logo.8fb1e7e2.svg` | [.ref/devices/182/static/media/windows_logo.8fb1e7e2.svg](../../.ref/devices/182/static/media/windows_logo.8fb1e7e2.svg) | 3 |
| `static/media/xbox-a-3.c85cb7b9.svg` | [.ref/devices/182/static/media/xbox-a-3.c85cb7b9.svg](../../.ref/devices/182/static/media/xbox-a-3.c85cb7b9.svg) | 3 |
| `static/media/xbox-a-white.ddb51c5e.svg` | [.ref/devices/182/static/media/xbox-a-white.ddb51c5e.svg](../../.ref/devices/182/static/media/xbox-a-white.ddb51c5e.svg) | 3 |
| `static/media/xbox-leftjoystick-press.a58c272c.svg` | [.ref/devices/182/static/media/xbox-leftjoystick-press.a58c272c.svg](../../.ref/devices/182/static/media/xbox-leftjoystick-press.a58c272c.svg) | 3 |
| `static/media/xbox-leftjoystick-up-7.dea6602a.svg` | [.ref/devices/182/static/media/xbox-leftjoystick-up-7.dea6602a.svg](../../.ref/devices/182/static/media/xbox-leftjoystick-up-7.dea6602a.svg) | 3 |
| `static/media/xbox-menu.029cb487.svg` | [.ref/devices/182/static/media/xbox-menu.029cb487.svg](../../.ref/devices/182/static/media/xbox-menu.029cb487.svg) | 3 |
| `static/media/xbox-o-white.722d78b0.svg` | [.ref/devices/182/static/media/xbox-o-white.722d78b0.svg](../../.ref/devices/182/static/media/xbox-o-white.722d78b0.svg) | 3 |
| `static/media/xbox-view.771d8231.svg` | [.ref/devices/182/static/media/xbox-view.771d8231.svg](../../.ref/devices/182/static/media/xbox-view.771d8231.svg) | 3 |
| `static/media/xbox-x-1.4c09810f.svg` | [.ref/devices/182/static/media/xbox-x-1.4c09810f.svg](../../.ref/devices/182/static/media/xbox-x-1.4c09810f.svg) | 3 |
| `static/media/xbox-x-white.16ac9300.svg` | [.ref/devices/182/static/media/xbox-x-white.16ac9300.svg](../../.ref/devices/182/static/media/xbox-x-white.16ac9300.svg) | 3 |
| `static/media/xbox-y-1.9c2fb89a.svg` | [.ref/devices/182/static/media/xbox-y-1.9c2fb89a.svg](../../.ref/devices/182/static/media/xbox-y-1.9c2fb89a.svg) | 3 |
| `static/media/xbox-y-white.7078f657.svg` | [.ref/devices/182/static/media/xbox-y-white.7078f657.svg](../../.ref/devices/182/static/media/xbox-y-white.7078f657.svg) | 3 |
| `static/media/yellow.a30299c6.svg` | [.ref/devices/182/static/media/yellow.a30299c6.svg](../../.ref/devices/182/static/media/yellow.a30299c6.svg) | 3 |

## 11. 资源验收

以“页面/状态 → 领域资源键 → 原请求 → 实际文件 → 解码/加载 → 显示尺寸/命中几何”逐项验证。检查 edition/layout、普通/hover/pressed/disabled、透明边距、1x/3x、字体 fallback、缺图/加载失败。补齐缺失文件后重新核对 manifest，不能只看目录文件数量。

本表中的路径检查只证明本地存在性与静态引用关系，没有验证网络服务、真实设备画面或 GPUI 运行时解码。

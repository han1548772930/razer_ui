# 资源使用与未接入清单

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](20-current-source-version.md)为准。

统计快照：**2026-10-02**。本页区分原始文件、导入资源、加载注册和实际页面消费者。已注册不等于已显示；条件分支可达也不等于服务已连接。动态产品图、映射图标、方向和 DPI 图标不能只用完整文件名字面量判断。

## 可复核的统计入口

- `python -B tools/validate-resources.py`：核对源/输出哈希、格式、嵌入键、原 Webpack request、Dashboard 下载映射和键盘几何。
- `python -X utf8 tools/validate-embedded-json.py`：核对每个 `include_str!("*.json")` 内嵌数据是否满足同文件（或唯一同名结构）里 `#[derive(Deserialize)]` 的必需字段，含 `Option`／`#[serde(default)]`／`rename`／`rename_all` 与嵌套 `Vec<T>`。内嵌 JSON 缺字段是启动时 panic，不是编译错误，因此必须有这条检查。
- `python -B tools/audit-resource-usage.py`：沿 `src/main.rs` 生产模块声明排除旧页面及测试，汇总直接引用及已审计的动态资源族。它是本仓库的静态清单，不是通用 Rust 调用图分析器。
- `.work/resource-env/Scripts/python.exe tools/prepare-resources.py`：使用现有资源环境离线再生成；不联网、不执行原 JavaScript、不修改 `.ref`。依赖版本见 `tools/requirements-resources.txt`。

静态校验通过：**272** 条 manifest 源/输出记录，**98** 个 Webpack request、**56** 个 Customize 部件/产品映射、**41** 个 Dashboard 身份，以及 **16** 个键盘布局的 **1901** 个精确形状。消费审计覆盖55个生产模块。未启动程序、DLL、服务或测试；尚无本轮窗口渲染验证。

## 原始资源的范围

| 范围 | 文件数 | 说明 |
| --- | ---: | --- |
| `.ref` 全部 | 10291 | 含前端、Electron、依赖、源码映射、日志及提取记录，并非图片数量 |
| `.ref/applications/synapse/dashboard` | 319 | 含 166 项 static/media |
| `.ref/settings` | 329 | 新取得的 Settings 前端，含 303 项 static/media |
| `.ref/profile-migration` | 73 | 独立迁移应用，含10项static/media |
| `.ref/release-patch-note` | 5 | 独立发行说明应用，含1项static/media |
| `.ref/devices/182` | 529 | 含 408 项 static/media |
| `.ref/devices/653` | 664 | 含 449 项 static/media |
| `.ref/devices/777` | 425 | 含 376 项 static/media |

| media 目录 | SVG | AVIF | PNG | WAV | 合计 | 独立内容 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| frontend | 152 | 14 | 0 | 0 | 166 | 165 |
| settings | 299 | 4 | 0 | 0 | 303 | 301 |
| profile-migration | 8 | 1 | 1 | 0 | 10 | 10 |
| release-patch-note | 1 | 0 | 0 | 0 | 1 | 1 |
| 182 | 382 | 26 | 0 | 0 | 408 | 403 |
| 653 | 397 | 52 | 0 | 0 | 449 | 444 |
| 777 | 358 | 16 | 0 | 2 | 376 | 372 |
| 合计 | **1597** | **113** | **1** | **2** | **1713** | **537（跨目录 SHA-256 去重）** |

大量通用 SVG 在多个包中相同，一个来源接入后可以共用。字体、Webpack JS/CSS、依赖和 `.map` 也不能各算一个缺失界面。新 Dashboard AVIF 和热点 SVG 来自精确静态 URL，原始下载另存 `assets/synapse/*-source.*`，不计入 `.ref` 的上述统计。

## 272 项导入资源的消费链

[manifest.json](../../assets/synapse/manifest.json) 含 **96 PNG、152 SVG、4 TTF、20 JSON**，230个不同源路径。所有272项列入 `embedded.rs`，其中227项有当前生产代码消费者，45项没有当前消费者。

| 状态 | 数量 | 消费入口 |
| --- | ---: | --- |
| 直接引用图像，包括条件/hover/显式预览分支 | 114 | shell、设备页、配对、Gamer Room、模块目录/预览、板载配置、Settings、迁移及发行说明；逐项位置由审计脚本输出 |
| 动态映射/方向/DPI 图像，扣除直接引用 | 47 | 15 个映射分类各自的默认/选中状态、12 个方向状态、5 个 DPI 阶段 |
| 动态 Customize 产品/鼠标底部，扣除前两类 | 28 | `resolve_device_image` → Customize/设备页面 |
| 动态 Dashboard/配对图像，扣除直接引用 | 17 | `dashboard_image` → `main_pages` / `pairing_page`；41个下载身份去重为19张，其中2张亦被迁移示例直接引用 |
| 字体 | 4 | `register_fonts` |
| 当前运行期 JSON | 17 | 16 份完整 Customize groupList + `keyboard-653-layouts.json` |
| 无当前图像消费者 | 42 | 下表，含低分辨率备用、条件部件和未接入图标 |
| 提取/追溯 JSON | 3 | deviceconfig、旧 keys、customize-layouts 汇总 |
| 合计 | **272** | **227 有消费者，45 无消费者** |

产品映射的 **56 行不是 56 张独立图片**。Product/MouseBottom 当前选择 28 张；KeyboardWrist/KeyboardDial 有映射但没有页面调用；Streamer 仍由耳机页直接引用。完整 groupList 含 **1965 个输入**，比 1901 个几何形状多出的 64 个是每布局 4 个不可见拨轮子输入，不应删除。

## 已纠正的原版细节

### Dashboard 与 Customize 是不同图片来源

原 `7861.1b0e99a4.chunk.js` 的卡片请求走 `products/{pid}/ui/{pid}_{edition}/PluginImages/{pid}_{edition}_{layout}_dashboard3x.avif`；设备 Customize 走 `img_prods` 的 Webpack context，不能互换。

已从原 URL 取得 **41 个真实 Dashboard AVIF**：182 的 6 个 edition、653 的 34 个 edition/layout 组合、777 标准图，去重后嵌入 19 张 PNG。[dashboard-image-map.json](../../assets/synapse/dashboard-image-map.json) 逐项记录身份、URL、原 AVIF 哈希及目标；`dashboard-images.rs` 是原生索引。`dashboard-182.png` 也已从此次可追溯 AVIF 重新转换，不再依赖缺失转换记录的历史 PNG。

`dashboard_image` 对旧键盘 layout 0 使用 1；明确的未知 layout/edition 返回无图，由页面展示占位，不用别的颜色、布局或 Customize 图替代。配对卡遵循原 `wt`：键盘取 layout 1、鼠标取 0；主页面保留设备实际 layout。demo PID 沿用既有 653 预览映射。

### Gamer Room

- 背景、灯泡、灯带、Lamp Pro、应用图标、二维码、移动下载图标来自原 frontend media；Aether Lamp 来自 9388 的 **71610** 模块内 PNG data URL。
- `gr-add-device.svg` 原本就是 `icon_add_smart_home_device_outline.01f8b64d.svg`，也是 `IotPopupRoot` 的 `Ae`；准备页可复用，不需要复制别名。
- 新 `gr-hotspot.svg` 来自原 `gamer_room_hotspot_animation.53dd5566.svg`。原 ellipse 没有静态 `rx/ry`，直接交给不执行 SMIL 的 SVG 解码器会不可见。现在冻结原 **keyTime 0.25（1/3 秒）**：半径 10、描边 4、透明度 0.2，保留原坐标变换；源 SVG、URL、哈希均保留。**这是静态环，未还原脉冲动画。**
- 添加设备的右箭头和教程步骤 indicator 已用原 SVG；箭头来自 IotPopupRoot 精确 URL，下载原图也保留。
- 介绍视频仍使用原精确媒体 URL 外部打开，没有打包或内嵌播放视频；本机没有 ffmpeg，未生成视频帧或安装解码工具。

### Settings、颜色弹层和设备页

- Settings 的 Synapse 4 标志按原SVG自然尺寸294.366×70显示；原CSS的120px为高度上限。Windows Dynamic Lighting图标和 `settings-warning.svg` 均由 `settings_lighting.rs` 消费，未知控制权不显示伪造的活动状态，显式预览可查看Chroma/WDL说明及切换状态。
- 社交图形从720实际渲染的inline SVG提取，共8组常态/hover。7个圆图标共享Facebook/YouTube注入的CSS：常态灰色，hover绿色、1.5px描边；270×50的Insider独立使用原边框悬停规则。不能直接采用独立media副本中不同或已注释的绿色样式。`prepare-resources.py` 静态解析形状与样式，不执行JS；manifest记录源码偏移和转换方式。
- `lighting_color.rs` 同时供设备灯光和Command Dial使用，主色板为40个预设加16个共享自定义槽，二级编辑器为220×269；加色、吸管图标来自原SVG。选中环、棋盘格、色相/饱和度平面及明度轨道按原CSS用原生绘制实现，并非漏导入图片。自定义槽保存/删除立即独立持久化，不提交其他草稿；系统吸管回调未接入，按钮禁用。
- 配对分类占位从 4130 CSS data URL 解码；paired/unpair 来自 React 内联 SVG，出处、偏移及哈希在 `pairing-manifest.json`，没有手工重绘图形。
- Command Dial自定义模式使用 `digital-dial.1e701318.avif`；本轮说明已按源控件/文本呈现，导入的 `keyboard-653-dial-mapping.svg` 与旧大拨轮 `keyboard-653-dial.png` 当前没有生产消费者，保留追溯。
- 映射弹层分类选中时使用15个原active图（Lighting来自原CSS的另一份PNG data URL），关闭按钮使用原icon_close。Command Dial的添加/禁用/hover、更多及hover、删除/重置及各自hover均已接入。
- `onboard_memory.rs` 消费原板载配置、同步/错误、锁定、宏、内存不足、传输提示及帮助图标，已有槽位选择、宏内存明细和冲突弹层。真实设备未读回时显示未知；仅 `PREVIEW-` 设备可打开明确标注的分配、同步、锁定、失败、宏内存和映射冲突预览，预览不写入设备或本地配置。Chroma Studio图标已有消费者，不能据此推断原生Studio编辑器已接入。

### 模块状态与配置迁移

- 模块目录保留安装状态未读取及禁用安装按钮；每一行现在带原版的盒名、窗口名与地址（`docs/re/display-window-contract.md` 的 `named_windows` 与模块盒去向表），本地已实现对应窗口的行（Alexa、配置文件迁移、介绍导览）直接显示“打开”并进入本地页面——原版点击模块盒本来就是 `focusTab(windowName)`，只有未安装时才先打开安装器。`module_preview.rs` 从设置本地工作区打开17种显式样例，覆盖6505的安装器、维护、已安装/断开设备、移除和固件连接限制。下载完成进入保存分支，不等于安装完成。卸载确认的Alexa清除设置复选框与宏保留说明分别按源分支呈现。
- `category-keyboard.svg`、`category-mouse.svg` 来自App模块96689的路径及最终变换表，显示为40×40、viewBox为20×20；不是同名映射分类图标。模块图标、宏/Alexa详情图由目录和预览共用，预览安装/移除/固件按钮不调用原生服务。
- `profile_migration.rs` 已从Settings迁移按钮可达。日期、损坏文件、游戏关联、缺失宏、未使用和成功状态图标来自独立profile-migration media；宏卡来自原AVIF，Chroma卡来自 `main.512f18b6.js` 的Dg内嵌PNG。设备示例卡复用已追溯的Dashboard图片。
- 迁移视图初始显示扫描状态未读取；11种明确标注的样例覆盖备份分组、扫描、准备/执行、成功警告及失败列表。原生备份扫描、导入队列和迁移结果读取未接入，不能把示例记录计作实际已迁移配置。
- `release_notes.rs` 已接Settings的Release Notes入口，使用独立release-patch-note应用的外链SVG和原章节布局。默认显示内容未读取，并提供官方发布记录及明确标注的章节示例；不是抓取到或已安装版本的真实发行内容。

## 无当前图像消费者的 42 项

| 分组 | 数量 | 文件/说明 |
| --- | ---: | --- |
| 腕托/滚轮图层 | 4 | `keyboard-653-wrist{,-white}.png`、`keyboard-653-roller{,-white}.png`；resolver 有映射但无页面调用，原 `km` 依赖设备/输入状态 |
| 182 低分辨率备用 | 9 | `product-182-*-1x.*.png`，3 张底部 + 6 张正面；当前选 3x |
| 653 低分辨率/通用备用 | 20 | 18 张 1x，加 `product-653-prd.4221e2bc.png`、`product-653-prd.5fbaa05f.png`；当前选布局 3x |
| 777 低分辨率备用 | 1 | `product-777-prd-1x.5cd1e5ea.png` |
| 旧 Windows 图标 | 1 | `windows.svg`；当前入口用 windows-11 图 |
| 未消费的 hover 文件 | 3 | `expand-hover.svg`、`dashboard-add-hover.svg`；基本图已显示，专用图hover分支尚未接入。`color-picker-hover.svg`对应的系统吸管尚禁用 |
| 旧拨轮说明图 | 2 | `keyboard-653-dial.png`、`keyboard-653-dial-mapping.svg`；当前说明由源码对应控件/文本呈现 |
| Dashboard 拖动别名 | 1 | `dashboard-drag.svg`；同源 `dpi-draggable.svg` 已用于 DPI |
| 旧 Chroma 区域图 | 1 | `keyboard-653-layout-1.svg` 来自 965 的 Chroma 区域，不能替代 Customize 精确命中图 |
| 合计 | **42** | 不以使用率为由常驻显示备用图或伪造硬件状态 |

## 未导入原文件不等于缺失页面

| 资源组 | 原始证据与当前结论 |
| --- | --- |
| 三种 `hyperspeed-dongle*.avif` | 4130 模块 2537/46384/62328 存在，但唯一引用在约字符 64269 的 `ct=(s(50202),s(46384),s(5951),s(2537),s(11970),s(62328),e=>...)`，返回值被逗号表达式丢弃。当前包无图片消费者，不能据此判定缺三张接收器卡或三页流程 |
| `icon_obm*`、`profile_1…4`、`profile_factory` | 653的 `renderOBM` / BLE能力分支已形成槽位、状态图标和显式预览；部分编号和工厂标记以原生文字/控件绘制。是否缺图必须逐项追踪实际分支，真实槽位与同步结果仍需设备读回 |
| `card_icon`、`installation_cover_placeholder`、`discover-more-razer-apps`、`install_sensahd` | 目录、设备/模块状态及安装器分支已有可达界面和显式预览；每个额外媒体仍需原组件/条件证据，不能把安装能力未知当作整页未实现，也不能把预览当作原生下载安装已接入 |
| `icon_audio_profile_*`、`icon_audio_enhancement_thx` | 耳机音频/EQ/增强分支；应按真实产品调用链补，不凭文件名新增功能 |
| 777 其他 `prd-*`、stream 1x/2x | 共享媒体集合；仅 product context 已解析分支可证明身份，不能全当成 777 edition |

## 329 个 assets 文件为何不等于 272

另外**57个文件**是：41个Dashboard原AVIF、1个迁移宏原AVIF、2个原SVG（热点与箭头）、4个自有窗口SVG、1个历史Dashboard PNG、4份生成Rust表，以及manifest/product-image-map/dashboard-image-map/pairing-manifest共4份追溯表。它们不是额外的57个界面消费者。自有窗口SVG仍未接入，当前窗口按钮来自Kit。历史Dashboard PNG保留追溯，已不用于生成本轮图像。

来源：[原资源索引](04-resource-index.md)、[页面覆盖](07-page-coverage.md)、[Customize 调用链](../screens/01-customize.md)、[资源 resolver](../../src/resources.rs)、[生成器](../../tools/prepare-resources.py)、[静态消费审计](../../tools/audit-resource-usage.py)。

# 产品独立模式接入核对（2026-10-05）

本轮只使用 `.ref/devices/<pid>/static/js/` 与同目录的当前 CSS；未读取任何已废弃前端/宿主目录。当前包 SHA-256 与既有注册导航、displayMode 清单重新比对，随后使用 Acorn 静态解析根组件。没有执行下载的 JavaScript，也没有运行应用、构建或测试。

## 本轮接入

- 28 个 Audio 产品的 `chromaApp` 根，原先都被 `SourceProductWorkspace::supports_lighting_page` 的家族白名单挡住。
- 新白名单来自逐产品的 `renderView` 条件、根组件、`connect` 包装、`render` 和该产品注册 Lighting 组件的调用关系。没有将整个 Audio 家族开放。
- 独立页直接读取原 `AudioProductWorkspace` 的编辑状态，既不修改主窗口的选中页，也不重复添加产品名标题。入口继续使用已有的 `has_chroma_device_page` 判断，因此这些本地根现在可以直接打开。
- 1465（Hammerhead V3 Chroma）的导航键为 `LIGHTING`，并非 `TAB_LIGHTING`；新路由保留原键。
- 修复 1303、1304 普通灯效页与独立灯效页中的布局：左侧一列连续放亮度、关灯两个 widget，右侧一列放效果。原本把三个 section 都做成独立列不符合 `wm` / `kM`。

可复查产物：

- 静态提取器：`tools/extract-product-mode-components.cjs`。
- 逐产品根条件、组件 renderer、引用位置及 CSS 证据：`docs/re/product-mode-current-components.json`。
- 实际消费的 28 产品白名单与 body 最小宽度：`src/features/audio_chroma_modes.json`。
- 本地路由：`src/features/source_workspace.rs` 的 `supports_lighting_page` / `lighting_page_element`。
- 独立 body 与两款 Nommo 布局修复：`src/features/audio_products.rs`。

## 明确核对到的调用链

| 产品 | 当前源调用链 | 结论 |
| --- | --- | --- |
| 1303 Nommo Chroma | `Qg.renderView` → `bg` → `Fg.render` → `zm` → `wm`；普通 Lighting 同样挂 `zm` | 可共享本地 Lighting body |
| 1304 Nommo Pro | `eH.renderView` → `Wf` → `Vf.render` → `KM` → `kM`；普通 Lighting 同样挂 `KM` | 可共享本地 Lighting body |
| 1313 Kraken Kitty Edition | `AK.renderView` → `iK` → `aK.render` → `DY` → `CY`；普通 Lighting 同样挂 `DY` | 根与普通 Lighting 相同，但普通 body 仍缺 cosplay 等细节 |
| 1342 Audio Mixer | 根 `$g` 的 renderer → `OP` → `lP`；普通 Lighting 同样挂 `OP` | 根可以复用，现有区域灯效 body 仍明显不完整 |
| 1370 Nommo V2 Pro | 根 renderer → `SW` → `dW`；普通 Lighting 同样挂 `SW` | 可共享现有 body，未宣称效果编辑器完整 |
| 1465 Hammerhead V3 Chroma | 根 renderer → `mP` → `pP`；注册页键 `LIGHTING` | 独立键不能被 `TAB_LIGHTING` 过滤掉 |

全部新增产品 ID：1303、1304、1313、1319、1325、1328、1330、1331、1332、1335、1342、1352、1353、1354、1364、1370、1372、1376、1378、1387、1389、1396、1406、1407、1439、1443、1462、1465。每个产品都在 JSON 中保留自己的 bundle SHA、root condition、root symbol、Lighting symbol、nav offset 和匹配位置。

根条件仍有区别。例如 1303/1304 要求 URL serial 与设备 serial 相同，1313 只要求传入 serial 非空。本地入口本身持有已选择设备的 workspace，未制造另一个设备实例，也未把这些源条件描述成完全一致。

## 尺寸、字体与 wrapper

1303 当前 CSS：`.ref/devices/1303/static/css/main.164978f0.css`；1304：`main.5b9f9366.css`；1313：`main.2fcc1f32.css`。其余新增产品的 CSS 路径和 SHA 见逐产品 JSON。

共同 `.body-wrapper` 是 `padding:10px 20px 20px`、默认 `min-width:600px`；`.main-container` 背景为 `#222`。本轮使用该 padding、Roboto 14px body 与现有 source panel 字体规则。

关键差异：1303/1304 根内联样式只取消 `.main-container` 的最小宽度，`.body-wrapper` 仍为 600px；另 26 个根明确同时取消 `.body-wrapper, .main-container` 的最小宽度。新生成表按根内联样式保留这个差异，而不是给整个家族统一 `min-width:0`。

Nommo 两款的三 widget 排列由各自 `wm` / `kM` 的 JSX 顺序验证：

- 1303：`Il(direction:"left")` → `tP`、`Ym`；`Il(direction:"right")` → `ym`。
- 1304：`bl(direction:"left")` → `op`、`rp`；`bl(direction:"right")` → `zM`。
- `.body-widgets` 为居中可换行、`max-width:1240px`；`.widget-col` 为 600px 列；widget 为 `margin:10px auto`、`padding:30px 40px`、`#111`、5px 圆角；标题 RazerF5 16px、`#44d62c`。复用的 `surface::page_columns` / `surface::panel` 具有对应规则。

本轮没有补充或猜测新动画；widget 的源 `height .2s`、效果状态/菜单动画等仍属于下述未完成 UI 细节。

## 发现但没有接错的 Armory 分支

这些产品存在根分支并不意味着它们应挂普通 Customize 页：

| 产品 | 当前 Armory 调用链 | 与通用 mapping 的区别 |
| --- | --- | --- |
| 1303 | `jg` → `Xg` 样式 wrapper → `Ul` → `Ll` | 只显示 `.widget-prod.dot-bg` 产品图；普通高度 250，Armory 取消容器最小宽度 |
| 1304 | `Qf` → `qf` 样式 wrapper → `eR` → `Zl` | 同上，不能挂声音控制或按键表 |
| 1313 | `IK` → `lK` 样式 wrapper → `Sy` | 产品图分支，非 Customize |
| 3893 Hanbo | `HH` → `fH` 样式 wrapper → `fg` | CPU/GPU 温度、频率、风扇/泵转速、冷却液温度、°C/°F 的仪表；不能复用风扇设置页 |
| 3907 Cooling Pad | `TU` → `rU` → `sU` | 专用 Armory 消息与设备图组件，需要单独实现 |

这些分支本轮没有开放到错误的映射 body。音频产品图还缺对应本地嵌入资源，3893 仪表也没有完整本地 renderer；需要继续提取资源和专用组件后接入。

## 仍未完成的范围

新增路由不代表其控件已经逐像素完成。现有 Audio descriptor renderer 仍有已查实的差异：

- 1313 的 `CY` 左列有三个组件，现有 descriptor 只表达亮度与关灯部分；cosplay 等内容未完全还原。
- 1342 的 `lP` 是左列四个、右列三个区域灯效组件，包含区域颜色、不同效果和示意图；现有 descriptor 退化为十个亮度 panel，必须重做专用 body。
- 1387/1396 的 Lighting renderer 还包含 `isWDLSupported` 条件组件；本轮未编造系统状态。
- 快速/高级效果切换、颜色编辑器、自定义颜色、关灯 idle 编辑器的产品差异和动画尚未完成。1303/1304 只修正列排列和 wrapper，没有把已有简化效果下拉宣称为完整源码复刻。
- 其他 Audio Armory 根、音频 lazy chunk 中的根，以及 3893/3907 配件灯效 body 仍需后续逐组件接入。本轮没有扩大 `supports_mapping_page` 的家族范围。
- Chroma 顶层快速预设当前仍依赖原 `DeviceWorkspace` 编辑接口；新增 Audio popup 复用自己的原生数据结构，这一轮没有伪造快捷预设写入。

验证限于静态源 hash、Acorn 解析、生成白名单与现有 descriptor 的交集检查、Rust 格式化；统一 `cargo check --locked --all-targets` 由主任务执行。

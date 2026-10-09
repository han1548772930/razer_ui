# Kraken V4 Pro 1383 OLED System Info 编辑器

2026-10-06。本页只使用 `.ref/devices/1383/asset-manifest.json` 声明的当前产品源码；
字体沿用已校验的当前官方 `.ref/host-4.0.827/` RazerF5。未使用历史前端或其他产品的
System Info 实现作为依据。

## 源码定位

主要脚本为 `static/js/main.a86f6801.js`、`static/js/6141.5d00192e.chunk.js`，
样式为 `static/css/main.7a0131e4.css`、`static/css/6141.d8b30400.chunk.css`。
完整路径、SHA-256、AST 原文与 UTF-16 code-unit 偏移见
[源码收据](audio-oled-system-source.json)。

| 当前模块/符号 | 依据 |
| --- | --- |
| `51278:_v` | 编辑器结构、局部状态、Reset、Apply、圆点切换 |
| `51278:gv/hv/uv/xv` | 三个固定幻灯片、空槽菜单、拖放目标、标签拖动 |
| `51278:vv` | OLED 预览内容、电量阈值、语言分支 |
| `51278:fv/hp` | 日期格式选项、原服务提交 action |
| `51278:Mv/Tn/oh` | 编辑器分派、公共弹层及操作区 |
| `96373:p/A/d` | 通用标签、三个标题、3/5/10 秒间隔 |
| `46472:Te/O` | 当前产品的 HEADSET BATTERY 标签及拖动 SVG |
| `79826:T` | system 初值、六个默认标签及预览示例数值 |
| `61050:s`、当前 `.tooltip_parent/.drop-tips` | 配置项帮助提示及问号资源 |

`tools/prepare-audio-oled-system.cjs` 使用维护中的 Acorn/CSS 静态解析器，解析导出、
局部声明、字面量、数组展开和静态成员索引；不加载或执行下载脚本。
生成的 `audio_oled_system_data.json` 与源码收据通过同一工具的 `--check` 核对。

## 已实现的 UI 与状态

Home 的 System Info EDIT 直接打开独立草稿编辑器。它固定显示三张幻灯片、每张左右两槽，
没有添加、删除或排序幻灯片的入口。九种标签依序为 CPU 使用率、CPU 温度、GPU 使用率、
GPU 温度、内存、日期、时间、笔记本电量、耳机电量。标签允许在多个槽中重复出现。

只有空槽接受拖动或显示添加菜单。拖动时标签透明度为 0.4，空目标为绿色 0.5，当前目标为
绿色 1；拖动预览使用原 SVG 及其固有尺寸。有内容的槽在 hover 时将标签透明度降至 0.3，
显示原红色删除图标。标题和原 6×6 圆点切换编辑器预览；Home 按所选间隔轮播并跳过全空幻灯片。

Apply 仅在六槽全空时禁用。Cancel 和关闭丢弃编辑草稿，Apply 写入本地 profile 的
`oledHome.system`，由现有 profile 机制保存。Reset 严格按 `_v` 只重置幻灯片、当前页、
轮播间隔和温度单位，**保留日期格式与时间格式**。

预览使用 `79826:T` 的源初值，包括 `25% / 35°C`、`35% / 45°C`、`07/15/2024 / 13:00`。
切换温度、日期、时间格式不会自行改写这些示例：原 `_v` 把这些设置保存在局部状态，却仍将
selector 提供的 `info` 传入 `vv`。数字电量显示百分号并按 `>75 / >50 / >25 / >5 / else`
选择原电量图标；字符串形式的耳机 `100%` 不额外附图标。OLED language 为 1 时采用标签的
当前中文 `deviceLabel`，其他情况使用初始 `info.label`。

尺寸、间距及颜色取自收据内当前 CustomizeSystemInfo/DisplayWidget CSS，集中颜色定义位于
`crates/razer-pages/src/features/audio_oled_system_theme.rs`。标签为 11px、标题为 12px、正文为 14px；
幻灯片 255×63px、槽 128×38px、OLED 内容 232×64px。标签/添加提示使用源 300ms 线性淡入、
300px 宽度与 29/40px top；配置帮助沿用当前 DropTips。公共弹层使用同源 Tn 外壳、焦点管理
及已接入的源入场动画，不添加原关闭分派中不存在的退出动画。

空槽菜单按 `gv → hv` 的实际父子结构放在幻灯片下，直接使用源 `position:absolute/top:61px`；
左侧 `left:0/width:126px`，右侧 `right:0/width:129px`。原 `Mv` 将外层 body 设为
`overflow:visible`，内部 CustomizeModal body 使用 `overflow-x:hidden/overflow-y:scroll`；
菜单保留在该编辑滚动布局内，没有上下翻转或视口吸附。

本地组合使用 Base `PopoverState` 管理受控开关、Escape、焦点捕获/恢复与生命周期，
Base Button 提供菜单项激活，父层/选项具有 Menu/MenuItem 语义。菜单外的 mousedown 关闭菜单，
点击空槽再次打开，与原事件顺序一致。`audio_oled_system_menu.rs` 仅适配源布局和绘制层次：
直接返回绝对子元素的布局，使用 deferred 对应源 `z-index:100`，在 slide 的 prepaint 捕获
当前祖先 ContentMask，并在延后 prepaint 和 paint 都恢复它。因此滚动剪掉的内容既不绘制，
也不拦截指针；不使用会强制吸附两轴的 Base Positioner。

## 本地恢复与资源

原通用合并器不会用 null 覆盖默认标签对象，因此恢复阶段对 **1383 的 system 对象**单独保留
保存值，再调用 `system::normalize`。规范化只接受三张幻灯片、已知 tag id 或 null，重建标签
元数据；间隔只接受 3/5/10，页码 0/1/2，时间格式 0/1，日期格式 0/1/2，温度单位仅接受
celsius/fahrenheit。`info` 从原初值重建，不接受本地文件伪造的遥测元数据。

`tools/prepare-audio-oled-system-assets.py` 准备并校验 19 个资源：9 个标签 SVG、5 个电量 SVG、
2 个内嵌圆点 PNG、原添加/删除 React SVG 树，以及当前问号 SVG。网络资源必须由该产品 manifest
声明；SVG 原文件按字节复制，React SVG 只序列化源字面量树，PNG 校验 6×6 尺寸。
源与输出 SHA-256 记录于 `assets/synapse/audio-oled-system-assets.json`，嵌入表为
`assets/synapse/audio-oled-system-embedded.rs`。拖动尺寸表用具名 `SystemAssetSizes` 反序列化，
由资源工具同步生成和校验。

源码生成 `--check`、资源 `--check` 与 Rustfmt 已通过。全局嵌入 JSON 静态校验为 45 份、
0 失败、4 项原有跳过；此实现没有增加跳过项。最终 `cargo check --locked --all-targets`
由主任务统一运行，以主任务最后结果为准。

## 尚未闭合的差异

未接真实遥测、`SET_OLED_DISPLAY_SYSTEM_INFO` 服务写入或设备确认；本地 Apply 不代表硬件事务成功。
日期控件沿用项目的源样式 Select，原浏览器文本行盒与原生控件的最终像素表现尚未运行验收。
没有运行应用、构建、测试、安装器、下载 JS 或 DLL；本页不宣称完成运行时视觉验收。

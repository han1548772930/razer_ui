> 2026-10-06 correction: [Current camera presentation audit](camera-presentation-current-audit.md) supersedes the processing mount, resolution-selector, disclosure and tooltip claims below. In particular, 3592 mounts a resolution selector on PROCESSING, and current advanced-camera preview layout retains the empty right-hand video surface; the earlier 360?202 placeholder description is historical.

# 当前摄像头取景与放置块审计

2026-10-03。用户要求按原代码补齐所有产品与界面的视觉、交互细节。本轮处理 Kiyo 系列在此前缺少的取景块（变焦、平移／倾斜、五个取景预设、预设快捷键）、分辨率行与水印放置盘。来源仅使用当前产品包 `.ref/devices/<ID>/`，未执行应用、测试、下载的 JavaScript 或 DLL。

## 来源证据

四个当前根都从 CAMERA 页挂载同一组组件。三个较早的根（3587／3589／3590）用共享 Customize 布局，没有这些控件，因此不生成。

| 产品 | 源文件 | SHA-256（前 16 位） | CAMERA 页控件数（本轮前 → 后） |
| --- | --- | --- | ---: |
| 3592 Kiyo Pro Ultra | main.323f0a8a.js | e2752209d1aa227a | 11 → 15 |
| 3594 Kiyo V2 Pro | main.bb246bb0.js | 3ac856b9d24a8043 | 9 → 14 |
| 3595 Kiyo V2 | main.19c1ceba.js | a6dcff393dd84d2b | 9 → 14 |
| 3596 Kiyo V2 X | main.0e57873f.js | d5c7ad36f9f5b145 | 4 → 9 |

生成器 `tools/generate-native-camera-controls.cjs` 为每个控件写入 `source` 收据（路径、偏移、结束位置）；缺少证据时直接抛错，不会生成猜测值。压缩名可能被其他模块复用，因此水印位置先经模块导出表 `SYM:()=>local` 再按作用域做字面量折叠，分辨率列表按挂载组件实际引用的绑定（`X||list`，3596 为模块属性）定位，并校验取值唯一。

## 已接入的控件

| 控件 | 种类 | 绑定 | 3592 | 3594 | 3595 | 3596 |
| --- | --- | --- | --- | --- | --- | --- |
| 分辨率 | `select` | `/camera/resolution` | 原版未挂载 | 10 项 | 10 项 | 10 项 |
| 变焦 | `slider` | `@view/zoom` | 1–4 / 0.1 | 1–4 / 0.1 | 1–4 / 0.1 | 1–1.4 / 0.01 |
| 平移／倾斜 | `pan_tilt` | `@view/pan` + `@view/tilt` | maxPanTilt 10 | 10 | 20 | 20 |
| 取景预设 | `preset` | `/camera/viewPresets/viewMode` | 1–5 | 1–5 | 1–5 | 1–5 |
| 预设快捷键 | `keys` | `@view/shortcutKey` | 有 | 有 | 有 | 有 |
| 水印放置 | `direction` | `/camera/watermark/position` | 6 个位置 | 挂载处传 `supportWatermark:!1` | 同左 | 同左 |

- 黑框内容尺寸四个产品都是 220×132（由生成器解析各自的压缩常量并要求取值唯一，未硬编码）。
- 水印位置按原数组顺序生成：`left-bottom`、`center-bottom`、`right-bottom`、`left-top`、`center-top`、`right-top`；放置盘以 `disabled_unless` 绑定到同一个开关。
- 分辨率行的挂载状态按原代码区分：3592 任何页面都不挂载预览块，因此没有分辨率选择器；其余三个根各 10 项，与产品自身的能力探测回退列表一致。
- 标签取自各产品本地化模块的真实符号：`ZOOM`、`ZOOM_TOOLTIP_CONTENT`、`PAN_AND_TILT`、`RECENTER_THE_VIEW`、`PRESET`、`SHORTCUT_KEY`、`SHORTCUT_KEY_INPUT`、`LENS_DISTORTION_COMPENSATION_DESCRIPTION`、`WATERMARK`、`WATERMARK_DESCRIPTION`、`PREVIEW_RESOLUTION_2`。

## 视觉与交互还原

| 元素 | 原代码 | 本地实现 |
| --- | --- | --- |
| 页面列 | `.camera-container`：`#111`、`padding:27px 20px`、`width:400px` | `ProductSpec.layout = "camera"` 分支渲染同一尺寸的列 |
| 分隔线 | `.camera-divider`：`1px solid #222`、圆角 2、上下外边距 20 | 同色同距的空 div 边框 |
| 取景预设 | `.preset-container`：`grid`、间距 10、列宽 50；`.preset-item` 高 27、`1px #5d5d5d`、圆角 3、`12px/14px`、`#ccc`；选中 `#292929` + `#44d62c` | 同尺寸按钮网格，选中态同色 |
| 平移／倾斜名 | `.pan-and-tilt-name`：`14px/16px` | 同字号 |
| 黑框 | 内容 220×132，行内 `borderWidth:1px`，`#5d5d5d` | 同尺寸同边框 |
| 白框 | 宽 `220/zoom`、高 `132/zoom`，`#ccc`，位置来自 `uU`／`CU` | `PanTiltBox::white_origin` 逐行移植同一算式（含 `floor`、`round` 与越界钳制） |
| 方向键 | 10×10，`1px #5d5d5d`，圆角 2，朝向黑框的一侧无边框；中心键 15×15 | 同尺寸与去边一侧 |
| 快捷键框 | `.display-name.keyboard_listen`：`#111`、`1px #5d5d5d`、`14px/17px`、宽 210、内边距 5；悬停／激活 `#44d62c`；空态 `#707070` | 同尺寸同色，点击进入监听 |
| 快捷键语义 | 同时含修饰键与普通键才保留；单个按键补 `Ctrl + Shift`；清除按钮复位为空数组 | `capture_shortcut` 按同一规则写回 |
| 水印放置盘 | `.direction-container`：`grid`、间距 6、列宽 48、`margin-top:9px`；`.direction-item` `#222`、48×27；`.direction-line` `#ccc` 20×3，选中 `.active` 转 `#44d62c`；`left/right/center` 与 `top/bottom` 组合决定偏移 | 同尺寸网格；每格按位置计算四条边偏移，选中转主色 |
| 水印说明 | `.mode-description`：`14px/17px`，位于开关行内 | 同字号说明位于放置盘上方 |

输入 ID 与显示名来自产品自身的按键表（`src/features/mapping_keys.rs`，随 `shortcut_engine_keys.rs` 一同由源码提取），在 `source_controls.rs` 中反查，未自造键名。禁用支流的视觉沿用本仓库相机控件的既有处理（`opacity 0.3` 且不可交互）；原代码的 `disabled` 类在 CSS 中没有独立规则，未额外发明样式。

## 复合禁用条件（LDC × 分辨率）

四个当前根的取景组件里都有同一个条件函数，逐字对应：

```js
const gate = () => {
  let disabled = false;
  const resolution = LIST.find(e => e.width === current?.width && e.height === current?.height && e.fps === current?.fps);
  if (!ldc || (resolution.name !== "4K 30FPS" && resolution.name !== "1440p 30FPS")) { /* 不动作 */ } else { disabled = true; }
  return disabled;
};
```

`ldc` 来自 `advancedWebcamReducer.ldc`。同一个返回值同时接到：变焦步进器 `disabledStepper`、变焦滑块 `active`、平移/倾斜面板 `isDisabled`，以及包住五个预设与快捷键的 `className: gate() ? "disabled" : ""` 外壳；禁用时那行才渲染 `.mode-description`（文案 `LENS_DISTORTION_COMPENSATION_DESCRIPTION`）。

本地把这段编码成描述符里的 `disabled_when_any`（任一条件组全部成立即禁用），四个取景控件各带两组：

| 组 | 条件 |
| --- | --- |
| 1 | `/camera/ldc` == `true` 且 `/camera/resolution` == `{3840,2160,30}` |
| 2 | `/camera/ldc` == `true` 且 `/camera/resolution` == `{2560,1440,30}` |

两条分辨率取值由生成器按门槛里的名字（`4K 30FPS`、`1440p 30FPS`）回到该产品的分辨率字面量里核对，要求唯一；`/camera/ldc` 由同一组件的 selector 与门槛变量名必须一致来确认。3592 的默认配置正是 `ldc = true` + 4K 30FPS，因此它的取景块一打开就是原版的禁用态并显示 LDC 说明；3594／3595 的默认配置没有 `ldc` 字段（原版为 `undefined`，条件为假），3596 为 `false`，都保持可用，与原版一致。

## 白框拖动与黑框几何

原版取景框是一个绝对定位的 `.white-box`（宽高 `220/zoom`、`132/zoom`）叠在 `.black-box` 上，拖动逻辑分两处：

- 白框 `onMouseDown`：`setDragging(true)` 并记录抓取偏移 `left/top/bottom/right`（光标相对白框四条边的距离）；`onMouseUp` 复位并清空偏移。
- 黑框 `onMouseMove`：仅在拖动中、且白框小于黑框时响应。用 `left = clientX - blackRect.left - offset.left`、`top` 同理得到新位置，然后依次套用原版边界：`left/top < 0` 归零、`bottom >= 黑框高` 时 `top = 高 - 白框高 - 2`、`right >= 黑框宽` 时 `left = 宽 - 白框宽 - 2`；位置有变化才继续，用 `Tm`/`Im` 把像素回算成 pan/tilt 并写回状态。

本地按同一顺序实现（`drag_pan_tilt`），并把白框的像素位置改成每次由 `white_origin` 从 pan/tilt 重算——这正是原版 `useEffect(H)` 的效果，因此原版里那两行「钳进内容盒但只写本地 state」的赋值不再需要。像素换算两个方向都按原式移植：值 → 位置是 `Om`/`Am`（`white_origin`），位置 → 值是 `Tm`/`Im`（`pan_from_left`/`tilt_from_top`，含 `ceil` 与 `±maxPanTilt` 钳制）。

黑框自身的盒模型也对齐了原版：`.black-box` 是 `width:220px;height:132px` 加 `1px` 边框的 content-box，边框盒为 222×134；本地此前把 220×132 当作边框盒（内容盒只有 218×130），现在按 222×134 绘制，内容盒正好 220×132，白框坐标与钳制常数随之与原版一致。拖拽需要的黑框窗口坐标由 `canvas` 在布局阶段记录，对应原版的 `getBoundingClientRect()`；方向键的 ±1 步进此前已论证与原版一致（原版把 pan/tilt ±1 换算成像素再换算回来，线性映射下等价，钳制点也相同）。

## 数字步进器（2026-10-04 重新核对）

本轮按四个当前产品包的实际类、设置行适配器及完整 CSS 顺序重审，收据由 `tools/generate-native-camera-controls.cjs` 写入 [camera-stepper-current-evidence.json](camera-stepper-current-evidence.json)。此前只读取前段 CSS、并把设置行调用处的属性当作输入属性，所得的尺寸与小数结论作废。

| 项 | 当前源码的最终规则 | 本地实现 |
| --- | --- | --- |
| 外框 | 后段 `.stepper` 覆盖为 `62×26`、相对定位、`1px solid #5d5d5d`；悬停/内部焦点绿色边框 | 同尺寸与状态，颜色来自产品主题令牌 |
| 输入区 | `38×24`、`left:6px`、`padding:0`、`14px/14px`、`#ccc`；透明背景露出外框 `#111` | 保留 `InputState`，38px 文字区及左侧 6px，支持真实文字编辑 |
| 箭头 | `14×12`；SVG 固有 2:1 比例按 8px 背景宽缩为 `8×4`；上偏移 5px、下偏移 3px | 同图标和几何 |
| 显隐与按压 | 产品末段规则令上下箭头始终 `opacity:1;visibility:visible`；hover `#ffffff1a`、active `#0000001a` | 常显并应用两个状态令牌；不再使用过期的 hover-only 规则 |
| 范围边界 | 达最小/最大值时对应箭头 `opacity:.3;pointer-events:none` | 对应方向淡化并停止指针重复 |
| 长按 | 按下立即一步，每 300ms 重复，松开/离开停止，更新到范围边界时清计时器 | 保留任务，松开、移出、禁用及实体销毁均停止；抑制 GPUI NumberInput 在释放时的额外 click，避免双步进 |
| 输入、键盘 | 正负整数或至多三位小数草稿，可暂时为空/`-`；方向上/下步进，Enter/Escape 失焦提交 | 复用 `Input` 与 Base `NumberInput` 的文字/键盘行为，显式禁用默认 numeric mask 以保留原草稿 |
| 失焦归整 | 整数先向上归到步长再钳制；小数先钳制，再以 `1e-10` 容差判断是否向上归到步长 | 同一顺序；点击输入首次全选，后续点击可定位光标；聚焦时滚轮步进 |

变焦行确实传入 `allowDecimal:true, roundUpDecimals:true`，但四个当前包的共享设置行都只转发 `allowDecimals`，**没有转发 `roundUpDecimals`**。实际输入未在点击编辑状态时使用 `toFixed(3)`，所以变焦静止显示 `1.000`，不是按 0.1／0.01 步长显示一位／两位。描述符现在记录经过适配器后的有效属性；静态扫描同时校验实际输入类与转发对象，不再由调用处推测。

已挂载的步进器共 23 个：3592/3594/3595 各 6 个，3596 共 5 个。

- 四个根都含变焦、亮度、对比度、饱和度、白平衡。图像前三项 `maxLength:3`，白平衡最大长度来自该产品 `whiteBalance.max.toString().length`，变焦未传长度上限。整数行按源码保留手工输入字符串后 `e + stepValue` 的转换行为；例如未失焦的 `12` 再按上键会先形成 `121`，随后进行范围钳制。
- 3592/3594/3595 另直接挂载曝光补偿步进器，步长 0.1、`roundUpDecimals:true`、`maxLength:1`，绕过上述设置行适配器。它与补偿滑块共用值，测光模式改变时同步最小值（-3／-1）。长度限制只约束手工输入，程序步进仍可显示小数，保持源行为。
- 所有步进器与同一行滑块分别创建保留状态，初始化滑块不会再通过 `else if` 跳过数字编辑器。同步不会回发用户变更；小数写回消除滑块 f32 带来的显示长尾。
- 锐度与增益在当前四个根都未启用：3592 挂载点未传 props、默认 false，其余三个显式传 false。它们不属于待补的已挂载控件，收据见 `camera-controls-source.json` 的 `sharpness_gain_rows`。

## 验证

- `node tools/generate-native-camera-controls.cjs`：重新生成 7 份描述符。相机控件合计 `direction` 1、`pan_tilt` 4、`preset` 4、`keys` 4、`select` 6（3 个快门速度 + 3 个分辨率）、滑块 46。
- `python -X utf8 tools/validate-native-product-data.py`：通过；新种类、`tilt_path`、`box_width`、水印位置取值与门控路径均已纳入断言。
- `python -X utf8 tools/audit-native-product-coverage.py`：覆盖统计不变（331 产品、1419 页）；没有把本轮工作写成整页完成。
- 本轮 Rust 编译与格式检查由主线程统一执行；此处不把之前的检查结果当作本轮通过证据。
- 未运行应用、构建、测试、安装器或下载的 JavaScript，因此没有真实窗口的像素、焦点与拖拽验收结论。

## 仍然缺的部分

- 实时摄像头画面与设备枚举（`navigator.mediaDevices`）、Camo／NVIDIA Broadcast／XSplit 的第三方分支与「更高代线材」提示。
- 水印的外观与处理说明文案（`WATERMARK_AVAILABLE_*` 一类），以及 HDR／自动取景开启时的附加提示。
- 取景禁用的条件分支已实现（见「复合禁用条件」）；本地数字编辑器禁用时停止键盘、文字和计时器变更。禁用时黑框显隐及跨区域滚轮捕获尚无真实窗口验收。
- AI 自动取景（`autoFraming`）在本代根上都被传 `supportAutoFrame:!1`，没有挂载，因此未生成。
- 白框拖动与黑框鼠标移动分支：已实现（见「白框拖动与黑框几何」）。仍缺原版在拖动过程中对方向键上下键的额外处理、以及拖动时是否暂停取景预设动画。
- 方向键图标已按当前产品 manifest 获取并静态验哈希，上、下、中心键均使用真实资源，收据见 [camera-pan-assets-current-evidence.json](camera-pan-assets-current-evidence.json)。此前“资源不存在、仅有边框”的记录作废；尚未做运行窗口的裁剪与像素验收。
- 分辨率列表在开启能力探测的根（3595）上由设备能力过滤，本地使用产品自身的回退列表。

## Legacy Customize preview mount

The three earlier roots (3587 Kiyo, 3589 Kiyo Pro and 3590 Kiyo X) use the shared `TAB_CUSTOMIZE` camera widget. Static JSX evidence in each current bundle contains `.camera_setting .main_preview`; companion CSS fixes its surface to `background-color:#222` and `height:292px`, with a 520px video frame (`video_frame` declares `width=520`, `height=292`). The native renderer now uses `layout="legacy-camera"` for these descriptors and reserves that exact frame before image and focus controls. Because this workspace has no camera stream or device-enumeration transport, the frame displays an explicit unavailable state and never fabricates a still image or camera identity.

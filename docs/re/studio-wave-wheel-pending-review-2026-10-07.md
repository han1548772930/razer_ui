# Studio Wave / Wheel 及剩余属性根静态复查（2026-10-07）

本批只读当前源码和已有实现，不修改 Rust。Wave、Wheel 优先细查；随后读完 Tidal、Audio、Chroma Generate 的实际挂载根和直接交互。五个根在 `StudioProperties::render` 中仍未挂载，所有下列控件都是明确的 UI 待办。描述符、参数默认值、共享组件已经存在均不能把这些控件计为完成。

[独立矩阵与收据](studio-pending-properties-current-review.json)记录 `4264:se` 的真实懒加载入口、每个根及其子组件、JSX 数值范围、相关共享模块、中心点画布生产者、CSS 与资源。当前共 5 个缺失属性根、86 项 AST、7 个 CSS 文件/102 条规则、19 个引用资源，其中 15 个源资源文件尚未下载。它们仅做待准备记录，没有拿近似图标代替，没有执行源码或下载资源。

## Wave：1591，J → k → S / E / 5805

| 控件 | 当前源码 | 原生缺口和实现边界 |
| --- | --- | --- |
| Gradient | `S → 3690`，180px，默认预设族，五组预设，最少 1/最多 7 色标 | 当前共享编辑器已支持 Default，但 Wave 没有挂载、回填或提交链；不能据此记为已有 Wave UI |
| Speed | `E → 768`，0–50，步长 1，默认 15；Change 写 params.speed，AfterChange 额外请求预览 | 下限与 Ripple 的 1 不同；需独立描述、数字/步进/滑条及两端标签 |
| Pause (sec) | Speed 组件右侧的 `2245` 数字框，0–60，默认 0；显示 pause/1000，提交 Number(value)×1000 | 不能当毫秒直接显示，也不能改成独立全宽滑条；与速度在同一 input-group-stepper 行 |
| Width (%) | 10–400，步长 1，默认 100 | 不能沿用 Ripple 的 100–400/100；右侧还有 Split 控件 |
| Split | `5196` 原版 switch，checked=params.split，直接提交 boolean；无根内附加禁用条件 | 必须接开关、状态、焦点和源宽度行布局，不能只保留默认字段 |
| Angle (°) | `v → p → h/x`；80px 圆盘和 0–359 数字框，默认 90 | 原生角度圆盘、数值联动、跨窗口拖动/释放、ResizeObserver 对应布局更新均缺 |
| Playback | `5805`；默认 random/never/-1 | 共享组件已存在但 Wave 未挂入；进入/离开效果时须清除该源实例的菜单/循环记忆 |

角度指针不是普通滑条：`x` 取实际框中心后在两个轴各减 2px，以 `atan2` 求角，`1981:t_` 使用 **Math.floor** 转为度数，再映射为上方 0°、顺时针增加。thumb 的位置按半径 `clientWidth/2−10` 和自身半宽计算，并用 Math.round 定位；容器尺寸变化重新计算。指针是 16px，背景颜色有 100ms ease-in-out 变化。实现时不得用没有依据的圆心、四舍五入角度或固定 thumb 坐标替代。

源码 mousedown 先修改临时值，再判断是否为中/右键以决定注册 window move/up；拖动仅松开时调用 afterChange。它的事件清理和非主键行为也应单列审查，不能凭常见旋钮经验推定与现有 GPUI 组件相同。当前尚无原生实现或运行验证。

## Wheel：8379，k → N → C / y / 5805

| 控件 | 当前源码 | 原生缺口和实现边界 |
| --- | --- | --- |
| Gradient | `C → 3690`，默认 1/7 预设族、180px | 缺实际挂载及局部编辑/提交 |
| Speed | `y → 768`，0–360，步长 1，默认 60 | 不用 Ripple 的范围；需要数字/步进/滑条、标签和预览请求边界 |
| Center Point 入口 | 按钮切换 `editor.isCenterPointActive`，不是 params 字段 | 缺可切换按钮、源选中/按下 0.9 遮罩和后续画布操作 |
| Direction | 读取 `effectLayer.params2.counterclockwise`，调用 `9286:XF` | 整个 `.btn-group-toggle` 的 click 都翻转 boolean；不是“点已选项无操作”的普通 radio。需要两个源方向图标、30px 高/最少 104px 容器和选中样式 |
| Playback | 默认 random/never/-1 | 缺实际挂载；共享能力不等于本页完成 |

中心点不是只有一个按钮。当前 `4264:vt` 的 `getCursorOnCanvas` 按缩放和画布偏移换算并四舍五入坐标；`editorOnMouseUp` 在中心点模式开启、目标非中心点自身、坐标在画布范围内时，写入独立 params2 的 `centerPointX/Y`、`centerPointX2/Y2` 和 `deviceWithCenterPoint`。坐标再通过 `1981:DO` 按 AT 缩放为逻辑坐标；设备命中 `1981:sO` 只从真实 device.items 找包含该点的设备，没有命中则为 null。

画布 `renderCenterPoint` 仅对 Wheel/Tidal 挂载，使用 params2 的原像素坐标和方向类；Select 模式下按住该点可以重新激活中心点模式。当前原生画布、模型和属性面板没有这些 params2/中心点能力。`2904:pR` 的默认值与查询真实设备命中分开；不能用目录产品或猜测设备造出命中结果。`9286:XF` 的源 previewEffect 调用也不能被写成实际预览成功。

## 另外三个根的直接差异

| 根 | 实际直接控件 | 不能直接复用的差异 |
| --- | --- | --- |
| Tidal 8154 | 两个单色、Random、0–50 Speed、Center Point、Direction、Playback | 两色采用 `color ? color : FU`，黑色也折为 FU；不是 Reactive 的保留黑色规则。中心模式字段为 `isTidalCenterPointActive`；方向显示 Outward/Inward，但仍用 params2.counterclockwise；与 Wheel 共用画布生产者，图标/类不同 |
| Audio 9120 | Audio 专用 Gradient，低/高端标签，Boost、Auto Gain、Decay | 渐变有自己的 4 组预设，最少 1/最多 7；不能归为 Default。Boost 0.25–4/0.25，Decay 0.1–2/0.1，允许小数输入；Auto Gain 仅禁用 Boost。新整数 numeric 的字符过滤不能原样用于这里；当前 GradientKind 也尚无 Audio 分支 |
| Chroma Generate 2474 | 文件选择/拖放、图像或静音循环视频预览、替换/删除、Generate、Previous/Next | 全部 UI 未挂载。文件选择 accept 是 `image/*, .mp4`，拖放入口另按 MIME 接受 image/video；要保留不同入口条件。选文件本身先改局部状态，新文件清空配置历史；Generate 才加入 profile.mediaList 并提交三个配置字段 |

Generate 根的生成动作在当前源码中构造随机配置参数，而不是在此直接调用 AI 服务。配置含 colorSource、offset、rotation、scale、mirror/flip、pattern 等；图像的 blend 固定 Pattern，视频有 15 类 blend。最多保留 5 个配置，满时丢最旧项；Previous/Next 按当前索引与媒体存在性禁用。文件身份是 `window.btoa(encodeURIComponent(JSON.stringify(path)))`，不能改称路径哈希。删除清媒体和配置，并走源 mediaList 删除动作。UI、本地文件预览和本地参数历史仍在本期范围，不能因为最终引擎/DLL 写回后置而把整个 Generate 功能永久禁用。

## 资源与全局边界

19 个相关引用中，中心点入口/四种画布中心点方向图、顺/逆时针灰/黑图标、向内/向外灰/黑图标及 Generate add-white 等共 15 个原文件尚未准备；矩阵逐条保留当前 manifest 路径。Wave 圆盘本身来自 CSS 几何，不需要虚构一张背景图。后续实现应从当前 manifest 准备这些资源，再验证源/本地字节。

五个属性根仍服从 `4264:se` 的真实设备选择/工具条件。UI 操作、本地工作参数和只读真实状态必须分开；params 与 params2 不能合并成同一随意 JSON 字段。只有 DLL 修改设备/服务和持久化的腿后置，界面编辑、局部状态、Apply/Save 流程及真实读取接口继续在目标内。

完整可见 Studio 页、图层编辑、画布、导入/导出及全部其他产品/页面仍在总队列；本报告不将五个源码根当成任何完成率，也不替代逐控件实现与窗口验收。本批只进行了静态源解析/读取，未运行应用、测试、厂商 JS 或 DLL。

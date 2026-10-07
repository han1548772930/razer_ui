# OLED 当前界面契约

691 与 1383 使用各自当前 manifest 声明的源码和资源，不能因控件相似就互相替代证据。本文记录当前实际界面、草稿边界和剩余缺口；源码默认示例与真实设备观察分别处理，静态收据通过不表示页面完成。

## 691 页面与状态归属

691 入口通过懒加载挂载 82189；基础 CSS 为 5171、导航为 7668，Customize/OLED/Power 各有自己的 chunk。`body,html` 为 Roboto 16px、`#ccc`、`#222`，body-wrapper 为 `10px 20px 20px`。OLED/Power/Help 的 `isEnableProfileBar=false` 只影响同步图标，配置栏仍挂载，下拉由独立状态控制。源链、CSS、拨盘与配置栏证据见[691 收据](keyboard-691-current-evidence.json)。

691 的 OLED reducer 独立于 DEFAULTPROFILE，本地使用 `source_device_settings.oled`。切换 profile 不更换这份设备设置；保存和放弃包含该镜像。设备字段迁移时既有设备值优先，其次为活动 profile，其他 profile 仅补缺失值，所有 profile 中的旧副本移除。3595 的 deviceMic 使用同一迁移边界，不能把本地存储 envelope 说成厂商 profile 字段。

主页卡区宽 1220px，窄屏为 600px、左右内距 25px；下方为两个 600px 列。Home 在两列上方，亮度/语言在左，回主屏/变暗/屏保在右。标题 RazerF5 16px，widget 正文 Roboto 14px；标题开关与右上 10/10 的 14px 帮助使用各自源文案。

| 设置 | 当前本地行为 |
| --- | --- |
| 亮度 | 20–100、step 1、源初值 50；64px 容器、6px 轨道、16px 手柄和数值提示 |
| 语言 | English / Simplified Chinese；选择暂存，Apply 才提交，离页丢弃未提交选择；补码字节按 `raw < 127 ? raw : 255 & ~raw` 解码；BLE 禁用 |
| 回主屏 | 5/10/15/20 秒，源初值 5 |
| 变暗 | 1/2/3/4 分钟，源初值 1；按源 enabled 禁用，不另加不存在的启用开关 |
| 屏保 | 0–3；0 为无屏保，1–3 为原动画；两列 260×68 卡、10px 间隙、256×64 图像 |

屏保设备协议仍需把 0 转成 disabled，把 1–3 转成 enabled 和 value 0–2。原 GIF 的 270/300/300 帧与转换 WebP 的帧像素、时长由准备工具验证。具体默认值及调用点见[OLED 控件收据](keyboard-oled-current-evidence.json)。

## 691 主页、预设与提示

七卡依次为 Animation(0)、Image(1)、Emote(4)、Banner(2)、Media(5)、System(6)、Keyboard(3)，默认启用并选择 Animation。每卡宽 236px、标题 19px、内容 232×64；选中与 hover 保留源边框顺序，标题 hover 优先于 selected。关闭 Home 保留选择并阻止编辑。

Animation/Image/Media/System 均已有实际编辑入口。Emote/Banner 的内容仍为空，Edit 禁用；Keyboard 使用当前内联原图且无 Edit。BLE 禁用 Media/System 卡，并禁止 Animation/Image 等编辑，但允许应用源准许的已有内容。这是 691 的规则，不能套用到 1383。

Animation 六槽与 Image 十槽使用当前资源与 ID。编辑器持有克隆草稿，至少留一个启用槽；关闭选中槽后选第一个启用项。Crop Apply 只更新该槽草稿，编辑器 Apply 才写入设备 OLED 镜像，Cancel/Escape/关闭丢弃。卡内开关阻止事件传播，不附带选择；Reset 只用于启用的自定义槽并清除原数据、size 与 crop 字段。

预设网格为三列、20px 间隙、顶部 30px。普通卡为 1px 边框加 1px margin，选中/hover 改为 2px/0；disabled 的后置规则为 `#5f5f5f4d`、图像 10% 透明度。hover 以裁剪中心放大 1.1，并显示 50% 黑底工具栏：左上启用、右上 Replace、左下 Reset。原 Replace/Reset SVG 已注册。hover 不修改保存的裁剪几何，也不重启动画身份。

Requires Synapse 图标以 `isMounted` 对应的 BottomRight 提示即时挂载。BLE 卡提示使用 `SourceTooltipKind::OledBleDisabled`：黑底、`#5d5d5d` 边框、14/16px、padding 8/10、left 20/top 185、300ms linear；卡内容的 .5 透明度不影响提示。BLE 编辑按钮也使用该 kind，但其触发目标是按钮，源卡片伪元素与本地按钮目标的完整几何仍须核对，不能把相同偏移常数当成像素一致。

以上源定位、原资源及原生调用由[预设卡片收据](oled-preset-cards-current-evidence.json)、[BLE 提示收据](oled-ble-tooltip-current-evidence.json)和[691 资源收据](keyboard-691-assets-current-evidence.json)记录。

## 691 媒体与系统编辑器

Media 暂存 info/visualizer；文字可放 top/bottom，可视化有三种原图。关闭任一区域会打开另一区域，二者不能同时关闭；Reset 为文字启用/top、可视化启用/0。内容为 232×64，可视化卡 232×44；原 GIF 保留 696×132 栅格与帧时长，按源缩放显示。该编辑器要求非 BLE 且 Home 启用，只提交本地 `oled.media`，不声明音轨或设备传输成功。证据见[媒体收据](oled-media-current-evidence.json)。

System 已挂载本地编辑器，包含三张双槽幻灯片、九个源标签及协议 type、3/5/10 秒、摄氏/华氏、三种日期格式和 12H/24H。只有 Apply 写入设备所有的 `oled.system`；源默认 CPU/GPU/日期等样例只用于预览，不是实时采样。

System 仍用文本预览，九份当前源 SVG 未接到该 691 renderer：cpu_usage、cpu_temp、gpu_usage、gpu_temp、memory、date、time、laptop_battery、keyboard_battery。不能因为 1383 有自己的 System 资源就视为 691 已完成。原始 System 卡呈现、拖放/槽位全流程、幻灯片服务调度与语言更新仍需逐项对照；已挂载编辑器不等于完整复刻。源默认与编辑器描述符保留在[691 编辑数据](../../src/features/keyboard_oled_editor_data.json)。

## 691 本地裁剪与资源处理

动画接受 GIF，图片接受 PNG/JPG/JPEG/BMP。文件读取、base64 和解码在后台执行；旧文件选择结果、已销毁编辑器或禁用槽不能接纳晚到结果。原数据 URL 和原字节数作为本地草稿保存，不能冒充处理后的上传数据。

当前裁剪容器 232×190，固定 crop box 为 left 0/top 63/232×64，viewMode 0、dragMode move。图片先 contain，再按最小 canvas 高 64 保持比例；允许裁剪范围出现黑边。拖动只移动 canvas，边界为 left `[−canvasWidth,232]`、top `[−canvasHeight,190]`；窗口级 release 清理拖动，坐标按 UI 缩放换回 CSS 单位。键盘补充 1px/Shift 10px 移动。

1–10 滑条表示源控制位置，并非绝对倍数。按钮传 ±0.1；滑条减少传 `(next−11)/10`，增加传 `next/10`；负值因子 `1/(1−input)`、正值 `1+input`，乘当前比例。zoom 围绕 canvas 中心，触及最小高度时保留源位置规则；活动几何没有 10 倍上限。Reset 恢复初始矩形及位置 1。

Apply 保存 `local_crop.canvas` 的 width/height/left/top，主页和预设使用同一矩形并减 crop-top 63。旧 zoom-only 草稿保留兼容显示；无效矩形回退。232×190 的遮罩、crop-face、虚线与角标已绘制，几何和回调依据见[裁剪收据](oled-canvas-current-evidence.json)、[裁剪/语言收据](oled-crop-language-current-evidence.json)。

本地 data URL 经原生 Asset 解码，不送入 HTTP。slot/content 稳定 ID 保持 GIF 帧状态；显示者持有共享 lease，最后一个释放时移除缓存。输入限 64MiB，静态检查格式尺寸、GIF 帧/子块边界及估算 256MiB RGBA 驻留量，解码后再核帧画布；这不是进程内存硬上限。极端 canvas 边界以 2^40 CSS 像素保护布局，属于本地资源约束。

worker、ImageMagick JS、WASM、动画及 source-map 响应均有[资源收据](oled-worker-resources-current-evidence.json)。WASM 只检查头与段边界；两份 source map 的 404 保留为缺失证据。当前 worker 的 coalesce、150 帧上限、20 帧批次、黑色画布、灰度/232×64、最小 GIF delay 和 optimizePlus 仅为[静态处理契约](oled-worker-current-evidence.json)，没有执行或声称实现编码。处理后 GIF/图片、帧数、字节数、传输估时与设备上传仍缺。

## 691 Power 与命令拨盘

Power 的 dim/sleep 两个 widget 已有标题开关、帮助和 48×27 原数字按钮；低电量 OLED 提醒、indicator、低功耗模式信息三个 widget 仍缺。Customize/Lighting/Help 内部结构也需继续逐页完成。

拨盘帮助使用 14px `#4a4a4a` 圆、right/top 10、BottomRight 即时提示，富文本行高 17px、列表缩进 20px。Add 使用全局 `[tooltip]` 样式：下沿 5px、右缘对齐、不换行、14/16px、padding 8/10、300ms linear，且不叠加 Kit 提示。Delete 确认打开时隐藏提示；该触发闭包的 group-hover 实现仍缺 .3s 淡入。Expand/Reset 的源提示标记未完成核实，保留明确差异；不据通用样式推定对应源行为。

691 的语言下载进度、取消确认、设备标签更新、System 服务提交与真实 transport 未接通；1383 的 runtime 模块不能算作 691 已完成。原滑条 Release/hover、提示祖先裁剪、字体 normal/fallback 与窗口输入仍需验证。

## 1383 当前页面与编辑器

1383 `xx` 使用自己的 OLED 根。左列亮度/语言，右列回主屏/变暗/屏保；七卡在两列上方。亮度为 30–100/step 1；其 `.slider` 手柄本身是 16px 圆点，14×20 的 path SVG 属于另一种 slider-more。Home/Dim 为 48×27 数字按钮；dim 禁用层 300×30、`#111`/.5；屏保两列 260×68，ID 与保存值 0–3 相同。来源见[五控件收据](audio-oled-1383-current-evidence.json)。

七卡为 Animation/Image/Emote/Banner/Audio Meter/System Info/Headset Info。桌面区 1220px、≤1279px 为 600px 与左右 25px；卡框 236px、标题 19px、内容 232×64。Home 关闭时有 80% `#111` 遮罩；hover EDIT/APPLY 层使用原 24px 图标、8px 间距、12/14px 文字，preview 与 overlay 共用 hover owner。源 reducer 初值足以显示卡片，不能将 GET 未返回解释成所有卡应为空。

六个可编辑模式均已有实际编辑器，Headset Info 无 Edit。动画/图片保留固定槽与本地裁剪；Emote 有 104 个源表情及搜索；Banner 有原图、字体、样式和四方向滚动；System 有三张双槽、菜单/拖放/格式；Media 有区域开关与三种可视化。全部独立暂存，Apply 才写入本地 `oledHome`，Cancel/关闭丢弃；完整细节见[编辑器契约](audio-oled-editors-current.md)。

源初值的 System CPU/GPU/日期样例和 Headset 电池初值不是设备报告。Banner 按 `3400 + 28 × scrollWidth` 周期，原型与字体测量生命周期以专项为准。公共弹层 Mv/Tn 保留分支 800/850px、100px top +110px margin、36px 标题、源码入场时序及直接卸载关闭；Requires Synapse portal 按先右后左、8px 边界修正，不新增纵向翻转。

1383 的 RazerF5 SemiBold/600 为真实字体面。Home 的 27 项资源使用自己 manifest 的六动画、十图片、默认表情/横幅、可视化、headset PNG 和 SVG；源 GIF 与 WebP 区分原字节相等和解码转换。来源见[Home 收据](audio-oled-home-source.json)、[共享图标](audio-oled-home-shared-assets.json)、[屏保原图](audio-oled-screensaver-assets.json)。全部编辑器 176 项资源及细分验证由编辑器契约记录。

## 1383 观察驱动的运行状态

`OledRuntimeObservation` 接收实际 Connection、Loading、Error、Language；`OledRuntimeRequested` 暴露源消息名和 payload。Audio → SourceWorkspace → ProductWorkspace 转发两端，进入 TAB_OLED 发源 GET 对应的 `ON_SET_OLED_DATA_TO_UI`。已存在这些 UI/事件路径，真实 provider 仍未接通；不能把事件发出说成设备接受。

| 分支 | 当前行为 |
| --- | --- |
| 连接初值/警告 | isBle=false、isDongle=false 来自源 reducer；不伪造 dongle。非 dongle 警告 400×117、无 footer/关闭/Escape/背景关闭，源橙色外观和 top 100 |
| Loading | 源 simple loader 的两秒旋转和 dash；动画/图片/reset 进度与语言进度面板；reset 的 Cancel All 禁用。300ms 进度下降时先完成前项、1ms 复位再显示新观察，不改变实际 progress |
| 语言取消 | CANCEL 发 `ON_CANCEL_OLED_LANGUAGE_UPDATE` 并只关确认；CONTINUE UPDATE 只关确认，不清进度或生成完成 |
| 错误/重试 | 普通 RETRY/REVERT 携观察 payload；reset retry 保留 error、隐藏 warning 并发源 RESET；reset revert 先做源 cancel-download 状态迁移 |
| BLE/进度 | progress 将 Home 控件 .3；BLE 将 Media/System .5 并禁用 hover/Requires Synapse 提示；1383 artwork/banner Edit 按源只变灰、仍可点击，未添加不存在的 ble-edit hover |
| 语言暂存 | 区分 raw/staged/decoded；BLE/progress 禁用；raw 改变才替换 staged，同 raw 仅计数更新保留用户选择；离页重开清 staged，profile restore 保留已挂载选择并取 retained runtime 语言 |

只有实际 MW Language 观察推进 changed；OLED 挂载、选 System 且 changed>1 时才按源重映射标签。Reset/上传/语言下载请求、重试和本地进度动画均不是合成设备成功。精确 AST/CSS、语言生命周期及两端接线见[运行状态收据](audio-oled-runtime-current-evidence.json)。

## 当前验证边界

维护工具以 Acorn/CSS、源/资源哈希、静态原生契约和媒体解码核实，不执行供应商 JavaScript、WASM 或 DLL。允许格式化、资源准备和 `cargo check --locked --all-targets`，不运行应用、构建或测试。

原 worker 编码、真实读取发布者、语言下载/上传服务、设备写回和完整视觉/焦点/滚动仍未完成。读取与状态观察、所有 UI 编辑和本地 Apply/Save 继续在范围内，DLL mutation/write-back 后置；不能因服务未接通删去已有 UI，也不能把本地 Apply 表述为设备成功。

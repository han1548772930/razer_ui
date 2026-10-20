# Chroma Studio：图层、灯珠草稿与真实输出

Studio 把真实连接设备的几何与灯珠身份放到同一画布，以配置档保存设备位置、图层、区域和媒体。编辑态先驱动独立 Preview；Save 把当前草稿保存为本机配置档，再交 Advanced 引擎输出。画布上出现颜色、本机保存完成、引擎启动和设备收到帧是不同结果。本页只展开本机页面、文件、存储、服务与系统能力；账号、云同步、在线分享及在线获取流程不在范围内。

## 实际挂载和布局

当前入口的 Provider 下依次挂错误边界、Shell、页面路由、观察组件及附属节点。普通页面 lazy 挂模块 4264 的 EditorCanvas；Help、Onboarding 有各自路由，不能把它们当作 EditorCanvas 内的一张效果表单。争用提示独立观察 Chroma Apps 控制权。

EditorCanvas 的实际组合为：

| 区域 | 当前内容与尺寸 |
| --- | --- |
| 左栏 | Effect Layers、Add Group、图层列表及 Effects List；初宽 250 px，拖动夹在 250…450 px，窗口宽度不超过 600 px 时恢复 250 px |
| 中栏 | 40 px 高的 header、选择／画笔／填充／移动工具、历史操作、设备画布、底部 Preview 与缩放 |
| 右栏 | 当前效果名称与 Reset、效果专有参数、Save；基础宽度 250 px |
| 画布 | 10000×10000 逻辑像素、40 px 格、红色中心轴；设备 SVG 及灯珠处于同一坐标系 |

面板与 header 背景为 `#222`，header 分隔线 `#383838`；编辑区背景 `#5d5d5d`，画布背景 `#383838`。右栏在 Select／Move 工具没有选中灯珠时灰态，Save 由实际脏草稿状态控制。左栏 Reactive、Ripple 要求至少存在 mouse、keyboard、keypad 或 system 类设备。当前构建的禁用高级效果数组为空，Chroma Generate、Tidal 均在实际效果列表内。

## 设备读取与本机配置档初始化

观察组件先处理安装状态、本机文档及设备数据，再初始化编辑器。EditorCanvas 取得真实 LE 设备列表，按设备配置加载几何／SVG，转换坐标后加入 reducer；后续分别消费 add、update、remove 和电源事件。同一稳定设备 ID 防止重复；移除时还有重复设备检查。设备恢复供电会清除旧帧缓存。

默认配置档的创建受真实条件约束：设备已就绪、安装已完成且当前配置档没有 ID，才由 `addProfile` 发起。新配置档使用当时的真实设备列表、12 字符 ID 和默认 Spectrum 图层；首个配置档另带 `default` 标记。这是在主机编辑功能内建立配置档，不能扩写为读取失败、无本机文件或重连时用默认灯效覆盖设备。

当前原件使用 `currentUserId` 选择本机文档所有者。共享索引为 `synapseChromaStudioUsers`，本机 IDB 字段为 `synapseChromaStudioProfileReducer`；`setRemote` 同时执行共享索引和本机保存，函数名本身不能证明全部操作属于远端。配置档摘要与完整内容分别保存，当前选择通过稳定 ID 连接两者。

## 草稿、选择和历史

以下状态由各自 reducer 持有，不是一个“当前灯光对象”：

| 状态 | 内容 |
| --- | --- |
| editor | 当前工具、zoom、拖动／平移、脏草稿、Preview 开关及速度 |
| device | 已取得设备、几何缓存、选中灯珠、正在移动的设备 |
| effectLayer | 图层、当前图层、区域参数、中心点等 `params2`、混合字段、复制内容 |
| profile | 已保存列表、当前配置档、媒体表、待清理媒体 |
| history | past／present／future；当前构建历史上限 20 |

设备 ID、配置档 ID、图层 ID 和灯珠的 row／col 承担领域身份。界面名称、排序下标、SVG 填色均不能代替这些身份。多选区域参数不一致时记录 `paramsMixed`；未修改的混合字段会保留各原区域值，不能以统一默认值覆盖。

Select 进行灯珠／矩形选择，Ctrl 保留并切换已有选择，双击按区域选择；Pen 连续修改经过的灯珠，Bucket 按同区域或整个设备的 hovered 灯珠应用效果。Move 调整设备位置，夹在画布范围内、按 40 px 格吸附并检查设备碰撞及附属设备挂接。单指触摸仅在 Select／Move 条件下复用对应拖动；Space 另触发画布平移。设备旋转、中心点选择和移除都通过各自命令修改草稿。一次编辑更新 Preview、脏状态及历史，尚未变成已保存配置档。

输入控件外的主要快捷键如下；当前 Chroma App 正在控制时，快捷键消费者先退出。

| 输入 | 动作 |
| --- | --- |
| S／P／B／M | Select／Pen／Bucket／Move |
| Ctrl+C／Ctrl+V | 当前参数复制／粘贴 |
| Ctrl+Shift+C／Ctrl+Shift+V | 跨层复制／粘贴 |
| Delete／Backspace | 清除选中灯珠效果 |
| Ctrl+Z／Ctrl+Shift+Z | Undo／Redo |
| Ctrl+S／Enter | 有脏草稿时保存；Enter 要求不处于文本输入 |
| Ctrl+F／Ctrl+Shift+F | 100%／适合全部设备 |
| Ctrl+E／Ctrl+D | 左栏当前项重命名／复制图层 |

Ctrl+L 切换设备名称显示。文本输入、contentEditable 与普通画布焦点有明确区分，不能用一个全窗口裸按键处理器替代。

## 配置档动作与 Save／Discard

新增建立新 ID 和默认图层；Duplicate 建立新 ID、生成不重名标题并移除 `default`。Rename 保留 ID，拒绝已有标题，并更新本机 linkedGames 中对应名称。Delete 移除 linkedGames 引用，选择删除位置的后一项或前一项。切换配置档先处理未保存修改，再投影所选配置、清灯珠选择和历史、重建引擎及缩放。

Save 取得当前图层草稿和当前设备位置，保留灯珠引用，清理确实不再引用的媒体，移除 `default`，先更新 reducer，再保存共享索引／本机 IDB，最后重建 Advanced／Preview。部分内部调用可显式不持久化或不重建，不能把这些参数分支写成全部 Save 都同一步骤。普通保存还检查当前设备是否为空。

配置档切换、新增、复制等动作遇到脏草稿会激活窗口并弹 Save／Discard。Save 先提交草稿，再继续原动作；Discard 丢弃当前编辑后继续；关闭弹窗不执行后续动作。右栏的 beforeunload／退出拒绝消费者同样触发该弹窗。单独 Discard 命令把 `profile.current` 重新投影到编辑态，重置历史并恢复引擎，不是写一份出厂默认值。

原件有先 reducer、后异步保存，以及部分保存调用不等待完成的路径。因此 Save 按钮恢复禁用不能证明磁盘提交、引擎应用和硬件传输都已成功；Rust 实现须保留具体结果，不能拿本机保存冒充设备确认。

## 图层、组与效果区域

图层有稳定 ID、`type:layer`、效果身份、visible、参数和 regions；每个 region 包含自身参数及设备／灯珠集合。组是 `type:group` 与子图层列表。左栏支持增加、重命名、复制、删除、显示／隐藏、改变效果，以及 before／after／add 拖放；组不能嵌套，也不能把组移动到其他组内。复制组会给子图层重新分配 ID。

区域编辑先从旧 region 移除本次所选灯珠，再将本次参数与相同参数的区域合并；清除效果仅移除选定灯珠，随后删除空区域。区域之间的参数差异、中心点坐标及混合字段分别处理。复制参数受当前灯珠／图层数量限制，跨层复制不是简单地把整个 profile JSON 覆盖到目标。

下表保持当前左栏顺序。ID 是交给效果引擎的身份，不是列表下标；默认值只用于正式新建／重置草稿。

| 效果／ID | 当前可编辑内容与有依据的范围 |
| --- | --- |
| Ambient／11 | full、left、top、right、bottom、custom 屏幕区域；blur 1…9，默认 5 |
| Audio／12 | 渐变；boost 0.25…4，步长 0.25，默认 1；Auto Boost；decay 0.1…2，步长 0.1，默认 1 |
| Breathing／5 | 双颜色或随机色；duration `[28000,14000,6000]` ms，默认 14000；播放条件 |
| Chroma Generate／18 | 本机图片／视频、程序生成配置、配置历史；详见下一节 |
| Fire／8 | 两种颜色，默认 `0xff0000` 与 `0xff8000` |
| Reactive／3 | 颜色或随机色；duration `[2000,1500,500]` ms，默认 1500 |
| Ripple／1 | 渐变、speed 1…50 默认 10、width 100…400 默认 300、播放条件 |
| Spectrum／4 | 渐变；duration `[75480,37740,18870]` ms，默认 37740 |
| Starlight／7 | 渐变或随机色；density 1…10 默认 1；duration `[3000,1700,500]` ms，默认 1700 |
| Static／6 | 单颜色及本机自定义色 |
| Tidal／19 | 双颜色或随机色、speed 0…50 默认 10、split 默认 true、angle 默认 160、中心点／内外方向及播放条件 |
| Wave／2 | 渐变、speed 0…50 默认 15、pause 0…60 秒、width 10…400 默认 100、split、angle 0…359 默认 90、播放条件 |
| Wheel／13 | 渐变、speed 0…360 默认 60、中心点、顺／逆时针及播放条件 |

渐变／单色 picker 先保留组件临时状态，`onAfterChange` 再进入区域命令。slider 的连续变化与最终提交使用不同参数，不能把拖动中每个值误称为一次最终保存。随机色会禁用相应手动色；Auto Boost 禁用手动 boost。混合值通过实际字段集合呈现。

播放条件由共享组件处理 random／onPress／onSelectedKeys，以及 never／after／toggle 和 cycles；每种效果只挂载自身原件中存在的控制，不能因为共享枚举存在就为所有效果添加播放面板。Wheel／Tidal 的画布中心点与效果参数分开保存，LE 对特殊产品还有中心点 Y 偏移分支。

## Chroma Generate 的本机媒体

当前组件接受 `image/*,.mp4`，支持文件选择和拖放。FileReader 读取 Data URL，路径派生媒体键，图片或循环视频用于本机预览；媒体内容保存在 profile 的 `mediaList`。删除媒体先检查其他效果区域是否仍引用，Save 才清理确定不用的内容。

Generate 使用 `Math.random` 组合 pattern、blend、scale、rotation、mirror、flip 等引擎参数；当前有 15 个 blend、9 个 pattern，历史最多 5 项，Prev／Next 改变当前配置。`colorSource` 的 `ai`／`procedural` 是传给引擎的 token，这个组件没有远程生成请求，不能因名称而删除本机功能。

## 本地文件导入和导出

Export 默认只勾当前配置档，可单选／全选；输出带 `version:1` 和 profiles 的 `.ChromaEffects` JSON，经本机保存对话框写文件。导出补充当前设备名称，并将内部 row／col 补成输入或区域身份，使文件可以在设备身份变化时重新映射。取消仅结束本机文件操作，没有设备写入。

Import 先解析 JSON，失败后才尝试 ZIP 中的旧 XML。转换包括 40 倍设备坐标、效果名／ID、图层／组顺序、颜色、播放条件及区域灯珠。导入项一律建立新配置档 ID；设备优先匹配稳定 ID，其次 productId／instanceId，再按设备类别及名称规则匹配剩余设备。keyboard／system 还核对 layout。

设备替换时按按键 inputType／inputData 或区域身份映射灯珠，找不到对应项时当前原件有采用目标灯珠集合的具体分支；不能将该分支扩大成读取失败默认覆盖。页面列出缺失／不兼容设备和 tooltip，只导入勾选项，并剔除 missing 设备。Cancel／关闭只丢弃本次导入选择；未点击 Import 不应修改已保存配置档。

## Ambient 区域和系统颜色拾取

Ambient 的 Edit 先最小化本机窗口，再启动区域选择。真实 `onRegionSelected` 先停止选择，再读取桌面尺寸，将四角归一到 0…65535，更新 `screenRect` 草稿并恢复窗口；取消只恢复窗口。区域预览在 hover 显示，离开关闭。

当前原生原件的 GetScreenResolution 用 GetDesktopWindow／GetWindowRect，返回 right、bottom 文本，调用方释放返回内存。选择窗口读取系统尺寸、置顶，安装低级鼠标／键盘 hook，由消息循环处理；回调把四角整理为 left、top、right、bottom。Stop 卸 hook、关窗口、等待线程并释放对象。颜色窗口用 GetPixel 采样，将 COLORREF 转为 `0xRRGGBB`；移动持续回调，鼠标释放触发完成并释放 DC。完成消费者先 closeColorPicker，再通知页面。

这些是纯 Rust 平台适配需独立实现的本机能力。原件 DLL、FFI wrapper 仅作为静态证据，不能成为产品后端。当前前端部分调用返回 true，部分回调只在 beforeunload 注销；普通效果卸载没有证明所有 native callback 已解除，不能补出统一清理成功。

## Preview、Advanced 和真实灯光帧

Preview 有独立开关；Normal 为 40 ms，Slow 为 100 ms。关闭会销毁 Preview、停止取帧，500 ms 后清画布灯珠；重新开启才重建。缩放范围 10…400%，预设为 `[400,300,200,100,50,25,10]`，支持适合全部／设备／选中范围；query 与 zoom 保存在 sessionStorage。

当前真实调用链为：

```text
EditorCanvas／区域草稿
→ Studio createPreviewEngine／refreshPreviewEngine 或 applyPreviewAndHardwareEngine
→ 仅投递 lighting-engine 窗口的 memory action
→ LE onChromaStudioAction
→ 停止旧 Quick／Advanced／Preview，加载所选本机 profile
→ 为真实设备建立 handle、区域、位置与灯珠 mask
→ 逆序应用可见图层及组，创建效果并 enable
→ Preview 取帧回传到 memory，画布更新 SVG 灯珠
→ Advanced processEvent，真实设备 setChromaFrame
```

LE 的 Advanced 与 Preview 各有 mutex，销毁旧引擎和应用新引擎放在同一临界流程中，finally 解锁。断开、关闭或 inactive 的设备区域先过滤；Reactive 另按产品能力建立 linked devices。设备位置使用 `row=y/40`、`col=x/40`，真实 deviceId／instance／region 参与 handle 构建，不是仅按产品 ID 输出。

取 Preview 帧按设备或 batch feature 分支执行；窗口可见、有设备且无 Chroma App 占用才轮询，停用传空 devices 及 intervalSpeed=-1。未收到帧不构造目标色。返回颜色按 instance／region 映射到 SVG，black 转透明受设备标志控制；特殊产品 104 不复用普通帧缓存。

真实 Advanced 帧继续进入设备逐行或批量 writer。部分逐行异常被记录后仍最终构造 SUCCESS，一般灯光帧不等待逐灯读回；Studio memory helper 也会 catch 后仅 warn。因此 Preview 显示、memory 投递和 writer SUCCESS 都不能单独证明全部 LED 已呈现。共享硬件帧、锁及产品退出恢复见 [系统灯光链](../system/customize-lighting.md#quickadvanced-到真实灯光帧)。

## 争用、失效结果和释放

Chroma App 控制权提示要求设备数据已加载且有设备，并排除 Help／Onboarding；它读取共享应用状态，清预览灯珠并提供本机控制窗口入口。LE 处于 Chroma App 控制时也拒绝 enable Advanced／Preview，UI 遮罩不能替代消费者检查。

EditorCanvas 卸载移除 touch、mouse、scroll、storage、resize 监听，移除设备并清 LED 缓存。当前 mousewheel 注册使用 debounce 包装、移除使用原函数，不能认定严格配对；内部部分匿名 memory／event 监听也没有证明统一注销。根 beforeunload 另执行引擎 unMount、销毁 Preview 和关闭区域选择窗口。LE destroyPreview 清轮询 timer，退出停止帧、释放区域／设备对象并按产品登记执行硬件效果恢复，不能统一写默认配置。

实现时页面 owner 必须持有任务、订阅、Preview、区域／颜色选择和连接代次；旧设备／旧配置档结果不能覆盖新选择。真实设备结果、本机保存版本、编辑草稿与诊断分别保留。原件的吞异常、未等待与清理不配对是已识别边界，不是本项目可以显示成功的依据。

## 原件与核对边界

获取、部署身份及字节 SHA-256 见 [Studio 收据](../../../.analysis/inputs/frontend/synapse-chroma-studio-current/acquisition.json) 与 [Lighting Engine 收据](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/acquisition.json)。表中 JS 区间均为原始 UTF-8 半开字节区间；数字模块是实际 bundle 模块号，不冒充缺失的官方源文件名。

| 所有者 | 当前原件／区间 |
| --- | --- |
| 根、页面、观察者、争用 | [main.6b22e9cc.js](../../../.analysis/inputs/frontend/synapse-chroma-studio-current/raw/apps.razer.com/synapse/chroma-studio/static/js/main.6b22e9cc.js)：`[447670,447904)`、`[419279,419944)`、`[412729,419165)`、`[421620,423226)` |
| 状态、常量、效果转参／默认值 | 同上，模块 1638 `[95638,110180)`、6257 `[218154,229647)`、2904 `[137743,146046)` |
| 本机存储、配置档、图层／区域动作 | 同上，模块 7759 `[300445,301468)`、8480 `[320223,332398)`、9286 `[341186,355384)` |
| LE 请求、预览生命周期、系统 wrapper | 同上，模块 2304 `[124876,127293)`、4048 `[170242,181971)`、6499 `[232341,239515)` |
| 编辑器组合、左／右栏、快捷键 | [EditorCanvas.c3ee330f.chunk.js](../../../.analysis/inputs/frontend/synapse-chroma-studio-current/raw/apps.razer.com/synapse/chroma-studio/static/js/EditorCanvas.c3ee330f.chunk.js)：`[115119,116026)`、`[14087,21298)`、`[24571,27418)`、`[27510,28885)` |
| 初始化、指针交互、卸载、Preview／zoom | 同上，`[90778,95159)`、`[95633,108691)`、`[114058,115119)`、`[42664,44353)`、`[45284,49667)` |
| 面板／画布 CSS | [EditorCanvas.1012a47a.chunk.css](../../../.analysis/inputs/frontend/synapse-chroma-studio-current/raw/apps.razer.com/synapse/chroma-studio/static/css/EditorCanvas.1012a47a.chunk.css)：panel `[4284,4495)`、header `[5163,5315)`、canvas `[6965,7062)` |
| Ambient、本机 Generate | `958.e77237aa.chunk.js` 模块 1958 `[546,5533)`；`474.2374fc9f.chunk.js` 模块 2474 `[546,7585)`，均在 Studio 原件 JS 目录 |
| Ripple／Wave／Reactive／Spectrum | 同目录 `255.55c55ae1.chunk.js` 模块 5305 `[10507,13704)`；`285.bbf66b46.chunk.js` 模块 1591 `[1347,7586)`；`42.02817760.chunk.js` 模块 7518 `[2625,5043)`；`820.c393c8b8.chunk.js` 模块 8552 `[8864,11072)` |
| Breathing／Static／Starlight／Fire | 同目录 `679.0781b947.chunk.js` 模块 2777 `[5026,7831)`；`435.fba6194a.chunk.js` 模块 3181 `[546,1614)`；`813.7628f9a2.chunk.js` 模块 6548 `[12065,15310)`；`941.5700335c.chunk.js` 模块 5591 `[1056,2931)` |
| Audio／Wheel／Tidal | 同目录 `412.2045a1a6.chunk.js` 模块 9120 `[10519,14224)`；`729.84fd7088.chunk.js` 模块 8379 `[13096,16602)`；`510.7368f605.chunk.js` 模块 8154 `[6858,10898)` |
| Import／Export／Save | 同目录 `ModalImport.320c0aae.chunk.js` 模块 6718 `[557,8674)`；`ModalExport.5d3609fd.chunk.js` 模块 6945 `[557,3092)`；`ModalSave.f069cb78.chunk.js` 模块 2502 `[558,1562)` |
| LE 消费、引擎创建、设备／图层／帧 | [main.a383ea9e.js](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/raw/apps.razer.com/synapse/lighting-engine/static/js/main.a383ea9e.js)：`[933285,934043)`、`[883732,889043)`、`[935541,937822)`、`[946016,947448)`、`[948255,954524)` |
| 系统区域／颜色选择原生完整捕获 | [capture.json](../../../.analysis/results/native/c9a61daec651215975dc9cd4c7f6a8037b6855981b403e6a9449b2e9bd102778/capture.json)，Image Base `0x180000000`；公开入口 RVA `0x1e00/0x1ef0/0x2030/0x2140/0x2180/0x2ad0/0x2cd0/0x3360/0x33f0/0x3410`，回调 `0x23e0/0x2770/0x2e60/0x30f0`，消息循环 `0x39530` |

Studio 收据中已取得的 306 项原件逐项核过长度与 SHA-256；48 项 unresolved 不被伪造为可用 map。本文已沿上述完整函数及本机消费者静态阅读，仍不代表引擎原生算法、全部设备协议或系统格式分支已全部闭合。

| 完成维度 | 本页能证明的边界 |
| --- | --- |
| 源码获取 | 当前收据及以上原件存在；未恢复的引用保持独立 |
| 逆向语义 | 本页具体 UI、存储、LE 与系统原生函数已静态追踪 |
| Rust 独立实现 | 文档不构成实现完成证据 |
| UI／后端接线 | 文档不构成产品接线完成证据 |
| 静态／模拟验证 | 仅原件身份与文档引用核对；未由本页运行设备模拟测试 |
| 运行时验收 | 未验证；未运行主应用、厂商 JavaScript、安装器或动态调试 |

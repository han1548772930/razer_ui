# Chroma Studio：图层、灯珠草稿与真实输出

Studio 把真实连接设备的几何与灯珠身份放到同一画布，以配置档保存设备位置、图层、区域和媒体。编辑态先驱动独立 Preview；Save 把当前草稿保存为本机配置档，再交 Advanced 引擎输出。画布上出现颜色、本机保存完成、引擎启动和设备收到帧是不同结果。本页只展开本机页面、文件、存储、服务与系统能力；账号、云同步、在线分享及在线获取流程不在范围内。

## 实际挂载和布局

当前入口的 Provider 下依次挂错误边界、Shell、页面路由、观察组件及附属节点。页面 `xt` 在 Help 状态下以模块 7943 替换模块 4264 的 EditorCanvas；Onboarding 模块 334 按独立条件附加挂载，不能把它们当作 EditorCanvas 内的一张效果表单。争用提示独立观察 Chroma Apps 控制权。

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

颜色采样、普通弹层 Save／Cancel、Studio Done 提交以及当前 Rust 取消完成缺项见[颜色选择与屏幕取色](../shared/color-picker.md)。区域选择使用不同窗口、回调和坐标，不能复用取色器的虚拟桌面快照语义。

### Edit 到实际框选窗口

当前完整链是 `1958:N → 1238:startSelectRegion → 6499:startSelectRegion → preload.doDLLMainAction → FFIPreloadMain.callDLL → StartSelectRegion`。1958 先最小化窗口，等待 start 调用返回后允许显示 custom preset；1238 设置 `isEditRegion`，6499 在调用前设置 `isOnRegionSelect`。这些 UI 标志不证明创建窗口或安装 hook 成功。

原生 `StartSelectRegion` 同时建立两个 owner：透明 CursorWindow 和实际 SelectionWindow。CursorWindow 以 `"0,0,0,0"` 创建，使用主屏宽高、十字光标和 alpha=1 的置顶窗口；SelectionWindow 创建自己的线程、消息循环和 layered／tool／topmost popup。`GetSystemMetrics(0/1)` 决定框选范围 `[0,Ws] × [0,Hs]`；本链没有取得虚拟桌面 left／top，也未证明多屏负坐标或 DPI 坐标经过统一转换。

`SelectionWindow::MainLoop` 安装 `WH_MOUSE_LL=14` 与 `WH_KEYBOARD_LL=13`，投递 `0x46f` 表示窗口已创建。鼠标 hook 的 `WM_LBUTTONDOWN` 保存起点，`WM_MOUSEMOVE` 在 drawing 时更新终点，`WM_LBUTTONUP` 更新终点、结束 drawing 并投递 `0x466`。每次将坐标夹在上述范围；右向和反向拖动共用这条路径。绘制用黑色透明底和 2 px 绿色矩形，恢复原 GDI 对象并删除临时 pen／brush。键盘 hook 只对 `WM_KEYDOWN + VK_ESCAPE` 投递 `0x465`；hook 最后仍调用 `CallNextHookEx`，没有吞掉这些输入。

三个 `std::function` 的间接目标已从原始 PE 的 vtable 槽和完整 thunk 核对，不能继续把完成／取消消费者写成未知：

| SelectionWindow 字段／触发 | vtable RVA 的 `+0x10` 槽 | 实际完整目标 |
| --- | --- | --- |
| `+56`，`0x466` 完成 | `0x73ce0 → 0x80d0` | `0x23e0`：整理 `min(x0,x1), min(y0,y1), max(x0,x1), max(y0,y1)`，发布 `{"event":"onRegionSelected","data":"left,top,right,bottom"}`，随后关闭并等待 CursorWindow |
| `+120`，`0x465` 取消 | `0x73ca8 → 0x80a0` | `0x2770`：先关闭并等待 CursorWindow，再发布 `{"event":"onRegionCancelled"}`；没有坐标和参数提交 |
| `+184`，`0x46f` 创建 | `0x73c70 → 0x8060` | 仅日志；没有向页面发布可用于确认创建成功的事件 |

`0x39220` 对完成和取消都先卸两种 hook，再调用对应函数；这一步尚未结束 SelectionWindow 的消息循环。事件字符串只在本次回调期间有效；`SetNodeFFIEvent` 保存的 callback 在当前宿主中声明为 `void(string)`，解析 JSON 后按 `event` 原名转发给对应页面 URL，不能保留原生临时字符串指针。

### 完成、取消与坐标提交

6499 的 `handleEvent` 对 Selected／Cancelled 都安排 `setTimeout(...,0)`，等待 `stopSelectRegion` 后才向自身 eventEmitter 发布。Stop 清 `isOnRegionSelect`，原生端再次卸仍活动的 hook，向 SelectionWindow 同步发送 `WM_CLOSE`，销毁窗口、等待线程并释放对象；没有对象时也返回 true。Stop 检查禁止在自身线程 join，因此不能把它直接搬到原生选择回调中同步执行。创建线程和 join 有抛出 C++ 错误的路径，布尔返回并未完整携带这些错误。

Selected 到达 1958 后，保留原始像素字符串供 custom hover 使用，再调用 GetScreenResolution。该导出执行 `GetDesktopWindow → GetWindowRect`，返回 `"right,bottom"`，不是自行计算 `right-left,bottom-top`。宿主同步默认调用者先 `readCString`，再在 finally 调用 `FreeMalloc`；普通原生事件字符串不走这条返回值释放路径。

页面以 GetScreenResolution 返回的 `Wd,Hd` 按 `Left=left×65535/Wd`、`Top=top×65535/Hd`、`Right=right×65535/Wd`、`Bottom=bottom×65535/Hd` 生成 `screenRect`，再 dispatch `9286:HZ → R` 更新效果参数草稿，最后恢复窗口。这里使用 JavaScript Number，没有取整；例如 `1920×1080` 的右下四分之一区域得到 `{Left:32767.5,Top:32767.5,Right:65535,Bottom:65535}`。不能提前改成 u16 截断；也不能把原始像素字符串替换成归一化坐标再交 native hover。框选的系统尺寸与这里的桌面边界来自两次不同 API，原件没有校验两者相等。HZ 默认还在 Select 工具下请求临时效果预览，未自动 Save 配置档或取得硬件确认。

当前 Electron 20 分支有一处已定位的真实断点：宿主按 `event` 转发 Cancelled，但 6499 constructor 只订阅 `onRegionSelected`、`onColorPicker`、`onColorPickerDone`，没有订阅 `onRegionCancelled`。1958 的 Cancelled 分支依赖该 wrapper 的 eventEmitter；在已取得的 Studio JS 集合中没有找到另一条 Cancelled 桥接。因此旧式 Node FFI callback 的“取消 → Stop → 恢复窗口”分支已查清，而当前 Electron 20 的这条接收链仍缺失，不能宣称 Esc 后 SelectionWindow 已关闭、线程已 join 或编辑窗口已恢复。原生取消自身只证明卸 hook 和关闭 CursorWindow，没有提交 `screenRect`。

### Hover 预览、失败与释放

1958 hover 通过 ShowSelectedRegion 的枚举 `full=0,left=1,top=2,right=3,bottom=4,custom=5` 创建独立 RubberbandWindow。左右预设取主屏宽度的 10%，上下取高度的 10%；右／下起点为 90%，native 转为整数。custom 把四项原始像素 CSV 传入，窗口大小为 `right-left,bottom-top`；不是屏幕尺寸的半区或归一化单位。离开调用 CloseSelectedRegion，关闭该窗口并等待线程，不能用关闭 hover 代替 StopSelectRegion。

Start／Show 的公开导出在发起创建后返回 true；鼠标 hook 失败仅日志，键盘 hook 失败没有失败事件，窗口类注册失败也没有传到页面的创建失败回执。重复 Start 会直接替换旧 SelectionWindow，旧对象析构仍检测 joinable 线程并可能 terminate，不能从“重新创建对象”推断已安全取消旧选择。1238 的包装器还会吞掉部分底层返回值。读取桌面尺寸或转换字段失败时，1958 没有独立的错误恢复／finally 路径。这些是当前原件的边界；Rust 必须保留真实失败，不能以 custom preset 已出现或双方都返回 true 作为成功。

1958 普通组件卸载只移除 beforeunload handler，没有移除已注册的 native eventEmitter listener；handler 真正执行时才 off listener 并 terminate。6499 terminate 仅按 `isSelectedWindowCreated` 关闭 hover，再请求 `FreeFFI` 并清初始化字段，没有主动 StopSelectRegion。当前宿主 `FFIPreloadMain.callDLL` 的 `FreeFFI` 分支直接返回 true，没有执行该 DLL／callback map 的释放。因此 terminate 返回 true 也不能证明选择线程、callback 和动态库均已结束。独立 Rust 适配需要由 retained owner 异步确认停止、恢复窗口和清理结果，并拒绝已切换图层／窗口的旧回调；不得接入官方业务 DLL 作为实现。

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

### LE 引擎动作的实际边界

当前 Lighting Engine 原件 [main.a383ea9e.js](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/raw/apps.razer.com/synapse/lighting-engine/static/js/main.a383ea9e.js) 的 `applyChromaStudioEngine` 起始于 UTF-8 byte `883730`：它会先原地遍历 profile 的 layer/group/region，把没有 `rzDevice`、设备掉电或 `perPortInfo.isActive === false` 的 region device 过滤掉，并只记录被过滤的 region；随后用 `fps:25` 和 `type: Preview/Advanced` 创建引擎。引擎创建成功但 `rzDevicesMap` 为空或 profile 缺失时直接返回，已有 handle 的回收不在这个早退分支内；正常路径才按设备位置、region 与 `Reactive` 能力建立设备并逆序应用 layer，最后 `enableEngine(1)`，Advanced 还触发 frame worker。

`onChromaStudioAction` 只接受 window storage 中可解析的 action；JSON 解析失败改成空对象，`cleared` 事件直接忽略，未知 action 只记 warning。`createPreviewEngine`、`destroyPreviewEngine`、`refreshPreviewEngine`、`applyPreviewAndHardwareEngine`、`getCurrentPreviewFrame`、`refreshDevices` 与 `identifyDevice` 都是独立分支，不能把一次 storage 投递当作整条链成功。`enablePreviewEngine` 和 `enableAdvancedEngine` 各自持有 mutex，先销毁旧 engine，再复制／读取 profile，失败只进入 catch 日志并在 finally 解锁；Chroma Connect 占用时启用请求直接返回。

`handleGetCurrentPreviewFrame`（原件 byte `886427` 附近）逐设备解析 `productId/instanceId/deviceId/instanceIdChromaStudio`，找不到 handle 就跳过；`intervalSpeed === -1` 会清 timer 并清空 `previewFrameDevices`。有 batch feature 时按 interval 启动 `getCurrentFrameMultipleDevicesNoReturn`，否则调用带返回值的批量接口；只有收到了非空 frame 才发布并写 memory storage，空帧不会构造目标颜色。`destroyDeviceLightingEngine` 则先禁用 engine，再销毁 engine/device handle、删 `devicesRegion` 并更新 handle；这些清理调用的异常不会变成硬件读回确认。

退出链 `onExit`（原件 byte `874722`）设置 `isExiting` 后停止 frame、终止 Lighting API、清空 preview/advanced/device map，按产品登记执行 `setDeviceToDefaultOnExit`，再清 linked devices、状态与 host callback。默认恢复包含 driver mode、quick effect、BLE FPS 等分支；它是退出恢复动作，不是 Studio Save 的设备确认。Rust 当前仅有图层存储与独立 renderer，尚未接通这些 action、真实 frame reader、Advanced writer 或逐设备恢复结果；因此本节的静态路径不能提升为 Rust 功能或官方 DLL／设备对照通过。

## 争用、失效结果和释放

Chroma App 控制权提示要求设备数据已加载且有设备，并排除 Help／Onboarding；它读取共享应用状态，清预览灯珠并提供本机控制窗口入口。LE 处于 Chroma App 控制时也拒绝 enable Advanced／Preview，UI 遮罩不能替代消费者检查。

EditorCanvas 卸载移除 touch、mouse、scroll、storage、resize 监听，移除设备并清 LED 缓存。当前 mousewheel 注册使用 debounce 包装、移除使用原函数，不能认定严格配对；内部部分匿名 memory／event 监听也没有证明统一注销。根 beforeunload 另执行引擎 unMount、销毁 Preview 和关闭区域选择窗口。LE destroyPreview 清轮询 timer，退出停止帧、释放区域／设备对象并按产品登记执行硬件效果恢复，不能统一写默认配置。

实现时页面 owner 必须持有任务、订阅、Preview、区域／颜色选择和连接代次；旧设备／旧配置档结果不能覆盖新选择。真实设备结果、本机保存版本、编辑草稿与诊断分别保留。原件的吞异常、未等待与清理不配对是已识别边界，不是本项目可以显示成功的依据。

## 当前 Rust 生产调用者的逐项核对

以下结论来自实际字段、调用者和结果消费者；页面说明、导出声明和曾经的 FFI 分析不能替代这些证据。

| 功能 | 当前实际 owner／消费者 | 已有结果与仍需实现的链 |
| --- | --- | --- |
| 页面组合、图标与参数控件 | [chroma_studio.rs](../../../crates/razer-pages/src/features/chroma/chroma_studio.rs) 的 `render/layer_panel/editor/inspector`；[properties](../../../crates/razer-pages/src/features/chroma/chroma_studio_properties.rs)；目录库读取与主题 | 已有 250 px 两侧栏、40 px 工具栏、效果图标、保留的参数子 Entity 和颜色／渐变等控件。左栏仍是固定宽度；完整 Generate 属性分支未挂载。不能据局部尺寸和词表判定布局、焦点、弹层及全部状态已 1:1 验收 |
| 真实设备／画布／选择 | [canvas](../../../crates/razer-pages/src/features/chroma/chroma_studio_canvas.rs) 的 `render` 只接受 zoom 与 show_grid；页面 `editor` 调用它 | 已有 Move 下的网格和中心轴。生产页面没有设备列表、LED 身份／几何输入及指针选择消费者，不能把始终空画布称为真实无设备观察；Pen／Bucket／Move 目前仅更换工具字段，清 LED 按钮始终禁用。设备挂接、碰撞、旋转、中心点、触摸和平移仍须接入实际设备与区域模型 |
| 图层、组、参数和历史 | [领域文档](../../../crates/razer-model/src/chroma_studio.rs) 的 `Document/Layer`；页面 `add_group/duplicate/remove/checkpoint/history` | 普通层新增、重命名、复制、可见性和撤销有真实草稿逻辑。当前组是扁平 `group:bool` 行，没有子图层业务结构／拖放消费者；`params/params2/paint_params` 仍用 Value，extensions 保留未知字段不等于已校验和执行 regions。checkpoint 没有官方的 20 项上限；Reactive／Ripple 添加与变更始终排除，尚未按真实设备能力开启 |
| 配置档与本机保存 | [StudioSession](../../../crates/razer-shell/src/shell/windows/chroma_studio_window.rs) 的 `new/start_pending/prepare/start_watch` → [chroma_profiles](../../../crates/razer-storage/src/chroma_profiles.rs) 的 `StudioLibrary::changed/update_studio` | 已有 12 字符 ID、增删改选／复制、异步保存、原子持久化入口、失败 mutation 保留、修订号及 500 ms 外部选择观察；Save 只有存储成功才更新 saved baseline。保存不是空函数。当前快照仍以图层文档为核心，设备位置／LED regions／媒体清理和实际设备就绪条件未闭合；不能以本机无文件自动推断设备需初始化 |
| Save／Discard、关闭和退出 | [profiles](../../../crates/razer-pages/src/features/chroma/chroma_studio_profiles.rs) 的 `request_profile`；[Chroma 窗口](../../../crates/razer-shell/src/shell/windows/chroma_window.rs) 的 `close_tab`；Shell `continue_save_queue` | 配置档 Add／Select／Duplicate 遇脏稿已有 Save／Discard，Save 事件排在后续动作前并受存储队列控制。关闭 Studio tab 仅 deactivate，Shell 保留 session；退出只等待已请求的保存，未连接官方 beforeunload／退出拒绝的脏稿选择。独立 Discard 恢复及关闭续动作仍须按当前官方 owner 接线，不能把保留内存草稿当作退出确认 |
| Save 的下游消费者 | `StudioSaveFinished` → [publish_committed_chroma_configuration](../../../crates/razer-shell/src/shell/gamepad/controller_chroma.rs) → workspace `observe_chroma_configuration` | 已提交的共享配置身份、选择及 enabled 会发布给产品页；例如键盘消费者更新观察和映射目标。这里传的是 Configuration 投影，没有把 Studio 层／区域编译为引擎效果，也没有请求 Advanced／Preview 或硬件 writer；共享选择刷新不能作为灯光已应用 |
| Preview 和独立 Rust 引擎 | 页面 `editor` 的 is_preview／slow_preview；[运行时引擎](../../../crates/razer-service/src/runtime/lighting_engine.rs) → [Rust renderer](../../../crates/razer-device/src/lighting/renderer/mod.rs) 及 [engine](../../../crates/razer-device/src/lighting/renderer/engine.rs) | 独立 Rust 引擎已经存在，运行时拥有真实系统音频峰值采样适配和软件帧逻辑，不能称为“仍依赖官方 DLL”。Studio 两个菜单当前只更新布尔值，没有引擎请求、40／100 ms 帧消费者或 SVG LED 更新。现有 renderer 明确只接收自身核实的效果／mode，不能扩写成完整支持 Studio 的 Ripple、Ambient、Wheel、Generate 及所有 Audio 自动选项 |
| Ambient 与系统取色 | properties `ambient`；颜色／渐变自己的 retained 子 Entity | Ambient preset／blur 能修改草稿，但 Edit 按钮始终禁用，没有区域选择、hover 窗口、完成坐标和取消确认消费者。上节已给出可实现的官方系统调用链；颜色拾取已有独立 Rust Windows 后端，具体正确项和取消终态缺口见[共享取色文档](../shared/color-picker.md) |
| 本地文件、争用与清理 | 页面事件仅 `Save/Profile`，配置档菜单仅 Add／Rename／Duplicate／Delete；ChromaWindow 负责 deactivate／focus | 当前实际菜单没有 Import／Export；Generate 的本机媒体链也没有对应页面消费者。Studio 尚无设备／Preview owner，因而不能声称已执行其退出恢复；Chroma App 的遮罩和后端 enable 拒绝须在实际新增消费者核对。动态灯光的系统能力另见[共享文档](../shared/dynamic-lighting.md) |

后续实现先建立真实设备、图层／区域及媒体的具名数据结构与 retained 生命周期，再把本节已经查明的系统区域链、保存编译、Preview／Advanced 接到独立 Rust 引擎和真实设备 writer。每次迁移都须同时迁移正常、失败、取消、旧代次拒绝、恢复和释放消费者；不能只解禁按钮或把 JSON 存进配置档后称为完成。

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
| 区域系统 façade、草稿提交与单位 | Studio main 模块 1238 `[14835,16745)`；`9286:R` `[346752,346934)`；`6257:Ye=65535` `[225860,225868)` |
| Ripple／Wave／Reactive／Spectrum | 同目录 `255.55c55ae1.chunk.js` 模块 5305 `[10507,13704)`；`285.bbf66b46.chunk.js` 模块 1591 `[1347,7586)`；`42.02817760.chunk.js` 模块 7518 `[2625,5043)`；`820.c393c8b8.chunk.js` 模块 8552 `[8864,11072)` |
| Breathing／Static／Starlight／Fire | 同目录 `679.0781b947.chunk.js` 模块 2777 `[5026,7831)`；`435.fba6194a.chunk.js` 模块 3181 `[546,1614)`；`813.7628f9a2.chunk.js` 模块 6548 `[12065,15310)`；`941.5700335c.chunk.js` 模块 5591 `[1056,2931)` |
| Audio／Wheel／Tidal | 同目录 `412.2045a1a6.chunk.js` 模块 9120 `[10519,14224)`；`729.84fd7088.chunk.js` 模块 8379 `[13096,16602)`；`510.7368f605.chunk.js` 模块 8154 `[6858,10898)` |
| Import／Export／Save | 同目录 `ModalImport.320c0aae.chunk.js` 模块 6718 `[557,8674)`；`ModalExport.5d3609fd.chunk.js` 模块 6945 `[557,3092)`；`ModalSave.f069cb78.chunk.js` 模块 2502 `[558,1562)` |
| LE 消费、引擎创建、设备／图层／帧 | [main.a383ea9e.js](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/raw/apps.razer.com/synapse/lighting-engine/static/js/main.a383ea9e.js)：`[933285,934043)`、`[883732,889043)`、`[935541,937822)`、`[946016,947448)`、`[948255,954524)` |
| 系统区域／颜色选择原生完整捕获 | [capture.json](../../../.analysis/results/native/c9a61daec651215975dc9cd4c7f6a8037b6855981b403e6a9449b2e9bd102778/capture.json)，Image Base `0x180000000`；公开入口 RVA `0x1e00/0x1ef0/0x2030/0x2140/0x2180/0x2ad0/0x2cd0/0x3360/0x33f0/0x3410`，回调 `0x23e0/0x2770/0x2e60/0x30f0`，消息循环 `0x39530` |
| 区域窗口、hook、间接完成目标与清理 | 同捕获的 `functions/`：`0x397f0` 建线程、`0x39e70/0x3a3b0` 建窗、`0x39220` 消息分发、`0x39960/0x39ba0` 输入 hook、`0x39c70` 绘制、`0x80d0/0x80a0/0x8060` thunk、`0x374c0/0x372c0/0x37920` CursorWindow、`0x38540/0x388a0/0x38cf0` hover、`0x4340/0x5ad0` 对象释放；函数边界及完整反汇编／伪代码以各 JSON 为准 |
| 当前宿主桥接及返回内存释放 | [FFIPreloadMain.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/ffi/FFIPreloadMain.js)：callback `[6351,7719)`、同步调用／FreeMalloc `[7758,8563)`、分发／FreeFFI `[8564,11243)`；[preload.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/preload.js) 的 `response/doDLLMainAction`；已与当前 [app.asar](../../../.analysis/inputs/installed-current/files/app-engine/resources/app.asar) 内对应偏移字节核对 |

Studio 收据中已取得的 306 项原件和 LE 收据中已取得的 165 项原件重新逐项核过实际长度与 SHA-256，未见差异；其余记录及分别 48／158 项 unresolved 保持独立，不能概称为全部都只是缺 map。本页新增区域链沿完整函数、原生 vtable／thunk 和真实宿主分发核对，所引用的 native 函数文件身份及 PE 函数字节已核对；这不代表整个原生库、引擎算法、全部设备协议或系统格式分支已闭合。

区域选择可以先做无设备纯测试：顺向／反向／退化矩形、超出主屏范围、主屏边界、浮点归一化、10% hover 取整、完成与取消的事件顺序、创建／hook／尺寸读取失败、同窗替换、切层旧回调和清理失败。独立期望必须来自上述官方字段与公式，包含 `32767.5`，不能从本地实现重新生成。官方实际 DLL 对照还要观察真实窗口、输入 hook、事件桥接、Stop join 及恢复结果，特别分开当前 Electron 20 缺失 Cancelled 桥接与旧式分支；仅 native 返回 true 或未连接都不能判通过。选择窗口不需要雷蛇设备，但运行时操作仍只允许显式对照测试入口，不能启动主应用或厂商 JavaScript。

Studio 全功能测试还须逐项覆盖设备增删／重连、草稿区域合并、组拖放、20 项历史、配置档与本地媒体、Save／Discard／关闭续动作、Preview 帧身份与速度、争用、真实传输及退出恢复；对应官方实际 DLL／设备结果未执行时保持未验证，遵守[验证规则](../../../.agents/rules/verification.md)。本次仅核对原件、完整 owner、Rust 调用者和文档引用，没有重跑现有 Rust 测试，也没有运行官方 DLL、真实窗口或设备。

| 完成维度 | 本页能证明的边界 |
| --- | --- |
| 源码获取 | 当前收据及以上原件存在；未恢复的引用保持独立 |
| 逆向语义 | 已追踪具体 UI、存储、LE；区域选择的 hook／坐标／间接完成目标／宿主桥接／Stop 已查清，当前 Electron 20 Cancelled 与 FreeFFI 边界明确保留 |
| Rust 独立实现 | 已有本机配置档／草稿控件、系统取色和独立灯光 renderer；Studio 真实区域、媒体及完整效果编译仍未完成 |
| UI／后端接线 | 已接存储及共享选择投影；画布设备、区域选择、Preview／Advanced 与真实 writer 尚未闭合 |
| 静态／模拟验证 | 原件身份、完整函数／vtable、当前 Rust 消费者和文档引用已核对；本次没有重跑 Rust 模拟测试 |
| 运行时验收 | 未验证；未运行主应用、官方 DLL、真实系统区域窗口、厂商 JavaScript、安装器、设备或动态调试 |

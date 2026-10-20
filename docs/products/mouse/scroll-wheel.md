# 鼠标：滚轮模式、Smart Reel 与滚动加速

滚轮卡片修改当前配置的 `scrollWheel`。模式控制机械／触觉滚动方式；Smart Reel 控制自动切换；加速控制虚拟滚动加速；level 字段是相应功能的档位。滚轮方向的按键映射属于 [Customize](customize.md)，并非这组设置。

## 产品差异与输入

| 当前完整审计 PID | 模式版本 | 其他实际分支 |
| --- | --- | --- |
| 153、170 | 普通模式 | 模式、Smart Reel、加速开关，没有本文所述 level 编辑 |
| 203、204、214、45066 | 普通模式 | 具有 Smart Reel／加速 level；45066 的模式板载同步只选第一个真实匹配槽位 |
| 226 | Mode V2 | Tactile、FreeSpin、MicroTactile；禁用模式集合；Smart Reel／加速 level |
| 167 | 独立 Scrolling 页 | 六种触觉预设、启用集合、双向曲线编辑、应用浏览模式与高分辨率滚动 |
| 231 | Customize 内的 Mode V2 | 三种模式和 disabledModes；没有挂载 PID167 的六预设／曲线编辑器 |

PID226 的 level 滑块是 0～4、步长 1，属于档位而非百分比。模式的协议值分别是 Tactile=0、FreeSpin=1、MicroTactile=2。界面最多允许禁用两个模式；禁用当前模式时，立即选择顺序中第一项仍可用模式。FreeSpin 被禁用时，Smart Reel 和加速卡片同时锁定，并显示其原因提示。

模式按钮先更新局部选中态，300 ms debounce 后提交，外部 scrollMode 变化另同步局部态。禁用模式的操作是另一个 action，不经过该模式按钮的延时分支。

## 保存、设备工作与结果

UI reducer 对各字段发送 `ON_SCROLL_MODE`／`ON_SCROLL_MODE_V2`、`ON_SMART_REEL`、`ON_SCROLL_ACCELERATION` 及对应 level／disabled-modes 事件。中间件先保存当前配置字段和版本，再派发设备任务。配置恢复依次处理 Mode、Smart Reel、可选 threshold、Acceleration、可选 level；PID226 最后恢复 Mode V2 的模式屏蔽与选中模式。

PID226 `85191` 的任务分别调用：

| 设置 | 实际 setter |
| --- | --- |
| 加速开关 | `setScrollVirtualAccelerationState(profile,0/1)` |
| 加速档位 | `setScrollVirtualAccelerationLevel(profile,level)` |
| Smart Reel | `setScrollModeAutoSwitchState(profile,0/1)` |
| Smart Reel 档位 | `setScrollWheelSmartReelModeThreshold(profile,level)` |
| 滚轮模式 | `setScrollFreeNotchedMode(profile,0/1/2)` |

这些 writer 在入口检查取消，捕获传输错误并 reject；完成依据是 setter 应答，完整函数中没有普遍追加 GET 比较。Mode V2 成功应答中的 mode 可以映射回界面值；普通分支的 UI 发布条件不同，不能用同一套消费者解释全部产品。

真实板载清单由 retained device session 读取最大容量、数量、顺序 ID 和完整身份，再按当前配置 GUID 选槽。Smart Reel 保留全部匹配槽位；PID45066 模式路径只取第一个匹配。当前配置的 GUID、本地数组序号和真实槽位 ID 是不同对象。

## Mode V2 的恢复与失败

PID226 `restoreScrollDisabledModes` 先从所选配置规范化 disabledModes，再从当前 scrollMode 与可用模式中得到 active mode，提交禁用集合，最后发布两个字段的 UI 投影。只有这条实际 restore 链能说明恢复顺序，不能把 reducer 初值当成设备读回。

原版错误通道记录失败，编辑器保持乐观字段；未见通用“失败回滚上一次配置”的动作。取消入口返回、未发出的 debounce、已保存配置、已发送设备命令和任务结算是不同阶段。源码存在若干入口仅记录 canceled 并返回的 writer，这不证明设备已还原，也不证明先前本地保存被撤销。

## PID167：触觉预设与曲线草稿

Scrolling 页依次挂载预设选择、加速／浏览选项和预设参数卡片。六种预设的领域身份是 `SW_STANDARD`、`SW_DISTINCT`、`SW_ULTRA_FINE`、`SW_ADAPTIVE`、`SW_SMOOTH_SCROLL`、`SW_CUSTOM`。标准数据的 tension／steps 依次为 60／24、58／15、65／96、75／24、0／32；Custom 为顺时针、逆时针分别保存数值和曲线。静态表本身没有把第六项变成所有产品均可编辑的能力。

预设启用开关至少保留一项；移除当前选中项后按组件的活动项顺序重新选择。设备的向上／向下循环映射同样在活动预设中有序循环，仅一项活动时不产生切换。加速开关只在 Smooth Scroll 上开放，其他预设展示其禁用状态和提示。

非 Custom 的参数卡显示只读 tension、steps。Custom 打开局部编辑弹窗：tension 0～100、steps 8～96，步长均为 1；上下方向各自编辑，Link 决定另一方向是否镜像。只有草稿改变时 Save 可用；Save 从曲线 ref 取得控制点后发送 `setScrollWheelStageCustomAction`。关闭或 Cancel 丢弃本次局部草稿并重新投影已提交参数；Save 调用自身没有额外关闭弹窗的动作。

曲线画布尺寸 534×344，绘图区 520×330，控制点使用 7 px 偏移。每个方向有 `pLeft`、`pMidLeft`、`pMid`、`pMidRight`、`pRight` 五点，形成两段二次 Bézier。拖动约束端点和中间点的横向顺序及纵向范围；Link 通过 `abs(534-x)` 镜像横坐标并反转点序，导出坐标减去绘图区偏移。组件卸载移除 document 的 mousemove／mouseup；对匿名 canvas mousedown 监听未找到配对移除调用，不能把这段称为完整释放。

## PID167：保存、写入与结果消费者

页面 action 进入 scrolling feature；maker 将实际提供的预设集合、activeStage、加速、浏览开关、应用列表和高分辨率字段保存到当前 profile 并递增版本。没有 `scrollWheelStages` 时 maker 按源码建立该配置结构；这段构造是本地 profile 的行为，不是设备读取结果。`SCROLL_CycleUp`／`SCROLL_CycleDown` 先更新当前 profile 和 UI 投影，再排设备任务。设备目标为 profile 1 加当前 profile 真实匹配的板载 slot，过滤空匹配，不能用 profile 数组下标代替 slot。

| 操作 | 实际设备任务及顺序 |
| --- | --- |
| 当前预设／循环切换 | 先 `getHapticScrollProfileActiveStage` 读第一个目标；与活动序号不同时对目标集合调用 `setHapticScrollProfileActiveStage` |
| 启用集合 | 按活动项顺序将领域身份映射为协议 preset，`setHapticScrollStageInformation(profile,activeOrder,count,list)`；activeOrder 从 1 开始 |
| 加速 | `setScrollVirtualAccelerationState`，使用其普通滚轮目标选择链 |
| Custom | 固定 stage 6；先元数据，后 generator setting，后 CW 表、CCW 表、velocity 表 |
| 关闭浏览模式 | 排浏览任务，按实际设备 enable 和应用匹配结果决定是否写入 |

writer 先把两向控制点从画布坐标转换为百分比：`x=round(x/5.2)`、`y=round(abs(y/3.3-100))`。这些整数点同时用于 Custom 元数据和后续曲线计算，不能直接以原画布坐标生成设备表。元数据先写 `setHapticScrollGeneratorCustomMetaData`：Link、启用状态、两向 steps／tension 及五控制点。每向曲线分别采样：第一段从左端到中点，第二段从右端到中点后反转合并，每段 50 次采样；前向离散算法在纵向误差阈值 0.2 内压缩点。角度输出为 `360/steps × x/100`，电流为 `0.2 × tension/100 × y/100`。首个最大 y 点决定 index angle；enable=1、alternateCurrent=1、两向 reduction angle=0。velocity 固定为 (-720,1)、(0,1)、(720,1)。

随后 `setHapticScrollProfileGeneratorSetting` 写两向角度、表长和 velocity 表长；`setHapticScrollProfileGeneratorTable` 每 8 点分块，type 1 为 CW、type 2 为 CCW，最后 type 3 写 velocity。任务之间保留官方顺序；未见整条 Custom 链末尾追加 GET 比较。预设与 Custom writer 检查入口 abort；任何一步非 SUCCESS 只把 `value.status` 改为 error 并返回，catch 保存错误并记录，回调只检查 completed／skipped 并输出错误。这与普通加速 writer 的 reject 通道不同，也没有失败后撤销已保存 profile 或已写前序分块的通用动作。

恢复链从当前 profile 投影预设和活动项，排当前 stage 及活动列表任务；当前为 Custom 时再排第六预设的完整自定义数据，最后恢复加速和浏览字段。UI 的恢复投影不能作为设备 GET 结果。Protocol25 的事件 58 解码为 activeStage、activeStageIndex 及其枚举；当前共享消费者在该 case 只为 `rzDevice30` 的 `DEVICE_EFFECT_CHANGE_EVENT` 分支转发灯效事件后返回 true，没有找到把本设备 Protocol25 触觉 stage 事件写回 scrolling UI 的操作。

## PID167：应用浏览模式与操作系统滚动行数

应用列表从文件选择器接收 `.exe`，读取图标后合入列表；路径／名称去空格、转小写后用于去重。删除应用和高分辨率开关分别保存 profile。打开浏览模式本身不立即排新的浏览设备任务；实际 foregroundWindow 通知在当前 profile 的 browsingModeEnabled 为 true 时，取 `event.name.toLowerCase()` 排应用浏览事件，进入 `taskMakerSetEnableScrollSmoothByApplication`。

浏览 writer 每次重新读取当前 profile 的应用列表和高分辨率字段，取各路径的最后一个反斜杠分段与前台名称比较。先读 OS `getWheelScrollLines()`：匹配应用且启用高分辨率时设为 1；不匹配但当前值为 1 且高分辨率开启时设为 3。之后读取 `getHapticScrollBrowserModeEnable`，只有实际 enable 与应用匹配结果不同才调用 `setHapticScrollBrowserModeEnable`。OS setter 返回假或设备非 SUCCESS 均把任务标为 error。

这里恢复的是源码固定的 3 行，没有保存并还原任意用户原始行数的分支；关闭高分辨率后也不应凭空补造该恢复动作。共享源码中没有另一条 `setWheelScrollLines` 退出调用，因此不能声称应用退出完整恢复了初始系统滚动参数。前台通知、profile 保存、OS 写入与设备浏览模式分别是不同消费者和结果。

## PID231：三模式选择的实际挂载

PID231 的 Customize 左侧挂载 Mode V2，右侧按 multiPairing 条件选择配对组件。它的 Performance 组件挂载 DPI／轮询率等卡片，没有 Scrolling 导航或 PID167 曲线组件。Mode V2 把 Tactile／FreeSpin／MicroTactile 映射为 0／1／2，预设身份分别为 Standard／Smooth Scroll／Distinct。disabledModes 最多两项，位掩码依次为 1／2／4；恢复先提交禁用集合，再投影当前 mode 与 stage。任务按实际 device 方法选择 `setLKScrollWheelModeSelectedInformation` 或 `setHapticScrollModeSelectedInformation`，再调用 `setScrollFreeNotchedMode`；目标为 profile 1 和大于 1 的匹配板载 slot，异常 catch 返回 false。

## 原件与复核位置

- PID226 UI：`.analysis/inputs/frontend/mouse-226-current/raw/apps.razer.com/synapse/products/226/ui/static/js/6424.1f24d872.chunk.js`，卡片、0～4 滑块、模式顺序、禁用规则与 300 ms debounce 位于 `[214200,218600)`；直接复读原始字节。
- PID226 中间件：`.analysis/inputs/frontend/mouse-mw-226-current/raw/apps.razer.com/synapse/products/226/mw/7846.8d3e3465fc6ae4ee8a38.js`，`restoreScrollDisabledModes` `[1066824,1067253)`；加速 writer `Wn` `[1713707,1714580)`、level `$n` `[1714580,1715368)`、Smart Reel `Yn` `[1715368,1716190)`、threshold `Jn` `[1716190,1716937)`、模式 `qn` `[1716937,1718065)`；直接复读完整函数。
- PID167 UI：`.analysis/inputs/frontend/mouse-167-current/raw/apps.razer.com/synapse/products/167/ui/static/js/main.05d42904.js`；预设 `[5562088,5564231)`、选项 `[5564833,5568674)`、曲线 `[5568783,5576241)`、Custom 卡 `[5576745,5582089)`、Scrolling 挂载 `[5582217,5582393)`。
- PID167 middleware：`.analysis/inputs/frontend/mouse-mw-167-current/raw/apps.razer.com/synapse/products/167/mw/763.1879673c551b57767a72.js`；保存与排队 `[1632019,1635152)`、前台 maker `[1635155,1635507)`、画布坐标转换 `[1686126,1686258)`、Bézier／离散／单位转换 `[1686266,1687956)`；active writer `[1824306,1825029)`、集合 writer `[1825209,1826010)`、Custom writer `[1826251,1829525)`、浏览 writer `[1829714,1831026)`、前台通知 `[1436235,1436630)`、硬件事件消费者 `[1247296,1248294)`；restore 所在完整 owner `[1402088,1433099)`。
- PID167 硬件事件 parser：同一 middleware 目录 `rzHardwareEventsProtocol25Parser.f69e9a24f72f76e05e2c.js`，`[1493,7422)`。产品入口 `main.121b0f27bcf228da20db.js` `[277259,277952)` 显式选 rzDevice25／Orca5；不能因 manifest 可加载 rzDevice30 就把它写成本设备实际 adapter。
- PID231 UI：`.analysis/inputs/frontend/mouse-231-current/raw/apps.razer.com/synapse/products/231/ui/static/js/main.eacf1afc.js`，Customize `[5522752,5524405)`、Performance `[5605725,5606092)`；Mode V2 feature 位于其 middleware `9697.9310c015f990fbf9b4aa.js`。
- 全产品分支索引：`.analysis/results/frontend/mouse-mw-audit.json` 的 `queries["mouse-scroll"].products[product_id].semantics`／`lifecycle_association_chains`／`complete_owners`，完整模块由该 query 的 `complete_owners[key]` 定位回当前原件。

本文完成上述静态语义记录，未执行真实滚轮切换、板载写入或窗口验收。

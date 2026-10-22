# 接收器、底座与采集卡

接收器的硬件配对表、本机设备目录、配置档和 UI 临时候选各有自己的来源。候选出现在扫描列表、设备被 USB 枚举、绑定命令返回以及收到绑定完成事件分别表示不同状态。本文分别记录当前官方页面、协议语义和现行 Rust 的实际调用边界；已有独立 HID 链不能归为纯草稿，页面发出事件也不能算设备写入完成。

## 实际挂载根与页面分支

下表的区间是各产品获取收据所校验原始 JS 的完整根组件 UTF-8 字节区间。产品链接保留各自原件路径、导航数组、页面构造参数及组件区间。导航顺序来自实际根组件，不能由公共能力表补页，亦不能把独立配对工具的导航并入普通设备页。

| 产品 | 完整导航根 | 普通设备页实际顺序与挂载差异 |
| --- | --- | --- |
| [126](products/126.md) | `yP` `[5354916,5362209)` | Lighting、Help；灯光根为左右列，不挂载 179 指示灯 |
| [164](products/164.md) | `VC` `[5376552,5383851)` | Customize、Lighting、Help；配对工具和设备页有不同根 |
| [179](products/179.md) | `Ql` `[5385626,5393795)` | Customize、Help；Customize 内再按平台选择配对组件，右列为指示灯 |
| [207](products/207.md) | `Ih` `[5265728,5274060)` | Customize、Help；配对、电池优化和自动配对在 Customize；导航关闭 profile bar |
| [241](products/241.md) | `nn` `[307740,315091)` | Pairing、Lighting、Help；根位于 `11.219fb515.chunk.js`，不能只检查 main |
| [3331](products/3331.md) | `kc` `[5038129,5044247)`；Setting 导航工厂 `[4554747,4555472)` | 根展开 `NAVIGATION_TABS` 后追加 Help；Setting 四行顺序为 Master、PC、HDMI、Mic，两组导航并存 |
| [3872](products/3872.md) | `ip` `[5376356,5383144)` | Audio、Lighting、Help；Audio 左列两个组件、右列一个组件 |
| [3873](products/3873.md) | `ap` `[5350258,5356990)` | Audio、Lighting、Help；Audio 额外包裹产品图，左右列各一个组件，不能直接复用 3872 树 |
| [3922](products/3922.md) | `Cl` `[406301,412999)` | Sound、Lighting、Help；根在 `2608.894c2893.chunk.js`；Sound 挂载时调用 `getSettings` |
| [3929](products/3929.md) | `Cp` `[5540220,5547122)` | Customize、Help；Customize 将 `setDisplaySaveAlertRef` 下传并指定 `showMouseUse:false`；没有 Lighting 标签 |
| [3932](products/3932.md) | `kP` `[5521015,5528143)` | Lighting、Help；Lighting 将 `setDisplaySaveAlertRef` 继续交给实际编辑根 |
| [3946](products/3946.md) | `Vv` `[7667565,7676390)` | Customize、Lighting、Help；Customize 指定 `isGlobalShortcuts:true`、`showMouseUse:false`；Lighting 还接收 `changeView` |

各根的 `renderView`、当前页、历史前进后退、profile bar 与 Help reset 消费者须一起保留。原版从配置装载的显示值仍属本地配置；不能将这些根里的 profile loader 当作真实 HID/音频 getter。Help/reset 的共享行为见[帮助与重置](../shared/help-reset.md)，灯光编辑器和完整资源条件见[灯光与冷却](lighting-and-cooling.md)。

## 179 的读取、指示灯和配对

179 的完整调用方、任务和协议所有者由 [接收器静态分析](../../../.analysis/results/frontend/receiver-local-runtime-audit.json)定位。UI 原件为 [main.55a48063.js](../../../.analysis/inputs/frontend/accessories-179-current/raw/apps.razer.com/synapse/products/179/ui/static/js/main.55a48063.js)，MW 为 [6120.ef5d145fafdfd0069d53.js](../../../.analysis/inputs/frontend/accessories-mw-179-current/raw/apps.razer.com/synapse/products/179/mw/6120.ef5d145fafdfd0069d53.js)。下列字节区间均指原始 UTF-8 字节。

Customize 根 `fE` `[4929733,4930704)` 的 `LE.Yh()` 经当前 webpack `3249.Yh` 解析为 `[219908,219959)` 的 `navigator.userAgent.indexOf("Windows",0)!==-1`，是平台判断。Windows 挂载 `memo(Re)` `[4787468,4794130)`；非 Windows 挂载 `memo(oE)` `[4808508,4812878)`。两者不能当作同一组件或多设备功能开关：Windows 组件先核对本地配对记录的 master/owner，临时 peer 在实际 PID/edition 匹配前显示 loading，最多重查 30 次、间隔 1 秒，另消费 20 秒的 `SLAVE_CONNECT_EVENT` loading；非 Windows 组件最多重查 5 次，空数组直接返回、不清现有 peer，打开设备时固定请求 Performance。Windows 分支按实际类别请求鼠标 Performance 或键盘 Customize。右列传入 `uppercase:false`。

连接初始化先读序列号与 edition，建立 DualLink 与硬件事件，再检查序列号是否可继续初始化；随后建立状态机、任务所有者、Uma 本地 driver-mode 标志、配置读取和输入监听，最后读取指示灯。成功的空序列号映射为 `NOSERIALNUMBER`，抛异常则保留空串；序列号解析保留所有 32…126 的可打印 ASCII，不以第一个 NUL 为终点。edition 最多读三次，失败间隔 100 ms，解码 edition、固件与 layout，而本产品 UI layout 固定为 0。不能把其中一种读取失败等同于首次设备初始化。

硬件事件监听早于序列号 gate：READY 且序列号仍为空时，后续无线 CONNECT_EVENT 会再次尝试对应 setup。setup guard 在状态初始化前置位；指示灯 getter 的拒绝进入顶层诊断 catch，因此不能假定所有失败都有两秒后的自动重试。Uma 的 `getDeviceMode/setDeviceMode` 在此只是本地 `isDriverMode` 标志，不产生 HID 模式写入。

| 操作 | `[数据长度, command class, command id]` | 返回或载荷边界 |
| --- | --- | --- |
| 序列号 | `[22, 0, 130]` | 实际报告字节经可打印 ASCII 过滤 |
| edition | `[3, 0, 134]` | edition、固件和 layout 分别解码 |
| 扫描 | `[1, 0, 70]` | 开始 status=1；取消 status=4 |
| 绑定 | `[3, 0, 65]` | mode=1，使用完整选中候选 |
| 解绑 | `[2, 0, 66]` | 使用实际已配对的 dongle PID |
| 配对状态 | `[80, 0, 191]` | 使用真实状态表；本地目录用于身份关联 |
| 指示灯 GET | `[1, 7, 144]` | `jsonData.status` 为 truthy 时才发布 |
| 指示灯 SET | `[1, 7, 16]` | Connection=1、Battery=2、Warning=3 |

指示灯的 UI 初值 1 属于 reducer 默认值。getter 对 0/空值不发布，未知的非零状态仍可进入 reducer；两者都不能解释成已确认的 Connection 状态。UI 点击先乐观更新，再发 `ON_SET_INDICATOR_LED`。task maker `[1663508,1663990)` 的 `deviceAccess` guard 同时包住配置保存和 SET 排队；仅获得 macro storage 锁不能保存该设置。实际 runner `[1844811,1845556)` 在 SET 成功后才更新缓存，失败保留错误；源码未追加 GET 或自动回滚。尚未初始化的任务管理器 `get()` 会抛异常，不存在可安全吞下动作的预初始化队列。

配对弹窗挂载先发送 `DUALLINK_BIND_INFO`；扫描发送 status=1 和大写类别，只有一个候选时自动进入绑定，否则由用户选择。绑定携带完整候选，不能只传 UI 下标。实际 `PAIR_STATUS_UPDATE` 的 TIMEOUT 发布错误并撤销注册；SUCCESS 继续读取 edition/layout、关联真实硬件与本地产品目录、发布绑定结果并维护配对记录。普通命令发送成功不能代替这一事件链。绑定/解绑失败显示对应状态，部分失败状态四秒后恢复；错误计时器由弹窗的清理函数清除。窗口/storage 监听和 MW_RESPONSE_UI 订阅分别在卸载时移除。

关闭配对对话框会清错误计时器并发送 `DUALLINK_CANCEL`。该动作忽略类别，清 pairing flag，并始终通过主 runtime 设备执行 `deviceScan(4)`，即使当前未在扫描；失败仅进入诊断。这不是销毁新建类别实例或只清本地候选。底层 `LINKER` 的 transaction ID 为 `224 | counter`，counter 到 31 时重置；scan/pair 清除 namespace 位。类别工厂每次创建新实例：`MOUSE` 对应命名为 DualLinkKeyboard 的适配器，普通事务 `128 | counter`、scan/pair 清 128；`KEYBOARD` 对应 DualLinkMouse，普通事务为 counter、scan/pair 加 128。不能按照类名自行交换这两个分支。主设备取消沿用其 counter，清除 224 namespace；主报告 ID=0、claim interface=0，主机写入报告为 91 字节。

本地文件固件传输与准备过程见[固件与安装生命周期](../host/firmware-and-installation.md)，设备扫描和配对各自保留独立的状态与失败结果。

## 164/207/241 与多设备差异

164 的 [rzDevice25Linker](../../../.analysis/inputs/frontend/accessories-mw-164-current/raw/apps.razer.com/synapse/products/164/mw/rzDevice25Linker.0bcb0968b55a39c2a1ad.js)使用 Linker 事务空间。`getMultipleDeviceWirelessConnectionStatusV2` 后先过滤 PID=65535，再选择第一条有效 secondary。连接最多尝试三次；传输由容器锁串行化，核对真实写入长度，再读取应答/决定重试，`finally` 释放 HID mutex。不得由“函数没有抛错”推断设备已收到完整报告。

207 的 [rzDevice25LinkerMultiDevices](../../../.analysis/inputs/frontend/accessories-mw-207-current/raw/apps.razer.com/synapse/products/207/mw/rzDevice25LinkerMultiDevices.48a7bd27ece6997a41a7.js)包含 Linker、Uma 和 MultiDevices 三个实际模块。MultiDevices 承接 Uma 本地模式标志，但其 pairedDevices 来自真实 V2 状态表：过滤 65535，优先选上次表中不存在的新 PID，否则取第一条有效项，再替换 pairedDevices。其 secondary 电源事件按 DualLink 状态、非空 pairedDevices、record/event ID 判断，不固定在单一缓存 secondary。不能把 164 的单设备规则直接共用于 207/241。

| PID | 实际注册差异 | 页面与配置边界 |
| --- | --- | --- |
| 126 | Mouse Dock；兼容 122/124/134/143 | 挂载鼠标容器；不能套用 Uma 指示灯和配对弹窗 |
| 164 | Linker、region=15、dockCompatible=170 | 使用本产品实际 Lighting/配对组件 |
| 207 | MultiDevices、single profile、auto pair、multiple devices | 独立 `deviceUI` 配置，组件状态包含 `showMultiPairingFeature` |
| 241 | MultiDevices、multiple devices、region=15、dockCompatible=170 | 按该产品实际页面与区域呈现；不能因共享适配器推断完全相同的 UI |

每个配对表记录的 owner、device container、产品身份与当前连接代次必须关联。本地配对索引用于关联和打开实际设备页，不是当前无线连接或硬件绑定表的替代读数。

207 的 `showMultiPairingFeature` 决定独立工具与内联旧配对器，不能与 179 的平台判断混淆。其旧组件有两个独立挂载 effect，各发送一次 `DUALLINK_BIND_INFO`，随后延迟查询连接类型和设备信息；不能为去重而悄悄删掉一个调用。电池优化和自动配对的 reducer、设备响应及配对状态分开；本地已有默认对象不表示设备已读到 50% 或自动配对关闭。

## 207 的自动配对、电池优化与真实确认

本节依据 [207 UI 获取收据](../../../.analysis/inputs/frontend/accessories-207-current/acquisition.json)与 [207 MW 获取收据](../../../.analysis/inputs/frontend/accessories-mw-207-current/acquisition.json)。UI 字节区间来自 [main.ca032f7b.js](../../../.analysis/inputs/frontend/accessories-207-current/raw/apps.razer.com/synapse/products/207/ui/static/js/main.ca032f7b.js)，下文 MW 区间除另注外来自 [6259.126c84a2cb9fa31f5a24.js](../../../.analysis/inputs/frontend/accessories-mw-207-current/raw/apps.razer.com/synapse/products/207/mw/6259.126c84a2cb9fa31f5a24.js)。

实际 Customize 根 `Eh` `[5262587,5263373)` 左列挂载自动配对 `aU(EU)` 和 `Jm({isMouseMat:true})`；右列在产品支持且 `global.multipairing.provisionFlags` 为真时挂载独立配对入口 `$U`，否则挂载旧内联 `km(zm)`。电池组件 `Jm` `[5242134,5244809)` 的阈值只有 50、55、60、65、70、75、80，另按配对表中的鼠标及其 status=1 决定可用状态。通用 charging override UI 在本产品不挂载：当前 MW `power.hasBatteryChargingOverrideEnabled=false`，根没有传相应 support prop。

UI reducer 初值、MW 本地默认和设备实测必须分开：

| 来源 | 电池优化 | 自动配对 |
| --- | --- | --- |
| UI `hs` `[4891908,4892069)`、`Fs` `[4894335,4894434)` | enabled=true、value=50、override=false | enabled=false、value=0 |
| MW [main.35716b945eedcc2ed000.js](../../../.analysis/inputs/frontend/accessories-mw-207-current/raw/apps.razer.com/synapse/products/207/mw/main.35716b945eedcc2ed000.js)完整默认模块 `[143486,145322)` | enabled=false、value=50 | enabled=true、value="" |

MW 默认模块还规定单配置档、`canPairTwoDevices=true`，edition 0/128 共用默认档结构。配置发布完整所有者 `[1372892,1403807)` 遍历当前本地文档，通用分支调用 `fe(...).catch(...)` 而没有逐项等待。`fe` `[1256715,1257256)` 对自动配对先发布本地值，再排 `ON_SET_AUTO_PAIRING`（LOCALSTORAGE）；当前整个 MW 输入集合只有 `getDeviceAutoPairingMode` 方法声明，没有实际 getter 调用者。因此原版初始化按已有本地配置写接收器，不能把自动配对 reducer 当作设备读数，也不能凭字段排列宣称电池读取已先完成。

电池 feature `d` `[1081766,1083460)` 有独立 `initFunction`：先读实际 bind info，有 MOUSE 才通过类别适配器调用真实 `getBatteryChargeLimiterControl`；没有 MOUSE 时发布并保存官方本地默认。bind-info 异常先 catch 成 null，也会进入无鼠标默认分支；该原版行为不证明设备已恢复 50%。实际 getter 异常则继续抛出；返回无 jsonData 时不能推断 enabled/value。配对和解绑成功回调还会重新读取 limiter，hook 构造器 `[1084818,1084999)` 与配对初始化 `Ge` `[1197221,1199565)` 共同规定该生命周期。

| 操作 | `[长度,class,id]` | 真实编码或解码 |
| --- | --- | --- |
| 自动配对 GET / SET | `[1,0,0xD5]` / `[1,0,0x55]` | SET byte0=isEnabled；SUCCESS 才解析 data[0] status/statusEnum；本产品 getter 未被初始化调用 |
| limiter GET / SET | `[1,7,0x92]` / `[1,7,0x12]` | SET byte0=(isEnabled<<7)\|value；SUCCESS 且 packetSizeReturned>0 才解析 enabled=(byte0&0x80)>>7、value=byte0&0x7F |
| V2 配对状态 | `[80,0,0xBF]` | byte0=count；后续逐条 status、PID 高字节、PID 低字节；过滤 65535 和 receiver 本身后再关联本地目录 |
| 配对 / 解绑 | `[3,0,0x41]` / `[2,0,0x42]` | 配对 mode 后接 PID 高/低字节；解绑接 PID 高/低字节 |
| 扫描 / 取消 | `[1,0,0x46]` | status=1 / status=4 |

协议完整所有者为 auto `[636986,637357)`、limiter `[696265,696660)`、V2/配对/解绑/扫描 `[634865,635828)`；常量 `[644387,645291)`、`[699872,700057)`。PID 为高字节在前，由 MW main 的 `Jz/E4/l7` `[120612,120662)` 核实。

UI 写入先乐观更新。通用 maker `pe` `[1251557,1256410)` 先按全局 storage 配置保存本地值，再排 NORMAL_SKIPPABLE 设备任务；`ue` `[1249945,1251556)` 提取有序参数、await 实际协议调用，返回后检查 abort signal，并把返回对象的 status 改为 completed。这里没有额外 GET 或自动回滚，maker 的 completed、runner 的 completed、协议应答和独立设备读回不能互相替代。

### 配对事件、解绑恢复、取消与退出

旧内联根 `zm` `[5235141,5238159)` 有两个独立 bind-info 挂载 effect，另外以 1 秒定时查询 CONNECT_TYPE/DEVICE_INFO。弹窗 `wm` `[5229238,5235140)` 有键盘 status 与鼠标 status2 两条状态轨；单候选必须与正在扫描轨的类别相符才自动绑定，键盘的特定 dongle 候选还经过继续确认。MW scan `[1202522,1205087)` 从当前真实扫描完成事件生成候选，并按官方 `DevicePairingBuddies` 筛选；不能用本地产品列表造候选。`Se` `[1193022,1193453)` 用 `getRzDevice(category,true)` 发配对，仅硬件 `PAIR_STATUS_UPDATE` SUCCESS 才进入 `Ee` `[1190620,1192694)` 读取 edition/layout、发布结果和维护本地关联；TIMEOUT 会撤销对应注册。

`Ge` `[1197221,1199565)` 的常驻 autoBind 只在手工扫描标志 Pe=false 且硬件 SUCCESS 时重读 V2 表、找 MOUSE，再复用 `Ee(...,false)`，因此自动与手动成功消费者不能分割实现。类别工厂 `lt` `[2215079,2215801)` 每次新建适配器：普通 limiter 访问使用默认 false，手工 scan/bind/unpair 使用 true，类别到 DualLinkMouse/Keyboard 的路由会随该参数交换。对应完整模块为 48957 `[934489,942746)` 和 79097 `[926755,934488)`，不能按类名猜实际 namespace。

主 adapter 由前述 `rzDevice25LinkerMultiDevices` 的 Linker→Uma→MultiDevices 继承链选择。每次真实主机传输核对写入长度，按容器 mutex 串行，在 finally 释放 HID mutex；临时类别适配器没有在通用 setter 返回处另调 terminate，不能把 mutex 释放说成所有 adapter 已销毁。

解绑 `Ae` `[1193454,1195699)` 先处理真实 master/owner 路由，正常结果须有 jsonData 才发布。异常后等 2 秒重查 bind-info，再按目标是否仍在表中判断结果。成功路径移除本地关联，500 ms 后发起最多 20 轮、初始间隔 1 秒且递增翻倍的重连，并重置主/次 PID；源码中的 `if(20===o)` 在 `o<20` 循环内不可达，不能据此宣称最后间隔被清零。这些恢复步骤不能被一个同步成功返回代替。

关闭弹窗执行 `vm` `[5213657,5213696)` 清错误计时器，再发送 `DUALLINK_CANCEL`。实际取消 `[1205138,1205366)` 只清 Pe 并通过主 adapter 发 `deviceScan(4)`，失败进入诊断；它没有注销 scan、bind、autoBind。注册/注销完整 owner 为 `[1242672,1242869)` / `[1242870,1242961)`，内部 Map 没有统一 clear 路径。旧 `wM` `[5210035,5210113)` 会在同事件新订阅时先 off 旧回调，但旧内联 root 的订阅 effect 与 1 秒 timeout 没有返回卸载清理函数，不能写成关闭时所有 listener/timer 已释放。

全设备退出 `hn` `[1364778,1366851)` 另处理模式恢复、关闭 DeviceBC、全局 shortcut 注销、设备 runtime storage、真实 USB 移除、配对 slave 清理、通知移除及 generic feature shutdown，最后关闭窗口；其中 terminate/handleBeforeUnload 未等待。该退出链与弹窗取消是不同所有者。自动配对组件 `EU` `[5244971,5250923)` 明确在卸载时移除 click/windowstorageevent 回调；其鼠标在线状态还消费 connectedDevices 的真实 powerStatus，鼠标页面导航通过实际 MWWindowName 和窗口查询定位。

207 的专用 Rust 页面已有动作、页面代次和本地草稿，但下表所列 Shell 生产分支仍没有 service/state/window adapter。179、164/241 已有独立 HID 链不能据此算作 207 已实现；这里已找到的协议、初始化条件、事件与恢复顺序应直接驱动该产品的独立实现及测试。

本节对 4 份当前原件核对 acquisition 的实际长度与 SHA-256，并与既有 full capture 的 original_inputs 交叉核对；62 个完整 AST 所有者边界一致，源文件最终快照未变。UI/MW capture 各仍有 1 个失败阶段，未据此提升整套捕获状态。本轮仅静态阅读，未执行厂商 JS、DLL、设备写入或模拟测试；设备/DLL 比对及运行时取消验收保持未验证。

## 采集卡和带音频的底座

3331 的 [rzDeviceKateT2](../../../.analysis/inputs/frontend/accessories-mw-3331-current/raw/apps.razer.com/synapse/products/3331/mw/rzDeviceKateT2.5614aee278520829235f.js)使用独立的 17 字节报告，首字节 `0x5a`，不能按通用 25 协议编码。其完整当前包装器明确给出下列载荷：

| 操作 | 命令字节 | 返回/写入 |
| --- | --- | --- |
| edition | 28 | data[6]、data[7] |
| 序列号高/低段 | 24 / 26 | data[6…12] + data[6…13]，过滤可打印 ASCII |
| 全通道音量读取 | 11 | mic=6、speaker=7、HDMI=8、PC=9、PCmic=10 |
| speaker/mic/HDMI/PC/PCmic 音量写入 | 12 / 13 / 14 / 15 / 16 | 新值位于 byte[7] |
| 全通道 mute 读取/写入 | 38 / 39 | 读取后保留伴随通道，仅改变目标，再写 byte[7…11] |
| HDMI 状态 | 45 | connected=6；宽度=8/7；高度=10/9；fps=11 |
| 固件版本 | 22 | data[6…9] |

custom init 报告的完整字节为 `[90,73,84,69,32,84,101,99,104,46,32,73,110,99,46,0,0]`。主机按实例 mutex 发送，核对写入数为 17，再以 report ID=90、`noSlice:true` 读取；WebUSB 使用 value=858 和实际 interface。当前主机接收分支将返回 data 标为成功，没有完成通用帧头/status 校验，因此不把它描述为已验证的 ACK。模式 getter/setter 同样只维护本地标志。

实际音频读取由 [6120.2586ebf1c7a5dd7992ce.js](../../../.analysis/inputs/frontend/accessories-mw-3331-current/raw/apps.razer.com/synapse/products/3331/mw/6120.2586ebf1c7a5dd7992ce.js) `[2313090,2313813)` 所有：先 getMute，去掉 pcMicMute，再 getVolume、去掉 pcMicVolume，把四个真实值和 `!mute` 形成初始 UI 对象。函数内部的 80 是待组装容器初值；任一 getter 失败都会抛错，没有“读取失败使用 80”的返回路径。初始化调用方 `[1390268,1392648)` 发布 `MW_INIT_AUDIO_MONITORING_STATUS_TO_UI`，再读取 HDMI 状态。HDMI 插拔由硬件事件消费，连接后等 2500 ms 再读取并显示实际分辨率；通知不是连接状态来源。

四个动作经状态机 `ON_SET_MASTER/HDMI/MIC/PC_VOLUME`、task maker `[1651632,1653013)`、对应真实 runner 进入设备/本机音频通道。配置保存在 profiles[0]，发布的 task-maker completed 仅确认该阶段；实际 runner 仍须独立判断返回值。master/HDMI/PC 先读全 mute，必要时改 mute；仅在 enabled、数值类型且 0…100 时写音量。mic 在已注册 audioProtocolInterface 时走本机麦克风接口。`notCallingAPI`、错误和真实调用结果分别保留，不能合并成设置成功。

| PID | 本机音频通道 | 实际页面差异 |
| --- | --- | --- |
| 3331 | simpleService、虚拟音频通道、audio monitoring | Setting 四通道，叠加公共导航/Help；不能把两组导航看作互斥分支 |
| 3872、3873 | simpleService；注册真实音频事件、dock edition API | Base Station V2 与 Thunderbolt 4 Dock 各用本页挂载 |
| 3922 | mappingEngine；注册音频服务事件 | Sound/Lighting |
| 3929 | mappingEngine；处理 unsupported mapping events | Customize/Help，映射与音频端点分开；没有 Lighting 标签 |
| 3932 | mappingEngine；同类映射事件处理 | 当前实际导航为 Lighting；没有由此新增 Sound 页的依据 |
| 3946 | `isAudio.augustT2="true"`、simpleService | Customize/Lighting；本机默认端点观察有独立消费者 |

simpleService 首先按当前设备识别扬声器/麦克风 ID，再读真实 volume/mute，写入同时携带目标 ID、mute 和音量。mappingEngine 使用实际默认扬声器/麦克风 getter、setter 与 speaker/microphone/sidetone 事件；默认端点和 dock HID 容器不是同一身份。两种通道由各自注册参数选择，不能在失败时改用另一端点伪装成功。共享音频接口默认最多九次读取尝试、间隔 2000 ms；返回值仍要求存在且 result 不是 false，不代表本产品每个消费者都已严查全部字段。

simpleService 注册 speaker/microphone 事件和 sidetone 配置，再监听 `audioServiceEvent`；mappingEngine 注册对应服务事件并监听三种变化。各分支的注销和回调移除须分别核实。3331 cleanup `[1403213,1405441)` 还包含任务/观察停止、退出模式、USB 资源移除、通知清除、音频注销和设备 terminate；部分 terminate 未等待，不能据此保证所有原生工作已退出。3946 的额外端点观察和退出缺口见下节，不能把共有 unregister 包装器存在当作该消费者已执行清理。

## 3946 的取放自动动作

当前原件为 [UI main.e3214cd7.js](../../../.analysis/inputs/frontend/accessories-3946-current/raw/apps.razer.com/synapse/products/3946/ui/static/js/main.e3214cd7.js)、[MW 6120.0e93878efc66d9af5fe6.js](../../../.analysis/inputs/frontend/accessories-mw-3946-current/raw/apps.razer.com/synapse/products/3946/mw/6120.0e93878efc66d9af5fe6.js) 和 [MW main.7b1c8d7d8f98b35284d1.js](../../../.analysis/inputs/frontend/accessories-mw-3946-current/raw/apps.razer.com/synapse/products/3946/mw/main.7b1c8d7d8f98b35284d1.js)。下述区间均为原始 UTF-8 字节。主入口实际注册 `rzDevice25`、`isAudio.augustT2="true"` 和 `audioProtocol.simpleService({registerAudioServiceEvent:true})`；不能用包内另一份未选用的默认配置推断实际音频后端。

主入口的产品描述为 VID=5426、PID=3946、claimInterface=2、version=1，`reportIDs` 为 `{4:"razerKeyReportID",5:"hardwareEventReportID"}`。设备默认自动规则仅有关闭的 chroma，两条取放 lane 也关闭，所带默认 Wave 不表示应在连接时写入硬件。默认亮度 100、Wave direction=3 等是配置默认值，不能据此判定设备首次初始化。

### 规则编辑、编译与发布

UI 的六类规则 ID 为 0/chroma、1/outputDevice、2/inputDevice、3/globalShortcut、4/launchGame、5/macro。每条规则含 `id/name/isEnabled/pickUp/putDown`；lane 含 `isEnabled/data`。编译器 `Un` `[6837348,6837499)`、`Gn` `[6838547,6840874)`、`vn` `[6840875,6841227)` 只消费启用 lane 的 `data[0]`，跳过未命名、外层禁用或两条 lane 都禁用的规则，并按当前规则数组顺序生成映射。输入常量位于 MW main 的对应常量模块以及 UI 模块 69937 `[6025370,6032902)`：

| 动作 | UI 输入身份 | 原生触发含义 |
| --- | --- | --- |
| 放下 | `DKM_M_93`、`DKMInput`、`Key93`、key=93、action=`down` | key 93 的新按下，原生 flag=0 |
| 拾起 | `DKM_M_94`、`DKMInput`、`Key94`、key=94、action=`up` | key 94 的新按下，原生 flag=0 |

这里 action 的 `up/down` 表示取放方向，不能解释成原生 key release/press；原生 flag=1 才是按键释放。

| 规则类型 | 生成的输出与分支 |
| --- | --- |
| chroma | 高级效果为 lightpac 的 `{ID,Name}`；关闭效果为 `offLightingGroup:{offLightingAssignment:"BrightnessToggle"}`；其余为完整 `specificQuickEffect` 配置 |
| outputDevice / inputDevice | 保留所选端点对象，分别生成 `outputDeviceGroup` / `inputDeviceGroup` |
| macro | 保留宏引用，设置 `macroPlaybackOption:"Once"`、`repeatCount:2`；MW 编译时 Once 覆盖为 `repeat:1`，生成 down 的普通宏与 disabled 的 up，实际只排一轮 |
| globalShortcut | 按 SWITCH_LIGHTING、SWITCH_DEVICE_PROFILE、SWITCH_DEVICE_SENSITIVITY、MACRO、TEXT_FUNCTION、LAUNCH_PROGRAM、MULTIMEDIA、WINDOWS_SHORTCUT 转换各自 assignmentGroup，不是另建一组全局热键注册 |
| launchGame | 拾起依次选择 launch_url、首个未被排除的 launch_helper（没有则取第一个）、launch_exe 或 location，并保留参数和引号；放下仅在 lane 启用且 `isMappingAgain` 时生成 Escape |

保存 `[7634091,7635193)` 先 ADD/EDIT，再触发 `le.HTo` 并关闭编辑器；toggle `[7363588,7363680)` 和确认删除 `[7633217,7633304)` 也各自触发 HTo。HTo 位于 reducer `wn` `[6841737,6849142)`，重新编译后发送 `ON_SET_KEYMAPPING {mappingList,viewIndex,sidepadLayout,isMultipleMapping:true}`。规则加载响应 `le.nI5` 仅替换规则列表，不能把它描述为必然重新发布原生映射。MW 自动规则 maker `[1678724,1681649)` 更新 profile、版本、本地缓存与 UI；规则保存和映射发布是两个动作。

MW `ON_SET_KEYMAPPING` 分发调用 `25947.sti=Lt` `[1556697,1560448)`，经 `je` `[1512841,1514076)` 生成 appEngine、hash 和受冲突重试保护的存储更新。`generateAppEngineMultiMappings` 所在类 `[2088281,2093780)` 先展开已有 multi，再按 input.key 分组，分别形成 flag=0/1 的有序 `multi.outputs`；flag=0 过滤 disabled，flag=1 保留其输出。保存 profile.appEngine 和实际 mappings 后，`y.mZ` 进入 storage owner。`h.lY=Ee` `[2237549,2238253)` 只发布 `DEVICE_RUNTIME_DATA`，**不是原生映射发布**。

实际发布由模块 60763 的 `PostSetLocalStorage` hook `[2213013,2213628)` 调 `20236.M6=Pe` `[2243894,2245251)`：只有 feature 允许且默认映射、activeProfile、profile hash/gamemode 等发生对应变化时，才向 `synapse_${actual_device_pid}_${container}` 写完整投影。没有序列号时 `y.mZ` 不保存。hook 内部异步 IIFE 未由调用方等待，因此 maker completed 不证明宿主观察者已处理、原生解析已完成或动作已执行。原生映射解析确认链及其仅确认内存映射的边界见[宏与按键映射](../shared/macros.md)。

### report 4 到映射执行的原生链

以下 RVA 均相对当前官方 mapping_engine.dll 的 Image Base `0x180000000`，完整伪代码、汇编、交叉引用和原字节位于[既有原生捕获](../../../.analysis/results/native/6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a/capture.json)。已经核对虚表在实际 PE 中的函数指针，不按反编译命名猜分发目标。

| 阶段 | 实际所有者与行为 |
| --- | --- |
| 打开与读取 | `ab7f0→ac040→ac850→123720→123c50→122fd8` 按实际设备身份建立 HardwareEventEndpointWin；CreateFileA 使用只读、share=3、OPEN_EXISTING、OVERLAPPED。`1232f8` 每次请求 100 字节；`123600` 检查完成错误和实际字节数，先重新投递另一缓冲区，再转发非空的已完成数据 |
| 跨线程与报告分类 | `1244e0` 核对 endpoint owner 并复制实际长度；实际 USB 接口虚表 `0x2e7e38+16/+24` 分别为 `6a330/a8de0`，经 `6a330→9d7d0→6a470` 转设备线程。`a8de0` 根据已解析 descriptor 查报告类型，长度不足直接丢弃；BLE 前缀仅在相应 flag 置位时剥离，本产品默认 USB 不剥离 |
| reportID 解码 | `e7080→e3bc8` 解析字符串枚举。raw reportID=4 对应 `razerKeyReportID`，即内部 kind=6，进入 `7fe46`；raw 5 对应 kind=8，进入硬件事件 `7f43e`。硬件事件枚举 LIFTED=5/DOWNED=6 不能替代 report 4 的 DKM 93/94 生产者 |
| 按键状态差分 | `7fe46` 将 byte[0] 当 reportID、byte[1…] 当键集合，并按 reportID 保存前次集合；从头到首个 0 为止生成新 press，再为前次存在而本次所有 key 字节中不存在的键生成 release，最后替换集合并移除 0。重复报告不重发 press；新 press 排在旧 release 前。全局 enabled byte `0x3811d9` 仅 gate 新 press，原 PE 初值为 1，由 `582e0/58420` 开关 |
| 原生输入与匹配 | `fa4d2→fa90e→104008` 形成 type=35 的 RazerKey 输入：key、flag=0/1、实际 GetKeyState 修饰键。仅 descriptor.dualLinked=true 才附 reportId，本产品默认未设置该 flag。`66d10` 查 active profile、HyperShift 与对应 typed mapping；version=1 使用 `e6766→f9604`。虚表 `0x2ef1d8` 的 `103d20` 查找身份包含 type/key/flag/可选 reportId，**不含 modifiers**；`103ec0` 的跟踪身份只含 type/key |
| 执行分发 | `69530` 先 `677ba` 跟踪输入，再处理 chord/SnapTap，进入 `797a8`。本产品默认 version=1 走 `ad38c`；multi 的实际 type=29，经 `8ca44` 按 outputs 原顺序逐项执行：keyboard 走 `ae010`，其余替换 output 后递归 `797a8` |
| 返回 MW 的动作 | `f2f76` 解析 multi；已识别宏、键盘、启动程序等走对应原生类型。四类 `specificQuickEffect/offLightingGroup/outputDeviceGroup/inputDeviceGroup` 保留完整 JSON 进入 `f49f8` 的 Unsupported，实际 type=40，经 `858fe` 返回宿主，再由 MW 执行灯效或音频动作。Unsupported 在此是分层分发方式，不表示产品能力缺失 |

例如从空集合依次输入 `[4,93,0]`、重复同一报告、`[4,94,0]`、`[4,0]`，可从上述代码推导出 press93、无新事件、press94 后 release93、release94。这是独立测试期望的静态依据，**不是实机采集报告或官方 DLL 已执行的收据**。还须单测首个 0 后非零字节、仅报告 ID、空数据、全局 enabled gate、dualLinked 和不同报告 ID 的集合隔离，不能仅用这个正常序列宣布报告解码全覆盖。

### 普通宏的实际执行、错误与取消

MW 宏输出编译 owner `[2061108,2061636)` 将 Once 编成 `repeat:1`，NTimes 才使用 repeatCount；3946 因而生成 down `{type:"macro",guid,flag:0,repeat:1,toggle:0}`、up `{type:"disabled"}`。这条链不能解释成执行两遍或释放即停止。其它模式仍须分别验证：原件中 `PhasedMacro` 的 flag=5 与 `"Phased"` 的 `type:"phasedMacro"` 是不同字符串／分支，不能自行合并。原生 `ed8d2→101ff0` 检查非空 GUID 及 flag 范围 0…4，嵌套事件解析 `ecee3` 又禁止 flag≥2；这些限制不能按 UI 枚举名推断相容。

宏内容来自独立的 `synapseMacros` 数据链：`562c8→f68c0` 建存储观察与专用线程，`f6a70→f6bd0→f7106` 解析候选宏 map，检查数组、GUID 唯一性、appEngine/hash/events。相同 GUID/hash 可复用旧对象；新 hash 经 `f86ae` 去掉 hash 字段后重新序列化计算比较，候选全部成功后才由 `f6bd0` 在锁内替换活动 map。执行查找沿 `5d020→5cf60→f7066` 按 GUID 取活动对象。映射存储 ACK、宏数据解析成功、宏排队和事件执行完成为四个不同结果。

实际 USB delegate 已沿构造实参及 PE 虚表闭合：`57380` 将 manager+24 传给 `a89f0→64e8c→546e8`；`55de4` 在该接口安装 `0x2e4cd8`，其 virtual+48 是 `5d600`。宏输出 type=24 的虚表 `0x2ee948` 首槽为 `101cf0`，执行分支 `893dc` 通过上述接口进入 `5d600→61aa0→a9500`，不能取 manager 主虚表相同槽号代替。`a9500→5ed88` 按 GUID 与 caller 找活动执行，flag=0 遇已有活动项时忽略新启动；当前 Once 经 `5ec3c→126f1c` 取得或创建 executor，由 `127892` 投递 executor thread，再经 `127aa0→128692` 建 frame。只有成功排队才登记活动项，但这仍不是设备输入已生效。

`128692` 拒绝找不到 GUID、空 events 和栈深度达到 100（`max macro recursion`），frame 独立保存 repeat、事件迭代器、valid 和 completion。`12b5b0` 每步再投递，repeat=0 表无限，非零每轮递减，当前 repeat=1 只跑一轮。`ecee3` 可解析 audio/clipboard/delay/keyboard/launch/loop/macro/mouse/mouseMove/randomDelay；`12be6e` 对 keyboard、launch、mouse、nested macro、mouseMove 使用各自 bool／回调，随后 `12c8f0` 合并 valid 并推进；audio、clipboard、delay、randomDelay 的底层 bool **没有在该分发中合入 frame.valid**。因此 frame 成功不代表全部 OS 副作用各自成功。`12bde4` 弹出 frame 并完成回调，`12dcf0` 再进入 context 收尾 `12a8ca`。

默认 Windows 键盘有独立执行线程，初始化为 `130b68→126388→1264d0`，按键为 `130c1a→126902→126a80→12ee96→12f3ce→12fd66`。`126902` 的 bool 表排队，`12ee96` 的 bool 表键盘输入合法性；下层 endpoint 发送失败没有自动并入该合法性。按下／释放、系统重复 delay/rate、repeat 与自动 release 分别在 `12f312/12f3ce/12fe22/12ff70/12fb30`。实际 endpoint 虚表 `0x2f2e70` 的 Start/Stop/isValid/SendInput 为 `134800/134af0/1107f0/134c80`；`134a02` 打开真实 endpoint，`134c80→13579a` 将 type=21/27 转成 32 字节结构，通过 `0x88883020` 写入驱动。同步成功或 ERROR_IO_PENDING=997 只表示接受；`135c20` 才按实际完成 status=0 回 bool。但当前宏／专用键盘调用传空 completion，**没有逐项等待物理输入确认**。未走专用键盘的 `130cf8` 在 endpoint valid 时调用 `1345aa` 后直接 true，否则才由 `11c510` 调系统 `SendInput(1,…,40)` 并检查返回是否为 1；不能把该 fallback 扩大到所有专用键盘发送失败。

`12a8ca` 克隆仍按住的输入，逐项调用 virtual+96 生成 release，经 keyboard `130c1a`／mouse `130cf8` 发送，不检查每项 release 的结果；然后清 pressed map、停止 endpoint、清 owner 并排键盘 Stop。最终 status 必须以完整汇编为准，反编译伪代码遗漏了一个参数：valid 且未 cancel 为 0、invalid 为 1、valid 且 cancel 为 2。manager+32 的实际虚表 `0x2e4d38+8→5dc90→5de50` 删除活动项、推进队列／完成 stop 等待，**仅 status=1** 向设备错误接口 virtual+496 发事件；status=2 独立表示取消，不能标为成功。

错误接口是 USB base+136 的 `0x2e7e68`（RTTI offset=136），virtual+496 的实际目标 `77740` 生成 `{type:"error",error,output}`，`77c40` 在保留的 base+208 callback 存在时投递，`9e490→18db0` 调该 callback，导出适配 `25600` 再检查 initialize gate 并发送 type=5。addDevice/addUsbDeviceWithoutFilterDriver/addUsbDevice 的 `25bf0/26e60/277d0` 均安装这个适配。宿主 `createCbDeviceEvent` 完整 owner `[113371,114222)` 将 type=5 转成按设备键及有效 webContents 路由的 `deviceEvent`；MW `K/$/F` 分别位于 `[1286752,1286843)`、`[1286846,1286891)`、`[1286894,1287128)`，只按 productId 过滤，仅对 `file not found`／`fail to launch` 显示启动错误，其余宏错误没有由这条消费者统一弹窗，也没有连接 generation 校验。它与 unsupported 的 base+736 回调是不同路径。

主动取消 `12772a→11a922` signal 事件，`12b5b0` 在后续步骤观察，delay 的 `12ab1e/12af54→11aa78` 也等待该事件，再走相同收尾。`57cd4` 停 caller，`5d770→5d890` 停匹配 mapping，`57090` 全局停 active／queue／exclusive 并等待活动 map 清空；`56f10→127574→127760` 清 executor。endpoint `134af0` 执行 CancelIo/close，`135c20→134b64` 只在 pending=0 时完成 shutdown，键盘清理另经 `130bba→12667e→1267d0→12edae`。当前取放宏的 up 为 disabled，不能把该输出当成已经调用上述取消、release 或排空链。本节定位的是普通 Once 及其共享调度／清理边界，尚不代表其它模式和各类宏事件的全部 OS 参数分支均已验收。

### unsupported 回调、事件消费者与退出

原生导出 `2cc00/2d410/2dc90` 的 ABI 分别是 `void(device_json,completion)`、`void(device_json,completion)`、`void(device_json,persistent_event,completion)`。它们核验非空 container 及非零 VID/PID，沿 manager `59150/59270/593a0` 查设备后转入设备接口；无引擎线程、无 manager、无设备分别产生 `invalid thread`、`invalid device manager`、`device not found`。`6c700` 首次置登记位，重复登记返回 true/`Warning: Already registered`；`6c8f0` 清登记位，未登记返回 false/`not registered`。不得将反编译器推测的返回寄存器解释为调用成功。

`858fe` 仅在排队前核对登记位与 persistent callback；`a3dbc→1cb4a0` 保留 callback，`a3eee` 深拷贝输入/输出 JSON 和 tick，向返回线程投递。`a3d10→a3d28` 执行捕获的 callback 时不重新检查登记位，所以 unregister 完成后仍可能收到已排队事件。`6cad0→6ccc0→1cb4d0` 负责单独替换 persistent callback，unregister 本身不清该 callback。适配器 `2dc20` 另检查全局 initialize gate，再发送 event type=2；`1fb30` 在 initialize 通知前开 gate，`1fdd0` 在 shutdown 通知后关 gate，该位不是连接代次。

宿主 [mapping_engine/win/index.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/mapping_engine/win/index.js) 的 setcallback/register/unregister 完整 owner 分别为 `[42542,43950)`、`[43951,45634)`、`[45635,47122)`。第一个 URL 才执行原生 register，成功后先加入 URL，再 await setcallback；setcallback 的 result 没有合并进 register 返回。最后一个 URL 删除后才执行原生 unregister，失败不恢复 URL 列表。持续回调 `[95803,96753)` 再查当前 deviceMap、URL 和 webContents 有效性，因此可以丢弃部分迟到事件，但没有 generation token，不能据此保证同身份重连隔离。一次 completion 由 cbMap 保活到完成后删除，持续 FFI callback 由实例字段保活。

MW 模块 38737 `[371313,393810)` 的注册函数 O 未等待注册结果，并添加匿名 Electron listener；该 listener 只按 productId 过滤。消费者 C `[374348,377625)` 解析 input/output JSON，失败日志返回；把 key93 解释为 action down，其余 key 为 up，并保留 tick，分别派发 `ON_SWITCH_TO_SPECIFIC_QUICK_EFFECT`、`ON_HANDLE_OFF_LIGHTING`、`ON_SWITCH_OUTPUT_DEVICE`、`ON_SWITCH_INPUT_DEVICE`。unregister 包装器 A 不包含该匿名 listener 的 off；不能声称 native unregister 同时移除了 MW 订阅。

引擎正常退出还经过 `56b40` 的 CleanupDevices、shortcut 清理、ShutdownMappingExecutors、CleanupLocalStorage 异步序列。设备析构 `656ca` 先重设基类 vptr，投递 `65c90` 的 virtual+72，再 `1b8670→1b88e0` StopSoon，由 `1a9710` WaitForSingleObject(INFINITE)/CloseHandle 等设备线程退出后释放 callback 等资源。原 PE 中该基类 virtual+72 是 `77f00→nullsub_1`，不能称为某项硬件恢复。manager `56128→5f8b4→5f8e6→5f917` 逐个析构设备。设备线程 join 与返回线程已排队 callback 的清空是不同边界；Rust owner 必须分别持有输入 I/O、设备任务、返回事件、订阅和连接代次，不能用一次 unregister completion 冒充完整释放。

USB/BLE 的 cleanup `53090` 先 signal 取消事件 `11a922`，再依序执行 begin、`78810→789a0` 的事件 callback 清理、本地 storage 清理、条件式 device-mode RemoveDevice、keyboard executor、global shortcut、driver thread、device data、未释放计数检查，最后 `54b00→54930` 完成状态。这里清理的是 input base+208，不能说成已清 unsupported 的 base+736。driver shutdown `ad240` 先经 `7961a→797a8` 释放仍活动的映射，再以 `79a2c` 为 report-kind 0/1/2 输入空报告产生释放，随后停止 driver owner。

driver 的实际停止还有独立的 I/O 排空边界。`1174e0` 先通过 `111150` 对 redirect 类型 21/27/28 发 `0x8888301c`（u32 mask=1/2/4 加 u8 false），再 `110830/110e20` 以 u32 false 关闭 hook/notify（`0x88883034/0x88883038`）；`1179f2→111b40` 逐个发 32 字节 ClearInputHook（`0x8888302c`）。随后经实际 DriverImpl 虚表 `0x2f0990+16` 调 `110700`：置 closing、保留 cleanup callback、CancelIo 并关闭 handle。`110d34` 对实际 DeviceIoControl 维护 pending 计数，`114850→110758` 仅在 handle 已关闭、closing 且 pending=0 时投递最终 completion，`10e868` 另 join driver 线程。多数逐项关闭操作使用空 callback，整体 completion 不证明每个 IOCTL 成功。HardwareEventThread 则使用另一条机制：`12430e→124470` 对各 path reader 调实际虚表 `0x2f2170+24→1235c0`，CancelIo 并关闭读取 handle，最后 `12394a` StopSoon/join。不能把这两种取消都缩写为 CloseHandle 后立即完成。

### 音频端点观察、RESTORE 和真实 setter

MW 初始化 `[1390349,1392577)` 先枚举端点，再注册 simpleService 的设备状态变化与 mappingEngine 的音频服务事件，选择枚举结果中的**第一个** output/input 注册 speaker/microphone 事件，随后取得默认扬声器/麦克风快照，最后建立默认端点变化和 `audioServiceEvent` 消费者。第一个枚举端点不等于默认端点。

枚举 `_t` `[2328304,2329187)` 解析实际 deviceList，按 speaker/microphone 分类，并排除名称包含当前产品 deviceName 的端点；每次枚举都把 currentSpeaker、previousSpeaker、currentMicrophone、previousMicrophone 重置成 id/name 为空、volume=0、muted=false 的对象。这些对象为 truthy，因此 `mt/ht` `[2329191,2330853)` 的 `!current...` 分支不会将初始 getter 快照补入 previous；实际默认变化事件按 deviceID→id 规范化后，才轮换 current/previous。不能把空对象视为已观测的默认端点。列表变化 `$n` `[1445917,1446165)` 再次枚举并重置这些对象，给新的首个端点注册事件，但没有在该路径注销旧端点，也没有更新初始化捕获的 getter 快照。

取放 runner `vi` `[1688745,1690281)` 在 storage-held gate 内找当前音频规则并取两条 lane 的 `data[0]`。放下遇 RESTORE 时使用 runtime 中的 previous 端点，否则用所选端点；拾起直接用所选 id。runtime current/previous、version 和 UI 在真实调用前更新；UI 中仍可保留 RESTORE 配置，传给服务的是解析后的 previous.id。空 previous、失效 endpoint、回调 false 及系统默认变化必须分别建模。runner await `simpleSetDefaultAudioDevice`，但不检查 `{result,reason}`，没有独立读回或回滚，异常只记录日志。局部 p 标志和 1 秒等待位于单次调用内，up/down 条件互斥，不能将其描述成已实现跨事件 debounce、取消或设备断开恢复。

宿主 [simple_service/win/index.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/simple_service/win/index.js) 的注册/注销 `[34536,37435)` 以 mutex/URL 引用计数控制首次 register、最后 unregister。getter owner 为 `[37436,39293)`，setter `[39294,40199)` 检查服务已初始化和非空 ID，通过 cbMap 保活 completion 并在完成后删除，返回 `{result,reason}`；没有该调用自身的 timeout/cancel。宿主声明的 setter 返回 bool 与原生导出修饰名的 void 不一致，对照测试必须使用已核实的实际 ABI。

当前 simple_service.dll 的[完整原生捕获](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/capture.json)给出实际 Windows 路径：export `17050→4f80→5130→323dc→32580` 投递 AudioServiceThread，经已核实的 `0x1f54d0+0x38` 虚表进入 `3a3b0`。它将端点 ID 转 UTF-16，以 CLSID `870af99c-171d-4f9e-af0d-e63df40c2bc9` 创建策略 COM 对象，依次尝试 IID `f8679f50-850a-41cf-9c72-430f290290c8`、`ca286fc3-91fd-42c3-8e9b-caafa66242e3`、再次尝试 f8679f50 接口、`568b9108-44bf-40b4-9006-86afe5b5a620` 及 IUnknown，经虚表 +104 设置 **role=1**，检查 HRESULT、失败返回十六进制错误并释放 COM 对象。没有在此函数循环设置全部 role、独立读回或自动恢复。

扬声器 getter `19310→6460→6690→33c50→33e80→3ac20→3eec8→3e9e4` 使用 flow=0、**role=0**；麦克风 getter `175c0→51f0→5420→326b0→328e0→3a740→3ed86→3e9e4` 使用 flow=1、**role=0**。`3e9e4` 取得实际默认 endpoint 并核对 active bit；具体 getter 再查 endpoint map 的 id/name/muted/volume，缺失返回 `Error : Cannot find audio device.`。因此这两个 getter 不能单独证明 setter 的 role=1 默认端点已生效。completion `16e80` 还检查服务 initialize 位，shutdown 后可能不再转发；不能以未返回结果推断设置被取消。

枚举 `16d40→4cb0→4ea0→32082→32270→3a170` 返回实际 type/id/containerId/name。设备状态事件 `166a0→4ab0` 与 `169b0→4bb0` 设置/清除服务线程登记位，重复操作分别返回错误。原生退出 `31e66→31ff0` 清音频 owner，`31a80` 停线程并释放引用，`38fde` 清 endpoint map 与观察者。MW 退出 `Tn` `[1402915,1404990)` 则是另一层：条件式 generic 音频注销、USB 移除、未等待的 terminate 和关窗口；当前完整 MW 未找到 augustT2 的 `simpleUnregisterAudioDeviceStateChangedEvent` 消费者，亦未找到这三个额外 listener 的配对 off。必须分别补齐 Rust 退出所有者并验证，不能从宿主包装器或 DLL 析构存在推断 MW 已调用。

### 灯效、宏、快捷动作和关联更新

关闭灯效 `pi` `[1683522,1684278)` 查当前启用 chroma，调用亮度关闭；所选 lane 的 isOff 且 applyToAll 时再发布全局亮度关闭。快捷灯效 `fi` `[1684278,1688745)` 先开亮度、发布全局亮度开启，再转换对应 lane 效果；Wave UpDown 的 direction=3 转 index=0，否则 index=1，方向数为 2。applyToOthers 发布同步存储，并通过 `h.td=lt` `[2259111,2260482)` 对当前其它实际容器排 `ON_SET_EFFECTS/ON_SET_MULTI_REGION_EFFECTS`，排除自身。高级 lightpac 由原生相应输出链处理，不走 fi。fi 的 catch 为空，局部 3 秒延迟也不能证明通用取放去抖或取消完成。

完整灯效协议和软件帧生命周期继续以[灯光与冷却](lighting-and-cooling.md)及对应共享灯效链为准；下述亮度链不能扩展证明所有效果、软件帧或高级 lightpac 的下游 runner 已完成。

宏规则实际编译宏引用，宏输入/输出及 map ACK 边界见[宏与按键映射](../shared/macros.md)。全局快捷方式更新 `yi` `[1690281,1691365)` 刷新关联引用并触发映射重建；删除 `gi` `[1691365,1692341)` 在首个 guid 命中时清对应取放 lane 并重建。游戏运行变化 `mi` `[1692341,1693347)` 暂存并移除拾起启动动作，游戏停止再恢复、保存和重建；pause 配置 `di` `[1682973,1683522)` 控制放下 lane 的 `isMappingAgain`，决定 Escape 映射。编辑器持久化、引用更新、游戏观察、原生映射和具体执行器须共同闭合，不能只迁六个静态规则名称。

### 亮度调度、真实 HID 与结果消费者

亮度 maker `rt` `[1523350,1526324)` 先更新 profile/UI，按标志持久化；仅在 deviceAccess 允许时排 `NORMAL_SKIPPABLE` 的 `setting` 任务，把旧／新有效亮度、version、region=null、OBM profiles 和 container 传给 `d.Vb7=xe`，关闭状态的有效亮度为 0。OBM helper `Ye` `[1514076,1514238)` 只从 active GUID 对应的 OBM profiles 提取 slotId，否则为空数组，不能据此推断 3946 一定有板载配置档。

runner `xe` `[1720133,1722817)` 只在入口检查 AbortSignal。3946 走普通 rzDevice25，而非包内 PhilipsHue、IoT、monitor、eGPU 分支。明确 region 时先 GET、值不同才 SET；默认路径取 feature.brightness.region 或 AllRegion=0，先 GET，返回 region 不同则 SET 请求的 region，否则在亮度不同且非 setByLightingEngine 时 SET（另有 NoaT3 分支）。这条链**没有 SET 后独立 GET，也没有额外比较 SET 的 jsonData**。主亮度成功后 `Ct` `[1765879,1766392)` 逐一 GET／必要时 SET 传入的 OBM slots，任一失败整体 false；随后更新 currentbrightness/version/memoryStorage，缓存失败也可使最终 false。硬件已改后 OBM 或缓存失败没有自动回滚，不能把整体 false 等同于硬件毫无变化。

rzDevice25 的 getBrightness `[899848,900121)` 解析 region；region=0 时先取 region list 并取非空首项，只有该读取异常时选择 region=1，这不是回退到本地亮度配置。setBrightness `[900121,900244)` 原样转发所传 region。协议模块 13953 `[702819,713603)` 与常量模块 78196 `[713604,716322)` 按 `[dataSize,class,command]` 定义 region list `[80,15,128]`、GET `[3,15,132]`、SET `[3,15,4]`；GET payload 为 profile/region/0，SET 为 profile/region/brightness byte。编码为 `floor(percent/100*255)`，拒绝 undefined／NaN，≥100 钳到 100，**没有下限钳制，负数进入 Uint8 可回绕**；解码为 `ceil(byte/255*100)` 并解析 profile/region，没有额外检查 payload 至少 3 字节。这些是原件边界行为，须独立测试，不能用更宽松的比对消除差异。

rzDevice25 构造 `[808462,812281)` 的当前参数为 claimInterface=2、reportId=0、reportLength=91、useSendCommandEx=false，当前宿主版本允许 HID mutex，锁身份取 actual PID/container。`_calculateChecksum` `[812329,812398)` 对逻辑 90 字节的 byte[2…87] XOR；`_getTransactionId` `[812398,812494)` 在 counter=31 时先清零再返回并递增，实际循环为 **0…30**。`_createDataSend` `[812494,812788)` 写 byte0=status0、byte1=transaction、byte2…4=0、byte5=length、byte6=class、byte7=command、byte8 起 payload、byte88=checksum、byte89=0。

sendCommand `[817066,821285)` 在 Electron 分支以 navigator lock 串行，要求 OUT 实际结果为 91，等待后取 IN，并在 finally release HID mutex。`_getUSBTransferInResult` `[814023,817066)` 检 transaction/class/command；status=0/new、3/failed、4/timeout 允许重发 OUT，1/busy 继续 IN 并允许后续 OUT，2/success 停止，5/unsupported 及未知状态失败且不重发；它**不核对响应 checksum**，falsy 的宿主结果会转成 90 字节零缓冲。非 Electron USB 分支另用 request=9/1、value=768、index=实际 interface，不能混作同一调用契约。

当前宿主 [UsbRzDeviceAction.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/UsbRzDeviceAction.js) 的 send 双 case 标签及完整处理为 `[10659,10687)`、`[10687,13211)`，get case 为 `[13211,14442)`：90 字节 OUT 的前缀取 `reportId ?? claimInterface`，GET 参数却取 `reportId ? reportId : claimInterface`，并默认移除响应首字节。故当前 **reportId=0、interface=2 时，OUT 前缀为 0，GET 参数为 2**，不能归一成两边 0。实际 HID.node 的[完整原生捕获](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/capture.json)中，worker `5280→50e0→4800` 将 kind=0/1 分到 SET `17a10`／GET `17ac0`；SET 调 `HidD_SetFeature`，成功返回长度、失败 -1；GET 则用 **DeviceIoControl `0xB0192` 加 GetOverlappedResult(wait=true)**，返回 BytesReturned+1，没有调用导入表里的 HidD_GetFeature。Promise `15fe0/160e0→161e0→15d40` 另负责结果与请求释放，互斥到期或 MW cancel 不等于已取消 native I/O。共享传输边界见[设备通信](../shared/device-communication.md)。

结果 owner `$e` `[1723903,1724859)` 先发送 `MW_UPDATE_BRIGHTNESS_UI_ACTION_DEVICE`／`MW_UPDATE_BRIGHTNESS_LOCALSTORAGE_ACTION_DEVICE`，再发各自 POST_PROCESS_COMPLETED。MW 字段为 `newVersion`，UI 常量模块 13254 `[351438,370832)` 的实际消费者 `de/ce/Se/ue` 分别位于 `[6770647,6771077)`、`[6771077,6771418)`、`[6771721,6771919)`、`[6771919,6772305)`：按 timertick 找 pending action，保存 devicestatus／移除 action／推入 errorActions，但解构的是 **`version`，不是 `newVersion`**，这条消息链未把新版本传入 device.version。消费者没有核对连接 generation；maker completed、缓存持久化、协议成功和 UI 确认必须分别记录。

任务类 `y` `[5774,6681)` 拥有 AbortController、navigator lock、task/result closure，队列完整 owner `[6686,9145)` 的取消将 current 移入 cancellingTask、abort 并清 current；未开始任务直接以 canceled/false 完成，被后续同类任务替换的 skippable 任务以 skipped/false 完成。已开始的闭包仍在 finally 发布结果，而 xe 仅入口查 signal，GET／SET 进行中取消不会停止 HID，也不会阻止后续结果。当前 6120 中 cancelAllTasks 的业务调用在 resetOBM `[1600078,1601715)`；退出 `Tn` `[1402915,1404990)` 所调 `Xt.cleanUp` 来自模块 64013 `[2209469,2209898)`，只清 do_not_disturb task marker，**不是这条任务队列的 cancel／排空**。不能把关窗口或 marker 清理称为亮度 I/O 已结束。

## 布局、资源、弹层与生命周期

[接收器页面完整捕获](../../../.analysis/results/frontend/receiver-page-layout-audit.json)保留了 164/179/207/241 的完整函数、模块、原始 CSS 规则及其外层媒体查询；CSS 区间为字符，JS 区间为 UTF-8 字节。241 的实际依赖包括 `11.219fb515.chunk.js`、`855.b3260189.chunk.js`、`914.4c13bdac.chunk.js` 与 `main.899712fe.js`，对应 CSS 也分在三个文件。不能只依据一个 main 的关键词认为页面树已闭合。

公共 `.body-wrapper` 为 `padding:10px 20px 20px`、`min-width:600px`；164/241 的实际包装根另外注入 `.body-wrapper,.main-container{min-width:unset}`，179/207 没有这条覆盖。`.body-widgets` 居中、允许换行、最大宽度 1240px，列宽 600px；`@media(max-width:1279px)` 又给列增加左右 30px margin。默认卡片为 `padding:30px 40px`、`margin:10px auto`，179 的 `hyperPolling` 卡片右 padding 改为 30px。布局应按完整 CSS 级联核对，不能在列 margin 外再猜一个统一 gap。

179 的配对说明宽 369px，图标 44px；空、临时名称、加载、真实配对与断开是不同分支，骨架图层、非英文文本 padding 和禁用解绑样式也独立。Windows 弹层内容最小宽 850px、最小高 685px；dongle 图为 77×60，配对设备卡高度 210px，不能用通用确认框替代。资源需沿 edition/layout 和实际页面分支选择；Ripsaw 顶视图同时声明 1x/3x 原件，音量图标、问号提示、slider disabled 与每行 mute 独立。

关闭与取消必须追到真实任务所有者：179 关闭清失败/成功计时器、取消扫描并恢复页面查询；164/241 的 modal subscriptions、活动操作、设备页观察和独立配对工具各自释放。设备/连接切换须拒绝旧 session、operation 与物理身份的应答。当前 Rust 已持有这些页面任务及关闭路径，但尚未通过运行时截图、键盘/焦点和真实取消验收，不能把静态尺寸接近记作完整 UI 验证。

## 现行 Rust 实际接线与未闭合项

以下结论来自当前生产调用方及服务/协议实现，不能用旧审计里的 `rust_projection` 或文件开头的 draft 注释替代。

| 功能 | 实际 Rust 所有者与传输终点 | 当前边界 |
| --- | --- | --- |
| 179 设备页、指示灯 | [receiver.rs](../../../crates/razer-pages/src/features/workspace/source_controls/receiver.rs) → workspace 事件 → [hyperpolling_runtime.rs](../../../crates/razer-shell/src/shell/receiver/hyperpolling_runtime.rs) → 独立 IPC 服务 → [Windows Uma 请求](../../../crates/razer-service/src/runtime/windows/receiver/hyperpolling_receiver.rs) → `razer_device::hyperpolling_receiver` / 系统 HID | 已有序列号、三次 edition、绑定表、指示灯 GET/SET、本地持久化、macro recorder 排他、代次与取消清理；系统路径核对 container/instance、报告长度并持有容器锁。SET 成功不伪造额外 GET，未观测值与 UI 默认值仍须分开；不能归为仅改本地草稿 |
| 179 配对弹层 | [receiver_pairing_view.rs](../../../crates/razer-pages/src/features/workspace/source_controls/receiver_pairing_view.rs)、`receiver_page_state.rs` → [hyperpolling_runtime/pairing.rs](../../../crates/razer-shell/src/shell/receiver/hyperpolling_runtime/pairing.rs) → 真实 receiver pairing / HID | 已有单候选自动绑定、多候选选择、硬件完成事件检查、四秒失败恢复、一秒成功关闭、页面重查及主 owner scan(4)；当前页面对应 Windows 分支，非 Windows 组件的不同重试、空结果与导航语义不能由此认定完成 |
| 164/241 配对及独立工具 | [dock_pairing.rs](../../../crates/razer-pages/src/features/accessories/dock_pairing.rs) → workspace / Shell → [receiver_pairing.rs](../../../crates/razer-shell/src/shell/receiver/receiver_pairing.rs) 及 `standalone.rs` / `reconnect.rs` → [Windows pairing](../../../crates/razer-service/src/runtime/windows/receiver/receiver_pairing.rs) → Rust `Session` / 系统 HID 与硬件事件 | 已有实际扫描、绑定、解绑、绑定表、edition/layout 后续查询、配对缓存、重连和 join/Shutdown 清理；绑定成功核对 event 54/status 2。解绑的原版 UI 响应、后续诊断读回与缓存失败分别保留；不能因增加诊断而改变原版响应顺序，也不能把缓存失败伪装成硬件绑定失败 |
| 164/241 亮度、软件灯效与监控 | `receiver_brightness_page.rs` → `receiver_profile_save.rs` / `receiver_profile_submission.rs`；[receiver_lighting_owner.rs](../../../crates/razer-shell/src/shell/receiver/receiver_lighting_owner.rs) → [Windows retained owner](../../../crates/razer-service/src/runtime/windows/receiver/receiver_lighting.rs) → `receiver_brightness` / `LightingHid` / 系统 HID | 已有持久化与真实传输；软件效果由 [Rust renderer owner](../../../crates/razer-service/src/runtime/lighting_engine.rs) 计算，独立持有帧发送和退出恢复。当前灯效配置限 164/241、edition 0，效果集合和 LED 顺序分产品；不是所有底座的通用后端，也没有以官方业务 DLL 转发代替实现。退出仍须核对模式/效果恢复 ACK、引擎释放和进程终止，实际 DLL 一致性未验证 |
| 207 Customize、配对、电池优化、自动配对 | [hyperflux_207.rs](../../../crates/razer-pages/src/features/accessories/hyperflux_207.rs) 及 `hyperflux_207_pairing.rs` → `HyperfluxRequested` → [Shell 分发](../../../crates/razer-shell/src/shell.rs) | 已有专用页面、源消息、两次挂载查询、延时任务、状态观察入口和关闭行为；Shell 的对应分支当前只记录适配器未实现，没有原生提交/读回，不能认作设备能力完成 |
| 3331 四通道监控 | [ripsaw_settings.rs](../../../crates/razer-pages/src/features/accessories/ripsaw_settings.rs) → `RipsawMonitoringRequested` → 同一 Shell 分发 | 已有四行专用 UI、mute、slider release、`timerTick`、真实观察入口与页面所有者校验；Shell 当前只记录提交/读回未实现，尚未连接本节所述 17 字节协议及本机 mic 通道 |
| 3872/3873 Audio、3922 Sound、3929 Customize、3932 Lighting | `SourceProductWorkspace` → [SourceControls](../../../crates/razer-pages/src/features/workspace/source_controls.rs) 的 `accessory_controls_data` 目录与通用编辑路径 | 这些产品不在 `audio_products_data` 的产品根，也不在 `accessory_system_products_data` 的产品根；实际泛型 edit 只更新 draft/发 Changed，不能借其它音频产品已连接的 simple audio 服务宣称已接线。各自端点发现、读取/观察、修改、映射、灯效、确认及注销仍需沿实际根补齐 |
| 3946 自动动作 | [accessory.rs](../../../crates/razer-pages/src/features/workspace/source_workspace/accessory.rs) → [Automation](../../../crates/razer-pages/src/features/automation.rs) / [editor.rs](../../../crates/razer-pages/src/features/automation/editor.rs) | Customize 已有动作编辑、确认删除、草稿持久化与 modal subscriptions；`AutomationChanged` 当前只捕获 accessory 草稿，没有接入上文 report4 差分、映射编译/发布/执行、端点发现与 RESTORE、快捷键、宏及灯效动作后端。恢复草稿时过滤无效项、截取 lane 首项并重建宏引用，也不证明设备执行。`launch_sound.rs` 仍有联网及固定 unavailable 分支，需按本地范围处理；不能把这些分支列作已完成本机功能 |

164/241 的真实解绑失败恢复、配对后的元数据查询、灯光 charging/battery monitor、默认恢复和共享本机服务回执均属于必须保留的原版链，不能为简化测试删掉。[配对路由](../../../crates/razer-shell/src/shell/receiver/receiver_pairing/route.rs)在非 Windows 平台显式报告物理接收器拓扑尚未实现；不能把共享纯 Rust 编码器视为该平台已能配对。独立 Rust 路径存在只说明代码职责与调用已落地；全部产品/版本语义与真实 DLL 对照仍是另一项验收。

## 验证边界

12 款导航根的原件长度、SHA-256、完整 AST 边界已与各自当前获取收据核对。接收器布局审计的局部查询未决为 0，但 164/179/207/241 的依赖获取闭包仍分别登记 20/18/21/36 个未决项，且各有一条完整捕获 validate 失败；局部读取成功不将这些失败清零，不表示主机/设备依赖已经全部取得。

3946 UI/MW 的全部保留原件分别为 544/106 份，实际长度和 SHA-256 与各自获取收据一致，但依赖获取仍分别登记 100/98 个未决项；主 UI 和 6120 MW 的完整 AST 均可解析，不据此宣称依赖闭包完成。宿主两个 JS 包装器已与当前 ASAR 提取收据及 ASAR 原始偏移字节交叉核对；上文选用的原生函数 JSON、raw chunks 和关键虚表指针也与当前对应 PE 原字节核对，仍只证明静态来源和控制流。

3946 还须逐项补充以下自动化行为测试及对应官方实际 DLL 对照，表中是验证要求，不表示已有测试通过：

| 功能边界 | 必须区分的输入、状态和结果 |
| --- | --- |
| 规则与映射 | 未命名/禁用/lane 空项、六类规则、保存/删除/toggle、macro/shortcut 引用变化、多规则输出顺序、flag0 disabled 过滤与 flag1、实际映射解析确认和动作执行分别判断 |
| report4 与输入 | 重复报告、press 先于 release、首个 0 及其后字节、短报告、report5 不触发 DKM、修饰键、dualLinked 可选 reportId、不同报告集合、全局 enabled gate、断开时活动映射释放 |
| 回调与取消 | 重复 register/unregister、setcallback 失败、URL 计数变化、已排队事件越过 unregister、同身份重连、关闭 gate、I/O pending 排空、driver IOCTL 失败与最终 completion、两个读取缓冲区和线程 join |
| 音频 | 枚举重置 current/previous、首项与默认端点不同、RESTORE 的空/失效 previous、端点拔出、role0/role1 差异、服务返回 false、未等回调即 shutdown、乐观 UI 与实际写入结果分离、全部事件订阅释放 |
| 普通宏 | Once 覆盖 repeatCount 为 1、up disabled、GUID/hash 更新原子性、重复启动与空 events、递归上限、逐事件 bool 是否进入 valid、排队／驱动接受／实际完成分离、最终 0/1/2、取消中的 delay、release 失败及 pending 排空、type5 错误消费者 |
| 亮度 | region0 的列表与异常分支、负值 Uint8 回绕、百分比取整、transaction 0…30、OUT=91、OUT 前缀0／GET 参数2、全部响应 status、缺 checksum／短 payload 行为、SET 无独立 GET、OBM／缓存失败后的硬件状态、newVersion/version 差异、skipped／cancel／迟到结果 |
| 灯效与关联动作 | 仅本机/全部/其它容器、Quick/Off/lightpac、亮度真实 runner 和 ACK、游戏运行/停止与 Escape 条件、宏释放、失败/取消后的已核实恢复和资源释放 |

源码获取、逆向语义、Rust 独立实现、UI/后端接线、静态/模拟验证和实际 DLL/设备验收分别记录。本节只进行了静态核对，没有运行应用、厂商 JS、安装器、DLL 或设备操作，也没有将现存单元测试标成这次已执行。真实配对、音频、灯效和清理对照仍为未验证。

自动化测试必须覆盖本页列出的每个产品、平台和传输分支，特别是 Windows/非 Windows 的重试与空结果差异、序列号/edition 失败、真实硬件事件与命令 ACK 分离、未知指示灯值、伴随 mute 通道保留、端点身份变化、过期应答、失败计时器、取消 scan(4)、重连、灯效首帧、退出恢复和句柄释放。无设备时只对有依据的纯逻辑、模拟传输与应答回放作相应范围的判断；实际官方 DLL 功能结果与 Rust 的输入、输出、状态变化、应答、错误及清理须另行逐项比对，不能以两边都未连接或只验证加载作通过依据，详见[验证规则](../../../.agents/rules/verification.md)。

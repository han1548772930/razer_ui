# 附件的灯光、显示与热管理

产品选择以 [56 款适用表](README.md#产品与功能入口)和实际挂载页面为入口。`Lighting`、`Customize`、`Performance` 是界面标签，不能用来推断底层协议、设备供电或传感器是否已观测。

## 通用灯光与实际状态

3072 使用 legacy 协议且亮度 region 为 5；3073/3074/3076/3077/3078/3080、3848/3853/3859/3863/3866/3869/3878/3879/3883/3894 使用各自当前 `rzDevice25` 与页面配置。亮度、quick effect、关灯条件、颜色、速度与方向由当前 profile/region 决定，不能把整个产品族的字段并成共同能力。3849 的 lighting-kit 端口、778/3871 的多端口和 3886 的多灯光，是三个独立选择条件。

配置编辑经 UI action、状态机 maker、存储与设备访问开关进入真实设备命令。保存 profile 只确认本地配置；已观测亮度、设备电源和用户编辑值要分别保留。原版部分 maker 先发布编辑状态再执行任务，失败只记录错误，没有统一回滚或二次弹窗；还原时不得凭一处日志写着 SUCCESS 就声称真实设备应答已核对。

3871 的 [Glitter 适配器](../../../.analysis/inputs/frontend/accessories-mw-3871-current/raw/apps.razer.com/synapse/products/3871/mw/rzDevice25Glitter.ef637706c64b096d73f6.js)先连接 Chroma frame 接口，再连接 protocol25 接口。帧发送使用独立接口与互斥，按行组织 region、首尾列和 RGB；末行标志不同，批处理时先排队再 flush。当前包装器有“调用未抛出即标成功”、逐行异常后继续以及按 status 真值判断的路径，均不等于已校验灯具应答。无线适配器另有 keepAlive/exitDimMode；3940 才在 bootstrap 明确注册 keep-alive 与 `devicePowerMode`。

外部供电、设备 power mode 与亮度为独立状态。Glitter `getPowerOnStatusCommon` 捕获供电读取失败后仍封装一个成功结果并给 false，是当前原版的失败遮蔽；本项目不能把它作为已观测“断电”。硬件事件只有满足该解析器的报文长度、recordId 与 eventId 才能改变电源观测，普通灯光写入不能制造连接或供电成功。

### 3940 Soma Chroma 的无线 Glitter 分支

3940 不能只按普通 `rzDevice25` 亮度页处理。当前 MW 入口 [main.d6b7ec7497c64ac394a8.js](../../../.analysis/inputs/frontend/accessories-mw-3940-current/raw/apps.razer.com/synapse/products/3940/mw/main.d6b7ec7497c64ac394a8.js)（163273 B，SHA-256 `f1cec7313ef45fe06825f33560d4303a991655a5350b39d309cdd000c2c69cd3`，bootstrap `[162674,163273)`）注册 `rzDevice25GlitterWireless`、`PROTOCOL25_GLITTER` 硬件事件解析、`usesExternalPowerSupply=yes`、`handleKeepAliveEvents=yes` 和 `genericFeature.devicePowerMode`。MW 运行时身份是 PID 3940、VID 5426、`claimInterface=0`、`dongleId=3940`，只声明 edition 0；UI manifest 仍列出 0/128，不能把 UI 的版本声明当作 MW 已支持的两个分支。UI 的实际页面数组是 `TAB_LIGHTING`、`TAB_POWER`、`HELP`，页面参数和重置入口见 [3940 产品页](products/3940.md)。

适配器原件 [rzDevice25GlitterWireless.d22d6239d5c6390e35ec.js](../../../.analysis/inputs/frontend/accessories-mw-3940-current/raw/apps.razer.com/synapse/products/3940/mw/rzDevice25GlitterWireless.d22d6239d5c6390e35ec.js)（11761 B，SHA-256 `705d27242073f2d9975cb5b3abde87bcd1721af71a02d3259c21a417ff0b78be`）的连接顺序固定为：先以 `claimInterface=1` 打开 Chroma-frame 接口，成功后再打开 protocol-25 接口；两个连接函数各自最多循环三次，protocol-25 的查询报告前缀在 Windows 取 `claimInterface`，非 Windows 取 `reportId`。因此“第二接口打开成功”不能代替第一接口已经成功，也不能把一次 HID 打开异常折算为设备离线。

帧写入使用 `reportMaxDataSize=380`、`chromaFrameInterface=1`。每一行分配 381 字节：`[0]=regionId`，`[1]` 为普通行 `4`、最后一行 `132`，`[2]=[3]=row`，`[4]=start`，`[5]=end`，颜色从 `[6]` 开始；超过报告剩余空间的颜色被截断。`setChromaFrameLightingEngine` 首次用 `eventData.numRow` 固定 `deviceMaxPortNumber`，逐行切出 `3*numCol` 的 RGB；自定义帧通过 `BatchLighting.sendRaw` 排队并在行间加入批处理 sleep，非批处理每行等待 5 ms，最后 `flush`。批处理分支直接返回本地 SUCCESS，普通分支只在 `hid.sendFeatureReport` 未抛异常时置 SUCCESS，两个结果都没有帧读回或灯珠确认；某一行异常会记录并继续，不能把整体 task 状态当成所有行已写入。

电源读取和无线恢复是另一条链。`getPowerOnStatusCommon`（原件 `[1139,2607)`）先调用 `getExternalPowerSupplyStatus`，仅 `status=SUCCESS` 且 `jsonData.status===1` 才把 `isPowerOn` 置真；异常或失败被记录后仍返回 `STATUS_SUCCESS`、`data=[1]`、`packetSizeReturned=1` 和 `jsonData={isPowerOn:false}`。当结果为真时，它还尝试读取无线连接状态并强制 `setActiveWirelessConnectionState("2.4G",1)`，这两步的失败只记日志，不改变外层成功结果。`getPowerOnStatusFromHardwareEvent`（`[2608,4867)`）只接受通知记录中的 `WIRELESS_CHANGE_EVENT`（state=3）或 `DEVICE_EXTERNAL_POWER_SOURCE_CHANGE_EVENT`（value=1）；真值时再读活动无线类型，不是 `2.4G` 就尝试切回，非目标事件返回 `undefined`。

keep-alive/退出暗亮分别转发到基类 `keepDeviceAwake`/`exitDimStatus`；`isPowerEvent` 只认上述两个事件。当前事件解析器 [rzHardwareEventsProtocol25GlitterParser.05e88da474662f10607c.js](../../../.analysis/inputs/frontend/accessories-mw-3940-current/raw/apps.razer.com/synapse/products/3940/mw/rzHardwareEventsProtocol25GlitterParser.05e88da474662f10607c.js)（5117 B，SHA-256 `a78cded3ccc81131a4dcc5c2b3f411746cc25237b521576bbf0ae96021df0bd3`）遇到 17 字节且首字节为 0 时先去掉首字节；通知事件再按 byte[1] 选择解析器。外部供电事件使用 byte[2]，无线事件使用 state=byte[2]/type=byte[3]，活动连接变化使用 type/connected=byte[2]/byte[3]，电源安全事件把 byte[4] 拆成端口位和 global 位；未知事件只告警。不能把普通亮度事件、未知事件或解析成功当作电源读回。

通用 `devicePowerMode` 的实际生命周期由特性模块执行：唤醒在 5 秒窗口内每 150 ms 重试 `NORMAL_MODE`，成功后再以同样窗口重试 `DRIVER_MODE`，两步都成功才发布 `active` 并排队本地 quick effect；待机重试 `STANDBY_MODE`，成功后发布 `standby` 并抑制 4 秒外部电源事件。外部电源打开时若没有近期 keep-alive 唤醒则走完整唤醒，否则只更新状态；关闭只发布 `off`。这些事件的 `execute` 以 mode setter 返回的 `STATUS_SUCCESS` 结算，没有额外 GET/设备状态确认。清理路径先停止系统监测、注销 keep-alive、移除 USB 和 GenericFeature，再调用 `terminate()`/`handleBeforeUnload()`；后两者未 await，不能写成 native 线程已 join。

本节核对的是当前 UI/MW 原件与静态调用链，未执行厂商 JS、业务 DLL、HID、设备写入或电源事件回放；帧 ACK、外部供电真实状态、无线切换、mode 重试和退出线程释放仍为未验证。Rust 必须分别建模帧传输结果、电源实测值、待提交配置、事件驱动状态及失败/取消/恢复，不能复用通用 3871 的帧大小或把外层 SUCCESS 当作设备确认。

电源页的“省电”是独立于 `devicePowerMode` 的设置。UI 产品模块给 3940 的选项是 20、30、40、50、60（分钟），而通用 reducer 初值仍是 enabled=true、value=15；只有收到 MW 的 `MW_SET_POWER_SAVING_TO_UI` 后才替换这个初值，因此 15 不能被当作设备读回。点击开关或选项先更新本地 reducer，并广播/排队 `ON_SET_POWER_SAVING`；maker 可以先保存 profile，再排 `NORMAL_SKIPPABLE` 任务。MW getter 通过 `powerSavingOverrides.getPowerSaving`，没有 override 时走默认实现；runner 在 setter 前后检查 AbortSignal，随后调用 `rzDevice.getTimeToSleep()`，除单 profile 特例外要求返回值等于所选分钟乘 60，不匹配就报错。任务 completed 只在该读回比较通过后发布，不能用 UI 乐观状态、profile 保存或 setter 未抛异常替代设备读回。

## 端口布局、识别和名称

778 的 edition 0/128 与 129 选择不同的固定端口/LED 表；3871 为 6 stripe 多端口，3884 为 3 stripe 多端口，3886 注册的是 3 stripe 多灯光。3849 的 HDK 端口、784 的可弯折灯带 layout 各有自己的编辑结构。固定 stripe 数表示布局声明，不能当成设备已连接多少条灯带的实读结果。

Customize 的端口选择、联动、名称、灯带种类与数量、分段/弯折以及 identify 必须一起追到 `ports`、`portsConfig` 和实际目标 region。已有端口按稳定 id 修改，保留列表顺序；不存在的端口不能凭 UI 下标杜撰身份。identify 是实际临时灯光写入，原状态恢复与取消也属于功能链，不能只点亮后返回成功。

名称控件不是通用文本框语义。当前完整所有者见 [text-input 分析](../../../.analysis/results/frontend/accessories-main-audit.json)：

| 产品 | 提交和取消语义 |
| --- | --- |
| 778、3871、3884 | 最大 32 个 UTF-16 code units；打开聚焦并全选，`defaultValue` 保留输入草稿；Enter 或 document mousedown outside 提交，普通 Blur 不提交；非空且变更才发动作，关闭编辑器；不 trim 空白，Escape 无专门处理 |
| 3886 | 没有同一 maxlength；Enter 连空串/未改变值也发动作；outside 仅非空提交，空串时保持编辑；不能套用上一行 |
| 784 | 最大 32 个 UTF-16 code units；Enter 使输入失焦，Blur 原样转发空串或空白；Escape 把原 prop 通过同一回调发回；`ON_NAME_IOT_DEVICE` 的 name 不自行 trim |

784 灯带真实 LED 数查询会更新 `detectedLedCount`；若当前分段总数超过实读值，原版缩成一个包含全部实读 LED 的 segment。查询失败会抛出；刷新 start 已发而 stop 位于成功尾部，不可据此伪造查询完成。

## 769 桥、娱乐区域和灯具

Hue 实际有 HOME 与灯具页面，不能由共享 Help 模块或曾经遗漏的导航数组判定“只有帮助”。完整桥、灯具、扫描/配对和取消管理器在 [9420 chunk](../../../.analysis/inputs/frontend/accessories-mw-769-current/raw/apps.razer.com/synapse/products/769/mw/9420.063fdd1a0fa811873155.js)。原版初始化链为 ConfigureFFI → GetDLLVersion → SetNodeFFIEvent → Create；其 `isInit` 在 ConfigureFFI 成功时已置真，后续 Create 返回不自动成为设备发现成功。

扫描调用 DiscoverBridges，按 IP 查找调用 FindBridge，各自建立 12 秒 timeout。找到桥、未找到桥、要求按实体配对键、连接完成和断开由不同事件驱动；要求按键时另建 30 秒 pairing timeout。取消扫描更新 UI 并发 Abort，取消配对也发 Abort。找到事件停止扫描计时，连接事件清配对计时；桥调用返回不是这些事件的替代。`destroy` 只调用原生 Destroy；该 JS 方法本身没有清除全部 debounce/timeout 或移除所有回调，因此不能宣称退出资源链已全部闭合。

连接后先把本地 smart-lighting controller 路径提交给 Chroma SDK，再取 EntertainmentGroups 和当前 selected group。若设备没有选区而真实列表非空，原版明确选择列表第一项；这个选择仅适用于此处源码，不能泛化为所有配置 restore 的“首项回退”。切换 group 先显示 loading、选择真实 group，再清灯列表，等待 group/info 事件重新取灯。灯具身份由 ledID 与 physicalModel 组合，灯事件分 new/update/remove；桥断开会移除灯实例并清列表，忙碌 streaming 会禁用控制。

灯具亮度 slider 0…100、step 1，启用条件为灯具 isOn、bridgeEnabled 且非 loading；缺少值时显示的 50 是呈现值。MW SetBrightness 将百分比向下取整为 0…255；regionId 非零时仅发该灯，否则依次发当前开启灯。灯具首次加入 `initHueLighting` 会按本地 profile 的 global 或 per-light brightness 执行真实写入，非全局缺少值时源码使用 50；这属于源码已存在的初始化写入，不能写成只读发现，更不能把本地 50 冒充实测亮度。

亮度编辑只修改选中 profile 已有 brightness，端口持久化只在其已有 ports 时执行；quick effect 的明确动作才按该调用链创建缺少的 quickEffects。关闭 Hue 发 TurnOffAllLights 并保存用户 enable 状态；Remove Bridge 依次移除 SDK 路径、UnPairBridge、清 hueInfo 与 UI。桥通信仍有原生内部协议须独立逆向；此处 JS 的缓存 Lights 与 `setChromaFrame` 封装成功不能证明设备读回。

## 780—791 局域网灯具

完整 IoT 协议包装器见 [791 rzDeviceIoT](../../../.analysis/inputs/frontend/accessories-mw-791-current/raw/apps.razer.com/synapse/products/791/mw/rzDeviceIoT.97b7b2bdf90bf616c8a7.js)，子设备所有者见 [6120 chunk](../../../.analysis/inputs/frontend/accessories-mw-791-current/raw/apps.razer.com/synapse/products/791/mw/6120.cc62b6ba444a5b1c77a0.js) 字节 `[1438742, 1462423)`。每台设备按 containerId 持有 runtime/profile 选择、代理、模式、电源、MAC 控制锁和 UI 可见状态。

连接先查询是否已建立本机网络设备连接；否则发 connect 并等待对应 connect response，最长 10 秒。普通 FW25 命令按设备 MAC 互斥，发送返回必须大于 0，再等同名 response；只接收 NET_READ_COMPLETE/CommandSuccessful 为成功。UDP 灯光帧路径跳过响应等待且直接标成功，所以只证明发送路径执行。`_waitForResponse` timeout 没有移除 once listener，重试时迟到响应隔离不能由该包装器保证。

连接后的本机控制顺序为读取本机 MAC、查询 grab-control 或 mode、判断他机锁，再按当前 override 决定是否进入 Driver mode/初始化。keyLights 使用 grab control，gameRoom 的模式内携带 power/LED type/MAC。读取模式成功后才更新真实模式来源；源码存在 absent state→On、默认 mode/brightness 的缓存值，文档不把这些值登记为设备观测。已有本地配置恢复会触发灯光/白光写入，不能用“发现设备”解释为纯读取。

白光面板与 RGB 分开读写。Get/SetChromaBrightness 按 255 比例四舍五入；Get/SetLEDBrightness 使用白光 LED 32；warmth 经 16 位参数编解码。keyLights 开 RGB 且白光高于 15 时先把白光降到 15；关闭白光而 RGB 仍开时会重发 RGB 亮度。791 的 `persistHardwareSettings` 在该子设备 setter 中触发亮度限制检查，而不是一个笼统的“所有设置已写入设备存储”证明。

790/791 查询 Power_Source_State；非高电流时再读 Physical_Brightness_Limit_Setting，分别得到白光/RGB 上限。读取失败只记录错误，构造时上限 100 不能成为已观测供电能力。设备亮度事件更新 runtime 与对应本地 profile，部分链把 RGB 0 转成 profile 的 1 但保留关灯控制；设备实测 0 与本地呈现值要分开。

改名先更新 runtime，再 SetMdnsName，失败抛错而不回滚已发布名称。移除设备尝试释放 grab control、还原 Normal mode、断开本地通信实例；destroy 尝试恢复模式并删运行窗口状态，但其中若干异步调用没有等待。`release` 只解绑该 host 的 IoT callback，不能替代 disconnect。本地发现、直接通信和必要模式恢复各自保留完整生命周期。

## 3893 Hanbo 风扇、水泵和温度

页面原件 [main.a7630c14.js](../../../.analysis/inputs/frontend/accessories-3893-current/raw/apps.razer.com/synapse/products/3893/ui/static/js/main.a7630c14.js) 内风扇/水泵组件分别为 `[5352832, 5354064)`、`[5354140, 5355262)`，曲线所有者 `[5349465, 5352745)`。风扇高级曲线 7 点、水泵 5 点；相邻温度/速度限制保持单调，拖动释放按一位小数提交。当前 effect 的 cleanup 也可能调用 onChange，不能假设卸载只清资源而无提交副作用。

MW [6259.e34ab9ddf1b145a67b62.js](../../../.analysis/inputs/frontend/accessories-mw-3893-current/raw/apps.razer.com/synapse/products/3893/mw/6259.e34ab9ddf1b145a67b62.js) 内真实风扇→水泵读取 `[535245, 535534)`，服务轮询所有者 `[855309, 856161)`：风扇/水泵 5000 ms，系统温度 1000 ms。设备 setup 就启动系统监测，不以内部 Performance/Lighting tab 为所有权边界。产品 tab 观察 debounce 400 ms 发 start/stop，但服务 dispatcher 只消费 start；失焦不保证停止 getter。

设备 edition=0 只禁用 ARGB port 2147483651；Pump=2、Fan=1 的热管理仍激活。runtime/profile 的端口顺序以 Pump、Fan 开头，UI 左右排列不能当作协议发送顺序。风扇按端口名称 `Fans` 选择，其他热端口走水泵方法。预设 Quiet/Normal/Performance 分别为 mode/speed 1/20、2/50、3/80；高级模式用各自曲线。

[Elsa 协议](../../../.analysis/inputs/frontend/accessories-mw-3893-current/raw/apps.razer.com/synapse/products/3893/mw/rzDeviceElsa.5a3710c37a030a35aebe.js) 的水泵读取 `[16744, 17447)` 从响应取 connected、set mode、actual mode、温度、RPM 和 duty；风扇读取 `[19154, 19759)` 另有 channel/detected，字段位置不同。setter 发送 mode/speed/0。风扇 7 点与水泵 5 点分别补尾端 100/100 到协议 9 点，再按真实 profile/长度写入，不能直接发送 UI 数组。

profile ports restore 依次保存、发布/diff 与真实设备任务；缺少端口 diff 时会恢复全部激活热端口。当前 UI port maker 同一原 diff 会重复保存/restore 两次，队列不去重；相等值导致 diff 缺失也可能两次全恢复。模式任务由 deviceAccess 控制，不能添加不存在的“写盘成功才写设备”条件。执行器捕获异常但未逐项检查全部返回 status；日志不代表核对每个应答。

Hanbo 没注册 `usesExternalPowerSupply` 或 `hasFanPowerStatus`，Elsa 也不实现通用供电 getter。热模块 fault bits 不能改作外部电源观测。其 reportLength=65 与 host 前缀组合形成 66 字节参数而写入计数比较 65；静态 host 链会转发并 padding，不能假定 Windows 自动截断；实际接受性仍未运行验证。

## 3900 PWM 风扇控制器

UI [main.1f8b3065.js](../../../.analysis/inputs/frontend/accessories-3900-current/raw/apps.razer.com/synapse/products/3900/ui/static/js/main.1f8b3065.js) 的 FP `[5126600, 5129187)` 显示 8 个固定风扇插座；左右顺序不是 thermalId。至少两个真实 ACTIVE 风扇时显示 link 切换；联动时从当前选中风扇复制模式及当前手动值/高级曲线到其他激活端口。外部电源与实际风扇状态控制禁用/空状态。

MW [5380 chunk](../../../.analysis/inputs/frontend/accessories-mw-3900-current/raw/apps.razer.com/synapse/products/3900/mw/5380.4c0adaba83bc33e5cf4b.js) 的 PWM 服务 `[1885810, 1888923)` 处理 FAN_EVENT/POWER_EVENT。事件把端口状态按 26+index 更新；转速轮询周期 2500 ms，对激活端口用 `thermalId=portId-21` 调 GetThermalFanSpeed(class=1)，实测使用 measuredFanSpeed。未成功不会成为 0 RPM；源码成功分支的 `value||0` 不能扩大到失败回退。

高级曲线跟随真实 CPU 温度。温度变化时筛选启用且 ACTIVE 的高级端口，按温度排序并选择首个阈值大于当前温度的档，数值变化才 SetThermalFanDutyCycle；无匹配对象/未观测温度的原版边界不能凭空补成功档。没有高级端口时停系统监测，POWER_EVENT=0 也停止。当前选中端口失效时显式选第一个真实激活端口并发端口配置事件，这只是本控件的选择规则。

专用 [PWM 设备类](../../../.analysis/inputs/frontend/accessories-mw-3900-current/raw/apps.razer.com/synapse/products/3900/mw/rzDevice25PWMPCFanController.eb5404a913a89aa2074e.js) 的 Set/GetThermalFanSpeed 头为 `[6,13,1]`/`[6,13,129]`；响应区分目标 thermalFanSpeed 与 measuredFanSpeed。ThermalFanInformation `[8,13,131]` 包含 sampling、hysteresis、update、maxDelta，不能把相同命名的 eGPU RPM 控制直接套给它。

## 3907 散热垫与 HyperBoost

3907 使用自己的 fanController：enabled、Fixed/Smart，固定 LOW/MID/HIGH 与 smart CPU/GPU/CPU+GPU、Quiet/Balanced/Performance 曲线。配置 maker 在允许存储时保存并更新 version，然后独立按 deviceAccess 排队；初始化 maker 发布配置及可能的设备任务，不证明是实测模式。

真实执行器见 [6259 chunk](../../../.analysis/inputs/frontend/accessories-mw-3907-current/raw/apps.razer.com/synapse/products/3907/mw/6259.9704435af82c7937e70b.js) `[1815737, 1816637)`、`[1816892, 1817776)`：关闭发 coolingMode=6；固定档换算协议速度，UI 动作与设备动作的标志不同；smart 的设备来源动作另发模式。取消在任务入口检查 AbortSignal，原版 result 接受 undefined 为成功，不能描述成已核实全部设备 ACK。

系统监测回调 `[1860720, 1863504)` 只在 initStatus 正常、所选 CPU/GPU 数据 enabled 且已存在时计算曲线；CPU+GPU 取两条结果最大值。转换为协议速度并比较 lastSetFanSpeed 后写入；缓存提前更新意味着失败后可能跳过同值重试，这不是设备实测缓存。finally 总会发布诊断/传感器字段，不能把其中计算出的 fanSpeed 当实测 RPM。

HyperBoost 由本地共享窗口状态协调。只有 mode=hyperboost、systemReady=true、外部电源=1、风扇电源=1 且本实例 ready 才激活，独立 fan table 采用 CPU/GPU trip 与 hysteresis。退出 HyperBoost 后明确恢复当前启用的固定档，不能擅自恢复默认曲线。状态生成器 `[2281904, 2283079)` 和初始化 `[2280239, 2280807)` 定义该恢复条件。风扇按键通过 `ON_SWITCH_FAN_POWER_BY_DEVICE` 入队，不能直接覆盖用户草稿或把事件计为写入成功。

## 3921 eGPU 温度曲线

3921 的 Customize/Lighting 与 edition 0/128 入口见 [产品文档](products/3921.md)。初始化按 ThermalFanInformation → FanMode → FanSpeed → ThermalReading 真实读取；从 information 的 B/F×100 取得 RPM 范围，保存当前模式、转速和机箱温度，再发布 systemMonitoring。读取失败不能用页面默认曲线标实测。

其 generic `SET_FAN_SETTINGS` 保存 `fiverFanController`，包含 Auto/Manual、activeManualPreset、各预设 chassis/GPU 曲线；模式变化写 SetThermalFanMode。手动 chassis 切换立即使用当前已读机箱温度计算，其后温度消费者持续调速。曲线按相邻点线性插值，GPU/机箱隐含起点分别 30/20°C、1000 RPM，低端至少 1000，最高点以上用末点，重复温度有单独处理；这套算法不同于 PWM 的离散阈值。

完整 feature 和消费者见 [3921 MW main](../../../.analysis/inputs/frontend/accessories-mw-3921-current/raw/apps.razer.com/synapse/products/3921/mw/main.072111f727c767458aea.js)，专用 [FiverV2 设备类](../../../.analysis/inputs/frontend/accessories-mw-3921-current/raw/apps.razer.com/synapse/products/3921/mw/rzDevice25FiverV2.699c898515afbaf0aacb.js)。feature 的 `{status:"completed"}` 仅是自身任务返回，不能替代 setter 返回/设备确认。该类同时打包未挂载的笔记本/显示系统方法，不能因此扩充 eGPU 可见功能。

## 3858 与 3880 显示器

两款挂载 Gaming/Color/Display/Lighting。HDMI、DP 与 USB-C 由 OS 监视器记录和 USB/HID 记录共同归并，控制目标使用 monitorSN；lightingContainerId 属于另一个 HID 实例。3858 旧分支存在把不匹配记录归入第一管理器、临时序列号等源码 fallback；3880 的多监视器分支不同，不能把临时身份作为真实设备序列号。

[9363 chunk](../../../.analysis/inputs/frontend/accessories-mw-3858-current/raw/apps.razer.com/synapse/products/3858/mw/9363.c6e4af200f89fe7ef96b.js) 的 Scarlett 包装器 `[15112, 40437)` 包含亮度/对比度、色彩预设/RGB gain/gamma/gamut、PIP 模式/来源/大小/位置/交换、图像缩放、显示预设、FreeSync/overdrive/运动模糊/HDR/FPS、输入/自动切换、刷新率、系统 HDR、ICC profile 的本机读取与修改。原版 init 检查 Initialize 后才置 isInit，与摄像头提前置真不同。SetMonitorVideoGain 的 JS 参数顺序与实际转发重排需按调用方保留，不能依据日志文字猜 ABI。

Gaming 自定义预设任务先读 DisplayMode，再在需要时改预设；custom=5 时并行读取亮度/对比度/overdrive/gamma 与存在的 gamut，只写差异。当前执行器未逐个消费 Promise 返回 bool；其 result=true 不能证明全部设置成功。OS 的 onDisplayChange 刷新 handles 和系统 HDR，再更新当前显示设备；3880 另消费硬件 stand 连接事件，不能把 USB-C 存在当所有版本灯座已连接。

identify 先保存真实亮度，按 300 ms 交替 40/80，再恢复初值；catch 中断后并无 finally 恢复，取消/失败不能宣称已恢复。DestroyMonitor/DestroyAll/terminate 与事件监听清理需要分别核对，普通 profile 删除仅移除本地 metadata，不意味着 OS/native handles 已释放。此处仅已读 JS 所有者，原生显示器内部 DDC/OS 协议的全部实现不在本文冒称完成。

## 系统温度服务和退出

Hanbo、PWM、散热垫与 eGPU 共用本地系统监测入口，但由各产品的服务与消费者决定启停。RzSystemCommon 维护隐藏窗口、计时器和命名管道接收，计时请求 GetAllHardwareInfo；服务端返回 JSON 指针由对应释放方法归还，接收端组装 CPU/GPU 并回调。缺少传感器保持 null/未观测，不补 0。

完整选定 native 释放证据保存在 [Hanbo 分析](../../../.analysis/results/frontend/accessories-main-audit.json)：产品 stop 注销 container 并结束 JS 服务，native DeviceTerminate 自身只日志；最后一个 container 注销才杀对应计时器。CommonTerminate 发 WM_DESTROY，关闭窗口线程 handle 而不 join 该线程，join 的是 pipe receiver，然后释放 buffer；WM_DESTROY 杀计时器并退出消息循环。DLL 服务卸载先 Terminate 再 FreeLibrary；CloseAPPServer 是整个 server 的动作，不能拿作单产品停止。本文不运行这些链，也不将原版释放缺口写成已解决。

### RzSystemCommon 1.0.29.0 的订阅、缓存与管道边界

依据 [RzSystemCommon 完整捕获](../../../.analysis/results/native/cc8c6aee3515f96f5bed7a11c4a4b3653b525ec2a09a87c2d062ec183af4a76d/capture.json)，`RegisterFFIEvent`（`0x6d900`）拒绝空事件名和 interval 0，但没有拒绝负 interval；按事件名去重，重复注册返回失败而不替换。每项保存 dataType、interval 与 tick。记录先进入列表并启动计时器，随后才初始化管道，后续失败没有回滚已加入订阅或计时器。

管道初始化 `0x61610` 只校验名称、保存名称、设置初始化标志并尝试连接，其返回 1 不证明连接成功。同步计时函数 `0x6c9e0` 根据连接与重连结果构造 `dllName=RzSystemService`、`dllFunc=GetAllHardwareInfo`、`handleOnOneThread=1`，可选附带 dllPath、relateDllPath、touchpadToolPath，再序列化交给 `0x619c0`。断连且回调存在时发送 `RzDLLServiceNeedCheckAndRestart` 事件；必须追实际管道应答更新缓存，不能把请求排队或 Init 成功当作已取得传感器值。

实际连接 `0x616e0` 使用同步 `CreateFileA` 打开读写命名管道，再设置 mode 3；状态探针 `0x60e10` 只判断 handle 非 0 且非 INVALID_HANDLE_VALUE，不询问对端存活。发送 `0x619c0` 在短写时重新发送同一完整 buffer，而非推进剩余偏移；只有 BOOL 成功、写入 0 且 LastError 0 的分支受十次／100 ms 限制，ERROR_PIPE_NOT_CONNECTED 的等待和短写重发没有同样的统一次数上限。ERROR_BROKEN_PIPE／ERROR_NO_DATA 关闭并清空 handle 后失败，其他错误保持各自返回；不能将该循环改述为所有失败统一重试十次，亦不能推定写完请求即收到硬件应答。

接收线程 `0x6cf00` 调用 `0x61de0`，只在单次读取恰好得到五字节独立消息 `<EOF>` 时发布完整标志，移除该 marker 后交给 `0x6bc20` 更新缓存。它不是任意字节流内搜索 EOF 的协议。读取 helper 每次先清空 buffer，内部至多执行 500 次读取，容量不足时倍增并复制已有字节；ERROR_MORE_DATA／ERROR_NO_DATA 与其他错误分别处理，其他错误关闭 handle。接收线程忽略 helper 的 BOOL，只检查完整标志，未完整时保留自己的 offset 并等待 100 ms；跨调用清 buffer 与保留 offset 的组合必须作为截断／部分消息测试输入，不能假设所有分片都会自动拼接正确。

`0x6bc20` 在解析 JSON **之前**设置已收数据标志 `0xC56D9` 并清除请求时间戳，再替换整份硬件缓存、分别更新 CPU／GPU／其他字段。解析异常返回失败，但未见恢复该标志或整组旧缓存的事务回滚。因此该标志只能证明尝试处理过数据，不能作为有效传感器结果的充分条件。CPUInfo 存在时选择首项；GPUInfo 多项时优先第一个 enable 的 NVIDIA，否则后遇到的 enable AMD 可覆盖既选项，没有前者时保留首个 enable Intel，仍无候选则取索引 0。缺项写入的 null、支持状态 0 和 notfound 是本地解析分支，不是硬件实际返回的测量值，Rust 必须保留其来源与未观测语义。

回调计时函数 `0x67df0` 按订阅间隔取缓存字段，组装事件名、`dllName=RzSystemCommon`、`dllFunc=HardwareMonitorCallback` 后同步调用用户回调；随后更新 lastTick，临时 JSON 字符串随后销毁。`GetHardwareDataByType`（`0x6e120`）同样读取缓存，dataType 0 选择常规字段而不自动包括高位 Info 字段；数值判断是 `<= 0x10000000`，不是 known-bit 集合校验。缺少缓存字段不能补造设备状态，静态 JSON 的 SUCCESS 不证明当前服务或设备读取成功。

`UnRegisterFFIEvent`（`0x6dd50`）对不存在的非空名字也返回成功，只有列表为空才关闭两个计时器，删除最短间隔项后未重算最小值。`SetFFIEventTimerIntervalTime`（`0x6d380`）先保存新值，末尾用同步计时器的 interval 0 判断是否关闭 FFI 计时器；没有核对 SetTimer／KillTimer 的 BOOL。`Terminate`（`0x67260`）关闭计时器和窗口、关闭窗口线程 handle、清对象和管道；这不证明窗口线程已 join 或用户回调已静止。相关正常、负间隔、断连、重入、取消与释放结果，以及官方实际 DLL 对照，均需独立验证。

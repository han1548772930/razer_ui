# 鼠标：灯光、休眠与电池设置

Lighting 调整亮度、快速效果和关灯条件；Power 调整空闲休眠、低电量模式及部分产品的电池类型。它们可共用同一个配置档，但不共用一个设备命令或完成判定。

## 产品与连接条件

每个产品的快速效果顺序来自当前 `QUICK_EFFECTS`，效果可编辑项来自对应 effect component；源码包含某种效果，不表示全部鼠标都挂载。没有 Lighting 导航的产品也可能具有 DPI／状态指示灯，不能把状态灯等同完整快速效果面板。

| 条件或产品 | 当前源码的实际区别 |
| --- | --- |
| 常见 PowerSaving 滑块 | 1～15、步长 1；一般路径按分钟转为设备秒数 |
| PID199／208 | Performance 内挂载六个休眠按钮，实际值 15、30、60、120、300、600 秒；单配置设备分支不再乘 60 |
| LowPowerMode 控件 | 5%～100%、步长 5%；普通／无线当前轮询率超过 1000 Hz 时不可编辑，并显示原因 |
| PID117／2594 | 额外 dimLighting 开关及 1～15 分钟滑块；该项与 sleep 分开 |
| PID184／224 | 可选择碱性、锂、可充电 NiMH 电池类型；属于设备电量换算／策略输入，并非充电动作 |
| BLE | 使用产品 BLE 配置及 Protocol30 分支，部分型号移除校准／Power 导航或禁止轮询率编辑；不能复用有线条件判断 |
| PID104／105 | 鼠标＋供电垫复合页面，另有 mouse/mat powerIndicator 颜色和独立开关 |

PID199／208 的界面 initial powerSavingValue 为 5，而按钮集合没有 5；源码并不因 reducer 有初值就选中 5 秒或 5 分钟按钮。这里必须同时阅读控件和所选产品的值表。

## 亮度、关灯和效果

常见亮度滑块为 0～100，开关决定是否输出亮度。关灯条件另存 `switchOffLighting.isDisplayOn`、`isIdleEnabled`、`idleMinutes`，常见鼠标 idle 滑块 1～15 分钟。设置亮度与修改空闲时长是两条动作，不可只改变图片透明度而称设备亮度已变化。

快速效果保存 `quickEffects.selectedEffectId` 及该效果自身颜色、速度、方向等设置。呼吸、静态、波浪、响应等控件各自决定输入数量与范围。高级效果选择关联本地 Chroma 配置和库状态；映射回调中的 Chroma 选择不证明 RGB 帧已经传给设备。

PID158 中间件亮度 writer 按 rzDeviceType／region／lighting engine 分流：可能先 `getBrightness`，同值跳过，按 region 写入；另有 feature 指定直接写的路径。源码返回的 region 与当前目标共同决定后续写入，所以不能用一套“总是读一次、总是写一次”的流程描述所有型号。

## Quick Effect 的实际帧消费者

页面保存 quickEffects 后，由独立 lighting engine 的 storage change 消费者按在线产品、localStoragePid、container 和 metadata activeProfileGuid 选取配置；多 region／ports 各有分支。该消费者比较效果与参数，并按设备的软件／硬件能力及节电状态分流。原版 metadata 缺失时使用存储 activeProfile 的行为属于其配置选择链，不能将其描述为真实设备 profile 读回。

硬件效果同步写 profile1 后等待 100 ms；退出同步则登记 quickEffectOnExit 并交给 HID 默认数据链。软件效果分支设置 device mode3、CustomFrame effect8，创建 engine／实际设备／effect、映射 LED 后启用。帧回调按 devicehandle/linkseq 查设备，执行 LED remap、frame filter、lighting lock，再调用批量 controlTransferOuts 或逐行 setChromaFrame。部分逐行异常只记录并继续，函数末尾仍返回 SUCCESS，因此不能把其顶层 SUCCESS 解释成每行都收到确认。

硬件专有效果和电池节能条件还会进入 `applyQuickEffect`：当前共享 owner 以 50 ms 定时器安排 SET，随即返回，调用方 await 不等待该 SET；定时器 callback 的 Promise 没有消费者，owner 也不保存可取消的定时器句柄。其共享 SET wrapper 丢弃设备返回，try 内错误仅记录日志，没有写后读回或回滚。即时同步、登记退出目标、安排定时 SET、实际传输完成必须分别判断；当前源码位置和完整共享收据见[键盘页的同一链](../keyboard/lighting-power.md#保存如何到真实灯光帧)，不由这条共享调用推定每款鼠标均启用该硬件分支。

停止时按官方退出链停止帧、终止／释放引擎、恢复设备默认数据、清空映射并关闭宿主、unInit。尊重 Advanced／Chroma Connect 占用、锁定和系统 WDL 的条件由 engine 消费者处理；关闭页面卡片不会自行等价于停止全局帧输出。完整消费者及原件字节位置见[系统灯光的同一共享链](../system/customize-lighting.md#quickadvanced-到真实灯光帧)。

## PID104／105：鼠标与供电垫

两个 UI 原件的此组组件字节相同，产品身份共用 product104／dongle105 和同一 profile。导航为 Customize → Performance → Lighting → Calibration → Help。鼠标未连接时初始视图为 Lighting，Customize／Performance／Calibration 禁用；isMouse 之后改变时更新代码把除索引2以外的导航全部切为 disabled 或可用，包括 Help，并切到 Lighting 或 Customize。初始 Help 与更新后的门控不同，应保留这个实际差异。

Lighting 左侧为亮度及关灯条件，右侧为效果及 Power Indicator。只有 isMatPlugged 时挂载 Power Indicator；isMouse=false 时给颜色和鼠标指示开关传 disabled class，垫指示开关没有这项额外 class。颜色选择器隐藏“无颜色”，修改颜色保留两个开关；勾选鼠标或垫各自只翻转对应字段，提交完整 `{color,isMatIndicatorOn,isMouseIndicatorOn}`。

打开设备时，若当前 rzDevice.productId=105，middleware 将 isMatPlugged=false 并发布；其他连接读取 `getDeviceWirelessConnectionStatus` 的 connected==1 判断鼠标。该读取 helper 的异常返回 false，随后按垫单独存在的分支更新本地产品文案、isMouse 和 Chroma 设备 SVG；这是原版错误归并，不能把读取失败当作已经确认鼠标断开。硬件无线连接事件另调用相同文案、runtime 和 SVG 更新消费者。

Indicator reducer 先更新编辑态并同时发送状态机事件和 mwBroadcastChannel 通知。maker 按允许的保存分支取得旧配置、保存新的 profile 字段、递增版本，发布 UI 投影和本地 completed，再排 NORMAL_SKIPPABLE 设备任务。restore 从 profile 投影并排 `ON_INIT_POWER_INDICATOR`，它与用户修改进入同一 maker。

writer 比较旧、新三个字段，仅对变化项写入：颜色变化且有值时 `setChromaEffectCommon({profileId:0,regionId:22,effectId:1,flags:0,rate:0,colorArray})`；垫开关变化且非 null 时 `setBrightness({profileId:1,regionId:22,brightness:100或0})`；鼠标开关同理写 region23。入口检查 abort；传输抛错 reject，已有前序颜色／亮度写入没有回滚；没有匹配 GET 或逐个 status 的核对。

该 writer 正常路径设置 result=true，value 为新配置加 checking 标志；结果回调据 result 更新 runtime powerIndicator，但 UI task 的成功判断另外要求 value.status==completed 或 error==skipped。writer 没有填该 status，不能把 result=true 直接写成 UI task 成功，亦不能据此宣称灯光读回确认。两条消费者均按原件保留，未执行设备对照来推定最终画面。

## 休眠、低电量与真实确认

PID158 `mousePowerSavingReducer` 先更新本地编辑值并发送 `ON_SET_POWER_SAVING_VALUE`。task maker 按本地保存／设备访问权限保存配置和版本，再派发序列化任务。实际 task runner 先检查取消，将普通值乘 60（singleProfileDevice 使用原数值），调用 `setTimeToSleep`，再次检查取消，执行 `getTimeToSleep`，精确比较；不一致抛出 `data not match`。setter 的 jsonData 单独作为 fromDevice 保存，确认使用的是后续 getter。

低电量模式的 task maker 则把 `lowPowerMode` 存在本地根字段，推进版本、保存和刷新，随后排入 lowPowerMode 任务；低电量灯光另存 `lowBatteryEffects` 并有独立 action。两者名称相似，实际字段和 task memory 不同。

当前 generic helper 还含另一套 powerSaving `{isEnabled,value}` 路径，会读取 autoSleepMode 和 timeToSleep 后按条件写入；这个 helper 存在并不说明普通鼠标 Power 页都会调用。dimLighting 则读取 batteryId 和 timeToDim，按 `enabled`、分钟和亮度 20 写回。

## PID190：当前休眠消费者与缓存边界

本链通过当前路由的 `current_source_selection` 选择 UI `mouse-190-snapshot-7a9576b5` 与 MW `mouse-mw-190-snapshot-44b54d19ebd53e129c9c`，不将旧 `mouse-190-current` 的完整捕获当成新部署已完成。新部署的选定函数直接从完整 AST 保留，两个部署的完整捕获缺项分别保留。统一入口为 `audit-family-function --family mouse --function mw --query mouse-power-190-consumers`，正式证据归 `.analysis/results/frontend/mouse-mw-audit.json` 的 `mouse_power_190_consumers`。

实际导航挂载 `pl/Pl`，左列给 `Cl` 传空 props；`Cl` 读取 `mousePowerSavingReducer.powerSavingValue` 并连接 `setPowerSaving → Sl → S1_`。因此 `ul` 使用 1～15、步长 1 的分钟滑块，没有启用可选秒值按钮。reducer 初始值 5 与用户修改后的乐观编辑值均独立于设备读取；控件没有 Apply／Cancel，也没有卸载时取消任务的方法。

当前 MW 主入口明确启用 `isBattery.yes.alwaysReadPowerSavingFromDevice=true`。模块 `45601.Ee` 在 READY、存在电池 feature 且当前 `powerStatus` 为真时，无参数等待 `96204.q`，将 `getTimeToSleep().jsonData.timeToSleep / 60` 发布为 `MW_SET_POWER_SAVING_VALUE_TO_UI`，对应 UI `WDf`。读取 helper 捕获异常后返回默认参数 5；这是原版失败分支，不能当成已观测设备值或本项目读取成功。

`powerSavingAction` 的实际 guard 根据来源和宏录制状态给出本地更新／设备访问权限。宏未录制时，UI、runtime、device 来源可更新本地并访问设备，local-storage 来源仅访问设备；录制时，非 local-storage 来源只允许本地更新，local-storage 来源被忽略。dispatcher 把这些真实权限、版本和值传入 `25947.gn`。上述 feature 使 `gn` 跳过配置文档的省电值及版本写入；它先发布本地 completed，再按权限排入 `NORMAL_SKIPPABLE` 的设备任务或发布后处理完成，不能将本地 completed 当设备确认。

UI `Le/Pe` 为动作记录生成本地 `Date.now()`，广播却显式携带 `timerTick:undefined`；实际发送器原样包装这份数据。MW `ye` 收到后重新生成 `Date.now()` 并覆盖事件 payload 的 `timertick`。UI 的本地、设备及完成消费者只按 `timertick` 精确匹配，未命中时保留原状态。两个异步时刻是否实际相同尚未对照，不能补造时间戳一致、自动关联或自动回滚。

`96571.wt` 先检查取消，按普通分钟值乘 60 后 SET，再检查取消、GET 并精确比较秒数；通过后先设置 `value.status=completed/fromDevice`，再处理宿主缓存。当前实际注入的 initializer `a` 未建立 PowerSaving maker／runner 根字段，序列化的 `memoryStorageCache.taskMaker/taskRunner` 也未挂相应键。模块 `36532` 在根字段缺失时建立独立 Proxy，没有把新对象写回 `window`；`gn` 写入 `Al` 的 `taskMakerPowerSavingMemoryStorageCache`，`wt` 写入 `OF` 后又直接访问 `window.taskRunnerPowerSavingValueMemoryStorageCache[setting]` 的值及版本。本链已核的当前声明脚本没有建立这三个根字段的直接赋值；外部或动态生产者仍待取证，不能据此宣称现场必然失败，也不能假定 Proxy 已进入宿主整包。

若后续根字段访问或宿主缓存拒绝，可能已发生真实 SET／GET，并且 completed 已先设置。当前任务 wrapper 在 reject 后仍于 `finally` 交付同一状态，没有重写其 error／status；`kt` 又以 `value.status=completed` 或 `error=skipped` 进入完成分支。因此缓存抛错不能直接改写成 UI 设备错误。其他结果会发布错误及后处理完成；UI 保留乐观编辑数值，按匹配动作记录 local/device 回执与 `errorActions`，没有回滚或把确认 getter 结果重新发布到编辑器的步骤。

宿主存储已按当前 Host 4.0.896 的官方包、ASAR 成员和实际字节绑定。`setMemoryStorageItem` 按 sender URL 维护进程内缓存、返回旧值，并通知符合条件的其他页面；它不表示磁盘持久化或设备确认。Power 卡片及响应 wrapper 没有额外卸载清理；共享 UI 的 `beforeunload` 移除已注册回调，单次发送关闭临时 BroadcastChannel。MW `shouldClose` 进入共享关闭所有者，完整宿主退出、在途任务静默与设备恢复继续由其独立链复核。独立 Rust、UI／后端接线、真实时间戳、DLL／设备结果与窗口验收仍为未验证。

## PID196：实际挂载与当前省电 feature

统一入口 `audit-family-function --family mouse --function mw --query mouse-power-196-consumers` 从既有 task-memory 批次的产品条目 `/queries/mouse-receiver-polling/power_task_memory_batch/products/58` 接续，正式结果归[鼠标 MW 收据](../../../.analysis/results/frontend/mouse-mw-audit.json)的 `mouse_power_196_consumers`。当前路由实际选择 [UI 刷新收据](../../../.analysis/inputs/frontend/mouse-196-current-refresh-ec3db4b29c5a/acquisition.json)与 [MW 第二次刷新收据](../../../.analysis/inputs/frontend/mouse-mw-196-current-refresh-ec3db4b29c5a-2/acquisition.json)；后者合法复用第一次刷新目录的原件。两层当前部署均无完整捕获，本链直接保留所需完整 AST，未将旧捕获提升为当前。

普通显示根 `yh → Pm/Lm` 在 READY 后挂载 `Dm/cm`；导航 id3 选择 `jS=XS`。其左列给 `wS` 传空 props，`wS` 连接 `KS`、reducer 与 `yS → S1_`，因此实际使用 1～15、步长 1 的分钟滑块，没有启用可选秒值按钮或额外 Apply／Cancel。当前产品定义明确为 PID196／dongle197，未声明 `singleProfileDevice`，故任务使用分钟乘 60 的普通分支。

实际 bootstrap 启用 `isBattery.yes.alwaysReadPowerSavingFromDevice=true`，READY 消费者仍要求当前 `powerStatus` 为真才读取休眠秒数并除以 60；原版读取 helper 的异常返回 5 仍是失败回退，不是设备观测。导航初始化还可从本地根 `powerSavingValue` 单独发布 `WDf`。上述 feature 让 maker 跳过该本地字段及版本的写入，本地 completed、设备 SET／GET 精确比较及宿主缓存仍分别判断。

当前 READY、guard、dispatcher、maker／runner、完成／错误／取消及任务 wrapper 等 17 个完整共享所有者与已审 PID190 同字节，直接复用其明确边界。PID196 自己的注入 initializer 同样没有建立 PowerSaving maker／runner 根字段，也没有将对应键接入整包 task cache；外部／动态生产者继续待取证。UI 本地 tick 与 MW 接收 tick 分别生成，回执只按精确 timertick 匹配；消费者保留乐观数值并维护动作记录，没有补造关联、回滚或卸载取消。

10 项定向维护测试覆盖当前路由、实际挂载／feature、共享复用与事件接线，以及来源篡改、缺失所有者和发布前依赖变化的拒绝。它们仅验证静态提取；独立 Rust、UI／后端、真实时间戳、DLL／设备结果和完整宿主释放仍未验证。本批没有新建临时目录、捕获或分析会话。

## 打开、事件、失败与退出

亮度、效果、idle、lowPowerMode 等字段由各产品挂载的 profile restore 和对应 MW_SET 消费者载入；设备电量／充电事件通过 batteryReducer 更新状态，不能用电量事件推断配置已保存。

原件部分读取 helper 捕获异常后返回传入默认值（例如 getPowerSavingFromDevice 默认 5）。这是需要原样理解的失败分支，不能在文档中称它为真实设备读数。常规 task runner 则把错误发布到 action/error 通道，保留已编辑的乐观值；上述完整 sleep writer 会在取消或读回不一致时失败，不执行成功结算。

连接类型、主接收器或配置改变会改变真正的 rzDevice 和 region 选择。设备任务由其队列拥有；效果动画、屏幕取色等持续功能另有自己的启动／停止链，不能把关闭一个卡片当作所有灯光工作已停止。

## 226 输入通知的驱动资格

当前 226 原件已核到 `RzDev_00e2.sys` 的真实产品接口及注册／分发槽位，INF 明确列出 1532:00e2／00e3 的 MI00 鼠标、MI02 键盘及 MI01 USB lower filter。它们接入当前 Common 的共享 304 字节通知链；普通鼠标通知只在原生输入偏移 4 的 DWORD 非零时生产，不能据此宣称纯位移、全部按钮组合或其他产品传输已覆盖。共享生产、队列失败与释放语义统一见[键鼠输入通知的内核生产链](../keyboard/lighting-power.md#键鼠输入通知的内核生产链)及统一收据 `mapping_kernel_input_producer`，不平行维护另一份进度。当前结论为静态资格，未执行驱动或真实设备对照。

## 原件与复核位置

- PID158 UI：`.analysis/inputs/frontend/mouse-158-current/raw/apps.razer.com/synapse/products/158/ui/static/js/main.d9d086de.js`，Power reducer `[5078768,5079536)`，PowerSaving 控件 `[5692706,5693416)`，LowPowerMode 类从字节 5693663 附近开始；本页直接复读。
- 秒值按钮：`.analysis/inputs/frontend/mouse-199-current/raw/apps.razer.com/synapse/products/199/ui/static/js/main.0b830f5b.js` 字节 174399 附近；PID208 `.analysis/inputs/frontend/mouse-208-current/raw/apps.razer.com/synapse/products/208/ui/static/js/main.1754b8f8.js` 字节 198899 附近；直接复读其六项数组。
- PID117 dim UI：`.analysis/inputs/frontend/mouse-117-current/raw/apps.razer.com/synapse/products/117/ui/static/js/main.479a8db6.js` `[5600976,5601375)`；PID184 电池类型 UI：`.analysis/inputs/frontend/mouse-184-current/raw/apps.razer.com/synapse/products/184/ui/static/js/main.059dbbc3.js` `[5374611,5375250)`。
- PID158 中间件：`.analysis/inputs/frontend/mouse-mw-158-current/raw/apps.razer.com/synapse/products/158/mw/6259.1944c2392b18ba3f8282.js`，sleep runner `[1711424,1712396)`，lowPower task maker `[1557615,1558572)`，brightness writer 字节 1663938 附近，generic sleep/dim helper 字节 2235400～2236800；直接复读上述原始函数与调用。
- PID104／105 UI：`.analysis/inputs/frontend/mouse-104-current/raw/apps.razer.com/synapse/products/104/ui/static/js/main.9b94769e.js` 与 `.analysis/inputs/frontend/mouse-105-current/raw/apps.razer.com/synapse/products/105/ui/static/js/main.ef834389.js`；Indicator 卡和 Lighting `[5571458,5573711)`、reducer `[5136249,5137050)`、初始导航及加载 `[5809946,5817125)`、连接更新 `[5817628,5818381)`。
- PID104 middleware：`.analysis/inputs/frontend/mouse-mw-104-current/raw/apps.razer.com/synapse/products/104/mw/763.2548e8e4d18b77f23137.js`；restore `[1387513,1387719)`、连接初始化 `[1409845,1410374)`、无线事件消费者 `[1245841,1246018)`、maker `[1564100,1565032)`、writer `[1730580,1731547)`、回调 `[1731547,1732610)`、实际连接读取 `[2191803,2191965)`。两个产品各自获取收据保留原始身份。
- 页面挂载与原始 CSS 定位：`.analysis/results/frontend/mouse-main-audit.json` 的 `mouse_page_layout`；该报告当前只保留 PID190 的查询结果，不是全产品索引。

这些记录是源码静态语义，未执行真实灯光／电池／休眠写入及恢复测试。

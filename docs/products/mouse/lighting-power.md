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

## 打开、事件、失败与退出

亮度、效果、idle、lowPowerMode 等字段由各产品挂载的 profile restore 和对应 MW_SET 消费者载入；设备电量／充电事件通过 batteryReducer 更新状态，不能用电量事件推断配置已保存。

原件部分读取 helper 捕获异常后返回传入默认值（例如 getPowerSavingFromDevice 默认 5）。这是需要原样理解的失败分支，不能在文档中称它为真实设备读数。常规 task runner 则把错误发布到 action/error 通道，保留已编辑的乐观值；上述完整 sleep writer 会在取消或读回不一致时失败，不执行成功结算。

连接类型、主接收器或配置改变会改变真正的 rzDevice 和 region 选择。设备任务由其队列拥有；效果动画、屏幕取色等持续功能另有自己的启动／停止链，不能把关闭一个卡片当作所有灯光工作已停止。

## 原件与复核位置

- PID158 UI：`.analysis/inputs/frontend/mouse-158-current/raw/apps.razer.com/synapse/products/158/ui/static/js/main.d9d086de.js`，Power reducer `[5078768,5079536)`，PowerSaving 控件 `[5692706,5693416)`，LowPowerMode 类从字节 5693663 附近开始；本页直接复读。
- 秒值按钮：`.analysis/inputs/frontend/mouse-199-current/raw/apps.razer.com/synapse/products/199/ui/static/js/main.0b830f5b.js` 字节 174399 附近；PID208 `.analysis/inputs/frontend/mouse-208-current/raw/apps.razer.com/synapse/products/208/ui/static/js/main.1754b8f8.js` 字节 198899 附近；直接复读其六项数组。
- PID117 dim UI：`.analysis/inputs/frontend/mouse-117-current/raw/apps.razer.com/synapse/products/117/ui/static/js/main.479a8db6.js` `[5600976,5601375)`；PID184 电池类型 UI：`.analysis/inputs/frontend/mouse-184-current/raw/apps.razer.com/synapse/products/184/ui/static/js/main.059dbbc3.js` `[5374611,5375250)`。
- PID158 中间件：`.analysis/inputs/frontend/mouse-mw-158-current/raw/apps.razer.com/synapse/products/158/mw/6259.1944c2392b18ba3f8282.js`，sleep runner `[1711424,1712396)`，lowPower task maker `[1557615,1558572)`，brightness writer 字节 1663938 附近，generic sleep/dim helper 字节 2235400～2236800；直接复读上述原始函数与调用。
- PID104／105 UI：`.analysis/inputs/frontend/mouse-104-current/raw/apps.razer.com/synapse/products/104/ui/static/js/main.9b94769e.js` 与 `.analysis/inputs/frontend/mouse-105-current/raw/apps.razer.com/synapse/products/105/ui/static/js/main.ef834389.js`；Indicator 卡和 Lighting `[5571458,5573711)`、reducer `[5136249,5137050)`、初始导航及加载 `[5809946,5817125)`、连接更新 `[5817628,5818381)`。
- PID104 middleware：`.analysis/inputs/frontend/mouse-mw-104-current/raw/apps.razer.com/synapse/products/104/mw/763.2548e8e4d18b77f23137.js`；restore `[1387513,1387719)`、连接初始化 `[1409845,1410374)`、无线事件消费者 `[1245841,1246018)`、maker `[1564100,1565032)`、writer `[1730580,1731547)`、回调 `[1731547,1732610)`、实际连接读取 `[2191803,2191965)`。两个产品各自获取收据保留原始身份。
- 页面挂载与原始 CSS 定位：`.analysis/results/frontend/mouse-main-audit.json` 的 `mouse_page_layout`；该报告当前只保留 PID190 的查询结果，不是全产品索引。

这些记录是源码静态语义，未执行真实灯光／电池／休眠写入及恢复测试。

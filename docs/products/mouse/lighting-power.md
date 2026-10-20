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
- 页面挂载与原始 CSS 定位：`.analysis/results/frontend/mouse-main-audit.json` 的 `mouse_page_layout`；该报告当前只保留 PID190 的查询结果，不是全产品索引。

这些记录是源码静态语义，未执行真实灯光／电池／休眠写入及恢复测试。

# 附件：产品分支与本机功能

当前保留的附件 UI/MW 入口覆盖 56 个 PID。附件并非单一灯光设备类型：接收器、显示器、摄像头、局域网灯具、热管理、音频端点和普通 Chroma 设备使用不同的设备所有者。下表按每款当前 MW bootstrap 的实际注册区分分支；导航顺序、页面参数和组件挂载见 [逐产品文档](products/)。

## 产品与功能入口

| PID | 当前设备所有者与产品差异 | 功能说明 |
| --- | --- | --- |
| 126 | `rzDevice25`；Mouse Dock；兼容鼠标 122/124/134/143，挂载鼠标容器 | [底座与连接](receiver-and-dock.md) |
| 164 | `rzDevice25` + DualLink `LINKER`；brightness region 15；底座兼容 170 | [配对与底座](receiver-and-dock.md) |
| 179 | DualLink `LINKER`；单配置；指示灯在 slave 模式禁用 | [接收器](receiver-and-dock.md#179-的读取指示灯和配对) |
| 207 | `LINKERMULTIDEVICES`；单配置、自动配对、多设备，独立 `deviceUI` 配置 | [多设备](receiver-and-dock.md#164207241-与多设备差异) |
| 241 | `LINKERMULTIDEVICES`；多设备、region 15，底座兼容 170 | [多设备](receiver-and-dock.md#164207241-与多设备差异) |
| 769 | `PhilipsHueMgr`；桥、娱乐区域、独立灯具生命周期 | [Hue](lighting-and-cooling.md#769-桥娱乐区域和灯具) |
| 778 | `rzASRock`；强制检查 edition；10 个 ARGB stripe，edition 0/128 与 129 使用不同固定端口/LED 表；原版 `RzASRock`/系统 helper | [ARGB](lighting-and-cooling.md#端口布局识别和名称) |
| 780 | `rzDeviceIoT`；`keyLights`，白光面板与 RGB 分开 | [IoT](lighting-and-cooling.md#780791-局域网灯具) |
| 781、782、783 | `rzDeviceIoT`；`gameRoom`；灯 Pro、灯、灯泡各用当前页面配置 | [IoT](lighting-and-cooling.md#780791-局域网灯具) |
| 784 | `gameRoom`；LED layout、RGB；挂载自定义灯带与 Lighting | [灯带](lighting-and-cooling.md#端口布局识别和名称) |
| 790 | `gameRoom`；`WhiteAndRGB`；查询供电状态 | [供电](lighting-and-cooling.md#780791-局域网灯具) |
| 791 | `gameRoom`；查询供电状态；注册 `persistHardwareSettings` | [亮度限制](lighting-and-cooling.md#780791-局域网灯具) |
| 3072 | `rzDevice25Legacy`；Firefly Hard，亮度 region 5 | [通用灯光](lighting-and-cooling.md#通用灯光与实际状态) |
| 3073、3074、3076、3077、3078、3080 | `rzDevice25`；Goliathus/Extended、Firefly V2、Strider、Goliathus 3XL、Firefly V2 Pro | [通用灯光](lighting-and-cooling.md#通用灯光与实际状态) |
| 3331 | `rzDeviceKateT2`；单配置、忽略自身 USB remove；音频监听、虚拟音频通道、`simpleService` 事件 | [采集与音频端点](receiver-and-dock.md#采集卡和带音频的底座) |
| 3587、3589、3590 | 分别 `rzCamKiyo`、`rzCamKiyoPro`、`rzCamKiyoX`；原版 DLL 加载后 `startCamera`，解锁重载，不运行通用设备模式初始化 | [摄像头](camera.md#358735893590-的摄像头生命周期) |
| 3592 | `rzCamKiyoProUltra`；advanced camera；解锁重载、跳过通用设备模式 | [Ultra](camera.md#3592-的独立高级摄像头) |
| 3594 | `rzCamKimberly`；V2 Pro；advanced、sharpness/gain/shutter guard/FW restore 声明；自动构图 speed/size 的实际挂载另行判断 | [V2 Pro](camera.md#3594-v2-pro) |
| 3595 | `rzCamKimberly`；V2；音频功能、扩展 UVC exposure；自动构图 size：balanced=0、far=1、near=2 | [V2](camera.md#3595-与-3596-的共同链和差异) |
| 3596 | `rzCamKimberly`；V2 X；非扩展 UVC exposure、固件不支持设备模式；未注册自动构图配置 | [V2 X](camera.md#3595-与-3596-的共同链和差异) |
| 3848、3853、3859、3863、3866、3869、3878、3879、3883、3894 | `rzDevice25`；Base Station、Laptop Stand、机箱、Core X Chroma、Mouse Bungee、Charging Pad、桌面机、Stand V2、Head Cushion 各按当前页/region 呈现 | [通用灯光](lighting-and-cooling.md#通用灯光与实际状态) |
| 3849 | `rzDevice25` + `isLightingKit`；Chroma HDK，端口灯光分支 | [端口](lighting-and-cooling.md#端口布局识别和名称) |
| 3858 | `rzDevice25Ex`、`isMonitor`；原版 Scarlett 加载后 `getMonitorId` | [显示器](lighting-and-cooling.md#3858-与-3880-显示器) |
| 3880 | `rzDevice25Ex`、`isMonitor/isScarlett15`；加载后 `getScarlett15Settings` | [165 Hz 分支](lighting-and-cooling.md#3858-与-3880-显示器) |
| 3871 | `rzDevice25Glitter`、`PROTOCOL25_GLITTER`；6 stripe 的多端口 ARGB、外部供电 | [ARGB](lighting-and-cooling.md#端口布局识别和名称) |
| 3884 | `rzDevice25GlitterWireless`；3 stripe 多端口；GLITTER 事件、`skipFilterDriver:false`、外部供电 | [无线 ARGB](lighting-and-cooling.md#端口布局识别和名称) |
| 3886 | 同名 wireless 适配器，但注册 `isMultiLightingDevice`，不是 3884 的 `isMultiPortDevice`；3 stripe、外部供电 | [多灯光](lighting-and-cooling.md#端口布局识别和名称) |
| 3872、3873 | `rzDevice25`；Base Station V2、Thunderbolt 4 Dock；`simpleService` 音频事件 | [音频底座](receiver-and-dock.md#采集卡和带音频的底座) |
| 3893 | `rzDeviceElsa`；Hanbo；可用 ARGB port 2147483651；独立风扇/水泵和系统温度 helper | [Hanbo](lighting-and-cooling.md#3893-hanbo-风扇水泵和温度) |
| 3900 | `rzDevice25PWMPCFanController`；`PWM_PC_FAN` 事件、跳过 filter driver、外部供电；无 stripe 数的多灯光配置 | [PWM](lighting-and-cooling.md#3900-pwm-风扇控制器) |
| 3907 | `rzDevice25`，设备模式 `ignoreCheckMode:true`；外部供电、fan power、HyperBoost、`skipSortGPU` | [散热垫](lighting-and-cooling.md#3907-散热垫与-hyperboost) |
| 3921 | `rzDevice25FiverV2`、`isEGPUV2`；`PROTOCOL25`/跳过 filter driver；系统 helper | [外置显卡机箱](lighting-and-cooling.md#3921-egpu-温度曲线) |
| 3922 | `rzDevice25`；Thunderbolt 5 Dock；`mappingEngine` 音频服务事件，挂载 Sound/Lighting | [音频底座](receiver-and-dock.md#采集卡和带音频的底座) |
| 3929 | Monitor Stand；`mappingEngine` 音频事件、处理不支持的映射事件；实际挂载 Customize | [映射](receiver-and-dock.md#采集卡和带音频的底座) |
| 3932 | Handheld Dock；同类音频/映射事件配置；实际导航为 Lighting，不能添加未挂载的 Sound 页 | [底座](receiver-and-dock.md#采集卡和带音频的底座) |
| 3940 | `rzDevice25GlitterWireless`；GLITTER/跳过 filter driver、外部供电、keep-alive、generic `devicePowerMode` | [电源与灯光](lighting-and-cooling.md#通用灯光与实际状态) |
| 3946 | `rzDevice25`；`isAudio.augustT2="true"`、`simpleService` 事件；Customize/Lighting | [自定义与音频](receiver-and-dock.md#采集卡和带音频的底座) |

## 功能阅读顺序

- [灯光、端口、显示器与散热](lighting-and-cooling.md)：端口及灯具身份、真实亮度、热曲线、系统温度、供电事件和停止清理。
- [接收器、底座与采集](receiver-and-dock.md)：配对的候选/绑定/取消、指示灯、多设备协议、音频服务与映射边界。
- [摄像头](camera.md)：预览、图像设置、缓存比较、保存、重置、原生属性页与产品固件分支。
- [配置档](../shared/profiles-linked-games.md)和[帮助与重置](../shared/help-reset.md)：与每款设备的特有 restore 顺序连读。

## 原件与完成边界

每款 UI 与 MW 的获取入口分别为 `.analysis/inputs/frontend/accessories-<PID>-current/acquisition.json` 和 `accessories-mw-<PID>-current/acquisition.json`。本表核对全部 56 份 MW main 的实际 AST 注册；引用文件的字节核对不等于每款 lazy chunk、原生库和本地服务的全部依赖闭包已核实。

深层函数由 [附件主分析](../../../.analysis/results/frontend/accessories-main-audit.json)、[接收器分析](../../../.analysis/results/frontend/receiver-local-runtime-audit.json)和[摄像头分析](../../../.analysis/results/frontend/accessories-camera-local-pages-audit.json)定位到当前原件。文档说明原版行为与本项目的离线范围，不把原版 DLL 包装器当成 Rust 独立实现，也不把静态阅读当成 UI/硬件验收。未解间接调用和原生语义仍由统一 gaps 维护。

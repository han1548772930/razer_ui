# 接收器、底座与采集卡

接收器的硬件配对表、本机设备目录、配置档和 UI 临时候选各有自己的来源。候选出现在扫描列表、设备被 USB 枚举、绑定命令返回以及收到绑定完成事件分别表示不同状态。各款实际页面树见 [126](products/126.md)、[164](products/164.md)、[179](products/179.md)、[207](products/207.md)和[241](products/241.md)。

## 179 的读取、指示灯和配对

179 的完整调用方、任务和协议所有者由 [接收器静态分析](../../../.analysis/results/frontend/receiver-local-runtime-audit.json)定位。UI 原件为 [main.55a48063.js](../../../.analysis/inputs/frontend/accessories-179-current/raw/apps.razer.com/synapse/products/179/ui/static/js/main.55a48063.js)，MW 为 [6120.ef5d145fafdfd0069d53.js](../../../.analysis/inputs/frontend/accessories-mw-179-current/raw/apps.razer.com/synapse/products/179/mw/6120.ef5d145fafdfd0069d53.js)。下列字节区间均指原始 UTF-8 字节。

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
| 3929 | mappingEngine；处理 unsupported mapping events | Customize/Lighting，映射与音频端点分开 |
| 3932 | mappingEngine；同类映射事件处理 | 当前实际导航为 Lighting；没有由此新增 Sound 页的依据 |
| 3946 | `isAudio.augustT2="true"`、simpleService | Customize/Lighting；本机默认端点观察有独立消费者 |

simpleService 首先按当前设备识别扬声器/麦克风 ID，再读真实 volume/mute，写入同时携带目标 ID、mute 和音量。mappingEngine 使用实际默认扬声器/麦克风 getter、setter 与 speaker/microphone/sidetone 事件；默认端点和 dock HID 容器不是同一身份。两种通道由各自注册参数选择，不能在失败时改用另一端点伪装成功。共享音频接口默认最多九次读取尝试、间隔 2000 ms；返回值仍要求存在且 result 不是 false，不代表本产品每个消费者都已严查全部字段。

simpleService 注册 speaker/microphone 事件和 sidetone 配置，再监听 `audioServiceEvent`；mappingEngine 注册对应服务事件并监听三种变化。清理按各自分支注销，再移除同一个回调。3331 cleanup `[1403213,1405441)` 还包含任务/观察停止、退出模式、USB 资源移除、通知清除、音频注销和设备 terminate；部分 terminate 未等待，不能据此保证所有原生工作已退出。3946 另有设备列表、默认端点变化与 `audioServiceEvent` 所有者，需连同该分支退出一起实现。

## 验证边界

本文来自当前完整 JS 的静态调用链与现有静态原生证据；实际 UI 分支、HID/服务读写和本地配置分别描述。原版包装器、本地目录读取或编译成功均不等于本项目 Rust 独立实现完成。没有运行应用、厂商 JS、安装器、DLL 或设备操作；真实配对、音频及清理验收保持未验证，继续沿当前原件闭合底层服务与协议。

# 系统：显示、GPU 切换与电池充电

显示设置改变当前 Windows 显示模式、GPU 路由或面板模式；Battery 页面改变充电限制及一次性充满开关。它们与 [性能模式](thermal-performance.md) 的 CPU/GPU 功率档位不同。实际入口和条件均以当前产品挂载为准，不能仅按标签页名称判断功能是否存在。

## 全产品页面分支

| 当前产品 PID | 实际入口与差异 |
| --- | --- |
| 563、564、569、570、571、576、581、582、586、587、588、589、594、595、597、598、601、616、618、621、622、623、624、630、633、634 | 无独立 Display／Battery 标签；显示相关卡片随各自 Performance 树挂载。例如 564、621 按 `isDDSSku`／`supportedFlag` 分流 GPU 切换或显示控制，598、622 的部分卡片显式 `isShowScreenRefreshrate=false`；不据此添加不存在的独立页面 |
| 650、651、652 | 独立 Battery；显示仍在 Performance，650／652 有 DDS SKU 条件，651 显式挂载刷新率卡片 |
| 669、670 | 独立 Display 和 Battery；Display focus 请求 `getColorProfilesList`，卸载移除同一处理器 |
| 671、672 | 独立 Display 和 Battery；native 模式卡片要求 SKU `miniLedResolution != 0`；671 的刷新率卡片使用 Blade2 Native 路径 |
| 694 | 独立 Display 和 Battery；Battery 增加一次性充满与 Fn+B；Display 使用按钮式刷新率与颜色配置卡片 |
| 695 | 同上；Mini LED 支持值决定 local dimming 和 native 模式卡片 |
| 696 | 同上；Mini LED 支持值决定 local dimming，当前挂载树未加入 native 模式卡片 |
| 709、710、736 | 同上；Mini LED 支持值决定 local dimming 和 native 模式卡片 |
| 711、737 | 当前 Display 显式 `supportLocalDimming=false`；native 模式给定 3840×2400 / 240 Hz 与 1920×1200 / 440 Hz；Battery 有一次性充满与 Fn+B |

逐产品页保留完整源文件、导航与页面 owner 区间，见 [系统索引](README.md)。表中“无独立标签”只描述导航，早期嵌入卡片仍属于真实产品功能。

## 刷新率、显示事件与电池策略

PID737 `pa` 卡片从 `refreshRateList`、`screenRefreshRate`、`screenDisplayMode`、真实分辨率和 GPU mode 构建按钮。分辨率任一维为 0 时显示未知并限制操作；仅外接显示模式也有独立禁用条件和提示。DDS 动态刷新率、100 Hz 过滤、额外独显刷新率均来自产品数据分支，不能将全部枚举固定写成相同列表。

点击刷新率先更新组件 state，再发 `setScreenRefreshRate`。UI 的初始选择采用 `screenRefreshRate || refreshRateList[0].value`；后续收到非 0 的中间件值才更新本地选择。这个表达式是原件显示逻辑，不证明第一个选项是设备实测值。

中间件初始化查询支持列表，并按当前供电／配置档分支决定是否继续读取当前刷新率，另查询 display mode、分辨率、HDR 和 SKU 硬件配置。共用 Blade2 wrapper 保留去重及相差 1 Hz 归一的 filter 方法；当前 PID737 未配置 `needFilterFreshRate`，实际走 `enumScreenRefreshRates()`／`getScreenRefreshRate()`，不能把未调用的 filter 当作本产品已执行步骤。`onDisplayChange` 由 native bridge 回调转换为 `eventEmitter` 事件，其后重新查询并发布状态。

maker `gd` 在允许保存且来源不是设备事件时更新设备级 `screenRefreshRate` 和版本，先广播本地 completed，再排队 runner `je`。runner 调用 `setScreenRefreshRate`，检查取消，更新任务内存缓存，发送 `MW_SET_SCREEN_REFRESH_RATE_TO_UI`；其本体没有写后 GET，也没有核对 setter 返回的布尔值。Blade2 wrapper 实际向 `blade2NativeAction` 发送 `SetScreenRefreshRate`，参数为一个刷新率数值；wrapper 返回 false 与 runner 抛异常是不同路径。

“电池时降低刷新率”保存在当前配置档 `isBatteryModeScreenRefreshRate`。电池供电时更改开关，maker `fd` 立即排入另一个 `ON_SET_SCREEN_REFRESH_RATE`：启用取产品 `batteryModeScreenRefreshRate`，缺省为 60 Hz；关闭取设备级保存的接电刷新率，并显示通知。启用且当前在电池上时，UI 禁用手动刷新率卡片。插拔电源也按同一策略决定恢复值。

## PID737 当前刷新率的原生结果消费者

统一静态入口为 `python -B -X utf8 tools/workspace.py audit-family-function --family system --function mw --query system-display-refresh-native-chain`，结果归现有 [system-mw-audit.json](../../../.analysis/results/frontend/system-mw-audit.json) 的 `display_refresh_native_chain`。本片绑定当前 [MW 获取收据](../../../.analysis/inputs/frontend/system-mw-737-current-refresh-ec3db4b29c5a-2/acquisition.json)、[UI 获取收据](../../../.analysis/inputs/frontend/system-737-current-refresh-ec3db4b29c5a/acquisition.json)及 [4.0.896 主机获取收据](../../../.analysis/inputs/host-current/releases/4.0.896/acquisition.json)。两份前端没有对应完整捕获，本片直接从经校验的当前原件提取所需完整 AST；未为此重捕整个产品。

当前 UI 的实际链为 `Ha → Ba → ma → ca`：Display 左列挂载按钮式刷新率卡片，`supportLocalDimming=false`；`ca` 先改变组件选择，再调用 `ia` action。`screenRefreshRateReducer` 只接受当前列表已有的请求值和观测值，按 timertick 分别记录本地持久化与设备任务结果；错误进入 `errorActions`，没有自动回滚选择。组件初值仍用 `screenRefreshRate || first value`，后续忽略 0 的 props 更新；这些显示行为均不证明该值已由设备确认。

当前 MW 的 `Fd` maker 先保存设备级请求值与版本、发布本地 completed，再排入可跳过任务；runner `et` await setter 后不检查返回的 bool，直接设置 `result=true`。只有实际抛异常才设置 `result=false,value=null` 并拒绝任务；abort 在 await 后、更新缓存和 UI 前检查，不能撤销已经完成的 OS 写入。没有写后 GET。完成消费者 `tt` 把 completed／skipped 转成完成；其异常分支又访问无空值保护的 `r.value.params`，与 runner 已置 `value=null` 的拒绝路径存在额外异常边界，不能补写为可靠的统一失败回滚。

Blade2 实际继承 `rzDevice25LedMatrixBlade` 的异步方法，走 `19635.oE → doDLLMainActionAsync → ffiPreloadAsync`。当前主机按通道持有 mutex，在 `finally` 释放；原生异步 callback 没有检查其 error 参数，只 resolve result 参数。bool／int 直接返回；pointer 成功 `readCString()` 后才置释放标志并在 `finally` 调 `FreeMalloc`，字符串转换抛错时该标志仍为 false。主机与桥的 catch 记录错误后返回未定义值，所以 `false`、`undefined`、抛异常和已确认成功必须分别保留；runner 会把前两种正常 resolve 的结果继续当成功。

当前 manifest 的 `BladeDLL` 为 **1.0.50.0**，实际 DLL 为 932,552 字节，SHA-256 `6f6c875527ca8a147ef07a2737979a5fa4af1f11b99f5e26be5128bc77baca73`。沿现有 [完整捕获](../../../.analysis/results/native/6f6c875527ca8a147ef07a2737979a5fa4af1f11b99f5e26be5128bc77baca73/capture.json)复用以下函数，Image Base 为 `0x180000000`，所有选定函数的原始机器码均与 PE 校验一致。

| 原生边界 | RVA 与当前行为 |
| --- | --- |
| 枚举 | `EnumScreenRefreshRates 4c590` 调 `491d0` 枚举活动内置面板；`47ec0` 依次尝试 QueryDisplayConfig flags 82／18／2，缓冲不足每组最多重试 8 次；`48160` 建面板身份，`48840 → 485a0` 合并当前尺寸的 normal／raw 可用模式；`48300` 只在 INTERNAL／EXTERNAL 且列表非空时学习进程内的面板模式并集。返回 CSV 包含已学习值，不等于本次 OS 独立返回的全部模式 |
| 共用备用枚举 | `EnumScreenRefreshRatesBlade 52520` 和 `42600` 匹配内置面板 DeviceID，并按当前 DEVMODE 的尺寸／色深／方向枚举；当前 PID737 的未过滤分支不调用它 |
| 当前读取 | `GetScreenRefreshRate 4ca40 → 49db0` 优先取当前 DEVMODE 的有效频率，再使用活动路径的频率；helper 失败时外层保留初始 0，不能将 0 提升为成功实测 |
| 设置 | `SetScreenRefreshRate 4caa0 → 42600` 查找活动内置面板、取当前 DEVMODE，只替换频率并调用 `ChangeDisplaySettingsExW(..., flags=1)`；非零 OS 结果返回 false，继续枚举多个匹配项时后一个结果可覆盖前一个 bool。它不执行写后 GET |
| 显示模式与返回内存 | `GetScreenDisplayMode 4cda0` 读取拓扑；`SetAllocator 4aaf0` 保存分配 callback，枚举导出使用该 callback 或本机分配；`FreeMalloc 4daf0` 对非空指针执行本机释放 |

观测消费者与写入分开：显示变更 `Wi → $i` 独立读取当前值和列表，按电池门禁决定发布及更新本地版本；初始化 `Mo` 在电池降刷新分支可只发布配置的 60 Hz，否则读取后排入来源为 local storage 的 setter。READY 广播另做 getter，并把空列表／假值转换成 60 Hz。这些源码回退属于界面或缓存来源，不能被独立 Rust 实现误标为设备实测，也不能据此新增读取失败后的设备默认写入。

本片保留 57 个角色、71 个唯一完整 AST owner、15 个完整原生函数及其直接／间接／未展开边。10 项定向纯测试覆盖实际来源、完整 owner、错误与取消、bool／0／空值、分配释放、变化反例、查询路由和快照变化先记失败再抛错。捕获中其他函数的 Hex-Rays 失败仍保留，未声称整个 DLL 语义完成。[现有通知／退出分析](../../../.analysis/results/host-current/local-audit.json)仅作为边界索引链接，本片不依赖其完成状态，也不新增 callback 排空、线程 join 或宿主卸载结论。未运行厂商 JS、DLL、主程序或系统写入；独立 Rust、UI／后端接线、实际 DLL／OS／设备对照仍未验证。GPU、ICC、Battery 和其他产品继续各自分析。

## Native 面板模式与 GPU 路由

native 模式卡片保留 `nativeDisplayMode` 草稿和 `showAlert`。点击模式只打开确认；Cancel 关闭并从 props 恢复原模式；确认后才发送 `setNativeDisplayMode`。未由产品提供模式表的共用卡片默认是 3840×2400 / 120 Hz 与 1920×1200 / 240 Hz；711／737 由页面覆盖为 240／440 Hz。runner `Xe` 先 `setMiniLEDPanelResolution`，再 `restartSystem`，随后结算；这条链没有在重启前另读面板模式。

### PID737 GPU 路由与重启的静态边界

统一入口为 `python -B -X utf8 tools/workspace.py audit-family-function --family system --function mw --query system-gpu-route-native-chain`，结果归现有 [system-mw-audit.json](../../../.analysis/results/frontend/system-mw-audit.json) 的 `gpu_route_native_chain`。本片使用当前 [MW 获取收据](../../../.analysis/inputs/frontend/system-mw-737-current-refresh-ec3db4b29c5a-2/acquisition.json)、[UI 获取收据](../../../.analysis/inputs/frontend/system-737-current-refresh-ec3db4b29c5a/acquisition.json)和 [4.0.896 主机获取收据](../../../.analysis/inputs/host-current/releases/4.0.896/acquisition.json)，直接提取当前原件完整 AST，复用已有原生捕获。公共 HID 复用 [audio-mw-audit.json](../../../.analysis/results/frontend/audio-mw-audit.json) 的 `/auto_pause_hid_chain/shared_feature_transport` 精确 canonical 子段，未冻结其无关联合收据或重捕公共原生链。

当前 `7934.25938d65.chunk.js` 的 Display `Ba → Va → Fa` 实际挂载 GPU 卡片。卡片保留 `mode`、`previousMode`、`restartRequired` 草稿；Cancel 恢复先前草稿，不写设备。非 DDS 只有 `supportedFlag=SWITCH(0)` 才显示 Optimus(0)／Dedicated GPU(1)；DDS 按模式 0／2 或 1 展示各自说明及 NVIDIA 控制面板入口。后者仍是独立动作，不据此闭合整个控制面板功能。props 更新仅对 `MICROSOFT_HYBRID(0)`／`DISCRETE_MUX_ONLY(1)` 同步草稿。Restart 的 `Pa` 依次 dispatch `W0e(mode)`、`mk1(null)`；reducer 先乐观更新 mode，再发送独立 `ON_SET_GPU_MODE`／`ON_RESTART_SYSTEM`，没有设备模式回滚。

初始化 `Vo` 通过 `tL` 查询 VGA mode，发布后再进行 DDS 分类；`ko → Po` 另行读取至运行缓存，READY 发布缓存 mode、DDS 和 supportedFlag。`tL` 捕获读取／解码异常后返回源码本地 `total=0,mode=0`，不是设备实测。PID737 为 series12，当前 `SupportedDDSSKU=[]`；共享 `extraCondition` 注入仍有条件分支，否则读取 Synapse UI 的 switchInterface 低四位及 VGA hardware bit4。分类读取失败保留本地 `supportedFlag=1`、`isDDSSku=false`，不能称作已读取的硬件能力。

真实继承为 Blade2 → Blade → LedMatrix → protocol25，VGA 四个方法没有中间 override。GET VGA 为 `[2,13,0x89]`，SET 为 `[2,13,0x09]`，两字节 SET payload 为 `[0,mode]`；其他两个分类 GET 分别为 `[2,13,0x8b]`、`[2,13,0x8a]`。setter 仅拒绝 undefined／NaN，其余值由 Uint8Array 转为字节，不添加推测值域。Report ID 0、90 字节 payload、OUT 期望返回 91；XOR bytes2..87 至 byte88。IN 检查 transaction byte1、class/id bytes6/7，status2 成功，0／3／4 请求 OUT 重试，busy1 继续 IN，5／未知失败；未校验响应 checksum。Host false／undefined 转零数组是本地缺失应答路径。GET／SET 共用 decoder，只在 SUCCESS 且 packetSizeReturned>0 时取 data0／data1，没有确认至少两字节。SET runner 忽略返回对象；成功返回后检查 abort，再记录请求 mode／版本，没有独立 GET 读回。

SET 与 Restart 均进入同一 `gpuMode` 的 `NORMAL_SKIPPABLE` 队列：空闲队列立即运行，积压时取最新并把旧 pending 记为 skipped。maker 记录请求与 `REQUIRED_RESTART_SYSTEM=true`；持久化和 completed 不证明模式已观测。Restart runner 先创建 5000ms timer、记录 result true，再检查 abort；不保存 timer handle、不随任务取消 timer，也不等待或消费延迟 callback 的拒绝。两个 completion 均按 value.status／skipped 结算；错误分支读取 `r.value.params`，而异常／取消路径可能给出 null。不能把排队 completed 当作原生调用或系统重启完成。

timer 的实际落点为 Blade2 `restartSystem → callDLLApi`。未初始化时它调用 `init()` 但不 await，只在当下 `isInit` 为 true 时发送；FFI 明确声明 `RestartSystem:["void",[]]`。当前主机通过 `ffiPreload`、通道 mutex 与同步 `_handleAction_Default` 执行，异常记录为 undefined，mutex 在 finally 释放；旧 bridge 在当前 preload 只记录 deprecated。当前 BladeDLL 1.0.50.0 的 `RestartSystem` RVA `0x4aab0` 把固定 `shutdown /f /r /t 0` 交给 CRT system `0x81b1c`，依 COMSPEC／cmd.exe／PATH 分支到 `CreateProcessW`，mode0 等待子进程并取退出码。已保留正常／失败的 `CloseHandle` 和临时内存释放；等待返回本身未检查。CRT 的子进程退出码或 -1 被 FFI void 丢弃，未取得 Windows 重启成功、新连接或新 mode 的确认。

本片保留 110 个角色、138 个唯一完整 AST owner、16 个完整原生函数及 150 条直接边。10 项定向纯测试覆盖当前来源、完整 owner、命令／ACK、回退、草稿与取消、timer／void、原生命令／进程结果／释放、上游变化及先记录失败再抛错。完整原生 helper 的未展开／导入／间接边和无关捕获失败仍保留，未宣称所有 CRT 异常、通知排空或 DLL 卸载完成。未执行厂商 JS、DLL、主程序或 OS 重启；独立 Rust、UI／后端接线与实际 DLL／OS／设备对照仍未验证，其他产品和 ICC／面板模式／性能分支分别继续。

## HDR、local dimming 与 ICC

local dimming 是否有 UI、产品是否走该功能、当前 Windows HDR 状态是三种条件。新系列 10／11／12 的 local dimming runner 通过 PowerModeControl 位读改写；其他系列调用 `setBladeMonitorWindowsHDRMode`。初始化也有不同系列的同步条件，不能将它们统一为“打开页面即写同一个开关”。

ICC 卡片用文件名匹配实际 `colorProfilesList`，提供 Native、Display P3、sRGB、BT.709、Adobe RGB 和 Custom 的产品列表。系统未安装的预置文件和正在加载时的其他选择会禁用；Custom 只从非预置文件中选择。HDR 开启时显示专用说明；缺少预置配置时展示恢复入口；颜色配置变更从中间件更新列表和选中项。

颜色 setter runner `ot` 区分两种路径：若文件名属于运行时的 presetColorProfileNameList，则转换为设备显示模式索引并执行 `colorProfileSetDisplayMode`，必要时包围 CRT 优化处理；其他文件直接调用 `setColorManagementDefaultICCProfile(fileName)`。恢复 runner 走独立 `H3(false)` 链。UI 的 Windows 颜色管理入口启动本机 `colorcpl.exe`，外接显示设置入口打开 Windows Display settings；它们不写某份本地 JSON 冒充系统设置。

## 电池限制、读取与一次性充满

当前 Battery 控件的阈值选项为 50、55、60、65、70、75、80%，滑块步长 5%。开关 `isEnabled` 与阈值 `value` 分开；有一次性充满能力的挂载另有 `batteryChargingOverrideEnabled`。开启一次性充满时阈值编辑禁用，限制开关关闭时一次性充满不可操作。Fn+B 只在支持的挂载显示。

PID737 当前挂载 `ea`，action `Ki`／`Qi` 经 `systemBatteryReducer` 发出完整限制设置或单独 override 事件，界面先更新请求值。启动初始化及 READY 会读取 `getBatteryChargeLimiterControl`；只有成功解码的 enabled/value 源于设备返回，一次性充满字段在这两处实际取自本地配置。getter `Se` 在无设备或捕获错误后返回 `{isEnabled:false,value:0,batteryChargingOverrideEnabled:false}`，这是源码的本地回退对象，不能据此称硬件已关闭限制或返回了 0%。

本地恢复路径另有明确门控：存在 `getBatteryOptimizerFromFW.yes` 时跳过缓存恢复；系列 12 也跳过该恢复写入。其余产品会发布缓存状态并排入 `ON_SET_BATTERY_OPTIMIZER`。初始化读取与缓存恢复不可合并成一种规则。

UI 更改阈值或开关发送完整 `{isEnabled,value,batteryChargingOverrideEnabled}`。当前 maker `pp` 更新设备级持久化和版本，广播本地完成后排入 `NORMAL_SKIPPABLE` 的 `batteryOptimizer` 任务；runner `Ni` 调用 `setBatteryChargeLimiterControl(isEnabled,value)`，再按能力和开关组合设置 PowerModeControl 的一次性 override 位。单独点击一次性充满走 `fp`／`Fi`。两个 runner 都忽略 setter 返回对象，写后才检查取消；完成消费者按 `value.status=completed` 或 `error=skipped` 选择完成事件。没有独立写后 GET 或回滚，requested Battery 值还实际写入源码名为 `taskRunnerColorProfileMemoryStorageCache` 的字段，不能把这一保存动作当作设备确认。

当前 PowerModeControl helper `Ge` 先读 cpuBoost、localDimming、maxFanSpeedMode、oneTimeOverride 四个已解码掩码，替换目标字段后 OR 合并写回，未知高四位不会保留。其 catch 只记日志并吞错，因此 runner 可以在 helper 失败后仍标记 completed。Fn+B 来自当前固定 HyperShift `KEY_B`／`assignment:toggle`：编译为按下 disabled、抬起 `bladeBattery/toggle`，映射输出消费者解析实际 output 后排入 `ON_SET_BATTERY_CHARGING_OVERRIDE_ENABLED_BY_DEVICE`。maker `gp` 在 `storage flag && id=toggle` 分支才检查 limiter 已启用，再先更新本地 cache／UI、显示通知并排设备任务；不能把这个条件推广为所有输入分支的共同门禁。充满事件消费者还会清除页面 override，并在本地记录为开启时请求清位；其完整通知订阅与设备行为仍须独立验收。

实际通信路径为 `Blade2 → Blade → LedMatrix → rzDevice25`：Blade 覆盖两个 limiter 方法并转发共享模块 `34836`，PowerModeControl 继承自 rzDevice25。limiter GET／SET 头分别为 `[1,7,0x92]`／`[1,7,0x12]`，SET 单字节为 `(enabled<<7)|value`，成功且返回长度大于 0 才解码 bit7 和低 7 位；PowerMode GET／SET 为 `[1,7,0x8f]`／`[1,7,0x0f]`，override 为 bit2（掩码 4）。50…80／step5 是 UI 限制，不能据此补造协议值域。

PID737 继承的当前 Electron 路径使用 protocol `25`、90 字节报文、reportId `0`、期望 OUT 返回 `91`；byte88 为 bytes2…87 的 XOR。Host 4.0.896 在该长度分支加 reportId，随后走共享 HID feature-report 原生队列；公共 Host／原生所有者及发送、读取、Promise、mutex、关闭语义复用[音频公共传输子段](../../../.analysis/results/frontend/audio-mw-audit.json#/auto_pause_hid_chain/shared_feature_transport)，按 JSON pointer 与 canonical SHA 绑定，发布前复核。产品 IN 必须匹配 transaction／class／command；status2 成功，busy 继续读，部分错误重发 OUT，不支持／未知状态失败结束。缺失响应生成的 90 字节零数组仍是本地回退；本层没有响应 checksum 检查。`finally` 请求释放 HID mutex，没有证据证明由此排空在途 I/O 或 callback。

本片的当前源码、完整 AST owner、协议、结果消费者和公共依赖记录于[Battery 静态收据](../../../.analysis/results/frontend/system-mw-audit.json#/battery_native_chain)，入口为 `audit-family-function --family system --function mw --query system-battery-native-chain`。只补当前缺项并复用现有捕获；其他产品／固件／传输分支、完整通知生命周期、独立 Rust、UI／后端接线及官方实际 DLL／设备对照仍分别保持未验证，不能把一个 PID 的有限语义核对称作整个 Battery 或系统产品完成。

## 错误、取消与退出

刷新率、GPU、ICC、充电限制和 native mode 各自具有任务队列、版本、maker 与 runner 错误通道。多数 runner 在实际调用后检查 abort 并拒绝旧任务；setter 抛异常、helper 吞掉异常、setter 返回 false 分别保留其实际区别。组件草稿的 Cancel 只撤销还未提交的选择；不等同于设备回滚。旧式页面的 focus 监听在卸载时显式移除；OS/native 显示监听属于设备／中间件生命周期。

## 实际页面与当前 Rust 对照

PID737 的完整 Display 根 `Wa` 使用 `.blade-performance` 的两列布局。左列依次为刷新率 `ua → pa`、面板模式 `va → _a`、GPU 路由 `Ha → Ba` 和外接显示 `Ia → Pa`；右列为校准报告 `Oa → Ra`、ICC 配置 `ba → Da`。当前挂载明确传入 `supportLocalDimming:false`，不能从共享刷新率组件具有 local dimming 分支推定该产品应出现这个开关。Battery 的完整连接组件另行挂载，读取 battery reducer、override 与产品能力。

| 页面或动作 | 当前官方行为 | 当前 Rust 对照 |
| --- | --- | --- |
| Display 组件树与卡片顺序 | 左侧四块、右侧两块；GPU／面板模式各自持有重启确认，报告和 ICC 分别判断 HDR／文件能力 | [system/mod.rs](../../../crates/razer-pages/src/features/system/mod.rs) 的 `display` 只有一个刷新率 panel，内放 OS 设置链接、降刷新开关和面板模式按钮；没有完整两列、GPU、ICC、校准报告与对应状态树 |
| 刷新率观测与门禁 | 消费真实列表、当前值、分辨率、显示模式、GPU、实际供电和电池策略；分辨率未知、外屏独占及电池降刷新分别影响显示或禁用 | Rust 从 `draft.display.refreshRateList` 生成按钮，仅按列表长度决定 enabled；没有该 owner 的真实显示／供电观察和对应门禁。种子列表为空不能证明无能力，列表非空也不能证明设备读回 |
| 面板模式与 GPU 草稿 | 模式点击只打开确认；Cancel 恢复 props／previousMode；Restart 才提交，且面板模式和 GPU 使用不同实际动作 | Rust `choice` 立即修改 `/display/nativeDisplayMode` 并发普通 Changed，没有该确认、Cancel、Restart 或实际面板提交；GPU 分支尚未挂载。补本地弹窗仍不足以完成 OS／设备链 |
| Battery 结构与文案 | 阈值有宽 62px 的下拉框、百分号和 50…80、步长 5 的 slider；支持的产品另有 override 说明、Fn+B 图形快捷键和警告 | Rust `battery` 只挂限制 toggle、slider、可选 override toggle 与一条 note，未挂阈值下拉和完整 Fn+B／说明结构；已有 `enabled && !override_enabled` 编辑门禁和关闭限制时禁用 override 与原件方向一致，不能因此认定整卡一致 |
| 实际初值与失败 | 官方读取、缓存恢复和固件能力门控是不同来源；源码吞错或回退的结果不能提升为硬件已确认 | Rust `new` 使用 `spec.initial`，`restore` 合并本地配置；这里没有已确认、未观测、失败与草稿的完整分离，也未接到充电限制／override 的真实观测。不能因本机无配置就恢复默认到设备 |
| ICC、报告与系统入口 | ICC 按实际文件匹配预置／Custom、loading 与 HDR；报告在 Custom／HDR 时禁用，点击走 `colorProfileViewReport`；颜色管理与外显设置是独立 OS 入口 | Rust 目前只找到 `ms-settings:display` 按钮；该按钮是实际 OS 入口，但不能代替 ICC 枚举／切换／恢复、报告、GPU 与显示事件链 |
| 修改、保存与清理 | 各类 maker、runner、设备事件与结果消费者分别结算；原件没有为全部 setter 添加统一 GET | Rust `SystemProductChanged` 在 [source_workspace.rs](../../../crates/razer-pages/src/features/workspace/source_workspace.rs) 中只捕获 profile 快照并发 `WorkspaceEvent::Changed`；[shell.rs](../../../crates/razer-shell/src/shell.rs) 与 [persistence.rs](../../../crates/razer-shell/src/shell/persistence.rs) 未为该变化派发显示／GPU／充电限制设备动作，相关观察、取消和释放也不能由本地文件保存视为完成 |

样式需跟随实际 CSS 级联逐项恢复。PID737 面板模式 tile 为 132×47px、margin 5px、padding 5px、1px 边框、3px 圆角，普通背景 `#111`，active 为 `#222` 和绿色边框；hover 也改变边框。Battery override 行允许换行，说明为 14px／17px，快捷键行高 27px，Fn／B 按键宽 47px、水平边距 10px。它们并非共享 GPUI Button 的默认尺寸；标题、禁用、下拉与 slider 的后载规则也需要一起核对。本批只静态读取，未执行窗口或像素比较。

后续纯／模拟测试应分别验证：设备实读与种子／缓存不同时正确消费；未知分辨率、外屏独占、DDS、AC／DC 与降刷新禁用；两种重启确认的 Cancel 不产生设备提交；ICC 缺文件、Custom、loading／HDR；Battery 阈值和 override／Fn+B 的能力分支；更换设备拒绝旧结果，失败和取消完整释放。官方实际 DLL／服务对照仍须针对各自真实完成契约执行，严禁把本地保存、setter 已调用或系统重启任务排队当作硬件状态已确认。本节只核对 PID737 的这些实际页面和当前生产所有者，不能扩展为全部系统产品已通过。

## 原件与复核位置

原件目录前缀均为 `.analysis/inputs/frontend/`，区间为原始 UTF-8 半开字节区间；各文件读取时核对获取收据的长度和 SHA-256。

- PID737 [获取收据](../../../.analysis/inputs/frontend/system-737-current/acquisition.json) 和 UI `system-737-current/raw/apps.razer.com/synapse/products/737/ui/static/js/8923.a0bbaf75.chunk.js`：Battery 控件 `[166341,168988)`；刷新率 `[169458,175054)`、连接 `[175060,175792)`；native mode `[176829,178465)`、连接 `[178471,178906)`；ICC `[179457,184847)`、连接 `[184853,185376)`；报告 `[185406,186633)`、连接 `[186639,186867)`；外接显示 `[186884,187516)`；GPU 路由 `[190057,192695)`、连接 `[192701,192863)`；完整 Display `[192864,193424)`。本批沿完整 JS 的词法绑定确认这些实际所有者，并核对获取收据、入口和 manifest 的字节身份，读取前后未变。
- 同 source set 的 `static/css/7838.8c72bcaa.chunk.css`：标题与正文完整规则 `[6101,6513)`；面板模式行／tile／active／hover `[13562,13958)`；override／说明／快捷键 `[25533,25999)`。`1709.83010020.chunk.css` 的 Battery 下拉后载规则 `[63926,64052)`，`main.e381c1df.css` 的禁用 slider 规则 `[43222,43406)`。这些区间记录完整规则，不能从单条属性推断所有产品或所有状态一致。
- PID737 中间件 `system-mw-737-current/raw/apps.razer.com/synapse/products/737/mw/6737.85397ce157d4cca228ee.js`：刷新率 maker `[876558,877904)`、电池策略 `[877904,879607)`；local dimming maker `[881748,882891)`；native mode maker `[882891,883735)`；ICC maker `[887437,888210)`、恢复 `[888948,889683)`；GPU mode maker `[889683,890509)`、重启 maker `[890509,891235)`。
- 同一中间件：刷新率 runner `[1126626,1127845)`；local dimming runner `[1130380,1131734)`；native mode runner `[1132683,1133928)`；ICC runner `[1142277,1143729)`、恢复 `[1146626,1147673)`；GPU mode runner `[1148501,1149619)`、重启 runner `[1150528,1151624)`；Battery maker `[916474,917563)`、Fn+B `[917563,919059)`、override maker `[919059,920017)`；Battery runner `[1222575,1223973)`、override runner `[1225006,1225666)`；PowerModeControl helper `[1267994,1268238)`。初始化 getter 调用从字节 151871 开始，本地恢复分支从字节 191766 开始。
- 同产品 `rzDevice25LedMatrixBlade2.f3d8428364673cd4283f.js`：构造器／回调与筛选 `[1301,4887)`；`SetScreenRefreshRate` wrapper `[11901,12257)`。`rzDevice25.1042baf62cda9ce0c1bc.js`：PowerModeControl `[141662,141833)`，VGA setter `[143915,144005)`，充电 getter/setter `[258365,258562)`。这些 JS wrapper 仍需与底层 native 服务／平台实现分层阅读。

本页只记录静态源码语义，未执行厂商 JavaScript、DLL、主程序或重启操作。底层二进制语义、Rust 独立实现、页面接线和真实运行验收不由此页自动判定完成。

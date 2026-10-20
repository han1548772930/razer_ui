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

中间件初始化先查询支持的刷新率和当前刷新率，排序后发布列表与当前值，另查询 display mode、分辨率、HDR 和 SKU 硬件配置。Blade2 Native 的 `EnumScreenRefreshRates` 路径去除重复值，并将相差 1 Hz 的显示模式合并到较大的值；读取当前值时也按已过滤列表转换相差 1 Hz 的返回值。`onDisplayChange` 由 native bridge 回调转换为 `eventEmitter` 事件，因此 OS 显示变化与点击动作有共同的后续刷新来源。

maker `gd` 在允许保存且来源不是设备事件时更新设备级 `screenRefreshRate` 和版本，先广播本地 completed，再排队 runner `je`。runner 调用 `setScreenRefreshRate`，检查取消，更新任务内存缓存，发送 `MW_SET_SCREEN_REFRESH_RATE_TO_UI`；其本体没有写后 GET，也没有核对 setter 返回的布尔值。Blade2 wrapper 实际向 `blade2NativeAction` 发送 `SetScreenRefreshRate`，参数为一个刷新率数值；wrapper 返回 false 与 runner 抛异常是不同路径。

“电池时降低刷新率”保存在当前配置档 `isBatteryModeScreenRefreshRate`。电池供电时更改开关，maker `fd` 立即排入另一个 `ON_SET_SCREEN_REFRESH_RATE`：启用取产品 `batteryModeScreenRefreshRate`，缺省为 60 Hz；关闭取设备级保存的接电刷新率，并显示通知。启用且当前在电池上时，UI 禁用手动刷新率卡片。插拔电源也按同一策略决定恢复值。

## Native 面板模式与 GPU 路由

native 模式卡片保留 `nativeDisplayMode` 草稿和 `showAlert`。点击模式只打开确认；Cancel 关闭并从 props 恢复原模式；确认后才发送 `setNativeDisplayMode`。未由产品提供模式表的共用卡片默认是 3840×2400 / 120 Hz 与 1920×1200 / 240 Hz；711／737 由页面覆盖为 240／440 Hz。runner `Xe` 先 `setMiniLEDPanelResolution`，再 `restartSystem`，随后结算；这条链没有在重启前另读面板模式。

GPU 路由卡片也保留 `mode`、`previousMode`、`restartRequired` 草稿。非 DDS 机型只有 `supportedFlag=SWITCH` 才显示可切换 UI；DDS 分支按当前 mode 显示不同说明，并可向主机请求 NVIDIA 控制面板。Cancel 恢复先前草稿；Restart 发 `restartSystem(mode)`，后续任务分别设置 VGA configuration 和执行重启。GPU setter 使用 `setVGAConfigureSetting`；单独重启 runner 先安排 5 秒 timer，再报告任务完成，不能把该 completed 当成系统已经重启并读回新 GPU 模式。

这些操作只描述原版动作链。本次分析未执行重启、显卡切换或 native 面板写入。

## HDR、local dimming 与 ICC

local dimming 是否有 UI、产品是否走该功能、当前 Windows HDR 状态是三种条件。新系列 10／11／12 的 local dimming runner 通过 PowerModeControl 位读改写；其他系列调用 `setBladeMonitorWindowsHDRMode`。初始化也有不同系列的同步条件，不能将它们统一为“打开页面即写同一个开关”。

ICC 卡片用文件名匹配实际 `colorProfilesList`，提供 Native、Display P3、sRGB、BT.709、Adobe RGB 和 Custom 的产品列表。系统未安装的预置文件和正在加载时的其他选择会禁用；Custom 只从非预置文件中选择。HDR 开启时显示专用说明；缺少预置配置时展示恢复入口；颜色配置变更从中间件更新列表和选中项。

颜色 setter runner `ot` 区分两种路径：若文件名属于运行时的 presetColorProfileNameList，则转换为设备显示模式索引并执行 `colorProfileSetDisplayMode`，必要时包围 CRT 优化处理；其他文件直接调用 `setColorManagementDefaultICCProfile(fileName)`。恢复 runner 走独立 `H3(false)` 链。UI 的 Windows 颜色管理入口启动本机 `colorcpl.exe`，外接显示设置入口打开 Windows Display settings；它们不写某份本地 JSON 冒充系统设置。

## 电池限制、读取与一次性充满

当前 Battery 控件的阈值选项为 50、55、60、65、70、75、80%，滑块步长 5%。开关 `isEnabled` 与阈值 `value` 分开；有一次性充满能力的挂载另有 `batteryChargingOverrideEnabled`。开启一次性充满时阈值编辑禁用，限制开关关闭时一次性充满不可操作。Fn+B 只在支持的挂载显示。

系统启动初始化会读取 `getBatteryChargeLimiterControl`，向 UI 发布真实返回的 enabled/value；一次性充满字段在这一处实际取自已有本地配置。该 getter 捕获错误后返回 `{isEnabled:false,value:0,batteryChargingOverrideEnabled:false}`，这是源码的失败回退行为，不能据此称硬件已关闭限制或返回了 0%。

本地恢复路径另有明确门控：存在 `getBatteryOptimizerFromFW.yes` 时跳过缓存恢复；系列 12 也跳过该恢复写入。其余产品会发布缓存状态并排入 `ON_SET_BATTERY_OPTIMIZER`。初始化读取与缓存恢复不可合并成一种规则。

UI 更改阈值或开关发送完整 `{isEnabled,value,batteryChargingOverrideEnabled}`。maker `Qd` 更新设备级持久化和版本，广播本地完成后排队；runner `Oi` 调用真实 `setBatteryChargeLimiterControl(isEnabled,value)`，再按产品能力设置 PowerModeControl 的一次性 override 位。单独点击一次性充满走 `Yd`／`bi`，只更新该位。

PowerModeControl helper `Re` 先读取现有 cpuBoost、localDimming、maxFanSpeedMode、oneTimeOverride，替换目标字段后把四者 OR 合并并写回。其 catch 只记录错误而不重新抛出；因此外层 runner 的成功结算并不保证 helper 在异常路径中成功写入。Fn+B 的 `Vd` 只在充电限制已启用时切换 override，先更新本地配置／页面并显示通知，再提交对应设备任务。上述路径没有统一的写后 GET。

## 错误、取消与退出

刷新率、GPU、ICC、充电限制和 native mode 各自具有任务队列、版本、maker 与 runner 错误通道。多数 runner 在实际调用后检查 abort 并拒绝旧任务；setter 抛异常、helper 吞掉异常、setter 返回 false 分别保留其实际区别。组件草稿的 Cancel 只撤销还未提交的选择；不等同于设备回滚。旧式页面的 focus 监听在卸载时显式移除；OS/native 显示监听属于设备／中间件生命周期。

## 原件与复核位置

原件目录前缀均为 `.analysis/inputs/frontend/`，区间为原始 UTF-8 半开字节区间；各文件读取时核对获取收据的长度和 SHA-256。

- PID737 UI `system-737-current/raw/apps.razer.com/synapse/products/737/ui/static/js/8923.a0bbaf75.chunk.js`：Battery 控件 `[166341,168988)`；刷新率 `[169458,175054)`；native mode `[176829,178465)`；ICC `[179457,184847)`；外接显示 `[186884,187516)`；GPU 路由 `[190057,192695)`；完整 Display `[192864,193424)`。
- PID737 中间件 `system-mw-737-current/raw/apps.razer.com/synapse/products/737/mw/6737.85397ce157d4cca228ee.js`：刷新率 maker `[876558,877904)`、电池策略 `[877904,879607)`；local dimming maker `[881748,882891)`；native mode maker `[882891,883735)`；ICC maker `[887437,888210)`、恢复 `[888948,889683)`；GPU mode maker `[889683,890509)`、重启 maker `[890509,891235)`。
- 同一中间件：刷新率 runner `[1126626,1127845)`；local dimming runner `[1130380,1131734)`；native mode runner `[1132683,1133928)`；ICC runner `[1142277,1143729)`、恢复 `[1146626,1147673)`；GPU mode runner `[1148501,1149619)`、重启 runner `[1150528,1151624)`；Battery maker `[916474,917563)`、Fn+B `[917563,919059)`、override maker `[919059,920017)`；Battery runner `[1222575,1223973)`、override runner `[1225006,1225666)`；PowerModeControl helper `[1267994,1268238)`。初始化 getter 调用从字节 151871 开始，本地恢复分支从字节 191766 开始。
- 同产品 `rzDevice25LedMatrixBlade2.f3d8428364673cd4283f.js`：构造器／回调与筛选 `[1301,4887)`；`SetScreenRefreshRate` wrapper `[11901,12257)`。`rzDevice25.1042baf62cda9ce0c1bc.js`：PowerModeControl `[141662,141833)`，VGA setter `[143915,144005)`，充电 getter/setter `[258365,258562)`。这些 JS wrapper 仍需与底层 native 服务／平台实现分层阅读。

本页只记录静态源码语义，未执行厂商 JavaScript、DLL、主程序或重启操作。底层二进制语义、Rust 独立实现、页面接线和真实运行验收不由此页自动判定完成。

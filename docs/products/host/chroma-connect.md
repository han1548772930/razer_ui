# Chroma Connect：本机广播设备与控制权

Connect 显示本机 Chroma 广播服务登记的设备项，为每项及全局设置 enabled，并把 Quick 或 Studio Advanced 灯效交给本地 Lighting Engine。真实服务连接、服务启用、单项 enable／running、本机保存和 UI 乐观状态分别存在。本页只展开本机设备、服务、窗口和存储；账号、在线目录及在线获取流程不在范围内。

## 当前入口和两列页面

当前顶层 Provider 下挂错误边界、版本节点、页面路由和观察组件；普通路由实际 lazy 挂模块 9073，Help 另挂模块 5388。历史 reducer 虽有 apps／devices 元数据，当前路由没有据此挂两个标签页，不能把共享代码的旧页面状态当成现有导航。

9073 以 `allAppsInfo.length` 决定两列内容或空态。这里的数组来自广播设备服务，名称中含 Apps 不表示它是在线应用目录。非空时左列显示总开关和各设备项，右列显示 Lighting、Quick／Advanced 及屏幕关闭时熄灯。

| 区域 | 当前呈现与操作 |
| --- | --- |
| 设备列表 | 保留服务顺序；图标、title、单项 enable、running 状态及 tooltip |
| 总开关 | 300 ms debounce；关闭时整组交互灰态 |
| 单项开关 | 按稳定程序名 `name` 发送；300 ms debounce，列表 render key 使用服务 index |
| 广播状态 | enabled 且 running 时显示广播标记；未 running 时有原版提示，不能推断设备已断开 |
| Lighting | 总开状态控制交互；另受当前 Chroma App 争用状态限制 |
| 空态 | 当前未取得可列设备项时显示独立空态图及本机说明 |

页面 body 最大宽度 1240 px，两列各 600 px。卡片背景 `#111`，padding 为 30 px 40 px、圆角 5 px、间距 20 px。列表行 padding 为 5 px 20 px 5 px 5 px，图标 30×30，状态图 20×20；空态图为 800×300，文本内容宽 600 px。加载、未初始化、断开与真实空列表在数据模型中必须区别，不能把原件空态条件直接当作失败结果处理规范。

## 初始化和状态来源

Broadcast reducer 的初始值为 `isConnected:false,isEnabled:false,allAppsInfo:[],appList:[],sdkVersion:"",isSwitchOffLighting:false`。它会从本机 `chromaConnectProfiles.isEnabled` 恢复开关显示；这不是一次服务 IsEnabled 的确认。

页面上下文另读 `chromaConnectAppConfig`，初始 searchText 为空、filterType=0、isDisabled=true、isCompatible=true。当前设备页没有消费这些字段来生成额外标签或筛选 UI，不能凭上下文对象补一套界面。

观察组件在 installData 到达后初始化：

1. 先向 UI 发布 connected；这是页面动作，不是 WebSocket open／Init 成功证据。
2. 仅在本机 profile 对象存在且 `isEnabled` 字段缺失时请求总开；不能推广为读取失败后自动开启。
3. 先读 devices memory，再读 apps memory；后者只用于本机程序控制权。
4. 后续 devices 事件要求 initialized 才消费 enabled／allAppsInfo；apps 事件以真实 enabled 与 appRunning 共同投影争用。

共享 MemoryStorageEvent owner 先 register，接宿主 `memoryStorageEvent` 或对应 window 事件，beforeunload 时 unregister。get helper 解析宿主返回数组的第一项 value，不会把未取得值自动补成真实设备列表。

本机效果和开关保存于 `chromaConnectProfiles`，配置档摘要观察 `synapseChromaStudioUsers`。本机配置拥有独立 getLocalDB／setLocalDB；保存 helper 把 quickEffects 身份和效果参数整理后写入当前本机文档。它们负责主机设置，不构成广播设备硬件状态。

## Quick 的实际四个效果

当前下拉由模块 8193 的四项数组构造，顺序为 Audio Meter、Breathing、Spectrum、Static。共享 render 中另有 Reactive、Ripple、Starlight、Wave、Wheel、Ambient 分支，当前数组并未提供这些项；不能因此扩增可选效果。

| 效果 | 当前实际参数及提交 |
| --- | --- |
| Audio Meter | colorBoost 0.25…4、步长 0.25，默认 1；文本合法性与取整在组件处理，变更后提交 |
| Breathing | 两个颜色及 Random Color；随机时禁用手动色；初始绿色与 no-color、随机关闭 |
| Spectrum | 无附加参数；本机默认所选效果，当前 Quick effectId 为 3 |
| Static | 一个颜色及本机自定义色；隐藏 no-color 选项，初始绿色 |

Quick effectId 与 Studio 效果 ID 属于不同枚举，不能因名称相同而混用。换效果通过 effect identity 取得参数并调用 setSelectedEffect，不使用下拉下标持久化身份。颜色提交更新本机配置并交 runner，页面没有独立 Save／Apply 按钮。

同步按钮查询当前 DEVICE_RUNTIME_DATA；当前代码排除同时满足 setupStatus=ready 且 chargingStatus=off 的条目，再判断余下是否有 Chroma 设备。它不是只看产品能力表，也不是仅保留 ready 设备的筛选。按钮将 timerTick、selectedEffectId、effectSettings 写 window storage，由其他设备消费者处理；1200 ms 旋转仅为 UI 反馈。收到同一同步记录时再按 effectId 恢复本页选择。

当前 Quick runner 对实际四项分别转参：Static 转整数颜色；Breathing 将 no-color／空色转换为具体颜色，并按随机／第二颜色条件使用 7000 或 14000 ms；Audio Meter 构造固定色标及 AutoBoost=0、boost、Decay=1000；Spectrum 传空参数。runner 另有 Wave／Reactive 分支，但没有覆盖全部共享 render 分支，不应宣称其支持所有代码中出现的效果。

真实提交经 LightingManagerChannel 的 `applyCustomQuickEffect`，携带 appName、productId、实例 deviceContainerId、效果 enum 和参数，最终由 LE 的对应 action 消费。channel 的 catch 只记录异常，不能把异步函数结束称为真实输出确认。Quick 到硬件帧的公共链见 [系统灯光链](../system/customize-lighting.md#quickadvanced-到真实灯光帧)。

## Advanced 和本机窗口

Advanced 使用共享 Studio 配置档列表，选择时取稳定 profile ID，写入本机 `synapseChromaStudioUsers.currentProfile`，同时更新本页所选项。配置档显示列表按名称大写后的比较排序，服务设备列表仍保留服务顺序，两者不能混成一种排序规则。

切换 Quick／Advanced 先改变本页 isAdvanced 与本机 Chroma enabled，再依据本机安装模块判定是否启用 Studio 引擎；写入共享 enabled 的条件为 `isAdvanced && studioInstalled`。没有安装模块与已有本机配置档是两个状态。Studio 按钮先查询窗口，已有窗口则激活，否则创建本机窗口。完整 Studio 草稿／保存／设备帧见 [chroma-studio.md](chroma-studio.md)。

争用提示读取共享 apps memory 的 appRunning 与 enabled；点击进入本机 Chroma Apps 控制窗口。程序 title 用于提示，稳定 name 才用于服务命令。不能只因某程序被列出，就判断当前已获得控制权。

## UI 请求到真实本地服务

页面 helper 向两个独立 memory action 通道投递，结果通道也分别保存：

| 通道 | 请求／结果 |
| --- | --- |
| devices | 读取当前广播设备；`setEnabled`、`setAppEnabled {name,enable}` |
| apps | 读取程序控制状态；`setEnabled`、`setAppEnabled`、`setAppList {numApp,appList}`，另有本机 Dock／Smart Lighting 命令 |

当前 Connect 页面只挂设备页；apps 请求能力仍是 LE 与本机控制界面的共享契约，不能把这张表当作当前页面多出一组按钮。

真实消费者在 Lighting Engine：

```text
Connect 开关／Quick／Advanced
→ memory action 或共享本机配置
→ LE 对应消费者
→ 本地广播／应用 WebSocket，或本机灯光引擎
→ 服务返回、事件及设备帧
→ 更新共享 memory
→ Connect observer 刷新实际状态
```

devices 服务为 `ws://127.0.0.1:1337/razer/rzchromabroadcastmanager`；apps 服务为 `ws://127.0.0.1:1337/razer/rzapichromasdk`。这是本地服务边界，属于离线本机能力。

两者发送 `{func,param,token:{id,param}}`，请求 ID 单调生成，以 token.id 匹配回复。Init 使用 result，其余使用 return；每次等待有 5 秒超时，并移除自己的 message listener、timer 和 pending 项。未初始化、socket 不可用、取消或 socket.send 异常有 resolve undefined 分支；undefined 不是真实服务成功。

socket open 先发布 connected，再发 Init(appid)，只有返回 0 才进入 initialized。初始化期间比较 socket 身份，旧连接结果被放弃。devices 对 1247 有最多 5 次、500 ms 的 unInit／disconnect／重试分支，其他失败进入既定重连；不能用本机 profile 填补服务读取失败。

## 总开、单项和失败边界

devices 初始化真实读取 SDK version、IsEnabled 及设备列表。当前官方代码还会比较明确布尔型的本机 `chromaConnectProfiles.isEnabled`，不同才经真实 Enable 恢复；这是已存在的具体主机服务设置恢复链，不是“无配置即恢复默认”。

总开执行 Enable 后再 IsEnabled 查询；单项执行 Enable(name)，后续事件触发列表更新，没有统一的单项同步读回。服务事件 type 1／3／4 请求列表刷新，type 2 更新 enabled。allAppsInfo 的 name、index、title、enable、running 各有用途，不能从其中一个字段推断其他字段。

当前官方 IsEnabled 会捕获请求异常并把未取得值强转为 false；有连接时还可能发布这个布尔值。总开外层部分事件又按请求值 emit。页面 helper 也吞掉 memory 投递异常，使上层 await 后仍更新 reducer，并增删虚拟设备。因此以下层级必须分别建模：投递结果、服务返回、后续 IsEnabled／列表事件、本机保存与 UI 乐观请求。不得把上述失败 false 或请求值作为本项目已确认服务状态。

屏幕熄灯 checkbox 当前只改变 reducer；显示电源事件再请求 brightness=0／100。实际 `broadcastAppSetBrightness` 对 `{globalVar:…}` 对象调用 forEach，异常被 catch；本页不能据 checkbox 已勾选宣称真实帧已关闭，也不能虚构该路径的读回确认。

## 虚拟 ChromaLink 与真实广播帧

每个 enable 的服务项建立一个虚拟 rzDeviceChromalink，productId=770、instanceId=服务 index、regionId=index+`0x80000000`。它进入公共设备 registry；列表变化按实例增、删、更新，LE 再注入真实 Broadcast manager。虚拟对象不是一台已完成 HID 读取的物理设备。

引擎帧按 numRow／numCol 取出每行 RGB；`setChromaFrame` 将每三个字节打包为 `R | G<<8 | B<<16`，复制首色到数组首位，再追加 1。brightness=0 时先改成五个零，仍经过追加 1。regionId 高位映射到对应 app index，再调用 `SetBroadcastEffectOnApp(index,colors,false)`；没有 regionId 则走全局广播。

当前 setBrightness 只改变虚拟对象亮度并立即返回 SUCCESS；真实输出要等后续 frame 消费。frame writer 逐行 catch，setChromaFrame 在 await manager 后构造 SUCCESS；这些不是设备逐灯珠读回。官方 wrapper 与 DLL 仅作协议／语义证据，本项目必须以 Rust 独立实现本地协议及完整消费者，不能把官方业务 DLL 作为生产后端。

## 本机应用争用、优先级与停止

apps 服务合并同时发起的 GetAllAppsInfo；请求失败或重复 name 时最多 3 次、间隔 300 ms，仍重复时保留每名首项，并由真实结果更新 appRunning。程序列表顺序是稳定 name 列表，SetAppList 传 numApp 与 appList，不按显示 title 重排。

自动优先级观察本机前台 exe_name：命中已 enable 项时把它移到首位并记住原位置，下次前台改变先恢复上次临时移动。手工 SetAppList 消费者删除这份临时记录，避免稍后恢复覆盖用户顺序。这是共享本机控制权语义，当前 Connect 设备页并未提供拖动程序优先级界面。

disconnect 取消 pending sends、停止对应监听并发布真实断开／未初始化 memory；非强制断开按服务自己的表安排重连。系统恢复先关闭旧 socket、取消等待并 reset，再 connect。beforeunload 另负责解绑存储、系统恢复及前台事件、unInit／disconnect；不能将一个 connected 布尔值复用于每个生命周期阶段。

页面层仍有不完整清理边界：Quick 的 windowstorageevent 未见配对卸载；观察组件 componentWillUnmount 只移除 storage，匿名 memory／宿主回调及 interval 没有证明全部解除。Advanced 和模式组件自身 storage 监听则有配对 off。Rust 页面 owner 应持有这些任务、订阅、虚拟设备与连接代次，离开／切换时停止在途工作，不能让旧结果覆盖新连接。

## 当前 Rust 页面与服务的对应边界

当前实现必须区分 Chroma Dashboard、产品 `displayMode=chromaApp` 弹层和本页 Broadcast 服务。它们虽然都带 Chroma 名称，但实际根、设备身份、Quick 列表和结果消费者不同；不能以其中一项已存在替代另外两项。

| 边界 | 当前生产所有者与实际行为 | 尚未对应的页面／服务链 |
| --- | --- | --- |
| Connect 模块入口 | [ChromaPage::modules](../../../crates/razer-app-pages/src/chroma/chroma_page.rs) 固定列出 Connect，按钮显示 Install 且禁用，点击只为 Studio 分发事件；[app_picker_host.rs](../../../crates/razer-shell/src/shell/app_picker_host.rs) 的可启动模块未含 Connect，强制发送其 Open 也进入通用“窗口服务尚未连接”分支 | 未找到本页 `9073` 的生产挂载：总开／设备行、running 提示、两列 Lighting、Quick／Advanced、熄灯和真实空态均不能由这个禁用入口算作完成。本机功能须独立实现并真实接线，不能永久保留安装占位 |
| Dashboard 与产品弹层 | [chroma_page.rs](../../../crates/razer-app-pages/src/chroma/chroma_page.rs) 挂 Dashboard／Modules／Apps；[chroma_product.rs](../../../crates/razer-pages/src/features/chroma/chroma_product.rs) 绑定实际产品 workspace，打开它自己的 Lighting 内容并保留 popup owner／释放 | 产品弹层和 Dashboard 的七个 preset 不是本页 Audio Meter／Breathing／Spectrum／Static 四项。不能借用产品设备列表代替 Broadcast `allAppsInfo`，也不能用另一个 Apps 页推断本页存在旧式 Apps／Devices 标签 |
| 已有本地 WebSocket 能力 | [Session](../../../crates/razer-service/src/lighting/chroma_connect.rs) 独立 Rust 连接 `rzapichromasdk`，由 [receiver_lighting_owner.rs](../../../crates/razer-shell/src/shell/receiver/receiver_lighting_owner.rs) 的后台 owner 使用；真实实现 Init、SetSynapseStatus、SetAttribute、重连、token／参数核对和退出 UnInit／socket 清理 | 这是底座属性桥接，不是本页 Broadcast owner。当前该 Session 没有设备服务 `rzchromabroadcastmanager` 的列表／IsEnabled／Enable／帧调用，也没有供本页消费的 apps 列表／running／手动顺序与前台优先级发布者。已有局部真实服务链应保留，不能笼统写成整个 Chroma 服务均未实现 |
| 退出与结果 | 底座 owner 退出先停止新请求，等待工作释放，随后调用共享 Session::finish，错误保留诊断；构造 Session 本身不联网 | 该清理只证明对应底座服务所有者存在，不能证明 Connect 页 debounce、memory／OS 事件、虚拟设备 registry、持续帧和 Quick／Advanced 任务已完整取消。连接、Init、属性返回和实际灯光输出分别验收 |

本批直接复核了当前路由、`9073` 导出、四项实际数组、总开／设备行、Quick、Advanced、模式和空态，以及现行 Shell／Session 调用方。完整 `73.78b50033.chunk.js` 中 `et` 单行绑定为 `[69875,70896)`、`tt` 总开绑定为 `[70897,71658)`、`st` 两列根为 `[71659,71726)`，最终 default `it` 在 `[71727,71920)` 才按列表长度选两列或空态；合并的上下文区间不能冒充一个完整函数。当前输入、CSS 与获取收据按字节身份核对，未执行厂商 JS、socket、DLL 或窗口。

后续自动化测试要从这个真实根验证服务非空／空、读取未完成／失败、enabled／running、300 ms debounce、争用禁用、四个效果参数、Advanced profile 身份和无隐式 Apply，再分别验证真实 Broadcast／Apps 请求及 memory 结果、帧消费者、旧连接拒绝、显示电源、取消与释放。纯模拟的请求／回应顺序不能替代官方实际服务／DLL 对照；没有服务或广播设备时仍应测试状态和传输错误分支，真实输出保持未验证。

## Broadcast DLL 公开包装层的实际边界

对当前 `RzChromaBroadcastManager64.dll` 的捕获（ImageBase `0x180000000`，输入 SHA-256 `555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51`）又核了公开包装层的完整函数体。它们能确定调用前后的门禁和错误码，不能把内部 `sub_*` 当作已经核实的设备传输或读回：

| 导出 | 已核实的包装行为 | 原始函数与代码区间 |
| --- | --- | --- |
| `InitGlobalBroadcastData` | 全局 `Block` 已存在返回 `1247`；否则分配并清零 `0x130` 字节，调用内部构造，非空返回 `0`，分配/构造失败返回 `5023`。 | [`26b40.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26b40.json)，RVA `0x26b40`–`0x26ba2`，98 B，chunk SHA-256 `cc3817cde396b0c4450f6bf498b9c7d77cbf98a5afbe01018335a89301d72e9a` |
| `UnInitGlobalBroadcastData` / `RefreshAppList` | 未初始化分别返回 `5023`；释放路径先调用内部清理、`free` 全局对象并置空。刷新只在 `Block` 存在时转发内部刷新，未初始化不产生列表。 | [`26bb0.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26bb0.json) 与 [`26c00.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26c00.json)，RVA `0x26bb0`/`0x26c00` |
| `SetBroadcastEffect` / `SetKeyboardEffect` | 先检查 `Block` 和两种允许的对象 ID 布局；有效分支把默认结果设为 `1627`，将效果数据提交给 PPL task collection 并 `RunAndWait` 后返回内部结果。ID 不匹配时结果改为 `5` 并记录 “Access Denied due to Invalid ID”。这只证明等待了本地任务，不证明 HID 写入、设备 ACK 或读回。 | [`25840.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/25840.json)（RVA `0x25840`–`0x25afc`，700 B）与 [`25b50.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/25b50.json)（RVA `0x25b50`–`0x25ea5`，853 B） |
| `SetBroadcastEffectOnApp` / `SetKeyboardEffectOnApp` | `Block` 为空返回 `5023`；否则复制固定大小的效果结构，按传入 app index 转给内部 `sub_18000B110`／`sub_18000B9B0`。包装层没有对 index 做成功确认，也没有暴露读回。 | [`25b00.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/25b00.json)（66 B）与 [`25eb0.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/25eb0.json)（166 B） |
| `Enable` / `IsEnabled` | `Enable` 仅做全局存在检查后转发名称和开关值；未初始化返回 `5023`。`IsEnabled` 未初始化直接返回 `0`，因此 `false` 既可能是未初始化也可能是内部查询结果。 | [`25f60.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/25f60.json) 与 [`25f80.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/25f80.json) |
| `GetNumApps` / `GetApp*` | 未初始化的数量返回 `0xffffffff`；`GetApp`、标题、路径、图标等 getter 返回 `5023`。这些包装不把失败转换为空字符串或空列表。 | [`25fc0.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/25fc0.json)、[`25fe0.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/25fe0.json)、[`26000.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26000.json)、[`26020.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26020.json)、[`26040.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26040.json) |
| `GetAppIndex` / `GetDeviceSupported` | 只接受第一个对象 ID 布局；全局/ID 无效返回 `0xffffffff`，空名称返回 `0xfffffffe`。有效名称拼接 `HKLM\\Software\\Razer\\ChromaBroadcast\\Apps\\<SubBlock>\\<name>`，读取 `Index` 或 `DeviceSupported` 的 DWORD；开键、查询或值缺失均保留 `0xffffffff`，句柄关闭在所有分支执行。 | [`26060.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26060.json) 与 [`26240.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26240.json)，两者各 465 B |

因此 Rust 侧必须把“未初始化”“无效 ID”“注册表缺项”“PPL 已等待”和“设备实际确认”建成不同状态。当前捕获没有证明 `sub_18000B110`、`sub_18000B9B0`、列表内部或真实服务端是否完成传输，也没有运行时 DLL／服务／设备对照；上述公开层结果仍属于静态已核、功能结果未验证。

### 包装层下的注册表和 app-index 分支

公开入口下方的已捕获函数还给出了本机 app 表的可复现边界：

- `sub_18000C2D0`（[`c2d0.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/c2d0.json)）只接受第一种对象 ID，空名称返回 `87`，向 `HKLM\\Software\\Razer\\ChromaBroadcast\\Apps`（管理器自身）或其 `<SubBlock>\\<name>` 子键写入 DWORD `Enable`；写入成功且内部状态为 4 时，还会 `OpenEventW`、`PulseEvent` 后关闭句柄。返回值是注册表写入结果，事件脉冲不等于设备确认。
- `sub_18000C500`（[`c500.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/c500.json)）用同一路径读取 `Enable`，无效 ID、空名称、开键失败、值缺失和读取失败都返回 `0`；因此 `IsEnabled=false` 不能区分未注册、读取失败和明确关闭。
- `sub_18000C910`／`sub_18000CB20`（[`c910.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/c910.json)、[`cb20.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/cb20.json)）枚举根键，只有名称中含 `.exe` 且前缀非空的子键计入 app 数量；按枚举索引返回子键名的 `CoTaskMemAlloc` 副本，找不到或开键失败保持 `1168`。顺序来自注册表枚举，不是显示 title 排序。
- `sub_18000CD70`／`sub_18000CF90`（[`cd70.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/cd70.json)、[`cf90.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/cf90.json)）从 app 子键读取 `Title`／`Path`，先询问长度、再分配并复制，最后关闭注册表句柄；无效 ID 为 `5`，空名称为 `87`，其余错误保留 Win32 注册表状态码。
- `sub_18000E7C0`（[`e7c0.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/e7c0.json)）刷新时再次枚举根键，读取 `Path` 与 `Index`，把内部判断为 `3` 或 `1154` 的条目标记并用 `RegDeleteKeyExW(..., 0x200, ...)` 删除；它返回最后的删除状态（没有删除时保留初始 `0xffffffff`），不能被当作“当前设备列表已同步”布尔值。
- `sub_18000B110`／`sub_18000B9B0`（[`b110.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/b110.json)、[`b9b0.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/b9b0.json)）对 app index 先拒绝最高位为 1 的值（`1413`），空 map 返回 `4306`，FNV-1a 风格桶查找不到 index 返回 `1168`；找到后才创建 continuation task。它们没有在该层暴露 frame ACK、读回或释放成功，Rust 不能把 task 完成直接映射成灯效已写入。

这些分支说明本机注册表 app 目录、前台争用列表和实际 Broadcast 帧是三个不同事实源。实现和测试必须分别覆盖注册表不存在／缺字段、索引越界、删除旧项、任务取消及真实服务返回；没有设备时只能用受证据约束的模拟应答，不能用 `IsEnabled=false` 或 task 返回码伪造硬件状态。

## 原件与核对边界

获取、部署身份及字节 SHA-256 见 [Connect 收据](../../../.analysis/inputs/frontend/synapse-chroma-connect-current/acquisition.json) 与 [Lighting Engine 收据](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/acquisition.json)。JS 为原始 UTF-8 半开字节区间。

| 所有者 | 当前原件／区间 |
| --- | --- |
| Provider、真实路由、观察组件 | [915.36ae8e5f.chunk.js](../../../.analysis/inputs/frontend/synapse-chroma-connect-current/raw/apps.razer.com/synapse/chroma-connect/static/js/915.36ae8e5f.chunk.js)：模块 7915 `[548,68910)`；路由 `[61704,62212)`、MemoryStorageEvent `[62439,63454)`、观察者 `[63800,67290)` |
| memory helper、开关动作、虚拟设备 | 同上，模块 3913 `[72916,77454)`、4094 `[78842,80193)`、782 `[80194,92964)`；虚拟帧 `[86390,88305)`、registry `[88432,89123)`、列表差异／生成 `[92039,92631)` |
| 设备页与实际 Quick 数组 | [73.78b50033.chunk.js](../../../.analysis/inputs/frontend/synapse-chroma-connect-current/raw/apps.razer.com/synapse/chroma-connect/static/js/73.78b50033.chunk.js)：模块 9073 `[681,71921)`、8193 `[547,680)` |
| 总开／行、Quick／Advanced／模式、空态 | 同上，`[69875,71726)`、`[58550,62130)`、`[62564,66018)`、`[66291,68538)`、`[1209,2400)` |
| 四项实际参数 UI | 同上，Breathing `[37143,38954)`、Static `[45187,46150)`、Audio `[55241,56522)`；Spectrum 在 Quick render 无附加参数 |
| 初始／持久化状态、Quick runner、LE channel | [767.69a5a160.chunk.js](../../../.analysis/inputs/frontend/synapse-chroma-connect-current/raw/apps.razer.com/synapse/chroma-connect/static/js/767.69a5a160.chunk.js)：模块 8305 `[25988,42417)`、1408 `[16555,18622)`、2300 `[3997,14098)`、2767 `[23178,25987)` |
| 两列／卡片 CSS、行／空态 CSS | [915.e0b8ccfc.chunk.css](../../../.analysis/inputs/frontend/synapse-chroma-connect-current/raw/apps.razer.com/synapse/chroma-connect/static/css/915.e0b8ccfc.chunk.css)、[73.a8feae2b.chunk.css](../../../.analysis/inputs/frontend/synapse-chroma-connect-current/raw/apps.razer.com/synapse/chroma-connect/static/css/73.a8feae2b.chunk.css) |
| Broadcast 服务所有者 | [main.a383ea9e.js](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/raw/apps.razer.com/synapse/lighting-engine/static/js/main.a383ea9e.js)：模块 5976 `[959155,974836)`；连接 `[961588,966087)`、send `[966265,967281)`、init `[967282,969626)` |
| Broadcast 读取／修改／真实 action 消费 | 同上，列表 `[970241,971158)`、本机明确设置恢复 `[971375,971942)`、IsEnabled／Enable `[971943,972941)`、按 app 发帧 `[973143,973366)`、action `[974201,974663)` |
| Apps 服务生命周期／send／init | 同上，模块 7815 `[1213492,1232498)`；connect／disconnect `[1215302,1219067)`、send `[1219246,1220259)`、init／unInit `[1220260,1223014)` |
| Apps 列表／开关／优先级与 action | 同上，列表 `[1223213,1225736)`、开关／顺序 `[1226445,1227501)`、前台优先级 `[1230056,1230826)`、action `[1231620,1232483)` |
| Broadcast 原生身份及捕获索引 | [capture.json](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/capture.json)；公开入口 RVA `0x25760/0x257d0/0x25840/0x25b00/0x25f60/0x25f80/0x25fc0/0x26420` |

Connect 收据中已取得的 243 项原件逐项核过长度与 SHA-256；35 项 unresolved 保持原状。本文静态阅读了列出的 UI、存储和 LE 完整消费者，Broadcast 原生原件仅核身份和捕获，未据导出表宣称内部服务／设备实现已经闭合。

| 完成维度 | 本页能证明的边界 |
| --- | --- |
| 源码获取 | 当前收据及以上原件存在；未恢复的引用保持独立 |
| 逆向语义 | 本页具体 UI、memory、WebSocket 与帧消费者已静态追踪；原生内部仍需继续 |
| Rust 独立实现 | 文档不构成实现完成证据 |
| UI／后端接线 | 文档不构成产品接线完成证据 |
| 静态／模拟验证 | 仅原件身份与文档引用核对；未由本页运行服务模拟测试 |
| 运行时验收 | 未验证；未运行主应用、厂商 JavaScript、安装器或动态调试 |

## Connect Manager／Server 的服务事件、工作线程与释放

这里补核服务模块入口到工作线程和释放路径。两份原件的 Image Base 均为 `0x10000000`，使用 IDA `9.1`／Hex-Rays `9.1.0.250226` 的既有完整捕获；原件、捕获索引中的函数 JSON SHA-256、完整 chunk 原字节与每条指令字节经双线程 Python 独立核对。统计对每个模块的 chunk 区间和指令地址去重，只表示本节所选范围。

| 原件及捕获 | 原件身份 | 本节核对范围 |
| --- | --- | --- |
| `RzChromaConnectManager.dll`；[capture.json](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/capture.json) | `117448 B`；SHA-256 `a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3`；捕获索引 SHA-256 `bc3bf726aa719c565aca07b4bab20c191ba961343938d3c2145e63ce22613126` | `19` 函数、`34` 唯一 chunks、`2197` 条指令、`7062 B`；RVA `f7a0,f840,f8a0,f910,f920,f930,3f90,4050,3ec0,fc38,50f0,42d0,3f60,3f70,3f80,69e0,6c20,6e70,51c0` |
| `RzChromaConnectServer.dll`；[capture.json](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/capture.json) | `591048 B`；SHA-256 `a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5`；捕获索引 SHA-256 `2ce03476a1e68777e86009029defa301a17d57723116a21d2ec6f6f3de7eaed9` | `13` 函数、`32` 唯一 chunks、`2415` 条指令、`8775 B`；RVA `5f3f0,5f480,5f4e0,5f580,5f590,13dd0,1c430,1c560,145a0,1ba40,1ba80,1bac0,1bb00` |

合计 `32` 函数、`66` 唯一 chunks、`4612` 条指令、`15837 B`，所选 chunks 无未列字节。Manager 的三个线程入口 `3f60/3f70/3f80` 和 Server 的 `1ba40` 只有反汇编，不能算作完整伪代码；Manager `42d0` 的伪代码另有栈指针警告，下面有关事件、线程、返回值的结论以反汇编核对。模块级完整捕获分别有 `343/343`、`1427/1427` 个函数，其中仍有 `3`、`2` 项反编译失败；本节没有把这些总数当成语义完成数。

### 导出入口会吞掉内部启动与停止错误

Manager [InitServiceModule `f7a0`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/f7a0.json) 在全局对象已存在时直接返回 `0`；否则经 `f930` 分配 `0xf4 B` 对象，调用虚表第一个槽后按全局对象是否仍存在返回 `0/1`。原件虚表 RVA `0x14d80` 的前两个指针确为 `0x1000f910/0x1000f920`。[`f910`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/f910.json) 调用 `3f90(this+4)`，随后 `xor eax,eax`；[`f920`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/f920.json) 调用 `4050(this+4)` 后同样返回 `0`。[UnInitServiceModule `f840`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/f840.json) 因此会在普通返回路径清空全局指针，调用 `3ec0` 析构并释放对象，不会把内部 `UnInit` 错误透传出来。

Server 的对应模式相同：[InitServiceModule `5f3f0`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/5f3f0.json) 分配 `0x4d0 B` 对象并构造 `this+8`；原件虚表 RVA `0x736b4` 的前两槽为 `0x1005f580/0x1005f590`。[`5f580`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/5f580.json)／[`5f590`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/5f590.json) 分别调用内部 `start/stop` 后固定返回 `0`。[UnInitServiceModule `5f480`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/5f480.json) 随后清空全局对象并析构。导出返回 `0` 只能证明这层包装普通返回，不能证明监听建立、工作线程退出、帧写入或设备确认。

| 服务事件 | Manager [`f8a0`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/f8a0.json) | Server [`5f4e0`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/5f4e0.json) |
| --- | --- | --- |
| `1` | 调用 `3f90` 初始化 BroadcastManager | 清除暂停／停止字节，调用 `1c430` 启动 |
| `2` | 调用 `4050` 停止并卸载模块 | 调用 `1c560` 停止 |
| `3` | 同 `2`，执行完整内部停止 | 仅置暂停／停止字节为 `1`，不调用 `stop` |
| `10` | 在同一对象上先 `4050`，后 `3f90` | 无对应分支 |
| 其它及返回 | 未匹配事件也返回 `0`；上述内部结果均被丢弃 | 未匹配事件也返回 `0`；上述内部结果均被丢弃 |

### Manager 的共享区、事件与停止边界

[`3f90`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/3f90.json) 先调用 `50f0`，忽略其结果，再检查 DLL handle。已有 handle 返回 `1247`；`LoadLibraryW(L"RzChromaBroadcastManager.dll")` 失败保留 `1627`；缺少 `Init` 导出为 `15841`。找到导出后以四个固定 32 位量 `-1592477790,1305303628,1057884053,900499181` 调用，保存其返回值，并无条件将内部标志 `this+236` 置为 `1`。接着调用 RVA `42d0`，但丢弃其返回值；这意味着底层 `Init` 失败仍会尝试创建事件与线程。这里记录的是原版调用形状，不能据此猜测公开 ABI 或把厂商 DLL 接入 Rust 生产路径。

[`42d0`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/42d0.json) 的日志身份为 `CBroadcastManager::InitEvents`；捕获中的 `___std_stacktrace_to_string@16` 名称不能作为函数含义。它创建三个手动复位命名事件 `Global\\{4D006319-9569-4E38-B0DF-811AA2DF115F}`、`Global\\{45C97C2C-2D50-4F30-B50E-AFBB1CE22E93}`、`Global\\{08983A2E-6665-42B2-9492-38D5B1F3340A}`，另创建一个匿名停止事件；三个线程入口分别 `3f60→69e0`、`3f70→6c20`、`3f80→6e70`，handle 位于内部对象 `+4/+32/+64`。成功检查包含三个线程、CL／KB 事件和停止事件，未检查 `+68` 的 monitor 事件；失败结果为 `1627`，调用方不消费。

[`69e0`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/69e0.json)／[`6c20`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/6c20.json) 均等待“数据事件或停止事件”，处理时还检查 `this+76`。CL 从映射偏移 `8+240×slot` 取 `240 B`，映射长度为 `12808 B`；KB 从偏移 `8+2968×slot` 取 `2968 B`，映射长度为 `40088 B`。二者本地 slot 都以 `0..9` 循环，直至追上共享区写索引；分别在临界区内调用 `4870/4c30`，随后等待下一事件。这里补核了消费与停止边界，`4870/4c30` 的全部按 app 分发及后续 DLL／设备应答仍需独立追踪。两个循环退出时只清空当前数据指针，没有关闭它们的线程或事件 handle。

[`50f0`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/50f0.json) 以 `OpenMutexW` 检查 `Global\\{48D72353-C43A-48FB-8AAA-31D9D768E492}`，成功时释放／关闭 mutex 并调用 `51c0` 更新活动 app map；失败清空 map 并返回 `1168`。`51c0` 从 `Software\Razer\ChromaBroadcast\Apps` 的 app 子键读取 `Pid/Index`，再检查带 PID 后缀的命名 mutex，按 index 增删 map；空注册表列表返回 `4306`，最终活动 map 为空返回 `1168`。此处是进程活动目录，不能当成真实设备列表。[`6e70`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/6e70.json) 等待 monitor／停止事件，读取该注册表根键 `Event`；当 `Event=2` 且 Broadcast 标志为 `1` 时先 `Sleep(4000)` 再刷新。停止不能打断这次睡眠。

[`4050`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/4050.json) 先清 `this+236`、置停止事件、将 `this+76` 置 `1`，对 monitor 线程最多等 `5000 ms`，对 CL 线程最多等 `2000 ms`，之后尝试关闭相应 handle。两个 `WaitForSingleObject` 返回值都不检查。该路径没有等待或关闭 `+32` 的 KB 线程，也没有关闭 `+36/+68` 的两个命名事件；已核三个循环也没有代关这些 handle。它清理 app maps 及条目 handle，调用 DLL `UnInit`，无论该结果是否非零仍尝试 `FreeLibrary` 并清空模块字段；缺导出为 `15841`，无模块为 `126`，`FreeLibrary` 失败仅记日志。最后关闭匿名停止事件。[`3ec0`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/3ec0.json) 继续删除临界区、清容器、解除 CL／KB 映射并关闭映射 handle。

因此，原版此处没有提供“所有线程已经退出”的确认。`42d0` 和 `3f90` 也没有重置 `+76`，故事件 `10` 的同对象重启不能仅凭第二次初始化返回值推定恢复完成；新建对象时该字段由构造清零。后续对照测试须分别观测事件、线程、活动目录与输出结果，并记录这些原版行为，不能把包装返回值当作释放或恢复完成。
## Server 的本地消息队列、端点回调和停止分支

本节核对 Server 的输入队列、输出队列、端点订阅、JSON 消息和断开清理。原件 `RzChromaConnectServer.dll` SHA-256 为 `a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5`，大小 `591048 B`，Image Base `0x10000000`；[capture.json](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/capture.json) SHA-256 为 `2ce03476a1e68777e86009029defa301a17d57723116a21d2ec6f6f3de7eaed9`。本节 18 个函数去重后为 `57` 个 chunk、`8535` 条指令、`31414 B`；`.work/chroma-connect-lifecycle-verify.py` 用 `ThreadPoolExecutor(max_workers=3)` 并行校验 Manager 生命周期、Server 生命周期和本节消息集合，逐个核对输入 PE、函数记录 SHA、chunk 原字节和指令字节，结果为 0 个未列原字节。该集合不与上一节的 13 个 Server 生命周期函数重复。

| RVA／函数记录 SHA-256 | 静态用途 |
| --- | --- |
| [14e50](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/14e50.json) `a4f02e04b0a0388fa69953fed2728dc872fecb61b266bd0a02507fd14e8171aa` | process_incoming 输入队列 |
| [16630](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/16630.json) `1138a0c96f0f6caa3a0b674a39ac87dce8d3071e4d55de83fea4dfb18c8b59fb` | process_outgoing 输出队列 |
| [17440](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/17440.json) `3e55bd15e749c28ed415ae060af3ccfc19de79b15c5f6f9b1474a8a76dbc6b78` | process_validate |
| [17ab0](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/17ab0.json) `971483a645d89c248950be3b3b8398b6ff658bf94157c9983c53de0c54598546` | process_subscribe |
| [18650](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/18650.json) `06f3efb9bbb0b850dab2cdb0d23dd3db76078014c122e6483d72d825efc36014` | process_message |
| [196c0](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/196c0.json) `f840745be319080643fd8d99f816f330b1c316c378623979f83dbfd51271590a` | terminate_endpoint |
| [19780](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/19780.json) `a3410bfe0554c083468cddaecb00c783bc38cfc495b3d43a3b041b34be2a9761` | initialize_datafile |
| [19e80](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/19e80.json) `ffdb95260bd3c4cdf398435d3c494527e1c98f4f0bfe6a1ea7ee7bb90cc3c8b9` | download_datafile |
| [1ab00](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1ab00.json) `a30bb0e850bb589cba9ab86a3a75b49d17b3c69252f1db93f9e96c3b0681851c` | ping |
| [1ac80](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1ac80.json) `815609796ba41e449384ffc5a64da498d47ba0d40d4cae33cacab58dcb8db43f` | disconnect |
| [1b360](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1b360.json) `2a1edfaf00aea9458798cbeb027f8b8d86b075bb3f6240cb549a50903741ec14` | timeout |
| [38230](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/38230.json) `b3e5d6efd635c23cf4b90ac2f68586bbe58343b56c9a8ecfc611252b4b35a656` | 有上下文消息辅助 |
| [38370](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/38370.json) `dff3abc0ef32352d235db96c4f17d68c8d00b83dfcb0d05ab0221dbbbbcef4a1` | 构造待发消息辅助 |
| [1a1d0](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1a1d0.json) `bb5c6fce0c80ff51dbf099baaafe6b73dc9014e7600164a40dda2fe3f9ac1694` | publisher 回调入队 |
| [1a690](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1a690.json) `3bebd5f330ee84fd423b937c70d015a30504359bb459c4f3416299b55a536612` | session 过滤入队 |
| [1d6f0](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1d6f0.json) `aa734fecbfd61fe9d874cdc30f2a8470ea71b0f8875ec34774df5dd8c21820e6` | RegisterFnCallback |
| [5ff20](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/5ff20.json) `0f6cfaec23d8027b23953c68086387da00d44fac159add554eb63ff516eea8a2` | CreatePingScheduler |
| [603c0](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/603c0.json) `5952d5e7460c7a8289f657fb2313b650c90859f62977034f7fedf9c7c5c8e5d9` | TriggerPingReply |

### 输入与输出队列

[14e50](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/14e50.json) 在 this+852 mutex 下等待 this+900 condition variable；this+848 保存待取数量，取出事件类型、路径字符串和引用计数对象后更新 head/count。静态分支：事件 0 创建端点、分配递增 session id（this+4）、插入端点树并调用 17ab0；事件 1 按 session 查找端点、调用 1ac80 发送 UnInit 后移除连接；事件 2 受 this+773 暂停保护，暂停返回 21，缺连接返回 6；事件 3 在 this+1148 rate limiter 取 token，允许时调用 17440，受限返回 353 并 signal this+1108；事件 4 记录 FAIL，缺端点返回 6；事件 5 记录 SHUTDOWN；其他事件返回 1211。这些数字是当前对象的内部结果，不能当跨版本公共枚举。

[16630](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/16630.json) 在 this+968 队列取事件。事件 2 检查暂停 21 和缺连接 6，再按 sub_1003BE00(byte_1006B8BE,0) 选择 38230 或 38370；两者调用 sub_10022B10，结果为空时经 sub_1003DA60 补建消息。38370 复制消息内容并将对象 +87 置 1。随后按 sub_100210D0(ArgList, 200 * index, ...) 取发送单元并记录 MESSAGE | SEND FAILED；200 倍索引是静态取值证据，不能据此断言网络帧必为 200 字节分片。事件 5 结束线程，未知事件为 1211。

### 订阅、验证和 JSON 消息

[17ab0](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/17ab0.json) 和 [17440](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/17440.json) 都要求 transport 状态有效、端点存在、session 非负，否则返回 87；路径匹配 /[0-9]+ 后追加 /client，表达的是 session 路由。subscribe 两次调用 sub_10039090(...,16)：首个分支创建 0x90 字节普通端点对象，第二个分支创建认证模式并构造 Auth，session 不可用返回 1413；两分支都设置 1b360 超时回调，并调用 1d6f0 注册 callback type 1。解析均失败返回 123 并记录 invalid endpoint，不能当设备认证成功。validate 成功把端点 +1056 置 1，两种格式均失败时置 -1、返回 123；它没有硬件查询或读回。

[18650](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/18650.json) 先验证全局状态、非空路径和 session，在路径寻找 client 后用十进制 strtol 解析；无数字和越界抛 invalid stoi 与 out-of-range。它读取 JSON 的 func、param、return。publisher 的 Init 只接受 nlohmann JSON 类型 4/5/6/7（bool、整数、无符号整数、double 转整数），其他类型抛 type error 302。Init 返回零时读取 DisablePing 与 Software\Razer\ChromaBroadcast\Apps；未禁用时注册 type 0 的 1ab00 回调并调用 5ff20 建立 ping scheduler；其他分支构造 Pong 并由 603c0 触发 ping event。subscriber 分支交给 1a690 入队；这些返回值是回调／队列状态，不是设备设置结果。

### 发布、断开、超时、datafile 和破坏性路径

[1a1d0](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1a1d0.json) 和 [1a690](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1a690.json) 查找连接／端点，把 sub_10012440 加入输出队列，增加 +241 计数并 signal +254 condition variable。1a1d0 可返回管理器缺失 4306、端点缺失 6、路径无效 123；1a690 比较 session，不匹配 1413，并保留前置状态 87；成功只代表事件入队。[1ac80](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1ac80.json) 在连接 map 缺失、session 不匹配、端点路径无效时返回 4306、1168、123；成功构造 /client、func=UnInit，调用 1a1d0 入队后返回 0。[1b360](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/1b360.json) 同样处理 4306/1168/123，成功 func=Timeout；二者都不等待远端响应。

[5ff20](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/5ff20.json) 在已有 event/thread 时记录 1247，创建两个 event 和一个线程，handle 缺失返回 6。[603c0](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/603c0.json) 没有 event 返回 1168，event 存在但端点未激活返回 21，激活时 PulseEvent 后返回 0。

[19780](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/19780.json) 构造远程 URL https://chroma-assets.razerzone.com/chromasdk/components/broadcastsdk/files/broadcast.dat，读取 DataPath、LastDownload 和 Software\Razer\ChromaBroadcast\Apps，拼出本地 broadcast.dat 路径并调用 [19e80](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/19e80.json)。19e80 结果 0 为下载完成、1 为已最新且写回 LastDownload/Apps；2/3/4/5/6/7 依次为 Internet handle 初始化、URL 打开、Last-Modified 获取、Last-Modified 解析、本地文件打开和文件写入失败，其他值为 unknown error。该联网下载和写回路径属于本地优先范围的删除对象，本轮未执行网络或注册表写入。

[196c0](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/196c0.json) 调用 OpenProcess(PROCESS_TERMINATE) 和 TerminateProcess(pid,1)，只有两个调用都成功才返回 1；这是破坏性进程终止路径，本轮未执行，不能包装为普通断开设备。

本节只证明 Server 本地队列、端点 JSON 和调度语义；未运行官方 DLL、Server、服务或设备，没有真实页面连接、设备 ACK、读回和硬件状态。后续 Rust 必须分别建模队列返回、publisher/subscriber 状态、远端应答、取消、线程 join 和资源释放；静态或模拟一致不能提升官方实际 DLL 对照或设备验收状态。

## Manager 的共享帧到应用管道消费者（CL／KB）

这次把 Manager 线程从共享映射区取出的 CL／KB 帧继续追到按应用分发、命名管道和直接导出调用。原件是 `RzChromaConnectManager.dll`，输入 SHA-256 `a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3`，大小 `117448 B`，Image Base `0x10000000`；捕获索引 SHA-256 `bc3bf726aa719c565aca07b4bab20c191ba961343938d3c2145e63ce22613126`。本节选 `25` 个完整捕获函数，去重后 `60` 个 chunk、`3882` 条指令、`13104 B`，未列原字节为 `0`。`.work/chroma-connect-lifecycle-verify.py` 用 `ThreadPoolExecutor(max_workers=4)` 独立检查每个函数 JSON SHA、chunk SHA、PE 原字节和逐条指令字节；这仍是静态证据，不是 DLL、服务、管道或设备执行。

| RVA | 捕获函数原字节 SHA-256 |
| --- | --- |
| [`4870.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/4870.json) / [`4c30.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/4c30.json) | `fd7e5393393cc1d1c48f5fd37b69611c6cd174824a2ca12c2d76e6f2b6bce4eb` / `b8422f24f4ebbb5288987c664f8e6a85e30cbacce1b0f3f9c57444cd53d8a31f` |
| [`d460.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/d460.json) / [`d630.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/d630.json) | `5efa990d4a339483ac4fb99a6cab0dc20370efc5d983a357307eafb6ec915e9c` / `211b443cd3566144d0e681c87b707aef839872002108034ebee4bb1850f48e7c` |
| [`dc00.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/dc00.json) / [`dbe0.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/dbe0.json) | `4029f9f41469651768bf52dd747a1326cba262079e6ce7ca473b438ddcb76be1` / `bb09cdc3f31b7d6e5305303ab8dac699700ceab333190f72542a15edf4ab0cc9` |
| [`dc90.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/dc90.json) / [`dc20.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/dc20.json) | `8a1cc00141f43eaa82d74e345104bbb3db9f7432da657af0583da82d2e8709c2` / `d68d4164b3c05f51f2f31064903d874a2c0e406c9410da735715ec82dce80771` |
| [`dd30.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/dd30.json) / [`dd00.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/dd00.json) | `822391ec878d9a78326991c99b007a6ebb24ec6422e1a5ee15408eeba31807fe` / `5a1a8f1f5adc9fd890a8a28409553d4edc694babd938e2b78f68b390c88fa91e` |
| [`e700.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/e700.json) / [`e710.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/e710.json) | `7e74e8befab0ca508a488b6dafaa8079dda38b1ab938ee6b4a94a2fbe662effc` / `35a2022598678b2cdbaf75042ae8bcf9e653d7fdb153d1d1c959a259fdb78f2e` |
| [`4530.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/4530.json) / [`46e0.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/46e0.json) | `36db6afe09e077e982fb5830f00402f19655ba454ea27df936b9b44319854e37` / `03e0b4723d67b466c876a8e20b798e3a899b3da2e315760be57c2b26698f60df` |
| [`5990.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/5990.json) / [`61a0.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/61a0.json) | `108d1a005cc9770610ca14c24a9a313c02de5e53ff5919f4c5344fcddcad70b9` / `82d9e77d781bef88200a476d9c182f56847660bacf73a16c3da6b5da38c9cb7b` |
| [`3c90.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/3c90.json) / [`7240.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/7240.json) | `92b0618eddd2d27b88c10160992207e95da6f400f5fc0b17c5488f7e8282c911` / `a95ad260bd7bd01be296d8bdbd0703edabc1b9d28274addb79666f7e017c2da5` |
| [`7340.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/7340.json) / [`77b0.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/77b0.json) | `8329dc22f2a7d369d6c3a19c579597c7cf13234fc46441d22f11dd0133eabd78` / `814e4de69ea6b49e4407076127414dc8bb177e7185910ce03fdc42bd2aec1b7b` |
| [`a0d0.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/a0d0.json) / [`7a30.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/7a30.json) | `2263cef3bb102b99598110ed50409a94ff61d86b57d618f7ecdc9ece49d2e73b` / `b5a2658fde285410afb9814c8d096e93b5910e3f288b7a2aeebe0731aad6dee1` |
| [`7460.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/7460.json) / [`76f0.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/76f0.json) | `1a08b9a4b31ae29f09eaafbdddd14a6006a8fd706aed961a792fb0760a5814be` / `4ee8503f1d8bd0198563895194773012783fae1b406a1ab38b63223b594b316c` |
| [`7bb0.json`](../../../.analysis/results/native/a5195ba067560744a6fe83a2c731b576fe557e6fa28c67f6644a4eb16ea677f3/functions/7bb0.json) | `24387608b3faedb50df979c8e51162213630ea2424a2a39b28cdc324443bf3fe` |

### 共享映射帧的门禁和并行分发

- `4870` 是 CL 消费入口。先要求 `this+80 == 1`，否则返回 `1168`；`this+236 == 0` 返回 `2250`；对象首字段仅接受 `0x800000` 或 `0x80ffff`，否则返回 `13`。类型 `6` 把 `v74[57]` 的 RGB 复制到四个颜色槽，类型 `7` 从 `v74[6..9]` 读取四个颜色并把 `v74[10]` 作为附加颜色；类型 `0` 保持清零。它在 `this+232` 共享锁内遍历 `this+88` 的 app tree，把帧和 `TaskCollection` 交给 `d460` 并等待全部任务；每帧按性能计数器补足约 `33.333333 ms`，未知类型返回 `1804`。
- `4c30` 是 KB 消费入口，门禁相同，但首字段接受 `0x10000` 或 `0x1ffff`。它构造 `0x304 B` 的临时帧：类型 `0` 清零八组 `0x60 B`，类型 `6` 将一个 RGB 复制到八组×24 槽，类型 `7/8/0x11` 复制 `0x300 B`；类型 `8` 另按每组 `0x16` 项检查 `0x1000000` 位并转换 RGB。随后在同一共享锁内复制帧，调用 `d630` 并等待；未知类型仍返回 `1804`，周期同为约 `33.333333 ms`。
- `d460`／`d630` 将 app tree 逐段收集为至多 `0x400` 个节点，创建并行任务；`dc00`／`dbe0` 是任务回调入口，分别再次调用 `d460`／`d630`。PPL 任务的 `dc90`／`dc20` 依据任务状态在最终单 app 路径调用 `dd30`／`dd00`，而 `e700`／`e710` 只是调用 `e720`／`eb20` 做分段取消 beacon、任务等待和释放。调用方已等待任务集合，但没有聚合每个 app 的设备写入结果。
- `dd30`／`dd00` 从 app 记录取得模块句柄和 app index，分别转入 `4530`／`46e0`。因此共享区消费到“按 app 调度”之后仍有第二个分支：它可能写本机命名管道，也可能从已加载模块取得导出函数。

### 命名互斥体分支、管道帧和导出调用

- `4530` 检查 `Global\\{43A54ADB-A530-4E8A-A3C6-88528EECFB17}<pid>`，`46e0` 检查 `Global\\{A59A8336-5A42-490E-832A-89B58D3F1347}<pid>`。互斥体已存在时，两者分别转入 `5990`／`61a0` 的管道写入；不存在时从 app 的模块句柄 `GetProcAddress` 查找 `SetBroadcastEffectOnApp`／`SetKeyboardEffectOnApp` 并直接调用。互斥体只决定分支，释放／关闭句柄后并不提供设备确认。
- `5990` 使用 `\\.\\pipe\\{F71C2619-0D73-4876-8CF5-16E2629DB71B}<pid>`，通过 app-index 的 FNV-1a 风格桶查找或 `3c90` 创建 `GENERIC_WRITE`、无共享、`OPEN_EXISTING` 管道；序列化的对象字段静态可见为 `eventId=2`、`start=0`、`numCol=5`、`numRow=1`、`regionId=0`、`payload=1` 和 `frame`。Broadcast 分支检查 `WriteFile` 的 BOOL；失败时读取 `GetLastError`，错误 `232/233` 或本地状态 `6` 会关闭并删除该管道句柄。返回的是本地写入错误码，未见远端 ACK 或读回。
- `61a0` 使用 `\\.\\pipe\\{5054F1BB-5F54-494C-9CC4-35F99D9E31BF}<pid>`，字段为 `eventId=2`、`start=0`、`numCol=24`、`numRow=8`、`regionId=0`、`payload=1` 和 `frame`；内层按连续输入地址写入 `8×24` 个 RGB 三元组。它在写入后直接调用 `GetLastError`，没有检查 `WriteFile` 返回 BOOL；因此 Rust 不能把该返回值当成键盘帧发送成功。错误 `232/233` 或状态 `6` 时同样关闭管道并从 map 删除。
- `7240` 删除 app-index 对应 map 节点并释放其管道状态；这只是本机缓存清理。`3c90` 的 `CreateFileW` 失败保留 Win32 错误，不能回退为设备在线。两个 `GetProcAddress` 分支和管道分支都未在捕获中出现读回、设备 ACK、取消确认或硬件释放握手。

### JSON 帧构造和生命周期边界

- `7340` 对 nlohmann JSON array 做下标访问，非 array 抛 type error `305`，容量不足时扩展并返回 16-byte 元素地址；`77b0` 按 JSON 类型 `1..7` 深拷贝对象、数组、字符串、布尔、数字或指针；`a0d0` 初始化 `null/object/array/string/bool/number` 等内部类型；`7a30` 把元素区间转换为临时数组，保留原有节点所有权转移规则。
- `7460` 建立 nlohmann output-string adapter 并调用内部序列化；`7bb0` 按尺寸分配并以给定字节填充缓冲；`76f0` 按 JSON 类型释放数组、对象、字符串和缓冲。`5990`／`61a0` 在 `WriteFile` 后释放临时字符串和 JSON 对象，`7240` 再释放 app-index 管道节点。异常路径可能只完成局部句柄／字符串清理，不能当成远端取消完成。

本节闭合了“共享区 → CL/KB 帧整形 → PPL app 遍历 → 管道或模块导出”的静态调用链，但没有声称官方模块实际执行成功。尚未验证的部分包括：命名管道服务端的协议解析、`Set*EffectOnApp` 被调模块的真实设备传输与读回、每个 app 的错误聚合、服务／线程取消竞态和硬件释放。Rust 迁移与测试必须分别保留 `1168/2250/13/1804` 门禁、管道创建／写入失败、键盘未检查 BOOL、导出缺失、任务取消、句柄释放和真实设备 ACK 状态；无设备时只能用协议约束的模拟应答，不能把 PPL 完成或 `WriteFile` 返回当作灯效已生效。

## RzChromaConnectAPI／API64 导出、通知与共享帧生命周期

本节把 Chroma Connect API 的导出包装、回调注册、共享映射、接收线程、在线事件和停止释放链接到已经核过的 Manager/Server 消费者。这里只使用当前原件、捕获索引和逐条原始指令；没有加载 DLL、服务、管道或设备，也没有把导出调用接入 Rust 生产路径。

- x86 `RzChromaConnectAPI.dll` 原件 SHA-256 为 `6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1`，大小 `198344 B`，Image Base `0x10000000`，固定文件版本 `3.1.1.102`；[capture.json](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/capture.json) SHA-256 为 `9719489260709a7fe71694ed11aaade9fe80f038b783f305feef19285f36bac3`。
- x64 `RzChromaConnectAPI64.dll` 原件 SHA-256 为 `babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f`，大小 `223432 B`，Image Base `0x180000000`，固定文件版本 `3.1.1.103`；[capture.json](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/capture.json) SHA-256 为 `bae39fcffaf59a8af8299a5ecad4c0f69f14d9c7382c69b70c77a70ec1571a73`。
- x86 选取 `23` 个完整函数，去重后 `42` 个 chunk、`3949` 条指令、原始 chunk `13962 B`、指令字节 `13955 B`，保留 `7 B` 未列为指令的对齐字节；x64 选取 `21` 个完整函数，去重后 `21` 个 chunk、`3682` 条指令、原始 chunk `16018 B`、指令字节 `15986 B`，保留 `32 B` 对齐字节。合计 `44` 个函数、`63` 个唯一 chunk、`7631` 条指令、`29980 B` 原始 chunk，未列指令字节 `39 B`。校验脚本 `.work/chroma-connect-lifecycle-verify.py` 以 `ThreadPoolExecutor(max_workers=4)` 并行核对原件 SHA、capture SHA、函数记录 SHA、chunk 原字节和逐条指令字节。
- x86 的 `7da0`、`7dc0` 捕获保留完整反汇编和原始字节，但 Hex-Rays 伪代码分析失败；x64 这 `21` 个选定函数均有伪代码。伪代码缺失不能当作函数不存在或 ABI 已确认。

### function records

Each row links to an individual function capture JSON. The four-thread verifier checked the function-record SHA-256, chunk SHA-256, raw bytes, and instruction bytes; the x86 and x64 sets are counted separately.

#### x86 function records

| RVA | function record SHA-256 | role |
| --- | --- | --- |
| [`1d2f0.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/1d2f0.json) | `e88409eeb570f325fbf4593807b28a370d604e431d5eec05aff1a99c410ed5e7` | Init export wrapper |
| [`1d4a0.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/1d4a0.json) | `81ab3a28b9f7781832d8e1f9179ad7f32ed4615d2f0a6d109bee6db9d1d6dfa4` | UnInit export wrapper |
| [`1d5b0.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/1d5b0.json) | `d10d5fd0ff5f5da031e78f3468a25038c701998fe76b844d5392586e13433adf` | RegisterBroadcast callback slot |
| [`1d5d0.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/1d5d0.json) | `1649b196a0905379136f0c8059b793cc1556d825e45bd470ae5e79749d8f15d7` | RegisterKeyboard callback slot |
| [`1d5f0.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/1d5f0.json) | `4a7f5a8a336d6c5572055b2dbfd2422122fef8e862081361935ee86af998f4e9` | UnRegister callback slot |
| [`9330.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/9330.json) | `f52efd132af11680be367747d408eda4ae241a09190fa7a63821b78cd0b6491b` | Init internal |
| [`9a50.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/9a50.json) | `5d06a5816c31312ebc6698c497aa187e3fac6050f739bb4f25fa56e0e2efc6ee` | UnInit internal |
| [`9d90.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/9d90.json) | `c0274b0b9117d4faca154ad66b1b48a5ede720d624429d016e471e77961c4b80` | Broadcast registration |
| [`a030.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/a030.json) | `cf91c5c72605877f5554869b87b19d3204a7feb08d0dfdd6ff64652071b47062` | Keyboard registration |
| [`ce50.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/ce50.json) | `d986c656f5b3238852fb90894462fae7ff92ec62ea1f3eb724221b72a6b5e338` | DeviceSupported capability write |
| [`5240.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/5240.json) | `e3d9fb06b8c8ccfb9f1f8b832539fade869b66ee86c7db5fad4c6b97c2b37899` | Apps Event registry helper |
| [`7d80.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/7d80.json) | `fccf8bb4ff7fa4666a9fb6141b46ea9a483453933d83eb1794fe952c938e23bb` | online thread wrapper |
| [`7da0.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/7da0.json) | `671567d4f70280f2863f2e489251b5790d823e11ce4ec064661d46d4d3ba8073` | CL thread wrapper (Hex-Rays failed) |
| [`7dc0.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/7dc0.json) | `6127864d8ee5b9824d57082258b4e3f4725a1cccea0392b31c4c8058fd977949` | KB thread wrapper (Hex-Rays failed) |
| [`7de0.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/7de0.json) | `fc4fbb253f50cf7ee3a401276945cdafce02dad1132f2678c06df08a421313c8` | online monitor thread |
| [`8090.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/8090.json) | `3f7ef2a8520c394382389c37e938cf862094d5f38b87fa3b49f944f1c3cff4d3` | CL shared-frame receiver |
| [`8950.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/8950.json) | `b011cea8537f7f6824207216bd7aa50e44f25459af242578696fbbe5655de18c` | KB shared-frame receiver |
| [`1090.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/1090.json) | `632b0552bc311a92471aae516e847ba0215ba85efd9b6f95bd3e69ba10cbe73b` | CL shared mapping |
| [`1380.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/1380.json) | `17f2e333a12a9c6623f52c7d190603c8cc28af546f52142d09cdfef6dd14279f` | KB shared mapping |
| [`1030.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/1030.json) | `1c3898ec3b9d7c6f910bfbe0a05db03463f974d1eb6d6eea204948136887917a` | mapping/queue cleanup |
| [`a220.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/a220.json) | `82e78c5b123a353588166c1acc18eacaa16f42b56b06d4ed7f051518da87758e` | status refresh |
| [`a430.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/a430.json) | `8c1f67e33e86c4619d4236f32022cd1817197f8e5ba30a46761b1c59a71bf44c` | status helper |
| [`4a20.json`](../../../.analysis/results/native/6f31bce73c2f1a869c6ff79802bfdc2500dabd17c4dd39488fde29efeaf06cb1/functions/4a20.json) | `7872c716133b6789c7bfa766fd009f33419093d05eb81e12ced831053fd44199` | object constructor |

#### x64 function records

| RVA | function record SHA-256 | role |
| --- | --- | --- |
| [`20940.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/20940.json) | `bd6d7063104de891cc8b6f8cd530a25f4c8754a907300b3653bb1ba857596673` | Init export wrapper |
| [`20b40.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/20b40.json) | `47e25dfeb336f5e8fb7e279eef92d64d3b959ccaf2fb3d7321126ad8db7b10a2` | UnInit export wrapper |
| [`20c20.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/20c20.json) | `e7d61aee7206e7574f40150a4fe78b89a08a2bacf369f9596a1921ff2de32ce2` | RegisterBroadcast callback slot |
| [`20c40.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/20c40.json) | `95277ef21ef690e3268e1fdc4d7e0281be7c80e3a9c61e01b591a2efcf059dc7` | RegisterKeyboard callback slot |
| [`20c60.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/20c60.json) | `044e2cbd7f7650cbfa83253bb4ceb18c98ba6857e40d5640c04e7c9ecab611e0` | UnRegister callback slot |
| [`a5a0.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/a5a0.json) | `1b199b91d5e5d6871d49fbb34347e1d388119ddd3f72aaefd687fe5b1014319a` | Init internal |
| [`ae40.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/ae40.json) | `bfbc172c0b808474bd552424c9d5318ca574629d7fafe7e6c08d1d4ea33b878f` | UnInit internal |
| [`b230.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/b230.json) | `c5d25226001de89541cb216db303043da3ccbb82e9b2bdacd324ba25c3514752` | Broadcast registration |
| [`b590.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/b590.json) | `e64f636653ba0e4567f94d20b26beee1cdafe817a134f71fab409a4711976abe` | Keyboard registration |
| [`e930.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/e930.json) | `86a8200b30455770e5010e31bfecfc07cd997b99f8e59b6d4ebad4c038b84c74` | DeviceSupported capability write |
| [`59f0.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/59f0.json) | `0b3b4f28b4629254a595c011caec58aea9eb859cd620b2a8d1642ba6babea090` | Apps Event registry helper |
| [`8790.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/8790.json) | `e002ec9548e08e6762f4d4bf7f1d35a3587293710e6d285a05c40fa73b9a56ab` | online thread wrapper |
| [`87a0.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/87a0.json) | `1ef4fc3a8eef70cd8d33359ae6f1882b3dc75fcfc708a85e3668202d81647f49` | CL thread wrapper |
| [`87b0.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/87b0.json) | `51ed5968cd3667b21e76c7e81618814dfa0dabd38e9b582f41cad0880ff67398` | KB thread wrapper |
| [`87c0.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/87c0.json) | `2d9d9a5df384e61dc22797c255fff3a1bebb2b3055984bf7f01e4a3ea445d931` | online monitor thread |
| [`8b60.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/8b60.json) | `86921260f879c755bbc29046f0c61d4722182e2975715551830f4763662b2a0a` | CL shared-frame receiver |
| [`9730.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/9730.json) | `7581ba4a5269b59bea85202ea8724d7edcc69f1a7c69b0168b417f085abc9bd3` | KB shared-frame receiver |
| [`10b0.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/10b0.json) | `9d1731842f5bdf9969627e693888a0e7cc9d01ab92cf112e96f2ef78111a7dd2` | CL shared mapping |
| [`1470.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/1470.json) | `38ef2c47def6ad90be102c235ff727c2cf4e648f2c130ff3a86c5dfad26a817f` | KB shared mapping |
| [`1040.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/1040.json) | `94c059be0834a9539af52ae0dbde9b1445c53ee9f000129272ea8397c1f7ec18` | mapping cleanup |
| [`1400.json`](../../../.analysis/results/native/babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f/functions/1400.json) | `9cd300664d7376e0eae4026053934c2d0f4ca7a1c021ec983d3aa7daf30a0d9c` | registry/object cleanup |

### 导出包装与对象生命周期

- x86 `1d2f0`、x64 `20940` 是 Init 导出。两者先拒绝已有全局对象（`1247`），分配并构造内部对象，再把全局对象指针写入 **Block**，之后才调用内部 Init（x86 `9330`，x64 `a5a0`）。内部 Init 的失败或异常不会由导出包装自动清掉已发布的 Block；因此“返回失败”与“全局对象没有留下”不是同一件事。x86 会把输入的 `16 B` 复制到栈上，x64 读取指向的 `16 B` 数据；这只是当前原始调用形状，公开头文件和跨架构 ABI 仍需单独核对。
- Init 内部访问 `HKLM\Software\Razer\ChromaBroadcast\Apps`，按 AppId 返回的验证值分支：`0` 抛未知 AppId，`1/2` 继续，`-1/4/3` 映射为不同的内部错误；AppId `89` 或 `85` 还会改变帧率标志。随后建立 `ns-cbevent<appid>` 命名空间和 Broadcast/Keyboard 手动复位事件，初始化临界区，创建 CL、KB、在线监视三个线程以及匿名停止事件。成功判断没有把停止事件以外的所有资源都纳入回滚；任一个事件或线程创建失败时，已创建对象可能保留。
- x86 的线程包装入口为 `7d80/7da0/7dc0`，x64 为 `8790/87a0/87b0`；实际循环分别在 x86 `7de0/8090/8950`、x64 `87c0/8b60/9730`。监视线程和两个接收线程共享同一对象；线程包装只负责把入口交给线程运行时，不能从包装返回值推断线程已启动或已退出。
- x86 `1d4a0`、x64 `20b40` 是 UnInit 导出。它们调用内部 `9a50/ae40`；只有内部结果为零时才继续析构、解除映射、释放全局对象并把 Block 清零。若内部返回非零，导出会保留对象和 Block，不能把错误码包装成已释放。

### 回调注册、通知和注销

- x86 `1d5b0/1d5d0`、x64 `20c20/20c40` 先检查 Block，再把回调交给 Broadcast 或 Keyboard 注册函数（x86 `9d90/a030`，x64 `b230/b590`）。注册函数只有在对应槽为空时才写入回调；槽已非空时直接返回 `0`，不会替换旧回调。空回调参数返回内部的无效参数值。
- 注册成功会把 `DeviceSupported` 能力位通过 x86 `ce50` 或 x64 `e930` 进行 OR 合并，并写到 `HKLM\Software\Razer\ChromaBroadcast\Apps\\<appid>\DeviceSupported`；随后把 `Event` DWORD 置为 `1`，并尝试 Pulse 全局变更事件 `Global\\{08983A2E-6665-42B2-9492-38D5B1F3340A}`。注册表创建、写入和事件打开失败只进入日志/局部错误分支，未形成设备确认或回调 ACK。
- `1d5f0`（x86）和 `20c60`（x64）注销使用 `TryEnterCriticalSection`。临界区忙时跳过清槽，仍返回 `0`；只有拿到锁才清除对应回调。该路径不停止接收线程、不解除共享映射、不重置 `DeviceSupported` 注册表值，也没有等待回调已经返回。因此 `0` 只表示注销路径被调用，不表示 callback 已经不可达。
- 注册表辅助 x86 `5240`、x64 `59f0` 会忽略 `RegSetValueEx` 的返回状态；这些函数只表示本地通知设置尝试完成。Rust 侧必须把“回调槽状态”“通知事件状态”“注册表写入结果”和“真实设备状态”分开建模。

### 共享映射和接收线程

- Broadcast/CL 映射函数为 x86 `1090`、x64 `10b0`，Keyboard/KB 为 x86 `1380`、x64 `1470`。它们先以 `F001F` 访问尝试打开既有映射：CL 名称为 `cbdata<appid>`／`ns-cbdata<appid>`／`{F036EB92-D237-492A-9B0C-25FA8DE0EA58}<appid>`，KB 名称为 `kbdata<appid>`／`ns-kbdata<appid>`／`{A063D707-375C-4A1E-B6FA-AA277843F4AD}<appid>`。打开失败后才创建可读写映射；映射成功仍会把整个区域清零，因而存在打开既有对象时覆盖内容的竞态。
- CL 视图大小是 `8 + 10×40` 字节，KB 视图大小是 `8 + 10×784` 字节。`8b60/8090`（CL）和 `9730/8950`（KB）读取共享区首 DWORD 作为读索引，遍历十个 ring slot，按 `0..9` 回绕；等待“数据事件或停止事件”。停止索引使线程退出，随后清空本地指针、解除映射和关闭映射句柄。
- 每个 slot 进入临界区时使用 `TryEnterCriticalSection`；锁忙也会前进读索引，所以帧可能被丢弃，原版没有排队重试或丢帧 ACK。CL 从 slot 拷贝固定 `40 B`，再构造带颜色和附加 DWORD 的回调临时数据；颜色高字节非零时按原版清零。KB 拷贝 `784 B`，回调临时帧为 `0x304 B`，其中前 `0x300 B` 来自 slot 的连续数据。接收频率通常为 30 FPS，AppId `85/89` 分支为 15 FPS。
- 回调前还检查连接字段、全局启用标志、虚拟 App 启用状态和相应 `DeviceSupported` 位；任一门禁不满足就不调用回调。回调事件 `1` 传递帧；x86 还在状态变化时传递事件 `2` 的在线/离线状态，x64 的反编译对第二参数显示不完整，公开 callback 签名不能仅按伪代码确定。回调返回值不参与设备确认。
- 状态刷新 x86 `a220` 检查 `Global\\{08B4F43A-DA51-4120-B388-CE0F8CE6F61A}` 互斥体并读取 App `Enable`，默认值为禁用；没有硬件查询。共享帧消费完成只代表本地回调被尝试，不代表灯效写入设备。

### 在线监视、取消和释放

- 在线监视 x86 `7de0`、x64 `87c0` 打开在线事件 `Global\\{A84AF9C8-EFE0-430D-871C-10DA760C2CCD}`、离线事件 `Global\\{0F9297E6-E80C-47E4-9A8B-1237E50484B7}` 和停止事件，以 `WaitForMultipleObjects(3, ..., INFINITE)` 等待。在线分支置连接为 `1`；离线分支置为 `0`，清理部分状态后直接调用两个回调槽的离线通知，不经过回调临界区。若两个在线事件中有一个打开失败，捕获的可见分支没有保证另一个已打开句柄被关闭。
- 离线通知还会把 `HKLM\Software\Razer\ChromaBroadcast\Apps\<appid>\Event` 置为 `2`，并 Pulse 全局变更事件；部分状态清理会保留指定的颜色索引。该注册表/事件动作仍是本地进程通知，不是服务或设备 ACK。
- 内部 UnInit `9a50/ae40` 设置停止事件，分别对 CL、KB、在线线程 `WaitForSingleObject(..., 2000 ms)`，但忽略等待返回值，随后关闭线程句柄和事件句柄；线程超时仍可能被释放对象继续使用。它把 `Event` 写为 `2`、清 `Pid` 并删除临界区/映射。x86 `1030`、x64 `1040/1400` 执行映射和注册表辅助清理，也不提供线程 join 证明。
- 由于注销可在锁忙时保留回调、在线线程可在未持锁时回调、UnInit 忽略 2 秒等待结果，取消/释放存在竞态边界。Rust 实现必须使用代次、取消令牌和明确的 join/句柄所有权；不能把 `UnInit == 0`、`WaitForSingleObject` 调用或 `FreeLibrary` 成功当作设备写入已经恢复或硬件释放完成。

### 证据边界

本节闭合了 API 导出 → 注册通知 → 共享映射 → CL/KB 接收 → 在线监视 → 停止释放的静态链，但没有闭合回调消费者实际写入设备的协议和读回。`GetProcAddress` 找到导出、映射写入成功、回调返回或注册表更新都不能替代真实设备确认。当前公开 ABI、回调结构布局、服务端 ACK、设备应答/读回、取消竞态和硬件释放均保持未验证；后续对照测试必须先只读、记录初始状态、写入后读回并恢复，且与独立 Rust 结果逐项保存。

## RzChromaBroadcastAPI/API64 外层加载器与导出转发

本节核对外层 `RzChromaBroadcastAPI.dll` 与 `RzChromaBroadcastAPI64.dll` 的加载、导出解析、通知转发及卸载。它们是连接 API 的加载包装，不是设备传输实现；本次只做静态原件和捕获校验，没有 LoadLibrary、服务、管道或设备执行。

- x86 原件 SHA-256 为 `e3e4ce109f92b22a000209113ede41c84cdcb72dc05c1c3868c502db1c000a6b`，大小 `46792 B`，Image Base `0x10000000`；[capture.json](../../../.analysis/results/native/e3e4ce109f92b22a000209113ede41c84cdcb72dc05c1c3868c502db1c000a6b/capture.json) SHA-256 为 `3653cb0833879db3e6d8544c204121679b4700545b80d1313eb7df9dad01f4df`。
- x64 原件 SHA-256 为 `21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55`，大小 `53448 B`，Image Base `0x180000000`；[capture.json](../../../.analysis/results/native/21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55/capture.json) SHA-256 为 `88ca1fc8a4cb43695974ef2488c551dda22bf6e9e75eb83d667c9403bdb1adde`。
- 两个原件捕获均 `capture_complete=true`。本节各取 5 个完整导出函数，共 `10` 函数、`10` 个唯一 chunk、`312` 条指令、`1177 B` 原始指令字节；x86 为 5/165/509 B，x64 为 5/147/667 B，x64 另保留 `1 B` 未列指令的对齐字节。`.work/chroma-connect-lifecycle-verify.py` 已将两个集合加入 `ThreadPoolExecutor(max_workers=4)`，并行复核输入 PE、函数记录 SHA、chunk 原字节与逐条指令字节。

### 方法记录

| 架构 | RVA | 函数记录 SHA-256 | 静态用途 |
| --- | --- | --- | --- |
| x86 | [`4480.json`](../../../.analysis/results/native/e3e4ce109f92b22a000209113ede41c84cdcb72dc05c1c3868c502db1c000a6b/functions/4480.json) | `627bca570d36fcb54ee71138b2f68e939375cabd4b105333e95470d63897d45c` | `Init` 导出加载并转发 |
| x86 | [`4610.json`](../../../.analysis/results/native/e3e4ce109f92b22a000209113ede41c84cdcb72dc05c1c3868c502db1c000a6b/functions/4610.json) | `85a95383e61acc1521d6372752970b237af890ab391771b27aa175c8625053ec` | `UnInit` 转发并卸载 |
| x86 | [`4650.json`](../../../.analysis/results/native/e3e4ce109f92b22a000209113ede41c84cdcb72dc05c1c3868c502db1c000a6b/functions/4650.json) | `b0e8aa65493e20847fb2780d0a1afd215061f5278389e141bd1f6264bdf8819f` | Broadcast 通知转发 |
| x86 | [`4670.json`](../../../.analysis/results/native/e3e4ce109f92b22a000209113ede41c84cdcb72dc05c1c3868c502db1c000a6b/functions/4670.json) | `b3717bea6a84acc0742943263db91a1e3347758c3c353a2ba3a6dd2cb9d92df0` | Keyboard 通知转发 |
| x86 | [`4690.json`](../../../.analysis/results/native/e3e4ce109f92b22a000209113ede41c84cdcb72dc05c1c3868c502db1c000a6b/functions/4690.json) | `847c1167cf8682ea99b2cc8fe53e4ac75fa8bc57138c00805f99e2d543f5f820` | 通知注销转发 |
| x64 | [`4b00.json`](../../../.analysis/results/native/21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55/functions/4b00.json) | `eb544f64e1db78c9b9b072ce5d2c33603e133313f6f04eef28a4798121645ff5` | `Init` 导出加载并转发 |
| x64 | [`4d20.json`](../../../.analysis/results/native/21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55/functions/4d20.json) | `261c1ee596a91e50c75632ba82a9f86c283a4eca946bf23241283c1588c3c133` | `UnInit` 转发并卸载 |
| x64 | [`4d60.json`](../../../.analysis/results/native/21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55/functions/4d60.json) | `2ced2e99f463d289f1d4af82d195d86556c733851ed944f5e02ca9a9d8e54a73` | Broadcast 通知转发 |
| x64 | [`4d80.json`](../../../.analysis/results/native/21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55/functions/4d80.json) | `c0b6cb781220474f2a6a5c8e7523b8be45b7a9ad165b1dea2bb0e9be21201d1d` | Keyboard 通知转发 |
| x64 | [`4da0.json`](../../../.analysis/results/native/21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55/functions/4da0.json) | `0c7d580eab504e7dd73fe01bbce1990516bc6cee949257e4be2df1e377f9eaaa` | 通知注销转发 |

### DLL 选择、导出解析和返回值

- x86 `4480` 与 x64 `4b00` 的初始状态都是 `5023`。已有 `hLibModule` 时不重复加载；否则先检查 `GetModuleHandleW(L"RzChromaBroadcastAPI.dll")`，命中后选择 `RzChromaConnectAPI.dll`。未命中时检查 `RzChromaBroadcastAPI64.dll`，选择 `RzChromaConnectAPI64.dll`；两个检查都未命中时使用原件内置的备用宽字符串。该分支是当前原件的模块选择证据，不足以推断跨架构 ABI 或版本兼容。
- `LoadLibraryW` 成功后把句柄写入全局 `hLibModule`，再解析 `Init`、`UnInit`、`RegisterEventNotification`、`RegisterKeyboardEventNotification` 和 `UnRegisterEventNotification` 五个导出地址。加载失败只记录 `GetLastError` 的 LSTATUS，并保持默认返回 `5023`。解析阶段没有把五个地址全部作为成功门禁：最终仅检查 `Init_0` 是否非空；其余转发函数各自检查对应槽位。
- x86 `4480` 将输入按四个 32 位量传给 `Init_0`；x64 `4b00` 先复制输入的 `16 B` 到临时 `Block`，再将 `Block` 传给 `Init_0`。这是反汇编看到的调用形状，当前公开头文件、结构布局和调用约定仍未独立确认，Rust 不得据此猜测 FFI 签名。
- `Init_0` 存在时，外层直接返回底层 API 的结果；缺少 `Init` 导出时返回 `5023`。外层没有在底层 Init 失败后自动清除 `hLibModule`，因此失败状态和模块仍被加载可以同时存在。

### 通知转发和卸载

- `4650/4d60`、`4670/4d80`、`4690/4da0` 没有参数转换：有对应地址就原样调用并返回结果，没有地址就返回 `5023`。它们只是把已经由底层 API 建立的回调/通知状态向外暴露，当前函数本身没有帧、设备 ID、写入数据或硬件读回。
- `4610/4d20` 的默认结果为 `5023`；若 `UnInit_0` 存在则返回其值。随后只要 `hLibModule` 非空，就无条件调用 `FreeLibrary` 并把模块句柄清零，即使底层 UnInit 返回非零。`FreeLibrary` 的 BOOL 没有转成外层结果；因此外层返回 `0` 不能证明线程、共享映射、回调或设备资源已经释放。
- 包装 DLL 没有把加载失败、缺导出、底层返回值、FreeLibrary 状态合并成统一的生命周期状态。Rust 迁移时需分别建模“模块选择/加载”“每个导出地址”“底层 API 结果”“卸载结果”，不能把 `5023` 或一次 FreeLibrary 调用当成设备离线。

### 链路边界

本节将外层 `RzChromaBroadcastAPI` 导出连接到上一节的 `RzChromaConnectAPI` Init/UnInit/注册链，但没有声称 DLL 已实际执行。真实页面调用方、服务端 ACK、硬件传输、设备读回、取消、线程 join 和释放仍未验证；官方 DLL 仅作静态对照原件，生产代码必须由独立 Rust 实现替代，后续对照测试需先只读并保存初始值、写入后的读回和恢复结果。
## RzChromaBroadcastManager x86 与 API64 公开方法补充

本节补齐 `RzChromaBroadcastManager.dll` 的 x86 公开入口，并补记 x64 捕获中此前未列的 `IsRunning`、`GetSDKVersion`、注册和注销入口。这里只做当前原件的静态捕获、反汇编、伪代码与原始字节核验；没有加载 DLL、服务、管道或设备。

- x86 原件 SHA-256 为 `13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6`，大小 `226504 B`，Image Base `0x10000000`；[capture.json](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/capture.json) SHA-256 为 `bfb1e7f7e6dcd1662abd46128264418a138f6b5298dc8c2cedd2f4c001104338`，`capture_complete=true`。
- x64 原件 SHA-256 为 `555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51`，大小 `259784 B`，Image Base `0x180000000`；[capture.json](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/capture.json) SHA-256 为 `9f0f4b83ac6accbfb0eb04f075b0ce9fcbb70fd8a314b9a7d8b7f11edd100dc8`，`capture_complete=true`。
- x86 选取 22 个完整函数、22 个唯一 chunk、1222 条指令、`4102 B`；x64 补充 4 个完整函数、4 个唯一 chunk、339 条指令、`1821 B`。合计 `26` 函数、`26` chunks、`1561` 条指令、`5923 B`，chunk 原字节与指令字节逐项相等，未列字节为 `0`。`.work/chroma-connect-lifecycle-verify.py` 已将两个集合放入 `ThreadPoolExecutor(max_workers=4)`，并行核对输入 PE SHA、Image Base、函数 JSON SHA、chunk SHA／原字节及每条指令字节。

### 方法身份与函数记录

| 架构 | RVA | 函数记录 | 函数记录 SHA-256 | 静态用途 |
| --- | --- | --- | --- | --- |
| x86 | `21940` | [`21940.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21940.json) | `651b2484d7a13ae2d0cc72fa35a53df64d9e6e3d8f526cd039530df632841a9f` | `Init` |
| x86 | `219a0` | [`219a0.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/219a0.json) | `c8825c7c1e3b1d1a00a342aa752ece52978871b79564c2524b47b6a6ef10f898` | `UnInit` |
| x86 | `219f0` | [`219f0.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/219f0.json) | `070f05656842425f4aa427d552460c61f332394c8168a400fc33087058e8df73` | `SetBroadcastEffect` |
| x86 | `21c00` | [`21c00.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21c00.json) | `aa61ed1ca2481a2459e173c03f380f40cb7befb6b9c45b860e1aa17d4ce836fe` | `SetBroadcastEffectOnApp` |
| x86 | `21c40` | [`21c40.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21c40.json) | `3465abfbdfe78d25b3d08dbd64a064d972e4b9c100395bfb78105dc437e35919` | `SetKeyboardEffect` |
| x86 | `21e50` | [`21e50.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21e50.json) | `28cbbbed34efb74617b9bdfcbff50a3ae14a230159b330ebf5ef7013d001a3a7` | `SetKeyboardEffectOnApp` |
| x86 | `21e90` | [`21e90.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21e90.json) | `04eef0f4eb1fc73936114e8cdb4c9d6e529656fa5b54bbd151a8a7d81c8eafff` | `Enable` || x86 | `21eb0` | [`21eb0.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21eb0.json) | `e84ad5ce4d657cf015ffa5b0d19f5cbd85cc8819e6700d15eae88951cfe28e5d` | `IsEnabled` |
| x86 | `21ed0` | [`21ed0.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21ed0.json) | `4ff842f22d334eba29e08f09abd1c4c419b364d5026d7d20c584794bda29d590` | `IsRunning` |
| x86 | `21ef0` | [`21ef0.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21ef0.json) | `c146bbb38586f02ce1107a75f73754a0c0444556f63df2c727887552e0c2b42f` | `GetNumApps` |
| x86 | `21f10` | [`21f10.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21f10.json) | `2ed2a9e9fd713055fda2614d378dd3c4b3b7163ade8d41ba48b10e17f8997a92` | `GetApp` |
| x86 | `21f30` | [`21f30.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21f30.json) | `c48410c85848fbc2bb03153815f055723a367f58cc9fb67452067820c81e44ec` | `GetAppTitle` |
| x86 | `21f50` | [`21f50.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21f50.json) | `b3d50cc21a8d381f8f7d3bee7c8e6e96336f6403ac8a48b84603a7b3a2b035e5` | `GetAppPath` |
| x86 | `21f70` | [`21f70.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21f70.json) | `69d829705b1b0c64c976c3af879d25cefa251c6a1f55fe0a502c5845dc269bdc` | `GetAppIcon` |
| x86 | `21f90` | [`21f90.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/21f90.json) | `9e83ef1d0dd199d04ba1dffcae523e45893ae5b20ea4de49fa0ab7196b7e36c6` | `GetAppIndex` |
| x86 | `22120` | [`22120.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/22120.json) | `39832bb4bf732eb0f05b05f0ac005d6abb262940ccb0b3046d5228f8af3da1e5` | `GetDeviceSupported` |
| x86 | `222b0` | [`222b0.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/222b0.json) | `eb196ccad3fc8f3492bca03488effff6f3643746ce8a38754763cf0f446cd2ef` | `GetSDKVersion` |
| x86 | `222d0` | [`222d0.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/222d0.json) | `67de8396788f662e70e0b1bff84130a03df68ef3a6313b7062d8715158ec4820` | `RegisterEventNotification` |
| x86 | `22650` | [`22650.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/22650.json) | `103595caf41e922d1f2c71b55e1b4edeaf9f258cc11ba7908da587e19f5e841d` | `UnRegisterEventNotification` |
| x86 | `22930` | [`22930.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/22930.json) | `11f244fd6b9493e335f3fa28d08a09595f6498d19e2c1c8ae8a629c2b07b7727` | `InitGlobalBroadcastData` |
| x86 | `22980` | [`22980.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/22980.json) | `87ae6e2ef09e8b956ad69f624517d473258668641c9bd0b1d0b6395f5c0a71c7` | `UnInitGlobalBroadcastData` |
| x86 | `229c0` | [`229c0.json`](../../../.analysis/results/native/13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6/functions/229c0.json) | `7d844d8cbf91bfde04adef845ff685e15be21a37c2356548ad98fa7366567452` | `RefreshAppList` |
| x64 | `25fa0` | [`25fa0.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/25fa0.json) | `b3a9e55132d9c7a93b1b18504078ec0e6d228fd2b3073ac810a63e1d36620af5` | `IsRunning` |
| x64 | `26420` | [`26420.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26420.json) | `5d8a02b309c6c06737c82d09118eeda35e828b3634197f9bcb0b7aa0774c1fd4` | `GetSDKVersion` |
| x64 | `26440` | [`26440.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26440.json) | `b372f571222c79bfde75e4818bb62501bc3a14aa49e4b92269a3e271ec911af2` | `RegisterEventNotification` |
| x64 | `26820` | [`26820.json`](../../../.analysis/results/native/555ea9ceb8cbe8e627a4847d023e500274477986f63c9e453972db75a9c49f51/functions/26820.json) | `3a6c9076193b3fe02971d33a3be9c7961d6afea6bec12d002fdf2ffb52ed2457` | `UnRegisterEventNotification` |

### x86 初始化、效果提交与应用路由

- `21940` 的 `Init` 在全局 `Block` 已存在时返回 `1247`；否则分配并清零 `0xA0 B`，调用内部构造后发布 `Block`，再调用内部初始化 `sub_10009530`。捕获只显示该调用形状，不足以证明线程、服务或设备已启动。`22930` 的 `InitGlobalBroadcastData` 只完成同一对象的分配／构造并返回 `0`，不调用后续内部初始化。
- `219a0` 的 `UnInit` 只有在内部 `sub_10009D30` 返回 `0` 时才析构、释放并清空 `Block`；非零结果会原样返回并保留全局对象。`22980` `UnInitGlobalBroadcastData` 不走该内部停止结果，直接析构、释放和清空；这两个入口的生命周期语义不能互换。
- `219f0` `SetBroadcastEffect` 与 `21c40` `SetKeyboardEffect` 先要求 `Block`，再将 16 B 对象标识与原件内两组常量比较；有效分支创建 PPL task collection、调用内部效果任务并 `RunAndWait`，默认错误槽为 `1627`。标识不匹配时返回 `5` 并记录 Invalid ID。PPL 等待只证明本地任务完成，不是 HID 写入、设备 ACK 或读回。
- `21c00` `SetBroadcastEffectOnApp` 直接把效果和 app 参数转给 `sub_10009F30`；`21e50` 复制固定 `0x304 B` 的键盘效果数据后转给 `sub_1000A820`。两者仅检查 `Block`，未在包装层验证 app index、传输结果或读回，未初始化均返回 `5023`。

### x86 状态、目录与通知

- `21e90` `Enable` 只在 `Block` 存在时调用内部开关并返回其 `LSTATUS`；`21eb0` `IsEnabled` 未初始化直接返回 `0`，因此 `false` 不能区分未初始化、注册表缺项和明确关闭。`21ed0` `IsRunning` 同样以 `0` 表示未初始化并转发内部查询。
- `21ef0` `GetNumApps` 未初始化返回 `-1`；`21f10`、`21f30`、`21f50`、`21f70` 的 app、标题、路径和图标 getter 未初始化均返回 `5023`。包装层没有把失败变成空字符串或空列表。
- `21f90` `GetAppIndex` 与 `22120` `GetDeviceSupported` 先验证第一组 16 B 对象标识；`Block`／标识无效或注册表打开／查询失败返回 `-1`，空名称返回 `-2`。有效路径为 `HKLM\\Software\\Razer\\ChromaBroadcast\\Apps\\<name>`，分别读取 `Index` 或 `DeviceSupported` 的 DWORD，句柄和临时字符串在成功／失败分支都释放。这里读取的是本机目录元数据，不是设备能力实测。
- `222b0` `GetSDKVersion` 只在 `Block` 存在时转发；`222d0` `RegisterEventNotification` 校验第二组对象标识并把非空回调写入槽位，空回调返回 `87`、已有回调返回 `1242`，成功返回 `0`。`22650` `UnRegisterEventNotification` 对同一标识清除槽位；未注册返回 `85`，成功返回 `0`。这两个入口不停止接收线程，也不提供硬件确认。
- `229c0` `RefreshAppList` 仅在 `Block` 存在时调用内部刷新，未初始化返回 `5023`；返回值没有被包装成“设备列表已同步”的证明。

### x64 未列的四个公开入口

- `25fa0` `IsRunning` 的门禁与 x86 相同：`Block` 为空返回 `0`，否则转发到内部查询。`26420` `GetSDKVersion` 未初始化返回 `5023`，否则转发。
- `26440` `RegisterEventNotification` 先检查全局对象和固定 16 B 标识；空回调返回 `87`，槽已有回调返回 `1242`，成功写入槽并返回 `0`，标识不符返回 `5`。`26820` `UnRegisterEventNotification` 对同一槽清零；槽为空返回 `85`，成功返回 `0`，标识不符返回 `5`。与 x86 一样，返回值只描述本地包装路径。

### 迁移与验证边界

这些入口把对象生命周期、PPL 任务、注册表元数据和通知槽暴露出来，但没有闭合内部 app 任务、共享帧消费者、服务端应答、HID 写入、设备读回、取消竞态或线程 join。`Init`／`UnInit`、`RunAndWait`、注册表 `Enable`／`DeviceSupported` 和回调返回值均不得在 Rust 中冒充设备实测状态；生产路径仍须由独立 Rust 实现替代，官方 DLL 仅作为静态对照。没有设备时只能测试对象门禁、参数校验、模拟传输和有依据的应答回放，实际 DLL／服务／硬件对照保持未验证。
## Chroma Connect Server 的 WebSocket 帧处理与 Ping 生命周期

本节继续追 `RzChromaConnectServer.dll` 中此前消息队列段未覆盖的 WebSocket 帧读写、端点子进程启动和 Ping scheduler。分析只读已有 IDA 捕获，没有启动 Server、客户端 EXE、DLL、网络连接或设备。

- 原件 SHA-256 为 `a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5`，大小 `591048 B`，Image Base `0x10000000`；[capture.json](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/capture.json) SHA-256 为 `2ce03476a1e68777e86009029defa301a17d57723116a21d2ec6f6f3de7eaed9`。
- 本节新增 `6` 个完整函数、`20` 个去重 chunk、`2112` 条指令、`7673 B` 原始指令字节，未列字节为 `0`。`.work/chroma-connect-lifecycle-verify.py` 用 `ThreadPoolExecutor(max_workers=4)` 对新增集合并行核对输入 PE、函数记录 SHA、chunk SHA／原字节及逐条指令字节。

### 方法身份

| RVA | 函数记录 SHA-256 | 静态用途 |
| --- | --- | --- |
| `4e040` | [`4e040.json`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/4e040.json) `292fde8645115c0538d3068b6a052448d0cdd8b2250ae454e90ef395a9e90c98` | WebSocket `handle_read_frame` |
| `4ec80` | [`4ec80.json`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/4ec80.json) `fb2d42d4a651ae308ab4b5cbb16635e61fd1f28243542f99e13804fe351bba07` | WebSocket `handle_write_frame` |
| `5f6a0` | [`5f6a0.json`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/5f6a0.json) `2d30126be781a15d08d2bd6a7e61a44fa5909fc47bbb5751da8cee4d797a944d` | `WSEndpoint::CreateEndpointInstance` |
| `5fee0` | [`5fee0.json`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/5fee0.json) `3ca94a14f53c9eccb57405b9abe96dc37e7f9d60ea3feb06689e0674df812ada` | Ping thread wrapper |
| `60000` | [`60000.json`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/60000.json) `f1da14e7d5e81d33b8d17c2d569f69134bde7e1ac515710bf12483c73aec3c39` | `WSEndpoint::HandlePing` |
| `60470` | [`60470.json`](../../../.analysis/results/native/a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5/functions/60470.json) `aa3be97979ea395387a891285f983e00b20a4f5658941e062b5a8bca7cb05c4e` | `WSEndpoint::DeletePingScheduler` |
### 入帧解析、分片消费与关闭错误

- `4e040` 接收 `(error_code, bytes_transferred)`。错误值为空时，若连接状态不是 `7`，原件合成 processor 类别 `9`；随后按 WebSocket 错误类别处理 EOF、closed state 和普通传输错误。状态为 closed 时记录 `got eof from closed con` 或 `handle_read_frame: got invalid istate in closed state`，状态为 closing 且收到非 close 帧时记录并丢弃。错误路径不把“收到 EOF”转成设备离线确认。
- 无错误时，函数把新字节追加到 `this+2976` 的接收缓冲，并反复调用 processor 虚表偏移 `+52` 的 `consume`。每次消费前后记录剩余字节；processor 报告完成后从虚表 `+60` 取完整消息。消息为空只记录 `null message from m_processor`；消息操作码大于等于 `8` 时按控制／关闭消息路径处理。
- 关闭消息只有连接状态为 `1` 且 close handler 存在时才传给该 handler；closing 状态收到非 close 帧会记录 `got non-close frame while closing`。processor 的错误类别映射为：`3/7/8/9/10/11/12/13/24/25 -> 1002`，`4 -> 1009`，`5/16 -> 1007`，其余为 `1011`，再调用内部关闭路径。这里是 WebSocket 协议错误映射，不是 Chroma 设备错误码。

### 出帧队列与端点创建

- `4ec80` 是 `handle_write_frame`。它维护 `this+19500/19504` 的写队列边界；参数帧非空时调用内部写出函数 `sub_1002CD70`，队列为空且连接仍可用时发送内部空控制帧。没有立即发送数据时锁住 `this+2928`、清除 `this+19512`，若仍有挂起对象则构造 continuation 调度；返回值是本地 WebSocket 写调度结果，未包含远端应用或设备应答。
- `5f6a0` 的 `CreateEndpointInstance` 先拒绝负 session（`87`），从当前配置读取 `InstallPath`、`Software\\Razer\\ChromaBroadcast` 以及已保存的客户端路径；存在路径时追加 `\\bin\\RzChromaConnectClient.exe`，构造命令行和 session 参数。它查询活动控制台会话，再以 `WTSQueryUserToken` 与 `CreateProcessAsUserW` 启动客户端；成功把子进程 PID写回端点并关闭进程、线程和 token 句柄。无活动会话返回 `1168`，创建失败返回 `1631`，令牌查询失败保留 Win32 `GetLastError`，无路径返回 `3`。本项目不执行该 EXE；这里只记录 DLL 仍依赖外部客户端的静态边界。
- `5fee0` 只记录 `Thread_PingScheduler` 启动并转入 `60000`，不能从包装函数返回值推断线程已经完成初始化。

### Ping、超时和停止释放

- `60000` 的 `HandlePing` 从 `HKLM\\Software\\Razer\\ChromaBroadcast\\Apps` 读取 `TimeoutPing`；读取失败时默认 `30`。它等待 `this+124`、`this+132` 两个句柄，超时时间为 `1000 * TimeoutPing` 毫秒，并用 `this+136` 作为停止标志。
- 收到第一个事件时清除“等待 pong”状态；超时且尚未发出 ping 时，通过 `this+76` 的回调发送当前 session 的 ping，并置发送标志。已发送 ping 仍未收到 pong 时记录 `timeout, no reply (pong)`，再通过 `this+116` 的回调请求断开；其他等待结果映射为 `1211` 并停止循环。回调与定时器状态是连接健康检查，不是设备写入确认。
- `60470` 的 `DeletePingScheduler` 在任一线程／事件句柄缺失时预置结果 `6`，随后设置停止事件，等待线程最多 `2000 ms`，关闭线程句柄，重置并关闭两个事件，最后关闭停止事件。它忽略 `WaitForSingleObject` 的具体结果；超时仍可能留下执行中的线程，因此返回 `0` 不足以证明 Ping 回调、连接或下游设备资源已经释放。

### 边界

本节补齐 WebSocket processor 的入帧分片、协议错误、出帧队列、客户端启动和 Ping 停止链，但没有闭合 `sub_1003DFE0` 后的远端响应、页面调用方、设备 ACK／读回或 Rust 实现。所有 DLL、服务、客户端和设备运行结果继续保持未验证；静态映射和模拟回应不能替代官方实际 DLL 对照。

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

同步按钮查询当前 DEVICE_RUNTIME_DATA；当前代码过滤 setupStatus=ready 且 chargingStatus=off 的条目，再判断是否有 Chroma 设备。它不是只看产品能力表。按钮将 timerTick、selectedEffectId、effectSettings 写 window storage，由其他设备消费者处理；1200 ms 旋转仅为 UI 反馈。收到同一同步记录时再按 effectId 恢复本页选择。

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

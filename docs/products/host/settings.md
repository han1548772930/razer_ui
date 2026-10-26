# 全局设置：独立宿主、Synapse 与 Chroma

设置页管理应用的启动、通知、教程、迁移入口、灯光控制权、语言和版本信息。独立宿主设置另有 Software、Systray 和 General 三个页签。这里的大部分选项属于本机应用状态；它们不等同于某个设备配置档，也不经过设备设置的统一 Apply 按钮。本文只展开本机功能。

## 产品内的两套真实页面

| 应用与导航 | 左列 | 右列 |
| --- | --- | --- |
| Synapse → SYNAPSE | 自动启动、通知 | 重置教程、配置迁移、Windows 动态灯光 |
| Synapse → GENERAL | 语言、本地 Release Notes 入口 | 本机版本信息 |
| Chroma → CHROMA_APP | 自动启动、通知 | 重置教程、配置迁移、Windows 动态灯光 |
| Chroma → GENERAL | 语言、本地 Release Notes 入口 | 本机版本信息 |

Synapse 的入口先调用异步 SDK init，再开始懒加载模块 4914；外层 Promise 没有等待 SDK 内部初始化完成。该入口及其基础／staging 配置均未提供 appName，而 SDK 只在 appName 非空时注册本机 App 条目，因此不能用注册函数内的默认值证明此入口初始化过 synapse 自启。Chroma 的 async 入口等待 SDK init 完成后才挂载模块 1743，其基础／staging 参数同样未传 appName。各自的 Provider 包裹对应设置树，外层窗口焦点组件决定 body 是否显示。Synapse 使用 `windowName="synapse"`、`tabName="settings-synapse"`，Chroma 使用 `chroma-app`、`settings-chroma`。隐藏 body 与卸载组件是两件事。

[共享窗口交互专项](../../../.analysis/results/frontend/synapse-update-fw-current/page-interactions.json)另将固件工具栏到 Synapse 设置的实际 SDK／preload／main、policy=3 页签创建与目标 BodyWrapper 连接起来。当前 SDK 激活保留 subTab／willShow；同名 preload 便捷函数不是这个调用方。目标 focus hook 查询宿主可见性／焦点，消费 focus／changeActiveTab／activeTabChanged，卸载调用对应 response disposer；已发起的查询没有取消或旧代次拒绝。该目标页的 accessibility MutationObserver 未保留 disconnect，不能与下面独立 `/settings/` 根自己可断开的头像 observer 混用。请求发出、窗口激活、控件加载和设备读取分别确认；本专项没有运行窗口或证明所有设置动作已经接线。

设置根组件挂载时初始化共享频道与存储，读取语言，调用 `RazerApp.setBrowserTabVisible(true)`，监听 `beforeunload` 和 `languageChange`。关闭处理先设置 `showSettings=false`；窗口关闭回调随后 `window.close()`。当前组件卸载只直接移除 `beforeunload`，不能据此声称它注销了所有外部监听。

导航将历史列表和当前位置写入 sessionStorage。点击另一项会把新项追加到历史；工具栏前进／后退按当前位置派发 `updateActiveView` 和 `updateTabNavigation`；刷新调用 `window.location.reload()`。文档中的页面表来自真实渲染调用，而非 CSS 中保留的旧设置类名。

## 独立宿主 `/settings/`

独立窗口的原件、导航和 About 值源已由当前部署补齐，完整词法所有者及值绑定保留在[独立设置静态审计](../../../.analysis/results/frontend/host-settings-current/semantic-audit.json)。它与上面的两个产品设置页分别挂载、分别取版本，不能用 `/synapse/settings/` 的组件替代其证据。

### 两条启动入口与窗口生命周期

原生托盘菜单检查 `winMap` 中的 `razer-settings`；已存在则 focus，不存在则走宿主 `launchRazerApps`。宿主默认 origin 是 `https://apps.razer.com`，`SETTINGS` 常量为 `/settings`，拼接得到 `/settings/`；`application-host` 参数仅按官方 `getAppHost` 的 localhost／Razer 域名规则覆写。原生启动请求 1280×720、最小 600×500、policy 5。policy 5 先加载本机窗口壳，再让 Tab 加载实际设置 URL。

前端托盘齿轮是另一条真实入口：先隐藏托盘，再调用 `launchSettings("",{minimum_height:768,minimum_width:1000})`。该 SDK 默认宽高为 1000×768，使用当前页面 origin 加 `settingsPath`，参数序列化为 query；已有窗口则 activate。通知和 Widgets 的实际调用方还传入 `section:"notifications"` 并发送 `settingsScrollToSection` 广播。当前独立设置的 32 份自身 JS 中未发现该广播名的精确字面量；此扫描只界定已保留原件，不能证明不存在间接消费者，也不能证明不同部署间已经成功滚动。

页面 bootstrap 先初始化 SDK，再加载模块 8821，语言准备后才呈现根。初始化会登记应用／窗口事件、Escape、语言 storage 监听和定时器，调用 centralize；100 ms 后查询 `getWindowStatus`，在非最大化且 bounds 不是 1280×720 时调用 `setBounds` 请求该尺寸，再显示窗口。另一个 100 ms timer 也请求显示。托盘覆写的最小高度与该尺寸请求能否同时满足，须以实际宿主窗口结果核对，不能仅凭请求参数宣称最终尺寸。

初始化 effect 的返回值只直接调用 `window-list-change` 的 disposer，未为其余匿名事件、keydown、interval 和 timeout 提供完整清理。根类卸载时只断开自己可能创建的头像 MutationObserver。原件中的账户、反馈和联网编排属于来源说明，不进入本地优先产品范围。

### 实际导航与 General 挂载树

| 页签身份 | 实际加载 |
| --- | --- |
| `TEXT_SOFTWARE`，默认选中 | chunks 439＋921，模块 9302，读取安装列表 |
| `TEXT_SYSTRAY_SETTING` | chunk 762，模块 9762，Quick Launcher、General、Widgets |
| `TEXT_GENERAL_SETTING` | chunk 414，模块 3414，语言与 About |

页签选择进入 Redux 的 `selectedTab`，异步 Router 根据当前名字选择组件；导航同时处理 Enter、窗口宽度变化及溢出菜单。Products 不在这份挂载描述中。根传给工具栏的 `currentTabNavigation` 固定为 0、`tabNavigations` 固定为空数组，也没有传入 back／forward 回调，因此两项始终禁用具有当前源码依据。Refresh 则沿工具栏调用 `window.location.reload()`。

General 沿 `BodyWrapper → body-widgets → 两个 widget-col` 挂载：左列 GENERAL 包含语言，右列 ABOUT 包含独立 About；此树没有 Release Notes 子项。语言顺序为 `en/de/es/fr/ja/kr/pt-br/ru/zh-cn/zh-tw`，点击先关闭菜单，派发 `updateLanguage`，再调用 `RazerApp.setLanguage`。组件另注册匿名 `languageChange` 监听，没有对应 `off` 返回。Software、Systray 的实际本机写入、托盘消费者和原生安装名单查询另保留在 [local-controls.json](../../../.analysis/results/frontend/host-settings-current/local-controls.json)，边界如下；这些静态证据不代表功能已由 Rust 实现或实际 DLL 对照通过。

### Software 的安装名单、Open 与更新状态

实际列表取三个来源的精确名称交集：模块 5596 的固定 catalog、Redux `app.items`、等待原生查询返回的安装名称。它不是遍历 catalog 后直接宣称软件已经安装，当前 join 也没有消费 catalog 的 `canShow`。挂载、窗口 focus、原生 uninstall 事件会刷新；应用列表改变时还受缓存 `apps` 与 Redux 列表非空条件限制。focus／uninstall 回调有同身份移除，已经开始的查询没有取消或旧代次拒绝。

`installedModules` 只补版本／日期等本机元数据：初读取 `installedDate`，storage 事件分支却把 `installedVersion` 赋给 date；`uninstallingModules` 事件则直接替换这份 observer 列表。更新日期解析失败后的日期显示不能当成真实安装时间，更不能用本地元数据替代安装查询或设备读回。

| 操作／状态 | 当前完整调用链与结果边界 |
| --- | --- |
| Open，已有窗口 | 等待模块 4518 的 `getWindowServiceClients`，解析实际 Host 的 tab／window／webview 清单，按 `windowName===item.name` 查找；命中调用 `activateWindowServiceClient`。Host 再区分存活、休眠窗口／tab，恢复、显示或聚焦，并广播 appFocus |
| Open，没有窗口 | 执行 catalog 的实际 `path` 函数。Synapse、Chroma、Virtual Ring Light、Streamer Companion 分别打开各自本机页面路径及稳定窗口名，policy 为 5；Host 的真实 window-open handler 消费该请求。它没有把任意安装名单文字作为可执行文件路径 |
| Open 失败／确认 | 查询 Promise 被等待，activation 与 `path` 结果未被等待；该点击没有局部 JSON／IPC catch 或“启动完成”确认。请求返回不等于窗口已经完成加载；完整窗口创建／休眠仍由其生命周期所有者验收 |
| 卸载、修复及补丁说明动作区 | 三项 `className:(item.canUninstall,"disabled")` 均恒为 disabled；共享 CSS 的 `.disabled` 明确 `pointer-events:none`。前两项没有 onClick，第三项虽保留补丁说明 callback，也不能当作可点击的功能 |
| Auto Update | 固定 disabled，没有可执行点击链。子项把 update callback 登记到父组件 Map；完整词法扫描只找到 Map 声明与 `.current.set` 两处引用，没有读取、遍历或调用这些 callback 的入口 |
| 更新进度 | 先观察 window-storage，再读 `<app>_installer-status`，按 downloading／installing 分支分别查询下载／文件进度；事件与初读使用字段标记避免旧初读覆盖已经到达的事件。Background Manager 单独读取，不能并作应用本身的进度 |

进度事件把变更后的 restart-required／skipped／completed 归为 completed，canceled 和 error 保留各自状态；下载 `-1` 转为 100。observer 卸载会 off，但在途 Promise 未取消，这些字段标记也不是页面实例代次。百分比、completed 或本机 `installedModules` 存在都不能证明真实安装成功。

不能因为 Map 没有调用就断言更新类完全不运行：`setDefaultData` 仍注册 online listener，online 事件可以取远端 manifest、写缓存和启动 timer。子项清理仅登记 noop 并清空 React ref，没有移除更新类的 online／hasUpdate 监听，也没有证明 timer、锁与请求已释放。该联网编排不进入本产品；原件补丁说明弹层中的 2019 年日期、1.0.0 和 50 MB 是固定样例，不能当作当前 Release Notes 或本地安装生命周期的实现。

### 安装名单的实际 Windows 原生函数

请求沿 `5507 → 4518.getInstalledRazerAppEngineProduct → preload.doElectronAction → Host GetInstalledRazerAppEngineProduct → sysutil/win.callDLL → ffiMain.callDLLMain` 进入当前原件。Windows ABI 为无参、返回 pointer，释放为 `FreeMalloc(void(pointer))`；这条调用走同步 ffiMain，不走异步 DLL 子进程。官方 DLL 仍仅是静态分析及独立对照对象，产品应以 Rust 独立实现该本机 OS 查询。

实际导出 RVA `0x73ff0` 跳到 `0x69590`，打开 `HKLM\SOFTWARE\WOW6432Node\Razer\RazerAppEngine\Dependencies`：首次 access 为 `0x20119`，失败返回 null；成功后关闭该句柄，再以 `0x20019` 获取枚举句柄。`0x5c980` 使用 `RegQueryInfoKeyW／RegEnumValueW` 只枚举值名，不读值数据，也不验证可执行文件、版本或安装健康。名称按枚举顺序用逗号拼接，没有转义。

枚举 helper 的 false 被调用方忽略，中途失败可能保留部分名单；第二次打开失败也可能返回分配后的空字符串。因此 null、空字符串、部分列表与确实无值须分别测试。原件转换用 UTF‑8 codepage 65001，分配字节长度加 1 并补零。Host 在 `readCString` 成功后才设 free 标记，再由 finally 调用 `0x6b9d0 → 0x97ce0 → 0xa98e0 → HeapFree`；读取字符串先抛错时不会进入这次释放分支。9 个选定函数的完整汇编、伪代码和实际 PE chunk 已逐项校验；未展开直接调用、import／间接边界仍保留，不能升级为整个 DLL 语义闭合。

前端仅把四个精确值名映射为内部身份：`Razer Synapse Beta／Razer Synapse → synapse`，`Razer Chroma Beta／Razer Chroma → chroma-app`。其他名称原样保留，没有 trim、大小写归一或去重；异常、false、空返回均折叠为 `[]`。这个来源能证明注册表中观察到的名称，不能单独证明安装完整，也不能把空 UI 当作成功验证“未安装”。已保留的平台选择器另有 Mac adapter，本导出没有在该分支建立等价链，不得据 Windows 结果推断跨平台支持。

### Systray 的五槽快捷启动与双击

Quick Launcher 固定呈现 5 槽，存储为 `launchers` JSON。无有效数组时，取非 website 应用按 title 排序、截前 5；数组内 null 变为空对象。初始化等待原生安装名单，删除已经不在 Redux catalog 的项及原生名单中缺失的 `type=app` 项，再立即写回。应用列表变化和 uninstall 重新执行，observer 有 off，在途初始化没有取消或代次核对。

选择 None 把本槽替换为 `{}`；选择已有应用会先清空它的旧槽，再赋给新槽，避免重复。None 的 disabled 条件取候选 `k` 列表长度，不是当前已填槽位数；下拉 CSS 禁止 disabled 项点击。state 与 JSON 立即更新，没有页面级 Apply／Save，也没有设备写入。

预览和候选 `k` 在同步 filter 中调用 `RazerApp.isAppInstalled`，但当前 Electron SDK 对已知应用返回 Promise。Promise 的 truthy 不能当作等待后的安装结果；另一个实际等待的原生名称过滤独立存在。真正托盘模块 9001 的 `Sr` 读取同一键，也有这一同步 Promise 边界；storage 的 `launchers` 事件调用 `Sr` 刷新，实际 launcher 组件还用 native `installedApps` 单独过滤 Synapse／Chroma。

真正点击进入 `9001.I0`：隐藏托盘，走应用 launch、focus／restore 或 policy 5 的 URL 打开。其原件包含账户判断、延迟闭包和 website 分支，不能照搬回本产品的去账户范围；本机 launch 参数、窗口结果、失败和释放需独立接通并测试。显示一个快捷图标或写入 `launchers` 不构成启动成功。

双击选择使用 `systrayDblClickAction`，缺值默认 `{type:"showMenu"}`；其他选项为非 website 应用的 `{type:"launch",payload:name}`。选择立即关闭并保存。实际托盘 `Je` 消费 `electron-action`：showMenu 显示托盘，launch 读取窗口后经原件登录条件进入 `I0`；其备用 catalog 的字段也不能当作已核实的通用 launcher。`Je` 返回真实 response disposer，应用列表改变时重绑。托盘 root 另有匿名 storage／languageChange／appExit 与 interval，卸载只移除 focus listener，不能据局部 disposer 宣称全托盘清理完成。

### Widgets 的显示偏好与真正设备动作

根按 `hasWidget` 和 widgetIndex 呈现，存在 Spatial Audio 时排除 sophie-lite。总开关写 `widgetsHidden`，拖动写 `widgetsSort`，再调用真实 `4672.bN` 重建应用项。该重建等待 SDK 安装观察并查询 Host 窗口，其 manifest 版本元数据与设备状态分别存在。真正托盘 `Oe` 消费 `showWidget&&hasWidget`、同样的排除规则与顺序，再用原生安装名单过滤 Synapse／Chroma，按应用 lazy 挂各自 widget。

| 设置键／分组 | 设置页写入与实际托盘消费者 |
| --- | --- |
| `synapseWidgetsSort／synapseWidgetsHidden／synapseWidgetsBatteryHidden` | 控制稳定产品身份的顺序、设备卡及电量显示。设置页读取 Host `connectedDevices`，实际托盘 5492 另行读取并 remap；这些键不改变设备电源／电量，也不证明设备能力 |
| `chromaAppWidgetsHidden` | 设置页只消费 `connectedDevices` 并写 studio／visualizer／设备 name 的隐藏名单。托盘 2215 选择 studio／visualizer／connect 组件，1899 再过滤 Connect 设备；隐藏选项不调用设备 enable |
| `spatialAudioWidgetsHidden` | `spatial-audio-on／profiles` 控制本机音频 toggle／profile selector 的显示；真正动作仍由对应音频消费者完成 |
| `cortexWidgetsHidden` | `game-launcher／button` 控制游戏列表／Boost 按钮的显示；真正点击才发送游戏／Boost／Restore 广播，不能以隐藏名单代替本机执行结果 |
| `goldAndSilverWidgetsHidden` | `gold／silver` 属于原件账户显示，仅记录来源，不进入本产品账户功能 |

各子组在“全部隐藏且根仍显示”时可触发真实根 toggle。设置页 Synapse 观察要求应用运行，再合并 productId／primaryPid、serial／primaryDevice，保留 ready 并排除明确 chargingStatus=off；只有数组长度变化才发布新列表。同数量替换、元数据或语言改变时可能留旧值。真正托盘另要求 serialNumber，且有 off 重复项处理，与设置页筛选不同；隐藏事件重新读取，排序／电量事件映射现有列表，不能合并成同一个观测算法。

Synapse 设置项的 appLaunched 有配对 off，但匿名 `T.on("windowstorageevent",…)` 没有；拖动的 mousemove／mouseup 有移除。真正托盘 window-storage 回调有 unregister 与 response disposer，已经开始的异步读取仍无最终代次核对。共享 observer 另在 beforeunload 注销，但没有保留 Electron response disposer；不能把关闭窗口或清空引用当完整取消。

Chroma 的真正设备 enable 是另一个动作：托盘 1899 的开关才调用 hook 的 `onChange(device)`，投递 `{action:"setAppEnabled",payload:{name,enable:!旧值}}` 到 `lighting-engine-chromaConnectDevices-action`。Host 将值保存在按来源 URL 分组的内存 Map，再向其他符合条件的 webContents 发 memory 事件；setter 返回旧值，不是服务确认。GET 汇集各来源的 truthy value，UI hook 只读第一项，没有来源／代次选择，不能把这份缓存直接当实测。

当前 LE 的真实 `MemoryStorageHandler` 持 mutex，按模块 1876 的 `HR` 常量进入 5976，调用 `setAppEnabled → send({func:"Enable",param:{name,enable}})`，服务地址为本机 `ws://127.0.0.1:1337/razer/rzchromabroadcastmanager`。列表事件与 memory 发布再供 UI 观察；投递完成和服务等待结束都不能当实际设备输出确认。完整服务状态、请求 token、超时与释放边界见 [chroma-connect.md](chroma-connect.md)。

还须保留一个精确差异：设置／托盘 hook 的 `MEMORY_STORAGE_CHROMA_CONNECT_APPS_ACTION_KEY` 实际为 `lighting-engine-chromaConnectApps`，LE 真正 Apps action 的 `Op` 却为带 `-action` 的键。当前设置只改显示，实际托盘只挂 device toggle，均未调用这个 apps 分支；不能因 helper 定义存在就宣称 Apps enable 已接通。hook 的 listener 有 off／unregister，singleton 另有注册及未保留 disposer，在途读无取消；本轮未完成原生服务／DLL 的实际结果对照。

### 独立 About 的真实版本 producer

About 的显示数字初值为空。挂载后先 `caches.match(new URL("/systray/systrayv2/manifest.json",location.href),{ignoreSearch:true})`；命中时直接取缓存 JSON，不再 fetch。未命中才 fetch，并仅在 HTTP 200 时解析网络 JSON。正常显示表达式是 `(e.version || "").replace("1.","4.") + "." + e.buildVersion`，这里的替换作用于第一次出现的 `1.`。

本次通过相同获取收据独立保留的托盘 manifest 给出 `version="1.0.14"`、`buildVersion=2609150422`，按上述表达式静态求值得 `4.0.14.2609150422`。这是输入与算法核对，没有运行页面。独立设置自身的部署版本为 `0.0.41.2606221239`；宿主版本和两个产品 Dashboard 的版本同样不能代替这个 About 值源。

非 200 且没有缓存时，原 helper 返回仅带 `buildVersion:0` 的对象，显示表达式得到 `.0`。缓存读取、fetch 或 JSON 解析抛错才进入 catch：同步调用 `RazerApp.getAppData("chroma-app","version")`，再显示 `"4." + 返回值`。该 API 从 `apps` 中按应用名读取字段，没有应用或字段时可能产生 `4.undefined`，原件没有补充校验。About 的 async effect 没有取消或旧代次拒绝，不能把组件卸载视为已终止请求，也不能把这个版本元数据回退扩展为设备读取失败后的默认值回退。

后续对照需分别覆盖缓存命中且不联网、未命中／200、非 200、缓存／JSON／网络异常、应用字段缺失、切换页签期间延迟返回和关闭清理。外链、版权及社交组件仍按本地范围规则筛选；不能因为整份组件被保留为分析证据就把联网业务重新挂回产品。

## 自启和启动时最小化

两个复选框的组件初值均为 true。挂载 effect 先发起 `RazerApp.getAppAutoStart(appName)` 异步读取，再同步调用 `getMinimizedOnStartUp(appName)` 从应用记录读取，只有非 undefined 值才覆盖最小化的组件初值。主自启关掉时，子复选框使用 CSS 禁用外观与鼠标事件；其实际 DOM 边界见下文。

点击自启先调用 `setAppAutoStart(appName,value)`，再更新页面；最小化同样先调用 `setMinimizedOnStartUp` 再更新页面。这两个 setter 更新应用项的 `isAutoStart`／`isMinimizedOnStartUp` 和 `lastUpdated`，然后重算整机启动参数。页面没有等待注册表写入确认，也没有通用失败回滚或写后 GET。

自启异步 GET 与编辑之间没有请求代次或修订比较，组件卸载也没有取消该 Promise；若返回晚于点击，原页面的 GET continuation 仍可再次设置 checkbox。两个点击回调不等待 setter：SDK setter 返回本机应用记录或未找到记录时的 undefined，`updateAppsAutoStart()` 的底层写入不作为其返回值。因此页面选中、本机记录更新和 OS 启动项更新是三个阶段，不能把 UI 的立即变化或返回记录当作系统写入确认。

聚合过程遍历本机应用列表：

1. 收集 `isAutoStart===true` 的应用，身份取 `name2 || name`，保留列表顺序。
2. 没有自启应用时调用低层 `setAppAutoStart(false)`。
3. 有自启应用时构造 `apps=<逗号分隔名称>&autoStart=1`。
4. 另收集所有 `isMinimizedOnStartUp===true` 的应用，追加 ` --launch-force-hidden=<名称列表>`。这份列表由最小化字段筛选，并未再与自启列表取交集。
5. Electron 分支去掉 `&autoStart=1` 再调用 `setApplicationAutoStart`；非正式 host 分支还附加 `--application-host`。

当前 Windows 原生实现打开 HKCU 的 `SOFTWARE\Microsoft\Windows\CurrentVersion\Run`，值名是 `RazerAppEngine`。`applicationAutoStart`（RVA `0x6e2d0`）以 lpData=NULL 查询该值，类型和长度输出指针非 NULL，但这两个输出不参与布尔结果；查询成功即 true，不检查类型、命令内容、目标文件或启动健康。普通返回路径的打开／查询失败为 false。

`setApplicationAutoStart`（`0x6e3c0`）只打开既有 Run key，不创建。开启时将传入的 NUL 结尾 UTF-8 转为宽字符，向 512 wchar 缓冲格式化 REG_SZ：`"<启动程序>" --url-params=<参数> --autoStart=1`，写入长度包含 NUL；关闭时删除该值。导出返回 void，打开失败静默返回，set／delete 失败仅日志，正常持有句柄的路径会关闭。不能从 IPC 完成或格式化失败后仍无布尔错误推断写入成功。

启动程序路径从 `GetModuleFileNameW(NULL,260)` 得到，即当前进程的主 EXE。函数分离父目录与文件名，区分大小写地查找父目录中首次出现的 `app-` 子串；找到时保留其前缀，直接拼接原文件名。它没有验证该子串恰为版本目录；未命中才保留原路径。该分支的父目录裁剪 `0x87110`、拼接 `0x7a230` 和查找 `0x893b0 → 0x89050` 均已保留完整函数和原始指令。

`getApplicationAutoStartApps`（`0x6e950`）先查询 cbData，再把该长度直接传给固定 2048 字节宽字符缓冲，没有核对上限、类型、实际长度或 NUL 终止。第二次查询后，先对 Data 做 NUL 扫描并构造宽字符串，到 `0x6eabf` 才检查查询状态，不能描述成失败时不消费缓冲。正常成功路径区分大小写地查找任意位置的 `--url-params=`，将命令转为 UTF-8，经 checked-copy 写入 1024 字节临时缓冲，再取 marker 后到首个 ASCII 空格或 NUL 的 token。marker 后立即结束或空格时，分配成功会返回一字节空串；普通缺失／查询失败可返回 NULL，异常、越界扫描、invalid-parameter 和 allocator/new-handler 路径不保证正常 NULL 返回。

上层只对 string 执行 `String.includes(appName)`，非 string 为 false；它没有精确解析 App 列表，原生也不解析 App JSON 或验证启动目标。原生分配沿 `0x9a1d0 → 0xa9940 → HeapAlloc`；释放沿 `FreeMalloc → 0x97ce0 → 0xa98e0 → HeapFree`。实际宿主 sysutil 使用同步 `ffiMain`：先 readCString，成功设置标志后才在 finally 中 FreeMalloc；readCString 抛错会跳过该 free 分支，不能称所有转换路径必然释放。宿主启动时另外把 `launch-force-hidden` 按逗号解析，因此自启与隐藏是独立条件。

Synapse 的两个点击回调每次都重新构造 `debounce(Xs,2000)` 并立即调用其返回函数。模块 6079 的默认值为 leading=false、trailing=true，因此每次点击各自启动一个尾沿 timer；回调没有保留同一个 debouncer，连续点击不会在这里合并成一个 2 秒后的 storeData。`Xs` 发送 `{action:"storeData",windowName:"settings-synapse"}`，返回函数和 timer 未被页面保留，卸载未取消。Chroma 当前分支只构造 `debounce(ks,2000)`，没有调用返回值，因此这两个表达式本身不启动 timer，也不触发 `settings-chroma` 的 storeData。两套页面不能归并为相同的延迟保存语义，底层自启 setter 也不依赖该后续表达式才能执行。

Synapse 的复选框类用 crypto 生成实际 ID，没有采用调用方的 `autoStart/minimizedOnStartup` ID。input 只有 type／id／checked／onChange，未设置 disabled 或 ARIA-disabled；onChange 只停止传播后调用父回调，没有再次检查禁用。CSS 隐藏 input，以 20×20 px label 和伪元素画勾，子项缩进 30 px、竖线高 37 px；`.check-item.disabled` 使用 opacity .3 和 pointer-events:none。四个可见词条已逐项解析实际 importer 的十种语言，字体依赖为四份 Roboto／RazerF5 normal 400 原件。这些静态规则不证明键盘、焦点或像素验收通过。

### Chroma 自启的独立挂载与保存

[Chroma 自启静态收据](../../../.analysis/results/frontend/chroma-app-settings-current/local-startup.json)另核对 209 个完整 AST 所有者、281 个角色、71 个绑定、49 个 exports、54 个词法 CSS 块、十语言四词条与四份字体，共 100 条静态断言和 926 个 artifact。复用的完整捕获为 367 个原件、510 个输出，其 validate 失败仍保留。独立复核重新校验当前部署、每个原始 AST／CSS 边界、共享宿主 ASAR 来源及所引用的原生边界。

实际挂载为 main 的 `E().then` → 714／602／75 懒加载 → 1743.default → `Go` Redux Provider → `Bo/Zo` → `hi` 页面状态 Provider → `Ei` → CHROMA_APP／默认 `mi` → 左列 `Hs`。GENERAL 不挂载该自启组件；带 `_settings_chroma-app` 后缀的 sessionStorage 导航历史与 SDK apps 数据分别维护。init 未提供 appName，不足以触发 SDK 的 App 注册 gate，不能将组件 true 初值解释成注册或系统写入结果。

两个点击仍立即执行 SDK setter、更新已有本地 apps 记录并发出宿主自启命令。与 Synapse 的差异只在后续 `Us()(ks,2e3)` 没有调用返回函数：完整 debounce 工厂的四处 setTimeout 均位于延后调用的函数中，这两个表达式未调度 2 秒后的 flush。`ks` 在所核页面工厂只有声明及这两处传参引用；该事实不排除整应用的其他保存路径。若其他已核路径执行 `ks`，其目标为 `storeData/settings-chroma`，宿主 session flush 没有返回保存确认。

两套控件的所选 SDK 数据、getter／setter、平台和 Electron 所有者分别按完整 AST、非 identifier 词法及一一变量映射比较；复选框的 constructor／update／render 独立核对，不能用整类变量名替换证明绑定。Chroma 同样没有 HTML disabled 或回调内 disabled 检查，禁用来自外层 CSS；语言 importer 的每个 locale 已连接实际 chunk／module。背景、字体、widget、树线与 note 同时保留 media 和末尾覆盖顺序，计算样式、GPUI 像素和键盘行为仍未运行验收。

当前 [Chroma 页面](../../../crates/razer-app-pages/src/chroma/chroma_settings.rs)以本地 preferences 初始化草稿，点击经 [state owner](../../../crates/razer-state/src/chroma_settings.rs)立即排队写 `chroma-settings.json`；没有两秒计时器，也没有原件对应的 OS 初读、per-app 聚合或真实自启写入。Shell 的 `PersistenceFinished` 只结算本地保存，另一实体的 `StartupRequested` 才进入 OS 适配器。全工作区 1,690 个 Rust 源文件的 AST 与消费者检索未找到 Chroma 到该适配器的生产链，不能凭共享模块存在认作已复用。

当前 Rust 另有最小化回调 guard、语义 disabled、未保存阻止关标签、retry／discard；所核官方 `Zo.unload` 没有这套 dirty 门禁。Rust retry 仅重排本地写入，discard 从 committed 本地副本重建草稿，均不代表 OS 恢复。独立 Chroma 标签关闭、主窗口 host tab 关闭、直接 OS 窗口关闭及全应用退出各有不同所有者，必须分别测试，不能把其中一个关闭检查泛化为全部窗口。

### 启动记录的实际注册入口

[同一自启收据](../../../.analysis/results/frontend/synapse-settings-current/local-startup.json)的 `application_registration` 另保留 356 个完整 AST 所有者、432 个角色、86 个绑定、48 个导出和 191 条静态断言，并比较 84 组完整函数。三个实际 dashboard 和两设置 SDK 的所选 init、注册、读写、正规化、聚合及平台函数分别按完整 AST、非 identifier 字节与一一 identifier 映射核对；这不代表整个 SDK 等同。四套完整捕获、六套部署来源和既有失败均独立保留。

设置入口不传 appName 只说明该入口不注册。真正的 Synapse dashboard 在两个保留部署中均经普通 dashboard 分支加载 SDK、以 `appName=synapse` 调用并等待 init；`#version` 与 `iotPopup` 旁支没有据此验收。Chroma dashboard 用 `appName=chroma-app`。三者均未传 appAutoStart，由 SDK 缺省为 true，appMinimizedOnStartUp 显式为 true；页面几何恢复的 absent→false 是另一处逻辑。各部署版本、原件和完整参数保留在收据，不能用某个入口版本代表全部页面。

SDK 仅在非空 appName 及 appData／manifest／engine gate 满足时注册；这三个 dashboard 未传 manifestURL，满足相应条件。`initAppDataItem` 按精确 name 查找：新条目追加，已有条目先删除本次参数中的两个启动字段，再合并旧条目和新元数据，因此保留旧启动布尔值，同时更新 url／urlPath、createdAt／lastOpened／lastUpdated 和 lastState。缺失或非 boolean 的旧 isAutoStart 使用 `getAppAutoStart2` 原样结果，最小化字段没有对应正规化；Electron getter 未等待原生命令结果，不能称已读回并修正为确定 bool。后续平台初始化失败也没有已证实的记录回滚。

注册随即写 `apps` 并重建整机启动命令。自启和最小化名单分别使用严格 `===true` 筛选、`name2||name` 编码；自启名单为空时关闭自启，否则形成 apps 参数，隐藏名单独立追加。设置 setter 对不存在的条目不创建，但仍会重建全列表。修改记录和发送宿主命令均没有 OS 写入确认。

Chroma 还有顺序 gate：先查询窗口，发现 Synapse 窗口时等待 `apps` 中出现精确 synapse 条目；不存在时保留 storage listener，等 apps 事件后再查。没有 Synapse 窗口则直接调用初始化包装函数。该包装函数既非 async，也不返回内部 init Promise；bootstrap 的 await 只等待窗口查询，不能作为注册完成屏障。此处没有已证实的 catch、取消或卸载退订。

dashboard 的几何更新会写窗口扩展字段并显式请求 `storeData/实际appName`，与设置点击的两个 timer 分支不同。其 resize debouncer 返回函数确实被调用，但未保存 cancel／flush；移除 listener 不能保证在途 timer 已取消。SDK beforeunload 也未提供 apps 落盘确认，宿主 session.flushStorageData 的结果没有返回或被等待。跨页面 storage 事件、重启落盘及真实启动仍未运行验证。

background 的卸载入口会查询应用并打开 installer；选定清理所有者按精确 name 从 apps 删除条目，另写卸载标志并请求 flush。该所有者内没有启动参数重建，不能由局部缺调用推断整个卸载链均未重建。下述原生脚本另有实际 Run 修改路径；installer／native 的真实结果、失败中断后的清理及辅助 dashboard 分支仍须分别核对。

### 卸载结果消费与启动项清理

[同一自启收据](../../../.analysis/results/frontend/synapse-settings-current/local-startup.json)的 `application_uninstall` 追加核对当前 background、installer、宿主和旧 SDK 的 147 个完整 owner、159 个角色、89 个绑定、59 条静态断言和 36 条调用边。后台 `KA` 启动核对会直接走 `WA/jA`，而后续 storage 清理由 `xA` 闭包中的 `t` 控制；两条路径不能合并成同一清理保证。Chroma 有 `shouldManualRemove` 的精确 apps 名称过滤，Synapse 的选定 owner 没有同样开关。

驱动卸载 wrapper 的宿主回调形状是 `{result, reason, exitCode}`，installer 只按整个对象是否 truthy 继续删除 `installedResources`，不检查 `result` 或 `exitCode`。普通文件 wrapper 则返回 `result` 布尔值；pending 清理把 truthy 返回当对象执行 `"result" in t`，当前静态链在返回 `true` 时会进入类型错误路径，`false` 才保留 pending。这些是源码类型与控制流事实，未执行厂商 JavaScript，不能改写成实际现场异常。

卸载记录删除、`COMPLETED`、进度清理和窗口关闭不等于文件、驱动、进程或注册表 Run 项已成功处理。宿主在卸载 CLI 分支把应用名加入 `Fe`，托盘据此抑制应用项重建，收到匹配的 create 事件才移除；它不是窗口创建 gate。原生 `--uninstall-app` producer 和两产品的 Run 修改已沿下述脚本补齐，最终 `Mn` 的 JS 分发顺序也已保留；原生释放完成、服务端真实删除结果、注册表效果、取消恢复和重启落盘仍未验证。纯逻辑／模拟测试须覆盖这些结果形状和失败顺序，再由显式对照入口记录实际 DLL／服务结果及可恢复写入的原值、读回和清理。

### 原生卸载脚本的 CLI、Run 与退出

同一收据的 `native_uninstall_producers` 将已保留的官方 Synapse／Chroma `4.0.827` 发布声明、NSIS 外层及内层 Setup Factory 原件逐层绑定。两个 `auxiliary/irsetup.dat` 合计保留 342 个完整词法 Lua 函数、94 个记录、20 个生命周期挂载、78 个有界引用、44 条调用边和 34 条静态断言。独立复核重新解压两层容器、核 CRC 和完整原始字节，再核词法函数及事件边界；该分析不是 Lua AST，也没有执行脚本或安装器。当前性限于获取收据的观察窗口。

`On Pre Uninstall → OnPreUninstall` 先移除本产品的 AppEngine 依赖。无剩余依赖时，`StopRazerAppEngine(gAutoStartName)` 构造 `--exit-application --uninstall-app=<app>`，随后进入整引擎及用户数据卸载；引擎需保留且进程存在时，单应用函数只构造 `--uninstall-app=<app>`。两者均通过 `File.Run` 指向 ProgramFilesFolder64 下的 `Razer\RazerAppEngine\RazerAppEngine.exe`，使用引擎工作目录、`SW_MINIMIZE` 和 wait=true。Synapse 名称为 `synapse`，Chroma 为 `chroma-app`；组合退出参数进入宿主的退出优先分支，不能与单应用卸载消息分支混同。

单应用函数只记录 `File.Run` 返回值，没有检查它证明卸载完成。等待 10 秒后，日志存在时最多读取五次，以应用名子串查找 `appLaunchedList.log`；日志缺失或读取错误才设置停止引擎标记，五次仍含名称不会设置该标记。仅停止标记为真时删除用户数据中的产品文件夹，备份 Apps 文件夹另行进入删除函数；`Folder.DeleteTree` 结果未在此函数确认。停止函数还有等待、进程重枚举和仍存活时的终止调用，这些调用与设备／服务已释放分别判断。

`On Post Uninstall → OnPostUninstall` 确实调用 `UpdateAutoStartRegistryRemove`。它读取当前用户 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 的 `RazerAppEngine`，按空格分割命令，再处理 `--url-params=apps=` 与 `--launch-force-hidden=` 的逗号名单，忽略大小写移除当前应用。apps 清空时删除整个值，否则重组写回；遍历使用 `pairs`，不能额外承诺列表顺序、完整引号语法或其他用户覆盖。写入类型的原文为 `REG_SW`，所核 Constant Definitions／Global Functions 未定义该名字，原生解析仍未闭合，不能自行改称 `REG_SZ` 或推断写入成功。此处没有写后读回；整引擎删除另有显式 HKCU Run 删除。

`RestartRazerAppEngine` 虽有定义，但 `OnPostUninstall` 中唯一同名调用在长注释内，不能按函数存在或 relaunch 标记认作真实重启。实际 `On Shutdown` 卸载事件只走 `OnShutdownUninstall → My_HandleSystemReboot`；需要系统重启时，静默模式返回 3010，交互模式按询问结果处理。安装完成的 `OnShutdown` 则另按 `gLaunchApp` 调用启动 helper，不能套到卸载结束。

当前包内脚本和挂载已经确认；安装时生成的 `Uninstall.exe／uninstall.xml`、原生 API 传递实现、真实进程／注册表／文件效果、错误与取消恢复、Rust 独立实现及实际 DLL 对照仍未验证。脚本中的调用存在不能替代成功应答、恢复或清理验收。

### 宿主最终退出清理消费者 Mn

当前宿主 `electron/main.js` 的 Mn owner 位于 UTF-8 半开区间 `[23367,24455)`。同文件的 Un owner `[37592,38836)` 在指定应用时只关该窗口；全应用分支先倒序请求关窗，10 秒后重试，再过 5 秒调用 Mn，随即另排 5 秒后的 `app.exit()`，不等待 Mn 完成。`window-all-closed` 的 async handler 也调用并等待 Mn，带独立 then／catch 消费者。强制退出计时不能证明清理已完成；本次未执行窗口或宿主。

收据的 `final_uninstall_cleanup` 保留 41 个角色、39 份按内容与范围去重的完整 AST owner、36 项断言、16 条调用边和 28 个证据文件。三个平台选择器虽共享同字节内容，仍各自保留来源路径；相对导入按所属模块解析，不能合并模块身份。Mn 自身没有在途重入门禁；两条调用路径可能各自进入清理，`isQuit` 赋值不能作为单次释放屏障。

Mn 先设置 `b.isQuit=true`，移除 `winMapChanged` 的托盘重建 listener，调用 `Fn()`。Fn 是 `ln(zn,250)` 返回的 cancel 解构绑定，所用 factory 为 `createDebouncedSystrayRebuild`，用于清除尚未触发的托盘重建 timer。随后等待 `Je.callDLLAsync` 的 `APIToCallWhenExit` 批次；存在 preload owner 时先同步调用 `callApiWhenExitDevice`、`callApiWhenShutdown`，仅 `callApiWhenExit` 带 await，再调用 `ae()` 所绑定的 `setExitEvent`，不等待其底层结果。

其后是 SysUtils `Terminate`、USB `An.stop()`、BLE `Qo()`、mapping engine `mappingEngineShutdown`、Simple Service `simpleServiceShutdown`、await logger `N()`，最后 `app.quit()` 再销毁 tray。三个原生关闭调用各包在独立的 3 秒 timeout／try-catch 中，捕获只日志后继续；timeout 的 Promise.race 不取消底层操作、不清除其 timer，也不能截断参数表达式先发生的同步阻塞。Mn 没有包住整链的 catch／finally，未被内部处理的异常可能中断后续。preload 的三套循环各自逐项 catch 日志，Device／Shutdown 的结果被丢弃，Exit 逐项 await；局部 catch 不能当作成功确认。该顺序和 JS 包装器不能证明原生回调、线程／句柄、外部服务或设备均已释放。Rust 独立实现、纯模拟、官方实际功能对照及恢复仍未验证。

### USB 与 BLE 的原生停止边界

同一收据的 `native_monitor_cleanup` 将八个 JS owner 接到两份当前 Windows `detection.node`：USB 九个、BLE 七个完整原生函数，共 15 项断言、八条调用边和 35 个证据文件。源码路径、实际导出注册、完整函数字节及依赖集合分别核对；本专项没有加载节点模块或执行官方 DLL。

USB 沿 `An.stop → rz-usb-detect.stopMonitoring → 0x48f0 → 0x8c20`。JS 先清 started 再同步转发；原生在 active 和线程句柄等条件满足时清 active、停止两个 uv signal、SetEvent、投递 WM_CLOSE，并调用 `WaitForSingleObject(INFINITE)`，随后请求 CloseHandle 与卸载 setupapi。等待返回和 close／unload 的返回值没有成为业务确认。窗口线程正常出口另请求注销设备通知、销毁窗口与窗口类，早期失败不能直接套用这段清理。JS 无 await 仍可能同步阻塞 Mn，外层定时器也不能截断该同步调用。

BLE 经 `Qo → endpoint-detect.stop → node-rz-ble-endpoint-detection.stopMonitoring → 0x3aa0`，所选 stop 只清 active、停止两个 uv signal 并 SetEvent，没有对应的窗口线程 join、WM_CLOSE 或线程句柄关闭；缓存 peripheral 的 disconnect 同样未被等待。JS 丢弃原生返回值，不能推定稳定 bool 或全部停止成功。after-work 的 active 检查仅限制所选回调入口，不能证明在途回调已静止。两条链的 event／uv handle、失败恢复和真实释放结果仍未验证。

### 自启专项的验证边界

[自启完整静态审计](../../../.analysis/results/frontend/synapse-settings-current/local-startup.json)的基础切片保留 167 个完整前端／宿主 AST 所有者、53 个词法 CSS 块、十语言四词条、四份字体及 16 个原生函数，共 1,224 条指令、13 条汇编断言。合并上述注册、卸载、原生脚本、最终 JS 清理、USB／BLE 原生及双入口动态灯光切片后，共复核 12,874 个去重 artifact，未发现核对期间字节变化。最终 JS 清理的 21 个篡改反例、USB／BLE 切片的十个反例与动态灯光的 31 个反例全部被拒绝；这些是静态核对器的测试。原完整捕获的 validate 失败仍保留，这些专项通过不表示整个页面依赖闭包已完成。Windows ABI 为 `applicationAutoStart:int()`、`getApplicationAutoStartApps:pointer()`、`setApplicationAutoStart:void(bool,string)` 和 `FreeMalloc:void(pointer)`；当前宿主仅 win32 选择此模块，保留的 mac 模块没有声明前三项入口，不能将 Windows 行为泛化到所有平台。

| 必测分支 | 核对目标 |
| --- | --- |
| 初读、缺 App 条目、undefined、非 string | 区分局部初值、已保存字段、原生结果，核对迟到 GET 不覆盖本项目的新编辑 |
| 自启聚合与最小化 | 严格 true、`name2||name`、列表顺序、独立隐藏名单、无自启项及非生产 host 参数 |
| 注册、已有条目与卸载 | 真实 dashboard gate、新条目默认、旧启动字段保留但元数据更新、非 boolean 旧值、Chroma 等待、重建与卸载失败分别测试 |
| 原生卸载事件与 Run | 单应用／组合退出参数、依赖清空分支、五次日志观察、Run 两名单修改、`REG_SW` 解析、禁用 relaunch 与静默 3010；实际 API 失败、原值恢复和生成卸载器另验 |
| 宿主退出与监控停止 | 分别核两条 Mn 调用、三个 race、USB 条件性无限等待及其返回、BLE 无 join、在途回调、event／uv 资源、重复停止和失败恢复；timer 与发停止信号均不作为释放确认 |
| 连续点击、卸载与 flush | Synapse 每次点击独立 2 秒 timer；Chroma 两点击仅构造 debouncer，不调度 timer；两者 setter 均已立即执行；分别核对读取／timer 清理、目标窗口和 flush 无确认 |
| 注册表查询与参数提取 | 值存在但类型／内容不同、缺 marker、空 token、大小写、ASCII 空格、长度／NUL 与第二次查询失败 |
| 路径、写入、删除和释放 | 当前进程路径、首个 `app-` 子串、UTF-8／容量失败、API 失败、CString 转换失败及对应资源归属 |
| 原版控件与本机效果 | CSS 禁用和 input 行为、十语言词条、关闭重开；OS 启动项和实际启动分别验收 |

纯／模拟测试应先覆盖上述有依据的正常与失败分支，实际 DLL 对照须比较相同输入和受控原值，写入／删除后恢复并确认。本专项没有执行 DLL、注册表读写、厂商 JavaScript、实际启动或窗口验收，也没有新增 Rust 实现；收据中的静态断言不等于这些功能测试已通过。

## 通知

通知键的实际名字为 `displayNotifications`。通知块读取它后调用与点击相同的 writer：先更新组件 state，再调用 `RazerApp.updateAppData(appName,{displayNotifications:value})`，最后写入 localStorage。因而首次 mount 和同键的外部 storage 事件也会重新请求更新本机应用记录，并非只有用户点击才产生写入；它没有设备 setter 或读回。storage 监听只对相同 key 重读，卸载移除原监听回调。用户点击立即执行本地记录与键写入，没有另一个 Save 按钮；底层记录不存在及部分写入失败的边界见下节。

两套页面的读取表达式不同，但通知键缺失时都会先产生 true，再经 JSON.parse 得到 true。Synapse 判断是否为字符串；Chroma 判断是否为 null／undefined。这里仅描述这一个本机通知键，不能把它套到设备读取。

Synapse 的 localStorage writer 依赖 setItem 的字符串转换，Chroma 显式 `JSON.stringify`；正常布尔值均写为 `true`／`false` 文本。两者均没有包住通知 JSON.parse 或本机写入的 catch，也没有把任意合法 JSON 限定为布尔值。损坏或非布尔存储的处理不能描述成“读取失败后仍默认成功启用”；该源码边界须在本项目解析契约和错误状态中单独处理。

本项目由共享 `UiPreferences` 的实际设置页挂载范围观察该键：每 250ms 在后台串行队列中读取原始字符串，仅该键变化或文件读取恢复时重走 typed read → 原 writer；其他键的修改和本次 writer 的成功发布不触发循环。离页取消计时器、排除旧代次的读取结果，已接受的写入仍按序完成；取消退出后使用新代次恢复观察。缺失、损坏、读取失败和已确认布尔值分别处理，不将失败改成默认写入。该有界重查会合并采样间的临时变化，不是浏览器每条 storage 事件均已交付的证据，也不主动刷新通知窗口或改动其队列。

## 公共设置保存、事件与宿主 flush

`updateAppData` 先剔除保留的身份／安装／窗口描述字段，再将其余扩展字段交给 `updateAppDataItem`；`displayNotifications` 属于保留的扩展字段。实际存储是 localStorage 中 `apps` 键的一整份 JSON 数组：按应用 `name` 找到记录，合并变更、更新 `lastUpdated`，再序列化写回。未找到记录时返回 undefined，不新增应用；通知组件忽略该返回值，仍继续写独立的 `displayNotifications` 键。两个键没有共同事务，任一同步写入抛错也没有调用方补偿，不能将两者描述成原子保存成功。

SDK 的全局 storage 消费者只在相应订阅存在时处理 `apps` 事件：比较旧／新数组的具名字段，向 `appsDataChange` 发送当前应用列表和差异；若当前窗口有 appName 且对应字段确实变化，再过滤本应用并发送 `appDataChange`。这些事件表示本机元数据发生变化，没有设备应答、OS 启动项确认或磁盘 flush 完成语义。通知组件还独立观察 `displayNotifications`，收到同键变化后重走自己的 writer；这两层事件消费者分别保留。

Synapse 自启回调的延迟 `storeData` 已追到实际宿主消费者：

| 阶段 | 当前调用及实际结果 |
| --- | --- |
| 设置页面 | `Xs` 调用 `apiElectron.doElectronAction({action:"storeData",windowName:"settings-synapse"})`，没有返回或等待该 Promise |
| preload | `doElectronAction` 通过 `ipcRenderer.invoke("electronAction",request)` 送入宿主 |
| 宿主 dispatch | `storeData` 分支调用 `TabManager.storeData(windowName)` 后 break，没有返回保存结果 |
| 窗口所有者 | `TabManager.storeData` 调用 `TabStore.writeData`；优先按名字找 tab，否则查独立窗口 |
| 存储所有者 | tab 分支直接调用该 webContents 的 `session.flushStorageData()`；独立窗口分支先检查 BrowserWindow 与 webContents 存在且未销毁，再调用同一方法；没有目标时不执行 flush |

这条链刷新目标 Chromium session 的存储，不是另一份设备配置写入，也没有自行把页面数据转成 JSON 文件。`writeData` 与上层方法都没有返回／等待 flush，页面没有收到成功、失败或读回；IPC 已完成只能说明 dispatch 已返回。失败不能被本项目升级为保存成功。

延迟任务只保存窗口名，没有页面实例或连接代次；若原窗口在 timer 到期前关闭、同名窗口随后建立，宿主按调用时的同名目标解析。Synapse 页面的各 timer 未在卸载取消，也没有对应恢复动作；Chroma 自启表达式未调用返回的 debouncer，因而没有这条 timer flush。通知监听的配对清理、自启任务的取消缺口和窗口存储的真实持久化应分别核对。

## 通知开关的实际消费者

通知应用的当前 main 依据 URL 中一次读取的 `notificationName` 选择真实根：值为 restart 时挂载 `SystemRestartRequiredWindow`，其余挂载普通 `Root`。两套入口收据分别保留，实际 JS 字节相同；两套收据的入口、asset-manifest 和 manifest 本轮前后采样均一致，已取得文件、19 项获取失败和 23 项 chunk 枚举未决仍独立记录。

普通通知根在 `notifications` 数组变化的 effect 中调用模块 2864 的 `Ru` 与 `Wy`。前者在通知为空时隐藏窗口，并负责按实际通知 DOM 测量定位；后者仅在通知非空时读取共享 localStorage 的 `displayNotifications`，键为 null 才写入 true，再调用 `setBrowserVisible(value)`。该根的 storage 监听只观察语言键，没有直接观察通知开关；因此切换设置不会在这条源码链中立即重新执行显示判定，也不会删除队列或取消通知 timer。宿主最终严格比较 true／false 才 show／hide，其他类型不能当作已经生效的显示状态。

模块 2864 的宽高回报函数 `aU`／`Y6` 调用同一个 `Wy`，并把其返回值赋回局部队列后写入 `notifications` 键；`Wy` 没有 return，因此这次存储收到 undefined，经当前序列化／setItem 路径成为 `undefined` 文本。该键不能被本项目当作有效队列或通知实际显示的证明；实际渲染来源仍是 Redux 的 `notification.notifications`。这里记录原件完整函数的实际数据流，没有据函数名称补造队列排序或正常持久化行为。

restart 根使用自己的 `resizeWindow`：宽高为 0 时隐藏，其他情况取得屏幕、居中后显示，没有调用 `Wy`。其关闭动画后的 250 ms timer 没有在 effect 中取消，ResizeObserver 则有 disconnect，语言监听也有同身份移除。普通根在 mount 注册 window-storage 观察并启动初始化；卸载只调用宿主 unregister，没有对 `J` 注册到共享 EventEmitter 的 callback 执行 off，也没有遍历清除通知 timer 表。初始化异步结果没有页面代次判定。逐条新增／更新／移除会按 id＋type 清理对应 timer，这与整页卸载是不同路径。实际通知条目另在 mount／load 和 ResizeObserver 中分别调用 `aU`／`Y6`，该观察没有保留用于 disconnect 的句柄。

`rz_native_notification` 又是独立分支：普通根组织原生通知参数后调用 `doNativeNotificationAction`；宿主 `handleNativeNotification(showNotification)` 先查询 dark mode，再调用 `launchRzNotification`。该 JS 分支未读取 `displayNotifications`，与上面的普通窗口显示开关分别建模。实际原生 ABI、系统提交结果、签名及退出链如下；本文没有执行原生通知。

## Windows 原生通知的真实边界

当前包的 `main` 直接指向 `build/Release/rzNotification.node`，没有另一层 JavaScript wrapper。PE 是 AMD64，`node_api_module_get_api_version_v1` 返回 8；`napi_register_module_v1` 经初始化函数注册以下三个 JavaScript 方法。它们是 N-API callback，不能当成同名 C ABI 导出用 `ctypes` 猜签名调用。

| JavaScript 方法 | 当前入参检查与结果 | 原生入口 RVA |
| --- | --- | --- |
| `showToastNotification(dataJson,hmacInfoJson)` | 至少两个参数且前两个均为字符串；否则同步返回 false。两个 UTF-8 JSON 字符串交给原生工作线程，同步等待其布尔结果，不返回 Promise，也不接收 JavaScript callback | `0x3050 → 0x9550 → 0x1e960` |
| `getNotificationSetting()` | 原生工作线程取得三个固定 AUMI 的 OS 设置，序列化后转成 JavaScript 对象；没有 app 参数 | `0x3240 → 0x9500 → 0x20e50` |
| `checkNotificationEnabled(aumi)` | 至少一个字符串，否则 false；空 AUMI、创建 notifier 失败、读取失败或设置值非 0 均为 false | `0x3930 → 0x94b0 → 0x218f0` |

三个方法分别拥有惰性建立的原生队列／线程单例；调用把 packaged task 入队、唤醒并等待 future。工作线程执行 `CoInitializeEx(NULL,COINIT_MULTITHREADED)`，退出时仅在该初始化成功的情况下配对 `CoUninitialize`。它们不是 N-API 异步工作完成回调。错误参数返回 false 这一分支已经确认，但 JSON 解析及字段类型错误会进入 C++ 异常路径，不能据此把全部异常都描述成 false 或保证能安全转为 JavaScript 异常。

通知 JSON 的实际字段来自当前原生完整函数：

| 字段 | 读取／分支语义 |
| --- | --- |
| `id`、`templateType` | id 缺失为空文本；templateType 缺失为 0。构造函数按索引读取固定文本行数 `[1,2,2,3,1,2,2,3]`，没有在入口校验索引边界；不能把任意整数当作合法模板 |
| `scenario`、`expiration`、`duration` | 缺失整数为 0。scenario 的 1／2／3 对应 Alarm／IncomingCall／Reminder，其余 Default；expiration 正值按毫秒加到系统 FILETIME；duration 为 1 时 short，其他非零值 long，0 不写该属性 |
| `text1/2/3` | 缺失为空文本，转 UTF-16 后仅填入模板实际具有的文本槽位；文本通过 DOM text node 写入，不直接拼接未经转义的 XML |
| `iconPath`、`iconName`、`isDarkMode`、`cropHint` | iconPath 优先；其为空且 iconName 非空时，按 isDarkMode 选择本机通知图片。cropHint 为 1 走圆形／appLogoOverride 分支，其他值不能自行推导成新能力 |
| `heroImagePath`、`inlineHeroImage`、`audioPath`、`audioOption` | 独立图片和音频分支。hero 图片受模板与 OS build 条件约束；audioOption 为 0 且路径为空时不修改模板音频 |
| `appName`、`appUserModelID`、`app` | 前两者可显式提供；缺失项仅在 app 命中固定映射时补齐。synapse → Razer Synapse 4／Razer.Synapse.4；chroma-app → Razer Chroma 4／Razer.Chroma.4；virtual-ring-light → Razer Virtual Ring Light／Razer.VirtualRingLight。未命中记录空字段并让初始化失败，没有通用成功默认应用 |
| `shortcutPolicy`、`actions` | shortcutPolicy 缺失为 0，非零才走 ShellLink 校验／创建链。actions 缺失直接通过；每项消费 index／id／name／arg，按 index 放入预先按集合长度分配的 action 槽位，越界不会插入该项。重复或稀疏 index 不能改写成简单遍历追加语义 |

数字 helper 对缺字段返回 0；已存在值仍要走 JSON 类型转换，字符串 helper 对非字符串抛类型异常。当前页面只发送 app、id、scenario、templateType、text1/2/3、iconName、cropHint 和 actions；过期时间、音频、hero、shortcutPolicy 等属于 addon 支持但当前 producer 没有送入的分支，不能宣称页面已接通。

宿主把系统查询结果写到 `payload.darkMode`，原生图片选择却读取 `isDarkMode`。当前页面与宿主均未补后者，因此这一调用链读取到缺失整数 0；不能写成深色查询结果已经控制原生选图。选图顺序为 isDarkMode=false 时 `<name>-black.png → <name>.png`，true 时反向，实际文件必须存在；两者均不存在返回空路径。当前函数使用可执行文件位置与 `CommonDLL\\NotificationImages` 构造路径，属于原件行为，本项目资源访问须归统一安装与缓存所有者。

显式 iconPath／heroImagePath 若为小写 http(s) 前缀，会走 WinHTTP GET、临时文件及句柄释放；本机路径不走下载。当前 helper 没有以完整 HTTP 状态、每次 WriteFile 的返回及写入长度确认下载成功，也没有在本条成功路径证明临时图片最终删除。该联网分支不直接进入本产品范围；授权静态媒体必须使用统一资源服务，不能照搬 addon 的任意 URL 下载和临时文件生命周期。

## Toast XML、系统提交与回调

实际 OS 链为取得 `ToastNotificationManager` activation factory → `CreateToastNotifierWithId(AUMI)` → `GetTemplateContent(templateType)` → 修改 DOM → `ToastNotification` factory 的 `CreateToastNotification(document)` → 设置可选过期时间 → 注册事件 → `ToastNotifier.Show(notification)`。最后一次调用的完整参数已由汇编核对，不能沿 Hex-Rays 省略的显示参数误写成零参数调用。

正文通过 `CreateTextNode`／`AppendChild` 写入。actions 以 `CreateElement("action")` 与 `SetAttribute(content,arguments,activationType)` 构造；需要时创建 actions 节点，设置 ToastGeneric 和 long duration。arg 含 `://` 时 activationType=protocol；不含协议分隔且非空时使用 background＋索引，空 arg 使用 system＋dismiss。进入这些 DOM 分支前，业务 helper 会检查所有非空 arg：逐字节转小写后必须以 `razerappengine` 开头，否则拒绝整条通知；该检查只是前缀检查，不等于宿主的完整 scheme 正则校验。正文点击没有从 payload 另取一个 launch URI，不能将动作按钮的 protocol 参数泛化成整张通知的点击链接。

Windows major ≤6 不执行现代 actions／audio／duration／scenario 分支；圆形图片和 hero 又包含 build ≥14393 的条件。模板索引、对应文本槽位、icon／hero 的选择及现代 OS 条件分别保留，不能因某个 ToastGeneric 用例通过就覆盖全部模板与系统分支。

原生显示函数以新 GUID 的 Data1 作为内部通知身份，保存 Toast 对象与 Activated／Dismissed／Failed 的 event token；wrapper 将返回身份是否非零转成 bool。false 可以来自初始化、DOM、事件注册或 Show 失败；true 只证明该路径产生非零身份并且 Show 返回非失败 HRESULT，没有等待 Activated、Dismissed 或用户实际看到通知。GUID 生成失败与部分注册失败的中途资源状态须独立测试，不能由正常路径的 Release 推断异常清理完整。

事件注册次序是 Activated → Dismissed → Failed。Activated 读取事件 arguments，有内容时按十进制索引进入 activatedWithIndex，否则进入 activated；Dismissed 读取系统 reason，并在相关过期时间已到时调整本地 reason；Failed 进入 failed。它们先调用共同清理函数，将内部通知标为结束、按保存的 token 移除三个 OS 事件并从集合释放。内部 handler 具有 toastChanged 的 activated／activatedWithIndex／dismissed／failed JSON 构造，但只有回调槽非空时才发出；当前三个 N-API 导出没有设置该槽，也没有 JavaScript 订阅导出。不能据这些保留函数声称宿主收到了每次失败或点击回调。

`getNotificationSetting` 逐项读取 Razer.Synapse.4、Razer.Chroma.4 和 Razer.VirtualRingLight 的 `ToastNotifier.Setting`。原生把 manager／notifier／Setting 调用失败映射为该 AUMI 的 5；宿主再映射 0 Enabled、1 DisabledForApplication、2 DisabledForUser、3 DisabledByGroupPolicy、4 DisabledByManifest、5 NotSupported，enabled 仅严格为 0。5 同时包含调用失败，不能从它反推系统确实不支持；宿主整体异常另返回空对象。该查询与本机 `displayNotifications`、页面显隐、Show 的 HRESULT 四者互不替代。

## Worker、签名和点击恢复

Windows producer 每次生成 8 字节随机 keyId 的十六进制文本和 32 字节随机 hmacKey 的十六进制文本，分别 JSON.stringify 通知与密钥信息，再开 Electron Worker 加载 addon 并同步调用显示函数。message 只返回 `{ok}`；worker catch 的 error 文本没有被 message 消费者记录为结构化失败。20 秒 timer 调用 terminate 并 resolve(false)，error／exit 也 resolve(false)；message 清除 timer 并 resolve(ok)。成功消息后没有显式 terminate／等待退出，也没有让页面保存这一任务的所有权。

原生对非空动作 URI 先规范化一处结尾斜杠：无 query／fragment 时移除协议之后的末尾 `/`；有 query／fragment 时移除最早分隔符前一位的 `/`。HMAC 为 BCrypt 的 SHA256＋HMAC 标志，**密钥使用 hmacKey 的十六进制字符串本身的 UTF-8 字节，不是解码成 32 字节**；数据使用规范化 URI 字节，输出小写 hex。随后在 fragment 前用 `?key=<id>&sig=<hex>` 或 `&key=...` 追加签名。BCrypt 中途失败返回空签名，当前函数保留原动作参数，不是拒绝该条的通用分支。

同一通知的动作共用 keyId。宿主只有收到 truthy ok 后才将其放入 pending keys 并尝试持久化；生成并提交 Toast 早于这一保存时点。密钥以 Electron safeStorage 加密写到 `State/pendingNotificationKeys.enc`，加密不可用时跳过写盘，写失败仅记录日志，没有原子临时文件或写后确认。10 天期限在加载及 prune 时过滤；verify 函数自身没有再次比较 createdAt，不能把它描述成每次点击都严格检查期限。

宿主恢复／激活按真实入口分别核对：

| 入口 | 当前行为与确认边界 |
| --- | --- |
| 初次启动 | ready 时先 loadPersistedKeys；在 Windows 找到匹配 URI 后，按 background-manager 已存在／加载中／尚未建立分别立即 fire、等 did-finish-load，或暂存并监听窗口创建。窗口监听在命中及 before-quit 移除 |
| second-instance | 保存本次 argv 并调用 fire；它使用宿主完整 scheme 正则，只处理首个匹配项 |
| fireNotificationURI | 提取 key／sig，移除签名参数并做同一斜杠规范化；URL 解码后查 key，以 Node crypto HMAC-SHA256 和 timingSafeEqual 比较严格小写 64 位 hex。有效后先删除该 key 并持久化，再向未销毁／未崩溃／未 disposed 的 webContents 广播 notification-uri。同通知第二个动作或重复激活会因同 key 已消费被拒绝；广播本身不确认业务动作已执行 |
| pending URI map | 按协议保存最后一条 URI；getPendingNotificationURIs 返回全部值并清空。store／drain 自身不校验 HMAC，也不消费密钥，不能把 drain 结果当作已通过 fire 校验的输入 |
| getNotificationURI／open-url | 前者只从当前 argv 取原始 URI；open-url 另广播 handle-notification-uri。两者不经过上述 fire 验签函数，必须按实际调用者继续追踪 |

当前 background-manager 的实际 HTML 通过 module script 启动 `index-b4c3eb99.js`，另以 deferred classic script 加载共享 manifest；import map 将入口真实使用的 `razer-config` 指向 `/synapse/assets/js/razer-config@latest/index.mjs`。启动时还以文档 URL 为基址登记 `./sw.js`，后者的唯一 `importScripts` 是 `./service-worker-message.js`。这些原件及其字面 ESM／动态 import 已补齐：[通知消费者静态审计](../../../.analysis/results/frontend/background-manager-current/notification-uri-consumers.json)保留 56 份 JS 的完整 AST 核对、515 条依赖边和实际启动根；该字面图没有缺失目标，但不涵盖任意运行时构造的模块 URL。

入口把 otfm 模块的导出 `R` 导入为 `e`；该导出真实绑定到 `Me=new Se`。`Se` 完整类的 constructor 将 preload 的 `response("notification-uri")` 转为这个实例上的 `emit("notification-uri")`。对上述 56 份原件枚举的 367 个事件注册调用中，没有发现同名业务注册；也未发现调用 SDK `getNotificationURI()` 的消费者。初筛中的 39 个动态事件参数及 46 个计算成员调用已逐项保存起点、完整所有者、参数域和当前语法调用者，不只保留计数：8 项事件参数来自 BLE／Monitor／IoT 的设备常量，16 项来自另一个 SDK 的具名事件常量，另有 1 项浏览器可见性事件的四种前缀分支，均核对到源码赋值。其余 14 项属于 EventEmitter 包装、SAX、analytics、RemoteStorage、账户 pubsub 或 Observable；其中 subscribe 的若干参数实际是账户协议／观察者回调，不是事件名字。EventEmitter helper 的两个当前词法调用方均在静态 once 实现内；全部原件的显式 `.once()` 调用也只有该 helper，`_delegateEvent` 没有当前引用。

46 个计算调用分别落入硬件 parser 表、卸载存储清理表、迭代器／生成器、Lodash、XML、Promise、日志、analytics、同步与 worker 缓存钩子等实际域。硬件 parser 的 32 个直接调用均传入具体 parser 身份；Loupedeck 的 URI 形式路由只有一个当前词法调用方，来自 `ws_message` 的数据，不能把它接到通知广播。RemoteStorage 的延迟方法六次入队只使用 get／put／delete；模块迭代的五个当前调用分别使用 `acceptAllLocal/acceptAllRemote/onUserLogout/clearUserData/onUserLogin`。公开通用包装仍接受调用者数据；99 个事件方法非调用引用、可能的别名逃逸、外部调用者与依赖流程的值流不由这些语法清单自动证明。因此审计没有将“当前所核调用域未发现该消费者”升级为所有动态消费者都不存在。另对 707 份含相关词的已保留前端 JS 扩大精确文本搜索，也未找到 pending／open-url 通道的字面调用者；这部分只是有界初筛，不代表这些其他应用已经逐份完成语义审计。

Service Worker 的 58 个 precache URL 已逐项保留原件，带 hash 的 JS 使用 `revision:null`，其中明确包含 `assets/otfm-d56d4f50.js`；这只是 URL 缓存条目，两个 worker 原件没有宏事件注册或 producer 改写。其 precache 优先缓存命中、缺失时可联网，导航读取 precached index；安装文件为 cache-first，安装 manifest 为 network-first，设备目录 JSON 为 cache-first。后两类使用忽略 query 的匹配／键规则，因此抓取到当前网络字节不能证明已有 renderer 一定执行同一字节。启动向 controller 发送 `updateURL(DeviceDataJson)` 与 `checkAssetsVersion`；前者逐 URL 更新，并仅在任一项目成功的响应下触发设备列表重查，不表示全部成功。后者按 `/synapse/assets/index.json` 更新 `cacheAssets`，单条失败继续，通用回复只投递给 `clients.matchAll()` 的第一项；当前入口没有消费该回复的分支。active／waiting worker、实际缓存身份、部分失败及客户端顺序仍未运行验证，这些原版联网缓存行为不进入本项目离线产品路径。

清单另包含 `sw.html`；它只引入共享 `/synapse/assets/js/sw.js`。这份额外 JS 也已保留和解析，但不能因为被 precache 就当作主页面已执行的模块。其 `window.onmessage` 从父窗口接收 `mfetch/start`，`start` 使用消息中的 url／sw 参数登记 worker，未给 sw 时才取 `service-worker.js`；未将猜测出的默认 URL 当成实际启动依赖。该缓存页额外产生的 1 个动态“response”其实是按相关 ID 完成 Promise 的消息总线，两个计算调用分别访问相关 ID 回调表及固定 `mfetch/start` 表；它们也逐项保留，不能与 preload 事件桥混为一谈。

pending drain 的真实入口是 `apiElectron.doNativeNotificationAction({action:"getPendingNotificationURIs"}) → nativeNotification IPC → handleNativeNotification → drainPendingNotificationURIs`，不是普通 `electronAction`。SDK `getNotificationURI()` 则走 `electronAction/getNotificationURI`，只取启动 argv；`open-url` 又使用另一个 `handle-notification-uri` 广播。当前字面依赖图中未发现 pending drain 请求者和上述两种 URI 的最终业务动作；不能将三个入口合并、替它们推定打开页面，或把未经校验的 drain 结果当作验签结果。点击的“OS 启动协议 → 宿主验签与广播”已有静态证据，最终打开页面／执行动作仍未闭合，也没有运行时对照。

原生队列的 atexit 清理设置 stop、通知条件变量、等待线程结束并释放队列；WinToast 单例析构则逐条撤销事件、释放通知与回调并配对其 COM 初始化。当前 N-API 注册未设置 env cleanup hook，atexit 也不能自动等同于 Electron Worker 正常退出／20 秒 terminate 时已完成全部清理。尤其线程归属、回调在途、部分注册失败、成功消息后的 worker 退出和进程级析构须保留为独立验证项。

这条链的最小测试集合须分别覆盖：参数数量／类型和损坏 JSON；8 个模板与文本／XML 特殊字符；三个 AUMI 与各系统设置值／失败；动作缺失、空 arg、外部前缀、稀疏／重复／越界 index；URI 的 query／fragment／斜杠规范化、文本密钥、签名失败、单次消费和密钥恢复；darkMode 字段差异；DOM／Show／部分事件注册失败；超时、退出、取消与全部 token／COM／线程释放。纯逻辑与模拟测试先使用独立原件期望；实际 addon／OS 对照必须通过已核实的显式测试入口，不能猜 N-API ABI、执行厂商 worker 充当普通测试或将显示与点击共同失败视为一致通过。当前文档只完成上述静态链核对，没有新增实际 DLL／OS 对照收据。

## 重置教程

这不是设备恢复，也没有设备写入。Synapse 当前重置动作的本机字段与操作如下；同一次点击按完整函数顺序执行，没有统一设备 Apply 或文件事务。

| 操作 | 当前实际键 |
| --- | --- |
| 标记本轮已经重置 | `disableSynapseSettingReset=true`、`resetTutorials=true` |
| 置 true | `showCalibrationMessage`、`isShowTutorialMacro`、`showWelcomeMessage`、`isShowIntroductionBanner`、`isShowArmoryIntroductionBanner`、`showTHXNews`、`isShowChromaStudioTutorial`、`showOnboardingTutorialCarol`、`isShowSensitivityTutorial`、`showAnalogTutorial`、`tutorialSeen` |
| 删除，让相应页面再次判断首次引导 | `showGamerRoomTutorial`、`showDashboardTutorial`、`showIotTutorial` |
| 修改已有产品文档 | 仅键匹配 `^synapse_\d+$`、JSON 可解析且两个教程字段至少一个严格为 false 的文档；对其中实际具有的 `showAIPromptMasterTutorial`／`showAppProfileAITutorial` 字段置 true，再写回完整文档 |

产品文档扫描逐项捕获 JSON.parse 失败并忽略该条，不给其余产品补字段。页面 mount 先读取 `disableSynapseSettingReset`，再检查固定教程键；初次 effect 没有立即调用产品文档扫描。产品键的 storage 事件才调用扫描并可重新允许重置；非产品键事件仅检查固定键。上述三个“删除”键只要存在布尔值即使为 false，也会重新允许重置，其余被观察的键用 falsy 判定。当前 Synapse effect 注册和移除 storage 监听时分别创建了新箭头函数，移除表达式不是同一个回调身份，不能写成已成功注销。

Chroma 的 mount 只以 `isShowTutorialChromaApp`、`isShowChromaStudioTutorial`、`isShowIntroductionBanner` 是否全部严格等于 true 决定按钮状态，之后仅在这三个键的 storage 新值为字符串 `"false"` 时重新启用。点击将三者与 `isShowArmoryIntroductionBanner`、`showTHXNews`、`showOnboardingTutorialCarol` 一并置 true；后面三个不在该监听的重新启用集合中。它使用同身份监听清理。两套页面的初始判定、被观察键集合、重置集合和卸载行为不能合并。

## 迁移入口和动态灯光

迁移按钮先查询窗口服务客户列表。Synapse 查找 `syn3-profile-migration`，Chroma 查找 `chroma-app-syn3-profile-migration`；已存在则 activate，不存在则打开 `/profile-migration/`，Chroma 附带 `?app=chroma-app`，窗口参数含 `policy=3,tab_visible=1,shouldFocus=1`。按钮本身不读取和转换配置文件。

动态灯光块只在 Windows build ≥22631，或 build 22621 且 UBR ≥2506 时显示。两页都立即更新本地 `isDynamicLighting` 意图，观察 `wdl-devices.isSwitching` 显示加载与禁用，系统设置链接调用 `msSettings("personalization-lighting")`。Synapse 在 await OS 版本后初读意图，Chroma 的本地初读与 OS async 并行；均没有预读 switching，也没有将 OS getter 当作当前模式。

[双入口静态切片](../../../.analysis/results/frontend/synapse-settings-current/local-startup.json)的 `dynamic_lighting_control` 追到 LE 的真实 storage 消费者、Configure／JsonApi 宿主 IPC 和 WindowStorage 事件。意图走 localStorage/storage；进度走 WindowStorage/IPC 原始对象，set 返回旧值。Configure 结果不回到控制权切换调用者，枚举失败可被 wrapper 变为空数组，update 的 finally 清 switching 也不证明成功。已发现的两条本机 WebSocket 不承载这些边。订阅按 URL 去重／移除；wrapper 丢弃 disposer，但 preload 的 pagehide 会清理 windowStorageEvent，不能概括为页面销毁后必然泄漏。初始化失败不清 isInit、onExit 拒绝会阻止后续 reset／移除的边界，以及两个现有 Rust 仅保存本地选择的差异，见[动态灯光的实际通道](../shared/dynamic-lighting.md#两个设置入口到后台的真实通道)。控制权、设备集合、恢复与清理仍须独立实现和实际对照。

## 语言下拉框与翻译

显示顺序是 English、Deutsch、Español、Français、日本語、한국어、Português (Brasileiro)、Русский、中文（简体）、中文（繁體）。它由固定数组决定，不做语言名称排序。选中下标用 `code === lang.toLowerCase()` 找到；点击取对应 code，先 `updateLanguage(code,translationsImporter)`，再以原始字符串写入语言键。storage 事件同步其他窗口的语言变化。

语言转换函数按区域别名映射到 `en/de/es/fr/ja/kr/pt-BR/ru/zh-CN/zh-TW`。读取语言键不是字符串时才取 `navigator.language` 并转小写。翻译导入器先取默认英文包，再按需要取选中语言包；Promise.all 完成后才派发语言 action。首次字典尚未建立时派发 `unknown`。当前 thunk 没有完整的导入失败 catch，不能描述为加载失败仍成功切换。

单项文案的查找规则则是选中语言字典 → 英文字典 → 文案 key，插值替换 `{{参数名}}`。这是具体翻译函数的规则，不是设备数据的读取规则。

当前 Synapse 下拉组件在打开时计算剩余空间与菜单高度，最多按 180 px 选择向上／向下展开；打开后将已选项滚入视野。点击外部或窗口失焦关闭，卸载移除原 click／blur 监听。没有选中项、下标为 -1 或越界时直接不渲染。该版本的值更新分支包含原拼写 `disabeld`；文档不把它改写成已更新 `disabled`。

## 关于及工具栏附属功能

Synapse 从 dashboard manifest 获取 version 和 buildVersion，失败分支读取本机应用项的 version；Chroma 请求自己的 manifest，并将显示版本第一段改为 4。Chroma 额外读取 `sensa-devices` 中的设备类别及 dualSenseControllerSupport 配置，二者同时成立才显示相关版权行。

版权年份取当前年份。本机工具栏附属功能包含通知、固件状态和迁移提示，其内部行为由对应功能文档说明。

## 页面状态、尺寸与当前 Rust 接线

表中的本机范围不能代替实际挂载树。Synapse 的 `ho` 左列实际为 `Ks/bn/Ia`，右列 `Un/Ta/ia`；`Ia` 是推荐、产品发布与促销偏好，属于排除的联网业务，不在本机页面保留禁用占位。`uo` 的左列 `Te` 同时挂载语言和 Release Notes，不只是下拉框；右列为 `Fs` 关于。Chroma 的 `mi` 左列 `Hs/ye`、右列 `An/Zs/de`，`vi` 左列 `Ae`、右列 `Ps`。Release Notes 的原调用先置 loading，检查 engineVersion ≥4.0.633，经 background-manager 频道请求窗口；收到 settings 频道 `releaseNotePatch/turnOffLoading` 才清 loading，engineVersion 缺失或过旧则立即清除。该原流程不能等同于 Rust 已打开本地缓存弹窗，也不能因它是联网上游就为本地窗口伪造成功。

原设置主体沿 `BodyWrapper → body-widgets → widget-col → widget → setting-block` 布局，列宽 600 px，正文 inset 为 `10px 20px 20px`；当前样式在窄窗将设置列 margin 归零，不能继续叠加通用列的 30 px。widget 标题 18 px，普通说明 14 px／17 px；语言下拉框的 Synapse 专用规则为 188 px，Chroma 挂载的 plain dropdown 为 230 px。通知帮助锚点为 left 226／top 4，提示框 left 242／top 4／width 300 px，包含说明与三个列表项。自启子项树状线、禁用透明度、重置按钮 disabled、灯光 switching 的 spinner／禁用、语言向上／向下展开、菜单 blur／外部关闭及窗口焦点隐藏必须分别保留。CSS 中的旧 side-navigation、device-setting 和 delete 样式没有被当前这两页根直接挂载，不能据保留 selector 额外制造页面或弹窗。

当前代码有三个独立设置表面，应按实际入口审查，而非将任一 `Settings` 类型计为全部已还原：

| 页面／动作 | 当前 Rust、UI 与后端所有者 | 已确认边界与剩余工作 |
| --- | --- | --- |
| Synapse 设置 root | [SettingsPage](../../../crates/razer-settings/src/preferences/settings_page.rs) 持有 Synapse／General／用户要求的本地 Products 三个页签、SelectState、订阅、草稿和保存快照；[product_settings_navigation](../../../crates/razer-shell/src/shell/devices/product_settings_navigation.rs) 将 Synapse 产品设置路由到同一 entity | Products 是本项目的本地浏览入口，不属于厂商的两个原始设置页签；当前 root 的页签切换与关闭／重开不能据共享 host navigation 自动算作原 sessionStorage 历史、刷新和 `showSettings` 链全部完成 |
| 独立主机设置窗口 | [SettingsWindow](../../../crates/razer-settings/src/window/settings_window.rs) 为另一个 `/settings/` 表面，包含 Software、Systray、General 和本地 Products；shell [settings_window](../../../crates/razer-shell/src/shell/windows/settings_window.rs) open-or-focus，普通最小 600×500、托盘齿轮覆写 1000×768，默认 1280×720 | 这个窗口不是 Synapse／Chroma 产品设置的替代物；固定禁用 back／forward 与当前独立原件一致，但 Rust refresh 只切回首项并卸载 tray selector，不等同原页面 reload。当前 Software 仅呈现 Installed 标题，launcher／Widgets 没有真实 host catalog；这些空内容是未完成接线，不能以源码的空态分支当成功能验收 |
| 独立 Systray 显示与动作 | [SystrayActionSelector](../../../crates/razer-settings/src/preferences/settings_systray_action.rs) 保留开闭动画任务与卸载；[tray.rs](../../../crates/razer-shell/src/shell/tray.rs) 消费 `TrayDoubleClickAction::ShowMenu` 显示实际托盘 | 当前 selector 与消费者只有 ShowMenu；原件非 website 应用 launch 选择、5 槽编辑／真实启动、卡片及子项显示／排序尚未逐项对应。该 ShowMenu 本机链存在，不能扩大为所有托盘动作已还原；没有实际窗口结果的本轮验收收据 |
| Synapse 自启 | `StartupRequested` 经 shell [startup owner](../../../crates/razer-shell/src/shell/host/startup.rs) 串行提交到 [razer-platform::startup](../../../crates/razer-platform/src/startup.rs)，由 Windows Run key 适配器真实读写本程序注册项；初次读取不产生保存，旧 revision 不能覆盖新编辑，退出排空已接受的写入 | 这是独立 Rust 的本机 OS 链，非调用官方业务 DLL；仅自动启动有 OS 初读，最小化来自本地草稿。setter 成功不增加原件没有的 GET／回滚；当前未有与官方实际函数和真实启动行为的对照收据 |
| Synapse 通知开关 | 主设置入口和产品齿轮入口共用同一页面挂载边界，两者互切保留观察；离开两者或切到 General／Products 时拒绝迟到读取并继续完成已接受写入。checkbox 与实际 mount 由共享 `UiPreferences::DisplayNotifications` owner 排队读取／写入 `source_local` 的真实 `displayNotifications`；挂载期间以 250ms 后台有界重查接入外部同键变化，自身提交不循环，失败恢复后重新读取。成功读取后按 720 writer 顺序更新已有 `apps` 中的 synapse 记录、最后发布独立键，记录缺失不创建。workspace 旧 `notifications` 仅迁移，不再充当运行时开关 | [实际 profile 通知消费者](../../../crates/razer-shell/src/shell/windows/profile_switch_notification.rs) 从相同键读取，缺失／null 才初始化；非布尔值不产生 show／hide，失败不覆盖当前显隐确认。设置点击不主动重读窗口、清空队列或取消 timer；仍按通知数组／测量消费者触发。采样间临时变更、原版 DLL 对照与实际窗口验收仍未验证，原生 Toast 保持独立 |
| Synapse 教程重置 | `ResetTutorials` 经 shell 更新设备 intro、Gamer Room、Dashboard 教程并保存；按钮本地 state 控制 disabled | 与上文官方固定键集合、产品文档扫描／删除字段及所有产品消费者尚未逐项对应；只重置这几个 entity 不代表完整教程恢复链 |
| Synapse 动态照明 | [settings_lighting](../../../crates/razer-settings/src/preferences/settings_lighting.rs) 呈现二选一、帮助、switching 和系统设置入口；点击发出 `DynamicLightingRequested` | 当前 shell 仅记录“本地选择”，没有提交控制权服务；初值来自 `dynamic_lighting_draft.unwrap_or(false)`，render 固定传 `Some`，尚未使用控件已有的未知状态禁用路径。不得将它显示为已经观测或确认的设备控制权 |
| Chroma 设置 | [ChromaSettings](../../../crates/razer-app-pages/src/chroma/chroma_settings.rs) 保留独立 page/history/focus、语言、说明、迁移、关于和弹窗；[state owner](../../../crates/razer-state/src/chroma_settings.rs) 串行读写，拒绝旧 generation／revision，页面观察该 owner | AutoStart、StartMinimized、Notifications、DynamicLighting 当前通过 `Change` 立即写 [Chroma 本地存储](../../../crates/razer-storage/src/chroma_settings.rs)；没有 Chroma OS 初读／自启 setter 或灯光控制权服务写入。现有 retry／discard／未保存阻止关标签与官方 `Zo.unload` 不同，retry／discard 只作用本地配置；不同窗口关闭链分别核验，不能将本地持久化等同功能闭合 |
| 语言、关于与 Release Notes | Synapse／Chroma Select 的确认进入共享语言 owner；SelectState 与订阅由各页面保留；版本／词表由本地目录读取，Release Notes 使用本地存储入口 | 独立 About 的托盘 manifest producer、两个产品的 Dashboard producer 与本地目录版本须分别对应；共享 locale 传播、原 dropdown 的失焦／外部关闭／无效选择以及 Chroma DualSense 条件仍需核对；本地文件／弹窗打开不覆盖原频道 loading 和关闭链 |
| 迁移、原生通知与清理 | 迁移进入独立 owning 页面；原生通知的实际 source→worker→addon→OS→URI 广播链见前文 | 迁移全流程、通知最终业务消费者、线程／事件 token 与窗口取消清理分别验收。没有实际 DLL 或真实设备对照时继续标未验证，不因入口存在、两边无设备或文档补全而升级 |

Rust 自启适配使用本程序的 `razer_ui` Run 值，不能覆盖官方 `RazerAppEngine`。当前 getter 对本程序命令提取 token 后匹配 synapse，接近官方 Apps getter 与 JS 消费者组合，并非 `applicationAutoStart` 的值存在判断。Windows 读取使用动态缓冲并校验 REG_SZ／UTF-16，写入使用 `RegCreateKeyExW` 并返回 OS 错误；当前 EXE 路径不做官方 `app-` 子串裁剪。这些是实际可定位的本程序适配边界，需在对照收据逐项解释，不能直接宣称与官方原生导出逐输入等价。退出排空已接受写入也须与真实关闭／重启结果分别验证。

现有 `preferences/settings_tests.rs` 含自启观察与命令、控件尺寸、语言及持久化相关 UI 测试，Chroma 的状态／存储模块另有纯测试。本轮只读取用例与代码，没有执行窗口测试。后续必须独立覆盖读取值与本地值不同、未观测／失败、点击期间延迟读取、文件失败、取消、关闭后重开、语言与焦点、单个控件实际消费者和 OS／服务结果；相关设备／原生功能仍须与当前官方实际 DLL 结果逐项对照。

## 原件位置与边界

所有位置为 UTF-8 半开字节区间，身份对应各 source set 的 acquisition.json；此处不重复维护哈希。

| 所有者 | 原件／位置 |
| --- | --- |
| 独立设置的部署与完整静态收据 | [host-settings acquisition](../../../.analysis/inputs/frontend/host-settings-current/acquisition.json)、[analysis.json](../../../.analysis/results/frontend/host-settings-current/analysis.json)、[semantic-audit.json](../../../.analysis/results/frontend/host-settings-current/semantic-audit.json)；45 个完整词法所有者，12 条选定语义链 |
| 独立 Software、Systray 与实际托盘消费者收据 | [local-controls.json](../../../.analysis/results/frontend/host-settings-current/local-controls.json)；690 个原件身份、245 份 JS 的 AST、184 个完整所有者（177 个 JS 所有者及 7 条 CSS rule）与 9 个选定原生函数；10 组语义记录分别覆盖结果、失败、取消／释放及未验证边界 |
| Software 挂载、列表与动作所有者 | [921.dfdde98f.chunk.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/921.dfdde98f.chunk.js)：完整模块 9302 `[1867,25619)`；列表 `[24300,25618)`、安装元数据 `[23037,24253)`、子项 `[17541,22207)`、父 Map `[22208,23036)`；精确函数以 local-controls 中 AST 区间为准 |
| Software 固定 catalog 与更新／进度 | [main.eda30dd1.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/main.eda30dd1.js) 模块 5596 `[14718,24569)`；上述 921 的更新类 `[7384,14691)`、进度 observer `[4251,6526)`；[921 CSS](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/css/921.08e5c8d7.chunk.css) 及共享 `.disabled` rule 单独保留 |
| Systray 设置的完整树与本机键 | [762.98d21f1a.chunk.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/762.98d21f1a.chunk.js) 模块 9762 `[12255,35331)`；[97.d87e27ff.chunk.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/97.d87e27ff.chunk.js) 的 4672、8531、7217、4518 工厂及真实导出／method，详见 local-controls；[762 CSS](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/css/762.226a09dd.chunk.css) 的 disabled、卡片与子项规则分别保留 |
| 实际托盘 launcher、卡片及事件消费者 | [main.9579c403.js](../../../.analysis/inputs/frontend/systray-left-current/raw/apps.razer.com/systray/systrayv2/static/js/main.9579c403.js) 的模块 9001 `[84320,88733)`；[554.2573b048.chunk.js](../../../.analysis/inputs/frontend/systray-left-current/raw/apps.razer.com/systray/systrayv2/static/js/554.2573b048.chunk.js) 的 launcher `[31861,32837)`、卡片根 `[37707,39239)`、双击 `[42576,43815)`、root 事件 `[45473,46858)`；子项 492／215／17／182／899／190／414／482 的完整工厂也保留于 local-controls |
| 安装名单的真实 Host／原生调用 | [sysutil/win/index.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/sysutil/win/index.js) 的 ABI 与 callDLL；[ffiMain.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/ffi/ffiMain.js) `[231,2766)`；[SysUtilsNative 捕获](../../../.analysis/results/native/01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318/capture.json)，Image Base `0x180000000`，RVA `0x73ff0/0x69590/0x5c980/0x9a1d0/0xa9940/0x9d2e0/0x6b9d0/0x97ce0/0xa98e0` |
| Chroma 隐藏与实际 enable 的分界 | 上述 762 的 `le` `[29466,30394)`；上述托盘 899 的完整 1899；[LE main.a383ea9e.js](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/raw/apps.razer.com/synapse/lighting-engine/static/js/main.a383ea9e.js) 的模块 1876 `[217525,218272)`、5976 `[959155,974836)`、7815 `[1213492,1232498)`、实际 memory handler `[1624329,1625173)`；[Host memory_storage](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/memory_storage/index.js) 与 window_storage 完整原件同时核对 |
| 独立设置的原生窗口 producer | [宿主 main.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/main.js) 的 `launchRazerApps` `[13415,19476)`、托盘回调 `[25734,25816)`；[getAppHost.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/lib/getAppHost.js) `[0,447)`；[constants.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/constants.js) 的路径 `[0,313)`、最小尺寸 `[2227,2337)` |
| 前端托盘的独立设置消费者 | [554.2573b048.chunk.js](../../../.analysis/inputs/frontend/systray-left-current/raw/apps.razer.com/systray/systrayv2/static/js/554.2573b048.chunk.js) 的 gear `[31606,31705)`、通知 `[35397,35567)`、Widgets `[38672,38836)`；其实际 SDK 714 的 launch、origin、settingsPath 及五项常量逐项保留于审计 |
| 独立设置 bootstrap、根与 Router | [main.eda30dd1.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/main.eda30dd1.js) 的 bootstrap `[74785,75697)`；[97.d87e27ff.chunk.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/97.d87e27ff.chunk.js) 的根类 `[190482,193232)`、初始化 `[9706,11878)`、工具栏 `[186275,190034)`、Router `[132884,133128)` |
| 独立 General、语言与 About | [414.717cc452.chunk.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/414.717cc452.chunk.js) 的根 `[38575,38880)`、语言 `[11189,12059)`、About `[35640,38520)`、版本 producer `[36187,36762)`；[414 CSS](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/css/414.2e30559c.chunk.css) 的 `.section.about` 与 dropdown 样式单独保留 |
| 独立 About 的实际输入与错误分支 helper | [同次获取的 systray manifest](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/systray/systrayv2/manifest.json)；[714.42204095.chunk.js](../../../.analysis/inputs/frontend/host-settings-current/raw/apps.razer.com/settings/static/js/714.42204095.chunk.js) 的 `getAppData` `[223264,223588)`、本机项查找 `[93587,93691)`、`apps` 读取 `[93329,93403)` |
| Synapse 设置树、导航与生命周期 | [720.1e5d1c8f.chunk.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/720.1e5d1c8f.chunk.js)，导航 `[213847,213878)`、页面 `[215189,216046)`、根 `[255096,256325)` |
| Synapse 自启、通知、教程 | 同上，`[80204,81361)`、`[48466,49912)`、`[50171,52516)` |
| Synapse 本机键身份 | [main.31a43758.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/main.31a43758.js)，模块 9937 `[5754,11536)`；通知、语言及教程键由实际导出别名解析 |
| Synapse 语言、关于 | 上述 720 原件，`[25795,28714)`、`[77041,80062)` |
| 翻译完整模块 | 上述 720 原件，模块 1778，`[256431,258548)` |
| 自启聚合／低层 SDK | [714.d742db5c.chunk.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/714.d742db5c.chunk.js)，模块 7071 `[84352,88321)`，底层方法 `[333519,334434)`；RazerApp 自启读取 `[219174,219737)`、自启设置 `[219964,220373)`、最小化读取 `[220374,220658)`、最小化设置 `[220659,221098)` |
| 本机应用记录与自启聚合的完整函数 | 同一 714 原件，`updateAppDataItem` `[87388,87572)`、`updateAppsAutoStart` `[87676,88205)`；页面 storeData 回调在上述 720 原件 `[80095,80194)` |
| 应用扩展字段、存储与事件消费者 | 同一 714 原件，`updateAppData` 注册调用 `[217578,218833)`、LocalStorage 完整模块 9790 `[202483,203227)`、`onStorageChange` `[105932,108308)`；模块 7071 的 `getAppsChanges` 比较具名应用字段 |
| Synapse debounce 的实际 timer 所有者 | [661.2b706756.chunk.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/661.2b706756.chunk.js)，完整模块 6079 `[16475,17424)`；每次构造各自拥有 timer、cancel 与 flush |
| Chroma 设置树与根 | [75.496bba91.chunk.js](../../../.analysis/inputs/frontend/chroma-app-settings-current/raw/apps.razer.com/chroma-app/settings/static/js/75.496bba91.chunk.js)，导航 `[196785,196845)`、页面 `[198155,198729)`、根组件 `[235505,236424)`、Provider `[236534,236732)` |
| Chroma 通知、教程、自启、迁移 | 同上，`[29823,30889)`、`[47363,48422)`、`[76096,76984)`（含绑定）、`[76994,78037)` |
| Chroma 本机键身份 | [main.7c706211.js](../../../.analysis/inputs/frontend/chroma-app-settings-current/raw/apps.razer.com/chroma-app/settings/static/js/main.7c706211.js)，模块 9937 `[5754,11369)`；与 Synapse 的导出别名分别解析 |
| Chroma debounce 的实际 timer 所有者 | [602.5664d830.chunk.js](../../../.analysis/inputs/frontend/chroma-app-settings-current/raw/apps.razer.com/chroma-app/settings/static/js/602.5664d830.chunk.js)，完整模块 6079 `[13248,14197)`；75 原件的 storeData callback `[75976,76073)` |
| Chroma 独立 SDK 的应用记录／存储消费者 | [714.9923a812.chunk.js](../../../.analysis/inputs/frontend/chroma-app-settings-current/raw/apps.razer.com/chroma-app/settings/static/js/714.9923a812.chunk.js)，模块 7071 `[97849,101818)`、LocalStorage 模块 9790 `[215980,216724)`、`updateAppData` 注册调用 `[231075,232330)`、`onStorageChange` `[119429,121805)`；已独立读取，与 Synapse 对应存储语义一致，页面 timer 分支仍分别判断 |
| Chroma 语言、关于、动态灯光 | 上述 75 原件，`[26061,28296)`、`[72416,75943)`、`[14039,15662)` |
| Synapse 设置样式 | [720.dbc9cca5.chunk.css](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/css/720.dbc9cca5.chunk.css)，设置主体 `[259805,266271)`、树状复选框 `[271270,271826)` |
| Windows 自启原生函数 | [chain-audit.json](../../../.analysis/results/native/chain-audit.json) 的 startup-settings；SysUtilsNative，Image Base `0x180000000`，RVA `0x6e2d0/0x6e3c0/0x6e950/0x6b9d0`；路径 helper `0x87110/0x7a230` |
| 宿主 storeData IPC | [preload.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/preload.js) 的 `doElectronAction` `[4469,4523)`；[main.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/main.js) 的 dispatch 分支 `[42758,42807)` |
| 宿主窗口与 session 存储所有者 | [TabManager.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/components/Tab/TabManager.js) 的 `storeData` `[4119,4147)`；[TabStore.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/components/Tab/TabStore.js) 的 `writeData` `[4364,4613)`；四份宿主原件均与当前输入索引逐项核对长度和 SHA-256 |
| 普通通知的开关、测量与局部存储 | [959.f2637d09.chunk.js](../../../.analysis/inputs/frontend/notification-manager-current/raw/apps.razer.com/notification-manager/static/js/959.f2637d09.chunk.js)，`aU`／`Y6` 完整函数 `[942,1426)`、`Wy` `[2221,2419)`；LocalStorage 模块 8531 byte 26453 起 |
| 通知的实际根、观察与清理 | [main.0e07a27f.js](../../../.analysis/inputs/frontend/notification-manager-current/raw/apps.razer.com/notification-manager/static/js/main.0e07a27f.js)，lazy 入口 byte 35736 起；[Root.7b6b2b9a.chunk.js](../../../.analysis/inputs/frontend/notification-manager-current/raw/apps.razer.com/notification-manager/static/js/Root.7b6b2b9a.chunk.js)，普通根 `[21916,23285)`、window-storage 初始化／事件 `[17199,21368)`、restart 窗口复用 `[21394,21915)`；实际通知条目在 byte 6383／6896 调用 `aU`／`Y6` |
| restart 根的独立显示 | [SystemRestartRequiredWindow.16fcd6e0.chunk.js](../../../.analysis/inputs/frontend/notification-manager-current/raw/apps.razer.com/notification-manager/static/js/SystemRestartRequiredWindow.16fcd6e0.chunk.js)，`resizeWindow` `[706,1148)`；同文件完整根保留关闭、观察与语言 effect |
| 通知窗口的最终显示消费者 | 同一宿主 preload 的 `setBrowserVisible` byte 6269 起；[common.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/components/Tab/common.js) 的 `BrowserUtil.setBrowserVisible` `[24814,25388)` |
| 独立原生通知 producer 与结果边界 | [nativeNotificationHandler.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/nativeNotificationHandler.js)，handler `[1157,1806)`、worker producer `[4428,5739)`；原生 addon 静态调用不等于实际 toast 已验证 |
| 通知业务消费者的启动域、订阅与 IPC 边界 | [background-manager acquisition](../../../.analysis/inputs/frontend/background-manager-current/acquisition.json)、[notification-uri-consumers.json](../../../.analysis/results/frontend/background-manager-current/notification-uri-consumers.json)；实际 HTML、import map、主字面图 56 份 JS、515 条边及条件缓存页的额外 1 份 JS；逐项参数／计算调用、138 个完整词法所有者及当前语法调用者均在审计。otfm 的 `Se` 完整类 `[4865,45300)`、实例 `[45306,45315)`；宿主 `nativeNotification` IPC `[60074,60125)`，preload 配对入口 `[4925,4993)`；pending drain 与独立 getter／open-url 不混用 |
| 后台 worker 缓存与更新的实际消费者 | [sw.js](../../../.analysis/inputs/frontend/background-manager-current/raw/apps.razer.com/background-manager/sw.js) 路由／precache `[14381,20767)`、OTFM 条目 `[14698,14743)`；[service-worker-message.js](../../../.analysis/inputs/frontend/background-manager-current/raw/apps.razer.com/background-manager/service-worker-message.js) actions `[310,4113)`、消息 handler `[4155,5382)`；[背景入口](../../../.analysis/inputs/frontend/background-manager-current/raw/apps.razer.com/background-manager/assets/index-b4c3eb99.js) 的 worker 初始化 `[516927,519393)`、更新响应消费者 `[519025,519298)`；完整策略类与全部 precache URL 也保留于通知审计 |
| 原生通知输入与完整静态捕获 | [rzNotification.node](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/node_modules/node-rz-notification/build/Release/rzNotification.node) 与 [capture.json](../../../.analysis/results/native/5bdea80bf1fb8ef93003f56ebd624e07c25942bfa8b7b2105317c16ccf059e5e/capture.json)；Image Base `0x180000000`，IDA／Hex-Rays 9.1，3024 个函数均保留汇编，`0x83d48` 反编译失败。宿主提取、解包与本机安装副本的实际字节一致；捕获收据的 9 项输出长度和 SHA-256 已复核。捕获完整不等于全部语义闭合 |
| N-API、工作线程与释放 | 上述捕获的 RVA `0x3f00/0x3f10/0x3ab0`、`0x3050/0x3240/0x3930`；队列 `0x2640/0x2810/0x2a80/0x2d30/0x2750`，atexit `0x9c950/0x9c960/0x9c970`，WinToast 析构 `0x56c90` |
| 通知输入、图片、签名与系统设置 | 同捕获 RVA `0x1e960/0x26240/0x26420/0x5dd40`、`0x1c410/0x1c980/0x54d90`、`0x1d730/0x1e2e0/0x1cac0/0x1cdf0/0x514a0`、`0x20e50/0x218f0`；本机 URI 验签与密钥保存读取同一 nativeNotificationHandler 完整原件 |
| Toast DOM、提交与事件消费者 | 同捕获 RVA `0x56f00/0x57810/0x59de0/0x540b0/0x5b290/0x54560`；`Show` 实参额外核对 `0x58c63–0x58c70` 汇编；事件 `0x55d00/0x55bd0/0x55b10`，撤销 `0x5d070/0x58fa0/0x5c5e0/0x5c760/0x5c940`；实际 handler `0x5ef00/0x5f4c0/0x5f040/0x5fa90/0x60050` |

上述内容为完整源函数和实际调用方的静态阅读，新增的状态／timer 所有者用项目管理的 JavaScript AST 解析器核对，未执行厂商 JavaScript。Synapse 设置 acquisition 分别保留 20 个 map 获取失败和 26 个 chunk 枚举不可用项；Chroma 设置分别为 19 和 25，这两类缺项不是同一计数，也不能由已恢复工厂数量判定动态依赖已闭合。各设置页面以自己的部署收据为准，不用宿主版本替代页面版本。

独立设置本次 444 个 URL 中 406 项保留了字节，共 15,194,348 字节；入口、两个设置 manifest 及 About 真正读取的托盘 manifest 四项稳定性核对均通过，38 个 source map 返回 404。40 份 JS 的自动 chunk 枚举未提供结果；维护入口另外完整恢复 32 个注册中的 301 个词法工厂，缺失工厂和扫描错误均为零，40 份原 JS 的 AST 解析通过。原 Wakaru validate 仍以 41 项 `unresolved_reference` 返回 1，该失败保留在收据，不能以词法恢复取代验证通过。General／About 的语义审计核对 413 个原件身份；local-controls 另核 Software、选定 Systray 实际消费者、Host 和 LE 原件，共 690 个身份及 2506 个原件／分析／工具 artifact。两份页面完整捕获的 validate 失败、托盘的 38 项 source map 获取失败及 34 项枚举未决都按各收据保留；245 份 JS 的 AST 通过不能取代这些失败或任意动态依赖闭合。

Software／Systray 后续测试必须覆盖名称／产品分支、枚举部分失败、UTF‑8／分配／释放、空态与读取失败分离、安装元数据变化、Open 的实际窗口结果、五槽去重与 None、应用卸载、同数量设备替换、排序／电量／隐藏键、Chroma 隐藏不产生 enable、真正 enable 的服务返回、双击及关闭重开后的 listener／任务释放。没有设备时先测纯逻辑及模拟传输；真实原生查询须通过已授权的独立入口与当前 DLL 逐项比较，真实服务与设备输出未取得时保持未验证。该 Software／Systray 静态专项没有实施 Rust 接线或执行 DLL；前文独立记录的通知开关实现与纯／模拟验证不扩大到这些功能。

background-manager 的 117 个 URL 中 64 条记录保留了字节，获取收据的部署稳定性通过；53 条 source map 获取失败和 8 项 chunk 枚举不可用仍未解决。通知审计的字面依赖目标全部存在，58 项 precache 输入也均保留；这只证明该搜索域能够完整读取，不能取代 source map、动态值流分析、最终业务消费者或实际 DLL／OS 对照。

没有执行页面、注册表写入、登录启动、官方 DLL 实际功能对照或窗口运行验收。共享工具栏、配置迁移内部和后台窗口管理继续由各自功能文档展开；本文不把设置页入口确认计为这些模块全部完成。

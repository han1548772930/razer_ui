# 当前宿主窗口策略审计

2026-10-04。当前正式宿主 4.0.827 的 `Tab.js` 证明：从已有 Dashboard 页签发出的应用内请求，
`policy=3` 添加或聚焦宿主中的具名页签；`policy=5/6/7/8` 才进入创建原生窗口的分支。
`windowName`、`frameName` 和 `sameWindow` 的名字本身不能证明“独立操作系统窗口”。

版本与提取来源见 [current-host-version-audit.md](current-host-version-audit.md)。本次只读取
`.ref/host-4.0.827/` 和当前 `.ref/applications/synapse/dashboard/`；没有引用被删除的旧宿主。

## 当前源码调用链

1. `Tab.js:createTab` 注册 `webContents.setWindowOpenHandler`。已有同名 Tab 时先
   `sendChangeActiveTab(frameName)` 并拒绝重复打开；命中 `winMap` 时发出 `appFocus` 并返回。
   这两项发生在按 policy 创建的分支之前。Razer ID 与外部 URL 有单独处理。
2. 对新名字，`policy=5/6/7/8` 调用导入的 `createWindow`；其当前实现包含 `new BrowserWindow(...)`。
   其余请求在能找到当前宿主时调用 `sendCreateNewTabAction(currentHost, frameName, url, features)`。
   `policy=3` 若带有可解析的 `attachToWindow`，则改用该名字所属的宿主窗口；这仍然是添加页签。
3. `common.js:sendCreateNewTabAction` 发出 `createNewTab`，载荷中的 `windowId` 来自选定的宿主。
   `sendTabMessage` 通过 `tab-message` 通道发送给该窗口及其页签。
4. `TabUI.js` 收到 `createNewTab` 后调用 `createNewTab`，创建页签 DOM，保持 `windowId`，
   再通过 `sendEvent` 发出 `tab-create`。`preload.js:doTabAction` 使用 `invoke("tabEvent", ...)`。
5. `TabManager.js` 的 `tabEvent` 处理器在 `tab-create` 分支调用
   `new Tab(url, features, windowId).init()`。`Tab.js` 建立 `WebContentsView`（旧 Electron 分支为
   `BrowserView`），通过 `contentView.addChildView` 或 `setBrowserView` 挂到既有宿主。

这里的结论限定于已有 `Tab` 发起的路径。`common.js` 中其他顶层 `BrowserWindow` 的打开处理器、
外部链接、登录窗口和宿主缺失情况不能简单套用这张表。

## Dashboard 入口证据

模块 84058 的枚举把 `sameWindow` 定义为 `policy=3`，`diffWindow` 定义为 `policy=5`。
其 `_P` 打开函数先查询并聚焦同名目标，目标不存在时才调用 `window.open(url, name, flags)`。
模块 54420 的当前表中，下列入口均登记 `policy=3`：

| 具名目标 | 当前源码路径表达式 |
| --- | --- |
| `profiles` | `"/synapse/profiles/"` |
| `feedback-synapse` | `"/feedback/?app=synapse&path="+window.location.pathname` |
| `macro` | `"/synapse/macro/"` |
| `alexa` | `"/synapse/alexa/"` |
| `armory` | `"/synapse/armory/"` |

配对入口同样如此：模块 22534 的设备盒构建产品 URL，写入 `containerId`、
`displayMode=multiDevicePairing` 及非空时的 `allMasters`，然后调用模块 84058 的 `_P`，
传入 `sameWindow` 与 `autoFocus`。它没有传入 `diffWindow`。完整 AST 范围、文件 SHA-256
及跨模块收据见 [host-window-policy-current-evidence.json](host-window-policy-current-evidence.json)。

## 本地实现与配对例外

模块入口使用 `open_module_tab -> navigate` 进入宿主页签，保留 `macro`、`armory`、
`profiles`、`alexa` 和 `feedback-synapse` 等源码身份。同名再次打开由宿主页签注册表复用。
`display_window::open_or_focus` 拒绝 `WindowPolicy::Same`，防止将 policy 3 误送到原生窗口创建路径。

**多设备配对使用具名宿主 Tab。** 本地 `open_product_pairing_tab` 保留来源的
`sameWindow/policy=3` 语义，按容器、产品与序列号生成 Tab 名称并在重复打开时复用。

## 静态复核

生成：`node tools/audit-host-window-policy.cjs`。只读校验：
`node tools/audit-host-window-policy.cjs --check`。校验模式只解析并比较证据与文档，不写文件。
该工具以 Acorn 解析引用文件，静态解析 CommonJS 导入/导出与 webpack 模块，完全不执行下载的
JavaScript。此次没有运行应用、构建、测试、安装器或 DLL；静态调用链不替代运行时交互校验。

# 宏应用与 `displayMode=macro` 当前源码审计

2026-10-04 实现状态补充：下文“尚未实现”属于初次审计时的历史状态。当前已接通本地宏编辑器和部分产品的会话内绑定流程，见[当前复核](macro-current-source-review.md)与[绑定专项](macro-bindings-current-audit.md)。后者另行重审了当前 21700 与 182 产品根，不能仅凭本旧文档的链接判断符号或行为已经复核。

> 来源：当前 Synapse 包 `.ref/applications/synapse/macro/`（manifest `gitMetadata.branch = master-21sep2026`、`commit = c100ac67…`）与当前 Dashboard 包。所有结论由 [audit-macro-app.cjs](../../tools/audit-macro-app.cjs) 静态提取，逐条带文件、偏移与 SHA-256 收据；没有执行任何下载的 JavaScript。

## 结论先说

1. **宏是一个独立应用窗口**，窗口名就是 `macro`，地址 `/synapse/macro/`，打开标志 `policy=3,tab_visible=1`（附着在 Synapse 窗口上的标签页）。它不是路由，也不是产品页里的面板。
2. **`displayMode=macro` 不是第二个产品窗口**：它是宏应用在「绑定到设备」弹层里嵌入的 **iframe**，`src` 指向该产品的 `ui/index.html`，参数为 `displayMode=macro&macro=<宏 id>&containerId=<容器>&deviceEditionInfo=<版本信息>&serialNumber=<序列号>`。产品包里的 `macro` 根分支只在那个 iframe 中出现。
3. 因此「宏窗口」和「宏模块页」是同一件事：先有宏应用的窗口（模块列表里点「MACRO」盒子），才谈得上它嵌入的产品取景页。

## 窗口契约（收据）

| 事实 | 取值 | 收据 |
| --- | --- | --- |
| 窗口名常量 | `O="macro"`（Dashboard 模块 69937，导出名 `BDG`） | `main.01550b17.js` @186872 |
| 窗口登记 | `{windowName:s.BDG,url:"/synapse/macro/",openParam:"policy=3,tab_visible=1"}` | `App.72827d47.chunk.js` @73137 附近 |
| 已安装分支 | `e.macro?j.A.autoOpen(c.BD):(<安装器>)` | `App.72827d47.chunk.js` |
| 安装器 | `window.open("/installer/#type=module&id=macro&location=synapse/macro","synapse_install_macro","policy=7,tab_visible=0,browser_visible=0")` | 同上 |
| 模块盒点击 | `case f.BDG:return this.focusTab(\`${h.BDG}\`)`（盒名 `MACRO` → 窗口 `macro`） | `7861.1b0e99a4.chunk.js` |
| 映射界面入口 | `getTabInfo("macro")` 命中则 `activateWindowServiceClient("macro")`，否则 `window.open("/synapse/macro/","macro","policy=3,tab_visible=1")` 并延时激活 | `MapMacro.ffac22d2.chunk.js` |
| 安装重试 | `"macro"===n.id&&(window.open("/installer/#type=module&id=macro&…"))` | `6505.84205103.chunk.js` |

同一张表里还有 `profiles`、`alexa`、`armory`、`feedback-synapse`、`settings-synapse`、`syn3-profile-migration`、`synapse-introduction`、`firmware_update`、`background-manager`、`notification-manager` 十个窗口，完整清单见[窗口打开契约](display-window-contract.md)（`named_windows`）。

## 宏应用包

| 项 | 值 |
| --- | --- |
| 入口 | `index.html`（`<title>` 为空，favicon `/synapse/assets/imgs/favicon/KEYBOARD.svg`） |
| manifest | `short_name/name = MACRO`，`start_url = .`，`display = standalone`，`theme_color = #000000` |
| 资源 | 72 个 JS 分块，共 7,476 KB；主分块 `main.3f4b9604.js` |
| 依赖声明 | commonrepository、jstestrzdevice、anne-common、razer-anne-utilities、razer-cli-tools、razer-config 等 |

同级的兄弟应用：`alexa`、`armory`、`introduction-tour`、`profiles`、`settings`、`update-fw`、`dashboard`。也就是说「模块」在原件里各自是一个可安装的 Web 应用，而不是 Dashboard 内的页面。

## 绑定弹层里的 iframe

```js
e=`${A.Tj}/products/${i.productId}/ui/index.html?displayMode=macro&macro=${h}&containerId=${t.deviceContainerId}&deviceEditionInfo=${t.deviceEditionInfo}&serialNumber=${t.serialNumber}`
…
<iframe src={t} title="MouseBind" data-loading="1" style={{minHeight:`${o}px`,paddingTop:"10px"}} …/>
```

- 弹层容器 id：`keybind-macro-popup`；内容体 id：`body_additem_wrapper`；背板类名 `backdrop glow`。
- 高度：`window.innerHeight - heightFromPopupToTop - 40`，并监听 `resize` 重算。
- 设备列表来自 `window.allRZDevices`，只在设备支持宏（`supportMacro` 或类别命中）时出现。

## 对本仓库的要求

- 模块列表里的盒子在有本地实现时必须**直接打开对应窗口**（`focusTab(windowName)`），只有真正没有本地实现时才显示下载／安装态；这一点现在有源码依据：安装器只是 `autoOpen` 之前的一步。
- 宏窗口的内容来自宏应用包，不是产品页；产品页只在它的弹层 iframe 里被复用。因此本轮不实现 `displayMode=macro` 的独立窗口。
- 具体缺口：宏应用的界面（宏列表、编辑器、绑定弹层）尚未实现；`A.Tj`（基址常量）与 `deviceEditionInfo` 的取值来源需要单独审计。

重新生成：`node tools/audit-macro-app.cjs`；校验：`node tools/audit-macro-app.cjs --check`。机器可读收据：[macro-app-current-audit.json](macro-app-current-audit.json)。

# 当前 displayMode 分支审计

本报告按静态文本窗口统计每个设备根读取 `displayMode` 后选择的分支，用于确定独立根的真实范围。`root` 表示该分支在同一三元链里直接选择组件，`inline` 表示只切换类名或隐藏元素。没有执行任何下载的 JavaScript。

扫描 331 个产品包；296 个读取 `displayMode`；246 个存在根级分支。

## 根级分支计数

| 模式 | 产品数 |
| --- | ---: |
| `armory` | 226 |
| `chromaApp` | 212 |
| `macro` | 174 |
| `multiDevicePairing` | 30 |

## 注册导航与根分支的差异

| 模式 | 注册的非主导航 | 审计到的根分支 | 注册但无根分支 | 有根分支但未注册 |
| --- | ---: | ---: | --- | --- |
| `multiDevicePairing` | 31 | 30 | 221 | 0 个 |
| `chromaApp` | 1 | 212 | 3886 | 212 个 |

## 说明

- 计数是产品包个数，不是独立界面数；同一产品的多个分支共用同一份样式与状态。
- `inline` 分支（例如 `chromaApp` 时隐藏设备图）不产生独立界面，但会改变默认根的呈现，实现默认界面时需要一并处理。
- 只扫描注册审计记录的主导航源文件；35 个产品在该文件里没有读取 `displayMode`（根选择器可能位于懒加载分块），这些产品的分支需要单独核对。
## 各模式由谁打开

| 模式 | 打开者 | 窗口名 | 地址 |
| --- | --- | --- | --- |
| `macro` | macro application window | `macro` | `/synapse/macro/` |
| `armory` | armory application window | `armory` | `/synapse/armory/` |
| `chromaApp` | Chroma application window (separate app) | `chroma-app` | `/chroma-app/dashboard/` |
| `multiDevicePairing` | Dashboard device box and the product-side pairing helper | `multi-device-pairing` | `/synapse/products/<pid>/ui/index.html` |

- `macro` 不是独立产品窗口：它是宏应用（`/synapse/macro/` 窗口）在「绑定到设备」弹层里嵌入的 iframe，参数 `displayMode=macro&macro=<id>&containerId=…&deviceEditionInfo=…&serialNumber=…`，因此产品包的 `macro` 分支只在那个 iframe 里出现，产品根本身不会自己开窗。
- `chromaApp` 在当前 Dashboard 包里出现 `0` 次：Synapse 不打开这个模式，它属于独立的 Chroma 应用窗口。

本地 `macro`、`armory`、`profiles`、`alexa` 和 `feedback-synapse` 通过宿主具名页签打开。当前 4.0.827 的 `Tab.js` 将 `policy=3` 分派到已有窗口的标签栏，同名再次打开时聚焦已有页签；旧版文档把它解释成第二个 gpui 窗口的结论已撤回。多设备配对仍按历史明确要求保留第二窗口，这是本地例外，其源调用同样传 `sameWindow`。详情见 [宿主策略审计](host-window-policy-current-audit.md)。页面与设备服务仍按各自边界记录。

- 窗口名、可见性与聚焦标志、每个模式的 URL 参数见 [窗口打开契约](display-window-contract.md)。
- 逐产品的分支、参数、引用组件与文本窗口范围见 [机器可读清单](display-mode-audit.json)。

重新生成：`node tools/audit-display-modes.cjs`；校验：`node tools/audit-display-modes.cjs --check`。

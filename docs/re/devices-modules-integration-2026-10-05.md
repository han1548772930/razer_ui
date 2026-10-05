# Devices & Modules 主路由接入（2026-10-05）

本轮依据 `ui-source-review-2026-10-05.md` 的 M01–M07，重新读取当前 `.ref/applications/synapse/dashboard/`。只运行维护中的静态 Acorn 提取器、JSON/资源校验与格式化；没有运行应用、构建、测试、下载的 JavaScript 或 DLL。

## 已接入的部分

主导航的 `main_pages.rs::modules_page` 现在直接挂载 `ModuleCatalog`。其实现拆至 `devices_modules_catalog.rs`，由 `service_pages.rs` 包含；这里实际依次调用四个组组件：

1. `FIRMWARE_UPDATES`。
2. `NEW_DEVICES`。
3. `AVAILABLE_MODULES`。
4. `UPDATED_RECENTLY`。

每组遵循当前 `44442/H`：空 items 返回空，不显示空标题；非空组宽 1220、左右居中、下方 margin 40。没有额外“设备”“模块目录”标题、顶部 30px 间距或安装服务说明段落。

目前第三组可用的是本地已经编译的模块页面。这是用户要求的直开策略，不是虚构的原生 installer 完成回执。成员从该页自己的 `ne` 表产生，顺序为 Alexa、Macro、linkedGames、Feedback、Armory；每项直接发出既有 `ModuleCatalogEvent::OpenModule`。Tour 与 ProfileMigration 保留各自 Dashboard/应用入口，本页不混用它们。

撤下了旧主路径自行创建的设备快照行及其 400px 居中 `DeviceDetails` 检查器。它们不属于当前 `H → w/O/L` 树。设备的本地入口仍由 Dashboard 保留；这里没有把历史快照冒充新连接设备、已安装设备或可升级设备。

## 当前源码与机器证据

当前路径：`App.72827d47.chunk.js` 的 `35378/x` 第三导航 → `6505.84205103.chunk.js` 的 module 44442 → `ae → H → L/w/O`。

- `ae`：从 installedDevices、installedModules、connectedDevices、deviceRuntimeData、uninstallingDevices/Modules、firmwareUpdateDevices 合成四组。
- `H`：空组不挂载，固件组用 L，最近更新组用 O，其余用 w。
- `ne`：本页五成员目录；`te`：Macro/Alexa 十语言完整介绍；`ee`：十语言标题和 Feedback beta 分支。
- `ie`：Armory 按 isExchangeEnabled 同时选择 Workshop/Exchange 标题和图标。
- `u`：源静态 detail.size 等于 0 时显示 `< 1 MB`，无须安装服务。

维护工具 `tools/extract-devices-modules-catalog.cjs` 只解析这些 AST 数据；输出：

- `src/shell/devices_modules_catalog.json`：实际消费的五成员、十语言标题/介绍和静态 detail 数据。
- `docs/re/devices-modules-current-catalog.json`：当前路径、SHA-256、UTF-16 偏移、上述源组件原文、77989/i 特性 hook 与提取结果。

重新生成：`node tools/extract-devices-modules-catalog.cjs`。只校验：加 `--check`。提取器把 Feedback 的 `X.Vb` 两个分支分别保存，未执行依赖 `window.apiElectron.isBeta` 的表达式。

## 本轮恢复的样式与文案

CSS 来自当前 `6505.9782778c.chunk.css`，原文与偏移同时保存在先前的 `ui-source-review-2026-10-05-evidence.json`。

| 源选择器 / JSX | 本地恢复 |
| --- | --- |
| `.items` / `H` | 宽 1220，margin `0 auto 40px`；空组不显示 |
| `.item-main-content` | 高 80，底间距 1，左右 padding 20/30，背景 #111 |
| `.item-icon` | 模块使用自己的源 logo，40×40 |
| `.item-name` | 固定 500，Roboto 16，#ccc，省略号；不再允许名字列缩小 |
| `.info-text` / `module_detail_action` | Roboto 14，#707070；展开/关闭用现行语言键 |
| `.item-moreInfo.item-description` | 背景 #2d2d2d，padding 20，min-height 202 |
| `.item-description-image` | 288×162，右间距 20，contain |
| `.item-description-info` | 宽 592，Roboto 14，#ccc |
| `.item-description-content` | 下间距 20，使用 `te` 的原文，删除工程说明 |
| `.item-description-extra-info` | Learn More 与 Size 同一横排；fileSize 左自动间距、右间距 20；显示源 `< 1 MB` |

`module_preview.rs` 与主页面现在调用相同的 `module_description` 和组标题函数，因此字号、原文及文件大小不会在两个入口继续分叉。操作按钮保留已核过的 90 最小宽/27 高和 200ms opacity 过渡；展开描述与源 `A ? ... : null` 相同，未新增猜测动画。

## 状态边界和仍需完成的工作

三个服务组是实际分开的 typed optional presentation slots：`None` 表示未观测，`Some(empty)` 表示服务已确认空。当前没有原生服务适配器，所以这些 slots 都尚未被填充；它们不是已经接通的安装、卸载或固件状态模型。完整服务行仍只在显式标注的 `ModulePreview` 样例中可审阅，本轮未把样例进度、日期或假设备搬入主页。

当前 `Device` 快照只有旧的 current_fw_version、setupStatus 等信息，没有可验证的 installedDate、installedDevices、needsUpgrade、firmwareUpdateInfo 或 installer 进度。由这些快照推断 NEW_DEVICES、UPDATED_RECENTLY 或 FIRMWARE_UPDATES 会歪曲源条件，因此没有这么做。

Armory 用 `Option<ObservedArmoryFeatures>` 保留四个真实源 flag 的边界；可计算的公式是 `(!profileSharing && !macroSharing && !chromaSharing) || !armoryNameChangeToWorkshop`。未观测时保持当前 hook 的初始 `isExchangeEnabled=false`（Workshop），不当作服务返回 false。已准备好 Exchange 原标题与 `module-armory-exchange.svg` 的匹配分支，特性服务本身仍未接入。Feedback 的 host beta 状态也使用 optional 字段，缺少 host 布尔值时遵循源的普通 Feedback 分支。

后续必须继续完成：

- 将真实服务记录投影为正式的 L/w/O 行组件，并接入三个 optional 分组；包括安装、取消、重试、卸载、断开设备、日期和固件条件。
- 真正的设备行必须使用当前 `96689/c` 按 category/subCategory 返回的分类图标。本轮删除了主路径把实物缩略图误当类别图标的消费者；未声称已把所有分类 SVG 接入生产行。
- 原版行内固件详情与删除确认（`.profile-del` right30/top51、Macro 特殊内容）仍待正式服务行接入。已删除的居中快照检查器不再被当作源详情。
- `ModulePreview` 剩余状态的精确布局、时间格式及 popup 行为还需要逐条复核。本轮只提升了共享描述和标题组件。

本轮未修改 Dashboard、Gamer Room、shell 模块事件路由或硬件服务。统一 `cargo check --locked --all-targets` 由主任务执行。

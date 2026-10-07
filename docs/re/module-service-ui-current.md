# Devices & Modules 当前界面与服务观察

当前 Dashboard 的 `35378/x → 44442/ae → H → w/O/L` 是正式挂载链。`main_pages.rs::modules_page` 直接挂载 `ModuleCatalog`，`devices_modules_catalog.rs`、`module_service_rows.rs` 和 `module_service_remove.rs` 由正式页面消费。静态来源是当前 `.ref/applications/synapse/dashboard/`，没有运行厂商 JavaScript、应用、测试或 DLL。

## 页面与本地入口

四组顺序为 Firmware Updates、New Devices、Available Modules、Updated Recently；空组不显示标题。组宽 1220、居中、下间距 40；主行高 80、左右 padding 20/30、分类图 40、标题列 500、文字 16。详情区最小高 202、padding 20、图片区 288×162、正文列 592；操作区最小宽 90、高 27。描述展开按源条件挂载，不添加未证实动画。

Available Modules 使用本页 `ne` 的五成员及顺序：Alexa、Macro、linkedGames、Feedback、Armory。十语言名称、Macro/Alexa 介绍和静态大小来自当前源；零大小显示 `< 1 MB`。Armory 使用已观察特性选择 Workshop/Exchange 标题和图标；未知时保留 hook 初始 Workshop 显示，不当作服务返回。Feedback 的 host beta 同样是可选观察。

已实现的本地模块直接打开。真实安装 phase 可显示，但不阻断本地页面。新设备行只打开身份匹配且已存在的本地产品工作区。页面没有额外“Razer 应用”按钮；发现软件入口属于独立 `rz-app-menu` 页脚，目标 `https://razer.com/pc/software`。

## 快照与投影

`features/module_service.rs` 读取工作区 v2 的可选 `module_services`。缺失/null 表示没有服务快照；取得快照时以下集合必须一起提供，空数组才表示已观察为空：installedDevices、installedModules、connectedDevices、deviceRuntimeData、cachedDeviceInfo、deviceManifest、uninstallingDevices、uninstallingModules、firmwareUpdateDevices、installerStatus。设备和服务对象保留未消费字段；`isOnline/canRemoveMacro/armoryAvailable` 是可选条件，不将未知当 true。

快照已接工作区读取和两条本地保存路径。它不是当前原生安装服务读取器；普通设备快照、支持产品目录或点击动作不能生成安装日期、连接集合、进度或升级结果。`ModulePreview` 是独立的显式预览。

- 新设备由 connectedDevices 减 installed PID，再按运行时 PID/serial 展开；没有运行时 serial 且未指定 serial 时才查缓存。
- 一般设备、MONITOR、IoT 使用各自断连/移除条件；一般设备考虑同 PID 数量，IoT 看子设备 power/locked/online；NOSERIALNUMBER 别名按源过滤。
- Recently 顺序为卸载设备、卸载模块、安装设备、安装模块；仅正常安装设备按 connected-first 稳定排序。模块名称/图标/可移除元数据来自当前目录，不受安装记录任意字段覆盖。
- 固件组先取 needsUpgrade 的 in-app 候选，再取 runtime firmwareUpdateInfo。外部项只与原始 in-app 集合去重，保留外部集合内部同身份项的顺序。
- 日期保持源 JS Date 毫秒语义、秒/毫秒分支、falsy/today、缺失/null 及 ISO UTC/本地差别；任意 V8 非标准日期字符串解析仍非完整兼容。

## 行状态、移除与固件

进度仅插值绘制宽度，阶段和可访问百分比仍取实际快照。200×8 条、轨道 `#2c5824`、指示 `#44d62c`，初始 0→目标为 300ms linear；下降按上次值先到 100、1ms 回零、再 300ms 到新目标。最新快照打断展示序列；减少动画直接显示目标，不复制 JS 多 transitionend 回调竞态。

移除要求最新观察仍可移除，Macro 额外要求明确 canRemoveMacro。菜单关闭 100ms 后打开，延迟完成与确认再次检查身份、连接/卸载状态；导航、刷新和销毁取消旧任务。确认定位行内 right30/top51、min-width300、padding20、红边/标题/按钮、300ms 透明度；外点和 Escape 取消并归还焦点。Macro 使用真实可选 macroAssignments 名称列表；其他模块的清除设置项取 `44442/E` 键表。

确认仅发 `ModuleCatalogEvent::ServiceCommand { action:"remove", record, clear_settings }`。安装/取消/重试/移除都不自行插入 completed/uninstalling 或删除快照。移除 spinner 复用与当前源字节一致的 `alexa-spinner.svg`。

固件行按当前 SDK dongle/BLE 限制、warning severity、USB/dongle release、日期/大小/说明/指南显示。名称优先 productName 再 title，只有 warning 品类图变橙。外链使用当前同字节 SVG；指南打开实际 URL，SDK 入口进入本地固件页，SDK 状态未因此被确认。

## 仍需完成

真实安装/卸载/固件服务快照生产者、完整重连更新和 beta/Armory 特性查询仍未连接。详情 `detail.srcImage` 缺经过验证的资源映射，保留源尺寸空区，不拿同 PID Dashboard 缩略图替代，也不假装 onError 已发生。尾部 `animated_startup_dotted_scaling.d5d9ac9c.svg` 仍缺；确认框 DOM 50ms 溢出滚动/帮助高度、部分 tooltip/按钮细节及实际字体、焦点、滚动、动画尚未验收。DLL 写回后置不等于免除 UI 操作及只读观察工作。

## 维护证据

- [目录与挂载](devices-modules-current-catalog.json)：`extract-devices-modules-catalog.cjs`。
- [源记录、移除和分类 SVG](module-service-current-evidence.json)：`audit-module-service.cjs`。
- [行状态、进度与固件](module-service-rows-current-evidence.json)：`audit-module-service-rows.cjs`。
- [缺失媒体的离线核对](shortcuts-service-media-offline-recovery.json)：`recover-shortcuts-service-media.cjs`。负面结果不表示资源已恢复。

JSON 中的 AST/CSS/资源事实保留；取证时 native 文件哈希不构成整个页面的运行验收。维护检查限于静态解析、资源校验、格式及允许的全目标类型检查。

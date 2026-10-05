# Devices & Modules 服务记录与正式行接入

本次延续链接会话 `01a109ca-c22e-73e3-9de0-a3eba5f79bc6` 的未完成工作。当前 Dashboard 的 `44442/ae → H → w/O/L` 是唯一页面行为来源；使用当前 manifest 中的 JavaScript/CSS 做静态解析，没有执行厂商代码。

## 数据入口与边界

原来的 `ObservedModuleServiceRows<Option<Vec<AnyView>>>` 空展示槽已撤下。新增 `features/module_service.rs` 对实际源记录做投影，由 `ModuleCatalog` 的正式行组件消费。

版本 2 的本地 workspace 文件新增可选 `module_services`。缺失/`null` 表示未取得服务快照；现有文件无需迁移，也不会从普通 `devices` 快照推断安装日期、连接清单或升级结果。有服务快照时，下列集合必须同时提供，空数组才表示该集合已观测为空：

| 本地快照字段 | 当前源含义 |
| --- | --- |
| `installedDevices` / `installedModules` | 安装服务的 PID/moduleName 和 installedDate |
| `connectedDevices` | deviceReducer.devices |
| `deviceRuntimeData` | commonReducer.validDevices，含序列号/容器/IoT 子设备/固件记录 |
| `cachedDeviceInfo` / `deviceManifest` | 缓存设备记录及源安装包描述/大小 |
| `uninstallingDevices` / `uninstallingModules` | 服务提供的卸载中身份集合 |
| `firmwareUpdateDevices` | fwReducer.devices 的 in-app 升级候选 |
| `installerStatus` | 以 PID/module ID 为键的 phase/downloadedPercent/installedFiles/totalFiles |

`isOnline`、`canRemoveMacro`、`armoryAvailable` 是可选观测条件；未观测不视为 true。快照保留额外字段，设备/服务对象用 JSON map 保留暂不消费的字段，不在本地保存时静默删除。可选 `macroAssignments` 是当前源 `z` 使用的配置映射扫描结果的本地边界，尚无原生扫描适配器。

该入口接在已有 workspace 读取和两条保存路径上。**没有新增原生服务读取、安装、卸载或固件 SDK 传输**。本次没有给实际配置文件写入示例快照，也没有给页面填入假设备、假安装日期或假进度。历史 `ModulePreview` 仍是单独的显式演示入口。

## 投影与正式页面

- 新设备由源 connectedDevices 减 installed PID 集合，再按运行时 PID/serial 展开；没有运行时 serial 且未限定 serial 才查缓存。
- 一般设备、MONITOR、IoT 的断连与可移除条件分别回溯 `ae.v`。普通设备考虑同 PID 连接数；IoT 读取子设备 power/locked/online。NOSERIALNUMBER 的离线别名按源条件过滤。
- 最近更新组严格保持 `[卸载设备, 卸载模块, 已安装设备, 已安装模块]`；只有正常已安装设备做 connected-first 稳定排序。模块元数据取当前 `ne/ie` 目录，安装记录中的任意 title/icon/removable 不覆盖目录。
- 固件组由 needsUpgrade 的 in-app 候选先行，随后加运行时 firmwareUpdateInfo；外部候选只与原始 in-app 集合去重，不误删外部列表内部的同身份记录。
- Available Modules 依已安装模块清单过滤。已经存在的本地模块直接打开；真实 installer phase 可显示在同一行，但不拿它门控本地页面。新设备行只对同身份、已有本地产品工作区提供直接打开。
- 正式安装行包含分类 SVG、标题、断连 dim、最后更新日期、移除中显示、100ms 延迟移除确认、清除设置与 Macro 说明。移除前重新核对库存和条件；离开主路由取消延迟任务，防止迟到焦点跳转。
- 正式固件行包含 SDK dongle/ble 限制、warning 强调、USB/dongle release 信息选择、原版日期/大小/发行说明和指南。外部指南打开实际记录中的 URL；SDK 入口进入既有本地固件页，SDK 状态本身仍未连接。
- 安装/取消/重试/移除只发语义命令；未连接的处理器报告实际边界，不修改快照为 completed/uninstalling，不清除真实设备或原版模块。

日期新增直接依赖已在 Cargo.lock 中存在的 chrono 0.4.45。保留 O 的 JS Date 毫秒语义与 W 的秒/毫秒分支、falsy/today、缺失/null差别、locale 格式以及 ISO UTC/本地差异。V8 对任意非标准字符串的宽松解析不保证完全复现。

## 来源和资源

- [主源收据](module-service-current-evidence.json)：`tools/audit-module-service.cjs` 提取 11 个 AST 合约、6505/55 CSS、34 个分类原 path/最终 transform 以及固件警告原内联 SVG。
- 新资源通过独立 `module-service-embedded.rs` 注册，避免多个资源准备器互相覆盖；`validate-resources.py` 检查源/输出 SHA-256、嵌入键与 SVG 格式。
- [服务行专项复核](module-service-rows-current-audit.md)：进度条的真实颜色、几何、300ms/1ms width 序列、固件图标/名称级联与外链原图标。
- [移除确认专项记录](module-service-remove-followup-2026-10-05.md)：当前 15597/A 内容、尺寸、延迟、生命周期与资源边界。

## 尚未完成

没有运行窗口，因此不宣称字体/滚动/焦点/动画的像素验收。真实服务适配器及完整重连更新仍缺；本地快照目前由 workspace 加载。宿主 beta/Armory 特性服务未接入。详情 `detail.srcImage` 尚无经过证明的资源映射，保留原尺寸空区域，没有拿 Dashboard 图片代替。移除中尾部动态点 SVG、删除确认的 DOM 溢出滚动及部分 tooltip/按钮细节仍有缺口。快速连续变化时的原 JS transitionend 多回调竞态没有复制。

允许的最终验证结果记录在 [本批交接](continuation-followup-2026-10-05.md)。

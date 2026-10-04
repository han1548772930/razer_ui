# Profiles 当前应用复核

2026-10-04。当前源码 `.ref/applications/synapse/profiles/`；[静态提取器](../../tools/audit-profiles-app.cjs) 按 webpack 模块局部作用域解析，不执行下载的 JavaScript。[机器收据](profiles-app-audit.json) 保存实际节点、文件偏移、SHA-256、十个语言包和 CSS。

## 已撤销的旧结论

旧报告将共享 URL 正则当成应用自己的五个页签，并称压缩符号无法解析、缺少中文资源。这些结论错误。CSS 中出现某个类，也不能证明该类已经挂载。

## 当前实际挂载

模块 43 的 Ba 根挂载 Xt 应用工具栏和内容。Ba.nav 只有两项：`ey1 → GAMES_HEADER → 游戏`、`db_ → DEVICE → 设备`，默认 Games；标题 `L$3 → LINKED_GAMES`。引用经模块 4693 的 export getter 解析。工具栏的前进/后退调用应用页签历史，页签行不再另加箭头。

游戏主列表使用 `.game-tile` 290×220、body 150、footer 70；设备页中嵌入的 `.linked-game-tile` 240×190 是另一个组件。添加磁贴的三段文案是 CLICK_TO_ADD、GAME_PROGRAM、DRAG_AND_DROP_HERE。删除游戏筛选不显示添加磁贴。

添加弹层由模块 3137 f → 5529 r 挂载。宽度虽然是 1050!important，但祖先命中的 max-width 为 800。固定 backdrop 的 margin-top 为 89；mat 的 margin-top 为 110、show top 为 20。GPUI 宿主栏为42，故窗口坐标终点为 42+89+110+20；100ms opacity、300ms top。head 高度仍为36，后续 padding 覆盖不能误加到49。此计算来自当前级联，尚无运行时像素验收。

本轮准备该应用工具栏与弹层原始SVG（含hover、灰色搜索、清除、关闭、返回和背景光晕），资源清单保留原文件hash。搜索为133×20，筛选框最大160；弹窗重开使用独立动画标识；当前静态收据包含314条CSS规则。

## 当前实现边界

本地已改为两个真实视图、应用内历史、搜索/筛选、添加磁贴和添加弹层空目录。设备视图现在使用本地设备实体绘制 290×220 磁贴；设备关联弹层使用当前 Profile 选择、重命名、真实本地关联记录、240×190 关联游戏磁贴、三态选择框和添加弹层返回链。已安装程序与扫描、全局游戏目录、设备子设备/editionName、拖放和完整服务筛选状态没有本地输入，保持空态或沿用本地已知关联；不能把这些 UI 状态当作服务同步结果。剩余弹层分支和最终动画/像素仍待复核。UI完成后再统一接后端/DLL。

静态契约通过不等于界面完整，也不证明视觉一致。

## 本轮边界增量（2026-10-04）

Profiles 的游戏扫描入口、已安装程序刷新、Browse 以及设备 Profile 的导入/导出/复制/删除菜单均保留原始控件外框，但在服务未接入时显式显示不可用提示；不会触发扫描、文件选择或伪造服务响应。添加游戏弹层的空目录现在显示 `GAME_NOT_SEE` 与 `Game discovery service unavailable`，使空状态可验证并与本地已知关联记录区分。设备关联弹层菜单中的服务命令同样提供 `Profile service unavailable` tooltip，重命名仍使用本地 ProductWorkspace 元数据路径。

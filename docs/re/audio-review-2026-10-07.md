# 音频组已有页面复查（2026-10-07）

本批扩展到 `audio_products_data.json` 的全部 **76 产品 / 309 页面**。逐产品回读 119 个实际参与页面树的当前源文件 SHA，并以完整文件 Acorn AST 核对 6835 条组件范围；每页均记录导航及直接 JSX 入口。EQ、播放、混音、麦克风、灯光、OLED、触觉等页面各自列在 [逐页子树矩阵](audio-review-current-evidence.json)，没有按代表型号推断其他产品。

这完成了每页当前源子树与已有控件清单的核对基础，**不等于 309 页的全部条件和交互已经复查或完成**。矩阵把 `source_ast_status` 与 `ui_review_status` 分开；本批深入核对并修复的是下面的灯光依赖。EQ 与混音的完整编辑/条件/服务分支仍明确保留在逐页 `unresolved` 中。

## 已修复：33 产品的 99 个灯光控件依赖

逐页查到源 `checkDisplay`、`checkIdle` 的 `disabled: !this.props.brightnessOn`，以及 idleMinutes 的 `active: this.props.switchOffLighting.isIdleEnabled && this.props.brightnessOn`。原本地 descriptor 丢失亮度依赖，关掉亮度后这两项复选框和空闲分钟滑块仍可编辑。

本轮为每个实际匹配上述源表达式、且有真实本地 brightness 字段的控件补入依赖：两个复选框用 `enabled_by`；分钟滑块保留原 idle 开关门控，再加亮度 `enabled_all`。`AudioProductWorkspace::enabled` 同时控制渲染禁用和 `edit` 写入口，所以旧事件也不能绕过门控修改草稿。

修复产品为：1306、1308、1313、1318、1319、1325、1330、1331、1335、1352、1353、1354、1364、1370、1372、1376、1378、1383、1387、1389、1391、1396、1406、1407、1415、1422、1439、1443、1446、1453、1462、**1465 的独立 LIGHTING**、3942。

总计核对 35 产品的 101 条源门控；1303/1304 原有两条门控已存在，另外 33 产品各三条、共 99 条本轮修复。每条记录保留具体产品、页面、native path、原 source SHA/范围和 JSX 条件，不应用“所有音频都一样”的家族默认。

`tools/prepare-audio-products.cjs` 同步按各页原 JSX 生成这些条件。目标数据通过 `tools/audit-audio-review-current.cjs --apply` 更新，仅补已证实的依赖；既有 OLED、混音、Nommo 等后续数据及覆盖缺口保留，不用整个基础生成器覆盖其他特定页面改动。

## 仍需逐项复查/完成

- 76 产品每页的具体运行条件、完整布局、键盘/鼠标行为仍需继续核对。EQ 的 77 个描述符并不证明 preset/reset/重命名、门控和全部图表交互完成；混音的静态总线与选择控件也不代表输入通道增删、真实播放列表和全部预设操作完成。
- 当前音频数据仍显式存在 3 个空页，既有验证记录保留这些缺口；不能以其它独立 Demo 实现或源组件存在擅自归零。
- 本轮未改变任何 DLL 写回、播放或设备数据；修改只影响已有本地草稿控件是否可编辑。源灯光值与设备观察仍是不同数据范围。
- 未运行应用、构建、测试、厂商 JavaScript 或 DLL；没有实窗、音频播放或设备读写验收。

## 检查与范围队列

维护工具 `audit-audio-review-current.cjs --apply` 已完成上述完整源 AST 核对及 99 项依赖修复。随后 `validate-audio-products.cjs` 通过：76 产品、1756 控件、77 EQ、2829 当前来源哈希；3 个空页继续显式记录。源 AST 出现在矩阵里与交互完成严格分开。Cargo 由父任务统一执行。

本组音频之外的范围在 [System/AccessorySystem/SourceControls 待审矩阵](accessory-system-review-queue-2026-10-07.json)：89 产品、325 主页面。该文件是范围盘点，只把本轮接收器/底座明确回读的 8 页标记局部复查；其余页面为待审，完整 UI 通过数为 0。

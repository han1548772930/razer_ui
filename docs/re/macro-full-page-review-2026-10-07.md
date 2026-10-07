# Macro 全页面复查（2026-10-07）

本轮重新解析当前 `.ref/applications/synapse/macro/asset-manifest.json` 指向的 main / 8190，不使用历史前端快照。`tools/audit-macro-full-page.cjs` 记录业务闭包 `Mn/hn/vn/Zs/$a`、实际 UI 根和相关 reducer、CSS 及 native 文件摘要。摘要匹配只说明复查对应这些文件，**不是界面完成或运行验收**。

## 已确认并改动

| 源入口 | 原本差异 | 本轮改动 |
| --- | --- | --- |
| 58190.St | 每个 Delay 行多出源码没有的模式切换按钮；仅按行 state 判断范围编辑 | 移除按钮；必须当前宏 delaySetting=2 且 Number 为范围才显示双输入，否则显示普通数值或原版随机范围说明。模式变化结束旧范围编辑；延迟显示字体恢复 14px。 |
| 58190.St / 25572.O / 47990.Ay | 新 Delay 显示 0.000，普通数值提交总是固定三位，仅失焦提交 | 模板是 Number=0，新增显示 0；普通输入依当前 St 在 150ms 后写动作草稿，失焦/Enter 完成编辑，整数不补零。定时回调被编辑器、文档、重排、undo/save 切换失效。录制设置不会自动把新增或旧 scalar 变成范围。 |
| 58190.br / Fr | 录制说明插在 actionbar 和列表之间；录制时仍显示 undo/redo/save | 录制配置说明放 actionbar 右侧，录制时隐藏历史/保存按钮，左边保持时长显示。错误/结束状态说明留在左侧工具区，明确属于本地错误反馈扩展，不改变列表高度。总时长也计入 MouseMovement.Number。 |
| 58190.ja / Le / 15030.cf | 悬停缺少配对连接线 | 标准/序列列表增加悬停配对虚线与端点，沿源 42px 行高、left39、行中点定位；配对使用 cf 所需身份与键盘 state parity。未冒充已完成 phased 分组线或 CSS 动画。 |
| 主业务 Mn / 25572.FM / N | Import / Export 永久禁用且无文件链 | 主菜单连接 `.xml` 文件选择、后台读写、版本/数据验证、源名字冲突规则、导入新的交换 GUID、本地库发布、取消/失败反馈、当前动作草稿导出和保存路径选择。导出临时文件写完同步后才替换所选文件。 |

## XML 文件契约与限制

- 按 Mn 导出 `Macro{Name, MacroEvents{MacroEvent}, DelaySetting, Guid, Version:4, MouseMoveType}`，三个空格缩进。录制元数据是 actionBar.recordProfile.mmtSetting。宏的本地数字 id 从不充当文件 GUID。新建/复制/导入生成独立交换 GUID，库校验保证唯一且有效。
- `hn` 支持的现有动作是 Delay、Keyboard、Mouse、Movement、Text、Command、Loop、Nested Macro、Launch。按源导入规则重建 Keyboard/Mouse Id；**Loop 导入没有配对 Id，不按位置自造身份**。未解析的 nested GUID 单独保留，既不按名字猜目标，也不表示设备已读到。
- `hn` 的 scalar Delay 导入是 `parseFloat(...).toFixed(3)`；St 逐行编辑则是整数/最多三位小数。这两个不同入口不能合并成一种显示规则。
- 当前 `hn` 会把对象型随机延迟 Number 转成 NaN，并丢弃 phase。此类文件明确报告兼容性缺口，整份导入不提交。导出仍保留当前动作的 Number 对象及 phase，不能称所有导出文件可重新无损导入。
- `dn` textarea -> `25572.NH` 保存 Text 原文；Mn 导出无额外 HTML 转义。但 Mn 导入 `X` 会在 XML 解码后再解码 `&amp; &lt; &gt; &quot; &#39;`。若这种第二次解码会改变字面文本，native 导入/导出明确拒绝并报告，避免静默损坏或自行引入不同语义。
- 旧 Synapse 3 的 `vn` 路径、当前未实现 UI 的动作类型不假装兼容；文件明确失败，不静默跳过动作。XML DTD 不允许；支持范围内的普通 XML 文本和 CDATA 保持完整。
- Movement 来自文件时明确 `imported_xml=true`、geometry=None。这不代表得到屏幕/DLL 实时观测；原生录制行依旧要求真实 geometry。当前 XML 不会执行命令、启动程序、播放宏或调用 DLL 写入。
- 导入完成时重新查看当前库，只有仍没有宏才选中新宏。用户等待文件读取期间的草稿不会被快照覆盖。导出克隆当前 `self.actions` 生成文件；`publish_library` 只发布 `entries` 中已明确保存的动作，不把导出当作编辑器 Save。
- 本地持久化由既有 MacroLibrary/AppShell store 处理。导入提示仅说进入本地库，并指向窗口的本地保存状态；导出成功只在实际文件写入完成后出现。都不是设备保存成功。

## 复查后仍未完成

以下项目不能用 routes、描述符、源码 receipt 数量或 cargo check 代替验收：

1. ja 的长列表虚拟化、phased 分组配对连线和 Le 的 200ms 背景虚线动画。
2. 200ms 首次响应/尾部防抖选择行为、完整 toolbar tooltip 与窄窗口导航收纳。
3. 上述 XML 旧版/随机延迟/阶段/字面实体兼容性缺口；导入 Loop 的缺失配对身份仍依源保留。导入/export 不代表所有宏互操作闭环完成。
4. Key Binds 的实际设备/服务观测与绑定回传、宏全局录制快捷键、鼠标轨迹录制与真实屏幕观测。界面本地编辑不能代替这些观测。
5. 实际渲染、键盘/焦点、滚动/悬停、系统文件选择与文件持久化运行验证。按用户约束，本轮不运行应用、测试、安装包、vendor JS 或 DLL。

主导航 inactive #5d5d5d、列表 `max(outerHeight-208,450)`、600px editor 及 <=1120 左定位均能从现行源码闭包/CSS 对上，本轮未根据过时报告误改。Key Binds/Help 实际路由仍分别是当前 21700/1519；此前 scoped 实现不等于本轮全页已无差异。

验证：rustfmt 与维护工具的 AST/CSS/原文件校验。`cargo check --locked --all-targets` 由根任务统一执行；本报告不预先声称其成功。

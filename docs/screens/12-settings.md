# Settings：独立设置应用

更新日期：2026-10-02。原版来源为静态取得的 [720 chunk](../../.ref/settings/static/js/720.1e5d1c8f.chunk.js)、[对应 CSS](../../.ref/settings/static/css/720.dbc9cca5.chunk.css) 与 [下载记录](../../.ref/settings/source.json)，未执行原版脚本。之前“缺少独立 Settings render”的结论已过时。

`ho` 渲染 Synapse：左列 `Ks` 自动启动、`bn` 通知、`Ia` 推荐；右列 `Un` 教程重置、`Ta` 配置迁移、`ia` 设备灯光。`uo` 渲染 General：左列 `Te` 语言与发行说明，右列 `Fs` 关于。导航只含 SYNAPSE/GENERAL；本项目另设“服务连接”，集中放置本地预览及连接工具。

| 原版细节 | 当前实现 |
| --- | --- |
| 设置标题覆盖共享样式为18px；语言下拉188px | [settings_page.rs](../../src/shell/settings_page.rs) 使用对应尺寸，导航与保存栏固定，正文独立滚动 |
| `Ie` 的10种语言 | SelectState 和下拉渲染传入同一组选项，修复空列表入口；同步全局locale及设备内保留的选项，丢弃恢复已保存语言，命令行覆盖不会被误标为已落盘 |
| `Ks` 的最小化选项依赖自动启动 | 恢复30px缩进、37px竖线、13px横线及10px说明间距；原生启动偏好未读取时禁用控件，不把源码默认勾选当作系统状态 |
| 关闭推荐总开关不清除各分类及发布/优惠选择 | 保留原值、禁用子控件 |
| `Ia` 重置清空 `ignoredPid` 和 `ownedPid`，不清空 `ignoredList` | 分别保存忽略产品、已拥有产品、忽略分类；重置只清前两者 |
| `Un` 在查看教程后重新允许重置 | 本地追踪介绍及Gamer Room教程联动；自动保存仅写教程标记，不提交其他设置或设备草稿 |
| 教程重置与迁移使用 `.setting-block .thx-btn.test` | [settings_button.rs](../../src/shell/settings_button.rs) 保留 100px 最小宽、27px 高、两侧10px内边距、12px大写文字及 `#707070` 背景；hover/按下改变整个按钮的透明度为0.8/0.6，禁用为0.3，过渡为300ms CSS ease，禁用时不响应激活 |
| `Fs` 的标志、版权、政策链接及社交行 | Synapse标志按原SVG自然尺寸294.366×70显示，120px是上限；Insider为270×50字标，另7个图标各28×28、间距24px。政策链接使用行内分隔符，Privacy另起一行，恢复20/30/10px段间距 |
| `Fs` 真正渲染inline SVG，并共享Facebook/YouTube样式 | 从720内联形状和实际CSS级联生成8组常态/hover；7个圆图标常态为灰色，hover为绿色1.5px描边；Insider只增加原绿色边框。独立media文件中不同的绿色样式不作为最终呈现依据 |
| `Te` 的发行说明入口嵌在 `RELEASE_PATCH_NOTE_WHATS_NEW` 原文中 | 保留可点击的Release Notes片段及前后译文，向Shell发出 `ReleaseNotes` 事件 |
| `ia` 的WDL与OS版本/切换状态有关 | [settings_lighting.rs](../../src/shell/settings_lighting.rs) 区分未知、Chroma、WDL及切换中；Windows 11 build至少22631，或22621且revision至少2506时显示入口。未知控制权时禁用切换；本地工作区另提供明确标注的状态预览 |

“保存设置到本机”只写设置和已保存的设备/快捷键快照。窗口顶部“保存”仍保存整个工作区。保存中禁止重复提交和丢弃；较早的完成事件不清除较新的修改。教程关闭/重置自动保存其标记，连续操作会在当前写入完成后补写最新标记。

迁移按钮在原版打开 `/profile-migration/` 的独立应用，当前已接到 [profile_migration.rs](../../src/shell/profile_migration.rs) 的迁移视图。初始状态为“扫描状态未读取”；明确标注的11种预览覆盖配置记录、空备份、扫描、准备/执行迁移、成功及游戏/宏警告、部分失败列表。分组选择使用GUID和账号/游客身份，准备阶段允许取消，执行阶段禁用取消。原生扫描与导入队列仍未接入，预览数据不写入设备、配置文件或本地存储。

原版发行说明要求引擎至少4.0.633，经宿主广播打开并接收内容。当前Settings事件已由Shell挂载 [release_notes.rs](../../src/shell/release_notes.rs)，布局依据独立 [release-patch-note应用](../../.ref/release-patch-note/static/js/main.b0db08bb.js) 及 [CSS](../../.ref/release-patch-note/static/css/main.008991af.css)：36px标题栏、47px底栏、最大850px宽及按hr/h2分节的正文。默认显示“发布说明尚未读取”，提供原规则生成的官方发布记录链接和明确标注的章节预览；预览只用于查看列表、长文本与滚动，不表示已获取真实发行记录。Escape和关闭按钮返回打开前的焦点。

关于区显示本项目版本，原版权年份标注为2026年源码快照；服务连接的版本查询不冒充原版前端版本。推荐数据源、开机启动、原版桌面通知、灯光控制权切换及迁移均不能据本地偏好宣称已应用。

“服务连接”的本地工作区还提供 [模块状态预览](../../src/shell/module_preview.rs)，包含下载、保存、安装、移除及固件连接限制等17种样例。样例按钮只改变预览状态；实际模块目录仍显示安装状态未读取，安装按钮禁用。

回归源码见 [settings_tests.rs](../../src/shell/settings_tests.rs)，覆盖产品重置范围、教程与草稿隔离、延迟保存及禁用状态。本轮仅编译检查，不运行测试、应用或服务。

# Settings：独立设置应用

更新日期：2026-10-03。当前来源为 [.ref/applications/synapse/settings](../../.ref/applications/synapse/settings/)。本轮重新读取当前 `720` JS/CSS；入口、清单、main、720 JS/CSS 共六份文件经独立 HTTPS GET 与本地逐字节一致，见 [下载核对记录](../re/settings-current-source-check.json) 和 [本轮源码审计](../re/settings-current-audit.md)。没有执行原版脚本。

`ho` 渲染 Synapse：左列自动启动、通知、推荐；右列教程重置、配置迁移、设备灯光。`uo` 渲染 General：左列语言与发行说明，右列关于。原导航只有 SYNAPSE/GENERAL；“服务连接”是用户保留的本地连接及预览入口。

| 本轮核实的原版细节 | 当前实现 |
| --- | --- |
| `bn`/`Ia`/`Te` 在控件变化时直接更新存储 | [settings_page.rs](../../src/shell/settings_page.rs) 发出 `Changed`，Shell 串行自动保存本地偏好；移除无源码依据的底部保存/丢弃栏。较早的保存完成不清除较新的修改；设备草稿不随设置提交 |
| `Ks` 的最小化选项依赖自动启动 | 保留30px缩进、37px竖线、13px横线。启动偏好尚未读取时禁用，不把源码初始化值当作真实系统状态；说明集中在服务连接页 |
| `Ia` 的设备分类是单列，`gap:10px` | 修正原先误做的两列布局；分组标题大写，恢复5/6/10/20px段落间距；重置按钮恢复灰底、27px高及27px水平内边距 |
| 关闭推荐总开关保留子项；RESET 只清 `ignoredPid`/`ownedPid` | 本地偏好保留忽略分类；关闭总开关时禁用子项；重置不改变分类选择 |
| `Un` 重置后只改变按钮禁用状态 | 移除额外“下次进入…显示本地教程”的自写说明。当前联动已实现的设备介绍、Dashboard 与 Gamer Room 教程；完整原版教程集合仍未全部接入 |
| `Ta` 激活或打开 `syn3-profile-migration` 页签 | 正式入口发出 `ProfileMigration` 事件交给 Shell；状态预览按钮只放在服务连接页 |
| `Te` 的语言宽188px，发行说明链接在原句中 | 保持十种语言、原译文与可点击的 Release Notes 片段；删除无源码依据的额外支持链接 |
| `Fs` 显示 Dashboard 前端清单版本 | 关于恢复 `VERSION` 原译文；按当前清单及原公式显示 `4.0.86.2609221012`。这是目标前端版本，不是本项目或已安装宿主版本 |
| `Fs` 使用 `new Date().getFullYear()` | [system.rs](../../src/backend/system.rs) 在 Windows 用只读 `GetLocalTime` 得到当地年份，保留原版权与商标译文；非 Windows 无本地年月实现时留空，不冒充 UTC 年份 |
| `ia` 仅在指定 Windows build/revision 上出现，`tips` 是右上角帮助提示 | 保留系统门槛、切换中禁用和 Chroma/WDL 内容分支；恢复14px帮助图标、距卡片右/上10px及原提示文案。正式页面不再把帮助文案和自写服务说明插在内容区 |

关于区保留原政策URL、Synapse标志、Insider字标与七个社交入口。社交资源仍由已有静态提取结果提供；本轮核对了 `Fs` 实际引用、链接与相关布局规则，没有以素材目录中另一个SVG替换其内联渲染形状。

本地工作区保留设备、模块、应用选择器、Alexa、教程、顶部状态、灯光及迁移状态预览。样例不表示原生接口已工作，也不写入真实设备。

尚未完成的设置服务行为有明确源码边界：自动启动读写、桌面通知写入宿主、推荐存储跨原版窗口同步、完整教程重置集合、WDL控制权读写/切换广播、发行说明的引擎版本门槛及加载回执、迁移原生扫描导入。详见审计文档中的逐项调用依据。本轮不把本地偏好保存或预览页面算作这些服务已接通。

验证限于格式化、静态源码/资源核对与统一的 `cargo check --locked --all-targets`；不运行测试、应用、安装器、下载脚本或 DLL。已有 [settings_tests.rs](../../src/shell/settings_tests.rs) 随编译检查，运行结果不作本轮证据。

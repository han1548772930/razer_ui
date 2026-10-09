# 515 Snap Tap：本地配对编辑

2026-10-06。本批仅覆盖当前 515 普通 Customize 的 `$l → Jl → ql`，不将共享组件存在视为其他产品或 displayMode 已完成。331 产品 / 1419 主页面仍为 partial，完整产品数仍为 0。

## 当前来源

- `.ref/devices/515/static/js/main.f60ca5aa.js`，SHA-256 `4a9db2072b3d64035e36d468fcc006b1930ebbf8c0bc620e4869a2e970c432e8`。
- `tools/prepare-snap-tap.cjs` 仅以 Acorn 解析 AST，以有限 literal/parseInt 文法提取表格；不加载或执行厂商 JavaScript。输出 `snap-tap-current-evidence.json` 的 34 项 AST 收据、113 条 CSS 规则、2 段关键帧和 3 个 SVG 的来源/哈希。
- 输入来自模块 46114 的 Rk 表；显示名称来自 90857 的布局表和函数；文字键来自 54693。`wc` 为当前代码提供的专用键回退表，优先使用产品 DKM_KEYS。
- 初态使用 `ha`：关闭，第一组 A/D、id=1、mode=LAST_INPUT；不使用 DEFAULTPROFILE.snapTap 的 true 冒充 reducer 初值。当前父根不传 supportsShortcut，因此没有 FN+L SHIFT 区。

## 本地状态与界面

实现位于 `crates/razer-pages/src/features/keyboard_snap_tap.rs`，由 KeyboardProductWorkspace 持有状态，放在 Gaming Mode 下方。框架 Base Button、SynapseSwitch、Dialog、Tooltip/Positioner 和 Animation 提供行为；应用提供当前源码尺寸与颜色。

| 操作 | 本地行为 |
| --- | --- |
| 开启/关闭 | 修改本地开关；只在实际观察到 adjustmentModeRunning=true 时显示无按钮的退出调整模式提示。收到 false 后关闭提示并恢复焦点 |
| 新增 | 最多四组；仅 READY 可新增；追加空组到临时列表并开始 KEY1，新增组不伪造 mode 字段 |
| 编辑 | KEY1 合法后暂存并切到 KEY2；KEY2 合法后提交本地列表，显示成功消息，三秒后回介绍 |
| 校验 | 跨所有配对检查重复；禁止 KEY_APPLICATION、KEY_LEFT_GUI、KEY_FN、DKM_F6、DKM_D2。当前 515 是 KEYBOARD，不误套 KEYPAD 的方向键限制 |
| 删除 | 第一组不显示删除入口；其他组删除并重新编号。按源回调的旧 warning/旧列表值判断回滚，不提前清 warning 后再决定 |
| 外部点击 | pair-list 和 Add 区以实际 prepaint bounds 排除；含空槽时显示重复警告，已有警告不提交，其余完整列表提交并结束录入 |
| 窗口失焦 | 去掉不完整组；warning 时还排除正在编辑的组；唯一组会被全部去掉时恢复 reducer 列表。READY 时也清除成功提示 |
| 页面/配置恢复 | 内部 TAB 切换结束录入并按 beforeunload 规则投影；恢复配置重建暂存状态、丢弃旧计时器/提示，布局与调整模式观察另行保留 |

按键框外尺寸至少 64×44（源 content-box 60×40 加 2px 边框），圆角 4px；文字 11px，录入内部 38×25；绿色/橙色一秒线性闪烁。行距/列距 10px，删除 20×20 加左距 20px；Add 为 64×44、圆角 5px。删除两张原 SVG 在同一位置切换。介绍灰色、警告橙色、成功 lime。禁用区 opacity=.3。

Add tooltip 按鼠标位置加 10/20 显示，采用当前 14px/17px、8/10 内边距、原底色/边框；最终越界处理交给 Base Positioner。调整模式提示使用当前通用 `.warning-alert` 的 400px 宽度、原图标、原标题/正文和无按钮结构。

## 保存与观察的边界

- 本地修改使用独立 `_snapTapLocalV1` 字段，存入活动 profile.source_settings；旧的 profile.snapTap 种子保留，不把它当作新 UI 的用户修改。初始化/纯观察不创建这个标记。
- snapshot 使用源 beforeunload 投影：仅 KEY2 阶段把暂存 key1 放回 reducer 列表，随后过滤不完整配对。每次编辑通过 KeyboardProductChanged → SourceProductWorkspace.capture 进入现有本机 Save/Discard 流程，不调用 DLL 或设备持久化 API。
- 配置、布局、调整模式和 inputredirect 的观察边界已贯通 ProductWorkspace → SourceProductWorkspace → KeyboardProductWorkspace。**没有真实发布者连接，不是设备读取完成。** 显式本地草稿优先保留，观察配置不悄悄覆盖它。
- 原始输入仅接受 keyboard/analogKey/razerKey，匹配原 scancode 与 outputFlag/flag，只处理奇数释放标记；保留 Pause 后一次 NumLock 抑制。这个接入口不启用输入重定向，不调用 disableMapping、registerNoBrowserInputHandler 或 DLL。
- AppShell 根捕获当前可见设备的 keyup，避免无效外部点击改变焦点后丢失录入；会抑制录入期间的本机快捷动作。隐藏产品不吞按键，父子不会重复录入。
- 2026-10-06 补充：Help role 的提前返回现先调用 leave_snap_tap，窗口根的 captures_snap_keys / capture_snap_key_up 同时排除 Help，避免底层 Gaming Mode 保留的页面身份让隐藏编辑器继续捕获释放事件。该补丁通过静态编译，未运行窗口验证。

## 未完成与验证限制

- GPUI Windows 将左右修饰键合并，并丢失部分位置/原始扫描码信息。当前原生路径只接受明确映射的字母、功能键、空格/退格/Tab/Esc、Menu 和导航键；数字、标点、Enter、修饰键、小键盘和专用键等精确录入仍待真实原始输入接入。无法确定的按键不会被猜成另一个键。
- 原生根捕获不等于源系统级映射/输入重定向服务。Meta 系统快捷键、不同布局、全部边缘交互和运行焦点仍未验收。
- 帮助 tooltip 复用已有 widget 行为；Add tooltip 的原始越界行为及完整父区布局仍需逐项核验。没有运行截图或像素对比。
- 宿主切换到 Dashboard 后再回来会保留该实体暂存状态；只核实了内部 TAB/配置恢复清理及隐藏时不吞键，尚未证明当前官方宿主切标签是否卸载 ql，不能据此声称宿主切换生命周期已完全对齐。
- `cargo check --locked --all-targets`、格式化、嵌入 JSON schema 及源码/资源静态校验通过。独立子任务重新核对全部 AST/CSS/SVG，并回读 KEY1/KEY2、删除、快照和根捕获链。未运行应用、测试、构建、安装器、下载的 JavaScript 或 DLL。

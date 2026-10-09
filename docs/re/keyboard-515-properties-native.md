# 515 Keyboard Properties：右列系统入口

2026-10-06。实现 `crates/razer-pages/src/features/keyboard_properties.rs`，当前 515 普通 Customize 右列 `$l → mA/MA → pA/LA`。本批不覆盖 analog Game Controller 分支、其他产品或 displayMode。

当前主包 SHA-256 为 `4a9db2072b3d64035e36d468fcc006b1930ebbf8c0bc620e4869a2e970c432e8`。`tools/prepare-keyboard-properties.cjs` 静态生成 `keyboard-properties-current-evidence.json`：12 项 AST（包含两个系统包装器）、66 条 CSS、2 个当前 manifest SVG。没有执行厂商脚本。独立审计的完整宿主链和参数见 [独立报告的 L 续记](ui-readonly-first-roadmap.md)。

## 已接入

- 独立右列面板，KEYBOARD_PROPERTIES_HEADER 标题、KEYBOARD_PROPERTIES_TOOLTIP 帮助、OPEN_KEYBOARD_PROPERTIES 操作；没有主开关、固件门控、快捷键区域或额外 ExternalLink 图标。
- 原 Windows 图标 44×44，图标右距 20px；操作文字 14px、行高 44px、下划线、hover 绿色、active opacity=.7。Base Button 提供焦点与键盘语义。
- 实体创建时复用既有 `backend::system::is_windows_11()` 只读版本判断，保留图标选择，不在 render 重复查询。读取失败采用 legacy 图标，不展示虚构的查询成功状态。该本机查询没有在本次开发验证中执行。
- 用户点击复用 `backend::system::open(Properties::Keyboard)` 与失败通知。现有本机实现启动 `control.exe main.cpl ,@1`；这个命令行来自本地已有后端，不能声称是静态还原出的 DLL 内部实现。
- 操作不修改 profile/source_settings，不 emit KeyboardProductChanged，不显示保存或设备成功；不调用厂商 DLL。

## 来源与仍待核验

当前厂商包装器发送零参数 OpenKeyboardProperties。当前 4.0.827 宿主将它转给 FFISysUtils；sysUtilsNative 声明是 `["void",[]]`。版本读取独立走 getWindowVersion/common.js；产品 LA 初始占位 "11" 不等于真实系统读取结果。

本批从当前 515 manifest 精确 URL 准备了两种图标，SHA 与独立子任务的 HTTP 200 字节比较一致：Win11 `aa3676b09d4555dbcfdcb81e8d2413fb0b958aebf49207fae94e1108d8596bba`，legacy `d3a5cbb29c8224935f19a9bf726d04e898f2901ec693d27bc8c72785da05a0c3`。资源已嵌入并纳入校验。

静态回读确认右列、文案、44/20 几何、命令/失败路径和不写草稿；`cargo check --locked --all-targets`、格式化与资源校验通过。真实系统窗口打开、像素/焦点、帮助 portal 的所有边缘定位、所有布局仍未运行验收。515 产品继续为 partial。

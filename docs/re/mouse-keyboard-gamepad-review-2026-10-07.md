# 鼠标、键盘与手柄分组复查（2026-10-07）

本批回到每款当前 `.ref/devices/<pid>/` 实际文件，核对 SHA-256、导航 AST、组件 UTF-16 切片及挂载链。只执行维护的静态解析工具，不执行厂商 JS、DLL、应用或测试。这里的根组件记录用于明确后续逐控件工作范围，不能作为页面完成证明；**完整页面和完整产品验收均新增 0**。

## 已修复的控件

- 鼠标 226 的 Polling Rate 已按当前挂载子树补齐连接分支、独立有线/无线本地字段、按钮/说明/帮助/高频警示、源 DualLink/Dock 限制和带 owner/profile/connection 代际的观察接口。Mouse Properties 面板也已接入当前资源与既有系统入口。真实发现代码现在发布连接类型；频率和拓扑发布者仍缺，未做硬件运行验证。详见 [226 复查](mouse-226-review-2026-10-07.md)。
- 52 款键盘各自的 Gaming Mode 原版子树均把 Windows 行的 checked/active 绑定到 `gamingMode.isWindowsKeyDisabled`，不能用 Gaming Mode 是否开启替代。原生现已改为读取该字段；Windows/Menu/Copilot 三类只读行沿用源禁用状态，不添加写设备动作。
- Menu 原先只显示于 515；逐产品核实挂载路径与实际 `KEY_APPLICATION` 后，现有 45 款满足显示条件，新增修复 44 款。不是把所有键盘默认显示 Menu。
- Copilot 原先只显示于 716；当前源码门控和实际 `DKM_D2`/`DKM_F6` 同时满足的只有 716、717、724，新增修复 717、724。717 上游另有 `isSystem` 条件，因此不将其通用 Menu 分支直接放行；该产品本身也没有 Menu 键。

逐产品键盘规则存于 `src/features/keyboard_gaming_review_data.json`，由静态工具生成，运行时仍检查真实按键定义。52 条 Windows 来源核对，不等于 52 款都显示 Menu/Copilot，更不等于 Customize 整页完成。

## 分组矩阵和明确待办

| 范围 | 本批核对 | 后续状态 |
| --- | --- | --- |
| 鼠标 Performance/Polling | 75 页分别核实当前 Polling 挂载链；226 局部修复，其他 74 页确认仍有源帮助/说明等差异 | 每款继续核对条件、CONFIG、连接/限频/本地字段；不同代际不能直接复制 226。162 本身没有连接标题或 Learn More，不将共享代码中的分支都认定为可达 |
| 键盘 Gaming Mode | 52 个当前子树；Windows 属性、45 款 Menu 和 3 款 Copilot 实际资格 | 各自完整开关、帮助、快捷键和额外控件仍待逐项复核 |
| 键盘 ACTUATION | 15 页的当前根及 Rapid Trigger 编辑器均已独立核实 | 原生目前只有首批触发点编辑；快速触发编辑、源 Snap Tap 控件、垂直布局、混合选择值和整体重置确认仍缺 |
| 键盘 OLED 691 | 从当前懒加载模块 42553 重新解析 default export `na`，验证实际导航和根切片 | 已有本地卡片、导入/裁剪/编辑器和 Apply；本批仅校验根，不能把早期文档中的旧缺项当成现状。仍需逐编辑器复核、真实下载/设备状态及完整交互验收 |
| 键盘校准 740/746 | 各自当前导航与根组件核实 | 已有 `keyboard_calibration.rs` 页面和弹窗；本批未重验其全部步骤、关闭/切配置生命周期、出厂配置限制和按键状态 |
| 手柄 Triggers | 7 款当前双柄组件重新核实；沿用已有指针预览/释放提交静态审计 | 产品特有模式/其余控件仍待复核；不能把一条 range 的修复视为整页完成 |
| 手柄校准 | 8 页、10 个实际向导/弹窗子树核实 | 原生仍只有说明、两个禁用按钮和服务不可用提示；完整 UI 确认缺失，不因 DLL 写回后置而删去 UI 工作 |
| 手柄其他页 | 9 款共 39 个已有原生页面逐一核对当前导航与根 | Customize、Thumbsticks、Lighting、Power 均保留逐控件待办，根核对不增加 UI 完成数 |

手柄校准必须保留两种来源形态：2629、2636、2647、2650、4133、4144 使用警示/确认加多步骤摇杆向导，包含步骤标记、产品图、摇杆模拟器及各产品方向/错误/取消表现；2676、2684 使用按侧选择的摇杆弹窗，另有独立扳机校准弹窗、计时器和半圆仪表。未取得真实状态前不得显示伪造的设备采样或完成结果。2650 没有 Triggers 页；4115 是 Kitsune 街机控制器，仅按其 Customize/Lighting 复查，不套用普通手柄校准页面。

完整范围没有缩减到本批细查子树：当前原生清单还有鼠标 Customize、Lighting、Calibration、Power、Scrolling、Advanced 和 HELP；键盘 Customize 其余区域、Lighting、Power、Pairing 和 HELP；以及上表手柄其他控件。它们均在总复查队列中保持 pending/partial。本批并未复核其全部内容。226 的 Sensitivity Matcher、DPI 框外 window wheel 等既有待办也继续保留。

UI 增删改、Apply/Save 和本地草稿仍在范围内；设备状态只读接入也仍在范围内。只有通过 DLL 修改设备/服务及持久化的腿后置。本地保存和纯观察必须分开，不能产生伪造的设备成功。

## 可复查产物

- [75 页鼠标 Polling 矩阵](mouse-polling-pages-review-2026-10-07.json)：每页实际切片、挂载链、源分支标记及剩余差异。
- [52 款键盘 Gaming 矩阵](keyboard-gaming-review-2026-10-07.json)：逐产品 Windows/Menu/Copilot 来源和实际按键资格。
- [键盘特殊页与 9 款手柄矩阵](keyboard-gamepad-groups-review-2026-10-07.json)：18 个键盘特殊页、39 个手柄页面；明确区分根核对、子树核对与 UI 缺口。

维护的 `review-mouse-polling-pages.cjs`、`review-keyboard-gaming.cjs`、`review-keyboard-gamepad-groups.cjs` 均已运行 `--check` 通过，重新读取当前源后比较矩阵。`audit-gamepad-trigger-lifecycle.py --check` 同时通过：7 款、21 个当前 AST 切片、77 条 CSS 收据及逐款 reset/mode helper。`git diff --check` 通过。最终 `cargo check --locked --all-targets` 由主任务统一执行；不将矩阵生成或静态编译视为视觉、窗口输入或硬件运行验收。

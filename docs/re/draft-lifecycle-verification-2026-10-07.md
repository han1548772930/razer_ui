# 已写 UI 的本地草稿与页面生命周期复核（2026-10-07）

本轮按 `ui-readonly-first-roadmap.md` 与前次独立报告末尾 N/O 续核。实际修复范围是鼠标页面隐藏后的输入生命周期；其余家族仅做下面明确列出的静态抽查。没有运行应用、构建、测试、供应商 JavaScript、安装器或 DLL；不能把本报告当成全部界面或实际设备读写验收。

## 已修复：宿主页切走后仍存活的鼠标输入

修复前，`AppShell::navigate_now` 离开设备只关闭 profile 对话框，保留产品实体及内部 `page`。`mouse_dpi_number::start_dpi_number_repeat` 的 300ms task 只检查窗口 active 和内部 Performance 页；宿主切到 Dashboard 后两者仍可为真。它没有显式的宿主离开取消链。是否还能收到旧元素的 mouseup/hover-out 取决于窗口事件；代码不应依赖这些已隐藏控件的事件来清理存活 task。此为静态生命周期缺口，未运行复现。

同时，普通 `add_range` 数字 Blur/Enter 只核 OTFS，没有重核当前行/XY/页面；70 的 slider Change 也没有对应写入口保护。226 grid 和数字 step 已有部分保护，但 Help 早退仍保留 Mouse 内部 Performance 字段，单凭该字段无法判断可见性。旧回调可越过只在呈现处设置的 disabled；这不表示已有用户文件发生损坏。

现在的链路：

1. Shell 离开/进入设备时调用 `ProductWorkspace::set_active`，转发 `SourceProductWorkspace::set_active`。Source 只把非 Help 的当前鼠标内容设为 active；内部页选择也重新计算这一条件。其他产品家族未被新增的鼠标清理逻辑改变。
2. Mouse 失活取消 numeric repeat、注册/typed 状态、grid pending/pressed、scroll level preview 与 tooltip，回填已提交的本地 DPI。它不提交当前预览，不发 Changed，不清本地配置或观察。返回原页恢复可编辑状态。
3. `dpi_number_editable` 同时核可见产品内容、Performance、OTFS、阶段拖排、实际槽位和 independent Y。Blur/Enter 与 step 都走它；无效 Enter 在调用 `window.blur` 前即返回，避免旧隐藏输入抢走新页面的焦点。隐藏 stage 的数字/XY 本来允许编辑，仍予以保留；阶段总开关关闭时仅实际 active 槽位可编辑。
4. 70 的 slider Change 在上述条件外还要求该行 visible；不满足则只回填控件值，不改草稿。226 grid 保留已有 preview/commit 边界，并通过新的内容 active 条件拒绝隐藏页回调。此入口门控限定为这批已核 70/226 DPI，普通 lighting/calibration 等范围控件保留原行为。
5. DPI 清理和 restore 都使阶段拖动代际失效并清源行标记；旧 drag start/drop/release 不再改变新代际的行状态。ScrollWheelEditor 保留既有预览/Release 语义；宿主离开设备与 Help 通过 Mouse 清除 preview，未 release 的旧预览不会在随后的 Release 被提交。

涉及 `shell.rs` 的改动为 `navigate_now` 和构造器尾部的最终激活同步。实际实现位于 `product_workspace.rs`、`source_workspace.rs`、`mouse_products.rs`、`mouse_dpi_number.rs`、`mouse_dpi_rows.rs`。没有改变 DPI 数值算法、拖排结果、scroll 六字段、local 标记 schema 或 DLL API。

### 激活入口补查

主线程回读指出 `--tab` 在构造器直接赋 `Location::Device`，不经过 `navigate_now`。已在所有启动参数处理完毕后，按最终 Location 同步设备 active；这样 `--tab Performance` 的当前鼠标可编辑，`--preview-product` 先导航而后被 `--tab Home` 覆盖的设备也会正确失活。

直接构造 `SourceProductWorkspace` 的非测试入口还包括 Aether 784 预览与键盘校准预览，本轮 active 转发只影响 Mouse，不会锁住它们。独立 Chroma popup 则共享 Mouse `lighting_page_element`，Armory 共享 `customize_element`；它们不以主宿主的 Device Location 判断自身可见性。因此已将初稿的普通范围 active 检查和 Scroll mounted 检查收窄/撤回，避免引入这两类入口的禁用回归。鼠标宿主页离开时取消 scroll preview 仍保留。Armory 独立根自己的卸载、晚到事件和非 DPI 范围控件的跨窗口生命周期未在本轮验收，不能由宿主 active 标记推定已完成。

## 当前源重新比对

以下维护工具已用 `--check` 运行。它们读取源文件为文本，经 Acorn/CSS 静态解析、受限字面量提取及字节比较；不导入或执行供应商模块。

| 工具 | 本次结果 |
| --- | --- |
| `audit-mouse-70-dpi.cjs` | 11 项 AST、158 条 CSS、11 份资源一致 |
| `prepare-mouse-226-dpi.cjs` | 26 项 AST、154 条 CSS、9 份资源一致 |
| `prepare-mouse-226-scroll.cjs` | 52 项 AST、83 条 CSS、3 个模式一致 |

源指纹（完整路径相对仓库）：

| 当前源 | SHA-256 |
| --- | --- |
| `.ref/devices/70/static/js/main.8f24b6a1.js` | `be0816be63fe674ea9fc106cbd55f5aabd509e1ff36aaf66e6c002d9ecd1f4b5` |
| `.ref/devices/226/static/js/8355.3d5e573e.chunk.js` | `b5d160fe0c231186d7f8271d13ece17a3824b0c35c4962cf2ecec72c46352eb8` |
| `.ref/devices/226/static/js/main.08f95762.js` | `ccbd5af37e23d1c64faf62551d15b0ef91d6e89fc06cafab3fb9464c598f884e` |
| `.ref/devices/226/static/js/3485.794fe922.chunk.js` | `42871b0390959f81d70f761f0429137a8311097d4551c5f7b5d3124bd45e6ebd` |
| `.ref/devices/226/static/js/2306.8464ad14.chunk.js` | `c4b6f4ed7d4bdef9bd9e27ef34712290dd24c1ad2723a08aaafba334d5642200` |
| `.ref/devices/226/static/js/1102.dc9e537e.chunk.js` | `d41f3163bca18c0ee1f8bd2cd4233ff9d71c9e7adf505a2e971b5f5a6ca0897d` |

70 的 module 4230 当前 UTF-16 `[185084,190952)` 中 `componentWillUnmount(){clearInterval(this.intervalId)}` 已重新读回；226 当前 Ft 在 unmount 移除 window mouseup，es 同样移除全局 down/up 监听。这些是局部控件生命周期证据。宿主本地 retained entity 的 active 转发是为隔离隐藏输入所作的本地实现，不据此宣称已经证明官方宿主每一种标签切换必然卸载 iframe。

本次 `rustfmt` 和 `git diff --check` 通过；只有仓库既有 LF/CRLF 提示。cargo 由主线程统一执行，本子任务未运行。本轮没有添加镜像实现的行为测试，也没有实际文件往返、窗口输入或像素验证。

另静态读取锁定的 `gpui-base 0.7.1`：`input/base/state.rs::set_value` 临时关闭 emit_events，`slider.rs::set_value` 只更新值并 notify。上述回填不会再发 InputEvent::Change/SliderEvent::Change，因此不会把拒绝后的回填重新当作用户修改或形成事件循环。

## 保存、restore 和观察的抽查结论

- Mouse snapshot 仍只 clone draft，scroll 的未编辑观察只供 value/display 使用；六字段 local 集合独立保存。新建/默认补齐使用空集合，已有无标记旧草稿按旧本地值恢复。失活不把观察升为 local，也不把未 release 的预览写入 profile。
- Source capture 仍把 device_fields 分出到 `source_device_settings`，其余归活动 profile；restore 会把默认补齐快照回写本地 profile，因此不能仅用字段存在判断它是显式用户覆盖。轮询 O 的新 local 字段处理仍待实现，不因这轮生命周期修复而计为完成。
- Shell 保存仍捕获点击时的 WorkspaceFile，并在异步成功后用所捕获 Device 快照 `mark_saved`；保存期间的新修改保持 dirty。既有状态文字为“已保存到本机 · 尚未发送到设备”，不表示 DLL 写回。
- Snap Tap restore 重建编辑状态，保留布局/调整模式观察及显式 local 配置；这里只回读局部方法，没有再次核完原生按键身份、publisher 或宿主离开后的 Snap Tap 行为。新增 active 链当前只作用于鼠标。
- 1382 restore 会销毁旧 editor/subscription 并恢复经验证的可选 audioGroup payload；已有 Save/Cancel/导航保护保留。3334/3337 `mixer_snapshot` 在没有显式 local_selection 时移除 playbackMixDevice，观察列表/活动混音标志不直接进快照。这里只检查对应方法，没有重新验收完整 Audio 家族。
- 2636 restore 清当前 warning/dialog，但 `previous_deadzones` 仅在进入 THUMBSTICKS 时更新。profile 切换时是否应重建该缓存需要继续查当前源 profile 更新生命周期，不能只按本地直觉认定差异；已转交产品核验，不在本补丁中改动。
- Scroll restore 清观察，但观察入口仍没有 profile/generation token；迟到旧 profile 观察的隔离尚待真实 publisher 接线时处理。当前未接 publisher，本轮没有虚构设备观察或声称 DLL 查询成功。

## 已写 feature 家族覆盖账目

“抽查”仅指本轮实际读取所列方法；不代表该家族所有产品、页面、条件或弹窗已验证。Source 的八种实际 body 均列出，另列旧适配器及独立应用，以免 routes/descriptor 数量代替 UI 验收。

| 家族/范围 | 本轮实际覆盖 | 本轮没有覆盖 |
| --- | --- | --- |
| Mouse | 70/226 DPI 数字、slider/grid、阶段、scroll、本地保存与失活链；当前源收据重比 | 其他鼠标产品、226 polling O、完整视觉/输入、真实观察生产者 |
| Keyboard | snapshot/restore、515 Snap Tap local/瞬态重建方法回读 | 其他产品、OLED/actuation/calibration、完整按键与宿主导航 |
| Gamepad | restore/set_page、2636 warning 与 previous_deadzones 路径回读 | 其他型号、整页源/像素、候选缓存跨 profile 规则 |
| SourceControls（含 supplement） | snapshot/restore/staged 清理、Source device/profile 分流 | Receiver/Camera/OLED 每个具体界面和数据源，本轮由其他任务接续 |
| Audio | snapshot/restore、1382 editor 生命周期、3334/3337 selection 标记 | OLED/Nommo/equalizer 所有条件、完整映射和所有 Apply/Save 根 |
| System | snapshot/restore 的模式/映射字段校正回读 | Blade 各页、风扇曲线、所有硬件模式 |
| AccessorySystem | snapshot 去除 uiRestraint、restore 限幅/枚举及清理抽查 | Monitor/Core X 等完整 UI 与动态状态 |
| Hue | 白名单 restore、brightness/effect snapshot、瞬态 alert 清理回读 | 桥接器/灯区/弹窗所有操作与真实服务 |
| 旧 DeviceWorkspace 适配器 | ProductWorkspace owner/转发边界 | 十个旧适配器每个编辑/保存操作，本轮未重新逐项核源 |
| 独立配件 | Source 的 accessory snapshot/restore 归属 | Aether/有线与无线 ARGB/Automation/Dock Pairing 内部所有编辑 |
| Shell/公共页面 | navigate_now、save snapshot/mark_saved 抽查 | Settings 产品入口、Receiver、Dashboard、全部弹窗；分别由其他任务处理或保持待办 |
| 独立应用/功能 | 仅识别存在的 Chroma/Studio/Macro/Profiles/Armory/Shortcuts 等所有者 | 本轮未逐项审这些应用 UI；Studio 尚有 roadmap 记录的未接属性根 |

本报告不改变 roadmap 的“331 产品 / 1419 主页面 partial、完整产品 0”口径，也不把上述局部方法覆盖宣称为“全部已写界面都验证完成”。后续需按这张账目逐个界面和条件继续，真实只读 DLL 能力与 UI 编辑/本地保存仍在范围内；DLL 写回继续后置。

# Raptor 3858 / 3880：六页当前源码复查

本批逐项复查两款 Raptor 的 Gaming、Color、Display 页面，修复明确缺项，六页均保持 **partial**。没有把组件目录、路由或收据数量当作 UI 完成证据；完整产品与完整页面验收数仍为 0。

当前原文与实际挂载路径见 [静态证据](monitor-pages-review-current-evidence.json)，由 `tools/review-monitor-pages.cjs` 从当前产品 manifest、Acorn AST 边界和已提取页面组件重建。3858 使用 `main.608e1599.js`（SHA-256 `68e3d86fe0798574914af256df203c7eca30ef5b4f3b2950c364c42cb9d2c02e`）；3880 使用 `main.0ff487d9.js`。完整 SHA、偏移、每个组件原文及 CSS 均写入证据。共 124 条挂载关系覆盖六页，包括功能组件、包装组件和共享基础组件；这一数量只证明当前源定位。

| 产品 / 页面 | 本次实际核对的主体与行为 | 本地结果与仍缺事项 |
| --- | --- | --- |
| 3858 / Gaming | `NTA → dTA → RTA`；六预设顺序 Default/FPS/MMO/Racing/Streaming/Custom；编辑现有预设后切 Custom；`STA` 两个 0–100 数字框、Overdrive 0–2、Gamma 0–2；此产品不挂载 gamut | 主控件和本地字段已存在，未重复实现。仍需修复源 `widgetContent.featureDisabled` 的完整父容器透明度层级；当前逐行禁用不能视为该层级已复现。预设/设置的真实观察和当前 MW 约束更新仍未发布。 |
| 3880 / Gaming | `FTA → BTA → vTA`；明确 `showGamut:true,useExtendedGammaSlider:true`；Gamma 多出 2.4；Native/Rec709/DCI-P3，非 Native 限制 Contrast/Gamma | 已有产品差异与本地编辑保留。与 3858 相同，父容器禁用外观仍不完整；`allowed()` 的本地组合限制还须逐项对照当前 reducer，而不能只据共享控件接受。 |
| 3858 / Color | `xAA → zAA → wAA`；NORMAL/LOW_BLUE_LIGHT/WARM/COOL/SRGB/CUSTOM 的 ID 与次序；仅 Custom 显示 RGB 三个 `STA`；`xAA` 的挂载/可见性同步调用 | 预设、RGB 与立即显隐已有实现。真实同步/可见性刷新未接；Color 父容器 `.featureDisabled` 与 RGB 子控件的透明度分工仍未等价。原文 `xAA` 直接调用 action creator，不能未经 middleware 验证就称该调用是成功设备读取。 |
| 3880 / Color | `NSA` 左 THX/Color Profile、右 HDR/Color Temperature；`rSA/ISA` 的开关隐藏条件；`RSA → k6O → z6O` 的 options≤1 禁用、选择回调、selected 路径包含匹配；颜色管理宿主命令 | 修复永久禁用下拉；允许真实提供的多选项本地选择，保留源空列表与无匹配分支；修复文件路径包含匹配；颜色管理链接改键盘可操作的 BaseButton，并随 restriction 禁用及变暗。选择意图与服务观察分离并明确未发送。THX/HDR 的完整状态观察、Color 子组件禁用外观与原 dropdown 的上翻/定位/tooltip 几何仍待验收。 |
| 3858 / Display | `cSA` 的来源/PIP 左列，Adaptive Sync/HDR/FPS 右列；`SSA/OSA` 相同源忽略、确认、外点取消、don't ask again；`eSA/JAA` PIP/PBP 和四角三尺寸；`RSA/NSA/nSA` 开关/文本/FPS | 补回四个原版来源图标与 68px 纵向等宽按钮；Adaptive Sync 关闭时正文按源变暗；修复 FPS 错误使用 refreshRate 限制。仍缺 `cSA` 的 `isGettingMonitorSetting` backdrop/spinner 与真实观察；确认弹层只监听 Display 内容内的外点，尚非 source document 范围，外部 shell 点击需继续核对。 |
| 3880 / Display | `qSA` 左来源/PIP，右 Adaptive Sync/Refresh Rate/FPS；`YSA/bSA/mSA/PSA/KSA/HSA`；`jSA → xSA` 的 rate options effect、先本地选中再发 action、restriction 下隐藏整段选择器和 Windows 链接 | 来源/FPS/Adaptive Sync 同上修复；刷新率胶囊与 Windows 链接改 BaseButton；restriction 时按源隐藏胶囊与链接。删除空观察时凭初始 useState 永久显示 60/120/144/165 的错误回退，空列表明确未读取。真实 options effect 及状态发布未接；PIP/FPS 细部焦点、位置与 Display 左列 stacking 尚未运行验收。 |

本次修复对应 `src/features/accessory_system_products.rs`：

- `color_profile_widget` 不再使用 `disabled(true || ...)`；`subscribe_color_profile` 再次检查真实选项和 restriction。源 `RSA` 的 `selectedColorProfile.includes(profile)` 支持服务返回完整路径；没有匹配且非空列表时不展示误选中的下拉。
- `refresh_rate_widget` 按 `jSA` 的条件只在无限制时挂载胶囊/系统链接，提供原生键盘操作。原版 effect 会用 `supportedRefreshRate` 覆盖其短暂 useState 初值，原生版因此不能把该初值当作已知设备能力。
- `pending_color_profile` / `pending_refresh_rate` 只保留本地未发送意图，不覆盖 `selected_*` 观察，不写入 profile snapshot，不冒充设备成功。下一份完整观察（或 `None`）清除该意图；这里没有新增设备写调用。
- `set_monitor_runtime` 明确接收完整快照；空、缺失或无效字段会清空相应观察、选中状态及旧 restriction，防止断开后残留。同步源 `uiRestraint` 保存在独立 `monitor_restraint`，本地配置恢复/Discard 不会清除当前观察，也不能用旧 saved profile 解除限制；编译用案例覆盖该边界。持久化快照仍剔除历史 `uiRestraint`。**真实发布者尚未接入**，不能计为 DLL 读取完成。主任务负责增加 SourceWorkspace/ProductWorkspace 只读透传；未来实际发布还需连接代次与事件顺序契约，不能把任意部分事件直接当完整快照。
- 原文 `nSA/HSA` 只读取 `refeshRateCounter.isEnabled/position`，不读取 `uiRestraint.refreshRate`；移除 `allowed()` 对 FPS 的错误限制。`RSA/KSA` 的正文在 off 或 restricted 时均变暗，按源补齐 off 状态。
- Windows 显示设置和颜色管理命令的失败现在显示在页面上，成功重试或切页清除旧错误；没有在本批验证中启动这些命令。
- `SSA/YSA` 四按钮的 SVG 从当前 bundle 导入路径下载为静态资源，两产品字节一致；保留 URL、模块/绑定 AST、源 SHA 与输出 SHA。原 CSS 为 `flex:1 0; flex-direction:column; gap:4px; height:68px; padding:15px 0`，图片 `40×20`。此前“资源不可用所以仅文字”的分支已移除。

验证边界：`node tools/review-monitor-pages.cjs --check` 通过（2 产品、6 页、124 挂载收据、4 个独立 SVG）；四 SVG 按 XML 检查，格式化与差异空白检查通过。增加断开清理、观察/意图区分、snapshot 隔离及 FPS 限制的编译用回归案例；未执行测试。Cargo 由主任务统一检查并在总记录汇总。本批未运行应用、构建、安装器、厂商 JavaScript 或 DLL，也未启动 Windows 设置命令进行验证。

这六页不涵盖两产品的 Lighting 和 Help；那四页继续 pending。附件/系统队列从 8 个 partial 子树页更新至 14 个，剩余 311 页仍待独立复查。

# 当前产品配置栏复核

2026-10-03。`isEnableProfileBar` 的名称不能作为隐藏或禁用整个配置栏的依据。对根组件、消费者和 Redux 页切换调用分别追踪后，当前普通产品入口的结果如下。

| 产品 | 配置栏 | 下拉框 | 同步图标 |
| --- | --- | --- | --- |
| 179 HyperPolling Wireless Dongle | 根传入 `renderProfileBar:false`，不挂载 | 不存在 | 不存在 |
| 769 Philips Hue | 根没有 renderProfileBar | 不存在 | 不存在 |
| 164 / 241 鼠标底座 | 保留，包括 HELP | 默认可用 | HELP 使用 unsupported |
| 778 / 3871 有线 ARGB | 保留 | 默认可用；不能因 Customize 禁用 | 仅 TAB_LIGHTING 启用 |
| 784 Aether | 保留，包括 HELP / CUSTOMIZED | CUSTOMIZED 禁用，HELP / LIGHTING 默认可用 | HELP / CUSTOMIZED 使用 unsupported |
| 3884 无线 ARGB | 保留 | 根 setProfileDropdownState 只允许 TAB_LIGHTING | 仅 TAB_LIGHTING 启用 |
| 3886 无线 ARGB | 保留 | 初始 enableSwitchProfile=true，根无页切换禁用调用 | 仅 TAB_LIGHTING 启用 |
| 3946 Base Station V3 | 保留 | 默认可用 | HELP 禁用；导航更新还排除 TAB_CALIBRATION |

“默认可用”描述无动态调整模式的本地初始状态。源 `enableSwitchProfile` 还会受调整模式和动态 Loupedeck 条件影响；本地尚无这些服务状态的完整映射，不能外推成所有运行状态都可用。3946 构造与导航更新的 CALIBRATION 差异另见自动化审计。

`tools/audit-product-profile-bars.cjs` 用 Acorn 保存上述八个有配置栏产品的根 props、renderProfileBarIcon、renderProfileBar、enableSwitchProfile、setProfileDropdownState 和相关 CSS，包含当前源 SHA-256 / UTF-16 偏移。179 单独见 [接收器审计](receiver-profile-header-current-audit.md)。784、无线、有线、自动化审计还记录了别名解析与 Redux 初始值。

本地以 `source_workspace/profile_bar.rs` 统一实际条件，使用已有 SynapseSelect 的源下拉样式取代宽 180 的通用 Kit Select：rename-rect 250px、左右各 10px、下拉 230×27px、Roboto 14px、loader 26px，以及原版 default / unsupported 图标。整栏禁用仅应用一次 .3 透明度，避免父容器与 Select 叠成 .09。

当前仍缺这批产品的完整配置更多菜单、部分右侧条件图标和动态模式行为；不能将下拉修正称为整个配置栏完整复刻。其余未重新追踪的产品 HELP 条件仍保留原实现，未以本表推广猜测。

静态证据：[product-profile-bars-current-evidence.json](product-profile-bars-current-evidence.json)。重新验证：`node tools/audit-product-profile-bars.cjs --check`。未执行原版 JavaScript。

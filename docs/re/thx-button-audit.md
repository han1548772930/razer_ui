# `.thx-btn`（Synapse 对话框按钮）依据审计

2026-10-03。机器可读结果 [thx-button-audit.json](thx-button-audit.json)，脚本 [tools/audit-thx-button.cjs](../../tools/audit-thx-button.cjs)（`--check` 失败即报错）。
源码：`.ref/applications/synapse/profiles/static/css/main.ee3cb5b6.css`。

| 选择器 | 源码声明 | 本地 |
| --- | --- | --- |
| `.thx-btn` | `background-color:#44d62c;border-radius:3px;color:#000;padding:.5rem 1.5rem;text-align:center;text-transform:uppercase;transition:opacity .3s` | 主按钮 `ThxKind::Primary`：`#44d62c` 底、`#000` 字、圆角 3px、文案 `to_uppercase()` |
| `.thx-btn:hover` | `opacity:.8` | `.hover(\|button\| button.opacity(0.8))` |
| `.thx-btn:active` | `opacity:.6` | 未实现：`gpui_kit` 的 `Button` 只暴露 `.hover`（按下态需要像 `hover_border_color` 那样自持状态插值），原值已记录 |
| `.thx-btn.disabled,.thx-btn.disabled:hover` | `cursor:default;opacity:.3` | `disabled(true)`（`Button` 自带的禁用态）+ 调用点的 `.disabled(…)` |
| `.thx-btn.test` | `background-color:#707070;border:1px solid #0000004d;color:#fff` | 次按钮 `ThxKind::Test`：`#707070` 底、`#fff` 字 |
| `.profile-del div.thx-btn` | `background-color:#fd4949;border:1px solid #0000004d;color:#111;font-size:12px;height:27px;line-height:14px;min-width:90px;padding:4px 5px` | 删除/重置确认弹层的红按钮仍在 `profile_confirmation` 里由 `ProfileAlertColors` 提供（含 777 的覆盖），因此没有做成第三个变体 |
| `.import-profile-btn-group .thx-btn` | `border:1px solid #0000004d;font-family:Roboto;font-size:12px;height:100%;width:fit-content` | 按钮 27px 高、12px 字、1px `#0000004d` 边框、`min-width:90px` |

本地实现是 [profile.rs](../../crates/razer-pages/src/features/profile.rs) 里的 `profile_dialog_button(id, label, ThxKind, cx)`，两个对话框（关联游戏、配置文件导入/导出）的取消/关闭按钮用 `ThxKind::Test`、确认/保存按钮用 `ThxKind::Primary`。

`transition:opacity .3s` 与按下态 `opacity:.6` 尚未做插值——本轮只做到静态色 + 悬停透明度，原值记在上面。删除确认里的红色按钮没有并入 `ThxKind`：它需要按产品（777）切换边框与文字色，且已有独立实现。

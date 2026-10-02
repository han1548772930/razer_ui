# 账户菜单

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](../re/20-current-source-version.md)为准。
依据 `.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js` 的 `K/X`，头像打开独立 `/rz-user-profile-menu/` iframe；实际菜单来自 [Root.012dd389.chunk.js](../../.ref/rz-user-profile-menu/static/js/Root.012dd389.chunk.js) 的 `N`，样式来自同应用 `main.090e2108.css`。

当前 [account_menu.rs](../../src/shell/account_menu.rs) 恢复本地访客分支：登录、分隔、反馈、分隔、退出。登录在原代码调用 `logOut` 重新进入宿主登录流程，不能替换为随意打开账户网页。反馈也需要原宿主窗口协议；当前两项禁用并说明原因。没有真实账户时不显示用户名、余额、修改密码或退出登录；评分项仍由原 `canShowRating` 条件约束，不默认增加。

头像触发器为 46×38，原 guest SVG 显示为20×20；展开背景 `#2d2d2d`。iframe 顶部40、菜单 margin-top2，相对38高触发器的弹出间距为4。菜单右齐、223宽、黑底、1px `#5d5d5d` 边、5px圆角、7×14内距；命令26高、5/18/4内距、13px圆角，前两行下边距4，分隔线上下各7。悬停 `#1f1f1f`，禁用文字30%不透明度。所有数值通过项目rem比例换算，颜色集中于主题层。

Base Popover 与 List 负责打开、方向键、Enter、Escape、外点关闭和焦点返回；命令不重写键盘导航。退出先关闭菜单，再由 AppShell 复用标题栏退出路径，保留未保存更改确认及保存期间的保护。未接入账户或反馈服务，不将本地菜单展示当作登录成功。

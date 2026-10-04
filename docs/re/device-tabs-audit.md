# 当前导航栏复核

2026-10-04 集成补充：已接上 Macro 和 Armory 的内部历史及壳观察订阅；关闭标签留下相邻重复外层记录时，前进/后退跳过重复项。Dashboard 96776 的 `l` 导入模块54693，`IUp → BACK`、`OXp → FORWARD`；Profiles与Macro也按自身作用域得到相同键，已撤销此前`NAVIGATE_BACK/FORWARD`的错误引用。静态校验覆盖五类内部历史的目标解析和激活接线，但没有运行点击验收。

2026-10-04。[独立源码复核](source-ui-review-2026-10-04.md)给出182的实际组件链与偏移；[机器收据](device-tabs-audit.json)由[当前静态提取器](../../tools/audit-device-tabs.cjs)生成。

旧审计要求设备页出现 `.nav.back/.nav.forward` 按钮是错误的：CSS有这两个类，但当前 mounted navs-wrapper 并未渲染它们。实际子节点是 profile-wrapper、navs-wrapper、right；right内才是电量和帮助。现已删除第二套箭头，壳工具栏调用产品内部历史。取消脏映射确认不会提前改变历史索引。

共享按钮与溢出行改为Base Button，避免component Button渲染时重复注册hover。选中标签hover保持绿色/黑字；溢出按钮按 `.navs-wrapper .dots3` 覆盖为无边框，选中项隐藏时用绿色底和原active图标。点击溢出项关闭菜单后导航。

左右列分别为1/0/25%、1/1/25%，中心grow1且不缩小。电量文本按Roboto14px测量并与尺寸一起归一化至CSS像素；已移除固定80px右区常量。源工作区和主导航已接共享溢出菜单。当前宽度来自已核对控件尺寸和文字测量，尚未达到原DOM逐子节点clientWidth测量完全等价；具体产品的额外header分支仍需继续接入。

Dashboard四项文案已换为真实模块导出：DASHBOARD_HEADER、GAMER_ROOM_HEADER、DEVICES_AND_MODULES_HEADER、GLOBAL_SHORTCUT_HEADER。校验覆盖十种当前应用语言。

用户反馈所有前进/后退无效后，启用状态与点击统一使用history_target：先解析当前应用/产品内部可走记录，在边界再使用已记录的外层页面切换。外层衔接是本地统一壳的产品适配，不冒充原版独立Web窗口行为。按钮采用Base40×38、20px原图、禁用opacity.3，去除styled Button中间层。未运行应用，点击效果仍没有运行时验收结论。

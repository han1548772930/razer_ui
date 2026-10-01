# 电源：182 与 777

> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 页面归属

`[JS]` 182 根 `GM.navs → MM`；777 根 `Ov.navs → KG → kG`。653 没有 Power 页，Lighting 中的关灯条件不能当作整机休眠。

两个产品虽然都显示电源选项，但值域、开关与限制不同，不能用一套通用页面覆盖。

## 2. 182 闲置与低功耗阈值

[182 main](../../.ref/devices/182/static/js/main.db20a7c4.js) 的 `CM`、`pM`：

| 控件 | 实际契约 |
|---|---|
| 闲置计时 | 1–15 分钟，连续整数；当前入口没有额外总开关 |
| 低功耗阈值 | 5–100，step 5 |
| 阈值禁用 | 有效回报率 >1000 时禁用，并显示对应提示 |
| profile/连接 | 从实际 profile 和有效 polling 数据同步，不能只看一个本地字段 |

“低功耗模式阈值”不是当前电量，不能作为电池百分比实时显示；也不能在禁用时仍接受 slider 操作。USB/wireless 有效回报率的来源与 Performance 保持一致。

[182 CSS](../../.ref/devices/182/static/css/main.48c20423.css) 使用通用 widget、slider、说明/警告布局。无证据要求在顶部附加产品大图。

## 3. 777 自动省电

[777 main](../../.ref/devices/777/static/js/main.eb70ce38.js) 的 `kG → zG → wG`：

- wG 的默认 min=15、max=60、step=1，但父组件 zG 实际传 **min=5**。
- 此入口有效范围为 **5–60 分钟，step 1**。
- 数据对象包含 `isEnabled` 与 `value`；开关决定计时 slider 的可编辑状态。
- 挂载时调用 `loadPowerSavingFromDevice` 取得设备值，不能仅从通用默认值初始化。
- 切开关不应把记住的 value 重置成默认；收到设备值后与 profile/页面状态同步。

CSS 为 [main.e4bab2aa.css](../../.ref/devices/777/static/css/main.e4bab2aa.css) 的通用 widget 和省电控件。5、15、60 属于业务分钟，不是原始 CSS px。

## 4. 资源与边界

本页主要由文字、Switch、Slider、tooltip 和 CSS 构成；没有必须新增的休眠动画或扫描插图。字体来自原包 Roboto；帮助/说明图标见 [资源索引](../re/04-resource-index.md)。

灯光关闭、整机省电、系统电源状态和设备低功耗阈值是不同领域，不能混合成一个“电源开关”。

## 5. 重构前基线与验收

`[RUST 基线]` [power.rs](../../src/features/power.rs) 尚未完整匹配产品差异；AppShell 的 setter 主要改变内存，缺设备读取和确认链。静态展示范围正确也不足以证明功能接通。

`[建议]` 按 product capability 构建两套明确配置，GPUI Kit 以 retained SliderState 保留拖动/焦点，禁用来自领域状态。验收覆盖：182 两个边界及 >1000 Hz 禁用；777 5/60 边界、开关保留值、设备初次回读、切 profile、断连和失败恢复。

## 6. 2026-10-01 样式接入

本轮复核 182 `CM/pM` 与 777 `wG.render` 后，[当前页面](../../src/features/device_pages.rs) 按产品分开构造卡片：182 保留无总开关的无线节能与低能耗两卡；777 使用 `panel_with_control` 把 32×18 开关放入节能标题行，移除原实现额外占一行的“自动关闭”开关标签。

卡片说明改用原语言键 `POWER_SAVING_DESC`、`LOW_POWER_MODE_DESC` 和 `AUDIO_POWER_SAVING_DESC`。滑条不再把“分钟/当前值/范围”拆成多个纵向段落，而是恢复源 `slider-container` 的 64px 区域：轨道位于距底 25px 处，当前值显示在上方气泡，下方两端分别为 1/15、5%/100% 或 5/60。气泡位置读取已有 SliderState 百分比；禁用仍来自领域状态，保留记住的分钟值和所有原订阅。

当前使用框架水平 Slider 保留拖动、键盘与焦点行为；气泡尖角及滑块悬停动画尚未完全复现 CSS。原硬件读取、有效无线回报率和提交确认仍属于功能缺口，不能据样式修正宣称硬件已接通。整个复核不使用旧截图。

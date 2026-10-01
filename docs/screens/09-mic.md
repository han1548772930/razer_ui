# 麦克风：777 独立 micEq

> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 实际入口只有 EQ

`[JS]` [777 main](../../.ref/devices/777/static/js/main.eb70ce38.js)：`Ov.navs → KM → kM → YM → gM/GM`。

kM.render 只挂载 EQ，传入 Mic_EQ_Tabs、micBandPreset、layoutOptions=wM、options=zM、widgetId=eqBox。没有独立 Mic Gain、Sidetone、Noise Cancellation、XLR 或麦克风音量卡。共有 profile 字段不能证明这些控件在此页显示。

YM 读取 **micEq** 的 activePresetMode、frequencyBands、customBands、deviceEqDifferent；setEQ 经 `vM → Oe.IkZ`，setSaveDeviceEQ 经 BM。与 audioEq 是不同的 action 和数据。

wM 为 sliderType=wide、yTitleAlign=flex-end、yTitleMarginRight=40px、tabScale=true；zM 为 **-5..5 dB、step 1**，Y 轴标签 -5/0/+5 dB。

## 2. 预设和原包数据矛盾

| preset ID | 各频段 dB |
|---|---|
| default | 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 |
| boost | 0, 0, 2, 4, 5, 5, 5, 5, 2, 1 |
| broadcast | -2, -1, 1, -1, 0, 0, 0, 2, 1, 1 |
| conference | -5, -5, -5, -3, 1, 0, 3, 2, 1, 0 |
| custom | 使用 mic 的 customBands，缺少则 default |

配置模块 7816 中存在明确不一致：

```js
// 导出的 micBandFrequency
N = [250,750,1000,1200,1500,2300,3000,4000,5000,7000];
// audioBandFrequency
u = [31,63,125,250,500,1000,2000,4000,8000,16000];
// 实际 micBandPreset 的 reduce 使用 u
C = Object.entries({...}).reduce((e,E) => {
  let a = (0,t.A)(E,2), _ = a[0], o = a[1];
  e[_] = o.map((e,E) => ({frequency:u[E],decibel:e}));
  return e;
}, {});
```

GM 对非 Custom 模式使用 presetData[activePresetMode]，因此本包预设画面/提交数据沿用 **u 频率集合**，不能只看到 micBandFrequency 导出就认定图中必然是 250–7000 Hz。

Custom 可能来自设备回传 frequencyBands；本轮没有验证真实设备最终采用哪一组频率。实现模型应保留 `{frequency, decibel}`，记录原包行为与硬件回读的区别，不能静默改成一组“更合理”的频率并宣称完全一致。

## 3. 编辑契约

与 [声音页](08-sound.md) 共用 GM 行为，但读写 mic 域：

- 拖动频段按 frequency 修改，进入 custom。
- 选择预设使用稳定 ID；Custom 恢复 mic customBands。
- Reset 生成 mode=custom 加 default bands，不自动选 default tab。
- deviceEqDifferent 有保留设备 EQ 的确认，确认结果仅作用于 mic。
- profile 切换后，preset、频段、custom 缓存与控件状态一致更新。

仅用 Vec<i8> 无法完整保留频率、预设与设备差异信息。

## 4. 布局和资源

[777 CSS](../../.ref/devices/777/static/css/main.e4bab2aa.css) 中 #eqBox 宽/最小宽/最大宽基线 940，最小高度 473。只有 kM 实际传入这个 ID，不能套在 Sound 上。

tabScale 分支宽 calc(100% + 10px)，预设等分伸展，padding 7px 0 6px；Y 轴区域约 300，wide 纵向 slider 与 Sound 的 narrow 不同。当前树不含产品 banner。共有 icon-tab 样式存在不代表 kM 传了 isIconTab。

资源为 [eq_reset](../../.ref/devices/777/static/media/eq_reset.e0c3c09c.svg)、[hover](../../.ref/devices/777/static/media/eq_reset_hover.186df33c.svg)、[pressed](../../.ref/devices/777/static/media/eq_reset_active.37c570d3.svg) 及 Roboto；图表坐标由 CSS/组件生成，不应改用静态截图。

当前Mic沿用原940px固定面板，通过正文横向滚动适配窄窗口，不压缩78px频段间距。新增生产视图测试覆盖1100→700→1100宽度变化，滚至16kHz和Reset、拖动末频段、验证10个保留控件与原有曲线数据，并检查Sound数据隔离；这些用例只编译，未运行。Mic与Sound共用已还原的随滑块移动数值气泡。

## 5. 重构前基线与验收

`[RUST 基线]` [mic.rs](../../src/features/mic.rs) 虽取得 features.mic，实际频段却读取 **features.sound.equalizer.bands**，slider 回调也调用仅写 sound 的 AppShell::set_eq_band。调整 Mic 会修改 Sound，是明确的数据隔离问题。

同时缺独立 mic preset/custom、Reset、设备 EQ 确认，wide 纵向布局也未还原。部分渲染字符串使用 Box::leak，应在后续代码修订时改为受控生命周期，不能每次 render 永久泄漏。

`[建议]` mic feature 拥有独立状态和 retained SliderState，订阅持有至 owner 生命周期结束。验收先保证改 Mic 不改 Sound，再验证五个 preset、Custom/Reset、设备差异确认、profile 隔离与频率来源。真实频率语义需设备验证，文档不替硬件作结论。

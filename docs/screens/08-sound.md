# 声音：777 音量、系统属性与 audioEq

> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 页面树

`[JS]` [777 main](../../.ref/devices/777/static/js/main.eb70ce38.js)：`Ov.navs → WM → VM`。

```text
VM / Bm body-widgets
├─ eM → Zm：产品图
├─ ym(direction=left)
│  ├─ NM → dM：VOLUME_HEADER
│  └─ rM → sM：SOUND_PROPERTIES
└─ ym(direction=right)
   └─ FM → gM → GM：声音 EQ
```

VM 没有给 EQ 传 widgetId=eqBox，不能套用 Mic 的固定 940 宽。当前树也没有 THX/环绕校准向导。

## 2. 音量与系统入口

NM 从 playbackReducer 读取 isEnabled/value。dM 开关回调调用 setMute(nextEnabled)，应保留原布尔语义，不要因函数名有 Mute 就在 UI 随意再取反。Slider 为 **0–100、step 1**，关闭时 inactive；拖动调用 setVolume，和 EQ 独立。

OPEN_WINDOW_VOLUME_MIXER 打开 Windows 音量混合器；SOUND_PROPERTIES 卡调用系统声音属性。sM 有 isTHX 共有分支，但本页 rM 没有传入该能力，不能自动添加 THX 图/按钮。

## 3. EQ 数据和预设

FM 读取 **audioEq**：activePresetMode、frequencyBands、customBands、deviceEqDifferent，setEQ 经 `fM → Oe.JSP` 对应音频 action。

产品配置模块 7816 的频率为：

`31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000 Hz`

| preset ID | 各频段 dB，按上述顺序 |
|---|---|
| default | 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 |
| amplified | -2, 2, 4, 5, 5, 5, 5, 5, 3, -2 |
| vocals | 3, 3, 3, 1, -2, -2, -2, 1, 3, 4 |
| bassboost | 5, 5, 5, 3, 0, 0, 0, 2, 2, 0 |
| enhancedclarity | -5, -4, -2, -1, 0, 0, 0, 2, 2, 2 |
| custom | 读本 profile 的 customBands，缺少时回退 default |

options bM 为 min=-5、max=5、step=1，Y 轴标签 -5dB/0dB/+5dB。layoutOptions yM 为 sliderType=narrow、yTitleAlign=flex-end、yTitleMarginRight=40px、tabScale=false。

## 4. 交互和布局

共有 GM 的契约：

| 操作 | 结果 |
|---|---|
| 拖动频段 | clone 当前 bands，按 frequency 更新；提交 mode=custom |
| 选择预设 | 使用稳定 preset ID 和 presetData，不用翻译文字识别 |
| 切 Custom | 恢复 customBands，缺少则 default |
| Reset | 提交 mode=custom、frequencyBands=presetData.default；不自动选 default tab |
| deviceEqDifferent | 初次挂载时可显示是否保留设备 EQ 的确认；setSaveDeviceEQ(true/false) |

原图表是十个纵向滑条，后接右侧 Y 轴与重置图标。`uM/cM` 的 range 长度为 300，narrow 每段总宽 47（15+16+16），频率标签距顶 310；Y 轴高 300，Reset 为 20px 图标并在同高区域居中。原先“共有容器高 350、padding 20px 19px”的描述不属于当前 `uM/cM` 渲染链，已撤回。audioOutputSource==Headset 影响可用性，不能把其它输出类型的分支当本产品新增页面。

[777 CSS](../../.ref/devices/777/static/css/main.e4bab2aa.css)：preset-list 使用 flex wrap、gap 12；普通 preset 高 27、min-width 90、12px、边框 #5d5d5d，active 有不同填充。窗口变窄时允许预设换行，不让频段标签互相覆盖。

当前 Rust 同步了 `cM` 的数值气泡：悬停时显示、随滑块移动，26×20、左偏移−28，纵向位置为 `282−284*(value+5)/10`。正数显示 `+n`、零显示 `0`，不加dB后缀。真实生产视图测试覆盖位置和显隐，现阶段仅编译检查；窄窗布局及正文滚动规则见[响应式复核](../re/06-style-source-audit.md#10-原版响应式规则与正文滚动)。

## 5. 资源

| 用途 | 原始名称 / 文件 |
|---|---|
| 产品 1x | ./777_0/img_prods/prd-1x.png → [prd-1x.5cd1e5ea.avif](../../.ref/devices/777/static/media/prd-1x.5cd1e5ea.avif)，模块 5298 |
| 产品 3x | [prd-3x.ede8800a.avif](../../.ref/devices/777/static/media/prd-3x.ede8800a.avif)，模块 4024 |
| Reset 默认 | [eq_reset.e0c3c09c.svg](../../.ref/devices/777/static/media/eq_reset.e0c3c09c.svg) |
| Reset hover | [eq_reset_hover.186df33c.svg](../../.ref/devices/777/static/media/eq_reset_hover.186df33c.svg) |
| Reset pressed | [eq_reset_active.37c570d3.svg](../../.ref/devices/777/static/media/eq_reset_active.37c570d3.svg) |
| 系统属性图 | sM 使用 nM 内嵌 SVG |

Helicopter WAV 虽由产品配置导出，VM 没有渲染测试按钮；不得仅凭音频文件存在就新增播放/环绕校准区。[资源索引](../re/04-resource-index.md) 包含完整导出映射。

## 6. 重构前基线与验收

`[RUST 基线]` [sound.rs](../../src/features/sound.rs) 以宽 EQ 和通用水平 slider 为主，缺实际左列/产品图/系统属性布局。预设是缺处理的 div，缺完整 preset/custom、Reset 和设备 EQ 确认。AppShell::set_eq_band 用 -12..12，不符合原版 -5..5。

`[建议]` 由 sound feature 保留各频段 Entity<SliderState>，使用 Slider::vertical()，ID 由设备/profile/频率派生。验收覆盖音量/静音、系统入口、六种 preset、编辑切 Custom、Reset 后保留 Custom 语义、设备差异弹窗、profile 隔离，以及与 Mic 互不影响。

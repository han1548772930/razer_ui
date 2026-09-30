# 麦克风（TAB_MIC）

> 本页由自动脚本从雷云**自己的模块**提取，未做臆测。
> 重新生成：`node .ref/tools/gen-screen-docs.js`

## 1. 出现在哪些设备上

| 设备 | productId | 设备类型 | 该设备标签页顺序 |
|---|---|---|---|
| RAZER KRAKEN BT SANRIO LIMITED EDITION | 777 | 耳机 | 第 6 个：自定义 · 灯光 · 校准 · 电源 · 声音 · 麦克风 |

标签页真实文案：**麦克风**（key `TAB_MIC`）

## 2. 布局

### 2.0 页面骨架（所有设备页共用）

设备页**不是**「一列全宽卡片」。真实骨架如下（逐条引自设备模块的 CSS）：

| 选择器 | 布局 | 样式 |
|---|---|---|
| `body, html` | `font-family:Roboto,sans-serif; font-size:16px; height:100%; margin:0; max-width:1920px; min-height:720px; overflow:hidden; width:100%` | `background-color:#222; color:#ccc; user-select:none` |
| `.main-container` | `display:flex; flex-direction:column; height:100%; min-width:600px; position:absolute; width:100%` | `background-color:#222` |
| `.nav-tabs` | `display:flex; align-items:center; min-height:48px; position:relative; width:100%; z-index:105` | `background-color:#222; border-bottom:2px solid #000; color:#5d5d5d` |
| `.body-wrapper` | `flex:1 1; height:100%; min-width:600px; padding:10px 20px 20px; width:100%` | — |
| `.body-widgets` | `flex-direction:row; flex-wrap:wrap; justify-content:center; margin:auto; max-width:1240px` | — |
| `.body-widgets .widget` | `flex:0 0 auto; height:auto; margin:10px auto; max-width:600px; min-width:600px; padding:30px 40px; position:relative; font-size:14px` | `background-color:#111; border-radius:5px` |
| `.widget-col` | `flex-direction:column; height:fit-content; width:600px` | — |
| `.widget-prod` | `height:250px; margin:10px auto; max-width:1220px; min-width:1024px; width:100%` | — |
| `.widget-prod img` | `left:50%; position:absolute; top:50%` | — |
| `.thx-btn` | `display:inline-block; padding:.5rem 1.5rem; text-align:center; text-transform:uppercase` | `background-color:#44d62c; border-radius:3px; color:#000` |
| `.thx-btn:hover` | — | `opacity:.8` |
| `.thx-btn:active` | — | `opacity:.6` |
| `.thx-btn.disabled` | `cursor:default` | `opacity:.3` |
| `.thx-btn.secondary` | — | `background-color:#707070; color:#fff` |

由此可推出的界面排布：

```
.main-container（纵向，底 #222）
├─ .nav-tabs           顶栏，最小高 48px，下边线 2px #000，文字 #5d5d5d
└─ .body-wrapper（内边距 10/20/20）
   ├─ .widget-prod      顶部产品图区，高 250px，宽 1024–1220px，图片绝对居中
   └─ .body-widgets     横向排列 + 自动换行 + 居中，最大宽 1240px
      ├─ .widget        固定 600px 宽，内边距 30/40，底 #111，圆角 5px，字号 14px
      ├─ .widget
      └─ .widget-col    600px 宽的列，内部再纵向堆叠
```

> 关键数值：**卡片固定 600px、容器最大 1240px、产品图区高 250px、控件圆角 3px、卡片圆角 5px**。

### 2.0.1 全局样式

**调色板**（按出现次数排序，共统计 1591 处颜色声明）：

| 次数 | 颜色 | 角色 |
|---:|---|---|
| 252 | `#44d62c` | 主色 / 激活态 / 标题 |
| 236 | `#ccc` | 主文字 |
| 145 | `#5d5d5d` | **通用边框** |
| 124 | `#111` | 卡片 / 输入框底色 |
| 123 | `#000` | 按钮文字、描边、顶栏下边线 |
| 93 | `#0000` | 透明 |
| 68 | `#707070` | 次要按钮底色 |
| 57 | `#fff` | 次要按钮文字 |
| 55 | `#999` | 次要文字 |
| 47 | `#222` | 页面底色 / 顶栏底色 |
| 39 | `#fd8611` | 橙色：提示 / 警告 / 分享 |
| 24 | `#ffffff1a` | 半透明白：hover 覆盖 |
| 20 | `#0000004d` | 半透明黑：按钮描边 |
| 18 | `#212121` | 深色文字（浅底上）/ 离线指示 |
| 18 | `#c8323c` | 危险红 |
| 17 | `#ffffff4d` | 半透明白：按下覆盖 |
| 14 | `#fd4949` | 危险红（亮） |
| 14 | `#333` | 开关关闭态底色 |
| 13 | `#2d2d2d` | **hover 底色** |
| 11 | `#515151` | **下拉框边框** |
| 10 | `#4a4a4a` | 帮助图标底色 |
| 9 | `#292929` |  |

**字体与圆角**（同样按出现次数排序）：

| 项 | 值 | 说明 |
|---|---|---|
| 正文字体 | `Roboto, sans-serif` | `body,html` 全局设置 |
| 标题字体 | `RazerF5` | 区块标题（如 `.key-config .body .heading`） |
| 正文字号 | `14px` | 出现 251 次，设备页里最常用 |
| 小字号 | `12px` | 出现 76 次，次要按钮与说明 |
| 全局字号 | `16px` | `body,html`；进入 `.widget` 后变为 14px |
| 控件圆角 | `3px` | 出现 89 次 |
| 卡片圆角 | `5px` | 出现 43 次 |
| 窗口最小尺寸 | `min-width:600px` / `min-height:720px` | `.main-container` / `body,html` |

> ⚠️ 通用边框是 **`#5d5d5d`**（出现 162 次），不是 `#555`（仅 7 次）。
> hover 底色是 **`#2d2d2d`**；下拉框边框是 **`#515151`**。

### 2.1 该界面的分区（布局 + 样式）

下表把该界面的类名按语义归入 UI 分区，并给出它们在 CSS 里的**实际布局与样式声明**。
分区顺序即界面自上而下 / 自左而右的排布。

#### RAZER KRAKEN BT SANRIO LIMITED EDITION（productId 777）

该界面共 64 个布局类名，分 2 个分区。

**① 麦克风音量 / 增益**（5）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `mic-container` | `position:relative`<br>`left:25%; max-width:300px; padding:8px 10px; position:absolute; top:40%` | `background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-family:Roboto; line-height:16px; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `mic-enhancements` | — | — |
| `micboost` | `width:32px` | `background-image:url(../../static/media/icon_audio_profile_mic_boost.04a476a1.svg)` |
| `microphone` | — | — |
| `profile-dynamic-tips` | `align-items:center; display:flex!important; flex-direction:column; left:10px; padding:20px; position:absolute`<br>`height:auto; left:50%; padding:20px; top:40px; width:300px` | `background-color:#111; background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px 0 #0003; box-shadow:0 6px 10px #0003; opacity:0; text-transform:none; transition:visibility 0s,opacity .3s linear`<br>`color:#ccc; font-size:14px`<br>`opacity:1` |

**③ 监听与降噪**（59）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `MonitoringDashboard_advancedTooltip__JWaSH` | `display:flex; flex-direction:column; gap:8px`<br>`align-items:flex-start; display:flex; gap:4px; min-width:0`<br>`display:-webkit-box; margin:0; overflow:hidden`<br>`align-items:center; display:flex; gap:20px` | `line-height:16px`<br>`font-size:20px; line-height:24px` |
| `MonitoringDashboard_arrowIcon__n2xJg` | `align-items:center; align-self:stretch; display:flex; padding:0 4px` | `background:none; border:none; color:#fff`<br>`background-color:#383838`<br>`border-right:1px solid #5d5d5d`<br>`border-left:1px solid #5d5d5d`<br>`opacity:.5` |
| `MonitoringDashboard_basicBar__aKKrl` | `height:5px; overflow:hidden; position:relative; width:80px`<br>`left:0; position:absolute; top:0` | `background-color:#111; border-radius:4px` |
| `MonitoringDashboard_basicHeader__VWU9U` | `align-items:center; display:flex; justify-content:space-between; width:100%` | — |
| `MonitoringDashboard_basicLeft__Xf223` | `align-items:center; display:flex; flex:1 1; gap:8px; min-width:0; overflow:hidden`<br>`min-width:0; overflow:hidden`<br>`flex-shrink:0` | — |
| `MonitoringDashboard_basicSection__xKLSI` | `min-width:100px` | — |
| `MonitoringDashboard_basicTooltipHeader__laBoJ` | `align-items:flex-start; display:flex; justify-content:space-between; min-height:35px`<br>`align-items:center; display:flex; flex:1 1; gap:8px; min-width:0; overflow:hidden`<br>`display:-webkit-box; flex:none; max-width:120px; overflow:hidden` | `text-align:right` |
| `MonitoringDashboard_basicTooltipTable__XhSAs` | `width:100%` | `font-size:12px; line-height:14px`<br>`text-align:right` |
| `MonitoringDashboard_basicWrapper__KHmpB` | `display:flex; flex-direction:column; gap:8px; width:100%` | — |
| `MonitoringDashboard_checkboxContainer__9450v` | `display:flex; flex-direction:column; gap:10px`<br>`margin:0` | — |
| `MonitoringDashboard_checkboxWrapper__AgIMu` | `align-items:center; display:flex; gap:8px; justify-content:space-between; width:100%`<br>`margin:0` | — |
| `MonitoringDashboard_disabled__sjQ5Z` | `display:none` | `color:#999`<br>`color:#fff`<br>`opacity:1` |
| `MonitoringDashboard_fanControls__NuJP5` | `align-items:center; display:flex` | — |
| `MonitoringDashboard_fitContent__Oqcl4` | `width:-webkit-fit-content; width:fit-content` | — |
| `MonitoringDashboard_iconSvg__2BdHI` | — | `color:#999` |
| `MonitoringDashboard_icon__m3QC-` | `align-items:center; display:inline-flex; flex-shrink:0` | — |
| `MonitoringDashboard_menuButton__IkMX-` | `align-items:center; display:flex; justify-content:center; padding:0; position:relative` | `background:none; border:1px solid #0000; border-radius:0; color:#ccc; cursor:pointer`<br>`outline:1px dotted #0f0`<br>`border-color:#5d5d5d`<br>`color:#999`<br>`color:#fff` |
| `MonitoringDashboard_menuContainer__IwRdF` | `align-items:center; display:flex; justify-content:flex-end; position:relative` | — |
| `MonitoringDashboard_menuItemActive__tQ6d5` | — | `color:#44d62c` |
| `MonitoringDashboard_menuItemDisabled__CtX9E` | — | `color:#666!important; cursor:not-allowed; opacity:.5` |
| `MonitoringDashboard_menuItemSelected__DKoXf` | — | `color:#44d62c!important` |
| `MonitoringDashboard_menuItem__j56gr` | `padding:4px 8px` | `color:#ccc; cursor:pointer; font-family:RazerF5,sans-serif; font-size:14px; text-align:left; transition:background-color .2s ease`<br>`background:#ffffff1a; color:#fff` |
| `MonitoringDashboard_menuSection__PXUeC` | `align-items:center; display:flex; flex:0 0 auto; height:auto; position:relative` | — |
| `MonitoringDashboard_menuSeparator__-P85c` | `height:1px; margin:4px 5px` | `background-color:#333` |
| `MonitoringDashboard_menu__WGvuo` | `min-width:200px; padding:4px 0` | `background:#1a1a1a; border:1px solid #333` |
| `MonitoringDashboard_metricBarFill__TKivl` | `height:100%; left:0; position:absolute; top:0`<br>`left:0; position:absolute; top:0` | `border-radius:4px; transition:width .3s ease,background-color .3s ease`<br>`background-color:initial`<br>`background-color:#44d62c`<br>`background-color:#ff4`<br>`background-color:#f84` |
| `MonitoringDashboard_metricBarGraphical__ImLFV` | `height:40px; overflow:hidden; position:relative`<br>`left:0; position:absolute; top:0` | `background:#111; border-radius:4px`<br>`transition:width .3s ease` |
| `MonitoringDashboard_metricBar__0oZYy` | `height:5px; overflow:hidden; position:relative; width:100%`<br>`min-width:60px; width:100%` | `background-color:#111; border-radius:4px` |
| `MonitoringDashboard_metricVertical__fXfJS` | `align-items:flex-end; display:flex; flex-direction:column; gap:4px`<br>`min-width:60px; width:100%` | — |
| `MonitoringDashboard_metric__fKu5T` | `display:flex; flex:0 0 auto; flex-direction:column; gap:4px` | — |
| `MonitoringDashboard_metrics__TiaS0` | `display:flex; flex-direction:row; flex-wrap:nowrap; gap:10px; justify-content:flex-start` | — |
| `MonitoringDashboard_model__JBSYk` | `min-width:0; overflow:hidden`<br>`flex:1 1`<br>`flex-shrink:0`<br>`display:-webkit-box; flex:none; max-width:120px; overflow:hidden` | `color:#ccc; font-family:Roboto,sans-serif; font-size:12px; text-align:left`<br>`text-align:right`<br>`line-height:16px` |
| `MonitoringDashboard_overlay__Cpz6F` | `align-items:flex-start; bottom:0; display:flex; height:100vh; justify-content:center; left:0; margin:0; overflow:hidden; position:absolute; right:0` | `background-color:#00000080` |
| `MonitoringDashboard_popupColumns__cAUDk` | `display:grid; gap:20px; grid-template-columns:1fr 1fr` | — |
| `MonitoringDashboard_popupContent__8o7ZR` | `height:100%; overflow-y:auto; padding:24px` | — |
| `MonitoringDashboard_popupControls__qLFiC` | `align-items:center; display:flex; justify-content:center; position:relative` | — |
| `MonitoringDashboard_popupHeader__TeQL4` | `align-items:center; justify-content:space-between; padding:11px 10px 9px; position:-webkit-sticky; position:sticky; top:0` | `background-color:#222; border-bottom:1px solid #333` |
| `MonitoringDashboard_popupTitle__F0h6n` | `margin:0` | `color:#5d5d5d; font-family:RazerF5,sans-serif; font-size:16px; font-weight:400; text-align:center` |
| `MonitoringDashboard_radioContainer__CsoCc` | `display:flex; flex-direction:column; gap:10px`<br>`margin:0` | — |
| `MonitoringDashboard_sectionContainer__TqDpM` | `display:inline-flex; overflow-y:hidden; padding:12px 10px; position:relative`<br>`display:none` | — |
| `MonitoringDashboard_sectionGroup__sPvdj` | `height:-webkit-fit-content; height:fit-content; min-height:-webkit-min-content; min-height:min-content; padding:15px; width:100%`<br>`align-items:center; display:flex; gap:8px` | `background:#111; border-radius:5px; text-align:left; transition:opacity .3s ease`<br>`text-align:left` |
| `MonitoringDashboard_sectionTitle__ZgP8U` | `align-items:center; display:flex; gap:8px` | `cursor:pointer` |
| `MonitoringDashboard_sectionToggle__EiZ04` | `align-items:center; display:flex` | — |
| `MonitoringDashboard_section__XkJWH` | `display:flex; flex:0 1 auto; flex-direction:column; justify-content:center; padding:0 12px` | `border-right:1px solid #5d5d5d` |
| `MonitoringDashboard_sectionsWrapper__Ldn29` | `align-items:center; display:flex; justify-content:center` | — |
| `MonitoringDashboard_sensorList__PYVVe` | — | `text-align:left` |
| `MonitoringDashboard_simpleTooltip__pbqV0` | `display:block; padding:4px 8px; width:auto` | `background-color:#333; border:none; box-shadow:none; font-size:12px; text-align:center` |
| `MonitoringDashboard_simpleViewBadge__w3ut-` | `padding:1px 6px` | `background-color:#222; border-radius:10px; color:#999; font-family:RazerF5,sans-serif; font-size:12px` |
| `MonitoringDashboard_suspendedContainer__GH5BY` | `align-items:center; display:flex; flex-direction:column; height:100%; justify-content:center; min-height:48px; padding:0 16px; width:100%` | — |
| `MonitoringDashboard_suspendedText__0IzZk` | `position:relative`<br>`height:16px; position:absolute; right:0; top:50%; width:1px` | `color:#ccc; font-size:14px`<br>`background-color:#5d5d5d` |
| `MonitoringDashboard_title__2zlq7` | `align-items:center; display:flex; gap:8px` | — |
| `MonitoringDashboard_toggleGroup__0p12e` | `align-items:center; display:flex; height:24px`<br>`flex:0 0 auto; justify-content:center; min-width:200px`<br>`justify-content:flex-end; min-width:80px; position:absolute; right:0; top:50%; width:auto` | — |
| `MonitoringDashboard_tooltipContainer__IQu6u` | `height:100%; left:0; position:fixed; top:0; width:100%`<br>`overflow:hidden`<br>`width:-webkit-fit-content; width:fit-content`<br>`display:none` | `opacity:1` |
| `MonitoringDashboard_tooltipText__XgXwU` | `overflow:hidden` | — |
| `MonitoringDashboard_tooltip__uQaOk` | `display:flex; flex-direction:column; gap:8px; max-width:300px; padding:8px 10px; position:absolute; width:240px`<br>`display:none`<br>`display:block; padding:4px 8px; width:auto` | `background-color:#000; border:1px solid #5d5d5d; box-shadow:0 2px 8px #0003; color:#ccc; font-size:14px; line-height:17px; text-align:left`<br>`opacity:1`<br>`background-color:#333; border:none; box-shadow:none; font-size:12px; text-align:center` |
| `MonitoringDashboard_valueRow__aV9sD` | `align-items:center; display:flex; gap:20px` | — |
| `MonitoringDashboard_value__c9hZz` | — | `font-size:20px; line-height:24px` |
| `MonitoringDashboard_width200__VbSC8` | `width:200px!important` | — |
| `MonitoringToggle_button__xH4PT` | `align-items:center; display:flex; justify-content:center; padding:4px` | `background:none; border:1px solid #0000; border-radius:0; color:#999; cursor:pointer; transition:all .2s ease`<br>`border-color:#5d5d5d; color:#fff`<br>`outline:1px dotted lime` |

## 3. 功能项

该界面对应的雷云文案 key，共 **106** 条（中文为雷云原文）：

| key | 中文 | English |
|---|---|---|
| `MIC` | 麦克风 | MIC |
| `MICE` | 鼠标 | Mice |
| `MICROPHONE` | 麦克风 | Microphone |
| `MICROPHONE_EFFECTS` | 麦克风效果 | MICROPHONE EFFECTS |
| `MICROPHONE_EQ_TOOLTIP` | 监控麦克风电平，并调整增益、耳机音量和麦克风监听。你还可以将旋钮设置为控制增益或耳机音量。 | Monitor your microphone levels and adjust the gain, headphone volume, and mic monitoring. You can also set the dial to control either the gain or the headphone volume. |
| `MICROPHONE_FRONT_DIAL_TIP` | 按住多功能轻触式静音传感器可在耳机监听音量（绿色 LED）和麦克风增益（蓝色 LED）之间切换。 | Press and hold the Multi-function Tap-to-mute Sensor to toggle between the Headphone Monitoring Volume (Green LED) and the Microphone Gain (Blue LED). |
| `MICROPHONE_HEADER` | 麦克风 | MICROPHONE |
| `MICROPHONE_MUTE` | 麦克风静音 | MICROPHONE MUTE |
| `MICROPHONE_MUTE_BUTTON_TIP` | 如果 Razer Synapse 雷云在后台运行，则多功能轻触式静音传感器可以执行其他自定义功能。 <br><br>如果 Razer Synapse 雷云未在后台运行，则多功能轻触式静音传感器会执行以下默认功能。 <br>点一下 - 麦克风静音/取消静音  <br>点两下 - 循环切换 Chroma 幻彩效果 <br>点三下 - 禁用/启用 Chroma 幻彩效果 | If Razer Synapse is running in the background, the Multi-function Tap-to-mute Sensor can perform other custom functions. <br><br>If Razer Synapse is not running in the background, the Multi-function Tap-to-mute Sensor will do its default functions below. <br>Single tap - Mute/Unmute Microphone <br>Double tap - Cycle Chroma Effects <br>Triple tap - Disable/Enable Chroma Effects |
| `MICROPHONE_MUTE_TOOLTIP` | 设置麦克风静音按钮在启用或不启用时的颜色 | Set the color of the microphone mute button when active and/or muted |
| `MICROPHONE_OPTION` | 麦克风 | Microphone |
| `MICROPHONE_TOOLTIP` | 调整麦克风输入音量。你可以选择将麦克风静音。 | Adjust the microphone's input volume. You can also choose to mute your microphone. |
| `MICROPHONE_TOOLTIP1` | 请确保设备已设置为你的默认输入设备。 | Adjust the microphone's input volume.<br><br>These settings are saved locally and are applied to all profiles. |
| `MICROPHONE_TOOLTIP_WITHOUT_MUTE` | 调整麦克风输入音量。 | Adjust the microphone's input volume. |
| `MICROPHONE_V2_TOOLTIP` | 麦克风增益/耳机音量<br>调整麦克风在拾取音频输入时的灵敏度和/或耳机端口音频输出的音量。<br><br>采样率<br>修改采样率以控制录音的解析度。<br><br>高通滤波器<br>启用即可过滤低频率的隆隆声和嗡嗡声，例如空调系统的风噪声和远处行驶的汽车发出的声音。<br><br>模拟增益限制器<br>启用即可自动防止削波、峰值和烦人的语音失真。 若启用此功能，麦克风输入栏的轮廓会呈橙色。 | Microphone Gain / Headphone Volume<br>Adjust the microphone’s sensitivity when picking up audio input and/or how loud the audio output coming from the headphone port will be.<br><br>Sampling Rate<br>Change the sampling rate to control the resolution of your recordings.<br><br>High Pass Filter<br>Enable to filter low-end rumbles and hums, such as wind noise from the air conditioning systems and cars driving by in the distance.<br><br>Analogue Gain Limiter<br>Enable to prevent clipping, peaking, and unwanted voice distortions automatically. The microphone input bar will show an orange outline when this is active. |
| `MICROPHONE_V2_TOOLTIP_2` | 若启用此功能，麦克风输入栏的轮廓会呈橙色。 |  |
| `MICROPHONE_V2_TOOLTIP_WITHOUT_HPF` | 麦克风增益/耳机音量<br>调整麦克风在拾取音频输入时的灵敏度和/或耳机端口音频输出的音量。<br><br>采样率<br>修改采样率以控制录音的解析度。<br><br>模拟增益限制器<br>启用即可自动防止削波、峰值和烦人的语音失真。 若启用此功能，麦克风输入栏的轮廓会呈橙色。 | Microphone Gain / Headphone Volume<br>Adjust the microphone’s sensitivity when picking up audio input and/or how loud the audio output coming from the headphone port will be.<br><br>Sampling Rate<br>Change the sampling rate to control the resolution of your recordings.<br><br>Analogue Gain Limiter<br>Enable to prevent clipping, peaking, and unwanted voice distortions automatically. The microphone input bar will show an orange outline when this is active. |
| `MICROPHONE_VOLUME` | 麦克风音量 | Microphone Volume |
| `MICROPHONE_VOLUME_ADJUSTMENT_INDICATOR` | 麦克风音量调节指示灯 | Microphone Volume Adjustment indicator |
| `MICROPHONE_VOLUME_ADJUSTMENT_INDICATOR_TIP` | 调节麦克风音量时，在 LED 上显示音量水平。 | Display the volume level on the LEDs while adjusting Microphone Volume |
| `MICROPHONE_VOLUME_FRONT_DIAL_MESSAGE` | 前旋钮设置为调节麦克风音量 | Front Dial is set to adjust Microphone Volume |
| `MICRO_TACTILE` | 精确触觉滚动 | Precision Tactile |
| `MIC_AI_NOISE_CANCELLATION` | 麦克风 AI 降噪 | Mic AI Noise Cancellation |
| `MIC_AI_NOISE_CANCELLATION_SUBTEXT` | 抑制背景噪音，并提升语音交流 | Suppress background noise and enhance vocal communication. |
| `MIC_BOOST` | 麦克风增强 | MIC BOOST |
| `MIC_BOOST_LOWERCASE` | 麦克风增强 | mic boost |
| `MIC_ENHANCEMENT` | 麦克风增强 | Mic Enhancement |
| `MIC_ENHANCEMENT_HEADER` | 麦克风增强 | MIC ENHANCEMENTS |
| `MIC_ENHANCEMENT_TIP` | 调整麦克风对声音的处理方式。 | Adjust how the voice is processed from the microphone. |
| `MIC_ENHANCEMENT_TOOLTIP` | 通过以下方式优化麦克风输出： | Enhances the microphone output by: |
| `MIC_EQUALIZER` | 麦克风均衡器 | MIC EQUALIZER |
| `MIC_EQUALIZER_TOOLTIP` | 借助任何可用的预设或根据需要单独调整每个设置，从而对麦克风的音效进行自定义。 | Customize how your mic sounds using any of the available presets or by individually adjusting each setting as needed. |
| `MIC_EQUALIZER_TOOLTIP1` | 借助任何可用的预设或根据需要单独调整每个设置，从而对麦克风的音效进行自定义。<br><br>这些设置保存在设备上，并应用于所有配置文件。 | Customize how your mic sounds using any of the available presets or by individually adjusting each setting as needed.<br><br>These settings are saved to your device and are applied to all profiles. |
| `MIC_EQUALIZER_TOOLTIP_2` | 使用任何可用的预设来自定义麦克风的音效。 | Customize how your mic sounds using any of the available presets. |
| `MIC_GAIN` | 麦克风增益 | Mic Gain |
| `MIC_IN` | 麦克风输入 | MIC IN |
| `MIC_LOWER` | 麦克风 | Mic |
| `MIC_MONITOR` | 麦克风监听 | MIC monitor |
| `MIC_MONITORING` | 麦克风侧音电平 | Mic Monitoring |
| `MIC_MONITORING_HEADER` | 麦克风监听（侧音) | MIC MONITORING (SIDE TONE) |
| `MIC_MONITORING_SIDETONE` | 麦克风监听（侧音） | Mic Monitoring (Sidetone) |
| `MIC_MONITORING_TOOLTIP` | 使用麦克风监听功能可即时了解自己的语音的输出情况，以便知道是否需要调整说话音量、清晰度和节奏。 | Use Mic Monitoring to get instant feedback about your voice to help regulate your speech volume, clarity, and rhythm. |
| `MIC_MONITORING_UNAVAILABLE_STREAM_MIXER` | 启用直播混音器时，无法监听麦克风。 | Mic monitoring unavailable while Stream Mixer is enabled. |
| `MIC_MONITOR_TOOLTIP` | 如果想听一听你的声音在直播中呈现的音量，请启用麦克风监听。 你可以使用滑块调整监听音量。 | Enable Mic monitoring if you want to hear how loud your voice is in your stream. You can adjust the monitoring volume using the slider. |
| `MIC_MUTE` | 麦克风静音/取消静音 | Mic Mute/Unmute |
| `MIC_MUTE_BUTTON` | 麦克风静音键 | Mic Mute Button |
| `MIC_MUTE_OR_UNMUTE` | Mic Mute/Unmute | Mic Mute/Unmute |
| `MIC_NOISE_CANCELLATION` | 麦克风降噪 | Mic Noise Cancellation |
| `MIC_OUT100` | 麦克风输出已设置为 100%。 | Mic Output has been set to 100%. |
| `MIC_OUT80` | 麦克风监听已设置为 80%。 | Mic Monitoring has been set to 80%. |
| `MIC_PREVIEW` | 麦克风测试 | MIC PREVIEW |
| `MIC_PREVIEW_OFF_DESC` | 激活以测试麦克风的音频设置 | Activate to test your mic audio settings |
| `MIC_PREVIEW_ON_DESC` | 说话以测试麦克风的音频设置 | Speak to test your mic audio settings |
| `MIC_PREVIEW_TIP` | 调整麦克风的输入音量。<br><br>这些设置保存在本地，并应用于所有配置文件。 | Please make sure this device is set as your default input device. |
| `MIC_PREVIEW_TOOLTIP` | 请确保设备已设置为你的默认输入设备。 | Please make sure this device is set as your default input device. |
| `MIC_RECORDING_PLAYBACK` | 麦克风录音与播放 | MIC RECORDING & PLAYBACK |
| `MIC_SENSITIVITY_DESC` | 调整此设置可消除不需要的背景噪音或增加麦克风输出音量 | Adjust this setting to remove unwanted background noise or increase the amount of mic output heard |
| `MIC_SENSITIVITY_HEADER` | 麦克风灵敏度 | MIC SENSITIVITY |
| `MIC_SETUP` | 麦克风设置 | MIC Setup |
| `MIC_SETUP_DESC` | 麦克风设置模式允许你聆听和测试麦克风音频设置。 | Mic setup mode allows you to listen and test your mic audio settings. |
| `MIC_SETUP_INTRO_LINE1` | 此设置将对你的音频录制进行采样并调整设置，助你轻松开始使用麦克风。 | This setup will sample your audio recording and adjust settings to help you get started with using your mic. |
| `MIC_SETUP_INTRO_LINE2` | 录音由音频驱动程序处理，且永远不会上传至我们的服务器。 | Recordings are processed by the audio driver and are never uploaded to our servers. |
| `MIC_SETUP_MODE` | 麦克风设置模式 | MIC SETUP MODE |
| `MIC_SETUP_MUSIC` | 音乐 | MUSIC |
| `MIC_SETUP_SLIDE1_LINE1` | 按下麦克风按钮开始 10 秒录音。 | Press the Mic button to start a 10-second audio recording. |
| `MIC_SETUP_SLIDE1_LINE2` | 请用你平常说话的音量说话。<br>试着聊聊你昨天做了什么，或者你最后一顿吃了什么。 | Speak using your normal speaking voice.<br>Try talking about what you did yesterday or what your last meal was. |
| `MIC_SETUP_SLIDE2_LINE1` | 按下麦克风按钮开始 10 秒录音。<br>请保持安静，以便我们采集环境噪音，并避免任何突然动作。<br>系统将根据录音内容隔离噪音，从而生成清晰的音频。 | Press the Mic button to start a 10-second audio recording.<br>Remain silent while we capture your ambient noise, and avoid any sudden movements.<br>Based on the recording, the system will isolate the noise to help produce clean audio. |
| `MIC_SETUP_TITLE` | 麦克风设置 | MIC SETUP |
| `MIC_SETUP_TOOLTIP` | 选择你将使用的麦克风输入端口。 如果你的麦克风需要额外的外部电源才能运行，你也可以启用幻象电源。 | Select which mic input port you'll be using on the device. You may also enable Phantom Power if your microphone requires additional external power to operate. |
| `MIC_SETUP_TOOLTIP_WARNING` | 警告！ 使用此功能之前，请检查 XLR 麦克风是否支持 48V 幻象电源，以免损坏麦克风。 | WARNING! Before using this feature, please check if your XLR microphone supports 48V phantom power to avoid damaging your microphone. |
| `MIC_SIDETONE_TOOLTIP1` | 使用麦克风监听功能可即时了解自己的语音的输出情况，以便知道是否需要调整说话音量、清晰度和节奏。<br><br>这些设置保存在设备上，并应用于所有配置文件。 | Use Mic Monitoring to get instant feedback about your voice to help regulate your speech volume, clarity, and rhythm.<br><br>These settings are saved to your device and are applied to all profiles. |
| `MIC_VOLUME` | 麦克风音量 | MIC VOLUME |
| `MIC_VOLUME_DESC` | 调整或静音麦克风输入音量 | Adjust or mute the microphone input volume |
| `MIC_VOLUME_DOWN` | 降低麦克风音量 | Mic Volume Down |
| `MIC_VOLUME_UP` | 提高麦克风音量 | Mic Volume Up |
| `MIC_VOLUME_UP_DOWN` | 调高/降低麦克风音量 | Mic Volume Up/Down |
| `MONITORING` | 监听 | Monitoring |
| `NOISE_CANCELLATION_DESCRIPTION` | 启用降噪功能可抑制背景噪音，提升语音通信质量。 | Enabling noise cancellation will suppress background noise, enhancing vocal communication |
| `NOISE_GATE` | 噪声门 | Noise Gate |
| `NOISE_GATE_ATTACK_TOOLTIP` | 确定如果麦克风输入低于阈值，多快会打开噪声门。 | Determines how quickly the noise gate will be opened if the mic input registers below the threshold. |
| `NOISE_GATE_REDUCTION_TOOLTIP` | 麦克风输入低于阈值时静音或减少的音量。 | The amount of reduction or muting of sound if the mic input registers below the threshold. |
| `NOISE_GATE_RELEASE_TOOLTIP` | 确定多快会降低麦克风输入音量 | Determines how fast the mic input will be reduced. |
| `NOISE_GATE_THRESHOLD_TOOLTIP` | 设置噪声门何时打开。 任何低于阈值的声音都会降低音量或静音。 | Sets when the Noise Gate will open. Any sound that registers below the threshold will be reduced or muted. |
| `NOISE_GATE_TOOLTIP` | 控制麦克风输入量，以便过滤背景噪音，使你的声音更为清晰。 你可以从这里调整以下属性： | Control the amount of mic input to cut out background noise and completely isolate your voice. The following properties can be adjusted from here: |
| `SIDETONE` | 侧音 | sidetone |
| `SIDETONE_DESC` | 通过耳机直接监听来自麦克风没有经过优化的声音。 | Listen to your microphone directly through your headset without any enhancements. |
| `SIDETONE_DESC_2` | 启用后，麦克风将保持活跃状态，且设备不会进入无线节能模式。 | When enabled, the microphone will stay active and the device will not enter Wireless Power Saving mode. |
| `SIDETONE_HEADER` | 侧音 | SIDETONE |
| `SIDETONE_LEVEL_DOWN` | 调低侧音 | Sidetone down |
| `SIDETONE_LEVEL_UP` | 调高侧音 | Sidetone up |
| `SIDETONE_ON_OFF` | 打开/关闭侧音 | Sidetone on/off |
| `SIDETONE_TOOLTIP` | 通过耳机直接监听来自麦克风没有经过优化的声音。 | Listen to your microphone directly through your headset without any enhancements. |
| `SIDETONE_TOOLTIP_2` | 使用麦克风监听功能可即时了解自己的语音的输出情况，以便知道是否需要调整说话音量、清晰度和节奏。 | Use Mic Monitoring to get instant feedback about your voice to help regulate your speech volume, clarity, and rhythm. |
| `VOICE` | 人声 | Voice |
| `VOICE_CHANGER` | 变声器 | Voice changer |
| `VOICE_CHANGER_TOOLTIP` | 使用任何提供的变声器预设在麦克风输入上应用人声效果。 | Apply a vocal effect on the mic input using any of the provided voice changer presets. |
| `VOICE_CHAT` | 语音聊天 | voice chat |
| `VOICE_CHAT_TOOLTIP` | 语音聊天混音指你所有聊天应用程序的音频通道。 静音或调整此通道的音量并选择可以在此混音中听到哪些通道。 | The Voice Chat mix is the audio channel for all your chat applications. Mute or adjust how loud this channel will be and select which channels can be heard on this mix. |
| `VOICE_CLARITY_DESC_1` | 提高音质，让你的声音清晰而明亮。 | Improves the quality of audio for clear and easily understandable speech. |
| `VOICE_CLARITY_DESC_2` | 注意：低音电平会降低。 | Note: Bass levels will be reduced. |
| `VOICE_CLARITY_HEADER` | 人声清晰度 | VOICE CLARITY |
| `VOICE_CLARITY_TOOLTIP` | 增强通过通信应用程序拨入的通话的效果。 | Enhances incoming dialogue from communication applications. |
| `VOICE_GATE` | 噪声门 | VOICE GATE |
| `VOICE_GATE_DESC` | 控制麦克风输入的阈值。 | Control the threshold of microphone input. |
| `VOICE_GATE_DESC_2` | 若阈值较低，则可录入的声音更多 | A lower threshold will allow more sound to be registered. |
| `VOICE_GATE_TOOLTIP` | 调整麦克风输入音量。<br><br>噪声门负责控制麦克风的输入量。低于阈值的所有声音都会被减弱，让它不会被听到。 | Adjust the microphone input volume.<br><br>Voice gate controls the amount of microphone input. Any sound that registers below the threshold will be attenuated and will not be heard. |
## 4. 本项目的实现状态

| 项 | 内容 |
|---|---|
| 本项目的页面 | src/pages/calibration.rs —— 已实现（表面配置文件 / 校准状态机） |
| 后端方案 | 方案 2：复用雷云原生引擎（`lighting_driver` / `RzLightingEngineApi` / `mapping_engine` / `simple_service`） |
| 证据等级 | 布局 `[前端]`（设备模块 CSS）、文案 `[文案]`（语言包）、设备归属 `[前端]`（设备模块常量块） |

> 尚未实现的界面在应用里走占位页；占位页列出该页**真实存在的 key**，不编造内容。


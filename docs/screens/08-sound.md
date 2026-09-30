# 声音（TAB_SOUND）

> 本页由自动脚本从雷云**自己的模块**提取，未做臆测。
> 重新生成：`node .ref/tools/gen-screen-docs.js`

## 1. 出现在哪些设备上

| 设备 | productId | 设备类型 | 该设备标签页顺序 |
|---|---|---|---|
| RAZER KRAKEN BT SANRIO LIMITED EDITION | 777 | 耳机 | 第 5 个：自定义 · 灯光 · 校准 · 电源 · 声音 · 麦克风 |

标签页真实文案：**声音**（key `TAB_SOUND`）

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

该界面共 43 个布局类名，分 4 个分区。

**① 音量与输出**（4）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `description-volume-map` | `display:flex; flex-direction:column; height:160px; justify-content:space-between`<br>`height:15px; width:15px`<br>`align-items:center; display:flex; justify-content:flex-start`<br>`display:block; height:16px; width:16px` | `font-size:14px; line-height:17px`<br>`text-transform:uppercase`<br>`background-color:#0000`<br>`background-image:url(../../static/media/clockwise-gray.8b2c0b15.svg); background-position:50%; background-repeat:no-repeat; background-size:auto; line-height:17px`<br>`color:#999; line-height:17px` |
| `volume` | `align-items:center; display:flex`<br>`display:inline-block; height:20px; width:20px`<br>`left:100%; right:auto` | `background-image:url(../../static/media/icon_external_link_sprite.0b292d50.svg#link); background-position:50%; background-repeat:no-repeat; line-height:20px; transition:color .3s,opacity .3s`<br>`background-image:url(../../static/media/icon_external_link_sprite.0b292d50.svg#hover)` |
| `volume-item` | — | `color:#999` |
| `volume-title` | `align-items:center; display:flex; justify-content:flex-start`<br>`display:block; height:16px; width:16px` | `text-transform:uppercase`<br>`background-color:#0000`<br>`background-image:url(../../static/media/clockwise-gray.8b2c0b15.svg); background-position:50%; background-repeat:no-repeat; background-size:auto; line-height:17px`<br>`background-image:url(../../static/media/counter-clockwise-gray.a5d7275a.svg); background-position:50%; background-repeat:no-repeat; background-size:auto`<br>`background-image:url(../../static/media/icon_config_multidial_white.c56d5b56.svg); background-position:50%; background-repeat:no-repeat; background-size:cover` |

**② 均衡器（EQ）**（8）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `description-eq-map` | `display:flex; flex-direction:column; height:240px; justify-content:space-evenly`<br>`align-items:center; display:flex; justify-content:flex-start`<br>`display:block; height:11px; width:11px` | `font-size:14px; line-height:17px`<br>`text-transform:uppercase`<br>`background-color:#0000`<br>`background-image:url(../../static/media/double-press.4e4a3975.svg); background-position:50%; background-repeat:no-repeat; background-size:auto`<br>`background-image:url(../../static/media/press-and-hold.0c1d60a8.svg); background-position:50%; background-repeat:no-repeat; background-size:auto` |
| `key-require-macro-module` | `height:20px; position:absolute; width:20px` | `background-size:cover`<br>`background-image:url(../../static/media/require_macro_module_icon.20694b5c.svg)` |
| `key-require-synapse` | `height:20px; position:absolute; width:20px` | `background-image:url(../../static/media/require_synapse_icon.d5e656b0.svg)`<br>`background-size:cover` |
| `require-synape-icon` | `align-items:center; display:flex; height:32px; justify-content:center; position:absolute; top:0; width:32px` | `background-color:#008900; border:1px solid #5d5d5d; border-radius:50%`<br>`opacity:1`<br>`background-color:#fd8611` |
| `require-synapse` | `height:44px; position:relative; width:210px`<br>`align-items:center`<br>`flex:0 0 auto`<br>`height:24px; width:24px` | `border:1px solid #707070; border-radius:3px`<br>`background-color:initial; background-image:url(../../static/media/logo_synapse.bc241e5e.svg)!important; background-repeat:no-repeat!important; background-size:24px 24px!important`<br>`background-image:url(data:image/png`<br>`color:#707070; font-size:14px; line-height:14px`<br>`background-color:#4a4a4a; background-image:url(../../static/media/tooltip_questionmark.96138d2f.svg); background-repeat:no-repeat; border-radius:7px; transition:background-color .3s` |
| `require-synapse-message` | — | `color:#000` |
| `switch-eq-item` | `align-items:center; display:flex; gap:4px` | `color:#999`<br>`color:#999; line-height:17px` |
| `switch-eq-title` | `align-items:center; display:flex; justify-content:flex-start`<br>`display:block; height:11px; width:11px`<br>`height:15px; width:15px`<br>`align-items:center; display:flex; gap:4px` | `text-transform:uppercase`<br>`background-color:#0000`<br>`background-image:url(../../static/media/double-press.4e4a3975.svg); background-position:50%; background-repeat:no-repeat; background-size:auto`<br>`background-image:url(../../static/media/press-and-hold.0c1d60a8.svg); background-position:50%; background-repeat:no-repeat; background-size:auto` |

**③ 增强（THX / Dolby / 低音）**（10）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `text-thx-spatial` | — | `text-transform:none` |
| `thx-btn` | `display:inline-block; padding:.5rem 1.5rem; width:auto`<br>`display:block!important; height:27px; margin:20px auto auto; width:90px`<br>`max-width:90px; padding:7px 0 6px`<br>`display:block!important; height:27px; margin:auto` | `background-color:#44d62c; border-radius:3px; color:#000; text-align:center; text-transform:uppercase; transition:opacity .3s`<br>`opacity:.8`<br>`opacity:.6`<br>`font-size:12px; line-height:11px`<br>`border:1px solid #0000004d; font-size:12px; line-height:12px` |
| `thx-head` | — | `color:#44d62c; text-transform:uppercase` |
| `thx-logo` | `height:32px; width:80px` | `background-image:url(../../static/media/thx_logo.2a283c30.svg); background-position:50%; background-repeat:no-repeat` |
| `thx-main-title` | `max-height:none` | `color:#ccc; font-weight:400; text-transform:uppercase` |
| `thx-modal` | `height:auto; width:850px` | — |
| `thx-reset` | `width:80px` | — |
| `thx-spatial` | `height:44px; min-width:44px`<br>`max-width:100px` | `background-position:50%; background-repeat:no-repeat`<br>`background-image:url(../../static/media/icon_audio_enhancement_thx.e501de27.svg); background-size:100px` |
| `thx-spatial-info` | — | `color:#707070; font-size:12px; line-height:5px` |
| `thx-wrapper` | `position:relative`<br>`left:auto; right:15px; top:30px`<br>`left:auto; right:5px` | `border:1px solid #44d62c` |

**⑫ 其它**（21）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `AUDIO_FUNCTION` | — | `background-image:url(../../static/media/icon_config_audio_function.36f4e93d.svg)`<br>`background-image:url(../../static/media/icon_config_audio_function_a.5e839cd7.svg)` |
| `AudioMeter_flowButtonActive__vq7uT` | — | `background:#292929; border:1px solid #44d62c` |
| `AudioMeter_flowButtonIcon__p7D6V` | `height:18px; width:18px` | — |
| `AudioMeter_flowButton__4sIhG` | `align-items:center; display:flex; flex-direction:column; gap:5px; height:66px; justify-content:center; min-width:160px; padding:7px 16px 6px; width:160px`<br>`height:18px; width:18px` | `background:#111; border:1px solid #5d5d5d; border-radius:3px; color:#ccc; cursor:pointer; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; opacity:1; text-transform:uppercase; transition:all .2s ease` |
| `AudioMeter_flowButtons__tOGNz` | `display:flex; gap:5px` | — |
| `AudioMeter_flowSelectorContainer__e0jy-` | — | — |
| `MonitoringDashboard_popupHeader__TeQL4` | `align-items:center; justify-content:space-between; padding:11px 10px 9px; position:-webkit-sticky; position:sticky; top:0` | `background-color:#222; border-bottom:1px solid #333` |
| `audio` | `height:24px; left:0; width:24px`<br>`position:absolute` | `background-image:url(../../static/media/icon_audio_3.b1be5331.svg); background-position:4px; background-size:20px; transition:background-image .3s`<br>`background-repeat:no-repeat` |
| `audio-left` | `left:auto; position:static; top:auto` | `opacity:50%` |
| `audio-power-saving` | `min-width:78px` | — |
| `audio-right` | `left:auto; position:static; top:auto` | `opacity:50%` |
| `audio-tutorial` | `display:flex; padding:0 10px; position:relative`<br>`flex-grow:1`<br>`height:14px; position:absolute; right:12px; top:12px; width:14px`<br>`display:flex; flex-direction:column; height:230px; justify-content:space-evenly; overflow:auto` | `border:1px solid #44d62c`<br>`background:#111; border-radius:5px`<br>`background-color:#0000; background-image:url(../../static/media/close_icon_tutorial.7ec2b4df.svg); background-position:50%; background-repeat:no-repeat; background-size:cover; cursor:pointer; transition:background-color .2s`<br>`font-family:Roboto; font-size:20px; line-height:20px`<br>`color:#ccc; font-weight:400; text-transform:uppercase` |
| `audio-tutorial-wrapper` | `align-items:flex-start; display:flex; justify-content:center` | — |
| `audio-tutorial__desc` | `display:flex; flex-direction:column; justify-content:space-between; padding:50px 49px; width:480px` | — |
| `audio-tutorial__video` | `height:420px; width:320px` | — |
| `close-audio` | `height:14px; position:absolute; right:12px; top:12px; width:14px` | `background-color:#0000; background-image:url(../../static/media/close_icon_tutorial.7ec2b4df.svg); background-position:50%; background-repeat:no-repeat; background-size:cover; cursor:pointer; transition:background-color .2s` |
| `dot-audio` | `height:4px; width:4px` | `background-color:#707070; border-radius:10px`<br>`background-color:#fff` |
| `launch-sound-app` | — | `color:#fff` |
| `main-title-audio` | — | `font-family:Roboto; font-size:20px; line-height:20px`<br>`color:#ccc; font-weight:400; text-transform:uppercase` |
| `text-sound-properties` | — | `text-transform:none` |
| `title-audio` | — | `color:#44d62c; font-family:RazerF5; font-size:14px; font-weight:400; line-height:14px; text-transform:uppercase` |

## 3. 功能项

该界面对应的雷云文案 key，共 **264** 条（中文为雷云原文）：

| key | 中文 | English |
|---|---|---|
| `AUDIO` | 音频设备 | Audio |
| `AUDIO_DEVICE` | 音频设备 | Audio Device |
| `AUDIO_DEVICES` | 音频设备 | Audio Devices |
| `AUDIO_DRIVEN_DESCRIPTION` | Razer Sensa HD 可实时自动完成音频到触觉的转换，适用于所有游戏、电影和音乐。 | Razer Sensa HD works across all games, movies, and music with automatic audio-to-haptics conversion in real-time. |
| `AUDIO_DRIVEN_HAPTICS` | 由音频转化而成的触觉反馈 | Audio-driven haptics |
| `AUDIO_ENHANCEMENT_DOLBY_TOOLTIP` | 通过杜比虚拟音箱启用虚拟声音。 | Enables virtual sound through Dolby Virtual Speaker. |
| `AUDIO_ENHANCEMENT_HEADER` | 音效增强 | AUDIO ENHANCEMENT |
| `AUDIO_ENHANCEMENT_PRESET_TOOLTIP` | 对各种音频频率进行过滤，从而控制音频输出的整体声调。 | Filters various audio frequencies, controlling the overall tone of your audio output. |
| `AUDIO_ENHANCEMENT_THX_TOOLTIP` | 享受 THX 认证的沉浸式影音体验。 | Experience THX certified cinematic audio immersion. |
| `AUDIO_ENHANCEMENT_TIP` | 使用这些声音处理选项之一修改音频播放。 | Modify the audio playback using one of these sound processing options. |
| `AUDIO_ENHANCEMENT_TOOLTIP` | 选择音效增强模式 | Select the mode of audio enhancement |
| `AUDIO_EQUALIZER` | 音频均衡器 | AUDIO EQUALIZER |
| `AUDIO_EQ_TOOLTIP` | 对各种音频频率进行过滤，从而控制音频输出的整体声调。<br><br>所有电竞均衡器调整都会保存在耳机上。 <br><br>对于标准均衡器调整，只有自定义均衡器配置文件会保存在耳机上。 | Filters various audio frequencies, controlling the overall tone of your audio output.<br><br>All Esports EQ adjustments will be saved on the headset. <br><br>For Standard EQ adjustments, only the Custom EQ profile will be saved on the headset. |
| `AUDIO_FUNCTION` | 音频功能 | AUDIO FUNCTION |
| `AUDIO_FUNCTION_DESC` | 在切换音频设备时更改音频输出。 | Change the audio output as you toggle through sound devices |
| `AUDIO_FUNCTION_DESC_AT_LEAST_ONE_AUDIO_DEVICE` | 此功能需要至少一个支持 Razer Synapse 雷云的音频设备。 | At least one Razer Synapse enabled audio device is required for this feature |
| `AUDIO_FUNCTION_EQ_SAVED_TO_HEADSET_DES` | 所有均衡器自定义设置都保存在耳麦上，以便快速调用。 | All EQ customizations are stored on your headset for quick access. |
| `AUDIO_HEADER` | 音频 | AUDIO |
| `AUDIO_METER` | 音频计 | Audio Meter |
| `AUDIO_MIRRORING_TOOLTIP` | 将前置扬声器中的立体声内容镜像到后置扬声器，为你带来更响亮的音效<br><br>播放多声道音频内容时，请取消选中该选项。 | Mirror stereo content from your front speakers to the rear speakers, enveloping you in louder audio.<br><br>Uncheck this option when playing content with multichannel audio. |
| `AUDIO_MIX` | Audio Mix | Audio Mix |
| `AUDIO_MODE` | 音频模式 | Audio Mode |
| `AUDIO_MODES` | 音频模式 | AUDIO MODES |
| `AUDIO_MODE_BUTTON` | 切换至音频模式 | TOGGLE TO Audio MODE |
| `AUDIO_MODE_CAMERA_DISABLE_TOLLTIP` | 当前音频模式禁用了摄像头追踪。 | Camera tracking is disabled for the current audio mode. |
| `AUDIO_MODE_CAMERA_ENABLE_TOLLTIP` | 你当前处于摄像头的追踪范围内 | You are currently within the tracking range of the camera. |
| `AUDIO_MODE_CAMERA_ENABLE_TOLLTIP1` | 你当前不在摄像头的追踪范围内 | You are currently out of the tracking range of the camera. |
| `AUDIO_MODE_CAMERA_ENABLE_TOLLTIP2` | 当前音频模式启用了摄像头追踪，以根据你的位置传送声音 | Camera tracking is enabled for the current audio mode to beam sound according to your position. |
| `AUDIO_MODE_CONTENT` | 将控制盘设置为音频模式，开始控制所关联的 Razer 雷蛇音箱系统。 | Set the control pod to Audio mode to begin controlling your linked Razer speaker system. |
| `AUDIO_MODE_DESC` | 提供未经任何处理的原声音效。 | Delivers native sound without any processing. |
| `AUDIO_MODE_DESC2` | 音频直接传送到你的耳朵，提供传统耳机才有的精准声音定位，让你获得身临其境的音效体验。 | Audio is beamed directly to your ears to deliver an immersive soundscape with pinpoint positional audio traditionally found in headsets. |
| `AUDIO_MODE_DESC3` | 音频传送到 7 个虚拟扬声器，提供始终以你为中心的广阔声场。 | Audio is beamed to 7 virtual speakers to deliver a wide soundstage that is always centered around you. |
| `AUDIO_MODE_DESC4` | 音频的传送方式力求在更广的聆听区域为多位听众提供宽广而平衡的音效体验。 | Audio is beamed to provide a wide and balanced soundscape over an extended listening area for multi-listeners. |
| `AUDIO_MODE_POPUP_BUTTON` | Toggle to Audio mode | Toggle to Audio mode |
| `AUDIO_MODE_POPUP_CONTENT_1` | 切换至音频模式，以便直接控制已关联至控制盘的 Razer 雷蛇音箱系统。 | Switch to Audio mode to directly control your Razer speaker system(s) linked to the control pod. |
| `AUDIO_MODE_TIP` | 提示：你也可以按三下音源按钮 ( {{icon}} ) 来切换至音频模式。 | Tip: You can also toggle to Audio mode by pressing the Source button ( {{icon}} ) 3 times. |
| `AUDIO_MODE_TIP1` | 打赏: 是享受立体声或双声道音频内容的理想之选 | Tip: Ideal for content with a stereo or binaural audio source. |
| `AUDIO_MODE_TIP2` | 打赏: 是享受多声道音频（5.1 或 7.1）内容的理想之选。 | Tip: Ideal for content with a multi-channel audio source (5.1 or 7.1). |
| `AUDIO_MODE_TOOLTIP` | 为获得最佳聆听体验，请确保距离条形音箱 50 到 150 厘米，距离两侧 15 到 30 厘米。 | For the best listening experience, stay within 50 ⁠– ⁠150 cm from the soundbar and 15 ⁠– ⁠30 cm to each side |
| `AUDIO_MODE_TOOLTIP_2` | 将控制盘设置为音频模式，开始控制通过 Razer Audio 音频应用程序关联的 Razer 雷蛇音箱系统。 <br><br>确保至少有一个 Razer 雷蛇音箱系统已通过 Razer Audio 音频应用程序与控制盘关联。 | Set the control pod to Audio mode to begin controlling your Razer speaker system linked via the Razer Audio app. <br><br>Ensure at least one Razer speaker system has been linked to the control pod via the Razer Audio app. |
| `AUDIO_MONITORING` | 音频监控 | AUDIO MONITORING |
| `AUDIO_MONITORING_CHECK` | 对立体声内容进行音频镜像 | Audio Mirroring for Stereo Content |
| `AUDIO_MONITORING_DESC` | 调节或静音 3.5 毫米插孔的音频输出音量。 | Adjust or mute the audio output volume from the 3.5mm jack. |
| `AUDIO_MONITORING_NOTE` | 注意：这些设置不会影响你所采集之游戏的实际音频输出。 | Note: These settings will not affect actual audio output on your game captures. |
| `AUDIO_MONITORING_TOOLTIP` | 更改在不同设置下从 3.5 毫米插孔送出的音量。 | Change the volume of different settings from your 3.5mm jack. |
| `AUDIO_OUTPUT` | 音频输出 | Audio Output |
| `AUDIO_POWER_SAVING_DESC` | 以电池供电时，在无活动（分钟）后，设备将会关闭。 | Device will turn off after (mins) of inactivity when running on battery. |
| `AUDIO_POWER_SAVING_DESC_2` | 以电池供电时，设备将会关闭。 | Device will turn off when running on battery. |
| `AUDIO_POWER_SAVING_HEADER` | 节能 | POWER SAVING |
| `AUDIO_POWER_SAVING_TOOLTIP` | 调整设备闲置多久即关闭设备电源，以节省电池电量。 | Adjust the duration of inactivity before the device is powered off to conserve battery. |
| `AUDIO_POWER_SAVING_TOOLTIP_ADDITIONAL` | 当设备使用无线连接且不处于充电状态时，此节能功能可起作用。 | This power saving tool works when the device is using wireless connection and not being charged. |
| `AUDIO_PROMPTS` | 音频提示音 | Audio Prompts |
| `AUDIO_PROMPTS_CONTROL_DESC` | 新增了切换麦克风静音和取消静音音频提示的选项。 | Added the option to toggle audio prompts for mic mute and unmute. |
| `AUDIO_PROMPTS_DESC` | 接收声音通知，在有事件发生时提醒你 | Receive sound notifications to alert you whenever an event occurs. |
| `AUDIO_TO_HAPTICS` | 音频到触觉 | Audio-to-Haptics |
| `AUDIO_TO_HAPTICS_AUDIO_CUES` | 音频提示 | Audio Cues |
| `AUDIO_TO_HAPTICS_AUDIO_CUES_TIPS` | 根据特定音频条件确定何时激活触觉反馈。 | Determines when the haptic feedback will activate based on specific audio conditions. |
| `AUDIO_TO_HAPTICS_AUDIO_INPUT_RANGE` | 音频输入范围 | Audio Input Range |
| `AUDIO_TO_HAPTICS_AUDIO_INPUT_RANGE_TIPS` | 转换为触觉反馈的音频频率范围。 | The range of audio frequencies that are converted to haptic responses. |
| `AUDIO_TO_HAPTICS_BASS_TEXT_LOWER` | 很少触发语音触觉反馈（包括游戏内聊天）。 | Rarely triggers haptics for voice (including in-game chat). |
| `AUDIO_TO_HAPTICS_BASS_TEXT_UPPER` | 主要提供深沉低音的配置文件，具有类似低音炮的响应。旨在通过身体感受增强低音的临场感。 | Deep, bass-focused profile with a subwoofer-like response. Designed to enhance bass presence with physical sensations. |
| `AUDIO_TO_HAPTICS_CONTROLLED` | Deep, bass-focused profile with a subwoofer-like response. Designed to enhance bass presence with physical sensations. | Deep, bass-focused profile with a subwoofer-like response. Designed to enhance bass presence with physical sensations. |
| `AUDIO_TO_HAPTICS_CUSTOMIZE_TIPS` | 现有的自定义配置文件将被覆盖。 | The existing Custom profile will be over-written. |
| `AUDIO_TO_HAPTICS_CUSTOM_BASE_LABEL` | 进行自定义时基于 | Customize based on |
| `AUDIO_TO_HAPTICS_CUSTOM_TEXT_LOWER` | May slightly trigger haptics for voice (including in-game chat) | May slightly trigger haptics for voice (including in-game chat) |
| `AUDIO_TO_HAPTICS_CUSTOM_TEXT_UPPER` | 基于：{{preset}}预设。 | Based on: {{preset}} preset. |
| `AUDIO_TO_HAPTICS_DECSRIPTON` | 通过系统声音转换的触觉反馈感受声音 — 当游戏支持时，游戏内触觉反馈功能将自动接管。 | Feel the audio through haptics from your system's sound—in-game haptics take over when supported. |
| `AUDIO_TO_HAPTICS_DESC` | 使用预设或自定义预设，选择 Razer Sensa HD 触觉反馈技术如何将音频转换为触觉反馈。 | Select how Razer Sensa HD Haptics converts audio to haptics by either using presets or customizing your own. |
| `AUDIO_TO_HAPTICS_ENTERTAINMENT_TEXT_LOWER` | 可能会轻微触发语音触觉反馈（包括游戏内聊天）。 | May slightly trigger haptics for voice (including in-game chat). |
| `AUDIO_TO_HAPTICS_ENTERTAINMENT_TEXT_UPPER` | 提供沉浸感的平衡配置文件。经过优化，在温和场景中响应平稳，在高强度事件中响应强烈。 | Balanced profile with immersion. Optimized to have a smooth response for mild scenes, and strong responses for high-intensity events. |
| `AUDIO_TO_HAPTICS_GAIN_LEVEL` | 增益水平 (%) | Gain Level (%) |
| `AUDIO_TO_HAPTICS_GAME_PROFILES` | 音频到触觉游戏配置文件 | Audio-to-Haptics Game Profiles |
| `AUDIO_TO_HAPTICS_GAME_TEXT_LOWER` | 可能会经常触发语音触觉反馈（包括游戏内聊天）。 | Might often trigger haptics for voice (including in-game chat). |
| `AUDIO_TO_HAPTICS_GAME_TEXT_UPPER` | 响应灵敏的配置文件，可最大限度地增强沉浸感和空间感。充满动作情节的事件会让人感觉更加生动和真实。 | Reactive profile maximizing immersion and spatial awareness. Action-packed events will feel more vivid and present. |
| `AUDIO_TO_HAPTICS_HAPTIC_GAIN` | 触觉反馈增益 | Haptic Gain |
| `AUDIO_TO_HAPTICS_HAPTIC_GAIN_TIPS` | 所选音频频率内的振动强度。 | Vibration strength within the selected audio frequencies. |
| `AUDIO_TO_HAPTICS_MAX_FREQUENCY` | 最大值 (Hz) | Max. (Hz) |
| `AUDIO_TO_HAPTICS_MIN_FREQUENCY` | 最小值 (Hz) | Min. (Hz) |
| `AUDIO_TO_HAPTICS_MODAL_TITLE` | 音频到触觉 — 自定义配置文件 | AUDIO-TO-HAPTICS - CUSTOM PROFILE |
| `AUDIO_TO_HAPTICS_NO_AUDIO_INDICATOR` | 通过播放媒体体验触觉反馈。 | Experience haptics by playing media. |
| `AUDIO_TO_HAPTICS_ONLY` | 仅音频到触觉 | Audio-to-Haptics only |
| `AUDIO_TO_HAPTICS_PEAK_METER_TOOLTIP` | 峰值表 | Peak Meter |
| `AUDIO_TO_HAPTICS_PROFILES` | 音频到触觉配置文件 | Audio-to-Haptics Profiles |
| `AUDIO_TO_HAPTICS_SUB_DECSRIPTON` | 系统音频 | System Audio |
| `AUDIO_TO_HAPTICS_TIPS` | 过滤出特定范围的音频频率，并将其转化为触觉反馈。 | Filters out specific range of audio frequencies and turns them into haptic feedback. |
| `AUDIO_TO_HAPTICS_TITLE` | 音频到触觉配置文件 | Audio-To-Haptics Profile |
| `AUDIO_TO_HAPTICS_VOICE` | 很少触发语音触觉反馈（包括游戏内聊天）。 | Rarely triggers haptics for voice (including in-game chat). |
| `AUDIO_TO_INPUT_GUIDE` | 如何将应用程序音频导入至输入通道 | How to route application audio to input channel |
| `AUDIO_TROUBLESHOOTING` | 音频故障排除 | AUDIO TROUBLESHOOTING |
| `AUDIO_TROUBLESHOOTING_BUTTON` | 重启音频设备 | RESTART AUDIO DEVICES |
| `AUDIO_TROUBLESHOOTING_DES` | 如果遇到声音或音频问题，请重启系统的音频驱动程序。 | Restart your system's audio drivers if you're having sound or audio problems. |
| `BASS` | 低音 | BASS |
| `BASS_BOOST` | 低音增强 | BASS BOOST |
| `BASS_BOOST_DESC` | 增强设备的低频音。 | Enhances your device's low-frequency sounds. |
| `BASS_BOOST_HEADER` | 低音增强 | BASS BOOST |
| `BASS_BOOST_TOOLTIP` | 增强耳麦的低频响应。 | Enhances the low frequency response of your headset. |
| `BASS_BOOST_TOOLTIP_NOMMO` | 增强低频响应以提高低音输出 | Enhances the low frequency response to improve the bass output |
| `DOLBY_GAME` | 杜比游戏 | Dolby Game |
| `DOLBY_MOVIE` | 杜比电影 | Dolby Movie |
| `DOLBY_MUSIC` | 杜比音乐 | Dolby Music |
| `ENHANCEMENTS_AMBIENT_NOISE_REEDUCTION_TOOLTIP` | 麦克风主动式消噪功能使用隐蔽的麦克风来消除环境噪音，确保清晰的通话效果。 | Change the intensity of the microphone's noise cancellation to enhance voice pickup. |
| `ENHANCEMENTS_DISABLED_MODE_BENEFIT` | 禁用时，可享受游戏级的低延迟、更大的有效范围和更长的电池续航时间。 | When disabled, enjoy gaming grade latency, extended range, and longer battery life. |
| `ENHANCEMENTS_HIGHSPEED_AUDIO_DESCRIPTION` | 通过第二代 Razer HyperSpeed 无线接收器连接，体验高速无线音频。 | Experience high-speed wireless audio while connected via the Razer HyperSpeed Wireless Gen-2 dongle. |
| `ENHANCEMENTS_IN_CALL_AUDIO_MIX_DESCRIPTION` | 选择在连接到 2.4GHz 模式时如何处理通过蓝牙模式接收到的移动设备来电。 | Select what happens for incoming calls from your mobile device via Bluetooth Mode while connected to 2.4GHz mode. |
| `ENHANCEMENTS_IN_CALL_AUDIO_MIX_NOTE` | 注意：只有同时连接蓝牙模式和 2.4GHz 模式时，才能使用此功能。 | Note: This feature is only available when simultaneously connected to both Bluetooth and 2.4GHz modes. |
| `ENHANCEMENTS_IN_CALL_AUDIO_MIX_TITLE` | 来电音频混合 | In-Call Audio Mix |
| `ENHANCEMENTS_IN_CALL_AUDIO_OPTION_COMBINE` | 结合 2.4GHz 和蓝牙 | Combine 2.4GHz and Bluetooth |
| `ENHANCEMENTS_IN_CALL_AUDIO_OPTION_LOWER` | 降低 2.4GHz 音量 | Lower 2.4GHz volume |
| `ENHANCEMENTS_IN_CALL_AUDIO_OPTION_MUTE` | 静音 2.4GHz | Mute 2.4GHz volume |
| `ENHANCEMENTS_LOW_LATENCY_BENEFIT` | 超低延迟提供极快的响应时间，低至约 10 毫秒。 | Ultra-Low Latency offers the fastest response time, as low as ~10ms. |
| `ENHANCEMENTS_SETTINGS_SAVED` | 这些设置保存在设备上，并应用于所有配置文件。 | These settings are saved to your device and are applied to all profiles. |
| `ENHANCEMENTS_SIMULTANEOUS_AUDIO_DESCRIPTION` | 在同步音频模式下使用本设备时，可同时收听蓝牙和 2.4GHz 音频连接。 | Listen to both Bluetooth and 2.4GHz audio connections at the same time when using this device in Simultaneous Audio mode. |
| `ENHANCEMENTS_SMARTSWITCH_TIP` | 提示：要在 HyperSpeed Wireless 无线模式、蓝牙模式和同步音频模式之间切换，请按两次设备上的 SmartSwitch 智能切换按键。 | Tip: To switch between HyperSpeed Wireless, Bluetooth, and Simultaneous Audio modes, double-press the SmartSwitch button on your device. |
| `ENHANCEMENTS_ULTRA_LOW_LATENCY_TITLE` | 超低延迟 | Ultra-low Latency |
| `ENHANCEMENTS_VOCAL_CLARITY_TOOLTIP` | “亮丽人声”可隔离并提高麦克风人声音域的音量。 | Vocal Clarity isolates and increases the volume of the vocal range for your microphone. |
| `ENHANCEMENTS_VOLUME_NORMALIZATION_TOOLTIP` | “音量标准化”通过减少突然的音量增高并增加柔和的音频，来确保一致的输出音量。 | Volume Normalization ensures consistent output levels by reducing sudden loud noises, and increasing soft audio. |
| `ENHANCEMENT_HEADER` | 增强 | ENHANCEMENTS |
| `EQUALIZER_ADJUSTMENTS` | 均衡器调整 | EQUALIZER ADJUSTMENTS |
| `EQUALIZER_ADJUSTMENTS_DESC` | 使用这些均衡器来平衡前后频率，以获得最佳的聆听体验。 | Use these equalizers to balance the front and rear frequencies for the optimal listening experience. |
| `EQUALIZER_ADJUSTMENTS_TOOLTIP` | 微调前置和后置扬声器的低频、中频和高频，使音频更加清晰。 | Fine-tune the Low, Mid, and High frequencies of the front and rear speakers for audio clarity. |
| `EQUALIZER_ADJUSTMENTS_TOOLTIP_1` | 微调前置和后置扬声器的低频、中频和高频，使音频更加清晰。这将覆盖现有的均衡器设置。 | Fine-tune the Low, Mid, and High frequencies of the front and rear speakers for audio clarity. This will be applied on top of existing EQ settings. |
| `EQUALIZER_ADJUSTMENTS_TOOLTIP_2` | 提示：如果听到任何失真，可根据音频内容，在此处或音频均衡器中降低均衡器电平。 | Tip: Depending on the audio content, reduce EQ levels here or in the Audio Equalizer if you hear any distortion. |
| `EQUALIZER_HEADER` | 均衡器 | EQUALIZER |
| `EQUALIZER_TOOLTIP` | 对各种音频频率进行过滤，从而控制音频输出的整体声调。<br><br>所有电竞均衡器调整都会保存在耳机上。 <br><br>对于标准均衡器调整，只有自定义均衡器配置文件会保存在耳机上。 | Filters various audio frequencies, controlling the overall tone of your audio output.<br><br>All Esports EQ adjustments will be saved on the headset.<br><br>For Standard EQ adjustments, only the Custom EQ profile will be saved on the headset. |
| `EQUALIZER_TOOLTIP_1` | 对各种音频频率进行过滤，从而控制音频输出的整体声调。<br><br>对于均衡器调整，只有自定义均衡器配置文件会保存在耳机上。 | Filters various audio frequencies, controlling the overall tone of your audio output.<br><br>Only Stereo EQ profiles will be saved to the headset. |
| `EQUALIZER_TOOLTIP_2` | 对各种音频频率进行过滤，从而控制音频输出的整体声调。 | Filters various audio frequencies, controlling the overall tone of your audio output. |
| `EQ_BAND` | 均衡器频段 | EQ Band |
| `EQ_BAND_TOOLTIP` | 选择其他均衡器频段进行调节。可用的均衡器设置会根据你选择的频段自动更新。 | Select different EQ bands to adjust them. The available EQ settings update automatically based on the band you choose. |
| `EQ_EFFECT` | 均衡器效果 | EQ Effect |
| `EQ_EFFECT_TOOLTIP` | 应用并调整嘶声消除、人声低音增强、人声激励器及高通滤波器的均衡效果属性。<br><br>嘶声消除器通过柔化尖锐的"嘶"声，使人声听起来更悦耳。<br><br>人声低音增强效果能使人声听起来更深沉、更饱满，同时不会听起来过于响亮。<br><br>人声激励器能使人声听起来更明亮、更清晰、更生动。<br><br>高通滤波器可消除低频噪声，使声音保持清晰和凝聚。 | Apply and adjust EQ Effects properties for De-esser, Vocal Bass, and Vocal Exciter and High Pass Filter.<br><br>De-esser makes vocals sound smoother by softening sharp "sss" sounds.<br><br>Vocal Bass makes vocals sound deeper and fuller without being louder.<br><br>Vocal Exciter makes vocals sound brighter, clearer, and more lively.<br><br>High Pass Filter removes low-frequency noise so the audio stays clear and focused. |
| `EQ_FREQUENCY` | 频率 | FREQUENCY |
| `EQ_GAIN` | 增益 | Gain |
| `EQ_LIBRARY` | 均衡器库 | EQ Library |
| `EQ_LIBRARY_SEARCH_RESULTS` | 均衡器库 - 搜索结果 | EQ Library - Search Results |
| `EQ_PRESET_ARENA` | 竞技场 | Arena |
| `EQ_PRESET_CUSTOM` | 自定义 | Custom |
| `EQ_PRESET_EDIT` | 添加、删除、切换、重新排列和/或浏览耳麦的均衡器预设。 | Add, remove, swap, rearrange, and/or browse your headset's EQ presets. |
| `EQ_PRESET_FPS_CALLOUT` | FPS 喊话 | FPS-Callout |
| `EQ_PRESET_PODCAST` | 播客 | Podcast |
| `EQ_PRESET_STORAGE_DESC` | 耳麦上最多可存储 9 个均衡器预设，其中至少有一个标准均衡器（默认、游戏、电影或音乐）。拖动可对均衡器预设重新排序。 | Store up to 9 EQ presets on your headset, with at least 1 standard EQ (Default, Game, Movie, or Music). Drag to reorder EQ presets. |
| `EQ_PRESET_STUDIO` | 录音室 | Studio |
| `EQ_PRESET_THX_WARNING` | 在 THX Spatial Audio 空间音效模式下，无法使用电竞均衡器预设。 | Esports EQ presets are not available in THX Spatial Audio mode. |
| `EQ_PROPERTIES` | 均衡器属性 | EQ Properties |
| `EQ_PROPERTIES_FILTER_TYPE_TOOLTIP` | 选择要应用于特定均衡器频段的滤波器类型。点击了解每种滤波器类型的详细信息。 | Select a filter type to apply to the specific EQ band. Click to learn more about each filter type. |
| `EQ_PROPERTIES_FREQUENCY_TOOLTIP` | 选择要调整哪一部分的声音。 | Select which part of the sound you are adjusting. |
| `EQ_PROPERTIES_GAIN_TOOLTIP` | 控制所选频率的提升或衰减程度，从而突出或抑制声音的特定部分。 | Controls how much a selected frequency is boosted or reduced to emphasize or tame specific parts of the sound. |
| `EQ_PROPERTIES_Q_FACTOR_TOOLTIP` | 控制频率调整的宽度或窄度。<br><br>低 Q 值会影响宽广的频率范围，使声音变化显得平滑且不明显。<br>高 Q 值仅影响极窄的频率范围，使你能够精准定位或消除特定的声音。 | Controls how wide or narrow a frequency adjustment is.<br><br>A low Q affects a wide range of frequencies, making changes sound smooth and subtle.<br>A high Q affects a very narrow range, letting you precisely target or remove specific problem sounds. |
| `EQ_PROPERTIES_TOOLTIP` | 调整特定均衡器频段的属性。<br><br>你可以选择并调节特定频段，并应用滤波器、增益和 Q 值。 | Adjust the properties of a specific EQ Band.<br><br>You can select and tune specific frequencies and apply filters, gain and q-factor. |
| `EQ_REVERT_ON_SWITCH` | 如果切换到不同的均衡器预设，对当前均衡器预设的自定义设置将恢复为默认设置。 | Customizations to your current EQ preset will revert to default settings if it is switched to a different EQ preset. |
| `MIXER_DEVICE_INIT_STATUS_FAILED_DES` | 无法加载音频驱动程序。我们建议你拔出设备再重新插上。 | Unable to load Audio Drivers. We recommended removing and re-plugging the device. |
| `PRESET` | 预设 | PRESET |
| `PRESETCALIBRATION` | 预设校准 | Preset Calibration |
| `PRESETS_LABEL` | 预设 | Presets |
| `PRESET_PROFILE` | 预设配置文件 | PRESET PROFILE |
| `PRESET_PROFILES_CONTENT_1_BODY` | 此配置文件包含光学键盘的基本功能。它无法重新映射，且触发点和快速触发灵敏度不可调整。 | This is the basic functionality of an optical keyboard. This profile cannot be remapped, and the actuation point and rapid trigger sensitivity are not adjustable. |
| `PRESET_PROFILES_CONTENT_1_BODY_V2` | 此配置文件不可重新映射，其触发点和快速触发灵敏度也无法自定义。 | This profile is non-remappable, and its actuation point and rapid trigger sensitivity are non-customizable. |
| `PRESET_PROFILES_CONTENT_1_HEAD` | 出厂默认（热键：fn + home） | Factory Default (Hotkey: fn + home) |
| `PRESET_PROFILES_CONTENT_1_HEAD_V2` | 出厂默认（热键：fn + ;) | Factory Default (Hotkey: fn + ;) |
| `PRESET_PROFILES_CONTENT_2_BODY` | 快速进入 Synapse 雷云上最后使用/已配置的配置文件。 | Quickly go to the last-used/configured profile on Synapse. |
| `PRESET_PROFILES_CONTENT_2_HEAD` | 上次使用的 Synapse 雷云配置文件（热键：fn + ins） | Last-used Synapse Profile (Hotkey: fn + ins) |
| `PRESET_PROFILES_CONTENT_2_HEAD_V2` | 上次使用的 Synapse 雷云配置文件（热键：fn + home) | Last-used Synapse Profile (Hotkey: fn + home) |
| `PRESET_PROFILES_CONTENT_2_HEAD_V3` | 上次使用的 Synapse 雷云配置文件 | Last-used Synapse Profile |
| `PRESET_PROFILES_CONTENT_3_BODY` | 如果你需要高灵敏度键盘，可以考虑此配置文件，因为其 WASD 键设置为 1.2 毫米的触发点和 0.3 毫米的灵敏度。此配置文件非常适合用于畅玩需要快速响应各种移动的游戏。 | If you require a high-sensitivity keyboard, all the keys on this profile are set to 1.2mm actuation point and 0.3 mm sensitivity. This profile is ideal for games that require responsive movement. |
| `PRESET_PROFILES_CONTENT_3_HEAD` | FPS 游戏快速触发（热键：fn + pg up） | FPS Rapid Trigger (Hotkey: fn + pg up) |
| `PRESET_PROFILES_CONTENT_3_HEAD_V2` | FPS 游戏快速触发 | FPS Rapid Trigger |
| `PRESET_PROFILES_CONTENT_4_BODY` | 此配置文件让你可以像使用手柄的左模拟摇杆一样使用 WASD 键；其触发点设置为 1.2 毫米，因此可在游戏中实现更流畅的移动。 | This profile allows you to use the WASD keys like a controller's left analog stick; ideal for smoother in-game movement with the actuation point set to 1.2mm. |
| `PRESET_PROFILES_CONTENT_4_HEAD` | 模拟 WASD（热键：fn + del） | Analog WASD (Hotkey: fn + del) |
| `PRESET_PROFILES_CONTENT_4_HEAD_V2` | 模拟 WASD | Analog WASD |
| `PRESET_PROFILES_CONTENT_5_BODY` | 此配置文件将 WASD 键分别重新映射为实现加速、左转、减速/倒车，以及右转的操作。模拟控制按钮和 1.2 毫米的触发点，让你在畅玩竞速游戏时可以实现精准的移动。 | This profile remaps the WASD keys to Accelerate, Turn Left, Break/Reverse, and Turn Right respectively. Enjoy precision driving with analog controls and an actuation point of 1.2mm. |
| `PRESET_PROFILES_CONTENT_5_HEAD` | 竞速游戏（热键：fn + end） | Racing (Hotkey: fn + end) |
| `PRESET_PROFILES_CONTENT_5_HEAD_V2` | 竞速游戏 | Racing |
| `PRESET_PROFILES_CONTENT_6_BODY` | 如果你需要一款反应速度较高的键盘，可以考虑此配置文件，因为其 WASD 键设置为 0.8 毫米的触发点和 0.3 毫米的灵敏度。此配置文件非常适合用于畅玩要求极快反应速度的游戏。 | If you need a highly reactive keyboard, all the keys on this profile are set to 0.8 mm actuation point and 0.3 mm sensitivity. This profile is ideal for games that require extreme reaction times. |
| `PRESET_PROFILES_CONTENT_6_BODY_V2` | 默认配置文件，旨在反映标准键盘操作，同时允许完全自定义。所有按键均设置为 2.0 毫米触发点，并禁用快速触发功能。 | Default profile designed to reflect standard keyboard behavior while allowing full customization. All keys are set to 2.0 mm actuation point with Rapid Trigger disabled. |
| `PRESET_PROFILES_CONTENT_6_HEAD` | 高灵敏度（热键：fn + pg dn） | High Sensitivity (Hotkey: fn + pg dn) |
| `PRESET_PROFILES_CONTENT_6_HEAD_V2` | 常规（热键：fn + pg dn） | General (Hotkey: fn + pg dn) |
| `PRESET_PROFILES_CONTENT_6_HEAD_V3` | 高灵敏度 | High Sensitivity |
| `PRESET_PROFILES_LOADING_TEXT` | Loading all presets | Loading all presets |
| `PRESET_PROFILES_MAIN_TITLE` | 预设配置文件 | PRESET PROFILES |
| `PRESET_PROFILES_MINI_CONTENT_0_BODY` | 循环切换配置文件（热键：fn + {{keyA}}）.<br>在出厂默认配置文件和上次使用的配置文件之间切换（热键：fn + {{keyB}}）. | Cycle through profiles (Hotkey: Fn + {{keyA}}).<br>Toggle between Factory and Last-used Profile (Hotkey: fn + {{keyB}}). |
| `PRESET_PROFILES_MINI_CONTENT_1_HEAD` | 出厂默认 | Factory Default |
| `PRESET_PROFILES_MINI_CONTENT_2_HEAD` | 上次使用的 Synapse 雷云配置文件 | Last-used Synapse Profile |
| `PRESET_PROFILES_MINI_CONTENT_3_HEAD` | FPS 游戏快速触发 | FPS Rapid Trigger |
| `PRESET_PROFILES_MINI_CONTENT_4_HEAD` | 模拟 WASD | Analog WASD |
| `PRESET_PROFILES_MINI_CONTENT_5_HEAD` | 竞速游戏 | Racing |
| `PRESET_PROFILES_MINI_CONTENT_6_HEAD` | 高灵敏度 | High Sensitivity |
| `PRESET_PROFILES_TITLE` | 你的首选模式 | Your Top Go-to Modes |
| `PRESET_SHORTCUT` | 预设快捷键 | PRESET SHORTCUT |
| `PRESET_USAGE` | ({{presetCount}} 个，共 {{maxPresets}} 个) | ({{presetCount}} out of {{maxPresets}}) |
| `SOUNDBAR` | 条形音箱 | Soundbar |
| `SOUNDBAR_DISABLED_TIP` | 切换至条形音箱输出以启用此设置。 | Toggle to Soundbar output to enable this setting. |
| `SOUNDBAR_HEADSET_DESC` | 在条形音箱和连接到其耳机端口的耳机之间切换。 | Switch between the soundbar and the headset connected to its headset port. |
| `SOUNDBAR_HEADSET_DISABLED_TIP` | 将耳机连接至条形音箱以启用此设置。 | Connect a headset to the soundbar to enable this setting. |
| `SOUNDBAR_HEADSET_TIP` | 打赏: 你也可以通过按两次音源按键，在条形音箱和所连的耳机之间切换。 | Tip: You can also toggle between the soundbar and the connected headset by double-pressing the Source button |
| `SOUNDBAR_HEADSET_TITLE` | 条形音箱 / 耳麦切换 | SOUNDBAR / HEADSET TOGGLE |
| `SOUNDBAR_HEADSET_TOOLTIP` | 快速切换到已连接至条形音箱的耳机。 此功能仅在有耳机插入到耳机端口时才可用。 | Quickly switch to the headset connected to your soundbar. This feature is only available when a headset is plugged into the headset port. |
| `SOUND_EFFECTS` | 声效 |  |
| `SOUND_NORMALIZATION_DESC` | 放大较细微的声音，如脚步声和轻柔的对话声。 | Amplifies quiet sounds such as footsteps and soft dialogues. |
| `SOUND_NORMALIZATION_HEADER` | 声音标准化 | SOUND NORMALIZATION |
| `SOUND_NORMALIZATION_TOOLTIP` | 使用带增益补偿的单频段动态范围压缩器。声音标准化会在不影响较响亮声音的同时，放大较细微的声音。 | Using a single-band dynamic range compressor with makeup gain. Sound Normalization amplifies quiet sounds while louder sounds remain unaffected. |
| `SOUND_PROPERTIES` | 声音属性 | SOUND PROPERTIES |
| `SOUND_PROPERTIES_DESC` | 选择未连接设备时，THX Spatial Audio 空间音效会回退到的默认输出设备。 | Select a default output device that THX Spatial Audio will fallback to when device is not connected. |
| `SOUND_PROPERTIES_TOOLTIP` | 打开 Windows 声音属性窗口。 | Launch Windows sound properties window. |
| `SOUND_SETTINGS` | 声音设置 | Sound settings |
| `THX_AUDIO_PLAYBACK` | 音频播放 | AUDIO PLAYBACK |
| `THX_AUDIO_PLAYBACK_DESC` | 将“{{deviceName}}”设置为默认音频设备。 | Set ‘{{deviceName}}’ as the default audio device. |
| `THX_AUDIO_SPATIAL_DESC` | 在“声音”选项卡中，在立体声和 THX Spatial Audio 空间音效之间切换。 | Switch between Stereo and THX Spatial Audio in the Sound tab |
| `THX_CALIBRATION_TOOLTIP` | 自定义空间音效引擎，确保最符合你的个人收听偏好。 | Customize the spatial audio engine to best fit your personal listening preferences. |
| `THX_CINEMA_DESC` | 调整在明亮房间时的色彩，以获得更佳的观看体验。 | Tunes the colors for viewing in a well-lit room. |
| `THX_CINEMA_HEADER` | THX 影院: 亮室 | THX CINEMA: BRIGHT ROOM |
| `THX_CINEMA_TOOLTIP` | 亮室选项会增强色彩，减少人工/自然光下色彩不够鲜艳的问题。<br><br>开启“THX 影院”时，某些设置将不可用，因为这些设置将会被自动控制。 | Bright Room enhances the colors to look less washed out in artificial/natural light.<br><br>When THX Cinema is switched on, certain settings will not be available as they are automatically controlled. |
| `THX_CLOSE` | Close | Close |
| `THX_COMP_ENV_PROFILES` | 竞技模式和环境模式配置文件 | Competitive and Environmental profiles |
| `THX_COMP_ENV_PROFILES_DESC` | 在你畅玩游戏时，系统会根据游戏的类型，自动加载相应的 THX 游戏配置文件。 | THX Game Profiles will be loaded automatically for compatible games as you play. |
| `THX_CUSTOMIZABLE_PROFILES` | 可自定义的配置文件 | Customizable Profiles |
| `THX_CUSTOMIZABLE_PROFILES_DESC` | 选择现有的均衡器预设，或为每个均衡器配置文件自定义专有的设置。每个配置文件都可以轻松重置为默认值。 | Select an existing EQ preset or customize your own for each EQ profile. Each profile can easily be reset to default. |
| `THX_CUSTOM_AUDIO_EQ` | 自定义音频均衡器 | Custom Audio EQ |
| `THX_CUSTOM_AUDIO_EQ_DESC` | 除了 THX 游戏配置文件之外，你还可以设置偏好的音频均衡器配置文件，让系统在你玩游戏时自动加载此配置文件。 | Aside from THX Game Profiles, you can also set your preferred audio EQ profile to load automatically when a game is in the foreground. |
| `THX_ENGINE` | THX 引擎 | THX ENGINE |
| `THX_FREQUENCY_BAND_LABELS` | 频段标签 | Frequency Band Labels |
| `THX_FREQUENCY_BAND_LABELS_DESC` | 调整频段时请参考频段标签，以进一步提升音效体验。 | Refer to the band labels when adjusting frequency bands to refine your sound experience further. |
| `THX_GAME_MODE` | 支持 THX 游戏模式。 | THX Game Modes supported. |
| `THX_GAME_PROFILE` | THX Game Profile | THX Game Profile |
| `THX_GAME_PROFILES` | THX 游戏配置文件 | THX Game Profiles |
| `THX_IMPORTANT` | 特别注意 | IMPORTANT |
| `THX_IMPORTANT_L1` | 如果尚未重启电脑，请重启。 | Please restart your computer if you haven't yet. |
| `THX_IMPORTANT_L2` | 将“{{deviceName}}”设置为默认音频设备。你的产品现已集成了 THX Spatial Audio 空间音效，因此你无需再因为此功能单独配置一个音频设备。 | Set ‘{{deviceName}}’ as the default audio device, THX Spatial Audio has now been integrated into your respective product and will no longer need to be a separate audio device. |
| `THX_IMPORTANT_L3` | THX Spatial Audio 空间音效引擎实现了诸多改进，因此旧引擎的设置无法移植过来并已重置。如果你已更新到最新版的引擎，则设置会保持不变。 | With the improvements on the THX Spatial Audio engine, the older engine's settings can't be ported over and have been reset. Your settings remain unchanged if you've already updated to the latest engine. |
| `THX_IN_USE` | THX Spatial Audio 空间音效目前正用于 {{device}}  | THX Spatial Audio is currently in use for {{device}}  |
| `THX_POSITIONAL_CALIBRATION` | 位置校准 | Positional Calibration |
| `THX_POSITIONAL_CALIBRATION_DESC` | 根据你的聆听偏好自定义虚拟扬声器，以提升 THX Spatial Audio 空间音效的效果。 | Customize the virtual speakers based on your listening preferences to enhance your THX Spatial Audio experience. |
| `THX_SPATIAL_AUDIO` | THX SPATIAL AUDIO 空间音效 | THX SPATIAL AUDIO |
| `THX_SPATIAL_AUDIO_CALIBRATOR` | 添加了空间音效校准功能。 | Spatial Audio Calibrator |
| `THX_SPATIAL_AUDIO_DEMO` | THX SPATIAL AUDIO 空间音效演示 | THX SPATIAL AUDIO DEMO |
| `THX_SPATIAL_AUDIO_DESC` | 你的全新 Razer 雷蛇音频设备能为你带来全新境界的沉浸式体验以及更强大的声音定位功能。 | With your new Razer audio device comes a new level of immersion with greater audio positioning. |
| `THX_SPATIAL_AUDIO_DESC_LINE2` | 安装 THX Spatial Audio 空间音效并使用代码进行激活。 | Install THX Spatial Audio and activate it with your code. |
| `THX_SPATIAL_AUDIO_DOWNLOAD` | 下载 THX SPATIAL AUDIO 空间音效 | DOWNLOAD THX SPATIAL AUDIO |
| `THX_SPATIAL_AUDIO_FOR_PC` | THX™ Spatial Audio 空间音效（电脑） | THX™ Spatial Audio for PC |
| `THX_SPATIAL_AUDIO_LAUNCH` | 启用 THX SPATIAL AUDIO 空间音效 | LAUNCH THX SPATIAL AUDIO |
| `THX_SPATIAL_AUDIO_NOTICE` | 要获得最佳的 THX Spatial Audio 空间音效体验，请确保关闭此设备的 Windows Sonic 音效。 | For the best experience of THX Spatial Audio, ensure Windows Sonic is switched off for this device. |
| `THX_SPATIAL_AUDIO_TOOLTIP` | 启用 THX Spatial Audio 空间音效以手动更改每个应用程序的音频输出 | Enable THX Spatial Audio to manually change the audio output for each application |
| `THX_SPATIAL_AUDIO_WARNING_TOOLTIP` | 建议的操作 | Actions recommended |
| `THX_STANDALONE_APP_DESC` | 如要在 Synapse 雷云中配置音频设置，请退出独立应用程序。 | To configure audio settings in Synapse, exit the standalone application. |
| `THX_STANDALONE_APP_HEADER` | 通过 THX SPATIAL AUDIO 空间音效客户端管理的设置 | SETTINGS MANAGE BY THX SPATIAL AUDIO CLIENT |
| `THX_STEREO` | THX 立体声 | THX Stereo |
| `THX_TEXT_WARNING_TOOLTIP` | 要使增强功能在输出设备上发挥作用，必须将 THX Spatial Audio 空间音效选为默认的系统播放设备。单击按钮即可再次查看“设置”。 | In order for the enhancements to take effect on your output device, THX Spatial Audio has to be selected as your default system playback device. View Setup again by clicking the button. |
| `THX_TIPS` | THX Spatial Audio 空间音效是享受双声道或多声道（5.1 或 7.1）音频源内容的理想之选。<br><br>为获得最佳聆听体验，请确保音箱彼此相距 60 – 80 厘米，正面与你相距 60 – 80 厘米，且音箱都朝向你。 | THX Spatial Audio is ideal for any content with a binaural or multi-channel (5.1 or 7.1) audio source.<br><br>For the best listening experience,position the speakers 60 ⁠– 80 cm from each other, stay within 60 ⁠– 80 cm from the front, and point the speakers towards you. |
| `THX_TURNED_OFF` | 关闭 THX Spatial Audio 空间音效时，所有音频输出将是立体声。 | While THX Spatial Audio is turned off, all audio output is in stereo. |
| `THX_TURNED_ON_BUT_DONT_HAVE_ANY_APPLICATIONS` | When launched, applications that has audio output will appear on this window. From here you can change the output setting for each application. | When launched, applications that has audio output will appear on this window. From here you can change the output setting for each application. |
| `THX_VIEW_SUPPORTED_GAMES` | 查看所支持的游戏 | View the list of supported games |
| `THX_VIEW_TUTORIAL` | 查看教程 | View Tutorial |
| `THX_WHAT_NEWS` | 新增功能特性 | WHAT’S NEW |
| `TREBLE` | 高音 | TREBLE |
| `VOLUME` | 音量 | Volume |
| `VOLUME_CAROL_TOOLTIP` | 调整设备的输出音量。你可通过系统的音量合成器单独控制每个应用程序的音量。<br><br>当设备用作环绕扬声器时，它会自动调节音响系统的主音量。 | Adjust the device's output volume. Each application volume can be individually controlled with the system’s volume mixer. <br><br>When the device is used as Surround speakers, it automatically adjusts the master volume of the sound system. |
| `VOLUME_DOWN` | 降低音量 | Volume Down |
| `VOLUME_GUIDE_DESC` | 建议你将所有音量设置为最高，然后使用耳机上的控制按钮作为主要的音量控制。 | It is recommended to set all volume levels to maximum then use the on-headset controls as your master volume control. |
| `VOLUME_HEADER` | 音量 | VOLUME |
| `VOLUME_LEVEL` | 音量 | Volume Level |
| `VOLUME_MUTE_OR_UNMUTE` | Volume Mute/Unmute | Volume Mute/Unmute |
| `VOLUME_NOMMO_TOOLTIP` | 调整设备的输出音量。你可通过音量合成器单独控制每个应用程序的音量。 | Adjust the device's output volume. Each application volume can be individually controlled with the volume mixer. |
| `VOLUME_NORMALIZATION` | 音量标准化 | VOLUME NORMALIZATION |
| `VOLUME_NORMALIZATION_LOWERCASE` | 音量标准化 | volume normalization |
| `VOLUME_NORMALIZATION_TOOLTIP` | 音量标准化通过调节频率级别来减少响度变化。 | VOLUME  NORMALIZATION reduces the variation of loudness by adjusting frequency levels. |
| `VOLUME_UP` | 提高音量 | Volume Up |
## 4. 本项目的实现状态

| 项 | 内容 |
|---|---|
| 本项目的页面 | src/pages/calibration.rs —— 已实现（表面配置文件 / 校准状态机） |
| 后端方案 | 方案 2：复用雷云原生引擎（`lighting_driver` / `RzLightingEngineApi` / `mapping_engine` / `simple_service`） |
| 证据等级 | 布局 `[前端]`（设备模块 CSS）、文案 `[文案]`（语言包）、设备归属 `[前端]`（设备模块常量块） |

> 尚未实现的界面在应用里走占位页；占位页列出该页**真实存在的 key**，不编造内容。


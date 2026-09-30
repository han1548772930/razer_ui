# 灯光（TAB_LIGHTING）

> 本页由雷云自己的产品模块提取；本文件只记录原版事实，不记录本项目实现状态。
> 证据工具：`node .ref/tools/gen-screen-docs.js`

## 1. 出现在哪些设备上

| 设备 | productId | 设备类型 | 该设备标签页顺序 |
|---|---|---|---|
| Blackwidow V4 Pro | 653 | 键盘 | 第 3 个：自定义 · 性能 · 灯光 · 电源 · 滚动 |
| RAZER KRAKEN BT SANRIO LIMITED EDITION | 777 | 耳机 | 第 2 个：自定义 · 灯光 · 校准 · 电源 · 声音 · 麦克风 |

标签页真实文案：**灯光**（key `TAB_LIGHTING`）

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

**调色板**（按出现次数排序，共统计 1857 处颜色声明）：

| 次数 | 颜色 | 角色 |
|---:|---|---|
| 303 | `#44d62c` | 主色 / 激活态 / 标题 |
| 269 | `#ccc` | 主文字 |
| 163 | `#5d5d5d` | **通用边框** |
| 147 | `#111` | 卡片 / 输入框底色 |
| 140 | `#000` | 按钮文字、描边、顶栏下边线 |
| 107 | `#0000` | 透明 |
| 79 | `#707070` | 次要按钮底色 |
| 71 | `#fd8611` | 橙色：提示 / 警告 / 分享 |
| 69 | `#999` | 次要文字 |
| 65 | `#fff` | 次要按钮文字 |
| 55 | `#222` | 页面底色 / 顶栏底色 |
| 26 | `#ffffff1a` | 半透明白：hover 覆盖 |
| 22 | `#212121` | 深色文字（浅底上）/ 离线指示 |
| 21 | `#0000004d` | 半透明黑：按钮描边 |
| 19 | `#c8323c` | 危险红 |
| 18 | `#ffffff4d` | 半透明白：按下覆盖 |
| 14 | `#fd4949` | 危险红（亮） |
| 14 | `#333` | 开关关闭态底色 |
| 13 | `#2d2d2d` | **hover 底色** |
| 13 | `#4a4a4a` | 帮助图标底色 |
| 11 | `#515151` | **下拉框边框** |
| 10 | `#292929` |  |

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

#### Blackwidow V4 Pro（productId 653）

该界面共 140 个布局类名，分 7 个分区。

**① 亮度**（9）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `brightness` | — | — |
| `brightness-and-light` | `display:flex; height:20px!important; margin:10px`<br>`align-items:center; display:inline-flex` | `text-transform:lowercase` |
| `brightness-container` | `padding:30px!important; width:600px!important`<br>`align-items:center; display:inline-flex; margin:0 10px 10px`<br>`display:flex; height:20px!important; margin:10px`<br>`align-items:center; display:inline-flex` | `color:#44d62c; font-size:16px`<br>`text-transform:lowercase` |
| `global-brightness` | `display:flex; height:20px!important; margin:10px`<br>`align-items:center; display:inline-flex` | `text-transform:lowercase` |
| `global-brightness-left` | `align-items:center; display:inline-flex` | — |
| `global-brightness-right` | — | `text-transform:lowercase` |
| `label-brightness` | — | `color:#44d62c; font-size:16px` |
| `logo-brightness-container` | `display:flex; gap:5px` | — |
| `switching-brightness` | `align-items:center; display:inline-flex; margin:0 10px 10px` | `color:#44d62c; font-size:16px` |

**② 快速效果（Quick Effects）**（6）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `multiple-quickeffect` | `display:flex; flex-wrap:wrap; gap:21px 0`<br>`width:50%`<br>`align-items:center; display:flex; gap:10px`<br>`margin:0; width:200px` | `opacity:.3`<br>`text-transform:uppercase` |
| `quickeffect` | `width:50%`<br>`flex-wrap:wrap; width:150px`<br>`flex-wrap:nowrap` | — |
| `quickeffect--disabled` | — | `opacity:.3` |
| `quickeffect-text` | — | `color:#ccc; font-size:14px; line-height:17px` |
| `quickeffect__dropdown` | `margin:0; width:200px` | — |
| `quickeffect__header` | `align-items:center; display:flex; gap:10px` | `text-transform:uppercase` |

**③ 效果选择网格**（12）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `adveffect-detail` | — | `line-height:18px` |
| `chargingEffect` | — | `opacity:1`<br>`opacity:.3` |
| `effect` | — | — |
| `effect-wrapper` | `display:flex; position:relative`<br>`flex:1 1 auto`<br>`height:27px; position:relative; width:60px`<br>`height:25px; padding:5px 18px 5px 5px; width:58px` | `border:1px solid #5d5d5d; transition:opacity .2s`<br>`background-color:#111; border:none; color:#ccc; font-size:14px; line-height:17px; text-align:left`<br>`border:1px solid #44d62c` |
| `effects-area` | `position:relative`<br>`display:flex`<br>`display:none`<br>`display:flex!important` | `background-color:#0000`<br>`background-image:url(../../static/media/icon_battery_graph.5140e4be.svg); background-repeat:no-repeat; background-size:cover`<br>`border:1px solid #5d5d5d; transition:opacity .2s`<br>`background-color:#111; border:none; color:#ccc; font-size:14px; line-height:17px; text-align:left`<br>`border:1px solid #44d62c` |
| `effects-area--wireless` | `flex-wrap:wrap; width:150px` | — |
| `idleEffect` | — | `opacity:1`<br>`opacity:.3` |
| `lighting-effect` | `height:26px; padding:5px 10px; position:relative`<br>`height:22px; padding:3px 10px; position:relative`<br>`top:50%` | `border-radius:13px`<br>`border-radius:11px` |
| `logo-effects-radio-container` | `display:flex; flex-direction:column; gap:10px` | — |
| `optionButtonGroup_effect-with-duration__PkoU2` | `display:flex; flex-direction:column` | `opacity:.5` |
| `optionButtonGroup_effects-area__8iBAF` | `display:flex; flex-direction:column`<br>`align-items:flex-start` | — |
| `power-saving-effect` | `display:flex; flex-direction:column; position:relative`<br>`margin:20px 0 10px`<br>`display:flex`<br>`width:-webkit-fit-content; width:fit-content` | `color:#ccc`<br>`opacity:1` |

**④ 效果参数（颜色 / 速度 / 方向）**（25）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `WaveWithSpeedV2_active__huGMw` | — | `background-color:#333; border-color:#44d62c` |
| `WaveWithSpeedV2_speedSelect__2ECqO` | `display:flex; gap:12px` | — |
| `WaveWithSpeedV2_speedValue__Ydwv2` | `min-width:70px; padding:4px 25px` | `background-color:#0000; border:1px solid #444; border-radius:4px; color:#fff; cursor:pointer; font-size:14px; line-height:17px; text-align:center; text-transform:uppercase; transition:all .2s ease`<br>`border-color:#44d62c`<br>`background-color:#333; border-color:#44d62c` |
| `WaveWithSpeedV2_waveButtons__vAgEX` | `display:grid; gap:20px` | — |
| `active-color` | — | `color:#44d62c` |
| `add-color` | `display:block; height:18px; width:18px` | `border:1px solid #000; box-shadow:0 0 0 1px #44d62c`<br>`border:1px solid #5d5d5d`<br>`background-image:url(../../static/media/plus.a386742c.svg); background-position:50%; background-repeat:no-repeat; background-size:8px; color:#5d5d5d`<br>`border-color:#44d62c; box-shadow:none`<br>`background-color:#ffffff1a!important; box-shadow:none` |
| `color` | `position:relative`<br>`height:8px; position:absolute; right:0; top:0; width:8px`<br>`height:100%; width:100%` | `border-radius:50%; color:var(--profile-color)`<br>`background-image:url(../../static/media/icon_obm.888208d6.svg)` |
| `color-container` | `height:100%; width:100%` | `border-radius:3px` |
| `color-control` | `flex:0 0 auto!important; height:48px; position:relative`<br>`bottom:0; position:absolute; width:calc(100% - 5px)` | `color:#ccc; font-size:14px; line-height:19px; text-align:center; text-transform:uppercase` |
| `color-label` | `height:20px; width:100%` | `color:#ccc; font-size:14px; line-height:20px; text-align:left; text-transform:capitalize` |
| `color-opts` | `display:none`<br>`display:block`<br>`flex-direction:column` | — |
| `color-pattern` | `justify-content:space-between; margin:6px 54px; width:112px` | — |
| `color-save` | — | — |
| `custom-color` | `align-items:center; display:flex; height:40px; padding:0 10px; width:100%`<br>`height:20px; margin:10px 5px; position:relative; width:20px`<br>`display:block; height:18px; position:absolute; width:18px` | `border-radius:3px`<br>`border:1px solid #000; box-shadow:0 0 0 2px #fff`<br>`background:#0000004d` |
| `customize-polling-rate-button-color` | `height:6px; width:6px` | `border-radius:50%` |
| `disable-color` | — | `color:#c8323c` |
| `dropdown-color` | `flex:0 0 auto!important; width:53px`<br>`height:0`<br>`height:20px; width:100%`<br>`height:250px; min-height:250px; min-width:250px; overflow:visible; overflow:initial; padding:5px 0 10px; width:250px` | `opacity:0; transition:all .1s,width 0`<br>`color:#ccc; font-size:14px; line-height:20px; text-align:left; text-transform:capitalize`<br>`background-color:#111; opacity:1`<br>`background-color:#fff; border:1px solid #0000004d; border-radius:3px; transition:border-color .2s,opacity .2s,background-color .2s` |
| `has-color` | `display:none` | — |
| `no-color` | `overflow:hidden; position:absolute; right:4px; top:155px`<br>`display:block; height:18px; left:0; position:absolute; top:0; width:18px`<br>`display:none` | `background:#c83200; background-image:url(../../static/media/disable_palette.03c60895.svg)` |
| `optionButtonGroup_random-color__wSIuX` | — | — |
| `profile-color` | `height:8px; position:absolute; right:0; top:0; width:8px` | `border-radius:50%; color:var(--profile-color)`<br>`background:#44d62c`<br>`background:red`<br>`background:lime`<br>`background:blue` |
| `random-color` | `width:120px`<br>`display:none`<br>`margin:30px 0 0`<br>`margin:0` | — |
| `selected-color-icon` | `height:9px; position:absolute; right:1px; top:1px; width:9px` | `border-radius:50%`<br>`background-color:red`<br>`background-color:#44d62c`<br>`background-color:green`<br>`background-color:blue` |
| `static-lighting-color` | `height:20px; margin:0 5px 5px 0; width:20px` | `background-color:var(--color); border-radius:20%`<br>`border:1px solid #000; box-shadow:0 0 0 2px #fff`<br>`border:1px solid #000; box-shadow:0 0 0 1px #44d62c` |
| `volume-dial-color` | `height:4px; width:12px` | `border-radius:31px` |

**⑤ 幻彩互联（Chroma Connect）**（24）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `btn-install-chroma` | `bottom:20px; left:50%; padding:4px 15px; position:absolute`<br>`display:flex`<br>`display:block; height:16px; position:relative; width:16px` | `background-color:#44d62c; border:1px solid #44d62c; border-radius:2px; color:#111; font-size:12px`<br>`background-color:#7de36c`<br>`background-color:#44d62c; opacity:.8`<br>`background-color:#44d62c`<br>`background-image:url('data:image/svg+xml; background-repeat:no-repeat; background-size:contain` |
| `chroma-flex-row` | `align-items:center`<br>`flex:0 0 auto!important`<br>`max-width:150px` | — |
| `chroma-icon` | `height:auto; padding:10px; position:absolute; width:25px` | `background-position:50%; background-repeat:no-repeat; background-size:20px 20px` |
| `chroma-rbg-text-container` | `align-items:flex-start; display:flex` | — |
| `chroma-studio` | `height:27px; min-width:150px; padding:5px 8px 5px 31px; position:relative`<br>`display:inline-block; height:18px; left:8px; position:absolute; top:4px; width:18px` | `background-color:#111; border:1px solid #737373; border-radius:3px; line-height:17px; text-transform:uppercase; transition:border-color .3s,background-color .3s`<br>`background-image:url(../../static/media/chroma_studio.55db6875.svg)`<br>`border-color:#44d62c`<br>`background-color:#ffffff1a` |
| `chroma-studio-animate` | `height:26px; position:relative; width:26px`<br>`display:inline-block; height:26px; left:0; position:absolute; top:0; width:26px` | `background-image:url(../../static/media/chroma_sync_v3_static.c8ddf315.svg)`<br>`border-color:#44d62c`<br>`background-color:#ffffff1a`<br>`background-color:#111; border-color:#737373; cursor:default` |
| `chroma-studio-btn` | `align-items:center; display:flex; height:50px; width:240px` | `background-color:#111; background-image:url(../../static/media/logo_chromastudio.5b277459.svg); background-position:16px; background-repeat:no-repeat; background-size:30px 30px; border:1px solid #ccc; border-radius:3px; text-transform:uppercase`<br>`background-color:#000`<br>`background-color:#111` |
| `chroma-studio-dropdown` | `margin:0!important; width:200px` | — |
| `chroma-studio-icon` | `height:18px; min-width:18px; width:18px` | `background-image:url(../../static/media/chroma_studio.55db6875.svg); background-repeat:no-repeat; background-size:18px` |
| `chroma-studio-message` | `padding:20px`<br>`display:flex; justify-content:space-between`<br>`align-items:center; display:inline-flex; min-height:27px; min-width:150px; padding:4px`<br>`height:18px; min-width:18px; width:18px` | `font-family:Roboto,sans-serif; font-size:14px`<br>`border:1px solid #707070; border-radius:5px; font-size:13px; line-height:16px; transition:all .3s ease`<br>`border:1px solid #737373; border-radius:3px; cursor:pointer`<br>`background-image:url(../../static/media/chroma_studio.55db6875.svg); background-repeat:no-repeat; background-size:18px`<br>`cursor:pointer` |
| `chroma-studio-message-btn` | `align-items:center; display:inline-flex; min-height:27px; min-width:150px; padding:4px`<br>`height:18px; min-width:18px; width:18px` | `border:1px solid #737373; border-radius:3px; cursor:pointer`<br>`background-image:url(../../static/media/chroma_studio.55db6875.svg); background-repeat:no-repeat; background-size:18px` |
| `chroma-studio-wrapper` | `display:flex; justify-content:space-between`<br>`align-items:center; display:inline-flex; min-height:27px; min-width:150px; padding:4px`<br>`height:18px; min-width:18px; width:18px`<br>`margin:auto 0` | `border:1px solid #737373; border-radius:3px; cursor:pointer`<br>`background-image:url(../../static/media/chroma_studio.55db6875.svg); background-repeat:no-repeat; background-size:18px`<br>`cursor:pointer` |
| `chroma-sync` | `align-items:center; position:relative`<br>`width:340px` | `color:#44d62c`<br>`color:#44d62c; opacity:.7`<br>`opacity:.3` |
| `chroma-sync-text` | `width:340px`<br>`display:flex; flex-direction:column; left:36px; min-width:180px; position:absolute; top:100%`<br>`align-items:center; display:flex; flex:1 1; padding:5px 6px` | `background-color:#000; border:1px solid #5d5d5d`<br>`color:#ccc; font-size:14px`<br>`background-color:hsla(0,0%,100%,.102)` |
| `chromastudio` | — | `background-size:80px 80px` |
| `iframe-chroma-app` | `top:0` | — |
| `img-chroma-wordmark` | `height:100%` | `background:url(../../static/media/razer_wordmark_chroma.0709fcef.svg)`<br>`background-position:50%; background-repeat:no-repeat` |
| `install-chroma-img` | `height:180px; position:relative; width:520px`<br>`bottom:20px; left:50%; padding:4px 15px; position:absolute`<br>`display:flex`<br>`display:block; height:16px; position:relative; width:16px` | `background-image:url(../../static/media/install_chroma.85d4bc96.avif)`<br>`background-color:#44d62c; border:1px solid #44d62c; border-radius:2px; color:#111; font-size:12px`<br>`background-color:#7de36c`<br>`background-color:#44d62c; opacity:.8`<br>`background-color:#44d62c` |
| `logo-chroma` | — | `background-image:url(data:image/png` |
| `no-chroma-studio-profile` | — | `color:#44d62c` |
| `panel-light--chroma` | `min-width:520px`<br>`position:relative`<br>`height:17px; left:0; position:absolute; top:0; width:20px` | `transition:opacity .2s`<br>`background-image:url(../../static/media/warning-icon.b0086fcc.svg); background-position:50%; background-size:20px 17px` |
| `panel-light--chroma__block` | `min-width:520px`<br>`position:relative`<br>`height:17px; left:0; position:absolute; top:0; width:20px` | `background-image:url(../../static/media/warning-icon.b0086fcc.svg); background-position:50%; background-size:20px 17px` |
| `panel-light--chroma__warning` | `height:17px; left:0; position:absolute; top:0; width:20px` | `background-image:url(../../static/media/warning-icon.b0086fcc.svg); background-position:50%; background-size:20px 17px` |
| `status-chroma-app` | `display:flex; height:45px; position:relative; width:100%`<br>`height:auto; padding:10px; position:absolute; width:25px`<br>`display:flex; flex:0 0 500px; flex-direction:column; justify-content:center`<br>`top:15px` | `background-color:#2d2d2d; border-radius:5px; font-size:14px; text-align:center`<br>`background-position:50%; background-repeat:no-repeat; background-size:20px 20px`<br>`color:#7d7d7d`<br>`color:#ccc`<br>`background-position:50%; background-size:16px 16px` |

**⑦ 通用控件**（5）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `Duallink_unpairbuttondisabled__ZdDMy` | `display:block; height:28px; left:50%; padding:0 10px; position:absolute; top:50%` | `background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; opacity:.5; text-align:center` |
| `indicator-led-tooltip` | `min-width:180px` | — |
| `indicator-led-tooltip-body` | `display:flex`<br>`height:85px; margin:0 10px; width:4px`<br>`display:flex; flex-direction:column; justify-content:space-between`<br>`display:flex; flex-direction:column; gap:6px` | `text-transform:uppercase`<br>`background:linear-gradient(180deg,#0f0 0,#e3b617 32.5%,#ff3d00 98.5%); border-radius:5px`<br>`border-radius:50%` |
| `indicator-led-tooltip-header` | `align-items:center; display:flex; justify-content:space-between`<br>`align-items:center; display:flex; gap:16px; padding:8px`<br>`height:8px; width:8px`<br>`align-items:center; display:flex` | `background-color:gray; border-radius:50%`<br>`background-color:#44d62c` |
| `optionButtonGroup_disabled__Q0Wrc` | — | `background:#ffffff1a`<br>`background:#292929; border:1px solid #44d62c`<br>`color:#ffffff80; cursor:not-allowed`<br>`opacity:.5` |

**⑫ 其它**（59）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `DEVICE_BRIGHTNESS` | — | `background-image:url(../../static/media/icon_config_brightness.7a95c958.svg)`<br>`background-image:url(../../static/media/icon_config_brightness_a.570b75be.svg)` |
| `DimKeyboardLighting_backdrop__WXv8r` | `height:30px; position:absolute; width:300px` | `background-color:#111; opacity:.5` |
| `DimKeyboardLighting_width-auto__C0C3F` | `width:48px!important` | — |
| `Duallink_disabledText__luVtO` | — | — |
| `Duallink_disabled__IFrAi` | — | `opacity:.45` |
| `GLOBAL_BRIGHTNESS` | — | `background-image:url(../../static/media/icon_config_brightness_global.755a1ebf.svg)`<br>`background-image:url(../../static/media/icon_config_brightness_global_a.dadb5e34.svg)` |
| `GameMode_disabled__Cmc9j` | `height:15.4px!important; left:8.6px; top:16.4px`<br>`height:9.6px!important; left:.8px; top:10.2px` | `background-color:#0000!important; border-color:#0000!important`<br>`background-color:#44d62c`<br>`opacity:1` |
| `MonitoringDashboard_disabled__sjQ5Z` | `display:none` | `color:#999`<br>`color:#fff`<br>`opacity:1` |
| `MonitoringDashboard_menuItemDisabled__CtX9E` | — | `color:#666!important; cursor:not-allowed; opacity:.5` |
| `SWITCH_LIGHTING` | — | `background-image:url(data:image/png` |
| `app-disabled` | — | `opacity:.3` |
| `canceled` | — | `color:#fd4949; font-size:12px` |
| `custom-lighting` | `min-width:0; min-width:auto; width:auto` | — |
| `dialClickDisabled` | — | `fill:#44d62c66`<br>`fill:#000c`<br>`fill:#c8323c80` |
| `disabled` | `width:56px`<br>`display:block`<br>`position:relative`<br>`bottom:0; left:0; position:absolute; right:0; top:0` | `cursor:default; opacity:.3`<br>`opacity:.5`<br>`opacity:.3`<br>`background-color:#0000; cursor:default`<br>`border:1px solid #515151; cursor:default` |
| `disabled-2` | — | `color:#44d62c; text-transform:none` |
| `disabled-banner` | — | `opacity:0` |
| `disabled-lighting-tab` | — | `opacity:.3` |
| `disabledxy` | `height:15px` | — |
| `enabled` | `display:none` | — |
| `enabled-pointer` | — | — |
| `icon-disabled-feature-container` | `height:20px; position:relative; width:20px` | — |
| `icon-led-detection` | `align-self:center!important` | — |
| `indicator-led` | `bottom:-5px; height:10px; left:5px; position:absolute; width:50px`<br>`left:36%; top:19px` | `opacity:1`<br>`background-color:#0000` |
| `indicator-led-image__wrapper` | `position:relative`<br>`width:150px`<br>`align-items:center; display:flex; left:53px; position:absolute; top:58px`<br>`height:14.5px; width:14.5px` | — |
| `indicator-led-select_item` | `display:flex; flex-direction:column; gap:10px`<br>`margin:0; width:160px` | — |
| `indicator-led-select_wrapper` | `display:flex; gap:20px` | — |
| `indicator-led__container` | `align-items:center; display:flex; flex-direction:column; gap:20px`<br>`align-items:center; display:inline-flex; gap:4px; height:20px; width:99px`<br>`height:20px; width:20px` | `opacity:1`<br>`color:#44d62c; cursor:pointer; font-family:Roboto,sans-serif; font-size:14px; font-weight:400; line-height:20px`<br>`cursor:pointer; opacity:1` |
| `indicator-led__image` | `width:150px` | — |
| `indicator-led__ledWrapper` | `align-items:center; display:flex; left:53px; position:absolute; top:58px`<br>`height:14.5px; width:14.5px` | — |
| `indicator-led__link` | `align-items:center; display:inline-flex; gap:4px; height:20px; width:99px` | `opacity:1` |
| `indicator-led__link-icon` | `height:20px; width:20px` | `cursor:pointer; opacity:1` |
| `indicator-led__link-text` | — | `color:#44d62c; cursor:pointer; font-family:Roboto,sans-serif; font-size:14px; font-weight:400; line-height:20px` |
| `keymapbar-enabled` | `justify-content:left; margin:0 0 0 20px`<br>`display:flex`<br>`flex:1 1 auto`<br>`display:none` | `color:#ccc` |
| `label-total-led-count` | `align-self:center` | — |
| `light-container--disabled` | — | `opacity:.3` |
| `no-of-led-dropdown` | `height:27px; margin:0!important; width:62px`<br>`align-items:center!important; display:inline-flex!important` | — |
| `number-led-container` | `align-items:center; display:flex; gap:10px`<br>`height:27px; margin:0!important; width:62px`<br>`align-items:center!important; display:inline-flex!important`<br>`height:27px; margin:0!important; position:relative!important; width:62px` | `border:1px solid #5d5d5d`<br>`opacity:.3`<br>`text-align:left` |
| `number-led-container--with-closed` | `width:80px` | — |
| `number-led-stepper` | `height:27px; margin:0!important; position:relative!important; width:62px`<br>`height:25px!important; width:60px!important` | `text-align:left` |
| `number-led-stepper--disabled-decrease` | — | `opacity:.3` |
| `number-led-stepper--disabled-increase` | — | `opacity:.3` |
| `number-led-stepper--stepper` | — | `border:1px solid #5d5d5d` |
| `number-led-wrapper` | `align-items:center; display:flex; gap:10px` | — |
| `oled-warning` | `align-items:center; display:flex; flex-wrap:wrap`<br>`height:20px; width:20px` | `color:#999; font-size:14px; line-height:17px`<br>`background-color:#111; border:2px solid #999; border-radius:50%; font-weight:700; text-align:center` |
| `port-items-led-image` | `display:flex; flex-direction:row`<br>`flex:1 1`<br>`align-items:center; display:inline-flex; height:27px`<br>`position:relative` | `transition:all .3s`<br>`font-family:Roboto,sans-serif; font-size:14px`<br>`color:#707070; cursor:pointer`<br>`color:#44d62c`<br>`background-color:#44d62c` |
| `powerSaving-wrap--disabled` | — | `opacity:30%` |
| `remap-2-disabled` | — | `color:#c8323c` |
| `reset-oled-download` | `align-items:center; display:flex; justify-content:center; position:relative`<br>`height:20px; width:20px`<br>`height:0; left:-255px; padding:20px; position:absolute; top:40px; width:300px`<br>`height:auto` | `background-image:url(../../static/media/icon_download_animated.c25fe72c.svg); background-repeat:no-repeat; background-size:cover`<br>`background:#111; border:1px solid #5d5d5d; opacity:0; transition:height 0s,visibility 0s,opacity .1s linear`<br>`opacity:1`<br>`text-transform:uppercase`<br>`background-color:#5d5d5d; border:none` |
| `shortcutButton-filled` | — | `background-color:#292929` |
| `small-twoway-lighting` | `height:32px; padding:5px`<br>`height:22px; padding:3px 10px; position:relative` | `background-color:#111; border:1px solid #5d5d5d; border-radius:16px; transition:border-color .2s,background-color .5s`<br>`background-color:#0000001a`<br>`border-radius:11px` |
| `static-lighting-active` | `display:block; height:11px; margin:3.75px; position:absolute; width:11px` | `border:2px solid #fff; border-radius:5px; box-shadow:0 0 2px 1px rgba(0,0,0,.702),inset 0 0 2px 1px rgba(0,0,0,.702)` |
| `tip-disabled` | `left:25%; max-width:300px; padding:8px 10px; position:absolute; top:40%` | `background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-family:Roboto; line-height:16px; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `total-LED-count` | `align-items:center; display:grid; grid-template-columns:150px auto auto; height:27px`<br>`align-self:center`<br>`align-self:center; width:50px`<br>`align-self:center!important` | `font-family:Roboto,sans-serif; font-size:14px` |
| `twoway-lighting` | `height:36px; padding:5px`<br>`height:26px; padding:5px 10px; position:relative` | `background-color:#111; border:1px solid #5d5d5d; border-radius:18px; transition:border-color .2s,background-color .5s`<br>`background-color:#0000001a`<br>`border-radius:13px` |
| `value-total-led-count` | `align-self:center; width:50px` | — |
| `wave-dir` | `align-items:start; overflow:visible; overflow:initial`<br>`width:51px`<br>`height:27px` | `background-color:#0000; border:1px solid #0000; transition:border .2s`<br>`background-color:#44d62c`<br>`border:1px solid #44d62c; transition:border 0s`<br>`background-color:#111`<br>`background-color:#ffffff1a` |
| `wrapper-disabled` | — | `opacity:1` |
| `y-enabled` | `height:114px`<br>`bottom:25px; display:block` | `opacity:1`<br>`opacity:0` |

#### RAZER KRAKEN BT SANRIO LIMITED EDITION（productId 777）

该界面共 127 个布局类名，分 7 个分区。

**① 亮度**（9）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `brightness` | — | — |
| `brightness-and-light` | `display:flex; height:20px!important; margin:10px`<br>`align-items:center; display:inline-flex` | `text-transform:lowercase` |
| `brightness-container` | `padding:30px!important; width:600px!important`<br>`align-items:center; display:inline-flex; margin:0 10px 10px`<br>`display:flex; height:20px!important; margin:10px`<br>`align-items:center; display:inline-flex` | `color:#44d62c; font-size:16px`<br>`text-transform:lowercase` |
| `global-brightness` | `display:flex; height:20px!important; margin:10px`<br>`align-items:center; display:inline-flex` | `text-transform:lowercase` |
| `global-brightness-left` | `align-items:center; display:inline-flex` | — |
| `global-brightness-right` | — | `text-transform:lowercase` |
| `label-brightness` | — | `color:#44d62c; font-size:16px` |
| `logo-brightness-container` | `display:flex; gap:5px` | — |
| `switching-brightness` | `align-items:center; display:inline-flex; margin:0 10px 10px` | `color:#44d62c; font-size:16px` |

**② 快速效果（Quick Effects）**（6）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `multiple-quickeffect` | `display:flex; flex-wrap:wrap; gap:21px 0`<br>`width:50%`<br>`align-items:center; display:flex; gap:10px`<br>`margin:0; width:200px` | `opacity:.3`<br>`text-transform:uppercase` |
| `quickeffect` | `width:50%`<br>`flex-wrap:wrap; width:150px`<br>`flex-wrap:nowrap` | — |
| `quickeffect--disabled` | — | `opacity:.3` |
| `quickeffect-text` | — | `color:#ccc; font-size:14px; line-height:17px` |
| `quickeffect__dropdown` | `margin:0; width:200px` | — |
| `quickeffect__header` | `align-items:center; display:flex; gap:10px` | `text-transform:uppercase` |

**③ 效果选择网格**（12）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `adveffect-detail` | — | `line-height:18px` |
| `chargingEffect` | — | `opacity:1`<br>`opacity:.3` |
| `effect` | — | — |
| `effect-wrapper` | `display:flex; position:relative`<br>`flex:1 1 auto`<br>`height:27px; position:relative; width:60px`<br>`height:25px; padding:5px 18px 5px 5px; width:58px` | `border:1px solid #5d5d5d; transition:opacity .2s`<br>`background-color:#111; border:none; color:#ccc; font-size:14px; line-height:17px; text-align:left`<br>`border:1px solid #44d62c` |
| `effects-area` | `position:relative`<br>`display:flex`<br>`display:none`<br>`display:flex!important` | `background-color:#0000`<br>`background-image:url(../../static/media/icon_battery_graph.5140e4be.svg); background-repeat:no-repeat; background-size:cover`<br>`border:1px solid #5d5d5d; transition:opacity .2s`<br>`background-color:#111; border:none; color:#ccc; font-size:14px; line-height:17px; text-align:left`<br>`border:1px solid #44d62c` |
| `effects-area--wireless` | `flex-wrap:wrap; width:150px` | — |
| `idleEffect` | — | `opacity:1`<br>`opacity:.3` |
| `lighting-effect` | `height:26px; padding:5px 10px; position:relative`<br>`height:22px; padding:3px 10px; position:relative`<br>`top:50%` | `border-radius:13px`<br>`border-radius:11px` |
| `logo-effects-radio-container` | `display:flex; flex-direction:column; gap:10px` | — |
| `optionButtonGroup_effect-with-duration__PkoU2` | `display:flex; flex-direction:column` | `opacity:.5` |
| `optionButtonGroup_effects-area__8iBAF` | `display:flex; flex-direction:column`<br>`align-items:flex-start` | — |
| `power-saving-effect` | `display:flex; flex-direction:column; position:relative`<br>`margin:20px 0 10px`<br>`display:flex`<br>`width:-webkit-fit-content; width:fit-content` | `color:#ccc`<br>`opacity:1` |

**④ 效果参数（颜色 / 速度 / 方向）**（21）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `WaveWithSpeedV2_active__huGMw` | — | `background-color:#333; border-color:#44d62c` |
| `WaveWithSpeedV2_speedSelect__2ECqO` | `display:flex; gap:12px` | — |
| `WaveWithSpeedV2_speedValue__Ydwv2` | `min-width:70px; padding:4px 25px` | `background-color:#0000; border:1px solid #444; border-radius:4px; color:#fff; cursor:pointer; font-size:14px; line-height:17px; text-align:center; text-transform:uppercase; transition:all .2s ease`<br>`border-color:#44d62c`<br>`background-color:#333; border-color:#44d62c` |
| `WaveWithSpeedV2_waveButtons__vAgEX` | `display:grid; gap:20px` | — |
| `add-color` | `display:block; height:18px; width:18px` | `border:1px solid #000; box-shadow:0 0 0 1px #44d62c`<br>`border:1px solid #5d5d5d`<br>`background-image:url(../../static/media/plus.a386742c.svg); background-position:50%; background-repeat:no-repeat; background-size:8px; color:#5d5d5d`<br>`border-color:#44d62c; box-shadow:none`<br>`background-color:#ffffff1a!important; box-shadow:none` |
| `color` | `position:relative`<br>`height:8px; position:absolute; right:0; top:0; width:8px`<br>`height:100%; width:100%`<br>`display:block; height:20px; position:absolute; width:20px` | `border-radius:50%; color:var(--profile-color)`<br>`background-image:url(../../static/media/icon_obm.888208d6.svg)`<br>`background-position:50%; background-repeat:no-repeat; background-size:20px`<br>`background-image:url(../../static/media/icon_obm_red.1ef87484.svg)`<br>`background-image:url(../../static/media/icon_obm_green.d44df233.svg)` |
| `color-container` | `height:100%; width:100%` | `border-radius:3px` |
| `color-control` | `flex:0 0 auto!important; height:48px; position:relative`<br>`bottom:0; position:absolute; width:calc(100% - 5px)` | `color:#ccc; font-size:14px; line-height:19px; text-align:center; text-transform:uppercase` |
| `color-label` | `height:20px; width:100%` | `color:#ccc; font-size:14px; line-height:20px; text-align:left; text-transform:capitalize` |
| `color-opts` | `display:none`<br>`display:block`<br>`flex-direction:column` | — |
| `color-pattern` | `justify-content:space-between; margin:6px 54px; width:112px` | — |
| `color-save` | — | — |
| `custom-color` | `align-items:center; display:flex; height:40px; padding:0 10px; width:100%`<br>`height:20px; margin:10px 5px; position:relative; width:20px`<br>`display:block; height:18px; position:absolute; width:18px` | `border-radius:3px`<br>`border:1px solid #000; box-shadow:0 0 0 2px #fff`<br>`background:#0000004d` |
| `dropdown-color` | `flex:0 0 auto!important; width:53px`<br>`height:0`<br>`height:20px; width:100%`<br>`height:250px; min-height:250px; min-width:250px; overflow:visible; overflow:initial; padding:5px 0 10px; width:250px` | `opacity:0; transition:all .1s,width 0`<br>`color:#ccc; font-size:14px; line-height:20px; text-align:left; text-transform:capitalize`<br>`background-color:#111; opacity:1`<br>`background-color:#fff; border:1px solid #0000004d; border-radius:3px; transition:border-color .2s,opacity .2s,background-color .2s` |
| `has-color` | `display:none` | — |
| `no-color` | `overflow:hidden; position:absolute; right:4px; top:155px`<br>`display:block; height:18px; left:0; position:absolute; top:0; width:18px` | `background:#c83200; background-image:url(../../static/media/disable_palette.03c60895.svg)` |
| `optionButtonGroup_random-color__wSIuX` | — | — |
| `profile-color` | `height:8px; position:absolute; right:0; top:0; width:8px` | `border-radius:50%; color:var(--profile-color)`<br>`background:#44d62c`<br>`background:red`<br>`background:lime`<br>`background:blue` |
| `random-color` | `width:120px`<br>`display:none`<br>`margin:30px 0 0`<br>`margin:0` | — |
| `selected-color-icon` | `height:9px; position:absolute; right:1px; top:1px; width:9px` | `border-radius:50%`<br>`background-color:red`<br>`background-color:#44d62c`<br>`background-color:green`<br>`background-color:blue` |
| `static-lighting-color` | `height:20px; margin:0 5px 5px 0; width:20px` | `background-color:var(--color); border-radius:20%`<br>`border:1px solid #000; box-shadow:0 0 0 2px #fff`<br>`border:1px solid #000; box-shadow:0 0 0 1px #44d62c` |

**⑤ 幻彩互联（Chroma Connect）**（24）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `btn-install-chroma` | `bottom:20px; left:50%; padding:4px 15px; position:absolute`<br>`display:flex`<br>`display:block; height:16px; position:relative; width:16px` | `background-color:#44d62c; border:1px solid #44d62c; border-radius:2px; color:#111; font-size:12px`<br>`background-color:#7de36c`<br>`background-color:#44d62c; opacity:.8`<br>`background-color:#44d62c`<br>`background-image:url('data:image/svg+xml; background-repeat:no-repeat; background-size:contain` |
| `chroma-flex-row` | `align-items:center`<br>`flex:0 0 auto!important`<br>`max-width:150px` | — |
| `chroma-icon` | `height:auto; padding:10px; position:absolute; width:25px` | `background-position:50%; background-repeat:no-repeat; background-size:20px 20px` |
| `chroma-rbg-text-container` | `align-items:flex-start; display:flex` | — |
| `chroma-studio` | `height:27px; min-width:150px; padding:5px 8px 5px 31px; position:relative`<br>`display:inline-block; height:18px; left:8px; position:absolute; top:4px; width:18px` | `background-color:#111; border:1px solid #737373; border-radius:3px; line-height:17px; text-transform:uppercase; transition:border-color .3s,background-color .3s`<br>`background-image:url(../../static/media/chroma_studio.55db6875.svg)`<br>`border-color:#44d62c`<br>`background-color:#ffffff1a` |
| `chroma-studio-animate` | `height:26px; position:relative; width:26px`<br>`display:inline-block; height:26px; left:0; position:absolute; top:0; width:26px` | `background-image:url(../../static/media/chroma_sync_v3_static.c8ddf315.svg)`<br>`border-color:#44d62c`<br>`background-color:#ffffff1a`<br>`background-color:#111; border-color:#737373; cursor:default` |
| `chroma-studio-btn` | `align-items:center; display:flex; height:50px; width:240px` | `background-color:#111; background-image:url(../../static/media/logo_chromastudio.5b277459.svg); background-position:16px; background-repeat:no-repeat; background-size:30px 30px; border:1px solid #ccc; border-radius:3px; text-transform:uppercase`<br>`background-color:#000`<br>`background-color:#111` |
| `chroma-studio-dropdown` | `margin:0!important; width:200px` | — |
| `chroma-studio-icon` | `height:18px; min-width:18px; width:18px` | `background-image:url(../../static/media/chroma_studio.55db6875.svg); background-repeat:no-repeat; background-size:18px` |
| `chroma-studio-message` | `padding:20px`<br>`display:flex; justify-content:space-between`<br>`align-items:center; display:inline-flex; min-height:27px; min-width:150px; padding:4px`<br>`height:18px; min-width:18px; width:18px` | `font-family:Roboto,sans-serif; font-size:14px`<br>`border:1px solid #707070; border-radius:5px; font-size:13px; line-height:16px; transition:all .3s ease`<br>`border:1px solid #737373; border-radius:3px; cursor:pointer`<br>`background-image:url(../../static/media/chroma_studio.55db6875.svg); background-repeat:no-repeat; background-size:18px`<br>`cursor:pointer` |
| `chroma-studio-message-btn` | `align-items:center; display:inline-flex; min-height:27px; min-width:150px; padding:4px`<br>`height:18px; min-width:18px; width:18px` | `border:1px solid #737373; border-radius:3px; cursor:pointer`<br>`background-image:url(../../static/media/chroma_studio.55db6875.svg); background-repeat:no-repeat; background-size:18px` |
| `chroma-studio-wrapper` | `display:flex; justify-content:space-between`<br>`align-items:center; display:inline-flex; min-height:27px; min-width:150px; padding:4px`<br>`height:18px; min-width:18px; width:18px`<br>`margin:auto 0` | `border:1px solid #737373; border-radius:3px; cursor:pointer`<br>`background-image:url(../../static/media/chroma_studio.55db6875.svg); background-repeat:no-repeat; background-size:18px`<br>`cursor:pointer` |
| `chroma-sync` | `align-items:center; position:relative`<br>`width:340px` | `color:#44d62c`<br>`color:#44d62c; opacity:.7`<br>`opacity:.3` |
| `chroma-sync-text` | `width:340px`<br>`display:flex; flex-direction:column; left:36px; min-width:180px; position:absolute; top:100%`<br>`align-items:center; display:flex; flex:1 1; padding:5px 6px` | `background-color:#000; border:1px solid #5d5d5d`<br>`color:#ccc; font-size:14px`<br>`background-color:hsla(0,0%,100%,.102)` |
| `chromastudio` | — | `background-size:80px 80px` |
| `iframe-chroma-app` | `top:0` | — |
| `img-chroma-wordmark` | `height:100%` | `background:url(../../static/media/razer_wordmark_chroma.0709fcef.svg)`<br>`background-position:50%; background-repeat:no-repeat` |
| `install-chroma-img` | `height:180px; position:relative; width:520px`<br>`bottom:20px; left:50%; padding:4px 15px; position:absolute`<br>`display:flex`<br>`display:block; height:16px; position:relative; width:16px` | `background-image:url(../../static/media/install_chroma.85d4bc96.avif)`<br>`background-color:#44d62c; border:1px solid #44d62c; border-radius:2px; color:#111; font-size:12px`<br>`background-color:#7de36c`<br>`background-color:#44d62c; opacity:.8`<br>`background-color:#44d62c` |
| `logo-chroma` | — | `background-image:url(data:image/png` |
| `no-chroma-studio-profile` | — | `color:#44d62c` |
| `panel-light--chroma` | `min-width:520px`<br>`position:relative`<br>`height:17px; left:0; position:absolute; top:0; width:20px` | `transition:opacity .2s`<br>`background-image:url(../../static/media/warning-icon.b0086fcc.svg); background-position:50%; background-size:20px 17px` |
| `panel-light--chroma__block` | `min-width:520px`<br>`position:relative`<br>`height:17px; left:0; position:absolute; top:0; width:20px` | `background-image:url(../../static/media/warning-icon.b0086fcc.svg); background-position:50%; background-size:20px 17px` |
| `panel-light--chroma__warning` | `height:17px; left:0; position:absolute; top:0; width:20px` | `background-image:url(../../static/media/warning-icon.b0086fcc.svg); background-position:50%; background-size:20px 17px` |
| `status-chroma-app` | `display:flex; height:45px; position:relative; width:100%`<br>`height:auto; padding:10px; position:absolute; width:25px`<br>`display:flex; flex:0 0 500px; flex-direction:column; justify-content:center`<br>`top:15px` | `background-color:#2d2d2d; border-radius:5px; font-size:14px; text-align:center`<br>`background-position:50%; background-repeat:no-repeat; background-size:20px 20px`<br>`color:#7d7d7d`<br>`color:#ccc`<br>`background-position:50%; background-size:16px 16px` |

**⑦ 通用控件**（4）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `indicator-led-tooltip` | `min-width:180px` | — |
| `indicator-led-tooltip-body` | `display:flex`<br>`height:85px; margin:0 10px; width:4px`<br>`display:flex; flex-direction:column; justify-content:space-between`<br>`display:flex; flex-direction:column; gap:6px` | `text-transform:uppercase`<br>`background:linear-gradient(180deg,#0f0 0,#e3b617 32.5%,#ff3d00 98.5%); border-radius:5px`<br>`border-radius:50%` |
| `indicator-led-tooltip-header` | `align-items:center; display:flex; justify-content:space-between`<br>`align-items:center; display:flex; gap:16px; padding:8px`<br>`height:8px; width:8px`<br>`align-items:center; display:flex` | `background-color:gray; border-radius:50%`<br>`background-color:#44d62c` |
| `optionButtonGroup_disabled__Q0Wrc` | — | `background:#ffffff1a`<br>`background:#292929; border:1px solid #44d62c`<br>`color:#ffffff80; cursor:not-allowed`<br>`opacity:.5` |

**⑫ 其它**（51）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `DEVICE_BRIGHTNESS` | — | `background-image:url(../../static/media/icon_config_brightness.7a95c958.svg)`<br>`background-image:url(../../static/media/icon_config_brightness_a.570b75be.svg)` |
| `GLOBAL_BRIGHTNESS` | — | `background-image:url(../../static/media/icon_config_brightness_global.755a1ebf.svg)`<br>`background-image:url(../../static/media/icon_config_brightness_global_a.dadb5e34.svg)` |
| `MonitoringDashboard_disabled__sjQ5Z` | `display:none` | `color:#999`<br>`color:#fff`<br>`opacity:1` |
| `MonitoringDashboard_menuItemDisabled__CtX9E` | — | `color:#666!important; cursor:not-allowed; opacity:.5` |
| `SWITCH_LIGHTING` | — | `background-image:url(data:image/png` |
| `app-disabled` | — | `opacity:.3` |
| `canceled` | — | `color:#fd4949; font-size:12px` |
| `custom-lighting` | `min-width:0; min-width:auto; width:auto` | — |
| `dialClickDisabled` | — | `fill:#44d62c66`<br>`fill:#000c`<br>`fill:#c8323c80` |
| `disabled` | `width:56px`<br>`width:164px`<br>`display:block`<br>`position:relative` | `cursor:default; opacity:.3`<br>`opacity:.5`<br>`opacity:.3`<br>`background-color:#0000; cursor:default`<br>`border:1px solid #515151; cursor:default` |
| `disabled-2` | — | `color:#44d62c; text-transform:none` |
| `disabled-lighting-tab` | — | `opacity:.3` |
| `disabledxy` | `height:15px` | — |
| `enabled` | `display:none` | — |
| `enabled-pointer` | — | — |
| `icon-led-detection` | `align-self:center!important` | — |
| `indicator-led` | `bottom:-5px; height:10px; left:5px; position:absolute; width:50px`<br>`left:36%; top:19px` | `opacity:1`<br>`background-color:#0000` |
| `indicator-led-image__wrapper` | `position:relative`<br>`width:150px`<br>`align-items:center; display:flex; left:53px; position:absolute; top:58px`<br>`height:14.5px; width:14.5px` | — |
| `indicator-led-select_item` | `display:flex; flex-direction:column; gap:10px`<br>`margin:0; width:160px` | — |
| `indicator-led-select_wrapper` | `display:flex; gap:20px` | — |
| `indicator-led__container` | `align-items:center; display:flex; flex-direction:column; gap:20px`<br>`align-items:center; display:inline-flex; gap:4px; height:20px; width:99px`<br>`height:20px; width:20px` | `opacity:1`<br>`color:#44d62c; cursor:pointer; font-family:Roboto,sans-serif; font-size:14px; font-weight:400; line-height:20px`<br>`cursor:pointer; opacity:1` |
| `indicator-led__image` | `width:150px` | — |
| `indicator-led__ledWrapper` | `align-items:center; display:flex; left:53px; position:absolute; top:58px`<br>`height:14.5px; width:14.5px` | — |
| `indicator-led__link` | `align-items:center; display:inline-flex; gap:4px; height:20px; width:99px` | `opacity:1` |
| `indicator-led__link-icon` | `height:20px; width:20px` | `cursor:pointer; opacity:1` |
| `indicator-led__link-text` | — | `color:#44d62c; cursor:pointer; font-family:Roboto,sans-serif; font-size:14px; font-weight:400; line-height:20px` |
| `keymapbar-enabled` | `justify-content:left; margin:0 0 0 20px`<br>`display:flex`<br>`flex:1 1 auto`<br>`display:none` | `color:#ccc` |
| `label-total-led-count` | `align-self:center` | — |
| `light-container--disabled` | — | `opacity:.3` |
| `no-of-led-dropdown` | `height:27px; margin:0!important; width:62px`<br>`align-items:center!important; display:inline-flex!important` | — |
| `number-led-container` | `align-items:center; display:flex; gap:10px`<br>`height:27px; margin:0!important; width:62px`<br>`align-items:center!important; display:inline-flex!important`<br>`height:27px; margin:0!important; position:relative!important; width:62px` | `border:1px solid #5d5d5d`<br>`opacity:.3`<br>`text-align:left` |
| `number-led-container--with-closed` | `width:80px` | — |
| `number-led-stepper` | `height:27px; margin:0!important; position:relative!important; width:62px`<br>`height:25px!important; width:60px!important` | `text-align:left` |
| `number-led-stepper--disabled-decrease` | — | `opacity:.3` |
| `number-led-stepper--disabled-increase` | — | `opacity:.3` |
| `number-led-stepper--stepper` | — | `border:1px solid #5d5d5d` |
| `number-led-wrapper` | `align-items:center; display:flex; gap:10px` | — |
| `port-items-led-image` | `display:flex; flex-direction:row`<br>`flex:1 1`<br>`align-items:center; display:inline-flex; height:27px`<br>`position:relative` | `transition:all .3s`<br>`font-family:Roboto,sans-serif; font-size:14px`<br>`color:#707070; cursor:pointer`<br>`color:#44d62c`<br>`background-color:#44d62c` |
| `powerSaving-wrap--disabled` | — | `opacity:30%` |
| `remap-2-disabled` | — | `color:#c8323c` |
| `reset-oled-download` | `align-items:center; display:flex; justify-content:center; position:relative`<br>`height:20px; width:20px`<br>`height:0; left:-255px; padding:20px; position:absolute; top:40px; width:300px`<br>`height:auto` | `background-image:url(../../static/media/icon_download_animated.c25fe72c.svg); background-repeat:no-repeat; background-size:cover`<br>`background:#111; border:1px solid #5d5d5d; opacity:0; transition:height 0s,visibility 0s,opacity .1s linear`<br>`opacity:1`<br>`text-transform:uppercase`<br>`background-color:#5d5d5d; border:none` |
| `shortcutButton-filled` | — | `background-color:#292929` |
| `small-twoway-lighting` | `height:32px; padding:5px`<br>`height:22px; padding:3px 10px; position:relative` | `background-color:#111; border:1px solid #5d5d5d; border-radius:16px; transition:border-color .2s,background-color .5s`<br>`background-color:#0000001a`<br>`border-radius:11px` |
| `static-lighting-active` | `display:block; height:11px; margin:3.75px; position:absolute; width:11px` | `border:2px solid #fff; border-radius:5px; box-shadow:0 0 2px 1px rgba(0,0,0,.702),inset 0 0 2px 1px rgba(0,0,0,.702)` |
| `tip-disabled` | `left:25%; max-width:300px; padding:8px 10px; position:absolute; top:40%` | `background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-family:Roboto; line-height:16px; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `total-LED-count` | `align-items:center; display:grid; grid-template-columns:150px auto auto; height:27px`<br>`align-self:center`<br>`align-self:center; width:50px`<br>`align-self:center!important` | `font-family:Roboto,sans-serif; font-size:14px` |
| `twoway-lighting` | `height:36px; padding:5px`<br>`height:26px; padding:5px 10px; position:relative` | `background-color:#111; border:1px solid #5d5d5d; border-radius:18px; transition:border-color .2s,background-color .5s`<br>`background-color:#0000001a`<br>`border-radius:13px` |
| `value-total-led-count` | `align-self:center; width:50px` | — |
| `wave-dir` | `align-items:start; overflow:visible; overflow:initial`<br>`width:51px`<br>`height:27px` | `background-color:#0000; border:1px solid #0000; transition:border .2s`<br>`background-color:#44d62c`<br>`border:1px solid #44d62c; transition:border 0s`<br>`background-color:#111`<br>`background-color:#ffffff1a` |
| `wrapper-disabled` | — | `opacity:1` |
| `y-enabled` | `height:114px`<br>`bottom:25px; display:block` | `opacity:1`<br>`opacity:0` |

## 3. 功能项

该界面对应的雷云文案 key，共 **131** 条（中文为雷云原文）：

| key | 中文 | English |
|---|---|---|
| `AUDIO_METER` | 音频计 | Audio Meter |
| `BREATHING` | 呼吸效果 | Breathing |
| `BREATHING_GREEN` | 绿色的呼吸效果 | Breathing Green |
| `BRIGHTNESS` | 亮度 | Brightness |
| `BRIGHTNESS_BATTERY_WARNING` | *此设置可能会降低电池续航时间 | *This setting may reduce battery life |
| `BRIGHTNESS_DOWN` | 调低亮度 | Decrease Brightness |
| `BRIGHTNESS_GLOBAL` | Brightness Global | Brightness Global |
| `BRIGHTNESS_GLOBAL_DESC` | 一次性调整 Razer Synapse 雷云中所有设备的亮度。 | Adjust the brightness level of all devices on Razer Synapse in one go |
| `BRIGHTNESS_GLOBAL_DESC_AT_LEAST_ONE_LED_DEVICE` | 此功能需要至少一个支持 Razer Synapse 雷云且配有 LED 的设备。 | At least one Razer Synapse enabled device with LED is required for this feature |
| `BRIGHTNESS_HEADER` | 亮度 | BRIGHTNESS |
| `BRIGHTNESS_TOGGLE` | 开关灯光 | Brightness Toggle |
| `BRIGHTNESS_TOOLTIP` | 调整设备灯光的亮度。 | Adjust the brightness of your device’s lighting. |
| `BRIGHTNESS_UP` | 调高亮度 | Increase Brightness |
| `BRIGHTNESS_WHEN_INACTIVE` | 启用时的亮度 | Brightness when inactive |
| `CHROMA` | CHROMA 幻彩 | Chroma |
| `CHROMAHDK_HEADER` | CHROMA 幻彩 HDK | CHROMA HDK |
| `CHROMA_APP` | Chroma 应用 | Chroma App |
| `CHROMA_APPS` | Chroma 幻彩应用 | Chroma Apps |
| `CHROMA_APPS_TOUR_CONTENT_1` | 借助 Razer Chroma™ 雷蛇幻彩 SDK，游戏开发人员可以创建智能灯光效果，让灯光可根据游戏内事件变化，例如在爆炸时闪烁、定义各种生命值状态下的颜色等。 | With the Razer Chroma™ SDK, game devs can create smart lighting effects that react to in-game events, such as flashes for explosions, health status colors, and much more. |
| `CHROMA_APP_PARTNERS_INSTRUCTION` | 开始用以下工具创建高级自定义灯光效果... | Start Creating Your Advanced Custom Lighting Effects with... |
| `CHROMA_APP_TUTORIAL_RESET_MSG` | 重置教程后，在下次启动 Razer Chroma 雷蛇幻彩时，你可以再次查看教程。 | Resetting the tutorial will allow you to view it again the next time you launch Razer Chroma. |
| `CHROMA_BRIGHTNESS_HEADER` | CHROMA 幻彩亮度 | CHROMA BRIGHTNESS |
| `CHROMA_CONNECT_INSTRUCTION` | 了解如何借助以下工具连接其他 Chroma 幻彩合作伙伴的设备 | Learn how to connect with other Chroma Partners devices using... |
| `CHROMA_EFFECT_DETAILS` | CHROMA 幻彩效果详情 | CHROMA EFFECT DETAILS |
| `CHROMA_HDK` | Chroma 幻彩 HDK | Chroma HDK |
| `CHROMA_IMMERSIVE_EXPERIENCE` | Chroma Immersive Experience | Chroma Immersive Experience |
| `CHROMA_IMMERSIVE_TOOLTIP_CONTENT` | 应用一种能够同步 RGB 的模式，RGB 可实时响应每个瞬间。 | Applying a mode that synchronized RGB that responds to every moment. |
| `CHROMA_INDICATOR_TIP_1` | Chroma 幻彩指示器可帮助你确定哪个应用当前正在控制 Chroma 幻彩设备的灯光效果 | The Chroma indicator helps you identify which app is currently controlling your Chroma device's  lighting |
| `CHROMA_INDICATOR_TIP_2` | Chroma 幻彩应用的优先级高于“快速效果”和“高级效果” | Chroma Apps have a higher priority over Quick Effects and Advanced Effects |
| `CHROMA_LIGHTING_OFF` | 关闭 | Off |
| `CHROMA_PROFILE` | Chroma Profile | Chroma Profile |
| `CHROMA_RGB` | CHROMA 幻彩 RGB | Chroma RGB |
| `CHROMA_RGB_MSG` | 为避免冲突，我们建议使用下面的链接在你的操作系统中禁用 Windows 动态照明。 | To avoid conflict, we recommend disabling Windows Dynamic Lighting on your Operating System using the link below. |
| `CHROMA_RGB_TITLE` | 通过 Synapse 雷云允许 Chroma 幻彩 RGB 功能控制你的设备。 | Allows your device to be controlled by Chroma RGB via Synapse. |
| `CHROMA_STUDIO` | 幻彩控制室 (Chroma Studio) | Chroma Studio |
| `CHROMA_STUDIO_PROMPT` | 系统会根据幻彩控制室 (Chroma Studio) 的设置自动录制预览。你可以按某些键、点击鼠标来制作动画效果。是否要立即开始录制？ | The system will automatically record the preview from your Chroma Studio settings. You can press some keys, click the mouse to create animation effects. Do you want to start recording now? |
| `CHROMA_VISUALIZER_ACTIVE` | 幻彩可视化工具 (Chroma Visualizer) 已启用 | Chroma visualizer active |
| `CHROMA_VISUALIZER_ACTIVE_DESC` | 幻彩可视化工具 (Chroma Visualizer) 已启用，但只有当它在优先级列表中的排名靠前时，其效果才会显示。 | Chroma Visualizer is on, but its effects show only when it’s higher in the priority list. |
| `CHROMA_VISUALIZER_IS_DISABLED` | 幻彩可视化工具 (Chroma Visualizer) 已禁用 | Chroma Visualizer is disabled |
| `CHROMA_VISUALIZER_RUNNING_MESSAGE` | 设备的灯光效果当前由 {{app}} 应用程序<br>	控制。 | The {{app}} app is currently controlling your<br>	device's lighting. |
| `COLOR` | 颜色 | Color |
| `COLOR_DROP_NAME` | 颜色 {{num}} | Color {{num}} |
| `COLOR_GAMUT` | 色域 | Color Gamut |
| `COLOR_GAMUT_TOOLTIP` | 此设备可生成的色彩范围。 | The range of colors that can be produced on this device. |
| `COLOR_GAMUT_WARNING` | 当色域不是原生色域时，某些设置会被禁用。 | Some settings are disabled when Color Gamut is not Native. |
| `COLOR_PROFILE_HEADER` | 颜色配置文件 | COLOR PROFILE |
| `COLOR_PROFILE_RESTORE_PROFILES_WARNING` | 系统中缺少预设系统颜色配置文件。  | Preset System Color Profile missing from system.  |
| `COLOR_PROFILE_RESTORE_PROFILES_WARNING_LINK` | 恢复配置文件 | Restore Profile(s). |
| `COLOR_PROFILE_TOOLTIP` | 根据自己的偏好，从不同的白平衡中选择一种。一般来说，白天建议使用较冷的色调，晚上建议使用较暖的色调。 | Select from different shades of white to best suit your preference. In general, cooler hues are recommended during the day and warmer hues are recommended at night. |
| `COLOR_TEMPERATURE_HEADER` | 色温 | COLOR TEMPERATURE |
| `DIM_KEYBOARD_LIGHTING_DESC` | 以电池供电时，在无活动（分钟）后，设备将会变暗。 | Device will turn dim after (mins) of inactivity when running on battery. |
| `DIM_KEYBOARD_LIGHTING_TIPS` | 设置设备闲置多长时间后调暗灯光效果。<br><br> 当设备使用无线连接且不处于充电状态时，调暗灯光功能可起作用。 | Set how long the device should be idle before it will dim the lighting.<br><br>This dim lighting function only works when the device using wireless connection and not being charged. |
| `DIM_LIGHTING_DESC` | 处于无线模式时，在闲置以下时间（分钟）后调暗灯光效果 | When wireless, dim lighting if idle for (minutes) |
| `DIM_LIGHTING_DESC_V2` | 系统在闲置（分钟）后，设备亮度将降至 20%。 | Device dims to 20% brightness after (mins) of system inactivity. |
| `DIM_LIGHTING_HEADER` | 暗光效果 | DIM LIGHTING |
| `DIM_LIGHTING_TOOLTIP` | 设置设备闲置多长时间后调暗灯光效果  | Set how long the device should be idle before it will dim the lighting  |
| `DIM_LIGHTING_TOOLTIP_V2` | 选择系统闲置多长时间后亮度会降至 20%。<br><br>此功能仅在设备亮度高于 20% 时激活。<br><br>当 Razer Synapse 雷云运行时，亮度会随系统活动自动恢复。若没有恢复，请按下设备上的任意按键以恢复亮度。<br><br>这些设置保存在设备上，并应用于所有配置文件。 | Choose how long the system should remain idle before the lighting dims to 20% brightness.<br><br>This feature only activates when the device brightness is set above 20%.<br><br>Brightness restores with system activity when Razer Synapse is running. Otherwise, press any button on the device to restore it.<br><br>These settings are saved to your device and are applied to all profiles. |
| `EFFECT` | 效果 | EFFECT |
| `EFFECTS` | 效果 | EFFECTS |
| `EFFECTS_APPLIED` | 应用的效果 | EFFECTS APPLIED |
| `EFFECTS_BLE_TOOLTIP` | 从预设列表中选用一种效果来自定义设备的灯光效果。使用蓝牙连接时可用的效果有限。 | Customize your device’s lighting effect from a list of presets.Limited effects on Bluetooth. |
| `EFFECTS_COMPRESSOR_TOOLTIP` | 调节麦克风输入量以便在声音太大时降低其音量。你可以从这里调整以下属性：<br><br>阈值 <br>设置压缩器何时启动。任何高于阈值的声音都会降低音量或静音。<br><br>增益<br>增加所处理的输出音量，以便在压缩后仍能保持其清晰度。<br><br>压缩比<br>设置压缩强度。<br><br>启动<br>确定如果麦克风输入高于阈值，多快会启动压缩。<br><br>释放<br>确定多快会压缩麦克风输入。 | Modulate the amount of mic input to reduce its volume when it gets too loud. The following properties can be adjusted from here:<br><br>Threshold<br>Sets when the compressor will engage. Any sound that registers above the threshold will be reduced or muted.<br><br>Gain<br>Increases the volume of the processed output to maintain its definition even after compression.<br><br>Ratio<br>Sets the intensity of the compression.<br><br>Attack<br>Determines how quickly the compression will engage if the mic input registers above the threshold.<br><br>Release<br>Determines how fast the mic input will be compressed. |
| `EFFECTS_DECAY` | 减弱 | Decay |
| `EFFECTS_NOISE_GATE_TOOLTIP` | 控制麦克风输入量，以便过滤背景噪音，使你的声音更为清晰。你可以调整以下属性：<br><br>阈值<br>设置噪声门何时打开。任何低于阈值的声音都会降低音量或静音。<br><br>减少<br>麦克风输入低于阈值时静音或减少的音量。<br><br>启动<br>确定如果麦克风输入低于阈值，多快会打开噪声门。<br><br>释放<br>确定多快会降低麦克风输入音量。 | Control the amount of mic input to cut out background noise and completely isolate your voice. The following properties can be adjusted:<br><br>Threshold<br>Sets when the Noise Gate will open. Any sound that registers below the threshold will be reduced or muted.<br><br>Reduction<br>The amount of reduction or muting of sound if the mic input registers below the threshold.<br><br>Attack<br>Determines how quickly the noise gate will be opened if the mic input registers below the threshold.<br><br>Release<br>Determines how fast the mic input will be reduced. |
| `EFFECTS_TOOLTIP` | 从预设列表中自定义设备的灯光效果，并将其同步到支持所选灯光效果的其他 Razer Chroma 雷蛇幻彩设备。 | Customize your device’s lighting effect from a list of presets, and synchronize it with other Razer Chroma devices that support the selected lighting effect. |
| `EFFECTS_TOOLTIP_RADIO` | 使用提供的任一预设更改设备的灯光效果。 | Change your device’s lighting effect using any of the provided presets. |
| `EFFECTS_TOOLTIP_RADIO_OPTION` | 从可用预设中选用一种效果来自定义设备的灯光效果。 | Customize your device's lighting effects from the available presets. |
| `EFFECT_SPEED` | 灯光效果速度 | Effect Speed |
| `FIRE` | 火焰效果 | Fire |
| `IDLE_EFFECT` | 闲置效果 | IDLE EFFECT |
| `IDLE_EFFECT_DESC` | 当充电板空闲或未为设备充电时会激活此灯光效果。 | This effect is active when the charging pad is idle or not charging a device. |
| `IDLE_EFFECT_TOOLTIP` | 自定义充电板空闲或未为设备充电时的灯光效果。 这些设置存储在充电板上。即使 Razer Synapse 雷云没有运行，只要设备有插入，那么就算设备处于空闲状态，这些设置仍会继续生效。 | Customize the lighting effect when the charging pad is idle or not charging a device. These settings are stored on the charging pad and will remain active while idle (and plugged in), even when Razer Synapse is not running. |
| `KEY_LIGHTS` | 补光灯 | KEY LIGHTS |
| `LIGHTING` | 灯光 | Lighting |
| `LIGHTING_BRIGHNESS` | 灯光亮度 | Lighting Brighness |
| `LIGHTING_BRIGHNESS_DOWN` | 调低亮度 | Brighness down |
| `LIGHTING_BRIGHNESS_UP` | 调高亮度 | Brighness up |
| `LIGHTING_CALIBRATION` | 灯光校准 | LIGHTING CALIBRATION |
| `LIGHTING_CALIBRATION_DESC` | 灯光必须经过校准，其效果才能正常显示。 | Calibration is required for lighting effects to work correctly. |
| `LIGHTING_CALIBRATION_STEP_1` | 散热器是如何安装的？ | How is the radiator installed? |
| `LIGHTING_CALIBRATION_STEP_2A` | 每个风扇都以特定的颜色点亮。 | Each fan is lit in a specific color. |
| `LIGHTING_CALIBRATION_STEP_2B` | 请为每个风扇选择一种颜色。 | Please select the color for each fan. |
| `LIGHTING_CALIBRATION_STEP_3A` | 每个风扇的 LED 沿特定方向旋转。 | Each fan's LED rotates in a specific direction. |
| `LIGHTING_CALIBRATION_STEP_3B` | 为每个风扇选择 LED 的旋转方向。 | Select which direction applies to each fan. |
| `LIGHTING_CALIBRATION_STEP_4A` | 有几个 LED 会以白色点亮，以帮助用户识别风扇的旋转方向或所安装的角度。 | We light up a few LEDs in white to help identify the rotation or angle the fans were installed. |
| `LIGHTING_CALIBRATION_STEP_4B` | 点击每个风扇上以白色点亮的 LED 的位置。 | Click on the location of the LEDs for each fan that are lighted in white. |
| `LIGHTING_CALIBRATION_TOOL_INTRO` | 此工具会指引你根据产品的安装方式设置产品。 | This tool will guide you in setting up the product based on how it's installed. |
| `LIGHTING_DEVICE_COMPATIBLE_DEVICES` | 查看兼容设备 | Compatible devices |
| `LIGHTING_DEVICE_HAS_NETWORK_ACCESS` | 请确保你的电脑可以访问网络。 | Please ensure that your computer has network access. |
| `LIGHTING_DEVICE_INTRO` | 请选择您希望添加的设备类型。 | Select the type of device you wish to add. |
| `LIGHTING_DEVICE_MOBILE_QR_DOWNLOAD_APP` | 在你的移动设备上下载并安装以下应用程序。 | Download and install the app on your mobile device. |
| `LIGHTING_DEVICE_MOBILE_QR_FAQ` | 需要设备方面的帮助？ | Need help with your device? |
| `LIGHTING_DEVICE_NO_DEVICE_FOUND` | 未找到设备 | NO DEVICES FOUND |
| `LIGHTING_DEVICE_NO_DEVICE_FOUND_DESC` | 确保你的 Razer 雷蛇设备处于配对模式并且 WiFi 连接处于有效状态，然后再次扫描。 | Make sure your Razer device is in pairing mode and your WiFi connection is active, then scan again. |
| `LIGHTING_DEVICE_NO_NETWORK_DETECTED` | 未检测到网络 | NO NETWORK DETECTED |
| `LIGHTING_DEVICE_PAIRING_FAILED` | 无法添加设备 | UNABLE TO ADD DEVICE |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_1` | 请重置设备，然后重试。若要重置，请执行以下操作： | Please reset your device and try again. To reset: |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_2` | 1.在 Razer 雷蛇幻彩 RGB 补光灯的电源开启时，按住“重置”按钮 10 秒钟。 | 1. With the Razer Key Light switched on, press and hold the Reset button for 10 seconds. |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_3` | 2.Razer 雷蛇幻彩 RGB 补光灯将短暂  | 2. The Razer Key Light will briefly  |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_4` | 闪烁白色 | flash in white |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_5` | 然后变为 | and change to |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_6` | 蓝色闪烁 | flashing blue |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_7` | 表示设备已重置。 | to indicate that the device has been reset. |
| `LIGHTING_DEVICE_PAIRING_SUCCESS_ADD_MORE` | 添加另一个设备 | ADD ANOTHER DEVICE |
| `LIGHTING_DEVICE_PAIRING_SUCCESS_HEADER` | 现在，你已连接到你的新 Razer 雷蛇设备。 | You're now connected to your new Razer device. |
| `LIGHTING_DEVICE_PAIRING_SUCCESS_HEADER_1` | 添加另一个 Razer 雷蛇设备或开始自定义！ | Add another Razer device or start customizing! |
| `LIGHTING_DEVICE_SCANNED_DEVICES_DEVICES_ON_THE_NETWORK` | 网络中的设备 | DEVICES ON THE NETWORK |
| `LIGHTING_DEVICE_SCANNED_DEVICES_SELECT_DEVICE` | 选择你的 Razer 雷蛇设备 | Select your Razer device |
| `LIGHTING_DEVICE_SCANNING_MSG` | 确保 WiFi 连接处于有效状态并且你的 Razer 雷蛇设备已开机。 | Make sure your Razer device is in pairing mode and your WiFi connection is active. |
| `LIGHTING_DEVICE_SCANNING_SUCCESS_NEW_DEVICE_TOOLTIP` | 此处列出的每个设备的最后 4 位数字对应于产品包装上印刷的 MAC 地址。 | The last 4 digits of each device listed here correspond with the MAC address printed on the product packaging. |
| `LIGHTING_DEVICE_SETUP_USING_MOBILE_PHONE` | 或者，在你的手机上设置并使用该设备。 | Alternatively, set up and use the device on your mobile phone. |
| `LIGHTING_DEVICE_TAKE_CONTROL_DESC` | 另一个 Razer 雷蛇应用程序正在控制你的设备。 | Another Razer app is controlling your device. |
| `LIGHTING_DEVICE_TAKE_CONTROL_DESC_1` | 另一个 Razer 雷蛇应用程序正在控制你的设备。 | Another Razer app is controlling your device. |
| `LIGHTING_DEVICE_TAKE_CONTROL_DESC_2` | 你想让 Synapse 雷云取回控制权吗？ | Do you want to take control from Synapse? |
| `LIGHTING_DEVICE_WIFI_NOT_ENABLED_ENABLE_WIFI` | 请在 中启用 WiFi | Please enable WiFi in |
| `LIGHTING_DEVICE_WIFI_NOT_ENABLED_HEADER` | WIFI 未启用 | WIFI NOT ENABLED |
| `LIGHTING_DEVICE_WIFI_NOT_ENABLED_WINDOWS_SETTINGS` | Windows 设置 | Windows Settings |
| `LIGHTING_EFFECT_AUTOMATION` | 要设置提起或放下耳机的灯光效果，请在{{automations}}中使用 Chroma 幻彩设置。 | To set effects for picking up or putting down your headset, use Chroma settings in {{automations}}. |
| `LIGHTING_LINK_TOOLTIP` | 关联 | Link |
| `LIGHTING_ON_BATTERTY` | 使用电池时 | On Battery |
| `LIGHTING_PLUGGED_IN` | 连接电源时 | Plugged In |
| `LIGHTING_UNLINK_TOOLTIP` | 取消关联 | Unlink |
| `REACTIVE` | 响应效果 | Reactive |
| `REACTIVE_WARNING` | Razer Synapse 雷云无法检测到所选效果的兼容 Razer 雷蛇鼠标。 | Razer Synapse was unable to detect a compatible Razer mouse for the selected effect. |
| `RIPPLE` | 涟漪效果 | Ripple |
| `RIPPLE_DELETE` | 波纹删除 | Ripple Delete |
| `SPECTRUM_CYCLING` | 光谱循环 | Spectrum Cycling |
| `STARLIGHT` | 星光效果 | Starlight |
| `STATIC` | 静态效果 | Static |
| `STATIC_GREEN` | 绿色常亮 | Static Green |
| `WAVE` | 波浪效果 | Wave |

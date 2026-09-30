# 性能（TAB_PERFORMANCE）

> 本页由雷云自己的产品模块提取；本文件只记录原版事实，不记录本项目实现状态。
> 证据工具：`node .ref/tools/gen-screen-docs.js`

## 1. 出现在哪些设备上

| 设备 | productId | 设备类型 | 该设备标签页顺序 |
|---|---|---|---|
| Razer DeathAdder V3 Pro | 182 | 鼠标 | 第 2 个：自定义 · 性能 · 正在配对 · 校准 · 电源 · 滚动 |
| Blackwidow V4 Pro | 653 | 键盘 | 第 2 个：自定义 · 性能 · 灯光 · 电源 · 滚动 |

标签页真实文案：**性能**（key `TAB_PERFORMANCE`）

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

**调色板**（按出现次数排序，共统计 1844 处颜色声明）：

| 次数 | 颜色 | 角色 |
|---:|---|---|
| 289 | `#44d62c` | 主色 / 激活态 / 标题 |
| 275 | `#ccc` | 主文字 |
| 157 | `#5d5d5d` | **通用边框** |
| 139 | `#111` | 卡片 / 输入框底色 |
| 132 | `#000` | 按钮文字、描边、顶栏下边线 |
| 108 | `#0000` | 透明 |
| 80 | `#707070` | 次要按钮底色 |
| 70 | `#999` | 次要文字 |
| 68 | `#fff` | 次要按钮文字 |
| 63 | `#fd8611` | 橙色：提示 / 警告 / 分享 |
| 59 | `#222` | 页面底色 / 顶栏底色 |
| 30 | `#ffffff1a` | 半透明白：hover 覆盖 |
| 25 | `#0000004d` | 半透明黑：按钮描边 |
| 23 | `#212121` | 深色文字（浅底上）/ 离线指示 |
| 19 | `#ffffff4d` | 半透明白：按下覆盖 |
| 15 | `#2d2d2d` | **hover 底色** |
| 15 | `#fd4949` | 危险红（亮） |
| 14 | `#c8323c` | 危险红 |
| 14 | `#333` | 开关关闭态底色 |
| 11 | `#515151` | **下拉框边框** |
| 11 | `#4a4a4a` | 帮助图标底色 |
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

#### Razer DeathAdder V3 Pro（productId 182）

该界面共 47 个布局类名，分 5 个分区。

**① DPI 档位区**（21）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `description-stages` | — | `color:#999; line-height:17px` |
| `dpi-xy` | `align-items:center; height:78px; justify-content:center; left:-9px; position:relative; width:calc(100% + 18px)`<br>`flex-direction:column` | — |
| `one-stage` | `display:none` | — |
| `stage` | `align-items:center; display:flex; flex:0 0 auto; height:68px; position:relative`<br>`display:none; height:6px; left:0; margin:0 auto; position:absolute; right:0; top:3px; width:8px`<br>`display:block`<br>`display:none` | `border-radius:3px; transition:background-color .2s`<br>`border-bottom:2px solid #44d62c`<br>`border-bottom:none; border-top:2px solid #44d62c`<br>`color:#ccc; font-size:14px; line-height:17px; text-align:center; transition:color .2s,font-weight .2s`<br>`background-color:#222` |
| `stage-circle-1` | — | `background-color:#ff1a1a` |
| `stage-circle-2` | — | `background-color:#24ff00` |
| `stage-circle-3` | — | `background-color:#006fff` |
| `stage-circle-4` | — | `background-color:#00edff` |
| `stage-circle-5` | — | `background-color:#fff700` |
| `stage-control` | `height:27px; position:relative`<br>`flex:0 0 auto!important; margin:0; width:50%`<br>`align-items:end; justify-content:flex-end`<br>`flex:0 0 auto!important` | `text-align:right` |
| `stage-header` | `height:20px; position:relative`<br>`bottom:0; left:50px; position:absolute`<br>`bottom:0; left:110px; position:absolute`<br>`bottom:0; left:375px; position:absolute` | — |
| `stage-input` | `height:26px; width:60px` | `background-color:#111; border:1px solid #5d5d5d; color:#ccc; font-size:14px; line-height:19px; text-align:center; transition:border .3s`<br>`border:1px solid #44d62c` |
| `stage-ordinal` | `height:30px; position:relative; width:30px`<br>`left:0; margin:0 auto; position:absolute; right:0; top:10px; width:7px`<br>`height:7px; margin:3px auto 0; width:9px` | `background-color:#44d62c`<br>`opacity:.3`<br>`background-color:#222; border-radius:50%`<br>`color:#ccc`<br>`background-repeat:no-repeat` |
| `stage-ordinal-number` | `left:0; margin:0 auto; position:absolute; right:0; top:10px; width:7px` | `color:#111` |
| `stage-title` | — | `color:#ccc; font-size:14px; line-height:17px; text-align:center; transition:color .2s,font-weight .2s`<br>`color:#212121; font-weight:700` |
| `stage-triangle-1` | — | `background-image:url(../../static/media/red.99a70e7d.svg)` |
| `stage-triangle-2` | — | `background-image:url(../../static/media/green.9abd2946.svg)` |
| `stage-triangle-3` | — | `background-image:url(../../static/media/blue.c01cdc30.svg)` |
| `stage-triangle-4` | — | `background-image:url(../../static/media/cyan.02647853.svg)` |
| `stage-triangle-5` | — | `background-image:url(../../static/media/yellow.a30299c6.svg)` |
| `stages` | `display:flex; flex-direction:column`<br>`align-items:center; display:flex; flex:0 0 auto; height:68px; position:relative`<br>`position:relative`<br>`display:block` | `border-radius:3px; transition:background-color .2s`<br>`border-bottom:2px solid #44d62c`<br>`border-bottom:none; border-top:2px solid #44d62c`<br>`background-color:#222`<br>`background-color:#1e1e1e` |

**② 轮询率**（10）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `customize-polling-rate-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:72px`<br>`min-width:90px` | `background-color:#222; border:1px solid #5d5d5d; border-radius:3px; color:#ccc; font-size:14px; text-transform:uppercase`<br>`border-color:#44d62c` |
| `customize-polling-rate-button-color` | `height:6px; width:6px` | `border-radius:50%` |
| `hyperPolling` | `padding:30px 30px 30px 40px` | — |
| `hyperpolling` | — | `background-image:url(../../static/media/icon-multidevicepairing2.af8657a6.svg)` |
| `hyperpolling-span-hover` | — | `color:#44d62c` |
| `ingame-polling-rate` | `margin:12px 0` | `font-family:RazerF5; font-size:14px` |
| `polling-btn-set` | `display:flex; flex-wrap:wrap; gap:10px 10px` | — |
| `polling-learn-more` | `align-items:center; display:inline-flex`<br>`height:16px; width:16px` | `cursor:pointer` |
| `polling-rate` | `position:relative`<br>`width:100px` | — |
| `polling-warn` | — | `opacity:.7` |

**③ 抬升距离 / 非对称中止**（2）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `lift-off` | `padding:20px; position:relative; width:290px`<br>`left:218px`<br>`right:0`<br>`display:flex` | `background-color:#111; border-radius:5px; color:#ccc; font-size:14px; line-height:17px`<br>`color:#44d62c; font-family:RazerF5; font-size:16px; text-transform:uppercase` |
| `lift-off-wrapper` | `max-width:290px` | — |

**④ 灵敏度与加速度**（6）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `analog_sensitivity_text` | `height:17px; position:relative`<br>`position:relative; top:1px` | `color:#ccc; font-size:14px` |
| `configure-sensitivity` | `margin:28px 0`<br>`margin:20px 0 10px` | `cursor:pointer`<br>`color:#ccc; cursor:pointer; font-family:Roboto; font-size:14px; text-align:left` |
| `description-cycleup-sensitivity-map` | `display:flex; flex-direction:column; height:140px; justify-content:space-between`<br>`margin:20px 0 10px` | `font-size:14px; line-height:17px`<br>`color:#999; line-height:17px`<br>`cursor:pointer` |
| `icon-sensitivity-xy` | `display:block`<br>`height:32px; width:32px`<br>`display:none`<br>`position:relative` | `background-position:50%; background-repeat:no-repeat`<br>`cursor:pointer`<br>`opacity:.3`<br>`background-image:url(../../static/media/icon_sensitivity_xy_disabled.3a6ee34e.svg)`<br>`background-image:url(../../static/media/icon_sensitivity_xy.b9eb5286.svg)` |
| `rapid-trigger-sensitivity-section` | — | `transition:none!important` |
| `two-way-tab-sensitivity` | `height:72px` | — |

**⑫ 其它**（8）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `MonitoringDashboard_sensorList__PYVVe` | — | `text-align:left` |
| `SENSITIVITY` | — | `background-image:url(../../static/media/icon_config_mouse_sensitivity.cd4e347f.svg)`<br>`background-image:url(../../static/media/icon_config_mouse_sensitivity_a.7f79e597.svg)` |
| `SWITCH_DEVICE_SENSITIVITY` | — | `background-image:url(../../static/media/icon_config_mouse_sensitivity.cd4e347f.svg)`<br>`background-image:url(../../static/media/icon_config_mouse_sensitivity_a.7f79e597.svg)` |
| `dots` | `display:flex`<br>`align-items:flex-start; display:flex; flex-direction:row; gap:12px; height:4px; padding:0`<br>`height:4px; position:relative; width:4px`<br>`align-items:flex-end; display:flex; height:42px` | `background-color:#707070; border-radius:10px; transition:none`<br>`background-color:#fff` |
| `dots3` | `position:relative`<br>`height:90%; position:absolute; right:-8px; width:32px` | `background-image:url(../../static/media/icon_more_default.eb7284c1.svg); border:none; border-radius:13px`<br>`background-color:#2d2d2d; background-image:url(../../static/media/icon_more.fb688d78.svg)`<br>`text-transform:uppercase`<br>`background-color:#000; color:#44d62c`<br>`background-color:#1a1a1a; color:#44d62c` |
| `dots_3` | `height:26px; width:26px` | `background-image:url(../../static/media/icon_more_g.32e1e984.svg); border-color:#222`<br>`background-position:50%; background-repeat:no-repeat; background-size:20px; transition:border-color .2s`<br>`border:1px solid #777`<br>`border:1px solid #44d62c` |
| `dots_3_show` | `height:26px; width:26px` | `background-position:50%; background-repeat:no-repeat; background-size:20px; transition:border-color .2s`<br>`background-image:url(../../static/media/icon_more.fb688d78.svg); border:1px solid #44d62c` |
| `performance-custom-boost` | — | `opacity:1` |

#### Blackwidow V4 Pro（productId 653）

该界面共 45 个布局类名，分 4 个分区。

**① DPI 档位区**（21）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `description-stages` | — | `color:#999; line-height:17px` |
| `dpi-xy` | `align-items:center; height:78px; justify-content:center; left:-9px; position:relative; width:calc(100% + 18px)`<br>`flex-direction:column` | — |
| `one-stage` | `display:none` | — |
| `stage` | `align-items:center; display:flex; flex:0 0 auto; height:68px; position:relative`<br>`display:none; height:6px; left:0; margin:0 auto; position:absolute; right:0; top:3px; width:8px`<br>`display:block`<br>`display:none` | `border-radius:3px; transition:background-color .2s`<br>`border-bottom:2px solid #44d62c`<br>`border-bottom:none; border-top:2px solid #44d62c`<br>`color:#ccc; font-size:14px; line-height:17px; text-align:center; transition:color .2s,font-weight .2s`<br>`background-color:#222` |
| `stage-circle-1` | — | `background-color:#ff1a1a` |
| `stage-circle-2` | — | `background-color:#24ff00` |
| `stage-circle-3` | — | `background-color:#006fff` |
| `stage-circle-4` | — | `background-color:#00edff` |
| `stage-circle-5` | — | `background-color:#fff700` |
| `stage-control` | `height:27px; position:relative`<br>`flex:0 0 auto!important; margin:0; width:50%`<br>`align-items:end; justify-content:flex-end`<br>`flex:0 0 auto!important` | `text-align:right` |
| `stage-header` | `height:20px; position:relative`<br>`bottom:0; left:50px; position:absolute`<br>`bottom:0; left:110px; position:absolute`<br>`bottom:0; left:375px; position:absolute` | — |
| `stage-input` | `height:26px; width:60px` | `background-color:#111; border:1px solid #5d5d5d; color:#ccc; font-size:14px; line-height:19px; text-align:center; transition:border .3s`<br>`border:1px solid #44d62c` |
| `stage-ordinal` | `height:30px; position:relative; width:30px`<br>`left:0; margin:0 auto; position:absolute; right:0; top:10px; width:7px`<br>`height:7px; margin:3px auto 0; width:9px` | `background-color:#44d62c`<br>`opacity:.3`<br>`background-color:#222; border-radius:50%`<br>`color:#ccc`<br>`background-repeat:no-repeat` |
| `stage-ordinal-number` | `left:0; margin:0 auto; position:absolute; right:0; top:10px; width:7px` | `color:#111` |
| `stage-title` | — | `color:#ccc; font-size:14px; line-height:17px; text-align:center; transition:color .2s,font-weight .2s`<br>`color:#212121; font-weight:700` |
| `stage-triangle-1` | — | `background-image:url(../../static/media/red.99a70e7d.svg)` |
| `stage-triangle-2` | — | `background-image:url(../../static/media/green.9abd2946.svg)` |
| `stage-triangle-3` | — | `background-image:url(../../static/media/blue.c01cdc30.svg)` |
| `stage-triangle-4` | — | `background-image:url(../../static/media/cyan.02647853.svg)` |
| `stage-triangle-5` | — | `background-image:url(../../static/media/yellow.a30299c6.svg)` |
| `stages` | `display:flex; flex-direction:column`<br>`align-items:center; display:flex; flex:0 0 auto; height:68px; position:relative`<br>`position:relative`<br>`display:block` | `border-radius:3px; transition:background-color .2s`<br>`border-bottom:2px solid #44d62c`<br>`border-bottom:none; border-top:2px solid #44d62c`<br>`background-color:#222`<br>`background-color:#1e1e1e` |

**② 轮询率**（10）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `customize-polling-rate-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:72px`<br>`min-width:90px` | `background-color:#222; border:1px solid #5d5d5d; border-radius:3px; color:#ccc; font-size:14px; text-transform:uppercase`<br>`border-color:#44d62c` |
| `customize-polling-rate-button-color` | `height:6px; width:6px` | `border-radius:50%` |
| `hyperPolling` | `padding:30px 30px 30px 40px` | — |
| `hyperpolling` | — | `background-image:url(../../static/media/icon-multidevicepairing2.af8657a6.svg)` |
| `hyperpolling-span-hover` | — | `color:#44d62c` |
| `ingame-polling-rate` | `margin:12px 0` | `font-family:RazerF5; font-size:14px` |
| `polling-btn-set` | `display:flex; flex-wrap:wrap; gap:10px 10px` | — |
| `polling-learn-more` | `align-items:center; display:inline-flex`<br>`height:16px; width:16px` | `cursor:pointer` |
| `polling-rate` | `position:relative`<br>`width:100px` | — |
| `polling-warn` | — | `opacity:.7` |

**④ 灵敏度与加速度**（6）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `analog_sensitivity_text` | `height:17px; position:relative`<br>`position:relative; top:1px` | `color:#ccc; font-size:14px` |
| `configure-sensitivity` | `margin:28px 0`<br>`margin:20px 0 10px` | `cursor:pointer`<br>`color:#ccc; cursor:pointer; font-family:Roboto; font-size:14px; text-align:left` |
| `description-cycleup-sensitivity-map` | `display:flex; flex-direction:column; height:140px; justify-content:space-between`<br>`margin:20px 0 10px` | `font-size:14px; line-height:17px`<br>`color:#999; line-height:17px`<br>`cursor:pointer` |
| `icon-sensitivity-xy` | `display:block`<br>`height:32px; width:32px`<br>`display:none`<br>`position:relative` | `background-position:50%; background-repeat:no-repeat`<br>`cursor:pointer`<br>`opacity:.3`<br>`background-image:url(../../static/media/icon_sensitivity_xy_disabled.3a6ee34e.svg)`<br>`background-image:url(../../static/media/icon_sensitivity_xy.b9eb5286.svg)` |
| `rapid-trigger-sensitivity-section` | — | `transition:none!important` |
| `two-way-tab-sensitivity` | `height:72px` | — |

**⑫ 其它**（8）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `MonitoringDashboard_sensorList__PYVVe` | — | `text-align:left` |
| `SENSITIVITY` | — | `background-image:url(../../static/media/icon_config_mouse_sensitivity.cd4e347f.svg)`<br>`background-image:url(../../static/media/icon_config_mouse_sensitivity_a.7f79e597.svg)` |
| `SWITCH_DEVICE_SENSITIVITY` | — | `background-image:url(../../static/media/icon_config_mouse_sensitivity.cd4e347f.svg)`<br>`background-image:url(../../static/media/icon_config_mouse_sensitivity_a.7f79e597.svg)` |
| `dots` | `display:flex`<br>`align-items:flex-start; display:flex; flex-direction:row; gap:12px; height:4px; padding:0`<br>`height:4px; position:relative; width:4px`<br>`align-items:flex-end; display:flex; height:42px` | `background-color:#707070; border-radius:10px; transition:none`<br>`background-color:#fff` |
| `dots3` | `position:relative`<br>`height:90%; position:absolute; right:-8px; width:32px` | `background-image:url(../../static/media/icon_more_default.eb7284c1.svg); border:none; border-radius:13px`<br>`background-color:#2d2d2d; background-image:url(../../static/media/icon_more.fb688d78.svg)`<br>`text-transform:uppercase`<br>`background-color:#000; color:#44d62c`<br>`background-color:#1a1a1a; color:#44d62c` |
| `dots_3` | `height:26px; width:26px` | `background-image:url(../../static/media/icon_more_g.32e1e984.svg); border-color:#222`<br>`background-position:50%; background-repeat:no-repeat; background-size:20px; transition:border-color .2s`<br>`border:1px solid #777`<br>`border:1px solid #44d62c` |
| `dots_3_show` | `height:26px; width:26px` | `background-position:50%; background-repeat:no-repeat; background-size:20px; transition:border-color .2s`<br>`background-image:url(../../static/media/icon_more.fb688d78.svg); border:1px solid #44d62c` |
| `performance-custom-boost` | — | `opacity:1` |

## 3. 功能项

该界面对应的雷云文案 key，共 **52** 条（中文为雷云原文）：

| key | 中文 | English |
|---|---|---|
| `CONFIGURE_POLLING_RATE` | 在此设备的性能设置中{{start}}配置轮询率{{end}}. | {{start}}Configure Polling Rate{{end}} on the Performance settings of this device. |
| `CONFIGURE_POLLING_RATE_DEVICE_DISCONNECTED_KEYBOARD_TEXT` | To configure the Polling Rate, ensure your keyboard is connected and installed on Razer Synapse, and only paired with this device. | To configure the Polling Rate, ensure your keyboard is connected and installed on Razer Synapse, and only paired with this device. |
| `CONFIGURE_POLLING_RATE_DEVICE_DISCONNECTED_TEXT` | 要配置轮询率，请确保鼠标已连接并已安装 Razer Synapse 雷云，并且仅与此设备配对。 | To configure the Polling Rate, ensure your device is connected and installed on Razer Synapse, and only paired with this dongle. |
| `CONFIGURE_SENSITIVITY_SETTINGS` | 配置灵敏度等级 | Configure Sensitivity Stages |
| `CURRENT_DPI` | 当前 DPI： | Current DPI: |
| `DPI_STAGE` | DPI 等级 | DPI Stage |
| `DPI_STAGES` | DPI 等级 | DPI STAGES |
| `INGAME_POLLING_RATE_HEADER` | 轮询率智能切换 | SMART POLLING RATE SWITCHER |
| `LIFTOFFDISTANCE` | 抬升距离 | LIFT-OFF DISTANCE |
| `LIFTOFFDISTANCE_1` | 抬升距离 | Lift-off distance |
| `LIFT_OFF_RANGE_DESC` | 如果遇到追踪问题，可以重置鼠标，或者提高抬升范围。 | If you’re experiencing tracking issues, you may either reset your mouse or increase the Lift-off Range. |
| `LIFT_OFF_RANGE_HEADER` | 抬升范围 | LIFT-OFF RANGE |
| `NEW_DPI` | 新 DPI： | New DPI: |
| `POLLING_RATE` | 轮询率 | Polling Rate |
| `POLLING_RATE_BLUETOOTH_INFO` | Bluetooth mode may show unusual polling rates due to how data is transmitted. | Bluetooth mode may show unusual polling rates due to how data is transmitted. |
| `POLLING_RATE_DESC` | 数据在一秒内的更新频率 (Hz)。 | The frequency (Hz) at which your device sends data to your computer. |
| `POLLING_RATE_DUAL_LINK_LIMITED` | 由于当前连接类型的限制，最大轮询率已经降低。 | The maximum polling rate has been reduced because of limitations in the current connection type. |
| `POLLING_RATE_HEADER` | 轮询率 | POLLING RATE |
| `POLLING_RATE_SUPPORTED_BUTTON_COMBINATION_TOOLTIP` | 调整设备向电脑回送报告的频率。较高的频率可以提高整体响应速度并减少输入延迟，但会消耗更多电量并缩短电池续航时间（若适用）。<br><br>所列的可用轮询率取决于连接类型和操作系统。<br><br>更改轮询率热键：按住滚轮点击键的同时按下电源键，即可循环切换不同的轮询率。 | Adjust the frequency at which the device reports back to the PC. Higher frequencies increase overall responsiveness and reduce input latency, but require more power and shorten battery life (if applicable).<br> <br>The list of available polling rates varies depending on the connection type and operating system.<br> <br>Change Polling Rate Hotkey: While holding the scroll-click, press the power button to cycle through different polling rates. |
| `POLLING_RATE_TOOLTIP` | 调整设备向电脑回送报告的频率。 | Adjust the frequency at which the device will report back to the PC. |
| `POLLING_RATE_V2_TOOLTIP` | 调整设备向电脑回送报告的频率。 如果频率更高，则整体响应速度更高、输入延迟更低，但同时，消耗的能量更多、电池续航时间更短（如果适用）。<br> <br>可用的频率取决于连接类型和操作系统。 | Adjust the frequency at which the device will report back to the PC. Higher frequencies increase overall responsiveness and reduce input latency, but consume more energy and reduce battery life (if applicable).<br> <br>The list of available frequencies depend on  the connection type and operating system. |
| `POLLING_RATE_WARN` | 轮询率越高，CPU 占用率越高，因此可能会影响帧率。 | Higher polling rates will result in reduced battery life and higher CPU usage which may impact frame rates. |
| `POLLING_RATE_WARN_NOBATTERY` | 更高的轮询率会导致更高的CPU使用率，从而可能影响帧率。 | Higher polling rates will result in higher CPU usage, which may impact frame rates. |
| `SENSITIVITY` | 灵敏度 | SENSITIVITY |
| `SENSITIVITY_ASSIGN_HEADER` | 指定灵敏度滑块 |  |
| `SENSITIVITY_ASSIGN_INFO` | 指定灵敏度滑块以启用。 | Assign Sensitivity Clutch to enable. |
| `SENSITIVITY_ASSIGN_WARNING_DESCRIPTION` | 要启用此配置，必须首先将左右灵敏度滑块重新映射到多功能按键上。 | To enable this configuration, you must first remap the left and right sensitivity clutch to the multi keys. |
| `SENSITIVITY_CLUTCH` | 灵敏度滑块 | Sensitivity Clutch |
| `SENSITIVITY_DESC` | 鼠标移动的每英寸点数 (DPI) 值。 | The number of dots-per-inch (DPI) of mouse movement. |
| `SENSITIVITY_DESCRIPTION` | 选择要进行配置的拇指控制杆。使用方向键来选择和修改灵敏度。 | Select the thumbstick you want to configure. Use the D-Pad to navigate and modify sensitivity. |
| `SENSITIVITY_HEADER` | 灵敏度 | SENSITIVITY |
| `SENSITIVITY_MAPPING_INFO` | 这一更改将影响所有具有相同功能的按键。 | This change will affect all buttons that shares the same function. |
| `SENSITIVITY_MATCHER` | 灵敏度匹配 | Sensitivity Matcher |
| `SENSITIVITY_MATCHER_ACTION_DESC_1` | 并排握好两个鼠标，然后点击 Razer Synapse 雷云上的任意位置。 | Hold your mice side by side then click anywhere on Razer Synapse. |
| `SENSITIVITY_MATCHER_ACTION_DESC_2` | 缓慢地将鼠标向一个方向移动，直至校准完成。 | Slowly move your mice in one direction until the calibration is complete. |
| `SENSITIVITY_MATCHER_ACTION_DESC_3` | 灵敏度匹配成功。 | Sensitivity matching successful. |
| `SENSITIVITY_MATCHER_CALIBRATION_ERROR` | 校准错误 | CALIBRATION ERROR |
| `SENSITIVITY_MATCHER_CALIBRATION_ERROR_DESC` | 请避免拔出第二个设备。重新开始校准过程，以确保结果准确。 | Please avoid unplugging your second device. Restart the calibration process to ensure accurate results. |
| `SENSITIVITY_MATCHER_DESC` | 创建新的灵敏度匹配校准。 | Create a new sensitivity matcher calibration from a different mouse. |
| `SENSITIVITY_MATCHER_PROFILE_DESC` | 将鼠标的灵敏度与另一个鼠标相匹配 | Match the sensitivity of your mouse to a previously calibrated mouse. |
| `SENSITIVITY_MATCHER_STEP_DESC` | 校准鼠标以模仿另一个鼠标的感觉和移动。 | Calibrate your mouse to mimic the feel and movement of another mouse. |
| `SENSITIVITY_MATCHER_TOOLTIP` | 微调鼠标以模仿另一个鼠标的感觉。需要有第二个鼠标才能使用此功能。 | Fine-tune your mouse to mimic the feel of another mouse. A second mouse is required to use this feature. |
| `SENSITIVITY_STAGES` | 灵敏度等级 | Sensitivity Stages |
| `SENSITIVITY_STAGE_DOWN` | 灵敏度等级降低 | Sensitivity Stage Down |
| `SENSITIVITY_STAGE_UP` | 灵敏度等级提高 | Sensitivity Stage Up |
| `SENSITIVITY_TOOLTIP` | 调整鼠标指针在任意方向的移动速度，或配置其灵敏度等级。 | Adjust the speed of the mouse pointer in any direction or configure its sensitivity stages. |
| `STAGE` | 等级 {x} | Stage |
| `STAGE1` | 等级 1 | Stage 1 |
| `STAGE2` | 等级 2 | Stage 2 |
| `STAGE3` | 等级 3 | Stage 3 |
| `STAGE4` | 等级 4 | Stage 4 |
| `STAGE5` | 等级 5 | Stage 5 |

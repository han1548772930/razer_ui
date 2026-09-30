# 滚动（TAB_SCROLLING）

> 本页由自动脚本从雷云**自己的模块**提取，未做臆测。
> 重新生成：`node .ref/tools/gen-screen-docs.js`

## 1. 出现在哪些设备上

| 设备 | productId | 设备类型 | 该设备标签页顺序 |
|---|---|---|---|
| Razer DeathAdder V3 Pro | 182 | 鼠标 | 第 6 个：自定义 · 性能 · 正在配对 · 校准 · 电源 · 滚动 |
| Blackwidow V4 Pro | 653 | 键盘 | 第 5 个：自定义 · 性能 · 灯光 · 电源 · 滚动 |

标签页真实文案：**滚动**（key `TAB_SCROLLING`）

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

该界面共 36 个布局类名，分 3 个分区。

**① 滚轮档位（阶段）**（33）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `custom-scrollable` | `【@media screen and (max-width:1279px)】overflow-y:hidden` | — |
| `customize-scroll` | `height:inherit; min-width:inherit; overflow-y:visible; overflow-y:initial; position:relative; width:inherit`<br>`display:flex; justify-content:center; left:50%; margin:10px; min-width:150px; position:fixed; width:150px`<br>`left:auto!important; position:relative!important; width:auto!important`<br>`height:275px; position:relative` | `transition:all .3s`<br>`fill:#0000; cursor:pointer`<br>`fill:#44d62c`<br>`fill:#39a029`<br>`fill:none` |
| `description-stages` | — | `color:#999; line-height:17px` |
| `haptic-content__scroll` | `display:flex; justify-content:space-between`<br>`padding:20px 0; width:255px` | `background-color:#292929; border:1px solid #0000; border-radius:4px; color:#ccc; font-size:14px; text-align:center`<br>`font-size:28px` |
| `no-scroll` | `overflow-y:hidden`<br>`overflow:hidden!important` | — |
| `noScrollX` | — | — |
| `one-stage` | `display:none` | — |
| `scroll-item` | `position:relative`<br>`left:6px; right:0`<br>`height:36px; margin:0; width:230px` | `background-color:#111; border:1px solid #5d5d5d`<br>`opacity:1`<br>`opacity:.3`<br>`text-align:left` |
| `scroll-item__title` | `display:flex`<br>`position:relative; right:0; top:0`<br>`width:100%` | `text-transform:uppercase` |
| `scroll-slide` | `display:flex; flex-direction:column; justify-content:space-between`<br>`position:relative`<br>`display:flex`<br>`position:relative; right:0; top:0` | `text-transform:uppercase`<br>`background-color:#111; border:1px solid #5d5d5d`<br>`opacity:1`<br>`opacity:.3`<br>`text-align:left` |
| `scrollable` | `overflow-y:hidden`<br>`【@media screen and (max-width:1279px)】min-height:auto; overflow:auto`<br>`【@media screen and (max-width:1279px)】overflow-y:hidden`<br>`overflow-y:auto` | — |
| `scrollmode-content` | `flex:1 1` | `opacity:.3` |
| `scrollmode-switch` | `align-items:center; display:flex; left:-30%; top:-200%; width:305px`<br>`margin:0`<br>`height:56px; width:22px`<br>`height:43px; width:14px` | `font-size:12px`<br>`background-image:url(../../static/media/scrollmode_switch_bg.cd01ecda.svg)`<br>`background-image:url(../../static/media/scrollmode_switch_top.5d132bac.svg)`<br>`opacity:.3` |
| `scrollmode-switch-bg` | `height:56px; width:22px` | `background-image:url(../../static/media/scrollmode_switch_bg.cd01ecda.svg)` |
| `scrollmode-switch-top` | `height:43px; width:14px` | `background-image:url(../../static/media/scrollmode_switch_top.5d132bac.svg)` |
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

**④ 触觉反馈**（2）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `haptic-content` | `position:relative`<br>`position:fixed`<br>`width:850px`<br>`padding:20px 30px` | `border:1px solid #5d5d5d`<br>`text-align:center`<br>`text-transform:uppercase`<br>`background-color:#111; border:1px solid #5d5d5d`<br>`opacity:1` |
| `haptic-content__title` | `margin:10px 0` | `color:#ccc; font-size:16px; text-align:center; text-transform:uppercase` |

**⑫ 其它**（1）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `SCROLLING` | — | `background-image:url(../../static/media/icon_config_mouse_scrolling.fd95e298.svg)`<br>`background-image:url(../../static/media/icon_config_mouse_scrolling_a.c975098f.svg)` |

#### Blackwidow V4 Pro（productId 653）

该界面共 36 个布局类名，分 3 个分区。

**① 滚轮档位（阶段）**（33）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `custom-scrollable` | `【@media screen and (max-width:1279px)】overflow-y:hidden` | — |
| `customize-scroll` | `height:inherit; min-width:inherit; overflow-y:visible; overflow-y:initial; position:relative; width:inherit`<br>`display:flex; justify-content:center; left:50%; margin:10px; min-width:150px; position:fixed; width:150px`<br>`left:auto!important; position:relative!important; width:auto!important`<br>`height:275px; position:relative` | `transition:all .3s`<br>`fill:#0000; cursor:pointer`<br>`fill:#44d62c`<br>`fill:#39a029`<br>`fill:none` |
| `description-stages` | — | `color:#999; line-height:17px` |
| `haptic-content__scroll` | `display:flex; justify-content:space-between`<br>`padding:20px 0; width:255px` | `background-color:#292929; border:1px solid #0000; border-radius:4px; color:#ccc; font-size:14px; text-align:center`<br>`font-size:28px` |
| `no-scroll` | `overflow-y:hidden`<br>`overflow:hidden!important` | — |
| `noScrollX` | — | — |
| `one-stage` | `display:none` | — |
| `scroll-item` | `position:relative`<br>`left:6px; right:0`<br>`height:36px; margin:0; width:230px` | `background-color:#111; border:1px solid #5d5d5d`<br>`opacity:1`<br>`opacity:.3`<br>`text-align:left` |
| `scroll-item__title` | `display:flex`<br>`position:relative; right:0; top:0`<br>`width:100%` | `text-transform:uppercase` |
| `scroll-slide` | `display:flex; flex-direction:column; justify-content:space-between`<br>`position:relative`<br>`display:flex`<br>`position:relative; right:0; top:0` | `text-transform:uppercase`<br>`background-color:#111; border:1px solid #5d5d5d`<br>`opacity:1`<br>`opacity:.3`<br>`text-align:left` |
| `scrollable` | `overflow-y:hidden`<br>`【@media screen and (max-width:1279px)】min-height:auto; overflow:auto`<br>`【@media screen and (max-width:1279px)】overflow-y:hidden`<br>`overflow-y:auto` | — |
| `scrollmode-content` | `flex:1 1` | `opacity:.3` |
| `scrollmode-switch` | `align-items:center; display:flex; left:-30%; top:-200%; width:305px`<br>`margin:0`<br>`height:56px; width:22px`<br>`height:43px; width:14px` | `font-size:12px`<br>`background-image:url(../../static/media/scrollmode_switch_bg.cd01ecda.svg)`<br>`background-image:url(../../static/media/scrollmode_switch_top.5d132bac.svg)`<br>`opacity:.3` |
| `scrollmode-switch-bg` | `height:56px; width:22px` | `background-image:url(../../static/media/scrollmode_switch_bg.cd01ecda.svg)` |
| `scrollmode-switch-top` | `height:43px; width:14px` | `background-image:url(../../static/media/scrollmode_switch_top.5d132bac.svg)` |
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

**④ 触觉反馈**（2）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `haptic-content` | `position:relative`<br>`position:fixed`<br>`width:850px`<br>`padding:20px 30px` | `border:1px solid #5d5d5d`<br>`text-align:center`<br>`text-transform:uppercase`<br>`background-color:#111; border:1px solid #5d5d5d`<br>`opacity:1` |
| `haptic-content__title` | `margin:10px 0` | `color:#ccc; font-size:16px; text-align:center; text-transform:uppercase` |

**⑫ 其它**（1）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `SCROLLING` | — | `background-image:url(../../static/media/icon_config_mouse_scrolling.fd95e298.svg)`<br>`background-image:url(../../static/media/icon_config_mouse_scrolling_a.c975098f.svg)` |

## 3. 功能项

该界面对应的雷云文案 key，共 **55** 条（中文为雷云原文）：

| key | 中文 | English |
|---|---|---|
| `ACTIVE_SCROLL_WHEEL_HAPTICS` | 当前的滚轮触觉 | Active Scroll Wheel Haptics |
| `ACTIVE_SCROLL_WHEEL_HAPTICS_TOOLTIP` | 查看每个滚轮触觉等级的滚轮触觉。 你可以通过选择自定义等级来自定义自己所需的滚轮触觉等级。 | View the scroll wheel haptics of each scroll wheel stage. <br><br>You can customize your own scroll wheel haptics by selecting the Custom stage. |
| `CONFIGURE_SCROLL_WHEEL_STAGES_SETTINGS` | 配置滚轮触觉等级 | Configure Scroll Wheel Stages |
| `CYCLE_DOWN_SCROLL_WHEEL_STAGES` | 向下循环滚轮触觉等级 | Cycle Down Scroll Wheel Stages |
| `CYCLE_UP_SCROLL_WHEEL_STAGES` | 向上循环滚轮触觉等级 | Cycle Up Scroll Wheel Stages |
| `FREE_SPIN` | 自由滚动 | Free-Spin |
| `FREE_SPIN_SCROLLING_MODE` | 向前推：自由滚动模式 | Push forward: Free-Spin Scrolling mode |
| `HIGH_RESOLUTION_SCROLLING` | 高分辨率滚动 | High resolution scrolling |
| `HIGH_RESOLUTION_SCROLLING_TOOLTIP` | 当浏览器检测处于启用状态时，享受更流畅的滚动体验。 | Enjoy an even smoother scrolling experience when Browser Detection is active. |
| `HORIZONTAL` | 水平 | Horizontal |
| `HORIZONTAL_SCROLLING` | 水平滚动 | Horizontal Scrolling |
| `SCROLLING` | 滚动 | SCROLLING |
| `SCROLLING_CUSTOM_TAB_TOOLTIP` | 启用“关联”后，向上和向下方向的点和曲线将相同。 | When Link is enabled, the points and curves in the Up and Down Directions will be identical. |
| `SCROLLING_MODE` | 滚动模式 |  |
| `SCROLL_ACCELERATION` | 滚动加速 | SCROLL ACCELERATION |
| `SCROLL_ACCELERATION_DESC` | 提升你滑动滚轮时的滚动速度。 | Increase the scroll speed the faster you scroll. |
| `SCROLL_ACCELERATION_LEVEL_DESC` | 提升你滑动滚轮时的滚动速度。 | Amplifies your scrolling speed when in Free Spinning mode. |
| `SCROLL_ACCELERATION_TOOLTIP` | 在滚轮触觉等级中启用“平滑滚动”. | Enable 'Smooth Scroll' in the Scroll Wheel Stages first. |
| `SCROLL_ACCELERATION_WITH_LEVEL` | 自由滚动模式下的滚动加速 | SCROLL ACCELERATION FOR FREE-SPIN |
| `SCROLL_CLICK` | 滚轮单击 | Scroll Click |
| `SCROLL_DISABLE_MODES_DESC` | 根据你的喜好，最多可禁用 2 个滚动模式等级。 | Disable up to 2 scrolling modes to suit your preference. |
| `SCROLL_DOWN` | 向下滚动 | Scroll Down |
| `SCROLL_HORIZONTAL` | 水平滚动 | Scroll Horizontal |
| `SCROLL_LEFT` | 向左滚动 | Scroll Left |
| `SCROLL_LEFT_RIGHT` | 向左/向右滚动 | Scroll Left/Right |
| `SCROLL_MODE` | 滚动模式 | SCROLLING MODE |
| `SCROLL_MODE_DESC` | 若追求精度，请用触觉滚动模式；若追求速度，请用自由滚动模式。 | Go for precision with Tactile Cycling or speed with Free-Spining. |
| `SCROLL_MODE_SWITCH` | 滚动模式切换开关 | Scroll Mode Switch |
| `SCROLL_MODE_SWITCH_DESCRIPTION` | 在自由滚动和触觉滚动模式之间切换。 | Switch between Free-Spin Scrolling and Tactile Cycling mode |
| `SCROLL_MODE_TOGGLE` | 滚动模式切换 | Scroll Mode Toggle |
| `SCROLL_OPTION` | 向上/向下滚动 | Scroll Up/Down |
| `SCROLL_RIGHT` | 向右滚动 | Scroll Right |
| `SCROLL_STEPS` | 滚动步数 | Scroll Steps |
| `SCROLL_STEPS_TOOLTIP` | 调整滚轮每转的级数，以获得你喜欢的触感。 | Adjust the number of steps per scroll wheel revolution to achieve your preferred tactile feel. |
| `SCROLL_TENSION` | 滚动阻力 | Scroll Tension |
| `SCROLL_TENSION_TOOLTIP` | 降低滚动阻力以使滚轮平滑滚动，或增加阻力以提升触感。 | Reduce the scroll tension for a smoother scroll wheel movement or increase it for a more tactile feel. |
| `SCROLL_THREE_MODE_DESC` | 若追求精度，请用 24 档触觉滚动模式；若追求速度，请用自由滚动模式。 | Go for precision with 24-step indent Tactile Cycling or speed with Free-Spinning. |
| `SCROLL_UP` | 向上滚动 | Scroll Up |
| `SCROLL_UP_AND_DOWN` | 向上滚动和向下滚动 | Scroll Up and Scroll Down |
| `SCROLL_UP_DOWN` | 向上/向下滚动 | Scroll Up/Down |
| `SCROLL_UP_PAGE_UP` | Scroll Up (Page Up) | Scroll Up (Page Up) |
| `SCROLL_VERTICAL` | 垂直滚动 | Scroll Vertical |
| `SCROLL_WHEEL` | 滚轮 | SCROLL WHEEL |
| `SCROLL_WHEEL_ACCELERATION` | 滚动加速 | Scroll Acceleration |
| `SCROLL_WHEEL_ACCELERATION_DESC` | 当平滑滚动等级已启用时，你滑动滚轮的速度越快，滚轮滚动的速度越快。 | Increase the scroll speed the faster you scroll when Smooth Scroll stage is active. |
| `SCROLL_WHEEL_OPTION_HEADER` | 滚动选项 | Scroll Options |
| `SCROLL_WHEEL_OPTION_TOOLTIP` | 自定义滚轮，确保当平滑滚动等级已启用滚动加速时，滚轮的滚动速度会匹配你滑动滚轮的速度。 启用浏览器检测，以便针对某些应用程序，自动将滚轮切换到平滑滚动模式。 | Customize the scroll wheel to adapt its scroll speed to how fast you scroll when the Smooth Scroll stage is enabled with Scroll Acceleration. <br> <br>Enable Browser Detection to automatically switch to Smooth Scroll mode when scrolling on certain applications. |
| `SCROLL_WHEEL_STAGES` | 滚轮触觉等级 | Scroll Wheel Stages |
| `SCROLL_WHEEL_STAGES_DESCRIPTION` | 通过鼠标上的滚动模式按键，选择所需的滚轮触觉等级或在滚轮触觉等级间循环切换 | Select your preferred scroll wheel stage or cycle through scroll wheel stages using the scroll mode button on your mouse. |
| `SCROLL_WHEEL_STAGES_DESCRIPTION_LINK` | 通过鼠标上的滚动模式按键在滚轮触觉等级间循环切换 | cycle through scroll wheel stages using the scroll mode button on your mouse |
| `SCROLL_WHEEL_STAGES_DESCRIPTION_WITH_LINK` | 选择所需的滚轮触觉等级，或{{link}}。 | Select your preferred scroll wheel stage or {{link}}. |
| `SCROLL_WHEEL_STAGES_TOOLTIP` | 启用或禁用滚轮触觉等级，或通过拖放等级图块的方式对其重新排序。若想加入自定义的滚轮触觉等级，请先启用自定义等级。 | Enable or disabled scroll wheel stages or reorder them by dragging and dropping stage tiles. <br><br>To include a customized scroll wheel stage, enable the Custom stage first. |
| `SCROLL_WHEEL_TOOLTIP` | 自定义滚轮，使其滚动速度匹配你滑动滚轮的速度。<br> <br>你还可以将其滚动模式更改为触觉滚动模式以便一次滚动一行，或自由滚动模式以便轻松快速浏览内容，也可以启用智能滚动模式以便根据滚轮的移动自动切换滚动模式。 | Customize the scroll wheel to adapt its scroll speed to how fast you scroll.<br> <br>You can also change its scroll mode to Tactile Cycling to scroll one line at a time, Free-Spin Scrolling to easily speed through content, or enable Smart-Reel to automatically switch scroll modes according to scroll wheel movement. |
| `SCROLL_WHEEL_TOOLTIP_V2` | 自定义滚轮，使其滚动速度匹配你滑动滚轮的速度。<br> <br>你可以根据需要更改滚动模式。滚动模式包括“触觉滚动”，可实现逐行滚动；“自由滚动”，可助你快速浏览内容；“精确触觉滚动”，可提供超精准控制；以及“智能滚动”，可根据滚轮的移动自动调整滚动模式。 | Customize the scroll wheel to adapt its scroll speed to how fast you scroll.<br> <br>You can change the scroll modes to suit your needs. Options include Tactile Cycling, which allows you to scroll one line at a time; Free-Spin Scrolling, which helps you quickly navigate through content; Precision Tactile, giving you ultra-precise control; or Smart-Reel, which automatically adjusts the scroll mode based on the movement of the scroll wheel. |
| `WHEEL` | 旋转效果 | Wheel |
## 4. 本项目的实现状态

| 项 | 内容 |
|---|---|
| 本项目的页面 | src/pages/calibration.rs —— 已实现（表面配置文件 / 校准状态机） |
| 后端方案 | 方案 2：复用雷云原生引擎（`lighting_driver` / `RzLightingEngineApi` / `mapping_engine` / `simple_service`） |
| 证据等级 | 布局 `[前端]`（设备模块 CSS）、文案 `[文案]`（语言包）、设备归属 `[前端]`（设备模块常量块） |

> 尚未实现的界面在应用里走占位页；占位页列出该页**真实存在的 key**，不编造内容。


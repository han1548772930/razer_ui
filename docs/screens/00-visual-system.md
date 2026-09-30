# 视觉规范（从设备模块 CSS 全量提取）

来源：`.ref/devices/777` 的全部 CSS。**未过滤属性**——每条规则原样给出，
包括布局、颜色、圆角、字号、边框、阴影、过渡、各状态（hover / active / disabled）。

重新生成：`node .ref/tools/visual-system.js .ref/devices/777`

| 组件 | 规则数 |
|---|---:|
| 页面骨架 | 125 |
| 顶栏 / 头部 | 93 |
| 侧边栏 / 设备 | 64 |
| 标签栏 | 50 |
| 按钮 | 307 |
| 开关 | 53 |
| 滑杆 / 步进 | 145 |
| 下拉 / 输入 | 278 |
| 提示 / 浮层 | 385 |
| 弹窗 / 遮罩 | 211 |
| 图表 | 40 |
| 滚动条 | 24 |
| 文字 | 238 |
| 图标 | 121 |
| 未归类 | 1507 |

## 与 gpui-kit 主题令牌的对应

gpui-kit《Design Guides · Color and themes》要求：

> Application UI should not contain raw hex, `rgb`/`rgba`, or `hsla` colors.
> Resolve colors from `cx.theme()` by semantic role. If the required role
> does not exist, define it in the product's theme/token layer rather than
> embedding a palette value at the call site.

因此下面每条雷云字面值都落到一个**命名令牌**上，原始值只出现在
`src/main.rs` 的 `Theme::update` 里。

### 颜色

| 雷云 CSS | 值 | gpui-kit 令牌 |
|---|---|---|
| `.main-container{background-color}` | `#222` | `colors.background` |
| `.body-widgets .widget{background-color}` | `#111` | `colors.group_box` |
| `.switch-button{background-color}` | `#333` | `colors.secondary` / `colors.input` |
| `.thx-btn{background-color}` | `#44d62c` | `colors.primary` / `colors.button_primary` |
| `.thx-btn{color}` | `#000` | `colors.primary_foreground` |
| `.thx-btn:hover{opacity:.8}` | ≈`#3db22a` | `colors.button_primary_hover` |
| `.thx-btn:active{opacity:.6}` | ≈`#368e28` | `colors.button_primary_active` |
| `.thx-btn.secondary{background-color}` | `#707070` | `colors.button` |
| `.switch-button{color}` | `#ccc` | `colors.foreground` |
| `.volume-item{color}` | `#999` | `colors.muted_foreground` |
| `.switch-button{border}` | `#5d5d5d` | `colors.border`（**157 次；此前误写 #555**） |
| `.nav:hover{background-color}` | `#2d2d2d` | `colors.secondary_hover` |
| `.s3-dropdown{border}` | `#515151` | `colors.border`（下拉框档位） |
| `.reshare{border}` / `.warning-alert{border}` | `#fd8611` | `colors.warning` |
| `.profile-del div.thx-btn{background-color}` | `#c8323c` | `colors.danger` |
| `body,html{font-family}` | `Roboto,sans-serif` | `theme.font_family` |
| `body,html{font-size}` | `16px`（`.widget` 内 14px） | `theme.font_size = px(14.)` |

### 圆角

雷云只有**两级**圆角，正好对上主题的命名档位（`radius_tokens()` 中
`md = radius`、`lg = radius_lg`）：

| 雷云 CSS | 值 | 主题设置 | 令牌 |
|---|---|---|---|
| `.thx-btn{border-radius}` | `3px` | `theme.radius = px(3.)` | `radius_tokens().md`（控件） |
| `.widget{border-radius}` | `5px` | `theme.radius_lg = px(5.)` | `radius_tokens().lg`（卡片） |

### 几何

这些是**复刻对象自身的固定尺寸**，不是通用间距，因此作为具名常量保留在
`src/pages/widgets.rs::geometry`，并逐条注明来自哪条 CSS：

| 雷云 CSS | 值 | 常量 |
|---|---|---|
| `.main-container` / `.body-wrapper{min-width}` | `600px` | `geometry::SHELL_MIN_WIDTH` |
| `.body-widgets{max-width}` | `1240px` | `geometry::BODY_MAX_WIDTH` |
| `.widget{min-width/max-width}` | `600px` | `geometry::WIDGET_WIDTH` |
| `.widget{padding}` | `30px 40px` | `geometry::WIDGET_PADDING_Y` / `_X` |
| `.widget{margin}` | `10px auto` | `geometry::WIDGET_MARGIN_Y` |
| `.widget-prod{height}` | `250px` | `geometry::PRODUCT_BANNER_HEIGHT` |
| `.widget-prod{min-width}` | `1024px` | `geometry::PRODUCT_BANNER_MIN_WIDTH` |
| `.widget-prod{max-width}` | `1220px` | `geometry::PRODUCT_BANNER_MAX_WIDTH` |
| `.body-wrapper{padding}` | `10px 20px 20px` | `geometry::BODY_PADDING_*` |

### 依据规范修正过的问题

| 问题 | 规范依据 | 修正 |
|---|---|---|
| 调用点直接写 `rgb(0x44d62c)` 等原始颜色 | Color and themes | 改走 `cx.theme()`；原始值只留在 `main.rs` 主题定义 |
| 按钮圆角写死 `px(3.)` | Radius, spacing, and density | 改用 `cx.theme().radius_tokens().md` |
| 页面用全宽竖排卡片 | Layout patterns | 改为雷云真实的 `.body-widgets` 600px 卡片两列换行 |
| 把 `audio-left`/`audio-right` 当成左右两栏 | — | 更正：它们是**产品图片**类名；真正分栏是 `.widget-col` |


---

## 页面骨架（125 条）

| 选择器 | 声明 |
|---|---|
| `.main-container` | `background-color:#222;display:flex;flex-direction:column;height:100%;min-width:600px;position:absolute;width:100%` |
| `.main-container>#body-wrapper>.body-widgets` | `min-width:0;min-width:auto` |
| `.body-wrapper` | `flex:1 1;height:100%;min-width:600px;padding:10px 20px 20px;width:100%` |
| `.body-wrapper .background-edition-gradient` | `background-image:radial-gradient(ellipse,#0000,#222 70%);height:600px;position:absolute;right:0;top:0;width:50%;z-index:2` |
| `.body-wrapper .background-edition` | `background-size:cover;height:600px;opacity:.5;position:absolute;right:0;top:0;width:50%;z-index:1` |
| `.body-wrapper.scrollable.no-scroll` | `overflow-y:hidden` |
| `#body-wrapper.body-wrapper.scrollable  [@media screen and (max-width:1279px)]` | `min-height:auto;overflow:auto` |
| `#body-wrapper.body-wrapper.scrollable.noScrollX  [@media screen and (max-width:1279px)]` | `overflow-x:hidden` |
| `#body-wrapper.body-wrapper.scrollable.custom-scrollable  [@media screen and (max-width:1279px)]` | `overflow-y:hidden` |
| `.main-container.backdrop-on .nav-tabs` | `opacity:.3;pointer-events:none` |
| `.body-wrapper-relative` | `bottom:auto;position:relative;top:auto` |
| `.body-wrapper-relative .body-wrapper-content` | `display:flex;flex-direction:row;opacity:.6;z-index:3` |
| `.body-wrapper-relative .body-wrapper-content .dot-row` | `display:flex` |
| `.body-wrapper-relative .body-wrapper-content .dot-col` | `display:flex;flex-direction:column` |
| `.body-wrapper-relative .body-wrapper-content .dot-wrap` | `align-items:center;background-color:#0000;display:flex;height:22px;justify-content:center;width:22px` |
| `.body-wrapper-relative .body-wrapper-content .dot-wrap .dot` | `background-color:#5d5d5d;height:2px;width:2px` |
| `#body-wrapper .profile-bar.flex,.profile-bar-macro` | `padding-left:3%` |
| `.body-widgets` | `flex-direction:row;flex-wrap:wrap;justify-content:center;margin:auto;max-width:1240px` |
| `.body-widgets .widget-col.col-left .tip` | `left:auto;right:14px;word-break:break-word` |
| `.body-widgets .widget-col.col-left .tip  [@media(max-width:1279px)]` | `left:auto;right:14px` |
| `.body-widgets .widget-col.col-left .widget.mic-enhancements .tip,.body-widgets .widget-col.col-left .widget.microphone .tip,.body-widgets .w` | `white-space:pre-line` |
| `.widget-col` | `flex-direction:column;height:-webkit-fit-content;height:fit-content;width:600px` |
| `.widget.flex-hidden` | `opacity:0;visibility:hidden` |
| `.body-widgets .widget` | `background-color:#111;border-radius:5px;flex:0 0 auto;font-size:14px;height:auto;margin:10px auto;max-width:600px;min-width:600px;padding:30px 40px;position:relative;transition:height .2s;will-change:height` |
| `.body-widgets .widget.hyperPolling` | `padding:30px 30px 30px 40px` |
| `.body-widgets .widget.global-shortcut` | `padding:20px 10px 30px` |
| `.body-widgets .widget.global-shortcut .body-text,.body-widgets .widget.global-shortcut .shortcut` | `margin-bottom:20px;padding:0 10px` |
| `.body-widgets .widget>.info` | `opacity:.3;pointer-events:none` |
| `.body-widgets .widget>.info.active` | `opacity:1;pointer-events:auto` |
| `.body-widgets .widget>.mic-container` | `position:relative` |
| `.body-widgets .widget>.mic-container .tip-disabled` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-family:Roboto;left:25%;line-height:16px;max-width:300px;opacity:0;padding:8px 10px;position:absolute;top:40%;transition:visibility 0s,opacity .3s linear;visibility:hidden;will-change:visibility,opacity` |
| `.body-widgets .widget>.mic-container:hover>.tip-disabled` | `opacity:1;visibility:visible;z-index:1000` |
| `.body-widgets .widget .shortcut>.shortcut-tip` | `transition:opacity .2s;will-change:opacity` |
| `.body-widgets .widget .shortcut:hover>.shortcut-tip` | `opacity:1;visibility:visible;z-index:100` |
| `.body-widgets .widget .ble-edit-tip,.body-widgets .widget .shortcut-tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;line-height:16px;max-width:300px;opacity:0;padding:8px 10px;position:absolute;right:39px;text-align:left;text-transform:none;top:67px;transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:pre-wrap;width:-webkit-max-content;width:max-content;will-change:visibility,opacity;z-index:99` |
| `.body-widgets .widget .hyperpolling-span-hover:hover` | `color:#44d62c` |
| `.body-widgets .widget>div:not(.widget-covering):not(.panel-light--chroma)` | `transition:opacity .2s` |
| `.body-widgets .widget.actuation-widget>div` | `will-change:auto!important` |
| `.body-widgets .widget.tip-break-spaces .tip` | `white-space:break-spaces` |
| `.body-widgets .widget.disabled>div` | `opacity:.3;pointer-events:none` |
| `.widget-tooltip.disabled` | `opacity:.5` |
| `.widget-tooltip.disabled .widget` | `pointer-events:auto` |
| `.widget-tooltip.disabled .widget div` | `pointer-events:none` |
| `.idleEffect .widget.disabled` | `opacity:1;pointer-events:auto` |
| `.idleEffect .widget.disabled>div` | `opacity:.3;pointer-events:none` |
| `.chargingEffect .widget.disabled,.idleEffect .widget.disabled .titleRow` | `opacity:1;pointer-events:auto` |
| `.chargingEffect .widget.disabled>div` | `opacity:.3;pointer-events:none` |
| `.chargingEffect .widget.disabled .titleRow` | `opacity:1;pointer-events:auto` |
| `.widget-switch` | `forced-color-adjust:none;position:relative` |
| `.widget-switch:hover .widget-switch-tooltip` | `opacity:1;visibility:visible` |
| `.widget-switch-tooltip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-family:Roboto,sans-serif;font-size:14px;left:10px;line-height:16px;margin:0;opacity:0;padding:8px 10px;position:absolute;text-align:left;text-transform:none;top:calc(100% + 2px);transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:nowrap;will-change:visibility,opacity;z-index:99` |
| `.widget-container` | `display:flex;position:relative` |
| `.widget-container:hover .widget-body-tooltip` | `opacity:1;visibility:visible` |
| `.widget-container .widget.volume .tip` | `left:100%;right:auto;transform:translateX(-8%)` |
| `.widget-body-tooltip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;left:50%;line-height:16px;margin:0;opacity:0;padding:8px 10px;position:absolute;text-align:left;text-transform:none;top:50%;transform:translate(-50%,-50%);transition:visibility 0s,opacity .3s linear;visibility:hidden;width:280px;will-change:visibility,opacity;z-index:99` |
| `.widget .titleRow` | `display:flex;justify-content:space-between` |
| `.widget .titleRow .title` | `color:#44d62c;display:flex;font-family:RazerF5,sans-serif;font-size:16px;margin-bottom:20px;text-transform:uppercase` |
| `.widget .titleRow .shortcuts` | `align-items:center;display:flex;height:27px` |
| `.widget .titleRow .shortcuts .shortcutButton` | `align-items:center;border:1px solid #5d5d5d;border-radius:3px;display:flex;height:100%;justify-content:center;margin:0 10px;width:47px` |
| `.widget .titleRow .shortcuts .shortcutButton-filled` | `background-color:#292929` |
| `.widget .h2-title` | `margin-bottom:20px;text-transform:uppercase` |
| `.widget .h2-body` | `color:#999;margin-bottom:20px` |
| `.widget .button` | `align-items:center;background:#707070;border:1px solid #0000004d;border-radius:3px;box-sizing:border-box;display:flex;flex:none;flex-direction:row;flex-grow:0;font-family:Roboto;font-size:12px;font-style:normal;font-weight:400;gap:10px;height:27px;justify-content:center;line-height:14px;margin-bottom:15px;order:2;padding:7px 16px 6px;text-align:center;text-transform:uppercase;width:173px` |
| `.widget .button:hover` | `background-color:#ffffff1a` |
| `.widget .tips` | `align-self:stretch;color:#999;flex:none;flex-grow:0;font-family:Roboto;font-size:14px;font-style:normal;font-weight:400;line-height:17px;order:3` |
| `.widget .content` | `margin-bottom:15px` |
| `.widget .exclamation` | `padding-left:20px` |
| `.widget .exclamation:before` | `background-color:#5d5d5d;background-image:url(../../static/media/tooltip_exclamationmark.cc8fb226.svg);content:"";display:inline-block;left:40px;margin-right:6px;top:30px` |
| `.widget .exclamation:before,.widget .help` | `background-repeat:no-repeat;border-radius:7px;height:14px;position:absolute;width:14px` |
| `.widget .help` | `background-color:#4a4a4a;background-image:url(../../static/media/tooltip_questionmark.96138d2f.svg);right:10px;top:10px;transition:background-color .3s;will-change:background-color` |
| `.widget .help:hover` | `background-color:#ffffff4d` |
| `.body-widget-tip-portal,.widget .tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;line-height:18px;max-width:300px;opacity:0;padding:8px 10px;position:absolute;right:14px;text-align:left;text-transform:none;top:34px;transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:pre-wrap;width:-webkit-max-content;width:max-content;will-change:visibility,opacity;z-index:99` |
| `.widget .tip.custom-tip` | `left:auto;right:14px` |
| `.widget .help:hover+.tip` | `opacity:1;visibility:visible;z-index:100` |
| `.widget .help:hover+.tip.mt-tip` | `background-color:#0000;border:none;padding:0` |
| `.widget-prod` | `height:250px;margin:10px auto;max-width:1220px;min-width:1024px;width:100%` |
| `.widget-prod.earbuds-svg` | `align-items:center;display:flex;justify-content:center` |
| `.widget-prod.custom-lighting` | `min-width:0;min-width:auto;width:auto` |
| `.widget-prod img` | `left:50%;position:absolute;top:50%;transform:translate(-50%,-50%);z-index:1` |
| `.widget-prod img.audio-left,.widget-prod img.audio-right` | `left:auto;position:static;top:auto;transform:none` |
| `.widget-prod img.audio-left.disabled,.widget-prod img.audio-right.disabled` | `opacity:50%;pointer-events:auto` |
| `.widget-prod .earbuds-img` | `display:flex;gap:8px` |
| `.widget .title .switch` | `left:10px;position:relative;top:3px` |
| `.widget-col  [@media(max-width:1279px)]` | `margin:0 30px` |
| `.customize .body-widgets.flex` | `flex-direction:row` |
| `.customize .body-wrapper` | `float:right;height:calc(100% - 96px);left:0;min-height:350px;position:absolute;top:100px;top:auto;transition:.2s;width:100%;z-index:0` |
| `.customize .body-wrapper.drawer-open` | `left:230px;width:calc(100% - 230px)` |
| `.customize .body-wrapper.config-headset` | `height:calc(100% - 96px);position:static` |
| `.customize .body-wrapper.config-headset.drawer-open` | `left:230px;width:calc(100% - 230px)` |
| `.customize .body-wrapper.drawer-open  [@media screen and (max-width:1250px)]` | `min-width:0;overflow-x:scroll` |
| `.customize .body-wrapper.drawer-open .body-widgets` | `width:1000px` |
| `#key-mapping-relative-wrapper .customize .body-wrapper.drawer-open` | `position:relative;top:0` |
| `.body-wrapper .key-config.open.right  [@media screen and (max-width:1220px)]` | `left:30vw` |
| `.body-wrapper .key-config.open.left  [@media screen and (max-width:1220px)]` | `left:39px` |
| `.widget-col .float-tip` | `background-color:#111;border:1px solid #707070;opacity:1;padding:8px;position:fixed;visibility:visible;z-index:13` |
| `.widget-col .float-tip.disabled` | `opacity:0;visibility:hidden` |
| `.widget-col .float-tip--content` | `font-size:14px` |
| `.body-wrapper.no-name-bar` | `bottom:0` |
| `.widget .effect-wrapper` | `z-index:10` |
| `.main-setting div .body-widgets` | `display:block;margin:0` |
| `.main-setting .widget.language-setting` | `z-index:110` |
| `.main-setting .widget .spinner-razer` | `height:20px;width:20px` |
| `.main-setting .widget .title` | `font-size:18px` |
| `.main-setting .widget .check-item` | `margin:0` |
| `.main-setting div.widget-col  [@media(max-width:1279px)]` | `margin:0` |
| `.main-setting .device-setting .widget` | `padding:20px` |
| `.main-setting .device-setting .widget .title` | `padding:0 10px` |
| `#multipleBrightness .body-wrapper-custom` | `overflow-y:scroll!important;padding:10px 0 0!important` |
| `.widget .help.indicator-led:hover .tip` | `opacity:1;visibility:visible;z-index:100` |
| `.widget .help.indicator-led:before` | `background-color:#0000;bottom:-5px;content:"";height:10px;left:5px;position:absolute;width:50px` |
| `.widget .help.indicator-led .tip` | `left:36%;top:19px` |
| `.widget .panel-light--block,.widget .panel-light--description` | `margin-bottom:20px` |
| `.widget .panel-light--block:last-child` | `margin-bottom:10px` |
| `.widget .panel-light--block.mb-none` | `margin-bottom:0` |
| `.widget .panel-light--chroma` | `margin-top:20px` |
| `.widget .panel-light--chroma .panel-light--chroma__block:first-child .panel-light-text` | `margin-bottom:20px;min-width:520px` |
| `.widget .panel-light--chroma .panel-light--chroma__block:last-child` | `position:relative` |
| `.widget .panel-light--chroma .panel-light--chroma__block:last-child .panel-light--chroma__warning` | `padding-left:30px` |
| `.widget .panel-light--chroma .panel-light--chroma__block:last-child .panel-light--chroma__warning:before` | `background-image:url(../../static/media/warning-icon.b0086fcc.svg);background-position:50%;background-size:20px 17px;content:"";height:17px;left:0;position:absolute;top:0;width:20px` |
| `.widget .panel-light--block.effect` | `padding-top:20px` |

> 另有 5 条未列出。

---

## 顶栏 / 头部（93 条）

| 选择器 | 声明 |
|---|---|
| `.toolbar .header-avatar.active` | `z-index:2` |
| `.toolbar .header-avatar .box .img` | `background-size:22px;height:20px;width:20px` |
| `.toolbar .header-avatar .box .img-overlay` | `height:22px;width:22px` |
| `.logo-effects-radio-container` | `display:flex;flex-direction:column;gap:10px;margin-left:-20px` |
| `.header-offline>.box` | `align-items:center;cursor:default;display:flex;height:48px;justify-content:center;transition:background-color .1s ease-in-out;width:60px` |
| `.header-offline .show` | `z-index:3` |
| `.header-offline>.box:active,.header-offline>.box:hover` | `background-color:#3cbf27` |
| `.header-offline>.box>.circle` | `align-items:center;background-color:#212121;border-radius:50%;display:flex;flex-direction:column;height:32px;justify-content:center;width:32px` |
| `.header-offline>.box>.circle>.icon` | `background-image:url(../../static/media/cloud-yellow.3957bb55.svg);background-position:50%;background-repeat:no-repeat;background-size:100%;height:20px;width:20px` |
| `.header-unsaved>.box` | `align-items:center;cursor:default;display:flex;height:48px;justify-content:center;position:relative;transition:background-color .1s ease-in-out;width:60px` |
| `.header-unsaved.active` | `z-index:105` |
| `.header-unsaved.active>.box,.header-unsaved>.box:active,.header-unsaved>.box:hover` | `background-color:#3cbf27` |
| `.header-unsaved.active>.box>.img-overlay,.header-unsaved>.box:active>.img-overlay` | `opacity:.1` |
| `.header-unsaved>.box>.img,.header-unsaved>.box>.img-overlay` | `border-radius:50%;bottom:0;content:"";height:32px;left:0;margin:auto;position:absolute;right:0;top:0;width:32px` |
| `.header-unsaved>.box>.img` | `background-color:#212121;background-image:url(../../static/media/save-white.783e6aa4.svg);background-position:50%;background-repeat:no-repeat;background-size:20px` |
| `.header-unsaved>.box>.img-overlay` | `background-color:#000;opacity:0;transition:opacity .1s linear;z-index:1` |
| `.header-unsaved>.box>.badge` | `background-color:#c8323c;border-radius:9px;font-size:10px;font-weight:700;height:18px;line-height:19px;margin:auto 3px 3px auto;min-width:18px;padding:0 6px;text-align:center;-webkit-user-select:none;user-select:none;z-index:0` |
| `.header-unsaved .dropdown-item.desc` | `color:#999;margin:0 auto;max-width:220px;padding:1px 5px;pointer-events:none;text-align:center;text-transform:none;white-space:normal` |
| `.header-unsaved .dropdown-razer-2` | `padding:7px 14px 12px;z-index:105` |
| `.header-unsaved .dropdown-razer-2 .dropdown-divider` | `background-color:#5d5d5d;height:1px;margin:10px 0;width:100%` |
| `.header-unsaved .dropdown-razer-2 .dropdown-btns` | `display:flex;flex-direction:row-reverse;padding:0 4px` |
| `.header-unsaved .dropdown-razer-2 .dropdown-btns>.btn` | `border-radius:2px;flex-shrink:0;font-size:12px;line-height:1;min-width:105px;padding:7px 13px 6px;white-space:nowrap;width:-webkit-fit-content;width:fit-content` |
| `.header-unsaved .dropdown-razer-2 .dropdown-btns>.btn:first-child` | `margin-left:8px;max-width:none` |
| `.header-update.active>.box .tooltip,.header-update>.box:active .tooltip,.header-update>.box:hover .tooltip` | `display:flex;flex-direction:column;opacity:1` |
| `.header-update>.box` | `align-items:center;background-color:#212121;display:flex;flex-direction:column;height:48px;justify-content:center;position:relative;width:60px` |
| `.header-update>.box>.icon` | `background-image:url(../../static/media/icon_download.1e6d735a.svg);background-repeat:no-repeat;background-size:100%;height:20px;width:20px` |
| `.header-update>.box>.tooltip` | `background-color:#111;border:1px solid #383838;display:none;font-size:14px;opacity:0;padding:8px 10px!important;position:absolute;text-align:center;top:38px;width:-webkit-max-content;width:max-content` |
| `.header-update>.box>.tooltip p` | `margin:0` |
| `.header-update>.box>.circle>.icon` | `background-image:url(../../static/media/icon_download.1e6d735a.svg);background-repeat:no-repeat;background-size:100%;height:14px;margin-bottom:2px;width:14px` |
| `.header-update>.box>.circle>.progress-bar` | `background-color:#2c5824;border-radius:1px;height:2px;overflow:hidden;position:relative;width:14px` |
| `.header-update>.box>.circle>.progress-bar>.progress` | `background-color:#44d62c;height:100%;left:0;position:absolute;top:0;transition:width .3s linear;width:0` |
| `.header-update .btn` | `background-color:#707070;color:#fff;width:auto` |
| `.header-update .box:active,.header-update .box:hover,.header-update.active>.box,.header-update:hover` | `background-color:#2d2d2d;cursor:pointer` |
| `.twoKeyMapping .logo` | `background-image:url(../../static/media/icon_secondaryfunction-1.734f766c.svg);background-size:20px 20px;height:20px;margin-left:-30px;position:absolute;top:15px;width:20px` |
| `.require-synapse .logo` | `background-color:initial;background-image:url(../../static/media/logo_synapse.bc241e5e.svg)!important;background-repeat:no-repeat!important;background-size:24px 24px!important;height:24px;margin-left:10px;width:24px` |
| `.require-synapse .logo-chroma` | `background-image:url(data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADwAAAA8CAYAAAA6/NlyAAAABHNCSVQICAgIfAhkiAAAExVJREFUaEPNmwmYVcWVgM/be6EBlX0HYUYJjiAY9s3MyKi0QaODg6gYGWEMjIRF1CQoIRmVRXDBJWJEICSfy2hCRB2c0EQ2GwmLCI400CBLd4NA7/32/KfuvY/Xz4aGtgGL73C39+6rv86pU6dOVbvkPJZ4PD6Y1w85fvx4N7fb3dHv97fJyMhonvyTFRUVhaFQ6CD39jZu3Phzjjkul2vN+aqWqz5fDGBj3ndPYWHhfc2bN7+qrKwsunnzZs/WrVslPz9f9FhTGTJkiAAr3bt3l549e0YbN` |
| `.header-alexa>.box` | `align-items:center;cursor:default;display:flex;height:38px;justify-content:center;position:relative;transition:background-color .1s ease-in-out;width:46px` |
| `.header-alexa>.box:active,.header-alexa>.box:hover` | `background-color:#2d2d2d` |
| `.header-alexa>.box>.circle,.header-alexa>.box>.circle:after` | `border-radius:50%;height:32px;width:32px` |
| `.header-alexa>.box>.circle` | `align-items:center;display:flex;flex-direction:column;justify-content:center;position:relative` |
| `.header-alexa>.box>.circle:after` | `background-color:#000;content:"";left:0;opacity:0;position:absolute;top:0;transition:opacity .1s linear;z-index:1` |
| `.header-alexa>.box:active>.circle:after` | `opacity:.1` |
| `.header-alexa>.box>.circle>.icon` | `background-image:url(../../static/media/alexa.b9b62050.svg);background-repeat:no-repeat;background-size:100%;height:20px;width:20px` |
| `.header-alexa>.box>.indicator` | `background-image:url(../../static/media/indicator.b7ce7af4.svg);bottom:-18px;height:36px;left:0;margin:auto;position:absolute;right:0;width:36px;z-index:102` |
| `.toolbar .right .header-avatar.active>.box,.toolbar .right .header-avatar.active>.box:active,.toolbar .right .header-avatar.active>.box:hove` | `background-color:#2d2d2d` |
| `.toolbar .right .header-avatar .tooltip,.toolbar .right .header-update .tooltip` | `left:auto;right:0` |
| `.toolbar .right .header-avatar .box[tooltip]:before` | `right:5px` |
| `.dashboard .box-item .logo-placeholder-container` | `height:100px;margin:15px auto 25px;position:relative;width:100px` |
| `.dashboard .box-item .logo-placeholder-container .logo-placeholder` | `background-position:50%;background-repeat:no-repeat;background-size:100px;height:100%;width:100%` |
| `.dashboard .box-item .logo-placeholder-container.loading:after` | `content:url(../../static/media/spinner.ef2d0235.svg);height:27px;left:calc(50% - 14px);position:absolute;top:calc(50% - 14px);width:27px` |
| `.dashboard .box-item .logo-placeholder-container.loading .logo-placeholder` | `opacity:.3` |
| `.dashboard .box-flip .box-flip-inner .box-flip-front .logo-placeholder` | `height:100px;margin-bottom:25px;width:100px` |
| `.dashboard .box-flip .box-flip-inner .box-flip-front .logo-placeholder.legacy-devices` | `background-color:#44d62c;border-radius:50px` |
| `.dashboard .box-flip .box-flip-inner .box-flip-back .header` | `align-self:center;margin-bottom:10px;text-transform:uppercase` |
| `.thx-logo` | `background-image:url(../../static/media/thx_logo.2a283c30.svg);background-position:50%;background-repeat:no-repeat;height:32px;width:80px` |
| `.power-saving-effect .header` | `margin:20px 0 10px` |
| `.logo-brightness-container` | `align-content:center;display:flex;gap:5px;margin-top:20px` |
| `.secondary-function .logo` | `background-image:url(../../static/media/icon_secondaryfunction.e4c96a5d.svg);background-size:40px 40px;height:40px;margin-left:10px;width:40px` |
| `.modal-patch-notes .modal-body>.header` | `margin-bottom:18px` |
| `.modal-patch-notes .modal-body>.header>.date` | `color:#44d62c;font-family:RazerF5,Roboto,Arial,Microsoft YaHei New,Microsoft Yahei,微软雅黑,宋体,SimSun,STXihei,华文细黑,sans-serif;font-size:24px;margin-right:10px` |
| `.modal-patch-notes .modal-body>.header>.size,.modal-patch-notes .modal-body>.header>.version` | `color:#999;margin-left:10px` |
| `.indicator-led-tooltip-body .header` | `margin-bottom:10px;text-transform:uppercase` |
| `.cp-tutorial .right .header` | `padding:8px` |
| `.cp-tutorial .right .header .close-btn` | `cursor:pointer;float:right;height:22px;width:22px` |
| `.btn-add-app .app-name` | `position:relative` |
| `.btn-add-app.disable .app-name-container` | `opacity:1;pointer-events:none` |
| `.btn-add-app .app-name-container` | `align-items:center;display:flex;justify-content:center;left:350px;overflow:visible;pointer-events:none;position:absolute;top:150px;width:290px;z-index:10` |
| `.btn-add-app .app-name-tip` | `word-wrap:break-word;background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;opacity:0;padding:8px 10px;text-align:center;transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:normal;width:100%;z-index:20` |
| `.btn-add-app:hover .app-name-container .app-name-tip` | `opacity:1;visibility:visible` |
| `.flex-item-app .preset-icon .app-name-container` | `align-items:center;display:flex;justify-content:center;left:16px;overflow:visible;pointer-events:none;position:absolute;top:40px;z-index:10` |
| `.flex-item-app .preset-icon .app-name-tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;opacity:0;padding:8px 10px;text-align:center;transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:nowrap;z-index:20` |
| `.flex-item-app .preset-icon:hover .app-name-container .app-name-tip` | `opacity:1;pointer-events:auto;visibility:visible` |
| `.header-avatar` | `position:relative` |
| `.header-avatar .dropdown-razer` | `min-width:223px` |
| `.header-avatar>.box` | `align-items:center;cursor:default;display:flex;height:38px;justify-content:center;transition:background-color .1s ease-in-out;width:46px` |
| `.header-avatar.active>.box>.img-overlay,.header-avatar>.box:active>.img-overlay` | `opacity:.1` |
| `.header-avatar>.box>.img,.header-avatar>.box>.img-overlay` | `border-radius:50%;bottom:0;content:"";height:32px;left:0;margin:auto;position:absolute;right:0;top:0;width:32px` |
| `.header-avatar>.box>.img` | `background-image:url(../../static/media/user.53ceca5f.svg);background-position:50%;background-repeat:no-repeat;background-size:22px;height:20px;width:20px` |
| `.header-avatar>.box>.img.new-guest` | `background-image:url(../../static/media/guest.00470cfc.svg)` |
| `.header-avatar>.box>.img-overlay` | `background-color:#000;opacity:0;transition:opacity .1s linear;z-index:1` |
| `.header-avatar .dropdown-razer-2` | `margin-right:5px` |
| `.header-avatar .dropdown-item.balances,.header-avatar .dropdown-item.balances>*` | `align-items:center;display:flex` |
| `.header-avatar .dropdown-item.balances` | `display:flex;height:33px;padding:3px 18px;position:relative` |
| `.header-avatar .dropdown-item.balances>.loading` | `align-items:center;color:#888;display:inline-flex;padding-left:30px;position:relative` |
| `.header-avatar .dropdown-item.balances>.loading:before` | `background-image:url(../../static/media/z-spinner.07b69ca1.svg);background-position:50%;background-repeat:no-repeat;background-size:100%;bottom:0;content:"";height:26px;left:0;margin:auto;position:absolute;top:-1px;width:26px` |
| `.header-avatar .dropdown-item.balances>.gold,.header-avatar .dropdown-item.balances>.silver` | `flex-shrink:0;line-height:27px;min-width:50%;padding-left:35px;position:relative` |
| `.header-avatar .dropdown-item.balances>.silver` | `color:#30d5ff` |
| `.header-avatar .dropdown-item.balances>.gold:before,.header-avatar .dropdown-item.balances>.silver:before` | `background-position:50%;background-repeat:no-repeat;background-size:100%;bottom:0;content:"";display:block;height:27px;left:0;margin:auto;position:absolute;top:0;width:27px` |
| `.header-avatar .dropdown-item.balances>.gold` | `color:#f5a623;padding-right:10px` |
| `.header-avatar .dropdown-item.balances>.gold:before` | `background-image:url(data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADYAAAA2CAYAAACMRWrdAAAABHNCSVQICAgIfAhkiAAAFydJREFUaEPFmnmMnOV9x7/vO/fMzux9n961vT7whY2NgRgfxUAwOSDEaZsoCqnSqhEJRKlaqVWFov7RVBEkQWmjRGn6RyLFIYFwxRzCNjbY4APfa6+9XnvtvY/Z3bln3qvf53nfd3bWGAIJaV/x6h3GszPv5/3+7udR8DEd155oCfm9SpOl+Nu9qrlR8WG1pVgrALVb8ahQFA+gqLB4tUyFv6r2KpZy2tLyJ/RC4ZBiFQYKujXc+q3B7MdxS+IX/qTj0r93lkeCeFjxYqevzGj2lemNnoDhU` |
| `.header-avatar .dropdown-item.balances>.silver:before` | `background-image:url(data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADYAAAA2CAYAAACMRWrdAAAABHNCSVQICAgIfAhkiAAAD39JREFUaEPFmnuw1eMax99lC7kUm2hXRG0lRJSSS7oMGnIpEcatMuQ6yjmOmWOahvOHTFNhkEvHYFxStks7EilhUooItbNFdCOi7RKy7fP9PPazzrvffmu3Jef8Ztas22+9v+f7XL7P93l/Kxe20TF27NjG22+/fYtGjRq1/u2337oXFRV1yuVyHbfbbrv2egS9tgeHvuepYtOmTYurq6sX6fu5er3i119/XT1ixIiN28Kk36/0J45bb7216W677TZESwwSqJY777xzyU477VS0ww47BIELA` |
| `.header-avatar .loading>div` | `animation:l 1s steps(3) infinite;-webkit-clip-path:inset(0 1ch 0 0);clip-path:inset(0 1ch 0 0)` |
| `.header-avatar .loading>div:after` | `animation:l 1s steps(4) infinite;-webkit-clip-path:inset(0 3ch 0 0);clip-path:inset(0 3ch 0 0);content:"..."` |

---

## 侧边栏 / 设备（64 条）

| 选择器 | 声明 |
|---|---|
| `div.nav-tabs` | `align-items:center;background-color:#222;border-bottom:2px solid #000;color:#5d5d5d;min-height:48px;position:relative;width:100%;z-index:105` |
| `div.nav-tabs.disabled` | `opacity:.5` |
| `.nav-tabs` | `display:flex` |
| `.nav-tabs .profile-wrapper` | `display:flex;flex:1 0 25%` |
| `.nav-tabs .profile-wrapper .profile-del` | `left:auto` |
| `.nav-tabs .right` | `flex:1 1 25%` |
| `.nav-tabs .navs-wrapper` | `flex:1 0 max-content` |
| `.nav-tabs.keymapbar-enabled .keymap-bar` | `color:#ccc;justify-content:left;margin:0 0 0 20px` |
| `.nav-tabs.keymapbar-enabled .profile-wrapper` | `display:flex` |
| `.nav-tabs.keymapbar-enabled .navs-wrapper` | `flex:1 1 auto` |
| `.nav-tabs.keymapbar-enabled .keymap-bar-name` | `display:none` |
| `.nav-tabs.keymapbar-enabled .profile-wrapper,.nav-tabs.keymapbar-enabled .right` | `flex:1 1 25%` |
| `.nav-tabs .profile-bar` | `color:#ccc;justify-content:normal;margin:0;width:auto` |
| `.nav-tabs .profile-bar .loader` | `margin-left:10px;margin-right:0;z-index:105` |
| `.nav-tabs .help` | `align-items:center;border-radius:5px;display:flex;height:24px;justify-content:center;margin-right:10px;width:24px` |
| `.nav-tabs .help .help-icon` | `background-color:#0000!important;background-image:url(../../static/media/icon_help.377359c3.svg#default);height:24px;width:24px` |
| `.nav-tabs .help.active .help-icon` | `background-image:url(../../static/media/icon_help.377359c3.svg#active)` |
| `.nav-tabs .help:not(.active):hover .help-icon` | `background-image:url(../../static/media/icon_help.377359c3.svg#hover)` |
| `.nav-tabs .thx-wrapper` | `position:relative` |
| `.nav-tabs .thx-wrapper .warn-tip` | `left:auto;right:15px;top:30px` |
| `.nav-tabs .thx-wrapper .warn-pop` | `left:auto;right:5px` |
| `.nav-tabs .thx-wrapper .hover-border.show` | `border:1px solid #44d62c` |
| `.nav-tabs .navs-wrapper` | `display:flex;font-family:Roboto,sans-serif;font-size:12px;justify-content:center` |
| `.nav-tabs .navs-wrapper .dots3` | `background-image:url(../../static/media/icon_more_default.eb7284c1.svg);border:none;border-radius:13px` |
| `.nav-tabs .navs-wrapper .dots3:hover` | `background-color:#2d2d2d;background-image:url(../../static/media/icon_more.fb688d78.svg)` |
| `.nav-tabs .navs-wrapper .dots3 .act.action.uppercase` | `text-transform:uppercase` |
| `.nav-tabs .navs-wrapper .dots3 .act.action.active` | `background-color:#000;color:#44d62c` |
| `.nav-tabs .navs-wrapper .dots3 .act.action.active:hover` | `background-color:#1a1a1a;color:#44d62c` |
| `.nav-tabs .navs-wrapper .dots3 .act` | `color:#ccc;font-size:14px` |
| `.nav-tabs .navs-wrapper .dots3 .act:hover` | `background-color:#1a1a1a` |
| `.nav-tabs .navs-wrapper .dots3.has-actived-option` | `background-color:#44d62c;background-image:url(../../static/media/icon_more_active.8e04906f.svg)` |
| `.nav-tabs .nav` | `background-repeat:no-repeat;border-radius:14px;color:#999;line-height:14px;margin-right:20px;padding:7px 10px;text-align:center;text-transform:uppercase;transition:background-color .3s,color,.1s;white-space:nowrap;will-change:background-color,color` |
| `.nav-tabs .nav:last-child` | `margin-right:0` |
| `.nav-tabs .batt,.nav-tabs .warn,.nav-tabs .warnRestart` | `background-position:50%;background-repeat:no-repeat;background-size:20px;height:26px;margin:0 10px;width:26px` |
| `.nav-tabs .warn` | `background-image:url(../../static/media/warning.ad3f47f8.svg);margin:0 5px` |
| `.nav-tabs .restartWarn,.nav-tabs .restartWarn-wrapper` | `align-items:center;display:flex` |
| `.nav-tabs .restartWarn` | `justify-content:center;margin:0 5px;position:relative` |
| `.nav-tabs .restartWarn:after` | `background-image:url(../../static/media/icon_addon_error.5a00577f.svg);content:"";height:20px;position:absolute;width:20px` |
| `.nav-tabs .batt` | `background-image:url(../../static/media/icon_battery_100.b00b88f9.svg);margin:0 5px` |
| `.nav-tabs .batt[tooltip]:before` | `white-space:pre-wrap` |
| `.nav-tabs .batt.batt-warning` | `background-image:url(../../static/media/icon_battery_error.44052666.svg)` |
| `.nav-tabs .batt.batt-warning[tooltip]:before` | `color:#ccc;font:normal normal normal 14px/16px Roboto,sans-serif;height:auto;letter-spacing:0;margin-right:16px;opacity:1;text-align:left;white-space:inherit;width:352px` |
| `.nav-tabs .arrow` | `background-position:50%;background-size:9px 18px;height:48px;max-width:40px;padding:0;width:40px` |
| `.nav-tabs .nav.active:hover` | `background-color:#44d62c;color:#111` |
| `.nav-tabs .nav:hover` | `background-color:#2d2d2d;color:#ccc` |
| `.nav-tabs .nav:active` | `background-color:#3cbf27;color:#111` |
| `.nav-tabs .user.disabled:hover,.nav.disabled:hover` | `background-color:#0000;cursor:default` |
| `.nav-tabs .right` | `align-items:center;display:flex;justify-content:flex-end` |
| `.nav-tabs .right .help-icon[tooltip]:before` | `right:5px` |
| `.dashboard .box-flip .box-flip-inner .box-flip-back .body .device-list ul` | `padding-left:10px` |
| `.iot-device-list .device-list-title` | `align-items:center;display:flex;margin:0 auto 10px;width:420px` |
| `.iot-device-list .device-list-title .title-content` | `color:#ccc;font-size:14px;margin-block:0;margin-right:10px;text-align:left;text-transform:uppercase` |
| `.iot-device-list .device-list-title .loading-icon` | `display:block;height:auto;margin-left:-3px;width:auto` |
| `.iot-device-list .device-list-title .mac-tooltip-wrapper` | `margin-right:10px;position:relative;z-index:5` |
| `.iot-device-list .device-list-title .mac-tooltip-wrapper .tooltip-icon` | `display:block;height:14px;width:14px` |
| `.iot-device-list .device-list-title .mac-tooltip-wrapper .tooltip-wrapper` | `box-sizing:border-box;left:0;padding-top:5px;position:absolute;top:100%;visibility:hidden;width:305px` |
| `.iot-device-list .device-list-title .mac-tooltip-wrapper .tooltip-content` | `background-color:#111;border:1px solid #5d5d5d;padding:8px 10px` |
| `.iot-device-list .device-list-title .mac-tooltip-wrapper .tooltip-content img` | `border-radius:3px;margin-top:3px;z-index:5` |
| `.iot-device-list .device-list-title .mac-tooltip-wrapper .tooltip-description` | `margin-bottom:10px;padding:0;text-align:left` |
| `.iot-device-list .device-list-title .mac-tooltip-wrapper:hover .tooltip-wrapper` | `visibility:visible` |
| `.iot-device-list .no-device-found` | `font-size:14px` |
| `.cp-tutorial .left .sidebar` | `display:flex;flex-direction:column` |
| `.cp-tutorial .left .sidebar .sidebar-item` | `color:#ccc;cursor:pointer;font-size:14px;font-weight:400;line-height:17px;padding:12px 0 12px 30px` |
| `.cp-tutorial .left .sidebar .sidebar-item.active` | `background-color:#111` |

---

## 标签栏（50 条）

| 选择器 | 声明 |
|---|---|
| `.modes-tab .slash` | `position:absolute;right:-5px;top:4px` |
| `.modes-tabs.round-corner` | `border:1px solid #5d5d5d;border-radius:3px;overflow:hidden` |
| `.modes-tabs.fix-h-27` | `height:27px;min-height:27px` |
| `.modes-tabs.fix-h-27 .modes-tab` | `height:25px;padding:4px 16px` |
| `.modes-tabs.fix-h-27.wave-dir.more` | `align-items:start;overflow:visible;overflow:initial` |
| `.modes-tabs.fix-h-27.wave-dir.more .modes-tab` | `background-color:#0000;border:1px solid #0000;transition:border .2s;width:51px;will-change:border` |
| `.modes-tabs.fix-h-27.wave-dir.more>div:first-child` | `border-bottom-left-radius:3px;border-top-left-radius:3px;margin-left:-1px` |
| `.modes-tabs.fix-h-27.wave-dir.more>div:last-child` | `border-bottom-right-radius:3px;border-top-right-radius:3px;margin-right:-1px` |
| `.modes-tabs.fix-h-27.wave-dir.more .modes-tab.active,.modes-tabs.fix-h-27.wave-dir.more .modes-tab.active:active,.modes-tabs.fix-h-27.wave-d` | `background-color:#44d62c` |
| `.modes-tabs.uppercase .modes-tab` | `text-transform:uppercase` |
| `.modes-tabs.wave-dir.more.modes-tabs.fix-h-27 .modes-tab:hover` | `border:1px solid #44d62c;height:27px;margin-top:-1px;transition:border 0s;will-change:border` |
| `.multiple-tabs-wrapper` | `height:100%` |
| `.multiple-tabs-wrapper .multiple-tabs` | `align-items:center;background-color:#111;border:1px solid #5d5d5d;border-radius:18px;display:flex;height:36px;justify-content:center;padding:4px;transition:border-color .2s,background-color .2s,color .2s;width:-webkit-fit-content;width:fit-content;will-change:border-color,background-color,color` |
| `.multiple-tabs-wrapper .multiple-tabs:hover` | `border:1px solid #44d62c` |
| `.multiple-tabs-wrapper .multiple-tabs .multiple-tab` | `align-items:center;border-radius:13px;display:flex;font-size:12px;height:100%;justify-content:center;min-width:36px;padding:5px 10px;transition:border-color .2s,background-color .2s,color .2s;will-change:border-color,background-color,color` |
| `.multiple-tabs-wrapper .multiple-tabs .multiple-tab--medium` | `font-size:14px` |
| `.multiple-tabs-wrapper .multiple-tabs .multiple-tab:not(:last-child)` | `margin-right:5px` |
| `.multiple-tabs-wrapper .multiple-tabs .multiple-tab--active` | `background-color:#44d62c;color:#111` |
| `.multiple-tabs-wrapper .multiple-tabs .multiple-tab .multitab-icon` | `align-items:center;display:flex;flex:1 1;height:20px;justify-content:center;padding:0 5px;width:20px` |
| `.multiple-tabs-wrapper .multiple-tabs .multiple-tab .multitab-icon--active` | `filter:invert(1)` |
| `.multiple-tabs-wrapper .multiple-tabs .multiple-tab span` | `-webkit-line-clamp:2;-webkit-box-orient:vertical;display:-webkit-box;flex:3 1;overflow:hidden;text-overflow:ellipsis` |
| `.cb-tabs` | `border:2px solid #d3d3d3;border-radius:5px;display:flex;flex-direction:row;font-size:.9rem` |
| `.cb-tabs>.tab-item` | `background-color:#000;border-left:1px solid #d3d3d3;border-right:1px solid #d3d3d3;color:#fff;flex:auto;padding:10px 20px` |
| `.cb-tabs>div:first-child` | `border-left:none` |
| `.cb-tabs>div:last-child` | `border-right:none` |
| `.cb-tabs>.tab-item.active,.cb-tabs>.tab-item:hover` | `background-color:#44d62c;color:#000` |
| `.two-way-tab-sensitivity` | `height:72px;padding-top:12px` |
| `#multipleBrightness .disabled-lighting-tab` | `opacity:.3;pointer-events:none` |
| `#multipleBrightness .multiple-quickeffect .quickeffect .modes-tabs` | `flex-wrap:nowrap` |
| `.haptic-content .chart-tab` | `border:1px solid #5d5d5d` |
| `.two-way-tab-container` | `position:relative;z-index:2` |
| `.modes-tabs` | `align-items:center;flex-wrap:wrap;float:left;margin-left:1px;margin-top:1px;min-height:32px;transition:border .3s;will-change:border` |
| `.modes-tab` | `background-color:#111;border:1px solid #5d5d5d;color:#ccc;flex:0 0 auto!important;font-size:14px;height:32px;line-height:17px;margin-left:-1px;margin-top:-1px;opacity:1;padding:7px 15px 8px;position:relative;text-align:left;text-align:center;text-transform:capitalize;transition:opacity .1s,color .2s,background-color .2s;will-change:opacity,color,background-color` |
| `.modes-tab.lighting-effect` | `top:50%;transform:translateY(-50%)` |
| `.modes-tab.active` | `background-color:#44d62c;color:#111` |
| `.modes-tab:hover` | `background-color:#ffffff1a` |
| `.modes-tab:active` | `background-color:#0000001a` |
| `.modes-tab.active:hover` | `background-color:#44d62c` |
| `.modes-tab.line2` | `top:-1px` |
| `.modes-tab:active` | `opacity:.7` |
| `.modes-tabs.no-inner-border` | `margin-right:0;margin-top:0` |
| `.modes-tabs.no-inner-border .modes-tab` | `border:none;margin-left:0;margin-top:0` |
| `.modes-tabs.dual .modes-tab:hover,.modes-tabs.wave-dir.more .modes-tab:hover` | `background-color:#111` |
| `.modes-tabs.dual .modes-tab.active:hover,.modes-tabs.wave-dir.more .modes-tab.active:hover` | `background-color:#44d62c` |
| `.modes-tabs.dual:hover` | `border:1px solid #44d62c` |
| `.modes-tabs.dual:active .modes-tab,.modes-tabs.wave-dir.more .modes-tab:active` | `background-color:#ffffff1a` |
| `.modes-tabs.dual:active .modes-tab.active,.modes-tabs.wave-dir.more:active .modes-tab.active` | `background-color:#44d62c` |
| `.modes-tabs.dual .modes-tab:active,.modes-tabs.wave-dir .modes-tab:active` | `opacity:1` |
| `.modes-tabs.wave-dir` | `margin-top:5px` |
| `.modes-tabs.wave-dir .modes-tab` | `background-position:50%;background-repeat:no-repeat;background-size:20px;width:51px` |

---

## 按钮（307 条）

| 选择器 | 声明 |
|---|---|
| `.thx-btn` | `background-color:#44d62c;border-radius:3px;color:#000;display:inline-block;padding:.5rem 1.5rem;text-align:center;text-transform:uppercase;transition:opacity .3s;-webkit-user-select:none;user-select:none;width:auto;will-change:opacity` |
| `.thx-btn:hover` | `opacity:.8` |
| `.thx-btn:active` | `opacity:.6` |
| `.thx-btn.sm` | `display:block!important;font-size:12px;height:27px;line-height:11px;margin:20px auto auto;width:90px` |
| `.thx-btn.inline` | `max-width:90px;padding:7px 0 6px` |
| `.thx-btn.fit,.thx-btn.inline` | `border:1px solid #0000004d;display:block!important;font-size:12px;height:27px;line-height:12px;margin:auto` |
| `.thx-btn.fit` | `max-width:-webkit-fit-content;max-width:fit-content;padding:7px 5px 6px` |
| `.thx-btn.test` | `background-color:#707070;border:1px solid #0000004d;color:#fff;display:block!important;margin:auto` |
| `.thx-btn.success` | `background-color:#44d62c!important;color:#111!important` |
| `.thx-btn.disabled,.thx-btn.disabled:hover` | `cursor:default;opacity:.3` |
| `.thx-btn.secondary` | `background-color:#707070;color:#fff;display:block!important;margin:auto` |
| `.hover-btn` | `transition:background-color .2s,opacity .2s;will-change:background-color,opacity` |
| `.hover-btn:hover` | `background-color:#ffffff1a` |
| `.hover-btn:active` | `opacity:.9` |
| `.import-profile-btn-group` | `display:inline-flex;flex-shrink:0;height:27px` |
| `.import-profile-btn-group .thx-btn` | `border:1px solid #0000004d;font-family:Roboto;font-size:12px;height:100%;line-height:0px;text-align:center;width:-webkit-fit-content;width:fit-content` |
| `.import-profile-btn-group .warning` | `align-items:center;display:flex;margin-left:10px;position:relative` |
| `.import-profile-btn-group .warning .tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-family:Roboto;font-size:14px;left:-113px;line-height:16px;opacity:0;padding:8px 10px 10px;position:absolute;text-transform:none;top:-200%;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:300px;will-change:visibility,opacity` |
| `.import-profile-btn-group .warning:hover>.tip` | `opacity:1;visibility:visible;z-index:100` |
| `.main-nav .icon-button:hover svg` | `fill:#44d62c` |
| `.main-nav .icon-button:active` | `fill:#44d62c;opacity:.7` |
| `.profile-del div.thx-btn` | `background-color:#fd4949;color:#111;line-height:14px;padding:4px 5px;white-space:nowrap` |
| `.profile-del div.thx-btn.cancel` | `background-color:#707070;color:#fff;margin-right:10px` |
| `.restart .btn-restart` | `gap:10px` |
| `.reshare__btn` | `background-color:#707070;background-color:#44d62c;border:1px solid #000;border-radius:3px;color:#000;cursor:pointer;font-size:12px;height:27px;line-height:14px;padding:6px 0 7px;text-align:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:100px` |
| `.reshare__btn,.reshare__edit` | `transition:opacity .3s;will-change:opacity` |
| `.dropdown-razer-2.dropdown-update .btns` | `margin-top:20px;text-align:center` |
| `.dropdown-razer-2.dropdown-update .btns>.btn` | `border-radius:2px;flex-shrink:0;font-size:12px;line-height:1;min-width:90px;padding:7px 16px 6px;text-align:center;text-transform:uppercase;white-space:nowrap` |
| `.toggle-hyper.drawer-open.hover-btn` | `transition:background-color .2s,opacity .2s,color .2s;will-change:background-color,opacity,color` |
| `.toggle-hyper.drawer-open.hover-btn:active` | `opacity:.9` |
| `.toggle-hyper.hyper-on.hover-btn` | `transition:background-color .2s,opacity .2s,color .2s;will-change:background-color,opacity,color` |
| `.toggle-hyper.hyper-on.hover-btn:hover` | `background-color:#fd922a` |
| `.toggle-hyper.hyper-on.hover-btn:active` | `opacity:.9` |
| `.config-block .config-btns` | `max-height:100%;max-width:235px` |
| `.config-btns-wrapper,.config-btns-wrapper .interdevice-icon-show-wrapper` | `position:relative` |
| `.config-btns-wrapper .interdevice-icon-show-wrapper .tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-family:Roboto;line-height:16px;opacity:0;padding:0 2px;position:absolute;top:140%;transition:visibility 0s,opacity .3s linear;visibility:hidden;will-change:visibility,opacity` |
| `.config-btns-wrapper .interdevice-icon-show-wrapper:hover>.tip` | `opacity:1!important;visibility:visible!important;z-index:100!important` |
| `.config-btns-wrapper .require-synape-icon` | `align-items:center;background-color:#008900;border:1px solid #5d5d5d;border-radius:50%;display:flex;height:32px;justify-content:center;position:absolute;top:0;width:32px` |
| `.config-btns-wrapper .require-synape-icon:hover>.tip` | `opacity:1;visibility:visible;z-index:100` |
| `.config-btns-wrapper .require-synape-icon img` | `pointer-events:none` |
| `.config-btns-wrapper .require-synape-icon.hyper-shift` | `background-color:#fd8611` |
| `.config-btns-wrapper .tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-family:Roboto;font-size:14px;line-height:16px;opacity:0;padding:8px 10px 10px;position:absolute;transition:visibility 0s,opacity .3s linear;visibility:hidden;will-change:visibility,opacity;z-index:100` |
| `.config-btns-wrapper .tip div` | `margin-top:5px` |
| `.config-btns-wrapper .tip .tip-content` | `font-family:Roboto;font-size:12px;margin-top:0;padding:3px 0 6px;text-align:center;text-transform:uppercase` |
| `.config-btns-wrapper .tip .tip-content .btn-default-name,.config-btns-wrapper .tip .tip-content .device-name` | `color:#707070` |
| `.config-btns-wrapper .tip .tip-content .btn-default-name` | `font-size:14px` |
| `.config-btns-wrapper .tip .tip-content .mapping-name` | `font-size:18px;text-transform:capitalize` |
| `.config-btns-wrapper .tip .tip-message` | `font-size:12px;padding:3px 26px;text-align:center` |
| `.config-btns-wrapper .tip .device-not-connected` | `background-color:#c8323c;color:#000` |
| `.config-btns-wrapper .tip .require-synapse-message` | `color:#000` |
| `.config-btns-wrapper .tip.scrollmode-switch` | `align-items:center;display:flex;font-size:12px;left:-30%;top:-200%;width:305px` |
| `.config-btns-wrapper .tip.scrollmode-switch div` | `margin:0` |
| `.config-btns-wrapper .tip.scrollmode-switch .scrollmode-switch-bg` | `background-image:url(../../static/media/scrollmode_switch_bg.cd01ecda.svg);height:56px;width:22px` |
| `.config-btns-wrapper .tip.scrollmode-switch .scrollmode-switch-top` | `background-image:url(../../static/media/scrollmode_switch_top.5d132bac.svg);height:43px;margin-left:4px;margin-top:3px;width:14px` |
| `.config-btns-wrapper .tip.scrollmode-switch .scrollmode-content` | `flex:1 1;margin-left:10px` |
| `.config-btns-wrapper .tip.scrollmode-switch .scrollmode-content .description` | `margin-bottom:10px` |
| `.config-btns-wrapper .tip.scrollmode-switch .scrollmode-content .nudge-backward` | `opacity:.3` |
| `.config-btns-wrapper .config-btn:hover+.tip,.config-btns-wrapper.wrapper-disabled:hover>.tip` | `opacity:1;visibility:visible;z-index:100` |
| `.config-btns-wrapper.wrapper-disabled:hover>.tip.scrollmode-switch .nudge-backward,.config-btns-wrapper.wrapper-disabled:hover>.tip.scrollmo` | `animation-duration:6.4s;animation-iteration-count:infinite;animation-name:scrollmode-switch` |
| `.config-btns-wrapper.wrapper-disabled:hover>.tip.scrollmode-switch .push-forward` | `animation-name:scrollmode-opacity` |
| `.config-btns-wrapper.wrapper-disabled:hover>.tip.scrollmode-switch .nudge-backward` | `animation-name:scrollmode-opacity-reverse` |
| `.config-btns-list` | `border:1px solid #5d5d5d;border-radius:5px;box-shadow:0 6px 10px #0003;display:flex;flex-direction:column;isolation:isolate;max-height:100%;padding:0;position:absolute;width:242px;z-index:2` |
| `.config-btns-list .tittle` | `align-items:center;align-self:stretch;background:#2d2d2d;border-radius:5px 5px 0 0;display:flex;flex:none;flex-direction:row;flex-grow:0;isolation:isolate;justify-content:center;order:0;padding:10px 20px;position:relative;width:240px;z-index:1` |
| `.config-btns-list .tittle .text` | `color:#999;font-size:14px;font-weight:400;text-transform:uppercase` |
| `.config-btns-list .tittle .dots3.hover-border` | `border-radius:3px;height:90%;position:absolute;right:-8px;width:32px` |
| `.config-btns-list .content` | `background-color:#111;border-radius:0 0 5px 5px;box-shadow:0 6px 10px #0003;flex-direction:column;padding:10px 20px;width:240px` |
| `.config-btns-list .content .item` | `align-items:center;display:flex;flex-direction:row` |
| `.config-btns-list .content .item>img` | `height:16px;padding:4px;width:16px` |
| `.config-btns-list .content .item .remapped` | `color:#44d62c` |
| `.config-btns-list .content .custom-config-btn .config-btn` | `margin:8px 0 8px 10px` |
| `.config-btns-list .content .custom-config-btn .config-btn .interdevice-icon-show-wrapper span` | `max-width:150px;padding-top:2px` |
| `.config-btns-list .content .config-btn` | `align-items:center;background-color:#0000;border-radius:3px;color:#ccc;display:flex;flex:0 0 auto;font-size:14px;height:30px;line-height:17px;margin-bottom:20px;max-width:180px;overflow:hidden;padding:6px 10px 7px;text-overflow:ellipsis;transition:background-color .2s,color .2s;white-space:nowrap;will-change:background-color,color` |
| `.config-btns-list .content .remap-2-disabled` | `color:#c8323c` |
| `.config-btns-list .content .info-dimmed` | `color:#ccc;opacity:.3;pointer-events:auto` |
| `.config-btns-list .content .warning-dimmed` | `color:#c8323c;opacity:.3;pointer-events:auto` |
| `.config-btns-list .content .config-btn:hover` | `background-color:#383838` |
| `.config-btns-list .content .config-btn:hover .interdevice-icon-show-wrapper .tip` | `opacity:1!important;visibility:visible!important;z-index:100!important` |
| `.config-btns-list .content .config-btns-wrapper.config-btn.remapped` | `color:#44d62c` |
| `.config-btns-list.left` | `left:-9px;top:90px` |
| `.config-btns-list.right` | `right:-9px;top:90px` |
| `.config-btns-list.bottom` | `left:calc(50% - 100px);top:calc(100% - 30px)` |
| `.config-btns` | `flex-direction:column;position:absolute` |
| `.config-btns.left` | `align-items:flex-end;padding-right:15px;right:535px;z-index:2` |
| `.config-btns.right` | `align-items:flex-start;left:535px;padding-left:15px;z-index:2` |
| `.config-btns .config-btn` | `align-items:center;background-color:#0000;border-radius:3px;color:#ccc;display:flex;flex:0 0 auto;font-size:14px;height:30px;line-height:17px;margin-bottom:20px;overflow:hidden;padding:0 10px;text-overflow:ellipsis;transition:background-color .2s,color .2s;white-space:nowrap;will-change:background-color,color` |
| `.config-btns .config-btn.disable-default:hover` | `background-color:red` |
| `.config-btns .config-btn:hover` | `background-color:#383838` |
| `.config-btns .config-btn:hover .interdevice-icon-show-wrapper .tip` | `opacity:1!important;visibility:visible!important;z-index:100!important` |
| `.config-btns .config-btn.hovered` | `background-color:gray` |
| `.config-btns .config-btn.disabled` | `color:#cccccc4d;pointer-events:none` |
| `.config-btns .config-btn.active,.config-btns .config-btn:active` | `background-color:#111` |
| `.config-btns .config-btn.remap-2-disabled` | `color:#c8323c` |
| `.config-btns .config-btn.remapped` | `color:#44d62c` |
| `.config-btns .config-btn.remapped.hyper-shift` | `color:#fd8611` |
| `.config-btns .has-icon .config-btn` | `max-width:none` |
| `.config-btns .has-icon .interdevice-icon-show` | `max-width:210px` |
| `.button-description` | `max-width:250px;overflow:hidden;text-overflow:ellipsis;white-space:pre` |
| `.button-description.flex-button` | `align-items:flex-start;display:flex;flex-direction:column;padding:2px 0` |
| `.button-description .description` | `color:#44d62c;font-size:10px;line-height:12px;text-transform:uppercase` |
| `.button-description .description.hyper-shift` | `color:#fd8611` |
| `.drawer-btn` | `align-items:center;background-color:#0000;height:auto;min-height:50px;padding:9px;position:relative;transition:background-color .2s;will-change:background-color` |
| `.drawer-btn .binding-value` | `line-height:16px;overflow:hidden;pointer-events:auto;text-overflow:ellipsis` |
| `.drawer-btn .index` | `display:flex;flex-direction:column;position:relative` |
| `.drawer-btn .index .wrapper .index-element` | `overflow:hidden!important;overflow-wrap:normal;text-overflow:ellipsis;white-space:nowrap` |
| `.drawer-btn .index .wrapper:hover .tip` | `opacity:1;visibility:visible;z-index:100` |
| `.drawer-btn .no-hover` | `pointer-events:none` |
| `.drawer-btn .tip` | `left:20px;opacity:0;position:absolute;text-align:left;top:35px;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:200px;will-change:visibility,opacity;z-index:99` |
| `.drawer-btn .tip .tip-text` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;display:inline-block;font-family:Roboto;font-size:14px;line-height:16px;padding:8px 10px` |
| `.drawer-btn:hover` | `background-color:#383838` |
| `.drawer-btn.active,.drawer-btn:active` | `background-color:#111` |
| `.drawer-btn.disabled,.drawer-btn:disabled` | `color:#cccccc4d;pointer-events:none` |
| `.drawer-btn.remapped` | `color:#44d62c` |
| `.drawer-btn.remapped.hyper-shift` | `color:#fd8611` |
| `.drawer-btn.flex>div` | `flex:0 0 auto;overflow-wrap:normal;text-overflow:ellipsis` |
| `.drawer-btn .index` | `color:#ccc` |
| `.drawer-btn .index,.drawer-btn.disabled .index` | `font-size:14px;letter-spacing:.03em;line-height:14px;margin-left:-10px;text-align:center;width:56px` |
| `.drawer-btn.disabled .index` | `color:#707070` |
| `.drawer-btn .key` | `color:#ccc` |
| `.drawer-btn .key,.drawer-btn.disabled .key` | `border-left:1px solid #707070;font-size:14px;line-height:16px;padding-left:5px;pointer-events:none;text-align:left;width:164px` |
| `.drawer-btn.disabled .key` | `color:#707070` |

> 另有 187 条未列出。

---

## 开关（53 条）

| 选择器 | 声明 |
|---|---|
| `.cloud-switch` | `background-color:#222;box-sizing:border-box;display:flex;flex-shrink:0;height:56px;height:49px;justify-content:center;padding:10px 0 20px;width:100%` |
| `.cloud-switch .cloud-wrapper` | `background-color:#111;border:1px solid #5d5d5d;border-radius:18px;height:36px;padding:5px;position:relative;transition:border-color .2s,background-color .2s;will-change:border-color,background-color` |
| `.cloud-switch .cloud-wrapper:hover` | `border-color:#44d62c` |
| `.cloud-switch .cloud-wrapper:active` | `background-color:#292929` |
| `.cloud-switch .cloud-wrapper .text` | `color:#212121;display:inline-block;font-size:14px;height:24px;line-height:14px;padding:6px 10px 5px;position:relative;text-transform:capitalize;transition:color .2s;will-change:color;z-index:2` |
| `.cloud-switch .cloud-wrapper .text.standard` | `background-color:#44d62c;border-radius:12px;margin-right:5px` |
| `.cloud-switch .cloud-wrapper .text.cloudshift` | `color:#ccc` |
| `.cloud-switch .cloud-wrapper.cloud-on .text.standard` | `background-color:#0000;color:#ccc` |
| `.cloud-switch .cloud-wrapper.cloud-on .text.cloudshift` | `background-color:#44d62c;border-radius:12px;color:#212121` |
| `.cloud-switch .cloud-wrapper .toggle` | `background-color:#44d62c;border-radius:12px;height:24px;left:5px;position:absolute;transition:left .2s ease,right .2s ease,width 0s,background-color .2s;will-change:left,right,width,background-color;z-index:1` |
| `.cloud-switch .cloud-wrapper.cloud-on .toggle` | `background-color:#44d62c;left:86px;width:84px` |
| `.cloud-switch .cloud-wrapper .toggle:before` | `color:#44d62c;content:"standard";display:inline-block;font-size:14px;height:24px;line-height:14px;padding:6px 10px 5px;text-transform:capitalize;transition:color .2s;will-change:color` |
| `.cloud-switch .cloud-wrapper.cloud-on .toggle:before` | `color:#44d62c;content:"cloudshift"` |
| `.switch` | `background-color:#707070;border:1px solid #0000004d;border-radius:16px;display:inline-block;height:18px;padding:2px;position:relative;transition:background-color .3s;width:32px;will-change:background-color` |
| `.switch:hover` | `opacity:.7` |
| `.switch.on` | `background-color:#44d62c` |
| `.switch .handle` | `background-color:#111;border-radius:7px;height:14px;left:1px;position:absolute;top:1px;transition:left .2s;width:14px;will-change:left` |
| `.switch.on .handle` | `left:15px` |
| `.h2-title .switch` | `left:10px;position:relative;top:3px` |
| `.mode-switcher .warning-alert` | `align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:360px;z-index:99` |
| `.mode-switcher .warning-alert .title` | `color:#fd8611;display:flex;margin-bottom:10px` |
| `.mode-switcher .warning-alert .title span` | `margin-right:4px` |
| `.mode-switcher .warning-alert .content` | `white-space:pre-wrap` |
| `.description-eq-map .switch-eq-title` | `align-items:center;display:flex;justify-content:flex-start;margin-bottom:5px;text-transform:uppercase` |
| `.description-eq-map .switch-eq-title:before` | `background-color:#0000;content:" ";display:block;height:11px;margin-right:6px;width:11px` |
| `.description-eq-map .switch-eq-title.top:before` | `background-image:url(../../static/media/double-press.4e4a3975.svg);background-position:50%;background-repeat:no-repeat;background-size:auto` |
| `.description-eq-map .switch-eq-title.bottom:before,.description-eq-map .switch-eq-title.mid:before` | `background-image:url(../../static/media/press-and-hold.0c1d60a8.svg);background-position:50%;background-repeat:no-repeat;background-size:auto` |
| `.description-eq-map .switch-eq-item` | `color:#999` |
| `.description-volume-map .switch-eq-title` | `margin-bottom:5px;text-transform:uppercase` |
| `.description-volume-map .switch-eq-title .icon-info` | `height:15px;width:15px` |
| `.description-volume-map .description-stages,.description-volume-map .switch-eq-item` | `color:#999;line-height:17px` |
| `.description-volume-map .switch-eq-item,.description-volume-map .switch-eq-title` | `align-items:center;display:flex;gap:4px` |
| `.knob-item .knob-switch` | `margin-right:9px;max-height:18px` |
| `.kb-flex .switch` | `display:inline-block;margin-left:10px;position:relative;top:5px` |
| `.stage-control .switch` | `margin-left:10px;margin-top:-1px` |
| `.stage:hover .icon-draggable,.stage:hover .icon-sensitivity-xy,.stage:hover .switch` | `display:block` |
| `.stage.one-stage:hover .icon-draggable,.stage.one-stage:hover .switch` | `display:none` |
| `.stage .switch` | `margin-left:15px` |
| `.stage .switch.disabled` | `opacity:.3` |
| `.stage .icon-draggable,.stage .icon-sensitivity-xy,.stage .switch` | `display:none` |
| `.switchkey` | `margin-bottom:46px` |
| `.switchkey .dropdown-area` | `margin-left:30px!important;width:180px!important` |
| `.switchkey .check-text` | `min-width:0;min-width:auto;width:180px` |
| `.group-button .switch_button` | `border:0;border-radius:20px;font-family:Roboto,sans-serif;font-size:12px;font-weight:400;height:22px;line-height:14px;margin:0 2px;min-width:40px;padding:4px 8px;text-transform:uppercase` |
| `.group-button .switch_button.active` | `background-color:#44d62c;color:#212121` |
| `.stages.draggable .stage .switch` | `display:block;margin-left:0` |
| `.stages.draggable .stage.not-checked .switch` | `cursor:pointer` |
| `#multipleBrightness .brightness-container .switching-brightness` | `align-items:center;display:inline-flex;margin:0 10px 10px` |
| `#multipleBrightness .brightness-container .switching-brightness .label-brightness` | `color:#44d62c;font-size:16px;margin-right:10px` |
| `.drag-sw-item .switch` | `min-width:32px` |
| `.profile-switching` | `align-items:start;display:flex;flex-direction:column;height:94px;justify-content:space-evenly` |
| `.profile-switching .title` | `text-transform:uppercase` |
| `.profile-switching-des` | `color:#999;height:-webkit-fit-content;height:fit-content` |

---

## 滑杆 / 步进（145 条）

| 选择器 | 声明 |
|---|---|
| `.battery-health .disabled .slider-container,.performance-custom-boost.disabled .slide-off.slide-on .recommended.disabled,.performance-custom` | `opacity:1` |
| `.slider-container` | `forced-color-adjust:none;height:64px;opacity:.3;pointer-events:none;position:relative;transition:opacity .3s;will-change:opacity` |
| `.slider-container.brightness` | `pointer-events:auto` |
| `.slider-container.on` | `opacity:1;pointer-events:auto` |
| `.slider-container.no-pointer` | `pointer-events:inherit` |
| `.slider-more` | `-webkit-appearance:none;background:#0000;border-radius:3px;bottom:25px;height:17px;margin:0;opacity:1;outline:none;position:absolute;transition:opacity .2s;width:100%;z-index:3` |
| `.slider-more.gradient` | `height:25px;width:25px` |
| `.slider-more::-webkit-slider-thumb` | `-webkit-appearance:none;appearance:none;background-image:url(../../static/media/path.0086a00e.svg);background-position:50%;background-repeat:no-repeat;background-size:cover;border-radius:3px;box-sizing:border-box;height:20px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:14px;will-change:transform,background` |
| `.slider` | `-webkit-appearance:none;background:#0000;border-radius:3px;bottom:25px;height:6px;margin:0;opacity:1;outline:none;position:absolute;transition:opacity .2s;width:100%;z-index:3` |
| `.slider.gradient` | `height:25px;width:25px` |
| `.slider-battery,.slider-left` | `left:50%;width:30%;z-index:2` |
| `.slider-left` | `background:#707070;border-radius:3px;bottom:25px;height:6px;position:absolute` |
| `.slider::-webkit-slider-thumb` | `-webkit-appearance:none;appearance:none;background:#44d62c;border-radius:8px;box-sizing:border-box;height:16px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:16px;will-change:transform,background` |
| `.slider-container.on .slider::-webkit-slider-thumb:hover` | `background:#5d5d5d;border:2px solid #44d62c` |
| `.slider-container.on .slider::-webkit-slider-thumb:active` | `background:#383838;border:2px solid #44d62c` |
| `.slider-tip,.thumb-tag` | `color:#212121;font-size:12px;pointer-events:none;position:absolute` |
| `.slider-tip` | `background-color:#44d62c;border-radius:3px;bottom:42px;line-height:14px;opacity:1;padding:4px 8px;transition:opacity .3s;transition:left 0s,opacity 0s;width:-webkit-max-content;width:max-content;will-change:opacity;will-change:left,opacity` |
| `.slider-tip.gradient` | `height:22px;width:22px` |
| `.slider-tip.tag-config` | `width:55px` |
| `.slider-container.no-tip` | `height:36px` |
| `.slider-container.no-tip .slider-tip` | `display:none` |
| `.slider-container .left` | `left:0;max-width:100%;width:50%` |
| `.slider-container .left,.slider-container .right` | `background:#44d62c;border-radius:3px;bottom:25px;height:6px;position:absolute;z-index:2` |
| `.slider-container .right` | `left:50%;max-width:30%` |
| `.slider-container .track` | `background:#44d62c4d;border-radius:3px;bottom:25px;height:6px;position:absolute;width:100%;z-index:1` |
| `.slider-container .foot` | `bottom:-2px;opacity:1;position:absolute;text-transform:uppercase;transition:visibility 0s,opacity .3s linear;visibility:visible;will-change:visibility,opacity` |
| `.slider-container.no-tag .foot` | `display:none` |
| `.slider-container.on .foot` | `opacity:1;visibility:visible` |
| `.slider-container .title-more` | `bottom:-25px;color:#999;font-size:12px;line-height:14px;opacity:1;position:absolute;text-transform:uppercase;transition:visibility 0s,opacity .3s linear;visibility:visible;will-change:visibility,opacity` |
| `.foot.min.slider-mode` | `top:45px` |
| `.foot.mid.slider-mode` | `top:45px` |
| `.foot.max.slider-mode` | `top:45px` |
| `.slider-container.indent` | `margin-left:30px;width:490px` |
| `.hide-slider .slider-container` | `height:0;margin-left:30px;opacity:0;overflow:hidden;transition:height .3s,opacity .3s;width:490px;will-change:height,opacity` |
| `.hide-slider .slider-container.on` | `height:64px;margin-bottom:20px;opacity:1;overflow:visible;overflow:initial` |
| `.has-slider .slider-container` | `margin-left:30px;width:490px` |
| `.slider-container.large` | `width:300px!important` |
| `.slider-container.no-bordered .left,.slider-container.no-bordered .track` | `border-radius:0` |
| `.slider-container .range-custom` | `display:flex;margin-top:2px!important` |
| `.slider-container .range-custom .range-item` | `background-color:#204c19;display:flex;height:16px;justify-content:center;position:absolute;transform:translateX(-50%);width:1px` |
| `.slider-container .range-custom .range-item div` | `font-size:11px;margin-top:24px` |
| `.slider.thumb-sm::-webkit-slider-thumb` | `height:12px;width:12px` |
| `.slider-container.no-bordered .thumb-tag` | `font-size:10px;height:auto;line-height:2.2;margin-left:0;width:auto` |
| `.key-config .body .slider-container .foot` | `color:#999;font-size:14px` |
| `.key-config .stepper,.modes-area .stepper,.stepper` | `border:1px solid #5d5d5d;box-sizing:border-box;height:27px;position:relative;transition:opacity .2s;width:60px;will-change:opacity` |
| `.key-config .stepper,.modes-area .stepper input,.stepper` | `background-color:#111;border:none;box-sizing:border-box;color:#ccc;font-size:14px;height:25px;left:0;line-height:17px;padding:5px 18px 5px 5px;text-align:left;width:58px` |
| `.modes-area .effects-area[data-effect=audioMeter] .stepper input` | `background-color:#0000` |
| `.stepper .icon.spinner` | `width:18px` |
| `.icon.spinner.down.disabled,.icon.spinner.up.disabled,.stepper.disabled,.turbo-label.disabled` | `opacity:.3;pointer-events:none` |
| `.key-config .stepper.custom-keymapping-stepper` | `border:1px solid #5d5d5d` |
| `.key-config .stepper.custom-keymapping-stepper:hover` | `border:1px solid #44d62c` |
| `.key-config .stepper.custom-keymapping-stepper input` | `background-color:initial;text-align:left;text-align:initial;width:90%` |
| `.key-config .stepper.custom-keymapping-stepper .icon.spinner` | `opacity:1;visibility:visible;visibility:initial` |
| `.slider-container .slider-range` | `background-color:#707070;border-radius:3px;bottom:25px;height:6px;opacity:.3;position:absolute;width:100%;z-index:1` |
| `.slider-container .foot.limit` | `bottom:0;left:50%;position:absolute;transform:translateX(-50%)` |
| `.stepper` | `background:#111;border:1px solid #0000;height:26px;position:absolute;transition:border .3s;width:62px;will-change:border` |
| `.stepper input` | `background-color:#0000;border:none;box-sizing:border-box;color:#ccc;font-size:14px;height:24px;left:6px;line-height:14px;padding:0;position:absolute;right:0;text-align:left;top:0;width:38px;z-index:3` |
| `.stepper .icon.audio` | `background-image:url(../../static/media/icon_audio_3.b1be5331.svg);background-position:4px;background-size:20px;height:24px;left:0;transition:background-image .3s;width:24px;will-change:background-image` |
| `.stepper .icon.audio,.stepper .icon.spinner` | `background-repeat:no-repeat;position:absolute` |
| `.stepper .icon.spinner` | `background-color:#0000;background-position:50%;background-size:8px;height:12px;right:0;width:14px;z-index:4` |
| `.stepper .icon.spinner:hover` | `background-color:#ffffff1a` |
| `.stepper .icon.spinner:active` | `background-color:#0000001a` |
| `.stepper input[type=number]::-webkit-inner-spin-button,.stepper input[type=number]::-webkit-outer-spin-button` | `-webkit-appearance:none;bottom:0;opacity:0;position:absolute;right:0;top:0;width:14px` |
| `.stepper:focus-within,.stepper:hover` | `background:#111;border:1px solid #44d62c` |
| `.stepper:focus-within .icon.spinner.down,.stepper:focus-within .icon.spinner.up,.stepper:hover .icon.spinner.down,.stepper:hover .icon.spinn` | `opacity:1;visibility:visible` |
| `.sliderChart__container,.sliderChart__preset-list` | `display:flex;margin-bottom:20px` |
| `.sliderChart__preset-list` | `flex-wrap:wrap;gap:12px;position:relative;z-index:2` |
| `.sliderChart__preset-list.scale` | `width:calc(100% + 10px)` |
| `.sliderChart__preset-list.scale div` | `flex:1 1;padding:7px 0 6px` |
| `.sliderChart__preset-list div` | `background-color:#111;border:1px solid #5d5d5d;border-radius:3px;color:#ccc;font-size:12px;height:27px;line-height:14px;min-width:90px;padding:6px 16px 7px;text-align:center;transition:border-color .3s,background-color .3s;white-space:nowrap;will-change:border-color,background-color` |
| `.sliderChart__preset-list div:hover` | `background-color:#0000001a;border-color:#44d62c` |
| `.sliderChart__preset-list div.active,.sliderChart__preset-list div:active` | `background-color:#ffffff1a;border-color:#44d62c` |
| `.sliderChart__preset-list div.preset-tab2` | `align-items:center;display:flex;flex:1 1;height:36px;justify-content:center;position:relative` |
| `.sliderChart__preset-list .name` | `opacity:0;position:absolute;visibility:hidden;z-index:2` |
| `.sliderChart__preset-list div.preset-tab2:hover:after` | `background-color:#111;border:1px solid #5d5d5d;border-radius:2px;color:#fff;content:attr(data-name);display:block;padding:7px 10px;position:absolute;top:42px` |
| `.sliderChart__preset-list div.preset-tab2:before` | `background-color:#0000;background-position:50%;background-repeat:no-repeat;content:" ";display:block;height:20px;width:20px` |
| `.sliderChart__preset-list div.preset-tab2.default:before` | `background-image:url(../../static/media/icon_audio_profile_default.00547e31.svg)` |
| `.sliderChart__preset-list div.preset-tab2.game:before` | `background-image:url(../../static/media/icon_audio_profile_game.29dccb25.svg)` |
| `.sliderChart__preset-list div.preset-tab2.movie:before` | `background-image:url(../../static/media/icon_audio_profile_movie.ab5bcd10.svg);background-position:100%;background-repeat:no-repeat` |
| `.sliderChart__preset-list div.preset-tab2.music:before` | `background-image:url(../../static/media/icon_audio_profile_music.63287106.svg);background-position:100%;background-repeat:no-repeat` |
| `.sliderChart__preset-list div.preset-tab2.esports:before` | `background-image:url(../../static/media/icon_audio_profile_esports.e9d29253.svg)` |
| `.sliderChart__preset-list div.preset-tab2.micboost:before` | `background-image:url(../../static/media/icon_audio_profile_mic_boost.04a476a1.svg);width:32px` |
| `.sliderChart__preset-list div.preset-tab2.broadcast:before` | `background-image:url(../../static/media/icon_host.bd3ef497.svg)` |
| `.sliderChart__preset-list div.preset-tab2.conference:before` | `background-image:url(../../static/media/icon_followers.01ea0265.svg)` |
| `.sliderChart__preset-list div.preset-tab2.custom:before` | `background-image:url(../../static/media/icon_audio_profile_custom.bf7f9b0f.svg)` |
| `.sliderChart__preset-list div.preset-tab2.default.use-new-default:before` | `background-image:url(../../static/media/icon_audio_profile_new_default.b985e049.svg)` |
| `.sliderChart__preset-list div.preset-tab2.flat:before` | `background-image:url(../../static/media/icon_audio_profile_default.00547e31.svg)` |
| `.sliderChart__yAxisTitle` | `align-items:flex-start;color:#ccc;display:flex;flex:1 1;flex-direction:column-reverse;font-size:12px;height:300px;justify-content:space-between;line-height:12px;width:45px` |
| `.sliderChart__yAxisTitle p` | `margin:0` |
| `.sliderChart__reset-button` | `background:url(../../static/media/eq_reset.e0c3c09c.svg) no-repeat 50%;background-size:20px 20px;height:300px;min-width:20px;opacity:.85;right:40px;transition:background-image .3s;will-change:background-image` |
| `.sliderChart__reset-button:hover` | `background-image:url(../../static/media/eq_reset_hover.186df33c.svg)` |
| `.sliderChart__reset-button:active` | `background-image:url(../../static/media/eq_reset_active.37c570d3.svg)` |
| `.vertical-slider__container--wide` | `padding:0 31px` |
| `.vertical-slider__container--narrow` | `padding:0 16px 0 15px` |
| `.vertical-slider__container--nommoSpecific` | `align-items:center;display:flex;flex-direction:column;padding:0 20.75px 0 20.25px` |
| `.vertical-slider__input` | `position:relative;width:16px` |
| `.vertical-slider__input input` | `-webkit-appearance:none;background-image:radial-gradient(circle at center,#44d62c 1.9%,#204d19 2%);border-radius:2.5px;cursor:pointer;height:6px;left:-296px;position:absolute;top:-2px;transform:rotate(270deg);transform-origin:right top;width:300px` |
| `.vertical-slider__input input:focus` | `outline:none` |
| `.vertical-slider__input input::-webkit-slider-thumb` | `-webkit-appearance:none;background:#44d62c;border:1px solid #44d62c;border-radius:50%;cursor:pointer;height:16px;pointer-events:auto;top:2px;width:16px` |
| `.vertical-slider__input input::-webkit-slider-thumb:hover` | `background:#707070` |
| `.vertical-slider__input input::-webkit-slider-thumb:active` | `background:#383838` |
| `.vertical-slider__input--read-only` | `position:relative;width:16px` |
| `.vertical-slider__input--read-only input` | `-webkit-appearance:none;border-radius:2.5px;border-radius:16px;height:6px;left:-296px;overflow:hidden;position:absolute;top:-2px;transform:rotate(270deg);transform-origin:right top;width:300px` |
| `.vertical-slider__input--read-only input:focus` | `outline:none` |
| `.vertical-slider__input--read-only input::-webkit-slider-runnable-track` | `background:#494949;border-radius:16px;height:6px` |
| `.vertical-slider__input--read-only input::-webkit-slider-thumb` | `-webkit-appearance:none;appearance:none;background-color:#ccc;border-radius:50%;box-shadow:-407px 0 0 400px #ccc;height:6px;top:10px;width:11px` |
| `.vertical-slider__bubble` | `opacity:0;position:absolute;transition:opacity .2s` |
| `.vertical-slider__bubble span` | `background:#44d62c;border-radius:3px;color:#000;display:block;font-family:Roboto,sans-serif;font-size:12px;height:20px;left:-28px;line-height:20px;position:absolute;text-align:center;top:302px;width:26px` |
| `.vertical-slider__title` | `color:#ccc;font-size:12px;margin-top:310px;position:absolute;text-align:center;z-index:10` |
| `.vertical-slider__title span` | `display:inline-block;left:50%;position:absolute;text-align:center;top:50%;transform:translate(-50%)` |
| `.effects-area .slider-label` | `margin-bottom:5px` |
| `.effects-area[data-effect=wave] #waveSlider` | `align-items:flex-end` |
| `.effects-area[data-effect=wave] .slider-wrapper` | `width:315px` |
| `.effects-area[data-effect=wave] .slider-wrapper>div` | `margin-top:10px` |
| `#reactiveSlider,#starSlider` | `display:none;margin-left:10px` |
| `.effects-area[data-effect=reactive] #reactiveSlider,.effects-area[data-effect=starlight] #starSlider` | `display:block` |
| `#starSlider` | `max-width:230px;min-width:230px` |
| `#reactiveSlider .foot.mid,#starSlider .foot.mid` | `opacity:1!important` |
| `.effect-wrapper .effects-area .stepper` | `border:1px solid #5d5d5d;box-sizing:border-box;height:27px;margin-top:10px;position:relative;transition:opacity .2s;width:60px;will-change:opacity` |
| `.effect-wrapper .effects-area .stepper input` | `background-color:#111;border:none;box-sizing:border-box;color:#ccc;font-size:14px;height:25px;line-height:17px;padding:5px 18px 5px 5px;text-align:left;width:58px` |

> 另有 25 条未列出。

---

## 下拉 / 输入（278 条）

| 选择器 | 声明 |
|---|---|
| `::selection` | `background:#44d62c4d` |
| `button:focus,input:focus,select:focus,textarea:focus` | `outline:none` |
| `.windown-dropdown .s3-options` | `width:100%` |
| `.key-config .body .specific-profile-interdevice .dropdown-area` | `width:180px` |
| `.key-config .body .specific-profile-interdevice .s3-dropdown.expand+.s3-options` | `z-index:10` |
| `.key-config .body .specific-profile .dropdown-area` | `width:180px` |
| `.clutch-Y .stage-input` | `margin-right:0` |
| `.toolbar .dropdown-item.short-key` | `display:flex;flex-direction:row` |
| `.toolbar .dropdown-item.short-key>div:first-child` | `flex-grow:1` |
| `.toolbar .dropdown-item.short-key>div:last-child` | `color:#999` |
| `.main-nav .search-wrapper .search-input` | `background:#111;border:1px solid #5d5d5d;color:#ccc;height:20px;line-height:17px;padding-left:27px;text-align:left;width:133px` |
| `.main-nav .search-wrapper .search-input:active,.main-nav .search-wrapper .search-input:hover` | `border-color:#44d62c` |
| `.main-nav .dropdown-block` | `display:-webkit-box;margin-right:5px` |
| `.main-nav .dropdown-block .custom-dropdown` | `display:-webkit-box;line-height:1` |
| `.main-nav .dropdown-block .custom-dropdown .dropdown-area,.main-nav .dropdown-block .custom-dropdown .dropdown-area .s3-options` | `max-width:160px` |
| `.main-nav .dropdown-block .custom-dropdown span` | `font-size:14px;line-height:1;vertical-align:-webkit-baseline-middle` |
| `.main-nav .dropdown-block .custom-dropdown:last-child .dropdown-area` | `margin-right:0` |
| `.s3-dropdown.disabled` | `opacity:.3;pointer-events:none` |
| `.s3-dropdown.disabled:hover` | `border:1px solid #515151;cursor:default` |
| `.s3-dropdown` | `background-color:#0000;border:1px solid #515151;color:#ccc;font-size:14px;height:27px;line-height:17px;max-height:27px;padding:4px 5px;position:relative;text-transform:none;transition:opacity .3s,border .3s;width:100%;will-change:opacity,border;z-index:1` |
| `.s3-dropdown>.label-prefix` | `align-items:center;display:flex;height:25px;line-height:30px;position:absolute;right:23px;top:0;width:auto` |
| `.s3-dropdown>.icon.expand` | `background-image:url(../../static/media/icon_expand.55a47b0c.svg);background-position:50%;background-repeat:no-repeat;background-size:10px;height:25px;position:absolute;right:0;top:0;transform:rotate(0deg);transition:transform .3s;width:29px;will-change:transform` |
| `.s3-dropdown.expand` | `border:1px solid #44d62c` |
| `.s3-dropdown.expand>.icon.expand` | `transform:rotate(180deg)` |
| `.s3-dropdown.surrounded-hover,.s3-dropdown:hover` | `border:1px solid #44d62c` |
| `.s3-dropdown.placeholder` | `color:#666` |
| `.s3-dropdown .selected` | `max-width:calc(100% - 24px);overflow:hidden;position:absolute;text-overflow:ellipsis;white-space:nowrap` |
| `.s3-dropdown .selected-has-prefix` | `max-width:calc(100% - 40px)` |
| `.s3-options .option.selected,.s3-options .option:active` | `color:#44d62c` |
| `.s3-dropdown .raw-text,.s3-options .raw-text` | `white-space:pre` |
| `.s3-dropdown .raw-text .img-wrap,.s3-options .raw-text .img-wrap` | `align-items:center;display:flex` |
| `.s3-dropdown .raw-text .img-wrap img,.s3-options .raw-text .img-wrap img` | `margin-right:5px` |
| `.s3-dropdown .raw-text .img-wrap span,.s3-options .raw-text .img-wrap span` | `overflow-x:hidden;text-overflow:ellipsis` |
| `.show>.dropdown-razer-2` | `opacity:1;pointer-events:auto;transform:translateY(0)` |
| `.dropdown-razer-2` | `background-color:#000;border:1px solid #5d5d5d;border-radius:5px;cursor:default;list-style:none;margin:2px 0 0;min-width:140px;opacity:0;padding:7px 14px;pointer-events:none;position:absolute;right:0;top:100%;transform:translateY(-7px);transition:opacity .2s linear,transform .2s ease-out;-webkit-user-select:none;user-select:none;z-index:1` |
| `.dropdown-razer-2.block` | `min-width:100%` |
| `.dropdown-razer-2.select` | `border-top:none;margin-top:0;padding:0` |
| `.dropdown-razer-2 .dropdown-item,.dropdown-razer-2 .dropdown-item-2` | `background-color:#0000;border:none;cursor:default;display:block;margin:0;outline:none;text-align:left;text-transform:capitalize;white-space:nowrap;width:100%` |
| `.dropdown-razer-2 .dropdown-item` | `border-radius:13px;color:#ccc;font-size:14px;line-height:17px;margin-bottom:4px;padding:5px 18px 4px` |
| `.dropdown-razer-2 .dropdown-item:last-child` | `margin-bottom:0` |
| `.dropdown-razer-2 .dropdown-item.disabled-2` | `color:#44d62c;pointer-events:none;text-transform:none` |
| `.dropdown-razer-2 .dropdown-item-2` | `border-bottom:1px solid #555;font-size:16px;margin-bottom:14px;padding:10px 0` |
| `.dropdown-razer-2 .dropdown-item:hover` | `background-color:#1f1f1f` |
| `.dropdown-razer-2 .dropdown-item-2.active,.dropdown-razer-2 .dropdown-item.active` | `color:#44d62c` |
| `.dropdown-razer-2 .dropdown-item-2.disabled,.dropdown-razer-2 .dropdown-item.disabled` | `background-color:#0000;opacity:.3;pointer-events:none` |
| `.dropdown-razer-2 .dropdown-divider` | `background-color:#5d5d5d;height:1px;margin:7px 0;width:100%` |
| `.dropdown-area` | `display:block;margin:0 10px;position:relative;text-align:left;width:230px;z-index:102` |
| `.dropdown-profile` | `align-items:center;display:flex;gap:10px;position:relative;z-index:103` |
| `.dropdown-area.above-tutorial` | `left:510px;position:absolute;top:112px` |
| `.s3-dropdown-with-dual-item.disabled` | `opacity:.3;pointer-events:none` |
| `.s3-dropdown-with-dual-item.disabled:hover` | `border:1px solid #515151;cursor:default` |
| `.s3-dropdown-with-dual-item` | `background-color:#0000;border:1px solid #515151;color:#ccc;font-size:14px;height:27px;line-height:17px;max-height:27px;padding:4px 5px;position:relative;text-transform:none;transition:opacity .3s,border .3s;width:100%;will-change:opacity,border;z-index:107` |
| `.s3-dropdown-with-dual-item>.icon.expand` | `background-image:url(../../static/media/icon_expand.55a47b0c.svg);background-position:50%;background-repeat:no-repeat;background-size:10px;height:25px;position:absolute;right:0;top:0;transform:rotate(0deg);transition:transform .3s;width:29px;will-change:transform` |
| `.s3-dropdown-with-dual-item.expand` | `border:1px solid #44d62c` |
| `.s3-dropdown-with-dual-item.expand>.icon.expand` | `transform:rotate(180deg)` |
| `.icon.warning-select` | `background-image:url(../../static/media/warning.ad3f47f8.svg);background-repeat:no-repeat;height:26px;width:26px` |
| `.s3-dropdown-with-dual-item:hover` | `border:1px solid #44d62c` |
| `.s3-dropdown-with-dual-item.placeholder` | `color:#666` |
| `.s3-dropdown-with-dual-item .option.selected-mapping,.s3-dropdown-with-dual-item .selected` | `max-width:calc(100% - 24px);overflow:hidden;position:absolute;text-overflow:ellipsis;white-space:nowrap` |
| `.s3-options-with-dual-item .option.selected,.s3-options-with-dual-item .option:active` | `color:#44d62c` |
| `.s3-options-with-dual-item .options.selected-mapping` | `background-color:hsla(0,0%,7%,.698);border:1px solid #44d62c;color:#ccc` |
| `.s3-dropdown-with-dual-item .raw-text,.s3-options-with-dual-item .raw-text` | `white-space:pre` |
| `.s3-dropdown-with-dual-item .raw-text .img-wrap span,.s3-options-with-dual-item .raw-text .img-wrap span` | `overflow-x:hidden;text-overflow:ellipsis` |
| `.s3-options-with-dual-item .custom-shortcut .display_input` | `padding:5px` |
| `.s3-options-with-dual-item .custom-shortcut .display_input .shortcut-mapping` | `margin-bottom:0;width:200px` |
| `.profile-tips .profile-dynamic-tips,.profile-tips .profile-selected-tips` | `align-items:center;background-color:#111;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px 0 #0003;box-shadow:0 6px 10px #0003;display:flex!important;flex-direction:column;left:10px;opacity:0;padding:20px;position:absolute;text-transform:none;transition:visibility 0s,opacity .3s linear;visibility:hidden;will-change:visibility,opacity;z-index` |
| `.profile-tips .profile-selected-tips` | `height:-webkit-fit-content;height:fit-content;top:40px;width:300px` |
| `.profile-tips .profile-selected-tips.enable-tip` | `opacity:1;visibility:visible` |
| `.obm-menu:not(.only-swap) .dropdown-area .s3-options>div:first-child` | `color:#cccccc4d` |
| `.obm[class*=selected-].raw-text` | `color:green` |
| `.obm[class*=selected-] .selected-color-icon` | `border-radius:50%;height:9px;position:absolute;right:1px;top:1px;width:9px` |
| `.obm[class*=selected-].selected-red .selected-color-icon` | `background-color:red` |
| `.obm[class*=selected-].selected-lightgreen .selected-color-icon` | `background-color:#44d62c` |
| `.obm[class*=selected-].selected-green .selected-color-icon` | `background-color:green` |
| `.obm[class*=selected-].selected-blue .selected-color-icon` | `background-color:blue` |
| `.obm[class*=selected-].selected-cyan .selected-color-icon` | `background-color:aqua` |
| `.obm[class*=selected-].selected-yellow .selected-color-icon` | `background-color:#ff0` |
| `.profile-bar .dropdown-area .tipIcon` | `background-color:#4a4a4a;background-image:url(../../static/media/tooltip_exclamationmark.cc8fb226.svg);background-repeat:no-repeat;border-radius:6px;display:inline-block;height:12px;margin:2px 5px 0;transition:background-color .3s;width:12px;will-change:background-color` |
| `.profile-bar .dropdown-area .tipIcon:hover` | `background-color:#ffffff4d` |
| `input[type=checkbox]` | `display:none` |
| `.check-box.checked,input[type=checkbox]:checked+.check-box` | `background-color:#44d62c;border-color:#44d62c` |
| `.check-box.checked:after,input[type=checkbox]:checked+.check-box:after` | `animation:tickbottom .1s ease 0s forwards;height:9.6px` |
| `.check-box.checked:before,input[type=checkbox]:checked+.check-box:before` | `animation:ticktop .2s ease 0s forwards;height:15.4px` |
| `[type=radio]:checked,[type=radio]:not(:checked)` | `left:-9999px;position:fixed` |
| `[type=radio]:checked+label,[type=radio]:not(:checked)+label` | `color:#ccc;display:inline-block;font-size:14px;line-height:20px;padding-left:30px;position:relative` |
| `[type=radio]:checked+label:before,[type=radio]:not(:checked)+label:before` | `background:#0000;border:1px solid #737373;border-radius:100%;box-sizing:border-box;content:"";height:20px;left:0;position:absolute;top:0;width:20px` |
| `[type=radio]:checked+label:after,[type=radio]:not(:checked)+label:after` | `background:#44d62c;border-radius:100%;content:"";height:10px;left:5px;position:absolute;top:5px;transition:all .2s ease;width:10px` |
| `[type=radio]:not(:checked)+label:after` | `opacity:0;transform:scale(0)` |
| `[type=radio]:checked+label:after` | `opacity:1;transform:scale(1)` |
| `.radio-item` | `display:inline-block;margin:0 20px` |
| `.vertical-list .radio-item` | `display:block;margin:0 0 10px` |
| `.show>.dropdown-razer-2.dropdown-offline` | `transform:translateY(0)` |
| `.dropdown-razer-2.dropdown-offline` | `margin:10px 10px 0 0` |
| `.dropdown-razer-2.dropdown-offline,.dropdown-razer-2.dropdown-warning` | `background-color:#111;border-color:#fd8611;border-radius:3px;padding:20px;text-align:center;transform:translateY(-7px);transition:opacity .2s linear,transform .2s ease-out;width:300px;z-index:105` |
| `.dropdown-razer-2.dropdown-warning` | `margin:10px -270px 10px 10px` |
| `.dropdown-razer-2.dropdown-info` | `background-color:#111;border-color:#fd8611;border-radius:3px;margin:15px -290px 10px 10px;padding:20px;text-align:center;transform:translateY(-7px);transition:opacity .2s linear,transform .2s ease-out;width:300px;z-index:105` |
| `.dropdown-razer-2.dropdown-info,.dropdown-razer-2.dropdown-offline .dropdown-title,.dropdown-razer-2.dropdown-warning .dropdown-title` | `color:#ccc;font-size:16px;text-transform:uppercase` |
| `.dropdown-razer-2.dropdown-info,.dropdown-razer-2.dropdown-offline .dropdown-description,.dropdown-razer-2.dropdown-warning .dropdown-descri` | `color:#999;font-size:14px` |
| `.show>.dropdown-razer-2.dropdown-update` | `transform:translateY(0)` |
| `.dropdown-razer-2.dropdown-update` | `background-color:#222;box-shadow:0 0 20px 0 #000;font-size:14px;margin:11px 11px 0 0;padding:0;transform:translateY(-7px);transition:opacity .2s linear,transform .2s ease-out;width:268px;z-index:105` |
| `.dropdown-razer-2.dropdown-update .dropdown-header` | `border-bottom:1px solid #5d5d5d;color:#999;padding:10px 10px 9px;text-align:center;text-transform:uppercase` |
| `.dropdown-razer-2.dropdown-update .dropdown-body` | `padding:20px 30px` |
| `.dropdown-razer-2.dropdown-update .read-more` | `cursor:default;display:inline-block;margin-top:10px;text-decoration:underline` |
| `.dropdown-razer-2.dropdown-update .progress-text` | `color:#707070;margin-bottom:8px;margin-top:20px` |
| `.drawer-top .dropdown-area` | `margin-left:0;max-width:180px` |
| `.key-config .dropdown-area .s3-options.unsetZ` | `max-width:100%;z-index:auto` |
| `.search-bar .search-wrapper .search-input` | `background:#111;border:1px;border-radius:5px;color:#ccc;height:32px;line-height:17px;padding-left:47px;width:241px` |
| `.search-bar .search-wrapper .search-input:active` | `align-items:center;border-color:#44d62c;display:inline-flex;height:32px;width:241px` |
| `.key-config .body .dropdown-area` | `margin:0 0 10px;width:210px` |
| `.key-config.macro .body .dropdown-area` | `position:absolute` |
| `.key-config.macro .body>.dropdown-playback` | `min-height:27px` |
| `.key-config.macro .body>.dropdown-playback .s3-options` | `width:100%` |
| `.key-config .body .radio-item` | `margin-bottom:10px;margin-left:0` |
| `.powerSaving-select` | `display:flex;gap:10px;margin-top:20px` |
| `.radio-item[tooltip]:before` | `display:none` |
| `.radio-commanddial[tooltip]:before` | `display:block;margin-top:40px;right:calc(100% - 80px);top:auto` |
| `.device_linked_game .device-tile .dropdown-area` | `margin:0 auto;width:200px` |
| `.block-input` | `align-items:center;display:flex` |
| `.block-input--container` | `margin-bottom:10px` |
| `.block-input--container.disabled` | `opacity:.2` |

> 另有 158 条未列出。

---

## 提示 / 浮层（385 条）

| 选择器 | 声明 |
|---|---|
| `.img-text .multipairing` | `background-image:url(../../static/media/icon-multideviceparing.9f560b69.svg);background-position:50%;background-repeat:no-repeat;height:44px;max-width:44px;min-width:44px` |
| `.img-text .multipairing.productivity` | `background-image:url(../../static/media/icon_productivity_multi_device_pairing.a7a2c84d.svg)` |
| `.img-text .multipairing.hyperpolling` | `background-image:url(../../static/media/icon-multidevicepairing2.af8657a6.svg)` |
| `.main-nav li .tooltip` | `display:none` |
| `.main-nav li:hover .tooltip` | `background-color:#000;border:1px solid #5d5d5d;display:block;font-size:14px;margin-left:15px;margin-top:30px;padding:8px 10px;position:absolute;width:-webkit-max-content;width:max-content;z-index:1` |
| `[tooltip]:before` | `background-color:#000;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;content:attr(tooltip);display:block;font-size:14px;height:auto;line-height:16px;opacity:0;padding:8px 10px;pointer-events:none;position:absolute;right:0;text-align:left;top:calc(100% + 5px);transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:nowrap;width:auto;will-change:visibility,opacity;z-index` |
| `[tooltip]:hover:before` | `opacity:1;visibility:visible` |
| `[data-tooltip]:before` | `background-color:#000;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;content:attr(data-tooltip);display:block;font-size:14px;height:auto;line-height:16px;opacity:0;padding:8px 10px;pointer-events:none;position:relative;text-align:left;top:60px;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:300px;will-change:visibility,opacity` |
| `[data-tooltip]:hover:before` | `opacity:1;visibility:visible` |
| `[indicator-tooltip]:before` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;content:attr(indicator-tooltip);font-size:14px;left:100px;line-height:16px;opacity:0;padding:8px 10px;position:absolute;text-align:left;top:100px;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:290px;will-change:visibility,opacity;z-index:106` |
| `[indicator-tooltip]:hover:before` | `opacity:1;visibility:visible` |
| `.tooltip-razer` | `height:100%;left:0;position:absolute;top:0;width:100%;z-index:1060` |
| `.tooltip-razer.bottom-left-edge>.main,.tooltip-razer.bottom-left>.main,.tooltip-razer.bottom-right-edge>.main,.tooltip-razer.bottom-right>.m` | `bottom:auto;margin-bottom:0;margin-top:5px;top:100%` |
| `.tooltip-razer.bottom-right-edge>.main,.tooltip-razer.bottom-right>.main,.tooltip-razer.top-right>.main` | `justify-content:flex-start;left:0;margin-left:0` |
| `.tooltip-razer.top>.main` | `bottom:100%;left:50%;margin-bottom:5px;margin-left:-150px;margin-top:0;top:auto` |
| `.tooltip-razer.bottom-left-edge>.main,.tooltip-razer.bottom-left>.main,.tooltip-razer.top-left>.main` | `justify-content:flex-end;left:auto;margin-left:0;right:0` |
| `.tooltip-razer>.main` | `bottom:100%;display:flex;justify-content:center;left:50%;margin-bottom:5px;margin-left:-150px;opacity:0;pointer-events:none;position:absolute;transition:opacity .1s linear;width:300px` |
| `.tooltip-razer.show>.main` | `opacity:1` |
| `.tooltip-razer>.main>.wrapper` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;display:inline-block;font-family:Roboto;font-size:14px;line-height:16px;padding:8px 10px;text-align:left;text-transform:none` |
| `.tooltip-razer .title` | `text-transform:uppercase` |
| `.tooltip-razer .title>span` | `color:#44d62c;margin-left:10px;text-transform:none` |
| `.tooltip-razer .title+*` | `margin-top:10px` |
| `.tooltip-razer.text-transform-capitalize .wrapper` | `text-transform:capitalize` |
| `.drop-tips` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;display:block;font-size:14px;height:auto;line-height:16px;opacity:0;padding:8px 10px;position:absolute;text-align:left;transition:top .1s,left .1s,visibility .2s,opacity .3s linear;visibility:hidden;width:300px;will-change:top,left,visibility,opacity;z-index:200` |
| `.drop-tips.on` | `height:auto!important;opacity:1;visibility:visible;white-space:pre-line;z-index:9999` |
| `.battery-tooltip` | `display:flex;gap:20px;height:100%` |
| `.battery-tooltip .battery-value` | `margin-top:5px` |
| `.battery-tooltip div` | `align-items:center;gap:5px;justify-content:center;text-align:center` |
| `.battery-tooltip .batt` | `background-position:50%;background-repeat:no-repeat;background-size:20px;height:20px;width:20px` |
| `.profile-tips .profile-dynamic-tips` | `color:#ccc;font-size:14px;height:auto;left:50%;padding:20px;top:40px;transform:translate(-50%);width:300px` |
| `.profile-tips:hover .profile-dynamic-tips` | `opacity:1;visibility:visible` |
| `.obm-slot.white .tip` | `left:calc(100% - 30px);top:70px` |
| `.obm-menu .tip` | `text-transform:none` |
| `.obm-menu .tooltip_parent` | `display:inline-block` |
| `.obm-menu .tooltip_parent .help` | `vertical-align:text-top` |
| `.obm-menu .tooltip_parent .help:hover` | `background-color:#6c6c6c` |
| `.obm-menu .keymap-head .help:hover+.tip` | `left:75%;right:auto;width:100%` |
| `.body-widget-tip-portal` | `opacity:1;position:fixed;right:auto;visibility:visible;z-index:10001` |
| `div.flex>div.wrapper-tooltip-widget` | `flex:none` |
| `.wrapper-tooltip-widget` | `position:relative` |
| `.wrapper-tooltip-widget .overlay` | `bottom:10px;left:10px;position:absolute;right:10px;top:10px;z-index:2` |
| `.wrapper-tooltip-widget .showTooltip` | `background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;width:300px;z-index:3` |
| `.wrapper-tooltip-widget .showTooltip p` | `font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0` |
| `.wrapper-tooltip-widget .hideTooltip` | `position:fixed;visibility:hidden` |
| `.warning-fw-update-tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-family:Roboto;font-size:14px;line-height:16px;max-width:300px;padding:8px 10px;position:fixed;transition:visibility 0s,opacity .3s linear;will-change:visibility,opacity;z-index:100` |
| `.config-row.custom .tip` | `margin-top:5px;width:300px` |
| `.config-row .tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;line-height:18px;min-width:300px;opacity:0;padding:8px 10px;position:absolute;text-align:left;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:20%;will-change:visibility,opacity;z-index:99` |
| `.config-row .tip.hypershift-mode-tip` | `max-width:300px;top:19px` |
| `.config-row .help:hover+.tip` | `opacity:1;visibility:visible;z-index:100` |
| `.config-row .help:hover+.tip.hypershift-mode-tip  [@media screen and (max-width:1279px)]` | `left:auto;right:1vw;top:26px` |
| `.tooltip-panel-button` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;left:61px;line-height:16px;max-width:358px;opacity:0;padding:8px 10px;position:absolute;text-align:left;visibility:hidden;word-break:break-word;z-index:999` |
| `.tooltip-panel-button.show` | `opacity:1;visibility:visible` |
| `.keymap-head .keymap-tip` | `background-color:#000;border:1px solid #5d5d5d;display:none;height:auto;left:calc(50% - 105px);max-width:-webkit-max-content;max-width:max-content;padding:10px;position:absolute;top:35px;z-index:1` |
| `.search-bar .search .tooltip` | `display:none;overflow:hidden` |
| `.search-bar .search:hover .tooltip` | `background-color:#000;border:1px solid #5d5d5d;display:block;font-size:14px;margin-left:15px;margin-top:30px;padding:8px 10px;position:absolute;width:-webkit-max-content;width:max-content;z-index:106` |
| `.keymap-head:hover .keymap-tip` | `display:block` |
| `.key-tip` | `background-color:#000;border:1px solid #5d5d5d;max-width:300px;min-height:75px;opacity:0;padding:8px 10px;position:absolute;text-align:center;transition:opacity .3s,visibility 0s,left 0s,top 0s;visibility:hidden;will-change:opacity,visibility,left,top;z-index:102` |
| `.key-tip.show` | `opacity:1;visibility:visible;z-index:999` |
| `.key-tip .index` | `color:#707070;font-size:14px;margin-bottom:6px` |
| `.key-tip .index,.key-tip .type` | `line-height:14px;text-transform:uppercase` |
| `.key-tip .type` | `color:#ccc;font-size:12px` |
| `.key-tip .name` | `color:#ccc;font-size:18px;overflow:hidden!important;overflow-wrap:break-word;text-overflow:ellipsis;text-transform:capitalize;white-space:nowrap` |
| `.require-synapse .tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;line-height:16px;opacity:0;padding:8px 10px;position:absolute;right:-227px;text-align:left;top:34px;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:250px;will-change:visibility,opacity;z-index:99` |
| `.require-synapse .help:hover+.tip` | `opacity:1;visibility:visible;z-index:100` |
| `.customize .key-tip` | `padding:initial` |
| `.status-chroma-app .manage .tip` | `min-width:165px;width:-webkit-fit-content;width:fit-content` |
| `.status-chroma-app .tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;line-height:16px;opacity:0;padding:8px 10px;position:absolute;right:15px;text-align:left;top:15px;transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:pre-wrap;width:410px;will-change:visibility,opacity;z-index:99` |
| `.status-chroma-app .help:hover>.tip` | `opacity:1;visibility:visible;z-index:100` |
| `.config-row .tip-config` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;line-height:18px;max-width:-webkit-max-content;max-width:max-content;opacity:0;padding:8px 10px;position:absolute;text-align:left;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:50%;will-change:visibility,opacity;z-index:99` |
| `.config-row .help-config:hover+.tip-config` | `opacity:1;visibility:visible;z-index:100` |
| `.knob-item .icon-info .tip` | `align-items:flex-start;background-color:#111;border:1px solid #5d5d5d;color:#ccc;display:flex;flex-direction:column;font-size:14px;justify-content:space-around;left:10px;line-height:17px;min-height:92px;min-width:132px;opacity:0;position:absolute;right:auto;top:30px;transition:visibility 0s,opacity .3s linear;visibility:hidden;will-change:visibility,opacity` |
| `.knob-item .icon-info .tip .des-item` | `align-items:center;display:flex;height:17px;margin-right:10px` |
| `.knob-item .icon-info .tip .des-1:before,.knob-item .icon-info .tip .des-2:before,.knob-item .icon-info .tip .des-3:before` | `background-color:#0000;content:" ";display:block;height:17px;margin:8px;width:20px` |
| `.knob-item .icon-info .tip .des-1:before` | `background-image:url(../../static/media/clockwise.eca45a25.svg);background-position:50%;background-repeat:no-repeat` |
| `.knob-item .icon-info .tip .des-2:before` | `background-image:url(../../static/media/counter-clockwise.93ff0031.svg);background-position:50%;background-repeat:no-repeat` |
| `.knob-item .icon-info .tip .des-3:before` | `background-image:url(../../static/media/dial-mapping.36cf0d1f.svg);background-position:50%;background-repeat:no-repeat` |
| `.knob-item .icon-info:hover>.tip` | `opacity:1;visibility:visible;z-index:100` |
| `.access-mode .access-tip .help` | `background-color:#4a4a4a;background-image:url(../../static/media/tooltip_questionmark.96138d2f.svg);background-repeat:no-repeat;border-radius:7px;height:14px;margin-left:10px;position:relative;transition:background-color .3s;width:14px;will-change:background-color` |
| `.access-mode .tip` | `align-items:center;background-color:#000;border:1px solid #5d5d5d;color:#ccc;display:flex;flex-direction:column;font-size:14px;height:152px;justify-content:space-around;line-height:16px;opacity:0;padding:8px 10px;position:absolute;text-align:left;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:300px;will-change:visibility,opacity;z-index:99` |
| `.access-mode .help:hover+.tip` | `opacity:1;visibility:visible;z-index:100` |
| `.toolbar [tooltip]:before` | `text-transform:capitalize` |
| `.toolbar .arrow:hover .tooltip` | `display:block` |
| `.toolbar .right>div:hover .tooltip` | `display:block;opacity:1;visibility:visible` |
| `.toolbar .right>div .tooltip` | `background-color:#000;border:1px solid #5d5d5d;display:none;font-size:14px;left:15px;opacity:0;padding:0 8px;position:absolute;top:40px;visibility:hidden;width:-webkit-max-content;width:max-content;z-index:1` |
| `.toolbar .right>div .tooltip p` | `text-transform:capitalize` |
| `.toolbar .navigation .arrow.back[tooltip]:before` | `left:5px` |
| `.app-explorer .app-explorer-popup` | `background:#000;border:1px solid #5d5d5d;height:0;left:-365px;opacity:0;position:absolute;top:40px;transition:height 0s,visibility 0s,opacity .1s linear;visibility:hidden;width:410px;will-change:height,visibility,opacity;z-index:3` |
| `.app-explorer .app-explorer-popup.show` | `height:auto;max-height:calc(100vh - 80px);opacity:1;overflow-x:auto;visibility:visible` |
| `.reset-oled-download .download-popup` | `background:#111;border:1px solid #5d5d5d;height:0;left:-255px;opacity:0;padding:20px;position:absolute;top:40px;transition:height 0s,visibility 0s,opacity .1s linear;visibility:hidden;width:300px;will-change:height,visibility,opacity;z-index:3` |
| `.reset-oled-download .download-popup.show` | `height:auto;opacity:1;visibility:visible` |
| `.reset-oled-download .download-popup .download-popup-header` | `text-transform:uppercase` |
| `.reset-oled-download .download-popup .separator` | `background-color:#5d5d5d;border:none;height:1px;margin:20px 0` |
| `.reset-oled-download .download-popup .download-popup-content` | `font-size:14px` |
| `.reset-oled-download .download-popup .download-popup-progress` | `align-items:center;display:flex;font-size:14px;justify-content:space-between` |
| `.reset-oled-download .download-popup .download-progress-text` | `color:#ccc;font-size:13px;margin-top:10px;text-align:right` |
| `.reset-oled-download .download-popup .progress-bar-header` | `display:flex;justify-content:space-between` |
| `.device_linked_game .popup-widget .game-detail .choose-a-mat` | `max-height:100vh!important` |
| `.device_linked_game .popup-widget .game-detail .choose-a-mat .scrollable` | `overflow-y:visible!important` |
| `.device_linked_game .popup-widget .game-detail .choose-a-mat .custom-cover-art .refresh-cover-art` | `padding-top:60px` |
| `.device_linked_game .popup-widget .game-detail .choose-a-mat .custom-cover-art .tooltip` | `background-color:#000;border:1px solid #5d5d5d;font-size:14px;margin-left:30px;padding:8px 10px;width:-webkit-max-content;width:max-content` |
| `.game-location .tooltip` | `background-color:#000;border:1px solid #5d5d5d;font-size:14px;left:50%;opacity:0;padding:8px 10px;position:absolute;text-align:left;top:70%;visibility:hidden;width:-webkit-max-content;width:max-content;z-index:1` |
| `.game-location:hover .tooltip` | `opacity:1;visibility:visible` |
| `.device_linked_game .popup-widget .game-detail .choose-a-mat  [@media only screen and (max-width:1284px)]` | `max-width:800px` |
| `.device_linked_game .popup-widget .game-detail .choose-a-mat .scrollable  [@media only screen and (max-width:1284px)]` | `overflow-y:visible!important` |
| `.dashboard .box-item .batt .tips-container` | `position:relative` |
| `.dashboard .box-item .batt .tips-container .tips` | `background-color:#000;border:1px solid #5d5d5d;box-shadow:none;color:#ccc;display:block;font-size:14px;height:auto;left:10px;line-height:16px;opacity:0;outline:none;padding:8px 10px;position:absolute;text-align:left;top:14px;transition:top .1s,left .1s,visibility .2s,opacity .3s linear;visibility:hidden;width:-webkit-max-content;width:max-content;will-change:top,left,visibility,opacity;z-index:200` |
| `.dashboard .box-item .batt:hover .tips` | `opacity:1;visibility:visible` |
| `.app-row .app-tip` | `background-color:#000;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;display:block;font-size:12px;height:auto;left:50px;line-height:16px;max-width:270px;opacity:0;padding:8px 10px;pointer-events:none;position:absolute;text-align:left;top:calc(100% - 5px);transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:pre-wrap;width:auto;will-change:visibility,opacity;z-index:1` |
| `.app-row .app-tip.show` | `opacity:1;visibility:visible` |
| `.power-saving-effect .content .tooltip_parent` | `position:relative` |
| `.power-saving-effect .content .tooltip_parent .help` | `position:static!important` |
| `.power-saving-effect .content .tooltip_parent:hover` | `opacity:1;visibility:visible;z-index:100` |
| `.power-saving-effect .content .tooltip_parent .tip` | `left:-197px;white-space:normal;width:305px` |
| `.warning-backdrop .popup` | `background-color:#111;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px 0 #000;color:#ccc;font-size:14px;line-height:17px;padding:20px;text-align:left;text-align:center;text-transform:none;width:400px` |
| `.chroma-sync-text .popup` | `background-color:#000;border:1px solid #5d5d5d;display:flex;flex-direction:column;left:36px;min-width:180px;position:absolute;top:100%;z-index:10` |
| `.chroma-sync-text .popup div` | `align-items:center;color:#ccc;display:flex;flex:1 1;font-size:14px;padding:5px 6px` |
| `.chroma-sync-text .popup div:hover` | `background-color:hsla(0,0%,100%,.102)` |
| `.stage .xy-tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;display:none;font-size:14px;left:20px;line-height:16px;opacity:0;padding:8px 10px;position:absolute;text-align:left;top:35px;visibility:hidden;white-space:nowrap;will-change:visibility,opacity;z-index:99` |
| `.icon-sensitivity-xy:hover .xy-tip` | `display:block;opacity:1;visibility:visible` |
| `.drag-mapping-tip` | `background:#44d62c33;border-radius:2px;cursor:grabbing!important;height:40px;position:fixed;transform:translate(10px,10px);width:40px;z-index:100` |

> 另有 265 条未列出。

---

## 弹窗 / 遮罩（211 条）

| 选择器 | 声明 |
|---|---|
| `.backdrop` | `background-color:#00000080;height:150%;left:0;opacity:0;position:absolute;top:0;transition:visibility 0s,opacity .1s linear;visibility:hidden;width:100%;will-change:visibility,opacity` |
| `.backdrop,.backdrop.glow` | `background-position:bottom;background-repeat:no-repeat` |
| `.backdrop.glow` | `background-image:url(../../static/media/background_glow.43f70cb2.svg)` |
| `.backdrop.show` | `opacity:1;visibility:visible;z-index:106` |
| `.backdrop.show .choose-a-mat` | `top:100px` |
| `.factory-default .warning-alert` | `align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:465px;z-index:1` |
| `.factory-default .warning-alert .title` | `color:#ccc;margin-bottom:10px` |
| `.key-config .body textarea,.save-alert` | `background-color:#111;color:#ccc;font-size:14px;line-height:17px` |
| `.save-alert` | `border:1px solid #44d62c;border-radius:5px;left:50%;padding:20px 30px;position:fixed;text-align:center;top:50%;transform:translateX(-50%) translateY(-100%);width:400px` |
| `.save-alert.large` | `align-items:center;background:#222;border:none;display:flex;flex-direction:column;flex-wrap:wrap;height:572px;padding:0;top:75%;width:850px` |
| `.save-alert .link` | `border-bottom:1px solid #5d5d5d;height:16px;margin-bottom:45px;width:220px` |
| `.save-alert .img` | `align-content:center;display:flex;font-size:large;height:120px;justify-content:center;margin-bottom:35px;width:100%` |
| `.save-alert .arrow` | `align-self:center;background-image:url(../../static/media/icon_arrow_right_thin.bef8ca32.svg);background-position:50%;background-repeat:no-repeat;height:20px;width:56px` |
| `.save-alert .hr-line` | `border-bottom:1px solid #ccc;margin-bottom:20px;width:100%` |
| `.save-alert .content` | `color:#ccc;flex:none;flex-grow:0;font-size:14px;font-style:normal;font-weight:400;height:34px;line-height:17px;margin-bottom:35px;order:0;text-align:center;width:520px;z-index:0` |
| `.save-alert.large .title` | `align-items:center;display:flex;font-weight:400;height:40px;justify-content:center;line-height:16px;margin-bottom:0;width:100%` |
| `.save-alert .title,.save-alert.large .title` | `color:#44d62c;font-family:RazerF5;font-size:16px;line-height:19px;text-transform:uppercase` |
| `.save-alert .title` | `margin-bottom:20px;text-align:center` |
| `.save-alert .close` | `background-color:#0000;background-image:url(../../static/media/icon_close.55fe41f1.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;height:20px;position:absolute;right:8px;top:8px;width:20px` |
| `.remove-alert,.warning-alert` | `background-color:#111;border:1px solid #fd8611;border-radius:5px;color:#ccc;font-size:14px;left:50%;line-height:17px;padding:20px 30px;position:fixed;top:50%;transform:translateX(-50%) translateY(-100%);width:400px` |
| `.remove-alert .title,.warning-alert .title` | `align-items:center;color:#fd8611;display:flex;font-family:Roboto;font-size:16px;justify-content:center;line-height:16.8px;margin-bottom:20px;text-align:center;text-transform:uppercase` |
| `.remove-alert .title .icon,.warning-alert .title .icon` | `background-image:url(../../static/media/warning.ad3f47f8.svg);background-position:50%;background-repeat:no-repeat;height:25px;margin-right:10px;width:25px` |
| `.remove-alert .body,.warning-alert .body` | `color:#ccc;font-family:Roboto;font-size:14px;line-height:16.8px;text-align:center` |
| `.remove-alert .action,.warning-alert .action` | `display:flex;gap:20px;justify-content:center` |
| `.remove-alert .action .enable-button,.warning-alert .action .enable-button` | `align-items:center;background-color:#44d62c;border-radius:3px;color:#212121;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px` |
| `.backdrop` | `height:100%!important` |
| `.s3-modal` | `align-items:center;bottom:0;display:flex;height:auto;justify-content:center;left:0;position:fixed;right:0;top:0;z-index:1000` |
| `.s3-modal--content` | `width:-webkit-fit-content;width:fit-content` |
| `.s3-modal--header` | `height:auto;height:30px;position:relative;text-align:center` |
| `.s3-modal--backdrop` | `background-color:#0000004d;bottom:0;left:0;position:fixed;right:0;top:0;z-index:9998` |
| `.dashboard .box-group .title:hover .drag-icon:before,.dashboard .box-group .title:hover~.backdrop-box,.dashboard .box-group:hover .title .dr` | `opacity:1;visibility:visible` |
| `.dashboard .box-group:not(.expand) .backdrop-box` | `transition:all .1s ease-in-out 0s` |
| `.dashboard .box-group.dragging .backdrop-box` | `border:2px solid #44d62c;border-radius:5px;opacity:1;transition:all .1s ease-in-out 0s;visibility:visible` |
| `.dashboard .box-group .backdrop-box` | `background-color:#333;border-radius:5px;bottom:10px;left:0;max-height:40px;opacity:0;position:absolute;right:0;top:0;transition:visibility .3s ease-in-out 0s,opacity .3s ease-in-out 0s,max-height .3s ease-in-out .3s;visibility:hidden;z-index:0` |
| `.dashboard .box-group.expand .backdrop-box` | `bottom:0;max-height:2000px` |
| `.warning-backdrop` | `align-items:center;background:#000000b3;display:flex;height:100%;justify-content:center;left:0;position:absolute;top:86px;width:100%;z-index:999` |
| `.warning-backdrop .warning:before` | `background-image:url(../../static/media/warning.ad3f47f8.svg);content:"";display:inline-block;height:20px;margin:0 50%;position:relative;transform:translateX(-50%);width:20px` |
| `.warning-backdrop p` | `margin:10px 0;text-align:left` |
| `.warning-backdrop a` | `color:#ccc` |
| `.warning-backdrop.iframe-chroma-app` | `top:0` |
| `.analog-tutorial-modal` | `background:#111;border:1px solid #44d62c;border-radius:5px;display:flex;margin-top:134px;position:relative` |
| `.analog-tutorial-modal-wrapper` | `align-items:flex-start;display:flex;justify-content:center` |
| `.analog-tutorial-modal>*` | `flex-grow:1` |
| `.analog-tutorial-modal__video` | `height:420px;width:320px` |
| `.analog-tutorial-modal__desc` | `display:flex;flex-direction:column;justify-content:space-between;padding:50px 60px;width:480px` |
| `.analog-tutorial-modal .close` | `background-color:#0000;background-image:url(../../static/media/icon_close.55fe41f1.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;cursor:pointer;height:36px;position:absolute;right:0;top:0;transition:background-color .2s;width:36px;will-change:background-color` |
| `.analog-tutorial-modal .close:hover` | `background-color:#ffffff1a` |
| `.analog-tutorial-modal .main-title` | `color:#fff;font-size:20px;margin-bottom:30px` |
| `.analog-tutorial-modal .title` | `color:#44d62c;font-size:14px;margin-bottom:5px` |
| `.analog-tutorial-modal p` | `font-size:14px;margin:0` |
| `.analog-tutorial-modal .spacer` | `height:20px` |
| `.analog-tutorial-modal .dots` | `display:flex;margin-bottom:30px;margin-left:5px` |
| `.analog-tutorial-modal .dot` | `background-color:#707070;border-radius:10px;height:4px;margin-right:12px;width:4px` |
| `.analog-tutorial-modal .dot.active` | `background-color:#fff` |
| `.analog-tutorial-modal button` | `background-color:#707070;border:1px solid #000;border-radius:3px;color:#fff;cursor:pointer;font-size:12px;height:27px;line-height:14px;margin:0 10px 0 0;padding:6px 0 7px;text-align:center;text-transform:uppercase;transition:opacity .3s;-webkit-user-select:none;user-select:none;width:100px;will-change:opacity` |
| `.analog-tutorial-modal button.green` | `background-color:#44d62c;color:#000` |
| `.analog-tutorial-modal button.green:hover` | `background-color:#6ade57` |
| `.analog-tutorial-modal button:disabled` | `cursor:auto;opacity:.5` |
| `.analog-tutorial-modal-v2` | `background:#111;border:1px solid #44d62c;border-radius:3px;display:flex;height:420px;margin-top:134px;opacity:1;position:relative;width:800px` |
| `.analog-tutorial-modal-v2.thx-modal` | `height:auto;width:850px` |
| `.analog-tutorial-modal-v2-wrapper` | `align-items:flex-start;display:flex;justify-content:center` |
| `.analog-tutorial-modal-v2>*` | `flex-grow:1` |
| `.analog-tutorial-modal-v2__video` | `gap:10px;height:420px;opacity:1;width:320px` |
| `.analog-tutorial-modal-v2__desc` | `display:flex;flex-direction:column;gap:16px;height:420px;opacity:1;width:480px` |
| `.analog-tutorial-modal-v2 .close-bar` | `height:36px;opacity:1;position:relative;width:480px` |
| `.analog-tutorial-modal-v2 .close` | `background-color:#0000;background-image:url(../../static/media/icon_close.55fe41f1.svg);background-position:50%;background-repeat:no-repeat;background-size:24px;cursor:pointer;height:36px;left:440px;opacity:1;padding:6px 8px;position:absolute;top:0;width:40px` |
| `.analog-tutorial-modal-v2 .stepsTutorial` | `display:flex;flex-direction:column;height:255px;overflow-x:hidden;overflow-y:auto;padding:0 50px 30px` |
| `.analog-tutorial-modal-v2 .stepsTutorial .main-title` | `color:#ccc;font-family:Roboto;font-size:20px;font-weight:400;line-height:20px;margin-bottom:5px;text-transform:uppercase` |
| `.analog-tutorial-modal-v2 .stepsTutorial .main-title.thx-main-title` | `color:#ccc;font-weight:400;max-height:none;text-transform:uppercase` |
| `.analog-tutorial-modal-v2 .stepsTutorial .title` | `color:#44d62c;font-family:RazerF5;font-size:14px;font-weight:400;line-height:14px;margin-bottom:10px;text-transform:uppercase` |
| `.analog-tutorial-modal-v2 .stepsTutorial .content` | `display:flex;flex-direction:column;gap:20px;opacity:1;padding-top:10px;width:380px` |
| `.analog-tutorial-modal-v2 .stepsTutorial .content__block` | `display:flex;flex-direction:column;gap:5px` |
| `.analog-tutorial-modal-v2 .stepsTutorial .content__head` | `color:#44d62c;font-family:RazerF5;font-size:14px;line-height:14px;text-transform:uppercase` |
| `.analog-tutorial-modal-v2 .stepsTutorial .content__body` | `color:#ccc;font-family:Roboto;font-size:14px;line-height:17px;white-space:pre-line` |
| `.analog-tutorial-modal-v2 .stepsTutorial .content__body+.content__link` | `margin:10px 0` |
| `.analog-tutorial-modal-v2 .stepsTutorial .content__body+.content__link .link` | `color:#ccc;font-size:14px` |
| `.analog-tutorial-modal-v2 .stepsTutorial .content__body+.content__link .link:after` | `background-image:url(../../static/media/icon_external_link.48227e72.svg);content:"";display:inline-block;height:20px;line-height:20px;margin-left:10px;vertical-align:bottom;width:20px` |
| `.analog-tutorial-modal-v2 .stepsTutorial .content .content__head.thx-head` | `color:#44d62c;margin-bottom:8px;text-transform:uppercase` |
| `.analog-tutorial-modal-v2 p` | `font-size:14px;margin:0` |
| `.analog-tutorial-modal-v2 .spacer` | `height:20px` |
| `.modal-open .modal-patch-notes::-webkit-scrollbar` | `width:0` |
| `.modal-patch-notes .modal-dialog` | `transform:translate(0);transition:transform .3s ease-out` |
| `.modal-patch-notes` | `opacity:1;transition:none` |
| `.modal-patch-notes .modal-dialog` | `min-height:100%;transform:translateY(100%)` |
| `.modal-patch-notes.show .modal-dialog` | `transform:translateY(0)` |
| `.modal-patch-notes .modal-content` | `background-color:#222;border:none;border-radius:5px 5px 0 0;max-width:800px;position:absolute` |
| `.modal-patch-notes .modal-content>.wrapper` | `padding:0` |
| `.modal-patch-notes .modal-header` | `border-bottom:1px solid #5d5d5d;color:#999;font-family:RazerF5,Roboto,Arial,Microsoft YaHei New,Microsoft Yahei,微软雅黑,宋体,SimSun,STXihei,华文细黑,sans-serif;font-size:16px;padding:9px 10px 8px;text-transform:uppercase` |
| `.modal-patch-notes .modal-body` | `font-size:14px;max-height:534px;overflow-x:hidden;overflow-y:auto;padding:17px 30px;text-align:left` |
| `.modal-patch-notes .modal-body>.title` | `color:#999;font-family:RazerF5,Roboto,Arial,Microsoft YaHei New,Microsoft Yahei,微软雅黑,宋体,SimSun,STXihei,华文细黑,sans-serif;font-size:16px;margin-bottom:10px;text-transform:uppercase` |
| `.modal-patch-notes .modal-body>.tag` | `background-color:#44d62c;border-radius:3px;color:#212121;display:inline-block;font-size:12px;font-weight:700;line-height:1;min-width:90px;padding:5px 16px;text-align:center;text-transform:uppercase` |
| `.modal-patch-notes .modal-body>.tag.fixed` | `background-color:#28aadc` |
| `.modal-patch-notes .modal-body>.tag.improvement` | `background-color:#8b7add` |
| `.modal-patch-notes .modal-body>ul` | `list-style:none;margin:10px 0;padding:0` |
| `.modal-patch-notes .modal-body>ul>li` | `margin-bottom:8px;padding-left:16px;position:relative` |
| `.modal-patch-notes .modal-body>ul>li:before` | `background-color:#ccc;border-radius:50%;content:"";height:6px;left:0;position:absolute;top:5px;width:6px` |
| `.modal-patch-notes .modal-body .spinner-razer` | `display:block;height:74px;margin:67px auto 70px;width:74px` |
| `.modal-patch-notes .modal-footer` | `border-top:1px solid #555;display:flex;padding:10px 30px` |
| `.modal-patch-notes .modal-footer>.info` | `text-align:left;width:100%` |
| `.modal-patch-notes .modal-footer>.info>.progress-text` | `color:#999;font-size:10px;margin-bottom:5px` |
| `.carousel .s3-modal` | `align-items:normal` |
| `.carousel .s3-modal .md-devices--item` | `align-items:center;display:flex;margin-bottom:10px` |
| `.carousel .s3-modal .md-devices--item .check-item` | `display:flex;margin-bottom:0` |
| `.carousel .s3-modal .md-devices--item:last-child` | `margin-bottom:16px` |
| `.carousel .s3-modal .md-devices--item-name` | `margin-left:10px` |
| `.carousel .s3-modal--header` | `display:none` |
| `.carousel .s3-modal .s3-modal--content` | `height:-webkit-fit-content;height:fit-content;margin-top:113px` |
| `.carousel .s3-modal .s3-modal--body` | `background-color:#000;border:1px solid #fd4949;border-radius:3px;min-width:300px;padding:20px;width:auto` |
| `.carousel .s3-modal .s3-modal--body .content` | `margin:0 auto;width:-webkit-fit-content;width:fit-content` |
| `.carousel .s3-modal .s3-modal--body .content--header` | `font-size:14px;margin-bottom:10px;text-align:center` |
| `.carousel .s3-modal .s3-modal--body .content--body` | `font-size:14px;margin-bottom:16px;text-align:center` |
| `.carousel .s3-modal--action-remove .content--header` | `color:#fd4949;font-size:16px;line-height:16px;text-transform:uppercase` |
| `.carousel .s3-modal--action-remove .content--body` | `font-size:14px;margin-bottom:16px;text-align:center` |
| `.carousel .s3-modal--action-take-control .s3-modal--body` | `border:1px solid #707070` |
| `.carousel .s3-modal--action-take-control .s3-modal--body .content--header` | `margin-bottom:25px` |
| `.carousel .s3-modal--action-take-control .s3-modal--body .content--body` | `margin-bottom:10px` |
| `.carousel .s3-modal--action-take-control .s3-modal--body .content--body,.carousel .s3-modal--action-take-control .s3-modal--body .content--h` | `text-align:left` |
| `.iot-tutorial-modal` | `background:#111;border:1px solid #fd8611;border-radius:5px;left:190px;padding:40px 20px 20px;position:fixed;top:114px;width:290px` |
| `.iot-tutorial-modal__wrapper` | `position:relative;z-index:1050` |
| `.iot-tutorial-modal__override` | `left:calc(50% - 11px);position:absolute;top:128px` |

> 另有 91 条未列出。

---

## 图表（40 条）

| 选择器 | 声明 |
|---|---|
| `canvas.config-ctx` | `left:230px;position:absolute;z-index:1` |
| `.cb-canvas` | `left:66px;position:absolute;top:26px;z-index:1` |
| `.kb-flex>.kb-canvas` | `height:110px;opacity:.3;transition:opacity .3s;width:120px;will-change:opacity` |
| `.kb-canvas.on` | `opacity:1` |
| `#lightEffects canvas` | `box-sizing:border-box` |
| `.cp-wrapper,canvas.picker` | `height:120px;width:200px` |
| `.map_controller .canvas_graph_container_wraper .test` | `left:0;position:absolute;top:500px` |
| `.map_controller .canvas_graph_container_wraper .graph_wrapper` | `height:150px;margin-top:20px;position:relative` |
| `.map_controller .canvas_graph_container_wraper .canvas_graph_container` | `background-color:#000;display:inline-block;left:28px;padding:0!important;position:absolute` |
| `.map_controller .canvas_graph_container_wraper .canvas_graph_container #CanvasDiv,.map_controller .canvas_graph_container_wraper .canvas_gra` | `position:absolute;z-index:1` |
| `.map_controller .canvas_graph_container_wraper .canvas_graph_container #CanvasDiv3` | `position:absolute;z-index:0` |
| `.map_controller .canvas_graph_container_wraper .canvas_graph_container .dot` | `background-color:#44d62c;border-radius:50%;height:8px;position:absolute;width:8px;z-index:2` |
| `.map_controller .canvas_graph_container_wraper .canvas_graph_container .dot:hover` | `background-color:#707070;border:1px solid #44d62c` |
| `.map_controller .canvas_graph_container_wraper .canvas_graph_container .on` | `display:inline;display:initial` |
| `.map_controller .canvas_graph_container_wraper .bottom-bar` | `display:flex;height:17px;justify-content:space-between;left:28px;position:absolute;top:135px;z-index:0` |
| `.map_controller .canvas_graph_container_wraper .bottom-bar span` | `color:#ccc;font-size:10px;text-align:left` |
| `.map_controller .canvas_graph_container_wraper .left-bar` | `display:flex;flex-direction:column;justify-content:space-between;position:absolute` |
| `.map_controller .canvas_graph_container_wraper .left-bar span` | `color:#ccc;font-size:10px;text-align:right` |
| `.map_controller .canvas_graph_container_wraper .left-bar .analog` | `margin-top:60px;text-align:end;transform:rotate(-90deg);white-space:nowrap` |
| `.map_controller .canvas_graph_container_wraper .opacity03` | `opacity:.3` |
| `.map_controller .map_controller_graph` | `position:absolute;z-index:100` |
| `.choose-network-container .paragraph-1` | `margin-bottom:0` |
| `.wifi-device-add-start .paragraph-1` | `margin-top:20px` |
| `.wifi-device-add-start .paragraph-2` | `margin-bottom:6px;margin-top:20px` |
| `.MonitoringDashboard_metrics__TiaS0` | `display:flex;flex-direction:row;flex-wrap:nowrap;gap:10px;justify-content:flex-start` |
| `.MonitoringDashboard_metric__fKu5T` | `display:flex;flex:0 0 auto;flex-direction:column;gap:4px` |
| `.MonitoringDashboard_metricValue__TD\+hD` | `color:#ccc;font-family:RazerF5,sans-serif;font-size:15px` |
| `.MonitoringDashboard_metricBar__0oZYy` | `background-color:#111;border-radius:4px;height:5px;overflow:hidden;position:relative;width:100%` |
| `.MonitoringDashboard_metricBarFill__TKivl` | `border-radius:4px;height:100%;left:0;position:absolute;top:0;transition:width .3s ease,background-color .3s ease` |
| `.MonitoringDashboard_metricBarFill__TKivl[style*="width: 0%"]` | `background-color:initial` |
| `.MonitoringDashboard_metricBarFill__TKivl[style*="width:"]` | `background-color:#44d62c` |
| `.MonitoringDashboard_metricBarFill__TKivl[style*="width: 6"],.MonitoringDashboard_metricBarFill__TKivl[style*="width: 7"]` | `background-color:#ff4` |
| `.MonitoringDashboard_metricBarFill__TKivl[style*="width: 8"]` | `background-color:#f84` |
| `.MonitoringDashboard_metricBarFill__TKivl[style*="width: 100%"],.MonitoringDashboard_metricBarFill__TKivl[style*="width: 9"]` | `background-color:#f44` |
| `.MonitoringDashboard_basicBar__aKKrl .MonitoringDashboard_metricBarFill__TKivl` | `left:0;position:absolute;top:0` |
| `.MonitoringDashboard_metricVertical__fXfJS` | `align-items:flex-end;display:flex;flex-direction:column;gap:4px` |
| `.MonitoringDashboard_metricVertical__fXfJS .MonitoringDashboard_metricValue__TD\+hD` | `margin-bottom:4px` |
| `.MonitoringDashboard_metricVertical__fXfJS .MonitoringDashboard_metricBar__0oZYy` | `min-width:60px;width:100%` |
| `.MonitoringDashboard_metricBarGraphical__ImLFV` | `background:#111;border-radius:4px;height:40px;overflow:hidden;position:relative` |
| `.MonitoringDashboard_metricBarGraphical__ImLFV .MonitoringDashboard_metricBarFill__TKivl` | `left:0;position:absolute;top:0;transition:width .3s ease` |

---

## 滚动条（24 条）

| 选择器 | 声明 |
|---|---|
| `.scrollable` | `overflow-x:hidden;overflow-y:auto` |
| `::-webkit-scrollbar` | `height:8px;width:8px` |
| `::-webkit-scrollbar-corner` | `background-color:#222` |
| `::-webkit-scrollbar-track` | `background-color:#222;height:8px;width:8px` |
| `::-webkit-scrollbar-thumb` | `background:#ffffff4d;background-clip:padding-box;border:1px solid #0000;border-radius:4px;box-sizing:border-box;height:6px;width:6px` |
| `::-webkit-scrollbar-thumb:hover` | `background:#44d62c;background-clip:padding-box;border:1px solid #0000` |
| `::-webkit-scrollbar-thumb:active` | `background:#2f941e;background-clip:padding-box;border:1px solid #0000` |
| `.config-drawer ::-webkit-scrollbar-track` | `background-color:initial` |
| `.no-scroll` | `overflow:hidden!important` |
| `.s3-options-with-dual-item.bg-black::-webkit-scrollbar-track` | `background-color:#0000` |
| `.actions.scrollable>div.last-child` | `border-bottom-left-radius:5px;border-bottom-right-radius:5px` |
| `.key-mapping.actions.scrollable` | `overflow-y:scroll` |
| `.device_linked_game #transparent-scrollbar` | `overflow-y:auto` |
| `.app-list .scrollable` | `max-height:364px` |
| `.haptic-content__scroll` | `display:flex;justify-content:space-between;margin-top:20px` |
| `.haptic-content .scroll-slide` | `display:flex;flex-direction:column;justify-content:space-between;margin-left:20px;margin-top:34px` |
| `.haptic-content .scroll-slide .scroll-item` | `margin-bottom:20px;position:relative` |
| `.haptic-content .scroll-slide .scroll-item__title` | `display:flex;margin-bottom:5px` |
| `.haptic-content .scroll-slide .scroll-item__title .uppercase` | `text-transform:uppercase` |
| `.haptic-content .scroll-slide .scroll-item__title .help` | `margin-left:10px;position:relative;right:0;top:0` |
| `.haptic-content .scroll-slide .scroll-item:last-child` | `margin-bottom:0` |
| `.haptic-content .scroll-slide .keymap-action` | `justify-content:flex-end;margin-bottom:20px` |
| `.haptic-content .scroll-slide .keymap-action .map-save` | `margin-right:0` |
| `.MonitoringDashboard_sectionContainer__TqDpM::-webkit-scrollbar` | `display:none` |

---

## 文字（238 条）

| 选择器 | 声明 |
|---|---|
| `.main-title` | `color:#44d62c;font-family:RazerF5;font-size:20px;font-weight:400;line-height:24px;margin-bottom:20px;max-height:24px;text-transform:uppercase` |
| `.sub-title` | `align-items:center;flex-direction:column;justify-content:center;padding:0 50px;width:100%` |
| `.sub-title>h1` | `color:#ccc;font-size:18px;font-weight:400;line-height:22px;margin:0 0 20px` |
| `.body-text` | `margin-bottom:30px` |
| `.body-text,.cosplay-body-text` | `color:#ccc;font-size:14px;line-height:17px` |
| `.cosplay-body-text` | `margin:10px 0` |
| `.desc-text` | `color:#5d5d5d;font-size:14px` |
| `.desc-text,.desc-text-small` | `margin-bottom:20px;margin-left:50px` |
| `.desc-text-small` | `color:#999;font-size:12px` |
| `.img-text .windows` | `background-image:url(../../static/media/windows_logo.8fb1e7e2.svg);margin-right:20px` |
| `.img-text .windows,.img-text .windows.window-big-icon` | `background-position:50%;background-repeat:no-repeat;height:44px;max-width:44px;min-width:44px` |
| `.img-text .windows.window-big-icon` | `background-image:url(../../static/media/window-big-icon.0ec8a269.svg)` |
| `.img-text .windows-11` | `background-image:url(../../static/media/common-windows-11.d477cadb.svg)` |
| `.img-text .nvidia` | `background-image:url(../../static/media/icon_nvidia_square.428fa903.svg);max-width:44px` |
| `.img-text .nvidia,.img-text .thx-spatial` | `background-position:50%;background-repeat:no-repeat;height:44px;margin-right:20px;min-width:44px` |
| `.img-text .thx-spatial` | `background-image:url(../../static/media/icon_audio_enhancement_thx.e501de27.svg);background-size:100px;max-width:100px` |
| `.img-text .window-game-controller` | `background-image:url(../../static/media/controller_icon.b3be182d.svg);background-position:50%;background-repeat:no-repeat;height:44px;margin-right:20px;max-width:44px;min-width:44px` |
| `.img-text .external` | `color:#ccc;font-size:14px;line-height:17px;line-height:44px;text-decoration:underline;text-transform:capitalize` |
| `.img-text .external:hover` | `color:#44d62c` |
| `.img-text .external:hover:after` | `background-image:url(../../static/media/icon_external_link_active.b0d1c555.svg)` |
| `.img-text .external:active` | `opacity:.7` |
| `.img-text .external.text-sound-properties,.img-text .external.text-thx-spatial` | `text-transform:none` |
| `.img-text .thx-spatial-info .text-standard-appli` | `color:#707070;font-size:12px;line-height:5px` |
| `.img-text .volume` | `align-items:center;display:flex` |
| `.img-text .volume:after` | `background-image:url(../../static/media/icon_external_link_sprite.0b292d50.svg#link);background-position:50%;background-repeat:no-repeat;content:"";display:inline-block;height:20px;line-height:20px;margin-bottom:2px;margin-left:4px;transition:color .3s,opacity .3s;vertical-align:bottom;width:20px;will-change:color,opacity` |
| `.img-text .volume:hover:after` | `background-image:url(../../static/media/icon_external_link_sprite.0b292d50.svg#hover)` |
| `.remove-text-keymapping` | `background-image:url(../../static/media/icon_close_enclosed.6056b667.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;height:16px;position:absolute;right:5px;top:5px;transition:background-image .2s;width:16px;will-change:background-image` |
| `.remove-text-keymapping:hover` | `background-image:url(../../static/media/icon_close_enclosed_a.83aff4bb.svg);cursor:pointer` |
| `.key-config .body .specific-profile-interdevice .two-line-text` | `margin-bottom:10px;position:relative;width:170px` |
| `.key-config .body .specific-profile .two-line-text` | `margin-bottom:10px;position:relative;width:170px` |
| `.offline-warning-box .offline-title` | `color:#44d62c;font-size:16px;margin-bottom:20px` |
| `.launch-sound-app .img-text` | `color:#fff` |
| `.s3-options .ellipsis-text` | `overflow:hidden;text-overflow:ellipsis;white-space:nowrap` |
| `.s3-options-with-dual-item .raw-text` | `white-space:normal` |
| `.s3-options-with-dual-item .custom-shortcut .title` | `font-size:14px;line-height:17px;padding:5px 5px 3px` |
| `.title-more` | `color:#999;font-size:12px;font-weight:400` |
| `.title-more.min` | `left:0;top:70px` |
| `.title-more.max` | `right:0;top:70px` |
| `.profile-del .del-title,.profile-del .del-title-normal` | `color:#fd4949;margin-bottom:10px;text-align:center;text-transform:uppercase` |
| `.profile-del .del-title-normal` | `font-weight:400` |
| `.restart .title-text` | `color:#44d62c;font-size:16px;margin-bottom:10px;text-align:center;text-transform:uppercase` |
| `.obm-slot.white .white-text` | `margin-left:10px` |
| `.obm-slot .lock .text` | `max-width:170px;overflow:hidden;text-overflow:ellipsis` |
| `.reshare__desc` | `text-align:center;text-transform:none` |
| `.img-text` | `align-items:center;flex-direction:row` |
| `.h2-body,.h2-title` | `transition:opacity .3s;will-change:opacity` |
| `.h2-body.disabled,.h2-title.disabled` | `opacity:.3;pointer-events:none` |
| `.warning-text` | `margin-top:15px` |
| `.body-text.fix-h` | `margin-bottom:10px;max-height:35px;min-height:35px` |
| `.check-text` | `color:#ccc;display:block;font-size:14px;left:30px;line-height:17px;max-height:20px;min-width:-webkit-max-content;min-width:max-content;position:relative;text-align:left;top:2px` |
| `.check-text.overflow` | `min-width:420px` |
| `.check-text.capitalize` | `text-transform:capitalize` |
| `.check-text:first-letter` | `text-transform:uppercase` |
| `.lable-spinner .check-text` | `position:static;width:auto` |
| `.lable-spinner.lable-spinner .check-text` | `opacity:.3` |
| `.two-line-text` | `word-wrap:break-word;min-width:0;min-width:auto;top:-1px` |
| `.hyper-wrapper .text` | `color:#212121;display:inline-block;font-size:14px;height:24px;line-height:14px;padding:6px 10px 5px;position:relative;text-transform:capitalize;transition:color .2s;will-change:color;z-index:2` |
| `.hyper-wrapper .text.standard` | `background-color:#44d62c;border-radius:12px;margin-right:5px` |
| `.hyper-wrapper .text.hypershift` | `color:#ccc` |
| `.hyper-wrapper.hyper-on .text.standard` | `background-color:#0000;color:#ccc` |
| `.hyper-wrapper.hyper-on .text.hypershift` | `background-color:#fd8611;border-radius:12px;color:#212121` |
| `#twoTap .secondary-keymap-extra .function-title` | `width:auto` |
| `#twoTap .secondary-keymap-extra .function-title` | `margin:0 20px` |
| `.require-synapse .text` | `color:#707070;font-size:14px;line-height:14px;margin:0 5px;width:142px` |
| `.key-config .body textarea` | `border:1px solid #5d5d5d;box-sizing:border-box;font-family:Roboto;height:96px;min-width:210px;overflow-y:auto;padding:5px;resize:none;width:210px` |
| `.macro-keypad-module .func-wrapper .function-title` | `width:auto` |
| `.function-title` | `color:#707070;font-family:Roboto;font-size:14px;margin-left:60px;padding-bottom:3px;padding-top:15px;text-align:left;text-transform:uppercase;width:230px` |
| `.status-chroma-app .title` | `display:flex;flex:0 0 500px;flex-direction:column;justify-content:center` |
| `.status-chroma-app .title>div:first-child` | `color:#7d7d7d` |
| `.status-chroma-app .title .current-app` | `color:#ccc` |
| `.powerSaving-desc` | `margin-top:20px` |
| `.description-eq-map` | `display:flex;flex-direction:column;font-size:14px;height:240px;justify-content:space-evenly;line-height:17px` |
| `.description-volume-map` | `display:flex;flex-direction:column;font-size:14px;height:160px;justify-content:space-between;line-height:17px;margin-bottom:20px` |
| `.description-volume-map .volume-title` | `align-items:center;display:flex;justify-content:flex-start;margin-bottom:5px;text-transform:uppercase` |
| `.description-volume-map .volume-title:before` | `background-color:#0000;content:" ";display:block;height:16px;margin-right:4px;width:16px` |
| `.description-volume-map .volume-title.top:before` | `background-image:url(../../static/media/clockwise-gray.8b2c0b15.svg);background-position:50%;background-repeat:no-repeat;background-size:auto;line-height:17px` |
| `.description-volume-map .configure-sensitivity` | `margin:28px 0;text-decoration:underline` |
| `.description-volume-map .configure-sensitivity:hover` | `cursor:pointer` |
| `.description-volume-map .volume-title.mid:before` | `background-image:url(../../static/media/counter-clockwise-gray.a5d7275a.svg);background-position:50%;background-repeat:no-repeat;background-size:auto` |
| `.description-volume-map .volume-title.bottom:before` | `background-image:url(../../static/media/icon_config_multidial_white.c56d5b56.svg);background-position:50%;background-repeat:no-repeat;background-size:cover` |
| `.description-volume-map .volume-item` | `color:#999` |
| `.description-cycleup-sensitivity-map` | `display:flex;flex-direction:column;font-size:14px;height:140px;justify-content:space-between;line-height:17px;margin-bottom:20px` |
| `.description-cycleup-sensitivity-map .description-stages` | `color:#999;line-height:17px` |
| `.description-cycleup-sensitivity-map .configure-sensitivity` | `margin:20px 0 10px;text-decoration:underline` |
| `.description-cycleup-sensitivity-map .configure-sensitivity:hover` | `cursor:pointer` |
| `.access-mode .access-mode-title` | `font-size:14px;font-weight:400;width:147px` |
| `.text-left` | `text-align:left` |
| `.default-AI-launcher .description` | `align-items:flex-start;color:#999;display:flex;flex-direction:column;font-size:14px;justify-content:space-between;line-height:17px` |
| `.default-AI-launcher .description>div` | `margin-bottom:15px` |
| `.default-AI-launcher .description>div:last-child` | `margin-bottom:0` |
| `.default-AI-launcher .description .top` | `align-items:center;display:flex;justify-content:start` |
| `.default-AI-launcher .description>.top:before` | `background-color:#0000;background-image:url(../../static/media/long-press-gray.b6251ea4.svg);background-position:50%;background-repeat:no-repeat;background-size:cover;content:" ";display:block;height:18px;margin-right:6px;width:18px` |
| `.topview .message-mapping .description` | `color:#999;font-size:14px;line-height:17px;margin:0 8px` |
| `.description-dial-map` | `display:flex;flex-direction:column;font-weight:400;height:160px;justify-content:space-around` |
| `.description-dial-map .dial-map-title` | `font-size:12px;line-height:14px;margin-bottom:5px;text-transform:uppercase` |
| `.description-dial-map .dial-map-item` | `color:#999;font-size:14px;line-height:17px` |
| `.warning-container .warning-text` | `font-size:14px;text-align:center` |
| `.toolbar .title` | `color:#999;flex:3 1 340px;font-family:Roboto,sans-serif;font-size:14px;line-height:38px;overflow:hidden;text-align:center;text-transform:uppercase` |
| `.app-explorer .app-module-wrapper .app-module.installing .item .item-title:after` | `animation:spin 1s linear infinite;background-image:url('data:image/svg+xml;utf8,%3Csvg fill="none" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" stroke="%2344d62c"%3E%3Cg fill-rule="evenodd" stroke-width="2"%3E%3Ccircle opacity=".3" cx="12" cy="12" r="11"%3E%3C/circle%3E%3Cpath d="M12 1c2.8 0 5.6 1.1 7.8 3.2 4.3 4.3 4.3 11.3 0 15.6s-11.3 4.3-15.6 0"%3E%3C/path%3E%3C/g%3E%3C/svg%3E');backg` |
| `.app-explorer .app-module-wrapper .title` | `font-size:12px;line-height:inherit;margin-bottom:10px;text-align:left` |
| `.app-explorer .item-wrapper .item-title` | `color:#ccc;font-family:Roboto,sans-serif;font-size:14px;line-height:1.2;margin:0 auto;text-align:center;width:110px` |
| `.app-explorer .item-wrapper .item-title.elipsis` | `-webkit-line-clamp:2;/*! autoprefixer: ignore next */-webkit-box-orient:vertical;display:block;display:-webkit-box;height:33px;margin:0 auto;overflow:hidden;text-overflow:ellipsis` |
| `.synapse-profile-migration .toast-ui-text` | `font-size:14px` |
| `.dashboard .box-group.dragging .title .drag-icon:before` | `opacity:1;visibility:visible` |
| `.dashboard .box-group .title` | `align-items:stretch;color:#ccc;display:flex;font-size:14px;position:relative;text-decoration:none;width:-webkit-fit-content;width:fit-content;width:100%;z-index:1` |
| `.dashboard .box-group .title .drag-div` | `align-items:center;cursor:grab;display:flex;flex:1 1 auto;height:17px;justify-content:center;opacity:1;top:-3px;z-index:4` |
| `.dashboard .box-group .title .drag-icon` | `bottom:0;display:flex;justify-content:center;left:0;position:absolute;right:0;top:0;z-index:3` |
| `.dashboard .box-group .title .drag-icon:before` | `background-image:url(../../static/media/icon_draggable_large.e4bda42a.svg);background-position:50%;background-repeat:no-repeat;background-size:contain;content:"";cursor:grab;display:block;height:19px;opacity:0;transform:rotate(90deg);visibility:hidden;width:22px` |
| `.dashboard .box-group .title .collapse` | `align-items:center;display:flex;flex:1 1;position:relative;z-index:2` |
| `.dashboard .box-group .title .collapse .collapse-action` | `z-index:5` |
| `.dashboard .box-group .title .collapse .icon` | `background-image:url(../../static/media/icon_expand.55a47b0c.svg);background-position:50%;background-repeat:no-repeat;background-size:10px;display:inline-block;height:10px;margin-right:10px;transform:rotate(-90deg);transition:transform .3s linear;width:10px;will-change:transform` |
| `.dashboard .box-group .title .collapse:hover` | `color:#fff` |
| `.dashboard .box-group .title .collapse:hover .icon` | `background-image:url(../../static/media/icon_expand_l.3b8f6b34.svg)` |
| `.dashboard .box-group.expand .title .icon` | `transform:rotate(0deg)` |
| `.dashboard .box-no-device .title` | `font-size:14px;justify-content:center;margin:57px auto 67px;text-align:center;text-transform:uppercase` |
| `.dashboard .box-flip .box-flip-inner .box-flip-back .body .description` | `margin-bottom:10px` |
| `.dash-new .title` | `color:#44d62c;font-family:RazerF5;font-size:26px;font-weight:300;line-height:30px;margin-bottom:10px;text-align:center;text-transform:uppercase` |
| `.dash-new .body-text` | `margin-bottom:20px` |
| `.custom-global-shortcuts .global-shortcut .titleRow` | `margin-left:10px;margin-right:10px` |
| `.shortcut_item .shortcut_item_title .shortcut_title_temp` | `background-color:#7070704d;height:9px;width:59px` |

> 另有 118 条未列出。

---

## 图标（121 条）

| 选择器 | 声明 |
|---|---|
| `.interdevice-icon-show` | `display:flex;justify-content:center` |
| `.interdevice-icon-show img` | `margin-right:4px` |
| `.interdevice-icon-show span` | `display:inline-block;max-width:200px;overflow:hidden!important;padding-top:3px;text-overflow:ellipsis;white-space:pre` |
| `.offline-warning-box .icon_close` | `background-image:url(../../static/media/icon_close.55fe41f1.svg);background-repeat:no-repeat;cursor:pointer;height:20px;margin-left:auto;margin-right:0;position:relative;right:-10px;top:-10px;width:20px` |
| `.main-nav .search-wrapper #remove-icon` | `background-image:url(../../static/media/icon_close_enclosed.6056b667.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;display:none;display:block;height:25px;position:absolute;right:0;top:0;transition:background-image .2s;width:25px;will-change:background-image` |
| `.main-nav .search-wrapper #remove-icon:hover` | `background-image:url(../../static/media/icon_close_enclosed_a.83aff4bb.svg);cursor:pointer` |
| `.profile-bar .app-profile-icon:before` | `text-transform:capitalize` |
| `.obm-slot .lock .icon` | `margin-left:10px;right:0` |
| `.gamer-room-icon` | `background-image:url(../../static/media/icon_gamer_room.730988fc.svg);background-position:50%;background-repeat:no-repeat;height:25px;width:25px` |
| `.maintenance-icon` | `background-image:url(../../static/media/icon_cone.7bf8041f.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;height:20px;margin-left:auto;width:20px` |
| `.search-bar .search-wrapper .back-icon` | `background-image:url(../../static/media/icon_back_arrow.b39e4841.svg);background-repeat:no-repeat;background-size:20px;cursor:pointer;height:36px;left:10px;position:absolute;top:22%;transition:background-color .2s;width:36px` |
| `.search-bar .search-wrapper #remove-icon` | `background-image:url(../../static/media/icon_close_enclosed.6056b667.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;display:none;display:block;height:36px;position:absolute;right:0;top:0;transition:background-image .2s;width:36px;will-change:background-image;z-index:107` |
| `.search-bar .search-wrapper #remove-icon:hover` | `background-image:url(../../static/media/icon_close_enclosed_a.83aff4bb.svg);cursor:pointer` |
| `.status-chroma-app .chroma-icon` | `background-position:50%;background-repeat:no-repeat;background-size:20px 20px;height:auto;padding:10px;position:absolute;width:25px` |
| `.knob-item.active .icon-info:before` | `background-image:url(../../static/media/more-info-green.97803e98.svg)` |
| `.knob-item .icon-info:before` | `background-color:#0000;background-image:url(../../static/media/more-info.573f407b.svg);content:" ";display:block;height:16px;margin:9px;width:16px` |
| `.knob-item .icon-info:hover:before` | `background-image:url(../../static/media/more-info-green.97803e98.svg)` |
| `.topview .message-mapping .info-icon` | `background-image:url(../../static/media/info-icon.769264c1.svg);background-size:cover` |
| `.topview .message-mapping .close,.topview .message-mapping .info-icon` | `background-position:50%;background-repeat:no-repeat;height:20px;width:20px` |
| `.warning-container .warning-icon` | `background-image:url(../../static/media/icon_warning.6c0cd78b.svg);background-position:50%;background-repeat:no-repeat;height:20px;width:20px` |
| `.toolbar .arrow.icon-refresh` | `background-size:20px` |
| `.app-explorer-icon` | `background-image:url(../../static/media/icon_app.b648a5e5.svg);background-repeat:no-repeat;height:20px;width:20px` |
| `.app-explorer .item-wrapper .item-icon` | `display:flex;justify-content:center;margin-bottom:5px;position:relative` |
| `.app-explorer .item-wrapper .item-icon img` | `-webkit-user-drag:none;display:block;height:40px;margin:auto;object-fit:contain;width:40px` |
| `.synapse-profile-migration .profile-migration-icon` | `background-image:url(../../static/media/synapse_profile_migration.c3622288.svg);background-position:50%;background-repeat:no-repeat;height:40px;width:40px` |
| `.reset-oled-download .download-icon` | `background-image:url(../../static/media/icon_download_animated.c25fe72c.svg);background-repeat:no-repeat;background-size:cover;height:20px;width:20px` |
| `.icon-refresh` | `background-image:url(../../static/media/icon_refresh_white.80aa16c3.svg);height:40px;width:40px` |
| `.dashboard .box-item .batt .batt-icon` | `background-image:url(../../static/media/icon_battery_charging_100.d01170a4.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;height:26px;width:26px` |
| `.dashboard .box-item .batt .batt-icon.charging` | `background-image:url(../../static/media/icon_battery_charging.99e70522.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-100` | `background-image:url(../../static/media/icon_battery_100.b00b88f9.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-80` | `background-image:url(../../static/media/icon_battery_80.cabac91c.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-90` | `background-image:url(../../static/media/icon_battery_90.f59d2a3d.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-70` | `background-image:url(../../static/media/icon_battery_70.73ff3b3b.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-60` | `background-image:url(../../static/media/icon_battery_60.ca9e0c58.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-50` | `background-image:url(../../static/media/icon_battery_50.b36451e7.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-40` | `background-image:url(../../static/media/icon_battery_40.425e62c7.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-30` | `background-image:url(../../static/media/icon_battery_30.2ea266e0.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-20` | `background-image:url(../../static/media/icon_battery_20.5573858b.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-10` | `background-image:url(../../static/media/icon_battery_10.5a0a21e9.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-disconnected` | `background-image:url(../../static/media/icon_battery_disconnected.e5f74ae3.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-off` | `background-color:red;background-image:none;-webkit-mask-image:url(../../static/media/icon_device_off.0a0b5f8a.svg);mask-image:url(../../static/media/icon_device_off.0a0b5f8a.svg)` |
| `.dashboard .box-item .batt .batt-icon.batt-warning` | `background-image:url(../../static/media/icon_battery_error.44052666.svg)` |
| `.dashboard .box-flip .box-flip-inner .box-flip-front .count-icon` | `align-items:center;background-color:#707070;border-radius:15px;display:flex;height:24px;justify-content:center;position:absolute;right:10px;top:10px;width:24px` |
| `.shortcut_item .shortcut_item_warning_icon_show` | `height:20px;min-width:20px;position:relative;visibility:visible;width:20px` |
| `.shortcut_item .shortcut_item_warning_icon_hidden,.shortcut_item .shortcut_item_warning_icon_show` | `background-image:url(../../static/media/icon_warning.6c0cd78b.svg);background-repeat:no-repeat;margin-right:10px;margin-top:10px` |
| `.shortcut_item .shortcut_item_warning_icon_hidden` | `height:26px;visibility:hidden;width:26px` |
| `.shortcut_item .shortcut_item_warning_icon_show:hover .shortcut_item_warning_content` | `visibility:visible` |
| `.icon.spinner.up` | `background-image:url(../../static/media/stepper_up.dcb04520.svg);background-position-y:5px;top:0` |
| `.icon.spinner.down,.icon.spinner.up` | `opacity:0;transition:visibility 0s,opacity .1s linear;visibility:hidden;will-change:visibility,opacity` |
| `.icon.spinner.down` | `background-image:url(../../static/media/stepper_down.349f755c.svg);background-position-y:3px;bottom:0` |
| `.app-row.app-disabled .app-icon,.app-row.app-disabled .name` | `opacity:.3` |
| `.app-row .app-icon` | `height:40px;max-width:40px;padding:5px` |
| `.app-row .app-icon div.icon` | `background-position:50%;background-repeat:no-repeat;background-size:30px;height:30px;width:30px` |
| `.app-row.thead .app-icon` | `background-color:#0000;height:1px` |
| `.app-item .app-icon` | `max-width:30px;padding:0` |
| `.external-link__icon` | `background:url(../../static/media/icon_external_link_sprite.0b292d50.svg#link) no-repeat 50%;display:inline-block;flex-shrink:0;height:20px;width:20px` |
| `.external-link:hover .external-link__icon` | `background:url(../../static/media/icon_external_link_sprite.0b292d50.svg#hover) no-repeat 50%` |
| `.modes-area>.warn-wrapper .info-icon` | `background-image:url(../../static/media/info-icon.769264c1.svg);background-position:50%;background-repeat:no-repeat;background-size:cover;flex-shrink:0;height:20px;margin:2px;width:20px` |
| `.stage:hover .icon.spinner` | `opacity:1;visibility:visible` |
| `.stage .icon-draggable` | `height:100%;margin-left:15px;width:10px` |
| `.stage .icon-draggable,.stage .icon-sensitivity-xy` | `background-position:50%;background-repeat:no-repeat` |
| `.stage .icon-sensitivity-xy` | `cursor:pointer;height:32px;margin-left:10px;width:32px` |
| `.stage.off .icon-sensitivity-xy` | `background-image:url(../../static/media/icon_sensitivity_xy_disabled.3a6ee34e.svg)` |
| `.icon-sensitivity-xy` | `background-image:url(../../static/media/icon_sensitivity_xy.b9eb5286.svg);position:relative` |
| `.icon-sensitivity-xy.active` | `background-image:url(../../static/media/icon_sensitivity_xy_active.39d46352.svg)` |
| `.icon-sensitivity-xy.disabled` | `background-image:url(../../static/media/icon_sensitivity_xy_disabled.3a6ee34e.svg)` |
| `.icon-draggable` | `background-image:url(../../static/media/icon_draggable_large.e4bda42a.svg)` |
| `.analog-tutorial-icon.keyboard` | `background-image:url(../../static/media/icon_tutorial_book.8b20c805.svg);background-position:50%;background-repeat:no-repeat;border:1px solid #0000;height:24px;margin-right:20px;opacity:1;text-transform:capitalize;transition:border-color .2s;width:24px;will-change:border-color` |
| `.analog-tutorial-icon.keyboard:before` | `left:-5px;top:30px;width:-webkit-max-content;width:max-content` |
| `.analog-tutorial-icon.keyboard.ja:before,.analog-tutorial-icon.keyboard.ru:before` | `left:-54px` |
| `.analog-tutorial-icon.keyboard:hover` | `border:1px solid #5d5d5d` |
| `.analog-tutorial-icon.keyboard.active,.audio-tutorial` | `border:1px solid #44d62c` |
| `.analog-tutorial-icon` | `background-image:url(../../static/media/icon_tutorial_book.8b20c805.svg);background-position:50%;background-repeat:no-repeat;height:25px;margin-right:20px;opacity:.8;width:25px` |
| `.analog-tutorial-icon:hover` | `opacity:1` |
| `.analog-tutorial-icon.active` | `background-image:url(../../static/media/icon_tutorial_book_active.bc3b0a42.svg)` |
| `.main-setting .about-setting .social-container .social .icon` | `height:27px;width:27px` |
| `.main-setting .about-setting .social-container .social .icon svg` | `padding:1px` |
| `.main-setting .about-setting .social-container .social .icon img` | `margin:50% 0 0 50%;transform:translate(-50%,-50%)` |
| `.icon-launch-config` | `background-image:url(../../static/media/icon_external_link.48227e72.svg)` |
| `.help-component .system-info .download .icon-dowload` | `background-image:url(../../static/media/icon_download.1e6d735a.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;height:20px;margin-right:5px;width:20px` |
| `.icon-actual-point` | `background-image:url(../../static/media/icon_actuation_point.bbadbed4.svg)` |
| `.actual-point .icon-actual-point` | `background-position:50%;background-repeat:no-repeat;height:20px;margin-right:5px;width:20px` |
| `.actual-point .icon-custom-release` | `background-position:50%;background-repeat:no-repeat;height:20px;margin-right:5px;width:20px` |
| `.icon-custom-release` | `background-image:url(../../static/media/icon_release.45f570dc.svg)` |
| `.sync .icon-sync` | `background-position:-2px -6px;background-repeat:no-repeat;height:20px;margin-right:5px;padding:6px;width:20px` |
| `.icon-sync` | `background-image:url(../../static/media/icon_sync.c57b3cca.svg)` |
| `.module-installation-container .module-installation .module-main-content .module-icon` | `align-items:center;display:flex;height:40px;justify-content:center;width:40px` |
| `.module-installation-container .module-installation .module-main-content .module-icon img` | `height:100%;pointer-events:none;width:100%` |
| `.stages.draggable .stage.not-checked .icon-draggable` | `pointer-events:none` |
| `.stages.draggable .stage .icon-draggable` | `display:block;margin-left:0;margin-right:15px;text-align:center` |
| `.detect-item,.detect-item__icon` | `align-items:center;display:flex` |
| `.detect-item__icon` | `height:30px;justify-content:center;margin-right:10px;width:30px` |
| `.detect-item__icon img` | `width:100%` |
| `.detect-item:hover .detect-item__icon img` | `content:url(../../static/media/icon_close_enclosed.6056b667.svg);height:20px;width:20px` |
| `.detect-item:hover .detect-item__icon img:hover` | `content:url(../../static/media/icon_close_enclosed_a.83aff4bb.svg)` |
| `.carousel--item .device.active .device--name.edit-icon:hover:after` | `background-image:url(../../static/media/pen_edit_hover.c995cdb0.svg);background-position:50%;background-repeat:no-repeat;content:"";height:24px;width:24px` |
| `.indicator-led__container .indicator-led__link-icon` | `cursor:pointer;height:20px;opacity:1;width:20px` |
| `.iot-device-mobile .mobile-icon-wrap` | `align-items:center;background:#44d62c;border-radius:50%;display:flex;height:32px;justify-content:center;margin-right:10px;width:32px` |
| `.iot-device-mobile .mobile-icon` | `display:block;height:32px;width:32px` |
| `.wifi-device-add-start .wifi-icon` | `height:36px;margin-top:25px;width:48px` |
| `.sw-option .sw-help.info-icon` | `background-image:url(../../static/media/icon_info_solid_gray.b945e153.svg)` |
| `.sw-option .sw-help.question-icon` | `background-image:url(../../static/media/tooltip_questionmark.96138d2f.svg)` |
| `.flex-item-app .preset-icon` | `align-items:center;border:2px solid #0000;border-radius:5px;display:flex;justify-content:center;margin-right:4px;overflow:visible;padding:5px;position:relative;transition:border .3s ease,transform .3s ease` |
| `.flex-item-app .preset-icon:hover .close-icon` | `background-image:url(../../static/media/close-white.58893552.svg);background-position:50%;background-repeat:no-repeat;background-size:cover;height:16px;transition:all .2s;width:16px` |
| `.flex-item-app .preset-icon:hover .close-icon:hover` | `background-image:url(../../static/media/close-red.ac5bd6f2.svg);transition:all .2s` |
| `.flex-item-app .preset-icon img` | `height:24px;width:24px` |
| `.flex-item-app .preset-icon.active,.flex-item-app .preset-icon.active:hover` | `border:2px solid #3ebd29` |
| `.flex-item-app .preset-icon:hover` | `border:2px solid rgba(68,214,44,.302)` |
| `.flex-item-app .preset-icon .close-icon` | `cursor:pointer;display:none;height:16px;position:absolute;right:-8px;top:-8px;width:16px;z-index:5` |
| `.flex-item-app .preset-icon .close-icon img` | `height:16px;width:16px` |
| `.flex-item-app .preset-icon:hover .close-icon` | `display:block` |
| `.detect-item-app,.detect-item-app__icon` | `align-items:center;display:flex` |
| `.detect-item-app__icon` | `height:30px;justify-content:center;margin-right:10px;width:30px` |
| `.detect-item-app__icon img` | `width:100%` |
| `.obm-menu .macros .heading .icon` | `background-image:url(../../static/media/icon_macro.1d733729.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;display:inline-block;flex:0 0 auto;height:20px;margin-right:12px;width:20px` |
| `.obm-slot .lock .icon` | `background-image:url(../../static/media/icon_lock.8f4a479b.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;display:inline-block;height:20px;position:absolute;right:20px;top:14px;width:20px` |
| `.MonitoringDashboard_icon__m3QC-` | `align-items:center;display:inline-flex;flex-shrink:0` |
| `.MonitoringDashboard_iconSvg__2BdHI` | `color:#999` |
| `.ImportExportModal_inWork__xU05k .ImportExportModal_cone-icon__0oCBX` | `margin-right:10px` |
| `.ImportExportModal_cone-icon__0oCBX` | `background-image:url(../../static/media/icon_cone.7bf8041f.svg);background-position:50%;background-repeat:no-repeat;background-size:auto;display:inline-block;height:40px;width:40px` |

> 另有 1 条未列出。

---

## 未归类（1507 条）

| 选择器 | 声明 |
|---|---|
| `#root` | `height:100%` |
| `0%` | `transform:rotate(0deg)` |
| `to` | `transform:rotate(1turn)` |
| `body,html` | `background-color:#222;color:#ccc;font-family:Roboto,sans-serif;font-size:16px;height:100%;margin:0;max-width:1920px;min-height:720px;overflow:hidden;-webkit-user-select:none;user-select:none;width:100%` |
| `div` | `box-sizing:border-box` |
| `.dim` | `color:#707070` |
| `.ml5,.mr5` | `margin-right:5px` |
| `.ml10` | `margin-left:10px` |
| `.mr10` | `margin-right:10px` |
| `.mr20` | `margin-right:20px` |
| `.mt20` | `margin-top:20px` |
| `.mt10` | `margin-top:10px` |
| `.mt0` | `margin-top:0` |
| `.mt-10` | `margin-top:-5px` |
| `.mb10` | `margin-bottom:10px` |
| `.mb20` | `margin-bottom:20px` |
| `.mb30` | `margin-bottom:30px` |
| `.mb40` | `margin-bottom:40px` |
| `.z100` | `z-index:100` |
| `.gap10` | `gap:10px` |
| `.warning` | `padding-left:30px;position:relative;word-break:break-word` |
| `.warning.hidden` | `visibility:collapse` |
| `.warning:before` | `background-image:url(../../static/media/warning.ad3f47f8.svg);background-position:50%;background-repeat:no-repeat;content:"";display:inline-block;height:20px;left:0;position:absolute;width:20px` |
| `.dismiss` | `color:#999;font-size:14px;line-height:16px;text-align:center;text-decoration:underline;text-transform:capitalize;transition:color .3s,opacity .3s;will-change:color,opacity` |
| `.dismiss:hover` | `color:#44d62c` |
| `.dismiss:active` | `opacity:.7` |
| `div.flex` | `display:flex` |
| `div.flex>div` | `flex:auto` |
| `div.flex-v` | `flex-direction:column` |
| `div.flex-0` | `flex:0 0 auto!important` |
| `div.flex-end` | `align-items:flex-end!important` |
| `.flex-row-20` | `height:5px;max-width:20px;min-width:20px` |
| `.flex-col-30` | `max-height:30px;min-height:30px;width:5px` |
| `.flex-row-flex` | `flex:1 0 20px!important;height:5px;max-width:120px` |
| `.config-drawer-content` | `max-height:calc(100% - 145px)` |
| `.name-bar` | `background-color:#2b2b2b;color:#707070;font-family:RazerF5;font-size:.875rem;font-weight:300;height:1.875rem;max-height:1.875rem;min-width:600px;padding:.5rem 0;text-align:center;text-transform:uppercase;width:100%;z-index:100` |
| `.insert-ref` | `display:none` |
| `.chroma-studio` | `background-color:#111;border:1px solid #737373;border-radius:3px;height:27px;line-height:17px;min-width:150px;padding:5px 8px 5px 31px;position:relative;text-transform:uppercase;transition:border-color .3s,background-color .3s;will-change:border-color,background-color` |
| `.chroma-studio:before` | `background-image:url(../../static/media/chroma_studio.55db6875.svg);content:"";display:inline-block;height:18px;left:8px;position:absolute;top:4px;width:18px` |
| `.chroma-studio-animate` | `height:26px;margin-right:10px;position:relative;width:26px` |
| `.chroma-studio-animate:before` | `background-image:url(../../static/media/chroma_sync_v3_static.c8ddf315.svg);content:"";display:inline-block;height:26px;left:0;position:absolute;top:0;width:26px` |
| `.chroma-studio-animate:hover,.chroma-studio:hover` | `border-color:#44d62c` |
| `.chroma-studio-animate:active,.chroma-studio:active` | `background-color:#ffffff1a` |
| `.chroma-studio-animate.turn:before` | `animation:chromasync .4s linear 3` |
| `.chroma-studio-animate.off:active,.chroma-studio-animate.off:hover` | `background-color:#111;border-color:#737373;cursor:default` |
| `.t-center` | `text-align:center` |
| `.game-chat-balance .tutorial:hover` | `color:#44d62c` |
| `.game-chat-balance .help-config` | `margin-bottom:1px` |
| `.stackpanel-hor` | `display:flex` |
| `.stackpanel-left` | `flex:0 0 30%` |
| `.stackpanel-right` | `flex:1 1;margin-left:20px` |
| `#profileList` | `max-height:204px` |
| `.hover-border` | `background-position:50%;background-repeat:no-repeat;background-size:20px;border:1px solid #222;height:26px;margin-right:10px;transition:border-color .2s;width:26px;will-change:border-color` |
| `.hover-border:hover` | `border-color:#5d5d5d` |
| `.hover-border.active,.hover-border:active` | `border-color:#44d62c` |
| `.img-nodrag` | `-webkit-user-drag:none;-webkit-backface-visibility:hidden;backface-visibility:hidden;-webkit-user-select:none;user-select:none` |
| `.img-nodrag.width-extend` | `width:auto` |
| `.dot-container` | `position:relative` |
| `.dot-container img` | `left:50%;position:absolute;transform:translateX(-50%);z-index:1` |
| `.zindex1` | `z-index:1` |
| `.mb5` | `margin-bottom:5px` |
| `.mt5` | `margin-top:5px` |
| `.key-require-synapse` | `background-image:url(../../static/media/require_synapse_icon.d5e656b0.svg)` |
| `.key-require-macro-module,.key-require-synapse` | `background-size:cover;height:20px;pointer-events:none;position:absolute;width:20px` |
| `.key-require-macro-module` | `background-image:url(../../static/media/require_macro_module_icon.20694b5c.svg)` |
| `.key-snap-tap` | `background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg);height:16px;width:16px` |
| `.key-dks,.key-snap-tap` | `background-size:cover;pointer-events:none;position:absolute` |
| `.key-dks` | `background-image:url(../../static/media/dynamic_key_stroke_icon.c4bd98bb.svg);height:10px;width:19px` |
| `.key-config .body .specific-profile-interdevice` | `margin-bottom:20px;margin-left:30px;width:180px` |
| `.key-config .body .specific-profile` | `margin-left:30px;width:180px` |
| `.show3dot-for-tartarus span` | `display:inline-block;max-width:101px;overflow:hidden!important;text-overflow:ellipsis;white-space:nowrap` |
| `.ml260` | `margin-left:260px` |
| `.left-click-disable-message` | `background-color:#6d6b6bf5;color:#ccc;font-family:Roboto;font-size:12px;padding:3px 20px;text-align:center` |
| `.clutch-XY` | `margin-right:10px` |
| `.clutch-Y` | `float:right` |
| `.macro-body-widget  [@media(min-width:1029px)]` | `overflow:hidden` |
| `.offline-warning-box` | `background-color:#000c;display:none;height:100vh;left:0;position:fixed;top:0;width:100vw;z-index:199` |
| `.offline-warning-box.show` | `display:block` |
| `.offline-warning-box a` | `color:#ccc` |
| `.offline-warning-box .offline-qr-code` | `background-position:50%;background-repeat:no-repeat;height:200px;margin-bottom:10px` |
| `.offline-warning-box .offline-warning` | `background-color:#222;border:1px solid #44d62c;border-radius:5px;font-size:14px;left:50%;line-height:16px;padding:20px;position:absolute;text-align:center;top:50%;transform:translate(-50%,-50%);width:380px` |
| `.clear-section` | `align-items:center;background-color:#5d5d5d;border-radius:3px;color:#ccc;display:flex;height:35px;justify-content:space-between;max-width:250px;opacity:0;padding:10px;position:absolute;visibility:hidden;width:250px` |
| `.clear-section>span` | `color:#999` |
| `.clear-section.show` | `opacity:1;visibility:visible;z-index:999` |
| `.clear-section:hover` | `background-color:#2d2d2d` |
| `.accessibility *` | `-webkit-tap-highlight-color:rgba(0,0,0,0);outline:0` |
| `a.nav` | `text-decoration:none` |
| `.nav.active` | `background-color:#44d62c;color:#111` |
| `.right .battery` | `align-items:center;color:#ccc;display:flex;font-size:14px;height:46px;justify-content:center` |
| `.nav.disabled` | `opacity:.3;pointer-events:none` |
| `.nav.back` | `background-image:url(../../static/media/nav_back_arrow.0203563a.svg)` |
| `.nav.forward` | `background-image:url(../../static/media/nav_fwd_arrow.8ceda865.svg)` |
| `.toolbar .user.trial` | `background-color:#44d62c;height:48px;right:60px;transition:background-color .3s;width:60px;will-change:background-color;z-index:99` |
| `.trial .countdown` | `background-color:#c8323c;border-radius:16px;color:#fff;font-size:14px;height:32px;line-height:17px;min-height:32px;opacity:1;padding:8px 0 7px;position:absolute;right:15px;text-align:center;top:8px;transition:top .3s,right .3s,width .3s ease,min-height .3s ease,padding .3s ease,background-color .3s,border .1s,border-radius .3s linear;visibility:visible;width:32px;will-change:width,min-height,backg` |
| `.toolbar .user .countdown:active,.toolbar.user:active .countdown` | `background-color:#8b2229;color:#b1b1b1` |
| `.pic .avatar` | `background-color:#ddd;border-radius:16px;height:32px;margin:8px 15px;width:32px` |
| `.user.trial .countdown.show` | `background-color:#111;border:1px solid #44d62c;border-radius:5px;height:auto;max-width:none;min-height:104px;padding:8px 30px 20px;position:absolute;right:-50px;top:58px;width:264px;z-index:100` |
| `.user.trial .countdown:before` | `content:"17";opacity:1;position:relative;transition:top .3s,visibility .3s,opacity 0s linear;visibility:visible;will-change:visibility,opacity` |
| `.user.trial .countdown.show:before` | `opacity:0;visibility:hidden` |
| `.user.trial .countdown .body` | `opacity:0;position:absolute;right:-63px;text-align:center;top:20px;transition-delay:.1s;transition:visibility 0s,opacity .5s linear,right .3s;visibility:hidden;width:262px;will-change:visibility,opacity` |
| `.user.trial .countdown.show .body` | `opacity:1;right:0;visibility:visible` |
| `.battery .low-batt` | `color:#c8323c` |
| `.hideBattValue` | `margin-right:17px` |
| `.main-nav` | `display:inline-flex;height:auto;margin-right:10px;width:100%` |
| `.main-nav .add` | `background-color:#0000;background-image:url(../../static/media/icon_add.c95a8d74.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;height:26px;transition:background-color .2s;width:26px;will-change:background-color` |
| `.main-nav .add:hover` | `background-image:url(../../static/media/icon_add.green.db605256.svg)` |
| `.main-nav .not-grey` | `background-image:url(../../static/media/icon_addnotgrey.f73db7b6.svg)` |
| `.main-nav .syncing_games` | `background:url(../../static/media/preloader.ef2d0235.svg) no-repeat 50%/26px;height:26px;width:26px` |
| `.main-nav .refresh_games` | `background:url(../../static/media/icon_refresh_gamelibrary.5f8edb51.svg) no-repeat 50%/20px;height:26px;width:26px` |
| `.main-nav .refresh_games:hover` | `background-image:url(../../static/media/icon_refresh_gamelibrary.green.66e2a41e.svg)` |
| `.main-nav .refresh` | `background-color:#0000;background-image:url(../../static/media/icon_refresh.80aa16c3.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;height:26px;opacity:1;transition:background-color .2s;visibility:visible;width:26px;will-change:background-color` |
| `.main-nav .refresh:hover` | `background-image:url(../../static/media/icon_refresh.green.10a444da.svg)` |
| `.main-nav .search` | `background-color:#0000;background-image:url(../../static/media/icon_search.b84dee08.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;height:26px;transition:background-color .2s;width:26px;will-change:background-color` |
| `.main-nav .search:hover` | `background-image:url(../../static/media/icon_search.green.f5e350b7.svg)` |
| `.main-nav .search-wrapper` | `position:relative` |
| `.main-nav .search-wrapper span:before` | `background-image:url(../../static/media/icon_search.grey.7c0cb7bb.svg);background-repeat:no-repeat;content:"";height:20px;left:3px;position:absolute;top:2px;width:20px` |
| `.main-nav .add:active,.main-nav .refresh:active,.main-nav .search:active` | `opacity:.7` |
| `.main-nav .loader` | `height:26px;margin:0;width:26px` |
| `.main-nav ul` | `align-items:center;display:flex;list-style:none;margin:0 10px 0 0;padding:0` |
| `.main-nav ul li` | `float:left;margin-right:10px` |

> 另有 1387 条未列出。

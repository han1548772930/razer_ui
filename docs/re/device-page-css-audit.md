# 设备页 CSS 逐条对账（基准：182）

2026-10-03。机器可读结果 [device-page-css-audit.json](device-page-css-audit.json)，脚本 [tools/audit-device-page-css.cjs](../../tools/audit-device-page-css.cjs)（`--check` 失败即报错）。
这是「逐页对账」方法的第一份产出：先钉住**每个页面都会用到**的组件级规则，再逐页细化。

## 组件级规则（源码 → 本地）

| 选择器（`.ref/devices/182/static/css/main.48c20423.css`） | 源码声明 | 本地 |
| --- | --- | --- |
| `.body-widgets .widget` | `background-color:#111;border-radius:5px;flex:0 0 auto;font-size:14px;height:auto;margin:10px auto;max-width:600px;min-width:600px;padding:30px 40px` | `surface::{WIDGET_RADIUS=5,WIDGET_WIDTH=600,WIDGET_PADDING_X=40,WIDGET_PADDING_Y=30}`、`group_box=#111`、`my(10px)`、`flex_shrink_0` |
| `.widget .titleRow` | `display:flex;justify-content:space-between` | `h_flex().justify_between()`（**原先用的是 20px 间距，控件没有贴右**，已修） |
| `.widget .titleRow .title` | `color:#44d62c;font-family:RazerF5,sans-serif;font-size:16px;margin-bottom:20px;text-transform:uppercase` | RazerF5 / 16px / `primary` / `mb(20px)` / 大写（原先用面板 `gap_4`=16px 近似，已修） |
| `.h1-body` | `color:#ccc;margin-bottom:10px` | 新增 `surface::h1_body`（`group_box_foreground=#CCCCCC` + `mb(10px)`），3 处说明文字改用它 |
| `.check-item` | `margin-bottom:9px` | 勾选项外包一层 `div().mb(9px)` |
| `.widget .content` | `margin-bottom:15px` | 滑杆块（`mouse_products::range`、`device_pages::source_range`）`mb(15px)` |
| `.slider-container` | `height:64px`；`.no-tip{height:36px}`；禁用 `opacity:.3;pointer-events:none`；轨道 `#44d62c4d`、填充 `#44d62c`、滑块 hover `#5d5d5d`+`2px #44d62c` | `device_pages::source_range` 已按 64/36 与 0.3 实现 |

## 已经定位但尚未实现的分支

`.powerSaving-value`（省电取值按钮，源码条件 `usePowerSavingButton`）：

| 选择器 | 声明 |
| --- | --- |
| `.powerSaving-value` | `align-items:center;background:#111;border:1px solid #5d5d5d;border-radius:3px;color:#fff;display:flex;font-size:12px;justify-content:center;line-height:14px` |
| `.powerSaving-value.mouse-power-saving` | `color:#ccc;font-size:12px;line-height:14px;min-width:78px;padding:7px 16px` |
| `.powerSaving-value:hover` | `border:1px solid #44d62c;cursor:pointer` |
| `.powerSaving-value.active` | `background:#292929;border-color:#44d62c` |
| `.powerSaving-wrap--disabled` | `opacity:30%;pointer-events:none;user-select:none` |
| `.powerSaving-select` | `display:flex;gap:10px;margin-top:20px` |
| `.powerSaving-desc` | `margin-top:20px` |

源码里这一分支的取值来自设备信息（`usePowerSavingButton`）或 `POWER_SAVING_VALUE_FROM_PRODUCT_INFO`，默认列表是 `[{id:0,value:15},{id:1,value:30},{id:2,value:45}]`；`src/fixtures/measured_devices.json` 里 182 没有这个字段，因此本地仍走源码的另一支（`min:1,max:15,step:1,minTag:"1",maxTag:"15"` 的滑杆），按钮分支待有真实设备信息接入后再实现（不伪造取值）。

## 性能页（第 16 轮）

| 选择器 | 源码声明 | 本地 |
| --- | --- | --- |
| `.polling-rate` | `padding-top:10px;position:relative;z-index:1` | 按钮区外包 `div().pt(10px)` |
| `.polling-btn-set` | `display:flex;flex-wrap:wrap;gap:10px 10px` | `h_flex().gap(10px).flex_wrap()` |
| `.customize-polling-rate-button` | `background-color:#222;border:1px solid #5d5d5d;border-radius:3px;color:#ccc;font-size:14px;height:27px;width:72px;justify-content:center;text-transform:uppercase`；`.active,:hover{border-color:#44d62c}` | 72×27、radius 3、`theme.background=#222`、`theme.border=#5d5d5d`、选中/hover 边框 `#44d62c` |
| `.polling-warn` | `margin-top:10px;opacity:.7` | 回报率 > 1000 时显示的提示块 |
| `.polling-learn-more` | `align-items:center;cursor:pointer;display:inline-flex;padding-left:5px;text-decoration:underline` | 「了解详情」+ 外链图标，`pl(5px)` + 下划线，URL 取源码 `https://www.razer.com/technology/razer-hyperpolling#best-practices-tips` |
| `.external-link-icon` | `background:url(icon_external_link.48227e72.svg);background-size:contain;display:inline-flex` | `synapse/external-link.svg`（同一文件，前几轮已打包） |
| `.external` | `color:#ccc;font-size:14px;line-height:44px;text-decoration:underline;text-transform:capitalize`；`:hover{color:#44d62c}` | 鼠标属性链接：14px、`#ccc`（`group_box_foreground`）、44px 行高、下划线、hover `#44d62c`（原先多画了一个外链图标，已去掉） |
| `.ingame-polling-rate` | `margin-top:20px`；`.title{font-family:RazerF5;font-size:14px;margin-top:20px}`；`.content{margin:12px 0}` | **未实现**：源码条件 `supportInGamePollingRate`，文案 key `INGAME_POLLING_RATE_HEADER`（轮询率智能切换）/ `AUTO_SWITCH_POLLING_RATE_WHEN_INGAME`（玩游戏时自动切换轮询率）已在本地语言包里，待接入设备信息后点亮 |
| `.stage-control` / `.stages .stage` | `.stage-control{height:27px;margin-top:25px;z-index:100}`、`.stage-control .switch{margin-left:10px;margin-top:-1px}`、`.stages .stage{height:68px;margin-bottom:4px;border-radius:3px}`、`.stage:last-child{margin-bottom:0}`、拖拽时 `border-bottom/top:2px solid #44d62c` | 灵敏度面板已有 27px 行高 + 25px 上边距 + 10px 间距；`margin-top:-1px` 与拖拽描边待核对 |

标题与文案改用源码 key：回报率面板 `HYPERPOLLING` / `HYPERPOLLING_WIRELESS`（对应源码 `isDongle||isBle` 分支），鼠标属性面板 `MOUSE_PROPERTIES_HEADER` / `MOUSE_PROPERTIES_DESC` / `MOUSE_PROPERTIES_TOOLTIP`。回报率警告句的 key 未在本地语言包里找到（设备包该表是运行时下发的译文），界面沿用既有中文说明，已在审计里标注为待替换。

## 对账方法（后续每页复用）

1. 从产品包 CSS 抽出该页类名的**原始声明**（工具已能按选择器精确取规则）；
2. 在本地找到对应渲染代码，逐声明比对；不一致就改本地，或把源码分支记录成「未实现 + 触发条件」；
3. 把结论写进本目录的 JSON + Markdown，工具 `--check` 保证不回退。

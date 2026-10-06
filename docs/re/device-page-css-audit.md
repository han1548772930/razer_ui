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

## 功耗取值标签、`BR` 组件与语言包缺口收口（第 8 轮）

`.ref/devices/112/static/js/main.47216244.js` 里的共享功耗组件：

```jsx
class GR extends Component {            // 单个取值按钮
  render() {
    const cls = "powerSaving-value " + (active ? "active" : "") + " " + (extraClass || "");
    return <div className={cls} id={item.id} onClick={this.changeValue}>
      <text text={item.value >= 60 ? ra.pHP : ra.yvH}
            data={{value: item.value >= 60 ? item.value / 60 : item.value}} />
    </div>;
  }
}
class BR extends Component {            // 面板：标题 + 提示 + h1-body + 按钮组 / 滑条
  render() {
    return <widget title={ra.bSO} tips={ra.RKF} hasSwitch={false}>
      <div className="h1-body"><text text={powerSavingConfig ? ra.YCM : ra.rGE} /></div>
      {usePowerSavingButton
        ? <div className="powerSaving-select">{(POWER_SAVING_VALUE_FROM_PRODUCT_INFO || fR).map(item =>
            <GR item={item} active={powerSavingValue === item.value} extraClass="mouse-power-saving" />)}</div>
        : <slider min={1} max={15} step={1} value={powerSavingValue} active minTag="1" maxTag="15" />}
    </widget>;
  }
}
```

别名经同一模块的导出表（与第 6 轮的 `ISS`/`CUC` 同表）解析：

| 别名 | 语言键 | 英文值 |
| --- | --- | --- |
| `ra.bSO` | `POWER_SAVING_HEADER` | WIRELESS POWER SAVING |
| `ra.RKF` | `POWER_SAVING_TOOLTIP` | Set how long the device should be idle before it enters sleep mode. |
| `ra.YCM` | `POWER_SAVING_DESC_V4` | Enter sleep mode if idle for |
| `ra.rGE` | `POWER_SAVING_DESC` | Enter sleep mode if idle for (minutes) |
| `ra.pHP` | `MIN` | `{{value}} min.` |
| `ra.yvH` | `SEC` | `{{value}} sec.` |

`value >= 60` 时才走 `MIN` 并把数值除以 60、否则走 `SEC`，说明该组件按**秒**判断取值
（列表里的数值即秒）。本地据此新增纯函数 `power_saving_label(value)`（3 条测试），
并用 `i18n::t_value(key, value)` 替换 `{{value}}` 占位符（10 份语言包都保留占位符）。
原先本地自造的 `"{value} MINUTES"` 标签已删除，手柄功耗页改用共享组件标题
`POWER_SAVING_HEADER`、帮助 `POWER_SAVING_TOOLTIP`、说明改用控制器自己的
`CONTROLLER_POWER_SAVING_DESC`（`Zae`，德语 "Das Gerät schaltet sich nach (Min)
Inaktivität aus."）。

| 选择器（`.ref/devices/2629/static/css/main.9f807a49.css`） | 源码声明 | 本地 |
| --- | --- | --- |
| `.powerSaving-select` | `display:flex;gap:10px;margin-top:20px` | `h_flex().gap_2()`（10px = `gap_2` 主题间距） |
| `.powerSaving-value` | `background:#111;border:1px solid #5d5d5d;border-radius:3px;color:#fff;font-size:12px;line-height:12px;min-width:90px;padding:7px 30px` | 由共享 `choice` 按钮承担（沿用第 5 轮已核对的按钮样式） |
| `.powerSaving-value:hover` | `border:1px solid #44d62c;cursor:pointer` | 同上 |
| `.powerSaving-value.active` | `background:#292929;border-color:#44d62c` | 同上（选中态） |
| `.powerSaving-desc` | `margin-top:20px` | `surface::note` 说明位于按钮组上方（与 `h1-body` 同序） |

手柄灯光页同时改回源码的「关闭灯光」组件（第 6 轮在键盘/鼠标上落地的那一个）：
面板标题 `SWITCH_OFF_LIGHTING_HEADER` + 右上帮助 `SWITCH_OFF_LIGHTING_TOOLTIP`，
两个 `.check-item` 用 `DISPLAY_TURNED_OFF`/`IDLE_FOR_MIN`、`.has-slider` 几何
（`margin-left:30px;width:490px`）与 1–15 滑条和 `1`/`15` 灰标；亮度组件改为
`panel_with_title_switch` + `BRIGHTNESS_TOOLTIP` + `0`/`100` 灰标。为此
`lighting`/`lighting_element` 与 `source_workspace` 的 Gamepad 分支都改为传入 `window`。

关联游戏弹层标题原先用 `t("LINKED_GAMES_TO")`——该键在**全部**当前源码里都不存在
（`python .work/search-app-key.py LINKED_GAMES_TO` → 0 个文件）。源码的对应文案键是
`LINKED_GAME_CHROMA_HEADER`＝"Games linked to profile:"（德语
"Mit Profil verknüpfte Spiele:"），冒号后接 Profile 名，本地按此拼接。

最后，`ADVANCED_EFFECT_DETAILS` 的缺口不是键名错，而是它只被设备包引用
（`oi("ADVANCED_EFFECT_DETAILS")`，`.ref/devices/769/static/js/main.ad1113f8.js`），
应用级提取工具没看到它。新增 `tools/prepare-device-locales.py`：用 JS 感知的括号扫描
（含 `[$o]:{` 这类计算键与 `\xfc` 这类 JS 转义）从设备包的语言表里取出该键的 10 种语言
文本并合并进 `locales/`，`--check` 现在报告「10 份语言包均存在」，`--self-test` 固定
提取、转义与计算键三种情况。语言包键审计的待回溯项因此从 5 → 3 → **0**。

## 手柄扳机页：`JP` 与 `QP` 双柄滑条（第 7 轮）

`.ref/devices/2629/static/js/main.000315e2.js`（Wolverine V3 TE）里，扳机页组件是 `JP`，
模拟模式用 `QP` 这个**双柄**滑条，而不是两个独立滑条：

```jsx
<widget title={LEFT|RIGHT_TRIGGER_MODE} tips={we.SdZ}>
  <div className="radioList">            // .radioList{display:grid}
    <check-item name={ANALOG}  id={TriggerType+"-analog"}  extraClass="radioItem" />
    <check-item name={DIGITAL} id={TriggerType+"-digital"} extraClass="radioItem" />
  </div>
  // 模拟分支：.h1-body = LEFT|RIGHT_TRIGGER_RANGE，reset-actuation，margin-top:35px → QP
  // 数字分支：.h1-body = LEFT|RIGHT_ACTUATION_POINT，reset-actuation，margin-top:20px
  //           <slider min={1} max={100} step={1} minTag="1%" maxTag="100%" tipFormat={v=>v+"%"} />
</widget>
```

`QP` 的结构与样式（逐条比对）：

| 选择器（`.ref/devices/2629/static/css/main.9f807a49.css`） | 源码声明 | 本地 |
| --- | --- | --- |
| `.rangeSlider` | `height:40px;margin:0 -12px -8px 10px;position:relative` | 相对容器 + `ml(10)/mr(-12)/mb(-8)/h(40)` |
| `.rangeSliderBackground` | `background:#44d62c;height:6px;margin-left:-10px;margin-right:12px;position:absolute;top:18px` + `opacity:.3` | `#44d62c4d` 6px 圆角 5px，`left(-10)/right(12)/top(18)` |
| `.rangeSliderHighlight` | 同上但不透明，行内 `left:start%`、`right:(100-end)%` | 同值，按当前取值定位 |
| `.rangeSlider input[type=range]::-webkit-slider-thumb` | `background:#44d62c;border-radius:50%;height:20px;width:20px`；`:hover{background:#5d5d5d;border:2px solid #44d62c}`；`:active{background:#383838;border:2px solid #44d62c}` | 20px 圆柄；悬停同值；按下态由拖拽状态驱动（GPUI 的 `Div` 没有 `:active`），颜色与描边同值 |
| `.sliderTipBar` / `.sliderTipBar div` | `position:relative;width:96%` / `background:#44d62c;border-radius:3px;color:#000;font-size:12px;padding:4px 9px;position:absolute;top:-16px;transform:translateX(-50%)` | 96% 宽相对条 + 两个绿色数值气泡，用「零宽容器 + `justify_center`」复刻 `translateX(-50%)` |
| 越界规则 | `onChange` 里 `Math.min(value, max-1)` / `Math.max(value, min+1)`（`onMouseUp` 才提交） | `range_handle_value(handle, value, start, end)`（纯函数，3 条测试固定 0–99 / 1–100 与相邻档位不越界） |
| 下方刻度 | `<div style="display:flex;justify-content:space-between"><span>0</span><span>100</span></div>` | `0`/`100` 两端对齐（字面量，不是语言键） |
| `.reset-actuation` | `font-size:14px;position:absolute;right:35px;text-decoration:underline;text-transform:capitalize`；`:before{background:#ccc;mask-image:url(icon_reset.f416d0b7.svg)}`；`:hover{color:#44d62c}`；调用方在未偏离默认时加 `.disabled` | 下划线链接、`#ccc`→悬停 `#44d62c`、未偏离默认时 `opacity(0.3)` 且不响应；**图标缺失**（该包只抓了 `js/`、`css/`，`.ref` 里没有任何 `icon_reset*`），因此只渲染文字，未画占位图形 |

本轮同时删掉了本地自造的 `t("MINIMUM")` / `t("MAXIMUM")` 标签（当前源码里没有这两个键），
改为源码的 `.h1-body` 文案键，并把面板标题从 `LEFT_TRIGGER`/`RIGHT_TRIGGER` 改成源码实际使用的
`LEFT_TRIGGER_MODE`/`RIGHT_TRIGGER_MODE`；`ACTUATION_DESC` 说明与
`minFWSupportTriggers` 固件提示条件分支在重写中一度被删掉，已按原样恢复（同一函数里）。

仍未接入：`.radioList` 里的两个模式项本地仍是按钮（源码是 `check-item` + `radioItem`，
`.radioItem{margin:0}`）、扳机 SVG 可视化与手柄测试器、以及 `icon_reset` 图标。

## `.check-item` / `.check-box` 与共享控件（第 6 轮）

第 14 行原先只做到「外包一层 `mb(9px)`」；本轮把勾选项做成真正的 `.check-box`，并把三处各自近似的
实现收敛到 `src/ui/surface.rs`（依据：182/100 等设备包的当前 CSS 与 `cn.A` 组件源码）：

| 选择器（`.ref/devices/100/static/css`） | 源码声明 | 本地 |
| --- | --- | --- |
| `.check-item` | `margin-bottom:9px;transition:opacity .3s`；`.disabled{opacity:.3;pointer-events:none}` | `surface::check_item`（`BaseButton`，`mb(9px)`，禁用时 `opacity(0.3)` + `disabled(true)`） |
| `.check-box` | `background-color:#0000;border:1px solid #737373;border-radius:2.4px;box-sizing:border-box;height:20px;margin-right:10px;width:20px;transition:border-color .3s ease`；`:hover{border-color:#44d62c}`；`.checked{background-color:#44d62c;border-color:#44d62c}` | 20×20、圆角 2.4、`#737373` 描边 + `#111` 底，悬停/选中描边 `#44d62c`、选中底 `#44d62c` |
| `.check-box:before/:after` | `width:3px;background:#111`；`:before{left:8.6px;top:16.4px;transform:rotate(-145deg)}` + `animation:ticktop .2s ease`，`:after{left:.8px;top:10.2px;transform:rotate(-50deg)}` + `animation:tickbottom .1s ease` | `check_tick()` 用 `canvas` 画两条 3px 斜线，长度按 `Presence` 的 200ms/100ms `Ease` 过渡插值（与 Armory 菜单里同一套勾选动画） |
| `.check-text` | `color:#ccc;display:block;font-size:14px;left:30px;line-height:17px;text-align:left;top:2px`；`:first-letter{text-transform:uppercase}` | `#ccc`、14px/17px、相对方框 `left(30px)`/`top(2px)`，首字母大写 |
| `.widget .help` | 14px 圆形、`#4a4a4a`、悬停 `#ffffff4d`、`transition:background-color .3s` | `surface::help_control`（从配件模块上移，键盘/鼠标页复用；悬停淡入淡出与 `.widget .tip` 几何不变） |
| `.foot` 灰标 | `.min{left:0}`、`.mid{left:0;width:100%;text-align:center}`、`.mid1{width:66%}`、`.mid2{width:133%}`、`.max{right:0}` | `surface::slider_tags(min, mid, max, boost)`（从配件 `slider_row` 上移，供各页复用） |
| `.widget-switch` | 位于 `.titleRow > .title` 内、紧跟标题文本 | `surface::panel_with_title_switch(title, title_control, corner_control, cx)`（第 5 轮新增，鼠标亮度组件已改用） |

### 「关闭灯光」组件（键盘 / 鼠标）

本地此前把两个勾选项塞进亮度面板，文案键还是自造的
`SWITCH_OFF_LIGHTING_WHEN_DISPLAY_IS_OFF` / `SWITCH_OFF_LIGHTING_WHEN_IDLE` 与
`MINUTES`（三者都**不在**当前源码里，`t()` 未命中会把 key 原样显示）。源码
（`.ref/devices/100/static/js/main.1734869e.js`，组件 `kI`/`xI`）实际是**独立组件**：

```jsx
<widget title={SWITCH_OFF_LIGHTING_HEADER} tips={SWITCH_OFF_LIGHTING_TOOLTIP} extraClass="has-slider">
  <check-item id="checkDisplay" disabled={!brightnessOn} name={DISPLAY_TURNED_OFF} />
  {DeviceInfo.hideLightingIdle ? null : (
    <div>
      <check-item id="checkIdle" disabled={!brightnessOn} name={IDLE_FOR_MIN} />
      <slider min={1} max={15} step={1} minTag="1" maxTag="15" value={idleMinutes}
              active={isIdleEnabled && brightnessOn} />
    </div>
  )}
</widget>
```

别名解析（`Rt.*` → 导出别名 → 英文字面量）：`mCP`→`wa`=`SWITCH_OFF_LIGHTING_HEADER`、
`cFD`→`za`=`SWITCH_OFF_LIGHTING_TOOLTIP`、`ISS`→`ka`=`DISPLAY_TURNED_OFF`、
`CUC`→`xa`=`IDLE_FOR_MIN`；亮度组件 `WI` 是 `title:Rt.UEk`(→`ya`=`BRIGHTNESS_HEADER`)、
`tips:Rt.WpG`(→`ba`=`BRIGHTNESS_TOOLTIP`)、`hasSwitch:true`、滑条
`minTag:"0"`/`maxTag:"100"`、`title:"BRIGHTNESS_HEADER"`。页面 `ql` 的左 `.widget-col`
是 `[亮度, 关闭灯光]`、右列是效果组件。本地已按此重写键盘与鼠标两页：
`t("DISPLAY_TURNED_OFF")` / `t("IDLE_FOR_MIN")`、1–15 滑条（灰标 `1`/`15`、无标签、
`.has-slider .slider-container{margin-left:30px;width:490px}` 的 30px 缩进与 490px 宽）、
亮度面板改为标题行开关。`IDLE_FOR_MIN` 的英文值本身含 “(minutes)”，所以不再需要单独的
`MINUTES` 文案。

鼠标页仍有**未回溯的中文字面量**（`"智能追踪"`、`"非对称中止"`、`"抬升距离"`、`"着陆距离"`、
`"表面校准"`、`"配对"` 等，见 `mouse_products.rs` 的 calibration/pairing 分支）：逐个对应源码
key 的核对未完成，本轮不动它们，已在缺口清单登记。

## 对账方法（后续每页复用）

1. 从产品包 CSS 抽出该页类名的**原始声明**（工具已能按选择器精确取规则）；
2. 在本地找到对应渲染代码，逐声明比对；不一致就改本地，或把源码分支记录成「未实现 + 触发条件」；
3. 把结论写进本目录的 JSON + Markdown，工具 `--check` 保证不回退。

## 2026-10-06 组件帮助提示：`.tip body-widget-tip-portal` 与 `positionTip`

外壳（3858 的 `KrA`、3880 的 `_TA`，其余产品同构）把帮助提示渲染成
**挂到 `document.body` 的 portal**：

```jsx
<div className="help" ref={this.helpRef} role="button" aria-label={text}
     onMouseOver={this.onHover} onMouseLeave={this.onLeave}/>
{this.state.showTip && createPortal(
  <div className="tip body-widget-tip-portal" ref={this.tipDiv}>{text}</div>, document.body)}
```

`.tip{…opacity:0;position:absolute;right:14px;top:34px;transition:visibility 0s,opacity .3s linear;
visibility:hidden;white-space:pre-wrap;max-width:300px;padding:8px 10px;line-height:18px}`
之上，`.body-widget-tip-portal{opacity:1;position:fixed;right:auto;visibility:visible;z-index:10001}`
覆盖了淡入与定位——所以提示是**立即出现**（React 挂载/卸载），由 JS 的 `positionTip()`
给出坐标：

| 步骤 | 源码 | 本地 `shell_tip_position` |
|---|---|---|
| 默认锚点 | `widget.right - 14 - tipWidth`、`widget.top + 34` | 换算到 `.help` 控件：`help.right - 4 - tipWidth`、`help.top + 24`（控件在 `right:10px;top:10px`） |
| 右溢出 | `container.right - 14 - tipWidth` | `viewport.width - 14 - tipWidth` |
| 左溢出 | `container.left + 14` | `14`（本地不测量容器左边） |
| 下溢出 | 翻到图标右侧：`help.right + 8`、`help.top` | 同 |
| 仍右溢出 | `help.left - 8 - tipWidth` | 同 |
| 仍下溢出 | `o -= s.bottom - n.bottom + 10` | `viewport.height - tipHeight - 10` |

`container` 在源码里是 `.main-container > #body-wrapper`；本地 Rust 层没有测量该元素，因此用
窗口视口代替（右/下边界一致，左边界取 0，顶边源算法不使用），这条近似与既有的 179
`receiver_help_control` 路径相同，已写进证据的 `approximation` 字段。本轮把
`SourceTooltipKind::WidgetTip` 从「300ms 淡入 + priority 200 + 只做静态锚点」改为与
`ReceiverWidgetPortal` 相同的 portal 语义（0ms 挂载、`z-index:10001`、上述溢出回退），
因为两者在源码里本来就是同一个 `.widget .help + .tip` portal。

`tools/audit-widget-tip-position.cjs --check` 逐条断言两个外壳里的 `positionTip` 片段、本地
`shell_tip_position` 的常量与两个 kind 的路由，并统计覆盖广度：**331 个当前设备包中 299 个
（271 个在 main bundle、28 个在懒加载 chunk）都挂载这个 portal**，逐包清单见
[widget-tip-position-current-evidence.json](widget-tip-position-current-evidence.json)。

## 2026-10-06 外壳的三个条件块：谁真的用到

读完 `_TA`/`KrA` 的 `render()` 后可以确认，外壳还渲染三个由 prop 驱动的条件块
（全部在 `.widget` 之外、`.widget-container` 之内或标题行内）：

| prop | 渲染 | CSS | 当前源里真正传值的产品 |
|---|---|---|---|
| `centerTips` | `{centerTips && (class 含 "disabled" \|\| false===active) && <p class="widget-body-tooltip">}` | `.widget-container{display:flex;position:relative}`、`.widget-container:hover .widget-body-tooltip{opacity:1;visibility:visible}`、`.widget-body-tooltip{…#000/1px #5d5d5d/#ccc 14px/16px、left:50%、top:50%、transform:translate(-50%,-50%)、opacity 0→1 .3s linear}` | 207（电池优化组件：`centerTips:R?Ve.tI6:""`）、717 |
| `disableSwitchTips` | `{r && R && <p class="widget-switch-tooltip">}`（开关被 `disable` 时显示） | `.widget-switch{forced-color-adjust:none;position:relative}`、`.widget-switch:hover .widget-switch-tooltip{opacity:1;visibility:visible}`、`.widget-switch-tooltip{…#000/1px/#ccc 14px/16px、left:10px、top:calc(100% + 2px)、opacity 0→1 .3s}` | 580（灵敏度组件） |
| `hasFWUpdate` + `fwUpdateComponent` | 由 `q7O.ju(fwUpdateComponent, DeviceInfo.featureMinFw, currentFWVersion) && hasFWUpdate` 得到 `G`；`G` 给 `.widget` 追加 `disabled` 并渲染 `<div class="warning-fw-update">` | `.warning-fw-update{#111/1px #fd8611/圆角 3/57px 高/300px 宽、left:50%、top:50%、transform:translate(-50%,-50%)}`、`.warning-fw-update-tip{#000/1px #5d5d5d/#ccc 14px/16px、max-width:300px、position:fixed、z-index:100}` | 1398/1401/1404/2638/2641/2644/4124（耳机，`fwUpdateComponent:"AudioPrompts"`） |

本地的面板层还没有这三个可选参数，而且**上述产品目前都没有本地页面**，所以本轮只把事实登记在
这里（连同各自的 CSS 与触发条件），等这些家族接入页面时再实现，避免先写出没有入口的假 UI；
`warning-fw-update` 还需要本地目前没有的固件版本数据（`DeviceInfo.featureMinFw`、组件的
`currentFWVersion`）。

## 2026-10-06 `.widget .help` 帮助控件（新增第 7 节断言）

当前源（`.ref/devices/100/static/css/main.dd229426.css`，331 个设备包同规则）：

```css
.widget .exclamation:before,.widget .help{background-repeat:no-repeat;border-radius:7px;height:14px;position:absolute;width:14px}
.widget .help{background-color:#4a4a4a;background-image:url(…/tooltip_questionmark.96138d2f.svg);right:10px;top:10px;transition:background-color .3s;will-change:background-color}
.widget .help:hover{background-color:#ffffff4d}
.widget .help:hover+.tip{opacity:1;visibility:visible;z-index:100}
.body-widget-tip-portal,.widget .tip{background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;line-height:18px;max-width:300px;opacity:0;padding:8px 10px;position:absolute;right:14px;text-align:left;text-transform:none;top:3…}
.body-widget-tip-portal{opacity:1;position:fixed;right:auto;visibility:visible;z-index:10001}
```

`tools/audit-device-page-css.cjs` 现在逐条断言上面这些声明、共享实现 `surface::help_control`
的存在，以及 `src/features/keyboard_controls.rs`（Snap Tap 帮助）与 `src/features/device_pages.rs`
（非对称中止说明）都改用该共享控件（不再自绘 `help-default.svg` 按钮 + Kit tooltip）。
`docs/re/device-page-css-audit.json` 新增 `widget_help` 收据（源码声明 + 本地指纹），`--check` 通过。

说明：源把 `.help` 绝对定位在 widget 的 `right:10px;top:10px`，本地沿用各 widget 的标题行右端
（`surface::panel_with_control` 的控件位）承载同一个 14px 控件，尺寸、配色、图标与 `.tip` 皮肤均与源
一致；这一放置差异已在此登记，未当作等价处理。

## 2026-10-06 效果参数的方向/屏幕标签改用已核对文案键

`src/features/device_pages.rs` 的效果方向与屏幕按钮此前用字面量中文标签。按
`locales/zh-CN.json` 的**唯一值命中**核对（并用 `locales/en.json` 复核语义）：

| 本地字面量 | 唯一命中文案键 | en 值 |
| --- | --- | --- |
| 顺时针 | `CLOCKWISE` | Clockwise |
| 逆时针 | `COUNTER_CLOCKWISE` | Counter-Clockwise |
| 顶部 | `TOP` | Top |
| 底部 | `BOTTOM` | Bottom |

这四处改为 `crate::i18n::t(...)`；其余（整个屏幕 / 左侧 / 右侧 / 向左 / 向右 / 向外 / 向内）
在本地文案表里没有唯一命中的键，**保留原字面量**并在代码里注明，未按近似键替换。
文案键存在性由 `tools/audit-locale-keys.py --check` 覆盖。

## 2026-10-06 灯效 widget 的帮助控件（源有条件文案）

源 100 `main.1734869e.js`：灯效 widget 不是自定义外框，而是共享组件 `cs`：

```jsx
<div className={this.props.nanoLeafEnabled ? "" : "disabled"} style={{flex:"initial"}}>
  <cs title={t ? Rt.t69 : Rt.tQN} tips={t ? Rt.uTU : i ? Rt.px6 : Rt.Zsw}
      extraClass={this.props.extraClass}>
    {this.renderEffect()}{this.renderChromaAppMessage()}
  </cs>
</div>
```

用 `tools/webpack-source.cjs` 的**模块作用域**解析（不是文本猜测）得到导出映射与文案键：

| 导出 | 文案键 | zh-CN |
| --- | --- | --- |
| `Rt.t69` | `CUSTOMIZE_SENSA` | 自定义 Sensa HD 触觉反馈功能 |
| `Rt.tQN` | `EFFECTS` | 效果 |
| `Rt.uTU` | `SENSA_HD_TOOLTIP` | 自定义您的设备如何响应不同类型的触发器。 |
| `Rt.px6` | `EFFECTS_BLE_TOOLTIP` | 从预设列表中选用一种效果来自定义设备的灯光效果。使用蓝牙连接时可用的效果有限。 |
| `Rt.Zsw` | `EFFECTS_TOOLTIP` | 从预设列表中自定义设备的灯光效果，并将其同步到支持所选灯光效果的其他 Razer Chroma 雷蛇幻彩设备。 |

因此该 widget 的帮助文案是**有条件**的：Sensa HD 设备用 `SENSA_HD_TOOLTIP`，蓝牙连接用
`EFFECTS_BLE_TOOLTIP`，其余用 `EFFECTS_TOOLTIP`。本地 `src/features/device_pages.rs` 现在按
`.widget .help` + `.widget .tip`（`surface::help_control("lighting-effects-help", …)`）接入，
并按 `self.device().use_ble` 在 `EFFECTS_BLE_TOOLTIP`／`EFFECTS_TOOLTIP` 之间切换；Sensa HD 分支
没有对应的本地设备标志，代码注释已写明未建模，未臆造该标志。
`tools/audit-device-page-css.cjs` 新增第 8 节断言（`title`/`tips` 结构、四个键的存在、本地接线）
与 `lighting_widget` 收据。

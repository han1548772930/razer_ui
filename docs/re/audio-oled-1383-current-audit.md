# 1383 Kraken V4 Pro OLED 页当前源复核

2026-10-06。产品 1383（Razer Kraken V4 Pro）的 `TAB_OLED` 页由自己的 OLED 根渲染，之前本地描述符把它
按通用音频控件（一个滑条 + 三个下拉框）挂载。本轮按当前源把这一页改成源码的形态。

来源与逐条标记（含文件 SHA-256、根/控件 UTF-16 code-unit 区间、语言符号解析、CSS 规则收据、本地文件指纹）见
[机器可读收据](audio-oled-1383-current-evidence.json)，由 `node tools/audit-audio-oled-1383.cjs`
生成/校验（`--check` 与工作区当前状态比对）。

## 当前源事实

`.ref/devices/1383/static/js/6141.5d00192e.chunk.js` 的 `xx`（约 661194–662123）：

- 头戴式耳机不在 dongle 上时先挂一个 `type:"warning"` 的 `WarningAlert`（117px 高、400px 宽、无底栏），
  标题/正文是 `OLED_NOT_SUPPORTED_WIRED_WARNING_TITLE` / `_DESC`（`V.QAd` / `V.l2t`）。
- 然后是两层 `body-widgets`（`.body-widgets{display:flex;flex-wrap:wrap;justify-content:center;
  margin:auto;max-width:1240px}`），内层按列挂五个 `.widget`：
  - `widget-col col-left`：亮度 `Nv`（调用点 `min:30,minTag:30`）、语言 `Bv`；
  - `widget-col col-right`：回主屏时间 `Ov`、息屏变暗 `Uv`、屏保 `Kv`。
- 内层五个控件前还挂 `Dv`（Home Screen Display 卡片网格，`extraClass=HomeScreenDisplay_body__ZOBui`）。
  当前 reducer 自带初始动画、图片及示例预览，不必等待 `GET_OLED_DATA` 才能恢复初始卡片。
  七卡、资源、响应式宽度及编辑器的独立复核见 [audio-oled-home-native.md](audio-oled-home-native.md)。

各控件的当前源形态：

| 控件 | 源码 | CSS |
| --- | --- | --- |
| 亮度 | `uo.A`，`min:30 max:100 step:1 minTag:30 maxTag:100`，`title:"BRIGHTNESS_HEADER"` | `.slider-container{height:64px;opacity:.3;transition:opacity .3s}`、`.brightness{pointer-events:auto}`、`.track{bottom:25px;height:6px;background:#44d62c4d;border-radius:3px}`、填充 `calc(8px + n*(100% - 16px))`、`.slider-tip{bottom:42px;background:#44d62c;border-radius:3px;padding:4px 8px}`、`.foot{bottom:-2px;color:#999;font-size:14px}` |
| 语言 | `ut.A` 下拉 + `mm.A` APPLY 按钮，`extraClass="OLEDLanguage_button …"`，未改动的暂存值加 `OLEDLanguage_disabled` | `.OLEDLanguage_dropdown{display:flex;margin:10px 0 20px;z-index:105}`、`.OLEDLanguage_button{background-color:#44d62c;color:#000}`、`.OLEDLanguage_disabled{opacity:.3;pointer-events:none}`、`.OLEDLanguage_desc{color:#999}` |
| 回主屏时间 | `.polling-btn-set`（`margin-top:22px`）+ `OLED_TIME_TO_HOME_SCREEN_VALUES`＝5/10/15/20 | `.customize-polling-rate-button{background:#222;border:1px solid #5d5d5d;border-radius:3px;color:#ccc;font-size:14px;height:27px;text-transform:uppercase}`、`:hover/.active{border-color:#44d62c}`、`.keyboard-btn-size{height:27px;padding:13px 15px}` + `DimKeyboardLighting_width-auto{width:48px!important}` |
| 息屏变暗 | 同上（`margin-top:20px`），选项 `OLED_DIM_DISPLAY_VALUES`＝1/2/3/4；禁用时叠一层 `DimKeyboardLighting_backdrop` | `#111`、300×30、`opacity:.5`、`position:absolute` |
| 屏保 | `OLEDScreensaver_oled-screensaver-options` 两列网格，四个 260×68 磁贴：第 0 个是文字磁贴 `No screensaver`，1–3 是三张 GIF | `.OLEDScreensaver_oled-screensaver-options{grid-gap:10px;display:grid;grid-template-columns:1fr 1fr;margin-top:20px}`、`.…-option{background:#000;border:1px solid #5d5d5d;color:#999;font-size:14px;height:68px;width:260px}`、`:hover{border:2px solid #166809}`、`selected{border:2px solid #44d62c;pointer-events:none}` |

`SET_OLED_SCREENSAVER` 存的是 `enabled ? value + 1 : 0`，而高亮比较的是 `t === i.id`，两者正好对齐：
磁贴 id 就等于落库值（0＝无屏保、1/2/3＝三张 GIF），所以本地描述符用 `value:0..3` 表达，不需要额外映射。

## 本轮本地实现

- 新增 `src/features/audio_oled.rs`：按当前源的列归属渲染两列，五个分区各自有自己的形态
  （亮度滑条、语言暂存 + APPLY、两排 48×27 按钮、260×68 屏保磁贴 + 两列网格），帮助按钮仍固定在
  `.widget` 右上 10/10。
- `src/features/audio_products.rs`：新增 `tips`、`apply_label`、`image` 字段与 `staged` 暂存表；
  带 `apply_label` 的选择行只暂存、按 APPLY 才提交；1383 的 `TAB_OLED` 交给新模块渲染；
  `options`/`image_options` 与 `select` 一样参与恢复时的取值校验。
- `tools/prepare-audio-products.cjs`：`TAB_OLED` 分支改成从当前源取证——分区标题/帮助取自证据里的
  `ho.A` 属性、亮度区间取根调用点 `min:30,minTag:30` 加组件默认 `max`、语言行带 `apply_label`、
  两个延迟/变暗行进 `options`、屏保进 `image_options` 并把 `src:Fv/$v/Zv` 符号解析成
  `static/media/*-random-sim-*.gif` 再映射到已备好的 `synapse/oled-screensaver-{1,2,3}.webp`。
- `tools/validate-audio-products.cjs`：新增 `options`/`image_options` 的取值断言，并要求预览图必须是
  本地 `synapse/…` 资源而不是源站地址。

## 明确未完成

- `Dv` 七卡和六个可编辑模式均已接入本地编辑器，后续范围见
  [编辑器续接记录](audio-oled-editors-2026-10-06.md)。
- 语言下载（`ux`/`cx` 进度弹层）、动画/图片上传、`OLED Not Configurable` 警告的 dongle 判定、
  以及设备更新流程仍属服务边界；本地不生成成功状态。
- 滑条外观已改由共享的 `src/ui/source_slider.rs`（`SourceSlider`）绘制，与 `STA`/`uo.A` 同一份
  源声明：`.slider{background:#0000;border-radius:3px;bottom:25px;height:6px}`、
  `.slider::-webkit-slider-thumb{background:#44d62c;border-radius:8px;height:16px;width:16px}`、
  `.on …:hover{background:#5d5d5d;border:2px solid #44d62c}`、`:active{background:#383838;…}`、
  `.track{background:#44d62c4d}`、`.left{background:#44d62c}`、`.slider-tip{bottom:42px}`。此前记录说
  “源的拇指是 `path.0086a00e.svg`、本地用 16px 圆点替代”**不准确**：那张 14×20 图属于
  `.slider-more::-webkit-slider-thumb`（另一种滑条元素），而 `.slider` 的拇指本来就是 16px
  `#44d62c`、`border-radius:8px` 的圆点，本地画的正是它。
- 未运行应用、构建或测试，因此没有实际窗口的像素、焦点、滚动验收结论。

此前五控件阶段验证：`node tools/prepare-audio-products.cjs`、`node tools/validate-audio-products.cjs`、
`node tools/audit-audio-oled-1383.cjs --check`、`python tools/validate-resources.py`、
`python tools/validate-embedded-json.py`、`python tools/audit-locale-keys.py --check`、
`cargo fmt --all -- --check`、`cargo check --locked --all-targets` 全部通过。
后续新增卡片和编辑器的验证与边界单独记录在上述 Home Screen 审计中。

# Kraken V4 Pro 1383 Home Screen Display

2026-10-06。只使用 `.ref/devices/1383/asset-manifest.json` 声明的当前 JS/CSS，以及当前官方
`.ref/host-4.0.827/` 字体。未用 691 或已删除的历史前端作为实现依据。

`tools/prepare-audio-oled-home.cjs` 用 Acorn 静态解析模块局部声明、导出和字面量，输出
`src/features/audio_oled_home_data.json` 及 [源码收据](audio-oled-home-source.json)。偏移单位为
UTF-16 code units；工具不加载或执行参考 JS。`tools/audit-audio-oled-home.cjs` 是同一当前产品的
只读 AST 定位器，可按模块/符号检查来源。

## 已接入

`xx → Dv → Hp/Bp` 的七卡顺序为 Animation(0)、Image(1)、Emote(4)、Banner(2)、
Audio Meter(5)、System Info(6)、Headset Info(3)。`79826:T` 本身提供可渲染的初值，
此前“必须取得设备 GET_OLED_DATA 才能还原卡片”的判断不准确。

- 源 `.widget` 的 max-width/min-width 共同决定桌面卡区宽度 1220px；在 `max-width:1279px`
  媒体查询下为 600px、左右 padding25px。卡片 wrapper236px，title19px，content232×64px；
  普通状态外框1px/margin1，选中或 hover 为2px/margin0。hover 的半透明绿优先于 selected。
- Bp 的黑色80%遮罩与 EDIT/APPLY 保留24px图标、四周8px间距、12/14px文字。
  hover owner 同时包含预览与 overlay，避免 overlay 遮住预览后反复触发离开；标题不激活 overlay。
  Home 开关关闭后原位叠 `#111`/80%遮罩。
- 七卡显示源初始动画/图片/表情/横幅/音频图及状态预览。System Info 按源3秒间隔轮换三组
  示例（CPU、GPU、日期/时间），数字和日期逐字取自 reducer；这不是实时系统采样。
  Headset 音量绑定本地音量草稿，电量0来自当前 batteryReducer 初值，也不是硬件报告。
- Banner 使用原 RazerF5 20px 字体、232×20px图、44px文字区，按实际字体测量长度计算
  `3400 + 28 × scrollWidth` 毫秒周期；保留0/49/50/51/52/100%位移及透明度关键帧。
- Media 的 EDIT 直接打开当前 `Mv → Kg` 编辑器。它保留独立草稿、即时预览、Reset、
  Cancel、Apply；两个开关按原逻辑互斥地保证至少有一个内容区域开启。
  Top/Bottom 单选、三列可视化图、禁用遮罩、所选边框及原音轨占位文本均来自 Kg/CSS。
  只有 Apply 将该编辑草稿存入本地 profile 快照；Cancel/关闭丢弃编辑。
- Media 弹层使用当前 Tn/choose-a-mat/popup-widget 的层级、100px top +110px margin、
  36px标题、35px底部操作区。min1400px窗口采用850px宽度，其余采用800px最小宽度。
  背景100ms线性淡入，top300ms CSS ease移动，关闭按钮背景200ms CSS ease，
  单选圆点200ms CSS ease。Base Dialog 提供焦点保存/恢复与键盘模态范围。
- 需要 Synapse 图标和提示绑定当前58837:p/h：15px图标、300px提示体、原定位偏移及100ms淡入。
- 后续五个编辑器已接到同一 EDIT 入口：动画/图片的固定预设槽与本地裁剪、104个表情、
  横幅字体/图片/四方向滚动，以及三张系统信息幻灯片。分支细节见
  [编辑器续接记录](audio-oled-editors-2026-10-06.md)。所有编辑器各自保留草稿，Cancel/关闭丢弃，
  Apply 才提交到本地 profile；编辑器种类用保留的 `AnyView` 统一挂载。
- 公共弹层迁到 `audio_oled_dialog.rs`，保留原焦点生命周期与所有入场时序；根据 Mv，
  动画/图片/表情固定800px，其余分支最大850px。标题下沿使用源码1px阴影，不占用边框空间。
- 需要 Synapse 提示迁到独立 portal，按源58837:h先检查右边界再检查左边界，保留8px距离。
  不增加源码不存在的纵向翻转；尺寸基于完整300px main，而不是较窄的文字wrapper。

全部颜色集中于 `audio_oled_home_theme.rs`。RazerF5 SemiBold600根据1383 main.css的
`@font-face`补入真实字体，不用400/700合成600；字体转换保留字形、cmap、布局及advance表。

## 资源与校验

`tools/prepare-audio-oled-home-assets.py` 准备27个独立资源：6个15fps动画、10张图片、
默认表情/横幅、3个visualizer、3个headset PNG、3个SVG。15fps分支来自46472导出的
`OLED_ANIMATION_FPS=15`；表情由8816:i的当前lazy声明定位76418模块中的GIF字面量。

动画转WebP保留源时长/循环并逐时间区间比较解码RGBA；grayscale100%按sRGB亮度转换，
保留alpha，不拉伸或重绘原始位图。SVG只序列化原React字面量树。
当前1383的关闭图标与共享mapping-close字节一致，见 [共享资源校验](audio-oled-home-shared-assets.json)。
另外三张已有屏保图已补取1383自身manifest声明的原GIF，与准备WebP时使用的源GIF逐字节比较，
见 [屏保来源校验](audio-oled-screensaver-assets.json)。GIF与WebP分别是源文件与转换输出，
不把仅文件名相同或不同编码格式说成字节相同。
独立资源表为 `assets/synapse/audio-oled-home-embedded.rs`，由统一资源加载器加载；
`assets/synapse/audio-oled-home-assets.json` 保存源/output SHA-256、尺寸、动画时长。
内嵌位图同时记录完整JS的source hash与解码字节的inline hash。

源码生成的 `--check`、资源准备的 `--check`、统一资源校验和Rust格式化通过。
最终 `cargo check --locked --all-targets` 由主任务统一运行；该项以主任务最后报告为准。
没有运行应用、构建、测试、安装器、下载JS或DLL，因此没有运行时像素验收结论。

## 仍未完成

卡片Apply表示本地预览选择。原System Info提交要经过SET_OLED_DISPLAY_SYSTEM_INFO与服务回写，
本地不声称完成该服务事务。

BLE条件、dongle warning、GET_OLED_DATA刷新、loading/error/retry、语言下载、原worker编码与
设备更新未接真实服务。本地文件选择/裁剪预览不表示设备上传完成。
默认卡片采用明确的源初值；未来服务状态接入后需补对应条件分支。
四种横幅方向及提示的8px碰撞重排已经接入。Media没有额外虚构关闭动画，
源Mv在关闭时直接卸载。

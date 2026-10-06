# 1383 OLED Banner 原生编辑器

本页只依据 `.ref/devices/1383/asset-manifest.json` 当前声明的 JS/CSS，以及当前官方 4.0.827 宿主包。未运行应用、安装器、测试、构建、厂商 JavaScript 或 DLL。

源码位置为 `51278/Ug`（编辑器）、`Hg`（预览）、`Bg`（CSS 模块映射）、`Mv`（850px 弹层选择），`9483`（图片、字体、字号）和 `79826/T/I`（初始状态、Banner reducer）。完整文件哈希、UTF-16 摘录范围、SVG 字面量、CSS 与关键帧见 [源码收据](audio-oled-banner-current-evidence.json)，独立静态验证见 [审计](audio-oled-banner-static-audit.json)。

实现文件为 `src/features/audio_oled_banner.rs`、独立数据 JSON 和主题文件。提供 8 张原版 Banner 图片、图片开关及上/下位置、64 个原版字体选项、14 个字号、字号增减、粗体/斜体/下划线、四向滚动、暂停、100ms 后重播、恢复默认、取消与应用。图片按源 CSS `grayscale(100%)` 转换并保留 alpha；11 枚图标从当前 React SVG 字面量静态序列化。19 项资源使用独立 embedded 表，未改写共享 manifest。

文字限制按源码的实际尺寸判断：水平 `offsetWidth > 920`；垂直在图片开启时 `offsetHeight > 176`、关闭时 `> 256`。超限按末尾 UTF-16 单元删减，字符计数也使用 UTF-16。若删减留下孤立代理单元，Rust 字符串保存其显示用的 U+FFFD；未声称能够保留 JavaScript 的无效 UTF-16 原始编码。

水平滚动使用 `3400 + 28 × scrollWidth` 毫秒，垂直为 5000 毫秒。四个关键帧保留 49%–52% 的透明度变化和 50%–51% 的位置跳转；竖向位移还包含源码的 5px 间距。左右用单行预格式文本并将换行替换为空格，上下保留原换行且不自动折行。

`Hg` 仅在 `text.value` 或 `text.scroll` 变化时重测水平时长。`local_duration_ms` 是明确的本地渲染缓存，零表示尚未首测；公开的 `measure_duration_ms` 供父层初始化和恢复使用。主页和编辑器是两个挂载实例：编辑器每次打开都重新测量；只改字体、字号或字形时保留该实例的缓存。Apply 若文字和方向均未变化，保留主页原有缓存；否则给主页按新内容测量。该缓存不冒充设备协议字段。

文本框的 391×104 外框不是猜测值。对当前 manifest 中全部作者 CSS 的 `box-sizing`、通配和 `textarea` 规则进行了枚举：`div { box-sizing:border-box }` 不匹配 textarea，且此属性不继承；仅 `.key-config .body textarea` 和 `.video-react *` 是可能覆盖 textarea 的规则，二者都不在 `Ug` 的挂载祖先中。因此其作者宽高 385×98 使用 content-box。当前官方包的 engine 字符串静态恢复出 Electron 41.2.0 / Chromium 146.0.7680.179；该 Chromium tag 的 `html.css` 声明 textarea padding 2px，加上作者 border 1px，得到上述外框。完整作者级联候选记录在源码收据的 `textareaCascade`，[浏览器版本与 UA 收据](current-browser-ua-evidence.json) 保留官方归档哈希、engine 哈希/字符串偏移和上游 URL。上游版本一致不等同于已证明所有编译后浏览器样式字节一致。

剩余边界：CSS 的 `line-height:normal` 使用当前原生字体的实际 ascent/descent，未进行被禁止的运行验证，因此不声称与 Chromium 的 line-gap、fallback 字体和原生输入绘制逐像素等价。原版 Apply 用 html2canvas 生成发送设备的 `imageData`；本地编辑器提交真实设置并清空旧位图，尚未实现设备位图编码或设备传输确认。本文不把这两项列为已完成。

验证入口：

- `node tools/prepare-audio-oled-banner.cjs --check`
- `.work/resource-env/Scripts/python.exe tools/prepare-audio-oled-banner-assets.py --check`
- `node tools/audit-audio-oled-banner.cjs --check`
- `python tools/extract-current-browser-ua.py --check`

Rust 格式化已完成；`cargo check --locked --all-targets` 由父任务统一执行。

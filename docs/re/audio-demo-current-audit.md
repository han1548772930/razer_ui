# 音频演示页（2026-10-03）

1392（CLIO）、1442（CLIO X）、3942（Marci）的 `TAB_DEMO` 此前均为空。本次沿实际导航重新解析挂载组件，加入 [audio_demo.rs](../../crates/razer-pages/src/features/audio_demo.rs) 的原生初始画面。完整组件树和来源指纹见 [audio-demo-current-evidence.json](audio-demo-current-evidence.json)，使用的默认值和 CSS 见 [audio_demo_data.json](../../crates/razer-pages/src/features/audio_demo_data.json)。

三者真实页面均为说明、800 × 450 的演示区，以及下方 800 宽的浮动播放复选框。封面引用 Webpack 模块 2222 的 `demo.ff6ba9d8.avif`；从三个当前产品地址获取并确认字节相同，转换为 PNG，保留 HTTP、源码及输出哈希。播放图形来自 3942 当前 JSX，白色由对应源 CSS 指定。来源收据为 [audio-demo-manifest.json](../../assets/synapse/audio-demo-manifest.json)。

浮动播放默认开启，属于 `floatingVideoReducer`，不在产品配置中。原生偏好由单独的保留实体持有，切换页签或配置不会重置，也不会使声音配置变脏。

原始播放器使用有声音轨的 `/synapse/assets/videos/audio_mode.mov`，包含播放/暂停、进度、全屏和浮动视频。现有教程组件只支持静音动画，不适合代替音频演示。当前点击封面后显示控制条并明确说明播放服务未接入，三个媒体控件禁用；没有下载/播放该视频，没有模拟音频、实时计时、浮动窗口或成功回执。这三页仍为 `partial_native`，原生媒体服务相关缺口完整保留。

提取器可用 `node tools/extract-audio-demo.cjs --check` 复核。通用解析器新增可选页面键参数，默认仍只解析 Help；这不代表历史 Help 记录已全部重新生成。资源准备只解码 AVIF 数据，不运行下载的 JavaScript。

## 当前外观核对

三款产品各自的挂载播放器显式设置 `disableDefaultControls:!0`，其 `children` 只有
`custom-play-toggle`、`custom-progress-bar` 和 `custom-fullscreen-toggle`。已删除此前自行加入的
音量、静音和百分比预览。Acorn 从每个实际挂载组件确认父节点和三个直接子节点，遇到改变即停止，
不是依据播放器库的默认控件推断。组件范围（UTF-16）分别为 1392 的 `[5712400,5713386)`、
1442 的 `[512621,513867)`、3942 的 `[304917,306150)`；完整原文见现有源码收据。

当前各产品最终 CSS 的 `.custom-control-bar` 指定 40px、零内边距、5px 圆角和 `#0003` 背景，
进度区为绝对定位且 `bottom:16px`。两个真实 button 同时包含 `video-react-button`：
`.video-react .video-react-button{font-size:inherit}` 的优先级 `(0,2,0)` 高于
`.custom-play-toggle/.custom-fullscreen-toggle{font-size:18px}` 的 `(0,1,0)`。
因此不能用 18px 计算 em；当前 `.video-react` 的有效字号为 10px，Player 的实际 `getStyle`
没有内联 fontSize。控件宽度 4em 与伪元素字号 1.8em 对应 **40px 和 18px**，
生成字段驱动 Rust，避免重新固定推断。背景定义位于 `AudioDemoColors`。

收据同时记录实际导入的 Player/PlayToggle/FullscreenToggle 模块、导出解析、完整函数及上述
CSS 规则。1392/1442 manifest 中虽存在 `1491` 视频通用样式，它只由异步资源 context 的
`"./scss/video-player.css":[21491,9,1491]` 声明。解析全部当前 manifest 声明脚本后，该 context
的六个真实请求均为产品图片 `/img_prods/prd-*`，没有 CSS 请求，也没有直接加载 1491 的调用；
3942 无此资源键。因此其中 `width:48px` 与背景 SVG 替代规则不参与当前 Demo 加载链。
若未来增加该 CSS 请求，提取器拒绝沿用现有尺寸，必须重新核对最终顺序及同优先级覆盖。
入口 HTML 先加载 main CSS；产品异步 CSS 由当前 runtime 向 head 追加。本文不把 manifest
中的所有 CSS 当成已加载样式。源码收据仍保留可选 CSS，作为静态清单而非有效级联。
控制条播放与全屏图形由三款当前 CSS 的同一内嵌 video-react WOFF 中 U+F200/U+F215
静态提取；资源收据保留各 CSS 哈希、font-face 偏移、字体哈希和 glyph 名称。
`tools/prepare-audio-demo.py --check` 校验资源与独立嵌入表，未执行字体或下载脚本。

源页面主体统一使用 `.body-wrapper` 的 `padding:10px 20px 20px/min-width:600px`；
已替换此前的 20px 全边内距和 800px 主体最小宽度。浮动播放复选框的真实挂载是
`check-item/check-box/check-text`，现复用项目对应源样式 helper，而非框架默认 Checkbox。
三款 `.check-box` 未选中背景为透明；本页通过 helper 的显式背景参数保留透明，
不再使用历史默认的 `#111` 底色，其他页面不受该页覆盖影响。
3942 的封面图最终 CSS 另有 `object-fit:cover`，1392/1442 不具有该覆盖；渲染依据生成字段分别
使用 Cover/Fill，没有通过产品编号猜测分支。

复选框未选中背景和下段勾线原点使用本页专用 `CheckItemStyle`：透明背景、`left:.8px/top:10.2px`。
不改其他页面 helper 的默认 `#111/(.6,10.)`；三款当前 `.check-box:after` 规则都由提取器确认。

进度条的浏览器行盒、悬停提示和 disabled 呈现、字体 glyph 的最终基线位置仍未运行比对，
不能把控件数量、控制条和封面修复算作完整视觉验收。官方 seek/play/pause/onTimeUpdate/循环播放
依赖真实媒体实例，完整 fullscreen 与 floating window 同样未接入；禁用媒体控件是明确的功能
缺口，媒体服务与设备设置写入属于独立链路。浮动偏好仍可编辑并保持独立实体状态。

## 播放服务边界

The three current audio demo roots mount a poster button for a real video with
an audio track. This workspace has no native media player transport, so the
poster cannot produce sound or report playback progress.

The poster action remains keyboard and pointer accessible. On an explicit click
it records a local request and shows `音频演示播放服务未接入`. The source-sized
control bar is present after that request. Its actual current mounted source
disables default controls and includes only play, progress and fullscreen; the
previous volume, mute and percentage preview were unsupported additions and
have been removed. The bar uses the source 40px height, zero padding, 5px radius
and translucent background. Its play/fullscreen outlines come from the current
product CSS's embedded font. Media controls remain disabled until an audible
transport is connected; they do not simulate time, playback or a successful
service response. The independent floating preference remains editable.

This is a real functionality gap: source play/pause/seek events, elapsed time,
duration, looping, fullscreen and the floating window require the media player.
Media playback and device-setting writes have separate transports and lifecycles. Detailed source receipts,
remaining progress geometry and runtime limits are recorded in
[the current audit](audio-demo-current-audit.md).

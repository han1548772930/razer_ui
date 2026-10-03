# 音频演示页（2026-10-03）

1392（CLIO）、1442（CLIO X）、3942（Marci）的 `TAB_DEMO` 此前均为空。本次沿实际导航重新解析挂载组件，加入 [audio_demo.rs](../../src/features/audio_demo.rs) 的原生初始画面。完整组件树和来源指纹见 [audio-demo-current-evidence.json](audio-demo-current-evidence.json)，使用的默认值和 CSS 见 [audio_demo_data.json](../../src/features/audio_demo_data.json)。

三者真实页面均为说明、800 × 450 的演示区，以及下方 800 宽的浮动播放复选框。封面引用 Webpack 模块 2222 的 `demo.ff6ba9d8.avif`；从三个当前产品地址获取并确认字节相同，转换为 PNG，保留 HTTP、源码及输出哈希。播放图形来自 3942 当前 JSX，白色由对应源 CSS 指定。来源收据为 [audio-demo-manifest.json](../../assets/synapse/audio-demo-manifest.json)。

浮动播放默认开启，属于 `floatingVideoReducer`，不在产品配置中。原生偏好由单独的保留实体持有，切换页签或配置不会重置，也不会使声音配置变脏。

原始播放器使用有声音轨的 `/synapse/assets/videos/audio_mode.mov`，包含播放/暂停、进度、全屏和浮动视频。现有教程组件只支持静音动画，不适合代替音频演示。因此当前播放按钮禁用并提供不可用说明；没有下载/播放该视频，没有模拟进度、浮动窗口或音效。这三页仅完成初始画面，仍为 `partial_native`，播放相关缺口完整保留。

提取器可用 `node tools/extract-audio-demo.cjs --check` 复核。通用解析器新增可选页面键参数，默认仍只解析 Help；这不代表历史 Help 记录已全部重新生成。资源准备只解码 AVIF 数据，不运行下载的 JavaScript。

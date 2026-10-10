# PID 1352 TAB_SOUND 音量卡片当前源码核对（2026-10-10）

对应完整页面：`local-ui-reverse/pages/synapse--products--1352--ui--TAB_SOUND--4712603.md`。该原文件追加写入被文件系统拒绝，本文件保留本轮局部审计。证据直接读取当前官方 JS/CSS，未执行 vendor JS、DLL、应用或设备操作。

| 证据 | SHA-256 |
| --- | --- |
| `local-ui-reverse/source/official/apps.razer.com/synapse/products/1352/ui/static/js/main.95a4f703.js` | `8dee0170de93fd9900024510e8c6edd315c30010d759ca12609ad6d2c8c73c80` |
| `local-ui-reverse/source/official/apps.razer.com/synapse/products/1352/ui/static/css/main.150adff3.css` | `b6970104c8a887ad5f6501afdcfd4548ae98bee838200b19cd0edcb21ad453d5` |
| 官方 `icon_external_link_sprite.0b292d50.svg`，1116 bytes | `cb5bd96f8c8671735c968584a0fa42692ac6bc4e68101e55c211646a243c549e` |

JS 字符偏移 `4577652` 的 `XU` 渲染标题 `VOLUME_HEADER`、标题开关、一个 `VU` 滑杆和 `OpenSoundVolume` 外部链接；没有独立的第二个 toggle 或读取状态文案。滑杆实参为 `min:0,max:100,step:1,minTag:"0",maxTag:"100"`，本页显示 `0/100`。通用 `VU` 虽支持省略标签时使用 `LOW/HIGH`，该默认分支不适用于此次调用。

`toggleSwitch` 保留 `value` 并翻转 `isEnabled`；`changeValue` 写入 `value` 并按 `!!value` 设置 `isEnabled`。Rust 保留现有真实音量读、写、取消、回读链；本地 draft 不代表设备写入成功。

CSS 字符偏移 `10916` 的 `.img-text .volume` 使用 flex/center。其 `:after` 为 20×20、`margin-left:4px`、`margin-bottom:2px`；正常引用原 sprite 的 `#link`，hover 引用 `#hover`。

原始 sprite 逐字节复制到 `assets/synapse/audio-volume-external-sprite.svg`，其 SHA 与上表相同。原文件包含完整 `symbol/use` 和以下两个视图：

```xml
<view id="link" viewBox="0 0 20 20" />
<view id="hover" viewBox="0 20 20 20" />
```

GPUI `img` 不执行浏览器 SVG fragment view 选择。`assets/synapse/audio-volume-sprite-embedded.rs` 嵌入原始字节，处理 `#link/#hover` 请求时，只按相应原 `<view>` 给根 `<svg>` 添加 `width="20" height="20" viewBox=...`。它不重画 path、不改色、不改 symbol/use；内存显示字节是 viewport 适配结果，不能将其哈希视为官方源文件哈希。436-byte 派生 hover 图已删除。

Rust 页面路径为 `crates/razer-pages/src/features/audio_volume.rs::source_volume_panel`；资源路径为 `synapse/audio-volume-external-sprite.svg#link` 和 `synapse/audio-volume-external-sprite.svg#hover`。`crates/razer-assets/src/lib.rs` 仅接入该小型 load/list 适配器。移除源没有的额外 toggle、读取状态文案与空页面占位文案。

验证：rustfmt、`git diff --check` 通过；主任务统一 `cargo check --locked --all-targets` 通过。未运行应用，因此运行视觉验收未完成。本次仅完成音量卡片局部对齐，PID 1352 整页六卡片布局与其他控件仍需逐项核对，不能据此标记整页完成。

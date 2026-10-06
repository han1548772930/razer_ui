# Studio 时长控件：2026-10-06

当前源码：`8552:C`（Spectrum 属性区域）、`1991:u/b`（共享预览与滑条）、`6257:Q9`（Spectrum 毫秒值）。完整文件哈希、UTF-16 范围和 JSX 收据见 [属性源码收据](chroma-studio-properties-source.json)。`tools/prepare-chroma-studio-properties.cjs` 已包含各份 CSS 的 duration-preview 规则。

Spectrum 三档值分别为 75480、37740、18870 毫秒。Rust 接入对应标题、帮助提示、时长标签、三档滑条、慢/快标签及动画预览。拖动更新控件临时位置，松开后将映射值写入工作参数；键盘和辅助功能修改也进入同一工作参数路径。不会把临时参数写进图层文档或设备区域。

预览来自当前 manifest 声明的五张原 SVG：普通慢/中/快，加 Reactive 中/快两张专用图。CSS 指定宽 218px 平铺、高 100px、纵向偏移 13px；SVG 保留 230:80 的比例。源码每次 requestAnimationFrame 分别增加 1.5、1.75、2px，不按秒换算。悬停预览或滑条、拖动期间播放，停止后保留相位；窗口失活清理交互，跨宿主重开重建窗口绑定。

`StudioProperties` 持有 `StudioDuration`，滑条使用已有 Base Slider，保留键盘方向键、Home/End、禁用值恢复；共享滑条的 FocusHandle 已补为 Tab stop。样式使用现有源码色板与 16px 根字号换算。

当前仅 Spectrum 挂载时长组件。Breathing、Reactive、Starlight 的独立根尚未接入，其毫秒数组已静态提取，不代表对应 UI 已实现。后续已接入 Spectrum 渐变编辑器，详见 [渐变审计](chroma-studio-gradient-native.md)。Spectrum 仍有弹层、光标和服务边界待核对；另有 10 个完全未接入的属性根。

禁止运行应用和 DLL，未做像素、焦点或真实设备验收。允许的格式化、cargo check、当前源码与资源静态检查结果记录在 [续接记录](ui-continuation-2026-10-06.md)。

# Introduction Tour

来源：`/synapse/introduction-tour/` 的 `main.bf769e69.js`（Fn/Cn/Dn/Pn/xn/jn）与 `main.adb3bb78.css`。独立下载快照在 `.ref/applications/synapse/introduction-tour`，不混用旧 frontend 的版本。更新：2026-10-02。

## Synapse 分支

| 步骤 | 原标题 key | 媒体 |
| --- | --- | --- |
| 1 | QUICK_EFFECTS_AND_ADVANCED_EFFECT_HEADER | quick_effect_advanced_effect.mp4 |
| 2 | THE_DEVICES_AND_MODULES_TAB_HEADER | devices_and_modules_tab.mp4 |
| 3 | MORE_RAZER_APPLICATIONS_HEADER | razer_more_apps.mp4 |
| 4 | USE_MACROS_TO_IMPROVE_EFFECIENCY_HEADER | macros.mp4 |
| 5 | LINKED_GAMES_TOUR_HEADER | linked_games.mp4 |

[Rust](../../src/shell/introduction_tour.rs) 保留原五步顺序和 locale 正文。第三步的 HTML 内联图标来自 `icon_app.b648a5e5.svg`；只扩展 SVG 视口表达两侧各 8px 空隙，图标路径保持原样。

描述列宽 450、媒体 640×360、列间 30；内容上距 80。最终 CSS 的 `!important` 规则令左／右 margin 为 20，并非居中。标题使用 RazerF5 24/23、正文 Roboto 14/17、段间 16；描述最小高 314。圆点直径 4、间距 12，当前点白色、其他 #707070，原点没有点击行为。背景为原 5120×1280 AVIF 转 PNG，cover 在 640 高区域；黑色 100px 带在 y=600，blur 25，由离线生成的 alpha 图表达。

第一步返回禁用，最后一步下一步改为 GET_STARTED 且 Skip 禁用。Skip 和最后一步完成发出关闭事件，由宿主移除页签；切换到别的页签保留当前步骤，关闭后重开从第一步开始。关闭同时删除历史中的 Tour 项，避免 Back/Forward 复活已关闭页签。Dashboard 单步教程的已读标记独立。

内容保留在布局流内，原生双轴滚动使 450px 描述列、媒体与操作在窄／短窗口可达。未给切步加原代码没有的淡入、滑动或缩放。按钮颜色、透明度端点与 opacity 200ms ease-out 过渡已按 CSS 接入；每个按钮保持独立过渡状态，鼠标释放／移出复位，支持减少动态效果。

## 媒体转换和边界

`tools/prepare-tutorial-media.py` 离线下载、验证并将源静音循环 MP4 转为无损动画 WebP，不降帧、不降尺寸，保留延时；相同帧由编码器合并并累加时间。Dashboard 单步与 Gamer Room 两步采用原 250×190 片段。转换依赖列于 `tools/requirements-tutorial-media.txt`。

GPUI 原生 `img` 负责动画、窗口失焦暂停和减少动态效果偏好；按片段使用 scoped `retain_all`，卸载时释放 CPU／GPU 图像。当前解码器一次展开一段全部帧：最长宏教程约 606MiB RGBA，因此还需实际窗口性能和内存测量，不能称为流式视频解码。

源／输出哈希、逐帧延时、尺寸和 URL 保存在 `assets/synapse/tutorial-media-manifest.json` 并并入总资源清单。`validate-resources.py` 从 WebP 容器独立解析画布、循环、帧区域和逐帧延时；[回归源码](../../src/shell/introduction_tour_tests.rs)覆盖五步导航、禁用状态、关闭通知、短窗口纵横滚动。仅做编译与静态核对，未运行应用或测试。

## 剩余范围

- `jn` 的 Chroma 分支另有 Quick Effects、Advanced Effects、Chroma Apps 三步静态图；已发现源码，尚未接入 Chroma 宿主。
- 原宿主的动态应用注册、完整顶栏更多应用入口尚未适配。
- 实际播放流畅度、缩放、焦点恢复和窗口边缘定位尚未动态验收。

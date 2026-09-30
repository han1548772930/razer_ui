# 滚动（`TAB_SCROLLING`）

> 本页只记录产品模块实际证明的滚轮配置结构。鼠标/键盘的滚轮映射编辑器、滚轮硬件阶段设置和通用 locale 文案不是同一个界面，必须分开处理。

## 1. 页面归属

| productId | 设备 | 是否有滚动标签 | 证据 |
|---:|---|---|---|
| `182` | Razer DeathAdder V3 Pro | 有，位于电源之后 | `.ref/devices/182/static/js/main.db20a7c4.js` 的 `TAB_SCROLLING` 与设备标签配置 |
| `653` | Blackwidow V4 Pro | 有，位于电源之后 | `.ref/devices/653/static/js/main.7b71cce5.js` 的 `TAB_SCROLLING` 与 `MapScrolling.d1bd18d8.chunk.js` |
| `777` | RAZER KRAKEN BT SANRIO LIMITED EDITION | 无 | `.ref/devices/777/static/js/main.eb70ce38.js` 的标签配置只有 `TAB_CUSTOMIZE/TAB_LIGHTING/TAB_CALIBRATION/TAB_POWER/TAB_SOUND/TAB_MIC` |

因此，777 不能显示滚动标签或鼠标滚轮阶段卡片。777 中出现的 `SCROLL_*` locale、滚轮动作名或共享 reducer 只代表公共代码/映射能力存在，不代表耳机页面有 `TAB_SCROLLING`。

## 2. 外层页面骨架

182 和 653 的滚动页都使用通用设备页面容器：

| 选择器 | 布局 | 样式 |
|---|---|---|
| `.main-container` | 纵向 flex，宽高占满，最小宽 `600px` | `background:#222` |
| `.nav-tabs` | 最小高 `48px`，底部 `2px solid #000` | `background:#222; color:#5d5d5d` |
| `.body-wrapper` | `padding:10px 20px 20px`，内容区域可滚动 | `overflow-y:auto`；窄窗口由 `.custom-scrollable`/`.noScrollX` 控制滚动方向 |
| `.body-widgets` | 横向 flex、允许换行、居中、最大宽 `1240px` | — |
| `.widget` | `600px` 固定宽度范围，`padding:30px 40px`，上下 `10px` 外边距 | `background:#111; border-radius:5px; font-size:14px` |

滚动页不应被实现成一张全宽、无滚动边界的深灰面板。

## 3. 182/653 实际滚轮配置结构

### 3.1 滚轮阶段选择与映射

两个设备的 `MapScrolling` 懒加载块结构一致，证据分别为：

- `.ref/devices/182/static/js/MapScrolling.edd6bbbe.chunk.js`
- `.ref/devices/653/static/js/MapScrolling.d1bd18d8.chunk.js`

实际渲染顺序：

1. 一个下拉选择器，数据来自 `scrollWheelStagesReducer.scrollWheelStages`；
2. 下方显示当前启用阶段，每个条目以 `Stage N - <effect>` 形式列出；
3. 显示可点击的下划线链接，用于跳转/编辑滚轮阶段配置；
4. 选择改变时比较当前 index 与已保存 index，调用 `enableSave` 标记未保存；若存在未保存映射，先显示保存提醒，再切换页面。

这里的下拉数据包含源码映射的效果 ID：

| 源码 ID | 说明 |
|---|---|
| `SW_ADAPTIVE` | 自适应滚动 |
| `SW_CUSTOM` | 自定义滚动 |
| `SW_DISTINCT` | 独立/区分滚动 |
| `SW_SMOOTH_SCROLL` | 平滑滚动 |
| `SW_STANDARD` | 标准滚动 |
| `SW_ULTRA_FINE` | 超精细滚动 |

这些 ID 来自 `MapScrolling` chunk 的 `S` 映射；页面只显示设备状态中 `isActive` 的阶段，不应静态画出所有阶段。

### 3.2 MapScrolling 的确切尺寸和颜色

`MapScrolling` chunk 使用内联样式，优先级高于泛化 CSS：

| 元素 | 确切样式 |
|---|---|
| 选择器下方容器 | `margin:20px 0` |
| 阶段标题 | `font-size:14px; color:#ccc; line-height:17px; margin-bottom:20px; text-transform:uppercase` |
| 启用阶段条目 | `font-size:14px; color:#707070; text-transform:capitalize` |
| 编辑链接 | `font-size:14px; color:#ccc; text-decoration:underline; margin-top:10px; margin-bottom:20px; cursor:pointer` |
| 下拉层级 | 选择器传入 `extraStyle:{zIndex:100}`，下拉菜单必须压在卡片内容之上 |

## 4. 182 鼠标滚轮阶段编辑器

182 的主模块 CSS 还证明了更完整的滚轮阶段/触觉编辑器结构。以下是实际存在且有尺寸或颜色证据的元素；是否显示由设备状态和当前编辑模式决定：

| 选择器 | 布局/尺寸 | 颜色/状态 |
|---|---|---|
| `.stages` | `display:flex; flex-direction:column; align-items:center`；阶段项目高 `68px` | `background:#222` 或编辑态 `#1e1e1e`；边界使用 `#44d62c` |
| `.stage` | `display:flex; align-items:center; flex:0 0 auto; height:68px; position:relative` | `border-radius:3px; transition:background-color .2s`；选中态使用 `border-bottom:2px solid #44d62c` 或顶部边线 |
| `.stage-title` | 文字居中，`font-size:14px; line-height:17px` | 默认 `#ccc`；激活/强调态 `#212121; font-weight:700` |
| `.stage-input` | `height:26px; width:60px` | `background:#111; border:1px solid #5d5d5d; color:#ccc; font-size:14px; line-height:19px; text-align:center`；聚焦边框 `#44d62c` |
| `.stage-control` | `height:27px; width:50%`，右侧控制区可 `align-items:end; justify-content:flex-end` | 文本右对齐 |
| `.stage-ordinal` | 默认 `30px` 方形区域；圆点状态 `7px/9px` | 激活序号 `#44d62c`，数字 `#111`，未激活圆点 `#222` |
| `.stage-circle-1..5` | 颜色标识圆点 | `#ff1a1a`、`#24ff00`、`#006fff`、`#00edff`、`#fff700` |
| `.scroll-item` | 相对定位；常见项目高 `36px`、宽 `230px` | `background:#111; border:1px solid #5d5d5d; text-align:left`；禁用态 `opacity:.3` |
| `.scroll-item__title` | `display:flex; margin-bottom:5px` | 标题按源码使用 uppercase 变换 |
| `.scroll-slide` | `display:flex; flex-direction:column; justify-content:space-between; margin-left:20px; margin-top:34px` | 子项禁用时保持低透明度 |

这些尺寸来自 `.ref/devices/182/static/css/main.48c20423.css`，不能用键盘页面的键位编辑器尺寸替代。

## 5. 触觉/滚轮高级配置面板

源码 CSS 同时包含 `haptic-content` 结构。它是滚轮高级配置或说明面板，不是页面默认一定展开的第二张卡片：

| 选择器 | 确切布局/样式 |
|---|---|
| `.haptic-content` | `position:relative; z-index:1`；弹层/选择表面变体使用 `850px` 宽 |
| `.haptic-content__title` | `margin:10px 0; color:#ccc; font-size:16px; text-align:center; text-transform:uppercase` |
| `.haptic-content__scroll` | `display:flex; justify-content:space-between; margin-top:20px` |
| `.fake-btn-value` | `width:255px; padding:20px 0; background:#292929; border:1px solid #0000; border-radius:4px; color:#ccc; font-size:14px; text-align:center` |
| `.fake-btn-value .value` | `font-size:28px; margin-bottom:10px` |
| `.custom-body` | `padding:20px 30px` |
| `.custom-body__desc` | `margin-bottom:30px; text-align:center` |
| `.custom-body__content` | `display:flex` |
| `.scroll-slide .scroll-item` | `margin-bottom:20px; position:relative` |
| `.scroll-slide .stepper` | `background:#111; border:1px solid #5d5d5d; margin-bottom:15px` |

可由 JS/locale 共同证实的高级参数名称包括 `SCROLL_STEPS`、`SCROLL_TENSION`、`SCROLL_ACCELERATION`、滚动模式和浏览器检测相关文案；但这些 locale key 不能单独证明所有设备、所有版本都同时显示全部参数。页面必须按实际产品分支逐项启用。

## 6. 滚动模式开关提示图

滚动模式帮助提示使用两个 SVG，而不是用 CSS 画一个绿色开关：

| 资源 | 尺寸 |
|---|---:|
| `scrollmode_switch_bg.svg` | `22px × 56px` |
| `scrollmode_switch_top.svg` | `14px × 43px`，左边距 `4px`、上边距 `3px` |

`.config-btns-wrapper .tip.scrollmode-switch` 的布局是 `display:flex; align-items:center; width:305px; font-size:12px; left:-30%; top:-200%`；说明内容 `flex:1`，左边距 `10px`。不可用状态的提示动画会让部分元素变为 `opacity:.3`。SVG 内部颜色必须保留原资源颜色，不得统一替换为项目主色。

## 7. 功能状态矩阵

| 状态 | 182/653 应表现 | 不应表现 |
|---|---|---|
| 正常进入滚动页 | 显示滚轮阶段下拉、启用阶段列表和编辑链接 | 不显示耳机专属声音/麦克风组件 |
| 当前阶段改变 | 更新选择 index，标记保存状态 | 不直接伪造“保存成功”提示 |
| 有未保存按键映射 | 切页前弹出原版保存提醒流程 | 不静默丢弃映射 |
| 阶段未启用 | 列表过滤 `isActive`，必要时以禁用样式呈现 | 不把所有阶段都标成 active |
| 777 耳机 | 不渲染本页，不在导航中出现 `TAB_SCROLLING` | 不因存在 `SCROLL_*` locale 或 reducer 添加页面 |

## 8. 明确删除的无证据内容

- 删除“777 也有滚动页”的断言；
- 删除把所有 `SCROLL_*` locale 都当作当前页面控件的做法；
- 删除“固定五个彩色阶段永远显示”的断言；彩色阶段资源和 CSS 存在，但显示受阶段数据和编辑状态控制；
- 删除将滚轮阶段配置与 `MapScrolling` 的按键映射编辑器混为一谈的布局；
- 删除未被 JS/CSS 证明的“拖动排序、任意阶段数量、自动保存成功、硬件触觉实时预览”等能力；
- 删除任何依赖 `image.png` 的尺寸、颜色或布局推断。

## 9. 证据索引

- 182 主模块：`.ref/devices/182/static/js/main.db20a7c4.js`
- 182 滚动映射块：`.ref/devices/182/static/js/MapScrolling.edd6bbbe.chunk.js`
- 182 样式：`.ref/devices/182/static/css/main.48c20423.css`
- 653 主模块：`.ref/devices/653/static/js/main.7b71cce5.js`
- 653 滚动映射块：`.ref/devices/653/static/js/MapScrolling.d1bd18d8.chunk.js`
- 653 样式：`.ref/devices/653/static/css/main.5425442a.css`
- 777 主模块/标签路由：`.ref/devices/777/static/js/main.eb70ce38.js`
- 设备 manifest：`.ref/devices/182/manifest.json`、`.ref/devices/653/manifest.json`、`.ref/devices/777/manifest.json`
- 资源索引：`.ref/devices/182/asset-manifest.json`、`.ref/devices/653/asset-manifest.json`、`.ref/devices/777/asset-manifest.json`

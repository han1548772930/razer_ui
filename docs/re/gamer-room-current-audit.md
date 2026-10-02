# Gamer Room 当前教程挂载与定位审计

复核日期：2026-10-02。仅静态读取当前 Dashboard 的 JS/CSS；没有读取历史 frontend/ASAR，也没有执行下载的脚本、应用或测试。当前来源身份见[版本说明](20-current-source-version.md)。

## 可重定位证据

下列偏移均为 UTF-8 解码后字符串的 **0 起始字符偏移**，不是字节偏移或旧构建中的同名符号。表内 JS/CSS 文件位于 `.ref/applications/synapse/dashboard/static/`。

| 当前文件 | 模块 / 偏移 | 证据 |
| --- | --- | --- |
| [9388.974b5d43.chunk.js](../../.ref/applications/synapse/dashboard/static/js/9388.974b5d43.chunk.js) | `19388` @1187；`He` 的 `id:"gamerRoom"` @338812 | 根节点依次挂载条件横幅 `o`、设备组 `oe` |
| 同上 | `19388 / je`，@337970、@338083 | `.dashboard.flex` 的行内 `width:"unset"`；子节点为无 gap 的 `display:flex;flexDirection:column` |
| 同上 | `19388 / ze`，@333454、@333660 | Synapse 组前挂载 `Pe`；`style:(ie.boxGroup,{zIndex:d})` 丢弃 `ie.boxGroup`，因此其中的 marginTop 30 不生效 |
| 同上 | `19388 / Pe` @330306；wrapper @330629 | 教程自己的 relative wrapper 包含 absolute 面板；初始步骤为 1，没有面板整体居中或测量重定位逻辑 |
| 同上 | `53700` @340556 | 两个 Gamer Room 教程 MP4 的静态 Webpack context，分别指向 module 51206 / chunk 1206 和 module 71641 / chunk 1641 |
| [main.01550b17.js](../../.ref/applications/synapse/dashboard/static/js/main.01550b17.js) | `22431` @44437；`P` @46690 | `GAMER_ROOM_TUTORIAL` 恰为两项，分别使用 Tutorial 1/2 媒体、同一标题及不同正文 |
| [55.4e8559cb.chunk.css](../../.ref/applications/synapse/dashboard/static/css/55.4e8559cb.chunk.css) | @3439、@182467、@182648 | `div` border-box；dashboard 的宽度/居中；组的 `margin:10px 0` |
| 同上 | @186048、@186153、@186208、@313796 | 1279 / 1260 / 600 / 2560 断点；1260 的 `width:min-content` 被 `je` 行内 `width:"unset"` 覆盖 |
| 同上 | @334355、@334514、@334581 | 教程面板 `(left:220,top:22,width:290)`、relative wrapper、第二步 `(213,-25)` |
| 同上 | @334639、@334763 | 第二步指示器 `left:-10%;top:50%;translate(-50%,-50%)`；默认 `left:-9%;top:0;translate(-50%)`；均为 36×36 |
| 同上 | @335059、@335260、@335326、@338625 | Skip 的 top15/right20；操作区上距20；页数上距10；dashboard 最终下外距50 |

源文件 SHA-256：

```text
9388.974b5d43.chunk.js  3f13c14fe1b040b3747825c7c308198fcc4a78f7a0aab11d7924fa79a000474a
55.4e8559cb.chunk.css   aba3cdc0ae2b3f36197ee327cc9a681a5680cf9117b6ee1bf59a867ebb57daf8
main.01550b17.js        b3a39adaa6507541fbdb3b16e5a9cd716b30289113859e057ef24a3925322917
```

## 实际 containing block 与尺寸

```text
He → #gamerRoom
├─ o → .gr-banner（条件显示）
└─ oe / je → .dashboard.flex（width:unset）
   └─ div（display:flex; flex-direction:column）
      ├─ ze / ue → Fragment
      │  ├─ Pe → .gamer-room-tutorial-modal__wrapper（relative，高度 0）
      │  │  └─ .gamer-room-tutorial-modal（absolute）
      │  └─ .box-group（margin-top / bottom 各 10）
      └─ ze / ue → 第二个 .box-group
```

wrapper 没有流内子项、padding 或 border，故高度为 0。其原点位于第一组 **10px 上外距之前**，横幅可见时纵坐标就是横幅底部。面板默认位于该原点右220、下22；第二步为右213、上25，切步位移恰为左7、上47。面板没有 `top:50%` 或 `translateY(-50%)`；只有第二步的指示器按面板高度垂直居中。

面板宽290，1px边框，内距 `40px 20px 20px`，实际内容宽248。原媒体 250×190 按内容宽度缩放，画面高188.48。面板未设置固定高度、最大高度或独立 overflow，整体高度取决于媒体、字体、当前正文换行、操作区与页数；不能给两步写死一个相同高度来对齐指示器。浏览器 `normal` 行高、内联 video 的基线以及中文字体回退仍需实际渲染测量，静态推导不能宣称完整面板像素高度一致。

指示器相对面板 padding box 定位。设面板外边界为 `(x,y,290,h)`，边框宽1：第一步圆心为 `(x+1−288×0.09, y+1+18)`；第二步为 `(x+1−288×0.10, y+h/2)`。指示器不是相对窗口或设备卡独立定位。

`.dashboard` 正常最大宽1220、最小宽620，1279及以下最大宽910，600及以下最大宽290且使用 `min-width:inherit`，2560及以上由更具体选择器改为最大宽2500。`inherit` 在此明确覆盖620；它取直接父节点 `#gamerRoom` 的计算值，不能跨过该节点去取更远祖先 `.body-wrapper` 的600。当前 `#gamerRoom` 没有 min-width 声明，其初始 `auto` 在此块级布局中取自动最小宽0，所以 Rust 在该断点使用0；不是让620与290同时生效后再判断最小宽优先。教程跟随设备组容器的横向位置；被居中的是该容器，面板仍使用固定 left。来源没有教程的窄窗碰撞翻转、窗口边缘吸附或独立高度压缩。

## 当前修正

[service_pages.rs](../../src/shell/service_pages.rs) 的 `GamerRoomPage` 已移除横幅与第一组之间无源码依据的本地说明/教程重播行。重置教程仍使用已有 Settings 入口。设备组使用独立容器承接当前断点、居中与50px下外距；教程 wrapper 的原点与该容器重合，不再把失效的30px样式或局部组 margin 加入偏移计算。

面板通过当前 `gpui-base 0.7.0` 的 `PopoverState` 保留受控开闭、Escape、焦点捕获/恢复及覆盖层注册；直接在设备组坐标中绝对定位。没有经过 `Popup` 强制的8px窗口边缘吸附，也没有独立 max-height 滚动区，正文滚动容器负责窄/短窗口访问。面板最后绘制，遮挡下方指针命中但允许滚轮交给正文。

第一页返回禁用，第二页返回第一页；下一步仅切到第二页，第二页按钮为 DONE；Skip/DONE 均标记已读并关闭。点外部不完成教程。添加设备模态打开时暂停教程，关闭后恢复原步骤。原始视频已转换为内嵌无损动画，来源和播放边界见[教程媒体说明](../screens/16-introduction-tour.md)。

[现有回归源码](../../src/shell/gamer_room_tutorial_tests.rs) 已改为首次未读自动显示，增加 wrapper 高度0、横幅/组/面板之间的独立坐标、1220/910/290容器宽度、窄短窗口不压缩/吸附，以及步骤位移断言。只做格式化和静态检查，统一 `cargo check --locked --all-targets` 由主任务执行；未运行测试。真实 IoT 服务、浏览器字体基线差异、滚轮/焦点实际窗口表现和最终像素高度仍未验收。

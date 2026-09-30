# 正在配对（TAB_PAIRING）

> 本页由自动脚本从雷云**自己的模块**提取，未做臆测。
> 重新生成：`node .ref/tools/gen-screen-docs.js`

## 1. 出现在哪些设备上

| 设备 | productId | 设备类型 | 该设备标签页顺序 |
|---|---|---|---|
| Razer DeathAdder V3 Pro | 182 | 鼠标 | 第 3 个：自定义 · 性能 · 正在配对 · 校准 · 电源 · 滚动 |

标签页真实文案：**正在配对**（key `TAB_PAIRING`）

## 2. 布局

### 2.0 页面骨架（所有设备页共用）

设备页**不是**「一列全宽卡片」。真实骨架如下（逐条引自设备模块的 CSS）：

| 选择器 | 布局 | 样式 |
|---|---|---|
| `body, html` | `font-family:Roboto,sans-serif; font-size:16px; height:100%; margin:0; max-width:1920px; min-height:720px; overflow:hidden; width:100%` | `background-color:#222; color:#ccc; user-select:none` |
| `.main-container` | `display:flex; flex-direction:column; height:100%; min-width:600px; position:absolute; width:100%` | `background-color:#222` |
| `.nav-tabs` | `display:flex; align-items:center; min-height:48px; position:relative; width:100%; z-index:105` | `background-color:#222; border-bottom:2px solid #000; color:#5d5d5d` |
| `.body-wrapper` | `flex:1 1; height:100%; min-width:600px; padding:10px 20px 20px; width:100%` | — |
| `.body-widgets` | `flex-direction:row; flex-wrap:wrap; justify-content:center; margin:auto; max-width:1240px` | — |
| `.body-widgets .widget` | `flex:0 0 auto; height:auto; margin:10px auto; max-width:600px; min-width:600px; padding:30px 40px; position:relative; font-size:14px` | `background-color:#111; border-radius:5px` |
| `.widget-col` | `flex-direction:column; height:fit-content; width:600px` | — |
| `.widget-prod` | `height:250px; margin:10px auto; max-width:1220px; min-width:1024px; width:100%` | — |
| `.widget-prod img` | `left:50%; position:absolute; top:50%` | — |
| `.thx-btn` | `display:inline-block; padding:.5rem 1.5rem; text-align:center; text-transform:uppercase` | `background-color:#44d62c; border-radius:3px; color:#000` |
| `.thx-btn:hover` | — | `opacity:.8` |
| `.thx-btn:active` | — | `opacity:.6` |
| `.thx-btn.disabled` | `cursor:default` | `opacity:.3` |
| `.thx-btn.secondary` | — | `background-color:#707070; color:#fff` |

由此可推出的界面排布：

```
.main-container（纵向，底 #222）
├─ .nav-tabs           顶栏，最小高 48px，下边线 2px #000，文字 #5d5d5d
└─ .body-wrapper（内边距 10/20/20）
   ├─ .widget-prod      顶部产品图区，高 250px，宽 1024–1220px，图片绝对居中
   └─ .body-widgets     横向排列 + 自动换行 + 居中，最大宽 1240px
      ├─ .widget        固定 600px 宽，内边距 30/40，底 #111，圆角 5px，字号 14px
      ├─ .widget
      └─ .widget-col    600px 宽的列，内部再纵向堆叠
```

> 关键数值：**卡片固定 600px、容器最大 1240px、产品图区高 250px、控件圆角 3px、卡片圆角 5px**。

### 2.0.1 全局样式

**调色板**（按出现次数排序，共统计 1844 处颜色声明）：

| 次数 | 颜色 | 角色 |
|---:|---|---|
| 289 | `#44d62c` | 主色 / 激活态 / 标题 |
| 275 | `#ccc` | 主文字 |
| 157 | `#5d5d5d` | **通用边框** |
| 139 | `#111` | 卡片 / 输入框底色 |
| 132 | `#000` | 按钮文字、描边、顶栏下边线 |
| 108 | `#0000` | 透明 |
| 80 | `#707070` | 次要按钮底色 |
| 70 | `#999` | 次要文字 |
| 68 | `#fff` | 次要按钮文字 |
| 63 | `#fd8611` | 橙色：提示 / 警告 / 分享 |
| 59 | `#222` | 页面底色 / 顶栏底色 |
| 30 | `#ffffff1a` | 半透明白：hover 覆盖 |
| 25 | `#0000004d` | 半透明黑：按钮描边 |
| 23 | `#212121` | 深色文字（浅底上）/ 离线指示 |
| 19 | `#ffffff4d` | 半透明白：按下覆盖 |
| 15 | `#2d2d2d` | **hover 底色** |
| 15 | `#fd4949` | 危险红（亮） |
| 14 | `#c8323c` | 危险红 |
| 14 | `#333` | 开关关闭态底色 |
| 11 | `#515151` | **下拉框边框** |
| 11 | `#4a4a4a` | 帮助图标底色 |
| 10 | `#292929` |  |

**字体与圆角**（同样按出现次数排序）：

| 项 | 值 | 说明 |
|---|---|---|
| 正文字体 | `Roboto, sans-serif` | `body,html` 全局设置 |
| 标题字体 | `RazerF5` | 区块标题（如 `.key-config .body .heading`） |
| 正文字号 | `14px` | 出现 251 次，设备页里最常用 |
| 小字号 | `12px` | 出现 76 次，次要按钮与说明 |
| 全局字号 | `16px` | `body,html`；进入 `.widget` 后变为 14px |
| 控件圆角 | `3px` | 出现 89 次 |
| 卡片圆角 | `5px` | 出现 43 次 |
| 窗口最小尺寸 | `min-width:600px` / `min-height:720px` | `.main-container` / `body,html` |

> ⚠️ 通用边框是 **`#5d5d5d`**（出现 162 次），不是 `#555`（仅 7 次）。
> hover 底色是 **`#2d2d2d`**；下拉框边框是 **`#515151`**。

### 2.1 该界面的分区（布局 + 样式）

下表把该界面的类名按语义归入 UI 分区，并给出它们在 CSS 里的**实际布局与样式声明**。
分区顺序即界面自上而下 / 自左而右的排布。

#### Razer DeathAdder V3 Pro（productId 182）

该界面共 72 个布局类名，分 3 个分区。

**① 配对状态与指引**（63）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `ConfirmDialog_unpairDialog__qWsuI` | `max-width:400px; padding:30px; width:90%` | `background:#1a1a1a; border:1px solid #ffffff1a; border-radius:8px; box-shadow:0 10px 40px #00000080` |
| `DeviceCard_pairedBadge__1Fx8u` | `align-items:center; display:flex; gap:4px; height:20px; justify-content:left; left:10px; min-width:66px; padding:2px 5px 2px 2px; position:absolute; top:10px`<br>`height:calc(100% + 12px); left:-4px; position:absolute; top:-6px; width:calc(100% + 8px)` | `border:1px solid #44d62c; border-radius:10px; border-radius:25px; color:#44d62c; cursor:pointer; font-size:12px; font-weight:600`<br>`cursor:pointer`<br>`border-color:#fd8611; color:#fd8611`<br>`outline:none`<br>`border:1px dashed #44d62c; border-radius:0` |
| `DeviceCard_pairingBadge__uQFji` | `align-items:center; display:flex; gap:6px; height:20px; justify-content:left; left:0; min-width:76px; padding:2px; position:absolute; top:0` | `border:1px solid #44d62c; border-radius:10px; border-radius:25px; color:#44d62c; font-size:12px; font-weight:600`<br>`line-height:1; text-transform:lowercase`<br>`text-transform:uppercase` |
| `Duallink_device-dongle__bCz5e` | `align-items:center; display:flex; flex-direction:column; height:100%; justify-content:center; position:relative; width:100%`<br>`display:block; height:80px` | — |
| `Duallink_dongle-img-box__H5om8` | `display:block; height:80px` | — |
| `Duallink_pairInfoBox__wg6qs` | `align-items:center; display:flex; justify-content:space-between`<br>`display:block; height:28px; min-width:90px; padding:0 5px` | `background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; opacity:.8; text-align:center`<br>`opacity:1` |
| `Duallink_unpairbutton__lTG1W` | `display:block; height:28px; left:50%; padding:0 10px; position:absolute; top:50%`<br>`display:block; height:28px; min-width:90px; padding:0 5px` | `background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; text-align:center`<br>`background-color:#9b9b9b`<br>`background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; opacity:.8; text-align:center`<br>`opacity:1` |
| `Duallink_unpairbuttondisabled__ZdDMy` | `display:block; height:28px; left:50%; padding:0 10px; position:absolute; top:50%` | `background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; opacity:.5; text-align:center` |
| `MultiDevicePairing_cell-block__mRcfS` | `display:block; margin:auto` | — |
| `MultiDevicePairing_content-text__ASC0b` | `width:100%`<br>`display:block; margin:auto`<br>`margin:20px auto 10px; max-width:100%; width:790px`<br>`display:flex; height:156px; margin:auto; width:604px` | `color:#ccc; font-family:Roboto; font-size:14px; text-align:center` |
| `MultiDevicePairing_content__v4i6C` | `display:flex; flex:1 1; flex-direction:column; width:100%`<br>`width:100%`<br>`display:block; margin:auto`<br>`margin:20px auto 10px; max-width:100%; width:790px` | `background:#222`<br>`color:#ccc; font-family:Roboto; font-size:14px; text-align:center`<br>`background-position:50%; background-repeat:no-repeat; background-size:100% 100%`<br>`background-image:url(../../static/media/icon_kb_ensure.ac7a960e.svg)`<br>`background-image:url(../../static/media/icon_mouse_ensure.42d0a024.svg)` |
| `MultiDevicePairing_device-dongle__YBXhq` | `align-items:center; display:flex; flex-direction:column; height:100%; justify-content:center; position:relative; width:100%`<br>`display:block; height:80px` | — |
| `MultiDevicePairing_disabledText__zZbCG` | — | — |
| `MultiDevicePairing_disabled__RL6a1` | — | `opacity:.45` |
| `MultiDevicePairing_dongle-area__dvj5e` | `display:flex; height:156px; margin:auto; width:604px`<br>`align-items:center; display:flex; flex-direction:column; height:100%; justify-content:center; position:relative; width:100%`<br>`display:block; height:80px` | — |
| `MultiDevicePairing_dongle-img-box__NcAgw` | `display:block; height:80px` | — |
| `MultiDevicePairing_dualLinkModal__o1tvl` | — | `background-color:#222!important` |
| `MultiDevicePairing_duallink-left-state__2B3DA` | `height:100%; padding:0 10px; width:250px` | `text-align:center` |
| `MultiDevicePairing_duallink-state__DbWWj` | `align-items:center; display:flex; height:20px; justify-content:center; margin:20px auto auto; width:520px`<br>`height:100%; padding:0 10px; width:250px` | `color:#44d62c; font-family:Roboto; font-size:14px; line-height:20px; text-align:center`<br>`text-align:center` |
| `MultiDevicePairing_enable-info__pat0Z` | `position:relative; top:-20px` | `color:#ccc; font-size:14px` |
| `MultiDevicePairing_introduction__Badwv` | `margin:20px auto 10px; max-width:100%; width:790px` | — |
| `MultiDevicePairing_pageContainer__CHfD7` | `height:100%; overflow:auto; padding:10px 10px 20px; width:100%` | — |
| `MultiDevicePairing_ruleItem__RPGY-` | — | `color:#999; font-family:Roboto; font-size:14px` |
| `MultiDevicePairing_text-cell__teyWT` | — | `color:#ccc; font-family:Roboto; font-size:14px; text-align:center` |
| `MultiDevicePairing_top-position__hlGjT` | `position:relative; top:-10px` | — |
| `PairingContent_devicesContainer__R6Ch9` | `display:flex; flex-wrap:wrap; gap:20px` | — |
| `PairingContent_emptyStateBox__XjlF7` | `align-items:center; display:flex; height:220px; justify-content:center; padding:36px; width:290px` | `border:2px dashed #666; border-radius:5px` |
| `PairingContent_emptyStateText__AjC20` | — | `color:#ccc; font-family:Roboto,sans-serif; font-size:14px; font-weight:400; letter-spacing:0; line-height:17px; text-align:center` |
| `PairingContent_instruction__AwuZc` | — | `color:#ccc; font-size:14px` |
| `PairingContent_pairingContent__PvgF1` | `width:100%` | — |
| `PairingContent_skeletonBadge__BnFEx` | `overflow:hidden; position:relative`<br>`align-self:flex-start; height:20px; min-width:76px; padding:2px 8px; width:-webkit-fit-content; width:fit-content` | `border:1px solid #999; border-radius:25px; color:#999; font-size:12px; font-weight:600; line-height:14px` |
| `PairingContent_skeletonBox__tt-fp` | `align-items:center; display:flex; flex-direction:column; justify-content:flex-start; padding:10px; position:relative` | `background:#0000004d; border:initial` |
| `PairingContent_skeletonImage__qxefc` | `overflow:hidden; position:relative`<br>`height:100%; left:-150%; position:absolute; top:0; width:150%`<br>`height:99px; width:248px` | `background:linear-gradient(90deg,#0000,#ffffff73 50%,#0000)`<br>`background:#ffffff08; border-radius:3px` |
| `PairingContent_skeletonText__o6hPc` | `overflow:hidden; position:relative`<br>`height:100%; left:-150%; position:absolute; top:0; width:150%`<br>`height:16px; width:248px` | `background:linear-gradient(90deg,#0000,#ffffff73 50%,#0000)`<br>`background:#ffffff08; border-radius:3px` |
| `PairingHeader_description__NOyrE` | `margin:0 0 16px` | `color:#ccc; font-size:14px; line-height:1.5` |
| `PairingHeader_headerLeft__6DHQu` | `flex:1 1` | — |
| `PairingHeader_header__PHKzH` | `display:flex; gap:60px; padding:30px 40px` | `background:#0003; border-radius:5px` |
| `PairingHeader_instructionList__DNdcU` | `margin:0; padding:0`<br>`position:relative`<br>`left:0; position:absolute` | `color:#ccc; font-size:14px; line-height:1.4`<br>`color:#ccc` |
| `PairingHeader_link__EV6wB` | `align-items:center; display:inline-flex; gap:6px`<br>`height:14px; width:14px` | `color:#ccc; cursor:pointer; font-size:14px; transition:color .2s`<br>`color:#44d62c` |
| `PairingHeader_title__X63KF` | `margin:0 0 12px` | `color:#44d62c; font-size:16px; font-weight:inherit; letter-spacing:.5px; text-transform:uppercase` |
| `box-scanselect-pair` | — | `background-color:#44d62c; color:#212121`<br>`opacity:.7` |
| `box-unpairselect-button` | `display:inline-block; height:28px; width:90px`<br>`min-width:-webkit-max-content; min-width:max-content; padding:0 10px; position:relative` | `border:1px solid #000; border-radius:2px; border-radius:3px; font-family:Roboto; font-size:12px; line-height:28px; text-align:center; text-transform:uppercase; transition:opacity .3s` |
| `box-unpairselect-cancel` | — | `background-color:#707070; color:#fff; opacity:.7`<br>`opacity:1` |
| `box-unpairselect-confirm` | — | `background-color:#44d62c; color:#212121`<br>`opacity:.7` |
| `box-unpairselect-tag` | `display:flex; justify-content:center`<br>`display:inline-block; height:28px; width:90px` | `color:#212121; font-family:Roboto; font-size:12px; line-height:28px; text-align:center`<br>`border:1px solid #000; border-radius:2px; border-radius:3px; font-family:Roboto; font-size:12px; line-height:28px; text-align:center; text-transform:uppercase; transition:opacity .3s`<br>`background-color:#707070; color:#fff; opacity:.7`<br>`opacity:1`<br>`background-color:#44d62c; color:#212121` |
| `confirm-unpairing` | `height:auto; left:50%; margin:10px auto auto; position:absolute; width:210px`<br>`margin:3px auto 9px; width:190px`<br>`min-width:-webkit-max-content; min-width:max-content; padding:0 10px; position:relative` | `background:#111; border:1px solid #fd8611; border-radius:3px; transition:all 1.2s ease-in-out 0s`<br>`color:#ccc; font-family:Roboto; font-size:14px; line-height:17px; text-align:center` |
| `confirm-unpairing-content` | `margin:3px auto 9px; width:190px` | `color:#ccc; font-family:Roboto; font-size:14px; line-height:17px; text-align:center` |
| `customize-center--wireless` | `left:auto!important; position:relative!important; width:auto!important` | — |
| `customize-container--wireless` | `align-items:flex-start; display:flex; flex-wrap:wrap; max-width:1240px`<br>`【@media(max-width:1200px)】justify-content:center`<br>`min-width:600px; width:600px` | — |
| `detecting-device--wireless` | `top:60px` | — |
| `detection-icons--wireless` | `gap:20px` | — |
| `dongle-img-box` | `display:block; height:80px; width:80px` | — |
| `dongle-info` | `width:100%`<br>`display:block; margin:auto` | `color:#ccc; font-family:Roboto; font-size:14px; text-align:center; text-transform:uppercase` |
| `dot-background--wireless` | `height:275px` | — |
| `duallink-device-dongle` | `align-items:center; display:flex; height:-webkit-fit-content; height:fit-content; justify-content:center; padding:0 10px; width:100%`<br>`align-items:center; display:flex; flex-direction:column; height:100%; position:relative; width:50%`<br>`display:block; height:80px; width:80px`<br>`width:100%` | `text-align:center`<br>`color:#ccc; font-family:Roboto; font-size:14px; text-align:center; text-transform:uppercase` |
| `duallink-keyboard-dongle` | `align-items:center; display:flex; flex-direction:column; height:100%; position:relative; width:50%`<br>`display:block; height:80px; width:80px`<br>`width:100%`<br>`display:block; margin:auto` | `text-align:center`<br>`color:#ccc; font-family:Roboto; font-size:14px; text-align:center; text-transform:uppercase` |
| `duallink-master-dongle` | `align-items:center; display:flex; flex-direction:column; height:100%; position:relative; width:50%`<br>`display:block; height:80px; width:80px`<br>`width:100%`<br>`display:block; margin:auto` | `text-align:center`<br>`color:#ccc; font-family:Roboto; font-size:14px; text-align:center; text-transform:uppercase` |
| `duallink-mouse-dongle` | `align-items:center; display:flex; flex-direction:column; height:100%; position:relative; width:50%`<br>`display:block; height:80px; width:80px`<br>`width:100%`<br>`display:block; margin:auto` | `text-align:center`<br>`color:#ccc; font-family:Roboto; font-size:14px; text-align:center; text-transform:uppercase` |
| `effects-area--wireless` | `flex-wrap:wrap; width:150px` | — |
| `multipairing` | `height:44px; max-width:44px; min-width:44px` | `background-image:url(../../static/media/icon-multideviceparing.9f560b69.svg); background-position:50%; background-repeat:no-repeat`<br>`background-image:url(../../static/media/icon_productivity_multi_device_pairing.a7a2c84d.svg)`<br>`background-image:url(../../static/media/icon-multidevicepairing2.af8657a6.svg)` |
| `pair-title` | `align-items:center; display:flex` | — |
| `product__icon--wireless` | `bottom:127px; height:115px` | — |
| `wireless-status-wrapper` | `align-items:center; display:flex` | — |

**② 扫描到的设备列表**（5）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `box-scanningdevice-tag` | `display:block; height:auto; position:block; width:100%` | `color:#44d62c; font-size:14px; line-height:16px; text-align:center; transition:color .2s` |
| `box-scanselect-button` | `display:inline-block; height:28px; width:90px` | `border:1px solid #000; border-radius:2px; border-radius:3px; font-family:Roboto; font-size:12px; line-height:28px; text-align:center; text-transform:uppercase; transition:opacity .3s` |
| `box-scanselect-cancel` | — | `background-color:#707070; color:#fff; opacity:.7`<br>`opacity:1` |
| `box-scanselect-tag` | `display:flex; justify-content:center`<br>`display:inline-block; height:28px; width:90px` | `color:#212121; font-family:Roboto; font-size:12px; line-height:28px; text-align:center`<br>`border:1px solid #000; border-radius:2px; border-radius:3px; font-family:Roboto; font-size:12px; line-height:28px; text-align:center; text-transform:uppercase; transition:opacity .3s`<br>`background-color:#707070; color:#fff; opacity:.7`<br>`opacity:1`<br>`background-color:#44d62c; color:#212121` |
| `scan-devices-list` | `display:flex; flex-direction:column; height:auto; position:relative; width:100%`<br>`height:20px; margin:10px 0; width:100%`<br>`height:104px; overflow-y:auto; position:relative; width:100%`<br>`display:flex; margin:0; min-height:20px` | `color:#ccc; font-family:Roboto; font-size:14px; line-height:20px; text-align:center`<br>`color:#fff; text-transform:uppercase` |

**⑫ 其它**（4）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `choose-network-container` | `align-items:center; display:flex; flex-direction:column; justify-content:center`<br>`align-items:baseline; display:flex; flex-direction:column; justify-content:center; position:relative; width:300px`<br>`width:285px`<br>`height:23px; width:279px` | `color:#fd4949`<br>`background-color:#111`<br>`background-color:#111; border:1px solid #515151; color:#ccc`<br>`cursor:pointer`<br>`background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-size:14px; line-height:16px; opacity:0; text-align:left; transition:visibility 0s,opacity .3s linear` |
| `choose-network-dropdown` | `width:285px` | `background-color:#111` |
| `choose-network-section` | `align-items:baseline; display:flex; flex-direction:column; justify-content:center; position:relative; width:300px`<br>`width:285px`<br>`height:23px; width:279px`<br>`bottom:7px; position:absolute; right:15px` | `background-color:#111`<br>`background-color:#111; border:1px solid #515151; color:#ccc`<br>`cursor:pointer`<br>`background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-size:14px; line-height:16px; opacity:0; text-align:left; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `text-connecting-network` | `height:104px` | — |

## 3. 功能项

该界面对应的雷云文案 key，共 **68** 条（中文为雷云原文）：

| key | 中文 | English |
|---|---|---|
| `DONGLE_IS_LATEST` | Razer HyperSpeed 无线接收器使用的已经是最新固件。 | The Razer HyperSpeed Wireless Dongle is already using the latest firmware. |
| `HYPERPOLLING_WIRELESS` | HYPERPOLLING WIRELESS | HYPERPOLLING WIRELESS |
| `HYPERPOLLING_WIRELESS_DONGLE_HEADER` | 使用 Razer HyperPolling 无线接收器，你可以配对兼容的设备以获得更优秀的性能。将兼容的 Razer 雷蛇设备连接到 Razer HyperPolling 无线接收器以开始。 | With the Razer HyperPolling Wireless Dongle, you can pair a compatible device to get a higher level of performance. Connect a compatible Razer device to the Razer HyperPolling Wireless Dongle to begin. |
| `HYPERPOLLING_WIRELESS_DONGLE_NOTE_FOUR` | 将设备放在 HyperPolling 无线接收器附近。 | Keep the device in close proximity to the HyperPolling Wireless Dongle |
| `HYPERPOLLING_WIRELESS_DONGLE_UNPAIR_CONFIRM_TEXT` | 你即将取消 Razer 雷蛇设备与 Razer HyperPolling 无线接收器的配对。确定要继续吗？ | You're about to unpair your Razer device with the Razer HyperPolling wireless. Are you sure you want to proceed? |
| `HYPERPOLLING_WIRELESS_HEADER` | 搭配兼容的 Razer 雷蛇鼠标，通过 HyperSpeed Wireless 无线技术解锁高达 8000Hz 的轮询率，实现无与伦比的精准度。 | Pair with a compatible Razer mouse to unlock up to 8,000 Hz polling rate via HyperSpeed Wireless technology for unmatched precision. |
| `HYPERPOLLING_WIRELESS_HEADER_1` | 有了 Razer 雷蛇鼠标底座专业版，你可以配对一个兼容的设备，借助我们进一步改良的自适应跳频技术实现超快速度和极低延迟。 |  |
| `HYPERPOLLING_WIRELESS_HEADER_2` | 将兼容的 Razer 雷蛇设备连接到 Razer 雷蛇鼠标底座专业版以开始。 |  |
| `HYPERPOLLING_WIRELESS_NOTE_FOUR` | 将设备放在 Razer 雷蛇鼠标底座专业版附近 | Keep the device in close proximity to the Razer Mouse Dock Pro |
| `HYPERPOLLING_WIRELESS_UNPAIR_CONFIRM_TEXT` | 你即将取消 Razer 雷蛇设备与 Razer 雷蛇鼠标底座专业版的配对。确定要继续吗？ | You're about to unpair your Razer device with the Razer Mouse Dock Pro. Are you sure you want to proceed? |
| `LIGHTING_DEVICE_PAIRING_FAILED` | 无法添加设备 | UNABLE TO ADD DEVICE |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_1` | 请重置设备，然后重试。若要重置，请执行以下操作： | Please reset your device and try again. To reset: |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_2` | 1.在 Razer 雷蛇幻彩 RGB 补光灯的电源开启时，按住“重置”按钮 10 秒钟。 | 1. With the Razer Key Light switched on, press and hold the Reset button for 10 seconds. |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_3` | 2.Razer 雷蛇幻彩 RGB 补光灯将短暂  | 2. The Razer Key Light will briefly  |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_4` | 闪烁白色 | flash in white |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_5` | 然后变为 | and change to |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_6` | 蓝色闪烁 | flashing blue |
| `LIGHTING_DEVICE_PAIRING_FAILED_MSG_7` | 表示设备已重置。 | to indicate that the device has been reset. |
| `LIGHTING_DEVICE_PAIRING_SUCCESS_ADD_MORE` | 添加另一个设备 | ADD ANOTHER DEVICE |
| `LIGHTING_DEVICE_PAIRING_SUCCESS_HEADER` | 现在，你已连接到你的新 Razer 雷蛇设备。 | You're now connected to your new Razer device. |
| `LIGHTING_DEVICE_PAIRING_SUCCESS_HEADER_1` | 添加另一个 Razer 雷蛇设备或开始自定义！ | Add another Razer device or start customizing! |
| `MULTI_DEVICE_DESCRIPTION` | Razer Sensa HD 通过协调让多个设备提供同步的触觉效果，确保让所有硬件共同为你打造全身性的体验。 | Razer Sensa HD coordinates multiple devices to deliver in-sync haptic effects, ensuring a full-body experience that isn’t limited to just a single piece of hardware. |
| `MULTI_DEVICE_DONGLE` | 多设备接收器 | MULTI DEVICE DONGLE |
| `MULTI_DEVICE_DONGLE_DUALLINK_NOTE_ONE` | Ensure the device is set to 2.4GHz wireless mode. | Ensure the device is set to 2.4GHz wireless mode. |
| `MULTI_DEVICE_HARMONIZED_HAPTICS` | 设备间的触觉效果协调一致 | MULTI-DEVICE HARMONIZED HAPTICS |
| `MULTI_DEVICE_HEADER` | 借助 Razer 雷蛇高效工作接收器，你可以额外将另一台兼容的设备与 Razer 雷蛇设备的无线 USB 接收器配对。 | With the Razer Productivity Dongle, you can pair an additional compatible device with the wireless USB dongle of your Razer device. |
| `MULTI_DEVICE_HYPER_SPEED_DONGLE` | 多设备 HYPERSPEED 接收器 | MULTI DEVICE HYPERSPEED DONGLE |
| `MULTI_DEVICE_PAIRING` | 多设备配对 | MULTI-DEVICE PAIRING |
| `MULTI_DEVICE_PAIRING_DESCRIPTION` | 将多个设备与 Razer 雷蛇高效工作接收器配对。 | Pair multiple devices to Razer Productivity Dongle. |
| `MULTI_DEVICE_PAIRING_UTILITY` | 多设备配对实用程序 | MULTI-DEVICE PAIRING UTILITY |
| `PAIR` | 配对 | PAIR |
| `PAIRED` | 已配对 | Paired |
| `PAIRED_WITH` | 已配对 | Paired with |
| `PAIRING` | 正在配对... | Pairing... |
| `PAIRING_COMPLETE` | 绑定完成 | Pairing Complete |
| `PAIRING_CONDITIONS_TITLE` | 请确保在配对前满足以下条件： | Please ensure these conditions are met before pairing: |
| `PAIRING_CONDITION_HYPERSPEED_MODE` | 设备设置为 HyperSpeed 模式 (2.4G)。 | The device is set to HyperSpeed mode (2.4GHz). |
| `PAIRING_CONDITION_KEEP_ACTIVE` | 每隔几分钟按下某一按键或按钮以防止设备进入待机模式。 | The device does not go into idle mode by pressing a key or button every few minutes. |
| `PAIRING_CONDITION_NO_DONGLE` | 设备的接收器未连接至此电脑。 | The device's dongle is not connected to this PC. |
| `PAIRING_CONDITION_NO_USB` | 设备不是通过 USB 线缆连接。 | The device is not connected via USB cable. |
| `PAIRING_CONDITION_PROXIMITY` | 设备位于 HyperSpeed 接收器附近。 | The device is in close proximity to the HyperSpeed dongle. |
| `PAIRING_CONNECT_DEVICES` | 连接已配对的设备。 | Connect your paired devices. |
| `PAIRING_DEVICE_NOT_FOUND` | 如果你的设备未出现在此列表中，请检查上述条件。如果问题仍然存在，请联系 Razer 雷蛇支持服务团队。 | If your device does not appear on this list, check the conditions above. If problem persists, please contact Razer support. |
| `PAIRING_FAILED` | 配对失败 | Pairing Failed |
| `PAIRING_PAIR_DEVICE_TITLE` | 配对设备 | PAIR DEVICE |
| `PAIRING_PAIR_NEW_DEVICE_WARNING` | 配对此新设备后，当前已配对的设备将解除配对，你将无法继续使用无线接收器。 | By pairing this new device, the current paired device will be unpaired and you will not be able to use with the wireless dongle. |
| `PAIRING_REMOVE` | 删除 | REMOVE |
| `PAIRING_SELECT_DEVICE` | 请从以下列表中选择你当前拥有的兼容设备。 | Use the list below to select the compatible device you currently own. |
| `PAIRING_UNPAIR_DEVICE_TITLE` | 取消设备的配对 | UNPAIR DEVICE |
| `PAIRING_UNPAIR_NEED_OTHER_DEVICE` | 解除配对后，你可能需要为电脑使用其他鼠标或键盘。 | You may need a different mouse or keyboard for your PC after you unpair this one. |
| `PAIRING_UNPAIR_WARNING` | 连接此新设备将解除当前所有已配对设备的连接，且你将无法继续使用现有的无线接收器。 | Connecting to this new device will unpair any currently paired devices, and you will not be able to use the existing wireless dongle. |
| `PAIRING_UNPAIR_WARNING_MESSAGE` | 解除配对后，你将无法再将设备与无线接收器搭配使用。 | You will not be able to use this device with the wireless dongle after unpairing. |
| `PAIRING_UTILITY` | 配对实用程序 | PAIRING UTILITY |
| `PAIR_NEW_DEVICE` | 配对新设备 | Pair New Device |
| `PAIR_NEW_DEVICE_BUTTON` | 激活配对模式 | ACTIVATE PAIRING MODE |
| `PAIR_NEW_DEVICE_LINK` | 兼容的 Razer 雷蛇音箱系统 | Compatible Razer speaker systems |
| `PAIR_NEW_DEVICE_POPUP_BUTTON` | 激活配对模式 | ACTIVATE PAIRING MODE |
| `PAIR_NEW_DEVICE_POPUP_CONTENT_1` | 通过 {{link}} 将控制盘与你的手机配对，以便关联最多 3 个 Razer 雷蛇音箱系统，并为每个设备自定义媒体控制功能。 | Pair the control pod with your mobile phone via the {{link}} to link up to 3 Razer speaker systems and customize media controls for each device. |
| `PAIR_NEW_DEVICE_POPUP_CONTENT_2` | 如果设置为配对模式，你则无法通过 Razer Synapse 雷云控制该设备。确定要继续吗？ | You won't be able to control this device on Razer Synapse if you set it to pairing mode. Are you sure you want to proceed? |
| `PAIR_NEW_DEVICE_TIP` | 提示：你也可以在设备处于命令模式时，长按音源按钮 ( {{icon}} ) 4 秒，从而激活配对模式。 | Tip: You can also activate pairing mode when you press and hold the Source button ( {{icon}} ) for 4 seconds while the device is in Command mode. |
| `PAIR_NEW_DEVICE_TOOLTIP` | 使用 Razer Audio 音频应用程序通过移动设备来激活配对模式，或使用 Razer Synapse 雷云应用程序通过新电脑来激活配对模式，以便连接并自定义控制盘。<br><br>目前，控制盘设置为命令模式，允许你在连接到电脑或移动设备时对其控件进行个性化设置。 | Activate Pairing mode to connect and customize the control pod with a mobile device using the Razer Audio app, or with a new PC using the Razer Synapse app. <br><br>Currently, the control pod is set to Command Mode, allowing you to personalize its controls when connected to a PC or a mobile device. |
| `PAIR_WITH_A_NEW_DEVICE` | 与新设备配对 | Pair with a New Device |
| `SCAN_AGAIN` | 重新扫描 | SCAN AGAIN |
| `UNPAIR` | 取消配对 | UNPAIR |
| `UNPAIRING` | 正在取消配对... | Unpairing... |
| `UNPAIRING_COMPLETE` | 您的设备已经解除绑定 | Your device has been unpaired |
| `UNPAIRING_FAILED` | 取消配对失败 | Unpairing Failed |
| `UNPAIR_CONFIRM_TEXT` | 设备将从此 HyperSpeed 接收器上移除。 | Device will be removed from this HyperSpeed dongle. |
## 4. 本项目的实现状态

| 项 | 内容 |
|---|---|
| 本项目的页面 | src/pages/pairing.rs —— 已实现（接收器 / 配对 / 取消配对） |
| 后端方案 | 方案 2：复用雷云原生引擎（`lighting_driver` / `RzLightingEngineApi` / `mapping_engine` / `simple_service`） |
| 证据等级 | 布局 `[前端]`（设备模块 CSS）、文案 `[文案]`（语言包）、设备归属 `[前端]`（设备模块常量块） |

> 尚未实现的界面在应用里走占位页；占位页列出该页**真实存在的 key**，不编造内容。


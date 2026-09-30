# 自定义（TAB_CUSTOMIZE）

> 本页由雷云自己的产品模块提取；本文件只记录原版事实，不记录本项目实现状态。
> 证据工具：`node .ref/tools/gen-screen-docs.js`

## 1. 出现在哪些设备上

| 设备 | productId | 设备类型 | 该设备标签页顺序 |
|---|---|---|---|
| Razer DeathAdder V3 Pro | 182 | 鼠标 | 第 1 个：自定义 · 性能 · 正在配对 · 校准 · 电源 · 滚动 |
| Blackwidow V4 Pro | 653 | 键盘 | 第 1 个：自定义 · 性能 · 灯光 · 电源 · 滚动 |
| RAZER KRAKEN BT SANRIO LIMITED EDITION | 777 | 耳机 | 第 1 个：自定义 · 灯光 · 校准 · 电源 · 声音 · 麦克风 |

标签页真实文案：**自定义**（key `TAB_CUSTOMIZE`）

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

该界面共 164 个布局类名，分 11 个分区。

**① 顶部：配置文件栏**（21）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `add-profile` | `left:6px; top:8px`<br>`height:36px; width:36px` | — |
| `app-profile-icon` | — | `text-transform:capitalize` |
| `custom-profile-bar` | `margin:0 0 0 20px; width:-webkit-fit-content; width:fit-content` | — |
| `dropdown-profile` | `align-items:center; display:flex; gap:10px; position:relative` | — |
| `profile-act` | `height:0; left:-1px; max-width:280px; min-width:155px; position:absolute; top:26px; width:-webkit-max-content; width:max-content`<br>`height:auto`<br>`align-items:center; display:flex; height:auto; padding:5px 6px`<br>`align-items:start; display:flex; flex-direction:column` | `background:#000; border:1px solid #5d5d5d; transition:height 0s,visibility 0s,opacity .1s linear`<br>`opacity:1`<br>`background-color:#0000; line-height:17px; text-transform:capitalize; transition:background-color .3s`<br>`background:#5d5d5d`<br>`background-color:#ffffff1a` |
| `profile-bar` | `justify-content:normal; margin:0; width:auto`<br>`align-items:center; flex-direction:row; height:26px; justify-content:center; margin:0 auto 10px; width:95%`<br>`align-items:center; flex:0 0 auto; position:relative`<br>`height:26px; margin:0 10px; width:26px` | `color:#ccc`<br>`font-size:14px; text-transform:uppercase`<br>`text-transform:capitalize`<br>`text-transform:none`<br>`background-image:url(../../static/media/icon_obm.888208d6.svg)` |
| `profile-bar-macro` | — | `color:#44d62c; font-size:14px; text-align:center` |
| `profile-bar-title` | — | — |
| `profile-color` | `height:8px; position:absolute; right:0; top:0; width:8px` | `border-radius:50%; color:var(--profile-color)`<br>`background:#44d62c`<br>`background:red`<br>`background:lime`<br>`background:blue` |
| `profile-del` | `left:auto`<br>`min-width:300px; top:42px`<br>`align-items:center; display:flex!important; flex-direction:column; padding:20px; position:absolute`<br>`position:absolute` | `border:1px solid #fd4949; border-radius:3px`<br>`background-color:#111; box-shadow:0 6px 10px 0 #0003; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`font-family:Roboto,sans-serif`<br>`color:#fd4949; text-align:center; text-transform:uppercase`<br>`font-weight:700` |
| `profile-dynamic-tips` | `align-items:center; display:flex!important; flex-direction:column; padding:20px; position:absolute`<br>`left:10px`<br>`height:auto; left:50%; padding:20px; top:40px; width:300px` | `background-color:#111; box-shadow:0 6px 10px 0 #0003; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px #0003; text-transform:none`<br>`color:#ccc; font-size:14px`<br>`opacity:1` |
| `profile-migration-icon` | `height:40px; width:40px` | `background-image:url(../../static/media/synapse_profile_migration.c3622288.svg); background-position:50%; background-repeat:no-repeat` |
| `profile-selected-tips` | `align-items:center; display:flex!important; flex-direction:column; padding:20px; position:absolute`<br>`left:10px`<br>`height:-webkit-fit-content; height:fit-content; top:40px; width:300px` | `background-color:#111; box-shadow:0 6px 10px 0 #0003; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px #0003; text-transform:none`<br>`opacity:1` |
| `profile-switching` | `align-items:start; display:flex; flex-direction:column; height:94px; justify-content:space-evenly` | `text-transform:uppercase` |
| `profile-switching-des` | `height:-webkit-fit-content; height:fit-content` | `color:#999` |
| `profile-tips` | `align-items:center; display:flex!important; flex-direction:column; padding:20px; position:absolute`<br>`left:10px`<br>`height:-webkit-fit-content; height:fit-content; top:40px; width:300px`<br>`height:auto; left:50%; padding:20px; top:40px; width:300px` | `background-color:#111; box-shadow:0 6px 10px 0 #0003; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px #0003; text-transform:none`<br>`opacity:1`<br>`color:#ccc; font-size:14px` |
| `profile-wrapper` | `display:flex; flex:1 0 25%`<br>`left:auto`<br>`display:flex`<br>`flex:1 1 25%` | — |
| `profileIcon` | `display:block; height:20px; width:20px`<br>`position:relative`<br>`display:block; height:20px; position:absolute; width:20px`<br>`left:0; position:absolute; top:28px; width:300px` | `background-position:50%; background-repeat:no-repeat; background-size:20px`<br>`background-image:url(../../static/media/icon_obm_1.b37eed4a.svg)`<br>`background-image:url(../../static/media/icon_obm_2.512f2e40.svg)`<br>`background-image:url(../../static/media/icon_obm_3.4350a917.svg)`<br>`background-image:url(../../static/media/icon_obm_4.379b007d.svg)` |
| `specific-profile` | `width:180px`<br>`position:relative; width:170px` | — |
| `specific-profile-interdevice` | `width:180px`<br>`position:relative; width:170px` | — |
| `synapse-profile-migration` | `align-items:center; display:flex; justify-content:center; position:relative`<br>`height:40px; width:40px`<br>`height:auto; left:-130px; min-height:91px; min-width:300px; padding:20px; position:absolute; top:48px; width:100%` | `background-image:url(../../static/media/synapse_profile_migration.c3622288.svg); background-position:50%; background-repeat:no-repeat`<br>`background-color:#111; border:1px solid #fd8611; border-radius:3px; text-align:center`<br>`font-size:14px` |

**② 弹窗：配置文件导入 / 导出 / 重置**（6）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `ImportExportModal_emptyProfile__N-nlD` | `flex-grow:1; padding:20px 50px 0` | `color:#fd4949; font-family:Roboto; font-size:14px; line-height:17px; text-align:center; text-transform:none` |
| `ImportExportModal_importProfileItem__-cpSh` | `position:relative`<br>`display:flex`<br>`left:130px; padding:8px 10px 10px; position:absolute; top:calc(100% + 10px); width:88px` | `background-color:#000; border:1px solid #5d5d5d; font-size:14px; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `empty-profile` | `height:27px; margin:0 10px; max-height:27px; padding:4px 5px; position:relative; width:100%` | `background-color:#0000; border:1px solid #515151; color:#ccc; font-size:14px; line-height:17px; text-transform:none; transition:opacity .3s,border .3s` |
| `import-profile-btn-group` | `display:inline-flex; flex-shrink:0; height:27px`<br>`height:100%; width:-webkit-fit-content; width:fit-content`<br>`align-items:center; display:flex; position:relative`<br>`left:-113px; padding:8px 10px 10px; position:absolute; top:-200%; width:300px` | `border:1px solid #0000004d; font-family:Roboto; font-size:12px; line-height:0px; text-align:center`<br>`background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-family:Roboto; font-size:14px; line-height:16px; opacity:0; text-transform:none; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `modal_emptyProfile__YmtfA` | `flex-grow:1; padding:20px 50px 0` | `color:#c8323c; font-family:Roboto; font-size:14px; line-height:17px` |
| `modal_importProfileItem__uXvd8` | `position:relative`<br>`display:flex`<br>`left:130px; padding:8px 10px 10px; position:absolute; top:calc(100% + 10px); width:88px` | `background-color:#000; border:1px solid #5d5d5d; font-size:14px; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |

**③ 弹窗：删除确认**（6）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `ConfirmDialog_cancelButton__Ifdx9` | `padding:10px 32px`<br>`padding:8px 16px; width:100%` | `background:#0000; border:1px solid #ffffff4d; border-radius:3px; color:#fff; cursor:pointer; font-size:14px; font-weight:600; letter-spacing:.5px; text-transform:uppercase; transition:all .2s`<br>`background:#ffffff0d; border-color:#ffffff80`<br>`font-size:12px` |
| `ConfirmDialog_dialogButtons__H8-TH` | `display:flex; gap:12px; justify-content:flex-end`<br>`gap:0; justify-content:center` | — |
| `ConfirmDialog_removeButton__hBxnw` | `padding:10px 32px`<br>`align-items:center; display:flex; height:27px; justify-content:center; padding:0; width:90px` | `background:#fd8611; border:none; border-radius:3px; color:#000; cursor:pointer; font-size:14px; font-weight:inherit; letter-spacing:.5px; text-transform:uppercase; transition:all .2s`<br>`background:#ff9530`<br>`color:#222; font-size:12px` |
| `keymap-delete-alert` | `max-width:330px` | — |
| `s3-modal--action-remove` | — | `color:#fd4949; font-size:16px; line-height:16px; text-transform:uppercase`<br>`font-size:14px; text-align:center` |
| `s3-modal--button` | `height:24px; position:absolute; right:0; top:0; width:24px` | `background-image:url(../../static/media/icon_close.55fe41f1.svg); background-position:50%; background-size:24px 24px` |

**④ 按键层切换：标准 / Hypershift**（10）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `RAZER_HYPERSHIFT` | — | `background-image:url(../../static/media/icon_config_hypershift.6c83f48a.svg)`<br>`background-image:url(../../static/media/icon_config_hypershift_a.33a591f6.svg)` |
| `hypershift` | — | `color:#ccc`<br>`background-color:#fd8611; border-radius:12px; color:#212121`<br>`background-color:#44d62c` |
| `hypershift-mode` | — | `background:#fd861133` |
| `hypershift-mode-tip` | `max-width:300px; top:19px`<br>`【@media screen and (max-width:1279px)】left:auto; right:1vw; top:26px` | — |
| `main-keymap` | `width:100%`<br>`display:inline-block; height:100%` | `border-left:none; border-top:none` |
| `primary-keymap` | `min-height:562px; position:-webkit-sticky; position:sticky` | — |
| `razer_hypershift` | — | `stroke:#69696c`<br>`fill:#c8323c80` |
| `secondary-keymap` | `display:inline-block; height:100%`<br>`display:none; position:relative`<br>`display:inline; display:initial; width:292px` | `border-left:none; border-top:none`<br>`background-color:#111; opacity:0; transition:visibility 0s,opacity .2s,left 0s`<br>`border:1px solid #5d5d5d; box-shadow:0 0 20px 0 #000000b3; cursor:default; opacity:1` |
| `secondary-keymap-extra` | `position:static; width:auto`<br>`width:auto`<br>`margin:0`<br>`margin:0 20px` | `border-left:1px solid #5d5d5d` |
| `standard` | — | `background-color:#44d62c; border-radius:12px`<br>`background-color:#0000; color:#ccc` |

**⑤ 设备图形与可点按键位**（19）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `disableKeypadButton` | — | `fill:#0000!important`<br>`fill:#44d62c66` |
| `is-keypad` | `【@media screen and (max-width:1279px)】left:0; right:auto` | — |
| `key-light-device` | — | `background-image:url(/synapse/assets/imgs/favicon/IOT_KEY_LIGHT.svg)` |
| `keymap-bar` | `justify-content:left; margin:0 0 0 20px`<br>`margin:0 0 0 20px; width:-webkit-fit-content; width:fit-content`<br>`align-items:center; display:flex; flex-direction:row; height:26px; justify-content:center; margin:20px 0 0 17px; width:100%`<br>`right:auto` | `color:#ccc`<br>`font-size:14px; text-transform:uppercase`<br>`text-transform:none` |
| `keymap-bar-icon` | `height:20px; width:20px`<br>`【@media screen and (max-width:1279px)】left:0; right:auto` | `background-image:url(../../static/media/icon_config_keymap_ccc.9255af63.svg); background-position:50%; background-repeat:no-repeat; background-size:14px; border:2px solid #ccc; border-radius:50%` |
| `keymap-bar-menu-wrap` | `flex:0 0 auto; position:relative` | — |
| `keymap-bar-name` | `display:none` | — |
| `keymap-component` | `height:100%`<br>`max-height:calc(100% - 35px)`<br>`【@media(max-height:570px)】height:320px; overflow-y:auto` | — |
| `keymap-head` | `top:0; width:270px`<br>`left:75%; right:auto; width:100%`<br>`width:600px`<br>`height:36px; left:-1px; padding:10px 0 9px; position:absolute; top:-36px; width:292px` | `text-transform:capitalize`<br>`background-color:#222; border:1px solid #5d5d5d; border-radius:5px 5px 0 0; color:#999; font-size:14px; line-height:17px; text-align:center`<br>`background-color:#000; border:1px solid #5d5d5d`<br>`border:none; border-bottom:1px solid #5d5d5d`<br>`background-color:#0000; background-image:url(../../static/media/icon_close.55fe41f1.svg); background-position:50%; background-repeat:no-repeat; background-size:20px; transition:background-color .2s` |
| `keymap-tip` | `display:none; height:auto; left:calc(50% - 105px); max-width:-webkit-max-content; max-width:max-content; padding:10px; position:absolute; top:35px`<br>`display:block` | `background-color:#000; border:1px solid #5d5d5d` |
| `keymap1` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap2` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap3` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap4` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap5` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap6` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap7` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap8` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymapbar-enabled` | `justify-content:left; margin:0 0 0 20px`<br>`display:flex`<br>`flex:1 1 auto`<br>`display:none` | `color:#ccc` |

**⑥ 按键指派（选中键 → 选动作）**（15）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `action` | `align-items:center; display:flex; height:auto; padding:5px 6px`<br>`align-items:start; display:flex; flex-direction:column`<br>`height:20px`<br>`display:flex; gap:20px; justify-content:center` | `text-transform:uppercase`<br>`background-color:#000; color:#44d62c`<br>`background-color:#1a1a1a; color:#44d62c`<br>`background-color:#0000; line-height:17px; text-transform:capitalize; transition:background-color .3s`<br>`background-color:#ffffff1a` |
| `action-wrapper` | `height:40px; position:relative`<br>`height:20px; left:10px; position:absolute; top:10px; width:20px` | `background-color:#0000; transition:background-color .2s`<br>`color:#ccc; font-size:12px; line-height:14px; text-align:left; text-transform:uppercase; transition:color .2s`<br>`background-color:#393939`<br>`background-color:#111`<br>`color:#44d62c` |
| `action_bar_wrapper` | `display:flex`<br>`bottom:-38px`<br>`height:20px; position:relative; width:20px`<br>`height:20px; width:20px` | `background-image:url(../../static/media/icon_emoji.07a71bf8.svg)`<br>`background-image:url(../../static/media/icon_emoji_active.b1a4efad.svg)`<br>`opacity:1`<br>`background-image:url(../../static/media/icon_close.130e45fb.svg)`<br>`background-image:url(../../static/media/icon_charactermap.025aae0b.svg)` |
| `actions` | `height:100%; max-width:250px; position:absolute; width:250px`<br>`max-width:40px`<br>`overflow:hidden`<br>`overflow:auto` | `background-color:#222; transition:max-width .2s` |
| `binding-value` | `overflow:hidden` | `line-height:16px` |
| `hoverGray-remappedBlack` | — | `fill:#000c`<br>`stroke:#000` |
| `isAssignment` | — | `fill:#44d62c66`<br>`fill:#000c`<br>`fill:#fd8611b3!important`<br>`fill:#fd8611b3` |
| `key-config` | `width:180px`<br>`position:relative; width:170px`<br>`display:flex; flex-direction:column; left:-121px; max-height:calc(100vh - 150px); min-height:260px; top:28px; width:270px`<br>`top:0; width:270px` | `border-top:initial`<br>`text-transform:capitalize`<br>`opacity:.3`<br>`background-color:#111; opacity:0; transition:visibility 0s,opacity 0s,left 0s`<br>`border:1px solid #5d5d5d; box-shadow:0 0 20px 0 #000000b3; cursor:default; opacity:1` |
| `key-config-flex` | `display:flex` | — |
| `key-mapping` | `display:none`<br>`overflow:hidden; padding:0`<br>`overflow:hidden`<br>`overflow:auto` | `line-height:40px` |
| `key_mapping_text_display` | `overflow:hidden` | — |
| `keymap-action` | `padding:0 20px; width:250px`<br>`flex:0 0 auto; height:27px; margin:0 10px 0 0; min-width:100px; padding:6px 10px 7px 6px`<br>`padding:0`<br>`justify-content:flex-end` | `border:1px solid #000; border-radius:3px; font-size:12px; line-height:14px; text-align:center` |
| `notRemapped` | — | `fill:#fd8611b3`<br>`stroke:#44d62c; fill:#0000`<br>`stroke:#fd8611`<br>`stroke:#69696c`<br>`fill:#c8323c80` |
| `remap-2-disabled` | — | `color:#c8323c` |
| `remapped` | `width:164px` | `color:#44d62c`<br>`color:#fd8611`<br>`border-left:1px solid #707070; font-size:14px; line-height:16px; text-align:left` |

**⑦ 预设动作快捷按钮**（9）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `collapse-action` | — | — |
| `custom-action-btn` | `margin:0 auto`<br>`width:486px` | — |
| `flex-button` | `align-items:flex-start; display:flex; flex-direction:column; padding:2px 0` | — |
| `group-button` | `align-items:center; display:flex; height:32px; margin:0 0 10px 30px; padding:5px 3px; width:-webkit-fit-content; width:fit-content`<br>`height:22px; margin:0 2px; min-width:40px; padding:4px 8px` | `border:1px solid gray; border-radius:20px`<br>`border:0; border-radius:20px; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; line-height:14px; text-transform:uppercase`<br>`background-color:#44d62c; color:#212121` |
| `setting-action` | `align-items:center; display:flex`<br>`margin:0 5px` | — |
| `shortcutButton` | `align-items:center; display:flex; height:100%; justify-content:center; margin:0 10px; width:47px` | `border:1px solid #5d5d5d; border-radius:3px` |
| `shortcutButton-filled` | — | `background-color:#292929` |
| `shortcut_act_action` | `height:27px; padding:5px 6px` | `background-color:#0000; line-height:17px; text-transform:capitalize; transition:background-color .3s`<br>`background-color:#222` |
| `shortcut_item_action` | `align-items:center; display:flex; position:relative` | — |

**⑧ 键盘独有：组合键（两键同按）**（1）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `twoKeyMapping` | `height:20px; position:absolute; top:15px; width:20px` | `background-image:url(../../static/media/icon_secondaryfunction-1.734f766c.svg); background-size:20px 20px`<br>`color:#707070; font-size:14px; line-height:14px; text-transform:uppercase` |

**⑩ 键盘独有：动态击键 / 按键录制**（3）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `key-dks` | `position:absolute`<br>`height:10px; width:19px` | `background-size:cover`<br>`background-image:url(../../static/media/dynamic_key_stroke_icon.c4bd98bb.svg)` |
| `macro-keypad-module` | `min-height:auto`<br>`position:static`<br>`position:absolute`<br>`flex-direction:column` | `font-size:14px`<br>`text-transform:capitalize`<br>`background-color:#000; border:1px solid #5d5d5d; border-radius:20px`<br>`background-color:#3cbf27; border-radius:20px; color:#212121`<br>`opacity:1` |
| `marco-keymap` | — | — |

**⑪ 通用控件**（27）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `button` | `align-items:center; display:flex; flex:none; flex-direction:row; flex-grow:0; gap:10px; height:27px; justify-content:center; order:2; padding:7px 16px 6px; width:173px`<br>`align-items:center; display:flex; flex:none; flex-direction:row; flex-grow:0; gap:10px; height:27px; justify-content:center; order:5; padding:7px 16px 6px; width:173px`<br>`margin:9px auto 0; padding:7px 16px; width:-webkit-fit-content; width:fit-content` | `background:#707070; border:1px solid #0000004d; border-radius:3px; font-family:Roboto; font-size:12px; font-weight:400; line-height:14px; text-align:center; text-transform:uppercase`<br>`background-color:#ffffff1a`<br>`background:#44d62c; border:1px solid #0000004d; border-radius:3px; color:#222; font-family:Roboto; font-size:12px; font-weight:400; line-height:14px; text-align:center; text-transform:uppercase`<br>`opacity:.8`<br>`background-color:#707070; border-radius:3px; cursor:pointer; text-transform:uppercase` |
| `button--cta` | `margin:9px auto 0; padding:7px 16px; width:-webkit-fit-content; width:fit-content`<br>`height:18px; position:relative; width:18px`<br>`left:0; max-width:300px; width:-webkit-max-content; width:max-content` | `background-color:#707070; border-radius:3px; cursor:pointer; text-transform:uppercase`<br>`background-position:50%; background-repeat:no-repeat; background-size:18px 18px`<br>`opacity:1` |
| `button--cta-close` | `height:24px; position:absolute; right:0; top:0; width:24px` | `background-image:url(../../static/media/icon_close.55fe41f1.svg); background-position:50%; background-size:24px 24px` |
| `button--cta-get-control` | `align-items:center; display:flex; height:27px; justify-content:center; margin:9px auto 0; padding:0 16px; width:-webkit-fit-content; width:fit-content` | `background-color:#707070; border-radius:3px; color:#ccc; color:#fff; cursor:pointer; font-size:14px; text-transform:uppercase` |
| `button--cta-link` | `left:0; max-width:300px; width:-webkit-max-content; width:max-content` | `background-image:url(../../static/media/unlink-btn.db633bad.svg)`<br>`opacity:1` |
| `button--cta-link-link` | — | `background-image:url(../../static/media/unlink-btn.db633bad.svg)`<br>`background-image:url(../../static/media/unlink-hovered-btn.67ac688d.svg)` |
| `button--cta-link-unlink` | — | `background-image:url(../../static/media/link-btn.fed18bb7.svg)`<br>`background-image:url(../../static/media/link-hovered-btn.0fde7f57.svg)` |
| `button--cta-remove` | `display:block; height:27px; margin:0 auto; min-width:90px; width:-webkit-fit-content; width:fit-content` | `background-color:#fd4949; border:1px solid #0000004d; border-radius:3px; color:#111; cursor:pointer; font-size:12px; line-height:14px; text-align:center; text-transform:uppercase` |
| `button--cta-show-active` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 15px 0 0; padding:0 16px; width:-webkit-fit-content; width:fit-content` | `background-color:#292929; border:1px solid #44d62c; border-radius:3px; color:#ccc; cursor:pointer; font-size:14px; text-transform:uppercase` |
| `button--cta-show-busy` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 auto; padding:0 16px; width:-webkit-fit-content; width:fit-content` | `border:1px solid #fd8611; border-radius:3px; color:#ccc; color:#fd8611; cursor:pointer; font-size:14px; text-transform:uppercase` |
| `button--cta-show-locked` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 10px 0 auto; padding:0 16px 0 40px; position:relative; width:-webkit-fit-content; width:fit-content`<br>`display:block; height:24px; left:0; position:absolute; top:0; width:24px` | `border:1px solid #e5e5e5; border-radius:3px; color:#ccc; color:#e5e5e5; cursor:pointer; font-size:14px; text-transform:uppercase`<br>`background-image:url(../../static/media/icon_lock_white.c066cd4d.svg); background-position:100%; background-repeat:no-repeat; background-size:70% 70%` |
| `button--cta-unlink` | — | `background-image:url(../../static/media/link-btn.fed18bb7.svg)`<br>`background-image:url(../../static/media/link-hovered-btn.0fde7f57.svg)` |
| `button-description` | `max-width:250px; overflow:hidden`<br>`align-items:flex-start; display:flex; flex-direction:column; padding:2px 0` | `color:#44d62c; font-size:10px; line-height:12px; text-transform:uppercase`<br>`color:#fd8611` |
| `button-group` | `display:flex; flex-wrap:wrap; gap:10px`<br>`height:70px; min-width:63px; padding:40px 0 0; width:63px` | `background-color:#111; background-repeat:no-repeat; border:1px solid gray; color:#ccc; font-family:Roboto,sans-serif; font-size:10px; line-height:0; text-align:center; text-transform:none`<br>`border:1px solid #44d62c`<br>`background-image:url(../../static/media/standard.cbf1362d.svg)`<br>`background-image:url(../../static/media/fast.43db44c4.svg)`<br>`background-image:url(../../static/media/slow.8b9c0df7.svg)` |
| `button_icon` | `align-items:center; display:flex` | `color:#707070; font-size:14px; line-height:17px; text-align:left`<br>`fill:#fff`<br>`fill:#44d62c` |
| `buttons-block` | `align-items:flex-start; display:flex; flex-direction:column; gap:16px; height:97px; padding:0 50px 50px; width:480px`<br>`align-items:flex-start; display:flex; flex-direction:row; gap:12px; height:4px; padding:0`<br>`height:4px; position:relative; width:4px`<br>`align-items:flex-start; display:flex; flex-direction:row; gap:10px; height:27px; padding:0; width:190px` | `opacity:1`<br>`background-color:#707070; border-radius:10px; transition:none`<br>`background-color:#fff`<br>`background:#707070; border:1px solid #0000004d; border-radius:3px; color:#fff; cursor:pointer; font-family:Roboto; font-size:12px; font-weight:400; line-height:14px; opacity:1; text-align:center; text-transform:uppercase; transition:background-color .3s,opacity .3s`<br>`background-color:#8a8a8a` |
| `buttons-row` | `align-items:flex-start; display:flex; flex-direction:row; gap:10px; height:27px; padding:0; width:190px` | — |
| `enable-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:90px` | `background-color:#44d62c; border-radius:3px; color:#212121; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `icon-button` | — | `fill:#44d62c`<br>`fill:#44d62c; opacity:.7` |
| `icon-button-duallink` | `height:28px; margin:auto; min-width:90px; width:-webkit-max-content; width:max-content`<br>`display:block; height:28px; padding:0 5px` | `text-transform:uppercase`<br>`background-color:#44d62c; border-radius:2px; color:#212121; font-family:Roboto; font-size:12px; line-height:28px; text-align:center` |
| `icon-button-text-duallink` | `display:block; height:28px; padding:0 5px` | `text-transform:uppercase`<br>`background-color:#44d62c; border-radius:2px; color:#212121; font-family:Roboto; font-size:12px; line-height:28px; text-align:center` |
| `login-button` | `height:27px; min-width:90px; padding:7px 16px 6px; width:-webkit-max-content; width:max-content` | `background-color:#44d62c; border-radius:3px; color:#111; cursor:pointer; text-align:center; text-transform:uppercase; transition:all .3s ease-out`<br>`background-color:#44d62cb3`<br>`background-color:#fff` |
| `ok-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:90px` | `background-color:#707070; border-radius:3px; color:#ccc; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `razer-button` | `min-width:90px; position:relative; width:-webkit-fit-content; width:fit-content`<br>`height:100%; left:0; position:absolute; top:0; width:100%` | `background-color:#44d62c; border:1px solid #000; border-radius:3px; color:#000; cursor:default; line-height:27px; text-transform:uppercase`<br>`background:#ffffff4d; border-radius:3px`<br>`background:#44d62cb3!important`<br>`background:none` |
| `switch-button` | `height:30px; width:90px` | `background-color:#333; border:1px solid #555; color:#ccc; cursor:pointer; font-size:14px; text-transform:uppercase; transition:background-color .3s,color .3s`<br>`background-color:#44d62c; border-color:#44d62c; color:#111`<br>`background-color:#444` |
| `switch_button` | `height:22px; margin:0 2px; min-width:40px; padding:4px 8px`<br>`align-items:center; display:flex; height:27px; justify-content:center; margin:0; min-width:90px; padding:0 16px` | `border:0; border-radius:20px; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; line-height:14px; text-transform:uppercase`<br>`background-color:#44d62c; color:#212121`<br>`background-color:#111; border:1px solid #5d5d5d; border-radius:3px; cursor:pointer; font-size:12px; text-transform:uppercase`<br>`border-color:#44d62c`<br>`background-color:#ffffff1a` |
| `tooltip-panel-button` | `left:61px; max-width:358px; padding:8px 10px; position:absolute` | `background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-size:14px; line-height:16px; opacity:0; text-align:left`<br>`opacity:1` |

**⑫ 其它**（47）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `AudioMeter_flowButtonActive__vq7uT` | — | `background:#292929; border:1px solid #44d62c` |
| `AudioMeter_flowButtonIcon__p7D6V` | `height:18px; width:18px` | — |
| `AudioMeter_flowButton__4sIhG` | `align-items:center; display:flex; flex-direction:column; gap:5px; height:66px; justify-content:center; min-width:160px; padding:7px 16px 6px; width:160px`<br>`height:18px; width:18px` | `background:#111; border:1px solid #5d5d5d; border-radius:3px; color:#ccc; cursor:pointer; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; opacity:1; text-transform:uppercase; transition:all .2s ease` |
| `AudioMeter_flowButtons__tOGNz` | `display:flex; gap:5px` | — |
| `Duallink_unpairbutton__lTG1W` | `display:block; height:28px; left:50%; padding:0 10px; position:absolute; top:50%`<br>`display:block; height:28px; min-width:90px; padding:0 5px` | `background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; text-align:center`<br>`background-color:#9b9b9b`<br>`background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; opacity:.8; text-align:center`<br>`opacity:1` |
| `Duallink_unpairbuttondisabled__ZdDMy` | `display:block; height:28px; left:50%; padding:0 10px; position:absolute; top:50%` | `background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; opacity:.5; text-align:center` |
| `HelpComponent_supportButton__S0UnH` | — | `cursor:pointer`<br>`color:#44d62c` |
| `MonitoringDashboard_menuButton__IkMX-` | `align-items:center; display:flex; justify-content:center; padding:0; position:relative` | `background:none; border:1px solid #0000; border-radius:0; color:#ccc; cursor:pointer`<br>`outline:1px dotted #0f0`<br>`border-color:#5d5d5d`<br>`color:#999`<br>`color:#fff` |
| `MonitoringToggle_button__xH4PT` | `align-items:center; display:flex; justify-content:center; padding:4px` | `background:none; border:1px solid #0000; border-radius:0; color:#999; cursor:pointer; transition:all .2s ease`<br>`border-color:#5d5d5d; color:#fff`<br>`outline:1px dotted lime` |
| `SWITCH_DEVICE_PROFILE` | — | `background-image:url(../../static/media/icon_config_switch_device_profile.88b9f2aa.svg)`<br>`background-image:url(../../static/media/icon_config_switch_device_profile_a.942d360a.svg)` |
| `SWITCH_KEYMAP` | — | `background-image:url(../../static/media/icon_config_keymap.cb5ba1f0.svg)`<br>`background-image:url(../../static/media/icon_config_keymap_a.e27c970d.svg)` |
| `SWITCH_PROFILE` | — | `background-image:url(../../static/media/icon_config_switch_device_profile.88b9f2aa.svg)`<br>`background-image:url(../../static/media/icon_config_switch_device_profile_a.942d360a.svg)` |
| `WaveWithSpeedV2_waveButtons__vAgEX` | `display:grid; gap:20px` | — |
| `add-keymap` | `display:flex; max-width:100%; min-height:25px; overflow:hidden; padding:4px 5px; width:100%`<br>`padding:0 4px` | `border-top:1px solid #515151; font-size:14px; text-transform:none`<br>`background-color:#ffffff1a` |
| `box-bindingdevice-tag` | `display:block; height:auto; position:block; width:100%` | `color:#44d62c; font-size:14px; line-height:16px; text-align:center; transition:color .2s` |
| `box-scanselect-button` | `display:inline-block; height:28px; width:90px` | `border:1px solid #000; border-radius:2px; border-radius:3px; font-family:Roboto; font-size:12px; line-height:28px; text-align:center; text-transform:uppercase; transition:opacity .3s` |
| `box-unbindingdevice-tag` | `display:block; height:auto; position:block; width:100%` | `color:#44d62c; font-size:14px; line-height:16px; text-align:center; transition:color .2s` |
| `box-unpairselect-button` | `display:inline-block; height:28px; width:90px`<br>`min-width:-webkit-max-content; min-width:max-content; padding:0 10px; position:relative` | `border:1px solid #000; border-radius:2px; border-radius:3px; font-family:Roboto; font-size:12px; line-height:28px; text-align:center; text-transform:uppercase; transition:opacity .3s` |
| `config-profile-img` | `position:absolute; right:calc(50% - 187px); top:-77px` | — |
| `custom-keymapping-stepper` | `width:90%` | `border:1px solid #5d5d5d`<br>`border:1px solid #44d62c`<br>`background-color:initial; text-align:left; text-align:initial`<br>`opacity:1` |
| `customize-polling-rate-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:72px`<br>`min-width:90px` | `background-color:#222; border:1px solid #5d5d5d; border-radius:3px; color:#ccc; font-size:14px; text-transform:uppercase`<br>`border-color:#44d62c` |
| `customize-polling-rate-button-color` | `height:6px; width:6px` | `border-radius:50%` |
| `customize-setting-button` | `width:150px`<br>`align-items:center; display:flex; height:27px; justify-content:center; min-width:90px` | `background-color:#707070; border-radius:3px; color:#fff; font-size:12px; text-transform:uppercase`<br>`opacity:.3` |
| `installation-action` | `margin:0 0 0 30px`<br>`height:27px; width:auto` | `opacity:.6` |
| `key-require-macro-module` | `height:20px; position:absolute; width:20px` | `background-size:cover`<br>`background-image:url(../../static/media/require_macro_module_icon.20694b5c.svg)` |
| `key-require-synapse` | `height:20px; position:absolute; width:20px` | `background-image:url(../../static/media/require_synapse_icon.d5e656b0.svg)`<br>`background-size:cover` |
| `key-snap-tap` | `height:16px; width:16px`<br>`position:absolute` | `background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg)`<br>`background-size:cover` |
| `key-tip` | `max-width:300px; min-height:75px; padding:8px 10px; position:absolute`<br>`overflow:hidden!important`<br>`padding:initial` | `background-color:#000; border:1px solid #5d5d5d; opacity:0; text-align:center; transition:opacity .3s,visibility 0s,left 0s,top 0s`<br>`opacity:1`<br>`color:#707070; font-size:14px`<br>`line-height:14px; text-transform:uppercase`<br>`color:#ccc; font-size:12px` |
| `message-container__button` | — | `background-color:#44d62c; color:#111` |
| `modal_importProfileInfoItem__GyCiB` | `align-items:center; display:flex; height:31px; position:relative`<br>`align-items:center; display:flex; height:100%; position:relative; width:20px`<br>`padding:8px 10px 10px; position:absolute; top:100%`<br>`align-self:stretch; width:3px` | `color:#ccc; font-family:Roboto; font-size:14px`<br>`background-color:#000; border:1px solid #5d5d5d; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `no-chroma-studio-profile` | — | `color:#44d62c` |
| `optionButtonGroup_active__-z-LG` | — | `background:#292929; border-color:#44d62c`<br>`background:#292929; border:1px solid #44d62c` |
| `optionButtonGroup_disabled__Q0Wrc` | — | `background:#ffffff1a`<br>`background:#292929; border:1px solid #44d62c`<br>`color:#ffffff80; cursor:not-allowed`<br>`opacity:.5` |
| `optionButtonGroup_effect-with-duration__PkoU2` | `display:flex; flex-direction:column` | `opacity:.5` |
| `optionButtonGroup_effects-area__8iBAF` | `display:flex; flex-direction:column`<br>`align-items:flex-start` | — |
| `optionButtonGroup_option-button-group__TUmjj` | `display:flex; gap:5px` | — |
| `optionButtonGroup_option-button__C62Cv` | `height:27px; margin:0; min-width:90px; padding:7px 16px 6px` | `background:#0000; border:1px solid #5d5d5d; border-radius:3px; color:#ffffffe6; cursor:pointer; font-family:Roboto; font-size:12px; font-weight:400; letter-spacing:0; line-height:14px; outline:none; text-align:center; text-transform:uppercase; transition:all .2s ease`<br>`background:#ffffff1a`<br>`background:#292929; border:1px solid #44d62c`<br>`color:#ffffff80; cursor:not-allowed` |
| `optionButtonGroup_option-label__J0Xc0` | — | `font-family:Roboto; font-size:14px; font-weight:400; line-height:14px; text-transform:uppercase` |
| `optionButtonGroup_random-color__wSIuX` | — | — |
| `optionButtonGroup_with-option-buttons__gEnlO` | `display:flex; flex-direction:column`<br>`align-items:flex-start` | — |
| `remove-text-keymapping` | `height:16px; position:absolute; right:5px; top:5px; width:16px` | `background-image:url(../../static/media/icon_close_enclosed.6056b667.svg); background-position:50%; background-repeat:no-repeat; background-size:20px; transition:background-image .2s`<br>`background-image:url(../../static/media/icon_close_enclosed_a.83aff4bb.svg); cursor:pointer` |
| `s3-modal--action-take-control` | — | `border:1px solid #707070`<br>`text-align:left` |
| `secondary-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:90px` | `background-color:#707070; border-radius:3px; color:#ccc; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `show-all-button` | — | `cursor:pointer; font-size:14px; text-align:center; text-transform:capitalize`<br>`color:#44d62c` |
| `show-all-button-arrow` | `height:20px; width:20px` | `transition:fill .2s ease` |
| `sliderChart__reset-button` | `height:300px; min-width:20px; right:40px` | `background:url(../../static/media/eq_reset.e0c3c09c.svg) no-repeat 50%; background-size:20px 20px; opacity:.85; transition:background-image .3s`<br>`background-image:url(../../static/media/eq_reset_hover.186df33c.svg)`<br>`background-image:url(../../static/media/eq_reset_active.37c570d3.svg)` |
| `text-standard-appli` | — | `color:#707070; font-size:12px; line-height:5px` |

#### Blackwidow V4 Pro（productId 653）

该界面共 216 个布局类名，分 12 个分区。

**① 顶部：配置文件栏**（21）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `add-profile` | `left:6px; top:8px`<br>`height:36px; width:36px` | — |
| `app-profile-icon` | — | `text-transform:capitalize` |
| `custom-profile-bar` | `margin:0 0 0 20px; width:-webkit-fit-content; width:fit-content` | — |
| `dropdown-profile` | `align-items:center; display:flex; gap:10px; position:relative` | — |
| `profile-act` | `height:0; left:-1px; max-width:280px; min-width:155px; position:absolute; top:26px; width:-webkit-max-content; width:max-content`<br>`height:auto`<br>`align-items:center; display:flex; height:auto; padding:5px 6px`<br>`align-items:start; display:flex; flex-direction:column` | `background:#000; border:1px solid #5d5d5d; transition:height 0s,visibility 0s,opacity .1s linear`<br>`opacity:1`<br>`background-color:#0000; line-height:17px; text-transform:capitalize; transition:background-color .3s`<br>`background:#5d5d5d`<br>`background-color:#ffffff1a` |
| `profile-bar` | `justify-content:normal; margin:0; width:auto`<br>`align-items:center; flex-direction:row; height:26px; justify-content:center; margin:0 auto 10px; width:95%`<br>`align-items:center; flex:0 0 auto; position:relative`<br>`height:26px; margin:0 10px; width:26px` | `color:#ccc`<br>`font-size:14px; text-transform:uppercase`<br>`text-transform:capitalize`<br>`text-transform:none`<br>`background-image:url(../../static/media/icon_obm.888208d6.svg)` |
| `profile-bar-macro` | — | `color:#44d62c; font-size:14px; text-align:center` |
| `profile-bar-title` | — | — |
| `profile-color` | `height:8px; position:absolute; right:0; top:0; width:8px` | `border-radius:50%; color:var(--profile-color)`<br>`background:#44d62c`<br>`background:red`<br>`background:lime`<br>`background:blue` |
| `profile-del` | `left:auto`<br>`min-width:300px; top:42px`<br>`align-items:center; display:flex!important; flex-direction:column; padding:20px; position:absolute`<br>`position:absolute` | `border:1px solid #fd4949; border-radius:3px`<br>`background-color:#111; box-shadow:0 6px 10px 0 #0003; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`font-family:Roboto,sans-serif`<br>`color:#fd4949; text-align:center; text-transform:uppercase`<br>`font-weight:700` |
| `profile-dynamic-tips` | `align-items:center; display:flex!important; flex-direction:column; padding:20px; position:absolute`<br>`left:10px`<br>`height:auto; left:50%; padding:20px; top:40px; width:300px` | `background-color:#111; box-shadow:0 6px 10px 0 #0003; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px #0003; text-transform:none`<br>`color:#ccc; font-size:14px`<br>`opacity:1` |
| `profile-migration-icon` | `height:40px; width:40px` | `background-image:url(../../static/media/synapse_profile_migration.c3622288.svg); background-position:50%; background-repeat:no-repeat` |
| `profile-selected-tips` | `align-items:center; display:flex!important; flex-direction:column; padding:20px; position:absolute`<br>`left:10px`<br>`height:-webkit-fit-content; height:fit-content; top:40px; width:300px` | `background-color:#111; box-shadow:0 6px 10px 0 #0003; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px #0003; text-transform:none`<br>`opacity:1` |
| `profile-switching` | `align-items:start; display:flex; flex-direction:column; height:94px; justify-content:space-evenly` | `text-transform:uppercase` |
| `profile-switching-des` | `height:-webkit-fit-content; height:fit-content` | `color:#999` |
| `profile-tips` | `align-items:center; display:flex!important; flex-direction:column; padding:20px; position:absolute`<br>`left:10px`<br>`height:-webkit-fit-content; height:fit-content; top:40px; width:300px`<br>`height:auto; left:50%; padding:20px; top:40px; width:300px` | `background-color:#111; box-shadow:0 6px 10px 0 #0003; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px #0003; text-transform:none`<br>`opacity:1`<br>`color:#ccc; font-size:14px` |
| `profile-wrapper` | `display:flex; flex:1 0 25%`<br>`left:auto`<br>`display:flex`<br>`flex:1 1 25%` | — |
| `profileIcon` | `display:block; height:20px; width:20px`<br>`position:relative`<br>`display:block; height:20px; position:absolute; width:20px`<br>`left:0; position:absolute; top:28px; width:300px` | `background-position:50%; background-repeat:no-repeat; background-size:20px`<br>`background-image:url(../../static/media/icon_obm_1.b37eed4a.svg)`<br>`background-image:url(../../static/media/icon_obm_2.512f2e40.svg)`<br>`background-image:url(../../static/media/icon_obm_3.4350a917.svg)`<br>`background-image:url(../../static/media/icon_obm_4.379b007d.svg)` |
| `specific-profile` | `width:180px`<br>`position:relative; width:170px` | — |
| `specific-profile-interdevice` | `width:180px`<br>`position:relative; width:170px` | — |
| `synapse-profile-migration` | `align-items:center; display:flex; justify-content:center; position:relative`<br>`height:40px; width:40px`<br>`height:auto; left:-130px; min-height:91px; min-width:300px; padding:20px; position:absolute; top:48px; width:100%` | `background-image:url(../../static/media/synapse_profile_migration.c3622288.svg); background-position:50%; background-repeat:no-repeat`<br>`background-color:#111; border:1px solid #fd8611; border-radius:3px; text-align:center`<br>`font-size:14px` |

**② 弹窗：配置文件导入 / 导出 / 重置**（6）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `ImportExportModal_emptyProfile__N-nlD` | `flex-grow:1; padding:20px 50px 0` | `color:#fd4949; font-family:Roboto; font-size:14px; line-height:17px; text-align:center; text-transform:none` |
| `ImportExportModal_importProfileItem__-cpSh` | `position:relative`<br>`display:flex`<br>`left:130px; padding:8px 10px 10px; position:absolute; top:calc(100% + 10px); width:88px` | `background-color:#000; border:1px solid #5d5d5d; font-size:14px; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `empty-profile` | `height:27px; margin:0 10px; max-height:27px; padding:4px 5px; position:relative; width:100%` | `background-color:#0000; border:1px solid #515151; color:#ccc; font-size:14px; line-height:17px; text-transform:none; transition:opacity .3s,border .3s` |
| `import-profile-btn-group` | `display:inline-flex; flex-shrink:0; height:27px`<br>`height:100%; width:-webkit-fit-content; width:fit-content`<br>`align-items:center; display:flex; position:relative`<br>`left:-113px; padding:8px 10px 10px; position:absolute; top:-200%; width:300px` | `border:1px solid #0000004d; font-family:Roboto; font-size:12px; line-height:0px; text-align:center`<br>`background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-family:Roboto; font-size:14px; line-height:16px; opacity:0; text-transform:none; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `modal_emptyProfile__YmtfA` | `flex-grow:1; padding:20px 50px 0` | `color:#c8323c; font-family:Roboto; font-size:14px; line-height:17px` |
| `modal_importProfileItem__uXvd8` | `position:relative`<br>`display:flex`<br>`left:130px; padding:8px 10px 10px; position:absolute; top:calc(100% + 10px); width:88px` | `background-color:#000; border:1px solid #5d5d5d; font-size:14px; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |

**③ 弹窗：删除确认**（3）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `keymap-delete-alert` | `max-width:330px` | — |
| `s3-modal--action-remove` | — | `color:#fd4949; font-size:16px; line-height:16px; text-transform:uppercase`<br>`font-size:14px; text-align:center` |
| `s3-modal--button` | `height:24px; position:absolute; right:0; top:0; width:24px` | `background-image:url(../../static/media/icon_close.55fe41f1.svg); background-position:50%; background-size:24px 24px` |

**④ 按键层切换：标准 / Hypershift**（11）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `RAZER_HYPERSHIFT` | — | `background-image:url(../../static/media/icon_config_hypershift.6c83f48a.svg)`<br>`background-image:url(../../static/media/icon_config_hypershift_a.33a591f6.svg)` |
| `hypershift` | — | `color:#ccc`<br>`background-color:#fd8611; border-radius:12px; color:#212121` |
| `hypershift-mode` | — | `background:#fd861133` |
| `hypershift-mode-tip` | `max-width:300px; top:19px`<br>`【@media screen and (max-width:1279px)】left:auto; right:1vw; top:26px`<br>`【@media screen and (max-width:1279px)】left:auto; right:1vw` | — |
| `main-keymap` | `width:100%`<br>`display:inline-block; height:100%` | `border-left:none; border-top:none` |
| `primary-keymap` | `min-height:562px; position:-webkit-sticky; position:sticky` | — |
| `razer_hypershift` | — | `stroke:#69696c`<br>`fill:#c8323c80` |
| `secondary-keymap` | `display:inline-block; height:100%`<br>`display:none; position:relative`<br>`display:inline; display:initial; width:292px` | `border-left:none; border-top:none`<br>`background-color:#111; opacity:0; transition:visibility 0s,opacity .2s,left 0s`<br>`border:1px solid #5d5d5d; box-shadow:0 0 20px 0 #000000b3; cursor:default; opacity:1` |
| `secondary-keymap-extra` | `position:static; width:auto`<br>`width:auto`<br>`margin:0`<br>`margin:0 20px` | `border-left:1px solid #5d5d5d` |
| `standard` | — | `background-color:#44d62c; border-radius:12px`<br>`background-color:#0000; color:#ccc` |
| `standard-mode` | `display:inline-block; height:20px; min-height:20px; min-width:20px; position:relative; width:20px`<br>`align-items:center; display:inline-flex; height:60px; padding:0 20px; width:520px`<br>`display:none`<br>`margin:0` | `background-position:50%; background-repeat:no-repeat; background-size:contain`<br>`color:#ccc; font-size:14px`<br>`background-image:url(../../static/media/icon_info_solid.a4297bf0.svg)`<br>`background:#111 0 0 no-repeat padding-box; border:1px solid #3a3a3a; color:#ccc; font-size:14px`<br>`background-position:50%; background-repeat:no-repeat; background-size:contain; cursor:grab` |

**⑤ 设备图形与可点按键位**（19）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `disableKeypadButton` | — | `fill:#0000!important`<br>`fill:#44d62c66` |
| `is-keypad` | `【@media screen and (max-width:1279px)】left:0; right:auto` | — |
| `key-light-device` | — | `background-image:url(/synapse/assets/imgs/favicon/IOT_KEY_LIGHT.svg)` |
| `keymap-bar` | `justify-content:left; margin:0 0 0 20px`<br>`margin:0 0 0 20px; width:-webkit-fit-content; width:fit-content`<br>`align-items:center; display:flex; flex-direction:row; height:26px; justify-content:center; margin:20px 0 0 17px; width:100%`<br>`right:auto` | `color:#ccc`<br>`font-size:14px; text-transform:uppercase`<br>`text-transform:none` |
| `keymap-bar-icon` | `height:20px; width:20px`<br>`【@media screen and (max-width:1279px)】left:0; right:auto` | `background-image:url(../../static/media/icon_config_keymap_ccc.9255af63.svg); background-position:50%; background-repeat:no-repeat; background-size:14px; border:2px solid #ccc; border-radius:50%` |
| `keymap-bar-menu-wrap` | `flex:0 0 auto; position:relative` | — |
| `keymap-bar-name` | `display:none` | — |
| `keymap-component` | `height:100%`<br>`max-height:calc(100% - 35px)`<br>`【@media(max-height:570px)】height:320px; overflow-y:auto` | — |
| `keymap-head` | `top:0; width:270px`<br>`left:75%; right:auto; width:100%`<br>`width:600px`<br>`display:inline-block; max-width:200px; overflow:hidden!important` | `text-transform:capitalize`<br>`background-color:#000; border:1px solid #5d5d5d`<br>`border:none; border-bottom:1px solid #5d5d5d`<br>`border:1px solid #5d5d5d`<br>`background-color:#222; border:1px solid #5d5d5d; border-radius:5px 5px 0 0; color:#999; font-size:14px; line-height:17px; text-align:center` |
| `keymap-tip` | `display:none; height:auto; left:calc(50% - 105px); max-width:-webkit-max-content; max-width:max-content; padding:10px; position:absolute; top:35px`<br>`display:block` | `background-color:#000; border:1px solid #5d5d5d` |
| `keymap1` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap2` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap3` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap4` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap5` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap6` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap7` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap8` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymapbar-enabled` | `justify-content:left; margin:0 0 0 20px`<br>`display:flex`<br>`flex:1 1 auto`<br>`display:none` | `color:#ccc` |

**⑥ 按键指派（选中键 → 选动作）**（21）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `action` | `align-items:center; display:flex; height:auto; padding:5px 6px`<br>`align-items:start; display:flex; flex-direction:column`<br>`height:20px`<br>`display:flex; gap:20px; justify-content:center` | `text-transform:uppercase`<br>`background-color:#000; color:#44d62c`<br>`background-color:#1a1a1a; color:#44d62c`<br>`background-color:#0000; line-height:17px; text-transform:capitalize; transition:background-color .3s`<br>`background-color:#ffffff1a` |
| `action-wrapper` | `height:40px; position:relative`<br>`height:20px; left:10px; position:absolute; top:10px; width:20px` | `background-image:url(../../static/media/icon_config_audio_function.36f4e93d.svg)`<br>`background-image:url(../../static/media/icon_config_multidial.50aa8a22.svg)`<br>`background-image:url(../../static/media/icon_config_mouse_sensitivity.cd4e347f.svg)`<br>`background-image:url(../../static/media/icon_config_switch_device_profile.88b9f2aa.svg)`<br>`background-image:url(../../static/media/icon_config_brightness_global.755a1ebf.svg)` |
| `action_bar_wrapper` | `display:flex`<br>`bottom:-38px`<br>`height:20px; position:relative; width:20px`<br>`height:20px; width:20px` | `background-image:url(../../static/media/icon_emoji.07a71bf8.svg)`<br>`background-image:url(../../static/media/icon_emoji_active.b1a4efad.svg)`<br>`opacity:1`<br>`background-image:url(../../static/media/icon_close.130e45fb.svg)`<br>`background-image:url(../../static/media/icon_charactermap.025aae0b.svg)` |
| `actions` | `overflow:hidden`<br>`overflow:auto`<br>`overflow-y:scroll`<br>`height:100%; max-width:250px; position:absolute; width:250px` | `background-color:#222; transition:max-width .2s` |
| `assigned-key-wrapper` | `align-items:center; display:flex; max-height:84px; min-height:72px; width:497px` | `color:#999; text-transform:uppercase` |
| `binding-value` | `overflow:hidden` | `line-height:16px` |
| `hoverGray-remappedBlack` | — | `fill:#000c`<br>`stroke:#000` |
| `isAssignment` | — | `fill:#44d62c66`<br>`fill:#000c`<br>`fill:#fd8611b3!important`<br>`fill:#fd8611b3` |
| `key-config` | `width:180px`<br>`position:relative; width:170px`<br>`display:flex; flex-direction:column; left:-121px; max-height:calc(100vh - 150px); min-height:260px; top:28px; width:270px`<br>`top:0; width:270px` | `border-top:initial`<br>`text-transform:capitalize`<br>`opacity:.3`<br>`transition:visibility 0s,opacity 0s,left 0s`<br>`cursor:default` |
| `key-config-flex` | `display:flex` | — |
| `key-map-action` | `margin:0 0 5px` | — |
| `key-map-action-group` | `align-items:center; display:flex; gap:10px`<br>`display:flex; flex:1 1; justify-content:space-between; margin:0 25px; position:relative`<br>`align-items:center; display:flex; height:18px; justify-content:center; position:relative; width:-webkit-fit-content; width:fit-content; width:18px`<br>`height:12px; width:12px` | `cursor:pointer`<br>`background-color:#44d62c; border-radius:50%; cursor:pointer`<br>`background-color:#707070`<br>`background-color:#44d62c; border-radius:10px; cursor:pointer` |
| `key-mapping` | `display:none`<br>`overflow:hidden; padding:0`<br>`overflow:hidden`<br>`overflow:auto` | `line-height:40px` |
| `key_mapping_text_display` | `overflow:hidden` | — |
| `keymap-action` | `padding:0 20px; width:250px`<br>`padding:6px 10px 7px 6px`<br>`justify-content:flex-end`<br>`padding:0` | `border:1px solid #000; border-radius:3px; font-size:12px; line-height:14px; text-align:center` |
| `map-action` | `align-items:center; display:flex; gap:10px`<br>`display:flex; flex:1 1; justify-content:space-between; margin:0 25px; position:relative`<br>`align-items:center; display:flex; height:18px; justify-content:center; position:relative; width:-webkit-fit-content; width:fit-content; width:18px`<br>`height:12px; width:12px` | `cursor:pointer`<br>`background-color:#44d62c; border-radius:50%; cursor:pointer`<br>`background-color:#707070`<br>`background-color:#44d62c; border-radius:10px; cursor:pointer` |
| `mapping-button` | `align-items:center; display:flex; flex-direction:row; min-height:30px; min-width:104px`<br>`align-items:center; display:flex; justify-content:center; margin:0 4px; min-height:inherit; min-width:74px; padding:7px 10px` | `background-color:#111; background:#111 0 0 no-repeat padding-box; border:1px solid #0000; border-radius:3px; font-size:14px`<br>`border:1px solid #44d62c`<br>`color:#44d62c` |
| `mapping-button-container` | `display:grid; row-gap:10px`<br>`align-items:center; display:flex; flex-direction:row; min-height:30px; min-width:104px`<br>`align-items:center; display:flex; justify-content:center; margin:0 4px; min-height:inherit; min-width:74px; padding:7px 10px` | `background-color:#111; background:#111 0 0 no-repeat padding-box; border:1px solid #0000; border-radius:3px; font-size:14px`<br>`border:1px solid #44d62c`<br>`color:#44d62c` |
| `notRemapped` | — | `fill:#fd8611b3`<br>`stroke:#44d62c; fill:#0000`<br>`stroke:#fd8611`<br>`stroke:#69696c`<br>`fill:#c8323c80` |
| `remap-2-disabled` | — | `color:#c8323c` |
| `remapped` | `width:164px` | `color:#44d62c`<br>`color:#fd8611`<br>`border-left:1px solid #707070; font-size:14px; line-height:16px; text-align:left` |

**⑦ 预设动作快捷按钮**（11）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `collapse-action` | — | — |
| `custom-action-btn` | `margin:0 auto`<br>`width:486px` | — |
| `flex-button` | `align-items:flex-start; display:flex; flex-direction:column; padding:2px 0` | — |
| `group-button` | `align-items:center; display:flex; height:32px; margin:0 0 10px 30px; padding:5px 3px; width:-webkit-fit-content; width:fit-content`<br>`height:22px; margin:0 2px; min-width:40px; padding:4px 8px` | `border:1px solid gray; border-radius:20px`<br>`border:0; border-radius:20px; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; line-height:14px; text-transform:uppercase`<br>`background-color:#44d62c; color:#212121` |
| `mediaButton-img` | `height:21px; left:21px; position:absolute; top:21px; width:21px` | — |
| `setting-action` | `align-items:center; display:flex`<br>`margin:0 5px` | — |
| `shortcutButton` | `align-items:center; display:flex; height:100%; justify-content:center; margin:0 10px; width:47px` | `border:1px solid #5d5d5d; border-radius:3px` |
| `shortcutButton-filled` | — | `background-color:#292929` |
| `shortcutButton-snaptap` | `padding:5px 10px; width:auto` | — |
| `shortcut_act_action` | `height:27px; padding:5px 6px` | `background-color:#0000; line-height:17px; text-transform:capitalize; transition:background-color .3s`<br>`background-color:#222` |
| `shortcut_item_action` | `align-items:center; display:flex; position:relative` | — |

**⑧ 键盘独有：组合键（两键同按）**（12）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `combined-key-blink-active` | `padding:0 10px!important` | — |
| `combined-key-checkbox-col` | `align-items:center; display:flex; justify-content:flex-start; width:60px` | — |
| `combined-key-container` | `display:flex; flex-direction:column; gap:15px` | — |
| `combined-key-delete-icon` | `height:20px; width:20px` | `background-color:#999; background-repeat:no-repeat; background-size:cover`<br>`background-color:#c8323c` |
| `combined-key-desc` | — | — |
| `combined-key-item` | `margin:0!important` | — |
| `combined-key-item-editing` | `height:20px!important; margin:0 1px!important; min-width:0!important; padding:0 5px` | `border-radius:3px; font-size:12px!important` |
| `combined-key-list-wrapper` | `align-items:center; display:flex; flex-direction:column` | — |
| `combined-key-pair` | `align-items:center; display:flex; gap:10px; width:100%` | — |
| `combined-key-row` | `align-items:center; display:flex; gap:16px` | — |
| `common-key-mapping` | `bottom:auto!important` | — |
| `twoKeyMapping` | `height:20px; position:absolute; top:15px; width:20px` | `background-image:url(../../static/media/icon_secondaryfunction-1.734f766c.svg); background-size:20px 20px`<br>`color:#707070; font-size:14px; line-height:14px; text-transform:uppercase` |

**⑨ 键盘独有：Snap Tap / 快速敲击**（13）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `single-key-snap-tap-desc` | `display:block; width:530px` | — |
| `single-key-snap-tap-key-list` | `display:flex; flex-direction:column; min-height:72px; width:160px` | — |
| `single-key-snap-tap-to-left` | — | — |
| `skst-key-input` | `height:30px; margin:0 2px; max-width:30px` | `border:1px solid #44d62c; border-radius:4px; color:#999`<br>`background:#1f1f1f; font-size:12px; text-align:center; text-transform:lowercase` |
| `skst-key-input-warning` | `height:30px; margin:0 2px; max-width:30px` | `background:#1f1f1f; font-size:12px; text-align:center; text-transform:lowercase`<br>`border:1px solid #fd8611; border-radius:4px; color:#fd8611` |
| `skst-key-record-item-v3` | `align-items:center; display:flex; height:30px; justify-content:right; min-width:30px` | `border:1px solid #666; border-radius:4px; color:#999`<br>`border:1px solid #9b9b9b` |
| `skst-key-record-item-v3-warning` | `align-items:center; display:flex; height:30px; justify-content:right; min-width:30px` | `border:1px solid #fd8611; border-radius:4px; color:#fd8611` |
| `snap-tap-add-button` | `align-items:center; display:flex; height:44px; justify-content:center; position:relative; width:64px`<br>`padding:8px 10px; position:fixed`<br>`margin:0`<br>`position:fixed` | `border:2px solid #ccc; border-radius:5px; color:#ccc; font-size:20px`<br>`opacity:30%`<br>`border-color:#44d62c; color:#44d62c`<br>`background-color:#111; border:1px solid #5d5d5d; color:#ccc`<br>`font-family:Roboto,sans-serif; font-size:14px; line-height:17px` |
| `snap-tap-add-button-overlay` | `position:absolute` | — |
| `snap-tap-add-button-v3` | `margin:auto; padding:6px; width:-webkit-fit-content; width:fit-content` | `color:#ccc; text-align:center`<br>`color:#44d62c` |
| `snap-tap-key-list` | `width:-webkit-fit-content; width:fit-content` | — |
| `snap-tap-key-list-v3` | `padding:10px`<br>`align-items:center; display:flex; justify-content:center` | `background:#1f1f1f; border-radius:5px`<br>`font-size:10px; line-height:12px; text-align:center` |
| `snap-tap-key-list-wrapper` | `align-items:center; display:flex; gap:10px` | — |

**⑩ 键盘独有：动态击键 / 按键录制**（18）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `DYNAMIC_KEY_STROKE` | — | `background-image:url(../../static/media/icon_dks-disabled.2d349f34.svg)`<br>`background-image:url(../../static/media/icon-dks.711bc1fd.svg)` |
| `dynamic-key-stroke` | `margin:0; padding:0` | `color:#999; font-size:14px; font-weight:400; line-height:17px` |
| `dynamic-key-stroke-body` | `flex:1 1`<br>`align-items:center; display:flex; justify-content:space-between`<br>`align-items:center; display:flex; flex-direction:column; gap:2px; position:relative`<br>`height:12px; width:12px` | `color:#ccc; cursor:pointer; font-size:14px`<br>`color:#44d62c`<br>`font-size:12px`<br>`background-color:#000; border:1px solid #666`<br>`text-transform:uppercase` |
| `key-dks` | `position:absolute`<br>`height:10px; width:19px` | `background-size:cover`<br>`background-image:url(../../static/media/dynamic_key_stroke_icon.c4bd98bb.svg)` |
| `key-record` | `gap:10px; width:-webkit-fit-content; width:fit-content`<br>`align-items:center; display:flex` | — |
| `key-record-item` | `align-items:center; display:flex`<br>`height:40px; justify-content:center; min-width:60px` | `border:2px solid #ccc; border-radius:4px` |
| `key-record-item-assignment` | `align-items:center; display:flex; justify-content:center`<br>`margin:auto 10px; padding:auto 10px` | `color:#fff`<br>`font-size:11px` |
| `key-record-item-assignment-active` | — | `background-color:#44d62c; border-color:#44d62c; color:#000` |
| `key-record-item-assignment-deactive` | — | `background-color:#888; border-color:#888; color:#000` |
| `key-record-item-editing` | `align-items:center; display:flex; height:25px; justify-content:center; margin:auto 10px; min-width:38px; padding:auto 10px` | `font-size:11px` |
| `key-record-item-editing-active` | — | `border-color:#44d62c` |
| `key-record-item-editing-warning` | — | `border-color:#fd8611` |
| `key-record-item-v3` | `align-items:center; display:flex; height:30px; justify-content:center; min-width:30px` | `border:1px solid #666; border-radius:4px` |
| `key-record-item-v3-skst-text` | `min-width:0!important; padding:0 10px!important` | `border-radius:3px; text-transform:none` |
| `key-record-item-v3-text` | `margin:0 2px!important; min-width:0!important; padding:0 10px!important` | `border-radius:3px` |
| `key-record-item-warning` | — | `border-color:#fd8611` |
| `macro-keypad-module` | `min-height:auto`<br>`position:static`<br>`position:absolute`<br>`flex-direction:column` | `font-size:14px`<br>`text-transform:capitalize`<br>`background-color:#000; border:1px solid #5d5d5d; border-radius:20px`<br>`background-color:#3cbf27; border-radius:20px; color:#212121`<br>`opacity:1` |
| `marco-keymap` | — | — |

**⑪ 通用控件**（27）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `button` | `align-items:center; display:flex; flex:none; flex-direction:row; flex-grow:0; gap:10px; height:27px; justify-content:center; order:2; padding:7px 16px 6px; width:173px`<br>`align-items:center; display:flex; flex:none; flex-direction:row; flex-grow:0; gap:10px; height:27px; justify-content:center; order:5; padding:7px 16px 6px; width:173px`<br>`margin:9px auto 0; padding:7px 16px; width:-webkit-fit-content; width:fit-content` | `background:#707070; border:1px solid #0000004d; border-radius:3px; font-family:Roboto; font-size:12px; font-weight:400; line-height:14px; text-align:center; text-transform:uppercase`<br>`background-color:#ffffff1a`<br>`background:#44d62c; border:1px solid #0000004d; border-radius:3px; color:#222; font-family:Roboto; font-size:12px; font-weight:400; line-height:14px; text-align:center; text-transform:uppercase`<br>`opacity:.8`<br>`background-color:#707070; border-radius:3px; cursor:pointer; text-transform:uppercase` |
| `button--cta` | `margin:9px auto 0; padding:7px 16px; width:-webkit-fit-content; width:fit-content`<br>`height:18px; position:relative; width:18px`<br>`left:0; max-width:300px; width:-webkit-max-content; width:max-content` | `background-color:#707070; border-radius:3px; cursor:pointer; text-transform:uppercase`<br>`background-position:50%; background-repeat:no-repeat; background-size:18px 18px`<br>`opacity:1` |
| `button--cta-close` | `height:24px; position:absolute; right:0; top:0; width:24px` | `background-image:url(../../static/media/icon_close.55fe41f1.svg); background-position:50%; background-size:24px 24px` |
| `button--cta-get-control` | `align-items:center; display:flex; height:27px; justify-content:center; margin:9px auto 0; padding:0 16px; width:-webkit-fit-content; width:fit-content` | `background-color:#707070; border-radius:3px; color:#ccc; color:#fff; cursor:pointer; font-size:14px; text-transform:uppercase` |
| `button--cta-link` | `left:0; max-width:300px; width:-webkit-max-content; width:max-content` | `background-image:url(../../static/media/unlink-btn.db633bad.svg)`<br>`opacity:1` |
| `button--cta-link-link` | — | `background-image:url(../../static/media/unlink-btn.db633bad.svg)`<br>`background-image:url(../../static/media/unlink-hovered-btn.67ac688d.svg)` |
| `button--cta-link-unlink` | — | `background-image:url(../../static/media/link-btn.fed18bb7.svg)`<br>`background-image:url(../../static/media/link-hovered-btn.0fde7f57.svg)` |
| `button--cta-remove` | `display:block; height:27px; margin:0 auto; min-width:90px; width:-webkit-fit-content; width:fit-content` | `background-color:#fd4949; border:1px solid #0000004d; border-radius:3px; color:#111; cursor:pointer; font-size:12px; line-height:14px; text-align:center; text-transform:uppercase` |
| `button--cta-show-active` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 15px 0 0; padding:0 16px; width:-webkit-fit-content; width:fit-content` | `background-color:#292929; border:1px solid #44d62c; border-radius:3px; color:#ccc; cursor:pointer; font-size:14px; text-transform:uppercase` |
| `button--cta-show-busy` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 auto; padding:0 16px; width:-webkit-fit-content; width:fit-content` | `border:1px solid #fd8611; border-radius:3px; color:#ccc; color:#fd8611; cursor:pointer; font-size:14px; text-transform:uppercase` |
| `button--cta-show-locked` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 10px 0 auto; padding:0 16px 0 40px; position:relative; width:-webkit-fit-content; width:fit-content`<br>`display:block; height:24px; left:0; position:absolute; top:0; width:24px` | `border:1px solid #e5e5e5; border-radius:3px; color:#ccc; color:#e5e5e5; cursor:pointer; font-size:14px; text-transform:uppercase`<br>`background-image:url(../../static/media/icon_lock_white.c066cd4d.svg); background-position:100%; background-repeat:no-repeat; background-size:70% 70%` |
| `button--cta-unlink` | — | `background-image:url(../../static/media/link-btn.fed18bb7.svg)`<br>`background-image:url(../../static/media/link-hovered-btn.0fde7f57.svg)` |
| `button-description` | `max-width:250px; overflow:hidden`<br>`align-items:flex-start; display:flex; flex-direction:column; padding:2px 0` | `color:#44d62c; font-size:10px; line-height:12px; text-transform:uppercase`<br>`color:#fd8611` |
| `button-group` | `display:flex; flex-wrap:wrap; gap:10px`<br>`height:70px; min-width:63px; padding:40px 0 0; width:63px` | `background-color:#111; background-repeat:no-repeat; border:1px solid gray; color:#ccc; font-family:Roboto,sans-serif; font-size:10px; line-height:0; text-align:center; text-transform:none`<br>`border:1px solid #44d62c`<br>`background-image:url(../../static/media/standard.cbf1362d.svg)`<br>`background-image:url(../../static/media/fast.43db44c4.svg)`<br>`background-image:url(../../static/media/slow.8b9c0df7.svg)` |
| `button_icon` | `align-items:center; display:flex` | `color:#707070; font-size:14px; line-height:17px; text-align:left`<br>`fill:#fff`<br>`fill:#44d62c` |
| `buttons-block` | `align-items:flex-start; display:flex; flex-direction:column; gap:16px; height:97px; padding:0 50px 50px; width:480px`<br>`align-items:flex-start; display:flex; flex-direction:row; gap:12px; height:4px; padding:0`<br>`height:4px; position:relative; width:4px`<br>`align-items:flex-start; display:flex; flex-direction:row; gap:10px; height:27px; padding:0; width:190px` | `opacity:1`<br>`background-color:#707070; border-radius:10px; transition:none`<br>`background-color:#fff`<br>`background:#707070; border:1px solid #0000004d; border-radius:3px; color:#fff; cursor:pointer; font-family:Roboto; font-size:12px; font-weight:400; line-height:14px; opacity:1; text-align:center; text-transform:uppercase; transition:background-color .3s,opacity .3s`<br>`background-color:#8a8a8a` |
| `buttons-row` | `align-items:flex-start; display:flex; flex-direction:row; gap:10px; height:27px; padding:0; width:190px` | — |
| `enable-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:90px` | `background-color:#44d62c; border-radius:3px; color:#212121; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `icon-button` | — | `fill:#44d62c`<br>`fill:#44d62c; opacity:.7` |
| `icon-button-duallink` | `height:28px; margin:auto; min-width:90px; width:-webkit-max-content; width:max-content`<br>`display:block; height:28px; padding:0 5px` | `background-color:#44d62c; border-radius:2px; color:#212121; font-family:Roboto; font-size:12px; line-height:28px; text-align:center`<br>`text-transform:uppercase` |
| `icon-button-text-duallink` | `display:block; height:28px; padding:0 5px` | `background-color:#44d62c; border-radius:2px; color:#212121; font-family:Roboto; font-size:12px; line-height:28px; text-align:center`<br>`text-transform:uppercase` |
| `login-button` | `height:27px; min-width:90px; padding:7px 16px 6px; width:-webkit-max-content; width:max-content` | `background-color:#44d62c; border-radius:3px; color:#111; cursor:pointer; text-align:center; text-transform:uppercase; transition:all .3s ease-out`<br>`background-color:#44d62cb3`<br>`background-color:#fff` |
| `ok-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:90px` | `background-color:#707070; border-radius:3px; color:#ccc; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `razer-button` | `min-width:90px; position:relative; width:-webkit-fit-content; width:fit-content`<br>`height:100%; left:0; position:absolute; top:0; width:100%` | `background-color:#44d62c; border:1px solid #000; border-radius:3px; color:#000; cursor:default; line-height:27px; text-transform:uppercase`<br>`background:#ffffff4d; border-radius:3px`<br>`background:#44d62cb3!important`<br>`background:none` |
| `switch-button` | `height:30px; width:90px` | `background-color:#333; border:1px solid #555; color:#ccc; cursor:pointer; font-size:14px; text-transform:uppercase; transition:background-color .3s,color .3s`<br>`background-color:#44d62c; border-color:#44d62c; color:#111`<br>`background-color:#444` |
| `switch_button` | `height:22px; margin:0 2px; min-width:40px; padding:4px 8px`<br>`align-items:center; display:flex; height:27px; justify-content:center; margin:0; min-width:90px; padding:0 16px` | `border:0; border-radius:20px; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; line-height:14px; text-transform:uppercase`<br>`background-color:#44d62c; color:#212121`<br>`background-color:#111; border:1px solid #5d5d5d; border-radius:3px; cursor:pointer; font-size:12px; text-transform:uppercase`<br>`border-color:#44d62c`<br>`background-color:#ffffff1a` |
| `tooltip-panel-button` | `left:61px; max-width:358px; padding:8px 10px; position:absolute` | `background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-size:14px; line-height:16px; opacity:0; text-align:left`<br>`opacity:1` |

**⑫ 其它**（54）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `AudioMeter_flowButtonActive__vq7uT` | — | `background:#292929; border:1px solid #44d62c` |
| `AudioMeter_flowButtonIcon__p7D6V` | `height:18px; width:18px` | — |
| `AudioMeter_flowButton__4sIhG` | `align-items:center; display:flex; flex-direction:column; gap:5px; height:66px; justify-content:center; min-width:160px; padding:7px 16px 6px; width:160px`<br>`height:18px; width:18px` | `background:#111; border:1px solid #5d5d5d; border-radius:3px; color:#ccc; cursor:pointer; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; opacity:1; text-transform:uppercase; transition:all .2s ease` |
| `AudioMeter_flowButtons__tOGNz` | `display:flex; gap:5px` | — |
| `Duallink_unpairbutton__lTG1W` | `display:block; height:28px; left:50%; padding:0 10px; position:absolute; top:50%`<br>`display:block; height:28px; min-width:90px; padding:0 5px` | `background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; text-align:center`<br>`background-color:#9b9b9b`<br>`background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; opacity:.8; text-align:center`<br>`opacity:1` |
| `Duallink_unpairbuttondisabled__ZdDMy` | `display:block; height:28px; left:50%; padding:0 10px; position:absolute; top:50%` | `background-color:#707070; border-radius:2px; color:#fff; font-family:Roboto; font-size:12px; line-height:28px; opacity:.5; text-align:center` |
| `HelpComponent_supportButton__S0UnH` | — | `cursor:pointer`<br>`color:#44d62c` |
| `MonitoringDashboard_menuButton__IkMX-` | `align-items:center; display:flex; justify-content:center; padding:0; position:relative` | `background:none; border:1px solid #0000; border-radius:0; color:#ccc; cursor:pointer`<br>`outline:1px dotted #0f0`<br>`border-color:#5d5d5d`<br>`color:#999`<br>`color:#fff` |
| `MonitoringToggle_button__xH4PT` | `align-items:center; display:flex; justify-content:center; padding:4px` | `background:none; border:1px solid #0000; border-radius:0; color:#999; cursor:pointer; transition:all .2s ease`<br>`border-color:#5d5d5d; color:#fff`<br>`outline:1px dotted lime` |
| `SWITCH_DEVICE_PROFILE` | — | `background-image:url(../../static/media/icon_config_switch_device_profile.88b9f2aa.svg)`<br>`background-image:url(../../static/media/icon_config_switch_device_profile_a.942d360a.svg)` |
| `SWITCH_KEYMAP` | — | `background-image:url(../../static/media/icon_config_keymap.cb5ba1f0.svg)`<br>`background-image:url(../../static/media/icon_config_keymap_a.e27c970d.svg)` |
| `SWITCH_PROFILE` | — | `background-image:url(../../static/media/icon_config_switch_device_profile.88b9f2aa.svg)`<br>`background-image:url(../../static/media/icon_config_switch_device_profile_a.942d360a.svg)` |
| `WaveWithSpeedV2_waveButtons__vAgEX` | `display:grid; gap:20px` | — |
| `action-trigger` | `display:flex; flex:1 1; justify-content:space-between; margin:0 25px; position:relative` | — |
| `action-trigger-icon` | `align-items:center; display:flex; height:18px; justify-content:center; position:relative; width:-webkit-fit-content; width:fit-content; width:18px`<br>`height:86px; left:50%; position:absolute; top:87%; width:1px` | `background-color:#707070` |
| `action-trigger-icon-dot` | `height:18px; left:50%; position:absolute; top:50%; width:18px` | `background-color:#44d62c; border-radius:50%; cursor:pointer` |
| `action-trigger-icon-plus` | `height:12px; width:12px` | `cursor:pointer` |
| `action-trigger-line` | `height:18px; left:0; position:absolute; top:0` | `background-color:#44d62c; border-radius:10px; cursor:pointer` |
| `add-keymap` | `display:flex; max-width:100%; min-height:25px; overflow:hidden; padding:4px 5px; width:100%`<br>`padding:0 4px` | `border-top:1px solid #515151; font-size:14px; text-transform:none`<br>`background-color:#ffffff1a` |
| `box-bindingdevice-tag` | `display:block; height:auto; position:block; width:100%` | `color:#44d62c; font-size:14px; line-height:16px; text-align:center; transition:color .2s` |
| `box-scanselect-button` | `display:inline-block; height:28px; width:90px` | `border:1px solid #000; border-radius:2px; border-radius:3px; font-family:Roboto; font-size:12px; line-height:28px; text-align:center; text-transform:uppercase; transition:opacity .3s` |
| `box-unbindingdevice-tag` | `display:block; height:auto; position:block; width:100%` | `color:#44d62c; font-size:14px; line-height:16px; text-align:center; transition:color .2s` |
| `box-unpairselect-button` | `display:inline-block; height:28px; width:90px`<br>`min-width:-webkit-max-content; min-width:max-content; padding:0 10px; position:relative` | `border:1px solid #000; border-radius:2px; border-radius:3px; font-family:Roboto; font-size:12px; line-height:28px; text-align:center; text-transform:uppercase; transition:opacity .3s` |
| `custom-keymapping-stepper` | `width:90%` | `border:1px solid #5d5d5d`<br>`border:1px solid #44d62c`<br>`text-align:left; text-align:initial`<br>`border:1px solid #5d5d5d!important`<br>`border:1px solid #44d62c!important` |
| `customize-fn-esc-button` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 5px; padding:13px 15px; width:auto` | `border:1px solid #5d5d5d; border-radius:3px; color:#ccc; font-size:14px; text-transform:uppercase` |
| `customize-polling-rate-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:72px`<br>`min-width:90px` | `background-color:#222; border:1px solid #5d5d5d; border-radius:3px; color:#ccc; font-size:14px; text-transform:uppercase`<br>`border-color:#44d62c` |
| `customize-polling-rate-button-color` | `height:6px; width:6px` | `border-radius:50%` |
| `customize-setting-button` | `width:150px`<br>`align-items:center; display:flex; height:27px; justify-content:center; min-width:90px` | `background-color:#707070; border-radius:3px; color:#fff; font-size:12px; text-transform:uppercase`<br>`opacity:.3` |
| `func-multi-button` | `margin:10px 0 0` | — |
| `installation-action` | `margin:0 0 0 30px`<br>`height:27px; width:auto` | `opacity:.6` |
| `key-require-macro-module` | `height:20px; position:absolute; width:20px` | `background-size:cover`<br>`background-image:url(../../static/media/require_macro_module_icon.20694b5c.svg)` |
| `key-require-synapse` | `height:20px; position:absolute; width:20px` | `background-image:url(../../static/media/require_synapse_icon.d5e656b0.svg)`<br>`background-size:cover` |
| `key-snap-tap` | `height:16px; width:16px`<br>`position:absolute` | `background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg)`<br>`background-size:cover` |
| `key-tip` | `padding:8px 10px`<br>`overflow:hidden!important`<br>`padding:initial`<br>`max-width:300px; min-height:75px; padding:initial; position:absolute` | `text-transform:capitalize`<br>`background-color:#000; border:1px solid #5d5d5d; line-height:14px; opacity:0; text-align:center; transition:opacity .3s,visibility 0s,left 0s,top 0s`<br>`font-family:Roboto; font-size:12px`<br>`background-color:#c8323c; color:#000`<br>`background-color:#44d62c; color:#000` |
| `message-container__button` | — | `background-color:#44d62c; color:#111` |
| `modal_importProfileInfoItem__GyCiB` | `align-items:center; display:flex; height:31px; position:relative`<br>`align-items:center; display:flex; height:100%; position:relative; width:20px`<br>`padding:8px 10px 10px; position:absolute; top:100%`<br>`align-self:stretch; width:3px` | `color:#ccc; font-family:Roboto; font-size:14px`<br>`background-color:#000; border:1px solid #5d5d5d; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `no-chroma-studio-profile` | — | `color:#44d62c` |
| `optionButtonGroup_active__-z-LG` | — | `background:#292929; border-color:#44d62c`<br>`background:#292929; border:1px solid #44d62c` |
| `optionButtonGroup_disabled__Q0Wrc` | — | `background:#ffffff1a`<br>`background:#292929; border:1px solid #44d62c`<br>`color:#ffffff80; cursor:not-allowed`<br>`opacity:.5` |
| `optionButtonGroup_effect-with-duration__PkoU2` | `display:flex; flex-direction:column` | `opacity:.5` |
| `optionButtonGroup_effects-area__8iBAF` | `display:flex; flex-direction:column`<br>`align-items:flex-start` | — |
| `optionButtonGroup_option-button-group__TUmjj` | `display:flex; gap:5px` | — |
| `optionButtonGroup_option-button__C62Cv` | `height:27px; margin:0; min-width:90px; padding:7px 16px 6px` | `background:#0000; border:1px solid #5d5d5d; border-radius:3px; color:#ffffffe6; cursor:pointer; font-family:Roboto; font-size:12px; font-weight:400; letter-spacing:0; line-height:14px; outline:none; text-align:center; text-transform:uppercase; transition:all .2s ease`<br>`background:#ffffff1a`<br>`background:#292929; border:1px solid #44d62c`<br>`color:#ffffff80; cursor:not-allowed` |
| `optionButtonGroup_option-label__J0Xc0` | — | `font-family:Roboto; font-size:14px; font-weight:400; line-height:14px; text-transform:uppercase` |
| `optionButtonGroup_random-color__wSIuX` | — | — |
| `optionButtonGroup_with-option-buttons__gEnlO` | `display:flex; flex-direction:column`<br>`align-items:flex-start` | — |
| `remove-text-keymapping` | `height:16px; position:absolute; right:5px; top:5px; width:16px` | `background-image:url(../../static/media/icon_close_enclosed.6056b667.svg); background-position:50%; background-repeat:no-repeat; background-size:20px; transition:background-image .2s`<br>`background-image:url(../../static/media/icon_close_enclosed_a.83aff4bb.svg); cursor:pointer` |
| `s3-modal--action-take-control` | — | `border:1px solid #707070`<br>`text-align:left` |
| `secondary-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:90px` | `background-color:#707070; border-radius:3px; color:#ccc; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `show-all-button` | — | `cursor:pointer; font-size:14px; text-align:center; text-transform:capitalize`<br>`color:#44d62c` |
| `show-all-button-arrow` | `height:20px; width:20px` | `transition:fill .2s ease` |
| `sliderChart__reset-button` | `height:300px; min-width:20px; right:40px` | `background:url(../../static/media/eq_reset.e0c3c09c.svg) no-repeat 50%; background-size:20px 20px; opacity:.85; transition:background-image .3s`<br>`background-image:url(../../static/media/eq_reset_hover.186df33c.svg)`<br>`background-image:url(../../static/media/eq_reset_active.37c570d3.svg)` |
| `text-standard-appli` | — | `color:#707070; font-size:12px; line-height:5px` |
| `tip-special-button-text` | `max-width:155px; overflow:hidden` | `color:#ccc` |

#### RAZER KRAKEN BT SANRIO LIMITED EDITION（productId 777）

该界面共 150 个布局类名，分 11 个分区。

**① 顶部：配置文件栏**（21）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `add-profile` | `left:6px; top:8px`<br>`height:36px; width:36px` | — |
| `app-profile-icon` | — | `text-transform:capitalize` |
| `custom-profile-bar` | `margin:0 0 0 20px; width:-webkit-fit-content; width:fit-content` | — |
| `dropdown-profile` | `align-items:center; display:flex; gap:10px; position:relative` | — |
| `profile-act` | `max-width:280px; min-width:155px; width:-webkit-max-content; width:max-content`<br>`align-items:center; display:flex`<br>`align-items:start; display:flex; flex-direction:column`<br>`height:20px` | `background:#000; border:1px solid #5d5d5d; transition:height 0s,visibility 0s,opacity .1s linear`<br>`opacity:1`<br>`background-color:#0000; line-height:17px; text-transform:capitalize; transition:background-color .3s`<br>`background:#5d5d5d`<br>`background-color:#ffffff1a` |
| `profile-bar` | `justify-content:normal; margin:0; width:auto`<br>`display:inline-block; height:12px; margin:2px 5px 0; width:12px`<br>`align-items:center; flex-direction:row; height:26px; justify-content:center; margin:0 auto 10px; width:95%`<br>`align-items:center; flex:0 0 auto; position:relative` | `color:#ccc`<br>`text-transform:capitalize`<br>`background-color:#4a4a4a; background-image:url(../../static/media/tooltip_exclamationmark.cc8fb226.svg); background-repeat:no-repeat; border-radius:6px; transition:background-color .3s`<br>`background-color:#ffffff4d`<br>`font-size:14px; text-transform:uppercase` |
| `profile-bar-macro` | — | `color:#44d62c; font-size:14px; text-align:center` |
| `profile-bar-title` | — | — |
| `profile-color` | `height:8px; position:absolute; right:0; top:0; width:8px` | `border-radius:50%; color:var(--profile-color)`<br>`background:#44d62c`<br>`background:red`<br>`background:lime`<br>`background:blue` |
| `profile-del` | `left:auto`<br>`min-width:300px; top:42px`<br>`padding:4px 5px`<br>`left:274px; top:53px` | `border:1px solid #fd4949`<br>`font-family:Roboto,sans-serif`<br>`color:#fd4949; text-align:center; text-transform:uppercase`<br>`font-weight:400`<br>`background-color:#fd4949; color:#111; line-height:14px` |
| `profile-dynamic-tips` | `align-items:center; display:flex!important; flex-direction:column; left:10px; padding:20px; position:absolute`<br>`height:auto; left:50%; padding:20px; top:40px; width:300px` | `background-color:#111; background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px 0 #0003; box-shadow:0 6px 10px #0003; opacity:0; text-transform:none; transition:visibility 0s,opacity .3s linear`<br>`color:#ccc; font-size:14px`<br>`opacity:1` |
| `profile-migration-icon` | `height:40px; width:40px` | `background-image:url(../../static/media/synapse_profile_migration.c3622288.svg); background-position:50%; background-repeat:no-repeat` |
| `profile-selected-tips` | `align-items:center; display:flex!important; flex-direction:column; left:10px; padding:20px; position:absolute`<br>`height:-webkit-fit-content; height:fit-content; top:40px; width:300px` | `background-color:#111; background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px 0 #0003; box-shadow:0 6px 10px #0003; opacity:0; text-transform:none; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `profile-switching` | `align-items:start; display:flex; flex-direction:column; height:94px; justify-content:space-evenly` | `text-transform:uppercase` |
| `profile-switching-des` | `height:-webkit-fit-content; height:fit-content` | `color:#999` |
| `profile-tips` | `align-items:center; display:flex!important; flex-direction:column; left:10px; padding:20px; position:absolute`<br>`height:-webkit-fit-content; height:fit-content; top:40px; width:300px`<br>`height:auto; left:50%; padding:20px; top:40px; width:300px` | `background-color:#111; background:#111 0 0 no-repeat padding-box; border:1px solid #fd8611; border-radius:3px; box-shadow:0 6px 10px 0 #0003; box-shadow:0 6px 10px #0003; opacity:0; text-transform:none; transition:visibility 0s,opacity .3s linear`<br>`opacity:1`<br>`color:#ccc; font-size:14px` |
| `profile-wrapper` | `display:flex; flex:1 0 25%`<br>`left:auto`<br>`display:flex`<br>`flex:1 1 25%` | — |
| `profileIcon` | `position:relative`<br>`display:block; height:20px; position:absolute; width:20px`<br>`left:0; position:absolute; top:28px; width:300px` | `background-position:50%; background-repeat:no-repeat; background-size:20px`<br>`background-image:url(../../static/media/icon_obm_1.b37eed4a.svg)`<br>`background-image:url(../../static/media/icon_obm_2.512f2e40.svg)`<br>`background-image:url(../../static/media/icon_obm_3.4350a917.svg)`<br>`background-image:url(../../static/media/icon_obm_4.379b007d.svg)` |
| `specific-profile` | `width:180px`<br>`position:relative; width:170px` | — |
| `specific-profile-interdevice` | `width:180px`<br>`position:relative; width:170px` | — |
| `synapse-profile-migration` | `align-items:center; display:flex; justify-content:center; position:relative`<br>`height:40px; width:40px`<br>`height:auto; left:-130px; min-height:91px; min-width:300px; padding:20px; position:absolute; top:48px; width:100%` | `background-image:url(../../static/media/synapse_profile_migration.c3622288.svg); background-position:50%; background-repeat:no-repeat`<br>`background-color:#111; border:1px solid #fd8611; border-radius:3px; text-align:center`<br>`font-size:14px` |

**② 弹窗：配置文件导入 / 导出 / 重置**（6）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `Dialog_emptyProfile__MF3h-` | `flex-grow:1; padding:20px 50px 0` | `color:#c8323c; font-family:Roboto; font-size:14px; line-height:17px` |
| `Dialog_importProfileItem__CAXFi` | `position:relative`<br>`display:flex`<br>`left:130px; padding:8px 10px 10px; position:absolute; top:calc(100% + 10px); width:88px` | `background-color:#000; border:1px solid #5d5d5d; font-size:14px; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `ImportExportModal_emptyProfile__N-nlD` | `flex-grow:1; padding:20px 50px 0` | `color:#fd4949; font-family:Roboto; font-size:14px; line-height:17px; text-align:center; text-transform:none` |
| `ImportExportModal_importProfileItem__-cpSh` | `position:relative`<br>`display:flex`<br>`left:130px; padding:8px 10px 10px; position:absolute; top:calc(100% + 10px); width:88px` | `background-color:#000; border:1px solid #5d5d5d; font-size:14px; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `empty-profile` | `height:27px; margin:0 10px; max-height:27px; padding:4px 5px; position:relative; width:100%` | `background-color:#0000; border:1px solid #515151; color:#ccc; font-size:14px; line-height:17px; text-transform:none; transition:opacity .3s,border .3s` |
| `import-profile-btn-group` | `display:inline-flex; flex-shrink:0; height:27px`<br>`height:100%; width:-webkit-fit-content; width:fit-content`<br>`align-items:center; display:flex; position:relative`<br>`left:-113px; padding:8px 10px 10px; position:absolute; top:-200%; width:300px` | `border:1px solid #0000004d; font-family:Roboto; font-size:12px; line-height:0px; text-align:center`<br>`background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-family:Roboto; font-size:14px; line-height:16px; opacity:0; text-transform:none; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |

**③ 弹窗：删除确认**（3）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `keymap-delete-alert` | `max-width:330px` | — |
| `s3-modal--action-remove` | — | `color:#fd4949; font-size:16px; line-height:16px; text-transform:uppercase`<br>`font-size:14px; text-align:center` |
| `s3-modal--button` | `height:24px; position:absolute; right:0; top:0; width:24px` | `background-image:url(../../static/media/icon_close.55fe41f1.svg); background-position:50%; background-size:24px 24px` |

**④ 按键层切换：标准 / Hypershift**（10）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `RAZER_HYPERSHIFT` | — | `background-image:url(../../static/media/icon_config_hypershift.6c83f48a.svg)`<br>`background-image:url(../../static/media/icon_config_hypershift_a.33a591f6.svg)` |
| `hypershift` | — | `color:#ccc`<br>`background-color:#fd8611; border-radius:12px; color:#212121` |
| `hypershift-mode` | — | `background:#fd861133` |
| `hypershift-mode-tip` | `max-width:300px; top:19px`<br>`【@media screen and (max-width:1279px)】left:auto; right:1vw; top:26px` | — |
| `main-keymap` | `width:100%`<br>`display:inline-block; height:100%` | `border-left:none; border-top:none` |
| `primary-keymap` | `min-height:562px; position:-webkit-sticky; position:sticky` | — |
| `razer_hypershift` | — | `stroke:#69696c`<br>`fill:#c8323c80` |
| `secondary-keymap` | `display:inline-block; height:100%`<br>`display:none; position:relative`<br>`display:inline; display:initial; width:292px` | `border-left:none; border-top:none`<br>`background-color:#111; opacity:0; transition:visibility 0s,opacity .2s,left 0s`<br>`border:1px solid #5d5d5d; box-shadow:0 0 20px 0 #000000b3; cursor:default; opacity:1` |
| `secondary-keymap-extra` | `position:static; width:auto`<br>`width:auto`<br>`margin:0`<br>`margin:0 20px` | `border-left:1px solid #5d5d5d` |
| `standard` | — | `background-color:#44d62c; border-radius:12px`<br>`background-color:#0000; color:#ccc` |

**⑤ 设备图形与可点按键位**（19）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `disableKeypadButton` | — | `fill:#0000!important`<br>`fill:#44d62c66` |
| `is-keypad` | `【@media screen and (max-width:1279px)】left:0; right:auto` | — |
| `key-light-device` | — | `background-image:url(/synapse/assets/imgs/favicon/IOT_KEY_LIGHT.svg)` |
| `keymap-bar` | `justify-content:left; margin:0 0 0 20px`<br>`margin:0 0 0 20px; width:-webkit-fit-content; width:fit-content`<br>`align-items:center; display:flex; flex-direction:row; height:26px; justify-content:center; margin:20px 0 0 17px; width:100%`<br>`right:auto` | `color:#ccc`<br>`font-size:14px; text-transform:uppercase`<br>`text-transform:none` |
| `keymap-bar-icon` | `height:20px; width:20px`<br>`【@media screen and (max-width:1279px)】left:0; right:auto` | `background-image:url(../../static/media/icon_config_keymap_ccc.9255af63.svg); background-position:50%; background-repeat:no-repeat; background-size:14px; border:2px solid #ccc; border-radius:50%` |
| `keymap-bar-menu-wrap` | `flex:0 0 auto; position:relative` | — |
| `keymap-bar-name` | `display:none` | — |
| `keymap-component` | `height:100%`<br>`max-height:calc(100% - 35px)`<br>`【@media(max-height:570px)】height:320px; overflow-y:auto` | — |
| `keymap-head` | `left:75%; right:auto; width:100%`<br>`width:600px`<br>`height:36px; left:-1px; padding:10px 0 9px; position:absolute; top:-36px; width:292px`<br>`display:inline-block; max-width:200px; overflow:hidden!important` | `background-color:#222; border:1px solid #5d5d5d; border-radius:5px 5px 0 0; color:#999; font-size:14px; line-height:17px; text-align:center`<br>`background-color:#000; border:1px solid #5d5d5d`<br>`border:none; border-bottom:1px solid #5d5d5d`<br>`background-color:#0000; background-image:url(../../static/media/icon_close.55fe41f1.svg); background-position:50%; background-repeat:no-repeat; background-size:20px; transition:background-color .2s`<br>`background-color:#ffffff1a` |
| `keymap-tip` | `display:none; height:auto; left:calc(50% - 105px); max-width:-webkit-max-content; max-width:max-content; padding:10px; position:absolute; top:35px`<br>`display:block` | `background-color:#000; border:1px solid #5d5d5d` |
| `keymap1` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap2` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap3` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap4` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap5` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap6` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap7` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymap8` | `display:flex`<br>`height:20px; width:20px` | `background-image:url(data:image/png` |
| `keymapbar-enabled` | `justify-content:left; margin:0 0 0 20px`<br>`display:flex`<br>`flex:1 1 auto`<br>`display:none` | `color:#ccc` |

**⑥ 按键指派（选中键 → 选动作）**（15）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `action` | `align-items:center; display:flex`<br>`align-items:start; display:flex; flex-direction:column`<br>`height:20px`<br>`display:flex; gap:20px; justify-content:center` | `text-transform:uppercase`<br>`background-color:#000; color:#44d62c`<br>`background-color:#1a1a1a; color:#44d62c`<br>`background-color:#707070; border-radius:3px; color:#ccc; cursor:pointer; font-size:12px; text-transform:uppercase`<br>`background-color:#44d62c; border-radius:3px; color:#212121; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `action-wrapper` | `height:40px; position:relative`<br>`height:20px; left:10px; position:absolute; top:10px; width:20px` | `background-color:#0000; transition:background-color .2s`<br>`color:#ccc; font-size:12px; line-height:14px; text-align:left; text-transform:uppercase; transition:color .2s`<br>`background-color:#393939`<br>`background-color:#111`<br>`color:#44d62c` |
| `action_bar_wrapper` | `display:flex`<br>`bottom:-38px`<br>`height:20px; position:relative; width:20px`<br>`height:20px; width:20px` | `background-image:url(../../static/media/icon_emoji.07a71bf8.svg)`<br>`background-image:url(../../static/media/icon_emoji_active.b1a4efad.svg)`<br>`opacity:1`<br>`background-image:url(../../static/media/icon_close.130e45fb.svg)`<br>`background-image:url(../../static/media/icon_charactermap.025aae0b.svg)` |
| `actions` | `height:100%; max-width:250px; position:absolute; width:250px`<br>`max-width:40px`<br>`overflow:hidden`<br>`overflow:auto` | `background-color:#222; transition:max-width .2s` |
| `binding-value` | `overflow:hidden` | `line-height:16px` |
| `hoverGray-remappedBlack` | — | `fill:#000c`<br>`stroke:#000` |
| `isAssignment` | — | `fill:#44d62c66`<br>`fill:#000c`<br>`fill:#fd8611b3!important`<br>`fill:#fd8611b3` |
| `key-config` | `width:180px`<br>`position:relative; width:170px`<br>`min-height:260px`<br>`display:flex; height:calc(100vh - 140px); max-height:570px; min-height:310px` | `opacity:.3`<br>`background-color:#111; opacity:0; transition:visibility 0s,opacity 0s,left 0s`<br>`border:1px solid #5d5d5d; box-shadow:0 0 20px 0 #000000b3; cursor:default; opacity:1`<br>`border-color:#0000; box-shadow:0 0 20px 0 #000000b3; cursor:default; opacity:1`<br>`color:#44d62c; font-family:RazerF5; font-size:16px; line-height:19px; text-align:left; text-transform:uppercase` |
| `key-config-flex` | `display:flex` | — |
| `key-mapping` | `display:none`<br>`overflow:hidden; padding:0`<br>`overflow:hidden`<br>`overflow:auto` | `line-height:40px` |
| `key_mapping_text_display` | `overflow:hidden` | — |
| `keymap-action` | `padding:0 20px; width:250px`<br>`flex:0 0 auto; height:27px; margin:0 10px 0 0; min-width:100px; padding:6px 10px 7px 6px`<br>`padding:0`<br>`justify-content:flex-end` | `border:1px solid #000; border-radius:3px; font-size:12px; line-height:14px; text-align:center` |
| `notRemapped` | — | `fill:#fd8611b3`<br>`stroke:#44d62c; fill:#0000`<br>`stroke:#fd8611`<br>`stroke:#69696c`<br>`fill:#c8323c80` |
| `remap-2-disabled` | — | `color:#c8323c` |
| `remapped` | `width:164px` | `color:#44d62c`<br>`color:#fd8611`<br>`border-left:1px solid #707070; font-size:14px; line-height:16px; text-align:left` |

**⑦ 预设动作快捷按钮**（9）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `collapse-action` | — | — |
| `custom-action-btn` | `margin:0 auto`<br>`width:486px` | — |
| `flex-button` | `align-items:flex-start; display:flex; flex-direction:column; padding:2px 0` | — |
| `group-button` | `align-items:center; display:flex; height:32px; margin:0 0 10px 30px; padding:5px 3px; width:-webkit-fit-content; width:fit-content`<br>`height:22px; margin:0 2px; min-width:40px; padding:4px 8px` | `border:1px solid gray; border-radius:20px`<br>`border:0; border-radius:20px; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; line-height:14px; text-transform:uppercase`<br>`background-color:#44d62c; color:#212121` |
| `setting-action` | `align-items:center; display:flex`<br>`margin:0 5px` | — |
| `shortcutButton` | `align-items:center; display:flex; height:100%; justify-content:center; margin:0 10px; width:47px` | `border:1px solid #5d5d5d; border-radius:3px` |
| `shortcutButton-filled` | — | `background-color:#292929` |
| `shortcut_act_action` | `height:27px; padding:5px 6px` | `background-color:#0000; line-height:17px; text-transform:capitalize; transition:background-color .3s`<br>`background-color:#222` |
| `shortcut_item_action` | `align-items:center; display:flex; position:relative` | — |

**⑧ 键盘独有：组合键（两键同按）**（1）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `twoKeyMapping` | `height:20px; position:absolute; top:15px; width:20px` | `background-image:url(../../static/media/icon_secondaryfunction-1.734f766c.svg); background-size:20px 20px`<br>`color:#707070; font-size:14px; line-height:14px; text-transform:uppercase` |

**⑩ 键盘独有：动态击键 / 按键录制**（3）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `key-dks` | `position:absolute`<br>`height:10px; width:19px` | `background-size:cover`<br>`background-image:url(../../static/media/dynamic_key_stroke_icon.c4bd98bb.svg)` |
| `macro-keypad-module` | `min-height:auto`<br>`position:static`<br>`position:absolute`<br>`flex-direction:column` | `font-size:14px`<br>`text-transform:capitalize`<br>`background-color:#000; border:1px solid #5d5d5d; border-radius:20px`<br>`background-color:#3cbf27; border-radius:20px; color:#212121`<br>`opacity:1` |
| `marco-keymap` | — | — |

**⑪ 通用控件**（25）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `button` | `align-items:center; display:flex; flex:none; flex-direction:row; flex-grow:0; gap:10px; height:27px; justify-content:center; order:2; padding:7px 16px 6px; width:173px`<br>`align-items:center; display:flex; flex:none; flex-direction:row; flex-grow:0; gap:10px; height:27px; justify-content:center; order:5; padding:7px 16px 6px; width:173px`<br>`margin:9px auto 0; padding:7px 16px; width:-webkit-fit-content; width:fit-content` | `background:#707070; border:1px solid #0000004d; border-radius:3px; font-family:Roboto; font-size:12px; font-weight:400; line-height:14px; text-align:center; text-transform:uppercase`<br>`background-color:#ffffff1a`<br>`background:#44d62c; border:1px solid #0000004d; border-radius:3px; color:#222; font-family:Roboto; font-size:12px; font-weight:400; line-height:14px; text-align:center; text-transform:uppercase`<br>`opacity:.8`<br>`background-color:#707070; border-radius:3px; cursor:pointer; text-transform:uppercase` |
| `button--cta` | `margin:9px auto 0; padding:7px 16px; width:-webkit-fit-content; width:fit-content`<br>`height:18px; position:relative; width:18px`<br>`left:0; max-width:300px; width:-webkit-max-content; width:max-content` | `background-color:#707070; border-radius:3px; cursor:pointer; text-transform:uppercase`<br>`background-position:50%; background-repeat:no-repeat; background-size:18px 18px`<br>`opacity:1` |
| `button--cta-close` | `height:24px; position:absolute; right:0; top:0; width:24px` | `background-image:url(../../static/media/icon_close.55fe41f1.svg); background-position:50%; background-size:24px 24px` |
| `button--cta-get-control` | `align-items:center; display:flex; height:27px; justify-content:center; margin:9px auto 0; padding:0 16px; width:-webkit-fit-content; width:fit-content` | `background-color:#707070; border-radius:3px; color:#ccc; color:#fff; cursor:pointer; font-size:14px; text-transform:uppercase` |
| `button--cta-link` | `left:0; max-width:300px; width:-webkit-max-content; width:max-content` | `background-image:url(../../static/media/unlink-btn.db633bad.svg)`<br>`opacity:1` |
| `button--cta-link-link` | — | `background-image:url(../../static/media/unlink-btn.db633bad.svg)`<br>`background-image:url(../../static/media/unlink-hovered-btn.67ac688d.svg)` |
| `button--cta-link-unlink` | — | `background-image:url(../../static/media/link-btn.fed18bb7.svg)`<br>`background-image:url(../../static/media/link-hovered-btn.0fde7f57.svg)` |
| `button--cta-remove` | `display:block; height:27px; margin:0 auto; min-width:90px; width:-webkit-fit-content; width:fit-content` | `background-color:#fd4949; border:1px solid #0000004d; border-radius:3px; color:#111; cursor:pointer; font-size:12px; line-height:14px; text-align:center; text-transform:uppercase` |
| `button--cta-show-active` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 15px 0 0; padding:0 16px; width:-webkit-fit-content; width:fit-content` | `background-color:#292929; border:1px solid #44d62c; border-radius:3px; color:#ccc; cursor:pointer; font-size:14px; text-transform:uppercase` |
| `button--cta-show-busy` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 auto; padding:0 16px; width:-webkit-fit-content; width:fit-content` | `border:1px solid #fd8611; border-radius:3px; color:#ccc; color:#fd8611; cursor:pointer; font-size:14px; text-transform:uppercase` |
| `button--cta-show-locked` | `align-items:center; display:flex; height:27px; justify-content:center; margin:0 10px 0 auto; padding:0 16px 0 40px; position:relative; width:-webkit-fit-content; width:fit-content`<br>`display:block; height:24px; left:0; position:absolute; top:0; width:24px` | `border:1px solid #e5e5e5; border-radius:3px; color:#ccc; color:#e5e5e5; cursor:pointer; font-size:14px; text-transform:uppercase`<br>`background-image:url(../../static/media/icon_lock_white.c066cd4d.svg); background-position:100%; background-repeat:no-repeat; background-size:70% 70%` |
| `button--cta-unlink` | — | `background-image:url(../../static/media/link-btn.fed18bb7.svg)`<br>`background-image:url(../../static/media/link-hovered-btn.0fde7f57.svg)` |
| `button-description` | `max-width:250px; overflow:hidden`<br>`align-items:flex-start; display:flex; flex-direction:column; padding:2px 0` | `color:#44d62c; font-size:10px; line-height:12px; text-transform:uppercase`<br>`color:#fd8611` |
| `button-group` | `display:flex; flex-wrap:wrap; gap:10px`<br>`height:70px; min-width:63px; padding:40px 0 0; width:63px` | `background-color:#111; background-repeat:no-repeat; border:1px solid gray; color:#ccc; font-family:Roboto,sans-serif; font-size:10px; line-height:0; text-align:center; text-transform:none`<br>`border:1px solid #44d62c`<br>`background-image:url(../../static/media/standard.cbf1362d.svg)`<br>`background-image:url(../../static/media/fast.43db44c4.svg)`<br>`background-image:url(../../static/media/slow.8b9c0df7.svg)` |
| `button_icon` | `align-items:center; display:flex` | `color:#707070; font-size:14px; line-height:17px; text-align:left`<br>`fill:#fff`<br>`fill:#44d62c` |
| `buttons-block` | `align-items:flex-start; display:flex; flex-direction:column; gap:16px; height:97px; padding:0 50px 50px; width:480px`<br>`align-items:flex-start; display:flex; flex-direction:row; gap:12px; height:4px; padding:0`<br>`height:4px; position:relative; width:4px`<br>`align-items:flex-start; display:flex; flex-direction:row; gap:10px; height:27px; padding:0; width:190px` | `opacity:1`<br>`background-color:#707070; border-radius:10px; transition:none`<br>`background-color:#fff`<br>`background:#707070; border:1px solid #0000004d; border-radius:3px; color:#fff; cursor:pointer; font-family:Roboto; font-size:12px; font-weight:400; line-height:14px; opacity:1; text-align:center; text-transform:uppercase; transition:background-color .3s,opacity .3s`<br>`background-color:#8a8a8a` |
| `buttons-row` | `align-items:flex-start; display:flex; flex-direction:row; gap:10px; height:27px; padding:0; width:190px` | — |
| `enable-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:90px` | `background-color:#44d62c; border-radius:3px; color:#212121; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `icon-button` | — | `fill:#44d62c`<br>`fill:#44d62c; opacity:.7` |
| `login-button` | `height:27px; min-width:90px; padding:7px 16px 6px; width:-webkit-max-content; width:max-content` | `background-color:#44d62c; border-radius:3px; color:#111; cursor:pointer; text-align:center; text-transform:uppercase; transition:all .3s ease-out`<br>`background-color:#44d62cb3`<br>`background-color:#fff` |
| `ok-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:90px` | `background-color:#707070; border-radius:3px; color:#ccc; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `razer-button` | `min-width:90px; position:relative; width:-webkit-fit-content; width:fit-content`<br>`height:100%; left:0; position:absolute; top:0; width:100%` | `background-color:#44d62c; border:1px solid #000; border-radius:3px; color:#000; cursor:default; line-height:27px; text-transform:uppercase`<br>`background:#ffffff4d; border-radius:3px`<br>`background:#44d62cb3!important`<br>`background:none` |
| `switch-button` | `height:30px; width:90px` | `background-color:#333; border:1px solid #555; color:#ccc; cursor:pointer; font-size:14px; text-transform:uppercase; transition:background-color .3s,color .3s`<br>`background-color:#44d62c; border-color:#44d62c; color:#111`<br>`background-color:#444` |
| `switch_button` | `height:22px; margin:0 2px; min-width:40px; padding:4px 8px` | `border:0; border-radius:20px; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; line-height:14px; text-transform:uppercase`<br>`background-color:#44d62c; color:#212121` |
| `tooltip-panel-button` | `left:61px; max-width:358px; padding:8px 10px; position:absolute` | `background-color:#000; border:1px solid #5d5d5d; color:#ccc; font-size:14px; line-height:16px; opacity:0; text-align:left`<br>`opacity:1` |

**⑫ 其它**（38）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `AudioMeter_flowButtonActive__vq7uT` | — | `background:#292929; border:1px solid #44d62c` |
| `AudioMeter_flowButtonIcon__p7D6V` | `height:18px; width:18px` | — |
| `AudioMeter_flowButton__4sIhG` | `align-items:center; display:flex; flex-direction:column; gap:5px; height:66px; justify-content:center; min-width:160px; padding:7px 16px 6px; width:160px`<br>`height:18px; width:18px` | `background:#111; border:1px solid #5d5d5d; border-radius:3px; color:#ccc; cursor:pointer; font-family:Roboto,sans-serif; font-size:12px; font-weight:400; opacity:1; text-transform:uppercase; transition:all .2s ease` |
| `AudioMeter_flowButtons__tOGNz` | `display:flex; gap:5px` | — |
| `Dialog_importProfileInfoItem__hy7pT` | `align-items:center; display:flex; height:31px; position:relative`<br>`align-items:center; display:flex; height:100%; position:relative; width:20px`<br>`padding:8px 10px 10px; position:absolute; top:100%`<br>`align-self:stretch; width:3px` | `color:#ccc; font-family:Roboto; font-size:14px`<br>`background-color:#000; border:1px solid #5d5d5d; opacity:0; transition:visibility 0s,opacity .3s linear`<br>`opacity:1` |
| `HelpComponent_supportButton__S0UnH` | — | `cursor:pointer`<br>`color:#44d62c` |
| `MonitoringDashboard_menuButton__IkMX-` | `align-items:center; display:flex; justify-content:center; padding:0; position:relative` | `background:none; border:1px solid #0000; border-radius:0; color:#ccc; cursor:pointer`<br>`outline:1px dotted #0f0`<br>`border-color:#5d5d5d`<br>`color:#999`<br>`color:#fff` |
| `MonitoringToggle_button__xH4PT` | `align-items:center; display:flex; justify-content:center; padding:4px` | `background:none; border:1px solid #0000; border-radius:0; color:#999; cursor:pointer; transition:all .2s ease`<br>`border-color:#5d5d5d; color:#fff`<br>`outline:1px dotted lime` |
| `SWITCH_DEVICE_PROFILE` | — | `background-image:url(../../static/media/icon_config_switch_device_profile.88b9f2aa.svg)`<br>`background-image:url(../../static/media/icon_config_switch_device_profile_a.942d360a.svg)` |
| `SWITCH_KEYMAP` | — | `background-image:url(../../static/media/icon_config_keymap.cb5ba1f0.svg)`<br>`background-image:url(../../static/media/icon_config_keymap_a.e27c970d.svg)` |
| `SWITCH_PROFILE` | — | `background-image:url(../../static/media/icon_config_switch_device_profile.88b9f2aa.svg)`<br>`background-image:url(../../static/media/icon_config_switch_device_profile_a.942d360a.svg)` |
| `WaveWithSpeedV2_waveButtons__vAgEX` | `display:grid; gap:20px` | — |
| `add-keymap` | `display:flex; max-width:100%; min-height:25px; overflow:hidden; padding:4px 5px; width:100%`<br>`padding:0 4px` | `border-top:1px solid #515151; font-size:14px; text-transform:none`<br>`background-color:#ffffff1a` |
| `custom-keymapping-stepper` | `width:90%` | `border:1px solid #5d5d5d`<br>`border:1px solid #44d62c`<br>`background-color:initial; text-align:left; text-align:initial`<br>`opacity:1` |
| `customize-setting-button` | `width:150px`<br>`align-items:center; display:flex; height:27px; justify-content:center; min-width:90px` | `background-color:#707070; border-radius:3px; color:#fff; font-size:12px; text-transform:uppercase`<br>`opacity:.3` |
| `installation-action` | `margin:0 0 0 30px`<br>`height:27px; width:auto` | `opacity:.6` |
| `key-require-macro-module` | `height:20px; position:absolute; width:20px` | `background-size:cover`<br>`background-image:url(../../static/media/require_macro_module_icon.20694b5c.svg)` |
| `key-require-synapse` | `height:20px; position:absolute; width:20px` | `background-image:url(../../static/media/require_synapse_icon.d5e656b0.svg)`<br>`background-size:cover` |
| `key-snap-tap` | `height:16px; width:16px`<br>`position:absolute` | `background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg)`<br>`background-size:cover` |
| `key-tip` | `max-width:300px; min-height:75px; padding:8px 10px; position:absolute`<br>`overflow:hidden!important`<br>`padding:initial` | `background-color:#000; border:1px solid #5d5d5d; opacity:0; text-align:center; transition:opacity .3s,visibility 0s,left 0s,top 0s`<br>`opacity:1`<br>`color:#707070; font-size:14px`<br>`line-height:14px; text-transform:uppercase`<br>`color:#ccc; font-size:12px` |
| `message-container__button` | — | `background-color:#44d62c; color:#111` |
| `no-chroma-studio-profile` | — | `color:#44d62c` |
| `optionButtonGroup_active__-z-LG` | — | `background:#292929; border-color:#44d62c`<br>`background:#292929; border:1px solid #44d62c` |
| `optionButtonGroup_disabled__Q0Wrc` | — | `background:#ffffff1a`<br>`background:#292929; border:1px solid #44d62c`<br>`color:#ffffff80; cursor:not-allowed`<br>`opacity:.5` |
| `optionButtonGroup_effect-with-duration__PkoU2` | `display:flex; flex-direction:column` | `opacity:.5` |
| `optionButtonGroup_effects-area__8iBAF` | `display:flex; flex-direction:column`<br>`align-items:flex-start` | — |
| `optionButtonGroup_option-button-group__TUmjj` | `display:flex; gap:5px` | — |
| `optionButtonGroup_option-button__C62Cv` | `height:27px; margin:0; min-width:90px; padding:7px 16px 6px` | `background:#0000; border:1px solid #5d5d5d; border-radius:3px; color:#ffffffe6; cursor:pointer; font-family:Roboto; font-size:12px; font-weight:400; letter-spacing:0; line-height:14px; outline:none; text-align:center; text-transform:uppercase; transition:all .2s ease`<br>`background:#ffffff1a`<br>`background:#292929; border:1px solid #44d62c`<br>`color:#ffffff80; cursor:not-allowed` |
| `optionButtonGroup_option-label__J0Xc0` | — | `font-family:Roboto; font-size:14px; font-weight:400; line-height:14px; text-transform:uppercase` |
| `optionButtonGroup_random-color__wSIuX` | — | — |
| `optionButtonGroup_with-option-buttons__gEnlO` | `display:flex; flex-direction:column`<br>`align-items:flex-start` | — |
| `remove-text-keymapping` | `height:16px; position:absolute; right:5px; top:5px; width:16px` | `background-image:url(../../static/media/icon_close_enclosed.6056b667.svg); background-position:50%; background-repeat:no-repeat; background-size:20px; transition:background-image .2s`<br>`background-image:url(../../static/media/icon_close_enclosed_a.83aff4bb.svg); cursor:pointer` |
| `s3-modal--action-take-control` | — | `border:1px solid #707070`<br>`text-align:left` |
| `secondary-button` | `align-items:center; display:flex; height:27px; justify-content:center; width:90px` | `background-color:#707070; border-radius:3px; color:#ccc; cursor:pointer; font-size:12px; text-transform:uppercase` |
| `show-all-button` | — | `cursor:pointer; font-size:14px; text-align:center; text-transform:capitalize`<br>`color:#44d62c` |
| `show-all-button-arrow` | `height:20px; width:20px` | `transition:fill .2s ease` |
| `sliderChart__reset-button` | `height:300px; min-width:20px; right:40px` | `background:url(../../static/media/eq_reset.e0c3c09c.svg) no-repeat 50%; background-size:20px 20px; opacity:.85; transition:background-image .3s`<br>`background-image:url(../../static/media/eq_reset_hover.186df33c.svg)`<br>`background-image:url(../../static/media/eq_reset_active.37c570d3.svg)` |
| `text-standard-appli` | — | `color:#707070; font-size:12px; line-height:5px` |

## 3. 功能项

该界面对应的雷云文案 key，共 **192** 条（中文为雷云原文）：

| key | 中文 | English |
|---|---|---|
| `ADD` | 添加 | Add |
| `ADDED` | 已新增 | Added |
| `ADD_AGAIN_GAME` | 删除的游戏。 | Add again game that was removed. |
| `ADD_ANOTHER_DEDICATED` | 添加另一个特定按键 | Add another dedicated key |
| `ADD_ANOTHER_KEY` | 添加另一个按键 | Add another key |
| `ADD_AN_APPLICATION` | 添加应用程序 | Add an application |
| `ADD_AN_APP_TO_CUSTOMIZE_SHORTCUTS` | {添加应用程序}，自定义不同于默认应用程序配置文件的快捷键。 | {{Add an app}} to customize shortcuts different from the Default App Profile. |
| `ADD_APPLICATION_TITLE` | 添加应用程序 | Add an App |
| `ADD_APP_DES` | to customize shortcuts different from the Default App Profile. | to customize shortcuts different from the Default App Profile. |
| `ADD_AUDIO_CUES` | 添加音频提示 | Add Audio Cues |
| `ADD_AUTOMATION` | 添加自动化操作 | Add Automation |
| `ADD_A_SURFACE` | 添加表面 | ADD A SURFACE |
| `ADD_COMMENT` | 添加注释 | Add Comment |
| `ADD_DETAILS` | 添加详细信息…… | Add details... |
| `ADD_EDIT` | 添加编辑 | Add Edit |
| `ADD_EDIT_CUT_AT_PLAYHEAD` | 添加编辑（在播放头剪切） | Add Edit (Cut at Playhead) |
| `ADD_GAME` | 添加游戏 | Add game |
| `ADD_GAME_AND_PROGRAM` | 游戏与程序 | Add Games & Programs |
| `ADD_GAME_OR_PROGRAM` | 添加游戏或程序 | Add games & programs |
| `ADD_GAME_TITLE` | 添加游戏/程序 | ADD A GAME/PROGRAM |
| `ADD_GAME_TO_START` | 添加游戏或前往“已关联的游戏”开始体验。 | Add a game or go to Linked Games to get started. |
| `ADD_GLOBAL_SHORTCUT_TO_START` | 添加通用快捷键以开始使用。 | Add a global shortcut to get started. |
| `ADD_HANGING_INDENT` | 添加悬挂缩进 | Add Hanging Indent |
| `ADD_KEY_LIGHT` | 添加补光灯 | ADD KEY LIGHT |
| `ADD_KEY_LIGHTS_DES` | 添加你的 Razer 雷蛇 Wi-Fi 设备以增强并最大化地提升你的体验。 | Add your Razer Wi-Fi device to enhance and maximize your experience. |
| `ADD_MACRO_EVENT` | 添加宏事件 | Add macro event |
| `ADD_MACRO_TO_START` | 添加宏以开始使用。 | Add a macro to get started. |
| `ADD_MARKER_WITH_TEXT` | 添加文字标记 | Add Marker With Text |
| `ADD_MAT` | 添加 | ADD |
| `ADD_MODULE_ANYTIME` | 你可以随时添加此模块。 | You can add this module anytime |
| `ADD_MORE_EQ_PRESET` | 添加更多均衡器预设 | Add more EQ presets |
| `ADD_NEW` | 新增 | Add new |
| `ADD_NEW_DEVICE` | 添加新设备 | Add a new device |
| `ADD_NEW_INPUT_HEADER` | 添加新输入 | Add New Input |
| `ADD_NEW_INPUT_TIP` | 添加虚拟输入通道和/或设备以灵活控制你的直播。 | Add virtual input channels and/or devices to control your stream flexibly. |
| `ADD_NEW_KEY_PAIR` | + 新增 | + ADD NEW |
| `ADD_NEW_MODE` | 添加新模式 | Add new mode |
| `ADD_OTHER_WIFI_DEVICE` | 添加游戏空间设备 | ADD GAMER ROOM DEVICE |
| `ADD_OTHER_WIFI_DEVICE_HOME_DESC` | 必须先使用 Razer Gamer Room 应用程序在你的网络中设置设备，之后你才能将设备添加到 Razer Synapse 雷云。 | Before you can add your device on Razer Synapse, you must first set it up on your network using the Razer Gamer Room app. |
| `ADD_OTHER_WIFI_DEVICE_HOME_DESC1` | 在网络中设置好你的设备后，搜索此设备并将其添加到 Razer Synapse 雷云中。 | After you've set up your device on your network, search and add the device to Razer Synapse. |
| `ADD_OTHER_WIFI_DEVICE_HOME_MOBILE_DEVICE_DESC` | 将 Gamer Room 应用程序下载到移动设备上。 | Get the Gamer Room app on your mobile device. |
| `ADD_OTHER_WIFI_DEVICE_HOME_SCAN_DESC` | 确保你已通过 Razer Gamer Room 应用程在网络中设置了你的设备。 | Make sure you've already set up your device on your network from the Razer Gamer Room app. |
| `ADD_SHORTCUT` | 添加快捷键 | ADD A SHORTCUT |
| `ADD_TO_RENDER_QUEUE` | Add to Render Queue | Add to Render Queue |
| `ADD_TRANSITION_AT_EDIT_POINT` | 在编辑点添加过渡 | Add Transition at Edit Point |
| `ADD_WIFI_DEVICE` | 添加 Wi-Fi 设备 | Add WiFi Device |
| `ASSIGN` | 指定 | ASSIGN |
| `ASSIGNED_KEY` | 分配的键 | Assigned Key |
| `ASSIGN_MACRO_DESC` | 从设备页面或宏模块分配宏。 | Assign a macro from the device page or the Macro module. |
| `ASSIGN_NOW` | 立即指定 | ASSIGN NOW |
| `DEFAULT` | 默认 | DEFAULT |
| `DEFAULT_APP_PROFILE` | 默认应用程序配置文件 | Default App Profile |
| `DEFAULT_DEVICE` | DEFAULT {{deviceType}} DEVICE | DEFAULT {{deviceType}} DEVICE |
| `DEFAULT_DEVICE_DESC` | 将“{{deviceName}}”  | Set “{{deviceName}}”  |
| `DEFAULT_DEVICE_INPUT_DESC` | 设置为默认音频设备。 | as the default input device to configure its mic settings on Razer Synapse. |
| `DEFAULT_DEVICE_OUTPUT_DESC` | 设置为默认音频设备。 | as the default output device to configure its audio settings on Razer Synapse. |
| `DEFAULT_GAME_MODE` | 游戏开发者认可的游戏模式。 | Certified Game Modes by the game developers. |
| `DEFAULT_INPUT_DEVICE` | 默认输入设备 | DEFAULT INPUT DEVICE |
| `DEFAULT_OUTPUT_DEVICE` | 默认输出设备 | DEFAULT OUTPUT DEVICE |
| `DEFAULT_OUTPUT_DEVICE_DESC` | 将扬声器或耳机 (Realtek(R) Audio Codec with THX Spatial Audio) 设置为默认输出设备，然后即可在 Razer Synapse 雷云中配置其音频设置。 | Set Speakers or Headphones (Realtek(R) Audio Codec with THX Spatial Audio) as the default output device to configure its audio settings on Razer Synapse. |
| `DEFAULT_PLAYBACK_DEVICE` | Default Playback Device | Default Playback Device |
| `DEFAULT_RECORDING_DEVICE` | Default Recording Device | Default Recording Device |
| `DEFAULT_SETTINGS` | 默认设置 | Default Settings |
| `DELETE` | 删除 | Delete |
| `DELETE_ACTION` | 删除操作 | Delete Action |
| `DELETE_ACTION_DESC` | 此自动化操作将被删除。 | This automated action will be removed. |
| `DELETE_ALL` | 全部删除 | Delete All |
| `DELETE_ALL_SENSITIVITY_MATCHER_POPUP_DESC` | 当你重置灵敏度匹配时，所有配置都将丢失并重置为默认值。你需要重新进行校准。 | When you reset the sensitivity matcher, all configurations will be lost and reset to default. You will need to recreate your calibrations. |
| `DELETE_ALL_SENSITIVITY_MATCHER_POPUP_HEADER` | 删除所有灵敏度匹配 | DELETE ALL SENSITIVITY MATCHER |
| `DELETE_BAND` | 删除均衡器频段 | Delete EQ Band |
| `DELETE_CELLS` | Delete Cells (Row/Column) | Delete Cells (Row/Column) |
| `DELETE_CHROMA_EFFECT` | 删除 Chroma 幻彩效果 |  |
| `DELETE_CHROMA_EFFECT_DESC` | 你将要删除此 Chroma 幻彩效果。 此按键映射中的所有层将会被删除。 |  |
| `DELETE_CONTENT` | 删除内容 | Delete content |
| `DELETE_GAME` | 删除游戏 | Delete Game |
| `DELETE_KEY` | 删除按键 | Delete key |
| `DELETE_KEYMAP_MSG` | 你将要删除此按键映射。此按键映射中的所有绑定将会被删除。 | You’re about to delete this keymap. All bindings in this keymap will be deleted. |
| `DELETE_KEYMAP_TITLE` | 删除按键映射 | DELETE KEYMAP |
| `DELETE_PROFILE_MSG` | 你将要删除此配置文件。此配置文件中的所有绑定将会被删除。 | You’re about to delete this profile. All bindings in this profile will be deleted. |
| `DELETE_PROFILE_TITLE` | 删除配置文件 | DELETE PROFILE |
| `DELETE_ROW_COLLUMN` | Delete Row/Column | Delete Row/Column |
| `DELETE_ROW_COLUMN` | 删除行/列 | Delete Row/Column |
| `DELETE_SELECTED_CELLS` | 删除所选单元格 | Delete Selected Cells |
| `DELETE_SHORTCUT` | 删除快捷键 | DELETE SHORTCUT |
| `DELETE_SHORTCUT_MESSAGE` | 你将要删除此快捷键。 | You’re about to delete this shortcut. |
| `DELETE_SPATIAL_OUTPUT_MSG` | 你将要删除此模式。 使用此模式的所有应用程序都会设为默认值。 | You're about to delete this mode. All applications using this mode will be set to default. |
| `DELETE_SPATIAL_OUTPUT_TITTLE` | 删除模式 | Delete mode |
| `DISABLE` | 禁用 | Disable |
| `DISABLED` | 已禁用 | Disabled |
| `DISABLEDASSIGNMENTTYPE` | 已禁用 | Disabled |
| `DISABLED_BY_COLOR_TEMPERATURE` | Certain settings will not be available with this Color Temperature. | Certain settings will not be available with this Color Temperature. |
| `DISABLED_BY_DUPLICATED_DISPLAY` | 复制显示时不可用 | Unavailable for Duplicate Display |
| `DISABLED_BY_GAMUT` | 当前选择的色域将无法使用设置。 | Settings will not be available with current selected color gamut. |
| `DISABLED_BY_HDR` | 启用 HDR 后，设置由 Windows 管理。 | Settings are managed by Windows when HDR is enabled. |
| `DISABLED_BY_HDR_2` | 启用 HDR 后不可用。 | Not available while HDR is enabled. |
| `DISABLED_BY_NOT_ACTIVE_DISPLAY` | 当设备不是活跃显示器时不可用。 | Not available when the device is not the active display. |
| `DISABLED_BY_PIP` | 在画中画 (PIP) 中不可用。 | Not available while in PIP. |
| `DISABLED_BY_THX_CINEMA` | 启用“THX 影院”后，设置将由该功能管理。 | Settings are managed by THX Cinema when it is enabled. |
| `DISABLED_FEATURE_DESC` | 此功能仍在开发中。 | This feature is currently in the works. |
| `DISABLED_MONITOR_LIGHTING_WARNING` | HDMI 和 DP 端口无法使用灯光设置。 | Lighting settings are not available for HDMI and DP ports. |
| `DISABLE_ALT_F4` | 禁用 Alt + F4 | Disable Alt + F4 |
| `DISABLE_ALT_SPACE` | 禁用 Alt + Space | Disable Alt + Space |
| `DISABLE_ALT_TAB` | 禁用 Alt + Tab | Disable Alt + Tab |
| `DISABLE_COMMAND_DIAL_TIPS` | At least one default mode must remain enabled. | At least one default mode must remain enabled. |
| `DISABLE_COPILOT_KEY` | 禁用 Copilot 键 | Disable Copilot Key |
| `DISABLE_FN_SLEEP` | 禁用 FN + {{keyName}} | Disable FN + {{keyName}} |
| `DISABLE_FN_Z` | Disable FN + Z | Disable FN + Z |
| `DISABLE_FREE_SPIN` | 禁用自由滚动模式 | Disable Free-Spin |
| `DISABLE_MENU_KEY` | 禁用菜单键 | Disable Menu Key |
| `DISABLE_MICROPHONE` | Disable Microphone | Disable Microphone |
| `DISABLE_MICRO_TACTILE` | 禁用精确触觉滚动 | Disable Precision Tactile |
| `DISABLE_MONITORING` | 	禁用监控 | Disable Monitoring |
| `DISABLE_MULTI_FUNCTION` | LED 将出现瞬间变化，表示输入已高于模拟增益限制器。 | Disabling multi-function controls will make the sensor more responsive but will also remove the additional functionalities.  |
| `DISABLE_PLAYBACK_MIX` | 禁用播放混音 | Mute Playback Mix |
| `DISABLE_STREAM_MIX` | 禁用直播混音 | Disable Stream Mix |
| `DISABLE_TACTILE` | 禁用触觉滚动模式 | Disable Tactile |
| `DISABLE_WINDOWS_KEY` | 禁用 Windows 键 | Disable Windows Key |
| `DISABLE_XY` | 禁用 X-Y 轴 | Disable X-Y |
| `DUPLICATE` | 复制 | Duplicate |
| `DUPLICATE_LAYER` | 复制图层 | Duplicate Layer |
| `DUPLICATE_SELECTED_LAYERS` | 复制所选图层 | Duplicate Selected Layers |
| `DUPLICATE_SELECTION` | 复制所选内容 | Duplicate Selection |
| `DUPLICATE_SHORTCUT_MESSAGE` | 已有操作使用了此按键组合。 | This key combination is being used for existing action(s). |
| `DUPLICATE_SLIDE` | Duplicate Slide | Duplicate Slide |
| `EXPORT` | 导出 | Export |
| `EXPORT_FRAME` | 导出帧 | Export Frame |
| `EXPORT_MEDIA` | 导出媒体 | Export Media |
| `HYPERSHIFT` | Hypershift | Hypershift |
| `HYPERSHIFT_CLUTCH` | Hypershift 切换 | Hypershift Clutch |
| `HYPERSHIFT_TOOLTIP` | 在此处配置 Razer Hypershift 快捷键。 利用 Razer Hypershift 键尽享额外按键集带来的酷炫功能。 | Configure Razer Hypershift shortcuts here. Enjoy an extra set of buttons using the Razer Hypershift key. |
| `IMPORT` | 导入 | Import |
| `IMPORT_EXPORT_MODAL_EMPTY_PROFILE` | Unable to import profile as the file might be empty, corrupted, and/or not compatible. Please try again. | Unable to import profile as the file might be empty, corrupted, and/or not compatible. Please try again. |
| `IMPORT_EXPORT_MODAL_SYNAPSE3_PROFILE` | Unable to import Razer Synapse 3 profiles. We're working to enable this function as quickly as possible. | Unable to import Razer Synapse 3 profiles. We're working to enable this function as quickly as possible. |
| `IMPORT_EXPORT_MODAL_TIP` | Not all mappings from this profile can be carried over to this device. | Not all mappings from this profile can be carried over to this device. |
| `IMPORT_EXPORT_MODAL_WILL_NOT_IMPORT` | 不会导出已关联的游戏和 Chroma 幻彩效果. | Linked Games and Chroma Effects will not be exported. |
| `IMPORT_FILE` | Import File | Import File |
| `IMPORT_MEDIA` | 导入媒体 | Import Media |
| `KEYMAP` | 按键映射 | Keymap |
| `KEYSWITCH_OPTIMIZATION` | 按键开关优化 | KEYSWITCH OPTIMIZATION |
| `KEYSWITCH_OPTIMIZATION_TOOLTIPS` | 选择你希望如何根据击键方式来优化轴。<br> <br>键入模式增加了回弹延迟，防止单次敲击产生额外的输入。<br> <br>游戏模式没有回弹延迟，因此在按键触发后会快速响应。 | Select how you would like the switches to be optimized based on your keystrokes.<br> <br>Typing mode adds a debounce delay to prevent extra inputs from a single keystroke.<br> <br>Gaming mode has zero debounce making it hyper responsive after the key is actuated. |
| `KEYSWITCH_OPTIMIZATION_TOOLTIPS_V2` | 调整键盘的响应速度，优化按键输入或使按键的按下或释放过程更快。 | Adjust the keyboard's responsiveness for optimized typing or swifter keypresses and releases. |
| `KEY_LIGHTS` | 补光灯 | KEY LIGHTS |
| `KEY_MENU` | 菜单 |  |
| `KEY_REPLACEMENT_PROMPT` | 所选按键可替换为设备操作系统的相应按键。 | The selected keys can be replaced with the corresponding ones for your device's OS. |
| `KEY_SELECTED` | 已选按键 | Key selected |
| `KEY_SHIFTER` | 变调 | KEY SHIFTER |
| `KEY_SHIFTER_TOOLTIP` | 启用即可使用滑块调整线路输入端口上任意音频输入的音高和速度。 | Adjust the pitch and tempo of any audio input on the Line In port using the slider. |
| `KEY_SYSTEM_GROUP` | 系统组 | System Group |
| `LEFT_HANDED` | 左手型 | Left-handed |
| `MOUSE_USE` | 鼠标使用 | MOUSE USE |
| `MOUSE_USE_TOOLTIP` | 选择是否要为左手或右手玩家进行鼠标配置。 | Select whether the mouse should be configured for a right or left handed person. |
| `PROFILE` | 配置文件 | PROFILE |
| `PROFILES` | 配置文件 | Profiles |
| `PROFILE_APPLIED_CLICK_TO_VIEW_CHROMA` | Profile applied! Click {{clickHere}} to launch Chroma Studio. | Profile applied! Click {{clickHere}} to launch Chroma Studio. |
| `PROFILE_APPLIED_CLICK_TO_VIEW_DEVICE` | Profile applied! Click {{clickHere}} to view device configuration. | Profile applied! Click {{clickHere}} to view device configuration. |
| `PROFILE_DETAILS` | 配置文件详情 | PROFILE DETAILS |
| `PROFILE_DETAILS_DOWNLOAD_TIP` | 你已有此内容 | You have downloaded this content. |
| `PROFILE_DIFFERENT_DEVICE_DESC` | 此配置文件中的某些设置可能无法与你的设备正常配合使用。 | Some settings in this profile may not work properly with your device. |
| `PROFILE_DIFFERENT_DEVICE_TITLE` | Profile created for different device | Profile created for different device |
| `PROFILE_DISABLED_TOOLTIP` | 这些设置保存在本地，并应用于所有配置文件。 | These settings are saved locally and are applied to all profiles. |
| `PROFILE_HAS_UPLOADED` | 此配置文件已上传。 | This Profile has been uploaded. |
| `PROFILE_HAVE_MACRO_WARNING` | 此配置文件包含一个不会被上传的宏。你可以单独上传宏，也可以单击图标删除宏，重新配置配置文件。 | This profile includes a macro that won't be uploaded. You can either upload the macro separately or remove it by clicking the icon to reconfigure the profile. |
| `PROFILE_LOWERCASE` | 配置文件 | profile |
| `PROFILE_MIGRATION` | SYNAPSE 雷云 3 配置文件迁移 | SYNAPSE 3 Profile Migration |
| `PROFILE_MIGRATION_DESC` | 将 Synapse 雷云 3 中所有已连接设备的配置文件、宏和 Chroma 幻彩效果迁移到 Synapse 雷云 4。 | Transfer your connected device's Profiles, Macros and Chroma Effects from Synapse 3 to Synapse 4. |
| `PROFILE_MIGRATION_DESC_2` | 你在 Synapse 雷云 4 中的现有配置文件不会受到影响，而备份文件将保存在此 | Your existing profiles in Synapse 4 will not be affected and backup files will be saved in this |
| `PROFILE_MIGRATION_HEADER_1` | 迁移你的 Synapse 雷蛇 3 配置文件 | Migrate your Synapse 3 profiles |
| `PROFILE_PLACEHOLDER` | 配置文件 | Profile |
| `PROFILE_SLOT_1` | 配置文件槽位 1 | Profile Slot 1 |
| `PROFILE_SLOT_2` | 配置文件槽位 2 | Profile Slot 2 |
| `PROFILE_SLOT_3` | 配置文件槽位 3 | Profile Slot 3 |
| `PROFILE_SLOT_4` | 配置文件槽位 4 | Profile Slot 4 |
| `PROFILE_SWITCHING` | 配置文件切换 | Profile Switching |
| `PROFILE_SWITCHING_AUTO_DES` | 只要你使用相应的应用程序，应用程序配置文件就会自动应用。当所列应用程序都未处于活动状态时，将应用默认应用程序配置文件。 | An App Profile will be automatically applied whenever you use its corresponding app. The Default App Profile will be applied when none of the listed apps are active. |
| `PROFILE_SWITCHING_MANUAL_DES` | 所选的应用程序配置文件始终处于活动状态，即使相关应用程序没有运行。 | The selected App Profile is always active, even when the related app is not running. |
| `RENAME` | 重命名 | Rename |
| `RESET_PROFILE` | 重置 | Reset |
| `RESET_PROFILE_BUTTON` | 重置配置文件 | RESET PROFILE |
| `RESET_PROFILE_DESC` | 当你重置配置文件时，所有按键绑定和已配置的设置都将丢失并重置为默认值。你可以重置配置文件或选择仅重置所有按键绑定。 | When you reset a profile, all key binds and configured settings will be lost and reset to default. You may reset the profile or opt to just reset all key binds. |
| `RESET_PROFILE_KEYBINDS_BUTTON` | 重置按键绑定 | RESET KEY BINDS |
| `RESET_PROFILE_TITLE` | 重置配置文件 | Reset Profile |
| `RESHARE` | 重新上传 | Reupload |
| `RESHARE_BUTTON_IN_CHROMA` | Launch Workshop To Retake | Launch Workshop To Retake |
| `RESHARE_DESC` | 配置文件已被修改，请重新上传到 Workshop 工作室。 | Your profile has been modified, reshare it to Workshop. |
| `RESHARE_DESC_IN_CHROMA` | Your profile has been modified—retake the preview to continue with the reupload. | Your profile has been modified—retake the preview to continue with the reupload. |
| `RESHARE_TO_WORKSHOP` | 重新上传到 Workshop 工作室 | Reupload to Workshop |
| `RIGHT_HANDED` | 右手型 | Right-handed |
| `STANDARD` | 标准 | Standard |
| `STANDARD_MACRO` | 标准宏 | Standard Macro |
| `STANDARD_MACRO_CONTENT` | 命令将根据分配的键或按钮的基本交互来执行。. | Commands are performed based on basic interactions with the assigned key or button. |
| `SYNCING_PROFILES` | 同步配置文件 | Syncing Profiles |
| `TEXT_KEYMAP` | Keymap {{number}} | Keymap {{number}} |

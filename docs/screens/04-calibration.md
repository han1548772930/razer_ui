# 校准（TAB_CALIBRATION）

> 本页由雷云自己的产品模块提取；本文件只记录原版事实，不记录本项目实现状态。
> 证据工具：`node .ref/tools/gen-screen-docs.js`

## 1. 出现在哪些设备上

| 设备 | productId | 设备类型 | 该设备标签页顺序 |
|---|---|---|---|
| Razer DeathAdder V3 Pro | 182 | 鼠标 | 第 4 个：自定义 · 性能 · 正在配对 · 校准 · 电源 · 滚动 |
| RAZER KRAKEN BT SANRIO LIMITED EDITION | 777 | 耳机 | 第 3 个：自定义 · 灯光 · 校准 · 电源 · 声音 · 麦克风 |

标签页真实文案：**校准**（key `TAB_CALIBRATION`）

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

该界面共 19 个布局类名，分 2 个分区。

**① 校准说明**（6）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `caliSteps` | `margin:10px` | — |
| `calibration-body` | `margin:auto; max-width:600px` | — |
| `calibration-welcome` | `padding:20px 30px; position:relative`<br>`height:36px; position:absolute; right:0; top:0; width:36px` | `background-color:#2d2d2d; border:1px solid #2d2d2d; border-radius:5px`<br>`background-image:url(../../static/media/icon_close_white.8ab462b8.svg); background-position:50%; background-repeat:no-repeat; background-size:20px; transition:background-color .2s`<br>`background-image:url(../../static/media/icon_close_green.45f61360.svg)`<br>`opacity:.7` |
| `calibration_tool_tip` | `display:flex`<br>`position:relative; top:1px`<br>`left:5px; position:relative`<br>`left:0; right:auto; top:25px` | — |
| `custom-calibration` | `width:-webkit-fit-content; width:fit-content` | `background-color:#44d62c` |
| `popup-calibration` | `max-height:calc(100vh - 105px); top:105px!important`<br>`【@media screen and (max-width:1398px)and (min-width:800px)】width:800px`<br>`【@media screen and (max-width:799px)】min-width:546px` | — |

**② 鼠标垫 / 表面列表**（13）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `Duallink_current-mousemat-category__OITEx` | `display:inline-block; height:60px; width:60px` | `background-position:50%; background-repeat:no-repeat; background-size:100% 100%`<br>`background-image:url(../../static/media/icon_mousemat_ensure.f46e119e.svg)` |
| `add-mat` | `display:block; height:auto; max-height:32px; min-height:16px; width:100%`<br>`flex:0 0 auto; flex-wrap:wrap`<br>`height:172px; margin:5px; padding:8px 20px 0; position:relative; width:240px`<br>`height:112px; margin:auto; width:196px` | `color:#ccc; font-size:14px; text-transform:uppercase`<br>`line-height:16px; text-align:center; transition:color .2s`<br>`color:#707070; font-size:12px`<br>`background-color:#111; border:2px solid #0000; border-radius:5px; transition:border-color .2s`<br>`border-color:#44d62c` |
| `add-mats` | `flex-wrap:wrap`<br>`flex:0 0 auto; flex-wrap:wrap` | — |
| `choose-a-mat` | `top:100px`<br>`max-height:100vh!important`<br>`overflow-y:visible!important`<br>`padding:8px 10px; width:-webkit-max-content; width:max-content` | `background-color:#000; border:1px solid #5d5d5d; font-size:14px`<br>`background-color:#222; border-radius:5px 5px 0 0; transition:top .3s`<br>`background-image:url(../../static/media/icon_close.55fe41f1.svg)`<br>`background-color:#0000; background-position:50%; background-repeat:no-repeat; background-size:20px; transition:background-color .2s`<br>`background-image:url(../../static/media/icon_back_arrow.b39e4841.svg)` |
| `chroma-studio-animate` | `height:26px; position:relative; width:26px`<br>`display:inline-block; height:26px; left:0; position:absolute; top:0; width:26px` | `background-image:url(../../static/media/chroma_sync_v3_static.c8ddf315.svg)`<br>`border-color:#44d62c`<br>`background-color:#ffffff1a`<br>`background-color:#111; border-color:#737373; cursor:default` |
| `contect-animated-dot` | `align-items:center; display:flex; height:30px; justify-content:center; margin:10px auto 13px`<br>`width:520px`<br>`height:100%; width:80px`<br>`left:88px; position:relative` | `background-image:url(../../static/media/icon_marching_ants_master_line.294a8460.svg); background-position:50%; background-repeat:no-repeat`<br>`background-image:url(../../static/media/icon_marching_ants_connected_right_to_left.325e3d12.svg)`<br>`background-image:url(../../static/media/icon_marching_ants_connecting_right_to_left.e9be0b6f.svg)`<br>`opacity:0`<br>`background-image:url(../../static/media/icon_marching_ants_connected_left_to_right.d11787b8.svg)` |
| `exclamation` | `display:inline-block; left:40px; top:30px`<br>`height:14px; position:absolute; width:14px` | `background-color:#5d5d5d; background-image:url(../../static/media/tooltip_exclamationmark.cc8fb226.svg)`<br>`background-repeat:no-repeat; border-radius:7px` |
| `icon-detection--animation` | — | — |
| `master-mousemat` | `width:520px`<br>`right:-3px`<br>`left:-3px` | `background-position:100%` |
| `master-mousemat-dot` | `height:100%; width:80px`<br>`align-self:baseline; height:58%; width:4px` | `background-image:url(../../static/media/icon_marching_ants_master_line.294a8460.svg); background-position:50%; background-repeat:no-repeat` |
| `mats` | `align-self:flex-start; flex-wrap:wrap`<br>`flex:0 0 auto` | — |
| `port-infomation` | `flex:1 1` | — |
| `surface` | `height:200px; margin:0 20px 20px 0; padding:8px 20px; position:relative; top:0; width:290px`<br>`top:-4px`<br>`top:0`<br>`height:8px` | `background-color:#111; border-radius:5px; transition:top .2s`<br>`background-color:#222; border:2px dashed #5d5d5d; transition:border-color .2s`<br>`border-color:#44d62c`<br>`background-color:#44d62c`<br>`color:#ccc; font-size:14px; text-transform:uppercase` |

#### RAZER KRAKEN BT SANRIO LIMITED EDITION（productId 777）

该界面共 6 个布局类名，分 2 个分区。

**① 校准说明**（1）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `calibration-body` | `margin:auto; max-width:600px` | — |

**② 鼠标垫 / 表面列表**（5）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `choose-a-mat` | `top:100px`<br>`max-height:100vh!important`<br>`overflow-y:visible!important`<br>`padding:8px 10px; width:-webkit-max-content; width:max-content` | `background-color:#000; border:1px solid #5d5d5d; font-size:14px` |
| `chroma-studio-animate` | `height:26px; position:relative; width:26px`<br>`display:inline-block; height:26px; left:0; position:absolute; top:0; width:26px` | `background-image:url(../../static/media/chroma_sync_v3_static.c8ddf315.svg)`<br>`border-color:#44d62c`<br>`background-color:#ffffff1a`<br>`background-color:#111; border-color:#737373; cursor:default` |
| `exclamation` | `display:inline-block; left:40px; top:30px`<br>`height:14px; position:absolute; width:14px` | `background-color:#5d5d5d; background-image:url(../../static/media/tooltip_exclamationmark.cc8fb226.svg)`<br>`background-repeat:no-repeat; border-radius:7px` |
| `icon-detection--animation` | — | — |
| `port-infomation` | `flex:1 1` | — |

## 3. 功能项

该界面对应的雷云文案 key，共 **74** 条（中文为雷云原文）：

| key | 中文 | English |
|---|---|---|
| `ADD_MAT` | 添加 | ADD |
| `CALIBBRTION_LEFT_THUMBSITCK` | 左拇指控制杆 | LEFT THUMBSTICK |
| `CALIBBRTION_RIGHT_THUMBSITCK` | 右拇指控制杆 | RIGHT THUMBSTICK |
| `CALIBRATE` | 校准 | CALIBRATE |
| `CALIBRATED` | 已校准:  | Calibrated:  |
| `CALIBRATE_ANOTHER_KEY` | 校准另一按键 | CALIBRATE ANOTHER KEY |
| `CALIBRATE_CALCULATING_TOLERANCE` | Calculating Tolerance | Calculating Tolerance |
| `CALIBRATE_END` | 再次单击鼠标左键结束校准。 | Click the left mouse button again to end calibration. |
| `CALIBRATE_MAX_HEIGHT` | 正在校准最大高度... | Calibrating maximum height… |
| `CALIBRATE_MSG` | 你的鼠标传感器需要进行微调，才能有效使用此预先校准的 Razer 雷蛇表面配置文件。 | Your mouse sensor requires fine-tuning in order to effectively use this pre-calibrated Razer surface profile. |
| `CALIBRATE_MSG1` | 鼠标的传感器需要进行微调，才能有效使用此 | Your sensor requires fine-tuning in order to effectively use this |
| `CALIBRATE_MSG2` | 预先校准的 Razer 雷蛇鼠标垫数据。 | pre-calibrated Razer mouse mat data. |
| `CALIBRATE_PHILIPS_SENSOR` | Without lifting the mouse, slowly move it along the mouse mat for at least 3 seconds | Without lifting the mouse, slowly move it along the mouse mat for at least 3 seconds |
| `CALIBRATE_STEP1` | 单击鼠标左键，并移动鼠标。 | Click the left mouse button and move the mouse. |
| `CALIBRATE_STEP2` | 以 Z 字形方式移动鼠标，并覆盖整个鼠标垫表面。 | Move the mouse in a zigzag pattern and cover the entire mouse mat surface. |
| `CALIBRATE_WARNING_TOO_FAST` | 鼠标移动过快！ | Moving your mouse too fast! |
| `CALIBRATING` | 校准中... | Calibrating... |
| `CALIBRATING_KEY` | 校准按键： | Calibrating Key: |
| `CALIBRATING_MAX_DEPTH` | 正在校准最大深度... | Calibrating maximum depth... |
| `CALIBRATING_STEP_1` | 按下想要校准的按键，然后选择“下一步”。 | Press the key you want to calibrate, then select NEXT |
| `CALIBRATING_STEP_2` | 将选定按键完全按到底，然后选择“下一步”。<br><br>在提示之前，请勿松开按键。 | Press and hold the selected key fully to the bottom and then select NEXT.<br><br>Do not release the key unless instructed. |
| `CALIBRATING_STEP_3` | 完全松开选定按键，然后选择“下一步”。<br><br> 请勿按下按键。 | Release the selected key fully then select NEXT.<br><br> Do not press the key. |
| `CALIBRATIONDETAILS3` | 独立追踪抬升和着陆距离 | Independently track cut-off and landing distances |
| `CALIBRATION_CENTER_TIP` | 你位于各虚拟扬声器之间的中心位置。 | You're at the center of the virtual speakers. |
| `CALIBRATION_COMPLETED` | 校准完成。 | Calibration Completed. |
| `CALIBRATION_DESC_LEFT` | 让我们来校准左拇指控制杆！ | LET’S CALIBRATE THE LEFT THUMBSTICK! |
| `CALIBRATION_DESC_RIGHT` | 让我们来校准右拇指控制杆！ | LET’S CALIBRATE THE RIGHT THUMBSTICK! |
| `CALIBRATION_ERROR` | 校准错误 | CALIBRATION ERROR |
| `CALIBRATION_ERROR_DESC` | 校准期间发生错误。要获得准确的结果，请重新开始校准流程。 | An error occurred during calibration. To get accurate results, please restart the calibration process. |
| `CALIBRATION_ERROR_MSG` | 在精调鼠标传感器时发生错误，校准过程可能产生了不一致。 | An error occurred while fine-tuning your mouse sensor, inconsistencies might have been made during the calibration. |
| `CALIBRATION_FAILED` | 校准失败。请重新开始校准。 | Calibration failed. Please restart calibration. |
| `CALIBRATION_HIGH` | 高 | High |
| `CALIBRATION_INFO` | 键盘在完成或取消校准之前将无法正常工作。<br>继续进行校准或退出以恢复功能。 | The keyboard will not function normally until calibration is completed or canceled.<br>Proceed with calibration or exit to restore functionality. |
| `CALIBRATION_INFORMATION` | 校准信息 | CALIBRATION INFORMATION |
| `CALIBRATION_INFO_NOTIFICATION` | 你的键盘从一开始就提供精准的击键体验。使用轴体校准工具，在需要时恢复利落灵敏的操作体验。 | Your keyboard delivers precise keystrokes from the start. Use the Switch Calibration Tool to bring back that crisp, responsive feel when needed. |
| `CALIBRATION_LOW` | 低 | Low |
| `CALIBRATION_PS_STEP1_0` | 将拇指控制杆一直向右移动 | Move the thumbstick to the right |
| `CALIBRATION_PS_STEP2_0` | 将拇指控制杆一直向下移动 | Move the thumbstick all the way down |
| `CALIBRATION_PS_STEP3_0` | 将拇指控制杆一直向左移动 | Move the thumbstick to the left |
| `CALIBRATION_PS_STEP4_0` | 将拇指控制杆一直向上移动 | Move the thumbstick all the way up |
| `CALIBRATION_PS_STEP5_0` | 最后，顺时针转动拇指控制杆 3 次 | Finally, rotate the thumbstick clockwise 3 times |
| `CALIBRATION_PS_STEP_1` | 然后慢慢回到中心位置 | Then slowly return it to the center |
| `CALIBRATION_PS_STEP_2` | 按 {{leftBumper}} 键进入下一步 | Press the {{leftBumper}} button to process to the next step |
| `CALIBRATION_PS_STEP_3` | 按 {{leftBumper}} 键保存数据 | Press the {{leftBumper}} button to save the data |
| `CALIBRATION_RECALIBRATE` | 重新校准 | RECALIBRATE |
| `CALIBRATION_REPEAT` | 请按照指示重新执行鼠标垫表面校准设置。 | Please repeat the Mouse Mat Surface Calibration setup as specified. |
| `CALIBRATION_STEP0` | 你想重新校准哪一个拇指控制杆？ | Would you like to recalibrate your thumbsticks? |
| `CALIBRATION_STEP1` | 将拇指控制杆一直向右移动，然后慢慢回到中心位置。按左肩键保存数据。 | Move the thumbstick all the way to the right, then slowly return it to the center. Press the Left Bumper to save the data. |
| `CALIBRATION_STEP2` | 将拇指控制杆一直向下移动，然后慢慢回到中心位置。按左肩键保存数据。 | Move the thumbstick all the way down, then slowly return it to the center. Press the Left Bumper to save the data. |
| `CALIBRATION_STEP3` | 将拇指控制杆一直向左移动，然后慢慢回到中心位置。按左肩键保存数据。 | Move the thumbstick all the way to the left, then slowly return it to the center. Press the Left Bumper to save the data. |
| `CALIBRATION_STEP4` | 将拇指控制杆一直向上移动，然后慢慢回到中心位置。按左肩键保存数据。 | Move the thumbstick all the way up, then slowly return it to the center. Press the Left Bumper to save the data. |
| `CALIBRATION_STEP5` | 最后，顺时针转动{{joystick}}拇指控制杆 3 次。按左肩键保存数据。 | Finally, rotate the {{joystick}} thumbstick clockwise 3 times. Press the Left Bumper to save the data. |
| `CALIBRATION_SUCCESS` | 校准成功 | CALIBRATION SUCCESSFUL |
| `CALIBRATION_SUCCESSFUL` | 校准成功！ | Calibration is successful! |
| `CALIBRATION_SUCCESS_DESC_LEFT` | 你已成功校准左拇指控制杆！ | You have successfully calibrated your left thumbstick! |
| `CALIBRATION_SUCCESS_DESC_RIGHT` | 你已成功校准右拇指控制杆！ | You have successfully calibrated your right thumbstick! |
| `CALIBRATION_TITLE` | 在此测试拇指控制杆的性能 | TEST THE PERFORMANCE OF YOUR THUMBSTICK |
| `CALIBRATION_V2_STEP5` | 将拇指控制杆移至边缘，然后顺时针转动 3 次。 | Move the thumbstick to the edge and then rotate it clockwise 3 times. |
| `CREATE_OWN_SURFACE_PROFILE` | 创建个性化的表面配置文件。 | Create your own surface profile. |
| `MATCH_FRAME` | Match Frame | Match Frame |
| `MATS` | 鼠标垫 | Mats |
| `SENSITIVITY_MATCHER` | 灵敏度匹配 | Sensitivity Matcher |
| `SENSITIVITY_MATCHER_ACTION_DESC_1` | 并排握好两个鼠标，然后点击 Razer Synapse 雷云上的任意位置。 | Hold your mice side by side then click anywhere on Razer Synapse. |
| `SENSITIVITY_MATCHER_ACTION_DESC_2` | 缓慢地将鼠标向一个方向移动，直至校准完成。 | Slowly move your mice in one direction until the calibration is complete. |
| `SENSITIVITY_MATCHER_ACTION_DESC_3` | 灵敏度匹配成功。 | Sensitivity matching successful. |
| `SENSITIVITY_MATCHER_CALIBRATION_ERROR` | 校准错误 | CALIBRATION ERROR |
| `SENSITIVITY_MATCHER_CALIBRATION_ERROR_DESC` | 请避免拔出第二个设备。重新开始校准过程，以确保结果准确。 | Please avoid unplugging your second device. Restart the calibration process to ensure accurate results. |
| `SENSITIVITY_MATCHER_DESC` | 创建新的灵敏度匹配校准。 | Create a new sensitivity matcher calibration from a different mouse. |
| `SENSITIVITY_MATCHER_PROFILE_DESC` | 将鼠标的灵敏度与另一个鼠标相匹配 | Match the sensitivity of your mouse to a previously calibrated mouse. |
| `SENSITIVITY_MATCHER_STEP_DESC` | 校准鼠标以模仿另一个鼠标的感觉和移动。 | Calibrate your mouse to mimic the feel and movement of another mouse. |
| `SENSITIVITY_MATCHER_TOOLTIP` | 微调鼠标以模仿另一个鼠标的感觉。需要有第二个鼠标才能使用此功能。 | Fine-tune your mouse to mimic the feel of another mouse. A second mouse is required to use this feature. |
| `SMART_TRACKING_DESC2` | Set a single cut off point to your preferred distance regardless of the surface it is placed on. | Set a single cut off point to your preferred distance regardless of the surface it is placed on. |
| `SMART_TRACKING_DISC` | 无论使用何种表面，都可以根据你的喜好设置单个中止点。 | Set a single cut off point to your preferred distance regardless of the surface it is placed on. |
| `SURFACEDISTANCE` | 表面距离 | SURFACE DISTANCE |

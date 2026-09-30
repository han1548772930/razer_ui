# 麦克风（`TAB_MIC`）

> 本文只记录 `.ref/devices/777` 原始前端能够证明的界面事实。通用 locale 中存在的能力不等于 RAZER KRAKEN BT SANRIO LIMITED EDITION 的麦克风页实际渲染能力。

## 1. 页面归属与入口

- 设备：`RAZER KRAKEN BT SANRIO LIMITED EDITION`。
- `productId`：`777`。
- 设备类别：耳机。
- 标签页 key：`TAB_MIC`；标签显示由产品 locale 提供。
- 页面主体与声音页共用 `body-widgets`，但麦克风页实际 EQ widget 使用 `widgetId:"eqBox"`，并且 CSS 强制为 `940px × 至少 473px`。

证据：

- `.ref/devices/777/manifest.json`
- `.ref/devices/777/static/js/main.eb70ce38.js` 的设备模块 `7816`、`TAB_MIC` 常量与 `DeviceInfo.productId:777`
- `.ref/frontend/locales/zh-CN.json` 的 `TAB_MIC`

## 2. 真实组件树

原始 bundle 对麦克风页 EQ 的组件链是：

```text
KM  麦克风页面根组件
└─ Bm / body-widgets
   └─ YM                 连接 micEq store 的 EQ 编辑器
      └─ gM              通用 EQ 编辑器
```

源码中的连接关系：

- `YM` 读取 `micEq.activePresetMode`。
- `YM` 读取 `micEq.frequencyBands`。
- `YM` 读取 `micEq.customBands`。
- `YM` 读取 `micEq.deviceEqDifferent`。
- `YM` 的动作是 `setEQ` 与 `setSaveDeviceEQ` 的麦克风版本。
- `KM` 将 `YM` 包在 `Bm / body-widgets` 中，并把 `widgetId:"eqBox"` 传给通用 EQ 编辑器。

这证明了 777 麦克风页的核心是麦克风 EQ 编辑器，不证明它一定还有音量、侧音、降噪、噪声门或语音增强卡片。

## 3. 外层布局与尺寸

### 3.1 页面外壳

| 选择器/组件 | 真实布局与尺寸 | 颜色/样式 |
|---|---|---|
| `body, html` | `height:100%`、`width:100%`、`min-height:720px`、`max-width:1920px`、`overflow:hidden` | `background-color:#222`、`color:#ccc`、`font-family:Roboto,sans-serif`、`font-size:16px` |
| `.main-container` | 绝对定位、宽高 100%、纵向 flex、`min-width:600px` | `background-color:#222` |
| `.nav-tabs` | `display:flex`、`min-height:48px`、宽 100% | 背景 `#222`，底边 `2px solid #000`，默认文字 `#5d5d5d` |
| `.body-wrapper` | `flex:1`、宽 100%、`min-width:600px`、内边距 `10px 20px 20px` | 作为内容区域的内边距来源 |
| `.body-widgets` | 横向 flex、允许换行、居中、`max-width:1240px` | 不额外生成一列全宽控制面板 |
| `#eqBox` | `min-width:940px`、`max-width:940px`、`width:940px`、`min-height:473px` | 页面主 widget，不能压缩为 600px |

### 3.2 EQ widget 颜色与控件样式

- 页面底色：`#222`。
- EQ/widget 背景：通用 widget 使用 `#111`；图表及辅助区域沿用 bundle 中的 `#222`、`#111`、`#5d5d5d`、`#ccc`。
- 激活色：`#44d62c`，用于 slider thumb、激活边框、hover 和可操作强调。
- slider 轨道：源码使用深灰轨道；可编辑 thumb 为绿色圆点，hover 为 `#707070`，active 为 `#383838`。
- dB 提示气泡：绿色 `#44d62c` 背景、黑色文字、`3px` 圆角、字号 `12px`、高度 `20px`、宽度 `26px`。
- read-only slider：轨道与填充使用 `#494949` / `#ccc`，提示气泡为灰色；不可编辑时隐藏 reset。
- EQ widget 的垂直 slider 使用 `vertical-slider__container--wide`；声音页使用 narrow，麦克风页不能复用 narrow 尺寸。

证据：`.ref/devices/777/static/css/main.e4bab2aa.css` 的 `#eqBox`、`.vertical-slider__*`、`.sliderChart__*` 与通用设备页样式。

## 4. 麦克风 EQ 功能

### 4.1 预设

`Mic_EQ_Tabs` 在设备模块 `7816` 中明确包含以下 id：

| id | 语义来源 |
|---|---|
| `default` | 默认麦克风 EQ |
| `boost` | 麦克风增强预设 |
| `broadcast` | 广播预设 |
| `conference` | 会议预设 |
| `custom` | 自定义 EQ |

显示名称由 locale key 映射，不能在文档中硬编码成另一套中文名称。

### 4.2 频段滑杆

`KM → YM → gM` 为麦克风 EQ 的实际编辑路径。源码明确参数如下：

| 参数 | 麦克风页真实值 |
|---|---|
| 滑杆类型 | `wide` |
| Y 轴对齐 | `flex-end` |
| Y 轴右边距 | `40px` |
| `tabScale` | `true` |
| 最小值 | `-5` |
| 最大值 | `5` |
| 步长 | `1` |
| Y 轴标签 | `-5dB`、`0dB`、`+5dB` |
| widget id | `eqBox` |

通用 `gM`：

- 遍历 store 提供的 `frequencyBands`，按每个频率的 `decibel` 值绘制 slider。
- 频率标签按值转换为 `Hz` 或 `kHz`。
- slider 改变后通过 `setEQ` 更新麦克风 EQ 状态。
- reset 由 EQ 编辑器提供；只读状态隐藏 reset。
- `customBands` 仅在 custom 模式下作为自定义频段数据保存/回填。

### 4.3 设备保存与不同步状态

- `deviceEqDifferent` 是源码明确提供的“设备 EQ 与当前 UI 状态不同”状态。
- `setSaveDeviceEQ` 是源码明确提供的保存到设备动作。
- 文档只记录这两个状态/动作存在，不虚构保存成功 toast、进度动画、断开重试或硬件同步提示；这些必须由实际 render 分支或状态机证据支持后才能加入。

## 5. 明确删除的未经证明内容

以下内容已从本页规格中删除，因为 777 的 `KM → YM → gM` render 没有证明这些控件属于麦克风页：

- 麦克风音量滑杆或静音开关。
- 侧音开关、侧音音量、Mic Monitoring。
- 降噪、噪声门、Voice Gate、AI Noise Suppression。
- 语音清晰度、语音增强、变声器。
- 游戏/聊天混音、输入/输出路由选择。
- 采样率、监听延迟、麦克风增益等额外硬件参数。
- 固定“10 段 EQ”或未经 `frequencyBands` 数组证明的频段数量。
- 将 `VOICE_*`、`SIDETONE_*`、`NOISE_*` locale key 的存在直接当作页面控件。

这些 key 可能属于其他产品、其他模块、共享组件或未启用功能；它们不能覆盖 777 实际组件树的证据。

## 6. 原始证据索引

- 组件树、预设、store 字段与动作：`.ref/devices/777/static/js/main.eb70ce38.js`
  - `7816`：`Mic_EQ_Tabs`、`micBandPreset`、`DeviceInfo`
  - `YM`：麦克风 EQ store connect
  - `KM`：麦克风页面根组件与 `widgetId:"eqBox"`
  - `gM`：通用 EQ 编辑器、频段滑杆、reset 与 read-only 分支
- 样式：`.ref/devices/777/static/css/main.e4bab2aa.css`
  - `#eqBox`
  - `.vertical-slider__container--wide`
  - `.vertical-slider__input`、`.vertical-slider__bubble`、`.vertical-slider__title`
  - `.sliderChart__container`、`.sliderChart__yAxisTitle`、`.sliderChart__reset-button`
- 设备信息：`.ref/devices/777/manifest.json`
- locale：`.ref/frontend/locales/zh-CN.json`、`.ref/frontend/locales/en.json`

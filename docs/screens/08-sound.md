# 声音（`TAB_SOUND`）

> 本文只记录 `.ref/devices/777` 原始前端能够证明的界面事实。不要把通用 locale、其他产品的音频能力或当前 Rust 实现推断成 777 本页功能。

## 1. 页面归属与入口

- 设备：`RAZER KRAKEN BT SANRIO LIMITED EDITION`。
- `productId`：`777`。
- 设备类别：耳机；manifest 的 `deviceName` 与 JS 模块 `DeviceInfo` 一致。
- 标签页 key：`TAB_SOUND`；标签显示由产品 locale 提供。
- 设备模块把页面导出为声音页，页面主体使用通用 `body-widgets` 容器。

证据：

- `.ref/devices/777/manifest.json`
- `.ref/devices/777/static/js/main.eb70ce38.js` 的设备模块 `7816`、`TAB_SOUND` 常量与 `DeviceInfo.productId:777`
- `.ref/frontend/locales/zh-CN.json` 的 `TAB_SOUND`

## 2. 真实组件树

原始 bundle 中声音 EQ 页面对应以下组件链：

```text
VM  声音页面根组件
├─ Bm / body-widgets
├─ eM                    页面左侧的声音相关模块
├─ ym(direction="left")
│  ├─ NM                 播放音量模块
│  └─ rM                 声音属性/输出属性模块
└─ ym(direction="right")
   └─ FM                 连接 audioEq store 的 EQ 编辑器
      └─ gM              通用 EQ 编辑器
```

关键点：

- `VM` 的 `render()` 明确把 `eM`、左列 `NM + rM`、右列 `FM` 作为三个区域渲染。
- `FM` 从 `audioEq` 读取 `activePresetMode`、`frequencyBands`、`customBands`、`deviceEqDifferent`，并连接 `setEQ` 与 `setSaveDeviceEQ`。
- `FM` 使用 `Audio_EQ_Tabs` 与 `audioBandPreset`，不是通用“音量卡片”或自定义的 10 段均衡器页面。

## 3. 外层布局与尺寸

### 3.1 页面外壳

声音页沿用 777 bundle 的通用设备页外壳：

| 选择器/组件 | 真实布局与尺寸 | 颜色/样式 |
|---|---|---|
| `body, html` | `height:100%`、`width:100%`、`min-height:720px`、`max-width:1920px`、`overflow:hidden` | `background-color:#222`、`color:#ccc`、`font-family:Roboto,sans-serif`、`font-size:16px` |
| `.main-container` | 绝对定位、宽高 100%、纵向 flex、`min-width:600px` | `background-color:#222` |
| `.nav-tabs` | `display:flex`、`min-height:48px`、宽 100% | 背景 `#222`，底边 `2px solid #000`，默认文字 `#5d5d5d` |
| `.body-wrapper` | `flex:1`、宽 100%、`min-width:600px`、内边距 `10px 20px 20px` | 作为滚动内容区域的内边距来源 |
| `.body-widgets` | 横向 flex、允许换行、居中、`max-width:1240px` | 不额外增加全宽卡片 |
| `.body-widgets .widget` | `min-width:600px`、`max-width:600px`、`padding:30px 40px`、`margin:10px auto`、字号 `14px` | `background-color:#111`、圆角 `5px` |
| `.widget-col` | 纵向 flex，宽 `600px` | 作为左右列的内部堆叠容器 |
| `#eqBox` | `min-width:940px`、`max-width:940px`、`width:940px`、`min-height:473px` | 不得压缩为 600px 普通卡片 |

因此 777 声音页的几何关系是：左侧 `600px` 列堆叠声音模块，右侧 EQ 区域固定 `940px`；页面需要在通用 `body-wrapper` 内横向排列并允许纵向滚动。不能把它实现为单列全宽卡片，也不能把右侧 EQ 缩成普通 600px widget。

### 3.2 控件公共样式

- 主色：`#44d62c`，用于激活边框、滑块、标题强调与交互 hover。
- 主文字：`#ccc`；次要文字：`#999` 或 `#707070`。
- 卡片/输入区域底色：`#111`；页面底色：`#222`。
- 常规边框：`#5d5d5d`；下拉框边框：`#515151`。
- 次要按钮：背景 `#707070`、文字 `#fff`；主按钮文字为 `#000`。
- 控件通常使用 `3px` 圆角，widget 使用 `5px` 圆角。

证据：`.ref/devices/777/static/css/main.e4bab2aa.css` 的全局设备布局、widget、按钮、slider 与 EQ 规则。

## 4. 功能与交互

### 4.1 播放音量模块（`NM`）

源码中的 `NM` 连接：

- `playbackReducer.isEnabled`
- `playbackReducer.value`
- `setVolume`
- `setMute`

渲染事实：

1. 以带开关的通用设置区块显示播放音量。
2. 开关由 `isEnabled` 控制；切换时调用 `setMute`。
3. 音量滑杆范围 `0..100`，步长 `1`，显示最小值 `0`、最大值 `100`。
4. 滑杆只有在启用状态下处于 active 状态。
5. 区块底部有“声音属性”外部入口，点击调用 Electron 的声音属性动作；这不是本页内部的额外混音器。
6. 组件在 props 改变时同步本地音量和开关状态，不能只做静态展示。

源码证据：`main.eb70ce38.js` 中 `dM` 的 `toggleSwitch`、`changeValue`、`soundPropertiesClicked` 与 `NM` 的 Redux connect。

### 4.2 声音属性模块（`rM`）

- `rM` 连接 `deviceReducer.thxDevice`。
- 内部组件根据 THX 设备是否存在渲染声音属性内容；源码明确有 `text-thx-spatial` 与标准应用文案分支。
- 有窗口/系统声音属性入口，使用 `text-sound-properties` 样式。
- 不应凭 locale 中存在的 key 加入“游戏/聊天混音”“THX 模式切换”“空间音效校准”等控件；777 的这段 render 只证明输出属性入口和 THX 设备分支。

源码证据：`main.eb70ce38.js` 中 `sM` 的条件 render 与 `rM` 的 `deviceReducer.thxDevice` connect。

### 4.3 音频 EQ（`FM → gM`）

`FM` 的明确输入与行为：

- 预设标签集合：`Audio_EQ_Tabs`。
- 预设数据：`audioBandPreset`。
- 当前模式：`audioEq.activePresetMode`。
- 当前频段：`audioEq.frequencyBands`。
- 自定义频段：`audioEq.customBands`。
- 设备 EQ 差异状态：`audioEq.deviceEqDifferent`。
- 修改：`setEQ`；保存到设备：`setSaveDeviceEQ`。

EQ 图表参数由源码明确为：

| 参数 | 声音页真实值 |
|---|---|
| 滑杆类型 | `narrow` |
| Y 轴对齐 | `flex-end` |
| Y 轴右边距 | `40px` |
| `tabScale` | `false` |
| 最小值 | `-5` |
| 最大值 | `5` |
| 步长 | `1` |
| Y 轴标签 | `-5dB`、`0dB`、`+5dB` |

`gM` 的图表逐项遍历 `frequencyBands`，每项使用频率与 dB 值渲染垂直 slider；频率标签按源码转换为 `Hz` 或 `kHz`。EQ 图表还有 reset 入口，read-only 状态下隐藏 reset 并使用灰色只读滑杆样式。

### 4.4 预设与修改状态

- 声音预设集合在 `Audio_EQ_Tabs` 中明确包含：`default`、`amplified`、`vocals`、`bassboost`、`enhancedclarity`、`custom`。
- 预设切换更新 `activePresetMode` 与频段数据。
- 选择 `custom` 时，源码会保存/更新 `customBands`。
- 设备 EQ 与当前 UI 不一致时，状态由 `deviceEqDifferent` 提供；文档只能记录该状态存在，不能凭空补充未在本页 render 中出现的提示文案或按钮。
- reset 只在可编辑 EQ 图表中显示；不能在只读状态显示可用 reset。

## 5. 不应加入的内容

以下内容在 777 的声音页 render/CSS/manifest 中没有被本页组件树证明，已从本规格删除：

- 固定 10 段 EQ、10 个具体频率或其他未经 `frequencyBands` 证明的频段数量。
- “游戏/聊天平衡”滑杆或独立混音卡片。
- 麦克风音量、侧音、降噪、噪声门、变声器、采样率等麦克风功能。
- 仅由全局 locale key 或其他设备 bundle 出现就推断出的 THX、空间音频高级控件。
- 假的保存成功、硬件连接成功、加载完成提示。

## 6. 原始证据索引

- 组件树与交互：`.ref/devices/777/static/js/main.eb70ce38.js`
  - `7816`：`Audio_EQ_Tabs`、`Mic_EQ_Tabs`、`audioBandPreset`、`micBandPreset`、`DeviceInfo`
  - `dM` / `NM`：播放音量
  - `sM` / `rM`：声音属性与 THX 设备分支
  - `VM` / `FM` / `gM`：声音页与音频 EQ
- 样式：`.ref/devices/777/static/css/main.e4bab2aa.css`
  - `.body-wrapper`、`.body-widgets`、`.widget`、`.widget-col`
  - `#eqBox`
  - `.vertical-slider__*`、`.sliderChart__*`
- 设备信息：`.ref/devices/777/manifest.json`
- locale：`.ref/frontend/locales/zh-CN.json`、`.ref/frontend/locales/en.json`

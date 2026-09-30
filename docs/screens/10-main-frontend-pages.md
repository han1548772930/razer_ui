# 主前端扩展页面

> 这些页面的命名、文案和部分布局来自主前端 `.ref/frontend/`。具体产品页仍必须以产品模块自己的 `main.js/main.css` 为最终依据；主前端存在某个 `TAB_*` 并不代表所有产品都显示它。

## 1. 首页 / Dashboard（`TAB_HOME`）

### 布局

```text
.main-container
├── .nav-tabs
└── #body-wrapper
    └── .dashboard
        ├── .box-group: 设备分组
        │   ├── .backdrop-box
        │   ├── 标题、折叠、拖拽把手
        │   └── .box-inner
        │       └── DeviceCard（约 290×220px）
        └── 空状态 / 扫描状态 / 添加设备入口
```

Dashboard 不是产品页的 `600px widget` 两列布局；它使用设备卡片网格和可重排分组。`.box-group` 支持展开/折叠，设备卡片显示产品图、设备名、分类图标、连接状态、电池、固件/额外信息和进入设备页的操作。

### 功能

- 发现、扫描、添加、移除和解绑设备。
- 设备分组的折叠、展开、拖拽排序和空分组。
- 在线、离线、扫描中、绑定失败、配对中、无设备等状态。
- 打开产品 UI、进入设备设置、更新固件和恢复控制权。
- 已关联游戏/应用入口，以及 profile 自动应用提示。
- 其他 Razer 应用入口：Chroma Studio、Chroma Connect、Chroma Visualizer、宏等模块。

### 样式

- 页面背景 `#222`，设备卡片/分段表面使用 `#111` 或主前端 CSS module 的深色表面。
- 设备卡片图片位于独立图片区，状态标签和分类图标不能覆盖产品主体。
- 分段标题支持折叠图标、拖拽手柄和 hover；卡片 hover 必须与选中/离线区分。
- 扫描和空状态使用明确标题、说明和主按钮，不显示空白网格。

## 2. 应用设置（`TAB_SETTING`）

### 布局

```text
.main-setting
├── .side-navigation       约 180px 固定宽度
└── .setting-content
    └── .widget / .config-row / .config-btns-wrapper
```

应用设置有自己的左侧导航，不能复用产品页的顶栏标签作为唯一导航。右侧内容使用 widget/row 组合并独立滚动。

### 功能

- 通用应用设置、语言、账户和登录状态。
- 设备和模块管理：安装、卸载或打开模块。
- profile 管理、profile 迁移、同步和下载/上传状态。
- 通知、启动行为、日志、更新和隐私说明。
- Synapse 3 profile、宏和 Chroma 效果迁移。

### 样式

- 设置页标题通常使用 `18px`，页面主标题可使用 `20px`。
- 左侧导航项使用深色 hover 表面和品牌绿色选中态。
- 设置 row 使用标签、说明、控件和操作区四段式排列；危险删除使用 `#c8323c`。
- 不支持的服务显示 disabled/说明，不显示可点击的假控件。

## 3. 电池摘要（`TAB_BATTERY`）

`TAB_BATTERY` 同时作为产品能力和顶栏状态的文案来源。产品页通常把电量放在顶栏右区，详细电源配置仍属于 `TAB_POWER`。

### 功能

- 百分比、电池图标、充电中、已充满、暂停充电和低电量。
- 低电量阈值、充电保护、节能模式和电池健康提示。
- 无线连接方式、dongle/蓝牙/有线状态。

### 状态样式

- 正常状态使用 muted/foreground。
- 低电量使用 warning 或 danger，不只显示一个变红的数字。
- 充电状态显示图标和文本，暂停充电必须说明原因。

## 4. 音频（`TAB_AUDIO`）

### 页面骨架

```text
audio page
├── 音频设备选择 widget
├── 音频模式/输出 widget
├── 主音量与静音 widget
└── 应用/设备切换说明或提示
```

### 功能

- 选择播放/通信设备。
- 主音量、静音、前后/左右平衡。
- 音频模式：原声、立体声、空间音频、THX 等设备支持模式。
- 音频设备切换、默认设备和快捷键动作。
- 音频到触觉和音频可视化入口。

### 样式

连续音量和混音使用 slider；模式使用 radio/dropdown 或 segmented control。说明文字包括当前模式对摄像头追踪、输出设备或空间音效的影响。

## 5. 游戏 / 性能（`TAB_GAMING`）

该页面在主前端作为词汇和扩展能力存在，具体产品可能把内容并入性能页或自定义页。

### 功能

- Gaming Mode 开关。
- Win 键、Alt+Tab、Alt+F4、快捷键锁定。
- polling rate、HyperPolling、低延迟模式。
- 设备 profile 与当前游戏/应用关联。
- 游戏启动时自动切换 profile。

### 布局和样式

与性能页共用 `body-widgets` + `600px widget`；开关放在标题/说明之后，危险或会改变键盘行为的选项必须有 tooltip。轮询率是枚举值，使用真实 dropdown，不使用步进器。

## 6. 灯光扩展：效果 / 颜色

主前端词汇包含 `TAB_EFFECTS`、`TAB_COLOR`，产品模块可能将它们合并为 `TAB_LIGHTING`。

### 功能

- 静态、呼吸、光谱循环、波浪、响应、星光、涟漪、火焰、音频计等效果。
- 颜色选择器、颜色预设、亮度、速度、区域和预览。
- 电池/接电两套亮度设置。
- Chroma Studio 高级效果、Chroma Connect 应用同步、Chroma Visualizer 音频可视化。

### 布局和状态

效果选择在卡片上方；颜色/速度按效果能力条件显示。高级效果可能作用于多台设备但不保存到设备 profile，必须在说明中明确这一点。灯光警告使用橙色 `#fd8611`，硬件不兼容时显示原因而不是空白预览。

## 7. 显示 / OLED（`TAB_DISPLAY` / `TAB_OLED`）

### 功能

- OLED 内容、文本、图像和动画。
- OLED 亮度、动画开关、空闲超时。
- 刷新率、显示性能模式和设备状态显示。
- 游戏启动器、Razer Gold/Silver 或系统信息显示（设备支持时）。

### 布局

产品图/显示预览占用一个独立 widget；亮度和超时使用 slider；内容/模式使用 dropdown；预览区不能与控制行挤在同一行导致窄窗口溢出。

## 8. 增强 / EQ / 混音（`TAB_ENHANCEMENT` / `TAB_EQ` / `TAB_MIXER`）

### Enhancement

- Bass Boost、Voice Clarity、Loudness、音效增强预设。
- 每项开关、强度或预设选择。
- Dolby/THX/空间音效入口及兼容性提示。

### EQ

- 标准 EQ 和电竞 EQ。
- 预设、新建、重命名、复制、删除和恢复默认。
- 多频段 gain slider。
- 说明哪些自定义设置保存在耳机，哪些只保存在本地。

### Mixer

- 系统音量、游戏/聊天平衡。
- 前置/后置或应用通道音量。
- 输入/输出设备选择。
- Stream Mixer 虚拟输入通道和输出通道。

### 样式

EQ 使用固定宽度的频段图表或 slider 组；混音使用通道列/行；通道不可用时保留名称并显示不可用原因。所有连续值使用原版 slider 外观：6px 轨道、16px 圆形手柄、绿色填充、hover 灰色手柄加绿色边框。

## 9. 触觉（`TAB_HAPTICS`）

### 功能

- 触觉总开关。
- Audio-to-Haptics：游戏、电影和音乐实时转换。
- 增益、强度、音频响应、Razer Resonance。
- 触觉预设、预览、关闭音频到触觉的确认对话框。

### 状态

音频设备不存在、Sensa/Resonance 被另一个模块控制或当前 profile 不支持时，显示具体依赖关系。关闭 Audio-to-Haptics 可能同时关闭 Resonance，必须使用明确确认文案。

## 10. 演示（`TAB_DEMO`）

演示页是教程/测试能力的容器，不是普通设备设置页。主前端 CSS/JS 中存在 dashboard tutorial、analog tutorial、dynamic sensitivity tutorial、monitoring dashboard 等组件。

- 教程使用 modal、步骤指示器、上一页/下一页、关闭和重新打开。
- 视频或动态图使用独立内容区，不在 widget 中强行缩放成普通文本。
- 测试操作显示实时结果、成功/失败和重试。
- 关闭教程后焦点返回打开按钮。

## 11. 扩展页的证据规则

| 情况 | 规则 |
|---|---|
| 产品模块存在页面 JS/CSS | 按产品模块重建完整布局 |
| 只有主前端 `TAB_*` 文案 | 只能确认名称和通用功能词，不能假造产品布局 |
| 主前端有组件和 CSS，但没有产品模块 | 记录组件结构和样式，等待目标产品模块确认顺序 |
| 设备没有能力 flag | 页面隐藏或显示明确不支持状态 |

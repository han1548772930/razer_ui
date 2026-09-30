# 自定义（`TAB_CUSTOMIZE`，productId `182`）

> 本文是对 Razer DeathAdder V3 Pro 模块的代码审计，不是对共享 CSS 或语言包的罗列。
> 只有在 `182` 模块的 `main.db20a7c4.js` / `ButtonPanelComponent.edcc6e34.chunk.js` / `Map*.chunk.js` 实际进入 render 的内容，才记为已确认。
> `653`、`777` 等其他产品没有在本文中作结论；需要单独读取对应模块后再补充。

## 1. 已确认的入口与条件

- 模块身份：`.ref/devices/182/manifest.json` 的 `deviceName` 为 `Razer DeathAdder V3 Pro`，`productId` 为 `182`。
- 页面标签：`TAB_CUSTOMIZE`；真实标签顺序由产品模块的标签路由提供，不能由共享语言 key 推导。
- 主页面组件实际输出 `config-wrapper dot-bg`、`config-block`、`config-ctx`、左右 `config-btns`、产品图组件和底部 `config-row`。
- `ButtonPanelComponent` 是独立的侧栏/抽屉组件，只有在页面状态 `isPanelOpen` 为真时显示；它不是主配置图的替代物。
- 映射内容按 `isHyperShiftOn`、`buttonList`、当前选中的 `activeButton` 和保存状态过滤；不能把所有 Map chunk 都当成页面同时渲染。

## 2. 实际组件树

```text
Customize root
├─ profile / keymap 相关顶部区域（由主页面状态和 profile 数据控制）
├─ config-wrapper.dot-bg
│  └─ config-block
│     ├─ config-ctx                  Canvas：绘制按钮之间的连线/提示状态
│     ├─ config-btns.flex.left       buttonList[0], [2], [6], [5], [7]
│     ├─ product image component      pid=DeviceInfo.productId、editionId、layoutId
│     ├─ optional cycle image        仅 CycleUpSensitivityStages hover/选择时
│     └─ config-btns.flex.right      buttonList[1], [3], [4]
├─ dim-corner
└─ config-row.mt20.mb10
   ├─ Standard / Hypershift 层切换
   ├─ 按键面板开关
   └─ 与保存/映射状态相关的操作
```

### 2.1 主配置图的真实尺寸

| DOM/CSS | 已确认值 |
|---|---|
| `.config-wrapper` | `height:340px; width:100%; min-width:770px; max-width:1220px; margin:0 auto auto` |
| `.config-block` | `height:340px; width:770px; margin:0 auto; position:relative; z-index:1` |
| `.config-btns` | `max-height:100%; max-width:235px` |
| `.config-block .mouse-svg` | `position:absolute; left:50%; transform:translateX(-50%); max-width:300px; z-index:1; max-height:340px` |
| `.config-ctx` | 位于 config block 内，用于绘制按钮连线、hover/active 标记和动态提示，不是装饰背景 |
| `.config-row` | 主图之后，源码使用 `mt20 mb10`，不是第二个产品图区 |

### 2.2 按钮顺序与交互

`main.db20a7c4.js` 的 JSX 明确按下列顺序创建按钮：

- 左列：`buttonList[0]`、`buttonList[2]`、`buttonList[6]`、`buttonList[5]`、`buttonList[7]`。
- 右列：`buttonList[1]`、`buttonList[3]`、`buttonList[4]`。
- 每个按钮把 `clickBtn`、`hoverBtn`、`leaveBtn` 传给按钮组件；点击会更新 `activeButton`，而不是直接把动作改成默认值。
- hover 会触发 `drawLines()`；`CycleUpSensitivityStages` 相关按钮会显示附加的 cycle 图像和阶段提示。
- 主图中的 `config-ctx` 与按钮状态同步重绘连线；实现时不能只渲染一组静态按钮列表。

## 3. 按键面板与动作映射

### 3.1 `ButtonPanelComponent` 的真实结构

独立 chunk `ButtonPanelComponent.edcc6e34.chunk.js` 的 render 为：

```text
Fragment
├─ .tooltip-panel-button
└─ [role=complementary][aria-label="button panel"].config-drawer
   ├─ .drawer-top.flex
   │  ├─ keysDrop 下拉
   │  └─ .close-drawer
   └─ .scrollable.config-drawer-content
      └─ ButtonPanelItem × filtered buttonList
```

过滤条件包括：

- `keyProperty`：普通、功能键、多功能等不同列表；
- `isHyperShiftOn`：只显示对应层的按钮；
- `isMapped`：筛选器为 mapped 时只保留已经映射的按钮；
- `disabled` 与 `isSidePanelList`；
- `isMappingChanged`：点击另一个按钮或关闭面板前先走保存提示，而不是静默丢弃。

### 3.2 单个面板项

`ButtonPanelComponent` 的 item render 实际包含：

- `.drawer-btn`：按 enabled / remapped / active / hyper-shift 组合状态挂类；
- `.index`：按钮序号；
- `.key`：动作类型与 binding value；
- `.binding-value`：hover 时显示完整文本 tooltip；
- 二键映射标记：由 `TwoTapKeyMapping` 条件渲染。

面板按钮点击调用 `clickBtn(buttonKey)`，再由主页面打开该按钮的映射编辑区；不能用普通下拉行替代整个行为链。

## 4. 映射编辑内容的证据边界

`MapDefault`、`MapMouse`、`MapSensitivity` 等 chunks 是按当前 `activeButton.mappingType` / `buttonKey` 动态选择的。

### 已确认

- `MapDefault`：默认/禁用/普通鼠标动作等基础映射。
- `MapMouse`：鼠标动作映射，并可能包含 Turbo 控件；源码中 `disableTurbo` 为真时不渲染 Turbo 区域。
- `MapSensitivity`：DPI、DPI Clutch、DPI On-the-Fly、阶段切换；源码根据 `CycleUpSensitivityStages` 和 `isHyperShiftOn` 过滤数据集。
- `MapScrolling`：只有当前映射动作进入该 chunk 时才出现滚动动作配置。
- `MapMacro`、`MapSwitchProfile` 等：不能因为文件存在就断言在 `182` 默认页面中显示；必须由当前 action 的动态 import 和 mapping type 触发。

### 不应写入实现的错误结论

- 不能把所有 `Map*.chunk.js` 直接平铺为页面卡片。
- 不能把共享语言包中的 `KEY_*`、`SNAP_TAP`、`OLED` 等 key 当作 182 页面功能。
- 不能把 `buttonList` 的数组索引当作稳定的设备输入 ID；原代码使用 `buttonKey`、`inputID` 和映射对象关联状态。

## 5. 视觉契约

以下值来自 `static/css/main.48c20423.css` 与 `ButtonPanelComponent` 相关样式，属于页面实际使用的公共外壳或组件样式：

| 角色 | 值 |
|---|---|
| 页面底色 | `#222` |
| 主文字 | `#ccc` |
| 卡片/输入底色 | `#111` |
| 主绿色 | `#44d62c` |
| 通用边框 | `#5d5d5d` |
| hover 覆盖 | `#ffffff1a` / `#2d2d2d`，按具体选择器使用 |
| 普通按钮次色 | `#707070` |
| 卡片 | `600px` 宽、`padding:30px 40px`、`border-radius:5px` |
| 行内按钮 | `height:27px`、`border-radius:3px`、字号 `12px` |

这些公共值只描述真实命中的组件；不代表每个共享 CSS 选择器都在 182 页面出现。

## 6. 验收清单

- [ ] `config-wrapper` 是主区域，不是通用 600px 设置卡片。
- [ ] 设备图居中，左右按钮顺序与源码索引一致。
- [ ] Canvas/连线/hover/active 状态存在；没有只显示文字按钮的替代实现。
- [ ] ButtonPanel 是可打开的 complementary drawer，有筛选、滚动、关闭和未保存保护。
- [ ] 动作编辑由 active button 的 mapping type 路由到对应 Map chunk。
- [ ] 未验证的共享 key、其他产品能力和未触发 chunk 不进入 182 页面说明。

## 7. 证据文件

- `.ref/devices/182/static/js/main.db20a7c4.js`
- `.ref/devices/182/static/js/ButtonPanelComponent.edcc6e34.chunk.js`
- `.ref/devices/182/static/js/MapDefault.5ba91040.chunk.js`
- `.ref/devices/182/static/js/MapMouse.d4aa1eba.chunk.js`
- `.ref/devices/182/static/js/MapSensitivity.ed0c234c.chunk.js`
- `.ref/devices/182/static/js/MapScrolling.edd6bbbe.chunk.js`
- `.ref/devices/182/static/css/main.48c20423.css`
- `.ref/devices/182/manifest.json`

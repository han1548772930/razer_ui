# 设备页标签栏（`.nav-tabs .nav`）依据审计

2026-10-03。机器可读结果 [device-tabs-audit.json](device-tabs-audit.json)，脚本 [tools/audit-device-tabs.cjs](../../tools/audit-device-tabs.cjs)（`--check` 失败即报错）。
源码：`.ref/devices/182/static/css/main.48c20423.css`。

## 标签按钮

| 选择器 | 源码声明 | 本地 |
| --- | --- | --- |
| `.nav-tabs .nav` | `border-radius:14px;color:#999;line-height:14px;margin-right:20px;padding:7px 10px;text-align:center;text-transform:uppercase;white-space:nowrap;transition:background-color .3s,color,.1s` | `surface::navigation_button`：`rounded(14px)`、`px(10px)`、`h(28px)`（= 2×7 + 14 行高）、字号 12px、标签 `to_uppercase()`；标签之间 20px 间距由容器 `gap(20px)` 提供 |
| `.nav-tabs .nav:hover` | `background-color:#2d2d2d;color:#ccc` | 悬停底色 `theme.secondary_hover`（实测 `#2D2D2D`）、悬停文字 `theme.foreground`（`#CCCCCC`）——**本轮修正**：原先没有改悬停文字色 |
| `.nav-tabs .nav:active` | `background-color:#3cbf27;color:#111` | 按下底色改为源码的 `#3cbf27`——**本轮修正**：原先用的是选中色 `#44d62c` |
| `.nav.active` | `background-color:#44d62c;color:#111` | 选中态 `theme.primary`（`#44D62C`）+ `theme.primary_foreground`（`#111111`） |
| `.nav.disabled` | `opacity:.3;pointer-events:none` | 不可用按钮 `opacity(0.3)` + `disabled(true)` |

主题色映射由脚本逐条核对：`muted_foreground=#999999`、`foreground=#CCCCCC`、`secondary_hover=#2D2D2D`、`primary=#44D62C`、`primary_foreground=#111111`。

仍未做到的一条（已记录值）：`transition:background-color .3s,color .1s` 与按下态文字色 `#111`——`gpui_kit` 的 `Button` 只暴露 `.hover`，要插值就得像 `surface::keymap_close_button` 那样自持 hover/pressed 状态；导航按钮目前用的是变体静态色。

## 导航栏的完整结构（源码 JSX）

profiles 应用 chunk `9449.8f17b519.chunk.js` 里的导航栏组件给出了整行的结构，逐条对应本地实现：

```jsx
<div className={classNames({"nav-tabs": true,
                            "keymapbar-enabled": renderKeyMapBar && displayKeyMapBar,
                            disabled: showLinkedGames})}>
  <div className="profile-wrapper" ref={profileEl}>    // profile 栏（+ 键位栏）
  <div className="navs-wrapper" role="tabs">
     {visibleNavs.map(nav => <NavItem ariaRole="tab" active={activeView===name} …/>)}
     {dropdownNavs.length > 0 &&
       <Dropdown menu={dropdownNavs}
                 toggleClass={"dots3 hover-border " + (inMenu ? "has-actived-option" : "")}
                 menuClass="profile-act" elementId="NavBarDropdown"/>}
  <div className="right" ref={rightEl}>                // 电量 / 帮助
```

- `filterNotHelpNavs()` + `visibleNavs` + `renderDropdownNavs()` = 本地 `split_navs` + `nav_overflow`；
- `toggleClass` 与本地 `.hover-border.dots3`（含 `has-actived-option` 绿底）一致；
- `showLinkedGames` 时整行加 `disabled` 类（本地在关联游戏弹层打开时会话尚未加这条，已记录）。

## 返回 / 前进箭头

| 选择器 | 源码声明 | 本地 |
| --- | --- | --- |
| `.nav.back` | `background-image:url(nav_back_arrow.0203563a.svg)` | 打包为 `synapse/nav-back-arrow.svg`（来源即该文件），`surface::nav_arrow_button` 渲染 9×18 的图 |
| `.nav.forward` | `background-image:url(nav_fwd_arrow.8ceda865.svg)` | 打包为 `synapse/nav-fwd-arrow.svg` |

两个箭头本身就是 `.nav`（同样是 28px 高、14px 圆角、`padding:7px 10px`、无文字标签），因此本地按同一几何绘制，不可用时同样套 `.nav.disabled` 的 `opacity:.3`。

行为依据：设备页组件自己在 `sessionStorage` 里维护 `tabNavigations` / `currentTabNavigation`，并提供 `navigateBack` / `navigateForward` / `updateNavigationView`。本地在 [workspace.rs](../../src/features/workspace.rs) 里加了对应的 `page_history` + `page_history_index`：切页时截断「前进」分支再追加（`record_page`），历史回放不重复入栈（`history_navigation` 标志），两个箭头按钮放在标签组最左边、按索引边界置灰。

## 标签溢出（`.navs-wrapper .dots3`）

设备页每页都要放下「自定义 / 性能 / 电源 / 校准」这些标签，窗口变窄时原版会把放不下的标签收进一个 `.dots3` 下拉。依据是设备页组件自己的两个方法：

```js
renderNavs = () => {
  const e = this.filterNotHelpNavs(),
        E = window.innerWidth
            - this.getElWidth(this.profileEl.current)      // profile 栏
            - this.getElWidth(this.rightEl.current)        // 右侧（电量/帮助）
            - this.getElWidth(document.querySelector(".navs-wrapper .dots3"))
            - 10;
  // 每个标签的宽度 = getTextWidth(文案, "normal 12px Roboto") + 20 内边距
  //               + 20 间距（最后一个不加），贪心放进可见列表
}
renderDropdownNavs = () => this.filterNotHelpNavs()
  .filter(nav => !this.state.visibleNavs.find(v => v.name === nav.name))
  .map(nav => ({...nav, action: nav.name, active: nav.name === this.state.activeView}))
```

`filterNotHelpNavs()` 把 HELP 从标签列表里排除（HELP 是右上角帮助按钮），这也印证了本地「页签不含帮助」的实现。

| 选择器 | 源码声明 | 本地 |
| --- | --- | --- |
| `.nav-tabs .navs-wrapper` | `display:flex;font-family:Roboto,sans-serif;font-size:12px;justify-content:center` | `#device-navs` 用同一套 gap 20 的 flex 行，标签宽度按 12px（0.75rem）文字宽度量 |
| `.hover-border` | `height:26px;width:26px;border:1px solid #222;border-radius:13px;background-size:20px;margin-right:10px;transition:border-color .2s;will-change:border-color` | 触发按钮 26×26、圆角 13、1px 边框（常态 `theme.background`=`#222`、悬停 `theme.border`=`#5d5d5d`、打开/激活 `#44d62c`）、20px 图标；边框色用 `motion::transition` 按 **200ms `ease`** 插值（`surface::hover_border_color`），profile 栏的「更多」与标签溢出按钮共用同一实现 |
| `.navs-wrapper .dots3` | `background-image:url(icon_more_default.eb7284c1.svg);border:none;border-radius:13px` | 打包为 `synapse/nav-more-default.svg` |
| `.dots3:hover` | `background-color:#2d2d2d;background-image:url(icon_more.fb688d78.svg)` | 悬停换图 `synapse/nav-more-hover.svg`（用 `NavMoreState` 自持 hover 状态，因为 `Button` 只能改样式不能换图） |
| `.dots3.has-actived-option` | `background-color:#44d62c;background-image:url(icon_more_active.8e04906f.svg)` | 当前页被收进溢出时：`#44d62c` 底 + `synapse/nav-more-active.svg` |
| `.profile-act`（容器） | `background:#000;border:1px solid #5d5d5d;min-width:155px;max-width:280px;position:absolute;top:26px;left:-1px` | 溢出面板 `#000` 底 + `1px #5d5d5d` 边框 + 最小 155 / 最大 280 宽——依据是导航栏 JSX 里 `menuClass:"profile-act"` |
| `.profile-act .action` | `display:flex;height:auto;line-height:17px;padding:5px 6px;text-transform:capitalize` | 行高 27px（= 5 + 17 + 5）、左右内边距 6px |
| `.act` | `color:#ccc;font-size:14px` | 菜单行 14px、常态 `theme.foreground`（`#CCCCCC`） |
| `.act:hover` | `background-color:#1a1a1a` | 悬停 `#1a1a1a` |
| `.act.action.active` | `background-color:#000;color:#44d62c` | 当前页行 `#000` 底 + `theme.primary` |
| `.act.action.active:hover` | `background-color:#1a1a1a;color:#44d62c` | 同上，悬停底色变 `#1a1a1a` 而文字保持绿色 |
| `.act.action.uppercase` | `text-transform:uppercase` | 行文案 `to_uppercase()` |

**行的几何来自源码本身**：导航栏 JSX 给下拉传的是 `menuClass:"profile-act"`、`toggleClass:"dots3 hover-border …"`，行元素类名是 `act action uppercase`，所以行高与内边距取的正是 `.profile-act .action{padding:5px 6px;line-height:17px}`（= 27px 行），容器取 `.profile-act` 的 `#000` 底 + `1px #5d5d5d` + 155/280 宽。上一轮曾把这条当作「借用同栏兄弟菜单」的值，本轮在 JSX 里找到 `menuClass:"profile-act"` 后已确认它就是源码规则本身。

`.hover-border` 的 `transition:border-color .2s` 本轮已按 CSS 默认的 `ease` 曲线插值（两个 `.dots3` 按钮共用 `surface::hover_border_color` + `surface::hover_border_button`）；`.dots3:hover` 的**底色**过渡源码里没有声明 `transition`，因此底色仍是即时的，只有边框色插值。

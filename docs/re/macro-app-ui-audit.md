# 宏应用界面当前源码审计

**2026-10-04 更正：下面的资源/语言缺失结论已失效。**宏应用229个SVG已按manifest准备，palette的11个图标均存在。专用中文并非公共trans包：`1250.ba9a498a.chunk.js`模块81250包含新建宏、文件夹、搜索和真实空态等文案。十个专用字典已定位，正在沿加载链复核并接入。旧脚本的unresolved_keys只反映其搜索范围不足，不可作为无源码的证明。当前UI仍在实施，资源存在不表示完成。


> 来源：当前 Synapse 包 `.ref/applications/synapse/macro/`。结论由 [extract-macro-app-ui.cjs](../../tools/extract-macro-app-ui.cjs) 静态提取，逐条带文件、偏移与 SHA-256 收据；没有执行任何下载的 JavaScript。机器可读收据：[macro-app-ui-audit.json](macro-app-ui-audit.json)。

## 窗口与外框

| 项 | 值 | 说明 |
| --- | --- | --- |
| 窗口名 / 地址 / 标志 | `macro` / `/synapse/macro/` / `policy=3,tab_visible=1` | 见[窗口打开契约](display-window-contract.md)与[宏应用审计](macro-app-current-audit.md) |
| 页面容器 | `MacroContainer_my_macro__jCmj8` | CSS 只有 `position:static`，容器不加定位 |
| 预载容器 | `MacroContainer_setup_svgs__92r2C` | `display:none` 的图标预载元素（undo/redo/delete 五个 SVG），不绘制 |
| 结构 | `<div class=my_macro><div class=setup_svgs/><MacroContent/></div>` | 组件 `xr` |

## 导航标签

| key | 简体中文（应用自带语言包） |
| --- | --- |
| `TEXT_NAV_TAB_MY_MACROS` | 我的宏 |
| `TEXT_NAV_TAB_KEY_BINDS` | 按键绑定 |

两个 key 在宏应用自己的 `trans-zh-CN` 分块与 Dashboard 的 `trans-zh-CN` 分块里都存在，值一致；本仓库 `locales/zh-CN.json` 中的同名 key 也是「我的宏」「按键绑定」。

## 功能面板（palette）

模块 81021 定义了两组条目，第二组是主面板，第一组是 AI 条目（原版由开关 `_` 控制是否显示）：

| 组 | 条目（类型 · 图标 · 文案 key） |
| --- | --- |
| `i`（AI） | ai_rephrase · `ai_rephrase.ce435691.svg` · `TEXT_AI_REPHRASE`；ai_summarize · `ai_summarize.953171a6.svg` · `TEXT_AI_SUMMARIZE`；ai_email_composer · `ai_compose_email.9888530a.svg` · `TEXT_AI_COMPOSE_EMAIL` |
| `JB`（主） | delay · `icon_delay_g.5050a3f7.svg` · `TEXT_ADD_MENU_DELAY`；keyboard · `icon_config_keyboard_a.7051c99b.svg` · `TEXT_ADD_MENU_KEYBOARD`；mouse · `icon_config_mouse_o.8a44fbe7.svg` · `TEXT_ADD_MENU_MOUSE_FUNCTION`；macro · `icon_macro_a.7e1bc94f.svg` · `TEXT_ADD_MENU_MACRO`；launch · `icon_config_launch_p.482fbff5.svg` · `TEXT_ADD_MENU_LAUNCH`；command · `icon_runcmd_b.10c00024.svg` · `TEXT_ADD_MENU_RUN_COMMAND`；text · `icon_config_text_b.bc93ac89.svg` · `TEXT_ADD_MENU_TEXT_FUNCTION`；loop · `icon_refresh-1_r.ff48f955.svg` · `TEXT_ADD_MENU_LOOP` |

面板行为：条目可拖动（`draggable`），点击/拖放插入到宏编辑器；当宏类型是「序列」或「分段」时，AI 组与 delay 条目加 `MacroMenu_disabled__m0kCs` 禁用类，两组之间有一条 `.line-divider`。

## 已记录的缺口

- **面板图标缺失**：宏应用在当前源码包中只有 JS/CSS/HTML，`static/media` 未取到，11 个图标全部不在本地（`icons_missing`）。因此本仓库尚未绘制该面板，也没有用别的图标代替。
- **部分文案缺失**：`TEXT_PROFILE_BAR_MACRO`、`TEXT_PROFILE_BAR_NEW_MACRO`、`TEXT_PROFILE_BAR_DROPDOWN`、`TEXT_PROFILE_BAR_DROPDOWN_ADD_MACRO`、`TEXT_ADD_MENU`、`TEXT_ADD_FOLDER`、`TEXT_MACRO_SEARCH`、`TEXT_MULTI_FUNCTION` 在宏应用与 Dashboard 的 `trans-zh-CN` 分块里都不存在（`unresolved_keys`），本地没有可靠译文，因此不臆造。
- 宏数据、录制、编辑器、按键绑定容器（`KeyBindContainer_keybind_container__cNbHR`）都还没有本地实现；宏窗口目前只呈现外框、两个导航标签与上述状态说明。

重新生成：`node tools/extract-macro-app-ui.cjs`；校验：`node tools/extract-macro-app-ui.cjs --check`。

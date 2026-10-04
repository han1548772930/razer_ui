# profiles 应用（`/synapse/profiles/`）路由依据

2026-10-03。机器可读结果 [profiles-app-audit.json](profiles-app-audit.json)，脚本 [tools/audit-profiles-app.cjs](../../tools/audit-profiles-app.cjs)（`--check` 失败即报错）。
源码：`.ref/applications/synapse/profiles/static/js/main.776df2b1.js`、`.ref/applications/rz-app-menu/static/js/main.83ced465.js`。

## 窗口

模块表里 `linkedGames` 的窗口是 **`profiles`**、URL `/synapse/profiles/`、打开参数
`policy=3,shouldFocus=1,tab_visible=1`（同窗口 + 聚焦 + 标签栏可见），所以「已关联的游戏」不是新进程窗口。

## 应用自己的路由（源码常量正则）

| 视图 | 源码常量 | 路由 | 本地文案 key |
| --- | --- | --- | --- |
| 配置文件 | `H=/\/devices\/\d+\//` | `/devices/<设备号>/` | `PROFILES`（配置文件） |
| 通用快捷键 | `b=/\/globalShortcuts\//` | `/globalShortcuts/` | `GLOBAL_SHORTCUT_HEADER`（通用快捷键） |
| 幻彩控制室 | `F=/\/chromaStudio\//` | `/chromaStudio/` | `CHROMA_STUDIO`（幻彩控制室 (Chroma Studio)） |
| 宏 | `B=/\/macro\//` | `/macro/` | `MACRO`（宏） |
| 已关联的游戏 | `k=/\/linkedGames\//` | `/linkedGames/` | `LINKED_GAMES`（已关联的游戏） |

状态键：`tabNavigations`、`currentTabNavigation`（应用按 URL 正则判断当前是哪个视图并记录在 store 里）。

## 本地实现

[src/shell/profiles_page.rs](../../src/shell/profiles_page.rs)：左侧列出上面五个视图（顺序与源码常量一致，标签用语言包里真实存在的 key），点选切换；每个视图给出该路由的**源码正则原文**与依据说明；`/linkedGames/` 视图额外渲染 `.list-box` + 源码里空列表也存在的 `.add-new` 虚线磁贴（样式依据见 [linked-game-tile-audit.md](linked-game-tile-audit.md)）。

各视图的**内容**尚未实现（配置文件列表与编辑、本地/云配置文件、导入导出、通用快捷键列表、已关联游戏数据），界面不显示任何伪造数据。

## 与既有审计的关系

- [module-registry-audit.md](module-registry-audit.md)：`linkedGames → profiles` 的窗口名/URL/打开参数来源。
- [linked-game-tile-audit.md](linked-game-tile-audit.md)：`.linked-game-tile` / `.add-new` 的全部样式声明与本地常量对照。

## 导航栏与视图容器（chunk `9449.8f17b519.chunk.js`）

profiles 应用里那个「设备配置文件」实例给出了导航栏的用法：

```jsx
class extends Component {
  navigateBack = () => { this.view = this.state.tabNavigation[this.state.currentTabNavigation - 1]; … }
  navigateForward = () => { this.view = this.state.tabNavigation[this.state.currentTabNavigation + 1]; … }
  render() {
    return (
      <>
        <Xt tabNavigations={…} title={t(r.L$3)} navigateBack={…} navigateForward={…} currentTabNavigation={…}/>
        <u extraClass="no-scroll razer-profiles">{this.renderView(this.renderNavbar())}</u>
      </>
    );
  }
}
```

- 导航栏（`Xt`）自己维护 `tabNavigation` / `currentTabNavigation`，最左边是 `.nav.back` / `.nav.forward` 两个箭头（用 `surface::nav_arrow_button`，到边界 `opacity:.3`）；
- 视图容器带 `razer-profiles` 类：
  `.razer-profiles{overflow:hidden!important;padding:0!important}` —— 这一层不滚动、不留内边距，间距由各视图自己给。

本地 [profiles_page.rs](../../src/shell/profiles_page.rs) 已按这两条实现：导航栏 = 返回/前进箭头 + 视图标签（20px 间距 = `.nav-tabs .nav{margin-right:20px}`），历史用 `history: Vec<ProfilesView>` + `history_index`（切视图时截断「前进」分支，回放不重复入栈）；视图容器 `overflow_hidden()` + `p_0()`，说明文字放在一个有内边距的内层容器里（真实视图各自管间距，本地是占位内容）。

**仍未实现**：两个视图（`{id:1,name:r.ey1}` / `{id:2,name:r.db_}`）的正文内容——它们的文案符号在压缩包里与其它模块的同名符号冲突，无法可靠还原；本页目前用应用自己的路由常量给出五个视图（见上表）。

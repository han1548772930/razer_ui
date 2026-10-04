# 已关联游戏磁贴（`.linked-game-tile`）依据审计

2026-10-03。机器可读结果 [linked-game-tile-audit.json](linked-game-tile-audit.json)，脚本 [tools/audit-linked-game-tile.cjs](../../tools/audit-linked-game-tile.cjs)（`--check` 失败即报错）。

- 结构出处：`.ref/applications/synapse/profiles/static/js/1255.b556ae48.chunk.js` 的 `renderGameTiles()`
  ```jsx
  <div className="list-box">
    {games.map(game => <GameTile …/>)}
    <div className="linked-game-tile add-new" onClick={openAddAppPopup}>
      <div className="game-body"><div className="plus-icon"/></div>
      <div className="game-footer"><label>…</label></div>
    </div>
  </div>
  ```
- 样式出处：`.ref/applications/synapse/profiles/static/css/1255.d3d31cbf.chunk.css`、`9449.b8e7f39a.chunk.css`

## 源码声明（提取到的原文）

| 选择器 | 声明 |
| --- | --- |
| `.popup-widget .list-box` | `display:flex;flex-wrap:wrap;margin-right:-5px` |
| `.device_to_linked_game .linked-game-tile` | `background-color:#111;border:1px solid #111;border-radius:5px;margin:10px 5px 0` |
| `.…linked-game-tile,.…linked-game-tile.undetected` | `height:190px;position:relative;width:240px` |
| `.…linked-game-tile:hover` | `border:1px solid #44d62c`（`.game-footer .name` 同时变 `#44d62c`） |
| `.…linked-game-tile:active` | `border:2px solid #44d62c` |
| `.…linked-game-tile.active` | `border:2px solid #44d62c!important` |
| `.…linked-game-tile.add-new` | `background:#0000;border:2px dashed #5d5d5d;transition:border-color .2s;will-change:border-color` |
| `.…add-new:active,:hover` | `border-color:#44d62c;box-shadow:none` |
| `.…add-new .game-footer` | `font-size:14px;line-height:16px;padding:0;text-align:center` |
| `.…linked-game-tile .game-body` | `align-items:center;background-position:50%;background-repeat:no-repeat;background-size:cover;border-radius:5px 5px 0 0;display:flex;height:120px;justify-content:center` |
| `.…linked-game-tile .game-footer` | `height:70px;padding:10px;width:100%` |
| `.…game-footer .name` | `color:#ccc;font-size:14px;line-height:19px;margin-bottom:3px;text-align:center;text-overflow:ellipsis;text-transform:…` |
| `.…linked-game-tile .linked-profile` | `background:#11111180;border-radius:5px 5px 0 0;height:40px;position:absolute;width:100%;z-index:1` |
| `.…linked-profile-name` | `color:#44d62c;font-size:14px;left:40px;line-height:17px;max-width:80%;position:absolute` |
| `.game-tile.add-new .plus-icon` | `left:50%;margin:auto;position:absolute;top:50%;transform:translate(-50%,-50%)` |

`120px`（游戏区）+ `70px`（页脚）= `190px`（磁贴高），审计脚本会校验这个等式。

## 本地实现

[src/ui/game_tile.rs](../../src/ui/game_tile.rs)：`list_box()`、`linked_game_tile()`、`add_new_tile()`，以及磁贴尺寸常量 `TILE_WIDTH=240`、`TILE_HEIGHT=190`、`BODY_HEIGHT=120`、`FOOTER_HEIGHT=70`、`PLUS_ICON=40`；样式逐条对应上表（`group_box=#111`、`primary=#44d62c`、`border=#5d5d5d`、虚线边框、圆角 5px、`.list-box` 的 `-5px` 右外边距）。

接入位置：

- [linked_games.rs](../../src/features/linked_games.rs) 的关联游戏对话框：从「列表 + 添加程序按钮」改为源码的**磁贴墙** —— `.list-box` 里每个已关联程序一张 `.linked-game-tile`（页脚渲染 `.game-footer .name`，保留本对话框原有的删除入口），末尾固定一张 `.add-new` 虚线磁贴，点它走原有的文件选择流程。
- [profiles_page.rs](../../src/shell/profiles_page.rs) 的「已关联的游戏」视图：本地没有该窗口的数据源，因此只渲染源码里空列表也会出现的那张 `.add-new` 磁贴。

「添加」磁贴的加号用源码资产：`.plus-icon{background-image:url(icon_add.c95a8d74.svg);background-size:40px;height:40px;width:40px}` —— 该文件与 182 设备包里的 `icon_add.c95a8d74.svg` 同名同哈希，本地打包名是 `synapse/dashboard-add.svg`（审计脚本会核对这条来源链）。

## 未实现（有依据，不伪造）

- **游戏封面 / 程序图标**：源码 `.game-body` 用游戏封面（`background-size:cover`），没有封面时用 `.game-body.ico-image{background-size:60px 60px}` 放程序图标。本地没有封面数据库、也没有从可执行文件取图标的实现，因此磁贴的游戏区**留空**（不画替代图），页脚照常渲染名称。
- **「添加」磁贴的完整三段文案**：源码是 `<Text rVC/> <Text YNT/><br/><Text CpJ/>`，三个符号压缩后改名（`main.js` 里映射到 `w`/`v`/`U`），无法逐一还原；profiles 窗口里用语义最接近、语言包确实存在的 `ADD_GAME`（添加游戏，对应源码英文 `Ht="Add"` + `xt="Games"`），设备对话框里用 `ADD_GAME_AND_PROGRAM`（游戏与程序，对应 `DN="Games & Programs"`）。
- **`.undetected` 未识别态**：源码 `.linked-game-tile.undetected .game-body{opacity:.3}` + 40px 图标层 + 悬停 tooltip；需要「游戏是否安装/可检测」的数据源，规则已记录，数据源接入后补。
- **`.linked-profile`（游戏→配置文件）条**：需要「游戏关联了哪些配置文件」的数据，规则同样已记录。

## 导入 / 导出配置文件的底栏（下一轮要用到的证据）

profiles 应用 chunk `9449.8f17b519.chunk.js` 里，导入/导出弹层的底栏结构是：

```jsx
<div className={styles.footer}>
  <div className={styles.willNotImport}><Text r.QN3/></div>              // mode==="import" 时隐藏
  <div className="import-profile-btn-group">
    <button className="thx-btn test" onClick={close}/>
    <button className={"thx-btn ml10 " + (canExport ? "" : "disabled")}/>
    {mode === "import" &&
      <div className={"warning " + (hasWarning ? "" : "hidden")}><div className="tip"/></div>}
  </div>
</div>
```

对应的 CSS（profiles 主样式）：`.import-profile-btn-group{display:inline-flex;flex-shrink:0;height:27px}`、`.import-profile-btn-group .thx-btn{border:1px solid #0000004d;font-family:Roboto;font-size:12px;height:100%;text-align:center}`、`.import-profile-btn-group .warning{align-items:center;display:flex;margin-left:10px;position:relative}`、`.warning .tip{background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-family:Roboto;font-size:14px;left:-113px;line-height:16px}`、`.warning:hover>.tip{opacity:1;visibility:visible;z-index:100}`。

同一模块还给出配置文件菜单的条目表：`[{gjU},{yE7},"divider",{Pie},{mjN},{l1q},"divider",{SJi}]` 与 `[{Pie}]`（两个分区），与本仓库已有配置文件菜单的分区一致。

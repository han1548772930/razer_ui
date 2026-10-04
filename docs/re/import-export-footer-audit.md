# 配置文件导入 / 导出弹层底栏依据审计

2026-10-03。机器可读结果 [import-export-footer-audit.json](import-export-footer-audit.json)，脚本 [tools/audit-import-export.cjs](../../tools/audit-import-export.cjs)（`--check` 失败即报错）。
源码：`.ref/applications/synapse/profiles/static/js/9449.8f17b519.chunk.js`（JSX）、`.ref/applications/synapse/profiles/static/css/main.ee3cb5b6.css`、`.ref/devices/182/static/css/main.48c20423.css`。

## 底栏结构（源码 JSX）

```jsx
<div className={styles.footer}>
  <div className={styles.willNotImport}><Text r.QN3/></div>     // 非导入（导出）模式显示
  <div className="import-profile-btn-group">
    <button className="thx-btn test" onClick={close}/>
    <button className={"thx-btn ml10 " + (canExport ? "" : "disabled")}/>
    {mode === "import" &&
      <div className={"warning " + (hasWarning ? "" : "hidden")}><div className="tip"/></div>}
  </div>
</div>
```

| 选择器 | 源码声明 | 本地 |
| --- | --- | --- |
| `.import-profile-btn-group` | `display:inline-flex;flex-shrink:0;height:27px` | 底栏按钮组：`h_flex` + `flex_shrink_0` + 27px 高按钮，间距 10px（源码 `.ml10{margin-left:10px}`） |
| `.import-profile-btn-group .thx-btn` | `border:1px solid #0000004d;font-family:Roboto;font-size:12px;height:100%;width:fit-content` | 见 [thx-button-audit.md](thx-button-audit.md)（同一套 `.thx-btn` 实现） |
| `.willNotImport` | CSS module 哈希类名，样式表里取不到 | 本地导出弹层底栏左侧渲染 `IMPORT_EXPORT_MODAL_WILL_NOT_IMPORT`（源码 `QN3` = "Linked Games and Chroma Effects will not be exported."），12px、`muted_foreground` |
| `.warning` / `:before` | `padding-left:30px;position:relative`，`::before` 是 20×20 的 `warning.ad3f47f8.svg`，绝对定位在左边 0 | **未渲染**（见下） |
| `.warning.hidden` | `visibility:collapse` | 同上 |
| `.warning .tip` | `background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;left:-113px;line-height:16px;opacity:0;padding:8px 10px 10px;position:absolute;text-transform:none;top:-200%;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:300px` | 同上 |
| `.warning:hover>.tip` | `opacity:1;visibility:visible;z-index:100` | 同上 |
| `div.nav-tabs.disabled` | `opacity:.5`（设备页在 `showLinkedGames` 时给整行加 `disabled`） | `DeviceWorkspace.linked_games_open`：打开关联游戏弹层时导航行 `opacity(0.5)`，弹层关闭时复位 |

文案 key（全部取自源码、在本地语言包里存在）：`IMPORT_EXPORT_MODAL_WILL_NOT_IMPORT`（`QN3`）、`IMPORT_EXPORT_MODAL_TIP`（`bG` = "Not all mappings from this profile can be carried over to this device."）、`TEXT_IMPORT_PROFILES`（`KG`）、`TEXT_EXPORT_PROFILES`（`zG`）。

## 未渲染的 `.warning` + `.tip`（不臆造条件）

源码只在 `hasWarning` 为真时才显示这个感叹号提示，含义是**这个配置文件里的部分映射无法迁移到当前设备**。本地导入要么整体成功、要么整体报错（`decode_profile` 会校验 `product_id` 与 `layout_id`，不一致直接拒绝），因此目前**没有**可渲染的真实状态；规则已逐条记录在上面，等有了「部分映射被丢弃」的真实信号再接。

底栏外壳（源码里的 `.footer` / `.willNotImport` 是 CSS module 哈希类名）在样式表里取不到，所以外层仍沿用本仓库既有的弹层底栏几何（居中、`padding:16px 20px`、上边框），只把**组内**结构与间距按源码对齐。

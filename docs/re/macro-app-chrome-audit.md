# 宏窗口（`/synapse/macro/`）外壳与资源缺口审计

2026-10-03。机器可读结果 [macro-app-chrome-audit.json](macro-app-chrome-audit.json)，脚本 [tools/audit-macro-app-chrome.cjs](../../tools/audit-macro-app-chrome.cjs)（`--check` 失败即报错）。
源码：`.ref/applications/synapse/macro/static/css/main.9ea5d7e7.css`、`.ref/applications/synapse/macro/static/js/main.3f4b9604.js`。

## 顶部导航（`.navbar`）

```jsx
<div className="navbar">
  <Mn hasTHXWarning={false} hasOBM={false} hasBattery={false} gameList={[]}
      isEnableProfileBar={true} isHelp={"help"===d}/>
  <div className="header-links module-nav">
    {T.map(e => <span className={"nav " + (e.view===d ? "active" : …)}><Text text={e.name}/></span>)}
    {P.length > 0 && <Dropdown menu={P} toggleClass={"dots3 " + (L?"has-actived-option":"")}
                               menuClass="profile-act size-auto" adaptivePosition/>}
  </div>
  <div className="right">…教程图标…</div>
</div>
```

| 选择器 | 源码声明 | 本地 |
| --- | --- | --- |
| `.nav-wrapper` | `background:#222;width:100vw` | 顶栏 `bg(theme.sidebar)`（实测 `#222222`） |
| `.nav-wrapper.over-border` | `border-bottom:2px solid #000` | `border_b_2()` + `theme.title_bar`（`#000000`） |
| `.nav-wrapper .module-nav` | `align-items:center;display:flex;flex:1 0 max-content;height:46px;justify-content:center` | 顶栏 `h(46.)` + `justify_center()`；标签本身仍是 `.nav-tabs .nav` 那套（28px、圆角 14、`#999/#ccc/#3cbf27/#44d62c`） |
| `.module-nav.disabled` / `.hide` | `cursor:default;pointer-events:none` / `display:none` | 未接入（宏窗口的禁用/隐藏条件依赖模块安装状态，本地没有该数据） |
| 溢出下拉 | `toggleClass:"dots3 …"`、`menuClass:"profile-act size-auto"` | 标签溢出组件已在设备页实现（见 [device-tabs-audit.md](device-tabs-audit.md)），宏窗口暂未接（本地宏只有两个视图，不会溢出） |

本轮把本地宏窗口顶栏从「48px + 1px `theme.border` + 左对齐」改成源码的 **46px / `#222` / 2px `#000` / 居中**。

## 左栏 `.navFolder`（已取到规则，图标缺失故未绘制）

```jsx
<div className="navFolder flex">
  <label>{t(Y.V8j)}</label>
  <div className="addMarco">
    <div className="btn addMarcoFile"   onClick={…}><div className="macro_tooltip">{t(Y.UVl)}</div></div>
    <div className="btn addMarcoFolder" onClick={…}><div className="macro_tooltip">{t(Y.tQC)}</div></div>
  </div>
</div>
```

| 选择器 | 源码声明 |
| --- | --- |
| `.navFolder` | `border-top:1px solid #515151;margin:10px 0;padding:10px 0` |
| `.navFolder .btn` | `margin-left:10px;position:relative` |
| `.navFolder label` / `.navFolder .addMarco` | `flex:1 1 50%` / `display:inline-flex;justify-content:flex-end` |
| `.navFolder .addMarcoFile` | `background-image:url(icon_new_marco.6edec51b.svg)`；`:hover` → `icon_new_marco-hover.47953e67.svg`；`:active` → `icon_new_marco-pressed.0faeaad4.svg` |
| `.navFolder .addMarcoFolder` | `background-image:url(icon_addfolder-1.3c65591c.svg)`；`:hover` → `icon_addfolder-hover.a804c680.svg`；`:active` → `icon_addfolder-pressed.aa5ec3ce.svg` |
| `.navFolder .macro_tooltip` | `background:#000;border:1px solid #515151;color:#ccc;display:none;left:50%;min-width:100px;padding:5px;position:absolute;…`，`:hover > .macro_tooltip{display:block}` |

**这两个按钮没有绘制**：`.ref/applications/synapse/macro/static/media/` 在本次抽取里不存在，按文件名与哈希（`6edec51b`/`3c65591c`/`47953e67`/`0faeaad4`/`a804c680`/`aa5ec3ce`）在整个 `.ref` 树里都找不到对应文件。目录树用的文件夹图标倒是有同哈希副本（设备包里的 `icon_folder_grey.220b24b0.svg` = 应用的 `icon_folder_g.220b24b0.svg`），`icon_layer.0c20e889.svg` 同样缺失。

脚本会统计宏应用 CSS 引用到的媒体文件在 `.ref` 里能否按哈希找到：**164 个引用中 39 个缺失**（其余在设备包里能找到同哈希副本）。这 39 个是宏功能面板、左右栏与部分弹层的图标，也是本地宏页面目前只给出说明文字的原因。

## 仍未接入

- `.navbar` 里的 profile 栏（源码 `Mn` 组件，`hasOBM=false`、`hasBattery=false`、`isEnableProfileBar=true`）与 `.right` 的教程图标；
- 宏列表 / 宏编辑器正文（依赖宏服务数据）；
- `.navFolder` 左栏（等图标补齐后再画，规则已记录）。

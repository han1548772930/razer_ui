# 产品侧 displayMode 根的本地实现

> 本文主体记录 2026-10-04 的初次接入规则。后续已确认独立 Armory 根不能普遍等同于主导航映射页；当前逐产品独立根见 [2026-10-05 审计](armory-independent-roots-2026-10-05.md)、[2026-10-06 第一批](armory-audio-entrypoints-2026-10-06.md)与[第二批](armory-audio-next-roots-2026-10-06.md)。下文“音频/配件家族不打开”的旧范围已被这些具体产品实现部分替代，未逐产品核验的根仍不能据表中“已接入”宣称完整复刻。

2026-10-04。归属表由 `tools/generate-display-mode-roots.cjs` 从
[分支审计](display-mode-audit.md)（`docs/re/display-mode-audit.json`，静态扫描当前产品包）
生成到 `src/features/display-mode-roots.json`；Rust 侧统一由
[display_mode_roots.rs](../../src/features/display_mode_roots.rs) 读取，窗口契约的
`DisplayMode::key()` 也引用同一张表，因此「某产品是否有某分支」永远来自产品包扫描，
不是手写名单。机检：`node tools/audit-display-mode-roots.cjs --check`，收据
[display-mode-roots-audit.json](display-mode-roots-audit.json)。

| 模式 | 根分支产品数 | 本地状态 | 本地规则与入口 |
| --- | ---: | --- | --- |
| `chromaApp` | 212 | 已接入 | 产品需有根分支**且**有本地灯光页（适配器灯光标签，或 source 家族的 `TAB_LIGHTING`）。Chroma 窗口的设备卡打开弹层根；无灯光页时保持原版不可用态。 |
| `armory` | 226 | 已接入 | 产品需有根分支**且**有本地映射页（适配器 Customize，或 source 家族的 `TAB_CUSTOMIZE`）。入口是设备「分享到工坊」，工坊窗口按设备类型挂 460/340/420px 的设备面板。 |
| `macro` | 174 | 见宏应用 | 产品侧分支只出现在宏应用「绑定到设备」弹层的 iframe 里；本地由宏页承载，不重复开第二个产品根。 |
| `multiDevicePairing` | 30 | 已接入 | 具名第二窗口；入口是配对页设备卡。 |

## 关键源码依据

- `chromaApp` 根（182 包 `LG` → `DG` → `CG` → `uG`）：只对请求的序列号渲染
  （`"chromaApp"===a && _===this.state.serialNumber`）；Escape 向父窗口发送
  `closePopup`；带一条样式覆盖 `.body-wrapper, .main-container{ min-width: unset; }`。
  本地按设备建立弹层，因此序列号条件天然成立；Escape 关弹层已接；覆盖以 `min_w_0()`
  体现。产品侧 `Jg`（armory 根）与 Chroma 根都只挂产品自己的页面，不重做布局。
- 设备卡样式来自 Chroma 应用 CSS：`.box-item-device{background-color:#111;border:2px solid #111;
  border-radius:5px;height:245px;padding:10px 20px 20px;width:290px}`、
  `:hover{border-color:#44d62c4d}`、`:active{border-color:#44d62c}`、
  `.disabled{opacity:.3;pointer-events:none}`、`.box-img-container{height:140px;width:250px}`、
  `.name-tag{height:50px;justify-content:flex-end}`（不再居中）、
  `.name-tag .name{font-size:14px;line-height:16px;text-transform:uppercase}`。
- `armory` 根（Armory 包 `main.3d0e8bd0.js`）：宿主把产品页按
  `/synapse/products/<referenceUuid>/ui/index.html?displayMode=armory` 嵌入，frame 高度
  `KEYPAD 460px` / `HEADSET·AUDIO 340px` / 其他 `420px`；握手消息
  `armoryIframeReady`（产品→宿主）、`armoryMappings-<productId>`（宿主→产品）、
  `armory-button-list`、`armory-change-viewIndex`（产品→宿主）、`hypershiftMode`（宿主→产品）。
  产品侧根只把 `showMouseUse:false` 的映射组件挂进 `drawer-open` 外壳，没有产品导航和配置栏。

## 本地偏差与边界

- 两侧消息不再经过 postMessage：本地两侧在同一进程，直接把同一份产品页挂进工坊窗口，
  消息名仅作契约记录（收据里逐条写明）。
- 音频、配件家族与没有本地灯光页/映射页的产品**不打开**对应根，也不伪造页面。
- `macro` 的产品侧根没有在本地产品页里重复实现，仍由宏应用窗口承载。

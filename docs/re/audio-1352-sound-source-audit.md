# Leviathan V2 Pro 当前 TAB_SOUND 六卡片批次（2026-10-10）

本批次直接读取当前官方 PID 1352 原 JS/CSS/静态资源。没有执行官方 JavaScript、DLL、应用、helper、安装器或设备操作；没有全库索引。对应既有完整页面为 `local-ui-reverse/pages/synapse--products--1352--ui--TAB_SOUND--4712603.md`（约 30 MB）。该既有文件的追加写入被文件系统拒绝，因此新增本小型附录，不替换旧页面全文。

## 当前源版本和实际页面树

JS：`local-ui-reverse/source/official/apps.razer.com/synapse/products/1352/ui/static/js/main.95a4f703.js`，SHA-256 `8dee0170de93fd9900024510e8c6edd315c30010d759ca12609ad6d2c8c73c80`。

CSS：`local-ui-reverse/source/official/apps.razer.com/synapse/products/1352/ui/static/css/main.150adff3.css`，SHA-256 `b6970104c8a887ad5f6501afdcfd4548ae98bee838200b19cd0edcb21ad453d5`。

以下为该 JS 的字符偏移。`LG` 在 `4593861`，首个 TAB_SOUND 导航实际挂载在 `4712637`。路由存在本身不代表卡片实现完成。

```js
LG=()=>(0,Y_.jsxs)(gR,{children:[
 (0,Y_.jsx)(HU,{}),
 (0,Y_.jsxs)(yR,{direction:"left",children:[
  (0,Y_.jsx)(jU,{}),(0,Y_.jsx)(DG,{}),
  (0,Y_.jsx)(rG,{}),(0,Y_.jsx)(tG,{})]}),
 (0,Y_.jsxs)(yR,{direction:"right",children:[
  (0,Y_.jsx)(NG,{}),(0,Y_.jsx)($U,{})]})]})
```

| 位置 | 组件/源偏移 | 实际内容及条件 |
| --- | --- | --- |
| 全宽顶部 | HU/UU `4562888` | `widget-prod dot-bg` 产品图，高 250px；常规最大 1220px、最小 1024px；chromaApp 显示模式不挂载。pU 使用官方 1x/3x artwork，edition 回退至 0。 |
| 左 1 | jU/XU `4577652` | VOLUME_HEADER 标题开关、0..100 step1 滑杆、明确 0/100 范围标签、Windows 音量混合器链接；没有第二个 toggle 或设备读取状态文案。 |
| 左 2 | DG `4593192` | SUBWOOFER_LEVEL；1..7 step1、LESS_BASS/MORE_BASS 标签；HEADSET 输出禁用并显示源 disabledTip；变动保留 subWoofer.isEnabled，300ms debounce；销毁取消 debounce。 |
| 左 3 | rG `4587662` | SOUNDBAR_HEADSET_TITLE、描述、36px 高双态圆角 selector、tip 与原内嵌输入源图标；无耳机连接时禁用。 |
| 左 4 | tG `4583617` | INPUT_SOURCE，源未挂载标题开关。SWITCH_TO_BLE 按钮打开 EG 确认 popup，点击 popup 外关闭。确认实际提交 bluetooth，再关闭。 |
| 右 1 | NG `4590351` | AUDIO_MODES；4 个选择：Stereo、THX Virtual Headset、THX Virtual Speaker、Room Fill；每个 mode 的独立描述/提示、相机圆形状态及 hover 文案、带原插图的帮助 tip、原 demo poster/play。HEADSET 输出禁用。 |
| 右 2 | $U/JU `4580289` | SOUND_PROPERTIES，Windows 属性链接。LG 未传 isTHX，因此本 PID 本调用不增加 THX 第二个链接。Windows 11 与其它 Windows 使用不同原图。 |

XU 的 `canDisabledVolume` 和 `isSystemVolumeMixer` 均没有从 LG/jU 的此次调用传入：本页不会仅因为 inputSource 为 bluetooth/aux/optical 而禁用音量，文案使用 OPEN_WINDOW_VOLUME_MIXER。通用 XU 的其它产品参数分支不能强加到本调用。

## 条件、状态和提交语义

原 reducer 初始值在 `4098631..4098872`：audioModeSelected=0、cameraTrackingStatus=0、audioOutputSource=SOUNDBAR、isHeadsetConnected=false、subWoofer={level:4,isEnabled:true}。这是源初始 UI 状态，不是实际设备观察结果。Rust 使用这些源初始呈现值；`leviathan_has_observation()` 仅在真实观察注入后为 true。新字段不写入既有本地 profile draft，不据此伪造设备成功结果。

| 源 UI 操作 | reducer 偏移 | 原 middleware 消息及 payload |
| --- | --- | --- |
| 调低音炮 | `4132331` | ON_SET_SUBWOOFER `{subWoofer:{level,isEnabled}}`；isEnabled 从原对象保留 |
| 切换音箱/耳机 | `4131469` | ON_SWITCH_AUDIO_OUTPUT_SOURCE `{audioOutputSource:"SOUNDBAR"或"HEADSET"}` |
| 调音频模式 | `4130678` | ON_SET_AUDIO_MODE `{audioModeSelected:0..3}` |
| 确认蓝牙输入 | `4136366` | ON_SET_INPUT_SOURCE `{inputSource:"bluetooth"}` |

这些是原 JS 的服务消息；它们不等于 HID report 或 native ABI 证据。Rust `LeviathanOperation::source_message` 保留消息名与参数，但尚不能宣称完成对应设备协议。

输入源 popup 使用源默认 VERSION_1（LG 未传 version），正文为 INPUT_CONFIRM_POPUP_CONTENT/TIP。其它产品调用可传 VERSION_2/3/4，必须单独审计，不向本 PID 添加版本选择器。源 CSS 在 `417159`：360px 宽、20px padding、10px gap、#111 背景、1px #fd8611 边、5px 圆角、top:-128%；只有一个 SWITCH_TO_BLE 按钮，没有虚构 Save/Cancel。

音频模式：0 与 3 相机点为灰色；1/2 且 cameraTrackingStatus===1 时绿色，否则 #fd8611。相机点 4px，left/top=6px；圆形容器 36px，原相机图片 18×20。源相机 tooltip 第二行取 AUDIO_MODE_CAMERA_ENABLE_TOLLTIP2，仅在 1/2 模式挂载。模式帮助中同时有 AUDIO_MODE_TOOLTIP 与 audioModeTip 原图。

## 原 CSS 几何

共享 `.body-wrapper` 位于 CSS `5036`：padding 10px 20px 20px，最小宽 600px。`.widget-col`/`.body-widgets .widget` 位于 `69321`：两列各 600px；卡片 margin 10px auto、padding 30px 40px、背景 #111、圆角 5px。Rust 使用现有 source page_columns/page_column，而非把六卡片扩成六列。

`.sht-wrap` 在 `154204`：column gap20px。`.multiple-tabs-wrapper .multiple-tabs` 在 `82119`：height36px、padding4px、radius18px、1px #5d5d5d；项 radius13px、5px 间距、选中 #44d62c/#111。

`.am-selector` 在 `463045`：flex row gap12px、margin-bottom20px；项 padding6px 20px、radius3px、1px #5d5d5d、选中背景#292929/绿色边；主字12px、上行10px/12px。描述最小高度56px，mode tip 左缩进46px。

音量 SVG view 细节见 `audio-1352-volume-source-audit.md`：保留原 1116-byte sprite SHA `cb5bd96f8c8671735c968584a0fa42692ac6bc4e68101e55c211646a243c549e`，GPUI adapter 仅按原 `<view>` 给 root 添加 viewport。源字节与显示适配字节分别记录。

## 本轮实现与资源准备

实现文件：`crates/razer-pages/src/features/audio_leviathan.rs`、`audio_products.rs`、`audio_volume.rs`；资源最小注册在 `crates/razer-assets/src/lib.rs`，不改其它 family renderer。

14 项资源的源/产物/hash/尺寸/适配边界见 `audio-1352-resource-evidence.json`。工具 `tools/audio_1352_assets.py` 只复制官方 SVG 或将官方 AVIF 静态解码到 RGBA PNG；PNG 与原图解码像素逐字节相同，无 resize。产品 1x/3x 来源分别由当前 `108.b86b4c1b.chunk.js`/`102.00e2235d.chunk.js` 明确导出。内嵌 aG 图标在 JS `4581852`，保留原 path、fill、宽高；新增 viewBox 等于源固有 viewport，用于 GPUI 显示，不能标为原独立 SVG 文件。

新增接口：LeviathanRequest generation+operation，observe_leviathan、complete_leviathan、request_current、cancel_leviathan。取消/离页清空 popup、debounce/pending、递增 generation，迟到回复无法覆盖新 scope。真实写入能力默认 false；没有接通服务时显示失败通知，不假装保存成功。主任务继续负责 workspace/shell 请求转发和服务缺口的失败 completion。

## 尚缺的完整范围

本轮恢复实际六卡片和源资源，不能标记整页/产品完成。disabled 卡片保留 #111 背景，仅标题/内容为 .3 opacity；原 overlay 覆盖卡片内缩 10px 区域，阻止子控件点击并按鼠标 x/y+20px 显示 300px 提示。GPUI Positioner 在视口边缘夹取提示，而源 fixed tooltip 没有明确边缘夹取，这是仍需对齐的边界差异。未完成：四设备操作的真实 local service/native/HID 提交与回读、非 UI 更新订阅、完整失败恢复分支、原视频播放器的可听播放/进度/全屏、不同产品和设备版型的同组件分支、所有 transition/overflow portal 的像素验收。缺失项仍属于完整需求，没有永久延期或只读完成标记。

未运行应用；未触发真实设备写入。已做静态资源像素验证、rustfmt；本整批 cargo check 由主任务统一执行，当前此附录不宣称该新六卡片批次已通过 check 或运行验收。

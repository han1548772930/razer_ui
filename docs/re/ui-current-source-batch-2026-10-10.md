# 2026-10-10 当前源码 UI 修复批次

行为依据为 `local-ui-reverse/source/official/apps.razer.com/` 中逐产品当前原始 HTML/JS/CSS；本文件是工作输出。未读取历史 `.ref` 快照作为实施证据，未执行厂商 JavaScript。

## 已修改的界面与交互

- 鼠标 Power、低功耗与固件分支、七产品 Rotation、167 自定义滚轮与应用列表的源条件已分别核对和修改。
- 七个实际 Advanced 页面（190/192/196/222/226/229/239）恢复动态敏感度源 PCHIP 曲线、250 项表、1100×300 画布、模板/拖动/缩放、即时提交及观察/取消代次。不能添加原界面不存在的 Save/Cancel。
- 动态敏感度教程恢复 header 控件、原弹层布局、关闭逻辑和全局本地偏好。226 不实际挂载教程；196 有固件门槛；其余五产品实际挂载。原视频 305 帧、640×840、时序和解码像素保留，逐帧比较通过。图标外框 24×24/右边距 10px，图案 25×25，opacity 0.8→1 的 200ms ease 过渡。Help 的 body activation 不再误控制 header 弹层。
- 键盘实际挂载的 15 个 Power、19 个 Polling 控件保留逐产品列位置、尺寸、开关与 BLE 条件。691 增加原三项 Indicator LED、提示、原 SVG 静态图层及透明度动画；消息保持 ON_SET_INDICATOR_LED 的原 payload。
- 普通 Snap Tap 已完整复核七产品的组件、录入 editor、行、CSS 和实际 caller：515/716/752 左列，565/567/659 右列，585 全宽。其它共享组件 marker 是候选证据，不能据此启用界面；analog/v3/v4 需要独立实现。
- 六产品 Help（167/190/226/679/740/742）恢复 card-local 单 Reset 按钮确认，保留 BLE/OBM 差异。解析各产品原 HTML noscript 版本，实时固件读取/断连同步 Help。未恢复的 MW/apps 存储字段继续隐藏。
- PID 1352 Sound 恢复原六卡结构和独立观察/请求链；原图像、SVG 与资源像素单独记录。
- 同批保留已核对的 Settings Lighting、About/social、release notes 空状态、字体及 host titlebar 原资源修复。

## 页面与资源记录

对 `local-ui-reverse/pages/` 中精确产品和页面 MD 追加本批原始源码、完整抽取片段、CSS、条件、源码 hash/偏移和实现差距；未把共享组件存在或路由注册算成界面完成。Snap Tap 七产品的完整附录已嵌入对应既有页面 MD。

详细原文件与资源凭证：

- `crates/razer-pages/src/features/mouse_dynamic_evidence.json`
- `crates/razer-pages/src/features/mouse_dynamic_tutorial_evidence.json`
- `crates/razer-pages/src/features/mouse_dynamic_tutorial_media.json`
- `crates/razer-pages/src/features/source_help_current_audit.json`
- `assets/synapse/keyboard-indicator-current-source-audit.json`
- `docs/re/keyboard-snap-tap-pages/`
- `assets/synapse/manifest.json`：原文件 hash、转换结果 hash 和制作步骤分开保留。

## 实际验证与未完成工作

集中检查发现 GPUI API 类型、trait、focus 和渲染封装错误，修复后必要复查通过：`cargo check --locked --all-targets`，exit 0。`git diff --check` exit 0。保留编译 warning；不把检查通过当成运行验收。本批未运行应用、测试、厂商 JS/DLL/helper/installer 或真实设备操作。

原生动态敏感度、actuation、Indicator LED 和 Leviathan 的设备/服务提交适配仍有缺口；已转发请求的未实施分支明确返回 Unsupported，不能把本地草稿保存表示成设备已保存。输入捕获、服务响应/刷新/存储、完整 mapping drawer、其它 Snap Tap 产品分支、部分 Help 和 generic tooltip 细节、生命周期与视觉运行验收仍需逐项实现和验证。

整体所有产品、界面和原生程序的逆向实施尚未完成。


## 190/679 re-audit correction (2026-10-10)

The earlier implementation was incomplete. Static caller and CSS checks against the current product bundles found that shared card primitives were valid only for the outer widget shell; they did not prove page-specific component trees. The old reachable paths therefore fabricated UI in two places: PID 190 used the generic 600px card/button mapping editor instead of the CO/gO 770x340 canvas and MapMouse drawer, and PID 679 reused ordinary keyboard Snap Tap/lighting/image branches instead of the DU/jm single-pair Actuation branch and Sh/ZM/Oh Customize branches. Those paths are now gated or replaced by product modules.

PID 190 root cause evidence: `main.81b09779.js` CO caller UTF-8 offset 4540319, gO mapping caller and `main.7ce428ff.css` `config-wrapper`/`config-btns` rules. PID 679 root cause evidence: `main.194d8a7b.js` DU/Ph/$L/em callers and `main.1c5a651a.css`; the independent extraction is recorded in `docs/re/keyboard-679-actuation-current-source.md`.

The profile-bar source was also rechecked for both products. Their mounted `renderProfileBar` remains present on Help; the source `displayProfileBar` state changes the loader/sync presentation rather than removing the whole bar. The current profile menu records were regenerated statically for PID 190 and 679 from their current bundles and merged into `source_profile_menu_data.json`.

Remaining acceptance limitation: no vendor application, JavaScript, DLL, installer or real device was executed. Rust compile verification is deferred until the current parallel edits settle.

# Base Station V3 Chroma (3946) automation audit

Current source: `.ref/devices/3946/static/js/main.*.js`, matched to
`pending-product-3946-source.json`. Source metadata names this product **Razer
Base Station V3 Chroma**; August T2 is an internal source name. Acorn extraction
records complete component snippets, offsets, hashes, ten locale tables, six
action categories, seven quick effects and literal effect defaults in
`automation-current-evidence.json` / `automation_data.json`.

The formal Customize page now contains the product image, left Automation
widget and right LaunchSoundApp THX/7.1 card, matching the mounted root at
6953104. The developer preview renders the same complete `Automation` view.
Its action editor is an overlay opened from that real page, not a replacement
for the page. Parent navigation/profile chrome is owned by SourceWorkspace.

Automation implements one row per source category (maximum six), enable
switches, add/edit, pickup/putdown toggles, category-specific empty lists,
disconnected selection feedback, restore-previous output/microphone selection,
pause-game selection, macro/game/shortcut candidate selection, Chroma quick /
advanced / off selection, apply-to-all flags, source effect defaults, and
dirty/nonempty Save validation. Save/delete mutate only local preferences.
Editor cancellation discards its private draft. The local profile snapshot
contains source-shaped `automationWidget` records. The official storage field
is `synapse_3946.profiles[activeProfile].automation`: the current root's
`loadActiveProfileSettings` at UTF-16 offset 6984615 loads this field through
`MW_SET_AUTOMATION_TO_UI` into `automationReducer.automationWidget`. This is
profile state even though it has a separate reducer. `automationLinkedGame`
is another source profile field; its editor is still pending. The `persistence`
and `automation_actions` evidence entries retain these paths. Catalogs and
install/service observations are never restored from profile data. Locally saved
quick macros are restored into the local candidate list from their rule lanes.
The explicit
fixture is isolated from persistence.

`crates/razer-pages/src/features/automation/theme.rs` defines the exact CSS palette for panel,
row, hover, pressed, foreground, muted, primary, border, footer, danger and
warning roles. Body uses Roboto 14px at the source 16px rem scale; widget title
uses RazerF5 16px. Widget and row widths/padding, 60px item height, 355px lane
width, 34/35px lane gaps, 37px category select, footer sizing and 109px modal
top margin are derived from the current CSS. Twenty-four SVG assets are recorded in
`automation-manifest.json`: nineteen exact copies from manifest-declared current
URLs, four type icons and one quick-macro delete icon converted from literal JSX SVG paths. Inline receipts
retain the current source range, fragment and source/output hashes.

## Explicit remaining fidelity and integration work

- The root has no automation executor or catalogs adapter. Hardware triggers,
  installed-app detection, audio endpoints, Chroma profile retrieval, THX
  entitlement and activation-code retrieval are not synthesized. A management
  request that cannot be fulfilled reports that it is unavailable; fixture
  requests are visibly labeled as examples.
- Static and Breathing now render the actual source color parameters for each
  trigger lane. Static exposes one color and hides no-color; Breathing exposes
  Color 1 / Color 2 (including no-color) and Random color. Random disables both
  color pickers without discarding their values. Both reuse the native source
  palette/custom-color editor: the validator compares all forty native preset
  colors against the current 3946 `ZT` literal, including `#ffc182`.
  Source-mounted `LP` selects `cl` (`_l`, 6423938–6425144) and `Hl` (`vl`),
  retained under `quick_color_parameters`. Each edit changes only its lane's
  local `setting` and clears apply-to-other-devices as `LP`'s `P` callback does.
  The editor retains these changes until Save; Cancel discards the rule draft.
  Starlight now shares the two-color/random controls and adds the actual 1–3
  duration slider with Short/Medium/Long tags and no numeric tooltip. Its changes
  merge settings. Wave uses `DeviceInfo.WAVE_DIRECTION=UpDown` and values 3/4;
  `GT/yT` toggles on either half, including the selected half, and on the outside
  frame. Current arrow artwork, 42px frame, 40×30 segments, active green fill and
  hover border are prepared from the current source. Audio Meter mounts the
  Color Boost stepper (0.25–4, step 0.25, maxLength 4); unlike the other effects,
  `AI.changeColorBoost` replaces settings with `{colorBoost}` after rounding up
  to a quarter. There is no mounted hardware color picker or Wave speed branch
  for this product. Fire and Spectrum Cycling mount no parameter component
  in this current `LP` switch; shared speed variants are not evidence that 3946
  should display additional controls.
  The number editor is current module 44230, whose behavior is identical to the
  retained native Kiyo editor. Its caller-provided `stepperRegex` is not consumed
  by that module. Automation adds the higher-specificity `.modes-area .stepper`
  presentation (60×27; input 58×25 with 5px left padding and 17px line height),
  keeping the default Kiyo 62×26 presentation. The source receipts include the
  complete stepper module, resolved aliases, parameter labels and the relevant
  ordered CSS rules. Duration owns retained slider/focus state per trigger;
  keyboard changes explicitly update the draft because GPUI `set_value` does not
  emit a SliderEvent. Pointer changes also set the discrete value back into the
  retained slider to snap its percentage to Short/Medium/Long. Visible color
  captions, their 5px trigger gap, the independently centered Medium label and
  30px Apply-to-all gap follow the source layout. Mode switches and Off's
  apply-to-all toggle merge into the existing lane data. Reselecting an effect
  restores that lane's saved setting if present, otherwise the source default;
  this is not a cache of unsaved alternate effects. `yP.renderEffects` contains
  an OR of the two lane props, but the two `GP` mounts each receive only their
  own data and its connector does not inject either lane, so no cross-lane
  advanced panel is implied. These changes have no executor or hardware I/O.
  The duration observer also handles Base Slider accessibility adjustments,
  which do not emit Change; equality checks avoid rewriting the lane or clearing
  apply flags during parent synchronization. Every parameter edit emits LP's
  exact quick-effect envelope, with Audio Meter retaining only its colorBoost
  setting as required by AI.
- The source quick-macro entry opens a 500×369 local editor derived from `aH`
  at UTF-16 offsets 6920385–6931761. It follows the empty-type initial state,
  `Macro N` default naming, four-type dropdown, keyboard capture with a
  ten-key limit and removable pills, launch program/website selection,
  run-command and 54px multiline text (including the source's 250 UTF-16-unit
  limit and counter).
  Clear resets action input; Save requires a name and nonempty action and
  resolves duplicate names before adding a local candidate to the selected
  lane. It stores UI-readable local action data, without a vendor macro service
  payload, native macro execution or driver acknowledgement.
  The Program branch now opens a single-file picker and shows a read-only
  selected `.exe` path; cancellation or invalid selection retains the previous
  draft. Program and Website preserve separate values; Clear and type changes
  invalidate older picker results. GPUI cannot filter extensions in its native
  picker, so `.exe` is checked after selection. See
  [current program picker evidence](automation-quick-program-current-audit.md).
  Native type-menu row icons, the source capture-session keyup behavior,
  pill hover/delete overlays and their 200ms transitions are now implemented;
  see [current keyboard capture evidence](automation-current-audit.md).
  The type menu now uses the source 8/12px option insets, selected text,
  300ms trigger/arrow transitions and immediate conditional mounting;
  see [current menu evidence](automation-current-audit.md).
  Caps Lock physical edges, browser-default trigger metrics and rendered
  focus/animation parity remain unresolved. Game browser/link editing and
  global-shortcut assignment remain follow-up work; their buttons continue to
  report the unavailable service boundary rather than inventing catalogs.
- Advanced Chroma profile selection requires the real profile catalog; source
  unavailable/install/empty states are represented. The THX activation overlay
  reports unavailable and does not generate a code.
- Source row background transitions (300ms) now have matching hover/pressed
  color states in the native row buttons. Action/text transitions (200ms),
  exact selector icon slots and original headset-toggle SVGs remain pending.
  Existing native controls supply
  keyboard behavior but are not evidence of exact source animations.
- The delete confirmation uses the anchored overlay recorded in
  [the current delete contract](automation-delete-current-audit.md); its
  resolved geometry, focus and resize behavior have not been run.
- Rendered geometry, platform font metrics, focus paths and animation timing
  remain unverified; development checks are static only.

These limits mean that this is a substantial native continuation, not a claim
that product 3946 is already pixel-identical or feature-complete.

## Profile header source

`setProfileDropdownState` appears only in the current action export, with no
root page-switch dispatch. The profile reducer initial value at 6066429 is
`enableSwitchProfile: true`; generic adjustment-mode actions may change it.
Evidence snippets are retained in `profile_bar`. Parent integration must not
hide or disable this product's profile dropdown merely because Customize is
selected.

`isEnableProfileBar` only gates the synchronization icon. The constructor's
`KH` excludes `HELP`; subsequent `updateView` and `updateNavigation` exclude
both `HELP` and `TAB_CALIBRATION`. Static label resolution is recorded in
`profile_bar_labels` (`_$r`, `tPc`, and `lgc`). This constructor/navigation
discrepancy is preserved in the audit rather than mistaken for a condition on
the entire dropdown.

## Verification and seam

`node tools/extract-automation.cjs --check`, `python tools/prepare-automation.py`,
`python tools/validate-automation.py` and rustfmt are the allowed checks used.
Compilation is owned by the root task's final `cargo check --locked --all-targets`.
No application, build, tests, downloaded JavaScript or DLLs ran.

Integration API: `Automation::new(&Device,window,cx)`, `supports_page(pid,key)`,
`snapshot()`, `restore(Option<&Value>,window,cx)`, `dismiss(window,cx)`,
`AutomationChanged`, and `open_preview(window,cx)`. Resource embedding consumes
`assets/synapse/automation-manifest.json`.

## 快捷宏键盘捕获与按键显示

2026-10-05。重新沿当前 3946 manifest 定位 `aH`（6920385–6931761），核对真实挂载的键盘分支、`Te` 开始捕获回调、document keydown/keyup 效果、`tH` 类型图标、`eH` 删除图标与有序 CSS。没有执行下载 JavaScript。

修复此前可持续追加按键、缺少捕获会话的实现。仅空列表的 `+` 开始录入；显示源 28px `Start typing` 输入，首次按键后切换到 pill 并保持监听焦点。任一 keyup 结束录入；去重、最多十键。录入时 Escape 作为按键，非录入时关闭弹框。已有按键旁不添加源码没有的继续录入入口，清空或逐个删除后重新出现 `+`。

GPUI Windows 将 Ctrl、Alt、Shift、Windows 独立发为 `ModifiersChanged`，现按按下边沿收集标签、任一释放结束会话。Caps Lock 事件只提供锁定状态，无法还原物理释放；这项仍有差距，没有模拟按键事件。

按键已补 8×10px 内边距、13px 文本、4px 圆角、`+` 分隔符和源 `eH` 24px 删除叠层，采用源正常/悬停/按下颜色、200ms ease 边框与透明度过渡。宏类型已在当前选项与四个菜单项中显示同一 `tH` 原图标；初始 Action 显示 0.3 透明度键盘图标。删除 SVG 由维护提取器静态转换，纳入原有 automation 资源 manifest（24 项）。

类型选择器已移除框架 Select，改为当前源按钮菜单；行内边距、选中文字、箭头旋转与实际条件挂载见 [菜单审计](automation-current-audit.md)。Caps Lock、系统截获组合键、渲染像素、字体与实际焦点未运行验证。保存仍只产生本地宏候选，未增加执行器或伪造服务结果。

允许的静态验证：`node tools/audit-automation-quick-keyboard.cjs --check`、`node tools/extract-automation.cjs --check`、`python tools/validate-automation.py`、rustfmt；统一 `cargo check --locked --all-targets` 由主任务完成。没有运行应用、构建、测试、安装器、下载 JavaScript 或 DLL。

完整源码范围、挂载分支、CSS 与资源哈希见 [automation-quick-keyboard-current-evidence.json](automation-quick-keyboard-current-evidence.json)。

## 快捷宏类型菜单

2026-10-05。从当前产品 manifest 解析 `aH`、`tH`、`Jv`，重新核对菜单 JSX、切换/清空/外部 mousedown 回调及有序 CSS；资源复用当前 automation manifest 的四个类型图标与 `icon_expand`，没有写公共资源。

原生已移除框架 Select。控制区域高27px、行高17px；菜单为黑底、`#515151` 边框、距控制区域1px，宽度至少等于控制区域、最高180px。最终生效的选项内边距是上下8px、左右12px，20px图标构成36px行；最小高度仍为源30px。只有菜单内已选项目的文字显示绿色，控制区域保持灰色；没有框架勾选图标或额外列表游标。

箭头复用当前源 SVG，槽位29×25px、图片大小10px，持久存在的箭头以300ms ease在0与180度间旋转；控制边框的hover/open同样采用300ms ease。菜单的CSS虽声明height/max-height 200ms，但实际JSX是 `k && menu`：初次挂载时已匹配open样式，关闭则直接卸载，没有闭态节点、延迟打开回调或 `@starting-style`。因此本实现按实际挂载链即时显示/移除菜单，没有添加源码未触发的展开收起动画。

触发按钮切换打开状态；菜单外mousedown关闭，触发按钮自身不被外部处理器提前关闭。选择任何项目（包括重复选择同类型）都按源清空动作并关闭菜单。普通原生按钮提供Tab、Enter和Space；未添加源码没有的方向键列表操作。Escape继续由外层快捷宏处理：非录制状态关闭弹框，录制状态记为按键。

选中后原生把焦点还给触发按钮，避免被卸载选项使GPUI失去按键路由；浏览器原页面对已移除焦点节点的处理并不完全相同。源CSS未覆盖按钮默认padding，原生保留6px水平按钮内边距，浏览器默认padding/基线仍未实测。字体度量、视口边缘定位、Tab顺序和真实动画未运行验证，不能据此声称像素一致。

`node tools/audit-automation-quick-menu.cjs --check` 静态核对条件挂载、完整原组件、最终CSS级联、图标字节与原生接入；键盘和Program现有审计同步保留。只执行静态解析与格式检查，未运行应用、构建、测试或下载代码。统一cargo check由主任务执行。

详见 [automation-quick-menu-current-evidence.json](automation-quick-menu-current-evidence.json)。

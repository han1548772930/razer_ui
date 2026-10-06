# Philips Hue 当前源码审计

## 2026-10-05 local Chroma route

The current Chroma application is implemented locally by `src/shell/chroma_page.rs` and `src/shell/chroma_window.rs`, and the shell already opens it under the audited `chroma-app` window contract. The source Hue advanced-effects branch has two explicit Chroma actions: the not-installed branch mounts `INSTALL_RAZER_CHROMA`, while the installed branch mounts `LAUNCH_CHROMA_STUDIO`. Both local Hue actions now emit a navigation-only `HueChromaRequested` event. `SourceProductWorkspace` forwards that as `WorkspaceEvent::OpenChroma`, and the shell opens the existing local Chroma window. The route does not set, persist, or infer `chroma_installed`; external installation, profiles, and synchronization remain unconnected.


2026-10-03。本页状态为 **partial_native**，不表示完整视觉等价或已连接真实 Hue 服务。

## 来源与实际入口

- 当前产品 769：`.ref/devices/769/static/js/main.ad1113f8.js`，SHA-256 `3d1660626594af99c54c622b2e1caf3190e172e8703392a65286e81b9a3fff9d`。
- 样式：`.ref/devices/769/static/css/main.341edb83.css`；清单、CSS 哈希、组件范围与默认值见 [结构化证据](hue-current-evidence.json)。
- 仅用 Acorn 解析下载源码；没有执行下载的 JavaScript。`tools/extract-hue.cjs --check` 可核对生成数据。
- 导航条 `l` 的组件分支与真正入口 `qA` 相反。实际 `qA` 在 `isPaired=true` 时挂载 `pA`，否则挂载 `Fi`，本实现按实际入口分派。
- `Xo` 没有收到 `renderProfileBar`，因此 Hue 不显示通用设备配置文件下拉框。

## 已挂载的原生内容

`src/features/hue.rs` 保留状态与控件实体；`hue/onboarding.rs`、`bridge.rs`、`brightness.rs`、`effects.rs` 分别拥有对应界面，外层 `SourceProductWorkspace` 只负责路由与本地草稿。

- 连接引导：标题、说明、扫描、扫描中、手动 IP、找到网桥、等待 Push-Link、失败与重试等待界面。
- 已配对工作区：两个 600px 栏，网桥、灯效、亮度和设备四块内容；保留共享的 1280px 栏切换规则。
- 网桥：开关、娱乐区选择、刷新与删除入口、关闭和占用状态、接管控制与移除确认提示。
- 亮度：总亮度开关、全局/逐灯方式、0–100 控件、设备离线禁用与计数。没有设备时计数为零，不使用源码 reducer 中的种子计数 `1/1/1`。
- 快速灯效顺序来自当前 `QUICK_EFFECTS`：环境感知 11、音频计 12、呼吸 2、光谱循环 3、静态 1。已接入屏幕区域、色彩增强、双色/随机颜色和单色控件。光谱循环没有额外参数。
- 高级灯效：Chroma 未安装图文和已安装但无配置的状态；安装、启动、跨设备同步服务未接入，相关命令不可用。当前源码在已配对且非 loading 的 Home 页面挂载一次性教程指示点：36×36px 动画资源 `indicator_animated.e9e90a63.svg`，点击后显示 `ADVANCED_EFFECT_DETAILS` 提示框（约 280px 宽，左偏移 90px、上偏移 -15px，橙色 1px 边框、#111 背景、14px/17px 文本）；本实现使用本地 `tutorial_visible` 状态关闭提示，不伪造 Chroma 服务状态或持久化。
- 自定义文案来自 `_i` 的十种语言，回退由模块 37 的 `Nm` 导出确认是 `en`。共有文案通过当前模块 4693 的导出核对。
- 提取 48 个灯具 archetype 路径、网桥/断开等 SVG、精确刷新/删除路径、安装图和高级灯效教程动画，共 63 个资源。资源来源、请求 URL 和输出 SHA-256 见 `assets/synapse/hue-manifest.json`。原 AVIF 静态转换为 PNG。
- 当前 Hue 的 40 个预设颜色与 `no-color` 顺序已和共享原生颜色选择器核对一致；这不替代整个颜色编辑器的视觉验收。

## 保留的源码行为

- `Pi` 输入初始是四个空值，不是 reducer 中的 `192.168.50.158`。`parseInt` 会移除前导零、解析数字前缀并把大于 255 的值截到 255；文本框的 HTML `pattern` 不阻止负号等输入。本实现保留这个实际行为，而非擅自改为严格 IPv4 校验。
- IP 键盘行为来自 `onKeyUp`：空值 Backspace 退到前一段，三字符值前进到后一段，首尾留在有效段内。
- 配对按钮请求 `WAIT_USER_CLICK_PAIR`，但 `Bi` 没有对应内容；必须等待服务提供 `PAIRING`。没有添加自动跳转。
- `vi` 的“重试”和“从头开始”按钮没有绑定回调。失败预览保留正常外观；`RETRY_PAIR` 预览显示禁用外观。
- `isGlobalBrightness` 缺失时视觉上默认选中；源回调否定原字段，所以第一次点击会写入 `true`，第二次才切换逐灯方式。
- `NA` 接收 `handleSelectDevice`，但实际 JSX 没有使用它。没有添加源码中不存在的勾选框。原生控件按灯具容器 ID 和区域 ID 保持身份，不使用位置序号。
- 灯具按 `deviceContainerId` 去重；通道数取 `name` 的去重数量。离线判断和连接计数来自灯具 `isOn`。

## 状态与当前限制

真实入口初始未配对，扫描按钮因缺少 Hue 通信适配而不可用。没有伪造已发现网桥、扫描结果、配对成功或移除成功。开发设置中的“预览 Philips Hue…”使用独立实体，可选择 18 种状态；示例灯具和娱乐区明确使用 Sample 名称，不写入设备或配置文件。

网桥状态、IP、分组、灯具列表、Chroma 安装状态和全局高级灯效开关不会写入配置文件。支持的本地草稿字段为 `brightness`、`quickEffects` 和 `ports`。预览操作保留源码的本地请求状态，刷新/配对/移除不会用计时器模拟服务回应。

仍未完成：Hue 网络发现和服务消息往返；实际灯具输出；Chroma 安装/启动/配置列表/同步；高级灯效教程提示的持久化；扫描和配对动画；滑块、步进器、提示框与禁用状态的全部原样式细节。普通滑块暂使用 GPUI 组件，尚未还原源 64px 容器、数值气泡位置与全部指针状态。颜色编辑器复用现有实现，动态语言切换和所有弹层的真实窗口行为仍需验收。

## 允许的验证

已通过 `cargo check --locked --all-targets`，包括新增契约测试的编译；测试未执行。静态校验覆盖来源哈希、导出文案、五种灯效、63 个资源和实际路由；格式与资源总表另外验证。没有运行应用、构建、测试、安装器或 DLL，也没有实际窗口截图、焦点/滚动与设备往返验证结果。编译通过不作为 UI 完成证明。

复查命令：

```text
node tools/extract-hue.cjs --check
.work/resource-env/Scripts/python.exe -X utf8 tools/prepare-hue.py
python -X utf8 tools/validate-hue.py
python -X utf8 tools/audit-native-product-coverage.py
```

## 2026-10-06 亮度滑条改用共享 `SourceSlider`

当前 769 的两个亮度控件都是共享 `OT`（`main.ad1113f8.js`）：

```jsx
<OT min={0} max={100} step={1} value={…} active={…} changeValue={…}
    minTag={w.KFn} maxTag={w.zrT} extraClass={active ? "" : sA}/>
```

`KFn`/`zrT` 依 769 的语言导出表就是 `OFF`/`BRIGHT`（证据里已有 `"minTag": "OFF"`）；没有传
`noTip`，所以数值显示在 `.slider-tip` 里；`active` 对应 `.slider-container.on`（否则整块 `.3`
透明度且 `pointer-events:none`）。CSS 仍是共享滑块那一套：
`.slider-container{height:64px;opacity:.3;pointer-events:none;position:relative}`、
`.slider-container.on{opacity:1;pointer-events:auto}`、
`.slider{background:#0000;border-radius:3px;bottom:25px;height:6px;width:100%;z-index:3}`、
`.slider::-webkit-slider-thumb{background:#44d62c;border-radius:8px;height:16px;width:16px}`、
`.slider-container.on .slider::-webkit-slider-thumb:hover{background:#5d5d5d;border:2px solid #44d62c}`、
`.slider-tip{background-color:#44d62c;border-radius:3px;bottom:42px;color:#212121;font-size:12px;
line-height:14px;padding:4px 8px;width:max-content}`，以及
`.slider-container .foot{bottom:-2px;opacity:1;position:absolute;text-transform:uppercase;…}` +
`.foot.min{left:0}` / `.foot.max{right:0}`（`.foot` 不声明颜色与字号，由父级继承）。

本地改动（`src/features/hue/brightness.rs`）：`brightness_slider` 不再用 GPUI Kit 的
`Slider` 加自绘数值行与 OFF/BRIGHT 行，而是 `SourceSlider::new(slider, value/100)`
`.tip(Some("{value:.0}"))` `.enabled(enabled)`（值进 `.slider-tip`、容器 64px、`.track`/`.left`
与滑柄 hover/active 配色都按源），并在容器下沿 `-2px` 增加 `.foot` 两端标签
（`OFF`/`BRIGHT`，`text-transform:uppercase`，禁用时随容器一起降到 `.3`）。
`tools/validate-hue.py` 新增这些 CSS、`OT` 参数（含两处 `minTag:w.KFn`/`maxTag:w.zrT`）与本地
标记断言。

仍未完成：Hue 网络发现与服务往返、实际灯具输出、Chroma 安装/启动/同步、高级灯效教程提示的
持久化、扫描与配对动画，以及其它控件（步进器/提示框）的原样式细节。

## 2026-10-06 高级灯效教程点的跨启动持久化

源（769 `main.ad1113f8.js` 的 `PA` 组件）：

```jsx
const T = () => { visibleRef.current && rootRef.current && (setVisible(false), k.A.set(u_, false)) };
useEffect(() => { (!1 === k.A.get(u_) ? setVisible(false) : isLoading || setVisible(true)); … }, [isLoading]);
```

- 存储键 `u_ = "isShowTutorialHue"`；`k.A` 是 localStorage 包装：`set(k,v)` 用
  `localStorage.setItem(k, JSON.stringify(v))`，`get(k)` 在第二个参数缺省时 `JSON.parse`
  读回，没写过是 `undefined`。
- 所以可见性是「没写过或写过 `true` → 可见；写过 `false` → 永久隐藏」，并且 `isLoading` 为真时
  隐藏；点击指示点即写入 `false` 并隐藏。指示点本体是 36×36 的
  `indicator_animated.e9e90a63.svg`（已提取为 `assets/synapse/hue-indicator_animated.svg`），
  悬挂在高级灯效标签页的 `HUE_TUTORIAL_INCLUDED` 容器里。

本地改动：`src/features/hue/effects.rs` 增加同一键名的持久化
（`TUTORIAL_STORAGE_KEY = "isShowTutorialHue"`、存到应用本地数据目录、
`load_tutorial_visibility` / `save_tutorial_visibility`）与
`sync_tutorial_visibility()`（`is_paired && !is_loading && 存储值不为 false`），
并在 `is_loading` 每次变化处调用（`bridge.rs` 的开关关闭、刷新、启用三处与 `preview.rs` 的状态
切换），点击指示点时写入 `false`。原先的注释说“服务不提供持久化，所以本地不做”——源码里它其实是
localStorage，已按源改正。`tools/validate-hue.py` 新增源片段（`u_="isShowTutorialHue"`、
`!1===k.A.get(u_)?o(!1):e||o(!0)`、`k.A.set(u_,!1)`、localStorage 包装）与本地标记断言。

仍未完成：Hue 网络发现与服务往返、实际灯具输出、Chroma 安装/启动/同步、扫描与配对动画，
以及步进器/提示框等控件的原样式细节。

## 2026-10-06 配对中的进度动画（“扫描和配对动画”里真正缺的那一半）

源 `Bi(e)` 的状态分派与两个组件的差别（769 `main.ad1113f8.js`）：

- `case I_ /* SCANNING */`、`case S_ /* SCANNING_IP */` → `hi()`：只有
  `TEXT_CASE_SCANNING_CONTENT` 与 `.Home_btnGroup` 里的取消按钮（`TEXT_CASE_SCANNING`，
  点击派发 `SCAN_CANCEL`）。**没有进度条。**
- `case C_ /* PAIRING */` → `fi()`：`TEXT_CASE_PAIRING_CONTENT` 加
  `<div className={Home_progressWrapper}><div className={Home_progress}><div className={Home_child}/></div>
  <div className={Home_close} onClick={() => Mi(e, PAIR_CANCEL)}/></div>`。

CSS：`.Home_progressWrapper{align-items:center;display:flex;height:20px;justify-content:center;
margin:10px auto 0;position:relative;width:300px}`、
`.Home_progress{background-color:#44d62c4d;border-radius:2.5px;height:5px;overflow:hidden;
position:relative;width:100%}`、
`.Home_child{animation:Home_move__oy9kP 2s linear infinite;background-color:#44d62c;
border-radius:2.5px;height:5px;position:absolute;width:80px}`，关键帧
`@keyframes Home_move__oy9kP{0%{left:-80px}to{left:100%}}`（80px 绿条在 300px 轨道上 2s 线性循环，
`-80/300 = -0.2667`）；`.Home_close{background-image:url(icon_close_enclosed_1.svg);height:20px;
left:100%;margin-left:10px;position:absolute;width:20px}` + `:hover` 换成 `_hover` 图标。

本轮本地改动（`src/features/hue/onboarding.rs`）：配对分支的 80px 绿条原来静止在轨道左端
（注释写“预览帧，不能暗示网桥回应”），现在按源做 2s 线性无限滑动
`left = phase*1.2667 - 0.2667`（`reduce_motion` 时停在 `-0.2667`），轨道补 `relative` 以便绝对定位
裁剪；300×20 容器、5px `#44d62c4d` 轨道、20×20 的 `left:100% + margin-left:10px` 关闭图标与
hover 图标都保持不变（原本已按源）。`tools/validate-hue.py` 新增上述 CSS/关键帧、`fi()` 的
JSX 片段（`Di`/`ci`/`ui`/`Li` 与 `Mi(e,A_)`）以及本地动画标记断言。

仍未完成：Hue 网络发现与服务往返、实际灯具输出、Chroma 安装/启动/同步，以及步进器/提示框等
控件的原样式细节。

## 2026-10-06 扫描的 13 秒超时

源里两个「开始扫描」按钮（`INIT` 页的 `TEXT_CASE_SCAN`、扫描失败页的
`TEXT_CASE_SCAN_AGAIN`）都做同样两件事：

```js
onClick: () => { Mi(e, SCANNING); mi = setTimeout(() => Ui(e), 13e3) }
const Mi = (e, status, ip) => e({type: h_, payload: {status, ip}});
let mi = null;  const Ui = e => { Mi(e, SCAN_FAILED) };
```

- `hi()`（`SCANNING`）的卸载清理是 `(0,y.useEffect)(() => () => { mi && (clearTimeout(mi), mi = null) }, [])`，
  所以取消扫描（`SCAN_CANCEL`）离开该状态时计时器被清掉；13 秒到点则推进到 `SCAN_FAILED`
  （这是源自身的界面超时，不是伪造网桥回应）。
- 手动 IP 搜索 `Mi(e, SCANNING_IP, a)` **不**启动这个计时器。

本轮本地改动：`HueWorkspace` 新增 `scan_task: Option<Task<()>>`，`arm_scan_timeout()`
用 `cx.background_executor().timer(Duration::from_secs(13))` 在到点后（仅当仍处于
`Integration::Scanning`）写入 `ScanFailed` 与对应的 `last_command`，并清空任务；
`set_integration` 在 `next != Integration::Scanning` 时清掉任务（对应 `hi()` 的卸载清理），
两个「开始扫描」按钮的点击里各调用一次 `arm_scan_timeout(cx)`，手动 IP 搜索不调用。
`tools/validate-hue.py` 新增源片段（两处 `mi=setTimeout(()=>Ui(e),13e3)`、`Ui`/`Mi` 定义、
卸载清理、`Mi(e,S_,a)` 不带计时器）与本地标记断言。

仍未完成：Hue 网络发现与服务往返、实际灯具输出、Chroma 安装/启动/同步，以及步进器/提示框等
控件的原样式细节。

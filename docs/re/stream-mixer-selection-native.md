# 3334/3337 混音开启确认与播放设备选择（partial）

2026-10-06，承接混音数字编辑批次。只静态读取当前产品源，未运行应用、构建、测试、安装器、厂商 JavaScript 或 DLL。

## 当前来源

可重复工具：`node tools/prepare-stream-mixer.cjs --check`，证据为 [stream-mixer-current-evidence.json](stream-mixer-current-evidence.json)。两个产品各自解析自己的 manifest/main、标签和动作导出、CSS，并比较各自原始 SVG；不是把一款产品的符号或行为推广给另一款。

| 产品 | 当前主包 | SHA-256 | yU 范围（UTF-16） |
| --- | --- | --- | --- |
| 3334 | `.ref/devices/3334/static/js/main.496bb585.js` | `320597b62d8538c929ca38bbf594297f7151582c0ac61b3224ba86faa0d7eeca` | `[4502941,4514367)` |
| 3337 | `.ref/devices/3337/static/js/main.a048e901.js` | `835ff3af59d774b85c02bfe97fae62e45ec0921218884fcc6f38224c4103b0b3` | `[4502901,4514327)` |

每产品含 Wn 初态、Yn reducer、Jm 选择动作和 yU 完整挂载根，以及 7 个标签、3 个动作导出收据，共 28 项 AST 收据、4 次产品资源比对。资源统一纳入 `validate-resources.py` 的来源、哈希、SVG 和嵌入键校验。

## 源码已核实 / 界面已接入

- 当前主开关关闭且 `activeStreamMixerStatus` 为真时，开启只打开警告，不提前提交。Cancel 只关闭，Enable 将主开关写入本地草稿后关闭。已开启时可直接关闭。源没有 Escape、Enter 或点击背景的确认/取消动作；本地 Dialog 不增加这几种快捷提交。
- 弹窗使用原始 `STREAM_MIXER_DETECTED`、`STREAM_MIXER_DETECTED_WARNING`、Cancel/Enable、警告 SVG；620×164、20/30 内边距、90×27 按钮、20 间距、100ms 背景显现和底边处于视口半高的位置来自当前内联样式与最终 CSS。用 `surface::css` 保留 rem 比例，颜色归入主题层。
- 页面切换和 restore 会清理当前确认状态并恢复焦点，旧弹窗的后续事件不能提交到新页面。显示中的警告不会因单独收到 conflict=false 消息而自动关闭，符合源 `f && !C` 的条件。
- Playback 列表使用实时名称，缺少当前名称时追加同名禁用行；包括已知空列表且当前名为空的源分支。不自动改选第一项，不添加产品目录、演示端点或伪造 None 设备。
- 列表、主开关、当前页面和弹窗状态均在选择事件到达时重查；断开设备、旧菜单确认和禁用状态不能写入草稿。列表仍可在总线静音时选择，禁用由主开关决定。
- 当前没有列表观察时显示“尚未读取播放设备”，保留本地选择；收到已知列表后才判定设备缺失并展示原警告图标和文案。主开关关闭也不开放警告悬停提示。

实现位于 `src/features/stream_mixer.rs`，由 `AudioProductWorkspace` 持有状态、订阅和本地草稿；`ProductWorkspace → SourceProductWorkspace → AudioProductWorkspace` 提供观察转发入口。只对 3334/3337 创建这些状态和分支。

## 本地草稿和真实状态

`prepare-audio-products.cjs` 新增第五个源字段 `playbackMixDevice`，同样核验 yU selector 与 Wn 初态，默认空串；不修改其他音频产品的初值来源。

运行列表、冲突状态、观察到的播放设备名和读取错误不进入本地快照。未手动选择时，播放设备显示真实观察值（若有），快照删除其本地覆盖字段；明确选择后才持久化 `/device/streamMixerSettings/playbackMixDevice`。因此编辑音量不会把观察值偷偷变成本地覆盖。用户明确选择恰好等于初值时，覆盖标记也会触发 `AudioProductChanged`，沿既有 capture/snapshot 链保存。restore 通过字段是否存在区分本地选择与未选择，旧快照没有该字段仍可恢复。

UI 文案明确为“本地草稿”，不宣称保存成功或设备已启用；本批未新增设备写请求，也不把官方 reducer 的 pending/广播队列伪造成成功响应。

## 只读数据未接通 / 写回后置

当前已确认的观察动作是 `MW_UPDATE_ACTIVE_STREAM_MIXER_TO_UI`（payload 投影为 active）、`MW_SET_PLAYBACK_DEVICES_TO_UI`（payload.playbackDevices → 名称列表）和 `MW_SET_PLAYBACK_MIX_DEVICE_TO_UI`（payload → 当前名称）。`StreamMixerObservation` 表示这些投影，不表示 DLL ABI。

真实中间件发布者尚未接入，当前不会自动制造这些观察。现有 `ServiceRequest::AudioDevices` 的 `simpleEnumerateAudioDevices` 不能仅凭“都是播放设备”就替代本分支的专用列表；两者是否等价及真实查询/订阅接口仍待当前服务封装证据核实。初始冲突状态使用未知，不当成“已读到无冲突”；未知时仍允许编辑本地开关，并明确显示未读取状态。

已经观测到冲突之后，本地开关操作不清除该外部事实；官方 reducer 的清空伴随着真实中间件写请求，本项目这一段仍后置。后续接入须协调请求、回调、观察刷新和本地覆盖的提交/清理规则，不可直接把本地选择当设备状态。

## 静态检查通过 / 运行和视觉未验收

`cargo check --locked --all-targets` 通过，仅原有 3 条 unused-method 警告。音频校验通过 76 产品、1756 控件、77 组 EQ、2829 个当前源哈希；嵌入 JSON 为 51 项、0 失败、4 项既有跳过。统一资源校验通过，新增 2 个混音 SVG。

仍保留：完整输出行/监听图标/电平表与主页面布局、输入通道增删、预设和快捷键、数字输入窗口级滚轮。播放选择器目前挂在原有通用 Playback 面板内；警告 Tooltip 使用框架样式，其源 200px 宽、28px 偏移及显隐细节未收口。Base Dialog 的全视口输入遮罩/焦点圈定也不能等同于源 `.customize .backdrop` 的祖先作用域。窗口像素、真实事件顺序、缩放与实际设备查询均未运行验收，两个产品仍为 partial。


## 最终独立回读

独立子任务再次逐项比较 28 条 AST 切片、两产品 34 条 CSS 规则和 4 次 SVG；确认本地确认/取消、过期选择拦截、restore/set_page 和 snapshot 覆盖逻辑，未发现本批阻断性本地状态缺陷。其指出的离线图标容器 margin-top:5px、margin-right:-10px 已补，移除了该行额外间距；完整下拉/Tooltip 外观与前述运行边界仍保留。最后 `cargo fmt --all -- --check` 和 `cargo check --locked --all-targets` 通过，仍为原有三条警告。

同批按独立报告 J 修复 515 Gaming Mode 的 Menu 只读状态行，仅应用于 pid=515 且当前 keys 含 KEY_APPLICATION，checked 取 isWindowsKeyDisabled，disabled 且无写回调。独立回读确认；Snap Tap（K）与 Keyboard Properties（L）继续待办。鼠标/键盘静态校验通过 76 鼠标规格/862 输入与 71 键盘/6494 源形状，数量不代表本次全部逐项验收。

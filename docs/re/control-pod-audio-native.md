# 1382 Audio Function（Playback / EQ，partial）

本批依据当前 `.ref/devices/1382/` 的 main、MapAudio/8191、9267 选项与映射构造、651 默认输入、9412 Radio、7734 下拉及 2321 服务封装。可重复证据见 [静态收据](control-pod-audio-current-evidence.json)，运行维护工具 `node tools/prepare-control-pod-audio.cjs --check` 只解析源码，不执行厂商脚本。

## 已接入

- 1382 已启用的音频映射行可打开独立临时编辑状态；三种 Playback 动作、两个播放设备选择、第二项 None、断开设备禁用与说明已挂载。选项和值来自当前源，不虚构端点 ID。
- 子编辑器打开时，在后台线程内启动现有 ServiceClient，调用只读 AudioDevices；工作线程负责服务子进程生命周期。此代码路径未在开发验证中运行，也不调用写入 API。
- Rust NativeRuntime 已解析 callback 第三参数，所以 UI 校验的是数组，而非 JS 包装层的 `deviceList`。只筛选 `type=speaker`，成功空数组、读取异常和格式错误分别保留；保存设备缺失时按源回填 disabled/isDisconnected 记录。
- 依据源 Be，只在 Toggle 映射回读设备对象与 ID；默认首设备及 None 按源假值规则处理 null/空串/0。异步结果仅更新发起查询的编辑器，不写产品草稿。
- Save 构造源 rq/Ee 同构映射，父层将 audioGroup 提交到既有 capture/snapshot 本地持久化链。Cancel 丢弃子状态。源 Ee 在 G 为空时省略 playbackPayload，且不禁止选择同一端点；本地没有擅自新增这两种限制。
- 通用 merge_known 不会恢复默认对象以外的可选字段，本批补上当前源模式/动作校验后的完整 audioGroup 回读，避免 playbackPayload 在重启后丢失。
- Reset、同映射类型/祖先/后代替换、切页和 restore 均使旧编辑器失效。Save 再核编辑器身份、当前音频类型、启用状态和打开时的基准值，避免旧编辑器覆盖后续修改。
- Playback Radio 使用 Base 行为，外形按当前源 20px 外圈、10px 选中点、30px 标签起点和 200ms Ease；说明与断开提示采用源字号、行高和颜色。

## 尚未完成

- EQ 的真实官方 validDevices 发布链仍未接通，目前仅有独立运行时观察入口；下述条件投影、控件和本地 payload 已接入。无真实观察数据时保留源无可用设备分支；本地预览、固定快照和 HID PID 不作为真实 EQ 产品。
- 当前编辑器挂在既有通用产品面板中，原左侧映射根、应用级跨设备/主导航保护、requireSynapse 提示和父 Save/Cancel 外观尚未完整复刻。已挂载音频输入之间及产品内部页/前后导航的保护见下文。
- 当前本地 profile 保留其他 assignment 的临时字段，尚不是向设备提交的 mappingList。UI 本地 Save 与 DLL 写回不同，后者继续后置。
- 非标准/损坏映射对象边界、枚举变化通知与重新读取、全部键盘/焦点/窗口像素尚未运行验收。产品保持 partial，不能据读取接口代码已接通宣称真实设备读取已验证。

允许的格式化、`cargo check --locked --all-targets` 与嵌入 JSON 静态校验通过；原有 3 条未使用方法警告。当前数据共 35 项源码收据、4 份相关 CSS 和 1 个共享关闭图标收据；嵌入 JSON 为 51 项通过、0 失败、4 项既有跳过。未运行应用、构建、测试、厂商 JS 或 DLL。


## 自身关闭提示（本轮静态复核通过）

新增 control_pod_audio_warning.rs，对应 913.closeMapping(false) → 主根 displaySaveAlert(clear)，仅用于编辑器自身 ×。dirty 时打开原版保存提示；无 dirty 直接关闭。底部 Cancel 仍不弹提示。

- Save：按源提示按钮不使用普通 canSave 禁用门，提交当前映射后关闭；父层仍核当前编辑器身份/类型/启用与基准，防止过期保存。
- Don't Save：仅将外层 dirty 清为 false、canSave 设为 true；保留子编辑值并留在编辑器。源在该入口没有提供第三个 dontSaveAction 回调。
- 提示 ×：保留外层 dirty 和子值，将 canSave 设为 true，关闭提示。

这些行为不能简化成通用的“保存/放弃并离开”。产品按钮、切页、前后导航也有各自 continuation；本批后续已接音频输入和产品内部导航，范围见下文；仅刷新图标在源中明确同时提供保存与不保存后的继续回调。

提示使用 Base Dialog/按钮，400px 面板、20/30px 内边距、绿色边框、RazerF5 标题、100ms 线性显现以及300ms按钮透明度均有当前 CSS 依据；顶部50%与 translateY(-100%) 对应面板底边位于视口中线。关闭图标与1382当前manifest资源逐字节匹配。原生整体挂载/焦点/真实窗口仍未验收，不计为完整父映射根。


## 换音频输入与产品内部导航

当前可挂载的音频输入编辑器之间切换时，先向现有子编辑器请求离开。dirty 时记录目标并显示保存提示；Save 在旧编辑器身份、启用状态、类型及基准值检查通过后，先提交旧 audioGroup、关闭旧实体，再重新验证目标并打开。Don't Save 或提示 × 不打开目标；前者清 dirty，后者保留 dirty。无 dirty 则可直接替换编辑器。

SourceProductWorkspace 的 set_page 和 step_page_history 在修改页面、历史数组或历史索引之前执行同一保护。提示 Save 后，AudioNavigation 回到父工作区：普通切页仍走原 set_page；前后导航走原 step_page_history，不把历史后退伪装成新增页面。没有 dirty 时清理旧编辑器，包含 Help 页不挂家族 body 的分支。

来源为当前产品 clickBtn、changeView、navigateBack、navigateForward，已加入静态收据。范围仅为现有1382音频编辑器与产品内部导航；原绘图映射根、其他映射类型、外部点击排除表、面板折叠、主Shell跨设备/应用跳转和刷新流程仍未完整接齐。

独立静态复核已确认：页面/历史在defer前未改变，旧实体身份与基准检查保留，历史回放仅移动索引；锁定的GPUI事件队列按FIFO处理，AudioProductChanged先于AudioNavigation，所以本地capture先于切页。cargo check --locked --all-targets 与35项源码收据检查通过；仅原有3条未使用方法警告，未运行应用、测试或DLL。


## EQ 条件控件与本地保存

已接入合格设备分支的设备下拉、三种 EQ 动作及 Specific 的五种预设。运行时 validDevices 与本地 profile 草稿分开保存；SourceProductWorkspace 提供观察转发入口，但当前宿主没有实际调用该入口，不计作真实设备读取完成。

- 显示条件 w 取 AUDIO 且 subCategory 为 SPEAKER 或 SPEAKER_HEAD_CUSHION 的产品是否存在；候选 K 是播放端点 containerId 与这些产品 deviceContainerId 的交集。w 为真不代表 K 非空，保留原版可能出现的空下拉。
- 投影依赖按源使用完整 validDevices 长度与播放端点 G 长度；同长度替换不会触发重算。保留所选数字索引，不在设备变化后自动改选第一个端点。
- 模式切换按旧模式 A 与 w 更新 canSave；EQ 首次挂载及 w 变化保留源 dirty/canSave 语义。EQ 设备、动作与预设回调只改索引，不擅自新增 dirty 或保存门条件。
- K 非空时本地映射包含 selectedPlaybackId、razerAudioSpeakerIds、selectedSpeakerPID，Specific 另含 specificEqValue；原记录缺失的可选字段省略。K 为空时省略 equalizerPayload。源 Playback payload 条件不受当前 EQ 模式额外限制，也予以保留。
- 所选索引失效或当前运行时匹配记录缺失时，普通 Save 和提示 Save 均保留草稿并显示错误，不保存或执行后续导航。没有把失效设备替换成其他设备。

独立复核已回读上述投影、回调和两条保存错误路径，未发现新的阻断缺陷。cargo check --locked --all-targets 通过，仍为三项既有未使用方法警告；未运行应用、测试或 DLL。真实 validDevices 发布、枚举变化通知、完整父映射根及应用级导航保护仍待完成。

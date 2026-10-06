# 226 Basilisk V4 Pro：滚轮模式与等级本地编辑

2026-10-06。仅覆盖当前普通 Customize 挂载的滚轮区域。主体控件和本地编辑已接入，产品仍为 partial；实际设备读取、完整页面布局和运行视觉尚未验收。

## 当前来源

使用 `.ref/devices/226/` 当前资源，普通导航链为 8355 → 模块 42 的 Ks/us → ps/Es → es/qt，非 macro 的 qt 左列挂 fe；fe 使用 Se 渲染 Acceleration / Smart Reel。

| 文件 | SHA-256 |
| --- | --- |
| static/js/main.08f95762.js | ccbd5af37e23d1c64faf62551d15b0ef91d6e89fc06cafab3fb9464c598f884e |
| static/js/8355.3d5e573e.chunk.js | b5d160fe0c231186d7f8271d13ece17a3824b0c35c4962cf2ecec72c46352eb8 |
| static/js/3485.794fe922.chunk.js | 42871b0390959f81d70f761f0429137a8311097d4551c5f7b5d3124bd45e6ebd |
| static/css/3485.3a185bc1.chunk.css | 7e17ff497704138c45fe9bb9a8ac781d095825742f9cdf6cb4402d89169c1293 |

维护中的 `tools/prepare-mouse-226-scroll.cjs` 只使用 Acorn/CSS 静态解析，记录 fe/Se、挂载父链、滑条/开关/勾选/提示模块、当前 profile 默认值、reducer、加载观察、文字和动作符号。输出 [当前证据](mouse-226-scroll-current-evidence.json) 的 52 项 AST 收据、83 条 CSS 规则以及 `mouse_226_scroll_data.json` 的三种模式与默认值。不执行厂商脚本。

## 已接入的本地行为

`src/features/mouse_226_scroll.rs` 的 retained `ScrollWheelEditor` 只由 226 的 MouseProductWorkspace 创建。它替代该产品原有两项通用滚轮 checkbox，经 Changed → MouseProductChanged → SourceProductWorkspace.capture 进入现有本地草稿/Save 流程。

| 操作 | 当前实现 |
| --- | --- |
| 选择模式 | Tactile、FreeSpin、MicroTactile，按源顺序；被禁用项不可选。选择立即更新本地草稿 |
| 禁用模式 | 最多禁用两项；已禁用项总能解除。禁用当前模式时同时回退到按上述顺序找到的首个可用模式 |
| 禁用 FreeSpin | 两组功能派生显示关闭并锁定；保留原 enabled 和 level。解除禁用后恢复保留的开关状态 |
| 两组开关/等级 | Acceleration 与 Smart Reel 独立开关、0–4 等级和 Low/Medium/High 标签；写入口再次检查禁用条件 |
| 拖动等级 | SliderEvent::Change 只预览；Release 才提交本地等级。程序 set_value 不发 Change，恢复/同步不产生编辑 |
| 失活 | 普通 TAB、Help、配置恢复和窗口失活清理未释放预览与 tooltip，并同步已提交等级；隐藏页面不继续提交预览 |

当前 profile 默认值为 FreeSpin、两组 enabled=false、等级 0、disabledModes=[]；主包 reducer 的临时默认 Tactile 单独保留在证据中，不据此声称读到了设备状态。fe 的 300ms debounce 属于发送模式变更请求的服务步骤；当前只完成即时本地选择，不发送这类设备写请求。

## 本地字段与真实观察

草稿保留未知字段。根级 `_scrollWheelLocalFieldsV1` 保存用户显式编辑的六项字段集合，Changed 将字段集合与 scrollWheel 原子写入父草稿。首次默认快照使用空集合，避免恢复配置后把自动默认值认作本地覆盖；无标记的旧草稿按已有字段兼容为本地修改。明确选择默认值也保存归属标记。

六类 `ScrollWheelObservation` 从 ProductWorkspace / SourceProductWorkspace 转发到编辑器，单独保存于 observed；未被用户覆盖的字段可显示观察值，snapshot 只投影 draft。切换配置清空旧观察。模式观察不打断无关等级预览；锁定/关闭观察取消受影响的预览，解锁清除失效提示。

**真实观察发布者尚未接入，不计为 DLL 读取完成。** 本地注记明确“本地草稿，尚未写入设备。”，没有模拟设备成功响应。后续应核实真实查询/订阅、配置身份和服务生命周期，DLL 修改与持久化仍统一后置。

## 外观、生命周期与剩余差异

Base Button、SynapseSwitch、SourceSlider、check_item、Tooltip/Positioner 提供行为；应用以当前 CSS 和 ScrollWheelColors 提供样式。模式容器高 36px、圆角 18px、内距 4px；内项圆角 14px、内距 4/16px，文字颜色过渡 200ms。无 tip 滑条区高 36px、上距 10px/下距 20px，刻度脚距底部 -2px。

禁用时外层 0.4/200ms 与内层滑条 0.3/300ms 按源叠加，稳定透明度为 0.12；标签也保留对应内层透明度。锁定开关/滑条 hover 提示随指针偏移 16/12px、最大宽 320px、14px 文字和 8/10px 内距，使用当前黑底及边框颜色。

本面板仍挂在既有映射区域下方，尚未完成当前 Customize 整页列布局；提示的越界策略、遮挡层级、键盘/辅助功能细节和全部窗口输入没有运行验收。Help 清理同时修正 515 隐藏 Snap Tap 的 keyup 捕获，宿主级导航的完整生命周期继续另审。

`cargo check --locked --all-targets`、格式化、专用提取器 `--check` 和嵌入 JSON 校验通过（53 项通过、0 失败、4 项既有跳过），仅三项既有 Rust dead-code 警告。[独立回读 M](prior-ui-verification-2026-10-06.md)重新比较全部 AST/CSS 收据并检查状态/快照链。未运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。

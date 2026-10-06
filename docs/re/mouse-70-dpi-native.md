# 产品 70 DPI 原生编辑器

更新：2026-10-06。状态 **partial**，不代表产品完整复刻或运行验收。

## 当前源与边界

只使用当前 `.ref/devices/70/static/js/main.8f24b6a1.js` 及其 manifest 声明的 CSS、SVG。SHA、11 项 AST 收据、2 份 CSS 和 11 张资源对应关系见 [源证据](mouse-70-dpi-current-evidence.json)，由 `tools/audit-mouse-70-dpi.cjs --check` 复查。

`zI → uI/cI → EI → XT` 是当前 Performance 的实际挂载链，未传 useTwoWayTab。数字控件是模块 4230。设备配置声明 minDPI=100、maxDPI=16000、dpiStep=1，没有单独的 dpiStepInBox 或网格滑块开关。

本地原始配置使用大写 X/Y/Independent/Active；界面源码的阶段适配对象使用小写 x/y/independent/visible。两者不能直接混写。实现只作用于已核对的产品 70，未推广到其他鼠标。

## 已接入的本地行为

- 关闭阶段时仅保留当前阶段一行；开启时逐行编辑，序号按可见阶段计数。
- 普通 X/Y 提交激活可见行，隐藏行编辑保留活动阶段。非独立 X 提交同步 Y。
- XY 开关使用专门的 cI.toggleY → II/TI → zE 语义：关闭且 X≠Y 才重置 Y，并将该行设为活动阶段，包括隐藏行；相等时不改变活动阶段。
- 可见性开关保留至少两个可见阶段。隐藏活动行后选择下一可见行，找不到再取首个可见行。
- 拖排更新数组及活动阶段，区分上/下插入提示；拒绝其他视图、过期草稿快照和越界活动索引。
- XY、阶段开关和拖动柄按行悬停显示；单行模式隐藏后两者。XY 提示随独立状态切换 ENABLE_XY/DISABLE_XY，依据原 CSS 定位。
- 数字编辑采用 Base NumberInput 和保留的 InputState/SliderState。方向键、步进按钮、控件内滚轮已接入，按下立即步进、每 300ms 重复，释放/移出/切页/恢复配置时取消。
- 对应源 isRegisterEvent：点击数字框后，步进更新本地显示预览，失焦后才提交阶段草稿；Enter/Escape 触发失焦。未注册的步进直接走本地编辑链。
- 输入允许最多六位数字和可选负号。空值/单负号提交按 0 再钳制；失焦总回填规范文本。保留源整数 volumeUp 的字符串加法：刚输入的文本先与步长拼接，再经 parseInput 规范化，未擅自改成算术加法。若步进结果与原行数值相等，按 componentDidUpdate 保留输入字符串，直到实际数值变化或失焦规范化。

配置提交继续发 MouseProductChanged，沿用工作区本地草稿保存；未引入 DLL 修改、设备写回或虚构读取结果。

## 尚未完成

- 原模块注册的是窗口级 mousewheel；目前只接数字控件内滚轮，鼠标移至控件外时的全窗口滚轮拦截未实现。
- 输入与两侧按钮的精确重叠布局、原滑块皮肤/拇指 X/Y、开关皮肤，以及 100ms spinner opacity / 300ms border 过渡仍需对齐。
- 拖动浮层的徽标、位置及编号：源 EI 使用大写 Active，默认 Redux 阶段只有 visible。正常行编号不能替代该字段；profile/MW 注入路径仍需追查。
- XY 提示层级、窗口边缘和其他像素/焦点细节未做运行验证。
- 真实设备读取和服务观察链尚不能由 profile/default 或静态封装证明成功；DLL 写回继续后置。

独立回读结果见 [验证报告](prior-ui-verification-2026-10-06.md)。格式化、cargo check 和静态校验只证明各自覆盖范围，不证明窗口行为或整产品验收。

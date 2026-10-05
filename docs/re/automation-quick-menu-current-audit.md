# 3946 快捷宏类型菜单

2026-10-05。从当前产品 manifest 解析 `aH`、`tH`、`Jv`，重新核对菜单 JSX、切换/清空/外部 mousedown 回调及有序 CSS；资源复用当前 automation manifest 的四个类型图标与 `icon_expand`，没有写公共资源。

原生已移除框架 Select。控制区域高27px、行高17px；菜单为黑底、`#515151` 边框、距控制区域1px，宽度至少等于控制区域、最高180px。最终生效的选项内边距是上下8px、左右12px，20px图标构成36px行；最小高度仍为源30px。只有菜单内已选项目的文字显示绿色，控制区域保持灰色；没有框架勾选图标或额外列表游标。

箭头复用当前源 SVG，槽位29×25px、图片大小10px，持久存在的箭头以300ms ease在0与180度间旋转；控制边框的hover/open同样采用300ms ease。菜单的CSS虽声明height/max-height 200ms，但实际JSX是 `k && menu`：初次挂载时已匹配open样式，关闭则直接卸载，没有闭态节点、延迟打开回调或 `@starting-style`。因此本实现按实际挂载链即时显示/移除菜单，没有添加源码未触发的展开收起动画。

触发按钮切换打开状态；菜单外mousedown关闭，触发按钮自身不被外部处理器提前关闭。选择任何项目（包括重复选择同类型）都按源清空动作并关闭菜单。普通原生按钮提供Tab、Enter和Space；未添加源码没有的方向键列表操作。Escape继续由外层快捷宏处理：非录制状态关闭弹框，录制状态记为按键。

选中后原生把焦点还给触发按钮，避免被卸载选项使GPUI失去按键路由；浏览器原页面对已移除焦点节点的处理并不完全相同。源CSS未覆盖按钮默认padding，原生保留6px水平按钮内边距，浏览器默认padding/基线仍未实测。字体度量、视口边缘定位、Tab顺序和真实动画未运行验证，不能据此声称像素一致。

`node tools/audit-automation-quick-menu.cjs --check` 静态核对条件挂载、完整原组件、最终CSS级联、图标字节与原生接入；键盘和Program现有审计同步保留。只执行静态解析与格式检查，未运行应用、构建、测试或下载代码。统一cargo check由主任务执行。

详见 [automation-quick-menu-current-evidence.json](automation-quick-menu-current-evidence.json)。

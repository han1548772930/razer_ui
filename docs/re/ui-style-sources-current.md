# 当前全页面样式加载、字体与基础布局依据

本页补的是实际 HTML 的样式声明顺序，以及字体、元素默认样式和条件规则的原文。页面挂载树、动态 class、inline style 与状态回调分别继续追到 [产品页面细节](product-ui-details-current.md)、[共享控件](shared-ui-controls-current.md)、[应用页面细节](application-ui-details-current.md)。它们共同用于逐页还原；不能单独把 CSS 清单认定为页面验收完成。

## 全量静态范围

依据仍为 2026-10-02 固定的当前产品/应用源码，本轮没有重新请求所有线上页面。维护工具为 `tools/audit-ui-style-sources-current.cjs`；[完整证据](evidence/ui-style-sources-current.json.zip) 持有每份 HTML/manifest/CSS 的实际字节 SHA-256、UTF-16 原文区间、选择器、条件与声明顺序。没有读取旧前端或旧宿主目录。

| 内容 | 已提取范围 |
| --- | ---: |
| 产品 HTML / 应用 HTML | 331 / 22 |
| HTML 中实际 stylesheet link | 347 |
| HTML inline style | 346 |
| manifest CSS 引用 | 2110，产品2022＋应用88 |
| 去重 CSS 路径 / 字节哈希 | 2064 / 460 |
| font-face 原文收据 | 4950，按文件路径计，含跨产品复用 |
| 全局及元素基础规则候选 | 9841，含 div、表单 focus 和 flex 基础规则 |
| 完整级联或视觉验收声明 | 0 |

`entries[].html_style_order` 保留 HTML 实际顺序、rel/as/media/disabled 等原属性及源码片段；预加载与应用样式明确区分。`declared_css` 单列 manifest entrypoint 与异步资源声明。`css[]` 保存 font-face、基础元素规则、自定义属性规则、keyframes、条件块、其他 at-rule 和 directive。字体 src 与选定基础/at-rule 中的 URL 保留本地文件存在性和已取得文件哈希；没有扫描所有 CSS 图片 URL，也没有将未取得资源当作已加载。

## 三处实际入口的不同加载关系

| 实际入口 | 原 HTML 样式顺序 | 还原约束 |
| --- | --- | --- |
| 产品241 | inline loader `[197,523)` → main CSS preload `[3657,3726)` → main stylesheet `[3726,3787)` | preload 不再增加一次样式级联。`11.54a7797c`、`914.530f0373` 只由 manifest 声明，最终插入先后须追 lazy 加载器。 |
| systray/systrayv2 | inline loader `[168,494)` → main CSS preload `[2754,2823)` → main stylesheet `[2823,2884)` | Widget CSS 随不同 lazy 分支挂载；不能只取 main CSS 描述六类 Widget。 |
| synapse/dashboard | inline loader `[168,494)`；HTML 无 stylesheet link | App 与子页面 CSS 在 manifest 中；不能因 HTML 无 link 就判定没有样式，或把 manifest 数组顺序当实际级联顺序。 |

每个区间对应 `entries` 中同路径的 HTML 哈希。这里没有执行厂商 JavaScript，也没有观察浏览器中 lazy 样式实际插入顺序。

## 字体必须追到 face、状态与实际文字

以产品241为例，`main.1525b0e6.css` SHA-256 为 `f4e4b1bcbe811fcb4347c01f942b29d626f5c789691d78048f895970d9d19229`。开头 `[0,3133)` 是13个 font-face：Roboto 300 normal、400 normal/italic、500 normal/italic、700 normal/italic；RazerF5 100 normal、400 normal/italic、600 normal、700 normal/italic。逐 face 的 `font-family/font-style/font-weight/src` 在证据中保留，Roboto 的本机 `local(...)` 优先项也没有删除。

`body,html` 在 `[3227,... )` 指定 Roboto,sans-serif / 16px / #ccc / #222，满宽高、max-width1920、min-height720、overflow:hidden。卡片和标题还分别指定14px、RazerF5及16px等，不能统一套用根字体。托盘的全局字体链含中文回退，必须读取托盘自己的基础规则，不能从产品241推到托盘。

已有字体文件、成功注册一个 family、CSS 声明 family 是三个不同事实。实际字形仍受 weight/style 匹配、local face、中文回退、加载完成时机及字形度量影响；本轮没有实窗验证。原有 CSS 中没有的 font-display、字号或 fallback 不作为官方依据添加。

## 间距、禁用与覆盖的实际归属

241 主样式明确有 `div{box-sizing:border-box}`、`div.flex{display:flex}`、`div.flex>div{flex:auto}`。这不是全元素 border-box；不能把原生 button/input 尺寸按同一个规则计算。后面的 `flex:0 0 auto!important` 又会覆盖一般子 div 的弹性。完整基础规则现已纳入证据，避免只匹配某个局部 class 后遗漏这些关系。

241 Lighting/Help 的600px列先由 BodyWidgets 排列，卡片自己承担 `margin:10px auto` 与 `padding:30px 40px`；`@media(max-width:1279px)` 再给列 `margin:0 30px`。双列变换、卡片内边距、同列卡片间距属于不同父子关系。flex 子项的上下 margin 不折叠；固定宽度、外 margin 与换行要一起审查，不能为“看起来空”另外叠加统一 gap。详情见 [接收器](receiver-ui-current.md)。

`.slider-container` 默认 opacity .3/pointer-events:none；brightness、on 和 no-pointer 各有独立覆盖，`.has-slider` 又只对特定祖先下的滑块加左距30/宽490。不能将全部灰色滑块统一视为无法调整。Pairing 对话框卡片的 inline 左右10px覆盖共享20px，与主页面卡片 padding 是不同层次。

`.modes-tab`、`.twoway-lighting .lighting-effect`、`.no-inner-border .modes-tab`、hover/active 的优先级和声明次序共同决定 Lighting 页签；不能只套一个32px按钮。实际 class 组合、inline 数据和相应分支均须从 JSX 追到调用方。自定义属性、!important、条件 at-rule 和伪元素仍保留原值；基础规则候选不是已经选出的最终匹配规则。

## 维护与剩余

`node tools/audit-ui-style-sources-current.cjs --check` 重读当前353份HTML及CSS，比较完整证据。这个检查证明文件、声明和区间没有漂移，不证明渲染像素一致。

尚须逐页追真实挂载节点、条件与动态 class，确定 selector 是否匹配、lazy 样式插入顺序、祖先继承、portal/fixed 坐标、媒体条件、伪元素、滚动与字体度量。当前禁止运行应用/厂商JS/DLL，实际窗口、焦点、DPI、像素与动画验收仍为未运行。本轮没有修改 Rust、Cargo、vendor 或设备写回行为。

# Macro 文件夹、树菜单与本地元数据复核

本轮只读取当前 `.ref/applications/synapse/macro/`，未执行应用、下载代码、录制或动作。静态工具 `tools/extract-macro-metadata.cjs` 以 Acorn 定位唯一业务闭包及 webpack 导出，收据为 `macro-metadata-source.json`。CSS 使用 `tools/css-source.cjs` 保留外围 `@media` 条件。

主 JS SHA-256：`fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d`。主 CSS 为 `570aa52da8c8d56ed6ee0a607d2e00b48d720f8b34df07f84666d629f10506ce`；8190 CSS 为 `fae96b1ea43dc90da9654e1fd8816daab4a3aa92e4428e8a0f947c19410bf911`。

## 修正前轮错误结论

| 当前源码范围 | 实际行为 | 本轮处理 |
| --- | --- | --- |
| `Ct @596638..596703`、`an @602203..606094` | **文件和文件夹**都使用 Rename、Duplicate、分隔线、Delete；右键也能切换菜单。更多按钮不能直接执行重命名。 | 两种树项共用该菜单，加入右键、菜单外点/Escape关闭与树内重命名。 |
| 25572 `OM @149881..150594`、`Zs h.$g @752779..753044` | 新建宏和文件夹都 `profiles.concat(...)` 添加到根目录；新文件夹不自动展开。 | 移除“添加到所选文件夹”和默认展开的本地假设。 |
| 25572 `iV @150597..151996` | 真实复制消费者是 `iV`。按文件/文件夹两组收集全局名称，递归赋新 GUID/date；去除末尾 `\(\d+\)$` 后，逐个找第一个未占用的 `(N)`。新副本加入原父目录。 | 新建递归本地副本；前轮用 `N` 命名函数实现菜单复制的结论撤销。`N`仍用于新建/导入命名，不能替代`iV`。 |
| `an` 的复制回调、`Mn @632162..640788` dispatch wrapper | 树菜单没有 `selectDuplicatedItem`；顶部更多菜单显式传true。只有true且复制文件时更新当前宏。 | 树复制保持原当前宏，顶部复制选中新宏。 |
| `Zs h.uZ @753075..753411`、`$s @735916..736058`、`Ws @736062..736228` | 拖到文件夹进入其childs；拖到文件则追加到该文件所在的兄弟列表；拒绝自己及后代目的地。 | 接入本地拖放并在删除旧父关系前检查循环；保留宏ID与创建顺序。 |
| `zs @736343..737389` | 删除文件夹会递归删除里面的宏；当前宏被删后选macroList[0]或空。 | 本地递归删除；用创建顺序选择替代宏，避免树移动顺序改变fallback。 |
| `Mn` delete confirm | 回“我的宏”只在被删项目自身guid等于唯一宏guid时成立；删除包含最后宏的文件夹不满足该判断。 | 区分直接删末宏和删文件夹，不再概括成“任何删到零都跳页”。 |
| `dn @606905..610301` | 已完成教程后，即使宏列表为空，仍可打开选择器；帮助/教程/录制等条件另外禁用。 | 移除宏计数为零的额外选择器门控，可继续管理剩余文件夹。 |

## 搜索、排序与重命名

- `dn` 搜索使用trim后的大小写不敏感文本。文件夹名称命中时保留其全部后代并展开；仅后代命中时保留路径与匹配后代。本轮纠正了“父文件夹命中却仍过滤其子项”的错误。树菜单复制接收这种搜索后的数据投影；顶部复制取完整元数据。
- `un @606126..606867` 只把根数组传给15030 `eh @99364..99869`，`an`递归子项没有再排序。本轮只排序根目录，保留子目录顺序。Name比较器的首个UTF-16单位对标点特殊加权，第二单位比较方向与首单位相反；保留该原始实现，没有换成普通字母序。Date沿创建次序排序，移动不改创建次序。
- `kt @595379..596456` 在Enter/blur提交trim名称，Escape还原；DOM `maxLength=32`。本地输入增加32 UTF-16单位validator，提交仍做同样检查。树重命名同时核对同级列表与全局宏名称；顶部重命名只核对宏列表。
- `dn` 的搜索placeholder在focus时清空，在blur时恢复当前语言。现已监听输入焦点与当前locale，语言切换不会继续保留旧语言placeholder。

## 尺寸、资源和动画

CSS收据包含主CSS的70条与8190的38条相关规则。`@media (max-width:900px)`的wrapper宽度及`@media screen and (max-width:1120px)`的editor偏移仍保留条件，未变为全局规则。

- 树根项经过`macroParent margin-left:-20`和首项margin-left20后对齐；每层实际缩进20px。行line-height30加2px边框，图标20px，left5/top5；名称框源宽225px。文件默认图标不会仅因其为当前宏而变绿，原`an`没有把currentProfile写成active类。
- 更多按钮按row右侧30px/顶部3px对齐。`profile-act`距24px按钮顶部26px；树菜单的后置规则把固定180px改为auto，本地按14px Roboto最大菜单文字加12px padding/2px边框确定宽度。菜单项27px高、padding5px 6px，divider上下4px/左右6px。
- 主菜单和树菜单保留100ms出现；菜单项白色`#ffffff1a`背景以300ms ease变化；selector灰色`#515151`至绿色`#44d62c`边框以300ms ease变化。未重复给同一GPUI元素设置hover refinement。
- 首次新建提示点按`.guide_firstTimeCreateDot`纠正为left100%/top50%，36px SVG的平移为(-9px,-18px)，不再使用前轮估值left24/top18。
- 拖动提示使用原两张20px图片、硬编码原文`Move {name}`、280×40、padding10px 20px、绿色背景和14px粗体。位置跟随指针减10 CSS px。树拖入边框为绿色，真正提交前才检查无效目的地。

新增资源映射由主任务维护资源管道：`tree-more-hover.svg`←`icon_more-pressed.f7f8588b.svg`，`tree-more-active.svg`←`icon_more-hover.aa4fe640.svg`（倒序来自真实CSS）；`drag-folder.svg`←`icon_folder_g.220b24b0.svg`，`drag-file.svg`←`icon_layer.0c20e889.svg`。目标均在`synapse/macro/`。

## 验证与范围

已执行格式化、两个宏静态提取器的`--check`和新工具的Node语法检查。39项宏资源消费者已静态枚举，新增4项等待主任务统一prepare后校验。统一`cargo check --locked --all-targets`及最终资源结论由主任务记录。

本地数据仍仅为用户明确创建的元数据，不是服务/DLL状态，未执行任何宏动作。事件编辑、palette插入、录制、XML、AI/阶段宏、真实设备分配仍未接入；本轮没有扩展这些入口。

以下细节不应由“菜单已接通”推断为完全同构：

- GPUI输入validator拒绝整段超长输入，原DOM对超长粘贴可能截断。已有超过32单位的外部名称、IME组合与重放输入的边界仍需专项静态处理；本轮没有运行应用验证。
- 嵌套菜单由Base Popover接管焦点。子菜单打开时暂停父选择器的外点关闭，防止原生浮层被父层提前卸载；外部点击先收起子菜单。这是本地浮层适配，不应声称为原DOM完整冒泡行为已复刻。
- 源拖动携带搜索后的`moveData`投影，极端情况下可能省略隐藏后代。本地移动保留真实子树，避免搜索时移动丢失隐藏数据；复制则按源投影创建新副本。
- 删除确认仍使用前轮源profilebar推导的固定锚点；不同文字宽度/空间不足时的最终定位、菜单复制后保持打开的精确时序、scrollbar拖动期间隐藏菜单和onboarding高度动画仍未全部完成。

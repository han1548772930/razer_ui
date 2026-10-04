# 当前 Macro 应用逐项复核

2026-10-04，独立子任务。只用 Acorn 静态 AST、模块导出与 CSS 声明读取，没有执行下载代码或应用。源码来自当前 `.ref/applications/synapse/macro/`；本报告不以旧 audit 中字符串存在作为挂载证明。

## 真实入口、语言加载与历史

`main.3f4b9604.js` 的应用业务代码处于 IIFE `@544312..775435`，不是 webpack 模块 81021。其 `$a @711452..714475` 挂载 `Jn` 页面壳，按 `view` 选择：

- `my-macro` → chunk 8190 / 模块 **58190**。
- `key-binds` → chunk 1700 / 模块 **21700**。
- `help` → chunk 1519 / 模块 **1519**。
- 未知内容 fallback → 58190；页面路由白名单由模块 68511 的 `OB` 提供。

页面壳 `Xn @653566..653844`（别名 `Jn`）默认 `withHeader:true`，挂载 `zn` header，再挂 `#page.main > .wrapper > children`。初始状态 `nc @755691..755938` 的 pages 是 `my-macro/Y.HtJ`、`key-binds/Y.jmi`、`help/Y._$r`。业务闭包 `Y=n(37927)`，模块导出逐级解析分别为 `TEXT_NAV_TAB_MY_MACROS`、`TEXT_NAV_TAB_KEY_BINDS`、`HELP`。

**宏有自己的页面历史。** `zn @648038..653562` 读取 `page.history`，其 `C(view,history)` 在 `activeRecord` 时返回；其他情况下找到 current 索引，清旧 current，截断未来，加入 `{view,current:true}`，通过 `changePageItem` 更新。toolbar back/forward 也在此组件内：起点/终点或录制中禁用，切换相邻项并更新 current。初始 history 只有 URL 指定的有效 view 或 `my-macro`。这不是名为 `tabNavigation` 的设备历史，也没有越界跳到其他应用的源分支。

**旧提取工具查错了语言字典。** `$a` 的实际 `updateLanguage` 用 chunk 8442 / 模块 **18442**；该模块中文映射是 `Promise.all([chunk1080,chunk1250]).then(bind(81250))`。中文专用字典 `1250.ba9a498a.chunk.js` 的模块 **81250** 又显式转出公共中文 46368 中需要的通用键。`18691.Cf` 执行的静态逻辑是加载英语和当前语言模块，`18691.JN` 先当前语言再英语，不存在再回显 key。header props 中另有 7248 公共字典 importer，但不是 `$a` 的实际宏字典加载入口。

完整专用字典：de=81655、en=51971、es=84520、fr=93828、ja=61121、kr=70497、pt-BR=81493、ru=83189、zh-CN=81250、zh-TW=67190。`tools/extract-macro-app-ui.cjs` 仅查公共 `trans-zh-CN` 后认定专用 key 缺失，结论无效；文件内写死的“图标未下载”也已失效。

## 顶栏及两页签

`zn` 实挂载树：

```text
header.nav-wrapper.over-border
  div.toolbar.flex [38px]
    navigation -> back, forward, refresh
    center.title -> TEXT_ADD_MENU_MACRO
    right -> source status widgets, app menu, settings, account
  div.navbar
    Mn profile-bar
    header-links.module-nav -> visible pages except HELP + overflow
    right -> phased-macro tutorial if enabled; Help
  source alert
  phased tutorial when capability + isShowMacroTutorial
```

刷新也是宏源 toolbar 的真实入口：无宏或当前宏已保存则刷新；未保存时打开保存确认，不能悄悄丢弃。`key-binds` 未选中时，宏列表为空或正在录制会加 `header-disabled`；所有切页处理器在录制时返回。Help 独立在右端，不能当第三个居中导航标签。

`main.9ea5d7e7.css`：

| 偏移/选择器 | 有效值 |
| --- | --- |
| 17314 `.nav-wrapper` / 17355 `.over-border` | `background:#222; width:100vw; border-bottom:2px solid #000` |
| 17409 `.nav-wrapper .module-nav` | `height:46px; flex:1 0 max-content; display:flex; align-items:center; justify-content:center` |
| 17634 `.module-nav .nav` | 12px Roboto；默认 `#5d5d5d`；左右 margin 10px；padding 7px 10px；圆角14px；uppercase |
| 17905 hover / 17977 active | hover `#2d2d2d/#ccc`；active `#44d62c/#000` |
| 18050 `.navbar` | `display:flex; position:relative; background:#222; z-index:1` |
| 18121 + 18870 `.navbar .profile-bar` | 先 `flex:1 0 max-content`，后更晚规则覆为 **`flex:1 1 25%`**；左 margin10px |
| 18241 + 18870 `.navbar .right` | `flex:1 1 25%; align-items:center; justify-content:flex-end` |
| 13074 `.navbar .help` / 13185 `.help-icon` | target24×24；icon26×26；JS外层 margin-right10 |
| 116277 `.toolbar .arrow` | 40×38 target；20px图像；禁用opacity .3；hover背景#2d2d2d |

当前 `macro_page.rs:132` 将整个含底边框导航行设为46px，少2px；`surface::navigation_button` 的通用 #999/#111 也不适合宏这组专用颜色。源的 `transition:background-color .3s,color,.1s` 应按真实 CSS 语法解析，不能擅自解释成“文字固定100ms”。

溢出逻辑 `zn` 内 `M @649411..650063`：可用宽度为 `innerWidth - getElWidth(.profile-bar)-30 -24 -getElWidth(.module-nav .dots3)`；按源代码使用 **14px Open Sans** 量文字宽度，再加20px padding与20/10px项间距（虽然绘制文字CSS是12px Roboto）。不能自行把两者合并。overflow `.dots3` 为24×24，菜单 `profile-act size-auto`。

## profile 栏、新建宏、文件夹

`Mn @632162..640788` 实际顺序：26px profile loader → 隐藏文件input → rename-rect（重命名输入与 dropdown `dn`）→ 更多菜单 → 删除确认 → 1×20分隔线 → 新建宏链接和首次提示圆点。

- loader 默认 `profile-default.f608d82c.svg`，先显示500ms spinner；Help用 `profile-unsupported.671bbbc6.svg`。
- `.rename-rect` CSS @53837 宽250px；`dn` 的内部padding左右10px，故实际selector宽230px。`.s3-dropdown @20886` 高27px，14/17px文字，padding4px5px，边框#515151；hover/展开#44d62c；箭头29×25px容器/10px图像，旋转300ms。
- 新建宏文案 `Y.UVl = TEXT_PROFILE_BAR_NEW_MACRO`；链接margin-left10、下划线、capitalize；hover#44d62c，active opacity .7。`Sn @629953..630126` 仅未创建且宏列表空时渲染36px `indicator_animated.b7ce7af4.svg`。
- profile 选择器 `dn @606905..610301`：未选择显示 `TEXT_PROFILE_BAR_DROPDOWN`，不是空白；未完成初始创建时整个selector禁用。首次引导已创建但未完成、录制中、设置窗口中、Help时由上层进一步禁用profile栏。
- dropdown 挂载条件 `show`。`.marcoDropdown @149312` 黑底、#515151边框、min-width340px、min-height200px、padding20px；搜索框27px、搜索15px图像；清空图标、排序label、四项排序dropdown、folderMarco。
- `Bt @600045..600879` 是folderMarco顶部工具栏：左侧 `TEXT_PROFILE_BAR_DROPDOWN_ADD_MACRO`，右侧20×20新宏、新文件夹按钮。新宏先检查未保存，文件夹触发独立add-folder动作。
- 文件夹按钮 tooltip 使用 **`TEXT_NEW_MACRO_FOLDER_TOOLTIP`**（新建文件夹），**`TEXT_ADD_FOLDER`**（文件夹）是生成默认名称所用key，不能混用。源命名函数 `Ys @735326..735912` 最后使用 `Folder N` 唯一名称算法。
- `.navFolder @152803` 顶边#515151、margin上下10、padding上下10；按钮间距10。默认图源为每条规则最后一个 background-image，不能把预加载用的前两次声明当默认态。

按钮图像及状态：

| 元素 | 默认 | hover | active |
| --- | --- | --- | --- |
| 新宏 | `icon_new_marco.6edec51b.svg` | `icon_new_marco-hover.47953e67.svg` | `icon_new_marco-pressed.0faeaad4.svg` |
| 新文件夹 | `icon_addfolder-1.3c65591c.svg` | `icon_addfolder-hover.a804c680.svg` | `icon_addfolder-pressed.aa5ec3ce.svg` |
| 文件夹树 | `icon_folder_close-2.523e657a.svg` | 展开=`icon_folder_open.076142a3.svg` | — |
| 宏文件 | `icon_macro-file.2f24e5a0.svg` | `icon_marco-file-hover.0e086ddd.svg` | 选中同hover |

更多菜单源数组 `Rt @596471..596634`：Add、Import、分隔、Upload、分隔、Rename、Duplicate、Export、分隔、Delete。`Mn` 会按 sharing能力/当前宏/是否可reshare过滤或替换共享项；基本增删改导入导出无需虚构共享能力。

## 我的宏：真实空态与布局

58190 的 export default → `Kr @1403136` → `memo(xr)` → `xr @1403033` → `kr @1402151` connect → `Fr @1398926..1402147`。`Fr` 的主树为：

```text
MacroContent (600px)
  !firstTime.isCompleted && !isOTFMMode -> onboarding Ya 或 qa
  editor (无宏时disable)
    actionbar br (无宏也挂载 {Type:actionBar})
    relative wrapper -> item editor ja
    source confirmation
  MacroMenu Me (绝对定位在编辑器左侧)
```

初始状态 `Co @662155..662586`：所有firstTime flag=false，profiles/macroList为空，currentProfile=""，activeRecord=false。因此空态不是一句技术说明或完全空白：左侧250px功能面板以opacity .7禁用，右侧600px编辑器、54px actionbar和首次引导仍存在。

`8190.5ef5dbbc.chunk.css`：

- `.MacroContent_macro_content__6RXBP @32173`：600px，top20px，left20px/right0，margin0 auto。**只有max-width1120px媒体查询**的@32386才改成margin-left270px/right:auto。后者不能当无条件级联覆盖。
- `.MacroMenu_macro_menu__rXsnP @0`：绝对定位、margin-left **-260px**、250px宽、padding10px 0、#111、圆角5px。故相对编辑器左侧留10px间隔。
- 编辑器内列表 `@32544`：min-height420px，overflow-y:auto；JS `ja @1361495..1365156` 的 viewport height 取`max(450,window.outerHeight-208)`，事件行42px，底部100px drop-space。当前 Rust 已把该 100px 保留区与事件行拖拽排序接入 `macro-item-list`；真实录制与设备传输仍在服务边界之外。
- `.page>.main` 位于主CSS@105729：height `calc(100vh - 85px)`，双向滚动；wrapper默认min-width1280px，**仅max-width900px**查询覆盖为900px。
- actionbar @20588：54px高，padding12px10px，左内边距被@20521!important设12px；背景#111、底边#222；三组checkbox/duration、record、undo/redo/save。无宏时duration文本不挂载，图标仍挂载。
- save按钮@22270：min-width100px、高27px、12/14px、#44d62c/#000、圆角3px；disabled opacity .3、hover#7ce26c。undo/redo各20px，undo右margin16px，使用实际默认/enable/hover SVG。

首次引导 `Ya @1365876..1366194`：300×97，top63px，左右自动居中；padding20px、#111、边框#fd8611、圆角3px、14/17px、box-shadow `0 6px 10px 0 #0003`。文本 `TEXT_MACRO_CONTENT_FIRST`，下方3个6px圆点间隔6px，首点橙色。`qa @1366335..1367547` 是创建后的第2/3步，包含Skip、Onboarding_Step2/3图、NEXT/DONE和indicator；height过渡200ms ease-out。录制步骤引导与实际宏事件录制是不同状态，不能伪造真实录制结果。

## 功能 palette：8项基础组和条件AI组

清单模块 **81021**：`vB -> h @513192`类型；`i -> f @513408..513536` 是3项AI；`JB -> m @513539..513816`是8项基础。真正消费者是58190的 `Me @1227367..1230880`，不是这个常量模块本身。

- 整体禁用：`activeRecord || !firstTime.isRecorded || macroList.length===0 || activeSettingWindow`，opacity .7。
- 基础标题 `R.GMf -> TEXT_ADD_MENU`（添加）。基础项保持delay、keyboard、mouse、macro、launch、command、text、loop顺序。
- AI组位于基础组**前面**，标题 `R.HVq -> TEXT_ADD_TEMPLATE`（添加模板）；仅 `Me` 的AI launcher能力布尔为true才挂载，默认false。该能力源含runtime data和外部checker，复刻可以先接受明确能力输入，禁止为探测能力执行源中的 `new Function`。
- sequence/phased宏禁用基础Delay及全部AI项，单项opacity .5/pointer-events:none。基础其他项仍存在。
- 菜单标题RazerF5/16px/30px高；每项12px/40px高、padding左右10px；图像20×20、右间距12px；hover背景#222并显示8×18拖柄；active背景#44d62c33且隐藏拖柄。

所有下列图像本轮均确认存在于当前源 `static/media/`：

| 类型 | 原key / 中文 | 图像 |
| --- | --- | --- |
| delay | TEXT_ADD_MENU_DELAY / 延迟 | icon_delay_g.5050a3f7.svg |
| keyboard | TEXT_ADD_MENU_KEYBOARD / 键盘功能 | icon_config_keyboard_a.7051c99b.svg |
| mouse | TEXT_ADD_MENU_MOUSE_FUNCTION / 鼠标功能 | icon_config_mouse_o.8a44fbe7.svg |
| macro | TEXT_ADD_MENU_MACRO / 宏 | icon_macro_a.7e1bc94f.svg |
| launch | TEXT_ADD_MENU_LAUNCH / 启动 | icon_config_launch_p.482fbff5.svg |
| command | TEXT_ADD_MENU_RUN_COMMAND / 运行命令 | icon_runcmd_b.10c00024.svg |
| text | TEXT_ADD_MENU_TEXT_FUNCTION / 文本功能 | icon_config_text_b.bc93ac89.svg |
| loop | TEXT_ADD_MENU_LOOP / 循环 | icon_refresh-1_r.ff48f955.svg |
| ai_rephrase | TEXT_AI_REPHRASE / AI 重新表述 | ai_rephrase.ce435691.svg |
| ai_summarize | TEXT_AI_SUMMARIZE / AI 总结 | ai_summarize.953171a6.svg |
| ai_email_composer | TEXT_AI_COMPOSE_EMAIL / AI 电子邮件撰写器 | ai_compose_email.9888530a.svg |

原工具写的 `TEXT_MULTI_FUNCTION` 不是当前两组标题；应删除该假设。`macro_page.rs:169..197` 的“缺图/服务未连接”说明和文件路径清单都不是原产品UI。

## 按键绑定与Help

21700的 default `U @7868..11200`：currentProfile严格为null时返回null；否则获取当前宏deviceList。没有validDevices时挂warning，仍挂载后面的添加卡片，但用相邻兄弟选择器整体opacity .3/pointer-events:none。不能用“去设备自定义页”的自写技术说明代替。

- warning key为拼写如此的 `TEXT_MARCRO_WARING`，图像 `icon_warning.6c0cd78b.svg`，20×17；文字14px，整体居中、margin-top22px。
- 添加卡片文案 `TEXT_ASSIGN_MACRO_TO_DEVICES`，40px `icon_add.a38ffd57.svg`；卡片高230px、#222、2px dashed #5d5d5d、圆角5px、padding8px20px；hover边框#44d62c。
- 真正分配弹层 `M @5190..7812` 条件是showPanel && validDevices.length>0；选择设备后挂该产品 `displayMode=macro` 分支iframe。应接对应产品分支，不能把普通customize页面当成同一分支。
- 1519 export default→`R`→connected `C` 的Help内容是一个Support widget和外链，key由37927的YHZ/L2X解析；URL来自68511.Nu：`https://www.razer.com/search/macros?sel=support`。

## 可直接使用的中文文案与字面量位置

以下位置均为 `1250.ba9a498a.chunk.js` 模块81250，按该模块export getter定位到局部binding，未全文件搜索同名符号。

| key | 原文 | 字面量偏移 |
| --- | --- | --- |
| TEXT_NAV_TAB_MY_MACROS | 我的宏 | 11577 |
| TEXT_NAV_TAB_KEY_BINDS | 按键绑定 | 11585 |
| TEXT_PROFILE_BAR_MACRO | 宏 | 11594 |
| TEXT_PROFILE_BAR_NEW_MACRO | 新建宏 | 11600 |
| TEXT_PROFILE_BAR_DROPDOWN | 没有宏 | 11608 |
| TEXT_PROFILE_BAR_DROPDOWN_ADD_MACRO | 宏 | 11616 |
| TEXT_NEW_MACRO_FOLDER_TOOLTIP | 新建文件夹 | 11631 |
| TEXT_PROFILE_BAR_S3_DROPDOWN_ADD | 添加 | 11641 |
| TEXT_ADD_MENU | 添加 | 11740 |
| TEXT_ADD_TEMPLATE | 添加模板 | 15517 |
| TEXT_ADD_FOLDER | 文件夹 | 12305 |
| TEXT_MACRO_SEARCH | 搜索 | 11962 |
| TEXT_MACRO_SORTBY | 排序方式 | 11969 |
| TEXT_MACRO_CONTENT_FIRST | 你尚未创建任何宏。创建一个宏以开始。 | 11840 |
| TEXT_MACRO_CONTENT_SECOND | 按此按钮开始录制连续动作。单击下拉按钮即可看到更多录制选项。 | 11863 |
| TEXT_MACRO_CONTENT_THIRD | 或者，你可以通过点击动作或从“添加”面板中拖放动作的方式手动插入单个动作。 | 11898 |
| TEXT_MACRO_CONTENT_SKIP / NEXT / DONE | 跳过 / 下一步 / 完成 | 11940 / 11947 / 11955 |
| TEXT_MARCRO_WARING | 你尚未连接 Razer 雷蛇输入设备。请先插入 Razer 雷蛇输入设备 | 12026 |
| TEXT_ASSIGN_MACRO_TO_DEVICES | 将宏分配给设备 | 14006 |
| TEXT_ACTION_BAR_RECORD / TEXT_LAUNCH_SAVE | 录制 / 保存 | 12067 / 12995 |

建议把完整宏专用字典放进独立 `MACRO_SOURCE` 命名空间，避免与Dashboard公共文案互相覆盖。

## 源哈希与本轮限制

| 文件 | SHA-256 |
| --- | --- |
| main.3f4b9604.js | fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d |
| 8190.61506f5b.chunk.js | 003b116007bb415462e5175c09fc84928d2a20c8d1a7c73113a432643d2cbe9f |
| 1700.a2780a35.chunk.js | 817af243f7964927bb63ff9ce11c26cef4535b770013a541daa8852a80c8415c |
| 1519.a5a62cfd.chunk.js | 4b1493116c8597e44b6d9661070da26d574e16a77437c92ff7bc0b3ab48c082f |
| 8442.e66ce60e.chunk.js | b62154d4ca5a0d9a5b242e8257db350deb0afeb395839a0f7b969977cd44343d |

本报告完成后，主任务授权同一子任务实施已证实的呈现和本地用户创建状态；设备/服务/后端/DLL接入仍由主任务统一安排。此处不是对所有宏编辑器、录制、播放、导入/导出或最终像素的验收声明。

## 本轮实施交接（2026-10-04）

主任务集成检查补充：`cargo check --locked --all-targets`最终通过，无警告。此前BoxShadow、边框、旋转和Popup API错误已修正；菜单使用实际Base Popover管理关闭与焦点。35项静态/展开动态资源引用均有效。未运行应用或测试。

- `src/shell/macro_page.rs` 与 `macro_page/{chrome,body,state}.rs` 替换了旧技术说明占位。接入独立两页签、Help、页面历史接口、真正的无宏初始布局、原资源palette/actionbar、三步教程、无有效设备的按键绑定分支和Support链接。
- 本地显式创建的宏/文件夹保存在该页面实体内存中；宏选择、递归搜索、四种排序、重命名、宏复制、宏删除/末宏回我的宏生效。初始不伪造宏或设备；宏标准名称仍使用源代码的`Macro N`/`Folder N`。未将这种本地元数据操作声称为原服务接入或持久化。
- `tools/extract-macro-locales.cjs` 将实际18442语言加载器指向的十个专用模块静态解析到`MACRO_SOURCE`命名空间，保留各语言真实省略项，并通过现有英语回退；当前每语言343–350项。来源与哈希见`macro-locale-source.json`。工具只解析AST，未执行参考JS。
- 原SVG由主任务资源管道提供到`synapse/macro/`。SMIL indicator复用已按同哈希SVG实现的`TutorialIndicator`，selector箭头保留300ms旋转，菜单100ms淡入、删除提示300ms淡入。
- 子任务执行了`rustfmt --edition 2024 src/shell/macro_page.rs`、`node --check tools/extract-macro-locales.cjs`、`node tools/extract-macro-locales.cjs --check`；未运行应用、构建、测试或安装器。统一`cargo check --locked --all-targets`由主任务负责，不能把仅格式化通过称为类型检查通过。
- 首轮集成检查发现的`BoxShadow.inset`、边框与图像旋转API和弹层类型错误已按当前gpui-base接口修正。三个弹层使用受控`base::Popover`，没有重复trigger切换或手写菜单外点关闭；其Escape/外点关闭和焦点恢复由库生命周期负责。静态核对35项直接/展开后的动态资源引用，全部在`assets/`存在；修正了原临时`palette-*`/`assign-add`名称与真实资源路径不一致的问题。最终类型检查结论以主任务后续`cargo check`为准。

### 仍需逐项补齐，不能据此声明Macro全部复刻完成

**后续更新：**文件夹/宏树菜单、递归复制/移动/删除、搜索排序、名称限制与placeholder已在下一专项继续实施；该专项还撤销了本报告实施时部分错误推断（尤其菜单复制实际使用25572.iV，而非N）。以[`macro-metadata-review-2026-10-04.md`](macro-metadata-review-2026-10-04.md)及其新AST/CSS收据为准。下面清单保留为前轮交接历史，不代表这些条目目前全都未实现。

1. Real recording, recording options, XML import/export, complete folder menus, and independent device binding remain service-boundary work. Action parameter editors for Delay, Keyboard, Mouse, Loop, Text, Command, and Launch, plus palette/event-row drag reorder, are implemented as local draft UI with undo/redo; they do not call device or recorder services.

## 2026-10-04 event parameter editor handoff

The Rust Macro page now stores each local event as `ActionItem { kind, value, secondary_value, number_min, number_max, state }`, extending the previous enum-only rows. This is a service-free static subset of the current stable 8190 chunk module 58190 `CustomInput`: the source selects editors by `item.Type` for Delay (`Number`), Keyboard (`KeyEvent`), Mouse (`MouseEvent`), Text (`Text`), Command (`Content`), Loop, and Launch (`Content0`/`Content1`).

- Delay starts at `0.000`; the local editor accepts the source `0..99999.999` range with three decimal places.
- Keyboard uses a local display editor because the source `ShortcutKey` capture depends on native input capture; Mouse and Loop expose their source-shaped local controls.
- Text uses a source-sized modal with an explicit Save/Cancel flow and a 250 UTF-16-unit limit. Command uses the source-width input. Launch keeps separate `Content0`/`Content1` targets and a local Program/Website selector; file picking and launch dispatch remain unavailable.
- Parameter edits share the existing local undo/redo snapshots and Save draft state.
- No value is sent to a device, recorder, mapping engine, or service; real key capture, mouse movement, macro lists, and transfer remain backend work.

Evidence: `8190.61506f5b.chunk.js`, module 58190, `En` dispatches by `item.Type` to `St/yn/je/dn/Ot/yt`; `yn` is the keyboard editor, `dn` the text editor, and `Fr` mounts `item_editor`/`MacroItem`. Verification remains formatting, `cargo check --locked --all-targets`, and static parsing only; the app, downloaded scripts, DLLs, installers, and test programs were not run.

## 2026-10-04 action row reorder handoff

Static review of current stable chunk `8190.61506f5b.chunk.js`, module 58190, confirms that each `.functional_item` sets a drag payload (`dragType` and source `index`), paints a two-pixel green dashed insertion line on drag-over, and dispatches `{ type: T9, index: currentIndex, dragType, indexMoveItem: sourceIndex }` on drop. The Rust editor now mirrors that interaction with a page-scoped `ActionDrag` payload, GPUI `on_drag`/`drag_over`/`on_drop` handlers on every event row, and a green insertion border. `MacroPage::move_action` inserts before the target row, records undo history, clears stale selection/edit state, and invalidates redo snapshots. No recorder, device, or service call is introduced.

The local Delay input follows source `InputNumber`'s `max=99999.999`; this remains a display-only draft value until a future service-backed persistence path is authorized.

## 2026-10-04 CustomInput fidelity correction

A second static pass over current `8190.61506f5b.chunk.js` module 58190 and `8190.5ef5dbbc.chunk.css` corrected the local editor contract:

- `CustomInput` dispatches Delay to `InputNumberCustom` (`Number`, step `.001`, clamp `0..99999.999`), Loop to `InputStepper` (`Number`, clamp `1..99999`, plus `LoopEvent.State`), Mouse to `CustomDropdown`, Keyboard to `ShortcutKey`, Text to the 250-character `CustomModal` textarea, Command to the 200px `InputText`, and Launch to the Program/Website modal with `Content0`/`Content1`.
- The Rust draft now stores `ActionItem { kind, value, secondary_value, state }`. Loop count and loop state are separate; arbitrary text, command content, and launch targets are rendered verbatim instead of being passed through the locale translator.
- Mouse uses the source's ten fixed dropdown keys (`TEXT_LEFT_CLICK`, `TEXT_RIGHT_CLICK`, `TEXT_SCROLL_CLICK`, `TEXT_MOUSE_BUTTON_4/5`, `TEXT_DOUBLE_CLICK`, `TEXT_SCROLL_LEFT/RIGHT/UP/DOWN`) in a local Popover. Keyboard, Delay, Command, Launch, and numeric Loop count use local editors; Text uses GPUI `Textarea` with the source 250-character boundary. No key-capture, file picker, recorder, or service dispatch is fabricated.
- Palette entries now carry the same page-scoped drag payload as event rows. Dropping a palette item inserts a freshly initialized action before a row or at the trailing drop space; moving an existing row remains undoable.

Static CSS receipts used for editor geometry: `.CustomDropdown` 160x27, `.InputNumberCustom_*` 88x27 (randomized min/max 60px), `.InputStepper_*` 88x27, `.InputText_*` 200x27, `.CustomModal` 250px with 210x96 textarea, and `.ShortcutKey_display_name` 210px minimum/27px.

The Text popup now follows the 250px container, 210x96 textarea, uppercase heading, count, and explicit Save/Cancel flow; the source close SVG was prepared byte-for-byte from current `close.1d7eff2a.svg`. Enter remains a multiline edit. The local limit and count use UTF-16 units as HTML `maxLength` and JavaScript `length` do. Source emoji categories and character-map launcher are still missing. Launch now presents a 250px local Program/Website radio-style modal with separate `Content0`/`Content1` text targets and source input widths; the native file picker remains deliberately unavailable. Delay random mode keeps local min/max fields with source 0..5 bounds and two-decimal draft validation. Loop and numeric Delay editors use 88x27 right-side stepper arrows. Keyboard capture and nested Macro selection remain unfinished. The preceding editor work is therefore a local subset, not a full Macro replica acceptance claim.

Verification after these edits: file-level `rustfmt --check`, `cargo check --locked --all-targets` (no Rust warnings), and scoped `git diff --check` passed. No app, test program, installer, downloaded JavaScript, or DLL was executed.

## 2026-10-04 numeric and Launch follow-up

`tools/audit-macro-editors.cjs` parses current module 58190 with the maintained `webpack-source.cjs` Acorn reader and records `St` (Delay), `yt` (Loop), `rt` (Launch), `z` (Program/Website radio list), and `En` (mounted CustomInput branches) in `macro-editors-current-audit.json`. The receipt includes module byte offsets, current JS/CSS SHA-256, actual mounted CSS rules, dictionary export resolutions, and byte-equality checks for stepper-up, stepper-down, and close SVGs. It does not evaluate downloaded JS.

- Fixed Delay edits use an 88x27 local number input with .001 steps, 0..99999.999 clamping, and three decimal places. The source mounts an HTML `type=number` input; GPUI uses `NumberInput` to supply the equivalent arrows. Loop count also uses an 88x27 editor, unit steps, and 1..99999 bounds. Existing local Loop state remains separate.
- Randomized Delay exposes two 60x27 draft inputs, permits empty partial drafts, restricts numeric text to three integer digits/two fraction digits and a maximum of 5, and follows the source crossing rule: changing minimum to `>= maximum` or maximum to `< minimum` adjusts maximum to `min(minimum + 1, 5)`. Blur between the pair keeps both editors open; leaving the pair/Enter commits one undo snapshot. Source uses `profile.delaySetting == 2` and an object-valued `Number` to mount this branch. The local per-action mode button makes that subset reviewable without fabricating a service profile or recorder setting; its position and mode ownership differ from the original.
- Launch now uses the source 250px popup/padding/title/close geometry with Program/Website radio choices, 142px program and 164x27 website fields, and the 30px field inset. Cancel/close/outside click discard the popup draft; Save records both content values and mode as one undo snapshot. The program field accepts an explicitly typed local path. It does not impersonate the source `showFileOpenDialog`, open a file selector, execute a program, or browse the website. A file-selection button/icon and service-backed selection still require integration.
- New action insertion, row reorder, undo/redo, deleting actions, and switching/creating/deleting macros clear old editor indices and popup drafts. A stale launch/range edit additionally checks the current macro/action kind before it can be committed.

Remaining Macro scope includes source emoji categories/character-map launch, real keyboard capture, nested Macro choice/cycle prevention, recorder options and profile-wide delay-setting ownership, import/export, device bindings, and service persistence. This static/local follow-up does not claim that Macro as a whole or all pixels are complete.

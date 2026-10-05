# Macro 文本与 emoji 弹层：当前源码续接

2026-10-05。仅使用当前 `.ref/applications/synapse/macro/`，没有读取历史前端/宿主目录，也没有执行应用、参考 JavaScript、安装器或 DLL。

## 来源

`tools/audit-macro-text.cjs` 用维护中的 Acorn/CSS 解析器提取 `8190.61506f5b.chunk.js` 模块 **58190** 的 `dn/on/en/an/tn/$t/Gt/Pt/vt/ze`，解析模块 **37927** 的文案键、模块 **4173** 的 `updateMacroItem`，并逐字节核对当前 Macro SVG 与已嵌入资源。机器收据见 [macro-text-current-evidence.json](macro-text-current-evidence.json)。该收据记录当前源码、CSS、资源与本地实现的 SHA-256；指纹不是运行验收。

`text_data.json` 从 Macro 自己的 `wt/ft/Lt/Ut` 提取，包含 **1,703 个分类项、1,936 个英文搜索项、230 组变体**。没有把 Dashboard MapText 数据当作 Macro 实现依据。9 张共用 SVG 与当前 Macro 原文件相同。

## 已实现行为

- `text.rs` 替换原 Text Popover，接入 250px 容器、210×96 textarea、20px 工具图标、计数、100px 最小宽度的源按钮，以及单独的文本草稿。
- **只有文本弹层自己的 Save 提交动作。** Cancel、close、外点、切换编辑器或外层保存均丢弃尚未确认的文本。文本 Save 更新当前宏动作并生成一次 undo 快照；Macro 顶部 Save 再保存共享文档库。
- 输入与计数使用 **250 UTF-16 单位**。复用本地已审计的插入区截断器，超长粘贴保留未替换的后缀；emoji 按 `dn` **追加到末尾**，以原完整内容长度检查容量，不替换选择区。
- `dn` 的 `O -> N` 会关闭并禁用 Save；trigger 的 `document.body.click()` 也走这条链。因此重新打开而没有修改时保持禁用。后续非空 draft 变化启用 Save，清空没有禁用分支；输入过非空内容后可保存空字符串。早期交接中“打开已有非空内容始终启用”的解释已修正。
- emoji 面板是 **430×337**，header 35、search 40、main 260。分类顺序及位置来自 `on.E`；点击分类修改活动标签并按 `group.offsetTop - 80` 滚动，滚动本身不切换活动标签。
- 搜索对英文名称忽略大小写。**保留源 clear-search 行为：只清空可见输入，不清除已有过滤结果**，直到下一次输入变化；这是 `resetEmoji:()=>{}` 的实际结果。
- `on` 的首次分类载入延迟 30ms。未接入 `getWindowVersion` 时保持未知值；只有字面量 `"11"` 才过滤 `Ut` 的五个 cat glyph，没有伪造系统版本。
- 使用一层 **500ms linear** 的 emoji 透明度过渡；提示为立即可见、**300ms linear**，没有 Dashboard 的一秒延迟。关闭时立即隐藏，保留透明度采样以处理快速反向切换。
- 字符映射表图标连接既有 `backend::system::open_character_map()`，只在未来实际点击时调用；本批没有启动它。原源调用经 WinLauncher 转发，本地直接使用既有系统适配器。

## 定位与源样式缺陷

`text_overlay.rs` 用当前帧 trigger bounds 计算 `x = left`、`y = bottom + 10`；若 `y + 250 > innerHeight`，使用 `top - 250`。原始 250 是源代码固定阈值，不替换为实际面板高度或通用 viewport flip。滚动后保留 150ms 防抖，检查 `trigger.top + 8` 是否超出动作编辑器。原生适配通过 trigger 几何变化观察滚动，因此也覆盖滚动条拖动；布局本身改变位置时同样更新，这是适配方式的额外情况。

emoji wrapper 的源 `top:184px` 从 modal padding box 计算，原生坐标包含 1px modal border；未设置 left 时沿内容起点。变体的 left 与 top 条件照 `en`，进入时确定相对偏移，滚动后随 cell 移动；绘制/命中裁剪保留 main 的 scroll viewport。

分类标题实际使用字面量 `emoji_popup_main_label`；变体子项使用字面量 `emoji_popup_main_item`。当前 manifest 的全部 CSS 中这两个选择器均不存在，只有 CSS-module 改名后的规则。工具会检查不存在匹配规则这一事实。因此标题继承上层文字样式，变体为自适应宽度的 Roboto 14/16 子项，没有擅自补成 40px 的 Segoe UI 格子。普通 emoji cell 才使用真实匹配的 40×40 样式。

没有复刻源 cleanup 再次注册 `macroRecordingEvent` 的监听泄漏。真实录制服务仍未接入。

## 验证和边界

已通过格式化、专项源数据/资源提取校验、嵌入 JSON 和语言键校验、资源校验及 `cargo check --locked --all-targets`。最新嵌入数据检查为 34 份，31 份结构校验、3 份既有跳过、0 失败；语言校验为 442 个字面量键、42 个 soft 键、51 个动态调用，字面量缺失为 0。资源校验为 1,112 个主资源和 35 个服务 SVG。检查仅余既有 `customize_page::layer_button` 未使用警告。

未运行应用或测试；不能据此声称浏览器内联基线、原生文字度量、IME、焦点、缩放、滚动条、动画与像素已经实际验收。钢笔编辑光标仍未接入。每行的搜索/分类/滚动使用本地 document/index 状态，原 DOM 在文档切换、行重排时的组件复用边界尚无运行验收。Macro 的录制器选项、Sequence/Phased 完整创作、XML、原生执行与产品服务等整体缺口仍保留。

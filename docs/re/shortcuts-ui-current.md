# Global Shortcuts 当前界面契约

当前来源为 Dashboard `4608.e973916f.chunk.js/94608`、lazy Map 组件和 `55.4e8559cb.chunk.css`。`Fe` 的列表 `Oe → Te → be` 与共享映射根 `se → ie` 为兄弟节点。源、CSS、数据、资源收据见 [shortcuts-current-ui-evidence.json](shortcuts-current-ui-evidence.json)，维护工具 `audit-shortcuts-current.cjs` 只静态解析下载文本。

## 列表、录入与保存

`shortcuts_view.rs` 提供 600px widget、标题/说明、顶部 Add、底部添加框、输出类型和值、临时空行、输入录制、冲突警告和 More 菜单。行透明 1px 边框、选择绿框、白色 10% hover；输出区宽 58%，类型 10px/`#6a6a6a`。空行使用源 59×9 与 128×15 矩形和各自 margin。

输入字段宽 190、min-height27，padding5/22；普通/监听/冲突边框按源灰/绿/橙。录入有 100ms 延迟，保持监听直到失焦，排除源 Pause/Cancel/Application/PrintScreen/ScrollLock/NumLock 等输入。右/中/后退/前进鼠标使用当前四个 inputID；名称取源目录和语言键。普通 GPUI 键事件只提供修饰键族，未重录的左右细分持久值保留；不冒充设备 inputredirect 或 Hypershift 观察。

More 为 26×26 点击区、20px 图、margin-left20；菜单 150×85、right-124/top27，≤850 时 right0，Edit/Duplicate/Delete 行高27。外点排除触发器。Duplicate 复制输出、新增身份、清空输入/修饰键，不另开输出编辑器。冲突保留在行上，警告20px，提示300×49。删除确认定位行内 left274/top53。

源先保存输出再录入输入，因此 `validate_stored_shortcuts` 允许空输入和重复 chord，但验证输出、身份与非空输入结构；空输入必须无修饰键。严格 `validate_shortcuts` 和纯 encoder 仍拒绝空输入/冲突，编码器无生产传输调用。UI 保存是本地工作区保存，未注册或执行原生快捷键。

## 映射根与类别

`shortcuts_mapping.rs` 使用 292px 根、36px header、250px body、40px 收起 rail 和200ms展开。实际生效定位为 `.body-wrapper .key-config.open.right`：left `50%+296px`、top142、min-height486；≤1220 为 `50%+110px`，≤824 为 `50%`。兄弟节点不匹配 `.custom-global-shortcuts` 后代规则，inline132 也不覆盖 `top142!important`。原生绝对定位不使用通用 popup 的视口钳制。

八类顺序取 `21368/E.functionList`：

| 类别 | 当前状态 |
| --- | --- |
| Device Profile | 空设备选择与五个禁用配置动作；ready-device/subdevice/single-profile 过滤及真实配置尚未适配 |
| Device Sensitivity | 源空设备及禁用初始 DPI 显示；40489 阶段/XY/滑条和设备配置连接尚未完成 |
| Switch Lighting | 空 Chroma profile 目录；Configure 进入实际 Chroma Studio 实体 |
| Macro | 消费共享持久 MacroLibrary，包括空宏、名称更新和失效引用；本地身份不当作服务 GUID |
| Text | 源 textarea、UTF-16 边界、emoji/search/variants、字符映射表入口 |
| Launch | 程序/网站独立字段和原 radio/文件选择结构，迟到 picker 受代际与身份保护，不执行所选程序 |
| Multimedia | 当前源有序10项，使用本地输出模型 |
| Windows Shortcut | 当前源有序26项，使用本地输出模型 |

Macro 普通播放为 Once/NTimes/ContinuousToggle/Queue，排除 ContinuousHeld；Sequence/Phased 使用各自集合。NTimes 为1–99，其余保存次数2。失效非零引用保持失效并禁止保存，不自动改绑；仅空库哨兵0可在首次出现文档时选首项。详见 [共享宏契约](shortcuts-macro-current-contract.md) 与 [宏界面](macro-ui-current.md)。

## Text 与 emoji

当前 `MapText/34198` 提供 210×96 textarea、250 UTF-16 限制、两个20px工具按钮和计数。超长插入只裁剪超额插入段，保留未替换后缀和光标。emoji 插入保留选区，但源容量判断先使用整个旧内容长度再加 emoji 长度；普通 Enter 抑制，粘贴依同一限制。

数据保留1703分类项、1936英文搜索记录、230变体和源重复/组合序列。430×337 popup 包含35 header、40 search、260 scroll、40×40 cell；按当帧 textarea/tool bounds 定位，≥600 且右侧溢出时左翻，不竖向钳制。八分类/搜索/clear/十列方向键及返回分类时200ms空列表按源处理，代际防止迟到 clear 覆盖新输入。

源分类组件收到 `p:true` 却读 `p.label`，因此分类标题为空。未观察 Windows 版本时采用该组件 null/Windows11 分支，排除五个 cat glyph。它与 Macro Text 的版本条件和末尾追加行为不同，不合并为同一规则。

外内两层 wrapper 各做500ms linear opacity，合成 alpha 为单层进度平方；关闭立即隐藏，快速重开可反向采样。提示与变体分别在显示/隐藏延迟1秒并300ms淡变；toolbar tip bottom-38、header top100%。减少动画绕过过渡。字符映射表按钮发事件至本地 `charmap.exe` 入口，审计没有实际调用。

## 仍需完成与验证边界

Device Profile/Sensitivity 的非空真实输入、Chroma profile 服务、部分 subtype 行、原生输入重定向/全局注册仍缺。录入/dots的200ms边框、菜单300ms背景、删除确认全部细节、窗口可见性边界、browser normal行高、焦点/滚动/变体快速搜索边界和字形仍需验收。

工具栏精确关闭图 `icon_close.130e45fb.svg` 当前缺失，使用已登记共享 close 图；不得记为原图对齐。当前路径、缺失项和离线查找证据保留在 [媒体证据](shortcuts-service-media-offline-recovery.json)。其他已准备控件/rail PNG/SVG 都由当前源哈希与字节比较核对，不需要重复下载。

现有真实控件测试、UTF-16 边界和映射数据验证只参与允许的类型检查；未运行应用、测试、下载 JavaScript 或 DLL。源收据及 native 指纹不能替代运行验收。

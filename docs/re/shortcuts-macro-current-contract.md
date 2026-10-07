# 当前 Macro 文档与全局快捷键契约

证据仅来自当前 `.ref/applications/synapse/macro/` 与 `.ref/applications/synapse/dashboard/` 的 manifest 声明文件。维护工具为 [audit-shortcuts-macro-contract.cjs](../../tools/audit-shortcuts-macro-contract.cjs)，完整 AST 原文、偏移、SHA-256 和 CSS 条件见 [机器收据](shortcuts-macro-current-contract.json)。工具只将厂商 JavaScript 解析为数据，没有执行它。

本文记录源码契约与本地适配要求，不代表所有相应 UI、服务或运行时行为已经完成。

共享本地文档库、保存/放弃提示与嵌套宏选择的当前状态见 [Macro界面契约](macro-ui-current.md)；全局快捷键实际挂载及未完成项见 [Shortcuts界面契约](shortcuts-ui-current.md)。58190/38162/97278/25572 AST和CSS保留在机器收据中；取证时Rust文件指纹不表示运行验收。

## 文档身份、草稿和持久化

当前 Macro `main.3f4b9604.js` 的 `25572.OM/H` 通过客户端 `uuidv4()` 新建文档。字段包括 `guid`、`name`、`macroPlaybackOption`、`macroType`、`delaySetting`、`macroEvents`、`appEngine`，另带编辑期 `saved`、`actionLoad` 与 `deviceList`。这个 GUID 不是由本机硬件或服务发放的身份。

新文档已经 `saved:true`，`appEngine.events` 为空；`macroEvents` 只有 `actionBar`。`macroList` 保存每个文档的活动草稿，`savedMacros` 分别保存各文档的已保存副本。切换当前文档只修改 `currentProfile`，不会清空其他文档的事件。

`25572._G/$` 仅将当前文档复制到同 GUID 的 `savedMacros` 槽位。`25572.i$/W` 从该文档的已保存副本恢复当前草稿，并清空其撤销/重做栈，保留当前 `deviceList`。`Js` 从 `savedMacros` 构造持久化数据，删除 `actionLoad`、`deviceList` 和 `saved`，而不是持久化所有活动草稿。

存储契约如下：

| 层 | 当前源码事实 |
| --- | --- |
| IndexedDB | `open("macros", 1)`；object store `macros`；`keyPath:"guid"`；唯一 GUID 索引 |
| 协作锁 | `navigator.locks` 名称 `macros-idb`；读取 shared、替换 exclusive |
| 替换与通知 | 同一事务先 clear 再 put 每份文档，等待事务完成后发 `macros-change-channel`，内容为 `{oldValue,newValue}` |
| 编辑器配置 | localStorage `macroEditorConfig`；包含 `currentProfile`、`firstTime`、`folderNames`、`profiles`、`recordProfile` |
| 引擎数据边界 | `qs` 经桥接接口写 `synapseMacros` 或 `synapsePhasedMacros`；不等同于本地 JSON 写入 |
| 重新载入 | `gr` 从存储重建 `savedMacros` 与 `macroList`，初始化已保存状态与空撤销栈；没有按事件数量过滤文档 |

本地工作区可采用持久单调 ID，但必须保存高水位，删除文档后也不能重用其身份。它应保持为本地引用，不能冒充 Synapse GUID 或已被原生引擎接受的数据。宏页显式保存后的文档与快捷键应进入同一工作区快照；异步写入完成只能确认实际写入的版本，不能顺带清除写入期间发生的新编辑。

## 未保存变更与复制

当前 Macro 业务闭包的 `Mn`、`Bt`、`an`、`zn`、`Gr` 及 reducer `Zs` 给出下列区别：

| 操作 | 当前源码行为 |
| --- | --- |
| 新建宏 | 没有当前文档或当前已保存时立即新建；否则保存 `suspendedAction={type:add}` 并打开保存提示 |
| 选择另一宏 | 当前草稿未保存时挂起 `setCurrentProfile(targetGuid)`；文件夹点击只展开 |
| 新建文件夹 | 直接添加，不挂起当前草稿 |
| 顶栏复制 | 无未保存确认；递归复制活动文档的内容，将副本标为已保存；文件副本成为当前文档 |
| 树菜单复制 | 同样复制活动文档且无未保存确认，但没有 `selectDuplicatedItem`，不改变当前宏 |
| 刷新 | 当前未保存时挂起 `refreshWindow({saveMacro:true})`；重载从已保存文档重建编辑状态 |
| Macro 内页前进/后退 | 有录制限制，没有以事件草稿为条件的保存提示；内存文档仍保留 |

复制用的是 `macroList` 活动内容，而非 `savedMacros`。因此顶栏复制后必须仍保留原文档的未保存草稿，返回原文档时能继续编辑。不能把复制操作改成静默保存原文档，也不能因为选中了副本就丢掉原草稿。

`Gr` 弹窗精确行为为：

- 标题 `SAVE_MACRO`，正文依次为 `SAVE_REMAPPED_BUTTON_MSG1`、两个换行、`SAVE_REMAPPED_BUTTON_MSG2`。
- 左按钮 `DONT_SAVE`：先 `discardMacroChanges`，再关闭提示，最后派发捕获的挂起操作。
- 右按钮 `SAVE`：先 `saveMacro`，再关闭提示，最后派发捕获的挂起操作。
- 关闭图标只隐藏提示；`Zs` 在隐藏时清除 `suspendedAction`，保留草稿。源 UI 没有第三个取消按钮。

CSS 为 400px 宽，20px/30px padding，`#111` 背景，1px 绿色边框，5px 圆角，水平垂直居中。标题居中，正文使用普通左对齐；按钮最小宽 100px、高 27px，按钮组上间距 20px。背景为半透明黑色，源码声明 100ms linear opacity。关闭图精确来源 `icon_close.4f578909.svg`，本地已经注册的同字节资源为 `synapse/macro/binding-close.svg`。焦点限制可通过本地弹窗生命周期实现；不能把它报告成已经调用 `setForceFocus` 宿主服务。

## 全局快捷键的宏映射

当前 Dashboard `MapMacro.ffac22d2.chunk.js / 82508` 的 `getPlayBackSet` 按文档 `macroPlaybackOption` 选择播放集合：

| 文档类型 | 全局快捷键播放 ID |
| --- | --- |
| 普通 | `Once`、`NTimes`、`ContinuousToggle`、`Queue` |
| `sequence` | `Sequence` |
| `phased` | `Phased` |

普通全局快捷键排除 `ContinuousHeld`。构造函数短暂排除 Queue，但挂载后的集合由 `getPlayBackSet` 替换，最终仍包含 Queue。不要依据构造函数的中间状态删除最终队列选项。

`NTimes` 显示 `NUMBER_OF_TIMES`，步进范围 1–99、步长 1、两位整数、允许输入时更新。保存字段为 `{name,guid,macroPlaybackOption,repeatCount}`；`NTimes` 使用所填次数，其余选项写 2。这些原生字段用于解释源码，不能直接将本地 ID 填入 `guid` 发给原生服务。

Dashboard `59007/G` 设置 `macroAvailable = payload.length > 0`，不检查 `macroEvents` 或 `appEngine.events`。因此空宏也是真实可选文档。没有文档时插入空白显示哨兵，播放禁用，显示 `CREATE_MACRO_MSG`；只有一个文档时通用下拉禁用展开，但显示该文档。`CONFIGURE_MACROS` 在源码中以模块安装状态为条件，本地实际已实现的 Macro 入口可直接导航。

`3844` 通用下拉在选中索引 -1 时不渲染，选项打开高度为 `min(180, 25 * 数量 + 2)`，根据视口空间向上展开；`97278` 选项为 25px 高，普通文字可截断，选中为绿色。播放下拉限制选项列表宽度为触发器宽度。

## 需要显式区分的源码缺陷与本地处理

1. `render` 会原地按名称排序宏列表，选中项却保持数值索引；数组替换时也没有恢复文档身份。缺失 GUID 会得到索引 -1。为避免本地持久引用误指向别的宏，应按 ID 保持选中；删除引用目标后保留失效身份，禁用保存并要求实际选择，不能自动替换为第一项。
2. `3466.eh` 不是标准 locale 字母序。它按特殊字符排名比较第一字符，第二字符方向相反，无法排名时使用 JS 字符串比较。收据保留完整比较器；不要宣称常规 locale 排序与之相同。
3. 文档类型变化分支错误地使用 `AX$.SYNAPSE.SEQUENCE`，但 `AX$` 实际是延迟选项，其成员没有 SEQUENCE。正确类型表在 `CKm`。此错误与 `getPlayBackSet` 的明确最终集合分开记录，本地根据真实类型选择合法播放集合。
4. UI 草稿、工作区已保存文档、磁盘写入确认、原生执行是四种状态。此专项没有执行厂商代码、应用、测试或硬件命令，也没有证明真实宏播放已经接通。
5. 本地 ID `0` 仅表示尚无文档的占位状态，首次有宏时选中首项；失效的非零 ID 保持失效。正常单选列表按源禁用展开，但失效引用仅剩一个候选时允许展开修复。载入映射的播放方式必须属于对应文档类型；非 `NTimes` 保存时统一写 2。临时空白或减号次数禁止本地类型化保存，这是本地数据适配规则，源 `changeTimes` 本身不校验。

静态复核命令：`node tools/audit-shortcuts-macro-contract.cjs --check`。

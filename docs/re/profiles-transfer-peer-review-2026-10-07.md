# Profiles 导入导出独立复查（2026-10-07）

复查对象：`profiles_page/transfer.rs`、`transfer_view.rs`、`transfer_codec.rs`、`devices.rs`，并对照当前 `43/Ia/Da/Ea/Ua/Aa/Na`、`1867/D` 和 stable JSON/MD5 原始模块收据。此处没有运行测试或应用。

## 发现与修复回读

1. **逐配置的宏选择曾在提交时丢失。** 当前 `Ia` 的 Import 回调传递选中的完整行；`Ua` 的 `selectedProfileData` 保留每行 `listItems/isMacroSelected`，而 `selectedMacroData` 是空数组。最初原生 intent 仅保留原始 profile 和所有行所选宏的并集，无法区分同一宏在 A 行选中、B 行取消。已回读父任务修复：现在 profile/name/isProfileSelected/hasWarning/fileType/listItems 全部保留，各宏 guid/text/isMacroSelected 随行持有；union 数据仅是附加数据，未伪称源码消息已发出。
2. **Dynamic Keystroke 过滤须遵守 JS 真值。** `Da` 使用 nested `dynamicKeyStrokeGroup` 真值判断；原先仅识别 null/false，漏了 0 和空字符串。最新 codec 已有 `js_truthy`，独立回读确认这两个值会保留，数组/对象仍为 true。
3. **生命周期保护已实际接入。** 浏览前 busy 防止重复 picker，关闭/切换 Local-Cloud 提升 generation；异步结果再次检查 closed/cloud/generation；取消保留上一次成功选择。Export 提交在父弹层针对最新 profiles 再验证所选 id，移除后的配置不会被当作成功请求。父弹层关闭会 dismiss 子层、取消删除任务并恢复焦点。

## 已定位的剩余工作

- `DeviceGamesDialog::profile_action` 的成功回调目前只设置 `collection_status` 和内存中的 `transfer_intent`。没有调用 profile/宏应用，没有目标文件选择、官方 envelope 编码或文件落盘。文案明确写“尚未应用/官方文件尚未生成”，不能把此次 UI 和解码完成计为完整 Import/Export。
- 这份 intent 仅属于当前弹层 Entity；关闭后重新创建弹层或退出进程不会持久保留它。后续实际 Apply 和本地草稿持久化应显式设计，当前“保留”不是持久保存。
- Export 列表当前只用原生 Profile 的 id/name；源 `Ia` 还会从 `macrosMapping` 构造宏行。缺少当前产品的完整 vendor profile/宏数据，因此不能把原生 settings 当作 vendor mappings 直接编码。
- Import 解码核对 category/productId/dongleId/bleId、base64 UTF-8、删除 hash/gamemode 后的 stable JSON MD5，再使用外层 name。逐项损坏产生 warning；JS 数字编码不兼容时哈希验证会拒绝，不会跳过 hash。
- 当前 `Da` 还有依赖真实 DeviceInfo/DEFAULTPROFILE 的 Snap Tap 默认 mode、特定 dongle polling 补齐，以及 Synapse 3 SharedWorker 转换。这些未完成；继续需要当前产品 MW 规范化证据，不能猜 schema 或运行下载 worker。
- `features/profile_transfer.rs` 是本地 `.razer-ui-profile.json`、单配置、布局校验和 `ProfileSettings` 模型；`features/source_workspace/profile_transfer.rs` 是另外的产品导入导出 UI。二者不能直接替代 `.synapse4` 的当前 Profiles 产品转换链。

本轮确认真实作用范围为源布局/选择/取消/本地读取解码/可见请求状态，后续应用与文件输出仍属当前任务待完成项，DLL 写回另按用户约定延后。

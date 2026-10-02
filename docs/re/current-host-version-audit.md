# 当前正式宿主版本审计（2026-10-02）

本次从 Razer 正式站点当前使用的更新链核实到 **Razer App Engine 4.0.827**，并从官方更新包静态提取其源码至 [`.ref/host-4.0.827/`](../../.ref/host-4.0.827/)。`4.0.821` 仅是此前机器安装版本，不能继续作为“最新正式宿主”的版本依据。

## 官方来源链

北京时间 2026-10-02 22:02 取得的正式站点 `https://apps.razer.com/background-manager/` 使用 `assets/index-8d39b3d5.js`。重新获取的脚本 SHA-256 为 `382ad87a9715c6417bac9d9f89a0a557521eaed4ed56a2512bc50c2d449b77c0`，与当前参考副本一致。

脚本的 `getManifestURL()` 根据当前页面 URL 选择更新清单：包含 `appsbeta` 时用 `beta/engine`，包含 `staging` 时用 `staging/engine`，其余情况用 `preprod/engine`。因此 `apps.razer.com` 的实际正式分支指向下面的 `preprod` 路径；路径名称本身不能用来判定它是测试版。完整代码片段及 HTTP 收据见 [engine-update-chain.json](../../.ref/host-4.0.827/source-evidence/engine-update-chain.json)。

1. [正式站点选择的 installer-manifest.json](https://apps.razer.com/synapse/dashboard/engine-manifests/preprod/engine/installer-manifest.json)：`latest.version = 4.0.827`，`latest.url = manifest-4.0.827.json`。
2. [manifest-4.0.827.json](https://apps.razer.com/synapse/dashboard/engine-manifests/preprod/engine/manifests/manifest-4.0.827.json)：资源 `EngineUpgrade`，版本 `4.0.827`，指定官方更新包与大小、SHA-256。
3. [官方更新包](https://app-assets.razer.com/files/synapse/RazerAppEngineUpgrade-v4.0.827.exe)：226,525,560 字节，实测 SHA-256 为 `d5e490647548389481321528fd7e0e1d536f0f8b92bf62747768e4ec3e9bc45d`，与清单完全一致。
4. 包内 `package.json` 为 `name=razerappengine`、`version=4.0.827`，`electron/engineVersion.js` 同样声明 `4.0.827`。

上述结论限定于 2026-10-02 核实的正式站点更新通道。它独立于本机是否已经安装该版本。

## 只做静态提取

使用本机已有的 7-Zip 24.02 将外层 EXE 当作 NSIS 压缩容器读取，仅提取内层 `RazerAppEngineUpgradeSetup-Internal-v.exe`；再将内层 EXE 当作含 ZIP 数据的容器读取，仅提取 `win-unpacked/resources/app.asar` 及一个 unpacked JS 文件。没有启动任何安装器、下载的 JavaScript、DLL、应用、构建或测试。

内层 ZIP 有“压缩数据结束后仍有附加数据”的容器提示，指定文件提取成功。外层完整更新包通过官方 SHA-256 校验，ASAR 内提供 SHA-256 integrity 的文件也逐个核验通过；没有通过执行安装器消除此提示。

- ASAR：102,766,875 字节，SHA-256 `b2ce8c54dc5c991feef3a24e1880ced57ba88a0f40a687a5b082187ac0a1cca6`。
- 提取全部 210 个非 `node_modules` 源码及资源文件，共 4,406,425 字节。
- ASAR 解析检查了头部及数据边界、输出绝对路径、路径分段、符号链接、文件大小和可用的 integrity 字段；所有目标限制在指定的 `.ref/host-4.0.827/` 内。
- 使用 Acorn 8.15.0 静态解析 112 个 JS/CJS 文件：105 个 script、7 个 module，全部通过。原始脚本没有被求值。

可复核收据：[ARCHIVE-CHAIN.json](../../.ref/host-4.0.827/ARCHIVE-CHAIN.json)、[EXTRACTION-RECEIPT.json](../../.ref/host-4.0.827/EXTRACTION-RECEIPT.json)、[STATIC-PARSE-RECEIPT.json](../../.ref/host-4.0.827/STATIC-PARSE-RECEIPT.json)。官方清单、HTTP 元数据和压缩目录清单保存在 [source-evidence/](../../.ref/host-4.0.827/source-evidence/)，不依赖旧源码目录。

## 与此前 4.0.821 的 UI 对比

210 个文件中，198 个与此前 4.0.821 字节一致，11 个修改、1 个新增、0 个删除。完整列表与旧版本每个文件的 SHA-256 均已固化在新来源目录的 [COMPARISON-4.0.821.json](../../.ref/host-4.0.827/COMPARISON-4.0.821.json) 和提取收据中，旧目录删除后仍能查看结果。

| 范围 | 核实结果 | 对界面的影响 |
| --- | --- | --- |
| `TabUI.js`、`TabManager.js`、`TabStore.js`、`Tab.js`、`LeftSystray.js`、tooltip CSS/JS | SHA-256 全部相同 | 页签、账户区、窗口控制和提示层的源码没有改版 |
| `electron/index.css`、`electron/index.html`、`electron/constants.js` | SHA-256 全部相同 | 宿主基础布局、样式和常量可以从 4.0.827 重新取证 |
| `electron/resources/` 的全部 30 个文件 | SHA-256 全部相同 | 图标及标题栏图像未变化 |
| `electron/components/Tab/common.js` | 增加 `background-manager` 加载失败后的重试恢复逻辑，另有压缩变量重命名 | 不改变正式页签或账户区域的标记及布局 |

`Tab/common.js` 新逻辑只对 `background-manager` 窗口生效：非取消型加载错误按 1、2、4、8、16、32 秒延迟重试；6 次耗尽后监听 `networkStatus`，恢复联网时重新尝试；成功后清零计数，窗口关闭时清理定时器和监听器。原有 `razer-id` 加载失败处理仍保留。通过 Acorn 分词消除短变量重命名噪声后逐段阅读，代码片段见 [TAB-COMMON-DIFF.json](../../.ref/host-4.0.827/TAB-COMMON-DIFF.json)，具体文件字节证据见 [TAB-LAYOUT-AUDIT.json](../../.ref/host-4.0.827/TAB-LAYOUT-AUDIT.json)。分词归一化仅是审阅辅助，不将其等同于完整语义验证。

其他已变更文件包括宿主版本/依赖信息、启动和系统工具代码；新增 `electron/lib/parseApplicationHostToken.js`。例如 `getAppHost.js` 改为先解析 hostname 再校验 `localhost` 或 `.razer.com`，`parseCmdParams.js` 要求参数位于字符串开头或空白之后。这些文件没有被概括为“与旧版一致”，本次界面结论仅覆盖上表明确复核的范围。

# 当前源码版本与用户问题复核

核验日期：2026-10-02。用户要求最新正式版雷云，旧参考源码不得继续作为实现依据。

## 当前可确认的版本

| 范围 | 本次证据 | 能确认的结论 |
| --- | --- | --- |
| 官方 Dashboard | `https://apps.razer.com/synapse/dashboard/` 的新HTTP响应；HTML、asset-manifest、manifest及相关JS/CSS逐字节比较 | 当前官方托管前端是 `0.0.86`，构建 `2609221012`，commit `6384376ce894f6fae1d69094b4feb7e507510242` |
| Alexa | `https://apps.razer.com/synapse/alexa/`，同次新HTTP核验 | 前端 `1.0.0`，构建 `2606221238`；当前本地应用快照一致 |
| 更多应用 | `https://apps.razer.com/rz-app-menu/`，同次新HTTP核验 | 前端 `0.0.1`，构建 `2609150253`；当前本地应用快照一致 |
| 官方当前宿主 | 正式站 background-manager 的默认更新分支、installer-manifest、manifest-4.0.827 与包 SHA-256 | 该正式更新流当前版本为 `4.0.827`；包内 ASAR 已纯静态提取到 `.ref/host-4.0.827/` |
| 本机已安装宿主 | 本机 `app-4.0.821/resources/app.asar` 的包信息 | 本机仍安装 `4.0.821`，本轮未安装、升级或运行宿主 |

新HTTP请求使用 no-cache/no-store/max-age=0 及独立查询参数。22项成功响应与 `.ref/applications` 中对应快照一致；响应、日期、ETag和SHA-256保存在 `.work/latest-source-check/frontend-live/report.json`。Source map未公开，不作为代码相同的证据。前端、宿主和安装器版本是不同的版本号。

当前 Dashboard 文件：`main.01550b17.js`、`App.72827d47.chunk.js`、`6505.84205103.chunk.js`、`9388.974b5d43.chunk.js`、`55.3f1ab18c.chunk.js`、`55.4e8559cb.chunk.css`。后续工具和资源准备已切换到 `.ref/applications/synapse/dashboard/`。

官方 `4.0.827` 宿主已提取210个非 node_modules 文件；ASAR SHA-256 为 `b2ce8c54dc5c991feef3a24e1880ced57ba88a0f40a687a5b082187ac0a1cca6`。TabUI、TabManager、TabStore、index.css、constants 及宿主图标与本机4.0.821逐字节一致；Tab/common.js 的变化是后台页面加载重试和网络恢复处理，不改变页签布局。正式环境不显示受 beta 条件控制的徽标。完整官方来源链、包哈希、提取及差异收据见[当前宿主核验](current-host-version-audit.md)。

## 用户指出的界面

- **设备与模块页的“Razer应用”按钮**：最新版6505模块及HomePage外层均未挂载该额外页脚入口，Rust中已删除。原应用发现入口在右上角更多应用弹层。见[专项证据](devices-modules-app-link-audit.md)。
- **顶部“保存”**：当前官方没有常驻的普通全局保存按钮，本地附加的 `save-all` 已移除；但源码确有条件出现的 `header-unsaved` 入口，下拉提供待保存配置列表、全部保存和全部丢弃。两者不能混为一谈。原版具体编辑器自己的保存/取消仍保留，详见[工具栏与保存复核](toolbar-current-audit.md)。
- **Gamer Room教程位置**：已删除横幅和设备组之间本地额外插入的一行，按最新真实组件树把教程锚定在设备组容器原点；第一步偏移为 `(220,22)`，第二步为 `(213,-25)`。取消原版没有的窗口边缘吸附和面板高度压缩。详见[当前挂载与定位审计](gamer-room-current-audit.md)。

## 资源与文案迁移

教程媒体脚本改从当前 Dashboard manifest 解析请求，缓存命中也刷新来源记录。配对图标从当前 `7861.1b0e99a4.chunk.js` / `7861.a49b4dc6.chunk.css` 重新静态提取，Dashboard 图片规则也改用该当前 chunk；没有沿用失效的4130编号。资源准备使用4.0.827的宿主图标和字体。

10种 Dashboard 语言包通过 [Acorn 静态提取工具](../../tools/extract-dashboard-locales.cjs) 重新解析模块内字符串、别名和导出 getter，未执行下载的 JS。保留设备/设置独有文案及 Alexa 独立命名空间，当前主前端的同名文案以新源为准；来源和键数见[文案收据](dashboard-locale-source.json)。应用目录从当前宿主、Dashboard 与 Settings 字面量重新生成，偏移与哈希重新计算，不只是替换文件名。

资源校验会在打开源文件前拒绝旧目录来源，并检查 Dashboard 静态文件是否由当前 manifest 声明。

快捷键的键位表已重新从当前2280模块629静态提取，127条有效记录与之前内容相同，源SHA更新为当前文件。原7282入口现为 `4608.e973916f.chunk.js`：模块94608继续导出 `GlobalShortcutsContainer`，`setDataToMappingEngine` 位于字符122924附近，仍先生成mappings再计算hash。对应文档的文件定位已更新；其余历史短符号仍不因此视为重审。

## 旧源码清理状态

旧 `.ref/frontend/`、`.ref/synapse-asar/`、`.ref/host-4.0.821/` 及 `.work/latest-source-check/host-4.0.821/` 已由用户于2026-10-02手动删除。之后使用 `Test-Path` 确认四个目录均不存在，当前Dashboard和4.0.827宿主目录仍存在；删除后的422项资源校验通过，未依赖旧源。此前自动审批拒绝删除的记录仅为历史过程。根 `AGENTS.md` 禁止重建或继续使用旧来源和旧提取脚本。

旧审计文档新增来源迁移提示。链接切换只解决文件定位，历史压缩符号与尚未重新审计的业务结论不自动升级为最新版结论。

## 本轮最终静态检查

`cargo check --locked --all-targets` 通过，包含新增交互测试源码的编译；`cargo fmt --all -- --check` 通过。资源校验通过422项源/输出哈希、132个Webpack请求、73个产品图变体、58个Dashboard变体、16个键盘布局/1901个输入形状。10种Dashboard字典及其当前来源收据、127条快捷键编码的 `--check` 通过，10种Alexa独立字典原文保持一致；8个本轮变更Python文件的静态语法解析和 `git diff --check` 通过。

没有运行应用、构建、测试、安装器、下载的JavaScript或DLL；资源转换和ASAR解析不执行包内代码。静态检查不证明真实窗口的最终像素、滚动、焦点或硬件行为已验收。

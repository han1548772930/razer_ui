# 当前源码版本依据

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

## 使用规则

产品与应用只使用各自当前 manifest 声明的源文件。静态提取工具位于 `tools/`；不得执行厂商 JavaScript 或 DLL。版本收据证明所取来源，不代表整个 UI 已验收；当前实现与缺口见 [文档索引](../README.md)。AGENTS.md 禁用的旧源码目录不得读取、重建或作为实现依据。

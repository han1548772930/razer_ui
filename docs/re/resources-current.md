# 当前资源契约

原始资源由各当前 manifest、Webpack 导入及实际挂载 CSS 指定。不能用同名文件、其他产品缩略图或 locale 的存在推断资源已正确接入。

[全量源码字节清单](full-source-corpus-current-evidence.json)逐文件记录当前规范目录的 SHA-256、HTTP 收据匹配以及 asset-manifest/webpackManifest 的取得与缺失目标。此清单用于查明资源与源码的来源，不替代本页的实际加载/消费检查；跨层关系入口见 [全量逆向地图](full-source-reverse-map.md)。

[主清单](../../assets/synapse/manifest.json)及各专项清单记录来源路径、源/输出 SHA 和映射；[资源加载器](../../crates/razer-assets/src/lib.rs)消费生成的嵌入表。清单登记、文件存在、加载器注册与页面实际消费分别核对，数量以工具当前输出为准，不维护过期的资源统计快照。

维护入口：

- `tools/validate-resources.py`：源与输出哈希、格式、嵌入键及映射。
- `tools/validate-embedded-json.py`：内嵌 JSON 与 Rust 所需字段；跳过的结构不能记为已验证。
- `tools/audit-resource-usage.py`：生产模块中的直接引用与已登记动态资源族，不是完整 Rust 调用图。
- `tools/audit-assets-current.py --check`：全目录文件与哈希、生产模块内嵌路径、运行时键、加载/列举一致性和逐组内容重复；[清单](assets-current-inventory.json)保留完整逐文件结果。
- `tools/prepare-resources.py`及各产品/应用准备器：只从当前源解析、转换并生成资源，不执行厂商 JavaScript。
- [当前媒体恢复规则](current-media-offline-recovery.md)：由当前引用及内容指纹恢复资源；保持原始来源证据。

产品编号/edition/layout 可以是资源身份键，不能成为共享组件里的产品特例。缺少身份或资源时保留明确缺口；不能将默认版号当作实际硬件读值。运行窗口、字体栅格、DPI 及像素匹配尚未验收。

## 2026-10-09 资源核对与清理

[逐文件清单](assets-current-inventory.json)覆盖 assets 全目录，记录字节数、SHA-256、内嵌映射、生产模块静态引用和同内容分组。重复运行时键与重复文件内容分别统计；格式化路径和实际渲染仍有静态检查边界。

修复 `SynapseAssets::list()` 漏列托盘 Widget 资源的问题，与 `load()` 的资源族恢复一致。`audio-demo-play.svg` 经当前源准备器 `--check` 验证后，修正主清单中的过期 SHA；专项音频清单与实际输出一致，图形内容未改动。

资源引用扫描改为遍历工作区生产 crate 与 Rust 模块。内嵌 JSON 校验使用完整文件路径和同 crate 类型解析，避免旧 src 路径、仅入口文件或同名 lib.rs 造成的漏检；动态 Value、无法解析的类型会明确报告检查边界。

清理前有 150 组字节相同的文件，额外副本合计 13,387,652 字节。这包含产品/edition/layout 别名、源 AVIF 与来源收据；不能按内容相同直接删除映射。保留这些来源与域身份，每组文件可在清单定位。原生 `.node` 文件仍被静态逆向工具引用，也保留。

移除以下无消费旧图标和空重试文件。当前窗口按钮使用 `synapse/host-*.svg`；空重试文件对应的 AVIF、成功 HTTP 200 收据、长度与 SHA 已独立核对并保留。

| 删除文件 | 字节 | 删除前 SHA-256 / 保留证据 |
| --- | ---: | --- |
| `assets/window-close.svg` | 258 | `70bff300124b187d4a2bbfc85bdf3745e2a54decc7468a09700dbdb4e4a05cae` |
| `assets/window-maximize.svg` | 465 | `d35892e655f05e3aabaf81d7a00a1c6916d4bb7da45d696871bbc257c180da6f` |
| `assets/window-minimize.svg` | 353 | `df697f1734e59696ccace9e1318417853a6052f07553f785c17b9aa456675ce3` |
| `assets/window-restore.svg` | 228 | `e5f7d587c6a5ef8a4fa7e5caae4eff799093aa944e2bf15544361dc4a5508c67` |
| `assets/synapse/dashboard-1303-0-0-source.avif.response` | 0 | [dashboard-1303-0-0-source.avif.http.json](../../assets/synapse/dashboard-1303-0-0-source.avif.http.json)；空文件 |
| `assets/synapse/dashboard-1304-0-0-source.avif.response` | 0 | [dashboard-1304-0-0-source.avif.http.json](../../assets/synapse/dashboard-1304-0-0-source.avif.http.json)；空文件 |
| `assets/synapse/dashboard-1313-0-0-source.avif.response` | 0 | [dashboard-1313-0-0-source.avif.http.json](../../assets/synapse/dashboard-1313-0-0-source.avif.http.json)；空文件 |
| `assets/synapse/dashboard-1313-128-0-source.avif.response` | 0 | [dashboard-1313-128-0-source.avif.http.json](../../assets/synapse/dashboard-1313-128-0-source.avif.http.json)；空文件 |
| `assets/synapse/dashboard-3893-0-0-source.avif.response` | 0 | [dashboard-3893-0-0-source.avif.http.json](../../assets/synapse/dashboard-3893-0-0-source.avif.http.json)；空文件 |
| `assets/synapse/dashboard-3893-128-0-source.avif.response` | 0 | [dashboard-3893-128-0-source.avif.http.json](../../assets/synapse/dashboard-3893-128-0-source.avif.http.json)；空文件 |
| `assets/synapse/dashboard-3894-0-0-source.avif.response` | 0 | [dashboard-3894-0-0-source.avif.http.json](../../assets/synapse/dashboard-3894-0-0-source.avif.http.json)；空文件 |
| `assets/synapse/dashboard-3894-255-0-source.avif.response` | 0 | [dashboard-3894-255-0-source.avif.http.json](../../assets/synapse/dashboard-3894-255-0-source.avif.http.json)；空文件 |
| `assets/synapse/dashboard-3907-0-0-source.avif.response` | 0 | [dashboard-3907-0-0-source.avif.http.json](../../assets/synapse/dashboard-3907-0-0-source.avif.http.json)；空文件 |

当前已登记/内嵌资源没有文件缺失，不代表当前官方整个 manifest 的远端资源全部取得；远端缺失与取得状态仍由全量源码清单分别记录。以上均为静态核对，未运行程序或验证像素渲染。

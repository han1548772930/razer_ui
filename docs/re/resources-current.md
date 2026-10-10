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

## 资源注册与校验

[逐文件清单](assets-current-inventory.json)记录 assets 全目录的字节数、SHA-256、内嵌映射、生产模块静态引用和同内容分组。重复运行时键与重复文件内容分别统计；格式化路径和实际渲染仍有静态检查边界。

`SynapseAssets::list()` 与 `load()` 覆盖相同资源族，包括托盘 Widget。当前窗口按钮使用 `synapse/host-*.svg`。主清单、专项清单和实际输出的 SHA 必须一致。

资源引用扫描遍历工作区生产 crate 与 Rust 模块。内嵌 JSON 校验使用完整文件路径和同 crate 类型解析；动态 Value、无法解析的类型会明确报告检查边界。

相同字节可能对应不同产品、edition、layout 或来源身份，不能按内容相同合并资源映射。每组文件可在清单定位；被静态逆向工具引用的原生 `.node` 文件也是当前证据输入。

当前已登记/内嵌资源没有文件缺失，不代表当前官方整个 manifest 的远端资源全部取得；远端缺失与取得状态仍由全量源码清单分别记录。以上均为静态核对，未运行程序或验证像素渲染。

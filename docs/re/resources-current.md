# 当前资源契约

原始资源由各当前 manifest、Webpack 导入及实际挂载 CSS 指定。不能用同名文件、其他产品缩略图或 locale 的存在推断资源已正确接入。

[主清单](../../assets/synapse/manifest.json)及各专项清单记录来源路径、源/输出 SHA 和映射；[资源加载器](../../src/resources.rs)消费生成的嵌入表。清单登记、文件存在、加载器注册与页面实际消费分别核对，数量以工具当前输出为准，不维护过期的资源统计快照。

维护入口：

- `tools/validate-resources.py`：源与输出哈希、格式、嵌入键及映射。
- `tools/validate-embedded-json.py`：内嵌 JSON 与 Rust 所需字段；跳过的结构不能记为已验证。
- `tools/audit-resource-usage.py`：生产模块中的直接引用与已登记动态资源族，不是完整 Rust 调用图。
- `tools/prepare-resources.py`及各产品/应用准备器：只从当前源解析、转换并生成资源，不执行厂商 JavaScript。
- [当前媒体恢复规则](current-media-offline-recovery.md)：由当前引用及内容指纹恢复资源；保持原始来源证据。

产品编号/edition/layout 可以是资源身份键，不能成为共享组件里的产品特例。缺少身份或资源时保留明确缺口；不能将默认版号当作实际硬件读值。运行窗口、字体栅格、DPI 及像素匹配尚未验收。

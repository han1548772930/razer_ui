# 文档清理记录：2026-10-06

本批删除 5 份已被当前入口替代、容易误导实现的 Markdown：

| 删除文件 | 原因 | 当前入口 |
| --- | --- | --- |
| `docs/RAZER-SYNAPSE-UI-SPEC.md` | 旧总规格混有三产品和旧宿主假设 | [文档索引](../README.md) |
| `docs/re/01-ipc-api-surface.md` | 4.0.563 IPC 历史审计 | [DLL 只读盘点](dll-readonly-inventory.md) |
| `docs/re/07-page-coverage.md` | 旧 21 页统计 | [当前覆盖](native-product-coverage.md) |
| `docs/re/product-ui-coverage-audit.md` | 旧未接入数量 | [当前覆盖](native-product-coverage.md) |
| `docs/re/ui-integration-checkpoint-2026-10-04.md` | 过期数量及 Chroma/Studio 身份结论 | [当前续接](ui-continuation-2026-10-06.md) |

重写 README、文档索引、实现差距、完成口径和剩余工作，修复链接与编码，统一指向当前实施路线。代码注释中的失效章节引用一并纠正；历史推断仍标为未核实，重定向不等于重新取证。

保留源码收据、JSON 和具有独有证据的逐页审计。`screens/12-settings.md` 对应 `/synapse/settings/`，不是新的 `/settings/`；`screens/15-account-menu.md` 是头像菜单，不是托盘，不能按名称相近删除。

本批清理限于文档与相关注释；工作区已存在的图片及 pip 文件删除状态未作恢复或进一步修改。

另清除 4 份历史文档的问号乱码段落或损坏引用，保留可读内容；没有按残存词句猜测还原。重定向后的链接说明也按目标文档范围修正。

最终检查：182 份项目 Markdown、334 个本地 Markdown 链接，断链为 0，连续问号乱码为 0。检查范围为根 README、AGENTS.md 与 docs，不扫描禁止使用的历史源码目录。格式化、cargo check、当前源码收据和资源检查通过；运行验收未进行。

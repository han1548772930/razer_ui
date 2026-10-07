# Profiles 导入／导出弹层续接（2026-10-07）

本批针对当前 `/synapse/profiles/` 的 `43/Ua → Ia → Da/Ea`，不是各产品独立 ImportExportModal 的通用替换。原码 SHA、AST 范围、CSS、图片与各产品导入兼容身份见 [静态证据](profiles-transfer-current-evidence.json)；工具是 `tools/prepare-profiles-transfer.cjs`，只解析当前源码，不执行厂商 JavaScript。

## 已接入

- 设备关联弹层的 Import/Export 菜单打开独立 retained 弹层；保留原版 602×481、顶部 104、36/49/47 区域尺寸及原版始终为 Export Profiles 的标题条件。尺寸经过 `surface::css` 转换，颜色来自 theme。
- Export 初选全部本地配置，排除原版特殊 GUID；逐项、全选／取消全选有独立选择状态。提交时再次验证配置 ID 存在，避免已删除目标进入后续请求。
- Import 使用本机文件选择器，读取 `.synapse4` JSON/base64/UTF-8、稳定序列化与 MD5 校验；坏条目不会冒充有效配置。显示所选文件名、有效配置及关联宏，支持独立宏勾选；取消选择器保留已有选择，关闭／切换 Local/Cloud 使迟到结果失效。
- 兼容性根据当前 MW 逐产品身份收据判定；非 `is8kAnalogDevice` 产品按 `Da` 过滤包含 Dynamic Keystroke 的嵌套映射。独立复核修复了 JS 真值、URL-safe/base64 清洗和整数／浮点形式产品 ID 的相等语义，见 [解码器复核](profiles-transfer-codec-review-2026-10-07.md)。
- 使用 5 张当前原始 SVG；警告在提交按钮旁，通过源码的悬浮提示显示，宏图标也保留提示。按钮透明度按 300ms 过渡。`.slide-off` 的 display:none 与 `.slide-on` 的 display:block 不被错误替换成自创高度动画。
- Cloud 显示当前源已存在的禁用设备选择及 in-development 状态；没有制造云端目录。
- 关闭／取消清理异步任务并恢复父弹层焦点；遵循原版没有 Escape、Enter 或遮罩点击提交／关闭的条件。

## 当前仍未完成的操作链

**提交目前仅保留明确的本地传输请求／选择，并提供撤销。** 导入没有应用到配置，导出没有生成官方文件，界面状态明确说明这两点。本地请求随设备关联弹层存活，不是持久化保存。现有 `.razer-ui-profile.json` 是另一个本地格式，不能直接贴上 `.synapse4` 后缀；本批没有这样做。

完整产品规范化、导入应用到本地配置草稿、宏集合导入及官方文件输出仍属于当前 UI 工作，不能归入 DLL 写回后置。`Da` 的 dongle polling 默认值与 Snap Tap 默认 mode 回填尚需有效 DEFAULTPROFILE 观察；Synapse 3 SharedWorker 迁移尚未实现。原始导入文档目前只用于预览和请求，不能假定后续能无损编码回官方格式。

官方文件的极端浮点最短表示及异常 Unicode 与原生解码仍有兼容性边界；MD5 不匹配会拒绝对应条目，不跳过校验。16 MiB 是本地解析上限。全部配置内容、滚动、焦点、控件几何及不同语言的显示仍未运行验收。

允许的验证为格式、静态源／资源校验和 `cargo check --locked --all-targets`。新增解码边界用例只参与编译检查，未运行测试。本页继续记为 partial，完整产品数不增加。

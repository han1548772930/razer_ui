# 本地产品预览名称复核（2026-10-07）

本次延续“设置本地测试缺产品”的问题，核对现有全量选择器和首页名称来源。当前选择器已有全部 331 个登记产品；本次没有把路由登记视为产品页面已经完成。

确认的缺口是名称：选择器只把 registry 的英文 `name` 放进 `Choice::title`，而 gpui-component 0.7.1 的 `SearchableListItem::matches` 仅对 title 做不区分大小写的子串匹配。因此输入当前官方中文产品名（例如“黑寡妇”）确实无法找到对应产品。本地预览工厂还把同一个英文名填入 en 和 zh-cn，且版本选择只有数字。

修复使用当前 `.ref/discovery/products/{pid}/{ui,mw}/manifest.json` 的 `productTranslations`：`PRODUCT` 用于产品名称和搜索，`DASHBOARD_NAME` 与 `DASHBOARD_EDITION` 用于首页。按原目录指纹逐项验证 662 份 manifest；资源涵盖 331 个产品、779 个源声明版本及 2 个未声明版本产品的本地默认项。相同英语译文去重，其他语区原文保留，缺少文本仍回退原注册名称，不编造译名。

设置的名称搜索同时保留产品 ID、中文名、英文名和当前语区名称。版本选择显示源版名，例如 92 / 128 的 `Overwatch`。切换语言仅刷新标签，并按值恢复原产品、版本和布局选择；程序设置不会发出用户 Confirm 事件。沿用框架 Select 的搜索、键盘选择、焦点和弹层行为，没有新增自绘选择器。版本控件适度加宽，并保留原横向换行布局。

`demo::apply_preview_names` 只处理同时满足 `PREVIEW-` 序列号与 `preview-` 容器的显式本地预览，保留“预览 / preview”标识和变体标识。它只更新名称和版本文案，不更新 setup、固件、电量、在线状态、设备能力或 profile。新建预览和设置选择非默认版本的路径都调用该函数。已落盘的旧预览不会被本次静态检查或打开资源工具批量改写。

当前 Dashboard 22534 的 G/V/K 也重新按当前文件 SHA-256 和 UTF-16 偏移核对：G 选择 zh-cn 或 en；V 从本语区 name、baseProductName、英文名依次回退；K 显示本语区 editionName。名称源及这三条当前切片保存于 [证据](preview-product-names-current-evidence.json)。

验证仅包括 `rustfmt`、`python tools/prepare-preview-product-names.py --check`、`python tools/audit-product-preview.py --check`，以及选择器 API 静态阅读。后一个检查仍确认 331 产品、779 声明版本、653 的 16 个已提取布局。其他键盘渲染器没有对应的多布局实现，本次没有开放未经支持的布局。统一 `cargo check --locked --all-targets` 由主任务记录。没有运行应用、构建、测试、下载 JavaScript 或 DLL，也没有作窗口像素验收。

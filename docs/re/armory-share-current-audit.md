# Armory 本地配置分享与详情预览

2026-10-04。产品配置文件菜单中的“分享”现在可以把已有设备与当前配置的快照交给 Armory，
打开实际可编辑的本地分享草稿。空的 Browse/My Downloads 不添加示例贡献；guest 的 My Uploads
仍保持隐藏。入口是明确的本地草稿能力，不意味着账户、远端分享或内容服务已经接通。

## 当前源码

- 66517 挂载分享页面；24988 定义 modal 和提交条件；13784 定义完整表单。
- PROFILE 提交条件要求设备和配置索引均非 -1、标题 trim 后非空且 hasToxicContent 为假。
  标题最大 32、描述最大 500 个 JavaScript UTF-16 单位，描述可选。
- 13784 选中设备后挂载 90516，使用 `hideProfileInfo=true`；该预览使用所选配置内容，
  不需要伪造服务器贡献作者、UUID、点赞或下载统计。57230 是 DPI 详情模块。
- 支持游戏最多 10 项；16230 的 Browse 使用 `.exe` / `.url` 多文件选择。
- 9728 的真实服务为 POST `/contributions` 和 POST `/toxicity/validate`。本地实现不调用它们。

源码表单使用 800px 内容、最大 1020px modal、36px 标题、27px 提交按钮、23px 标题输入和
91px 描述框。当前 10 种语言的表单文案已逐项与现有 locale 比较，没有改用自行翻译的替代内容。

PID 182/653 的当前 profile 类分别为 lD/UD，菜单 `s1I` 均解析为 `SHARE_TO_WORKSHOP`。
两者 `openArmoryEditView` 都传递 selectedProfileGuid/productId，以 `policy=3,tab_visible=1` 打开
`/synapse/armory/?view=my-upload&sharePopup=true`，已有目标则广播并激活。支持类别均为
KEYBOARD、MOUSE、AUDIO、SYSTEM、BROADCASTER、MOUSEPLUSMAT；常量、方法和当前 manifest 均有静态收据。

官方菜单还要求已安装 Armory、非 guest、开启 profile-sharing feature；已分享和维护状态会禁用分享。
原生入口遵照用户直接访问已实现模块的要求，开放明确的本地草稿，并不表示远端账户或特性门槛已满足。
入口只添加到已经完整实现 profile more-menu 的 PID 182/653，其余设备未拼接局部替代菜单。

## 已实现分支

分享草稿保留传入快照，预选当前配置，标题从空值开始。用户可选择该设备已有配置、输入标题和
描述、查看 UTF-16 计数、添加或移除游戏文件。单配置设备不进入可分享选择；没有真实配置时
提交保持禁用。游戏文件只保留路径与名称，不执行、不上传文件。

本地字段满足条件后提交会显示“分享服务未连接”，保留输入；不会把未执行的在线内容检测写成
已通过，也不会制造加载进度或成功状态。文件选择等待来自真实的系统选择器。

下方详情预览显示真实设备、配置名及已有鼠标 DPI 参数。文件大小明确标为“本地草稿”，计算的是
当前本地 Profile 快照的 UTF-8 JSON 字节，不使用源示例 contribution 或硬编码的 3KB。
Base Dialog/DialogPopup 承担模态、Escape、焦点隔离和面板点击边界；关闭回到 Armory。

## 尚未完成

完整远端卡片、远端详情、发布后的作者/点赞/下载、账户与维护状态、在线内容检测及上传仍未实现。
本地详情目前是可达的配置摘要/DPI 预览；原生产品图、键位映射、音频等完整 90516 子区尚未补齐。
游戏添加直接走源 Browse 对应的系统文件选择；安装游戏扫描、源中间选择页和拖动排序尚未实现。
本地快照格式不等同于官方 contribution 上传包；草稿不写回设备或发布到服务器。

## 校验

`node tools/audit-armory-share.cjs --check` 只解析与比较证据，不写文件。
生成证据使用 `node tools/audit-armory-share.cjs`；默认根审计由 `audit-armory-default.cjs` 保持。
静态 AST、CSS、现有 locale 和原生接线收据见 [armory-share-current-evidence.json](armory-share-current-evidence.json)。
未运行应用、构建、测试、下载的 JavaScript 或 DLL。像素、IME/粘贴边界、系统文件选择器和
焦点恢复仍需后续获准运行验证；本审计不把编译或片段检查当作运行等价证明。

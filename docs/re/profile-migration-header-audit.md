# 当前工具栏配置迁移入口

核验日期：2026-10-03。只读取当前源码、静态下载资源和解析文本；未执行原 JavaScript、应用、DLL、构建或测试。

## 当前源码依据

Dashboard 使用 `.ref/applications/synapse/dashboard/`，版本依据见 [当前源版本](20-current-source-version.md)。本次独立对照生产 `https://apps.razer.com/profile-migration/` 的 HTML、asset-manifest、`main.512f18b6.js`、`main.e18d10bc.css`，四项与 `.ref/profile-migration/` 均逐字节相同。下载 URL、SHA-256 和比较结果见 [静态核验收据](profile-migration-current-source.json)。未使用被删除的历史目录。

| 行为或外观 | 当前源中的直接依据 |
| --- | --- |
| 默认第四项是配置迁移，位于应用选择器左边 | `App.72827d47.chunk.js` 模块 `96776`，`We.render` 的 `.right` children 顺序：`this.props.isShowProfileMigrationIcon && je`、`E`、`setting`、`ie`。`We.defaultProps.isShowProfileMigrationIcon = true`。前面的更新、离线、未保存、下载、OLED 等是各自条件项，不能据此把“始终四个”当成所有状态的规则。 |
| 迁移项默认显示，关闭页签才持久隐藏 | 同模块 `je`（字符偏移 188522）先 `useState(!0)`，再由 `O.A.get(s.PkL)` 的布尔值覆盖。`main.01550b17.js` 将 `PkL` 导出为常量 `p = "showProfileMigrationIcon"`。`tab-message` 同时满足 `action === "tabClosed"` 和 `data.tabName === d` 后写 `false` 并隐藏。点击本身不隐藏。 |
| 点击是同窗口页签 | `je` 调用 `V._P('/profile-migration/?app=' + currentApp, d, sameWindow, tabVisible, autoFocus)`；Synapse 的 `d` 为 `syn3-profile-migration`，Chroma 才加 `chroma-app-` 前缀。 |
| 图标尺寸 | `55.4e8559cb.chunk.css`：`.toolbar .right>div` 为 `46px × 38px`；`.synapse-profile-migration` flex 居中；`.profile-migration-icon` 为 `40px × 40px`，背景居中、不重复。原 `synapse_profile_migration.c3622288.svg` 是 `24 × 24`，没有 background-size 覆盖，所以图案保持 24px，不能拉伸为 40px。 |
| 悬停、提示 | 同 CSS 的 `.toolbar .right>div:hover` 为 `#2d2d2d`；`je` tooltip 使用 `l.b1N`，当前导出为 `PROFILE_MIGRATION`。 |
| 独立页签标题、favicon、页内导航 | 当前迁移 `main.512f18b6.js` 的 `OD` 写 `document.title = "PROFILE MIGRATION"`，给 toolbar `pD` 传 `tabNavigations: []`、`PROFILE_MIGRATION` 页内标题。HTML 使用 `./favicon.svg`；该资源已静态下载并记录哈希。空导航使前后按钮禁用；toolbar 保留刷新。 |
| 页内外距 | 当前迁移 CSS：`.body-wrapper { padding: 10px 20px 20px }`，`.profile-migration-view { padding-top: 10px }`，合并后内容上距 20px、左右和下距 20px。原 `mg` 横幅仍由现有共享页渲染。 |

## 实现边界

- 工具栏、设置中的原迁移按钮、应用选择器的迁移项共用 `Location::ProfileMigration`，创建或聚焦唯一 `HostTab::ProfileMigration`。该页签不是普通弹窗。刷新重新创建页状态，关闭释放页实体并把 `AppPreferences.profile_migration_icon_visible` 置为 `false`；沿现有自动保存队列持久化。旧工作区缺此字段时默认 `true`。已关闭后仍可从设置和应用选择器重新打开，不自动恢复已隐藏的工具栏图标。
- 正常页不创建场景选择器，也不装载示例记录。“服务连接 → 打开配置迁移”现与工具栏及原设置按钮共用正式 HostTab，不再调用 `open_preview` 测试弹窗。入口修复依据见 [设置入口](settings-entrypoints-current.md)。
- 扫描器及迁移 DLL 尚未接入。正常页只显示已有原版横幅和禁用的迁移按钮；不把“未知”伪装成原版的“未找到备份”，也不虚构扫描进度或导入结果。这是本地适配器对未知状态的处理，不声称等同于原版服务已运行后的页面。
- 原迁移应用 `OD` 会初始化 `_g` 并调用 `scanData()`；本实现没有执行这些下载的脚本或 DLL。预览中的记录、进度和日期仍是明确标识的示例，不作为当前用户数据或服务事实。
- `je` 另有迁移提示气泡：Dashboard `7861.1b0e99a4.chunk.js` 的 `closeIntroductionBanner` 发 `show-profile-migration-toast`，且 `!validDevices.some(firmwareUpdateInfo) && !newFWVersion` 才显示；提示文案是 `TRANSFER_ALL_SYNAPSE_PROFILES_DESC`。本地尚无这两个真实固件状态，不能把 Unknown 当成“无更新”；本次不自动触发该气泡，固件预览状态也不代替真实条件。
- 迁移应用 `OD` 实际传的是 `showProfileMigration: false`，而 toolbar 检查的是另一个属性 `isShowProfileMigrationIcon`。本次按实际使用的默认值和持久化条件处理，没有把相近属性名当作隐藏证据。

## 静态验证

使用现有 `.work/resource-env/Scripts/python.exe tools/prepare-resources.py` 统一生成资源，随后 `python tools/validate-resources.py` 通过：432 条源/输出 SHA-256、图像格式和嵌入索引，132 个 Webpack 请求、73 个产品解析项、58 个 Dashboard 身份、16 个布局 / 1901 个输入形状。此数包含本轮并行固件任务的原 SVG 增量。

迁移 SVG 及独立页 favicon 均由维护中的资源工具复制并嵌入，来源和输出哈希归入 `assets/synapse/manifest.json`。新增默认值/关闭后序列化及单一页签重复打开/关闭/重开回归用例；按用户约束未运行测试。Rust 编译检查由主任务统一执行。

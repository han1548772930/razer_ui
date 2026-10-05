# 链接会话续接：2026-10-05

继续会话 `01a109ca-c22e-73e3-9de0-a3eba5f79bc6` 的完整 Synapse UI 复刻目标。本批改动已经落在工作区；**总体目标仍未完成，没有将任何产品提升为完整复刻**。

实施证据限于当前 `.ref/applications/synapse/dashboard/`、`.ref/host-4.0.827/` 和对应当前产品包。没有读取或重建用户删除的四个历史目录，没有执行厂商 JavaScript、DLL、应用、安装器、构建或测试。

## 本批接入

| 范围 | 已完成的本地改动 | 证据与边界 |
| --- | --- | --- |
| Devices & Modules | 用实际服务记录投影替换空展示槽；新设备、已安装/卸载中设备和模块、固件正式行；本地已实现入口直接打开；移除确认与 100ms 延迟；源进度颜色/尺寸和 300ms/1ms 时序 | [集成记录](module-service-integration-2026-10-05.md)、[服务行](module-service-rows-current-audit.md)、[移除确认](module-service-remove-followup-2026-10-05.md)。workspace v2 的 `module_services` 是可选快照入口，未连接原生服务，没有写入示例快照或伪造安装结果 |
| 全局快捷键文本 | 静态提取 1,703 个分类条目、1,936 个搜索记录及 230 个变体列表；430×337 弹窗、搜索/清空、键盘导航、保留选区插入与 UTF-16 250 单位限制；字符映射表实际点击入口 | [文本专项](shortcuts-text-followup-2026-10-05.md)、[当前证据](shortcuts-current-ui-evidence.json)。字符映射表没有在本次工作中启动 |
| 文本面板时序补齐 | 两层各 500ms linear 的弹窗 opacity 按合成 alpha 处理；关闭立即隐藏，快速重开延续过渡；变体、分类名及工具栏提示恢复 1s 可见性延迟、300ms linear opacity；搜索点击聚焦及焦点下划线 | 当前 `34198/b/v/D/R`、外层 `81787/l` 及有序 CSS。提示延迟隐藏期间保存各自当帧命中范围；deferred 内容显式继承弹窗透明度。支持减少动画设置，尚无运行时像素验收 |
| 3946 quick macro | 空列表开始捕获、任意 keyup 结束、去重/十键上限、Escape 分支、修饰键事件、原类型图标与删除叠层、200ms 过渡 | [专项审计](automation-quick-keyboard-current-audit.md)。Caps Lock 物理释放及菜单几何仍有缺口，未连接自动化执行器 |
| 产品独立 Armory | 纠正 3894 为 Head Cushion Chroma；按固定 ACCESSORY 分类选择当前可达图片根；接入 3907 独立无遥测分支、六份 Smart 曲线草稿、单位/节点编辑及纵向轴 | [独立根审计](armory-independent-roots-2026-10-05.md)。不把 3894 当前不可达 DEFAULT 当作必须补造的页面；无虚构遥测，原产品位图仍缺 |
| 资源完整性 | 35 个服务 SVG 独立注册；七个文本 SVG 和 24 个 automation SVG 纳入主嵌入清单；17 个 Cooling Pad 图标/字体轮廓资源；修复轴 SVG 的换行/字节哈希问题 | 主资源校验覆盖源/输出哈希及实际嵌入键。automation 准备器对相同字节跳过重写，并维护主清单；专用校验器同时检查嵌入注册 |

原版安装/移除、固件 SDK、遥测、服务扫描与执行器仍为明确的服务边界。本地保存或 UI 草稿不代表硬件已执行。

## 允许的验证

最终 Rust 内容的 `cargo check --locked --all-targets` 成功（8.31s）；只有既有 `customize_page::layer_button` 的 dead_code 警告。该命令检查全部 target，但没有运行测试或应用。

以下均成功：

- `cargo fmt --all -- --check`，以及对 include 文件的显式 `rustfmt --check`。
- `audit-module-service.cjs --check`：11 个 AST 合约、35 个分类/警告 SVG。
- `audit-module-service-rows.cjs --check`：75551 进度、详情图边界、固件链接及警告样式。
- `audit-shortcuts-current.cjs --check`：八种映射分支、emoji 字面量、14 个源模块、双层弹窗时序/定位证据、文本实现哈希及七个资源注册。
- `audit-automation-quick-keyboard.cjs --check`、`extract-automation.cjs --check`、`validate-automation.py`：六种类别、七种灯效、真实参数分支、十份语言表和 24 个 SVG。
- `audit-armory-remaining-roots.cjs --check`、`extract-armory-cooling-icons.cjs --check`、`prepare-armory-cooling-axis.py --check`。
- `validate-resources.py`：**1,112 项主资源哈希/嵌入键 + 35 个服务 SVG**；133 个 Webpack 请求、74 个产品图变体、59 个 Dashboard 变体、16 个键盘布局及 1,901 个输入形状。
- `validate-embedded-json.py`：33 份 JSON 语法解析成功、0 失败；其中 30 份通过结构检查，三份既有无法推断结构的 JSON 跳过结构检查。
- `audit-locale-keys.py --check`：441 个字面量 `t()` 键，缺失 0；42 个软 fallback 和 50 个动态调用另行记录，仍有五个既有软键缺失。
- 限定 `src tools docs assets Cargo.toml Cargo.lock .gitignore` 的 `git diff --check`；仅 Git 换行转换提示，无空白错误。没有暂存、提交、重置用户更改或处理无关 pip 目录。

上述检查不替代字体、缩放、焦点、滚动、动画和交互的实际窗口验收；用户当前禁止运行应用，所以未做此类验收。

## 资源下载故障

普通网络下载失败。后续网络提权操作未执行：自动审批服务返回 `404 unsupported review model gpt-5.6-luna`。这是审批基础设施故障，并非用户拒绝或不安全判定，没有绕过审批。

维护工具 `recover-shortcuts-service-media.cjs --check` 用 167 个当前 SVG 校准文件名指纹，仅扫描当前产品、当前应用与本地 assets 的 2,780 个 SVG，未找到关闭图标 `130e45fb` 或卸载动态点 `d5d9ac9c`。Armory 原始产品位图也未补齐。关闭图标仍使用专项记录中披露的共享 fallback；产品图没有换成 Dashboard 缩略图，动态点没有凭空重画。

## 后续推进位置

1. Macro 持久身份、共享仓储、快捷键选择/playback 和未保存提示已由 [后续集成](macro-library-integration-2026-10-05.md) 接入；嵌套宏身份引用也已接入，仍需处理其菜单剩余细节、完整 Sequence/Phased 创作及服务数据边界。
2. 继续逐产品补控制项、条件分支、弹层及动画；3946 类型菜单已在同一后续批次接入，3907 已连接分支和服务行剩余细节继续保留为缺口。
3. 为服务正式 UI 接观测和刷新适配器；保留记录缺失与已观测空集合的区别。
4. 原资源下载能力恢复后补齐精确原图/图标；独立 Chroma Studio 等缺失源代码继续先取证，不猜写页面。

完整剩余范围见 [尚未完成的 UI](remaining-ui-work.md)。本记录取代本批子任务里“等待主任务统一检查”的临时描述，不改变总体未完成状态。

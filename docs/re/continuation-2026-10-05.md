# 2026-10-05 本批续作与校验

目标仍是完成全部现行 Synapse 产品/UI 复刻；本批不把路由登记、存在 iframe/root 分支或可编译等同于逐产品完成。`display-mode-roots-audit.json` 已改为明确区分“源码分支数”和本地 UI 部分覆盖，避免原输出 `226 products, implemented` 被误读为 226 款已完成。

## 本批落地

| 范围 | 已实现内容 | 详细记录 |
| --- | --- | --- |
| 首页 Dashboard | 双应用导览横幅/关闭偏好、八模块直开、名称/版本/配置状态、电池基础条件及2秒延迟、空卡、间距/箭头/宽度与拖动边界 | [Dashboard](dashboard-review-fixes-2026-10-05.md) |
| Devices & Modules | 主路由接四类源分组、五项真实目录、源语言文案和行内详情；无真实观测的服务组不虚构条目 | [设备与模块](devices-modules-integration-2026-10-05.md) |
| Global Shortcuts | 行内录制、三点菜单、重复提示、删除、八类映射入口及实际级联定位；文本输入上限/粘贴、程序和网站独立状态；键名直接由当前 AST 提取 | [快捷键](shortcuts-ui-integration-2026-10-05.md) |
| Gamer Room | 子设备数据分支、状态浮层、Power/Override请求、产品入口；常驻收展动画/help/营销hover与原图标；恢复36×36热点原SMIL动画 | [Gamer Room](gamer-room-current-review-2026-10-05.md) |
| Chroma | 导航pill、预设尺寸/可见名称、共享导览横幅、分组动画/响应式及部分设备状态；纠正Studio不是Dashboard | [Chroma](chroma-review-fixes-2026-10-05.md) |
| Audio chromaApp | 28款按各自root→Lighting组件证明接入，其中1465使用实际LIGHTING key | [Audio独立模式](product-mode-integration-2026-10-05.md) |
| 独立 Armory | 1303/1304/1313产品图root、3893独立CoolerInfo、限定Accessory/Mousepad类别的3894图片root | [Armory](armory-independent-roots-2026-10-05.md) |
| 键盘740/746校准 | GUID工厂配置判定、警告/弹层几何、进出/悬停/加载时序、正确条件状态 | [校准](keyboard-calibration-review-2026-10-05.md) |

上述各行都有未覆盖的内部条件，详细记录保留明确边界。已实现入口直接打开；未实现Studio没有继续冒用Chroma Dashboard作为替代页。

## 最终校验

- `cargo fmt --all`：成功。
- `cargo check --locked --all-targets`：成功，包含最后的快捷键键名修正与动态热点代码。仅现有 `customize_page::layer_button` 未使用方法警告。
- `python tools/validate-resources.py`：1087项源/输出哈希及资源格式、嵌入引用通过；含133条Webpack图片请求、74产品变体、59 Dashboard变体、16键盘布局/1901输入形状。
- `python tools/validate-embedded-json.py`：31份已检查JSON无失败；另有3份原有文档因静态类型解析器不支持而跳过，不把它们算作通过。
- `python tools/audit-locale-keys.py --check`：427个字面调用键检查通过，没有新增缺失键；动态调用、已有soft fallback及各语言原先不齐全的部分仍是检查范围限制。
- Dashboard、共享横幅、校准、快捷键、Gamer Room、SMIL、displayMode roots及离线恢复的专属静态收据检查通过；逐页审计和Chroma/Armory源合约收据已刷新。
- 没有运行应用、构建命令、测试、安装程序、Razer JavaScript或DLL。上述所有检查都是用户允许的类型检查、格式化、静态解析与资源验证。

## 原资源恢复和剩余缺口

热点原SVG通过当前manifest内容指纹离线恢复：166个已有当前SVG校准了MD4前8位命名，原始候选精确匹配当前 `53dd5566`，随后静态解析8个SMIL节点并直接生成动画参数。该方法和完整哈希见[离线恢复说明](current-media-offline-recovery.md)。这不是重新联网后的独立逐字节验证。

仍未完成的主要范围包括产品专有内部控制/弹层、3907及3894 DEFAULT Armory、快捷键部分映射体和emoji面板、真实IoT/安装/固件/遥测服务条件、Gamer Room背景模糊和部分持久化/文本细节，以及独立Studio源代码。

Chroma的15个灰色/动画SVG、Dashboard的3个专用电源/控制器SVG及新Armory产品图仍缺原文件。普通网络请求失败；部分提权下载被自动审批服务503阻止执行，返回明确不是“不安全”判定。已经能由当前缓存证明的资源均继续恢复/校验；其余未猜画或拿其他页面的图替代。

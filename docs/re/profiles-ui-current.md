# Profiles 当前界面与本地传输契约

来源为当前 `.ref/applications/synapse/profiles/`，模块43的 Ba挂载Games(oe)和Devices(Ga)，工具栏Xt使用独立应用历史。共享URL正则不是额外导航。设备关联链为 Ga→$n→Ua→7693/I；两处Add都走3137/f→5529/r。完整源范围、SHA、语言和CSS见文末证据；静态契约不代表运行验收。

## 页面、卡片与关联目标

- 全局Roboto16/`#ccc`，局部字号按源。导航外高52（28+22+2），Games绝对正文从50开始，Devices跟随52；Games drag-area最低900，Devices没有同样限制。
- Games卡为290×220、body150/footer70；关联卡为240×190、body120/footer70；Devices卡290×220、图片250×140。关联卡checkbox使用源两段3px/100与200ms几何，active/busy及添加卡页脚各自处理。
- Games清空搜索会收起输入，Views/Order区域点击不提前关空搜索；Add清空只清文字。Add/Search使用源即时tooltip。Scan/Refresh/Browse未有真实结果时保留不可用边界。
- Add根只有fixed-header和内容rows，不添加工程空目录文字。当前f不向r转发goBack，因此无额外返回按钮；来自设备页时关闭仍回设备弹层，no-popup切换保留。
- Add/DeviceGames遵守各自backdrop、标题36、关闭36/图20、100ms opacity/300ms位置与媒体条件。两种实际挂载祖先都用 `min-width:0;width:calc(100vw - 40px)` 取消公共800px最小宽；DeviceGames和其Add子层≥1600时采用源1300px。窄窗不再人为强制800px；主Games的900px内容限制仍单独保留。

`ProductWorkspace`提供真实本地设备/配置实体。弹层订阅传入workspaces，吸收后续已知游戏关联；解除关联后曾见过的游戏留在本次All列表，不推断为卸载。Linked Games只包含当前任一workspace中实际存在的关联；Removed需要真实removed/missing状态。全局gameList/扫描目录仍未连接。

Profile下拉只改关联目标，不激活硬件配置。目标更新时保留仍存在的选项，否则回退实际active/首个可见项，清除旧重命名和延迟删除。重命名限32 UTF-16，ECMAScript trim、大小写敏感同名检查，Enter/Blur提交、Escape取消；最终元数据层的Rust trim及超长输入拒绝与浏览器maxlength仍有适配差异。

## Profile本地操作

Add、Duplicate、Rename、Delete均已接入本地草稿，完成后刷新关联目标。Add按主机名生成默认名；Duplicate保留产品设置和opaque source_settings，以当前1867/d9去编号/唯一名称规则命名，游戏关联使用独立GUID域。ID避开工作集合及已保存基线，不因删除新增复用。

Delete等100ms后显示红色确认，外点/Escape可取消，确认才改集合；至少保留一项，删除活动项才恢复下一项。删除前按已有mapping_dirty和AudioEditor.changed拒绝丢弃未提交映射/音频编辑。名称查找超过源100次后继续找唯一名、至少保留一项属于本地完整性约束。

这些操作不发ON_ADD_PROFILE等服务命令，不表示写入硬件。关闭弹层后可通过顶部未保存配置入口保存本地草稿；错误和状态有稳定test-support标识。

## Import/Export弹层

Ua→Ia→Da/Ea使用独立retained实体：602×481、顶部104、36/49/47区段，标题沿源始终Export Profiles。原始5个SVG、警告/宏tooltip、300ms按钮透明度保留；slide-off/on是display切换，不造高度动画。Cloud呈现源禁用设备选择和in-development。

Export初选全部本地配置并排除源特殊GUID，支持逐项/全选；提交再次检查最新ID。Import用本机文件选择读取.synapse4，预览有效配置和关联宏，逐配置保存profile/name/isProfileSelected/hasWarning/fileType/listItems，各宏guid/text/isMacroSelected跟随行；同宏在A/B配置的选择不能被并集覆盖。

busy防重复picker；关闭或Local/Cloud切换递增代际，迟到结果不写入；取消文件选择保留上一次有效选择。父层关闭dismiss子层、取消删除任务并恢复焦点。此传输层按源没有Escape、Enter或遮罩提交/关闭行为，不能套用普通删除框语义。

提交当前只保留内存TransferIntent/选择并可撤销；没有应用导入到配置/宏，没有导出官方文件，没有持久化intent。UI明确显示“尚未应用”或“官方文件尚未生成”。本地.razer-ui-profile.json和产品独立传输根不是这条.synapse4转换链。

## 解码与校验

- 外层文件先按FileReader UTF-8消费一份BOM、替换无效序列；内层base64按7207将URL-safe -/_归一，清除包括padding在内的非base64字符，再严格解析UTF-8/JSON。
- 1867/D先克隆解码对象，移除顶层hash/gamemode，按5371 stable JSON序列化并MD5，再与外层hash精确比较；验证之后才覆盖外层profile name。
- 对象键按JS UTF-16字典序手工输出，数组保持顺序。普通有限数按[1e-6,1e21)定点/其他指数规则；极端最短浮点表示不保证完整ECMAScript等价。哈希不匹配拒绝对应条目，不跳过校验。
- category/PID/dongle/BLE兼容性来自逐产品MW证据。有限数字2636与2636.0相等，数字字符串不相等。Da的dynamicKeyStrokeGroup/is8kAnalogDevice遵守JS真值：0/-0/空串为false，空数组/对象为true。
- 16MiB为本地解析上限；异常孤立surrogate、非有限数及过深嵌套拒绝，不生成未校验配置。

## 剩余项

导入应用到本地配置草稿、宏集合导入、完整产品规范化、官方envelope编码和文件落盘仍有实现缺口；本地配置、外部文件和设备/引擎持久化分别核对。Da的Snap Tap默认mode、dongle polling默认值需要真实DeviceInfo/DEFAULTPROFILE；Synapse3 SharedWorker迁移未实现。

Games非空正常/removed/missing卡片、详情/封面、安装程序列表/扫描/多选exe/url Browse/拖放、IOT子设备及editionName/图标、Last/Most Played仍缺真实输入或实现。现有DeviceGames关联订阅不替代全局安装目录。窄窗flex收缩、溢出导航、正常行高、通用Select过渡、特殊GAMEPAD菜单条件、删除框DOM溢出/额外高度、window-blur、焦点/IME/缩放/tooltip裁剪与运行视觉继续待验收。

## 维护证据

- [应用与语言](profiles-app-audit.json)：`audit-profiles-app.cjs`。
- [正文、菜单和图标](profiles-content-current-evidence.json)：`audit-profiles-content.cjs`。
- [传输组件、编解码和兼容身份](profiles-transfer-current-evidence.json)：`prepare-profiles-transfer.cjs`。

编解码回归和真实弹层test-support仅参与允许的全目标类型检查；没有运行应用、测试、下载JavaScript/worker或DLL。源码/资源收据和native指纹保留，但不作为完整Profiles验收证明。

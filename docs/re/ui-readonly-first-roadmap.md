# 逆向与实现路线图

本页只维护当前阶段、方向与下一步。工程规则见[AGENTS.md](../../AGENTS.md)（方向见第1.3节，体积与工具见第7.2—7.3节）；详细证据归 `.analysis/results/` 正式收据，产品语义归[产品文档](../products/README.md)。

当前目标：优先让鼠标、键盘、耳机的全部范围内本地功能与当前官方版本对齐，按代码变体复用逆向证据，逐项完成独立 Rust、原版 UI、真实设备／服务交互和验证闭环。

**第一阶段指第一梯队产品的完整交付。** 下文的源码、静态、Rust、接线和对照是每项能力内部的验收环节，不是可以代替产品交付的阶段目标；逆向报告、身份索引或没有生产调用方的模块均不能结束第一阶段。

## 当前状态（2026-11-10）

- **共享接收器已接生产消费者**：鼠标 `/mapping_receiver_physical_session_production_progress`（`66c2d4061e48c20156f7d3f67e8344af0119d9dc43d156a7c1dbae20c590b7b6`）接通14产品真实无线别名的物理会话、页面Save、后台投影、发布确认、回调和退出；只确认投影实际包含的成员，旧连接结果与失败拓扑不能沿用。同PID的148／185／224以 `/mapping_same_pid_dongle_owner_correction`（`5cd3e90debb508c9a71a66914126c465ae4ba352fd7eda490d6e5c95837787ba`）修正提前启动直连owner的路径。Discovery 6项、页面1项、服务15项、Shell 5项及Shell全目标检查通过，更正后的重连4项另行通过；实际设备／官方业务／窗口未验收，105双身份、键盘从设备、Mika／多设备分支继续推进。
- **键盘worker与耳机DSP执行范围**：键盘 `/keyboard_obm_worker_realms_and_relative_macro_delivery`（`0210c37b606f0cf49efbfc50e55482b48b7cf2c135a7ad59028738d72fc65ad5`）核对42产品worker实际依赖，修正740／746相对宏位移的零值过滤顺序，4项测试通过；完整OBM初始化／转换／差异写回继续。音频 `/headset_thx_apo_execution_graph_boundary`（`f81bbf9d2ae039a6bec050b2f3de3a569ad883ec217e27a557f32cfef086c8d7`）接回八产品182个当前APO执行／释放函数，原生索引逐产品绑定八个实际原件；DSP数值语义、独立Rust和完整Sound尚未闭合。原生索引产品范围／数组入口的11项及前端审阅30项验证通过，未新增捕获。

- **本批鼠标与键盘实现**：鼠标 `/mapping_ordinary_default_variants_production_delivery`（`a1d00228613545c2e7c1773ae490290ace8d740358f719f2ebc2bad080f21894`）接通96／112普通物理连接的实际Save、持久化、回调与OTFS恢复；96报告表来自初始化文档，112保留原版导航字段大小写与缺省参数。此前十二产品段（`abbcc63b1976ae5f97ca9dea03d10fcaab9e36c265b6df917f883124b55f91b1`）已补入当前阶段映射。键盘 `/keyboard_obm_inventory_current_schema_delivery`（`2b1b30640bcda7bf971f89c0097ee83b5db66e323279a2e4380886b6328b2986`）按42产品实际引擎区分首槽读取、六字节映射哈希与失败保留，并接回原有消费者；完整OBM初始化、转换、差异写回与恢复继续推进。新依赖下85项鼠标／共享编译器／键盘设备测试通过，包含在途接收器投影的两项纯测试，不由此宣称接收器生产会话已完成。
- **实际只读与官方HID对照**：键盘收据 `/keyboard_obm_readonly_runtime_acceptance`（`427372931be7512dc849a2c6e864370935cd54c7928929d5ff98440a4f7cb9bc`）记录190经191接收器的8项查询，以及679的6档案、82键和全部档案映射哈希读取通过；实际宏目录为空，非空宏正文未验。51个已准入GET的官方HID导出结果逐字节一致，235个其余GET仍在该对照入口范围之外。首次键盘读取因收到不匹配灯光应答失败，原始失败和日志保留于 `/keyboard_obm_readonly_runtime_validation`；用户退出官方Synapse后复测通过。未执行设置写入、主应用或安装器，整项业务DLL／窗口／写入验收仍未完成。该次实机结果绑定依赖更新前的快照，不自动扩展到后续依赖及实现。
- **阶段增量复用**：台账已建立静态范围指纹；仅追加交付证据时，核对全部已索引段的canonical身份后复用原静态结果。产品范围、来源、挂载、角色证据及已审阅段变化仍要求重新传播；缺段、重复段和并发字节变化拒绝复用。30项身份／台账定向测试通过，本批鼠标、键盘、耳机及实际只读证据均已回接并生成。此次测量定位到队列查询反复扫描1651648行，已补变体索引；仅队列重建部分从905.662秒降到0.139264秒，6742行前后逐字段一致，验证事务已回滚。计时与校验保留在传播收据的`maintenance_timing`，没有重复捕获原件。

- **BlackShark编辑与音频独立性**：音频收据 `/blackshark_local_eq_editor_notification_delivery`（`0234da7b5228351165d60e71c25a1f113970c1a347ea72d4ac1724f03cb7840d`）接通六产品的库列表拖放、未知ID保留／移除、搜索焦点与清空、Reset资格，以及实测字段／草稿／通知分离；新依赖下6项设备、4项服务、5项页面验证和最终1项多active标志回归通过。2154字节原版图标已发布并回读安装目录库，[实际清理收据](../../.analysis/results/scratch-cleanup-blackshark-local-eq-editor-final-plan-applied.json)确认删除上传副本并释放146162968字节，失败0。五耳塞激活码仍以 `/earbuds_activation_independence_correction` 为当前边界，只验收读取／复制。`thx_direct::LocalRpc` 仍依赖官方THX业务服务；完整空间效果／DSP独立实现、完整Sound与实际窗口／设备对照继续未完成。
- **代码身份已建库**：[前端索引收据](../../.analysis/results/code-identity.json)与 `.analysis/results/code-identity.sqlite3` 覆盖第一梯队199产品的398个保留部署；32149份不同字节的产品JS产生269194次factory出现、56860个规范化factory身份。原生含已接回共享依赖共203198函数、192684个保守代码身份，缺原件／捕获与未确立加载的容器候选单列。必要共享宏应用已按独立命名空间接入：[宏索引](../../.analysis/results/code-identity-synapse-macro.json)含30个源文件、359代码身份，不默认绑定全部产品。
- **当前挂载已接回**：[导航收据](../../.analysis/results/code-identity-navigation.json)覆盖199产品、1077个完整occurrence。此前12项缺口已按真实源码修正：580／747的ASCII名称转换，以及10款鼠标构造器中带BLE条件的导航push；没有按漏解析删除功能。当前全部934项页面能力有当前实现根依据，843种实现根身份保留了真实条件、props、同名遮蔽和选择消费者。完整页面交互及依赖仍未闭合。
- **能力阶段**：[当前台账](../../.analysis/results/capability-ledger.json)已登记75款鼠标399项、73款键盘320项、51款耳机／适配器456项，共1175项；静态闭合26项、部分289项、未开始860项。Rust有26项implemented（当前Windows范围）和147项partial；UI／后端173项partial，实际对照173项unverified，其余尚未登记交付证据。全量剩余变体数仍未知；分项硬件读取及HID字节一致不能提升为整页实际对照通过。完整第一阶段尚未完成。
- **证据归并**：[传播收据](../../.analysis/results/code-identity-propagation.json)已归并92段、118023个所有者，其中107971个绑定5200种身份，9647个当前非factory范围与405个其它来源范围单列，已映射段invalid为0。当前源码签名仍为`4c92a414ba527523cb8fda9869d8303b1826755d6a177c10309602ef791e1b3b`。实现验收只进入stage证据，不混入静态来源映射；旧段保留上游引用，已被后续完整证据覆盖的范围不重新派工。
- **真实语义推进**：鼠标75产品参数、四产品多槽对应、当前HID/host队列及关闭边界已发布；键盘73根组成、73产品任务队列内部语义与719亮度分派/应答链已发布。耳机THX变体、版本代码复用、33消息／16方法RPC契约及Node/FFI交接已分别发布；参数计数、raw preset id解析和未调用的Terminate均按真实失败行为记录。完整Customize／Lighting／Sound仍继续，未以局部结论进入产品交付。
- **行为队列**：[传播收据](../../.analysis/results/code-identity-propagation.json)当前记录6204个行为候选（1036无审阅记录、5168局部），610个纯字符串依赖已分离。这是已关联的代码候选，不是剩余功能数量；压缩命名、chunk编号及模块搬移按身份与实际调用证明复用。
- **共享词法与引用复核**：生成器参数／var作用域、循环绑定重命名及公开生成器名称已修正。紧凑所有者引用、共享应用非factory范围、shared deployment签名及additional complete-owner引用的37项定向测试通过；每个冻结引用独立核验，冲突不合并，原件与引用的字节/SHA门槛保留。现有索引继续复用，不重新捕获整套源码。
- **当前精确边界**：HID SEND与GET的reportId规则已分别核对；键盘73产品物理亮度资格、BLE/Mika/DualLink选择、profile-loader及事件生命周期已有分项静态证据，后续按实际输入接线，不能重开已闭合传输。THX旧1.0.3.1原件及15导出捕获保留；新版RPC/FFI结论不能直接覆盖旧版全部客户端。
- **自动暂停已接产品链**：[音频收据](../../.analysis/results/frontend/audio-mw-audit.json)的`/auto_pause_rust_ui_backend_acceptance`（`bcc864838081c9a9ba6c00adae7a6482e725b2fa3fcecd9c0dfad0e2ddb795b0`）及`/auto_pause_notification_stop_acceptance`覆盖5产品实际页面、统一profile保存队列、HID真实请求／应答、事件、重连、物理事务计数和取消释放。首次档案在真实读取与源码初始化门禁后创建，使用完整typed模板、新GUID与computerName-Default；旧缓存／模板不冒充设备值。验收包含40协议、13存储、9服务、4个mock GPUI用例及全workspace check；后续DND共享存储扩展的15项测试另行通过。
- **DND已接Windows控制链**：同一音频收据`/do_not_disturb_capability_static_closure`闭合当前5个注册产品／4变体；`/do_not_disturb_rust_ui_backend_acceptance`（`48459502598b48ff24e406be18cad68116757818c005d6288c33ad878ece3535`）记录GET→相等跳过／不同SET、最新实测与写失败分离、保存／UI及恢复顺序。44项协议、15项存储、7项mock GPUI及全workspace check通过，实际对照和整页邻接功能仍待验收；不扩展到仅携带同factory的其它产品。
- **鼠标当前边界**：[Customize收据](../../.analysis/results/frontend/mouse-customize-command-variants-review.json)在当前Host4.0.896／Mapping1.3.17.2资格证明之后，新增`/mapping_ordinary_dispatcher_interdevice_routing`、`/mapping_current_native_json_hash_boundary`、`/mapping_ordinary_business_consumer_dependencies`、`/mapping_current_native_nested_descriptor_parsers`及`/mapping_current_native_input_descriptor_classes`。这些当前段已核对75产品的实际普通dispatcher／常量／跨设备接收者、业务任务直接绑定、JSON/hash边界、嵌套output/config及全部十类input descriptor；精确canonical和冻结review由各段及台账映射保留。递归OTFS、DPI／profile完整恢复、实际ScrollModeV2／Chroma及物理消费者仍需闭合，完整Customize与Rust／UI／实际对照未结项。既有键盘analog/digital选择与cache、父编辑器、提交应答、准入、队列和Profile缓存继续复用；不从上游旧pending重开已覆盖分支。231当前repair为false；Armory仅排除已证明的联网分支。
- **键盘当前实现**：`/keyboard_all_direct_brightness_delivery`（`9525d3ba3f5dec53b5c1f75f1093d4608b504962168ab774613db8472beb17e5`）已接通73款logical产品自身PID的HID亮度能力及原有页面／IPC／服务消费者，六款legacy使用独立取整规则，551／585等自身dongle别名按真实物理factory校验。54项device、3项discovery、23项页面状态测试及Shell全目标检查通过。既有14款BLE与18款自动熄灯交付证据继续保留；756 BLE自动路径排除。独立692／Mika接收器、硬件首档案初始化及完整Lighting仍未完成；实际OS／设备／窗口对照未验证。一次宽过滤命中的无关DeviceWorkspace控件测试持续未完成后已中止，未计作通过。
- **耳塞当前实现**：`/earbuds_enhancement_rust_ui_backend_acceptance`（`69cc5d230813323f7abad983857426b0ae036fa6c29e44f72263c1ce0c95e2c3`）验收5产品的26项注册能力，包括2657 ANC；统一62项device、11项service、26项页面测试通过，复用未变19项存储测试及当前Shell全目标检查。涵盖严格gate／FW／restore顺序、同GUID更新、typed Unknown及物理ANC cache隔离。四项媒体（54003字节）已发布至`d6aaa0f7168be86c560ef0697b95914771a62671`并核对安装SQLite别名；`/earbuds_gesture_media_distribution_acceptance`保留发布证据。实机／窗口验收仍未完成。
- **耳塞EQ卡片已接生产页**：[音频收据](../../.analysis/results/frontend/audio-mw-audit.json)的`/earbuds_equalizer_card_ui_backend_progress`（`9e238d6d1a21205774385dda92d3872fbc343c29facced0a2dbba3cf23c897c5`）在既有曲线／档案／保存／设备队列上补齐2657／2660／2664／2668／2689的原版compact卡片、实际配置档顺序的预设、按该档案默认值Reset、badge、帮助／禁用提示及150ms曲线／200ms bubble／300ms提示过渡。63个JS所有者、43种相关CSS规则及五产品绑定已核实，旧50段canonical保持不变；同身份停用时清空提示与图表并递增代次。最终36项耳塞headless测试和Shell全目标检查通过；动态视觉、实际窗口／设备／安装运行均未验收，完整Sound保持partial。
- **耳塞系统音量已接生产链**：同一音频收据的`/earbuds_system_volume_current_chain`（`7621e3c1018712023bcba53e6e3f0f2f7a40e51bf107854d539819980c8b89d4`）闭合上述五产品完整静态链；`/earbuds_system_volume_rust_ui_backend_acceptance`（`35cef89d43d52fe42f3a1064bbd91ffad7fc771d17ef3650e186e73fe9c20f00`）连接实际container端点选择、首次只读、完整UI Setting、volume→mute及真实读回、默认speaker通知消费者、Storage_NoSave与取消／注销／等待回调／释放。实测、乐观草稿与drag preview分离，读取失败不显示旧值；原版逐字段prop变化同步保留。3项device、9项service、40项耳塞headless及Shell全目标检查通过，包含真实轨道按下／外部释放和旧subscription拒绝。两个当前原版SVG已发布至`b453a88599c1fe1a36ca7a415ee037ce5693f18e`并回读安装目录库；完整Sound仍partial，实际Core Audio／设备／官方功能／窗口／安装对照均未验证。
- **鼠标当前档案兼容**：182／190实际读取入口已复用共享13→14迁移，接受当前schema14并保留原始LevelDB字节哈希、顺序号和完整配置字段；226等其他产品保持其独立版本范围。`/mouse_current_schema14_catalog_delivery`记录63项存储测试通过，1项显式安装数据测试未执行；不触发设备默认写入。
- **媒体发布收尾**：四个远端SVG正文共54003字节已实际获取并逐项核对长度／SHA，安装目录库已回读四个别名。Shell全目标检查通过；[发布清理收据](../../.analysis/results/scratch-cleanup-earbuds-media-published-plan-applied.json)实际删除已验证上传仓库，释放146260731字节。历史凭据失败与后续校验事实保留，窗口及设备验收不由资源交付推断。
- **本批体积与时点**：[体积收尾收据](../../.analysis/results/workspace-storage-earbuds-system-volume-plan-applied-summary.json)记录2026-11-07 12:28:23 UTC盘点，实占`.analysis`353.410537→353.413369 GB、`target`19.223126→0.000603 GB、`.work`43.210846→43.218565 GB，三目录均无扫描失败或重解析点；并发分析变化不计作本次清理释放。前值盘点后继续完成了输入确认层定向验证，因此Cargo执行前值与计划快照不同。本批超过5 GB的构建占用来自此前清理后的定向测试和Shell重建，现已清除。
- **本批清理已执行**：[Cargo执行收据](../../.analysis/results/workspace-storage-earbuds-system-volume-plan-applied.json)记录实际释放19623618268字节；[恢复后的临时文件清理收据](../../.analysis/results/scratch-cleanup-earbuds-system-volume-resumed-plan-applied.json)删除已正式归并的system-volume审阅JSON，释放234989字节，失败0。共享媒体上传副本已由[鼠标清理收据](../../.analysis/results/scratch-cleanup-mouse-customize-20261107-plan-applied.json)删除，本轮不重复计算。旧耳塞计划因包含已删除共享副本保留为未执行；原件与正式收据保留，未发布OTFS recursive／transition证据与helper仍有后续依赖。
- **输入确认层增量**：[冻结验收](../../.analysis/results/reverse-pending/review-sources/2b6dd9b844b351fea60686ce51d32e12abbcf386fea3dd6e15ef4df6c077317d.json)核对五产品同字节警告图标与CSS，接入资源作用域延迟绘制及命中遮挡；2项定向测试包含内部点击、外部右键、触发焦点和空格，最终Shell检查通过。正式音频段`/earbuds_input_confirmation_layer_acceptance`（`648016cbf3fd43d91257fff09709b99af4acd59b382a62ca038a2f926ffcc41b`）已接入五产品Sound阶段映射，生成台账待同批后续发布合并刷新；真实窗口视觉／焦点顺序和设备对照未验证。
- **输入源断开提示已接线**：音频段`/earbuds_input_disconnected_state_acceptance`（`4dfc6ca54b3723f2bc4131454fdc740ccbb12933656ac13d9455a599c76d452d`）接回五产品同源断开包装层、指针提示及页面／连接清理；共享EQ／输入提示修正了窗口坐标与父元素坐标重复叠加。43项耳塞测试通过，最终新增五产品坐标断言的定向测试另行通过；Shell检查与本批稳定实现合并执行，真实窗口／设备对照未验证。
- **Sound本地系统集成已发布增量**：`/earbuds_system_integration_ui_backend_progress`（`2e2096d09cca647dae7303eb0f3d5c8e50ee8b6733333df4e09194cb4569016e`）接通五产品系统声音属性／音量窗口的真实按钮和独立Rust系统调用；2项定向测试以及上述43项耳塞测试通过，原版资源已交付。`/earbuds_system_version_source_closure`（`ee43f0e33b98bf55338414f13c65cc5a8b25c30850518635f211eca272391e4e`）已核对当前Electron／Node／libuv来源并将共享版本适配改为系统RtlGetVersion；14个纯分类案例通过，未知release与读取不可用分离。完整Sound、实际OS／DLL与窗口对照仍未闭合。
- **鼠标OTFS本地转换已发布**：`/mapping_ordinary_otfs_temporary_mapping_restore`（`5d91e7d542964ec36c7264b5c4fa537c9d4bd2799f42cbfb8364c62499eeed47`）已归并75产品快照、重复启用恢复、持久化／发布顺序与真实取消边界；新增1304个所有者和复用1837个所有者验证通过。当前Rust临时文档与恢复语义仍需连同生产消费者修正；完整Customize保持partial。
- **键盘模式初始化绑定已发布**：`/keyboard_setup_device_mode_current_bindings`（`e8e02e7d0ff5f35bf3851043f598745d54a594a5476d671800a3f4db83010680`）覆盖73产品285个实际调用点与1140个源码所有者；先GET、必要时SET(3,0)，72产品等待、739不等待。756 BLE758继承的getter读取本地标志而非硬件，真实协议提供者和关闭／OBM恢复链继续核对；不以该绑定证明硬件初始化已交付。

- **耳塞顶部图已接生产页**：音频段 `/earbuds_sound_graphic_delivery`（`844e642b524a79aa5b1aa35822fca1368cc54cb72dbf089f23c163e2deec8616`）接通五产品顶部双耳图、真实左右耳电量、断开提示与连接／页面清理。四项SVG共53578字节已发布到`d41d9386b1feaa3a27692d111cd3875bb10bb693`，安装SQLite四别名及SHA回读通过；45项耳塞测试及Shell全目标检查通过，真实窗口／设备／安装对照未验证。

## 能力内验收环节

以下环节按完整能力逐项执行；一项能力通过静态验收后立即进入实现与接线，其他能力继续并行取证。第一梯队的全部范围内能力完成交付验收，才代表第一阶段完成。

| 环节 | 工作 | 完成条件 |
| --- | --- | --- |
| 1. 确定范围 | 当前部署、身份索引、能力台账 | 每项能力有产品、版本／固件、传输身份；缺证据项有取证路径 |
| 2. 共享变体逆向 | 发现／身份、传输、配置档、映射／宏、存储、事件与生命周期 | 输入到释放全链有依据，结论按变体传播 |
| 3. 差异变体 | 只审真实差异的产品／固件／传输分支 | 无未解释差异与未知间接调用 |
| 4. 能力静态验收 | 合并该能力最新完整证明，只补真实缺口 | 该能力全部实际变体和必要依赖闭合，当前映射及台账已归并 |
| 5. 独立Rust与接线 | 协议、平台、服务、状态与原版UI | 真实读写与消费者闭环，官方业务DLL生产依赖清除 |
| 6. 分项验证 | 自动化／模拟测试与授权的DLL／设备对照 | 收据完整；受限项保持未验证 |

## 下一步

1. 耳机继续完整Sound。五耳塞输入源切换已接原版确认层、独立Protocol40命令与既有队列／服务，见`/earbuds_input_source_implementation_progress`（`2b1f2e38839450d1f60f602b344180e834f4370715242cd9d475c0e3e6823cbe`）。2项协议、2项页面测试通过，后者覆盖五产品实际按钮、禁用、外部关闭、页面切换及360px宽度；不保存、不恢复，不把ACK当成输入模式观察。Shell检查通过。完整Sound、资源／焦点／层叠验收和实机对照仍未完成；EQ与系统音量继续复用现有证据。Gesture四项媒体已交付。
2. 鼠标继续105界面／104业务双身份的真实协议与保存消费者、键盘从设备的转换／确认、Mika和Linker多设备分支；复用已接通的14产品物理会话、原版普通编译器及既有UI／队列。各完整分支证据齐备后立即接到同一生产链，不以接口或报告结项。
3. 键盘已接通73产品typed本地document／首profile与真实metadata代次加载，见`/keyboard_local_profile_initial_document_delivery`（`187a2ebf662ea6dc6176447274c9e5c4ad69507a6c3660c62ead6795bc2e39e9`）。版本12／13／14、edition、keymaps、旋钮和708多档案沿用完整配方；23项存储测试、17项亮度生命周期测试及Shell全目标检查通过，包含同路径断开重连拒绝旧回调。硬件初始化、设备模式及板载档案恢复仍须完成；本地文档建立不证明设备初始化或真实写入成功。
4. 691／692 OLED无线亮度已接入实际接口2／前缀0的Protocol25路径，见`/keyboard_oled_radio_brightness_delivery`（`2afc340ddfd1218db5c8acef9e50d9e238c3593662269d92567738ae9321c782`）；55项device、3项discovery测试及Shell全目标检查通过，最终扩展的OLED用例另行通过。实际DLL／设备对照未验证；后续只补未闭合的初始化与Lighting链。
5. Mika574／2308与2595／2593的主机多产品发现、便携键盘collection、metadata身份准入及亮度传输已接通，见`/keyboard_mika_discovery_brightness_delivery`（`9b77a7e8095aef9b661a4f571669d1edb627a1f3d76f420f4c65aa8e32426ac1`）。键盘56项协议测试、discovery全121项测试与Shell检查通过；使用真实`0xf0 | counter`序列，USB／HID存在不冒充无线在线。完整初始化／通知／Lighting和实机对照继续。鼠标`/mapping_peer_compiler_provider_bindings`已归并17个编译器变体的提供者绑定，剩余实际处理器差异继续复用现有证据推进。
6. 五耳塞EQ的独立固件命令、typed配置、先保存后提交、注册顺序恢复、原版曲线及compact卡片已接生产消费者，依次复用`/earbuds_equalizer_production_wiring_progress`、`/earbuds_equalizer_profile_lifecycle_progress`、`/earbuds_equalizer_chart_ui_backend_progress`及`/earbuds_equalizer_card_ui_backend_progress`。最后一段36项定向测试通过，覆盖真实预设点击／空格、非零档案默认值Reset、帮助型号差异、禁用与停用清理；Shell全目标检查通过。六项原版媒体共6771字节已发布到`ec0b428f7a15668b3dbef0148a9393783b19c3a3`并生成独立安装目录库，旧游戏图标复用已有别名。继续完整Sound与实际窗口／设备／安装对照；不把档案、ACK或headless测试当作独立硬件观察或动画像素验收。
7. 随完成目标归并最新收据；下一次派工前更新当前映射和台账，同批相邻发布合并传播与缺项刷新。维护代码只修复阻断实际工作的问题，整库体积盘点和重复构建不进入逐片段循环。

## 每批收尾

1. 槽位完成目标后立即发布；协调者核对canonical SHA、适用范围、上游引用及 `verified_artifacts` 不含被写收据，接回当前映射。同批相邻发布合并执行必要的传播、台账、定向验证与 `refresh-gaps --owner`；下一次派工前同步完成，输入未变不重复全库刷新。
2. 批末执行 `clean --dry-run` 与 `clean --apply <计划> --plan-sha256 <SHA>`（引用扫描需要 `rg`），并记录体积前后占用。
3. 用 `git diff --check`、Python AST 与文档相对链接检查收尾；失败事实写入收据。
4. 本页只写当前状态与下一步，删除过期分工与历史叙述，不新增平行进度文档。报告实际消除的阻碍、能力所处验收环节及剩余边界，不以工具调用、收据或测试数量替代交付进度；时间窗口到期也不得放宽完成条件。

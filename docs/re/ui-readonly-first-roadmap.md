# 逆向与实现路线图

本页只维护当前阶段、方向与下一步。工程规则见[AGENTS.md](../../AGENTS.md)（方向见第1.3节，体积与工具见第7.2—7.3节）；详细证据归 `.analysis/results/` 正式收据，产品语义归[产品文档](../products/README.md)。

当前目标：优先让鼠标、键盘、耳机的全部范围内本地功能与当前官方版本对齐，按代码变体复用逆向证据，逐项完成独立 Rust、原版 UI、真实设备／服务交互和验证闭环。

**第一阶段指第一梯队产品的完整交付。** 下文的源码、静态、Rust、接线和对照是每项能力内部的验收环节，不是可以代替产品交付的阶段目标；逆向报告、身份索引或没有生产调用方的模块均不能结束第一阶段。

## 当前状态（2026-11-07）

- **代码身份已建库**：[前端索引收据](../../.analysis/results/code-identity.json)与 `.analysis/results/code-identity.sqlite3` 覆盖第一梯队199产品的398个保留部署；32149份不同字节的产品JS产生269194次factory出现、56860个规范化factory身份。原生含已接回共享依赖共131341函数、126189个保守代码身份，缺原件／捕获与未确立加载的容器候选单列。必要共享宏应用已按独立命名空间接入：[宏索引](../../.analysis/results/code-identity-synapse-macro.json)含30个源文件、359代码身份，不默认绑定全部产品。
- **当前挂载已接回**：[导航收据](../../.analysis/results/code-identity-navigation.json)覆盖199产品、1077个完整occurrence。此前12项缺口已按真实源码修正：580／747的ASCII名称转换，以及10款鼠标构造器中带BLE条件的导航push；没有按漏解析删除功能。当前全部934项页面能力有当前实现根依据，843种实现根身份保留了真实条件、props、同名遮蔽和选择消费者。完整页面交互及依赖仍未闭合。
- **能力阶段**：[台账](../../.analysis/results/capability-ledger.json)为75款鼠标399项、73款键盘320项、51款耳机／适配器456项，共1175项；静态闭合26项、部分281项、未开始868项。Rust有26项implemented（当前Windows范围）和73项partial；UI／后端99项partial，实际对照99项unverified，其余尚未登记交付证据。键盘新增登记仅证明直接亮度路径，不能代表完整Lighting；第一阶段尚未完成。
- **证据归并**：[传播收据](../../.analysis/results/code-identity-propagation.json)已归并78段、58913个所有者，其中48966个绑定4035种身份，9616个当前非factory范围与331个其它来源范围单列，当前invalid为0。实现验收只进入stage证据，不混入要求静态快照字段的来源映射；旧段保留上游引用，已被后续完整证据覆盖的范围不重新派工。
- **真实语义推进**：鼠标75产品参数、四产品多槽对应、当前HID/host队列及关闭边界已发布；键盘73根组成、73产品任务队列内部语义与719亮度分派/应答链已发布。耳机THX变体、版本代码复用、33消息／16方法RPC契约及Node/FFI交接已分别发布；参数计数、raw preset id解析和未调用的Terminate均按真实失败行为记录。完整Customize／Lighting／Sound仍继续，未以局部结论进入产品交付。
- **行为队列**：[传播收据](../../.analysis/results/code-identity-propagation.json)当前记录5008个行为候选（1038无审阅记录、3970局部），610个纯字符串依赖已分离。这是已关联的代码候选，不是剩余功能数量；压缩命名、chunk编号及模块搬移按身份与实际调用证明复用。
- **共享词法与引用复核**：生成器参数／var作用域、循环绑定重命名及公开生成器名称已修正。紧凑所有者引用、共享应用非factory范围和shared deployment签名27项定向测试通过；原件与引用的字节/SHA门槛保留。现有索引继续复用，不重新捕获整套源码。
- **当前精确边界**：HID SEND与GET的reportId规则已分别核对；键盘73产品物理亮度资格、BLE/Mika/DualLink选择、profile-loader及事件生命周期已有分项静态证据，后续按实际输入接线，不能重开已闭合传输。THX旧1.0.3.1原件及15导出捕获保留；新版RPC/FFI结论不能直接覆盖旧版全部客户端。
- **自动暂停已接产品链**：[音频收据](../../.analysis/results/frontend/audio-mw-audit.json)的`/auto_pause_rust_ui_backend_acceptance`（`bcc864838081c9a9ba6c00adae7a6482e725b2fa3fcecd9c0dfad0e2ddb795b0`）及`/auto_pause_notification_stop_acceptance`覆盖5产品实际页面、统一profile保存队列、HID真实请求／应答、事件、重连、物理事务计数和取消释放。首次档案在真实读取与源码初始化门禁后创建，使用完整typed模板、新GUID与computerName-Default；旧缓存／模板不冒充设备值。验收包含40协议、13存储、9服务、4个mock GPUI用例及全workspace check；后续DND共享存储扩展的15项测试另行通过。
- **DND已接Windows控制链**：同一音频收据`/do_not_disturb_capability_static_closure`闭合当前5个注册产品／4变体；`/do_not_disturb_rust_ui_backend_acceptance`（`48459502598b48ff24e406be18cad68116757818c005d6288c33ad878ece3535`）记录GET→相等跳过／不同SET、最新实测与写失败分离、保存／UI及恢复顺序。44项协议、15项存储、7项mock GPUI及全workspace check通过，实际对照和整页邻接功能仍待验收；不扩展到仅携带同factory的其它产品。
- **鼠标当前边界**：[Customize收据](../../.analysis/results/frontend/mouse-customize-command-variants-review.json)已接齐父编辑器、UI提交应答、真实准入与队列、输入变体和已声明的Profile缓存范围。当前只补Generic表／overlay到映射编码、按实际P25／Legacy／BLE／DualLink路由的assignment写读，以及PostSet本地Mapping发布与释放。231当前repair开关为false；Armory仅已证明的在线上传／重分享状态分支排除，其余本地Profile保留。
- **键盘当前实现**：`/keyboard_all_direct_brightness_delivery`（`9525d3ba3f5dec53b5c1f75f1093d4608b504962168ab774613db8472beb17e5`）已接通73款logical产品自身PID的HID亮度能力及原有页面／IPC／服务消费者，六款legacy使用独立取整规则，551／585等自身dongle别名按真实物理factory校验。54项device、3项discovery、23项页面状态测试及Shell全目标检查通过。既有14款BLE与18款自动熄灯交付证据继续保留；756 BLE自动路径排除。独立692／Mika接收器、硬件首档案初始化及完整Lighting仍未完成；实际OS／设备／窗口对照未验证。一次宽过滤命中的无关DeviceWorkspace控件测试持续未完成后已中止，未计作通过。
- **耳塞当前实现**：`/earbuds_enhancement_rust_ui_backend_acceptance`（`69cc5d230813323f7abad983857426b0ae036fa6c29e44f72263c1ce0c95e2c3`）验收5产品的26项注册能力，包括2657 ANC；统一62项device、11项service、26项页面测试通过，复用未变19项存储测试及当前Shell全目标检查。涵盖严格gate／FW／restore顺序、同GUID更新、typed Unknown及物理ANC cache隔离。四项媒体（54003字节）已发布至`d6aaa0f7168be86c560ef0697b95914771a62671`并核对安装SQLite别名；`/earbuds_gesture_media_distribution_acceptance`保留发布证据。实机／窗口验收仍未完成。
- **鼠标当前档案兼容**：182／190实际读取入口已复用共享13→14迁移，接受当前schema14并保留原始LevelDB字节哈希、顺序号和完整配置字段；226等其他产品保持其独立版本范围。`/mouse_current_schema14_catalog_delivery`记录63项存储测试通过，1项显式安装数据测试未执行；不触发设备默认写入。
- **媒体发布收尾**：四个远端SVG正文共54003字节已实际获取并逐项核对长度／SHA，安装目录库已回读四个别名。Shell全目标检查通过；[发布清理收据](../../.analysis/results/scratch-cleanup-earbuds-media-published-plan-applied.json)实际删除已验证上传仓库，释放146260731字节。历史凭据失败与后续校验事实保留，窗口及设备验收不由资源交付推断。
- **维护耗时与体积**：本次75节／46344所有者合批传播实测101.148秒。1175项台账生成原耗时109.160秒；定向计时确认热点在每个非factory所有者重复执行的当前产品来源关联查询，改为一次产品来源集合查询后降至15.146秒，台账解析内容逐项完全一致，27项回归通过。未重写工具链，也未改变完成判定。输入未变不重跑。[前次盘点](../../.analysis/results/workspace-storage-earbuds-integration-plan.json)遍历5837488个分析文件，约377秒；[后次盘点](../../.analysis/results/workspace-storage-earbuds-integration-after-plan.json)记录实占`.analysis`350.56→350.59 GB、`target`15.92→22.75 GB、`.work`43.10→43.10 GB，后次逻辑大小404.22／23.96／43.10 GB。该区间构建增加约6.84 GB，仍用于在途验证；不逐片段清空重建，原件40.2 MB重复仅规划去重。
- **本批清理已执行**：[首批执行收据](../../.analysis/results/scratch-cleanup-completed-json-v2-plan-applied.json)、[projection执行收据](../../.analysis/results/scratch-cleanup-mouse-projection-plan-applied.json)及[codec执行收据](../../.analysis/results/scratch-cleanup-mouse-codec-v2-plan-applied.json)合计实际删除5个已结束JSON中间文件，释放29693104字节。后两项按引用依赖先后删除，首个codec计划的保留事实不改写。正式冻结证据、在用鼠标查询helper与storage-native-tail输入继续保留。上列体积属于对应盘点时点；批末再盘点，不逐片段遍历数百万文件。
- **本次体积复核**：[最新盘点](../../.analysis/results/workspace-storage-first-tier-controls-after-plan.json)实占为`.analysis`350.59→350.67 GB、`target`22.75→47.79 GB、`.work`43.103→43.110 GB，三项扫描均无失败。构建产物增加25.04 GB，来自本轮多crate／feature组合测试和增量编译，仍供在途验证复用，未执行Cargo清理；不将其计作证据增长。原件约40.15 MB重复仍仅规划去重，实际释放仅计上述执行收据。
- **MCP已实际收尾**：旧版THX客户端24份查询、398个选定函数及原件字节已复核，原IDA进程17108正常退出。统一计划已删除4个私有文件，实际释放210670904字节，目录已不存在；[执行收据](../../.analysis/results/scratch-cleanup-thx1031-mcp-plan-applied.json)失败0。官方原件13.53 MB、24份查询约15.86 MB与历史收据继续保留。后续复核复用这些正式输入，不重新创建已无必要的私有副本。

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
2. 鼠标槽完成输入编码、实际传输分支的设备写读、Mapping本地后处理与释放；已闭合UI、准入、队列及缓存只引用。完整静态门槛通过后立即迁移Rust实际调用方并接通页面，不停在分析报告。
3. 键盘已接通73产品typed本地document／首profile与真实metadata代次加载，见`/keyboard_local_profile_initial_document_delivery`（`187a2ebf662ea6dc6176447274c9e5c4ad69507a6c3660c62ead6795bc2e39e9`）。版本12／13／14、edition、keymaps、旋钮和708多档案沿用完整配方；23项存储测试、17项亮度生命周期测试及Shell全目标检查通过，包含同路径断开重连拒绝旧回调。硬件初始化、设备模式及板载档案恢复仍须完成；本地文档建立不证明设备初始化或真实写入成功。
4. 691／692 OLED无线亮度已接入实际接口2／前缀0的Protocol25路径，见`/keyboard_oled_radio_brightness_delivery`（`2afc340ddfd1218db5c8acef9e50d9e238c3593662269d92567738ae9321c782`）；55项device、3项discovery测试及Shell全目标检查通过，最终扩展的OLED用例另行通过。Mika574／2308与2595／2593仍需补齐共享接收器的真实身份与端点准入，不能以首个目录候选代替。实际DLL／设备对照未验证。
5. Mika574／2308与2595／2593的主机多产品发现、便携键盘collection、metadata身份准入及亮度传输已接通，见`/keyboard_mika_discovery_brightness_delivery`（`9b77a7e8095aef9b661a4f571669d1edb627a1f3d76f420f4c65aa8e32426ac1`）。键盘56项协议测试、discovery全121项测试与Shell检查通过；使用真实`0xf0 | counter`序列，USB／HID存在不冒充无线在线。完整初始化／通知／Lighting和实机对照继续。鼠标`/mapping_peer_compiler_provider_bindings`已归并17个编译器变体的提供者绑定，剩余实际处理器差异继续复用现有证据推进。
6. 随完成目标归并最新收据；下一次派工前更新当前映射和台账，同批相邻发布合并传播与缺项刷新。维护代码只修复阻断实际工作的问题，整库体积盘点和重复构建不进入逐片段循环。

## 每批收尾

1. 槽位完成目标后立即发布；协调者核对canonical SHA、适用范围、上游引用及 `verified_artifacts` 不含被写收据，接回当前映射。同批相邻发布合并执行必要的传播、台账、定向验证与 `refresh-gaps --owner`；下一次派工前同步完成，输入未变不重复全库刷新。
2. 批末执行 `clean --dry-run` 与 `clean --apply <计划> --plan-sha256 <SHA>`（引用扫描需要 `rg`），并记录体积前后占用。
3. 用 `git diff --check`、Python AST 与文档相对链接检查收尾；失败事实写入收据。
4. 本页只写当前状态与下一步，删除过期分工与历史叙述，不新增平行进度文档。报告实际消除的阻碍、能力所处验收环节及剩余边界，不以工具调用、收据或测试数量替代交付进度；时间窗口到期也不得放宽完成条件。

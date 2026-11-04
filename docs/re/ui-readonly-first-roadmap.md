# 逆向与实现路线图

本页统一维护当前阶段、已确认语义和下一批边界。用户要求的“先逆向、不扩需求、按依赖拆分、证据足够即收束、确认后实际清理”已写入 [AGENTS.md 第 1.1 节](../../AGENTS.md#11-当前执行优先级先完成逆向)及第 7.1 节；本页不重复维护工程规则。正式证据归 `.analysis/` 收据，未决项归 `gaps.json`。

## 当前阶段

当前处于阶段 1—3：核实当前来源、闭合共享调用链并解释真实产品差异；尚未满足阶段 4 的全量语义验收条件。Rust 迁移、UI／后端完整接线和实际 DLL／设备对照继续单独记账。

已有基线：[输入审计](../../.analysis/results/input-audit.json)于 2026-10-29 核对 279,961 项保留输入，缺失／不匹配／错误均为 0；[原生索引](../../.analysis/results/native-bulk/semantic-index.json)覆盖 881 个 DLL／NODE／SYS 模块；[950 项冻结队列](../../.analysis/results/reverse-pending.json)最初通过 889 项，余下 61 项由[后续增量](../../.analysis/results/reverse-pending/unresolved-current.json)复核通过。这些证明输入和选定静态复核结果，收据仍为 `semantics_complete=false`，不能换算为全项目完成比例。各产品现状见[产品文档](../products/README.md)，已有 Rust 验证仍按[独立收据](../../.analysis/validation.json)判断。

既有共享链、产品结论和验证事实继续复用，详细证据归[产品专题](../products/README.md)及正式收据。本轮从用户指定的上一对话接续，复核其未发布内容，再批量处理可共用依赖的剩余转换；没有重复启动原生捕获，也没有修改产品 Rust。

2026-11-04（Asia/Shanghai）本轮分两批发布以下八组静态结果：

| 功能链 | 正式结果与本次核对范围 | 结论与边界 |
| --- | --- | --- |
| 系统 IPF 激活与生命周期 | [系统收据](../../.analysis/results/frontend/system-mw-audit.json)的 `overclock_ipf_lifecycle_chain`；93个完整IL方法、3个原生函数。 | 激活先于硬件服务；30次retries最多调用31次。注册表写入后设备重启失败不回滚，SafeHandle释放不证明provider已停止。ClientApi／provider／mailbox原生链及实际安装绑定继续独立。 |
| AMD K17／K19／family1a Package | [配件收据](../../.analysis/results/frontend/accessories-main-audit.json)的 `hanbo_amd_k17_package_chain`；16个完整函数、6个PE槽。 | 单／多节点校准条件不同，校准仅在传感器创建时确定；失败／零样本保留旧值。多节点名为Package (Node %d)，不能冒充页面读取的Package；Node编号无额外加1。非空SMU owner+216来源仍待证明，既有Rust事实另记。 |
| 旧配置源产品映射 | [鼠标收据](../../.analysis/results/frontend/mouse-mw-audit.json)的 `legacy_product_mapping_chain`；11个完整模块及整份worker的直接8061导入核对。 | 6892侧板、8667广播器、8311音频三类实际源调用已核；只给专属转换传profile，deviceInfo并非必定注入。成功的专属转换实际可调用两次，失败边界和结果消费者按原件保留。 |
| 跨产品共享标量与开关 | 同一[鼠标收据](../../.analysis/results/frontend/mouse-mw-audit.json)的 `legacy_shared_scalar_chain`；25个转换及6个依赖模块。 | 一批核对键盘／鼠标／音频／系统／充电设置的真值、默认、NaN、取整、同字段覆盖顺序和三种catch差异。其余17个共享转换由本轮后续结果接续，不再按父收据旧pending状态重复领取。 |
| 其余共享结构化转换 | 同一鼠标收据的 `legacy_shared_structured_chain`；17个转换及依赖共28个完整模块。 | EQ／数组、相机、灯光、analog／dial、风扇／性能的实际形状、PID分支、覆盖与失败已核。`imported_shared_inventory_coverage` 核对当前52个导入模块恰为9＋1＋25＋17，无遗漏；该注册表静态覆盖已闭合。 |
| 其余源产品专属转换 | 同一鼠标收据的 `legacy_source_product_chain`；22个转换及依赖共30个完整模块。 | `source_product_inventory_coverage` 核对26＝1＋3＋22项。保留实际PID组、mixer初值区间与18步、相同函数正文但不同PID组、性能默认值、顺序及失败覆盖；该源注册表静态覆盖已闭合。 |
| 当前Host显示器枚举 | 同一鼠标收据的 `host_monitor_enumeration_chain`；22个JS owner、21个当前Host4.0.896原生函数。 | 两次枚举共享全局状态，首次空结果null与复用数组后的空结果不同；virtual按GetSystemMetrics计数，原版未检查部分OS返回。字符串读取异常先于FreeMalloc标志赋值，不能声称失败释放齐全。未选CRT反编译失败保留。 |
| PID2636退出与重连 | [手柄收据](../../.analysis/results/frontend/gamepad-mw-audit.json)的 `gamepad_2636_shutdown_reconnect_chain`；49个角色、68个完整AST owner。 | 当前defaults/bootstrap的generic feature列表为空；Sage继承空terminate，mode setter只回显成功而无传输。5×3连接尝试、重连亮度及唤醒电源顺序已核，退订不取消旧promise。未审阅外部注入仍独立，不据通用loader补造功能退出。 |

[最终工具验证](../../.analysis/results/tool-verification.json)为53/53通过，前一批47/47通过并由 `previous_receipt` 保留；两批共100项，失败／错误／跳过均为0，各批55项工具输入前后稳定。更早的79项验证仍在原历史链内，不计入本轮。八段均已正式发布和回读，四份父收据的旧字段完整保留；[gaps.json](../../.analysis/results/gaps.json)分别只刷新对应批次owner，四个受影响owner之外的3199项保持不变。已有 `Cargo.lock` 改动保持原样。

开发定向验证曾发现四处测试AST筛选过宽及一处K17节点编号草稿误读，均按实际AST／汇编修正后验证，未发布错误语义。IPF依赖核验曾拒绝未明确绑定的同字节包别名，已补齐实际字节与两份提取收据的对应关系。首次正式发布命令因不适用的 `--no-refresh-gaps` 参数在解析阶段被拒绝；改用该子命令支持的参数后四项均成功。最终通过范围只按当前统一收据判断。

后续结构化组的预检发现缺少局部import，首轮两处AST调用计数误包含外层IIFE，已改为精确callee检查。Host枚举首轮在setUp阶段因官方artifact所有者拒绝Rust路径而未执行测试，现将Rust身份与官方证据分开记录；零项执行不计通过。最后22项源产品转换14项定向测试首轮通过。原始工具输出保留在本轮对话回执，未虚构独立磁盘日志；最终53项统一结果有上述正式收据。

全量语义尚未完成。缺项中同时保留历史获取／分析器失败、静态语义、独立实现及运行验收，且父链的旧待办可能已有后续子收据；缺项条数、捕获数和本批测试数均不能当成项目完成比例。当前接续只按真实未闭合语义及最新结果，不重跑已完成取证。独立Rust、UI／后端和实际DLL／设备验收仍分阶段记录。

按2026-11-04回读的[产品结构收据](../../.analysis/results/product-documentation.json)，登记的331个PID均有源码输入、页面结构分析和现存产品文档，完全没有该层分析记录的为0。六族UI／MW[产品矩阵](../../.analysis/results/families/)的产品级语义状态仍停留在2026-10-17的pending，未归并后续功能链结果；不能将其当成当前“331个产品都没逆向完”，也不能用当前并行任务数代替剩余产品数。准确的逐产品全量语义完成数仍需基于最新调用链覆盖复核，当前不提供推测比例。

## 分阶段路线

| 阶段 | 工作 | 完成条件 |
| --- | --- | --- |
| 1. 确定当前范围 | 复用现有获取与捕获，核对当前部署、目录、挂载／初始化入口和依赖 | 每个范围内功能有产品、版本／固件、传输／平台身份；缺证据项有明确取证路径 |
| 2. 完成共享链逆向 | 优先处理发现／身份、传输、配置档、映射／宏、存储、事件和清理生命周期 | 输入、读取／初始化、观察、计算、写入、应答／读回、消费者、持久化、失败、取消、恢复和释放均有依据 |
| 3. 收束产品差异 | 共享结论接回鼠标、键盘、音频、手柄、配件／摄像头、系统、灯光和主机的真实分支 | 固件／连接／布局差异已解释，未知间接调用和必需来源缺口已解决 |
| 4. 全量逆向验收 | 从 UI 动作向下、从 dispatcher／服务事件向上复核，只补变化输入和受影响依赖 | 范围内没有未归属、漏审或未解释语义缺口，并完成本阶段清理 |
| 5. 独立 Rust 与接线 | 按确认语义迁移协议、平台、服务、状态和原版 UI，接通全部调用方 | 真实读写和消费者闭环，状态／生命周期所有权清晰，官方业务 DLL 生产依赖及失效数据清除 |
| 6. 分项验证 | 运行适用的自动化／模拟测试及已授权的 DLL／服务／设备对照 | 输入、双方结果、差异、恢复和清理均有收据；受限制项保持未验证 |

阶段 1—4 连续推进，阶段 2 与 3 仅在依赖独立时并行；不以编译、单页通过或工具测试通过替代全量语义验收。

## 当前接续边界

此前八组结果均已冻结、发布和清理。旧配置52个导入共享模块和26项源产品注册表、所选Host枚举及PID2636退出／重连路径不再重复领取；这些范围不等于整个worker、产品或运行验收完成。当前三个独立任务已从最新收据接续，未发布结果不计完成：

| 槽位 | 功能链 | 输入与完成条件 |
| --- | --- | --- |
| 协调者 | 正式发布、缺项与清理 | 串行维护共享结果；已通过的定向验证直接复用，只检查新输入、未闭合语义及受影响依赖。 |
| A | 系统ClientApi代理与回调 | 从 `overclock_ipf_lifecycle_chain/remaining_chains/1` 接续。当前ClientApi实际加载ClientApiProxy并协商ClientApiR4；沿真实选择路径获取代理原件，核properties/value/command、TLS错误及对象释放。复用managed生命周期，实际安装绑定仍单独判定。 |
| B | AMD可选SMU来源与真实适用分支 | 从 `hanbo_amd_k17_package_chain/remaining_chains/0` 接续。已定位初始化0x191c60调用0x209750分配CSMUInterface并写owner+216；继续核实际family/model配置、影响Package的字段／阈值及析构。不重做Package数值，也不扩非温度业务。 |
| C | PID756模拟输入采样注册 | 从 `keyboard_756_appengine_apply_chain/remaining_chains/0` 接续78127的skip-count重新注册、65200 MappingEngine provider、采样／回调／注销与计时器释放。相机／音频中已被后续子段覆盖的旧待办不重复领取；不扩Xbox展示或输出编译器。 |

其他Intel／owner32、完整导入发布／取消、各族其余功能及平台分支继续保留在正式收据和缺项中；达到可核验边界后转向独立项，不无限追加单一产品的邻接范围。已完成片段与仍待证明的实际分支分别判断；源码未取得或调用未解析时继续取证，不据此判定功能不存在。

## 每批收尾与文档维护

正式结果、引用接线和定向验证完成后：

1. 运行 `python -X utf8 tools/workspace.py clean --dry-run`，核对计划路径、SHA、候选、保留原因、锁和占用。
2. 仅对复核后的计划执行 `clean --apply <计划> --plan-sha256 <SHA>`；候选为空也保留实际 apply 收据。
3. 用 `git diff --check`、Python AST／编译和文档相对链接检查收尾；失败事实写入收据，不删除正式原件或未决证据。
4. 路线与当前阶段只写入本页，具体语义写入产品专题，机器可读结果写入 `.analysis/results/`；删除重复历史叙述、过期分工和失效链接，不新增平行进度档案。

首批[清理计划](../../.analysis/results/scratch-cleanup-20261104T113445013202Z-plan.json)与[执行收据](../../.analysis/results/scratch-cleanup-20261104T113445013202Z-plan-applied.json)保留。后续[清理计划](../../.analysis/results/scratch-cleanup-20261104T115745686190Z-plan.json)的SHA-256为 `beefa4fdbbc52ec92c2b6d279b2041960f379b8c7bf34644a8502445744e520c`，[执行收据](../../.analysis/results/scratch-cleanup-20261104T115745686190Z-plan-applied.json)为 `completed`。两批均删除0项、释放0字节、失败0项；八组查询均未新增临时副本、捕获或分析会话。175项既有内容依其真实引用／未决依赖继续保留，不能将保留项或预计节省报为已清理。更早释放5,432,542字节的事实仍见[原执行收据](../../.analysis/results/scratch-cleanup-20261104T105637132423Z-plan-applied.json)，不计入本轮。

本轮只更新维护查询／测试、五个产品／主机专题与本路线图，不新增平行进度档案。最终5份Python的AST、404个文档相对链接及 `git diff --check` 均通过；55项工具输入在发布后再次核对未变。未执行官方JS、业务DLL、应用、安装器、调试器或设备操作；应用窗口和实际硬件验收保持未验证。

# UI 源码复核与集成检查记录

旧会话 `C:\Users\han\Desktop\session.v4.jsonl` 已恢复：停止原因是 round-limit，不是目标完成。331个产品、1419个主导航页仍是部分实现。旧文件中出现某个CSS类或语言字符串，不能证明本地挂载和条件分支已复刻；Profiles五页签、宏缺少语言和资源、设备导航应额外包含历史箭头等旧结论已撤销。

当前仅使用 Dashboard `.ref/applications/synapse/dashboard/`、host 4.0.827、当前应用与产品包。未访问四个已废弃来源目录，未使用`.ref/tools/`。

本轮修正：

- navigation、profile dialog按钮和overflow行使用Base Button，解除component Button重复注册hover的已定位路径。
- 电量与Help重新放入同一个右列，恢复三列basis/shrink约束，修正CSS像素量宽及实际tooltip分支；`hasBattery`、负电量和未知充电状态显示也作了修正。
- 顶部按钮统一解析启用状态和点击目标，连接设备、Profiles、Alexa、Macro和Armory内部历史。内部边界使用外层记录；关闭标签产生的重复项会跳过。Alexa弹层锁定保持有效。取消设备映射确认不再提前改历史索引。跨应用衔接属于本地统一壳适配，不冒充原独立Web窗口行为。
- Profiles使用源码实际两个页签，补搜索/筛选/排序和添加程序空目录弹层。设备列表、游戏详情和关联仍未完成。
- Macro补原始空页面、元数据新建/选择/搜索/排序/重命名/复制/删除、教程、无设备绑定分支及Help。宏/文件夹仅保留在页面内存；事件编辑、录制、XML、部分菜单、模式分支和动画尚未完成。
- Armory补默认能力缺省分支、内部历史和原图介绍横幅，删除技术说明占位；搜索/筛选/详情和服务分支仍未完成。托盘补已核实的100ms颜色/透明度过渡。

最终检查：

| 检查 | 结果 |
| --- | --- |
| `cargo check --locked --all-targets` | 通过，无警告；Macro集成期间发现的类型/API错误均已修正 |
| Rust格式化 | 通过 |
| `validate-resources.py` | 1027项来源/输出hash、格式和嵌入键通过 |
| `audit-device-tabs.cjs` | 当前挂载三组结构、四个Dashboard标签/十语言、五类内部历史接线通过 |
| `audit-profiles-app.cjs --check` | 两个真实页签、十语言、287条CSS收据通过 |
| `audit-battery-indicator.cjs --check` | 27图标，零问题 |
| `extract-macro-locales.cjs --check` | 十种宏专用语言通过，每语言343–350项 |
| `audit-armory-default.cjs` | 七个模块作用域、十语言、102条CSS收据通过 |
| `validate-embedded-json.py` | 22个文档检查通过；两个Value型文档跳过，不能声称已验证其类型结构 |
| `audit-resource-usage.py` | 可静态枚举的字面量及声明的动态图族引用有效；另30个WebP/RGBA资源的消费者未在此脚本追踪，格式/hash由资源校验覆盖 |

没有运行应用、构建、测试、安装器、下载的JavaScript或DLL。以上结果不证明实际点击、遮挡、焦点或最终像素正确。全部产品仍未标记完整；后端与DLL修改继续后置。

详细依据：[导航](device-tabs-audit.md)、[电量](battery-indicator-audit.md)、[Profiles](profiles-app-audit.md)、[Macro](macro-current-source-review.md)、[Armory](armory-app-current-audit.md)、[剩余工作](remaining-ui-work.md)。

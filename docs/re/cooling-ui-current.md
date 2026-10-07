# 散热配件当前界面契约

本文覆盖 3893 Hanbo、3900 PWM 控制器、3907 Laptop Cooling Pad 的 Performance。3858/3880 显示器统一见 [monitor-ui-current.md](monitor-ui-current.md)，3921 Core X V2 的独立 fan 模型见 [corex-fan-current-audit.md](corex-fan-current-audit.md)；共享工作区存在不代表这些产品全部完成。

当前依据为 [accessory-system-source.json](accessory-system-source.json)、[准备收据](accessory-system-native-audit.json)与实际 [AccessorySystemProductWorkspace](../../src/features/accessory_system_products.rs)。提取工具仅解析当前产品 bundle；本次复核了相关产品源文件 hash、Rust 范围/状态/草稿处理，没有执行厂商代码或重用旧源符号。

| 产品 | 当前本地内容 | 尚需真实观察或 UI 完成 |
| --- | --- | --- |
| 3893 | 独立 fan/pump 模式请求，Quiet/Normal/Performance/Advanced | source 初始 ports 为空，真实模式、RPM、曲线、能力限值和 update-required 不能补造；完整曲线主体仍缺 |
| 3900 | 8 个端口、联动、Quiet/Normal/Performance/Manual/Advanced；Manual 33–100%步进 1，Advanced 九点 25–100%并约束单调 | 端口图、重命名、真实 active ports/电源/RPM 及只对实际端口应用的 UI 条件 |
| 3907 | enable、Fixed/Smart、Low/Medium/High 独立值、CPU/GPU 独立 Smart 预设、摄氏/华氏、百分比/RPM 及当前预设 Reset | 源 2–20 节点增删、曲线直接拖动、传感器/系统状态、兼容 Blade 性能导航、硬件能力门控、HyperBoost 与映射按键内容 |

PWM 联动复制所选 activeThermalIndicator；Manual 额外复制速度，Advanced 额外复制曲线。它是本地配置意图，页面不显示固定示例 RPM 作为读数。

Cooling Pad Fixed 范围为 Low500–2000、Medium1500–2500、High1900–3200 RPM，步进 50；Smart 范围 500–3200 并保留邻点单调约束。百分比按 RPM/3200 换算，最低为 15.625%；保存的 cpu_gpu 预设保留，但 UI 只挂实际源 CPU/GPU 选项，不增造第三个传感器页签。Reset 只恢复当前本地预设，不表示硬件已重置。

控件实体/订阅在 render 外保留。restore 按当前 schema 接纳已知字段、固定端口 ID/温度坐标/数组长度、校验选择值并钳制数值，不发用户更改事件。当前图表仍为保留滑条加折线的交互表达，与源图表的完整几何和直接拖动不同；不能以数值约束已实现推定原 UI 完成。

Lighting 的 3893/3907 初值来自各自 reducer 种子，DEFAULTPROFILE 只含身份时不能漏掉这部分；来源见 [accessory-controls-audit.json](accessory-controls-audit.json)。Hanbo 遵守 hideLightingIdle；3929 的灯光在唯一 Customize 内，不能增造 Lighting 页签。这些说明不替代各产品 Lighting 主体的单独核验。

本地草稿与实际设备模式、RPM、电源、温度及服务限制分开；未知读数保持未知。UI 编辑、增删改、Apply/Save 继续在当前范围，DLL 写回后置。只做静态源码/资源校验及允许的格式/编译检查，未运行应用、测试、安装器、下载 JavaScript 或 DLL；视觉、输入和设备往返仍未验证。

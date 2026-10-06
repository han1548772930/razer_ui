# 当前剩余工作

更新：2026-10-06。按 [实施路线](ui-readonly-first-roadmap.md)逐项推进，完整复刻尚未完成。

1. 收齐 Studio Ambient、Static 的交互边界，逐项接入其余 11 个属性根及共享控件，保留设备条件和临时参数语义。
2. 补齐 Studio 分组、拖排、配置、导入导出、关联与独立模式，见 [根审计](chroma-studio-native-root.md)、[属性审计](chroma-studio-properties.md)、[图层审计](chroma-studio-layers-native.md)。
3. 依据 [DLL 只读盘点](dll-readonly-inventory.md)核实实际 DLL 身份、导出和 ABI，补全查询、观察与 UI 消费链。静态封装证据不等于实际读取成功。
4. 逐产品检查控件、弹层、条件、动画与独立根；[331 个产品、1419 个主导航页](native-product-coverage.md)已有部分内容，完整验收产品为 0。
5. UI 编辑、增删、应用、保存和本地草稿现在实现；通过 DLL 修改状态、写回和保存最后统一接入。

Studio 原生取色、区域选择、设备 LED 数据尚需服务证据，自定义颜色仅会话保存。Settings 宿主目录、托盘真实账户与通知、OLED 运行数据仍有缺口，见 [Settings](settings-window-implementation.md)、[托盘](tray-account-continuation-2026-10-06.md)、[OLED](audio-oled-runtime-current-audit.md)。

只允许静态解析、资源验证、格式化与 `cargo check --locked --all-targets`；应用运行、像素、DPI、焦点和真实设备响应未验收。批次详情见 [续接记录](ui-continuation-2026-10-06.md)。

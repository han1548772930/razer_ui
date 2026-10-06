# 当前实现差距

更新：2026-10-06。遵循 [实施路线](ui-readonly-first-roadmap.md)，逐项依据当前源码补齐。

- [331 个产品、1419 个主导航页](native-product-coverage.md)均部分接入，完整验收产品为 0；路由不能证明全部交互完成。
- Studio Ambient、Static 属性根已接入，其余 11 个属性根待接入；分组、拖排、配置与服务能力仍有缺口。
- 产品页控件、独立根、弹层、条件分支和动画分别核对，未建立全部子界面的总数。
- UI 编辑、增删、应用、保存和本地草稿属于当前范围。仅 DLL 修改状态、写回与设备保存后置。
- DLL 读取需核实实际文件、ABI、请求回调和 UI 消费；[盘点](dll-readonly-inventory.md)尚未证明实际安装 DLL 的全部契约。
- 运行、像素和真实设备响应未验收。

批次见 [续接记录](ui-continuation-2026-10-06.md)，任务见 [剩余工作](remaining-ui-work.md)。

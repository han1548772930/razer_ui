# 电量控件当前复核

2026-10-04。实际挂载与CSS级联见[独立复核](source-ui-review-2026-10-04.md)和[导航栏机器收据](device-tabs-audit.json)。[电量表审计](battery-indicator-audit.json)只验证图标/文案表，不再充当位置正确的证明。

已修正：两种产品工作区将电量和帮助放入同一右侧容器；仅has_battery且有power_status时显示；电量用有符号值表达原来的负值显示“-”分支；未知充电状态的默认100图标仍用实际level作提示文案。

内部几何仍按实际级联：46px行、14px文字、26px图标盒、20px图像、左右各5px。百分比在图标之前，0到10为低电红字。

原节点不含tooltip属性，因此 `.batt.batt-warning[tooltip]:before` 的352px不适用。已改用真实tooltip-razer语义：300px对齐容器、按内容收缩的wrapper、Roboto14/16、padding8/10、黑底灰边；右缘对齐、电池底部+5px、水平8px边缘调整、100ms opacity。

仍未完成hideBattValue、externalPowerConnected、左右耳塞电量及对应root状态输入。这些须按产品继续核对，后端/DLL最后统一接入。静态验证不能替代运行时像素验收。

# 控制器：当前源码中的本机功能

本目录按实际挂载的产品根、MW feature configuration 和设备适配器说明功能。产品描述对象里的通用 `QUICK_EFFECTS`、共享校准类或协议 getter，并不自动使该产品具有对应页面。逐 PID 的原件与部署分支见 [products](products/)。

## 产品与适用边界

| PID | 当前 UI 设备名 | 产品根挂载的本机页 | MW 连接、设备与事件分支 |
| --- | --- | --- | --- |
| 2629 | Razer Wolverine V3 Tournament Edition | Customize、Triggers、Thumbsticks、Calibration、Help | 有线；`rzDevice25Sage`；`PROTOCOL25_CONTROLLER`；`supportsLighting:false`；按序列号存储、单配置、从设备加载 |
| 2636 | Razer Wolverine V3 Pro | Customize、Triggers、Thumbsticks、Lighting、Power、Calibration、Help | 无线；Sage，三个传输等待均为 10 ms；`PROTOCOL25_CONTROLLER`；查询电池；`supportsLighting:true` |
| 2647 | RAZER WOLVERINE V3 PRO 8K PC | Customize、Triggers、Thumbsticks、Power、Calibration、Help | SagePC，report ID 10、三个等待均为 30 ms；`PROTOCOL25_CONTROLLER_8K`，60 ms 事件轮询；同步有线与无线 polling rate；Triggers 受实读固件 `1.4.0.0` 门槛控制 |
| 2650 | Razer Wolverine V3 Tournament Edition 8K PC | Customize、Thumbsticks、Calibration、Help | 有线 Sage；`PROTOCOL25_CONTROLLER_8K`，60 ms；没有挂载 Triggers；泛型 feature 注册 UI 硬件事件 |
| 2676 | Razer Wolverine V4 Pro | Customize、Triggers、Thumbsticks、Power、Calibration、Help | 无线 SagePC，report ID 10、30 ms；`PROTOCOL25_CONTROLLER_RECORDID05`，60 ms；同步两种 polling rate；`supportsLighting:false` |
| 2684 | Razer Silver T2 X | Customize、Triggers、Thumbsticks、Calibration、Help | 有线 SagePC，report ID 10、30 ms；`PROTOCOL25_CONTROLLER_RECORDID05`，60 ms；D-pad SOCD 首次从设备读取 |
| 4115 | Razer Kitsune | Customize、Lighting、Help | 有线、PlayStation、SagePC、10 ms；`PROTOCOL25_PS_ARCADE_CONTROLLER`；Chroma 软件灯光；SOCD 不执行设备初始化读取；软件 reset 与 controller mode switch |
| 4133 | Razer Raiju V3 Pro | Customize、Triggers、Thumbsticks、Power、Calibration、Help | PlayStation，无线 SagePC，10 ms；`PROTOCOL25_PLAYSTATION_CONTROLLER`；强制核对 edition、查询电池；D-pad SOCD 从设备读取 |
| 4144 | Razer Raiju V3 Pro Signature Edition | 同 4133 | 独立 PID、dongle 4146、BLE 4147；同一类 PlayStation 协议与页面组合；当前 bootstrap 未注册 4133 的 `useGetDockEditionAPI` |

2684 的根经 `main.fbe5395b.js` 的模块 4841 动态依赖加载，实际页面数组在 `.analysis/inputs/frontend/gamepad-2684-current/raw/apps.razer.com/synapse/products/2684/ui/static/js/8169.3cf63c70.chunk.js`，UTF-8 byte 549199 附近；不能只看 main 中的 lazy wrapper。其余八款的根直接位于各自 UI main；各页数组的 byte 起点依次为 2629：7563783、2636：7507081、2647：7479125、2650：7427750、2676：7652768、4115：7518571、4133：7595295、4144：7541198。

## 功能与状态边界

- [映射、配置档与灵敏度](mapping-and-sensitivity.md)：配置集合、真实 OBM 槽、触发器、死区、灵敏度、圆度、D-pad、polling 与输入事件消费者。
- [校准与灯光](calibration-and-lighting.md)：V1/V2/V3 校准处理器、采样/计算/写入与扳机读回、2636 硬件效果、4115 软件灯光和退出释放。

四个 OBM 槽是 UI descriptor 的能力声明；设备当前返回的 ID 列表、活动 ID、名称、数量是另一组读取结果。`guid` 是本地配置身份，`obmSlotId` 可以是一组设备槽 ID，不能拿列表下标或本地当前选中项代替真实槽。

初始 UI profile 与 MW profile 也不是同一个阶段：例如初始 UI 保留的 `quickEffects` 字段不能证明 `supportsLighting:true`。MW 会在真实设备导入阶段补充 `controllerDeadzone`、`controllerSensitivity`、触发器等字段。只保存字段、收到 UI reducer 动作或写文件完成，都不能证明设备接受了设置。

## 原件与追溯入口

9 款产品分别使用 `.analysis/inputs/frontend/gamepad-<PID>-current/acquisition.json` 与 `.analysis/inputs/frontend/gamepad-mw-<PID>-current/acquisition.json` 的当前部署收据。完整实际路径、HTTP 来源、部署稳定性与字节身份由这些收据及 `.analysis/inputs.json` 维护，不在功能正文复制哈希。

关键审计入口为 `.analysis/results/frontend/gamepad-mw-audit.json` 的 `products[product_id].mounted_main/source_set`，以及 `.analysis/results/frontend/gamepad-main-audit.json` 的 `gamepad_lighting.products[].source_sets`、`source_semantics`、`palette_profile_chain` 与 `native_engine`。这里保留了完整源函数、AST UTF-8 byte 边界和间接依赖。静态源码理解、Rust 独立实现、UI/后端接线、模拟验证、真实运行验收分别判断；本目录描述的是源码语义。

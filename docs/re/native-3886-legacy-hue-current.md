# 产品 3886 当前 Hue/native 单文件链路

审计日期：2026-10-09。依据当前 `.ref/middleware/3886/main.df6f64c941b9efded61e.js` 的 Acorn AST 与实际字节哈希，未执行 vendor JS、应用或 DLL。机器证据见 [native-3886-legacy-hue-current-evidence.json](evidence/native-3886-legacy-hue-current-evidence.json.zip)，生成器见 [audit-native-3886-legacy-hue.cjs](../../tools/audit-native-3886-legacy-hue.cjs)。

## 已闭合的静态链路

当前文件 SHA-256 为 `95bd47acbc00240cca06bc0f95e57bbfc5d2fa3427926a1e44969a489d3c3e5b`。类 `DX` 位于 `[2062852,2073545)`，真实对象名为 `philipsHueMgr`；其 `init` 在 `[2063261,2065536)`。产品服务管理器 `P2` 位于 `[2226856,2234799)`。

启动分支先在 `P2.connect` 中执行 `Is.rzDevice=this.philipsHueMgr=new CX`，再由 `C6` 调用 `T6`。`T6` 从注入特性读取 `hasDll`；只有该特性为真才调用 `mZ`。`mZ` 读取 `Ad.getItem("installedResources")`，合并 `Common` 与 `Synapse`，按 `record.name === requestedName` 且 `record.usedBy.includes(`${ya.productId}`)` 选择记录，拼接 `userDataDir/Apps/${record.filePath}` 并转成反斜杠路径，最后执行 `Is.rzDevice.init(path)`。因此实际 init 路径来自运行时 installedResources 内容，而不是此文件中的 DLL 默认字面量。

`C6` 仅在 `T6` 返回 true 后继续初始化本地状态、调用 `P2.registerHueEvent`；P2 的事件订阅随后把 Hue bridge/light 事件转成 UI 状态和设备列表。P2 提供扫描、按 IP 查找、配对、取消、解绑、亮度、灯列表、组切换和关闭全部灯等调用，均通过 `philipsHueMgr` 转发到 DX。

## DLL ABI 与状态边界

DX `ConfigureFFI` 的 API 对象包含 23 个静态声明：`GetDLLVersion`、`FreeMalloc`、`SetNodeFFIEvent`、`Create`、`Destroy`、`DiscoverBridges`、`PairBridge`、`UnPairBridge`、`Abort`、`FindBridge`、`GetBridgeConfigFileLocation`、`GetConnectedBridgeInfo`、`GetEntertainmentGroupList`、`GetSelectedEntertainmentGroup`、`SelectEntertainmentGroup`、`GetLightList`、`SetCustomChromaFrame`、`SetBrightness`、`AcquireControl`、`ReleaseControl`、`GetControllerFilePath`、`GetCurrentCacheLights`、`TurnOffAllLights`。返回类型和参数类型均保留在 JSON 的 `abi_entries`，不能根据函数名猜测结构体 ABI。

`DX.init` 的 fallback 是 `userDataDir\\Apps\\Synapse\\PhilipsHueNative.dll`，仅当传入参数为空时使用。源码链没有证明该 fallback 分支被选中，也没有证明某一实际 DLL 文件已经加载。`ConfigureFFI` 成功后才依次请求 `GetDLLVersion`、`SetNodeFFIEvent`、`Create`；返回值、设备状态和硬件成功均未运行验证。P2.destroy 转发 DX.destroy，但销毁时机由外部生命周期决定。

事件名称和消费者已逐项保存：bridge 完成/断开/连接、灯上线/离线、更新信息、组选择完成、忙状态，共 8 类。静态代码会把这些事件映射到 UI 状态、lightDevices 增删改和当前组刷新；这证明消息处理逻辑存在，不代表运行时收到过事件。

## 尚未可证明的部分

- 当前单文件没有静态 `hasDll` 资源列表字面量，注入特性的来源在运行时配置边界之外；因此链路是“若 `hasDll` 注入，则 mZ 资源选择并 init”，不能写成产品 3886 必然加载某个 DLL。
- `installedResources` 的实际内容、选中的 `filePath`、DLL 文件字节与 FFI 返回值未知；manifest、PE 导出名和 fallback 名称都不能替代这些运行时事实。
- 该产品的 UI 页面可能发送 Hue 操作，但本证据只闭合 middleware 管理器与 DLL ABI，不宣称 UI 操作已经执行或写回硬件。

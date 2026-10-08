# 原代码原生件总表（DLL / 原生插件 / 辅助程序）

本文只按**当前逆向源码**登记原件，不按产品猜测，也不表示已执行。事实来源分三类：

- **宿主**：`.ref/host-4.0.827/electron/**`（当前 4.0.827 静态提取），可读中间层在 `.ref/host-4.0.827/source-evidence/background-current-source.js`。
- **设备中间件**：`.ref/middleware/<productId>/**`（各产品 web 应用 / `project_uma_mw` 包）与其 `manifest.json`。
- **包清单**：`.ref/host-4.0.827/source-evidence/inner-archive-list.txt`、`native-evidence/EXTRACTION-RECEIPT.json`。

判定口径：`ffi-napi-rz` 的 `Library(dllPath, apiObj)` = 直接加载 DLL；`require("*.node")` = Node 原生插件；`spawn` 的 `.exe` = 独立辅助程序。函数名清单来自 JS 绑定表字面量（`名字:["类型",[参数]]`），**绑定表存在只证明 JS 侧声明，不证明导出真实存在、更不证明已被执行**。Electron/Chromium 自带的 DLL（`ffmpeg.dll`、`libEGL.dll`、`libGLESv2.dll`、`vk_swiftshader.dll`、`vulkan-1.dll`、`dxcompiler.dll`、`dxil.dll`、`d3dcompiler_47.dll`）不属于雷云功能，本文只列名不计入。

## 1. 主表

| 原生件 | 类别 | 路径 / 加载方式 | 作用 | 当前源码证据 |
| --- | --- | --- | --- | --- |
| `mapping_engine.dll` | 引擎服务 | `CommonDLL\`，`ffi-napi-rz.Library` + `ConfigureFFI` | 按键重映射、Hypershift、宏录制、全局快捷键、输入重定向、输入监控、设备模式 | `electron/modules/mapping_engine/win/index.js:166794`、`dll_registry` 登记 `:13204` |
| `simple_service.dll`（另有 `simple_service_v1.2.2.9.dll`） | 引擎服务 | 同上 | 服务版本、用户应用管理、音频设备枚举、音量/静音、侧音、以管理员启动 | `electron/modules/simple_service/win/index.js:3553`、`:4405` |
| `SysUtilsNative.dll` | 引擎服务 | 同上 | 机器 ID、序列号、驱动/模块信息、前台窗口与键盘布局监控、应用图标、系统设置对话框、动态灯光/暗色/全屏判断、RzEngineMon | `electron/modules/sysutil/win/index.js:3662` |
| `lighting_driver.dll` | 灯光写出 | `ffi-napi-rz.Library`（宿主 `FFILightingDriver.initDll`） | 注册设备、下发灯光帧、写回调、暂停/恢复 | `electron/modules/lighting/ffiLightingDriver.js`（7 个绑定） |
| `RzLightingEngineApi` | 灯光引擎 | 动作 ID 分发（非普通导出库） | 效果编排：建引擎/建设备/加效果/取帧 | `.ref/applications/synapse/dashboard/static/js/2973.acc7b128.chunk.js`，清单见 `src/backend/protocol.rs` |
| `IoTNative.dll` | IoT | `ConfigureFFI`（宿主 `IoTSDKNative_Action`） | IoT 设备 Chroma 帧、白色 LED 亮度；设备侧另含 mDNS 扫描/WiFi 连接 | `electron/modules/IoT/IoTNativeAction.js:370`；设备侧 `.ref/middleware/179/7254.be9dba3fe6dc42d7f519.js:10186` |
| `bladeNative.dll` / `blade2Native_v*.dll` / `BladeNative_v*.dll` | 笔记本 Blade | `ConfigureFFI`（通道 `bladeNativeAction_<hash>`） | 屏幕刷新率、颜色管理 ICC、GPU/HDR、BIOS/EC 版本、序列号、通知 | `source-evidence/background-current-source.js:327757`、`:328732`；清单见 `.ref/middleware/563/manifest.json:5958` 等 |
| `RzNative_0517.dll` | 音频 | `ConfigureFFI` | 扬声器音量/静音（9 绑定） | `.ref/middleware/162/main.12e8af9bc3e4a37e6b0a.js:1489914` |
| `RzNative_0542.dll` / `RzNative_0543.dll` | 音频（Camy 摄像头混音） | `ConfigureFFI` | 音量/静音、混音使能/电平、麦克风 HPF/限幅/增益、播放设备枚举、背景录制 | `.ref/middleware/162/…:1718117`、`.ref/middleware/3886/main.df6f64c941b9efded61e.js:1444927` |
| `RzNative_0518` / `_0E03` / `_0E05` / `_0E06` / `_0E08` / `RzNative_GenericCamera.dll` | 摄像头 | `ConfigureFFI` | 摄像头参数（亮度/对比度/饱和度/白平衡/对焦/HDR/LDC…）、属性对话框、启停、序列号/固件 | `.ref/middleware/162/…:1485533`、`:1458029`、`:1468948`、`:1769614`；`.ref/middleware/179/rzCamKimberly.92d455a5af199b2ec1c0.js:30162`（99 绑定） |
| `RzAudioUtil.dll` | 音频工具 | `ConfigureFFI` | 单一 `Dispatch` 分发入口 | `.ref/middleware/1306/3487.591e6c57187c1625f29c.js:569206` |
| `RzAV.dll` | 音视频 | `ConfigureFFI` | AV 采集句柄获取/释放、启停、采样率与频谱、音频数据 | 同上 `:1663704` |
| `audCapNative.dll`（`_v1.0.2.0`） | 音频采集 | `ConfigureFFI` | 音频采集（仅名与版本，绑定表待补） | 名称 `.ref/middleware/1306/3487.591e6c57187c1625f29c.js:1580433`；版本 `.ref/middleware/1383/manifest.json:3669` |
| `CmMixerLib_v1.0.1.0.dll` | 音频混音 | `ConfigureFFI` | 混音库（仅版本名，绑定表待补） | `.ref/middleware/1342/manifest.json:6296` |
| `ScarlettNative.dll` | 显示器 | `ConfigureFFI` | 58 个绑定：亮度/对比度/色彩预设、PiP、缩放、FreeSync/Overdrive、HDR、输入源、THX 模式、色域、刷新率、色彩配置 | `.ref/middleware/162/…:1805252` |
| `ThxV4Native.dll` / `ThxV3Native.dll` / `thxv2api.dll` / `ThxVADCarolNative_v*.dll` / `thxvadvirtualroutenative_*` / `*_routing_client` | THX 音频 | `ConfigureFFI` | 设备增删、状态、属性读写、预设选择/保存；虚拟声道路由 | `ThxV4` 绑定表 `.ref/middleware/1306/3487.…:484087`；版本名 `.ref/middleware/709/manifest.json:11991`、`1392/manifest.json:5478`、`1442/manifest.json:3073` |
| `PhilipsHueNative.dll` / `philipshuev2.dll` | 第三方灯光 | `ConfigureFFI` | 桥发现/配对、娱乐组与灯列表、自定义 Chroma 帧、亮度、占用控制、关灯 | `.ref/middleware/3886/…:2063570`、`.ref/middleware/179/5559.7ce9faacbc35b39648d2.js:2325` |
| `NanoleafNative.dll` | 第三方灯光 | `ConfigureFFI` | Nanoleaf 灯光接入（绑定表待补） | `.ref/middleware/162/…:1790632`、`.ref/middleware/3886/…:1998262` |
| `RzSystemMon.dll` | 系统监控 | `ConfigureFFI` | 硬件数据读取、事件注册、APP server 状态 | `.ref/middleware/162/…:1910321` |
| `RzAMDOverClock` / `RzIntelOverClock` / `CTR.dll` / `cpuidsdk64_v1.3.1.2.dll` | 超频/CPU | `ConfigureFFI` | 曲线优化、功耗墙、电压偏移等异步读写；CPU 识别 | `RzAMDOverClock` 绑定 `.ref/middleware/2595/2592.f6ecdbca2cff19e73d8c.js:986063`；`CTR.dll` 同文件 `:1343152`；`cpuidsdk64` `.ref/middleware/694/manifest.json:6826` |
| `RzASRock.dll` | 主板灯光 | `ConfigureFFI` | 亮度、静态/默认/自定义效果、Chroma 灯带 LED 数 | `.ref/middleware/179/rzASRock.9ea6f530e3e142c44259.js:9627`（20 绑定） |
| `RzUMASDKWrapper*` / `Razer_Upgrade_SDK.dll` | 固件升级 | `ConfigureFFI` | 固件升级 SDK（仅名，绑定表待补） | `.ref/applications/synapse/update-fw/static/js/main.69cc5fbd.js:1875963` |
| `HID.node` | Node 插件 | `app.asar.unpacked\node_modules\node-rz-hid\build\Release\` | HID 枚举与 Feature 收发（`hid_open_path`/`hid_send_feature_report`/`hid_get_feature_report`/`hid_close`/`hid_enumerate`/`hid_free_enumeration` 等，AMD64 23 个 PE 导出） | 包清单 `inner-archive-list.txt`；ABI 证据 `docs/re/receiver-native-hid-current-evidence.json` |
| `detection.node`（`rz-usb-detect`） | Node 插件 | 同目录 `rz-usb-detect\build\Release\` | 物理 USB 枚举（`USB_DEVICE` GUID `a5dcbf10-…`） | 包清单；`docs/re/usb-native-current-evidence.json` |
| `detection.node`（`node-rz-ble-endpoint-detection`） | Node 插件 | 同目录 | BLE 端点探测 | 包清单 |
| `ffi_bindings.node`（`ffi-napi-rz`） | Node 插件 | 同目录 | 上面所有 `Library`/`Callback` 的底层 FFI | 包清单；`electron/modules/ffi/ffiMain.js:656` |
| `noble.node` / `binding.node`（`node-ble-rz`） | Node 插件 | 同目录 | BLE 中央设备（配对/连接） | 包清单 |
| `bluetooth_hci_socket.node` | Node 插件 | `@abandonware\bluetooth-hci-socket` | Windows BLE 的 HCI 套接字后端 | 包清单 |
| `bindings.node`（`@serialport`） | Node 插件 | `@serialport\bindings-cpp` | 串口（`rzSerial`：串口设备/连接/读写） | 包清单；`.ref/middleware/179/rzUSB.b989822eaf686393015c.js` 中的 `serial.*` 动作 |
| `mjpeg-hotpatch.node` | Node 插件 | `mjpeg-hotpatch` | MJPEG 流补丁（摄像头预览） | 包清单 |
| `rzNotification.node` | Node 插件 | `node-rz-notification` | 原生通知 | 包清单 |
| `node.napi.node`（`usb`） | Node 插件 | `usb\prebuilds\win32-x64` | 通用 USB 访问（非 Razer 专有） | 包清单 |
| `ref-napi` `binding.node` | Node 插件 | `ref-napi` | FFI 指针辅助 | 包清单 |
| `RzPowerTool.exe` | 辅助程序 | `CommonDLL\` | 服务状态/启动/停止：`--get-service-status`、`--start-service`、`--stop-service` | `electron/serviceFunction.js:314` |
| `RzSecurityTool.exe` | 辅助程序 | `CommonDLL\` | 设备安全校验：`--verify-device-security` | `electron/modules/security/win/index.js` |
| `RzHandle.exe` / `RzEngineMon.exe` | 辅助程序 | `CommonDLL\` | 句柄/进程工具；引擎监视（`SysUtilsNative.launchRzEngineMon`） | 包清单；`sysutil/win/index.js:2806` 起的绑定 |
| `RazerAppEngine.exe` / `RzEngineMon` 监视链 | 宿主进程 | `app-<ver>\` | 引擎宿主与升级链 | `electron/main.js`（`runRzEngineMonitor`）、`ARCHIVE-CHAIN.json` |
| `hid_hidraw.node` | Node 插件 | 见宿主 token 扫描 | Linux HID raw（非 Windows 主路径） | `.ref/host-4.0.827` 文本命中 |
| `RazerAppEngineUpgrade(-Setup-Internal)-v*.exe` | 安装器 | 下载包 | 引擎升级安装包（解包处理，不执行） | `ARCHIVE-CHAIN.json:193`、`:596` |

## 2. 引擎服务 DLL 的完整绑定表（宿主侧）

### 2.1 `mapping_engine.dll` — 131 个声明

来源：`electron/modules/mapping_engine/win/index.js`（`apiObj` 由多个版本表合并：`:166794` 处 `{...i}`，失败时回退旧版本 DLL）。

`stopRunningTurbosAndMacros, mappingEngineInitialize, mappingEngineShutdown, addUsbDevice, removeUsbDevice, isInputNotificationRegistered, registerInputNotification, unregisterInputNotification, setInputNotificationCallback, registerHardwareEvent, unregisterHardwareEvent, isHardwareEventRegistered, setHardwareEventCallback, enableMapping, disableMapping, isUnsupportedMappingRegistered, setUnsupportedMappingCallback, registerUnsupportedMapping, unregisterUnsupportedMapping, isInputRedirectEnabled, enableInputRedirect, disableInputRedirect, setInputRedirectCallback, isMouseMoveRedirectEnabled, enableMouseMoveRedirect, disableMouseMoveRedirect, setMouseMoveRedirectCallback, queryRunningMacrosAndKeysPressed, localStorageSetItem, localStorageDeleteItem, localStorageDeleteAllItems, registerAudioServiceEvent, unregisterAudioServiceEvent, setSpeakerVolumeChangedCallback, setMicrophoneVolumeChangedCallback, setDefaultMicrophoneDeviceChangedCallback, setDefaultSpeakerDeviceChangedCallback, getDefaultSpeakerDevice, getDefaultMicrophoneDevice, microphoneVolumeStepDown, microphoneVolumeStepUp, setMicrophoneMute, setMicrophoneVolume, setSpeakerMute, setSpeakerVolume, speakerVolumeStepDown, speakerVolumeStepUp, hasMacroRecordingStarted, startMacroRecording, stopMacroRecording, registerMacroRecorderEvent, unregisterMacroRecorderEvent, setMacroRecorderEventCallback, getGlobalShortcuts, isGlobalShortcutRegistered, setGlobalShortcutEventCallback, registerGlobalShortcut, unregisterGlobalShortcut, unregisterGlobalShortcuts, enableGlobalShortcut, disableGlobalShortcut, isGlobalShortcutEnableEventRegistered, setGlobalShortcutEnableEventCallback, registerGlobalShortcutEnableEvent, unregisterGlobalShortcutEnableEvent, getMacroRecordingMode, isDeviceModeChangedEventRegistered, registerDeviceModeChangedEvent, unregisterDeviceModeChangedEvent, setDeviceModeChangedEventCallback, isHypershiftActive, isOtfsActive, getDeviceMode, getGlobalMode, addUsbDeviceWithoutFilterDriver, enableKeyboardInputRedirect, disableKeyboardInputRedirect, enableMouseInputRedirect, disableMouseInputRedirect, enableRazerKeyInputRedirect, disableRazerKeyInputRedirect, isKeyboardInputRedirectEnabled, isMouseInputRedirectEnabled, isRazerKeyInputRedirectEnabled, isAnalogKeyEventRegistered, registerAnalogKeyEvent, unregisterAnalogKeyEvent, unregisterAllAnalogKeysEvent, setAnalogKeyEventCallback, addDevice, removeDevice, notifyInputForExternalDevice, setGlobalShortcutUnsupportedMappingCallback, registerAllAnalogKeysEvent, isJoystickEventRegistered, registerJoystickEvent, unregisterJoystickEvent, setJoystickEventCallback, isXboxGamepadEventRegistered, registerXboxGamepadEvent, unregisterXboxGamepadEvent, setXboxGamepadEventCallback, isAllDevicesHooksEnabled, enableAllDevicesHooks, disableAllDevicesHooks, setInputMonitoringCallback, startKeyboardInputMonitoring, stopKeyboardInputMonitoring, hasKeyboardInputMonitoringStarted, startMouseInputMonitoring, stopMouseInputMonitoring, hasMouseInputMonitoringStarted, getMouseSensorRotationAngle, setMouseSensorRotationAngle, isSnaptapKeyEventRegistered, registerSnaptapKeyEvent, unregisterSnaptapKeyEvent, setSnaptapKeyEventCallback, registerIntervaledHardwareEvent, isAllAnalogKeysEventRegistered, registerIntervaledJoystickEvent, registerIntervaledXboxGamepadEvent, registerIntervaledAnalogKeysEvent, isBigDataEventRegistered, registerBigDataEvent, unregisterBigDataEvent, setBigDataEventCallback, hasKeepAliveStarted, startKeepAlive, stopKeepAlive, setKeepAliveCallback`

划分：`mappingEngineInitialize/Shutdown` 生命周期；`*InputRedirect*`/`*InputMonitoring*`/`*Joystick*`/`*XboxGamepad*`/`*Snaptap*` 输入通道；`*Macro*` 录制；`*GlobalShortcut*` 全局快捷键；`localStorage*` 引擎侧存储；`getGlobalMode`/`getDeviceMode`、`enableMapping`/`disableMapping` 状态。

### 2.2 `simple_service.dll` — 39 个声明

来源：`electron/modules/simple_service/win/index.js:3553`（版本回退时合并不同表：`:3738`）。

`simpleServiceInitialize, simpleServiceShutdown, isAppsServiceEventRegistered, registerAppsServiceEvent, unregisterAppsServiceEvent, setAppsServiceEventCallback, simpleGetVersionInfo, simpleGetUserApps, simpleAddUserAppFile, simpleRemoveUserAppFile, simpleRemoveUserApp, simpleRemoveUserAppDirectory, simpleLaunchUserAppProcess, simpleLaunchUserAppProcessNoWait, simpleLaunchRazerApp, simpleLaunchRazerAppNoWait, simpleEnumerateAudioDevices, simpleGetMicrophoneVolume, simpleSetMicrophoneVolume, simpleGetSpeakerVolume, simpleSetSpeakerVolume, setAudioServiceEventCallback, simpleRegisterMicrophoneEvent, simpleRegisterSpeakerEvent, simpleUnregisterMicrophoneEvent, simpleUnregisterSpeakerEvent, simpleGetDefaultSpeaker, simpleGetDefaultMicrophone, simpleSetDefaultAudioDevice, simpleSetSidetoneConfig, simpleGetSidetoneVolume, simpleSetSidetoneVolume, simpleRegisterAudioDeviceStateChangedEvent, simpleUnregisterAudioDeviceStateChangedEvent, simpleLaunchUserAppElevated, simpleLaunchUserAppElevatedNoWait, simpleLaunchRazerAppElevated, simpleLaunchRazerAppElevatedNoWait, simpleLaunchUserAppInAsciiFolder`

### 2.3 `SysUtilsNative.dll` — 76 个声明

来源：`electron/modules/sysutil/win/index.js:3662`（`{...i}`）。

`GetDLLVersion, FreeMalloc, SetNodeFFIEvent, Initialize, Terminate, applicationAutoStart, setApplicationAutoStart, getApplicationAutoStartApps, launchMicrosoftApp, launchTaskManager, launchFileExplorer, msSettings, getApplicationIcons, getFileVersionInfo, windowsSystemDirectory, systemSKU, computerName, keyboardLayout, hibernateWorkstation, lockWorkstation, restartWorkstation, shutdownWorkstation, sleepWorkstation, OpenAudioProperties, OpenKeyboardProperties, OpenMouseProperties, OpenSoundVolume, getInstalledApplicationList, displayPowerState, getMonitorInfo, getVirtualScreenRect, getCurrentMonitor, SetProcessAppId, SetWindowAppId, ShowColorPicker, CloseColorPicker, GetGMS3Info, getNetworkStatus, StartMonitorForegroundWindow, StopMonitorForegroundWindow, IsScreenLocked, GetDriverInfo, GetSystemLanguage, systemFamily, MigrateRzAppEngineRegistry, GetSystemSerialNumber, StartMonitorKeyboardLayout, StopMonitorKeyboardLayout, getTouchPadEnableStatus, toggleTouchPadEnableStatus, registerDiscord, getWheelScrollLines, setWheelScrollLines, GetURLFromInternetShortcut, OpenGameController, GetRazerMonitorList, GetModuleInfo, SetProcessPowerThrottling, GetInstalledRazerAppEngineProduct, GetMachineId, GetPathFromShortcutLnk, CreateExitEvent, SetExitEvent, isDarkMode, isWindowsDynamicLightingEnabled, isFullScreenMode, getASRockSystemEdition, getApplicationIconsAlt, BringProcessWindowToFront, IsAppLaunched, RegisterAppLaunched, UnRegisterAppLaunched, GetRegisteredApplaunchedList, launchRzEngineMon, isMediaFoundationInstalled, SimulateMemoryLeak`

### 2.4 `lighting_driver.dll` — 7 个声明

来源：`electron/modules/lighting/ffiLightingDriver.js`（`initDll`）。

`Startup:["void",[]], Shutdown:["void",[]], Configure:["char*",["string"]], FreeString:["void",["pointer"]], HookLightingCallback:["bool",["string","string","pointer"]], SetWriteFFICallback:["void",["pointer"]], GetDllVersion:["char*",[]]`

动作：`InitDLL / AddDevice / RemoveDevice / Configure / Pause / Resume / HookLightingCallback`（`device.register`、`device.unregister`、`mode.set` 由 `Configure(JSON)` 承载）。

### 2.5 `IoTNative.dll`

宿主侧 `IoTSDKNative_Action`：`IOT_RunCmdFw25(host, cmd, json, len, flag)`，动作 `IoT.SetChromaFrameV2 / IoT.SetChromaFrame / IoT.UpdateWhiteLEDBrightness / IoT.IsLightingDriverSupport`（`electron/modules/IoT/IoTNativeAction.js:370`）。
设备侧绑定（`.ref/middleware/179/7254.be9dba3fe6dc42d7f519.js:10186`）：`Init, UnInit, GetDLLVersion, FreeMalloc, SetNodeFFIEvent, IOT_StartMdnsDeviceScan, IOT_StopMdnsDeviceScan, IOT_IsNetDeviceConnected, IOT_ConnectNetDevice, IOT_DisconnectNetDevice, IOT_RunCmdFw25, IOT_RunCmdEsp32, IOT_GetSdkVersion, IOT_SetSdkLog, IOT_GetControllerFilePath, IOT_SetDeviceInfoToFile, WIFI_GetCurrentConnectionsAp, WIFI_GetConnectedProfileList, WIFI_ConnectWifiFromProfile`。

### 2.6 通用原生库加载协议（`bladeNative` / IoT / 设备类共用）

`source-evidence/background-current-source.js:328732` 一带：`ConfigureFFI` → `DeviceInit` → `GetLibVersion` → `getSDKVersion` → `ConfigureFFI_APIToCallWhenExit(uninitSDK)`；失败则“try loading older version of lib”。`electron/modules/ffi/ffiMain.js:656` 的 `callDLLMain` 是所有 `Library(dllPath, apiObj)` 的实际入口，另有 `callDLLMainAsync`、`SetNodeFFIEvent` 回调注册与 `FreeMalloc` 释放约定。

## 3. 设备中间件原生库分组

| 产品范围（示例） | 原生件 | 作用 |
| --- | --- | --- |
| 音频/摄像头（179、226、162、3886、1306–1318、2595/2596） | `RzNative_0542/0543`、`RzNative_0517`、`RzAudioUtil`、`RzAV`、`audCapNative`、`CmMixerLib`、`thxv2api`、`ThxV3/V4Native`、`ThxVAD*`、路由客户端 | 混音、音量、麦克风处理、AV 采集与频谱、THX 属性与虚拟声道 |
| 摄像头（179 `rzCamKimberly`、162） | `RzNative_GenericCamera`、`RzNative_0518/0E03/0E05/0E06/0E08` | 图像参数、属性对话框、HDR/LDC、自动取景、镜头联动、Camo Studio 启动 |
| 显示器（162） | `ScarlettNative` | 显示器全部设置与色彩管理 |
| 第三方灯光 | `PhilipsHueNative`/`philipshuev2`、`NanoleafNative` | Hue/Nanoleaf 桥接与帧下发 |
| 主板/超频 | `RzASRock`、`RzAMDOverClock`、`RzIntelOverClock`、`CTR.dll`、`cpuidsdk64` | 主板灯光；CPU 曲线/功耗/电压；CPU 识别 |
| 系统/固件 | `RzSystemMon`、`RzUMASDKWrapper`、`Razer_Upgrade_SDK` | 硬件监控；固件升级 |
| 笔记本 | `bladeNative`、`blade2Native_v*`、`BladeNative_v*` | 屏幕、ICC、GPU/HDR、BIOS/EC |

DLL 文件名与版本同时写在对应产品的 `.ref/middleware/<id>/manifest.json`（例如 `BladeNative_v1.0.10.1.dll` `.ref/middleware/563/manifest.json:5958`、`ThxV4Native_v1.1.1.0.dll` `709/manifest.json:11991`、`audCapNative_v1.0.2.0.dll` `1383/manifest.json:3669`、`cpuidsdk64_v1.3.1.2.dll` `694/manifest.json:6826`）。

## 4. 与读取链路的关系（鼠标为什么“读不到”）

原代码里读设备只有一条链，全部经宿主转发：

1. `usb.getDevices` → `rz-usb-detect/detection.node`（物理 USB）；`hid.getDevices` → `node-rz-hid/HID.node`（HID 接口，条目含 `path / vendorId / productId / interface / deviceContainerId`）。见 `electron/UsbRzDeviceAction.js:4749`（hid.getDevices）与 `:4006`（usb.getDevices）。
2. 中间层 `rzHidDevices.connectHidDevice({productId, vendorId, deviceContainerId, claimInterface, queryData})` 在 `hidDevices` 中按 **productId+vendorId+deviceContainerId** 过滤，再要求 `l.interface === claimInterface`，然后 `hid.openDevice{path,…}`（`source-evidence/background-current-source.js:20553` 一带）。
3. 设备类（`rzDevice25` 基类，`reportLength=91`）用 `hid.sendFeatureReport` / `hid.getFeatureReport` 收发，`_createDataSend` 布局为 90 字节 + reportId，校验和 XOR `[2..87)`；179 的 `DeviceInfo` 声明 `claimInterface:0`（`.ref/middleware/179/main.6d1e356030ef563555d4.js` @137547 起），`dongleId:179`。
4. 双联鼠标（182 等）由 `rzDevice25DualLinkMouse` 承担，构造时 `productId` 取 **dongleId**、`claimInterface` 取 `e.master.claimInterface`，即依旧走**接收器那个 HID 接口**（证据 `docs/re/mouse-read-capabilities-current-evidence.json` 中 182 工厂 `module 87887`）。

对照本仓库实现的三处结构性差异（都以原代码为准）：

- 本项目用 `windows-sys`/SetupAPI 自建 USB+HID 枚举（`src/backend/runtime_usb.rs`、`runtime_hid.rs`），原代码是 `detection.node` + `HID.node`。`HID.node` 只用于实际读写（`runtime_hid_transport.rs`），枚举口径因此可能与原实现不同。
- 本项目在匹配接口时额外要求 `feature_report_bytes == report_bytes`（`runtime_device_reads.rs::current_target`、`receiver_requests`），原代码只匹配 `interface`，不比对 Feature 长度。
- 身份目录里 179 未标 `dongleId`（`src/backend/discovery_catalog.json`），而原 179 `DeviceInfo` 明确 `dongleId:179`；因此本项目把 179 当有线配件（`ObservedTransport::Wired`）。
- 读取能力表现在只有产品 182（`assets/data/device-read-capabilities.json`，来源 `docs/re/mouse-read-capabilities-current-evidence.json`）；配对的若是其它鼠标型号，本项目没有任何查询可用，即使设备已被发现也不会读到 DPI/电量/固件。

## 5. 生成资产与已接入的读取路径

| 资产 | 生成工具 | Rust 消费点 |
| --- | --- | --- |
| `assets/data/native-library-inventory.json` | `tools/generate-native-library-inventory.cjs`（宿主 wrapper + 各产品 `apiObj` 绑定表 + 单库文件的整表回退 + `userDataDir\Apps\...` 默认路径；48 个库、822 个声明函数、360 份源码收据） | [`native_library.rs`](../../src/backend/native_library.rs)：`all()` / `find()` / `with_function()` / `candidate_paths()`，以及 [`mod.rs`](../../src/backend/mod.rs) 的 `native_library_candidates()` |
| `src/backend/discovery_catalog.json` | `tools/prepare-discovery-catalog.py`（现同时合并 `docs/re/middleware-device-bindings-current.json` 的逐产品 `DeviceInfo`） | [`device_identity.rs`](../../src/backend/device_identity.rs)：`is_dongle`/`is_ble`、`lookup_receiver_peer()`、`claim_interface()`、`middleware_category()` |
| `assets/data/device-read-capabilities.json` | `tools/audit-mouse-read-capabilities.cjs` | [`device_reads.rs`](../../src/backend/device_reads.rs) 的 `capability()` |
| `assets/data/receiver-query-capabilities.json` | `tools/audit-receiver-catalog.cjs` | [`receiver_capabilities.rs`](../../src/backend/receiver_capabilities.rs) |

对齐原代码的两处修正（都不是给单个产品打补丁）：

1. **身份事实来自原代码逐产品数据**：`prepare-discovery-catalog.py` 合并 middleware `DeviceInfo` 后，330 行里 328 行带生成事实、99 行带 `dongle_id`、301 行带 `claim_interface`；其中 12 行（104/164/179/207/241/1420/1465/2676/3909/3940/3949/4126）的 `dongleId` 只由原代码的 middleware 声明，由生成器统一补齐并附 `receipt`。
2. **HID 接口选择与源码一致**：`discovery::select_interface_paths()` 只按 `vendorId`+`productId`+`deviceContainerId`+`interface` 选择（原 `rzHidDevices.connectHidDevice` 的规则），Feature 长度只作为**偏好排序**与**实际观察值**记录；worker 侧 `runtime_receiver.rs` / `runtime_device_reads.rs` 的身份校验也改为"长度存在且真实"而非"必须等于能力常量"。

已接入 = 上述生成资产 + 只读路径解析 + 统一只读版本查询 + 现有只读查询（枚举、接收器无线状态、设备字段、simple/version/audio）。

- `ServiceRequest::NativeLibraryVersion { library }` → worker 由 [`native_query.rs`](../../src/backend/native_query.rs) 处理：只接受绑定表声明的**无参、返回字符串**的版本导出（`GetDLLVersion`/`GetDllVersion`/`GetLibVersion`/`getSDKVersion`），加载路径来自生成清单，返回串用该库自己的 `FreeMalloc` 释放；已知加载会阻塞的库（`SysUtilsNative`）直接拒绝。
- 命令行只读入口：`--dll-version <库ID>` 与 `--dll-get <库ID> <导出名> <设备ID> [productId]`，与 `--probe`/`--lighting` 一样在独立进程内执行，UI 之外。
- 通用只读 getter（[`native_read.rs`](../../src/backend/native_read.rs)，请求 `NativeLibraryGetter`）：只接受清单里名字以 `Get`/`Is`/`Has` 开头、签名恰为单个 `string` 参数、返回 `pointer`/`bool`/`long`/`int` 的导出；其余（含全部 `Set*`、`Device*`、`Init/Terminate`、`Register*`）在加载任何库之前就被拒绝。当前清单中有 **12 个库**具备该形态，共 156 个可调用 getter：`RzNative_GenericCamera` 36、`RzNative_0E08` 31、`ScarlettNative` 27、`RzNative_0542` 11、`RzNative_0543` 8、`RzNative_0518/0E03/0E05/0E06` 各 7、`SysUtilsNative` 5（已知加载阻塞，实际被拒）、`PhilipsHueNative`/`philipshuev2` 各 1。
- 归属规则：优先取该库 `ConfigureFFI` 载荷最近的 `apiObj` 表；文件只提到一个原生库时，回退到该文件自身的 FFI 形态条目（例如 `rzCamKimberly` 的 99 项）。跨类的整文件合并不做，避免把别的设备的表记到本库名下。
- 调用点证据：生成器同时记录每个 getter 在原码里的调用形态，当前共 **216 条**，全部是 `Name(dispatched-by-action-name)` 形态——设备类并不直接以字面参数调用这些导出，而是把动作名交给共享 FFI 桥（`callDLLApi`/`_actionWithParams`），由桥按设备上下文（`instanceId`/`deviceContainerId`）组装那个 string 参数。因此 `native_read` 要求调用方显式给出 `device_id`，值取自已观察到的设备（`DeviceReadTarget.device_container_id`），不猜、不从动作名推断。
- 已有 UI 消费者：[`diagnostics.rs`](../../src/shell/runtime_page/diagnostics.rs) 的 `native_libraries()` 把生成清单写进 `%APPDATA%\razer_ui\discovery-latest.json`（库数、声明函数数、各库候选路径规则与**哪些规则下文件真实存在**、是否已知加载阻塞）。只按文件存在性判定，明确标注未加载任何库，因此"文件在"不会被读成"功能可用"。
- 同文件的每条设备观察另带 `declared_libraries`：由 `native_library::libraries_for_product()` 用生成清单的 `products` 字段，把观察到的产品关联到它自己 middleware 声明的库（供后续以 `device_container_id` 作 `device_id` 接线用）。**列出库名不等于读到了值**，诊断里二者分列。

**未接入** = 各库的写入/生命周期动作（`enableMapping`、`SetProperty`、`SetBrightness`、`DeviceInit` 之外的 `set*`、固件升级等）仍只作为声明保留，不提供 UI 入口、不返回成功。

## 6. 未证明 / 待补

- 仅按名称登记、尚无绑定表：`audCapNative`、`CmMixerLib`、`NanoleafNative`、`RzUMASDKWrapper`、`Razer_Upgrade_SDK`、`cpuidsdk64`、`RzIntelOverClock`、`ThxV3Native`、`thxv2api`、`ThxVAD*`、`*_routing_client`、`blade2Native`。
- `RzLightingEngineApi` 的完整动作表与调用契约；`lighting_driver` 的 `Configure` JSON schema。
- 所有 `.dll`/`.node` 的真实导出集合、位数与加载结果均未验证；本文只证明 JS 侧声明与包内路径。
- 本文不改变 `docs/re/dll-readonly-inventory.md` 的请求/返回契约，也不表示任何写回、设备持久化已被接入。

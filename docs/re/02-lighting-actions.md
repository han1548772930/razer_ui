# RzLightingEngineApi 动作表（从前端提取）

来源：`.ref\frontend\static\js\2973.2f1d1e6a.chunk.js`。

灯光引擎不是普通导出函数，而是**动作 ID 协议**：调用方送一个数字动作，
引擎按表分发。这张表就是方案 2 驱动灯光要用的协议。

## 动作 ID → 函数

| ID | 函数 |
|---:|---|
| 1 | `LightingEngine_CreateDevice` |
| 2 | `LightingEngine_DestroyDevice` |
| 3 | `LightingEngine_CreateEngine` |
| 4 | `LightingEngine_DestroyEngine` |
| 17 | `LightingEngine_AddDevice` |
| 18 | `LightingEngine_RemoveDevice` |
| 19 | `LightingEngine_SetDevicePosition` |
| 20 | `LightingEngine_GetNumRows` |
| 21 | `LightingEngine_GetNumCols` |
| 33 | `LightingEngine_AddEffect` |
| 34 | `LightingEngine_RemoveEffect` |
| 35 | `LightingEngine_SetMask` |
| 36 | `LightingEngine_GetMask` |
| 49 | `LightingEngine_EnableEngine` |
| 50 | `LightingEngine_SetPreviewMode` |
| 51 | `LightingEngine_SyncEffect` |
| 52 | `LightingEngine_GetCurrentFrame` |
| 53 | `LightingEngine_GetCurrentFrameMultipleDevices` |
| 57344 | `LightingEngine_InitChromaAiColorGen` |
| 61440 | `LightingDevice_UpdateDeviceConfig` |
| 61441 | `LightingDevice_GetNumRows` |
| 61442 | `LightingDevice_GetNumCols` |
| 61443 | `LightingDevice_AddLinkedDevice` |
| 61444 | `LightingDevice_RemoveLinkedDevice` |
| 61445 | `LightingDevice_InputEvent` |
| 61446 | `LightingDevice_MapInput` |
| 61447 | `LightingDevice_SetBrightnessState` |
| 61448 | `LightingDevice_GetBrightnessState` |
| 61449 | `LightingDevice_SetBrightnessLevel` |
| 61450 | `LightingDevice_GetBrightnessLevel` |
| 61451 | `LightingDevice_Identify` |
| 61697 | `LightingDevice_GetNumRegions` |
| 61698 | `LightingDevice_EnumRegion` |
| 61699 | `LightingDevice_GetRegionInfo` |
| 61700 | `LightingDevice_UpdateRegionInfo` |
| 65520 | `LightingEngine_Terminate` |

## 效果 / 事件 ID

| ID | 名称 |
|---:|---|
| 1 | `LightingEngine_EffectId_Ripple` |
| 1 | `LightingEngine_Event_CustomMode` |
| 2 | `LightingEngine_EffectId_Wave` |
| 2 | `LightingEngine_Event_RenderFrame` |
| 3 | `LightingEngine_EffectId_Reactive` |
| 3 | `LightingEngine_Event_InputNotify` |
| 4 | `LightingEngine_EffectId_Spectrum` |
| 5 | `LightingEngine_EffectId_Breath` |
| 6 | `LightingEngine_EffectId_Static` |
| 7 | `LightingEngine_EffectId_Starlight` |
| 8 | `LightingEngine_EffectId_Fire` |
| 11 | `LightingEngine_EffectId_Ambience` |
| 12 | `LightingEngine_EffectId_AudioVU` |
| 13 | `LightingEngine_EffectId_ColorWheel` |
| 15 | `LightingEngine_EffectId_SpecialEdition` |
| 256 | `LightingEngine_Event_PreviewFrame` |

## 设备侧动作 ID

| ID | 名称 |
|---:|---|
| 61440 | `LightingDevice_UpdateDeviceConfig` |
| 61441 | `LightingDevice_GetNumRows` |
| 61442 | `LightingDevice_GetNumCols` |
| 61443 | `LightingDevice_AddLinkedDevice` |
| 61444 | `LightingDevice_RemoveLinkedDevice` |
| 61445 | `LightingDevice_InputEvent` |
| 61446 | `LightingDevice_MapInput` |
| 61447 | `LightingDevice_SetBrightnessState` |
| 61448 | `LightingDevice_GetBrightnessState` |
| 61449 | `LightingDevice_SetBrightnessLevel` |
| 61450 | `LightingDevice_GetBrightnessLevel` |
| 61451 | `LightingDevice_Identify` |
| 61452 | `LightingDevice_FlushFrame` |
| 61697 | `LightingDevice_GetNumRegions` |
| 61698 | `LightingDevice_EnumRegion` |
| 61699 | `LightingDevice_GetRegionInfo` |
| 61700 | `LightingDevice_UpdateRegionInfo` |

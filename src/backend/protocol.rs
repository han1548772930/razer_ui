//! `RzLightingEngineApi` 的动作 ID 协议。
//!
//! # 这不是普通的导出函数库
//!
//! 灯光引擎的调用方式是**动作 ID 分发**：调用方送一个数字动作，
//! 引擎按表分发到具体函数。这张表**原样提取自雷云前端**
//! （`.ref/frontend/static/js/2973.2f1d1e6a.chunk.js`），
//! 全量清单见 [`docs/re/02-lighting-actions.md`](../../docs/re/02-lighting-actions.md)。
//!
//! 效果 ID 与 Razer 老版 Chroma SDK 的 `EFFECT_TYPE` 同源，可交叉印证。
//!
//! # 与 `lighting_driver` 的分工
//!
//! | DLL | 接口形态 | 用途 |
//! |---|---|---|
//! | `RzLightingEngineApi` | 动作 ID（本文件） | 效果编排：建引擎/加设备/加效果/渲染帧 |
//! | [`super::lighting`] `lighting_driver` | `Configure(json)` | 底层写出：注册设备、下发帧 |
//!
//! 两者配合：引擎算帧 → 驱动写出。

#![allow(dead_code)]

/// 引擎侧动作（`LightingEngine_*`）。
///
/// 数值即线上协议值，**不要改动**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum EngineAction {
    CreateDevice = 1,
    DestroyDevice = 2,
    CreateEngine = 3,
    DestroyEngine = 4,
    AddDevice = 17,
    RemoveDevice = 18,
    SetDevicePosition = 19,
    GetNumRows = 20,
    GetNumCols = 21,
    AddEffect = 33,
    RemoveEffect = 34,
    SetMask = 35,
    GetMask = 36,
    EnableEngine = 49,
    SetPreviewMode = 50,
    SyncEffect = 51,
    GetCurrentFrame = 52,
    GetCurrentFrameMultipleDevices = 53,
    InitChromaAiColorGen = 57344,
    Terminate = 65520,
}

impl EngineAction {
    /// 前端表里的名字，用于日志与文档对照。
    pub fn name(self) -> &'static str {
        match self {
            Self::CreateDevice => "LightingEngine_CreateDevice",
            Self::DestroyDevice => "LightingEngine_DestroyDevice",
            Self::CreateEngine => "LightingEngine_CreateEngine",
            Self::DestroyEngine => "LightingEngine_DestroyEngine",
            Self::AddDevice => "LightingEngine_AddDevice",
            Self::RemoveDevice => "LightingEngine_RemoveDevice",
            Self::SetDevicePosition => "LightingEngine_SetDevicePosition",
            Self::GetNumRows => "LightingEngine_GetNumRows",
            Self::GetNumCols => "LightingEngine_GetNumCols",
            Self::AddEffect => "LightingEngine_AddEffect",
            Self::RemoveEffect => "LightingEngine_RemoveEffect",
            Self::SetMask => "LightingEngine_SetMask",
            Self::GetMask => "LightingEngine_GetMask",
            Self::EnableEngine => "LightingEngine_EnableEngine",
            Self::SetPreviewMode => "LightingEngine_SetPreviewMode",
            Self::SyncEffect => "LightingEngine_SyncEffect",
            Self::GetCurrentFrame => "LightingEngine_GetCurrentFrame",
            Self::GetCurrentFrameMultipleDevices => "LightingEngine_GetCurrentFrameMultipleDevices",
            Self::InitChromaAiColorGen => "LightingEngine_InitChromaAiColorGen",
            Self::Terminate => "LightingEngine_Terminate",
        }
    }
}

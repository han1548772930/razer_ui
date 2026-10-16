# Local-first product scope

- The product must work without an account or Internet connection. Implement all local device configuration, host/background services, native libraries and plugins, storage, OS integration, local installation and lifecycle paths. See `docs/re/local-first-scope.md`.
- Implement device discovery, direct communication, queries, state observation, mutations, device/service write-back, profiles, mappings/macros, lighting, audio/camera settings, local presets, persistence, errors, cancellation and cleanup. Local firmware transfer from a local file and local DLL replacement/write-back remain in scope.
- Remove accounts/login, cloud identity, cloud synchronization/storage, online catalogs/content, remote downloads, remote update orchestration and other Internet-dependent features from product UI and dispatch paths. Do not retain disabled placeholders or fabricate successful operations.
- Local IPC, loopback and sockets communicating with installed local services remain in scope. Classify features by their actual call chains: Cloud EQ local preset storage, DSP/HID writes and local restore are required; remote catalog retrieval and cloud sync are excluded.
- Keep shared Rust protocol logic independent of the OS. Isolate platform-specific HID, COM, driver and service adapters; unsupported capabilities must be explicit. OpenLogi is an architecture reference only. Razer protocol evidence must come from current Razer source and binaries.
- Distinguish local draft persistence, simulated transport results and device/service write-back. Do not fabricate successful device reads, writes or saves.

# Full UI implementation

- Implement each local UI page completely from current original JS/CSS and resources. Follow `docs/re/ui-readonly-first-roadmap.md`.
- Trace the mounted component tree and all product/version/accessory branches. Match layout, sizes, typography, colors, images/icons, wording, tabs, panels, drawers, dialogs, tooltips and keyboard/focus behavior, including normal, selected, disabled, loading, empty and error states.
- Connect every action, including edit/add/delete and Apply/Save/Cancel, through caller, parameters, transport/service, response, state refresh, confirmation, cancellation, persistence and cleanup.
- Preserve original optimistic updates, failure and confirmation semantics. Do not invent dialogs, rollback or getter-to-UI updates without a current-source call chain. Keep additional Rust diagnostics separate from source UI state.
- Record source acquisition, reverse-engineered semantics, Rust implementation, UI/backend connection, static/mock verification and runtime acceptance separately. Partial implementation must list remaining items. A page is complete only when its entire source-to-Rust/UI/backend checklist is satisfied; routes, descriptors, wrappers and subsets of controls are insufficient.
- Keep missing source, unresolved indirect calls, ABI/protocol branches and absent implementations explicit. Do not label them completed or permanently deferred.
- 未实现的功能必须继续实现，不得仅登记缺口后留空或结束对应任务。缺项清单用于驱动后续实现，不是交付结果；必须沿当前源码继续完成具体逻辑、平台适配、UI／后端接线、持久化、取消和清理。
- 不得用 TODO、空函数、占位接口、恒定返回“不支持／未连接”或仅有 mock 的实现代替范围内功能。尚缺证据时继续获取和静态逆向，查清实际调用链后实现；不得杜撰协议、界面行为或伪造成功。仅 AGENTS.md 已明确排除的联网功能可以移除；受执行限制禁止的真实运行验收须如实保持未验证。

# GPUI Kit

- Before GPUI changes, read `.agents/skills/gpui-kit/SKILL.md` and `.agents/skills/gpui-kit-design-guides/SKILL.md`, including applicable Coding Guides, Design Guides and mechanism/component references.
- Confirm API signatures against the locked GPUI Kit version. Keep retained state, subscriptions, tasks, focus, stable domain IDs, scrolling and side effects with their proper owners.
- Use semantic components for behavior and product-owned theme/scale tokens for source-verified presentation. Match the original Razer pages; source-derived exceptions to guide defaults need current-source citations.
- Apply the execution limits below to skill verification recipes. Real-window/application/device acceptance remains unverified.

# Current source

- Use the latest official stable Razer Synapse sources. Dashboard originals and their deployment-stability receipt are under `.analysis/inputs/frontend/synapse-dashboard-current/`; 124 referenced source maps are unavailable. Complete current host and device source sets still require acquisition and a version audit.
- Retained originals are under `.analysis/inputs/`, indexed by `.analysis/inputs.json`. Verify acquisition provenance, current version, manifests, byte identity and dependency/resource closure before using them as implementation evidence.
- Do not recreate, read or use `.ref/frontend/`, `.ref/synapse-asar/`, `.ref/host-4.0.821/` or `.work/latest-source-check/host-4.0.821/` as implementation evidence.
- Use maintained static extraction tools in `tools/`. Do not use historical `.ref/tools/` scripts.
- Analysis results must identify complete original inputs, tool versions, full source/function bodies, failures and unresolved chains.

# Analysis workflow and layout

- Use installed `E:/IDA` (IDA Pro 9.1) or `D:/ida/IDA Pro 8.3` and Hex-Rays for native static analysis. Analyze verified private input copies; retain byte hashes, image base, RVA/function boundaries, complete pseudocode/disassembly, types and cross references. Inspect current JS/CSS directly for page behavior.
- Record analyzer versions, capabilities and limitations in `docs/re/tooling.md`.
- Maintain one comprehensive workflow through `tools/workspace.py`. Capture all pages, branches, JS/CSS/resources, host/background services, native libraries, local IPC/storage/OS integration and lifecycle paths. Reuse complete captures for queries; supplement explicit unresolved items.
- Keep current workflow documents in `docs/`, maintained tools in `tools/`, original inputs/references/results in `.analysis/`, and disposable scratch in `.work/`. IDA MCP configuration and plugins belong under `tools/ida/`. Keep analysis tools separate from product runtime dependencies.
- Keep missing inputs, unresolved calls and unimplemented local chains in `.analysis/results/gaps.json`. Do not create a separate script or progress document for each detail, or retain historical archives.
- Build/test fixtures belong to the owning crate's `testdata/`. Builds and tests must not depend on scratch paths or analysis reports.
- Compute evidence SHA-256 from actual input bytes and store it in evidence manifests. Use project-relative paths. Do not use workstation paths or source/code evidence hashes as runtime compatibility gates. Preserve protocol-defined hashes, checksums and versions.

# Execution limits

- Allow static analyzers, source parsing, resource preparation/validation, formatting, `cargo check --locked --all-targets`, pure Rust unit tests and simulated transport tests with `cargo test`.
- Do not run the application, standalone builds for runtime acceptance, vendor JavaScript, target DLLs/exports, helpers or installers. Do not invoke debugger execution or perform real vendor-service/device operations.
- Downloading official source/binary evidence for static analysis is permitted acquisition work. It does not add Internet-dependent product features to the implementation scope.

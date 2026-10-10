"""Compact current-source/consumer matrix; static evidence is never execution.

All source paths come from maintained current receipts. This tool reads data,
hashes files and indexes source text; it never imports vendor JS/native code.
Literal Rust references are locators, not proof of implemented semantics.
"""
from __future__ import annotations

import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs/re/whole-program-closure-current-evidence.json"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_json(name):
    raw = (ROOT / name).read_bytes()
    return json.loads(gzip.decompress(raw) if name.endswith(".gz") else raw)


def receipt(name):
    raw = (ROOT / name).read_bytes()
    return {"path": name, "sha256": digest(raw), "bytes": len(raw)}


def rust_anchor(path, token):
    text = (ROOT / path).read_text(encoding="utf-8-sig")
    at = text.find(token)
    if at < 0:
        raise ValueError(f"Reviewed consumer anchor missing: {path}: {token}")
    return {"path": path, "symbol_or_token": token,
            "line": text[:at].count("\n") + 1}


# These are reviewed boundaries, not keyword-derived completion statuses.
DOMAINS = [
    ("startup_and_single_instance", ["main.js"],
     [("crates/razer-app/src/lib.rs", "pub fn run")],
     "Current host launch options, startup collections, recovery and single-instance branches remain unclosed."),
    ("key_storage", ["keyStorage.js"],
     [("crates/razer-storage/src/host.rs", "pub struct HostStorage"),
      ("crates/razer-service/src/runtime/mod.rs", "ServiceRequest::HostStorageCall")],
     "HostStorage implements URL subscriptions, tombstones and IPC event queues; actual host window registration and page consumers remain absent. Local draft files are separate."),
    ("memory_storage", ["modules/memory_storage/index.js"],
     [("crates/razer-storage/src/host.rs", "pub struct HostStorage"),
      ("crates/razer-service/src/runtime/mod.rs", "ServiceRequest::HostStorageEvents")],
     "Per-URL ordered maps, truthy getters, old/new events, targetUrlArray and reset are implemented; actual host/page consumers and cross-window lifecycle remain unconnected."),
    ("window_storage", ["modules/window_storage/index.js"],
     [("crates/razer-storage/src/host.rs", "pub struct HostStorage"),
      ("crates/razer-service/src/runtime/mod.rs", "ServiceRequest::HostStorageClose")],
     "Per-URL maps, subscription rules, clear/remove distinctions and reset are implemented; host window lifecycle registration and UI actions remain unconnected."),
    ("module_install_remove_cancel", ["preload.js"],
     [("crates/razer-shell/src/shell.rs", "ModuleCatalogEvent::ServiceCommand"),
      ("crates/razer-app-pages/src/module_service_remove.rs", "service-remove-confirm-")],
     "UI confirmation emits a request; shell reports not executed. Installer, cache verification, service status publisher and clear-settings persistence are absent."),
    ("main_program_update", ["mainSubFunction.js", "lib/aio.js"],
     [("crates/razer-service/src/runtime/mod.rs", "pub fn run_worker")],
     "Network-only update flows are excluded for local mode. Local host package installation, launcher relaunch and old-version cleanup remain unclosed; worker lifecycle does not implement them."),
    ("firmware_update", ["main.js"],
     [("crates/razer-app-pages/src/firmware_update/state.rs", "local preview data only")],
     "Source-stage preview is not updater discovery, firmware transfer, hardware progress, completion, cancellation or rollback."),
    ("windows_service_control", ["serviceFunction.js"],
     [("crates/razer-platform/src/windows_service_status.rs", "pub fn query"),
      ("crates/razer-platform/src/windows_service_status.rs", "pub fn start"),
      ("crates/razer-platform/src/windows_service_status.rs", "pub fn stop"),
      ("crates/razer-platform/src/platform/windows/services.rs", "fn start_handle"),
      ("crates/razer-platform/src/platform/windows/services.rs", "fn stop_handle"),
      ("crates/razer-service/src/runtime/mod.rs", "ServiceRequest::WindowsServiceStatus"),
      ("crates/razer-service/src/runtime/mod.rs", "ServiceRequest::WindowsServiceStart"),
      ("crates/razer-service/src/runtime/mod.rs", "ServiceRequest::WindowsServiceStop"),
      ("crates/razer-service/src/runtime/windows/native.rs", "ServiceRequest::Shutdown")],
     "IDA-proved query/start/stop for 12 allowed names, C ASCII comparison, SCM permissions, dependent recursion, checkpoint/waitHint polling, exit semantics and handle cleanup are implemented in the Windows adapter and shared IPC without loading RzPowerTool. Other platforms explicitly reject SCM operations. Original host/page consumers, locale changes and elevated helper launch remain unclosed: RzPowerTool requiresAdministrator while the worker does not acquire an elevated token. Exit 1 confirms the parent result only; original dependency errors are ignored. IPC timeout does not undo possible service side effects; mapping/audio shutdown is separate."),
    ("device_security", ["modules/security/win/index.js"], [],
     "RzSecurityTool --verify-device-security helper PE is acquired statically; its return semantics and Rust security consumer remain unclosed."),
    ("notifications_and_deep_links", ["nativeNotificationHandler.js"], [],
     "Missing Windows addon worker, URI HMAC verification/one-use key deletion, safeStorage encrypted key file, TTL and pending-protocol queue."),
    ("identity_and_login", ["lib/identityPipe.js", "lib/getIdentityFeature.js"], [],
     "Missing original identity pipe/event propagation, authentication response, credential handling and secure storage."),
    ("iot_and_lamp_array", ["modules/IoT/IoTNativeAction.js", "modules/LampArray/LampArrayAction.js"],
     [("crates/razer-shell/src/shell.rs", "GamerRoomEvent::DeviceCommand")],
     "Shell does not send device command; original native API/transport, notification and write confirmation remain unclosed."),
    ("lighting", ["modules/lighting/ffiLightingDriver.js"],
     [("crates/razer-service/src/lighting.rs", "type FnShutdown")],
     "Existing adapter loads lighting_driver; it is not a Rust replacement of lighting engine/frame/device lifecycle."),
    ("simple_audio_volume", ["modules/simple_service/win/index.js"],
     [("crates/razer-device/src/simple_audio_volume.rs", "pub fn scalar_to_volume"),
      ("crates/razer-service/src/simple_audio_volume.rs", "pub fn write"),
      ("crates/razer-service/src/runtime/windows/audio_volume.rs", "fn observe"),
      ("crates/razer-service/src/runtime/portable.rs", "ServiceRequest::AudioVolumeWrite"),
      ("crates/razer-shell/src/shell/audio_volume.rs", "pub(super) fn request_audio_volume"),
      ("crates/razer-pages/src/features/audio_volume.rs", "pub fn finish_volume")],
     "Current IDA-recovered shared speaker/microphone endpoint calls use Core Audio without simple_service DLL loading: native quantization, volume-before-mute, equal-value skips, partial failure without rollback, and separate real readback. Current 1352 speaker page connects actual exact-container endpoint reads, release/toggle writes, serialized cancellable requests and stale-response filtering; observations do not overwrite local saved drafts. Native productName fallback, original notification-maintained cache, endpoint replacement/notification lifecycle, full initialization retry behavior and other product/microphone UI consumers remain incomplete. Windows-only endpoint capabilities explicitly fail on other platforms."),
    ("host_ffi_lifecycle", ["modules/ffi/FFIPreloadMain.js", "modules/ffi_subprocess/FFIProcess.js"],
     [("crates/razer-service/src/runtime/mod.rs", "pub fn run_worker"),
      ("crates/razer-service/src/native_read.rs", "ManuallyDrop::new")],
     "Isolated worker and gated original-DLL getters are partial; suspend/shutdown/exit-device callback maps, subprocess crash/restart and each library ABI remain unclosed."),
    ("hardware_interrupt_and_receiver_pairing", ["modules/hidHardwareEvents/index.js", "modules/mapping_engine/win/index.js"],
     [("crates/razer-device/src/backend.rs", "fn read_interrupt"),
      ("crates/razer-hid/src/transport/hidapi.rs", ".read_timeout(report, timeout)"),
      ("crates/razer-device/src/receiver_pairing.rs", "pub trait HardwareEvents"),
      ("crates/razer-device/src/receiver_pairing.rs", "pub fn pair"),
      ("crates/razer-service/src/runtime/windows/receiver_events.rs", "impl HardwareEvents for Events"),
      ("crates/razer-service/src/runtime/windows/receiver_pairing.rs", "pub(super) struct Controller"),
      ("crates/razer-shell/src/shell/receiver_pairing.rs", "fn submit_dock_pairing"),
      ("crates/razer-storage/src/receiver_pairing.rs", "pub struct PairingCache")],
     "IDA-proved mapping_engine all-collection ContainerId/PID selector and actual interrupt bytes are implemented without vendor loading. Typed asynchronous Start/Poll/Cancel controller and Dock UI preserve actual event54/55 completion, owner epochs, cancellation cleanup and full-array/lane scan semantics. Local ye/ge physical-key cache add/remove is connected only after real hardware confirmation; it is a local document adaptation, not a device read or original Chromium file. Post-pair metadata, product runtime/serial publishing, profile combining/reconnect, and equivalent non-Windows physical grouping remain separate required gaps. Runtime acceptance not run."),
    ("audio_util_notifications", [],
     [("crates/razer-device/src/audio_notification.rs", "pub struct NotificationQueue"),
      ("crates/razer-service/src/audio_notification.rs", "pub fn enable"),
      ("crates/razer-service/src/platform/windows/audio_notification.rs", "struct Registration"),
      ("crates/razer-service/src/runtime/portable.rs", "ServiceRequest::AudioNotificationsEnable"),
      ("crates/razer-shell/src/shell/audio_notifications.rs", "fn run"),
      ("crates/razer-shell/src/shell/audio_notifications.rs", "fn sync_audio_notifications")],
     "Current RzAudioUtil 1.0.3.1 IDA proves filter 0/1/3, exact payload, repeated callback enable and all-of-type disable. Direct Windows COM callback and worker IPC implement registration, events, unregister, reference ownership and apartment cleanup; other platforms explicitly reject this OS capability. Shell starts one retained worker per uniquely observed current instance of the eight source-verified products, enables once, drains actual notifications, and disables/shuts down on loss or quit; 1401 is excluded. No application/notification registration runtime acceptance is performed. Native event DeviceChange differs in case from product deviceChange listeners, which only log; no automatic device cache refresh is invented."),
    ("audio_util_source_versions", [],
     [("crates/razer-service/src/native_query.rs", "fn source_rzaudioutil_version")],
     "Current GetDLLVersion bodies for eight 1.0.3.1 products and separate 1401 1.0.1.1 resource are replaced by byte-proved constant formatting without vendor DLL loading. Other libraries' generic version path still loads original exports and allocators; a deterministic version query is not a complete DLL replacement."),
    ("sysutils_foreground_monitor", ["modules/sysutil/win/index.js"],
     [("crates/razer-platform/src/foreground_monitor.rs", "pub struct ForegroundMonitor"),
      ("crates/razer-platform/src/platform/windows/foreground_monitor.rs", "SetWinEventHook(3, 0x17"),
      ("crates/razer-service/src/runtime/portable.rs", "ServiceRequest::ForegroundMonitorEvents")],
     "Current IDA hooks, listener thread/window, 300 ms debounce, Explorer recheck, UWP child resolution and path event formatter are implemented with URL subscriptions and worker IPC, without SysUtilsNative loading. Original per-product activation/profile task, direct scroll/haptic setter consumer and non-default CRT locale case comparison remain unclosed; a shared task body in 182 middleware is not evidence that DeathAdder supports or enables haptic scrolling. Other platforms explicitly reject this Windows hook capability; runtime acceptance is not run."),
    ("sysutils_keyboard_layout_query", ["modules/sysutil/win/index.js"],
     [("crates/razer-platform/src/keyboard_layout.rs", "pub fn get"),
      ("crates/razer-platform/src/platform/windows/keyboard_layout.rs", "GetKeyboardLayoutNameA"),
      ("crates/razer-service/src/runtime/mod.rs", "ServiceRequest::KeyboardLayoutRead")],
     "Current IDA calling-thread KLID getter preserves zero initialization, ignored BOOL, hexadecimal parse and signed int FFI bits; independent Windows adapter and typed IPC replace this getter without vendor loading. Page/macro consumers remain unconnected. The separate two-second timer is recorded in its own domain; macOS/Linux equivalents require their own source proof."),
    ("sysutils_keyboard_layout_monitor", ["modules/sysutil/win/index.js"],
     [("crates/razer-platform/src/keyboard_layout_monitor.rs", "pub struct KeyboardLayoutMonitor"),
      ("crates/razer-platform/src/platform/windows/keyboard_layout_monitor.rs", "SetTimer")],
     "IDA-proved calling-thread 2000ms timer, signed change events, repeated start and original host stop/foreground-list bug are implemented with thread-affine ownership and URL routing. No current product start/stop caller was found by the maintained static search. Actual message-pumped UI owner integration remains a gap; blocking-stdin worker or a substitute thread must not be presented as an equivalent consumer. No timer runtime acceptance performed."),
    ("sysutils_system_properties_launch", ["modules/sysutil/win/index.js"],
     [("crates/razer-platform/src/system.rs", "pub fn open_display_settings"),
      ("crates/razer-platform/src/platform/windows/system.rs", "WinExec")],
     "Five current native command bodies and existing mouse/keyboard/audio/display page callers are verified: control keyboard, control mmsys.cpl sounds, control main.cpl, sndvol.exe and explorer ms-settings:display use WinExec SW_SHOW=5 without vendor DLL loading. Native launch failures are surfaced explicitly. Generic parameterized msSettings, other system launch functions and full DLL Initialize/Terminate chains remain gaps; non-Windows capability is explicit and no system commands were executed."),
    ("whole_program_exit", ["main.js"],
     [("crates/razer-service/src/runtime/windows/native.rs", "self.shutdown_macro_recording")],
     "Worker mapping/simple shutdown exists; host cannot-exit guards, app collection ordering, poweroff, pending callbacks and full plugin teardown are not equivalent."),
    ("compatibility_and_mutex", ["lib/exeCompatibility.js", "lib/RzMutx.js"], [],
     "Original Windows compatibility-registry checks and token-based cancellable mutex have no equivalent consumer recorded."),
]


def inspect():
    names = ["assets/data/native-library-inventory.json",
             "docs/re/host-architecture-current-evidence.json",
             "docs/re/full-source-corpus-current-evidence.json.gz",
             "docs/re/dll-device-communication-current.json.gz"]
    inventory, architecture, corpus, graph = [read_json(n) for n in names]
    host_files = {f["path"]: f for f in architecture["files"]}
    source_root = ".ref/host-4.0.827/electron/"
    domains = []
    for name, sources, anchors, gap in DOMAINS:
        references = []
        for source in sources:
            path = source_root + source
            item = host_files[path]
            current = receipt(path)
            if current["sha256"] != item["sha256"]:
                raise ValueError(f"Current architecture source hash mismatch: {path}")
            current["functions"] = [{k: f[k] for k in ("name", "start", "end")}
                                    for f in item.get("functions", [])]
            references.append(current)
        domains.append({"id": name, "current_sources": references,
                        "reviewed_rust_boundaries": [rust_anchor(*a) for a in anchors],
                        "status": "partially_implemented" if anchors else "consumer_missing",
                        "gap": gap, "runtime_acceptance": "not_run"})

    rust = [(p.relative_to(ROOT).as_posix(), p.read_text(encoding="utf-8-sig"))
            for p in sorted((ROOT / "crates").rglob("*.rs"))]
    libraries = []
    for library in inventory["libraries"]:
        declarations = library["declared_functions"]
        # Eligible names still depend on manifest resource/product identity,
        # initialization session, allocator, platform, and callback gates.
        versions = [d["name"] for d in declarations if d["name"] in
                    ("GetDLLVersion", "GetDllVersion", "GetLibVersion", "getSDKVersion")
                    and not d["args"]]
        resources = library["resources"]
        refs = []
        for path, text in rust:
            at = text.lower().find(library["id"].lower())
            if at >= 0:
                refs.append({"path": path, "line": text[:at].count("\n") + 1})
        libraries.append({"id": library["id"], "products": library["products"],
                          "declared_functions": len(declarations),
                          "signature_conflicts": len(library["declaration_conflicts"]),
                          "session_candidates": len(library["sessions"]),
                          "version_declaration_candidates": versions,
                          "resources": [{k: r[k] for k in
                                         ("file", "sha256", "bytes", "machine")} for r in resources],
                          "literal_rust_locators_not_consumer_proof": refs,
                          "entire_library_replacement": "not_completed",
                          "acquisition_gap": not bool(resources)})

    native_files = []
    for f in graph["files"]:
        path = ROOT / f["path"]
        if digest(path.read_bytes()) != f["sha256"]:
            raise ValueError(f"Current native graph byte mismatch: {f['path']}")
        native_files.append({k: f[k] for k in
                             ("path", "file", "sha256", "bytes", "machine", "semantic_status", "status")})
        native_files[-1]["import_libraries"] = sorted({i["library"] for i in f["imports"]})
        native_files[-1]["api_category_import_counts"] = dict(sorted(Counter(
            i["api_category"] for i in f["imports"] if i.get("api_category")).items()))

    # Host corpus contains non-Windows native prebuilds beyond the 19 Windows
    # plugins covered by the PE instruction graph. Include them in full scope.
    host_binaries = [{k: f[k] for k in ("path", "sha256", "bytes")}
                     for f in corpus["files"] if f["scope"] == "host" and f["role"] == "native_binary"]
    helpers = {}
    for path, f in sorted(host_files.items()):
        text = (ROOT / path).read_text(encoding="utf-8-sig")
        for match in re.finditer(r"\b[A-Za-z0-9_-]+\.exe\b", text, re.I):
            name = match.group()
            helpers.setdefault(name, []).append({"path": path, "sha256": f["sha256"],
                                                "utf16_offset": len(text[:match.start()].encode("utf-16-le")) // 2})
    helper_evidence_path = ROOT / "docs/re/host-helpers-current-evidence.json"
    helper_evidence = json.loads(helper_evidence_path.read_text(encoding="utf-8")) if helper_evidence_path.exists() else {"helpers": []}
    helper_bodies = {Path(row["path"]).name.lower(): row for row in helper_evidence.get("helpers", [])}
    helper_rows = [{"file": name, "call_string_references": refs,
                    "host_body_paths": [f["path"] for f in host_binaries
                                        if Path(f["path"]).name.lower() == name.lower()],
                    "acquired_static_body": helper_bodies.get(name.lower()),
                    "reference_only_not_body_semantics": True}
                   for name, refs in sorted(helpers.items())]

    return {"schema_version": 1, "scope": "Entire current program: all products, pages, host, services, native libraries, device communication, storage, update and lifecycle",
            "scope_boundary": {"implementation_scope": ["all source-verified products and pages", "all host and service chains", "all native libraries and plugins", "device communication and DLL replacement", "local storage, local security, local installation and local lifecycle"],
                               "excluded_by_product_mode": ["online update manifests", "remote package download", "cloud update orchestration and other network-only upgrade flows"],
                               "status_policy": "All required local domains remain in scope; only the listed network-only upgrade flows are excluded. Completion requires source evidence, Rust implementation, consumer connection and permitted static verification, with runtime acceptance recorded separately; unresolved branches remain explicit gaps.",
                               "runtime_policy": "Application, DLL, vendor JavaScript and hardware execution remain prohibited during development, so runtime acceptance is recorded separately and never fabricated."},
            "method": "Static receipt/hash verification and reviewed Rust boundary locators; no vendor code execution",
            "generator_sha256": digest(Path(__file__).read_bytes()),
            "inputs": [receipt(n) for n in [*names,
                "docs/re/receiver-query-projection-current-evidence.json",
                "docs/re/cross-platform-hid-current-evidence.json",
                "docs/re/simple-audio-volume-current-evidence.json",
                "docs/re/leviathan-volume-ui-current-evidence.json",
                "docs/re/mouse-dpi-stages-current-evidence.json",
                "docs/re/hid-interrupt-current-evidence.json",
                "docs/re/receiver-pairing-transport-current-evidence.json",
                "docs/re/audio-util-notifications-current-evidence.json",
                "docs/re/rzaudioutil-dll-version-current-evidence.json",
                "docs/re/sysutils-keyboard-layout-current-evidence.json",
                "docs/re/sysutils-foreground-current-evidence.json",
                "docs/re/sysutils-keyboard-monitor-current-evidence.json",
                "docs/re/sysutils-system-launch-current-evidence.json",
                "docs/re/audio-mixer-page-bindings-current.json"]],
            "summary": {"native_libraries": len(libraries),
                        "libraries_without_acquired_resources": sum(r["acquisition_gap"] for r in libraries),
                        "native_pe_files": len(native_files),
                        "host_native_binary_entries_all_platforms": len(host_binaries),
                        "helper_reference_names": len(helper_rows),
                        "host_helper_exe_bodies": len(helper_bodies),
                        "reviewed_non_product_domains": len(domains),
                        "entire_library_replacements_completed": 0,
                        "entire_program_equivalence_completed": False},
            "domains": domains, "libraries": libraries,
            "native_dependency_matrix": native_files, "host_native_corpus": host_binaries,
            "helper_call_strings": helper_rows,
            "runtime_acceptance": "not_run",
            "limitations": ["Rust literal references and eligible generic FFI names do not prove consumer coverage or direct Rust replacement.",
                            "PE imports do not prove invocation; dynamic DLL loads/virtual calls need caller and parameter recovery.",
                            "Helper PE acquisition and static metadata do not prove branch semantics, process results, or a Rust replacement.",
                            "All mutations and persistence remain required implementation scope; no separate deferred integration phase."]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = inspect()
    raw = (json.dumps(result, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_bytes() != raw:
            raise SystemExit("Whole-program closure evidence is stale; review and regenerate")
    else:
        OUTPUT.write_bytes(raw)
    print(json.dumps(result["summary"], ensure_ascii=False))


if __name__ == "__main__":
    main()

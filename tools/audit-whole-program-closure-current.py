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
      ("crates/razer-service/src/runtime.rs", "ServiceRequest::HostStorageCall")],
     "HostStorage implements URL subscriptions, tombstones and IPC event queues; actual host window registration and page consumers remain absent. Local draft files are separate."),
    ("memory_storage", ["modules/memory_storage/index.js"],
     [("crates/razer-storage/src/host.rs", "pub struct HostStorage"),
      ("crates/razer-service/src/runtime.rs", "ServiceRequest::HostStorageEvents")],
     "Per-URL ordered maps, truthy getters, old/new events, targetUrlArray and reset are implemented; actual host/page consumers and cross-window lifecycle remain unconnected."),
    ("window_storage", ["modules/window_storage/index.js"],
     [("crates/razer-storage/src/host.rs", "pub struct HostStorage"),
      ("crates/razer-service/src/runtime.rs", "ServiceRequest::HostStorageClose")],
     "Per-URL maps, subscription rules, clear/remove distinctions and reset are implemented; host window lifecycle registration and UI actions remain unconnected."),
    ("module_install_remove_cancel", ["preload.js"],
     [("crates/razer-shell/src/shell.rs", "ModuleCatalogEvent::ServiceCommand"),
      ("crates/razer-app-pages/src/module_service_remove.rs", "service-remove-confirm-")],
     "UI confirmation emits a request; shell reports not executed. Installer, cache verification, service status publisher and clear-settings persistence are absent."),
    ("main_program_update", ["mainSubFunction.js", "lib/aio.js"],
     [("crates/razer-service/src/runtime.rs", "pub fn run_worker")],
     "Network-only update flows are excluded for local mode. Local host package installation, launcher relaunch and old-version cleanup remain unclosed; worker lifecycle does not implement them."),
    ("firmware_update", ["main.js"],
     [("crates/razer-app-pages/src/firmware_update/state.rs", "local preview data only")],
     "Source-stage preview is not updater discovery, firmware transfer, hardware progress, completion, cancellation or rollback."),
    ("windows_service_control", ["serviceFunction.js"],
     [("crates/razer-service/src/runtime_native.rs", "ServiceRequest::Shutdown")],
     "Mapping/audio service shutdown is not RzPowerTool get/start/stop service commands; helper PE is acquired statically, but branch semantics, process result and Rust consumer remain unclosed."),
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
    ("host_ffi_lifecycle", ["modules/ffi/FFIPreloadMain.js", "modules/ffi_subprocess/FFIProcess.js"],
     [("crates/razer-service/src/runtime.rs", "pub fn run_worker"),
      ("crates/razer-service/src/native_read.rs", "ManuallyDrop::new")],
     "Isolated worker and gated original-DLL getters are partial; suspend/shutdown/exit-device callback maps, subprocess crash/restart and each library ABI remain unclosed."),
    ("whole_program_exit", ["main.js"],
     [("crates/razer-service/src/runtime_native.rs", "self.shutdown_macro_recording")],
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
            "inputs": [receipt(n) for n in names],
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

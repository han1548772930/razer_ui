"""Static artifact acquisition and professional analyzer orchestration."""
from __future__ import annotations

import concurrent.futures
from contextlib import contextmanager
import base64
from collections import Counter, defaultdict, deque
from datetime import datetime, timezone
import hashlib
import http.client
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import threading
import time
from types import SimpleNamespace
import uuid
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parent.parent


def builtin_data_path(crate, symbol, *, macro_name=None):
    import asset_release
    return asset_release.builtin_data_path(crate, symbol, root=ROOT, macro_name=macro_name)


def save_builtin_data(symbol, value):
    import asset_release
    return asset_release.save_builtin_data(symbol, value, root=ROOT)


def _resolved_path_identity(path):
    """Compare resolved Win32 paths using equivalent DOS/UNC spellings.

    GetFinalPathNameByHandle may return the extended prefix for just one side
    of a containment check while another thread creates the destination. The
    paths must already be resolved so reparse-point traversal remains checked.
    Other device namespaces are deliberately left distinct.
    """
    value = str(path)
    if os.name == "nt":
        if value.lower().startswith("\\\\?\\unc\\"):
            return Path("\\\\" + value[8:])
        if re.match(r"^\\\\\?\\[A-Za-z]:\\", value):
            return Path(value[4:])
    return path


def resolve(name, boundary):
    path = (ROOT / name).resolve()
    canonical = _resolved_path_identity(path)
    root = _resolved_path_identity((ROOT / boundary).resolve())
    if not canonical.is_relative_to(root) or canonical == root:
        raise ValueError(f"Path outside {boundary}: {name}")
    return path


def rel(path):
    return _resolved_path_identity(path.resolve()).relative_to(_resolved_path_identity(ROOT)).as_posix()


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024*1024), b""):
            h.update(block)
    return h.hexdigest()


def save(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(path.name + "." + uuid.uuid4().hex + ".tmp")
    temp.write_text(json.dumps(data, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    try:
        # Windows readers and antivirus can briefly hold a share-delete-denied
        # handle. Keep the complete owned temp until atomic replacement succeeds.
        deadline = time.monotonic() + 5
        while True:
            try:
                temp.replace(path)
                break
            except PermissionError:
                if os.name != "nt" or time.monotonic() >= deadline:
                    raise
                time.sleep(0.05)
    finally:
        temp.unlink(missing_ok=True)
    maintained = ROOT / ".analysis/inputs/typed-tables"
    if path.resolve().is_relative_to(maintained.resolve()):
        import asset_release
        asset_release.regenerate_builtin_path(path, root=ROOT)


def stamp():
    return datetime.now(timezone.utc).isoformat()


@contextmanager
def index_lock(path=None):
    """Serialize acquisition merges; OS locks are released even on process failure."""
    path = path or ROOT / ".analysis/inputs.lock"
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+b") as stream:
        if stream.seek(0, os.SEEK_END) == 0:
            stream.write(b"\0")
            stream.flush()
        deadline = time.monotonic() + 60
        while True:
            try:
                stream.seek(0)
                if os.name == "nt":
                    import msvcrt
                    msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
                else:
                    import fcntl
                    fcntl.flock(stream.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except OSError:
                if time.monotonic() >= deadline:
                    raise TimeoutError(f"Analysis receipt is busy: {rel(path)}")
                time.sleep(0.05)
        try:
            yield
        finally:
            stream.seek(0)
            if os.name == "nt":
                msvcrt.locking(stream.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(stream.fileno(), fcntl.LOCK_UN)


def verify_source_rows(rows):
    """Validate retained originals before a source owner publishes their index."""
    snapshots = {}
    for row in rows:
        name = row.get("path", "")
        if (not isinstance(name, str) or Path(name).is_absolute() or ".." in Path(name).parts
                or not name.startswith((".analysis/inputs/", ".analysis/references/"))):
            raise ValueError("Source publication requires a retained project-relative input: " + str(name))
        path = resolve(name, ".analysis/inputs" if name.startswith(".analysis/inputs/") else ".analysis/references")
        if (ROOT / name).is_symlink():
            raise ValueError("Source publication cannot follow a symbolic link: " + name)
        before = path.stat()
        actual = sha(path)
        after = path.stat()
        if ((before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns)
                or type(row.get("bytes")) is not int or before.st_size != row["bytes"]
                or actual != row.get("sha256")):
            raise ValueError("Source publication byte identity differs: " + name)
        snapshots[path] = (after.st_size, after.st_mtime_ns)
    return snapshots


def index_files(rows, *, publication=None):
    rows = list(rows)
    path = ROOT / ".analysis/inputs.json"
    with index_lock():
        index = json.loads(path.read_bytes())
        snapshots = verify_source_rows(rows) if publication is not None else {}
        if publication is not None:
            if (not publication.get("owner", "").startswith("tools/") or not publication.get("reason")
                    or not isinstance(publication.get("verification"), dict) or not publication["verification"]
                    or publication.get("receipt") not in {row["path"] for row in rows}):
                raise ValueError("Source publication requires its owner, reason, verification and indexed receipt")
        existing = {row["path"]: row for row in index["inputs"]}
        changes = []
        for row in rows:
            previous = existing.get(row["path"])
            if previous != row:
                changes.append({"path": row["path"], "previous": previous, "current": row})
            existing[row["path"]] = row
        if not changes:
            return
        for source, snapshot in snapshots.items():
            current = source.stat()
            if (current.st_size, current.st_mtime_ns) != snapshot:
                raise ValueError("Source changed during index publication: " + rel(source))
        if publication is not None:
            index.setdefault("source_publications", []).append({"recorded_at_utc": stamp(), **publication,
                "changes": changes, "target_execution": False,
                "scope": "Verified retained input identities; source semantics and runtime acceptance remain separate"})
        index["inputs"] = sorted(existing.values(), key=lambda row: row["path"])
        save(path, index)


def reconcile_native_extractions(args=None):
    """Keep failed decompression bytes out of the original-source corpus."""
    index_path = ROOT / ".analysis/inputs.json"
    with index_lock():
        index = json.loads(index_path.read_bytes())
        indexed = {row["path"]: row for row in index["inputs"]}
        receipts, direct, failures = [], {}, []
        for receipt in sorted((ROOT / ".analysis/results/native-packages").glob("*/inspection.json")):
            record = json.loads(receipt.read_bytes())
            if not record.get("files"):
                continue
            binding = {"path": rel(receipt), "sha256": sha(receipt)}
            receipts.append(binding)
            complete = bool(record.get("extracted") and record.get("extraction_exit_code", 0) == 0)
            container = record["container"]
            for artifact in record["files"]:
                row = indexed.get(artifact["path"])
                if row is None:
                    failures.append({"input": artifact["path"], "error": "Extraction artifact is not indexed"})
                    continue
                if any(row.get(key) != artifact.get(key) for key in ("bytes", "sha256", "container", "container_sha256")):
                    raise ValueError("Extraction receipt conflicts with actual indexed input identity: " + artifact["path"])
                direct[row["path"]] = {"complete": complete, "receipt": binding,
                                      "container": container["path"], "exit_code": record.get("extraction_exit_code")}
        rejected = {path for path, state in direct.items() if not state["complete"]}
        while True:
            additions = {path for path, row in indexed.items() if row.get("container") in rejected} - rejected
            if not additions:
                break
            rejected.update(additions)
        changed = []
        for path, row in indexed.items():
            state = direct.get(path)
            if state is None and path not in rejected:
                continue
            updated = dict(row)
            updated["container_extraction_complete"] = path not in rejected
            if path in rejected:
                updated["source_bytes_verified"] = False
                updated["source_validation_failure"] = ("Static decompression did not complete; retained partial bytes are not original-source evidence"
                                                         if state and not state["complete"] else
                                                         "An ancestor input was produced by failed static decompression")
            if state:
                updated["extraction_receipt"] = state["receipt"]
            if updated != row:
                indexed[path] = updated
                changed.append(path)
        index["inputs"] = sorted(indexed.values(), key=lambda row: row["path"])
        save(index_path, index)
    report = {"schema_version": 1, "recorded_at_utc": stamp(), "receipts": receipts,
              "rejected_inputs": sorted(rejected), "updated_inputs": changed, "failures": failures,
              "target_execution": False, "real_device_operations": False,
              "scope": "Partial decompression bytes retained for diagnostics and excluded from original-source analysis; no input bytes altered"}
    output = ROOT / ".analysis/results/native-packages/source-validation.json"
    save(output, report)
    print(json.dumps({"receipt": rel(output), "rejected_inputs": len(rejected),
                      "updated_inputs": len(changed), "failures": len(failures)}), flush=True)
    if not getattr(args, "no_refresh_gaps", False):
        refresh_gaps()
    return report


def host_app_native_manifest_sources(index):
    """Find native manifests declared by actual retained stable host HTML."""
    from html.parser import HTMLParser
    indexed = {row["path"]: row for row in index["inputs"]}
    candidates = defaultdict(list)
    failures = []

    def verified(claim):
        row = indexed.get(claim["path"], {})
        path = resolve(claim["path"], ".analysis/inputs")
        raw = path.read_bytes()
        if any(claim.get(key) != row.get(key) for key in ("bytes", "sha256")) or len(raw) != row["bytes"] or hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError("Host manifest source bytes differ from their indexed acquisition")
        return raw

    class ManifestLinks(HTMLParser):
        def __init__(self):
            super().__init__()
            self.links, self.base = [], None

        def handle_starttag(self, tag, attrs):
            attrs = dict(attrs)
            if tag == "base" and self.base is None:
                self.base = attrs.get("href")
            if tag == "link" and "manifest" in attrs.get("rel", "").lower().split() and attrs.get("href"):
                self.links.append(attrs["href"])

    for receipt in sorted((ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json")):
        if receipt.parent.name.startswith("installed-") or re.fullmatch(r"(?:mouse|keyboard|audio|gamepad|system|accessories)-(?:mw-)?\d+-current", receipt.parent.name):
            continue
        try:
            raw = receipt.read_bytes()
            claim = {"path": rel(receipt), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
            data = json.loads(verified(claim))
            if data.get("source_current_during_acquisition") is not True:
                continue
            originals = {row["url"]: row for row in data.get("files", []) if "path" in row}
            entry = originals.get(data.get("entry_url"))
            if entry is None:
                continue
            parser = ManifestLinks()
            parser.feed(verified(entry).decode("utf-8"))
            base = urllib.parse.urljoin(entry.get("final_url", entry["url"]), parser.base or "")
            stability = {row["url"]: row for row in data.get("stability", [])}
            for literal in parser.links:
                url = artifact_url(base, literal)
                if url is None or urllib.parse.urlsplit(url).netloc != "apps.razer.com":
                    raise ValueError("Host native manifest is outside its official source origin")
                manifest = originals.get(url)
                if manifest is None:
                    raise ValueError("HTML-declared host manifest is absent from the acquisition")
                body = json.loads(verified(manifest))
                if not isinstance(body, dict) or not body.get("resourceManifest", {}).get("resources"):
                    continue
                for item in (entry, manifest):
                    check = stability.get(item["url"], {})
                    if check.get("unchanged") is not True or check.get("before_sha256") != item["sha256"] or check.get("after_sha256") != item["sha256"]:
                        raise ValueError("Host native manifest HTML/declaration stability does not bind actual bytes")
                reference = {"acquisition": claim, "html": {key: entry[key] for key in ("path", "bytes", "sha256", "url")},
                             "manifest": {key: manifest[key] for key in ("path", "bytes", "sha256", "url")},
                             "literal": literal}
                if reference not in candidates[url]:
                    candidates[url].append(reference)
        except Exception as exc:
            failures.append({"kind": "host_app_native_manifest_source_failure", "input": rel(receipt), "error": str(exc)})
    return dict(candidates), failures


def acquire_native(args=None):
    """Acquire production manifests and all declared resources as inert bytes."""
    output = ROOT / ".analysis/inputs/official-native-current"
    receipt = output / "acquisition.json"
    previous_raw = receipt.read_bytes() if receipt.is_file() else None
    previous = json.loads(previous_raw) if previous_raw is not None else {}
    indexed = json.loads((ROOT / ".analysis/inputs.json").read_bytes())
    indexed_paths = {row["path"]: row for row in indexed["inputs"]}
    by_identity = {}
    for row in indexed["inputs"]:
        by_identity.setdefault(row["sha256"], []).append(row)
    endpoints = {}
    include_host_apps = bool(getattr(args, "include_host_app_resources", False) or previous.get("includes_host_app_resources"))
    host_sources, host_source_failures = host_app_native_manifest_sources(indexed) if include_host_apps else ({}, [])
    for item in previous.get("manifests", []):
        parsed = urllib.parse.urlsplit(item["url"])
        if parsed.scheme == "https" and parsed.netloc == "apps.razer.com" and re.fullmatch(r"/synapse/products/[0-9]+/mw/manifest\.json", parsed.path) and not parsed.query and not parsed.fragment:
            endpoints[item["url"]] = item.get("path")
    for url, sources in host_sources.items():
        endpoints[url] = sources[0]["manifest"]["path"]
    for url in getattr(args, "manifest_url", None) or []:
        parsed = urllib.parse.urlsplit(url)
        if (parsed.scheme != "https" or parsed.netloc != "apps.razer.com"
                or not re.fullmatch(r"/synapse/products/[0-9]+/mw/manifest\.json", parsed.path)
                or parsed.query or parsed.fragment):
            raise ValueError("Expected explicit production device manifest URL: " + url)
        endpoints[url] = None
    for path in sorted((ROOT / ".analysis/inputs/device-native-current").rglob("manifest.json.http.json")):
        row = json.loads(path.read_bytes())
        url = row["source_url"]
        if urllib.parse.urlsplit(url).netloc != "apps.razer.com":
            raise ValueError("Expected production Razer manifest: " + url)
        endpoints[url] = rel(path.with_name("manifest.json"))
    if not endpoints:
        raise ValueError("No retained production manifest endpoints; supply --manifest-url from an official device source")

    request_headers = {"User-Agent": "razer-ui-static-evidence", "Accept-Encoding": "identity",
                       "Cache-Control": "no-cache", "Pragma": "no-cache"}

    def fetch(url):
        attempts = []
        for ordinal in range(1, 4):
            attempt = {"attempt": ordinal, "started_at_utc": stamp(), "url": url,
                       "request_headers": request_headers}
            try:
                request = urllib.request.Request(url, headers=request_headers)
                with urllib.request.urlopen(request, timeout=90) as response:
                    final = response.geturl()
                    attempt.update(final_url=final, http_status=response.status, headers=dict(response.headers))
                    parsed_final = urllib.parse.urlsplit(final)
                    if parsed_final.scheme != "https" or parsed_final.netloc not in {"apps.razer.com", "app-assets.razer.com"}:
                        raise ValueError("Unexpected acquisition redirect: " + final)
                    data = response.read()
                    row = {"url": url, "final_url": final, "http_status": response.status,
                           "headers": dict(response.headers), "request_headers": request_headers,
                           "acquired_at_utc": stamp(),
                           "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
                    attempts.append({**attempt, "completed_at_utc": row["acquired_at_utc"],
                                     "bytes": row["bytes"], "sha256": row["sha256"], "complete_response": True})
                    return data, {**row, "download_attempts": attempts}
            except Exception as error:
                attempt.update(error=str(error), error_type=type(error).__name__,
                               completed_at_utc=stamp(), complete_response=False)
                if isinstance(error, http.client.IncompleteRead):
                    attempt.update(received_bytes=len(error.partial), remaining_bytes=error.expected,
                                   partial_sha256=hashlib.sha256(error.partial).hexdigest(),
                                   partial_bytes_retained=False)
                if isinstance(error, urllib.error.HTTPError):
                    attempt.update(http_status=error.code, headers=dict(error.headers or {}), final_url=error.geturl())
                attempts.append(attempt)
                retryable = (isinstance(error, (OSError, urllib.error.URLError, http.client.IncompleteRead))
                             and (not isinstance(error, urllib.error.HTTPError)
                                  or error.code in {408, 429, 500, 502, 503, 504}))
                if not retryable or ordinal == 3:
                    failure = RuntimeError(str(error))
                    failure.download_attempts = attempts
                    raise failure from error

    def manifest(url):
        try:
            raw, row = fetch(url)
            path = output / "manifests" / row["sha256"] / "manifest.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            value = json.loads(raw)
            row.update({"path": rel(path), "previous_input": endpoints[url], "version": value.get("version"),
                        "build_version": value.get("buildVersion"), "git_metadata": value.get("gitMetadata"),
                        "manifest_scope": "host_app" if url in host_sources else "device",
                        "resources": value.get("resourceManifest", {}).get("resources", [])})
            if url in host_sources:
                row["host_app_sources"] = host_sources[url]
                if any(source["manifest"]["sha256"] != row["sha256"] for source in host_sources[url]):
                    row.update(error="Current host app manifest changed from its retained HTML deployment; reacquire that app before native analysis", resources=[])
            return row
        except Exception as exc:
            return {"url": url, "error": str(exc), "acquired_at_utc": stamp(),
                    "download_attempts": getattr(exc, "download_attempts", [])}

    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
        manifests = list(pool.map(manifest, sorted(endpoints)))
    resources = {}
    for row in manifests:
        for resource in row.get("resources", []):
            url = resource["url"]
            parsed = urllib.parse.urlsplit(url)
            if parsed.scheme != "https" or parsed.netloc != "app-assets.razer.com":
                raise ValueError("Resource is not an official artifact URL: " + url)
            key = (url, resource["sha256"].lower(), resource["size"])
            group = resources.setdefault(key, {"resource": resource, "manifests": []})
            group["manifests"].append({"url": row["url"], "path": row["path"],
                                       "sha256": row["sha256"], "resource_name": resource["resourceName"],
                                       "resource_version": resource["resourceVersion"],
                                       "manifest_scope": row["manifest_scope"]})
    old_resources = {(r["url"], r.get("declared_sha256"), r.get("declared_bytes")): r
                     for r in previous.get("resources", [])}

    def artifact(item):
        resource, parents = item["resource"], item["manifests"]
        identity = resource["sha256"].lower()
        row = {"url": resource["url"], "declared_sha256": identity, "declared_bytes": resource["size"],
               "manifests": parents, "target_execution": False}
        old = old_resources.get((row["url"], identity, resource["size"]))
        if old:
            # Keep the actual prior HTTP result, including a manifest mismatch,
            # when a subsequent recheck succeeds, fails or reuses retained bytes.
            # Flatten the history so repeated acquisition does not nest copies.
            row["previous_observations"] = list(old.get("previous_observations", []))
            row["previous_observations"].append({key: value for key, value in old.items()
                                                 if key != "previous_observations"})
        try:
            if resource.get("integrity") and resource["integrity"] != "sha256-" + base64.b64encode(bytes.fromhex(identity)).decode():
                raise ValueError("Manifest SHA and integrity disagree")
            candidates = [old] if old and "path" in old else []
            candidates += by_identity.get(identity, [])
            candidate = next((r for r in candidates if (ROOT / r["path"]).is_file()
                              and (ROOT / r["path"]).stat().st_size == resource["size"]
                              and sha(ROOT / r["path"]) == identity), None)
            filename = Path(urllib.parse.urlsplit(resource["url"]).path).name
            if any(c in filename for c in '\\:*?"<>|') or filename in {"", ".", ".."}:
                raise ValueError("Unsafe native resource filename")
            if candidate:
                row.update({"path": candidate["path"], "bytes": resource["size"], "sha256": identity,
                            "acquisition": "verified retained bytes", "verified_at_utc": stamp()})
                canonical = output / "blobs" / identity / filename
                expected = indexed_paths.get(rel(canonical))
                if (expected and not canonical.exists() and expected.get("sha256") == identity
                        and expected.get("bytes") == resource["size"]):
                    canonical.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(ROOT / candidate["path"], canonical)
                    if canonical.stat().st_size != expected["bytes"] or sha(canonical) != expected["sha256"]:
                        raise ValueError("Restored indexed native path differs from verified retained bytes")
                    row.update(path=rel(canonical), acquisition="verified retained bytes restored to indexed path",
                               restored_from={key: candidate[key] for key in ("path", "bytes", "sha256")})
            else:
                raw, download_row = fetch(resource["url"])
                if download_row["sha256"] != identity or len(raw) != resource["size"]:
                    # Retain what the official endpoint actually returned for
                    # investigation, separately from verified production bytes.
                    path = output / "quarantine" / download_row["sha256"] / filename
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(raw)
                    row["observed_artifact"] = {**download_row, "path": rel(path),
                                                "verified_against_manifest": False}
                    raise ValueError("Resource bytes do not match production manifest")
                path = output / "blobs" / identity / filename
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(raw)
                row.update(download_row)
                row.update({"path": rel(path), "acquisition": "official resource download"})
            row["status"] = "verified"
        except Exception as exc:
            row.update({"status": "failed", "error": str(exc), "recorded_at_utc": stamp()})
            if hasattr(exc, "download_attempts"):
                row["download_attempts"] = exc.download_attempts
        return row

    data = {"schema_version": 1, "started_at_utc": stamp(), "manifest_endpoint_count": len(endpoints),
            "endpoint_scope": "Explicit and retained production device manifests plus HTML-declared current host app manifests when selected; global dependency closure remains pending",
            "includes_host_app_resources": include_host_apps, "host_app_source_failures": host_source_failures,
            "manifests": manifests, "resources": [], "target_execution": False, "real_device_operations": False,
            "tool": {"path": "tools/analysis.py", "sha256": sha(Path(__file__))},
            "http_request_policy": request_headers,
            "http_retry_policy": {"maximum_attempts": 3, "timeout_seconds": 90,
                                  "retryable_http_statuses": [408, 429, 500, 502, 503, 504],
                                  "partial_response_reassembly": False}}
    if previous_raw is not None:
        data["previous_receipt"] = {"bytes": len(previous_raw), "sha256": hashlib.sha256(previous_raw).hexdigest(),
                                    "started_at_utc": previous.get("started_at_utc"),
                                    "finished_at_utc": previous.get("finished_at_utc"),
                                    "scope": "Prior receipt byte identity; prior resource HTTP observations remain inline"}
    save(receipt, data)
    print(json.dumps({"production_manifests": len(manifests), "resources": len(resources),
                      "manifest_failures": sum("error" in row for row in manifests)}), flush=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        for row in pool.map(artifact, resources.values()):
            data["resources"].append(row)
            save(receipt, data)
            if len(data["resources"]) % 20 == 0 or len(data["resources"]) == len(resources):
                print(json.dumps({"resources_checked": len(data["resources"]), "resources_total": len(resources),
                                  "failures": sum(r["status"] == "failed" for r in data["resources"])}), flush=True)

    def stability(row):
        if "error" in row:
            return {"url": row["url"], "error": row["error"], "unchanged": False}
        try:
            _, after = fetch(row["url"])
            return {"url": row["url"], "before_sha256": row["sha256"],
                    "after_sha256": after["sha256"], "unchanged": row["sha256"] == after["sha256"],
                    "download_attempts": after["download_attempts"]}
        except Exception as exc:
            return {"url": row["url"], "error": str(exc), "unchanged": False,
                    "download_attempts": getattr(exc, "download_attempts", [])}

    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
        data["stability"] = list(pool.map(stability, manifests))
    data["finished_at_utc"] = stamp()
    data["source_current_during_acquisition"] = all(row["unchanged"] for row in data["stability"])
    data["all_declared_resources_verified"] = all(row["status"] == "verified" for row in data["resources"])
    data["complete"] = not host_source_failures and not any("error" in row for row in manifests) and data["source_current_during_acquisition"] and data["all_declared_resources_verified"]
    save(receipt, data)
    rows = [{key: row[key] for key in ("path", "bytes", "sha256")} | {
                "kind": "official_host_app_manifest" if row.get("manifest_scope") == "host_app" else "official_device_manifest", "url": row["url"]}
            for row in manifests if "path" in row]
    rows.extend({key: row[key] for key in ("path", "bytes", "sha256")} | {
                    "kind": "official_host_app_resource" if all(parent.get("manifest_scope") == "host_app" for parent in row["manifests"]) else "official_device_resource", "url": row["url"]}
                for row in data["resources"] if row["status"] == "verified")
    rows.extend({key: row["observed_artifact"][key] for key in ("path", "bytes", "sha256", "url")}
                | {"kind": "official_resource_manifest_mismatch", "verified_against_manifest": False,
                   "declared_sha256": row["declared_sha256"], "declared_bytes": row["declared_bytes"]}
                for row in data["resources"] if "observed_artifact" in row)
    rows.append({"path": rel(receipt), "bytes": receipt.stat().st_size, "sha256": sha(receipt), "kind": "acquisition_receipt"})
    index_files(rows)
    inspect_corpus(args)
    print(json.dumps({"receipt": rel(receipt), "complete": data["complete"],
                      "stable_manifests": sum(r["unchanged"] for r in data["stability"])}), flush=True)


def extract_native_packages(args):
    """Use the shared static archive workflow for all declared native scopes."""
    import families
    from types import SimpleNamespace
    acquired = json.loads((ROOT / ".analysis/inputs/official-native-current/acquisition.json").read_bytes())
    urls = [item["url"] for item in acquired.get("manifests", [])
            if args.scope == "all" or item.get("manifest_scope") == "host_app"]
    if not urls:
        raise ValueError("No acquired native manifests match the selected scope")
    families.extract_native_packages(SimpleNamespace(family=args.scope, manifest_url=urls,
        seven_zip=args.seven_zip, workers=args.workers,
        include_manifest_mismatches=args.include_manifest_mismatches), [])
    if not args.no_refresh_gaps:
        refresh_gaps()


def refresh_gaps(owners=None):
    """Reconcile retained receipts without rerunning analyzers or target code."""
    with index_lock():
        if owners is not None:
            with index_lock(ROOT / ".analysis/gaps.lock"):
                return refresh_selected_gaps(owners)
        return _refresh_gaps()


def embedded_input_gaps(data, evidence):
    """Keep CAB byte observations distinct from installation-chain closure."""
    rows = [{"kind": "embedded_input_extraction_failure", "evidence": evidence, **failure}
            for failure in data.get("failures", [])]
    for item in data.get("inputs", []):
        extraction = item.get("extraction", {})
        if extraction.get("kind") != "bounded_cabinet_candidate":
            continue
        cabinet = extraction["cabinet"]
        layout = cabinet.get("layout", {})
        outers = cabinet.get("containing_compressed_blocks", [])
        scope = {"input": item["path"], "sha256": item["sha256"], "evidence": evidence}
        if not layout.get("complete") and outers:
            rows.append({"kind": "embedded_cabinet_header_inside_compressed_stream", **scope,
                "layout_failure": layout.get("failure"), "outer_cabinets": outers,
                "outer_extraction_verified_at_capture": all(
                    outer.get("outer_static_extraction", {}).get("current_complete") for outer in outers),
                "independent_cabinet_established": False,
                "scope": "Retained failed slice is not proof of a damaged official package; partial decompression outputs remain excluded and installation semantics remain pending"})
        elif not layout.get("complete"):
            rows.append({"kind": "embedded_cabinet_layout_unresolved", **scope,
                         "failure": layout.get("failure")})
        elif not extraction.get("static_extraction", {}).get("current_complete"):
            rows.append({"kind": "embedded_cabinet_extraction_pending", **scope,
                         "static_extraction": extraction.get("static_extraction", {})})
    rows.append({"kind": "embedded_installation_chain_semantics_pending", "evidence": evidence,
                 "scope": "Exact resource ranges and successful static container extraction do not establish installer branches, OS integration, native ABI or Rust/UI implementation"})
    return rows


def semantic_query_gaps(value, evidence, pointer="", product_id=None):
    """Preserve acquisition, analyzer and semantic findings as separate claims."""
    kinds = {"failures": "semantic_query_failure", "unresolved": "semantic_query_chain_unresolved",
             "unresolved_chains": "semantic_query_chain_unresolved",
             "remaining_chains": "semantic_query_chain_unresolved",
             "remaining_closure": "semantic_query_chain_unresolved",
             "remaining_local_chains": "semantic_query_chain_unresolved",
             "failed_acquisition": "acquisition_query_failure",
             "unresolved_acquisition": "acquisition_query_reference_unresolved",
             "capture_failures": "analyzer_query_failure"}
    rows = []
    if isinstance(value, dict):
        product_id = value.get("product_id", product_id)
        for key, child in value.items():
            location = pointer + "/" + key.replace("~", "~0").replace("/", "~1")
            if key in kinds and isinstance(child, list):
                for finding in child:
                    kind = kinds[key]
                    if (key == "failures" and isinstance(finding, dict)
                            and isinstance(finding.get("stage"), str)
                            and re.fullmatch(r"(?:[0-9a-f]+:)?hexrays", finding["stage"])
                            and isinstance(finding.get("error"), str) and finding["error"]):
                        kind = "analyzer_query_failure"
                    rows.append({"kind": kind, "evidence": evidence, "pointer": location,
                                 "product_id": product_id, "finding": finding})
            else:
                rows.extend(semantic_query_gaps(child, evidence, location, product_id))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            rows.extend(semantic_query_gaps(child, evidence, pointer + "/" + str(index), product_id))
    return rows


def semantic_result_gaps(data, evidence):
    rows = semantic_query_gaps(data, evidence)
    if evidence == ".analysis/results/host-current/local-audit.json":
        for query, report in data.items():
            if isinstance(report, dict) and report.get("source_chain_complete") is False:
                rows.append({"kind": "host_local_source_chain_pending", "evidence": evidence,
                             "pointer": "/" + query.replace("~", "~0").replace("/", "~1"),
                             "scope": "Retained complete owners do not establish every native target and consumer"})
        rows.append({"kind": "host_implementation_acceptance_pending", "evidence": evidence,
                     "scope": "Static host queries do not establish complete Rust/UI wiring, actual DLL comparisons or runtime acceptance"})
    else:
        rows.append({"kind": "page_implementation_acceptance_pending", "evidence": evidence,
                     "scope": "semantic queries are separate from each full UI/backend checklist and runtime acceptance"})
    return rows


def native_capture_gaps(data, receipt):
    resolved = native_reanalysis_resolution(receipt.parent, data)
    rows = [{"kind": "native_capture_failure", "evidence": rel(receipt), **row}
            for row in resolved["remaining_failures"]]
    rows.extend({"kind": "native_supplement_verification_failure", "evidence": rel(receipt), **row}
                for row in resolved["verification_failures"])
    if not data.get("capture_complete"):
        rows.append({"kind": "native_capture_incomplete", "evidence": rel(receipt)})
    if data.get("semantic_audit") != "complete":
        rows.append({"kind": "semantic_audit_pending", "scope": "ABI, selectors, transport, responses and lifecycle"})
    if data.get("implementation_acceptance") != "complete":
        rows.append({"kind": "implementation_acceptance_pending", "scope": "Rust protocol/service/UI chains"})
    return rows


def refresh_selected_gaps(owners):
    """Publish only explicitly selected semantic or native capture owners."""
    if not owners or len(owners) != len(set(owners)):
        raise ValueError("Selected gap owners must be a nonempty unique list")
    selected = []
    for name in owners:
        path = resolve(name, ".analysis/results")
        relative = rel(path)
        if relative != name:
            raise ValueError("Gap owner requires its canonical project-relative path")
        semantic = (path.parent == (ROOT / ".analysis/results/frontend").resolve()
                    and path.name.endswith("-audit.json")
                    and not path.name.endswith("-shared-page-layout-audit.json"))
        native = bool(re.fullmatch(r"\.analysis/results/native/[0-9a-f]{64}/capture\.json", relative))
        if not semantic and not native and relative != ".analysis/results/host-current/local-audit.json":
            raise ValueError("Selected gap owner is not a semantic audit or native capture: " + name)
        selected.append((path, native))

    def read_owner(item):
        path, native = item
        raw = path.read_bytes()
        data = json.loads(raw)
        if not isinstance(data, dict):
            raise ValueError("Gap owner must contain a result object: " + rel(path))
        rows = native_capture_gaps(data, path) if native else semantic_result_gaps(data, rel(path))
        return rel(path), rows, {"path": rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

    with concurrent.futures.ThreadPoolExecutor(max_workers=min(8, max(2, len(selected)))) as pool:
        updates = list(pool.map(read_owner, selected))
    path = ROOT / ".analysis/results/gaps.json"
    ledger = json.loads(path.read_bytes())
    for name, rows, source in updates:
        current = (ROOT / name).read_bytes()
        if len(current) != source["bytes"] or hashlib.sha256(current).hexdigest() != source["sha256"]:
            raise ValueError("Selected gap source changed during reconciliation: " + name)
    ledger["owners"].update({name: rows for name, rows, _ in updates})
    ledger["derived_owners"] = sorted(set(ledger.get("derived_owners", [])) | set(owners))
    ledger["updated_at_utc"] = stamp()
    ledger["selected_refresh"] = {"sources": [source for _, _, source in updates], "target_execution": False}
    save(path, ledger)
    return ledger


def typed_maintenance_gaps(registry):
    """Recompute maintenance availability without promoting projections to originals."""
    rows = []
    for recipe in registry["tables"]:
        scope = {"crate": recipe["crate"], "symbol": recipe["symbol"], "input": recipe["source"]}
        try:
            source = resolve(recipe["source"], ".analysis/inputs/typed-tables")
            current = sha(source)
        except FileNotFoundError:
            rows.append({"kind": "typed_maintenance_input_missing", **scope,
                         "expected_sha256": recipe["source_sha256"]})
            continue
        except (OSError, ValueError) as error:
            rows.append({"kind": "typed_maintenance_input_unverified", **scope,
                         "error": str(error).replace(str(ROOT), "<project>")})
            continue
        if current != recipe["source_sha256"]:
            rows.append({"kind": "typed_maintenance_input_identity_mismatch", **scope,
                         "expected_sha256": recipe["source_sha256"], "current_sha256": current})
        recovery = recipe.get("maintenance_recovery")
        if recovery and recovery.get("original_evidence_fields_restored") is not True:
            rows.append({"kind": "typed_maintenance_source_evidence_pending", **scope,
                         "maintenance_sha256": current, "recovery": recovery,
                         "excluded_evidence_fields": recipe.get("excluded_evidence_fields", []),
                         "scope": "Available maintenance bytes and successful regeneration establish storage equivalence; original acquisition, current version and full source semantics remain separate"})
    verification = ROOT / ".analysis/results/typed-data.json"
    report = json.loads(verification.read_bytes()).get("maintenance_inputs", {}) if verification.is_file() else {}
    current = False
    if report.get("passed") and report.get("inputs") and report.get("tables") == len(registry["tables"]):
        import rust_data
        try:
            current = report["inputs"] == rust_data.maintenance_verification_inputs(registry)
        except (OSError, ValueError):
            pass
    if not current:
        rows.append({"kind": "typed_maintenance_regeneration_unverified", "evidence": rel(verification),
                     "next_step": "python tools/workspace.py prepare-typed-data --check --record-verification",
                     "scope": "A recorded complete check must match all current maintenance, schema, generator and output bytes"})
    return rows


def host_recheck_gaps(scope, index):
    """Reconcile independent current host rechecks against retained originals."""
    from host import OFFICIAL, STABLE_DOWNLOAD_PAGE, STABLE_BOOTSTRAP
    stable_key, matches_key = {
        "bootstrap": ("bootstrap_stable", "matches_previous_bootstrap"),
        "metadata": ("deployment_metadata_stable", "matches_previous_metadata"),
    }[scope]
    name = f".analysis/inputs/host-current/{scope}-recheck.json"
    path = resolve(name, ".analysis/inputs/host-current")
    if not path.is_file():
        return [{"kind": f"host_{scope}_recheck_pending", "evidence": name}]
    indexed = {row["path"]: row for row in index["inputs"]}
    rows = []

    def finding(kind, **fields):
        rows.append({"kind": f"host_{scope}_" + kind, **fields, "evidence": name})

    def verified(claim):
        source = resolve(claim["path"], ".analysis/inputs")
        expected = indexed.get(claim["path"], {})
        if (any(claim.get(key) != expected.get(key) for key in ("bytes", "sha256"))
                or source.stat().st_size != claim["bytes"] or sha(source) != claim["sha256"]):
            raise ValueError("Host recheck original differs from its indexed byte identity")

    try:
        raw = path.read_bytes()
        verified({"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
        data = json.loads(raw)
        for failure in data.get("failures", []):
            finding("recheck_failure", failure=failure)
        if not data.get("finished_at_utc"):
            finding("recheck_incomplete")
        if data.get(stable_key) is not True:
            finding("deployment_changed_or_unverified")
        if data.get(matches_key) is False:
            finding("changed_since_acquisition", baseline=data.get("baseline"),
                    comparisons=data.get("comparisons", []))
        files = defaultdict(list)
        for artifact in data.get("files", []):
            try:
                verified(artifact)
                for url in (artifact["url"], artifact.get("final_url", artifact["url"])):
                    parsed = urllib.parse.urlsplit(url)
                    if parsed.scheme != "https" or parsed.netloc not in OFFICIAL:
                        raise ValueError("Host recheck original does not have an official HTTPS origin")
                if artifact.get("http_status", 200) != 200:
                    raise ValueError("Host recheck original does not have an HTTP 200 response")
                files[artifact["url"]].append(artifact)
            except (OSError, ValueError, KeyError, TypeError) as exc:
                finding("recheck_original_unverified", input=artifact.get("path"), error=str(exc))
        stability = {row["url"]: row for row in data.get("stability", [])}
        expected_urls = {STABLE_DOWNLOAD_PAGE, STABLE_BOOTSTRAP} if scope == "bootstrap" else set(files)
        if not expected_urls:
            finding("recheck_originals_absent")
        for url in sorted(expected_urls):
            samples, check = files.get(url, []), stability.get(url, {})
            if (len(samples) < 2 or check.get("unchanged") is not True
                    or check.get("before_sha256") != samples[0]["sha256"]
                    or check.get("after_sha256") != samples[-1]["sha256"]
                    or any(sample["sha256"] != samples[0]["sha256"] or sample["bytes"] != samples[0]["bytes"]
                           for sample in samples[1:])):
                finding("stability_proof_unverified", url=url, stability=check)
    except (OSError, ValueError, KeyError, TypeError) as exc:
        finding("recheck_receipt_unverified", error=str(exc))
    return rows


def _refresh_gaps():
    path = ROOT / ".analysis/results/gaps.json"
    ledger = json.loads(path.read_bytes()) if path.exists() else {"schema_version": 1, "owners": {}}
    derived = {}
    typed_registry = ROOT / "tools/typed-data.json"
    if typed_registry.is_file():
        derived["tools/typed-data.json"] = typed_maintenance_gaps(json.loads(typed_registry.read_bytes()))
    for receipt in sorted((ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json")):
        data = json.loads(receipt.read_bytes())
        rows = list(data.get("unresolved", []))
        rows.extend({"kind": "fetch_failure", **row} for row in data["files"] if "error" in row)
        # A successful analyzer process may still have no chunk enumeration.
        # Keep this separate from resource HTTP failures and semantic closure.
        chunk_dir = ROOT / ".analysis/results/frontend" / receipt.parent.name / "chunk-references"
        reported_urls = {row.get("url") for row in rows if row.get("kind") == "chunk_enumeration_unavailable"}
        for original in data["files"]:
            if "path" not in original or Path(urllib.parse.urlsplit(original["url"]).path).suffix.lower() not in {".js", ".mjs"}:
                continue
            identifier = hashlib.sha256(original["url"].encode()).hexdigest()[:24]
            report = chunk_dir / (identifier + ".json")
            try:
                enumeration = json.loads(report.read_bytes()).get("enumeration")
            except (OSError, ValueError):
                enumeration = None
            if enumeration is None and original["url"] not in reported_urls:
                rows.append({"kind": "chunk_enumeration_unavailable", "url": original["url"],
                             "input": original["path"], "evidence": rel(report),
                             "scope": "dynamic chunk/resource closure remains unverified"})
        if not data.get("source_current_during_acquisition"):
            rows.append({"kind": "deployment_stability_unverified", "evidence": rel(receipt)})
        derived[rel(receipt)] = rows
        # Acquisition and analysis have independent owners; fetching cannot erase analysis findings.
        ledger["owners"].pop(rel(receipt.parent), None)
    for receipt in sorted((ROOT / ".analysis/results/frontend").glob("*/analysis.json")):
        data = json.loads(receipt.read_bytes())
        rows = [{"kind": "analyzer_failure", **row} for row in data["commands"] if row["exit_code"]]
        for command in data["commands"]:
            if command["stage"] == "validate":
                try:
                    validation = json.loads((ROOT / command["stdout"]).read_bytes())
                    findings = validation["findings"]
                    if not isinstance(findings, list):
                        raise ValueError("Validation findings must be an array")
                except (OSError, ValueError, KeyError, TypeError) as error:
                    rows.append({"kind": "frontend_validation_report_unavailable",
                                 "evidence": command["stdout"], "error": str(error),
                                 "exit_code": command["exit_code"]})
                    continue
                rows.extend({"kind": "frontend_graph_finding", "evidence": command["stdout"],
                             "finding": row} for row in findings)
        rows.extend({"kind": "source_map_missing_content", **row} for row in data["source_maps"]
                    if row["embedded_sources"] < row["source_count"] or row["indexed_sections"])
        if data.get("semantic_audit") != "complete":
            rows.append({"kind": "semantic_audit_pending",
                         "scope": "mounted tree, product/accessory branches, JS/CSS states, actions, persistence and cleanup"})
        if data.get("implementation_acceptance") != "complete":
            rows.append({"kind": "implementation_acceptance_pending", "scope": "source-to-Rust/UI/service chains"})
        derived[rel(receipt)] = rows
    for receipt in sorted((ROOT / ".analysis/results/frontend").glob("*-shared-page-layout-audit.json")):
        data = json.loads(receipt.read_bytes())
        rows = []
        for product in data.get("products", []):
            scope = {"product_id": product["product_id"], "family": data["family"], "evidence": rel(receipt)}
            rows.extend({"kind": "shared_layout_query_failure", "failure": failure, **scope}
                        for failure in product.get("failures", []))
            rows.extend({"kind": "shared_layout_chain_unresolved", "chain": chain, **scope}
                        for chain in product.get("unresolved_chains", []))
        rows.append({"kind": "page_implementation_acceptance_pending", "evidence": rel(receipt),
                     "scope": "shared wrapper query is separate from each complete page, local action and backend checklist"})
        derived[rel(receipt)] = rows
    # Reusable semantic queries share family receipts. Keep their failures and
    # explicitly unresolved local chains independent of acquisition/capture
    # findings, including follow-up queries appended after the initial audit.
    for receipt in sorted((ROOT / ".analysis/results/frontend").glob("*-audit.json")):
        data = json.loads(receipt.read_bytes())
        shared_layout = receipt.name.endswith("-shared-page-layout-audit.json")
        rows = list(derived.get(rel(receipt), []))
        if shared_layout:
            data = {key: value for key, value in data.items() if key != "products"}

        rows.extend(semantic_query_gaps(data, rel(receipt)) if shared_layout else
                    semantic_result_gaps(data, rel(receipt)))
        derived[rel(receipt)] = rows
    captures = {}
    for receipt in sorted((ROOT / ".analysis/results/native").glob("*/capture.json")):
        data = json.loads(receipt.read_bytes())
        captures[data["input_sha256"]] = data
        derived[rel(receipt)] = native_capture_gaps(data, receipt)
    for receipt in sorted((ROOT / ".analysis/results/native").glob("*/exception-regions/*/capture.json")):
        data = json.loads(receipt.read_bytes())
        rows = [{"kind": "native_exception_region_decode_failure", "evidence": rel(receipt), **row}
                for row in data.get("failures", [])]
        if data.get("capture_complete") is not True:
            rows.append({"kind": "native_exception_region_export_incomplete", "evidence": rel(receipt)})
        coverage = data.get("exception_regions", {}).get("captured_chunk_coverage", {})
        rows.extend({"kind": "native_exception_region_missing_from_base_chunks", "evidence": rel(receipt),
                     "region": row["begin_rva"], "uncovered": row["uncovered"]}
                    for row in coverage.get("regions", []) if row["uncovered"])
        rows.append({"kind": "native_exception_region_semantics_pending", "evidence": rel(receipt),
                     "scope": "Exception fragments require control-flow ownership, unwind and ABI review; static byte decoding does not complete function discovery or Rust behavior"})
        derived[rel(receipt)] = rows
    embedded = ROOT / ".analysis/results/native-packages/embedded-inputs.json"
    chain = ROOT / ".analysis/results/native/chain-audit.json"
    if chain.is_file():
        data = json.loads(chain.read_bytes())
        derived[rel(chain)] = semantic_query_gaps(data, rel(chain))
    if embedded.is_file():
        derived[rel(embedded)] = embedded_input_gaps(json.loads(embedded.read_bytes()), rel(embedded))
    validation = ROOT / ".analysis/results/native-packages/source-validation.json"
    if validation.is_file():
        data = json.loads(validation.read_bytes())
        rows = [{"kind": "partial_static_extraction_source_excluded", "input": item,
                 "evidence": rel(validation),
                 "scope": "Retained diagnostic bytes must not become implementation evidence"}
                for item in data.get("rejected_inputs", [])]
        rows.extend({"kind": "native_extraction_source_validation_failure", **failure,
                     "evidence": rel(validation)} for failure in data.get("failures", []))
        derived[rel(validation)] = rows
    corpus = ROOT / ".analysis/results/corpus.json"
    official = ROOT / ".analysis/inputs/official-native-current/acquisition.json"
    current_identities = set()
    if official.is_file():
        acquisition = json.loads(official.read_bytes())
        rows = [{"kind": "manifest_acquisition_failure", **row}
                for row in acquisition["manifests"] if "error" in row]
        rows.extend(acquisition.get("host_app_source_failures", []))
        stable_urls = {row["url"] for row in acquisition.get("stability", []) if row["unchanged"]}
        for resource in acquisition["resources"]:
            if resource["status"] != "verified":
                rows.append({"kind": "native_resource_acquisition_failure", **resource})
            elif any(parent["url"] in stable_urls for parent in resource["manifests"]):
                current_identities.add(resource["sha256"])
                if Path(resource["path"]).suffix.lower() == ".exe":
                    rows.append({"kind": "packed_resource_or_helper_analysis_pending", "path": resource["path"],
                                 "sha256": resource["sha256"], "manifests": resource["manifests"]})
        rows.extend({"kind": "manifest_deployment_changed_or_unverified", **row}
                    for row in acquisition.get("stability", []) if not row["unchanged"])
        rows.append({"kind": "global_product_catalog_closure_pending", "evidence": rel(official)})
        derived[rel(official)] = rows
    if corpus.is_file():
        data = json.loads(corpus.read_bytes())
        lineage = data.get("current_version_lineage", {})
        rows = [{**failure, "evidence": rel(corpus)} for failure in lineage.get("failures", [])]
        rows.extend({**failure, "evidence": rel(corpus)} for failure in data.get("failures", []))
        rows.extend(cached_download_fragment_gaps(data.get("cached_download_fragments", {}), rel(corpus)))
        if lineage and lineage.get("snapshot_unchanged") is not True:
            rows.append({"kind": "current_version_lineage_snapshot_changed", "evidence": rel(corpus),
                         "inputs": lineage.get("changed_during_audit", [])})
        if data.get("input_index", {}).get("unchanged") is not True or data.get("input_index", {}).get("sha256_at_start") != sha(ROOT / ".analysis/inputs.json"):
            rows.append({"kind": "corpus_input_index_stale", "evidence": rel(corpus),
                         "next_step": "python tools/workspace.py inspect-corpus --no-refresh-gaps"})
        for kind in ("native_inputs", "managed_inputs", "mixed_inputs", "other_inputs", "unclassified_inputs"):
            for item in data.get(kind, []):
                proof = item.get("current_version_provenance", {})
                if (lineage and proof.get("status") != "proven_current_resource_bytes") or (not lineage and item["sha256"] not in current_identities):
                    rows.append({"kind": "current_native_version_audit_pending", "input_kind": kind,
                                 "sha256": item["sha256"], "aliases": item.get("verified_aliases", []),
                                 "provenance": proof, "evidence": rel(corpus)})
        for native in data.get("native_inputs", []) + data.get("mixed_inputs", []):
            if native["sha256"] not in captures:
                rows.append({"kind": "native_capture_pending", "sha256": native["sha256"],
                             "aliases": native.get("verified_aliases", []), "evidence": rel(corpus)})
        derived[rel(corpus)] = rows
    native_queue = ROOT / ".analysis/results/native-corpus-capture.json"
    if native_queue.is_file():
        data = json.loads(native_queue.read_bytes())
        rows = [{"kind": "native_corpus_capture_failed", "evidence": rel(native_queue), **row}
                for row in data.get("modules", []) if row.get("status") == "failed"]
        if not data.get("finished_at_utc") or len(data.get("modules", [])) != data.get("module_count"):
            rows.append({"kind": "native_corpus_capture_incomplete", "evidence": rel(native_queue),
                         "modules_reported": len(data.get("modules", [])), "modules_selected": data.get("module_count")})
        try:
            current_digest = sha(resolve(data["acquisition"], ".analysis"))
            if current_digest != data.get("acquisition_sha256") or data.get("snapshot_unchanged") is False:
                rows.append({"kind": "native_corpus_capture_inventory_stale", "evidence": rel(native_queue),
                             "acquisition": data["acquisition"], "expected_sha256": data.get("acquisition_sha256"),
                             "current_sha256": current_digest})
        except (OSError, ValueError, KeyError) as exc:
            rows.append({"kind": "native_corpus_capture_inventory_unverified", "evidence": rel(native_queue),
                         "error": str(exc).replace(str(ROOT), "<project>")})
        if data.get("original_inputs_snapshot_unchanged") is False:
            rows.append({"kind": "native_corpus_capture_original_snapshot_changed", "evidence": rel(native_queue),
                         "inputs": data.get("original_inputs_changed_during_capture", [])})
        if data.get("pending_inputs") or data.get("full_corpus_captured") is not True:
            rows.append({"kind": "native_corpus_capture_scope_pending", "evidence": rel(native_queue),
                         "pending_inputs": data.get("pending_inputs", []),
                         "scope": "Each current corpus input still requires a verified complete capture; a finished subset is separate"})
        derived[rel(native_queue)] = rows
    index = json.loads((ROOT / ".analysis/inputs.json").read_bytes())
    derived[".analysis/inputs.json"] = [
        {"kind": "current_source_missing", "source_root": root} for root in index["missing_current_roots"]]
    for name, source in index.get("current_sources", {}).items():
        if source.get("status") in {
            "official_packages_acquired_full_dependency_closure_pending",
            "partial_current_source_sets_acquired_full_catalog_closure_pending",
        }:
            derived[".analysis/inputs.json"].append({
                "kind": "current_source_set_closure_pending", "source": name, **source})
    for receipt in sorted((ROOT / ".analysis/results/families").glob("*.json")):
        matrix = json.loads(receipt.read_bytes())
        if "layer" not in matrix:
            continue  # Native package extraction has its own capture receipts.
        rows = []
        for product in matrix.get("products", []):
            scope = {"product_id": product["product_id"], "family": matrix["family"],
                     "layer": matrix["layer"], "evidence": rel(receipt)}
            if not product.get("probe", {}).get("entry_available"):
                rows.append({"kind": "product_entry_unverified", **scope,
                             "probe": product.get("probe")})
            if not product.get("acquisition", {}).get("completed"):
                rows.append({"kind": "product_resource_closure_pending", **scope,
                             "input": product["input"]})
            if not product.get("analysis", {}).get("completed"):
                rows.append({"kind": "product_static_recovery_incomplete", **scope})
            if product.get("semantic_audit") != "complete":
                rows.append({"kind": "semantic_audit_pending", **scope,
                             "scope": "complete mounted branches, local callers, responses and lifecycle"})
            if product.get("implementation_acceptance") != "complete":
                rows.append({"kind": "implementation_acceptance_pending", **scope,
                             "scope": "complete source-to-Rust/UI/backend checklist"})
        derived[rel(receipt)] = rows
    audit_path = ROOT / ".analysis/results/input-audit.json"
    if audit_path.is_file():
        audit = json.loads(audit_path.read_bytes())
        rows = [{"kind": "indexed_file_" + row["status"], **row, "evidence": rel(audit_path)}
                for row in audit["entries"] if row["status"] != "verified"]
        rows.extend({"kind": "input_index_error", **row} for row in audit["index_errors"])
        if audit["index"].get("sha256") != sha(ROOT / ".analysis/inputs.json"):
            rows.append({"kind": "input_audit_stale", "evidence": rel(audit_path),
                         "next_step": "python tools/workspace.py audit-inputs"})
        derived[rel(audit_path)] = rows
    resource_audit = ROOT / ".analysis/results/resource-audit.json"
    if resource_audit.is_file():
        import workspace
        audit = json.loads(resource_audit.read_bytes())
        # verify-resources and audit-runtime-resources maintain independent
        # sections of this receipt; either may be absent.
        rows = [{"kind": "runtime_image_verification_failure", "evidence": rel(resource_audit), **entry}
                for entry in audit.get("entries", []) if not entry["passed"]]
        try:
            current = ("entries" in audit and audit.get("inputs_unchanged")
                       and audit["inputs"] == workspace.image_resource_inputs())
        except (OSError, ValueError) as error:
            current = False
            rows.append({"kind": "runtime_image_include_closure_failure", "evidence": rel(resource_audit),
                         "error": type(error).__name__})
        if not current:
            rows.append({"kind": "resource_audit_stale", "evidence": rel(resource_audit),
                         "next_step": "python tools/workspace.py verify-resources"})
        closure = audit.get("runtime_reference_closure")
        if closure is not None:
            rows.extend({"kind": "runtime_resource_reference_missing", "evidence": rel(resource_audit), **row}
                        for row in closure.get("missing_references", []))
            if not closure.get("passed"):
                rows.append({"kind": "runtime_resource_closure_failed", "evidence": rel(resource_audit),
                             "next_step": "python tools/workspace.py audit-runtime-resources"})
        derived[rel(resource_audit)] = rows
    host = ROOT / ".analysis/inputs/host-current/acquisition.json"
    if host.is_file():
        acquisition = json.loads(host.read_bytes())
        rows = [{"kind": "host_acquisition_failure", **row} for row in acquisition.get("failures", [])]
        rows.extend({"kind": "host_deployment_changed_or_unverified", **row}
                    for row in acquisition.get("stability", []) if not row["unchanged"])
        if not acquisition.get("host_package_closure_complete"):
            rows.append({"kind": "host_package_closure_pending", "evidence": rel(host),
                         "scope": "complete host JS/native packages, dependencies, installed local lifecycle"})
        derived[rel(host)] = rows
        for scope in ("bootstrap", "metadata"):
            derived[f".analysis/inputs/host-current/{scope}-recheck.json"] = host_recheck_gaps(scope, index)
    managed = ROOT / ".analysis/results/host-current/managed/capture.json"
    if managed.is_file():
        data = json.loads(managed.read_bytes())
        rows = []
        for assembly in data.get("assemblies", []):
            rows.extend({"kind": "managed_analyzer_failure", "input": assembly["input"], **command}
                        for command in assembly.get("commands", []) if command["exit_code"])
            rows.extend({**diagnostic, "input": assembly["input"], "evidence": rel(managed)}
                        for diagnostic in assembly.get("diagnostics", []))
            if not assembly.get("input_bytes_unchanged"):
                rows.append({"kind": "managed_input_identity_unverified", "input": assembly["input"]})
        rows.append({"kind": "semantic_audit_pending",
                     "scope": "managed local installation, lifecycle, native dependencies and Rust adapters"})
        derived[rel(managed)] = rows
    host_js = ROOT / ".analysis/results/host-current/javascript/capture.json"
    if host_js.is_file():
        data = json.loads(host_js.read_bytes())
        rows = list(data.get("unresolved", []))
        for command in data.get("commands", []):
            if command["exit_code"]:
                rows.append({"kind": "host_javascript_analyzer_failure", **command})
            if command["stage"] == "validate":
                validation = json.loads((ROOT / command["stdout"]).read_bytes())
                rows.extend({"kind": "host_javascript_graph_finding", "evidence": command["stdout"],
                             "finding": finding} for finding in validation.get("findings", []))
        if not data.get("input_bytes_unchanged"):
            rows.append({"kind": "host_javascript_input_identity_unverified"})
        rows.append({"kind": "semantic_audit_pending",
                     "scope": "host IPC, installed native dependencies, local storage, lifecycle and Rust adapters"})
        derived[rel(host_js)] = rows
    host_local = ROOT / ".analysis/results/host-current/local-audit.json"
    if host_local.is_file():
        data = json.loads(host_local.read_bytes())
        derived[rel(host_local)] = semantic_result_gaps(data, rel(host_local))
    for owner in ledger.get("derived_owners", []):
        ledger["owners"].pop(owner, None)
    ledger["owners"].update(derived)
    ledger["derived_owners"] = sorted(derived)
    ledger["updated_at_utc"] = stamp()
    save(path, ledger)
    return ledger


def prepare_ida_python():
    """Restore only byte-exact members of the pinned official analyzer runtime."""
    import io
    import zipfile

    directory = ROOT / "tools/analyzers/python-3.11.9"
    receipt_path = directory / "acquisition.json"
    receipt = json.loads(receipt_path.read_bytes())
    expected = {item["path"]: item for item in receipt["files"]}
    if (not expected or len(expected) != len(receipt["files"])
            or any(Path(name).name != name or name in {".", ".."} for name in expected)):
        raise ValueError("IDA Python runtime receipt contains invalid or duplicate member names")
    with index_lock(directory / "restore.lock"):
        missing = [name for name, item in expected.items()
                   if not (directory / name).is_file() or (directory / name).stat().st_size != item["bytes"]
                   or sha(directory / name) != item["sha256"]]
        if missing:
            parsed = urllib.parse.urlparse(receipt["url"])
            if parsed.scheme != "https" or parsed.hostname != "www.python.org":
                raise ValueError("IDA Python runtime requires its pinned official HTTPS source")
            cache = ROOT / ".work/analyzers/python-3.11.9-embed-amd64.zip"
            if cache.is_file() and cache.stat().st_size == receipt["bytes"] and sha(cache) == receipt["sha256"]:
                raw = cache.read_bytes()
            else:
                with urllib.request.urlopen(receipt["url"], timeout=60) as response:
                    raw = response.read(receipt["bytes"] + 1)
                    final = urllib.parse.urlparse(response.url)
                if final.scheme != "https" or final.hostname != "www.python.org":
                    raise ValueError("IDA Python runtime redirected outside its official source")
            if len(raw) != receipt["bytes"] or hashlib.sha256(raw).hexdigest() != receipt["sha256"]:
                raise ValueError("IDA Python runtime archive differs from pinned acquisition bytes")
            cache.parent.mkdir(parents=True, exist_ok=True)
            if not cache.is_file():
                cache.write_bytes(raw)
            with zipfile.ZipFile(io.BytesIO(raw)) as archive:
                names = archive.namelist()
                archive_name = Path(parsed.path).name
                archive_entry = expected.get(archive_name)
                retained_archive = (archive_entry is not None and archive_entry["sha256"] == receipt["sha256"]
                                    and archive_entry["bytes"] == receipt["bytes"])
                members = set(expected) - ({archive_name} if retained_archive else set())
                if len(names) != len(set(names)) or not members.issubset(names):
                    raise ValueError("IDA Python runtime archive member set differs: " + str(sorted(members - set(names))))
                payloads = {}
                for name, item in expected.items():
                    payload = raw if retained_archive and name == archive_name else archive.read(name)
                    if len(payload) != item["bytes"] or hashlib.sha256(payload).hexdigest() != item["sha256"]:
                        raise ValueError("IDA Python runtime member differs: " + name)
                    payloads[name] = payload
            for name in missing:
                pending = directory / (name + ".restoring-" + uuid.uuid4().hex)
                try:
                    pending.write_bytes(payloads[name])
                    pending.replace(directory / name)
                finally:
                    pending.unlink(missing_ok=True)
        return {"receipt": rel(receipt_path), "archive_sha256": receipt["sha256"],
                "verified_members": len(expected), "restored_members": len(missing)}


def configure_ida():
    runtime = prepare_ida_python()
    package = ROOT / "tools/ida/ida-pro-mcp-2.0/src/ida_pro_mcp"
    plugin = ROOT / "tools/ida/idausr/plugins/ida_mcp.py"
    plugin.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(package / "ida_mcp.py", plugin)
    shutil.copytree(package / "ida_mcp", plugin.with_suffix(""), dirs_exist_ok=True)
    config = {"mcpServers": {"ida-pro-mcp": {"url": "http://127.0.0.1:13337/mcp"}}}
    save(ROOT / "tools/ida/mcp.json", config)
    print(json.dumps({"configuration": "tools/ida/mcp.json", "version": "2.0.0",
                      "plugin_sha256": sha(plugin), "python_runtime": runtime}), flush=True)


def probe_ida(args):
    """Verify the local MCP endpoint against original bytes, using static reads only."""
    source = resolve(args.input, ".analysis/inputs")
    indexed = json.loads((ROOT / ".analysis/inputs.json").read_bytes())
    source_relative = rel(source)
    row = next((r for r in indexed["inputs"] if r["path"] == source_relative), None)
    if row is None or not source.is_file() or sha(source) != row["sha256"] or source.stat().st_size != row["bytes"]:
        raise ValueError("MCP probe requires an indexed, byte-verified original input")
    endpoint = "http://127.0.0.1:13337/mcp"
    destination = ROOT / ".analysis/results/ida-mcp-connection.json"
    receipt = {"schema_version": 1, "started_at_utc": stamp(), "endpoint": endpoint,
               "input": rel(source), "input_sha256": row["sha256"], "input_bytes": row["bytes"],
               "plugin_package_version": "2.0.0", "target_execution": False,
               "real_device_operations": False, "calls": [], "failures": [], "passed": False}

    def portable(value):
        if isinstance(value, dict):
            return {key: portable(item) for key, item in value.items()}
        if isinstance(value, list):
            return [portable(item) for item in value]
        if isinstance(value, str):
            return value.replace(str(ROOT) + "\\", "").replace(ROOT.as_posix() + "/", "")
        return value

    def request(method, params, notification=False):
        payload = {"jsonrpc": "2.0", "method": method, "params": params}
        if not notification:
            payload["id"] = len(receipt["calls"]) + 1
        headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}
        req = urllib.request.Request(endpoint, data=json.dumps(payload).encode(), headers=headers)
        with urllib.request.urlopen(req, timeout=60) as response:
            raw = response.read().decode("utf-8")
        result = None
        if raw.strip():
            if raw.lstrip().startswith("data:") or raw.lstrip().startswith("event:"):
                raw = "\n".join(line[5:].lstrip() for line in raw.splitlines() if line.startswith("data:"))
            message = json.loads(raw)
            if message.get("error"):
                raise ValueError(str(message["error"]))
            result = message.get("result")
        if method == "resources/read" and result is not None:
            result = [{"uri": item["uri"], "data": json.loads(item["text"])} for item in result["contents"]]
        elif method == "tools/call" and result is not None:
            if result.get("isError"):
                raise ValueError(str(result))
            result = result.get("structuredContent") or json.loads(result["content"][0]["text"])
        receipt["calls"].append({"method": method, "params": params, "result": portable(result)})
        save(destination, receipt)
        return result

    def tool(name, arguments):
        return request("tools/call", {"name": name, "arguments": arguments})

    try:
        receipt["initialize"] = request("initialize", {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "razer-static-analysis", "version": "1"}})
        request("notifications/initialized", {}, notification=True)
        health = tool("server_health", {})
        private = Path(health["input_path"]).resolve()
        if not private.is_relative_to((ROOT / ".work/native").resolve()) or sha(private) != row["sha256"]:
            raise ValueError("MCP database is not the verified private analysis input")
        if not health["auto_analysis_ready"] or not health["hexrays_ready"]:
            raise ValueError("IDA autoanalysis or Hex-Rays is not ready")
        metadata = request("resources/read", {"uri": "ida://idb/metadata"})[0]["data"]
        if metadata["sha256"] != row["sha256"] or int(metadata["filesize"], 16) != row["bytes"]:
            raise ValueError("MCP metadata differs from original input bytes")
        lookup = tool("lookup_funcs", {"queries": [args.function]})
        found = lookup["result"][0] if isinstance(lookup, dict) else lookup[0]
        if found.get("error") or not found.get("fn"):
            raise ValueError("MCP function lookup failed: " + str(found))
        function = found["fn"]
        receipt["function"] = {**function, "rva": hex(int(function["addr"], 16) - int(metadata["base"], 16))}
        for name, size_key in (("decompile", "max_lines"), ("disasm", "max_instructions")):
            offset = 0
            for _ in range(10000):
                page = tool(name, {"addr": function["addr"], size_key: 5000, "offset": offset})
                if page.get("error") or not page.get("code" if name == "decompile" else "asm"):
                    raise ValueError(name + " failed: " + str(page))
                cursor = page.get("cursor", {})
                if cursor.get("cancelled"):
                    raise ValueError(name + " was cancelled")
                if cursor.get("done"):
                    break
                following = cursor.get("next")
                if not isinstance(following, int) or following <= offset:
                    raise ValueError(name + " did not provide a progressing continuation cursor")
                offset = following
            else:
                raise ValueError(name + " exceeded the pagination limit; body is incomplete")
        receipt["passed"] = True
    except Exception as exc:
        receipt["failures"].append({"error": str(exc)})
        raise
    finally:
        receipt["finished_at_utc"] = stamp()
        save(destination, portable(receipt))
        print(json.dumps({"receipt": rel(destination), "passed": receipt["passed"]}), flush=True)


def native_database_resume(private):
    """Select an existing packed or recoverable unpacked private IDA database."""
    packed = private.with_name(private.name + ".i64")
    if packed.is_file() and packed.stat().st_size:
        files, selected, kind = [packed], packed, "packed_database"
    else:
        components = [private.with_name(private.name + extension) for extension in (".id0", ".id1", ".id2", ".nam", ".til")]
        if not all(path.is_file() and path.stat().st_size for path in components[:2]):
            return None
        files, selected, kind = [path for path in components if path.is_file()], private, "recover_unpacked_database"
    return {"kind": kind, "input": rel(selected),
            "files_before_resume": [{"path": rel(path), "bytes": path.stat().st_size, "sha256_at_resume": sha(path)} for path in files],
            "validation": "IDA must additionally verify its stored original-input SHA before exporting"}


def reject_native_database(private):
    """Retain a diagnosed invalid scratch database before a clean analysis."""
    scratch = (ROOT / ".work/native").resolve()
    private = private.resolve()
    if not private.is_relative_to(scratch):
        raise ValueError("Invalid database is outside private native analysis scratch")
    records = []
    for extension in (".i64", ".id0", ".id1", ".id2", ".nam", ".til"):
        source = private.with_name(private.name + extension)
        if not source.is_file():
            continue
        if not source.resolve().is_relative_to(scratch):
            raise ValueError("Invalid database component escaped private analysis scratch")
        digest = sha(source)
        destination = private.parent / "invalid-database" / digest / source.name
        if not destination.resolve().is_relative_to(scratch):
            raise ValueError("Invalid database retention escaped private analysis scratch")
        destination.parent.mkdir(parents=True, exist_ok=True)
        record = {"path_at_failure": rel(source), "path": rel(destination),
                  "bytes": source.stat().st_size, "sha256": digest}
        if destination.exists():
            if destination.stat().st_size != record["bytes"] or sha(destination) != digest:
                raise ValueError("Retained invalid database component has different bytes")
            source.unlink()
        else:
            source.replace(destination)
        records.append(record)
    return records


def capture_native(args):
    from host import inspect_pe

    source = resolve(args.input, ".analysis/inputs")
    indexed = json.loads((ROOT / ".analysis/inputs.json").read_bytes())
    source_relative = rel(source)
    row = next((r for r in indexed["inputs"] if r["path"] == source_relative), None)
    identity = sha(source)
    if row is None or identity != row["sha256"] or source.stat().st_size != row["bytes"]:
        raise ValueError("Native input not verified by input index")
    if row.get("container_extraction_complete") is False or row.get("source_bytes_verified") is False:
        raise ValueError("Native input comes from unverified or failed static decompression")
    pe = inspect_pe(source.read_bytes())
    if pe["kind"] not in {"native", "mixed_managed_native"}:
        raise ValueError("Indexed input is not a native or mixed native PE image")
    if getattr(args, "exception_regions_only", False):
        if args.resume or args.mcp_only:
            raise ValueError("Read-only exception-region export uses a new private database copy; do not combine it with resume or MCP")
        return capture_native_exception_regions(args, source, row, pe)
    loader_options = []
    loader_selection = {"pe_kind": pe["kind"], "architecture": pe.get("architecture"),
                        "image_base": pe.get("image_base"), "selection": "automatic"}
    if pe["kind"] == "mixed_managed_native":
        processors = {"x86": "80386", "x64": "AMD64"}
        processor = processors.get(pe.get("architecture"))
        if processor is None:
            raise ValueError("Mixed CLR/native PE requires an explicit supported native loader architecture")
        loader_options = ["-TPortable executable for " + processor + " (PE)", "-pmetapc"]
        loader_selection.update({"selection": "native PE view of mixed CLR/native input",
                                 "arguments": loader_options,
                                 "managed_view": ".analysis/results/host-current/managed/" + identity[:12]})
    output = ROOT / ".analysis/results/native" / identity
    if output.exists() and any(output.iterdir()) and not args.resume and not args.mcp_only:
        raise ValueError("Native result already exists; inspect capture.json before regenerating")
    previous = {}
    if args.resume:
        previous = json.loads((output / "plan.json").read_bytes())
        if previous["sha256"] != identity:
            raise ValueError("Resume input identity changed")
    prior_capture = output / "capture.json"
    if loader_options and prior_capture.is_file():
        previous_capture = json.loads(prior_capture.read_bytes())
        if previous_capture.get("input_sha256") != identity:
            raise ValueError("Existing mixed native capture has different input bytes")
        # IDA's default mixed-mode choice can expose CIL token addresses instead
        # of native PE RVAs. Retain that complete alternate view and its failures
        # before capturing the required native code with the PE loader.
        if any(int(function["address"], 16) < int(pe["image_base"], 16)
               for function in previous_capture.get("functions", [])):
            alternate = (output / "cil-view").resolve()
            if not alternate.is_relative_to(output.resolve()):
                raise ValueError("Alternate loader output escaped native result")
            records = previous_capture["functions"] + previous_capture.get("outputs", [])
            records += [{"path": name, "sha256": sha(output / name)} for name in
                        ("capture.json", "plan.json", "batch-session.json") if (output / name).is_file()]
            for record in records:
                original, retained = (output / record["path"]).resolve(), (alternate / record["path"]).resolve()
                if not original.is_relative_to(output.resolve()) or not retained.is_relative_to(alternate):
                    raise ValueError("Mixed loader artifact escaped its native result")
                if not original.is_file() or sha(original) != record["sha256"]:
                    raise ValueError("Mixed loader artifact changed: " + rel(original))
                retained.parent.mkdir(parents=True, exist_ok=True)
                if retained.exists() and sha(retained) != record["sha256"]:
                    raise ValueError("Retained CIL loader artifact changed: " + rel(retained))
                shutil.copyfile(original, retained)
                if sha(retained) != record["sha256"]:
                    raise ValueError("Retained CIL loader copy differs")
            for record in records:
                (output / record["path"]).unlink()
    alternate_capture = output / "cil-view/capture.json"
    if alternate_capture.is_file():
        loader_selection["retained_cil_capture"] = {"path": rel(alternate_capture), "sha256": sha(alternate_capture)}
    scratch = ROOT / ".work/native" / identity / ("mcp" if args.mcp_only else "batch")
    scratch.mkdir(parents=True, exist_ok=True)
    private = scratch / source.name
    if private.exists():
        if sha(private) != identity:
            raise ValueError("Existing private analysis copy differs from verified input")
    else:
        shutil.copyfile(source, private)
    if sha(private) != identity:
        raise ValueError("Private analysis copy differs")
    output.mkdir(parents=True, exist_ok=True)
    failed_capture_raw = prior_capture.read_bytes() if prior_capture.is_file() else None
    failed_integrity = (json.loads(failed_capture_raw).get("database_integrity", {}).get("verified") is False
                        if failed_capture_raw else False)
    rebuild = None
    if args.resume and not args.mcp_only and failed_integrity:
        rebuild = {"reason": "Prior resumed database failed read-only integrity validation; rebuild from the verified original",
                   "failed_capture": {"path": rel(prior_capture), "bytes": len(failed_capture_raw),
                                      "sha256": hashlib.sha256(failed_capture_raw).hexdigest()},
                   "retained_invalid_database": reject_native_database(private)}
    database_resume = (native_database_resume(private) if args.resume and not args.mcp_only and not rebuild
                       and previous.get("loader") == loader_selection else None)
    plan = {"input": rel(source), "sha256": identity, "output": rel(output), "loader": loader_selection,
            "database_resume": database_resume, "database_rebuild": rebuild,
            "acquisition_binding": row, "symbol_policy": "PDB plugin disabled; only retained original image bytes are analyzed"}
    save(output / "plan.json", plan)
    if not getattr(args, "ida_configuration_prepared", False):
        configure_ida()
    names = ("ida64.exe", "ida.exe") if args.mcp_only else ("idat64.exe", "idat.exe")
    executable = next((Path(args.ida_root) / name for name in names
                       if (Path(args.ida_root) / name).is_file()), None)
    if executable is None:
        raise ValueError(f"IDA not found in {args.ida_root}; expected {', '.join(names)}")
    env = os.environ.copy()
    env["IDAUSR"] = str(ROOT / "tools/ida/idausr")
    if args.mcp_only:
        env.pop("RAZER_IDA_CAPTURE_PLAN", None)
    else:
        env["RAZER_IDA_CAPTURE_PLAN"] = str(output / "plan.json")
    create_options = [] if database_resume else ["-c"]
    analysis_input = ROOT / database_resume["input"] if database_resume else private
    command = [str(executable), *create_options, "-A", "-Opdb:off", *loader_options, "-L"+str(scratch / "ida.log"),
               "-S"+str(ROOT / "tools/ida/ida-start-mcp.py"),
               str(analysis_input)]
    options = {}
    if os.name == "nt":
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        options["startupinfo"] = startup
        if not args.mcp_only:
            options["creationflags"] = subprocess.CREATE_NO_WINDOW
    session_path = output / ("mcp-session.json" if args.mcp_only else "batch-session.json")
    previous_attempts = []
    if session_path.is_file():
        prior_session = json.loads(session_path.read_bytes())
        previous_attempts = prior_session.pop("previous_attempts", [])
        if prior_session.get("passed") is False:
            prior_session["plan_before_resume"] = previous
            if rebuild:
                prior_session["failed_database_after_attempt"] = rebuild
                prior_session["failed_capture_bytes"] = {**rebuild["failed_capture"],
                    "raw_base64": base64.b64encode(failed_capture_raw).decode()}
            prior_session["retained_logs"] = []
            for log_path in (scratch / "ida.log", scratch / "process.log"):
                if log_path.is_file():
                    raw = log_path.read_bytes()
                    prior_session["retained_logs"].append({"path_at_attempt": rel(log_path), "bytes": len(raw),
                        "sha256": hashlib.sha256(raw).hexdigest(), "raw_base64": base64.b64encode(raw).decode()})
            previous_attempts.append(prior_session)
    with (scratch / "process.log").open("wb") as log:
        proc = subprocess.Popen(command, cwd=scratch, env=env, stdout=log,
                                stderr=subprocess.STDOUT, **options)
    recorded_command = [executable.name, *create_options, "-A", "-Opdb:off", *loader_options, "-L" + rel(scratch / "ida.log"),
                        "-S" + rel(ROOT / "tools/ida/ida-start-mcp.py"), rel(analysis_input)]
    session = {"pid": proc.pid, "started_at_utc": stamp(), "input": rel(private),
               "analyzer": executable.name, "analyzer_sha256": sha(executable),
               "analyzer_bytes": executable.stat().st_size,
               "argv": recorded_command, "mode": "mcp" if args.mcp_only else "batch",
               "loader": loader_selection, "symbol_policy": plan["symbol_policy"],
               "database_resume": database_resume, "previous_attempts": previous_attempts,
               "target_execution": False, "real_device_operations": False}
    save(session_path, session)
    print(json.dumps({"pid": proc.pid, "result": rel(output),
                      "log": rel(scratch / "ida.log")}), flush=True)
    if args.mcp_only:
        return session
    session["exit_code"] = proc.wait()
    session["finished_at_utc"] = stamp()
    manifest = output / "capture.json"
    session["capture_complete"] = manifest.is_file() and json.loads(manifest.read_bytes()).get("capture_complete", False)
    session["passed"] = session["exit_code"] == 0 and session["capture_complete"]
    save(session_path, session)
    if not getattr(args, "no_refresh_gaps", False):
        refresh_gaps()
    print(json.dumps({"session": rel(session_path), "passed": session["passed"], "exit_code": session["exit_code"]}), flush=True)
    return session


def capture_native_exception_regions(args, source, original, pe):
    """Export audited IA64 exception regions from a new private base-IDB copy."""
    from host import IA64_EXCEPTION_ANALYZER_FILES, IA64_EXCEPTION_SDK_MODULES, native_exception_region_coverage

    directory = pe.get("exception_directory", {})
    if pe.get("architecture") != "ia64" or directory.get("validated") is not True or not directory.get("regions"):
        raise ValueError("Exception-region export requires a complete validated nonempty IA64 PE exception directory")
    digest = original["sha256"]
    result = ROOT / ".analysis/results/native" / digest
    base_capture = result / "capture.json"
    capture = json.loads(base_capture.read_bytes())
    if capture.get("capture_complete") is not True or capture.get("input_sha256") != digest:
        raise ValueError("Exception-region export requires a complete matching base capture")
    functions, function_sources = [], []
    for record in capture["functions"]:
        path = resolve(rel(result / record["path"]), ".analysis/results/native")
        if sha(path) != record["sha256"]:
            raise ValueError("Exception coverage function differs from its base capture")
        functions.append(json.loads(path.read_bytes()))
        function_sources.append({"path": rel(path), "sha256": record["sha256"]})
    source_database = ROOT / ".work/native" / digest / "batch" / (source.name + ".i64")
    if not source_database.is_file():
        raise ValueError("Exception-region export has no retained base IDB")
    def identity(path):
        return {"path": rel(path), "bytes": path.stat().st_size, "sha256": sha(path)}
    ida_root = Path(args.ida_root).resolve()
    executable = next((ida_root / name for name in ("idat64.exe", "idat.exe")
                       if (ida_root / name).is_file()), None)
    if executable is None:
        raise ValueError("Static IDA executable is unavailable")

    def retain_tool(path, *, installed=False):
        path = path.resolve()
        if installed and not path.is_relative_to(ida_root):
            raise ValueError("Exception-region analyzer tool escaped its installation")
        raw = path.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        snapshot = result / "tool-sources" / (digest + path.suffix)
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        if snapshot.exists():
            if snapshot.read_bytes() != raw:
                raise ValueError("Retained exception-region tool snapshot changed")
        else:
            with snapshot.open("xb") as stream:
                stream.write(raw)
        location = {"installation_path": path.relative_to(ida_root).as_posix()} if installed else {"path": rel(path)}
        return {**location, "bytes": len(raw), "sha256": digest, "snapshot": identity(snapshot)}

    tool_sources = {name: retain_tool(ROOT / path) for name, path in {
        "capture": "tools/ida/capture.py", "parser": "tools/host.py",
        "entry": "tools/ida/ida-start-mcp.py", "support": "tools/ida/capture_io.py",
    }.items()}
    tool_sources["analyzer"] = retain_tool(executable, installed=True)
    tool_sources.update({name: retain_tool(ida_root / path, installed=True)
                         for name, path in IA64_EXCEPTION_ANALYZER_FILES.items()})
    tool_sources["sdk"] = {name: retain_tool(ida_root / "python" / (name + ".py"), installed=True)
                           for name in IA64_EXCEPTION_SDK_MODULES}
    tool_rows = [row for name, row in tool_sources.items() if name != "sdk"] + list(tool_sources["sdk"].values())

    def verify_tools():
        for row in tool_rows:
            path = ida_root / row["installation_path"] if "installation_path" in row else ROOT / row["path"]
            retained = ROOT / row["snapshot"]["path"]
            if (path.stat().st_size != row["bytes"] or sha(path) != row["sha256"]
                    or retained.stat().st_size != row["bytes"] or sha(retained) != row["sha256"]):
                raise ValueError("Exception-region analyzer tool or snapshot changed")

    tool, parser = tool_sources["capture"], tool_sources["parser"]
    base_identity, database_identity = identity(base_capture), identity(source_database)
    selection = {"mode": "read_only_ia64_exception_regions", "directory": directory,
        "captured_chunk_coverage": native_exception_region_coverage(directory, functions, pe["image_base"]),
        "capture_tool": tool, "parser_tool": parser, "base_capture": base_identity,
        "source_database": database_identity, "tool_sources": tool_sources,
        "processor": {"name": "ia64l", "module": "ia", "bits": 64, "big_endian": False},
        "reason": "The installed IA64 processor explicitly does not parse .pdata. Export every original exception region, including bytes outside captured chunks, using decode_insn without defining functions, changing ownership, ABI/SP or invoking a decompiler."}
    selection_id = hashlib.sha256(json.dumps(selection, sort_keys=True).encode()).hexdigest()[:16]
    output = result / "exception-regions" / selection_id
    scratch = ROOT / ".work/native" / digest / "exception-regions" / selection_id
    if output.exists() and any(output.iterdir()):
        raise ValueError("Identical exception-region export already has an attempt; inspect its immutable result")
    configure_ida()
    output.mkdir(parents=True, exist_ok=True)
    scratch.mkdir(parents=True, exist_ok=True)
    database = scratch / source_database.name
    shutil.copyfile(source_database, database)
    if sha(database) != database_identity["sha256"] or sha(source_database) != database_identity["sha256"]:
        raise ValueError("Exception-region private IDB copy differs")
    plan = {"schema_version": 1, "input": rel(source), "sha256": digest, "input_bytes": original["bytes"],
        "output": rel(output), "acquisition_binding": original,
        "database": identity(database), "source_database": database_identity,
        "base_capture": base_identity, "capture_tool": tool, "parser_tool": parser,
        "tool_sources": tool_sources,
        "exception_regions": selection, "target_execution": False, "real_device_operations": False}
    save(output / "plan.json", plan)
    env = os.environ.copy()
    env["IDAUSR"] = str(ROOT / "tools/ida/idausr")
    env.pop("RAZER_IDA_SUPPLEMENT_PLAN", None)
    env["RAZER_IDA_CAPTURE_PLAN"] = str(output / "plan.json")
    # The installed command-line reference defines -a as disabling autoanalysis.
    command = [str(executable), "-a", "-A", "-Opdb:off", "-L" + str(scratch / "ida.log"),
               "-S" + str(ROOT / "tools/ida/ida-start-mcp.py"), str(database)]
    options = {}
    if os.name == "nt":
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        options = {"startupinfo": startup, "creationflags": subprocess.CREATE_NO_WINDOW}
    verify_tools()
    with (scratch / "process.log").open("wb") as log:
        proc = subprocess.Popen(command, cwd=scratch, env=env, stdout=log, stderr=subprocess.STDOUT, **options)
    session_path = output / "session.json"
    session = {"pid": proc.pid, "started_at_utc": stamp(), "analyzer": executable.name,
        "analyzer_sha256": tool_sources["analyzer"]["sha256"], "capture_tool": tool, "parser_tool": parser,
        "tool_sources": tool_sources,
        "base_capture": base_identity, "source_database": database_identity,
        "argv": [executable.name, "-a", "-A", "-Opdb:off", "-L" + rel(scratch / "ida.log"),
                 "-Stools/ida/ida-start-mcp.py", rel(database)],
        "target_execution": False, "real_device_operations": False}
    save(session_path, session)
    print(json.dumps({"pid": proc.pid, "result": rel(output), "log": rel(scratch / "ida.log")}), flush=True)
    session["exit_code"] = proc.wait()
    session["finished_at_utc"] = stamp()
    verify_tools()
    for row in [base_identity, database_identity, tool, parser, *function_sources]:
        if sha(ROOT / row["path"]) != row["sha256"]:
            raise ValueError("Exception-region original evidence changed during export")
    manifest_path = output / "capture.json"
    manifest = json.loads(manifest_path.read_bytes()) if manifest_path.is_file() else {}
    session["passed"] = (session["exit_code"] == 0 and manifest.get("capture_complete") is True
        and manifest.get("input_sha256") == digest and manifest.get("exception_regions") == selection
        and manifest.get("tool_sources") == tool_sources and manifest.get("function_chunks_unchanged") is True)
    save(session_path, session)
    if not getattr(args, "no_refresh_gaps", False):
        refresh_gaps()
    print(json.dumps({"session": rel(session_path), "passed": session["passed"]}), flush=True)
    return session


def native_pointer_targets(raw, table_rva, count):
    """Resolve an explicit native pointer table from file-backed PE bytes."""
    from host import inspect_pe

    pe = inspect_pe(raw)
    if pe["kind"] not in {"native", "mixed_managed_native"}:
        raise ValueError("Pointer-table input is not a native PE image")
    if not 1 <= count <= 4096 or table_rva < 0:
        raise ValueError("Pointer-table range is invalid")
    width = 8 if pe["format"] == "pe32+" else 4
    base = int(pe["image_base"], 16)
    pe_offset = int.from_bytes(raw[0x3c:0x40], "little")
    section_table = pe_offset + 24 + int.from_bytes(raw[pe_offset + 20:pe_offset + 22], "little")

    def executable(rva):
        for index, section in enumerate(pe["sections"]):
            if section["rva"] <= rva < section["rva"] + section["bytes"]:
                flags_at = section_table + index * 40 + 36
                return bool(int.from_bytes(raw[flags_at:flags_at + 4], "little") & 0x20000000)
        return False

    def offset(rva, length):
        matches = [section for section in pe["sections"]
                   if 0 <= rva - section["rva"] and rva - section["rva"] + length <= section["bytes"]]
        if len(matches) != 1:
            raise ValueError("Pointer-table range is not uniquely file-backed: " + hex(rva))
        return matches[0]["file_offset"] + rva - matches[0]["rva"]

    start = offset(table_rva, width * count)
    table = raw[start:start + width * count]
    targets = []
    for index in range(count):
        value = table[index * width:(index + 1) * width]
        address = int.from_bytes(value, "little")
        rva = address - base
        target_offset = offset(rva, 1)
        if not executable(rva):
            raise ValueError("Pointer-table target is not in an executable PE section: " + hex(rva))
        targets.append({"index": index, "slot_rva": hex(table_rva + index * width),
                        "slot_file_offset": start + index * width, "slot_raw_hex": value.hex(),
                        "address": hex(address), "rva": hex(rva), "file_offset": target_offset})
    return {"image_base": hex(base), "architecture": pe["architecture"], "pointer_bytes": width,
            "rva": hex(table_rva), "file_offset": start, "bytes": len(table),
            "raw_hex": table.hex(), "sha256": hashlib.sha256(table).hexdigest(), "targets": targets}


def native_member_dispatch_thunk_size(raw, architecture):
    """Recognize only the complete x64 bound-member tail call.

    The instructions load the target from this+8 and the bound object from
    this+24, then tail-call RAX. Constructors must prove both stored values;
    recognizing this boundary does not resolve or execute the target.
    """
    expected = bytes.fromhex("48 8b 41 08 48 8b 49 18 48 ff e0")
    return len(expected) if architecture == "x64" and raw == expected else None


def native_import_type_recoveries(raw, specifications, evidence):
    """Bind reviewed pointer declarations only to byte-backed named PE IAT slots."""
    from host import inspect_pe
    pe = inspect_pe(raw)
    if pe["architecture"] not in {"x86", "x64"}:
        raise ValueError("Import type recovery supports only retained x86/x64 PE imports")
    if any(row.get("stage") == "imports" for row in pe["unresolved"]):
        raise ValueError("Import type recovery requires a complete PE import directory")
    width = 8 if pe["architecture"] == "x64" else 4

    def offset(rva, count):
        matches = [section for section in pe["sections"]
                   if 0 <= rva - section["rva"] and rva - section["rva"] + count <= section["bytes"]]
        if len(matches) != 1:
            raise ValueError("Import type recovery RVA is not uniquely file-backed")
        return matches[0]["file_offset"] + rva - matches[0]["rva"]

    header = int.from_bytes(raw[0x3c:0x40], "little") + 24
    directory = header + (112 if width == 8 else 96) + 8
    rva = int.from_bytes(raw[directory:directory + 4], "little")
    size = int.from_bytes(raw[directory + 4:directory + 8], "little")
    slots = {}
    if rva and size:
        for index, module in enumerate(pe["imports"]):
            at = offset(rva + index * 20, 20)
            table = int.from_bytes(raw[at + 16:at + 20], "little")
            for symbol_index, symbol in enumerate(module["symbols"]):
                slot = table + width * symbol_index
                start = offset(slot, width)
                slots[slot] = {"library": module["library"], **symbol,
                               "rva": hex(slot), "raw_hex": raw[start:start + width].hex(), "bytes": width}
    recovered = []
    for specification in specifications:
        rva_text, separator, declaration = specification.partition("=")
        if not separator or not declaration.strip() or not evidence or not evidence.strip():
            raise ValueError("Import type recovery requires RVA=DECLARATION and assembly evidence")
        slot = int(rva_text, 0)
        if slot not in slots or "name" not in slots[slot] or any(row["rva"] == hex(slot) for row in recovered):
            raise ValueError("Import type recovery must select a unique named normal PE IAT slot")
        recovered.append({**slots[slot], "declaration": declaration.strip(), "assembly_evidence": evidence.strip()})
    return recovered


def native_import_binding_matches(requested, binding):
    """Keep original PE bytes separate from IDA's enumerated import view."""
    try:
        return (binding["rva"] == requested["rva"] and binding["name"] == requested["name"]
            and Path(binding["module"]).stem.casefold() == Path(requested["library"]).stem.casefold()
            and len(bytes.fromhex(binding["loaded_slot_raw_hex"])) == requested["bytes"])
    except (ValueError, KeyError, TypeError):
        return False


def native_import_thunks(raw, capture, imports, pe):
    """Bind reviewed IAT types to exact retained six-byte import jumps."""
    slots = {int(row["rva"], 16): row for row in imports}
    base = int(pe["image_base"], 16)
    selected, seen = [], set()
    pe_offset = int.from_bytes(raw[0x3c:0x40], "little")
    section_table = pe_offset + 24 + int.from_bytes(raw[pe_offset + 20:pe_offset + 22], "little")
    for row in capture["functions"]:
        rva = int(row["rva"], 16)
        sections = [(index, section) for index, section in enumerate(pe["sections"])
                    if 0 <= rva - section["rva"] and rva - section["rva"] + 6 <= section["bytes"]]
        if len(sections) != 1:
            continue
        index, section = sections[0]
        start = section["file_offset"] + rva - section["rva"]
        code = raw[start:start + 6]
        if code[:2] != b"\xff\x25":
            continue
        operand = int.from_bytes(code[2:], "little", signed=pe["architecture"] == "x64")
        slot = rva + 6 + operand if pe["architecture"] == "x64" else operand - base
        imported = slots.get(slot)
        if imported is None:
            continue
        flags_at = section_table + index * 40 + 36
        if (rva in seen or int(row["address"], 16) != base + rva
                or not int.from_bytes(raw[flags_at:flags_at + 4], "little") & 0x20000000):
            raise ValueError("Import thunk must use a unique retained executable function boundary")
        seen.add(rva)
        if capture.get("input_sha256") != hashlib.sha256(raw).hexdigest():
            raise ValueError("Import thunk original identity differs from its base capture")
        result = ROOT / ".analysis/results/native" / capture["input_sha256"]
        path = resolve(rel(result / row["path"]), rel(result))
        retained = path.read_bytes()
        digest = hashlib.sha256(retained).hexdigest()
        if digest != row["sha256"] or ("bytes" in row and len(retained) != row["bytes"]):
            raise ValueError("Retained import thunk function identity changed: " + rel(path))
        body = json.loads(retained)
        chunks, assembly = body.get("chunks"), body.get("assembly")
        address = base + rva
        if (int(body["address"], 16) != address or int(body["rva"], 16) != rva
                or int(body["end"], 16) != address + 6
                or not isinstance(chunks, list) or len(chunks) != 1
                or not isinstance(assembly, list) or len(assembly) != 1):
            raise ValueError("Retained import thunk must contain one exact six-byte chunk and JMP")
        chunk, instruction = chunks[0], assembly[0]
        if (int(chunk["start"], 16) != address or int(chunk["end"], 16) != address + 6
                or chunk.get("raw_hex") != code.hex()
                or ("sha256" in chunk and chunk["sha256"] != hashlib.sha256(code).hexdigest())
                or int(instruction["address"], 16) != address
                or ("rva" in instruction and int(instruction["rva"], 16) != rva)
                or instruction.get("bytes") != code.hex()
                or not re.match(r"^jmp\s+", instruction.get("text", "").strip(), re.IGNORECASE)):
            raise ValueError("Retained import thunk differs from the original six-byte FF25 jump")
        selected.append({"rva": hex(rva), "raw_hex": code.hex(), "bytes": 6,
                         "slot_rva": hex(slot), "library": imported["library"], "name": imported["name"],
                         "source_function": {"path": rel(path), "bytes": len(retained), "sha256": digest}})
    return selected


def native_function_merges(raw, capture, function_rvas, specifications, evidence, pe):
    """Bind explicitly reviewed adjacent functions to their complete original bytes.

    Contiguity and incoming call checks constrain a requested static merge; they
    do not prove that adjacent instructions share one semantic function.
    """
    digest = hashlib.sha256(raw).hexdigest()
    if (capture.get("input_sha256") != digest or capture.get("input_bytes") != len(raw)
            or capture.get("capture_complete") is not True):
        raise ValueError("Function merge requires a complete capture of the exact original input")
    if not evidence or not evidence.strip():
        raise ValueError("Function merge requires explicit assembly evidence")
    base = int(pe["image_base"], 16)
    if int(capture.get("image_base", "-1"), 16) != base:
        raise ValueError("Function merge capture image base differs from the original PE")
    result = ROOT / ".analysis/results/native" / digest
    existing = {}
    for row in capture["functions"]:
        rva = int(row["rva"], 16)
        if rva in existing:
            raise ValueError("Function merge requires unique retained function boundaries")
        existing[rva] = row

    def original_bytes(rva, size):
        matches = [section for section in pe["sections"]
                   if 0 <= rva - section["rva"] and rva - section["rva"] + size <= section["bytes"]]
        if size <= 0 or len(matches) != 1:
            raise ValueError("Function merge bytes are not uniquely file-backed")
        section = matches[0]
        at = section["file_offset"] + rva - section["rva"]
        code = raw[at:at + size]
        if len(code) != size:
            raise ValueError("Function merge original bytes are truncated")
        return code

    merges, used = [], set()
    for specification in specifications:
        owner_text, separator, member_text = specification.partition("=")
        try:
            owner = int(owner_text, 0)
            members = [int(value.strip(), 0) for value in member_text.split(",")]
        except ValueError as error:
            raise ValueError("Function merge requires OWNER=RVA,RVA,...") from error
        if (not separator or not 2 <= len(members) <= 4096 or members[0] != owner
                or owner not in function_rvas or len(set(members)) != len(members)
                or any(rva in used for rva in members)
                or any(rva in function_rvas for rva in members[1:])):
            raise ValueError("Function merge requires a selected first owner and unique unselected members")
        retained, bodies, previous_end = [], [], None
        for rva in members:
            row = existing.get(rva)
            if row is None or row.get("assembly_captured") is not True:
                raise ValueError("Function merge member lacks a complete retained assembly")
            path = resolve(rel(result / row["path"]), rel(result))
            data = path.read_bytes()
            identity = {"rva": hex(rva), "path": rel(path), "bytes": len(data),
                        "sha256": hashlib.sha256(data).hexdigest()}
            if identity["sha256"] != row["sha256"] or ("bytes" in row and len(data) != row["bytes"]):
                raise ValueError("Function merge retained member identity changed")
            body = json.loads(data)
            start, end = int(body["address"], 16), int(body["end"], 16)
            chunks, assembly = body.get("chunks"), body.get("assembly")
            if (int(row["address"], 16) != base + rva or start != base + rva
                    or int(body["rva"], 16) != rva or end <= start
                    or not isinstance(chunks, list) or len(chunks) != 1
                    or not isinstance(assembly, list) or not assembly
                    or not isinstance(body.get("xrefs_to_entry"), list)):
                raise ValueError("Function merge member must have one complete exact chunk")
            if previous_end is not None and start != previous_end:
                raise ValueError("Function merge members have an overlap, hole or unordered boundary")
            chunk = chunks[0]
            code = original_bytes(rva, end - start)
            if (int(chunk["start"], 16) != start or int(chunk["end"], 16) != end
                    or chunk.get("raw_hex") != code.hex()
                    or chunk.get("sha256") != hashlib.sha256(code).hexdigest()):
                raise ValueError("Function merge member chunk differs from original PE bytes")
            instruction_end = start
            for instruction in assembly:
                at = int(instruction["address"], 16)
                value = bytes.fromhex(instruction["bytes"])
                if (not value or at < instruction_end or at < start or at + len(value) > end
                        or ("rva" in instruction and int(instruction["rva"], 16) != at - base)
                        or original_bytes(at - base, len(value)) != value):
                    raise ValueError("Function merge member assembly differs from original PE bytes")
                instruction_end = at + len(value)
            retained.append(identity)
            bodies.append(body)
            previous_end = end
        start, end = base + owner, previous_end
        for body in bodies[1:]:
            for xref in body["xrefs_to_entry"]:
                if (xref.get("code") is True and xref.get("type") in {16, 17}
                        and not start <= int(xref["from"], 16) < end):
                    raise ValueError("Function merge member is an external call target")
        code = original_bytes(owner, end - start)
        merges.append({"owner_rva": hex(owner), "member_rvas": [hex(rva) for rva in members],
                       "assembly_evidence": evidence.strip(), "members": retained,
                       "expected_start": hex(start), "expected_end": hex(end),
                       "expected_chunks": [{"start": hex(start), "end": hex(end),
                                            "sha256": hashlib.sha256(code).hexdigest(), "raw_hex": code.hex()}]})
        used.update(members)
    return merges


def native_function_merge_body(merge):
    """Compose a baseline from the exact previously PE-checked member identities."""
    bodies = []
    for member in merge["members"]:
        path = resolve(member["path"], ".analysis/results/native")
        data = path.read_bytes()
        if len(data) != member["bytes"] or hashlib.sha256(data).hexdigest() != member["sha256"]:
            raise ValueError("Function merge member identity changed while composing its baseline")
        bodies.append(json.loads(data))
    baseline = dict(bodies[0])
    baseline["end"] = merge["expected_end"]
    baseline["chunks"] = merge["expected_chunks"]
    baseline["assembly"] = [instruction for body in bodies for instruction in body["assembly"]]
    return baseline


def native_function_outlines(raw, capture, function_rvas, outline_rvas, evidence, pe):
    """Bind reviewed outlined helpers to unchanged complete retained code."""
    digest = hashlib.sha256(raw).hexdigest()
    if (capture.get("input_sha256") != digest or capture.get("input_bytes") != len(raw)
            or capture.get("capture_complete") is not True or not evidence or not evidence.strip()):
        raise ValueError("Outlined helpers require exact complete input and explicit assembly evidence")
    values = [int(value, 0) if isinstance(value, str) else value for value in outline_rvas]
    if (not values or len(values) > 4096 or len(set(values)) != len(values)
            or any(type(value) is not int or value in function_rvas for value in values)):
        raise ValueError("Outlined helpers must be unique retained boundaries separate from output functions")
    base = int(pe["image_base"], 16)
    result = ROOT / ".analysis/results/native" / digest
    existing = {int(row["rva"], 16): row for row in capture["functions"]}
    if len(existing) != len(capture["functions"]) or int(capture["image_base"], 16) != base:
        raise ValueError("Outlined helper capture boundaries or image base differ")

    def original_bytes(start, end):
        matches = [section for section in pe["sections"]
                   if 0 <= start - base - section["rva"]
                   and end - base - section["rva"] <= section["bytes"]]
        if end <= start or len(matches) != 1:
            raise ValueError("Outlined helper code is not uniquely file-backed")
        section = matches[0]
        offset = section["file_offset"] + start - base - section["rva"]
        value = raw[offset:offset + end - start]
        if len(value) != end - start:
            raise ValueError("Outlined helper original code is truncated")
        return value

    outlines = []
    for rva in values:
        row = existing.get(rva)
        if row is None or row.get("assembly_captured") is not True:
            raise ValueError("Outlined helper lacks complete retained assembly")
        path = resolve(rel(result / row["path"]), rel(result))
        data = path.read_bytes()
        identity = {"path": rel(path), "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        if identity["sha256"] != row["sha256"] or ("bytes" in row and row["bytes"] != len(data)):
            raise ValueError("Outlined helper retained identity changed")
        body = json.loads(data)
        chunks, assembly = body.get("chunks"), body.get("assembly")
        if (body["address"] != hex(base + rva) or body["rva"] != hex(rva)
                or row["address"] != body["address"] or type(body.get("flags")) is not int
                or not isinstance(chunks, list) or not chunks or not isinstance(assembly, list) or not assembly):
            raise ValueError("Outlined helper lacks its exact complete boundary")
        previous_end = -1
        for chunk in chunks:
            start, end = int(chunk["start"], 16), int(chunk["end"], 16)
            code = original_bytes(start, end)
            if (start < previous_end or chunk.get("raw_hex") != code.hex()
                    or chunk.get("sha256") != hashlib.sha256(code).hexdigest()):
                raise ValueError("Outlined helper chunk differs from original code")
            previous_end = end
        if not any(int(chunk["start"], 16) == base + rva for chunk in chunks):
            raise ValueError("Outlined helper chunks omit the entry")
        previous_end = -1
        for instruction in assembly:
            at = int(instruction["address"], 16)
            code = bytes.fromhex(instruction["bytes"])
            if (not code or at < previous_end or original_bytes(at, at + len(code)) != code
                    or not any(int(chunk["start"], 16) <= at < at + len(code) <= int(chunk["end"], 16) for chunk in chunks)):
                raise ValueError("Outlined helper instruction differs from complete original chunks")
            previous_end = at + len(code)
        outlines.append({"rva": hex(rva), "source_function": identity, "expected_end": body["end"],
                         "expected_chunks": chunks, "expected_flags": body["flags"],
                         "set_flag": "FUNC_OUTLINE", "assembly_evidence": evidence.strip()})
    return outlines


def native_function_shared_tails(raw, capture, function_rvas, specifications, evidence, pe):
    """Bind exact shared code tails to their original retained function owners.

    A shared tail is a byte-identical interval that already belongs to the
    primary retained function and is reached by one or more other retained
    entries.  The input describes only the reviewed ownership claim; every
    boundary, byte, instruction and incoming edge is recomputed from the
    original PE and retained capture before it is placed in a supplement plan.
    """
    digest = hashlib.sha256(raw).hexdigest()
    if (capture.get("input_sha256") != digest or capture.get("input_bytes") != len(raw)
            or capture.get("capture_complete") is not True or pe["architecture"] not in {"x86", "x64"}
            or not specifications or len(specifications) > 4096):
        raise ValueError("Shared function tails require an exact complete original capture")
    base = int(pe["image_base"], 16)
    if int(capture.get("image_base", "-1"), 16) != base:
        raise ValueError("Shared function tail capture image base differs from the original PE")
    result = ROOT / ".analysis/results/native" / digest
    existing = {}
    for row in capture.get("functions", []):
        rva = int(row["rva"], 16)
        if rva in existing:
            raise ValueError("Shared function tails require unique retained function boundaries")
        existing[rva] = row

    def integer(value, label):
        try:
            parsed = int(value, 0) if isinstance(value, str) else value
            if type(parsed) is not int or parsed < 0:
                raise ValueError(label)
            return parsed
        except (TypeError, ValueError) as error:
            raise ValueError("Shared function tail has an invalid " + label) from error

    def parse(specification):
        if isinstance(specification, dict):
            required = {"tail_start_rva", "tail_end_rva", "owner_rvas", "primary_owner_rva", "inline_owners"}
            if not required.issubset(specification):
                raise ValueError("Shared function tail plan omits a required field")
            start = integer(specification["tail_start_rva"], "tail start")
            end = integer(specification["tail_end_rva"], "tail end")
            if not isinstance(specification["owner_rvas"], list):
                raise ValueError("Shared function tail owner RVAs must be a list")
            owners = [integer(value, "owner RVA") for value in specification["owner_rvas"]]
            primary = integer(specification["primary_owner_rva"], "primary owner RVA")
            inline = specification["inline_owners"]
            note = specification.get("assembly_evidence") or evidence
        else:
            text = str(specification).strip()
            owner_text, separator, remainder = text.partition("=")
            if not separator:
                raise ValueError("Shared function tail requires PRIMARY=START:END:OWNER,OWNER")
            pieces = remainder.split(":")
            if len(pieces) < 3:
                raise ValueError("Shared function tail requires PRIMARY=START:END:OWNER,OWNER")
            primary = integer(owner_text, "primary owner RVA")
            start = integer(pieces[0], "tail start")
            end = integer(pieces[1], "tail end")
            owner_text = ":".join(pieces[2:])
            inline = True
            if ":" in owner_text:
                owner_text, mode = owner_text.rsplit(":", 1)
                if mode.strip().casefold() == "retain":
                    inline = False
                elif mode.strip().casefold() != "inline":
                    raise ValueError("Shared function tail mode must be inline or retain")
            owners = [integer(value.strip(), "owner RVA") for value in owner_text.split(",")]
            note = evidence
        if (type(inline) is not bool or end <= start or not 2 <= len(owners) <= 4096
                or len(set(owners)) != len(owners) or primary not in owners
                or not isinstance(note, str) or not note.strip()):
            raise ValueError("Shared function tail requires unique owners, a primary and assembly evidence")
        return {"tail_start_rva": hex(start), "tail_end_rva": hex(end),
                "owner_rvas": [hex(value) for value in owners],
                "primary_owner_rva": hex(primary), "inline_owners": inline,
                "assembly_evidence": note.strip()}

    def file_bytes(start, end):
        matches = [section for section in pe["sections"]
                   if 0 <= start - base - section["rva"]
                   and end - base - section["rva"] <= section["bytes"]]
        if end <= start or len(matches) != 1:
            raise ValueError("Shared function tail range is not uniquely file-backed")
        section = matches[0]
        offset = section["file_offset"] + start - base - section["rva"]
        value = raw[offset:offset + end - start]
        if len(value) != end - start:
            raise ValueError("Shared function tail original bytes are truncated")
        return value

    def edge_key(edge):
        if (not isinstance(edge, dict) or not isinstance(edge.get("from"), str)
                or not isinstance(edge.get("to"), str) or type(edge.get("type")) is not int
                or type(edge.get("code")) is not bool
                or ("user" in edge and type(edge["user"]) is not bool)):
            raise ValueError("Shared function tail contains an incomplete xref")
        return (int(edge["from"], 16), int(edge["to"], 16), edge["type"], edge["code"], edge.get("user", False))

    def canonical_edges(edges):
        result_edges = {}
        for edge in edges:
            key = edge_key(edge)
            if key in result_edges:
                raise ValueError("Shared function tail contains a duplicated xref")
            result_edges[key] = dict(edge)
        return [result_edges[key] for key in sorted(result_edges)]

    def read_owner(rva):
        row = existing.get(rva)
        if row is None or row.get("assembly_captured") is not True:
            raise ValueError("Shared function tail owner lacks complete retained assembly: " + hex(rva))
        path = resolve(rel(result / row["path"]), rel(result))
        data = path.read_bytes()
        identity = {"path": rel(path), "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        if identity["sha256"] != row["sha256"] or ("bytes" in row and row["bytes"] != len(data)):
            raise ValueError("Shared function tail owner retained identity changed")
        body = json.loads(data)
        start, end = int(body["address"], 16), int(body["end"], 16)
        if (body.get("rva") != hex(rva) or start != base + rva or end <= start
                or row.get("address") != hex(start) or type(body.get("flags")) is not int
                or not isinstance(body.get("chunks"), list) or not body["chunks"]
                or not isinstance(body.get("assembly"), list) or not body["assembly"]
                or not isinstance(body.get("xrefs_to_entry"), list)):
            raise ValueError("Shared function tail owner lacks one exact complete retained body")
        chunks, previous = [], -1
        for chunk in body["chunks"]:
            cstart, cend = int(chunk["start"], 16), int(chunk["end"], 16)
            code = file_bytes(cstart, cend)
            if (cend <= cstart or cstart < previous
                    or chunk.get("raw_hex") != code.hex()
                    or chunk.get("sha256") != hashlib.sha256(code).hexdigest()):
                raise ValueError("Shared function tail owner chunk differs from original PE")
            chunks.append({"start": hex(cstart), "end": hex(cend), "raw_hex": code.hex(),
                           "sha256": hashlib.sha256(code).hexdigest()})
            previous = cend
        if not any(chunk["start"] == hex(start) and chunk["end"] == hex(end) for chunk in chunks):
            raise ValueError("Shared function tail owner omits its exact original entry chunk")
        assembly, previous, addresses = [], -1, set()
        for instruction in body["assembly"]:
            at = int(instruction["address"], 16)
            code = bytes.fromhex(instruction["bytes"])
            if (not code or at in addresses or at < previous
                    or ("rva" in instruction and int(instruction["rva"], 16) != at - base)
                    or file_bytes(at, at + len(code)) != code
                    or not any(int(c["start"], 16) <= at < at + len(code) <= int(c["end"], 16) for c in chunks)
                    or not isinstance(instruction.get("xrefs_from"), list)
                    or not isinstance(instruction.get("xrefs_to"), list)):
                raise ValueError("Shared function tail owner assembly differs from original PE")
            for direction, field in (("from", "xrefs_from"), ("to", "xrefs_to")):
                canonical_edges(instruction[field])
                if any(int(edge[direction], 16) != at for edge in instruction[field]):
                    raise ValueError("Shared function tail instruction xref uses a different instruction boundary")
            assembly.append(instruction)
            addresses.add(at)
            previous = at + len(code)
        entry = next((instruction for instruction in assembly if int(instruction["address"], 16) == start), None)
        if entry is None or body["xrefs_to_entry"] != entry["xrefs_to"]:
            raise ValueError("Shared function tail owner omits its original entry or entry references")
        return {"rva": hex(rva), "body": body, "source": identity, "start": start, "end": end,
                "chunks": chunks, "assembly": assembly}

    def control_flow(instruction):
        """Check the relative x86/x64 transfers used by the retained tails."""
        at, code = int(instruction["address"], 16), bytes.fromhex(instruction["bytes"])
        stop = at + len(code)
        if code in {b"\xc3", b"\xf3\xc3", b"\xf2\xc3"} or len(code) == 3 and code[0] == 0xc2:
            return set()
        if len(code) == 5 and code[0] in {0xe8, 0xe9}:
            target = stop + int.from_bytes(code[1:], "little", signed=True)
            return {(17, target), (21, stop)} if code[0] == 0xe8 else {(19, target)}
        if len(code) == 2 and code[0] == 0xeb:
            return {(19, stop + int.from_bytes(code[1:], "little", signed=True))}
        if len(code) == 2 and (0x70 <= code[0] <= 0x7f or 0xe0 <= code[0] <= 0xe3):
            return {(19, stop + int.from_bytes(code[1:], "little", signed=True)), (21, stop)}
        if len(code) == 6 and code[0] == 0x0f and 0x80 <= code[1] <= 0x8f:
            return {(19, stop + int.from_bytes(code[2:], "little", signed=True)), (21, stop)}
        mnemonic = instruction.get("text", "").strip().split(" ", 1)[0].lower()
        if (not mnemonic or mnemonic.startswith(("j", "ret", "call", "iret", "loop"))
                or mnemonic in {"rep", "repe", "repz", "repne", "repnz", "int", "int3", "ud2", "hlt"}):
            raise ValueError("Shared function tail has an unproved control transfer")
        return {(21, stop)}

    def verify_flow(instruction):
        edges = [edge for edge in instruction["xrefs_from"] if edge["code"]]
        if {(edge["type"], int(edge["to"], 16)) for edge in edges} != control_flow(instruction):
            raise ValueError("Shared function tail xrefs differ from actual relative control flow")
        return edges

    plans, seen_owners, seen_ranges = [], set(), []
    for specification in specifications:
        requested = parse(specification)
        start_rva, end_rva = int(requested["tail_start_rva"], 16), int(requested["tail_end_rva"], 16)
        start, end = base + start_rva, base + end_rva
        owners = [int(value, 16) for value in requested["owner_rvas"]]
        primary = int(requested["primary_owner_rva"], 16)
        if any(owner in seen_owners for owner in owners):
            raise ValueError("Shared function tail owner is already assigned to another shared tail")
        if any(start < old_end and old_start < end for old_start, old_end in seen_ranges):
            raise ValueError("Shared function tails overlap")
        owner_data = {rva: read_owner(rva) for rva in owners}
        donor = owner_data[primary]
        if not donor["start"] < start < end == donor["end"]:
            raise ValueError("Shared tail must be an exact suffix of its primary retained function")
        if any(start_rva <= rva < end_rva for rva in existing):
            raise ValueError("Shared tail would absorb another original function entry")
        donor_instruction_starts = {int(ins["address"], 16) for ins in donor["assembly"]}
        if start not in donor_instruction_starts:
            raise ValueError("Shared tail starts inside an original instruction")
        tail_assembly = [ins for ins in donor["assembly"] if start <= int(ins["address"], 16) < end]
        cursor = start
        for instruction in tail_assembly:
            at = int(instruction["address"], 16)
            code = bytes.fromhex(instruction["bytes"])
            if at < cursor or at + len(code) > end:
                raise ValueError("Shared tail omits an original instruction or byte")
            cursor = at + len(code)
        if cursor != end:
            raise ValueError("Shared tail does not contain complete original instructions")
        for instruction in tail_assembly:
            verify_flow(instruction)
        tail_code = file_bytes(start, end)
        tail = {"start": hex(start), "end": hex(end), "raw_hex": tail_code.hex(),
                "sha256": hashlib.sha256(tail_code).hexdigest()}

        # Build exact post-application chunks.  The primary retains every
        # original byte before the suffix and receives the common tail chunk;
        # every other owner retains its original chunks and receives that same
        # exact file-backed tail.
        def split_primary_chunks():
            values = []
            for chunk in donor["chunks"]:
                cstart, cend = int(chunk["start"], 16), int(chunk["end"], 16)
                if cstart < start:
                    stop = min(cend, start)
                    if stop > cstart:
                        code = file_bytes(cstart, stop)
                        values.append({"start": hex(cstart), "end": hex(stop), "raw_hex": code.hex(),
                                       "sha256": hashlib.sha256(code).hexdigest()})
                if cend > end:
                    code = file_bytes(max(cstart, end), cend)
                    values.append({"start": hex(max(cstart, end)), "end": hex(cend), "raw_hex": code.hex(),
                                   "sha256": hashlib.sha256(code).hexdigest()})
            values.append(dict(tail))
            return sorted(values, key=lambda value: int(value["start"], 16))

        original_ranges, instructions, instruction_owners = [], {}, {}
        for rva, owner in owner_data.items():
            for chunk in owner["chunks"]:
                lo, hi = int(chunk["start"], 16), int(chunk["end"], 16)
                if any(lo < old_hi and old_lo < hi for old_lo, old_hi in original_ranges):
                    raise ValueError("Shared tail original owners overlap outside declared ownership")
                original_ranges.append((lo, hi))
            for instruction in owner["assembly"]:
                at = int(instruction["address"], 16)
                if at in instructions:
                    raise ValueError("Shared tail original owner instructions overlap")
                instructions[at], instruction_owners[at] = instruction, rva
        incoming, outgoing, incoming_owners = [], [], set()
        for instruction in tail_assembly:
            for edge in instruction["xrefs_to"]:
                source = int(edge["from"], 16)
                if not edge["code"]:
                    if not start <= source < end:
                        incoming.append(edge)
                    continue
                site = instructions.get(source)
                if site is None:
                    raise ValueError("Shared tail has a foreign incoming code edge")
                if edge not in verify_flow(site):
                    raise ValueError("Shared tail incoming edge differs from its original source instruction")
                if not start <= source < end:
                    if edge["type"] not in {19, 21}:
                        raise ValueError("Shared tail has an incoming CALL or unproved entry")
                    incoming.append(edge)
                    incoming_owners.add(instruction_owners[source])
            for edge in instruction["xrefs_from"]:
                target = int(edge["to"], 16)
                if edge["code"]:
                    if start <= target < end:
                        if target not in instructions or edge not in instructions[target]["xrefs_to"]:
                            raise ValueError("Shared tail internal control-flow references are incomplete")
                    elif edge["type"] not in {16, 17}:
                        raise ValueError("Shared tail has a nonlocal non-CALL control-flow exit")
                if not start <= target < end:
                    outgoing.append(edge)
        # Cross-check both directions so removing a retained incoming xref
        # cannot hide a real owner jump or fallthrough into the tail.
        for at, instruction in instructions.items():
            for edge in instruction["xrefs_from"]:
                target = int(edge["to"], 16)
                if edge["code"] and start <= target < end:
                    if target not in instructions or edge not in instructions[target]["xrefs_to"]:
                        raise ValueError("Shared tail omits a real incoming code edge")
        incoming = canonical_edges(incoming)
        outgoing = canonical_edges(outgoing)
        if incoming_owners != set(owners):
            raise ValueError("Shared tail incoming edges do not identify exactly all declared owners")
        expected_owners = []
        for rva in owners:
            owner = owner_data[rva]
            chunks = split_primary_chunks() if rva == primary else sorted(owner["chunks"] + [dict(tail)],
                                                                          key=lambda value: int(value["start"], 16))
            expected_owners.append({"rva": hex(rva), "source_function": owner["source"],
                "original_end": hex(owner["end"]), "original_chunks": owner["chunks"],
                "expected_end": hex(start) if rva == primary else hex(owner["end"]), "expected_chunks": chunks,
                "expected_flags": owner["body"]["flags"]})
        plans.append({**requested, "expected_tail": tail, "incoming_edges": incoming,
                      "outgoing_edges": outgoing, "owners": expected_owners})
        seen_owners.update(owners)
        seen_ranges.append((start, end))
    return plans


def native_function_partitions(raw, capture, function_rvas, specifications, evidence, pe):
    """Bind explicit local CALL/RET helpers to an exact partition of retained code."""
    digest = hashlib.sha256(raw).hexdigest()
    if (capture.get("input_sha256") != digest or capture.get("input_bytes") != len(raw)
            or capture.get("capture_complete") is not True or pe["architecture"] != "x64"
            or not evidence or not evidence.strip()):
        raise ValueError("Function partitions require exact complete x64 input and explicit assembly evidence")
    base = int(pe["image_base"], 16)
    existing = {int(row["rva"], 16): row for row in capture["functions"]}
    if len(existing) != len(capture["functions"]) or int(capture["image_base"], 16) != base:
        raise ValueError("Function partition capture boundaries or image base differ")
    result = ROOT / ".analysis/results/native" / digest

    def chunk(start, end):
        matches = [section for section in pe["sections"]
                   if 0 <= start - base - section["rva"]
                   and end - base - section["rva"] <= section["bytes"]]
        if end <= start or len(matches) != 1:
            raise ValueError("Function partition range is not uniquely file-backed")
        section = matches[0]
        at = section["file_offset"] + start - base - section["rva"]
        code = raw[at:at + end - start]
        if len(code) != end - start:
            raise ValueError("Function partition original bytes are truncated")
        return {"start": hex(start), "end": hex(end), "raw_hex": code.hex(),
                "sha256": hashlib.sha256(code).hexdigest()}

    partitions, owners = [], set()
    for specification in specifications:
        try:
            owner_text, separator, ranges_text = specification.partition("=")
            owner = int(owner_text, 0)
            ranges = [tuple(int(value, 0) + base for value in span.split(":"))
                      for span in ranges_text.split(",")]
        except (ValueError, TypeError) as error:
            raise ValueError("Function partition requires OWNER=START:END,START:END") from error
        if (not separator or owner not in function_rvas or owner in owners
                or not 1 <= len(ranges) <= 4096 or any(len(span) != 2 for span in ranges)):
            raise ValueError("Function partition requires unique selected owners and explicit helper ranges")
        owners.add(owner)
        row = existing.get(owner)
        if row is None or row.get("assembly_captured") is not True:
            raise ValueError("Function partition owner lacks complete retained assembly")
        path = resolve(rel(result / row["path"]), rel(result))
        retained = path.read_bytes()
        source = {"path": rel(path), "bytes": len(retained), "sha256": hashlib.sha256(retained).hexdigest()}
        if source["sha256"] != row["sha256"] or ("bytes" in row and row["bytes"] != len(retained)):
            raise ValueError("Function partition owner identity changed")
        body = json.loads(retained)
        start, end = base + owner, int(body["end"], 16)
        original = chunk(start, end)
        assembly = body.get("assembly")
        if (body.get("address") != hex(start) or body.get("rva") != hex(owner)
                or row.get("address") != hex(start) or body.get("chunks") != [original]
                or type(body.get("flags")) is not int or not isinstance(assembly, list) or not assembly
                or not isinstance(body.get("xrefs_to_entry"), list)):
            raise ValueError("Function partition owner requires one exact complete retained chunk")
        previous_end, by_address = start, {}
        for instruction in assembly:
            at, code = int(instruction["address"], 16), bytes.fromhex(instruction["bytes"])
            if (not code or at < previous_end or at < start or at + len(code) > end
                    or ("rva" in instruction and int(instruction["rva"], 16) != at - base)
                    or code.hex() != chunk(at, at + len(code))["raw_hex"]
                    or not isinstance(instruction.get("xrefs_from"), list)
                    or not isinstance(instruction.get("xrefs_to"), list)):
                raise ValueError("Function partition assembly or complete references differ from original code")
            by_address[at] = instruction
            previous_end = at + len(code)
        helpers, owner_chunks, previous = [], [], start
        for first, stop in ranges:
            if (not start < first < stop < end or first < previous
                    or first not in by_address or first - base in existing
                    or any(at < first < at + len(bytes.fromhex(ins["bytes"]))
                           or at < stop < at + len(bytes.fromhex(ins["bytes"]))
                           for at, ins in by_address.items())):
                raise ValueError("Function partition helper overlaps code, an existing boundary or another helper")
            instructions = [ins for at, ins in by_address.items() if first <= at < stop]
            if not instructions or int(instructions[-1]["address"], 16) + len(bytes.fromhex(instructions[-1]["bytes"])) != stop:
                raise ValueError("Function partition helper ends outside complete instruction boundaries")
            calls, returns = [], []
            for instruction in instructions:
                at, code = int(instruction["address"], 16), bytes.fromhex(instruction["bytes"])
                outgoing = [xref for xref in instruction["xrefs_from"] if xref.get("code") is True]
                if code == b"\xc3":
                    expected_targets = set()
                elif len(code) == 5 and code[0] == 0xe9:
                    expected_targets = {at + 5 + int.from_bytes(code[1:], "little", signed=True)}
                elif len(code) == 2 and code[0] == 0xeb:
                    expected_targets = {at + 2 + int.from_bytes(code[1:], "little", signed=True)}
                elif len(code) == 2 and (0x70 <= code[0] <= 0x7f or 0xe0 <= code[0] <= 0xe3):
                    expected_targets = {at + 2, at + 2 + int.from_bytes(code[1:], "little", signed=True)}
                elif len(code) == 6 and code[0] == 0x0f and 0x80 <= code[1] <= 0x8f:
                    expected_targets = {at + 6, at + 6 + int.from_bytes(code[2:], "little", signed=True)}
                else:
                    expected_targets = {at + len(code)}
                if {int(xref["to"], 16) for xref in outgoing} != expected_targets:
                    raise ValueError("Function partition helper references differ from its actual relative control flow")
                if any(xref.get("type") in {16, 17} or not first <= int(xref["to"], 16) < stop for xref in outgoing):
                    raise ValueError("Function partition helper has nested calls or a nonlocal control-flow edge")
                mnemonic = instruction.get("text", "").strip().split(" ", 1)[0].lower()
                if code == b"\xc3":
                    if outgoing:
                        raise ValueError("Function partition RET has an unexpected control-flow edge")
                    returns.append(hex(at - base))
                elif not outgoing or mnemonic.startswith(("ret", "call")):
                    raise ValueError("Function partition helper has an unproved terminal or call instruction")
                for xref in instruction["xrefs_to"]:
                    if xref.get("code") is not True:
                        if xref.get("type") != 0:
                            raise ValueError("Function partition helper has a data reference requiring separate review")
                        continue
                    caller = int(xref["from"], 16)
                    if first <= caller < stop:
                        continue
                    site = by_address.get(caller)
                    value = bytes.fromhex(site["bytes"]) if site else b""
                    if (at != first or xref.get("type") != 17 or len(value) != 5 or value[0] != 0xe8
                            or caller + 5 + int.from_bytes(value[1:], "little", signed=True) != first
                            or any(a <= caller < b for a, b in ranges)):
                        raise ValueError("Function partition helper has a nonlocal, non-CALL or interior entry")
                    calls.append({"rva": hex(caller - base), "target_rva": hex(first - base), "bytes": value.hex()})
            if not calls or not returns:
                raise ValueError("Function partition helper requires actual owner CALL edges and plain RET exits")
            owner_chunks.append(chunk(previous, first))
            helpers.append({"rva": hex(first - base), "expected_end": hex(stop),
                "expected_chunks": [chunk(first, stop)], "expected_xrefs_to_entry": by_address[first]["xrefs_to"],
                "call_edges": sorted(calls, key=lambda item: int(item["rva"], 16)), "return_rvas": returns})
            previous = stop
        owner_chunks.append(chunk(previous, end))
        # Every original byte, including padding omitted from the instruction list,
        # belongs to exactly one output owner chunk or a complete local helper.
        pieces = sorted(owner_chunks + [helper["expected_chunks"][0] for helper in helpers],
                        key=lambda item: int(item["start"], 16))
        if (pieces[0]["start"] != original["start"] or pieces[-1]["end"] != original["end"]
                or any(left["end"] != right["start"] for left, right in zip(pieces, pieces[1:]))
                or "".join(piece["raw_hex"] for piece in pieces) != original["raw_hex"]):
            raise ValueError("Function partition does not preserve the exact complete original byte union")
        partitions.append({"owner_rva": hex(owner), "source_function": source, "expected_flags": body["flags"],
            "original_chunks": [original], "owner_chunks": owner_chunks, "helpers": helpers,
            "assembly_evidence": evidence.strip()})
    return partitions


def native_function_partition_bodies(partition):
    """Derive exact owner/helper baselines without synthesizing instructions or ABI."""
    source = partition["source_function"]
    raw = resolve(source["path"], ".analysis/results/native").read_bytes()
    if len(raw) != source["bytes"] or hashlib.sha256(raw).hexdigest() != source["sha256"]:
        raise ValueError("Function partition source identity changed while composing baselines")
    original = json.loads(raw)

    def compose(chunks):
        body = dict(original)
        body["chunks"] = chunks
        body["end"] = chunks[0]["end"]
        body["assembly"] = [ins for ins in original["assembly"]
                            if any(int(c["start"], 16) <= int(ins["address"], 16) < int(c["end"], 16) for c in chunks)]
        return body

    owner, helpers = compose(partition["owner_chunks"]), {}
    base = int(original["address"], 16) - int(original["rva"], 16)
    for helper in partition["helpers"]:
        body = compose(helper["expected_chunks"])
        body.update(address=hex(base + int(helper["rva"], 16)), rva=helper["rva"],
                    xrefs_to_entry=helper["expected_xrefs_to_entry"], pseudocode=None)
        helpers[helper["rva"]] = body
    return {"owner": owner, "helpers": helpers}


def native_shared_tail_specifications(rows):
    """Return stable CLI forms for normalized shared-tail rows."""
    values = []
    for row in rows or ():
        if not isinstance(row, dict):
            raise ValueError("Shared function tail row must be an object")
        mode = "inline" if row.get("inline_owners") is True else "retain"
        owners = ",".join(str(value) for value in row.get("owner_rvas", ()))
        values.append("{}={}:{}:{}:{}".format(row.get("primary_owner_rva"), row.get("tail_start_rva"),
                                               row.get("tail_end_rva"), owners, mode))
    return values


def native_observation_evidence(specification):
    """Bind a static observation to retained review and observer source bytes."""
    if not isinstance(specification, str):
        raise ValueError("Static observation requires a retained review reference")
    name, marker, pointer = specification.partition("#")
    if Path(name).is_absolute() or not marker or not pointer.startswith("/"):
        raise ValueError("Static observation requires a project-relative review and JSON pointer")
    path = resolve(name, ".analysis/results")
    raw = path.read_bytes()
    value = json.loads(raw)
    try:
        for token in pointer[1:].split("/"):
            if re.search(r"~(?![01])", token):
                raise ValueError("Invalid JSON pointer escape")
            token = token.replace("~1", "/").replace("~0", "~")
            if isinstance(value, list):
                if not re.fullmatch(r"0|[1-9][0-9]*", token):
                    raise ValueError("Invalid JSON pointer array index")
                value = value[int(token)]
            else:
                value = value[token]
    except (KeyError, IndexError, TypeError, ValueError) as error:
        raise ValueError("Static observation review pointer does not resolve") from error
    if (not isinstance(value, dict) or set(value) != {"reason", "observer_source"}
            or not isinstance(value["reason"], str) or not value["reason"].strip()
            or len(value["reason"]) > 16384):
        raise ValueError("Static observation requires an explicit bounded reason and observer source")
    source = value["observer_source"]
    if (not isinstance(source, dict) or set(source) != {"path", "bytes", "sha256"}
            or not isinstance(source["path"], str) or Path(source["path"]).is_absolute()
            or Path(source["path"]).suffix != ".py"
            or type(source["bytes"]) is not int or source["bytes"] <= 0
            or not isinstance(source["sha256"], str)
            or not re.fullmatch(r"[0-9a-f]{64}", source["sha256"])):
        raise ValueError("Static observation observer source identity is incomplete")
    observer = resolve(source["path"], ".analysis/results")
    if observer.stat().st_size != source["bytes"] or sha(observer) != source["sha256"]:
        raise ValueError("Static observation observer source bytes changed")
    return {"review_source": {"path": rel(path), "bytes": len(raw),
                             "sha256": hashlib.sha256(raw).hexdigest(), "pointer": pointer},
            "reason": value["reason"], "observer_source": dict(source)}


def native_function_fresh_inlines(raw, capture, selection, specifications, evidence, pe):
    """Bind fresh SDK inlining to complete helpers and original direct CALLs."""
    leaf_scope = "reviewed_nested_register_leaf"
    values, leaf_values = [], set()
    for specification in specifications:
        value = specification
        if isinstance(value, dict):
            if (set(value) != {"rva", "inline_scope", "flag_names"}
                    or value.get("inline_scope") != leaf_scope
                    or value.get("flag_names") != ["INLINE_DONTCOPY"]):
                raise ValueError("Fresh inline leaf requires an explicit register-only SDK scope")
            value = value["rva"]
            value = int(value, 0) if isinstance(value, str) else value
            if type(value) is not int:
                raise ValueError("Fresh inline leaf requires an exact retained RVA")
            leaf_values.add(value)
        values.append(int(value, 0) if isinstance(value, str) else value)
    if (pe["architecture"] not in {"x86", "x64"} or not evidence or not evidence.strip()
            or not values or len(values) > 256 or len(set(values)) != len(values)
            or any(type(value) is not int or value < 0 for value in values)):
        raise ValueError("Fresh inlining requires unique reviewed x86/x64 helper boundaries")
    helpers = {row["rva"]: row for row in selection.get("function_outlines", [])}
    for shared in selection.get("function_shared_tails", []):
        if shared["inline_owners"]:
            helpers.update({owner["rva"]: owner for owner in shared["owners"]})
    if any(hex(value) not in helpers for value in values):
        raise ValueError("Fresh inlining requires an explicitly reviewed outlined helper")
    base = int(selection["image_base"], 16)
    result = ROOT / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
    existing = {row["rva"]: row for row in capture["functions"]}

    def retained_body(record):
        path = resolve(rel(result / record["path"]), rel(result))
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != record["sha256"] or ("bytes" in record and len(data) != record["bytes"]):
            raise ValueError("Fresh inline original function evidence changed")
        return json.loads(data), {"path": rel(path), "bytes": len(data), "sha256": record["sha256"]}

    def original_code(first, stop):
        sections = [s for s in pe["sections"] if 0 <= first - base - s["rva"]
                    and stop - base - s["rva"] <= s["bytes"]]
        if stop <= first or len(sections) != 1:
            raise ValueError("Fresh inline code is not uniquely file-backed")
        at = sections[0]["file_offset"] + first - base - sections[0]["rva"]
        return raw[at:at + stop - first]

    calls = {hex(value): [] for value in values if value not in leaf_values}
    parent_bodies = []

    def original_body(rva):
        if rva not in existing:
            raise ValueError("Fresh inline function is absent from its complete original capture")
        record = existing[rva]
        body, identity = retained_body(record)
        if (record.get("assembly_captured") is not True or body.get("rva") != rva
                or body.get("address") != hex(base + int(rva, 16))
                or not body.get("chunks") or not body.get("assembly")):
            raise ValueError("Fresh inline parent lacks complete retained assembly")
        for chunk in body["chunks"]:
            code = original_code(int(chunk["start"], 16), int(chunk["end"], 16))
            if code.hex() != chunk["raw_hex"] or hashlib.sha256(code).hexdigest() != chunk["sha256"]:
                raise ValueError("Fresh inline parent chunks differ from original PE bytes")
        for instruction in body["assembly"]:
            at, code = int(instruction["address"], 16), bytes.fromhex(instruction["bytes"])
            if (not code or original_code(at, at + len(code)) != code
                    or not any(int(c["start"], 16) <= at < at + len(code) <= int(c["end"], 16)
                               for c in body["chunks"])):
                raise ValueError("Fresh inline parent instruction differs from complete original chunks")
        return body, identity

    def original_calls(body, identity, destinations):
        found = []
        for instruction in body["assembly"]:
            at, code = int(instruction["address"], 16), bytes.fromhex(instruction["bytes"])
            if len(code) != 5 or code[0] != 0xe8:
                continue
            destination = at + 5 + int.from_bytes(code[1:], "little", signed=True)
            rva = hex(destination - base)
            if rva not in destinations:
                continue
            edges = [x for x in instruction.get("xrefs_from", []) if x.get("code") and x.get("type") in {16, 17}]
            if len(edges) != 1 or edges[0].get("to") != hex(destination) or edges[0].get("from") != hex(at):
                raise ValueError("Fresh inline direct CALL lacks its exact retained code edge")
            found.append({"parent_rva": body["rva"], "parent_function": identity,
                          "callsite_rva": hex(at - base), "target_rva": rva, "raw_hex": code.hex()})
        return found

    for target in selection["targets"]:
        body, identity = original_body(target["rva"])
        if body["address"] != target["address"]:
            raise ValueError("Fresh inline parent differs from its selected original address")
        parent_bodies.append(body)
        for call in original_calls(body, identity, calls):
            calls[call["target_rva"]].append(call)
    plans = []
    for value in values:
        rva = hex(value)
        if value in leaf_values:
            continue
        if not calls[rva] or len(calls[rva]) > 256:
            raise ValueError("Fresh inline helper requires a bounded original direct CALL in selected parents")
        helper = helpers[rva]
        body, identity = retained_body(existing[rva])
        expected_edges = {int(c["callsite_rva"], 16) + base for c in calls[rva]}
        retained_edges = {int(x["from"], 16) for x in body.get("xrefs_to_entry", [])
                          if x.get("code") and x.get("type") in {16, 17}
                          and any(any(int(c["start"], 16) <= int(x["from"], 16) < int(c["end"], 16)
                                      for c in parent["chunks"]) for parent in parent_bodies)}
        if retained_edges != expected_edges:
            raise ValueError("Fresh inline helper and parent CALL edges disagree")
        plans.append({"rva": rva, "source_function": identity, "expected_chunks": helper["expected_chunks"],
                      "callsites": calls[rva], "flag_names": ["INLINE_EXTFRAME", "INLINE_DONTCOPY"],
                      "assembly_evidence": evidence.strip()})
    for value in values:
        if value not in leaf_values:
            continue
        rva = hex(value)
        body, identity = original_body(rva)
        if (pe["architecture"] != "x64" or len(body["assembly"]) > 256
                or any(not isinstance(item.get("text"), str) or not item["text"].strip()
                       or re.search(r"\b(?:r(?:sp|bp)|e(?:sp|bp)|push\w*|pop\w*|enter|leave|call\w*)\b",
                                    item["text"], re.IGNORECASE)
                       or re.search(r"\bret(?:n|f)?\s+\S", item["text"], re.IGNORECASE)
                       for item in body["assembly"])
                or not any(re.fullmatch(r"\s*ret(?:n)?\s*", item["text"], re.IGNORECASE)
                           for item in body["assembly"])):
            raise ValueError("Fresh inline register leaf requires reviewed complete code without a stack frame or nested CALL")
        nested_calls, nested_parents = [], []
        for parent in plans:
            parent_body, parent_identity = original_body(parent["rva"])
            edges = original_calls(parent_body, parent_identity, {rva})
            if not edges:
                continue
            nested_parents.append(parent_body)
            for outer in parent["callsites"]:
                for inner in edges:
                    nested_calls.append({**inner, "owner_rva": outer["parent_rva"],
                                         "call_path": [dict(outer), dict(inner)]})
        if not nested_calls or len(nested_calls) > 256:
            raise ValueError("Fresh inline register leaf lacks a bounded explicitly reviewed outer helper CALL path")
        expected_edges = {int(call["callsite_rva"], 16) + base for call in nested_calls}
        retained_edges = {int(edge["from"], 16) for edge in body.get("xrefs_to_entry", [])
                          if edge.get("code") and edge.get("type") in {16, 17}
                          and any(any(int(chunk["start"], 16) <= int(edge["from"], 16) < int(chunk["end"], 16)
                                      for chunk in parent["chunks"]) for parent in nested_parents)}
        if retained_edges != expected_edges:
            raise ValueError("Fresh inline register leaf and its original parent CALL edges disagree")
        plans.append({"rva": rva, "source_function": identity, "expected_chunks": helpers[rva]["expected_chunks"],
                      "callsites": nested_calls, "inline_scope": leaf_scope, "flag_names": ["INLINE_DONTCOPY"],
                      "assembly_evidence": evidence.strip()})
    return plans


def native_function_selection(raw, capture, function_rvas, type_specs=(), type_note=None, max_function_kib=None,
                              import_specs=(), merge_specs=(), outline_rvas=(), partition_specs=(), shared_tail_specs=(),
                              observation_evidence=None, fresh_inline_rvas=(), callsite_reviews=(), reanalyze_type_owners=False,
                              propagate_direct_calls=False, boundary_reviews=(), instruction_reviews=(),
                              fresh_inline_register_leaf_rvas=(), decompiler_prepass_rvas=(), frame_reviews=(), switch_reviews=(), stack_reviews=(),
                              shared_tail_context_reviews=(), optimizer_property_reviews=(), recursive_context_reviews=()):
    """Select retained boundaries and explicit, assembly-reviewed type repairs."""
    from host import inspect_pe

    pe = inspect_pe(raw)
    if pe["kind"] not in {"native", "mixed_managed_native"}:
        raise ValueError("Function selection input is not a native PE image")
    if not function_rvas or len(function_rvas) > 4096 or len(set(function_rvas)) != len(function_rvas):
        raise ValueError("Function selection requires unique, bounded function RVAs")
    existing = {int(row["rva"], 16): row for row in capture["functions"]}
    types = []
    for spec in type_specs:
        rva_text, separator, declaration = spec.partition("=")
        if not separator or not declaration.strip() or not type_note or not type_note.strip():
            raise ValueError("Function type recovery requires RVA=DECLARATION and an assembly evidence note")
        rva = int(rva_text, 0)
        if rva not in function_rvas or any(row["rva"] == hex(rva) for row in types):
            raise ValueError("Type recovery must name a unique explicitly selected function boundary")
        types.append({"rva": hex(rva), "declaration": declaration.strip(), "assembly_evidence": type_note.strip()})
    base = int(pe["image_base"], 16)
    pe_offset = int.from_bytes(raw[0x3c:0x40], "little")
    section_table = pe_offset + 24 + int.from_bytes(raw[pe_offset + 20:pe_offset + 22], "little")
    targets = []
    for index, rva in enumerate(function_rvas):
        row = existing.get(rva)
        if row is None or int(row["address"], 16) != base + rva:
            raise ValueError("Function selection must use a retained exact function boundary: " + hex(rva))
        sections = [(number, section) for number, section in enumerate(pe["sections"])
                    if 0 <= rva - section["rva"] < section["bytes"]]
        if len(sections) != 1:
            raise ValueError("Function selection is not uniquely file-backed: " + hex(rva))
        number, section = sections[0]
        flags_at = section_table + number * 40 + 36
        if not int.from_bytes(raw[flags_at:flags_at + 4], "little") & 0x20000000:
            raise ValueError("Function selection is not in executable PE code: " + hex(rva))
        targets.append({"index": index, "address": hex(base + rva), "rva": hex(rva),
                        "file_offset": section["file_offset"] + rva - section["rva"]})
    selection = {"mode": "retained_function_reanalysis", "image_base": hex(base), "architecture": pe["architecture"],
                 "targets": targets, "type_recoveries": types}
    if recursive_context_reviews:
        if (type_specs or import_specs or merge_specs or outline_rvas or partition_specs or shared_tail_specs
                or observation_evidence or fresh_inline_rvas or callsite_reviews or reanalyze_type_owners
                or propagate_direct_calls or boundary_reviews or instruction_reviews
                or fresh_inline_register_leaf_rvas or decompiler_prepass_rvas or frame_reviews
                or switch_reviews or stack_reviews or shared_tail_context_reviews or optimizer_property_reviews):
            raise ValueError("Recursive root contexts require a separate exact reviewed supplement")
        import native_boundaries
        selection["recursive_fallthrough_contexts"] = native_boundaries.select_recursive_contexts(
            sys.modules[__name__], raw, capture, function_rvas, recursive_context_reviews)
    if shared_tail_context_reviews:
        if (import_specs or merge_specs or outline_rvas or partition_specs or shared_tail_specs
                or fresh_inline_rvas or callsite_reviews or reanalyze_type_owners or propagate_direct_calls
                or boundary_reviews or instruction_reviews or fresh_inline_register_leaf_rvas
                or decompiler_prepass_rvas or frame_reviews or switch_reviews or stack_reviews):
            raise ValueError("Existing shared-tail context requires its own exact typed supplement")
        import native_boundaries
        selection["shared_tail_contexts"] = native_boundaries.select_shared_tail_contexts(
            sys.modules[__name__], raw, capture, function_rvas, shared_tail_context_reviews, types)
    if observation_evidence is not None:
        selection["static_observation"] = native_observation_evidence(observation_evidence)
    if max_function_kib is not None:
        if type(max_function_kib) is not int or not 64 <= max_function_kib <= 1024:
            raise ValueError("Decompiler function size limit must be an integer from 64 to 1024 KiB")
        selection["decompiler_limits"] = {"max_function_kib": max_function_kib}
    if boundary_reviews:
        if (type_specs or import_specs or merge_specs or partition_specs or shared_tail_specs
                or fresh_inline_rvas or callsite_reviews or reanalyze_type_owners or propagate_direct_calls
                or instruction_reviews or fresh_inline_register_leaf_rvas or decompiler_prepass_rvas
                or frame_reviews or switch_reviews or stack_reviews or observation_evidence):
            raise ValueError("Exception-backed boundary recovery requires a separate supplement without ABI or inline repairs")
        import native_boundaries
        if pe["architecture"] == "x86":
            selection["x86_entry_partitions"] = native_boundaries.select_entry_partitions(
                sys.modules[__name__], raw, capture, function_rvas, boundary_reviews, outline_rvas)
            selection["decompiler_prepasses"] = native_boundaries.entry_partition_prepasses(
                sys.modules[__name__], raw, capture, function_rvas, selection["x86_entry_partitions"])
        else:
            if outline_rvas:
                raise ValueError("Exception-backed boundary recovery requires a separate supplement without outlined helpers")
            selection["function_boundary_recoveries"] = native_boundaries.select(
                sys.modules[__name__], raw, capture, function_rvas, boundary_reviews)
    if instruction_reviews:
        if (type_specs or import_specs or merge_specs or outline_rvas or partition_specs or shared_tail_specs
                or fresh_inline_rvas or callsite_reviews or reanalyze_type_owners or propagate_direct_calls
                or fresh_inline_register_leaf_rvas):
            raise ValueError("Original instruction recovery requires a separate supplement without ABI or boundary repairs")
        import native_boundaries
        selection["instruction_boundary_recoveries"] = native_boundaries.select_instructions(
            sys.modules[__name__], raw, capture, function_rvas, instruction_reviews)
    if import_specs:
        selection["import_type_recoveries"] = native_import_type_recoveries(raw, import_specs, type_note)
        selection["import_thunks"] = native_import_thunks(raw, capture, selection["import_type_recoveries"], pe)
    if merge_specs:
        selection["function_merges"] = native_function_merges(raw, capture, function_rvas, merge_specs, type_note, pe)
    if shared_tail_specs:
        if merge_specs or partition_specs:
            raise ValueError("Shared function tails cannot be combined with boundary merges or partitions")
        shared = native_function_shared_tails(raw, capture, function_rvas, shared_tail_specs, type_note, pe)
        shared_owners = {row["rva"] for plan in shared for row in plan["owners"]}
        if any(hex(int(value, 0) if isinstance(value, str) else value) in shared_owners for value in outline_rvas):
            raise ValueError("Shared function tail owners cannot also be ordinary outlined helpers")
        selection["function_shared_tails"] = shared
    if outline_rvas:
        if merge_specs:
            raise ValueError("Outlined helpers and boundary merges require separate reviewed supplements")
        selection["function_outlines"] = native_function_outlines(raw, capture, function_rvas, outline_rvas, type_note, pe)
    if partition_specs:
        if merge_specs or outline_rvas or type_specs or import_specs:
            raise ValueError("Function partitions require a separate reviewed supplement without ABI or boundary repairs")
        selection["function_partitions"] = native_function_partitions(raw, capture, function_rvas, partition_specs, type_note, pe)
    if fresh_inline_rvas or fresh_inline_register_leaf_rvas:
        selection["function_fresh_inlines"] = native_function_fresh_inlines(
            raw, capture, selection, [*fresh_inline_rvas,
                *({"rva": hex(int(value, 0) if isinstance(value, str) else value),
                   "inline_scope": "reviewed_nested_register_leaf", "flag_names": ["INLINE_DONTCOPY"]}
                  for value in fresh_inline_register_leaf_rvas)], type_note, pe)
    if callsite_reviews:
        if merge_specs or outline_rvas or partition_specs or shared_tail_specs or fresh_inline_rvas or fresh_inline_register_leaf_rvas:
            raise ValueError("Resolved callsite types require a separate supplement without boundary or inline changes")
        import native_callsites
        selection["callsite_type_recoveries"] = native_callsites.select(
            sys.modules[__name__], raw, capture, function_rvas, callsite_reviews)
    if type(reanalyze_type_owners) is not bool:
        raise ValueError("Automatic type-owner reanalysis must be an explicit boolean")
    if type(propagate_direct_calls) is not bool or (propagate_direct_calls and not reanalyze_type_owners):
        raise ValueError("Direct call type propagation requires explicit type-owner reanalysis")
    if reanalyze_type_owners:
        if (not (types or import_specs) or merge_specs or outline_rvas or partition_specs
                or shared_tail_specs or fresh_inline_rvas or callsite_reviews or fresh_inline_register_leaf_rvas):
            raise ValueError("Automatic type-owner reanalysis requires separate exact type repairs without boundary changes")
        import native_adjudication
        owners = []
        result = ROOT / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        for rva in function_rvas:
            record = existing[rva]
            path = resolve(rel(result / record["path"]), rel(result))
            body_raw = path.read_bytes()
            if hashlib.sha256(body_raw).hexdigest() != record["sha256"]:
                raise ValueError("Automatic type-owner reanalysis source function changed")
            body = json.loads(body_raw)
            native_adjudication.original_machine(raw, body)
            owners.append({"owner_rva": hex(rva), "owner_function": {"path": rel(path),
                "bytes": len(body_raw), "sha256": record["sha256"]}})
        selection["type_owner_reanalysis"] = owners
        if propagate_direct_calls:
            import native_callsites
            selection["direct_call_type_propagations"] = native_callsites.select_direct(
                sys.modules[__name__], raw, capture, owners, types)
    if frame_reviews:
        if (boundary_reviews or instruction_reviews or merge_specs or outline_rvas or partition_specs
                or shared_tail_specs or fresh_inline_rvas or callsite_reviews or fresh_inline_register_leaf_rvas
                or observation_evidence is None):
            raise ValueError("CPU frame reconstruction requires complete explicit observation without boundary or inline changes")
        import native_boundaries
        selection["frame_rebuilds"] = native_boundaries.select_frame_rebuilds(
            sys.modules[__name__], raw, capture, function_rvas, frame_reviews)
    if stack_reviews:
        if (boundary_reviews or instruction_reviews or merge_specs or outline_rvas or partition_specs
                or shared_tail_specs or fresh_inline_rvas or fresh_inline_register_leaf_rvas
                or callsite_reviews or frame_reviews or switch_reviews):
            raise ValueError("Derived stack points require separate complete owners without boundary changes")
        import native_boundaries
        selection["derived_stack_points"] = native_boundaries.select_stack_points(
            sys.modules[__name__], raw, capture, function_rvas, stack_reviews)
    if switch_reviews:
        if (type_specs or import_specs or boundary_reviews or instruction_reviews or merge_specs or outline_rvas
                or partition_specs or shared_tail_specs or fresh_inline_rvas or callsite_reviews
                or fresh_inline_register_leaf_rvas or reanalyze_type_owners or propagate_direct_calls
                or frame_reviews):
            raise ValueError("Switch metadata recovery requires a separate supplement without ABI, boundary or frame changes")
        import native_callsites
        selection["switch_metadata_recoveries"] = native_callsites.select_switches(
            sys.modules[__name__], raw, capture, function_rvas, switch_reviews)
    if decompiler_prepass_rvas:
        if boundary_reviews:
            raise ValueError("Decompiler prepass cannot change a selected owner's exception-backed boundary")
        prepass_rvas = [int(value, 0) if isinstance(value, str) else value for value in decompiler_prepass_rvas]
        changed_rvas = ({int(value, 16) for merge in selection.get("function_merges", [])
                         for value in merge["member_rvas"]}
            | {int(owner["rva"], 16) for shared in selection.get("function_shared_tails", []) for owner in shared["owners"]}
            | {int(helper["rva"], 16) for key in ("function_outlines", "function_fresh_inlines")
               for helper in selection.get(key, [])})
        if changed_rvas.intersection(prepass_rvas):
            raise ValueError("Decompiler prepass cannot change a selected owner's original boundary")
        import native_boundaries
        selection["decompiler_prepasses"] = native_boundaries.select_prepasses(
            sys.modules[__name__], raw, capture, function_rvas, prepass_rvas, type_note,
            selection.get("function_partitions", ()), selection.get("instruction_boundary_recoveries", ()))
    if optimizer_property_reviews:
        if (import_specs or merge_specs or partition_specs or observation_evidence
                or fresh_inline_rvas or callsite_reviews or reanalyze_type_owners or propagate_direct_calls
                or boundary_reviews or instruction_reviews or fresh_inline_register_leaf_rvas or frame_reviews
                or switch_reviews or stack_reviews or shared_tail_context_reviews or not outline_rvas):
            raise ValueError("Optimizer properties require exact outlined helpers and independently validated type/shared-tail context")
        import native_boundaries
        selection["optimizer_properties"] = native_boundaries.select_optimizer_properties(
            sys.modules[__name__], raw, capture, function_rvas, optimizer_property_reviews,
            selection["function_outlines"], selection.get("function_shared_tails", ()), selection["type_recoveries"])
    return selection


def native_supplement_request(source, capture_path, database, selection_key, selection, executable):
    """Bind a new private attempt to exact sources, analyzer and immutable evidence."""
    import tempfile

    def identity(path):
        return {"path": rel(path), "bytes": path.stat().st_size, "sha256": sha(path)}

    sources = {}
    for name, filename in {
        "orchestrator": "tools/analysis.py", "entry": "tools/workspace.py",
        "queue": "tools/reverse_pending.py", "supplement": "tools/ida/supplement.py",
        "capture": "tools/ida/capture.py", "capture_support": "tools/ida/capture_io.py",
        "serializer": "tools/ida/capture_values.py", "parser": "tools/host.py",
        "adjudication": "tools/native_adjudication.py", "boundaries": "tools/native_boundaries.py",
        "callsites": "tools/native_callsites.py",
    }.items():
        path = ROOT / filename
        raw = path.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        snapshot = ROOT / ".analysis/results/reverse-pending/tool-sources" / (digest + ".py")
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(prefix=".source-", suffix=".tmp", dir=snapshot.parent, delete=False) as stream:
                temporary = Path(stream.name)
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
            try:
                os.link(temporary, snapshot)
            except FileExistsError:
                if snapshot.read_bytes() != raw:
                    raise ValueError("Retained supplement tool source changed")
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
        sources[name] = {"path": filename, "bytes": len(raw), "sha256": digest,
                         "snapshot": identity(snapshot)}
    return {"schema_version": 1, "input": identity(source), "base_capture": identity(capture_path),
            "source_database": identity(database), "selection_key": selection_key, "selection": selection,
            "tool_sources": sources, "analyzer": {"installation_path": executable.name,
                "bytes": executable.stat().st_size, "sha256": sha(executable)}}


def native_supplement_request_sha256(request):
    return hashlib.sha256(json.dumps(request, sort_keys=True).encode()).hexdigest()


def native_supplement_attempt_name(name):
    """Recognize canonical attempt names and the source-bound version-1 format."""
    return isinstance(name, str) and re.fullmatch(r"[0-9a-f]{16}(?:-[0-9a-f]{16})?", name) is not None


def verify_native_supplement_request(plan, session, *, root=None):
    """Validate immutable attempt bindings while preserving legacy formal captures."""
    request = plan.get("analysis_request")
    if request is None:
        if any(key in plan or key in session for key in ("request_sha256", "attempt_id", "attempt_nonce", "tool_sources")):
            raise ValueError("Native supplement immutable attempt identity lost its source-bound request")
        return
    root = ROOT if root is None else root
    digest = native_supplement_request_sha256(request)
    attempt = plan.get("attempt_id")
    nonce = plan.get("attempt_nonce")
    if nonce is None:
        valid_attempt = isinstance(attempt, str) and re.fullmatch(digest[:16] + r"-[0-9a-f]{16}", attempt) is not None
    else:
        valid_attempt = (isinstance(nonce, str) and re.fullmatch(r"[0-9a-f]{32}", nonce) is not None
            and session.get("attempt_nonce") == nonce
            and attempt == hashlib.sha256((digest + nonce).encode()).hexdigest()[:16])
    if (request.get("schema_version") != 1 or plan.get("request_sha256") != digest
            or not valid_attempt
            or Path(plan["output"]).name != attempt or session.get("attempt_id") != attempt
            or session.get("request_sha256") != digest
            or plan.get(request.get("selection_key")) != request.get("selection")
            or request.get("input") != {"path": plan.get("input"), "bytes": plan.get("input_bytes"),
                                        "sha256": plan.get("sha256")}
            or any({key: request[field][key] for key in ("path", "sha256")} != plan.get(field)
                   for field in ("base_capture", "source_database"))
            or session.get("tool_sources") != request.get("tool_sources")
            or session.get("analyzer") != request["analyzer"]["installation_path"]
            or session.get("analyzer_sha256") != request["analyzer"]["sha256"]):
        raise ValueError("Native supplement immutable attempt identity differs")
    for record in request["tool_sources"].values():
        snapshot = record["snapshot"]
        path = (root / snapshot["path"]).resolve()
        boundary = (root / ".analysis/results/reverse-pending/tool-sources").resolve()
        if (not path.is_relative_to(boundary) or path == boundary
                or path.stat().st_size != record["bytes"] or sha(path) != record["sha256"]
                or snapshot["bytes"] != record["bytes"] or snapshot["sha256"] != record["sha256"]):
            raise ValueError("Native supplement retained tool snapshot changed")
    if session.get("passed") and session.get("source_identity_rechecked") is not True:
        raise ValueError("Native supplement did not recheck its exact source identities")


def supplement_native(args):
    """Supplement evidenced native entries in a private retained-IDB copy."""
    source = resolve(args.input, ".analysis/inputs")
    index = json.loads((ROOT / ".analysis/inputs.json").read_bytes())
    source_name = rel(source)
    row = next((item for item in index["inputs"] if item["path"] == source_name), None)
    if row is None or sha(source) != row["sha256"] or source.stat().st_size != row["bytes"]:
        raise ValueError("Supplement input differs from its original input receipt")
    if row.get("container_extraction_complete") is False or row.get("source_bytes_verified") is False:
        raise ValueError("Supplement input comes from failed static decompression")
    result = ROOT / ".analysis/results/native" / row["sha256"]
    capture_path = result / "capture.json"
    capture = json.loads(capture_path.read_bytes())
    if not capture.get("capture_complete") or capture["input_sha256"] != row["sha256"]:
        raise ValueError("Supplement requires the original complete full-function capture")
    function_rvas = getattr(args, "function_rva", None)
    if function_rvas:
        if args.pointer_table_rva is not None or args.pointer_count is not None:
            raise ValueError("Function reanalysis and pointer-table selection are separate modes")
        selection = native_function_selection(source.read_bytes(), capture, function_rvas,
            getattr(args, "function_type", None) or (), getattr(args, "type_evidence", None),
            getattr(args, "max_function_kib", None), getattr(args, "import_type", None) or (),
            getattr(args, "merge_functions", None) or (), getattr(args, "outline_function_rva", None) or (),
            getattr(args, "partition_functions", None) or (), getattr(args, "share_function_tail", None) or (),
            getattr(args, "observation_evidence", None), getattr(args, "fresh_inline_function_rva", None) or (),
            getattr(args, "callsite_type_review", None) or (), getattr(args, "reanalyze_type_owners", False),
            getattr(args, "propagate_direct_calls", False), getattr(args, "boundary_review", None) or (),
            getattr(args, "instruction_review", None) or (), getattr(args, "fresh_inline_register_leaf_rva", None) or (),
            getattr(args, "decompiler_prepass_rva", None) or (), getattr(args, "frame_review", None) or (),
            getattr(args, "switch_review", None) or (), getattr(args, "stack_review", None) or (),
            getattr(args, "shared_tail_context_review", None) or (), getattr(args, "optimizer_property_review", None) or (),
            getattr(args, "recursive_context_review", None) or ())
        observation = selection.get("static_observation")
        if observation and sha(ROOT / "tools/ida/supplement.py") != observation["observer_source"]["sha256"]:
            raise ValueError("Static observation tool differs from its reviewed source")
        selection_key = "function_selection"
    else:
        if getattr(args, "max_function_kib", None) is not None:
            raise ValueError("Decompiler size limit requires retained function reanalysis")
        if (getattr(args, "function_type", None) or getattr(args, "import_type", None) or getattr(args, "type_evidence", None)
                or getattr(args, "merge_functions", None) or getattr(args, "outline_function_rva", None)
                or getattr(args, "partition_functions", None) or getattr(args, "share_function_tail", None)
                or getattr(args, "observation_evidence", None) or getattr(args, "fresh_inline_function_rva", None)
                or getattr(args, "reanalyze_type_owners", False)
                or getattr(args, "propagate_direct_calls", False)
                or getattr(args, "boundary_review", None)
                or getattr(args, "instruction_review", None)
                or getattr(args, "fresh_inline_register_leaf_rva", None)
                or getattr(args, "decompiler_prepass_rva", None)
                or getattr(args, "frame_review", None)
                or getattr(args, "switch_review", None)
                or getattr(args, "stack_review", None)
                or getattr(args, "shared_tail_context_review", None)
                or getattr(args, "optimizer_property_review", None)
                or getattr(args, "recursive_context_review", None)
                or getattr(args, "callsite_type_review", None)):
            raise ValueError("Function type recovery requires explicit function reanalysis")
        if args.pointer_table_rva is None or args.pointer_count is None:
            raise ValueError("Pointer-table selection requires its RVA and count")
        selection = native_pointer_targets(source.read_bytes(), args.pointer_table_rva, args.pointer_count)
        selection_key = "pointer_table"
    existing = {}
    for record in capture["functions"]:
        path = resolve(rel(result / record["path"]), ".analysis/results/native")
        if sha(path) != record["sha256"]:
            raise ValueError("Original captured function changed: " + rel(path))
        existing[record["rva"]] = {**record, "path": rel(path)}
    # The fixed, original full-capture IDB is never edited or regenerated here.
    database = ROOT / ".work/native" / row["sha256"] / "batch" / (source.name + ".i64")
    if not database.is_file():
        raise ValueError("Retained private full-capture IDB is unavailable")
    executable = next((Path(args.ida_root) / name for name in ("idat64.exe", "idat.exe")
                       if (Path(args.ida_root) / name).is_file()), None)
    if executable is None:
        raise ValueError("Static IDA executable is unavailable")
    request = native_supplement_request(source, capture_path, database, selection_key, selection, executable)
    request_sha256 = native_supplement_request_sha256(request)
    attempt_nonce = uuid.uuid4().hex
    attempt_id = hashlib.sha256((request_sha256 + attempt_nonce).encode()).hexdigest()[:16]
    output = result / "supplements" / attempt_id
    scratch = ROOT / ".work/native" / row["sha256"] / "supplements" / attempt_id
    private_database = scratch / database.name
    session_path = output / "session.json"
    previous_attempts = []
    with index_lock(scratch.parent / ".reservation.lock"):
        for prior_path in sorted((result / "supplements").glob("*/session.json")):
            prior_plan = prior_path.parent / "plan.json"
            if not prior_plan.is_file():
                prior_data = json.loads(prior_path.read_bytes())
                if prior_data.get("selection") != selection:
                    continue
            elif json.loads(prior_plan.read_bytes()).get(selection_key) != selection:
                continue
            original_session = prior_path.read_bytes()
            prior = json.loads(original_session)
            record = {"session": {"path": rel(prior_path), "bytes": len(original_session),
                                   "sha256": hashlib.sha256(original_session).hexdigest()}}
            if not prior.get("finished_at_utc"):
                if not getattr(args, "recover_interrupted", False):
                    raise ValueError("Previous supplemental session has not finished; inspect it before rerunning")
                from reverse_pending import process_alive
                if process_alive(prior.get("pid") or prior.get("coordinator_pid")) is not False:
                    raise ValueError("Interrupted supplement process is still running or cannot be verified absent")
                record["interruption_recovery"] = {
                    "observed_inactive_at_utc": stamp(), "process_confirmed_absent": True,
                    "original_session_sha256": hashlib.sha256(original_session).hexdigest(),
                    "original_session_raw_base64": base64.b64encode(original_session).decode(),
                    "finished_at_utc": None, "exit_code": None}
            previous_attempts.append(record)
        output.mkdir(parents=True, exist_ok=False)
        scratch.mkdir(parents=True, exist_ok=False)
        session = {"pid": None, "coordinator_pid": os.getpid(), "started_at_utc": stamp(),
            "state": "reserved", "selection": selection, "attempt_id": attempt_id, "attempt_nonce": attempt_nonce,
            "request_sha256": request_sha256, "analyzer": executable.name,
            "analyzer_sha256": request["analyzer"]["sha256"], "tool_sources": request["tool_sources"],
            "capture_tool": {"path": "tools/ida/supplement.py",
                             "sha256": request["tool_sources"]["supplement"]["sha256"]},
            "previous_attempts": previous_attempts, "passed": False, "exit_code": None,
            "target_execution": False, "real_device_operations": False}
        save(session_path, session)

    def verify_sources():
        records = [request["input"], request["base_capture"], request["source_database"],
                   *request["tool_sources"].values(), *existing.values()]
        for record in records:
            path = ROOT / record["path"]
            if sha(path) != record["sha256"] or ("bytes" in record and path.stat().st_size != record["bytes"]):
                raise ValueError("Native supplement input or tool changed during its attempt: " + record["path"])
        if sha(executable) != request["analyzer"]["sha256"] or executable.stat().st_size != request["analyzer"]["bytes"]:
            raise ValueError("Native supplement analyzer changed during its attempt")

    env = os.environ.copy()
    env["IDAUSR"] = str(ROOT / "tools/ida/idausr")
    env.pop("RAZER_IDA_CAPTURE_PLAN", None)
    env["RAZER_IDA_SUPPLEMENT_PLAN"] = str(output / "plan.json")
    command = [str(executable), "-A", "-Opdb:off", "-L" + str(scratch / "ida.log"),
               "-S" + str(ROOT / "tools/ida/supplement.py"), str(private_database)]
    options = {}
    if os.name == "nt":
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        options = {"startupinfo": startup, "creationflags": subprocess.CREATE_NO_WINDOW}
    proc = None
    try:
        verify_sources()
        with database.open("rb") as original, private_database.open("xb") as private:
            shutil.copyfileobj(original, private)
        if sha(private_database) != request["source_database"]["sha256"]:
            raise ValueError("Private supplemental database copy differs")
        plan = {"schema_version": 1, "input": rel(source), "sha256": row["sha256"],
            "input_bytes": row["bytes"], "output": rel(output), "acquisition_binding": row,
            "database": {"path": rel(private_database), "sha256": sha(private_database)},
            "source_database": {key: request["source_database"][key] for key in ("path", "sha256")},
            "base_capture": {key: request["base_capture"][key] for key in ("path", "sha256")},
            "analysis_request": request, "request_sha256": request_sha256, "attempt_id": attempt_id,
            "attempt_nonce": attempt_nonce,
            selection_key: selection, "existing_functions": existing,
            "target_execution": False, "real_device_operations": False}
        save(output / "plan.json", plan)
        session["database"] = plan["database"]
        session["argv"] = [executable.name, "-A", "-Opdb:off", "-L" + rel(scratch / "ida.log"),
                           "-Stools/ida/supplement.py", rel(private_database)]
        verify_sources()
        with (scratch / "process.log").open("xb") as log:
            proc = subprocess.Popen(command, cwd=scratch, env=env, stdout=log,
                                    stderr=subprocess.STDOUT, **options)
        session.update(pid=proc.pid, state="running")
        save(session_path, session)
        print(json.dumps({"pid": proc.pid, "result": rel(output), "log": rel(scratch / "ida.log")}), flush=True)
        session["exit_code"] = proc.wait()
        session["process_released"] = True
        verify_sources()
        session["source_identity_rechecked"] = True
        manifest_path = output / "capture.json"
        session["passed"] = (session["exit_code"] == 0 and manifest_path.is_file()
                             and json.loads(manifest_path.read_bytes()).get("capture_complete", False))
        session["state"] = "finished"
    except BaseException as error:
        session.update(state="cancelled" if isinstance(error, (KeyboardInterrupt, SystemExit)) else "failed",
                       error=type(error).__name__ + ": " + str(error), passed=False)
        raise
    finally:
        if proc is not None and session["exit_code"] is None:
            try:
                proc.terminate()
                try:
                    session["exit_code"] = proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    session["exit_code"] = proc.wait(timeout=10)
                session["process_released"] = True
            except BaseException as error:
                session.update(state="cleanup_failed", cleanup_error=type(error).__name__ + ": " + str(error),
                               process_released=False, passed=False)
        if proc is None or session.get("process_released") is True:
            session["finished_at_utc"] = stamp()
        if private_database.is_file():
            session["retained_private_database"] = {"path": rel(private_database),
                "bytes": private_database.stat().st_size, "sha256": sha(private_database),
                "reason": "Retain until formal evidence validation and unified dependency-aware cleanup"}
        save(session_path, session)
    if not getattr(args, "no_refresh_gaps", False):
        refresh_gaps()
    print(json.dumps({"session": rel(session_path), "passed": session["passed"]}), flush=True)
    return session


def verify_native_typed_serializer(manifest):
    """Bind typed SDK observations to the exact retained serializer source."""
    record = manifest.get("typed_value_serializer")
    if record is None:
        return None
    if (not isinstance(record, dict) or set(record) != {"path", "bytes", "sha256"}
            or record.get("path") != "tools/ida/capture_values.py"
            or type(record.get("bytes")) is not int or record["bytes"] <= 0
            or not isinstance(record.get("sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", record["sha256"])):
        raise ValueError("Native typed serializer source identity is incomplete")
    retained = ROOT / ".analysis/results/reverse-pending/tool-sources" / (record["sha256"] + ".py")
    with index_lock(ROOT / ".analysis/native-serializer-source.lock"):
        source = retained if retained.is_file() else resolve(record["path"], "tools/ida")
        raw = source.read_bytes()
        if len(raw) != record["bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]:
            raise ValueError("Native typed serializer source bytes changed")
        if not retained.is_file():
            retained.parent.mkdir(parents=True, exist_ok=True)
            retained.write_bytes(raw)
    return {"path": rel(retained), "bytes": len(raw), "sha256": record["sha256"]}


def verify_nested_inline_mapping(selection, owner_rva, expected, actual_call, actual_calls):
    """Bind a nested CALL to the SDK's exact newly inserted outer block range."""
    base = int(selection["image_base"], 16)
    call_ea = actual_call.get("sdk_call_ea")
    original = hex(base + int(expected["callsite_rva"], 16))
    if (actual_call.get("mapping_api") != "mba_t.map_fict_ea"
            or actual_call.get("mapping_status") != "api_returned"
            or not isinstance(call_ea, str) or not re.fullmatch(r"0x[0-9a-f]+", call_ea)
            or hex(int(call_ea, 16)) != call_ea or actual_call.get("mapped_original_call_ea") != original
            or type(actual_call.get("fictitious_call")) is not bool
            or actual_call["fictitious_call"] != (call_ea != original)):
        raise ValueError("Native nested inline omitted its exact SDK instruction mapping")
    path = expected.get("call_path")
    if not isinstance(path, list) or len(path) != 2:
        raise ValueError("Native nested inline lacks its complete original CALL path")
    outer = path[0]
    parents = [call for call in actual_calls if all(call.get(key) == value for key, value in outer.items())]
    if len(parents) != 1:
        raise ValueError("Native nested inline lacks a unique reviewed outer CALL operation")
    parent = parents[0]
    scope = actual_call.get("source_inline_scope")
    if (not isinstance(scope, dict) or scope != parent.get("new_block_scope")
            or set(scope) != {"api", "mba_entry_ea", "source_block", "helper_ea", "inserted_block_interval"}
            or scope.get("api") != "Hexrays_Hooks.inlined_func"
            or scope.get("mba_entry_ea") != hex(base + int(owner_rva, 16))
            or type(scope.get("source_block")) is not int or scope["source_block"] < 0
            or scope["source_block"] != parent.get("block_serial_before")
            or scope.get("helper_ea") != hex(base + int(outer["target_rva"], 16))):
        raise ValueError("Native nested inline differs from its exact SDK outer insertion scope")
    interval = scope.get("inserted_block_interval")
    block = actual_call.get("sdk_block_serial")
    if (not isinstance(interval, list) or len(interval) != 2 or any(type(value) is not int for value in interval)
            or not 0 <= interval[0] < interval[1] or type(block) is not int
            or not interval[0] <= block < interval[1] or actual_call.get("block_serial_before") != block):
        raise ValueError("Native nested inline CALL is outside the SDK's exact inserted block range")


def native_reanalysis_resolution(result, capture=None, *, adjudication_record=None, attempt_adjudication_record=None):
    """Verify retained-function supplements and separate recovered decompilations."""
    capture_path = result / "capture.json"
    capture = capture if capture is not None else json.loads(capture_path.read_bytes())
    resolution = {"base_capture": {"path": rel(capture_path), "bytes": capture_path.stat().st_size,
                                   "sha256": sha(capture_path)},
                  "supplements": [], "resolved_failures": [], "remaining_failures": list(capture["failures"]),
                  "verification_failures": [], "complete_functions": {}, "conflicting_function_rvas": [],
                  "conflicting_functions": {}, "complete_contexts": {}, "conflicting_contexts": {},
                  "target_execution": False}
    candidates = sorted((result / "supplements").glob("*/capture.json"))
    if not candidates:
        import native_embedded
        native_embedded.apply_resolution(sys.modules[__name__], result, resolution, capture)
        import native_import_forwarders
        native_import_forwarders.apply_resolution(sys.modules[__name__], result, resolution, capture)
        import native_static_reviews
        native_static_reviews.apply_resolution(sys.modules[__name__], result, resolution, capture)
        return resolution
    original = resolve(capture["input"], ".analysis/inputs")
    raw = None
    base_functions = {row["rva"]: row for row in capture["functions"]}
    function_sources = {}
    verified_candidates = defaultdict(list)

    def checked(path, record):
        path = resolve(rel(path), rel(result))
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != record["sha256"] or ("bytes" in record and len(data) != record["bytes"]):
            raise ValueError("Native supplement evidence changed: " + rel(path))
        return json.loads(data)

    for path in candidates:
        try:
            supplement = json.loads(path.read_bytes())
            selection = supplement.get("function_selection")
            if selection is None:
                continue
            import native_adjudication
            finalized = native_adjudication.finalized_export(sys.modules[__name__], result, path)
            if (supplement.get("input_sha256") != capture["input_sha256"]
                    or supplement.get("base_capture") != {"path": rel(capture_path), "sha256": sha(capture_path)}
                    or (not finalized and supplement.get("capture_complete") is not True)
                    or (not finalized and supplement.get("all_pseudocode_captured") is not True) or supplement.get("failures")
                    or supplement.get("database_integrity", {}).get("verified") is not True
                    or (not finalized and supplement.get("base_capture_unchanged") is not True)
                    or (not finalized and supplement.get("base_database_unchanged") is not True)
                    or supplement.get("target_execution") is not False):
                raise ValueError("Native function supplement is incomplete, changed or lacks static integrity evidence")
            session_path = path.parent / "session.json"
            session = json.loads(session_path.read_bytes())
            if ((not finalized and (session.get("passed") is not True or session.get("exit_code") != 0))
                    or session.get("capture_tool") != supplement.get("capture_tool")):
                raise ValueError("Native supplement does not have a matching successful analyzer session")
            verify_native_typed_serializer(supplement)
            plan = json.loads((path.parent / "plan.json").read_bytes())
            if (plan.get("function_selection") != selection or plan.get("base_capture") != supplement["base_capture"]
                    or plan.get("source_database") != supplement.get("source_database")):
                raise ValueError("Native supplement differs from its exact static analysis plan")
            verify_native_supplement_request(plan, session)
            if raw is None:
                raw = original.read_bytes()
                if hashlib.sha256(raw).hexdigest() != capture["input_sha256"] or len(raw) != capture["input_bytes"]:
                    raise ValueError("Native supplement original bytes changed")
            recoveries = selection["type_recoveries"]
            imports = selection.get("import_type_recoveries", [])
            merges = selection.get("function_merges", [])
            outlines = selection.get("function_outlines", [])
            partitions = selection.get("function_partitions", [])
            shared_tails = selection.get("function_shared_tails", [])
            fresh_inlines = selection.get("function_fresh_inlines", [])
            callsite_types = selection.get("callsite_type_recoveries", [])
            boundary_recoveries = selection.get("function_boundary_recoveries", [])
            entry_partitions = selection.get("x86_entry_partitions", [])
            instruction_recoveries = selection.get("instruction_boundary_recoveries", [])
            prepasses = selection.get("decompiler_prepasses", [])
            frame_rebuilds = selection.get("frame_rebuilds", [])
            derived_stack_plans = selection.get("derived_stack_points", [])
            switch_recoveries = selection.get("switch_metadata_recoveries", [])
            tail_contexts = selection.get("shared_tail_contexts", [])
            recursive_contexts = selection.get("recursive_fallthrough_contexts", [])
            shared_notes = recoveries or imports or merges or outlines or partitions or shared_tails or prepasses
            actual = native_function_selection(raw, capture, [int(row["rva"], 16) for row in selection["targets"]],
                [row["rva"] + "=" + row["declaration"] for row in recoveries],
                shared_notes[0].get("assembly_evidence", shared_notes[0].get("reason")) if shared_notes else None,
                selection.get("decompiler_limits", {}).get("max_function_kib"),
                [row["rva"] + "=" + row["declaration"] for row in imports],
                [row["owner_rva"] + "=" + ",".join(row["member_rvas"]) for row in merges],
                [row["rva"] for row in outlines],
                [row["owner_rva"] + "=" + ",".join(helper["rva"] + ":" +
                    hex(int(helper["expected_end"], 16) - int(selection["image_base"], 16))
                    for helper in row["helpers"]) for row in partitions],
                [dict(row) for row in shared_tails],
                (selection["static_observation"]["review_source"]["path"] + "#"
                 + selection["static_observation"]["review_source"]["pointer"])
                if "static_observation" in selection else None,
                fresh_inline_rvas=[row["rva"] for row in fresh_inlines
                                   if row.get("inline_scope") != "reviewed_nested_register_leaf"],
                fresh_inline_register_leaf_rvas=[row["rva"] for row in fresh_inlines
                                                 if row.get("inline_scope") == "reviewed_nested_register_leaf"],
                callsite_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"]
                                  for row in callsite_types],
                reanalyze_type_owners=bool(selection.get("type_owner_reanalysis")),
                propagate_direct_calls="direct_call_type_propagations" in selection,
                boundary_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"]
                                  for row in [*boundary_recoveries, *entry_partitions]],
                instruction_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"]
                                     for row in instruction_recoveries],
                decompiler_prepass_rvas=[row["rva"] for row in prepasses] if not entry_partitions else (),
                frame_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"]
                               for row in frame_rebuilds],
                switch_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"]
                                 for row in switch_recoveries],
                stack_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"]
                               for row in derived_stack_plans],
                shared_tail_context_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"]
                                             for row in tail_contexts],
                optimizer_property_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"]
                                            for row in selection.get("optimizer_properties", [])],
                recursive_context_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"]
                                           for row in recursive_contexts])
            if actual != selection:
                raise ValueError("Native supplement selection no longer matches the original bytes")
            if tail_contexts:
                import native_boundaries
                native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
                native_boundaries.verify_shared_tail_contexts(sys.modules[__name__], tail_contexts,
                    supplement.get("shared_tail_context_recoveries"))
                for context in supplement["shared_tail_context_recoveries"]:
                    record = next((row for row in supplement["functions"]
                                   if row["rva"] == context["request"]["owner_rva"]), None)
                    if (record is None or context.get("capture") != {
                            "path": rel(path.parent / record["path"]),
                            "bytes": (path.parent / record["path"]).stat().st_size,
                            "sha256": record["sha256"]}):
                        raise ValueError("Native shared-tail context differs from its exact owner capture")
            elif supplement.get("shared_tail_context_recoveries"):
                raise ValueError("Native supplement has unselected existing-tail context changes")
            if derived_stack_plans:
                import native_boundaries
                native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
                native_boundaries.verify_stack_points(sys.modules[__name__], derived_stack_plans,
                                                     supplement.get("derived_stack_points"))
            elif supplement.get("derived_stack_points"):
                raise ValueError("Native supplement has unselected derived stack operations")
            if frame_rebuilds:
                import native_boundaries
                native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
                native_boundaries.verify_frame_rebuilds(sys.modules[__name__], frame_rebuilds,
                                                       supplement.get("frame_rebuilds"))
            elif supplement.get("frame_rebuilds"):
                raise ValueError("Native supplement contains an unreviewed CPU frame reconstruction")
            if switch_recoveries:
                import native_callsites
                native_callsites.retain_source(sys.modules[__name__], supplement.get("callsite_type_validator"))
                native_callsites.verify_switches(sys.modules[__name__], switch_recoveries,
                                                supplement.get("switch_metadata_recoveries"))
            elif supplement.get("switch_metadata_recoveries"):
                raise ValueError("Native supplement contains an unreviewed switch metadata recovery")
            observation = selection.get("static_observation")
            if observation and supplement.get("capture_tool", {}).get("sha256") != observation["observer_source"]["sha256"]:
                raise ValueError("Native supplement did not use its reviewed static observer")
            if shared_tails:
                retained_database = plan.get("source_database")
                if (not isinstance(retained_database, dict)
                        or not isinstance(retained_database.get("path"), str)
                        or not isinstance(retained_database.get("sha256"), str)
                        or sha(resolve(retained_database["path"], ".work/native")) != retained_database["sha256"]):
                    raise ValueError("Native shared-tail source database changed or is unavailable")
            if "decompiler_limits" in selection and supplement.get("applied_decompiler_limits") != selection["decompiler_limits"]:
                raise ValueError("Native supplement did not apply its exact decompiler size limit")
            if callsite_types:
                import native_callsites
                native_callsites.retain_source(sys.modules[__name__], supplement.get("callsite_type_validator"))
                native_callsites.verify_applied(callsite_types, supplement.get("callsite_type_recoveries"))
                if "callsite_automatic_reanalysis" in supplement:
                    native_callsites.verify_reanalysis(sys.modules[__name__], callsite_types,
                                                       supplement["callsite_automatic_reanalysis"])
            elif supplement.get("callsite_type_recoveries"):
                raise ValueError("Native supplement contains unreviewed callsite type changes")
            if selection.get("type_owner_reanalysis"):
                import native_callsites
                native_callsites.retain_source(sys.modules[__name__], supplement.get("callsite_type_validator"))
                native_callsites.verify_reanalysis(sys.modules[__name__], selection["type_owner_reanalysis"],
                                                   supplement.get("type_owner_automatic_reanalysis"))
                if any(not row.get("analysis_invalidation") for row in supplement["type_owner_automatic_reanalysis"]):
                    raise ValueError("Type-owner reanalysis omitted explicit derived-stack invalidation")
                direct = selection.get("direct_call_type_propagations", [])
                native_callsites.verify_direct_applied(direct, supplement.get("direct_call_type_propagations", []))
            elif "type_owner_automatic_reanalysis" in supplement:
                raise ValueError("Native supplement contains unreviewed type-owner reanalysis")
            elif supplement.get("direct_call_type_propagations"):
                raise ValueError("Native supplement contains unreviewed direct call type propagation")
            boundary_baselines, verified_boundary_helpers = {}, []
            entry_baselines, verified_entry_helpers = {}, []
            if entry_partitions:
                import native_boundaries
                native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
                native_boundaries.verify_entry_partitions(sys.modules[__name__], entry_partitions,
                    supplement.get("x86_entry_partition_recoveries"))
                entry_baselines = {row["owner_rva"]: native_boundaries.entry_partition_bodies(
                    sys.modules[__name__], row)[0] for row in entry_partitions}
                verified_entry_helpers = native_boundaries.verify_entry_helpers(sys.modules[__name__], raw,
                    entry_partitions, supplement.get("entry_partition_helpers"), path.parent)
            elif supplement.get("x86_entry_partition_recoveries") or supplement.get("entry_partition_helpers"):
                raise ValueError("Native supplement contains unreviewed x86 entry partitions")
            if boundary_recoveries:
                import native_boundaries
                native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
                native_boundaries.verify_applied(sys.modules[__name__], boundary_recoveries,
                                                supplement.get("function_boundary_recoveries"))
                boundary_baselines = {owner["owner_rva"]: native_boundaries.owner_body(
                    sys.modules[__name__], row, owner["owner_rva"])
                    for row in boundary_recoveries for owner in row["shared_owners"]}
                helpers = {row["helper"]["rva"]: native_boundaries.helper_body(row) for row in boundary_recoveries}
                records = supplement.get("boundary_helpers", [])
                if (len(records) != len(helpers) or {row.get("rva") for row in records} != set(helpers)):
                    raise ValueError("Native boundary recovery omitted complete helper assembly")
                for row in records:
                    record = {**row, "path": rel(path.parent / row["path"])}
                    body = checked(ROOT / record["path"], record)
                    baseline = helpers[row["rva"]]
                    if (row.get("assembly_captured") is not True or row.get("pseudocode_captured") is not False
                            or body.get("pseudocode") is not None
                            or any(body.get(key) != baseline.get(key) for key in ("address", "rva", "end", "chunks"))
                            or [(ins["address"], ins["bytes"]) for ins in body.get("assembly", [])]
                            != [(ins["address"], ins["bytes"]) for ins in baseline.get("assembly", [])]):
                        raise ValueError("Native recovered helper omits original instructions or differs from its complete exception chain")
                    verified_boundary_helpers.append(record)
            elif supplement.get("function_boundary_recoveries") or supplement.get("boundary_helpers"):
                raise ValueError("Native supplement contains unreviewed function boundary changes")
            instruction_baselines = {}
            if instruction_recoveries:
                import native_boundaries
                native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
                native_boundaries.verify_instructions(sys.modules[__name__], instruction_recoveries,
                                                     supplement.get("instruction_boundary_recoveries"))
                instruction_baselines = {row["owner_rva"]: native_boundaries.instruction_body(
                    sys.modules[__name__], row) for row in instruction_recoveries}
            elif supplement.get("instruction_boundary_recoveries"):
                raise ValueError("Native supplement contains unreviewed instruction decoding changes")
            if (len(supplement["functions"]) != len(selection["targets"])
                    or {row["rva"] for row in supplement["functions"]} != {row["rva"] for row in selection["targets"]}):
                raise ValueError("Native supplement omitted selected function boundaries")
            if (len(supplement["type_recoveries"]) != len(recoveries)
                    or any(any(applied.get(key) != requested[key] for key in requested)
                           or not applied.get("applied_type") or applied.get("base_database_modified") is not False
                           for applied, requested in zip(supplement["type_recoveries"], recoveries))):
                raise ValueError("Native supplement did not retain the exact reviewed type recoveries")
            if (len(supplement.get("import_type_recoveries", [])) != len(imports)
                    or any(any(applied.get(key) != requested[key] for key in requested)
                           or not applied.get("applied_type") or applied.get("base_database_modified") is not False
                           or not native_import_binding_matches(requested, applied.get("database_import_binding", {}))
                           for applied, requested in zip(supplement.get("import_type_recoveries", []), imports))):
                raise ValueError("Native supplement did not retain the exact reviewed import types")
            thunks = selection.get("import_thunks", [])
            applied_thunks = supplement.get("import_thunk_type_recoveries", [])
            if (len(applied_thunks) != len(thunks)
                    or any(any(applied.get(key) != requested[key] for key in requested)
                           or not isinstance(applied.get("applied_type"), str) or not applied["applied_type"].strip()
                           or applied.get("base_database_modified") is not False
                           for applied, requested in zip(applied_thunks, thunks))):
                raise ValueError("Native supplement did not retain the exact reviewed import thunk types")
            applied_merges = supplement.get("function_merge_recoveries", [])
            if (len(applied_merges) != len(merges)
                    or any(any(applied.get(key) != requested[key] for key in requested)
                           or applied.get("base_database_modified") is not False
                           or applied.get("applied_chunks") != requested["expected_chunks"]
                           for applied, requested in zip(applied_merges, merges))):
                raise ValueError("Native supplement did not retain the exact reviewed function merges")
            if "function_merge_automatic_reanalysis" in supplement:
                if not merges:
                    raise ValueError("Native supplement contains unreviewed merged-owner reanalysis")
                import native_boundaries
                native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
                native_boundaries.verify_merge_reanalysis(sys.modules[__name__], merges,
                                                         supplement["function_merge_automatic_reanalysis"])
            if prepasses:
                import native_boundaries
                native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
                native_boundaries.verify_prepasses(sys.modules[__name__], prepasses,
                                                   supplement.get("decompiler_prepasses"), path.parent)
            elif "decompiler_prepasses" in supplement:
                raise ValueError("Native supplement contains an unreviewed decompiler prepass")
            merged_baselines = {merge["owner_rva"]: native_function_merge_body(merge) for merge in merges}
            shared_baselines, shared_helpers = {}, {}
            for shared in shared_tails:
                tail = shared["expected_tail"]
                tail_start, tail_end = int(tail["start"], 16), int(tail["end"], 16)
                tail_assembly = []
                primary = shared["primary_owner_rva"]
                owners = {row["rva"]: row for row in shared["owners"]}
                primary_source = checked(ROOT / owners[primary]["source_function"]["path"],
                                         owners[primary]["source_function"])
                tail_assembly = [instruction for instruction in primary_source["assembly"]
                                 if tail_start <= int(instruction["address"], 16) < tail_end]
                for owner in shared["owners"]:
                    source = checked(ROOT / owner["source_function"]["path"], owner["source_function"])
                    body = dict(source)
                    body["chunks"] = owner["expected_chunks"]
                    body["end"] = owner["expected_end"]
                    if owner["rva"] != primary:
                        existing_addresses = {int(item["address"], 16) for item in source["assembly"]}
                        body["assembly"] = sorted(source["assembly"] + [item for item in tail_assembly
                            if int(item["address"], 16) not in existing_addresses], key=lambda item: int(item["address"], 16))
                    shared_baselines[owner["rva"]] = body
                    if owner["rva"] not in {target["rva"] for target in selection["targets"]}:
                        shared_helpers[owner["rva"]] = body
            applied_shared = supplement.get("function_shared_tail_recoveries", [])
            if len(applied_shared) != len(shared_tails):
                raise ValueError("Native supplement omitted exact reviewed shared function tails")
            for applied, requested in zip(applied_shared, shared_tails):
                if (any(applied.get(key) != value for key, value in requested.items())
                        or applied.get("base_database_modified") is not False
                        or applied.get("applied_tail") != requested["expected_tail"]
                        or applied.get("applied_primary_owner_rva") != requested["primary_owner_rva"]
                        or applied.get("applied_owner_rvas") != requested["owner_rvas"]
                        or len(applied.get("applied_owners", [])) != len(requested["owners"])):
                    raise ValueError("Native supplement shared-tail ownership differs from exact original evidence")
                for actual_owner, owner in zip(applied["applied_owners"], requested["owners"]):
                    expected_flags = owner["expected_flags"]
                    outline_flag = actual_owner.get("outline_flag")
                    if (type(outline_flag) is not int or outline_flag <= 0
                            or outline_flag & (outline_flag - 1)):
                        raise ValueError("Native supplement shared-tail outline flag differs")
                    expected_applied = expected_flags | outline_flag if requested["inline_owners"] else expected_flags
                    frame_keys = {"frame_id", "local_bytes", "saved_register_bytes", "argument_bytes", "frame_pointer_delta"}
                    frame = actual_owner.get("previous_frame")
                    if (not isinstance(frame, dict) or set(frame) != frame_keys
                            or any(type(value) is not int or value < 0 for value in frame.values())
                            or actual_owner.get("applied_frame") != frame
                            or (actual_owner.get("previous_type") is not None
                                and not isinstance(actual_owner["previous_type"], str))
                            or "previous_type" not in actual_owner or "applied_type" not in actual_owner
                            or actual_owner["applied_type"] != actual_owner["previous_type"]):
                        raise ValueError("Native supplement shared-tail operation changed its original frame or type")
                    def stack_points(field):
                        rows, points = actual_owner.get(field), {}
                        if not isinstance(rows, list):
                            raise ValueError("Native supplement shared-tail stack points are missing")
                        for point in rows:
                            if (not isinstance(point, dict) or set(point) != {"address", "delta"}
                                    or not isinstance(point["address"], str) or type(point["delta"]) is not int):
                                raise ValueError("Native supplement shared-tail stack point is invalid")
                            address = int(point["address"], 16)
                            if (point["address"] != hex(address) or address < int(selection["image_base"], 16)
                                    or address in points):
                                raise ValueError("Native supplement shared-tail stack point address is invalid or duplicated")
                            points[address] = point["delta"]
                        return points
                    previous_points = stack_points("previous_stack_points")
                    applied_points = stack_points("applied_stack_points")
                    if (any(applied_points.get(address) != delta for address, delta in previous_points.items())
                            or any(not int(requested["expected_tail"]["start"], 16) <= address
                                       <= int(requested["expected_tail"]["end"], 16)
                                   for address in applied_points if address not in previous_points)):
                        raise ValueError("Native supplement shared-tail operation changed original stack points or unrelated code")
                    if (actual_owner.get("rva") != owner["rva"]
                            or actual_owner.get("chunks") != owner["expected_chunks"]
                            or actual_owner.get("end") != owner["expected_end"]
                            or actual_owner.get("base_database_modified") is not False
                            or type(actual_owner.get("previous_flags")) is not int
                            or actual_owner.get("previous_flags") != expected_flags
                            or type(outline_flag) is not int or outline_flag <= 0
                            or outline_flag & (outline_flag - 1)
                            or actual_owner.get("applied_flags") != expected_applied):
                        raise ValueError("Native supplement shared-tail owner chunks or flags differ")
                    shared_baselines[owner["rva"]]["flags"] = expected_applied
            shared_helper_records = supplement.get("shared_tail_helpers", [])
            if (len(shared_helper_records) != len(shared_helpers)
                    or {row.get("rva") for row in shared_helper_records} != set(shared_helpers)):
                raise ValueError("Native supplement omitted complete shared-tail helper assembly")
            verified_shared_helpers = []
            for row in shared_helper_records:
                record = {**row, "path": rel(path.parent / row["path"])}
                body = checked(ROOT / record["path"], record)
                baseline = shared_helpers[row["rva"]]
                if (row.get("assembly_captured") is not True or row.get("pseudocode_captured") is not False
                        or body.get("pseudocode") is not None
                        or any(body.get(key) != baseline.get(key) for key in ("address", "rva", "end", "chunks", "xrefs_to_entry", "flags"))
                        or [(ins["address"], ins["bytes"], ins.get("xrefs_from"), ins.get("xrefs_to"))
                            for ins in body.get("assembly", [])]
                        != [(ins["address"], ins["bytes"], ins.get("xrefs_from"), ins.get("xrefs_to"))
                            for ins in baseline.get("assembly", [])]):
                    raise ValueError("Native shared-tail helper omitted code or differs from original assembly")
                verified_shared_helpers.append(record)
            applied_outlines = supplement.get("function_outline_recoveries", [])
            if (len(applied_outlines) != len(outlines)
                    or any(any(applied.get(key) != value for key, value in requested.items())
                           or applied.get("base_database_modified") is not False
                           or applied.get("applied_chunks") != requested["expected_chunks"]
                           or type(applied.get("outline_flag")) is not int or applied["outline_flag"] <= 0
                           or applied["outline_flag"] & (applied["outline_flag"] - 1)
                           or applied.get("previous_flags") != requested["expected_flags"]
                           or applied.get("applied_flags") != (requested["expected_flags"] | applied["outline_flag"])
                           for applied, requested in zip(applied_outlines, outlines))):
                raise ValueError("Native supplement did not retain the exact reviewed outlined helpers")
            applied_partitions = supplement.get("function_partition_recoveries", [])
            if len(applied_partitions) != len(partitions):
                raise ValueError("Native supplement omitted exact reviewed function partitions")
            partitioned_baselines, helper_baselines, helper_flags = {}, {}, {}
            for applied, requested in zip(applied_partitions, partitions):
                if (any(applied.get(key) != value for key, value in requested.items())
                        or applied.get("base_database_modified") is not False
                        or applied.get("applied_owner_chunks") != requested["owner_chunks"]
                        or len(applied.get("applied_helpers", [])) != len(requested["helpers"])):
                    raise ValueError("Native supplement function partition differs from its complete original byte union")
                bodies = native_function_partition_bodies(requested)
                partitioned_baselines[requested["owner_rva"]] = bodies["owner"]
                helper_baselines.update(bodies["helpers"])
                for actual_helper, helper in zip(applied["applied_helpers"], requested["helpers"]):
                    previous, flag = actual_helper.get("previous_flags"), actual_helper.get("outline_flag")
                    if (actual_helper.get("rva") != helper["rva"]
                            or actual_helper.get("chunks") != helper["expected_chunks"]
                            or actual_helper.get("base_database_modified") is not False
                            or type(previous) is not int or previous < 0 or type(flag) is not int or flag <= 0
                            or flag & (flag - 1) or actual_helper.get("applied_flags") != previous | flag):
                        raise ValueError("Native supplement did not retain exact partition helper chunks and outline flags")
                    helper_flags[helper["rva"]] = actual_helper["applied_flags"]
            helper_records = supplement.get("partition_helpers", [])
            if (len(helper_records) != len(helper_baselines)
                    or {row["rva"] for row in helper_records} != set(helper_baselines)):
                raise ValueError("Native supplement omitted complete partition helper assembly")
            verified_helpers = []
            for row in helper_records:
                record = {**row, "path": rel(path.parent / row["path"])}
                body = checked(ROOT / record["path"], record)
                baseline = helper_baselines[row["rva"]]
                if (row.get("assembly_captured") is not True or row.get("pseudocode_captured") is not False
                        or body.get("pseudocode") is not None or body.get("flags") != helper_flags[row["rva"]]
                        or any(body[key] != baseline[key] for key in ("address", "rva", "end", "chunks", "xrefs_to_entry"))
                        or [(ins["address"], ins["bytes"]) for ins in body["assembly"]]
                        != [(ins["address"], ins["bytes"]) for ins in baseline["assembly"]]):
                    raise ValueError("Native partition helper omitted code or differs from its complete original assembly")
                verified_helpers.append(record)
            verified = {}
            for row in supplement["functions"]:
                record = {**row, "path": rel(path.parent / row["path"])}
                body = checked(ROOT / record["path"], record)
                base_record = base_functions.get(row["rva"])
                if base_record is None:
                    raise ValueError("Native reanalysis introduced a boundary absent from its base capture")
                baseline = (shared_baselines[row["rva"]] if row["rva"] in shared_baselines
                            else entry_baselines[row["rva"]] if row["rva"] in entry_baselines
                            else boundary_baselines[row["rva"]] if row["rva"] in boundary_baselines
                            else instruction_baselines[row["rva"]] if row["rva"] in instruction_baselines
                            else partitioned_baselines[row["rva"]] if row["rva"] in partitioned_baselines
                            else merged_baselines[row["rva"]] if row["rva"] in merged_baselines
                            else checked(result / base_record["path"], base_record))
                if (not row.get("assembly_captured") or not row.get("pseudocode_captured")
                        or not isinstance(body.get("pseudocode"), dict) or not body["pseudocode"].get("lines")
                        or not isinstance(body["pseudocode"].get("locals"), list)
                        or any(body[key] != baseline[key] for key in ("address", "rva", "end", "chunks", "xrefs_to_entry"))
                        or [(item["address"], item["bytes"]) for item in body["assembly"]]
                        != [(item["address"], item["bytes"]) for item in baseline["assembly"]]):
                    raise ValueError("Native supplemental function omitted code or differs from original complete assembly")
                if shared_tails and (body.get("flags") != baseline.get("flags")
                        or [(item["address"], item.get("xrefs_from"), item.get("xrefs_to")) for item in body["assembly"]]
                        != [(item["address"], item.get("xrefs_from"), item.get("xrefs_to")) for item in baseline["assembly"]]):
                    raise ValueError("Native supplemental shared owner flags or references differ from original assembly")
                expected_calls = [(call, helper) for helper in fresh_inlines for call in helper["callsites"]
                                  if call.get("owner_rva", call["parent_rva"]) == row["rva"]]
                if expected_calls:
                    audit = body.get("fresh_inline_audit", {})
                    dontcopy, external = audit.get("dontcopy_flag"), audit.get("external_frame_flag")
                    if (audit.get("complete") is not True or audit.get("hook_registered") is not True
                            or audit.get("hook_unregistered") is not True or audit.get("errors")
                            or audit.get("target_execution") is not False
                            or audit.get("base_database_modified") is not False
                            or type(dontcopy) is not int or dontcopy <= 0 or dontcopy & (dontcopy - 1)
                            or type(external) is not int or external <= 0 or external & (external - 1)
                            or dontcopy == external or len(audit.get("calls", [])) != len(expected_calls)):
                        raise ValueError("Native fresh inlining lacks its complete SDK operation and cleanup audit")
                    actual_calls = audit["calls"]
                    for actual_call, (expected, helper) in zip(actual_calls, expected_calls):
                        nested = helper.get("inline_scope") == "reviewed_nested_register_leaf"
                        names = ["INLINE_DONTCOPY"] if nested else ["INLINE_EXTFRAME", "INLINE_DONTCOPY"]
                        if (any(actual_call.get(key) != value for key, value in expected.items())
                                or actual_call.get("result_code") != 0 or actual_call.get("status") != "inlined"
                                or actual_call.get("inline_flags") != (dontcopy if nested else dontcopy | external)
                                or actual_call.get("flag_names") != names):
                            raise ValueError("Native fresh inline SDK calls differ from the exact reviewed callsites")
                        if nested:
                            verify_nested_inline_mapping(selection, row["rva"], expected, actual_call, actual_calls)
                if selection.get("optimizer_properties"):
                    import native_boundaries
                    native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
                    native_boundaries.verify_optimizer_properties(selection["optimizer_properties"], body)
                elif "optimizer_property_audit" in body:
                    raise ValueError("Native supplement contains unreviewed optimizer property changes")
                verified[row["rva"]] = {"capture": record, **body}
            import native_adjudication
            if recursive_contexts:
                import native_boundaries
                native_boundaries.retain_source(sys.modules[__name__], supplement.get("boundary_validator"))
            verified_contexts = native_adjudication.recursive_context_coverage(
                sys.modules[__name__], raw, capture, supplement, path.parent)
            receipt = {"path": rel(path), "bytes": path.stat().st_size, "sha256": sha(path)}
            summary = {"receipt": receipt,
                **({"independent_export_finalization": finalized} if finalized else {}),
                "session": {"path": rel(session_path), "bytes": session_path.stat().st_size, "sha256": sha(session_path)},
                "ida_version": supplement["ida_version"], "hexrays_version": supplement["hexrays_version"],
                "type_recoveries": supplement["type_recoveries"], "functions": list(verified),
                **({"recursive_context_recoveries": supplement["recursive_context_recoveries"],
                    "complete_contexts": verified_contexts, "boundary_validator": supplement["boundary_validator"]}
                   if recursive_contexts else {}),
                **({"type_owner_automatic_reanalysis": supplement["type_owner_automatic_reanalysis"],
                    "type_owner_validator": supplement["callsite_type_validator"]}
                   if selection.get("type_owner_reanalysis") else {}),
                **({"callsite_type_recoveries": supplement["callsite_type_recoveries"]} if callsite_types else {}),
                **({"function_boundary_recoveries": supplement["function_boundary_recoveries"],
                    "boundary_helpers": verified_boundary_helpers} if boundary_recoveries else {}),
                **({"x86_entry_partition_recoveries": supplement["x86_entry_partition_recoveries"],
                    "entry_partition_helpers": verified_entry_helpers,
                    "boundary_validator": supplement["boundary_validator"]} if entry_partitions else {}),
                **({"instruction_boundary_recoveries": supplement["instruction_boundary_recoveries"]}
                   if instruction_recoveries else {}),
                **({"function_merge_recoveries": applied_merges} if merges else {}),
                **({"function_merge_automatic_reanalysis": supplement["function_merge_automatic_reanalysis"],
                    "boundary_validator": supplement["boundary_validator"]}
                   if "function_merge_automatic_reanalysis" in supplement else {}),
                **({"decompiler_prepasses": supplement["decompiler_prepasses"],
                    "boundary_validator": supplement["boundary_validator"]} if prepasses else {}),
                **({"optimizer_properties": selection["optimizer_properties"],
                    "boundary_validator": supplement["boundary_validator"]} if selection.get("optimizer_properties") else {}),
                **({"frame_rebuilds": supplement["frame_rebuilds"],
                    "boundary_validator": supplement["boundary_validator"]} if frame_rebuilds else {}),
                **({"derived_stack_points": supplement["derived_stack_points"],
                    "boundary_validator": supplement["boundary_validator"]} if derived_stack_plans else {}),
                **({"shared_tail_context_recoveries": supplement["shared_tail_context_recoveries"],
                    "boundary_validator": supplement["boundary_validator"]} if tail_contexts else {}),
                **({"switch_metadata_recoveries": supplement["switch_metadata_recoveries"],
                    "callsite_type_validator": supplement["callsite_type_validator"]} if switch_recoveries else {}),
                **({"function_outline_recoveries": applied_outlines} if outlines else {}),
                **({"function_shared_tail_recoveries": applied_shared,
                    "shared_tail_helpers": verified_shared_helpers} if shared_tails else {}),
                **({"function_partition_recoveries": applied_partitions,
                    "partition_helpers": verified_helpers} if partitions else {})}
        except (OSError, ValueError, KeyError, TypeError) as error:
            resolution["verification_failures"].append({"supplement": rel(path),
                "error": str(error).replace(str(ROOT), "<project>")})
            continue
        resolution["supplements"].append(summary)
        for core_rva, context in verified_contexts.items():
            context = {**context, "supplement": receipt}
            previous = resolution["complete_contexts"].get(core_rva)
            if core_rva in resolution["conflicting_contexts"] or previous is not None and previous != context:
                if core_rva not in resolution["conflicting_contexts"]:
                    resolution["conflicting_contexts"][core_rva] = [previous]
                resolution["conflicting_contexts"][core_rva].append(context)
                resolution["complete_contexts"].pop(core_rva, None)
                resolution["verification_failures"].append({"supplement": rel(path), "function_rva": core_rva,
                    "error": "Native recursive contexts conflict for " + core_rva})
                continue
            resolution["complete_contexts"][core_rva] = context
        for rva, body in verified.items():
            record = body["capture"]
            verified_candidates[rva].append({"capture": record, "body": {key: value for key, value in body.items()
                                             if key != "capture"}, "supplement": receipt})
            conflict = resolution["conflicting_functions"].get(rva)
            previous = resolution["complete_functions"].get(rva)
            retained = conflict["retained_function"] if conflict else previous["capture"] if previous else None
            if retained is not None and retained["sha256"] != record["sha256"]:
                if conflict is None:
                    conflict = {"retained_function": retained, "retained_supplement": function_sources[rva],
                                "alternatives": [], "semantic_equivalence": "unverified"}
                    resolution["conflicting_functions"][rva] = conflict
                    del resolution["complete_functions"][rva]
                conflict["alternatives"].append({"complete_function": record, "supplement": receipt})
                resolution["verification_failures"].append({"supplement": rel(path), "function_rva": rva,
                    "error": "Native supplemental functions conflict for " + rva})
            elif retained is None:
                resolution["complete_functions"][rva] = body
                function_sources[rva] = receipt
    import native_adjudication
    function_sources.update(native_adjudication.apply(sys.modules[__name__], result, resolution,
                                                     verified_candidates, adjudication_record))
    for rva, body in list(resolution["complete_functions"].items()):
        try:
            native_adjudication._typed_function(body)
        except (ValueError, KeyError, TypeError) as error:
            resolution["verification_failures"].append({"supplement": function_sources[rva]["path"],
                "function_rva": rva, "error": str(error)})
            del resolution["complete_functions"][rva]
            del function_sources[rva]
    resolution["conflicting_function_rvas"] = sorted(resolution["conflicting_functions"], key=lambda rva: int(rva, 16))
    resolution["remaining_failures"] = []
    for failure in capture["failures"]:
        match = re.fullmatch(r"([0-9a-fA-F]+):hexrays", failure.get("stage", ""))
        rva = hex(int(match[1], 16)) if match else None
        if rva in resolution["complete_functions"]:
            resolution["resolved_failures"].append({"original_failure": failure, "function_rva": rva,
                "supplement": function_sources[rva], "complete_function": resolution["complete_functions"][rva]["capture"]})
        elif rva in resolution["complete_contexts"]:
            context = resolution["complete_contexts"][rva]
            resolution["resolved_failures"].append({"original_failure": failure, "function_rva": rva,
                "resolved_by": "verified_original_root_context", "supplement": context["supplement"],
                "complete_context": context})
        else:
            resolution["remaining_failures"].append(failure)
    import native_import_forwarders
    native_import_forwarders.apply_resolution(sys.modules[__name__], result, resolution, capture)
    import native_static_reviews
    native_static_reviews.apply_resolution(sys.modules[__name__], result, resolution, capture)
    native_adjudication.apply_attempts(sys.modules[__name__], result, resolution, attempt_adjudication_record)
    import native_embedded
    native_embedded.apply_resolution(sys.modules[__name__], result, resolution, capture)
    return resolution


def verify_native_capture(result, resource):
    """Verify a complete retained capture without trusting its success flag alone."""
    manifest_path = resolve(rel(result / "capture.json"), rel(result))
    before = manifest_path.stat()
    raw = manifest_path.read_bytes()
    manifest = json.loads(raw)
    if manifest.get("input_sha256") != resource["sha256"] or manifest.get("input_bytes") != resource["bytes"]:
        raise ValueError("Existing native capture has a different original identity")
    if manifest.get("capture_complete") is not True or manifest.get("functions_complete") is False:
        raise ValueError("Native capture is not complete")
    verify_native_typed_serializer(manifest)
    functions = manifest.get("functions")
    outputs = manifest.get("outputs")
    if not isinstance(functions, list) or not isinstance(outputs, list):
        raise ValueError("Native capture lacks complete function and output inventories")
    count = len(functions)
    for field in ("functions_captured", "functions_total", "settled_functions_total"):
        value = manifest.get(field)
        if type(value) is not int or value < 0 or value != count:
            raise ValueError("Native capture function inventory differs: " + field)
    if manifest.get("newly_discovered_addresses") or manifest.get("removed_function_boundaries"):
        raise ValueError("Native capture function discovery has not settled")
    if "aggregate_functions_written" in manifest and manifest["aggregate_functions_written"] != count:
        raise ValueError("Native capture aggregate function count differs")
    datasets = ("segments", "imports", "strings", "entrypoints", "names", "local_types", "structures")
    for name in datasets:
        value = manifest.get(name + "_count")
        if type(value) is not int or value < 0:
            raise ValueError("Native capture dataset is incomplete: " + name)
    required = {name + ".json" for name in datasets} | {"all-functions.c", "all-functions.asm"}
    output_paths = [record.get("path") for record in outputs]
    if not required.issubset(output_paths):
        raise ValueError("Native capture required outputs are missing: " + ", ".join(sorted(required - set(output_paths))))
    base = int(manifest["image_base"], 16)
    addresses, rvas = set(), set()
    for record in functions:
        address, rva = int(record["address"], 16), int(record["rva"], 16)
        if address < base or address - base != rva or address in addresses or rva in rvas:
            raise ValueError("Native capture function identities are invalid or duplicated")
        if record.get("assembly_captured") is not True:
            raise ValueError("Native capture function assembly is incomplete: " + record["rva"])
        addresses.add(address)
        rvas.add(rva)
    paths = set()
    for record in functions + outputs:
        path = resolve(rel(result / record["path"]), rel(result))
        if path in paths:
            raise ValueError("Native capture artifact path is duplicated: " + rel(path))
        paths.add(path)
        snapshot = path.stat()
        if (not path.is_file() or ("bytes" in record and snapshot.st_size != record["bytes"])
                or sha(path) != record["sha256"]):
            raise ValueError("Completed native capture changed: " + rel(path))
        after = path.stat()
        if (after.st_size, after.st_mtime_ns) != (snapshot.st_size, snapshot.st_mtime_ns):
            raise ValueError("Native capture artifact changed during verification: " + rel(path))
    after = manifest_path.stat()
    if (after.st_size, after.st_mtime_ns) != (before.st_size, before.st_mtime_ns):
        raise ValueError("Native capture inventory changed during verification")
    return manifest, {"path": rel(manifest_path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}


def capture_corpus(args):
    workers = getattr(args, "workers", 4)
    if type(workers) is not int or not 2 <= workers <= 4:
        raise ValueError("Native capture workers must be between two and four")
    receipt_path = ROOT / (".analysis/results/corpus.json" if args.source == "retained"
                           else ".analysis/inputs/official-native-current/acquisition.json")
    acquisition_bytes = receipt_path.read_bytes()
    acquisition = json.loads(acquisition_bytes)
    modules = {}
    if args.source == "retained":
        modules = {row["sha256"]: {"sha256": row["sha256"], "bytes": row["bytes"],
                                  "path": row.get("verified_aliases", row["aliases"])[0], "aliases": row["aliases"]}
                   for row in acquisition["native_inputs"] + acquisition.get("mixed_inputs", [])
                   if row.get("verified_aliases", row["aliases"])}
    else:
        if not acquisition.get("complete"):
            raise ValueError("Production acquisition is incomplete; inspect its recorded failures and stability")
        for resource in acquisition["resources"]:
            if resource["status"] == "verified" and Path(resource["path"]).suffix.lower() in {".dll", ".node", ".sys"}:
                modules.setdefault(resource["sha256"], resource)
    patterns = [re.compile(value) for value in (getattr(args, "include_path", None) or [])]
    pending = [row for row in modules.values() if patterns and not any(
        pattern.search(alias) for pattern in patterns for alias in row.get("aliases", [row["path"]]))]
    modules = {identity: row for identity, row in modules.items() if row not in pending}
    output = ROOT / ".analysis/results/native-corpus-capture.json"
    queue = {"schema_version": 1, "started_at_utc": stamp(), "acquisition": rel(receipt_path),
             "acquisition_sha256": hashlib.sha256(acquisition_bytes).hexdigest(), "module_count": len(modules), "modules": [],
             "include_path_patterns": getattr(args, "include_path", None) or [], "pending_inputs": pending,
             "scope": ("all retained unique native bytes; current-version lineage remains a separate audit" if args.source == "retained" else
                       "all unique DLL/NODE/SYS bytes declared by acquired production device manifests; packaged EXE and host inputs remain separate gaps"),
             "workers": workers, "target_execution": False, "real_device_operations": False}
    save(output, queue)
    original_snapshots = {}
    configuration_lock = threading.Lock()
    configuration_prepared = False

    def capture_module(identity, resource):
        nonlocal configuration_prepared
        result = ROOT / ".analysis/results/native" / identity
        manifest_path = result / "capture.json"
        row = {"sha256": identity, "input": resource["path"], "result": rel(result)}
        snapshot = None
        try:
            source = resolve(resource["path"], ".analysis/inputs")
            before = source.stat()
            if before.st_size != resource["bytes"] or sha(source) != identity:
                raise ValueError("Current corpus original bytes differ: " + resource["path"])
            if (source.stat().st_size, source.stat().st_mtime_ns) != (before.st_size, before.st_mtime_ns):
                raise ValueError("Current corpus original changed during identity verification")
            row["original_identity_verified"] = True
            snapshot = (before.st_size, before.st_mtime_ns)
            if manifest_path.is_file():
                manifest = json.loads(manifest_path.read_bytes())
                if manifest["input_sha256"] != identity or manifest.get("input_bytes") != resource["bytes"]:
                    raise ValueError("Existing native capture has a different original identity")
                if manifest.get("capture_complete"):
                    manifest, capture_identity = verify_native_capture(result, resource)
                    row.update({"status": "reused_complete_capture", "functions": manifest["functions_captured"],
                                "failures": len(manifest["failures"]), "capture": capture_identity})
            if "status" not in row:
                if workers > 1:
                    with configuration_lock:
                        if not configuration_prepared:
                            configure_ida()
                            configuration_prepared = True
                session = capture_native(SimpleNamespace(input=resource["path"], resume=(result / "plan.json").is_file(),
                                                        mcp_only=False, ida_root=args.ida_root, no_refresh_gaps=True,
                                                        ida_configuration_prepared=workers > 1))
                row.update({"status": "captured" if session["passed"] else "failed", "exit_code": session["exit_code"]})
                if manifest_path.is_file():
                    manifest = json.loads(manifest_path.read_bytes())
                    row.update({"functions": manifest.get("functions_captured", 0), "failures": len(manifest["failures"])})
                if session["passed"]:
                    manifest, row["capture"] = verify_native_capture(result, resource)
            stable = (source.stat().st_size, source.stat().st_mtime_ns) == (before.st_size, before.st_mtime_ns)
            row["original_snapshot_unchanged"] = stable
            if not stable:
                raise ValueError("Current corpus original changed during capture verification")
        except Exception as exc:
            row.update({"status": "failed", "error": str(exc).replace(str(ROOT), "<project>")})
        return row, snapshot

    ordered_modules = sorted(modules.items(), key=lambda item: (item[1]["bytes"], item[0]))
    order = {identity: index for index, (identity, _) in enumerate(ordered_modules)}

    def publish(completed):
        row, snapshot = completed
        if snapshot is not None:
            original_snapshots[row["input"]] = snapshot
        queue["modules"].append(row)
        queue["modules"].sort(key=lambda item: order[item["sha256"]])
        save(output, queue)
        print(json.dumps({"module_done": len(queue["modules"]), "module_total": len(modules), **row}), flush=True)

    # Native analysis is always a bounded multi-threaded batch.  A single
    # worker would silently turn the corpus command into a serial reverse
    # engineering run and violate the batch scheduling contract.
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as executor:
        futures = [executor.submit(capture_module, identity, resource) for identity, resource in ordered_modules]
        for future in concurrent.futures.as_completed(futures):
            publish(future.result())
    queue["finished_at_utc"] = stamp()
    changed_originals = []
    for name, expected in original_snapshots.items():
        try:
            current = resolve(name, ".analysis/inputs").stat()
            if (current.st_size, current.st_mtime_ns) != expected:
                changed_originals.append(name)
        except (OSError, ValueError):
            changed_originals.append(name)
    queue["original_inputs_snapshot_unchanged"] = not changed_originals
    queue["original_inputs_changed_during_capture"] = changed_originals
    queue["capture_complete"] = not changed_originals and all(row["status"] != "failed" for row in queue["modules"])
    queue["current_acquisition_sha256"] = sha(receipt_path)
    queue["snapshot_unchanged"] = queue["current_acquisition_sha256"] == queue["acquisition_sha256"]
    queue["full_corpus_captured"] = queue["capture_complete"] and not pending and queue["snapshot_unchanged"]
    save(output, queue)
    if not getattr(args, "no_refresh_gaps", False):
        refresh_gaps()
    print(json.dumps({"receipt": rel(output), "capture_complete": queue["capture_complete"]}), flush=True)


def wakaru():
    directory = ROOT / "tools/analyzers/wakaru-1.14.0"
    tool = directory / "wakaru.exe"
    if not tool.is_file():
        raise ValueError("Missing Wakaru executable")
    receipt = json.loads((directory / "acquisition.json").read_bytes())
    expected = receipt.get("executable_sha256") or receipt.get("binary_sha256")
    # The receipt may contain the extracted binary as an artifact object.
    if expected is None:
        for entry in receipt.get("files", []):
            if entry.get("path", "").endswith("wakaru.exe"):
                expected = entry.get("sha256")
    if expected is None:
        for value in receipt.values():
            if isinstance(value, dict) and value.get("path", "").endswith("wakaru.exe"):
                expected = value.get("sha256")
    if expected is None:
        raise ValueError("Wakaru acquisition receipt has no executable digest")
    if sha(tool) != expected:
        raise ValueError("Wakaru executable identity changed")
    return tool


class References(HTMLParser):
    def __init__(self):
        super().__init__()
        self.refs = []
        self.base = None

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "base":
            self.base = attrs.get("href")
        if tag in {"script", "img", "source", "video", "audio", "iframe", "embed"}:
            for key in ("src", "poster"):
                if attrs.get(key):
                    self.refs.append((attrs[key], "html:"+tag+":"+key))
        if tag == "link" and attrs.get("href") and set(attrs.get("rel", "").split()) & {
                "stylesheet", "icon", "manifest", "preload", "modulepreload", "shortcut"}:
            self.refs.append((attrs["href"], "html:link"))
        if attrs.get("srcset"):
            for candidate in attrs["srcset"].split(","):
                self.refs.append((candidate.strip().split()[0], "html:srcset"))


def artifact_url(base, literal):
    literal = literal.strip()
    if not literal or literal.startswith(("data:", "blob:", "#", "javascript:")):
        return None
    parsed = urllib.parse.urlsplit(urllib.parse.urljoin(base, literal))
    if parsed.scheme != "https" or parsed.username or parsed.password:
        return None
    path = urllib.parse.quote(parsed.path, safe="/%:@!$&'()*+,;=-._~")
    return urllib.parse.urlunsplit(parsed._replace(path=path, fragment=""))


def raw_path(output, url):
    parsed = urllib.parse.urlsplit(url)
    parts = [urllib.parse.unquote(p) for p in parsed.path.split("/") if p]
    if any(p in {".", ".."} or any(c in p for c in '\\:*?"<>|') for p in parts):
        raise ValueError("Unsafe URL artifact path")
    path = output / "raw" / parsed.netloc / Path(*parts)
    if not parts or parsed.path.endswith("/"):
        path = path / "index.html"
    if parsed.query:
        path = path.with_name(path.name + ".query-"+hashlib.sha256(parsed.query.encode()).hexdigest()[:16])
    # A slashless HTML request may already occupy the path which becomes a
    # directory after its observed redirect. Retain that original unchanged;
    # store child responses under an explicit per-URL namespace instead.
    if any(parent.is_file() for parent in path.parents if parent != output and parent.is_relative_to(output)):
        path = output / "raw-by-url" / hashlib.sha256(url.encode()).hexdigest() / path.name
    resolved_path, resolved_output = (
        _resolved_path_identity(path.resolve()), _resolved_path_identity(output.resolve()))
    if not resolved_path.is_relative_to(resolved_output):
        def portable(value):
            return str(value).replace(str(ROOT), "<project>").replace(str(ROOT).replace("\\", "/"), "<project>")
        raise ValueError("URL escaped output: " + json.dumps({"url": url, "candidate": portable(path),
            "resolved_candidate": portable(resolved_path), "resolved_output": portable(resolved_output)}, ensure_ascii=False))
    return path


def download(url, output):
    try:
        request = urllib.request.Request(url, headers={"User-Agent": "razer-ui-static-evidence", "Accept-Encoding": "identity"})
        with urllib.request.urlopen(request, timeout=40) as response:
            raw = response.read()
            row = {"url": url, "final_url": response.geturl(), "http_status": response.status,
                   "content_type": response.headers.get("Content-Type"), "headers": dict(response.headers),
                   "acquired_at_utc": stamp(), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        path = raw_path(output, url)
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists() and path.read_bytes() != raw:
            raise ValueError("Different bytes already retained for URL")
        path.write_bytes(raw)
        row["path"] = rel(path)
        return row
    except Exception as exc:
        return {"url": url, "error": str(exc), "acquired_at_utc": stamp()}


def literal_webpack_chunks(text):
    """Supplement analyzer failures only for complete literal webpack functions.

    Accept the observed (name[id] || id) + '.' + hash[id] + '.js' grammar,
    with one literal publicPath assignment. Never evaluate a JS expression.
    Other runtimes remain explicitly unresolved by the primary analyzer.
    """
    if ".u" not in text:
        return []
    # Start at a complete identifier. Without this boundary a long embedded
    # asset string is retried at every suffix even though it cannot be a loader.
    identifier = r"(?<![\w$])[A-Za-z_$][\w$]*"
    expression = (r"\(\s*\{(?P<names>[^{}]*)\}\s*\[\s*(?P=arg)\s*\]\s*"
                  r"\|\|\s*(?P=arg)\s*\)\s*\+\s*\"\.\"\s*\+\s*"
                  r"\{(?P<hashes>[^{}]*)\}\s*\[\s*(?P=arg)\s*\]\s*\+\s*\"\.js\"")
    patterns = (
        ("literal-arrow", rf"(?P<runtime>{identifier})\.u\s*=\s*(?P<arg>{identifier})\s*=>\s*"
         + r"\(\s*" + expression + r"\s*\)(?=\s*(?:[,;]|$))"),
        ("literal-function-return", rf"(?P<runtime>{identifier})\.u\s*=\s*function\s*"
         + rf"\(\s*(?P<arg>{identifier})\s*\)\s*\{{\s*return\s*"
         + expression + r"\s*;?\s*\}(?=\s*(?:[,;]|$))"),
    )
    results = []
    matches = [(kind, match) for kind, pattern in patterns for match in re.finditer(pattern, text)]
    for kind, match in sorted(matches, key=lambda item: item[1].start()):
        runtime = re.escape(match["runtime"])
        assignments = list(re.finditer(rf"\b{runtime}\.p\s*=(?!=)", text))
        if len(assignments) != 1:
            continue
        public = re.match(rf'{runtime}\.p\s*=\s*("(?:[^"\\]|\\.)*")(?=\s*(?:[,;]|$))', text[assignments[0].start():])
        if public is None:
            continue
        try:
            tables = [json.loads(re.sub(r'([{,])\s*(\d+)\s*:', r'\1"\2":', "{" + match[name] + "}"))
                      for name in ("names", "hashes")]
            names, hashes = tables
            public_path = json.loads(public[1])
        except (ValueError, TypeError):
            continue
        if not hashes or any(not key.isdecimal() or not isinstance(value, str)
                             or not re.fullmatch(r"[0-9a-f]{16,64}", value) for key, value in hashes.items()):
            continue
        if any(not key.isdecimal() or not isinstance(value, str)
               or not re.fullmatch(r"[A-Za-z0-9_.-]+", value) for key, value in names.items()):
            continue
        results.append({"grammar": kind,
                        "source_start_byte": len(text[:match.start()].encode("utf-8")),
                        "source_end_byte": len(text[:match.end()].encode("utf-8")),
                        "complete_assignment": match[0], "public_path_assignment": public[0],
                        "public_path": public_path,
                        "filenames": [f"{names.get(key, key)}.{value}.js" for key, value in hashes.items()],
                        "scope": "all entries of this literal filename table only; other dynamic expressions and resource references remain unverified"})
    return results


FRONTEND_REFERENCE_VERSION = 2


def enumerate_frontend_chunks(path, tool, report_dir, identifier, tool_identity=None):
    """Reuse a complete static invocation only while every byte identity matches."""
    def identity(item):
        return {"path": rel(item), "bytes": item.stat().st_size, "sha256": sha(item)}

    source = identity(path)
    analyzer = tool_identity or identity(tool)
    output = report_dir / (identifier + ".json")
    errors = report_dir / (identifier + ".stderr.txt")
    receipt = report_dir / (identifier + ".capture.json")
    contract = {"schema_version": 1, "input": source, "analyzer": analyzer,
                "arguments": ["debug", "enumerate-chunks", source["path"]],
                "target_execution": False}
    try:
        prior = json.loads(receipt.read_bytes())
        if (all(prior.get(key) == value for key, value in contract.items())
                and prior.get("input_bytes_unchanged")
                and prior.get("stdout") == identity(output)
                and prior.get("stderr") == identity(errors)
                and type(prior.get("exit_code")) is int):
            return SimpleNamespace(returncode=prior["exit_code"], stdout=output.read_bytes(),
                                   stderr=errors.read_bytes())
    except (OSError, ValueError, KeyError):
        pass
    completed = subprocess.run([str(tool), "debug", "enumerate-chunks", str(path)],
                               capture_output=True, check=False)
    report_dir.mkdir(parents=True, exist_ok=True)
    output.write_bytes(completed.stdout)
    errors.write_bytes(completed.stderr)
    unchanged = source == identity(path)
    save(receipt, {**contract, "recorded_at_utc": stamp(), "exit_code": completed.returncode,
                   "stdout": identity(output), "stderr": identity(errors),
                   "input_bytes_unchanged": unchanged})
    if not unchanged:
        raise ValueError("Frontend input changed during static enumeration: " + source["path"])
    return completed


def fetch_frontend(args, *, refresh_ledger=True):
    refresh_ledger = refresh_ledger and not getattr(args, "no_refresh_gaps", False)
    output = resolve(args.output, ".analysis/inputs")
    if getattr(args, "repair", False):
        import families
        repair_entry = artifact_url(args.url, args.url)
        if repair_entry is None:
            raise ValueError("Expected HTTPS static HTML input")
        repair_receipt = output / "acquisition.json"
        if (repair_receipt.is_file()
                and json.loads(repair_receipt.read_bytes()).get("entry_url") != repair_entry):
            raise ValueError("Output belongs to a different app entry")
        # Reuse the same retained-byte, stale-deployment and HTTP-history repair
        # used for device families. Its recursive crawl does not set repair.
        result = families.acquire_product({"input": rel(output), "entry_url": repair_entry},
                                          families.SharedArtifacts(), retry_transient=True)
        if "error" in result:
            raise ValueError(result["error"])
        if getattr(args, "seed_urls", None):
            return fetch_frontend(SimpleNamespace(**{**vars(args), "repair": False}),
                                  refresh_ledger=refresh_ledger)
        if refresh_ledger:
            refresh_gaps()
        return result
    output.mkdir(parents=True, exist_ok=True)
    tool = wakaru()
    tool_identity = {"path": rel(tool), "bytes": tool.stat().st_size, "sha256": sha(tool)}
    downloader = getattr(args, "download_artifact", download)
    entry = artifact_url(args.url, args.url)
    if entry is None:
        raise ValueError("Expected HTTPS static HTML input")
    origin = urllib.parse.urlsplit(entry).netloc
    acquisition = output / "acquisition.json"
    previous = json.loads(acquisition.read_bytes()) if acquisition.exists() else {}
    if previous and previous["entry_url"] != entry:
        raise ValueError("Output belongs to a different app entry")
    rows = {}
    for row in previous.get("files", []):
        normalized = artifact_url(entry, row["url"])
        row["url"] = normalized
        rows[normalized] = row
    unknown = []
    queue = {entry: [{"source": "entry", "kind": "entry"}]}
    entry_response_url = entry
    asset_manifest_probe = urllib.parse.urljoin(entry, "asset-manifest.json")
    processed = set()
    edges = []
    declared_manifests = set()
    report_dir = ROOT / ".analysis/results/frontend" / output.name / "chunk-references"
    report_dir.mkdir(parents=True, exist_ok=True)

    def enqueue(base, value, kind, *, owner=None):
        url = artifact_url(base, value)
        if url is None:
            return
        edge = {"from": owner or base, "literal": value, "to": url, "kind": kind}
        if owner is not None and owner != base:
            edge["resolution_base"] = base
        edges.append(edge)
        if Path(urllib.parse.urlsplit(url).path).name in {"asset-manifest.json", "webpackManifest.json", "manifest.json"}:
            declared_manifests.add(url)
        if urllib.parse.urlsplit(url).netloc != origin:
            unknown.append({"kind": "external_static_reference", **edge})
            return edge
        queue.setdefault(url, []).append({"source": base, "kind": kind})
        return edge

    for seed in getattr(args, "seed_urls", None) or []:
        # Caller-supplied product manifest, explicitly distinguished from an
        # HTML reference. It still passes normal origin/path validation.
        enqueue(entry, seed, "explicit-retained-manifest")

    while set(queue) - processed:
        batch = sorted(set(queue) - processed)
        needed = []
        for url in batch:
            row = rows.get(url)
            if row and "path" in row:
                path = ROOT / row["path"]
                if not path.is_file() or sha(path) != row["sha256"]:
                    raise ValueError("Acquired bytes changed: "+row["path"])
            elif row and (row.get("http_status") == 404 or row.get("error", "").startswith("HTTP Error 404:")):
                continue
            else:
                needed.append(url)
        with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
            for row in pool.map(lambda url: downloader(url, output), needed):
                rows[row["url"]] = row
        # Independent JS parses use a bounded pool; traversal and receipt
        # publication still run in one owner. No vendor code is executed.
        javascript = [url for url in batch if "path" in rows[url]
                      and Path(urllib.parse.urlsplit(url).path).suffix.lower() in {".js", ".mjs"}
                      and rows[url].get("content_type", "").split(";", 1)[0].lower() != "text/html"]
        def enumerate_url(url):
            identifier = hashlib.sha256(url.encode()).hexdigest()[:24]
            return enumerate_frontend_chunks(ROOT / rows[url]["path"], tool, report_dir,
                                             identifier, tool_identity)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            enumerations = dict(zip(javascript, pool.map(enumerate_url, javascript)))
        for url in batch:
            processed.add(url)
            row = rows[url]
            if "path" not in row:
                continue
            reference_url = artifact_url(url, row.get("final_url", url))
            if reference_url is None or urllib.parse.urlsplit(reference_url).netloc != origin:
                unknown.append({"kind": "response_redirect_outside_origin", "url": url, "final_url": row.get("final_url")})
                continue
            if url == entry:
                entry_response_url = reference_url
                asset_manifest_probe = urllib.parse.urljoin(entry_response_url, "asset-manifest.json")
                queue.setdefault(asset_manifest_probe, [{"source": entry, "kind": "manifest"}])
            path = ROOT / row["path"]
            url_path = urllib.parse.urlsplit(url).path
            suffix = Path(url_path).suffix.lower()
            # Directory app entries are retained as index.html. Looking only
            # at the URL suffix silently skipped their actual HTML includes.
            if url_path.endswith("/") or row.get("content_type", "").split(";", 1)[0].lower() == "text/html":
                suffix = ".html"
            if suffix not in {".js", ".mjs", ".css", ".json", ".html"}:
                continue
            text = path.read_text(encoding="utf-8-sig", errors="strict")
            if suffix == ".html":
                parser = References()
                parser.feed(text)
                reference_base = urllib.parse.urljoin(reference_url, parser.base) if parser.base else reference_url
                for ref, kind in parser.refs:
                    enqueue(reference_base, ref, kind, owner=url)
            elif suffix == ".json":
                try:
                    data = json.loads(text)
                except ValueError as exc:
                    unknown.append({"kind": "json_parse_failure", "url": url, "error": str(exc)})
                    continue
                if (urllib.parse.urlsplit(url).path == "/synapse/assets/index.json"
                        and isinstance(data, list)):
                    declared_manifests.add(url)
                    for ordinal, item in enumerate(data):
                        if isinstance(item, dict) and isinstance(item.get("url"), str) and isinstance(item.get("revision"), str):
                            edge = enqueue(reference_url, item["url"], "shared-resource-index", owner=url)
                            if edge is None:
                                unknown.append({"kind": "shared_resource_index_url_rejected",
                                                "url": url, "entry": ordinal, "literal": item["url"]})
                            else:
                                edge["declared_revision"] = item["revision"]
                                edge["revision_validation"] = "source-declared value retained; digest algorithm not inferred"
                        else:
                            unknown.append({"kind": "shared_resource_index_entry_unresolved",
                                            "url": url, "entry": ordinal})
                elif isinstance(data, dict):
                    # Current middleware HTML declares webpackManifest.json, whose
                    # complete chunk map is a flat {"name.js": "name.hash.js"}.
                    # Parse the declared data without executing the webpack runtime.
                    if Path(urllib.parse.urlsplit(url).path).name == "webpackManifest.json":
                        for name, value in data.items():
                            if isinstance(value, str):
                                enqueue(reference_url, value, "webpack-manifest", owner=url)
                            else:
                                unknown.append({"kind": "webpack_manifest_entry_unresolved",
                                                "url": url, "entry": name})
                    for value in data.get("files", {}).values():
                        if isinstance(value, str):
                            enqueue(reference_url, value, "asset-manifest", owner=url)
                    for value in data.get("entrypoints", []):
                        enqueue(reference_url, value, "entrypoint", owner=url)
                    for icon in data.get("icons", []):
                        if isinstance(icon, dict) and icon.get("src"):
                            enqueue(reference_url, icon["src"], "web-manifest-icon", owner=url)
            elif suffix == ".css":
                for quoted, ref in re.findall(r'url\(\s*([\'"]?)([^)\'"\s]+)\1\s*\)', text):
                    enqueue(reference_url, ref, "css-url", owner=url)
                for ref in re.findall(r'@import\s+[\'"]([^\'"]+)[\'"]', text):
                    enqueue(reference_url, ref, "css-import", owner=url)
            else:
                identifier = hashlib.sha256(url.encode()).hexdigest()[:24]
                completed = enumerations[url]
                literal_tables = literal_webpack_chunks(text)
                if literal_tables:
                    save(report_dir / (identifier + ".literal-tables.json"),
                         {"input": rel(path), "input_sha256": sha(path),
                          "tool": {"path": "tools/analysis.py", "sha256": sha(Path(__file__))},
                          "tables": literal_tables, "target_execution": False})
                    for table in literal_tables:
                        for filename in table["filenames"]:
                            enqueue(urllib.parse.urljoin(entry_response_url, table["public_path"]), filename, "source-literal-webpack-chunk", owner=url)
                if completed.returncode:
                    unknown.append({"kind": "chunk_enumeration_failure", "url": url,
                                    "exit_code": completed.returncode, "log": rel(report_dir / (identifier+".stderr.txt"))})
                else:
                    enumeration = json.loads(completed.stdout).get("enumeration")
                    if enumeration is None:
                        unknown.append({"kind": "chunk_enumeration_unavailable", "url": url,
                                        "input": rel(path), "evidence": rel(report_dir / (identifier+".json")),
                                        "scope": "dynamic chunk/resource closure remains unverified"})
                    if enumeration:
                        for ref in enumeration.get("relative_imports", []):
                            enqueue(reference_url, ref["specifier"], "literal-import", owner=url)
                        public = enumeration["public_path"]
                        for asset in enumeration["assets"]:
                            if asset["status"] != "enumerated":
                                unknown.append({"kind": "chunk_unresolved", "url": url, "asset": asset})
                            for ref in asset["urls"]:
                                if public["status"] == "static":
                                    enqueue(urllib.parse.urljoin(entry_response_url, public["value"]), ref["url"], "webpack-chunk", owner=url)
                                elif public["status"] == "script_relative":
                                    enqueue(urllib.parse.urljoin(reference_url, public["value"]), ref["url"], "webpack-chunk", owner=url)
                                else:
                                    unknown.append({"kind": "chunk_public_path_unresolved", "url": url,
                                                    "public_path": public, "reference": ref})
            if suffix in {".css", ".js", ".mjs"}:
                for ref in re.findall(r'sourceMappingURL=([^\s*]+)', text):
                    enqueue(reference_url, ref, "source-map", owner=url)
        save(acquisition, {"schema_version": 1, "entry_url": entry, "recorded_at_utc": stamp(),
                           "files": sorted(rows.values(), key=lambda row: row["url"]), "edges": edges,
                           "reference_parser_version": FRONTEND_REFERENCE_VERSION,
                           "reference_resolution_policy": "response_final_url_with_requested_source_owner_v1",
                           "requested_urls": sorted(queue), "entry_response_url": entry_response_url,
                           "unresolved": unknown, "target_execution": False})
        print(json.dumps({"acquired": sum("path" in r for r in rows.values()),
                          "requested": len(queue), "failures": sum("error" in r for r in rows.values())}), flush=True)
    # Re-read the entry and manifest to detect a deployment changing during acquisition.
    stability = []
    asset_manifest = asset_manifest_probe
    if rows.get(asset_manifest, {}).get("path"):
        declared_manifests.add(asset_manifest)
    for url in dict.fromkeys([entry, *sorted(declared_manifests)]):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "razer-ui-static-evidence"}), timeout=40) as response:
                actual = hashlib.sha256(response.read()).hexdigest()
            stability.append({"url": url, "before_sha256": rows[url].get("sha256"), "after_sha256": actual,
                              "unchanged": actual == rows[url].get("sha256")})
        except Exception as exc:
            stability.append({"url": url, "before_sha256": rows[url].get("sha256"),
                              "unchanged": False, "error": str(exc)})
    data = json.loads(acquisition.read_bytes())
    data["stability"] = stability
    declared_urls = {entry, *(edge["to"] for edge in edges)}
    data["declared_references_fetched"] = all("path" in rows.get(url, {}) for url in declared_urls)
    data["probes"] = [{"url": asset_manifest, "declared": asset_manifest in declared_manifests}]
    data["source_current_during_acquisition"] = all(r["unchanged"] for r in stability)
    save(acquisition, data)
    if tool_identity["sha256"] != sha(tool):
        raise ValueError("Wakaru changed during frontend acquisition")
    indexed_rows = []
    for row in rows.values():
        if "path" in row:
            indexed_rows.append({key: row[key] for key in ("path", "bytes", "sha256", "url", "acquired_at_utc")}
                                | {"kind": "official_frontend_artifact"})
    indexed_rows.append({"path": rel(acquisition), "bytes": acquisition.stat().st_size,
                         "sha256": sha(acquisition), "kind": "acquisition_receipt"})
    getattr(args, "index_artifacts", index_files)(indexed_rows)
    print(json.dumps({"acquisition": rel(acquisition), "files": len(rows), "bytes": sum(r.get("bytes",0) for r in rows.values()),
                      "deployment_stable": data["source_current_during_acquisition"], "unresolved": len(unknown),
                      "gap_refresh": "pending" if refresh_ledger else "deferred_to_batch"}), flush=True)
    if refresh_ledger:
        try:
            refresh_gaps()
        except Exception as error:
            raise RuntimeError(f"Frontend acquisition receipt retained; gaps refresh failed: {error}") from error


def host_app_descriptors(text):
    """Locate actual literal window descriptors, retaining their full owner.

    This enumerates source references only; it does not assert that a window
    is mounted or that its local and remote feature branches are understood.
    """
    import families
    found, unresolved, modules, source_tree = [], [], None, None
    raw = text.encode("utf-8")

    def parsed_source():
        nonlocal source_tree
        if source_tree is None:
            sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
            from tree_sitter import Language, Parser
            import tree_sitter_javascript
            source_tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
        if source_tree.root_node.has_error:
            raise ValueError("Application source grammar has errors")
        return source_tree

    def nonwebpack_owner(character_offset):
        # Vite/ES module entry scripts have real top-level declarations rather
        # than webpack factory wrappers. Retain the exact declaration/function
        # which owns the reference, never an unrelated factory in the file.
        offset = len(text[:character_offset].encode())
        node = parsed_source().root_node.descendant_for_byte_range(offset, offset + 1)
        declaration = None
        while node is not None:
            if node.type in {"function_declaration", "function_expression", "arrow_function", "method_definition"}:
                declaration = node
                break
            if node.type == "variable_declarator" and declaration is None:
                declaration = node
            node = node.parent
        if declaration is None:
            raise ValueError("Reference has no complete function or declaration owner")
        start, end = declaration.start_byte, declaration.end_byte
        return {"module_id": "esm-declaration@" + str(start), "owner_kind": declaration.type,
                "character_start": len(raw[:start].decode()), "character_end": len(raw[:end].decode()),
                "body": raw[start:end].decode()}
    literal = r'(?P<literal>"(?:[^"\\]|\\.)*")'
    pattern = re.compile(r'(?:\burl\s*:\s*|\bwindow\.open\(\s*)' + literal)
    for match in pattern.finditer(text):
        value = json.loads(match["literal"])
        parsed = urllib.parse.urlsplit(value)
        if (not value.startswith("/") or value.startswith("//") or parsed.netloc
                or not parsed.path.endswith("/") or "/products/" in parsed.path or "/assets/" in parsed.path):
            continue
        if match[0].lstrip().startswith("url"):
            # A generic API/config URL is not an application entry. Require
            # the original window descriptor, rather than guessing from a
            # route's spelling or treating arbitrary URL prefixes as pages.
            start = text.rfind("{", 0, match.start())
            try:
                end = families.javascript_delimited(text, start)
            except ValueError:
                continue
            descriptor = text[start:end]
            if end <= match.end() or not all(re.search(r'\b' + name + r'\s*:', descriptor)
                                             for name in ("windowName", "openParam")):
                continue
        else:
            descriptor = None
        try:
            if modules is None:
                modules = families.keyboard_source_modules(text)
            owners = [row for row in modules if row["character_start"] <= match.start() < row["character_end"]]
            owner = (dict(min(owners, key=lambda row: row["character_end"] - row["character_start"]))
                     if owners else nonwebpack_owner(match.start()))
        except ValueError as error:
            unresolved.append({"kind": "host_app_reference_owner_unresolved", "literal": value,
                               "start_byte": len(text[:match.start()].encode()), "error": str(error)})
            continue
        owner["start_byte"] = len(text[:owner["character_start"]].encode())
        owner["end_byte"] = len(text[:owner["character_end"]].encode())
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        if text[match.end():].lstrip().startswith(("+", ".", "[")):
            try:
                offset = len(text[:match.start("literal")].encode())
                expression = parsed_source().root_node.descendant_for_byte_range(offset, offset)
                while expression.parent is not None and expression.parent.type not in {"arguments", "pair"}:
                    expression = expression.parent
                if expression.parent is None:
                    raise ValueError("Dynamic URL expression has no property or call argument boundary")
                body = raw[expression.start_byte:expression.end_byte]
                unresolved.append({"kind": "host_app_dynamic_url_suffix", "literal_prefix": value,
                    "literal_reference": match[0], "window_descriptor": descriptor,
                    "start_byte": len(text[:match.start()].encode()), "complete_owner": owner,
                    "expression": {"start_byte": expression.start_byte, "end_byte": expression.end_byte,
                                   "syntax": expression.type, "body": body.decode(),
                                   "sha256": hashlib.sha256(body).hexdigest()},
                    "scope": "Literal is only a prefix; exact URL requires caller/export data"})
            except ValueError as error:
                unresolved.append({"kind": "host_app_dynamic_expression_unresolved", "literal_prefix": value,
                    "start_byte": len(text[:match.start()].encode()), "complete_owner": owner, "error": str(error)})
            continue
        found.append({"url_literal": value, "literal_reference": match[0],
                      "window_descriptor": descriptor,
                      "start_byte": len(text[:match.start()].encode()), "complete_owner": owner,
                      "mount_and_local_feature_semantics": "requires_call_chain_audit"})
    return found, unresolved


def host_app_static_url(reference, original, files, audit):
    """Resolve only literal concatenation and verified webpack string exports.

    Runtime locations, calls and reassigned bindings remain unresolved. The
    complete caller, import declaration, factory, export getter and literal
    declaration are retained through the shared byte-verified source store.
    """
    raw = audit.read(original)
    expression = reference["expression"]
    start, end = expression["start_byte"], expression["end_byte"]
    if raw[start:end].decode() != expression["body"] or hashlib.sha256(raw[start:end]).hexdigest() != expression["sha256"]:
        raise ValueError("Dynamic application URL expression differs from original bytes")
    node = audit.tree(original).root_node.descendant_for_byte_range(start, end - 1)
    if (node.start_byte, node.end_byte) != (start, end):
        raise ValueError("Dynamic application URL expression has no exact syntax boundary")
    links = []
    active = set()

    def declaration(source, name, offset):
        binding = audit.binding("application_url_binding", source, name, offset)
        if binding["resolution"] != "declaration" or binding["syntax"] != "variable_declarator":
            raise ValueError("Application URL binding is absent, ambiguous or runtime-dependent: " + name)
        tree = audit.tree(source)
        variable = tree.root_node.descendant_for_byte_range(binding["start_byte"], binding["end_byte"] - 1)
        if variable.type != "variable_declarator" or variable.child_by_field_name("value") is None:
            raise ValueError("Application URL binding has no complete initializer")
        # An initializer is not a constant if the same lexical binding is
        # assigned again. Shadowed names in other functions do not count.
        pending = [tree.root_node]
        while pending:
            candidate = pending.pop()
            pending.extend(candidate.named_children)
            if candidate.type not in {"assignment_expression", "augmented_assignment_expression", "update_expression"}:
                continue
            target = candidate.child_by_field_name("left") or candidate.child_by_field_name("argument")
            targets = [target] if target is not None else []
            while targets:
                target = targets.pop()
                targets.extend(target.named_children)
                if target.type in {"identifier", "shorthand_property_identifier_pattern"} and target.text.decode() == name:
                    other = audit.binding("application_url_binding_write", source, name, target.start_byte)
                    if other.get("start_byte") == binding["start_byte"]:
                        raise ValueError("Application URL binding is reassigned or mutated: " + name)
        links.append({"original": {key: source[key] for key in ("path", "bytes", "sha256")}, "binding": binding})
        return variable

    def value(source, current):
        key = source["sha256"], current.start_byte, current.end_byte
        if key in active:
            raise ValueError("Application URL binding cycle")
        active.add(key)
        try:
            if current.type == "string":
                decoded = json.loads(current.text)
                if not isinstance(decoded, str):
                    raise ValueError("Application URL literal is not a string")
                return decoded
            if current.type == "parenthesized_expression" and len(current.named_children) == 1:
                return value(source, current.named_children[0])
            if current.type == "binary_expression" and current.child_by_field_name("operator").text == b"+":
                return value(source, current.child_by_field_name("left")) + value(source, current.child_by_field_name("right"))
            if current.type == "identifier":
                variable = declaration(source, current.text.decode(), current.start_byte)
                return value(source, variable.child_by_field_name("value"))
            if current.type == "member_expression":
                obj, prop = current.child_by_field_name("object"), current.child_by_field_name("property")
                if obj is None or obj.type != "identifier" or prop is None or prop.type != "property_identifier":
                    raise ValueError("Application URL member requires runtime/caller analysis: " + current.text.decode())
                variable = declaration(source, obj.text.decode(), obj.start_byte)
                call = variable.child_by_field_name("value")
                if call.type != "call_expression":
                    raise ValueError("Application URL member is not a webpack import")
                factory = variable.parent
                while factory is not None and factory.type not in {"arrow_function", "function_expression", "method_definition"}:
                    factory = factory.parent
                parameters = factory.child_by_field_name("parameters") if factory is not None else None
                params = parameters.named_children if parameters is not None else []
                callee, arguments = call.child_by_field_name("function"), call.child_by_field_name("arguments")
                args = arguments.named_children if arguments is not None else []
                if (len(params) != 3 or params[2].type != "identifier" or callee is None
                        or callee.type != "identifier" or callee.text != params[2].text
                        or len(args) != 1 or args[0].type != "number"):
                    raise ValueError("Application URL import has no verified webpack require signature")
                require_binding = audit.binding("application_url_require_parameter", source, callee.text.decode(), callee.start_byte)
                if (require_binding["resolution"] != "parameter_requires_caller_analysis"
                        or require_binding["scope"]["start_byte"] != factory.start_byte
                        or require_binding["scope"]["end_byte"] != factory.end_byte):
                    raise ValueError("Application URL require is shadowed or rebound")
                module_id = audit.families.javascript_integer_literal(args[0].text.decode())
                exported = audit.webpack_export("application_url_export", files, module_id, prop.text.decode())
                links.append(exported)
                target = exported["original"]
                binding = exported["binding"]
                variable = declaration(target, binding["name"], binding["reference_byte"])
                return value(target, variable.child_by_field_name("value"))
            raise ValueError("Application URL expression requires runtime/caller analysis: " + current.text.decode())
        finally:
            active.remove(key)

    resolved = value(original, node)
    parsed = urllib.parse.urlsplit(resolved)
    if not resolved.startswith("/") or resolved.startswith("//") or parsed.netloc:
        raise ValueError("Resolved application URL is not an absolute same-origin path")
    return {"url_literal": resolved, "resolution": "verified_static_string_expression", "binding_chain": links}


def host_app_deployment_audit(acquisition, deployment):
    """Audit real retained app versions/resources without equating host versions."""
    versions, unavailable, changed, unreferenced, probes = [], [], [], [], []
    requested = set(acquisition.get("requested_urls", []))
    undeclared_probes = {row["url"] for row in acquisition.get("probes", []) if row.get("declared") is False}
    for original in acquisition["files"]:
        current_request = not requested or original["url"] in requested
        current_reference = current_request and original["url"] not in undeclared_probes
        if not current_request:
            unreferenced.append(original)
        if original["url"] in undeclared_probes:
            probes.append(original)
        if "path" not in original:
            if current_reference:
                unavailable.append({**original, "source_map": urllib.parse.urlsplit(original["url"]).path.endswith(".map")})
            continue
        source = resolve(original["path"], ".analysis/inputs")
        if source.stat().st_size != original["bytes"] or sha(source) != original["sha256"]:
            changed.append({"path": original["path"], "error": "Retained bytes differ from the app acquisition receipt"})
            continue
        if not current_reference:
            continue
        if Path(urllib.parse.urlsplit(original["url"]).path).name not in {"manifest.json", "version.json", "package.json"}:
            continue
        metadata = json.loads(source.read_bytes())
        if isinstance(metadata, dict) and any(key in metadata for key in ("version", "buildVersion", "gitMetadata")):
            versions.append({"original": {key: original[key] for key in ("path", "bytes", "sha256", "url")},
                             "declaration": {key: metadata[key] for key in
                                ("name", "short_name", "version", "buildVersion", "gitMetadata", "deviceName", "dependencies", "resourceManifest")
                                if key in metadata}})
    failures = deployment.get("failures", [])
    runtime_missing = [row for row in unavailable if not row["source_map"]]
    return {"declared_versions": versions, "version_namespace": "Actual app declarations; independent of the native host release number",
            "all_retained_bytes_verified": not changed, "changed_inputs": changed,
            "retained_without_current_request": unreferenced,
            "undeclared_availability_probes": probes,
            "deployment_stable": acquisition.get("source_current_during_acquisition") is True,
            "reference_parser_version": acquisition.get("reference_parser_version"),
            "unavailable_source_maps": [row for row in unavailable if row["source_map"]],
            "unavailable_non_map_resources": runtime_missing,
            "declared_non_map_resources_available": not changed and not runtime_missing and not failures,
            "unresolved_static_references": acquisition.get("unresolved", []),
            "resource_dependency_closure_complete": (not changed and not unavailable and not failures
                and not acquisition.get("unresolved") and acquisition.get("source_current_during_acquisition") is True),
            "mounted_component_tree_semantics": "pending", "runtime_acceptance": "unverified"}


def acquire_host_apps(args):
    """Follow source-declared host windows through the shared acquisition path."""
    import evidence
    import families
    import host
    from types import SimpleNamespace
    result_path = ROOT / ".analysis/results/frontend/host-applications.json"
    result = {"schema_version": 1, "recorded_at_utc": stamp(), "tool": families.identity(Path(__file__)),
              "source_sets": [], "applications": {}, "failures": [], "target_execution": False,
              "semantics_complete": False, "runtime_acceptance": "unverified",
              "scope": "Literal and verified static-string window descriptors reachable from current acquired host application sources; runtime expressions and native-created windows require separate tracing"}
    selected = set(args.source_set or [])
    frontier = []
    for path in sorted((ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json")):
        if selected and path.parent.name not in selected:
            continue
        if re.fullmatch(r'(?:mouse|keyboard|audio|gamepad|system|accessories)(?:-mw)?-\d+-current', path.parent.name):
            continue
        retained = json.loads(path.read_bytes())
        if retained.get("source_current_during_acquisition") and "installed-" not in path.parent.name:
            frontier.append(path)
    scanned, acquired, parsed = set(), set(), {}
    source_sets = {}
    while frontier:
        current, frontier = frontier, []
        for receipt_path in current:
            if receipt_path in scanned:
                continue
            scanned.add(receipt_path)
            acquisition = json.loads(receipt_path.read_bytes())
            deployment = evidence.frontend_deployment_sources(acquisition)
            record = {"acquisition": families.identity(receipt_path), "deployment": deployment,
                      "version_and_resource_audit": host_app_deployment_audit(acquisition, deployment),
                      "references": [], "unresolved": [], "dynamic_url_owners": []}
            result["source_sets"].append(record)
            source_sets[receipt_path.parent.name] = receipt_path.parent
            if acquisition.get("source_current_during_acquisition") is not True:
                result["failures"].append({"source_set": receipt_path.parent.name, "error": "Deployment stability is unverified"})
                continue
            files = [source["original"] for source in deployment["sources"] if source["identity_verified"]]
            dynamic_audit = None
            for source in deployment["sources"]:
                original = source["original"]
                if not source["identity_verified"] or not original["path"].endswith(".js"):
                    continue
                if original["sha256"] not in parsed:
                    parsed[original["sha256"]] = host_app_descriptors((ROOT / original["path"]).read_text(encoding="utf-8-sig"))
                literal_references, unresolved = parsed[original["sha256"]]
                references = list(literal_references)
                for pending in unresolved:
                    if pending["kind"] != "host_app_dynamic_url_suffix":
                        record["unresolved"].append({"original": original, **pending})
                        continue
                    if dynamic_audit is None:
                        dynamic_audit = host.HostLocalAudit()
                    try:
                        resolved = host_app_static_url(pending, original, files, dynamic_audit)
                        references.append({"url_literal": resolved["url_literal"],
                            "literal_reference": pending["literal_reference"],
                            "window_descriptor": pending["window_descriptor"],
                            "start_byte": pending["start_byte"], "complete_owner": pending["complete_owner"],
                            "dynamic_url_expression": pending["expression"], "static_url_resolution": resolved,
                            "mount_and_local_feature_semantics": "requires_call_chain_audit"})
                    except ValueError as error:
                        record["unresolved"].append({"original": original, **pending, "static_resolution_error": str(error)})
                for reference in references:
                    url = artifact_url(acquisition["entry_url"], reference["url_literal"])
                    if url is None or urllib.parse.urlsplit(url).netloc != "apps.razer.com":
                        continue
                    link = {"original": original, "deployment_role": source["deployment_role"], **reference}
                    record["references"].append(link)
                    slug = "-".join(part for part in urllib.parse.urlsplit(url).path.split("/") if part)
                    if not slug or not re.fullmatch(r"[A-Za-z0-9_-]+", slug):
                        result["failures"].append({"url": url, "error": "Application namespace requires explicit mapping"})
                        continue
                    if urllib.parse.urlsplit(url).query:
                        # Query-dependent windows retain the exact URL identity.
                        slug += "-" + hashlib.sha256(urllib.parse.urlsplit(url).query.encode()).hexdigest()[:12]
                    input_name = ".analysis/inputs/frontend/" + slug + "-current"
                    existing_receipt = ROOT / input_name / "acquisition.json"
                    if existing_receipt.is_file() and json.loads(existing_receipt.read_bytes()).get("entry_url") != url:
                        # /app/ and /app/index.html are distinct original URL
                        # identities until their actual responses establish an
                        # alias. Preserve both receipts instead of rewriting one.
                        input_name = ".analysis/inputs/frontend/" + slug + "-entry-" + hashlib.sha256(url.encode()).hexdigest()[:12] + "-current"
                    app = result["applications"].setdefault(url, {"url": url,
                        "input": input_name, "references": [],
                        "local_feature_classification": "requires_actual_call_chain_audit"})
                    app["references"].append({"source_set": receipt_path.parent.name,
                        "source": original, "owner_module": reference["complete_owner"]["module_id"],
                        "reference_start_byte": reference["start_byte"]})
            if dynamic_audit is not None:
                record["dynamic_url_owners"] = list(dynamic_audit.owners.values())
                changed = dynamic_audit.final_snapshot()
                if changed:
                    result["failures"].append({"source_set": receipt_path.parent.name,
                        "stage": "dynamic_url_bindings", "error": "Dynamic URL originals changed during analysis", "inputs": changed})
        pending = [row for url, row in result["applications"].items() if url not in acquired]
        for row in pending:
            acquired.add(row["url"])
            if args.stage in {"acquire", "all"}:
                try:
                    fetch_frontend(SimpleNamespace(url=row["url"], output=row["input"], no_refresh_gaps=True), refresh_ledger=False)
                except Exception as error:
                    row["error"] = str(error)
                    result["failures"].append({"url": row["url"], "stage": "acquire", "error": str(error)})
            receipt = ROOT / row["input"] / "acquisition.json"
            if receipt.is_file():
                row["acquisition"] = families.identity(receipt)
                frontier.append(receipt)
            save(result_path, result)
    if args.stage in {"analyze", "all"}:
        for name, input_dir in sorted(source_sets.items()):
            if not (input_dir / "acquisition.json").is_file():
                continue
            capture = ROOT / ".analysis/results/frontend" / name / "analysis.json"
            try:
                analyze_frontend(SimpleNamespace(input=rel(input_dir), refresh=capture.is_file(), no_refresh_gaps=True), refresh_ledger=False)
            except Exception as error:
                result["failures"].append({"source_set": name, "stage": "analyze", "error": str(error)})
            save(result_path, result)
    result["finished_at_utc"] = stamp()
    result["snapshot_changed_receipts"] = [row["acquisition"]["path"] for row in result["source_sets"]
                                            if not families.valid(row["acquisition"])]
    result["snapshot_unchanged"] = not result["snapshot_changed_receipts"]
    stable_host = ROOT / ".analysis/inputs/host-current/metadata-recheck.json"
    if stable_host.is_file():
        result["native_host_metadata_context"] = {"original": families.identity(stable_host),
            "stable_products": json.loads(stable_host.read_bytes()).get("stable_products", []),
            "scope": "Native host metadata context only; frontend app version declarations remain independent"}
    result["summary"] = {"source_sets": len(result["source_sets"]), "application_references": len(result["applications"]),
                         "failures": len(result["failures"]), "unresolved_owner_references": sum(len(row["unresolved"]) for row in result["source_sets"])}
    save(result_path, result)
    print(json.dumps({"receipt": rel(result_path), **result["summary"]}), flush=True)
    if not getattr(args, "no_refresh_gaps", False):
        refresh_gaps()
    return result


def analyze_frontend(args, *, refresh_ledger=True):
    import frontend
    workers = getattr(args, "workers", 2)
    if type(workers) is not int or not 2 <= workers <= 8:
        raise ValueError("Frontend analysis workers must be between two and eight")
    if getattr(args, "recover_missing", False) or getattr(args, "all_acquired", False):
        return frontend.recover_frontend(args)
    refresh_ledger = refresh_ledger and not getattr(args, "no_refresh_gaps", False)
    input_dir = resolve(args.input, ".analysis/inputs")
    acquisition = json.loads((input_dir / "acquisition.json").read_bytes())
    output = ROOT / ".analysis/results/frontend" / input_dir.name
    output.mkdir(parents=True, exist_ok=True)
    unpacked = output / "modules"
    if unpacked.exists() and any(unpacked.iterdir()) and not getattr(args, "refresh", False):
        raise ValueError("Recovered modules already exist; inspect analysis.json")
    tool = wakaru()
    records = [r for r in acquisition["files"] if "path" in r]
    for r in records:
        path = ROOT / r["path"]
        if sha(path) != r["sha256"]:
            raise ValueError("Original frontend input changed")
    if getattr(args, "refresh", False):
        # Only replace generated recovery trees after verifying every original.
        # Keep acquisition/chunk-enumeration receipts and never remove inputs.
        for name in ("modules", "original-sources", "registration-inputs", "lexical-inputs"):
            generated_tree = resolve(rel(output / name), ".analysis/results/frontend")
            if generated_tree.parent != output.resolve():
                raise ValueError("Recovery output escaped product directory")
            if generated_tree.exists():
                shutil.rmtree(generated_tree)
    inputs = [ROOT / r["path"] for r in records
              if Path(urllib.parse.urlsplit(r["url"]).path).suffix.lower() in {".js", ".mjs"}]
    commands = []
    def command(label, argv, cwd=ROOT):
        stdout = output / (label+".stdout.txt")
        stderr = output / (label+".stderr.txt")
        print(json.dumps({"analyzing": label}), flush=True)
        with stdout.open("wb") as out, stderr.open("wb") as err:
            completed = subprocess.run(argv, cwd=cwd, stdout=out, stderr=err, check=False)
        row = {"stage": label, "argv": [rel(Path(a)) if Path(a).is_absolute() and Path(a).is_relative_to(ROOT) else a for a in argv],
               "cwd": rel(cwd), "exit_code": completed.returncode,
               "stdout": rel(stdout), "stderr": rel(stderr)}
        commands.append(row)
        return row
    # Keep every JS explicit: Wakaru's directory scanner skips ordinary scripts.
    # Relative arguments avoid Windows' command-line limit without omitting them.
    retained_input_root = frontend.input_root(input_dir, inputs)
    unpack_cwd = retained_input_root
    unpack_args = [str(tool), *(os.path.relpath(p, unpack_cwd) for p in inputs),
                   "--unpack", "--level", "minimal", "--provenance",
                   "--emit-source-map", "--diagnostics", "--json", "-o", str(unpacked)]
    if os.name == "nt" and len(subprocess.list2cmdline(unpack_args)) >= 32000:
        unpack_cwd = Path(os.path.commonpath([path.parent for path in inputs]))
        if not unpack_cwd.is_relative_to(retained_input_root):
            raise ValueError("Frontend input common directory escaped retained originals")
        unpack_args = [str(tool), *(os.path.relpath(path, unpack_cwd) for path in inputs),
                       "--unpack", "--level", "minimal", "--provenance",
                       "--emit-source-map", "--diagnostics", "--json", "-o", str(unpacked)]
        if len(subprocess.list2cmdline(unpack_args)) >= 32000:
            raise ValueError("Complete explicit Wakaru input list still exceeds Windows command-line limit")
    command("unpack", unpack_args, cwd=unpack_cwd)
    if unpacked.exists():
        command("validate", [str(tool), "debug", "validate", str(unpacked), "--input", str(retained_input_root), "--json"])
    map_inputs = [r for r in records if r["url"].endswith(".map")]
    maps = []

    def extract_map(r):
        p = ROOT / r["path"]
        data = json.loads(p.read_bytes())
        contents = data.get("sourcesContent", []) or []
        map_row = {"input": r["path"], "source_count": len(data.get("sources", [])),
                   "embedded_sources": sum(s is not None for s in contents),
                   "indexed_sections": len(data.get("sections", []))}
        if not any(s is not None for s in contents):
            return map_row, None
        identifier = hashlib.sha256(r["url"].encode()).hexdigest()[:24]
        label = "map-" + identifier
        stdout = output / (label + ".stdout.txt")
        stderr = output / (label + ".stderr.txt")
        argv = [str(tool), "extract", str(p), "-o", str(output / "original-sources" / identifier)]
        with stdout.open("wb") as out, stderr.open("wb") as err:
            completed = subprocess.run(argv, cwd=ROOT, stdout=out, stderr=err, check=False)
        command_row = {"stage": label,
                       "argv": [rel(Path(value)) if Path(value).is_absolute() and Path(value).is_relative_to(ROOT) else value
                                for value in argv],
                       "cwd": rel(ROOT), "exit_code": completed.returncode,
                       "stdout": rel(stdout), "stderr": rel(stderr)}
        return map_row, command_row

    if map_inputs:
        map_workers = max(2, min(workers, len(map_inputs)))
        with concurrent.futures.ThreadPoolExecutor(max_workers=map_workers) as pool:
            for map_row, command_row in pool.map(extract_map, map_inputs):
                maps.append(map_row)
                if command_row is not None:
                    commands.append(command_row)
    generated = [{"path": rel(p), "bytes": p.stat().st_size, "sha256": sha(p)}
                 for p in sorted(output.rglob("*")) if p.is_file() and p.name != "analysis.json"]
    unchanged = all(sha(ROOT / r["path"]) == r["sha256"] for r in records)
    result = {"schema_version": 1, "input": rel(input_dir), "recorded_at_utc": stamp(),
              "tool": {"name": "Wakaru", "version": "1.14.0", "sha256": sha(tool)},
              "original_inputs": [{key: r[key] for key in ("path", "bytes", "sha256")} for r in records],
              "original_js_inputs": [rel(p) for p in inputs], "input_bytes_unchanged": unchanged,
              "source_maps": maps, "commands": commands, "outputs": generated,
              "target_execution": False, "semantic_audit": "pending", "implementation_acceptance": "pending"}
    save(output / "analysis.json", result)
    recovery = frontend.recover_frontend(SimpleNamespace(input=rel(input_dir), no_refresh_gaps=True))
    if any(row["status"] == "failed" for row in recovery):
        raise ValueError("Frontend recovery failed; analyzer receipts and exact derived inputs retained")
    print(json.dumps({"result": rel(output / "analysis.json"), "js_inputs": len(inputs),
                      "generated_files": len(generated), "failed_stages": sum(bool(c["exit_code"]) for c in commands),
                      "input_bytes_unchanged": unchanged,
                      "gap_refresh": "pending" if refresh_ledger else "deferred_to_batch"}), flush=True)
    if refresh_ledger:
        try:
            refresh_gaps()
        except Exception as error:
            raise RuntimeError(f"Frontend analysis receipt retained; gaps refresh failed: {error}") from error


def cabinet_layout(raw, offset, size):
    """Read CAB tables and CFDATA byte ranges, without decompressing payloads.

    A literal MSCF header can occur inside another cabinet's compressed stream.
    Keep that observation separate from a verified, independently extractable
    container. Structural success here does not verify compression or checksums.
    """
    import struct
    result = {"complete": False, "folders": [], "files": [],
              "compression_verified": False, "checksums_verified": False}
    try:
        if offset < 0 or size < 36 or offset + size > len(raw):
            raise ValueError("CAB byte range is outside the verified parent")
        body = memoryview(raw)[offset:offset + size]
        signature, r1, declared, r2, files_offset, r3, minor, major, folder_count, file_count, flags, _, _ = \
            struct.unpack_from("<4sIIIII BBHHHHH", body)
        if signature != b"MSCF" or (r1 | r2 | r3) or declared != size or (major, minor) != (1, 3) or flags & ~7:
            raise ValueError("CAB fixed header differs from the bounded candidate")

        def bounded(start, count):
            if start < 0 or count < 0 or start + count > size:
                raise ValueError("CAB table or CFDATA range exceeds the candidate")
            return body[start:start + count]

        def zero_string(start):
            end = raw.find(b"\0", offset + start, offset + size)
            if end < 0:
                raise ValueError("CAB string has no terminator within the candidate")
            return raw[offset + start:end], end + 1 - offset

        cursor, header_reserve, folder_reserve, data_reserve = 36, 0, 0, 0
        if flags & 4:
            header_reserve, folder_reserve, data_reserve = struct.unpack("<HBB", bounded(cursor, 4))
            bounded(cursor + 4, header_reserve)
            cursor += 4 + header_reserve
        for flag in (1, 2):
            if flags & flag:
                _, cursor = zero_string(cursor)
                _, cursor = zero_string(cursor)
        for number in range(folder_count):
            start, blocks, compression = struct.unpack("<IHH", bounded(cursor, 8))
            bounded(cursor + 8, folder_reserve)
            cursor += 8 + folder_reserve
            method = compression & 15
            if method not in {0, 1, 2, 3}:
                raise ValueError("CAB folder compression method is not a CAB-defined value")
            result["folders"].append({"index": number, "data_offset": start, "blocks_count": blocks,
                "compression_type": compression, "compression_method": {0: "none", 1: "MSZip", 2: "Quantum", 3: "LZX"}[method],
                "compressed": method != 0, "data_blocks": []})
        if files_offset < cursor:
            raise ValueError("CAB file table overlaps its header or folder table")
        cursor = files_offset
        for number in range(file_count):
            length, start, folder, date, time, attributes = struct.unpack("<IIHHHH", bounded(cursor, 16))
            name, cursor = zero_string(cursor + 16)
            if folder >= folder_count and folder not in {0xfffd, 0xfffe, 0xffff}:
                raise ValueError("CAB file references a nonexistent folder")
            result["files"].append({"index": number, "name_raw_hex": name.hex(), "bytes": length,
                "uncompressed_offset": start, "folder": folder, "date": date, "time": time, "attributes": attributes})
        ranges = []
        for folder in result["folders"]:
            current = folder["data_offset"]
            if current < cursor:
                raise ValueError("CAB CFDATA overlaps the complete file table")
            for number in range(folder["blocks_count"]):
                checksum, compressed, uncompressed = struct.unpack("<IHH", bounded(current, 8))
                payload = current + 8 + data_reserve
                bounded(current + 8, data_reserve + compressed)
                if uncompressed > 32768:
                    raise ValueError("CAB CFDATA uncompressed size exceeds the 32 KiB format limit")
                ranges.append((current, payload + compressed))
                folder["data_blocks"].append({"index": number, "header_offset": offset + current,
                    "payload_offset": offset + payload, "compressed_bytes": compressed,
                    "uncompressed_bytes": uncompressed, "checksum": hex(checksum),
                    "header_raw_hex": bounded(current, 8 + data_reserve).hex()})
                current = payload + compressed
        ordered = sorted(ranges)
        if any(left[1] > right[0] for left, right in zip(ordered, ordered[1:])):
            raise ValueError("CAB CFDATA ranges overlap across folders")
        result.update(complete=True, file_table_end=cursor, header_reserve_bytes=header_reserve,
                      folder_reserve_bytes=folder_reserve,
                      data_reserve_bytes=data_reserve)
    except (ValueError, struct.error) as error:
        result["failure"] = str(error)
    return result


def embedded_cabinets(raw):
    """Enumerate bounded CAB headers; decompression remains a static tool step."""
    import struct
    rows, position = [], 0
    while True:
        offset = raw.find(b"MSCF", position)
        if offset < 0:
            break
        position = offset + 4
        if offset + 36 > len(raw):
            continue
        signature, reserved1, size, reserved2, files, reserved3, minor, major, folders, count, flags, set_id, index = \
            struct.unpack_from("<4sIIIII BBHHHHH", raw, offset)
        if (reserved1 or reserved2 or reserved3 or major != 1 or minor != 3
                or not folders or not count or flags & ~7 or files < 36 or files >= size
                or size < 36 or offset + size > len(raw)):
            continue
        rows.append({"offset": offset, "bytes": size, "files_offset": files, "folders": folders,
                     "files": count, "flags": flags, "set_id": set_id, "cabinet_index": index,
                     "layout": cabinet_layout(raw, offset, size),
                     "validation": "bounded CAB header only; static extractor must validate complete content"})
    for candidate in rows:
        candidate["containing_compressed_blocks"] = []
        for outer in rows:
            if outer is candidate or not outer["layout"]["complete"]:
                continue
            for folder in outer["layout"]["folders"]:
                if not folder["compressed"]:
                    continue
                for block in folder["data_blocks"]:
                    start, end = block["payload_offset"], block["payload_offset"] + block["compressed_bytes"]
                    if start <= candidate["offset"] and candidate["offset"] + 36 <= end:
                        candidate["containing_compressed_blocks"].append({
                            "outer_offset": outer["offset"], "outer_bytes": outer["bytes"],
                            "outer_sha256": hashlib.sha256(raw[outer["offset"]:outer["offset"] + outer["bytes"]]).hexdigest(),
                            "folder_index": folder["index"], "compression": folder["compression_method"],
                            "block": block,
                            "candidate_entirely_within_block": candidate["offset"] + candidate["bytes"] <= end,
                            "meaning": "The fixed MSCF header occurs inside compressed CFDATA; it does not establish an independent cabinet"})
    return rows


def prepare_reviewed_embedded_pe_inputs(specs, originals, prefix):
    """Check every explicit PE review before publishing any extracted originals."""
    import host
    prepared, destinations, verified = [], set(), {}

    def check_identity(row, boundary):
        if not isinstance(row, dict) or set(row) != {"path", "bytes", "sha256"}:
            raise ValueError("Embedded PE source identity fields are invalid")
        name = row["path"]
        if (not isinstance(name, str) or not name.startswith(boundary + "/")
                or "\\" in name or ":" in name or any(p in {"", ".", ".."} for p in name.split("/"))
                or type(row["bytes"]) is not int or row["bytes"] <= 0
                or not isinstance(row["sha256"], str) or re.fullmatch(r"[0-9a-f]{64}", row["sha256"]) is None):
            raise ValueError("Embedded PE source requires a complete project-relative identity")
        path = resolve(name, boundary)
        try:
            raw = path.read_bytes()
        except OSError as error:
            raise ValueError("Embedded PE source is unavailable: " + name) from error
        if len(raw) != row["bytes"] or hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError("Embedded PE source byte identity differs: " + name)
        verified[name] = dict(row)
        return raw

    if not isinstance(specs, list):
        raise ValueError("Embedded PE reviews must be a list of explicit references")
    for spec in specs:
        if not isinstance(spec, str):
            raise ValueError("Embedded PE review reference must be a string")
        name, marker, pointer = spec.partition("#")
        if (not name.startswith(".analysis/results/") or "\\" in name or ":" in name
                or any(p in {"", ".", ".."} for p in name.split("/"))):
            raise ValueError("Embedded PE review must be a retained project-relative result")
        review_path = resolve(name, ".analysis/results")
        try:
            review_raw = review_path.read_bytes()
        except OSError as error:
            raise ValueError("Embedded PE review is unavailable: " + name) from error
        reference = {"path": rel(review_path), "bytes": len(review_raw),
                     "sha256": hashlib.sha256(review_raw).hexdigest()}
        verified[reference["path"]] = reference
        value = json.loads(review_raw)
        if marker and pointer:
            if not pointer.startswith("/") or re.search(r"~(?![01])", pointer):
                raise ValueError("Embedded PE review requires an RFC 6901 JSON pointer")
            for token in pointer[1:].split("/"):
                token = token.replace("~1", "/").replace("~0", "~")
                try:
                    if isinstance(value, list):
                        if re.fullmatch(r"0|[1-9][0-9]*", token) is None:
                            raise ValueError("Invalid JSON array index")
                        value = value[int(token)]
                    else:
                        value = value[token]
                except (KeyError, IndexError, TypeError, ValueError) as error:
                    raise ValueError("Embedded PE review JSON pointer does not resolve") from error
        groups = value if isinstance(value, list) else [value]
        if not groups:
            raise ValueError("Embedded PE review contains no range groups")
        for group in groups:
            if (not isinstance(group, dict) or set(group) != {"schema_version", "source_input", "ranges"}
                    or type(group["schema_version"]) is not int or group["schema_version"] != 1
                    or not isinstance(group["ranges"], list) or not group["ranges"]):
                raise ValueError("Embedded PE review group schema is invalid")
            claim = group["source_input"]
            raw = check_identity(claim, ".analysis/inputs")
            parent = originals.get(claim["path"])
            if (parent is None or not claim["path"].startswith(prefix)
                    or any(parent.get(key) != claim[key] for key in ("bytes", "sha256"))
                    or parent.get("container_extraction_complete") is False
                    or parent.get("source_bytes_verified") is False):
                raise ValueError("Embedded PE parent is outside the verified indexed scope")
            ranges = host.inspect_embedded_pe_ranges(raw, group["ranges"])
            for item in ranges:
                for evidence in item["review"]["evidence"]:
                    check_identity(evidence, ".analysis")
                destination = (ROOT / ".analysis/inputs/official-native-current/embedded"
                               / claim["sha256"] / item["name"])
                key = (claim["sha256"], item["name"].casefold())
                if key in destinations:
                    raise ValueError("Embedded PE reviews publish the same child more than once")
                destinations.add(key)
                body = raw[item["offset"]:item["offset"] + item["bytes"]]
                if destination.exists() and destination.read_bytes() != body:
                    raise ValueError("Existing embedded original differs from reviewed PE bytes")
            frozen = ROOT / ".analysis/results/native-packages/review-sources" / (reference["sha256"] + ".json")
            prepared.append({"parent": parent, "raw": raw, "ranges": ranges, "review_raw": review_raw,
                "review_origin": reference, "review_pointer": pointer if marker else "",
                "review_source": {**reference, "path": rel(frozen)}})
    # Detect a changing parent or evidence source before creating any child.
    for row in verified.values():
        check_identity(row, ".analysis")
    return prepared


def retain_embedded_review_source(raw):
    """Keep exact reviewed/report bytes when their mutable owner is updated."""
    digest = hashlib.sha256(raw).hexdigest()
    path = ROOT / ".analysis/results/native-packages/review-sources" / (digest + ".json")
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("xb") as stream:
            stream.write(raw)
    except FileExistsError:
        if path.read_bytes() != raw:
            raise ValueError("Retained embedded review source bytes changed")
    return {"path": rel(path), "bytes": len(raw), "sha256": digest}


def extract_embedded_inputs(args):
    """Retain exact resource, CAB and reviewed PE ranges without loading targets."""
    import families
    import struct
    index_path = ROOT / ".analysis/inputs.json"
    index_raw = index_path.read_bytes()
    index = json.loads(index_raw)
    originals = {row["path"]: row for row in index["inputs"]}
    indexed_paths = set(originals)
    managed_path = ROOT / ".analysis/results/host-current/managed/capture.json"
    managed = json.loads(managed_path.read_bytes()) if managed_path.is_file() else {}
    prefix = getattr(args, "source_prefix", ".analysis/inputs/official-native-current/")
    if not prefix.startswith(".analysis/inputs/") or ".." in Path(prefix).parts:
        raise ValueError("Embedded input scope must remain within current originals")
    pe_reviews = prepare_reviewed_embedded_pe_inputs(getattr(args, "pe_review", None) or [], originals, prefix)
    output_path = ROOT / ".analysis/results/native-packages/embedded-inputs.json"
    previous_raw = output_path.read_bytes() if output_path.is_file() else None
    report = {"schema_version": 3, "recorded_at_utc": stamp(), "tool": families.identity(Path(__file__)),
              "input_index": {"path": rel(index_path), "sha256_at_start": hashlib.sha256(index_raw).hexdigest()},
              "source_prefix": prefix, "managed_capture": families.identity(managed_path) if managed_path.is_file() else None,
              "inputs": [], "failures": [], "scanned_portable_executables": [], "excluded_inputs": [], "pe_reviews": [],
              "target_execution": False,
              "scope": "Exact unique CLR resources with matching length prefix, structurally bounded CAB candidates, and explicit evidence-reviewed complete PE file ranges; original failures and earlier extraction scopes remain in the retained previous report, and installation/native semantics remain unverified"}
    new_rows, source_cache, extraction_cache, alias_cache = [], {}, {}, {}
    parents_by_sha = {}
    for row in originals.values():
        if row["path"].startswith(prefix):
            parents_by_sha.setdefault(row["sha256"], []).append(row)

    def parent_aliases(row):
        if row["sha256"] not in alias_cache:
            aliases = []
            for candidate in parents_by_sha.get(row["sha256"], [row]):
                if (candidate.get("container_extraction_complete") is False
                        or candidate.get("source_bytes_verified") is False):
                    continue
                if not families.valid(candidate):
                    report["failures"].append({"input": candidate["path"], "stage": "container_alias_identity",
                        "error": "Same-digest container alias differs from its indexed original bytes"})
                    continue
                aliases.append(candidate)
            # A host copy can precede hundreds of identical manifest-bound
            # driver copies. Preserve every verified edge instead of losing
            # family provenance when the PE body is scanned only once.
            aliases.sort(key=lambda item: (item.get("verified_against_manifest") is not True,
                item.get("container_extraction_complete") is not True, item["path"]))
            alias_cache[row["sha256"]] = aliases
        aliases = alias_cache[row["sha256"]]
        if not aliases:
            raise ValueError("Embedded source has no byte-verified container alias")
        return aliases

    def extraction_state(digest):
        if digest not in extraction_cache:
            path = ROOT / ".analysis/results/native-packages" / digest / "inspection.json"
            state = {"attempted": path.is_file(), "complete_recorded": False,
                     "actual_container_verified": False, "actual_outputs_verified": False, "current_complete": False}
            if path.is_file():
                inspection = json.loads(path.read_bytes())
                if inspection.get("container", {}).get("sha256") != digest:
                    raise ValueError("CAB extraction receipt identifies different input bytes")
                state.update(receipt=families.identity(path),
                    listing_exit_code=inspection.get("listing_exit_code"),
                    extraction_exit_code=inspection.get("extraction_exit_code"),
                    complete_recorded=inspection.get("extracted") is True and inspection.get("input_bytes_unchanged") is True)
                state["actual_container_verified"] = families.valid(inspection["container"])
                artifacts = (inspection.get("files", []) + inspection.get("outputs", [])
                             + inspection.get("reconstructed_outputs", []))
                state["actual_outputs_verified"] = bool(artifacts) and all(families.valid(row) for row in artifacts)
                state["current_complete"] = (state["complete_recorded"] and state["actual_container_verified"]
                                             and state["actual_outputs_verified"])
            extraction_cache[digest] = state
        return extraction_cache[digest]

    def source(row):
        if row.get("container_extraction_complete") is False or row.get("source_bytes_verified") is False:
            raise ValueError("Embedded parent comes from failed static decompression")
        if row["sha256"] not in source_cache:
            if not families.valid(row):
                raise ValueError("Embedded source bytes differ from input index")
            source_cache.clear()
            source_cache[row["sha256"]] = (ROOT / row["path"]).read_bytes()
        return source_cache[row["sha256"]]

    def retain(row, raw, offset, name, details):
        destination = ROOT / ".analysis/inputs/official-native-current/embedded" / row["sha256"] / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        if destination.exists() and destination.read_bytes() != raw:
            raise ValueError("Existing embedded original differs from verified byte range")
        destination.write_bytes(raw)
        aliases = parent_aliases(row)
        preferred = aliases[0]
        binding = {key: preferred[key] for key in ("verified_against_manifest", "declared_sha256", "declared_bytes",
                                             "declared_container_sha256", "container_extraction_complete",
                                             "source_bytes_verified") if key in preferred}
        artifact = {**families.identity(destination), **binding, "kind": "static_embedded_native_input",
                    "container": preferred["path"], "container_sha256": row["sha256"], "container_offset": offset,
                    "container_aliases": [{**{key: item[key] for key in ("path", "bytes", "sha256")},
                                            "byte_identity_verified": True} for item in aliases],
                    "byte_range_verified": True, "extraction": details}
        new_rows.append(artifact)
        report["inputs"].append(artifact)
        return artifact

    for reviewed in pe_reviews:
        retain_embedded_review_source(reviewed["review_raw"])
    if previous_raw is not None:
        report["previous_report"] = retain_embedded_review_source(previous_raw)
    for reviewed in pe_reviews:
        reference = reviewed["review_source"]
        report["pe_reviews"].append({"review_source": reference, "review_origin": reviewed["review_origin"],
            "review_pointer": reviewed["review_pointer"],
            "source_input": {key: reviewed["parent"][key] for key in ("path", "bytes", "sha256")}})
        for item in reviewed["ranges"]:
            offset, size = item["offset"], item["bytes"]
            retain(reviewed["parent"], reviewed["raw"][offset:offset + size], offset, item["name"],
                {"kind": "reviewed_embedded_pe_range", "pe": item["pe"], "layout": item["layout"],
                 "review": item["review"], "review_source": reference,
                 "review_origin": reviewed["review_origin"], "review_pointer": reviewed["review_pointer"]})

    resource_extensions = {".exe", ".dll", ".sys", ".zip", ".cab", ".msi", ".7z", ".bin", ".json", ".config", ".xml"}
    for assembly in managed.get("assemblies", []):
        parent = originals.get(assembly["input"])
        if parent is None or not parent["path"].startswith(prefix) or not assembly.get("capture_complete"):
            continue
        try:
            raw = source(parent)
            for output in assembly.get("outputs", []):
                path = ROOT / output["path"]
                if "/source/" not in output["path"] or path.suffix.lower() not in resource_extensions:
                    continue
                if not families.valid(output):
                    raise ValueError("Static managed output byte identity differs")
                body = path.read_bytes()
                if not body:
                    continue
                offset = raw.find(body)
                if (offset < 4 or raw.find(body, offset + 1) >= 0
                        or struct.unpack_from("<I", raw, offset - 4)[0] != len(body)):
                    continue
                artifact = retain(parent, raw[offset:offset + len(body)], offset, path.name,
                                  {"kind": "exact_length_prefixed_managed_resource",
                                   "analyzer_output": output, "length_prefix_offset": offset - 4})
                originals[artifact["path"]] = artifact
        except Exception as error:
            report["failures"].append({"input": parent["path"], "stage": "managed_resource", "error": str(error)})
    seen = set()
    for parent in list(originals.values()):
        if not parent["path"].startswith(prefix) or parent["sha256"] in seen:
            continue
        if parent.get("container_extraction_complete") is False or parent.get("source_bytes_verified") is False:
            report["excluded_inputs"].append({"path": parent["path"], "reason": "Unverified partial static decompression"})
            continue
        try:
            # SfxCA and Burn/MSI payloads can be DLLs or opaque a0/u4 files.
            # Only actual MZ bytes select PE candidates; verify their complete
            # indexed identity before inspecting or retaining any byte range.
            with resolve(parent["path"], ".analysis/inputs").open("rb") as stream:
                if stream.read(2) != b"MZ":
                    continue
            raw = source(parent)
            seen.add(parent["sha256"])
            report["scanned_portable_executables"].append({key: parent[key] for key in ("path", "bytes", "sha256")})
            for cabinet in embedded_cabinets(raw):
                offset, size = cabinet["offset"], cabinet["bytes"]
                for containing in cabinet["containing_compressed_blocks"]:
                    containing["outer_static_extraction"] = extraction_state(containing["outer_sha256"])
                retain(parent, raw[offset:offset + size], offset, "cabinet-" + str(offset) + ".cab",
                       {"kind": "bounded_cabinet_candidate", "cabinet": cabinet,
                        "static_extraction": extraction_state(hashlib.sha256(raw[offset:offset + size]).hexdigest())})
        except Exception as error:
            report["failures"].append({"input": parent["path"], "stage": "cabinet_range", "error": str(error)})
    report["input_index"]["unchanged_before_merge"] = sha(index_path) == report["input_index"]["sha256_at_start"]
    if new_rows:
        index_files(new_rows)
    report["summary"] = {"inputs": len(report["inputs"]), "failures": len(report["failures"]),
                         "newly_indexed_inputs": sum(row["path"] not in indexed_paths for row in report["inputs"]),
                         "portable_executables_scanned": len(report["scanned_portable_executables"]),
                         "excluded_partial_inputs": len(report["excluded_inputs"]),
                         "managed_resources": sum(row["extraction"]["kind"] == "exact_length_prefixed_managed_resource" for row in report["inputs"]),
                          "reviewed_portable_executables": sum(row["extraction"]["kind"] == "reviewed_embedded_pe_range" for row in report["inputs"]),
                         "cabinet_candidates": sum(row["extraction"]["kind"] == "bounded_cabinet_candidate" for row in report["inputs"]),
                         "cabinet_invalid_layouts": sum(row["extraction"]["kind"] == "bounded_cabinet_candidate"
                             and not row["extraction"]["cabinet"]["layout"]["complete"] for row in report["inputs"]),
                         "cabinet_headers_inside_compressed_cfdata": sum(row["extraction"]["kind"] == "bounded_cabinet_candidate"
                             and bool(row["extraction"]["cabinet"]["containing_compressed_blocks"]) for row in report["inputs"])}
    save(output_path, report)
    print(json.dumps({"receipt": rel(output_path), **report["summary"]}), flush=True)
    if not getattr(args, "no_refresh_gaps", False):
        refresh_gaps()
    return report


def corpus_current_lineage(index, groups, verified_stats=None):
    """Prove current PE provenance through actual official declarations and bytes.

    Installed paths, version strings and inherited verification flags are never
    roots. Equal-byte aliases transfer identity only after their bytes have been
    checked. Successful static extraction receipts establish individual edges;
    they do not establish installer or device semantics.
    """
    originals = {row["path"]: row for row in index["inputs"]}
    snapshots = dict(verified_stats or {})
    checked = {name: {key: row[key] for key in ("bytes", "sha256")}
               for row in groups for name in row.get("verified_aliases", []) if name in snapshots}
    receipts, failures, roots = {}, [], {}
    failure_keys = set()

    def stat(path):
        value = path.stat()
        return (value.st_size, value.st_mtime_ns)

    def fail(kind, **details):
        record = {"kind": kind, **details}
        key = json.dumps(record, sort_keys=True)
        if key not in failure_keys:
            failures.append(record)
            failure_keys.add(key)

    def original(claim):
        name = claim["path"]
        row = originals.get(name)
        if row is None or any(row.get(key) != claim.get(key) for key in ("bytes", "sha256")):
            raise ValueError("Original identity is absent or conflicts with the input index: " + name)
        if row.get("container_extraction_complete") is False or row.get("source_bytes_verified") is False:
            raise ValueError("Original was rejected by static extraction validation: " + name)
        actual = checked.get(name)
        path = resolve(name, ".analysis/inputs")
        if name in snapshots and stat(path) != snapshots[name]:
            raise ValueError("Original changed during current-version lineage collection: " + name)
        if actual is None:
            before = stat(path)
            actual = {"bytes": path.stat().st_size, "sha256": sha(path)}
            if stat(path) != before or any(actual[key] != row[key] for key in ("bytes", "sha256")):
                raise ValueError("Original bytes differ from their indexed identity: " + name)
            checked[name] = actual
            snapshots[name] = before
        return {"path": name, **actual}

    def original_json(claim):
        actual = original(claim)
        raw = (ROOT / actual["path"]).read_bytes()
        if len(raw) != actual["bytes"] or hashlib.sha256(raw).hexdigest() != actual["sha256"]:
            raise ValueError("Original JSON changed before static parsing: " + actual["path"])
        return json.loads(raw)

    def document(name):
        if name in receipts:
            return receipts[name][1]
        boundary = ".analysis/inputs" if name.startswith(".analysis/inputs/") else ".analysis/results"
        path = resolve(name, boundary)
        before = stat(path)
        raw = path.read_bytes()
        if stat(path) != before:
            raise ValueError("Evidence document changed during static reading: " + name)
        identity = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        if boundary == ".analysis/inputs":
            original(identity)
        value = json.loads(raw)
        snapshots[name] = before
        receipts[name] = (identity, value)
        return value

    def root(artifact, authority, source_kind):
        from host import OFFICIAL
        url = urllib.parse.urlsplit(artifact["url"])
        allowed = {"app-assets.razer.com"} if source_kind == "official_device_resource" else OFFICIAL
        if url.scheme != "https" or url.netloc not in allowed:
            raise ValueError("Current resource is not an official HTTPS artifact URL")
        actual = original(artifact)
        key = source_kind + ":" + hashlib.sha256((artifact["url"] + ":" + actual["sha256"]).encode()).hexdigest()
        entry = roots.setdefault(key, {"id": key, "source_kind": source_kind,
                                      "artifact": actual, "url": artifact["url"], "authorities": []})
        if authority not in entry["authorities"]:
            entry["authorities"].append(authority)

    native_name = ".analysis/inputs/official-native-current/acquisition.json"
    if (ROOT / native_name).is_file():
        try:
            native = document(native_name)
            if not native.get("finished_at_utc"):
                raise ValueError("Native resource acquisition has no completion receipt")
            stability = {row["url"]: row for row in native.get("stability", [])}
            resources = {(row["url"], row.get("declared_sha256"), row.get("declared_bytes")): row
                         for row in native.get("resources", [])}
            host_manifest_sources = {}
            if any(row.get("manifest_scope") == "host_app" for row in native.get("manifests", [])):
                host_manifest_sources, source_failures = host_app_native_manifest_sources(index)
                for failure in source_failures:
                    fail(**failure)
            for manifest in native.get("manifests", []):
                try:
                    url = urllib.parse.urlsplit(manifest["url"])
                    host_scope = manifest.get("manifest_scope") == "host_app"
                    if url.scheme != "https" or url.netloc != "apps.razer.com" or url.query or url.fragment:
                        raise ValueError("Current native manifest URL is not an official production endpoint")
                    if not host_scope and not re.fullmatch(r"/synapse/products/[0-9]+/mw/manifest\.json", url.path):
                        raise ValueError("Current device manifest URL is not an official production endpoint")
                    actual = original(manifest)
                    after = stability.get(manifest["url"], {})
                    if (after.get("unchanged") is not True or after.get("before_sha256") != actual["sha256"]
                            or after.get("after_sha256") != actual["sha256"]):
                        raise ValueError("Current native manifest deployment stability does not bind its actual bytes")
                    source = original_json(actual)
                    if host_scope:
                        source_refs = host_manifest_sources.get(manifest["url"], [])
                        if not source_refs or any(ref["manifest"]["sha256"] != actual["sha256"] for ref in source_refs):
                            raise ValueError("Host native manifest is not bound to its current actual HTML deployment")
                        for ref in source_refs:
                            document(ref["acquisition"]["path"])
                            original(ref["html"])
                            original(ref["manifest"])
                    for declared in source.get("resourceManifest", {}).get("resources", []):
                        digest = declared["sha256"].lower()
                        artifact = resources.get((declared["url"], digest, declared["size"]), {})
                        if (artifact.get("status") != "verified" or artifact.get("sha256") != digest
                                or artifact.get("bytes") != declared["size"]):
                            fail("current_native_resource_unverified", manifest=actual,
                                 url=declared["url"], declared_sha256=digest, declared_bytes=declared["size"],
                                 observed_sha256=artifact.get("observed_artifact", {}).get("sha256"),
                                 error=artifact.get("error"))
                            continue
                        if declared.get("integrity") and declared["integrity"] != "sha256-" + base64.b64encode(bytes.fromhex(digest)).decode():
                            raise ValueError("Actual native manifest SHA and integrity disagree")
                        root(artifact, {"manifest": actual, "endpoint": manifest["url"],
                             "version": source.get("version"), "build_version": source.get("buildVersion"),
                             "resource_name": declared.get("resourceName"), "resource_version": declared.get("resourceVersion")},
                             "official_host_app_resource" if host_scope else "official_device_resource")
                except Exception as exc:
                    fail("current_native_manifest_unverified", input=manifest.get("path"), error=str(exc))
        except Exception as exc:
            fail("current_native_acquisition_unverified", input=native_name, error=str(exc))

    bootstrap_name = ".analysis/inputs/host-current/bootstrap-recheck.json"
    if (ROOT / bootstrap_name).is_file():
        try:
            from host import OFFICIAL, STABLE_DOWNLOAD_PAGE, STABLE_BOOTSTRAP
            bootstrap = document(bootstrap_name)
            if not bootstrap.get("finished_at_utc") or bootstrap.get("failures") or bootstrap.get("bootstrap_stable") is not True:
                raise ValueError("Current official bootstrap revalidation did not finish successfully")
            samples = defaultdict(list)
            for item in bootstrap.get("files", []):
                final = urllib.parse.urlsplit(item["final_url"])
                if final.scheme != "https" or final.netloc not in OFFICIAL or item.get("http_status") != 200:
                    raise ValueError("Bootstrap evidence response is not an official HTTPS success")
                original(item)
                samples[item["url"]].append(item)
            stability = {row["url"]: row for row in bootstrap.get("stability", [])}
            for url, kind in ((STABLE_DOWNLOAD_PAGE, "official_stable_download_page"),
                              (STABLE_BOOTSTRAP, "official_stable_bootstrap_installer")):
                pair = samples.get(url, [])
                check = stability.get(url, {})
                if (len(pair) != 2 or any(item.get("kind") != kind for item in pair)
                        or pair[0]["final_url"] != pair[1]["final_url"] or pair[0]["bytes"] != pair[1]["bytes"]
                        or pair[0]["sha256"] != pair[1]["sha256"] or check.get("unchanged") is not True
                        or check.get("before_sha256") != pair[0]["sha256"] or check.get("after_sha256") != pair[1]["sha256"]):
                    raise ValueError("Official bootstrap page/entry has no matching actual two-sample stability proof")
            page = samples[STABLE_DOWNLOAD_PAGE][-1]
            if STABLE_BOOTSTRAP not in (ROOT / page["path"]).read_text(encoding="utf-8"):
                raise ValueError("Current official page does not declare the observed bootstrap endpoint")
            artifact = samples[STABLE_BOOTSTRAP][-1]
            with (ROOT / artifact["path"]).open("rb") as stream:
                if stream.read(2) != b"MZ":
                    raise ValueError("Current official bootstrap bytes are not a PE candidate")
            root(artifact, {"download_page": original(page), "entry": STABLE_BOOTSTRAP,
                           "revalidation": receipts[bootstrap_name][0]}, "official_host_bootstrap")
        except Exception as exc:
            fail("current_host_bootstrap_unverified", input=bootstrap_name, error=str(exc))

    host_name = ".analysis/inputs/host-current/acquisition.json"
    if (ROOT / host_name).is_file():
        try:
            host = document(host_name)
            recheck_name = ".analysis/inputs/host-current/metadata-recheck.json"
            current_name = recheck_name if (ROOT / recheck_name).is_file() else host_name
            current = document(current_name)
            if not current.get("finished_at_utc") or current.get("failures"):
                raise ValueError("Latest host deployment metadata acquisition did not finish successfully")
            metadata = {}
            from host import OFFICIAL
            for item in current.get("files", []):
                if item.get("kind") in {"official_host_release_manifest", "official_host_resource_manifest",
                                        "official_host_discovery", "official_host_product_release"}:
                    url = urllib.parse.urlsplit(item["url"])
                    if url.scheme != "https" or url.netloc not in OFFICIAL:
                        raise ValueError("Current host metadata is not an official HTTPS source")
                    original(item)
                    metadata[item["url"]] = item
            stability = {row["url"]: row for row in current.get("stability", [])}
            for url, item in metadata.items():
                after = stability.get(url, {})
                if (after.get("unchanged") is not True or after.get("before_sha256") != item["sha256"]
                        or after.get("after_sha256") != item["sha256"]):
                    raise ValueError("Latest host metadata stability does not bind its actual bytes: " + url)
            if not metadata:
                raise ValueError("Latest host deployment metadata is absent")
            bodies = {url: original_json(item) for url, item in metadata.items()}
            declarations = defaultdict(list)
            for url, item in metadata.items():
                if item.get("kind") == "official_host_release_manifest":
                    body = bodies[url]
                    selected = "https://apps.razer.com/" + body["baseURL"].strip("/") + "/" + body["latest"]["url"]
                    if selected not in metadata or metadata[selected].get("kind") != "official_host_resource_manifest":
                        raise ValueError("Current host resource manifest referenced by its release index is absent")
                    authority = {"release_manifest": original(item), "manifest": original(metadata[selected]),
                                 "version": body["latest"].get("version"), "release": body["latest"]}
                    for declared in bodies[selected].get("resources", []):
                        declarations[declared["url"]].append((declared, authority, "sha256"))
                elif item.get("kind") == "official_host_discovery":
                    production = next(entry for entry in bodies[url]["data"]["items"] if entry["name"] == "prod")
                    if not re.fullmatch(r"[A-Za-z0-9]+", production["hash"]):
                        raise ValueError("Current host production discovery hash is invalid")
                    releases = [(release_url, candidate) for release_url, candidate in metadata.items()
                                if candidate.get("kind") == "official_host_product_release"
                                and urllib.parse.urlsplit(release_url).path ==
                                "/api/v1/releases/" + production["hash"] + "/tags/prod/products"]
                    if not releases:
                        raise ValueError("Current production host release metadata is absent")
                    for release_url, candidate in releases:
                        for product in bodies[release_url]["data"]["items"]:
                            if product["name"] not in {"Anne", "Chroma"}:
                                continue
                            authority = {"discovery": original(item), "manifest": original(candidate),
                                         "product": product["name"], "version": product["version"]}
                            for declared in product.get("modules", []):
                                declarations[declared["download_url"]].append((declared, authority, "md5"))
            for artifact in host.get("files", []):
                for declared, authority, algorithm in declarations.get(artifact["url"], []):
                    try:
                        actual = original(artifact)
                        if algorithm == "sha256":
                            digest = declared["sha256"].lower()
                            if (actual["sha256"] != digest or actual["bytes"] != declared["size"]
                                    or declared.get("integrity") != "sha256-" + base64.b64encode(bytes.fromhex(digest)).decode()):
                                raise ValueError("Host resource bytes differ from the current manifest declaration")
                        else:
                            with (ROOT / actual["path"]).open("rb") as stream:
                                digest = hashlib.file_digest(stream, "md5").hexdigest()
                            if actual["bytes"] != declared["file_size"] or digest != declared["download_file_checksum"].lower():
                                raise ValueError("Host package bytes differ from the current length/MD5 declaration")
                        root(artifact, {**authority, "declaration": declared}, "official_host_resource")
                    except Exception as exc:
                        fail("current_host_resource_unverified", input=artifact.get("path"), error=str(exc))
        except Exception as exc:
            fail("current_host_acquisition_unverified", input=host_name, error=str(exc))

    # All candidate edges remain inert until their parent has a proven current
    # root. A forged inherited flag or an arbitrary installed path cannot seed it.
    candidates = defaultdict(list)
    unpacked_siblings = []
    kinds = {"static_extracted_native_input", "static_embedded_native_input",
             "static_extracted_host_input", "static_extracted_host_asar_input"}
    for item in originals.values():
        if (item.get("kind") in kinds and item.get("container") and item.get("container_sha256")
                and item.get("container_extraction_complete") is not False and item.get("source_bytes_verified") is not False):
            if item["kind"] == "static_extracted_host_asar_input" and item.get("unpacked"):
                unpacked_siblings.append(item)
            else:
                candidates[item["container_sha256"]].append(item)
    reachable = defaultdict(set)
    proven_aliases = defaultdict(set)
    for key, entry in roots.items():
        reachable[entry["artifact"]["sha256"]].add(key)
        proven_aliases[entry["artifact"]["sha256"]].add(entry["artifact"]["path"])
    pending = deque(reachable)
    queued = set(pending)
    edges, checked_edges = [], {}
    host_containers = None

    def edge(item):
        nonlocal host_containers
        child = original(item)
        parent_row = originals.get(item["container"])
        if parent_row is None or parent_row["sha256"] != item["container_sha256"]:
            raise ValueError("Static input container identity differs from the index")
        parent = original(parent_row)
        kind = item["kind"]
        proof = {"parent": parent, "child": child, "kind": kind}
        if kind == "static_embedded_native_input":
            offset = item.get("container_offset")
            if type(offset) is not int or offset < 0 or offset + child["bytes"] > parent["bytes"]:
                raise ValueError("Embedded input has no valid exact byte range")
            with (ROOT / parent["path"]).open("rb") as stream:
                stream.seek(offset)
                raw = stream.read(child["bytes"])
            if len(raw) != child["bytes"] or hashlib.sha256(raw).hexdigest() != child["sha256"]:
                raise ValueError("Embedded input bytes differ from their actual parent range")
            proof["container_offset"] = offset
        else:
            if kind == "static_extracted_native_input":
                name = ".analysis/results/native-packages/" + parent["sha256"] + "/inspection.json"
                record = document(name)
                container = record.get("container", {})
                complete = (record.get("extracted") is True and record.get("listing_exit_code") == 0
                            and record.get("extraction_exit_code") == 0 and record.get("input_bytes_unchanged") is True)
            elif kind == "static_extracted_host_input":
                name = ".analysis/results/host-current/inspection.json"
                inspection = document(name)
                if host_containers is None:
                    host_containers = {row["sha256"]: row for row in inspection.get("containers", [])}
                record = host_containers.get(parent["sha256"], {})
                container = record.get("input_record", {})
                integrity = record.get("retained_archive_integrity", {})
                static_verified = (record.get("extraction_exit_code") == 0 or
                                   (integrity.get("exit_code") == 0 and integrity.get("input_bytes_unchanged") is True
                                    and integrity.get("input", {}).get("sha256") == parent["sha256"]
                                    and len(integrity.get("outputs", [])) == 2
                                    and record.get("retained_outputs_verified") is True))
                if record.get("extraction_exit_code") != 0 and static_verified:
                    original(integrity["input"])
                    for artifact in integrity.get("outputs", []):
                        path = resolve(artifact["path"], ".analysis/results")
                        before = stat(path)
                        if before[0] != artifact["bytes"] or sha(path) != artifact["sha256"] or stat(path) != before:
                            raise ValueError("Retained archive integrity output byte identity differs")
                        snapshots[artifact["path"]] = before
                complete = (record.get("extracted") is True and record.get("list_exit_code") == 0
                            and static_verified and record.get("input_bytes_unchanged") is True)
                if record.get("inno_setup"):
                    complete = (complete and record["inno_setup"].get("list_exit_code") == 0
                                and record["inno_setup"].get("archive_entry_closure", {}).get("complete") is True)
            else:
                name = ".analysis/results/host-current/asar/" + parent["sha256"] + "/capture.json"
                record = document(name)
                container = {"path": record.get("input"), "bytes": record.get("input_bytes"),
                             "sha256": record.get("input_sha256")}
                complete = record.get("complete") is True and not record.get("unresolved")
            if not complete or container.get("sha256") != parent["sha256"]:
                raise ValueError("Complete static extraction receipt does not bind this container")
            original(container)
            recorded = next((row for row in record.get("files", []) if row.get("path") == child["path"]), None)
            if recorded is None or any(recorded.get(key) != child[key] for key in ("bytes", "sha256")):
                raise ValueError("Static extraction receipt does not contain this exact output")
            proof["receipt"] = receipts[name][0]
            if kind == "static_extracted_host_asar_input":
                if item.get("unpacked"):
                    aliases = proven_aliases[child["sha256"]] - {child["path"]}
                    if not aliases:
                        raise ValueError("ASAR header alone does not establish current bytes of its unpacked sibling; an official package alias is required")
                    alias = original(originals[sorted(aliases)[0]])
                    if alias["bytes"] != child["bytes"]:
                        raise ValueError("Independent ASAR unpacked alias has a different actual length")
                    proof.update(parent=alias, declared_asar=parent, kind="verified_current_equal_byte_alias",
                                 scope="Current bytes derive from the independently verified alias; ASAR header does not transfer identity")
                    return proof
                offset = recorded.get("offset")
                if type(offset) is not int or offset < 0 or offset + child["bytes"] > parent["bytes"]:
                    raise ValueError("Packed ASAR entry has no valid original byte range")
                with (ROOT / parent["path"]).open("rb") as stream:
                    stream.seek(offset)
                    raw = stream.read(child["bytes"])
                if len(raw) != child["bytes"] or hashlib.sha256(raw).hexdigest() != child["sha256"]:
                    raise ValueError("Packed ASAR bytes differ from the actual archive span")
                proof["container_offset"] = offset
        return proof

    while pending:
        parent_sha = pending.popleft()
        queued.discard(parent_sha)
        for item in candidates.get(parent_sha, []):
            key = item["path"]
            if key not in checked_edges:
                try:
                    proof = edge(item)
                    checked_edges[key] = proof
                    edges.append(proof)
                except Exception as exc:
                    checked_edges[key] = None
                    fail("current_container_edge_unverified", input=key, container=item["container"], error=str(exc))
            if checked_edges[key] is None:
                continue
            child_sha = item["sha256"]
            proven_aliases[child_sha].add(item["path"])
            added = reachable[parent_sha] - reachable[child_sha]
            if added:
                reachable[child_sha].update(added)
                if child_sha not in queued:
                    pending.append(child_sha)
                    queued.add(child_sha)
    # Unpacked entries are outside the archive. Resolve only after all genuine
    # official package edges, so collection order cannot invent or hide proof.
    for item in unpacked_siblings:
        if not reachable.get(item["container_sha256"]):
            continue
        try:
            edges.append(edge(item))
        except Exception as exc:
            fail("current_container_edge_unverified", input=item["path"], container=item["container"], error=str(exc))
    changed = []
    for name, before in snapshots.items():
        try:
            if stat(ROOT / name) != before:
                changed.append(name)
        except OSError:
            changed.append(name)
    unresolved = []
    for group in groups:
        current_roots = sorted(reachable.get(group["sha256"], []))
        group["current_version_provenance"] = {
            "status": "snapshot_changed" if changed else "proven_current_resource_bytes" if current_roots else "unresolved",
            "root_ids": current_roots,
            "identity_transfer": "Actual length/SHA-256 equality for each verified alias; no installed path or version-string inference"}
        if not current_roots:
            unresolved.append({"sha256": group["sha256"], "aliases": group.get("verified_aliases", [])})
    return {"schema_version": 1, "receipts": [item[0] for item in receipts.values()],
            "roots": list(roots.values()), "container_edges": edges, "failures": failures,
            "snapshot_unchanged": not changed, "changed_during_audit": changed,
            "unresolved_pe_inputs": unresolved, "retained_pe_provenance_complete": bool(groups) and not unresolved and not changed,
            "scope": "Current declared official artifact bytes and verified static container descendants only; dependency closure, installer branches, ABI semantics and product/runtime acceptance remain separate"}


def cached_download_policy(raw):
    """Read the complete current installer chunk owner without executing JS."""
    import evidence
    import importlib.metadata
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Installer download source has JavaScript parser errors")
    pending, matches = [tree.root_node], []
    while pending:
        node = pending.pop()
        pending.extend(node.named_children)
        if node.type != "string" or node.text != b'"pindex"':
            continue
        current, owners = node.parent, []
        while current is not None:
            if current.type in {"arrow_function", "function_expression", "function_declaration"}:
                body = current.text.decode("utf-8")
                if 'caches.open("downloaded")' in body and 'caches.open("razer-installer-files")' in body:
                    owners.append(current)
            current = current.parent
        if owners:
            matches.append((owners[0], node))
    if len(matches) != 1:
        raise ValueError("Installer chunked download owner is not unique")
    owner, index_node = matches[0]
    body = owner.text.decode("utf-8")
    chunk = re.search(r"for\(let ([\w$]+)=0;\1<([\w$]+);\1\+=([\w$]+)\)\{const ([\w$]+)=Math\.min\(\1\+\3-1,\2-1\)", body)
    if (chunk is None or '.searchParams.set("pindex",' not in body or '.Range=' not in body
            or '"bytes=${' not in body.replace("`", '"')):
        raise ValueError("Installer chunk index and range construction is unresolved")
    request_owner = index_node.parent
    while request_owner is not None and request_owner.type not in {"arrow_function", "function_expression", "function_declaration"}:
        request_owner = request_owner.parent
    if request_owner is None:
        raise ValueError("Installer chunk request has no callable owner")
    request_binding = request_owner.parent
    parameter_node = request_owner.child_by_field_name("parameters")
    parameters = parameter_node.named_children if parameter_node is not None else []
    if (request_binding is None or request_binding.type != "variable_declarator" or len(parameters) < 3
            or parameters[0].type != "assignment_pattern" or parameters[1].type != "assignment_pattern"):
        raise ValueError("Installer chunk request parameters are unresolved")
    index_name = parameters[0].child_by_field_name("left").text.decode("ascii")
    start_name = parameters[1].child_by_field_name("left").text.decode("ascii")
    end_name = parameters[2].text.decode("ascii")
    request_name = request_binding.child_by_field_name("name").text.decode("ascii")
    request_body = request_owner.text.decode("utf-8")
    start_binding = re.search(r"\blet ([\w$]+)=" + re.escape(start_name) + r"[;,]", request_body)
    if (start_binding is None
            or f'.searchParams.set("pindex",{index_name}.toString())' not in request_body
            or 'bytes=${' + start_binding[1] + '}-${' + end_name + '||""}' not in request_body
            or not re.search(re.escape(request_name) + r"\([\w$]+," + re.escape(chunk[1]) + "," + re.escape(chunk[4]) + r"[,)]", body[chunk.end():])):
        raise ValueError("Installer chunk index does not bind the declared byte range")
    declarations = {}

    def number(node, reference, seen=()):
        if node is None:
            raise ValueError("Installer chunk size has no static numeric expression")
        if node.type == "number":
            value = float(node.text.decode("ascii"))
        elif node.type == "identifier":
            name = node.text.decode("ascii")
            if name in seen:
                raise ValueError("Installer chunk size has a cyclic static binding")
            definition = evidence.javascript_binding(raw, tree, name, reference)
            if definition.get("resolution") != "declaration" or definition.get("syntax") != "variable_declarator":
                raise ValueError("Installer chunk size has an unresolved static binding")
            declarations[name] = definition
            declaration = tree.root_node.descendant_for_byte_range(definition["start_byte"], definition["end_byte"] - 1)
            if (declaration.parent is None or declaration.parent.type != "lexical_declaration"
                    or declaration.parent.children[0].type != "const"):
                raise ValueError("Installer chunk size binding is not constant")
            value = number(declaration.child_by_field_name("value"), definition["start_byte"], (*seen, name))
        elif node.type == "binary_expression" and node.child_by_field_name("operator").text == b"*":
            value = number(node.child_by_field_name("left"), reference, seen) * number(node.child_by_field_name("right"), reference, seen)
        elif node.type == "call_expression" and node.child_by_field_name("function").text == b"Math.floor":
            args = node.child_by_field_name("arguments").named_children
            if len(args) != 1:
                raise ValueError("Unexpected installer chunk floor arguments")
            import math
            value = math.floor(number(args[0], reference, seen))
        else:
            raise ValueError("Installer chunk size is not a supported literal numeric expression")
        if not 0 < value <= 2**32:
            raise ValueError("Installer chunk size is outside static numeric bounds")
        return value

    name = chunk[3]
    ref = owner.start_byte + len(body[:chunk.start(3)].encode("utf-8"))
    token = tree.root_node.descendant_for_byte_range(ref, ref + len(name) - 1)
    size = number(token, ref)
    if not float(size).is_integer():
        raise ValueError("Installer chunk size is not an integer")
    return {"chunk_bytes": int(size), "index_parameter": "pindex",
            "range_rule": "start=index*chunk_bytes; end=min(start+chunk_bytes-1,total_bytes-1)",
            "complete_owner": {"start_byte": owner.start_byte, "end_byte": owner.end_byte,
                               "syntax": owner.type, "body": body, "sha256": hashlib.sha256(owner.text).hexdigest()},
            "constant_declarations": list(declarations.values()),
            "parser": {"tree_sitter": importlib.metadata.version("tree-sitter"),
                       "tree_sitter_javascript": importlib.metadata.version("tree-sitter-javascript")}}


def cached_download_fragment_gaps(report, evidence):
    """Keep fragment proof and the complete parent's provenance independently visible."""
    rows = [{**failure, "evidence": evidence} for failure in report.get("failures", [])]
    if "snapshot_unchanged" in report and report["snapshot_unchanged"] is not True:
        if not any(row.get("kind") == "cached_download_fragment_snapshot_changed" for row in rows):
            rows.append({"kind": "cached_download_fragment_snapshot_changed", "evidence": evidence,
                         "inputs": report.get("changed_during_audit", [])})
    for fragment in report.get("fragments", []):
        proof = fragment.get("complete_resource_provenance", {})
        if proof.get("status") != "proven_current_resource_bytes":
            rows.append({"kind": "cached_download_fragment_parent_unverified", "evidence": evidence,
                         "input": fragment["artifact"]["path"], "complete_resource": fragment["complete_resource"],
                         "provenance": proof})
    return rows


def cached_download_fragments(index):
    """Prove cached chunks against the source policy and actual complete bytes."""
    indexed = {row["path"]: row for row in index["inputs"]}
    candidates = [row for row in index["inputs"] if row.get("kind") == "installed_cached_official_resource"
                  and "pindex" in urllib.parse.parse_qs(urllib.parse.urlsplit(row.get("url", "")).query)]
    report = {"schema_version": 1, "fragments": [], "failures": [], "sources": [],
              "target_execution": False,
              "scope": "Cached inert download chunks are separate from complete PE/native inputs; no whole-file identity transfer"}
    if not candidates:
        return report
    snapshots = {}

    def original(row):
        path = resolve(row["path"], ".analysis/inputs")
        before = path.stat()
        raw = path.read_bytes()
        expected = indexed.get(row["path"], {})
        after = path.stat()
        if (any(row.get(key) != expected.get(key) for key in ("bytes", "sha256"))
                or len(raw) != row["bytes"] or hashlib.sha256(raw).hexdigest() != row["sha256"]
                or (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns)):
            raise ValueError("Cached fragment evidence differs from its indexed original bytes")
        snapshots[row["path"]] = (after.st_size, after.st_mtime_ns)
        return raw

    try:
        receipt_name = ".analysis/inputs/frontend/installer-current/acquisition.json"
        receipt = indexed[receipt_name]
        acquired = json.loads(original(receipt))
        if (acquired.get("source_current_during_acquisition") is not True
                or acquired.get("entry_url") != "https://apps.razer.com/installer/"):
            raise ValueError("Installer current deployment is unverified")
        files = {row["url"]: row for row in acquired["files"] if "path" in row}
        entry = files[acquired["entry_url"]]
        html = original(entry)
        stability = {row["url"]: row for row in acquired["stability"]}
        stable = stability.get(entry["url"], {})
        if (stable.get("unchanged") is not True or stable.get("before_sha256") != entry["sha256"]
                or stable.get("after_sha256") != entry["sha256"]):
            raise ValueError("Installer HTML stability does not bind its actual bytes")
        policies = []
        for edge in acquired.get("edges", []):
            if edge.get("from") != entry["url"] or edge.get("kind") != "html:script:src":
                continue
            source = files.get(edge.get("to"))
            if source is None:
                continue
            raw = original(source)
            if b'pindex' not in raw:
                continue
            if edge["literal"].encode("utf-8") not in html:
                raise ValueError("Installer chunk source is not declared by actual HTML")
            if urllib.parse.urljoin(entry.get("final_url", entry["url"]), edge["literal"]) != source["url"]:
                raise ValueError("Installer chunk source URL does not resolve from its HTML declaration")
            policies.append({"source": source, **cached_download_policy(raw)})
        if len(policies) != 1:
            raise ValueError("Current installer has no unique declared chunk policy")
        report["sources"] = [receipt, entry]
        report["policy"] = policies[0]
        size = policies[0]["chunk_bytes"]
        native_name = ".analysis/inputs/official-native-current/acquisition.json"
        native = json.loads(original(indexed[native_name]))
        if not native.get("finished_at_utc"):
            raise ValueError("Complete resource acquisition is unfinished")
        report["sources"].append(indexed[native_name])
        resources = {row["url"]: row for row in native["resources"] if row.get("status") == "verified"}
        complete_bytes = {}
        for row in candidates:
            try:
                query = urllib.parse.parse_qs(urllib.parse.urlsplit(row["url"]).query)
                indices = query["pindex"]
                if len(indices) != 1 or not re.fullmatch(r"0|[1-9][0-9]*", indices[0]):
                    raise ValueError("Cached chunk index is not a single nonnegative integer")
                parts = urllib.parse.urlsplit(row["url"])
                canonical = urllib.parse.urlunsplit((parts.scheme, parts.netloc, parts.path, "", ""))
                if row.get("canonical_url") != canonical:
                    raise ValueError("Cached chunk canonical resource URL disagrees")
                complete = resources[canonical]
                if complete["sha256"] != complete["declared_sha256"] or complete["bytes"] != complete["declared_bytes"]:
                    raise ValueError("Complete resource is not verified against its declaration")
                if canonical not in complete_bytes:
                    complete_bytes[canonical] = original(complete)
                chunk, whole = original(row), complete_bytes[canonical]
                offset = int(indices[0]) * size
                length = min(size, len(whole) - offset)
                if not 0 < length < len(whole) or len(chunk) != length or chunk != whole[offset:offset + length]:
                    raise ValueError("Cached bytes do not match the exact source-defined complete-resource range")
                report["fragments"].append({"artifact": {key: row[key] for key in ("path", "bytes", "sha256", "url")},
                    "index": int(indices[0]), "offset": offset, "end_exclusive": offset + length,
                    "complete_resource": {key: complete[key] for key in ("path", "bytes", "sha256", "url")},
                    "status": "verified_cached_download_fragment", "whole_file_identity": False})
            except (OSError, ValueError, KeyError, TypeError) as exc:
                report["failures"].append({"kind": "cached_download_fragment_unverified", "input": row["path"], "error": str(exc)})
    except (OSError, ValueError, KeyError, TypeError, ImportError) as exc:
        report["failures"].append({"kind": "cached_download_fragment_policy_unverified", "error": str(exc)})
    changed = [name for name, before in snapshots.items() if not (ROOT / name).is_file()
               or ((ROOT / name).stat().st_size, (ROOT / name).stat().st_mtime_ns) != before]
    report["snapshot_unchanged"] = not changed
    report["changed_during_audit"] = changed
    if changed:
        report["failures"].append({"kind": "cached_download_fragment_snapshot_changed", "inputs": changed})
        report["fragments"] = []
    return report


def inspect_corpus(args=None):
    from host import inspect_pe
    index_path = ROOT / ".analysis/inputs.json"
    index_hash = sha(index_path)
    data = json.loads((ROOT / ".analysis/inputs.json").read_bytes())
    fragments = cached_download_fragments(data)
    fragment_paths = {row["artifact"]["path"] for row in fragments["fragments"]}
    groups = {}
    verified_stats = {}
    excluded = []
    failures = []
    for row in data["inputs"]:
        p = Path(row["path"])
        if not row["path"].startswith(".analysis/inputs/"):
            continue
        if row["path"] in fragment_paths:
            excluded.append({"path": row["path"], "reason": "verified cached download fragment; complete resource separately retained"})
            continue
        if row.get("container_extraction_complete") is False or row.get("source_bytes_verified") is False:
            excluded.append({"path": row["path"], "reason": "unverified partial static decompression",
                             "extraction_receipt": row.get("extraction_receipt")})
            continue
        if "target" in p.parts or "keyboard-control-check2" in p.parts:
            excluded.append({"path": row["path"], "reason": "local Rust build artifact"})
            continue
        eligible = p.suffix.lower() in {".dll", ".node", ".exe", ".sys"}
        if not eligible and row.get("kind") in {"static_extracted_native_input", "static_embedded_native_input"}:
            # Burn cabinets use opaque payload IDs such as a0/u4. Classify
            # their actual PE headers instead of dropping them by filename.
            source = resolve(row["path"], ".analysis/inputs")
            if source.is_file():
                with source.open("rb") as stream:
                    eligible = stream.read(2) == b"MZ"
        if eligible:
            group = groups.setdefault(row["sha256"], {"sha256": row["sha256"], "bytes": row["bytes"], "aliases": [],
                                                     "verified_aliases": [], "acquisition_bindings": []})
            group["aliases"].append(row["path"])
            group["acquisition_bindings"].append(row)
            try:
                source = resolve(row["path"], ".analysis/inputs")
                before = (source.stat().st_size, source.stat().st_mtime_ns)
                raw = source.read_bytes()
                if (len(raw) != row["bytes"] or hashlib.sha256(raw).hexdigest() != row["sha256"]
                        or (source.stat().st_size, source.stat().st_mtime_ns) != before):
                    raise ValueError("Indexed input bytes differ")
                group["verified_aliases"].append(row["path"])
                verified_stats[row["path"]] = before
                if "pe" not in group:
                    group["pe"] = inspect_pe(raw)
            except Exception as exc:
                failures.append({"input": row["path"], "stage": "static_pe_inventory", "error": str(exc).replace(str(ROOT), "<project>")})
    lineage = corpus_current_lineage(data, list(groups.values()), verified_stats)
    fragment_roots = defaultdict(list)
    for root in lineage["roots"]:
        fragment_roots[root["artifact"]["sha256"]].append(root["id"])
    for fragment in fragments["fragments"]:
        root_ids = sorted(fragment_roots[fragment["complete_resource"]["sha256"]])
        fragment["complete_resource_provenance"] = {
            "status": "proven_current_resource_bytes" if root_ids and lineage["snapshot_unchanged"] else "unresolved",
            "root_ids": root_ids}
    kinds = {"native_inputs": [], "managed_inputs": [], "mixed_inputs": [], "other_inputs": [], "unclassified_inputs": []}
    for row in sorted(groups.values(), key=lambda row: row["sha256"]):
        kind = row.get("pe", {}).get("kind")
        key = {"native": "native_inputs", "managed": "managed_inputs", "mixed_managed_native": "mixed_inputs", "other": "other_inputs"}.get(kind, "unclassified_inputs")
        kinds[key].append(row)
    result = {"schema_version": 3, "recorded_at_utc": stamp(), "tool": {"path": "tools/host.py", "sha256": sha(ROOT / "tools/host.py")},
              "lineage_tool": {"path": "tools/analysis.py", "sha256": sha(Path(__file__))},
              "input_index": {"path": rel(index_path), "sha256_at_start": index_hash, "unchanged": sha(index_path) == index_hash},
              **kinds, "excluded_inputs": excluded, "failures": failures,
              "cached_download_fragments": fragments,
              "original_inputs_count": len(data["inputs"]), "current_version_audit": "incomplete",
              "current_version_lineage": lineage,
              "target_execution": False, "real_device_operations": False,
              "scope": "All indexed DLL/NODE/EXE/SYS aliases and PE-header-bearing static extracted payloads verified and classified using PE headers, CLR metadata, imports, exports and version resources"}
    save(ROOT / ".analysis/results/corpus.json", result)
    if not getattr(args, "no_refresh_gaps", False):
        refresh_gaps()
    print(json.dumps({"corpus": ".analysis/results/corpus.json", **{k: len(v) for k, v in kinds.items()}, "failures": len(failures)}), flush=True)
    return result


def native_chain_function_reference(record):
    """Link a complete verified function artifact without duplicating its xrefs."""
    return {key: record[key] for key in ("path", "bytes", "sha256", "name", "rva", "address", "end",
                                        "type", "full_assembly", "full_pseudocode") if key in record}


def feature_physical_transports(row, parser, originals):
    """Resolve the retained mounted factory; never use a capability class label."""
    owners = [owner for source in row["sources"] for owner in source["owners"]]
    infos = [owner["body"] for owner in owners if owner["role"] == "device_info"]
    factories = [owner["body"] for owner in owners if owner["role"] == "physical_transport_factory"]
    if len(infos) != 1 or len(factories) != 1:
        raise ValueError("Physical transport requires exactly one current DeviceInfo and factory")
    info, factory = infos[0], factories[0]
    fields = {}
    node = parser.parse(info.encode()).root_node.named_children[0]
    # The retained variable_declarator is parsed as an assignment expression.
    pending = [node]
    while pending:
        part = pending.pop()
        if part.type == "pair":
            key, value = part.child_by_field_name("key"), part.child_by_field_name("value")
            if key.text in {b"productId", b"dongleId", b"claimInterface"}:
                fields[key.text.decode()] = value.text.decode()
        pending.extend(part.named_children)
    pid = int(fields["productId"])
    dongle = int(fields["dongleId"]) if "dongleId" in fields else None
    if pid != row["product_id"]:
        raise ValueError("DeviceInfo does not describe this product")
    interfaces = (json.loads(re.sub(r'(?<=[{,])(\d+):', r'"\1":', fields["claimInterface"]))
                  if "claimInterface" in fields else None)
    registrations = {}
    for owner in owners:
        if owner["role"] != "transport_feature_registration":
            continue
        pending = [parser.parse(owner["body"].encode()).root_node]
        while pending:
            part = pending.pop()
            pending.extend(part.named_children)
            if part.type != "call_expression":
                continue
            function, arguments = part.child_by_field_name("function"), part.child_by_field_name("arguments")
            if function is None or not function.text.endswith(b".useFeature") or arguments is None:
                continue
            values = arguments.named_children
            if len(values) not in (2, 3) or [value.text for value in values[:2]] != [b'"rzDeviceType"', b'"rzDevice25"']:
                continue
            options = values[2] if len(values) == 3 else None
            registrations[options.text if options else b"{}"] = options
    if len(registrations) != 1:
        raise ValueError("Current mounted transport registration is ambiguous")
    options = next(iter(registrations.values()))
    settings = {}
    if options is not None:
        if options.type != "object":
            raise ValueError("Current transport registration options are not a literal")
        for part in options.named_children:
            if part.type != "pair":
                raise ValueError("Current transport registration has unresolved option syntax")
            settings[part.child_by_field_name("key").text.decode()] = part.child_by_field_name("value").text.decode()
    permitted = {"isDualLinkDevice", "isMikaDevice", "isAnalogDevice", "regionIdEnum", "brightness", "setDeviceMode",
                 "sleepTimeBetweenOut", "sleepTimeBetweenOutIn", "sleepTimeBetweenIn", "maxRetryIn", "maxRetryOut"}
    if set(settings) - permitted:
        raise ValueError("Current mounted transport registration requires a separate option audit: " + str(settings))
    is_dual, is_mika = settings.get("isDualLinkDevice") == "!0", settings.get("isMikaDevice") == "!0"
    if is_dual and is_mika:
        raise ValueError("Combined DualLink/Mika precedence needs a separate audit")
    if not all(token in factory for token in ("DeviceInfo.dongleId", "rzDevice25()", "rzDevice25DualLink")):
        raise ValueError("Complete physical factory no longer has the selected wired/DualLink branches")
    classes = {}
    for name in ("rzDevice25", "rzDevice25DualLinkMouse", "rzDevice25DualLinkKeyboard"):
        bodies = [owner["body"] for owner in owners if owner["role"] == "complete_protocol_class"
                  and f'this.name="{name}"' in owner["body"]]
        if bodies:
            classes[name] = min(bodies, key=len)
    base = classes["rzDevice25"]
    if not all(token in base for token in ("reportId:0", "this.reportLength=91", "31===this.transactionId", "this.transactionId++")):
        raise ValueError("Base report shape or transaction counter differs")
    def module_at(tree, offset):
        node = tree.root_node.descendant_for_byte_range(offset, offset)
        while node is not None:
            if node.type == "method_definition" and node.child_by_field_name("name").type == "number":
                return node
            parent = node.parent
            key = parent.child_by_field_name("key") if parent is not None and parent.type == "pair" else None
            if node.type in {"arrow_function", "function_expression"} and key is not None and key.type == "number":
                return node
            node = parent
        raise ValueError("Complete source webpack module is unresolved")
    base_record, base_owner = next((source, owner) for source in row["sources"] for owner in source["owners"] if owner["body"] == base)
    raw = (ROOT / base_record["original"]["path"]).read_bytes()
    base_module = module_at(parser.parse(raw), base_owner["byte_start"]).text.decode()
    inherited = re.search(r'class [\w$]+ extends ([\w$]+)\.([\w$]+)\{', base)
    parent_import = re.search(r'(?<![\w$])' + re.escape(inherited[1]) + r'=[\w$]+\((\d+)\)', base_module) if inherited else None
    if parent_import is None:
        raise ValueError("Physical factory superclass import is unresolved")
    parents = []
    for source in row["sources"]:
        for owner in source["owners"]:
            if owner["role"] != "complete_protocol_base_class":
                continue
            parent_raw = (ROOT / source["original"]["path"]).read_bytes()
            parent_module = module_at(parser.parse(parent_raw), owner["byte_start"])
            parent_key = parent_module.child_by_field_name("name") if parent_module.type == "method_definition" else parent_module.parent.child_by_field_name("key")
            if parent_key is not None and parent_key.text.decode() == parent_import[1]:
                parents.append((owner["body"], parent_module.text.decode()))
    if len(parents) != 1:
        raise ValueError("Physical factory superclass is missing or ambiguous")
    parent_body, parent_module = parents[0]
    if interfaces is None:
        # Some wired DeviceInfo objects omit claimInterface entirely. Trace
        # the selected superclass's undefined-value default instead of
        # inventing an interface or rejecting these real wired products.
        if not re.search(r'this\.claimInterface=[\w$]+,void 0===this\.claimInterface&&\(this\.claimInterface=0\)', parent_body):
            raise ValueError("Selected base constructor's default interface is unresolved")
        interfaces = 0
    parameters = re.findall(r'=([\w$]+)\.([\w$]+)', base[:base.index('{owner:')])[:5]
    if len(parameters) != 5 or len({part[0] for part in parameters}) != 1:
        raise ValueError("Base constructor's complete retry/timing defaults are unresolved")
    namespace = parameters[0][0]
    imported = re.search(r'(?<![\w$])' + re.escape(namespace) + r'=[\w$]+\((\d+)\)', base_module)
    if imported is None:
        raise ValueError("Base constructor constants import is unresolved")
    constants_id = imported[1]
    modules = []
    for original in row["current_originals"]:
        if not original["path"].endswith(".js"):
            continue
        raw = (ROOT / original["path"]).read_bytes()
        if len(raw) != original["bytes"] or hashlib.sha256(raw).hexdigest() != original["sha256"]:
            raise ValueError("Physical factory dependency changed during query: " + original["path"])
        for match in re.finditer(rb'(?<![\w$])' + constants_id.encode() + rb'(?::(?:\(|function\()|\()', raw):
            tree = parser.parse(raw)
            module = module_at(tree, match.end())
            body = module.text.decode()
            if body not in [item[0] for item in modules]:
                modules.append((body, original, module.start_byte, module.end_byte))
    if len(modules) != 1:
        raise ValueError("Selected constructor constants module is missing or ambiguous")
    body, original, start, end = modules[0]
    constants_source = next((source for source in row["sources"] if source["original"]["path"] == original["path"]), None)
    if constants_source is None:
        expected = originals.get(original["path"])
        status = "missing" if expected is None else "verified" if all(
            original[key] == expected.get(key) for key in ("bytes", "sha256")) else "mismatch"
        constants_source = {"original": original, "owners": [], "index_status": status}
        row["sources"].append(constants_source)
        if status != "verified":
            row["failures"].append({"kind": "middleware_original_index_identity_unverified", "path": original["path"], "status": status})
    constants_source["owners"].append({"role": "selected_transport_constants_module", "module_id": int(constants_id),
        "byte_start": start, "byte_end": end, "body": body, "body_sha256": hashlib.sha256(body.encode()).hexdigest()})
    defaults = {}
    fields = ("SLEEP_TIME_BETWEEN_OUT", "SLEEP_TIME_BETWEEN_OUT_IN", "SLEEP_TIME_BETWEEN_IN", "MAX_RETRY_IN", "MAX_RETRY_OUT")
    for field, (_, exported_name) in zip(fields, parameters):
        exported = re.search(r'(?<![\w$])' + re.escape(exported_name) + r':(?:\(\)=>|function\(\)\{return )([\w$]+)', body)
        if exported is None:
            exported = re.search(r'"' + re.escape(exported_name) + r'",0,([\w$]+)(?=[,\]])', body)
            if exported is not None and exported[1].isdigit():
                defaults[field] = int(exported[1])
                continue
        literal = re.search(r'(?<![\w$])' + re.escape(exported[1]) + r'=(\d+)(?=[,;])', body) if exported else None
        if literal is None:
            raise ValueError("Selected constructor constant is not a resolved number: " + field)
        defaults[field] = int(literal[1])
    for property_name, (_, exported_name) in zip(("maxRetryIn", "maxRetryOut"), parameters[3:]):
        clamp = re.search(r'this\.' + property_name + r'>([\w$]+)\.' + re.escape(exported_name)
                          + r'&&\(this\.' + property_name + r'=\1\.' + re.escape(exported_name) + r'\)', parent_body)
        constant_import = re.search(r'(?<![\w$])' + re.escape(clamp[1]) + r'=[\w$]+\((\d+)\)', parent_module) if clamp else None
        if constant_import is None or constant_import[1] != constants_id:
            raise ValueError("Selected base constructor retry limit is unresolved: " + property_name)
        if re.search(r'this\.' + property_name + r'\s*=', base):
            raise ValueError("Protocol25 overrides its base retry limit: " + property_name)
    timing = [name for _, name in parameters[:3]]
    if not re.search(r'5\*[\w$]+\.' + re.escape(timing[0]) + r',5\*[\w$]+\.' + re.escape(timing[1]) + r',5\*[\w$]+\.' + re.escape(timing[2]), factory):
        raise ValueError("Physical DualLink timing multiplier differs from the selected constants")
    radio_class = "rzDevice25DualLinkKeyboard" if row["namespace"].startswith("keyboard-") else "rzDevice25DualLinkMouse"
    def method(body, name):
        parsed = parser.parse(body.encode()).root_node
        pending = [parsed]
        found = []
        while pending:
            part = pending.pop()
            if part.type == "method_definition" and part.child_by_field_name("name").text == name.encode():
                found.append(part.text.decode())
            pending.extend(part.named_children)
        if len(found) != 1:
            raise ValueError("Selected physical class method is ambiguous: " + name)
        return found[0]
    if "reportId:this.reportId" not in method(base, "sendCommand"):
        raise ValueError("Current base SET selector semantics differ")
    if is_dual and "reportId:" in method(classes[radio_class], "sendCommand"):
        raise ValueError("Current DualLink SET selector semantics differ")
    if is_dual and re.search(r'this\.maxRetry(?:In|Out)\s*=', classes[radio_class]):
        raise ValueError("Current DualLink overrides its inherited retry limits")
    prefix = 0
    if is_dual and radio_class.endswith("Keyboard"):
        if not re.search(r',128\|this.transactionId\+\+\}', method(classes[radio_class], "_getTransactionId")):
            raise ValueError("Current DualLink keyboard counter prefix differs")
        prefix = 128
    branches = {}
    for physical in {value for value in (pid, dongle) if value is not None and value != 0}:
        dual = physical == dongle and is_dual
        mika = physical == dongle and is_mika
        interface = interfaces[str(physical)] if isinstance(interfaces, dict) else interfaces
        multiplier = 5 if dual or mika else 1
        branches[str(physical)] = {"source_class": radio_class if dual else "rzDevice25",
            "claim_interface": interface, "report_id": interface if dual else 0,
            "transaction_prefix": prefix if dual else 0, "transaction_modulus": 31,
            "max_retry_in": min(20, defaults["MAX_RETRY_IN"]) if mika else defaults["MAX_RETRY_IN"],
            "max_retry_out": min(20, defaults["MAX_RETRY_OUT"]) if mika else defaults["MAX_RETRY_OUT"],
            "sleep_between_out_ms": multiplier * defaults["SLEEP_TIME_BETWEEN_OUT"],
            "sleep_between_out_in_ms": multiplier * defaults["SLEEP_TIME_BETWEEN_OUT_IN"],
            "sleep_between_in_ms": multiplier * defaults["SLEEP_TIME_BETWEEN_IN"], "is_duallink": dual}
        if not (dual or mika):
            for option, field in (("sleepTimeBetweenOut", "sleep_between_out_ms"),
                                  ("sleepTimeBetweenOutIn", "sleep_between_out_in_ms"),
                                  ("sleepTimeBetweenIn", "sleep_between_in_ms"),
                                  ("maxRetryIn", "max_retry_in"), ("maxRetryOut", "max_retry_out")):
                if option in settings:
                    branches[str(physical)][field] = int(settings[option])
            branches[str(physical)]["max_retry_in"] = min(branches[str(physical)]["max_retry_in"], defaults["MAX_RETRY_IN"])
            branches[str(physical)]["max_retry_out"] = min(branches[str(physical)]["max_retry_out"], defaults["MAX_RETRY_OUT"])
    return branches


def feature_report_callers(parser, originals, selected_capabilities=None):
    """Retain current MW report framing owners, including physical factories.

    This inventory deliberately does not infer the mounted transport class from
    a generated capability's source_class. A wired device, its own dongle and
    another DualLink master can select different constructors and report IDs.
    """
    import evidence
    if selected_capabilities is None:
        import typed_tables
        capability_path = ROOT / typed_tables.READ_OUTPUT
        capability_raw = capability_path.read_bytes()
        _, constructors = typed_tables.read_table_constructors(capability_raw)
        if not constructors:
            raise ValueError("Read table has no production capability constructors")
        capabilities = [{"product_id": pid} for pid in constructors]
        capability_input = {"selection": "all production typed Rust read constructors",
            "path": rel(capability_path), "bytes": len(capability_raw),
            "sha256": hashlib.sha256(capability_raw).hexdigest(),
            "product_ids": list(constructors)}
    else:
        capabilities = selected_capabilities
        capability_input = {"selection": "explicit current-source products", "products": capabilities}
    # These three additional keyboard factories own nonzero-interface receiver
    # senders also used by the dedicated keyboard metadata/restore protocols.
    if selected_capabilities is None:
        capabilities += [{"product_id": pid} for pid in (631, 658, 756)
                         if not any(cap["product_id"] == pid for cap in capabilities)]
    result = {"capabilities": capability_input, "products": [],
              "scope": "Every production typed Rust device-read capability plus 631/658/756 receiver senders; complete retained current MW owners, not a claim of all product/factory implementation",
              "target_execution": False, "runtime_acceptance": "unverified"}
    methods = {b"constructor", b"connectDevice", b"connectHidDevice", b"sendCommand",
               b"_getUSBTransferInResult", b"_createDataSend", b"getFeatureReport",
               b"sendFeatureReport", b"_getTransactionId", b"_calculateChecksum"}

    def actual(path):
        raw = path.read_bytes()
        return raw, {"path": rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

    def indexed_identity(record):
        expected = originals.get(record["path"])
        return ("missing" if expected is None else "verified" if all(
            record[key] == expected.get(key) for key in ("bytes", "sha256")) else "mismatch")

    for cap in capabilities:
        pid = cap["product_id"]
        family = "keyboard" if 500 <= pid < 1000 else "mouse"
        namespace = f"{family}-mw-{pid}-current"
        root = ROOT / ".analysis/inputs/frontend" / namespace
        row = {"product_id": pid, "namespace": namespace,
               "selected_capability": cap, "sources": [], "failures": [],
               "selected_transport_semantics": "Inspect complete factory branches; source_class alone is not a physical transport selector"}
        result["products"].append(row)
        receipt_path = root / "acquisition.json"
        try:
            receipt_raw, receipt_identity = actual(receipt_path)
            receipt = json.loads(receipt_raw)
            row["acquisition"] = receipt_identity
            row["acquisition_index_status"] = indexed_identity(receipt_identity)
            if row["acquisition_index_status"] != "verified":
                row["failures"].append({"kind": "middleware_receipt_index_identity_unverified",
                    "status": row["acquisition_index_status"]})
            row["deployment_stability"] = receipt.get("stability", [])
            row["source_current_during_acquisition"] = receipt.get("source_current_during_acquisition")
            row["declared_references_fetched"] = receipt.get("declared_references_fetched")
            row["acquisition_unresolved"] = receipt.get("unresolved", [])
            if receipt.get("source_current_during_acquisition") is not True:
                row["failures"].append({"kind": "middleware_deployment_not_verified_stable"})
            deployment = evidence.frontend_deployment_sources(receipt)
            row["current_originals"] = [item["original"] for item in deployment["sources"]
                                        if item.get("original") and item.get("identity_verified")]
            current_paths = {item["path"] for item in row["current_originals"]}
            row["retained_without_current_reference"] = deployment["retained_without_current_reference"]
            row["deployment_failures"] = deployment["failures"]
            seen = set()
            for declared in receipt.get("files", []):
                name = declared.get("path", "")
                if not name.endswith(".js") or name in seen or name not in current_paths:
                    continue
                seen.add(name)
                path = resolve(name, rel(root))
                # Read source as inert bytes; parse only framing, connection,
                # factory or DeviceInfo carriers. Never execute vendor JS.
                if not path.is_file():
                    continue
                raw, source_identity = actual(path)
                protocol_file = (path.name.startswith("rzDevice25") or b'this.name="rzDevice25' in raw
                                 or b'this.name="rzDevice"' in raw)
                factory_file = b'getFeatures("rzDeviceType")' in raw
                connection_file = b"connectHidDevice({" in raw
                info_file = path.name.startswith("main.") and b"productId:" in raw
                constants_file = bool(re.search(rb'MAX_RETRY_IN:(?:\(\)=>|function\(\))', raw))
                if not (protocol_file or factory_file or connection_file or info_file or constants_file):
                    continue
                if any(source_identity[key] != declared.get(key) for key in ("bytes", "sha256")):
                    row["failures"].append({"kind": "middleware_original_receipt_mismatch", "actual": source_identity})
                    continue
                source_record = {"original": source_identity, "url": declared.get("url"),
                    "index_status": indexed_identity(source_identity), "owners": []}
                row["sources"].append(source_record)
                if source_record["index_status"] != "verified":
                    row["failures"].append({"kind": "middleware_original_index_identity_unverified",
                        "path": name, "status": source_record["index_status"]})
                tree = parser.parse(raw)
                if tree.root_node.has_error:
                    row["failures"].append({"kind": "middleware_ast_error", "path": name})
                    continue
                retained = set()

                def retain(node, role):
                    key = (node.start_byte, node.end_byte, role)
                    if key in retained:
                        return
                    retained.add(key)
                    body = raw[node.start_byte:node.end_byte]
                    source_record["owners"].append({"role": role, "syntax": node.type,
                        "byte_start": node.start_byte, "byte_end": node.end_byte,
                        "body_sha256": hashlib.sha256(body).hexdigest(), "body": body.decode("utf-8")})

                if factory_file:
                    for match in re.finditer(re.escape(b'getFeatures("rzDeviceType")'), raw):
                        node = tree.root_node.descendant_for_byte_range(match.start(), match.end())
                        while node is not None and not (node.type in {"arrow_function", "function_expression"}
                                and b"DeviceInfo.claimInterface" in node.text
                                and b"DeviceInfo.dongleId" in node.text):
                            node = node.parent
                        if node is None:
                            row["failures"].append({"kind": "complete_transport_factory_unresolved", "path": name})
                        else:
                            retain(node, "physical_transport_factory")
                if constants_file:
                    for match in re.finditer(rb'MAX_RETRY_IN:(?:\(\)=>|function\(\))', raw):
                        node = tree.root_node.descendant_for_byte_range(match.start(), match.end())
                        while node is not None:
                            parent = node.parent
                            key = parent.child_by_field_name("key") if parent is not None and parent.type == "pair" else None
                            if node.type in {"arrow_function", "function_expression"} and key is not None and key.type == "number":
                                retain(node, "transport_constants_module")
                                break
                            node = parent
                pending = [tree.root_node]
                protocol_classes = set()
                while pending:
                    node = pending.pop()
                    pending.extend(reversed(node.named_children))
                    if node.type == "method_definition":
                        method = node.child_by_field_name("name")
                        owner = node.parent.parent if node.parent is not None else None
                        in_protocol = owner is not None and (owner.start_byte, owner.end_byte) in protocol_classes
                        if method is not None and method.text in methods and (
                                in_protocol or method.text == b"connectHidDevice"):
                            retain(node, method.text.decode())
                    elif protocol_file and node.type in {"class_declaration", "class"} and (
                            b'this.name="rzDevice25' in node.text or b'this.name="rzDevice"' in node.text):
                        # This complete declaration preserves inheritance,
                        # constructor initialization and assigned method overrides.
                        retain(node, "complete_protocol_class" if b'this.name="rzDevice25' in node.text else "complete_protocol_base_class")
                        protocol_classes.add((node.start_byte, node.end_byte))
                    elif info_file and node.type == "variable_declarator":
                        value = node.child_by_field_name("value")
                        if value is not None and value.type == "object" and b"productId:" in value.text:
                            fields = {child.child_by_field_name("key").text: child.child_by_field_name("value")
                                for child in value.named_children if child.type == "pair"
                                and child.child_by_field_name("key") is not None}
                            product = fields.get(b"productId")
                            if product is not None and product.text == str(pid).encode() and (
                                    b"claimInterface" in fields or {b"minDPI", b"maxDPI", b"vendorId", b"category"}.issubset(fields)):
                                retain(node, "device_info")
                    elif info_file and node.type == "call_expression":
                        body = node.text
                        if b"rzDeviceType" in body[:150] or b"isDualLinkDevice" in body[:150] or b"isMikaDevice" in body[:150]:
                            retain(node, "transport_feature_registration")
                if actual(path)[1] != source_identity:
                    row["failures"].append({"kind": "middleware_original_changed_during_query", "path": name})
            roles = Counter(owner["role"] for source in row["sources"] for owner in source["owners"])
            row["owner_counts"] = dict(roles)
            for role in ("physical_transport_factory", "device_info", "connectHidDevice", "sendCommand", "_getUSBTransferInResult"):
                if not roles[role]:
                    row["failures"].append({"kind": "middleware_required_owner_missing", "role": role})
            if actual(receipt_path)[1] != receipt_identity:
                row["failures"].append({"kind": "middleware_acquisition_changed_during_query"})
            try:
                row["physical_transports_by_pid"] = feature_physical_transports(row, parser, originals)
            except Exception as exc:
                row["failures"].append({"kind": "physical_transport_factory_unresolved", "error": str(exc)})
        except Exception as exc:
            row["failures"].append({"kind": "feature_report_caller_query_failure", "error": str(exc).replace(str(ROOT), ".")})
        row["source_owners_complete"] = not row["failures"]
    result["products_total"] = len(result["products"])
    result["source_owners_complete"] = all(row["source_owners_complete"] for row in result["products"])
    if selected_capabilities is None and capability_path.read_bytes() != capability_raw:
        raise ValueError("Read capability table changed during evidence inventory")
    result["limitations"] = ["Missing indexed originals/receipts require acquisition repair before implementation evidence is complete",
        "Complete protocol classes include unselected alternatives; use the full product registration and physical factory before selecting framing",
        "GET truthy fallback, SET nullish fallback, Windows connection query and source noSlice handling remain distinct host branches",
        "This inventory does not establish native execution, physical descriptor lengths or UI/device acceptance"]
    return result


def hid_indirect_evidence(module, native_path, capture_dir, manifest, exports, strings,
                          functions, function, identity, selected):
    """Resolve only byte-backed HID globals and constructor-proven table slots."""
    base = int(manifest["image_base"], 16)
    raw = native_path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != module["original"]["sha256"]:
        raise ValueError("HID input changed while resolving indirect calls")
    string_addresses = {int(row["address"], 16): row for row in strings}
    bindings, sites = {}, {}

    def support(rva, anchors=()):
        address = base + rva
        body, ref = function(address)
        text = "\n".join(line["text"] for line in (body.get("pseudocode") or {}).get("lines", []))
        if not all(anchor in text for anchor in anchors):
            raise ValueError("HID indirect-call evidence requires a new semantic audit: " + hex(rva))
        selected.add(address)
        return body, native_chain_function_reference(ref)

    init_address = int(exports["hid_init"]["address"], 16)
    init, init_ref = support(init_address - base, ("LoadLibraryA(\"hid.dll\")", "GetProcAddress",
        "FreeLibrary(hLibModule)", "byte_18005E2AC = 1", "byte_18005E2AC = 0", "0xFFFFFFFF"))
    _, exit_ref = support(int(exports["hid_exit"]["address"], 16) - base,
                         ("FreeLibrary(hLibModule)", "hLibModule = 0", "byte_18005E2AC = 0"))
    instructions = init["assembly"]
    for index, instruction in enumerate(instructions):
        if not re.match(r"call\s+cs:GetProcAddress$", instruction["text"]):
            continue
        arguments = []
        for previous in instructions[max(0, index - 2):index]:
            if re.match(r"lea\s+rdx,", previous["text"]):
                for edge in previous["xrefs_from"]:
                    string = string_addresses.get(int(edge["to"], 16))
                    if string and re.fullmatch(r"Hid[DP]_[A-Za-z]+", string["text"]):
                        arguments.append((previous, string))
        if len(arguments) != 1 or index + 1 >= len(instructions):
            raise ValueError("GetProcAddress argument is not statically unique")
        store = instructions[index + 1]
        destinations = [edge for edge in store["xrefs_from"] if edge["type"] == 2]
        if not re.match(r"mov\s+cs:[^,]+, rax$", store["text"]) or len(destinations) != 1:
            raise ValueError("GetProcAddress result is not stored in one proven global")
        argument, string = arguments[0]
        global_address = int(destinations[0]["to"], 16)
        bindings[global_address] = {"module": "hid.dll", "name": string["text"],
            "pointer_address": hex(global_address), "pointer_rva": hex(global_address - base),
            "initialization_function": init_ref, "name_literal": string,
            "argument_instruction": argument, "resolver_instruction": instruction,
            "store_instruction": store}
    if len(bindings) != 12:
        raise ValueError("Original hid_init dynamic function set changed")
    module["dynamic_import_bindings"] = list(bindings.values())
    module["hid_module_lifecycle"] = {"init": init_ref, "exit": exit_ref,
        "semantics": "hid_init returns zero if already initialized; otherwise it loads hid.dll and requires all 12 GetProcAddress results. Any failure frees the module, clears the handle/initialized flag and returns -1. hid_exit frees the module and clears those two fields; stored function pointers are not individually zeroed."}

    tables = {rva: native_pointer_targets(raw, rva, count)
              for rva, count in ((0x51c60, 5), (0x51a60, 3), (0x51ba0, 1))}
    table = tables[0x51a60]
    selection = hashlib.sha256(json.dumps(table, sort_keys=True).encode()).hexdigest()[:16]
    supplement_path = capture_dir / "supplements" / selection / "capture.json"
    supplemental = json.loads(supplement_path.read_bytes())
    if (supplemental.get("input_sha256") != module["original"]["sha256"]
            or supplemental.get("pointer_table") != table
            or not supplemental.get("capture_complete") or supplemental.get("failures")):
        raise ValueError("HID shared-worker table supplement is incomplete or differs")
    identity(capture_dir / "capture.json", supplemental["base_capture"])
    for row in supplemental["functions"]:
        path = supplement_path.parent / row["path"]
        identity(path, row)
        address = int(row["address"], 16)
        functions[address] = {**row, "path": path.relative_to(capture_dir).as_posix()}
    for row in supplemental["references"]:
        identity(ROOT / row["path"], row)
    module["native_supplements"] = [identity(supplement_path)]
    module["native_pointer_tables"] = list(tables.values())

    _, job_ref = support(0x4800, ("switch (", "case 3:", "off_180051C60", "sub_180016370", "sub_1800164B0"))
    _, copy_ref = support(0x137d0, ("*a2 = off_180051C60", "a2[1] = *(_QWORD *)(a1 + 8)", "return a2"))
    _, destroy_ref = support(0x13590, ("if ( a2 )", "sub_1800184D0(a1)"))
    _, batch_ref = support(0x16370, ("v7 - 240", "Sleep(v14)", "a1[10]", "+ 16LL))(v13, &v19, &v18)"))
    support(0x13700, ("qword_18005E2D8", "return v5", "0xFFFFFFFF"))
    cleanup, cleanup_ref = support(0x164b0, ("v2[10]", "*v3 + 32", "sub_1800184D0(v2)"))
    _, constructor_ref = support(0x5440, ("off_180051A60", "sub_1800045D0", "sub_180010210",
        "_InterlockedExchangeAdd(v45 + 2", "_InterlockedExchangeAdd(v45 + 3", "*(_QWORD *)(a1 + 56) = 0"))
    _, map_ref = support(0x10210, ("*(_QWORD *)(v23 + 64) = 0", "*(_QWORD *)(v23 + 72) = 0"))
    _, worker_ref = support(0x45d0, ("off_180051BA0", "sub_1800050E0", "a1 + 216"))
    _, worker_destroy_ref = support(0x4710, ("lpMem[200] = 1", "sub_1800195E8", "GetCurrentThreadId", "sub_18001BB64"))
    support(0x10ae0)
    support(0x10ac0, ("+ 16LL))(a1, 1)",))
    support(0x12200, ("off_180051A60", "sub_1800184D0(a1)"))

    def bind(owner_rva, site_rva, table_rva, slot, expected_instruction, context, proof):
        body, owner_ref = function(base + owner_rva)
        instruction = next((item for item in body["assembly"] if int(item["rva"], 16) == site_rva), None)
        if instruction is None or not re.fullmatch(expected_instruction, instruction["text"]):
            raise ValueError("HID indirect instruction changed: " + hex(site_rva))
        pointer = tables[table_rva]["targets"][slot]
        target = int(pointer["address"], 16)
        _, target_ref = function(target)
        selected.add(target)
        sites[hex(site_rva)] = {"resolution": "constructor_and_original_pointer_table",
            "owner": native_chain_function_reference(owner_ref), "table_rva": hex(table_rva),
            "pointer": pointer, "target_function": native_chain_function_reference(target_ref),
            "context": context, "supporting_functions": proof}

    for site, slot, instruction in ((0x4a1c, 0, r"call\s+r8"), (0x4a62, 0, r"call\s+qword ptr \[rax\]"),
            (0x4a86, 4, r"call\s+qword ptr \[rax\+20h\]"), (0x4ac1, 4, r"call\s+qword ptr \[rax\+20h\]")):
        bind(0x4800, site, 0x51c60, slot, instruction,
            "executeJob case 3 constructs lambda_2 on the stack; slot 0 copies the table and captured Job pointer unchanged; slot 4 frees storage only when the supplied heap flag is true.",
            [job_ref, copy_ref, destroy_ref])
    bind(0x16370, 0x1645f, 0x51c60, 2, r"call\s+qword ptr \[rax\+10h\]",
         "For the executeJob case-3 caller, the batch object's callback is the copied lambda_2; slot 2 performs HidD_SetFeature. This is a caller-specific binding, not an assertion about arbitrary callers.", [job_ref, copy_ref, batch_ref])
    cleanup_calls = [item for item in cleanup["assembly"] if re.fullmatch(r"call\s+qword ptr \[rax\+20h\]", item["text"])]
    if len(cleanup_calls) != 1:
        raise ValueError("Batch cleanup callback is not unique")
    bind(0x164b0, int(cleanup_calls[0]["rva"], 16), 0x51c60, 4, r"call\s+qword ptr \[rax\+20h\]",
         "The executeJob case-3 cleanup releases its copied lambda before freeing the batch object.", [job_ref, copy_ref, cleanup_ref])
    for site, slot in ((0x5beb, 0), (0x5c00, 1), (0x5c78, 0), (0x5c8c, 1)):
        bind(0x5440, site, 0x51a60, slot,
             r"call\s+qword ptr \[rax\]" if slot == 0 else r"call\s+qword ptr \[rax\+8\]",
             "The path map stores the shared worker/control-block allocated by this constructor. Strong-count zero destroys the embedded worker; weak-count zero destroys/deallocates its control block. Newly inserted map entries and this constructor's previous member reference are zero, so their release branches are skipped on fresh construction.",
             [constructor_ref, map_ref, worker_ref])
    bind(0x10ac0, 0x10acd, 0x51a60, 2, r"jmp\s+qword ptr \[rax\+10h\]",
         "For the shared-worker table, weak-count cleanup dispatches its deleting destructor with flag 1.", [constructor_ref])
    bind(0x10ae0, 0x10aea, 0x51ba0, 0, r"jmp\s+qword ptr \[rax\]",
         "The byte-exact forwarding thunk adjusts this by 16 and invokes the embedded worker destructor with delete flag 0; the containing control block owns the allocation.", [constructor_ref, worker_ref, worker_destroy_ref])
    module["indirect_resolution_scope"] = "HID dynamic globals, executeJob case-3 batch lambda, and HID constructor shared-worker ownership. Context-specific table targets do not prove arbitrary callers of reusable compiler helpers."
    return bindings, sites


def merge_native_chain_modules(previous, current):
    """Retain scoped evidence only while its referenced bytes are still current."""
    identities = {}

    def identity(name):
        if name not in identities:
            path = resolve(name, ".")
            identities[name] = {"path": rel(path), "bytes": path.stat().st_size, "sha256": sha(path)}
        return identities[name]

    def verify(value, ancestry=()):
        # The capture manifest retains the analyzer script identity at capture
        # time. Its current checkout bytes are not an input to that capture.
        if ancestry == ("analyzer", "capture_tool"):
            return
        if isinstance(value, dict):
            if {"path", "bytes", "sha256"} <= value.keys():
                actual = identity(value["path"])
                if any(actual[key] != value[key] for key in ("path", "bytes", "sha256")):
                    raise ValueError("Retained native-chain evidence changed: " + value["path"])
            for key, item in value.items():
                verify(item, (*ancestry, key))
        elif isinstance(value, list):
            for index, item in enumerate(value):
                verify(item, (*ancestry, index))

    def unique(rows, key):
        return list({key(row): row for row in rows}.values())

    retained = {module["name"]: module for module in previous}
    selected = {module["name"]: module for module in current}
    if len(retained) != len(previous) or len(selected) != len(current):
        raise ValueError("Duplicate native-chain module identity")
    for name, old in retained.items():
        if name not in selected:
            verify(old)
            continue
        fresh = selected[name]
        if fresh.get("original") is not None and old.get("original") != fresh["original"]:
            continue
        # Rust changes invalidate an implementation assessment, independently
        # of unchanged original-source and disassembly semantics.
        verify({key: value for key, value in old.items() if key != "rust_owners"})
        if "original" not in fresh and "original" in old:
            fresh["original"] = old["original"]
        owners = {row["path"]: row for row in old.get("rust_owners", [])}
        owners.update({row["path"]: row for row in fresh.get("rust_owners", [])})
        current_owners = [identity(name) for name in owners if resolve(name, ".").is_file()]
        old_owners = {row["path"]: row for row in old.get("rust_owners", [])}
        owners_unchanged = old_owners == {row["path"]: row for row in current_owners}
        fresh["rust_owners"] = current_owners
        if owners_unchanged:
            if "implementation_acceptance" in old:
                fresh["implementation_acceptance"] = old["implementation_acceptance"]
        else:
            fresh["implementation_acceptance"] = (
                "Rust owners changed; source-to-implementation acceptance requires a new review. "
                "Revalidated source evidence does not establish implementation completion.")
        for field in ("semantic_evidence", "verified_semantics", "remaining_semantic_work",
                      "implementation_adaptations", "rotation_native", "gamepad_native"):
            if field not in fresh and field in old:
                fresh[field] = old[field]
        for field, key in (
            ("sources", lambda row: row["original"]["path"]),
            ("native_functions", lambda row: row["path"]),
            ("native_edges", lambda row: (row["caller"], row["from"], row["to"], row["type"])),
            ("unresolved_indirect_calls", lambda row: row["address"]),
            ("resolved_indirect_calls", lambda row: row["address"]),
        ):
            fresh[field] = unique([*old.get(field, []), *fresh.get(field, [])], key)
        resolved = {row["address"] for row in fresh["resolved_indirect_calls"]}
        fresh["unresolved_indirect_calls"] = [
            row for row in fresh["unresolved_indirect_calls"] if row["address"] not in resolved]
        fresh["all_linked_function_bodies_verified"] = all(
            row["full_assembly"] for row in fresh["native_functions"])
    retained.update(selected)
    return list(retained.values())


def mapping_gamepad_observer_evidence(module, native_path, function, selected):
    """Retain the output callback chain with PE-backed functions and vtables."""
    from host import inspect_pe

    raw = native_path.read_bytes()
    pe = inspect_pe(raw)
    base = int(module["image_base"], 16)
    rvas = (0x41C80, 0x42490, 0x42CF0, 0x43550, 0x43F20,
            0xF360, 0xF550, 0xF640, 0xF830, 0xF920, 0xFAF0, 0xFBE0, 0xFE10,
            0x5BD60, 0x5BE90, 0x5BFC0, 0x5C0F0,
            0x757E0, 0x759D0, 0x75B50, 0x75D40, 0x75EC0, 0x760A0, 0x76210, 0x76420,
            0xAFB14, 0x10F582, 0x81D6E, 0x81FB6, 0x82220, 0x9E490, 0xA425C,
            0x1054A8, 0x105590, 0x1055F0)
    anchors = {
        0x759D0: ["*(_DWORD *)(a1 + 512) = 0", "*(_DWORD *)(a1 + 516) = a2"],
        0x75D40: ["257", "1000 * a2"],
        0x760A0: ["*(_WORD *)(a1 + 528) = 0", "not registered"],
        0x76420: ["a1 + 520", "a1 + 568"],
        0x81D6E: ["*(_DWORD *)(a1 + 516) + 1", "*(_DWORD *)(a1 + 512) < v4", "sub_18009E4A2"],
        0x81FB6: ["0x2711", "a1 + 536", "sub_180082220"],
        0x82220: ["a1 + 544", "a1 + 528", "a1 + 568", "a1 + 530"],
        0xAFB14: ["sub_1800FBCB4", "a1 + 328", "sub_18010F582", "sub_180081FB6", "sub_180081D6E"],
        0x1055F0: ['"buttons"', '"leftTrigger"', '"rightTrigger"', '"leftThumbX"', '"rightThumbY"'],
    }
    refs = []
    for rva in rvas:
        body, ref = function(base + rva)
        if not ref["full_assembly"] or not ref["full_pseudocode"]:
            raise ValueError("Gamepad callback capture is incomplete: " + hex(rva))
        text = "\n".join(line["text"] for line in body["pseudocode"]["lines"])
        if not all(anchor in text for anchor in anchors.get(rva, [])):
            raise ValueError("Gamepad callback semantics require a new source audit: " + hex(rva))
        for chunk in body["chunks"]:
            start = int(chunk["start"], 16) - base
            captured = bytes.fromhex(chunk["raw_hex"])
            regions = [section for section in pe["sections"]
                       if 0 <= start - section["rva"] and start - section["rva"] + len(captured) <= section["bytes"]]
            if len(regions) != 1:
                raise ValueError("Gamepad function is not uniquely file-backed: " + hex(start))
            offset = regions[0]["file_offset"] + start - regions[0]["rva"]
            if raw[offset:offset + len(captured)] != captured:
                raise ValueError("Gamepad function differs from original PE bytes: " + hex(start))
        selected.add(base + rva)
        refs.append(ref)
    tables = []
    for rva, count, expected in (
        (0x2E4C20, 4, [0x5BD60, 0x5BE90, 0x5BFC0, 0x5C0F0]),
        *((rva, 4, [0x757E0, 0x75B50, 0x75EC0, 0x76210])
          for rva in (0x2E41A0, 0x2E5830, 0x2E5C90, 0x2E7B20, 0x2E8000)),
        (0x2EF5C8, 2, [0x105590, 0x1055F0]),
    ):
        table = native_pointer_targets(raw, rva, count)
        if [int(row["rva"], 16) for row in table["targets"]] != expected:
            raise ValueError("Gamepad callback vtable changed: " + hex(rva))
        tables.append(table)
    host = module["sources"][0]
    names = {"registerXboxGamepadEvent", "registerIntervaledXboxGamepadEvent",
             "unregisterXboxGamepadEvent", "setXboxGamepadEventCallback", "createCbXboxGamepadEvent"}
    callers = [row for row in host["callables"] if row["name"] in names]
    if {row["name"] for row in callers} != names:
        raise ValueError("Gamepad host callable ownership changed")
    return {
        "host": host["original"], "host_callables": callers,
        "native_functions": refs, "native_pointer_tables": tables,
        "native_semantic_anchors": {hex(key): value for key, value in anchors.items()},
        "frontend_query": ".analysis/results/frontend/keyboard-mw-audit.json#queries.gamepad_tester",
        "verified_semantics": [
            "The host requires an initialized engine and existing USB identity, registers the native stream only for the first subscriber, retains its interval/skip, and unregisters at the last subscriber. Normal subscribers accept different skip arguments. Stored interval zero is the normal sentinel even after an initial native interval-zero registration.",
            "Exports queue through F360/F640/F920/FBE0, the registered-device manager5BD60/5BE90/5BFC0/5C0F0 and device757E0/75B50/75EC0/76210. Actor759D0/75D40 rejects duplicate registration;760A0 rejects absent unregister;76420 retains both callbacks and acknowledges installation.",
            "Normal registration759D0 initializes threshold+512 to zero and counter+516 to the supplied skip.81D6E increments the wrapping counter and compares it with threshold; skip50 therefore emits every changed sample. Counter wrapping can suppress one sample unless a new button press, trigger0/255, or signed thumb endpoint forces it.",
            "AFB14 compares actual computed output banks, requires the retained driver-thread owner, queues SendInput10F582 and immediately schedules observation without waiting for IOCTL completion. It performs no device GET or output readback. Unchanged banks emit nothing, and registration produces no initial zero snapshot.",
            "81FB6 retains every changed output in order, uses wrapping milliseconds*1000, flushes at10001 samples and otherwise schedules from the previous scheduling time.82220 drains the batch, requires enabled callback ownership, stamps the batch at flush and clears scheduled state; old capacity-flush timers may still run.",
            "MappingDataXboxInput equality105590 compares the twelve output bytes. Serializer1055F0 emits buttons, unsigned triggers and signed thumb axes. Host createCbXboxGamepadEvent forwards type8 JSON output only to registered live web contents with matching USB identity.",
        ],
        "implementation_scope": "Rust computed-output subscription, typed IPC, shared acknowledged session, device callback owner, host scope fanout and existing PID679 tester consumer are implemented. Output aggregation is shared with driver reports. Retired event generations and queued interval samples are discarded on stop; execution failure remains separate from computed sample provenance.",
        "remaining_local_chains": [
            "PID678/688 classic tester mapping ownership and modern interval5 viewport/firmware output branches remain incomplete; PID679 closure does not establish completion for the other eleven frontend products.",
            "Real-device output, driver failure timing, vendor-DLL comparison, reconnect and actual-window tester acceptance remain unverified. Static captures and pure/mock tests are independent of runtime acceptance.",
            "Joystick callback subscriptions and other mapping-engine product/version branches remain outside this output-observer implementation.",
        ],
        "implementation_adaptations": [
            "Native callback ownership is represented by scoped UUID generations and typed IPC; a completed stop discards retired batches so old results cannot cross page/connection replacement.",
            "Rust keeps the last subscriber until native-equivalent stop succeeds, permitting truthful cleanup failure and retry. The host removes its URL before awaiting native unregister.",
            "Computed samples queued before an output failure are delivered once, then the retained transport error is surfaced independently; samples never constitute driver confirmation.",
        ],
        "validation": ".analysis/validation.json records executed Rust suites separately; runtime acceptance remains unverified.",
    }


def audit_native_chain(args):
    """Join verified host source/FFI declarations with complete native captures.

    This is an evidence query. It reads raw JS as an AST and saved IDA records;
    no JavaScript, target export, vendor service or device is executed.
    """
    import bisect
    import importlib.metadata
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    parser = Parser(Language(tree_sitter_javascript.language()))
    indexed = json.loads((ROOT / ".analysis/inputs.json").read_bytes())
    originals = {item["path"]: item for item in indexed["inputs"]}
    result = {"schema_version": 2, "recorded_at_utc": stamp(), "query": args.query,
              "tool": {"path": "tools/analysis.py", "sha256": sha(Path(__file__))},
              "parsers": {name: importlib.metadata.version(name) for name in ("tree-sitter", "tree-sitter-javascript")},
              "modules": [], "failures": [], "target_execution": False,
              "real_device_operations": False, "runtime_acceptance": "unverified",
              "semantic_scope": "Full original callable bodies, literal FFI signatures, exact exports and immediate native xrefs; unresolved indirect calls remain explicit"}

    def identity(path, expected=None):
        path = resolve(rel(path), ".analysis") if path.is_relative_to(ROOT / ".analysis") else path
        actual = {"path": rel(path), "bytes": path.stat().st_size, "sha256": sha(path)}
        if expected and any(actual[key] != expected[key] for key in ("bytes", "sha256") if key in expected):
            raise ValueError("Evidence identity changed: " + actual["path"])
        return actual

    def original(path):
        row = originals.get(rel(path))
        if row is None:
            raise ValueError("Original is absent from input index: " + rel(path))
        return identity(path, row)

    installed_asars = [row for row in originals.values() if row["path"].startswith(
        ".analysis/inputs/installed-current/files/app-engine/") and row["path"].endswith("/app.asar")]
    if len(installed_asars) != 1:
        raise ValueError("Expected one acquired installed AppEngine ASAR")
    asar = installed_asars[0]
    result["installed_asar"] = original(ROOT / asar["path"])
    official_asars = [row for row in originals.values() if row["path"].startswith(
        ".analysis/inputs/host-current/extracted/") and row["sha256"] == asar["sha256"]]
    result["official_asar_matches"] = [original(ROOT / row["path"]) for row in official_asars]
    if not official_asars:
        result["failures"].append({"kind": "installed_host_not_matched_to_current_official_package"})
    source_root = ROOT / ".analysis/inputs/host-current/asar" / asar["sha256"][:12]
    acquisition = ROOT / ".analysis/inputs/host-current/acquisition.json"
    result["host_acquisition"] = original(acquisition)
    acquired = json.loads(acquisition.read_bytes())
    result["official_stability"] = acquired.get("stability", [])
    result["stable_products"] = [{"name": row["name"], "version": row["version"]}
                                  for row in acquired.get("stable_products", [])]
    if acquired.get("failures") or not result["official_stability"] or not all(
        row.get("unchanged") for row in result["official_stability"]):
        result["failures"].append({"kind": "host_acquisition_or_stability_incomplete"})

    def source(path):
        record = {"original": original(path), "callables": [], "ffi_declarations": []}
        raw = path.read_bytes()
        tree = parser.parse(raw)
        if tree.root_node.has_error:
            raise ValueError("Original JavaScript has an AST error: " + rel(path))
        pending = [tree.root_node]
        while pending:
            node = pending.pop()
            pending.extend(reversed(node.named_children))
            name = None
            if node.type in {"method_definition", "field_definition", "function_declaration"}:
                name = node.child_by_field_name("name") or node.child_by_field_name("property")
            elif node.type == "assignment_expression":
                right = node.child_by_field_name("right")
                if right and right.type in {"function_expression", "arrow_function"}:
                    name = node.child_by_field_name("left")
            elif node.type in {"variable_declarator", "pair"}:
                value = node.child_by_field_name("value")
                if value and value.type in {"function_expression", "arrow_function"}:
                    name = node.child_by_field_name("name") or node.child_by_field_name("key")
            elif node.type == "call_expression":
                # Transpiled SDKs register named methods as
                # defineProperty(owner, "method", function (...) {...}).
                # Keep the complete registration/callable, not a text excerpt.
                arguments = node.child_by_field_name("arguments")
                children = arguments.named_children if arguments else []
                if (len(children) >= 2 and children[-1].type in {"function_expression", "arrow_function"}
                        and children[-2].type == "string"):
                    name = children[-2]
            if name is not None:
                body = raw[node.start_byte:node.end_byte]
                record["callables"].append({"name": raw[name.start_byte:name.end_byte].decode("utf-8"),
                    "node_type": node.type, "byte_start": node.start_byte, "byte_end": node.end_byte,
                    "body_sha256": hashlib.sha256(body).hexdigest(), "body": body.decode("utf-8")})
            if node.type == "pair":
                key, value = node.child_by_field_name("key"), node.child_by_field_name("value")
                if not key or not value or value.type != "array":
                    continue
                try:
                    literal = json.loads(raw[value.start_byte:value.end_byte])
                except (ValueError, UnicodeDecodeError):
                    continue
                if (isinstance(literal, list) and len(literal) == 2 and isinstance(literal[0], str)
                        and isinstance(literal[1], list) and all(isinstance(x, str) for x in literal[1])):
                    name = raw[key.start_byte:key.end_byte].decode("utf-8")
                    if key.type == "string":
                        name = json.loads(name)
                    record["ffi_declarations"].append({"name": name, "return_type": literal[0],
                        "argument_types": literal[1], "byte_start": node.start_byte, "byte_end": node.end_byte,
                        "body": raw[node.start_byte:node.end_byte].decode("utf-8"),
                        "body_sha256": hashlib.sha256(raw[node.start_byte:node.end_byte]).hexdigest()})
        record["complete_module_bytes"] = len(raw)
        return record

    configurations = [
        ("hid-feature", "resources/app.asar.unpacked/node_modules/node-rz-hid/build/Release/HID.node",
         ["electron/UsbRzDeviceAction.js", "electron/lib/RzMutx.js", "node_modules/node-rz-hid/nodehid.js"],
         ["crates/razer-hid/src/transport/hidapi.rs", "crates/razer-service/src/runtime/windows/hid/hid_transport.rs",
          "crates/razer-device/src/transport/feature_exchange.rs", "crates/razer-device/src/transport/device_query.rs",
          "crates/razer-device/src/transport/device_reads.rs", "crates/razer-device/src/transport/backend.rs",
          "crates/razer-device/src/receiver/receiver_pairing.rs",
          "crates/razer-device/src/receiver/receiver_capabilities.rs",
          "crates/razer-device/src/keyboard/keyboard_metadata.rs",
          "crates/razer-device/src/keyboard/keyboard_settings.rs",
          "crates/razer-device/src/keyboard/keyboard_analog.rs"]),
        ("mapping-engine", "CommonDLL/mapping_engine.dll", ["electron/modules/mapping_engine/win/index.js"],
         ["crates/razer-service/src/runtime/windows/mapping_native_session.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/driver.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/driver/rotation.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/connections.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/backend.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/backend/rotation.rs",
          "crates/razer-service/src/runtime/windows/native.rs",
          "crates/razer-service/src/runtime/portable.rs",
          "crates/razer-platform/src/input/global_shortcuts.rs",
          "crates/razer-service/src/mapping/hotkeys.rs",
          "crates/razer-service/src/mapping/mouse_rotation.rs",
          "crates/razer-service/src/mouse/mouse_rotation.rs",
          "crates/razer-service/src/mouse/worker.rs",
          "crates/razer-ipc/src/mouse_rotation.rs",
          "crates/razer-shell/src/shell/mouse/mouse_rotation.rs",
          "crates/razer-shell/src/shell/mouse/mouse_rotation_save.rs",
          "crates/razer-shell/src/shell.rs",
          "crates/razer-shell/src/shell/persistence.rs",
          "crates/razer-shell/src/shell/devices/device_discovery.rs",
          "crates/razer-pages/src/features/mouse/mouse_rotation.rs",
          "crates/razer-pages/src/features/mouse/mouse_rotation_state.rs",
          "crates/razer-pages/src/features/workspace/product_workspace.rs",
          "crates/razer-pages/src/features/workspace/source_workspace.rs",
          "crates/razer-device/src/mapping/mapping_native_rotation.rs",
          "crates/razer-device/src/mapping/mapping_native_output.rs",
          "crates/razer-device/src/mapping/mapping_native_gamepad_observation.rs",
          "crates/razer-device/src/mapping/mapping_native_observation.rs",
          "crates/razer-ipc/src/gamepad_observation.rs",
          "crates/razer-ipc/src/lib.rs",
          "crates/razer-service/src/mapping/gamepad_observation.rs",
          "crates/razer-service/src/mapping/observation.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/direct.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/direct/output_state.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/device.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/device/gamepad_observation.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/backend/gamepad_observation.rs",
          "crates/razer-service/src/runtime/windows/mapping_runtime/executor.rs",
          "crates/razer-pages/src/features/keyboard/actuation/keyboard_analog_gamepad.rs"]),
        ("simple-service", "CommonDLL/simple_service.dll", ["electron/modules/simple_service/win/index.js"],
         ["crates/razer-device/src/host/user_app_files.rs"]),
        ("notification-restart", "CommonDLL/SysUtilsNative.dll", ["electron/modules/sysutil/win/index.js"],
         ["crates/razer-shell/src/shell/navigation/main_pages/dashboard_cards.rs",
          "crates/razer-dashboard/src/restart_notification.rs",
          "crates/razer-shell/src/shell/windows/restart_notification.rs",
          "crates/razer-platform/src/system.rs", "crates/razer-platform/src/platform/windows/system.rs"]),
        ("startup-settings", "CommonDLL/SysUtilsNative.dll", ["electron/modules/sysutil/win/index.js"],
         ["crates/razer-settings/src/preferences/settings_page.rs",
          "crates/razer-shell/src/shell/host/startup.rs", "crates/razer-shell/src/shell/tray.rs",
          "crates/razer-app/src/lib.rs", "crates/razer-platform/src/startup.rs",
          "crates/razer-platform/src/platform/windows/startup.rs"]),
    ]
    for label, native_relative, sources, rust in configurations:
        if args.query not in {"all", label}:
            continue
        module = {"name": label, "sources": [], "ffi_exports": [], "native_functions": [],
                  "native_edges": [], "unresolved_indirect_calls": [], "failures": [],
                  "implementation_acceptance": "not established by source/evidence presence"}
        result["modules"].append(module)
        try:
            native_path = ROOT / ".analysis/inputs/installed-current/files/app-engine" / native_relative
            module["original"] = original(native_path)
            module["official_package_matches"] = [original(ROOT / row["path"]) for row in originals.values()
                if row["path"].startswith(".analysis/inputs/host-current/extracted/")
                and row["sha256"] == module["original"]["sha256"]]
            if not module["official_package_matches"]:
                module["failures"].append({"kind": "installed_native_not_matched_to_official_package"})
            capture_dir = ROOT / ".analysis/results/native" / module["original"]["sha256"]
            capture_path = capture_dir / "capture.json"
            manifest = json.loads(capture_path.read_bytes())
            module["capture"] = identity(capture_path)
            if manifest["input_sha256"] != module["original"]["sha256"] or not manifest.get("capture_complete"):
                raise ValueError("Native capture is incomplete or has different original bytes")
            module["analyzer"] = {key: manifest[key] for key in ("ida_version", "hexrays_version", "python_version", "capture_tool")}
            module["native_capture_failures"] = manifest["failures"]
            module["image_base"] = manifest["image_base"]
            outputs = {row["path"]: row for row in manifest["outputs"]}
            def dataset(name):
                path = capture_dir / (name + ".json")
                identity(path, outputs[path.name])
                return json.loads(path.read_bytes())
            entries, imports = dataset("entrypoints"), dataset("imports")
            module["exports_dataset"] = identity(capture_dir / "entrypoints.json", outputs["entrypoints.json"])
            module["imports_dataset"] = identity(capture_dir / "imports.json", outputs["imports.json"])
            exports = {row["name"]: row for row in entries}
            functions = {int(row["address"], 16): row for row in manifest["functions"]}
            starts = sorted(functions)
            cached = {}
            def function(address):
                if address not in cached:
                    row = functions[address]
                    path = capture_dir / row["path"]
                    ref = identity(path, row)
                    data = json.loads(path.read_bytes())
                    for chunk in data["chunks"]:
                        if hashlib.sha256(bytes.fromhex(chunk["raw_hex"])).hexdigest() != chunk["sha256"]:
                            raise ValueError("Native function bytes disagree with captured hash")
                    cached[address] = (data, {**ref, "name": data["name"], "rva": data["rva"],
                        "address": data["address"], "end": data["end"], "type": data["type"],
                        "full_assembly": data["assembly"] is not None, "full_pseudocode": data["pseudocode"] is not None,
                        "xrefs_to_entry": data["xrefs_to_entry"], "chunks": [
                            {k: value for k, value in chunk.items() if k != "raw_hex"} for chunk in data["chunks"]]})
                return cached[address]
            def owner(address):
                index = bisect.bisect_right(starts, address) - 1
                if index < 0:
                    return None
                candidate = starts[index]
                body, _ = function(candidate)
                return candidate if any(int(c["start"], 16) <= address < int(c["end"], 16) for c in body["chunks"]) else None
            selected = set()
            dynamic_bindings, indirect_sites = {}, {}
            module["sources"] = [source(source_root / name) for name in sources]
            for src in module["sources"]:
                for declaration in src["ffi_declarations"]:
                    name = declaration["name"]
                    sysutil_names = {
                        "notification-restart": {"restartWorkstation"},
                        "startup-settings": {"applicationAutoStart", "getApplicationAutoStartApps", "setApplicationAutoStart", "FreeMalloc"},
                    }.get(label)
                    if sysutil_names is not None and name not in sysutil_names:
                        module.setdefault("other_sysutil_ffi_declarations", []).append(declaration)
                        continue
                    export = exports.get(name)
                    binding = {"name": name, "original": src["original"], "declaration": declaration,
                               "source_callables": [body["name"] for body in src["callables"] if body["name"] == name],
                               "export": export}
                    module["ffi_exports"].append(binding)
                    if export is None:
                        module["failures"].append({"kind": "ffi_export_missing", "name": name})
                    elif int(export["address"], 16) not in functions:
                        module["failures"].append({"kind": "ffi_export_body_missing", "name": name})
                    else:
                        selected.add(int(export["address"], 16))
            if label == "mapping-engine":
                if module["original"]["sha256"] != "6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a":
                    raise ValueError("Rotation semantics require a new native-source audit")
                rotation_functions = []
                for rva in (0x39550, 0x39DB0, 0x39200, 0x39AB0, 0xC520, 0xC7E0,
                            0xC6F0, 0xC9D0, 0x5A9D0, 0x5AB00, 0x71520, 0x71C30,
                            0x71720, 0x71E20, 0x10F73E, 0x10F8C8, 0x1188F0,
                            0x118AF0, 0x113CA0, 0x114130, 0x114070, 0x118A30,
                            0x118C40, 0x7CFA0, 0x7D270, 0x110700, 0x110758,
                            0x110B5A, 0x110D34, 0x114850, 0x110C04, 0x1158C6):
                    address = int(module["image_base"], 16) + rva
                    body, ref = function(address)
                    if not ref["full_assembly"] or not ref["full_pseudocode"]:
                        raise ValueError("Rotation function capture is incomplete: " + hex(rva))
                    selected.add(address)
                    rotation_functions.append(ref)
                host = module["sources"][0]
                names = {"getMouseSensorRotationAngle", "setMouseSensorRotationAngle"}
                callers = [row for row in host["callables"] if row["name"] in names]
                if {row["name"] for row in callers} != names:
                    raise ValueError("Rotation host callable ownership changed")
                module["rotation_native"] = {
                    "host": host["original"], "host_callables": callers,
                    "native_functions": rotation_functions,
                    "verified_semantics": [
                        "Host requires initialized engine and an existing deviceMap identity; native manager5A9D0/5AB00 requires an existing registered InputDeviceInterface. Neither operation implicitly registers a source mapping device.",
                        "Exports39550/39DB0 queue through engineC520/C7E0, manager5A9D0/5AB00, device71520/71C30 and its device thread71720/71E20 to the retained filter-driver owner.",
                        "GET113CA0 submits DeviceIoControl0x88883184 synchronously with null OVERLAPPED and a four-byte output. The callback chain returns the source result, reason, identity and signed angle.",
                        "SET114130 submits DeviceIoControl0x88883180 with four-byte input and retained OVERLAPPED; immediate success or ERROR_IO_PENDING is followed by actual completion114850/110B5A. SET has no implicit GET or angle verification request.",
                        "Cleanup110700 cancels driver I/O and closes its handle;110758 defers cleanup completion until pending count reaches zero. The captured cleanup path does not issue a rotation reset.",
                    ],
                    "implementation_scope": "Typed Rotation protocol, shared physical driver lease, per-operation ownership, Windows synchronous GET, asynchronous SET and service Start/Request/Poll/Stop are implemented. The retained public worker and Shell own registration, request identities, cancellation, scope revocation and cleanup. Existing mouse pages dispatch typed edits and explicit profile restores through source-backed product/observed-firmware routing, durable local-save-before-SET and separate draft/persisted/acknowledged state. Native targets retain the actual discovered physical identity; direct requests use the shared HID worker cleanup. The UI does not add an implicit GET, default initialization write or SET readback. The frontend query records the product branches separately.",
                    "remaining_local_chains": [
                        "Real-device GET/SET, receiver-specific physical routing, close/reopen state, cancellation and vendor-DLL comparison remain unverified; static capture and simulated tests cannot establish runtime acceptance.",
                        "Real-window interaction, profile switching and shutdown acceptance remain unverified. Rotation implementation does not establish completion of the remaining mapping-engine product/version branches or other native features.",
                    ],
                    "implementation_adaptations": [
                        "Rust rejects GET buffers shorter than four actual returned bytes; the native source does not validate returned length before consuming its integer.",
                        "Channel failure retires an unknown in-flight operation as failed only after the shared driver owner has completed cleanup; it cannot report success or keep polling a dead reply channel forever.",
                    ],
                    "validation": ".analysis/validation.json retains executed Rust suite outcomes independently of source evidence; real-device and real-window acceptance remain unverified.",
                }
                module["gamepad_native"] = mapping_gamepad_observer_evidence(module, native_path, function, selected)
            if label == "hid-feature":
                module["feature_report_callers"] = feature_report_callers(parser, originals)
                if not module["feature_report_callers"]["source_owners_complete"]:
                    module["failures"].append({"kind": "feature_report_caller_evidence_incomplete",
                        "products": [row["product_id"] for row in module["feature_report_callers"]["products"]
                                     if not row["source_owners_complete"]]})
                strings = dataset("strings")
                names = {"getFeatureReport", "getFeatureReportSync", "sendFeatureReport", "sendFeatureReportSync", "close", "HID"}
                module["napi_registration_strings"] = [row for row in strings if row["text"] in names]
                for row in module["napi_registration_strings"]:
                    for xref in row["xrefs"]:
                        address = owner(int(xref["from"], 16))
                        if address is not None:
                            selected.add(address)
                for name in ("hid_enumerate", "hid_free_enumeration", "hid_open_path", "hid_send_feature_report",
                             "hid_get_feature_report", "hid_read_timeout", "hid_close"):
                    export = exports.get(name)
                    if export is None:
                        raise ValueError("Required original HID export missing: " + name)
                    address = int(export["address"], 16)
                    selected.add(address)
                    for xref in function(address)[0]["xrefs_to_entry"]:
                        caller = owner(int(xref["from"], 16))
                        if xref["code"] and caller is not None:
                            selected.add(caller)
                dynamic_bindings, indirect_sites = hid_indirect_evidence(module, native_path, capture_dir,
                    manifest, exports, strings, functions, function, identity, selected)
                starts[:] = sorted(functions)
                host_body = (source_root / sources[0]).read_text(encoding="utf-8")
                host_anchors = ["dataSend.length", "reportLength", "reportId", "claimInterface", "noSlice", ".slice(1)"]
                if not all(anchor in host_body for anchor in host_anchors):
                    raise ValueError("Original host feature framing requires a new semantic audit")
                semantic_functions = {}
                for name, anchors in {
                    "hid_send_feature_report": ["HidD_SetFeature", "return a3;", "0xFFFFFFFF"],
                    "hid_get_feature_report": ["DeviceIoControl", "0xB0192", "997", "GetOverlappedResult", "BytesReturned + 1"],
                    "hid_close": ["CancelIo", "CloseHandle", "LocalFree"],
                }.items():
                    body, ref = function(int(exports[name]["address"], 16))
                    text = "\n".join(line["text"] for line in (body.get("pseudocode") or {}).get("lines", []))
                    if not all(anchor in text for anchor in anchors):
                        raise ValueError("Native feature behavior requires a new semantic audit: " + name)
                    semantic_functions[name] = {"function": ref, "observed_pseudocode_anchors": anchors}
                module["semantic_evidence"] = {"host": module["sources"][0]["original"],
                    "host_anchors": host_anchors, "native": semantic_functions}
                module["verified_semantics"] = [
                    "Original host prepends reportId ?? claimInterface only when dataSend has 90 bytes and reportLength is 91 or undefined; other lengths pass through unchanged.",
                    "Original host GET chooses reportId ? reportId : claimInterface; explicit SET zero and GET interface selectors therefore differ. Returned prefixes are discarded, not correlated with either selector.",
                    "Original feature getter defaults reportLength to 91 and strips the first byte unless noSlice is true; transport failures do not synthesize returned device data.",
                    "Native hid_send_feature_report calls HidD_SetFeature with the exact requested length and returns that length only on success; failure returns -1.",
                    "Native hid_get_feature_report uses DeviceIoControl 0xB0192, accepts ERROR_IO_PENDING 997, waits through GetOverlappedResult and returns BytesReturned + 1.",
                    "Native hid_close calls CancelIo, closes the two event handles and the device handle, and frees stored error/read-buffer/device allocations.",
                ]
                module["remaining_semantic_work"] = [
                    "The dynamic hid.dll exports, executeJob batch lambda and shared-worker destructor dispatch are source/byte-resolved. Other N-API promise registrations, callback ownership and indirect dispatch remain outside this focused resolution.",
                    "Rust transport/platform owners are linked by actual current bytes; full per-product caller, protocol, UI and device acceptance must be established separately.",
                ]
            if label == "notification-restart":
                frontend_root = ROOT / ".analysis/inputs/frontend/notification-manager-current"
                frontend_receipt = frontend_root / "acquisition.json"
                frontend_acquisition = json.loads(frontend_receipt.read_bytes())
                module["notification_acquisition"] = original(frontend_receipt)
                module["frontend_acquisition_unresolved"] = frontend_acquisition.get("unresolved", [])
                module["frontend_stability"] = frontend_acquisition.get("stability", [])
                raw_root = frontend_root / "raw/apps.razer.com/notification-manager"
                source_patterns = ["Root.*.chunk.js", "SystemRestartRequiredWindow.*.chunk.js", "main.*.js", "959.*.chunk.js", "971.*.chunk.js"]
                module["ui_sources"] = []
                for pattern in source_patterns:
                    paths = list((raw_root / "static/js").glob(pattern))
                    if len(paths) != 1:
                        raise ValueError("Notification source owner is ambiguous or missing: " + pattern)
                    module["ui_sources"].append(source(paths[0]))
                module["host_dispatch_sources"] = [source(source_root / name) for name in (
                    "electron/main.js", "electron/preload.js", "electron/modules/window_storage/index.js",
                    "electron/modules/ffi/ffiMain.js", "electron/modules/ffi/FFIPreloadMain.js")]
                module["styles"] = [original(path) for path in sorted((raw_root / "static/css").glob("*.css"))]
                dashboard_root = ROOT / ".analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js"
                dashboard_paths = [path for path in dashboard_root.glob("*.js")
                                   if b"notification_manager_unique_noti_restart" in path.read_bytes()]
                module["dashboard_callers"] = [source(path) for path in dashboard_paths]
                if not dashboard_paths:
                    raise ValueError("Current Dashboard restart caller is missing")
                address = int(exports["restartWorkstation"]["address"], 16)
                body, ref = function(address)
                text = "\n".join(line["text"] for line in (body.get("pseudocode") or {}).get("lines", []))
                anchors = ["WinExec", '"shutdown -r -t 0"', ", 0)"]
                if not all(anchor in text for anchor in anchors):
                    raise ValueError("Native restart behavior needs a new semantic audit")
                module["semantic_evidence"] = {"restartWorkstation": {"function": ref, "observed_pseudocode_anchors": anchors}}
                module["verified_semantics"] = [
                    "Dashboard writes string true to notification_manager_unique_noti_restart through WindowStorage; it does not directly restart the computer.",
                    "Current host WindowStorage keeps URL-owned values in memory and broadcasts created/changed events to eligible other webContents; registration and unload unregistration remain source-owned.",
                    "Notification Root consumes notification_manager_unique_noti_ created/changed events, parses newValue, and deduplicates truthy restart callers by srcURL. Even repeated true for the same srcURL returns a new caller array, so the effect reruns and can reopen a manually closed window. A false request clears the entire restart caller list.",
                    "The restart effect queries existing WindowService clients, opens at most one notification-restart window for nonempty callers without focusing an existing window, and closes a nonhibernated existing window when callers become empty.",
                    "SystemRestartRequiredWindow renders the source title, description, LATER button, RESTART NOW button and X close control. The disabled CSS class on LATER is visual; its click handler is enabled.",
                    "LATER and X animate opacity for 200ms and call window.close after 250ms. ResizeObserver disconnects and the language storage listener is removed on unmount.",
                    "RESTART NOW calls the Electron restartWorkstation action. Host dispatch reaches FFISysUtils.callDLLAsync then callDLLMainAsync for sysUtilsNative; FFI declares void with no arguments.",
                    "Native restartWorkstation at RVA 0x70370 tail-calls WinExec with shutdown -r -t 0 and SW_HIDE 0. The original void FFI discards WinExec's result and has no extra source confirmation or success receipt.",
                ]
                module["remaining_semantic_work"] = [
                    "Other notification variants, native-toast branches and remaining SysUtils exports require their own complete source/ABI connection; this query establishes the restart notification path only.",
                    "The source provides no device-write result for restart. Actual window placement, focus, OS restart, vendor service and device acceptance remain unverified under execution limits.",
                ]
            if label == "startup-settings":
                frontend_root = ROOT / ".analysis/inputs/frontend/synapse-settings-current"
                frontend_receipt = frontend_root / "acquisition.json"
                acquired_frontend = json.loads(frontend_receipt.read_bytes())
                module["frontend_acquisition"] = original(frontend_receipt)
                module["frontend_acquisition_unresolved"] = acquired_frontend.get("unresolved", [])
                module["frontend_stability"] = acquired_frontend.get("stability", [])
                if not module["frontend_stability"] or not all(row.get("unchanged") for row in module["frontend_stability"]):
                    raise ValueError("Current Settings deployment stability is not verified")
                raw_root = frontend_root / "raw/apps.razer.com/synapse/settings"
                module["ui_sources"] = []
                for pattern in ("720.*.chunk.js", "714.*.chunk.js", "main.*.js"):
                    paths = list((raw_root / "static/js").glob(pattern))
                    if len(paths) != 1:
                        raise ValueError("Settings source owner is ambiguous or missing: " + pattern)
                    module["ui_sources"].append(source(paths[0]))
                module["styles"] = [original(path) for path in sorted((raw_root / "static/css").glob("*.css"))]
                module["host_dispatch_sources"] = [source(source_root / name) for name in (
                    "electron/main.js", "electron/preload.js", "electron/lib/parseCmdParams.js",
                    "electron/components/Tab/Tab.js", "electron/modules/ffi/ffiMain.js",
                    "electron/modules/ffi/FFIPreloadMain.js")]
                semantic_functions = {}
                for name, anchors in {
                    "applicationAutoStart": ["RegOpenKeyW", "HKEY_CURRENT_USER", "CurrentVersion", "RegQueryValueExW", "RazerAppEngine", "RegCloseKey"],
                    "setApplicationAutoStart": ["RegOpenKeyW", "GetModuleFileNameW", "--url-params=%s --autoStart=1", "RegSetValueExW", "RegDeleteValueW", "RegCloseKey"],
                    "getApplicationAutoStartApps": ["RegQueryValueExW", "--url-params=", "!= 32", "WideCharToMultiByte", "RegCloseKey"],
                }.items():
                    body, ref = function(int(exports[name]["address"], 16))
                    text = "\n".join(line["text"] for line in (body.get("pseudocode") or {}).get("lines", []))
                    if not all(anchor in text for anchor in anchors):
                        raise ValueError("Native startup behavior needs a new semantic audit: " + name)
                    semantic_functions[name] = {"function": ref, "observed_pseudocode_anchors": anchors}
                module["semantic_evidence"] = semantic_functions
                module["verified_semantics"] = [
                    "Settings Ks starts both checkboxes true, queries getAppAutoStart(synapse) and getMinimizedOnStartUp, and disables the minimized child when autostart is false. Clicks update their editor optimistically; setter acknowledgements do not trigger rollback or a getter refresh.",
                    "RazerApp setters persist isAutoStart/isMinimizedOnStartUp in local app data, then updateAppsAutoStart filters enabled local apps. No enabled apps disables OS registration; otherwise the parameter is apps=<names>&autoStart=1 with a launch-force-hidden list for minimized apps.",
                    "The lower SDK removes &autoStart=1 before sending setApplicationAutoStart; native RVA 0x6e3c0 quotes the executable and appends --url-params=<parameters> --autoStart=1. It sets/deletes the current-user Run REG_SZ named RazerAppEngine.",
                    "Native applicationAutoStart RVA 0x6e2d0 tests value existence; getApplicationAutoStartApps RVA 0x6e950 reads it, returns the --url-params token up to the next space, and the SDK uses String.includes(synapse). Returned pointer ownership is retained in the complete FFI dispatch/FreeMalloc bodies.",
                    "Host xn reads launch-force-hidden by comma-separated app name. autoStart alone marks the tab launch; it does not imply hidden. isLaunchBackgroundOnStart controls the initial background window lifecycle.",
                ]
                module["implementation_adaptations"] = [
                    "The standalone program registers its own current executable under Run/razer_ui and does not alter the installed vendor's Run/RazerAppEngine. Only the local synapse app is registered; no account or remote application host parameters are introduced.",
                    "The local workspace persists minimized/editor intent; actual Run-value reads populate the source editor separately. OS writes run serially, retain the latest pending click, propagate real failures without source-state rollback, and flush a pending write on application quit.",
                    "The main window reads the source launch-force-hidden flag before creation. If tray registration fails, the existing shell fallback reveals the main window so it remains reachable.",
                ]
                module["remaining_semantic_work"] = [
                    "This query covers startup settings only. Other Settings local controls, host launch variants and complete page/component acceptance require separate source-to-runtime checks.",
                    "Source-map/chunk acquisition failures remain explicit. Real Run-key writes, OS logon launch, tray/window visibility and runtime acceptance remain unverified under execution limits.",
                ]
            import_map = {int(row["address"], 16): row for row in imports}
            module["resolved_indirect_calls"] = []
            for address in sorted(selected):
                body, ref = function(address)
                module["native_functions"].append(ref)
                for instruction in body["assembly"] or []:
                    outgoing = [edge for edge in instruction["xrefs_from"]
                                if edge["type"] in {16, 17, 18, 19} or int(edge["to"], 16) in import_map]
                    for edge in outgoing:
                        target = int(edge["to"], 16)
                        module["native_edges"].append({"caller": ref["rva"], **edge,
                            "target_function": native_chain_function_reference(function(target)[1]) if target in functions else None,
                            "target_import": {key: value for key, value in import_map[target].items() if key != "xrefs"}
                                             if target in import_map else None})
                    control = instruction["text"].lstrip().lower()
                    is_indirect = (control.startswith("call ") or
                        re.match(r"jmp\s+(?:qword ptr \[|[re][abcds][xip]|r(?:[89]|1[0-5])\b)", control))
                    if is_indirect and not outgoing:
                        call = {"owner": ref["rva"],
                            "address": instruction["address"], "rva": instruction["rva"],
                            "instruction": instruction["text"], "bytes": instruction["bytes"]}
                        dynamic = [dynamic_bindings[int(edge["to"], 16)] for edge in instruction["xrefs_from"]
                                   if int(edge["to"], 16) in dynamic_bindings]
                        resolution = indirect_sites.get(instruction["rva"])
                        if resolution:
                            module["resolved_indirect_calls"].append({**call, **resolution})
                        elif len(dynamic) == 1:
                            module["resolved_indirect_calls"].append({**call,
                                "resolution": "GetProcAddress_result_stored_in_exact_global", "target_dynamic_import": dynamic[0]})
                        else:
                            module["unresolved_indirect_calls"].append(call)
            module["rust_owners"] = [identity(ROOT / name) for name in rust]
            module["all_linked_function_bodies_verified"] = all(x["full_assembly"] for x in module["native_functions"])
        except Exception as exc:
            module["failures"].append({"kind": "native_chain_query_failure", "error": str(exc).replace(str(ROOT), ".")})
    return save_native_chain_report(ROOT / ".analysis/results/native/chain-audit.json", result)


def save_native_chain_report(destination, result):
    """Keep revalidated evidence and persist failed queries before returning failure."""
    with index_lock(destination.with_suffix(".lock")):
        previous = json.loads(destination.read_bytes()) if destination.exists() else {"modules": []}
        result["modules"] = merge_native_chain_modules(previous["modules"], result["modules"])
        result["finished_at_utc"] = stamp()
        result["evidence_query_complete"] = not result["failures"] and all(not m["failures"] for m in result["modules"])
        save(destination, result)
    print(json.dumps({"receipt": rel(destination), "evidence_query_complete": result["evidence_query_complete"],
        "modules": [{"name": m["name"], "sources": len(m["sources"]), "ffi_exports": len(m["ffi_exports"]),
                     "native_functions": len(m["native_functions"]), "native_edges": len(m["native_edges"]),
                     "resolved_indirect_calls": len(m.get("resolved_indirect_calls", [])),
                     "unresolved_indirect_calls": len(m["unresolved_indirect_calls"]), "failures": m["failures"]}
                    for m in result["modules"]]}), flush=True)
    if not result["evidence_query_complete"]:
        raise SystemExit("Native-chain evidence query is incomplete; the failure report was saved to " + rel(destination))
    return result


def audit_reference(args):
    """Verify a complete external Git snapshot without executing project code."""
    import tomllib
    directory = (ROOT / args.directory).resolve()
    if not directory.is_relative_to((ROOT / ".analysis/references").resolve()) or directory.is_symlink():
        raise ValueError("Architecture references must remain under .analysis/references/")
    retained_snapshot = getattr(args, "retained_snapshot", False)
    if not retained_snapshot:
        top = subprocess.run(["git", "-C", str(directory), "rev-parse", "--show-toplevel"], check=True,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.decode().strip()
        if Path(top).resolve() != directory:
            raise ValueError("Reference Git metadata is absent; refusing to audit the enclosing workspace repository")
    def git(*arguments):
        return subprocess.run(["git", "-C", str(directory), *arguments], check=True,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
    acquisition_path = directory / "acquisition.json"
    previous = json.loads(acquisition_path.read_bytes())
    commit = previous.get("commit") if retained_snapshot else git("rev-parse", "HEAD").decode().strip()
    if commit != args.commit or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Reference checkout differs from its pinned commit")
    if not retained_snapshot and git("diff", "--exit-code", "--no-ext-diff", "HEAD", "--").strip():
        raise ValueError("Reference tracked files have been modified")
    if retained_snapshot and (args.check_upstream or args.restore_original_bytes):
        raise ValueError("Retained snapshot verification cannot claim an upstream check or restore unretained Git objects")
    latest = None
    if args.check_upstream:
        repository = re.fullmatch(r"https://github\.com/([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+?)(?:\.git)?", previous["url"])
        if repository is None:
            raise ValueError("Upstream audit requires an explicit public GitHub repository")
        request = urllib.request.Request("https://api.github.com/repos/" + repository[1] + "/commits/HEAD",
                                         headers={"User-Agent": "local-static-reference-audit"})
        with urllib.request.urlopen(request, timeout=30) as response:
            remote = json.load(response)
        latest = {"checked_at_utc": stamp(), "commit": remote["sha"],
                  "committed_at_utc": remote["commit"]["committer"]["date"]}
        if latest["commit"] != commit:
            raise ValueError("Reference is no longer the current upstream commit; reacquire before comparison")
    files, restored = [], []
    if retained_snapshot:
        if previous.get("method") != "complete_git_tree_blob_identity_and_workspace_manifest_audit":
            raise ValueError("Retained reference lacks a complete original Git tree audit")
        recorded = previous.get("files", [])
        if (len(recorded) != previous.get("summary", {}).get("files") or
                sum(row["bytes"] for row in recorded) != previous.get("summary", {}).get("bytes")):
            raise ValueError("Retained reference tree summary differs")
        entries = []
        for row in recorded:
            path = resolve(row["path"], rel(directory))
            name = path.relative_to(directory).as_posix()
            entries.append(f"{row['git_mode']} blob {row['git_blob']} {row['bytes']}\t{name}".encode())
        actual_paths = {rel(path) for path in directory.rglob("*") if path.is_file()
                        and path != acquisition_path and ".git" not in path.relative_to(directory).parts}
        if actual_paths != {row["path"] for row in recorded}:
            raise ValueError("Retained reference file closure differs from the complete original tree audit")
        verify_source_rows(recorded)
    else:
        entries = git("ls-tree", "-r", "-l", "-z", commit).split(b"\0")
    for entry in entries:
        if not entry:
            continue
        metadata, spelling = entry.split(b"\t", 1)
        mode, kind, blob, length = metadata.split()
        name = spelling.decode("utf-8")
        path = directory / name
        if kind != b"blob" or mode not in {b"100644", b"100755", b"120000"} or not path.absolute().is_relative_to(directory):
            raise ValueError("Unsupported reference tree entry: " + name)
        if any(parent.is_symlink() for parent in path.parents if parent.is_relative_to(directory)):
            raise ValueError("Reference traverses a link: " + name)
        raw = os.readlink(path).encode("utf-8") if path.is_symlink() and mode == b"120000" else path.read_bytes()
        actual_blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
        if len(raw) != int(length) or actual_blob != blob.decode():
            if not args.restore_original_bytes:
                raise ValueError("Reference bytes differ from committed Git object: " + name)
            original = git("cat-file", "blob", blob.decode())
            if hashlib.sha1(b"blob " + str(len(original)).encode() + b"\0" + original).hexdigest() != blob.decode():
                raise ValueError("Reference Git object is corrupt: " + name)
            restored.append({"path": rel(path), "checkout_sha256": hashlib.sha256(raw).hexdigest(),
                             "original_sha256": hashlib.sha256(original).hexdigest()})
            path.write_bytes(original)
            raw, actual_blob = original, blob.decode()
        files.append({"path": path.relative_to(ROOT).as_posix(), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), "git_mode": mode.decode(),
                      "git_blob": actual_blob, "kind": "external_architecture_reference"})
    workspace = tomllib.loads((directory / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]
    manifests = []
    for member in workspace["members"]:
        for owner in sorted(directory.glob(member)):
            manifest = owner / "Cargo.toml"
            value = tomllib.loads(manifest.read_text(encoding="utf-8"))
            dependencies = {**value.get("dependencies", {})}
            for target in value.get("target", {}).values():
                dependencies.update(target.get("dependencies", {}))
            resolved = {name: workspace.get("dependencies", {}).get(name, item) if isinstance(item, dict) and item.get("workspace") else item
                        for name, item in dependencies.items()}
            manifests.append({"name": value["package"]["name"], "manifest": rel(manifest),
                              "internal_dependencies": sorted(name for name, item in resolved.items()
                                                              if isinstance(item, dict) and "path" in item)})
    if retained_snapshot and manifests != previous.get("workspace_members"):
        raise ValueError("Retained reference workspace manifests differ from the complete original audit")
    result = {**previous, "schema_version": 1, "url": previous["url"], "commit": commit,
              "audited_at_utc": previous.get("audited_at_utc") if retained_snapshot else stamp(),
              "method": "complete_git_tree_blob_identity_and_workspace_manifest_audit",
              "git_version": previous.get("git_version") if retained_snapshot else subprocess.run(["git", "--version"], capture_output=True, text=True, check=True).stdout.strip(),
              "upstream": latest or previous.get("upstream"), "files": files, "workspace_members": manifests,
              "checkout_filter_restorations": restored or previous.get("checkout_filter_restorations", []),
              "summary": {"files": len(files), "bytes": sum(row["bytes"] for row in files), "members": len(manifests)},
              "target_execution": False,
              "scope": "Complete acquisition/byte and dependency inventory; selected architecture call chains read separately. No device-protocol evidence or runtime acceptance is implied."}
    if retained_snapshot:
        result["retained_snapshot_verification"] = {"verified_at_utc": stamp(),
            "method": "complete_retained_tree_sha256_git_blob_identity_closure_and_manifest_verification",
            "upstream_rechecked": False, "git_objects_reacquired": False,
            "scope": "Revalidate the previously audited fixed snapshot; original upstream audit time and currentness scope remain unchanged"}
    save(acquisition_path, result)
    index_files(files + [{"path": rel(acquisition_path), "bytes": acquisition_path.stat().st_size,
                          "sha256": sha(acquisition_path), "kind": "acquisition_receipt"}],
                publication={"owner": "tools/analysis.py", "receipt": rel(acquisition_path),
                    "reason": "Publish the complete pinned Git tree and workspace audit, including original blob bytes after checkout-filter restoration",
                    "verification": {"method": result["method"], "commit": commit, **result["summary"],
                        "upstream_rechecked": args.check_upstream,
                        "checkout_filter_restorations": result["checkout_filter_restorations"]}})
    print(json.dumps({"receipt": rel(acquisition_path), "commit": commit, **result["summary"], "passed": True}), flush=True)
    return result


def publish_instruction_manual(receipt_path):
    """Verify retained architecture PDF identities without executing a target."""
    receipt_path = resolve(rel(receipt_path), ".analysis/inputs/platform-abi-current")
    raw = receipt_path.read_bytes()
    receipt = json.loads(raw)
    if (receipt.get("schema_version") != 1 or receipt.get("target_execution") is not False
            or not isinstance(receipt.get("files"), list) or not receipt["files"]):
        raise ValueError("Instruction manual requires a complete static acquisition receipt")
    rows, identities = [], set()
    for member in receipt["files"]:
        source = resolve(member["path"], ".analysis/inputs/platform-abi-current")
        if source.parent != receipt_path.parent or source.suffix.lower() != ".pdf":
            raise ValueError("Instruction manual is outside its acquisition directory")
        for key in ("url", "final_url"):
            url = urllib.parse.urlparse(member.get(key, ""))
            if (url.scheme != "https" or url.hostname != "www.intel.com" or url.port not in (None, 443)
                    or url.username is not None or url.password is not None or url.query or url.fragment
                    or not url.path.startswith("/content/dam/www/public/us/en/documents/manuals/")
                    or not url.path.endswith(".pdf")):
                raise ValueError("Instruction manual has no supported official HTTPS origin")
        if (member.get("http_status") != 200 or member.get("content_type") != "application/pdf"
                or any(not isinstance(member.get(key), str) or not member[key].strip()
                       for key in ("document_number", "revision", "publication_date"))
                or any(type(member.get(key)) is not int or member[key] <= 0 for key in ("volume", "pdf_pages"))
                or member["path"] in identities):
            raise ValueError("Instruction manual has incomplete or repeated document metadata")
        identities.add(member["path"])
        with source.open("rb") as stream:
            if stream.read(5) != b"%PDF-":
                raise ValueError("Instruction manual bytes are not a PDF")
        rows.append({key: member[key] for key in ("path", "bytes", "sha256", "url", "final_url",
                                                 "document_number", "revision", "publication_date", "volume")}
                    | {"kind": "official_platform_instruction_manual"})
    receipt_row = {"path": rel(receipt_path), "bytes": len(raw),
                   "sha256": hashlib.sha256(raw).hexdigest(), "kind": "acquisition_receipt"}
    index_files([*rows, receipt_row], publication={"owner": "tools/analysis.py", "receipt": receipt_row["path"],
        "reason": "Reconcile retained official instruction manual acquisition with its complete PDF byte identities",
        "verification": {"method": "official_https_receipt_pdf_header_complete_bytes_sha256_and_document_revision",
                         "documents": len(rows), "network_requests": False,
                         "scope": "Architecture instruction encoding only; no product protocol or target execution claim"}})
    return {"receipt": receipt_row["path"], "documents": len(rows)}


def publish_platform_contract(receipt_path):
    """Verify pinned official OS contracts without importing product semantics."""
    receipt_path = resolve(rel(receipt_path), ".analysis/inputs/platform-abi-current")
    raw = receipt_path.read_bytes()
    receipt = json.loads(raw)
    version = receipt.get("version", "")
    if (receipt.get("schema_version") != 1 or receipt.get("target_execution") is not False or
            not isinstance(receipt.get("scope"), str) or not receipt["scope"].strip() or
            not isinstance(version, str) or not re.fullmatch(r"KMDF [0-9]+\.[0-9]+", version) or
            not isinstance(receipt.get("repositories"), list) or not receipt["repositories"] or
            not isinstance(receipt.get("files"), list) or not receipt["files"]):
        raise ValueError("Platform contract requires a complete static acquisition and ABI version")
    allowed = {"https://github.com/microsoft/Windows-Driver-Frameworks",
               "https://github.com/MicrosoftDocs/windows-driver-docs-ddi"}
    repositories = {}
    for row in receipt["repositories"]:
        name = row.get("repository")
        if (name not in allowed or name in repositories or
                not re.fullmatch(r"[a-f0-9]{40}", row.get("commit", "")) or
                not isinstance(row.get("license_paths"), list) or not row["license_paths"]):
            raise ValueError("Platform contract repository is not a unique pinned official source")
        repositories[name] = row
    rows, names, retained_members, declarations = [], set(), set(), 0
    members_by_path = {}
    for member in receipt["files"]:
        source = resolve(member["path"], ".analysis/inputs/platform-abi-current")
        repository = repositories.get(member.get("repository"))
        repo_path = member.get("repository_path", "")
        parts = repo_path.split("/") if isinstance(repo_path, str) else []
        if (repository is None or member.get("commit") != repository["commit"] or
                not parts or any(part in {"", ".", ".."} or "\\" in part or ":" in part for part in parts) or
                not source.is_relative_to(receipt_path.parent) or source == receipt_path or
                member["path"] in names or member.get("http_status") != 200 or
                not isinstance(member.get("version"), str) or not member["version"].strip()):
            raise ValueError("Platform contract member has invalid ownership, version or HTTP provenance")
        if member.get("license_paths", repository["license_paths"]) != repository["license_paths"]:
            raise ValueError("Platform contract member license ownership differs from its repository")
        repo_suffix = member["repository"].removeprefix("https://github.com/")
        expected_url = "https://raw.githubusercontent.com/" + repo_suffix + "/" + repository["commit"] + "/" + repo_path
        if member.get("url") != expected_url or member.get("final_url") != expected_url:
            raise ValueError("Platform contract must retain its exact commit-pinned official raw URL")
        if (member["repository"].endswith("Windows-Driver-Frameworks") and source.suffix.lower() == ".h" and
                "/kmdf/" + version.removeprefix("KMDF ") + "/" not in "/" + repo_path):
            raise ValueError("Platform contract header does not belong to the declared KMDF ABI")
        content = source.read_bytes()
        if (type(member.get("bytes")) is not int or len(content) != member["bytes"] or not content or
                hashlib.sha256(content).hexdigest() != member.get("sha256")):
            raise ValueError("Platform contract source differs from its complete original bytes")
        lines = content.decode("utf-8-sig").splitlines()
        raw_lines = content.splitlines(keepends=True)
        claims = member.get("declarations", [])
        if not isinstance(claims, list):
            raise ValueError("Platform contract declarations must preserve source line ranges")
        for claim in claims:
            first, last = claim.get("line_start"), claim.get("line_end")
            if (type(first) is not int or type(last) is not int or not 1 <= first <= last <= len(lines) or
                    not isinstance(claim.get("symbol"), str) or not claim["symbol"].strip() or
                    claim["symbol"] not in "\n".join(lines[first - 1:last]) or
                    not isinstance(claim.get("claim"), str) or not claim["claim"].strip()):
                raise ValueError("Platform contract declaration is not bound to its actual source lines")
            if ("sha256" in claim and hashlib.sha256(b"".join(raw_lines[first - 1:last])).hexdigest()
                    != claim["sha256"]):
                raise ValueError("Platform contract declaration byte identity changed")
            declarations += 1
        names.add(member["path"])
        member_key = (member["repository"], repo_path)
        if member_key in retained_members:
            raise ValueError("Platform contract duplicates a retained repository member")
        retained_members.add(member_key)
        members_by_path[member["path"]] = member
        rows.append({key: member[key] for key in ("path", "bytes", "sha256", "url", "final_url",
                                                  "repository", "commit", "repository_path", "version")}
                    | {"kind": "official_platform_contract_source"})
    for name, repository in repositories.items():
        for license_path in repository["license_paths"]:
            license_member = members_by_path.get(license_path, {})
            license_name = Path(license_member.get("repository_path", "")).name
            if (license_member.get("repository") != name or
                    not re.fullmatch(r"LICENSE(?:[-.][A-Za-z0-9_.-]+)?", license_name)):
                raise ValueError("Platform contract omits a declared source license")
    if not declarations:
        raise ValueError("Platform contract contains no source-bound ABI declarations")
    receipt_row = {"path": rel(receipt_path), "bytes": len(raw),
                   "sha256": hashlib.sha256(raw).hexdigest(), "kind": "acquisition_receipt"}
    index_files([*rows, receipt_row], publication={"owner": "tools/analysis.py", "receipt": receipt_row["path"],
        "reason": "Reconcile pinned official OS ABI and lifecycle contract sources with complete byte identities",
        "verification": {"method": "official_repository_commit_raw_url_bytes_sha256_license_and_declaration_lines",
            "files": len(rows), "declarations": declarations, "version": version, "network_requests": False,
            "scope": "Platform ABI and lifecycle only; not Razer protocol evidence or runtime acceptance"}})
    return {"receipt": receipt_row["path"], "files": len(rows), "declarations": declarations}


def reconcile_source_index(args):
    """Reconcile only inputs validated by their actual acquisition/publication owners."""
    import families
    import asset_release
    instruction_manuals = getattr(args, "instruction_manual", None) or []
    platform_contracts = getattr(args, "platform_contract", None) or []
    if not args.frontend and not args.publication and not instruction_manuals and not platform_contracts:
        raise ValueError("Source reconciliation requires explicit acquisition or publication receipts")
    for name in args.frontend or []:
        families.publish_frontend_acquisition(resolve(name, ".analysis/inputs/frontend"),
            reason="Reconcile retained supplemental frontend acquisition after complete byte, deployment-root, manifest and stability verification")
    for name in args.publication or []:
        asset_release.index_addition_sources(name,
            reason="Reconcile targeted resource publication after source-receipt, pinned resource and alias verification")
    for name in instruction_manuals:
        publish_instruction_manual(resolve(name, ".analysis/inputs/platform-abi-current"))
    for name in platform_contracts:
        publish_platform_contract(resolve(name, ".analysis/inputs/platform-abi-current"))
    print(json.dumps({"frontend_receipts": args.frontend or [], "publication_manifests": args.publication or [],
                      "instruction_manuals": instruction_manuals, "platform_contracts": platform_contracts,
                      "network_requests": False, "target_execution": False, "passed": True}), flush=True)


def add_commands(parsers):
    import host
    host.add_commands(parsers)
    pending = parsers.add_parser("reverse-pending", help="Run retained pending native and frontend static work in one bounded resumable queue")
    pending.add_argument("--workers", type=int, default=min(16, os.cpu_count() or 2))
    pending.add_argument("--process-workers", action="store_true", help="Run independent static tasks in bounded spawned processes to use multiple CPU cores")
    pending.add_argument("--ida-root", default=os.environ.get("RAZER_IDA_ROOT", "E:/IDA"))
    pending.add_argument("--resume", action="store_true", help="Continue the exact unfinished queue, preserving completed tasks and all interrupted evidence")
    pending.add_argument("--resume-reason", help="Record the explicit reason for resuming an interrupted queue after reviewed maintenance changes; requires --resume")
    pending.add_argument("--verify-only", action="store_true", help="Strictly recheck a finished static queue without starting IDA or network repair")
    pending.add_argument("--native-only", action="store_true", help="Review remaining native failures without repeating unchanged analyzer attempts or frontend work")
    pending.add_argument("--native-review", action="append", help="Explicit assembly-reviewed plan in a retained receipt: project-relative.json#JSON-pointer; repeat for independent modules")
    pending.add_argument("--recheck-unresolved", action="store_true", help="Strictly recheck only frozen unresolved tasks into a separate immutable increment, preserving the completed baseline")
    pending.add_argument("--task-id", action="append", help="Select an exact frozen unresolved task for incremental recheck; repeat as needed")
    adjudication = parsers.add_parser("adjudicate-native", help="Publish an explicit SHA-bound review of every complete native function alternative without altering raw evidence")
    adjudication.add_argument("--input", required=True)
    adjudication.add_argument("--review", required=True)
    adjudication.add_argument("--review-sha256", required=True)
    finalization = parsers.add_parser("finalize-native", help="Independently validate completed static exports after an exact serializer guard failure, retaining the failed session unchanged")
    finalization.add_argument("--input", required=True)
    finalization.add_argument("--review", required=True)
    finalization.add_argument("--review-sha256", required=True)
    attribution = parsers.add_parser("audit-native-attribution", help="Verify original embedded PE function attribution without altering raw parent or child captures")
    attribution.add_argument("--review", required=True)
    attribution.add_argument("--output")
    attribution.add_argument("--previous-audit-sha256", help="Explicit exact predecessor for a complete input-index revalidation; preserve the old audit")
    for command, description in (("bind-native-import-forwarders", "Bind exact independently verified original import-jump semantics"),
                                 ("bind-native-static-review", "Bind independently verified source or architecture contracts while retaining decompiler failures")):
        binding = parsers.add_parser(command, help=description)
        binding.add_argument("--input", required=True)
        binding.add_argument("--review", required=True)
        binding.add_argument("--review-sha256", required=True)
        if command == "bind-native-static-review":
            import native_static_reviews
            binding.add_argument("--kind", choices=native_static_reviews.KINDS, required=True)
            binding.add_argument("--previous-binding-sha256", help="Exact current binding SHA when retaining and replacing a reviewed static contract")
    arithmetic = parsers.add_parser("verify-native-arithmetic", help="Purely interpret a reviewed native arithmetic request without executing a target DLL")
    arithmetic.add_argument("--request", required=True)
    arithmetic.add_argument("--request-sha256", required=True)
    arithmetic.add_argument("--workers", type=int, choices=range(1, 17), default=4)
    compaction = parsers.add_parser("compact-analysis", help="Plan or apply transparent NTFS compression while retaining native evidence logical bytes and hashes")
    compaction.add_argument("--plan", default=".analysis/results/storage-compaction.json")
    compaction.add_argument("--minimum-mib", type=int, default=100)
    compaction.add_argument("--workers", type=int, default=8)
    compaction.add_argument("--review-receipt", action="append", help="Also compact this exact SHA-named immutable native review receipt; repeat for independent receipts")
    compaction.add_argument("--function-receipt", action="append", help="Also compact this exact canonical native function JSON, SHA-bound by its capture index and terminal analyzer session; repeat for independent functions")
    compaction.add_argument("--apply", action="store_true")
    compaction.add_argument("--plan-sha256", help="Exact SHA-256 of the independently reviewed dry-run plan")
    reference = parsers.add_parser("audit-reference", help="Verify every tracked byte and workspace dependency in a pinned architecture reference")
    reference.add_argument("--directory", required=True)
    reference.add_argument("--commit", required=True)
    reference.add_argument("--check-upstream", action="store_true")
    reference.add_argument("--restore-original-bytes", action="store_true", help="Replace clean-checkout filter conversions with verified raw Git blobs")
    reference.add_argument("--retained-snapshot", action="store_true", help="Revalidate a previously complete detached snapshot against its original Git blob and SHA-256 receipt; no new upstream currentness claim")
    reconcile = parsers.add_parser("reconcile-source-index", help="Verify retained source receipts through their owners before publishing complete input identities")
    reconcile.add_argument("--frontend", action="append", help="Explicit frontend acquisition receipt; repeat for multiple source sets")
    reconcile.add_argument("--platform-contract", action="append",
                           help="Explicit pinned official OS ABI/lifecycle acquisition; verify complete sources and declaration lines offline")
    reconcile.add_argument("--publication", action="append", help="Explicit targeted resource publication manifest; validates pinned source and alias identities without network")
    reconcile.add_argument("--instruction-manual", action="append", help="Explicit retained official architecture PDF acquisition receipt; verifies its origin, revision and complete bytes without network")
    parsers.add_parser("configure-ida", help="Regenerate direct IDA MCP configuration and plugin")
    probe = parsers.add_parser("probe-ida", help="Verify static MCP metadata, function lookup, complete pseudocode and assembly")
    probe.add_argument("--input", required=True)
    probe.add_argument("--function", required=True)
    inventory = parsers.add_parser("inspect-corpus", help="Verify original bytes and classify native/CLR binaries, aliases, imports, exports and versions")
    inventory.add_argument("--no-refresh-gaps", action="store_true")
    gaps = parsers.add_parser("refresh-gaps", help="Reconcile acquisition, graph and native failures from existing receipts")
    gaps.add_argument("--owner", action="append",
                      help="Refresh only this canonical semantic audit or native capture owner; repeat to reconcile a batch")
    acquisition = parsers.add_parser("acquire-native", help="Verify production device manifests and acquire all declared resources without execution")
    acquisition.add_argument("--manifest-url", action="append", help="Explicit official device manifest URL; repeat to acquire multiple products when retained receipts are unavailable")
    acquisition.add_argument("--include-host-app-resources", action="store_true", help="Also acquire resources declared by actual stable official host app HTML manifests")
    acquisition.add_argument("--no-refresh-gaps", action="store_true", help="Leave gap reconciliation to the owning batch")
    packages = parsers.add_parser("extract-native-packages", help="Apply the shared static archive workflow to acquired device and host app resources")
    packages.add_argument("--scope", choices=["all", "host-apps"], default="all")
    packages.add_argument("--seven-zip", default="C:/Program Files/7-Zip/7z.exe")
    packages.add_argument("--workers", type=int, default=2)
    packages.add_argument("--include-manifest-mismatches", action="store_true", help="Retain failed manifest roots as explicit static investigation without promoting their bytes")
    packages.add_argument("--no-refresh-gaps", action="store_true")
    corpus = parsers.add_parser("capture-corpus", help="Capture every unique DLL/NODE/SYS declared by verified production device manifests")
    corpus.add_argument("--ida-root", default=os.environ.get("RAZER_IDA_ROOT", "E:/IDA"))
    corpus.add_argument("--source", choices=["production", "retained"], default="production",
                        help="Capture verified production modules or all retained unique bytes while version acquisition proceeds")
    corpus.add_argument("--include-path", action="append", help="Prioritize matching alias regexes in this batch; every other module remains explicitly pending")
    corpus.add_argument("--workers", type=int, choices=range(2, 5), default=4,
                        help="Concurrent independent IDA processes; reuse verified captures and publish receipts from one coordinator")
    corpus.add_argument("--no-refresh-gaps", action="store_true")
    native = parsers.add_parser("capture-native", help="Launch static IDA/Hex-Rays full-function capture")
    native.add_argument("--input", required=True)
    native.add_argument("--resume", action="store_true", help="Reuse captured bodies and fill missing data for the same input")
    native.add_argument("--mcp-only", action="store_true", help="Open verified private input for static MCP queries without recapturing")
    native.add_argument("--exception-regions-only", action="store_true", help="Decode all validated original IA64 exception regions in a new private base-IDB copy without defining functions or invoking Hex-Rays")
    native.add_argument("--ida-root", default=os.environ.get("RAZER_IDA_ROOT", "E:/IDA"))
    native.add_argument("--no-refresh-gaps", action="store_true", help="Leave gap reconciliation to the caller after concurrent analyses finish")
    supplement = parsers.add_parser("supplement-native", help="Follow original PE pointer tables or reanalyze retained functions with assembly-reviewed types in a copied static IDB")
    supplement.add_argument("--input", required=True)
    supplement.add_argument("--pointer-table-rva", type=lambda value: int(value, 0))
    supplement.add_argument("--pointer-count", type=int)
    supplement.add_argument("--function-rva", type=lambda value: int(value, 0), action="append",
                            help="Reanalyze this exact retained function boundary; repeat for callers and callees")
    supplement.add_argument("--function-type", action="append", help="Explicit RVA=C function declaration backed by the complete retained assembly")
    supplement.add_argument("--decompiler-prepass-rva", action="append", type=lambda value: int(value, 0),
                            help="Explicit unchanged retained owner for one complete audited static derivation before final capture")
    supplement.add_argument("--import-type", action="append", help="Explicit normal-IAT RVA=C function-pointer declaration backed by import bytes and reviewed ABI evidence")
    supplement.add_argument("--reanalyze-type-owners", action="store_true", help="Invalidate only derived stack-analysis readiness and request processor reanalysis for exact selected owners after reviewed function/import type repairs")
    supplement.add_argument("--propagate-direct-calls", action="store_true", help="Explicitly propagate reviewed function types to byte-verified original E8 callsites before type-owner reanalysis")
    supplement.add_argument("--boundary-review", action="append", help="Exact reviewed original x64 PE exception-chain or x86 CI/stack/context entry recovery JSON[#pointer]; preserves all owner and helper bytes")
    supplement.add_argument("--instruction-review", action="append", help="Exact reviewed control-flow target and original E8 near-call decode JSON[#pointer]; preserves original PE bytes and function chunks")
    supplement.add_argument("--frame-review", action="append", help="Exact complete corrupt frame and machine-reference review JSON[#pointer]; CPU rebuilds only its private frame without supplied frame/SP values")
    supplement.add_argument("--stack-review", action="append", help="Exact complete ARM64 allocation/probe review JSON[#pointer]; derives a missing automatic stack point from original machine bytes")
    supplement.add_argument("--shared-tail-context-review", action="append", help="Exact existing x86 EH shared-tail parent/ABI review JSON[#pointer]; scopes CPU derivation and capture to its real owner and restores the original primary")
    supplement.add_argument("--switch-review", action="append", help="Exact original exhaustive masked signed32 jump-table review JSON#pointer; repairs only private-IDB switch metadata")
    supplement.add_argument("--optimizer-property-review", action="append", help="Exact original x64 outlined ADCX/ADOX and MULX review JSON[#pointer]; public DONT_PROP only on complete SDK def-use-proven generated closures")
    supplement.add_argument("--recursive-context-review", action="append", help="Exact original x64 root fallthrough and separate recursive core review JSON[#pointer]; shared SDK frame with all physical CALLs and RETs retained")
    supplement.add_argument("--merge-functions", action="append",
                            help="Explicit reviewed OWNER=RVA,RVA,... merge of adjacent complete retained functions")
    supplement.add_argument("--outline-function-rva", action="append", type=lambda v: int(v, 0),
                            help="Explicit reviewed complete helper boundary to mark FUNC_OUTLINE in the private IDB")
    supplement.add_argument("--partition-functions", action="append",
                            help="Explicit reviewed OWNER=START:END,... partition of embedded local CALL/RET helpers")
    supplement.add_argument("--share-function-tail", action="append",
                            help="Reviewed PRIMARY=START:END:OWNER,OWNER[:inline|:retain] exact shared function tail")
    supplement.add_argument("--fresh-inline-function-rva", action="append", type=lambda value: int(value, 0),
                            help="Fresh SDK inline of an already reviewed outlined helper with its own frame; bypass reused inline copies")
    supplement.add_argument("--fresh-inline-register-leaf-rva", action="append", type=lambda value: int(value, 0),
                            help="Explicit complete register-only leaf nested in a reviewed fresh helper; exact SDK fictitious mapping, no external frame flag")
    supplement.add_argument("--type-evidence", help="Explain the argument/register/return evidence for explicit type recovery")
    supplement.add_argument("--observation-evidence",
                            help="Retained JSON review#pointer with a new static observation reason and exact observer source identity")
    supplement.add_argument("--callsite-type-review", action="append",
                            help="Exact retained JSON#pointer proving an indirect CALL, original vtable slot, object binding and callee ABI")
    supplement.add_argument("--max-function-kib", type=int,
                            help="Record and apply a per-process Hex-Rays function size limit, 64 to 1024 KiB")
    supplement.add_argument("--recover-interrupted", action="store_true",
                            help="Preserve and resume an unfinished supplement only after its process is confirmed absent")
    supplement.add_argument("--ida-root", default=os.environ.get("RAZER_IDA_ROOT", "E:/IDA"))
    supplement.add_argument("--no-refresh-gaps", action="store_true")
    chain = parsers.add_parser("audit-native-chain", help="Join complete original host callables, literal FFI signatures and verified native bodies/xrefs")
    chain.add_argument("--query", choices=["all", "hid-feature", "mapping-engine", "simple-service", "notification-restart", "startup-settings"], default="all")
    fetch = parsers.add_parser("fetch-frontend", help="Fetch HTML, manifest and static dependency closure")
    fetch.add_argument("--url", required=True)
    fetch.add_argument("--output", required=True)
    fetch.add_argument("--repair", action="store_true", help="Reacquire missing or changed bytes, failed stability endpoints and transient HTTP failures through the shared family repair workflow")
    fetch.add_argument("--no-refresh-gaps", action="store_true", help="Leave gap reconciliation to the caller after concurrent analyses finish")
    fetch.add_argument("--manifest-url", dest="seed_urls", action="append",
                       help="Additional product manifest verified by the acquisition caller")
    frozen_sources = parsers.add_parser("acquire-frozen-frontend-sources",
        help="Acquire only exact official dependency URLs missing from the frozen unresolved frontend tasks")
    frozen_sources.add_argument("--workers", type=int, choices=range(2, 17), default=8)
    apps = parsers.add_parser("acquire-host-apps", help="Follow literal current host window descriptors and acquire their complete static references")
    apps.add_argument("--source-set", action="append", help="Start from a selected retained stable host app namespace; repeat for several")
    apps.add_argument("--stage", choices=["report", "acquire", "analyze", "all"], default="all")
    apps.add_argument("--no-refresh-gaps", action="store_true")
    embedded = parsers.add_parser("extract-embedded-inputs", help="Retain verified managed resources, bounded CABs and explicitly reviewed PE files for static analysis")
    embedded.add_argument("--source-prefix", default=".analysis/inputs/official-native-current/")
    embedded.add_argument("--pe-review", action="append", help="Explicit reviewed PE groups in a retained result: project-relative.json#JSON-pointer; repeat for independent sources")
    embedded.add_argument("--no-refresh-gaps", action="store_true")
    extracted = parsers.add_parser("reconcile-native-extractions", help="Exclude failed static decompression artifacts and descendants from original-source analysis")
    extracted.add_argument("--no-refresh-gaps", action="store_true")
    frontend = parsers.add_parser("analyze-frontend", help="Restore all acquired JS and embedded original sources")
    frontend_scope = frontend.add_mutually_exclusive_group(required=True)
    frontend_scope.add_argument("--input")
    frontend_scope.add_argument("--all-acquired", action="store_true", help="Recover missing module structure across all retained frontend source sets")
    frontend.add_argument("--refresh", action="store_true", help="Replace generated recovery trees after verifying originals")
    frontend.add_argument("--recover-missing", action="store_true", help="Extend a verified existing capture with modules hidden in top-level sequence expressions")
    frontend.add_argument("--workers", type=int, default=2, help="Independent static recovery workers for --all-acquired (2 to 8)")
    frontend.add_argument("--no-refresh-gaps", action="store_true", help="Leave gap reconciliation to the caller after concurrent analyses finish")


def run(args):
    if args.command in {"bind-native-import-forwarders", "bind-native-static-review"}:
        import native_static_reviews
        return native_static_reviews.publish_operation(sys.modules[__name__], args)
    if args.command == "audit-native-attribution":
        import native_embedded
        return native_embedded.audit_function_attribution(args)
    if args.command == "acquire-frozen-frontend-sources":
        import frontend
        return frontend.acquire_frozen_frontend_sources(args)
    if args.command == "verify-native-arithmetic":
        import native_arithmetic
        return native_arithmetic.run(sys.modules[__name__], args)
    if args.command == "adjudicate-native":
        import native_adjudication
        return native_adjudication.run(sys.modules[__name__], args)
    if args.command == "finalize-native":
        import native_adjudication
        return native_adjudication.finalize(sys.modules[__name__], args)
    if args.command == "compact-analysis":
        import analysis_storage
        return analysis_storage.run(args)
    if args.command == "reverse-pending":
        import reverse_pending
        return reverse_pending.run(args)
    if args.command in {"acquire-installed", "acquire-host", "inspect-host", "analyze-host"}:
        import host
        return host.run(args)
    {"audit-reference": audit_reference, "reconcile-source-index": reconcile_source_index,
     "configure-ida": lambda _: configure_ida(), "inspect-corpus": inspect_corpus,
     "extract-native-packages": extract_native_packages,
     "probe-ida": probe_ida,
     "refresh-gaps": lambda args: print(json.dumps({"owners": len(refresh_gaps(getattr(args, "owner", None))["owners"]),
         "selected_owners": getattr(args, "owner", None)}), flush=True),
     "acquire-native": acquire_native, "capture-corpus": capture_corpus,
     "capture-native": capture_native, "supplement-native": supplement_native,
     "audit-native-chain": audit_native_chain, "fetch-frontend": fetch_frontend,
     "analyze-frontend": analyze_frontend, "acquire-host-apps": acquire_host_apps,
     "extract-embedded-inputs": extract_embedded_inputs,
     "reconcile-native-extractions": reconcile_native_extractions}[args.command](args)

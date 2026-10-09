"""Inventory current reference bytes and manifests without executing any source.

This is acquisition/provenance coverage, not a claim of semantic decompilation.
Only explicit current roots are scanned; obsolete and historical caches are
never traversed. Output is deterministic, so --check compares actual bytes.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import gzip
import json
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs/re/full-source-corpus-current-evidence.json"
DETAIL = OUTPUT.with_suffix(".json.gz")
SCOPES = {
    "host": ".ref/host-4.0.827",
    "applications": ".ref/applications",
    "products": ".ref/devices",
    "middleware": ".ref/middleware",
    "native_resources": ".ref/native-libraries",
    "framework": ".ref/framework",
}
CODE = {".js", ".cjs", ".mjs", ".jsx", ".ts", ".tsx", ".css", ".html", ".map", ".cc", ".h", ".c", ".cpp", ".py"}
BINARY = {".dll", ".node", ".exe"}


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def relative(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def inspect() -> dict:
    files, manifests, issues, scopes = [], [], [], {}
    rows: dict[str, dict] = {}
    recovery_path = ROOT / "docs/re/current-media-offline-recovery.json"
    recovery_bytes = recovery_path.read_bytes()
    recovery = {item["target"]: item for item in json.loads(recovery_bytes).get("recovered", [])}
    for scope, directory in SCOPES.items():
        base = ROOT / directory
        counts, size = Counter(), 0
        for path in sorted(base.rglob("*")):
            if not path.is_file():
                continue
            if path.is_symlink():
                issues.append({"kind": "symlink_not_followed", "path": relative(path)})
                continue
            # HTTP receipts/errors are evidence, not the acquired source body.
            if path.name.endswith(".http.json") or path.suffix == ".response":
                continue
            data = path.read_bytes()
            suffix = path.suffix.lower()
            role = "code" if suffix in CODE else "native_binary" if suffix in BINARY else "metadata" if suffix == ".json" else "resource"
            row = {"path": relative(path), "scope": scope, "role": role,
                   "bytes": len(data), "sha256": sha(data)}
            receipt_path = path.with_name(path.name + ".http.json")
            if receipt_path.is_file():
                receipt_data = receipt_path.read_bytes()
                try:
                    receipt = json.loads(receipt_data)
                    matches = receipt.get("sha256") == row["sha256"]
                    length_matches = receipt.get("bytes") in (None, len(data))
                    status = receipt.get("http_status", receipt.get("status"))
                    source_url = receipt.get("source_url", receipt.get("url"))
                    row["http"] = {"path": relative(receipt_path), "sha256": sha(receipt_data),
                                   "status": status,
                                   "source_url": source_url,
                                   "final_url": receipt.get("final_url", source_url),
                                   "fetched_at_utc": receipt.get("fetched_at_utc"),
                                   "body_hash_matches": matches,
                                   "body_length_matches": length_matches}
                    recovered = recovery.get(row["path"])
                    if recovered and recovered["sha256"] == row["sha256"]:
                        candidate = ROOT / recovered["candidate"]
                        recovery_manifest = json.loads(recovery_bytes)["manifest"]
                        assert candidate.is_file() and sha(candidate.read_bytes()) == row["sha256"]
                        assert sha((ROOT / recovery_manifest["path"]).read_bytes()) == recovery_manifest["sha256"]
                        row["offline_recovery"] = {"path": relative(recovery_path), "sha256": sha(recovery_bytes),
                                                   "candidate": recovered["candidate"], "fingerprint": recovered["fingerprint"],
                                                   "independent_http_byte_comparison": False}
                        issues.append({"kind": "offline_recovery_with_historical_failed_http", "path": row["path"], "receipt": relative(receipt_path)})
                    elif not matches or not length_matches or status not in (200, None):
                        issues.append({"kind": "http_body_mismatch", "path": row["path"], "receipt": relative(receipt_path)})
                    elif status is None:
                        issues.append({"kind": "http_status_not_recorded", "path": row["path"], "receipt": relative(receipt_path)})
                except (ValueError, TypeError) as error:
                    issues.append({"kind": "invalid_http_receipt", "path": relative(receipt_path), "error": str(error)})
            rows[row["path"]] = row
            files.append(row)
            counts[role] += 1
            counts["suffix:" + suffix] += 1
            size += len(data)
        scopes[scope] = {"root": directory, "files": sum(counts[role] for role in ("code", "native_binary", "metadata", "resource")),
                         "bytes": size, "counts": dict(sorted(counts.items()))}
        print(f"Corpus {scope}: {scopes[scope]['files']} files inspected", flush=True)

    for row in files:
        path = ROOT / row["path"]
        if path.name not in ("asset-manifest.json", "webpackManifest.json"):
            continue
        try:
            manifest = json.loads(path.read_text(encoding="utf-8"))
        except ValueError as error:
            issues.append({"kind": "invalid_manifest", "path": row["path"], "error": str(error)})
            continue
        values = manifest.get("files", manifest)
        if not isinstance(values, dict):
            issues.append({"kind": "unsupported_manifest_shape", "path": row["path"]})
            continue
        targets = {}
        manifest_url = row.get("http", {}).get("source_url") or ""
        url_base = urlsplit(manifest_url).path.rsplit("/", 1)[0] + "/"
        for key, value in values.items():
            if not isinstance(value, str):
                continue
            url = urlsplit(value)
            name = unquote(url.path)
            local_absolute = name.startswith(url_base) and url_base != "/" and (
                not url.netloc or url.netloc == urlsplit(manifest_url).netloc)
            if (url.scheme or url.netloc) and not local_absolute:
                targets.setdefault(value, {"value": value, "status": "external_url", "keys": []})["keys"].append(key)
                continue
            if local_absolute:
                name = name[len(url_base):]
            target = (path.parent / name.removeprefix("./")).resolve()
            if name.startswith("/") or not target.is_relative_to(path.parent.resolve()):
                targets.setdefault(value, {"value": value, "status": "absolute_or_outside_manifest_directory", "keys": []})["keys"].append(key)
                continue
            name = relative(target)
            status = "acquired" if name in rows else "not_acquired"
            entry = targets.setdefault(name, {"path": name, "status": status, "keys": []})
            entry["keys"].append(key)
            if name in rows:
                entry["sha256"] = rows[name]["sha256"]
            if status != "acquired" and target.suffix.lower() in {".js", ".css"}:
                issues.append({"kind": "manifest_code_not_acquired", "manifest": row["path"], "path": name})
        manifests.append({"path": row["path"], "sha256": row["sha256"],
                          "targets": list(targets.values()),
                          "statuses": dict(Counter(target["status"] for target in targets.values()))})

    # A failed response has no source body; retain its status without pretending
    # that an absent endpoint is an empty application or an empty product UI.
    failed_responses = []
    for directory in SCOPES.values():
        for path in sorted((ROOT / directory).rglob("*.http.json")):
            body = path.with_name(path.name.removesuffix(".http.json"))
            if body.is_file():
                continue
            try:
                data = path.read_bytes()
                receipt = json.loads(data)
                failed_responses.append({"path": relative(path), "sha256": sha(data),
                                         "source_url": receipt.get("source_url"),
                                         "http_status": receipt.get("http_status"),
                                         "result": receipt.get("result")})
            except ValueError as error:
                issues.append({"kind": "invalid_http_receipt", "path": relative(path), "error": str(error)})

    return {"schema_version": 1, "method": "Static filesystem/hash/JSON parsing only; no vendor source or native binary executed",
            "generator_sha256": sha(Path(__file__).read_bytes().replace(b"\r\n", b"\n")),
            "scopes": scopes,
            "summary": {"files": len(files), "bytes": sum(row["bytes"] for row in files),
                        "code_files": sum(row["role"] == "code" for row in files),
                        "native_binary_files": sum(row["role"] == "native_binary" for row in files),
                        "http_body_receipts": sum("http" in row for row in files),
                        "manifests": len(manifests), "failed_or_absent_bodies": len(failed_responses),
                        "issue_counts": dict(Counter(issue["kind"] for issue in issues))},
            "files": files, "manifests": manifests, "absent_bodies": failed_responses, "issues": issues,
            "boundaries": ["Current means the locally acquired versions established by existing version/HTTP/archive receipts; this scan does not re-fetch the latest online release.",
                           "No obsolete directories, discovery historical snapshots, .ref/tools scripts or historical aliases are inspected.",
                           "A matching hash proves byte identity only; it is not a whole-function semantic review, original C++ source recovery, UI completion or hardware validation.",
                           "Legacy HTTP receipts may omit status or describe a preserved failed request before separately evidenced offline recovery; these are explicit provenance boundaries, not fabricated HTTP successes.",
                           "Manifest resources absent locally remain explicit gaps; acquired JS/CSS does not imply all images, remote services, conditional chunks or native implementations are recovered.",
                           "Host extraction/native package completeness is governed by ARCHIVE-CHAIN and native extraction receipts, not by the count of files in this directory."],
            "runtime_validation": "not_run"}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="Write newly inspected evidence")
    args = parser.parse_args()
    data = inspect()
    detail_bytes = (json.dumps(data, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
    summary = {key: value for key, value in data.items() if key not in {"files", "manifests", "absent_bodies", "issues"}}
    summary["detail"] = {"path": relative(DETAIL), "encoding": "gzip UTF-8 JSON; deterministic mtime=0",
                         "uncompressed_bytes": len(detail_bytes), "uncompressed_sha256": sha(detail_bytes)}
    summary["manifest_status_counts"] = dict(sum((Counter(manifest["statuses"]) for manifest in data["manifests"]), Counter()))
    summary["issue_examples"] = data["issues"][:20]
    summary["absent_body_examples"] = data["absent_bodies"][:20]
    serialized = json.dumps(summary, ensure_ascii=False, indent=2) + "\n"
    if args.write:
        DETAIL.write_bytes(gzip.compress(detail_bytes, mtime=0))
        OUTPUT.write_text(serialized, encoding="utf-8")
    else:
        assert OUTPUT.read_text(encoding="utf-8").replace("\r\n", "\n") == serialized, "Corpus evidence changed; review before --write"
        assert gzip.decompress(DETAIL.read_bytes()) == detail_bytes, "Corpus detail changed; review before --write"
    print(json.dumps(data["summary"], ensure_ascii=False))
    print("Corpus byte/manifest inventory checked; semantic reverse-engineering remains separately evidenced.")


if __name__ == "__main__":
    main()

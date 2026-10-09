"""Audit local asset bytes, registration, consumers and duplication statically.

Literal references are exact observations. Unmatched dynamic names remain
unknown; absence from this scan is never permission to delete a runtime asset.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs/re/assets-current-inventory.json"
OBSOLETE = (".ref/frontend/", ".ref/synapse-asar/", ".ref/host-4.0.821/",
            ".work/latest-source-check/host-4.0.821/")
INCLUDE = re.compile(r'include(?:_bytes|_str)?!\(\s*"([^"\n]+)"\s*\)')
PAIR = re.compile(r'\(\s*"(synapse/[^"\n]+)"\s*,\s*include_bytes!\(\s*"([^"\n]+)"\s*\)')
LITERAL = re.compile(r'"(synapse/[\w./-]+\.(?:png|svg|webp|avif|rgba|ttf|json))"')


def production_modules() -> list[Path]:
    """Walk workspace package roots, then declared non-test Rust modules."""
    manifests = {}
    for p in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        data = tomllib.loads(p.read_text("utf-8"))
        manifests[data["package"]["name"]] = (p.parent, data)
    root = tomllib.loads((ROOT / "Cargo.toml").read_text("utf-8"))
    package_names, pending = set(), [name for name in root["dependencies"] if name in manifests]
    while pending:
        name = pending.pop()
        if name in package_names:
            continue
        package_names.add(name)
        _, data = manifests[name]
        sections = [data.get("dependencies", {})]
        sections += [v.get("dependencies", {}) for v in data.get("target", {}).values()]
        pending.extend(dep for section in sections for dep in section if dep in manifests)
    files = set()

    def visit(p: Path) -> None:
        p = p.resolve()
        if p in files:
            return
        assert p.is_file(), p
        files.add(p)
        text = p.read_text("utf-8").split("#[cfg(test)]\nmod tests")[0]
        declaration = r'(?P<attrs>(?:#\[[^\]]+\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?mod\s+(?P<name>\w+)\s*;'
        for match in re.finditer(declaration, text):
            if "cfg(test)" in match["attrs"]:
                continue
            override = re.search(r'path\s*=\s*"([^"]+)"', match["attrs"])
            base = p.parent if p.stem in {"lib", "main", "mod"} else p.parent / p.stem
            child = p.parent / override[1] if override else base / (match["name"] + ".rs")
            if not override and not child.is_file():
                child = base / match["name"] / "mod.rs"
            visit(child)

    for name in sorted(package_names):
        directory, data = manifests[name]
        lib = directory / data.get("lib", {}).get("path", "src/lib.rs")
        if lib.exists():
            visit(lib)
        for binary in data.get("bin", []):
            visit(directory / binary["path"])
    for binary in root["bin"]:
        visit(ROOT / binary["path"])
    return sorted(files)


def audit() -> dict:
    files = sorted(p for p in (ROOT / "assets").rglob("*") if p.is_file())
    cache, references, registrations = {}, defaultdict(set), defaultdict(list)
    errors, metadata_checks = [], 0

    def relative(p):
        return p.relative_to(ROOT).as_posix()

    def digest(p):
        if p not in cache:
            cache[p] = hashlib.sha256(p.read_bytes()).hexdigest()
        return cache[p]

    def verify(name, expected, owner):
        if name.startswith(OBSOLETE):
            errors.append({"kind": "obsolete_source", "owner": owner, "path": name})
            return
        p = (ROOT / name).resolve()
        if not p.is_relative_to(ROOT):
            errors.append({"kind": "outside_workspace", "owner": owner, "path": name})
        elif not p.is_file():
            errors.append({"kind": "missing_receipt_file", "owner": owner, "path": name})
        elif digest(p) != expected:
            errors.append({"kind": "digest", "owner": owner, "path": name,
                           "expected": expected, "actual": digest(p)})

    def metadata(value, owner):
        nonlocal metadata_checks
        if isinstance(value, dict):
            for name_key, hash_keys in (("source", ("source_sha256",)),
                                        ("output", ("output_sha256", "sha256")),
                                        ("module_file", ("module_sha256",)),
                                        ("context", ("context_sha256",))):
                name = value.get(name_key)
                expected = next((value.get(k) for k in hash_keys if value.get(k)), None)
                if isinstance(name, str) and expected and name.startswith(("assets/", ".ref/")):
                    metadata_checks += 1
                    verify(name, expected, owner)
            for nested in value.values():
                metadata(nested, owner)
        elif isinstance(value, list):
            for nested in value:
                metadata(nested, owner)

    for p in files:
        if p.suffix == ".json":
            try:
                metadata(json.loads(p.read_text("utf-8")), relative(p))
            except (UnicodeError, json.JSONDecodeError) as error:
                errors.append({"kind": "json", "path": relative(p), "error": str(error)})

    modules = production_modules()
    embedded = set()

    def includes(p):
        if p in embedded:
            return
        embedded.add(p)
        text = p.read_text("utf-8").split("#[cfg(test)]\nmod tests")[0]
        for match in INCLUDE.finditer(text):
            target = (p.parent / match[1]).resolve()
            if not target.is_file():
                errors.append({"kind": "missing_include", "owner": relative(p), "path": match[1]})
                continue
            if target.is_relative_to(ROOT / "assets"):
                references[relative(target)].add(relative(p))
                if target.suffix == ".rs":
                    includes(target)
        for key, name in PAIR.findall(text):
            registrations[key].append({"table": relative(p), "file": relative((p.parent / name).resolve())})
        # The Demo controls use match arms, rather than a tuple table.
        for key, name in re.findall(r'"(synapse/[^"\n]+)"\s*=>\s*Some\(include_bytes!\("([^"\n]+)"\)\)', text):
            registrations[key].append({"table": relative(p), "file": relative((p.parent / name).resolve())})

    for p in modules:
        includes(p)
    direct = defaultdict(set)
    for p in [*modules, *(ROOT / "crates").rglob("*.json"), *(ROOT / "assets").rglob("*.rs")]:
        text = p.read_text("utf-8").split("#[cfg(test)]\nmod tests")[0]
        for key in LITERAL.findall(text):
            direct[key].add(relative(p))
    for key in sorted(direct):
        if key not in registrations:
            # JSON literals may describe source resources before preparation.
            errors.append({"kind": "unregistered_literal", "key": key, "consumers": sorted(direct[key])})
        else:
            for row in registrations[key]:
                references[row["file"]].update(direct[key])
    for key, rows in sorted(registrations.items()):
        if len(rows) > 1:
            errors.append({"kind": "duplicate_runtime_key", "key": key, "registrations": rows})
    resource = (ROOT / "crates/razer-assets/src/lib.rs").read_text("utf-8")
    load, listed = resource.split("fn list(", 1)
    listed = listed.split("pub fn register_fonts", 1)[0]
    if set(re.findall(r'\.chain\((\w+)\)', load)) != set(re.findall(r'\.chain\((\w+)\)', listed)):
        errors.append({"kind": "load_list_family_mismatch"})
    by_hash = defaultdict(list)
    inventory = []
    for p in files:
        name = relative(p)
        by_hash[digest(p)].append(name)
        inventory.append({"path": name, "bytes": p.stat().st_size, "sha256": digest(p),
                          "static_references": sorted(references[name])})
    sizes = {row["path"]: row["bytes"] for row in inventory}
    duplicates = [{"sha256": sha, "bytes_each": sizes[names[0]], "files": names,
                   "status": "same_bytes; domain keys/provenance reviewed separately"}
                  for sha, names in sorted(by_hash.items()) if len(names) > 1]
    return {"schema_version": 1,
            "method": "Workspace package/module traversal, literal includes/keys, resource metadata hashes and exact byte duplication; no application or vendor execution",
            "summary": {"files": len(files), "bytes": sum(sizes.values()),
                        "production_modules": len(modules), "runtime_keys": len(registrations),
                        "metadata_hash_checks": metadata_checks, "duplicate_content_groups": len(duplicates),
                        "redundant_bytes": sum(g["bytes_each"] * (len(g["files"])-1) for g in duplicates),
                        "errors": len(errors)},
            "errors": errors, "files": inventory, "duplicate_content": duplicates,
            "registrations": dict(sorted(registrations.items())),
            "limits": ["Static literals do not fully resolve formatted names, conditions or runtime data.",
                       "Registered but unmatched resources remain candidates, never proved unused.",
                       "Same bytes do not imply identical product/edition/layout identity or source provenance.",
                       "No pixel/font/runtime rendering acceptance; missing upstream manifest targets are tracked separately in the full-source corpus."]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()
    report = audit()
    encoded = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.check:
        if OUTPUT.read_text("utf-8") != encoded:
            raise SystemExit("Asset inventory changed; inspect evidence before updating")
    elif not args.summary:
        OUTPUT.write_text(encoded, encoding="utf-8", newline="\n")
    print(json.dumps(report["summary"]))
    for error in report["errors"]:
        print(json.dumps(error, ensure_ascii=False))
    return int(bool(report["errors"]))


if __name__ == "__main__":
    sys.exit(main())

"""Refresh current workspace structure; preserve all historical relocation facts.

These fingerprints describe current files, not source fidelity or completion.
Review actual diffs before invoking this after parallel edits and formatting.
"""
import argparse
import hashlib
import json
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def read(path):
    return json.loads(path.read_text("utf-8"))


def store(path, data, check):
    if check:
        assert read(path) == data, f"Current workspace receipt is stale: {path.name}"
    else:
        path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    architecture_path = ROOT / "docs/re/workspace-openlogi-architecture-current.json"
    architecture = read(architecture_path)
    for entry in architecture["current_packages"]:
        directory = ROOT / "crates" / entry["name"]
        manifest = tomllib.loads((directory / "Cargo.toml").read_text("utf-8"))
        assert manifest["package"]["name"] == entry["name"]
        files = list((directory / "src").rglob("*.rs"))
        entry["rust_files"] = len(files)
        entry["rust_lines"] = sum(len(path.read_text("utf-8").splitlines()) for path in files)
        entry["dependencies"] = sorted(name for name in manifest.get("dependencies", {}) if name.startswith("razer-"))
    shell = next(e for e in architecture["current_packages"] if e["name"] == "razer-shell")
    architecture["host_reduction"]["after_rust_files"] = shell["rust_files"]
    architecture["host_reduction"]["after_rust_lines"] = shell["rust_lines"]
    store(architecture_path, architecture, args.check)

    relocation_path = ROOT / "docs/re/workspace-relocation-current.json"
    relocation = read(relocation_path)
    # A newly added file was not relocated from the historical tree. Only
    # refresh existing current_files digests; leave before/after maps untouched.
    for entry in relocation["current_files"]:
        entry["sha256"] = hashlib.sha256((ROOT / entry["path"]).read_bytes()).hexdigest()
    store(relocation_path, relocation, args.check)
    print(f"Current workspace: {len(architecture['current_packages'])} packages; {len(relocation['current_files'])} current file fingerprints; historical facts untouched")


if __name__ == "__main__":
    main()

"""Raw Hugging Face/Git LFS resource snapshots; workspace.py owns the CLI.

Developer preparation only. Neither Cargo nor the product downloads resources.
"""
from __future__ import annotations

import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import urllib.parse
import urllib.request

import analysis

ROOT = Path(__file__).resolve().parent.parent
LOCK = "tools/asset-release.json"
OUTPUT = ".work/asset-releases"
NATIVE_ICONS = (
    "a64b8a64b721fb74-tray-native.rgba", "f979021796dfd342-tray-app.rgba",
    "81851a9004a803d3-tray-icon-24.rgba", "71b354f660adb5db-tray-icon.rgba",
    "f8fd980e3bec82fe-tray-icon-40.rgba", "5e5df98271550aca-tray-icon-48.rgba",
    "afb1350c23ce2416-tray-icon-64.rgba", "7747f51f284ded65-tray-gear-dark.rgba",
    "33892a0b56b91958-tray-gear-light.rgba",
)
GIT_ICONS = {"assets/synapse/shared/pending-source-audit/" + n for n in NATIVE_ICONS}
META_FILES = {".gitattributes", "README.md", "release-index.json"}
LFS_ATTRIBUTES = (b"* -filter -diff -merge -text\n"
                  b"assets/** filter=lfs diff=lfs merge=lfs -text\n")


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def published_manifest(lock):
    # A commit cannot contain its own hash; only the source lock pins storage.
    return {k: v for k, v in lock.items() if k != "storage"}


def content_hash(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                    separators=(",", ":")).encode("utf-8")).hexdigest()


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def canonical(name):
    if (not isinstance(name, str) or not name.startswith("assets/")
            or any(c in name for c in '\\:<>"|?*') or any(ord(c) < 32 for c in name)
            or any(p in ("", ".", "..") or p.endswith((" ", ".")) for p in name.split("/"))
            or any(re.fullmatch(r"(?i)(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?", p)
                   for p in name.split("/"))):
        raise ValueError("Noncanonical asset path: " + repr(name))
    return name


def source_path(name, root=None):
    canonical(name)
    root = (root or ROOT).resolve()
    path, current = root / name, root / name
    while current != root:
        if (current.is_symlink() or (current.exists()
                and getattr(current.lstat(), "st_file_attributes", 0) & 0x400)):
            raise ValueError("Asset path traverses a link: " + name)
        current = current.parent
    if not path.resolve().is_relative_to(root / "assets"):
        raise ValueError("Asset path escaped assets/: " + name)
    return path


def in_git(name):
    return (name.startswith("assets/synapse/tables/") and name.endswith(".rs")) or name in GIT_ICONS


def identity(name):
    path = source_path(name)
    if not path.is_file():
        raise ValueError("Missing asset source: " + name)
    size = path.stat().st_size
    if size < 1024 and path.read_bytes().startswith(b"version https://git-lfs.github.com/spec/v1\n"):
        raise ValueError("Hydrate the Git LFS pointer first: " + name)
    return {"path": name, "bytes": size, "sha256": digest(path)}


def group_name(name):
    match = re.match(r"assets/synapse/products/([1-9][0-9]*)/", name)
    if name.startswith("assets/synapse/products/") and not match:
        raise ValueError("Asset product directory has no canonical PID: " + name)
    return "product-" + match[1] if match else "shared"


def inventory():
    paths = sorted(p.relative_to(ROOT).as_posix() for p in (ROOT / "assets").rglob("*")
                   if p.is_file() and p != ROOT / "assets/synapse.pack"
                   and not (p.parent == ROOT / "assets" and p.name.startswith("synapse.pack.")
                            and p.name.endswith(".tmp")))
    if not paths or len({p.casefold() for p in paths}) != len(paths):
        raise ValueError("Asset inventory is empty or contains Windows path collisions")
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        rows = list(executor.map(identity, paths))
    groups, retained = {"shared": []}, []
    for row in rows:
        if in_git(row["path"]):
            retained.append(row)
        else:
            groups.setdefault(group_name(row["path"]), []).append(row)
    result = []
    for name, files in sorted(groups.items()):
        required = set() if name == "shared" else {"shared"}
        if name != "shared":
            pid = int(name.removeprefix("product-"))
            index = ROOT / f"assets/synapse/products/{pid}/resources.json"
            if index.is_file():
                data = json.loads(index.read_bytes())
                if data.get("product_id") != pid:
                    raise ValueError("Product resource index identity differs: " + str(pid))
                for item in data.get("resources", []):
                    dependency = group_name(canonical(item["physical_path"]))
                    if dependency != name:
                        required.add(dependency)
        if not required <= groups.keys():
            raise ValueError("Missing cross-product resource dependency: " + name)
        result.append({"name": name, "requires": sorted(required), "files": files})
    return result, retained


def repository_name(value):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", value or ""):
        raise ValueError("Expected a Hugging Face owner/dataset")
    return value


def rows_for(lock, groups=None):
    return [r for g in (lock["groups"] if groups is None else groups) for r in g["files"]]


def matches(row, root=None):
    path = source_path(row["path"], root)
    return path.is_file() and path.stat().st_size == row["bytes"] and digest(path) == row["sha256"]


def copy_verified(row, source_root, destination_root):
    source, target = source_path(row["path"], source_root), source_path(row["path"], destination_root)
    if not source.is_file():
        raise ValueError("Missing snapshot source: " + row["path"])
    target.parent.mkdir(parents=True, exist_ok=True)
    count, h = 0, hashlib.sha256()
    with source.open("rb") as stream, target.open("xb") as output:
        while block := stream.read(min(1024 * 1024, row["bytes"] - count + 1)):
            count += len(block)
            if count > row["bytes"]:
                raise ValueError("Snapshot file exceeds its locked length: " + row["path"])
            h.update(block)
            output.write(block)
    if count != row["bytes"] or h.hexdigest() != row["sha256"]:
        raise ValueError("Snapshot file SHA-256 differs: " + row["path"])


def prepare(repository=None):
    old = json.loads((ROOT / LOCK).read_bytes()) if (ROOT / LOCK).is_file() else {}
    repository = repository_name(repository or old.get("storage", {}).get("repository"))
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        groups, retained = inventory()
        source_hash = content_hash({"groups": groups, "git_files": retained})
        lock = {"schema_version": 2, "format": "files", "tag": "assets-" + source_hash[:20],
                "source_sha256": source_hash, "git_files": retained, "groups": groups,
                "storage": {"provider": "huggingface", "repository": repository, "revision": None}}
        if published_manifest(old) == published_manifest(lock) and old.get("storage", {}).get("repository") == repository:
            lock["storage"] = old["storage"]
        directory = ROOT / OUTPUT / lock["tag"]
        directory.mkdir(parents=True, exist_ok=True)
        for row in rows_for(lock):
            if not matches(row, directory):
                if source_path(row["path"], directory).exists():
                    raise ValueError("Prepared snapshot was edited: " + row["path"])
                copy_verified(row, ROOT, directory)
        (directory / "release-index.json").write_bytes(encoded(published_manifest(lock)))
        (directory / ".gitattributes").write_bytes(LFS_ATTRIBUTES)
        (directory / "README.md").write_text(
            "---\npretty_name: Razer UI local product resources\ntags:\n- desktop-ui\n---\n\n"
            f"Raw files for `{lock['tag']}`. Products retain `assets/synapse/products/<PID>/`; "
            "shared files retain their original paths. Resource payloads use Git LFS. No ZIP archives.\n\n"
            "Use the matching source checkout and its `tools/asset-release.json`: "
            "`python tools/workspace.py fetch-asset-release`. Add `--product PID` for a source update "
            "including shared and cross-product dependencies. Full workspace compilation requires all products. "
            "Prepare the local runtime pack with the source workspace tools. The application never downloads resources.\n\n"
            "Vendor artwork, fonts and binaries retain their original rights. This dataset grants no license "
            "to vendor content.\n", encoding="utf-8", newline="\n")
        if inventory() != (groups, retained):
            raise ValueError("Asset sources changed during preparation; retry after writers finish")
        analysis.save(ROOT / LOCK, lock)
    return {"tag": lock["tag"], "directory": directory.relative_to(ROOT).as_posix(),
            "products": len(groups) - 1, "source_files": len(rows_for(lock)),
            "source_bytes": sum(r["bytes"] for r in rows_for(lock)), "git_files": len(retained), "passed": True}


def validate_lock(lock):
    storage = lock.get("storage", {})
    if storage.get("provider") != "huggingface":
        raise ValueError("Asset storage provider must be Hugging Face")
    repository_name(storage.get("repository"))
    if storage.get("revision") is not None and not re.fullmatch(r"[0-9a-f]{40}", storage["revision"]):
        raise ValueError("Asset downloads require a complete immutable HF commit")
    if (lock.get("schema_version") != 2 or lock.get("format") != "files"
            or not re.fullmatch(r"assets-[a-f0-9]{20}", lock.get("tag", ""))):
        raise ValueError("Invalid raw asset snapshot lock")
    seen, groups = set(), {}
    for group in lock.get("groups", []):
        name = group.get("name", "")
        if not re.fullmatch(r"shared|product-[1-9][0-9]*", name) or name in groups:
            raise ValueError("Invalid or repeated resource group")
        groups[name] = group
        for row in group.get("files", []):
            path = canonical(row["path"])
            if in_git(path) or path == "assets/synapse.pack" or group_name(path) != name:
                raise ValueError("Invalid asset group member: " + path)
    if "shared" not in groups or not 0 < len(rows_for(lock)) < 100000:
        raise ValueError("Invalid resource file set")
    for name, group in groups.items():
        requires = group.get("requires", [])
        if (not isinstance(requires, list) or len(requires) != len(set(requires))
                or not set(requires) <= groups.keys() or name in requires
                or (name != "shared" and "shared" not in requires)):
            raise ValueError("Invalid resource group dependencies")
    for row in lock.get("git_files", []):
        if not in_git(canonical(row["path"])):
            raise ValueError("Invalid retained Git resource")
    for row in rows_for(lock) + lock.get("git_files", []):
        path = row["path"]
        if (path.casefold() in seen or type(row.get("bytes")) is not int or row["bytes"] < 0
                or not re.fullmatch(r"[a-f0-9]{64}", row.get("sha256", ""))):
            raise ValueError("Invalid or repeated resource identity: " + path)
        seen.add(path.casefold())
    actual = content_hash({"groups": lock["groups"], "git_files": lock["git_files"]})
    if actual != lock.get("source_sha256") or lock["tag"] != "assets-" + actual[:20]:
        raise ValueError("Resource snapshot identity differs from its lock")
    return lock


def load_lock():
    return validate_lock(json.loads((ROOT / LOCK).read_bytes()))


def selected_groups(lock, products=None):
    if not products:
        return lock["groups"]
    groups = {g["name"]: g for g in lock["groups"]}
    selected = {"shared"} | {"product-" + str(p) for p in products}
    if not selected <= groups.keys():
        raise ValueError("Unknown asset PID: " + ", ".join(sorted(selected - groups.keys())))
    while True:
        before = set(selected)
        for name in before:
            selected.update(groups[name]["requires"])
        if selected == before:
            return [g for g in lock["groups"] if g["name"] in selected]


def verify(directory=None, check_local=False):
    lock = load_lock()
    directory = Path(directory).resolve() if directory else ROOT / OUTPUT / lock["tag"]
    if (directory / "release-index.json").read_bytes() != encoded(published_manifest(lock)):
        raise ValueError("Snapshot index differs from the checked-in lock")
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        results = list(executor.map(lambda r: matches(r, directory), rows_for(lock)))
    if not all(results):
        raise ValueError("Snapshot source SHA-256 or length differs")
    if check_local:
        groups, retained = inventory()
        if content_hash({"groups": groups, "git_files": retained}) != lock["source_sha256"]:
            raise ValueError("Local asset sources differ from the snapshot")
    return {"tag": lock["tag"], "source_files_verified": len(results),
            "local_sources_verified": check_local, "passed": True}


def git(*args, cwd=None):
    env = dict(os.environ, GIT_LFS_SKIP_SMUDGE="1", GIT_TERMINAL_PROMPT="0", GCM_INTERACTIVE="never")
    result = subprocess.run(["git", *args], cwd=cwd or ROOT, env=env, capture_output=True,
                            text=True, encoding="utf-8", errors="replace")
    if result.returncode:
        message = re.sub(r"hf_[A-Za-z0-9]+", "[redacted]", result.stderr.strip() or result.stdout.strip())
        raise ValueError("Git resource operation failed: " + message[-5000:])
    return result.stdout


def hf_headers():
    token = os.environ.get("HF_TOKEN")
    if not token:
        env = dict(os.environ, GIT_TERMINAL_PROMPT="0", GCM_INTERACTIVE="never")
        result = subprocess.run(["git", "credential", "fill"], cwd=ROOT, env=env,
                                input="protocol=https\nhost=huggingface.co\n\n", capture_output=True,
                                text=True, encoding="utf-8", errors="replace")
        credentials = dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)
        token = credentials.get("password")
    if not token:
        raise ValueError("Configure Hugging Face authentication in Git Credential Manager or HF_TOKEN")
    return {"Authorization": "Bearer " + token, "User-Agent": "razer-ui-asset-maintenance"}


def remote_tree(repository, revision):
    url = f"https://huggingface.co/api/datasets/{repository}/tree/{revision}?recursive=true&limit=1000"
    headers, files, pages = hf_headers(), {}, set()
    while url:
        if (urllib.parse.urlsplit(url).netloc != "huggingface.co" or not url.startswith("https://")
                or url in pages):
            raise ValueError("Unexpected HF pagination URL")
        pages.add(url)
        with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=120) as response:
            data = json.load(response)
            link = response.headers.get("Link", "")
        for item in data:
            if item["type"] == "file":
                if item["path"] in files:
                    raise ValueError("Repeated remote tree file")
                files[item["path"]] = item
        next_link = re.search(r'<([^>]+)>;\s*rel="next"', link)
        url = next_link[1] if next_link else None
    return files


def verify_remote_files(lock, tree):
    expected = {r["path"]: r for r in rows_for(lock)}
    if tree.keys() != expected.keys() | META_FILES:
        raise ValueError("Remote snapshot file set differs from its lock")
    for name, row in expected.items():
        actual, lfs = tree[name], tree[name].get("lfs")
        if actual.get("size") != row["bytes"]:
            raise ValueError("Remote resource length differs: " + name)
        if lfs:
            if lfs.get("oid") != row["sha256"] or lfs.get("size") != row["bytes"]:
                raise ValueError("Remote LFS SHA-256 differs: " + name)
        elif row["bytes"] != 0 or row["sha256"] != hashlib.sha256(b"").hexdigest():
            raise ValueError("Remote resource is not stored in Git LFS: " + name)


def verify_remote(lock=None):
    lock = lock or load_lock()
    storage = lock["storage"]
    if not storage.get("revision"):
        raise ValueError("Resource snapshot has no verified immutable HF commit")
    tree = remote_tree(storage["repository"], storage["revision"])
    verify_remote_files(lock, tree)
    raw_expected = encoded(published_manifest(lock))
    url = f"https://huggingface.co/datasets/{storage['repository']}/resolve/{storage['revision']}/release-index.json"
    with urllib.request.urlopen(urllib.request.Request(url, headers=hf_headers()), timeout=120) as response:
        raw = response.read(len(raw_expected) + 1)
    if raw != raw_expected:
        raise ValueError("Remote manifest differs from the checked-in lock")
    return {"repository": storage["repository"], "revision": storage["revision"],
            "source_files_verified": len(rows_for(lock)), "passed": True}


def checkout(lock, groups):
    storage = lock["storage"]
    if not storage.get("revision"):
        raise ValueError("Snapshot is not pinned; use --from-dir for offline raw files")
    directory = ROOT / OUTPUT / "downloads" / storage["revision"]
    directory.parent.mkdir(parents=True, exist_ok=True)
    url = "https://huggingface.co/datasets/" + storage["repository"]
    if not directory.exists():
        git("clone", "--no-checkout", url, str(directory))
    if git("remote", "get-url", "origin", cwd=directory).strip() != url:
        raise ValueError("Resource cache has an unexpected Git origin")
    git("config", "core.longpaths", "true", cwd=directory)
    git("lfs", "install", "--local", cwd=directory)
    git("fetch", "--depth=1", "origin", storage["revision"], cwd=directory)
    git("-c", "core.autocrlf=false", "checkout", "--detach", "FETCH_HEAD", cwd=directory)
    if (directory / "release-index.json").read_bytes() != encoded(published_manifest(lock)):
        raise ValueError("Downloaded snapshot manifest differs")
    if len(groups) == len(lock["groups"]):
        git("lfs", "fetch", "origin", storage["revision"], "--include=assets/**", "--exclude=", cwd=directory)
    else:
        git("lfs", "fetch", "origin", storage["revision"], "--include=assets/**",
            "--exclude=assets/synapse/products/**", cwd=directory)
        patterns = ["assets/synapse/products/" + g["name"].removeprefix("product-") + "/**"
                    for g in groups if g["name"] != "shared"]
        for offset in range(0, len(patterns), 100):
            git("lfs", "fetch", "origin", storage["revision"], "--include=" + ",".join(patterns[offset:offset + 100]),
                "--exclude=", cwd=directory)
    git("lfs", "checkout", cwd=directory)
    return directory


def restore(products=None, directory=None, replace=False):
    lock = load_lock()
    groups = selected_groups(lock, products)
    for row in lock["git_files"]:
        if not matches(row):
            raise ValueError("Use the matching checkout for this snapshot: " + row["path"])
    work = ROOT / ".work"
    work.mkdir(parents=True, exist_ok=True)
    restored, unchanged = 0, 0
    with analysis.index_lock(work / "resource-writer.lock"), tempfile.TemporaryDirectory(prefix="asset-restore-", dir=work) as temp:
        changes = []
        for row in rows_for(lock, groups):
            if matches(row):
                unchanged += 1
                continue
            target = source_path(row["path"])
            if target.exists() and not target.is_file():
                raise ValueError("Asset destination is not a regular file: " + row["path"])
            if target.exists() and not replace:
                raise ValueError("Local asset was edited; use --replace-existing to restore it: " + row["path"])
            changes.append(row)
        cache = Path(directory).resolve() if directory else checkout(lock, groups) if changes else None
        stage = Path(temp) / "new"
        if cache:
            if (cache / "release-index.json").read_bytes() != encoded(published_manifest(lock)):
                raise ValueError("Snapshot index differs from the checked-in lock")
            for row in changes:
                copy_verified(row, cache, stage)
        recovery_root = work / "asset-recovery"
        recovery_root.mkdir(parents=True, exist_ok=True)
        backups, applied, cleanup_backups = None, [], False
        try:
            if changes:
                backups = Path(tempfile.mkdtemp(prefix="restore-", dir=recovery_root))
            for row in changes:
                target = source_path(row["path"])
                if target.exists() and not target.is_file():
                    raise ValueError("Asset destination changed into a non-file: " + row["path"])
                target.parent.mkdir(parents=True, exist_ok=True)
                backup = source_path(row["path"], backups)
                had_old = target.exists()
                if had_old:
                    backup.parent.mkdir(parents=True, exist_ok=True)
                    os.replace(target, backup)
                applied.append((target, backup if had_old else None))
                os.replace(source_path(row["path"], stage), target)
                restored += 1
            cleanup_backups = True
        except BaseException as original:
            failures = []
            for target, backup in reversed(applied):
                try:
                    if backup:
                        os.replace(backup, target)
                    elif target.exists():
                        target.unlink()
                except BaseException as error:
                    failures.append(f"{target.relative_to(ROOT).as_posix()}: {type(error).__name__}")
            if failures:
                raise RuntimeError("Asset rollback needs recovery; backups preserved at "
                                   + backups.relative_to(ROOT).as_posix() + "; " + "; ".join(failures)) from original
            cleanup_backups = True
            raise
        finally:
            if backups and cleanup_backups:
                checked = backups.resolve()
                if checked == recovery_root.resolve() or not checked.is_relative_to(recovery_root.resolve()):
                    raise ValueError("Recovery cleanup escaped its dedicated directory")
                shutil.rmtree(checked)
    return {"tag": lock["tag"], "groups": sorted(g["name"] for g in groups),
            "restored_files": restored, "unchanged_files": unchanged,
            "scope": "source hydration; full workspace compilation and pack preparation require all products", "passed": True}


def publish():
    lock = load_lock()
    directory = ROOT / OUTPUT / lock["tag"]
    verify(directory, check_local=True)
    if lock["storage"].get("revision"):
        return verify_remote(lock)
    uploads = ROOT / OUTPUT / "uploads"
    uploads.mkdir(parents=True, exist_ok=True)
    upload = Path(tempfile.mkdtemp(prefix=lock["tag"] + "-", dir=uploads))
    url = "https://huggingface.co/datasets/" + lock["storage"]["repository"]
    git("clone", url, str(upload))
    git("lfs", "install", "--local", cwd=upload)
    git("config", "core.autocrlf", "false", cwd=upload)
    git("config", "core.longpaths", "true", cwd=upload)
    old_rows = []
    if (upload / "release-index.json").is_file():
        previous = json.loads((upload / "release-index.json").read_bytes())
        previous["storage"] = dict(lock["storage"], revision=None)
        old_rows = rows_for(validate_lock(previous))
    existing = set(git("ls-files", cwd=upload).splitlines())
    if not existing <= {r["path"] for r in old_rows} | META_FILES:
        raise ValueError("HF repository has unowned files; refusing to replace its content")
    expected = {r["path"] for r in rows_for(lock)}
    for row in old_rows:
        if row["path"] not in expected:
            source_path(row["path"], upload).unlink()
    for row in rows_for(lock):
        target = source_path(row["path"], upload)
        if target.exists():
            target.unlink()  # This is an isolated owned scratch clone, never the source checkout.
        copy_verified(row, directory, upload)
    for name in META_FILES:
        shutil.copyfile(directory / name, upload / name)
    git("add", "--force", "--all", "--", "assets", *sorted(META_FILES), cwd=upload)
    if set(git("ls-files", "-z", cwd=upload).strip("\0").split("\0")) != expected | META_FILES:
        raise ValueError("Staged HF file set differs from the verified snapshot")
    if git("status", "--porcelain", cwd=upload).strip():
        git("-c", "user.name=Razer UI asset maintenance", "-c", "user.email=assets@local.invalid",
            "commit", "-m", lock["tag"], cwd=upload)
    revision = git("rev-parse", "HEAD", cwd=upload).strip()
    print(json.dumps({"phase": "upload", "files": len(expected),
                      "bytes": sum(r["bytes"] for r in rows_for(lock))}), flush=True)
    git("push", "origin", "HEAD:refs/heads/main", cwd=upload)
    lock["storage"]["revision"] = revision
    print(json.dumps({"phase": "verify_remote", "revision": revision}), flush=True)
    report = verify_remote(lock)
    analysis.save(ROOT / LOCK, lock)
    return report


def add_commands(commands):
    p = commands.add_parser("prepare-asset-release", help="Snapshot raw resource files and SHA-256 identities")
    p.add_argument("--repository", help="Hugging Face owner/dataset; defaults to the existing lock")
    p = commands.add_parser("verify-asset-release", help="Verify every raw snapshot file against its lock")
    p.add_argument("--from-dir", help="Offline raw snapshot directory")
    p.add_argument("--check-local", action="store_true")
    p.add_argument("--remote", action="store_true", help="Verify pinned remote LFS identities and manifest")
    p = commands.add_parser("fetch-asset-release", help="Restore raw resources from pinned HF Git LFS or offline files")
    p.add_argument("--product", type=int, action="append", help="PID plus shared/dependency sources; omit for all")
    p.add_argument("--from-dir", help="Prepared raw snapshot directory; no network")
    p.add_argument("--replace-existing", action="store_true", help="Explicitly replace modified resource files")
    commands.add_parser("publish-asset-release", help="Push raw files with Git LFS, verify and pin the HF commit")


def run(args):
    if args.command == "prepare-asset-release":
        return prepare(args.repository)
    if args.command == "verify-asset-release":
        return verify_remote() if args.remote else verify(args.from_dir, args.check_local)
    if args.command == "fetch-asset-release":
        return restore(args.product, args.from_dir, args.replace_existing)
    if args.command == "publish-asset-release":
        return publish()
    raise ValueError("Unknown asset snapshot command")

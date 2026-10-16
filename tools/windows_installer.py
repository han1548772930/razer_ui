"""Explicit Windows installer inputs; tools/workspace.py owns the CLI.

Preparation and PE/dependency inspection never execute application code. The
separate compile command invokes an existing Inno Setup compiler, never Setup.
"""
from __future__ import annotations

import hashlib
import json
import mmap
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tempfile
import tomllib
import resource_database

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = "target/installer/windows"
SCRIPT = "packaging/windows/RazerUI.iss"
APP_NAME = "razer_ui.exe"
MARKER_NAME = "razer-ui-installation.json"
ICO_NAME = "Razer UI.ico"
MACHINES = {"x64": 0x8664, "arm64": 0xAA64}
ICON_INPUTS = (
    (16, "a64b8a64b721fb74-tray-native.rgba"),
    (20, "f979021796dfd342-tray-app.rgba"),
    (24, "81851a9004a803d3-tray-icon-24.rgba"),
    (32, "71b354f660adb5db-tray-icon.rgba"),
    (40, "f8fd980e3bec82fe-tray-icon-40.rgba"),
    (48, "5e5df98271550aca-tray-icon-48.rgba"),
    (64, "afb1350c23ce2416-tray-icon-64.rgba"),
)
# OS-owned imports only. VC/UCRT redistributable *wrapper* DLLs such as
# vcruntime140.dll and msvcp140.dll must be supplied explicitly instead.
SYSTEM_DLLS = frozenset("""
advapi32.dll avrt.dll bcrypt.dll bcryptprimitives.dll cfgmgr32.dll comctl32.dll
comdlg32.dll crypt32.dll cryptbase.dll cryptsp.dll d2d1.dll d3d9.dll d3d11.dll
d3d12.dll d3dcompiler_47.dll dbghelp.dll dcomp.dll dnsapi.dll dsound.dll dwmapi.dll
dwrite.dll dxcore.dll dxgi.dll gdi32.dll gdiplus.dll hid.dll icuin.dll icuuc.dll imm32.dll iphlpapi.dll
kernel32.dll kernelbase.dll ksuser.dll mf.dll mfplat.dll mfreadwrite.dll mfuuid.dll
msacm32.dll msvcrt.dll mswsock.dll ncrypt.dll netapi32.dll ntdll.dll ole32.dll
oleacc.dll oleaut32.dll opengl32.dll powrprof.dll propsys.dll psapi.dll rasapi32.dll
rpcrt4.dll secur32.dll setupapi.dll shcore.dll shell32.dll shlwapi.dll sspicli.dll
ucrtbase.dll uiautomationcore.dll user32.dll userenv.dll usp10.dll uxtheme.dll version.dll winhttp.dll
wininet.dll winmm.dll winspool.drv wintrust.dll wldap32.dll ws2_32.dll wtsapi32.dll
""".split())


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def plain_path(path):
    """Reject links/junctions at all existing ancestors, before file writes."""
    path = Path(os.path.abspath(path))
    for current in (path, *path.parents):
        try:
            info = current.lstat()
        except FileNotFoundError:
            continue
        if current.is_symlink() or getattr(info, "st_file_attributes", 0) & 0x400:
            raise ValueError("Installer path traverses a link: " + str(current))
    return path


def output_path(path):
    path = plain_path(path)
    base = plain_path(ROOT / OUTPUT)
    if path == base or not path.is_relative_to(base):
        raise ValueError("Installer staging must be a child of " + OUTPUT)
    return path


def numeric_version(version=None):
    if version is None:
        version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    if not re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", version):
        raise ValueError("Installer version must be a numeric x.y.z version")
    if any(int(part) > 65535 for part in version.split(".")):
        raise ValueError("Installer version fields must fit Windows version resources")
    return version


def pe_info(path):
    """Parse PE32+ imports, including delay imports, without loading the image."""
    path = plain_path(path)
    if not path.is_file() or path.stat().st_size < 64:
        raise ValueError("Missing or truncated PE input: " + str(path))
    with path.open("rb") as stream, mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ) as raw:
        size = len(raw)

        def unpack(fmt, offset):
            if offset < 0 or offset + struct.calcsize(fmt) > size:
                raise ValueError("Truncated PE structures: " + path.name)
            return struct.unpack_from(fmt, raw, offset)

        if raw[:2] != b"MZ":
            raise ValueError("Not a Windows PE input: " + path.name)
        pe = unpack("<I", 0x3C)[0]
        if pe < 64 or raw[pe:pe + 4] != b"PE\0\0":
            raise ValueError("Invalid PE signature: " + path.name)
        machine, sections, _, _, _, optional_size, flags = unpack("<HHIIIHH", pe + 4)
        optional = pe + 24
        if optional_size < 112 or unpack("<H", optional)[0] != 0x20B:
            raise ValueError("Only PE32+ x64/arm64 inputs are supported")
        image_base = unpack("<Q", optional + 24)[0]
        headers = unpack("<I", optional + 60)[0]
        subsystem = unpack("<H", optional + 68)[0]
        directories = unpack("<I", optional + 108)[0]
        if sections == 0 or sections > 96 or optional_size < 112 + min(directories, 16) * 8:
            raise ValueError("Invalid PE directory/section layout")
        section_rows = []
        for index in range(sections):
            header = optional + optional_size + index * 40
            virtual_size, virtual_address, raw_size, raw_start = unpack("<IIII", header + 8)
            section_rows.append((virtual_address, virtual_size, raw_start, raw_size))

        def file_offset(rva, needed=1):
            if 0 <= rva < min(headers, size) and rva + needed <= min(headers, size):
                return rva
            for address, virtual_size, start, length in section_rows:
                delta = rva - address
                if 0 <= delta and delta + needed <= length and start + delta + needed <= size:
                    return start + delta
            raise ValueError("Unmapped PE import RVA: " + path.name)

        def dll_name(rva):
            start = file_offset(rva)
            end = raw.find(b"\0", start, min(size, start + 260))
            if end < 0:
                raise ValueError("Unterminated PE import name")
            file_offset(rva, end - start + 1)
            name = raw[start:end].decode("ascii").lower()
            if not re.fullmatch(r"[a-z0-9_.-]+\.(dll|drv)", name):
                raise ValueError("Invalid PE import name: " + repr(name))
            return name

        imports = set()
        for index, entry_size in ((1, 20), (13, 32)):
            if index >= directories:
                continue
            rva, length = unpack("<II", optional + 112 + index * 8)
            if not rva and not length:
                continue
            if not rva or length < entry_size or length > 16 * 1024 * 1024:
                raise ValueError("Invalid PE import directory size")
            terminated = False
            for delta in range(0, length - entry_size + 1, entry_size):
                row = unpack("<" + "I" * (entry_size // 4), file_offset(rva + delta, entry_size))
                if not any(row):
                    terminated = True
                    break
                if index == 1:
                    name_rva = row[3]
                else:
                    if row[0] not in (0, 1):
                        raise ValueError("Unsupported PE delay-import attributes")
                    name_rva = row[1] if row[0] & 1 else row[1] - image_base
                imports.add(dll_name(name_rva))
            if not terminated:
                raise ValueError("Unterminated PE import descriptor table")
        return {"machine": machine, "subsystem": subsystem, "dll": bool(flags & 0x2000),
                "imports": sorted(imports)}


def check_closure(images, architecture):
    if architecture not in MACHINES:
        raise ValueError("Installer architecture must be x64 or arm64")
    names = {name.casefold() for name in images}
    for name, image in images.items():
        if image["machine"] != MACHINES[architecture]:
            raise ValueError("PE architecture mismatch: " + name)
        if (name == APP_NAME and image["dll"]) or (name != APP_NAME and not image["dll"]):
            raise ValueError("Unexpected executable/DLL kind: " + name)
        if name == APP_NAME and image["subsystem"] != 2:
            raise ValueError("Package input must be a Windows GUI executable, not a console/debug build")
        unresolved = [dependency for dependency in image["imports"]
                      if dependency not in names and dependency not in SYSTEM_DLLS
                      and not dependency.startswith(("api-ms-win-", "ext-ms-win-"))]
        if unresolved:
            raise ValueError("Missing explicitly supplied runtime DLLs for " + name + ": " + ", ".join(unresolved))
    reachable, pending = {APP_NAME}, [APP_NAME]
    indexed = {name.casefold(): name for name in images}
    while pending:
        for dependency in images[pending.pop()]["imports"]:
            name = indexed.get(dependency)
            if name and name not in reachable:
                reachable.add(name)
                pending.append(name)
    if reachable != set(images):
        raise ValueError("Unnecessary DLLs are not installer inputs: " + ", ".join(sorted(set(images) - reachable)))


def icon_bytes():
    """Wrap retained, unmodified RGBA variants in standard ICO/DIB frames."""
    frames = []
    sources = []
    for pixels, name in ICON_INPUTS:
        source = ROOT / "assets/synapse/shared/pending-source-audit" / name
        raw = plain_path(source).read_bytes()
        if len(raw) != pixels * pixels * 4:
            raise ValueError("Incorrect native icon RGBA length: " + name)
        bitmap = bytearray()
        mask_stride = ((pixels + 31) // 32) * 4
        mask = bytearray()
        for y in reversed(range(pixels)):
            mask_row = bytearray(mask_stride)
            for x in range(pixels):
                r, g, b, a = raw[(y * pixels + x) * 4:(y * pixels + x + 1) * 4]
                bitmap.extend((b, g, r, a))
                if a == 0:
                    mask_row[x // 8] |= 0x80 >> (x % 8)
            mask.extend(mask_row)
        header = struct.pack("<IiiHHIIiiII", 40, pixels, pixels * 2, 1, 32, 0, len(bitmap), 0, 0, 0, 0)
        frames.append((pixels, header + bitmap + mask))
        sources.append({"path": source.relative_to(ROOT).as_posix(), "bytes": len(raw),
                        "sha256": hashlib.sha256(raw).hexdigest()})
    offset = 6 + 16 * len(frames)
    directory = bytearray(struct.pack("<HHH", 0, 1, len(frames)))
    for pixels, frame in frames:
        directory.extend(struct.pack("<BBBBHHII", pixels, pixels, 0, 0, 1, 32, len(frame), offset))
        offset += len(frame)
    return bytes(directory) + b"".join(frame for _, frame in frames), sources


def runtime_include(names):
    # Names are validated basenames; no source path or wildcard comes from users.
    return "".join('Source: "{#PayloadDir}\\' + name + '"; DestDir: "{app}"; Flags: ignoreversion\n'
                   for name in sorted(names, key=str.casefold)).encode("utf-8")


def prepare(executable=None, runtime_dlls=(), architecture="x64", version=None, output=None):
    version = numeric_version(version)
    executable = plain_path(ROOT / (executable or "target/release/razer_ui.exe"))
    if not executable.is_file():
        raise ValueError("Missing prebuilt release executable; preparation does not build or run it: " + str(executable))
    sources = {APP_NAME: executable}
    for dll in runtime_dlls:
        source = plain_path(ROOT / dll)
        if (not re.fullmatch(r"[A-Za-z0-9_.-]+\.dll", source.name, flags=re.I)
                or source.name.casefold() in {name.casefold() for name in sources}):
            raise ValueError("Runtime DLL basename is invalid or duplicated: " + source.name)
        if source.name.casefold() in SYSTEM_DLLS or source.name.lower().startswith(("api-ms-win-", "ext-ms-win-")):
            raise ValueError("Do not redistribute Windows system DLLs: " + source.name)
        sources[source.name] = source
    images = {name: pe_info(source) for name, source in sources.items()}
    check_closure(images, architecture)
    icon, icon_sources = icon_bytes()
    policy = resource_database.installation_policy(ROOT)
    catalog = plain_path(ROOT / policy["catalog_source"])
    if not catalog.is_file():
        raise ValueError("Missing packaged resource catalog")
    marker = {"source": "windows-inno", "version": version, "architecture": architecture,
              "executable": APP_NAME, "cache_relative": policy["cache_relative"],
              "catalog_filename": policy["catalog_filename"]}
    destination = output_path(ROOT / (output or f"{OUTPUT}/{version}-{architecture}"))
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=".prepare-", dir=destination.parent))
    try:
        payload = temporary / "payload"
        payload.mkdir()
        for name, source in sources.items():
            shutil.copyfile(source, payload / name)
            if digest(source) != digest(payload / name):
                raise ValueError("Installer input changed while copying: " + name)
        (payload / ICO_NAME).write_bytes(icon)
        (payload / MARKER_NAME).write_bytes(encoded(marker))
        shutil.copyfile(catalog, payload / policy["catalog_filename"])
        if digest(catalog) != digest(payload / policy["catalog_filename"]):
            raise ValueError("Resource catalog changed while preparing installer")
        include = temporary / "runtime-files.iss"
        include.write_bytes(runtime_include(set(sources) - {APP_NAME}))
        rows = [{"path": p.relative_to(temporary).as_posix(), "bytes": p.stat().st_size, "sha256": digest(p)}
                for p in sorted(payload.iterdir())] + [{"path": include.name, "bytes": include.stat().st_size,
                                                        "sha256": digest(include)}]
        receipt = {"schema": 1, "version": version, "architecture": architecture,
                   "script": {"path": SCRIPT, "sha256": digest(ROOT / SCRIPT)}, "inputs": rows,
                   "native_icons": icon_sources, "pe_images": images,
                   "resource_payloads_included": False, "cache_relative": policy["cache_relative"],
                   "catalog_filename": policy["catalog_filename"],
                   "installation_policy_sha256": digest(ROOT / resource_database.INSTALLATION)}
        (temporary / "installer-input.json").write_bytes(encoded(receipt))
        if destination.exists():
            current = verify(destination)
            if (destination / "installer-input.json").read_bytes() != encoded(receipt):
                raise ValueError("Existing installer stage differs; choose a new --output directory")
            return current
        temporary.rename(destination)
    finally:
        if temporary.exists():
            # Owned mkdtemp path, checked against the intended output root.
            output_path(temporary)
            shutil.rmtree(temporary)
    return verify(destination)


def verify(stage):
    stage = output_path(ROOT / stage)
    receipt_path = plain_path(stage / "installer-input.json")
    receipt = json.loads(receipt_path.read_bytes())
    if not isinstance(receipt.get("version"), str):
        raise ValueError("Installer receipt has no numeric version")
    version = numeric_version(receipt.get("version"))
    architecture = receipt.get("architecture")
    script = receipt.get("script")
    if receipt.get("schema") != 1 or script != {"path": SCRIPT, "sha256": digest(ROOT / SCRIPT)}:
        raise ValueError("Installer recipe changed or receipt is invalid; prepare a new stage")
    images = receipt.get("pe_images", {})
    policy = resource_database.installation_policy(ROOT)
    if (APP_NAME not in images or receipt.get("cache_relative") != policy["cache_relative"]
            or receipt.get("resource_payloads_included") is not False
            or receipt.get("catalog_filename") != policy["catalog_filename"]
            or receipt.get("installation_policy_sha256") != digest(ROOT / resource_database.INSTALLATION)):
        raise ValueError("Invalid installer image/cache policy")
    allowed_payload = {APP_NAME, ICO_NAME, MARKER_NAME, policy["catalog_filename"]}
    for name in images:
        if name != APP_NAME:
            if not re.fullmatch(r"[A-Za-z0-9_.-]+\.dll", name, flags=re.I):
                raise ValueError("Unexpected installer payload: " + name)
            allowed_payload.add(name)
    if len({name.casefold() for name in allowed_payload}) != len(allowed_payload):
        raise ValueError("Case-colliding installer payload")
    expected = {"payload/" + name for name in allowed_payload} | {"runtime-files.iss"}
    rows = receipt.get("inputs", [])
    if len(rows) != len(expected) or {row.get("path") for row in rows} != expected:
        raise ValueError("Installer input allowlist differs from its receipt")
    actual = set()
    for current, directories, files in os.walk(stage, followlinks=False):
        for name in directories + files:
            path = plain_path(Path(current) / name)
            if path.is_file():
                actual.add(path.relative_to(stage).as_posix())
            elif path != stage / "payload":
                raise ValueError("Unexpected directory in installer stage")
    if actual != expected | {"installer-input.json"}:
        raise ValueError("Unowned files in installer stage; no assets/ or pack is allowed")
    for row in rows:
        path = plain_path(stage / row["path"])
        if not path.is_file() or path.stat().st_size != row["bytes"] or digest(path) != row["sha256"]:
            raise ValueError("Installer input bytes changed: " + row["path"])
    marker = json.loads((stage / "payload" / MARKER_NAME).read_bytes())
    if marker != {"source": "windows-inno", "version": version, "architecture": architecture,
                  "executable": APP_NAME, "cache_relative": policy["cache_relative"],
                  "catalog_filename": policy["catalog_filename"]}:
        raise ValueError("Installer ownership marker differs from the payload")
    if (stage / "runtime-files.iss").read_bytes() != runtime_include(set(images) - {APP_NAME}):
        raise ValueError("Installer DLL include differs from its allowlist")
    if digest(stage / "payload" / policy["catalog_filename"]) != digest(ROOT / policy["catalog_source"]):
        raise ValueError("Installer resource catalog differs from maintained database")
    icon, icon_sources = icon_bytes()
    if (stage / "payload" / ICO_NAME).read_bytes() != icon or receipt.get("native_icons") != icon_sources:
        raise ValueError("Installer icon differs from retained native icon bytes")
    current_images = {name: pe_info(stage / "payload" / name) for name in images}
    if current_images != images:
        raise ValueError("Installer PE imports changed")
    check_closure(current_images, architecture)
    return {"stage": stage.relative_to(ROOT).as_posix(), "version": version, "architecture": architecture,
            "payload_files": len(allowed_payload), "resource_payloads_included": False,
            "cache_relative": policy["cache_relative"], "catalog_filename": policy["catalog_filename"], "passed": True,
            "scope": "static installer inputs only; application and installer not executed"}


def compile_installer(stage, compiler=None):
    report = verify(stage)
    stage = ROOT / report["stage"]
    compiler = compiler or shutil.which("ISCC.exe")
    if not compiler or not Path(compiler).is_file():
        raise ValueError("Inno Setup >= 6.3 ISCC.exe is required; no compiler is downloaded or installed automatically")
    setup_output = output_path(stage.with_name(stage.name + "-compiled"))
    if setup_output.exists():
        raise ValueError("Installer output already exists; choose a fresh prepared stage")
    # ISCC is a compiler, not the generated installer. Never invoke its output.
    command = [str(compiler), "/Qp", "/DPayloadDir=" + str(stage / "payload"),
               "/DAppVersion=" + report["version"], "/DAppArch=" + report["architecture"],
               "/DCatalogFilename=" + report["catalog_filename"],
               "/DCacheRelative=" + report["cache_relative"].replace("/", "\\"),
               "/DOutputRoot=" + str(setup_output), str(ROOT / SCRIPT)]
    result = subprocess.run(command, cwd=ROOT, check=False)
    if result.returncode:
        raise ValueError("Inno Setup compilation failed with exit code " + str(result.returncode))
    output = setup_output / f"Razer-UI-{report['version']}-{report['architecture']}-Setup.exe"
    if not plain_path(output).is_file():
        raise ValueError("Inno Setup did not produce the expected installer")
    return {**report, "installer": output.relative_to(ROOT).as_posix(),
            "bytes": output.stat().st_size, "sha256": digest(output),
            "scope": "installer compilation only; installation and runtime acceptance unverified"}


def add_commands(commands):
    p = commands.add_parser("prepare-windows-installer", help="Stage exact prebuilt EXE/runtime DLLs; never build or run")
    p.add_argument("--exe", help="Prebuilt Windows GUI EXE; defaults to target/release/razer_ui.exe")
    p.add_argument("--runtime-dll", action="append", default=[], help="Explicit app-local DLL; repeat for dependency closure")
    p.add_argument("--architecture", choices=tuple(MACHINES), default="x64")
    p.add_argument("--version", help="Numeric x.y.z; defaults to Cargo package version")
    p.add_argument("--output", help="Fresh staging child of target/installer/windows/")
    p = commands.add_parser("verify-windows-installer", help="Statically verify staged installer inputs and PE imports")
    p.add_argument("--stage", required=True)
    p = commands.add_parser("compile-windows-installer", help="Compile with existing ISCC; never launch Setup")
    p.add_argument("--stage", required=True)
    p.add_argument("--iscc", help="Existing Inno Setup >= 6.3 ISCC.exe")


def run(args):
    if args.command == "prepare-windows-installer":
        return prepare(args.exe, args.runtime_dll, args.architecture, args.version, args.output)
    if args.command == "verify-windows-installer":
        return verify(args.stage)
    if args.command == "compile-windows-installer":
        return compile_installer(args.stage, args.iscc)
    raise ValueError("Unknown Windows installer command")

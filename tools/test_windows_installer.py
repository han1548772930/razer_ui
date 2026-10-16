"""Inert PE fixtures: packaging safety and closure, without executing binaries."""
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

import windows_installer as installer


def pe_bytes(*, imports=(), delay_imports=(), dll=False, machine=0x8664, subsystem=2):
    raw = bytearray(0x800)
    raw[:2] = b"MZ"
    struct.pack_into("<I", raw, 0x3C, 0x80)
    raw[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HHIIIHH", raw, 0x84, machine, 1, 0, 0, 0, 240, 0x2022 if dll else 0x22)
    optional = 0x98
    struct.pack_into("<H", raw, optional, 0x20B)
    struct.pack_into("<Q", raw, optional + 24, 0x140000000)
    struct.pack_into("<I", raw, optional + 60, 0x200)
    struct.pack_into("<H", raw, optional + 68, subsystem)
    struct.pack_into("<I", raw, optional + 108, 16)
    struct.pack_into("<IIII", raw, optional + 240 + 8, 0x600, 0x1000, 0x600, 0x200)
    name_at = 0x500
    for index, names, table, entry_size in ((1, imports, 0x200, 20), (13, delay_imports, 0x380, 32)):
        if not names:
            continue
        struct.pack_into("<II", raw, optional + 112 + index * 8, table + 0xE00, (len(names) + 1) * entry_size)
        for row, name in enumerate(names):
            name_bytes = name.encode("ascii") + b"\0"
            raw[name_at:name_at + len(name_bytes)] = name_bytes
            if index == 1:
                struct.pack_into("<IIIII", raw, table + row * entry_size, 1, 0, 0, name_at + 0xE00, 1)
            else:
                struct.pack_into("<IIIIIIII", raw, table + row * entry_size, 1, name_at + 0xE00, 0, 1, 0, 0, 0, 0)
            name_at += len(name_bytes)
    return bytes(raw)


class WindowsInstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.root_patch = patch.object(installer, "ROOT", self.root)
        self.root_patch.start()
        self.write("Cargo.toml", b'[package]\nversion = "0.1.0"\n')
        self.write(installer.SCRIPT, b"; inert fixture recipe\n")
        self.write(installer.resource_database.INSTALLATION, json.dumps({
            "schema_version": 1, "catalog_source": "assets/synapse/resource-catalog.sqlite3",
            "catalog_filename": "resource-catalog.sqlite3", "cache_relative": "assets/cache",
        }).encode())
        self.write("assets/synapse/resource-catalog.sqlite3", b"inert catalog fixture")
        for pixels, name in installer.ICON_INPUTS:
            # Distinct coordinates exercise bottom-up rows and RGBA -> BGRA.
            raw = bytes(component for y in range(pixels) for x in range(pixels)
                        for component in (x, y, 31, 0 if x == 0 else 255))
            self.write("assets/synapse/shared/pending-source-audit/" + name, raw)

    def tearDown(self):
        self.root_patch.stop()
        self.temporary.cleanup()

    def write(self, name, raw):
        destination = self.root / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(raw)
        return destination

    def stage(self, imports=("kernel32.dll",), runtime_dlls=()):
        self.write("target/release/razer_ui.exe", pe_bytes(imports=imports))
        return installer.prepare(runtime_dlls=runtime_dlls)

    def test_import_and_delay_import_dependencies_are_both_required(self):
        exe = self.write("razer_ui.exe", pe_bytes(imports=("kernel32.dll",), delay_imports=("vcruntime140.dll",)))
        info = installer.pe_info(exe)
        self.assertEqual(info["imports"], ["kernel32.dll", "vcruntime140.dll"])
        with self.assertRaisesRegex(ValueError, "vcruntime140.dll"):
            installer.check_closure({installer.APP_NAME: info}, "x64")

    def test_explicit_runtime_dependency_is_verified_and_only_payload_is_staged(self):
        self.write("runtime/vcruntime140.dll", pe_bytes(dll=True, imports=("kernel32.dll",)))
        result = self.stage(imports=("kernel32.dll", "vcruntime140.dll"), runtime_dlls=("runtime/vcruntime140.dll",))
        self.assertTrue(result["passed"])
        self.assertEqual(result["payload_files"], 5)
        stage = self.root / result["stage"]
        self.assertEqual({p.name for p in (stage / "payload").iterdir()},
                         {installer.APP_NAME, installer.ICO_NAME, installer.MARKER_NAME, "vcruntime140.dll", "resource-catalog.sqlite3"})
        self.assertEqual(installer.prepare(runtime_dlls=("runtime/vcruntime140.dll",)), result)

    def test_resource_or_pack_injection_is_rejected_before_compilation(self):
        result = self.stage()
        self.write(result["stage"] + "/payload/synapse.pack", b"must not ship")
        with self.assertRaisesRegex(ValueError, "Unowned files"):
            installer.verify(result["stage"])

    def test_changed_payload_and_changed_recipe_cannot_reuse_receipt(self):
        result = self.stage()
        self.write(result["stage"] + "/payload/razer_ui.exe", pe_bytes(imports=("user32.dll",)))
        with self.assertRaisesRegex(ValueError, "bytes changed"):
            installer.verify(result["stage"])
        self.write(installer.SCRIPT, b"changed recipe")
        with self.assertRaisesRegex(ValueError, "recipe changed"):
            installer.verify(result["stage"])

    def test_include_tampering_cannot_add_arbitrary_sources_even_with_updated_receipt(self):
        result = self.stage()
        stage = self.root / result["stage"]
        injected = b'Source: "assets\\*"; DestDir: "{app}"; Flags: recursesubdirs\n'
        (stage / "runtime-files.iss").write_bytes(injected)
        receipt = json.loads((stage / "installer-input.json").read_bytes())
        row = next(row for row in receipt["inputs"] if row["path"] == "runtime-files.iss")
        row.update(bytes=len(injected), sha256=hashlib.sha256(injected).hexdigest())
        (stage / "installer-input.json").write_bytes(installer.encoded(receipt))
        with self.assertRaisesRegex(ValueError, "DLL include differs"):
            installer.verify(result["stage"])

    def test_wrong_architecture_console_and_unnecessary_dlls_fail(self):
        info = installer.pe_info(self.write("razer_ui.exe", pe_bytes(machine=0xAA64)))
        with self.assertRaisesRegex(ValueError, "architecture mismatch"):
            installer.check_closure({installer.APP_NAME: info}, "x64")
        info = installer.pe_info(self.write("razer_ui.exe", pe_bytes(subsystem=3)))
        with self.assertRaisesRegex(ValueError, "Windows GUI"):
            installer.check_closure({installer.APP_NAME: info}, "x64")
        self.write("runtime/unused.dll", pe_bytes(dll=True))
        with self.assertRaisesRegex(ValueError, "Unnecessary DLLs"):
            self.stage(runtime_dlls=("runtime/unused.dll",))

    def test_malformed_pe_and_missing_release_fail_without_execution(self):
        with self.assertRaisesRegex(ValueError, "Missing prebuilt"):
            installer.prepare()
        source = self.write("bad.exe", b"MZ" + b"\0" * 100)
        with self.assertRaisesRegex(ValueError, "Invalid PE signature"):
            installer.pe_info(source)
        raw = bytearray(pe_bytes(imports=("kernel32.dll",)))
        struct.pack_into("<I", raw, 0x200 + 12, 0xFFFFFF00)
        with self.assertRaisesRegex(ValueError, "Unmapped PE import"):
            installer.pe_info(self.write("bad.exe", raw))

    def test_output_is_confined_and_linked_parents_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "staging must be a child"):
            installer.output_path(self.root / "target/release")
        link = self.root / "target/installer/windows/linked"
        link.parent.mkdir(parents=True)
        other = self.root / "other"
        other.mkdir()
        try:
            link.symlink_to(other, target_is_directory=True)
        except OSError:
            # Windows may restrict symlink creation; exercise the same lstat
            # result that junctions expose without mutating external paths.
            with patch.object(installer.Path, "lstat") as lstat:
                lstat.return_value.st_file_attributes = 0x400
                lstat.return_value.st_mode = 0o040755
                with self.assertRaisesRegex(ValueError, "traverses a link"):
                    installer.output_path(link / "stage")
        else:
            with self.assertRaisesRegex(ValueError, "traverses a link"):
                installer.output_path(link / "stage")

    def test_ico_pixels_preserve_original_rgba_and_transparent_mask(self):
        raw, sources = installer.icon_bytes()
        self.assertEqual(struct.unpack_from("<HHH", raw), (0, 1, len(installer.ICON_INPUTS)))
        self.assertEqual(len(sources), len(installer.ICON_INPUTS))
        width, height, _, _, _, bits, size, offset = struct.unpack_from("<BBBBHHII", raw, 6)
        self.assertEqual((width, height, bits), (16, 16, 32))
        self.assertEqual(raw[offset + 40:offset + 48], bytes((31, 15, 0, 0, 31, 15, 1, 255)))
        self.assertEqual(raw[offset + 40 + 16 * 16 * 4], 0x80)
        self.assertEqual(size, 40 + 16 * 16 * 4 + 16 * 4)


if __name__ == "__main__":
    unittest.main()

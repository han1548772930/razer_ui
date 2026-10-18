"""Pure byte-level tests for static PE routing; no target is loaded or executed."""
from __future__ import annotations

import struct
import unittest

from host import inspect_pe, pe_cursor_groups, pe_resource_entries


def pe_fixture(bits=64):
    raw = bytearray(0x1000)
    raw[:2] = b"MZ"
    struct.pack_into("<I", raw, 0x3c, 0x80)
    raw[0x80:0x84] = b"PE\0\0"
    size = 240 if bits == 64 else 224
    struct.pack_into("<HHIIIHH", raw, 0x84, 0x8664 if bits == 64 else 0x14c, 1, 0, 0, 0, size, 0x2102)
    optional = 0x98
    struct.pack_into("<H", raw, optional, 0x20b if bits == 64 else 0x10b)
    struct.pack_into("<Q" if bits == 64 else "<I", raw, optional + (24 if bits == 64 else 28), 0x180000000 if bits == 64 else 0x10000000)
    struct.pack_into("<I", raw, optional + 60, 0x200)
    directory = optional + (112 if bits == 64 else 96)
    struct.pack_into("<I", raw, directory - 4, 16)
    struct.pack_into("<8sIIII", raw, optional + size, b".rdata\0\0", 0xe00, 0x1000, 0xe00, 0x200)
    return raw, directory


def cursor_fixture():
    raw, directory = pe_fixture()
    struct.pack_into("<II", raw, directory + 16, 0x1200, 0x400)

    def node(offset, entries):
        base = 0x400 + offset
        struct.pack_into("<IIHHHH", raw, base, 0, 0, 0, 0, 0, len(entries))
        for index, entry in enumerate(entries):
            struct.pack_into("<II", raw, base + 16 + index * 8, *entry)

    node(0, [(1, 0x80000030), (12, 0x80000060)])
    node(0x30, [(7, 0x80000090)])
    node(0x60, [(102, 0x800000b0)])
    node(0x90, [(1033, 0xd0)])
    node(0xb0, [(1033, 0xe0)])
    dib = struct.pack("<IiiHHIIiiII", 40, 2, 4, 1, 32, 0, 0, 0, 0, 0, 0) + bytes(range(16)) + b"\x80\0\0\0" * 2
    cursor = struct.pack("<HH", 1, 0) + dib
    group = struct.pack("<HHHHHHHIH", 0, 2, 1, 2, 4, 1, 32, len(cursor), 7)
    struct.pack_into("<IIII", raw, 0x4d0, 0x1400, len(cursor), 0, 0)
    struct.pack_into("<IIII", raw, 0x4e0, 0x1500, len(group), 0, 0)
    raw[0x600:0x600 + len(cursor)] = cursor
    raw[0x700:0x700 + len(group)] = group
    return raw, dib


class StaticPeTests(unittest.TestCase):
    def test_architecture_and_native_classification_come_from_headers(self):
        for bits, machine in ((32, "x86"), (64, "x64")):
            with self.subTest(bits=bits):
                raw, _ = pe_fixture(bits)
                result = inspect_pe(raw)
                self.assertEqual(result["kind"], "native")
                self.assertEqual(result["architecture"], machine)
                self.assertEqual(result["unresolved"], [])

    def test_import_name_and_ordinal_are_decoded_with_pointer_width(self):
        for bits in (32, 64):
            with self.subTest(bits=bits):
                raw, directory = pe_fixture(bits)
                struct.pack_into("<II", raw, directory + 8, 0x1100, 40)
                struct.pack_into("<IIIII", raw, 0x300, 0x1180, 0, 0, 0x1160, 0x1180)
                raw[0x360:0x36d] = b"KERNEL32.dll\0"
                struct.pack_into("<QQQ" if bits == 64 else "<III", raw, 0x380, 0x11c0, (1 << (bits - 1)) | 9, 0)
                raw[0x3c2:0x3ca] = b"WinExec\0"
                result = inspect_pe(raw)
                self.assertEqual(result["imports"], [{"library": "KERNEL32.dll", "symbols": [{"name": "WinExec"}, {"ordinal": 9}]}])

    def test_exports_distinguish_code_rva_and_forwarder(self):
        raw, directory = pe_fixture()
        struct.pack_into("<II", raw, directory, 0x1200, 0x100)
        struct.pack_into("<IIHHIIIIIII", raw, 0x400, 0, 0, 0, 0, 0, 1, 2, 1, 0x1250, 0x1260, 0x1270)
        struct.pack_into("<II", raw, 0x450, 0x1700, 0x1280)
        struct.pack_into("<I", raw, 0x460, 0x1290)
        struct.pack_into("<H", raw, 0x470, 0)
        raw[0x480:0x48d] = b"OTHER.Target\0"
        raw[0x490:0x49a] = b"DoRestart\0"
        exports = inspect_pe(raw)["exports"]
        self.assertEqual(exports[0], {"ordinal": 1, "names": ["DoRestart"], "rva": "0x1700"})
        self.assertEqual(exports[1]["forwarder"], "OTHER.Target")

    def test_clr_and_ready_to_run_do_not_enter_pure_native_queue(self):
        for flags, ready, expected in ((1, False, "managed"), (0, False, "mixed_managed_native"), (0xc, True, "managed")):
            with self.subTest(flags=flags, ready=ready):
                raw, directory = pe_fixture()
                struct.pack_into("<II", raw, directory + 14 * 8, 0x1400, 72)
                struct.pack_into("<IHHIIII", raw, 0x600, 72, 2, 5, 0x1460, 0x40, flags, 0)
                raw[0x660:0x664] = b"BSJB"
                if ready:
                    struct.pack_into("<II", raw, 0x640, 0x1500, 16)
                    struct.pack_into("<IHHII", raw, 0x700, 0x00525452, 9, 1, 0x20, 5)
                result = inspect_pe(raw)
                self.assertEqual(result["kind"], expected)
                self.assertEqual("ready_to_run" in result["clr"], ready)

    def test_bad_clr_metadata_is_rejected_instead_of_routed_as_native(self):
        raw, directory = pe_fixture()
        struct.pack_into("<II", raw, directory + 14 * 8, 0x1400, 72)
        struct.pack_into("<IHHIIII", raw, 0x600, 72, 2, 5, 0x1460, 0x40, 1, 0)
        with self.assertRaisesRegex(ValueError, "valid metadata"):
            inspect_pe(raw)

    def test_invalid_import_rva_remains_explicit(self):
        raw, directory = pe_fixture()
        struct.pack_into("<II", raw, directory + 8, 0xffff0000, 40)
        result = inspect_pe(raw)
        self.assertEqual(result["unresolved"][0]["stage"], "imports")
        self.assertEqual(result["imports"], [])

    def test_truncated_and_non_pe_inputs(self):
        self.assertEqual(inspect_pe(b"\x7fELF"), {"format": "elf", "kind": "other"})
        with self.assertRaisesRegex(ValueError, "exceeds input"):
            inspect_pe(b"MZ")


class StaticCursorTests(unittest.TestCase):
    def test_reconstruction_preserves_dib_mask_hotspot_and_resource_identity(self):
        raw, dib = cursor_fixture()
        leaves = pe_resource_entries(raw)
        self.assertEqual([(row["type"], row["name"], row["language"]) for row in leaves], [(1, 7, 1033), (12, 102, 1033)])
        result, = pe_cursor_groups(raw, 102)
        self.assertEqual(result["data"][:22], struct.pack("<HHHBBBBHHII", 0, 2, 1, 2, 2, 0, 0, 1, 0, len(dib), 22))
        self.assertEqual(result["data"][22:], dib)
        self.assertEqual(result["frames"][0]["group_height"], 4)
        self.assertEqual(result["frames"][0]["original"], leaves[0])

    def test_missing_or_foreign_language_frame_is_rejected(self):
        raw, _ = cursor_fixture()
        struct.pack_into("<I", raw, 0x4a0, 1031)
        with self.assertRaisesRegex(ValueError, "language/identifier"):
            pe_cursor_groups(raw, 102)

    def test_hotspot_and_truncated_and_mask_are_rejected(self):
        raw, _ = cursor_fixture()
        struct.pack_into("<H", raw, 0x600, 2)
        with self.assertRaisesRegex(ValueError, "hotspot"):
            pe_cursor_groups(raw, 102)
        raw, _ = cursor_fixture()
        struct.pack_into("<I", raw, 0x4d4, 64)
        struct.pack_into("<I", raw, 0x70e, 64)
        with self.assertRaisesRegex(ValueError, "mask length"):
            pe_cursor_groups(raw, 102)

    def test_cyclic_and_out_of_directory_references_are_rejected(self):
        raw, _ = cursor_fixture()
        struct.pack_into("<I", raw, 0x414, 0x80000000)
        with self.assertRaisesRegex(ValueError, "cyclic"):
            pe_resource_entries(raw)
        raw, _ = cursor_fixture()
        struct.pack_into("<I", raw, 0x4a4, 0x10000)
        with self.assertRaisesRegex(ValueError, "declared bounds"):
            pe_resource_entries(raw)


if __name__ == "__main__":
    unittest.main()

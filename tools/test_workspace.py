"""Pure byte tests for analyzer wheel identity and extraction boundaries."""
import base64
import csv
import hashlib
import io
import unittest
import warnings
import zipfile
import copy
import json
from pathlib import Path
import sqlite3
import tempfile

import workspace
import resource_database
import families


def wheel_bytes(members, *, records=None, extra=()):
    record_name = "analyzer-1.dist-info/RECORD"
    if records is None:
        records = [(name, "sha256=" + base64.urlsafe_b64encode(hashlib.sha256(raw).digest()).rstrip(b"=").decode(), str(len(raw)))
                   for name, raw in members]
        records.append((record_name, "", ""))
    table = io.StringIO()
    csv.writer(table).writerows(records)
    raw = io.BytesIO()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", UserWarning)
        with zipfile.ZipFile(raw, "w") as archive:
            for name, content in [*members, *extra, (record_name, table.getvalue().encode())]:
                archive.writestr(name, content)
    return raw.getvalue(), record_name


class AnalyzerWheelTests(unittest.TestCase):
    def verify(self, raw, record):
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            return workspace.verified_wheel_members(archive, record)

    def test_complete_member_hashes_and_lengths_verify(self):
        raw, record = wheel_bytes([("analyzer/module.py", b"inert source"), ("analyzer/native.pyd", b"inert binary")])
        self.assertEqual(self.verify(raw, record), ["analyzer/module.py", "analyzer/native.pyd", record])

    def test_archive_member_missing_from_record_is_rejected(self):
        raw, record = wheel_bytes([("analyzer/module.py", b"source")], extra=[("analyzer/hidden.pyd", b"unlisted")])
        with self.assertRaisesRegex(ValueError, "cover every"):
            self.verify(raw, record)

    def test_record_cannot_accept_mutated_member(self):
        raw, record = wheel_bytes([("analyzer/module.py", b"altered")], records=[
            ("analyzer/module.py", "sha256=" + base64.urlsafe_b64encode(hashlib.sha256(b"original").digest()).rstrip(b"=").decode(), "8"),
            ("analyzer-1.dist-info/RECORD", "", "")])
        with self.assertRaisesRegex(ValueError, "does not verify"):
            self.verify(raw, record)

    def test_unsigned_payload_is_rejected(self):
        raw, record = wheel_bytes([("analyzer/native.pyd", b"payload")], records=[
            ("analyzer/native.pyd", "", ""), ("analyzer-1.dist-info/RECORD", "", "")])
        with self.assertRaisesRegex(ValueError, "lacks a RECORD identity"):
            self.verify(raw, record)

    def test_unsafe_paths_are_rejected_before_any_publication(self):
        for name in ("../escape", "/absolute", "C:/absolute", "folder\\escape"):
            with self.subTest(name=name):
                raw, record = wheel_bytes([(name, b"inert")])
                with self.assertRaisesRegex(ValueError, "escaped"):
                    self.verify(raw, record)

    def test_duplicate_archive_members_are_rejected(self):
        raw, record = wheel_bytes([("analyzer/module.py", b"first"), ("analyzer/module.py", b"second")])
        with self.assertRaisesRegex(ValueError, "duplicate members"):
            self.verify(raw, record)


class ProductRegistryTests(unittest.TestCase):
    def records(self):
        return {"schema_version": 1, "products": [{"id": 756, "name": "Keyboard", "categories": ["KEYBOARD"],
            "edition_ids": [0, 128], "navigations": [
                {"key": "keyboard-default", "display_mode": "default", "primary": True, "pages": [
                    {"key": "keyboard-calibration", "kind": "TAB_CALIBRATION", "role": "Page", "help_selector": 10},
                    {"key": "keyboard-help", "kind": "HELP", "role": "Help", "help_selector": 11}]},
                {"key": "keyboard-ble", "display_mode": "default", "primary": False, "pages": [
                    {"key": "keyboard-ble-help", "kind": "HELP", "role": "Help", "help_selector": 21}]}]}]}

    def load(self, document):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "registry.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            return resource_database.product_registry(path)

    def test_complete_connection_branches_and_help_selectors_roundtrip_through_sql(self):
        products = self.load(self.records())
        with sqlite3.connect(":memory:") as connection:
            connection.execute("PRAGMA foreign_keys=ON")
            connection.executescript((resource_database.ROOT / resource_database.SCHEMA).read_text(encoding="utf-8"))
            counts = resource_database.fill_product_registry(connection, products)
            self.assertEqual(counts["ui_page"], 3)
            self.assertEqual(connection.execute("SELECT key, display_mode, is_primary FROM ui_navigation ORDER BY position").fetchall(),
                             [("keyboard-default", "default", 1), ("keyboard-ble", "default", 0)])
            self.assertEqual(connection.execute("SELECT key, help_selector FROM ui_page WHERE role='Help' ORDER BY help_selector").fetchall(),
                             [("keyboard-help", 11), ("keyboard-ble-help", 21)])
            with self.assertRaises(sqlite3.IntegrityError):
                connection.execute("INSERT INTO ui_page VALUES ('orphan', 'missing-navigation', 0, 'HELP', 'Help', 1)")

    def test_source_reaudit_retains_help_identity_when_source_positions_change(self):
        previous = self.records()["products"][0]["navigations"]
        current = [{"source": {"path": "new-current.chunk.js"}, "branch": "wired", "offset": 1000,
                    "display_mode": "default", "primary": True,
                    "pages": [{"kind": "TAB_CALIBRATION", "offset": 1000}, {"kind": "HELP", "offset": 2000}]},
                   {"source": {"path": "new-current.chunk.js"}, "branch": "ble", "offset": 3000,
                    "display_mode": "default", "primary": False, "pages": [{"kind": "HELP", "offset": 4000}]}]
        product = families.registry_product_record(756, "Keyboard", "KEYBOARD", [0, 128], current, previous)
        self.assertEqual(product, self.records()["products"][0])
        bindings = [{"product_id": 756, "pages": [{"offset": 11}, {"offset": 21}]}]
        self.assertEqual(resource_database.validate_product_help_bindings([product], bindings), 2)
        product["navigations"][1]["pages"][0]["help_selector"] = 11
        bindings[0]["pages"] = [{"offset": 21}]
        with self.assertRaisesRegex(ValueError, "no local presentation record"):
            resource_database.validate_product_help_bindings([product], bindings)

    def test_help_binding_rejects_ambiguous_presentation_records(self):
        products = self.records()["products"]
        for records in ([{"product_id": 756, "pages": [{"offset": 11}, {"offset": 11}]}],
                        [{"product_id": 756, "pages": []}, {"product_id": 756, "pages": []}]):
            with self.subTest(records=records):
                with self.assertRaisesRegex(ValueError, "Ambiguous"):
                    resource_database.validate_product_help_bindings(products, records)

    def test_duplicate_stable_ids_missing_primary_unknown_fields_and_bad_editions_fail(self):
        cases = []
        duplicate = self.records()
        duplicate["products"][0]["navigations"][1]["pages"][0]["key"] = "keyboard-help"
        cases.append(duplicate)
        no_primary = self.records()
        no_primary["products"][0]["navigations"][0]["primary"] = False
        cases.append(no_primary)
        unknown = self.records()
        unknown["products"][0]["navigations"][0]["source"] = ".ref/never-a-runtime-input"
        cases.append(unknown)
        editions = self.records()
        editions["products"][0]["edition_ids"] = [0, True]
        cases.append(editions)
        unsorted = self.records()
        unsorted["products"].append(copy.deepcopy(unsorted["products"][0]))
        unsorted["products"][1]["id"] = 12
        cases.append(unsorted)
        for document in cases:
            with self.subTest(document=document):
                with self.assertRaises(ValueError):
                    self.load(document)


class FrontendClosureTests(unittest.TestCase):
    def test_declared_resources_maps_and_exploratory_probes_remain_separate(self):
        document = {"files": [{"url": "https://official/main.js", "path": "original.js"},
                              {"url": "https://official/main.js.map", "error": "404"},
                              {"url": "https://official/favicon.svg", "error": "404"},
                              {"url": "https://official/asset-manifest.json", "error": "404"},
                              {"url": "https://official/unknown.bin", "error": "404"}],
                    "edges": [{"from": "https://official/main.js", "to": "https://official/main.js.map", "kind": "source-map"},
                              {"from": "https://official/index.html", "to": "https://official/favicon.svg", "kind": "html:link"}],
                    "probes": [{"url": "https://official/asset-manifest.json", "declared": False}],
                    "unresolved": [{"kind": "chunk_unresolved", "asset": {"template": "[id].js"}}]}
        gaps = families.frontend_reference_gaps(document)
        self.assertEqual(gaps["declared_source_maps_missing"], 1)
        self.assertEqual([row["url"] for row in gaps["declared_non_map_missing"]], ["https://official/favicon.svg"])
        self.assertEqual([row["url"] for row in gaps["undeclared_probes_unavailable"]], ["https://official/asset-manifest.json"])
        self.assertEqual([row["url"] for row in gaps["unattributed_non_map_missing"]], ["https://official/unknown.bin"])
        self.assertFalse(gaps["all_declared_non_map_references_retained"])
        self.assertEqual(gaps["unresolved_by_kind"], {"chunk_unresolved": 1})

    def test_opaque_original_keeps_parameter_reuse_free_exports_and_unicode_byte_ranges(self):
        for body in ('21:function(e,t,n){e.exports=t=function(e,t){return n(e,t)};t.methods=exports}',
                     '21(e,t,n){n.d(t,{A:()=>exports});e=n.hmd(e)}'):
            source = 'const label="原始";const modules={' + body + '};'
            module = next(row for row in families.keyboard_source_modules(source) if row["module_id"] == 21)
            capture = families.webpack_factory_snapshot(module)
            raw = source.encode("utf-8")
            self.assertEqual(capture["body"], body)
            self.assertEqual(capture["parameters"], "(e,t,n)")
            self.assertEqual(raw[capture["byte_start"]:capture["byte_end"]], body.encode())
            self.assertEqual(raw[capture["statements_byte_start"]:capture["statements_byte_end"]], capture["statements"].encode())
            self.assertIn("exports", capture["statements"])


if __name__ == "__main__":
    unittest.main()

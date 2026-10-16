"""Pure targeted-publication planning tests; no network or resource hydration."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import asset_release as assets


class AssetAdditionTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name).resolve()
        self.root_patch = patch.object(assets, "ROOT", self.root)
        self.root_patch.start()
        base = {"path": "assets/synapse/shared/base.svg", "bytes": 4, "sha256": hashlib.sha256(b"base").hexdigest()}
        lock = {"schema_version": 2, "format": "files", "git_files": [],
                "groups": [{"name": "product-1392", "requires": ["shared"], "files": []},
                           {"name": "shared", "requires": [], "files": [base]}],
                "storage": {"provider": "huggingface", "repository": "owner/assets", "revision": "0" * 40}}
        lock["source_sha256"] = assets.content_hash({"groups": lock["groups"], "git_files": lock["git_files"]})
        lock["tag"] = "assets-" + lock["source_sha256"][:20]
        self.write(assets.LOCK, lock)
        controls = [{"key": "synapse/audio-demo-control-" + name + ".svg", "path": base["path"]}
                    for name in ("play", "pause", "fullscreen", "fullscreen-exit")]
        alias = "synapse/base.svg"
        selectors = {}
        for name, columns in assets.SELECTOR_FIELDS.items():
            selectors[name] = [{column: (alias if column == "key" else "Product" if column == "part"
                                        else "prd" if column == "kind" else False if column == "wrist"
                                        else 1 if column in ("product_id", "density") else 0)
                               for column in columns}]
        self.write(assets.MAPPINGS, {"schema_version": 1,
            "tables": [{"name": "ASSETS", "entries": [{"key": alias, "path": base["path"]}]}],
            "audio_demo_controls": controls, "audio_volume_sprite": {"key": "synapse/sprite.svg", "path": base["path"]},
            "selectors": selectors})
        raw = b'<svg xmlns="http://www.w3.org/2000/svg"/>'
        source_path = self.root / ".analysis/inputs/frontend/original.svg"
        source_path.parent.mkdir(parents=True)
        source_path.write_bytes(raw)
        source = {"path": source_path.relative_to(self.root).as_posix(), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        receipt_path = self.write(".analysis/inputs/frontend/acquisition.json", {"files": [source]})
        receipt = {"path": receipt_path.relative_to(self.root).as_posix(), "bytes": receipt_path.stat().st_size,
                   "sha256": assets.digest(receipt_path)}
        self.item = {"path": "assets/synapse/products/1392/drag.svg", "source": source,
                     "acquisition": receipt, "alias": "synapse/drag.svg", "table": "ASSETS"}
        self.manifest = ".analysis/inputs/resource-publication.json"

    def tearDown(self):
        self.root_patch.stop()
        self.directory.cleanup()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value), encoding="utf-8")
        return path

    def plan(self, rows=None):
        self.write(self.manifest, {"schema_version": 1, "files": rows or [self.item]})
        return assets.release_additions(self.manifest)

    def test_verified_addition_preserves_existing_lock_and_does_not_restore_assets(self):
        before = (self.root / assets.LOCK).read_bytes()
        old, lock, mappings, rows, _ = self.plan()
        self.assertEqual((self.root / assets.LOCK).read_bytes(), before)
        self.assertFalse((self.root / "assets").exists())
        self.assertNotEqual(old["tag"], lock["tag"])
        self.assertEqual(len(assets.rows_for(lock)), len(assets.rows_for(old)) + 1)
        self.assertIn({"key": "synapse/drag.svg", "path": self.item["path"]}, mappings["tables"][0]["entries"])
        self.assertEqual(rows[0]["asset"]["sha256"], self.item["source"]["sha256"])

    def test_source_change_is_rejected_before_lock_or_mapping_updates(self):
        (self.root / self.item["source"]["path"]).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "identity differs"):
            self.plan()

    def test_source_must_be_declared_by_the_verified_acquisition(self):
        path = self.write(self.item["acquisition"]["path"], {"files": []})
        self.item["acquisition"].update(bytes=path.stat().st_size, sha256=assets.digest(path))
        with self.assertRaisesRegex(ValueError, "not declared"):
            self.plan()

    def test_replacing_existing_resource_bytes_is_rejected(self):
        self.item["path"] = "assets/synapse/shared/base.svg"
        with self.assertRaisesRegex(ValueError, "cannot replace existing resource bytes"):
            self.plan()

    def test_duplicate_addition_paths_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "repeats"):
            self.plan([self.item, self.item])

    def test_alias_rebinding_is_rejected(self):
        self.item["alias"] = "synapse/base.svg"
        with self.assertRaisesRegex(ValueError, "replace an existing alias"):
            self.plan()


if __name__ == "__main__":
    unittest.main()

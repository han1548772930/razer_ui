"""Pure targeted-publication planning tests; no network or resource hydration."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import asset_release as assets


class OwnedUploadCloneTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name).resolve()
        self.root_patch = patch.object(assets, "ROOT", self.root)
        self.root_patch.start()

    def tearDown(self):
        self.root_patch.stop()
        self.directory.cleanup()

    def test_owned_clone_is_released_on_success_and_on_failure(self):
        with assets._owned_upload_clone("assets-" + "a" * 20) as normal:
            (normal / "payload").write_bytes(b"private copy")
        self.assertFalse(normal.exists())
        with self.assertRaisesRegex(RuntimeError, "clone failed"):
            with assets._owned_upload_clone("assets-" + "a" * 20) as failed:
                (failed / "payload").write_bytes(b"partial clone")
                raise RuntimeError("clone failed")
        self.assertFalse(failed.exists())

    def test_current_verification_checkout_is_retained(self):
        with assets._owned_upload_clone("assets-" + "b" * 20) as current:
            report = self.root / ".analysis/results/asset-release-update.json"
            report.parent.mkdir(parents=True)
            report.write_text(json.dumps({"verification_checkout": current.relative_to(self.root).as_posix()}), encoding="utf-8")
        self.assertTrue(current.is_dir())

    def test_readonly_git_pack_is_cleaned_with_its_owned_clone(self):
        import stat
        with assets._owned_upload_clone("assets-" + "e" * 20) as target:
            pack = target / ".git/objects/pack/private.pack"
            pack.parent.mkdir(parents=True)
            pack.write_bytes(b"immutable private Git pack")
            pack.chmod(stat.S_IREAD)
        self.assertFalse(target.exists())

    def test_cleanup_cannot_target_the_user_workspace_or_an_outside_directory(self):
        for path in (self.root, self.root / ".work", self.root.parent / "outside"):
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "outside"):
                assets._checked_upload_tree(path)
        self.assertTrue(self.root.is_dir())

    def test_replaced_owned_directory_is_retained_instead_of_recursively_deleted(self):
        with self.assertRaisesRegex(ValueError, "changed"):
            with assets._owned_upload_clone("assets-" + "c" * 20) as target:
                target.rename(target.with_name(target.name + "-original"))
                target.mkdir()
                (target / "keep").write_bytes(b"another owner")
        self.assertEqual((target / "keep").read_bytes(), b"another owner")

    def test_reparse_guard_rejects_the_owned_tree_before_cleanup(self):
        with self.assertRaisesRegex(ValueError, "reparse"):
            with assets._owned_upload_clone("assets-" + "d" * 20) as target:
                original = Path.is_junction
                with patch.object(Path, "is_junction", lambda path: path == target or original(path)):
                    assets._checked_upload_tree(target)


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

    def test_publication_source_index_requires_pinned_files_and_keeps_manifest_identity(self):
        _, lock, mappings, _, _ = self.plan()
        self.write(assets.LOCK, lock)
        self.write(assets.MAPPINGS, mappings)
        index = self.write(".analysis/inputs.json", {"inputs": []})
        before = (self.root / self.manifest).read_bytes()
        with patch.object(assets.analysis, "ROOT", self.root), patch.object(assets, "git", side_effect=AssertionError("No remote publication")):
            assets.index_addition_sources(self.manifest, reason="Verified offline reconciliation")
        document = json.loads(index.read_bytes())
        self.assertEqual(len(document["inputs"]), 3)
        manifest = next(row for row in document["inputs"] if row["path"] == self.manifest)
        self.assertEqual(manifest["sha256"], hashlib.sha256(before).hexdigest())
        self.assertEqual((self.root / self.manifest).read_bytes(), before)
        self.assertTrue(document["source_publications"][0]["verification"]["source_receipt_membership_verified"])

    def test_source_index_cannot_publish_unreleased_resource_additions(self):
        self.plan()
        with self.assertRaisesRegex(ValueError, "requires additions in the pinned"):
            assets.index_addition_sources(self.manifest, reason="Must fail")

    def test_prepared_native_resource_does_not_become_an_official_frontend_original(self):
        native_name = ".analysis/inputs/native-resources/cursor.cur"
        raw = (self.root / self.item["source"]["path"]).read_bytes()
        native = self.root / native_name
        native.parent.mkdir(parents=True)
        native.write_bytes(raw)
        self.item["source"]["path"] = native_name
        receipt = self.write(".analysis/inputs/native-resources/acquisition.json", {"files": [self.item["source"]]})
        self.item["acquisition"] = {"path": receipt.relative_to(self.root).as_posix(),
                                    "bytes": receipt.stat().st_size, "sha256": assets.digest(receipt)}
        _, lock, mappings, _, _ = self.plan()
        self.write(assets.LOCK, lock)
        self.write(assets.MAPPINGS, mappings)
        index = self.write(".analysis/inputs.json", {"inputs": []})
        with patch.object(assets.analysis, "ROOT", self.root):
            assets.index_addition_sources(self.manifest, reason="Verify native provenance")
        row = next(row for row in json.loads(index.read_bytes())["inputs"] if row["path"] == native_name)
        self.assertEqual(row["kind"], "prepared_native_cursor")
        self.assertEqual(row["acquisition_receipt"], self.item["acquisition"]["path"])

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

    def test_alias_reuse_verifies_existing_bytes_without_hydrating_or_mutating_files(self):
        before = (self.root / assets.LOCK).read_bytes()
        result = assets.verified_alias_reuse(b"base", "synapse/base.svg")
        self.assertEqual(result["locked_file"]["sha256"], hashlib.sha256(b"base").hexdigest())
        self.assertEqual(result["transformation"], "identity")
        self.assertTrue(result["inverse_restores_original_bytes"])
        self.assertFalse((self.root / "assets").exists())
        self.assertEqual((self.root / assets.LOCK).read_bytes(), before)
        with self.assertRaisesRegex(ValueError, "differ"):
            assets.verified_alias_reuse(b"same-name-but-different-content", "synapse/base.svg")

    def test_alias_reuse_requires_exact_reversible_line_ending_conversion(self):
        lock = json.loads((self.root / assets.LOCK).read_bytes())
        original = b"first\nsecond\n"
        prepared = original.replace(b"\n", b"\r\n")
        lock["groups"][1]["files"][0].update(bytes=len(prepared), sha256=hashlib.sha256(prepared).hexdigest())
        lock["source_sha256"] = assets.content_hash({"groups": lock["groups"], "git_files": lock["git_files"]})
        lock["tag"] = "assets-" + lock["source_sha256"][:20]
        self.write(assets.LOCK, lock)
        result = assets.verified_alias_reuse(original, "synapse/base.svg", normalize_source_lf=True)
        self.assertTrue(result["inverse_restores_original_bytes"])
        self.assertEqual(result["source_lf_count"], 2)
        self.assertFalse((self.root / "assets").exists())
        with self.assertRaisesRegex(ValueError, "differ"):
            assets.verified_alias_reuse(original, "synapse/base.svg")
        with self.assertRaisesRegex(ValueError, "existing CR"):
            assets.verified_alias_reuse(prepared, "synapse/base.svg", normalize_source_lf=True)


if __name__ == "__main__":
    unittest.main()

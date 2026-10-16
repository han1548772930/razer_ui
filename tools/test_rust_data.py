"""Validate domain-table generation at parsing and interchange boundaries."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import rust_data
import resource_database
import sqlite3
from copy import deepcopy


class RustDataTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.crate = self.root / "crates/razer-fixture"
        self.module = self.crate / "src/lib.rs"
        self.module.parent.mkdir(parents=True)
        self.patch = patch.object(rust_data, "ROOT", self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)

    def schema(self, text):
        self.module.write_text(text, encoding="utf-8")
        return rust_data.RustData(self.crate)

    def test_required_fields_ranges_and_wrong_types_are_rejected(self):
        schema = self.schema('''
            #[serde(deny_unknown_fields)]
            struct Device { product_id: u16, connected: bool }
        ''')
        for invalid in ({"product_id": 65536, "connected": True},
                        {"product_id": -1, "connected": True},
                        {"product_id": True, "connected": True},
                        {"product_id": 756},
                        {"product_id": 756, "connected": "true"},
                        {"product_id": 756, "connected": True, "new_field": 1}):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                schema.render(invalid, "Device", self.module)

    def test_aliases_skip_flatten_and_associated_types_have_distinct_ownership(self):
        schema = self.schema('''
            type ProductId = u16;
            struct Device {
                #[serde(rename = "productId", alias = "pid")]
                product_id: ProductId,
                #[serde(skip)] cache: Option<String>,
                #[serde(flatten)] extra: std::collections::BTreeMap<String, String>,
            }
            impl Visitor for Device { type ProductId = String; }
        ''')
        result = schema.render({"pid": 756, "label": "Keyboard"}, "Device", self.module)
        self.assertIn("product_id:756u16", result)
        self.assertIn("cache:Default::default()", result)
        self.assertIn('("label".into(),"Keyboard".into())', result)
        self.assertFalse(schema.excluded)

    def test_missing_and_present_null_remain_distinct_and_integers_stay_integral(self):
        schema = self.schema('''
            #[serde(untagged)] enum Scalar { Decimal(f64), Integer(u64), Null(()) }
            struct Device { value: Option<Scalar> }
        ''')
        self.assertIn("value:None", schema.render({}, "Device", self.module))
        self.assertIn("Some(Scalar::Null(()))", schema.render({"value": None}, "Device", self.module))
        self.assertIn("Some(Scalar::Integer(1u64))", schema.render({"value": 1}, "Device", self.module))
        self.assertIn("Some(Scalar::Decimal(1.5f64))", schema.render({"value": 1.5}, "Device", self.module))

    def test_tagged_enum_alias_and_external_nested_types_are_resolved(self):
        other = self.root / "crates/razer-other/src/lib.rs"
        other.parent.mkdir(parents=True)
        other.write_text('struct Key { position: [u8; 2] }', encoding="utf-8")
        schema = self.schema('''
            use razer_other::Key;
            #[serde(tag = "kind", rename_all = "snake_case")]
            enum Geometry { #[serde(alias = "old_path")] Path { points: Vec<u8> } }
            struct Device { keys: Vec<Key>, geometry: Geometry }
        ''')
        result = schema.render({"keys": [{"position": [2, 4]}],
                                "geometry": {"kind": "old_path", "points": [3]}}, "Device", self.module)
        self.assertIn("razer_other::Key{position:[2u8,4u8]}", result)
        self.assertIn("Geometry::Path{points:vec![3u8]}", result)

    def test_value_requires_explicit_boundary_and_emits_no_recursive_json_macro(self):
        schema = self.schema("struct Device { payload: serde_json::Value }")
        value = {"payload": {"a": [None, -2, 3, 1.5, True, "quoted,[]{}\"\\\n"]}}
        with self.assertRaisesRegex(ValueError, "explicit external-format boundary"):
            schema.render(value, "Device", self.module)
        result = schema.render(value, "Device", self.module, allow_value=True)
        self.assertNotIn("json!", result)
        self.assertIn("Value::Number((-2i64).into())", result)
        self.assertIn("Value::Number(3u64.into())", result)
        self.assertEqual(schema.value_fields, {"$.payload"})
        with self.assertRaises(ValueError):
            rust_data.json_boundary(float("nan"))

    def test_formatting_preserves_string_tokens_and_splits_large_collections(self):
        schema = self.schema("struct Device { label: String, rows: Vec<u16> }")
        expression = schema.render({"label": 'literal,[]{}:"\\\n', "rows": list(range(100))}, "Device", self.module)
        formatted = rust_data.pretty_expression(expression, indent=8)
        self.assertIn(rust_data.rust_string('literal,[]{}:"\\\n'), formatted)
        self.assertGreater(len(formatted.splitlines()), 90)
        tree = schema.parser.parse(("fn table() -> Device {" + formatted + "}").encode())
        self.assertFalse(tree.root_node.has_error)

    def test_product_factories_reject_duplicate_stable_identities(self):
        schema = self.schema("struct Product { pid: u32 }")
        source = self.root / "products.json"
        source.write_text(json.dumps([{"pid": 756}, {"pid": 756}]), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "duplicate product ID"):
            rust_data.build_table("razer-fixture", "PRODUCTS", self.module, "Vec<Product>",
                                  source=source, schema=schema, product_factory=True)

    def test_repeated_large_subtrees_share_typed_constructors_without_losing_fields(self):
        schema = self.schema('''
            struct Row { label: String }
            struct Device { rows: Vec<Row> }
        ''')
        schema.expression_pool = {}
        value = {"rows": [{"label": "repeated" * 200}] * 30}
        expression = schema.render(value, "Device", self.module)
        self.assertEqual(expression, "source_table_3()")
        self.assertEqual(len(schema.shared_constructors), 4)
        row = schema.shared_constructors[1]
        self.assertEqual(row[1], "Row")
        self.assertEqual(schema.shared_constructors[2][2].count(row[0] + "()"), 30)
        helpers = "".join("fn " + name + "() -> " + typ + " {" + body + "}"
                          for name, typ, body in schema.shared_constructors)
        tree = schema.parser.parse(("fn table() -> Device {" + helpers + expression + "}").encode())
        self.assertFalse(tree.root_node.has_error)
        self.assertFalse(schema.excluded)
        self.assertFalse(schema.value_fields)

    def test_saving_maintenance_input_regenerates_owned_rust_and_its_receipt(self):
        import analysis
        import asset_release
        schema = self.schema("struct Product { product_id: u32 }")
        source = self.root / ".analysis/inputs/typed-tables/razer-fixture/products.json"
        source.parent.mkdir(parents=True)
        source.write_text('{"product_id":756}', encoding="utf-8")
        with patch.object(analysis, "ROOT", self.root), patch.object(asset_release, "ROOT", self.root):
            report = rust_data.build_table("razer-fixture", "PRODUCTS", self.module, "Product", source=source, schema=schema)
            output = self.root / report["output"]
            self.assertIn("756u32", output.read_text(encoding="utf-8"))
            analysis.save(source, {"product_id": 2647})
            self.assertIn("2647u32", output.read_text(encoding="utf-8"))
            updated = json.loads((self.root / "tools/typed-data.json").read_bytes())["tables"][0]
            self.assertNotEqual(updated["source_sha256"], report["source_sha256"])
            rust_data.build_table("razer-fixture", "PRODUCTS", self.module, "Product", source=source, schema=schema, check=True)


class ResourceDatabaseTests(unittest.TestCase):
    def test_packaged_catalog_is_excluded_from_remote_release_inventory(self):
        import asset_release
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, raw in ((asset_release.RUNTIME_METADATA, b"local seed"),
                              (asset_release.RUNTIME_METADATA + ".partial", b"unpublished seed"),
                              ("assets/synapse/products/70/icon.png", b"media")):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(raw)
            with patch.object(asset_release, "ROOT", root):
                groups, retained = asset_release.inventory()
            self.assertFalse(retained)
            self.assertEqual([row["path"] for group in groups for row in group["files"]],
                             ["assets/synapse/products/70/icon.png"])

    def fixture(self):
        search = [{"emo": "😀", "name": "Grinning Face"}, {"emo": "", "name": "Man"},
                  {"emo": "😀", "name": "Grinning Face"}]
        mapping = {"groups": [{"values": ["😀", "", "😀"]}], "tabs": [{"emo": "😀", "name": "Smileys"}],
                   "search": search, "variants": {"👋": ["👋🏻", "👋🏼"]}, "excluded_without_windows_10": ["😀"]}
        alternate = deepcopy(mapping)
        alternate["excluded_without_windows_10"] = []
        macro = deepcopy(mapping)
        macro["groups"][0].update(label="SMILEYS", name="TEXT_EMOJI_SMILEYS")
        macro["tabs"][0].update(name="TEXT_EMOJI_SMILEYS", pos=0)
        macro["excluded_windows_11"] = macro.pop("excluded_without_windows_10")
        first = {"path": "assets/test/first.svg", "bytes": 7, "sha256": "a" * 64}
        second = {"path": "assets/test/second.svg", "bytes": 8, "sha256": "b" * 64}
        manifest = {"schema_version": 1, "storage": {"provider": "huggingface", "repository": "owner/data", "revision": "c" * 40},
                    "entries": [{"key": "synapse/same", **first}, {"key": "synapse/same", **second, "view_box": "0 0 20 20", "width": 20, "height": 20}],
                    "list_entries": [{"key": "synapse/same", "provider": "tables"}] * 2,
                    "fonts": [first], "files": [first, second]}
        datasets = {"mapping": mapping, "mapping_without_exclusions": alternate, "macro": macro}
        connection = sqlite3.connect(":memory:")
        self.addCleanup(connection.close)
        resource_database.fill(connection, manifest, datasets, search)
        return connection, manifest, datasets, search

    def test_complete_roundtrip_keeps_duplicates_and_profile_branches(self):
        connection, manifest, datasets, search = self.fixture()
        resource_database.verify_rows(connection, manifest, datasets, search)
        self.assertEqual(connection.execute("SELECT count(*) FROM emoji_entry").fetchone()[0], 2)
        self.assertEqual(connection.execute("SELECT count(*) FROM emoji_group_list").fetchone()[0], 1)
        self.assertEqual(connection.execute("SELECT count(*) FROM emoji_variant_set").fetchone()[0], 1)
        aliases = connection.execute("SELECT key, file_id FROM resource_alias ORDER BY position").fetchall()
        self.assertEqual(aliases, [("synapse/same", 0), ("synapse/same", 1)])

    def test_changed_order_or_text_is_rejected_and_foreign_keys_are_enforced(self):
        connection, manifest, datasets, search = self.fixture()
        with self.assertRaises(sqlite3.IntegrityError):
            connection.execute("INSERT INTO emoji_search VALUES (3, 999)")
        connection.execute("UPDATE emoji_search SET entry_id=1 WHERE position=0")
        with self.assertRaisesRegex(ValueError, "order, duplicates or text"):
            resource_database.verify_rows(connection, manifest, datasets, search)


if __name__ == "__main__":
    unittest.main()

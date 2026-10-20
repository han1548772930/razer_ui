"""Validate domain-table generation at parsing and interchange boundaries."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import rust_data
import resource_database
import catalog_data
import sqlite3
from types import SimpleNamespace
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

    def test_recorded_maintenance_check_rejects_partial_or_mutating_modes(self):
        registry = self.root / "tools/typed-data.json"
        registry.parent.mkdir()
        registry.write_text('{"tables":[]}', encoding="utf-8")
        for check, crates, symbols, recover in ((False, None, None, False), (True, ["fixture"], None, False),
                                               (True, None, ["TABLE"], False), (True, None, None, True)):
            with self.subTest(check=check, crates=crates, symbols=symbols, recover=recover), \
                    self.assertRaisesRegex(ValueError, "unfiltered --check"):
                rust_data.run(SimpleNamespace(command="prepare-typed-data", record_verification=True,
                    check=check, crate=crates, symbol=symbols, recover_maintenance=recover))

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

    def test_raw_identifiers_and_static_borrowed_tables_use_the_rust_schema(self):
        schema = self.schema('''
            #[serde(rename_all = "camelCase")]
            struct Device { r#macro: &'static str, keys: &'static [u16] }
        ''')
        result = schema.render({"macro": "macro-name", "keys": [1, 256]}, "Device", self.module)
        self.assertIn('r#macro:"macro-name"', result)
        self.assertIn('keys:&[1u16,256u16]', result)

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

    def test_nested_use_and_aliases_resolve_the_actual_external_type_owner(self):
        other = self.root / "crates/razer-other/src/capability.rs"
        other.parent.mkdir(parents=True)
        other.write_text('pub struct Key { position: [u8; 2] }', encoding="utf-8")
        schema = self.schema('''
            use razer_other::{capability::{Key as SelectedKey}};
            struct Device { key: SelectedKey }
        ''')
        result = schema.render({"key": {"position": [2, 4]}}, "Device", self.module)
        self.assertIn("razer_other::capability::Key{position:[2u8,4u8]}", result)
        self.assertEqual(schema.definition('SelectedKey', self.module, {}).module, other)

    def test_local_module_import_is_not_treated_as_an_external_crate(self):
        local = self.crate / "src/geometry.rs"
        local.write_text('pub struct Point { x: u16, y: u16 }', encoding="utf-8")
        schema = self.schema('mod geometry; use geometry::Point as Position; struct Device { origin: Position }')
        expression = schema.render({"origin": {"x": 2, "y": 4}}, "Device", self.module)
        self.assertIn("origin:Position{x:2u16,y:4u16}", expression)
        self.assertEqual(schema.definition('Position', self.module, {}).module, local)

    def test_catalog_local_module_preserves_its_primitive_decoder_scope(self):
        local = self.crate / "src/geometry.rs"
        local.write_text('pub struct Point { x: u16, y: u16 }', encoding="utf-8")
        self.module.write_text('mod geometry; use geometry::Point; struct Device { version: u16, origin: Point }', encoding="utf-8")
        row = {"crate": "razer-fixture", "macro": "geometry", "root_type": "Device", "module": "crates/razer-fixture/src/lib.rs"}
        with patch.object(catalog_data, "ROOT", self.root):
            document = catalog_data.Codec(self.crate).document(row, {"version": 1, "origin": {"x": 2, "y": 4}})
        self.assertEqual(document["code"].count("anyhow::Result<u16>"), 2)

    def test_catalog_owner_move_preserves_complete_installed_codec_and_bytes(self):
        definitions = 'struct Target { product_id: u32, category: Category } enum Category { Keyboard, Mouse }'
        value = {"product_id": 756, "targets": [{"product_id": 756, "category": "Keyboard"}, {"product_id": 70, "category": "Mouse"}]}
        row = {"crate": "razer-fixture", "macro": "targets", "root_type": "SourceData",
               "module": "crates/razer-fixture/src/lib.rs"}
        self.module.write_text('struct SourceData { product_id: u32, targets: Vec<Target> } ' + definitions, encoding="utf-8")
        with patch.object(catalog_data, "ROOT", self.root):
            before = catalog_data.Codec(self.crate).document(row, value)
            other = self.root / "crates/razer-other/src/capability.rs"
            other.parent.mkdir(parents=True)
            other.write_text(definitions, encoding="utf-8")
            self.module.write_text('use razer_other::capability::{Target, Category}; struct SourceData { product_id: u32, targets: Vec<Target> }', encoding="utf-8")
            row["reader_scoped_types"] = ["Target", "Category"]
            after = catalog_data.Codec(self.crate).document(row, value)
        self.assertEqual(after["raw"], before["raw"])
        self.assertEqual(after["code"], before["code"])
        self.assertEqual(after["codec"], before["codec"])

    def test_catalog_lazy_sequence_preserves_values_and_order_with_a_lazy_reader(self):
        row = {"crate": "razer-fixture", "macro": "layouts", "root_type": "Atlas",
               "module": "crates/razer-fixture/src/lib.rs"}
        value = {"layouts": [{"id": 679}, {"id": 756}, {"id": 679}]}
        with patch.object(catalog_data, "ROOT", self.root):
            self.module.write_text('struct Layout { id: u32 } struct Atlas { layouts: Vec<Layout> }', encoding="utf-8")
            before = catalog_data.Codec(self.crate).document(row, value)
            self.module.write_text('struct Layout { id: u32 } struct Atlas { layouts: razer_resource_db::CatalogSequence<Layout> }', encoding="utf-8")
            after = catalog_data.Codec(self.crate).document(row, value)
        self.assertEqual(after["raw"], before["raw"])
        self.assertIn('r.lazy_sequence(', after["code"])
        self.assertNotIn('r.sequence(', after["code"])
        self.assertNotEqual(after["codec"], before["codec"])
        self.assertEqual(after["roots"], before["roots"])

    def test_catalog_reader_scopes_reject_invalid_missing_and_unused_imports(self):
        other = self.root / "crates/razer-other/src/lib.rs"
        other.parent.mkdir(parents=True)
        other.write_text('struct Target { product_id: u32 }', encoding="utf-8")
        self.module.write_text('use razer_other::Target; struct Device { id: u32 }', encoding="utf-8")
        row = {"crate": "razer-fixture", "macro": "device", "root_type": "Device", "module": "crates/razer-fixture/src/lib.rs"}
        for scopes in (["Target", "Target"], ["unknown"], ["Target"], ["serde_json::Value"], "Target"):
            with self.subTest(scopes=scopes), patch.object(catalog_data, "ROOT", self.root), self.assertRaises(ValueError):
                catalog_data.Codec(self.crate).document({**row, "reader_scoped_types": scopes}, {"id": 1})

    def test_catalog_publication_preserves_parallel_owner_edits_before_any_write(self):
        first, second = self.module, self.crate / "src/second.rs"
        first.write_bytes(b"first owner")
        second.write_bytes(b"second owner")
        artifacts = catalog_data.AdapterArtifacts()
        self.assertEqual(artifacts.source(first), b"first owner")
        artifacts[first] = b"first generated reader"
        artifacts[second] = b"second generated reader"
        second.write_bytes(b"second owner plus a newly declared module")
        with patch.object(catalog_data, "ROOT", self.root), self.assertRaisesRegex(ValueError, "owner changed"):
            catalog_data.publish_adapters(artifacts, {"tables": []})
        self.assertEqual(first.read_bytes(), b"first owner")
        self.assertEqual(second.read_bytes(), b"second owner plus a newly declared module")
        fresh = catalog_data.AdapterArtifacts()
        fresh[first] = b"first generated reader"
        fresh[second] = second.read_bytes()
        with patch.object(catalog_data, "ROOT", self.root):
            catalog_data.publish_adapters(fresh, {"tables": []})
        self.assertEqual(first.read_bytes(), b"first generated reader")
        self.assertEqual(second.read_bytes(), b"second owner plus a newly declared module")

    def test_catalog_external_value_import_keeps_the_explicit_boundary_codec(self):
        row = {"crate": "razer-fixture", "macro": "boundary", "root_type": "Device",
               "module": "crates/razer-fixture/src/lib.rs", "allow_value": True}
        value = {"payload": {"a": [None, -2, 3, True]}, "label": "caption"}
        with patch.object(catalog_data, "ROOT", self.root):
            self.module.write_text('struct Device { payload: serde_json::Value, label: SharedString }', encoding="utf-8")
            before = catalog_data.Codec(self.crate).document(row, value)
            self.module.write_text('use serde_json::Value; use gpui_kit::SharedString; struct Device { payload: Value, label: SharedString }', encoding="utf-8")
            after = catalog_data.Codec(self.crate).document(row, value)
        self.assertEqual(after["raw"], before["raw"])
        self.assertEqual(after["code"], before["code"])
        self.assertEqual(after["codec"], before["codec"])

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
            recipes_path = self.root / "tools/typed-data.json"
            recipes = json.loads(recipes_path.read_bytes())
            provenance = {"kind": "complete_current_javascript_literal", "ordered_rows": 1}
            recipes["tables"][0]["source_evidence"] = provenance
            analysis.save(recipes_path, recipes)
            analysis.save(source, {"product_id": 2647})
            self.assertIn("2647u32", output.read_text(encoding="utf-8"))
            updated = json.loads((self.root / "tools/typed-data.json").read_bytes())["tables"][0]
            self.assertNotEqual(updated["source_sha256"], report["source_sha256"])
            self.assertEqual(updated["source_evidence"], provenance)
            rust_data.build_table("razer-fixture", "PRODUCTS", self.module, "Product", source=source, schema=schema, check=True)


class MappingEngineGenerationTests(unittest.TestCase):
    @staticmethod
    def source():
        return {"keys": [{"name": "A", "inputID": "KEY_A", "type": "Alphanumeric", "scancode": 30,
            "analogKeyID": 31, "virtualKey": "65", "keyCode": "65", "flag": 0, "pageID": "7", "HID": "4"}],
            "default_turbo_ids": ["Test"], "turbo_events": {"Test": []},
            "constants": {"SINGLE_STATE_INPUT_ID": ["ScrollUp"], "SCROLL_INPUT_ID": ["ScrollUp"],
                          "SCROLL_TURBO_DELAY_MS": 30}}

    def test_current_schema_rejects_missing_fields_duplicate_order_and_nulls(self):
        valid = {"740": self.source(), "746": self.source()}
        tables, products = rust_data.mapping_engine_tables(valid)
        self.assertEqual(len(tables), 1)
        self.assertEqual(products, [(740, 0), (746, 0)])
        invalid = []
        row = deepcopy(valid)
        row["740"]["constants"].pop("SCROLL_INPUT_ID")
        invalid.append(row)
        row = deepcopy(valid)
        row["740"]["default_turbo_ids"].append(row["740"]["default_turbo_ids"][0])
        invalid.append(row)
        row = deepcopy(valid)
        row["740"]["keys"][0] = None
        invalid.append(row)
        row = deepcopy(valid)
        row["740"]["unknown"] = True
        invalid.append(row)
        for row in invalid:
            with self.subTest(row=row), self.assertRaises(ValueError):
                rust_data.mapping_engine_tables(row)

    def test_product_branch_is_retained_and_only_complete_equal_tables_share_storage(self):
        source = {"70": self.source(), "182": self.source()}
        tables, products = rust_data.mapping_engine_tables(source)
        self.assertEqual(products, [(70, 0), (182, 0)])
        self.assertEqual(len(tables), 1)
        for field in ("scancode", "outputFlag"):
            changed = deepcopy(source)
            changed["182"]["keys"][0][field] = 31
            tables, products = rust_data.mapping_engine_tables(changed)
            self.assertEqual(products, [(70, 0), (182, 1)])
            self.assertEqual(len(tables), 2)

    def test_complete_generated_bytes_and_summary_are_checked_for_every_product(self):
        owner = (rust_data.ROOT / rust_data.MAPPING_ENGINE_MODULE).read_bytes()
        with tempfile.TemporaryDirectory() as directory, patch.object(rust_data, "ROOT", Path(directory)):
            root = Path(directory)
            module = root / rust_data.MAPPING_ENGINE_MODULE
            module.parent.mkdir(parents=True)
            module.write_bytes(owner)
            summary = root / "crates/razer-device/testdata/re/mapping-engine-source-summary.json"
            summary.parent.mkdir(parents=True)
            source = {"70": self.source(), "182": self.source()}
            path = root / rust_data.MAPPING_ENGINE_SOURCE
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(source))
            rust_data.prepare_mapping_engine_data(source)
            self.assertEqual(rust_data.check_mapping_engine_data()["products"], [70, 182])
            for changed in ({"70": self.source()}, {"70": self.source(), "182": self.source()}):
                if "182" in changed:
                    changed["182"]["constants"]["SCROLL_TURBO_DELAY_MS"] = 31
                path.write_text(json.dumps(changed))
                with self.assertRaisesRegex(ValueError, "generated Rust differs"):
                    rust_data.check_mapping_engine_data()
            path.write_text(json.dumps(source))
            summary.write_bytes(summary.read_bytes() + b" ")
            with self.assertRaisesRegex(ValueError, "summary differs"):
                rust_data.check_mapping_engine_data()

    def test_unfiltered_maintenance_check_rejects_a_failed_specialized_mapping_table(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(rust_data, "ROOT", Path(directory)):
            root = Path(directory)
            path = root / "tools/typed-data.json"
            path.parent.mkdir()
            path.write_text(json.dumps({"tables": [{"crate": "razer-device", "symbol": "TEST", "module": "src/lib.rs",
                "root_type": "Test", "source": "test.json", "macro": "test"}]}))
            args = SimpleNamespace(command="prepare-typed-data", check=True, crate=None, symbol=None,
                                   record_verification=False, recover_maintenance=False)
            with patch.object(rust_data, "RustData"), patch.object(rust_data, "build_table"), \
                    patch.object(rust_data, "check_mapping_engine_data", side_effect=ValueError("stale mapping branch")):
                with self.assertRaisesRegex(ValueError, "stale mapping branch"):
                    rust_data.run(args)

    def test_unfiltered_storage_check_includes_specialized_receiver_reset_table(self):
        import typed_tables
        with tempfile.TemporaryDirectory() as directory, patch.object(rust_data, "ROOT", Path(directory)):
            root = Path(directory)
            path = root / "tools/typed-data.json"
            path.parent.mkdir()
            path.write_text(json.dumps({"tables": [{"crate": "razer-storage", "symbol": "TEST", "module": "src/lib.rs",
                "root_type": "Test", "source": "test.json", "macro": "test"}]}))
            args = SimpleNamespace(command="prepare-typed-data", check=True, crate=["razer-storage"], symbol=None,
                                   record_verification=False, recover_maintenance=False)
            with patch.object(rust_data, "RustData"), patch.object(rust_data, "build_table"), \
                    patch.object(typed_tables, "check_receiver_reset_storage", side_effect=ValueError("stale receiver branch")):
                with self.assertRaisesRegex(ValueError, "stale receiver branch"):
                    rust_data.run(args)

    def test_summary_keeps_independent_event_hashes_and_each_product_without_raw_tables(self):
        summary = rust_data.mapping_engine_summary({"746": self.source(), "740": self.source()})
        self.assertEqual(summary["products"], [{"product_id": 740, "table": 0}, {"product_id": 746, "table": 0}])
        self.assertEqual(len(summary["tables"]), 1)
        table = summary["tables"][0]
        self.assertEqual(table["default_turbos"][0]["source_hash"], "7c29ae49a296fae31b7a8f763361d24e")
        self.assertEqual(table["default_turbos"][0]["events_md5"], "d751713988987e9331980363e24189ce")
        self.assertEqual(table["key_count"], 1)
        self.assertEqual(table["keys_without_input_id"], 0)
        self.assertFalse({"keys", "constants", "events"} & set(table))
        for field in ("name", "scancode", "flag", "outputFlag"):
            changed = self.source()
            changed["keys"][0][field] = "other" if field == "name" else 7
            result = rust_data.mapping_engine_summary({"740": self.source(), "746": changed})
            self.assertEqual(result["products"][1]["table"], 1)
            self.assertEqual(len(result["tables"]), 2)

    def test_summary_rejects_nonexact_numbers_and_verifies_ordered_readback(self):
        for value in (1.0, 2**53, {1: "integer key"}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                rust_data.mapping_source_md5(value)
        with tempfile.TemporaryDirectory() as directory, patch.object(rust_data, "ROOT", Path(directory)):
            output = Path(directory) / "crates/razer-device/testdata/re/mapping-engine-source-summary.json"
            output.parent.mkdir(parents=True)
            source = {"740": self.source()}
            report = rust_data.prepare_mapping_engine_summary(source)
            self.assertLess(report["bytes"], 1500)
            rust_data.prepare_mapping_engine_summary(source, check=True)
            output.write_bytes(output.read_bytes().replace(b'"event_count": 0', b'"event_count": 1'))
            with self.assertRaisesRegex(ValueError, "differs"):
                rust_data.prepare_mapping_engine_summary(source, check=True)


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

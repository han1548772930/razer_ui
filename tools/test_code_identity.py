"""Identity collisions must not erase executable distinctions."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import code_identity as identity
import frontend
import host


class AstIdentityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parser = host.HostLocalAudit().parser

    def norm(self, raw):
        tree = self.parser.parse(raw)
        self.assertFalse(tree.root_node.has_error)
        return identity.normalized_ast(tree.root_node)

    def test_trivia_only_is_equal(self):
        self.assertEqual(self.norm(b"x = 1 + y;"), self.norm(b"x=1/* comment */+y;"))

    def test_inputs_bindings_operators_and_asi_remain_distinct(self):
        for left, right in [(b"x=1", b"x=2"), (b"x=r(12)", b"x=r(13)"),
                            (b"x=y+z", b"x=y-z"), (b"x=y", b"x=z"),
                            (b"function x(){return 2}", b"function x(){return\n2}")]:
            with self.subTest(left=left):
                self.assertNotEqual(self.norm(left), self.norm(right))

    def test_webpack_module_number_only_is_omitted(self):
        def factory(raw):
            tree = self.parser.parse(raw)
            rows = frontend.recovery_segments(tree.root_node, include_standalone=True)
            self.assertEqual(len(rows), 1)
            owner = rows[0]["factories"][0]
            start, end = owner["factory_range"]
            return identity.normalized_ast(tree.root_node.descendant_for_byte_range(start, end - 1), factory=True)
        self.assertEqual(factory(b"self.webpackChunkx.push([[1],{12(a,b,r){b.x=r(90)}}]);"),
                         factory(b"self.webpackChunkx.push([[1],{33(a,b,r){b.x=r(90)}}]);"))
        self.assertNotEqual(factory(b"self.webpackChunkx.push([[1],{12(a,b,r){b.x=r(90)}}]);"),
                            factory(b"self.webpackChunkx.push([[1],{33(a,b,r){b.x=r(91)}}]);"))

    def test_exponent_module_key_keeps_entire_table(self):
        tree = self.parser.parse(b'self.webpackChunkx.push([[1],{3:(a,b,r)=>{r(7)},44e3:(a,b,r)=>{b.x=1}}]);')
        rows = frontend.recovery_segments(tree.root_node, include_standalone=True)
        self.assertEqual([f["module_id"] for f in rows[0]["factories"]], [3, 44000])

    def test_streamed_sections_keep_exact_json_semantics(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "receipt.json"
            expected = {"one": {"body": "quoted \"{}\"", "failures": []}, "two": [1, 2.5, None]}
            path.write_text(json.dumps(expected), encoding="utf-8")
            self.assertEqual(dict(identity.stream_sections(path)), expected)

    def test_streaming_preserves_uint64_and_standard_float_canonical_form(self):
        raw = '{"native":{"mask":18446744073709551615,"larger":184467440737095516160,"values":[0.25,-0.0,9007199254740993.0]},"next":{"ready":true}}'
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "receipt.json"
            path.write_text(raw, encoding="utf-8")
            actual = dict(identity.stream_sections(path))
            self.assertEqual(identity.digest(actual), identity.digest(json.loads(raw)))
            self.assertEqual(actual["native"]["mask"], 18446744073709551615)

    def test_factory_and_registration_are_saved_from_one_ast(self):
        raw = b'self.webpackChunkx.push([[1],{12(a,b,r){r(90);b.useFeature("genericFeature",null,{features:[{featureKey:"lighting"}]});}}]);'
        row = {"path": ".analysis/inputs/test.js", "bytes": len(raw), "sha256": identity.hashlib.sha256(raw).hexdigest()}
        with patch.object(host.HostLocalAudit, "read", return_value=raw):
            factories, markers, registrations = identity.parse_source(row, {"lighting"})
        with tempfile.TemporaryDirectory() as directory:
            db = identity.connect(Path(directory) / "index.sqlite3")
            try:
                db.executemany("INSERT INTO factories VALUES(?,?,?,?,?,?,?,?)", factories)
                db.executemany("INSERT INTO registrations VALUES(?,?,?,?,?)", registrations)
                self.assertEqual(json.loads(db.execute("SELECT dependencies FROM factories").fetchone()[0]), ["90"])
                self.assertEqual(json.loads(db.execute("SELECT generic_keys FROM registrations").fetchone()[0]), ["lighting"])
                self.assertEqual(markers[0][3], "lighting")
            finally:
                db.close()

    def test_shared_app_identity_does_not_fabricate_product_bindings(self):
        with tempfile.TemporaryDirectory() as directory:
            db = identity.connect(Path(directory) / "index.sqlite3")
            try:
                db.execute("INSERT INTO shared_deployments VALUES(?,?,?,?,NULL)", ("synapse-macro", "https://apps.razer.com/synapse/macro/", "acquisition.json", "a" * 64))
                db.execute("INSERT INTO sources VALUES(?,?,?)", ("macro.js", "b" * 64, 10))
                db.execute("INSERT INTO shared_sources VALUES(?,?,?)", ("synapse-macro", "macro.js", "https://apps.razer.com/synapse/macro/main.js"))
                db.execute("INSERT INTO factories VALUES(?,?,?,?,?,?,?,?)", ("b" * 64, 0, 10, "1", "c" * 64, "d" * 64, "webpack_factory", "[]"))
                self.assertEqual(db.execute("SELECT COUNT(*) FROM shared_variants").fetchone()[0], 1)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM product_variants").fetchone()[0], 0)
                db.execute("UPDATE shared_deployments SET failure='source_changed'")
                self.assertEqual(db.execute("SELECT COUNT(*) FROM shared_variants").fetchone()[0], 0)
            finally:
                db.close()


if __name__ == "__main__":
    unittest.main()

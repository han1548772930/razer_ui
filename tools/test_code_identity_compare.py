"""Counterexamples for accidentally erasing JS binding or public-key changes."""
import unittest

import host
import code_identity_compare as compare


class BindingComparisonTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parser = host.HostLocalAudit().parser

    def check(self, left, right, **kwargs):
        factory = kwargs.pop("factory", False)
        a = compare.SourceContext.parse(self.parser, left.encode(), factory=factory)
        b = compare.SourceContext.parse(self.parser, right.encode(), factory=factory)
        return compare.compare(a, b, **kwargs)

    def test_local_rename_and_nested_shadowing_correspond(self):
        result = self.check("function f(x){let y=x;return function(x){return x+y}}",
                            "function g(a){let b=a;return function(c){return c+b}}")
        self.assertTrue(result["structure_corresponds"])
        self.assertEqual(result["behavior_equivalence"], "not_inferred")

    def test_same_spelling_cannot_switch_binding(self):
        self.assertFalse(self.check("function f(x){return function(y){return x}}",
                                    "function f(x){return function(x){return x}} ")["structure_corresponds"])

    def test_iteration_binding_renames_keep_lexical_owners(self):
        for left, right in [
            ("function f(xs){for(let t of xs) use(t)}",
             "function g(ys){for(let q of ys) use(q)}"),
            ("function f(xs){for(var t in xs) use(t); return t}",
             "function g(ys){for(var q in ys) use(q); return q}"),
            ("function f(xs){for(let t of xs){for(let t of xs) use(t);use(t)}}",
             "function g(ys){for(let q of ys){for(let r of ys) use(r);use(q)}}"),
            ("function f(xs){for(const {key:t} of xs) use(t)}",
             "function g(ys){for(const {key:q} of ys) use(q)}"),
        ]:
            with self.subTest(left=left):
                self.assertTrue(self.check(left, right)["structure_corresponds"])

    def test_iteration_scope_and_global_changes_are_not_renames(self):
        for left, right in [
            ("function f(xs){for(let t of xs) use(t);return t}",
             "function g(ys){for(let q of ys) use(q);return q}"),
            ("function f(xs){for(t of xs) use(t)}",
             "function g(ys){for(q of ys) use(q)}"),
            ("function f(xs){for(let t of xs){for(let y of xs) use(t)}}",
             "function g(ys){for(let q of ys){for(let r of ys) use(r)}}"),
        ]:
            with self.subTest(left=left):
                self.assertFalse(self.check(left, right)["structure_corresponds"])

    def test_two_locals_cannot_merge(self):
        self.assertFalse(self.check("function f(a,b){return a+b}",
                                    "function g(x,y){return x+x}")["structure_corresponds"])

    def test_globals_keep_names_and_positions(self):
        self.assertFalse(self.check("const a=Number(x)+String(y)",
                                    "const b=String(x)+Number(y)")["structure_corresponds"])

    def test_object_and_destructuring_shorthand_keys_are_public(self):
        self.assertFalse(self.check("function f(a){return {a}}", "function g(b){return {b}}")["structure_corresponds"])
        self.assertFalse(self.check("const {a}=input; use(a)", "const {b}=input; use(b)")["structure_corresponds"])
        self.assertTrue(self.check("const {a:x}=input; use(x)", "const {a:y}=input; use(y)")["structure_corresponds"])

    def test_method_property_export_names_remain_distinct(self):
        for left, right in [("class A{go(x){return x}}", "class B{stop(y){return y}}"),
                            ("class A{field=1}", "class B{other=1}"),
                            ("export function f(x){return x}", "export function g(y){return y}"),
                            ("export function* f(x){yield x}", "export function* g(y){yield y}"),
                            ("const a=1; export {a}", "const b=1; export {b}"),
                            ("const a=1; export {a as publicName}", "const b=1; export {b as changedName}")]:
            with self.subTest(left=left):
                self.assertFalse(self.check(left, right)["structure_corresponds"])
        self.assertTrue(self.check("const a=1; export {a as publicName}",
                                   "const b=1; export {b as publicName}")["structure_corresponds"])

    def test_literals_and_operators_are_preserved(self):
        for right in ["function g(y){return y+2}", "function g(y){return y-1}"]:
            self.assertFalse(self.check("function f(x){return x+1}", right)["structure_corresponds"])

    def test_dependency_difference_requires_explicit_pair(self):
        left, right = "(m,e,r)=>{const a=r(12);e.run=a.run}", "(x,y,z)=>{const b=z(34);y.run=b.run}"
        self.assertFalse(self.check(left, right, factory=True)["structure_corresponds"])
        result = self.check(left, right, factory=True, import_substitutions=[[12, 34]])
        self.assertTrue(result["structure_corresponds"])
        self.assertEqual(result["dependency_changes"][0]["provider_equivalence"], "not_inferred")

    def test_import_pair_does_not_erase_value_or_shadowed_call(self):
        self.assertFalse(self.check("(m,e,r)=>{e.value=12}", "(x,y,z)=>{y.value=34}",
                                    factory=True, import_substitutions=[(12, 34)])["structure_corresponds"])
        self.assertFalse(self.check("(m,e,r)=>{function f(r){return r(12)}}",
                                    "(x,y,z)=>{function g(z){return z(34)}}", factory=True,
                                    import_substitutions=[(12, 34)])["structure_corresponds"])

    def test_comments_do_not_change_structure(self):
        self.assertTrue(self.check("function f(x){/* memo */return x}", "function g(y){return y}")["structure_corresponds"])

    def test_function_value_parentheses_are_transparent_without_erasing_other_boundaries(self):
        for left, right in [
            ("wrap(e=>({value:e}))", "wrap((x=>({value:x})))"),
            ("wrap(function(e){return e})", "wrap(((function(x){return x})))"),
            ("wrap(function*(e){yield e})", "wrap((function*(x){yield x}))"),
        ]:
            with self.subTest(left=left):
                self.assertTrue(self.check(left, right)["structure_corresponds"])
        self.assertFalse(self.check("wrap(e=>({value:1}))", "wrap((e=>({value:2})))")["structure_corresponds"])
        self.assertFalse(self.check("a?.b.c", "(a?.b).c")["structure_corresponds"])
        self.assertFalse(self.check("a?.b()", "(a?.b)()")["structure_corresponds"])

    def test_incomplete_input_is_rejected(self):
        with self.assertRaises(ValueError):
            self.check("function f(", "function g(x){}")

    def test_selected_owner_reports_external_dependency_obligation(self):
        sources = [b"const helper=first;function f(x){return helper(x)}",
                   b"const helper=second;function f(y){return helper(y)}"]
        contexts = [compare.SourceContext.parse(self.parser, s).select(s.index(b"function"), len(s)) for s in sources]
        result = compare.compare(*contexts)
        self.assertTrue(result["structure_corresponds"])
        self.assertEqual(result["external_references"][0]["name"], "helper")
        self.assertEqual(result["external_references"][0]["provider_equivalence"], "not_inferred")
        with self.assertRaises(ValueError):
            contexts[0].select(1, 10)


if __name__ == "__main__":
    unittest.main()

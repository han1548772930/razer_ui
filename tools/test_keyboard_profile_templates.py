"""Counterexamples for the current keyboard typed recipe boundary."""
import hashlib
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest

sys.path[:0] = [str(Path(__file__).parent), str(Path(__file__).parent / "analyzers/python-static")]
from tree_sitter import Language, Parser
import tree_sitter_javascript
import code_identity as ci
import keyboard_profile_templates as recipes


class RecipeParsing(unittest.TestCase):
    def module(self, text, *, product=515, modules=None, syntax=None):
        raw = text.encode()
        parser = Parser(Language(tree_sitter_javascript.language()))
        prefix = b"({" if raw[:1].isdigit() else b"("
        suffix = b"})" if len(prefix) == 2 else b")"
        tree = parser.parse(prefix + raw + suffix)
        self.assertFalse(tree.root_node.has_error)
        factory = tree.root_node.descendant_for_byte_range(len(prefix), len(prefix) + len(raw) - 1)
        record = {"start_byte": 0, "end_byte": len(raw), "normalized_sha": ci.normalized_ast(factory, factory=True)}
        index = SimpleNamespace(parser=parser, syntax={} if syntax is None else syntax,
                                original=lambda _: raw, module=lambda pid, key: modules[pid, key])
        return recipes.Module(index, product, record, {})

    def test_factory_forms_have_the_same_literal_and_spread_contract(self):
        bodies = ["17(e,t,r){%s}", "function(e,t,r){%s}", "(e,t,r)=>{%s}"]
        for shape in bodies:
            module = self.module(shape % 'const base={value:30,enabled:!0},profile={...base,value:50};')
            self.assertEqual(module.value(module.bindings["profile"]), {"value": 50, "enabled": True})

    def test_shared_factory_imports_keep_the_product_context(self):
        imported = {pid: self.module('function(e,t,r){r.d(t,{A:()=>v});const v=%d;}' % value)
                    for pid, value in [(515, 30), (600, 100)]}
        modules = {(pid, "9"): value for pid, value in imported.items()}
        syntax = {}
        source = 'function(e,t,r){const dep=r(9),profile={value:dep.A};}'
        a = self.module(source, product=515, modules=modules, syntax=syntax)
        b = self.module(source, product=600, modules=modules, syntax=syntax)
        self.assertIs(a.tree, b.tree)
        self.assertEqual(a.value(a.bindings["profile"]), {"value": 30})
        self.assertEqual(b.value(b.bindings["profile"]), {"value": 100})

    def test_guid_allocation_references_are_shared_only_for_the_same_source_call(self):
        module = self.module('function(e,t,r){const mode={uid:g.newGuid()},other={uid:g.newGuid()},profile={selected:mode.uid,modes:[mode,other]};}')
        module.guid_functions.add("g.newGuid")
        profile = module.value(module.bindings["profile"])
        self.assertEqual(profile["selected"], profile["modes"][0]["uid"])
        self.assertNotEqual(profile["modes"][0]["uid"], profile["modes"][1]["uid"])
        rejected = self.module('function(e,t,r){const profile={uid:g.otherGuid()};}')
        rejected.guid_functions.add("g.newGuid")
        with self.assertRaisesRegex(ValueError, "Unresolved keyboard profile expression"):
            rejected.value(rejected.bindings["profile"])

    def test_oled_repeated_entries_preserve_their_complete_independent_shapes(self):
        module = self.module('function(e,t,r){const profile=Array.from({length:6},()=>({src:"",isEnabled:!0}));}')
        rows = module.value(module.bindings["profile"])
        self.assertEqual(rows, [{"src": "", "isEnabled": True}] * 6)
        rows[0]["src"] = "changed"
        self.assertEqual(rows[1]["src"], "")
        for expression in ['Array.from({length:100000},()=>({}))', 'Array.from({length:6},(i)=>i)']:
            module = self.module('function(e,t,r){const profile=' + expression + ';}')
            with self.assertRaises(ValueError):
                module.value(module.bindings["profile"])

    def test_enum_alias_and_static_name_interpolation_follow_the_actual_export(self):
        module = self.module('function(e,t,r){var E;!function(v){v[v.Disable=0]="Disable";v[v.Enable=1]="Enable"}(E||(E={}));const label="VOLUME",profile={mode:E.Disable,name:`${label}`};}')
        self.assertEqual(module.value(module.bindings["profile"]), {"mode": 0, "name": "VOLUME"})

    def test_non_import_comma_effects_and_unknown_literals_are_rejected(self):
        for value in ['(mutate(),{value:30})', '{value:readHardware()}', '{value:missingBinding}']:
            module = self.module('function(e,t,r){const profile=' + value + ';}')
            with self.assertRaises(ValueError):
                module.value(module.bindings["profile"])

    def test_direct_default_export_keeps_its_actual_top_level_owner(self):
        module = self.module('function(e,t,r){const v={brightness:30},other={};other.default=0;function hidden(t){t.default=9;}t.default=v;}')
        self.assertEqual(module.value(module.exports["default"]), {"brightness": 30})
        hidden = self.module('function(e,t,r){function hidden(t){t.default=9;}if(enabled){t.default=7;}}')
        self.assertNotIn("default", hidden.exports)
        with self.assertRaisesRegex(ValueError, "assigned more than once"):
            self.module('function(e,t,r){t.default=0;t.default=1;}')

    def test_nested_fields_do_not_evaluate_unrelated_source_calls(self):
        module = self.module('function(e,t,r){const info={stamp:readClock(),obm:{brightness:!0}},profile=info.obm.brightness;}')
        self.assertTrue(module.value(module.bindings["profile"]))
        with self.assertRaises(ValueError):
            module.value(module.bindings["info"])

    def test_namespace_copy_and_spread_keep_last_property_precedence(self):
        source = self.module('function(e,t,r){r.d(t,{TABLE:()=>v,unused:()=>unknown});const v={value:30};}')
        module = self.module('function(e,t,r){const dep=r(9),copy=Object.assign({},dep),base={value:1},profile={copied:copy.TABLE.value,first:({value:2,...base}).value,last:({...base,value:3}).value};}',
                             modules={(515, "9"): source})
        self.assertEqual(module.value(module.bindings["profile"]), {"copied": 30, "first": 1, "last": 3})
        for expression in ['Object.assign(target,{value:1}).value', '({value:1,...unknown}).value']:
            module = self.module('function(e,t,r){const profile=' + expression + ';}')
            with self.assertRaises(ValueError):
                module.value(module.bindings["profile"])

    def test_owner_boundary_and_normalized_factory_changes_are_not_accepted(self):
        module = self.module('function(e,t,r){const profile={value:30};}')
        node = module.bindings["profile"]
        with self.assertRaises(ValueError):
            module.owner_value({"byte_start": node.start_byte + module.offset + 1,
                                "byte_end": node.end_byte + module.offset})
        original = 'function(e,t,r){const profile={value:30};}'
        changed = original.replace("30", "31").encode()
        module.index.original = lambda _: changed
        with self.assertRaises(ValueError):
            recipes.Module(module.index, 515, module.record, {})


if __name__ == "__main__":
    unittest.main()

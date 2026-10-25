"""Typed static capture contracts; these tests never load IDA or a target DLL."""
import ast
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import unittest


SOURCE = Path(__file__).with_name("ida") / "capture_values.py"
SPEC = importlib.util.spec_from_file_location("typed_capture_values", SOURCE)
VALUES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALUES)


class Location:
    TAGS = {"none": 0, "stack": 1, "scattered": 2, "register": 3,
            "register_pair": 4, "register_relative": 5, "static_address": 6, "custom": 7}

    def __init__(self, kind, *payload):
        self.kind, self.payload = kind, payload

    def __repr__(self):
        raise AssertionError("SDK object representations must never enter capture output")

    def __eq__(self, other):
        return self.kind == other.kind and self.payload == other.payload

    def atype(self):
        return self.TAGS[self.kind]

    def is_badloc(self): return self.kind == "none"
    def is_reg1(self): return self.kind == "register"
    def is_reg2(self): return self.kind == "register_pair"
    def is_stkoff(self): return self.kind == "stack"
    def is_ea(self): return self.kind == "static_address"
    def is_rrel(self): return self.kind == "register_relative"
    def is_scattered(self): return self.kind == "scattered"
    def is_custom(self): return self.kind == "custom"
    def reg1(self): return self.payload[0]
    def reg2(self): return self.payload[1]
    def regoff(self): return self.payload[1]
    def stkoff(self): return self.payload[0]
    def get_ea(self): return self.payload[0]
    def get_rrel(self): return SimpleNamespace(reg=self.payload[0], off=self.payload[1])
    def scattered(self): return SimpleNamespace(size=lambda: len(self.payload), at=self.payload.__getitem__)


class TypedCaptureValuesTests(unittest.TestCase):
    def test_tagged_payloads_are_copied_as_json_values_without_proxy_formatting(self):
        cases = [(Location("register", 8, 1), {"register": 8, "register_offset": 1}),
                 (Location("register_pair", 8, 12), {"register": 8, "second_register": 12}),
                 (Location("stack", 40), {"stack_offset": 40}),
                 (Location("register_relative", 20, -8), {"register": 20, "offset": -8}),
                 (Location("static_address", 0x10001000), {"address": "0x10001000"})]
        for location, expected in cases:
            with self.subTest(kind=location.kind):
                value = VALUES.argument_location(location, "hexrays_microregisters")
                self.assertTrue(value["complete"])
                self.assertEqual(value["kind"], location.kind)
                self.assertEqual({key: value[key] for key in expected}, expected)
                self.assertEqual(json.loads(json.dumps(value)), value)

    def test_scattered_offsets_sizes_and_unknown_locations_remain_explicit(self):
        first, second = Location("register", 8, 0), Location("stack", 48)
        first.off, first.size = 0, 4
        second.off, second.size = 4, 8
        value = VALUES.argument_location(Location("scattered", first, second), "hexrays_microregisters")
        self.assertEqual([(part["argument_offset"], part["size"]) for part in value["parts"]], [(0, 4), (4, 8)])
        self.assertEqual(value["parts"][1]["stack_offset"], 48)
        for kind in ("none", "custom"):
            missing = VALUES.argument_location(Location(kind), "hexrays_microregisters")
            self.assertEqual(missing["status"], "unobserved")
            self.assertFalse(missing["complete"])
            self.assertIn("reason", missing)

    def test_depth_bound_preserves_the_observation_as_incomplete(self):
        value = VALUES.argument_location(Location("scattered"), "hexrays_microregisters",
                                         VALUES.LOCATION_LIMITS["depth"])
        self.assertFalse(value["complete"])
        self.assertEqual(value["status"], "truncated")
        self.assertEqual(value["reason"], "location_depth_limit")

    def locals(self, *, mismatch=False, conversion_error=False):
        stack = Location("stack", 40)
        reg = Location("register", 8, 0)
        variables = [SimpleNamespace(name=name, type=lambda typ=typ: typ, is_arg_var=argument,
                                     width=width, defea=0x10001234, defblk=0, location=loc)
                     for name, typ, argument, width, loc in
                     [("saved_cw", "unsigned __int16[4]", lambda: False, 8, stack),
                      ("error_class", "int", True, 4, reg)]]

        def vd2ida(location, width):
            if conversion_error:
                raise ValueError("coordinate conversion unavailable")
            return location

        mba = SimpleNamespace(entry_ea=0x10001000, vd2idaloc=vd2ida,
            idaloc2vd=lambda location, width: Location("stack", 44) if mismatch else location,
            stkoff_vd2ida=lambda offset: offset - 32, stkoff_ida2vd=lambda offset: offset + 32)
        cfunc = SimpleNamespace(mba=mba, get_lvars=lambda: variables, get_stkoff_delta=lambda: 32)
        sdk = SimpleNamespace(print_vdloc=lambda location, width: "stack:28.8" if location is stack else "eax.4")
        return VALUES.local_variables(cfunc, sdk)

    def test_local_values_retain_real_coordinates_and_verified_roundtrips(self):
        result = self.locals()
        self.assertTrue(result["locals_capture"]["complete"])
        saved, error = result["locals"]
        self.assertEqual(saved["location"]["stack_offset"], 40)
        self.assertEqual(saved["stack_coordinate_conversion"]["ida_offset"], 8)
        self.assertEqual(saved["definition_ea"], "0x10001234")
        self.assertEqual(saved["width"], 8)
        self.assertFalse(saved["argument"])
        self.assertTrue(error["argument"])
        self.assertEqual(error["location"]["register"], 8)
        self.assertEqual(json.loads(json.dumps(result)), result)

    def test_failed_or_mismatched_conversion_cannot_be_reported_complete(self):
        for options in ({"mismatch": True}, {"conversion_error": True}):
            with self.subTest(options=options):
                result = self.locals(**options)
                self.assertFalse(result["locals_capture"]["complete"])
                saved = result["locals"][0]
                self.assertEqual(saved["location"]["stack_offset"], 40)
                self.assertFalse(saved["ida_location_conversion"]["complete"])

    def test_both_capture_owners_call_the_typed_serializer(self):
        for name in ("capture.py", "supplement.py"):
            tree = ast.parse(SOURCE.with_name(name).read_text(encoding="utf-8"))
            calls = [node for node in ast.walk(tree) if isinstance(node, ast.Call)]
            self.assertTrue(any(ast.unparse(call.func) == "capture_values.local_variables" for call in calls), name)
            self.assertFalse(any(ast.unparse(call.func) == "str" and call.args
                                 and isinstance(call.args[0], ast.Attribute)
                                 and call.args[0].attr == "location" for call in calls), name)


if __name__ == "__main__":
    unittest.main()

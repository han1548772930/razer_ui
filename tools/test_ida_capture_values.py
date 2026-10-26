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

    def locals(self, *, mismatch=False, conversion_error=False, stack_offset=40,
               stack_delta=32, mba_stack_error=False):
        stack = Location("stack", stack_offset)
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

        def mba_stack(offset):
            if mba_stack_error:
                raise ValueError("MBA stack mapping unavailable")
            return max(0, offset - stack_delta)

        mba = SimpleNamespace(entry_ea=0x10001000, vd2idaloc=vd2ida,
            idaloc2vd=lambda location, width: Location("stack", 44) if mismatch else location,
            stkoff_vd2ida=mba_stack, stkoff_ida2vd=lambda offset: offset + stack_delta)
        cfunc = SimpleNamespace(mba=mba, get_lvars=lambda: variables, get_stkoff_delta=lambda: stack_delta)
        sdk = SimpleNamespace(print_vdloc=lambda location, width: "stack:28.8" if location is stack else "eax.4",
                              is_kreg=lambda reg: False)
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
        self.assertEqual(result["locals_capture"]["schema_version"], 2)
        self.assertEqual(json.loads(json.dumps(result)), result)

    def test_cfunc_frame_coordinates_keep_negative_offsets_and_separate_mba_observations(self):
        result = self.locals(stack_offset=4, stack_delta=72)
        self.assertTrue(result["locals_capture"]["complete"])
        saved = result["locals"][0]
        conversion = saved["stack_coordinate_conversion"]
        self.assertEqual(conversion["coordinate_namespace"], "ida_frame_offsets")
        self.assertEqual(conversion["conversion_api"], "cfunc_t.get_stkoff_delta")
        self.assertEqual(conversion["ida_offset"], -68)
        self.assertEqual(conversion["roundtrip_decompiler_offset"], 4)
        self.assertEqual(conversion["cfunc_stack_offset_delta"], 72)
        self.assertFalse(saved["mba_stack_coordinate_observation"]["roundtrip_matches"])
        self.assertFalse(saved["mba_stack_coordinate_observation"]["used_for_cfunc_frame_conversion"])

    def test_unavailable_mba_mapping_does_not_replace_documented_cfunc_coordinates(self):
        result = self.locals(stack_offset=4, stack_delta=72, mba_stack_error=True)
        self.assertTrue(result["locals_capture"]["complete"])
        saved = result["locals"][0]
        self.assertEqual(saved["stack_coordinate_conversion"]["ida_offset"], -68)
        self.assertEqual(saved["mba_stack_coordinate_observation"]["status"], "diagnostic_error")
        self.assertFalse(saved["mba_stack_coordinate_observation"]["complete"])

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


class LocalNamespaceDiagnosticTests(unittest.TestCase):
    def fixture(self, *, fail_mapping=False):
        source = SOURCE.with_name("supplement.py")
        definitions = [node for node in ast.parse(source.read_text(encoding="utf-8")).body
                       if isinstance(node, ast.FunctionDef)
                       and node.name in {"_static_diagnostic_error", "local_namespace_diagnostics"}]
        block = SimpleNamespace(serial=3, flags=0x30,
                                head=SimpleNamespace(ea=0xF0000010), tail=SimpleNamespace(ea=0xF0000014))
        variable = SimpleNamespace(name="status_c0", width=1, defea=0xF0000010, defblk=3,
                                   location=Location("register", 181, 0), type=lambda: "bool",
                                   is_arg_var=False)

        def cpu_mapping(reg, width):
            if fail_mapping:
                raise ValueError("SDK mapping unavailable")
            return -1

        def vd2ida(location, width):
            return Location("none")

        def ida2vd(location, width):
            raise RuntimeError("INTERR: 51111")

        sdk = SimpleNamespace(MBL_INLINED=0x10, MBL_EXTFRAME=0x20, get_mreg_name=lambda r, w: "c0",
                              mreg2reg=cpu_mapping, is_kreg=lambda r: True,
                              get_temp_regs=lambda: SimpleNamespace(has=lambda r: False),
                              print_vdloc=lambda location, width: "c0.1")
        mba = SimpleNamespace(entry_ea=0x10001000, qty=5,
                              map_fict_ea=lambda ea: 0x10002000 + (ea - 0xF0000010),
                              get_mblock=lambda number: block, vd2idaloc=vd2ida, idaloc2vd=ida2vd)
        owner = SimpleNamespace(start_ea=0x10002000, end_ea=0x10002050, flags=0)
        namespace = {"capture_values": VALUES, "ida_hexrays": sdk,
                     "idaapi": SimpleNamespace(BADADDR=0xFFFFFFFF),
                     "ida_funcs": SimpleNamespace(get_func=lambda ea: owner)}
        exec(compile(ast.Module(body=definitions, type_ignores=[]), str(source), "exec"), namespace)
        cfunc = SimpleNamespace(mba=mba, get_lvars=lambda: [variable], get_stkoff_delta=lambda: 136)
        return namespace["local_namespace_diagnostics"], cfunc, sdk, variable

    def test_sdk_kernel_register_keeps_its_namespace_and_has_no_processor_conversion(self):
        observe, cfunc, sdk, variable = self.fixture()
        before = VALUES.local_variables(cfunc, sdk)
        result = observe(cfunc)
        after = VALUES.local_variables(cfunc, sdk)
        self.assertEqual(before, after)
        self.assertTrue(after["locals_capture"]["complete"])
        proof = after["locals"][0]["virtual_register_proof"]
        self.assertTrue(proof["complete"] and proof["is_kernel_register"])
        self.assertEqual((proof["register"], proof["register_offset"], proof["width"], proof["name"]),
                         (181, 0, 1, "c0"))
        self.assertEqual(proof["processor_register"], -1)
        self.assertEqual(after["locals"][0]["ida_location_conversion"], {
            "status": "not_applicable", "complete": True,
            "reason": "sdk_kernel_register_has_no_processor_location"})
        self.assertTrue(result["diagnostics_complete"])
        mapped = result["locals"][0]["register_observations"][0]
        self.assertEqual(mapped["namespace"], "hexrays_microregisters")
        self.assertEqual(mapped["processor_register"], {"status": "api_returned", "value": -1,
                                                       "maps_to_processor_register": False})
        self.assertTrue(mapped["kernel_register"]["value"])
        self.assertFalse(mapped["temporary_register"]["value"])
        self.assertEqual(json.loads(json.dumps(result)), result)

    def test_kernel_claim_requires_complete_sdk_identity_and_cannot_hide_unknown_cpu_locations(self):
        for failure in ("not_kernel", "processor_mismatch", "missing_name", "mapping_failure"):
            with self.subTest(failure=failure):
                _, cfunc, sdk, _ = self.fixture(fail_mapping=failure == "mapping_failure")
                if failure == "not_kernel":
                    sdk.is_kreg = lambda reg: False
                elif failure == "processor_mismatch":
                    sdk.mreg2reg = lambda reg, width: 8
                elif failure == "missing_name":
                    sdk.get_mreg_name = lambda reg, width: ""
                value = VALUES.local_variables(cfunc, sdk)
                self.assertFalse(value["locals_capture"]["complete"])
                self.assertFalse(value["locals"][0]["ida_location_conversion"]["complete"])

    def test_inline_definition_uses_sdk_block_and_mapped_original_function(self):
        observe, cfunc, sdk, variable = self.fixture()
        result = observe(cfunc)
        saved = result["locals"][0]
        self.assertEqual(saved["definition_address"]["definition_ea"], "0xf0000010")
        self.assertEqual(saved["definition_address"]["api_mapped_ea"], "0x10002000")
        self.assertEqual(saved["definition_address"]["mapped_function_owner"]["start_ea"], "0x10002000")
        block = saved["definition_block_observation"]
        self.assertEqual(block["serial"], 3)
        self.assertTrue(block["inlined"] and block["external_frame"])
        self.assertEqual(block["tail"]["api_mapped_ea"], "0x10002004")
        self.assertEqual(result["cfunc_stack_offset_delta"], 136)

    def test_mapping_failure_preserves_other_observations_and_out_of_range_block(self):
        observe, cfunc, sdk, variable = self.fixture(fail_mapping=True)
        variable.defblk = -1
        result = observe(cfunc)
        self.assertFalse(result["diagnostics_complete"])
        saved = result["locals"][0]
        self.assertEqual(saved["definition_block_observation"]["status"], "unobserved")
        mapped = saved["register_observations"][0]
        self.assertEqual(mapped["processor_register"]["status"], "diagnostic_error")
        self.assertEqual(mapped["name"]["value"], "c0")
        self.assertTrue(mapped["kernel_register"]["value"])
        self.assertIn("SDK mapping unavailable", mapped["processor_register"]["diagnostic_errors"][0]["error"])


if __name__ == "__main__":
    unittest.main()

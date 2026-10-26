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


class PhysicalTbyteRegisterTests(unittest.TestCase):
    def fixture(self, issue=None):
        register, physical = 713, 45

        class Type:
            def __str__(self): return "_TBYTE"
            def is_float(self): return issue == "float32"
            def is_floating(self): return issue != "not_float"
            def is_tbyte(self): return issue != "not_tbyte"
            def get_size(self):
                if issue == "type_error": raise ValueError("SDK type unavailable")
                return 8 if issue == "type_width" else 10

        class Operand:
            def make_reg(self, value, width):
                self.r = value + (issue == "operand_register")
                self.size = width - (issue == "operand_width")
            def is_reg(self, value, width):
                return issue != "not_exact" and self.r == value and self.size == width
            def is_cc(self): return issue == "condition"
            def is_bit_reg(self): return issue == "bit"
            def is_kreg(self): return issue == "operand_kernel"

        def mapped(value, width):
            self.assertEqual(value, register)
            if issue == "mapping_error": raise ValueError("SDK mapping unavailable")
            return (4 if issue == "original_width_mapping" else -1) if width == 10 else (
                -1 if issue == "missing_identity_mapping" else physical)

        def parse(info, name):
            info.reg = physical + (issue == "parsed_register")
            info.size = 8 if issue == "capacity" else 10
            return issue != "parse"

        sdk = SimpleNamespace(mop_t=Operand, is_kreg=lambda value: issue == "kernel",
            get_temp_regs=lambda: SimpleNamespace(has_all=lambda value, width: issue == "temporary"),
            mreg2reg=mapped, reg2mreg=lambda value: register + (issue == "identity_roundtrip"))
        processor = SimpleNamespace(get_reg_name=lambda value, width: "" if issue == "name" else "physical_float",
            reg_info_t=SimpleNamespace, parse_reg_name=parse,
            ph_get_tbyte_size=lambda: 8 if issue == "tbyte" else 10)
        location = Location("register", register, 1 if issue == "offset" else 0)
        observed = VALUES.argument_location(location, "hexrays_microregisters")
        return observed, Type(), sdk, processor, location

    def test_physical_proof_requires_each_independent_sdk_fact_and_keeps_ten_bytes(self):
        issues = (None, "not_float", "not_tbyte", "float32", "type_width", "type_error", "operand_register", "operand_width", "not_exact",
                  "condition", "bit", "operand_kernel", "kernel", "temporary", "original_width_mapping",
                  "mapping_error", "missing_identity_mapping", "parsed_register", "capacity", "parse",
                  "identity_roundtrip", "name", "tbyte", "offset")
        for issue in issues:
            with self.subTest(issue=issue):
                observed, typ, sdk, processor, _ = self.fixture(issue)
                proof = VALUES.physical_tbyte_register_proof(observed, 10, typ, sdk, processor)
                self.assertEqual(proof["complete"], issue is None)
                self.assertEqual(proof["width"], 10)
                self.assertEqual(proof["identity_width"], 8)
                self.assertTrue(proof["identity_mapping_only"])
                self.assertEqual(json.loads(json.dumps(proof)), proof)
        for width in (1, 2, 4, 8, 12, 16):
            observed, typ, sdk, processor, _ = self.fixture()
            self.assertIsNone(VALUES.physical_tbyte_register_proof(observed, width, typ, sdk, processor))

    def capture(self, issue=None):
        observed, typ, sdk, processor, location = self.fixture(issue)
        first, second = Location("register", 45, 0), Location("register", 44, 0)
        first.off, first.size, second.off, second.size = 0, 8, 8, 2
        if issue == "split_processor": first.payload = (44, 0)
        if issue == "split_width": second.size = 1
        if issue == "split_offset": second.off = 9
        converted = Location("scattered", first, second)
        back_first = Location("register", 713, 0)
        back_first.off, back_first.size = 0, 10
        if issue == "back_register": back_first.payload = (714, 0)
        if issue == "back_width": back_first.size = 8
        returned = Location("scattered", back_first)
        if issue == "back_split":
            back_second = Location("register", 705, 0)
            back_first.size, back_second.off, back_second.size = 8, 8, 2
            returned = Location("scattered", back_first, back_second)
        if issue == "back_unknown": returned = Location("none")

        def vd2ida(value, width):
            if issue == "conversion_error": raise ValueError("SDK conversion unavailable")
            self.assertEqual(width, 10)
            return converted

        mba = SimpleNamespace(entry_ea=0x10001000, vd2idaloc=vd2ida,
                              idaloc2vd=lambda value, width: returned)
        variable = SimpleNamespace(name="value", type=lambda: typ, is_arg_var=True, width=10,
                                   defea=0x10001000, defblk=0, location=location)
        cfunc = SimpleNamespace(mba=mba, get_lvars=lambda: [variable], get_stkoff_delta=lambda: 0)
        sdk.print_vdloc = lambda value, width: "different_micro_float_name"
        return VALUES.local_variables(cfunc, sdk, processor)

    def test_independent_proof_preserves_failed_scattered_conversion_and_actual_back_coordinates(self):
        result = self.capture()
        self.assertTrue(result["locals_capture"]["complete"])
        local = result["locals"][0]
        self.assertEqual(local["width"], 10)
        self.assertNotIn("virtual_register_proof", local)
        conversion = local["ida_location_conversion"]
        self.assertEqual(conversion["status"], "physical_register_verified")
        self.assertNotIn("roundtrip_matches", conversion)
        original = conversion["original_conversion"]
        self.assertFalse(original["complete"] or original["roundtrip_matches"])
        self.assertEqual(original["status"], "api_returned")
        self.assertEqual([(part["register"], part["argument_offset"], part["size"])
                          for part in original["location"]["parts"]], [(45, 0, 8), (44, 8, 2)])
        self.assertEqual(original["roundtrip_location"]["part_count"], 1)
        self.assertEqual([(part["register"], part["argument_offset"], part["size"])
                         for part in original["roundtrip_location"]["parts"]], [(713, 0, 10)])
        proof = local["physical_register_proof"]
        self.assertFalse(proof["type_is_float"])
        self.assertTrue(proof["type_is_floating"] and proof["type_is_tbyte"])
        self.assertEqual(proof["processor_name"], "physical_float")
        self.assertNotEqual(proof["processor_name"], local["location"]["text"])

    def test_unverified_type_capacity_mapping_or_original_conversion_cannot_use_independent_proof(self):
        for issue in ("type_width", "not_float", "not_tbyte", "float32", "capacity", "identity_roundtrip", "offset", "back_unknown",
                      "parse", "conversion_error", "mapping_error", "missing_identity_mapping",
                      "split_processor", "split_width", "split_offset", "back_register", "back_width", "back_split"):
            with self.subTest(issue=issue):
                result = self.capture(issue)
                self.assertFalse(result["locals_capture"]["complete"])
                self.assertFalse(result["locals"][0]["ida_location_conversion"]["complete"])


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

    def test_codegen_temporary_requires_full_sdk_membership_and_no_processor_mapping(self):
        for failure in (None, "not_member", "processor_mapping", "empty_name", "membership_failure"):
            with self.subTest(failure=failure):
                _, cfunc, sdk, variable = self.fixture()
                variable.location = Location("register", 192, 0)
                variable.width = 8
                sdk.is_kreg = lambda register: False
                sdk.get_mreg_name = lambda register, width: "" if failure == "empty_name" else "rtt"
                sdk.mreg2reg = lambda register, width: 9 if failure == "processor_mapping" else -1

                def membership(register, width):
                    self.assertEqual((register, width), (192, 8))
                    if failure == "membership_failure": raise ValueError("SDK unavailable")
                    return failure != "not_member"

                sdk.get_temp_regs = lambda: SimpleNamespace(has_all=membership)
                value = VALUES.local_variables(cfunc, sdk)
                self.assertEqual(value["locals_capture"]["complete"], failure is None)
                observed = value["locals"][0]
                if failure is None:
                    proof = observed["virtual_register_proof"]
                    self.assertFalse(proof["is_kernel_register"])
                    self.assertTrue(proof["is_codegen_temporary"])
                    self.assertEqual(proof["temporary_api"], "ida_hexrays.get_temp_regs")
                    self.assertEqual(proof["membership_api"], "mlist_t.has_all")
                    self.assertEqual(proof["processor_register"], -1)
                    self.assertEqual(observed["ida_location_conversion"], {"status": "not_applicable",
                        "complete": True, "reason": "sdk_temporary_register_has_no_processor_location"})
                else:
                    self.assertFalse(observed["ida_location_conversion"]["complete"])

    def test_status_bits_require_live_operand_predicates_and_never_fake_cpu_roundtrips(self):
        for failure in (None, "not_bit", "processor", "readback", "width", "offset", "empty_name", "predicate"):
            with self.subTest(failure=failure):
                _, cfunc, sdk, variable = self.fixture()
                sdk.is_kreg = lambda register: False
                sdk.get_temp_regs = lambda: SimpleNamespace(has_all=lambda register, width: False)
                sdk.mreg2reg = lambda register, width: 7 if failure == "processor" else -1
                sdk.get_mreg_name = lambda register, width: "" if failure == "empty_name" else "c0"
                if failure == "width": variable.width = 2
                if failure == "offset": variable.location = Location("register", 181, 1)

                class Operand:
                    def make_reg(self, register, width):
                        self.r, self.size = (register + (failure == "readback")), width
                    def is_reg(self, register, width): return self.r == register and self.size == width
                    def is_cc(self): return False
                    def is_bit_reg(self):
                        if failure == "predicate": raise RuntimeError("SDK unavailable")
                        return failure != "not_bit"

                sdk.mop_t = Operand
                result = VALUES.local_variables(cfunc, sdk)
                self.assertEqual(result["locals_capture"]["complete"], failure is None)
                local = result["locals"][0]
                if failure is None:
                    proof = local["virtual_register_proof"]
                    self.assertTrue(proof["is_bit_register"] and proof["operand_is_exact_register"])
                    self.assertFalse(proof["is_condition_code"] or proof["is_kernel_register"]
                                     or proof["is_codegen_temporary"])
                    self.assertEqual((proof["operand_register"], proof["operand_width"]), (181, 1))
                    self.assertEqual(local["ida_location_conversion"], {"status": "not_applicable", "complete": True,
                        "reason": "sdk_bit_register_has_no_processor_location"})
                else:
                    self.assertFalse(local["ida_location_conversion"]["complete"])

    def test_processor_mapped_condition_flags_keep_the_actual_cpu_roundtrip(self):
        for register, processor_register in [(0, 35), (1, 36)]:
            with self.subTest(register=register):
                _, cfunc, sdk, variable = self.fixture()
                variable.location = Location("register", register, 0)
                sdk.is_kreg = lambda reg: False
                sdk.get_temp_regs = lambda: SimpleNamespace(has_all=lambda reg, width: False)
                sdk.mreg2reg = lambda reg, width: processor_register

                class Operand:
                    def make_reg(self, reg, width): self.r, self.size = reg, width
                    def is_reg(self, reg, width): return self.r == reg and self.size == width
                    def is_cc(self): return True
                    def is_bit_reg(self): return True

                sdk.mop_t = Operand
                cfunc.mba.vd2idaloc = lambda location, width: Location("register", processor_register, 0)
                cfunc.mba.idaloc2vd = lambda location, width: Location("register", register, 0)
                result = VALUES.local_variables(cfunc, sdk)
                local = result["locals"][0]
                self.assertTrue(result["locals_capture"]["complete"])
                self.assertNotIn("virtual_register_proof", local)
                conversion = local["ida_location_conversion"]
                self.assertTrue(conversion["complete"] and conversion["roundtrip_matches"])
                self.assertEqual(conversion["status"], "api_returned")
                self.assertEqual(conversion["location"]["register_namespace"], "ida_processor_registers")
                self.assertEqual(conversion["location"]["register"], processor_register)

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

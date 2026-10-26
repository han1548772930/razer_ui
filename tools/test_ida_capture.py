"""Pure tests for static SDK observation boundaries in the full capture owner."""
import ast
from pathlib import Path
from types import SimpleNamespace
import unittest


def observer():
    source = Path(__file__).parent / "ida/capture.py"
    tree = ast.parse(source.read_text(encoding="utf-8"))
    function = next(row for row in tree.body if isinstance(row, ast.FunctionDef)
                    and row.name == "microregister_predicates")
    namespace = {}
    exec(compile(ast.Module(body=[function], type_ignores=[]), str(source), "exec"), namespace)
    return namespace[function.name]


class MicroregisterPredicatesTest(unittest.TestCase):
    def sdk(self, *, bit=True, condition=False, kernel=False, temporary=False,
            changed=None, failing=None):
        calls = []

        class Operand:
            def make_reg(self, register, width):
                self.t, self.r, self.size = 1, register, width
                if changed:
                    setattr(self, changed, -1)
                calls.append(("make_reg", register, width))

            def is_reg(self, register, width):
                return self.r == register and self.size == width and changed != "is_reg"

            def is_cc(self):
                return condition

            def is_bit_reg(self):
                if failing == "is_bit_reg":
                    raise RuntimeError("unavailable bit predicate")
                return bit

            def is_kreg(self):
                return kernel

        def has_all(register, width):
            calls.append(("has_all", register, width))
            if failing == "has_all":
                raise RuntimeError("unavailable membership")
            return temporary

        return SimpleNamespace(mop_t=Operand, mop_r=1,
            mreg2reg=lambda register, width: -1,
            get_mreg_name=lambda register, width: "c0" if width == 1 else "rt0",
            is_kreg=lambda register: kernel,
            get_temp_regs=lambda: SimpleNamespace(has_all=has_all)), calls

    def test_bit_register_can_be_distinct_from_condition_kernel_and_temporary(self):
        sdk, calls = self.sdk()
        observed = observer()(sdk, [{"register": 181, "width": 1}])
        row = observed["observations"][0]
        self.assertTrue(observed["complete"])
        self.assertTrue(row["is_bit_register"])
        for name in ("is_condition_code", "is_kernel_register", "global_is_kernel_register",
                     "temporary_register_full_range"):
            self.assertIs(row[name], False)
        self.assertEqual(row["processor_register"], -1)
        self.assertEqual(row["operand_readback"],
                         {"kind": 1, "register": 181, "width": 1, "is_reg": True})
        self.assertNotIn("operand", row)
        self.assertEqual(calls, [("make_reg", 181, 1), ("has_all", 181, 1)])
        self.assertIs(observed["microcode_modified"], False)

    def test_temporary_membership_uses_the_entire_exact_width(self):
        sdk, calls = self.sdk(bit=False, temporary=True)
        row = observer()(sdk, [{"register": 48, "width": 8}])["observations"][0]
        self.assertTrue(row["temporary_register_full_range"])
        self.assertFalse(row["is_bit_register"])
        self.assertEqual(calls[-1], ("has_all", 48, 8))

    def test_changed_operand_identity_cannot_be_complete(self):
        for changed in ("t", "r", "size", "is_reg"):
            with self.subTest(changed=changed):
                sdk, _ = self.sdk(changed=changed)
                result = observer()(sdk, [{"register": 181, "width": 1}])
                self.assertFalse(result["complete"])
                self.assertEqual(result["observations"][0]["errors"][0]["stage"], "operand_readback")

    def test_api_failures_retain_other_actual_observations(self):
        for failing, stage in (("is_bit_reg", "is_bit_register"),
                               ("has_all", "temporary_register_full_range")):
            with self.subTest(failing=failing):
                sdk, _ = self.sdk(failing=failing)
                result = observer()(sdk, [{"register": 181, "width": 1}])
                self.assertFalse(result["complete"])
                row = result["observations"][0]
                self.assertEqual(row["errors"][0]["stage"], stage)
                self.assertEqual(row["processor_register"], -1)
                self.assertNotIn("operand", row)

    def test_invalid_or_duplicate_request_never_constructs_an_operand(self):
        for values in ([], [{"register": -1, "width": 1}], [{"register": 181, "width": 0}],
                       [{"register": True, "width": 1}], [{"register": 181, "width": True}],
                       [{"register": 181, "width": 1, "namespace": "processor"}],
                       [{"register": 181, "width": 1}] * 2,
                       [{"register": x, "width": 1} for x in range(257)]):
            with self.subTest(values=str(values)[:120]):
                sdk, calls = self.sdk()
                with self.assertRaises(ValueError):
                    observer()(sdk, values)
                self.assertEqual(calls, [])

    def test_physical_float_register_preserves_actual_width_and_direct_roundtrip(self):
        sdk, _ = self.sdk(bit=False)
        sdk.mreg2reg = lambda register, width: 45
        sdk.reg2mreg = lambda register: 172

        def parse(info, name):
            self.assertEqual(name, "st7")
            info.reg, info.size = 45, 10
            return True

        processor = SimpleNamespace(get_reg_name=lambda register, width: "st7",
            reg_info_t=SimpleNamespace, parse_reg_name=parse, ph_get_tbyte_size=lambda: 10)
        observed = observer()(sdk, [{"register": 172, "width": 10}], processor)
        row = observed["observations"][0]
        self.assertTrue(observed["complete"])
        self.assertEqual(row["processor_register_info"],
                         {"parsed": True, "register": 45, "bytes": 10})
        self.assertEqual(row["processor_tbyte_bytes"], 10)
        self.assertTrue(row["direct_register_roundtrip_matches"])
        self.assertEqual(row["roundtrip_microregister"], 172)

    def test_physical_register_mismatch_is_retained_without_claiming_roundtrip(self):
        sdk, _ = self.sdk(bit=False)
        sdk.mreg2reg = lambda register, width: 45
        sdk.reg2mreg = lambda register: 164
        row = observer()(sdk, [{"register": 172, "width": 10}])["observations"][0]
        self.assertFalse(row["direct_register_roundtrip_matches"])
        self.assertEqual(row["roundtrip_microregister"], 164)

    def test_missing_processor_register_name_keeps_the_other_sdk_facts(self):
        sdk, _ = self.sdk(bit=False)
        sdk.mreg2reg = lambda register, width: 45
        sdk.reg2mreg = lambda register: 172
        processor = SimpleNamespace(get_reg_name=lambda register, width: "st7",
            reg_info_t=SimpleNamespace, parse_reg_name=lambda info, name: False,
            ph_get_tbyte_size=lambda: 10)
        row = observer()(sdk, [{"register": 172, "width": 10}], processor)["observations"][0]
        self.assertEqual(row["processor_register_info"],
                         {"parsed": False, "register": None, "bytes": None})
        self.assertEqual(row["processor_tbyte_bytes"], 10)

    def test_failed_physical_roundtrip_preserves_forward_mapping(self):
        sdk, _ = self.sdk(bit=False)
        sdk.mreg2reg = lambda register, width: 45

        def missing(register):
            raise RuntimeError("unavailable reverse mapping")

        sdk.reg2mreg = missing
        result = observer()(sdk, [{"register": 172, "width": 10}])
        self.assertFalse(result["complete"])
        row = result["observations"][0]
        self.assertEqual(row["processor_register"], 45)
        self.assertNotIn("direct_register_roundtrip_matches", row)
        self.assertEqual(row["errors"][0]["stage"], "roundtrip_microregister")


if __name__ == "__main__":
    unittest.main()

"""Pure checks for retained x64 arithmetic interpretation and input boundaries."""
from copy import deepcopy
from contextlib import redirect_stdout
from fractions import Fraction
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

import native_arithmetic as arithmetic


class ArithmeticInstructionTests(unittest.TestCase):
    def machine(self, rows, registers=None, limit=100):
        instructions = {}
        address = 0x1000
        for data, text in rows:
            blob = bytes.fromhex(data)
            instructions[address] = {'address': hex(address), 'rva': hex(address-0x1000),
                'text': text, 'bytes': data, 'xrefs_from': []}
            address += len(blob)
        instructions[address] = {'address': hex(address), 'rva': hex(address-0x1000),
                                 'text': 'rep retn', 'bytes': 'f3c3', 'xrefs_from': []}
        memory = arithmetic.BoundedMemory()
        memory.map(0x8000, 0x100)
        memory.write(0x8080, 0x9000, 64)
        program = arithmetic.X64Program(0x1000, b'', (), instructions)
        values = {'rsp': 0x8080}
        values.update(registers or {})
        return arithmetic.X64Machine(program, memory, values, entry=0x1000, stop=0x9000, step_limit=limit)

    def test_32_bit_write_zero_extends_and_adc_preserves_carry_across_mov(self):
        machine = self.machine([('b8ffffffff', 'mov eax, 0FFFFFFFFh'), ('83c001', 'add eax, 1'),
            ('bb00000000', 'mov ebx, 0'), ('83d300', 'adc ebx, 0')], {'rax': arithmetic.MASK})
        machine.run()
        self.assertEqual(machine.regs['rax'], 0)
        self.assertEqual(machine.regs['rbx'], 1)

    def test_inc_preserves_unsigned_carry_and_sub_sets_borrow(self):
        machine = self.machine([('4829c0', 'sub rax, rax'), ('4883e801', 'sub rax, 1'),
            ('48ffc3', 'inc rbx'), ('4883d100', 'adc rcx, 0')], {'rbx': 7})
        machine.run()
        self.assertEqual(machine.regs['rax'], arithmetic.MASK)
        self.assertEqual(machine.regs['rbx'], 8)
        self.assertEqual(machine.regs['rcx'], 1)

    def test_adcx_and_adox_keep_independent_carry_chains(self):
        machine = self.machine([('660f38f6c3', 'adcx rax, rbx'), ('f30f38f6d6', 'adox rdx, rsi')],
                              {'rax': arithmetic.MASK, 'rbx': 0, 'rdx': arithmetic.MASK, 'rsi': 0})
        machine.cf, machine.of = 1, 0
        machine.run()
        self.assertEqual(machine.regs['rax'], 0)
        self.assertEqual(machine.regs['rdx'], arithmetic.MASK)
        self.assertEqual((machine.cf, machine.of), (1, 0))

    def test_mulx_preserves_flags_and_assigns_high_then_low(self):
        machine = self.machine([('c4e2e3f6c1', 'mulx rax, rbx, rcx')],
                              {'rdx': arithmetic.MASK, 'rcx': 3})
        machine.cf, machine.of = 1, 1
        machine.run()
        self.assertEqual(machine.regs['rax'], 2)
        self.assertEqual(machine.regs['rbx'], arithmetic.MASK-2)
        self.assertEqual((machine.cf, machine.of), (1, 1))

    def test_sse_dword_lanes_and_shuffle(self):
        value = 1 | (2 << 32) | (3 << 64) | (4 << 96)
        machine = self.machine([('660f70c11b', 'pshufd xmm0, xmm1, 1Bh'),
                                ('660ffec1', 'paddd xmm0, xmm1')], {'xmm1': value})
        machine.run()
        self.assertEqual(machine.regs['xmm0'], sum(5 << (i*32) for i in range(4)))

    def test_unknown_instruction_and_missing_target_fail(self):
        with self.assertRaisesRegex(ValueError, 'unsupported|rdtsc'):
            self.machine([('0f31', 'rdtsc')]).run()
        machine = self.machine([('e800000000', 'call outside')])
        row = machine.program.instructions[0x1000]
        row['xrefs_from'] = [{'code': True, 'type': 17, 'to': '0x123456'}]
        with self.assertRaisesRegex(ValueError, 'outside retained'):
            machine.run()

    def test_loop_limit_is_a_failure(self):
        machine = self.machine([('ebfe', 'jmp self')], limit=5)
        machine.program.instructions[0x1000]['xrefs_from'] = [{'code': True, 'type': 19, 'to': '0x1000'}]
        with self.assertRaisesRegex(ValueError, 'limit'):
            machine.run()

    def test_memory_rejects_uninitialized_overlapping_and_unmapped_access(self):
        memory = arithmetic.BoundedMemory()
        memory.map(0x1000, 16)
        with self.assertRaisesRegex(ValueError, 'Uninitialized'):
            memory.read(0x1000, 64)
        with self.assertRaisesRegex(ValueError, 'Overlapping'):
            memory.map(0x1008, 16)
        with self.assertRaisesRegex(ValueError, 'outside'):
            memory.write(0x100c, 0, 64)
        memory.write(0x1000, 123, 64)
        memory.regions[0].readonly = True
        with self.assertRaisesRegex(ValueError, 'outside mutable'):
            memory.write(0x1000, 0, 64)

    def test_address_parser_rejects_calls_and_unsupported_operators(self):
        for expression in ('int(123)', '2**3', '__import__(123)', '1/2'):
            with self.subTest(expression=expression), self.assertRaises(ValueError):
                arithmetic.arithmetic_expression(expression)

    def test_montgomery_square_keeps_exact_noncanonical_representative(self):
        # 9^2 + (9^2 * 11 mod 16) * 13 = 224; 224 / 16 = 14.
        self.assertEqual(arithmetic.bounded_montgomery_square(9, 13, 16, 11), 14)
        # 10^2 + (10^2 * 11 mod 16) * 13 = 256; carry selects 16 - 13.
        self.assertEqual(arithmetic.bounded_montgomery_square(10, 13, 16, 11), 3)


class ArithmeticEvidenceTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        raw = bytearray(0x500)
        raw[:2] = b'MZ'
        struct.pack_into('<I', raw, 0x3c, 0x80)
        raw[0x80:0x84] = b'PE\x00\x00'
        struct.pack_into('<HH', raw, 0x84, 0x8664, 1)
        struct.pack_into('<H', raw, 0x94, 0xf0)
        struct.pack_into('<H', raw, 0x98, 0x20b)
        struct.pack_into('<Q', raw, 0xb0, 0x180000000)
        struct.pack_into('<IIII', raw, 0x190, 0x100, 0x1000, 0x100, 0x400)
        raw[0x400:0x408] = bytes.fromhex('e90100000090f3c3')
        self.source = self.write('input.bin', bytes(raw))
        self.function = {'chunks': [{'start': '0x180001000', 'end': '0x180001008',
            'raw_hex': raw[0x400:0x408].hex(), 'sha256': hashlib.sha256(raw[0x400:0x408]).hexdigest()}],
            'assembly': [{'address': '0x180001000', 'rva': '0x1000', 'bytes': 'e901000000',
                'text': 'jmp destination', 'xrefs_from': [{'from': '0x180001000', 'to': '0x180001006',
                                                          'code': True, 'type': 19}]}]}
        self.reference = self.write('capture/functions/1000.json', self.function)
        self.capture_data = {'input': 'input.bin', 'input_sha256': self.source['sha256'],
            'input_bytes': self.source['bytes'], 'image_base': '0x180000000',
            'functions': [{'path': 'functions/1000.json', 'sha256': self.reference['sha256']}]}
        self.capture = self.write('capture/capture.json', self.capture_data)

    def write(self, name, value):
        data = value if isinstance(value, bytes) else json.dumps(value).encode()
        path = self.root/name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return {'path': name, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}

    def load(self):
        return arithmetic.load_program(self.root, self.source, self.capture, [self.reference])

    def update_function(self):
        self.reference = self.write('capture/functions/1000.json', self.function)
        self.capture_data['functions'][0]['sha256'] = self.reference['sha256']
        self.capture = self.write('capture/capture.json', self.capture_data)

    def test_exact_original_bytes_capture_and_relative_edge_bind(self):
        self.assertEqual(len(self.load().instructions), 1)

    def test_original_byte_change_rejects_even_with_new_input_identity(self):
        raw = bytearray((self.root/'input.bin').read_bytes())
        raw[0x400] = 0x90
        self.source = self.write('input.bin', bytes(raw))
        self.capture_data['input_sha256'] = self.source['sha256']
        self.capture = self.write('capture/capture.json', self.capture_data)
        with self.assertRaisesRegex(ValueError, 'chunk differs'):
            self.load()

    def test_relative_target_change_cannot_be_authorized_by_new_receipt_hash(self):
        self.function['assembly'][0]['xrefs_from'][0]['to'] = '0x180001007'
        self.update_function()
        with self.assertRaisesRegex(ValueError, 'Relative edge'):
            self.load()

    def test_function_outside_capture_or_workspace_is_rejected(self):
        self.capture_data['functions'] = []
        self.capture = self.write('capture/capture.json', self.capture_data)
        with self.assertRaisesRegex(ValueError, 'not bound'):
            self.load()
        reference = deepcopy(self.source)
        reference['path'] = '../input.bin'
        with self.assertRaisesRegex(ValueError, 'escapes'):
            arithmetic.identity(self.root, reference)

    def test_case_bounds_reject_inferred_or_invalid_contracts(self):
        good = arithmetic.MontgomeryCase(8, (1 << 512)-569, 1, tuple(range(32)), 0, 0)
        good.validate()
        for key, value in (('limbs', 7), ('modulus', 2), ('operand', -1), ('index', 32),
                           ('capability_flags', -1), ('table', (1, 2)), ('alias_output', 1)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                arithmetic.MontgomeryCase(**{**good.__dict__, key: value}).validate()


class ArithmeticCliTests(unittest.TestCase):
    def test_request_sha_mismatch_never_starts_interpretation(self):
        import analysis
        with tempfile.TemporaryDirectory() as temporary, patch.object(analysis, 'ROOT', Path(temporary)):
            path = Path(temporary) / '.analysis/results/request.json'
            path.parent.mkdir(parents=True)
            path.write_bytes(b'{}')
            args = SimpleNamespace(request='.analysis/results/request.json', request_sha256='0' * 64, workers=1)
            with patch.object(arithmetic, 'verify_power5_batch') as verify:
                with self.assertRaisesRegex(ValueError, 'request SHA-256'):
                    arithmetic.run(analysis, args)
                verify.assert_not_called()

    def test_failed_case_receipt_is_frozen_before_command_reports_failure(self):
        import analysis
        with tempfile.TemporaryDirectory() as temporary, patch.object(analysis, 'ROOT', Path(temporary)):
            path = Path(temporary) / '.analysis/results/request.json'
            path.parent.mkdir(parents=True)
            raw = b'{}'
            path.write_bytes(raw)
            source = Path(arithmetic.__file__).read_bytes()
            result = {'tool': {'path': 'tools/native_arithmetic.py', 'bytes': len(source),
                'sha256': hashlib.sha256(source).hexdigest()}, 'passed': False,
                'summary': {'cases': 1, 'passed': 0, 'failed': 1},
                'cases': [{'passed': False, 'differences': ['exact integer differs']}], 'target_execution': False}
            args = SimpleNamespace(request='.analysis/results/request.json',
                request_sha256=hashlib.sha256(raw).hexdigest(), workers=1)
            output = io.StringIO()
            with patch.object(arithmetic, 'verify_power5_batch', return_value=result), redirect_stdout(output):
                with self.assertRaisesRegex(ValueError, 'retained differences'):
                    arithmetic.run(analysis, args)
            published = json.loads(output.getvalue())
            frozen = Path(temporary) / published['receipt']['path']
            self.assertEqual(hashlib.sha256(frozen.read_bytes()).hexdigest(), published['receipt']['sha256'])
            self.assertEqual(json.loads(frozen.read_bytes())['cases'][0]['differences'], ['exact integer differs'])
            self.assertFalse(published['passed'])

    def test_arithmetic_verifier_is_explicitly_reachable_from_cli(self):
        import analysis
        parser = argparse.ArgumentParser()
        analysis.add_commands(parser.add_subparsers(dest='command', required=True))
        args = parser.parse_args(['verify-native-arithmetic', '--request', '.analysis/results/request.json',
            '--request-sha256', '0' * 64, '--workers', '4'])
        with patch.object(arithmetic, 'run', return_value='receipt') as run:
            self.assertEqual(analysis.run(args), 'receipt')
            run.assert_called_once_with(analysis, args)

    def test_aborted_identity_or_arithmetic_verification_retains_exact_failure(self):
        import analysis
        with tempfile.TemporaryDirectory() as temporary, patch.object(analysis, 'ROOT', Path(temporary)):
            path = Path(temporary) / '.analysis/results/request.json'
            path.parent.mkdir(parents=True)
            raw = b'{"schema_version":3}'
            path.write_bytes(raw)
            args = SimpleNamespace(request='.analysis/results/request.json',
                request_sha256=hashlib.sha256(raw).hexdigest(), workers=1)
            output = io.StringIO()
            with patch.object(arithmetic, 'verify_power5_batch', side_effect=ValueError('original count differs')), \
                    redirect_stdout(output), self.assertRaisesRegex(ValueError, 'retained differences'):
                arithmetic.run(analysis, args)
            published = json.loads(output.getvalue())
            result = json.loads((Path(temporary)/published['receipt']['path']).read_bytes())
            self.assertEqual(result['error'], 'ValueError: original count differs')
            self.assertTrue(result['summary']['verification_aborted'])
            self.assertFalse(result['passed'])
            self.assertEqual((Path(temporary)/result['request_source']['path']).read_bytes(), raw)


class ArithmeticFamilyTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.case = {'limbs': 8, 'modulus': hex((1 << 512)-569), 'operand': '1',
            'table': [str(v) for v in range(32)], 'index': 0, 'capability_flags': 0,
            'scratch_seed': 0, 'alias_output': False}
        self.spec = {'entry_rva': '0x1000', 'capability_rva': '0x2000',
                     'optimized_mask': '0x80108', 'completion_rvas': ['0x3000']}
        self.modules = [{'source_input': {'path': name, 'bytes': 1, 'sha256': digit*64},
            'source_capture': {'path': name+'.json'}, 'functions': [{'path': name+'/1000.json'}],
            'power5': self.spec} for name, digit in (('first.bin', '1'), ('second.bin', '2'))]
        raw = json.dumps({'schema_version': 1, **self.modules[0], 'cases': [self.case]}).encode()
        (self.root/'cases.json').write_bytes(raw)
        self.request = {'schema_version': 2,
            'cases_source': {'path': 'cases.json', 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()},
            'modules': self.modules}
        self.success = {'passed': True, 'limbs': 8, 'capability_flags': '0x0', 'alias_output': False,
            'table_read_trace_sha256': '3'*64, 'instruction_visit_counts_sha256': '4'*64}

    def test_each_input_is_loaded_and_compared_independently(self):
        with patch.object(arithmetic, 'load_program', side_effect=['first', 'second']) as load, \
                patch.object(arithmetic, 'verify_power5_case', return_value=self.success) as verify:
            result = arithmetic.verify_power5_batch(self.root, self.request, workers=1)
        self.assertEqual([call.args[1]['path'] for call in load.call_args_list], ['first.bin', 'second.bin'])
        self.assertEqual([call.args[0] for call in verify.call_args_list], ['first', 'second'])
        self.assertEqual(result['summary'], {'modules': 2, 'passed_modules': 2, 'failed_modules': 0,
            'cases': 2, 'passed': 2, 'failed': 0, 'unexecuted': 0})
        self.assertTrue(result['passed'])
        self.assertFalse(result['target_execution'])

    def test_shared_case_identity_change_cannot_start_interpretation(self):
        (self.root/'cases.json').write_bytes(b'{}')
        with patch.object(arithmetic, 'load_program') as load, self.assertRaisesRegex(ValueError, 'identity changed'):
            arithmetic.verify_power5_batch(self.root, self.request, workers=1)
        load.assert_not_called()

    def test_duplicate_inputs_and_unknown_module_fields_are_rejected(self):
        for mutate in (lambda r: r['modules'].__setitem__(1, deepcopy(r['modules'][0])),
                       lambda r: r['modules'][1].update(assume_same_family=True)):
            request = deepcopy(self.request)
            mutate(request)
            with self.subTest(request=request), self.assertRaises(ValueError):
                arithmetic.verify_power5_batch(self.root, request, workers=1)

    def test_invalid_module_preserves_unexecuted_cases_and_valid_module_result(self):
        with patch.object(arithmetic, 'load_program', side_effect=[ValueError('Original changed'), object()]), \
                patch.object(arithmetic, 'verify_power5_case', return_value=self.success):
            result = arithmetic.verify_power5_batch(self.root, self.request, workers=1)
        self.assertFalse(result['passed'])
        self.assertEqual(result['summary'], {'modules': 2, 'passed_modules': 1, 'failed_modules': 1,
            'cases': 2, 'passed': 1, 'failed': 0, 'unexecuted': 1})
        self.assertIn('Original changed', result['modules'][0]['error'])
        self.assertTrue(result['modules'][1]['cases'][0]['passed'])

    def test_case_difference_and_instruction_failure_both_remain_failures(self):
        difference = {**self.success, 'passed': False, 'differences': ['exact integer differs']}
        with patch.object(arithmetic, 'load_program', return_value=object()), \
                patch.object(arithmetic, 'verify_power5_case', side_effect=[difference, ValueError('unknown instruction')]):
            result = arithmetic.verify_power5_batch(self.root, self.request, workers=1)
        self.assertEqual(result['summary']['failed'], 2)
        self.assertEqual(result['summary']['unexecuted'], 0)
        self.assertEqual(result['modules'][0]['cases'][0]['differences'], ['exact integer differs'])
        self.assertIn('unknown instruction', result['modules'][1]['cases'][0]['error'])
        self.assertFalse(result['passed'])

    def test_case_source_changed_during_verification_is_rejected(self):
        def compare(*args):
            (self.root/'cases.json').write_bytes(b'changed')
            return self.success
        with patch.object(arithmetic, 'load_program', return_value=object()), \
                patch.object(arithmetic, 'verify_power5_case', side_effect=compare), \
                self.assertRaisesRegex(ValueError, 'identity changed'):
            arithmetic.verify_power5_batch(self.root, self.request, workers=1)


class ExactPeakMeterPreludeTests(unittest.TestCase):
    def test_binary_rounding_ties_and_directions_use_exact_bits(self):
        halfway = Fraction(1)+Fraction(1, 1 << 24)
        for value, nearest, down, up, zero in ((halfway, 0x3f800000, 0x3f800000, 0x3f800001, 0x3f800000),
                (-halfway, 0xbf800000, 0xbf800001, 0xbf800000, 0xbf800000)):
            for mode, expected in zip(('nearest_even', 'down', 'up', 'zero'), (nearest, down, up, zero)):
                with self.subTest(value=value, mode=mode):
                    bits = arithmetic.round_ieee(value, 32, mode)
                    self.assertEqual(bits, expected)
                    arithmetic.verify_rounding_interval(value, bits, 32, mode)
        odd_halfway = Fraction(1)+Fraction(3, 1 << 24)
        self.assertEqual(arithmetic.round_ieee(odd_halfway, 32, 'nearest_even'), 0x3f800002)

    def test_normal_subnormal_boundary_retains_exact_nearest_even(self):
        halfway = Fraction(1, 1 << 126)-Fraction(1, 1 << 150)
        self.assertEqual(arithmetic.round_ieee(halfway, 32, 'nearest_even'), 0x00800000)
        self.assertEqual(arithmetic.round_ieee(halfway, 32, 'down'), 0x007fffff)
        arithmetic.verify_rounding_interval(halfway, 0x00800000, 32, 'nearest_even')

    def test_interval_oracle_rejects_wrong_result_and_tie_direction(self):
        with self.assertRaisesRegex(ValueError, 'interval'):
            arithmetic.verify_rounding_interval(Fraction(1)+Fraction(1, 1 << 24), 0x3f800001, 32, 'nearest_even')
        with self.assertRaisesRegex(ValueError, 'interval'):
            arithmetic.verify_rounding_interval(Fraction(-1)-Fraction(1, 1 << 24), 0xbf800001, 32, 'zero')
        with self.assertRaisesRegex(ValueError, 'finite'):
            arithmetic.ieee_fraction(0x7fc00000, 32)

    def test_counter_tracks_32_bit_multiply_carry_borrow_and_double_shift(self):
        rows = ['mov eax, [ebp+InBuffer]', 'mov ecx, 2EE000h', 'mul ecx',
                'mov ecx, eax', 'sub ecx, 1', 'sbb edx, 0', 'add ecx, 1', 'adc edx, 0',
                'shrd ecx, edx, 10', 'shr edx, 10', 'call __ultod3']
        instructions = [{'text': v} for v in rows]
        for parameter in (0, 1, 1398, 1399, 0x7fffffff, 0xfffffffe, 0xffffffff):
            with self.subTest(parameter=parameter):
                self.assertEqual(arithmetic.interpret_peak_counter(instructions, parameter),
                                 (3072000*parameter) >> 10)
        with self.assertRaisesRegex(ValueError, 'Unknown retained'):
            arithmetic.interpret_peak_counter([{'text': 'rdtsc now'}], 1)
        with self.assertRaisesRegex(ValueError, 'uint32'):
            arithmetic.interpret_peak_counter(instructions, -1)

    def test_nonzero_peak_exponent_is_normal_under_all_masked_rounding_modes(self):
        for parameter in (1, 2, 65535, 1 << 24, (1 << 32)-1):
            for mode in ('nearest_even', 'down', 'up', 'zero'):
                count_bits = arithmetic.round_ieee(Fraction(3072*parameter), 32, mode)
                quotient = Fraction(-6)/arithmetic.ieee_fraction(count_bits, 32)
                quotient_bits = arithmetic.round_ieee(quotient, 32, mode)
                arithmetic.verify_rounding_interval(quotient, quotient_bits, 32, mode)
                exponent = arithmetic.ieee_fraction(quotient_bits, 32)/20
                bits = arithmetic.round_ieee(exponent, 32, mode)
                arithmetic.verify_rounding_interval(exponent, bits, 32, mode)
                self.assertGreater((bits >> 23) & 255, 0)
                self.assertLess((bits >> 23) & 255, 255)


if __name__ == '__main__':
    unittest.main()

"""Bounded pure arithmetic interpretation of retained x64 analysis records.

The supported instructions cover reviewed integer, SSE2, BMI2 and ADX arithmetic
chains. Unknown instructions, memory and call targets fail closed. This module
never maps executable memory, loads a target library, or executes target code.
It trusts the mnemonic/operand interpretation of the identity-bound IDA records;
matching original bytes does not establish a separate instruction decoder.
"""
from __future__ import annotations

import ast
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass
from fractions import Fraction
import hashlib
import json
import multiprocessing
from pathlib import Path
import re
import struct
from typing import Callable

MASK = (1 << 64) - 1
ALIASES = {'eax':'rax', 'ebx':'rbx', 'ecx':'rcx', 'edx':'rdx', 'esi':'rsi', 'edi':'rdi', 'ebp':'rbp', 'esp':'rsp'}
ALIASES.update({f'r{i}d':f'r{i}' for i in range(8,16)})
REGEX = re.compile(r'\b(?:r(?:ax|bx|cx|dx|si|di|bp|sp|[89]|1[0-5])|e(?:ax|bx|cx|dx|si|di|bp|sp)|r(?:[89]|1[0-5])d)\b')
REGISTERS = ('rax','rbx','rcx','rdx','rsi','rdi','rbp','rsp', *(f'r{i}' for i in range(8,16)))
NONVOLATILE = ('rbx','rbp','rdi','rsi','r12','r13','r14','r15')


def width(op: str) -> int:
    if op in ALIASES:
        return 32
    if op.startswith('xmm') or 'xmmword ptr' in op:
        return 128
    if 'dword ptr' in op and 'qword' not in op:
        return 32
    return 64


def arithmetic_expression(expression: str) -> int:
    def visit(node):
        if isinstance(node, ast.Constant) and type(node.value) is int:
            return node.value
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.UAdd, ast.USub)):
            value = visit(node.operand)
            return -value if isinstance(node.op, ast.USub) else value
        if isinstance(node, ast.BinOp):
            left, right = visit(node.left), visit(node.right)
            if isinstance(node.op, ast.Add):
                return left + right
            if isinstance(node.op, ast.Sub):
                return left - right
            if isinstance(node.op, ast.Mult):
                return left * right
        raise ValueError('Unsupported arithmetic address expression')
    return visit(ast.parse(expression, mode='eval').body)


def identity(root: Path, reference: dict) -> tuple[Path, bytes]:
    name = reference.get('path')
    if not isinstance(name, str) or not name or '\\' in name:
        raise ValueError('Expected project-relative evidence identity')
    path = root / name
    if Path(name).is_absolute() or '..' in Path(name).parts or not path.resolve().is_relative_to(root.resolve()):
        raise ValueError('Evidence path escapes workspace')
    if any(p.is_symlink() or (hasattr(p, 'is_junction') and p.is_junction()) for p in (path, *path.parents) if p != root.parent):
        raise ValueError('Evidence path follows a filesystem link')
    data = path.read_bytes()
    if reference.get('bytes') != len(data) or reference.get('sha256') != hashlib.sha256(data).hexdigest():
        raise ValueError('Evidence identity changed: ' + name)
    return path, data


@dataclass(frozen=True)
class X64Program:
    image_base: int
    original: bytes
    sections: tuple[tuple[int, int, int, int], ...]
    instructions: dict[int, dict]

    def original_bytes(self, address: int, size: int) -> bytes:
        rva = address - self.image_base
        for virtual_size, start, raw_size, offset in self.sections:
            if start <= rva and rva + size <= start + raw_size:
                data = self.original[offset+rva-start:offset+rva-start+size]
                if len(data) == size:
                    return data
        raise ValueError(f'Original PE read is not file-backed: {address:#x}+{size}')


def load_program(root: Path, source: dict, capture: dict, functions: list[dict]) -> X64Program:
    _, raw = identity(root, source)
    _, capture_raw = identity(root, capture)
    manifest = json.loads(capture_raw)
    if (manifest.get('input') != source['path'] or manifest.get('input_sha256') != source['sha256']
            or manifest.get('input_bytes') != source['bytes']):
        raise ValueError('Capture does not bind exact original input')
    if raw[:2] != b'MZ' or len(raw) < 0x40:
        raise ValueError('Expected x64 PE original')
    pe = struct.unpack_from('<I', raw, 0x3c)[0]
    if raw[pe:pe+4] != b'PE\x00\x00' or struct.unpack_from('<H', raw, pe+4)[0] != 0x8664:
        raise ValueError('Arithmetic records require AMD64 PE')
    count = struct.unpack_from('<H', raw, pe+6)[0]
    optional_size = struct.unpack_from('<H', raw, pe+20)[0]
    optional = pe+24
    if struct.unpack_from('<H', raw, optional)[0] != 0x20b:
        raise ValueError('Expected PE32+ optional header')
    image_base = struct.unpack_from('<Q', raw, optional+24)[0]
    if manifest.get('image_base') != hex(image_base):
        raise ValueError('Image base differs from original PE')
    sections = tuple(struct.unpack_from('<IIII', raw, o+8)
                     for o in range(optional+optional_size, optional+optional_size+40*count, 40))
    program = X64Program(image_base, raw, sections, {})
    declared = {row['path']: row for row in manifest['functions']}
    capture_directory = Path(capture['path']).parent
    for reference in functions:
        path, record_raw = identity(root, reference)
        relative = path.relative_to(root / capture_directory).as_posix()
        if reference['sha256'] != declared.get(relative, {}).get('sha256'):
            raise ValueError('Function is not bound by retained capture index')
        record = json.loads(record_raw)
        chunks = []
        for chunk in record['chunks']:
            start, end = int(chunk['start'], 0), int(chunk['end'], 0)
            data = bytes.fromhex(chunk['raw_hex'])
            if (end <= start or len(data) != end-start or hashlib.sha256(data).hexdigest() != chunk['sha256']
                    or program.original_bytes(start, len(data)) != data):
                raise ValueError('Complete function chunk differs from original PE')
            chunks.append((start, end))
        for instruction in record['assembly']:
            address = int(instruction['address'], 0)
            data = bytes.fromhex(instruction['bytes'])
            if (not data or not any(start <= address and address+len(data) <= end for start, end in chunks)
                    or program.original_bytes(address, len(data)) != data):
                raise ValueError('Instruction differs from original complete chunk')
            if address in program.instructions and program.instructions[address] != instruction:
                raise ValueError('Overlapping instruction interpretations conflict')
            for edge in instruction['xrefs_from']:
                if edge['code'] and edge['type'] in (17, 19):
                    if int(edge['from'], 0) != address:
                        raise ValueError('Direct edge source changed')
                    if data[0] in (0xe8, 0xe9) and len(data) == 5:
                        target = address + 5 + int.from_bytes(data[1:], 'little', signed=True)
                    elif data[0] in (0xeb, 0xe3, *range(0x70, 0x80)) and len(data) == 2:
                        target = address + 2 + int.from_bytes(data[1:], 'little', signed=True)
                    elif len(data) == 6 and data[0] == 0x0f and 0x80 <= data[1] <= 0x8f:
                        target = address + 6 + int.from_bytes(data[2:], 'little', signed=True)
                    else:
                        raise ValueError('Unsupported exact direct-edge encoding')
                    if target != int(edge['to'], 0):
                        raise ValueError('Relative edge differs from original bytes')
            program.instructions[address] = instruction
    spans = sorted((address, address+len(bytes.fromhex(row['bytes']))) for address, row in program.instructions.items())
    if any(a_end > b_start for (_, a_end), (b_start, _) in zip(spans, spans[1:])):
        raise ValueError('Instruction byte ranges overlap')
    return program


@dataclass
class MemoryRegion:
    start: int
    size: int
    fill_seed: int | None
    readonly: bool = False


class BoundedMemory:
    def __init__(self, original_reader=None):
        self.regions: list[MemoryRegion] = []
        self.values: dict[int, int] = {}
        self.original_reader = original_reader

    def map(self, start, size, *, fill_seed=None):
        if size <= 0 or start < 0 or start+size > 1 << 64:
            raise ValueError('Invalid mapped memory range')
        if any(start < r.start+r.size and r.start < start+size for r in self.regions):
            raise ValueError('Overlapping memory maps')
        self.regions.append(MemoryRegion(start, size, fill_seed))

    def region(self, address, size):
        return next((r for r in self.regions if r.start <= address and address+size <= r.start+r.size), None)

    def read(self, address, bits):
        if bits not in (8, 16, 32, 64, 128) and bits % 64:
            raise ValueError('Unsupported memory width')
        size = bits//8
        region = self.region(address, size)
        if region is None:
            if self.original_reader is not None:
                return int.from_bytes(self.original_reader(address, size), 'little')
            raise ValueError(f'Read outside mapped memory: {address:#x}+{size}')
        if region.fill_seed is None and not all(address+i in self.values for i in range(size)):
            raise ValueError(f'Uninitialized mapped memory: {address:#x}+{size}')
        return int.from_bytes(bytes(self.values.get(address+i, ((address+i)*37+(region.fill_seed or 0))&255)
                                    for i in range(size)), 'little')

    def write(self, address, value, bits):
        size = bits//8
        region = self.region(address, size)
        if region is None or region.readonly:
            raise ValueError(f'Write outside mutable mapped memory: {address:#x}+{size}')
        value &= (1 << bits)-1
        self.values.update((address+i, byte) for i, byte in enumerate(value.to_bytes(size, 'little')))


class X64Machine:
    def __init__(self, program: X64Program, memory: BoundedMemory, registers: dict[str, int],
                 *, entry: int, stop: int, step_limit=3_000_000,
                 on_return: Callable | None = None, trace_read_range=None):
        if type(step_limit) is not int or step_limit < 1 or step_limit > 10_000_000:
            raise ValueError('Expected bounded arithmetic simulation step limit')
        self.program, self.memory = program, memory
        self.regs = {r: 0 for r in (*REGISTERS, *(f'xmm{i}' for i in range(16)))}
        if not set(registers).issubset(self.regs):
            raise ValueError('Unsupported initial register')
        for name, value in registers.items():
            if type(value) is not int or not 0 <= value < 1 << width(name):
                raise ValueError('Initial register value outside width')
        self.regs.update(registers)
        self.cf = self.of = self.zf = self.sf = 0
        self.steps = 0
        self.visits = Counter()
        self.calls = []
        self.pc, self.stop, self.step_limit = entry, stop, step_limit
        self.on_return, self.trace_read_range = on_return, trace_read_range
        self.table_reads = []
        self.interpreting = False

    def read(self, address, bits):
        if self.interpreting and self.trace_read_range:
            start, end = self.trace_read_range
            if start <= address < end:
                self.table_reads.append((address-start, bits//8))
        return self.memory.read(address, bits)

    def write(self, address, value, bits):
        self.memory.write(address, value, bits)

    def reg(self,name): return self.regs[ALIASES.get(name,name)]&((1<<width(name))-1)
    def setreg(self,name,value): self.regs[ALIASES.get(name,name)]=value&((1<<width(name))-1)
    def ea(self,op):
        if '[' not in op:
            sym=re.search(r'(?:xmmword|qword|dword|byte|off|unk|loc)_(?:0x)?([0-9A-Fa-f]+)',op)
            if sym:return int(sym[1],16)
            raise ValueError(('address',op))
        expr=op.split('[',1)[1].split(']',1)[0]
        expr=re.sub(r'var_([0-9A-Fa-f]+)',lambda m:str(-int(m[1],16)),expr)
        expr=re.sub(r'arg_([0-9A-Fa-f]+)',lambda m:str(8+int(m[1],16)),expr)
        expr=re.sub(r'\b([0-9][0-9A-Fa-f]*)h\b',lambda m:str(int(m[1],16)),expr)
        expr=REGEX.sub(lambda m:str(self.reg(m[0])),expr)
        if not re.fullmatch(r'[0-9+*() -]+',expr):
            raise ValueError('Unsupported address expression: ' + expr)
        return arithmetic_expression(expr) & MASK
    def get(self,op,bits=None):
        if op in self.regs or op in ALIASES:return self.reg(op)
        if '[' in op or 'cs:' in op:return self.read(self.ea(op),bits or width(op))
        if op.startswith('offset '):return self.ea(op)
        return int(op[:-1],16) if op.endswith('h') else int(op,0)
    def put(self,op,value,bits):
        if op in self.regs or op in ALIASES:self.setreg(op,value)
        else:self.write(self.ea(op),value,bits)
    def flags(self,a,b,result,bits,sub=False,carry=0):
        mask=(1<<bits)-1
        v=result&mask
        self.cf=int(a < b+carry) if sub else int(result>mask)
        self.of=int(bool(((a^b)&(a^v))&(1<<(bits-1)))) if sub else int(bool((~(a^b)&(a^v))&(1<<(bits-1))))
        self.zf=int(v==0);self.sf=(v>>(bits-1))&1
    def branch(self,ins):
        edges=[x for x in ins['xrefs_from'] if x['code'] and x['type'] in (17,19)]
        if len(edges) != 1:
            raise ValueError(f'Expected one exact direct edge: {self.pc:#x}')
        return int(edges[0]['to'],0)
    def run(self):
        self.interpreting = True
        while self.pc!=self.stop:
            if self.steps >= self.step_limit:raise ValueError('instruction simulation limit')
            try:
                ins=self.program.instructions[self.pc]
            except KeyError as error:
                raise ValueError(f'Instruction outside retained program: {self.pc:#x}') from error
            self.steps+=1;self.visits[self.pc]+=1
            self.next=self.pc+len(bytes.fromhex(ins['bytes']))
            text=ins['text'].split(';',1)[0].strip()
            if text=='rep retn':mnemonic='ret';ops=[]
            else:
                parts=text.split(None,1);mnemonic=parts[0];ops=[x.strip() for x in parts[1].split(',')] if len(parts)>1 else []
            m=mnemonic
            bits=width(ops[0]) if ops else 64
            if m in ('mov','movq','movd','movdqa'):
                if m=='movq':bits=64
                elif m=='movd':bits=32
                elif m=='movdqa':bits=128
                elif ops[0] not in self.regs and ops[0] not in ALIASES and 'ptr' not in ops[0]:bits=width(ops[1])
                self.put(ops[0],self.get(ops[1],bits)&((1<<bits)-1),bits)
            elif m=='lea':self.put(ops[0],self.ea(ops[1]),bits)
            elif m=='push':self.regs['rsp']-=8;self.write(self.regs['rsp'],self.get(ops[0]),64)
            elif m=='call':
                target=self.branch(ins);self.calls.append((self.pc,target))
                self.regs['rsp']-=8;self.write(self.regs['rsp'],self.next,64);self.next=target
            elif m=='ret':
                self.next=self.read(self.regs['rsp'],64);self.regs['rsp']+=8
                if self.on_return is not None:
                    self.on_return(self, self.pc)
            elif m=='jmp':self.next=self.branch(ins)
            elif m in ('jz','jnz','jb','jnb','ja','jrcxz'):
                yes={'jz':bool(self.zf),'jnz':not self.zf,'jb':bool(self.cf),'jnb':not self.cf,'ja':not self.cf and not self.zf,'jrcxz':self.regs['rcx']==0}[m]
                if yes:self.next=self.branch(ins)
            elif m=='cmovb':
                if self.cf:self.put(ops[0],self.get(ops[1],bits),bits)
            elif m in ('add','adc','sub','sbb','cmp'):
                a,b=self.get(ops[0],bits),self.get(ops[1],bits);carry=self.cf if m in ('adc','sbb') else 0
                sub=m in ('sub','sbb','cmp');result=a-b-carry if sub else a+b+carry
                self.flags(a,b,result,bits,sub,carry)
                if m!='cmp':self.put(ops[0],result,bits)
            elif m in ('adcx','adox'):
                flag=self.cf if m=='adcx' else self.of
                result=self.get(ops[0],bits)+self.get(ops[1],bits)+flag
                if m=='adcx':self.cf=int(result>>bits!=0)
                else:self.of=int(result>>bits!=0)
                self.put(ops[0],result,bits)
            elif m in ('inc','dec'):
                a=self.get(ops[0],bits);b=1;cf=self.cf;result=a+1 if m=='inc' else a-1
                self.flags(a,b,result,bits,m=='dec');self.cf=cf;self.put(ops[0],result,bits)
            elif m=='neg':
                a=self.get(ops[0],bits);result=-a;self.flags(0,a,result,bits,True);self.put(ops[0],result,bits)
            elif m=='not':self.put(ops[0],~self.get(ops[0],bits),bits)
            elif m in ('xor','pxor','and','or','pand','por','andn'):
                a,b=self.get(ops[0],bits),self.get(ops[1],bits)
                if m in ('xor','pxor'):value=a^b
                elif m in ('and','pand'):value=a&b
                elif m in ('or','por'):value=a|b
                else:value=(~b)&self.get(ops[2],bits)
                self.put(ops[0],value,bits)
                if m not in ('pxor','pand','por'):
                    self.cf=self.of=0;self.zf=int(value&((1<<bits)-1)==0);self.sf=(value>>(bits-1))&1
            elif m in ('shl','shr','sar'):
                a=self.get(ops[0],bits);shift=self.get(ops[1])&(63 if bits==64 else 31)
                if shift:
                    self.cf=(a>>(bits-shift))&1 if m=='shl' else (a>>(shift-1))&1
                    signed=a-(1<<bits) if a>>(bits-1) else a
                    value=a<<shift if m=='shl' else signed>>shift if m=='sar' else a>>shift
                    self.put(ops[0],value,bits);value&=(1<<bits)-1
                    self.zf=int(value==0);self.sf=(value>>(bits-1))&1
                    self.of=(self.sf^self.cf) if m=='shl' and shift==1 else (a>>(bits-1))&1 if m=='shr' and shift==1 else 0
            elif m=='mul':
                result=self.regs['rax']*self.get(ops[0],64);self.regs['rax']=result&MASK;self.regs['rdx']=result>>64;self.cf=self.of=int(result>>64!=0)
            elif m=='mulx':
                result=self.regs['rdx']*self.get(ops[2],64);self.put(ops[0],result>>64,64);self.put(ops[1],result&MASK,64)
            elif m=='imul':
                a,b=self.get(ops[0],bits),self.get(ops[1],bits)
                a=a-(1<<bits) if a>>(bits-1) else a;b=b-(1<<bits) if b>>(bits-1) else b
                result=a*b;self.cf=self.of=int(not -(1<<(bits-1))<=result<1<<(bits-1));self.put(ops[0],result,bits)
            elif m in ('pcmpeqd','paddd','pshufd'):
                a,b=self.get(ops[0],128),self.get(ops[1],128)
                aa=[(a>>(32*i))&0xffffffff for i in range(4)];bb=[(b>>(32*i))&0xffffffff for i in range(4)]
                if m=='pcmpeqd':words=[0xffffffff if x==y else 0 for x,y in zip(aa,bb)]
                elif m=='paddd':words=[(x+y)&0xffffffff for x,y in zip(aa,bb)]
                else:
                    imm=self.get(ops[2]);words=[bb[(imm>>(2*i))&3] for i in range(4)]
                self.put(ops[0],sum(v<<(32*i) for i,v in enumerate(words)),128)
            elif m=='nop':pass
            else:raise ValueError((hex(self.pc-self.program.image_base),text))
            self.pc=self.next
        return self


@dataclass(frozen=True)
class MontgomeryPower5Spec:
    entry_rva: int
    capability_rva: int
    optimized_mask: int
    completion_rvas: tuple[int, ...]

    @classmethod
    def parse(cls, value):
        if set(value) != {'entry_rva', 'capability_rva', 'optimized_mask', 'completion_rvas'}:
            raise ValueError('Unexpected Montgomery power5 specification fields')
        number = lambda v: int(v, 0) if isinstance(v, str) else v
        result = cls(number(value['entry_rva']), number(value['capability_rva']),
                     number(value['optimized_mask']), tuple(number(v) for v in value['completion_rvas']))
        if (any(type(v) is not int or not 0 < v < 1 << 32
                for v in (result.entry_rva, result.capability_rva, result.optimized_mask, *result.completion_rvas))
                or not result.completion_rvas or len(set(result.completion_rvas)) != len(result.completion_rvas)):
            raise ValueError('Invalid Montgomery power5 address or mask')
        return result


@dataclass(frozen=True)
class MontgomeryCase:
    limbs: int
    modulus: int
    operand: int
    table: tuple[int, ...]
    index: int
    capability_flags: int
    scratch_seed: int = 0
    alias_output: bool = False

    def validate(self):
        if type(self.limbs) is not int or self.limbs < 8 or self.limbs > 128 or self.limbs % 8:
            raise ValueError('Reviewed power5 path requires 8 to 128 limbs in multiples of 8')
        radix = 1 << (64*self.limbs)
        if (type(self.modulus) is not int or not 1 < self.modulus < radix or not self.modulus & 1
                or type(self.operand) is not int or not 0 <= self.operand < self.modulus
                or len(self.table) != 32 or any(type(v) is not int or not 0 <= v < self.modulus for v in self.table)
                or type(self.index) is not int or not 0 <= self.index < 32
                or type(self.capability_flags) is not int or not 0 <= self.capability_flags < 1 << 32
                or type(self.scratch_seed) is not int or not 0 <= self.scratch_seed < 256
                or type(self.alias_output) is not bool):
            raise ValueError('Case is outside reviewed Montgomery input contract')


def verify_power5_case(program: X64Program, spec: MontgomeryPower5Spec, case: MontgomeryCase) -> dict:
    """Compare every retained square/correction return against big integers.

    Case bounds describe only the reviewed caller's multiple-of-eight path.
    Rejecting a case here makes no claim about another firmware or product ABI.
    """
    case.validate()
    n = case.limbs
    output = 0x30000000 if case.alias_output else 0x20000000
    source, table, modulus, inverse, stack = 0x30000000, 0x40000000, 0x50000000, 0x60000000, 0x70010008
    stop = 0xffff000000000001
    memory = BoundedMemory(program.original_bytes)
    if output != source:
        memory.map(output, n*8, fill_seed=case.scratch_seed)
    memory.map(source, n*8)
    memory.map(table, n*256)
    memory.map(modulus, n*8)
    memory.map(inverse, 8)
    memory.map(stack-0x100000, 0x100080, fill_seed=case.scratch_seed)
    memory.map(program.image_base+spec.capability_rva, 4)
    for address, value, bits in ((stack, stop, 64), (stack+40, inverse, 64), (stack+48, n, 64),
            (stack+56, case.index, 32), (inverse, -pow(case.modulus, -1, 1 << 64), 64),
            (program.image_base+spec.capability_rva, case.capability_flags, 32),
            (source, case.operand, n*64), (modulus, case.modulus, n*64)):
        memory.write(address, value, bits)
    for index, value in enumerate(case.table):
        for limb in range(n):
            memory.write(table+limb*256+index*8, (value >> (64*limb)) & MASK, 64)
    registers = {r: 0x123456789abcdef0+i for i, r in enumerate(REGISTERS)}
    registers.update(rcx=output, rdx=source, r8=table, r9=modulus, rsp=stack)
    saved = {r: registers[r] for r in NONVOLATILE}
    stages = []

    def on_return(machine, address):
        if address-program.image_base in spec.completion_rvas:
            stages.append(machine.read(output, n*64))

    machine = X64Machine(program, memory, registers, entry=program.image_base+spec.entry_rva,
                         stop=stop, on_return=on_return, trace_read_range=(table, table+n*256))
    machine.run()
    if machine.regs['rsp'] != stack+8 or machine.regs['rax'] != 1 or any(machine.regs[k] != v for k, v in saved.items()):
        raise ValueError('Original return value, stack or nonvolatile registers did not restore')
    radix = 1 << (64*n)
    reciprocal = pow(radix, -1, case.modulus)
    negative_inverse = -pow(case.modulus, -1, radix) % radix
    current = case.operand
    expected_stages = []
    for _ in range(5):
        current = bounded_montgomery_square(current, case.modulus, radix, negative_inverse)
        expected_stages.append(current)
    product = current*case.table[case.index]
    correction = (product*negative_inverse) % radix
    final = (product+correction*case.modulus)//radix
    if final >= case.modulus:
        final -= case.modulus
    expected_stages.append(final)
    closed_form = (pow(case.operand, 32, case.modulus)*case.table[case.index]
                   *pow(radix, -32, case.modulus)) % case.modulus
    result = memory.read(output, n*64)
    passed = stages == expected_stages and result == closed_form and result == expected_stages[-1]
    values = [hex(value) for value in stages]
    trace = json.dumps(machine.table_reads, separators=(',', ':')).encode()
    instructions = json.dumps(sorted((hex(a-program.image_base), count) for a, count in machine.visits.items()),
                              separators=(',', ':')).encode()
    return {'limbs': n, 'index': case.index, 'capability_flags': hex(case.capability_flags),
            'optimized_selected': (case.capability_flags & spec.optimized_mask) == spec.optimized_mask,
            'scratch_seed': case.scratch_seed, 'alias_output': case.alias_output,
            'modulus': hex(case.modulus), 'operand': hex(case.operand), 'table': [hex(v) for v in case.table],
            'actual_stages': values, 'expected_stages': [hex(v) for v in expected_stages],
            'closed_form_expected': hex(closed_form), 'result': hex(result),
            'steps': machine.steps, 'visited_instructions': len(machine.visits), 'calls': len(machine.calls),
            'call_rvas': [[hex(a-program.image_base), hex(b-program.image_base)] for a, b in machine.calls],
            'table_read_count': len(machine.table_reads), 'table_read_trace_sha256': hashlib.sha256(trace).hexdigest(),
            'instruction_visit_counts_sha256': hashlib.sha256(instructions).hexdigest(),
            'stack_and_nonvolatile_restored': True, 'return_value': machine.regs['rax'], 'passed': passed,
            'differences': [] if passed else ['Retained arithmetic differs from independent Montgomery expectation'],
            'target_execution': False, 'official_dll_comparison': 'not_executed', 'real_device_acceptance': 'not_executed'}


def bounded_montgomery_square(operand: int, modulus: int, radix: int, negative_inverse: int) -> int:
    """Match the reviewed carry-only square correction before final normalization.

    The square/correction helpers select subtraction from the extra radix carry.
    They keep a value in [0, radix), including values greater than the modulus.
    The multiplication/gather chain performs the final canonical subtraction.
    """
    product = operand*operand
    correction = (product*negative_inverse) % radix
    result = (product+correction*modulus)//radix
    return result-modulus if result >= radix else result


def parse_case(row: dict) -> MontgomeryCase:
    required = {'limbs', 'modulus', 'operand', 'table', 'index', 'capability_flags', 'scratch_seed', 'alias_output'}
    if set(row) != required:
        raise ValueError('Unexpected Montgomery case fields')
    number = lambda v: int(v, 0) if isinstance(v, str) else v
    case = MontgomeryCase(number(row['limbs']), number(row['modulus']), number(row['operand']),
        tuple(number(v) for v in row['table']), number(row['index']), number(row['capability_flags']),
        number(row['scratch_seed']), row['alias_output'])
    case.validate()
    return case


_WORKER_PROGRAM = None
_WORKER_SPEC = None


def _initialize_batch(root, request):
    global _WORKER_PROGRAM, _WORKER_SPEC
    _WORKER_PROGRAM = load_program(Path(root), request['source_input'], request['source_capture'], request['functions'])
    _WORKER_SPEC = MontgomeryPower5Spec.parse(request['power5'])


def _verify_batch_case(row):
    try:
        return verify_power5_case(_WORKER_PROGRAM, _WORKER_SPEC, parse_case(row))
    except Exception as error:
        return {'passed': False, 'input': row, 'error': type(error).__name__+': '+str(error),
                'target_execution': False, 'official_dll_comparison': 'not_executed'}


def verify_power5_batch(root: Path, request: dict, *, workers: int = 4) -> dict:
    """Return a resumable caller-owned receipt without writing or replacing files."""
    if request.get('schema_version') == 2:
        return verify_power5_family(root, request, workers=workers)
    if request.get('schema_version') == 3:
        return verify_peak_meter_prelude(root, request)
    required = {'schema_version', 'source_input', 'source_capture', 'functions', 'power5', 'cases'}
    if set(request) != required or request['schema_version'] != 1:
        raise ValueError('Unsupported arithmetic batch request')
    if type(workers) is not int or not 1 <= workers <= 16:
        raise ValueError('Expected one to sixteen arithmetic workers')
    if (not isinstance(request['cases'], list) or not 1 <= len(request['cases']) <= 4096
            or not isinstance(request['functions'], list) or not 1 <= len(request['functions']) <= 64):
        raise ValueError('Arithmetic batch must have bounded nonempty cases and functions')
    program = load_program(root, request['source_input'], request['source_capture'], request['functions'])
    spec = MontgomeryPower5Spec.parse(request['power5'])
    for row in request['cases']:
        parse_case(row)
    if workers == 1:
        results = []
        for row in request['cases']:
            try:
                results.append(verify_power5_case(program, spec, parse_case(row)))
            except Exception as error:
                results.append({'passed': False, 'input': row, 'error': type(error).__name__+': '+str(error),
                                'target_execution': False, 'official_dll_comparison': 'not_executed'})
    else:
        with ProcessPoolExecutor(max_workers=workers, mp_context=multiprocessing.get_context('spawn'),
                initializer=_initialize_batch, initargs=(str(root.resolve()), request)) as pool:
            results = list(pool.map(_verify_batch_case, request['cases']))
    groups = {}
    for result in results:
        if result['passed']:
            key = (result['limbs'], result['capability_flags'], result['alias_output'])
            groups.setdefault(key, set()).add((result['table_read_trace_sha256'], result['instruction_visit_counts_sha256']))
    same_trace = all(len(traces) == 1 for traces in groups.values())
    passed = all(row['passed'] for row in results) and same_trace
    source = Path(__file__).read_bytes()
    return {'schema_version': 1, 'mode': 'retained_x64_arithmetic_simulation', 'workers': workers,
        'source_input': request['source_input'], 'source_capture': request['source_capture'],
        'functions': request['functions'], 'power5': request['power5'], 'cases': results,
        'tool': {'path': 'tools/native_arithmetic.py', 'bytes': len(source), 'sha256': hashlib.sha256(source).hexdigest()},
        'summary': {'cases': len(results), 'passed': sum(row['passed'] for row in results),
                    'failed': sum(not row['passed'] for row in results),
                    'table_reads_and_instruction_visit_counts_invariant_within_layout': same_trace},
        'passed': passed, 'target_execution': False, 'rust_implementation_changed': False,
        'semantics_complete': False, 'official_dll_comparison': 'not_executed', 'real_device_acceptance': 'not_executed',
        'limitations': ['Pure interpretation trusts the identity-bound retained IDA mnemonic and operand records.',
            'This does not repair missing entry preludes, failed Hex-Rays capture or establish a callable ABI.',
            'Arithmetic cases do not execute the official DLL, operating-system exception unwinder or a real device.',
            'Only the reviewed multiple-of-eight Montgomery power5 path and supported instruction subset are interpreted.']}


def _verify_family_module(task):
    root, module, cases = task
    try:
        result = verify_power5_batch(Path(root), {'schema_version': 1, **module, 'cases': cases}, workers=1)
        result['summary']['unexecuted'] = 0
        return result
    except Exception as error:
        return {'schema_version': 1, 'mode': 'retained_x64_arithmetic_simulation', **module,
            'summary': {'cases': len(cases), 'passed': 0, 'failed': 0, 'unexecuted': len(cases)},
            'error': type(error).__name__ + ': ' + str(error), 'passed': False, 'target_execution': False,
            'semantics_complete': False, 'official_dll_comparison': 'not_executed',
            'real_device_acceptance': 'not_executed'}


def verify_power5_family(root: Path, request: dict, *, workers: int = 4) -> dict:
    """Interpret each exact input independently while sharing one frozen case set."""
    if set(request) != {'schema_version', 'cases_source', 'modules'} or request['schema_version'] != 2:
        raise ValueError('Unsupported arithmetic family request')
    if type(workers) is not int or not 1 <= workers <= 16:
        raise ValueError('Expected one to sixteen arithmetic workers')
    if not isinstance(request['modules'], list) or not 1 <= len(request['modules']) <= 128:
        raise ValueError('Arithmetic family must have bounded nonempty modules')
    _, cases_raw = identity(root, request['cases_source'])
    case_request = json.loads(cases_raw)
    if (set(case_request) != {'schema_version', 'source_input', 'source_capture', 'functions', 'power5', 'cases'}
            or case_request['schema_version'] != 1 or not isinstance(case_request['cases'], list)
            or not 1 <= len(case_request['cases']) <= 4096):
        raise ValueError('Family case source must be an exact version-one arithmetic request')
    cases = case_request['cases']
    for row in cases:
        parse_case(row)
    required = {'source_input', 'source_capture', 'functions', 'power5'}
    seen = set()
    for module in request['modules']:
        if not isinstance(module, dict) or set(module) != required:
            raise ValueError('Unexpected arithmetic family module fields')
        if not isinstance(module['functions'], list) or not 1 <= len(module['functions']) <= 64:
            raise ValueError('Arithmetic module must have bounded nonempty functions')
        MontgomeryPower5Spec.parse(module['power5'])
        digest = module['source_input'].get('sha256')
        if not isinstance(digest, str) or not re.fullmatch('[0-9a-f]{64}', digest) or digest in seen:
            raise ValueError('Arithmetic family requires unique exact input identities')
        seen.add(digest)
    tasks = [(str(root.resolve()), module, cases) for module in request['modules']]
    if workers == 1:
        results = [_verify_family_module(task) for task in tasks]
    else:
        with ProcessPoolExecutor(max_workers=workers, mp_context=multiprocessing.get_context('spawn')) as pool:
            results = list(pool.map(_verify_family_module, tasks))
    # The case set is shared by reference, but every module has its own original-byte
    # checks, machine state, integer comparisons and retained success/failure result.
    identity(root, request['cases_source'])
    source = Path(__file__).read_bytes()
    digest = hashlib.sha256(source).hexdigest()
    for result in results:
        if result.get('tool', {}).get('sha256', digest) != digest:
            raise ValueError('Arithmetic worker tool differs from family owner')
    summary = {'modules': len(results), 'passed_modules': sum(row['passed'] for row in results),
        'failed_modules': sum(not row['passed'] for row in results),
        **{key: sum(row['summary'][key] for row in results) for key in ('cases', 'passed', 'failed', 'unexecuted')}}
    return {'schema_version': 2, 'mode': 'retained_x64_arithmetic_family_simulation', 'workers': workers,
        'cases_source': request['cases_source'], 'modules': results,
        'tool': {'path': 'tools/native_arithmetic.py', 'bytes': len(source), 'sha256': digest},
        'summary': summary, 'passed': all(row['passed'] for row in results), 'target_execution': False,
        'rust_implementation_changed': False, 'semantics_complete': False,
        'official_dll_comparison': 'not_executed', 'real_device_acceptance': 'not_executed',
        'limitations': ['Each input is interpreted independently against the same frozen cases; family membership does not imply a pass.',
            'Pure interpretation trusts the identity-bound retained IDA mnemonic and operand records.',
            'This does not repair missing entry preludes, failed Hex-Rays capture or establish a callable ABI.',
            'Arithmetic cases do not execute the official DLL, operating-system exception unwinder or a real device.']}


def ieee_fraction(bits: int, width: int) -> Fraction:
    """Decode a finite IEEE binary32/64 value without host floating arithmetic."""
    fraction_bits, exponent_bits = {32: (23, 8), 64: (52, 11)}[width]
    sign = -1 if bits >> (width-1) else 1
    exponent = (bits >> fraction_bits) & ((1 << exponent_bits)-1)
    significand = bits & ((1 << fraction_bits)-1)
    if exponent == (1 << exponent_bits)-1:
        raise ValueError('Expected finite IEEE value')
    bias = (1 << (exponent_bits-1))-1
    if exponent:
        significand |= 1 << fraction_bits
    power = (exponent or 1)-bias-fraction_bits
    return sign*Fraction(significand)*(2**power if power >= 0 else Fraction(1, 1 << -power))


def round_ieee(value: Fraction, width: int, mode: str) -> int:
    """Round an exact rational to a finite binary result under the reviewed RC."""
    fraction_bits, exponent_bits = {32: (23, 8), 64: (52, 11)}[width]
    if mode not in ('nearest_even', 'down', 'up', 'zero'):
        raise ValueError('Unknown IEEE rounding mode')
    sign = int(value < 0)
    value = abs(Fraction(value))
    if not value:
        return sign << (width-1)
    bias = (1 << (exponent_bits-1))-1
    exponent = value.numerator.bit_length()-value.denominator.bit_length()
    if value < (1 << exponent if exponent >= 0 else Fraction(1, 1 << -exponent)):
        exponent -= 1
    unit_power = max(exponent, 1-bias)-fraction_bits
    scaled = value/(1 << unit_power if unit_power >= 0 else Fraction(1, 1 << -unit_power))
    integer, remainder = divmod(scaled.numerator, scaled.denominator)
    increment = (2*remainder > scaled.denominator or
                 (2*remainder == scaled.denominator and integer & 1)) if mode == 'nearest_even' else (
                    bool(remainder) and ((mode == 'down' and sign) or (mode == 'up' and not sign)))
    integer += increment
    if integer >= 1 << (fraction_bits+1):
        integer >>= 1
        exponent += 1
    encoded_exponent = max(exponent+bias, 1) if integer >= 1 << fraction_bits else 0
    if encoded_exponent >= (1 << exponent_bits)-1:
        raise ValueError('Rational rounding exceeded reviewed finite prelude domain')
    return (sign << (width-1)) | (encoded_exponent << fraction_bits) | (integer & ((1 << fraction_bits)-1))


def verify_rounding_interval(value: Fraction, bits: int, width: int, mode: str):
    """Certify the chosen bit pattern by exact adjacent-value inequalities."""
    actual = ieee_fraction(bits, width)
    magnitude = bits & ((1 << (width-1))-1)
    if not magnitude:
        if value:
            raise ValueError('Nonzero underflow is outside reviewed prelude interval certification')
        return
    negative = bool(bits >> (width-1))
    below = ieee_fraction(bits+1 if negative else bits-1, width)
    above = ieee_fraction(bits-1 if negative else bits+1, width)
    if actual == value:
        return
    if mode == 'nearest_even':
        lower, upper = (below+actual)/2, (actual+above)/2
        passed = lower <= value <= upper and (value not in (lower, upper) or not bits & 1)
    elif mode == 'down' or (mode == 'zero' and value >= 0):
        passed = actual < value < above
    else:
        passed = below < value < actual
    if not passed:
        raise ValueError('Rounded IEEE bit pattern violates exact adjacent-value interval')


def interpret_peak_counter(instructions: list[dict], parameter: int) -> int:
    """Interpret the retained x86 MUL/ADC/SHRD prelude; no target execution."""
    if type(parameter) is not int or not 0 <= parameter < 1 << 32:
        raise ValueError('Peak meter decay parameter requires uint32')
    registers = dict.fromkeys(('eax', 'ebx', 'ecx', 'edx', 'esi', 'edi'), 0)
    memory = {'InBuffer': parameter}
    mask = (1 << 32)-1
    carry = 0
    def read(operand):
        if operand in registers:
            return registers[operand]
        if operand.startswith('[ebp+') and operand.endswith(']'):
            return memory[operand[5:-1]]
        return int(operand[:-1], 16) if operand.endswith('h') else int(operand, 0)
    def write(operand, value):
        if operand in registers:
            registers[operand] = value & mask
        elif operand.startswith('[ebp+') and operand.endswith(']'):
            memory[operand[5:-1]] = value & mask
        else:
            raise ValueError('Unknown x86 counter destination')
    for instruction in instructions:
        parts = instruction['text'].split(';', 1)[0].strip().split(None, 1)
        mnemonic = parts[0]
        operands = [v.strip() for v in parts[1].split(',')]
        if mnemonic == 'mov':
            write(operands[0], read(operands[1]))
        elif mnemonic == 'mul':
            product = registers['eax']*read(operands[0])
            registers['eax'], registers['edx'] = product & mask, product >> 32
            carry = int(product > mask)
        elif mnemonic in ('add', 'adc', 'sub', 'sbb'):
            left, right = read(operands[0]), read(operands[1])
            extra = carry if mnemonic in ('adc', 'sbb') else 0
            subtract = mnemonic in ('sub', 'sbb')
            value = left-right-extra if subtract else left+right+extra
            carry = int(left < right+extra) if subtract else int(value > mask)
            write(operands[0], value)
        elif mnemonic == 'xor':
            write(operands[0], read(operands[0]) ^ read(operands[1]))
            carry = 0
        elif mnemonic == 'shrd':
            count = read(operands[2]) & 31
            left, right = read(operands[0]), read(operands[1])
            if count:
                write(operands[0], (left >> count) | (right << (32-count)))
                carry = (left >> (count-1)) & 1
        elif mnemonic == 'shr':
            count = read(operands[1]) & 31
            left = read(operands[0])
            if count:
                write(operands[0], left >> count)
                carry = (left >> (count-1)) & 1
        elif mnemonic == 'call' and operands == ['__ultod3']:
            return registers['ecx'] | (registers['edx'] << 32)
        else:
            raise ValueError('Unknown retained x86 counter instruction: ' + instruction['text'])
    raise ValueError('Retained counter prelude omitted its original conversion boundary')


def verify_peak_meter_prelude(root: Path, request: dict) -> dict:
    """Validate exact integer and IEEE input stages while retaining POW as unverified."""
    import native_adjudication
    import host
    from host import inspect_pe
    required = {'schema_version', 'review_kind', 'modules', 'parameters'}
    if (set(request) != required or request['schema_version'] != 3
            or request['review_kind'] != 'retained_x86_peak_meter_prelude'
            or not isinstance(request['modules'], list) or not 1 <= len(request['modules']) <= 16
            or not isinstance(request['parameters'], list) or not 1 <= len(request['parameters']) <= 4096
            or len(set(request['parameters'])) != len(request['parameters'])
            or any(type(n) is not int or not 0 <= n < 1 << 32 for n in request['parameters'])):
        raise ValueError('Unsupported exact peak meter prelude request')
    results = []
    seen_inputs = set()
    for module in request['modules']:
        if set(module) != {'source_input', 'source_capture', 'function'}:
            raise ValueError('Unexpected exact peak meter module fields')
        if module['source_input']['sha256'] in seen_inputs:
            raise ValueError('Peak meter modules require distinct original input identities')
        seen_inputs.add(module['source_input']['sha256'])
        _, raw = identity(root, module['source_input'])
        _, capture_raw = identity(root, module['source_capture'])
        _, function_raw = identity(root, module['function'])
        capture, function = json.loads(capture_raw), json.loads(function_raw)
        relative = Path(module['function']['path']).relative_to(Path(module['source_capture']['path']).parent).as_posix()
        if (capture.get('input') != module['source_input']['path']
                or capture.get('input_sha256') != module['source_input']['sha256']
                or capture.get('input_bytes') != module['source_input']['bytes']
                or not any(row['path'] == relative and row['sha256'] == module['function']['sha256'] for row in capture['functions'])):
            raise ValueError('Peak meter function does not bind exact current original capture')
        native_adjudication.original_machine(raw, function)
        pe = inspect_pe(raw)
        if pe['architecture'] != 'x86' or function['rva'] != '0xac80':
            raise ValueError('Unreviewed peak meter machine layout')
        def original(rva, size):
            rows = [s for s in pe['sections'] if s['rva'] <= rva and rva+size <= s['rva']+s['bytes']]
            if len(rows) != 1:
                raise ValueError('Peak meter constant is not uniquely file backed')
            start = rows[0]['file_offset']+rva-rows[0]['rva']
            return raw[start:start+size]
        for start, end, expected in ((0xad0b, 0xad8d, '5bb30d4911fc2e32b5529b76cf3f212760666101f8f15cc501c820d3d5920817'),
                (0xad8d, 0xadcb, '524c6b7bebbbfc903e2a5351d236de141a5afaaaf0dd2d283d510ddfa746a357')):
            if hashlib.sha256(original(start, end-start)).hexdigest() != expected:
                raise ValueError('Peak meter arithmetic differs from reviewed complete original instructions')
        for address, expected in ((0x30df8, '0000000000002440'), (0x30e00, '0000a041'),
                (0x30e04, '0000c0c0'), (0x30e08, '000080ff')):
            if original(address, len(expected)//2).hex() != expected:
                raise ValueError('Peak meter arithmetic constant differs from reviewed original bits')
        instructions = [v for v in function['assembly'] if 0xad0b <= int(v['rva'], 0) < 0xad8d]
        rows = []
        for parameter in request['parameters']:
            count = interpret_peak_counter(instructions, parameter)
            if count != 3072*parameter:
                raise ValueError('Original x86 integer prelude differs from exact count identity')
            for mode in ('nearest_even', 'down', 'up', 'zero'):
                stages = []
                for value, width in ((Fraction(count), 64), (Fraction(count), 32)):
                    bits = round_ieee(value, width, mode)
                    verify_rounding_interval(value, bits, width, mode)
                    stages.append({'width': width, 'bits': hex(bits), 'exact_value': str(value)})
                if count:
                    quotient = Fraction(-6)/ieee_fraction(int(stages[-1]['bits'], 0), 32)
                    bits = round_ieee(quotient, 32, mode)
                    verify_rounding_interval(quotient, bits, 32, mode)
                    stages.append({'width': 32, 'bits': hex(bits), 'exact_value': str(quotient)})
                    exponent = ieee_fraction(bits, 32)/20
                    bits = round_ieee(exponent, 32, mode)
                    verify_rounding_interval(exponent, bits, 32, mode)
                    stages.append({'width': 32, 'bits': hex(bits), 'exact_value': str(exponent)})
                    extended = ieee_fraction(bits, 32)
                    bits = round_ieee(extended, 64, mode)
                    verify_rounding_interval(extended, bits, 64, mode)
                    stages.append({'width': 64, 'bits': hex(bits), 'exact_value': str(extended)})
                else:
                    stages.append({'width': 32, 'bits': '0xff800000', 'operation': 'masked -6 / +0'})
                rows.append({'parameter': parameter, 'rounding': mode, 'original_count': count,
                    'expected_count': 3072*parameter, 'stages': stages, 'pow_called': bool(count),
                    'zero_branch_output_bits': None if count else '0x00000000',
                    'exact_adjacent_value_intervals_verified': True, 'passed': True,
                    'pow_result': 'not_verified', 'fp_status_and_unmasked_exceptions': 'not_executed'})
        results.append({**module, 'cases': rows, 'passed': True, 'full_original_function_bytes_verified': True})
    source = Path(__file__).read_bytes()
    dependencies = []
    for dependency in (native_adjudication, host):
        path = Path(dependency.__file__)
        body = path.read_bytes()
        dependencies.append({'path': 'tools/' + path.name, 'bytes': len(body),
                             'sha256': hashlib.sha256(body).hexdigest()})
    return {'schema_version': 3, 'mode': 'retained_x86_peak_meter_prelude', 'modules': results,
        'summary': {'modules': len(results), 'cases': sum(len(v['cases']) for v in results),
                    'passed': sum(len(v['cases']) for v in results), 'failed': 0}, 'passed': True,
        'tool': {'path': 'tools/native_arithmetic.py', 'bytes': len(source), 'sha256': hashlib.sha256(source).hexdigest()},
        'tool_dependencies': dependencies,
        'target_execution': False, 'semantics_complete': False, 'rust_implementation_changed': False,
        'official_dll_comparison': 'not_executed', 'real_device_acceptance': 'not_executed',
        'limitations': ['Only the original integer prelude is instruction-interpreted; IEEE stages use exact rational arithmetic with independent adjacent-value certification.',
            'The IEEE result model requires masked exceptions and does not measure CPU exception flags, traps, FTZ/DAZ or asynchronous device state.',
            'POW, FYL2X, the final binary32 POW result and device writes are not numerically verified here.']}


def run(a, args):
    """Execute an explicit, identity-bound pure request and freeze its full result."""
    path = a.resolve(args.request, '.analysis/results')
    raw = path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if not re.fullmatch('[0-9a-f]{64}', args.request_sha256) or digest != args.request_sha256:
        raise ValueError('Arithmetic request SHA-256 differs from the reviewed bytes')
    reference = {'path': a.rel(path), 'bytes': len(raw), 'sha256': digest}
    request = json.loads(raw)
    try:
        result = verify_power5_batch(a.ROOT, request, workers=args.workers)
    except Exception as error:
        source = Path(__file__).read_bytes()
        result = {'schema_version': request.get('schema_version'), 'passed': False,
            'summary': {'cases': 0, 'passed': 0, 'failed': 0, 'verification_aborted': True},
            'error': type(error).__name__ + ': ' + str(error),
            'tool': {'path': 'tools/native_arithmetic.py', 'bytes': len(source),
                     'sha256': hashlib.sha256(source).hexdigest()},
            'target_execution': False, 'semantics_complete': False,
            'official_dll_comparison': 'not_executed', 'real_device_acceptance': 'not_executed'}
    result.update(request=reference, recorded_at_utc=a.stamp())
    if path.read_bytes() != raw:
        raise ValueError('Arithmetic request changed during verification')
    source = Path(__file__).read_bytes()
    tool = result['tool']
    if hashlib.sha256(source).hexdigest() != tool['sha256'] or len(source) != tool['bytes']:
        raise ValueError('Arithmetic tool changed during verification')
    directory = a.ROOT / '.analysis/results/reverse-pending'

    def freeze(relative, body):
        destination = directory / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        try:
            with destination.open('xb') as stream:
                stream.write(body)
        except FileExistsError:
            if destination.read_bytes() != body:
                raise ValueError('Immutable arithmetic evidence already has different bytes')
        if destination.read_bytes() != body:
            raise ValueError('Arithmetic evidence publication differs on readback')
        return {'path': a.rel(destination), 'bytes': len(body), 'sha256': hashlib.sha256(body).hexdigest()}

    result['tool_source'] = freeze('tool-sources/' + tool['sha256'] + '.py', source)
    if result.get('tool_dependencies'):
        result['dependency_sources'] = [freeze('tool-sources/' + row['sha256'] + '.py', identity(a.ROOT, row)[1])
                                        for row in result['tool_dependencies']]
    result['request_source'] = freeze('review-sources/' + digest + '.json', raw)
    body = (json.dumps(result, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    receipt = freeze('review-sources/' + hashlib.sha256(body).hexdigest() + '.json', body)
    print(json.dumps({'receipt': receipt, 'summary': result['summary'], 'passed': result['passed'],
        'target_execution': False, 'official_dll_comparison': 'not_executed'}), flush=True)
    if not result['passed']:
        raise ValueError('Pure arithmetic verification contains retained differences; inspect its immutable receipt')
    return receipt

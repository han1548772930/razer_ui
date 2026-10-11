"""Run INSIDE IDAPython for static function evidence, never a debugger.

Usage: idat64 -A -S".../ida-static-functions.py CONFIG.json" INPUT_COPY
CONFIG supplies expected_sha256, output and function_rvas. Analyse a private
copy, not the maintained original. No target initialization/export is invoked.
"""
import hashlib
import json
from pathlib import Path
import traceback

import ida_auto
import ida_bytes
import ida_funcs
import ida_hexrays
import ida_nalt
import ida_pro
import idaapi
import idautils
import idc
import re


def main():
    config = json.loads(Path(idc.ARGV[1]).read_text(encoding="utf-8"))
    source = Path(ida_nalt.get_input_file_path())
    raw = source.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == config["expected_sha256"]
    ida_auto.auto_wait()
    base = ida_nalt.get_imagebase()
    has_decompiler = ida_hexrays.init_hexrays_plugin()
    requested = list(config.get("function_rvas", []))
    entries = []
    unresolved = []
    function_index = []
    if config.get("corpus_mode"):
        for _, ordinal, address, name in idautils.Entries():
            entries.append({"ordinal": ordinal, "rva": address-base, "name": name})
        roots = [base+rva for rva in requested] + [e["rva"]+base for e in entries]
        # Include native plugin entry/registration paths even without exports.
        roots.extend(base+rva for rva in config.get("entry_rvas", []))
        wanted = set()
        for address in roots:
            function = ida_funcs.get_func(address)
            if function:
                wanted.add(function.start_ea)
            else:
                unresolved.append({"rva": address-base, "reason": "IDA did not identify an entry function; forwarding/data/architecture must be reviewed"})
        # One direct-reference expansion is explicit; no claim of full closure.
        for address in list(wanted):
            for ea in idautils.FuncItems(address):
                for target in idautils.CodeRefsFrom(ea, False):
                    function = ida_funcs.get_func(target)
                    if function:
                        wanted.add(function.start_ea)
        patterns = re.compile(config.get("string_pattern", r"(?i)hid|pair|firmware|device|service|mapping|lighting|volume|mute|ioctl|upgrade"))
        string_anchors = []
        for item in idautils.Strings():
            value = str(item)
            if patterns.search(value):
                refs = []
                for reference in idautils.XrefsTo(item.ea):
                    function = ida_funcs.get_func(reference.frm)
                    if function:
                        wanted.add(function.start_ea)
                        refs.append({"from_rva": reference.frm-base, "function_rva": function.start_ea-base})
                string_anchors.append({"rva": item.ea-base, "text": value, "references": refs})
        requested = sorted(address-base for address in wanted)
        for address in idautils.Functions():
            function = ida_funcs.get_func(address)
            function_index.append({"rva": address-base, "end_rva": function.end_ea-base,
                                   "name": ida_funcs.get_func_name(address),
                                   "chunks": [[start-base, end-base] for start, end in idautils.Chunks(address)]})
    import_anchors = []
    if config.get("import_pattern"):
        pattern = re.compile(config["import_pattern"])
        def imported(address, name, ordinal):
            if name and pattern.search(name):
                references = []
                for reference in idautils.XrefsTo(address):
                    function = ida_funcs.get_func(reference.frm)
                    references.append({"from_rva": reference.frm-base,
                                       "function_rva": function.start_ea-base if function else None})
                    if function and function.start_ea-base not in requested:
                        requested.append(function.start_ea-base)
                import_anchors.append({"rva": address-base, "name": name,
                                       "ordinal": ordinal, "references": references})
            return True
        for index in range(ida_nalt.get_import_module_qty()):
            ida_nalt.enum_import_names(index, imported)
    functions = []
    for rva in requested:
        address = base + rva
        function = ida_funcs.get_func(address)
        assert function and function.start_ea == address, hex(rva)
        pseudocode, failure = None, None
        if has_decompiler:
            try:
                pseudocode = str(ida_hexrays.decompile(address))
            except Exception as error:
                failure = str(error)
        refs = [{"from_rva": x.frm - base, "type": x.type,
                 "is_code": bool(x.iscode)} for x in idautils.XrefsTo(address)]
        instructions = [{"rva": ea-base, "text": idc.generate_disasm_line(ea, 0)}
                        for ea in idautils.FuncItems(address)]
        functions.append({"rva": rva, "end_rva": function.end_ea-base,
                          "name": ida_funcs.get_func_name(address),
                          "code_sha256": hashlib.sha256(ida_bytes.get_bytes(
                              address, function.end_ea-address)).hexdigest(),
                          "pseudocode": pseudocode,
                          "decompiler_error": failure,
                          "incoming_references": refs,
                          "instructions": instructions})
    result = {"schema_version": 1, "method": "IDA autoanalysis and Hex-Rays static decompilation; no debugger or target execution",
              "input_sha256": config["expected_sha256"], "input_bytes": len(raw),
              "ida_version": idaapi.get_kernel_version(), "image_base": base,
              "hexrays_available": has_decompiler, "functions": functions,
              "limitations": ["Pseudocode is inferred from machine code; check types and indirect targets against original bytes.",
                               "Selected functions only, not full-program semantic completion."]}
    if config.get("data_ranges"):
        data_ranges = []
        for item in config["data_ranges"]:
            rva, size = item["rva"], item["size"]
            data = ida_bytes.get_bytes(base+rva, size)
            assert data is not None and len(data) == size
            data_ranges.append({"name": item["name"], "rva": rva, "size": size,
                                "hex": data.hex(), "sha256": hashlib.sha256(data).hexdigest(),
                                "incoming_references": [{"from_rva": ref.frm-base,
                                    "type": ref.type, "is_code": bool(ref.iscode)}
                                    for ref in idautils.XrefsTo(base+rva)]})
        result["data_ranges"] = data_ranges
    if import_anchors:
        result["import_anchors"] = import_anchors
    if config.get("corpus_mode"):
        result.update({"entries": entries, "unresolved_entries": unresolved,
                       "function_index": function_index, "string_anchors": string_anchors,
                       "selection": "exports/PE entry, one direct-reference expansion, and all recovered keyword string xref functions; indirect calls and other function bodies remain gaps"})
    Path(config["output"]).write_text(json.dumps(result, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")


try:
    main()
except Exception:
    traceback.print_exc()
    ida_pro.qexit(1)
else:
    ida_pro.qexit(0)

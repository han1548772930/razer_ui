"""Capture evidenced native entries from a private copy of a static IDB.

This supplements gaps in IDA's function enumeration without changing the
original full capture. Missing boundaries may be created only for byte-exact
x86/x64 direct-jump thunks or the exact x64 embedded-object destructor forwarding
thunk; other unknown code remains an explicit failure.
Retained functions can also be reanalyzed with explicit types derived from
their complete original-byte assembly. Type repairs never modify the base IDB.
"""
import hashlib
import json
import os
from pathlib import Path
import sys
import traceback
from datetime import datetime, timezone

import ida_auto
import ida_bytes
import ida_funcs
import ida_gdl
import ida_hexrays
import ida_ida
import ida_kernwin
import ida_lines
import ida_nalt
import ida_segment
import ida_typeinf
import ida_ua
import idaapi
import idautils
import idc


def capture():
    root = Path(__file__).resolve().parents[2]
    plan_path = Path(os.environ["RAZER_IDA_SUPPLEMENT_PLAN"]).resolve()
    plan = json.loads(plan_path.read_bytes())
    original = (root / plan["input"]).resolve()
    output = (root / plan["output"]).resolve()
    private = Path(idaapi.get_input_file_path()).resolve()
    if not output.is_relative_to(root / ".analysis/results/native"):
        raise ValueError("Supplement output escaped native evidence")
    if not private.is_relative_to(root / ".work/native"):
        raise ValueError("Database input is not a private analysis copy")
    for path in (original, private):
        if hashlib.sha256(path.read_bytes()).hexdigest() != plan["sha256"]:
            raise ValueError("Supplement input identity changed")
    stored = ida_nalt.retrieve_input_file_sha256()
    if stored is None or bytes(stored).hex() != plan["sha256"]:
        raise ValueError("Database's original input SHA differs")
    base_capture = root / plan["base_capture"]["path"]
    if hashlib.sha256(base_capture.read_bytes()).hexdigest() != plan["base_capture"]["sha256"]:
        raise ValueError("Original capture changed during supplemental analysis")
    ida_auto.auto_wait()
    base = idaapi.get_imagebase()
    table = plan.get("pointer_table")
    selection = table or plan["function_selection"]
    if hex(base) != selection["image_base"]:
        raise ValueError("Database image base differs from actual input")
    raw = original.read_bytes()
    sys.path.insert(0, str(root / "tools"))
    import analysis
    from host import inspect_pe
    if table:
        table_ea = base + int(table["rva"], 16)
        if (ida_bytes.get_bytes(table_ea, table["bytes"]) or b"").hex() != table["raw_hex"]:
            raise ValueError("Database pointer-table bytes differ from original file")
        actual = analysis.native_pointer_targets(raw, int(table["rva"], 16), len(table["targets"]))
    else:
        recoveries = selection["type_recoveries"]
        actual = analysis.native_function_selection(raw, json.loads(base_capture.read_bytes()),
            [int(row["rva"], 16) for row in selection["targets"]],
            [row["rva"] + "=" + row["declaration"] for row in recoveries],
            recoveries[0]["assembly_evidence"] if recoveries else None)
    if actual != selection:
        raise ValueError("Native selection no longer matches its bounded original-byte plan")
    pe = inspect_pe(raw)
    output.mkdir(parents=True, exist_ok=True)
    functions = output / "functions"
    functions.mkdir(exist_ok=True)
    manifest = {"schema_version": 1, "input": plan["input"], "input_sha256": plan["sha256"],
                "image_base": hex(base),
                "pointer_table" if table else "function_selection": selection,
                "base_capture": plan["base_capture"], "database": plan["database"],
                "source_database": plan.get("source_database"),
                "ida_version": ida_kernwin.get_kernel_version(),
                "hexrays_available": bool(ida_hexrays.init_hexrays_plugin()),
                "hexrays_version": str(ida_hexrays.get_hexrays_version()), "python_version": sys.version,
                "capture_tool": {"path": "tools/ida/supplement.py",
                                 "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
                "started_at_utc": datetime.now(timezone.utc).isoformat(),
                "functions": [], "references": [], "chains": [], "failures": [], "type_recoveries": [],
                "capture_complete": False, "all_pseudocode_captured": False,
                "boundary_policy": "Only exact direct-jump or x64 embedded-object destructor forwarding thunks receive new function boundaries; existing functions keep IDA boundaries",
                "target_execution": False, "real_device_operations": False,
                "semantic_audit": "pending", "implementation_acceptance": "pending"}

    def save(path, value):
        def portable(item):
            if isinstance(item, str):
                return item.replace(str(root), "<project>").replace(root.as_posix(), "<project>")
            if isinstance(item, list):
                return [portable(child) for child in item]
            if isinstance(item, dict):
                return {key: portable(child) for key, child in item.items()}
            return item
        analysis.save(path, portable(value))

    save(output / "capture.json", manifest)

    def ref(x):
        return {"from": hex(x.frm), "to": hex(x.to), "type": int(x.type),
                "code": bool(x.iscode), "user": bool(x.user)}

    issues = int(ida_ida.validate_idb(ida_ida.VLD_SILENT))
    manifest["database_integrity"] = {"operation": "read-only validation; no automatic repairs",
                                      "records_with_issues": issues, "verified": issues == 0}
    if issues:
        manifest.update(capture_complete=False, all_pseudocode_captured=False)
        manifest["failures"].append({"stage": "database_integrity", "error": "{} inconsistent database records".format(issues)})
        save(output / "capture.json", manifest)
        raise ValueError("Private supplemental database failed read-only integrity validation")

    def file_bytes(rva, length):
        matches = [section for section in pe["sections"]
                   if 0 <= rva - section["rva"] and rva - section["rva"] + length <= section["bytes"]]
        if len(matches) != 1:
            raise ValueError("Target byte range is not uniquely file-backed: " + hex(rva))
        offset = matches[0]["file_offset"] + rva - matches[0]["rva"]
        return raw[offset:offset + length]

    def jump_target(ea):
        if selection["architecture"] not in {"x86", "x64"}:
            return None
        first = file_bytes(ea - base, 1)
        length = 5 if first == b"\xe9" else 2 if first == b"\xeb" else 0
        if not length:
            return None
        original_bytes = file_bytes(ea - base, length)
        if ida_bytes.get_bytes(ea, length) != original_bytes:
            raise ValueError("Database jump bytes differ from original input")
        target = ea + length + int.from_bytes(original_bytes[1:], "little", signed=True)
        segment = ida_segment.getseg(target)
        if segment is None or not segment.perm & ida_segment.SEGPERM_EXEC:
            raise ValueError("Direct-jump target is not executable database code")
        if ida_bytes.get_bytes(target, 1) != file_bytes(target - base, 1):
            raise ValueError("Direct-jump target differs from original file bytes")
        return target, length

    captured = set()

    def forwarding_thunk(ea):
        # This compiler thunk appears as an actual vtable slot. It forwards the
        # embedded object's virtual destructor with delete=0. Recognize the
        # whole byte sequence and keep its indirect target unresolved here;
        # callers must prove the embedded object's table from its constructor.
        if selection["architecture"] != "x64":
            return None
        chunk = file_bytes(ea - base, 13)
        if (chunk[:3] != b"\x48\x8b\x41" or chunk[4:7] != b"\x48\x83\xc1"
                or chunk[3] != chunk[7] or not 0 < chunk[3] < 0x80
                or chunk[8:] != b"\x33\xd2\x48\xff\x20"):
            return None
        if ida_bytes.get_bytes(ea, len(chunk)) != chunk:
            raise ValueError("Database forwarding-thunk bytes differ from original input")
        return len(chunk)

    def export(ea):
        rva = hex(ea - base)
        existing = plan["existing_functions"].get(rva) if table else None
        if existing:
            path = (root / existing["path"]).resolve()
            if not path.is_relative_to(root / ".analysis/results/native"):
                raise ValueError("Original captured function path escaped evidence")
            if hashlib.sha256(path.read_bytes()).hexdigest() != existing["sha256"]:
                raise ValueError("Original captured function changed")
            body = json.loads(path.read_bytes())
            for chunk in body["chunks"]:
                first, end = int(chunk["start"], 16), int(chunk["end"], 16)
                if file_bytes(first - base, end - first).hex() != chunk["raw_hex"]:
                    raise ValueError("Original captured function bytes differ")
            if rva not in captured:
                manifest["references"].append(existing)
                captured.add(rva)
            return body
        if rva in captured:
            return json.loads((functions / (rva[2:] + ".json")).read_bytes())
        func = ida_funcs.get_func(ea)
        created = False
        boundary_source = "retained IDA function boundary"
        if func is None:
            branch = jump_target(ea)
            length = branch[1] if branch else forwarding_thunk(ea)
            if length is None:
                raise ValueError("No existing function boundary and target is not an evidenced supported thunk: " + rva)
            boundary_source = ("byte-exact direct-jump thunk" if branch
                               else "byte-exact x64 embedded-object destructor forwarding thunk")
            cursor = ea
            while cursor < ea + length:
                decoded = ida_ua.create_insn(cursor)
                if decoded <= 0 or cursor + decoded > ea + length:
                    raise ValueError("IDA could not decode the exact thunk boundary: " + rva)
                cursor += decoded
            if not ida_funcs.add_func(ea, ea + length):
                raise ValueError("IDA could not define the exact supported thunk: " + rva)
            ida_auto.auto_wait()
            func = ida_funcs.get_func(ea)
            created = True
        if func is None or func.start_ea != ea:
            raise ValueError("Requested entry lies inside a different function: " + rva)
        row = {"address": hex(ea), "rva": rva, "end": hex(func.end_ea),
               "name": ida_funcs.get_func_name(ea), "type": idc.get_type(ea), "flags": int(func.flags),
               "boundary_source": boundary_source,
               "chunks": [], "xrefs_to_entry": [ref(x) for x in idautils.XrefsTo(ea, 0)],
               "comments": {"regular": ida_funcs.get_func_cmt(func, False),
                            "repeatable": ida_funcs.get_func_cmt(func, True)},
               "assembly": [], "basic_blocks": [], "pseudocode": None}
        for first, end in idautils.Chunks(ea):
            chunk = ida_bytes.get_bytes(first, end - first)
            if chunk != file_bytes(first - base, end - first):
                raise ValueError("Supplement function bytes differ from the original input")
            row["chunks"].append({"start": hex(first), "end": hex(end), "raw_hex": chunk.hex(),
                                  "sha256": hashlib.sha256(chunk).hexdigest()})
        for item in idautils.FuncItems(ea):
            length = ida_bytes.get_item_size(item)
            row["assembly"].append({"address": hex(item), "rva": hex(item - base),
                "bytes": (ida_bytes.get_bytes(item, length) or b"").hex(),
                "text": ida_lines.tag_remove(idc.generate_disasm_line(item, 0) or ""),
                "xrefs_from": [ref(x) for x in idautils.XrefsFrom(item, 0)],
                "xrefs_to": [ref(x) for x in idautils.XrefsTo(item, 0)],
                "comment": idc.get_cmt(item, False), "repeatable_comment": idc.get_cmt(item, True)})
        row["basic_blocks"] = [{"id": block.id, "start": hex(block.start_ea), "end": hex(block.end_ea),
                                "predecessors": [x.id for x in block.preds()],
                                "successors": [x.id for x in block.succs()]} for block in ida_gdl.FlowChart(func)]
        try:
            failure = ida_hexrays.hexrays_failure_t()
            cfunc = ida_hexrays.decompile(ea, failure)
            if cfunc is None:
                raise ValueError("Hex-Rays: " + failure.desc())
            row["pseudocode"] = {"entry": hex(cfunc.entry_ea), "lines": [
                {"line": index, "text": ida_lines.tag_remove(line.line)}
                for index, line in enumerate(cfunc.get_pseudocode())], "locals": [
                {"name": value.name, "type": str(value.type()), "location": str(value.location),
                 "argument": bool(value.is_arg_var() if callable(value.is_arg_var) else value.is_arg_var)}
                for value in cfunc.get_lvars()]}
        except Exception:
            manifest["failures"].append({"rva": rva, "stage": "hexrays", "error": traceback.format_exc()})
        path = functions / (rva[2:] + ".json")
        save(path, row)
        manifest["functions"].append({"address": hex(ea), "rva": rva, "name": row["name"],
            "path": path.relative_to(output).as_posix(), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "assembly_captured": True, "pseudocode_captured": row["pseudocode"] is not None})
        captured.add(rva)
        ida_hexrays.clear_cached_cfuncs()
        return row

    for recovery in selection.get("type_recoveries", []):
        ea = base + int(recovery["rva"], 16)
        retained = plan["existing_functions"][recovery["rva"]]
        evidence = root / retained["path"]
        if hashlib.sha256(evidence.read_bytes()).hexdigest() != retained["sha256"]:
            raise ValueError("Assembly evidence changed before type recovery")
        body = json.loads(evidence.read_bytes())
        for chunk in body["chunks"]:
            first, end = int(chunk["start"], 16), int(chunk["end"], 16)
            if (file_bytes(first - base, end - first).hex() != chunk["raw_hex"]
                    or ida_bytes.get_bytes(first, end - first).hex() != chunk["raw_hex"]):
                raise ValueError("Type recovery assembly differs from original image or database")
        previous = idc.get_type(ea)
        typ = ida_typeinf.tinfo_t()
        if not ida_typeinf.parse_decl(typ, None, recovery["declaration"], ida_typeinf.PT_SIL) or not typ.is_func():
            raise ValueError("Explicit recovery declaration is not a valid function type")
        if not ida_typeinf.apply_tinfo(ea, typ, ida_typeinf.TINFO_DEFINITE):
            raise ValueError("IDA rejected the explicit recovered function type")
        ida_hexrays.clear_cached_cfuncs()
        applied = ida_typeinf.tinfo_t()
        if not ida_nalt.get_tinfo(applied, ea) or applied != typ:
            raise ValueError("Applied recovered type differs from the reviewed declaration")
        manifest["type_recoveries"].append({**recovery, "address": hex(ea), "previous_type": previous,
            "applied_type": str(applied), "assembly_evidence_reference": retained,
            "base_database_modified": False})

    for target in selection["targets"]:
        identity = {"slot_rva": target["slot_rva"]} if "slot_rva" in target else {"requested_rva": target["rva"]}
        chain = {**identity, "index": target["index"], "entries": [], "complete": False}
        visited = set()
        ea = int(target["address"], 16)
        try:
            for _ in range(64):
                if ea in visited:
                    raise ValueError("Direct-jump chain contains a cycle")
                visited.add(ea)
                export(ea)
                branch = jump_target(ea)
                chain["entries"].append({"address": hex(ea), "rva": hex(ea - base),
                    "direct_jump_target_rva": hex(branch[0] - base) if branch else None})
                if branch is None:
                    chain["complete"] = True
                    break
                ea = branch[0]
            else:
                raise ValueError("Direct-jump chain exceeds explicit static-analysis limit")
        except Exception:
            manifest["failures"].append({**identity, "stage": "target", "error": traceback.format_exc()})
        manifest["chains"].append(chain)
        save(output / "capture.json", manifest)
    manifest["capture_complete"] = all(chain["complete"] for chain in manifest["chains"])
    manifest["all_pseudocode_captured"] = not manifest["failures"]
    manifest["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    for path in (original, private):
        if hashlib.sha256(path.read_bytes()).hexdigest() != plan["sha256"]:
            raise ValueError("Original input changed during supplemental analysis")
    if hashlib.sha256(base_capture.read_bytes()).hexdigest() != plan["base_capture"]["sha256"]:
        raise ValueError("Base capture changed during supplemental analysis")
    if plan.get("source_database"):
        retained = plan["source_database"]
        if hashlib.sha256((root / retained["path"]).read_bytes()).hexdigest() != retained["sha256"]:
            raise ValueError("Base database changed during supplemental analysis")
    manifest["base_capture_unchanged"] = True
    manifest["base_database_unchanged"] = bool(plan.get("source_database"))
    save(output / "capture.json", manifest)
    return manifest


if __name__ == "__main__":
    exit_code = 0
    try:
        capture()
    except Exception:
        exit_code = 1
        print("[STATIC-SUPPLEMENT] fatal failure:\n" + traceback.format_exc(), flush=True)
    idc.qexit(exit_code)

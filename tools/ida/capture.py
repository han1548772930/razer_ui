"""Export one complete IDA database through static IDAPython/Hex-Rays APIs."""
import hashlib
import json
import os
import sys
import traceback
from datetime import datetime, timezone
from pathlib import Path

import ida_auto
import ida_bytes
import ida_diskio
import ida_funcs
import ida_gdl
import ida_hexrays
import ida_ida
import ida_idp
import ida_kernwin
import ida_lines
import ida_loader
import ida_nalt
import ida_segment
import ida_typeinf
import ida_ua
import ida_xref
import idaapi
import idautils
import idc

sys.path.insert(0, str(Path(__file__).resolve().parent))
import capture_io
from capture_io import file_sha256, write_function_aggregates, write_json_array


def write_json(path, value):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)


def capture_exception_regions(plan):
    """Decode exact original IA64 exception regions without creating functions."""
    root = Path(__file__).resolve().parents[2]
    source = (root / plan["input"]).resolve()
    output = (root / plan["output"]).resolve()
    scratch = (root / ".work/native").resolve()
    database = (root / plan["database"]["path"]).resolve()
    if (not source.is_relative_to(root / ".analysis/inputs")
            or not output.is_relative_to(root / ".analysis/results/native")
            or not database.is_relative_to(scratch)):
        raise ValueError("Exception-region capture escaped its evidence/private database owners")
    if Path(ida_loader.get_path(ida_loader.PATH_TYPE_IDB)).resolve() != database:
        raise ValueError("Exception-region analyzer opened a different private database")

    def verify_identity(identity):
        path = (root / identity["path"]).resolve()
        if (not path.is_relative_to(root) or path.stat().st_size != identity["bytes"]
                or file_sha256(path) != identity["sha256"]):
            raise ValueError("Exception-region capture evidence or tool changed")

    identities = [plan["source_database"], plan["base_capture"], plan["capture_tool"], plan["parser_tool"]]
    for identity in identities:
        verify_identity(identity)
    if (Path(__file__).resolve() != (root / plan["capture_tool"]["path"]).resolve()
            or (root / plan["parser_tool"]["path"]).resolve() != root / "tools/host.py"):
        raise ValueError("Exception-region running capture or parser path differs")
    raw = source.read_bytes()
    if hashlib.sha256(raw).hexdigest() != plan["sha256"]:
        raise ValueError("Exception-region original input changed")
    stored = ida_nalt.retrieve_input_file_sha256()
    if stored is None or bytes(stored).hex() != plan["sha256"]:
        raise ValueError("Exception-region database input SHA differs")
    sys.path.insert(0, str(root / "tools"))
    import host
    if Path(host.__file__).resolve() != (root / plan["parser_tool"]["path"]).resolve():
        raise ValueError("Exception-region loaded parser module differs")
    tool_sources = plan["tool_sources"]
    required = {"capture", "parser", "entry", "support", "analyzer", "sdk", *host.IA64_EXCEPTION_ANALYZER_FILES}
    if (set(tool_sources) != required or set(tool_sources["sdk"]) != set(host.IA64_EXCEPTION_SDK_MODULES)
            or tool_sources["capture"] != plan["capture_tool"] or tool_sources["parser"] != plan["parser_tool"]):
        raise ValueError("Exception-region tool closure is incomplete or inconsistent")
    for name, expected in {"entry": root / "tools/ida/ida-start-mcp.py",
                           "support": Path(capture_io.__file__).resolve()}.items():
        if (root / tool_sources[name]["path"]).resolve() != expected:
            raise ValueError("Exception-region running entry or support module differs")
    installation = Path(ida_diskio.idadir("")).resolve()
    for name, relative in host.IA64_EXCEPTION_ANALYZER_FILES.items():
        if tool_sources[name]["installation_path"] != relative:
            raise ValueError("Exception-region analyzer module path differs")
    processor_file = Path(ida_diskio.getsysfile("ia.dll", "procs")).resolve()
    if processor_file != (installation / tool_sources["processor"]["installation_path"]).resolve():
        raise ValueError("Exception-region processor resolves outside the bound installation")
    for name, identity in tool_sources["sdk"].items():
        if (identity["installation_path"] != "python/" + name + ".py"
                or Path(globals()[name].__file__).resolve() != installation / identity["installation_path"]):
            raise ValueError("Exception-region running SDK module differs: " + name)
    tool_rows = [row for name, row in tool_sources.items() if name != "sdk"] + list(tool_sources["sdk"].values())

    def verify_tools():
        for row in tool_rows:
            if "installation_path" in row:
                path = (installation / row["installation_path"]).resolve()
                if not path.is_relative_to(installation):
                    raise ValueError("Exception-region analyzer tool escaped its installation")
                if path.stat().st_size != row["bytes"] or file_sha256(path) != row["sha256"]:
                    raise ValueError("Exception-region analyzer module changed")
            else:
                verify_identity(row)
            snapshot = row["snapshot"]
            if (not (root / snapshot["path"]).resolve().is_relative_to(root / ".analysis/results/native")
                    or snapshot["bytes"] != row["bytes"] or snapshot["sha256"] != row["sha256"]):
                raise ValueError("Exception-region tool snapshot binding differs")
            verify_identity(snapshot)

    verify_tools()
    pe = host.inspect_pe(raw)
    selection = plan["exception_regions"]
    if (pe.get("architecture") != "ia64" or pe.get("exception_directory") != selection["directory"]
            or selection["directory"].get("validated") is not True or not selection["directory"].get("regions")
            or selection.get("tool_sources") != tool_sources
            or selection.get("source_database") != plan["source_database"]):
        raise ValueError("Exception-region selection differs from the original PE directory")
    processor = {"name": ida_ida.inf_get_procname(), "module": (ida_idp.get_idp_name() or "").lower(),
                 "bits": 64 if ida_ida.inf_is_64bit() else 32, "big_endian": bool(ida_ida.inf_is_be())}
    if processor != {"name": "ia64l", "module": "ia", "bits": 64, "big_endian": False} or selection.get("processor") != processor:
        raise ValueError("Exception-region database has a different processor, bitness or byte order")
    ida_auto.enable_auto(False)
    if ida_auto.is_auto_enabled():
        raise ValueError("Exception-region automatic analysis could not be disabled")
    base = idaapi.get_imagebase()
    if base != int(pe["image_base"], 0):
        raise ValueError("Exception-region database image base differs")
    integrity = int(ida_ida.validate_idb(ida_ida.VLD_SILENT))
    if integrity:
        raise ValueError("Exception-region private database failed read-only integrity validation")

    def function_chunks():
        return [{"address": hex(address), "chunks": [[hex(a), hex(b)] for a, b in sorted(idautils.Chunks(address))]}
                for address in sorted(idautils.Functions())]

    before = function_chunks()
    output.mkdir(parents=True, exist_ok=True)
    bodies = output / "regions"
    bodies.mkdir(exist_ok=True)
    manifest = {"schema_version": 1, "input": plan["input"], "input_sha256": plan["sha256"],
        "architecture": "ia64", "processor": processor["name"], "processor_identity": processor, "image_base": hex(base),
        "ida_version": ida_kernwin.get_kernel_version(), "capture_tool": plan["capture_tool"],
        "parser_tool": plan["parser_tool"], "tool_sources": tool_sources, "automatic_analysis_enabled": False,
        "base_capture": plan["base_capture"], "source_database": plan["source_database"],
        "exception_regions": selection, "regions": [], "failures": [], "capture_complete": False,
        "function_discovery_complete": False, "pseudocode_captured": False,
        "decompilation_attempted": False, "functions_created": False,
        "target_execution": False, "real_device_operations": False,
        "boundary": "Read-only instruction decoding of original exception regions, including fragments; no function ownership, unwind semantics, ABI or product behavior is inferred"}
    for region in selection["directory"]["regions"]:
        begin, end = base + int(region["begin_rva"], 0), base + int(region["end_rva"], 0)
        original = raw[region["file_offset"]:region["file_offset"] + end - begin]
        loaded = ida_bytes.get_bytes(begin, end - begin)
        if loaded is None or bytes(loaded) != original or hashlib.sha256(original).hexdigest() != region["sha256"]:
            raise ValueError("Exception-region database bytes differ from original PE bytes")
        row = {**region, "address": hex(begin), "end": hex(end), "raw_hex": original.hex(),
            "assembly": [], "undecoded_ranges": [], "decoded_bytes": 0,
            "undecoded_bytes": 0, "decode_complete": False}
        cursor = begin
        while cursor < end:
            insn = ida_ua.insn_t()
            size = int(ida_ua.decode_insn(insn, cursor))
            if size <= 0 or cursor + size > end:
                # A failed slot cannot establish an instruction length. Resume
                # only at the next original bundle, retaining the entire gap.
                resume = min(end, ((cursor // 16) + 1) * 16)
                missing = {"start_rva": hex(cursor - base), "end_rva": hex(resume - base),
                    "bytes": resume - cursor, "raw_hex": original[cursor - begin:resume - begin].hex()}
                row["undecoded_ranges"].append(missing)
                row["undecoded_bytes"] += resume - cursor
                manifest["failures"].append({"region": region["begin_rva"], "rva": hex(cursor - base),
                    "stage": "instruction_decode", "decoded_size": size,
                    "reason": "instruction_missing_or_exceeds_original_region",
                    "undecoded_range": missing, "resumed_at_rva": hex(resume - base)})
                cursor = resume
                continue
            operands = []
            for index, operand in enumerate(insn.ops):
                if operand.type == ida_ua.o_void:
                    break
                operands.append({"index": index, "type": int(operand.type), "dtype": int(operand.dtype),
                    "register": int(operand.reg), "value": int(operand.value), "address": int(operand.addr),
                    "specval": int(operand.specval),
                    "specflags": [int(getattr(operand, "specflag" + str(i))) for i in range(1, 5)],
                    "text": ida_lines.tag_remove(ida_ua.print_operand(cursor, index) or "")})
            row["assembly"].append({"address": hex(cursor), "rva": hex(cursor - base), "size": size,
                "bytes": original[cursor - begin:cursor - begin + size].hex(),
                "itype": int(insn.itype), "auxpref": int(insn.auxpref),
                "mnemonic": insn.get_canon_mnem(), "operands": operands,
                "idb_is_code": bool(ida_bytes.is_code(ida_bytes.get_full_flags(cursor))),
                "idb_disassembly": ida_lines.tag_remove(idc.generate_disasm_line(cursor, 0) or ""),
                "xrefs_from": [{"from": hex(x.frm), "to": hex(x.to), "type": int(x.type),
                    "code": bool(x.iscode), "user": bool(x.user)} for x in idautils.XrefsFrom(cursor, 0)],
                "xrefs_to": [{"from": hex(x.frm), "to": hex(x.to), "type": int(x.type),
                    "code": bool(x.iscode), "user": bool(x.user)} for x in idautils.XrefsTo(cursor, 0)]})
            row["decoded_bytes"] += size
            cursor += size
        row["decode_complete"] = row["decoded_bytes"] == end - begin and not row["undecoded_ranges"]
        path = bodies / (region["begin_rva"][2:] + ".json")
        write_json(path, row)
        manifest["regions"].append({"begin_rva": region["begin_rva"], "end_rva": region["end_rva"],
            "path": path.relative_to(output).as_posix(), "sha256": file_sha256(path),
            "decoded_bytes": row["decoded_bytes"], "undecoded_bytes": row["undecoded_bytes"],
            "decode_complete": row["decode_complete"]})
    for identity in identities:
        try:
            verify_identity(identity)
        except ValueError as exc:
            raise ValueError("Exception-region evidence or tool changed during capture") from exc
    verify_tools()
    if file_sha256(source) != plan["sha256"]:
        raise ValueError("Exception-region original changed during capture")
    after = function_chunks()
    if before != after or ida_auto.is_auto_enabled():
        raise ValueError("Exception-region capture changed function/chunk ownership or enabled automatic analysis")
    manifest["function_chunks_unchanged"] = True
    manifest["function_chunks_snapshot"] = {"functions": len(before),
        "chunks": sum(len(row["chunks"]) for row in before),
        "sha256_before": hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest(),
        "sha256_after": hashlib.sha256(json.dumps(after, sort_keys=True).encode()).hexdigest(),
        "rows": before}
    manifest["source_database_unchanged"] = True
    manifest["base_capture_unchanged"] = True
    manifest["capture_complete"] = not manifest["failures"] and all(row["decode_complete"] for row in manifest["regions"])
    manifest["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    write_json(output / "capture.json", manifest)
    return manifest


def capture(round_number=0):
    plan = json.loads(Path(os.environ["RAZER_IDA_CAPTURE_PLAN"]).read_bytes())
    if "exception_regions" in plan:
        return capture_exception_regions(plan)
    # The maintained script owns the workspace root. Evidence plans remain
    # portable and never retain a workstation-specific absolute root.
    root = Path(__file__).resolve().parents[2]
    original = (root / plan["input"]).resolve()
    private = Path(idaapi.get_input_file_path()).resolve()
    output = (root / plan["output"]).resolve()
    scratch = (root / ".work/native").resolve()
    if not output.is_relative_to(root / ".analysis/results/native") or not private.is_relative_to(scratch):
        raise ValueError("Capture paths escaped the analysis directories")
    for path in (private, original):
        if file_sha256(path) != plan["sha256"]:
            raise ValueError("Native input byte identity changed")
    stored = ida_nalt.retrieve_input_file_sha256()
    if stored is None or bytes(stored).hex() != plan["sha256"]:
        raise ValueError("Database original input SHA differs")
    ida_auto.auto_wait()
    base = idaapi.get_imagebase()
    addresses = list(idautils.Functions())
    output.mkdir(parents=True, exist_ok=True)
    bodies = output / "functions"
    bodies.mkdir(exist_ok=True)
    hexrays_available = bool(ida_hexrays.init_hexrays_plugin())
    manifest = {
        "schema_version": 1, "input": plan["input"], "input_sha256": plan["sha256"],
        "input_bytes": private.stat().st_size, "image_base": hex(base),
        "architecture": plan["loader"]["architecture"], "processor": ida_ida.inf_get_procname(),
        "ida_version": ida_kernwin.get_kernel_version(),
        "hexrays_available": hexrays_available,
        "hexrays_version": str(ida_hexrays.get_hexrays_version()) if hexrays_available else None,
        "python_version": sys.version,
        "capture_tool": {"path": "tools/ida/capture.py",
                         "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
        "capture_support": {"path": "tools/ida/capture_io.py",
                            "sha256": file_sha256(Path(__file__).with_name("capture_io.py"))},
        "dataset_serialization": "streamed JSON array; one record retained at a time",
        "started_at_utc": datetime.now(timezone.utc).isoformat(),
        "functions_total": len(addresses), "functions": [], "failures": [],
        "target_execution": False, "real_device_operations": False,
        "semantic_audit": "pending", "implementation_acceptance": "pending",
    }
    previous_path = output / "capture.json"
    previous = json.loads(previous_path.read_bytes()) if previous_path.is_file() else {}
    if previous and previous.get("input_sha256") != plan["sha256"]:
        raise ValueError("Existing native capture has different input bytes")
    reuse = bool(previous) and type(previous.get("hexrays_available")) is bool and all(
        previous.get(key) == manifest[key] for key in (
        "architecture", "processor", "ida_version", "hexrays_available", "hexrays_version",
        "capture_tool", "capture_support"))
    retained = {item["path"]: item for item in previous.get("functions", [])}
    manifest["retained_functions_reused"] = 0
    if previous:
        manifest["previous_capture"] = {
            "sha256": hashlib.sha256(previous_path.read_bytes()).hexdigest(),
            "architecture": previous.get("architecture"),
            "processor": previous.get("processor"),
            "ida_version": previous.get("ida_version"),
            "hexrays_available": previous.get("hexrays_available"),
            "hexrays_version": previous.get("hexrays_version"),
            "capture_tool": previous.get("capture_tool"),
            "reuse_allowed": reuse,
            "reason": "identical input and analyzer/tool capability identity" if reuse else "analyzer capability or capture script changed; regenerate function bodies",
        }

    def guarded(label, callback):
        try:
            return callback()
        except Exception as exc:
            detail = traceback.format_exc().replace(str(root) + os.sep, "")
            manifest["failures"].append({"stage": label, "error": str(exc), "traceback": detail})
            return None

    if plan.get("database_resume"):
        issues = guarded("database_integrity", lambda: int(ida_ida.validate_idb(ida_ida.VLD_SILENT)))
        manifest["database_integrity"] = {"operation": "read-only validation; no automatic repairs",
                                          "records_with_issues": issues, "verified": issues == 0}
        if issues:
            manifest["failures"].append({"stage": "database_integrity",
                "error": "Recovered database still contains {} inconsistent records".format(issues)})
    else:
        manifest["database_integrity"] = {"operation": "new database; recovery validation not required", "verified": None}

    if manifest["database_integrity"]["verified"] is False:
        manifest.update(capture_complete=False, functions_complete=False,
                        capture_phase="database_integrity_failed",
                        finished_at_utc=datetime.now(timezone.utc).isoformat())
        write_json(output / "capture.json", manifest)
        raise RuntimeError("Recovered IDA database failed read-only integrity validation; no datasets or function bodies were exported")

    def ref(x):
        return {"from": hex(x.frm), "to": hex(x.to), "type": int(x.type),
                "code": bool(x.iscode), "user": bool(x.user)}

    def segment_rows():
        for ea in idautils.Segments():
            seg = ida_segment.getseg(ea)
            yield {"start": hex(seg.start_ea), "end": hex(seg.end_ea),
                           "rva": hex(seg.start_ea - base), "name": ida_segment.get_segm_name(seg),
                           "class": ida_segment.get_segm_class(seg), "permissions": int(seg.perm)}

    def import_rows():
        for ordinal in range(ida_nalt.get_import_module_qty()):
            result = []
            module = ida_nalt.get_import_module_name(ordinal)
            def callback(ea, name, number):
                result.append({"address": hex(ea), "rva": hex(ea-base), "name": name,
                               "ordinal": number, "module": module,
                               "xrefs": [ref(x) for x in idautils.XrefsTo(ea, 0)]})
                return True
            ida_nalt.enum_import_names(ordinal, callback)
            yield from result

    def string_rows():
        strings = idautils.Strings()
        strings.setup(strtypes=[idc.STRTYPE_C, idc.STRTYPE_C_16], minlen=1,
                      only_7bit=False, ignore_instructions=True, display_only_existing_strings=False)
        manifest["string_enumeration"] = {
            "types": ["C", "C_16"], "minimum_length": 1,
            "include_undefined_candidates": True, "ignore_instructions": True,
            "scope": "candidate strings; existence does not prove application use",
        }
        for s in strings:
            # get_strlit_contents converts codepoints to UTF-8. It is not the
            # original bytes of a UTF-16 literal (and omits its terminator).
            decoded = ida_bytes.get_strlit_contents(s.ea, s.length, s.strtype)
            raw = ida_bytes.get_bytes(s.ea, s.length)
            yield {"address": hex(s.ea), "rva": hex(s.ea-base), "bytes": s.length,
                           "type": s.strtype, "text": str(s),
                           "raw_hex": raw.hex() if raw is not None else None,
                           "raw_sha256": hashlib.sha256(raw).hexdigest() if raw is not None else None,
                           "decoded_utf8_hex": decoded.hex() if decoded is not None else None,
                           "xrefs": [ref(x) for x in idautils.XrefsTo(s.ea, 0)]}

    def local_types():
        til = ida_typeinf.get_idati()
        ordinal_limit = getattr(ida_typeinf, "get_ordinal_limit", None)
        if ordinal_limit is None:
            ordinal_limit = ida_typeinf.get_ordinal_qty
        for ordinal in range(1, ordinal_limit(til)):
            typ = ida_typeinf.tinfo_t()
            if typ.get_numbered_type(til, ordinal):
                yield {"ordinal": ordinal,
                               "name": ida_typeinf.get_numbered_type_name(til, ordinal),
                               "declaration": str(typ), "size": int(typ.get_size())}

    datasets = {
        "segments": segment_rows,
        "imports": import_rows,
        "strings": string_rows,
        "entrypoints": lambda: ({"index": i, "ordinal": o, "address": hex(ea),
                                 "rva": hex(ea-base), "name": name}
                                for i, o, ea, name in idautils.Entries()),
        "names": lambda: ({"address": hex(ea), "rva": hex(ea-base), "name": name}
                          for ea, name in idautils.Names()),
        "local_types": local_types,
        "structures": lambda: ({"index": i, "id": sid, "name": name,
                                 "members": [{"offset": off, "name": field, "bytes": size}
                                             for off, field, size in idautils.StructMembers(sid)]}
                                for i, sid, name in idautils.Structs()),
    }
    # Keep a packed checkpoint before the potentially long dataset/function
    # pass. An interrupted capture can reopen the same analyzed database.
    if round_number == 0:
        def checkpoint():
            database = Path(ida_loader.get_path(ida_loader.PATH_TYPE_IDB)).resolve()
            if not database.is_relative_to(scratch):
                raise ValueError("Database checkpoint escaped private analysis scratch")
            if not ida_loader.save_database(str(database), ida_loader.DBFL_COMP):
                raise RuntimeError("IDA could not save the private database checkpoint")
            return {"path": database.relative_to(root).as_posix(), "bytes": database.stat().st_size,
                    "sha256_at_checkpoint": file_sha256(database), "saved": True}
        manifest["database_checkpoint"] = guarded("database_checkpoint", checkpoint)
    write_json(output / "capture.json", manifest)
    for name, action in datasets.items():
        path = output / (name + ".json")
        def progress(count):
            manifest["dataset_in_progress"] = {"name": name, "records_written": count}
            write_json(output / "capture.json", manifest)
            print("[STATIC-CAPTURE] {}: {} records".format(name, count), flush=True)
        count = guarded(name, lambda: write_json_array(path, action(), progress=progress))
        if count is None:
            # The streaming writer leaves a prior complete dataset intact on
            # failure. Keep those diagnostic bytes explicitly out of current
            # completion rather than overwriting them with an empty success.
            retained_output = path.is_file()
            manifest.setdefault("dataset_failures", {})[name] = {"retained_previous_output": retained_output}
            if not retained_output:
                write_json(path, None)
        manifest[name + "_count"] = count
        manifest.pop("dataset_in_progress", None)
        write_json(output / "capture.json", manifest)
    checkpoint_interval = max(25, min(1000, len(addresses) // 1000 * 25))

    for index, ea in enumerate(addresses):
        func = ida_funcs.get_func(ea)
        prefix = "{:x}".format(ea-base)
        path = bodies / (prefix + ".json")
        existing = None
        relative_path = path.relative_to(output).as_posix()
        if reuse and path.is_file() and relative_path in retained:
            raw = path.read_bytes()
            if hashlib.sha256(raw).hexdigest() != retained[relative_path]["sha256"]:
                raise ValueError("Retained native function capture byte identity changed: " + relative_path)
            existing = json.loads(raw)
        if existing is not None and existing.get("address") != hex(ea):
            raise ValueError("Existing capture has different function identity")
        if existing is not None and existing.get("end") != hex(func.end_ea):
            existing = None
        if existing is not None:
            manifest["retained_functions_reused"] += 1
        row = existing or {"address": hex(ea), "rva": hex(ea-base), "name": ida_funcs.get_func_name(ea),
               "end": hex(func.end_ea), "flags": int(func.flags), "type": idc.get_type(ea),
               "chunks": [{"start": hex(a), "end": hex(b),
                           "sha256": hashlib.sha256(ida_bytes.get_bytes(a, b-a) or b"").hexdigest(),
                           "raw_hex": (ida_bytes.get_bytes(a, b-a) or b"").hex()}
                          for a, b in idautils.Chunks(ea)],
               "xrefs_to_entry": [ref(x) for x in idautils.XrefsTo(ea, 0)],
               "comments": {"regular": ida_funcs.get_func_cmt(func, False),
                            "repeatable": ida_funcs.get_func_cmt(func, True)}}
        def assembly():
            result = []
            for item in idautils.FuncItems(ea):
                size = ida_bytes.get_item_size(item)
                result.append({"address": hex(item), "rva": hex(item-base),
                               "text": ida_lines.tag_remove(idc.generate_disasm_line(item, 0) or ""),
                               "bytes": (ida_bytes.get_bytes(item, size) or b"").hex(),
                               "xrefs_from": [ref(x) for x in idautils.XrefsFrom(item, 0)],
                               "xrefs_to": [ref(x) for x in idautils.XrefsTo(item, 0)],
                               "comment": idc.get_cmt(item, False),
                               "repeatable_comment": idc.get_cmt(item, True)})
            return result
        if row.get("assembly") is None:
            row["assembly"] = guarded(prefix + ":assembly", assembly)
        if row.get("basic_blocks") is None:
            row["basic_blocks"] = guarded(prefix + ":cfg", lambda: [
            {"id": block.id, "start": hex(block.start_ea), "end": hex(block.end_ea),
             "predecessors": [x.id for x in block.preds()], "successors": [x.id for x in block.succs()]}
            for block in ida_gdl.FlowChart(func)])
        def pseudocode():
            failure = ida_hexrays.hexrays_failure_t()
            cfunc = ida_hexrays.decompile(ea, failure)
            if cfunc is None:
                raise RuntimeError("Hex-Rays returned no function: {} (code {}, address {})".format(
                    failure.desc(), failure.code, hex(failure.errea)))
            result = {"entry": hex(cfunc.entry_ea), "lines": [
                {"line": i, "text": ida_lines.tag_remove(s.line)}
                for i, s in enumerate(cfunc.get_pseudocode())]}
            def locals_list():
                result = []
                for v in cfunc.get_lvars():
                    argument = v.is_arg_var
                    result.append({"name": v.name, "type": str(v.type()),
                                   "argument": bool(argument() if callable(argument) else argument),
                                   "location": str(v.location)})
                return result
            result["locals"] = guarded(prefix + ":locals", locals_list)
            return result
        before = len(manifest["failures"])
        if not hexrays_available:
            row["pseudocode"] = None
            row["pseudocode_status"] = "unverified"
            manifest["failures"].append({
                "stage": prefix + ":hexrays", "reason": "decompiler_unavailable",
                "decompilation_attempted": False,
                "error": "Hex-Rays decompiler is unavailable for architecture {} (IDA processor {}); pseudocode remains unverified".format(
                    manifest["architecture"], manifest["processor"]),
            })
        elif row.get("pseudocode") is None:
            row["pseudocode"] = guarded(prefix + ":hexrays", pseudocode)
            row["pseudocode_status"] = "captured" if row["pseudocode"] is not None else "failed"
        else:
            row["pseudocode_status"] = "captured"
        write_json(path, row)
        manifest["functions"].append({"address": row["address"], "rva": row["rva"], "name": row["name"],
                                      "path": path.relative_to(output).as_posix(),
                                      "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                                      "assembly_captured": row["assembly"] is not None,
                                      "pseudocode_captured": row["pseudocode"] is not None,
                                      "pseudocode_status": row["pseudocode_status"],
                                      "new_failures": len(manifest["failures"]) - before})
        if (index+1) % checkpoint_interval == 0 or index+1 == len(addresses):
            manifest["functions_captured"] = index+1
            write_json(output / "capture.json", manifest)
            print("[STATIC-CAPTURE] {}/{} functions; {} failures".format(index+1, len(addresses), len(manifest["failures"])), flush=True)
        if hexrays_available:
            ida_hexrays.clear_cached_cfuncs()
    for path in (private, original):
        if file_sha256(path) != plan["sha256"]:
            raise ValueError("Input changed during capture")
    manifest["function_pass_finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    manifest["functions_captured"] = len(addresses)
    ida_auto.auto_wait()
    settled = set(idautils.Functions())
    newly_discovered = sorted(settled-set(addresses))
    manifest["discovery_round"] = round_number
    manifest["settled_functions_total"] = len(settled)
    manifest["newly_discovered_addresses"] = [hex(ea) for ea in newly_discovered]
    manifest["removed_function_boundaries"] = [hex(ea) for ea in sorted(set(addresses)-settled)]
    manifest["functions_complete"] = (not newly_discovered
                                    and not manifest["removed_function_boundaries"]
                                    and manifest["database_integrity"]["verified"] is not False
                                    and all(manifest.get(name + "_count") is not None for name in datasets)
                                    and all(row["assembly_captured"] for row in manifest["functions"])
                                    and len(manifest["functions"]) == len(settled))
    # Interrupted aggregate output must never leave a reusable complete receipt.
    manifest["capture_complete"] = False
    manifest["capture_phase"] = "function_capture_settled"
    manifest["all_pseudocode_captured"] = hexrays_available and all(
        r["pseudocode_captured"] for r in manifest["functions"])
    write_json(output / "capture.json", manifest)
    if newly_discovered and round_number < 8:
        return capture(round_number+1)
    emitted = set()
    def function_rows():
        for entry in manifest["functions"]:
            path = output / entry["path"]
            emitted.add(path.resolve())
            yield json.loads(path.read_bytes())
    aggregate_count = guarded("function_aggregates", lambda: write_function_aggregates(output, function_rows()))
    manifest["aggregate_functions_written"] = aggregate_count
    # Keep exactly the current database's function capture, after analysis settles.
    if aggregate_count is not None:
        for path in bodies.glob("*.json"):
            if path.resolve() not in emitted:
                path.unlink()
    manifest["outputs"] = [{"path": path.relative_to(output).as_posix(), "bytes": path.stat().st_size,
                            "sha256": file_sha256(path)}
                           for path in sorted(output.iterdir()) if path.is_file()
                           and path.name not in {"capture.json", "plan.json"} and not path.name.endswith("session.json")]
    manifest["capture_complete"] = manifest["functions_complete"] and aggregate_count == len(manifest["functions"])
    manifest["capture_phase"] = "complete" if manifest["capture_complete"] else "incomplete"
    manifest["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    write_json(output / "capture.json", manifest)
    return manifest

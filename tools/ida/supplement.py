"""Capture evidenced native entries from a private copy of a static IDB.

This supplements gaps in IDA's function enumeration without changing the
original full capture. Missing boundaries may be created only for byte-exact
x86/x64 direct-jump thunks, the exact x64 embedded-object destructor forwarding
thunk or the exact x64 bound-member tail-call thunk; other unknown code remains
an explicit failure.
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
from types import SimpleNamespace

import ida_auto
import ida_bytes
import ida_frame
import ida_funcs
import ida_gdl
import ida_hexrays
import ida_ida
import ida_idp
import ida_kernwin
import ida_lines
import ida_nalt
import ida_segment
import ida_typeinf
import ida_ua
import idaapi
import idautils
import idc

sys.path.insert(0, str(Path(__file__).resolve().parent))
import capture_values


def _static_diagnostic_error(record, stage, error):
    """Keep diagnostic exceptions as bounded text, never traceback/proxy objects."""
    try:
        message = type(error).__name__ + ": " + str(error)
    except Exception:
        message = type(error).__name__
    errors = record.setdefault("diagnostic_errors", [])
    if len(errors) < 4:
        errors.append({"stage": stage, "error": message[:2048]})
    else:
        record["diagnostic_errors_truncated"] = True
    if len(message) > 2048:
        record["diagnostic_errors_truncated"] = True


def local_namespace_diagnostics(cfunc):
    """Observe SDK register/frame namespaces without repairing local locations."""
    result = {"schema_version": 1, "diagnostics_complete": True, "locals": [],
              "target_execution": False, "local_capture_result_unchanged": True}

    def error(record, stage, exception):
        result["diagnostics_complete"] = False
        _static_diagnostic_error(record, stage, exception)

    def address(mba, ea):
        value = {"definition_ea": hex(ea), "status": "unobserved"}
        try:
            mapped = int(mba.map_fict_ea(ea))
            value.update(api_mapped_ea=hex(mapped),
                         status="badaddr" if mapped == idaapi.BADADDR else "api_returned")
            if mapped != idaapi.BADADDR:
                owner = ida_funcs.get_func(mapped)
                value["mapped_function_owner"] = (None if owner is None else
                    {"start_ea": hex(int(owner.start_ea)), "end_ea": hex(int(owner.end_ea)),
                     "flags": int(owner.flags)})
        except Exception as exception:
            value["status"] = "diagnostic_error"
            error(value, "definition_address_mapping", exception)
        return value

    def register(number, width):
        value = {"namespace": "hexrays_microregisters", "register": number, "width": width}
        for key, api in (("name", lambda: ida_hexrays.get_mreg_name(number, width)),
                         ("processor_register", lambda: int(ida_hexrays.mreg2reg(number, width))),
                         ("kernel_register", lambda: bool(ida_hexrays.is_kreg(number))),
                         ("temporary_register", lambda: bool(ida_hexrays.get_temp_regs().has(number)))):
            observation = {"status": "unobserved"}
            value[key] = observation
            try:
                observed = api()
                if key == "name" and not isinstance(observed, str):
                    raise TypeError("SDK microregister formatter did not return text")
                observation.update(status="api_returned", value=observed)
                if key == "processor_register":
                    observation["maps_to_processor_register"] = observed != -1
            except Exception as exception:
                observation["status"] = "diagnostic_error"
                error(observation, "microregister." + key, exception)
        return value

    def location_registers(location, width):
        kind = location.get("kind")
        if kind in ("register", "register_relative"):
            return [register(location["register"], width)]
        if kind == "register_pair":
            return [register(location[key], width) for key in ("register", "second_register")]
        if kind == "scattered":
            return [item for part in location.get("parts", [])
                    for item in location_registers(part, part["size"])]
        return []

    try:
        mba = cfunc.mba
        result.update(mba_entry_ea=hex(int(mba.entry_ea)),
                      cfunc_stack_offset_delta=int(cfunc.get_stkoff_delta()),
                      mba_block_count=int(mba.qty))
        for index, variable in enumerate(cfunc.get_lvars()):
            value = {"index": index}
            result["locals"].append(value)
            try:
                width, block_number = int(variable.width), int(variable.defblk)
                value.update(name=str(variable.name), width=width, definition_block=block_number)
                value["definition_address"] = address(mba, int(variable.defea))
                location = capture_values.argument_location(variable.location, "hexrays_microregisters")
                value["location"] = location
                value["register_observations"] = location_registers(location, width)
                if 0 <= block_number < result["mba_block_count"]:
                    block = mba.get_mblock(block_number)
                    flags = int(block.flags)
                    definition = {"status": "api_returned", "serial": int(block.serial), "flags": flags,
                                  "inlined": bool(flags & ida_hexrays.MBL_INLINED),
                                  "external_frame": bool(flags & ida_hexrays.MBL_EXTFRAME)}
                    value["definition_block_observation"] = definition
                    for key in ("head", "tail"):
                        instruction = getattr(block, key)
                        definition[key] = None if instruction is None else address(mba, int(instruction.ea))
                else:
                    value["definition_block_observation"] = {"status": "unobserved",
                        "reason": "sdk_definition_block_outside_current_mba"}
            except Exception as exception:
                error(value, "local_namespace", exception)
    except Exception as exception:
        error(result, "local_namespace_owner", exception)
    return result


class _FrozenInsnVisitor(ida_hexrays.minsn_visitor_t):
    def __init__(self, mba, block, trace, row):
        super().__init__(mba, block)
        self.trace, self.row = trace, row

    def visit_minsn(self):
        trace, row = self.trace, self.row
        if (len(row["instructions"]) >= trace.limits["instructions_per_event"]
                or trace.instruction_records >= trace.limits["instructions_total"]):
            trace._truncate(row, "instruction_records")
            # Stop only this read-only visitor, never the Hex-Rays callback.
            return 1
        item = {"mapping_status": "unobserved"}
        row["instructions"].append(item)
        trace.instruction_records += 1
        try:
            insn = self.curins
            address = int(insn.ea)
            item.update(ea=hex(address), opcode=int(insn.opcode))
        except Exception as error:
            trace._error(row, item, "instruction_identity", error)
            return 0
        try:
            mapped = int(self.mba.map_fict_ea(address))
            item.update(api_mapped_ea=hex(mapped),
                        mapping_status="badaddr" if mapped == idaapi.BADADDR else "api_returned")
        except Exception as error:
            item["mapping_status"] = "diagnostic_error"
            trace._error(row, item, "map_fict_ea", error)
        try:
            trace._text(row, item, "text", str(insn.dstr()), "instruction_text")
        except Exception as error:
            trace._error(row, item, "instruction_text", error)
        try:
            block = self.blk
            if block is not None:
                flags = int(block.flags)
                item.update(block=int(block.serial), block_flags=flags,
                            inlined=bool(flags & ida_hexrays.MBL_INLINED),
                            external_frame=bool(flags & ida_hexrays.MBL_EXTFRAME))
        except Exception as error:
            trace._error(row, item, "instruction_block", error)
        try:
            item["operands"] = trace._instruction_operands(row, insn, self.mba)
            item["stack_point_queries"] = trace._instruction_stack_points(row, item, self.mba)
            if self.blk is not None:
                trace._capture_block(row, self.blk)
        except Exception as error:
            trace._error(row, item, "instruction_coordinates", error)
        try:
            if (item["opcode"] in (ida_hexrays.m_call, ida_hexrays.m_icall)
                    or row.get("block_tail_ea") == item["ea"]):
                trace._capture_call(row, item, insn, self.mba)
        except Exception as error:
            trace._error(row, item, "call_observation", error)
        return 0


class StaticMicrocodeTrace(ida_hexrays.Hexrays_Hooks):
    """Copy bounded static observations while each callback's MBA is alive."""
    DEFAULT_LIMITS = {"events": 128, "instructions_per_event": 4096,
                      "instructions_total": 32768, "text_characters": 2048,
                      "text_characters_total": 1048576, "inline_ranges": 256, "owner_chunks": 256,
                      "calls_per_event": 64, "calls_total": 1024, "types_total": 3072,
                      "arguments_per_type": 32, "arguments_total": 8192,
                      "argloc_parts": 16, "argloc_depth": 2, "argloc_records_total": 16384,
                      "function_owners": 32, "operand_depth": 4, "operand_records_total": 65536,
                      "stack_points_per_event": 1024, "stack_points_total": 32768,
                      "blocks_per_event": 512, "blocks_total": 8192, "cfg_edges": 256,
                      "inline_context_depth": 16, "probe_heads": 512, "mba_objects_per_event": 64}
    FRAME_FIELDS = ("frsize", "frregs", "fpd", "fullsize", "stacksize", "inargoff", "minstkref",
                    "tmpstk_size", "retsize", "spd_adjust")

    def __init__(self, target_ea):
        super().__init__()
        self.target_ea = int(target_ea)
        self.limits = dict(self.DEFAULT_LIMITS)
        self.active = False
        self.events = []
        self.callbacks_seen = self.instruction_records = self.diagnostic_error_count = 0
        self.dropped_events = 0
        self.call_records = self.type_records = self.argument_records = self.argloc_records = self.text_characters = 0
        self.operand_records = self.stack_point_records = self.block_records = 0
        self.inline_contexts = []
        self.inline_context_overflow = 0
        self._mba_indexes, self._point_indexes, self._block_indexes = {}, {}, {}
        self.truncation = {key: False for key in (
            "event_records", "instruction_records", "instruction_text", "inline_ranges", "owner_chunks",
            "call_records", "type_records", "argument_records", "argloc_parts", "argloc_depth",
            "argloc_records", "function_owners", "detail_text", "operand_records", "operand_depth",
            "stack_point_records", "block_records", "cfg_edges", "inline_context_depth", "probe_heads", "mba_objects")}

    def _event(self, phase):
        if not self.active:
            return None
        sequence = self.callbacks_seen
        self.callbacks_seen += 1
        if len(self.events) >= self.limits["events"]:
            self.dropped_events += 1
            self.truncation["event_records"] = True
            return None
        row = {"phase": phase, "target_ea": hex(self.target_ea), "sequence": sequence,
               "instructions": [], "complete": True, "truncated": []}
        if self.inline_contexts and not self.inline_context_overflow:
            # Contexts contain frozen scalars/lists only and are never mutated.
            row["inline_context"] = dict(self.inline_contexts[-1])
        elif self.inline_context_overflow:
            row["inline_context_status"] = "depth_limit"
            self._truncate(row, "inline_context_depth")
        self.events.append(row)
        return row

    def _truncate(self, row, kind):
        self.truncation[kind] = True
        row["complete"] = False
        if kind not in row["truncated"]:
            row["truncated"].append(kind)

    def _error(self, row, record, stage, error):
        row["complete"] = False
        self.diagnostic_error_count += 1
        _static_diagnostic_error(record, stage, error)

    def _text(self, row, record, key, value, kind="detail_text"):
        if not isinstance(value, str):
            raise TypeError("SDK formatter did not return text")
        available = max(0, self.limits["text_characters_total"] - self.text_characters)
        text = value[:min(self.limits["text_characters"], available)]
        record[key] = text
        self.text_characters += len(text)
        if len(text) < len(value):
            record[key + "_truncated"] = True
            self._truncate(row, kind)

    def _type_text(self, row, record, key, tif):
        try:
            self._text(row, record, key, ida_typeinf.print_tinfo(
                "", 0, 0, ida_typeinf.PRTYPE_1LINE, tif, "", ""))
        except Exception as error:
            self._error(row, record, key, error)

    def _scalar_fields(self, row, source, names, stage):
        values = {}
        for name in names:
            value = {"status": "unobserved"}
            values[name] = value
            try:
                value.update(status="api_returned", value=int(getattr(source, name)))
            except AttributeError as error:
                value["reason"] = "sdk_attribute_unavailable"
                self._error(row, value, stage + "." + name, error)
            except Exception as error:
                value["status"] = "diagnostic_error"
                self._error(row, value, stage + "." + name, error)
        return values

    def _mba_reference(self, row, mba, observed_mba):
        """Copy MBA identity/frame metadata without retaining a SWIG proxy."""
        values = row.setdefault("mba_objects", [])
        indexes = self._mba_indexes.setdefault(row["sequence"], {})
        token = None
        same = {"status": "unobserved"}
        try:
            # SWIG's scalar address conversion is used only for equality/dedup.
            token = int(mba.this)
            same.update(status="api_returned", value=token == int(observed_mba.this))
        except Exception as error:
            self._error(row, same, "mba_identity", error)
        if token is not None and token in indexes:
            return indexes[token]
        if len(values) >= self.limits["mba_objects_per_event"]:
            self._truncate(row, "mba_objects")
            return None
        index = len(values)
        value = {"reference": index, "same_as_observed_mba": same}
        values.append(value)
        if token is not None:
            indexes[token] = index
        try:
            value["entry_ea"] = hex(int(mba.entry_ea))
            owner = mba.mbr.pfn
            value["function_owner_ea"] = None if owner is None else hex(int(owner.start_ea))
        except Exception as error:
            self._error(row, value, "mba_owner", error)
        value["frame"] = (row["mba_frame"] if same.get("value") is True and "mba_frame" in row
                          else self._scalar_fields(row, mba, self.FRAME_FIELDS, "operand_mba_frame"))
        return index

    def _operand(self, row, operand, mba, depth=0):
        value = {"status": "unobserved"}
        if self.operand_records >= self.limits["operand_records_total"]:
            value["reason"] = "operand_record_limit"
            self._truncate(row, "operand_records")
            return value
        self.operand_records += 1
        try:
            kind = int(operand.t)
            value.update(status="observed", kind=kind, size=int(operand.size))
            if kind == ida_hexrays.mop_S:
                reference = operand.s
                if reference is None or reference.mba is None:
                    raise ValueError("stack operand has no owning MBA")
                owner = reference.mba
                offset = int(reference.off)
                value["stack"] = {"decompiler_offset": offset,
                                  "mba_reference": self._mba_reference(row, owner, mba)}
                conversion = {"status": "unobserved"}
                value["stack"]["coordinate_conversion"] = conversion
                try:
                    ida_offset = int(owner.stkoff_vd2ida(offset))
                    conversion.update(ida_offset=ida_offset)
                    returned = int(owner.stkoff_ida2vd(ida_offset))
                    conversion.update(status="api_returned", roundtrip_decompiler_offset=returned,
                                      roundtrip_matches=returned == offset)
                except Exception as error:
                    conversion["status"] = "diagnostic_error"
                    self._error(row, conversion, "stack_coordinate_conversion", error)
            elif kind == ida_hexrays.mop_r:
                register = int(operand.r)
                value["register"] = register
                self._text(row, value, "register_name", ida_hexrays.get_mreg_name(register, int(operand.size)))
            elif kind == ida_hexrays.mop_n:
                value.update(unsigned_value=int(operand.unsigned_value()), signed_value=int(operand.signed_value()))
            elif kind in (ida_hexrays.mop_a, ida_hexrays.mop_d):
                if depth >= self.limits["operand_depth"]:
                    value.update(status="truncated", reason="operand_depth_limit")
                    self._truncate(row, "operand_depth")
                elif kind == ida_hexrays.mop_a:
                    # mop_addr_t derives from mop_t; its own tag selects data.
                    address = operand.a
                    if address is None:
                        raise ValueError("address operand has no tagged operand")
                    value["address_operand"] = self._operand(row, address, mba, depth + 1)
                else:
                    nested = operand.d
                    if nested is None:
                        raise ValueError("nested operand has no instruction")
                    value["instruction"] = {"ea": hex(int(nested.ea)), "opcode": int(nested.opcode),
                        "operands": self._instruction_operands(row, nested, mba, depth + 1)}
        except Exception as error:
            value["status"] = "diagnostic_error"
            self._error(row, value, "microcode_operand", error)
        return value

    def _instruction_operands(self, row, instruction, mba, depth=0):
        values = {}
        for name in ("l", "r", "d"):
            try:
                values[name] = self._operand(row, getattr(instruction, name), mba, depth)
            except Exception as error:
                values[name] = {"status": "diagnostic_error"}
                self._error(row, values[name], "instruction_operand." + name, error)
        return values

    def _stack_point(self, row, mba, address):
        values = row.setdefault("stack_points", [])
        reference = self._mba_reference(row, mba, mba)
        if reference is None:
            return {"status": "unobserved", "reason": "mba_object_record_limit"}
        key = (reference, address)
        indexes = self._point_indexes.setdefault(row["sequence"], {})
        if key in indexes:
            return {"status": "recorded", "record": indexes[key]}
        if (len(values) >= self.limits["stack_points_per_event"]
                or self.stack_point_records >= self.limits["stack_points_total"]):
            self._truncate(row, "stack_point_records")
            return {"status": "unobserved", "reason": "stack_point_record_limit"}
        index = len(values)
        indexes[key] = index
        value = {"query_ea": hex(address), "mba_reference": reference, "status": "unobserved",
                 "source": "live_mba.locate_stkpnt"}
        values.append(value)
        self.stack_point_records += 1
        try:
            point = mba.locate_stkpnt(address)
            if point is None:
                value["reason"] = "locate_stkpnt_returned_null"
            else:
                point_ea = int(point.ea)
                value.update(status="api_returned", point_ea=hex(point_ea), spd=int(point.spd),
                             point_ea_matches_query=point_ea == address)
        except Exception as error:
            value["status"] = "diagnostic_error"
            self._error(row, value, "live_mba_stack_point", error)
        return {"status": "recorded", "record": index}

    def _instruction_stack_points(self, row, item, mba):
        value = {"instruction_ea": self._stack_point(row, mba, int(item["ea"], 16))}
        if item.get("mapping_status") == "api_returned":
            value["mapped_original_ea"] = self._stack_point(row, mba, int(item["api_mapped_ea"], 16))
        else:
            value["mapped_original_ea"] = {"status": "unobserved", "reason": "no_api_mapped_address"}
        return value

    def _mapped_address(self, row, mba, address):
        value = {"ea": hex(address), "mapping_status": "unobserved"}
        try:
            mapped = int(mba.map_fict_ea(address))
            value.update(api_mapped_ea=hex(mapped),
                         mapping_status="badaddr" if mapped == idaapi.BADADDR else "api_returned")
        except Exception as error:
            value["mapping_status"] = "diagnostic_error"
            self._error(row, value, "map_block_address", error)
        return value

    def _capture_block(self, row, block):
        values = row.setdefault("blocks", [])
        indexes = self._block_indexes.setdefault(row["sequence"], set())
        serial = int(block.serial)
        if serial in indexes:
            return
        if len(values) >= self.limits["blocks_per_event"] or self.block_records >= self.limits["blocks_total"]:
            self._truncate(row, "block_records")
            return
        indexes.add(serial)
        self.block_records += 1
        value = {"serial": serial, "flags": int(block.flags), "maxbsp": int(block.maxbsp)}
        values.append(value)
        for field in ("head", "tail"):
            instruction = getattr(block, field)
            value[field] = None if instruction is None else self._mapped_address(row, block.mba, int(instruction.ea))
        for field, count_api, item_api in (("predecessors", block.npred, block.pred),
                                            ("successors", block.nsucc, block.succ)):
            count = int(count_api())
            value[field + "_count"] = count
            value[field] = [int(item_api(index)) for index in range(min(count, self.limits["cfg_edges"]))]
            if count > self.limits["cfg_edges"]:
                self._truncate(row, "cfg_edges")

    def _range_probe_heads(self, row, ranges):
        addresses, seen = [], set()
        visited = 0
        for interval in ranges:
            start, stop = int(interval["start"], 16), int(interval["end"], 16)
            for address in idautils.Heads(start, stop):
                if visited >= self.limits["probe_heads"]:
                    self._truncate(row, "probe_heads")
                    return addresses
                visited += 1
                address = int(address)
                if address in seen:
                    continue
                seen.add(address)
                addresses.append(address)
        return addresses

    def _capture_early_stack_points(self, row, mba):
        probes = []
        row["range_stack_point_queries"] = probes
        try:
            mbr = mba.mbr
            if mbr.pfn is not None:
                ranges = []
                for index, (start, stop) in enumerate(idautils.Chunks(int(mbr.pfn.start_ea))):
                    if index >= self.limits["owner_chunks"]:
                        self._truncate(row, "owner_chunks")
                        break
                    ranges.append({"start": hex(int(start)), "end": hex(int(stop))})
            else:
                ranges = []
                for index, interval in enumerate(mbr.ranges):
                    if index >= self.limits["inline_ranges"]:
                        self._truncate(row, "inline_ranges")
                        break
                    ranges.append({"start": hex(int(interval.start_ea)), "end": hex(int(interval.end_ea))})
            row["mba_probe_ranges"] = ranges
            for address in self._range_probe_heads(row, ranges):
                probes.append({"role": "current_mba_range_head", "ea": hex(address),
                               "query": self._stack_point(row, mba, address)})
            context = row.get("inline_context", {})
            for address in context.get("probe_eas", []):
                probes.append({"role": "active_inline_original_head", "ea": address,
                               "query": self._stack_point(row, mba, int(address, 16))})
            source = context.get("source_call", {})
            for field in ("ea", "api_mapped_ea"):
                if source.get(field) is not None and source[field] != hex(idaapi.BADADDR):
                    probes.append({"role": "active_inline_source_" + field, "ea": source[field],
                                   "query": self._stack_point(row, mba, int(source[field], 16))})
        except Exception as error:
            self._error(row, row, "early_stack_points", error)

    def _argloc(self, row, location, depth=0):
        value = {"status": "unobserved"}
        if self.argloc_records >= self.limits["argloc_records_total"]:
            value["reason"] = "argloc_record_limit"
            self._truncate(row, "argloc_records")
            return value
        self.argloc_records += 1
        try:
            value["atype"] = int(location.atype())
            if location.is_badloc():
                value["reason"] = "no_location"
            elif location.is_reg1():
                value.update(status="observed", kind="register", register=int(location.reg1()),
                             register_offset=int(location.regoff()))
            elif location.is_reg2():
                value.update(status="observed", kind="register_pair", register=int(location.reg1()),
                             second_register=int(location.reg2()))
            elif location.is_stkoff():
                value.update(status="observed", kind="stack", stack_offset=int(location.stkoff()))
            elif location.is_ea():
                value.update(status="observed", kind="static_address", address=hex(int(location.get_ea())))
            elif location.is_rrel():
                relative = location.get_rrel()
                value.update(status="observed", kind="register_relative", register=int(relative.reg),
                             offset=int(relative.off))
            elif location.is_scattered():
                value.update(status="observed", kind="scattered", parts=[])
                if depth >= self.limits["argloc_depth"]:
                    value["status"] = "truncated"
                    self._truncate(row, "argloc_depth")
                else:
                    parts = location.scattered()
                    count = int(parts.size())
                    value["part_count"] = count
                    for index in range(min(count, self.limits["argloc_parts"])):
                        if self.argloc_records >= self.limits["argloc_records_total"]:
                            value["status"] = "truncated"
                            self._truncate(row, "argloc_records")
                            break
                        part = parts.at(index)
                        item = self._argloc(row, part, depth + 1)
                        item.update(argument_offset=int(part.off), size=int(part.size))
                        value["parts"].append(item)
                    if count > self.limits["argloc_parts"]:
                        value["status"] = "truncated"
                        self._truncate(row, "argloc_parts")
            elif location.is_custom():
                value["reason"] = "custom_location_payload_not_observed"
            else:
                value["reason"] = "unknown_location_kind"
        except Exception as error:
            value["status"] = "diagnostic_error"
            self._error(row, value, "argument_location", error)
        return value

    def _arguments(self, row, vector, *, microcode=False, mba=None):
        result = {"status": "unobserved", "arguments": []}
        try:
            count = int(vector.size())
            result.update(status="observed", count=count)
            available = max(0, self.limits["arguments_total"] - self.argument_records)
            limit = min(count, self.limits["arguments_per_type"], available)
            for index in range(limit):
                value = {"index": index}
                result["arguments"].append(value)
                self.argument_records += 1
                try:
                    arg = vector.at(index)
                    value["flags"] = int(arg.flags)
                    self._text(row, value, "name", str(arg.name))
                    self._type_text(row, value, "type", arg.type)
                    value["location"] = self._argloc(row, arg.argloc)
                    if microcode:
                        address = int(arg.ea)
                        value.update(initialized_ea=hex(address),
                                     initialized_ea_status="unobserved" if address == idaapi.BADADDR else "api_returned",
                                     mop_kind=int(arg.t), size=int(arg.size))
                        self._text(row, value, "value_text", str(arg.dstr()))
                        value["operand"] = self._operand(row, arg, mba)
                except Exception as error:
                    self._error(row, value, "argument", error)
            if limit < count:
                result["status"] = "truncated"
                self._truncate(row, "argument_records")
        except Exception as error:
            result["status"] = "diagnostic_error"
            self._error(row, result, "arguments", error)
        return result

    def _function_type(self, row, tif, mode):
        value = {"status": "unobserved", "argloc_request": mode}
        try:
            details = ida_typeinf.func_type_data_t()
            flags = ida_typeinf.GTD_NO_ARGLOCS if mode == "no_calculation" else ida_typeinf.GTD_CALC_ARGLOCS
            if not tif.get_func_details(details, flags):
                value["reason"] = "get_func_details_returned_false"
                return value
            value.update(status="observed", cc=int(details.cc), flags=int(details.flags),
                         stack_arguments_size=int(details.stkargs))
            self._type_text(row, value, "return_type", details.rettype)
            value["return_location"] = self._argloc(row, details.retloc)
            value["parameters"] = self._arguments(row, details)
        except Exception as error:
            value["status"] = "diagnostic_error"
            self._error(row, value, "function_type_details", error)
        return value

    def _stored_type(self, row, address, operand=None):
        value = {"status": "unobserved", "address": hex(address),
                 "api": "ida_nalt.get_tinfo" if operand is None else "ida_nalt.get_op_tinfo"}
        if operand is not None:
            value["operand"] = operand
        if self.type_records >= self.limits["types_total"]:
            value["reason"] = "type_record_limit"
            self._truncate(row, "type_records")
            return value
        self.type_records += 1
        try:
            tif = ida_typeinf.tinfo_t()
            observed = (ida_nalt.get_tinfo(tif, address) if operand is None
                        else ida_nalt.get_op_tinfo(tif, address, operand))
            if not observed:
                value["reason"] = "no_stored_tinfo"
                return value
            value["status"] = "observed_stored_tinfo"
            self._type_text(row, value, "declaration", tif)
            if tif.is_func():
                value["kind"] = "function"
                function = tif
            elif tif.is_funcptr():
                value["kind"] = "function_pointer"
                function = tif.get_pointed_object()
            else:
                value["kind"] = "not_function"
                return value
            # NO_ARGLOCS can expose previously calculated locations. Keep both
            # API requests separate; neither proves serialized argloc storage.
            value["serialized_locations_verified"] = False
            value["without_argloc_calculation"] = self._function_type(row, function, "no_calculation")
            value["with_argloc_calculation"] = self._function_type(row, function, "calculate")
        except Exception as error:
            value["status"] = "diagnostic_error"
            self._error(row, value, "stored_tinfo", error)
        return value

    def _owners_and_sp(self, row, address):
        result = {"status": "unobserved", "owners": [], "owners_complete": False,
                  "sp_source": "static_idb_analysis", "sp_delta_zero_means": "no_change_point_at_address"}
        try:
            primary = ida_funcs.get_func(address)
            result["primary_owner_ea"] = None if primary is None else hex(int(primary.start_ea))
            chunk = ida_funcs.get_fchunk(address)
            if chunk is None:
                result["reason"] = "no_function_chunk"
                return result
            result["chunk"] = {"start": hex(int(chunk.start_ea)), "end": hex(int(chunk.end_ea)),
                               "flags": int(chunk.flags), "is_tail": bool(ida_funcs.is_func_tail(chunk))}
            if result["chunk"]["is_tail"]:
                parents = ida_funcs.func_parent_iterator_t()
                if not parents.set(chunk):
                    result["reason"] = "parent_iterator_rejected_chunk"
                    return result
                addresses = iter(parents)
            else:
                addresses = iter((int(chunk.start_ea),))
            seen = set()
            result["status"] = "observed"
            for index, owner_ea in enumerate(addresses):
                if index >= self.limits["function_owners"]:
                    result["status"] = "truncated"
                    self._truncate(row, "function_owners")
                    break
                owner_ea = int(owner_ea)
                owner = {"owner_ea": hex(owner_ea), "status": "unobserved"}
                result["owners"].append(owner)
                try:
                    if owner_ea in seen:
                        raise ValueError("parent iterator repeated an owner")
                    seen.add(owner_ea)
                    function = ida_funcs.get_func(owner_ea)
                    if function is None or int(function.start_ea) != owner_ea:
                        raise ValueError("parent does not resolve to its function entry")
                    number = int(ida_funcs.get_func_chunknum(function, address))
                    owner.update(flags=int(function.flags), containing_chunk_number=number)
                    if number < 0:
                        raise ValueError("parent function does not contain the instruction")
                    owner["status"] = "observed"
                    owner["frame"] = self._scalar_fields(row, function,
                        ("frame", "frsize", "frregs", "argsize", "fpd"), "owner_frame")
                    for name, getter in (("spd_before", ida_frame.get_spd),
                                         ("effective_spd", ida_frame.get_effective_spd),
                                         ("sp_change_point_delta", ida_frame.get_sp_delta)):
                        try:
                            owner[name] = {"status": "api_returned", "value": int(getter(function, address))}
                        except Exception as error:
                            owner[name] = {"status": "diagnostic_error"}
                            self._error(row, owner[name], name, error)
                except Exception as error:
                    owner["status"] = "diagnostic_error"
                    self._error(row, owner, "function_owner", error)
            else:
                result["owners_complete"] = bool(result["owners"]) and all(
                    owner["status"] == "observed" for owner in result["owners"])
                if not result["owners"]:
                    result.update(status="unobserved", reason="parent_iterator_returned_no_owners")
        except Exception as error:
            result["status"] = "diagnostic_error"
            self._error(row, result, "owners_and_sp", error)
        return result

    def _original_call(self, row, item):
        value = {"status": "unobserved", "source": "current_static_idb", "original_pe_validated": False}
        if item.get("mapping_status") != "api_returned":
            value["reason"] = "no_api_mapped_address"
            return value
        try:
            address = int(item["api_mapped_ea"], 16)
            value["address"] = hex(address)
            if not ida_bytes.is_loaded(address) or not ida_bytes.is_code(ida_bytes.get_full_flags(address)):
                value["reason"] = "mapped_address_is_not_loaded_code"
                return value
            if int(ida_bytes.get_item_head(address)) != address:
                value["reason"] = "mapped_address_is_not_instruction_head"
                return value
            instruction = ida_ua.insn_t()
            size = int(ida_ua.decode_insn(instruction, address))
            if size <= 0:
                value["reason"] = "decode_returned_zero"
                return value
            if int(instruction.ea) != address or int(instruction.size) != size or int(ida_bytes.get_item_size(address)) != size:
                value["reason"] = "decoded_instruction_disagrees_with_idb_item"
                return value
            value.update(status="observed", size=size, itype=int(instruction.itype),
                         is_call=bool(ida_idp.is_call_insn(instruction)),
                         is_indirect_jump=bool(ida_idp.is_indirect_jump_insn(instruction)))
            value["function_context"] = self._owners_and_sp(row, address)
            if not value["is_call"]:
                value["call_operands"] = {"status": "unobserved", "reason": "mapped_instruction_is_not_call"}
                if not value["is_indirect_jump"]:
                    return value
                value["transfer_kind"] = "indirect_jump"
                value["instruction_stored_tinfo"] = self._stored_type(row, address)
            else:
                value["transfer_kind"] = "call"
            value["operands"] = []
            for index in range(8):
                op = instruction.ops[index]
                kind = int(op.type)
                if kind == ida_ua.o_void:
                    break
                operand = {"index": index, "n": int(op.n), "kind": kind, "dtype": int(op.dtype)}
                value["operands"].append(operand)
                operand["stored_tinfo"] = self._stored_type(row, address, int(op.n))
                if kind in (ida_ua.o_near, ida_ua.o_far, ida_ua.o_mem):
                    target = int(op.addr)
                    role = "memory_slot" if kind == ida_ua.o_mem else "direct_callee"
                    operand.update(address=hex(target), address_role=role)
                    operand["address_stored_tinfo"] = self._stored_type(row, target)
                    if kind == ida_ua.o_mem:
                        operand["runtime_target"] = {"status": "unobserved", "reason": "memory_slot_not_dereferenced"}
                elif kind == ida_ua.o_reg:
                    operand["register"] = int(op.reg)
                elif kind in (ida_ua.o_phrase, ida_ua.o_displ):
                    operand["phrase"] = int(op.phrase)
                    if kind == ida_ua.o_displ:
                        operand["displacement"] = int(op.addr)
        except Exception as error:
            value["status"] = "diagnostic_error"
            self._error(row, value, "original_instruction", error)
        return value

    def _microcode_callinfo(self, row, instruction, mba):
        value = {"status": "unobserved"}
        try:
            destination = instruction.d
            if int(destination.t) != ida_hexrays.mop_f:
                value["reason"] = "callinfo_operand_not_present"
                return value
            info = destination.f
            if info is None:
                value["reason"] = "null_callinfo"
                return value
            value["status"] = "observed"
            for name in ("call_spd", "stkargs_top", "cc", "solid_args"):
                try:
                    value[name] = int(getattr(info, name))
                except Exception as error:
                    self._error(row, value, name, error)
            callee = int(info.callee)
            value.update(callee=hex(callee), callee_status="unobserved" if callee == idaapi.BADADDR else "api_returned")
            self._type_text(row, value, "return_type", info.return_type)
            value["return_location"] = self._argloc(row, info.return_argloc)
            value["parameters"] = self._arguments(row, info.args, microcode=True, mba=mba)
        except Exception as error:
            value["status"] = "diagnostic_error"
            self._error(row, value, "microcode_callinfo", error)
        return value

    def _capture_call(self, row, item, instruction, mba):
        count = row.get("call_records", 0)
        if count >= self.limits["calls_per_event"] or self.call_records >= self.limits["calls_total"]:
            item["callsite"] = {"status": "unobserved", "reason": "call_record_limit"}
            self._truncate(row, "call_records")
            return
        row["call_records"] = count + 1
        self.call_records += 1
        item["callsite"] = {"original_instruction": self._original_call(row, item),
                            "microcode_callinfo": self._microcode_callinfo(row, instruction, mba)}

    def _capture_mba(self, row, mba, block=None, *, instructions=True):
        visitor = None
        try:
            row["mba_entry_ea"] = hex(int(mba.entry_ea))
            row["mba_maturity"] = int(mba.maturity)
            row["mba_flags"] = int(mba.get_mba_flags())
            row["mba_flags2"] = int(mba.get_mba_flags2())
            row["has_outlines"] = bool(mba.has_outlines())
        except Exception as error:
            self._error(row, row, "mba_metadata", error)
        row["mba_frame"] = self._scalar_fields(row, mba, self.FRAME_FIELDS, "mba_frame")
        row["mba_boundaries"] = self._scalar_fields(row, mba,
            ("last_prolog_ea", "first_epilog_ea"), "mba_boundaries")
        row["observed_mba_reference"] = self._mba_reference(row, mba, mba)
        if not instructions or row.get("inline_context"):
            self._capture_early_stack_points(row, mba)
        if not instructions:
            return
        try:
            if block is not None:
                row.update(observed_block=int(block.serial), block_tail_ea=hex(int(block.tail.ea)))
            visitor = _FrozenInsnVisitor(mba, block, self, row)
            result = block.for_all_insns(visitor) if block is not None else mba.for_all_insns(visitor)
            if result != 0 and "instruction_records" not in row["truncated"]:
                self._error(row, row, "instruction_visitor", ValueError("visitor returned a nonzero code"))
        except Exception as error:
            self._error(row, row, "instruction_visitor", error)
        finally:
            # Inherited mba/blk/curins/topins are borrowed; keep the visitor local
            # and release it before returning control to the decompiler.
            if visitor is not None:
                del visitor

    def _observe(self, phase, mba):
        row = self._event(phase)
        if row is not None:
            self._capture_mba(row, mba)
        return 0

    def _inline_event(self, phase, cdg, block, mbr, first=None, stop=None):
        row = self._event(phase)
        if row is None:
            return 0
        value = {}
        try:
            value = {"source_block": int(block), "owner_ea": None, "owner_flags": None,
                     "owner_chunks": [], "listed_ranges": []}
            row["inline_event"] = value
            for index, interval in enumerate(mbr.ranges):
                if index >= self.limits["inline_ranges"]:
                    self._truncate(row, "inline_ranges")
                    break
                value["listed_ranges"].append({"start": hex(int(interval.start_ea)), "end": hex(int(interval.end_ea))})
            owner = mbr.pfn
            if owner is not None:
                address = int(owner.start_ea)
                value.update(owner_ea=hex(address), owner_flags=int(owner.flags))
                for index, (start, end) in enumerate(idautils.Chunks(address)):
                    if index >= self.limits["owner_chunks"]:
                        self._truncate(row, "owner_chunks")
                        break
                    value["owner_chunks"].append({"start": hex(int(start)), "end": hex(int(end))})
            if first is not None:
                value["inserted_block_interval"] = [int(first), int(stop)]
            value["probe_eas"] = [hex(address) for address in self._range_probe_heads(
                row, value["owner_chunks"] if owner is not None else value["listed_ranges"])]
            source = cdg.mba.get_mblock(int(block))
            if source is not None and source.tail is not None:
                value["source_call"] = self._mapped_address(row, cdg.mba, int(source.tail.ea))
        except Exception as error:
            self._error(row, row, "inline_metadata", error)
        closing = False
        if phase == "inlining_func":
            if self.inline_context_overflow or len(self.inline_contexts) >= self.limits["inline_context_depth"]:
                self.inline_context_overflow += 1
                row.pop("inline_context", None)
                row["inline_context_status"] = "depth_limit"
                self._truncate(row, "inline_context_depth")
            else:
                context = {"opening_sequence": row["sequence"], "source_block": int(block),
                    "owner_ea": value.get("owner_ea"), "probe_eas": list(value.get("probe_eas", [])),
                    "source_call": dict(value.get("source_call", {})), "depth": len(self.inline_contexts) + 1}
                self.inline_contexts.append(context)
                row["inline_context"] = dict(context)
        elif self.inline_context_overflow:
            self.inline_context_overflow -= 1
        elif (self.inline_contexts and self.inline_contexts[-1]["source_block"] == int(block)
                and self.inline_contexts[-1]["owner_ea"] == value.get("owner_ea")):
            closing = True
        else:
            row.pop("inline_context", None)
            row["inline_context_status"] = "unmatched_completion"
        try:
            self._capture_mba(row, cdg.mba)
        except Exception as error:
            self._error(row, row, "inline_mba", error)
        finally:
            if closing:
                self.inline_contexts.pop()
        return 0

    def stkpnts(self, mba, _sps):
        row = self._event("stkpnts")
        if row is not None:
            # stkpnts_t is opaque in the installed Python API. Never cast it.
            row["stack_points_payload"] = {"status": "unobserved", "reason": "opaque_sdk_stkpnts"}
            try:
                self._capture_mba(row, mba, instructions=False)
            except Exception as error:
                self._error(row, row, "stkpnts_mba", error)
        return 0

    def prolog(self, mba, fc, reachable_blocks, decomp_flags):
        row = self._event("prolog")
        if row is not None:
            try:
                row["decompilation_flags"] = int(decomp_flags)
                # Microcode/CFG is not yet generated; observe only public scalars.
                self._capture_mba(row, mba, instructions=False)
            except Exception as error:
                self._error(row, row, "prolog_mba", error)
        return 0

    def microcode(self, mba):
        return self._observe("microcode", mba)

    def preoptimized(self, mba):
        return self._observe("preoptimized", mba)

    def locopt(self, mba):
        return self._observe("locopt", mba)

    def resolve_stkaddrs(self, mba):
        return self._observe("resolve_stkaddrs", mba)

    def calls_done(self, mba):
        return self._observe("calls_done", mba)

    def build_callinfo(self, blk, out_type):
        row = self._event("build_callinfo")
        if row is not None:
            try:
                self._capture_mba(row, blk.mba, block=blk)
            except Exception as error:
                self._error(row, row, "callinfo_mba", error)
        # The output type stays untouched and no custom callinfo is supplied.
        return None

    def callinfo_built(self, blk):
        row = self._event("callinfo_built")
        if row is not None:
            try:
                self._capture_mba(row, blk.mba, block=blk)
            except Exception as error:
                self._error(row, row, "callinfo_mba", error)
        return 0

    def inlining_func(self, cdg, blk, mbr):
        return self._inline_event("inlining_func", cdg, blk, mbr)

    def inlined_func(self, cdg, blk, mbr, i1, i2):
        return self._inline_event("inlined_func", cdg, blk, mbr, i1, i2)

    def maturity(self, cfunc, new_maturity):
        row = self._event("ctree_maturity")
        if row is not None:
            try:
                row["ctree_maturity"] = int(new_maturity)
                self._capture_mba(row, cfunc.mba)
            except Exception as error:
                self._error(row, row, "ctree_mba", error)
        return 0

    def summary(self):
        # This and lookup_failure use only frozen Python values after decompile.
        return {"target_ea": hex(self.target_ea), "limits": dict(self.limits),
                "callbacks_seen": self.callbacks_seen, "events_recorded": len(self.events),
                "instruction_records": self.instruction_records, "dropped_events": self.dropped_events,
                "call_records": self.call_records, "type_records": self.type_records,
                "argument_records": self.argument_records, "argloc_records": self.argloc_records,
                "operand_records": self.operand_records, "stack_point_records": self.stack_point_records,
                "block_records": self.block_records, "open_inline_contexts": len(self.inline_contexts),
                "inline_context_overflow": self.inline_context_overflow,
                "text_characters": self.text_characters,
                "diagnostic_error_count": self.diagnostic_error_count, "truncation": dict(self.truncation),
                "diagnostics_complete": bool(self.events) and not any(self.truncation.values())
                    and all(row["complete"] for row in self.events)}

    def lookup_failure(self, failure_ea):
        wanted = hex(int(failure_ea))
        matches = [{"sequence": row["sequence"], "phase": row["phase"],
                    "mba_entry_ea": row.get("mba_entry_ea"), "mba_maturity": row.get("mba_maturity"),
                    "instruction": dict(insn)} for row in self.events for insn in row["instructions"]
                   if insn.get("ea") == wanted]
        target_matches = [item for item in matches if item["mba_entry_ea"] == hex(self.target_ea)]
        candidates = sorted({item["instruction"]["api_mapped_ea"] for item in target_matches
                             if item["instruction"].get("mapping_status") == "api_returned"})
        statuses = {item["instruction"].get("mapping_status") for item in target_matches}
        if not target_matches:
            status = "unobserved_in_target_context"
        elif len(candidates) > 1:
            status = "conflicting"
        elif statuses == {"badaddr"}:
            status = "badaddr"
        elif statuses != {"api_returned"} or not self.summary()["diagnostics_complete"]:
            status = "incomplete"
        else:
            status = "unique_api_candidate_requires_original_pe_validation"
        return {"raw_failure_ea": wanted, "matches": matches, "mapping_status": status,
                "target_context_mapping_candidates": candidates,
                "mapping_unique_in_target_context": len(candidates) == 1,
                "original_pe_instruction_validated": False,
                "original_pe_instruction_validation": "required separately; no address guessed"}


class FreshInlineController(ida_hexrays.Hexrays_Hooks):
    """Ask the SDK to regenerate reviewed helper copies, without editing microcode."""
    def __init__(self, target_ea, base, helpers, audit):
        super().__init__()
        self.target_ea, self.base, self.helpers, self.audit = int(target_ea), int(base), helpers, audit
        self.active = False
        self.entered = False

    def begin_inlining(self, cdg, decomp_flags):
        if not self.active or int(cdg.mba.entry_ea) != self.target_ea:
            return 0
        try:
            if self.entered:
                raise ValueError("Fresh inline phase repeated for the selected parent")
            self.entered = True
            for helper in self.helpers:
                for expected in helper["callsites"]:
                    if self.base + int(expected["parent_rva"], 16) != self.target_ea:
                        continue
                    call_ea = self.base + int(expected["callsite_rva"], 16)
                    helper_ea = self.base + int(helper["rva"], 16)
                    if (ida_bytes.get_bytes(call_ea, 5) or b"").hex() != expected["raw_hex"]:
                        raise ValueError("Fresh inline original direct CALL bytes changed")
                    function = ida_funcs.get_func(helper_ea)
                    if (function is None or int(function.start_ea) != helper_ea
                            or not function.flags & ida_funcs.FUNC_OUTLINE):
                        raise ValueError("Fresh inline helper lost its exact outlined boundary")
                    chunks = []
                    for first, stop in sorted(idautils.Chunks(helper_ea)):
                        code = ida_bytes.get_bytes(first, stop - first)
                        if code is None:
                            raise ValueError("Fresh inline helper bytes are unavailable")
                        chunks.append({"start": hex(first), "end": hex(stop), "raw_hex": code.hex(),
                                       "sha256": hashlib.sha256(code).hexdigest()})
                    if chunks != helper["expected_chunks"]:
                        raise ValueError("Fresh inline helper chunks differ from the complete reviewed function")
                    mba = cdg.mba
                    matches = []
                    for index in range(int(mba.qty)):
                        block = mba.get_mblock(index)
                        tail = block.tail
                        if tail is not None and int(tail.ea) == call_ea:
                            if (int(tail.opcode) != ida_hexrays.m_call or int(tail.l.t) != ida_hexrays.mop_v
                                    or int(tail.l.g) != helper_ea):
                                raise ValueError("Fresh inline block is not the reviewed original direct CALL")
                            matches.append(int(block.serial))
                    if len(matches) != 1:
                        raise ValueError("Fresh inline direct CALL block is absent or ambiguous")
                    row = {**expected, "block_serial_before": matches[0],
                           "flag_names": list(helper["flag_names"]), "status": "requested"}
                    self.audit["calls"].append(row)
                    flags = int(ida_hexrays.INLINE_EXTFRAME) | int(ida_hexrays.INLINE_DONTCOPY)
                    row["inline_flags"] = flags
                    ranges = ida_hexrays.mba_ranges_t()
                    ranges.pfn = function
                    result = int(mba.inline_func(cdg, matches[0], ranges, int(decomp_flags), flags))
                    row["result_code"] = result
                    if result != ida_hexrays.MERR_OK:
                        row["status"] = "sdk_error"
                        self.audit["errors"].append("SDK fresh inlining returned " + str(result))
                        return result
                    row["status"] = "inlined"
            return 0
        except Exception as error:
            self.audit["errors"].append((type(error).__name__ + ": " + str(error))[:2048])
            return ida_hexrays.MERR_CANCELED


def decompile_with_fresh_inlines(ea, failure, diagnostics, helpers, base, audit):
    """Keep SDK inlining, diagnostics and both hook lifetimes separately auditable."""
    audit.update(hook_registered=False, hook_unregistered=False, complete=False, calls=[], errors=[],
                 target_execution=False, base_database_modified=False,
                 dontcopy_flag=int(ida_hexrays.INLINE_DONTCOPY),
                 external_frame_flag=int(ida_hexrays.INLINE_EXTFRAME))
    controller = FreshInlineController(ea, base, helpers, audit)
    try:
        audit["hook_registered"] = bool(controller.hook())
        controller.active = audit["hook_registered"]
        if not controller.active:
            raise ValueError("Hex-Rays rejected fresh inline hooks")
        result = decompile_with_diagnostics(ea, failure, diagnostics)
    finally:
        # Leave an unsuccessfully removed hook inert for all later exports.
        controller.active = False
        try:
            audit["hook_unregistered"] = bool(controller.unhook())
        except Exception as error:
            audit["errors"].append((type(error).__name__ + ": " + str(error))[:2048])
    expected = [call for helper in helpers for call in helper["callsites"]
                if base + int(call["parent_rva"], 16) == ea]
    audit["complete"] = bool(audit["hook_registered"] and audit["hook_unregistered"] and controller.entered
                             and not audit["errors"] and len(audit["calls"]) == len(expected)
                             and all(row["status"] == "inlined" for row in audit["calls"]))
    if not audit["complete"]:
        raise ValueError("Fresh inline SDK operation or hook cleanup is incomplete")
    return result


def decompile_with_diagnostics(ea, failure, diagnostics):
    """Observe one existing static decompile without changing its result/errors."""
    trace = None
    outcome = "raised"
    diagnostics["hook_registered"] = False
    try:
        trace = StaticMicrocodeTrace(ea)
        diagnostics["hook_registered"] = bool(trace.hook())
        trace.active = diagnostics["hook_registered"]
        if not trace.active:
            _static_diagnostic_error(diagnostics, "hook_registration", ValueError("Hex-Rays rejected diagnostic hooks"))
    except Exception as error:
        _static_diagnostic_error(diagnostics, "hook_registration", error)
    try:
        result = ida_hexrays.decompile(ea, failure)
        outcome = "returned_none" if result is None else "returned_cfunc"
        return result
    finally:
        if trace is not None:
            # A failed unhook must leave this observer inert for later exports.
            trace.active = False
            try:
                diagnostics["hook_unregistered"] = bool(trace.unhook())
                if not diagnostics["hook_unregistered"]:
                    _static_diagnostic_error(diagnostics, "hook_cleanup", ValueError("Hex-Rays rejected hook removal"))
            except Exception as error:
                _static_diagnostic_error(diagnostics, "hook_cleanup", error)
        try:
            diagnostics["decompile_outcome"] = outcome
            if trace is not None:
                diagnostics.update(trace.summary())
                diagnostics["diagnostics_complete"] = bool(diagnostics["diagnostics_complete"]
                    and diagnostics["hook_registered"] and diagnostics.get("hook_unregistered")
                    and not diagnostics.get("diagnostic_errors"))
                if outcome != "returned_cfunc" or diagnostics["hook_registered"]:
                    diagnostics["events"] = trace.events
                if outcome != "returned_cfunc":
                    diagnostics["failure_mapping"] = trace.lookup_failure(int(failure.errea))
                    mapping = diagnostics["failure_mapping"]
                    if (not diagnostics["diagnostics_complete"]
                            and mapping["mapping_status"] == "unique_api_candidate_requires_original_pe_validation"):
                        mapping["mapping_status"] = "incomplete"
        except Exception as error:
            _static_diagnostic_error(diagnostics, "frozen_observations", error)


def apply_shared_function_tail(shared, base, file_bytes):
    """Share an evidenced suffix without replacing either retained entry/frame."""
    def frame(function):
        return {"frame_id": int(function.frame), "local_bytes": int(function.frsize),
                "saved_register_bytes": int(function.frregs), "argument_bytes": int(function.argsize),
                "frame_pointer_delta": int(function.fpd)}

    def chunks(ea):
        values = []
        for start, stop in sorted(idautils.Chunks(ea)):
            code = ida_bytes.get_bytes(start, stop - start)
            if code != file_bytes(start - base, stop - start):
                raise ValueError("Shared-tail database bytes differ from the original PE")
            values.append({"start": hex(start), "end": hex(stop), "raw_hex": code.hex(),
                           "sha256": hashlib.sha256(code).hexdigest()})
        return values

    primary = base + int(shared["primary_owner_rva"], 16)
    start, stop = int(shared["expected_tail"]["start"], 16), int(shared["expected_tail"]["end"], 16)
    original_frames, original_types, original_tinfos, original_points = {}, {}, {}, {}
    for owner in shared["owners"]:
        ea = base + int(owner["rva"], 16)
        function = ida_funcs.get_func(ea)
        if (function is None or function.start_ea != ea or hex(function.end_ea) != owner["original_end"]
                or function.flags != owner["expected_flags"] or chunks(ea) != owner["original_chunks"]):
            raise ValueError("Private shared-tail owner differs from its original boundary, bytes or flags")
        original_frames[owner["rva"]] = frame(function)
        original_types[owner["rva"]] = idc.get_type(ea)
        typ = ida_typeinf.tinfo_t()
        original_tinfos[owner["rva"]] = (bool(ida_nalt.get_tinfo(typ, ea)), typ)
        original_points[owner["rva"]] = [{"address": hex(point.ea), "delta": int(point.spd)} for point in function.points]

    # Shrinking the main chunk retains the compiler's existing frame and type.
    # The same bytes are immediately reattached as a tail before autoanalysis.
    if not ida_funcs.set_func_end(primary, start):
        raise ValueError("IDA rejected the exact reviewed shared-tail head boundary")
    for owner in shared["owners"]:
        ea = base + int(owner["rva"], 16)
        if not ida_funcs.append_func_tail(ida_funcs.get_func(ea), start, stop):
            raise ValueError("IDA rejected a complete reviewed shared-tail attachment")
    tail = ida_funcs.get_fchunk(start)
    if tail is None or not ida_funcs.set_tail_owner(tail, primary):
        raise ValueError("IDA rejected the reviewed shared-tail primary owner")
    ida_auto.auto_wait()
    tail = ida_funcs.get_fchunk(start)
    expected_owners = {base + int(value, 16) for value in shared["owner_rvas"]}
    if (tail is None or tail.start_ea != start or tail.end_ea != stop
            or not tail.flags & ida_funcs.FUNC_TAIL or tail.owner != primary
            or tail.refqty != len(expected_owners)
            or set(ida_funcs.func_parent_iterator_t(tail)) != expected_owners):
        raise ValueError("Applied shared-tail parent set, primary owner or range differs from review")
    tail_code = ida_bytes.get_bytes(start, stop - start)
    actual_tail = {"start": hex(start), "end": hex(stop), "raw_hex": tail_code.hex(),
                   "sha256": hashlib.sha256(tail_code).hexdigest()}
    if actual_tail != shared["expected_tail"]:
        raise ValueError("Applied shared-tail bytes differ from the complete original suffix")
    applied = []
    for owner in shared["owners"]:
        ea = base + int(owner["rva"], 16)
        function = ida_funcs.get_func(ea)
        if (function is None or function.start_ea != ea or hex(function.end_ea) != owner["expected_end"]
                or chunks(ea) != owner["expected_chunks"]):
            raise ValueError("Applied shared-tail owner omitted its original entry or complete chunks")
        typ = ida_typeinf.tinfo_t()
        present = bool(ida_nalt.get_tinfo(typ, ea))
        prior_present, prior_type = original_tinfos[owner["rva"]]
        if (frame(function) != original_frames[owner["rva"]] or idc.get_type(ea) != original_types[owner["rva"]]
                or present != prior_present or present and typ != prior_type):
            raise ValueError("Shared-tail attachment changed the retained function frame or type")
        current_points = [{"address": hex(point.ea), "delta": int(point.spd)} for point in function.points]
        old_points = original_points[owner["rva"]]
        if (any(point not in current_points for point in old_points)
                or any(point not in old_points and not start <= int(point["address"], 16) <= stop for point in current_points)):
            raise ValueError("Shared-tail attachment changed an original stack delta outside its reviewed suffix")
        before, flag = owner["expected_flags"], int(ida_funcs.FUNC_OUTLINE)
        flags = before | flag if shared["inline_owners"] else before
        function.flags = flags
        if not ida_funcs.update_func(function) or ida_funcs.get_func(ea).flags != flags:
            raise ValueError("IDA rejected the reviewed shared-tail owner flags")
        function = ida_funcs.get_func(ea)
        applied.append({"rva": owner["rva"], "end": hex(function.end_ea), "chunks": chunks(ea),
                        "previous_flags": before, "applied_flags": flags, "outline_flag": flag,
                        "previous_frame": original_frames[owner["rva"]], "applied_frame": frame(function),
                        "previous_type": original_types[owner["rva"]], "applied_type": idc.get_type(ea),
                        "previous_stack_points": old_points, "applied_stack_points": current_points,
                        "base_database_modified": False})
    tail = ida_funcs.get_fchunk(start)
    return {**shared, "applied_primary_owner_rva": hex(tail.owner - base),
            "applied_owner_rvas": list(shared["owner_rvas"]), "applied_tail": actual_tail,
            "applied_owners": applied, "base_database_modified": False}


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
        notes = (recoveries or selection.get("import_type_recoveries", [])
                 or selection.get("function_merges", []) or selection.get("function_outlines", [])
                 or selection.get("function_partitions", []) or selection.get("function_shared_tails", []))
        partition_options = {"partition_specs": [row["owner_rva"] + "=" + ",".join(
            helper["rva"] + ":" + hex(int(helper["expected_end"], 16) - base)
            for helper in row["helpers"]) for row in selection["function_partitions"]
        ]} if selection.get("function_partitions") else {}
        if selection.get("function_shared_tails"):
            partition_options["shared_tail_specs"] = selection["function_shared_tails"]
        if selection.get("function_fresh_inlines"):
            partition_options["fresh_inline_rvas"] = [row["rva"] for row in selection["function_fresh_inlines"]]
        if "static_observation" in selection:
            review_source = selection["static_observation"]["review_source"]
            partition_options["observation_evidence"] = review_source["path"] + "#" + review_source["pointer"]
        if selection.get("callsite_type_recoveries"):
            partition_options["callsite_reviews"] = [row["review_source"]["path"] + "#"
                + row["review_source"]["pointer"] for row in selection["callsite_type_recoveries"]]
        if selection.get("type_owner_reanalysis"):
            partition_options["reanalyze_type_owners"] = True
        if "direct_call_type_propagations" in selection:
            partition_options["propagate_direct_calls"] = True
        actual = analysis.native_function_selection(raw, json.loads(base_capture.read_bytes()),
            [int(row["rva"], 16) for row in selection["targets"]],
            [row["rva"] + "=" + row["declaration"] for row in recoveries],
            notes[0]["assembly_evidence"] if notes else None,
            selection.get("decompiler_limits", {}).get("max_function_kib"),
            [row["rva"] + "=" + row["declaration"] for row in selection.get("import_type_recoveries", [])],
            [row["owner_rva"] + "=" + ",".join(row["member_rvas"]) for row in selection.get("function_merges", [])],
            [row["rva"] for row in selection.get("function_outlines", [])], **partition_options)
    if actual != selection:
        raise ValueError("Native selection no longer matches its bounded original-byte plan")
    capture_tool = {"path": "tools/ida/supplement.py",
                    "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    observation = selection.get("static_observation")
    if observation and capture_tool["sha256"] != observation["observer_source"]["sha256"]:
        raise ValueError("Supplement did not use its reviewed static observer")
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
                "capture_tool": capture_tool,
                "started_at_utc": datetime.now(timezone.utc).isoformat(),
                "functions": [], "references": [], "chains": [], "failures": [], "type_recoveries": [],
                "import_type_recoveries": [],
                "import_thunk_type_recoveries": [],
                "function_merge_recoveries": [],
                "function_outline_recoveries": [],
                "function_partition_recoveries": [], "partition_helpers": [],
                "function_shared_tail_recoveries": [], "shared_tail_helpers": [],
                "decoded_thunks": [],
                "capture_complete": False, "all_pseudocode_captured": False,
                "boundary_policy": "Only exact direct-jump, x64 embedded-object destructor forwarding or x64 bound-member tail-call thunks receive new function boundaries; existing functions keep IDA boundaries",
                "target_execution": False, "real_device_operations": False,
                "semantic_audit": "pending", "implementation_acceptance": "pending"}
    manifest["typed_value_serializer"] = capture_values.source_identity(root)
    if selection.get("callsite_type_recoveries") or selection.get("type_owner_reanalysis"):
        import native_callsites
        manifest["callsite_type_validator"] = native_callsites.source_identity(root)
        manifest["callsite_type_recoveries"] = []

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

    limits = selection.get("decompiler_limits")
    if limits is not None:
        if not manifest["hexrays_available"]:
            raise ValueError("Decompiler limits require an available Hex-Rays plugin")
        directive = "MAX_FUNCSIZE = {}".format(limits["max_function_kib"])
        if not ida_hexrays.change_hexrays_config(directive):
            raise ValueError("Hex-Rays rejected the explicit function size limit")
        manifest["applied_decompiler_limits"] = limits
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

    captured = {}

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

    def member_dispatch_thunk(ea):
        chunk = file_bytes(ea - base, 11)
        length = analysis.native_member_dispatch_thunk_size(chunk, selection["architecture"])
        if length is not None and ida_bytes.get_bytes(ea, length) != chunk:
            raise ValueError("Database bound-member thunk bytes differ from original input")
        return length

    shared_owner_rvas = {owner["rva"] for row in selection.get("function_shared_tails", []) for owner in row["owners"]}

    def export(ea, assembly_only=False, helper_kind="partition_helpers"):
        rva = hex(ea - base)
        destination = output / helper_kind.replace("_", "-") if assembly_only else functions
        destination.mkdir(exist_ok=True)
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
                captured[rva] = (path, False)
            return body
        if rva in captured:
            path, was_assembly_only = captured[rva]
            if was_assembly_only and not assembly_only:
                raise ValueError("An assembly-only helper cannot satisfy a requested pseudocode output: " + rva)
            return json.loads(path.read_bytes())
        func = ida_funcs.get_func(ea)
        created = False
        boundary_source = "retained IDA function boundary"
        if func is None:
            branch = jump_target(ea)
            length = branch[1] if branch else forwarding_thunk(ea)
            member_length = member_dispatch_thunk(ea) if length is None else None
            if member_length is not None:
                length = member_length
            if length is None:
                raise ValueError("No existing function boundary and target is not an evidenced supported thunk: " + rva)
            boundary_source = ("byte-exact direct-jump thunk" if branch
                               else "byte-exact x64 bound-member tail-call thunk" if member_length
                               else "byte-exact x64 embedded-object destructor forwarding thunk")
            if member_length:
                decoded_rows = []
                position = ea
                while position < ea + length:
                    insn = ida_ua.insn_t()
                    count = ida_ua.decode_insn(insn, position)
                    if count <= 0 or position + count > ea + length:
                        raise ValueError("IDA could not independently decode the exact member thunk")
                    operands = []
                    for operand in insn.ops:
                        if operand.type == ida_ua.o_void:
                            break
                        operands.append({"type": int(operand.type), "dtype": int(operand.dtype),
                            "reg": int(operand.reg), "phrase": int(operand.phrase),
                            "address": int(operand.addr), "value": int(operand.value)})
                    decoded_rows.append({"address": hex(position), "rva": hex(position - base),
                        "bytes": file_bytes(position - base, count).hex(),
                        "mnemonic": insn.get_canon_mnem(), "operands": operands})
                    position += count
                manifest["decoded_thunks"].append({"rva": rva, "start": hex(ea), "end": hex(ea + length),
                    "raw_hex": file_bytes(ea - base, length).hex(), "assembly": decoded_rows,
                    "boundary_source": "actual PE pointer-table target and exact supported instruction sequence",
                    "database_function_created": False, "pseudocode_captured": False})
            cursor = ea
            while cursor < ea + length:
                instruction = ida_ua.insn_t()
                decoded = ida_ua.decode_insn(instruction, cursor)
                if decoded <= 0 or cursor + decoded > ea + length:
                    raise ValueError("IDA could not decode the exact thunk boundary: " + rva)
                flags = ida_bytes.get_flags(cursor)
                if ida_bytes.is_code(flags):
                    if ida_bytes.get_item_head(cursor) != cursor or ida_bytes.get_item_size(cursor) != decoded:
                        raise ValueError("Existing instruction differs from exact thunk decoding: " + hex(cursor - base))
                else:
                    created_length = ida_ua.create_insn(cursor)
                    if created_length != decoded:
                        sane = int(ida_idp._processor_t.is_sane_insn(instruction, 0))
                        manifest["failures"].append({"rva": hex(cursor - base),
                            "stage": "instruction_creation", "decoded_bytes": decoded,
                            "created_bytes": created_length, "processor_sanity": sane,
                            "decoded_mnemonic": instruction.get_canon_mnem(),
                            "original_raw_hex": file_bytes(cursor - base, decoded).hex()})
                        raise ValueError("IDA could not create the exact thunk instruction: " + hex(cursor - base)
                                         + " item=" + hex(ida_bytes.get_item_head(cursor) - base)
                                         + ".." + hex(ida_bytes.get_item_end(cursor) - base))
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
               "assembly": [], "basic_blocks": [], "pseudocode": None,
               "capture_stages": {"type": "before_decompile", "assembly": "before_decompile"}}
        chunk_ranges = idautils.Chunks(ea)
        if rva in shared_owner_rvas:
            chunk_ranges = sorted(chunk_ranges)
        for first, end in chunk_ranges:
            chunk = ida_bytes.get_bytes(first, end - first)
            if chunk != file_bytes(first - base, end - first):
                raise ValueError("Supplement function bytes differ from the original input")
            row["chunks"].append({"start": hex(first), "end": hex(end), "raw_hex": chunk.hex(),
                                  "sha256": hashlib.sha256(chunk).hexdigest()})
        items = idautils.FuncItems(ea)
        if rva in shared_owner_rvas:
            items = sorted(items)
        def assembly(addresses):
            result = []
            for item in addresses:
                length = ida_bytes.get_item_size(item)
                result.append({"address": hex(item), "rva": hex(item - base),
                    "bytes": (ida_bytes.get_bytes(item, length) or b"").hex(),
                    "text": ida_lines.tag_remove(idc.generate_disasm_line(item, 0) or ""),
                    "xrefs_from": [ref(x) for x in idautils.XrefsFrom(item, 0)],
                    "xrefs_to": [ref(x) for x in idautils.XrefsTo(item, 0)],
                    "comment": idc.get_cmt(item, False), "repeatable_comment": idc.get_cmt(item, True)})
            return result
        row["assembly"] = assembly(items)
        row["basic_blocks"] = [{"id": block.id, "start": hex(block.start_ea), "end": hex(block.end_ea),
                                "predecessors": [x.id for x in block.preds()],
                                "successors": [x.id for x in block.succs()]} for block in ida_gdl.FlowChart(func)]
        if not assembly_only:
            failure = None
            try:
                failure = ida_hexrays.hexrays_failure_t()
                row["type_before_decompile"] = idc.get_type(ea)
                row["hexrays_diagnostics"] = {}
                fresh = selection.get("function_fresh_inlines", [])
                if any(call["parent_rva"] == rva for helper in fresh for call in helper["callsites"]):
                    row["fresh_inline_audit"] = {}
                    cfunc = decompile_with_fresh_inlines(ea, failure, row["hexrays_diagnostics"],
                                                        fresh, base, row["fresh_inline_audit"])
                else:
                    cfunc = decompile_with_diagnostics(ea, failure, row["hexrays_diagnostics"])
                if cfunc is None:
                    raise ValueError("Hex-Rays: " + failure.desc())
                row["pseudocode"] = {"entry": hex(cfunc.entry_ea), "lines": [
                    {"line": index, "text": ida_lines.tag_remove(line.line)}
                    for index, line in enumerate(cfunc.get_pseudocode())]}
                row["pseudocode"].update(capture_values.local_variables(cfunc, ida_hexrays))
                row["hexrays_diagnostics"]["local_namespaces"] = local_namespace_diagnostics(cfunc)
            except Exception:
                error = {"rva": rva, "stage": "hexrays", "error": traceback.format_exc()}
                if failure is not None:
                    address = int(failure.errea)
                    error["hexrays_failure"] = {
                        "code": int(failure.code), "address": hex(address),
                        "rva": hex(address - base) if address != idaapi.BADADDR and address >= base else None,
                        "description": failure.desc(), "detail": str(failure.str)}
                manifest["failures"].append(error)
            row["type_after_decompile"] = idc.get_type(ea)
            after_items = idautils.FuncItems(ea)
            if rva in shared_owner_rvas:
                after_items = sorted(after_items)
            row["assembly_after_decompile"] = assembly(after_items)
            row["capture_stages"].update(type_before_decompile="before_decompile",
                type_after_decompile="after_pseudocode_and_typed_locals",
                assembly_after_decompile="after_pseudocode_and_typed_locals")
        for decoded_thunk in manifest["decoded_thunks"]:
            if decoded_thunk["rva"] == rva:
                decoded_thunk["database_function_created"] = True
                decoded_thunk["pseudocode_captured"] = row["pseudocode"] is not None
        path = destination / (rva[2:] + ".json")
        save(path, row)
        manifest[helper_kind if assembly_only else "functions"].append({"address": hex(ea), "rva": rva, "name": row["name"],
            "path": path.relative_to(output).as_posix(), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "assembly_captured": True, "pseudocode_captured": row["pseudocode"] is not None})
        captured[rva] = (path, assembly_only)
        ida_hexrays.clear_cached_cfuncs()
        return row

    for shared in selection.get("function_shared_tails", []):
        for owner in shared["owners"]:
            source = owner["source_function"]
            data = (root / source["path"]).read_bytes()
            if len(data) != source["bytes"] or hashlib.sha256(data).hexdigest() != source["sha256"]:
                raise ValueError("Reviewed shared-tail function evidence changed")
        manifest["function_shared_tail_recoveries"].append(apply_shared_function_tail(shared, base, file_bytes))
        save(output / "capture.json", manifest)
    ida_hexrays.clear_cached_cfuncs()

    for partition in selection.get("function_partitions", []):
        retained = root / partition["source_function"]["path"]
        source_bytes = retained.read_bytes()
        if (len(source_bytes) != partition["source_function"]["bytes"]
                or hashlib.sha256(source_bytes).hexdigest() != partition["source_function"]["sha256"]):
            raise ValueError("Reviewed function partition evidence changed")
        owner_ea = base + int(partition["owner_rva"], 16)
        function = ida_funcs.get_func(owner_ea)
        if (function is None or function.start_ea != owner_ea
                or function.flags != partition["expected_flags"]):
            raise ValueError("Private partition owner differs from its retained boundary or flags")

        def partition_chunks(ea):
            values = []
            for start, stop in idautils.Chunks(ea):
                code = ida_bytes.get_bytes(start, stop - start)
                if code != file_bytes(start - base, stop - start):
                    raise ValueError("Partition database bytes differ from the original PE")
                values.append({"start": hex(start), "end": hex(stop), "raw_hex": code.hex(),
                               "sha256": hashlib.sha256(code).hexdigest()})
            return values

        if partition_chunks(owner_ea) != partition["original_chunks"]:
            raise ValueError("Private partition owner chunks differ from the complete retained function")
        if not ida_funcs.del_func(owner_ea):
            raise ValueError("IDA could not remove the explicitly reviewed private partition owner")
        main, *tails = partition["owner_chunks"]
        if not ida_funcs.add_func(int(main["start"], 16), int(main["end"], 16)):
            raise ValueError("IDA could not create the exact partition owner entry chunk")
        for chunk in tails:
            function = ida_funcs.get_func(owner_ea)
            if not ida_funcs.append_func_tail(function, int(chunk["start"], 16), int(chunk["end"], 16)):
                raise ValueError("IDA could not retain a complete partition owner tail chunk")
        for helper in partition["helpers"]:
            start, stop = base + int(helper["rva"], 16), int(helper["expected_end"], 16)
            if not ida_funcs.add_func(start, stop):
                raise ValueError("IDA could not create an exact reviewed embedded helper boundary")
        ida_auto.auto_wait()
        if partition_chunks(owner_ea) != partition["owner_chunks"]:
            raise ValueError("Applied partition owner chunks differ from the exact original-byte residual")
        applied_helpers = []
        for helper in partition["helpers"]:
            ea = base + int(helper["rva"], 16)
            function = ida_funcs.get_func(ea)
            if (function is None or function.start_ea != ea or hex(function.end_ea) != helper["expected_end"]
                    or partition_chunks(ea) != helper["expected_chunks"]):
                raise ValueError("Created embedded helper differs from its exact reviewed boundary")
            before, flag = function.flags, int(ida_funcs.FUNC_OUTLINE)
            function.flags = before | flag
            if not ida_funcs.update_func(function) or ida_funcs.get_func(ea).flags != before | flag:
                raise ValueError("IDA rejected the embedded helper's explicit outline flag")
            applied_helpers.append({"rva": helper["rva"], "chunks": helper["expected_chunks"],
                "previous_flags": before, "applied_flags": before | flag, "outline_flag": flag,
                "base_database_modified": False})
        manifest["function_partition_recoveries"].append({**partition,
            "applied_owner_chunks": partition_chunks(owner_ea), "applied_helpers": applied_helpers,
            "base_database_modified": False})
        ida_hexrays.clear_cached_cfuncs()
        for helper in partition["helpers"]:
            export(base + int(helper["rva"], 16), assembly_only=True)
        save(output / "capture.json", manifest)

    for outline in selection.get("function_outlines", []):
        retained = root / outline["source_function"]["path"]
        value = retained.read_bytes()
        if (len(value) != outline["source_function"]["bytes"]
                or hashlib.sha256(value).hexdigest() != outline["source_function"]["sha256"]):
            raise ValueError("Reviewed outlined helper evidence changed")
        ea = base + int(outline["rva"], 16)
        function = ida_funcs.get_func(ea)
        if (function is None or function.start_ea != ea or hex(function.end_ea) != outline["expected_end"]
                or function.flags != outline["expected_flags"]):
            raise ValueError("Private outlined helper boundary or flags differ from its retained evidence")
        chunks = []
        for start, stop in idautils.Chunks(ea):
            code = ida_bytes.get_bytes(start, stop - start)
            if code != file_bytes(start - base, stop - start):
                raise ValueError("Outlined helper database bytes differ from the original PE")
            chunks.append({"start": hex(start), "end": hex(stop), "raw_hex": code.hex(),
                           "sha256": hashlib.sha256(code).hexdigest()})
        if chunks != outline["expected_chunks"]:
            raise ValueError("Outlined helper database chunks differ from complete original evidence")
        before, flag = function.flags, int(ida_funcs.FUNC_OUTLINE)
        function.flags = before | flag
        if not ida_funcs.update_func(function) or ida_funcs.get_func(ea).flags != before | flag:
            raise ValueError("IDA rejected the explicit outlined helper flag")
        manifest["function_outline_recoveries"].append({**outline, "previous_flags": before,
            "applied_flags": before | flag, "outline_flag": flag, "applied_chunks": chunks,
            "base_database_modified": False})
    ida_hexrays.clear_cached_cfuncs()

    for merge in selection.get("function_merges", []):
        for member in merge["members"]:
            retained = root / member["path"]
            data = retained.read_bytes()
            if len(data) != member["bytes"] or hashlib.sha256(data).hexdigest() != member["sha256"]:
                raise ValueError("Original function merge evidence changed")
            body = json.loads(data)
            ea = base + int(member["rva"], 16)
            function = ida_funcs.get_func(ea)
            if (function is None or function.start_ea != ea
                    or list(idautils.Chunks(ea)) != [(int(chunk["start"], 16), int(chunk["end"], 16)) for chunk in body["chunks"]]):
                raise ValueError("Private database function merge members differ from retained boundaries")
        for member in reversed(merge["members"]):
            if not ida_funcs.del_func(base + int(member["rva"], 16)):
                raise ValueError("IDA could not remove an explicitly reviewed private function fragment")
        first, end = int(merge["expected_start"], 16), int(merge["expected_end"], 16)
        if not ida_funcs.add_func(first, end):
            raise ValueError("IDA could not define the exact reviewed combined function")
        applied = []
        for start, stop in idautils.Chunks(first):
            chunk = ida_bytes.get_bytes(start, stop - start)
            if chunk != file_bytes(start - base, stop - start):
                raise ValueError("Combined function bytes differ from the original PE")
            applied.append({"start": hex(start), "end": hex(stop), "raw_hex": chunk.hex(),
                            "sha256": hashlib.sha256(chunk).hexdigest()})
        if applied != merge["expected_chunks"]:
            raise ValueError("IDA combined boundary does not exactly cover the reviewed original fragments")
        manifest["function_merge_recoveries"].append({**merge, "base_database_modified": False,
                                                     "applied_chunks": applied})
    ida_hexrays.clear_cached_cfuncs()

    imported_function_types = {}
    for recovery in selection.get("import_type_recoveries", []):
        ea = base + int(recovery["rva"], 16)
        bindings = []
        for module_index in range(ida_nalt.get_import_module_qty()):
            module = ida_nalt.get_import_module_name(module_index)

            def imported(address, name, ordinal):
                if address == ea:
                    bindings.append({"rva": hex(address - base), "name": name, "ordinal": int(ordinal),
                        "module": module, "loaded_slot_raw_hex":
                            (ida_bytes.get_bytes(address, recovery["bytes"]) or b"").hex()})
                return True

            if ida_nalt.enum_import_names(module_index, imported) == -1:
                raise ValueError("IDA import enumeration failed")
        if len(bindings) != 1 or not analysis.native_import_binding_matches(recovery, bindings[0]):
            raise ValueError("Reviewed PE import does not match IDA's unique named import registration")
        previous = idc.get_type(ea)
        typ = ida_typeinf.tinfo_t()
        if (not ida_typeinf.parse_decl(typ, None, recovery["declaration"], ida_typeinf.PT_SIL)
                or not typ.is_funcptr() or typ.get_size() != recovery["bytes"]):
            raise ValueError("Reviewed import declaration must be a function pointer")
        if not ida_typeinf.apply_tinfo(ea, typ, ida_typeinf.TINFO_DEFINITE):
            raise ValueError("IDA rejected the reviewed import pointer type")
        applied = ida_typeinf.tinfo_t()
        if not ida_nalt.get_tinfo(applied, ea) or applied != typ:
            raise ValueError("Applied import type differs from reviewed declaration")
        manifest["import_type_recoveries"].append({**recovery, "address": hex(ea),
            "previous_type": previous, "applied_type": str(applied), "base_database_modified": False,
            "database_import_binding": bindings[0]})
        imported_function_types[recovery["rva"]] = typ.get_pointed_object()

    for thunk in selection.get("import_thunks", []):
        ea = base + int(thunk["rva"], 16)
        function = ida_funcs.get_func(ea)
        if (function is None or function.start_ea != ea
                or (ida_bytes.get_bytes(ea, thunk["bytes"]) or b"").hex() != thunk["raw_hex"]):
            raise ValueError("Exact reviewed import thunk differs from the private database")
        retained = root / thunk["source_function"]["path"]
        if hashlib.sha256(retained.read_bytes()).hexdigest() != thunk["source_function"]["sha256"]:
            raise ValueError("Exact import thunk assembly evidence changed")
        typ = imported_function_types[thunk["slot_rva"]]
        previous = idc.get_type(ea)
        if not typ.is_func() or not ida_typeinf.apply_tinfo(ea, typ, ida_typeinf.TINFO_DEFINITE):
            raise ValueError("IDA rejected the exact import thunk's verified pointed function type")
        applied = ida_typeinf.tinfo_t()
        if not ida_nalt.get_tinfo(applied, ea) or applied != typ:
            raise ValueError("Applied import thunk type differs from the verified import function type")
        manifest["import_thunk_type_recoveries"].append({**thunk, "address": hex(ea),
            "previous_type": previous, "applied_type": str(applied), "base_database_modified": False})
    ida_hexrays.clear_cached_cfuncs()

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

    if selection.get("callsite_type_recoveries"):
        sdk = SimpleNamespace(bytes=ida_bytes, types=ida_typeinf, nalt=ida_nalt)
        for recovery in selection["callsite_type_recoveries"]:
            manifest["callsite_type_recoveries"].append(native_callsites.apply(recovery, base, sdk))
        native_callsites.verify_applied(selection["callsite_type_recoveries"], manifest["callsite_type_recoveries"])
        sdk.root, sdk.funcs, sdk.utils = root, ida_funcs, idautils
        sdk.auto, sdk.frame = ida_auto, ida_frame
        manifest["callsite_automatic_reanalysis"] = native_callsites.reanalyze_owners(
            selection["callsite_type_recoveries"], base, sdk)
        ida_hexrays.clear_cached_cfuncs()

    if selection.get("type_owner_reanalysis"):
        sdk = SimpleNamespace(root=root, funcs=ida_funcs, utils=idautils, bytes=ida_bytes,
                              auto=ida_auto, frame=ida_frame, types=ida_typeinf, nalt=ida_nalt)
        direct = selection.get("direct_call_type_propagations", [])
        if direct:
            manifest["direct_call_type_propagations"] = [native_callsites.apply_direct(row, base, sdk) for row in direct]
            native_callsites.verify_direct_applied(direct, manifest["direct_call_type_propagations"])
        manifest["type_owner_automatic_reanalysis"] = native_callsites.reanalyze_owners(
            selection["type_owner_reanalysis"], base, sdk)
        ida_hexrays.clear_cached_cfuncs()

    selected_rvas = {row["rva"] for row in selection["targets"]}
    for rva in sorted(shared_owner_rvas - selected_rvas, key=lambda value: int(value, 16)):
        export(base + int(rva, 16), assembly_only=True, helper_kind="shared_tail_helpers")

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
                # Retained selections already own complete function chunks;
                # an entry JMP can target another block of that same function.
                branch = jump_target(ea) if table else None
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
    if capture_values.source_identity(root) != manifest["typed_value_serializer"]:
        raise ValueError("Typed capture serializer identity changed during supplemental analysis")
    if selection.get("callsite_type_recoveries") or selection.get("type_owner_reanalysis"):
        if native_callsites.source_identity(root) != manifest["callsite_type_validator"]:
            raise ValueError("Callsite type validator changed during supplemental analysis")
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

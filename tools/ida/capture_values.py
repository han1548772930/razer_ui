"""Copy typed SDK values while their owning decompiler objects are alive."""
import hashlib
from pathlib import Path


_SOURCE = Path(__file__).read_bytes()
LOCATION_LIMITS = {"depth": 8, "parts": 256}


def source_identity(root):
    path = Path(__file__).resolve()
    if path.read_bytes() != _SOURCE:
        raise ValueError("Typed capture serializer changed after it was imported")
    return {"path": path.relative_to(root).as_posix(), "bytes": len(_SOURCE),
            "sha256": hashlib.sha256(_SOURCE).hexdigest()}


def _error(record, stage, error):
    record["complete"] = False
    record.setdefault("errors", []).append({"stage": stage,
        "error": (type(error).__name__ + ": " + str(error))[:2048]})


def argument_location(location, namespace, depth=0):
    """Read the tagged payload, preserving unknown and bounded-out observations."""
    result = {"status": "unobserved", "complete": True,
              "register_namespace": namespace}
    try:
        result["atype"] = int(location.atype())
        if location.is_badloc():
            result.update(kind="none", reason="sdk_has_no_location", complete=False)
        elif location.is_reg1():
            result.update(status="observed", kind="register", register=int(location.reg1()),
                          register_offset=int(location.regoff()))
        elif location.is_reg2():
            result.update(status="observed", kind="register_pair", register=int(location.reg1()),
                          second_register=int(location.reg2()))
        elif location.is_stkoff():
            result.update(status="observed", kind="stack", stack_offset=int(location.stkoff()))
        elif location.is_ea():
            result.update(status="observed", kind="static_address", address=hex(int(location.get_ea())))
        elif location.is_rrel():
            relative = location.get_rrel()
            result.update(status="observed", kind="register_relative", register=int(relative.reg),
                          offset=int(relative.off))
        elif location.is_scattered():
            result.update(status="observed", kind="scattered", parts=[])
            parts = location.scattered()
            count = int(parts.size())
            result["part_count"] = count
            if depth >= LOCATION_LIMITS["depth"]:
                result.update(status="truncated", complete=False, reason="location_depth_limit")
            else:
                for index in range(min(count, LOCATION_LIMITS["parts"])):
                    part = parts.at(index)
                    value = argument_location(part, namespace, depth + 1)
                    value.update(argument_offset=int(part.off), size=int(part.size))
                    result["parts"].append(value)
                if count > LOCATION_LIMITS["parts"]:
                    result.update(status="truncated", complete=False, reason="location_part_limit")
                if not all(value["complete"] for value in result["parts"]):
                    result["complete"] = False
        elif location.is_custom():
            result.update(kind="custom", reason="custom_payload_not_observed", complete=False)
        else:
            result.update(kind="unknown", reason="unknown_location_tag", complete=False)
    except Exception as error:
        result["status"] = "diagnostic_error"
        _error(result, "location_payload", error)
    return result


def kernel_register_proof(observed, width, hexrays):
    """Prove SDK virtual register classes without inventing CPU coordinates."""
    if observed.get("kind") != "register":
        return None
    result = {"status": "unobserved", "complete": False,
              "register_namespace": "hexrays_microregisters",
              "register": observed["register"],
              "register_offset": observed["register_offset"], "width": width,
              "kernel_api": "ida_hexrays.is_kreg",
              "processor_api": "ida_hexrays.mreg2reg",
              "name_api": "ida_hexrays.get_mreg_name"}
    try:
        result["is_kernel_register"] = bool(hexrays.is_kreg(observed["register"]))
        if not result["is_kernel_register"]:
            if not hasattr(hexrays, "get_temp_regs"):
                return None
            result.update(temporary_api="ida_hexrays.get_temp_regs", membership_api="mlist_t.has_all",
                          is_codegen_temporary=bool(hexrays.get_temp_regs().has_all(observed["register"], width)))
            if not result["is_codegen_temporary"]:
                if not hasattr(hexrays, "mop_t"):
                    return None
                operand = hexrays.mop_t()
                operand.make_reg(observed["register"], width)
                result.update(operand_api="ida_hexrays.mop_t.make_reg",
                              operand_register=int(operand.r), operand_width=int(operand.size),
                              operand_is_exact_register=bool(operand.is_reg(observed["register"], width)),
                              bit_api="ida_hexrays.mop_t.is_bit_reg",
                              condition_api="ida_hexrays.mop_t.is_cc",
                              is_bit_register=bool(operand.is_bit_reg()),
                              is_condition_code=bool(operand.is_cc()))
                if not result["is_bit_register"]:
                    return None
        result["processor_register"] = int(hexrays.mreg2reg(observed["register"], width))
        if result.get("is_bit_register") is True and result["processor_register"] != -1:
            return None
        result["name"] = hexrays.get_mreg_name(observed["register"], width)
        result["status"] = "api_returned"
        result["complete"] = (width > 0 and observed["register_offset"] >= 0
                              and result["processor_register"] == -1
                              and isinstance(result["name"], str) and bool(result["name"]))
        if result.get("is_codegen_temporary") is True:
            result["complete"] = result["complete"] and observed["register_offset"] == 0
        if result.get("is_bit_register") is True:
            result["complete"] = (result["complete"] and width == 1 and observed["register_offset"] == 0
                                  and result["operand_is_exact_register"]
                                  and result["operand_register"] == observed["register"]
                                  and result["operand_width"] == width)
    except Exception as error:
        result["status"] = "diagnostic_error"
        _error(result, "kernel_register_identity", error)
    return result


def physical_tbyte_register_proof(observed, width, variable_type, hexrays, processor):
    """Verify an extended physical register independently of a failed location conversion."""
    if observed.get("kind") != "register" or width != 10 or processor is None:
        return None
    register = observed["register"]
    proof = {"status": "unobserved", "complete": False,
        "register_namespace": "hexrays_microregisters", "register": register,
        "register_offset": observed["register_offset"], "width": width,
        "identity_width": 8, "identity_mapping_only": True,
        "apis": {"type_float": "tinfo_t.is_float", "type_floating": "tinfo_t.is_floating",
            "type_tbyte": "tinfo_t.is_tbyte", "type_size": "tinfo_t.get_size",
            "operand": "ida_hexrays.mop_t.make_reg", "operand_exact": "mop_t.is_reg",
            "bit": "mop_t.is_bit_reg", "condition": "mop_t.is_cc",
            "operand_kernel": "mop_t.is_kreg", "kernel": "ida_hexrays.is_kreg",
            "temporary": "ida_hexrays.get_temp_regs; mlist_t.has_all",
            "processor": "ida_hexrays.mreg2reg", "identity_roundtrip": "ida_hexrays.reg2mreg",
            "processor_name": "ida_idp.get_reg_name", "processor_info": "ida_idp.parse_reg_name; reg_info_t.reg/size",
            "tbyte_size": "ida_idp.ph_get_tbyte_size"}}
    try:
        proof.update(type_is_float=bool(variable_type.is_float()),
                     type_is_floating=bool(variable_type.is_floating()), type_is_tbyte=bool(variable_type.is_tbyte()),
                     type_bytes=int(variable_type.get_size()),
                     is_kernel_register=bool(hexrays.is_kreg(register)),
                     is_codegen_temporary=bool(hexrays.get_temp_regs().has_all(register, width)))
        operand = hexrays.mop_t()
        operand.make_reg(register, width)
        proof.update(operand_register=int(operand.r), operand_width=int(operand.size),
                     operand_is_exact_register=bool(operand.is_reg(register, width)),
                     is_bit_register=bool(operand.is_bit_reg()), is_condition_code=bool(operand.is_cc()),
                     operand_is_kernel_register=bool(operand.is_kreg()),
                     processor_register_at_original_width=int(hexrays.mreg2reg(register, width)),
                     identity_processor_register=int(hexrays.mreg2reg(register, proof["identity_width"])))
        physical = proof["identity_processor_register"]
        if physical >= 0:
            proof["identity_roundtrip_microregister"] = int(hexrays.reg2mreg(physical))
            proof["processor_name_width"] = width
            name = processor.get_reg_name(physical, width)
            if not isinstance(name, str):
                raise TypeError("Processor register name is not text")
            proof["processor_name"] = name
            info = processor.reg_info_t()
            proof["processor_parsed"] = bool(processor.parse_reg_name(info, name))
            if proof["processor_parsed"]:
                proof.update(processor_parsed_register=int(info.reg), processor_capacity_bytes=int(info.size))
        proof["processor_tbyte_bytes"] = int(processor.ph_get_tbyte_size())
        proof["status"] = "api_returned"
        proof["complete"] = (observed["register_offset"] == 0 and not proof["type_is_float"]
            and proof["type_is_floating"] and proof["type_is_tbyte"]
            and proof["type_bytes"] == width and not proof["is_kernel_register"]
            and not proof["is_codegen_temporary"] and not proof["operand_is_kernel_register"]
            and not proof["is_bit_register"] and not proof["is_condition_code"]
            and proof["operand_register"] == register and proof["operand_width"] == width
            and proof["operand_is_exact_register"] and proof["processor_register_at_original_width"] == -1
            and physical >= 0 and proof.get("identity_roundtrip_microregister") == register
            and bool(proof.get("processor_name")) and proof.get("processor_parsed") is True
            and proof.get("processor_parsed_register") == physical
            and proof.get("processor_capacity_bytes") == width == proof["processor_tbyte_bytes"])
    except Exception as error:
        proof["status"] = "diagnostic_error"
        _error(proof, "physical_tbyte_register_identity", error)
    return proof


def _tbyte_scattered_location(record, namespace, first_register, sizes):
    if (record.get("status") != "observed" or record.get("complete") is not True
            or record.get("kind") != "scattered" or record.get("register_namespace") != namespace
            or record.get("part_count") != len(sizes) or len(record.get("parts", [])) != len(sizes)):
        return False
    offset = 0
    for part, size in zip(record["parts"], sizes):
        if (part.get("status") != "observed" or part.get("complete") is not True
                or part.get("kind") != "register" or part.get("register_namespace") != namespace
                or type(part.get("register")) is not int or part["register"] < 0
                or any(type(part.get(key)) is not int or part[key] != value for key, value in {
                    "argument_offset": offset, "size": size, "register_offset": 0}.items())):
            return False
        offset += size
    return record["parts"][0]["register"] == first_register


def local_variables(cfunc, hexrays, processor=None):
    """Return only Python values; no SDK proxy or object address is serialized."""
    values = []
    mba = cfunc.mba
    stack_delta = int(cfunc.get_stkoff_delta())
    for variable in cfunc.get_lvars():
        argument = variable.is_arg_var
        variable_type = variable.type()
        value = {"name": str(variable.name), "type": str(variable_type),
                 "argument": bool(argument() if callable(argument) else argument),
                 "width": int(variable.width), "definition_ea": hex(int(variable.defea)),
                 "definition_block": int(variable.defblk), "complete": True}
        if value["width"] <= 0:
            value["complete"] = False
        location = variable.location
        observed = argument_location(location, "hexrays_microregisters")
        value["location"] = observed
        if not observed["complete"]:
            value["complete"] = False
        try:
            observed["text"] = hexrays.print_vdloc(location, value["width"])
            if not isinstance(observed["text"], str):
                raise TypeError("print_vdloc did not return text")
        except Exception as error:
            _error(value, "print_vdloc", error)
        if value["width"] > 0 and observed["status"] == "observed":
            conversion = {"status": "unobserved", "complete": True}
            value["ida_location_conversion"] = conversion
            kernel = kernel_register_proof(observed, value["width"], hexrays)
            if kernel is not None:
                value["virtual_register_proof"] = kernel
                if kernel.get("is_kernel_register") is True:
                    reason = "sdk_kernel_register_has_no_processor_location"
                elif kernel.get("is_codegen_temporary") is True:
                    reason = "sdk_temporary_register_has_no_processor_location"
                elif kernel.get("is_bit_register") is True:
                    reason = "sdk_bit_register_has_no_processor_location"
                else:
                    reason = None
                conversion.update(status="not_applicable" if reason else "diagnostic_error",
                                  reason=reason or "virtual_register_identity_unverified", complete=kernel["complete"])
            else:
                try:
                    ida_location = mba.vd2idaloc(location, value["width"])
                    conversion["location"] = argument_location(ida_location, "ida_processor_registers")
                    back = mba.idaloc2vd(ida_location, value["width"])
                    conversion.update(status="api_returned", roundtrip_matches=bool(back == location))
                    conversion["complete"] = bool(conversion["location"]["complete"]
                                                   and conversion["roundtrip_matches"])
                    if value["width"] == 10 and observed.get("kind") == "register" and not conversion["complete"]:
                        conversion.update(conversion_api="mba_t.vd2idaloc", roundtrip_api="mba_t.idaloc2vd",
                            roundtrip_location=argument_location(back, "hexrays_microregisters"))
                        physical = physical_tbyte_register_proof(observed, value["width"], variable_type,
                                                                 hexrays, processor)
                        if physical is not None:
                            value["physical_register_proof"] = physical
                            if (physical["complete"] and conversion["roundtrip_matches"] is False
                                    and _tbyte_scattered_location(conversion["location"], "ida_processor_registers",
                                                                  physical["identity_processor_register"], (8, 2))
                                    and _tbyte_scattered_location(conversion["roundtrip_location"], "hexrays_microregisters",
                                                                  observed["register"], (10,))):
                                conversion = {"status": "physical_register_verified", "complete": True,
                                    "reason": "sdk_x87_tbyte_capacity_and_identity_roundtrip",
                                    "original_conversion": conversion}
                                value["ida_location_conversion"] = conversion
                except Exception as error:
                    conversion["status"] = "diagnostic_error"
                    _error(conversion, "vd2idaloc_roundtrip", error)
            if not conversion["complete"]:
                value["complete"] = False
        if observed.get("kind") == "stack":
            conversion = {"status": "unobserved", "complete": True}
            value["stack_coordinate_conversion"] = conversion
            try:
                offset = observed["stack_offset"]
                # cfunc_t documents this rule for optimized lvars; mba_t's
                # stack mapping uses a different coordinate space.
                ida_offset = offset - stack_delta
                returned = ida_offset + stack_delta
                conversion.update(status="api_returned", ida_offset=ida_offset,
                                  coordinate_namespace="ida_frame_offsets",
                                  conversion_api="cfunc_t.get_stkoff_delta",
                                  rule="ida_stkoff = v.location.stkoff() - cfunc.get_stkoff_delta()",
                                  cfunc_stack_offset_delta=stack_delta,
                                  decompiler_offset=offset,
                                  roundtrip_decompiler_offset=returned, roundtrip_matches=returned == offset,
                                  complete=returned == offset)
            except Exception as error:
                conversion["status"] = "diagnostic_error"
                _error(conversion, "stack_coordinate_roundtrip", error)
            if not conversion["complete"]:
                value["complete"] = False
            observation = {"status": "unobserved", "complete": False,
                           "coordinate_namespace": "hexrays_mba_stack_offsets",
                           "used_for_cfunc_frame_conversion": False}
            value["mba_stack_coordinate_observation"] = observation
            try:
                ida_offset = int(mba.stkoff_vd2ida(observed["stack_offset"]))
                returned = int(mba.stkoff_ida2vd(ida_offset))
                observation.update(status="api_returned", ida_offset=ida_offset,
                    roundtrip_decompiler_offset=returned,
                    roundtrip_matches=returned == observed["stack_offset"],
                    complete=returned == observed["stack_offset"])
            except Exception as error:
                observation["status"] = "diagnostic_error"
                _error(observation, "mba_stack_coordinate_observation", error)
        values.append(value)
    return {"locals": values, "locals_capture": {"schema_version": 2,
        "complete": all(value["complete"] for value in values),
        "location_limits": dict(LOCATION_LIMITS), "mba_entry_ea": hex(int(mba.entry_ea)),
        "stack_offset_delta": stack_delta}}

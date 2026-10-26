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
    """Keep SDK kernel registers in their actual, non-processor namespace."""
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
            return None
        result["processor_register"] = int(hexrays.mreg2reg(observed["register"], width))
        result["name"] = hexrays.get_mreg_name(observed["register"], width)
        result["status"] = "api_returned"
        result["complete"] = (width > 0 and observed["register_offset"] >= 0
                              and result["processor_register"] == -1
                              and isinstance(result["name"], str) and bool(result["name"]))
    except Exception as error:
        result["status"] = "diagnostic_error"
        _error(result, "kernel_register_identity", error)
    return result


def local_variables(cfunc, hexrays):
    """Return only Python values; no SDK proxy or object address is serialized."""
    values = []
    mba = cfunc.mba
    stack_delta = int(cfunc.get_stkoff_delta())
    for variable in cfunc.get_lvars():
        argument = variable.is_arg_var
        value = {"name": str(variable.name), "type": str(variable.type()),
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
                conversion.update(
                    status="not_applicable" if kernel.get("is_kernel_register") is True else "diagnostic_error",
                    reason="sdk_kernel_register_has_no_processor_location"
                    if kernel.get("is_kernel_register") is True else "kernel_register_identity_unverified",
                    complete=kernel["complete"])
            else:
                try:
                    ida_location = mba.vd2idaloc(location, value["width"])
                    conversion["location"] = argument_location(ida_location, "ida_processor_registers")
                    back = mba.idaloc2vd(ida_location, value["width"])
                    conversion.update(status="api_returned", roundtrip_matches=bool(back == location))
                    conversion["complete"] = bool(conversion["location"]["complete"]
                                                   and conversion["roundtrip_matches"])
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

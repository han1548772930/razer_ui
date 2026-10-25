"""Start the project-owned static IDA bridge with the selected analysis mode.

Pass only a private, hash-verified analysis input to IDA. This script starts
the analyzer plugin; it never calls target exports or debugger operations.
"""
import hashlib
import json
import os
import traceback
from pathlib import Path

import ida_auto
import idaapi

if os.environ.get("RAZER_IDA_CAPTURE_PLAN"):
    exit_code = 0
    try:
        plan = json.loads(Path(os.environ["RAZER_IDA_CAPTURE_PLAN"]).read_bytes())
        if "exception_regions" in plan:
            ida_auto.enable_auto(False)
            entry = plan["tool_sources"]["entry"]
            actual = Path(__file__).resolve()
            if ((actual.parents[2] / entry["path"]).resolve() != actual
                    or actual.stat().st_size != entry["bytes"]
                    or hashlib.sha256(actual.read_bytes()).hexdigest() != entry["sha256"]):
                raise ValueError("Exception-region running entry differs from its bound source")
        else:
            ida_auto.auto_wait()
        import runpy
        module = runpy.run_path(str(Path(__file__).with_name("capture.py")))
        module["capture"]()
    except Exception:
        exit_code = 1
        print("[STATIC-CAPTURE] fatal failure:\n" + traceback.format_exc(), flush=True)
    import idc
    idc.qexit(exit_code)
else:
    ida_auto.auto_wait()
    result = idaapi.load_and_run_plugin("ida_mcp", 0)
    print("[IDA-MCP] project plugin startup:", result)

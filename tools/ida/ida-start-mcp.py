"""Start the project-owned static IDA bridge after autoanalysis.

Pass only a private, hash-verified analysis input to IDA. This script starts
the analyzer plugin; it never calls target exports or debugger operations.
"""
import os
import traceback
from pathlib import Path

import ida_auto
import idaapi

ida_auto.auto_wait()

if os.environ.get("RAZER_IDA_CAPTURE_PLAN"):
    exit_code = 0
    try:
        import runpy
        module = runpy.run_path(str(Path(__file__).with_name("capture.py")))
        module["capture"]()
    except Exception:
        exit_code = 1
        print("[STATIC-CAPTURE] fatal failure:\n" + traceback.format_exc(), flush=True)
    import idc
    idc.qexit(exit_code)
else:
    result = idaapi.load_and_run_plugin("ida_mcp", 0)
    print("[IDA-MCP] project plugin startup:", result)

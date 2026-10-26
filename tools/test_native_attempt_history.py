"""Reject incomplete, changed and falsely successful analyzer-history evidence."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import analysis
import native_adjudication as evidence
import native_attempt_history as history
import test_native_import_forwarders as forwarder_fixtures


class AttemptHistoryTests(unittest.TestCase):
    def partial_fixture(self, root, pseudocode=True):
        f = forwarder_fixtures.ImportForwarderTests().fixture(root)
        raw = bytearray(f.raw)
        raw[0x220:0x227] = bytes.fromhex("eb03000000c3c3")
        raw = bytes(raw)
        f.source.write_bytes(raw)
        f.raw = raw
        f.result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        assembly = [{"address": "0x401020", "rva": "0x1020", "bytes": "eb03", "text": "jmp short interior",
                     "xrefs_from": [{"from": "0x401020", "to": "0x401025", "type": 19, "code": True, "user": False}],
                     "xrefs_to": []},
                    {"address": "0x401025", "rva": "0x1025", "bytes": "c3", "text": "ret", "xrefs_from": [], "xrefs_to": []},
                    {"address": "0x401026", "rva": "0x1026", "bytes": "c3", "text": "ret", "xrefs_from": [], "xrefs_to": []}]
        pseudo = {"lines": [{"text": "return;"}], "locals": [],
                  "locals_capture": {"complete": True, "schema_version": 2, "mba_entry_ea": "0x401020"}}
        old = {"address": "0x401020", "rva": "0x1020", "end": "0x401027", "name": "entry_jump", "flags": 0,
               "type": "void()", "type_before_decompile": "void()", "type_after_decompile": "void()",
               "chunks": [{"start": "0x401020", "end": "0x401027", "raw_hex": "eb03000000c3c3",
                           "sha256": hashlib.sha256(bytes.fromhex("eb03000000c3c3")).hexdigest()}],
               "assembly": assembly, "assembly_after_decompile": copy.deepcopy(assembly), "xrefs_to_entry": [],
               "pseudocode": pseudo if pseudocode else None}
        old_id = f.write(f.result / "functions/1020.json", old)
        base_record = {"rva": "0x1020", "address": "0x401020", "path": "functions/1020.json", "sha256": old_id["sha256"],
                       "assembly_captured": True, "pseudocode_captured": pseudocode}
        f.capture.update(input_sha256=hashlib.sha256(raw).hexdigest(), functions=[base_record], functions_total=1,
                         functions_captured=1, settled_functions_total=1, failures=[])
        base_id = f.write(f.result / "capture.json", f.capture)
        selection = analysis.native_function_selection(raw, f.capture, [0x1020], max_function_kib=1024)
        source_path = root / ".work/native" / f.result.name / "batch/jump.sys.i64"
        source_path.parent.mkdir(parents=True); source_path.write_bytes(b"actual retained source database")
        source_db = {key: value for key, value in evidence.identity(analysis, source_path).items() if key != "bytes"}
        f.directory = f.result / "supplements/partial"
        private = {"path": analysis.rel(root / ".work/native" / f.result.name / "supplements/partial/jump.sys.i64"),
                   "sha256": source_db["sha256"]}
        f.old_path = f.directory / "functions/1020.json"
        identity = f.write(f.old_path, old)
        record = {**base_record, "sha256": identity["sha256"]}
        f.plan = {"input": analysis.rel(f.source), "sha256": f.result.name, "input_bytes": len(raw),
                  "output": analysis.rel(f.directory), "base_capture": {key: base_id[key] for key in ("path", "sha256")},
                  "database": private, "source_database": source_db, "function_selection": selection,
                  "target_execution": False, "real_device_operations": False}
        tool = {"path": "tools/ida/supplement.py", "sha256": "a" * 64}
        f.session = {"started_at_utc": "2026-10-20T00:00:00+00:00", "finished_at_utc": "2026-10-20T00:00:03+00:00",
                     "passed": False, "exit_code": 0, "capture_tool": tool, "database": private,
                     "target_execution": False, "real_device_operations": False}
        f.manifest = {"input": analysis.rel(f.source), "input_sha256": f.result.name, "image_base": "0x400000",
                      "base_capture": f.plan["base_capture"], "source_database": source_db, "database": private,
                      "function_selection": selection, "capture_complete": False, "all_pseudocode_captured": False,
                      "database_integrity": {"verified": True}, "base_capture_unchanged": True, "base_database_unchanged": True,
                      "capture_tool": tool, "started_at_utc": "2026-10-20T00:00:01+00:00",
                      "finished_at_utc": "2026-10-20T00:00:02+00:00", "target_execution": False, "real_device_operations": False,
                      "functions": [record], "chains": [{"index": 0, "requested_rva": "0x1020", "complete": False,
                          "entries": [{"address": "0x401020", "rva": "0x1020", "direct_jump_target_rva": "0x1025"}]}],
                      "failures": [{"requested_rva": "0x1020", "stage": "target", "error":
                          "Traceback (most recent call last):\n  original traceback remains\nValueError: Requested entry lies inside a different function: 0x1025\n"}]}
        if not pseudocode:
            f.manifest["failures"].insert(0, {"rva": "0x1020", "stage": "hexrays", "error":
                                          "Traceback (most recent call last):\nValueError: Hex-Rays: call analysis failed\n"})
        f.failure = {"supplement": analysis.rel(f.directory / "capture.json"), "error": "original strict verification failed"}
        current = copy.deepcopy(old); current["pseudocode"] = pseudo
        current_dir = f.result / "supplements/complete"
        current_id = f.write(current_dir / "functions/1020.json", current)
        current_record = {**record, "sha256": current_id["sha256"], "pseudocode_captured": True}
        replacement_manifest = f.write(current_dir / "capture.json", {"functions": [current_record]})
        f.resolution = {"base_capture": base_id, "target_execution": False, "verification_failures": [f.failure],
                        "complete_functions": {"0x1020": {"capture": current_id, **current}},
                        "conflicting_functions": {}, "conflicting_function_rvas": [],
                        "supplements": [{"receipt": replacement_manifest}]}
        f.old, f.current_path, f.source_db_path = old, current_dir / "functions/1020.json", source_path
        self.save_partial(f)
        return f

    def save_partial(self, f):
        for name in ("plan", "session", "manifest"):
            f.write(f.directory / ("capture.json" if name == "manifest" else name + ".json"), getattr(f, name))

    def partial(self, f):
        return history.reviewed_partial_attempt_coverage(analysis, f.result, f.resolution, f.failure)

    def test_partial_target_failures_preserve_exact_failure_and_full_undecoded_bytes(self):
        for pseudocode in (True, False):
            with self.subTest(pseudocode=pseudocode), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.partial_fixture(Path(directory), pseudocode)
                files = {p: p.read_bytes() for p in f.directory.rglob("*.json")}
                resolution = copy.deepcopy(f.resolution)
                coverage = self.partial(f)
                self.assertEqual(coverage["verification_failure"], f.failure)
                self.assertEqual(coverage["original_failures"], f.manifest["failures"])
                self.assertEqual(coverage["capture_outcome"], "ended_partial_target_chain_failure")
                self.assertFalse(coverage["failed_attempt_success"])
                scope = coverage["functions"][0]["previous_machine_scope"]
                self.assertEqual(scope["bytes_not_marked_as_instructions"][0]["raw_hex"], "000000")
                self.assertFalse(scope["undecoded_bytes_reinterpreted_as_code"])
                self.assertEqual(f.resolution, resolution)
                self.assertEqual({p: p.read_bytes() for p in files}, files)

    def test_missing_target_original_byte_changes_and_incomplete_current_replacement_are_rejected(self):
        cases = [lambda f: f.manifest["functions"].clear(),
                 lambda f: f.manifest["functions"][0].update(sha256="0" * 64),
                 lambda f: f.old["chunks"][0].update(raw_hex="eb03000000c390"),
                 lambda f: f.old["assembly"][0].update(bytes="eb02"),
                 lambda f: f.old["assembly"][0].update(rva="0x1021"),
                 lambda f: f.resolution["complete_functions"].clear(),
                 lambda f: f.resolution["supplements"].clear(),
                 lambda f: f.resolution["conflicting_functions"].update({"0x1020": []}),
                 lambda f: f.resolution["complete_functions"]["0x1020"]["pseudocode"].update(lines=[])]
        for mutation in cases:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.partial_fixture(Path(directory), False); mutation(f)
                if f.old != json.loads(f.old_path.read_bytes()):
                    changed = f.write(f.old_path, f.old); f.manifest["functions"][0]["sha256"] = changed["sha256"]
                self.save_partial(f)
                with self.assertRaises((ValueError, KeyError)): self.partial(f)

    def test_session_success_preexport_live_or_changed_source_cannot_be_retired_as_partial(self):
        cases = [lambda f: f.session.update(passed=True), lambda f: f.session.update(exit_code=1),
                 lambda f: f.session.pop("finished_at_utc"), lambda f: f.manifest.update(capture_complete=True),
                 lambda f: f.manifest.update(base_database_unchanged=False),
                 lambda f: f.session.update(finished_at_utc="2026-10-20T00:00:00+00:00"),
                 lambda f: f.plan["source_database"].update(sha256="0" * 64),
                 lambda f: f.source_db_path.write_bytes(b"changed actual source database"),
                 lambda f: f.resolution["verification_failures"].clear(),
                 lambda f: f.plan["function_selection"].update(unreviewed_operation=True)]
        for mutation in cases:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.partial_fixture(Path(directory)); mutation(f); self.save_partial(f)
                with self.assertRaises((ValueError, KeyError)): self.partial(f)

    def test_every_target_chain_and_full_original_traceback_must_still_be_present(self):
        cases = [lambda f: f.manifest["chains"].clear(),
                 lambda f: f.manifest["chains"][0].update(complete=True),
                 lambda f: f.manifest["chains"][0]["entries"][0].update(direct_jump_target_rva="0x1026"),
                 lambda f: f.manifest["failures"].clear(),
                 lambda f: f.manifest["failures"][0].update(error="short summary"),
                 lambda f: f.manifest["failures"][0].update(requested_rva="0x1026"),
                 lambda f: f.manifest["failures"].append(copy.deepcopy(f.manifest["failures"][0]))]
        for mutation in cases:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.partial_fixture(Path(directory)); mutation(f); self.save_partial(f)
                with self.assertRaises((ValueError, KeyError)): self.partial(f)

    def abi_fixture(self, root):
        f = forwarder_fixtures.ImportForwarderTests().fixture(root)
        definitions = "\n".join(["typedef char CHAR;", "#define CONST               const",
            "typedef _Null_terminated_ CONST CHAR *LPCSTR, *PCSTR;", "typedef unsigned long ULONG;"])
        header_path = root / ".analysis/inputs/sdk/ntdef.h"; header_path.parent.mkdir(parents=True); header_path.write_text(definitions)
        header_id = evidence.identity(analysis, header_path)
        header = {**header_id, "kind": "installed_os_sdk_header", "target_execution": False,
            "acquisition_chain_retained": False, "acquisition_receipt": {"sdk_version": "10.0.26100.0",
            "source_locator": "Windows Kits/10/Include/10.0.26100.0/shared/ntdef.h",
            "installed_source_bytes_verified_before_retention": True, "retained_original_header": True}}
        docs = []
        independent_sources = {
            "dbgprint": ("https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-dbgprint", "ULONG DbgPrint( PCSTR Format, ... );"),
            "cdecl": ("https://learn.microsoft.com/en-us/cpp/cpp/cdecl?view=msvc-170", "Calling function pops the arguments from the stack."),
            "argument_passing": ("https://learn.microsoft.com/en-us/cpp/cpp/argument-passing-and-naming-conventions?view=msvc-170", "Return values are also widened to 32 bits and returned in the EAX register"),
            "type_ranges": ("https://learn.microsoft.com/en-us/cpp/cpp/data-type-ranges?view=msvc-170", "unsigned __int32")}
        for role, (url, text) in independent_sources.items():
            path = root / ".analysis/inputs/docs" / (role + ".html"); path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("<html><p>" + text + "</p></html>")
            docs.append({**evidence.identity(analysis, path), "role": role, "url": url, "final_url": url,
                         "http_status": 200, "kind": "official_windows_abi_document", "target_execution": False})
        f.acquisition = {"schema_version": 1, "operation": "retain_windows_sdk_and_official_x86_abi_evidence",
                         "header": header, "documents": docs, "target_execution": False}
        source_path = root / ".analysis/results/input.c"; source_path.write_bytes(history.alias_compiler_source().encode())
        ir = 'target triple = "i686-pc-windows-msvc19.44.35228"\ntarget datalayout = "e-m:x-p:32:32-rest"\n'
        for name in ("call_original", "call_previous"):
            ir += 'define dso_local i32 @' + name + '(ptr noundef %0, ptr noundef %1, i32 noundef %2) #0 {\n  %10 = call i32 (ptr, ...) %0(ptr noundef %1, i32 noundef %2)\n  ret i32 %10\n}\n'
        f.ir_path = root / ".analysis/results/abi.ll"; f.ir_path.write_text(ir)
        f.compiler = {"schema_version": 1, "operation": "static_i686_windows_variadic_alias_abi_compile",
            "argv": ["<clang-static-compiler>", "-target", "i686-pc-windows-msvc", "-S", "-emit-llvm", "-O0", "-fno-ident", "-fdebug-compilation-dir=.", "-x", "c", "-o", "-", "-"],
            "exit_code": 0, "stderr": None, "target_execution": False, "temporary_files_created": False,
            "compiler_version": "clang version independent test fixture", "compiler": {"tool_locator": "LLVM/bin/clang.exe", "bytes": 100, "sha256": "1" * 64},
            "stdin": evidence.identity(analysis, source_path), "stdout": evidence.identity(analysis, f.ir_path)}
        f.acquisition_path, f.compiler_path = root / ".analysis/results/acquisition.json", root / ".analysis/results/compiler.json"
        return f

    def abi(self, f):
        return history._abi_sources(analysis, f.write(f.acquisition_path, f.acquisition), f.write(f.compiler_path, f.compiler))

    def test_sdk_and_full_official_sources_preserve_unsigned_const_pointer_varargs_and_caller_cleanup(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.abi_fixture(Path(directory)); abi = self.abi(f)
            self.assertEqual(abi["return_bits"], 32); self.assertFalse(abi["return_signed"])
            self.assertEqual(abi["return_register"], "eax"); self.assertEqual(abi["stack_cleanup"], "caller")
            self.assertTrue(abi["variadic"]); self.assertTrue(abi["fixed_parameter"]["pointee_const"])
            self.assertFalse(abi["c_language_type_identity_claimed"])
            self.assertFalse(abi["sdk_installer_acquisition_chain_retained"])

    def test_changed_sdk_signature_source_url_or_missing_whole_document_fail(self):
        cases = [lambda f: f.acquisition["documents"].pop(),
                 lambda f: f.acquisition["documents"][0].update(url="https://example.invalid/forged"),
                 lambda f: f.acquisition["documents"][0].update(sha256="0" * 64),
                 lambda f: f.acquisition["header"].update(sha256="0" * 64),
                 lambda f: f.acquisition.update(target_execution=True),
                 lambda f: f.acquisition["header"]["acquisition_receipt"].update(retained_original_header=False)]
        for mutation in cases:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.abi_fixture(Path(directory)); mutation(f)
                with self.assertRaises(ValueError): self.abi(f)

    def test_compiler_failure_removed_ellipsis_changed_width_and_stdcall_lowering_fail(self):
        cases = [lambda f: f.compiler.update(exit_code=1), lambda f: f.compiler.update(target_execution=True),
                 lambda f: f.compiler["argv"].__setitem__(2, "x86_64-pc-windows-msvc"),
                 lambda f: f.ir_path.write_text(f.ir_path.read_text().replace("call i32 (ptr, ...)", "call i32 (ptr)")),
                 lambda f: f.ir_path.write_text(f.ir_path.read_text().replace("i32", "i64")),
                 lambda f: f.ir_path.write_text(f.ir_path.read_text().replace("call i32", "call x86_stdcallcc i32")),
                 lambda f: f.ir_path.write_text(f.ir_path.read_text().replace("ret i32 %10", "ret i32 0", 1))]
        for mutation in cases:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.abi_fixture(Path(directory)); mutation(f)
                f.compiler["stdout"] = evidence.identity(analysis, f.ir_path)
                with self.assertRaises(ValueError): self.abi(f)

    def alias_fixture(self, root):
        f = self.abi_fixture(root)
        f.proof = forwarder_fixtures.ImportForwarderTests().prove(f)
        previous = copy.deepcopy(f.bodies[0]); previous.update(type="unsigned __int32(const char *Format, ...)", boundary_source="existing")
        f.old_path = f.result / "supplements/alias/functions/1000.json"
        f.old_identity = f.write(f.old_path, previous)
        request = {"declaration": "unsigned __int32 (__cdecl *DbgPrint)(const char *Format, ...);"}
        f.alias_manifest = {"function_selection": {"import_type_recoveries": [request]},
            "import_type_recoveries": [{**request, "previous_type": "ULONG (*)(PCSTR Format, ...)",
                "applied_type": "unsigned __int32 (*)(const char *Format, ...)", "base_database_modified": False}],
            "import_thunk_type_recoveries": [{"previous_type": "ULONG(PCSTR Format, ...)",
                "applied_type": previous["type"], "source_function": f.proof["function"], "base_database_modified": False}]}
        f.write(f.old_path.parent.parent / "capture.json", f.alias_manifest)
        f.write(f.old_path.parent.parent / "plan.json", {"function_selection": f.alias_manifest["function_selection"]})
        return f

    def alias_review(self, f):
        return history.build_alias_review(analysis, f.result, f.proof, f.old_identity,
            f.write(f.acquisition_path, f.acquisition), f.write(f.compiler_path, f.compiler))

    def test_explicit_alias_review_does_not_relax_default_forwarder_or_relabel_failure_as_pseudocode(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), patch.object(
                history, "source_identity", return_value={"path": "tools/native_attempt_history.py", "bytes": 1, "sha256": "2" * 64}), patch.object(history, "_caller_cleanup", return_value=[]):
            f = self.alias_fixture(Path(directory))
            with self.assertRaises(ValueError):
                history.forwarders.previous_function_coverage(analysis, f.result, f.proof, f.old_identity)
            review = self.alias_review(f); identity = f.write(f.root / ".analysis/results/alias-review.json", review)
            before = f.old_path.read_bytes()
            coverage = history.previous_alias_forwarder_coverage(analysis, f.result, {**f.proof, "alias_abi_review": identity}, f.old_identity)
            self.assertTrue(coverage["machine_transfer_complete"]); self.assertFalse(coverage["whole_attempt_retired"])
            self.assertFalse(coverage["original_hexrays_failure_reinterpreted_as_success"])
            self.assertFalse(coverage["pseudocode_recovered"]); self.assertEqual(f.old_path.read_bytes(), before)
            review["abi"]["variadic"] = False
            with self.assertRaises(ValueError):
                history.previous_alias_forwarder_coverage(analysis, f.result,
                    {**f.proof, "alias_abi_review": f.write(f.root / ".analysis/results/alias-review.json", review)}, f.old_identity)

    def test_alias_cannot_cover_different_machine_flags_callers_or_const_and_variadic_types(self):
        mutations = [lambda b: b.update(type="unsigned __int32(char *Format, ...)"),
                     lambda b: b.update(type="unsigned __int32(const char *Format)"),
                     lambda b: b.update(flags=0), lambda b: b["assembly"][0]["xrefs_to"].pop(),
                     lambda b: b.update(pseudocode={"lines": [{"text": "invented success"}]}),
                     lambda b: b.update(extra_unreviewed_field=True)]
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), patch.object(
                    history, "source_identity", return_value={"path": "tools/native_attempt_history.py", "bytes": 1, "sha256": "2" * 64}), patch.object(history, "_caller_cleanup", return_value=[]):
                f = self.alias_fixture(Path(directory)); old = json.loads(f.old_path.read_bytes()); mutation(old)
                f.old_identity = f.write(f.old_path, old)
                with self.assertRaises(ValueError): self.alias_review(f)

    def test_original_caller_cleanup_witnesses_are_byte_bound_and_not_a_document_only_claim(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory); calls = []
            for rva, cleanup in [("0x8593a", ["59"]), ("0x88c5f", ["59", "59"]), ("0x89cc2", ["83c410"])]:
                at = 0x400000 + int(rva, 16)
                rows = [{"address": hex(at), "rva": rva, "bytes": "e800000000"}]
                at += 5
                for code in cleanup:
                    rows.append({"address": hex(at), "bytes": code}); at += len(bytes.fromhex(code))
                path = root / ".analysis/results" / (rva[2:] + ".json"); path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps({"assembly": rows})); calls.append({"call_rva": rva, "owner_function": evidence.identity(analysis, path)})
            current = {"incoming_calls": calls}
            proved = history._caller_cleanup(analysis, current)
            self.assertEqual([row["caller_releases_argument_bytes"] for row in proved], [4, 8, 16])
            path = root / calls[0]["owner_function"]["path"]
            changed = json.loads(path.read_bytes()); changed["assembly"][1]["bytes"] = "5a"; path.write_text(json.dumps(changed))
            calls[0]["owner_function"] = evidence.identity(analysis, path)
            with self.assertRaises(ValueError): history._caller_cleanup(analysis, current)


if __name__ == "__main__":
    unittest.main()

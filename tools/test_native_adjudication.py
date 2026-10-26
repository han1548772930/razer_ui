"""Behavioral checks for explicit native evidence adjudication boundaries."""
import copy
from contextlib import redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import analysis
import native_adjudication as adjudication
import test_evidence


class NativeAdjudicationTests(unittest.TestCase):
    def fixture(self, root):
        owner = test_evidence.NativeSupplementTests()
        result, first, output, capture, manifest, body, _, write = owner.conflicting_reanalysis_fixture(
            root, location_only=True)
        body["type"] = body["type_before_decompile"] = body["type_after_decompile"] = "void __cdecl()"
        body["assembly_after_decompile"] = copy.deepcopy(body["assembly"])
        body["pseudocode"]["locals"] = []
        body["pseudocode"]["locals_capture"] = {"complete": True, "mba_entry_ea": body["address"]}
        record = write(output / "functions/1000.json", body)
        manifest["functions"][0]["sha256"] = record["sha256"]
        write(output / "capture.json", manifest)
        resolution = analysis.native_reanalysis_resolution(result)
        evidence_path = root / ".analysis/results/reviewed-machine.json"
        write(evidence_path, {"scope": "synthetic original machine and every changed field reviewed"})
        evidence = adjudication.identity(analysis, evidence_path)
        candidates = []
        for directory in (first, output):
            candidates.append({"function": adjudication.identity(analysis, directory / "functions/1000.json"),
                               "supplement": adjudication.identity(analysis, directory / "capture.json")})
        selected = adjudication.identity(analysis, output / "functions/1000.json")
        previous = adjudication.identity(analysis, first / "functions/1000.json")
        changes = adjudication.differences(json.loads((first / "functions/1000.json").read_bytes()), body)
        review = {"schema_version": 1, "review_kind": "native_complete_artifact_adjudication",
                  "source_input": adjudication.identity(analysis, root / capture["input"]),
                  "base_capture": resolution["base_capture"], "target_execution": False,
                  "raw_artifacts_modified": False, "evidence": [evidence], "functions": [{
                      "rva": "0x1000", "candidates": candidates, "selected": selected,
                      "evidence": [evidence], "reason": "Synthetic independently reviewed complete artifact",
                      "comparisons": [{"previous": previous, "differences": [dict(change,
                          classification="capture_metadata_replacement", reason="Exact synthetic field review")
                          for change in changes]}]}]}
        review_path = root / ".analysis/results/review.json"
        write(review_path, review)
        return SimpleNamespace(result=result, first=first, output=output, capture=capture,
            manifest=manifest, body=body, write=write, review=review, review_path=review_path,
            review_record=adjudication.identity(analysis, review_path), evidence_path=evidence_path)

    def resolve(self, fixture):
        return analysis.native_reanalysis_resolution(fixture.result, adjudication_record=fixture.review_record)

    def test_kernel_register_requires_exact_live_sdk_proof_in_its_own_namespace(self):
        for issue in (None, "schema", "kernel", "cpu", "name", "width", "register", "offset", "kind", "api"):
            with self.subTest(issue=issue):
                body = {"address": "0x1000", "type": "void()", "type_before_decompile": "void()",
                    "type_after_decompile": "void()", "assembly": [], "assembly_after_decompile": [],
                    "pseudocode": {"locals_capture": {"schema_version": 2, "complete": True,
                                                      "mba_entry_ea": "0x1000"}, "locals": [{
                        "complete": True, "width": 4,
                        "location": {"complete": True, "status": "observed", "kind": "register",
                                     "register_namespace": "hexrays_microregisters", "register": 384, "register_offset": 0},
                        "ida_location_conversion": {"status": "not_applicable", "complete": True,
                                                    "reason": "sdk_kernel_register_has_no_processor_location"},
                        "virtual_register_proof": {"status": "api_returned", "complete": True,
                            "register_namespace": "hexrays_microregisters", "register": 384, "register_offset": 0,
                            "width": 4, "kernel_api": "ida_hexrays.is_kreg", "processor_api": "ida_hexrays.mreg2reg",
                            "name_api": "ida_hexrays.get_mreg_name", "is_kernel_register": True,
                            "processor_register": -1, "name": "kr00_4"}}]}}
                local = body["pseudocode"]["locals"][0]
                proof = local["virtual_register_proof"]
                if issue == "schema": body["pseudocode"]["locals_capture"]["schema_version"] = 1
                elif issue == "kernel": proof["is_kernel_register"] = False
                elif issue == "cpu": proof["processor_register"] = 0
                elif issue == "name": proof["name"] = ""
                elif issue == "width": proof["width"] = 8
                elif issue == "register": proof["register"] = 385
                elif issue == "offset": proof["register_offset"] = 4
                elif issue == "kind": local["location"]["kind"] = "register_pair"
                elif issue == "api": proof["kernel_api"] = "inferred_name"
                if issue is None: adjudication._typed_function(body)
                else:
                    with self.assertRaises(ValueError): adjudication._typed_function(body)

    def test_cfunc_stack_coordinates_verify_signed_offsets_delta_and_exact_documented_rule(self):
        for issue in (None, "delta", "offset", "ida_offset", "roundtrip", "namespace", "rule", "api", "bool"):
            with self.subTest(issue=issue):
                body = {"address": "0x1000", "type": "void()", "type_before_decompile": "void()",
                    "type_after_decompile": "void()", "assembly": [], "assembly_after_decompile": [],
                    "pseudocode": {"locals_capture": {"schema_version": 2, "complete": True,
                        "mba_entry_ea": "0x1000", "stack_offset_delta": 72}, "locals": [{
                        "complete": True, "width": 4,
                        "location": {"complete": True, "status": "observed", "kind": "stack",
                                     "register_namespace": "hexrays_microregisters", "stack_offset": 4},
                        "ida_location_conversion": {"complete": True, "roundtrip_matches": True},
                        "stack_coordinate_conversion": {"complete": True, "roundtrip_matches": True,
                            "coordinate_namespace": "ida_frame_offsets", "conversion_api": "cfunc_t.get_stkoff_delta",
                            "rule": "ida_stkoff = v.location.stkoff() - cfunc.get_stkoff_delta()",
                            "cfunc_stack_offset_delta": 72, "decompiler_offset": 4, "ida_offset": -68,
                            "roundtrip_decompiler_offset": 4}}]}}
                stack = body["pseudocode"]["locals"][0]["stack_coordinate_conversion"]
                if issue == "delta": stack["cfunc_stack_offset_delta"] = 0
                elif issue == "offset": stack["decompiler_offset"] = 76
                elif issue == "ida_offset": stack["ida_offset"] = 4
                elif issue == "roundtrip": stack["roundtrip_decompiler_offset"] = 76
                elif issue == "namespace": stack["coordinate_namespace"] = "mba_stack_offsets"
                elif issue == "rule": stack["rule"] = "clamp_to_zero"
                elif issue == "api": stack["conversion_api"] = "mba_t.stkoff_vd2ida"
                elif issue == "bool": body["pseudocode"]["locals_capture"]["stack_offset_delta"] = True
                if issue is None: adjudication._typed_function(body)
                else:
                    with self.assertRaises(ValueError): adjudication._typed_function(body)

    def test_instruction_text_changes_require_exact_review_and_preserve_all_other_instruction_fields(self):
        for issue in (None, "missing", "text", "reason", "duplicate", "bytes", "xref"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                before = json.loads((f.first / "functions/1000.json").read_bytes())
                before["assembly"][0]["text"] = "call original_symbol"
                old = f.write(f.first / "functions/1000.json", before)
                first_manifest = json.loads((f.first / "capture.json").read_bytes())
                first_manifest["functions"][0]["sha256"] = old["sha256"]
                f.write(f.first / "capture.json", first_manifest)
                f.body["assembly"][0]["text"] = "call reviewed_symbol"
                if issue == "bytes": f.body["assembly"][0]["bytes"] = "90"
                elif issue == "xref": f.body["assembly"][0]["xrefs_from"] = [{"to": "0x140001001"}]
                f.body["assembly_after_decompile"] = copy.deepcopy(f.body["assembly"])
                record = f.write(f.output / "functions/1000.json", f.body)
                f.manifest["functions"][0]["sha256"] = record["sha256"]
                f.write(f.output / "capture.json", f.manifest)
                entry = f.review["functions"][0]
                entry["candidates"] = [{"function": adjudication.identity(analysis, directory / "functions/1000.json"),
                                        "supplement": adjudication.identity(analysis, directory / "capture.json")}
                                       for directory in (f.first, f.output)]
                entry["selected"] = adjudication.identity(analysis, f.output / "functions/1000.json")
                comparison = entry["comparisons"][0]
                comparison["previous"] = adjudication.identity(analysis, f.first / "functions/1000.json")
                comparison["differences"] = [dict(change, classification="capture_metadata_replacement",
                    reason="Synthetic complete display and type metadata review")
                    for change in adjudication.differences(before, f.body)]
                comparison["assembly_rendering_differences"] = [{"address": before["assembly"][0]["address"],
                    "previous_text": "call original_symbol", "selected_text": "call reviewed_symbol",
                    "reason": "Synthetic symbol display replacement; bytes and all instruction metadata are identical"}]
                if issue == "missing": del comparison["assembly_rendering_differences"]
                elif issue == "text": comparison["assembly_rendering_differences"][0]["previous_text"] = "invented"
                elif issue == "reason": comparison["assembly_rendering_differences"][0]["reason"] = ""
                elif issue == "duplicate": comparison["assembly_rendering_differences"] *= 2
                f.write(f.review_path, f.review)
                f.review_record = adjudication.identity(analysis, f.review_path)
                resolved = self.resolve(f)
                self.assertEqual("0x1000" in resolved["adjudicated_functions"] if "adjudicated_functions" in resolved else False,
                                 issue is None)
                self.assertEqual(bool(resolved["verification_failures"]), issue is not None)

    def test_explicit_adjudication_recovers_failure_and_retains_every_conflict_fact(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            paths = [f.result / "capture.json", f.first / "functions/1000.json", f.output / "functions/1000.json"]
            before = [p.read_bytes() for p in paths]
            resolved = self.resolve(f)
            self.assertFalse(resolved["verification_failures"])
            self.assertFalse(resolved["remaining_failures"])
            self.assertFalse(resolved["conflicting_functions"])
            self.assertEqual(set(resolved["complete_functions"]), {"0x1000", "0x1010"})
            self.assertEqual(set(resolved["historical_conflicting_functions"]), {"0x1000"})
            self.assertTrue(resolved["historical_verification_failures"])
            self.assertEqual(resolved["resolved_failures"][0]["original_failure"], f.capture["failures"][0])
            self.assertEqual([p.read_bytes() for p in paths], before)
            ordinary = analysis.native_reanalysis_resolution(f.result)
            self.assertEqual(ordinary["conflicting_function_rvas"], ["0x1000"])

    def test_every_changed_field_and_every_complete_candidate_must_be_bound(self):
        mutations = ("missing_candidate", "missing_difference", "changed_difference", "missing_reason", "wrong_selected",
                     "wrong_original", "wrong_base", "missing_evidence", "duplicate_function", "duplicate_comparison")
        for issue in mutations:
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                entry = f.review["functions"][0]
                if issue == "missing_candidate": entry["candidates"].pop()
                elif issue == "missing_difference": entry["comparisons"][0]["differences"].pop()
                elif issue == "changed_difference": entry["comparisons"][0]["differences"][0]["selected"]["sha256"] = "0" * 64
                elif issue == "missing_reason": entry["comparisons"][0]["differences"][0]["reason"] = ""
                elif issue == "wrong_selected": entry["selected"]["sha256"] = "0" * 64
                elif issue == "wrong_original": f.review["source_input"]["sha256"] = "0" * 64
                elif issue == "wrong_base": f.review["base_capture"]["sha256"] = "0" * 64
                elif issue == "missing_evidence": entry["evidence"] = []
                elif issue == "duplicate_function": f.review["functions"].append(copy.deepcopy(entry))
                else: entry["comparisons"].append(copy.deepcopy(entry["comparisons"][0]))
                f.write(f.review_path, f.review)
                f.review_record = adjudication.identity(analysis, f.review_path)
                resolved = self.resolve(f)
                self.assertEqual(resolved["conflicting_function_rvas"], ["0x1000"])
                self.assertEqual(resolved["remaining_failures"], [f.capture["failures"][0]])
                self.assertTrue(any("adjudication" in x for x in resolved["verification_failures"]))

    def test_changed_review_or_evidence_cannot_adjudicate(self):
        for kind in ("review", "evidence"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                (f.review_path if kind == "review" else f.evidence_path).write_bytes(b"{}")
                resolved = self.resolve(f)
                self.assertEqual(resolved["conflicting_function_rvas"], ["0x1000"])
                self.assertTrue(resolved["remaining_failures"])

    def test_new_later_candidate_invalidates_review_instead_of_overwriting_it(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            later = f.result / "supplements/zz-later"
            manifest = json.loads((f.output / "capture.json").read_bytes())
            for record in manifest["functions"]:
                destination = later / record["path"]
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes((f.output / record["path"]).read_bytes())
            for name in ("capture.json", "plan.json", "session.json"):
                (later / name).write_bytes((f.output / name).read_bytes())
            resolved = self.resolve(f)
            self.assertEqual(resolved["conflicting_function_rvas"], ["0x1000"])
            self.assertTrue(any("candidate set changed" in x["error"] for x in resolved["verification_failures"]))

    def test_complete_machine_and_live_local_checks_are_required(self):
        for issue in ("prepost", "locals", "assembly", "typed"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "prepost": f.body["type_after_decompile"] = "int __cdecl()"
                elif issue == "locals": f.body["pseudocode"]["locals_capture"]["complete"] = False
                elif issue == "assembly": f.body["assembly_after_decompile"] = []
                else: f.body["pseudocode"]["locals"] = [{"name": "unknown", "complete": False}]
                record = f.write(f.output / "functions/1000.json", f.body)
                f.manifest["functions"][0]["sha256"] = record["sha256"]
                f.write(f.output / "capture.json", f.manifest)
                entry = f.review["functions"][0]
                entry["selected"] = adjudication.identity(analysis, f.output / "functions/1000.json")
                entry["candidates"][1] = {"function": entry["selected"],
                    "supplement": adjudication.identity(analysis, f.output / "capture.json")}
                old = json.loads((f.first / "functions/1000.json").read_bytes())
                entry["comparisons"][0]["differences"] = [dict(x, classification="capture_metadata_replacement",
                    reason="Synthetic reviewed field") for x in adjudication.differences(old, f.body)]
                f.write(f.review_path, f.review)
                f.review_record = adjudication.identity(analysis, f.review_path)
                resolved = self.resolve(f)
                self.assertTrue(resolved["remaining_failures"])
                self.assertEqual(resolved["conflicting_function_rvas"], ["0x1000"])

    def test_explicit_command_publishes_binding_then_rechecks_it_on_every_resolution(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            args = SimpleNamespace(input=f.capture["input"], review=f.review_record["path"],
                                   review_sha256=f.review_record["sha256"])
            with redirect_stdout(io.StringIO()): adjudication.run(analysis, args)
            resolved = analysis.native_reanalysis_resolution(f.result)
            self.assertFalse(resolved["remaining_failures"])
            self.assertFalse(resolved["verification_failures"])
            f.review_path.write_bytes(b"{}")
            resolved = analysis.native_reanalysis_resolution(f.result)
            self.assertTrue(resolved["remaining_failures"])
            self.assertEqual(resolved["conflicting_function_rvas"], ["0x1000"])

    def test_review_sha_and_different_existing_binding_cannot_publish(self):
        for issue in ("sha", "binding"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                args = SimpleNamespace(input=f.capture["input"], review=f.review_record["path"],
                                       review_sha256=f.review_record["sha256"])
                if issue == "sha": args.review_sha256 = "0" * 64
                else: (f.result / "reanalysis-adjudication.json").write_text("{}")
                with self.assertRaises(ValueError), redirect_stdout(io.StringIO()):
                    adjudication.run(analysis, args)

    def test_difference_inventory_preserves_missing_null_and_json_pointer_escaping(self):
        found = adjudication.differences({"a/b": {"~x": None}, "gone": None}, {"a/b": {"~x": 2}, "new": None})
        self.assertEqual([x["pointer"] for x in found], ["/a~1b/~0x", "/gone", "/new"])
        self.assertTrue(found[1]["previous"]["present"])
        self.assertFalse(found[1]["selected"]["present"])
        self.assertFalse(found[2]["previous"]["present"])
        self.assertTrue(found[2]["selected"]["present"])

    def attempt_fixture(self, root):
        owner = test_evidence.NativeSupplementTests()
        result, first, capture, manifest, body, write = owner.reanalysis_fixture(root)
        failed = result / "supplements/z-failed"
        old_body = copy.deepcopy(body)
        old_body["pseudocode"] = None
        old_record = write(failed / "functions/1000.json", old_body)
        attempt = copy.deepcopy(manifest)
        attempt["all_pseudocode_captured"] = False
        attempt["failures"] = [{"rva": "0x1000", "stage": "hexrays", "error": "original call analysis failure"}]
        attempt["functions"][0].update(sha256=old_record["sha256"], pseudocode_captured=False)
        write(failed / "capture.json", attempt)
        session = {"passed": True, "exit_code": 0, "finished_at_utc": "2026-10-26T00:00:00Z",
                   "target_execution": False, "capture_tool": manifest["capture_tool"]}
        write(failed / "session.json", session)
        plan = json.loads((first / "plan.json").read_bytes())
        plan.update(input=capture["input"], sha256=capture["input_sha256"], input_bytes=capture["input_bytes"],
                    target_execution=False)
        write(failed / "plan.json", plan)
        resolution = analysis.native_reanalysis_resolution(result)
        failure = resolution["verification_failures"][0]
        entry = adjudication.attempt_coverage(analysis, result, resolution, failure)
        evidence_path = root / ".analysis/results/coverage-review.json"
        write(evidence_path, {"scope": "every failed selection target and exact replacement reviewed"})
        review = {"schema_version": 1, "review_kind": "native_superseded_attempt_adjudication",
                  "source_input": adjudication.identity(analysis, root / capture["input"]),
                  "base_capture": resolution["base_capture"], "target_execution": False,
                  "raw_artifacts_modified": False, "evidence": [adjudication.identity(analysis, evidence_path)],
                  "attempts": [{**entry, "reason": "Every synthetic original target is independently complete"}]}
        review_path = root / ".analysis/results/attempt-review.json"
        write(review_path, review)
        return SimpleNamespace(result=result, first=first, failed=failed, capture=capture,
            attempt=attempt, session=session, plan=plan, old_body=old_body, write=write, review=review,
            review_path=review_path, review_record=adjudication.identity(analysis, review_path))

    def test_failed_attempt_is_historical_only_after_every_exact_target_is_covered(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.attempt_fixture(Path(directory))
            before = (f.failed / "capture.json").read_bytes()
            result = analysis.native_reanalysis_resolution(f.result, attempt_adjudication_record=f.review_record)
            self.assertFalse(result["verification_failures"])
            self.assertEqual(len(result["historical_verification_failures"]), 1)
            self.assertEqual(len(result["superseded_attempts"]), 1)
            self.assertEqual((f.failed / "capture.json").read_bytes(), before)
            self.assertTrue(analysis.native_reanalysis_resolution(f.result)["verification_failures"])

    def test_failed_attempt_with_changed_outputs_session_or_unknown_failure_is_not_hidden(self):
        for issue in ("unfinished", "outside", "machine", "new_failure", "missing_output", "wrong_input"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.attempt_fixture(Path(directory))
                if issue == "unfinished":
                    del f.session["finished_at_utc"]
                    f.write(f.failed / "session.json", f.session)
                elif issue == "outside":
                    f.attempt["failures"][0]["stage"] = "capture"
                    f.write(f.failed / "capture.json", f.attempt)
                elif issue == "machine":
                    f.old_body["assembly"][0]["bytes"] = "ffff"
                    record = f.write(f.failed / "functions/1000.json", f.old_body)
                    f.attempt["functions"][0]["sha256"] = record["sha256"]
                    f.write(f.failed / "capture.json", f.attempt)
                elif issue == "new_failure":
                    f.attempt["failures"].append({"rva": "0x1010", "stage": "hexrays", "error": "uncovered"})
                    f.write(f.failed / "capture.json", f.attempt)
                elif issue == "missing_output":
                    (f.failed / "functions/1000.json").unlink()
                else:
                    f.plan["sha256"] = "0" * 64
                    f.write(f.failed / "plan.json", f.plan)
                current = analysis.native_reanalysis_resolution(f.result)
                with self.assertRaises((ValueError, FileNotFoundError)):
                    adjudication.attempt_coverage(analysis, f.result, current, current["verification_failures"][0])
                result = analysis.native_reanalysis_resolution(f.result, attempt_adjudication_record=f.review_record)
                self.assertTrue(result["verification_failures"])
                self.assertNotIn("superseded_attempts", result)

    def test_failed_attempt_review_must_bind_all_outputs_replacements_and_original_errors(self):
        for issue in ("functions", "replacement", "original_failure", "reason", "duplicate"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.attempt_fixture(Path(directory))
                entry = f.review["attempts"][0]
                if issue == "functions": entry["functions"] = []
                elif issue == "replacement": entry["functions"][0]["replacement_function"]["sha256"] = "0" * 64
                elif issue == "original_failure": entry["original_failures"][0]["error"] = "ignored"
                elif issue == "reason": entry["reason"] = ""
                else: f.review["attempts"].append(copy.deepcopy(entry))
                f.write(f.review_path, f.review)
                f.review_record = adjudication.identity(analysis, f.review_path)
                result = analysis.native_reanalysis_resolution(f.result, attempt_adjudication_record=f.review_record)
                self.assertTrue(result["verification_failures"])
                self.assertNotIn("superseded_attempts", result)

    def test_published_attempt_review_does_not_suppress_a_new_failure(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.attempt_fixture(Path(directory))
            args = SimpleNamespace(input=f.capture["input"], review=f.review_record["path"],
                                   review_sha256=f.review_record["sha256"])
            with redirect_stdout(io.StringIO()): adjudication.run(analysis, args)
            self.assertFalse(analysis.native_reanalysis_resolution(f.result)["verification_failures"])
            another = f.result / "supplements/zz-new-failure"
            for name in ("capture.json", "session.json", "plan.json", "functions/1000.json"):
                destination = another / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes((f.failed / name).read_bytes())
            result = analysis.native_reanalysis_resolution(f.result)
            self.assertEqual(len(result["superseded_attempts"]), 1)
            self.assertEqual(len(result["verification_failures"]), 1)
            self.assertIn("zz-new-failure", result["verification_failures"][0]["supplement"])

    def test_pre_export_failure_needs_original_database_diagnostic_and_every_target(self):
        for issue in (None, "database", "diagnostic", "observation", "exit"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                f = self.attempt_fixture(root)
                source = root / ".work/native/source/original.i64"
                source.parent.mkdir(parents=True)
                source.write_bytes(b"synthetic unchanged original database")
                database = {"path": source.relative_to(root).as_posix(), "sha256": analysis.sha(source)}
                f.attempt.update(capture_complete=False, functions=[], failures=[], source_database=database)
                f.session.update(passed=False, exit_code=1)
                f.plan["source_database"] = database
                diagnostic_path = root / ".analysis/results/failed-analyzer.txt"
                diagnostic_path.write_bytes(b"[STATIC-SUPPLEMENT] fatal failure:\nTraceback\nValueError: original failure\n")
                diagnostic = adjudication.identity(analysis, diagnostic_path)
                if issue == "database": source.write_bytes(b"changed original database")
                elif issue == "diagnostic": diagnostic_path.write_bytes(b"empty success report")
                elif issue == "observation": f.attempt["function_selection"]["static_observation"] = {"required": True}
                elif issue == "exit": f.session["exit_code"] = None
                f.write(f.failed / "capture.json", f.attempt)
                f.write(f.failed / "session.json", f.session)
                f.write(f.failed / "plan.json", f.plan)
                resolved = analysis.native_reanalysis_resolution(f.result)
                failure = resolved["verification_failures"][0]
                if issue is None:
                    entry = adjudication.attempt_coverage(analysis, f.result, resolved, failure, diagnostic)
                    self.assertEqual(entry["capture_outcome"], "failed_before_export")
                    self.assertEqual(entry["functions"][0]["previous_function"]["path"],
                                     (f.result / "functions/1000.json").relative_to(root).as_posix())
                    f.review["attempts"] = [{**entry, "reason": "Exact original target fully covered after an ended failure"}]
                    f.write(f.review_path, f.review)
                    record = adjudication.identity(analysis, f.review_path)
                    resolved = analysis.native_reanalysis_resolution(f.result, attempt_adjudication_record=record)
                    self.assertFalse(resolved["verification_failures"])
                    self.assertEqual(len(resolved["historical_verification_failures"]), 1)
                else:
                    with self.assertRaises(ValueError):
                        adjudication.attempt_coverage(analysis, f.result, resolved, failure, diagnostic)

    def test_stage_type_refinement_requires_its_own_exact_review(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            f.body["type_after_decompile"] = "void __cdecl(void)"
            record = f.write(f.output / "functions/1000.json", f.body)
            f.manifest["functions"][0]["sha256"] = record["sha256"]
            f.write(f.output / "capture.json", f.manifest)
            entry = f.review["functions"][0]
            entry["selected"] = adjudication.identity(analysis, f.output / "functions/1000.json")
            entry["candidates"][1] = {"function": entry["selected"],
                "supplement": adjudication.identity(analysis, f.output / "capture.json")}
            old = json.loads((f.first / "functions/1000.json").read_bytes())
            entry["comparisons"][0]["differences"] = [dict(x, classification="capture_metadata_replacement",
                reason="Exact reviewed synthetic field") for x in adjudication.differences(old, f.body)]
            f.write(f.review_path, f.review)
            f.review_record = adjudication.identity(analysis, f.review_path)
            self.assertTrue(self.resolve(f)["remaining_failures"])
            entry["stage_differences"] = [dict(x, classification="observed_type_refinement",
                reason="The exact synthetic declarations have the same void argument contract") for x in
                adjudication.differences({"type": f.body["type_before_decompile"]}, {"type": f.body["type_after_decompile"]})]
            f.write(f.review_path, f.review)
            f.review_record = adjudication.identity(analysis, f.review_path)
            self.assertFalse(self.resolve(f)["remaining_failures"])

    def test_attempt_original_complete_target_uses_strict_base_verification(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.attempt_fixture(Path(directory))
            raw_body = json.loads((f.first / "functions/1000.json").read_bytes())
            record = f.write(f.result / "functions/1000.json", raw_body)
            f.capture["functions"][0].update(sha256=record["sha256"], assembly_captured=True, pseudocode_captured=True)
            f.capture["failures"] = []
            f.write(f.result / "capture.json", f.capture)
            capture_ref = {"path": (f.result / "capture.json").relative_to(Path(directory)).as_posix(),
                           "sha256": analysis.sha(f.result / "capture.json")}
            f.attempt["base_capture"] = capture_ref
            f.plan["base_capture"] = capture_ref
            f.write(f.failed / "capture.json", f.attempt)
            f.write(f.failed / "plan.json", f.plan)
            (f.first / "capture.json").unlink()
            resolved = analysis.native_reanalysis_resolution(f.result)
            self.assertFalse(resolved["complete_functions"])
            with patch.object(analysis, "verify_native_capture", return_value=(f.capture, capture_ref)) as verify:
                entry = adjudication.attempt_coverage(analysis, f.result, resolved, resolved["verification_failures"][0])
                self.assertEqual(verify.call_count, 1)
                self.assertEqual(entry["functions"][0]["replacement_supplement"], resolved["base_capture"])
            with patch.object(analysis, "verify_native_capture", side_effect=ValueError("changed base output")):
                with self.assertRaisesRegex(ValueError, "changed base output"):
                    adjudication.attempt_coverage(analysis, f.result, resolved, resolved["verification_failures"][0])

    def merged_attempt_fixture(self, root, *, partitioned=False):
        owner = test_evidence.NativeSupplementTests()
        if partitioned:
            result, complete, capture, manifest, body, _, write = owner.partition_reanalysis_fixture(root)
        else:
            result, complete, capture, manifest, body, write = owner.merge_reanalysis_fixture(root)
        failed = result / "supplements/failed-head"
        previous = json.loads((result / "functions/1000.json").read_bytes())
        record = write(failed / "functions/1000.json", previous)
        raw = (root / capture["input"]).read_bytes()
        selection = analysis.native_function_selection(raw, capture, [0x1000])
        attempt = copy.deepcopy(manifest)
        attempt.update(all_pseudocode_captured=False, function_selection=selection,
            function_merge_recoveries=[], failures=[{"rva": "0x1000", "stage": "hexrays",
                "error": "original truncated boundary failure"}],
            functions=[{**record, "path": "functions/1000.json", "address": previous["address"],
                "rva": "0x1000", "assembly_captured": True, "pseudocode_captured": False}])
        write(failed / "capture.json", attempt)
        write(failed / "session.json", {"passed": True, "exit_code": 0,
            "finished_at_utc": "2026-10-26T00:00:00Z", "target_execution": False,
            "capture_tool": manifest["capture_tool"]})
        write(failed / "plan.json", {"function_selection": selection, "base_capture": manifest["base_capture"],
            "source_database": manifest.get("source_database"), "input": capture["input"],
            "sha256": capture["input_sha256"], "input_bytes": capture["input_bytes"], "target_execution": False})
        return SimpleNamespace(result=result, complete=complete, failed=failed, previous=previous,
            capture=capture, manifest=manifest, body=body, write=write, raw=raw)

    def test_failed_old_head_is_covered_only_by_verified_exact_complete_merge(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.merged_attempt_fixture(Path(directory))
            before = (f.failed / "capture.json").read_bytes()
            resolution = analysis.native_reanalysis_resolution(f.result)
            failure = resolution["verification_failures"][0]
            entry = adjudication.attempt_coverage(analysis, f.result, resolution, failure)
            coverage = entry["functions"][0]["merged_original_coverage"]
            self.assertEqual(coverage["coverage_kind"], "reviewed_complete_function_merge")
            self.assertEqual(coverage["complete_merge"]["member_rvas"], ["0x1000", "0x1002", "0x1004"])
            self.assertEqual(coverage["original_head"]["path"],
                             (f.result / "functions/1000.json").relative_to(Path(directory)).as_posix())
            self.assertEqual((f.failed / "capture.json").read_bytes(), before)

    def test_merged_old_attempt_rejects_missing_members_altered_head_or_truncated_union(self):
        for issue in ("unreviewed", "member", "head", "union", "xref"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.merged_attempt_fixture(Path(directory))
                if issue == "unreviewed": f.manifest["function_selection"]["function_merges"] = []
                elif issue == "member": f.manifest["function_selection"]["function_merges"][0]["members"][1]["sha256"] = "0" * 64
                elif issue == "head": f.previous["chunks"][0]["raw_hex"] = "9090"
                elif issue == "union": f.body["assembly"].pop()
                else: f.body["xrefs_to_entry"] = []
                with self.assertRaises(ValueError):
                    adjudication.merged_attempt_coverage(analysis, f.raw, f.capture,
                        f.previous, f.body, f.manifest)

    def test_failed_original_body_requires_every_exact_partition_helper_for_coverage(self):
        for issue in (None, "missing", "duplicate", "bytes", "xref", "pseudocode", "union"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.merged_attempt_fixture(Path(directory), partitioned=True)
                helper_record = f.manifest["partition_helpers"][0]
                helper_path = f.complete / helper_record["path"]
                helper = json.loads(helper_path.read_bytes())
                if issue == "missing": f.manifest["partition_helpers"] = []
                elif issue == "duplicate": f.manifest["partition_helpers"] *= 2
                elif issue == "bytes": helper["assembly"][0]["bytes"] = "9090"
                elif issue == "xref": helper["xrefs_to_entry"] = []
                elif issue == "pseudocode": helper["pseudocode"] = {"lines": [{"text": "invented"}]}
                elif issue == "union": f.manifest["function_selection"]["function_partitions"][0]["owner_chunks"].pop()
                if issue in {"bytes", "xref", "pseudocode"}:
                    reference = f.write(helper_path, helper)
                    helper_record["sha256"] = reference["sha256"]
                if issue:
                    with self.assertRaises(ValueError):
                        adjudication.partitioned_attempt_coverage(analysis, f.raw, f.capture,
                            f.previous, f.body, f.manifest, f.complete)
                else:
                    resolution = analysis.native_reanalysis_resolution(f.result)
                    coverage = adjudication.attempt_coverage(analysis, f.result, resolution,
                        resolution["verification_failures"][0])["functions"][0]["partitioned_original_coverage"]
                    self.assertEqual(coverage["coverage_kind"], "reviewed_complete_function_partition")
                    self.assertEqual(len(coverage["complete_helper_assemblies"]), 1)
                    self.assertFalse(coverage["helper_semantics_or_abi_completion"])

    def test_binding_extension_requires_exact_previous_binding_and_retained_snapshot(self):
        for issue in (None, "previous", "snapshot"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.attempt_fixture(Path(directory))
                args = SimpleNamespace(input=f.capture["input"], review=f.review_record["path"], review_sha256=f.review_record["sha256"])
                with redirect_stdout(io.StringIO()): adjudication.run(analysis, args)
                binding = f.result / "reanalysis-attempt-adjudication.json"
                snapshot = Path(directory) / ".analysis/results/previous-binding.json"
                snapshot.write_bytes(binding.read_bytes())
                f.review["previous_binding"] = adjudication.identity(analysis, binding)
                f.review["previous_binding_snapshot"] = adjudication.identity(analysis, snapshot)
                if issue == "previous": f.review["previous_binding"]["sha256"] = "0" * 64
                elif issue == "snapshot": snapshot.write_bytes(b"{}")
                f.write(f.review_path, f.review)
                args.review_sha256 = analysis.sha(f.review_path)
                if issue is None:
                    with redirect_stdout(io.StringIO()): adjudication.run(analysis, args)
                    self.assertEqual(json.loads(binding.read_bytes())["previous_binding_snapshot"], f.review["previous_binding_snapshot"])
                    self.assertFalse(analysis.native_reanalysis_resolution(f.result)["verification_failures"])
                    with redirect_stdout(io.StringIO()): adjudication.run(analysis, args)
                else:
                    with self.assertRaises(ValueError), redirect_stdout(io.StringIO()): adjudication.run(analysis, args)


if __name__ == "__main__":
    unittest.main()

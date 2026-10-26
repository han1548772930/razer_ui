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
    def test_nested_inline_mapping_accepts_real_or_fictitious_ea_only_in_exact_sdk_new_blocks(self):
        for fictitious in (False, True):
            for issue in (None, "mapping", "fictitious", "scope", "owner", "source", "helper", "interval",
                          "outside", "block", "duplicate", "missing", "api", "bool"):
                with self.subTest(fictitious=fictitious, issue=issue):
                    outer = {"parent_rva": "0x1000", "target_rva": "0x2000", "callsite_rva": "0x1010",
                             "raw_hex": "e8eb0f0000", "parent_function": {"path": "retained/owner.json"}}
                    expected = {"callsite_rva": "0x2020", "call_path": [outer, {}]}
                    scope = {"api": "Hexrays_Hooks.inlined_func", "mba_entry_ea": "0x140001000",
                             "source_block": 4, "helper_ea": "0x140002000", "inserted_block_interval": [28, 37]}
                    parent = {**outer, "block_serial_before": 4, "new_block_scope": copy.deepcopy(scope)}
                    call = {"sdk_call_ea": "0xf0000020" if fictitious else "0x140002020",
                        "mapped_original_call_ea": "0x140002020", "mapping_api": "mba_t.map_fict_ea",
                        "mapping_status": "api_returned", "fictitious_call": fictitious,
                        "source_inline_scope": copy.deepcopy(scope), "sdk_block_serial": 34, "block_serial_before": 34}
                    calls = [parent, call]
                    if issue == "mapping": call["mapped_original_call_ea"] = "0x140002024"
                    elif issue == "fictitious": call["fictitious_call"] = not fictitious
                    elif issue == "scope": call["source_inline_scope"]["inserted_block_interval"] = [20, 37]
                    elif issue in {"owner", "source", "helper", "interval", "api", "bool"}:
                        key, value = {"owner": ("mba_entry_ea", "0x140009000"), "source": ("source_block", 6),
                            "helper": ("helper_ea", "0x140007000"), "interval": ("inserted_block_interval", [37, 28]),
                            "api": ("api", "guessed_range"), "bool": ("inserted_block_interval", [True, 37])}[issue]
                        call["source_inline_scope"][key] = parent["new_block_scope"][key] = value
                    elif issue == "outside": call["sdk_block_serial"] = call["block_serial_before"] = 37
                    elif issue == "block": call["block_serial_before"] = 33
                    elif issue == "duplicate": calls.insert(0, copy.deepcopy(parent))
                    elif issue == "missing": calls.remove(parent)
                    if issue is None:
                        analysis.verify_nested_inline_mapping({"image_base": "0x140000000"}, "0x1000", expected, call, calls)
                    else:
                        with self.assertRaises(ValueError):
                            analysis.verify_nested_inline_mapping({"image_base": "0x140000000"}, "0x1000", expected, call, calls)

    def finalization_fixture(self, root):
        result, output, capture, manifest, body, write = test_evidence.NativeSupplementTests().reanalysis_fixture(root)
        body["pseudocode"]["locals_capture"]["schema_version"] = 2
        record = write(output / "functions/1000.json", body)
        manifest["functions"][0]["sha256"] = record["sha256"]
        source_db = root / ".work/native/source.i64"
        source_db.parent.mkdir(parents=True)
        source_db.write_bytes(b"inert static database fixture")
        source = {"path": source_db.relative_to(root).as_posix(), "sha256": analysis.sha(source_db)}
        private = {"path": ".work/native/copy.i64", "sha256": analysis.sha(source_db)}
        serializer = root / "tools/ida/capture_values.py"
        serializer.parent.mkdir(parents=True)
        serializer.write_bytes(b"# inert serializer source fixture\n")
        manifest.update(capture_complete=False, all_pseudocode_captured=False, real_device_operations=False,
            source_database=source, database=private, typed_value_serializer=adjudication.identity(analysis, serializer),
            database_integrity={"verified": True, "records_with_issues": 0},
            chains=[{"requested_rva": "0x1000", "complete": True,
                "entries": [{"address": body["address"], "rva": "0x1000", "direct_jump_target_rva": None}]}])
        manifest.pop("base_capture_unchanged")
        manifest.pop("base_database_unchanged")
        write(output / "capture.json", manifest)
        plan = {"input": capture["input"], "input_bytes": capture["input_bytes"], "sha256": capture["input_sha256"],
            "base_capture": manifest["base_capture"], "function_selection": manifest["function_selection"],
            "source_database": source, "database": private, "target_execution": False, "real_device_operations": False}
        write(output / "plan.json", plan)
        session = {"passed": False, "exit_code": 1, "finished_at_utc": "2026-10-26T00:00:00Z", "pid": 1234567,
            "capture_tool": manifest["capture_tool"], "database": private, "target_execution": False,
            "real_device_operations": False}
        write(output / "session.json", session)
        diagnostic = root / ".analysis/results/diagnostic.log"
        diagnostic.write_bytes(b"[STATIC-SUPPLEMENT] fatal failure:\nTraceback (most recent call last):\n"
            b"  File capture_values.py, in source_identity\n"
            b"ValueError: Typed capture serializer changed after it was imported\n")
        return SimpleNamespace(result=result, output=output, manifest=manifest, capture=capture,
            body=body, plan=plan, session=session, source_db=source_db, write=write,
            diagnostic=adjudication.identity(analysis, diagnostic))

    def test_finalization_independently_checks_full_exports_without_changing_failed_session(self):
        import reverse_pending
        for issue in (None, "active", "exit", "runtime", "failure", "incomplete_chain", "duplicate",
                      "function_bytes", "original_database", "missing_serializer", "missing_locals",
                      "wrong_diagnostic", "incomplete_typed"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)), \
                    patch.object(reverse_pending, "process_alive", return_value=issue == "active"):
                fixture = self.finalization_fixture(Path(directory))
                f = fixture
                if issue == "exit": f.session["exit_code"] = 0
                elif issue == "runtime": f.session["target_execution"] = True
                elif issue == "failure": f.manifest["failures"] = [{"stage": "hexrays", "error": "failed"}]
                elif issue == "incomplete_chain": f.manifest["chains"][0]["complete"] = False
                elif issue == "duplicate": f.manifest["functions"] *= 2
                elif issue == "function_bytes": (f.output / "functions/1000.json").write_bytes(b"changed")
                elif issue == "original_database": f.source_db.write_bytes(b"changed")
                elif issue == "missing_serializer": f.manifest.pop("typed_value_serializer")
                elif issue in {"missing_locals", "incomplete_typed"}:
                    if issue == "missing_locals": f.body["pseudocode"].pop("locals_capture")
                    else: f.body["pseudocode"]["locals_capture"]["complete"] = False
                    record = f.write(f.output / "functions/1000.json", f.body)
                    f.manifest["functions"][0]["sha256"] = record["sha256"]
                elif issue == "wrong_diagnostic":
                    (Path(directory) / f.diagnostic["path"]).write_bytes(b"another fatal failure")
                    f.diagnostic = adjudication.identity(analysis, Path(directory) / f.diagnostic["path"])
                f.write(f.output / "capture.json", f.manifest)
                f.write(f.output / "session.json", f.session)
                if issue not in {None, "incomplete_typed"}:
                    with self.assertRaises(ValueError):
                        adjudication.finalization_coverage(analysis, f.result, f.output / "capture.json", f.diagnostic)
                    continue
                original_session = (f.output / "session.json").read_bytes()
                original_capture = (f.output / "capture.json").read_bytes()
                coverage = adjudication.finalization_coverage(analysis, f.result, f.output / "capture.json", f.diagnostic)
                self.assertEqual(coverage["functions"][0]["typed_validation"]["complete"], issue is None)
                review = {"schema_version": 1, "review_kind": "native_completed_export_finalization_review",
                    "target_execution": False, "raw_artifacts_modified": False,
                    "historical_session_outcome_preserved": True, "reason": "Complete inert fixture observations reviewed",
                    "evidence": [f.diagnostic], "finalization": coverage}
                review_path = Path(directory) / ".analysis/results/finalization.json"
                f.write(review_path, review)
                args = SimpleNamespace(review=analysis.rel(review_path), review_sha256=analysis.sha(review_path),
                                       input=f.capture["input"])
                with redirect_stdout(io.StringIO()): adjudication.finalize(analysis, args)
                resolved = analysis.native_reanalysis_resolution(f.result)
                self.assertEqual(len(resolved["resolved_failures"]), 1 if issue is None else 0)
                self.assertEqual(len(resolved["remaining_failures"]), 0 if issue is None else 1)
                self.assertEqual((f.output / "session.json").read_bytes(), original_session)
                self.assertEqual((f.output / "capture.json").read_bytes(), original_capture)
                self.assertFalse(resolved["supplements"][0]["independent_export_finalization"]["original_session_passed"])
                changed = copy.deepcopy(review)
                changed["finalization"]["functions"][0]["function"]["sha256"] = "0" * 64
                f.write(review_path, changed)
                with self.assertRaises(ValueError):
                    adjudication.finalized_export(analysis, f.result, f.output / "capture.json")

    def test_exception_boundary_attempt_preserves_old_owner_and_full_helper_with_independent_rechecks(self):
        import native_boundaries
        import test_native_boundaries
        for issue in (None, "missing_helper", "helper_bytes", "helper_record", "old_bytes", "owner_bytes", "plan", "sdk"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                helper = test_native_boundaries.NativeBoundaryTests()
                fixture = helper.fixture(Path(directory))
                row = helper.select(fixture)[0]
                sdk, _ = helper.sdk(fixture, row)
                applied = native_boundaries.apply(row, fixture.base, sdk)
                output = fixture.result / "supplements/complete"
                body = native_boundaries.helper_body(row)
                body["pseudocode"] = None
                record = fixture.write(output / "helpers/1040.json", body)
                record.update(path="helpers/1040.json", rva="0x1040", assembly_captured=True, pseudocode_captured=False)
                manifest = {"function_selection": {"targets": [{"rva": "0x1000"}, {"rva": "0x1010"}],
                    "function_boundary_recoveries": [row]}, "function_boundary_recoveries": [applied],
                    "boundary_helpers": [record]}
                previous = copy.deepcopy(fixture.bodies["0x1000"])
                replacement = native_boundaries.owner_body(analysis, row)
                if issue == "missing_helper": manifest["boundary_helpers"] = []
                elif issue == "helper_bytes":
                    body["assembly"].pop()
                    changed = fixture.write(output / "helpers/1040.json", body)
                    record["sha256"] = changed["sha256"]
                elif issue == "helper_record": record["pseudocode_captured"] = True
                elif issue == "old_bytes": previous["chunks"][-1]["raw_hex"] = "90c3"
                elif issue == "owner_bytes": replacement["assembly"][0]["bytes"] = "e93b000000"
                elif issue == "plan": row["owner_chunks"][0]["raw_hex"] = "c3"
                elif issue == "sdk": applied["tail_removed"] = False
                if issue is None:
                    result = adjudication.boundary_attempt_coverage(analysis, fixture.raw, fixture.capture,
                        previous, replacement, manifest, output)
                    self.assertEqual(result["coverage_kind"], "reviewed_exception_chain_boundary_recovery")
                    self.assertTrue(result["all_original_bytes_preserved"])
                    self.assertFalse(result["helper_semantics_or_abi_completion"])
                else:
                    with self.assertRaises(ValueError):
                        adjudication.boundary_attempt_coverage(analysis, fixture.raw, fixture.capture,
                            previous, replacement, manifest, output)

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

    def test_instruction_attempt_proof_preserves_full_bytes_and_exact_cpu_recovery(self):
        import native_boundaries
        import test_native_boundaries
        for issue in (None, "old_machine", "replacement", "review", "sdk", "duplicate"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                helper = test_native_boundaries.NativeBoundaryTests()
                f = helper.instruction_fixture(Path(directory))
                row = helper.instruction_select(f)[0]
                sdk, _ = helper.instruction_sdk(f, row)
                applied = native_boundaries.apply_instructions(row, f.base, sdk)
                old = copy.deepcopy(f.bodies["0x1000"])
                selected = native_boundaries.instruction_body(analysis, row)
                manifest = {"function_selection": {"targets": [{"rva": "0x1000"}],
                    "instruction_boundary_recoveries": [row]}, "instruction_boundary_recoveries": [applied]}
                if issue == "old_machine": old["assembly"][0]["bytes"] = "90"
                elif issue == "replacement": selected["assembly"].pop()
                elif issue == "review": row["instruction"]["bytes"] = "e800000000"
                elif issue == "sdk": applied["exact_e8_created"] = False
                elif issue == "duplicate": manifest["function_selection"]["instruction_boundary_recoveries"].append(copy.deepcopy(row))
                if issue is None:
                    proof = adjudication.instruction_attempt_coverage(analysis, f.raw, f.capture, old, selected, manifest)
                    self.assertTrue(proof["all_original_bytes_preserved"])
                    self.assertTrue(proof["original_decode_failure_preserved"])
                else:
                    with self.assertRaises(ValueError):
                        adjudication.instruction_attempt_coverage(analysis, f.raw, f.capture, old, selected, manifest)

    def entry_partition_attempt_fixture(self, root):
        import native_boundaries
        import test_native_boundaries

        owner = test_native_boundaries.X86EntryPartitionTests()
        f = owner.fixture(root)
        f.requests = owner.select(f)
        sdk, _ = owner.sdk(f)
        applied = native_boundaries.apply_entry_partition(f.requests[0], f.base, sdk, lambda row: None)
        f.output = f.result / "supplements/complete"
        f.previous = copy.deepcopy(f.bodies[0])
        pieces = native_boundaries.entry_partition_bodies(analysis, f.requests[0])
        f.selected, f.helper_bodies, records = pieces[0], pieces[1:], []
        for body in f.helper_bodies:
            body["pseudocode"] = {"lines": [{"text": "complete typed fixture"}], "locals": []}
            test_evidence.NativeSupplementTests.typed_fixture(body)
            name = "entry-partition-helpers/" + body["rva"][2:] + ".json"
            identity = f.write(f.output / name, body)
            records.append({**identity, "path": name, "rva": body["rva"],
                            "assembly_captured": True, "pseudocode_captured": True})
        f.manifest = {"function_selection": {"targets": [{"rva": hex(f.first)}],
            "function_outlines": [{"rva": "0x1120"}], "x86_entry_partitions": f.requests},
            "x86_entry_partition_recoveries": [applied], "entry_partition_helpers": records}
        return f

    def test_entry_partition_attempt_requires_complete_bytes_edges_typed_helpers_and_exact_target(self):
        for issue in (None, "missing_partition", "missing_piece", "piece_bytes", "sdk", "review_edge",
                      "wrong_target", "wrong_owner", "old_bytes", "old_edge", "owner_bytes", "owner_edge",
                      "missing_helper", "duplicate_helper", "helper_identity", "helper_bytes", "helper_entry",
                      "helper_edge", "helper_typed", "helper_pseudocode", "helper_target"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.entry_partition_attempt_fixture(Path(directory))
                row = f.requests[0]
                record = f.manifest["entry_partition_helpers"][0]
                body = f.helper_bodies[0]
                if issue == "missing_partition": f.manifest["function_selection"]["x86_entry_partitions"] = []
                elif issue == "missing_piece": row["pieces"].pop()
                elif issue == "piece_bytes": row["pieces"][1]["chunks"][0]["raw_hex"] = "c3"
                elif issue == "sdk": f.manifest["x86_entry_partition_recoveries"][0]["auto_wait_completed"] = False
                elif issue == "review_edge": row["external_incoming"] = []
                elif issue == "wrong_target": f.manifest["function_selection"]["targets"] = [{"rva": "0x1140"}]
                elif issue == "wrong_owner": row["owner_rva"] = "0x1140"
                elif issue == "old_bytes": f.previous["assembly"][0]["bytes"] = "90"
                elif issue == "old_edge": f.previous["assembly"][6]["xrefs_to"] = []
                elif issue == "owner_bytes": f.selected["assembly"].pop()
                elif issue == "owner_edge": f.selected["assembly"][0]["xrefs_from"] = [{"from": "0x10001000"}]
                elif issue == "missing_helper": f.manifest["entry_partition_helpers"].pop()
                elif issue == "duplicate_helper": f.manifest["entry_partition_helpers"].append(copy.deepcopy(record))
                elif issue == "helper_identity": record["sha256"] = "0" * 64
                elif issue == "helper_bytes": body["assembly"][0]["bytes"] = "90"
                elif issue == "helper_entry": body["xrefs_to_entry"] = []
                elif issue == "helper_edge": body["assembly"][0]["xrefs_to"] = []
                elif issue == "helper_typed": body.pop("type_before_decompile")
                elif issue == "helper_pseudocode": body["pseudocode"]["lines"] = []
                elif issue == "helper_target": record["rva"] = "0x1140"
                if issue in {"helper_bytes", "helper_entry", "helper_edge", "helper_typed", "helper_pseudocode"}:
                    identity = f.write(f.output / record["path"], body)
                    record.update(bytes=identity["bytes"], sha256=identity["sha256"])
                if issue is None:
                    proof = adjudication.entry_partition_attempt_coverage(analysis, f.raw, f.capture,
                        f.previous, f.selected, f.manifest, f.output)
                    self.assertEqual(proof["coverage_kind"], "reviewed_complete_x86_entry_partition")
                    self.assertEqual(len(proof["complete_typed_entry_helpers"]), 2)
                    self.assertTrue(proof["all_original_bytes_preserved"])
                    self.assertTrue(proof["all_original_edges_preserved"])
                    self.assertFalse(proof["helper_semantics_or_abi_completion"])
                    self.assertFalse(adjudication._same_machine(f.previous, f.selected))
                else:
                    with self.assertRaises((ValueError, KeyError)):
                        adjudication.entry_partition_attempt_coverage(analysis, f.raw, f.capture,
                            f.previous, f.selected, f.manifest, f.output)

    def test_attempt_dispatch_keeps_x86_entry_partition_union_separate_from_complete_merges(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.entry_partition_attempt_fixture(Path(directory))
            failed = f.result / "supplements/failed"
            record = f.write(failed / "functions/1000.json", f.previous)
            base = adjudication.identity(analysis, f.result / "capture.json")
            base_reference = {key: base[key] for key in ("path", "sha256")}
            selection = {"targets": [{"rva": hex(f.first)}]}
            attempt = {"capture_complete": True, "all_pseudocode_captured": False,
                "input_sha256": f.capture["input_sha256"], "base_capture": base_reference,
                "target_execution": False, "capture_tool": {"path": "inert-fixture"},
                "function_selection": selection, "database_integrity": {"verified": True},
                "base_capture_unchanged": True, "base_database_unchanged": True,
                "functions": [{**record, "path": "functions/1000.json", "rva": hex(f.first),
                    "assembly_captured": True, "pseudocode_captured": False}],
                "failures": [{"rva": hex(f.first), "stage": "hexrays", "error": "original full-owner failure"}]}
            capture = f.write(failed / "capture.json", attempt)
            f.write(failed / "session.json", {"passed": True, "exit_code": 0,
                "finished_at_utc": "2026-10-27T00:00:00Z", "target_execution": False,
                "capture_tool": attempt["capture_tool"]})
            f.write(failed / "plan.json", {"base_capture": base_reference, "function_selection": selection,
                "source_database": None, "input": f.capture["input"], "sha256": f.capture["input_sha256"],
                "input_bytes": f.capture["input_bytes"], "target_execution": False})
            selected = f.write(f.output / "functions/1000.json", f.selected)
            complete = f.write(f.output / "capture.json", f.manifest)
            f.selected["capture"] = selected
            resolution = {"base_capture": base, "complete_functions": {hex(f.first): f.selected},
                          "supplements": [{"receipt": complete}], "conflicting_functions": {}}
            proof = adjudication.attempt_coverage(analysis, f.result, resolution, {"supplement": capture["path"]})
            self.assertIn("entry_partition_original_coverage", proof["functions"][0])
            self.assertNotIn("merged_original_coverage", proof["functions"][0])
            self.assertTrue(proof["functions"][0]["entry_partition_original_coverage"]["all_original_bytes_preserved"])

    def test_preexport_instruction_failure_needs_full_scope_source_and_verified_replacement(self):
        import native_boundaries
        import test_native_boundaries
        for issue in (None, "scope", "old_review", "unreviewed_outline", "missing_replacement",
                      "replacement_bytes", "replacement_manifest", "sdk", "observation", "prepass"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                helper = test_native_boundaries.NativeBoundaryTests()
                f = helper.instruction_fixture(root)
                row = helper.instruction_select(f)[0]
                sdk, _ = helper.instruction_sdk(f, row)
                applied = native_boundaries.apply_instructions(row, f.base, sdk)
                selected = native_boundaries.instruction_body(analysis, row)
                output = f.result / "supplements/complete"
                function = f.write(output / "functions/1000.json", selected)
                selected["capture"] = function
                manifest = {"function_selection": {"targets": [{"rva": "0x1000"}],
                    "instruction_boundary_recoveries": [copy.deepcopy(row)]},
                    "instruction_boundary_recoveries": [applied]}
                manifest_record = f.write(output / "capture.json", manifest)
                resolution = {"complete_functions": {"0x1000": selected},
                              "supplements": [{"receipt": manifest_record}]}
                failed_selection = copy.deepcopy(manifest["function_selection"])
                if issue == "scope": failed_selection["targets"].append({"rva": "0x1040"})
                elif issue == "old_review": failed_selection["instruction_boundary_recoveries"][0]["instruction"]["bytes"] = "e800000000"
                elif issue == "unreviewed_outline": failed_selection["function_outlines"] = [{"rva": "0x1040"}]
                elif issue == "missing_replacement": resolution["complete_functions"] = {}
                elif issue == "replacement_bytes": selected["assembly"][0]["bytes"] = "90"
                elif issue == "replacement_manifest": resolution["supplements"] = []
                elif issue == "sdk":
                    manifest["instruction_boundary_recoveries"][0]["exact_e8_created"] = False
                    resolution["supplements"][0]["receipt"] = f.write(output / "capture.json", manifest)
                elif issue == "observation": failed_selection["static_observation"] = {"required": True}
                elif issue == "prepass": failed_selection["decompiler_prepasses"] = [{"rva": "0x1000", "reason": "unproved"}]
                if issue is None:
                    proof = adjudication.preexport_instruction_scope(analysis, f.raw, f.capture, failed_selection, resolution)
                    self.assertEqual(len(proof), 1)
                    self.assertTrue(proof[0]["complete_replacement_proof"]["all_original_bytes_preserved"])
                else:
                    with self.assertRaises((ValueError, KeyError)):
                        adjudication.preexport_instruction_scope(analysis, f.raw, f.capture, failed_selection, resolution)

    def resolve(self, fixture):
        return analysis.native_reanalysis_resolution(fixture.result, adjudication_record=fixture.review_record)

    def frame_failure_fixture(self, root):
        import native_boundaries
        import test_native_boundaries

        helper = test_native_boundaries.NativeFrameReconstructionTests()
        f = helper.fixture(root)
        observer = root / ".analysis/results/observer.py"
        observer.write_bytes(b"# inert retained static observer fixture\n")
        observation_path = root / ".analysis/results/observation.json"
        f.write(observation_path, {"observation": {"reason": "Retain exact original frame observations.",
                  "observer_source": adjudication.identity(analysis, observer)}})
        selection = analysis.native_function_selection(f.raw, f.capture, [0x1000],
            type_note="Complete unchanged original owner evidence for a retained derivation.",
            observation_evidence=analysis.rel(observation_path) + "#/observation",
            frame_reviews=[f.specification], decompiler_prepass_rvas=["0x1000"])
        request = selection["frame_rebuilds"][0]
        sdk, _, _ = helper.sdk(f)
        stages = []
        applied = native_boundaries.apply_frame_rebuild(request, f.base, sdk,
                    lambda row: stages.append(copy.deepcopy(row)))
        failed_stage = stages[1]
        failed_stage["operation"] = "processor_t.create_func_frame"
        validator = root / "tools/native_boundaries.py"
        validator.parent.mkdir(parents=True)
        validator.write_bytes(b"# inert exact earlier processor binding fixture\n"
                             b"result = int(sdk.idp.processor_t.create_func_frame(function))\n")
        diagnostic = root / ".analysis/results/failed-frame.log"
        diagnostic.write_bytes(b"[STATIC-SUPPLEMENT] fatal failure:\nTraceback (most recent call last):\n"
            b"  File native_boundaries.py, in apply_frame_rebuild\n"
            b"    result = int(sdk.idp.processor_t.create_func_frame(function))\n"
            b"AttributeError: type object 'processor_t' has no attribute 'create_func_frame'\n")
        output = f.result / "supplements/complete"
        selected = copy.deepcopy(f.bodies["0x1000"])
        selected["capture"] = f.write(output / "functions/1000.json", selected)
        complete = {"function_selection": copy.deepcopy(selection), "frame_rebuilds": [applied]}
        complete_record = f.write(output / "capture.json", complete)
        manifest = {"function_selection": selection, "frame_rebuilds": [failed_stage],
            "real_device_operations": False, "database_integrity": {"verified": True},
            "boundary_validator": adjudication.identity(analysis, validator),
            "capture_tool": {"path": "tools/ida/supplement.py", "sha256": analysis.sha(observer)}}
        resolution = {"base_capture": adjudication.identity(analysis, f.result / "capture.json"),
            "complete_functions": {"0x1000": selected}, "conflicting_functions": {},
            "supplements": [{"receipt": complete_record}]}
        return SimpleNamespace(**vars(f), manifest=manifest, resolution=resolution, selected=selected,
            complete=complete, output=output, diagnostic=diagnostic, validator=validator)

    def test_preexport_frame_binding_needs_original_checkpoint_and_actual_complete_processor_replacement(self):
        for issue in (None, "scope", "review", "observer", "source", "diagnostic", "stage", "checkpoint",
                      "comment", "stack", "flags", "frame", "manual", "missing", "replacement_bytes",
                      "replacement_identity", "receipt", "cpu", "marker", "prepass", "operations", "integrity"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.frame_failure_fixture(Path(directory))
                selection, stage = f.manifest["function_selection"], f.manifest["frame_rebuilds"][0]
                if issue == "scope": selection["function_outlines"] = [{"rva": "0x1000"}]
                elif issue == "review": selection["frame_rebuilds"][0]["original_frame"]["members"][0]["comment"] = "changed"
                elif issue == "observer": f.manifest["capture_tool"]["sha256"] = "0" * 64
                elif issue == "source": f.validator.write_bytes(b"# a different actual binding\n")
                elif issue == "diagnostic": f.diagnostic.write_bytes(b"fatal failure\nTraceback\nValueError: unrelated\n")
                elif issue == "stage": stage["completed"] = True
                elif issue == "checkpoint": stage["processor_result"] = 1
                elif issue == "comment": stage["frame_before"]["members"][0]["comment"] = "discarded"
                elif issue == "stack": stage["before"]["stack_points"].pop()
                elif issue == "flags": stage["before"]["flags"] += 1
                elif issue == "frame": stage["before"]["native_frame"]["fpd"] += 1
                elif issue == "manual": stage["manual_frame_values_applied"] = True
                elif issue == "missing": f.resolution["complete_functions"] = {}
                elif issue == "replacement_bytes": f.selected["assembly"][0]["bytes"] = "cc"
                elif issue == "replacement_identity": f.selected["capture"]["sha256"] = "0" * 64
                elif issue == "receipt": f.resolution["supplements"] = []
                elif issue == "cpu": f.complete["frame_rebuilds"][0]["processor_result"] = 0
                elif issue == "marker": f.complete["frame_rebuilds"][0]["frame_after"]["members"][0]["return_address"] = False
                elif issue == "prepass": f.complete["function_selection"]["decompiler_prepasses"] = []
                elif issue == "operations": f.manifest["real_device_operations"] = True
                elif issue == "integrity": f.manifest["database_integrity"]["verified"] = False
                if issue in ("cpu", "marker", "prepass"):
                    f.resolution["supplements"][0]["receipt"] = f.write(f.output / "capture.json", f.complete)
                diagnostic = adjudication.identity(analysis, f.diagnostic)
                if issue is None:
                    proof = adjudication.preexport_frame_scope(analysis, f.raw, f.capture, f.manifest,
                                                               f.resolution, diagnostic)
                    self.assertTrue(proof["original_binding_failure_preserved"])
                    self.assertFalse(proof["original_failed_frame_restored_or_completed"])
                    self.assertFalse(proof["failed_processor_stage"]["completed"])
                    self.assertEqual(proof["complete_replacement_proof"]["completed_processor_reconstruction"]["processor_result"], 1)
                else:
                    with self.assertRaises((ValueError, KeyError)):
                        adjudication.preexport_frame_scope(analysis, f.raw, f.capture, f.manifest,
                                                          f.resolution, diagnostic)

    def switch_failure_fixture(self, root):
        import native_callsites
        import test_native_callsites

        helper = test_native_callsites.NativeSwitchMetadataTests()
        f = helper.fixture(root)
        selection = analysis.native_function_selection(f.raw, f.capture, [0x1000], switch_reviews=[f.specification])
        applied = native_callsites.apply_switch(selection["switch_metadata_recoveries"][0], f.base,
                                               helper.sdk(f, selection["switch_metadata_recoveries"][0]))
        output = f.result / "supplements/complete"
        selected = copy.deepcopy(f.body)
        selected["capture"] = f.write(output / "functions/1000.json", selected)
        complete = {"function_selection": copy.deepcopy(selection), "switch_metadata_recoveries": [applied]}
        record = f.write(output / "capture.json", complete)
        resolution = {"complete_functions": {"0x1000": selected}, "supplements": [{"receipt": record}]}
        diagnostic = root / ".analysis/results/failed-switch.log"
        diagnostic.write_bytes(b"[STATIC-SUPPLEMENT] fatal failure:\nTraceback (most recent call last):\n"
            b"  native_callsites.apply_switch(row, base, sdk)\n  File native_callsites.py, in apply_switch\n"
            b"ValueError: SDK switch metadata readback differs from its exact reviewed request\n")
        manifest = {"function_selection": selection, "real_device_operations": False, "switch_metadata_recoveries": [],
            "callsite_type_validator": {"path": "tools/native_callsites.py", "bytes": 100, "sha256": "a" * 64}}
        return SimpleNamespace(**vars(f), output=output, manifest=manifest, complete=complete,
                               selected=selected, resolution=resolution, diagnostic=diagnostic)

    def test_preexport_switch_failure_preserves_missing_old_readback_and_requires_every_new_case_and_xref(self):
        for issue in (None, "scope", "review", "mask", "table", "target", "default", "stages", "diagnostic",
                      "validator", "missing", "identity", "receipt", "bytes", "readback", "cases", "xrefs",
                      "frame", "manual", "replacement_review", "operations"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.switch_failure_fixture(Path(directory))
                selection = f.manifest["function_selection"]
                row = selection["switch_metadata_recoveries"][0]
                actual = f.complete["switch_metadata_recoveries"][0]
                if issue == "scope": selection["decompiler_prepasses"] = [{"rva": "0x1000"}]
                elif issue == "review": row["review_source"]["sha256"] = "0" * 64
                elif issue == "mask": row["mask"] = 3
                elif issue == "table": row["table"]["raw_hex"] = "00" * 8
                elif issue == "target": row["case_targets"][0]["target_rva"] = "0x1040"
                elif issue == "default": row["default_policy"] = "skip_default"
                elif issue == "stages": f.manifest["switch_metadata_recoveries"] = [copy.deepcopy(actual)]
                elif issue == "diagnostic": f.diagnostic.write_bytes(b"fatal failure\nTraceback\nValueError: unrelated\n")
                elif issue == "validator": f.manifest["callsite_type_validator"]["sha256"] = "not_a_hash"
                elif issue == "missing": f.resolution["complete_functions"] = {}
                elif issue == "identity": f.selected["capture"]["sha256"] = "0" * 64
                elif issue == "receipt": f.resolution["supplements"] = []
                elif issue == "bytes": f.selected["assembly"][0]["bytes"] = "cc"
                elif issue == "readback": actual["switch_metadata_readback_matches"] = False
                elif issue == "cases": actual["sdk_case_targets"].pop()
                elif issue == "xrefs": actual["sdk_xref_targets"].pop()
                elif issue == "frame": actual["after"]["native_frame"]["fpd"] += 1
                elif issue == "manual": actual["manual_stack_points_modified"] = True
                elif issue == "replacement_review": f.complete["function_selection"]["switch_metadata_recoveries"][0]["mask"] = 3
                elif issue == "operations": f.manifest["real_device_operations"] = True
                if issue in ("readback", "cases", "xrefs", "frame", "manual", "replacement_review"):
                    f.resolution["supplements"][0]["receipt"] = f.write(f.output / "capture.json", f.complete)
                diagnostic = adjudication.identity(analysis, f.diagnostic)
                if issue is None:
                    proof = adjudication.preexport_switch_scope(analysis, f.raw, f.capture, f.manifest,
                                                                f.resolution, diagnostic)
                    self.assertTrue(proof["original_metadata_readback_failure_preserved"])
                    self.assertEqual(proof["failed_switch_readback_values"], "unobserved")
                    self.assertEqual(proof["failed_switch_results"], "not_returned")
                    self.assertFalse(proof["failed_validator_source_bytes_verified"])
                    self.assertIsNone(proof["failed_validator_source"])
                    self.assertEqual(len(proof["functions"][0]["complete_sdk_switch_results"][0]["sdk_xref_targets"]), 2)
                else:
                    with self.assertRaises((ValueError, KeyError)):
                        adjudication.preexport_switch_scope(analysis, f.raw, f.capture, f.manifest,
                                                           f.resolution, diagnostic)

    def signed_switch_failure_fixture(self, root):
        import native_boundaries
        import native_callsites
        import test_native_callsites

        f = test_native_callsites.NativeAbsoluteSwitchTests().fixture(root)
        selection = analysis.native_function_selection(f.raw, f.capture, [0x1000],
            switch_reviews=[f.specification], decompiler_prepass_rvas=[0x1000],
            type_note="Retain the first unchanged owner derivation.")
        requested = selection["switch_metadata_recoveries"][0]
        sdk = test_native_callsites.NativeSwitchMetadataTests().sdk(f, requested)
        function = SimpleNamespace(start_ea=f.base + 0x1000, end_ea=f.base + 0x10a0, flags=0,
            frame=1, frsize=0, frregs=0, argsize=0, fpd=0)
        sdk.funcs.get_func = lambda ea: function
        sdk.utils.Chunks = lambda ea: [(function.start_ea, function.end_ea)]
        sdk.hexrays.mark_cfunc_dirty = lambda ea, close: None
        cases = requested["case_targets"]
        sdk.xref.calc_switch_cases = lambda ea, info: SimpleNamespace(
            cases=[[row["value"] & 0xffffffffffffffff] for row in cases],
            targets=[f.base + int(row["target_rva"], 16) for row in cases])
        sdk.xref.create_switch_xrefs = lambda ea, info: True
        sdk.utils.XrefsFrom = lambda ea: [SimpleNamespace(to=f.base + int(row["target_rva"], 16), iscode=True)
                                        for row in cases]
        applied = native_callsites.apply_switch(requested, f.base, sdk)
        output = f.result / "supplements/complete"
        selected = copy.deepcopy(f.body)
        selected["capture"] = f.write(output / "functions/1000.json", selected)
        prepass = selection["decompiler_prepasses"][0]
        initial = copy.deepcopy(f.body)
        initial["assembly_after_decompile"] = copy.deepcopy(initial["assembly"])
        snapshot = native_boundaries.prepass_snapshot(prepass, sdk)
        initial["decompiler_prepass"] = {**prepass, "before": snapshot, "after": copy.deepcopy(snapshot),
            "manual_stack_points_modified": False, "manual_frames_modified": False,
            "instructions_patched": False, "base_database_modified": False, "target_execution": False}
        prepass_record = f.write(output / "preanalysis/1000.json", initial)
        complete = {"function_selection": copy.deepcopy(selection), "switch_metadata_recoveries": [applied],
                    "decompiler_prepasses": [{**prepass, "capture": {**prepass_record, "path": "preanalysis/1000.json"}}]}
        record = f.write(output / "capture.json", complete)
        resolution = {"complete_functions": {"0x1000": selected}, "supplements": [{"receipt": record}]}
        diagnostic = root / ".analysis/results/failed-switch.log"
        diagnostic.write_bytes(b"fatal failure:\nTraceback (most recent call last):\n"
            b"  native_callsites.apply_switch(row, base, sdk)\n  File native_callsites.py, in apply_switch\n"
            b"  File native_callsites.py, in _x86_switch_cases\n"
            b"ValueError: SDK x86 case value is not an exact zero/sign extension of its 32-bit expression\n")
        observer_bytes = ("def capture():\n"
            "    if selection.get('switch_metadata_recoveries'):\n"
            "        manifest['switch_metadata_recoveries'] = [native_callsites.apply_switch(row, base, sdk) "
            "for row in selection['switch_metadata_recoveries']]\n"
            "    for prepass in selection.get('decompiler_prepasses', []):\n"
            "        export(base + int(prepass['rva'], 16), prepass=prepass)\n").encode()
        validator_bytes = ("def _x86_switch_cases(rows, signed):\n"
            "    raise ValueError('SDK x86 case value is not an exact zero/sign extension of its 32-bit expression')\n"
            "def apply_switch(requested, base, sdk):\n"
            "    raw_calculated = _switch_cases(sdk, 0, None, base)\n"
            "    calculated = _x86_switch_cases(raw_calculated, True)\n"
            "    sdk.xref.create_switch_xrefs(0, None)\n").encode()
        retained = root / ".analysis/results/reverse-pending/tool-sources"
        retained.mkdir(parents=True)
        observer = retained / (hashlib.sha256(observer_bytes).hexdigest() + ".py")
        observer.write_bytes(observer_bytes)
        validator = retained / (hashlib.sha256(validator_bytes).hexdigest() + ".py")
        validator.write_bytes(validator_bytes)
        manifest = {"function_selection": selection, "real_device_operations": False, "switch_metadata_recoveries": [],
            "capture_tool": {"path": "tools/ida/supplement.py", "sha256": analysis.sha(observer)},
            "callsite_type_validator": {"path": "tools/native_callsites.py", "bytes": len(validator_bytes),
                                       "sha256": analysis.sha(validator)}}
        return SimpleNamespace(**vars(f), output=output, manifest=manifest, complete=complete, selected=selected,
            resolution=resolution, diagnostic=diagnostic, observer=observer, validator=validator)

    def test_signed_case_preexport_requires_unexecuted_prepass_sources_complete_tables_and_real_new_derivation(self):
        for issue in (None, "new_reason", "prepass_scope", "prepass_executed", "missing_observer", "changed_observer",
                      "observer_order", "missing_validator", "changed_validator", "validator_order",
                      "stage", "table", "case", "raw_case", "xref", "frame", "new_prepass",
                      "new_prepass_file", "new_prepass_bytes", "new_prepass_scope", "unreviewed_scope"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.signed_switch_failure_fixture(Path(directory))
                selection = f.manifest["function_selection"]
                if issue == "new_reason":
                    reason = "Same exact unchanged owner, independently documented new observation reason."
                    current = f.complete["function_selection"]["decompiler_prepasses"][0]
                    current["reason"] = reason
                    record = f.complete["decompiler_prepasses"][0]
                    record["reason"] = reason
                    path = f.output / "preanalysis/1000.json"
                    body = json.loads(path.read_bytes()); body["decompiler_prepass"]["reason"] = reason
                    record["capture"] = {**f.write(path, body), "path": "preanalysis/1000.json"}
                elif issue == "prepass_scope": selection["decompiler_prepasses"][0]["rva"] = "0x1010"
                elif issue == "prepass_executed": f.manifest["decompiler_prepasses"] = [{"rva": "0x1000"}]
                elif issue == "missing_observer": f.observer.unlink()
                elif issue == "changed_observer": f.observer.write_bytes(b"# changed original observer\n")
                elif issue == "observer_order":
                    raw = f.observer.read_text().splitlines(keepends=True)
                    replacement = "".join([raw[0], *raw[3:], *raw[1:3]]).encode()
                    path = f.observer.with_name(hashlib.sha256(replacement).hexdigest() + ".py")
                    path.write_bytes(replacement); f.manifest["capture_tool"]["sha256"] = analysis.sha(path)
                elif issue == "missing_validator": f.validator.unlink()
                elif issue == "changed_validator": f.validator.write_bytes(b"# changed signed case validator\n")
                elif issue == "validator_order":
                    raw = f.validator.read_text().splitlines(keepends=True)
                    replacement = "".join([*raw[:3], raw[5], *raw[3:5]]).encode()
                    path = f.validator.with_name(hashlib.sha256(replacement).hexdigest() + ".py")
                    path.write_bytes(replacement)
                    f.manifest["callsite_type_validator"].update(bytes=len(replacement), sha256=analysis.sha(path))
                elif issue == "stage": f.diagnostic.write_bytes(b"fatal failure\nTraceback\nValueError: unknown failure\n")
                elif issue == "table": selection["switch_metadata_recoveries"][0]["table"]["raw_hex"] = "00" * 32
                elif issue == "case": f.complete["switch_metadata_recoveries"][0]["sdk_case_targets"].pop()
                elif issue == "raw_case": f.complete["switch_metadata_recoveries"][0]["sdk_case_targets_raw"].pop()
                elif issue == "xref": f.complete["switch_metadata_recoveries"][0]["sdk_xref_targets"].pop()
                elif issue == "frame": f.complete["switch_metadata_recoveries"][0]["after"]["native_frame"]["fpd"] += 1
                elif issue == "new_prepass": f.complete["decompiler_prepasses"] = []
                elif issue == "new_prepass_file": (f.output / "preanalysis/1000.json").unlink()
                elif issue == "new_prepass_bytes": (f.output / "preanalysis/1000.json").write_bytes(b"{}")
                elif issue == "new_prepass_scope": f.complete["function_selection"]["decompiler_prepasses"] = []
                elif issue == "unreviewed_scope": selection["function_outlines"] = [{"rva": "0x1000"}]
                if issue in ("case", "raw_case", "xref", "frame", "new_prepass", "new_prepass_scope", "new_reason"):
                    f.resolution["supplements"][0]["receipt"] = f.write(f.output / "capture.json", f.complete)
                diagnostic = adjudication.identity(analysis, f.diagnostic)
                if issue in (None, "new_reason"):
                    proof = adjudication.preexport_switch_scope(analysis, f.raw, f.capture, f.manifest,
                                                                f.resolution, diagnostic)
                    self.assertTrue(proof["original_signed_case_failure_preserved"])
                    self.assertFalse(proof["original_metadata_readback_failure_preserved"])
                    self.assertEqual(proof["failed_switch_failure_stage"], "signed32_case_validation")
                    self.assertFalse(proof["prepass_stage_proof"]["selected_prepasses_executed"])
                    self.assertTrue(proof["failed_validator_source_bytes_verified"])
                else:
                    with self.assertRaises((ValueError, KeyError, FileNotFoundError)):
                        adjudication.preexport_switch_scope(analysis, f.raw, f.capture, f.manifest,
                                                           f.resolution, diagnostic)

    def test_preexport_frame_attempt_still_requires_ended_failure_and_unchanged_source_database(self):
        for issue in (None, "database", "session", "diagnostic"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                f = self.frame_failure_fixture(root)
                source = root / ".work/native/source.i64"
                source.parent.mkdir(parents=True)
                source.write_bytes(b"inert unchanged original frame database")
                database = {"path": analysis.rel(source), "sha256": analysis.sha(source)}
                base_identity = {key: f.resolution["base_capture"][key] for key in ("path", "sha256")}
                f.manifest.update(input_sha256=f.capture["input_sha256"], base_capture=base_identity,
                    source_database=database, target_execution=False, capture_complete=False,
                    all_pseudocode_captured=False, functions=[], failures=[])
                failed = f.result / "supplements/failed"
                f.write(failed / "capture.json", f.manifest)
                session = {"finished_at_utc": "2026-10-27T00:00:00Z", "target_execution": False,
                    "capture_tool": f.manifest["capture_tool"], "passed": False, "exit_code": 1}
                if issue == "session": session["passed"] = True
                f.write(failed / "session.json", session)
                f.write(failed / "plan.json", {"base_capture": base_identity, "function_selection": f.manifest["function_selection"],
                    "source_database": database, "sha256": f.capture["input_sha256"], "input_bytes": f.capture["input_bytes"],
                    "input": f.capture["input"], "target_execution": False})
                if issue == "database": source.write_bytes(b"changed database")
                diagnostic = None if issue == "diagnostic" else adjudication.identity(analysis, f.diagnostic)
                failure = {"supplement": analysis.rel(failed / "capture.json"), "error": "Incomplete failed export"}
                if issue is None:
                    proof = adjudication.attempt_coverage(analysis, f.result, f.resolution, failure, diagnostic)
                    self.assertEqual(proof["capture_outcome"], "failed_before_export")
                    self.assertIn("preexport_frame_scope", proof)
                    self.assertNotIn("preexport_instruction_scope", proof)
                else:
                    with self.assertRaises(ValueError):
                        adjudication.attempt_coverage(analysis, f.result, f.resolution, failure, diagnostic)

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

    def test_temporary_register_requires_exact_full_membership_and_cannot_hide_unknown_locations(self):
        for issue in (None, "kernel", "membership", "api", "member", "processor", "name",
                      "offset", "width", "namespace", "class", "register_bool"):
            with self.subTest(issue=issue):
                body = {"address": "0x1000", "type": "void()", "type_before_decompile": "void()",
                    "type_after_decompile": "void()", "assembly": [], "assembly_after_decompile": [],
                    "pseudocode": {"locals_capture": {"schema_version": 2, "complete": True,
                                                      "mba_entry_ea": "0x1000"}, "locals": [{
                        "complete": True, "width": 8,
                        "location": {"complete": True, "status": "observed", "kind": "register",
                                     "register_namespace": "hexrays_microregisters", "register": 192, "register_offset": 0},
                        "ida_location_conversion": {"status": "not_applicable", "complete": True,
                            "reason": "sdk_temporary_register_has_no_processor_location"},
                        "virtual_register_proof": {"status": "api_returned", "complete": True,
                            "register_namespace": "hexrays_microregisters", "register": 192, "register_offset": 0,
                            "width": 8, "kernel_api": "ida_hexrays.is_kreg", "processor_api": "ida_hexrays.mreg2reg",
                            "name_api": "ida_hexrays.get_mreg_name", "is_kernel_register": False,
                            "temporary_api": "ida_hexrays.get_temp_regs", "membership_api": "mlist_t.has_all",
                            "is_codegen_temporary": True, "processor_register": -1, "name": "rtt"}}]}}
                local = body["pseudocode"]["locals"][0]
                proof = local["virtual_register_proof"]
                if issue == "kernel": proof["is_kernel_register"] = True
                elif issue == "membership": proof["membership_api"] = "mlist_t.has"
                elif issue == "api": proof["temporary_api"] = "inferred_name"
                elif issue == "member": proof["is_codegen_temporary"] = False
                elif issue == "processor": proof["processor_register"] = 0
                elif issue == "name": proof["name"] = ""
                elif issue == "offset": proof["register_offset"] = local["location"]["register_offset"] = 1
                elif issue == "width": proof["width"] = 4
                elif issue == "namespace": proof["register_namespace"] = "processor_registers"
                elif issue == "class": local["ida_location_conversion"]["reason"] = "unknown_register"
                elif issue == "register_bool": proof["register"] = local["location"]["register"] = True
                if issue is None: adjudication._typed_function(body)
                else:
                    with self.assertRaises(ValueError): adjudication._typed_function(body)

    def test_status_bits_require_exact_sdk_predicates_and_matching_operand_readback(self):
        for issue in (None, "bit", "predicate", "operand_api", "condition_api", "readback", "operand_width",
                      "offset", "wide", "kernel", "temporary", "processor", "condition_bool"):
            with self.subTest(issue=issue):
                body = {"address": "0x1000", "type": "void()", "type_before_decompile": "void()",
                    "type_after_decompile": "void()", "assembly": [], "assembly_after_decompile": [],
                    "pseudocode": {"locals_capture": {"schema_version": 2, "complete": True,
                                                      "mba_entry_ea": "0x1000"}, "locals": [{
                        "complete": True, "width": 1,
                        "location": {"complete": True, "status": "observed", "kind": "register",
                            "register_namespace": "hexrays_microregisters", "register": 181, "register_offset": 0},
                        "ida_location_conversion": {"status": "not_applicable", "complete": True,
                            "reason": "sdk_bit_register_has_no_processor_location"},
                        "virtual_register_proof": {"status": "api_returned", "complete": True,
                            "register_namespace": "hexrays_microregisters", "register": 181, "register_offset": 0,
                            "width": 1, "kernel_api": "ida_hexrays.is_kreg", "processor_api": "ida_hexrays.mreg2reg",
                            "name_api": "ida_hexrays.get_mreg_name", "is_kernel_register": False,
                            "temporary_api": "ida_hexrays.get_temp_regs", "membership_api": "mlist_t.has_all",
                            "is_codegen_temporary": False, "processor_register": -1, "name": "c0",
                            "operand_api": "ida_hexrays.mop_t.make_reg", "operand_is_exact_register": True,
                            "operand_register": 181, "operand_width": 1, "is_bit_register": True,
                            "is_condition_code": False, "bit_api": "ida_hexrays.mop_t.is_bit_reg",
                            "condition_api": "ida_hexrays.mop_t.is_cc"}}]}}
                local = body["pseudocode"]["locals"][0]
                proof = local["virtual_register_proof"]
                if issue == "bit": proof["is_bit_register"] = False
                elif issue == "predicate": proof["bit_api"] = "inferred_name"
                elif issue == "operand_api": proof["operand_api"] = "inferred_register"
                elif issue == "condition_api": proof["condition_api"] = "inferred_cc"
                elif issue == "readback": proof["operand_register"] = 182
                elif issue == "operand_width": proof["operand_width"] = 2
                elif issue == "offset": proof["register_offset"] = local["location"]["register_offset"] = 1
                elif issue == "wide": proof["width"] = proof["operand_width"] = local["width"] = 2
                elif issue == "kernel": proof["is_kernel_register"] = True
                elif issue == "temporary": proof["is_codegen_temporary"] = True
                elif issue == "processor": proof["processor_register"] = 0
                elif issue == "condition_bool": proof["is_condition_code"] = 0
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

    def test_instruction_annotations_need_exact_values_presence_reason_and_unchanged_machine(self):
        for issue in (None, "missing", "value", "presence", "presence_type", "missing_null", "reason",
                      "duplicate", "bytes", "xref", "unreviewed_metadata"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                old = json.loads((f.first / "functions/1000.json").read_bytes())
                f.body["assembly"][0]["comment"] = "buffer"
                if issue == "bytes": f.body["assembly"][0]["bytes"] = "90"
                elif issue == "xref": f.body["assembly"][0]["xrefs_from"] = [{"to": "0x140001001"}]
                elif issue == "unreviewed_metadata": f.body["assembly"][0]["new_metadata"] = "invented"
                f.body["assembly_after_decompile"] = copy.deepcopy(f.body["assembly"])
                record = f.write(f.output / "functions/1000.json", f.body)
                f.manifest["functions"][0]["sha256"] = record["sha256"]
                f.write(f.output / "capture.json", f.manifest)
                entry = f.review["functions"][0]
                entry["candidates"] = [{"function": adjudication.identity(analysis, d / "functions/1000.json"),
                                        "supplement": adjudication.identity(analysis, d / "capture.json")}
                                       for d in (f.first, f.output)]
                entry["selected"] = adjudication.identity(analysis, f.output / "functions/1000.json")
                comparison = entry["comparisons"][0]
                comparison["differences"] = [dict(change, classification="capture_metadata_replacement",
                    reason="Complete original instruction annotation and type evidence reviewed")
                    for change in adjudication.differences(old, f.body)]
                annotation = {"address": old["assembly"][0]["address"],
                    "previous_comment": old["assembly"][0].get("comment"), "selected_comment": "buffer",
                    "previous_present": "comment" in old["assembly"][0], "selected_present": True,
                    "reason": "Exact argument annotation from the independently reviewed physical ABI"}
                comparison["assembly_comment_differences"] = [annotation]
                if issue == "missing": del comparison["assembly_comment_differences"]
                elif issue == "value": annotation["previous_comment"] = "invented"
                elif issue == "presence": annotation["previous_present"] = not annotation["previous_present"]
                elif issue == "presence_type": annotation["selected_present"] = 1
                elif issue == "missing_null": del annotation["previous_comment"]
                elif issue == "reason": annotation["reason"] = ""
                elif issue == "duplicate": comparison["assembly_comment_differences"] *= 2
                f.write(f.review_path, f.review)
                f.review_record = adjudication.identity(analysis, f.review_path)
                resolved = self.resolve(f)
                self.assertEqual("0x1000" in resolved.get("adjudicated_functions", {}), issue is None)
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
                f.review_path = Path(directory) / ".analysis/results/extended-attempt-review.json"
                f.write(f.review_path, f.review)
                args.review = analysis.rel(f.review_path)
                args.review_sha256 = analysis.sha(f.review_path)
                if issue is None:
                    with redirect_stdout(io.StringIO()): adjudication.run(analysis, args)
                    self.assertEqual(json.loads(binding.read_bytes())["previous_binding_snapshot"], f.review["previous_binding_snapshot"])
                    self.assertFalse(analysis.native_reanalysis_resolution(f.result)["verification_failures"])
                    with redirect_stdout(io.StringIO()): adjudication.run(analysis, args)
                else:
                    with self.assertRaises(ValueError), redirect_stdout(io.StringIO()): adjudication.run(analysis, args)

    def test_binding_extension_preserves_every_old_function_and_attempt_decision(self):
        for attempts in (False, True):
            for issue in (None, "drop", "duplicate", "input", "base", "kind"):
                with self.subTest(attempts=attempts, issue=issue), tempfile.TemporaryDirectory() as directory, \
                        patch.object(analysis, "ROOT", Path(directory)):
                    root = Path(directory)
                    owner = root / ".analysis/results"
                    owner.mkdir(parents=True)
                    entries = ([{"capture": {"path": "first.json"}}, {"capture": {"path": "second.json"}}]
                               if attempts else [{"rva": "0x1000"}, {"rva": "0x2000"}])
                    name = "attempts" if attempts else "functions"
                    previous = {"review_kind": "native_superseded_attempt_adjudication" if attempts else "native_complete_artifact_adjudication",
                                "source_input": {"sha256": "a" * 64}, "base_capture": {"sha256": "b" * 64}, name: entries}
                    path = owner / "previous.json"
                    path.write_text(json.dumps(previous), encoding="utf-8")
                    binding = json.dumps({"review": adjudication.identity(analysis, path)}).encode()
                    review = copy.deepcopy(previous)
                    review[name].append({"capture": {"path": "third.json"}} if attempts else {"rva": "0x3000"})
                    if issue == "drop": review[name].pop(0)
                    elif issue == "duplicate": review[name].append(copy.deepcopy(review[name][0]))
                    elif issue == "input": review["source_input"]["sha256"] = "c" * 64
                    elif issue == "base": review["base_capture"]["sha256"] = "c" * 64
                    elif issue == "kind": review["review_kind"] = "different"
                    if issue is None:
                        adjudication.preserve_binding_scope(analysis, binding, review, attempts)
                    else:
                        with self.assertRaises(ValueError):
                            adjudication.preserve_binding_scope(analysis, binding, review, attempts)


class PhysicalTbyteAdjudicationTests(unittest.TestCase):
    def body(self):
        def part(register, offset, size, namespace):
            return {"status": "observed", "complete": True, "kind": "register",
                    "register_namespace": namespace, "register": register,
                    "register_offset": 0, "argument_offset": offset, "size": size}

        proof = {"status": "api_returned", "complete": True,
            "register_namespace": "hexrays_microregisters", "register": 713, "register_offset": 0,
            "width": 10, "identity_width": 8, "identity_mapping_only": True,
            "type_is_float": False, "type_is_floating": True, "type_is_tbyte": True, "type_bytes": 10,
            "operand_register": 713, "operand_width": 10, "operand_is_exact_register": True,
            "is_bit_register": False, "is_condition_code": False, "is_kernel_register": False,
            "operand_is_kernel_register": False, "is_codegen_temporary": False,
            "processor_register_at_original_width": -1, "identity_processor_register": 45,
            "identity_roundtrip_microregister": 713, "processor_name_width": 10,
            "processor_name": "physical_float", "processor_parsed": True,
            "processor_parsed_register": 45, "processor_capacity_bytes": 10, "processor_tbyte_bytes": 10,
            "apis": {"type_float": "tinfo_t.is_float", "type_floating": "tinfo_t.is_floating",
                "type_tbyte": "tinfo_t.is_tbyte", "type_size": "tinfo_t.get_size",
                "operand": "ida_hexrays.mop_t.make_reg", "operand_exact": "mop_t.is_reg",
                "bit": "mop_t.is_bit_reg", "condition": "mop_t.is_cc",
                "operand_kernel": "mop_t.is_kreg", "kernel": "ida_hexrays.is_kreg",
                "temporary": "ida_hexrays.get_temp_regs; mlist_t.has_all",
                "processor": "ida_hexrays.mreg2reg", "identity_roundtrip": "ida_hexrays.reg2mreg",
                "processor_name": "ida_idp.get_reg_name", "processor_info": "ida_idp.parse_reg_name; reg_info_t.reg/size",
                "tbyte_size": "ida_idp.ph_get_tbyte_size"}}
        original = {"status": "api_returned", "complete": False, "roundtrip_matches": False,
            "conversion_api": "mba_t.vd2idaloc", "roundtrip_api": "mba_t.idaloc2vd",
            "location": {"status": "observed", "complete": True, "kind": "scattered",
                "register_namespace": "ida_processor_registers", "part_count": 2,
                "parts": [part(45, 0, 8, "ida_processor_registers"), part(44, 8, 2, "ida_processor_registers")]},
            "roundtrip_location": {"status": "observed", "complete": True, "kind": "scattered",
                "register_namespace": "hexrays_microregisters", "part_count": 1,
                "parts": [part(713, 0, 10, "hexrays_microregisters")]}}
        local = {"name": "value", "type": "_TBYTE", "width": 10, "complete": True,
            "location": {"status": "observed", "complete": True, "kind": "register",
                "register_namespace": "hexrays_microregisters", "register": 713, "register_offset": 0,
                "text": "unrelated_micro_float_name"},
            "physical_register_proof": proof, "ida_location_conversion": {
                "status": "physical_register_verified", "complete": True,
                "reason": "sdk_x87_tbyte_capacity_and_identity_roundtrip", "original_conversion": original}}
        return {"address": "0x1000", "type": "void()", "type_before_decompile": "void()",
            "type_after_decompile": "void()", "assembly": [], "assembly_after_decompile": [],
            "pseudocode": {"locals_capture": {"schema_version": 2, "complete": True,
                                              "mba_entry_ea": "0x1000"}, "locals": [local]}}

    def test_physical_proof_retains_failed_conversion_without_virtual_exemption_or_rounding(self):
        body = self.body()
        adjudication._typed_function(body)
        local = body["pseudocode"]["locals"][0]
        self.assertEqual(local["width"], 10)
        conversion = local["ida_location_conversion"]
        self.assertNotIn("roundtrip_matches", conversion)
        self.assertFalse(conversion["original_conversion"]["roundtrip_matches"])
        self.assertNotIn("virtual_register_proof", local)

    def test_each_physical_identity_type_capacity_operand_or_api_error_is_rejected(self):
        changes = {"status": "unobserved", "complete": False, "register_namespace": "processor",
            "register": 714, "register_offset": 1, "width": 8, "identity_width": 10,
            "identity_mapping_only": False, "type_is_float": True, "type_is_floating": False,
            "type_is_tbyte": False, "type_bytes": 8,
            "operand_register": 714, "operand_width": 8, "operand_is_exact_register": False,
            "is_bit_register": True, "is_condition_code": True, "is_kernel_register": True,
            "operand_is_kernel_register": True, "is_codegen_temporary": True,
            "processor_register_at_original_width": 45, "identity_processor_register": -1,
            "identity_roundtrip_microregister": 714, "processor_name_width": 8,
            "processor_name": "", "processor_parsed": False, "processor_parsed_register": 44,
            "processor_capacity_bytes": 8, "processor_tbyte_bytes": 8}
        for key, value in changes.items():
            with self.subTest(key=key):
                body = self.body()
                body["pseudocode"]["locals"][0]["physical_register_proof"][key] = value
                with self.assertRaises(ValueError): adjudication._typed_function(body)
        for key in self.body()["pseudocode"]["locals"][0]["physical_register_proof"]["apis"]:
            with self.subTest(api=key):
                body = self.body()
                body["pseudocode"]["locals"][0]["physical_register_proof"]["apis"][key] = "inferred"
                with self.assertRaises(ValueError): adjudication._typed_function(body)

    def test_failed_conversion_and_every_scattered_coordinate_remain_required(self):
        issues = ("schema", "local_width", "location_register", "location_offset", "location_kind",
            "virtual", "fabricated_roundtrip", "lost_original", "original_complete", "original_roundtrip",
            "original_status", "conversion_api", "roundtrip_api", "lost_back", "back_kind", "back_register",
            "back_part_width", "back_part_offset", "back_part_namespace", "back_split", "ida_kind", "ida_register",
            "ida_part_width", "ida_part_offset", "ida_part_namespace", "register_bool", "width_bool")
        for issue in issues:
            with self.subTest(issue=issue):
                body = self.body(); local = body["pseudocode"]["locals"][0]
                proof, location, conversion = (local[key] for key in (
                    "physical_register_proof", "location", "ida_location_conversion"))
                original = conversion["original_conversion"]
                if issue == "schema": body["pseudocode"]["locals_capture"]["schema_version"] = 1
                elif issue == "local_width": local["width"] = 8
                elif issue == "location_register": location["register"] = 714
                elif issue == "location_offset": location["register_offset"] = 1
                elif issue == "location_kind": location["kind"] = "register_pair"
                elif issue == "virtual": local["virtual_register_proof"] = {}
                elif issue == "fabricated_roundtrip": conversion["roundtrip_matches"] = True
                elif issue == "lost_original": del conversion["original_conversion"]
                elif issue == "original_complete": original["complete"] = True
                elif issue == "original_roundtrip": original["roundtrip_matches"] = True
                elif issue == "original_status": original["status"] = "diagnostic_error"
                elif issue in ("conversion_api", "roundtrip_api"): original[issue] = "guessed"
                elif issue == "lost_back": del original["roundtrip_location"]
                elif issue == "back_kind": original["roundtrip_location"]["kind"] = "unknown"
                elif issue == "back_register": original["roundtrip_location"]["parts"][0]["register"] = 714
                elif issue == "back_part_width": original["roundtrip_location"]["parts"][0]["size"] = 8
                elif issue == "back_part_offset": original["roundtrip_location"]["parts"][0]["argument_offset"] = 1
                elif issue == "back_part_namespace": original["roundtrip_location"]["parts"][0]["register_namespace"] = "cpu"
                elif issue == "back_split":
                    original["roundtrip_location"]["part_count"] = 2
                    original["roundtrip_location"]["parts"].append(copy.deepcopy(original["roundtrip_location"]["parts"][0]))
                elif issue == "ida_kind": original["location"]["kind"] = "register"
                elif issue == "ida_register": original["location"]["parts"][0]["register"] = 44
                elif issue == "ida_part_width": original["location"]["parts"][1]["size"] = 1
                elif issue == "ida_part_offset": original["location"]["parts"][1]["argument_offset"] = 9
                elif issue == "ida_part_namespace": original["location"]["parts"][1]["register_namespace"] = "micro"
                elif issue == "register_bool": proof["register"] = location["register"] = True
                elif issue == "width_bool": proof["type_bytes"] = True
                with self.assertRaises(ValueError): adjudication._typed_function(body)


class NativeRecursiveContextCoverageTests(unittest.TestCase):
    def fixture(self, root):
        raw = test_evidence.NativeSupplementTests().fixture()
        raw[0x200:0x208] = bytes.fromhex("6690e8fbffffffc3")
        source = root / ".analysis/inputs/recursive.dll"
        source.parent.mkdir(parents=True)
        source.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        output = result / "supplements/1234567890abcdef"
        image = 0x140000000

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return adjudication.identity(analysis, path)

        flow = {"from": hex(image + 0x1000), "to": hex(image + 0x1002),
                "type": 21, "code": True, "user": False}
        recursive = {"from": hex(image + 0x1002), "to": hex(image + 0x1002),
                     "type": 17, "code": True, "user": False}
        original = []
        for rva, code, sizes in ((0x1000, bytes.fromhex("6690"), (2,)),
                                 (0x1002, bytes.fromhex("e8fbffffffc3"), (5, 1))):
            cursor = image + rva
            assembly = []
            position = 0
            for size in sizes:
                assembly.append({"address": hex(cursor), "rva": hex(cursor-image),
                    "bytes": code[position:position+size].hex(), "text": "ret" if size == 1 else "call core" if size == 5 else "xchg ax, ax",
                    "xrefs_from": [recursive] if size == 5 else [flow] if rva == 0x1000 else [],
                    "xrefs_to": [flow, recursive] if cursor == image + 0x1002 else []})
                cursor += size
                position += size
            body = {"address": hex(image+rva), "rva": hex(rva), "end": hex(cursor),
                "chunks": [{"start": hex(image+rva), "end": hex(cursor), "raw_hex": code.hex(),
                            "sha256": hashlib.sha256(code).hexdigest()}],
                "assembly": assembly, "xrefs_to_entry": assembly[0]["xrefs_to"], "pseudocode": None}
            original.append(body)
        original_refs = [write(result / "functions" / (body["rva"][2:] + ".json"), body) for body in original]

        def index(record, body, group, typed):
            return {**record, "path": group + "/" + body["rva"][2:] + ".json", "rva": body["rva"],
                    "address": body["address"], "assembly_captured": True, "pseudocode_captured": typed}

        base = {"input": analysis.rel(source), "input_sha256": hashlib.sha256(raw).hexdigest(),
                "input_bytes": len(raw), "image_base": hex(image),
                "functions": [index(ref, body, "functions", False) for ref, body in zip(original_refs, original)]}
        write(result / "capture.json", base)
        root_body, core_body = copy.deepcopy(original)
        root_body["pseudocode"] = {"entry": root_body["address"], "lines": [{"line": 0, "text": "void root(void) { core(); }"}], "locals": []}
        test_evidence.NativeSupplementTests.typed_fixture(root_body)
        root_ref = write(output / "functions/1000.json", root_body)
        core_ref = write(output / "context-helpers/1002.json", core_body)
        request = {"root_rva": "0x1000", "core_rva": "0x1002", "root_function": original_refs[0],
            "core_function": original_refs[1], "proof": {
                "original_machine_union": {"start": root_body["address"], "end": core_body["end"], "bytes": 8,
                    "sha256": hashlib.sha256(bytes(raw[0x200:0x208])).hexdigest(), "instructions": 3},
                "original_fallthrough": original[0]["assembly"][-1],
                "recursive_call_instructions": [original[1]["assembly"][0]],
                "all_original_return_instructions": [original[1]["assembly"][1]]}}
        recovery = {"request": request, "root_capture": root_ref, "core_capture": core_ref, "warnings": []}
        manifest = {"input_sha256": base["input_sha256"], "function_selection": {"recursive_fallthrough_contexts": [request]},
            "recursive_context_recoveries": [recovery], "functions": [index(root_ref, root_body, "functions", True)],
            "context_helpers": [index(core_ref, core_body, "context-helpers", False)],
            "target_execution": False, "failures": [], "capture_complete": True, "all_pseudocode_captured": True}
        return SimpleNamespace(raw=bytes(raw), base=base, manifest=manifest, output=output, write=write,
            root=root_body, core=core_body, request=request, recovery=recovery)

    def test_complete_context_keeps_root_cfunc_identity_and_separate_original_core(self):
        import native_boundaries
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), \
                patch.object(native_boundaries, "verify_recursive_contexts", create=True) as verify:
            f = self.fixture(Path(directory))
            result = adjudication.recursive_context_coverage(analysis, f.raw, f.base, f.manifest, f.output)
            verify.assert_called_once_with(analysis, [f.request], [f.recovery])
            self.assertEqual(set(result), {"0x1002"})
            self.assertEqual(result["0x1002"]["complete_root"], f.recovery["root_capture"])
            self.assertFalse(result["0x1002"]["numerical_semantics_complete"])
            self.assertIsNone(f.core["pseudocode"])
            self.assertEqual(f.root["pseudocode"]["locals_capture"]["mba_entry_ea"], f.root["address"])

    def old_context_function(self, f, merged):
        root = json.loads((analysis.ROOT / f.request["root_function"]["path"]).read_bytes())
        core = json.loads((analysis.ROOT / f.request["core_function"]["path"]).read_bytes())
        if not merged:
            return core
        raw = b"".join(bytes.fromhex(row["raw_hex"]) for body in (root, core) for row in body["chunks"])
        return {**root, "end": core["end"], "assembly": [*root["assembly"], *core["assembly"]],
            "chunks": [{"start": root["address"], "end": core["end"], "raw_hex": raw.hex(),
                        "sha256": hashlib.sha256(raw).hexdigest()}]}

    def test_old_core_and_merged_owner_keep_complete_context_proof_and_original_failure(self):
        import native_boundaries
        for merged in (False, True):
            with self.subTest(merged=merged), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)), \
                    patch.object(native_boundaries, "verify_recursive_contexts", create=True):
                f = self.fixture(Path(directory))
                result = f.output.parent.parent
                base_ref = adjudication.identity(analysis, result / "capture.json")
                base_short = {key: base_ref[key] for key in ("path", "sha256")}
                current_ref = f.write(f.output / "capture.json", f.manifest)
                contexts = adjudication.recursive_context_coverage(analysis, f.raw, f.base, f.manifest, f.output)
                old = self.old_context_function(f, merged)
                rva = old["rva"]
                failed = result / "supplements/old-context"
                previous_ref = f.write(failed / "functions" / (rva[2:] + ".json"), old)
                selection = {"targets": [{"rva": rva}]}
                old_manifest = {"input_sha256": f.base["input_sha256"], "base_capture": base_short,
                    "target_execution": False, "all_pseudocode_captured": False, "capture_complete": True,
                    "capture_tool": {"path": "tools/ida/supplement.py", "sha256": "0"*64},
                    "database_integrity": {"verified": True}, "base_capture_unchanged": True,
                    "base_database_unchanged": True, "function_selection": selection,
                    "functions": [{**previous_ref, "path": "functions/" + rva[2:] + ".json", "rva": rva,
                        "assembly_captured": True, "pseudocode_captured": False}],
                    "failures": [{"rva": rva, "stage": "hexrays", "error": "original call analysis failure"}]}
                old_capture = f.write(failed / "capture.json", old_manifest)
                f.write(failed / "session.json", {"target_execution": False, "finished_at_utc": "2026-10-26T00:00:00Z",
                    "capture_tool": old_manifest["capture_tool"], "passed": True, "exit_code": 0})
                f.write(failed / "plan.json", {"base_capture": base_short, "function_selection": selection,
                    "input": f.base["input"], "sha256": f.base["input_sha256"], "input_bytes": f.base["input_bytes"],
                    "source_database": None, "target_execution": False})
                resolution = {"base_capture": base_ref, "complete_functions": {
                    "0x1000": {"capture": f.recovery["root_capture"], **f.root}},
                    "complete_contexts": {r: {**v, "supplement": current_ref} for r, v in contexts.items()},
                    "conflicting_functions": {}, "supplements": [{"receipt": current_ref}]}
                before = (failed / "capture.json").read_bytes()
                proof = adjudication.attempt_coverage(analysis, result, resolution, {"supplement": old_capture["path"]})
                covered = proof["functions"][0]["recursive_context_original_coverage"]
                self.assertEqual(covered["previous_machine_kind"],
                    "complete_original_root_core_union" if merged else "original_physical_core")
                self.assertEqual(covered["complete_context"]["complete_root"], f.recovery["root_capture"])
                self.assertFalse(covered["core_relabelled_as_root"])
                self.assertFalse(covered["numerical_semantics_complete"])
                self.assertEqual(proof["original_failures"], old_manifest["failures"])
                self.assertEqual((failed / "capture.json").read_bytes(), before)

    def test_context_attempt_rejects_changed_old_union_core_root_or_target(self):
        import native_boundaries
        for merged in (False, True):
            for issue in ("entry", "end", "chunks", "missing_instruction", "instruction_bytes", "entry_edges", "replacement_core", "replacement_local"):
                with self.subTest(merged=merged, issue=issue), tempfile.TemporaryDirectory() as directory, \
                        patch.object(analysis, "ROOT", Path(directory)), \
                        patch.object(native_boundaries, "verify_recursive_contexts", create=True):
                    f = self.fixture(Path(directory))
                    old = self.old_context_function(f, merged)
                    replacement = copy.deepcopy(f.root)
                    if issue == "entry": old["rva"] = "0x1001"
                    elif issue == "end": old["end"] = hex(int(old["end"], 16)-1)
                    elif issue == "chunks": old["chunks"][0]["sha256"] = "0"*64
                    elif issue == "missing_instruction": old["assembly"].pop()
                    elif issue == "instruction_bytes": old["assembly"][0]["bytes"] = "90"
                    elif issue == "entry_edges": old["xrefs_to_entry"] = [dict(from_address="fake")]
                    elif issue == "replacement_core": replacement = copy.deepcopy(f.core)
                    elif issue == "replacement_local": replacement["pseudocode"]["locals_capture"]["complete"] = False
                    with self.assertRaises(ValueError):
                        adjudication.recursive_context_attempt_coverage(analysis, f.raw, f.base, old,
                            replacement, f.manifest, f.output)

    def test_context_rejects_identity_relabeling_missing_original_bytes_or_index_and_incomplete_root(self):
        import native_boundaries
        cases = ("unselected", "missing_recovery", "duplicate_owner", "core_index", "root_index", "index_hash",
            "core_index_typed", "core_pseudocode", "source", "source_claim", "request", "root_path", "core_path",
            "root_file_changed", "base_index", "root_relabel", "core_boundary", "root_boundary", "typed_entry",
            "missing_lines", "pseudo_entry", "warning", "diagnostic", "union_size", "union_hash", "union_count",
            "fallthrough", "missing_calls", "missing_ret", "runtime", "capture_failure", "incomplete")
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)), \
                    patch.object(native_boundaries, "verify_recursive_contexts", create=True):
                f = self.fixture(Path(directory))
                if case == "unselected": f.manifest["function_selection"].clear()
                elif case == "missing_recovery": f.manifest["recursive_context_recoveries"].clear()
                elif case == "duplicate_owner":
                    f.manifest["function_selection"]["recursive_fallthrough_contexts"] *= 2
                    f.manifest["recursive_context_recoveries"] *= 2
                    f.manifest["context_helpers"] *= 2
                elif case == "core_index": f.manifest["context_helpers"].clear()
                elif case == "root_index": f.manifest["functions"].clear()
                elif case == "index_hash": f.manifest["context_helpers"][0]["sha256"] = "0"*64
                elif case == "core_index_typed": f.manifest["context_helpers"][0]["pseudocode_captured"] = True
                elif case == "source": f.raw = f.raw[:-1] + b"X"
                elif case == "source_claim": f.manifest["input_sha256"] = "0"*64
                elif case == "request": f.recovery["request"] = {}
                elif case in ("root_path", "core_path"):
                    f.recovery[case.split("_")[0]+"_capture"]["path"] += ".elsewhere"
                elif case == "root_file_changed": (f.output / "functions/1000.json").write_bytes(b"changed")
                elif case == "base_index": f.base["functions"][1]["sha256"] = "0"*64
                elif case == "root_relabel": f.root["rva"] = "0x1002"
                elif case == "core_boundary": f.core["end"] = "0x140001007"
                elif case == "root_boundary": f.root["end"] = f.core["end"]
                elif case == "typed_entry": f.root["pseudocode"]["locals_capture"]["mba_entry_ea"] = f.core["address"]
                elif case == "core_pseudocode": f.core["pseudocode"] = {}
                elif case == "missing_lines": f.root["pseudocode"]["lines"] = []
                elif case == "pseudo_entry": f.root["pseudocode"]["entry"] = f.core["address"]
                elif case == "warning": f.recovery["warnings"] = [{"id": 1}]
                elif case == "diagnostic": f.root["hexrays_diagnostics"] = {"diagnostics_complete": False}
                elif case in ("union_size", "union_hash", "union_count"):
                    key, value = {"union_size": ("bytes", 7), "union_hash": ("sha256", "0"*64), "union_count": ("instructions", 2)}[case]
                    f.request["proof"]["original_machine_union"][key] = value
                elif case == "fallthrough": f.request["proof"]["original_fallthrough"] = {}
                elif case == "missing_calls": f.request["proof"]["recursive_call_instructions"] = []
                elif case == "missing_ret": f.request["proof"]["all_original_return_instructions"] = []
                elif case == "runtime": f.manifest["target_execution"] = True
                elif case == "capture_failure": f.manifest["failures"] = [{"stage": "hexrays"}]
                elif case == "incomplete": f.manifest["capture_complete"] = False
                if case in ("root_relabel", "root_boundary", "typed_entry", "missing_lines", "pseudo_entry", "diagnostic"):
                    ref = f.write(f.output / "functions/1000.json", f.root)
                    f.recovery["root_capture"] = ref
                    f.manifest["functions"][0].update({k: v for k, v in ref.items() if k != "path"})
                if case in ("core_boundary", "core_pseudocode"):
                    ref = f.write(f.output / "context-helpers/1002.json", f.core)
                    f.recovery["core_capture"] = ref
                    f.manifest["context_helpers"][0].update({k: v for k, v in ref.items() if k != "path"})
                with self.assertRaises(ValueError):
                    adjudication.recursive_context_coverage(analysis, f.raw, f.base, f.manifest, f.output)


class NativeSharedTailAttemptTests(unittest.TestCase):
    def fixture(self, root, *, secondary=False, inline=False):
        owner = test_evidence.NativeSupplementTests()
        result, output, capture, manifest, primary, other, _, write = owner.shared_tail_reanalysis_fixture(root, inline)
        raw = (root / capture["input"]).read_bytes()
        rva = "0x1040" if secondary else "0x1000"
        if secondary:
            manifest["function_selection"] = analysis.native_function_selection(raw, capture, [0x1040],
                type_note="Complete shared-entry control-flow review",
                shared_tail_specs=analysis.native_shared_tail_specifications(
                    manifest["function_selection"]["function_shared_tails"]))
            primary["pseudocode"] = None
            other["pseudocode"] = {"lines": [{"line": 0, "text": "int secondary(void) { return 0; }"}], "locals": []}
            owner.typed_fixture(other)
            replacement, helper = other, primary
        else:
            replacement, helper = primary, other
        def index(group, body, pseudo):
            name = group + "/" + body["rva"][2:] + ".json"
            ref = write(output / name, body)
            return {**ref, "path": name, "rva": body["rva"], "address": body["address"],
                    "assembly_captured": True, "pseudocode_captured": pseudo}
        manifest["functions"] = [index("functions", replacement, True)]
        manifest["shared_tail_helpers"] = [index("shared-tail-helpers", helper, False)]
        write(output / "capture.json", manifest)
        write(output / "plan.json", {"function_selection": manifest["function_selection"],
            "base_capture": manifest["base_capture"], "source_database": manifest["source_database"]})
        previous = json.loads((result / "functions" / (rva[2:] + ".json")).read_bytes())
        return SimpleNamespace(result=result, output=output, capture=capture, manifest=manifest,
            previous=previous, body=replacement, helper=helper, write=write, raw=raw, index=index)

    def test_primary_segmentation_and_secondary_extension_preserve_complete_original_bytes_and_edges(self):
        for secondary in (False, True):
            for inline in (False, True):
                with self.subTest(secondary=secondary, inline=inline), tempfile.TemporaryDirectory() as directory, \
                        patch.object(analysis, "ROOT", Path(directory)):
                    f = self.fixture(Path(directory), secondary=secondary, inline=inline)
                    before = {path: path.read_bytes() for path in (f.result / "capture.json",
                        f.output / "capture.json", f.result / "functions/1000.json", f.result / "functions/1040.json")}
                    self.assertFalse(adjudication._same_machine(f.previous, f.body))
                    proof = adjudication.shared_tail_attempt_coverage(analysis, f.raw, f.capture,
                        f.previous, f.body, f.manifest, f.output)
                    self.assertEqual(proof["coverage_kind"], "reviewed_complete_shared_tail_owner")
                    self.assertEqual(proof["original_function"]["path"], analysis.rel(
                        f.result / "functions" / (f.previous["rva"][2:] + ".json")))
                    self.assertEqual(len(proof["complete_helper_assemblies"]), 1)
                    self.assertTrue(proof["all_original_bytes_preserved"])
                    self.assertTrue(proof["all_original_edges_preserved"])
                    self.assertFalse(proof["helper_semantics_or_abi_completion"])
                    self.assertFalse(proof["target_execution"])
                    self.assertEqual({path: path.read_bytes() for path in before}, before)

    def test_shared_tail_coverage_rejects_changed_source_owner_tail_edges_replacement_and_helpers(self):
        cases = ("unreviewed", "source_identity", "source_owner", "tail_bytes", "tail_incoming", "tail_outgoing",
            "missing_recovery", "wrong_primary", "wrong_owner", "applied_end", "applied_chunks", "applied_flags",
            "old_bytes", "old_incoming", "old_outgoing", "replacement_end", "replacement_chunks",
            "replacement_missing", "replacement_bytes", "replacement_incoming", "replacement_outgoing",
            "replacement_flags", "missing_typed", "missing_helper", "duplicate_helper", "helper_bytes",
            "helper_incoming", "helper_outgoing", "helper_flags", "helper_pseudocode", "helper_hash", "helper_path")
        for secondary in (False, True):
            for case in cases:
                with self.subTest(secondary=secondary, case=case), tempfile.TemporaryDirectory() as directory, \
                        patch.object(analysis, "ROOT", Path(directory)):
                    f = self.fixture(Path(directory), secondary=secondary, inline=True)
                    shared = f.manifest["function_selection"]["function_shared_tails"][0]
                    applied = f.manifest["function_shared_tail_recoveries"][0]
                    record = f.manifest["shared_tail_helpers"][0]
                    if case == "unreviewed": f.manifest["function_selection"]["function_shared_tails"] = []
                    elif case == "source_identity": shared["owners"][0]["source_function"]["sha256"] = "0" * 64
                    elif case == "source_owner": shared["owners"][0]["source_function"] = shared["owners"][1]["source_function"]
                    elif case == "tail_bytes": shared["expected_tail"]["raw_hex"] = "9090c3"
                    elif case == "tail_incoming": shared["incoming_edges"].pop()
                    elif case == "tail_outgoing": shared["outgoing_edges"] = [shared["incoming_edges"][0]]
                    elif case == "missing_recovery": f.manifest["function_shared_tail_recoveries"] = []
                    elif case == "wrong_primary": applied["applied_primary_owner_rva"] = "0x1040"
                    elif case == "wrong_owner": applied["applied_owners"][0]["rva"] = "0x1040"
                    elif case == "applied_end": applied["applied_owners"][0]["end"] = "0x140001007"
                    elif case == "applied_chunks": applied["applied_owners"][0]["chunks"] = []
                    elif case == "applied_flags": applied["applied_owners"][0]["applied_flags"] = 0
                    elif case == "old_bytes": f.previous["assembly"][0]["bytes"] = "90"
                    elif case == "old_incoming": f.previous["assembly"][-1]["xrefs_to"].append(
                        copy.deepcopy(f.previous["assembly"][0]["xrefs_from"][0]))
                    elif case == "old_outgoing": f.previous["assembly"][0]["xrefs_from"] = []
                    elif case == "replacement_end": f.body["end"] = "0x140001080"
                    elif case == "replacement_chunks": f.body["chunks"].pop()
                    elif case == "replacement_missing": f.body["assembly"].pop()
                    elif case == "replacement_bytes": f.body["assembly"][0]["bytes"] = "90"
                    elif case == "replacement_incoming": f.body["assembly"][1]["xrefs_to"] = []
                    elif case == "replacement_outgoing": f.body["assembly"][0]["xrefs_from"] = []
                    elif case == "replacement_flags": f.body["flags"] = 0
                    elif case == "missing_typed": f.body["pseudocode"] = None
                    elif case == "missing_helper": f.manifest["shared_tail_helpers"] = []
                    elif case == "duplicate_helper": f.manifest["shared_tail_helpers"] *= 2
                    elif case == "helper_bytes": f.helper["assembly"][0]["bytes"] = "90"
                    elif case == "helper_incoming": f.helper["assembly"][1]["xrefs_to"] = []
                    elif case == "helper_outgoing": f.helper["assembly"][0]["xrefs_from"] = []
                    elif case == "helper_flags": f.helper["flags"] = 0
                    elif case == "helper_pseudocode": f.helper["pseudocode"] = {"lines": [{"text": "invented"}]}
                    elif case == "helper_hash": record["sha256"] = "0" * 64
                    elif case == "helper_path": record["path"] = "shared-tail-helpers/../" + record["path"]
                    if case in ("helper_bytes", "helper_incoming", "helper_outgoing", "helper_flags", "helper_pseudocode"):
                        f.manifest["shared_tail_helpers"] = [f.index("shared-tail-helpers", f.helper, False)]
                    with self.assertRaises(ValueError):
                        adjudication.shared_tail_attempt_coverage(analysis, f.raw, f.capture,
                            f.previous, f.body, f.manifest, f.output)

    def test_failed_attempt_uses_explicit_shared_tail_coverage_and_retains_original_failure(self):
        for secondary in (False, True):
            with self.subTest(secondary=secondary), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory), secondary=secondary)
                rva = f.previous["rva"]
                selection = analysis.native_function_selection(f.raw, f.capture, [int(rva, 16)])
                failed = f.result / "supplements/failed-without-shared-tail"
                previous_ref = f.write(failed / "functions" / (rva[2:] + ".json"), f.previous)
                attempt = copy.deepcopy(f.manifest)
                attempt.update(all_pseudocode_captured=False, function_selection=selection,
                    function_shared_tail_recoveries=[], shared_tail_helpers=[],
                    failures=[{"rva": rva, "stage": "hexrays", "error": "original missing shared tail failure"}],
                    functions=[{**previous_ref, "path": "functions/" + rva[2:] + ".json", "rva": rva,
                        "address": f.previous["address"], "assembly_captured": True, "pseudocode_captured": False}])
                ref = f.write(failed / "capture.json", attempt)
                f.write(failed / "session.json", {"passed": True, "exit_code": 0, "target_execution": False,
                    "finished_at_utc": "2026-10-26T00:00:00Z", "capture_tool": f.manifest["capture_tool"]})
                f.write(failed / "plan.json", {"function_selection": selection, "base_capture": f.manifest["base_capture"],
                    "source_database": f.manifest["source_database"], "input": f.capture["input"],
                    "sha256": f.capture["input_sha256"], "input_bytes": f.capture["input_bytes"], "target_execution": False})
                before = (failed / "capture.json").read_bytes()
                resolution = analysis.native_reanalysis_resolution(f.result)
                with patch.object(adjudication, "merged_attempt_coverage", side_effect=AssertionError("must use shared-tail proof")):
                    proof = adjudication.attempt_coverage(analysis, f.result, resolution, {"supplement": ref["path"]})
                covered = proof["functions"][0]["shared_tail_original_coverage"]
                self.assertEqual(covered["coverage_kind"], "reviewed_complete_shared_tail_owner")
                self.assertNotIn("merged_original_coverage", proof["functions"][0])
                self.assertEqual(proof["original_failures"], attempt["failures"])
                self.assertEqual((failed / "capture.json").read_bytes(), before)


if __name__ == "__main__":
    unittest.main()

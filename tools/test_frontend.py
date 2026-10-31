"""Pure tests for byte-preserving incremental frontend capture."""
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
import urllib.error
from unittest.mock import patch

import analysis
import evidence
import frontend
import host

sys.path.insert(0, str(analysis.ROOT / "tools/analyzers/python-static"))


class DeploymentObservationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        change = patch.object(analysis, "ROOT", self.root)
        change.start()
        self.addCleanup(change.stop)
        self.url = "https://apps.razer.com/synapse/products/756/mw/index.html"
        self.original = self.write_original("current", b"current source")
        self.original["url"] = self.url
        self.acquired = {"recorded_at_utc": "2026-10-30T10:00:00+00:00"}

    def write_original(self, name, raw):
        path = self.root / ".analysis/inputs/frontend-revalidation/blobs" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest()}

    def write_report(self, name, time, original, *, status=200, classification="unchanged"):
        path = self.root / ".analysis/results/frontend-revalidation" / (name + ".json")
        path.parent.mkdir(parents=True, exist_ok=True)
        rounds = [{"round": number, "url": self.url, "final_url": self.url,
                   "status": "ok" if status == 200 else "http_error", "http_status": status,
                   "bytes": original["bytes"], "sha256": original["sha256"]} for number in (1, 2)]
        path.write_text(json.dumps({"kind": "immutable_current_frontend_deployment_revalidation",
            "schema_version": 1, "target_execution": False, "created_at_utc": time,
            "observations": [{"url": self.url, "classification": classification,
                              "rounds": rounds, "observed_original": original}]}), encoding="utf-8")
        return path

    def test_latest_success_supersedes_earlier_failed_observation_without_rewriting_it(self):
        earlier = self.write_report("earlier", "2026-10-30T11:00:00+00:00", self.original, status=503)
        unchanged = earlier.read_bytes()
        self.write_report("latest", "2026-10-30T12:00:00+00:00", self.original,
                          classification="changed_or_new_deployment")
        result = frontend.verify_latest_deployment_observations(self.acquired, [self.original], host.HostLocalAudit())
        self.assertTrue(result["passed"])
        self.assertEqual(result["comparisons"][0]["status"], "matches_latest_http_bytes")
        self.assertEqual(earlier.read_bytes(), unchanged)

    def test_later_failure_drift_and_same_time_conflicts_block_selected_old_source(self):
        changed = self.write_original("changed", b"a changed deployment")
        for case in ("http_failure", "drift", "same_time"):
            with self.subTest(case=case):
                paths = [self.write_report("first", "2026-10-30T12:00:00+00:00",
                         self.original if case == "http_failure" else changed,
                         status=503 if case == "http_failure" else 200)]
                if case == "same_time":
                    paths.append(self.write_report("second", "2026-10-30T12:00:00+00:00", self.original))
                audit = host.HostLocalAudit()
                latest = frontend.latest_deployment_observations(audit, reports=paths)
                with self.assertRaisesRegex(ValueError, "later unresolved deployment"):
                    frontend.verify_latest_deployment_observations(
                        self.acquired, [self.original], audit, observations=latest)

    def test_failed_older_observation_does_not_override_new_verified_acquisition(self):
        self.write_report("old", "2026-10-30T09:00:00+00:00", self.original, status=404)
        result = frontend.verify_latest_deployment_observations(self.acquired, [self.original], host.HostLocalAudit())
        self.assertEqual(result["comparisons"][0]["status"], "no_observation_after_acquisition")

    def test_matching_claim_requires_retained_actual_bytes_and_timezone(self):
        observed = self.write_original("observation", b"current source")
        self.write_report("latest", "2026-10-30T12:00:00+00:00", observed)
        (self.root / observed["path"]).write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "identity differs"):
            frontend.verify_latest_deployment_observations(self.acquired, [self.original], host.HostLocalAudit())
        with self.assertRaisesRegex(ValueError, "timezone"):
            frontend.verify_latest_deployment_observations(
                {"recorded_at_utc": "2026-10-30T10:00:00"}, [self.original], host.HostLocalAudit())

    def test_unchanged_reuse_requires_the_exact_retained_path_and_actual_bytes(self):
        path = self.write_report("reused", "2026-10-30T12:00:00+00:00", self.original,
                                 classification="unchanged_reused")
        report = json.loads(path.read_bytes())
        observation = report["observations"][0]
        observation.pop("observed_original")
        observation["retained_original"] = self.original
        observation["reused_path"] = self.original["path"]
        path.write_text(json.dumps(report), encoding="utf-8")
        self.assertTrue(frontend.verify_latest_deployment_observations(
            self.acquired, [self.original], host.HostLocalAudit())["passed"])
        observation["reused_path"] = ".analysis/inputs/unrelated.js"
        path.write_text(json.dumps(report), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "later unresolved deployment"):
            frontend.verify_latest_deployment_observations(self.acquired, [self.original], host.HostLocalAudit())

    def test_reference_sidecars_are_separate_from_http_observation_receipts(self):
        observed = self.write_report("latest", "2026-10-30T12:00:00+00:00", self.original)
        sidecar = observed.with_name("references.json")
        sidecar.write_text(json.dumps({"kind":"current_deployment_reference_edges", "schema_version":1,
                                      "target_execution":False, "edges":[]}), encoding="utf-8")
        self.assertTrue(frontend.verify_latest_deployment_observations(
            self.acquired, [self.original], host.HostLocalAudit())["passed"])
        report = json.loads(observed.read_bytes())
        report["target_execution"] = True
        observed.write_text(json.dumps(report), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "schema or execution boundary"):
            frontend.verify_latest_deployment_observations(self.acquired, [self.original], host.HostLocalAudit())


class IncrementalCaptureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.addCleanup(patch.stopall)
        patch.object(analysis, "ROOT", self.root).start()
        self.input = self.root / ".analysis/inputs/frontend/example"
        self.output = self.root / ".analysis/results/frontend/example"
        self.source = self.input / "raw/site/old.js"
        self.tool = self.root / "tools/wakaru.exe"
        self.write(self.source, b"old()")
        self.write(self.tool, b"static analyzer fixture")
        self.write(self.output / "modules/old.js", b"old();\n")
        self.write(self.output / "factory-recovery-tool.py", b"original recipe")
        self.provenance = self.output / "modules/provenance.json"
        analysis.save(self.provenance, {"modules": {"old.js": {
            "input": "site/old.js", "ranges": [[0, 5]], "extraction": "heuristic"}}})
        self.old = self.identity(self.source)
        self.receipt = self.output / "analysis.json"
        self.original = {"schema_version": 1, "tool": {"sha256": analysis.sha(self.tool)},
            "original_inputs": [self.old], "original_js_inputs": [self.old["path"]],
            "commands": [], "outputs": [self.identity(path) for path in
                (self.output / "modules/old.js", self.provenance, self.output / "factory-recovery-tool.py")],
            "module_recovery": {"registered_factories": 3, "recovered_modules": 3,
                "registrations": [], "factories": [], "scan_failures": [], "missing_factories": []}}
        analysis.save(self.receipt, self.original)
        self.rows = [self.old]
        self.acquisition()

    def write(self, path, raw):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)

    def identity(self, path):
        raw = path.read_bytes()
        return {"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest()}

    def acquisition(self):
        analysis.save(self.input / "acquisition.json", {"files": self.rows})

    def add(self, name, raw):
        path = self.input / "raw/site" / name
        self.write(path, raw)
        self.rows.append(self.identity(path))
        self.acquisition()
        return path

    def fake_unpack(self, argv, *, cwd, **kwargs):
        destination = Path(argv[argv.index("-o") + 1])
        destination.mkdir(parents=True)
        inputs = argv[1:argv.index("--unpack")]
        modules = {}
        for index, name in enumerate(inputs):
            source = (cwd / name).resolve()
            target = destination / (str(index) + ".js")
            self.write(target, source.read_bytes() + b";\n")
            modules[target.name] = {"input": name, "ranges": [[0, source.stat().st_size]],
                                    "extraction": "heuristic"}
        analysis.save(destination / "provenance.json", {"modules": modules})
        return SimpleNamespace(returncode=0)

    def extend(self):
        return frontend.extend_capture(self.input, self.tool, host.HostLocalAudit())

    def test_non_javascript_dependency_preserves_every_reused_byte(self):
        self.add("image.svg", b"<svg/>")
        old_module = (self.output / "modules/old.js").read_bytes()
        original_raw = self.receipt.read_bytes()
        with patch("frontend.subprocess.run") as run:
            self.assertTrue(self.extend())
            run.assert_not_called()
        result = json.loads(self.receipt.read_bytes())
        self.assertEqual(result["module_recovery"], self.original["module_recovery"])
        self.assertEqual(len(result["original_inputs"]), 2)
        retained = result["incremental_captures"][0]["previous_receipt"]
        self.assertEqual((self.root / retained["path"]).read_bytes(), original_raw)
        self.assertEqual((self.output / "modules/old.js").read_bytes(), old_module)
        self.assertFalse(self.extend())

    def test_only_added_javascript_is_passed_to_analyzer(self):
        added = self.add("worker.js", b"newWorker()")
        self.add("settings.json", b"{}")
        with patch("frontend.subprocess.run", side_effect=self.fake_unpack) as run:
            self.assertTrue(self.extend())
        argv = run.call_args.args[0]
        self.assertEqual(argv[1:argv.index("--unpack")], [os.path.relpath(added, self.input / "raw")])
        result = json.loads(self.receipt.read_bytes())
        self.assertEqual(result["pending_previous_module_recovery"], self.original["module_recovery"])
        self.assertNotIn("module_recovery", result)
        self.assertEqual((self.output / "factory-recovery-tool.py").read_bytes(), b"original recipe")
        provenance = json.loads(self.provenance.read_bytes())
        self.assertEqual(len(provenance["modules"]), 2)
        self.assertEqual(provenance["modules"]["old.js"]["input"], "site/old.js")

    def test_changed_original_claim_cannot_be_promoted_as_addition(self):
        self.add("image.svg", b"<svg/>")
        self.rows[0] = {**self.rows[0], "sha256": "0" * 64}
        self.acquisition()
        with self.assertRaisesRegex(ValueError, "replace or remove"):
            self.extend()
        self.assertEqual(json.loads(self.receipt.read_bytes()), self.original)

    def test_indexed_mjs_omitted_by_an_older_capture_is_actually_analyzed(self):
        added = self.add("config.mjs", b"export const value=1")
        capture = json.loads(self.receipt.read_bytes())
        capture["original_inputs"].append(self.rows[-1])
        analysis.save(self.receipt, capture)
        with patch("frontend.subprocess.run", side_effect=self.fake_unpack) as run:
            self.assertTrue(self.extend())
        argv = run.call_args.args[0]
        self.assertEqual(argv[1:argv.index("--unpack")], [os.path.relpath(added, self.input / "raw")])
        current = json.loads(self.receipt.read_bytes())
        self.assertEqual(current["incremental_captures"][-1]["added_originals"], [])
        self.assertIn(self.rows[-1]["path"], current["original_js_inputs"])

    def provider(self):
        source_dir = self.root / ".analysis/inputs/frontend/provider"
        output = self.root / ".analysis/results/frontend/provider"
        script = source_dir / "raw/site/worker.js"
        self.write(script, b"providerWorker()")
        row = self.identity(script)
        analysis.save(source_dir / "acquisition.json", {"source_current_during_acquisition": True,
            "files": [{**row, "url": "https://apps.razer.com/site/worker.js"}]})
        self.write(output / "modules/worker.js", b"providerWorker();\n")
        self.write(output / "recipe.py", b"verified provider recipe")
        capture = {"input": source_dir.relative_to(self.root).as_posix(), "input_bytes_unchanged": True,
            "tool": {"sha256": analysis.sha(self.tool)}, "original_inputs": [row],
            "original_js_inputs": [row["path"]],
            "outputs": [self.identity(output / "modules/worker.js"), self.identity(output / "recipe.py")],
            "module_recovery": {"recipe": self.identity(output / "recipe.py"),
                "registered_factories": 10, "recovered_modules": 10, "missing_factories": [], "scan_failures": []}}
        path = output / "analysis.json"
        analysis.save(path, capture)
        self.rows.append({**row, "url": "https://apps.razer.com/site/worker.js"})
        self.acquisition()
        return path, capture, row

    def test_cross_source_capture_is_reused_without_copy_or_analyzer(self):
        path, capture, row = self.provider()
        with patch("frontend.subprocess.run") as run:
            self.assertTrue(self.extend())
        run.assert_not_called()
        result = json.loads(self.receipt.read_bytes())
        self.assertEqual(result["module_recovery"], self.original["module_recovery"])
        self.assertEqual(result["borrowed_captures"][0]["originals"], [row])
        self.assertEqual(result["borrowed_captures"][0]["capture"], self.identity(path))
        self.assertEqual(result["incremental_captures"][0]["analyzed_scripts"], [])
        self.assertFalse((self.output / "modules/increments").exists())
        inputs = frontend.verify_borrowed_capture(result["borrowed_captures"][0], self.tool, host.HostLocalAudit())
        self.assertEqual(inputs, {row["path"]})

    def test_incomplete_or_wrong_scope_provider_is_rejected_without_mutating_owner(self):
        path, captured, row = self.provider()
        for mutation in ("missing_factory", "scan_failure", "nested", "changed_owner", "wrong_tool", "stale_scope"):
            with self.subTest(mutation=mutation):
                fixture = json.loads(json.dumps(captured))
                if mutation == "missing_factory":
                    fixture["module_recovery"]["missing_factories"] = [1]
                elif mutation == "scan_failure":
                    fixture["module_recovery"]["scan_failures"] = [{"error": "unresolved"}]
                elif mutation == "nested":
                    fixture["borrowed_captures"] = [{"capture": self.identity(self.receipt)}]
                elif mutation == "changed_owner":
                    fixture["input"] = ".analysis/inputs/frontend/another-owner"
                elif mutation == "wrong_tool":
                    fixture["tool"]["sha256"] = "0" * 64
                else:
                    fixture["input_bytes_unchanged"] = False
                analysis.save(path, fixture)
                with self.assertRaises(ValueError):
                    self.extend()
                self.assertEqual(json.loads(self.receipt.read_bytes()), self.original)

    def test_borrowed_provider_identity_change_is_detected_on_reuse(self):
        path, capture, row = self.provider()
        self.extend()
        result = json.loads(self.receipt.read_bytes())
        capture["changed"] = True
        analysis.save(path, capture)
        with self.assertRaisesRegex(ValueError, "artifact identity differs"):
            frontend.verify_borrowed_capture(result["borrowed_captures"][0], self.tool, host.HostLocalAudit())

    def test_missing_provider_generated_output_is_not_an_excuse_to_omit_the_script(self):
        path, captured, row = self.provider()
        (path.parent / "modules/worker.js").unlink()
        with self.assertRaises(FileNotFoundError):
            self.extend()
        self.assertEqual(json.loads(self.receipt.read_bytes()), self.original)

    def test_missing_reused_output_is_a_failure(self):
        self.add("image.svg", b"<svg/>")
        (self.output / "modules/old.js").unlink()
        with self.assertRaises(FileNotFoundError):
            self.extend()
        self.assertEqual(json.loads(self.receipt.read_bytes()), self.original)

    def test_failed_analyzer_preserves_the_original_capture(self):
        self.add("worker.js", b"newWorker()")
        with patch("frontend.subprocess.run", return_value=SimpleNamespace(returncode=1)), \
                self.assertRaisesRegex(ValueError, "not completely captured"):
            self.extend()
        self.assertEqual(json.loads(self.receipt.read_bytes()), self.original)

    def test_missing_script_coverage_fails_before_receipt_mutation(self):
        self.add("worker.js", b"newWorker()")
        def omitted(argv, *, cwd, **kwargs):
            destination = Path(argv[argv.index("-o") + 1])
            destination.mkdir(parents=True)
            analysis.save(destination / "provenance.json", {"modules": {}})
            return SimpleNamespace(returncode=0)
        with patch("frontend.subprocess.run", side_effect=omitted), \
                self.assertRaisesRegex(ValueError, "omitted an added"):
            self.extend()
        self.assertEqual(json.loads(self.receipt.read_bytes()), self.original)

    def test_input_mutation_during_analysis_is_detected(self):
        self.add("worker.js", b"newWorker()")
        def changing(argv, **kwargs):
            result = self.fake_unpack(argv, **kwargs)
            self.source.write_bytes(b"changed()")
            return result
        with patch("frontend.subprocess.run", side_effect=changing), \
                self.assertRaisesRegex(ValueError, "changed during"):
            self.extend()
        self.assertEqual(json.loads(self.receipt.read_bytes()), self.original)

    def test_synthetic_source_set_uses_shared_retained_root(self):
        other = self.root / ".analysis/inputs/frontend/other/raw/site/other.js"
        self.write(other, b"other()")
        synthetic = self.root / ".analysis/inputs/frontend/synthetic"
        expected = self.root / ".analysis/inputs/frontend"
        self.assertEqual(frontend.input_root(synthetic, [self.source, other]), expected)
        self.assertFalse((synthetic / "raw").exists())

    def test_input_root_rejects_missing_and_unretained_inputs(self):
        outside = self.root / "unretained.js"
        self.write(outside, b"outside()")
        for inputs in ([], [outside], [self.input / "raw/missing.js"]):
            with self.subTest(inputs=inputs), self.assertRaisesRegex(ValueError, "missing or outside"):
                frontend.input_root(self.input, inputs)


class InlineEntrypointTests(unittest.TestCase):
    def compare(self, left, right):
        return frontend.inline_script_equivalence(left, right, host.HostLocalAudit().parser)

    def test_typeof_string_comparison_and_trailing_comment_are_equivalent(self):
        result = self.compare(b'run("object" === typeof state);\n//# sourceMappingURL=runtime.js.map',
                              b'run("object" == typeof state)')
        self.assertTrue(result["equivalent"])
        self.assertEqual(result["typeof_string_comparisons"], [1, 1])

    def test_other_equality_rewrites_and_changed_semantics_are_rejected(self):
        examples = [
            (b'run(a === b)', b'run(a == b)'),
            (b'run(typeof state === expected)', b'run(typeof state == expected)'),
            (b'run("object" === typeof state)', b'run("string" == typeof state)'),
            (b'run("object" !== typeof state)', b'run("object" == typeof state)'),
            (b'run(first(), second())', b'run(second(), first())'),
            (b'run("//# sourceMappingURL=x")', b'run("//# sourceMappingURL=y")'),
            (b'run()', b'run('),
        ]
        for left, right in examples:
            with self.subTest(left=left, right=right):
                self.assertFalse(self.compare(left, right)["equivalent"])

    def deployment(self, root, body, *, content_type="text/html", inline_type="", different=False):
        folder = root / ".analysis/inputs/frontend/example/raw"
        folder.mkdir(parents=True)
        script = b'run("object" === typeof state);\n//# sourceMappingURL=runtime.js.map'
        if body is None:
            body = ('<!doctype html><script' + inline_type + '>run("' +
                    ('string' if different else 'object') + '" == typeof state)</script>').encode()
        rows = []
        entry = "https://apps.razer.com/example/" if content_type == "text/html" else "https://apps.razer.com/example/devices.json"
        for name, url, raw, media in [
                ("entry.html", entry, body, content_type),
                ("asset-manifest.json", "https://apps.razer.com/example/asset-manifest.json",
                 b'{"files":{"runtime.js":"runtime.js"},"entrypoints":["runtime.js"]}', "application/json"),
                ("runtime.js", "https://apps.razer.com/example/runtime.js", script, "application/javascript")]:
            path = folder / name
            path.write_bytes(raw)
            rows.append({"url": url, "final_url": url, "path": path.relative_to(root).as_posix(),
                         "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), "content_type": media})
        return {"entry_url": entry, "files": rows, "source_current_during_acquisition": True}

    def test_complete_inline_runtime_proof_binds_both_originals_and_byte_range(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            acquired = self.deployment(root, None)
            with patch.object(analysis, "ROOT", root):
                result = evidence.frontend_deployment_sources(acquired)
            self.assertEqual(result["failures"], [])
            self.assertTrue(result["entry_is_html"])
            proof, = result["inlined_manifest_entrypoints"]
            raw = (root / proof["entry_original"]["path"]).read_bytes()
            inline = raw[proof["inline_start_byte"]:proof["inline_end_byte"]]
            self.assertEqual(hashlib.sha256(inline).hexdigest(), proof["inline_sha256"])
            self.assertEqual(result["sources"][0]["deployment_role"], "inline_equivalent_original")

    def test_changed_inline_runtime_keeps_failure_and_comparison_facts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            acquired = self.deployment(root, None, different=True)
            with patch.object(analysis, "ROOT", root):
                result = evidence.frontend_deployment_sources(acquired)
            self.assertIn("manifest_entrypoint_missing_from_html", [row["kind"] for row in result["failures"]])
            comparison, = result["inline_entrypoint_comparisons"]
            self.assertFalse(comparison["equivalent"])
            self.assertEqual(result["inlined_manifest_entrypoints"], [])

    def test_non_executable_inline_script_cannot_satisfy_a_runtime_entrypoint(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            acquired = self.deployment(root, None, inline_type=' type="application/json"')
            with patch.object(analysis, "ROOT", root):
                result = evidence.frontend_deployment_sources(acquired)
            self.assertIn("manifest_entrypoint_missing_from_html", [row["kind"] for row in result["failures"]])
            self.assertEqual(result["inlined_manifest_entrypoints"], [])

    def test_json_catalog_does_not_acquire_html_script_requirements(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            acquired = self.deployment(root, b'[{"productId":747}]', content_type="application/json")
            with patch.object(analysis, "ROOT", root):
                result = evidence.frontend_deployment_sources(acquired)
            self.assertFalse(result["entry_is_html"])
            self.assertEqual(result["failures"], [])
            self.assertEqual(result["sources"][0]["deployment_role"], "conditional_or_declared_candidate")


class CurrentSourceSelectionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", self.root)
        patched.start()
        self.addCleanup(patched.stop)
        self.entry = "https://apps.razer.com/synapse/studio/"
        self.candidates = []

    def source(self, name, date, *, entry=None, script=b"run();", stable=True):
        entry = self.entry if entry is None else entry
        folder = self.root / ".analysis/inputs/frontend" / name
        rows = []
        for filename, url, raw, media in [
                ("index.html", entry, b'<html><script src="main.js"></script></html>', "text/html"),
                ("main.js", "https://apps.razer.com/synapse/studio/main.js", script, "application/javascript")]:
            path = folder / "raw" / filename
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            rows.append({"path": path.relative_to(self.root).as_posix(), "url": url,
                         "bytes": len(raw), "sha256": analysis.sha(path), "content_type": media})
        acquired = {"entry_url": entry, "recorded_at_utc": date, "files": rows, "edges": [],
                    "source_current_during_acquisition": stable, "target_execution": False,
                    "stability": [{"url": entry, "unchanged": stable,
                                   "before_sha256": rows[0]["sha256"], "after_sha256": rows[0]["sha256"]}]}
        receipt = folder / "acquisition.json"
        analysis.save(receipt, acquired)
        self.candidates.append(receipt)
        return acquired

    def select(self, audit=None):
        return frontend.current_source_selection(self.entry, audit or host.HostLocalAudit(), candidates=self.candidates)

    def test_newest_stable_route_selected_and_unstable_or_foreign_sources_kept_separate(self):
        self.source("old", "2026-10-15T00:00:00+00:00")
        self.source("latest", "2026-10-26T00:00:00+00:00", entry=self.entry + "index.html", script=b"newRun();")
        self.source("changed", "2026-10-27T00:00:00+00:00", stable=False)
        self.source("foreign", "2026-10-28T00:00:00+00:00", entry="https://example.com/synapse/studio/")
        result = self.select()
        self.assertEqual(result["name"], "latest")
        self.assertEqual(len(result["originals"]), 1)
        self.assertEqual(result["capture_failures"], ["complete_capture_unavailable"])
        self.assertEqual({row["name"]: row["status"] for row in result["candidate_decisions"]}, {
            "old": "superseded_stable_acquisition", "latest": "selected_latest_stable_acquisition",
            "changed": "not_a_stable_current_acquisition", "foreign": "different_or_unverified_deployment_route"})

    def test_corrupt_latest_original_never_falls_back_to_old_deployment(self):
        self.source("old", "2026-10-15T00:00:00+00:00")
        acquired = self.source("latest", "2026-10-26T00:00:00+00:00")
        (self.root / acquired["files"][1]["path"]).write_bytes(b"changed();")
        with self.assertRaisesRegex(ValueError, "Newest frontend deployment failed"):
            self.select()

    def test_same_time_conflicting_dependency_bytes_fail_even_when_entry_matches(self):
        self.source("left", "2026-10-26T00:00:00+00:00")
        self.source("right", "2026-10-26T08:00:00+08:00", script=b"changed();")
        with self.assertRaisesRegex(ValueError, "Same-time.*conflicting URL bytes"):
            self.select()

    def test_failed_capture_is_returned_truthfully_and_is_not_replaced_by_old(self):
        self.source("old", "2026-10-15T00:00:00+00:00")
        self.source("latest", "2026-10-26T00:00:00+00:00")
        capture = self.root / ".analysis/results/frontend/latest/analysis.json"
        capture.parent.mkdir(parents=True)
        capture.write_bytes(b"{}")
        audit = host.HostLocalAudit()
        failed = {"capture_failures": [{"stage": "validate", "exit_code": 1}], "successful_full_capture": False}
        with patch.object(audit, "frontend_capture", return_value=failed) as verify:
            result = self.select(audit)
        self.assertEqual(result["name"], "latest")
        self.assertEqual(result["capture"], failed)
        self.assertEqual(result["capture_failures"], failed["capture_failures"])
        verify.assert_called_once_with("latest", result["originals"])

    def test_capture_identity_failure_never_falls_back(self):
        self.source("old", "2026-10-15T00:00:00+00:00")
        self.source("latest", "2026-10-26T00:00:00+00:00")
        capture = self.root / ".analysis/results/frontend/latest/analysis.json"
        capture.parent.mkdir(parents=True)
        capture.write_bytes(b"{}")
        audit = host.HostLocalAudit()
        with patch.object(audit, "frontend_capture", side_effect=ValueError("changed output")), \
                self.assertRaisesRegex(ValueError, "changed output"):
            self.select(audit)

    def test_deployment_only_selection_verifies_originals_without_claiming_capture_validation(self):
        acquired = self.source("latest", "2026-10-26T00:00:00+00:00")
        capture = self.root / ".analysis/results/frontend/latest/analysis.json"
        capture.parent.mkdir(parents=True)
        capture.write_bytes(b"not a valid capture")
        audit = host.HostLocalAudit()
        with patch.object(audit, "frontend_capture") as verify:
            result = frontend.current_source_selection(self.entry, audit, candidates=self.candidates, verify_capture=False)
        verify.assert_not_called()
        self.assertIsNone(result["capture"])
        self.assertEqual(result["capture_failures"], ["capture_not_requested"])
        self.assertEqual(result["name"], "latest")
        (self.root / acquired["files"][1]["path"]).write_bytes(b"changed();")
        with self.assertRaisesRegex(ValueError, "Newest frontend deployment failed"):
            frontend.current_source_selection(self.entry, host.HostLocalAudit(), candidates=self.candidates,
                                              verify_capture=False)

    def test_route_normalization_does_not_merge_queries_or_neighbors(self):
        self.assertEqual(frontend.canonical_frontend_entry(self.entry.rstrip("/")), self.entry)
        self.assertEqual(frontend.canonical_frontend_entry(self.entry + "index.html"), self.entry)
        self.assertNotEqual(frontend.canonical_frontend_entry(self.entry + "?v=2"), self.entry)
        for url in ["http://apps.razer.com/synapse/studio/", "https://apps.razer.com.evil.example/synapse/studio/",
                    "https://apps.razer.com/synapse/../studio/", "https://apps.razer.com/synapse/studio/#x"]:
            with self.subTest(url=url), self.assertRaises(ValueError):
                frontend.canonical_frontend_entry(url)


class LexicalContextTests(unittest.TestCase):
    def test_whole_script_preserves_every_original_byte_and_complete_syntax(self):
        parser = host.HostLocalAudit().parser
        originals = [
            b'!function(e,t){"object"==typeof exports?t(exports,require("react")):t(e.result={},e.React)}(this,function(e,t){const o=t; e.value=o;});',
            b'(()=>{const table={284:(t,e,n)=>{t=n.hmd(t); e.value=()=>t.require("process");}};return table;})();',
            'function f(){const s="\u2713"; const t=`a${s};{}`;return /[;}]/.test(t);}'.encode(),
            b'function f(){return\n{x:1}}\nlet value=1\nvalue++\nuse(value)',
            b'function f(){let x=1;// keep comment\nreturn x/* block */+2}',
        ]
        for raw in originals:
            with self.subTest(raw=raw):
                readable, proof = frontend.readable_lexical_script(raw, parser)
                inserted = {offset + index for index, offset in enumerate(proof["inserted_lf_original_offsets"])}
                restored = bytes(byte for index, byte in enumerate(readable) if index not in inserted)
                self.assertEqual(restored, raw)
                self.assertEqual(frontend.syntax_identity(parser.parse(raw).root_node),
                                 frontend.syntax_identity(parser.parse(readable).root_node))
                self.assertTrue(all(readable[index] == 10 for index in inserted))

    def test_syntax_identity_detects_operator_literal_and_order_changes(self):
        parser = host.HostLocalAudit().parser
        baseline = frontend.syntax_identity(parser.parse(b'run(a===b,"first",second())').root_node)
        for changed in [b'run(a==b,"first",second())', b'run(a===b,"other",second())', b'run(a===b,second(),"first")']:
            with self.subTest(changed=changed):
                self.assertNotEqual(baseline, frontend.syntax_identity(parser.parse(changed).root_node))

    def test_incomplete_original_cannot_be_recovered_as_complete_context(self):
        with self.assertRaisesRegex(ValueError, "unresolved JavaScript syntax"):
            frontend.readable_lexical_script(b'function f(){', host.HostLocalAudit().parser)

    def test_failed_esm_receipt_and_diagnostics_remain_failed_after_context_recovery(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            input_dir = root / ".analysis/inputs/frontend/context"
            output = root / ".analysis/results/frontend/context"
            original = input_dir / "raw/umd.js"
            original.parent.mkdir(parents=True)
            original.write_bytes(b'!function(e,t){t(e.result={},e.React)}(this,function(e,t){const o=t;e.value=o;});')
            target = output / "modules/module.js"
            target.parent.mkdir(parents=True)
            target.write_bytes(b'const o = t;\nexports.value = o;\n')
            validation = output / "validate.stdout.txt"
            finding = {"filename": "module.js", "line": 1, "column": 11, "kind": "unresolved_reference", "message": "lost t"}
            analysis.save(validation, {"findings": [finding], "inputs": 1, "modules": 1})
            provenance = output / "modules/provenance.json"
            analysis.save(provenance, {"modules": {"module.js": {"input": "umd.js", "ranges": [[0, original.stat().st_size]], "extraction": "structural"}}})
            def identity(path):
                return {"path": path.relative_to(root).as_posix(), "bytes": path.stat().st_size, "sha256": analysis.sha(path)}
            command = {"stage": "validate", "exit_code": 1, "stdout": validation.relative_to(root).as_posix()}
            captured = {"original_inputs": [identity(original)], "outputs": [identity(p) for p in (target, validation, provenance)], "commands": [command]}
            receipt = output / "analysis.json"
            analysis.save(receipt, captured)
            previous_raw = receipt.read_bytes()
            with patch.object(analysis, "ROOT", root):
                result = frontend.recover_validation_context(input_dir, host.HostLocalAudit())
                updated = json.loads(receipt.read_bytes())
                self.assertEqual(result["diagnostics_covered"], 1)
                self.assertFalse(result["derived_esm_validation_passed"])
                self.assertEqual(updated["commands"], [command])
                self.assertEqual(updated["lexical_context_recovery"]["scripts"][0]["failed_modules"]["module.js"]["diagnostics"], [finding])
                retained = updated["lexical_context_recovery"]["previous_receipt"]
                self.assertEqual((root / retained["path"]).read_bytes(), previous_raw)
                self.assertEqual(frontend.recover_validation_context(input_dir, host.HostLocalAudit())["status"], "reused_verified_lexical_contexts")
                original.write_bytes(b'changed();')
                with self.assertRaisesRegex(ValueError, "artifact identity differs"):
                    frontend.recover_validation_context(input_dir, host.HostLocalAudit())


class InstalledCacheProvenanceTests(unittest.TestCase):
    def fixture(self, root, *, wrong_slice=False, promoted=False):
        cached = root / ".analysis/inputs/frontend/installed-example"
        official = root / ".analysis/inputs/frontend/example-current"
        installed = root / ".analysis/inputs/installed-current"
        url = "https://apps.razer.com/synapse/example/index.html"
        body = b"<html>old</html>"
        def write(path, raw):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(raw), "sha256": analysis.sha(path)}
        container = write(installed / "cache/entries/body_0", b"header" + body + b"footer")
        resource = {**write(installed / "cache/resources/index.html", body), "url": url, "canonical_url": url,
                    "container": container["path"], "container_sha256": container["sha256"],
                    "body_offset": 7 if wrong_slice else 6, "body_bytes": len(body),
                    "cache_crc_verified": True, "deployment_current_verified": False}
        row = {**resource, **write(cached / "raw/index.html", body), "cached_url": url}
        cache_data = {"source": ".analysis/inputs/installed-current/acquisition.json", "files": [row],
                      "source_current_during_acquisition": promoted, "target_execution": False}
        analysis.save(cached / "acquisition.json", cache_data)
        analysis.save(installed / "acquisition.json", {"target_execution": False, "source_current_during_acquisition": False,
            "files": [container], "resources": [resource], "frontend_groups": [{"input": cached.relative_to(root).as_posix(), "files": 1, "conflicts": 0}]})
        current = {**write(official / "raw/index.html", b"<html>current</html>"), "url": url, "content_type": "text/html"}
        analysis.save(official / "acquisition.json", {"entry_url": url, "recorded_at_utc": "2026-10-26T00:00:00+00:00",
            "source_current_during_acquisition": True, "target_execution": False, "files": [current],
            "stability": [{"url": url, "unchanged": True, "before_sha256": current["sha256"], "after_sha256": current["sha256"]}]})
        return cached, official, cache_data

    def test_actual_container_slice_and_changed_official_entry_remain_distinct(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            cached, official, data = self.fixture(root)
            with patch.object(analysis, "ROOT", root):
                audit = host.HostLocalAudit()
                result = evidence.installed_frontend_provenance(cached, official, audit)
                self.assertEqual(audit.final_snapshot(), [])
            self.assertEqual(result["cache_bodies_verified"], 1)
            self.assertEqual(result["same_url_and_bytes"], 0)
            self.assertEqual(len(result["different_bytes"]), 1)
            self.assertFalse(result["cache_promoted_to_current_deployment"])
            self.assertFalse(data["source_current_during_acquisition"])
            deployment = evidence.frontend_deployment_sources(data)
            self.assertEqual([row["kind"] for row in deployment["failures"]], ["installed_cache_current_deployment_unverified"])

    def test_wrong_container_slice_is_rejected_even_if_all_individual_hashes_match(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            cached, official, _ = self.fixture(root, wrong_slice=True)
            with patch.object(analysis, "ROOT", root), self.assertRaisesRegex(ValueError, "actual container slice"):
                evidence.installed_frontend_provenance(cached, official, host.HostLocalAudit())

    def test_cache_cannot_be_promoted_by_changing_current_flag(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            cached, official, _ = self.fixture(root, promoted=True)
            with patch.object(analysis, "ROOT", root), self.assertRaisesRegex(ValueError, "bounded cache snapshot"):
                evidence.installed_frontend_provenance(cached, official, host.HostLocalAudit())


class HostFrontendEntryTests(unittest.TestCase):
    def fixture(self, root, *, bad_offset=False):
        def write(path, raw):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(raw), "sha256": analysis.sha(path)}
        base = root / ".analysis/inputs/host-current"
        package = {**write(base / "blobs/package.exe", b"package fixture"), "kind": "official_stable_host_package",
                   "declaration": {"name": "Razer App Engine", "version": "4.0.fixture"}}
        texts = {
            "electron/main.js": b'const {getAppHost:I}=require("./lib/getAppHost"),{razerAppPathEnum:R,other:N}=require("./constants"),{createWindow:$o}=require("./components/Tab/common");const Re="https://apps.razer.com";let {gAppHost:ze,appHostCmdline:je}=I(Me,Re);function xn(e){let n;switch(e){case "synapse":n=`${ze}${R.SYNAPSE_PATH}/`;break;}$o(n,e,i);}',
            "electron/constants.js": b'module.exports.razerAppPathEnum={SYNAPSE_PATH:"/synapse/dashboard"};',
            "electron/lib/getAppHost.js": b'module.exports.getAppHost=(args,origin)=>({gAppHost:origin,appHostCmdline:""});',
        }
        archive_raw = b"prefix" + b"".join(texts.values())
        alias = ".analysis/inputs/installed-current/files/app-engine/resources/app.asar"
        archive = {**write(base / "extracted/package/resources/app.asar", archive_raw), "verified_installed_aliases": [alias]}
        sources, offset = [], len(b"prefix")
        for relative, raw in texts.items():
            sources.append({**write(base / "asar/fixture" / relative, raw), "container": alias,
                            "container_sha256": archive["sha256"], "unpacked": False, "integrity_verified": True,
                            "offset": offset + (1 if bad_offset and relative == "electron/main.js" else 0)})
            offset += len(raw)
        manifest = {**write(base / "blobs/installer-manifest.json", b'{"baseURL":"synapse/dashboard/manifests"}'),
                    "url": "https://apps.razer.com/synapse/dashboard/installer-manifest.json"}
        analysis.save(base / "acquisition.json", {"files": [package, manifest], "target_execution": False})
        results = root / ".analysis/results/host-current"
        analysis.save(results / "inspection.json", {"containers": [{"input": package["path"], "sha256": package["sha256"],
            "retained_outputs_verified": True, "input_bytes_unchanged": True, "files": [archive]}]})
        analysis.save(results / "javascript/capture.json", {"inputs": sources, "input_bytes_unchanged": True, "target_execution": False})

    def test_default_dashboard_route_is_bound_to_complete_original_host_owners(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            self.fixture(root)
            with patch.object(analysis, "ROOT", root):
                audit = host.HostLocalAudit()
                result = evidence.host_frontend_entry_provenance(audit)
                self.assertEqual(audit.final_snapshot(), [])
            self.assertEqual(result["entry_url"], "https://apps.razer.com/synapse/dashboard/")
            self.assertEqual(len(result["owners"]), 10)
            self.assertFalse(result["root_route_is_actual_entry"])
            self.assertIn('"./constants"', result["lexical_import_bindings"]["R"]["body"])

    def test_misattributed_asar_slice_fails_despite_matching_source_file_hash(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            self.fixture(root, bad_offset=True)
            with patch.object(analysis, "ROOT", root), self.assertRaisesRegex(ValueError, "official ASAR byte slice"):
                evidence.host_frontend_entry_provenance(host.HostLocalAudit())


class FrozenFrontendRecheckTests(unittest.TestCase):
    def fixture(self, root, *, requested="extra.js", stability_failure=False):
        folder = root / ".analysis/inputs/frontend/example-current"
        entry = "https://apps.razer.com/synapse/example/"

        def write(path, raw):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(raw), "sha256": analysis.sha(path)}

        html = write(folder / "raw/index.html", b'<html><script src="main.js"></script><script src="extra.js"></script></html>')
        script = write(folder / "raw/main.js", b'function View(){return ui.jsx("div",{children:"x"})} View();')
        files = [{**html, "url": entry, "content_type": "text/html"},
                 {**script, "url": entry + "main.js", "content_type": "application/javascript"}]
        acquired = {"entry_url": entry, "files": files, "target_execution": False,
                    "source_current_during_acquisition": True, "edges": [],
                    "stability": [{"url": entry, "unchanged": True,
                                   "before_sha256": html["sha256"], "after_sha256": html["sha256"]}]}
        acquisition_path = folder / "acquisition.json"
        analysis.save(acquisition_path, acquired)
        acquisition = {"path": acquisition_path.relative_to(root).as_posix(),
                       "bytes": acquisition_path.stat().st_size, "sha256": analysis.sha(acquisition_path)}
        missing_url = entry + requested
        old = {"acquisition": acquisition, "target_execution": False,
               "deployment": evidence.frontend_deployment_sources(acquired),
               "failures": [{"kind": "deployment_original_unavailable", "url": missing_url},
                            {"kind": "source_identity_unverified", "source": {"url": missing_url}}]}
        if stability_failure:
            old["failures"].append({"kind": "current_deployment_stability_unverified"})
        receipt_path = root / ".analysis/results/reverse-pending/verified-frontend/example-current.json"
        analysis.save(receipt_path, old)
        receipt = {"path": receipt_path.relative_to(root).as_posix(), "bytes": receipt_path.stat().st_size,
                   "sha256": analysis.sha(receipt_path)}
        for path, identity in [(acquisition_path, acquisition), (receipt_path, receipt)]:
            write(root / ".analysis/results/reverse-pending/review-sources" / (identity["sha256"] + ".json"), path.read_bytes())
        extra = write(folder / "raw/extra.js", b"extra();")
        files.append({**extra, "url": entry + "extra.js", "content_type": "application/javascript"})
        analysis.save(acquisition_path, acquired)
        current = {"acquisition": {"path": acquisition["path"], "bytes": acquisition_path.stat().st_size,
                                   "sha256": analysis.sha(acquisition_path)},
                   "target_execution": False, "bundles": []}
        analysis.save(receipt_path, current)
        return {"kind": "frontend", "id": "fixture-task", "input": folder.relative_to(root).as_posix(),
                "receipt": receipt, "acquisition": acquisition}, script, current, receipt_path

    def test_exact_url_repair_uses_frozen_receipt_and_preserves_every_file_without_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                task, _, _, _ = self.fixture(root)
                before = {path.relative_to(root): path.read_bytes() for path in root.rglob("*") if path.is_file()}
                with patch("subprocess.run", side_effect=AssertionError("Target execution is forbidden")):
                    result = frontend.recheck_frozen_frontend(task)
                after = {path.relative_to(root): path.read_bytes() for path in root.rglob("*") if path.is_file()}
            self.assertEqual(result["status"], "passed")
            self.assertEqual(result["historical_receipt"]["sha256"], task["receipt"]["sha256"])
            self.assertIn("review-sources", result["historical_receipt"]["path"])
            self.assertTrue(result["acquisition_changed_since_frozen_task"])
            self.assertTrue(all(row["resolved"] for row in result["failure_dispositions"]))
            self.assertFalse(result["semantics_complete"])
            self.assertEqual(before, after)

    def test_similarly_named_url_cannot_resolve_the_frozen_url(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                task, _, _, _ = self.fixture(root, requested="extra.js?v=old")
                result = frontend.recheck_frozen_frontend(task)
            self.assertEqual(result["status"], "unresolved")
            self.assertEqual(len(result["failures"]), 2)

    def test_old_stability_failure_remains_when_only_missing_originals_were_repaired(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                task, _, _, _ = self.fixture(root, stability_failure=True)
                result = frontend.recheck_frozen_frontend(task)
            self.assertEqual(result["status"], "unresolved")
            self.assertEqual(result["failures"], [{"kind": "current_deployment_stability_unverified"}])

    def test_changed_original_bytes_fail_before_a_missing_reference_is_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                task, script, _, _ = self.fixture(root)
                (root / script["path"]).write_bytes(b"changed();")
                with self.assertRaisesRegex(ValueError, "artifact identity differs"):
                    frontend.recheck_frozen_frontend(task)

    def test_ast_with_valid_self_reported_hash_cannot_omit_real_elements(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                task, script, current, receipt_path = self.fixture(root)
                raw = (root / script["path"]).read_bytes()
                tree = {"input_sha256": script["sha256"], "input_bytes": len(raw), "target_execution": False,
                        **evidence.ui_tree(raw)}
                self.assertGreater(len(tree["elements"]), 0)
                tree["elements"] = []
                tree_path = root / ".analysis/results/frontend/ui-trees" / (script["sha256"] + ".json")
                analysis.save(tree_path, tree)
                current["bundles"].append({"input": script, "tree": {"path": tree_path.relative_to(root).as_posix(),
                    "bytes": tree_path.stat().st_size, "sha256": analysis.sha(tree_path)}})
                analysis.save(receipt_path, current)
                with self.assertRaisesRegex(ValueError, "AST differs from complete original syntax"):
                    frontend.recheck_frozen_frontend(task)

    def test_missing_frozen_receipt_is_rejected_instead_of_trusting_current_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                task, _, _, _ = self.fixture(root)
                (root / ".analysis/results/reverse-pending/review-sources" / (task["receipt"]["sha256"] + ".json")).unlink()
                with self.assertRaisesRegex(ValueError, "Exact frozen frontend bytes are unavailable"):
                    frontend.recheck_frozen_frontend(task)


class FrozenFrontendAcquisitionTests(unittest.TestCase):
    def fixture(self, root):
        original = root / ".analysis/inputs/frontend/installed-example/raw/body.json"
        original.parent.mkdir(parents=True)
        original.write_bytes(b'{"version":1}')
        identity = {"path": original.relative_to(root).as_posix(), "bytes": original.stat().st_size,
                    "sha256": analysis.sha(original)}
        master = root / ".analysis/results/reverse-pending.json"
        analysis.save(master, {"tasks": []})
        url = "https://apps.razer.com/synapse/example/body.json"
        plan = {"frozen_tasks": {"path": master.relative_to(root).as_posix(), "bytes": master.stat().st_size,
                                 "sha256": analysis.sha(master)}, "source_receipts": [],
                "comparisons": [{"url": url, "cache_references": [{"original": identity}],
                                 "retained_official": None, "other_official_identities": []}]}
        return original, url, plan

    def run_acquisition(self, root, responses):
        original, url, plan = self.fixture(root)

        class Response:
            status = 200
            headers = {"Content-Type": "application/json"}

            def __init__(self, body):
                self.body = body

            def __enter__(self):
                return self

            def __exit__(self, *args):
                return False

            def read(self):
                return self.body

            def geturl(self):
                return url

        def open_response(request, timeout):
            self.assertEqual(request.full_url, url)
            self.assertEqual(request.get_header("Cache-control"), "no-cache")
            item = responses.pop(0)
            if isinstance(item, Exception):
                raise item
            return Response(item)

        opener = unittest.mock.Mock()
        opener.open.side_effect = open_response
        with patch.object(frontend, "_frozen_frontend_cache_plan", return_value=plan), \
                patch("urllib.request.build_opener", return_value=opener), \
                patch.object(analysis, "index_files") as index:
            report = frontend.acquire_frozen_frontend_sources(SimpleNamespace(workers=2))
            self.assertTrue(index.called)
        return original, report

    def test_two_official_rounds_reuse_identical_cached_bytes_without_copying(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                original, report = self.run_acquisition(root, [b'{"version":1}', b'{"version":1}'])
            row = report["files"][0]
            self.assertTrue(row["stable_exact_url"])
            self.assertEqual(row["path"], original.relative_to(root).as_posix())
            self.assertEqual(len(row["http_observations"]), 2)
            self.assertFalse((root / ".analysis/inputs/current-frontend-dependencies/blobs").exists())

    def test_changed_rounds_keep_both_originals_and_never_claim_stability(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                original, report = self.run_acquisition(root, [b'{"version":1}', b'{"version":2}'])
            row = report["files"][0]
            self.assertFalse(row["stable_exact_url"])
            self.assertEqual(original.read_bytes(), b'{"version":1}')
            self.assertEqual((root / row["path"]).read_bytes(), b'{"version":2}')
            self.assertNotEqual(*[item["sha256"] for item in row["http_observations"]])

    def test_http_404_is_retained_as_two_failed_observations(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            url = "https://apps.razer.com/synapse/example/body.json"
            failures = [urllib.error.HTTPError(url, 404, "Not Found", {}, None) for _ in range(2)]
            with patch.object(analysis, "ROOT", root):
                _, report = self.run_acquisition(root, failures)
            row = report["files"][0]
            self.assertFalse(row["stable_exact_url"])
            self.assertNotIn("path", row)
            self.assertEqual([item["http_status"] for item in row["http_observations"]], [404, 404])


class FrontendRuntimeClosureTests(unittest.TestCase):
    def source(self, root, *, include_asset=True, optional_declared=False, css_owner=b""):
        folder = root / ".analysis/inputs/frontend/runtime-current/raw"
        folder.mkdir(parents=True)
        entry = "https://apps.razer.com/synapse/example/"
        raw = (b'function runtime(x){return x;}runtime.u=e=>"static/"+({1:"one"}[e]||e)+"."+{1:"hash"}[e]+".js";'
               + css_owner + b'runtime.p="./";function unrelated(runtime){runtime.p="https://other.example/";}const record={};record.u=[];')
        path = folder / "main.js"
        path.write_bytes(raw)
        main = {"path": path.relative_to(root).as_posix(), "bytes": len(raw), "sha256": analysis.sha(path), "url": entry + "main.js"}
        files = [main]
        if include_asset:
            asset = folder / "one.hash.js"
            asset.write_bytes(b"registered();")
            files.append({"path": asset.relative_to(root).as_posix(), "bytes": asset.stat().st_size,
                          "sha256": analysis.sha(asset), "url": entry + "static/one.hash.js"})
        probe = entry + "asset-manifest.json"
        files.extend([{"url": entry + "main.js.map", "error": "HTTP 404"}, {"url": probe, "error": "HTTP 404"}])
        acquired = {"entry_url": entry, "files": files, "unresolved": [{"kind": "chunk_enumeration_unavailable", "url": main["url"]}],
                    "probes": [{"url": probe, "declared": False}], "edges": [{"to": probe}] if optional_declared else []}
        return {"acquired": acquired, "files": {row["url"]: row for row in files}}

    def test_literal_runtime_complete_owners_use_actual_binding_and_cover_exact_assets(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                source = self.source(root)
                result = frontend._frontend_runtime_closure(source, host.HostLocalAudit())
            self.assertEqual(result["status"], "verified")
            self.assertEqual(len(result["runtimes"]), 1)
            runtime = result["runtimes"][0]
            self.assertEqual(runtime["public_path"], "./")
            self.assertEqual(runtime["assets"][0]["url"], "https://apps.razer.com/synapse/example/static/one.hash.js")
            self.assertEqual(result["unavailable_source_maps"], 1)
            self.assertEqual(len(result["unavailable_undeclared_probes"]), 1)
            self.assertEqual({owner["property"] for owner in runtime["owners"]}, {"u", "p"})

    def test_empty_css_owner_preserves_binding_and_complete_body_without_inventing_assets(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                source = self.source(root, css_owner=b'runtime.miniCssF=e=>{};')
                result = frontend._frontend_runtime_closure(source, host.HostLocalAudit())
            self.assertEqual(result["status"], "verified")
            runtime = result["runtimes"][0]
            owner = next(owner for owner in runtime["owners"] if owner["property"] == "miniCssF")
            self.assertEqual(owner["body"], "runtime.miniCssF=e=>{}")
            self.assertEqual(owner["disposition"], "complete_empty_css_filename_owner")
            self.assertEqual(len(runtime["assets"]), 1)
            self.assertEqual(runtime["assets"][0]["property"], "u")

    def test_css_owner_with_any_statement_remains_unresolved(self):
        for body in (b'{load()}', b'{return "static/style.css"}', b'{;}', b'{/* incomplete */}'):
            with self.subTest(body=body), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                with patch.object(analysis, "ROOT", root):
                    source = self.source(root, css_owner=b'runtime.miniCssF=e=>' + body + b';')
                    result = frontend._frontend_runtime_closure(source, host.HostLocalAudit())
                self.assertEqual(result["status"], "unresolved")
                self.assertIn("runtime_literal_grammar_unresolved", [row["kind"] for row in result["failures"]])

    def test_empty_javascript_owner_still_requires_a_complete_filename_table(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                source = self.source(root)
                original = source["acquired"]["files"][0]
                path = root / original["path"]
                raw = b'function runtime(x){return x;}runtime.u=e=>{};runtime.p="./";'
                path.write_bytes(raw)
                original.update(bytes=len(raw), sha256=analysis.sha(path))
                result = frontend._frontend_runtime_closure(source, host.HostLocalAudit())
            self.assertEqual(result["status"], "unresolved")
            self.assertIn("runtime_literal_grammar_unresolved", [row["kind"] for row in result["failures"]])

    def test_missing_literal_asset_remains_unresolved(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                result = frontend._frontend_runtime_closure(self.source(root, include_asset=False), host.HostLocalAudit())
            self.assertEqual(result["status"], "unresolved")
            self.assertIn("runtime_literal_dependency_missing", [row["kind"] for row in result["failures"]])

    def test_declared_dependency_cannot_be_hidden_as_an_optional_probe(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                result = frontend._frontend_runtime_closure(self.source(root, optional_declared=True), host.HostLocalAudit())
            self.assertEqual(result["status"], "unresolved")
            self.assertIn("current_nonsourcemap_original_missing", [row["kind"] for row in result["failures"]])

    def test_evaluator_rejects_target_calls_duplicate_tables_and_missing_hashes(self):
        parser = host.HostLocalAudit().parser
        for raw in [b'loadChunk()', b'{1:"a",1:"b"}', b'"x"+{1:"a"}[2]']:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                tree = parser.parse(b"(" + raw + b")")
                node = tree.root_node.named_children[0].named_children[0]
                frontend._literal_chunk_expression(node, "e", 1)


class CacheReplacementCoverageTests(unittest.TestCase):
    def fixture(self, root, *, mutated_round=False, current_raw=None):
        url = "https://apps.razer.com/synapse/assets/src/version.js"
        old_raw = b"export const APP_VERSION='1.0.1';export const APP_NAME='Razer Assets';"
        current_raw = current_raw or b"export const APP_VERSION='1.0.2';export const APP_NAME='Razer Assets';"

        def write(relative, raw):
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            return {"path": relative, "bytes": len(raw), "sha256": analysis.sha(path)}

        old = {**write(".analysis/inputs/frontend/installed-example/raw/version.js", old_raw), "url": url}
        actual = {**write(".analysis/inputs/current-frontend-dependencies/blobs/current/version.js", current_raw), "url": url}
        acquisition = write(".analysis/inputs/frontend/installed-example/acquisition.json", b"{}")
        rounds = [{**actual, "phase": phase, "final_url": url, "http_status": 200} for phase in ("before", "after")]
        if mutated_round:
            rounds[1]["sha256"] = "0" * 64
        supplement = {"target_execution": False, "kind": "official_frontend_dependency_supplement",
                      "comparisons": [{"url": url, "retained_official": None,
                                       "cache_references": [{"cache_acquisition": acquisition, "original": {k: old[k] for k in ("path", "bytes", "sha256")}}]}],
                      "files": [{**actual, "stable_exact_url": True, "http_observations": rounds}]}
        analysis.save(root / ".analysis/inputs/current-frontend-dependencies/acquisition.json", supplement)
        return {"acquisition": acquisition, "acquired": {"files": [old]}}, {"files": {}}

    def test_whole_version_record_is_replaced_explicitly_without_claiming_same_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                source, comparison = self.fixture(root)
                result = frontend._cache_current_replacement(source, comparison, host.HostLocalAudit())
            self.assertEqual(result["status"], "verified")
            self.assertEqual(result["same_bytes"], 0)
            changed = result["comparisons"][0]["version_difference"]
            self.assertEqual(changed["previous"]["APP_VERSION"], "1.0.1")
            self.assertEqual(changed["current"]["APP_VERSION"], "1.0.2")

    def test_stability_boolean_cannot_hide_different_actual_round_hashes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                source, comparison = self.fixture(root, mutated_round=True)
                with self.assertRaisesRegex(ValueError, "HTTP observations differ"):
                    frontend._cache_current_replacement(source, comparison, host.HostLocalAudit())

    def test_version_file_with_additional_business_logic_is_not_metadata_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(analysis, "ROOT", root):
                source, comparison = self.fixture(root, current_raw=b"export const APP_VERSION='1.0.2';export const APP_NAME='Razer Assets';writeDevice();")
                with self.assertRaisesRegex(ValueError, "nonliteral version logic"):
                    frontend._cache_current_replacement(source, comparison, host.HostLocalAudit())


if __name__ == "__main__":
    unittest.main()

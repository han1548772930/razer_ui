"""Counterexamples for source-scope propagation and honest capability closure."""
from contextlib import ExitStack
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import analysis as a
import capability_ledger as ledger
import code_identity as ci
import code_identity_reviews as reviews


class IdentityReviewTests(unittest.TestCase):
    def setUp(self):
        sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))
        self.stack = ExitStack()
        self.addCleanup(self.stack.close)
        self.root = Path(self.stack.enter_context(tempfile.TemporaryDirectory()))
        self.stack.enter_context(patch.object(a, "ROOT", self.root))
        self.db = ci.connect(self.root / "identities.sqlite3")
        self.addCleanup(self.db.close)
        reviews.schema(self.db)
        self.raw = b"function(e,t,n){return n(7)+1}"
        self.sha = hashlib.sha256(self.raw).hexdigest()
        self.variant = "a" * 64
        self.path = ".analysis/inputs/frontend/test/raw/products/1/mw/main.js"
        path = self.root / self.path
        path.parent.mkdir(parents=True)
        path.write_bytes(self.raw)
        for pid, sha, variant in ((1, self.sha, self.variant), (2, self.sha, self.variant),
                                  (3, "b" * 64, "c" * 64)):
            name = self.path if pid < 3 else ".analysis/inputs/frontend/other.js"
            self.db.execute("INSERT INTO deployments VALUES(?,?,?,?,?,?)", (pid, "mouse", "mw", "a.json", "d" * 64, None))
            self.db.execute("INSERT OR IGNORE INTO sources VALUES(?,?,?)", (name, sha, len(self.raw)))
            self.db.execute("INSERT INTO deployment_sources VALUES(?,?,?,?)", (pid, "mw", name, "https://apps.razer.com/main.js"))
            self.db.execute("INSERT OR IGNORE INTO factories VALUES(?,?,?,?,?,?,?,?)",
                            (sha, 0, len(self.raw), "1", variant, sha, "webpack_factory", '["7"]'))
            self.db.execute("INSERT INTO capability_scope VALUES(?,?,?,?)", (pid, "mouse", "feature:example", "current_registration"))
            self.db.execute("INSERT INTO capability_roots VALUES(?,?,?,?)", (pid, "feature:example", variant, "current_feature_registration_factory"))
        self.db.commit()

    def owner(self, start=0, end=None):
        end = len(self.raw) if end is None else end
        body = self.raw[start:end]
        return {"id": f"{self.sha}:{start}:{end}", "original": {"path": self.path, "sha256": self.sha,
                 "bytes": len(self.raw)}, "byte_start": start, "byte_end": end,
                "body": body.decode(), "body_sha256": hashlib.sha256(body).hexdigest()}

    def section(self, owner=None, closed=False):
        return {"target_execution": False, "failures": [], "changed_during_query": [],
                "source_chain_complete": closed, "scope": "Complete input and result chain for product 1",
                "complete_source_owners": [owner or self.owner()]}

    def publish(self, section):
        reviews.review_section(self.db, reviews.OwnerVerifier(self.db), "receipt.json", "chain", section,
                               {"product_ids": {1}, "capabilities": {"feature:example"}})
        reviews.populate_candidates(self.db)
        reviews.populate_queue(self.db)

    def test_source_slice_propagates_only_to_equal_factory(self):
        self.publish(self.section(self.owner(16, 24)))
        products = [row[0] for row in self.db.execute("SELECT DISTINCT product_id FROM review_matches ORDER BY product_id")]
        self.assertEqual(products, [1, 2])
        self.assertEqual(self.db.execute("SELECT source_coverage FROM review_owners").fetchone()[0], "source_slice")
        self.assertEqual(ledger._static_state(ledger._sections(self.db, 2, "feature:example")), "partial")

    def test_shared_variant_has_one_reserved_owner(self):
        self.publish(self.section())
        self.db.execute("INSERT OR REPLACE INTO metadata VALUES('review_source_signature',?)", (reviews.source_signature(self.db),))
        self.db.commit()
        queue = self.root / ".analysis/results/variant-work-queue.json"
        queue.parent.mkdir(parents=True, exist_ok=True)
        queue.write_text(json.dumps({"schema_version": 1, "groups": []}), encoding="utf-8")
        with patch.object(reviews, "QUEUE", queue):
            result = reviews.assign_capabilities(self.root / "identities.sqlite3", [
                "first,mouse,feature:example", "second,mouse,feature:example"])
        self.assertGreater(result["assignments"][0]["owned_variants"], 0)
        self.assertEqual(result["assignments"][1]["owned_variants"], 0)
        self.assertEqual(self.db.execute("SELECT COUNT(DISTINCT owner) FROM variant_assignments").fetchone()[0], 1)

    def test_exact_module_pair_maps_to_its_value_factory(self):
        import host
        parser = host.HostLocalAudit().parser
        body = b"5e3:" + self.raw
        original_sha = hashlib.sha256(body).hexdigest()
        (self.root / self.path).write_bytes(body)
        value = parser.parse(b"({" + body + b"})").root_node.descendant_for_byte_range(6, len(body) + 1)
        normalized = ci.normalized_ast(value, factory=True)
        self.db.execute("INSERT INTO factories VALUES(?,?,?,?,?,?,?,?)",
                        (original_sha, 4, len(body), "5000", normalized, self.sha, "webpack_factory", "[]"))
        owner = {"id": "pair", "syntax": "pair", "original": {"path": self.path, "sha256": original_sha, "bytes": len(body)},
                 "byte_start": 0, "byte_end": len(body), "body": body.decode(), "body_sha256": original_sha}
        verified = reviews.OwnerVerifier(self.db).verify(owner)
        self.assertEqual(verified["status"], "matched")
        self.assertEqual(verified["source_coverage"], "whole_factory_source")
        self.db.execute("UPDATE factories SET module_id='5001' WHERE source_sha=?", (original_sha,))
        self.assertNotEqual(reviews.OwnerVerifier(self.db).verify(owner)["status"], "matched")

    def test_closed_product_chain_does_not_close_equal_product(self):
        self.publish(self.section(closed=True))
        self.assertEqual(ledger._static_state(ledger._sections(self.db, 1, "feature:example")), "closed")
        self.assertEqual(ledger._static_state(ledger._sections(self.db, 2, "feature:example")), "partial")
        self.assertEqual(self.db.execute("SELECT state FROM variant_queue").fetchone()[0], "partial_review_scope")

    def test_compact_closure_keeps_reused_source_owners_in_identity_propagation(self):
        original = self.owner()
        section = self.section(closed=True)
        section["complete_source_owners"] = []
        section["reused_source_identities"] = [{"original": original["original"]}]
        section["upstream_sections"] = [{"pointer": "/previous", "canonical_sha256": "b" * 64}]
        section["reused_owner_verification"] = [{"owner_id": original["id"],
            "original_sha256": self.sha, "byte_range": [0, len(self.raw)],
            "body_sha256": self.sha, "proof_sections": ["previous"]}]
        self.publish(section)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM review_matches").fetchone()[0], 2)
        self.assertEqual(ledger._static_state(ledger._sections(self.db, 1, "feature:example")), "closed")
        self.assertEqual(ledger._static_state(ledger._sections(self.db, 2, "feature:example")), "partial")

    def test_compact_closure_without_source_or_upstream_is_invalid(self):
        section = self.section(closed=True)
        section["complete_source_owners"] = []
        section["reused_owner_verification"] = [{"owner_id": "missing",
            "original_sha256": self.sha, "byte_range": [0, len(self.raw)],
            "body_sha256": self.sha, "proof_sections": ["previous"]}]
        self.publish(section)
        self.assertEqual(self.db.execute("SELECT state FROM review_sections").fetchone()[0], "invalid")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM review_matches").fetchone()[0], 0)

    def test_regular_owners_keep_legacy_upstream_metadata(self):
        section = self.section()
        section["upstream_sections"] = ["legacy prose reference"]
        self.publish(section)
        self.assertEqual(self.db.execute("SELECT state FROM review_sections").fetchone()[0], "partial")

    def test_reused_reference_verifies_and_selects_only_declared_owners(self):
        owner = self.owner()
        path = self.root / ".analysis/results/reference.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"complete_owners": [owner, {**owner, "id": "unselected"}]}), encoding="utf-8")
        section = self.section(closed=True)
        section["complete_source_owners"] = []
        section["reused_owners"] = {
            "reference": {"path": ".analysis/results/reference.json", "bytes": path.stat().st_size,
                          "sha256": a.sha(path), "pointer": "/complete_owners"},
            "owners": [{"owner_id": owner["id"], "original_sha256": self.sha, "body_sha256": self.sha}]}
        self.assertEqual([item["id"] for item in reviews.owner_records(section)], [owner["id"]])
        self.publish(section)
        self.assertEqual(self.db.execute("SELECT state FROM review_sections").fetchone()[0], "closed")
        del section["reused_owners"]["owners"][0]["original_sha256"]
        self.assertEqual([item["original"] for item in reviews.owner_records(section)], [owner["original"]])
        section["reused_owners"]["owners"][0]["body_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "differs"):
            list(reviews.owner_records(section))

    def test_selected_native_code_evidence_stays_partial_on_target_product(self):
        self.publish(self.section(closed=True))
        self.db.execute("CREATE TABLE native_review_matches(section_id TEXT, product_id INTEGER, normalized_sha TEXT)")
        self.db.execute("INSERT INTO native_review_matches VALUES(?,?,?)", ("receipt.json#/chain", 3, "d" * 64))
        sections = ledger._sections(self.db, 3, "feature:example")
        self.assertEqual(sections[0]["matched_native_function_count"], 1)
        self.assertEqual(sections[0]["matched_js_owner_count"], 0)
        self.assertEqual(ledger._static_state(sections), "partial")

    def test_shared_library_without_capability_consumption_stays_candidate(self):
        self.publish(self.section(closed=True))
        self.db.execute("DELETE FROM capability_roots WHERE product_id=2")
        sections = ledger._sections(self.db, 2, "feature:example")
        self.assertEqual(sections[0]["candidate_js_owner_count"], 1)
        self.assertEqual(sections[0]["matched_js_owner_count"], 0)
        self.assertEqual(sections[0]["basis"], "reusable_code_candidate")
        self.assertEqual(ledger._static_state(sections), "not_started")

    def test_current_entrypoint_outside_factories_keeps_explicit_closure(self):
        self.db.execute("DELETE FROM factories WHERE source_sha=?", (self.sha,))
        self.publish(self.section(closed=True))
        self.assertEqual(self.db.execute("SELECT status FROM review_owners").fetchone()[0], "verified_current_non_factory_scope")
        sections = ledger._sections(self.db, 1, "feature:example")
        self.assertEqual(sections[0]["current_non_factory_owner_count"], 1)
        self.assertEqual(ledger._static_state(sections), "closed")
        self.assertEqual(ledger._static_state(ledger._sections(self.db, 2, "feature:example")), "not_started")

    def test_shared_entrypoint_requires_current_deployment_and_retains_explicit_scope(self):
        self.db.execute("DELETE FROM factories WHERE source_sha=?", (self.sha,))
        self.db.execute("DELETE FROM deployment_sources WHERE path=?", (self.path,))
        self.db.execute("INSERT INTO shared_deployments VALUES(?,?,?,?,NULL)",
                        ("shared-app", "https://apps.razer.com/shared/", "shared.json", "e" * 64))
        self.db.execute("INSERT INTO shared_sources VALUES(?,?,?)",
                        ("shared-app", self.path, "https://apps.razer.com/shared/main.js"))
        self.publish(self.section(closed=True))
        self.assertEqual(self.db.execute("SELECT status FROM review_owners").fetchone()[0],
                         "verified_current_non_factory_scope")
        self.assertEqual(ledger._static_state(ledger._sections(self.db, 1, "feature:example")), "evidence_unmatched")
        self.assertEqual(ledger._static_state(ledger._sections(self.db, 2, "feature:example")), "not_started")
        signature = reviews.source_signature(self.db)
        self.db.execute("UPDATE shared_deployments SET failure='source_changed'")
        self.assertNotEqual(reviews.source_signature(self.db), signature)
        self.assertEqual(reviews.OwnerVerifier(self.db).verify(self.owner())["status"],
                         "source_not_in_current_factory_index")
        self.db.execute("DELETE FROM shared_deployments")
        self.assertEqual(reviews.OwnerVerifier(self.db).verify(self.owner())["status"],
                         "source_not_in_current_factory_index")

    def test_source_owners_reference_verifies_exact_frozen_bytes(self):
        owner = self.owner()
        path = self.root / ".analysis/results/source-reference.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"complete_source_owners": [owner]}), encoding="utf-8")
        reference = {"path": ".analysis/results/source-reference.json", "bytes": path.stat().st_size,
                     "sha256": a.sha(path), "pointer": "/complete_source_owners"}
        section = {"complete_source_owners_reference": reference}
        self.assertEqual(list(reviews.owner_records(section)), [owner])
        with self.assertRaisesRegex(ValueError, "disagree"):
            list(reviews.owner_records({**section, "complete_owner_reference": {**reference, "bytes": 0}}))
        path.write_text("{}", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "byte identity"):
            list(reviews.owner_records(section))

    def test_body_tampering_invalidates_section_and_matches(self):
        owner = self.owner()
        owner["body_sha256"] = "0" * 64
        self.publish(self.section(owner))
        self.assertEqual(self.db.execute("SELECT state FROM review_sections").fetchone()[0], "invalid")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM review_matches").fetchone()[0], 0)

    def test_additional_frozen_references_propagate_and_verify_every_input(self):
        first, second = self.owner(), self.owner(16, 24)
        references = []
        for name, owners in (("primary", [first]), ("additional", [first, second])):
            path = self.root / f".analysis/results/{name}.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({"complete_source_owners": owners}), encoding="utf-8")
            references.append({"path": f".analysis/results/{name}.json", "bytes": path.stat().st_size,
                               "sha256": a.sha(path), "pointer": "/complete_source_owners"})
        section = self.section()
        del section["complete_source_owners"]
        section["complete_owner_reference"] = references[0]
        section["additional_complete_owner_references"] = references[1:]
        self.assertEqual(list(reviews.owner_records(section)), [first, second])
        self.publish(section)
        self.assertEqual(self.db.execute("SELECT owner_count,state FROM review_sections").fetchone()[:],
                         (2, "partial"))
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM review_owners").fetchone()[0], 2)
        self.assertEqual([r[0] for r in self.db.execute(
            "SELECT DISTINCT product_id FROM review_matches ORDER BY product_id")], [1, 2])
        path.write_text(path.read_text(encoding="utf-8") + " ", encoding="utf-8")
        reviews.review_section(self.db, reviews.OwnerVerifier(self.db),
            "receipt.json", "chain", section,
            {"product_ids": {1}, "capabilities": {"feature:example"}})
        self.assertEqual(self.db.execute("SELECT state FROM review_sections").fetchone()[0], "invalid")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM review_matches").fetchone()[0], 0)

    def test_additional_references_reject_bad_shape_and_conflicting_owner_identities(self):
        for invalid in (None, {}, [None]):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, "list of references"):
                list(reviews.owner_records({**self.section(),
                    "additional_complete_owner_references": invalid}))
        owner = self.owner()
        path = self.root / ".analysis/results/conflicting.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"complete_source_owners": [
            {**owner, "body_sha256": "0" * 64}]}), encoding="utf-8")
        section = {**self.section(), "additional_complete_owner_references": [
            {"path": ".analysis/results/conflicting.json", "bytes": path.stat().st_size,
             "sha256": a.sha(path), "pointer": "/complete_source_owners"}]}
        self.publish(section)
        state, errors = self.db.execute("SELECT state,verification_errors FROM review_sections").fetchone()
        self.assertEqual(state, "invalid")
        self.assertIn("Conflicting retained owner identity", errors)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM review_matches").fetchone()[0], 0)

    def test_byte_changes_fail_verification(self):
        (self.root / self.path).write_bytes(self.raw.replace(b"+1", b"+2"))
        with self.assertRaisesRegex(ValueError, "byte identity"):
            reviews.OwnerVerifier(self.db).verify(self.owner())

    def test_factory_without_capability_evidence_is_not_remaining_work(self):
        self.db.execute("DELETE FROM capability_roots")
        self.db.execute("INSERT INTO markers VALUES(?,?,?,?)", (self.sha, 0, len(self.raw), "example"))
        reviews.populate_candidates(self.db)
        reviews.populate_queue(self.db)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM variant_queue").fetchone()[0], 1)
        self.assertEqual(self.db.execute("SELECT state FROM variant_queue").fetchone()[0], "unreviewed")

    def role_fixture(self, *, role="static_string_exports", pure=True):
        import frontend
        import host
        raw = b'self.webpackChunkx.push([[1],{1:(e,t,n)=>{n.d(t,{A:()=>a});const a="word"}}]);'
        tree = host.HostLocalAudit().parser.parse(raw)
        factory = frontend.recovery_segments(tree.root_node, include_standalone=True)[0]["factories"][0]
        start, end = factory["factory_range"]
        node = tree.root_node.descendant_for_byte_range(start, end - 1)
        variant = ci.normalized_ast(node, factory=True)
        sha = hashlib.sha256(raw).hexdigest()
        (self.root / self.path).write_bytes(raw)
        self.db.execute("UPDATE sources SET sha256=?,bytes=? WHERE path=?", (sha, len(raw), self.path))
        self.db.execute("INSERT INTO factories VALUES(?,?,?,?,?,?,?,?)",
                        (sha, start, end, "1", variant, hashlib.sha256(raw[start:end]).hexdigest(), "webpack_factory", "[]"))
        self.db.execute("INSERT INTO capability_candidates VALUES(?,?,?,?)", (1, "feature:example", variant, "ast_token_candidate"))
        report = {"schema_version": 1, "kind": "assigned_code_variant_roles", "target_execution": False, "failures": [],
                  "variant_count": 1, "selected_variant_set_sha256": ci.digest([variant]), "variants": [
                      {"original": {"path": self.path, "sha256": sha, "bytes": len(raw)},
                       "normalized_sha256": variant, "module_id": "1", "start_byte": start, "end_byte": end,
                       "role": role, "pure_data": pure}]}
        path = self.root / ".analysis/results/roles.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(report), encoding="utf-8")
        return {"path": ".analysis/results/roles.json", "sha256": a.sha(path)}

    def test_verified_data_is_kept_as_dependency_outside_behavior_queue(self):
        ref = self.role_fixture()
        self.assertEqual(reviews.import_roles(self.db, [ref]), {"static_string_exports": 1})
        reviews.populate_queue(self.db)
        self.assertEqual(self.db.execute("SELECT state FROM variant_queue").fetchone()[0], "verified_data_dependency")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM capability_candidates").fetchone()[0], 1)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM review_matches").fetchone()[0], 0)

    def test_runtime_role_never_becomes_completed_data(self):
        ref = self.role_fixture(role="locale_runtime_with_dictionaries", pure=False)
        reviews.import_roles(self.db, [ref])
        reviews.populate_queue(self.db)
        self.assertEqual(self.db.execute("SELECT state FROM variant_queue").fetchone()[0], "unreviewed")

    def test_role_receipt_and_source_tampering_are_rejected(self):
        ref = self.role_fixture()
        bad = dict(ref, sha256="0" * 64)
        with self.assertRaisesRegex(ValueError, "receipt changed"):
            reviews.import_roles(self.db, [bad])
        (self.root / self.path).write_bytes(b"different source")
        with self.assertRaisesRegex(ValueError, "original changed"):
            reviews.import_roles(self.db, [ref])
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM variant_roles").fetchone()[0], 0)

    def test_legacy_role_inventory_is_bound_without_rewriting_receipt(self):
        ref = self.role_fixture()
        path = self.root / ref["path"]
        report = json.loads(path.read_bytes())
        del report["selected_variant_set_sha256"]
        report["role_counts"] = {"static_string_exports": 1}
        path.write_text(json.dumps(report), encoding="utf-8")
        before = path.read_bytes()
        reviews.import_roles(self.db, [dict(ref, sha256=a.sha(path))])
        self.assertEqual(path.read_bytes(), before)
        scopes = json.loads(self.db.execute("SELECT value FROM metadata WHERE key='review_role_scopes'").fetchone()[0])
        self.assertEqual(scopes[0]["basis"], "immutable_receipt_bytes_and_complete_inventory")

    def test_one_variant_has_one_assignment_even_for_multiple_capabilities(self):
        self.publish(self.section())
        self.db.execute("INSERT INTO capability_scope VALUES(?,?,?,?)", (2, "mouse", "page:other", "current_mount_source_identity"))
        self.db.execute("INSERT INTO capability_candidates VALUES(?,?,?,?)", (2, "page:other", self.variant, "ast_token_candidate"))
        self.db.execute("DELETE FROM variant_queue")
        reviews.populate_queue(self.db)
        row = self.db.execute("SELECT * FROM variant_queue WHERE normalized_sha=?", (self.variant,)).fetchone()
        self.assertEqual(json.loads(row["capabilities"]), ["feature:example", "page:other"])
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM variant_queue WHERE normalized_sha=?", (self.variant,)).fetchone()[0], 1)

    def navigation_fixture(self, acquisition_sha="d" * 64):
        self.db.execute("INSERT INTO deployments VALUES(?,?,?,?,?,?)", (1, "mouse", "ui", "ui.json", "d" * 64, None))
        self.db.execute("INSERT INTO deployment_sources VALUES(?,?,?,?)", (1, "ui", self.path, "https://apps.razer.com/main.js"))
        self.db.execute("CREATE TABLE navigation_products(product_id INTEGER PRIMARY KEY,receipt TEXT)")
        original = {"path": self.path, "bytes": len(self.raw), "sha256": self.sha}
        report = {"product_id": 1, "target_execution": False, "acquisition_sha256": acquisition_sha,
                  "source_scope_complete": False, "failures": [{"reason": "other branch unresolved"}],
                  "owners": {"root": {"original": original, "start_byte": 0, "end_byte": len(self.raw),
                      "syntax": "function_expression", "raw_sha256": self.sha, "normalized_sha256": self.variant}},
                  "pages": [{"kind": "NEW", "component": "root", "mount_path": ["root"],
                             "binding_resolution": "implementation_root", "selection_status": "first_match_candidate"}]}
        self.db.execute("INSERT INTO navigation_products VALUES(?,?)", (1, json.dumps(report)))

    def test_current_mount_adds_real_root_without_erasing_unresolved_old_scope(self):
        self.navigation_fixture()
        records, summary = reviews.bind_navigation(self.db)
        self.assertEqual(summary["implementation_roots"], 1)
        self.db.execute("DELETE FROM capability_scope")
        self.db.execute("DELETE FROM capability_roots")
        reviews.populate_scope(self.db, {"families": {"mouse": {"product_ids": [1]}}},
                               {"products": [{"product_id": 1, "current_page_kinds": ["OLD"]}]}, records)
        states = dict(self.db.execute("SELECT capability,source_state FROM capability_scope"))
        self.assertEqual(states, {"page:NEW": "current_navigation_partial", "page:OLD": "mount_source_identity_unmatched"})
        self.assertEqual(self.db.execute("SELECT normalized_sha FROM capability_roots WHERE capability='page:NEW'").fetchone()[0], self.variant)

    def test_stale_navigation_never_binds_current_pages(self):
        self.navigation_fixture(acquisition_sha="0" * 64)
        records, summary = reviews.bind_navigation(self.db)
        self.assertEqual(records, {})
        self.assertEqual(summary["stale_products"], [1])

    def test_legacy_navigation_does_not_hide_later_current_products(self):
        self.navigation_fixture()
        records, _ = reviews.bind_navigation(self.db)
        self.db.execute("INSERT INTO deployments VALUES(?,?,?,?,?,?)", (2, "mouse", "ui", "ui2.json", "d" * 64, None))
        self.db.execute("INSERT INTO deployment_sources VALUES(?,?,?,?)", (2, "ui", self.path, "https://apps.razer.com/main.js"))
        records[2] = dict(records[1], product_id=2)
        self.db.execute("DELETE FROM capability_scope")
        self.db.execute("DELETE FROM capability_roots")
        legacy = {"product_id": 1, "current_page_kinds": ["OLD"],
                  "source_inputs": [{"path": self.path, "sha256": self.sha}],
                  "navigations": [{"source": self.path, "pages": []}]}
        reviews.populate_scope(self.db, {"families": {"mouse": {"product_ids": [1, 2]}}},
                               {"products": [legacy]}, records)
        self.assertEqual(self.db.execute("SELECT source_state FROM capability_scope WHERE product_id=2 AND capability='page:NEW'").fetchone()[0],
                         "current_navigation_partial")

    def test_changed_dependency_source_invalidates_propagation_signature(self):
        before = reviews.source_signature(self.db)
        self.db.execute("UPDATE deployments SET acquisition_sha='changed' WHERE product_id=1")
        self.assertNotEqual(before, reviews.source_signature(self.db))
        before = reviews.source_signature(self.db)
        self.db.execute("INSERT INTO metadata VALUES('factory_extraction_version','2')")
        self.assertNotEqual(before, reviews.source_signature(self.db))

    def test_failed_or_changed_section_never_propagates(self):
        section = self.section()
        section["changed_during_query"] = [self.path]
        self.publish(section)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM review_matches").fetchone()[0], 0)

    def test_generator_and_ledger_preserve_stage_and_scope_boundaries(self):
        results = self.root / ".analysis/results"
        results.mkdir(parents=True)
        manifest_path = self.root / "tools/capability-ledger.json"
        manifest_path.parent.mkdir()
        receipt_path = results / "frontend/current.json"
        receipt_path.parent.mkdir()
        receipt_path.write_text(json.dumps({"chain": self.section(closed=True)}), encoding="utf-8")
        manifest_path.write_text(json.dumps({"families": {"mouse": {"source_family": "mouse", "product_ids": [1, 2, 3]}},
            "capability_sections": [{"product_ids": [1], "capabilities": ["feature:example"],
                "sections": [{"receipt": ".analysis/results/frontend/current.json", "section": "chain"}]}]}), encoding="utf-8")
        (results / "product-documentation.json").write_text(json.dumps({"products": []}), encoding="utf-8")
        output = results / "capability-ledger.json"
        output.write_text(json.dumps({"products": [{"product_id": 1, "capabilities": [
            {"capability": "feature:example", "stages": {"static": "closed", "rust": "partial", "ui_backend": "not_started",
                "official_comparison": "not_started"}}]}]}), encoding="utf-8")
        for sha in (self.sha, "b" * 64):
            self.db.execute("INSERT INTO parsed_sources VALUES(?,?,NULL)", (sha, ci.NORMALIZATION))
            self.db.execute("INSERT INTO registrations VALUES(?,?,?,?,?)", (sha, 0, len(self.raw), "genericFeature", '["example"]'))
        self.db.commit()
        for module, name, value in ((ci, "DATABASE", self.root / "identities.sqlite3"),
                                    (reviews, "REPORT", results / "propagation.json"),
                                    (reviews, "QUEUE", results / "queue.json"),
                                    (ledger, "MANIFEST", manifest_path), (ledger, "OUTPUT", output)):
            self.stack.enter_context(patch.object(module, name, value))
        report = reviews.build(ci.DATABASE)
        self.assertEqual(report["matched_variants"], 1)
        result = ledger.build(db_path=ci.DATABASE)
        self.assertEqual(result["totals"]["mouse"]["static"], {"closed": 1, "partial": 1, "not_started": 1})
        self.assertEqual(result["totals"]["mouse"]["rust"], {"partial": 1, "not_started": 2})
        receipt_path.write_text(json.dumps({"chain": self.section(closed=False)}), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "Formal review changed"):
            ledger.build(db_path=ci.DATABASE)


if __name__ == "__main__":
    unittest.main()

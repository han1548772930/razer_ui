"""Pure source-owner and semantic-boundary tests for family audits."""
import copy
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

import analysis
import evidence
import families
import host
from test_host import pe_fixture


class SourceCaptureModuleTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        change = patch.object(analysis, "ROOT", self.root)
        change.start()
        self.addCleanup(change.stop)

    def write(self, name, raw):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

    def fixture(self, *, host_capture=False, legacy_raw=False, ranges=((0, 9),)):
        if host_capture:
            source_name = ".analysis/inputs/host-current/asar/main.js"
            recorded, cwd = source_name, "."
            schema = {"application_js_inputs": 1, "integration_dependency_js_inputs": 0,
                      "target_execution": False}
        else:
            source_name = (".analysis/inputs/frontend/" + ("synthetic" if legacy_raw else "shared")
                           + "/raw/site/runtime.js")
            recorded = "site/runtime.js" if legacy_raw else source_name.removeprefix(".analysis/inputs/")
            cwd = ".analysis/inputs/frontend/synthetic/raw" if legacy_raw else ".analysis/inputs"
            schema = {"input": ".analysis/inputs/frontend/synthetic"}
        source = self.write(source_name, b"source();")
        output = self.write(".analysis/results/frontend/synthetic/modules/module.js", b"source();\n")
        provenance = self.write(".analysis/results/frontend/synthetic/modules/provenance.json",
            json.dumps({"modules": {"module.js": {"input": recorded.replace("/", "\\"),
                "ranges": ranges, "extraction": "structural"}}}).encode())
        commands = [] if legacy_raw else [{"stage": "unpack", "exit_code": 0, "cwd": cwd,
            "argv": ["wakaru.exe", recorded.replace("/", "\\"), "--unpack"]}]
        return {**schema, "original_inputs": [source], "outputs": [output, provenance],
                "commands": commands}, source

    def test_frontend_cross_directory_original_preserves_recorded_unpack_resolution(self):
        captured, source = self.fixture()
        modules = families.source_capture_modules(captured, source)
        self.assertEqual(len(modules), 1)
        module = modules[0]
        self.assertEqual(module["original"], source)
        self.assertEqual(module["original_ranges"], [[0, 9]])
        self.assertEqual(module["output"], captured["outputs"][0])
        self.assertEqual(module["provenance"], captured["outputs"][1])
        self.assertEqual(module["input_resolution"], {"cwd": ".analysis/inputs",
            "argument": "frontend/shared/raw/site/runtime.js"})

    def test_legacy_raw_owner_keeps_absent_unpack_resolution(self):
        captured, source = self.fixture(legacy_raw=True)
        module, = families.source_capture_modules(captured, source)
        self.assertEqual(module["original"], source)
        self.assertIsNone(module["input_resolution"])

    def test_original_and_output_must_be_declared_in_the_capture(self):
        for field in ("original_inputs", "outputs"):
            captured, source = self.fixture()
            captured[field] = [] if field == "original_inputs" else captured[field][1:]
            with self.subTest(field=field), self.assertRaises(ValueError):
                families.source_capture_modules(captured, source)

    def test_changed_output_bytes_are_rejected_without_changing_size(self):
        captured, source = self.fixture()
        path = self.root / captured["outputs"][0]["path"]
        path.write_bytes(b"change();\n")
        with self.assertRaisesRegex(ValueError, "output differs"):
            families.source_capture_modules(captured, source)

    def test_original_ranges_are_nonempty_integer_spans_inside_the_declared_original(self):
        invalid = (None, [], [[0]], [[0, 0]], [[-1, 9]], [[0, 10]], [[0, 9.0]], [[False, 9]])
        for host_capture in (False, True):
            for ranges in invalid:
                captured, source = self.fixture(host_capture=host_capture, ranges=ranges)
                with self.subTest(host_capture=host_capture, ranges=ranges), self.assertRaises(ValueError):
                    families.source_capture_modules(captured, source)

    def test_host_asar_schema_and_both_existing_unpack_path_forms_still_link(self):
        for relative_to_asar in (False, True):
            captured, source = self.fixture(host_capture=True)
            if relative_to_asar:
                provenance_path = self.root / captured["outputs"][1]["path"]
                provenance = json.loads(provenance_path.read_bytes())
                provenance["modules"]["module.js"]["input"] = "main.js"
                captured["outputs"][1] = self.write(captured["outputs"][1]["path"],
                    json.dumps(provenance).encode())
                captured["commands"][0].update(cwd=".analysis/inputs/host-current/asar",
                    argv=["wakaru.exe", "main.js", "--unpack"])
            with self.subTest(relative_to_asar=relative_to_asar):
                module, = families.source_capture_modules(captured, source)
                self.assertEqual(module["original"], source)
                self.assertEqual(module["original_ranges"], [[0, 9]])
                self.assertEqual(module["input_resolution"], {
                    "cwd": ".analysis/inputs/host-current/asar" if relative_to_asar else ".",
                    "argument": "main.js" if relative_to_asar else source["path"]})


class AudioSeirenCurrentUiTests(unittest.TestCase):
    def test_current_ui_review_is_reachable_through_the_maintained_dispatch(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "audio", "--function", "ui",
                                  "--query", "audio-seiren-eq", "--product", "1422"])
        with patch.object(families, "audit_audio_seiren_current_ui") as current:
            families.audit_family_function(args)
        current.assert_called_once_with(args)

    @classmethod
    def setUpClass(cls):
        import frontend
        root = analysis.ROOT / ".analysis/inputs/frontend"
        if not (root / "audio-1422-current/acquisition.json").is_file():
            raise unittest.SkipTest("Retained Seiren UI inputs are unavailable")
        audit = host.HostLocalAudit()
        selected = frontend.current_source_selection("https://apps.razer.com/synapse/products/1422/ui/", audit,
            candidates=[root / "audio-1422-current/acquisition.json", *sorted(root.glob("audio-1422-snapshot-*/acquisition.json"))])
        cls.selection = selected
        cls.original_bytes = [(Path(item["path"]).name, audit.read(item)) for item in selected["originals"]
                              if item["path"].endswith(".js")]

    def analyze(self, transform=lambda name, raw: raw):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", root)
        patched.start()
        self.addCleanup(patched.stop)
        audit = host.HostLocalAudit()
        originals = []
        for name, original in self.original_bytes:
            raw = transform(name, original)
            path = root / ".analysis/inputs/frontend/seiren-fixture" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            originals.append({"path": path.relative_to(root).as_posix(), "bytes": len(raw),
                              "sha256": hashlib.sha256(raw).hexdigest()})
        return audit, families.audio_seiren_current_ui_chain(audit, originals)

    def test_current_mount_pages_meter_and_cleanup_owners_are_complete_original_slices(self):
        audit, result = self.analyze()
        self.assertEqual([page["name"] for page in result["pages"]],
                         ["TAB_EQ", "TAB_EFFECTS", "STREAM_MIXER_HEADER", "TAB_LIGHTING", "HELP"])
        self.assertEqual(result["footer_excluded_pages"], ["TAB_LIGHTING", "HELP"])
        self.assertEqual(result["microphone_device_enum"], "DeviceId_MarieMic")
        self.assertEqual(result["supported_sampling_rates"], [48000, 96000])
        self.assertFalse(result["whole_product_complete"])
        self.assertEqual(result["official_dll_comparison"], "not_executed")
        for owner in audit.owners.values():
            raw = audit.read(owner["original"])[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(raw.decode(), owner["body"])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), owner["body_sha256"])
        waveform = audit.owners[result["roles"]["waveform"]]["body"]
        self.assertIn("getTracks().forEach", waveform)
        self.assertIn("cancelAnimationFrame", waveform)
        self.assertEqual(audit.final_snapshot(), [])

    def test_minified_component_alias_changes_keep_the_actual_lexical_consumers(self):
        import re
        def aliases(name, raw):
            for old, new in ((b"Un", b"CurrentFooter"), (b"Pn", b"CurrentWaveform"), (b"Fa", b"ReadyProduct")):
                raw = re.sub(rb"\b" + old + rb"\b", new, raw)
            return raw
        audit, result = self.analyze(aliases)
        self.assertIn("CurrentFooter=", audit.owners[result["roles"]["footer"]]["body"])
        self.assertIn("CurrentWaveform=", audit.owners[result["roles"]["waveform"]]["body"])

    def test_unmounted_ready_product_cannot_borrow_a_declared_export(self):
        with self.assertRaisesRegex(ValueError, "no statically resolved mounted JSX"):
            self.analyze(lambda name, raw: raw.replace(b"a.bind(a,2975)", b"a.bind(a,2976)"))

    def test_unrequested_product_chunk_cannot_prove_a_lazy_mount(self):
        with self.assertRaisesRegex(ValueError, "does not request its target"):
            self.analyze(lambda name, raw: raw.replace(b"a.e(890)", b"a.e(891)"))

    def test_missing_eq_factory_is_not_satisfied_by_another_page(self):
        with self.assertRaisesRegex(ValueError, "factory 6008 is absent"):
            self.analyze(lambda name, raw: raw.replace(b"Promise.resolve().then(s.bind(s,6007))",
                                                     b"Promise.resolve().then(s.bind(s,6008))"))

    def test_a_different_reducer_does_not_prove_the_microphone_meter_consumer(self):
        with self.assertRaisesRegex(ValueError, "footer meter selector"):
            self.analyze(lambda name, raw: raw.replace(b"levelMeter:e.micReducer.levelMeter",
                                                     b"levelMeter:e.headphoneReducer.levelMeter"))

    def test_missing_meter_cannot_be_claimed_as_observed_zero(self):
        with self.assertRaisesRegex(ValueError, "distinguishes absent and observed"):
            self.analyze(lambda name, raw: raw.replace(b":void 0,V=F", b":0,V=F"))

    def test_changed_footer_visibility_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "page exclusion changed"):
            self.analyze(lambda name, raw: raw.replace(b"this.state.active_view!==M._$r",
                                                     b"this.state.active_view===M._$r"))

    def test_recorder_wizard_callbacks_keep_ui_results_separate_from_device_confirmation(self):
        audit, result = self.analyze()
        reviewed = result["recording_and_wizard"]
        self.assertTrue(reviewed["bounded_recording_and_wizard_ui_complete"])
        self.assertEqual(reviewed["file_save_action"], "absent_in_mounted_component")
        self.assertEqual(reviewed["device_write_completion"], "not_established_by_ui")
        self.assertEqual(reviewed["return_switch_action"], "ON_MIC_RETURN_SWITCH")
        self.assertEqual(reviewed["measurement_analyses"]["analyzeMicGain"]["module_id"], 9909)
        self.assertEqual(reviewed["measurement_analyses"]["analyzeNoiseGate"]["module_id"], 596)
        for role in ("record_start", "record_onstop", "record_onloadend", "record_unmount", "recording_reducer", "play_start",
                     "play_stop", "wizard_snapshot", "wizard_restore", "wizard_apply", "measurement_start",
                     "measurement_stop", "measurement_unmount", "measurement_worklet"):
            self.assertIn(reviewed["roles"][role], audit.owners)
        self.assertIn("no generation guard", reviewed["semantics"]["recording_late_results"])

    def test_wizard_must_be_mounted_by_its_actual_root_condition(self):
        with self.assertRaisesRegex(ValueError, "root wizard condition"):
            self.analyze(lambda name, raw: raw.replace(b"this.props.isMicSetupOpen&&(0,E.jsx)(ha,{})",
                                                     b"this.props.otherPopupOpen&&(0,E.jsx)(ha,{})"))

    def test_cancel_must_restore_before_closing_the_wizard(self):
        with self.assertRaisesRegex(ValueError, "cancel no longer restores then closes"):
            self.analyze(lambda name, raw: raw.replace(b"V=()=>{m(),F()}", b"V=()=>{F(),m()}"))

    def test_worklet_url_cleanup_cannot_be_inferred_from_another_audio_owner(self):
        with self.assertRaisesRegex(ValueError, "worklet URL cleanup changed"):
            self.analyze(lambda name, raw: raw.replace(b"finally{L&&URL.revokeObjectURL(L)}", b"finally{}"))

    def test_unreferenced_gain_function_cannot_satisfy_the_actual_barrel_dependency(self):
        with self.assertRaisesRegex(ValueError, "analyzeMicGain reexported function"):
            self.analyze(lambda name, raw: raw.replace(b"i(r(9909),e)", b"i(r(9999),e)"))

    def test_declared_recorder_does_not_replace_a_missing_imported_export(self):
        with self.assertRaisesRegex(ValueError, "export 9374.B is absent"):
            self.analyze(lambda name, raw: raw.replace(b"children:(0,E.jsx)(jn.A,{})",
                                                     b"children:(0,E.jsx)(jn.B,{})"))

    def test_changed_recording_reducer_cannot_reuse_the_optimistic_confirmation_conclusion(self):
        with self.assertRaisesRegex(ValueError, "audited optimistic request state"):
            self.analyze(lambda name, raw: raw.replace(b"micReturnSwitch:{switchstate:",
                                                     b"returnSwitchDraft:{switchstate:"))


class AudioDriverLifecycleDispatchTests(unittest.TestCase):
    def test_query_routes_without_replaying_endpoint_dsp_or_camera_audits(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "audio", "--function", "mw",
            "--query", "audio-driver-lifecycle", "--product", "1422"])
        with patch.object(families, "audit_audio_driver_lifecycle") as selected, \
                patch.object(families, "audit_audio_endpoint_local_chain") as endpoint, \
                patch.object(families, "audit_audio_seiren_native_dsp") as dsp, \
                patch.object(families, "audit_camera_image_host_chain") as camera:
            families.audit_family_function(args)
        selected.assert_called_once_with(args)
        for shared in (endpoint, dsp, camera):
            shared.assert_not_called()

    def test_scope_product_and_production_preparation_are_rejected(self):
        import argparse
        for family, function in (("accessories", "mw"), ("audio", "main")):
            with self.subTest(family=family, function=function), \
                    patch.object(families, "audit_audio_driver_lifecycle") as selected, \
                    self.assertRaisesRegex(ValueError, "current audio middleware scope"):
                families.audit_family_function(argparse.Namespace(family=family, function=function,
                    query="audio-driver-lifecycle"))
            selected.assert_not_called()
        for products, prepare in (([1422], True), ([1423], False), ([1422, 1423], False)):
            with self.subTest(products=products, prepare=prepare), \
                    self.assertRaisesRegex(ValueError, "PID1422 without production preparation"):
                families.audit_audio_driver_lifecycle(argparse.Namespace(family="audio", function="mw",
                    product=products, prepare=prepare))

    def test_failure_and_changed_snapshot_preserve_unresolved_facts(self):
        import argparse
        for changed in (False, True):
            with self.subTest(changed=changed), \
                    patch.object(families, "collect_audio_driver_lifecycle", **({"return_value": {}} if changed
                        else {"side_effect": ValueError("missing current audio driver owner")})), \
                    patch.object(host.HostLocalAudit, "final_snapshot", return_value=["audio-owner"] if changed else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "audio driver lifecycle observation remains unresolved"):
                families.audit_audio_driver_lifecycle(argparse.Namespace(family="audio", function="mw",
                    product=[1422], prepare=False))
            report, _, _, section = publish.call_args.args
            self.assertEqual(section, "driver_lifecycle")
            self.assertFalse(report["bounded_observation_complete"])
            self.assertFalse(report["source_chain_complete"])
            self.assertEqual(report["snapshot_stable"], not changed)
            self.assertIn("audio-owner" if changed else "missing current audio driver owner", report["failures"][0])
            self.assertFalse(report["target_execution"])
            self.assertEqual(report["official_dll_comparison"], "not_executed")


class CameraMediaPreviewDispatchTests(unittest.TestCase):
    def test_query_routes_without_replaying_other_camera_chains(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "accessories", "--function", "main",
            "--query", "camera-media-preview-chain", "--product", "3595", "--product", "3596"])
        with patch.object(families, "audit_camera_media_preview_chain") as selected, \
                patch.object(families, "audit_camera_device_lifecycle") as lifecycle, \
                patch.object(families, "audit_camera_image_properties") as image:
            families.audit_family_function(args)
        selected.assert_called_once_with(args)
        lifecycle.assert_not_called()
        image.assert_not_called()

    def test_scope_products_and_production_preparation_are_rejected(self):
        import argparse
        for family, function in (("audio", "main"), ("accessories", "mw")):
            with self.subTest(family=family, function=function), \
                    patch.object(families, "audit_camera_media_preview_chain") as selected, \
                    self.assertRaisesRegex(ValueError, "current accessories main scope"):
                families.audit_family_function(argparse.Namespace(family=family, function=function,
                    query="camera-media-preview-chain"))
            selected.assert_not_called()
        for products, prepare in (([3595], False), ([3596], False), ([3595, 3596, 3594], False), ([3595, 3596], True)):
            with self.subTest(products=products, prepare=prepare), \
                    self.assertRaisesRegex(ValueError, "PID3595 and PID3596 without production preparation"):
                families.audit_camera_media_preview_chain(argparse.Namespace(family="accessories", function="main",
                    product=products, prepare=prepare))

    def test_success_updates_only_the_media_preview_section(self):
        import argparse
        with patch.object(families, "collect_camera_media_preview_chain", return_value={"failures": []}) as selected, \
                patch.object(families, "collect_camera_device_lifecycle") as lifecycle, \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            report = families.audit_camera_media_preview_chain(argparse.Namespace(family="accessories", function="main",
                product=[3595, 3596], prepare=False))
        selected.assert_called_once()
        lifecycle.assert_not_called()
        self.assertIs(publish.call_args.args[0], report)
        self.assertEqual(publish.call_args.args[1], families.a.ROOT / ".analysis/results/frontend/accessories-camera-local-pages-audit.json")
        self.assertEqual(publish.call_args.args[3], "media_preview_chain")
        self.assertTrue(report["bounded_observation_complete"])
        self.assertTrue(report["snapshot_stable"])
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["whole_product_complete"])
        self.assertFalse(report["target_execution"])
        self.assertEqual(report["official_dll_comparison"], "not_executed")

    def test_failed_collection_or_changed_snapshot_preserves_unresolved_facts(self):
        import argparse
        for mode in ("exception", "returned_failure", "changed"):
            configured = ({"side_effect": ValueError("unresolved preview owner")} if mode == "exception" else {
                "return_value": {"failures": ["unresolved preview owner"] if mode == "returned_failure" else []}})
            with self.subTest(mode=mode), \
                    patch.object(families, "collect_camera_media_preview_chain", **configured), \
                    patch.object(host.HostLocalAudit, "final_snapshot", return_value=["preview-owner"] if mode == "changed" else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "camera media preview observation remains unresolved"):
                families.audit_camera_media_preview_chain(argparse.Namespace(family="accessories", function="main",
                    product=[3595, 3596], prepare=False))
            report = publish.call_args.args[0]
            self.assertEqual(publish.call_args.args[3], "media_preview_chain")
            self.assertFalse(report["bounded_observation_complete"])
            self.assertFalse(report["source_chain_complete"])
            self.assertEqual(report["snapshot_stable"], mode != "changed")
            self.assertIn("preview-owner" if mode == "changed" else "unresolved preview owner", report["failures"][0])
            self.assertFalse(report["target_execution"])


class CameraDeviceLifecycleDispatchTests(unittest.TestCase):
    def test_success_updates_only_the_selected_lifecycle_section(self):
        import argparse
        with patch.object(families, "collect_camera_device_lifecycle", return_value={"failures": []}) as selected, \
                patch.object(families, "collect_camera_image_host_chain") as host_chain, \
                patch.object(families, "collect_audio_driver_lifecycle") as audio, \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            result = families.audit_camera_device_lifecycle(argparse.Namespace(family="accessories", function="main",
                product=[3595, 3596], prepare=False))
        selected.assert_called_once()
        host_chain.assert_not_called()
        audio.assert_not_called()
        report, destination, _, section = publish.call_args.args
        self.assertIs(result, report)
        self.assertEqual(section, "device_lifecycle")
        self.assertEqual(destination, families.a.ROOT / ".analysis/results/frontend/accessories-camera-local-pages-audit.json")
        self.assertTrue(report["bounded_observation_complete"])
        self.assertTrue(report["snapshot_stable"])
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["whole_product_complete"])
        self.assertFalse(report["target_execution"])
        self.assertEqual(report["official_dll_comparison"], "not_executed")

    def test_query_routes_without_replaying_properties_host_or_audio(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "accessories", "--function", "main",
            "--query", "camera-device-lifecycle", "--product", "3595", "--product", "3596"])
        with patch.object(families, "audit_camera_device_lifecycle") as selected, \
                patch.object(families, "audit_camera_image_properties") as image, \
                patch.object(families, "audit_camera_image_host_chain") as host_chain, \
                patch.object(families, "audit_audio_driver_lifecycle") as audio:
            families.audit_family_function(args)
        selected.assert_called_once_with(args)
        for shared in (image, host_chain, audio):
            shared.assert_not_called()

    def test_scope_product_and_production_preparation_are_rejected(self):
        import argparse
        for family, function in (("audio", "main"), ("accessories", "mw")):
            with self.subTest(family=family, function=function), \
                    patch.object(families, "audit_camera_device_lifecycle") as selected, \
                    self.assertRaisesRegex(ValueError, "current accessories main scope"):
                families.audit_family_function(argparse.Namespace(family=family, function=function,
                    query="camera-device-lifecycle"))
            selected.assert_not_called()
        for products, prepare in (([3595], False), ([3596], False), ([3595, 3596, 3597], False), ([3595, 3596], True)):
            with self.subTest(products=products, prepare=prepare), \
                    self.assertRaisesRegex(ValueError, "PID3595 and PID3596 without production preparation"):
                families.audit_camera_device_lifecycle(argparse.Namespace(family="accessories", function="main",
                    product=products, prepare=prepare))

    def test_failed_collection_or_changed_snapshot_preserves_unresolved_facts(self):
        import argparse
        for mode in ("exception", "returned_failure", "changed"):
            returned = {"failures": ["unresolved current camera owner"]} if mode == "returned_failure" else {}
            configured = {"side_effect": ValueError("unresolved current camera owner")} if mode == "exception" else {
                "return_value": returned}
            with self.subTest(mode=mode), \
                    patch.object(families, "collect_camera_device_lifecycle", **configured), \
                    patch.object(host.HostLocalAudit, "final_snapshot", return_value=["camera-owner"] if mode == "changed" else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "camera device lifecycle observation remains unresolved"):
                families.audit_camera_device_lifecycle(argparse.Namespace(family="accessories", function="main",
                    product=[3595, 3596], prepare=False))
            report, _, _, section = publish.call_args.args
            self.assertEqual(section, "device_lifecycle")
            self.assertFalse(report["bounded_observation_complete"])
            self.assertFalse(report["source_chain_complete"])
            self.assertEqual(report["snapshot_stable"], mode != "changed")
            self.assertIn("camera-owner" if mode == "changed" else "unresolved current camera owner", report["failures"][0])
            self.assertFalse(report["target_execution"])
            self.assertEqual(report["official_dll_comparison"], "not_executed")


class CameraImageHostChainDispatchTests(unittest.TestCase):
    def test_query_routes_to_its_own_collector_without_replaying_image_properties(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "accessories", "--function", "main",
            "--query", "camera-image-host-chain", "--product", "3595", "--product", "3596"])
        with patch.object(families, "audit_camera_image_host_chain") as selected, \
                patch.object(families, "audit_camera_image_properties") as image, \
                patch.object(families, "audit_camera_properties_lifecycle") as lifecycle:
            families.audit_family_function(args)
        selected.assert_called_once_with(args)
        image.assert_not_called()
        lifecycle.assert_not_called()

    def test_scope_and_production_preparation_are_rejected(self):
        import argparse
        for family, function in (("audio", "main"), ("accessories", "mw")):
            with self.subTest(family=family, function=function), \
                    patch.object(families, "audit_camera_image_host_chain") as selected, \
                    self.assertRaisesRegex(ValueError, "current accessories main scope"):
                families.audit_family_function(argparse.Namespace(family=family, function=function,
                    query="camera-image-host-chain"))
            selected.assert_not_called()
        for products, prepare in (([3595], False), ([3596], False), ([3595, 3596, 3597], False), ([3595, 3596], True)):
            with self.subTest(products=products, prepare=prepare), \
                    self.assertRaisesRegex(ValueError, "PID3595 and PID3596 without production preparation"):
                families.audit_camera_image_host_chain(argparse.Namespace(family="accessories", function="main",
                    product=products, prepare=prepare))

    def test_failure_and_changed_snapshot_cannot_be_published_as_completion(self):
        import argparse
        for changed in (False, True):
            with self.subTest(changed=changed), \
                    patch.object(families, "collect_camera_image_host_chain", **({"return_value": {}} if changed
                        else {"side_effect": ValueError("missing current camera host owner")})), \
                    patch.object(host.HostLocalAudit, "final_snapshot", return_value=["camera-owner"] if changed else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "camera IMAGE host observation remains unresolved"):
                families.audit_camera_image_host_chain(argparse.Namespace(family="accessories", function="main",
                    product=[3595, 3596], prepare=False))
            report, _, _, section = publish.call_args.args
            self.assertEqual(section, "image_host_chain")
            self.assertFalse(report["bounded_observation_complete"])
            self.assertFalse(report["source_chain_complete"])
            self.assertEqual(report["snapshot_stable"], not changed)
            self.assertIn("camera-owner" if changed else "missing current camera host owner", report["failures"][0])
            self.assertFalse(report["target_execution"])
            self.assertEqual(report["official_dll_comparison"], "not_executed")


class CameraAndHanboChainDispatchTests(unittest.TestCase):
    def test_new_accessories_queries_route_without_replaying_old_capture_audits(self):
        import argparse
        for query, publisher, products in (
                ("camera-image-properties", "audit_camera_image_properties", [3595, 3596]),
                ("hanbo-gpu-temperature-chain", "audit_hanbo_gpu_temperature_chain", [3893]),
                ("hanbo-system-monitor-versions", "audit_hanbo_system_monitor_versions", [3893])):
            parser = argparse.ArgumentParser()
            families.add_commands(parser.add_subparsers(dest="command", required=True))
            command = ["audit-family-function", "--family", "accessories", "--function", "main", "--query", query]
            for product in products:
                command.extend(["--product", str(product)])
            args = parser.parse_args(command)
            with self.subTest(query=query), patch.object(families, publisher) as selected, \
                    patch.object(families, "audit_hanbo_local_runtime") as hanbo, \
                    patch.object(families, "audit_camera_local_pages") as camera:
                families.audit_family_function(args)
            selected.assert_called_once_with(args)
            hanbo.assert_not_called()
            camera.assert_not_called()

    def test_new_accessories_queries_reject_unrelated_family_and_middleware(self):
        import argparse
        for query, publisher in (("camera-image-properties", "audit_camera_image_properties"),
                                 ("hanbo-gpu-temperature-chain", "audit_hanbo_gpu_temperature_chain"),
                                 ("hanbo-system-monitor-versions", "audit_hanbo_system_monitor_versions")):
            for family, function in (("audio", "main"), ("accessories", "mw")):
                args = argparse.Namespace(family=family, function=function, query=query)
                with self.subTest(query=query, family=family, function=function), \
                        patch.object(families, publisher) as selected, \
                        self.assertRaisesRegex(ValueError, "current accessories main scope"):
                    families.audit_family_function(args)
                selected.assert_not_called()


class HanboSystemMonitorVersionsDispatchTests(unittest.TestCase):
    def test_product_scope_and_production_preparation_are_rejected_before_collection(self):
        import argparse
        for products, prepare in (([3900], False), ([3893, 3900], False), ([3893], True)):
            with self.subTest(products=products, prepare=prepare), \
                    patch.object(families, "collect_hanbo_system_monitor_versions") as collect, \
                    self.assertRaisesRegex(ValueError, "accessories/main PID3893 without preparation"):
                families.audit_hanbo_system_monitor_versions(argparse.Namespace(family="accessories", function="main",
                    product=products, prepare=prepare, query="hanbo-system-monitor-versions"))
            collect.assert_not_called()

    def test_success_publishes_only_the_selected_system_monitor_section(self):
        import argparse
        with patch.object(families, "collect_hanbo_system_monitor_versions", return_value={"failures": []}) as collect, \
                patch.object(families, "collect_hanbo_gpu_temperature_chain") as gpu, \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            report = families.audit_hanbo_system_monitor_versions(argparse.Namespace(family="accessories", function="main",
                product=[3893], prepare=False, query="hanbo-system-monitor-versions"))
        collect.assert_called_once()
        gpu.assert_not_called()
        self.assertIs(publish.call_args.args[0], report)
        self.assertEqual(publish.call_args.args[1], families.a.ROOT / ".analysis/results/frontend/accessories-main-audit.json")
        self.assertEqual(publish.call_args.args[3], "hanbo_system_monitor_versions")
        self.assertTrue(report["bounded_observation_complete"])
        self.assertTrue(report["snapshot_stable"])
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["target_execution"])
        self.assertEqual(report["runtime_acceptance"], "unverified")

    def test_collection_failure_or_changed_snapshot_remains_unresolved(self):
        import argparse
        for mode in ("exception", "returned_failure", "changed"):
            configured = ({"side_effect": ValueError("unresolved monitor owner")} if mode == "exception" else {
                "return_value": {"failures": ["unresolved monitor owner"] if mode == "returned_failure" else []}})
            with self.subTest(mode=mode), \
                    patch.object(families, "collect_hanbo_system_monitor_versions", **configured), \
                    patch.object(host.HostLocalAudit, "final_snapshot", return_value=["monitor-owner"] if mode == "changed" else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "Hanbo system monitor versions observation remains unresolved"):
                families.audit_hanbo_system_monitor_versions(argparse.Namespace(family="accessories", function="main",
                    product=[3893], prepare=False, query="hanbo-system-monitor-versions"))
            report = publish.call_args.args[0]
            self.assertEqual(publish.call_args.args[3], "hanbo_system_monitor_versions")
            self.assertFalse(report["bounded_observation_complete"])
            self.assertFalse(report["source_chain_complete"])
            self.assertEqual(report["snapshot_stable"], mode != "changed")
            self.assertIn("monitor-owner" if mode == "changed" else "unresolved monitor owner", report["failures"][0])
            self.assertFalse(report["target_execution"])


class AudioEndpointNotificationDispatchTests(unittest.TestCase):
    QUERY = "audio-endpoint-notification-chain"
    COLLECTOR = "collect_audio_endpoint_notification_chain"
    WRAPPER = "audit_audio_endpoint_notification_chain"
    SECTION = "endpoint_notification_chain"
    UNRESOLVED = "audio endpoint notification observation remains unresolved"

    def test_query_routes_without_replaying_endpoint_or_driver_collectors(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "audio", "--function", "mw",
            "--query", self.QUERY, "--product", "1422"])
        with patch.object(families, self.WRAPPER) as selected, \
                patch.object(families, "audit_audio_endpoint_local_chain") as endpoint, \
                patch.object(families, "audit_audio_driver_lifecycle") as driver:
            families.audit_family_function(args)
        selected.assert_called_once_with(args)
        endpoint.assert_not_called()
        driver.assert_not_called()

    def test_scope_products_and_production_preparation_are_rejected(self):
        import argparse
        for family, function in (("accessories", "mw"), ("audio", "main")):
            with self.subTest(family=family, function=function), \
                    patch.object(families, self.WRAPPER) as selected, \
                    self.assertRaisesRegex(ValueError, "current audio middleware scope"):
                families.audit_family_function(argparse.Namespace(family=family, function=function,
                    query=self.QUERY))
            selected.assert_not_called()
        for products, prepare in (([1423], False), ([1422, 1423], False), ([1422], True)):
            with self.subTest(products=products, prepare=prepare), \
                    self.assertRaisesRegex(ValueError, "PID1422 without production preparation"):
                getattr(families, self.WRAPPER)(argparse.Namespace(family="audio", function="mw",
                    product=products, prepare=prepare))

    def test_success_updates_only_the_selected_section(self):
        import argparse
        with patch.object(families, self.COLLECTOR, return_value={"failures": []}) as selected, \
                patch.object(families, "collect_audio_endpoint_local_chain") as endpoint, \
                patch.object(families, "collect_audio_driver_lifecycle") as driver, \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            report = getattr(families, self.WRAPPER)(argparse.Namespace(family="audio", function="mw",
                product=[1422], prepare=False))
        selected.assert_called_once()
        endpoint.assert_not_called()
        driver.assert_not_called()
        self.assertIs(publish.call_args.args[0], report)
        self.assertEqual(publish.call_args.args[1], families.a.ROOT / ".analysis/results/frontend/audio-MapAudio-audit.json")
        self.assertEqual(publish.call_args.args[3], self.SECTION)
        self.assertTrue(report["bounded_observation_complete"])
        self.assertTrue(report["snapshot_stable"])
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["whole_product_complete"])
        self.assertFalse(report["target_execution"])
        self.assertEqual(report["official_dll_comparison"], "not_executed")

    def test_failed_collection_or_changed_snapshot_preserves_unresolved_facts(self):
        import argparse
        for mode in ("exception", "returned_failure", "changed"):
            configured = ({"side_effect": ValueError("unresolved notification owner")} if mode == "exception" else {
                "return_value": {"failures": ["unresolved notification owner"] if mode == "returned_failure" else []}})
            with self.subTest(mode=mode), \
                    patch.object(families, self.COLLECTOR, **configured), \
                    patch.object(host.HostLocalAudit, "final_snapshot", return_value=["notification-owner"] if mode == "changed" else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, self.UNRESOLVED):
                getattr(families, self.WRAPPER)(argparse.Namespace(family="audio", function="mw",
                    product=[1422], prepare=False))
            report = publish.call_args.args[0]
            self.assertEqual(publish.call_args.args[3], self.SECTION)
            self.assertFalse(report["bounded_observation_complete"])
            self.assertFalse(report["source_chain_complete"])
            self.assertEqual(report["snapshot_stable"], mode != "changed")
            self.assertIn("notification-owner" if mode == "changed" else "unresolved notification owner", report["failures"][0])
            self.assertFalse(report["target_execution"])


class AudioDriverCloseQuiescenceDispatchTests(AudioEndpointNotificationDispatchTests):
    QUERY = "audio-driver-close-quiescence"
    COLLECTOR = "collect_audio_driver_close_quiescence"
    WRAPPER = "audit_audio_driver_close_quiescence"
    SECTION = "driver_close_quiescence"
    UNRESOLVED = "audio driver close quiescence observation remains unresolved"


class AudioEndpointLocalChainDispatchTests(unittest.TestCase):
    def test_endpoint_query_routes_without_replaying_dsp_or_legacy_audits(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "audio", "--function", "mw",
                                  "--query", "audio-endpoint-local-chain", "--product", "1422"])
        with patch.object(families, "audit_audio_endpoint_local_chain") as selected, \
                patch.object(families, "audit_audio_seiren_native_dsp") as dsp, \
                patch.object(families, "audit_audio_seiren_eq") as legacy:
            families.audit_family_function(args)
        selected.assert_called_once_with(args)
        dsp.assert_not_called()
        legacy.assert_not_called()

    def test_endpoint_query_rejects_other_scope_and_production_preparation(self):
        import argparse
        for family, function in (("mouse", "mw"), ("audio", "ui")):
            args = argparse.Namespace(family=family, function=function, query="audio-endpoint-local-chain")
            with self.subTest(family=family, function=function), \
                    patch.object(families, "audit_audio_endpoint_local_chain") as selected, \
                    self.assertRaisesRegex(ValueError, "current audio middleware scope"):
                families.audit_family_function(args)
            selected.assert_not_called()
        for products, prepare in (([1422], True), ([1422, 1423], False), ([1423], False)):
            with self.subTest(products=products, prepare=prepare), \
                    self.assertRaisesRegex(ValueError, "PID1422 without production preparation"):
                families.audit_audio_endpoint_local_chain(argparse.Namespace(product=products, prepare=prepare))

    def test_endpoint_publisher_preserves_failure_and_changed_input_facts(self):
        import argparse
        import host
        for changed in (False, True):
            with self.subTest(changed=changed), \
                    patch.object(families, "collect_audio_endpoint_local_chain", **(
                        {"return_value": {}} if changed else {"side_effect": ValueError("missing endpoint owner")})), \
                    patch.object(host.HostLocalAudit, "final_snapshot", return_value=["endpoint.dll"] if changed else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "audio endpoint observation remains unresolved"):
                families.audit_audio_endpoint_local_chain(argparse.Namespace(product=[1422], prepare=False))
            report, _, _, section = publish.call_args.args
            self.assertEqual(section, "endpoint_local_chain")
            self.assertFalse(report["bounded_observation_complete"])
            self.assertEqual(report["snapshot_stable"], not changed)
            self.assertIn("endpoint.dll" if changed else "missing endpoint owner", report["failures"][0])
            self.assertFalse(report["target_execution"])
            self.assertEqual(report["official_dll_comparison"], "not_executed")


class AudioSeirenNativeDspDispatchTests(unittest.TestCase):
    def test_current_dsp_query_routes_without_legacy_preparation(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "audio", "--function", "mw",
                                  "--query", "audio-seiren-native-dsp", "--product", "1422"])
        with patch.object(families, "audit_audio_seiren_native_dsp") as current, \
                patch.object(families, "audit_audio_seiren_eq") as legacy:
            families.audit_family_function(args)
        current.assert_called_once_with(args)
        legacy.assert_not_called()

    def test_current_dsp_query_rejects_other_family_or_ui_scope(self):
        import argparse
        for family, function in (("mouse", "mw"), ("audio", "ui")):
            args = argparse.Namespace(family=family, function=function, query="audio-seiren-native-dsp")
            with self.subTest(family=family, function=function), \
                    patch.object(families, "audit_audio_seiren_native_dsp") as current, \
                    self.assertRaisesRegex(ValueError, "current audio middleware scope"):
                families.audit_family_function(args)
            current.assert_not_called()


class AudioSeirenCurrentMwDispatchTests(unittest.TestCase):
    def test_current_mw_query_routes_without_legacy_data_preparation(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "audio", "--function", "mw",
                                  "--query", "audio-seiren-current-mw", "--product", "1422"])
        with patch.object(families, "audit_audio_seiren_current_mw") as current, \
                patch.object(families, "audit_audio_seiren_eq") as legacy:
            families.audit_family_function(args)
        current.assert_called_once_with(args)
        legacy.assert_not_called()

    def test_current_mw_query_rejects_other_family_or_ui_scope(self):
        import argparse
        for family, function in (("mouse", "mw"), ("audio", "ui")):
            args = argparse.Namespace(family=family, function=function, query="audio-seiren-current-mw")
            with self.subTest(family=family, function=function), \
                    patch.object(families, "audit_audio_seiren_current_mw") as current, \
                    self.assertRaisesRegex(ValueError, "current audio middleware scope"):
                families.audit_family_function(args)
            current.assert_not_called()


class AudioSeirenCurrentMwTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not (analysis.ROOT / ".analysis/results/frontend/audio-MapAudio-audit.json").is_file():
            raise unittest.SkipTest("Retained Seiren UI/MW evidence is unavailable")
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_audio_seiren_current_mw(cls.audit)
        cls.original_bytes = {"mw": {}, "ui": {}}
        for owner in cls.audit.owners.values():
            original = owner["original"]
            for layer in cls.original_bytes:
                if "/1422/" + layer + "/" in original["path"] and original["path"].endswith(".js"):
                    cls.original_bytes[layer][original["path"]] = cls.audit.read(original)

    def owner(self, role):
        return self.audit.owners[self.result["chain"]["roles"][role]]

    def analyze(self, role, before, after):
        owner = self.owner(role)
        self.assertIn(before, owner["body"])
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", root)
        patched.start()
        self.addCleanup(patched.stop)
        audit = host.HostLocalAudit()
        originals = {"mw": [], "ui": []}
        for layer, sources in self.original_bytes.items():
            for relative, original in sources.items():
                raw = original
                if relative == owner["original"]["path"]:
                    changed = owner["body"].replace(before, after, 1).encode()
                    raw = original[:owner["byte_start"]] + changed + original[owner["byte_end"]:]
                path = root / ".analysis/inputs/frontend/seiren-fixture" / layer / Path(relative).name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(raw)
                originals[layer].append({"path": path.relative_to(root).as_posix(), "bytes": len(raw),
                                         "sha256": hashlib.sha256(raw).hexdigest()})
        return families.audio_seiren_current_mw_chain(audit, originals["mw"], originals["ui"])

    def test_current_sources_and_native_dependencies_keep_incomplete_semantics_explicit(self):
        selected = self.result["source_sets"]["mw"]
        self.assertEqual(selected["canonical_entry_url"], "https://apps.razer.com/synapse/products/1422/mw/")
        self.assertIsNone(selected["capture"])
        self.assertIn("complete_capture_unavailable", selected["capture_failures"])
        self.assertTrue(selected["failed_acquisition"])
        self.assertTrue(selected["unresolved_acquisition"])
        self.assertFalse(self.result["source_chain_complete"])
        self.assertFalse(self.result["chain"]["source_chain_complete"])
        self.assertFalse(self.result["whole_product_complete"])
        native = self.result["native_gain"]
        self.assertEqual(native["resource"]["resourceName"], "MarieNativeDLL")
        self.assertFalse(native["whole_native_semantics_complete"])
        self.assertEqual(native["official_dll_comparison"], "not_executed")
        self.assertEqual(len(native["slice"]["complete_functions"]), 20)
        self.assertTrue(native["slice"]["unexpanded_direct_edges"])
        self.assertTrue(native["slice"]["indirect_or_import_calls"])

    def test_command_selection_uses_actual_distinct_get_set_helpers(self):
        self.assertEqual({name: row["command"] for name, row in self.result["chain"]["hid_commands"].items()}, {
            "getReturnSwitchStatus": [2, 8, 162], "setReturnSwitchStatus": [2, 8, 34],
            "getNoiseGate": [9, 8, 155], "setNoiseGate": [9, 8, 27],
            "getAINoiseSuppression": [2, 8, 156], "setAINoiseSuppression": [2, 8, 28],
            "getMicCompressor": [9, 8, 157], "setMicCompressor": [9, 8, 29],
            "getMicReverb": [3, 8, 159], "setMicReverb": [3, 8, 31],
            "getAudioInputControlState": [3, 8, 150], "setAudioInputControlState": [3, 8, 22],
            "getParametricEQSetting": [9, 8, 154], "setParametricEQSetting": [9, 8, 26],
            "getParametricEQStatus": [11, 8, 161], "setParametricEQStatus": [11, 8, 33]})
        self.assertIn("yield e.setReturnSwitchStatus(0,t),!0", self.owner("setMicReturnSwitch")["body"])
        self.assertEqual(self.result["chain"]["events"]["ON_MIC_RETURN_SWITCH"]["storage"], "Storage_NoSave")
        task = self.owner("manager_taskRunnerSetDevice")["body"]
        self.assertLess(task.index("a.result=n"), task.index("r.signal.aborted"))
        self.assertLess(task.index("o.postExecute(e)"), task.index('a.value.status="completed"'))
        self.assertIn("payload:t.payload", self.owner("ui_micEQEffectsReducer")["body"])

    def test_all_retained_owners_reproduce_original_utf8_bytes(self):
        for owner in self.audit.owners.values():
            raw = self.audit.read(owner["original"])[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(raw.decode(), owner["body"])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), owner["body_sha256"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_orphan_feature_cannot_borrow_a_still_declared_factory(self):
        with self.assertRaisesRegex(ValueError, "feature lazy target changed"):
            self.analyze("marie_fw_request", "i.bind(i,43828)", "i.bind(i,43829)")

    def test_return_switch_cannot_silently_change_profile_storage(self):
        with self.assertRaisesRegex(ValueError, "storage policy changed"):
            self.analyze("descriptor_ON_MIC_RETURN_SWITCH", "Storage_NoSave", "Storage_Profile")

    def test_protocol_setter_cannot_reuse_getter_command_evidence(self):
        with self.assertRaisesRegex(ValueError, "audited command selection changed"):
            self.analyze("adapter_setReturnSwitchStatus", "o.P6", "o.f_")

    def test_false_execute_result_cannot_be_replaced_by_completed_notice(self):
        with self.assertRaisesRegex(ValueError, "task_result_boundary"):
            self.analyze("manager_taskRunnerSetDevice", "a.result=n", "a.result=!0")

    def test_local_vocal_payload_does_not_prove_a_second_hardware_action(self):
        with self.assertRaisesRegex(ValueError, "vocal_first_key"):
            self.analyze("ui_eqEffectReducer", "Object.keys(t.payload)[0]", "Object.keys(t.payload)[1]")

    def test_native_set_order_and_actual_constructor_are_checked(self):
        with self.assertRaisesRegex(ValueError, "volume/mute order changed"):
            self.analyze("mixer_setMicVolume", "i.setMicrophoneVolume(n)",
                         "i.setMicrophoneMute(!o),i.setMicrophoneVolume(n)")
        with self.assertRaisesRegex(ValueError, "native owner"):
            self.analyze("native_constructor", "T=O", "T=T")

    def test_publication_separates_stable_observation_from_unfinished_source_chain(self):
        import argparse
        args = argparse.Namespace(product=[1422], prepare=False)
        for changed in ([], ["changed-source.js"]):
            with self.subTest(changed=changed):
                audit = host.HostLocalAudit()
                with patch.object(host, "HostLocalAudit", return_value=audit), \
                        patch.object(audit, "final_snapshot", return_value=changed), \
                        patch.object(families, "collect_audio_seiren_current_mw", return_value={"source_chain_complete": False}), \
                        patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
                    if changed:
                        with self.assertRaisesRegex(ValueError, "observation remains unresolved"):
                            families.audit_audio_seiren_current_mw(args)
                    else:
                        families.audit_audio_seiren_current_mw(args)
                report = publish.call_args.args[0]
                self.assertEqual(report["bounded_observation_complete"], not changed)
                self.assertFalse(report["source_chain_complete"])
                self.assertFalse(report["target_execution"])


class AudioDriverLifecycleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        receipt = analysis.ROOT / ".analysis/results/frontend/audio-MapAudio-audit.json"
        if not receipt.is_file():
            raise unittest.SkipTest("Retained current PID1422 driver evidence is unavailable")
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_audio_driver_lifecycle(cls.audit)

    def changed_source(self, role, before, after):
        owners = {key: dict(self.audit.owners[value]) for key, value in self.result["roles"].items()}
        self.assertIn(before, owners[role]["body"])
        owners[role]["body"] = owners[role]["body"].replace(before, after, 1)
        return families.audio_driver_source_contract(owners)

    def test_current_package_host_and_runtime_boundaries_are_separate(self):
        result = self.result
        self.assertEqual(result["kind"], "current_audio_driver_lifecycle")
        self.assertEqual(result["product_id"], 1422)
        self.assertEqual(result["host_release"]["version"], "4.0.896")
        self.assertTrue(all(row["member_offset_verified"] for row in result["host_release"]["members"]))
        self.assertTrue(result["selected_driver_semantics_reviewed"])
        self.assertTrue(result["source_contract"]["independently_reviewed_audio_route"])
        for key in ("source_chain_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(result[key])
        self.assertEqual(result["official_dll_comparison"], "not_executed")
        self.assertEqual(result["runtime_acceptance"], "unverified")
        self.assertEqual(result["configuration"]["actual_registered_path"], "unobserved")
        self.assertEqual(result["native_contract"]["selection"]["actual_loaded_module"], "unobserved")
        self.assertEqual(result["temporary_artifacts_created"], [])

    def test_audio_route_and_preload_capability_cannot_be_replaced_by_another_owner(self):
        self.assertEqual(self.result["source_contract"]["route"][1], "MW 80995.oE")
        for role, before, after in (
            ("native_callDLLApi", "yield(0,E.oE)", "yield(0,E.I3)"),
            ("preload_support", "supportElectron20:!0", "supportElectron20:!1"),
            ("preload_async", 'e.invoke("ffiPreloadAsync",t,i)', 'e.invoke("ffiSubPreloadAsync",t,i)'),
            ("mw_bridge", "{apiElectron:A}=window", "{apiElectron:A}=window.top")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "current source contract changed"):
                self.changed_source(role, before, after)
        self.assertEqual(self.result["source_contract"]["initialization_order"],
            ["ConfigureFFI", "isInit=true", "GetDLLVersion", "SetNodeFFIEvent", "DeviceInitialize"])
        self.assertFalse(self.result["source_contract"]["device_initialize_result_controls_isInit"])

    def test_current_package_configuration_guid_registration_files_and_order_are_bound(self):
        members = self.result["package"]["members"]
        raw = {name: self.audit.read(members[name]) for name in ("custom.ini", "tlsetupfx_drv.xml", "RazerSeirenV3Pro.inf")}
        contract = self.result["configuration"]
        self.assertEqual(contract["interface_guid"], "{D132049C-EE3B-4647-AC9E-E754FA90FD46}")
        self.assertEqual(contract["registration_files"], ["RazerSeirenV3Proapi.dll", "RazerSeirenV3Proapi_x64.dll",
            "RazerSeirenV3Proasio.dll", "RazerSeirenV3Proasio_x64.dll"])
        self.assertTrue(contract["registration_precedes_driver_install"])
        for filename, before, after in (
            ("custom.ini", b"D132049C", b"D132049D"),
            ("tlsetupfx_drv.xml", b"RazerSeirenV3Proapi_x64.dll", b"OtherDriver.dll")):
            altered = dict(raw)
            self.assertIn(before, altered[filename])
            altered[filename] = altered[filename].replace(before, after, 1)
            with self.subTest(filename=filename), self.assertRaisesRegex(ValueError, "configuration"):
                families.audio_driver_configuration_contract(*(altered[name] for name in raw))
        import xml.etree.ElementTree as ET
        root = ET.fromstring(raw["tlsetupfx_drv.xml"])
        install = root.find("./Task[@Id='Install']")
        register = install.find("./Action[@Id='RegisterInProcessServer']")
        install.remove(register)
        install.append(register)
        with self.assertRaisesRegex(ValueError, "registration order changed"):
            families.audio_driver_configuration_contract(raw["custom.ini"], ET.tostring(root), raw["RazerSeirenV3Pro.inf"])

    def test_api_5_14_request_is_compatible_with_unsigned_minor_5_15(self):
        contract = self.result["native_contract"]["api_compatibility"]
        self.assertEqual(contract["comparison"], "major == 5 && unsigned minor <= 15")
        self.assertEqual(contract["package_api_version"], "5.15")
        self.assertTrue(contract["request_5_14_accepted"])
        driver = copy.deepcopy(self.result["driver"])
        instruction = next(row for function in driver["complete_functions"]
            for row in function["capture"]["assembly"] if row["rva"] == "0x185a")
        instruction["bytes"] = "0f94c0"
        with self.assertRaisesRegex(ValueError, "physical lifecycle ABI changed"):
            families.audio_driver_native_contract(self.result["marie"], driver)

    def test_enqueue_ack_and_meter_timeout_do_not_prove_termination(self):
        contract = self.result["native_contract"]["termination"]
        self.assertTrue(contract["export_returns_after_enqueue"])
        self.assertFalse(contract["waits_for_container_removal"])
        self.assertEqual(contract["meter_thread_wait_ms"], 1000)
        self.assertTrue(contract["timeout_still_closes_thread_handle_and_continues"])
        self.assertFalse(contract["whole_callback_and_pending_task_quiescence_proven"])
        marie = copy.deepcopy(self.result["marie"])
        function = next(row["capture"] for row in marie["complete_functions"] if row["capture"]["rva"] == "0x34790")
        function["pseudocode"]["lines"].append({"text": "WaitForSingleObject(task, INFINITE);"})
        with self.assertRaisesRegex(ValueError, "queued termination waiting boundary changed"):
            families.audio_driver_native_contract(marie, self.result["driver"])

    def test_driver_device_must_close_before_api_unload_and_clear(self):
        contract = self.result["native_contract"]["termination"]
        self.assertTrue(contract["driver_device_closed_before_api_unload"])
        self.assertTrue(contract["api_function_table_cleared"])
        self.assertTrue(contract["driver_close_validates_slot_and_exact_handle"])
        marie = copy.deepcopy(self.result["marie"])
        function = next(row["capture"] for row in marie["complete_functions"] if row["capture"]["rva"] == "0x4bef0")
        lines = function["pseudocode"]["lines"]
        first = next(row for row in lines if "TUSBAUDIO_CloseDevice(*(unsigned int *)" in row["text"])
        second = next(row for row in lines if "FreeLibrary(hLibModule);" in row["text"])
        first["text"], second["text"] = second["text"], first["text"]
        with self.assertRaisesRegex(ValueError, "module/device close order changed"):
            families.audio_driver_native_contract(marie, self.result["driver"])

    def test_physical_destructor_tables_and_timeout_are_verified(self):
        for module, role in (("marie", "terminate_lambda"), ("marie", "device_refcount"), ("driver", "device_vtable")):
            altered = copy.deepcopy(self.result[module])
            region = next(row for row in altered["verified_data_regions"] if row["role"] == role)
            region["raw_hex"] = "00" * region["bytes"]
            args = (altered, self.result["driver"]) if module == "marie" else (self.result["marie"], altered)
            with self.subTest(module=module, role=role), self.assertRaisesRegex(ValueError, "lifecycle vtable changed"):
                families.audio_driver_native_contract(*args)
        self.assertEqual(len(self.result["native_contract"]["instruction_assertions"]), 14)

    def test_wrapper_listener_removal_and_host_mutex_are_not_callback_quiescence(self):
        source = self.result["source_contract"]
        self.assertFalse(source["native_callback_unregistered_by_wrapper"])
        self.assertFalse(source["free_ffi_releases_maps"])
        self.assertFalse(source["unload_awaits_termination"])
        self.assertFalse(source["whole_session_quiescence_proven"])
        for role, before, after in (
            ("host_async", "finally{r&&r()}", "finally{}"),
            ("native_handleBeforeUnload", "this.terminate()", "await this.terminate()")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "current source contract changed|unload waiting"):
                self.changed_source(role, before, after)

    def test_sources_native_chunks_section_dependencies_and_roundtrip_are_reproducible(self):
        for dependency in self.result["upstream_sections"]:
            self.assertNotIn(dependency["receipt"], self.audit.artifacts)
            section = json.loads((analysis.ROOT / dependency["receipt"]).read_bytes())[dependency["section"]]
            self.assertEqual(hashlib.sha256(json.dumps(section, ensure_ascii=False, sort_keys=True,
                separators=(",", ":")).encode()).hexdigest(), dependency["canonical_sha256"])
        for owner in self.audit.owners.values():
            self.assertEqual(host.verify_profiles_owner(self.audit, owner).text.decode(), owner["body"])
        self.assertEqual([len(self.result[name]["complete_functions"]) for name in ("marie", "driver")], [17, 21])
        for name in ("marie", "driver"):
            self.assertTrue(self.result[name]["all_selected_pseudocode_captured"])
            self.assertTrue(all(row["input_chunks_verified"] for row in self.result[name]["complete_functions"]))
        report = {**self.result, "complete_source_owners": list(self.audit.owners.values()),
                  "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertEqual(self.audit.final_snapshot(), [])


class AudioEndpointNotificationChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not (analysis.ROOT / ".analysis/results/frontend/audio-MapAudio-audit.json").is_file():
            raise unittest.SkipTest("Retained current PID1422 endpoint notification evidence is unavailable")
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_audio_endpoint_notification_chain(cls.audit)

    def changed_source(self, role, before, after):
        owners = {key: dict(self.audit.owners[value]) for key, value in self.result["roles"].items()}
        self.assertIn(before, owners[role]["body"])
        owners[role]["body"] = owners[role]["body"].replace(before, after, 1)
        owners[role]["body_sha256"] = hashlib.sha256(owners[role]["body"].encode()).hexdigest()
        return families._audio_endpoint_notification_source_contract(owners)

    def changed_native(self, rva, before, after):
        native = copy.deepcopy(self.result["native"])
        function = next(row["capture"] for row in native["complete_functions"] if row["capture"]["rva"] == rva)
        line = next(row for row in function["pseudocode"]["lines"] if before in row["text"])
        line["text"] = line["text"].replace(before, after, 1)
        return families._audio_endpoint_notification_native_contract(native)

    def test_selected_notification_review_keeps_all_other_delivery_stages_unverified(self):
        result = self.result
        self.assertEqual(result["kind"], "current_audio_endpoint_notification_chain")
        self.assertEqual(result["product_id"], 1422)
        self.assertTrue(result["selected_notification_semantics_reviewed"])
        self.assertEqual(result["host_release"]["version"], "4.0.896")
        self.assertEqual([row["query"] for row in result["upstream_sections"]], ["endpoint_local_chain", "driver_lifecycle"])
        for key in ("source_chain_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(result[key])
        self.assertFalse(result["native_contract"]["notification"]["complete_inflight_barrier_proven"])
        self.assertEqual(result["official_dll_comparison"], "not_executed")
        self.assertEqual(result["runtime_acceptance"], "unverified")
        self.assertEqual(result["temporary_artifacts_created"], [])

    def test_current_source_bytes_and_two_round_deployment_remain_separate_from_dependency_closure(self):
        proof = self.result["deployment_revalidation"]
        self.assertFalse(proof["full_dependency_closure_reviewed"])
        original = proof["owner"]["retained_original"]
        self.assertEqual(original["sha256"], "fc7ad62d2db2f6adc447b0129d8e6ea62012b9d5d80381d5f0103234f2b1c48c")
        self.assertEqual([(row["round"], row["http_status"], row["sha256"]) for row in proof["owner"]["rounds"]],
            [(1, 200, original["sha256"]), (2, 200, original["sha256"])])
        self.assertIsNone(self.result["source_set"]["capture"])
        self.assertIn("complete_capture_unavailable", self.result["source_set"]["capture_failures"])

    def test_host_callback_cache_url_lookup_and_container_filter_changes_are_rejected(self):
        for role, before, after in (
                ("host_callback", "r=a.SetNodeFFIEvent(h),this.ffiCallbackMap.set(o,h)", "r=a.SetNodeFFIEvent(h),r&&this.ffiCallbackMap.set(o,h)"),
                ("host_callback", "a=t.ffiPreloadMain.ffiLibMap.get(o)", "a=this.ffiLibMap.get(o)"),
                ("host_callback", "a.urlList.includes(e.getURL())", "a.urlList.includes(i)"),
                ("native_handleEvent", "this.deviceContainerId!==", "this.deviceContainerId===")):
            with self.subTest(role=role, before=before), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source(role, before, after)
        source = self.result["source_contract"]
        self.assertTrue(source["callback_false_result_still_cached"])
        self.assertFalse(source["connection_generation_checked"])
        self.assertFalse(source["free_ffi_releases_maps"])

    def test_registration_default_callback_and_listener_removal_cannot_be_recast_as_drain(self):
        for role, before, after in (
                ("native_setNodeFFIEvent", '{action:"SetNodeFFIEvent"}', '{action:"SetNodeFFIEvent",payload:{useNewCallback:!0}}'),
                ("ffi_callback_factory", "isAnneCallback = false", "isAnneCallback = true"),
                ("native_terminate", "this.removeListener()", "await this.removeListener()"),
                ("native_handleBeforeUnload", "this.terminate()", "await this.terminate()"),
                ("host_async", 'case"FreeFFI":return!0', 'case"FreeFFI":return!1')):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source(role, before, after)
        self.assertFalse(self.result["source_contract"]["pid1422_anne_callback_requested"])
        self.assertFalse(self.result["source_contract"]["before_unload_awaits_terminate"])

    def test_worker_priority_move_and_unlock_before_invoke_are_bound_to_complete_functions(self):
        self.assertEqual(self.result["native_contract"]["worker"]["queue_count_priority_offsets"], [208, 288, 248])
        for rva, before, after in (("0x5a9c0", "v13 = 2", "v13 = 1"),
                ("0x5a9c0", "*(_QWORD *)v17 = 0", "*(_QWORD *)v17 = v8"),
                ("0x5a9c0", "Mtx_unlock((_Mtx_t)(v1 + 16))", "Mtx_lock((_Mtx_t)(v1 + 16))"),
                ("0x5b4f0", "+ 16LL", "+ 24LL")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.changed_native(rva, before, after)

    def test_initialization_wait_reuse_and_termination_enqueue_do_not_claim_cancellation(self):
        worker = self.result["native_contract"]["worker"]
        self.assertEqual(worker["initialize_wait_seconds"], 30)
        self.assertFalse(worker["initialize_timeout_cancels_task"])
        self.assertFalse(worker["terminate_waits_for_removal"])
        self.assertFalse(worker["ordinary_terminate_stops_worker"])
        for rva, before, after in (("0x33ad0", "v20 = 30", "v20 = 0"),
                ("0x34ca0", "v8 = 1", "v8 = 0"), ("0x34790", "result = 1", "result = 0"),
                ("0x5c610", "lstrcmpiW(v8, a2)", "lstrcmpW(v8, a2)")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.changed_native(rva, before, after)

    def test_stop_joins_worker_and_disposes_pending_type1_tasks_without_executing_them(self):
        for rva, before, after in (("0x5a3e0", "*(_BYTE *)(a1 + 168) = 0", "*(_BYTE *)(a1 + 168) = 1"),
                ("0x5b1e0", "sub_180037BE0", "sub_18005B4F0"),
                ("0x37be0", "sub_180038DD0", "sub_18005B4F0")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.changed_native(rva, before, after)
        self.assertIn("without invocation", self.result["native_contract"]["worker"]["stop"])

    def test_copied_raw_device_callback_and_meter_timeout_remain_independent_of_clear(self):
        lifetime = self.result["native_contract"]["device_lifetime"]
        self.assertTrue(lifetime["meter_timeout_still_continues_destructor"])
        self.assertIn("raw CCamyDevice pointer", lifetime["callback_capture"])
        for rva, before, after in (("0xa9b0", "a2[1] = *(_QWORD *)(a1 + 8)", "a2[1] = 0"),
                ("0x509d0", "Mtx_unlock((_Mtx_t)v12)", "Mtx_lock((_Mtx_t)v12)"),
                ("0x50cd0", "Mtx_unlock", "Mtx_lock"),
                ("0x51180", "a1 + 600", "a1 + 608"),
                ("0x4a50", "0x3E8u", "0xFFFFFFFFu")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.changed_native(rva, before, after)

    def test_actual_worker_calls_callback_calls_and_indirect_slots_cannot_be_substituted(self):
        for rva in ("0x5b52b", "0x6463", "0x5af63"):
            native = copy.deepcopy(self.result["native"])
            row = next(row for function in native["complete_functions"] for row in function["capture"]["assembly"] if row["rva"] == rva)
            row["bytes"] = "90" * (len(row["bytes"]) // 2)
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "physical worker/callback ABI changed|actual native edge changed"):
                families._audio_endpoint_notification_native_contract(native)
        for role in ("initialize_lambda", "terminate_lambda", "device_refcount", "device_callback_vtable"):
            native = copy.deepcopy(self.result["native"])
            row = next(row for row in native["verified_data_regions"] if row["role"] == role)
            row["raw_hex"] = "00" * row["bytes"]
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "actual native slot changed"):
                families._audio_endpoint_notification_native_contract(native)

    def test_shared_addon_queue_is_reused_and_does_not_become_device_generation_protection(self):
        dependency = self.result["shared_addon_dependency"]
        self.assertEqual(dependency["pointer"], "/verified_contract/addon")
        self.assertEqual(dependency["provenance"]["version"], "1.0.11")
        self.assertEqual(self.result["addon_native"]["original"]["sha256"], dependency["provenance"]["original"]["sha256"])
        self.assertEqual({row["capture"]["rva"] for row in self.result["addon_native"]["complete_functions"]},
            {"0x94c0", "0xa0f0", "0x93e0", "0x8c20"})
        self.assertIn("waits on a condition", dependency["semantics"])
        self.assertIn("not device cancellation", dependency["finalization_and_limits"])

    def test_complete_owner_byte_roundtrip_and_final_snapshot_are_reproducible(self):
        for row in self.audit.owners.values():
            raw = self.audit.read(row["original"])[row["byte_start"]:row["byte_end"]]
            self.assertEqual(raw.decode(), row["body"])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), row["body_sha256"])
        contract = self.result["native_contract"]
        self.assertEqual(len(contract["verified_complete_boundary_hashes"]), 36)
        self.assertEqual(len(contract["verified_reference_edges"]), 21)
        self.assertEqual(len(contract["verified_instruction_anchors"]), 12)
        self.assertEqual(self.audit.final_snapshot(), [])


class AudioDriverCloseQuiescenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_audio_driver_close_quiescence(cls.audit)
        cls.previous = json.loads((analysis.ROOT / ".analysis/results/frontend/audio-MapAudio-audit.json").read_bytes())
        cls.raw = cls.audit.read(cls.result["native"]["original"])
        cls.reused = [cls.previous["driver_lifecycle"]["marie"], cls.previous["endpoint_notification_chain"]["native"],
            cls.previous["endpoint_local_chain"]["native"]]

    def verify_native(self, native=None, reused=None):
        return families._audio_driver_close_quiescence_native_contract(self.raw,
            self.result["native"] if native is None else native, self.reused if reused is None else reused)

    def changed_body(self, rva, before, after, *, reused=False):
        native, previous = copy.deepcopy(self.result["native"]), copy.deepcopy(self.reused)
        rows = [row for module in (previous if reused else [native]) for row in module["complete_functions"]
            if row["capture"]["rva"] == rva]
        self.assertTrue(rows)
        for row in rows:
            line = next(line for line in row["capture"]["pseudocode"]["lines"] if before in line["text"])
            line["text"] = line["text"].replace(before, after, 1)
        return self.verify_native(native, previous)

    def test_current_scope_reuses_prior_owners_without_promoting_quiescence(self):
        result = self.result
        self.assertEqual((result["kind"], result["query"], result["product_id"]),
            ("current_audio_driver_close_quiescence", "audio-driver-close-quiescence", 1422))
        self.assertEqual(result["host_release"]["version"], "4.0.896")
        self.assertTrue(result["selected_close_semantics_reviewed"])
        self.assertEqual({key: result["reused_native"][key] for key in
            ("marie_unique_functions", "driver_functions", "addon_functions", "new_marie_functions")},
            {"marie_unique_functions": 55, "driver_functions": 21, "addon_functions": 4, "new_marie_functions": 19})
        old = {row["capture"]["rva"] for module in self.reused for row in module["complete_functions"]}
        self.assertFalse(old.intersection(row["capture"]["rva"] for row in result["native"]["complete_functions"]))
        for key in ("source_chain_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(result[key])
        self.assertFalse(result["native_contract"]["destruction"]["complete_session_quiescence_proven"])
        self.assertEqual(result["official_dll_comparison"], "not_executed")

    def test_actual_com_table_preserves_three_events_and_two_noop_slots(self):
        contract = self.result["native_contract"]
        listener = contract["listener"]
        self.assertEqual(listener["events"], {"state_changed": 3, "added": 1, "removed": 2})
        self.assertEqual((listener["device_subobject_offset"], listener["com_callback_offset"]), (672, 688))
        self.assertEqual(contract["verified_indirect_slots"]["endpoint_change_listener"],
            ["0x573f0", "0x40880", "0x40880", "0x57230", "0x571d0", "0x57170", "0x16b0", "0x16b0"])
        self.assertFalse(listener["filter_by_endpoint_or_container_or_generation"])
        self.assertEqual(listener["payload"], "borrowed endpoint-ID pointer; size8; new state not forwarded")
        for rva, before, after in (("0x57230", "v6 = 3", "v6 = 4"), ("0x57170", "v5 = 8", "v5 = 4")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.changed_body(rva, before, after)

    def test_listener_registration_flag_and_ignored_hresult_remain_physical_facts(self):
        listener = self.result["native_contract"]["listener"]
        self.assertEqual(listener["addref_release"], "constant1")
        self.assertFalse(listener["unregister_is_inflight_barrier"])
        native = copy.deepcopy(self.result["native"])
        row = next(row for row in native["verified_data_regions"] if row["role"] == "endpoint_change_listener")
        row["raw_hex"] = "00" * row["bytes"]
        row["sha256"] = hashlib.sha256(bytes(row["bytes"])).hexdigest()
        with self.assertRaisesRegex(ValueError, "data region differs"):
            self.verify_native(native)
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_body("0x57340", "*(_DWORD *)(a1 + 24) = 0", "*(_DWORD *)(a1 + 24) = 1")

    def test_events_reset_one_shot_three_second_timer_without_device_ownership(self):
        delayed = self.result["native_contract"]["delayed_reacquire"]
        self.assertEqual((delayed["due_time_100ns"], delayed["delay_ms"]), (-30000000, 3000))
        self.assertFalse(delayed["device_ownership_added_by_lambda_copy"])
        self.assertFalse(delayed["generation_guard"])
        for rva, before, after in (("0x5a0c0", "-30000000LL", "-10000000LL"),
                ("0x533b0", "a2[1] = *(_QWORD *)(a1 + 8)", "a2[1] = 0")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.changed_body(rva, before, after)

    def test_timer_clears_callable_before_wait_and_does_not_drain_host_queue(self):
        delayed = self.result["native_contract"]["delayed_reacquire"]
        self.assertEqual(delayed["timer_destructor"], ["destroy/clear stored callable", "SetThreadpoolTimer(NULL)",
            "WaitForThreadpoolTimerCallbacks(cancel_pending=true)", "CloseThreadpoolTimer"])
        self.assertTrue(delayed["callable_cleared_before_timer_callbacks_wait"])
        self.assertFalse(delayed["native_timer_wait_reaches_host_ffi_queue"])
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_body("0x57290", "WaitForThreadpoolTimerCallbacks(*((PTP_TIMER *)Block + 1), 1)",
                "WaitForThreadpoolTimerCallbacks(*((PTP_TIMER *)Block + 1), 0)")

    def test_reacquisition_timeout_and_cached_replay_cannot_be_readback(self):
        delayed = self.result["native_contract"]["delayed_reacquire"]
        self.assertFalse(delayed["replayed_values_are_fresh_device_observation"])
        self.assertFalse(delayed["replay_results_checked"])
        for rva, before, after in (("0x51290", "10000000000LL", "1000000000LL"),
                ("0x51850", "if ( a1[72] && a1[73] )", "if ( a1[72] || a1[73] )"),
                ("0x51850", "Sleep(0x3E8u)", "Sleep(0x64u)")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.changed_body(rva, before, after)

    def test_api_unload_precedes_listener_unregister_in_complete_destructor(self):
        destruction = self.result["native_contract"]["destruction"]
        self.assertFalse(destruction["endpoint_listener_unregistered_before_api_unload"])
        self.assertFalse(destruction["endpoint_unregister_hresult_checked"])
        self.assertFalse(destruction["global_ffi_callback_unregistered"])
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_body("0x4bef0", "sub_180057340(a1 + 672)", "sub_180057340(a1 + 688)", reused=True)

    def test_meter_inflight_and_queued_termination_keep_their_original_boundaries(self):
        contract = self.result["native_contract"]
        self.assertTrue(contract["destruction"]["meter_timeout_still_continues_destruction"])
        self.assertFalse(contract["destruction"]["endpoint_callback_copy_owns_device"])
        self.assertFalse(contract["reconnect"]["terminate_export_waits_for_removal"])
        self.assertTrue(contract["reconnect"]["initialize_can_reuse_pending_removal_object"])
        self.assertFalse(contract["reconnect"]["ordinary_terminate_joins_global_worker"])
        self.assertFalse(contract["reconnect"]["initialize_timeout_cancels_task"])
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_body("0x4a50", "0x3E8u", "0xFFFFFFFFu", reused=True)
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_body("0x4cd0", "v2 + 19) < 0xFFFFFFu", "v2 + 19) < 0xFFFFFFFFu")

    def test_physical_instructions_and_actual_direct_edges_cannot_be_forged(self):
        contract = self.result["native_contract"]
        self.assertEqual((len(contract["verified_complete_boundary_hashes"]), len(contract["verified_reference_edges"]),
            len(contract["verified_instruction_anchors"])), (24, 13, 12))
        for rva in ("0x57372", "0x5a166", "0x5a0f6"):
            native = copy.deepcopy(self.result["native"])
            row = next(row for function in native["complete_functions"] for row in function["capture"]["assembly"] if row["rva"] == rva)
            row["bytes"] = "90" * (len(row["bytes"]) // 2)
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "instruction differs"):
                self.verify_native(native)

    def test_current_host_listener_removal_has_no_native_generation_barrier(self):
        source = self.result["source_contract"]
        self.assertFalse(source["connection_generation_checked"])
        self.assertFalse(source["free_ffi_releases_maps"])
        self.assertFalse(source["before_unload_awaits_terminate"])
        owners = {role: dict(self.audit.owners[owner]) for role, owner in self.result["roles"].items()}
        row = owners["host_callback"]
        row["body"] = row["body"].replace("a.urlList.includes(e.getURL())", "a.urlList.includes(i)", 1)
        row["body_sha256"] = hashlib.sha256(row["body"].encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "complete source boundary changed"):
            families._audio_endpoint_notification_source_contract(owners)

    def test_dependency_receipts_keep_failures_and_registration_separate_from_runtime(self):
        for dependency in self.result["upstream_sections"]:
            section = self.previous[dependency["section"]]
            self.assertEqual(hashlib.sha256(json.dumps(section, ensure_ascii=False, sort_keys=True,
                separators=(",", ":")).encode()).hexdigest(), dependency["canonical_sha256"])
        self.assertTrue(self.result["reused_native"]["existing_capture_failures_retained"])
        self.assertTrue(self.result["native"]["capture_failures"])
        registration = self.result["registration_contract"]
        self.assertFalse(registration["installer_executed"])
        self.assertEqual(registration["actual_registered_path"], "unobserved")
        self.assertFalse(self.result["native_contract"]["reconnect"]["other_driver_notification_branches_reviewed"])

    def test_complete_owner_roundtrip_snapshot_and_no_temporary_resources(self):
        self.assertEqual(len(self.audit.owners), 13)
        for row in self.audit.owners.values():
            self.assertEqual(host.verify_profiles_owner(self.audit, row).text.decode(), row["body"])
        self.assertTrue(all(row["input_chunks_verified"] for row in self.result["native"]["complete_functions"]))
        self.assertEqual(json.loads(json.dumps(self.result, ensure_ascii=False)), self.result)
        self.assertEqual(self.result["temporary_artifacts_created"], [])
        self.assertEqual(self.result["analysis_sessions_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class AudioEndpointLocalChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not (analysis.ROOT / ".analysis/results/frontend/audio-MapAudio-audit.json").is_file():
            raise unittest.SkipTest("Retained current audio endpoint evidence is unavailable")
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_audio_endpoint_local_chain(cls.audit)

    def owner(self, role):
        return self.audit.owners[self.result["roles"][role]]

    def source_contract(self, role, before, after):
        owners = {key: dict(self.owner(key)) for key in self.result["roles"]}
        self.assertIn(before, owners[role]["body"])
        owners[role]["body"] = owners[role]["body"].replace(before, after, 1)
        return families.audio_endpoint_source_contract(owners)

    def native_instruction(self, rva, changed):
        native = copy.deepcopy(self.result["native"])
        instruction = next(row for function in native["complete_functions"]
                           for row in function["capture"]["assembly"] if row["rva"] == rva)
        instruction["bytes"] = changed
        return families.audio_endpoint_native_contract(native)

    def test_current_bounded_chain_is_separate_from_runtime_and_whole_product(self):
        result = self.result
        self.assertEqual(result["kind"], "current_audio_endpoint_local_chain")
        self.assertEqual(result["product_id"], 1422)
        self.assertEqual(result["upstream_mw"]["query"], "seiren_current_mw")
        self.assertEqual(result["upstream_monitor"]["pointer"], "/monitor_driver/contract")
        self.assertTrue(result["selected_endpoint_semantics_reviewed"])
        self.assertFalse(result["source_chain_complete"])
        self.assertFalse(result["whole_product_complete"])
        self.assertFalse(result["target_execution"])
        self.assertEqual(result["official_dll_comparison"], "not_executed")
        self.assertEqual(result["temporary_artifacts_created"], [])
        for source in result["source_sets"].values():
            self.assertIsNone(source["capture"])
            self.assertIn("complete_capture_unavailable", source["capture_failures"])

    def test_selection_uses_active_friendly_names_and_service_is_not_retry_requirement(self):
        native = self.result["native_contract"]
        selected = native["selection"]
        self.assertEqual(selected["active_state_mask"], 1)
        self.assertFalse(selected["container_id_used_for_matching"])
        self.assertEqual(selected["attempts"], 10)
        self.assertEqual([(row["name"], row["flow"], row["device_id"]) for row in selected["endpoints"]],
            [("Microphone", "capture", 262145), ("Service", "capture", 262146), ("Headphones", "render", 524289)])
        self.assertEqual(selected["successful_retry_stop_requires"], ["Microphone", "Headphones"])
        self.assertEqual(native["initial_reads"]["service_scalar_write"], 1.0)
        self.assertFalse(native["initial_reads"]["service_write_hresult_checked"])
        self.assertFalse(native["initial_reads"]["constructor_cache"]["device_observation"])

    def test_initial_read_rounding_notification_truncation_and_result_failures_remain_distinct(self):
        native = self.result["native_contract"]
        self.assertEqual(native["initial_reads"]["volume_conversion"], "roundf(binary32_scalar * 100)")
        self.assertEqual(native["notification"]["volume_conversion"], "truncate binary32_scalar * 100")
        self.assertTrue(native["notification"]["same_context_ignored"])
        self.assertFalse(native["notification"]["initial_getters_emit_notifications"])
        self.assertEqual(native["notification"]["dispatch_order"], ["changed_volume", "changed_mute"])
        self.assertEqual(native["notification"]["payload_order"], ["device_id", "mute", "level"])
        for key in ("activate_hresult_checked", "mute_hresult_checked", "notification_registration_hresult_checked_by_owner"):
            self.assertFalse(native["initial_reads"][key])
        self.assertFalse(self.result["source_contract"]["setter_readback"])

    def test_event_container_subscription_and_unload_contracts_reject_silent_changes(self):
        for role, before, after in (
            ("native_handleEvent", "this.deviceContainerId!==", "this.deviceContainerId==="),
            ("mixer_handleAddLevelMeterListener", 'type:"MW_UPDATE_MIC_TO_UI"', 'type:"MW_CHANGE_VOLUME_TO_UI"'),
            ("mixer_stopStreamMixerRecording", ".offEvent(e)", ".onEvent(e)"),
            ("native_handleBeforeUnload", "this.terminate()", "await this.terminate()")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "endpoint source contract changed|asynchronous boundary"):
                self.source_contract(role, before, after)
        self.assertEqual(self.result["source_contract"]["runtime_getter_order"],
                         ["headphones_volume", "headphones_mute", "microphone_volume", "microphone_mute"])
        self.assertFalse(self.result["source_contract"]["unload_awaits_termination"])

    def test_actual_com_slots_float_argument_and_callback_call_are_byte_bound(self):
        for rva, changed in (("0x3ea65", "ff5020"), ("0x3eb34", "ff5078"),
                             ("0x5047d", "0f28d6"), ("0x6463", "ff151f030800")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "physical COM/callback ABI changed"):
                self.native_instruction(rva, changed)
        self.assertEqual(len(self.result["native_contract"]["instruction_assertions"]), 30)

    def test_actual_callback_vtable_guid_property_key_and_service_scalar_are_verified(self):
        for role in ("endpoint_callback_vtable", "microphone_callback_vtable", "event_context",
                     "friendly_name_key", "service_initial_scalar"):
            native = copy.deepcopy(self.result["native"])
            region = next(row for row in native["verified_data_regions"] if row["role"] == role)
            region["raw_hex"] = "00" * region["bytes"]
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "data contract changed|actual callback vtable changed"):
                families.audio_endpoint_native_contract(native)
        self.assertEqual(self.result["native_contract"]["callback_tables"]["endpoint_callback_vtable"][3], "0x3ec30")
        self.assertEqual(self.result["native_contract"]["callback_tables"]["device_callback_vtable"][2], "0xa9a0")

    def test_ignored_activation_result_and_release_cannot_be_recast_as_safe_completion(self):
        native = copy.deepcopy(self.result["native"])
        function = next(row["capture"] for row in native["complete_functions"] if row["capture"]["rva"] == "0x3e890")
        line = next(row for row in function["pseudocode"]["lines"] if "&unk_18006E048" in row["text"])
        line["text"] += "\n      if ( activation_result < 0 ) return activation_result;"
        with self.assertRaisesRegex(ValueError, "ignored Activate result boundary changed"):
            families.audio_endpoint_native_contract(native)
        release = self.result["native_contract"]["release"]
        self.assertEqual(release["ordinary_release_owner_offsets"], [576, 584])
        self.assertEqual(release["service_release"], "device destructor")
        self.assertFalse(release["unregister_hresult_checked"])
        self.assertFalse(release["destructor_retries_failed_unregistration_after_callback_clear"])
        self.assertEqual(release["callback_addref_release_return"], 1)
        self.assertFalse(release["callback_quiescence_proven"])

    def test_complete_source_native_bytes_and_roundtrip_are_reproducible(self):
        for owner in self.audit.owners.values():
            raw = self.audit.read(owner["original"])[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(raw.decode(), owner["body"])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), owner["body_sha256"])
        self.assertEqual(len(self.result["native"]["complete_functions"]), 26)
        self.assertTrue(all(row["input_chunks_verified"] for row in self.result["native"]["complete_functions"]))
        report = {**self.result, "complete_owners": list(self.audit.owners.values()),
                  "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertEqual(self.audit.final_snapshot(), [])


class AudioSeirenNativeDspTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not (analysis.ROOT / ".analysis/results/frontend/audio-MapAudio-audit.json").is_file():
            raise unittest.SkipTest("Retained current Seiren evidence is unavailable")
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_audio_seiren_native_dsp(cls.audit)
        cls.original_bytes = {owner["original"]["path"]: cls.audit.read(owner["original"])
                              for owner in cls.audit.owners.values()
                              if "/1422/mw/" in owner["original"]["path"]}

    def owner(self, role):
        return self.audit.owners[self.result["chain"]["roles"][role]]

    def analyze(self, role, before, after):
        owner = self.owner(role)
        self.assertIn(before, owner["body"])
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", root)
        patched.start()
        self.addCleanup(patched.stop)
        originals = []
        for relative, original in self.original_bytes.items():
            raw = original
            if relative == owner["original"]["path"]:
                changed = owner["body"].replace(before, after, 1).encode()
                raw = original[:owner["byte_start"]] + changed + original[owner["byte_end"]:]
            path = root / ".analysis/inputs/frontend/seiren-dsp-fixture" / Path(relative).name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            originals.append({"path": path.relative_to(root).as_posix(), "bytes": len(raw),
                              "sha256": hashlib.sha256(raw).hexdigest()})
        return families.audio_seiren_native_dsp_chain(host.HostLocalAudit(), originals)

    def test_real_eq_hpf_and_vocal_use_hid_with_exact_preset_profile_identities(self):
        chain = self.result["chain"]
        self.assertEqual([(row["preset_id"], row["profile_id"]) for row in chain["preset_profiles"]],
                         [(6, 1), (7, 2), (8, 3), (9, 4), (10, 5), (16, 6)])
        self.assertEqual(chain["hid_commands"]["getEQPreset"], [2, 8, 130])
        self.assertEqual(chain["hid_commands"]["setEQPreset"], [2, 8, 2])
        self.assertEqual(chain["hid_commands"]["getMicrophoneHPF"], [2, 8, 149])
        self.assertEqual(chain["hid_commands"]["setMicrophoneHPF"], [2, 8, 21])
        self.assertIn("HID via rzDevice", chain["physical_dsp_transport"])
        self.assertFalse(self.result["source_chain_complete"])
        self.assertFalse(self.result["target_execution"])
        self.assertEqual(self.result["official_dll_comparison"], "not_executed")

    def test_source_result_readback_and_release_boundaries_remain_distinct(self):
        setter = self.owner("setParametricEQ")["body"]
        calls = ["setParametricEQStatus", "setEQPreset", "this.setEQBand", "setEqVocalBass", "setEqVocalExciter", "setHighPassFilter"]
        self.assertEqual([setter.index(name) for name in calls], sorted(setter.index(name) for name in calls))
        self.assertNotIn("rzDevice", self.owner("initDataFromDevice")["body"])
        self.assertNotIn("getEQActiveBands", self.owner("getRuntimeData")["body"])
        self.assertIn('action:"hid.releaseMutex"', self.owner("hid_releaseHIDMutex")["body"])
        self.assertIn("getParametricEQSetting(0,0,t.jsonData.parametricEQStatuses[r])", self.owner("getEQActiveBands")["body"])
        self.assertIn("t=!0", self.owner("setHighPassFilter")["body"])
        self.assertNotIn(".status", self.owner("setHighPassFilter")["body"])

    def test_owner_slices_and_native_chunks_retain_actual_verified_inputs(self):
        for owner in self.audit.owners.values():
            raw = self.audit.read(owner["original"])[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(raw.decode(), owner["body"])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), owner["body_sha256"])
        for name in ("marie", "driver"):
            self.assertTrue(all(row["input_chunks_verified"] for row in self.result["monitor_driver"][name]["complete_functions"]))
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_hpf_setter_cannot_borrow_the_getter_command(self):
        with self.assertRaisesRegex(ValueError, "command selection changed"):
            self.analyze("adapter_setMicrophoneHPF", "R.d7", "R.b8")

    def test_signed_gain_and_big_endian_frequency_parsers_are_checked(self):
        for before, after in (("t.data[6]<<24>>24", "t.data[6]<<24>>>24"),
                              ("t.data[4]<<8|t.data[5]", "t.data[5]<<8|t.data[4]")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "parser_getParametricEQSetting"):
                self.analyze("parser_getParametricEQSetting", before, after)

    def test_source_hpf_failure_default_and_ignored_set_status_cannot_be_reinterpreted(self):
        with self.assertRaisesRegex(ValueError, "getHighPassFilter"):
            self.analyze("getHighPassFilter", "{isEnabled:!0,value:3}", "{isEnabled:!1,value:0}")
        with self.assertRaisesRegex(ValueError, "setHighPassFilter"):
            self.analyze("setHighPassFilter", "t=!0", 't="SUCCESS"===n.status')

    def test_gain_limits_storage_and_full_status_clear_are_not_silent_changes(self):
        for role, before, after in (("gain_conversion", "e<-12||e>12", "e<-15||e>15"),
            ("descriptor_ON_SET_PARAMETRIC_EQ_SETTINGS", "Storage_Global", "Storage_Profile"),
            ("setParametricEQ", "[0,0,0,0,0,0,0,0,0,0]", "[0,0,0,0,0,0,0,0,0,1]")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "changed"):
                self.analyze(role, before, after)

    def test_physical_monitor_arguments_include_all_channels_and_distinct_driver_directions(self):
        native = self.result["monitor_driver"]
        contract = native["contract"]
        self.assertEqual(contract["mute"], {"entity": 10, "request": 1, "selector": 1, "channels": [0], "payload_bytes": 1, "timeout": 100})
        self.assertEqual(contract["volume"], {"entity": 10, "request": 1, "selector": 2, "channels": [1, 2, 3], "payload_bytes": 2, "timeout": 100})
        self.assertEqual(contract["ioctl_set"], "0x80882081")
        self.assertEqual(contract["ioctl_get"], "0x80882086")
        for module, rva, changed in (("marie", "0x4e60e", "c644242001"), ("driver", "0x1837", "41b001"),
                                    ("driver", "0xa1ff", "ba86208880")):
            with self.subTest(module=module, rva=rva):
                altered = copy.deepcopy(native[module])
                row = next(row for function in altered["complete_functions"] for row in function["capture"]["assembly"] if row["rva"] == rva)
                row["bytes"] = changed
                with self.assertRaisesRegex(ValueError, "physical ABI changed"):
                    families.audio_seiren_monitor_contract(altered if module == "marie" else native["marie"],
                        altered if module == "driver" else native["driver"])

    def test_snapshot_change_prevents_bounded_observation_completion(self):
        import argparse
        audit = host.HostLocalAudit()
        with patch.object(host, "HostLocalAudit", return_value=audit), \
                patch.object(audit, "final_snapshot", return_value=["changed-source"]), \
                patch.object(families, "collect_audio_seiren_native_dsp", return_value={"source_chain_complete": False}), \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            with self.assertRaisesRegex(ValueError, "observation remains unresolved"):
                families.audit_audio_seiren_native_dsp(argparse.Namespace(product=[1422], prepare=False))
        self.assertFalse(publish.call_args.args[0]["bounded_observation_complete"])


class GamepadCurrentModeSocdTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not (analysis.ROOT / ".analysis/inputs/frontend/gamepad-4115-current/acquisition.json").is_file():
            raise unittest.SkipTest("Retained current 4115 inputs are unavailable")
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_gamepad_current_mode_socd(cls.audit)
        cls.owners = {owner["id"]: owner for owner in cls.result["complete_source_owners"]}

    def owner(self, role):
        return self.owners[self.result["roles"][role]]

    def test_cli_dispatch_reaches_the_bounded_4115_query(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command"))
        args = parser.parse_args(["audit-family-function", "--family", "gamepad", "--function", "mw",
                                  "--query", "gamepad-mode-socd", "--product", "4115"])
        with patch.object(families, "audit_gamepad_current_mode_socd", return_value="published") as query:
            self.assertEqual(families.audit_family_function(args), "published")
        query.assert_called_once_with(args)

    def test_current_selection_preserves_capture_limits_and_real_mounted_arcade_consumer(self):
        selected = self.result["source_sets"][1]
        self.assertEqual(selected["current_source_selection"]["canonical_entry_url"],
                         "https://apps.razer.com/synapse/products/4115/mw/")
        self.assertEqual(selected["acquisition"], selected["current_source_selection"]["acquisition"])
        self.assertNotEqual(selected["name"], "gamepad-mw-4115-current")
        self.assertIsNone(selected["capture"])
        self.assertIn("complete_capture_unavailable", selected["capture_failures"])
        self.assertEqual(self.result["mounted_consumers"]["socd"]["mounts"][0]["props"],
                         {"isArcadeController": "!0"})
        self.assertIn("TAB_CUSTOMIZE", self.result["exports"]["customize_tab_label"]["binding"]["body"])
        self.assertFalse(self.result["whole_page_complete"])
        self.assertEqual(self.result["actual_dll_comparison"], "unverified")

    def test_complete_owners_reproduce_current_original_byte_ranges(self):
        for owner in self.owners.values():
            raw = (analysis.ROOT / owner["original"]["path"]).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), owner["original"]["sha256"])
            body = raw[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(body.decode(), owner["body"])
            self.assertEqual(hashlib.sha256(body).hexdigest(), owner["body_sha256"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_protocol_uses_distinct_get_set_ids_and_no_invented_postwrite_read(self):
        self.assertEqual(self.result["protocol"], {
            "getControllerModeSwitch": [2, 12, 156], "setControllerModeSwitch": [2, 12, 28],
            "getControllerSOCDCleaningOption": [2, 12, 155], "setControllerSOCDCleaningOption": [2, 12, 27]})
        self.assertNotIn("getControllerModeSwitch", self.owner("mode_feature_setter")["body"])
        self.assertIn('"safe"===e.mode?0:1', self.owner("mode_feature_setter")["body"])

    def test_shape_review_accepts_lexical_rename_but_rejects_changed_mode_or_storage_semantics(self):
        import re
        owner = copy.deepcopy(self.owner("mode_feature_setter"))
        owner["body"] = re.sub(r"\be\b", "selection", owner["body"])
        families.verify_gamepad_mode_socd_owner(self.audit.parser, "mode_feature_setter", owner)
        for role, before, after in (
            ("mode_feature_setter", '"safe"===e.mode?0:1', '"safe"===e.mode?1:0'),
            ("mode_descriptor", "Storage_Global", "Storage_Profile"),
            ("socd_descriptor", "restoreDeviceData", "unusedRestoreDeviceData"),
            ("mode_ui", 't!==a.toLowerCase()', 't===a.toLowerCase()'),
            ("socd_class", "initFromDevice", "alwaysReadFromDevice"),
        ):
            with self.subTest(role=role):
                owner = copy.deepcopy(self.owner(role))
                self.assertIn(before, owner["body"])
                owner["body"] = owner["body"].replace(before, after)
                with self.assertRaisesRegex(ValueError, "requires semantic review"):
                    families.verify_gamepad_mode_socd_owner(self.audit.parser, role, owner)

    def test_orphan_loader_and_hardware_event_target_changes_cannot_reuse_the_old_conclusion(self):
        for role, before, after in (
            ("mode_loader", "58255", "99999"),
            ("socd_loader", "84470", "99999"),
            ("hardware_socd", "controllerDPadSOCD", "controllerModeSwitch"),
            ("slot_projection", "0===o", "2===o"),
            ("mode_decoder", "data[1]", "data[0]"),
            ("factory_58255", "n(77846)", "n(99999)"),
        ):
            with self.subTest(role=role):
                owner = copy.deepcopy(self.owner(role))
                self.assertIn(before, owner["body"])
                owner["body"] = owner["body"].replace(before, after)
                with self.assertRaisesRegex(ValueError, "requires semantic review"):
                    families.verify_gamepad_mode_socd_owner(self.audit.parser, role, owner)

    def test_false_execute_and_completed_log_remain_separate_from_success(self):
        owner = copy.deepcopy(self.owner("taskRunnerSetDevice"))
        self.assertIn('a.result=null!=o&&o', owner["body"])
        self.assertIn('a.value.status="completed"', owner["body"])
        owner["body"] = owner["body"].replace('a.result=null!=o&&o', 'a.result=!0')
        with self.assertRaisesRegex(ValueError, "requires semantic review"):
            families.verify_gamepad_mode_socd_owner(self.audit.parser, "taskRunnerSetDevice", owner)


class CurrentLightingFamilySourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        for name in ("gamepad-mw-4115-current", "synapse-lighting-engine-current"):
            if not (analysis.ROOT / ".analysis/inputs/frontend" / name / "acquisition.json").is_file():
                raise unittest.SkipTest("Retained lighting frontend inputs are unavailable")

    def test_palette_middleware_uses_current_selector_and_current_owner_bytes(self):
        result = families.gamepad_lighting_palette_sources()
        selected = next(row for row in result["source_sets"] if "current_source_selection" in row)
        self.assertEqual(selected["current_source_selection"]["canonical_entry_url"],
                         "https://apps.razer.com/synapse/products/4115/mw/")
        self.assertEqual(selected["acquisition"], selected["current_source_selection"]["acquisition"])
        acquired = json.loads((analysis.ROOT / selected["acquisition"]["path"]).read_bytes())
        current = {row["path"]: row for row in acquired["files"] if row.get("path")}
        owners = {row["id"]: row for row in result["complete_owners"]}
        for role in ("palette_task", "effect_task_preserves_palette", "profile_palette_refresh", "palette_command_dispatch"):
            for owner_id in result["roles"][role]:
                original = owners[owner_id]["original"]
                self.assertEqual({key: current[original["path"]][key] for key in ("path", "bytes", "sha256")}, original)

    def test_current_4115_selection_uses_its_own_ast_and_keeps_missing_capture(self):
        result = families.gamepad_lighting_selection_sources()
        selected = next(row for row in result["products"] if row["product_id"] == 4115)
        self.assertEqual(selected["current_source_selection"]["canonical_entry_url"],
                         "https://apps.razer.com/synapse/products/4115/mw/")
        self.assertTrue(selected["source_branch_selection_complete"])
        self.assertTrue(selected["capture_sources"])
        self.assertTrue(all(row["extraction"] == "complete_original_ast_factory" for row in selected["capture_sources"]))
        self.assertIn("capture_failures", selected)
        self.assertFalse(result["whole_page_complete"])
        self.assertEqual(result["official_dll_comparison"], "not_executed")


class LightingEngineInstanceSourceTests(unittest.TestCase):
    @staticmethod
    def fixture():
        prefix = ".analysis/inputs/frontend/engine/raw/apps.razer.com/synapse/lighting-engine/static/js/"
        originals = [{"path": prefix + name, "bytes": 100000, "sha256": str(index) * 64}
                     for index, name in enumerate(("main.current.js", "7665.1042e1fc.chunk.js", "5451.5593c8f6.chunk.js"), 1)]
        modules, classes = [], []
        for mid, role in families.GAMEPAD_LIGHTING_ENGINE_INSTANCE_MODULES.items():
            original = originals[1 if mid == 4921 else 2 if mid == 5451 else 0]
            modules.append({"module_id": mid, "roles": [role], "original": copy.deepcopy(original),
                            "byte_start": 10, "byte_end": 1000})
            if mid in (9396, 4001, 4921, 5451):
                classes.append({"module_id": mid, "original": copy.deepcopy(original),
                                "byte_start": 20, "byte_end": 999, "methods": ["constructor"]})
        manifests = []
        for edition in (0, 128, 129, 130, 131, 132, 133):
            name = f"DeviceManifest_4115_{edition}.json"
            manifests.append({"original": {"path": ".analysis/inputs/frontend/middleware/" + name,
                "url": "https://apps.razer.com/synapse/products/4115/mw/lighting-manifests/" + name,
                "bytes": 2000, "sha256": "a" * 64}, "complete_manifest": {"productId": 4115, "vendorId": 5426,
                    "rzDeviceConfig": {"claimInterface": 2, "maxRetryIn": 30, "maxRetryOut": 20,
                        "rzDeviceName": "rzDevice25SagePC", "sleepTimeBetweenIn": 5,
                        "sleepTimeBetweenOut": 5, "sleepTimeBetweenOutIn": 5}}})
        return {"complete_original_inputs": originals, "complete_modules": modules, "adapter_classes": classes}, manifests

    def test_complete_instance_requires_every_protocol_owner_and_keeps_manifest_references_only(self):
        self.assertEqual(set(families.GAMEPAD_LIGHTING_ENGINE_INSTANCE_MODULES),
                         {1653, 4001, 4611, 4921, 5451, 5954, 6411, 8199, 9396})
        self.assertEqual(families.GAMEPAD_LIGHTING_ENGINE_INSTANCE_SCRIPTS,
                         {4921: "7665.1042e1fc.chunk.js", 5451: "5451.5593c8f6.chunk.js"})
        engine, manifests = self.fixture()
        references = families.validate_gamepad_lighting_instance_sources(engine, manifests)
        self.assertEqual(references, [{key: row["original"][key] for key in ("path", "bytes", "sha256")}
                                      for row in manifests])
        self.assertTrue(all(set(row) == {"path", "bytes", "sha256"} for row in references))

    def test_missing_ambiguous_or_wrong_original_factories_and_incomplete_classes_are_rejected(self):
        for mode in ("missing_factory", "duplicate_factory", "wrong_role", "wrong_original", "wrong_script",
                     "missing_class", "duplicate_class", "class_original", "class_range", "class_constructor"):
            engine, manifests = self.fixture()
            module = next(row for row in engine["complete_modules"] if row["module_id"] == 5451)
            adapter = next(row for row in engine["adapter_classes"] if row["module_id"] == 5451)
            if mode == "missing_factory":
                engine["complete_modules"].remove(module)
            elif mode == "duplicate_factory":
                engine["complete_modules"].append(copy.deepcopy(module))
            elif mode == "wrong_role":
                module["roles"] = ["other_adapter"]
            elif mode == "wrong_original":
                module["original"]["sha256"] = "f" * 64
            elif mode == "wrong_script":
                for original in (engine["complete_original_inputs"][-1], module["original"], adapter["original"]):
                    original["path"] = original["path"].replace("5451.5593c8f6", "9158.unselected")
            elif mode == "missing_class":
                engine["adapter_classes"].remove(adapter)
            elif mode == "duplicate_class":
                engine["adapter_classes"].append(copy.deepcopy(adapter))
            elif mode == "class_original":
                adapter["original"]["sha256"] = "f" * 64
            elif mode == "class_range":
                adapter["byte_end"] = module["byte_end"] + 1
            else:
                adapter["methods"] = []
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                families.validate_gamepad_lighting_instance_sources(engine, manifests)

    def test_each_edition_and_its_actual_middleware_configuration_are_required(self):
        for mode in ("missing", "duplicate", "other_product_url", "other_edition", "product", "vendor", "timing", "retry"):
            engine, manifests = self.fixture()
            selected = manifests[-1]
            if mode == "missing":
                manifests.pop()
            elif mode == "duplicate":
                manifests.append(copy.deepcopy(selected))
            elif mode == "other_product_url":
                selected["original"]["url"] = selected["original"]["url"].replace("/4115/", "/2636/")
            elif mode == "other_edition":
                selected["original"]["url"] = selected["original"]["url"].replace("_133.json", "_134.json")
            elif mode == "product":
                selected["complete_manifest"]["productId"] = 2636
            elif mode == "vendor":
                selected["complete_manifest"]["vendorId"] = 0
            elif mode == "timing":
                selected["complete_manifest"]["rzDeviceConfig"]["sleepTimeBetweenOut"] = 10
            else:
                selected["complete_manifest"]["rzDeviceConfig"]["maxRetryIn"] = 10
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                families.validate_gamepad_lighting_instance_sources(engine, manifests)


class MatcherDeploymentComparisonTests(unittest.TestCase):
    def setUp(self):
        self.audit = host.HostLocalAudit()
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        root_patch = patch.object(analysis, "ROOT", self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)

    def original(self, name, raw):
        path = self.root / f".analysis/inputs/frontend/{name}/raw/apps.razer.com/synapse/products/190/mw/main.js"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest()}

    def compare(self, old_raw, new_raw, *, old_module=7, owner_body=None, roles=None):
        old = self.original("old-current", old_raw)
        new = self.original("new-snapshot", new_raw)
        factory = next(module for module in families.keyboard_source_modules(old_raw.decode())
                       if module["module_id"] == old_module)
        start, end = factory["byte_start"], factory["byte_end"]
        if owner_body is not None:
            start = old_raw.index(owner_body, start, end)
            end = start + len(owner_body)
        body = old_raw[start:end]
        request = {"product_id": 190, "layer": "mw", "roles": roles or ["monitor_factory"],
                   "baseline_references": ["baseline/owner"],
                    "owner": {"original": old, "byte_start": start,
                              "byte_end": end, "body_sha256": hashlib.sha256(body).hexdigest()}}
        source = {"product_id": 190, "layer": "mw", "originals": [new],
                  "source_references": {new["path"]: {"url": "https://apps.razer.com/synapse/products/190/mw/main.js",
                    "references": [{"role": "html_script_include", "from": "https://apps.razer.com/index.html",
                                    "literal": "main.js"}]}}}
        return families.matcher_owner_comparisons([request], source, self.audit), old, new

    def test_same_factory_bytes_at_new_utf8_offsets_are_the_only_unchanged_case(self):
        result, old, new = self.compare(b"const modules={7:(e,t,n)=>{t.value=1}};",
                                        'const unicode="设备";const modules={7:(e,t,n)=>{t.value=1}};'.encode())
        row = result["owner_comparisons"][0]
        self.assertEqual(row["comparison"], "exact_factory_bytes")
        left, right = (result["complete_owners"][row[key]] for key in ("old_owner", "new_owner"))
        self.assertNotEqual(left["byte_start"], right["byte_start"])
        self.assertEqual(left["body_sha256"], right["body_sha256"])
        for original, owner in ((old, left), (new, right)):
            raw = (self.root / original["path"]).read_bytes()[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(hashlib.sha256(raw).hexdigest(), owner["body_sha256"])
        self.assertEqual(result["unresolved"], [])

    def test_identical_module_number_and_renamed_variables_do_not_prove_semantics(self):
        result, _, _ = self.compare(b"const modules={7:(e,t,n)=>{t.value=1}};",
                                    b"const modules={7:(a,b,c)=>{b.value=1}};")
        row = result["owner_comparisons"][0]
        self.assertEqual(row["comparison"], "factory_changed")
        self.assertTrue(result["unresolved"])
        self.assertIn("first_different_relative_byte", row["difference"])

    def test_exact_child_bytes_do_not_promote_a_changed_factory_to_unchanged(self):
        child = b"()=>{return 3}"
        result, _, _ = self.compare(b"const modules={7:(e,t,n)=>{t.run=()=>{return 3};t.flag=1}};",
                                    b"const modules={7:(e,t,n)=>{t.flag=2;t.run=()=>{return 3}}};",
                                    owner_body=child)
        row = result["owner_comparisons"][0]
        self.assertEqual(row["comparison"], "factory_changed")
        self.assertTrue(result["unresolved"])
        owner = result["complete_owners"][row["exact_role_bytes_in_changed_factory"]]
        self.assertEqual(owner["syntax"], "arrow_function")
        self.assertEqual(owner["body_sha256"], hashlib.sha256(child).hexdigest())

    @staticmethod
    def state_machine_factory(module_id=7):
        return str(module_id).encode() + (
            b':(e,t,n)=>{const matcher=e=>({initial:"idle",on:{'
            b'ON_DPI_CALIBRATION_GET_CURRENT_STATE:{actions:()=>{}},'
            b'ON_DPI_CALIBRATION_RESET:{target:"dpiMatcher.idle"},'
            b'ON_UPDATE_SENSITIVITY_PROFILE:{actions:[]}},states:{'
            b'idle:{entry:()=>release()},requestingLock:{invoke:{id:"getDpiMatcherLocks",src:()=>locks()}},'
            b'calibrating:{on:{ON_DPI_CALIBRATION_CANCEL:{target:"idle"}}},'
            b'confirming:{entry:()=>release()},error:{entry:()=>release()}}})}')

    def test_relocated_state_machine_keeps_a_changed_factory_and_actual_ast_candidate(self):
        result, _, new = self.compare(b"const modules={" + self.state_machine_factory() + b"};",
            b"const modules={" + self.state_machine_factory(8) + b"};", roles=["matcher_state_machine"])
        row = result["owner_comparisons"][0]
        self.assertEqual(row["candidate_locator"], "complete_factory_with_matcher_state_machine_AST")
        self.assertEqual(row["comparison"], "factory_changed")
        self.assertTrue(result["unresolved"])
        owner = result["complete_factories"][row["new_owner"]]
        self.assertEqual(owner["original"], new)
        self.assertEqual(owner["module_id"], 8)
        self.assertIn("exact_role_bytes_in_changed_factory", row)

    def test_state_machine_locator_ignores_same_id_decoys_strings_and_split_nested_fields(self):
        decoy = (b'7:(e,t,n)=>{const text="ON_DPI_CALIBRATION_CANCEL getDpiMatcherLocks";'
                 b'const partial=e=>({initial:"idle",nested:{on:{ON_DPI_CALIBRATION_RESET:{}},'
                 b'states:{idle:{},requestingLock:{},calibrating:{},confirming:{},error:{}}}})}')
        result, _, _ = self.compare(b"const modules={" + self.state_machine_factory() + b"};",
            b"const modules={" + decoy + b"," + self.state_machine_factory(8) + b"};",
            roles=["matcher_state_machine"])
        row = result["owner_comparisons"][0]
        self.assertEqual(result["complete_factories"][row["new_owner"]]["module_id"], 8)
        self.assertEqual(row["comparison"], "factory_changed")

    def test_state_machine_locator_keeps_multiple_complete_owners_ambiguous(self):
        for current in (self.state_machine_factory(7) + b"," + self.state_machine_factory(8),
                        self.state_machine_factory(8) + b"," + self.state_machine_factory(9)):
            result, _, _ = self.compare(b"const modules={" + self.state_machine_factory() + b"};",
                b"const modules={" + current + b"};", roles=["matcher_state_machine"])
            row = result["owner_comparisons"][0]
            self.assertEqual(row["comparison"], "ambiguous_factory_candidates")
            self.assertEqual(len(row["candidate_factories"]), 2)
            self.assertNotIn("new_owner", row)

    def test_state_machine_locator_rejects_partial_changed_guards_spreads_and_duplicate_fields(self):
        old = b"const modules={" + self.state_machine_factory() + b"};"
        for changed in (
            self.state_machine_factory(8).replace(b'getDpiMatcherLocks', b'getOtherLocks'),
            self.state_machine_factory(8).replace(b'ON_DPI_CALIBRATION_CANCEL:{target:"idle"}',
                                                 b'ON_DPI_CALIBRATION_CANCEL:{target:"error"}'),
            self.state_machine_factory(8).replace(b'initial:"idle",', b'initial:"idle",...dynamic,'),
            self.state_machine_factory(8).replace(b'initial:"idle",', b'initial:"idle",initial:"error",'),
            self.state_machine_factory(8).replace(b'ON_UPDATE_SENSITIVITY_PROFILE:{actions:[]}', b'OTHER:{actions:[]}'),
        ):
            result, _, _ = self.compare(old, b"const modules={" + changed + b"};", roles=["matcher_state_machine"])
            row = result["owner_comparisons"][0]
            self.assertEqual(row["comparison"], "module_missing")
            self.assertFalse(row["candidate_factories"])
        with self.assertRaisesRegex(ValueError, "baseline state machine"):
            self.compare(old.replace(b'getDpiMatcherLocks', b'getOtherLocks'), old,
                         roles=["matcher_state_machine"])

    def test_cli_routes_explicit_repeatable_acquisitions_to_matcher_review(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command"))
        args = parser.parse_args(["audit-family-function", "--family", "mouse", "--function", "main",
            "--query", "mouse-matcher-deployment", "--product", "190", "--acquisition", "first.json",
            "--acquisition", "second.json"])
        self.assertEqual(args.acquisition, ["first.json", "second.json"])
        with patch.object(families, "audit_mouse_matcher_deployment", return_value="selected") as run:
            self.assertEqual(families.audit_family_function(args), "selected")
            run.assert_called_once_with(args)

    def ui_name_fixture(self):
        raw = (b'const modules={1867:(e,t,n)=>{n.d(t,{KX:()=>key,K2:()=>normalize,qv:()=>unique,$B:()=>valid});'
               b'const key=e=>e?typeof e==="object"?e.en||Object.values(e)[0]||"":e:"",'
               b'normalize=e=>e.map(e=>e),unique=(e,t)=>t,valid=(e,t)=>!e.some(e=>e.name===t)}};'
               b'function require(id){return modules[id]}const helper=require(1867);'
               b'function none(e){return e&&"object"===typeof e?e.en||"":e||""}'
               b'function sort(e){e.sort((a,b)=>helper.KX(a.name)<helper.KX(b.name)?-1:0)}'
               b'const reducer=(e,E)=>{switch(E.type){case SELECT:{const p=structuredClone(e.sensitivityMatcher.profiles);'
               b'p.filter(e=>"None"!==none(e.name));sort(p);helper.K2(p);helper.qv(p,E.payload);'
               b'channel.postMessage({type:"ON_UPDATE_SENSITIVITY_PROFILE"});return p}}};'
               b'const SELECT="SET_SELECTED_SENSITIVITY_MATCHER_PROFILE";'
               b'class Profiles{constructor(){this.changeProfile=e=>this.props.selectedSensitivityMatcher;'
               b'this.setProfileName=e=>helper.$B(this.props.sensitivityMatcher.profiles,e);'
               b'this.componentDidUpdate=e=>e}computeDataSet(){return this.props.sensitivityMatcher.profiles}'
               b'componentDidMount(){}componentWillUnmount(){}}'
               b'class Calibration{constructor(){this.openSensitivityCalibration=e=>e}}'
               b'const stageReducer=function(e,E){return{stages:JSON.parse(JSON.stringify(E.payload.stages)),action:"ON_STAGES_CHANGE"}};')
        original = self.original("ui-names", raw)
        return raw, {"product_id": 190, "layer": "ui", "originals": [original]}

    def test_ui_names_retains_complete_consumers_and_actual_helper_export_bindings(self):
        raw, source = self.ui_name_fixture()
        result = families.matcher_ui_name_consumers(source, self.audit)
        self.assertEqual(set(result["imported_helpers"]), {"1867.KX", "1867.K2", "1867.qv", "1867.$B"})
        self.assertEqual(set(result["local_helpers"]), {"sort", "none"})
        self.assertIn("SELECT", result["reducer_event_keys"])
        self.assertFalse(result["semantic_equivalence_claim"])
        for role, identifier in result["owners"].items():
            owner = self.audit.owners[identifier]
            self.assertEqual(hashlib.sha256(raw[owner["byte_start"]:owner["byte_end"]]).hexdigest(), owner["body_sha256"], role)
        for helper in result["imported_helpers"].values():
            self.assertEqual(helper["namespace_binding"]["body"], "helper=require(1867)")
            self.assertIn(helper["binding"]["owner_id"], self.audit.owners)

    def test_ui_names_does_not_borrow_missing_or_ambiguous_mounted_owners(self):
        raw, source = self.ui_name_fixture()
        for changed in (raw.replace(b'this.openSensitivityCalibration=', b'this.other='),
                        raw + b'class Another{constructor(){this.openSensitivityCalibration=e=>e}}'):
            with self.subTest(changed=changed[-80:]):
                self.audit = host.HostLocalAudit()
                original = self.original("changed-ui-names", changed)
                with self.assertRaisesRegex(ValueError, "missing or ambiguous"):
                    families.matcher_ui_name_consumers({**source, "originals": [original]}, self.audit)

    def test_ui_names_rejects_dynamic_helper_imports_and_wrong_product_scope(self):
        raw, source = self.ui_name_fixture()
        original = self.original("dynamic-ui-helper", raw.replace(b'helper=require(1867)', b'helper=runtime'))
        with self.assertRaisesRegex(ValueError, "literal webpack request"):
            families.matcher_ui_name_consumers({**source, "originals": [original]}, self.audit)
        for change in ({"product_id": 226}, {"layer": "mw"}):
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "PID190 UI source"):
                families.matcher_ui_name_consumers({**source, **change}, self.audit)

    def test_cli_routes_immutable_ui_name_review_without_changing_deployment_query(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command"))
        args = parser.parse_args(["audit-family-function", "--family", "mouse", "--function", "main",
            "--query", "mouse-matcher-ui-names", "--product", "190", "--acquisition", "old.json",
            "--acquisition", "new.json"])
        with patch.object(families, "audit_mouse_matcher_ui_names", return_value="selected") as run:
            self.assertEqual(families.audit_family_function(args), "selected")
            run.assert_called_once_with(args)
        args.product = [226]
        with self.assertRaisesRegex(ValueError, "currently covers PID190"):
            families.audit_mouse_matcher_ui_names(args)

    def test_relocated_matcher_id_is_a_changed_candidate_and_retains_real_helper_reexports(self):
        old = b'const modules={7:(e,t,n)=>{const maxTravelDistance=10;const CONFIRM=()=>1}};'
        new = (b'const modules={8:(e,t,n)=>{const maxTravelDistance=10;const CONFIRM=()=>n(11).Sl("{}")},'
               b'11:(e,t,n)=>{n.d(t,{Sl:()=>o.Sl});const o=n(12)},'
               b'12:(e,t,n)=>{const parse=e=>JSON.parse(e);n.d(t,{Sl:()=>parse})}};')
        result, _, _ = self.compare(old, new)
        row = result["owner_comparisons"][0]
        self.assertEqual(row["comparison"], "factory_changed")
        self.assertEqual(row["candidate_locator"], "complete_factory_with_maxTravelDistance_and_CONFIRM")
        self.assertEqual(result["complete_factories"][row["new_owner"]]["module_id"], 8)
        modules = {owner["module_id"] for owner in result["complete_factories"].values()}
        self.assertTrue({7, 8, 11, 12} <= modules)
        self.assertEqual({edge["module_id"] for edge in result["new_direct_dependency_edges"]}, {11, 12})

    def test_missing_and_ambiguous_modules_remain_unresolved(self):
        old = b"const modules={7:(e,t,n)=>{t.value=1}};"
        for new, status in ((b"const modules={8:(e,t,n)=>{t.value=1}};", "module_missing"),
                            (b"const a={7:(e,t,n)=>{t.value=1}},b={7:(e,t,n)=>{t.value=2}};",
                             "ambiguous_factory_candidates")):
            with self.subTest(status=status):
                result, _, _ = self.compare(old, new)
                self.assertEqual(result["owner_comparisons"][0]["comparison"], status)
                self.assertTrue(result["unresolved"])
                self.audit = host.HostLocalAudit()

    def test_truncated_owner_ranges_are_rejected_as_incomplete_ast(self):
        original = self.original("old-current", b"const modules={7:(e,t,n)=>{t.value=1}};")
        raw = self.audit.read(original)
        module = families.keyboard_source_modules(raw.decode())[0]
        start, end = module["byte_start"], module["byte_end"] - 1
        request = {"product_id": 190, "layer": "mw", "roles": ["monitor"], "baseline_references": [],
                   "owner": {"original": original, "byte_start": start, "byte_end": end,
                             "body_sha256": hashlib.sha256(raw[start:end]).hexdigest()}}
        with self.assertRaisesRegex(ValueError, "complete AST owner"):
            families.matcher_owner_comparisons([request], {"product_id": 190, "layer": "mw", "originals": []}, self.audit)

    def test_new_source_requires_both_index_identity_and_actual_root_reference(self):
        directory = self.root / ".analysis/inputs/frontend/new-snapshot"
        main = self.original("new-snapshot", b"const modules={7:(e,t,n)=>{t.value=1}};")
        entry_url = "https://apps.razer.com/synapse/products/190/mw/index.html"
        entry = directory / "entry.html"
        entry.write_bytes(b'<html><script src="main.js"></script></html>')
        root = {"path": entry.relative_to(self.root).as_posix(), "bytes": entry.stat().st_size,
                "sha256": hashlib.sha256(entry.read_bytes()).hexdigest()}
        acquired = {"entry_url": entry_url, "target_execution": False, "source_current_during_acquisition": True,
                    "files": [{**root, "url": entry_url}, {**main, "url": entry_url.replace("index.html", "main.js")}],
                    "stability": [{"url": entry_url, "before_sha256": root["sha256"],
                                   "after_sha256": root["sha256"], "unchanged": True}]}
        acquisition = directory / "acquisition.json"
        acquisition.write_text(json.dumps(acquired), encoding="utf-8")
        name = acquisition.relative_to(self.root).as_posix()
        indexed = {row["path"]: row for row in (root, main)}
        indexed[name] = {"path": name, "bytes": acquisition.stat().st_size,
                         "sha256": hashlib.sha256(acquisition.read_bytes()).hexdigest()}
        result = families.matcher_deployment_source(name, self.audit, indexed)
        self.assertEqual(result["product_id"], 190)
        self.assertEqual(result["layer"], "mw")
        self.assertEqual(result["originals"], [main])
        self.assertEqual(result["source_references"][main["path"]]["references"][0]["role"], "html_script_include")
        self.assertFalse(result["dependency_closure_verified"])
        indexed[name]["sha256"] = "incorrect identity"
        with self.assertRaisesRegex(ValueError, "acquisition is not indexed"):
            families.matcher_deployment_source(name, self.audit, indexed)


class FrontendReferenceGapTests(unittest.TestCase):
    def test_bounded_observation_url_edges_keep_their_declared_missing_originals(self):
        url = "https://apps.razer.com/catalog.json"
        edge = {"from": "https://apps.razer.com/main.js", "url": url, "role": "explicit source URL"}
        result = families.frontend_reference_gaps({"files": [{"url": url}], "edges": [edge]})
        self.assertEqual(result["declared_non_map_missing"], [{"url": url, "declared_by": [edge]}])
        self.assertEqual(result["unattributed_non_map_missing"], [])
        self.assertFalse(result["all_declared_non_map_references_retained"])

    def test_mixed_manifest_and_observation_edges_preserve_debug_map_boundaries(self):
        result = families.frontend_reference_gaps({
            "files": [{"url": "https://apps.razer.com/catalog.json", "path": "retained.json"},
                      {"url": "https://apps.razer.com/main.js.map"}],
            "edges": [{"url": "https://apps.razer.com/catalog.json"},
                      {"to": "https://apps.razer.com/main.js.map"}],
            "unresolved": [{"kind": "dependency_closure_outside_observation"}]})
        self.assertEqual(result["declared_source_maps_missing"], 1)
        self.assertTrue(result["all_declared_non_map_references_retained"])
        self.assertEqual(result["unresolved_by_kind"], {"dependency_closure_outside_observation": 1})

    def test_ambiguous_or_missing_edge_targets_are_rejected(self):
        for edge in ({}, {"url": None}, {"to": ""}, {"to": "first", "url": "second"}):
            with self.subTest(edge=edge), self.assertRaisesRegex(ValueError, "unique target URL"):
                families.frontend_reference_gaps({"files": [], "edges": [edge]})


class PromiseParameterBindingTests(unittest.TestCase):
    def setUp(self):
        self.parser = host.HostLocalAudit().parser

    def test_executor_shadow_does_not_consume_the_outer_wire_argument(self):
        source = (b'class Transport { write(output,input,bytes,silent,guid,noData) {'
            b'return new Promise(async(noData,reject)=>{output.write(bytes,silent);'
            b'input.on("read",()=>{console.log(guid);noData(true)});});}}')
        result = families.javascript_promise_parameter_uses(source, self.parser, 'write')
        self.assertEqual([item['name'] for item in result['method_parameters']],
                         ['output', 'input', 'bytes', 'silent', 'guid', 'noData'])
        no_data = result['method_parameters'][5]
        self.assertTrue(no_data['shadowed_by_executor_parameter'])
        self.assertEqual(no_data['references_in_executor'], [])
        self.assertTrue(result['executor_parameters'][0]['shadowed_outer_name_references'])
        self.assertTrue(all(item['references_in_executor'] for item in result['method_parameters'][:5]))

    def test_renamed_executor_preserves_actual_outer_argument_references(self):
        source = (b'class Transport { write(payload,noData) {'
            b'return new Promise((resolve,reject)=>{resolve(noData?[]:payload)});}}')
        result = families.javascript_promise_parameter_uses(source, self.parser, 'write')
        self.assertTrue(result['method_parameters'][1]['references_in_executor'])
        self.assertFalse(result['method_parameters'][1]['shadowed_by_executor_parameter'])

    def test_nested_same_name_parameter_is_a_separate_scope(self):
        source = (b'class Transport { write(payload) {'
            b'return new Promise((resolve,reject)=>{const later=(payload)=>resolve(payload);'
            b'later(7);});}}')
        result = families.javascript_promise_parameter_uses(source, self.parser, 'write')
        self.assertEqual(result['method_parameters'][0]['references_in_executor'], [])

    def test_indirect_executor_duplicate_method_and_default_parameters_require_review(self):
        for source in (
            b'class Transport { write(payload){return new Promise(executor)}}',
            b'class Transport { write(payload){return new Promise((resolve)=>resolve(payload))}'
            b'write(other){return new Promise((resolve)=>resolve(other))}}',
            b'class Transport { write(payload=0){return new Promise((resolve)=>resolve(payload))}}',
            b'class Transport { write(payload){const other=1;return new Promise((resolve)=>resolve(payload))}}',
        ):
            with self.subTest(source=source), self.assertRaises(ValueError):
                families.javascript_promise_parameter_uses(source, self.parser, 'write')


class CameraImagePropertyChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not (analysis.ROOT / ".analysis/inputs/frontend/accessories-3595-current-refresh-ec3db4b29c5a/acquisition.json").is_file():
            raise unittest.SkipTest("Retained refreshed V2/X camera inputs are unavailable")
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_camera_image_properties(cls.audit)
        cls.products = {row["product_id"]: row for row in cls.result["products"]}

    def owner(self, role, pid=3595):
        return self.audit.owners[self.products[pid]["roles"][role]]

    def analyze(self, role, before, after):
        owner = self.owner(role)
        self.assertIn(before, owner["body"])
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", root)
        patched.start()
        self.addCleanup(patched.stop)
        sources = {row["original"]["path"]: row["original"] for row in self.audit.owners.values()
                   if "/3595/" in row["original"]["path"]}
        layers = {"ui": [], "mw": []}
        for relative, source in sources.items():
            raw = self.audit.read(source)
            if relative == owner["original"]["path"]:
                changed = owner["body"].replace(before, after, 1).encode()
                raw = raw[:owner["byte_start"]] + changed + raw[owner["byte_end"]:]
            layer = "ui" if "/3595/ui/" in relative else "mw"
            path = root / ".analysis/inputs/frontend/camera-image-fixture" / layer / Path(relative).name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            layers[layer].append({"path": path.relative_to(root).as_posix(), "bytes": len(raw),
                                  "sha256": hashlib.sha256(raw).hexdigest()})
        return families.camera_image_property_chain(host.HostLocalAudit(), 3595, layers["ui"], layers["mw"])

    def test_current_mount_and_product_differences_use_refreshed_inputs(self):
        self.assertEqual(self.products[3595]["white_balance_range"], {"min": 2800, "max": 7500})
        self.assertEqual(self.products[3596]["white_balance_range"], {"min": 2800, "max": 6500})
        self.assertTrue(self.products[3595]["preview_off_editing"])
        self.assertFalse(self.products[3596]["preview_off_editing"])
        for pid, row in self.products.items():
            self.assertEqual(row["event"], "ON_SET_IMAGE_PRESETS")
            self.assertEqual(row["mounted_props"]["supportSharpness"], "!1")
            self.assertEqual(row["mounted_props"]["supportGain"], "!1")
            for selected in row["source_sets"].values():
                self.assertIn("refresh", selected["name"])
                self.assertIsNone(selected["capture"])
                self.assertIn("complete_capture_unavailable", selected["capture_failures"])
            self.assertIn("refresh", self.owner("image_settings", pid)["original"]["path"])
        self.assertFalse(self.result["source_chain_complete"])
        self.assertFalse(self.result["whole_page_complete"])
        self.assertFalse(self.result["target_execution"])
        self.assertEqual(self.result["official_dll_comparison"], "not_executed")

    def test_real_preset_runner_result_and_lifecycle_are_separate(self):
        for pid, row in self.products.items():
            self.assertEqual(row["write_order"], ["brightness", "contrast", "saturation", "whiteBalance", "sharpness", "gain"])
            self.assertIn('currentCameraSettings:E.payload', self.owner("cache_response_reducer", pid)["body"])
            self.assertIn("getAdvancedCameraSettingsCache", self.owner("ui_cache_request", pid)["body"])
            self.assertNotIn("getWhiteBalance", self.owner("image_result_consumer", pid)["body"])
            self.assertIn('return{}', self.owner("adapter_setSaturation", pid)["body"])
            self.assertIn('this.terminate()', self.owner("adapter_destroy", pid)["body"])
            self.assertNotIn('await', self.owner("adapter_destroy", pid)["body"])
        self.assertEqual(self.result["native_contract"]["properties"],
                         {"brightness": 0, "contrast": 1, "saturation": 3, "whiteBalance": 7})
        self.assertEqual(self.result["native_contract"]["release_vtable_offset"], 16)

    def test_complete_original_slices_native_chunks_and_json_roundtrip(self):
        for owner in self.audit.owners.values():
            body = self.audit.read(owner["original"])[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(body.decode(), owner["body"])
            self.assertEqual(hashlib.sha256(body).hexdigest(), owner["body_sha256"])
        self.assertTrue(all(row["input_chunks_verified"] for row in self.result["native"]["complete_functions"]))
        complete = {**self.result, "complete_owners": list(self.audit.owners.values()),
                    "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_hidden_controls_and_cross_product_white_balance_range_are_rejected(self):
        for role, before, after, error in (("mounted_pages", "supportSharpness:!1", "supportSharpness:!0", "mounted props"),
            ("device_info", "max:7500", "max:6500", "white-balance range")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, error):
                self.analyze(role, before, after)

    def test_preset_write_order_and_early_cancel_contract_are_checked(self):
        for before, after, error in (('["brightness","contrast","saturation","whiteBalance","sharpness","gain"]',
            '["contrast","brightness","saturation","whiteBalance","sharpness","gain"]', "completion and cancellation"),
            (".signal.aborted", ".signal.cancelled", "completion and cancellation")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, error):
                self.analyze("image_runner", before, after)

    def test_ignored_false_ack_and_log_only_consumer_cannot_be_claimed_as_readback(self):
        with self.assertRaisesRegex(ValueError, "ignored setter acknowledgements"):
            self.analyze("image_runner", 'yield t.setBrightness(i[n],!1)',
                'if(!(yield t.setBrightness(i[n],!1)))throw new Error("failed")')
        with self.assertRaisesRegex(ValueError, "log-only result consumer"):
            self.analyze("image_result_consumer", "console.log", "rzDevice.getWhiteBalance")

    def test_empty_native_pointer_parser_is_not_reinterpreted_as_array(self):
        with self.assertRaisesRegex(ValueError, "getter parse boundary getWhiteBalance"):
            self.analyze("adapter_getWhiteBalance", "?{}:", "?[]:")

    def test_native_property_ids_flags_hresult_and_release_are_byte_bound(self):
        native = self.result["native"]
        for rva, changed in (("0x5a898", "ba06000000"), ("0x4bd4e", "4183f102"), ("0x4be39", "ff5020")):
            with self.subTest(rva=rva):
                altered = copy.deepcopy(native)
                instruction = next(row for function in altered["complete_functions"] for row in function["capture"]["assembly"]
                                   if row["rva"] == rva)
                instruction["bytes"] = changed
                with self.assertRaisesRegex(ValueError, "native property ABI changed"):
                    families.camera_image_native_contract(altered)
        altered = copy.deepcopy(native)
        altered["verified_data_regions"][0]["raw_hex"] = "00" * 16
        with self.assertRaisesRegex(ValueError, "VideoProcAmp IID changed"):
            families.camera_image_native_contract(altered)
        altered = copy.deepcopy(native)
        setter = next(row["capture"] for row in altered["complete_functions"] if row["capture"]["rva"] == "0x4bce0")
        line = next(row for row in setter["pseudocode"]["lines"] if "v9 >= 0" in row["text"])
        line["text"] = line["text"].replace("v9 >= 0", "v9 == 0")
        with self.assertRaisesRegex(ValueError, "HRESULT/result boundary changed"):
            families.camera_image_native_contract(altered)

    def test_changed_snapshot_cannot_publish_success(self):
        import argparse
        audit = host.HostLocalAudit()
        with patch.object(host, "HostLocalAudit", return_value=audit), \
                patch.object(audit, "final_snapshot", return_value=["changed-source"]), \
                patch.object(families, "collect_camera_image_properties", return_value={"source_chain_complete": False}), \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            with self.assertRaisesRegex(ValueError, "observation remains unresolved"):
                families.audit_camera_image_properties(argparse.Namespace(family="accessories", function="main", product=[3595, 3596], prepare=False))
        self.assertFalse(publish.call_args.args[0]["bounded_observation_complete"])
        self.assertEqual(publish.call_args.args[3], "image_properties")


class CameraImageHostChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        receipt = analysis.ROOT / ".analysis/results/frontend/accessories-camera-local-pages-audit.json"
        if not receipt.is_file():
            raise unittest.SkipTest("Retained camera IMAGE host dependency is unavailable")
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_camera_image_host_chain(cls.audit)

    def source_contract(self, role, before, after):
        owners = {key: dict(self.audit.owners[value]) for key, value in self.result["roles"].items()}
        self.assertIn(before, owners[role]["body"])
        owners[role]["body"] = owners[role]["body"].replace(before, after, 1)
        return families.camera_image_host_source_contract(owners)

    def test_actual_current_host_path_is_main_process_and_completion_remains_bounded(self):
        result = self.result
        self.assertEqual(result["host_release"]["version"], "4.0.896")
        self.assertEqual([row["product_id"] for row in result["products"]], [3595, 3596])
        self.assertEqual(result["kind"], "current_camera_image_host_chain")
        self.assertEqual(result["upstream_image"]["section"], "image_properties")
        self.assertIn("FFIPreloadMain.callDLLAsync", result["source_contract"]["route"][-2])
        self.assertNotIn("FFIProcess", " ".join(result["source_contract"]["route"]))
        self.assertTrue(result["selected_host_semantics_reviewed"])
        for key in ("source_chain_complete", "whole_page_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(result[key])
        self.assertEqual(result["official_dll_comparison"], "not_executed")
        self.assertEqual(result["runtime_acceptance"], "unverified")
        self.assertEqual(result["temporary_artifacts_created"], [])
        self.assertTrue(all(row["member_offset_verified"] for row in result["host_release"]["members"]))

    def test_output_receipt_is_a_section_dependency_not_an_immutable_artifact(self):
        dependency = self.result["upstream_image"]
        self.assertNotIn(dependency["receipt"], self.audit.artifacts)
        upstream = json.loads((analysis.ROOT / dependency["receipt"]).read_bytes())[dependency["section"]]
        digest = hashlib.sha256(json.dumps(upstream, ensure_ascii=False, sort_keys=True,
            separators=(",", ":")).encode()).hexdigest()
        self.assertEqual(dependency["section"], "image_properties")
        self.assertEqual(dependency["canonical_sha256"], digest)

    def test_bridge_cannot_silently_switch_to_other_ffi_owner(self):
        for role, before, after in (
            ("preload_async", 'e.invoke("ffiPreloadAsync",t,i)', 'e.invoke("ffiSubPreloadAsync",t,i)'),
            ("host_async_dispatch", "eo.callDLLAsync(e,o,n)", "oo.callDLLAsync(e,o,n)"),
            ("mw_3595_async_export", "r.doDLLMainActionAsync(e,t)", "r.doDLLSubProcessActionAsync(e,t)"),
            ("mw_3596_electron_bridge", "window.top", "window")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "host source contract changed"):
                self.source_contract(role, before, after)

    def test_failed_native_initialization_is_not_promoted_to_device_success(self):
        source, native = self.result["source_contract"]["initialization"], self.result["native_contract"]["initialization"]
        self.assertTrue(source["isInit_set_before_device_success"])
        self.assertFalse(source["device_initialize_success_required_before_set_event"])
        self.assertEqual((source["wrapper_attempts"], source["retry_delay_ms"]), (10, 100))
        self.assertEqual((native["attempts_per_export"], native["retry_delay_ms"]), (30, 1000))
        self.assertTrue(native["container_lookup_reuses_existing_owner"])
        self.assertEqual(native["selected_product_classes"], {"3595": "CRzDeviceAccess", "3596": "CRzDeviceAccess_0E0C"})
        with self.assertRaisesRegex(ValueError, "host source contract changed"):
            self.source_contract("mw_3595_adapter_init", "this.isInit=!0", "this.isInit=!!deviceInitialized")

    def test_pointer_copy_failure_error_argument_and_finally_remain_explicit(self):
        result = self.result["source_contract"]["result"]
        self.assertFalse(result["copy_failure_still_frees_pointer"])
        self.assertFalse(result["ffi_error_parameter_checked"])
        self.assertFalse(result["exceptions_rethrown"])
        self.assertFalse(result["setter_readback_added"])
        for before, after in (("o=s.readCString(),r=!0", "r=!0,o=s.readCString()"),
                              ("(e,a)=>{n(a)}", "(e,a)=>{e?reject(e):n(a)}"),
                              ("e.FreeMalloc(s)", "e.FreeMalloc(o)")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "host source contract changed"):
                self.source_contract("host_invoke", before, after)

    def test_native_callback_is_borrowed_and_not_a_property_confirmation(self):
        callback = self.result["native_contract"]["callback"]
        self.assertEqual(callback["global_pointer_rva"], "0xd5f78")
        self.assertEqual(callback["event"], "displayCameraPropertiesDialogClosed")
        self.assertEqual(callback["payload_fields"], ["event", "containerId"])
        self.assertFalse(callback["event_confirms_image_write"])
        self.assertFalse(callback["pointer_cleared_by_device_terminate"])
        for role, before, after in (
            ("host_callback", "a.urlList.includes(e.getURL())", "true"),
            ("mw_3596_adapter_handleEvent", "this.containerId===(null==t?void 0:t.containerId)&&", "true&&"),
            ("preload_response", "e.removeListener(t,n)", "e.removeAllListeners(t)")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "host source contract changed"):
                self.source_contract(role, before, after)

    def test_release_acknowledgement_is_not_session_quiescence(self):
        ownership = self.result["source_contract"]["ownership"]
        self.assertTrue(ownership["mutex_released_in_finally"])
        self.assertTrue(ownership["free_ffi_branch_returns_true"])
        for key in ("free_ffi_releases_library_or_callback", "termination_unregisters_native_callback",
                    "unload_awaits_termination", "connection_generation_checked", "whole_session_quiescence_proven"):
            self.assertFalse(ownership[key])
        for role, before, after in (
            ("host_async", 'case"FreeFFI":return!0', 'case"FreeFFI":return this.ffiLibMap.delete(n)'),
            ("host_async", "finally{r&&r()}", "finally{}"),
            ("mw_3595_adapter_destroy", "this.terminate()", "await this.terminate()")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "host source contract changed|unload waiting"):
                self.source_contract(role, before, after)
        native = self.result["native_contract"]["release"]
        self.assertEqual(native["thread_join_timeout_ms"], "INFINITE")
        self.assertTrue(native["thread_handle_closed_after_join"])
        self.assertTrue(native["dialog_close_can_emit_event"])
        self.assertFalse(native["whole_callback_and_task_quiescence_proven"])

    def test_physical_native_callback_release_and_thread_join_are_byte_bound(self):
        for rva, changed in (("0x57500", "48890571ea0700"), ("0x4b720", "ff5018"),
                             ("0x69d7a", "33d2"), ("0x3e367", "ffd0")):
            native = copy.deepcopy(self.result["native"])
            instruction = next(row for function in native["complete_functions"]
                for row in function["capture"]["assembly"] if row["rva"] == rva)
            instruction["bytes"] = changed
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "host native ABI changed"):
                families.camera_image_host_native_contract(native)
        self.assertEqual(len(self.result["native_contract"]["byte_assertions"]), 18)

    def test_complete_owners_native_chunks_and_json_roundtrip_are_reproducible(self):
        for owner in self.audit.owners.values():
            node = host.verify_profiles_owner(self.audit, owner)
            self.assertEqual(node.text.decode(), owner["body"])
        self.assertEqual(len(self.result["native"]["complete_functions"]), 13)
        self.assertTrue(all(row["input_chunks_verified"] for row in self.result["native"]["complete_functions"]))
        self.assertTrue(self.result["native"]["all_selected_pseudocode_captured"])
        report = {**self.result, "complete_source_owners": list(self.audit.owners.values()),
                  "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertEqual(self.audit.final_snapshot(), [])


class CameraMediaPreviewChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_camera_media_preview_chain(cls.audit)
        cls.products = {row["product_id"]: row for row in cls.result["products"]}

    def body(self, pid, role):
        return self.audit.owners[self.products[pid]["roles"][role]]["body"]

    def mutate(self, pid, role, before, after):
        key = self.products[pid]["roles"][role]
        changed = dict(self.audit.owners[key])
        self.assertIn(before, changed["body"])
        changed["body"] = changed["body"].replace(before, after, 1)
        owners = {**self.audit.owners, key: changed}
        return families.camera_media_preview_source_contract(self.result["products"], owners)

    def test_refreshed_owner_identity_and_actual_product_props_are_separate(self):
        expected = {3595: ("62a03244bbf292947383bd287eed289ef32c23d924c8f0189aa7f8efb67d6560",
                (5725229, 5733337), (5715823, 5722590)),
            3596: ("082c24e92ca502c579c43d89b8c067878aee594e5030974f0e94e2cc7984ad8f",
                (5715203, 5723311), (5705888, 5712658))}
        for pid, (digest, preview, picker) in expected.items():
            product = self.products[pid]
            self.assertEqual(product["media_original"]["sha256"], digest)
            for role, bounds in (("preview", preview), ("source_picker", picker)):
                owner = self.audit.owners[product["roles"][role]]
                self.assertEqual((owner["byte_start"], owner["byte_end"]), bounds)
                self.assertEqual(owner["original"]["sha256"], digest)
            self.assertEqual(product["roles"]["mounted_preview"], product["roles"]["preview"])
        self.assertTrue(self.products[3595]["capability_probe_enabled_by_mounted_page"])
        self.assertFalse(self.products[3596]["capability_probe_enabled_by_mounted_page"])
        self.assertEqual(self.products[3596]["mounted_picker_props"]["checkUSBCapacity"], "!1")
        revalidation = self.result["deployment_revalidation"]
        self.assertEqual(revalidation["observation_count"], 32)
        self.assertFalse(revalidation["proves_complete_dependency_acquisition"])

    def test_actions_resolve_to_real_static_events_and_the_actual_reducer(self):
        expected = {"B_": "SET_IS_VIDEO_PLAYED", "ur": "SET_IS_VIDEO_LOADING", "kq": "SET_STOP_VIDEO_TRIGGER",
            "zy": "SET_HAS_VIDEO_RESTART", "F4": "REFRESH_CAMERA", "RI": "GET_CAMERA_HARDWARE_ID",
            "SO": "CAMERA_TURN_OFF_COMPLETED", "ku": "SET_IS_USED_BY_OTHER_APP",
            "Ty": "SET_RECONNECT_CAMERA", "WD": "SET_IS_PREVIEW_ENABLED", "$P": "SET_RESOLUTION",
            "Ag": "SET_CAMERA_DEVICE_LIST", "HG": "SWITCH_CAMERA_OUTPUT"}
        for product in self.products.values():
            actions = {row["export"]: row for row in product["actions"]}
            self.assertEqual(len(actions), 22)
            for exported, label in expected.items():
                self.assertEqual(actions[exported]["event"], label)
            for row in actions.values():
                self.assertEqual(json.loads(self.audit.owners[row["event_owner"]]["body"]), row["event"])
                self.assertEqual("reducer" in row, row["event"] != "CAMERA_TURN_OFF_COMPLETED")
            toggle = self.audit.owners[actions["WD"]["reducer"]]["body"]
            self.assertIn('type:"ON_SET_IS_PREVIEW"', toggle)
            self.assertIn('isPreview:E.payload', toggle)

    def test_lexical_references_and_all_complete_ast_boundaries_are_reproducible(self):
        for product in self.products.values():
            for row in product["bindings"]:
                if "reference_byte" not in row:
                    continue
                binding = row["binding"]
                current = self.audit.binding("media_preview_binding_check", row["original"],
                    binding["name"], row["reference_byte"])
                self.assertEqual(current["start_byte"], binding["start_byte"])
                self.assertEqual(current["end_byte"], binding["end_byte"])
        for owner in list(self.audit.owners.values()):
            self.assertEqual(host.verify_profiles_owner(self.audit, owner).text.decode(), owner["body"])

    def test_enabled_late_guard_is_not_generation_or_post_play_cancellation(self):
        value = self.result["source_contract"]["late_acquisition"]
        self.assertTrue(value["checks_enabled_ref"])
        for name in ("checks_connection_generation", "checks_mount_identity", "aborts_pending_get_user_media",
                     "checks_again_after_play_promise"):
            self.assertFalse(value[name])
        for pid in self.products:
            with self.subTest(pid=pid), self.assertRaisesRegex(ValueError, "late acquisition guard"):
                self.mutate(pid, "preview", 'p.current=o', 'p.current=o;const generation=0')

    def test_play_rejection_does_not_silently_become_a_track_release_claim(self):
        value = self.result["source_contract"]["play_failure"]
        self.assertFalse(value["stops_acquired_tracks"])
        self.assertFalse(value["clears_src_object"])
        for pid in self.products:
            with self.subTest(pid=pid), self.assertRaisesRegex(ValueError, "play failure release"):
                self.mutate(pid, "play_rejection", 'C(!1)', 'C(!1),h()')

    def test_probe_and_stopping_order_keep_their_actual_exception_boundaries(self):
        value = self.result["source_contract"]["capability_probe"]
        self.assertTrue(value["capabilities_read_precedes_stop"])
        self.assertFalse(value["finally_releases_tracks"])
        stop = self.result["source_contract"]["stop"]
        self.assertEqual(stop["notification_delay_ms"], 1000)
        self.assertTrue(stop["missing_video_returns_before_notification"])
        self.assertFalse(stop["track_ended_acknowledgement"])
        self.assertFalse(stop["pending_acquisition_drained"])
        for pid, role, old, changed in ((3595, "capability_probe", 'catch(_)', 'finally{}catch(_)'),
                (3596, "stop", '(1e3).then(', '(0).then('),
                (3595, "stop", 'e.srcObject=null,e.pause()', 'e.pause(),e.srcObject=null')):
            with self.subTest(pid=pid, role=role), self.assertRaises(ValueError):
                self.mutate(pid, role, old, changed)

    def test_source_picker_capture_is_disabled_by_the_real_x_mount(self):
        self.assertTrue(self.result["source_contract"]["capability_probe"]["separate_from_preview_stream"])
        for pid in self.products:
            body = self.body(pid, "source_picker")
            self.assertIn('cameraHardwareDeviceId_', body)
            self.assertIn('currentPreviewCameraHardwareId', body)
            self.assertIn('reconnectCamera', body)
        self.assertIn('checkUSBCapacity:!1', self.body(3596, "camera_page"))
        self.assertNotIn('checkUSBCapacity:!1', self.body(3595, "camera_page"))

    def test_host_focus_visibility_and_style_hiding_are_distinct_from_stop(self):
        value = self.result["source_contract"]["focus"]
        self.assertTrue(value["host_tab_gate_before_capture"])
        self.assertFalse(value["all_hidden_playing_streams_proven_stopped"])
        self.assertTrue(self.result["source_contract"]["product_mount"]["preview_retained_when_help_hidden_by_style"])
        for pid in self.products:
            self.assertIn('extraStyle:e?null:{display:"none"}', self.body(pid, "product_render"))
            with self.subTest(pid=pid), self.assertRaisesRegex(ValueError, "focus_status"):
                self.mutate(pid, "focus_status", 'action:"getWindowStatus"', 'action:"getFocusTab"')

    def test_preview_toggle_persists_configuration_and_does_not_start_native_stream(self):
        value = self.result["source_contract"]["persistence"]
        self.assertTrue(value["preview_toggle_maker_only_saves_is_preview"])
        self.assertFalse(value["native_start_stop_called_by_toggle"])
        for pid in self.products:
            with self.subTest(pid=pid), self.assertRaisesRegex(ValueError, "persistence/native boundary"):
                self.mutate(pid, "preview_persistence", 'if(i)', 'device.startCamera();if(i)')

    def test_stop_message_lock_timeout_and_post_wait_abort_are_not_quiescence(self):
        value = self.result["source_contract"]["stop_lock"]
        self.assertEqual(value["promise_timeout_ms"], 5000)
        self.assertEqual(value["callers"], ["watermark", "mjpegQuality", "hdr", "ldc"])
        self.assertTrue(value["web_lock_if_available"])
        self.assertFalse(value["abort_rechecked_after_wait"])
        self.assertFalse(value["notification_proves_tracks_ended"])
        for pid in self.products:
            for name in value["callers"]:
                with self.subTest(pid=pid, caller=name), self.assertRaisesRegex(ValueError, "post-lock cancellation"):
                    self.mutate(pid, "stop_lock_caller_" + name, 's.result=!0', 'if(a.signal.aborted)throw Error();s.result=!0')

    def test_physical_matching_and_unawaited_cleanup_keep_their_own_owner(self):
        value = self.result["source_contract"]["physical_removal"]
        self.assertTrue(value["matches_container_and_product"])
        self.assertTrue(value["ignore_flag_can_skip_cleanup"])
        self.assertFalse(value["cleanup_awaits_adapter_terminate"])
        self.assertTrue(value["also_calls_before_unload_handler"])
        self.assertFalse(value["host_callback_drain_proven"])
        for pid in self.products:
            with self.subTest(pid=pid), self.assertRaisesRegex(ValueError, "physical_removal_consumer"):
                self.mutate(pid, "physical_removal_consumer", 'e.deviceContainerId===f.wI.CONTAINER_ID', 'true')

    def test_current_identity_stays_pending_and_native_lifecycle_is_reused(self):
        value = self.result["source_contract"]["identity"]
        self.assertEqual((value["lookup_attempts"], value["retry_delay_ms"]), (9, 1000))
        self.assertTrue(value["exact_container_comparison"])
        self.assertFalse(value["kimberly_host_identity_implementation_reviewed"])
        self.assertEqual(self.result["reused_native_function_count"], 42)
        self.assertEqual(self.result["new_native_captures"], 0)
        for pid in self.products:
            with self.subTest(pid=pid), self.assertRaisesRegex(ValueError, "hardware_id_request"):
                self.mutate(pid, "hardware_id_request", 'e.value.containerId===D.wI.CONTAINER_ID', 'true')
        self.assertEqual([row["section"] for row in self.result["upstream_sections"]],
            ["image_properties", "image_host_chain", "device_lifecycle"])

    def test_tampered_complete_owner_is_rejected_and_snapshot_stays_stable(self):
        owner = self.audit.owners[self.products[3595]["roles"]["preview"]]
        for changed in ({"body": '()=>true'}, {"byte_start": owner["byte_start"] + 1}):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                host.verify_profiles_owner(self.audit, {**owner, **changed})
        self.assertEqual(self.audit.final_snapshot(), [])
        report = {**self.result, "complete_source_owners": list(self.audit.owners.values())}
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["target_execution"])
        self.assertEqual(report["official_dll_comparison"], "not_executed")
        self.assertEqual(report["temporary_artifacts_created"], [])


class CameraKimberlyIdentityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_camera_kimberly_identity_chain(cls.audit)
        cls.owners = {role: cls.audit.owners[key] for role, key in cls.result["roles"].items()}
        cls.raw = cls.audit.read(cls.result["native"]["original"])

    def test_current_action_host_export_and_browser_result_keep_their_identities(self):
        value = self.result["source_contract"]
        self.assertEqual((value["lookup_attempts"], value["retry_delay_ms"]), (9, 1000))
        self.assertEqual(value["native_arguments"], ["browser deviceId", "origin"])
        self.assertEqual(value["published_value"], "matching browser deviceId, not native container")
        self.assertTrue(value["selects_result_by_exact_current_container"])
        self.assertFalse(value["enumeration_repeated_on_retry"])
        self.assertFalse(value["request_generation_or_abort_guard"])
        self.assertFalse(value["host"]["result"]["copy_failure_still_frees_pointer"])

    def test_current_mf_hmac_salt_and_initialized_instance_are_separate_dependencies(self):
        value = self.result["native_contract"]
        self.assertEqual(value["mapping"]["hash_key"], "origin UTF-8 bytes")
        self.assertEqual(value["mapping"]["hash_input"], "full MF symbolic link in UTF-8 followed by unmodified preference salt")
        self.assertEqual(value["mapping"]["algorithm"], "HMAC-SHA256")
        self.assertEqual(value["mapping"]["result"], "64 lowercase hex digits")
        self.assertTrue(value["mapping"]["requires_initialized_native_instance"])
        self.assertEqual(value["mapping"]["native_path_comparison"], "exact UTF-16 prefix before #{")
        self.assertEqual(value["salt"]["json_path"], ["electron", "media", "device_id_salt"])
        self.assertFalse(value["salt"]["negative_result_cached"])
        self.assertFalse(value["salt"]["creates_or_replaces_salt"])

    def test_empty_identity_allocation_and_original_failure_cleanup_are_not_success(self):
        value = self.result["native_contract"]
        self.assertIn("empty string", value["mapping"]["no_match_or_empty_input"])
        self.assertEqual(value["mapping"]["allocation_failure"], "null pointer")
        self.assertFalse(value["mapping"]["null_native_arguments_are_validated"])
        self.assertFalse(value["lifetime"]["mf_early_failure_guarantees_balanced_shutdown"])
        self.assertFalse(value["lifetime"]["failed_second_allocated_string_frees_first"])
        self.assertFalse(value["lifetime"]["algorithm_provider_closed_when_nonnull"])
        self.assertEqual(value["lifetime"]["release"], "FreeMalloc -> thunk -> HeapFree")

    def test_changed_device_id_origin_abi_exact_match_or_result_consumer_is_rejected(self):
        for role, old, new in (("identity_3595_adapter", 'actionArgs:[t,i]', 'actionArgs:[i,t]'),
                ("identity_3596_abi", '["pointer",', '["string",'),
                ("identity_3595_request", 'e.value.containerId===D.wI.CONTAINER_ID', 'true'),
                ("identity_3596_response", 'cameraHardwareId:E.payload', 'cameraHardwareId:cached'),
                ("identity_3595_action_event", 'GET_CAMERA_HARDWARE_ID', 'GET_DEVICE_ID')):
            owners = {**self.owners, role: dict(self.owners[role])}
            self.assertIn(old, owners[role]["body"])
            owners[role]["body"] = owners[role]["body"].replace(old, new, 1)
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "Kimberly identity source contract"):
                families.camera_kimberly_identity_source_contract(owners)

    def test_complete_native_identity_helpers_and_failure_branches_cannot_be_rewritten(self):
        for rva, old, new in (("0x3ee40", 'v81 == v10', 'v81 <= v10'),
                ("0x1b880", 'if ( !phAlgorithm[0] )', 'if ( phAlgorithm[0] )'),
                ("0x1ac00", 'device_id_salt', 'device_id_hash'),
                ("0x1be10", 'MFStartup(0x20070u, 0)', 'MFStartup(0x20070u, 1)')):
            native = copy.deepcopy(self.result["native"])
            body = next(row["capture"] for row in native["complete_functions"] if row["capture"]["rva"] == rva)
            row = next(row for row in body["pseudocode"]["lines"] if old in row["text"])
            row["text"] = row["text"].replace(old, new, 1)
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete helper boundary"):
                families.camera_kimberly_identity_native_contract(native, self.raw)

    def test_missing_complete_function_actual_call_edge_or_export_is_rejected(self):
        native = copy.deepcopy(self.result["native"])
        native["complete_functions"] = [row for row in native["complete_functions"] if row["capture"]["rva"] != "0x1ac00"]
        with self.assertRaisesRegex(ValueError, "complete helper scope"):
            families.camera_kimberly_identity_native_contract(native, self.raw)
        native = copy.deepcopy(self.result["native"])
        native["direct_edges"] = [row for row in native["direct_edges"]
            if not (row["from"] == "0x18003ee40" and row["function"] == "0x18001b880")]
        with self.assertRaisesRegex(ValueError, "actual helper call edge"):
            families.camera_kimberly_identity_native_contract(native, self.raw)
        native = copy.deepcopy(self.result["native"])
        native["exports"][0]["rva"] = "0x5edd0"
        with self.assertRaisesRegex(ValueError, "current export"):
            families.camera_kimberly_identity_native_contract(native, self.raw)

    def test_native_machine_code_and_rehashed_region_mutations_disagree_with_actual_pe(self):
        native = copy.deepcopy(self.result["native"])
        row = next(row for item in native["complete_functions"] for row in item["capture"]["assembly"] if row["rva"] == "0x3f250")
        row["bytes"] = "e82cc6fdff"
        with self.assertRaisesRegex(ValueError, "instruction differs from PE"):
            families.camera_kimberly_identity_native_contract(native, self.raw)
        native = copy.deepcopy(self.result["native"])
        row = next(row for row in native["verified_data_regions"] if row["role"] == "hex_digits")
        row["raw_hex"] = b"0123456789ABCDEF".hex()
        row["sha256"] = hashlib.sha256(bytes.fromhex(row["raw_hex"])).hexdigest()
        with self.assertRaisesRegex(ValueError, "data region differs from PE"):
            families.camera_kimberly_identity_native_contract(native, self.raw)

    def test_current_acquisition_change_cannot_reuse_the_selected_identity_semantics(self):
        import frontend
        with patch.object(frontend, "current_source_selection", return_value={"acquisition": {"path": "old/acquisition.json"}}), \
                self.assertRaisesRegex(ValueError, "current acquisition changed"):
            families.collect_camera_kimberly_identity_chain(host.HostLocalAudit())

    def test_complete_owners_snapshot_and_json_keep_local_review_separate_from_acceptance(self):
        for owner in self.audit.owners.values():
            self.assertEqual(host.verify_profiles_owner(self.audit, owner).text.decode(), owner["body"])
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(len(self.result["native"]["complete_functions"]), 16)
        self.assertEqual(self.result["native_contract"]["verified_instruction_count"], 3138)
        self.assertEqual(len(self.result["native_contract"]["required_direct_edges"]), 14)
        self.assertEqual(self.result["new_native_captures"], 0)
        self.assertEqual(json.loads(json.dumps(self.result, ensure_ascii=False)), self.result)
        for key in ("source_chain_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(self.result[key])
        self.assertEqual(self.result["official_dll_comparison"], "not_executed")
        self.assertEqual(self.result["temporary_artifacts_created"], [])


class CameraKimberlyIdentityDispatchTests(unittest.TestCase):
    def test_query_routes_without_replaying_camera_preview_or_device_queries(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-family-function", "--family", "accessories", "--function", "main",
            "--query", "camera-kimberly-identity-chain", "--product", "3595", "--product", "3596"])
        with patch.object(families, "audit_camera_kimberly_identity_chain") as selected, \
                patch.object(families, "audit_camera_media_preview_chain") as preview, \
                patch.object(families, "audit_camera_device_lifecycle") as device:
            families.audit_family_function(args)
        selected.assert_called_once_with(args)
        preview.assert_not_called()
        device.assert_not_called()

    def test_scope_products_and_production_preparation_are_rejected(self):
        import argparse
        for family, function in (("audio", "main"), ("accessories", "mw")):
            with self.subTest(family=family, function=function), self.assertRaisesRegex(ValueError, "current accessories main scope"):
                families.audit_family_function(argparse.Namespace(family=family, function=function, query="camera-kimberly-identity-chain"))
        for products, prepare in (([3595], False), ([3596], False), ([3595, 3596, 3594], False), ([3595, 3596], True)):
            with self.subTest(products=products, prepare=prepare), self.assertRaisesRegex(ValueError, "PID3595 and PID3596 without production preparation"):
                families.audit_camera_kimberly_identity_chain(argparse.Namespace(family="accessories", function="main", product=products, prepare=prepare))

    def test_success_updates_only_the_kimberly_identity_section(self):
        import argparse
        with patch.object(families, "collect_camera_kimberly_identity_chain", return_value={"failures": []}) as selected, \
                patch.object(families, "collect_camera_media_preview_chain") as preview, \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            report = families.audit_camera_kimberly_identity_chain(argparse.Namespace(family="accessories", function="main", product=[3595, 3596], prepare=False))
        selected.assert_called_once()
        preview.assert_not_called()
        self.assertIs(publish.call_args.args[0], report)
        self.assertEqual(publish.call_args.args[3], "kimberly_identity_chain")
        self.assertTrue(report["bounded_observation_complete"])
        self.assertTrue(report["snapshot_stable"])
        self.assertFalse(report["source_chain_complete"])
        self.assertEqual(report["official_dll_comparison"], "not_executed")

    def test_failure_or_changed_snapshot_is_preserved_in_the_existing_receipt(self):
        import argparse
        for mode in ("exception", "returned_failure", "changed"):
            configured = ({"side_effect": ValueError("unresolved identity helper")} if mode == "exception" else {
                "return_value": {"failures": ["unresolved identity helper"] if mode == "returned_failure" else []}})
            with self.subTest(mode=mode), patch.object(families, "collect_camera_kimberly_identity_chain", **configured), \
                    patch.object(host.HostLocalAudit, "final_snapshot", return_value=["identity-owner"] if mode == "changed" else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "Kimberly identity observation remains unresolved"):
                families.audit_camera_kimberly_identity_chain(argparse.Namespace(family="accessories", function="main", product=[3595, 3596], prepare=False))
            report = publish.call_args.args[0]
            self.assertEqual(publish.call_args.args[3], "kimberly_identity_chain")
            self.assertFalse(report["bounded_observation_complete"])
            self.assertEqual(report["snapshot_stable"], mode != "changed")
            self.assertIn("identity-owner" if mode == "changed" else "unresolved identity helper", report["failures"][0])


class CameraDeviceLifecycleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.audit = host.HostLocalAudit()
        cls.result = families.collect_camera_device_lifecycle(cls.audit)
        cls.owners = {role: cls.audit.owners[key] for role, key in cls.result["roles"].items()}

    def test_browser_selection_keeps_label_group_and_native_identity_boundaries(self):
        value = self.result["source_contract"]["browser_selection"]
        self.assertEqual(value["label_substrings"], ["1532:0e0a", "1532:0e0b", "1532:0e0c"])
        self.assertTrue(value["requested_id_checked_only_after_label_match"])
        self.assertIn("last matching video wins", value["fallback"])
        self.assertFalse(value["clears_prior_device_id_before_enumeration"])
        self.assertFalse(value["connect_device_result_proves_native_open"])
        self.assertFalse(value["browser_identity_is_native_container_id"])

    def test_directshow_open_retains_its_actual_matching_and_error_boundaries(self):
        value = self.result["native_contract"]
        self.assertEqual(value["enumeration"]["vendor_id"], 0x1532)
        self.assertEqual(value["enumeration"]["native_product_ids"], [0x0e0a, 0x0e0b, 0x0e0c])
        self.assertEqual(value["enumeration"]["container_match"], "exact length and bytes")
        self.assertTrue(value["enumeration"]["enumerate_export_ignores_refresh_failure"])
        self.assertEqual(value["open"]["device_path_match"], "wide string substring")
        self.assertFalse(value["open"]["bind_to_object_hresult_checked"])
        self.assertTrue(value["open"]["success_requires_non_null_filter"])
        self.assertTrue(value["open"]["matching_branch_skips_property_bag_release_and_variant_clear"])
        self.assertTrue(value["open"]["manager_retains_only_after_connect_success"])

    def test_actual_product_refcount_and_destructor_slots_are_separately_resolved(self):
        value = self.result["native_contract"]
        self.assertEqual(value["open"]["product_classes"], {"3595": "CRzDeviceAccess", "3596": "CRzDeviceAccess_0E0C"})
        self.assertEqual(value["vtable_targets"]["base_refcount"], ["0x35020", "0x35000", "0x42f30"])
        self.assertEqual(value["vtable_targets"]["x_refcount"], ["0x35020", "0x35000", "0x42f60"])
        self.assertEqual(value["vtable_targets"]["base_deleting_destructor"], ["0x4adf0"])
        self.assertEqual(value["vtable_targets"]["x_deleting_destructor"], ["0x4adf0"])
        self.assertTrue(value["ownership"]["lookup_drops_temporary_strong_reference_before_return"])
        self.assertTrue(value["ownership"]["lookup_returns_borrowed_raw_pointer"])

    def test_normal_worker_join_does_not_claim_concurrent_calls_or_media_are_stopped(self):
        value = self.result["native_contract"]
        self.assertTrue(value["ownership"]["disconnect_filter_release_precedes_worker_stop"])
        self.assertTrue(value["ownership"]["worker_created_before_filter_open"])
        self.assertEqual(value["ownership"]["join_wait_ms"], "INFINITE")
        self.assertTrue(value["ownership"]["pending_queues_disposed_by_destructor"])
        self.assertFalse(value["ownership"]["queue_disposal_proves_each_hardware_operation_canceled"])
        self.assertFalse(value["reconnect"]["property_calls_are_queued_to_worker"])
        self.assertFalse(value["reconnect"]["terminate_uses_the_same_instance_mutex"])
        self.assertFalse(value["reconnect"]["generation_guard_or_callback_quiescence_proven"])
        self.assertFalse(value["stream"]["native_exports_confirm_media_tracks_started_or_stopped"])

    def test_changed_real_selector_fallback_label_and_native_call_are_rejected(self):
        for role, old, changed in (("mw_3595_device_select", "i.groupId===t.groupId", "i.deviceId===t.deviceId"),
                ("mw_3596_device_products", "1532:0e0c", "1532:0e0d"),
                ("mw_3595_connect_device", "{result:!0,error:", "this.callDLLApi({result:!0,error:")):
            owners = {**self.owners, role: dict(self.owners[role])}
            owners[role]["body"] = owners[role]["body"].replace(old, changed, 1)
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "Camera (device|connectDevice)"):
                families.camera_device_source_contract(owners)

    def test_complete_ast_owner_rejects_body_or_byte_range_tampering(self):
        for changed in ({"body": "getDevice(){return true}"}, {"byte_start": self.owners["mw_3595_device_select"]["byte_start"] + 1}):
            owner = {**self.owners["mw_3595_device_select"], **changed}
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                host.verify_profiles_owner(host.HostLocalAudit(), owner)

    def test_physical_native_branch_bind_thread_and_refcount_bytes_are_bound(self):
        for rva, raw in (("0x3dabf", "c78424900000000d0e0b0e"), ("0x4b2d0", "ff5048"),
                         ("0x4b366", "eb00"), ("0x3bc43", "498bc4"), ("0x69d7a", "33d2")):
            native = copy.deepcopy(self.result["native"])
            row = next(row for item in native["complete_functions"] for row in item["capture"]["assembly"] if row["rva"] == rva)
            row["bytes"] = raw
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "physical lifecycle ABI"):
                families.camera_device_native_contract(native)
        self.assertEqual(len(self.result["native_contract"]["instruction_assertions"]), 39)

    def test_vtable_and_container_property_are_bound_to_the_original_pe(self):
        for role in ("base_refcount", "x_refcount", "x_deleting_destructor", "DEVPKEY_Device_ContainerId"):
            native = copy.deepcopy(self.result["native"])
            row = next(row for row in native["verified_data_regions"] if row["role"] == role)
            row["raw_hex"] = "00" + row["raw_hex"][2:]
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "vtable|identity"):
                families.camera_device_native_contract(native)

    def test_changed_release_order_is_rejected_without_relaxing_the_result(self):
        native = copy.deepcopy(self.result["native"])
        function = next(item["capture"] for item in native["complete_functions"] if item["capture"]["rva"] == "0x4b680")
        lines = function["pseudocode"]["lines"]
        clear = next(i for i, row in enumerate(lines) if "a1 + 64) = 0" in row["text"])
        stop = next(i for i, row in enumerate(lines) if "return sub_18004A9B0" in row["text"])
        lines[clear], lines[stop] = lines[stop], lines[clear]
        with self.assertRaisesRegex(ValueError, "release order"):
            families.camera_device_native_contract(native)

    def test_complete_functions_call_edges_snapshot_and_json_remain_reproducible(self):
        native = self.result["native"]
        self.assertEqual(len(native["complete_functions"]), 42)
        self.assertTrue(native["all_selected_pseudocode_captured"])
        self.assertTrue(all(item["input_chunks_verified"] for item in native["complete_functions"]))
        edges = {(row["from"], row["function"]) for row in native["direct_edges"]}
        for parent, child in ((0x3bc50, 0x3d4f0), (0x3bc50, 0x4aee0), (0x4adf0, 0x4ae30),
                              (0x4ae30, 0x4a3d0), (0x4b680, 0x4a9b0), (0x4a9b0, 0x69d64)):
            self.assertIn((hex(0x180000000 + parent), hex(0x180000000 + child)), edges)
        for owner in self.audit.owners.values():
            node = host.verify_profiles_owner(self.audit, owner)
            self.assertEqual(node.text.decode(), owner["body"])
        report = {**self.result, "complete_source_owners": list(self.audit.owners.values())}
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["target_execution"])
        self.assertEqual(report["official_dll_comparison"], "not_executed")


class CameraLifecycleOwnerTests(unittest.TestCase):
    def setUp(self):
        self.audit = host.HostLocalAudit()
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.root_patch = patch.object(analysis, "ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.raw = b'const open=()=>{return device.showSettingsPage()};'
        self.path = self.root / '.analysis/inputs/camera/fixture.js'
        self.path.parent.mkdir(parents=True)
        self.path.write_bytes(self.raw)
        self.source = {'path': '.analysis/inputs/camera/fixture.js', 'bytes': len(self.raw),
                       'sha256': hashlib.sha256(self.raw).hexdigest()}
        node = self.audit.tree(self.source).root_node.named_children[0].named_children[0]
        self.owner = {'original': self.source, 'byte_start': node.start_byte, 'byte_end': node.end_byte,
                      'syntax': node.type, 'body': node.text.decode(),
                      'body_sha256': hashlib.sha256(node.text).hexdigest()}

    def test_whole_owner_is_reconstructible_without_duplicate_body(self):
        result = families.camera_lifecycle_owner(self.audit, self.owner)
        body = self.path.read_bytes()[result['byte_start']:result['byte_end']]
        self.assertEqual(hashlib.sha256(body).hexdigest(), result['body_sha256'])
        self.assertEqual(result['syntax'], 'variable_declarator')
        self.assertNotIn('body', result)
        self.assertEqual(result['body_storage'], 'complete_verified_original_slice')

    def test_modified_body_and_partial_or_wrong_syntax_owner_are_rejected(self):
        owner = copy.deepcopy(self.owner)
        owner['body'] = 'open=()=>true'
        with self.assertRaisesRegex(ValueError, 'differs'):
            families.camera_lifecycle_owner(self.audit, owner)
        for change in ({'byte_start': -1}, {'byte_end': len(self.raw) + 1}, {'syntax': 'function_expression'}):
            owner = {**self.owner, **change}
            with self.subTest(change=change), self.assertRaises(ValueError):
                families.camera_lifecycle_owner(self.audit, owner)
        owner = copy.deepcopy(self.owner)
        owner['byte_start'] += 1
        fragment = self.raw[owner['byte_start']:owner['byte_end']]
        owner.update(body=fragment.decode(), body_sha256=hashlib.sha256(fragment).hexdigest())
        with self.assertRaisesRegex(ValueError, 'complete original AST'):
            families.camera_lifecycle_owner(self.audit, owner)

    def test_changed_original_and_outside_input_path_are_rejected(self):
        self.path.write_bytes(self.raw.replace(b'device', b'cached'))
        with self.assertRaisesRegex(ValueError, 'identity differs'):
            families.camera_lifecycle_owner(host.HostLocalAudit(), self.owner)
        owner = copy.deepcopy(self.owner)
        owner['original']['path'] = '.work/camera.js'
        with self.assertRaisesRegex(ValueError, 'outside current inputs'):
            families.camera_lifecycle_owner(host.HostLocalAudit(), owner)


class NativeDescriptorWireTests(unittest.TestCase):
    def test_complete_nested_descriptor_preserves_field_numbers_and_wire_types(self):
        wire = bytes.fromhex("0a015812090a0178180120012808")
        outer = families.audio_proto_descriptor_fields(wire)
        self.assertEqual(outer, [(1, 2, b"X"), (2, 2, bytes.fromhex("0a0178180120012808"))])
        self.assertEqual(families.audio_proto_descriptor_fields(outer[1][2]),
                         [(1, 2, b"x"), (3, 0, 1), (4, 0, 1), (5, 0, 8)])

    def test_mixed_varint_fixed_and_utf8_wire_values_are_bounded_bytes(self):
        wire = bytes.fromhex("08ac021101020304050607081a02c3a925aabbccdd")
        self.assertEqual(families.audio_proto_descriptor_fields(wire),
            [(1, 0, 300), (2, 1, bytes(range(1, 9))), (3, 2, "é".encode()),
             (4, 5, bytes.fromhex("aabbccdd"))])
        self.assertEqual(families.audio_proto_descriptor_fields(b""), [])

    def test_uint64_max_value_is_accepted_without_truncation(self):
        self.assertEqual(families.audio_proto_descriptor_fields(b"\x08" + b"\xff"*9 + b"\x01"),
                         [(1, 0, (1 << 64)-1)])

    def test_truncated_tags_values_lengths_and_fixed_fields_are_rejected(self):
        for wire in (b"\x80", b"\x08\x80", b"\x0a\x80", b"\x0a\x03ab",
                     b"\x09" + b"\x00"*7, b"\x0d" + b"\x00"*3):
            with self.subTest(wire=wire.hex()), self.assertRaisesRegex(ValueError, "Truncated"):
                families.audio_proto_descriptor_fields(wire)

    def test_varint_overflow_and_unbounded_continuation_are_rejected(self):
        for value in (b"\xff"*9 + b"\x02", b"\x80"*10, b"\xff"*11):
            with self.subTest(value=value.hex()), self.assertRaisesRegex(ValueError, "uint64"):
                families.audio_proto_descriptor_fields(b"\x08" + value)

    def test_zero_or_overwide_field_numbers_and_group_wire_types_are_rejected(self):
        for wire in (b"\x00\x01", bytes.fromhex("808080801000"), b"\x0b", b"\x0c", b"\x0e", b"\x0f"):
            with self.subTest(wire=wire.hex()), self.assertRaises(ValueError):
                families.audio_proto_descriptor_fields(wire)


class AudioNativePeEvidenceTests(unittest.TestCase):
    def test_no_export_service_exe_still_provides_exact_file_backed_ranges(self):
        raw, _ = pe_fixture()
        struct.pack_into("<H", raw, 0x96, 0x22)
        raw[0x210:0x214] = b"FIR!"
        base, read, exports = families.audio_native_evidence_pe(raw)
        self.assertEqual(base, 0x180000000)
        self.assertEqual(exports, [])
        self.assertEqual(read(0x1010, 4), b"FIR!")
        self.assertEqual(read.file_offset(0x1010, 4), 0x210)

    def test_virtual_padding_and_empty_or_cross_section_ranges_are_rejected(self):
        raw, _ = pe_fixture()
        struct.pack_into("<I", raw, 0x188+8, 0x1000)
        _, read, _ = families.audio_native_evidence_pe(raw)
        for rva, size in ((0x1e00, 1), (0x1000, 0), (0x1dff, 2), (0xfff, 1)):
            with self.subTest(rva=rva, size=size), self.assertRaisesRegex(ValueError, "unique file backing"):
                read(rva, size)

    def test_overlapping_rva_sections_are_rejected_instead_of_selecting_first(self):
        raw, _ = pe_fixture()
        struct.pack_into("<H", raw, 0x86, 2)
        raw[0x1b0:0x1d8] = raw[0x188:0x1b0]
        _, read, _ = families.audio_native_evidence_pe(raw)
        with self.assertRaisesRegex(ValueError, "unique file backing"):
            read(0x1000, 1)

    def test_section_file_offset_beyond_actual_original_is_rejected(self):
        raw, _ = pe_fixture()
        struct.pack_into("<I", raw, 0x188+20, 0x2000)
        _, read, _ = families.audio_native_evidence_pe(raw)
        with self.assertRaisesRegex(ValueError, "truncated"):
            read.file_offset(0x1000, 1)

    def test_x86_and_truncated_headers_are_not_accepted_as_x64_evidence(self):
        raw, _ = pe_fixture(32)
        with self.assertRaisesRegex(ValueError, "x64"):
            families.audio_native_evidence_pe(raw)
        raw, _ = pe_fixture()
        for value in (b"", b"MZ", bytes(raw[:0x90]), bytes(raw[:0x1a0])):
            with self.subTest(length=len(value)), self.assertRaises(ValueError):
                families.audio_native_evidence_pe(value)


class AudioFirUnwindBoundaryTests(unittest.TestCase):
    def reader(self, spans):
        def read(rva, size):
            matches = [value[rva-start:rva-start+size] for start, value in spans.items()
                       if start <= rva and rva+size <= start+len(value)]
            if len(matches) != 1 or size <= 0:
                raise ValueError("fixture range has no unique backing")
            return matches[0]
        read.file_offset = lambda rva, size: rva if read(rva, size) else None
        return read

    def test_no_handler_and_gs_only_records_remain_distinct_from_cpp_cleanup(self):
        read = self.reader({0x1000: bytes.fromhex("0114080014640a00145409001434080014521070"),
                            0x2000: bytes.fromhex("1916030007c20370023000003861190048000000")})
        direct = families.audio_x64_unwind_record(read, 0x1000)
        registration = families.audio_x64_unwind_record(read, 0x2000)
        self.assertEqual(direct['flags'], 0)
        self.assertNotIn('handler_rva', direct)
        self.assertEqual(registration['flags'], 3)
        self.assertEqual(registration['handler_rva'], '0x196138')
        self.assertEqual(registration['handler_data_u32'], 72)

    def test_chained_registration_retains_parent_handler_and_exact_bounds(self):
        read = self.reader({0x1000: bytes.fromhex("21000000b0f30100f7f3010000200000"),
                            0x2000: bytes.fromhex("1916030007c20370023000003861190048000000")})
        record = families.audio_x64_unwind_record(read, 0x1000)
        self.assertEqual(record['flags'], 4)
        self.assertEqual(record['chain']['begin_rva'], '0x1f3b0')
        self.assertEqual(record['chain']['end_rva'], '0x1f3f7')
        self.assertEqual(record['chain']['unwind']['handler_rva'], '0x196138')

    def test_unwind_cycle_mixed_flags_bad_version_and_truncation_are_rejected(self):
        for value in (bytes.fromhex('21000000001000002010000000100000'),
                      bytes.fromhex('39000000'), bytes.fromhex('02000000'),
                      bytes.fromhex('19000000'), bytes.fromhex('21000000201000001010000000200000')):
            with self.subTest(value=value.hex()), self.assertRaises(ValueError):
                families.audio_x64_unwind_record(self.reader({0x1000: value}), 0x1000)

    def test_fh4_compressed_count_and_guard_offset_use_original_signed_table(self):
        table = bytes.fromhex('fffefffdfffefffcfffefffdfffefffb1912190b191219041912190b19121900')
        read = self.reader({0x1000: bytes.fromhex('000000020ad027020040')})
        self.assertEqual(families.audio_fh4_unsigned(read, 0x1003, table), (1, 0x1004))
        self.assertEqual(families.audio_fh4_unsigned(read, 0x1004, table), (5, 0x1005))
        self.assertEqual(families.audio_fh4_unsigned(read, 0x1009, table), (32, 0x100a))

    def test_fh4_invalid_tables_and_unbacked_integer_reads_fail(self):
        read = self.reader({0x1000: b'\0'})
        for table in (b'', b'\0'*32, b'\xff'*32):
            with self.subTest(table=table.hex()), self.assertRaises(ValueError):
                families.audio_fh4_unsigned(read, 0x1000, table)

    def fh4_scope_fixture(self, header=0x28, unwind=bytes.fromhex('020ad027020040'),
                          ipmap=bytes.fromhex('0402020a00')):
        table = bytes.fromhex('fffefffdfffefffcfffefffdfffefffb1912190b191219041912190b19121900')
        info = bytes([header])+(struct.pack('<I', 0x3000) if header & 8 else b'')+struct.pack('<I', 0x4000)
        return self.reader({0x22d008: table, 0x2000: info,
                            0x2ffd: b'\0'*3+unwind, 0x3ffd: b'\0'*3+ipmap})

    def test_worker_fh4_scope_preserves_resource_guard_and_covered_instruction_states(self):
        result = families.audio_fir_fh4_scope(self.fh4_scope_fixture(), 0x2000, 0x5000)
        self.assertFalse(result['noexcept'])
        self.assertFalse(result['try_block_map_present'])
        self.assertEqual(result['unwind_entries'], [{'rva': '0x3001', 'next_byte_offset': 1,
            'kind': 1, 'action_rva': '0x227d0', 'object_offset': 32}])
        self.assertEqual(result['ip_to_state'], [{'ip_rva': '0x5001', 'state': 0},
                                                {'ip_rva': '0x5006', 'state': -1}])
        self.assertEqual(len(result['data_regions']), 3)

    def test_worker_noexcept_scope_has_no_local_unwind_action_or_catch(self):
        read = self.fh4_scope_fixture(header=0x60, ipmap=bytes.fromhex('020200'))
        result = families.audio_fir_fh4_scope(read, 0x2000, 0x5000)
        self.assertTrue(result['noexcept'])
        self.assertEqual(result['unwind_entries'], [])
        self.assertFalse(result['try_block_map_present'])
        self.assertEqual(result['ip_to_state'], [{'ip_rva': '0x5001', 'state': -1}])

    def test_worker_unreviewed_headers_unwind_count_and_invalid_ip_states_fail(self):
        for values in ({'header': 0x68}, {'unwind': bytes.fromhex('040ad027020040')},
                       {'ipmap': b'\x12'}, {'ipmap': bytes.fromhex('020204')},
                       {'header': 0x60, 'ipmap': bytes.fromhex('020202')}):
            with self.subTest(values=values), self.assertRaises(ValueError):
                families.audio_fir_fh4_scope(self.fh4_scope_fixture(**values), 0x2000, 0x5000)


class AudioVadLifecycleScopeTests(unittest.TestCase):
    reader = AudioFirUnwindBoundaryTests.reader

    def fixture(self, header=0x28, unwind=bytes.fromhex('060a40cd0000e10644b0c50000b036b81e2200'),
                ipmap=bytes.fromhex('06020202040206')):
        table = bytes.fromhex('fffefffdfffefffcfffefffdfffefffb1912190b191219041912190b19121900')
        info = bytes([header])+(struct.pack('<I', 0x3000) if header & 8 else b'')+struct.pack('<I', 0x4000)
        return self.reader({0x323b18: table, 0x2000: info,
                            0x2ffd: b'\0'*3+unwind, 0x3ffd: b'\0'*3+ipmap})

    def test_distinct_string_mutex_and_terminate_actions_keep_real_parent_offsets(self):
        result = families.audio_vad_fh4_scope(self.fixture(), 0x2000, 0x5000)
        self.assertEqual([(row['kind'], row['action_rva'], row.get('object_offset'), row['next_state'])
                          for row in result['unwind_entries']],
                         [(1, '0xcd40', 440, -1), (2, '0xc5b0', 88, -1), (3, '0x221eb8', None, 1)])
        self.assertEqual(result['ip_to_state'], [{'ip_rva': '0x5001', 'state': 0},
            {'ip_rva': '0x5002', 'state': 1}, {'ip_rva': '0x5003', 'state': 2}])
        self.assertFalse(result['noexcept'])
        self.assertFalse(result['try_block_map_present'])

    def test_noexcept_scope_does_not_invent_a_handle_release_guard(self):
        result = families.audio_vad_fh4_scope(self.fixture(header=0x60, ipmap=bytes.fromhex('020200')),
                                              0x2000, 0x5000)
        self.assertTrue(result['noexcept'])
        self.assertEqual(result['unwind_entries'], [])
        self.assertEqual(result['ip_to_state'], [{'ip_rva': '0x5001', 'state': -1}])

    def test_unknown_header_bad_parent_state_and_repeated_ip_are_rejected(self):
        for values in ({'header': 0x38}, {'unwind': bytes.fromhex('020240cd000020')},
                       {'ipmap': bytes.fromhex('020208')}, {'ipmap': bytes.fromhex('0402020004')}):
            with self.subTest(values=values), self.assertRaises(ValueError):
                families.audio_vad_fh4_scope(self.fixture(**values), 0x2000, 0x5000)


class AudioVadKernelEventEvidenceTests(unittest.TestCase):
    reader = AudioFirUnwindBoundaryTests.reader

    def import_fixture(self, descriptors=None, lookup=None, names=None):
        raw = bytearray(512)
        struct.pack_into('<I', raw, 0x3c, 0x80)
        struct.pack_into('<II', raw, 0x80+24+120, 0x1000, 40)
        return bytes(raw), self.reader({
            0x1000: descriptors if descriptors is not None else
                struct.pack('<IIIII', 0x2000, 0, 0, 0x3000, 0x4000)+b'\0'*20,
            0x2000: lookup if lookup is not None else struct.pack('<QQ', 0x3100, 0),
            0x3000: names if names is not None else b'ntoskrnl.exe\0',
            0x3100: b'\0\0IoCreateNotificationEvent\0',
        })

    def test_kernel_import_names_and_iat_slots_come_from_original_descriptors(self):
        raw, read = self.import_fixture()
        self.assertEqual(families.audio_vad_kernel_import_slots(raw, read), {
            '0x4000': {'library': 'ntoskrnl.exe', 'name': 'IoCreateNotificationEvent'}})

    def test_ordinal_missing_originals_duplicate_slots_and_unterminated_descriptors_fail(self):
        for values in (
            {'lookup': struct.pack('<QQ', (1 << 63)+3, 0)},
            {'lookup': struct.pack('<QQ', 0x3200, 0)},
            {'descriptors': struct.pack('<IIIII', 0x2000, 0, 0, 0x3000, 0x4000)*2},
            {'descriptors': struct.pack('<IIIII', 0, 0, 0, 0x3000, 0x4000)+b'\0'*20},
        ):
            with self.subTest(values=values), self.assertRaises(ValueError):
                families.audio_vad_kernel_import_slots(*self.import_fixture(**values))
        raw, read = self.import_fixture()
        short = bytearray(raw)
        struct.pack_into('<I', short, 0x80+24+124, 20)
        with self.assertRaisesRegex(ValueError, 'terminator'):
            families.audio_vad_kernel_import_slots(bytes(short), read)

    def test_direct_signal_calls_ignore_printed_xrefs_and_decode_signed_displacement(self):
        bodies = {0x5000: {'assembly': [
            {'rva': '0x5000', 'bytes': 'e8fbefffff', 'text': 'call unrelated_label'},
            {'rva': '0x5005', 'bytes': 'e8f60f0000', 'text': 'call event_signal'},
            {'rva': '0x500a', 'bytes': 'ff1500000000', 'text': 'call event_signal'},
        ]}}
        read = self.reader({0x5000: bytes.fromhex('e8fbefffffe8f60f0000ff1500000000')})
        self.assertEqual(families.audio_vad_kernel_direct_calls(bodies, read, 0x4000),
                         [{'function_rva': '0x5000', 'instruction_rva': '0x5000'}])
        self.assertEqual(families.audio_vad_kernel_direct_calls(bodies, read, 0x6000),
                         [{'function_rva': '0x5000', 'instruction_rva': '0x5005'}])
        self.assertEqual(families.audio_vad_kernel_direct_calls(bodies, read, 0x7000), [])
        changed = copy.deepcopy(bodies)
        changed[0x5000]['assembly'][0]['bytes'] = 'e8fcefffff'
        with self.assertRaisesRegex(ValueError, 'bytes differ'):
            families.audio_vad_kernel_direct_calls(changed, read, 0x4000)


class AudioVadKernelBatchTests(unittest.TestCase):
    reader = AudioFirUnwindBoundaryTests.reader
    import_fixture = AudioVadKernelEventEvidenceTests.import_fixture

    def test_exact_two_builds_keep_distinct_package_and_event_identities(self):
        specs = families.audio_vad_kernel_build_specs()
        self.assertEqual(list(specs), [1442, 3942])
        self.assertNotEqual(specs[1442]['driver_sha256'], specs[3942]['driver_sha256'])
        self.assertNotEqual(specs[1442]['package_sha256'], specs[3942]['package_sha256'])
        self.assertIn('ThxVad_clio_x_DataAvailableEvent', families.audio_vad_kernel_event_contract(1442)['event_identity'])
        self.assertIn('ThxVad_soma_pro_DataAvailableEvent', families.audio_vad_kernel_event_contract(3942)['event_identity'])
        with self.assertRaises(KeyError):
            families.audio_vad_kernel_event_contract(1392)

    def test_referenced_bodies_reopen_without_modifying_or_duplicating_the_source(self):
        native = {'selected_body_storage': 'retained_function_artifacts',
                  'complete_selected_functions': [{'artifact': {'path': 'function.json'}, 'role': 'event'}]}
        before = copy.deepcopy(native)
        with patch.object(families, 'audio_native_evidence_original', return_value=b'{"rva":"0x1000"}') as original:
            result = families.audio_vad_kernel_native_evidence(native)
        original.assert_called_once_with({'path': 'function.json'})
        self.assertEqual(native, before)
        self.assertNotIn('selected_body_storage', result)
        self.assertEqual(result['complete_selected_functions'][0]['capture'], {'rva': '0x1000'})
        for changed in (dict(native, selected_body_storage='inline_json'),
                        dict(native, complete_selected_functions=[{'artifact': {}, 'capture': {}}])):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                families.audio_vad_kernel_native_evidence(changed)

    def opcode_fixture(self):
        body = {'assembly': [{'rva': '0x1000', 'bytes': '90', 'text': 'left printed label'}],
                'chunks': [{'start': '0x140001000', 'end': '0x140001002', 'raw_hex': '90cc'}]}
        return {1442: {0x1000: copy.deepcopy(body)}, 3942: {0x1000: copy.deepcopy(body)}}

    def test_opcode_comparison_includes_rvas_bytes_and_unenumerated_chunk_bytes(self):
        bodies = self.opcode_fixture()
        bodies[3942][0x1000]['assembly'][0]['text'] = 'unrelated printed label'
        with patch.object(families, 'audio_vad_kernel_event_selections', return_value={0x1000: 'event'}):
            result = families.collect_audio_vad_kernel_opcode_comparison(bodies)
        self.assertEqual(result['functions'], [{'function_rva': '0x1000', 'role': 'event', 'instructions': 1,
            'instruction_bytes': 1, 'raw_chunk_bytes': 2, 'instruction_rvas_and_bytes_equal': True,
            'raw_chunks_equal': True}])
        self.assertTrue(result['event_names_compared_separately'])

    def test_missing_build_changed_opcode_rva_and_unenumerated_bytes_are_rejected(self):
        with patch.object(families, 'audio_vad_kernel_event_selections', return_value={0x1000: 'event'}):
            with self.assertRaisesRegex(ValueError, 'both reviewed builds'):
                families.collect_audio_vad_kernel_opcode_comparison({1442: {}})
            for field, value in (('rva', '0x1001'), ('bytes', 'c3')):
                bodies = self.opcode_fixture()
                bodies[3942][0x1000]['assembly'][0][field] = value
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'opcode sequence'):
                    families.collect_audio_vad_kernel_opcode_comparison(bodies)
            bodies = self.opcode_fixture()
            bodies[3942][0x1000]['chunks'][0]['raw_hex'] = '90cd'
            with self.assertRaisesRegex(ValueError, 'raw chunks'):
                families.collect_audio_vad_kernel_opcode_comparison(bodies)

    def test_named_service_slots_keep_ordinal_rejection_with_an_explicit_query_boundary(self):
        raw, read = self.import_fixture(lookup=struct.pack('<QQQ', 0x3100, (1 << 63)+3, 0))
        with self.assertRaisesRegex(ValueError, 'ordinal'):
            families.audio_vad_kernel_import_slots(raw, read)
        self.assertEqual(families.audio_vad_kernel_import_slots(raw, read, selected_slots={'0x4000'}),
                         {'0x4000': {'library': 'ntoskrnl.exe', 'name': 'IoCreateNotificationEvent'}})
        with self.assertRaisesRegex(ValueError, 'ordinal'):
            families.audio_vad_kernel_import_slots(raw, read, selected_slots={'0x4008'})
        with self.assertRaisesRegex(ValueError, 'absent'):
            families.audio_vad_kernel_import_slots(raw, read, selected_slots={'0x5000'})

    def package_fixture(self, root):
        import json
        def artifact(name, content):
            path = root / '.analysis' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
            return families.identity(path)
        service = artifact('service.exe', b'service')
        package = artifact('package.exe', b'prefix:service:suffix')
        package.update(url='https://app-assets.razer.com/package.exe', status='verified',
                       declared_sha256=package['sha256'], declared_bytes=package['bytes'], manifests=[])
        manifest = artifact('manifest.json', json.dumps({'resourceManifest': {'resources': [{
            'resourceName': 'THXVAD1442', 'resourceVersion': '1.2.0.51', 'sha256': package['sha256'],
            'size': package['bytes'], 'url': package['url']}]}}).encode())
        manifest['url'] = 'https://apps.razer.com/synapse/products/1442/mw/manifest.json'
        row = {'product_id': 1442, 'manifest': manifest, 'package': package, 'service': service,
            'service_lineage': [{'child': service, 'parent': {k: package[k] for k in ('path', 'bytes', 'sha256')},
                                'kind': 'static_embedded_native_input', 'container_offset': 7}]}
        spec = dict(families.audio_vad_kernel_build_specs()[1442], service_path=service['path'],
            service_sha256=service['sha256'], package_sha256=package['sha256'],
            package_bytes=package['bytes'], lineage_edges=1)
        return row, {'manifests': [copy.deepcopy(manifest)], 'resources': [copy.deepcopy(package)]}, {1442: spec}

    def test_package_verifier_requires_own_manifest_lineage_and_actual_embedded_bytes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, 'ROOT', Path(directory)):
            row, acquisition, specs = self.package_fixture(Path(directory))
            with patch.object(families, 'audio_vad_kernel_build_specs', return_value=specs):
                families.verify_audio_vad_kernel_package_branch(row, acquisition)
                for path, value in ((('manifest', 'url'), 'https://apps.razer.com/synapse/products/3942/mw/manifest.json'),
                                    (('package', 'declared_sha256'), '0'*64),
                                    (('service', 'sha256'), '0'*64),
                                    (('service_lineage',), []),
                                    (('service_lineage', 0, 'container_offset'), 0)):
                    changed = copy.deepcopy(row)
                    owner = changed
                    for key in path[:-1]:
                        owner = owner[key]
                    owner[path[-1]] = value
                    with self.subTest(path=path), self.assertRaises(ValueError):
                        families.verify_audio_vad_kernel_package_branch(changed, acquisition)

    def test_cli_batch_modes_route_once_and_reject_filters_or_other_native_modes(self):
        import argparse
        import contextlib
        import io
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest='command', required=True))
        command = ['audit-family-function', '--family', 'audio', '--function', 'mw', '--query', 'audio-surround']
        for mode in ('capture', 'check', 'negative'):
            args = parser.parse_args(command + ['--native-vad-kernel-batch', mode])
            with patch.object(families, 'audit_audio_vad_kernel_event_batch', return_value='batch') as audit:
                self.assertEqual(families.audit_audio_surround(args), 'batch')
            audit.assert_called_once_with(args)
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            parser.parse_args(command + ['--native-vad-kernel-batch', 'check', '--native-vad-lifecycle', 'check'])
        for args in (argparse.Namespace(family='audio', function='mw', product=[1442]),
                     argparse.Namespace(family='audio', function='ui', product=None),
                     argparse.Namespace(family='keyboard', function='mw', product=None)):
            with self.subTest(args=args), self.assertRaisesRegex(ValueError, 'both exact reviewed builds'):
                families.audit_audio_vad_kernel_event_batch(args)


class CarolFfiBindingTests(unittest.TestCase):
    def setUp(self):
        self.parser = host.HostLocalAudit().parser

    def inspect(self, table, prefix="", suffix=""):
        return families.audio_carol_ffi_binding((prefix +
            'class Carol{constructor(){this.initElectron=()=>{const api=' + table +
            ';for(let n=api.length;n>0;n--){configure(Object.assign({},...api.slice(0,n)))}}}}' + suffix).encode(), self.parser)

    def test_complete_initializer_and_ordered_optional_group_are_retained(self):
        table = '[{GetDLLVersion:["pointer",[]],DeviceInitialize:["bool",["string"]]},'
        table += '{StartMonitor:["bool",["string","int"]]}]'
        row = self.inspect(table, 'const label="设备";')
        self.assertEqual(row["configuration"]["body"], table)
        self.assertEqual([[item["name"] for item in group] for group in row["groups"]],
                         [["GetDLLVersion","DeviceInitialize"],["StartMonitor"]])
        self.assertTrue(row["initializer"]["body"].startswith("this.initElectron="))
        self.assertIn("api.slice(0,n)", row["initializer"]["body"])
        self.assertEqual(row["focus_apis"], [])

    def test_comments_strings_and_other_abi_tables_do_not_create_false_binding(self):
        row = self.inspect('[{GetDLLVersion:["pointer",[]]}]',
            '// const fake=[{GetDLLVersion:["int",[]]}];\nconst label="Virtualizer";')
        self.assertEqual(row["groups"][0][0]["signature"], ["pointer",[]])
        self.assertEqual(row["focus_apis"], [])

    def test_real_focus_api_is_not_hidden_by_unrelated_text(self):
        row = self.inspect('[{GetDLLVersion:["pointer",[]],SetVirtualizerFirs:["int",["pointer"]]}]')
        self.assertEqual(row["focus_apis"], ["SetVirtualizerFirs"])

    def test_ambiguous_tables_and_wrong_lexical_owner_are_rejected(self):
        with self.assertRaisesRegex(ValueError,"ambiguous"):
            self.inspect('[{GetDLLVersion:["pointer",[]]}]', suffix='const fake=[{GetDLLVersion:["int",[]]}];')
        with self.assertRaisesRegex(ValueError,"complete initializer"):
            families.audio_carol_ffi_binding(b'const table=[{GetDLLVersion:["int",[]]}]', self.parser)

    def test_dynamic_properties_signatures_and_grammar_errors_are_rejected(self):
        for table in ('[{GetDLLVersion:signature}]', '[{GetDLLVersion:["int",[]],...extra}]',
                      '[{GetDLLVersion:["int",[]],[name]:["int",[]]}]'):
            with self.subTest(table=table), self.assertRaises(ValueError):
                self.inspect(table)
        with self.assertRaisesRegex(ValueError,"grammar"):
            families.audio_carol_ffi_binding(b'const a=[{GetDLLVersion:', self.parser)


class NativeInitializerTests(unittest.TestCase):
    @staticmethod
    def store(address, target, value):
        encoded = (b"\xc7\x05" + (target - address - 10).to_bytes(4, "little", signed=True)
                   + value.to_bytes(4, "little"))
        return {"address": hex(address), "bytes": encoded.hex()}

    def test_actual_instruction_bytes_cover_all_entries_in_source_order(self):
        rows = [self.store(0x4000, 0x3004, 0x80000001), self.store(0x4010, 0x3000, 17)]
        self.assertEqual(families.native_rip_u32_initializers(rows, 0x3000, 2), [17, 0x80000001])
        rows[0]["text"] = "misleading decompiler text"
        self.assertEqual(families.native_rip_u32_initializers(rows, 0x3000, 2), [17, 0x80000001])
        with self.assertRaisesRegex(ValueError, "complete source table"):
            families.native_rip_u32_initializers(rows[1:], 0x3000, 2)
        with self.assertRaisesRegex(ValueError, "duplicate"):
            families.native_rip_u32_initializers(rows + rows[:1], 0x3000, 2)
        with self.assertRaisesRegex(ValueError, "unaligned"):
            families.native_rip_u32_initializers([self.store(0x4000, 0x3001, 1)], 0x3000, 2)


class FeatureCallInventoryTests(unittest.TestCase):
    def inspect(self, source):
        raw = source.encode("utf-8")
        tree = host.HostLocalAudit().parser.parse(raw)
        self.assertFalse(tree.root_node.has_error)
        rows = families.javascript_feature_calls(tree)
        for row in rows:
            self.assertTrue(raw[row["byte_start"]:row["byte_end"]].endswith(b")"))
        return rows

    def test_actual_calls_ignore_comments_strings_and_method_declarations(self):
        rows = self.inspect('const label="设备 .useFeature(fake)";'
            '// injector.useFeature("hardwareEvent", "config")\n'
            'class Injector {useFeature(name, variant) {}}'
            'injector.useFeature("rzDeviceType","rzDevice25",{isDualLinkDevice:!0});'
            'injector.useFeature("battery","yes");')
        self.assertEqual([(row["feature"], row["variant"]) for row in rows],
                         [("rzDeviceType", "rzDevice25"), ("battery", "yes")])
        self.assertEqual(rows[0]["arguments"][2], "{isDualLinkDevice:!0}")

    def test_literal_computed_optional_and_dynamic_arguments_are_retained(self):
        rows = self.inspect('injector["useFeature"]("hardwareEvent", "config", settings);'
            'injector?.useFeature(featureName, /* current variant */ variantName);')
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]["feature"], "hardwareEvent")
        self.assertIsNone(rows[1]["feature"])
        self.assertIsNone(rows[1]["variant"])
        self.assertEqual(rows[1]["arguments"], ["featureName", "variantName"])

    def test_escaped_strings_are_not_silently_treated_as_literal_selection(self):
        rows = self.inspect(r'injector.useFeature("hardware\u0045vent", "config");')
        self.assertEqual(len(rows), 1)
        self.assertIsNone(rows[0]["feature"])


class MouseRotationAuditTests(unittest.TestCase):
    def setUp(self):
        self.audit = host.HostLocalAudit()
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.root_patch = patch.object(analysis, "ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(self.root_patch.stop)

    def original(self, text):
        raw = text.encode()
        path = self.root / ".analysis/inputs/rotation" / (hashlib.sha256(raw).hexdigest()+".js")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return families.identity(path)

    def test_getter_inventory_ignores_declarations_text_but_keeps_aliases_and_computed_calls(self):
        source = self.original('const title="设备 getProximitySensorAngleTune";'
            '// device.getProximitySensorAngleTune()\n'
            'class D {getProximitySensorAngleTune(){}}'
            'device?.getProximitySensorAngleTune(1);'
            'const read=device["getProximitySensorAngleTune"];'
            'device["getMouseSensorRotationAngle"]();')
        rows = families.rotation_member_references(self.audit,[source],
            ("getProximitySensorAngleTune","getMouseSensorRotationAngle"))
        self.assertEqual([row["name"] for row in rows],
            ["getProximitySensorAngleTune","getProximitySensorAngleTune","getMouseSensorRotationAngle"])
        for row in rows:
            owner = self.audit.owners[row["owner"]]
            body = self.audit.read(source)[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(body.decode(),owner["body"])
            self.assertEqual(hashlib.sha256(body).hexdigest(),owner["body_sha256"])

    def test_factory_closure_follows_transitive_requires_and_ignores_shadowed_parameters(self):
        source = self.original('const title="设备";self.chunks.push([[1],{'
            '1:(e,t,n)=>{const helper=n(2);function f(n){return n(999)}},'
            '2(e,t,n){const parse=n(3);},3:(e)=>{e.exports=4}}]);')
        rows = families.rotation_factory_closure(self.audit,[source],[1])
        self.assertEqual([(row["module_id"],row["dependencies"]) for row in rows],
            [(1,[2]),(2,[3]),(3,[])])
        self.assertEqual(self.audit.final_snapshot(),[])

    def test_factory_closure_rejects_dynamic_missing_and_ambiguous_dependencies(self):
        source = self.original('self.chunks.push([[1],{1:(e,t,n)=>{n(dynamic)}}]);')
        with self.assertRaisesRegex(ValueError,"dynamic dependency"):
            families.rotation_factory_closure(self.audit,[source],[1])
        source = self.original('self.chunks.push([[1],{1:(e,t,n)=>{n(2)}}]);')
        with self.assertRaisesRegex(ValueError,"absent or ambiguous"):
            families.rotation_factory_closure(self.audit,[source],[1])
        duplicate = self.original('self.chunks.push([[1],{1:(e,t,n)=>{n(3)}}]);')
        with self.assertRaisesRegex(ValueError,"absent or ambiguous"):
            families.rotation_factory_closure(self.audit,[source,duplicate],[1])

    def test_ble_array_header_is_explicit_and_validated_at_its_lexical_owner(self):
        source = self.original('const command=[2,15,20];'
            'class Device {setProximitySensorAngleTune(p,s,a){return this.sendCommand(command,p,s,a)}}')
        root = self.audit.tree(source).root_node
        cls = next(node for node in root.named_children if node.type=="class_declaration")
        commands,_ = families.direct_command_headers(self.audit,source,cls,
            ("setProximitySensorAngleTune",),array_header=True)
        self.assertEqual(commands,{"setProximitySensorAngleTune":[2,15,20]})
        with self.assertRaisesRegex(ValueError,"fixed three-byte"):
            families.direct_command_headers(self.audit,source,cls,("setProximitySensorAngleTune",))
        changed = self.original('const command=[2,15,dynamic];'
            'class Device {setProximitySensorAngleTune(){return this.sendCommand(command)}}')
        cls = next(node for node in self.audit.tree(changed).root_node.named_children if node.type=="class_declaration")
        with self.assertRaisesRegex(ValueError,"fixed three-byte"):
            families.direct_command_headers(self.audit,changed,cls,("setProximitySensorAngleTune",),array_header=True)

    def test_task_maker_imports_resolve_actual_exports_instead_of_same_named_decoys(self):
        source = self.original('self.chunks.push([[1],{'
            '1:(e,t,n)=>{var tasks=n(2);function make(){tasks.run.bind(null,1);tasks.done(1)}},'
            '2:(e,t,n)=>{n.d(t,{run:()=>runner,done:()=>callback});'
            'function runner(){return "actual"}function callback(){return "requested value"}},'
            '3:(e,t,n)=>{n.d(t,{run:()=>runner});function runner(){return "decoy"}}}]);')
        owner = self.audit.retain("maker",source,"function make()",{"function_declaration"})[0]
        rows = families.rotation_imported_owners(self.audit,[source],source,owner)
        self.assertEqual({(row["export"]["module_id"],row["export"]["export"]) for row in rows},
            {(2,"run"),(2,"done")})
        bodies = {row["export"]["binding"]["body"] for row in rows}
        self.assertEqual(bodies,{'function runner(){return "actual"}',
                                'function callback(){return "requested value"}'})


class MouseIdleInitializationTests(unittest.TestCase):
    def setUp(self):
        # Load the pinned parser before replacing only the evidence workspace.
        self.audit = host.HostLocalAudit()
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.root_patch = patch.object(analysis, "ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(self.root_patch.stop)

    @staticmethod
    def source():
        return ('const unicode="设备";self.chunks.push([[6259],{'
            '50350:(e,t,n)=>{n.d(t,{pH:()=>_e});var Z=n(87887);'
            'const _e=()=>de(void 0,void 0,void 0,(function*(){'
            'const{isBattery:b}=features();let C=runtime;'
            'switch(m.A.runtimeData.setupStatus){case c.Dh.READY:{const e=[];'
            'if(b&&C.powerStatus){'
            'if(null===(y=b.yes)||void 0===y?void 0:y.alwaysReadPowerSavingFromDevice){'
            'const t=yield(0,Z.kp)();e.push({type:i.WDf,payload:t})}}'
            'const t=yield manager.getRuntimeData(e);publish(t);break}}}))},'
            '87887:(e,t,n)=>{n.d(t,{kp:()=>Z});'
            'const Z=(e=5)=>K(void 0,void 0,void 0,(function*(){try{'
            'const e=o.jf.rzDevice,{jsonData:t}=yield e.getTimeToSleep();'
            'return t.timeToSleep/60'
            '}catch(t){return console.log("getPowerSavingFromDevice",t.message),e}}))},'
            '90012:(e,t,n)=>{n.d(t,{pH:()=>_e});const _e=()=>"unrelated owner"}'
            '}]);').encode()

    def inspect(self, raw):
        path = self.root / ".analysis/inputs/test" / (hashlib.sha256(raw).hexdigest() + ".js")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        original = families.identity(path)
        result = families.mouse_idle_initialization_source(self.audit, [original])
        self.assertEqual(self.audit.final_snapshot(), [])
        return original, result

    def test_ready_export_and_lexical_import_retain_complete_original_utf8_owners(self):
        raw = self.source()
        original, result = self.inspect(raw)
        self.assertEqual(result["publisher"]["module_id"], 50350)
        self.assertEqual(result["publisher"]["binding"], "_e")
        self.assertEqual(result["getter"]["module_id"], 87887)
        self.assertEqual(result["getter"]["binding"], "Z")
        self.assertEqual(len(result["complete_owners"]), 7)
        for owner in result["complete_owners"]:
            body = raw[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(owner["original"], original)
            self.assertEqual(body.decode(), owner["body"])
            self.assertEqual(hashlib.sha256(body).hexdigest(), owner["body_sha256"])
        self.assertTrue(all(owner["body"].startswith(("50350:", "87887:", "n.d(", "_e=", "Z="))
                            for owner in result["complete_owners"]))

    def test_source_default_and_actual_failure_remain_distinct(self):
        _, result = self.inspect(self.source())
        self.assertEqual(result["getter"]["source_fallback_minutes"], 5)
        self.assertIn("publish 5 after a source getter failure", result["source_failure"])
        self.assertIn("not be projected as an actual device value", result["actual_failure"])
        self.assertNotIn("90012", result["initialization"])

    def test_wrong_factory_dynamic_export_or_shadowed_getter_import_is_rejected(self):
        for old, new in (
            (b"Z=n(87887)", b"Z=n(90012)"),
            (b"pH:()=>_e", b"pH:()=>wrap(_e)"),
            (b"const t=yield(0,Z.kp)()", b"const Z={kp:other};const t=yield(0,Z.kp)()"),
        ):
            with self.subTest(replacement=new), self.assertRaises(ValueError):
                self.inspect(self.source().replace(old, new))

    def test_ready_and_battery_feature_conditions_are_required(self):
        for old, new in (
            (b"case c.Dh.READY:", b"case c.Dh.SYNCING:"),
            (b"b&&C.powerStatus", b"b||C.powerStatus"),
            (b"alwaysReadPowerSavingFromDevice", b"alwaysReadPowerSavingFromCache"),
        ):
            with self.subTest(replacement=new), self.assertRaises(ValueError):
                self.inspect(self.source().replace(old, new))

    def test_publisher_must_await_default_getter_and_publish_that_result(self):
        for old, new in (
            (b"yield(0,Z.kp)()", b"yield(0,Z.kp)(10)"),
            (b"yield(0,Z.kp)()", b"(0,Z.kp)()"),
            (b"{type:i.WDf,payload:t}", b"{type:i.WDf,payload:5}"),
        ):
            with self.subTest(replacement=new), self.assertRaises(ValueError):
                self.inspect(self.source().replace(old, new))

    def test_getter_method_units_and_catch_parameter_are_verified(self):
        for old, new in (
            (b"Z=(e=5)", b"Z=(e=10)"),
            (b"e.getTimeToSleep()", b"e.getCachedTimeToSleep()"),
            (b"t.timeToSleep/60", b"t.timeToSleep/100"),
            (b'catch(t){return console.log', b'catch(t){const e=7;return console.log'),
            (b't.message),e}}))', b't.message),0}}))'),
        ):
            with self.subTest(replacement=new), self.assertRaises(ValueError):
                self.inspect(self.source().replace(old, new))

    def test_duplicate_getter_call_or_factory_is_not_silently_selected(self):
        for raw in (
            self.source().replace(b"const t=yield(0,Z.kp)()", b"yield(0,Z.kp)();const t=yield(0,Z.kp)()"),
            self.source().replace(b"90012:(e,t,n)", b"87887:(e,t,n)"),
        ):
            with self.subTest(source_sha256=hashlib.sha256(raw).hexdigest()), self.assertRaises(ValueError):
                self.inspect(raw)

    def test_source_identity_must_match_actual_bytes(self):
        raw = self.source()
        path = self.root / ".analysis/inputs/test/tampered.js"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        original = families.identity(path)
        path.write_bytes(raw.replace(b"getTimeToSleep", b"getCachedTimeToSleep"))
        with self.assertRaisesRegex(ValueError, "differs"):
            families.mouse_idle_initialization_source(self.audit, [original])


class MouseReceiverPowerBatchTests(unittest.TestCase):
    def setUp(self):
        self.parser = host.HostLocalAudit().parser
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.root_patch = patch.object(analysis, "ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        tool = self.root / "tools/families.py"
        tool.parent.mkdir(parents=True)
        tool.write_text("fixture source identity\n", encoding="utf-8")
        file_patch = patch.object(families, "__file__", str(tool))
        file_patch.start()
        self.addCleanup(file_patch.stop)
        self.index = []
        for pid in (11, 22):
            base = f".analysis/inputs/frontend/mouse-mw-{pid}-current"
            url = f"https://apps.razer.com/synapse/products/{pid}/mw/"
            source = self.write(base + "/main.js", self.source(pid, initialized=pid == 11))
            html = self.write(base + "/index.html", b'<html><script src="main.js"></script></html>')
            acquired = {"entry_url": url + "index.html", "source_current_during_acquisition": True,
                "target_execution": False, "files": [{**html, "url": url + "index.html", "content_type": "text/html"},
                                                       {**source, "url": url + "main.js", "content_type": "application/javascript"}]}
            self.write(base + "/acquisition.json", json.dumps(acquired).encode())
            self.write(f".analysis/results/frontend/mouse-mw-{pid}-current/analysis.json",
                       json.dumps({"target_execution": False, "input_bytes_unchanged": True,
                                   "original_inputs": [html, source], "commands": []}).encode(), indexed=False)
        self.write(".analysis/inputs/host-current/asar/fixture/electron/modules/memory_storage/index.js",
                   b'class Storage{constructor(){this.memoryStorageMap=new Map}call(action,key,value,url){'
                   b'let values=this.memoryStorageMap.get(url);switch(action){'
                   b'case"setMemoryStorageItem":{const previous=values.get(key);values.set(key,value);return previous}'
                   b'case"getMemoryStorageItem":return JSON.stringify([{value:values.get(key),windowName:url}]);'
                   b'case"clearMemoryStorage":return this.memoryStorageMap.delete(url)}}}')
        self.write(".analysis/inputs.json", json.dumps({"inputs": self.index}).encode(), indexed=False)

    def write(self, name, raw, *, indexed=True):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        item = families.identity(path)
        if indexed:
            self.index.append(item)
        return item

    @staticmethod
    def source(pid, *, initialized=True):
        initialization = ('function init(){window.taskMakerPowerSavingValueMemoryStorageCache={setPowerSavingValue:{timerTick:null,value:{}}};'
            'window.taskRunnerPowerSavingValueMemoryStorageCache={currValue:{},lastModifiedDate:null,setting:{version:null,value:{}}}}'
            if initialized else 'function init(){window.otherCache={value:7}}')
        return (f'const label="设备",cache={{runtimeData:{{productId:{pid},setupStatus:"waiting"'
            + (',hasBattery:!0' if pid == 11 else '') + '}};' + initialization +
            'Promise.resolve().then((()=>{injector.injectData("initMemoryStorageCache",init)}));'
            'const defaults=e=>{const t={};e.runtimeData.hasBattery&&(t.isBattery={yes:{alwaysReadPowerSavingFromDevice:!1}});return t};'
            'Promise.resolve().then((()=>{injector.useFeature("isBattery","yes",{alwaysReadPowerSavingFromDevice:!0})}));'
            'function maker(tick,from,value,update,access,version=0){'
            'if(update&&!feature.alwaysReadPowerSavingFromDevice){version=versions.nextVersion(version)}'
            'publish({actionType:"taskMakerSetPowerSavingValue"});'
            'if(access)queue.enqueueTask("skippable","powerSaving",runner("setting","version",version,value));'
            'cache.timerTick=tick;cache.value=value;api.setMemoryStorageItem("task",JSON.stringify(cache))}'
            'const runner=(setting,key,version,value)=>(queue,lock,token,state)=>helper(function*(){'
            'state.value={params:{powerSavingValue:value}};if(token.signal.aborted)throw new Error("canceled");'
            'let seconds=60*value;device.singleProfileDevice&&(seconds=value);'
            'const result=yield device.setTimeToSleep(seconds);if(token.signal.aborted)throw new Error("canceled");'
            'const{timeToSleep}=yield device.getTimeToSleep();if(timeToSleep!==seconds)throw new Error("data not match");'
            'state.value.status="completed";cache.currValue={powerSavingValue:value};'
            'cache.lastModifiedDate=clock.currentTimerTick?clock.currentTimerTick:Date.now();'
            'window.taskRunnerPowerSavingValueMemoryStorageCache[setting].value.powerSavingValue=value;'
            'window.taskRunnerPowerSavingValueMemoryStorageCache[setting][key]=version;'
            'yield api.setMemoryStorageItem("task",JSON.stringify(cache))});'
            'function dispatch(e){switch(e.type){case"ON_SET_POWER_SAVING_VALUE":{const{powerSavingValue:value,version:v}=e.payload;'
            'maker(e.timerTick,e.from,value,true,true,v);break}}}'
            'api.setMemoryStorageItem=(key,value)=>api.callElectronAction({action:"setMemoryStorageItem",payload:{key,value}});'
        ).encode()

    def collect(self):
        return families.collect_mouse_receiver_power_task_batch()

    def test_all_existing_endpoints_are_audited_in_one_batch_and_shared_owners_are_deduplicated(self):
        result = self.collect()
        self.assertEqual(result["selected_products"], [11, 22])
        self.assertEqual(result["summary"]["failures"], 0)
        self.assertEqual(len(result["source_facts"]), 2)
        shared = [owner for owner in result["complete_owners"] if len(owner["instances"]) == 2]
        self.assertTrue(shared)
        with patch.object(families, "collect_mouse_receiver_power_task_batch", side_effect=AssertionError("collector called")):
            self.assertEqual(families.verify_mouse_receiver_power_task_batch(result)["independent_verification"], "passed")

    def test_missing_runtime_fields_and_custom_initializer_do_not_acquire_another_products_defaults(self):
        result = self.collect()
        first, second = result["products"]
        self.assertEqual(len(first["task_chain"]["initializers"]), 2)
        self.assertEqual(second["task_chain"]["initializers"], [])
        self.assertEqual(first["runtime_cache_declarations"][0]["fields"]["hasBattery"], "!0")
        self.assertNotIn("hasBattery", second["runtime_cache_declarations"][0]["fields"])
        self.assertNotIn("device_connected", second)

    def test_runner_preserves_real_set_get_cancel_and_memory_event_order(self):
        facts = families.mouse_receiver_power_task_facts(self.source(11), self.parser)
        runner = facts["runners"][0]
        calls = [row["member"] for row in runner["events"] if row["kind"] == "call"]
        self.assertEqual(calls, ["setTimeToSleep", "getTimeToSleep", "now", "setMemoryStorageItem"])
        self.assertEqual(len(runner["readback_conditions"]), 1)
        self.assertTrue(any("singleProfileDevice" in row for row in runner["single_profile_override"]))
        self.assertEqual(len([row for row in runner["events"] if row["kind"] == "throw_statement"]), 3)

    def test_battery_default_and_explicit_startup_override_have_separate_complete_owners(self):
        raw = b'(()=>{' + self.source(11) + b'})();'
        facts = families.mouse_receiver_power_task_facts(raw, self.parser)
        self.assertEqual(len(facts["feature_defaults"]), 1)
        self.assertEqual(len(facts["feature_overrides"]), 1)
        owners = {owner["body_sha256"]: owner for owner in facts["owners"]}
        default = owners[facts["feature_defaults"][0]]
        override = owners[facts["feature_overrides"][0]["owner"]]
        self.assertTrue(default["body"].startswith('e=>{const t={};'))
        self.assertIn('alwaysReadPowerSavingFromDevice:!1', default["body"])
        self.assertIn('alwaysReadPowerSavingFromDevice:!0', override["body"])
        self.assertNotEqual(default["body_sha256"], override["body_sha256"])
        self.assertNotIn('runtime_battery_feature_defaults', override["roles"])
        self.assertTrue(all(owner["body"] != raw.decode()[0:-1] for owner in facts["owners"]))

    def test_product_source_swap_is_rejected_even_when_the_swapped_sources_have_valid_hashes(self):
        result = self.collect()
        result["products"][0]["source_facts"] = result["products"][1]["source_facts"]
        with self.assertRaisesRegex(ValueError, "current deployment"):
            families.verify_mouse_receiver_power_task_batch(result)

    def test_owner_bytes_ranges_and_ast_facts_cannot_be_replaced_by_cached_claims(self):
        original = self.collect()
        for mutate in (
            lambda value: value["complete_owners"][0].__setitem__("body", "unrelated"),
            lambda value: value["complete_owners"][0]["instances"][0].__setitem__("byte_start", 0),
            lambda value: value["source_facts"][next(iter(value["source_facts"]))]["runners"].clear(),
        ):
            value = copy.deepcopy(original)
            mutate(value)
            with self.assertRaises(ValueError):
                families.verify_mouse_receiver_power_task_batch(value)

    def test_source_verification_cannot_promote_execution_acceptance_or_host_cache_confirmation(self):
        original = self.collect()
        for field in ("target_execution", "real_device_operations", "official_dll_comparison", "rust_implementation_acceptance", "runtime_acceptance"):
            value = copy.deepcopy(original)
            value[field] = True if field in {"target_execution", "real_device_operations"} else "passed"
            with self.subTest(field=field), self.assertRaises(ValueError):
                families.verify_mouse_receiver_power_task_batch(value)
        value = copy.deepcopy(original)
        value["host_memory_storage"]["contract"]["device_confirmation"] = True
        with self.assertRaisesRegex(ValueError, "device acknowledgement"):
            families.verify_mouse_receiver_power_task_batch(value)

    def test_missing_endpoint_or_changed_original_is_rejected(self):
        original = self.collect()
        value = copy.deepcopy(original)
        value["candidate_products"].pop()
        with self.assertRaisesRegex(ValueError, "candidate"):
            families.verify_mouse_receiver_power_task_batch(value)
        source = next(iter(original["source_instances"].values()))[0]
        (self.root / source["path"]).write_bytes(b"const changed=true;")
        with self.assertRaisesRegex(ValueError, "differs"):
            families.verify_mouse_receiver_power_task_batch(original)

    def test_failed_capture_and_invalid_selected_product_are_not_silently_successful(self):
        path = self.root / ".analysis/results/frontend/mouse-mw-11-current/analysis.json"
        captured = json.loads(path.read_bytes())
        captured["input_bytes_unchanged"] = False
        path.write_text(json.dumps(captured), encoding="utf-8")
        result = self.collect()
        self.assertEqual(result["summary"]["failures"], 1)
        with self.assertRaises(ValueError):
            families.verify_mouse_receiver_power_task_batch(result)
        with self.assertRaisesRegex(ValueError, "existing current"):
            families.collect_mouse_receiver_power_task_batch([99])


class MatcherNumericReviewTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not (analysis.ROOT / families.MATCHER_NUMERIC_REVALIDATION["path"]).is_file():
            raise unittest.SkipTest("Retained Matcher deployment comparison is unavailable")
        cls.review = families.collect_mouse_matcher_numeric_review()

    def test_complete_numeric_review_reuses_seven_factory_references_without_copied_js(self):
        snapshot = families.verify_mouse_matcher_numeric_review(copy.deepcopy(self.review))
        self.assertEqual(len(self.review["products"]), 7)
        self.assertEqual(len(snapshot["indexed_originals"]), 14)
        self.assertTrue(all("body" not in row.get("factory", {}) for row in self.review["products"]))
        self.assertNotIn("complete_owners", self.review)
        self.assertTrue(all("matcher_unary_tick" in row["consumers"] for row in self.review["products"][:6]))
        self.assertNotIn("matcher_unary_tick", self.review["products"][-1]["consumers"])
        self.assertFalse(self.review["source_semantics_complete"])

    def test_numeric_review_rejects_false_scope_semantics_and_completion_claims(self):
        mutations = [
            lambda r: r.update(target_execution=True),
            lambda r: r.update(whole_page_complete=True),
            lambda r: r.update(source_semantics_complete=True),
            lambda r: r.update(actual_dll_comparison="verified"),
            lambda r: r.update(runtime_acceptance="verified"),
            lambda r: r.update(rust_validation="passed"),
            lambda r: r["source_review"].update(sha256="0" * 64),
            lambda r: r["products"].pop(),
            lambda r: r["products"].append(copy.deepcopy(r["products"][0])),
            lambda r: r["semantics"].update(finite_nonnegative_tick_guard=True),
            lambda r: r["semantics"].update(matcher_tick="rust_float_parse"),
            lambda r: r["semantics"].update(dynamic_tick_comparison="always_number"),
            lambda r: r["semantics"]["baseline"].update(device_GET=True),
            lambda r: r["remaining_local_chains"].clear(),
            lambda r: r["rust_inputs"].pop(),
            lambda r: r.update(complete_owners={}),
        ]
        for index, mutate in enumerate(mutations):
            review = copy.deepcopy(self.review)
            mutate(review)
            with self.subTest(case=index), self.assertRaises(ValueError):
                families.verify_mouse_matcher_numeric_review(review)

    def test_numeric_review_rejects_stale_product_owner_hash_range_and_consumer_references(self):
        mutations = [
            lambda r: r["products"][0]["factory_reference"].update(owner=r["products"][1]["factory_reference"]["owner"]),
            lambda r: r["products"][0].update(matcher=False),
            lambda r: r["products"][0]["acquisition"].update(sha256="0" * 64),
            lambda r: r["products"][0]["consumers"]["matcher_unary_tick"].update(byte_start=0),
            lambda r: r["products"][0]["consumers"]["dynamic_comparison"].update(syntax="unary_expression"),
            lambda r: r["products"][-1]["factory"].update(byte_end=1146580),
            lambda r: r["products"][-1]["factory"].update(body_sha256="0" * 64),
            lambda r: r["products"][-1]["factory"].update(syntax_sha256="0" * 64),
            lambda r: r["products"][-1]["factory"].update(body="copied source"),
            lambda r: r["products"][-1]["factory"]["original"].update(bytes=1),
        ]
        for index, mutate in enumerate(mutations):
            review = copy.deepcopy(self.review)
            mutate(review)
            with self.subTest(case=index), self.assertRaises(ValueError):
                families.verify_mouse_matcher_numeric_review(review)

    def test_numeric_ast_requires_a_complete_factory_and_the_actual_consumer_structure(self):
        import frontend
        for raw in (b"({7:()=>{const tick=+e.timeTick;}})", b"({7:()=>{const tick=+e.timeTick;}}) broken("):
            with self.subTest(raw=raw), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                path = root / ".analysis/inputs/frontend/fixture/original.js"
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(raw)
                audit = host.HostLocalAudit()
                original = families.identity(path)
                tree = audit.parser.parse(raw)
                node = tree.root_node.descendant_for_byte_range(raw.index(b"7:"), raw.index(b"}}") + 1)
                owner = {"original": original, "byte_start": node.start_byte, "byte_end": node.end_byte,
                    "syntax": node.type, "module_id": 7, "body_sha256": hashlib.sha256(node.text).hexdigest(),
                    "syntax_sha256": frontend.syntax_identity(node)}
                with self.assertRaises(ValueError):
                    families.matcher_numeric_ast(audit, owner, matcher=True)

    @staticmethod
    def freeze(root, review):
        import os
        raw = (json.dumps(review, ensure_ascii=False, indent=2) + "\n").replace("\n", os.linesep).encode()
        digest = hashlib.sha256(raw).hexdigest()
        path = root / ".analysis/results/reverse-pending/review-sources" / (digest + ".json")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return families.identity(path)

    def publication_fixture(self, root):
        original = root / ".analysis/inputs/frontend/official.js"
        original.parent.mkdir(parents=True, exist_ok=True)
        original.write_bytes(b"official source fixture")
        source = families.identity(original)
        analysis.save(root / ".analysis/inputs.json", {"inputs": [source]})
        snapshot = {"artifacts": [source], "indexed_originals": [source]}
        review = copy.deepcopy(self.review)
        reference = self.freeze(root, review)
        canonical = root / ".analysis/results/frontend/mouse-mw-audit.json"
        document = {"queries": {"dpi_matcher": {"old": "retained"}, "dynamic_sensitivity": {"other": [196]}},
            "complete_owners": {"existing": "retained"}, "sibling": {"value": 17}}
        analysis.save(canonical, document)
        return review, reference, canonical, document, snapshot

    @staticmethod
    def publication_args(reference=None, previous=None):
        from types import SimpleNamespace
        return SimpleNamespace(family="mouse", function="mw", query="mouse-dpi-matcher", product=None,
            prepare=False, receipt_only=False, matcher_numeric_boundary="capture",
            semantic_review=reference["path"] if reference else None,
            semantic_review_sha256=reference["sha256"] if reference else None, semantic_previous_review=previous)

    def test_numeric_publication_is_idempotent_and_keeps_sibling_and_historical_reviews(self):
        import contextlib
        import io
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            review, reference, canonical, original, snapshot = self.publication_fixture(root)
            with patch.object(families, "verify_mouse_matcher_numeric_review", return_value=snapshot), \
                    contextlib.redirect_stdout(io.StringIO()):
                families.publish_frontend_semantic_review(self.publication_args(reference))
                published = json.loads(canonical.read_bytes())
                expected = copy.deepcopy(original)
                for name in ("dpi_matcher", "dynamic_sensitivity"):
                    expected["queries"][name]["numeric_boundary_review"] = reference
                self.assertEqual(published, expected)
                before = canonical.read_bytes()
                with patch.object(families, "collect_mouse_matcher_numeric_review", return_value=copy.deepcopy(review)):
                    repeated = families.audit_mouse_matcher_numeric_boundary(self.publication_args())
                self.assertEqual(repeated["review_source"], reference)
                self.assertEqual(canonical.read_bytes(), before)
                self.assertEqual(len(list((root / reference["path"]).parent.iterdir())), 1)
                self.assertFalse(list(root.rglob("*.tmp")))

    def test_numeric_merge_requires_both_observed_references_and_the_frozen_predecessor(self):
        reference = {"path": "review", "bytes": 1, "sha256": "a" * 64}
        replacement = {"path": "next", "bytes": 2, "sha256": "b" * 64}
        document = {"queries": {name: {"numeric_boundary_review": reference} for name in ("dpi_matcher", "dynamic_sensitivity")}}
        review = {**copy.deepcopy(self.review), "previous_review_source": reference}
        for mode in ("missing_expected", "wrong_expected", "missing_predecessor", "removed", "one_query_changed", "false_guard"):
            candidate, current = copy.deepcopy(review), copy.deepcopy(document)
            expected = reference
            if mode == "missing_expected":
                expected = None
            elif mode == "wrong_expected":
                expected = replacement
            elif mode == "missing_predecessor":
                candidate.pop("previous_review_source")
            elif mode == "removed":
                for query in current["queries"].values():
                    query.pop("numeric_boundary_review")
            elif mode == "one_query_changed":
                current["queries"]["dynamic_sensitivity"]["numeric_boundary_review"] = replacement
            else:
                candidate["semantics"]["finite_nonnegative_tick_guard"] = True
            before = copy.deepcopy(current)
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                families.merge_frontend_semantic_review(current, candidate, replacement, expected_previous_review=expected)
            self.assertEqual(current, before)
        families.merge_frontend_semantic_review(document, review, replacement, expected_previous_review=reference)
        self.assertTrue(all(query["numeric_boundary_review"] == replacement for query in document["queries"].values()))

    def test_numeric_publisher_rejects_input_index_review_and_canonical_mutation_without_overwrite(self):
        import contextlib
        import io
        for mode in ("source", "index", "candidate", "canonical", "prior"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                review, prior, canonical, document, snapshot = self.publication_fixture(root)
                for query in document["queries"].values():
                    query["numeric_boundary_review"] = prior
                analysis.save(canonical, document)
                review["previous_review_source"] = prior
                reference = self.freeze(root, review)
                before = canonical.read_bytes()
                merge = families.merge_frontend_semantic_review
                concurrent = []
                def mutate(*args, **kwargs):
                    result = merge(*args, **kwargs)
                    if mode == "source":
                        (root / snapshot["artifacts"][0]["path"]).write_bytes(b"changed")
                    elif mode == "index":
                        analysis.save(root / ".analysis/inputs.json", {"inputs": []})
                    elif mode == "candidate":
                        (root / reference["path"]).write_bytes(b"changed")
                    elif mode == "prior":
                        (root / prior["path"]).write_bytes(b"changed")
                    else:
                        changed = {**document, "sibling": {"value": 18}}
                        analysis.save(canonical, changed)
                    concurrent.append(canonical.read_bytes())
                    return result
                with patch.object(families, "verify_mouse_matcher_numeric_review", return_value=snapshot), \
                        patch.object(families, "merge_frontend_semantic_review", side_effect=mutate), \
                        contextlib.redirect_stdout(io.StringIO()), self.assertRaises(ValueError):
                    families.publish_frontend_semantic_review(self.publication_args(reference, prior))
                self.assertEqual(canonical.read_bytes(), concurrent[0])
                if mode != "canonical":
                    self.assertEqual(canonical.read_bytes(), before)
                self.assertFalse(list(root.rglob("*.tmp")))

    def test_numeric_capture_preserves_concurrent_sibling_and_rejects_removed_or_changed_previous(self):
        import contextlib
        import io
        for mode in ("sibling", "removed", "changed_bytes", "newer_review"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                review, prior, canonical, document, snapshot = self.publication_fixture(root)
                for query in document["queries"].values():
                    query["numeric_boundary_review"] = prior
                analysis.save(canonical, document)
                candidate = copy.deepcopy(review)
                candidate["rust_inputs"][0]["sha256"] = "f" * 64
                concurrent = []
                def collect():
                    changed = copy.deepcopy(document)
                    if mode == "sibling":
                        changed["sibling"]["value"] = 18
                    elif mode == "removed":
                        for query in changed["queries"].values():
                            query.pop("numeric_boundary_review")
                    elif mode == "changed_bytes":
                        (root / prior["path"]).write_bytes(b"changed")
                    else:
                        newer = {**copy.deepcopy(review), "previous_review_source": prior}
                        reference = self.freeze(root, newer)
                        for query in changed["queries"].values():
                            query["numeric_boundary_review"] = reference
                    analysis.save(canonical, changed)
                    concurrent.append(canonical.read_bytes())
                    return candidate
                with patch.object(families, "collect_mouse_matcher_numeric_review", side_effect=collect), \
                        patch.object(families, "verify_mouse_matcher_numeric_review", return_value=snapshot), \
                        contextlib.redirect_stdout(io.StringIO()):
                    if mode == "sibling":
                        families.audit_mouse_matcher_numeric_boundary(self.publication_args())
                        self.assertEqual(json.loads(canonical.read_bytes())["sibling"]["value"], 18)
                    else:
                        with self.assertRaises(ValueError):
                            families.audit_mouse_matcher_numeric_boundary(self.publication_args())
                        self.assertEqual(canonical.read_bytes(), concurrent[0])
                self.assertFalse(list(root.rglob("*.tmp")))


class FrontendSemanticMergeTests(unittest.TestCase):
    def receiver_mapping_fixture(self):
        reference = {"path": "review.json", "bytes": 10, "sha256": "a" * 64}
        base = {"path": "base.json", "bytes": 20, "sha256": "b" * 64}
        original = {"path": "official.js", "bytes": 1, "sha256": "c" * 64}
        owner = {"original": original, "byte_start": 0, "byte_end": 1, "body": "x", "body_sha256": "d" * 64}
        required = {"PreSetLocalStorage", "PostSetLocalStorage", "publisher", "merger", "localStorageSetItem",
            "callMappingEngineAction", "isElectronApp", "initialization_owner", "derived_feature_owner", "default_module_owner",
            "initial_document_factory", "runtime_identity_parameters"}
        observed = {"slave_storage_changed", "storage_listener_registration", "slave_storage_observer_startup",
            "serial_window_storage_listener", "serial_ready_republish"}
        products, reviewed = [], []
        for pid in families.LEGACY_RECEIVER_MAPPING_PRODUCTS:
            identity = {"productId": pid, "dongleId": pid + 1, "bleId": 213 if pid == 211 else None}
            branches = [{"physical_product_id": pid, "parser": "PROTOCOL25"}]
            dual = pid in {192, 211, 222}
            runtime = {"identity": identity, "physical_branches": branches,
                "gates": {"isDualLinkDevice": {"value": dual}, "isMikaDevice": {"value": False}}}
            publication = {"storage_hooks": {field: "owner" for field in ("PreSetLocalStorage", "PostSetLocalStorage")},
                "post_storage_callers": [{"complete_owner": "owner"}], "semantics": {"publication": "previous"}}
            products.append({"product_id": pid, "runtime_acceptance": "unverified", "remaining": "preserve",
                "duallink_protocol": {"runtime_selection": runtime, "event_selection_and_publication": publication}})
            reviewed.append({"product_id": pid, "runtime_identity": identity, "physical_branches": branches,
                "base_selection_review": base, "target_execution": False, "whole_page_complete": False,
                "source_branch_selection_complete": True, "runtime_acceptance": "unverified", "remaining_local_chains": ["executors"],
                "suppression_gate": {"feature": "controllerDevice", "active_at_current_source_startup": False, "owner": "owner"},
                "hash": {"implementation_owner": "owner"}, "chain_owners": {field: "owner" for field in required},
                "slave_storage_observers": {field: "owner" for field in observed},
                "master_observer_feature_gate": dual, "mika_observer_feature_gate": False,
                "wireless_physical_ids": [value for field in ("dongleId", "bleId") if (value := identity[field])],
                "initial_descriptor": {"owner": "owner", "version": 1, "vendorId": 5426, "productId": pid,
                    "category": "MOUSE", "reportIDs": {"4": "razerKeyReportID", "5": "hardwareEventReportID"}},
                "master_observer_callers": [{"owner": "owner", "gates": [{"owner": "owner"}, {"owner": "owner"}]}],
                "change_gate": {"report_ids_independent_trigger": False, "storage_boolean_result_consumed": False,
                    "cache_rollback_on_storage_failure": False, "cache_updated_before_storage_await": True},
                "semantics": {"publisher": "controllerDevice is the actual gate"}})
        document = {"receiver_legacy": {"runtime_selection_review": base, "products": products,
            "complete_owners": {"owner": copy.deepcopy(owner)}, "host_native": {"preserve": True},
            "generated_descriptors": [{"path": "current.rs"}]}, "sibling_query": {"preserve": True}}
        review = {"kind": "current_legacy_receiver_mapping_descriptor_review", "products": reviewed,
            "base_selection_review": base, "host_descriptor_review": copy.deepcopy(families.MAPPING_DESCRIPTOR_HOST_REVIEW),
            "host_native_original": {"sha256": "e" * 64}, "host_sources": {}, "host_owners": {}, "source_sets": [],
            "complete_owners": {"owner": owner}, "target_execution": False, "whole_page_complete": False,
            "actual_dll_comparison": "unverified", "independent_execution_connected": False}
        return document, review, reference

    def test_receiver_mapping_increment_preserves_siblings_and_completion_boundaries(self):
        document, review, reference = self.receiver_mapping_fixture()
        original = copy.deepcopy(document)
        result = families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(result["products"], [158, 192, 194, 211, 222])
        for key in ("host_native", "generated_descriptors", "runtime_selection_review", "complete_owners"):
            self.assertEqual(document["receiver_legacy"][key], original["receiver_legacy"][key])
        self.assertEqual(document["sibling_query"], original["sibling_query"])
        contract = document["receiver_legacy"]["mapping_descriptor_contract"]
        for field in ("runtime_acceptance", "rust_implementation", "ui_backend_wiring", "actual_dll_comparison"):
            self.assertEqual(contract[field], "unverified")
        self.assertFalse(contract["whole_page_complete"])
        self.assertFalse(contract["target_execution"])
        for row in document["receiver_legacy"]["products"]:
            self.assertEqual(row["remaining"], "preserve")
            self.assertEqual(row["duallink_protocol"]["event_selection_and_publication"]["semantics"]["publication"],
                             "controllerDevice is the actual gate")
        first = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document, first)

    def test_receiver_mapping_rejects_stale_identity_partial_join_and_false_verification(self):
        mutations = [
            lambda r: r["products"].pop(),
            lambda r: r["products"].append(copy.deepcopy(r["products"][0])),
            lambda r: r["base_selection_review"].update(sha256="0" * 64),
            lambda r: r["host_descriptor_review"].update(sha256="0" * 64),
            lambda r: r.update(target_execution=True),
            lambda r: r.update(whole_page_complete=True),
            lambda r: r.update(actual_dll_comparison="verified"),
            lambda r: r.update(independent_execution_connected=True),
            lambda r: r["products"][0]["runtime_identity"].update(productId=159),
            lambda r: r["products"][0]["physical_branches"][0].update(parser="PROTOCOL30"),
            lambda r: r["products"][0]["suppression_gate"].update(feature="recording"),
            lambda r: r["products"][0]["suppression_gate"].update(active_at_current_source_startup=True),
            lambda r: r["products"][0]["change_gate"].update(report_ids_independent_trigger=True),
            lambda r: r["products"][0]["change_gate"].update(cache_updated_before_storage_await=False),
            lambda r: r["products"][0]["change_gate"].update(storage_boolean_result_consumed=True),
            lambda r: r["products"][0]["change_gate"].update(cache_rollback_on_storage_failure=True),
            lambda r: r["products"][0]["slave_storage_observers"].pop("serial_ready_republish"),
            lambda r: r["products"][0]["chain_owners"].pop("runtime_identity_parameters"),
            lambda r: r["products"][0].update(master_observer_feature_gate=True),
            lambda r: r["products"][0].update(mika_observer_feature_gate=True),
            lambda r: r["products"][0].update(wireless_physical_ids=[158]),
            lambda r: r["products"][0]["initial_descriptor"]["reportIDs"].pop("5"),
            lambda r: r["products"][0]["master_observer_callers"][0]["gates"].pop(),
            lambda r: r["products"][0]["hash"].update(implementation_owner="missing"),
        ]
        for index, mutate in enumerate(mutations):
            with self.subTest(case=index):
                document, review, reference = self.receiver_mapping_fixture()
                # Input identity sharing is convenient for fixtures; mutation must
                # affect the incoming review alone so stale joins remain observable.
                review = copy.deepcopy(review)
                mutate(review)
                with self.assertRaises(ValueError):
                    families.merge_frontend_semantic_review(document, review, reference)

    def test_receiver_mapping_rejects_conflicting_owner_or_newer_review(self):
        document, review, reference = self.receiver_mapping_fixture()
        document["receiver_legacy"]["complete_owners"]["owner"]["body"] = "different"
        with self.assertRaisesRegex(ValueError, "retained original owner"):
            families.merge_frontend_semantic_review(document, review, reference)
        document, review, reference = self.receiver_mapping_fixture()
        document["receiver_legacy"]["mapping_descriptor_contract"] = {"review_source": {"sha256": "0" * 64}}
        with self.assertRaisesRegex(ValueError, "newer retained review"):
            families.merge_frontend_semantic_review(document, review, reference)

    def test_receiver_mapping_replacement_requires_the_observed_previous_reference(self):
        document, review, reference = self.receiver_mapping_fixture()
        prior = {"path": "previous.json", "bytes": 30, "sha256": "f" * 64}
        document["receiver_legacy"]["mapping_descriptor_contract"] = {"review_source": copy.deepcopy(prior)}
        original = copy.deepcopy(document)
        with self.assertRaisesRegex(ValueError, "newer retained review"):
            families.merge_frontend_semantic_review(document, review, reference,
                expected_previous_review={"path": "stale.json", "bytes": 30, "sha256": "0" * 64})
        self.assertEqual(document, original)
        families.merge_frontend_semantic_review(document, review, reference, expected_previous_review=prior)
        self.assertEqual(document["receiver_legacy"]["mapping_descriptor_contract"]["review_source"], reference)

    def camera_fixture(self):
        reference = {"path": "review.json", "bytes": 10, "sha256": "a" * 64}
        document = {"products": [{"product_id": pid, "failure": "retained"} for pid in (3594, 3595, 3596)],
            "complete_owners": {"new_camera": {"body": "complete"}},
            "source_review": {"runtime_acceptance": "unverified", "products": [3594, 3595, 3596]}}
        products = []
        required = ("selected_adapter_loader", "adapter_init", "adapter_terminate", "read_standard_settings",
                    "task_focus", "task_image", "camera_dispatch", "profile_camera_restore", "ui_mounted_product")
        for pid in families.CAMERA_LEGACY_PRODUCTS:
            products.append({"product_id": pid, "selection": {"device_type": families.CAMERA_LEGACY_TYPES[pid],
                "dll": {"name": "RzNativeDLL" + str(pid), "onDllLoaded": "startCamera"},
                "camera_config": {"isAdvancedCamera": True} if pid == 3592 else {}},
                "ui_gates": {"supports_hdr": pid == 3589, "supports_field_of_view": pid == 3589,
                    "advanced_pages": pid == 3592},
                "chain_owners": {name: "owner" for name in required},
                "adapter_members": {"init": {"owner": "owner"}},
                "source_branch_selection_complete": True, "target_execution": False, "whole_page_complete": False,
                "remaining_local_chains": ["Rust/device comparison and exact UI acceptance remain unverified"],
                "native": {"resource": {"resourceName": "RzNativeDLL" + str(pid), "sha256": "B" * 64, "size": 100},
                           "original": {"sha256": "b" * 64, "bytes": 100}}})
        review = {"kind": "current_legacy_camera_selection_review", "products": products,
            "complete_owners": {"owner": {}}, "source_sets": [], "verified_artifacts": [],
            "target_execution": False, "whole_page_complete": False}
        return document, review, reference

    def test_legacy_camera_increment_preserves_newer_camera_audits_and_runtime_boundaries(self):
        document, review, reference = self.camera_fixture()
        before = copy.deepcopy(document)
        result = families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(result["products"], [3587, 3589, 3590, 3592])
        for key in before:
            self.assertEqual(document[key], before[key])
        selected = document["legacy_selection"]
        self.assertFalse(selected["whole_page_complete"])
        self.assertEqual(selected["official_dll_comparison"], "not_executed")
        self.assertEqual(selected["rust_implementation"], "unverified")
        self.assertEqual(selected["ui_backend_wiring"], "unverified")
        self.assertEqual(selected["runtime_acceptance"], "unverified")
        first = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document, first)
        selected["native_property_contract"] = {"status": "retained", "review_source": {"sha256": "d" * 64}}
        document["legacy_selection"]["native_property_contract"] = copy.deepcopy(selected["native_property_contract"])
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document["legacy_selection"]["native_property_contract"], selected["native_property_contract"])

    def native_camera_fixture(self):
        document, base_review, base_reference = self.camera_fixture()
        for product in base_review["products"]:
            product["native"]["resource"]["resourceVersion"] = "current"
        families.merge_frontend_semantic_review(document, base_review, base_reference)
        names = {prefix + name for prefix in ("Get", "Set") for name in
                 ("Brightness", "CameraFocus", "Contrast", "Saturation", "WhiteBalance")}
        functions = {name: {"input_chunks_verified": True} for name in names | {
            "container_lookup", "video_get", "video_set", "camera_get", "camera_set",
            "async_camera_set", "async_camera_get", "async_video_set", "async_video_get"}}
        products = []
        for row in base_review["products"]:
            pid = row["product_id"]
            selected_functions = copy.deepcopy(functions)
            for i in range((27 if pid == 3592 else 25) - len(selected_functions)):
                selected_functions["callback_" + str(i)] = {"input_chunks_verified": True}
            products.append({"product_id": pid, "original": copy.deepcopy(row["native"]["original"]),
                "resource_version": "current", "base_selection_review": base_reference,
                "target_execution": False, "whole_native_semantics": "unverified", "official_dll_comparison": "not_executed",
                "actual_exports": [{"name": name} for name in sorted(names)],
                "actual_callback_slots": [{}] * (8 if pid == 3592 else 4), "functions": selected_functions,
                "interfaces": [{"interface": name, "actual_pe_bytes_verified": True} for name in
                               ("IAMCameraControl", "IAMVideoProcAmp")]})
        review = {"kind": "current_legacy_camera_native_property_review", "base_selection_review": base_reference,
            "products": products, "scope": ["Brightness", "CameraFocus", "Contrast", "Saturation", "WhiteBalance"],
            "complete_owners": {}, "target_execution": False, "whole_page_complete": False}
        reference = {"path": "native-review.json", "sha256": "e" * 64, "bytes": 100}
        return document, review, reference

    def test_native_camera_property_merge_keeps_complete_frontend_review_and_separate_completion_dimensions(self):
        document, review, reference = self.native_camera_fixture()
        before = copy.deepcopy(document)
        result = families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(result["complete_native_functions"], 102)
        for key in ("products", "complete_owners", "source_review"):
            self.assertEqual(document[key], before[key])
        for key in before["legacy_selection"]:
            self.assertEqual(document["legacy_selection"][key], before["legacy_selection"][key])
        section = document["legacy_selection"]["native_property_contract"]
        self.assertEqual(section["official_dll_comparison"], "not_executed")
        self.assertEqual(section["rust_implementation"], "unverified")
        self.assertFalse(section["whole_page_complete"])
        first = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document, first)

    def test_native_camera_property_scope_originals_slots_exports_and_completeness_are_validated_before_mutation(self):
        for mode in ("product", "base", "scope", "original", "version", "row_base", "runtime", "whole_native",
                     "comparison", "function", "chunks", "slot", "export", "interface", "guid", "newer_review"):
            document, review, reference = self.native_camera_fixture()
            row = review["products"][0]
            if mode == "product":
                review["products"].pop()
            elif mode == "base":
                review["base_selection_review"] = {"sha256": "c" * 64}
            elif mode == "scope":
                review["scope"].pop()
            elif mode == "original":
                row["original"]["sha256"] = "c" * 64
            elif mode == "version":
                row["resource_version"] = "other"
            elif mode == "row_base":
                row["base_selection_review"] = {"sha256": "c" * 64}
            elif mode == "runtime":
                row["target_execution"] = True
            elif mode == "whole_native":
                row["whole_native_semantics"] = "complete"
            elif mode == "comparison":
                row["official_dll_comparison"] = "passed"
            elif mode == "function":
                row["functions"].pop("camera_get")
            elif mode == "chunks":
                row["functions"]["camera_get"]["input_chunks_verified"] = False
            elif mode == "slot":
                row["actual_callback_slots"].pop()
            elif mode == "export":
                row["actual_exports"][0]["name"] = "OtherExport"
            elif mode == "interface":
                row["interfaces"].pop()
            elif mode == "guid":
                row["interfaces"][0]["actual_pe_bytes_verified"] = False
            else:
                document["legacy_selection"]["native_property_contract"] = {"review_source": {"sha256": "c" * 64}}
            before = copy.deepcopy(document)
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                families.merge_frontend_semantic_review(document, review, reference)
            self.assertEqual(document, before)

    def test_legacy_camera_rejects_cross_product_native_or_ui_selection_and_partial_owner_sets_before_mutation(self):
        for mode in ("product", "duplicate", "type", "dll", "advanced", "hdr", "fov", "owner", "missing_chain",
                     "native_name", "native_sha", "native_bytes", "row_runtime", "row_page", "global_runtime",
                     "global_page", "no_remaining", "newer_review"):
            document, review, reference = self.camera_fixture()
            row = review["products"][0]
            if mode == "product":
                review["products"].pop()
            elif mode == "duplicate":
                review["products"].append(copy.deepcopy(row))
            elif mode == "type":
                row["selection"]["device_type"] = "rzCamKiyoProUltra"
            elif mode == "dll":
                row["selection"]["dll"]["name"] = "RzNativeDLL3592"
            elif mode == "advanced":
                row["selection"]["camera_config"]["isAdvancedCamera"] = True
            elif mode in ("hdr", "fov"):
                row["ui_gates"]["supports_hdr" if mode == "hdr" else "supports_field_of_view"] = True
            elif mode == "owner":
                review["complete_owners"].clear()
            elif mode == "missing_chain":
                row["chain_owners"].pop("task_focus")
            elif mode == "native_name":
                row["native"]["resource"]["resourceName"] = "RzNativeDLL3592"
            elif mode == "native_sha":
                row["native"]["original"]["sha256"] = "c" * 64
            elif mode == "native_bytes":
                row["native"]["original"]["bytes"] = 99
            elif mode == "row_runtime":
                row["target_execution"] = True
            elif mode == "row_page":
                row["whole_page_complete"] = True
            elif mode == "global_runtime":
                review["target_execution"] = True
            elif mode == "global_page":
                review["whole_page_complete"] = True
            elif mode == "no_remaining":
                row["remaining_local_chains"].clear()
            else:
                document["legacy_selection"] = {"review_source": {"sha256": "c" * 64}}
            before = copy.deepcopy(document)
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                families.merge_frontend_semantic_review(document, review, reference)
            self.assertEqual(document, before)

    def gamepad_fixture(self):
        reference = {"path": "review.json", "bytes": 10, "sha256": "a" * 64}
        previous = "Current2676 shared notification/OBM calls into extended-static helper require full indirect dispatch resolution despite the unmounted Lighting page; presence of helper bodies is not a capability proof"
        products, reviewed = [], []
        for pid in families.GAMEPAD_LIGHTING_SELECTION_PRODUCTS:
            products.append({"product_id": pid, "unresolved_chains": [previous, "independent runtime requirement"],
                "source_chain_complete": False, "whole_page_complete": False, "runtime_acceptance": "unverified"})
            lighting = pid in {2636, 4115}
            reviewed.append({"product_id": pid, "identity": {"productId": pid},
                "source_branch_selection_complete": True, "target_execution": False,
                "remaining_local_chains": ["independent runtime requirement"],
                "controller_config": {"supportsLighting": lighting}, "ui_lighting_mounted": lighting,
                "default_profile_fields": {"controllerLightingEffects": "{}"} if pid == 2636 else {},
                "obm": {"reads_controller_lighting": pid == 2636, "semantics": "actual fields only"},
                "notification_resolution": "actual parser event categories", "bootstrap_owner": "owner",
                "chain_owners": {"11365": "owner"}, "ui_mount_owners": ["owner"]})
        document = {"sibling_audit": {"status": "retained"}, "gamepad_lighting": {
            "products": products, "native_engine": {"status": "retained"},
            "complete_module_bodies": {"existing": "complete"}, "runtime_acceptance": "unverified"}}
        review = {"kind": "current_gamepad_lighting_selection_review", "complete_owners": {"owner": {}},
            "products": reviewed, "source_sets": [], "semantics": {}, "verified_artifacts": []}
        return document, review, reference

    def test_gamepad_increment_closes_only_selected_source_gap_and_keeps_native_runtime_boundaries(self):
        document, review, reference = self.gamepad_fixture()
        before = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, review, reference)
        section = document["gamepad_lighting"]
        self.assertEqual(document["sibling_audit"], before["sibling_audit"])
        for field in ("native_engine", "complete_module_bodies", "runtime_acceptance"):
            self.assertEqual(section[field], before["gamepad_lighting"][field])
        for original, product in zip(before["gamepad_lighting"]["products"], section["products"]):
            self.assertEqual(product["unresolved_chains"], ["independent runtime requirement"]
                             if product["product_id"] == 2676 else original["unresolved_chains"])
            self.assertFalse(product["source_chain_complete"])
            self.assertFalse(product["whole_page_complete"])
            self.assertEqual(product["runtime_acceptance"], "unverified")
        merged = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document, merged)

    def test_gamepad_missing_product_owner_conflicting_gate_or_newer_review_fails_before_mutation(self):
        for mode in ("product", "owner", "lighting", "profile", "identity", "newer_review", "product_review", "source_review"):
            document, review, reference = self.gamepad_fixture()
            if mode == "product":
                review["products"].pop()
            elif mode == "owner":
                review["complete_owners"].clear()
            elif mode == "lighting":
                review["products"][0]["ui_lighting_mounted"] = True
            elif mode == "profile":
                review["products"][0]["obm"]["reads_controller_lighting"] = True
            elif mode == "identity":
                review["products"][0]["identity"]["productId"] = 999
            elif mode == "product_review":
                document["gamepad_lighting"]["products"][0]["branch_selection"] = {"review_source": {"sha256": "b" * 64}}
            elif mode == "source_review":
                document["gamepad_lighting"]["products"][0]["source_review"] = {"review_source": {"sha256": "b" * 64}}
            else:
                document["gamepad_lighting"]["branch_selection"] = {"review_source": {"sha256": "b" * 64}}
            before = copy.deepcopy(document)
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                families.merge_frontend_semantic_review(document, review, reference)
            self.assertEqual(document, before)

    def test_gamepad_replacement_requires_the_exact_observed_previous_review(self):
        document, review, reference = self.gamepad_fixture()
        families.merge_frontend_semantic_review(document, review, reference)
        prior = copy.deepcopy(reference)
        replacement = {"path": "replacement.json", "bytes": 20, "sha256": "b" * 64}
        next_review = copy.deepcopy(review)
        next_review["previous_review_source"] = prior
        original = copy.deepcopy(document)
        for expected in (None, {**prior, "bytes": 11}):
            with self.subTest(expected=expected), self.assertRaisesRegex(ValueError, "newer retained review"):
                families.merge_frontend_semantic_review(document, next_review, replacement,
                    expected_previous_review=expected)
            self.assertEqual(document, original)
        for predecessor in (None, {**prior, "bytes": 11}):
            bad = {**next_review, "previous_review_source": predecessor}
            with self.subTest(predecessor=predecessor), self.assertRaisesRegex(ValueError, "newer retained review"):
                families.merge_frontend_semantic_review(document, bad, replacement, expected_previous_review=prior)
            self.assertEqual(document, original)
        families.merge_frontend_semantic_review(document, next_review, replacement, expected_previous_review=prior)
        selected = document["gamepad_lighting"]["branch_selection"]
        self.assertEqual(selected["review_source"], replacement)
        self.assertEqual(selected["previous_review_source"], prior)
        self.assertTrue(all(row["branch_selection"]["review_source"] == replacement
                            for row in document["gamepad_lighting"]["products"]))
        merged = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, next_review, replacement, expected_previous_review=prior)
        self.assertEqual(document, merged)

    def test_gamepad_rebuild_keeps_successful_selection_and_other_products(self):
        document, review, reference = self.gamepad_fixture()
        families.merge_frontend_semantic_review(document, review, reference)
        before = copy.deepcopy(document)
        result = {"products": [{"product_id": 4115, "source_sets": ["current middleware"],
            "failures": [{"stage": "capture", "error": "unavailable"}], "unresolved_chains": ["current source gap"]},
            {"product_id": 2676, "source_review": {"mount": "current mount", "notification_branch": "indirect branch"}}],
            "complete_module_bodies": {"current": "complete"}, "complete_callable_bodies": {},
            "runtime_acceptance": "unverified"}
        families.merge_gamepad_lighting_query(document, result)
        section = document["gamepad_lighting"]
        self.assertEqual(section["branch_selection"], before["gamepad_lighting"]["branch_selection"])
        self.assertEqual(section["native_engine"], before["gamepad_lighting"]["native_engine"])
        self.assertEqual(section["complete_module_bodies"], {"existing": "complete", "current": "complete"})
        for previous, product in zip(before["gamepad_lighting"]["products"], section["products"]):
            self.assertEqual(product["branch_selection"], previous["branch_selection"])
            if product["product_id"] == 2676:
                self.assertEqual(product["source_review"]["mount"], "current mount")
                for field in ("review_source", "notification_branch", "obm_branch"):
                    self.assertEqual(product["source_review"][field], previous["source_review"][field])
            elif product["product_id"] != 4115:
                self.assertEqual(product, previous)
            else:
                self.assertEqual(product["failures"], result["products"][0]["failures"])
                self.assertEqual(product["unresolved_chains"], ["current source gap"])
        self.assertEqual(result["products"][0].get("branch_selection"), None)
        for field in ("branch_selection", "products", "duplicate_products"):
            bad = copy.deepcopy(result)
            if field == "branch_selection":
                bad[field] = {"review_source": {"sha256": "c" * 64}}
            elif field == "products":
                bad[field][0]["branch_selection"] = {"review_source": {"sha256": "c" * 64}}
            else:
                bad["products"].append(copy.deepcopy(bad["products"][0]))
            retained = copy.deepcopy(document)
            with self.subTest(field=field), self.assertRaises(ValueError):
                families.merge_gamepad_lighting_query(document, bad)
            self.assertEqual(document, retained)

    @staticmethod
    def freeze_gamepad_review(root, review):
        import os
        raw = (json.dumps(review, ensure_ascii=False, indent=2) + "\n").replace("\n", os.linesep).encode()
        path = root / ".analysis/results/reverse-pending/review-sources" / (hashlib.sha256(raw).hexdigest() + ".json")
        analysis.save(path, review)
        return families.identity(path)

    def gamepad_publication_fixture(self, root):
        document, review, _ = self.gamepad_fixture()
        original_path = root / ".analysis/inputs/frontend/gamepad-test/raw/source.js"
        original_path.parent.mkdir(parents=True)
        raw = b"const observed=7;"
        original_path.write_bytes(raw)
        original = families.identity(original_path)
        owner_key = original["sha256"] + ":0:" + str(len(raw))
        review["complete_owners"] = {owner_key: {"original": original, "byte_start": 0, "byte_end": len(raw),
            "body": raw.decode(), "body_sha256": original["sha256"]}}
        review.update(originals=[original], target_execution=False, whole_page_complete=False)
        for row in review["products"]:
            row["bootstrap_owner"] = owner_key
            row["chain_owners"] = {"11365": owner_key}
            row["ui_mount_owners"] = [owner_key]
        reference = self.freeze_gamepad_review(root, review)
        families.merge_frontend_semantic_review(document, review, reference)
        canonical = root / ".analysis/results/frontend/gamepad-main-audit.json"
        analysis.save(canonical, document)
        return document, review, reference, canonical

    @staticmethod
    def gamepad_selection_args():
        from types import SimpleNamespace
        return SimpleNamespace(family="gamepad", function="main", query="gamepad-lighting-selection",
                               product=None, prepare=False)

    def test_gamepad_selection_publication_chains_verified_previous_and_reuses_identical_review(self):
        import contextlib
        import io
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            _, review, prior, canonical = self.gamepad_publication_fixture(root)
            current = copy.deepcopy(review)
            current["semantics"]["source_selection"] = "revalidated current source"
            with patch.object(families, "gamepad_lighting_selection_sources", return_value=copy.deepcopy(current)), \
                    contextlib.redirect_stdout(io.StringIO()):
                published = families.audit_gamepad_lighting_selection(self.gamepad_selection_args())
            selected = json.loads(canonical.read_bytes())["gamepad_lighting"]["branch_selection"]
            self.assertEqual(selected["previous_review_source"], prior)
            reference = published["review_source"]
            self.assertNotEqual(reference, prior)
            self.assertEqual(json.loads((root / reference["path"]).read_bytes())["previous_review_source"], prior)
            self.assertTrue(families.valid(prior))
            before = canonical.read_bytes()
            artifacts = sorted((root / ".analysis/results/reverse-pending/review-sources").iterdir())
            with patch.object(families, "gamepad_lighting_selection_sources", return_value=copy.deepcopy(current)), \
                    contextlib.redirect_stdout(io.StringIO()):
                repeated = families.audit_gamepad_lighting_selection(self.gamepad_selection_args())
            self.assertEqual(repeated["review_source"], reference)
            self.assertEqual(canonical.read_bytes(), before)
            self.assertEqual(sorted((root / ".analysis/results/reverse-pending/review-sources").iterdir()), artifacts)
            self.assertFalse(list(root.rglob("*.tmp")))

    def test_gamepad_selection_rejects_invalid_stored_identity_before_collecting(self):
        for mode in ("bytes", "sha256", "outside", "actual_bytes", "missing_reference", "scope"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                document, review, reference, canonical = self.gamepad_publication_fixture(root)
                stored = document["gamepad_lighting"]["branch_selection"]["review_source"]
                if mode == "bytes":
                    stored["bytes"] += 1
                elif mode == "sha256":
                    stored["sha256"] = "0" * 64
                elif mode == "outside":
                    stored["path"] = ".work/" + reference["sha256"] + ".json"
                elif mode == "actual_bytes":
                    (root / reference["path"]).write_bytes(b"changed")
                elif mode == "missing_reference":
                    document["gamepad_lighting"]["branch_selection"].pop("review_source")
                else:
                    wrong = {**review, "kind": "current_legacy_camera_selection_review"}
                    document["gamepad_lighting"]["branch_selection"]["review_source"] = self.freeze_gamepad_review(root, wrong)
                analysis.save(canonical, document)
                before = canonical.read_bytes()
                with patch.object(families, "gamepad_lighting_selection_sources") as collect, self.assertRaises(ValueError):
                    families.audit_gamepad_lighting_selection(self.gamepad_selection_args())
                collect.assert_not_called()
                self.assertEqual(canonical.read_bytes(), before)

    def test_gamepad_selection_rechecks_previous_reference_and_bytes_inside_publication_lock(self):
        import contextlib
        import io
        for mode in ("newer_review", "removed_review", "changed_bytes", "changed_during_merge"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                document, review, reference, canonical = self.gamepad_publication_fixture(root)
                candidate = copy.deepcopy(review)
                candidate["semantics"]["generation"] = "candidate"
                concurrent_bytes = []
                def collect():
                    if mode == "newer_review":
                        concurrent = {**copy.deepcopy(review), "previous_review_source": reference}
                        concurrent["semantics"]["generation"] = "concurrent"
                        newer = self.freeze_gamepad_review(root, concurrent)
                        families.merge_frontend_semantic_review(document, concurrent, newer, expected_previous_review=reference)
                        analysis.save(canonical, document)
                    elif mode == "removed_review":
                        document["gamepad_lighting"].pop("branch_selection")
                        analysis.save(canonical, document)
                    elif mode == "changed_bytes":
                        (root / reference["path"]).write_bytes(b"changed during collection")
                    concurrent_bytes.append(canonical.read_bytes())
                    return candidate
                merge = families.merge_frontend_semantic_review
                def merge_and_change(*args, **kwargs):
                    result = merge(*args, **kwargs)
                    if mode == "changed_during_merge":
                        (root / reference["path"]).write_bytes(b"changed during publication")
                    return result
                with patch.object(families, "gamepad_lighting_selection_sources", side_effect=collect), \
                        patch.object(families, "merge_frontend_semantic_review", side_effect=merge_and_change), \
                        contextlib.redirect_stdout(io.StringIO()), self.assertRaises(ValueError):
                    families.audit_gamepad_lighting_selection(self.gamepad_selection_args())
                self.assertEqual(canonical.read_bytes(), concurrent_bytes[0])
                self.assertFalse(list(root.rglob("*.tmp")))

    def receiver_fixture(self):
        reference = {"path": "review.json", "bytes": 10, "sha256": "a" * 64}
        previous = "Runtime feature injection, actual adapter identity and parser overrides must be joined to each source product/version/host initialization branch; retained alternate modules alone do not establish a live selection"
        products = [{"product_id": pid, "unresolved_chains": [previous, "separate native requirement"],
            "source_chain_complete": False, "whole_page_complete": False, "runtime_acceptance": "unverified",
            "duallink_protocol": {"complete_owners": [], "unresolved_chains": [previous, "separate native requirement"],
                "source_chain_complete": False, "whole_page_complete": False, "runtime_acceptance": "unverified"}}
            for pid in (158, 192, 194, 211, 222)]
        document = {"sibling_audit": {"status": "retained"}, "receiver_legacy": {"products": products,
            "complete_owners": {}, "resource_preparation": {"prepared": True},
            "generated_descriptors": [{"path": "descriptor.rs", "sha256": "b" * 64}]}}
        review = {"kind": "current_legacy_receiver_runtime_selection_review", "complete_owners": {},
            "products": [{"product_id": pid, "runtime_selection": {"identity": {"productId": pid},
                "source_branch_selection_complete": True, "target_execution": False},
                "selected_chain_owners": {}, "remaining_local_chains": ["native and final UI still required"]}
                for pid in (158, 192, 194, 211, 222)]}
        return document, review, reference

    def test_receiver_increment_preserves_preparation_siblings_and_unrelated_gaps(self):
        document, review, reference = self.receiver_fixture()
        before = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document['sibling_audit'], before['sibling_audit'])
        for field in ('resource_preparation', 'generated_descriptors'):
            self.assertEqual(document['receiver_legacy'][field], before['receiver_legacy'][field])
        for product in document['receiver_legacy']['products']:
            for branch in (product, product['duallink_protocol']):
                self.assertEqual(branch['unresolved_chains'], ['separate native requirement', 'native and final UI still required'])
                self.assertTrue(branch['source_branch_selection_complete'])
                self.assertFalse(branch['source_chain_complete'])
                self.assertFalse(branch['whole_page_complete'])
                self.assertEqual(branch['runtime_acceptance'], 'unverified')
        merged = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document, merged)

    def test_incomplete_product_set_and_conflicting_current_selection_fail_before_mutation(self):
        for mode in ('missing_product', 'conflict'):
            document, review, reference = self.receiver_fixture()
            if mode == 'missing_product':
                review['products'].pop()
            else:
                document['receiver_legacy']['products'][0]['duallink_protocol']['runtime_selection'] = {'newer': True}
            before = copy.deepcopy(document)
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                families.merge_frontend_semantic_review(document, review, reference)
            self.assertEqual(document, before)

    def test_keypad_increment_keeps_existing_owners_and_runtime_boundaries(self):
        document = {'other_query': {'retained': True}, 'queries': {'gamepad_tester': {
            'complete_owners': [], 'verified_artifacts': [], 'whole_page_complete': False,
            'runtime_acceptance': 'unverified', 'keypad_747_review': {'direction_mode_write_and_obm': 'incomplete',
                'rust_input_stream': 'implemented_with_pure_simulated_tests', 'font_pixel_acceptance': 'unverified'}}}}
        review = {'kind': 'current_747_direction_obm_semantic_review', 'product_id': 747,
            'target_execution': False, 'whole_page_complete': False, 'complete_owners': {},
            'semantics': {'payload': 'actual selected packet size10'}, 'remaining_local_chains': ['native consumer']}
        reference = {'path': 'review.json', 'bytes': 12, 'sha256': 'a' * 64}
        families.merge_frontend_semantic_review(document, review, reference)
        query = document['queries']['gamepad_tester']
        self.assertEqual(query['keypad_747_review']['rust_input_stream'], 'implemented_with_pure_simulated_tests')
        self.assertEqual(query['keypad_747_review']['font_pixel_acceptance'], 'unverified')
        self.assertEqual(query['runtime_acceptance'], 'unverified')
        self.assertFalse(query['whole_page_complete'])
        self.assertEqual(document['other_query'], {'retained': True})
        self.assertEqual(query['verified_artifacts'], [reference])
        merged = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document, merged)

    def test_receiver_resource_rebinding_preserves_runtime_evidence_and_rejects_changed_outputs(self):
        document, _, reference = self.receiver_fixture()
        before = copy.deepcopy(document)
        current = [{'path': 'current.rs', 'bytes': 20, 'sha256': 'c' * 64}]
        review = {'kind': 'current_legacy_receiver_resource_owner_review', 'complete_owners': {},
            'product_ids': [158, 192, 194, 211, 222],
            'superseded_descriptor_receipts': copy.deepcopy(document['receiver_legacy']['generated_descriptors']),
            'generated_descriptors': current, 'generated_descriptor_owners': [{'storage': 'rust', 'reader': 'current'}]}
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document['receiver_legacy']['generated_descriptors'], current)
        for field in ('products', 'complete_owners', 'resource_preparation'):
            self.assertEqual(document['receiver_legacy'][field], before['receiver_legacy'][field])
        self.assertEqual(document['sibling_audit'], before['sibling_audit'])
        merged = copy.deepcopy(document)
        families.merge_frontend_semantic_review(document, review, reference)
        self.assertEqual(document, merged)
        document['receiver_legacy']['generated_descriptors'] = [{'newer': True}]
        with self.assertRaisesRegex(ValueError, 'changed'):
            families.merge_frontend_semantic_review(document, review, reference)


class KeyboardDriverRequestLifecycleTests(unittest.TestCase):
    def abi_fixture(self):
        raw, _ = pe_fixture()
        raw.extend(b'\0' * (0x3200 - len(raw)))
        struct.pack_into('<Q', raw, 0x98 + 24, 0x140000000)
        struct.pack_into('<8sIIII', raw, 0x188, b'.rdata\0\0', 0x3000, 0x9000, 0x3000, 0x200)
        raw[0x22d0:0x2300] = bytes.fromhex(
            '30000000000000008094004001000000010000000f00000000000000bc010000'
            'f8b30040010000000000000000000000')
        raw[0x450:0x458] = bytes.fromhex('1020004001000000')
        return raw

    def test_binding_and_physical_cleanup_are_decoded_from_independent_literal_bytes(self):
        abi, read = families.keyboard_driver_abi_bytes(self.abi_fixture())
        self.assertEqual(abi['version'], [1, 15, 0])
        self.assertEqual(abi['function_count'], 444)
        self.assertEqual(abi['function_table_rva'], '0xb3f8')
        self.assertEqual(abi['cleanup_target_rva'], '0x2010')
        self.assertEqual(read(0x9250, 8), bytes.fromhex('1020004001000000'))

    def test_wrong_abi_version_table_parent_target_and_unbacked_spans_are_rejected(self):
        for changed in ('architecture', 'version', 'table_owner', 'control_cleanup', 'raw_backing', 'overlap'):
            raw = self.abi_fixture()
            if changed == 'architecture':
                struct.pack_into('<H', raw, 0x84, 0x14c)
            elif changed == 'version':
                raw[0x22d0 + 20] = 16
            elif changed == 'table_owner':
                raw[0x22d0 + 32] ^= 8
            elif changed == 'control_cleanup':
                raw[0x450:0x458] = bytes.fromhex('c01f004001000000')
            elif changed == 'raw_backing':
                struct.pack_into('<I', raw, 0x188 + 16, 0x1000)
            else:
                struct.pack_into('<H', raw, 0x86, 2)
                raw[0x188 + 40:0x188 + 80] = raw[0x188:0x188 + 40]
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                families.keyboard_driver_abi_bytes(raw)

    def test_product_identity_is_not_the_hardware_pid_and_mode_cannot_select_another_query(self):
        from types import SimpleNamespace
        for product in ([692], [691, 691], [691, 692]):
            args = SimpleNamespace(family='keyboard', function='mw', product=product)
            with self.subTest(product=product), self.assertRaisesRegex(ValueError, 'hardware PID'):
                families.audit_keyboard_driver_request_lifecycle(args)
        args = SimpleNamespace(query='keyboard-gamepad-tester', native_driver_lifecycle='negative')
        with self.assertRaisesRegex(ValueError, 'Native keyboard lifecycle mode'):
            families.audit_family_function(args)


class KeyboardGamepadCompilerReviewTests(unittest.TestCase):
    def source_fixture(self):
        transaction = ('function save(){DeviceInfo.isFWAnalogMapping||'
            '(e(C)&&Boolean(feature.analogV3)?(yield(0,events.XC)(),yield(0,events.Y$)()):yield(0,events.XC)());'
            'doc.version=2;yield(0,storage.Pf)(doc,false);yield(0,runtime.qw)(doc);'
            'post({status:"completed"});manager.enqueueTask(work);try{}catch(e){post({status:"error"})}finally{clear()}}')
        compiler = ('function compile(){result.mappings=generateAppEngineMappings(keys);'
            'const analog=generateAnalogMappings(keys);result.actuations=analog.actuations;'
            'result.rapidTriggers=analog.rapidTriggers;result.hash=hash(result);'
            'do{try{storage.save(JSON.stringify(result))}catch(e){if(e.name==="RzLockableStorageError")retry=true}}while(retry&&attempt<3)}')
        registration = ('async function register(){if(await api.isXboxGamepadEventRegistered(device))return;'
            'let skip=50;if(DeviceInfo.PollingRateSkipMap)skip=DeviceInfo.PollingRateSkipMap[rate];'
            '(yield api.registerXboxGamepadEvent(device,skip))?api.on("xboxGamepadEvent",callback):console.warn("Failed to register")}')
        unregister = ('async function unregister(){(yield api.isXboxGamepadEventRegistered(device))?'
            '(yield api.unregisterXboxGamepadEvent(device))?api.off("xboxGamepadEvent",callback):'
            'console.warn("Failed to unregister"):console.warn("not registered")}')
        callback = ('async function callback(e){if(e.productId!==DeviceInfo.productId)return;'
            'O=t;const data=JSON.parse(e.jsonEvent);if(Array.isArray(data))for(const sample of data){'
            'if(O!==t)return;publish(sample);yield new Promise(e=>setTimeout(e,5))}O=null}')
        return [transaction, compiler, registration, unregister, callback, 'class Page{render(){return jsx(Tester,{})}}']

    def test_non_firmware_compiler_and_observation_are_distinct_from_confirmation(self):
        contract = families.keyboard_gamepad_compiler_contract(*self.source_fixture())
        self.assertEqual(contract['mapping_mode'], 'analogV3_non_firmware')
        self.assertEqual(contract['compiler_outputs'], ['mappings', 'actuations', 'rapidTriggers', 'hash'])
        self.assertEqual(contract['events_requested_skip'], 50)
        self.assertEqual(contract['turbo_storage_conflict_attempts'], 3)
        self.assertFalse(contract['document_commit_is_device_confirmation'])
        self.assertFalse(contract['callback_is_device_readback'])
        self.assertFalse(contract['target_execution'])
        self.assertEqual(contract['runtime_acceptance'], 'unverified')

    def test_firmware_compiler_and_missing_rapid_trigger_output_require_another_review(self):
        for index, old, new in ((0, 'isFWAnalogMapping||', 'isFWAnalogMapping&&'),
                                 (1, 'result.rapidTriggers=', 'result.triggers='),
                                 (1, 'attempt<3', 'attempt<8')):
            source = self.source_fixture()
            source[index] = source[index].replace(old, new)
            with self.subTest(index=index, changed=new), self.assertRaises(ValueError):
                families.keyboard_gamepad_compiler_contract(*source)

    def test_registration_failure_and_unregistration_do_not_cancel_started_array_playback(self):
        source = self.source_fixture()
        contract = families.keyboard_gamepad_compiler_contract(*source)
        self.assertIn('false unregister does not detach', contract['registration_failure'])
        self.assertTrue(contract['new_callback_replaces_old_playback'])
        self.assertEqual(contract['callback_playback_ms'], 5)
        self.assertFalse(contract['unregister_cancels_started_playback'])
        for index, old, new in ((2, '))?api.on(', '));api.on('),
                                 (3, '))?api.off(', '));api.off('),
                                 (3, 'async function unregister(){', 'async function unregister(){O=null;')):
            changed = list(source)
            changed[index] = changed[index].replace(old, new)
            with self.subTest(index=index, changed=new), self.assertRaises(ValueError):
                families.keyboard_gamepad_compiler_contract(*changed)

    def test_in_view_interval_or_early_completion_cannot_reuse_this_source_contract(self):
        source = self.source_fixture()
        self.assertFalse(families.keyboard_gamepad_compiler_contract(*source)['ui_in_view_interval5_mounted'])
        for body in ('class Page{mount(){this.toggleXboxRegistration(true)}}',
                     'class Page{mount(){new IntersectionObserver(callback)}}'):
            with self.subTest(body=body), self.assertRaisesRegex(ValueError, 'in-view'):
                families.keyboard_gamepad_compiler_contract(*source[:-1], body)
        source[0] = source[0].replace('doc.version=2;', 'post({status:"completed"});doc.version=2;')
        with self.assertRaisesRegex(ValueError, 'status/commit'):
            families.keyboard_gamepad_compiler_contract(*source)

    def publication_fixture(self):
        owner = {'id': 'a:10:20', 'roles': ['existing'], 'body': 'complete body'}
        artifact = {'path': 'original.js', 'bytes': 10, 'sha256': 'a' * 64}
        document = {'sibling': {'retained': True}, 'queries': {'other': {'retained': True}, 'gamepad_tester': {
            'products': [{'product_id': 679, 'rust_consumer': 'implemented'}, {'product_id': 678, 'old': True}],
            'complete_owners': [copy.deepcopy(owner)], 'verified_artifacts': [copy.deepcopy(artifact)],
            'exports': {'679': 'retained'}, 'keypad_747_review': {'review_source': {'sha256': 'b' * 64}},
            'whole_page_complete': False, 'runtime_acceptance': 'unverified'}}}
        result = {'products': [{'product_id': 678, 'non_firmware_mapping_review': {'runtime_acceptance': 'unverified'}},
                               {'product_id': 688, 'rust_consumer': 'incomplete'}],
            'complete_owners': [{**owner, 'roles': ['new']}], 'verified_artifacts': [artifact],
            'whole_page_complete': False, 'runtime_acceptance': 'unverified'}
        return document, result

    def test_increment_keeps_679_keypad_review_and_sibling_queries(self):
        document, result = self.publication_fixture()
        before = copy.deepcopy(document)
        query = families.merge_keyboard_gamepad_tester_query(document, result)
        self.assertEqual(document['sibling'], before['sibling'])
        self.assertEqual(document['queries']['other'], before['queries']['other'])
        self.assertEqual(query['products'][0], before['queries']['gamepad_tester']['products'][0])
        self.assertEqual(query['keypad_747_review'], before['queries']['gamepad_tester']['keypad_747_review'])
        self.assertEqual(query['exports'], {'679': 'retained'})
        self.assertEqual(query['complete_owners'][0]['roles'], ['existing', 'new'])
        self.assertFalse(query['whole_page_complete'])
        self.assertEqual(query['runtime_acceptance'], 'unverified')
        merged = copy.deepcopy(document)
        families.merge_keyboard_gamepad_tester_query(document, result)
        self.assertEqual(document, merged)

    def test_owner_conflict_input_drift_and_duplicate_products_fail_before_publication(self):
        for field in ('owner', 'artifact', 'product'):
            document, result = self.publication_fixture()
            if field == 'owner':
                result['complete_owners'][0]['body'] = 'different body'
            elif field == 'artifact':
                result['verified_artifacts'][0]['sha256'] = 'c' * 64
            else:
                result['products'].append(copy.deepcopy(result['products'][0]))
            before = copy.deepcopy(document)
            with self.subTest(field=field), self.assertRaises(ValueError):
                families.merge_keyboard_gamepad_tester_query(document, result)
            self.assertEqual(document, before)

    def metadata_fixture(self):
        from types import SimpleNamespace
        document, result = self.publication_fixture()
        query = document['queries']['gamepad_tester']
        source = {'path': '.analysis/inputs/frontend/keyboard-678-current/raw/current.js',
                  'bytes': 3, 'sha256': hashlib.sha256(b'abc').hexdigest()}
        owner = {'id': source['sha256'] + ':0:3', 'roles': ['existing'], 'body': 'abc',
                 'original': source, 'syntax': 'program', 'byte_start': 0, 'byte_end': 3,
                 'body_sha256': source['sha256']}
        sets, artifacts = [], [source]
        for name in ('keyboard-678-current', 'keyboard-mw-678-current',
                     'keyboard-688-current', 'keyboard-mw-688-current'):
            acquisition = {'path': '.analysis/inputs/frontend/' + name + '/acquisition.json',
                           'bytes': 10, 'sha256': 'a' * 64}
            capture = {'path': '.analysis/results/frontend/' + name + '/analysis.json',
                       'bytes': 20, 'sha256': 'b' * 64}
            sets.append({'name': name, 'acquisition': acquisition, 'capture': capture,
                'deployment': {'sources': [{'original': source, 'identity_verified': True}]},
                'capture_failures': [{'stage': 'validate', 'exit_code': 1}],
                'unresolved_acquisition': ['source map unavailable'], 'failed_acquisition': []})
            artifacts.extend([acquisition, capture])
        query.update(source_sets=copy.deepcopy(sets), verified_artifacts=copy.deepcopy(artifacts),
                     complete_owners=[copy.deepcopy(owner)])
        sets[0]['capture'] = {**sets[0]['capture'], 'bytes': 21, 'sha256': 'c' * 64}
        result.update(source_sets=sets, verified_artifacts=[sets[0]['capture'] if item['path'] ==
            sets[0]['capture']['path'] else copy.deepcopy(item) for item in artifacts],
            complete_owners=[copy.deepcopy(owner)])
        actual = copy.deepcopy(sets)
        indexed = {row['name']: row for row in actual}
        def read(item):
            if item != source:
                raise ValueError('Actual original bytes differ')
            return b'abc'
        audit = SimpleNamespace(owners={owner['id']: copy.deepcopy(owner)},
            source_set=lambda name: (copy.deepcopy(indexed[name]), [copy.deepcopy(source)]),
            read=read, retain_range=lambda *args: owner['id'], final_snapshot=lambda: [],
            frontend_capture=lambda name, originals: {'manifest': copy.deepcopy(indexed[name]['capture']),
                'originals_verified': 1, 'outputs_verified': 1, 'successful_full_capture': False,
                'capture_failures': [{'stage': 'validate', 'exit_code': 1}]})
        return document, result, audit

    def test_renewal_preserves_predecessor_and_failures_after_current_source_revalidation(self):
        document, result, audit = self.metadata_fixture()
        previous = copy.deepcopy(document)
        query = families.merge_keyboard_gamepad_tester_query(document, result, audit)
        review = query['non_firmware_metadata_revalidation']
        self.assertEqual(review['selected_products'], [678, 688])
        self.assertFalse(review['strict_publication_rejection']['document_written'])
        renewed = review['source_sets'][0]
        transition = renewed['metadata_transitions'][0]
        self.assertEqual(transition['previous'], previous['queries']['gamepad_tester']['source_sets'][0]['capture'])
        self.assertEqual(transition['current'], result['source_sets'][0]['capture'])
        self.assertEqual(renewed['previous_capture_failures'], [{'stage': 'validate', 'exit_code': 1}])
        self.assertFalse(renewed['current_capture']['successful_full_capture'])
        self.assertEqual(len(renewed['previous_complete_owners_revalidated']), 1)
        self.assertEqual(query['products'][0], previous['queries']['gamepad_tester']['products'][0])
        merged = copy.deepcopy(document)
        families.merge_keyboard_gamepad_tester_query(document, result, audit)
        self.assertEqual(document, merged)

    def test_metadata_renewal_rejects_forged_acquisition_original_and_nonselected_drift(self):
        for changed in ('acquisition', 'deployment', 'body', 'nonselected'):
            document, result, audit = self.metadata_fixture()
            if changed == 'acquisition':
                result['source_sets'][0]['acquisition']['sha256'] = 'd' * 64
            elif changed == 'deployment':
                result['source_sets'][0]['deployment']['sources'][0]['original']['sha256'] = 'd' * 64
            elif changed == 'body':
                document['queries']['gamepad_tester']['complete_owners'][0]['body'] = 'forged body'
            else:
                result['source_sets'].append({**copy.deepcopy(result['source_sets'][0]), 'name': 'keyboard-679-current'})
            before = copy.deepcopy(document)
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                families.merge_keyboard_gamepad_tester_query(document, result, audit)
            self.assertEqual(document, before)


class WebpackStaticDependencyTests(unittest.TestCase):
    def inspect(self, source):
        raw = source.encode()
        parser = host.HostLocalAudit().parser
        tree = parser.parse(raw)
        self.assertFalse(tree.root_node.has_error)
        pending, modules = [tree.root_node], []
        while pending:
            node = pending.pop()
            if node.type in {"pair", "method_definition"}:
                name = node.child_by_field_name("key" if node.type == "pair" else "name")
                if name is not None and name.text == b"100":
                    modules.append(node)
            pending.extend(node.named_children)
        self.assertEqual(len(modules), 1)
        return families.javascript_factory_static_imports(modules[0], evidence.JavascriptBindings(raw, tree))

    def test_exact_require_parameter_and_nested_shadow_are_distinct(self):
        self.assertEqual(self.inspect('({100(module,exports,load){const one=load(7),two=load(3);'
            'load.d(exports,{one:()=>one});function inner(load){load(99)};load(7)}})'), [3, 7])

    def test_commonjs_pair_and_factory_without_require_are_supported(self):
        self.assertEqual(self.inspect('({100:function(module,exports,load){module.exports=load(42)}})'), [42])
        self.assertEqual(self.inspect('({100(module){module.exports=42}})'), [])

    def test_for_of_lexical_binding_does_not_escape_or_import_from_outer_require(self):
        self.assertEqual(self.inspect('({100(module,exports,load){for(const load of values){load(99)}'
            'for(let load in values){load(98)}load(3)}})'), [3])

    def test_dynamic_identity_or_parameter_reassignment_fails(self):
        for expression in ('load(variable)', 'load(7,8)', 'load=other;load(7)', 'load++;load(7)',
                           'const indirect=load;indirect(7)', 'module.exports=load'):
            with self.subTest(expression=expression), self.assertRaises(ValueError):
                self.inspect('({100(module,exports,load){' + expression + '}})')


class ReceiverBootstrapFeatureTests(unittest.TestCase):
    def setUp(self):
        self.parser = host.HostLocalAudit().parser

    def inspect(self, body, declaration='const injector=require(95414);'):
        return families.legacy_receiver_bootstrap_features(
            ('(()=>{' + declaration + body + '})()').encode(), self.parser, 95414, 'default')

    def test_namespace_import_and_actual_product_gate_are_preserved(self):
        result = self.inspect('injector.default.useFeature("rzDeviceType","rzDevice25",{isDualLinkDevice:!0});'
            'injector.default.useFeature("isBattery","yes",{shouldQueryBatteryStatus:!0});')
        self.assertEqual(result['protocol25_config'], {'isDualLinkDevice': True})
        self.assertEqual(result['feature_names'], ['rzDeviceType', 'isBattery'])
        self.assertEqual(result['registrations'][0]['singleton_import']['body'], 'injector=require(95414)')
        self.assertNotIn('canPairMultipleDevices', result['feature_names'])

    def test_absent_gate_is_distinct_from_enabled_gate(self):
        result = self.inspect('injector.default.useFeature("rzDeviceType","rzDevice25");')
        self.assertEqual(result['protocol25_config'], {})

    def test_generic_bootstrap_preserves_actual_controller_variant_and_ordered_config(self):
        raw = ('(()=>{const injector=load(47973);'
            'injector.A.useFeature("rzDeviceType","rzDevice25SagePC",{reportId:10});'
            'injector.A.useFeature("hardwareEvent","config",{hardwareEventParserType:"PROTOCOL25_CONTROLLER_RECORDID05"});'
            'injector.A.useFeature("controllerDevice","config",{supportsLighting:!1});})()').encode()
        result = families.javascript_bootstrap_features(raw, self.parser, 47973, 'A')
        self.assertEqual(result['feature_names'], ['rzDeviceType', 'hardwareEvent', 'controllerDevice'])
        self.assertEqual(result['registrations'][0]['variant'], 'rzDevice25SagePC')
        self.assertEqual(result['registrations'][0]['arguments'][2], '{reportId:10}')
        self.assertNotIn('protocol25_config', result)
        with self.assertRaisesRegex(ValueError, 'module'):
            families.javascript_bootstrap_features(raw.replace(b'load(47973)', b'load(99)'), self.parser, 47973, 'A')

    def test_wrong_namespace_export_module_and_shadowing_fail(self):
        for body, declaration in (
            ('injector.A.useFeature("rzDeviceType","rzDevice25");', 'const injector=require(95414);'),
            ('injector.default.useFeature("rzDeviceType","rzDevice25");', 'const injector=require(99);'),
            ('(function(injector){injector.default.useFeature("rzDeviceType","rzDevice25")})(other);',
             'const injector=require(95414);'),
        ):
            with self.subTest(body=body), self.assertRaises(ValueError):
                self.inspect(body, declaration)

    def test_namespace_reassignment_and_dynamic_or_duplicate_features_fail(self):
        for body in (
            'injector=other;injector.default.useFeature("rzDeviceType","rzDevice25");',
            'injector.default.useFeature(name,"rzDevice25");',
            'injector.default.useFeature("rzDeviceType",variant);',
            'injector.default.useFeature("rzDeviceType","rzDevice25");injector.default.useFeature("rzDeviceType","other");',
            'injector.default.useFeature("rzDeviceType","rzDevice25",{isDualLinkDevice:runtime});',
            'injector.default.useFeature("rzDeviceType","rzDevice25",{...runtime});',
            'injector.default.useFeature("rzDeviceType","rzDevice25",{isDualLinkDevice:!0,isDualLinkDevice:!1});',
        ):
            with self.subTest(body=body), self.assertRaises(ValueError):
                self.inspect(body, 'let injector=require(95414);')


class MouseInputMonitoringErrorConsumerTests(unittest.TestCase):
    def setUp(self):
        self.parser = host.HostLocalAudit().parser
        self.callback = (
            b'class Host{createCbInputMonitoringEvent=()=>{const s=function(e,t,s){'
            b'let n;try{n=JSON.parse(e)}catch(e){console.log(e)}'
            b'if(n)for(const e of [])try{e.send("inputMonitoringEvent",{jsonEvent:s})}'
            b'catch(e){console.log(e)}}}}')

    def test_complete_host_callback_preserves_parse_catch_and_identity_gate(self):
        projection = families.mouse_input_error_host_ast(self.parser, self.callback)
        self.assertEqual(projection, {"parse_argument": "deviceInfoJson", "parse_catch": "log_only",
            "send_gate": "parsed_identity_truthy", "native_empty_identity_forwarded": False})

    def test_missing_parse_catch_and_ungated_send_are_rejected(self):
        cases = [
            self.callback.replace(b'try{n=JSON.parse(e)}catch(e){console.log(e)}', b'n=JSON.parse(e);'),
            self.callback.replace(b'if(n)for', b'if(true)for'),
            self.callback.replace(b'JSON.parse(e)', b'JSON.parse(s)'),
            self.callback.replace(b'catch(e){console.log(e)}if(n)', b'catch(e){n={}}if(n)'),
            self.callback.replace(b'catch(e){console.log(e)}if(n)', b'catch(e){return}if(n)'),
            self.callback.replace(b'function(e,t,s)', b'function(e,t,s,tick)'),
        ]
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                families.mouse_input_error_host_ast(self.parser, value)

    def test_error_callback_cannot_send_from_parse_catch_or_add_another_send(self):
        cases = [
            self.callback.replace(b'catch(e){console.log(e)}if(n)',
                b'catch(e){e.send("inputMonitoringEvent",{})}if(n)'),
            self.callback.replace(b'let n;', b'let n;e.send("inputMonitoringEvent",{});'),
            self.callback.replace(b'if(n)for', b'n={};if(n)for'),
        ]
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                families.mouse_input_error_host_ast(self.parser, value)

    def test_native_error_identity_ack_and_two_invalidation_kinds_are_distinct(self):
        semantics = families.mouse_input_error_semantics()
        self.assertEqual(semantics["native_device_info_json"], "")
        self.assertEqual(semantics["native_event_type"], 2)
        self.assertEqual(semantics["native_error_fields"], ["errorCode", "errorString"])
        self.assertFalse(semantics["native_error_reaches_product_mw"])
        self.assertFalse(semantics["start_ack_proves_registration"])
        self.assertFalse(semantics["allocator_check_is_cancellation_token"])
        self.assertTrue(semantics["weak_service_and_observer_registration_checks_are_distinct"])
        self.assertFalse(semantics["cleanup_guarantees_all_queued_callbacks_drained"])

    def test_bounded_selection_contains_actual_error_result_and_release_consumers(self):
        roles = families.mouse_input_error_native_roles()
        self.assertEqual(roles[0xd2950], "once_error_callback_delivery")
        self.assertEqual(roles[0x19000], "engine_error_empty_device_identity")
        self.assertEqual(roles[0xcf320], "weak_service_error_dispatch")
        self.assertEqual(roles[0xb1640], "observer_registration_generation_check")
        self.assertEqual(roles[0xdfa06], "allocator_liveness_check")
        self.assertIn(0xcc030, roles)
        self.assertIn(0x1c68e0, roles)
        self.assertIn(0x212ec0, roles)


class MouseInputMonitoringIdentityConsumerTests(unittest.TestCase):
    def setUp(self):
        self.parser = host.HostLocalAudit().parser
        self.callback = ('createCbInputMonitoringEvent=()=>{const s=function(e,t,s){let n;'
            'try{n=JSON.parse(e)}catch(e){console.log(e)}if(n)e.send("inputMonitoringEvent",'
            '{containerId:n?.containerId,productId:n?.productId,vendorId:n?.vendorId,'
            'usbInstanceId:n?.usbInstanceId,inputMonitoringEventType:t,jsonEvent:s})}}')

    def test_host_projection_preserves_identity_and_separate_event_arguments(self):
        result = families.mouse_input_identity_host_projection(self.parser, self.callback)
        self.assertEqual(result, {"containerId": "n?.containerId", "productId": "n?.productId",
            "vendorId": "n?.vendorId", "usbInstanceId": "n?.usbInstanceId",
            "inputMonitoringEventType": "t", "jsonEvent": "s"})

    def test_extra_missing_changed_or_spread_identity_fields_are_rejected(self):
        cases = [
            self.callback.replace('containerId:n?.containerId,', ''),
            self.callback.replace('containerId:n?.containerId,', 'guid:n?.guid,containerId:n?.containerId,'),
            self.callback.replace('containerId:n?.containerId', 'containerId:n?.guid'),
            self.callback.replace('containerId:n?.containerId', '...n'),
            self.callback.replace('inputMonitoringEventType:t', 'inputMonitoringEventType:1'),
            self.callback.replace('jsonEvent:s', 'jsonEvent:e'),
        ]
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                families.mouse_input_identity_host_projection(self.parser, value)

    def test_repeated_field_or_second_renderer_send_is_rejected(self):
        cases = [
            self.callback.replace('containerId:n?.containerId,', 'containerId:n?.containerId,containerId:n?.guid,'),
            self.callback.replace('let n;', 'let n;e.send("inputMonitoringEvent",{});'),
        ]
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                families.mouse_input_identity_host_projection(self.parser, value)


class HanboRuntimeFlagTests(unittest.TestCase):
    def setUp(self):
        self.parser = host.HostLocalAudit().parser
        self.defaults = (b'BC={runtimeData:{cpuTemp:0,cpuSpeed:0,fanSpeed:0,pumpSpeed:0,'
                         b'liquidTemp:0,gpuTemp:0,gpuSpeed:0,isHardwareOutdated:!1}}')
        self.reducer = (
            b'FC=(e,E)=>{if(null!==e&&void 0!==e||(e=BC),E.type===se.B8N){'
            b'const a=(0,g.A)((0,g.A)({},e.runtimeData),E.payload);'
            b'return(0,g.A)((0,g.A)({},e),{},{runtimeData:a})}return e}')
        self.service = (
            b'48290:(e,t,n)=>{var o=n(67061),a=n(47399);'
            b'let c={cpuTemp:"0",cpuSpeed:"0",fanSpeed:0,pumpSpeed:0,liquidTemp:0,gpuTemp:"0",gpuSpeed:"0"};'
            b'const fan=({fanSpeed:e,pumpSpeed:t,liquidTemp:n})=>{'
            b'c=Object.assign(Object.assign({},c),{fanSpeed:e,pumpSpeed:t,liquidTemp:n});v()},'
            b'system=({CPUTemperature:e,CPUClock:t,GPUTemperature:n,GPUClock:o})=>{const a=t,l=o;'
            b'c=Object.assign(Object.assign({},c),{cpuTemp:null==e?void 0:e.toFixed(1),cpuSpeed:a,'
            b'gpuTemp:null==n?void 0:n.toFixed(1),gpuSpeed:l});v()},'
            b'v=()=>{(0,a.a)({type:o.B8N,payload:c},!1)}}')
        self.event = b'Ur="MW_UPDATE_DEVICE_RUNTIMEDATA"'

    def inspect(self, **changes):
        values = {"defaults": self.defaults, "reducer": self.reducer,
                  "service": self.service, "event_binding": self.event}
        values.update(changes)
        return families.hanbo_runtime_flag_ast(self.parser, **values)

    def test_local_false_and_unobserved_result_are_distinct(self):
        contract = self.inspect()
        self.assertFalse(contract["ui_initial_hardware_outdated"])
        self.assertFalse(contract["ui_initial_value_is_device_observation"])
        self.assertEqual(contract["hardware_outdated_observation"], "unobserved")
        self.assertFalse(contract["service_flag_producer_present"])
        self.assertEqual(contract["service_state_reference_count"], 6)
        self.assertEqual(contract["service_runtime_fields"],
                         ["cpuTemp", "cpuSpeed", "fanSpeed", "pumpSpeed", "liquidTemp", "gpuTemp", "gpuSpeed"])
        self.assertTrue(contract["missing_payload_flag_preserves_prior_value"])
        self.assertTrue(contract["external_payload_flag_would_be_merged"])

    def test_added_flag_dynamic_merge_duplicate_field_and_escape_fail(self):
        cases = [
            self.service.replace(b'fanSpeed:e,pumpSpeed:t,liquidTemp:n}',
                                 b'fanSpeed:e,pumpSpeed:t,liquidTemp:n,isHardwareOutdated:!0}'),
            self.service.replace(b'fanSpeed:e,pumpSpeed:t,liquidTemp:n}',
                                 b'fanSpeed:e,pumpSpeed:t,liquidTemp:n,...external}'),
            self.service.replace(b'fanSpeed:e,pumpSpeed:t,liquidTemp:n}',
                                 b'fanSpeed:e,pumpSpeed:t,liquidTemp:n,fanSpeed:external}'),
            self.service.replace(b'Object.assign({},c)', b'Object.assign(external,c)'),
            self.service.replace(b'const fan=', b'external(c);const fan='),
            self.service.replace(b'const fan=', b'c.isHardwareOutdated=!0;const fan='),
            self.service.replace(b'const fan=', b'const alias=c;const fan='),
            self.service.replace(b'const fan=', b'function shadow(c){return c}const fan='),
        ]
        for raw in cases:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                self.inspect(service=raw)

    def test_event_namespace_payload_and_identity_drift_fail(self):
        for before, after in [
            (b'o=n(67061)', b'o=n(67062)'), (b'a=n(47399)', b'a=n(47398)'),
            (b'type:o.B8N', b'type:o.OTHER'), (b'payload:c', b'payload:external'),
            (b'gpuSpeed:l', b'gpuSpeed:o'), (b'cpuTemp:"0"', b'cpuTemp:0'),
        ]:
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.inspect(service=self.service.replace(before, after))
        with self.assertRaises(ValueError):
            self.inspect(event_binding=b'Ur="ANOTHER_EVENT"')

    def test_initializer_reducer_reversal_or_default_on_every_merge_fail(self):
        for before, after in [
            (b'!1', b'!0'), (b'isHardwareOutdated:!1', b'isHardwareOutdated:observed'),
            (b'cpuTemp:0', b'cpuTemp:external'),
        ]:
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.inspect(defaults=self.defaults.replace(before, after))
        for before, after in [
            (b'{},e.runtimeData),E.payload', b'{},E.payload),e.runtimeData'),
            (b'{},e.runtimeData),E.payload', b'{},BC.runtimeData),E.payload'),
            (b'E.type===se.B8N', b'E.type===se.OTHER'),
        ]:
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.inspect(reducer=self.reducer.replace(before, after))


class HanboSystemMonitorVersionsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.audit = host.HostLocalAudit()
        native_contract = families._hanbo_system_monitor_versions_native_contract
        source_contract = families._hanbo_system_monitor_versions_source_contract

        def native(modules):
            cls.native = modules
            return native_contract(modules)

        def source(owners):
            cls.owners = owners
            return source_contract(owners)

        with patch.object(families, "_hanbo_system_monitor_versions_native_contract", side_effect=native), \
                patch.object(families, "_hanbo_system_monitor_versions_source_contract", side_effect=source):
            cls.result = families.collect_hanbo_system_monitor_versions(cls.audit)

    def changed_native(self, name, rva, before, after, *, assembly=False):
        modules = copy.deepcopy(self.native)
        row, = [row for row in modules[name]["complete_selected_functions"]
            if int(row["capture"]["rva"], 0) == rva]
        lines = row["capture"]["assembly"] if assembly else row["capture"]["pseudocode"]["lines"]
        line = next(line for line in lines if before in line["text"])
        line["text"] = line["text"].replace(before, after, 1)
        return families._hanbo_system_monitor_versions_native_contract(modules)

    def changed_source(self, role, before, after):
        owners = copy.deepcopy(self.owners)
        self.assertIn(before, owners[role]["body"])
        owners[role]["body"] = owners[role]["body"].replace(before, after, 1)
        return families._hanbo_system_monitor_versions_source_contract(owners)

    def test_current_versions_are_actual_declared_pe_inputs_with_separate_acceptance(self):
        result = self.result
        self.assertEqual(result["host_release"]["version"], "4.0.896")
        self.assertEqual(result["versions"], {name: "1.0.30.0" for name in
            ("RzSystemCommon", "RzSystemService", "RzDLLService")})
        self.assertEqual({name: len(module["functions"]) for name, module in result["native"].items()},
            {"RzSystemCommon": 38, "RzSystemService": 21, "RzDLLService": 6})
        self.assertEqual({name: len(module["functions"]) for name, module in result["legacy_native"].items()},
            {"RzSystemCommon": 34, "RzSystemService": 12, "RzDLLService": 3})
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                "real_device_operations", "actual_installed_or_loaded_versions_observed"):
            self.assertFalse(result[key])
        self.assertEqual(result["actual_dll_comparison"], "unverified")

    def test_common_comparison_limits_equality_to_complete_selected_bodies(self):
        comparisons = self.result["common_version_comparisons"]
        self.assertEqual(len(comparisons), 34)
        self.assertEqual({row["legacy_rva"] for row in comparisons
            if not row["machine_address_normalized_pseudocode_equal"]}, {"0x619c0", "0x61de0"})
        self.assertEqual(self.result["native_contract"]["common"]["numeric_missing"], "null")
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_native("RzSystemCommon", 0x6d360, "if ( !v6 || !v21 )", "if ( !v6 || v21 <= 0 )")

    def test_current_pipe_marker_search_is_actual_strstr_but_not_cross_call_reassembly(self):
        common = self.result["native_contract"]["common"]
        self.assertTrue(common["also_searches_current_read_region_with_strstr"])
        self.assertFalse(common["search_includes_preceding_reads"])
        self.assertTrue(common["clears_buffer_at_read_helper_entry"])
        self.assertTrue(common["receiver_retains_offset_when_incomplete"])
        for rva, before, after in ((0x60e10, "return strstr(a1, a2)", "return 0"),
                (0x61e00, "v20 = 500", "v20 = 50"),
                (0x61e00, 'Push(v29, "<EOF>")', 'Push(v19, "<EOF>")')):
            with self.subTest(rva=hex(rva)), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.changed_native("RzSystemCommon", rva, before, after)

    def test_received_flag_cache_callbacks_and_device_stop_do_not_prove_freshness_or_quiescence(self):
        common = self.result["native_contract"]["common"]
        self.assertTrue(common["message_flag_set_before_json_parse"])
        self.assertFalse(common["parse_is_transactional"])
        self.assertFalse(common["cache_callback_is_fresh_device_observation"])
        self.assertFalse(common["window_thread_joined"])
        self.assertTrue(common["pipe_receiver_joined"])
        self.assertEqual(common["device_terminate"], "logging_only")
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_native("RzSystemCommon", 0x66cc0, "CloseHandle(hObject)", "WaitForSingleObject(hObject, 0)")

    def test_refresh_failure_counts_reinitialize_without_clearing_count_and_disable_at_ten(self):
        service = self.result["native_contract"]["service"]
        self.assertEqual(service["refresh_failure_reinitialize_counts"], [3, 6, 9])
        self.assertEqual(service["disable_at_consecutive_failures"], 10)
        self.assertFalse(service["reinitialization_resets_failure_count"])
        self.assertTrue(service["normal_update_resets_failure_count"])
        self.assertFalse(service["refresh_result_changes_export_success_directly"])
        for before, after in (("if ( v2 < 10 )", "if ( v2 < 11 )"),
                ("if ( v2 == 3 * (v2 / 3) )", "if ( v2 == 2 * (v2 / 2) )"),
                ("_InterlockedExchange(&dword_180072760, 0)", "_InterlockedExchange(&dword_180072760, 1)")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.changed_native("RzSystemService", 0x18340, before, after)

    def test_actual_seh_scope_and_assembly_cover_handler_omitted_from_pseudocode(self):
        service = self.result["native_contract"]["service"]
        self.assertEqual(service["refresh_exception_filter_codes"], ["0xc0000005", "0xc0000006",
            "0xc000001d", "0xc000008c", "0xc0000094", "0xc0000096", "0xc00000fd"])
        spans = service["verified_seh_spans"]
        self.assertEqual(struct.unpack("<III", bytes.fromhex(spans[0]["raw_hex"])), (0x141d0, 0x14209, 0x6542c))
        self.assertEqual(struct.unpack("<IIIIII", bytes.fromhex(spans[1]["raw_hex"])[8:]),
            (0x50523, 1, 0x141dd, 0x141f5, 0x514a0, 0x141f5))
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_native("RzSystemService", 0x141d0, "xor     al, al", "mov     al, 1", assembly=True)

    def test_service_window_wait_preserves_timeout_failure_and_wait_failed_limitation(self):
        service = self.result["native_contract"]["service"]
        self.assertEqual((service["window_wait_step_ms"], service["window_wait_timeout_ms"]), (200, 5000))
        self.assertTrue(service["sdk_released_before_window_wait"])
        self.assertTrue(service["window_timeout_keeps_owner_and_initialized_flag"])
        self.assertFalse(service["wait_failure_distinguished_from_signaled"])
        self.assertFalse(service["preload_or_thread_creation_failure_prevents_initialized_flag"])
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_native("RzSystemService", 0x39e80,
                "WaitForSingleObject(hObject, 0xC8u) != 258", "WaitForSingleObject(hObject, 0xC8u) == 0")

    def test_current_server_consumes_terminate_false_before_unload(self):
        server = self.result["native_contract"]["server"]
        self.assertTrue(server["terminate_false_skips_free_library"])
        self.assertFalse(server["terminate_false_removes_module_from_map"])
        self.assertTrue(server["missing_terminate_allows_free_library"])
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_native("RzDLLService", 0x32280,
                "if ( !Terminate || Terminate() )", "if ( !Terminate || !Terminate() )")

    def test_server_release_order_and_forced_worker_stop_do_not_promote_queue_drain(self):
        server = self.result["native_contract"]["server"]
        self.assertTrue(server["module_release_precedes_pool_shutdown_flag_and_worker_stop"])
        self.assertFalse(server["complete_server_task_and_callback_quiescence_proven"])
        self.assertEqual((server["worker_wait_ms"], server["force_terminate_followup_wait_ms"]), (10000, 1000))
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.changed_native("RzDLLService", 0x39580, "byte_14009EB74 = 1", "byte_14009EB74 = 0")

    def test_current_hanbo_filters_info_fields_and_keeps_zero_projection_as_source_fallback(self):
        source = self.result["source_contract"]
        self.assertEqual(source["data_type_mask"], 0x300000)
        self.assertEqual(source["callback_filters"], {"dllName": "RzSystemCommon",
            "dllFunc": "HardwareMonitorCallback", "status": "SUCCESS"})
        self.assertTrue(source["projection"]["missing_measurements_default_to_zero_in_source"])
        self.assertFalse(source["projection"]["zero_projection_is_confirmed_hardware_observation"])
        self.assertFalse(source["projection"]["hanbo_checks_projection_status_before_forwarding"])
        for role, before, after in (("hanbo_subscription", "[Ye.vC.CPU_Info,Ye.vC.GPU_Info]", "[]"),
                ("system_info_projection", "if(0!==v.initStatus)return v", "if(0===v.initStatus)return v"),
                ("shared_monitor_filter", '"RzSystemCommon"===t.dllName', '"Other"===t.dllName')):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source(role, before, after)

    def test_current_server_start_probe_order_is_distinct_from_old_register_first(self):
        source = self.result["source_contract"]
        self.assertTrue(source["server_start_precedes_subscription"])
        self.assertEqual(source["server_probe_delays_ms"], [100, 200, 400, 700, 1100])
        self.assertTrue(source["registers_after_all_server_probes_fail"])
        with self.assertRaisesRegex(ValueError, "complete source boundary changed"):
            self.changed_source("shared_start_stop", "n<5", "n<10")

    def test_product_stop_and_current_host_retention_remain_separate_from_common_terminate(self):
        source = self.result["source_contract"]
        self.assertFalse(source["stop_awaits_device_terminate"])
        self.assertFalse(source["stop_calls_common_terminate"])
        self.assertFalse(source["host_callback_map_released_on_product_stop"])
        self.assertTrue(source["current_host_ffi_bodies_equal_legacy"])
        self.assertEqual(len(self.result["roles"]), 17)
        self.assertEqual(self.audit.final_snapshot(), [])
        with self.assertRaisesRegex(ValueError, "complete source boundary changed"):
            self.changed_source("shared_start_stop", ",e.terminate()", ",yield e.terminate()")


class HanboGpuTemperatureChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sdk = {rva: json.loads((analysis.ROOT / ".analysis/results/native" /
            "c67f23ef5fd6e5a1db8f96d9a0509447f9657969e7a2c7fed7b293b72240cfb7/functions" /
            (format(rva, "x") + ".json")).read_bytes()) for rva in families.hanbo_gpu_temperature_roles()}
        cls.services = {}
        for version, digest in (("1.0.29.0", "605eea0c35225bac06b8d9a6f93a7c57d8d4b7a3058a8e718fd032c39e77bbf3"),
                ("1.0.30.0", "645d8ffdcaefe57b06ab4dd9888a7ce028fe25052f87762c23c452d3936a3cf8")):
            cls.services[version] = {rva: json.loads((analysis.ROOT / ".analysis/results/native" / digest /
                "functions" / (format(rva, "x") + ".json")).read_bytes())
                for rva in families.hanbo_gpu_temperature_service_roles(version)}
        sentinel = struct.unpack("<f", bytes.fromhex("0c139af9"))[0]
        cls.constants = {"q8_8_scale": 1 / 256, "sensor_valid_above": sentinel,
            "initial_current": sentinel, "initial_minimum": -sentinel, "initial_maximum": sentinel}

    def project(self, *, sdk=None, service=None, constants=None, version="1.0.29.0"):
        return families.hanbo_gpu_temperature_projection(sdk if sdk is not None else self.sdk,
            service if service is not None else self.services[version],
            constants if constants is not None else self.constants, service_version=version)

    def replaced(self, bodies, rva, before, after):
        candidate = dict(bodies)
        candidate[rva] = dict(bodies[rva])
        lines = bodies[rva]["pseudocode"]["lines"]
        self.assertTrue(any(before in row["text"] for row in lines), before)
        candidate[rva]["pseudocode"] = dict(bodies[rva]["pseudocode"],
            lines=[dict(row, text=row["text"].replace(before, after)) for row in lines])
        return candidate

    def test_current_and_retained_service_share_selected_semantics(self):
        old = self.project()
        current = self.project(version="1.0.30.0")
        self.assertEqual(old.pop("service_version"), "1.0.29.0")
        self.assertEqual(current.pop("service_version"), "1.0.30.0")
        self.assertEqual(old, current)
        self.assertEqual(old["target"], 1)
        self.assertEqual(old["sensor_identifier"], 0x4002001)
        self.assertEqual(old["fields"], {"raw": 336, "current": 348, "minimum": 352, "maximum": 356, "multiplier": 360})
        self.assertFalse(old["failed_refresh_clears_existing_sensor"])
        self.assertFalse(old["failed_refresh_is_new_observation"])
        self.assertNotEqual(old["sensor_valid_above"], old["service_valid_above"])
        self.assertEqual(json.loads(json.dumps(old, sort_keys=True)), old)

    def test_target_sensor_kind_extrema_and_success_gate_changes_fail(self):
        for before, after in (("         1,", "         2,"),
                ("|= 0x4002000u", "|= 0x4402000u"),
                ("if ( v8 > v7[89] )", "if ( v8 < v7[89] )"),
                ("if ( v7[88] > v8 )", "if ( v7[88] < v8 )"),
                ("v7[87] = v385 * v7[90]", "v7[87] = 0"),
                ("if ( !*(_QWORD *)(a1 + 944) )", "if ( *(_QWORD *)(a1 + 944) )")):
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.project(sdk=self.replaced(self.sdk, 0x1f53d0, before, after))

    def test_sleep_q8_8_enabled_type_mask_and_thermal_positive_boundaries_fail(self):
        cases = ((0x11f2c0, "== -220", "== -1"),
            (0x11d3e0, "!_bittest(&v15, v18)", "_bittest(&v15, v18)"),
            (0x11d3e0, "(v20[v8 + 10] & 0xFF00) == 0xFF00", "v20[v8 + 10] == 0"),
            (0x11d3e0, "* 0.00390625", "* 0.0078125"),
            (0x11d2d0, "i[1] != v7 || *i <= 0", "i[1] != v7 || *i < 0"),
            (0x11d2d0, "v7 = 1", "v7 = 8"))
        for rva, before, after in cases:
            with self.subTest(rva=rva, before=before), self.assertRaises(ValueError):
                self.project(sdk=self.replaced(self.sdk, rva, before, after))

    def test_actual_result_consumers_are_checked_in_both_versions(self):
        for version, iterator, getter, current in (("1.0.29.0", 0x17300, 0x5410, "v121"),
                ("1.0.30.0", 0x17570, 0x5420, "v131")):
            for rva, before, after in ((iterator, current + " > -4294967300.0", current + " >= -4294967300.0"),
                    (iterator, "round(" + current + " * 100.0) / 100.0", "0.0"),
                    (getter, "SysFreeString((BSTR)Source)", "keep(Source)")):
                with self.subTest(version=version, before=before), self.assertRaises(ValueError):
                    self.project(version=version, service=self.replaced(self.services[version], rva, before, after))

    def test_native_release_and_complete_owner_boundaries_fail(self):
        for rva, before, after in ((0xa280, "*(_QWORD *)v3 + 40LL", "*(_QWORD *)v3 + 88LL"),
                (0x5a70, "FreeLibrary(v4)", "keep(v4)"), (0xe990, "j_free(Block)", "keep(Block)")):
            with self.subTest(rva=rva), self.assertRaises(ValueError):
                self.project(sdk=self.replaced(self.sdk, rva, before, after))
        for field, value in (("assembly", []), ("chunks", []), ("rva", "0x627d1")):
            sdk = dict(self.sdk)
            sdk[0x627d0] = dict(sdk[0x627d0], **{field: value})
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.project(sdk=sdk)
        for field, value in (("q8_8_scale", 1 / 128), ("initial_current", 0), ("initial_minimum", 0)):
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.project(constants=dict(self.constants, **{field: value}))
        with self.assertRaises(ValueError):
            self.project(version="1.0.31.0", service=self.services["1.0.30.0"])

    def extraction_fixture(self):
        original = {"path": ".analysis/inputs/example/cpuidsdk64.dll", "bytes": 7, "sha256": "a" * 64,
            "container": ".analysis/inputs/example/package.exe", "container_sha256": "b" * 64}
        package = {"path": original["container"], "sha256": original["container_sha256"]}
        extraction = {"target_execution": False, "containers": [{"input": package["path"],
            "sha256": package["sha256"], "extracted": True, "extraction_exit_code": 0,
            "input_bytes_unchanged": True, "files": [dict(original)]}]}
        return original, package, extraction

    def test_extraction_metadata_extension_preserves_actual_identity(self):
        original, package, extraction = self.extraction_fixture()
        original["extraction_receipt"] = {"path": ".analysis/results/example/inspection.json", "sha256": "c" * 64}
        self.assertIs(families.hanbo_sensor_extraction_member(original, package, extraction), extraction["containers"][0])
        self.assertNotEqual(original, extraction["containers"][0]["files"][0])

    def test_extraction_changed_byte_or_parent_identity_missing_or_duplicate_fail(self):
        for key, value in (("path", ".analysis/inputs/other.dll"), ("bytes", 8), ("sha256", "d" * 64),
                ("container", ".analysis/inputs/other.exe"), ("container_sha256", "e" * 64)):
            original, package, extraction = self.extraction_fixture()
            with self.subTest(key=key), self.assertRaises(ValueError):
                families.hanbo_sensor_extraction_member(dict(original, **{key: value}), package, extraction)
        for kind in ("missing", "duplicate", "parent", "failed", "executed"):
            original, package, extraction = self.extraction_fixture()
            container = extraction["containers"][0]
            if kind == "missing": container["files"] = []
            elif kind == "duplicate": container["files"].append(dict(original))
            elif kind == "parent": container["sha256"] = "f" * 64
            elif kind == "failed": container["extraction_exit_code"] = 1
            else: extraction["target_execution"] = True
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                families.hanbo_sensor_extraction_member(original, package, extraction)


class DriverInstallSelectionTests(unittest.TestCase):
    def test_signed_version_boundaries_and_failed_version_are_distinct(self):
        for value, expected in ((None, "legacy"), (0x00060003, "legacy"), (0x0009ffff, "legacy"),
                (0x000a0000, "universal"), (0x000a0001, "universal"), (0x7fffffff, "universal"),
                (0x80000000, "legacy"), (0xffffffff, "legacy")):
            with self.subTest(value=value):
                actual = families.driver_install_selection_branch(value)
                self.assertEqual(actual["selected_branch"], expected)
                self.assertEqual(actual["version_observed"], value is not None)
                self.assertEqual(actual["first_extraction_entry"], 56 if expected == "legacy" else 39)
        for value in (True, -1, 0x100000000, "655360", 655360.0):
            with self.subTest(value=value), self.assertRaises(ValueError):
                families.driver_install_selection_branch(value)

    def test_nsis_compression_requires_complete_bounded_stream(self):
        import lzma
        payload = b"independent bounded file contents"
        filters = [{"id": lzma.FILTER_LZMA1, "dict_size": 4096, "lc": 3, "lp": 0, "pb": 2}]
        compressed = bytes([93]) + struct.pack("<I", 4096) + lzma.compress(payload, format=lzma.FORMAT_RAW,
                                                                                       filters=filters)
        raw = struct.pack("<I", len(compressed) | 0x80000000) + compressed
        actual, record = families.driver_install_nsis_chunk(raw, 0, len(raw))
        self.assertEqual(actual, payload)
        self.assertEqual(record["sha256"], hashlib.sha256(payload).hexdigest())
        plain = struct.pack("<I", len(payload)) + payload
        self.assertEqual(families.driver_install_nsis_chunk(plain, 0, len(plain))[0], payload)
        for candidate, maximum in ((raw[:-1], 4096), (raw, 3), (plain, 3),
                (struct.pack("<I", len(compressed) + 1 | 0x80000000) + compressed + b"!", 4096)):
            with self.subTest(candidate=candidate[:8], maximum=maximum), self.assertRaises(ValueError):
                families.driver_install_nsis_chunk(candidate, 0, len(candidate), maximum_output=maximum)

    def test_nsis_string_tokens_preserve_variable_and_shell_identity(self):
        strings = b"\x02$$\\System32\\kernel32.dll\0\x03\x8a\x80\0"
        self.assertEqual(families.driver_install_nsis_string(strings, 0),
            [{"kind": "shell", "index": 4644, "encoded": [36, 36]},
             {"kind": "literal", "value": "\\System32\\kernel32.dll"}])
        self.assertEqual(families.driver_install_nsis_string(strings, strings.index(b"\x03")),
            [{"kind": "variable", "index": 10, "encoded": [138, 128]}])
        for value, offset in ((b"\x03\x8a", 0), (b"text", 0), (b"\x04\0", 0), (b"a\0", -1)):
            with self.subTest(value=value), self.assertRaises(ValueError):
                families.driver_install_nsis_string(value, offset)

    def test_nsis_header_requires_exact_entry_and_string_boundaries(self):
        header = bytearray(100)
        for index, pair in enumerate(((68, 0), (68, 0), (68, 1), (96, 0), (100, 0), (0, 0), (0, 0), (0, 0))):
            struct.pack_into("<II", header, 4 + 8 * index, *pair)
        record = struct.pack("<I", len(header)) + header
        first = struct.pack("<7I", 2, 0xdeadbeef, 0x6c6c754e, 0x74666f73, 0x74736e49,
                            len(header), 28 + len(record))
        raw = first + record
        actual, strings = families.driver_install_nsis_header(raw, 0)
        self.assertEqual(actual["entry_count"], 1)
        self.assertEqual(strings, b"\0" * 4)
        for location, value in ((4, 0), (20, 101), (28 + 4 + 4 + 2 * 8 + 4, 2)):
            changed = bytearray(raw)
            struct.pack_into("<I", changed, location, value)
            with self.subTest(location=location), self.assertRaises(ValueError):
                families.driver_install_nsis_header(changed, 0)

    def test_current_manifest_scan_keeps_shared_consumers_and_acquisition_failures(self):
        import json
        package = "a" * 64
        rows, contents = [], {}
        for pid, target in ((106, package), (107, package), (108, "b" * 64)):
            row = {"url": f"https://apps.razer.com/synapse/products/{pid}/mw/manifest.json",
                   "http_status": 200, "path": f".analysis/inputs/manifest-{pid}.json"}
            contents[row["path"]] = json.dumps({"resourceManifest": {"resources": [{"sha256": target,
                "size": 2824776, "resourceName": "NormalDriver_PID006aDrv", "resourceVersion": "0.1.0.0",
                "url": "https://app-assets.razer.com/driver.exe", "action": {"saveToDisk": {}}}]}}).encode()
            rows.append(row)
        class Audit:
            def artifact(self, path, row):
                return contents[row["path"]]
        receipt = {"manifests": rows, "manifest_endpoint_count": 3, "complete": False,
                   "all_declared_resources_verified": False}
        actual = families.driver_install_selection_manifests(Audit(), receipt, package)
        self.assertEqual([row["product_id"] for row in actual["shared_binary_consumers"]], [106, 107])
        self.assertEqual(len(actual["all_retained_manifests"]), 3)
        self.assertFalse(actual["acquisition_complete"])
        for mutation in (lambda value: value["manifests"].append(copy.deepcopy(value["manifests"][0])),
                         lambda value: value["manifests"][0].update(http_status=404)):
            changed = copy.deepcopy(receipt)
            mutation(changed)
            with self.assertRaises(ValueError):
                families.driver_install_selection_manifests(Audit(), changed, package)

    def test_cli_routes_shared_binary_query_to_mouse_and_preserves_modes(self):
        import argparse
        parser = argparse.ArgumentParser()
        families.add_commands(parser.add_subparsers(dest="command"))
        args = parser.parse_args(["audit-family-function", "--family", "mouse", "--function", "mw",
            "--query", "driver-install-selection", "--native-install-selection", "negative"])
        self.assertEqual(args.native_install_selection, "negative")
        with patch.object(families, "audit_driver_install_selection", return_value="selected") as run:
            self.assertEqual(families.audit_family_function(args), "selected")
            run.assert_called_once_with(args)
        args.family = "keyboard"
        with self.assertRaisesRegex(ValueError, "catalog family is mouse"):
            families.audit_driver_install_selection(args)
        args.query = "keyboard-driver-request-lifecycle"
        with self.assertRaisesRegex(ValueError, "requires the driver-install-selection query"):
            families.audit_family_function(args)


class CommonDriverInstallSelectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.audit = host.HostLocalAudit()
        cls.report = families.collect_common_driver_install_selection(cls.audit)
        package = cls.report["provenance"]["official_package"]
        cls.raw = cls.audit.read(package)
        cls.header, cls.strings = families.driver_install_nsis_header(cls.raw, 0x9000)
        cls.bodies = {label: {int(row["rva"], 16): json.loads(
            (analysis.ROOT / row["artifact"]["path"]).read_bytes()) for row in native["functions"]}
            for label, native in cls.report["native"].items()}

    def proof(self, bodies=None):
        selected = self.bodies if bodies is None else bodies
        return families._common_driver_install_selection_proof(
            selected["installer"], selected["system_plugin"], selected["exec_plugin"])

    def test_current_static_result_preserves_independent_unverified_stages(self):
        report = self.report
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False, allow_nan=False)), report)
        self.assertEqual(report["kind"], "current_common_driver_install_selection")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertEqual([(row["all_functions"], row["selected_functions"])
                          for row in report["native_verification"].values()], [(105, 11), (62, 13), (17, 4)])
        self.assertTrue(report["selected_static_semantics_reviewed"])
        for name in ("source_chain_complete", "whole_product_complete", "installed_branch_observed",
                     "native_machine_qualified", "target_execution", "real_device_operations", "product_code_modified"):
            self.assertFalse(report[name])
        for name in ("ui_backend_wiring", "actual_dll_comparison", "runtime_acceptance"):
            self.assertEqual(report[name], "unverified")
        self.assertEqual((report["new_native_captures"], report["new_analysis_sessions"]), (0, 0))
        self.assertEqual(report["temporary_files_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_signed_version_failure_and_jump_boundaries(self):
        for value, branch in ((None, "legacy"), (0x00060003, "legacy"), (0x0009ffff, "legacy"),
                (0x000a0000, "universal"), (0x000a0001, "universal"), (0x7fffffff, "universal"),
                (0x80000000, "legacy"), (0xffffffff, "legacy")):
            with self.subTest(value=value):
                selected = families.common_driver_install_selection_branch(value)
                self.assertEqual(selected["selected_branch"], branch)
                self.assertEqual(selected["first_extraction_entry"], 44 if branch == "legacy" else 39)
                self.assertEqual(selected["version_observed"], value is not None)
        for value in (True, -1, 0x100000000, "655360", 655360.0):
            with self.subTest(value=value), self.assertRaises(ValueError):
                families.common_driver_install_selection_branch(value)

    def test_architecture_gate_ors_actual_outputs_without_native_qualification(self):
        for machine, wow64, expected in ((None, None, 0), (0, 0, 0), (0x14c, 0, 0x14c),
                                       (0, 1, 1), (0x14c, 1, 0x14d)):
            with self.subTest(machine=machine, wow64=wow64):
                result = families.common_driver_install_architecture_gate(machine, wow64)
                self.assertEqual(result["aggregate"], expected)
                self.assertEqual(result["dpinst_requested"], expected != 0)
                self.assertEqual(result["both_outputs_observed"], machine is not None and wow64 is not None)
                self.assertEqual(result["zero_jump_entry"], 85)
                self.assertFalse(result["native_machine_output_checked"])
                self.assertFalse(result["installed_architecture_verified"])
        for machine, wow64 in ((True, 0), (-1, 0), (0x10000, 0), (0, True), (0, 0x100000000)):
            with self.subTest(machine=machine, wow64=wow64), self.assertRaises(ValueError):
                families.common_driver_install_architecture_gate(machine, wow64)

    def test_both_official_sys_names_keep_distinct_retained_bytes_and_wdf_bindings(self):
        branches = self.report["selection"]["branches"]
        self.assertEqual([row["entry_range"] for row in branches], [[39, 42], [44, 48]])
        self.assertTrue(all(row["continuation_entry"] == 49 for row in branches))
        files = [next(row for row in branch["files"] if row["filename"] == "RzCommon.sys") for branch in branches]
        self.assertEqual([row["data_relative_offset"] for row in files], [930991, 975111])
        self.assertNotEqual(files[0]["block"]["sha256"], files[1]["block"]["sha256"])
        self.assertTrue(files[0]["retained_original"]["path"].endswith("/RzCommon.sys"))
        self.assertTrue(files[1]["retained_original"]["path"].endswith("/RzCommon_1.sys"))
        drivers = self.report["driver_branches"]
        self.assertEqual([(row["file_version"], row["wdf_binding_version"], row["wdf_function_count"],
                           row["functions_captured"]) for row in drivers],
                         [("1.0.0.76", [1, 15, 0], 444, 121), ("1.0.0.71", [1, 9, 7600], 396, 105)])
        self.assertTrue(all(row["capture_complete"] and not row["capture_failures"] for row in drivers))
        self.assertTrue(all(not row["whole_driver_semantics_reviewed_by_this_query"] for row in drivers))

    def test_inf_contract_and_wide_version_apis_are_actual_package_boundaries(self):
        infs = self.report["inf_contracts"]
        self.assertEqual([row["coinstaller_for_nt61"] for row in infs], [False, True])
        self.assertTrue(all(row["hardware_id"] == "RAZER\\DeviceContol" for row in infs))
        self.assertTrue(all(row["service_binary"] == "%12%\\RzCommon.sys" for row in infs))
        self.assertEqual([(row["table_index"], row["symbol"]) for row in self.report["version_api_table"]],
                         [(10, "GetFileVersionInfoSizeW"), (11, "GetFileVersionInfoW"), (12, "VerQueryValueW")])
        proof = self.proof()
        self.assertEqual(len(proof["instruction_anchors"]), 9)
        self.assertEqual(len(proof["actual_edges"]), 20)
        self.assertTrue(proof["positive_stack_warning_retained"])
        self.assertEqual(proof["system_call_bridge_rva"], "0x2b98")
        self.assertEqual(proof["dynamic_abi_execution"], "unverified")

    def test_exec_text_coercion_reboot_bit_and_absence_do_not_prove_success(self):
        for text, numeric, reboot in (("0", 0, False), ("1073741824", 0x40000000, True),
                ("2147483648", 0x80000000, False), ("-1", 0xffffffff, True),
                ("error", 0, False), ("timeout", 0, False), ("0x40000000tail", 0x40000000, True),
                ("010", 8, False), ("08", 8, False), ("4294967296", 0, False), ("+1", 0, False), (" 1", 0, False)):
            with self.subTest(text=text):
                result = families.common_driver_install_result(text)
                self.assertEqual(result["stack_text"], text)
                self.assertTrue(result["stack_value_observed"])
                self.assertEqual(result["numeric_u32"], numeric)
                self.assertEqual(result["reboot_bit"], 0x40000000 if reboot else 0)
                self.assertEqual(result["requested_exit_code"], 3010 if reboot else None)
                self.assertFalse(result["installation_success_proven"])
        absent = families.common_driver_install_result(None)
        self.assertFalse(absent["stack_value_observed"])
        self.assertIsNone(absent["numeric_u32"])
        self.assertIsNone(absent["reboot_bit"])
        for invalid in (True, 0, 3010, b"0"):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                families.common_driver_install_result(invalid)

    def test_changed_signed_jump_architecture_target_and_result_mask_are_rejected(self):
        for index, operand, value in ((38, 6, 1), (38, 4, 44), (79, 3, 85), (87, 3, 281), (88, 4, 90)):
            header = copy.deepcopy(self.header)
            header["entries"][index][operand] = value
            with self.subTest(index=index, operand=operand), \
                    patch.object(families, "driver_install_nsis_header", return_value=(header, self.strings)), \
                    self.assertRaisesRegex(ValueError, "selected instruction differs"):
                families.common_driver_install_selection_snapshot(self.raw)

    def test_changed_unicode_threshold_signature_or_command_are_rejected(self):
        original = host.nsis_unicode_string
        for offset in (355, 724, 764, 824):
            def changed(strings, selected, target=offset):
                result = original(strings, selected)
                return [{"kind": "literal", "value": "changed"}] if selected == target else result
            with self.subTest(offset=offset), patch.object(host, "nsis_unicode_string", side_effect=changed), \
                    self.assertRaisesRegex(ValueError, "selected string differs"):
                families.common_driver_install_selection_snapshot(self.raw)

    def test_changed_current_release_package_identity_cannot_start_native_review(self):
        release = copy.deepcopy(host.verified_host_release_receipt())
        package = next(row for row in release["files"] if row["path"].endswith("/RzComDriver_v23.0.6.0.exe"))
        package["sha256"] = "0" * 64
        with patch.object(host, "verified_host_release_receipt", return_value=release), \
                patch.object(families, "driver_install_selection_native_reference") as native, \
                self.assertRaisesRegex(ValueError, "requires source and semantic review"):
            families.collect_common_driver_install_selection(host.HostLocalAudit())
        native.assert_not_called()

    def test_changed_extraction_execution_or_member_identity_are_rejected(self):
        reference = self.report["provenance"]["extraction_receipt"]
        for kind in ("target", "list_failure", "member"):
            audit = host.HostLocalAudit()
            original = audit.artifact
            def changed(path, expected=None):
                raw = original(path, expected)
                if analysis.rel(path) != reference["path"]:
                    return raw
                document = json.loads(raw)
                if kind == "target":
                    document["target_execution"] = True
                elif kind == "list_failure":
                    document["containers"][0]["list_exit_code"] = 1
                else:
                    for row in document["containers"][0]["files"]:
                        row["sha256"] = "0" * 64
                return json.dumps(document).encode()
            with self.subTest(kind=kind), patch.object(audit, "artifact", side_effect=changed), \
                    self.assertRaisesRegex(ValueError, "extraction is incomplete|retained member identity differs"):
                families._common_driver_install_selection_inputs(audit)

    def test_changed_complete_owner_semantics_are_rejected(self):
        for label, rva in (("installer", 0x1434), ("installer", 0x6467),
                           ("system_plugin", 0x2655), ("exec_plugin", 0x1096)):
            bodies = copy.deepcopy(self.bodies)
            bodies[label][rva]["pseudocode"]["lines"][0]["text"] += " changed"
            with self.subTest(label=label, rva=rva), self.assertRaisesRegex(ValueError, "owner differs"):
                self.proof(bodies)

    def test_changed_actual_signed_instruction_and_call_edges_are_rejected(self):
        bodies = copy.deepcopy(self.bodies)
        line = next(row for body in bodies["installer"].values() for row in body["assembly"]
                    if row["rva"] == "0x1a92")
        line["bytes"] = "7d08"
        with self.assertRaisesRegex(ValueError, "interpreter instruction differs"):
            self.proof(bodies)
        for label, owner, address in (("installer", 0x1389, "0x401434"),
                ("system_plugin", 0x1817, "0x10002b98"), ("exec_plugin", 0x1000, "0x10001096")):
            bodies = copy.deepcopy(self.bodies)
            for row in bodies[label][owner]["assembly"]:
                row["xrefs_from"] = [xref for xref in row.get("xrefs_from", []) if xref["to"] != address]
            with self.subTest(label=label), self.assertRaisesRegex(ValueError, "actual owner edge differs"):
                self.proof(bodies)


if __name__ == "__main__":
    unittest.main()

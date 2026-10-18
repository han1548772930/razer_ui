"""Pure source-owner and semantic-boundary tests for family audits."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import analysis
import families
import host


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


if __name__ == "__main__":
    unittest.main()

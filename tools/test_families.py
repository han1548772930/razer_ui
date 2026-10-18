"""Pure source-owner and semantic-boundary tests for family audits."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import analysis
import families
import host


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


if __name__ == "__main__":
    unittest.main()

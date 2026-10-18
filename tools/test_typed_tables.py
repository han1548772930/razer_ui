"""Reject source-table ambiguity before fixed protocol data enters Rust."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import typed_tables


def catalog():
    return {"schema_version": 1, "products": [{
        "product_id": 124, "source_class": "rzDevice25DualLinkMouse",
        "min_dpi": 100, "max_dpi": 20000,
        "polling_codes": {"RATE_1000Hz": 1},
        "writes": [{"kind": "polling", "method": "setProfilePollingRate", "command": [2, 0, 14],
                    "selector": 1, "read_kind": "polling", "min_response_bytes": 2}],
    }]}


class TypedTableTests(unittest.TestCase):
    @staticmethod
    def battery_peer_manifest():
        return {"productId":226,"vendorId":5426,"wiredProductId":226,"dongleProductId":227,"bleProductId":228,
            "category":"MOUSE","supportBatteryLevelMonitoring":True,"supportDockingStationV2":True,
            "rzDeviceConfig":{"claimInterface":3,"maxRetryIn":30,"maxRetryOut":20,"rzDeviceName":"rzDevice25",
                "sleepTimeBetweenIn":5,"sleepTimeBetweenOut":5,"sleepTimeBetweenOutIn":5},
            "quickEffectOnExit":{"effectId":1,"params":{"color":"#a3e01f"}},
            "chargingEffect":{"full":{"min":100,"max":100,"effectId":6,"effectParams":{"color":65280}},
                "high":{"min":75,"max":99,"effectId":5,"effectParams":{"color":65280}},
                "mid":{"min":50,"max":74,"effectId":5,"effectParams":{"color":16776960}},
                "low":{"min":25,"max":49,"effectId":5,"effectParams":{"color":16729344}},
                "critical":{"min":0,"max":24,"effectId":5,"effectParams":{"color":16711680}}},
            "ledConfig":{"ledConfig_128_0":{"Version":2,"VID":5426,"PID":226,"EID":128,"Layout":0,
                "MatrixMaxRow":1,"MatrixMaxCol":2,"DeviceMaxRow":1,"DeviceMaxCol":2,"LedWidth":40,"LedHeight":40,
                "LedWidthOffset":0,"LedHeightOffset":0,"MaxRegion":0,"RegionMaxRow":0,"RegionMaxCol":0,
                "LedMatrix":[{"MatrixPos":[0,0],"DevicePos":[0,1]},{"MatrixPos":[0,1],"DevicePos":[0,0]}],
                "LedRegion":[{"RegionId":1,"NumLed":1,"MaxRow":1,"MaxCol":1,"RegionMatrix":[{"MatrixPos":[0,1]}]}],
                "LedInputMap":[{"InputType":"dkm","InputData":[81,16],"MatrixPos":[65535,65535]}]}}}

    def test_battery_peer_projection_preserves_declared_pid_optional_flags_geometry_and_exit(self):
        manifest = self.battery_peer_manifest()
        row = typed_tables.receiver_battery_peer_projection(manifest,227,128,{"selectedEffectId":3})
        self.assertEqual(row["support"]["source_product_id"],227)
        self.assertEqual(row["support"]["manifest_product_id"],226)
        self.assertFalse(row["support"]["is_dual_link"])
        self.assertIsNone(row["support"]["send_chroma_frames_to_dock"])
        self.assertEqual(row["support"]["quick_effect_on_exit"],{"_type":"PeerExitEffect::Static","color":[163,224,31]})
        self.assertEqual(row["support"]["default_profile_effect"],"PeerProfileEffect::Spectrum")
        self.assertEqual(row["geometries"][0]["led_input_map"][0]["matrix_pos"],[65535,65535])
        self.assertEqual(row["charging_effects"]["mid"]["color"],16776960)
        for mutate in (
                lambda m:m["rzDeviceConfig"].update(unknown=True),
                lambda m:m.update(isDualLink=1),
                lambda m:m["quickEffectOnExit"]["params"].update(speed=1),
                lambda m:m["ledConfig"]["ledConfig_128_0"].update(EID=0),
                lambda m:m["ledConfig"]["ledConfig_128_0"]["LedInputMap"][0].update(InputType="unknown"),
                lambda m:m["chargingEffect"]["low"].update(min=90)):
            changed = copy.deepcopy(manifest)
            mutate(changed)
            with self.assertRaises(ValueError):
                typed_tables.receiver_battery_peer_projection(changed,227,128,{"selectedEffectId":3})

    def test_battery_peer_default_profile_effect_preserves_current_mw_color_and_rejects_unresolved_fields(self):
        self.assertEqual(typed_tables.receiver_battery_peer_default_effect({"selectedEffectId":3}),
                         "PeerProfileEffect::Spectrum")
        self.assertEqual(typed_tables.receiver_battery_peer_default_effect(
            {"selectedEffectId":1,"selectedEffectSetting":{"color1":"#a3e01f"}}),
            {"_type":"PeerProfileEffect::Static","color":[163,224,31]})
        for invalid in ({}, {"selectedEffectId":True}, {"selectedEffectId":3,"unknown":1},
                {"selectedEffectId":1,"selectedEffectSetting":{"color1":"#a3e01f","speed":1}},
                {"selectedEffectId":1,"selectedEffectSetting":{"color1":"green"}},
                {"effectId":1,"params":{"color":"#a3e01f"}}):
            with self.assertRaises(ValueError):
                typed_tables.receiver_battery_peer_default_effect(invalid)

    def test_battery_peer_rust_equivalence_rejects_changed_support_regions_inputs_and_effects(self):
        row = typed_tables.receiver_battery_peer_projection(self.battery_peer_manifest(),227,128,
            {"selectedEffectId":1,"selectedEffectSetting":{"color1":"#a3e01f"}})
        rows = [row]
        raw = typed_tables.render_receiver_battery_peers(rows)
        self.assertTrue(typed_tables.verify_receiver_battery_peers(raw,rows)["all_fields_and_order_equal"])
        for before, after in ((b"manifest_product_id: 226",b"manifest_product_id: 227"),
                (b"claim_interface: 3",b"claim_interface: 0"),
                (b"PeerDeviceFactory::RzDevice25",b"PeerDeviceFactory::RzDevice30"),
                (b"RegionMatrix { matrix_pos: [0, 1] }",b"RegionMatrix { matrix_pos: [0, 0] }"),
                (b"matrix_pos: [65535, 65535]",b"matrix_pos: [0, 0]"),
                (b"color: 16776960",b"color: 255"),
                (b"PeerProfileEffect::Static",b"PeerExitEffect::Static"),
                (b"[163, 224, 31]",b"[31, 224, 163]"),
                (b"(227,128,0)",b"(226,128,0)")):
            self.assertIn(before,raw)
            with self.assertRaises(ValueError):
                typed_tables.verify_receiver_battery_peers(raw.replace(before,after),rows)

    def test_receiver_sender_table_uses_mounted_selector_and_keeps_other_factories(self):
        raw = b'''fn products(){vec![ReceiverCapability{source_product_id:241,product_id:241,
            claim_interface:3,report_id:0,source_class:"rzDevice25LinkerMultiDevices".into()},
            PairingCapability{transport:ReceiverCapability{source_product_id:756,product_id:241,
            claim_interface:3,report_id:0,source_class:"rzDevice25".into()}}]}'''
        bindings = {241:{"source_class":"rzDevice25LinkerMultiDevices","claim_interface":3,"report_id":3}}
        with self.assertRaisesRegex(ValueError,"report_id differs"):
            typed_tables.receiver_sender_table_projection(raw,bindings)
        updated,rows=typed_tables.receiver_sender_table_projection(raw,bindings,update=True)
        self.assertEqual(updated.count(b"report_id:3"),1)
        self.assertEqual(updated.count(b"report_id:0"),1)
        self.assertEqual(rows,[{"source_product_id":241,"product_id":241,
            "source_class":"rzDevice25LinkerMultiDevices","claim_interface":3,"report_id":3}])
        self.assertEqual(typed_tables.receiver_sender_table_projection(updated,bindings)[0],updated)
        for invalid in (updated.replace(b"claim_interface:3",b"claim_interface:0",1),
                        updated.replace(b"rzDevice25LinkerMultiDevices",b"rzDevice25",1),
                        updated.replace(b"report_id:3",b"report_id:3,report_id:3",1)):
            with self.assertRaises(ValueError):
                typed_tables.receiver_sender_table_projection(invalid,bindings)

    @staticmethod
    def receiver_179_namespace_sources():
        def factory(prefix, rewrite):
            sequence = str(prefix) + "|this.transactionId++" if prefix else "this.transactionId++"
            return ("class Device{_getTransactionId(){return 31===this.transactionId&&(this.transactionId=0),"
                    + sequence + "}sendCommand(header){const frame=this._createDataSend(header,[]);"
                    "if(header[2]!==commands.pJ[2]&&header[2]!==commands.fr[2]||(frame[1]"
                    + rewrite + "),this.isElectron){}}}")
        return {"protocol_common_device": "class{constructor(){this.transactionId=0}}",
            "protocol_pairing_mouse_category": factory(128, "&=31"),
            "protocol_pairing_keyboard_category": factory(0, "|=128"),
            "linker_factory": factory(224, "&=31"),
            "pairing_factory_V": "V=require(48320)", "pairing_factory_z": "z=require(14770)",
            "mw_pairing_factory_selection": "select=(category,mode=!1)=>{const selected="
                "null===(temp={true:{[types.KEYBOARD]:V.default,[types.MOUSE]:z.default},"
                "false:{[types.MOUSE]:V.default,[types.KEYBOARD]:z.default}}[String(mode)])"
                "||void 0===temp?void 0:temp[category];return new selected(info)}"}

    def test_receiver_179_namespaces_derive_fresh_category_factories_and_primary_cancel(self):
        sources = self.receiver_179_namespace_sources()
        contract = typed_tables.receiver_pairing_namespace_contract(sources)
        self.assertEqual(contract["categories"], typed_tables.RECEIVER_179_PAIRING_NAMESPACES)
        self.assertEqual(contract["primary_prefix"], 224)
        self.assertEqual(contract["category_modules"]["true"], {"KEYBOARD":48320,"MOUSE":14770})
        for role, before, after in (
                ("protocol_common_device", "transactionId=0", "transactionId=1"),
                ("protocol_pairing_mouse_category", "128|this.transactionId++", "this.transactionId++"),
                ("protocol_pairing_keyboard_category", "|=128", "&=31"),
                ("linker_factory", "&=31", "|=128"),
                ("protocol_pairing_keyboard_category", "commands.fr[2]", "commands.other[2]"),
                ("protocol_pairing_mouse_category", "31===this.transactionId", "32===this.transactionId"),
                ("pairing_factory_V", "48320", "14770"),
                ("mw_pairing_factory_selection", "String(mode)", "String(category)"),
                ("mw_pairing_factory_selection", "temp[category]", "temp[mode]"),
                ("mw_pairing_factory_selection", "new selected", "new other"),
                ("mw_pairing_factory_selection", "[types.KEYBOARD]:V.default,[types.MOUSE]:z.default",
                    "[types.KEYBOARD]:z.default,[types.MOUSE]:V.default")):
            changed = dict(sources)
            self.assertIn(before, changed[role])
            changed[role] = changed[role].replace(before, after, 1)
            with self.assertRaises(ValueError, msg=role + " " + before):
                typed_tables.receiver_pairing_namespace_contract(changed)

    def test_receiver_179_namespace_projection_updates_both_branches_and_preserves_other_owners(self):
        contract = typed_tables.receiver_pairing_namespace_contract(self.receiver_179_namespace_sources())
        raw = b'''fn products(){vec![PairingCapability{product_id:179,category:"MOUSE".into(),
            scan_pair_prefix:128,unpair_prefix:128,cancel_prefix:128,metadata_prefix:0},
            PairingCapability{product_id:179,category:"KEYBOARD".into(),
            scan_pair_prefix:0,unpair_prefix:0,cancel_prefix:128,metadata_prefix:128},
            PairingCapability{product_id:164,category:"MOUSE".into(),scan_pair_prefix:0,
            unpair_prefix:224,cancel_prefix:0,metadata_prefix:0}]}'''
        with self.assertRaisesRegex(ValueError, "differs from its current source owner"):
            typed_tables.receiver_pairing_namespace_table_projection(raw,contract)
        updated, rows = typed_tables.receiver_pairing_namespace_table_projection(raw,contract,update=True)
        self.assertEqual(len(rows),2)
        self.assertIn(b"unpair_prefix:224,cancel_prefix:0,metadata_prefix:0",updated)
        self.assertEqual(typed_tables.receiver_pairing_namespace_table_projection(updated,contract)[0],updated)
        for before, after in ((b'scan_pair_prefix:128',b'scan_pair_prefix:0'),
                              (b'cancel_prefix:0',b'cancel_prefix:128'),
                              (b'category:"KEYBOARD"',b'category:"MOUSE"'),
                              (b'category:"KEYBOARD"',b'category:"OTHER"'),
                              (b'product_id:179,category:"KEYBOARD"',b'product_id:164,category:"KEYBOARD"')):
            with self.assertRaises(ValueError):
                typed_tables.receiver_pairing_namespace_table_projection(updated.replace(before,after,1),contract)

    @staticmethod
    def transfer_factory(fields=None, export="DeviceInfo:()=>info"):
        return ('42:(module,exports,require)=>{require.d(exports,{' + export + '});'
                'const deviceName="A \\"quoted\\" keyboard",base={productId:756,category:"KEYBOARD"},'
                'info={' + (fields if fields is not None else
                '...base,dongleId:757,bleId:758,is8kAnalogDevice:!0,deviceName') + '};}')

    def test_receiver_reset_check_compares_complete_product_fields_and_output_bytes(self):
        source = json.loads((typed_tables.ROOT / typed_tables.RESET_INPUT).read_bytes())
        with tempfile.TemporaryDirectory() as directory, patch.object(typed_tables, "ROOT", Path(directory)):
            root = Path(directory)
            path = root / typed_tables.RESET_INPUT
            output = root / typed_tables.RESET_OUTPUT
            path.parent.mkdir(parents=True)
            output.parent.mkdir(parents=True)
            raw = json.dumps(source).encode()
            path.write_bytes(raw)
            output.write_bytes(typed_tables.render_reset_storage(raw))
            result = typed_tables.check_receiver_reset_storage()
            self.assertEqual(result["products"], [164, 241])
            changed = copy.deepcopy(source)
            changed["products"][1]["default_profile"]["brightness"]["value"] = 37
            path.write_bytes(json.dumps(changed).encode())
            with self.assertRaisesRegex(ValueError, "differs from complete source defaults"):
                typed_tables.check_receiver_reset_storage()
            path.write_bytes(raw)
            output.write_bytes(output.read_bytes() + b" ")
            with self.assertRaisesRegex(ValueError, "differs from complete source defaults"):
                typed_tables.check_receiver_reset_storage()
            for field, value in (("canPairTwoDevices", False), ("unknown", True)):
                changed = copy.deepcopy(source)
                changed["products"][1]["default_profile"][field] = value
                with self.assertRaises(ValueError):
                    typed_tables.render_reset_storage(json.dumps(changed).encode())

    def test_transfer_projection_resolves_fixed_alias_spread_shorthand_and_getter_forms(self):
        source = self.transfer_factory()
        projected, evidence = typed_tables.profile_transfer_source_projection(source)
        self.assertEqual(projected, {"productId":756,"dongleId":757,"bleId":758,
            "category":"KEYBOARD","is8kAnalogDevice":True,"deviceName":'A "quoted" keyboard'})
        self.assertFalse(evidence["absent_optional_fields"])
        for binding in evidence["bindings"]:
            self.assertEqual(source.encode()[binding["byte_start"]:binding["byte_end"]].decode(), binding["body"])
            self.assertEqual(hashlib.sha256(binding["body"].encode()).hexdigest(), binding["body_sha256"])
        old_getter, _ = typed_tables.profile_transfer_source_projection(
            self.transfer_factory(export="DeviceInfo:function(){return info}"))
        self.assertEqual(old_getter, projected)

    def test_transfer_projection_retains_verified_equal_duplicate_fields_and_sequence_result(self):
        fields = '...base,category:"KEYBOARD",deviceName,deviceName:deviceName'
        projected, evidence = typed_tables.profile_transfer_source_projection(self.transfer_factory(fields))
        self.assertEqual(projected["category"],"KEYBOARD")
        self.assertEqual([row["field"] for row in evidence["equal_duplicate_properties"]],["category","deviceName"])
        source = self.transfer_factory().replace('info={','info=(require.p,{').replace('};}', '});}')
        projected, evidence = typed_tables.profile_transfer_source_projection(source)
        self.assertEqual(projected["productId"],756)
        self.assertEqual(len(evidence["sequence_results"]),1)
        with self.assertRaises(ValueError):
            typed_tables.profile_transfer_source_projection(source.replace('require.p,','sideEffect(),'))

    def test_transfer_projection_resolves_actual_imported_enum_owner(self):
        factory = self.transfer_factory().replace('const deviceName=', 'const enums=require(43);const deviceName=')
        factory = factory.replace('category:"KEYBOARD"','category:enums.Kind.KEYBOARD')
        enum = '43:(module,exports,require)=>{require.d(exports,{Kind:()=>kind});const kind={KEYBOARD:"KEYBOARD",AUDIO:"AUDIO"};}'
        raw = ('(function(){const modules={' + factory + ',' + enum + '};})();').encode()
        owners, modules = typed_tables.profile_transfer_source_owners(raw)
        self.assertEqual(len(owners),1)
        projection, evidence = typed_tables.profile_transfer_source_projection(owners[0]["body"],modules=modules)
        self.assertEqual(projection["category"],"KEYBOARD")
        self.assertEqual(evidence["imported_fixed_fields"][0]["owner"]["module_id"],43)
        with self.assertRaises(ValueError):
            typed_tables.profile_transfer_source_projection(owners[0]["body"],modules={})

    def test_transfer_source_owner_is_the_inner_complete_concatenated_entry(self):
        raw = ('(()=>{const nested=()=>{return 1};(()=>{const exports={};require.d(exports,{DeviceInfo:()=>info});'
               'const info={productId:769,category:"HUE"};})();})();').encode()
        owners, _ = typed_tables.profile_transfer_source_owners(raw)
        self.assertEqual(len(owners),1)
        owner = owners[0]
        self.assertEqual(raw[owner["byte_start"]:owner["byte_end"]].decode(),owner["body"])
        self.assertTrue(owner["body"].startswith('()=>{const exports={}'))
        projection, _ = typed_tables.profile_transfer_source_projection(owner["body"])
        self.assertEqual(projection,{"productId":769,"category":"HUE"})

    def test_transfer_projection_preserves_absent_optional_fields_without_inventing_names(self):
        projected, evidence = typed_tables.profile_transfer_source_projection(
            self.transfer_factory('...base'))
        self.assertEqual(projected, {"productId":756,"category":"KEYBOARD"})
        self.assertEqual(evidence["absent_optional_fields"], ["bleId","deviceName","dongleId","is8kAnalogDevice"])
        self.assertEqual(typed_tables.profile_transfer_semantics(projected), {
            "product_id":756,"dongle_id":None,"ble_id":None,"category":"Keyboard","is_eight_k_analog_device":False})

    def test_transfer_projection_rejects_missing_dynamic_conflicting_and_invalid_fields(self):
        for fields in ('productId:756', 'category:"KEYBOARD"',
                       '...base,productId:100', '...base,bleId:query()',
                       'productId:756,category:"UNKNOWN"', '...base,is8kAnalogDevice:1',
                       '...base,dongleId:-1', '...base,deviceName:""',
                       '...unknown,deviceName', '...base,["dongleId"]:757'):
            with self.subTest(fields=fields), self.assertRaises(ValueError):
                typed_tables.profile_transfer_source_projection(self.transfer_factory(fields))
        with self.assertRaises(ValueError):
            typed_tables.profile_transfer_source_projection(
                self.transfer_factory(export="DeviceInfo:function(){sideEffect();return info}"))

    def test_transfer_constructor_roundtrip_checks_every_field_and_order(self):
        projection, _ = typed_tables.profile_transfer_source_projection(self.transfer_factory())
        writer = {**projection,"dongleId":759}
        products = [{"mounted_product_id":756,"projection":projection,"export_projection":writer,
                     "export_rule":"ProfilesWithSelectedMacros","fixed_export_name":projection["deviceName"]}]
        rendered = typed_tables.render_profile_transfer_products(products)
        self.assertTrue(typed_tables.verify_profile_transfer_constructor(rendered,products)["all_fields_and_order_equal"])
        for old, new in ((b"mounted_product_id: 756",b"mounted_product_id: 700"),
                         (b"product_id: 756",b"product_id: 700"),
                         (b"dongle_id: Some(757)",b"dongle_id: None"),
                         (b"ble_id: Some(758)",b"ble_id: None"),
                         (b"dongle_id: Some(759)",b"dongle_id: Some(757)"),
                         (b"TransferCategory::Keyboard",b"TransferCategory::Mouse"),
                         (b"is_eight_k_analog_device: true",b"is_eight_k_analog_device: false"),
                         (b"ProfileExportRule::ProfilesWithSelectedMacros",b"ProfileExportRule::ProfilesOnly"),
                         (b"device_name: Some(",b"device_name: None, unknown: Some(")):
            with self.subTest(old=old), self.assertRaises(ValueError):
                typed_tables.verify_profile_transfer_constructor(rendered.replace(old,new),products)
        absent = [{"mounted_product_id":756,"projection":{"productId":756,"category":"KEYBOARD"},
                   "export_projection":{"productId":756,"category":"KEYBOARD"},"export_rule":"ProfilesOnly"}]
        typed_tables.verify_profile_transfer_constructor(typed_tables.render_profile_transfer_products(absent),absent)
        for invalid in ([], products + products, [products[0], {"mounted_product_id":70,"projection":projection,"export_projection":projection}]):
            with self.assertRaises(ValueError):
                typed_tables.render_profile_transfer_products(invalid)

    def test_transfer_export_name_requires_the_actual_file_producer(self):
        def factory(expression):
            return ('1:(module,exports,require)=>{function exportProfiles(){return run(function*(){'
                    'const file={productId:product.DeviceInfo.productId,deviceName:' + expression + ','
                    'category:product.DeviceInfo.category,profiles:[],macros:[]};'
                    'emit({actionType:"taskMakerExportProfile",status:"completed"});});}}').encode()
        for expression, mode in (("product.DeviceInfo.deviceName","middleware_device_info"),
                                 ("labels.current.getCurLangNameData().productName","current_localized_product_name")):
            raw = b"(self.webpackChunk=self.webpackChunk||[]).push([[1],{" + factory(expression) + b"}]);"
            capture = typed_tables.profile_export_name_source(raw)
            self.assertEqual(capture["name_source"],mode)
            body = capture["function"]
            self.assertEqual(raw[body["byte_start"]:body["byte_end"]].decode(),body["body"])
            self.assertEqual(hashlib.sha256(body["body"].encode()).hexdigest(),body["sha256"])
        raw = b"(self.webpackChunk=self.webpackChunk||[]).push([[1],{" + factory("otherDevice.DeviceInfo.deviceName") + b"}]);"
        with self.assertRaises(ValueError):
            typed_tables.profile_export_name_source(raw)

    def test_transfer_export_anonymous_function_retains_concatenated_writer_linkage(self):
        raw = ('(()=>{const exports={};require.d(exports,{DeviceInfo:()=>info});'
               'const info={productId:162,category:"MOUSE"};'
               'const exportProfiles=()=>run(function*(){const file={productId:info.productId,'
               'deviceName:labels.getCurLangNameData().productName,category:info.category,profiles:[],macros:[]};'
               'emit({actionType:"taskMakerExportProfile"});});})();').encode()
        owners, modules = typed_tables.profile_transfer_source_owners(raw)
        _, evidence = typed_tables.profile_transfer_source_projection(owners[0]["body"],modules=modules)
        captured = typed_tables.profile_export_name_source(raw,device_info_owner=owners[0],
            device_info_binding=evidence["exported_binding"])
        self.assertEqual(captured["verified_device_info_linkage"],{"kind":"same_complete_owner","binding":"info"})
        with self.assertRaisesRegex(ValueError,"binding does not resolve"):
            typed_tables.profile_export_name_source(raw,device_info_owner=owners[0],device_info_binding="other")

    @staticmethod
    def transfer_writer(macro_mode="skip_combined"):
        macro = ('row.listItems.forEach(choice=>{if(choice.isMacroSelected){'
                 'const macro=localMacros.find(item=>item.guid===choice.guid);'
                 + ('if(!macro)return;' if macro_mode == "skip_separate" else '')
                 + 'if(' + ('macro&&' if macro_mode == "skip_combined" else '')
                 + '-1===selectedMacros.findIndex(item=>item.guid===macro.guid)){'
                 'const projected={guid:macro.guid,macroEvents:macro.macroEvents,macroPlaybackOption:macro.macroPlaybackOption},'
                 'payload=JSON.stringify(projected),encoded=Buffer.from(payload).toString("base64");'
                 'selectedMacros.push({name:macro.name,payload:encoded,hash:hash(projected)})}}});')
        return ('function*(){const profiles=JSON.parse(JSON.stringify(cache.profiles)),exported=[],selectedMacros=[],localMacros=load();'
                'selected.forEach(row=>{const profile=JSON.parse(JSON.stringify(profiles.find(item=>item.guid===row.id)));'
                'delete profile.appEngine;const payload=JSON.stringify(profile),encoded=Buffer.from(payload).toString("base64");'
                'exported.push({name:profile.name,payload:encoded,hash:hash(profile)});'
                + (macro if macro_mode != "none" else '')
                + '});const file={productId:p.DeviceInfo.productId,deviceName:p.DeviceInfo.deviceName,'
                'category:p.DeviceInfo.category,profiles:exported'
                + (',macros:selectedMacros' if macro_mode != "none" else '')
                + '};emit({actionType:"taskMakerExportProfile"});}')

    def test_transfer_writer_payload_projection_preserves_all_three_macro_failure_branches(self):
        for mode, rule in (("skip_combined","ProfilesWithSelectedMacros"),
                           ("skip_separate","ProfilesWithSelectedMacros"),
                           ("required","ProfilesWithRequiredSelectedMacros"),
                           ("none","ProfilesOnly")):
            with self.subTest(mode=mode):
                result = typed_tables.profile_export_payload_semantics(self.transfer_writer(mode))
                self.assertEqual(result["export_rule"],rule)
                self.assertEqual(result["missing_profile"],"fail entire export")
        source = self.transfer_writer()
        for old, new in (('delete profile.appEngine','delete profile.name'),
                         ('delete profile.appEngine;','delete profile.appEngine;profile.name="mutated";'),
                         ('item.guid===row.id','item.guid===row.name'),
                         ('hash:hash(profile)','hash:hash(cache)'),
                         ('JSON.stringify(profile),encoded','JSON.stringify(cache),encoded'),
                         ('macroPlaybackOption:macro.macroPlaybackOption','macroPlaybackOption:macro.macroPlaybackOption,extra:macro.name'),
                         ('choice.isMacroSelected','choice.isSelected'),
                         ('item.guid===macro.guid','item.guid===macro.name'),
                         ('row.listItems.forEach','other.listItems.forEach'),
                         ('item.guid===choice.guid','item.guid===choice.id')):
            with self.subTest(old=old), self.assertRaises(ValueError):
                typed_tables.profile_export_payload_semantics(source.replace(old,new))

    def transfer_acquisition_fixture(self, root):
        registry = root / ".analysis/inputs/typed-tables/razer-catalog/product_registry.json"
        registry.parent.mkdir(parents=True)
        registry.write_text(json.dumps({"products":[{"id":756}]}),encoding="utf-8")
        (root / "tools").mkdir()
        for name in ("typed_tables.py","families.py","workspace.py"):
            (root / "tools" / name).write_text("# test maintenance tool\n",encoding="utf-8")
        receipts = []
        for role in ("ui","mw"):
            base = f"https://apps.razer.com/synapse/products/756/{role}/"
            directory = root / f".analysis/inputs/frontend/test-{role}-756-current"
            directory.mkdir(parents=True)
            main_name = ("static/js/" if role == "ui" else "") + "main.123abc.js"
            export = ('99:(module,exports,require)=>{function exportProfiles(){return run('
                      + self.transfer_writer() + ');}const p=require(42);}')
            main = ('(self.webpackChunk=self.webpackChunk||[]).push([[1],{' + self.transfer_factory()
                    + ("," + export if role == "mw" else "") + '}]);').encode()
            rows = []
            for name, content in (("index.html",f'<script src="{main_name}"></script>'.encode()),
                                  ("manifest.json",b"{}"),(main_name,main)):
                path = directory / "raw/apps.razer.com/synapse/products/756" / role / name
                path.parent.mkdir(parents=True,exist_ok=True)
                path.write_bytes(content)
                rows.append({"url":base+name,"path":path.relative_to(root).as_posix(),
                    "bytes":len(content),"sha256":hashlib.sha256(content).hexdigest()})
            document = {"entry_url":base+"index.html","files":rows,
                "source_current_during_acquisition":True,"stability":[
                    {"url":row["url"],"before_sha256":row["sha256"],"after_sha256":row["sha256"],"unchanged":True}
                    for row in rows[:2]]}
            path = directory / "acquisition.json"
            path.write_text(json.dumps(document),encoding="utf-8")
            receipts.append((path,document))
        return receipts

    def test_transfer_generation_verifies_official_entry_bytes_versions_and_required_roles(self):
        import analysis
        import sys
        sys.path.insert(0,str(typed_tables.ROOT / "tools/analyzers/python-static"))
        errors = {"unstable":"deployment do not verify", "entry_bytes":"deployment do not verify",
                  "main_bytes":"mounted main bytes do not verify", "missing_mw":"coverage is incomplete"}
        for mutation in ("unstable","entry_bytes","main_bytes","missing_mw"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                receipts = self.transfer_acquisition_fixture(root)
                path, document = receipts[1]
                if mutation == "unstable":
                    document["stability"][0]["unchanged"] = False
                elif mutation in ("entry_bytes","main_bytes"):
                    row = document["files"][0 if mutation == "entry_bytes" else 2]
                    (root / row["path"]).write_bytes(b"changed bytes")
                elif mutation == "missing_mw":
                    path.unlink()
                if mutation != "missing_mw":
                    path.write_text(json.dumps(document),encoding="utf-8")
                with patch.object(typed_tables,"ROOT",root), patch.object(analysis,"ROOT",root), \
                        patch.object(typed_tables,"__file__",str(root / "tools/typed_tables.py")):
                    with self.assertRaisesRegex(ValueError,errors[mutation]):
                        typed_tables.prepare_profile_transfer_table()
                self.assertFalse((root / typed_tables.TRANSFER_OUTPUT).exists())

    def test_transfer_generation_and_check_use_the_same_source_owned_output(self):
        import analysis
        import sys
        sys.path.insert(0,str(typed_tables.ROOT / "tools/analyzers/python-static"))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            receipts = self.transfer_acquisition_fixture(root)
            path, document = receipts[1]
            row = document["files"][2]
            raw = (root / row["path"]).read_bytes().replace(b"bleId:758",b"bleId:759")
            (root / row["path"]).write_bytes(raw)
            row.update(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
            path.write_text(json.dumps(document),encoding="utf-8")
            with patch.object(typed_tables,"ROOT",root), patch.object(analysis,"ROOT",root), \
                    patch.object(typed_tables,"__file__",str(root / "tools/typed_tables.py")):
                result = typed_tables.prepare_profile_transfer_table()
                self.assertEqual(result["products"],1)
                output = root / typed_tables.TRANSFER_OUTPUT
                self.assertIn(b"product_id: 756",output.read_bytes())
                self.assertIn(b"ble_id: Some(758)",output.read_bytes())
                self.assertIn(b"ble_id: Some(759)",output.read_bytes())
                self.assertTrue(typed_tables.prepare_profile_transfer_table(check=True)["passed"])
                source = output.read_bytes()
                output.write_bytes(source.replace(b"product_id: 756",b"product_id: 700"))
                with self.assertRaisesRegex(ValueError,"products differ"):
                    typed_tables.prepare_profile_transfer_table(check=True)

    def source_fixture(self, name):
        transport = {
            "source_product_id":241,"vendor_id":5426,"product_id":241,"claim_interface":3,
            "report_bytes":91,"report_id":0,"protocol":"razer_device25_wireless_status_v2",
            "command":[80,0,191],"transaction_prefix":0,"transaction_modulus":31,
            "max_retry_in":10,"max_retry_out":20,"sleep_between_out_ms":5,
            "sleep_between_out_in_ms":5,"sleep_between_in_ms":5,"peer_match_product_id":False,
            "source_class":"rzDevice25LinkerMultiDevices","evidence_path":"current-receiver-source",
        }
        read = {key: value for key, value in transport.items() if key not in {
            "source_product_id","protocol","command","peer_match_product_id","evidence_path"
        }}
        read.update({"source_class":"rzDevice25DualLinkMouse","direct_pids":[241],
                     "polling_codes":{"RATE_1000Hz":1},"charging_codes":{},
                     "queries":[
                         {"name":"firmware","method":"getExtendedFirmwareVersion","command":[4,0,135],"payload":[],"min_response_bytes":4},
                         {"name":"polling","method":"getProfilePollingRate","command":[2,0,142],"payload":[1],"min_response_bytes":2},
                     ],"alternate_commands":[{"command":[1,0,70]}]})
        samples = {
            "device-read-capabilities.json":{"schema_version":1,"products":[read]},
            "discovery-catalog.json":[{"product_id":756,"dongle_id":757,"ble_id":758,"middleware":{"category":"KEYBOARD","claim_interface":3,"receipt":{}}}],
            "receiver-query-capabilities.json":{"schema_version":1,"scope":"receiver_queries","capabilities":[transport]},
            "receiver-pairing-capabilities.json":{"schema_version":1,"products":[{
                "product_id":241,"category":"MOUSE","scan_pair_prefix":0,"unpair_prefix":0,
                "cancel_prefix":0,"metadata_prefix":0,"metadata_mouse_layout":False,"transport":transport,
            }]},
            "receiver-pairing-catalog.json":{"schema_version":1,"source_scope":"pairing","source_evidence":{},
                "masters":[{"productId":756,"dongleId":757,"bleId":758,"claimInterface":3,"canPairTwoDevices":False}],
                "devices":[{"DeviceDonglePid":757,"ProductInfo":[{"EID":0,"Name":"Keyboard","CHSName":"键盘"}],"DevicePairingBuddies":[183],"DeviceType":"KEYBOARD"}],
                "available":[{"productId":241,"dongleId":[241]},{"productId":756,"dongleId":757}],
            },
        }
        return json.dumps(samples[name],ensure_ascii=False).encode("utf-8")

    def test_complete_local_protocol_inputs_render_without_runtime_json(self):
        for name, renderer in (
            ("device-read-capabilities.json", typed_tables.render_read_catalog),
            ("discovery-catalog.json", typed_tables.render_identity_catalog),
            ("receiver-query-capabilities.json", typed_tables.render_receiver_query_catalog),
            ("receiver-pairing-capabilities.json", typed_tables.render_pairing_capabilities),
            ("receiver-pairing-catalog.json", typed_tables.render_pairing_source_catalog),
        ):
            raw = self.source_fixture(name)
            rendered = renderer(raw).decode("utf-8")
            self.assertIn(hashlib.sha256(raw).hexdigest(), rendered)
            self.assertNotIn("serde_json", rendered)
            self.assertNotIn("include_str!", rendered)
            self.assertNotIn(".ref/", rendered)

    def test_fixed_reads_reject_wrong_methods_offsets_namespaces_and_new_fields(self):
        for mutation in range(6):
            source = json.loads(self.source_fixture("device-read-capabilities.json"))
            product = source["products"][0]
            if mutation == 0:
                product["queries"][0]["method"] = "getBatteryLevel"
            elif mutation == 1:
                product["polling_value_offset"] = 79
            elif mutation == 2:
                product["transaction_prefix"] = 1
            elif mutation == 3:
                product["claim_interfaces_by_pid"] = {"00124": 0}
            elif mutation == 4:
                product["alternate_commands"][0]["command"] = [1, 0, 71]
            else:
                product["unmapped_source_field"] = True
            with self.assertRaises(ValueError, msg=str(mutation)):
                typed_tables.render_read_catalog(json.dumps(source).encode())

    def test_fixed_identity_rejects_unknown_categories_ids_and_repeated_source_rows(self):
        for source in (
            [{"product_id": 756, "middleware": {"category": "UNKNOWN"}}],
            [{"product_id": 756, "dongle_id": []}],
            [{"product_id": 756, "ble_id": 70000}],
            [{"product_id": 756}, {"product_id": 756}],
            [{"product_id": 0}],
        ):
            with self.assertRaises(ValueError):
                typed_tables.render_identity_catalog(json.dumps(source).encode())
        shared = [{"product_id": 600, "dongle_id": 625}, {"product_id": 601, "dongle_id": 625}]
        self.assertEqual(len(typed_tables.validate_identity_catalog(json.dumps(shared))), 2)

    def test_fixed_receiver_query_rejects_protocol_retry_and_repeated_pid_branches(self):
        for mutation in range(3):
            source = json.loads(self.source_fixture("receiver-query-capabilities.json"))
            if mutation == 0:
                source["capabilities"][0]["protocol"] = "unknown_protocol"
            elif mutation == 1:
                source["capabilities"][0]["max_retry_out"] = 0
            else:
                source["capabilities"].append(copy.deepcopy(source["capabilities"][0]))
            with self.assertRaises(ValueError):
                typed_tables.render_receiver_query_catalog(json.dumps(source).encode())

    def test_fixed_pairing_rejects_transport_owner_and_category_branch_mismatches(self):
        for mutation in range(3):
            source = json.loads(self.source_fixture("receiver-pairing-capabilities.json"))
            if mutation == 0:
                source["products"][0]["transport"]["product_id"] = 183
            elif mutation == 1:
                source["products"][0]["category"] = "ACCESSORY"
            else:
                source["products"].append(copy.deepcopy(source["products"][0]))
            with self.assertRaises(ValueError):
                typed_tables.render_pairing_capabilities(json.dumps(source).encode())

    def test_fixed_pairing_rejects_obsolete_179_category_or_cancel_namespaces(self):
        source = json.loads(self.source_fixture("receiver-pairing-capabilities.json"))
        product = source["products"][0]
        product["product_id"] = product["transport"]["product_id"] = product["transport"]["source_product_id"] = 179
        for category, namespaces in typed_tables.RECEIVER_179_PAIRING_NAMESPACES.items():
            product.update(category=category, **namespaces)
            typed_tables.render_pairing_capabilities(json.dumps(source).encode())
            for field in namespaces:
                changed = copy.deepcopy(source)
                changed["products"][0][field] = 128 if namespaces[field] == 0 else 0
                with self.assertRaisesRegex(ValueError,"PID179 pairing namespaces differ"):
                    typed_tables.render_pairing_capabilities(json.dumps(changed).encode())

    def test_fixed_pairing_keeps_scalar_array_distinction_and_rejects_edition_loss(self):
        raw = self.source_fixture("receiver-pairing-catalog.json")
        rendered = typed_tables.render_pairing_source_catalog(raw).decode("utf-8")
        self.assertIn("CatalogProductIds::Many(vec![241])", rendered)
        self.assertIn("CatalogProductIds::One(757)", rendered)
        source = json.loads(raw)
        source["devices"][0]["ProductInfo"].append(copy.deepcopy(source["devices"][0]["ProductInfo"][0]))
        with self.assertRaisesRegex(ValueError, "Repeated.*edition"):
            typed_tables.render_pairing_source_catalog(json.dumps(source).encode())

    def test_ambiguous_keys_and_duplicate_product_ids_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "Duplicate source-table JSON key"):
            typed_tables.validate_write_catalog(b'{"schema_version":1,"schema_version":1,"products":[]}')
        source = catalog()
        source["products"].append(copy.deepcopy(source["products"][0]))
        with self.assertRaisesRegex(ValueError, "Duplicate.*product ID"):
            typed_tables.validate_write_catalog(json.dumps(source))

    def test_unknown_method_and_mismatched_read_parser_cannot_be_silently_omitted(self):
        for field, value in (("method", "unprovenMethod"), ("read_kind", "dpi"), ("kind", "future_kind")):
            source = catalog()
            source["products"][0]["writes"][0][field] = value
            with self.assertRaises(ValueError):
                typed_tables.render_write_catalog(json.dumps(source).encode())

    def test_invalid_integer_widths_and_packet_shapes_fail_before_output(self):
        for field, value in (("selector", 256), ("selector", True), ("command", [1, 0, 14]),
                             ("command", [2, 0, 256]), ("command", [2, 14])):
            source = catalog()
            source["products"][0]["writes"][0][field] = value
            with self.assertRaises(ValueError):
                typed_tables.validate_write_catalog(json.dumps(source))

    def test_invalid_dpi_ranges_polling_rates_and_extra_source_fields_fail(self):
        for field, value in (("max_dpi", 99), ("polling_codes", {"RATE_0Hz": 1}),
                             ("polling_codes", {"RATE_1000Hz": 256}), ("source_class", "unprovenClass"),
                             ("new_protocol_field", 1)):
            source = catalog()
            source["products"][0][field] = value
            with self.assertRaises(ValueError):
                typed_tables.validate_write_catalog(json.dumps(source))

    def test_check_preserves_different_checked_in_table_instead_of_overwriting_it(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(typed_tables, "ROOT", Path(directory)):
            root = Path(directory)
            source = root / typed_tables.WRITE_INPUT
            source.parent.mkdir(parents=True)
            source.write_text(json.dumps(catalog()), encoding="utf-8")
            typed_tables.prepare_write_table()
            output = root / typed_tables.WRITE_OUTPUT
            output.write_bytes(b"locally edited Rust table")
            with self.assertRaisesRegex(ValueError, "differs from the maintenance input"):
                typed_tables.prepare_write_table(check=True)
            self.assertEqual(output.read_bytes(), b"locally edited Rust table")


if __name__ == "__main__":
    unittest.main()

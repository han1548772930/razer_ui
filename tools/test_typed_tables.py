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
    def current_read_fixture(self):
        template = json.loads(self.source_fixture("device-read-capabilities.json"))["products"][0]
        template.pop("alternate_commands")
        template.update(source_class="rzDevice25", polling_codes={}, queries=[template["queries"][0]])
        products = []
        for pid, interface, selector, physical in ((190, 0, 0, [190, 191]), (679, 3, 3, [679])):
            product = copy.deepcopy(template)
            product.update(product_id=pid, claim_interface=interface, report_id=selector, direct_pids=physical)
            product["claim_interfaces_by_pid"] = {str(key): interface for key in physical}
            product["physical_transports_by_pid"] = {
                str(key): {**{field: product[field] for field in (
                    "source_class", "claim_interface", "report_id", "transaction_prefix", "transaction_modulus",
                    "max_retry_in", "max_retry_out", "sleep_between_out_ms", "sleep_between_out_in_ms", "sleep_between_in_ms")},
                    "is_duallink": False} for key in physical}
            if pid == 190:
                product["queries"] += [{"name": name, "method": method, "command": command,
                    "payload": payload, "min_response_bytes": minimum} for method, name, command, payload, minimum in (
                    ("getBatteryLevel", "battery", [2, 7, 128], [0], 2),
                    ("getChargingStatus", "charging", [2, 7, 132], [0], 2),
                    ("getTimeToSleep", "idle", [2, 7, 131], [], 2))]
                product["charging_codes"] = {"NoCharge_BatteryFull": 0, "Charging": 1, "ChargingFailed": 2,
                    "ForeignObjectDetection": 3, "FastCharging": 4, "FullCharging": 5, "ReachChargingLimit": 6}
            products.append(product)
        return products

    def test_current_read_projection_checks_every_transport_command_selector_and_response_field(self):
        products = self.current_read_fixture()
        raw = typed_tables.render_read_catalog(json.dumps({"schema_version": 1, "products": products}).encode())
        self.assertEqual(typed_tables.current_read_table_projection(raw, products), (raw, [190, 679]))
        decorated = raw.replace(b"command: [4, 0, 135]", b"command: /* retained */ [ 4, 0, 135 ]")
        self.assertEqual(typed_tables.current_read_table_projection(decorated, products)[0], decorated)
        for before, after in (
                (b"claim_interface: 0", b"claim_interface: 3"),
                (b"report_id: 3", b"report_id: 0"),
                (b"direct_pids: vec![190, 191]", b"direct_pids: vec![190]"),
                (b"max_retry_in: 10", b"max_retry_in: 11"),
                (b"transaction_modulus: 31", b"transaction_modulus: 32"),
                (b"command: [4, 0, 135]", b"command: [4, 0, 134]"),
                (b"payload: vec![0]", b"payload: vec![1]"),
                (b"min_response_bytes: 4", b"min_response_bytes: 3"),
                (b'("Charging".into(), 1)', b'("Charging".into(), 4)'),
                (b"is_duallink: false", b"is_duallink: true"),
                (b"source_class:", b"unexpected_field: 0, source_class:"),
                (b"product_id: 190,", b"product_id: 190, product_id: 190,")):
            self.assertIn(before, raw)
            with self.assertRaises(ValueError, msg=str(before)):
                typed_tables.current_read_table_projection(raw.replace(before, after, 1), products)

    def test_current_read_projection_updates_only_verified_products_and_rejects_wrong_owners(self):
        products = self.current_read_fixture()
        other = copy.deepcopy(products[1])
        other.update(product_id=708, direct_pids=[708], physical_transports_by_pid={}, claim_interfaces_by_pid={})
        original = typed_tables.render_read_catalog(json.dumps({"schema_version": 1, "products": [other, products[0]]}).encode())
        with self.assertRaisesRegex(ValueError, "omits current product 679"):
            typed_tables.current_read_table_projection(original, products)
        stale = original.replace(b"claim_interface: 0", b"claim_interface: 2", 1)
        updated, ids = typed_tables.current_read_table_projection(stale, products, update=True)
        self.assertEqual(ids, [190, 679])
        self.assertEqual(typed_tables.current_read_table_projection(updated, products)[0], updated)
        self.assertFalse(any(line.endswith((b" ", b"\t")) for line in updated.splitlines()))
        other_start = original.index(b"        DeviceReadCapability {")
        other_end = original.index(b"        DeviceReadCapability {", other_start + 1)
        self.assertIn(original[other_start:other_end], updated)
        no_trailing_comma = original.replace(b"        },\n    ]", b"        }\n    ]")
        updated, _ = typed_tables.current_read_table_projection(no_trailing_comma, products, update=True)
        self.assertEqual(typed_tables.current_read_table_projection(updated, products)[0], updated)
        for invalid in (original.replace(b"fn products()", b"fn fixture()"),
                        original.replace(b"product_id: 708", b"product_id: 190"),
                        original.replace(b"product_id: 190", b"product_id: id()"),
                        b"mod fixture {" + original + b"}"):
            with self.assertRaises(ValueError):
                typed_tables.current_read_table_projection(invalid, products, update=True)

    @staticmethod
    def current_decoder_helpers():
        return {
            "getExtendedFirmwareVersion": "f=e=>wrap(function*(){const r=yield e.sendCommand(c.fw,c.description);"
                "return(r=>{r.status===status.STATUS_SUCCESS&&(r.jsonData={major:r.data[0],minor:r.data[1],"
                "internal:r.data[2],reserved:r.data[3]})})(r),r})",
            "getBatteryLevel": "f=(e,id)=>wrap(function*(){const p=new Uint8Array(c.battery[0]);p[0]=id;"
                "const r=yield e.sendCommand(c.battery,c.description,p);"
                "return r.jsonData={batteryId:r.data[0],batteryLevel:Math.floor(r.data[1]/255*100)},r})",
            "getChargingStatus": "f=(e,id)=>wrap(function*(){const p=new Uint8Array(c.charging[0]);p[0]=id;"
                "const r=yield e.sendCommand(c.charging,c.description,p);return r.jsonData={batteryId:r.data[0],"
                "chargingStatus:r.data[1],chargingStatusEnum:helper.enumValue(c.Jz,r.data[1])},r})",
            "getTimeToSleep": "f=e=>wrap(function*(){const p=new Uint8Array(c.idle[0]),r=yield "
                "e.sendCommand(c.idle,c.description,p);return decoder(r),r})",
        }

    def test_current_read_decoders_reject_changed_payload_offsets_units_and_missing_fields(self):
        helpers = self.current_decoder_helpers()
        for method, body in helpers.items():
            self.assertIn("payload", typed_tables.current_read_decoder_contract(method, body))
        for method, before, after in (
                ("getExtendedFirmwareVersion", "minor:r.data[1]", "minor:r.data[2]"),
                ("getExtendedFirmwareVersion", ",reserved:r.data[3]", ""),
                ("getExtendedFirmwareVersion", "c.fw,c.description", "c.fw,c.description,[]"),
                ("getBatteryLevel", "p[0]=id", "p[1]=id"),
                ("getBatteryLevel", "p[0]=id", "p[0]=0"),
                ("getBatteryLevel", "/255*100", "/100*255"),
                ("getBatteryLevel", "Math.floor", "Math.round"),
                ("getChargingStatus", "chargingStatus:r.data[1]", "chargingStatus:r.data[0]"),
                ("getChargingStatus", "c.Jz", "c.otherEnum"),
                ("getTimeToSleep", "return decoder(r)", "p[0]=1;return decoder(r)")):
            with self.assertRaises(ValueError, msg=method + " " + before):
                typed_tables.current_read_decoder_contract(method, helpers[method].replace(before, after))
        idle = "f=r=>{r.jsonData={timeToSleep:bytes.be(r.data[0],r.data[1])}}"
        self.assertEqual(typed_tables.current_read_decoder_contract("getTimeToSleep", idle, decoder_only=True)["observed_fields"],
                         {"timeToSleep": ["big_endian", ["byte", 0], ["byte", 1]]})
        with self.assertRaises(ValueError):
            typed_tables.current_read_decoder_contract("getTimeToSleep", idle.replace("r.data[0],r.data[1]", "r.data[1],r.data[0]"),
                                                       decoder_only=True)

    def test_current_read_key_inventory_uses_normal_fwids_and_rejects_ambiguous_or_dynamic_keys(self):
        import sys
        sys.path.insert(0, str(Path(typed_tables.__file__).resolve().parent / "analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        parser = Parser(Language(tree_sitter_javascript.language()))

        def project(buttons, mapping):
            button_tree = parser.parse(("const keys=" + buttons).encode())
            mapping_tree = parser.parse(("const keys=" + mapping).encode())
            return typed_tables.current_read_key_inventory(
                button_tree.root_node.named_children[0].named_children[0].child_by_field_name("value"),
                mapping_tree.root_node.named_children[0].named_children[0].child_by_field_name("value"))

        buttons = '[{inputID:"W"},{inputID:"A"},{inputID:"W"},{isHyperShift:!0,inputID:"only_shift"}]'
        mapping = '{W:{fwID:18,inputID:"W",analogKeyID:17},A:{fwID:31,inputID:"A",HID:4}}'
        self.assertEqual(project(buttons, mapping), [
            {"firmware_key_id": 18, "input_id": "W"}, {"firmware_key_id": 31, "input_id": "A"}])
        for changed_buttons, changed_mapping in (
                (buttons, mapping.replace("fwID:18", "fwID:255")),
                (buttons, mapping.replace("fwID:31", "fwID:18")),
                (buttons, mapping.replace("fwID:18", "fwID:deviceKey()")),
                (buttons, mapping.replace('inputID:"W"', 'inputID:"other"')),
                (buttons, mapping.replace("analogKeyID:17", "ignoreInObm:!0")),
                (buttons.replace('inputID:"A"', 'inputID:"missing"'), mapping),
                (buttons.replace("isHyperShift:!0", "isHyperShift:dynamic"), mapping),
                (buttons.replace('{inputID:"W"}', '{...deviceKey}'), mapping)):
            with self.assertRaises(ValueError):
                project(changed_buttons, changed_mapping)

    def test_current_read_probe_allowlists_match_literal_source_inventory_and_module_ownership(self):
        expected = [18, 31, 32, 33]
        rust = b"const KEYBOARD_679_LAYOUT_1_KEYS:&[u8]=&[18,31,32,33];"
        python = b"_KEYBOARD_679_LAYOUT_1_KEYS=frozenset((18,31,32,33))"
        self.assertEqual(typed_tables.verify_current_read_key_fixtures(expected, rust, python)["keys"], 4)
        for invalid_rust, invalid_python in (
                (rust.replace(b"18,", b"255,"), python),
                (rust.replace(b"18,", b""), python),
                (rust.replace(b"18,", b"18,18,"), python),
                (rust.replace(b"18,", b"id(),"), python),
                (b"fn fixture(){" + rust + b"}", python),
                (rust, python.replace(b"18,", b"17,")),
                (rust, python.replace(b"18,", b"18,18,")),
                (rust, python.replace(b"frozenset", b"set")),
                (rust, python + b"\n" + python),
                (rust, b"def fixture():\n    " + python)):
            with self.assertRaises(ValueError):
                typed_tables.verify_current_read_key_fixtures(expected, invalid_rust, invalid_python)

    def test_current_onboard_inventory_applies_whole_entry_overrides_and_independent_mode_filters(self):
        import sys
        sys.path.insert(0, str(Path(typed_tables.__file__).resolve().parent / "analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        parser = Parser(Language(tree_sitter_javascript.language()))

        def value(source):
            tree = parser.parse(("const value=" + source).encode())
            self.assertFalse(tree.root_node.has_error)
            return tree.root_node.named_children[0].named_children[0].child_by_field_name("value")

        buttons = '[{inputID:"B"},{inputID:"A"},{inputID:"A"},{inputID:"only_shift",isHyperShift:!0}]'
        generic = ('{NormalMode:{A:{inputID:"A",fwID:18},B:{inputID:"B",fwID:31}},'
            'HypershiftMode:{A:{inputID:"A",fwID:18,ignoreInObm:!0},B:{inputID:"B",fwID:31}}}')
        overrides = '{HypershiftMode:{A:{inputID:"A",fwID:18},B:{inputID:"B",fwID:31,ignoreInObm:!0}}}'
        mouse = '{NormalMode:{M:{inputID:"M",fwID:18,ignoreInObm:!0}},HypershiftMode:{}}'

        def project(ui=buttons, base=generic, patch=overrides, shared=mouse):
            return typed_tables.current_onboard_key_inventory(value(ui), value(base), value(patch), value(shared))

        self.assertEqual(project(), [
            {"input_id": "A", "firmware_key_id": 18, "ignore_normal": True, "ignore_hypershift": False},
            {"input_id": "B", "firmware_key_id": 31, "ignore_normal": False, "ignore_hypershift": True}])
        for changed_ui, changed_base, changed_patch, changed_mouse in (
                (buttons, generic, overrides.replace("ignoreInObm:!0", "ignoreInObm:enabled"), mouse),
                (buttons, generic, overrides.replace('A:{inputID:"A",fwID:18}', 'A:{inputID:"A",fwID:19}'), mouse),
                (buttons, generic.replace('B:{inputID:"B",fwID:31}', 'B:{inputID:"B",fwID:18}'), overrides, mouse),
                (buttons.replace('inputID:"B"', 'inputID:"missing"'), generic, overrides, mouse),
                (buttons.replace('isHyperShift:!0', 'isHyperShift:enabled'), generic, overrides, mouse),
                (buttons, generic, '{OtherMode:{}}', mouse),
                (buttons, generic, overrides, mouse.replace('fwID:18,', '')),
                (buttons, generic, overrides, mouse.replace('fwID:18', 'fwID:getKey()')),
                (buttons, generic, overrides, mouse.replace('inputID:"M"', 'inputID:"other"'))):
            with self.assertRaises(ValueError):
                project(changed_ui, changed_base, changed_patch, changed_mouse)

    def test_current_onboard_inventory_renderer_checks_fields_identity_order_and_all_layouts(self):
        keys = [{"input_id": "A", "firmware_key_id": 18, "ignore_normal": False, "ignore_hypershift": True},
                {"input_id": "B", "firmware_key_id": 31, "ignore_normal": True, "ignore_hypershift": False}]
        inventory = {"inventories": {"BUTTON_LIST": keys, "BUTTON_LIST_UK": keys[:1]}, "uk_layout_ids": [2, 9]}
        output = typed_tables.render_current_onboard_inventory(inventory)
        typed_tables.receiver_table_tree(output)
        self.assertIn(b'Key::new("A", 18, false, true)', output)
        self.assertIn(b'Key::new("B", 31, true, false)', output)
        self.assertIn(b'if [2, 9].contains(&layout_id)', output)
        for changed in (
                {**inventory, "uk_layout_ids": [2, 2]},
                {**inventory, "uk_layout_ids": []},
                {**inventory, "inventories": {"BUTTON_LIST": keys}},
                {**inventory, "inventories": {"BUTTON_LIST": list(reversed(keys)), "BUTTON_LIST_UK": keys}},
                {**inventory, "inventories": {"BUTTON_LIST": keys + keys[:1], "BUTTON_LIST_UK": keys}},
                {**inventory, "inventories": {"BUTTON_LIST": [{**keys[0], "ignore_normal": 0}], "BUTTON_LIST_UK": keys}},
                {**inventory, "inventories": {"BUTTON_LIST": [{**keys[0], "unknown": True}], "BUTTON_LIST_UK": keys}}):
            with self.assertRaises(ValueError):
                typed_tables.render_current_onboard_inventory(changed)

    def test_current_onboard_literal_rejects_dynamic_members_duplicates_and_sparse_arrays(self):
        import sys
        sys.path.insert(0, str(Path(typed_tables.__file__).resolve().parent / "analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        parser = Parser(Language(tree_sitter_javascript.language()))

        def decode(source):
            tree = parser.parse(("const value=" + source).encode())
            value = tree.root_node.named_children[0].named_children[0].child_by_field_name("value")
            return typed_tables.current_onboard_literal(value)

        self.assertEqual(decode('{NormalMode:{"A":[7,!0,false,/* source comment */"key",]},1:!1}'),
                         {"NormalMode": {"A": [7, True, False, "key"]}, "1": False})
        for invalid in ('{key:1,key:2}', '{key:1,"key":2}', '{...shared}', '{[name]:1}',
                        '{key}', '{key(){return 1}}', '[...keys]', '[,1]', '[1,,2]', '[1,,]',
                        '{key:lookup()}', '{key:state.value}', '{key:null}', '{key:undefined}',
                        '{key:-1}', '{key:1.5}', '{key:0x10}', '{key:01}'):
            with self.subTest(source=invalid), self.assertRaises(ValueError):
                decode(invalid)

    @staticmethod
    def current_onboard_mapping_fixture():
        modifiers = ["LeftCtrl", "LeftShift", "LeftAlt", "LeftGUI", "RightCtrl", "RightShift", "RightAlt", "RightGUI"]
        names = ["Left_Ctrl", "Left_Shift", "Left_Alt", "Left_GUI", "Right_Ctrl", "Right_Shift", "Right_Alt", "Right_GUI"]
        key = {"fwID": 31, "inputID": "A", "inputType": "KeyInput", "keyInput": {"HID": "4", "pageID": "7"},
               "outputType": "keyboardGroup", "keyboardGroup": {"key": {"HID": "5", "flag": 1, "pageID": "7"},
               "modifiers": ["Left_Ctrl", "Extended_Key"], "extendedKey": "Extended_00"}}
        disabled = {"fwID": 64, "inputID": "DKM", "inputType": "DKMInput", "dkmInput": "DKM", "outputType": "disableGroup"}
        click = {"fwID": 1, "inputID": "LeftClick", "inputType": "MouseInput", "mouseInput": "LeftClick",
                 "outputType": "mouseGroup", "mouseGroup": {"mouseAssignment": "Click"}}
        dpi = {"fwID": 15, "inputID": "CLUTCH", "inputType": "DKMInput", "dkmInput": "CLUTCH",
               "outputType": "sensitivityGroup", "sensitivityGroup": {"sensitivityAssignment": "DPI_Clutch", "x": 800, "y": 900}}
        return {"keyboard": {"NormalMode": {"A": key, "DKM": disabled},
                             "HypershiftMode": {"A": {**copy.deepcopy(key), "isHyperShift": True}}},
                "mouse": {mode: {"LeftClick": copy.deepcopy(click), "CLUTCH": copy.deepcopy(dpi)}
                          for mode in ("NormalMode", "HypershiftMode")},
                "encoding": {"function_ids": {"Off": 0, "KeyCode": 2, "MediaKeys": 10},
                             "modifier_bits": {name: 1 << bit for bit, name in enumerate(modifiers)},
                             "modifier_names": dict(zip(modifiers, names)),
                             "media_codes": {"MediaPlayOrPause": 205}, "media_names": {"MediaPlayOrPause": "Play"},
                             "power_codes": {"SystemSleep": 130}, "razer_codes": {"FnKey": 1},
                             "shortcut_codes": {"Screenshot": 99}, "shortcut_names": {"Screenshot": "Screenshot"},
                             "mouse_names": {"LeftClick": "Click"}, "profile_names": {"Specific": "Specific"}}}

    def test_current_onboard_mapping_renderer_preserves_source_codes_modes_and_keyboard_fields(self):
        projection = self.current_onboard_mapping_fixture()
        output = typed_tables.render_current_onboard_mapping(projection)
        typed_tables.receiver_table_tree(output)
        self.assertIn(b'205 => Some(MediaAction::Play)', output)
        self.assertIn(b'130 => Some(PowerAction::SystemSleep)', output)
        self.assertIn(b'99 => Some(ShortcutAction::Screenshot)', output)
        self.assertIn(b'(128, "Right_GUI")', output)
        self.assertIn(b'input_hid: Some(4), output: Output::Keyboard(KeyboardOutput { key: HidKey { hid: 5, flag: 1, page_id: 7 }', output)
        self.assertIn(b'modifiers: 1, extended_modifier: true, extended_key: Some(ExtendedKey::Extended00)', output)
        self.assertIn(b'input_id: "A", mode: Mode::Hypershift', output)
        self.assertIn(b'input_id: "DKM", mode: Mode::Normal, firmware_key_id: Some(64), input_hid: None, output: Output::Disabled', output)
        self.assertIn(b'(Mode::Normal, 1, MouseAction::Click)', output)
        self.assertIn(b'(Mode::Hypershift, 1, MouseAction::Click)', output)
        self.assertLess(output.index(b'input_id: "A", mode: Mode::Normal'), output.index(b'input_id: "DKM"'))

    def test_current_onboard_mapping_renderer_rejects_lost_fields_ambiguous_codes_and_identity(self):
        mutations = [
            lambda p: p.update(unknown=[]),
            lambda p: p["encoding"].update(unknown={}),
            lambda p: p["encoding"]["function_ids"].update(Other="2"),
            lambda p: p["encoding"]["function_ids"].update(Other=True),
            lambda p: p["encoding"]["function_ids"].update(Bad_Code=3, BadCode=4),
            lambda p: p["encoding"]["media_codes"].update(Extra=206),
            lambda p: p["encoding"]["media_names"].update(Extra="Next"),
            lambda p: p["encoding"]["shortcut_names"].clear(),
            lambda p: p["encoding"]["mouse_names"].update(RightClick="Click"),
            lambda p: p["encoding"]["modifier_bits"].update(LeftCtrl=True),
            lambda p: p["encoding"]["modifier_names"].update(RightGUI="Left_Ctrl"),
            lambda p: p["keyboard"].update(OtherMode={}),
            lambda p: p["keyboard"]["NormalMode"]["A"].update(inputID="other"),
            lambda p: p["keyboard"]["NormalMode"]["A"].update(fwID=None),
            lambda p: p["keyboard"]["NormalMode"]["A"].update(fwID=64),
            lambda p: p["keyboard"]["NormalMode"]["A"].update(isHyperShift=1),
            lambda p: p["keyboard"]["NormalMode"]["A"]["keyInput"].update(pageID="bad"),
            lambda p: p["keyboard"]["NormalMode"]["A"].update(dkmInput="A"),
            lambda p: p["keyboard"]["NormalMode"]["A"]["keyboardGroup"].update(unknown=1),
            lambda p: p["keyboard"]["NormalMode"]["A"]["keyboardGroup"].update(modifiers=["Left_Ctrl", "Left_Ctrl"]),
            lambda p: p["keyboard"]["NormalMode"]["A"]["keyboardGroup"].update(extendedKey="other"),
            lambda p: p["keyboard"]["NormalMode"]["DKM"].update(disableGroup={}),
            lambda p: p["mouse"]["NormalMode"]["LeftClick"].update(inputID="other"),
            lambda p: p["mouse"]["NormalMode"]["LeftClick"].update(fwID=15),
            lambda p: p["mouse"]["NormalMode"]["LeftClick"]["mouseGroup"].update(turboMode={"isTurbo": True, "keysPerSecond": 0}),
            lambda p: p["mouse"]["NormalMode"]["LeftClick"]["mouseGroup"].update(mouseAssignment="missing"),
            lambda p: p["mouse"]["NormalMode"]["CLUTCH"]["sensitivityGroup"].update(unknown=1),
            lambda p: p["mouse"]["NormalMode"]["CLUTCH"]["sensitivityGroup"].update(y=65536),
        ]
        for index, mutate in enumerate(mutations):
            projection = self.current_onboard_mapping_fixture()
            mutate(projection)
            with self.subTest(mutation=index), self.assertRaises(ValueError):
                typed_tables.render_current_onboard_mapping(projection)

    def test_current_read_inventory_selection_rejects_swapped_layouts_and_override_precedence(self):
        selection = ('w=()=>{let e=[];return e=o.Yi0.includes(m.A.runtimeData.layoutId)&&n.BUTTON_LIST_UK?'
            '[...n.BUTTON_LIST_UK]:m.A.runtimeData.layoutId===_.Z7j.Japanese&&n.BUTTON_LIST_JP?'
            '[...n.BUTTON_LIST_JP]:[...n.BUTTON_LIST],e}')
        merging = ('C=()=>{var e,t;if(O)return O;let i={};'
            'const{inputMapping:n,inputMappingJP:r,inputMappingUK:s}=p.A.getFeatureParam("isObm","yes");'
            'return i=m.A.runtimeData.layoutId===_.Z7j.Japanese&&r?r:o.Yi0.includes(m.A.runtimeData.layoutId)&&s?s:n,'
            'i?(O={NormalMode:Object.assign(Object.assign({},a.Bx.NormalMode),'
            '(null===(e=i.keyboard)||void 0===e?void 0:e.NormalMode)||{}),'
            'HypershiftMode:Object.assign(Object.assign({},a.Bx.HypershiftMode),'
            '(null===(t=i.keyboard)||void 0===t?void 0:t.HypershiftMode)||{})},O):(O=a.Bx,O)}')
        self.assertTrue(typed_tables.current_read_inventory_selection_contract(selection, merging)["layout_and_override_selection_equal"])
        for invalid_selection, invalid_merging in (
                (selection.replace("[...n.BUTTON_LIST_UK]", "[...n.BUTTON_LIST]"), merging),
                (selection.replace("===_.Z7j.Japanese", "!==_.Z7j.Japanese"), merging),
                (selection.replace("&&n.BUTTON_LIST_JP", "||n.BUTTON_LIST_JP"), merging),
                (selection, merging.replace("a.Bx.NormalMode", "a.Bx.HypershiftMode")),
                (selection, merging.replace("&&s?s:n", "&&s?n:s")),
                (selection, merging.replace('getFeatureParam("isObm","yes")', 'getFeatureParam("other","yes")'))):
            with self.assertRaises(ValueError):
                typed_tables.current_read_inventory_selection_contract(invalid_selection, invalid_merging)

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

    def test_receiver_evidence_tracks_source_product_and_preserves_physical_receiver(self):
        raw = b'''fn products(){vec![ReceiverCapability{source_product_id:756,product_id:241,
            report_id:0}]}'''
        expected = ".analysis/results/frontend/keyboard-mw-756-current/analysis.json"
        with patch.object(typed_tables, "receiver_evidence_path", return_value=expected) as resolve:
            updated, rows = typed_tables.receiver_evidence_table_projection(raw)
            resolve.assert_called_with(756)
            self.assertEqual(updated, raw)
            self.assertEqual(rows, [{"source_product_id": 756, "evidence_path": expected}])
            self.assertEqual(typed_tables.receiver_evidence_table_projection(updated)[0], updated)
            for evidence in (b'"stale".into()', json.dumps(expected).encode() + b'.into()', b'path()'):
                leaked = raw.replace(b"report_id:0", b"evidence_path:" + evidence + b",report_id:0")
                with self.assertRaisesRegex(ValueError, "maintenance receipts"):
                    typed_tables.receiver_evidence_table_projection(leaked)
            for broken in (updated.replace(b"source_product_id:756", b"source_product_id:unknown"),
                           updated.replace(b"report_id:0", b"report_id:0,report_id:0")):
                with self.assertRaises(ValueError):
                    typed_tables.receiver_evidence_table_projection(broken)

    def test_receiver_evidence_rejects_missing_ambiguous_and_wrong_family_capture(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(typed_tables, "ROOT", Path(directory)):
            root = Path(directory)
            def capture(family):
                name = f"{family}-mw-179-current"
                source = ".analysis/inputs/frontend/" + name
                (root / source).mkdir(parents=True)
                path = root / ".analysis/results/frontend" / name / "analysis.json"
                path.parent.mkdir(parents=True)
                path.write_text(json.dumps({"input": source, "original_inputs": [{"path": source + "/main.js"}]}))
                return path
            with self.assertRaisesRegex(ValueError, "exactly one"):
                typed_tables.receiver_evidence_path(179)
            path = capture("accessories")
            self.assertEqual(typed_tables.receiver_evidence_path(179), path.relative_to(root).as_posix())
            path.write_text(json.dumps({"input": "wrong", "original_inputs": [1]}))
            with self.assertRaisesRegex(ValueError, "current originals"):
                typed_tables.receiver_evidence_path(179)
            capture("receiver")
            with self.assertRaisesRegex(ValueError, "exactly one"):
                typed_tables.receiver_evidence_path(179)

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

    def test_static_identity_fields_retain_object_branches_and_ordered_arrays(self):
        factory = self.transfer_factory('...base,claimInterface:{756:3,757:2},ids:[757,758,]')
        projected, _ = typed_tables._profile_transfer_static_fields(
            factory, "DeviceInfo", {"claimInterface", "ids"}, {})
        self.assertEqual(projected, {"claimInterface": {"756": 3, "757": 2}, "ids": [757, 758]})
        factory = self.transfer_factory('...base,claimInterface:{0x2f4:3,"756":3}')
        projected, evidence = typed_tables._profile_transfer_static_fields(
            factory, "DeviceInfo", {"claimInterface"}, {})
        self.assertEqual(projected, {"claimInterface": {"756": 3}})
        self.assertEqual(evidence["equal_duplicate_properties"][0]["field"], "756")

    def test_static_identity_fields_reject_nested_conflicts_holes_spreads_and_cycles(self):
        for value in ('{756:3,756:2}', '{0x2f4:3,"756":2}', '{[756]:3}',
                      '[,3]', '[3,,4]', '[...values]', '{756:query()}', 'info'):
            factory = self.transfer_factory('...base,claimInterface:' + value)
            with self.subTest(value=value), self.assertRaises(ValueError):
                typed_tables._profile_transfer_static_fields(factory, "DeviceInfo", {"claimInterface"}, {})
        factory = self.transfer_factory('...base,claimInterface:cyclic').replace(
            'const deviceName=', 'const cyclic={self:cyclic,self:cyclic};const deviceName=')
        with self.assertRaisesRegex(ValueError, "Cyclic"):
            typed_tables._profile_transfer_static_fields(factory, "DeviceInfo", {"claimInterface"}, {})

    def test_registry_projection_resolves_whole_exported_arrays_and_keeps_absence_explicit(self):
        factory = self.transfer_factory(export="DeviceInfo:function(){return info},AVAILABLE_EDITION_ID:function(){return editions}")
        factory = factory.replace("const deviceName=", "const original=[3,0,1],editions=original;const deviceName=")
        projected, evidence = typed_tables.registry_identity_projection(factory)
        self.assertEqual(projected["edition_ids"], [3, 0, 1])
        self.assertTrue(evidence["edition_ids"]["export_observed"])
        self.assertEqual(projected["manufacturer_name"], 'A "quoted" keyboard')
        for expression in ("[0,0]", "[,0]", "[...ids]", "null", "lookup()"):
            with self.subTest(expression=expression), self.assertRaises(ValueError):
                typed_tables.registry_identity_projection(factory.replace("[3,0,1]", expression))
        absent, absent_evidence = typed_tables.registry_identity_projection(self.transfer_factory("...base"))
        self.assertIsNone(absent["edition_ids"])
        self.assertIsNone(absent["manufacturer_name"])
        self.assertFalse(absent_evidence["edition_ids"]["export_observed"])
        named, _ = typed_tables.registry_identity_projection(self.transfer_factory(
            "...base", export="DeviceInfo:()=>info,DEVICE_NAME:()=>deviceName"))
        self.assertEqual(named["manufacturer_name"], 'A "quoted" keyboard')
        with self.assertRaises(ValueError):
            typed_tables.registry_identity_projection(factory.replace("return editions", "return lookup()"))

    def test_registry_manifest_editions_use_explicit_product_keys_without_icon_inference(self):
        import families
        values = {"productTranslations": {"104_1": {}, "105_3": {}, "105_0": {}, "104_0": {}},
                  "icons": [{"src": "productCategoryIcon.svg"}]}
        editions, evidence = families.registry_manifest_editions(values, 105, 104)
        self.assertEqual(editions, [3, 0])
        self.assertEqual(evidence["declared_product_id"], 105)
        self.assertIsNone(families.registry_manifest_editions({"icons": values["icons"]}, 105, 104)[0])
        for invalid in ({"105_00": {}}, {"105_0": []}, {"105_-1": {}}, {"105_4294967296": {}}):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                families.registry_manifest_editions({"productTranslations": invalid}, 105, 104)

    @staticmethod
    def registry_ast(raw):
        import families
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
        if tree.root_node.has_error:
            raise AssertionError("navigation fixture has an AST error")
        original = {"path": ".analysis/inputs/frontend/test/navigation.js", "bytes": len(raw),
                    "sha256": hashlib.sha256(raw).hexdigest()}
        owners = families.keyboard_source_modules(raw.decode())
        modules = {row["module_id"]: [(original, row)] for row in owners}
        return tree, owners, modules

    @staticmethod
    def registry_root_fixture(*, macro=False, chroma=False, detached_create=False):
        branches = []
        if macro:
            branches.append('macro:(0,j.jsx)(Lazy,{macro:macroValue,lazy:()=>Promise.all([require.e(8),require.e(9)]).then(require.bind(require,80))})')
        if chroma:
            branches.append('chromaApp:(0,j.jsx)(Lazy,{lazy:()=>require.e(7).then(require.bind(require,70))})')
        branches.append('default:(0,j.jsx)(Lazy,{lazy:()=>Promise.all([require.e(1),require.e(2)]).then(require.bind(require,10))})')
        create = '(0,ReactDOM.createRoot)' if detached_create else 'ReactDOM.createRoot'
        return ('(()=>{function Page(){const {displayMode:mode,macro:macroValue}=React.useMemo(()=>{'
                'const url=new URL(location.href);return {displayMode:url.searchParams.get("displayMode"),macro:url.searchParams.get("macro")}},[]);'
                'return(0,j.jsx)(Selector,{$name:mode,children:{' + ','.join(branches) + '}})}'
                + create + '(document.getElementById("root")).render((0,j.jsx)(Page,{}));})();').encode()

    def test_registry_root_contract_reads_actual_optional_modes_and_both_create_root_forms(self):
        import families
        for macro, chroma, detached in ((False, True, False), (True, False, True), (True, True, False)):
            raw = self.registry_root_fixture(macro=macro, chroma=chroma, detached_create=detached)
            tree, owners, _ = self.registry_ast(raw)
            contract = families.registry_display_root_contract(raw, tree, owners)
            self.assertEqual([row["display_mode"] for row in contract["roots"]],
                             (["macro"] if macro else []) + (["chromaApp"] if chroma else []) + ["default"])
            self.assertEqual(contract["roots"][-1]["chunk_ids"], [1, 2])
            self.assertEqual(contract["roots"][-1]["module_id"], 10)
        source = self.registry_root_fixture(macro=True)
        for before, after in ((b'get("displayMode")', b'get("otherMode")'),
                              (b'require.bind(require,10)', b'require.bind(other,10)'),
                              (b'require.e(1),require.e(2)', b'require.e(1),,require.e(2)'),
                              (b'default:(0,j.jsx)', b'unknown:(0,j.jsx)'),
                              (b'(0,j.jsx)(Selector', b'(effect(),j.jsx)(Selector')):
            changed = source.replace(before, after)
            tree, owners, _ = self.registry_ast(changed)
            with self.subTest(before=before), self.assertRaises(ValueError):
                families.registry_display_root_contract(changed, tree, owners)

    @staticmethod
    def registry_navigation_fixture(*, callback=False, ble=False, fallback=False, fragment=False):
        field = "renderComponent:()=>" if callback else "component:"
        pages = ('[{id:1,name:labels.LIGHTING,' + field + '(0,j.jsx)(shared.A,{})},'
                 '{id:2,name:labels.HELP,' + field + '(0,j.jsx)(Help,{})}]')
        secondary = '[{id:2,name:labels.HELP,' + field + '(0,j.jsx)(Help,{})}]'
        lookup = 'this.state.navs.find(row=>row.name===this.state.active_view)'
        rendered = lookup + ('.renderComponent()' if callback else '.component')
        if fallback:
            rendered = '{let selected=' + lookup + ';return selected||(selected=this.state.navs[0]),selected.component}'
        ready = '(0,j.jsx)(Connected,{})'
        if fragment:
            ready = '(0,j.jsx)(j.Fragment,{children:' + ready + '})'
        return ('(()=>{const modules={101:(module,exports,require)=>{require.d(exports,{default:()=>Root});'
                'const labels=require(201),shared=require(401);class Help extends React.Component{}'
                'class Navigator extends React.Component{constructor(){super();const full=' + pages + ',ble=' + secondary + ';'
                'this.state={navs:' + ('this.props.isBle?ble:full' if ble else 'full') + '};this.renderView=()=>'
                + rendered + ';}render(){return(0,j.jsx)(Frame,{children:this.renderView()})}}'
                'const Connected=(0,redux.connect)(state=>state)(Navigator);'
                'class Gate extends React.Component{constructor(){super();this.renderView=()=>this.props.setupStatus===State.READY?'
                + ready + ':(0,j.jsx)(Loading,{});}render(){return(0,j.jsx)(Frame,{children:this.renderView()})}}'
                'const Root=(0,redux.connect)(state=>state)(Gate);},'
                '201:(module,exports,require)=>{require.d(exports,{LIGHTING:()=>light,HELP:()=>help});const light="TAB_LIGHTING",help="HELP";},'
                '301:(module,exports,require)=>{require.d(exports,{default:()=>Root});const shared=require(401),frame=require(402),tutorial=require(403);'
                'class Chroma extends React.Component{render(){return(0,j.jsxs)(frame.A,{children:['
                '(0,j.jsx)(tutorial.A,{autoOpen:!1}),(0,j.jsx)(shared.A,{isIframe:!0})]})}}'
                'const Root=(0,redux.connect)(state=>state)(Chroma);}};})();').encode()

    def test_registry_navigation_traces_gate_arrays_and_direct_or_callback_rendering(self):
        import families
        for callback, ble, fallback, fragment in ((False, False, True, False), (False, False, False, True), (True, True, False, False)):
            raw = self.registry_navigation_fixture(callback=callback, ble=ble, fallback=fallback, fragment=fragment)
            tree, _, modules = self.registry_ast(raw)
            result = families.registry_default_navigation_contract(raw, tree, modules[101][0][1], modules)
            self.assertEqual(result["owner_name"], "Navigator")
            self.assertEqual([page["kind"] for page in result["groups"][0]["pages"]], ["TAB_LIGHTING", "HELP"])
            self.assertEqual(result["renderer_field"], "renderComponent" if callback else "component")
            self.assertEqual(result["missing_active_view"], "first_navigation" if fallback else "required_match")
            self.assertEqual(len(result["groups"]), 2 if ble else 1)
            if ble:
                self.assertFalse(result["groups"][1]["primary"])
                self.assertEqual([page["kind"] for page in result["groups"][1]["pages"]], ["HELP"])

    def test_registry_navigation_rejects_unwired_gate_unknown_lookup_and_unobserved_rows(self):
        import families
        source = self.registry_navigation_fixture(fallback=True)
        for before, after in ((b'this.props.setupStatus===State.READY', b'this.props.setupStatus===State.UNKNOWN'),
                              (b'children:this.renderView()', b'children:null'),
                              (b'row.name===this.state.active_view', b'row.name===this.state.previous_view'),
                              (b'selected=this.state.navs[0]', b'selected=this.state.navs[1]'),
                              (b'const full=[{id:1', b'const full=[,{id:1'),
                              (b'labels.LIGHTING', b'labels.ABSENT')):
            raw = source.replace(before, after)
            tree, _, modules = self.registry_ast(raw)
            with self.subTest(before=before), self.assertRaises(ValueError):
                families.registry_default_navigation_contract(raw, tree, modules[101][0][1], modules)

    def test_standalone_navigation_matches_shared_page_exports_without_reducer_text_inference(self):
        import families
        raw = self.registry_navigation_fixture()
        tree, _, modules = self.registry_ast(raw)
        default = families.registry_default_navigation_contract(raw, tree, modules[101][0][1], modules)
        standalone = families.registry_standalone_navigation_contract(raw, tree, modules[301][0][1], default["groups"])
        self.assertEqual([page["kind"] for page in standalone["pages"]], ["TAB_LIGHTING"])
        self.assertEqual(standalone["pages"][0]["props"], {"isIframe": "!0"})
        self.assertEqual(len(standalone["other_mounts"]), 2)
        changed = raw.replace(b'shared=require(401),frame=', b'shared=require(999),frame=')
        tree, _, modules = self.registry_ast(changed)
        with self.assertRaisesRegex(ValueError, "references are absent"):
            families.registry_standalone_navigation_contract(changed, tree, modules[301][0][1], default["groups"])

    @staticmethod
    def registry_edition_fixture():
        return (b'const action="device/editionLayoutId",defaults={editionId:0,layoutId:0};'
                b'const reducer=(state,event)=>{switch(event.type){'
                b'case action:return{...state,...links(event.payload.editionId),'
                b'editionId:event.payload.editionId,layoutId:event.payload.layoutId};'
                b'default:return state}};'
                b'const combined=combineReducers({deviceReducer:reducer});')

    def test_registry_editions_keep_ui_and_middleware_declarations_distinct(self):
        import families
        ui = {"definition": {"edition_ids": None}}
        peer = {"status": "verified", "definition": {"edition_ids": [0, 128, 130, 131]}}
        result, evidence = families.registry_edition_metadata(ui, peer, ui_main=self.registry_edition_fixture())
        self.assertEqual(result, [0, 128, 130, 131])
        self.assertIsNone(ui["definition"]["edition_ids"])
        self.assertIsNone(evidence["ui_declared_edition_ids"])
        self.assertEqual(evidence["selected_source_role"], "middleware")
        self.assertEqual(evidence["ui_reducer_contract"]["action"], "device/editionLayoutId")
        initialized = self.registry_edition_fixture().replace(
            b'switch(event.type)', b'switch(state||(state=defaults),observe(event,state),event.type)')
        self.assertEqual(families.registry_ui_edition_contract(initialized)["payload_fields"], ["editionId", "layoutId"])
        ui["definition"]["edition_ids"] = [3, 0]
        result, evidence = families.registry_edition_metadata(ui, peer, ui_main=b'not parsed')
        self.assertEqual(result, [3, 0])
        self.assertEqual(evidence["selected_source_role"], "ui")
        ui["definition"]["edition_ids"] = None
        peer = {"status": "official_source_unavailable"}
        result, evidence = families.registry_edition_metadata(ui, peer, ui_main=self.registry_edition_fixture())
        self.assertIsNone(result)
        self.assertIsNone(evidence["selected_source_role"])

    def test_registry_editions_reject_filtered_wrong_and_unregistered_ui_payloads(self):
        import families
        source = self.registry_edition_fixture()
        for before, after in (
            (b'editionId:event.payload.editionId', b'editionId:event.payload.editionId||0'),
            (b'layoutId:event.payload.layoutId', b'layoutId:event.payload.editionId'),
            (b'layoutId:event.payload.layoutId}', b'layoutId:event.payload.layoutId,...defaults}'),
            (b'deviceReducer:reducer', b'deviceReducer:otherReducer'),
            (b'device/editionLayoutId', b'unrelated/action'),
            (b'switch(event.type)', b'switch(other.type)'),
            (b'switch(event.type)', b'switch(event.payload.editionId=0,event.type)'),
            (b'...state,...links', b'...defaults,...links'),
            (b'...state,...links', b'...state,editionId:0,...links'),
        ):
            with self.subTest(after=after), self.assertRaises(ValueError):
                families.registry_ui_edition_contract(source.replace(before, after))

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

    def test_current_identity_catalog_preserves_duplicate_rows_and_scalar_array_branches(self):
        raw = b'[{"productId":691,"dongleId":692,"bleId":[693]}, {"productId":691}]'
        self.assertEqual(typed_tables.validate_identity_catalog(raw), [
            {"product_id": 691, "dongle_id": 692, "ble_id": [693]}, {"product_id": 691}])
        for value in ([{"productId": 756, "dongleId": []}], [{"productId": 756, "bleId": 70000}],
                      [{"productId": 0}], [{"productId": True}], [{"productId": 756, "middleware": {}}],
                      [{"productId": 756, "xboxId": 757}], [{"productId": 756, "isNotChromaDevice": 1}]):
            with self.subTest(value=value), self.assertRaises(ValueError):
                typed_tables.validate_identity_catalog(json.dumps(value))
        with self.assertRaises(ValueError):
            typed_tables.validate_identity_catalog('[{"productId":756,"productId":757}]')
        self.assertEqual(typed_tables.validate_identity_catalog('[]', allow_empty=True), [])

    def identity_projection_fixture(self):
        catalog = [{"product_id": 105}, {"product_id": 691, "dongle_id": 692, "ble_id": [693]},
                   {"product_id": 691}, {"product_id": 3884, "dongle_id": 3884, "ble_id": 3885}]
        definitions = [
            {"mounted_product_id": 105, "ui": {"productId": 104, "dongleId": 105, "category": "MOUSEPLUSMAT"},
             "middleware": {"productId": 104, "dongleId": 105, "category": "MOUSEPLUSMAT", "claimInterface": 0}},
            {"mounted_product_id": 691, "ui": {"productId": 691, "category": "KEYBOARD", "claimInterface": {"691": 3, "692": 2}},
             "middleware": {"productId": 691, "category": "KEYBOARD", "claimInterface": {"691": 3, "692": 2}}},
            {"mounted_product_id": 3884, "ui": {"productId": 3884, "category": "ACCESSORY"},
             "middleware": {"productId": 3884, "category": "ACCESSORY"}},
            {"mounted_product_id": 3886, "ui": {"productId": 3886, "dongleId": 3884, "bleId": 3885, "category": "ACCESSORY"},
             "middleware": {"productId": 3886, "dongleId": 3884, "bleId": 3885, "category": "ACCESSORY"}},
        ]
        return catalog, definitions, []

    def test_current_identity_constructor_checks_every_field_duplicate_role_and_physical_branch(self):
        catalog, definitions, development = self.identity_projection_fixture()
        raw = typed_tables.render_identity_catalog(catalog, definitions, development)
        self.assertTrue(typed_tables.verify_identity_constructor(raw, catalog, definitions, development)[
            "all_fields_order_duplicates_and_physical_branches_equal"])
        for before, after in ((b'physical_product_id: 692', b'physical_product_id: 693'),
                              (b'interface: 2', b'interface: 3'), (b'ClaimInterface::Uniform(0)', b'ClaimInterface::Uniform(3)'),
                              (b'dongle_id: Some(105)', b'dongle_id: None'),
                              (b'Ids::Many(&[693])', b'Ids::One(693)'),
                              (b'mounted_product_id: 105', b'mounted_product_id: 104'),
                              (b'product_id: 104', b'product_id: 105'),
                              (b'DeviceCategory::Mouseplusmat', b'DeviceCategory::Mouse'),
                              (b'vendor_id: None', b'vendor_id: Some(5426)'),
                              (b'is_not_chroma_device: false', b'is_not_chroma_device: true'),
                              (b'ble_id: None', b'ble_id: None, ignored: None'),
                              (b'IN_DEVELOPMENT: &[u32] = &[]', b'IN_DEVELOPMENT: &[u32] = &[691]')):
            self.assertIn(before, raw)
            with self.subTest(before=before), self.assertRaises(ValueError):
                typed_tables.verify_identity_constructor(raw.replace(before, after, 1), catalog, definitions, development)
        for changed in (catalog[:2] + catalog[3:], [catalog[1], catalog[0], *catalog[2:]]):
            with self.assertRaises(ValueError):
                typed_tables.verify_identity_constructor(
                    typed_tables.render_identity_catalog(changed, definitions, development), catalog, definitions, development)
        missing = copy.deepcopy(definitions)
        missing[0]["ui"] = None
        with self.assertRaises(ValueError):
            typed_tables.verify_identity_constructor(
                typed_tables.render_identity_catalog(catalog, missing, development), catalog, definitions, development)
        for invalid in (definitions[:-2], list(reversed(definitions)), definitions + definitions):
            with self.assertRaises(ValueError):
                typed_tables.render_identity_catalog(catalog, invalid, development)

    def test_identity_definition_validates_all_interface_shapes_and_unobserved_fields(self):
        source = self.transfer_factory('...base,dongleId:0,claimInterface:{756:3,757:2}')
        projection, _ = typed_tables.identity_definition_projection(source)
        self.assertEqual(projection, {"productId":756, "category":"KEYBOARD", "dongleId":0,
                                     "claimInterface":{"756":3,"757":2}})
        for field, value in (("productId", 0), ("vendorId", False), ("dongleId", -1),
                             ("category", "UNKNOWN"), ("claimInterface", {}), ("claimInterface", []),
                             ("claimInterface", True), ("claimInterface", 256),
                             ("claimInterface", {"0":3}), ("claimInterface", {"0756":3}),
                             ("claimInterface", {"756":256}), ("claimInterface", {"756":False})):
            changed = {**projection, field: value}
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                typed_tables.validate_identity_definition(changed)
        absent = typed_tables.identity_definition_semantics({"productId":100,"category":"MOUSE"})
        self.assertIsNone(absent["claim_interface"])
        self.assertIsNone(absent["identity"]["dongle_id"])

    @staticmethod
    def identity_host_fixture():
        return b'''
        const directories={AVAILABLE_DEVICE:`${window.location.origin}/synapse/dashboard/AvailableDevices.json`,
          WINDOW_10_DEVICE:`${window.location.origin}/synapse/dashboard/Windows10Devices.json`,WINDOW_8_DEVICE:null,
          SYNAPSE_2_DEVICE:`${window.location.origin}/synapse/dashboard/Synapse2Devices.json`,
          XBOX_HEADSETS:`${window.location.origin}/synapse/dashboard/XboxHeadsets.json`,
          XBOX_CONTROLLERS:`${window.location.origin}/synapse/dashboard/XboxDevices.json`,
          IN_DEVELOPMENT_DEVICES:`${window.location.origin}/synapse/dashboard/inDevelopmentDevices.json`};
        function aliases(ids,raw){return null!=ids&&(Array.isArray(ids)?ids.includes(raw):ids===raw)}
        class Detector {
          getLatestDeviceData=async()=>{const tasks=Object.values(directories);
            const[available,n,r,o,s,i,development]=await Promise.all(tasks);
            this.available=available.filter(row=>!development.find(other=>other.productId===row.productId));};
          _checkUSBDetail=async device=>{const {productId:raw}=device;
            const monitor=this.available.find(row=>row.monitorPortIds&&row.monitorPortIds.includes(device.productId));
            const matches=this.available.filter(row=>row.productId===raw||aliases(row.dongleId,raw)||aliases(row.bleId,raw)||row.repId===raw||aliases(row.xBoxId,raw)||aliases(row.wiredId,raw)||row.psModeIds?.includes(raw));
            const result={};result.productId=matches[0].repId||matches[0].productId;
            result.isDongle=aliases(matches[0].dongleId,result.realProductId);
            result.isBle=aliases(matches[0].bleId,result.realProductId);
            result.isXBox=aliases(matches[0].xBoxId,result.realProductId);
            result.isPlaystation=matches[0].psModeIds?.includes(result.realProductId);
            return matches.map(({productId:primary,repId:route})=>({productId:route||primary,deviceContainerId:container,type:"available",realProductId:device.productId,baseProductName:name||device.productName,isDongle:!0}));
          }
        }
        '''

    def test_identity_host_contract_checks_alias_matcher_order_filter_and_monitor_find(self):
        source = self.identity_host_fixture()
        contract = typed_tables.identity_discovery_contract(source)
        self.assertTrue(contract["catalog_order_and_duplicates_retained"])
        self.assertTrue(contract["middleware_aliases_are_not_host_candidates"])
        for before, after in ((b'ids===raw', b'ids==raw'),
                              (b'row.wiredId,raw', b'row.middlewareId,raw'),
                              (b'this.available.find(row=>row.monitorPortIds', b'this.available.filter(row=>row.monitorPortIds'),
                              (b'other.productId===row.productId', b'other.dongleId===row.productId'),
                              (b'[available,n,r,o,s,i,development]', b'[development,n,r,o,s,i,available]'),
                              (b'realProductId:device.productId', b'realProductId:primary'),
                              (b'result.isBle=aliases(matches[0].bleId', b'result.isBle=aliases(matches[0].dongleId'),
                              (b'/synapse/dashboard/AvailableDevices.json', b'/synapse/dashboard/OtherDevices.json')):
            with self.subTest(before=before), self.assertRaises(ValueError):
                typed_tables.identity_discovery_contract(source.replace(before, after))

    def identity_acquisition_fixture(self, root):
        receipts = self.transfer_acquisition_fixture(root)
        (root / "tools/frontend.py").write_text("# test source-selection owner\n", encoding="utf-8")
        routes = {}
        for path, document in receipts:
            document.update(target_execution=False, recorded_at_utc="2026-10-30T10:00:00+00:00")
            path.write_text(json.dumps(document), encoding="utf-8")
            routes[document["entry_url"].removesuffix("index.html")] = [path]
        return routes, receipts

    def test_identity_capture_uses_each_endpoint_role_and_retains_declared_aliases(self):
        import analysis
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            routes, receipts = self.identity_acquisition_fixture(root)
            path, document = receipts[1]
            row = document["files"][2]
            raw = (root / row["path"]).read_bytes().replace(b"productId:756", b"productId:755")
            (root / row["path"]).write_bytes(raw)
            row.update(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
            path.write_text(json.dumps(document), encoding="utf-8")
            with patch.object(typed_tables, "ROOT", root), patch.object(analysis, "ROOT", root):
                result, _ = typed_tables._capture_identity_product((756, routes, {}))
            self.assertEqual(result["ui"]["productId"], 756)
            self.assertEqual(result["middleware"]["productId"], 755)
            self.assertEqual(result["mounted_product_id"], 756)

    def test_identity_capture_rejects_missing_manifest_tampered_main_and_latest_failure(self):
        import analysis
        for mutation in ("manifest", "main_bytes", "latest_failure"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                routes, receipts = self.identity_acquisition_fixture(root)
                path, document = receipts[1]
                if mutation == "manifest":
                    document["stability"] = document["stability"][:1]
                elif mutation == "main_bytes":
                    (root / document["files"][2]["path"]).write_bytes(b"changed")
                else:
                    document["source_current_during_acquisition"] = False
                path.write_text(json.dumps(document), encoding="utf-8")
                with patch.object(typed_tables, "ROOT", root), patch.object(analysis, "ROOT", root), self.assertRaises(ValueError):
                    typed_tables._capture_identity_product((756, routes, {}))

    def test_identity_capture_retains_missing_media_without_accepting_a_missing_mounted_script(self):
        import analysis
        for tag, required in ((b'<link rel="icon" href="missing.svg">', False),
                              (b'<script src="missing.js"></script>', True)):
            with self.subTest(required=required), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                routes, receipts = self.identity_acquisition_fixture(root)
                path, document = receipts[1]
                original = document["files"][0]
                raw = (root / original["path"]).read_bytes() + tag
                missing = original["url"].removesuffix("index.html") + ("missing.js" if required else "missing.svg")
                document["files"].append({"url": missing, "error": "HTTP Error 404: Not Found"})
                document["unresolved"] = [{"url": missing, "error": "HTTP Error 404: Not Found"}]
                self.update_identity_original(root, path, document, 0, raw)
                with patch.object(typed_tables, "ROOT", root), patch.object(analysis, "ROOT", root):
                    if required:
                        with self.assertRaises(ValueError):
                            typed_tables._capture_identity_product((756, routes, {}))
                    else:
                        result, _ = typed_tables._capture_identity_product((756, routes, {}))
                        self.assertEqual(result["middleware"]["productId"], 756)
                        retained = json.loads(path.read_bytes())
                        self.assertEqual(retained["unresolved"], document["unresolved"])

    def test_identity_official_absence_requires_two_matching_failures_and_static_acquisition(self):
        import analysis
        import frontend
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            directory = root / ".analysis/inputs/frontend/missing"
            directory.mkdir(parents=True)
            path = directory / "acquisition.json"
            url = "https://apps.razer.com/synapse/products/680/ui/index.html"
            document = {"entry_url":url, "recorded_at_utc":"2026-10-30T10:00:00+00:00",
                        "target_execution":False, "source_current_during_acquisition":False,
                        "files":[{"url":url,"error":"HTTP Error 404: Not Found"}],
                        "stability":[{"url":url,"unchanged":False,"before_sha256":None,"error":"HTTP Error 404: Not Found"}]}
            path.write_text(json.dumps(document), encoding="utf-8")
            with patch.object(typed_tables, "ROOT", root), patch.object(analysis, "ROOT", root):
                result = frontend.current_deployment_source(
                    url.removesuffix("index.html"), candidates=[path], observations={})
                self.assertEqual(result["status"], "official_source_unavailable")
                self.assertEqual(result["http_status"], 404)
                for key, value in (("target_execution",True), ("stability",[]),
                                   ("files",[{"url":url,"error":"HTTP Error 503: Service Unavailable"}])):
                    path.write_text(json.dumps({**document,key:value}), encoding="utf-8")
                    with self.subTest(key=key), self.assertRaises(ValueError):
                        frontend.current_deployment_source(
                            url.removesuffix("index.html"), candidates=[path], observations={})

    def identity_generation_fixture(self, root):
        _, receipts = self.identity_acquisition_fixture(root)
        for name in ("analysis.py", "evidence.py", "host.py"):
            (root / "tools" / name).write_text("# maintained test owner\n", encoding="utf-8")

        def acquire(name, entry, assets):
            directory = root / ".analysis/inputs/frontend" / name
            rows = []
            for index, (url, body) in enumerate(assets):
                path = directory / "raw" / (str(index) + ".source")
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(body)
                rows.append({"url":url, "path":path.relative_to(root).as_posix(),
                             "bytes":len(body), "sha256":hashlib.sha256(body).hexdigest()})
            document = {"entry_url":entry,"files":rows,"target_execution":False,
                        "recorded_at_utc":"2026-10-30T10:00:00+00:00","source_current_during_acquisition":True,
                        "stability":[{"url":row["url"],"unchanged":True,
                                      "before_sha256":row["sha256"],"after_sha256":row["sha256"]} for row in rows]}
            (directory / "acquisition.json").write_text(json.dumps(document), encoding="utf-8")

        acquire("available", typed_tables.IDENTITY_AVAILABLE_URL,
                [(typed_tables.IDENTITY_AVAILABLE_URL, b'[{"productId":756,"dongleId":757,"bleId":758},{"productId":756}]')])
        acquire("development", typed_tables.IDENTITY_DEVELOPMENT_URL, [(typed_tables.IDENTITY_DEVELOPMENT_URL,b'[]')])
        acquire("host", typed_tables.IDENTITY_HOST_URL, [
            (typed_tables.IDENTITY_HOST_URL, b'<script src="assets/index-abc.js"></script>'),
            (typed_tables.IDENTITY_HOST_URL + "assets/index-abc.js", self.identity_host_fixture())])
        return receipts

    @staticmethod
    def update_identity_original(root, receipt, document, index, raw):
        row = document["files"][index]
        (root / row["path"]).write_bytes(raw)
        row.update(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
        for claim in document["stability"]:
            if claim["url"] == row["url"]:
                claim.update(before_sha256=row["sha256"], after_sha256=row["sha256"])
        receipt.write_text(json.dumps(document), encoding="utf-8")

    def registry_identity_fixture(self, root):
        receipts = self.identity_generation_fixture(root)
        registry = root / ".analysis/inputs/typed-tables/razer-catalog/product_registry.json"
        registry.write_text(json.dumps({"schema_version": 1, "products": [{
            "id": 756, "name": "Existing title", "categories": ["KEYBOARD"], "edition_ids": [0],
            "navigations": [{"key": "stable-navigation", "display_mode": "default", "primary": True,
                "pages": [{"key": "stable-help", "kind": "HELP", "role": "Help", "help_selector": 19}]}]}]}), encoding="utf-8")
        selected = []
        for index, (old_receipt, document) in enumerate(receipts):
            name = "UI Keyboard" if index == 0 else "ui keyboard"
            factory = self.transfer_factory('...base,deviceName:' + json.dumps(name),
                export="DeviceInfo:()=>info,AVAILABLE_EDITION_ID:()=>editions")
            factory = factory.replace("productId:756", "productId:755").replace(
                "const deviceName=", "const editions=[3,0];const deviceName=")
            main = ('(self.webpackChunk=self.webpackChunk||[]).push([[1],{' + factory + '}]);').encode()
            self.update_identity_original(root, old_receipt, document, 2, main)
            document["recorded_at_utc"] = "2026-10-30T11:00:00+00:00"
            current = root / ".analysis/inputs/frontend" / ("opaque-updated-owner-" + str(index)) / "acquisition.json"
            current.parent.mkdir()
            current.write_text(json.dumps(document), encoding="utf-8")
            selected.append((current, document))
        return registry, selected

    def test_registry_audit_uses_actual_routes_and_preserves_a_catalog_declared_alias(self):
        import analysis
        import families
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            registry, _ = self.registry_identity_fixture(root)
            original = registry.read_bytes()
            with patch.object(typed_tables, "ROOT", root), patch.object(analysis, "ROOT", root):
                report = families.audit_registry_identity()
            self.assertEqual([row["endpoint_product_id"] for row in report["products"]], [756])
            product = report["products"][0]
            self.assertEqual(product["endpoint_semantics"], "catalog_route_with_declared_alias")
            self.assertEqual(product["ui_identity"]["selection"]["name"], "opaque-updated-owner-0")
            self.assertEqual(product["middleware_identity"]["selection"]["name"], "opaque-updated-owner-1")
            self.assertEqual(product["middleware_identity"]["manufacturer_names"]["status"], "different_source_spellings")
            projected = product["runtime_directory_projection"]
            self.assertEqual((projected["id"], projected["edition_ids"]), (756, [3, 0]))
            page = projected["navigations"][0]["pages"][0]
            self.assertEqual((projected["navigations"][0]["key"], page["key"], page["help_selector"]),
                             ("stable-navigation", "stable-help", 19))
            self.assertEqual(registry.read_bytes(), original)

    def test_registry_alias_exclusion_requires_the_current_independent_catalog_target(self):
        import analysis
        import families
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, selected = self.registry_identity_fixture(root)
            _, ui = selected[0]
            target = copy.deepcopy(ui)
            target["entry_url"] = target["entry_url"].replace("/756/", "/755/")
            for row in [*target["files"], *target["stability"]]:
                row["url"] = row["url"].replace("/756/", "/755/")
            receipt = root / ".analysis/inputs/frontend/actual-alias-target/acquisition.json"
            receipt.parent.mkdir()
            receipt.write_text(json.dumps(target), encoding="utf-8")
            catalog = root / ".analysis/inputs/frontend/available/acquisition.json"
            document = json.loads(catalog.read_bytes())
            self.update_identity_original(root, catalog, document, 0, b'[{"productId":755}]')
            with patch.object(typed_tables, "ROOT", root), patch.object(analysis, "ROOT", root):
                report = families.audit_registry_identity(products=[756])
                self.assertEqual(report["products"][0]["disposition"], "exclude_non_independent_alias_keep_acquisition_locator")
                target["source_current_during_acquisition"] = False
                receipt.write_text(json.dumps(target), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "remains unresolved"):
                    families.audit_registry_identity(products=[756])

    def test_registry_peer_rejects_latest_failure_and_cross_role_product_mismatch(self):
        import analysis
        import families
        import frontend
        for mutation in ("latest_failure", "different_product"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                _, selected = self.registry_identity_fixture(root)
                with patch.object(typed_tables, "ROOT", root), patch.object(analysis, "ROOT", root):
                    routes = frontend.current_acquisition_routes()
                    ui, _ = families.registry_source_identity(756, "ui", candidates=routes[
                        "https://apps.razer.com/synapse/products/756/ui/"], observations={})
                    receipt, document = selected[1]
                    if mutation == "latest_failure":
                        document["source_current_during_acquisition"] = False
                        receipt.write_text(json.dumps(document), encoding="utf-8")
                    else:
                        raw = (root / document["files"][2]["path"]).read_bytes().replace(b"productId:755", b"productId:754")
                        self.update_identity_original(root, receipt, document, 2, raw)
                    with self.assertRaises(ValueError):
                        families.current_registry_peer(756, ui["definition"], candidates=routes[
                            "https://apps.razer.com/synapse/products/756/mw/"], observations={})

    def test_current_identity_generation_checks_the_same_source_owned_artifact(self):
        import analysis
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.identity_generation_fixture(root)
            with patch.object(typed_tables,"ROOT",root), patch.object(analysis,"ROOT",root):
                generated = typed_tables.prepare_current_identity_table(workers=2)
                self.assertEqual(generated["catalog_rows"], 2)
                self.assertTrue(generated["all_product_sources_complete"])
                output = root / typed_tables.IDENTITY_OUTPUT
                raw = output.read_bytes()
                checked = typed_tables.prepare_current_identity_table(check=True, workers=2)
                self.assertTrue(checked["passed"])
                self.assertEqual(output.read_bytes(), raw)
                receipt = json.loads((root / ".analysis/results/typed-data.json").read_bytes())["current_device_identities"]
                self.assertFalse(receipt["published"])
                self.assertEqual(receipt["publication_status"], "verified_existing")
                output.write_bytes(raw.replace(b'ble_id: Some(758)', b'ble_id: None', 1))
                with self.assertRaisesRegex(ValueError,"identities differ"):
                    typed_tables.prepare_current_identity_table(check=True,workers=2)

    def test_default_protocol_generation_uses_the_same_current_identity_schema(self):
        import analysis
        reads = self.current_read_fixture()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.identity_generation_fixture(root)
            sources = {
                typed_tables.READ_INPUT: json.dumps({"schema_version": 1, "products": reads}).encode(),
                typed_tables.QUERY_INPUT: self.source_fixture("receiver-query-capabilities.json"),
                typed_tables.PAIR_CAP_INPUT: self.source_fixture("receiver-pairing-capabilities.json"),
                typed_tables.PAIR_CATALOG_INPUT: self.source_fixture("receiver-pairing-catalog.json"),
            }
            for name, raw in sources.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(raw)
            with patch.object(typed_tables, "ROOT", root), patch.object(analysis, "ROOT", root), \
                    patch.object(typed_tables, "current_read_sources", return_value=(reads, [])):
                typed_tables.prepare_current_identity_table(workers=2)
                output = root / typed_tables.IDENTITY_OUTPUT
                explicit = output.read_bytes()
                generated = typed_tables.prepare_local_protocol_tables(workers=2)
                self.assertIn(typed_tables.IDENTITY_OUTPUT, generated["paths"])
                self.assertEqual(output.read_bytes(), explicit)
                self.assertTrue(typed_tables.prepare_local_protocol_tables(check=True, workers=2)["passed"])
                self.assertEqual(output.read_bytes(), explicit)

    def test_identity_publication_failure_keeps_old_table_and_releases_its_temporary(self):
        import analysis
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.identity_generation_fixture(root)
            with patch.object(typed_tables,"ROOT",root), patch.object(analysis,"ROOT",root):
                typed_tables.prepare_current_identity_table(workers=2)
                output = root / typed_tables.IDENTITY_OUTPUT
                original = output.read_bytes()
                with patch("os.replace",side_effect=OSError("publication denied")), self.assertRaisesRegex(OSError,"publication denied"):
                    typed_tables.prepare_current_identity_table(workers=2)
                self.assertEqual(output.read_bytes(), original)
                self.assertEqual(list(output.parent.glob(".identity-*.rs")), [])

    def test_identity_source_failure_records_incomplete_status_and_preserves_old_table(self):
        import analysis
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            receipts = self.identity_generation_fixture(root)
            with patch.object(typed_tables,"ROOT",root), patch.object(analysis,"ROOT",root):
                typed_tables.prepare_current_identity_table(workers=2)
                output = root / typed_tables.IDENTITY_OUTPUT
                original = output.read_bytes()
                path, document = receipts[1]
                document["stability"] = document["stability"][:1]
                path.write_text(json.dumps(document),encoding="utf-8")
                with self.assertRaisesRegex(ValueError,"source chains failed"):
                    typed_tables.prepare_current_identity_table(workers=2)
                self.assertEqual(output.read_bytes(), original)
                receipt = json.loads((root / ".analysis/results/typed-data.json").read_bytes())["current_device_identities"]
                self.assertFalse(receipt["published"])
                self.assertFalse(receipt["all_product_sources_complete"])
                self.assertEqual([row["mounted_product_id"] for row in receipt["failures"]], [756])

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

    def test_receiver_renderers_keep_provenance_out_of_every_runtime_constructor(self):
        for name, renderer in (
                ("receiver-query-capabilities.json", typed_tables.render_receiver_query_catalog),
                ("receiver-pairing-capabilities.json", typed_tables.render_pairing_capabilities)):
            source = json.loads(self.source_fixture(name))
            transport = (source["capabilities"][0] if "capabilities" in source
                         else source["products"][0]["transport"])
            transport["source_product_id"] = 756
            transport["evidence_path"] = ".analysis/results/frontend/keyboard-mw-756-current/analysis.json"
            raw = json.dumps(source).encode()
            with patch.object(typed_tables, "receiver_evidence_path", side_effect=AssertionError("runtime rendering must not resolve evidence")):
                rendered = renderer(raw)
            self.assertNotIn(b"evidence_path", rendered)
            self.assertNotIn(b".analysis/", rendered)
            tree = typed_tables.receiver_table_tree(rendered)
            pending, constructors = [tree.root_node], []
            while pending:
                node = pending.pop(); pending.extend(node.named_children)
                if node.type == "struct_expression" and node.child_by_field_name("name").text == b"ReceiverCapability":
                    fields = {field.child_by_field_name("field").text.decode(): field.child_by_field_name("value").text
                              for field in node.child_by_field_name("body").named_children}
                    constructors.append(fields)
            self.assertEqual(len(constructors), 1)
            self.assertEqual(set(constructors[0]), typed_tables.RECEIVER_FIELDS - {"evidence_path"} | {"startup_retry"})
            self.assertEqual(constructors[0]["source_product_id"], b"756")
            self.assertEqual(constructors[0]["product_id"], b"241")
            del transport["evidence_path"]
            with self.assertRaises(ValueError):
                renderer(json.dumps(source).encode())

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

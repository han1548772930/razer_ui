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

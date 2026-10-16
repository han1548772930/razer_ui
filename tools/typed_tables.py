"""Generate source-owned typed protocol tables; workspace.py owns the CLI.

JSON is an optional maintenance input, never a build or runtime dependency.
The checked-in Rust table is the production source of fixed capabilities.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
WRITE_INPUT = ".analysis/inputs/typed-tables/razer-device/device-write-capabilities.json"
WRITE_OUTPUT = "crates/razer-device/src/transport/device_write_capabilities.rs"
PAIRING_INPUT = ".analysis/inputs/typed-tables/razer-storage/receiver-pairing-catalog.json"
RESET_INPUT = ".analysis/inputs/typed-tables/razer-storage/receiver-reset-defaults.json"
PAIRING_OUTPUT = "crates/razer-storage/src/receiver/receiver_pairing_catalog.rs"
RESET_OUTPUT = "crates/razer-storage/src/receiver/receiver_reset_defaults.rs"
KINDS = {"dpi": "Dpi", "polling": "Polling", "idle": "Idle"}
METHODS = {
    "setDpiLevel": ("DpiLevel", "dpi", 7),
    "setPollingRate": ("PollingRate", "polling", 1),
    "setProfilePollingRate": ("ProfilePollingRate", "polling", 2),
    "setUSBHighSpeedPollingRate": ("UsbHighSpeedPollingRate", "polling", 2),
    "setTimeToSleep": ("TimeToSleep", "idle", 2),
}
SOURCE_CLASSES = {"rzDevice25DualLinkMouse": "DualLinkMouse", "rzDevice25Legacy": "Legacy"}


def duplicate_free(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("Duplicate source-table JSON key: " + key)
        result[key] = value
    return result


def exact_fields(value, fields, owner):
    if not isinstance(value, dict) or set(value) != set(fields):
        raise ValueError("Unexpected source-table fields: " + owner)


def bounded(value, maximum, owner, minimum=0):
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError("Source-table integer out of range: " + owner)
    return value


def validate_write_catalog(raw):
    catalog = json.loads(raw, object_pairs_hook=duplicate_free)
    exact_fields(catalog, ("schema_version", "products"), "catalog")
    if catalog["schema_version"] != 1 or type(catalog["schema_version"]) is not int:
        raise ValueError("Unsupported device-write schema version")
    products = catalog["products"]
    if not isinstance(products, list) or not products:
        raise ValueError("Device-write product table is empty")
    seen = set()
    for product in products:
        exact_fields(product, ("product_id", "source_class", "min_dpi", "max_dpi", "polling_codes", "writes"), "product")
        pid = bounded(product["product_id"], 0xFFFFFFFF, "product_id", 1)
        if pid == 70 or pid in seen:
            raise ValueError("Duplicate or separately owned legacy product ID: " + str(pid))
        seen.add(pid)
        if product["source_class"] not in SOURCE_CLASSES:
            raise ValueError("Unmapped source class; extend the typed enum from current evidence")
        minimum = bounded(product["min_dpi"], 0xFFFF, "min_dpi", 1)
        maximum = bounded(product["max_dpi"], 0xFFFF, "max_dpi", 1)
        if minimum > maximum:
            raise ValueError("Source DPI range is reversed")
        if not isinstance(product["polling_codes"], dict):
            raise ValueError("Polling codes must be a numeric-rate mapping")
        for rate, code in product["polling_codes"].items():
            match = re.fullmatch(r"RATE_([1-9][0-9]*)Hz", rate)
            if not match:
                raise ValueError("Unrecognized polling-rate key: " + rate)
            bounded(int(match[1]), 0xFFFFFFFF, "polling Hz", 1)
            bounded(code, 0xFF, "polling code")
        if not isinstance(product["writes"], list) or not product["writes"]:
            raise ValueError("Product write-command table is empty")
        kinds = set()
        for command in product["writes"]:
            exact_fields(command, ("kind", "method", "command", "selector", "read_kind", "min_response_bytes"), "write command")
            kind = command["kind"]
            if kind not in KINDS or kind in kinds or command["read_kind"] != kind:
                raise ValueError("Duplicate or mismatched typed write/read kind")
            kinds.add(kind)
            method = METHODS.get(command["method"])
            if method is None or method[1] != kind:
                raise ValueError("Write method does not match its typed setting")
            header = command["command"]
            if not isinstance(header, list) or len(header) != 3:
                raise ValueError("Write command must retain three protocol header bytes")
            for value in header:
                bounded(value, 0xFF, "command byte")
            if header[0] != method[2]:
                raise ValueError("Write command payload length differs from its current method")
            bounded(command["selector"], 0xFF, "selector")
            bounded(command["min_response_bytes"], 80, "min_response_bytes")
        if "polling" in kinds and not product["polling_codes"]:
            raise ValueError("Polling setter has no source rate codes")
    return sorted(products, key=lambda product: product["product_id"])


def render_write_catalog(raw):
    products = validate_write_catalog(raw)
    lines = [
        "//! Fixed source-derived capabilities. Regenerate with workspace.py prepare-device-write-table.",
        "//! Maintenance input SHA-256: " + hashlib.sha256(raw).hexdigest(),
        "//! No source JSON parsing, file access or allocation is needed to select a capability.",
        "use crate::device_reads::DeviceReadKind;",
        "use crate::device_writes::{",
        "    DeviceWriteCapability, DeviceWriteKind, PollingCode, WriteCommand, WriteMethod,",
        "    WriteSourceClass,",
        "};", "",
        "#[rustfmt::skip]",
        "pub(crate) static PRODUCTS: &[DeviceWriteCapability] = &[",
    ]
    for product in products:
        lines.extend([
            "    DeviceWriteCapability {",
            f"        product_id: {product['product_id']},",
            f"        source_class: WriteSourceClass::{SOURCE_CLASSES[product['source_class']]},",
            f"        min_dpi: {product['min_dpi']},",
            f"        max_dpi: {product['max_dpi']},",
            "        polling_codes: &[",
        ])
        rates = sorted((int(re.fullmatch(r"RATE_([1-9][0-9]*)Hz", name)[1]), code)
                       for name, code in product["polling_codes"].items())
        for hz, code in rates:
            lines.append(f"            PollingCode {{ hz: {hz}, code: {code} }},")
        lines.extend(["        ],", "        writes: &["])
        for command in product["writes"]:
            lines.extend([
                "            WriteCommand {",
                f"                kind: DeviceWriteKind::{KINDS[command['kind']]},",
                f"                method: WriteMethod::{METHODS[command['method']][0]},",
                f"                command: {command['command']},",
                f"                selector: {command['selector']},",
                f"                read_kind: DeviceReadKind::{KINDS[command['read_kind']]},",
                f"                min_response_bytes: {command['min_response_bytes']},",
                "            },",
            ])
        lines.extend(["        ],", "    },"])
    lines.extend(["];", ""])
    return "\n".join(lines).encode("utf-8")


def local_path(value):
    path = ROOT / value
    if not path.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError("Table path must stay within the workspace")
    current = path
    while current != ROOT and current != current.parent:
        if current.is_symlink() or (current.exists() and getattr(current.lstat(), "st_file_attributes", 0) & 0x400):
            raise ValueError("Table path traverses a link")
        current = current.parent
    return path


def prepare_write_table(source=WRITE_INPUT, check=False):
    raw = local_path(source).read_bytes()
    result = render_write_catalog(raw)
    output = local_path(WRITE_OUTPUT)
    if check:
        if output.read_bytes() != result:
            raise ValueError("Typed write table differs from the maintenance input; regenerate it")
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(result)
    products = validate_write_catalog(raw)
    return {"path": WRITE_OUTPUT, "products": len(products),
            "commands": sum(len(product["writes"]) for product in products),
            "sha256": hashlib.sha256(result).hexdigest(), "passed": True,
            "scope": "maintenance generation only; no runtime JSON or device operations"}


def boolean(value, owner):
    if type(value) is not bool:
        raise ValueError("Source-table boolean is invalid: " + owner)
    return "true" if value else "false"


def option_bool(value, key):
    return "Some(" + boolean(value[key], key) + ")" if key in value else "None"


def render_pairing_storage(raw):
    catalog = json.loads(raw, object_pairs_hook=duplicate_free)
    exact_fields(catalog, ("schema_version", "source_scope", "masters", "devices", "available", "source_evidence"),
                 "pairing storage catalog")
    if type(catalog.get("schema_version")) is not int or catalog["schema_version"] != 1:
        raise ValueError("Unsupported source pairing catalog schema")
    if not isinstance(catalog["available"], list) or not catalog["available"] or not isinstance(catalog["source_scope"], str) or not catalog["source_scope"]:
        raise ValueError("Pairing storage catalog has no available source scope")
    masters = catalog.get("masters")
    devices = catalog.get("devices")
    if not isinstance(masters, list) or not masters or not isinstance(devices, list) or not devices:
        raise ValueError("Pairing source masters and devices must be present")
    lines = [
        "//! Source pairing storage projection; runtime uses no JSON catalog.",
        "//! Maintenance input SHA-256: " + hashlib.sha256(raw).hexdigest(),
        "use crate::receiver_defaults::PairingMaster;", "", "#[rustfmt::skip]",
        "pub(crate) static MASTERS: &[PairingMaster] = &[",
    ]
    seen_masters = set()
    for master in masters:
        required = {"productId", "dongleId", "bleId", "claimInterface", "canPairTwoDevices"}
        if not isinstance(master, dict) or not required <= set(master) or set(master) - required - {"supports8KHzPollingRate"}:
            raise ValueError("Pairing storage source master has unmapped fields")
        product = bounded(master["productId"], 0xFFFF, "master product", 1)
        if product in seen_masters:
            raise ValueError("Duplicate pairing storage master")
        seen_masters.add(product)
        dongle = bounded(master["dongleId"], 0xFFFF, "master dongle", 1)
        ble = bounded(master["bleId"], 0xFFFF, "master ble")
        interface = bounded(master["claimInterface"], 0xFF, "claim interface")
        boolean(master["canPairTwoDevices"], "canPairTwoDevices")
        lines.extend([
            "    PairingMaster {",
            f"        product_id: {product}, dongle_id: {dongle}, ble_id: {ble}, claim_interface: {interface},",
            "        supports_eight_khz: " + option_bool(master, "supports8KHzPollingRate") + ",",
            "    },",
        ])
    radios = []
    for device in devices:
        required = {"ProductInfo", "DeviceDonglePid", "DevicePairingBuddies", "DeviceType"}
        if not isinstance(device, dict) or not required <= set(device) or set(device) - required - {"DeviceFamily", "DeviceSiblings"}:
            raise ValueError("Pairing storage source radio has unmapped fields")
        if device["DeviceType"] not in ("MOUSE", "KEYBOARD", "MOUSEMAT", "ACCESSORY"):
            raise ValueError("Pairing storage source radio has an unknown category")
        if not isinstance(device["ProductInfo"], list) or not device["ProductInfo"]:
            raise ValueError("Pairing storage radio has no source editions")
        editions = set()
        for edition in device["ProductInfo"]:
            exact_fields(edition, ("EID", "Name", "CHSName"), "pairing radio edition")
            edition_id = bounded(edition["EID"], 0xFFFFFFFF, "pairing edition")
            if edition_id in editions or any(not isinstance(edition[key], str) or not edition[key] for key in ("Name", "CHSName")):
                raise ValueError("Pairing storage source edition is duplicated or unnamed")
            editions.add(edition_id)
        for field in ("DevicePairingBuddies", "DeviceSiblings"):
            if field in device:
                if not isinstance(device[field], list):
                    raise ValueError("Pairing storage source radio IDs must remain an ordered array")
                for identity in device[field]:
                    bounded(identity, 0xFFFF, field, 1)
        if "DeviceFamily" in device and not isinstance(device["DeviceFamily"], str):
            raise ValueError("Pairing storage source family must be a string")
        radio = bounded(device["DeviceDonglePid"], 0xFFFE, "source radio", 1)
        if radio in radios:
            raise ValueError("Duplicate pairing storage radio")
        radios.append(radio)
    lines.extend([
        "];", "", "// Source order is retained; this set only gates the source756 callback.",
        "#[rustfmt::skip]",
        "pub(crate) static RADIO_IDS: &[u16] = &[",
    ])
    for offset in range(0, len(radios), 10):
        lines.append("    " + ", ".join(map(str, radios[offset:offset + 10])) + ",")
    lines.extend(["];", ""])
    return "\n".join(lines).encode("utf-8")


def render_reset_storage(raw):
    catalog = json.loads(raw, object_pairs_hook=duplicate_free)
    exact_fields(catalog, ("schema_version", "products"), "reset catalog")
    if type(catalog["schema_version"]) is not int or catalog["schema_version"] != 1:
        raise ValueError("Unsupported receiver reset schema")
    if not isinstance(catalog["products"], list) or not catalog["products"]:
        raise ValueError("Receiver reset defaults are empty")
    lines = [
        "//! Source local reset documents; no JSON parsing or remote resource is required.",
        "//! Maintenance input SHA-256: " + hashlib.sha256(raw).hexdigest(),
        "use crate::receiver_defaults::{",
        "    Brightness, GlobalDocument, ProfileDocument, QuickEffects, ReportIds, ResetDefaults,",
        "    SwitchOffLighting,", "};", "", "#[rustfmt::skip]",
        "pub(crate) static PRODUCTS: &[ResetDefaults] = &[",
    ]
    seen = set()
    for source in catalog["products"]:
        exact_fields(source, ("product_id", "edition_id", "device_name", "schema_version", "default_profile",
                             "default_global", "actual_task", "factory_reset_command_reachable",
                             "preserves_existing_profiles", "refresh_events"), "reset product")
        product = bounded(source["product_id"], 0xFFFFFFFF, "reset product", 1)
        edition = bounded(source["edition_id"], 0xFFFFFFFF, "reset edition")
        identity = (product, edition)
        if identity not in ((164, 0), (241, 0)):
            raise ValueError("Receiver reset product/edition needs its current-source implementation")
        if identity in seen:
            raise ValueError("Duplicate reset product/edition")
        seen.add(identity)
        schema = bounded(source["schema_version"], 0xFFFFFFFF, "reset schema", 1)
        if schema != 13:
            raise ValueError("Receiver reset schema needs its current-source migration")
        name = source["device_name"]
        if not isinstance(name, str) or not name or any(ord(character) < 32 for character in name):
            raise ValueError("Invalid reset device name")
        if (source["actual_task"] != "taskMakerResetOBM" or source["factory_reset_command_reachable"] is not False
                or source["preserves_existing_profiles"] is not True
                or source["refresh_events"] != ["ON_INIT_BRIGHTNESS", "ON_SET_EFFECTS", "ON_SET_KEYMAPPING"]):
            raise ValueError("Reset task semantics differ from the implemented source chain")
        profile = source["default_profile"]
        global_document = source["default_global"]
        profile_fields = {"guid", "name", "brightness", "switchOffLighting", "quickEffects", "mappings"}
        if not isinstance(profile, dict) or not profile_fields <= set(profile) or set(profile) - profile_fields - {"canPairTwoDevices"}:
            raise ValueError("Reset profile has unmapped source fields")
        if profile["guid"] != "" or profile["name"] != " " or profile["mappings"] != []:
            raise ValueError("Reset profile identity or mapping defaults changed")
        brightness = profile["brightness"]
        exact_fields(brightness, ("isEnabled", "value"), "brightness")
        enabled = boolean(brightness["isEnabled"], "brightness enabled")
        brightness_value = bounded(brightness["value"], 100, "brightness percent")
        lighting = profile["switchOffLighting"]
        exact_fields(lighting, ("isDisplayOn", "isIdleEnabled", "idleMinutes"), "lighting idle")
        display = boolean(lighting["isDisplayOn"], "display on")
        idle = boolean(lighting["isIdleEnabled"], "idle enabled")
        minutes = bounded(lighting["idleMinutes"], 0xFFFF, "idle minutes")
        exact_fields(profile["quickEffects"], ("selectedEffectId",), "quick effect")
        effect = bounded(profile["quickEffects"]["selectedEffectId"], 0xFFFF, "effect ID")
        global_fields = {"version", "vendorId", "productId", "category", "reportIDs"}
        if not isinstance(global_document, dict) or not global_fields <= set(global_document) or set(global_document) - global_fields - {"canPairTwoDevices"}:
            raise ValueError("Reset global defaults have unmapped source fields")
        if global_document["productId"] != product or global_document["category"] != "ACCESSORY":
            raise ValueError("Reset global owner/category differs from its source")
        version = bounded(global_document["version"], (1 << 53) - 1, "default version")
        vendor = bounded(global_document["vendorId"], 0xFFFF, "vendor ID", 1)
        if vendor != 5426:
            raise ValueError("Receiver reset vendor differs from its protocol owner")
        for document in (profile, global_document):
            if ((product == 164 and "canPairTwoDevices" in document)
                    or (product == 241 and document.get("canPairTwoDevices") is not True)):
                raise ValueError("Receiver reset canPairTwoDevices field presence differs from its source branch")
        reports = global_document["reportIDs"]
        if reports != {"4": "razerKeyReportID", "5": "hardwareEventReportID"}:
            raise ValueError("Reset source report-ID meanings changed")
        lines.extend([
            "    ResetDefaults {",
            f"        product_id: {product}, edition_id: {edition},",
            "        device_name: " + json.dumps(name, ensure_ascii=False) + ",",
            f"        schema_version: {schema},",
            "        default_profile: ProfileDocument {",
            '            guid: "", name: " ",',
            f"            brightness: Brightness {{ is_enabled: {enabled}, value: {brightness_value} }},",
            f"            switch_off_lighting: SwitchOffLighting {{ is_display_on: {display}, is_idle_enabled: {idle}, idle_minutes: {minutes} }},",
            f"            quick_effects: QuickEffects {{ selected_effect_id: {effect} }},",
            "            mappings: &[],",
            "            can_pair_two_devices: " + option_bool(profile, "canPairTwoDevices") + ",",
            "        },",
            "        default_global: GlobalDocument {",
            f"            version: {version}, vendor_id: {vendor}, product_id: {product},",
            '            category: "ACCESSORY",',
            '            report_ids: ReportIds { key_report: "razerKeyReportID", hardware_event: "hardwareEventReportID" },',
            "            can_pair_two_devices: " + option_bool(global_document, "canPairTwoDevices") + ",",
            "        },",
            "    },",
        ])
    lines.extend(["];", ""])
    return "\n".join(lines).encode("utf-8")


def prepare_receiver_storage(pairing=PAIRING_INPUT, reset=RESET_INPUT, check=False):
    outputs = [(PAIRING_OUTPUT, render_pairing_storage(local_path(pairing).read_bytes())),
               (RESET_OUTPUT, render_reset_storage(local_path(reset).read_bytes()))]
    for name, raw in outputs:
        output = local_path(name)
        if check:
            if output.read_bytes() != raw:
                raise ValueError("Typed receiver storage differs from its source: " + name)
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(raw)
    return {"paths": [name for name, _ in outputs], "passed": True,
            "scope": "typed local defaults; no runtime JSON catalog, network or device operations"}



READ_INPUT = ".analysis/inputs/typed-tables/razer-device/device-read-capabilities.json"
IDENTITY_INPUT = ".analysis/inputs/typed-tables/razer-device/discovery-catalog.json"
QUERY_INPUT = ".analysis/inputs/typed-tables/razer-device/receiver-query-capabilities.json"
PAIR_CAP_INPUT = ".analysis/inputs/typed-tables/razer-device/receiver-pairing-capabilities.json"
PAIR_CATALOG_INPUT = ".analysis/inputs/typed-tables/razer-device/receiver-pairing-catalog.json"
READ_OUTPUT = "crates/razer-device/src/transport/device_read_capabilities.rs"
IDENTITY_OUTPUT = "crates/razer-device/src/identity/device_identity_catalog.rs"
QUERY_OUTPUT = "crates/razer-device/src/receiver/receiver_query_capabilities.rs"
PAIR_CAP_OUTPUT = "crates/razer-device/src/receiver/receiver_pairing_capabilities.rs"
PAIR_CATALOG_OUTPUT = "crates/razer-device/src/receiver/receiver_pairing_catalog.rs"

READ_METHODS = {
    "getExtendedFirmwareVersion": ("ExtendedFirmware", "firmware", [4, 0, 135], [], 4),
    "getBatteryLevel": ("BatteryLevel", "battery", [2, 7, 128], [0], 2),
    "getChargingStatus": ("ChargingStatus", "charging", [2, 7, 132], [0], 2),
    "getPollingRate": ("PollingRate", "polling", [1, 0, 133], [], 1),
    "getProfilePollingRate": ("ProfilePollingRate", "polling", [2, 0, 142], [1], 2),
    "getUSBHighSpeedPollingRate": ("UsbHighSpeedPollingRate", "polling", [2, 0, 192], [1], 2),
    "getDpiLevel": ("DpiLevel", "dpi", [7, 4, 133], [0], 7),
    "getTimeToSleep": ("TimeToSleep", "idle", [2, 7, 131], [], 2),
}
READ_KINDS = {"firmware": "Firmware", "battery": "Battery", "charging": "Charging", **KINDS}
READ_CLASSES = {"rzDevice25", "rzDevice25DualLinkMouse", "rzDevice25DualLinkKeyboard"}
RECEIVER_CLASSES = READ_CLASSES | {"rzDevice25Linker", "rzDevice25LinkerUma", "rzDevice25LinkerMultiDevices"}
IDENTITY_CATEGORIES = {
    "ACCESSORY": "Accessory", "AUDIO": "Audio", "BROADCASTER": "Broadcaster", "CASE": "Case",
    "CHAIR": "Chair", "EGPU": "Egpu", "GAMEPAD": "Gamepad", "HUE": "Hue", "IOT": "Iot",
    "KEYBOARD": "Keyboard", "KEYPAD": "Keypad", "MONITOR": "Monitor", "MOUSE": "Mouse",
    "MOUSEMAT": "Mousemat", "MOUSEPLUSMAT": "Mouseplusmat", "SYSTEM": "System",
}
PAIRING_CATEGORIES = {"MOUSE": "Mouse", "KEYBOARD": "Keyboard", "ACCESSORY": "Accessory", "MOUSEMAT": "MouseMat"}
POLLING_HZ = {100, 125, 200, 250, 333, 500, 1000, 2000, 4000, 8000}
CHARGING_NAMES = {"NoCharge_BatteryFull", "Charging", "ChargingFailed", "ForeignObjectDetection", "FastCharging", "FullCharging", "ReachChargingLimit"}


def fields_with_optional(value, required, optional, owner):
    if not isinstance(value, dict) or not set(required) <= set(value) or set(value) - set(required) - set(optional):
        raise ValueError("Unmapped source-table fields: " + owner)


def rust_string(value):
    if not isinstance(value, str) or any(ord(char) < 32 or ord(char) == 127 for char in value):
        raise ValueError("Invalid source-table string")
    return json.dumps(value, ensure_ascii=False)


def source_list(value, owner, nonempty=False):
    if not isinstance(value, list) or (nonempty and not value):
        raise ValueError("Invalid source-table list: " + owner)
    return value


def usb_product(value, owner="USB product ID", allow_zero=False):
    return bounded(value, 0xFFFF, owner, 0 if allow_zero else 1)


def product_ids(value, owner, nonempty=False):
    for item in source_list(value, owner, nonempty):
        usb_product(item, owner)
    return value


def protocol_bytes(value, owner, length=None):
    source_list(value, owner)
    if length is not None and len(value) != length:
        raise ValueError("Unexpected source protocol byte count: " + owner)
    for item in value:
        bounded(item, 0xFF, owner)
    return value


def namespace(prefix, modulus):
    bounded(prefix, 0xFF, "transaction prefix")
    bounded(modulus, 0xFF, "transaction modulus", 1)
    bits = 0
    for counter in range(modulus):
        bits |= counter
    if prefix & bits:
        raise ValueError("Transaction namespace overlaps its source counter")


def validate_transport(value, receiver=False):
    for field in ("claim_interface", "report_id"):
        bounded(value[field], 0xFF, field)
    for field in ("max_retry_in", "max_retry_out"):
        bounded(value[field], 0xFF, field, 1)
    for field in ("sleep_between_out_ms", "sleep_between_out_in_ms", "sleep_between_in_ms"):
        bounded(value[field], (1 << 64) - 1, field)
    namespace(value["transaction_prefix"], value["transaction_modulus"])
    if value["source_class"] not in (RECEIVER_CLASSES if receiver else READ_CLASSES):
        raise ValueError("Unknown source transport class")


def validate_code_map(value, charging=False):
    if not isinstance(value, dict):
        raise ValueError("Protocol codes must be a mapping")
    seen = set()
    for name, code in value.items():
        if charging:
            if name not in CHARGING_NAMES:
                raise ValueError("Unknown source charging status")
        else:
            match = re.fullmatch(r"RATE_([1-9][0-9]*)Hz", name)
            if match is None or int(match[1]) not in POLLING_HZ:
                raise ValueError("Unknown source polling rate")
        bounded(code, 0xFF, "protocol enum code")
        if code in seen:
            raise ValueError("Ambiguous source protocol code")
        seen.add(code)


def validate_read_catalog(raw):
    catalog = json.loads(raw, object_pairs_hook=duplicate_free)
    exact_fields(catalog, ("schema_version", "products"), "read catalog")
    if type(catalog["schema_version"]) is not int or catalog["schema_version"] != 1:
        raise ValueError("Unsupported device-read schema")
    products = source_list(catalog["products"], "read products", True)
    seen = set()
    transport_fields = {"source_class", "claim_interface", "report_id", "transaction_prefix", "transaction_modulus",
                        "max_retry_in", "max_retry_out", "sleep_between_out_ms", "sleep_between_out_in_ms", "sleep_between_in_ms"}
    required = transport_fields | {"product_id", "direct_pids", "vendor_id", "report_bytes", "queries", "polling_codes", "charging_codes"}
    optional = {"claim_interfaces_by_pid", "physical_transports_by_pid", "polling_value_offset", "polling_default_hz", "polling_physical_product_id", "alternate_commands"}
    for product in products:
        fields_with_optional(product, required, optional, "read product")
        pid = usb_product(product["product_id"])
        if pid in seen:
            raise ValueError("Repeated read source product")
        seen.add(pid)
        product_ids(product["direct_pids"], "direct products", True)
        bounded(product["vendor_id"], 0xFFFF, "vendor ID", 1)
        if type(product["report_bytes"]) is not int or product["report_bytes"] != 91:
            raise ValueError("Invalid source feature report length")
        validate_transport(product)
        for field in ("claim_interfaces_by_pid", "physical_transports_by_pid"):
            mapping = product.get(field, {})
            if not isinstance(mapping, dict):
                raise ValueError("Physical source branches must be a PID mapping")
            for key, branch in mapping.items():
                if not re.fullmatch(r"[1-9][0-9]*", key):
                    raise ValueError("Physical PID must be canonical decimal")
                usb_product(int(key))
                if field == "claim_interfaces_by_pid":
                    bounded(branch, 0xFF, "physical claim interface")
                else:
                    exact_fields(branch, transport_fields | {"is_duallink"}, "physical transport")
                    validate_transport(branch)
                    boolean(branch["is_duallink"], "is_duallink")
        if "polling_value_offset" in product:
            bounded(product["polling_value_offset"], 79, "polling offset")
        if "polling_default_hz" in product and product["polling_default_hz"] not in POLLING_HZ:
            raise ValueError("Unknown source polling default")
        if "polling_physical_product_id" in product:
            usb_product(product["polling_physical_product_id"])
        kinds = set()
        for query in source_list(product["queries"], "read queries", True):
            exact_fields(query, ("name", "method", "command", "payload", "min_response_bytes"), "read query")
            method = READ_METHODS.get(query["method"])
            if method is None or query["name"] != method[1] or query["name"] in kinds:
                raise ValueError("Unknown, repeated or mismatched read method/kind")
            kinds.add(query["name"])
            protocol_bytes(query["command"], "read command", 3)
            protocol_bytes(query["payload"], "read selector")
            bounded(query["min_response_bytes"], 80, "read minimum", 1)
            if (query["command"], query["payload"], query["min_response_bytes"]) != (method[2], method[3], method[4]):
                raise ValueError("Read method differs from its source packet/decoder contract")
            if query["name"] == "polling" and product.get("polling_value_offset", 1) >= query["min_response_bytes"]:
                raise ValueError("Polling offset is outside the source response")
        validate_code_map(product["polling_codes"])
        validate_code_map(product["charging_codes"], charging=True)
        for alternate in source_list(product.get("alternate_commands", []), "alternate commands"):
            exact_fields(alternate, ("command",), "alternate command")
            protocol_bytes(alternate["command"], "alternate command", 3)
            if alternate["command"] not in ([1, 0, 70], [3, 0, 65]):
                raise ValueError("Unmapped alternate source command")
    return products


def option_expr(value, key, render=str):
    return "Some(" + render(value[key]) + ")" if key in value else "None"


def rust_vec(value):
    return "vec![" + ", ".join(map(str, value)) + "]"


def rust_code_map(value):
    return "BTreeMap::from([" + ", ".join("(" + rust_string(key) + ".into(), " + str(code) + ")" for key, code in value.items()) + "])"


def generated_header(raw, title, imports):
    return ["//! " + title, "//! Maintenance input SHA-256: " + hashlib.sha256(raw).hexdigest(),
            "//! No source JSON parsing or runtime resource access is required.", *imports, "", "#[rustfmt::skip]"]


def render_read_catalog(raw):
    products = validate_read_catalog(raw)
    lines = generated_header(raw, "Fixed local device query capabilities.", [
        "use std::collections::BTreeMap;",
        "use crate::device_reads::{DeviceReadCapability, DeviceReadKind, PhysicalFeatureTransport, ReadCommand, SourceReadMethod};",
    ])
    lines.extend(["pub(crate) fn products() -> Vec<DeviceReadCapability> {", "    vec!["])
    scalar_fields = ("product_id", "vendor_id", "claim_interface", "report_id", "report_bytes", "transaction_prefix", "transaction_modulus",
                     "max_retry_in", "max_retry_out", "sleep_between_out_ms", "sleep_between_out_in_ms", "sleep_between_in_ms")
    branch_fields = ("claim_interface", "report_id", "transaction_prefix", "transaction_modulus", "max_retry_in", "max_retry_out",
                     "sleep_between_out_ms", "sleep_between_out_in_ms", "sleep_between_in_ms")
    for product in products:
        lines.extend(["        DeviceReadCapability {", "            source_class: " + rust_string(product["source_class"]) + ".into(),"])
        lines.extend(f"            {field}: {product[field]}," for field in scalar_fields)
        lines.append("            direct_pids: " + rust_vec(product["direct_pids"]) + ",")
        lines.append("            claim_interfaces_by_pid: " + rust_code_map(product.get("claim_interfaces_by_pid", {})) + ",")
        lines.append("            physical_transports_by_pid: BTreeMap::from([")
        for pid, branch in product.get("physical_transports_by_pid", {}).items():
            lines.extend(["                (" + rust_string(pid) + ".into(), PhysicalFeatureTransport {",
                          "                    source_class: " + rust_string(branch["source_class"]) + ".into(),"])
            lines.extend(f"                    {field}: {branch[field]}," for field in branch_fields)
            lines.extend(["                    is_duallink: " + boolean(branch["is_duallink"], "is_duallink") + ",", "                }),"])
        lines.extend(["            ]),", "            queries: vec!["])
        for query in product["queries"]:
            lines.extend(["                ReadCommand {",
                          "                    name: DeviceReadKind::" + READ_KINDS[query["name"]] + ",",
                          "                    method: SourceReadMethod::" + READ_METHODS[query["method"]][0] + ".name().into(),",
                          "                    command: " + str(query["command"]) + ",",
                          "                    payload: " + rust_vec(query["payload"]) + ",",
                          f"                    min_response_bytes: {query['min_response_bytes']},", "                },"])
        lines.append("            ],")
        for field in ("polling_codes", "charging_codes"):
            lines.append(f"            {field}: " + rust_code_map(product[field]) + ",")
        for field in ("polling_value_offset", "polling_default_hz", "polling_physical_product_id"):
            lines.append(f"            {field}: " + option_expr(product, field) + ",")
        lines.append("        },")
    lines.extend(["    ]", "}", ""])
    return "\n".join(lines).encode("utf-8")


def validate_identity_catalog(raw):
    products = source_list(json.loads(raw, object_pairs_hook=duplicate_free), "identity catalog", True)
    seen = set()
    optional = {"rep_id", "dongle_id", "ble_id", "xbox_id", "wired_id", "ps_mode_ids", "monitor_port_ids", "is_not_chroma_device", "middleware"}
    for product in products:
        fields_with_optional(product, ("product_id",), optional, "identity product")
        pid = usb_product(product["product_id"])
        if pid in seen:
            raise ValueError("Repeated identity source product")
        seen.add(pid)
        if "rep_id" in product:
            usb_product(product["rep_id"], allow_zero=True)
        for field in ("dongle_id", "ble_id", "xbox_id", "wired_id"):
            if field in product:
                value = product[field]
                if isinstance(value, list):
                    product_ids(value, field, True)
                else:
                    usb_product(value, field)
        for field in ("ps_mode_ids", "monitor_port_ids"):
            if field in product:
                product_ids(product[field], field, True)
        if "is_not_chroma_device" in product:
            boolean(product["is_not_chroma_device"], "is_not_chroma_device")
        if "middleware" in product:
            facts = product["middleware"]
            fields_with_optional(facts, (), ("dongle_id", "ble_id", "claim_interface", "category", "receipt"), "middleware identity")
            for field in ("dongle_id", "ble_id"):
                if field in facts:
                    usb_product(facts[field], field, True)
            if "claim_interface" in facts:
                bounded(facts["claim_interface"], 0xFF, "claim interface")
            if "category" in facts and facts["category"] not in IDENTITY_CATEGORIES:
                raise ValueError("Unknown middleware identity category")
            if "receipt" in facts and not isinstance(facts["receipt"], dict):
                raise ValueError("Identity maintenance receipt must be an evidence object")
    return products


def render_identity_catalog(raw):
    products = validate_identity_catalog(raw)
    lines = generated_header(raw, "Fixed source product identities; no hardware observations.", [
        "use crate::device_identity::{CatalogProduct, DeviceCategory, Ids, MiddlewareFacts};",
    ])
    lines.extend(["pub(crate) fn products() -> Vec<CatalogProduct> {", "    vec!["])
    for product in products:
        lines.extend(["        CatalogProduct {", f"            product_id: {product['product_id']},",
                      "            rep_id: " + option_expr(product, "rep_id") + ","])
        for field in ("dongle_id", "ble_id", "xbox_id", "wired_id"):
            lines.append(f"            {field}: " + option_expr(product, field, lambda ids: "Ids::Many(" + rust_vec(ids) + ")" if isinstance(ids, list) else f"Ids::One({ids})") + ",")
        for field in ("ps_mode_ids", "monitor_port_ids"):
            lines.append(f"            {field}: " + option_expr(product, field, rust_vec) + ",")
        lines.append("            is_not_chroma_device: " + boolean(product.get("is_not_chroma_device", False), "is_not_chroma_device") + ",")
        if "middleware" in product:
            facts = product["middleware"]
            lines.append("            middleware: Some(MiddlewareFacts {")
            for field in ("dongle_id", "ble_id", "claim_interface"):
                lines.append(f"                {field}: " + option_expr(facts, field) + ",")
            lines.extend(["                category: " + option_expr(facts, "category", lambda category: "DeviceCategory::" + IDENTITY_CATEGORIES[category]) + ",",
                          "                _receipt: None,", "            }),"])
        else:
            lines.append("            middleware: None,")
        lines.append("        },")
    lines.extend(["    ]", "}", ""])
    return "\n".join(lines).encode("utf-8")


RECEIVER_FIELDS = {"source_product_id", "vendor_id", "product_id", "claim_interface", "report_bytes", "report_id", "protocol", "command",
                   "transaction_prefix", "transaction_modulus", "max_retry_in", "max_retry_out", "sleep_between_out_ms", "sleep_between_out_in_ms",
                   "sleep_between_in_ms", "peer_match_product_id", "source_class", "evidence_path"}


def validate_receiver_capability(value):
    fields_with_optional(value, RECEIVER_FIELDS, ("startup_retry",), "receiver capability")
    for field in ("source_product_id", "vendor_id", "product_id"):
        usb_product(value[field], field)
    if type(value["report_bytes"]) is not int or value["report_bytes"] != 91:
        raise ValueError("Invalid receiver feature report length")
    if value["protocol"] != "razer_device25_wireless_status_v2" or value["command"] != [80, 0, 191]:
        raise ValueError("Unmapped receiver query protocol/header")
    protocol_bytes(value["command"], "receiver command", 3)
    validate_transport(value, receiver=True)
    boolean(value["peer_match_product_id"], "peer_match_product_id")
    rust_string(value["evidence_path"])
    if value.get("startup_retry") is not None:
        retry = value["startup_retry"]
        exact_fields(retry, ("first_peer_status", "delays_ms"), "receiver startup retry")
        bounded(retry["first_peer_status"], 0xFF, "peer status")
        for delay in source_list(retry["delays_ms"], "startup attempts", True):
            bounded(delay, (1 << 64) - 1, "startup delay")


def render_receiver_capability(value, indent):
    lines = [indent + "ReceiverCapability {"]
    for field in ("source_product_id", "vendor_id", "product_id", "claim_interface", "report_bytes", "report_id", "transaction_prefix",
                  "transaction_modulus", "max_retry_in", "max_retry_out", "sleep_between_out_ms", "sleep_between_out_in_ms", "sleep_between_in_ms"):
        lines.append(indent + f"    {field}: {value[field]},")
    lines.extend([indent + "    protocol: ReceiverProtocol::RazerDevice25WirelessStatusV2,",
                  indent + "    command: " + str(value["command"]) + ",",
                  indent + "    peer_match_product_id: " + boolean(value["peer_match_product_id"], "peer_match_product_id") + ","])
    for field in ("source_class", "evidence_path"):
        lines.append(indent + f"    {field}: " + rust_string(value[field]) + ".into(),")
    retry = value.get("startup_retry")
    if retry is not None:
        lines.extend([indent + "    startup_retry: Some(ReceiverStartupRetry {",
                      indent + f"        first_peer_status: {retry['first_peer_status']},",
                      indent + "        delays_ms: " + rust_vec(retry["delays_ms"]) + ",", indent + "    }),"])
    else:
        lines.append(indent + "    startup_retry: None,")
    lines.append(indent + "}")
    return lines


def validate_receiver_query_catalog(raw):
    catalog = json.loads(raw, object_pairs_hook=duplicate_free)
    exact_fields(catalog, ("schema_version", "scope", "capabilities"), "receiver query catalog")
    if type(catalog["schema_version"]) is not int or catalog["schema_version"] != 1:
        raise ValueError("Unsupported receiver query schema")
    rust_string(catalog["scope"])
    products = source_list(catalog["capabilities"], "receiver queries", True)
    seen = set()
    for capability in products:
        validate_receiver_capability(capability)
        if capability["product_id"] in seen:
            raise ValueError("Repeated receiver query source product")
        seen.add(capability["product_id"])
    return products


def render_receiver_query_catalog(raw):
    products = validate_receiver_query_catalog(raw)
    lines = generated_header(raw, "Fixed local receiver query transports.", [
        "use crate::receiver_capabilities::{ReceiverCapability, ReceiverProtocol, ReceiverStartupRetry};",
    ])
    lines.extend(["pub(crate) fn products() -> Vec<ReceiverCapability> {", "    vec!["])
    for capability in products:
        row = render_receiver_capability(capability, "        ")
        row[-1] += ","
        lines.extend(row)
    lines.extend(["    ]", "}", ""])
    return "\n".join(lines).encode("utf-8")


def validate_pairing_capabilities(raw):
    catalog = json.loads(raw, object_pairs_hook=duplicate_free)
    exact_fields(catalog, ("schema_version", "products"), "pairing capability catalog")
    if type(catalog["schema_version"]) is not int or catalog["schema_version"] != 1:
        raise ValueError("Unsupported pairing capability schema")
    products = source_list(catalog["products"], "pairing capabilities", True)
    seen = set()
    required = {"product_id", "category", "scan_pair_prefix", "unpair_prefix", "cancel_prefix", "metadata_prefix", "metadata_mouse_layout", "transport"}
    for capability in products:
        fields_with_optional(capability, required, ("metadata_claim_interface", "metadata_source_class"), "pairing capability")
        pid = usb_product(capability["product_id"])
        category = capability["category"]
        if category not in {"MOUSE", "KEYBOARD"} or (pid, category) in seen:
            raise ValueError("Unknown or repeated pairing product/category branch")
        seen.add((pid, category))
        validate_receiver_capability(capability["transport"])
        if pid != capability["transport"]["product_id"]:
            raise ValueError("Pairing owner differs from its transport")
        for field in ("scan_pair_prefix", "unpair_prefix", "cancel_prefix", "metadata_prefix"):
            namespace(capability[field], capability["transport"]["transaction_modulus"])
        if "metadata_claim_interface" in capability:
            bounded(capability["metadata_claim_interface"], 0xFF, "metadata interface")
        if "metadata_source_class" in capability and capability["metadata_source_class"] not in READ_CLASSES:
            raise ValueError("Unknown pairing metadata factory")
        boolean(capability["metadata_mouse_layout"], "metadata_mouse_layout")
    return products


def render_pairing_capabilities(raw):
    products = validate_pairing_capabilities(raw)
    lines = generated_header(raw, "Fixed local pairing command factories.", [
        "use crate::receiver_capabilities::{ReceiverCapability, ReceiverProtocol, ReceiverStartupRetry};",
        "use crate::receiver_pairing::PairingCapability;",
    ])
    lines.extend(["pub(crate) fn products() -> Vec<PairingCapability> {", "    vec!["])
    for capability in products:
        lines.extend(["        PairingCapability {", f"            product_id: {capability['product_id']},",
                      "            category: " + rust_string(capability["category"]) + ".into(),"])
        for field in ("scan_pair_prefix", "unpair_prefix", "cancel_prefix", "metadata_prefix"):
            lines.append(f"            {field}: {capability[field]},")
        lines.extend(["            metadata_claim_interface: " + option_expr(capability, "metadata_claim_interface") + ",",
                      "            metadata_source_class: " + option_expr(capability, "metadata_source_class", lambda value: rust_string(value) + ".into()") + ",",
                      "            metadata_mouse_layout: " + boolean(capability["metadata_mouse_layout"], "metadata_mouse_layout") + ","])
        transport = render_receiver_capability(capability["transport"], "            ")
        transport[0] = transport[0].replace("ReceiverCapability", "transport: ReceiverCapability", 1)
        transport[-1] += ","
        lines.extend(transport + ["        },"])
    lines.extend(["    ]", "}", ""])
    return "\n".join(lines).encode("utf-8")


def validate_pairing_source_catalog(raw):
    catalog = json.loads(raw, object_pairs_hook=duplicate_free)
    exact_fields(catalog, ("schema_version", "source_evidence", "source_scope", "masters", "devices", "available"), "pairing source catalog")
    if type(catalog["schema_version"]) is not int or catalog["schema_version"] != 1:
        raise ValueError("Unsupported pairing source schema")
    rust_string(catalog["source_scope"])
    if not isinstance(catalog["source_evidence"], dict):
        raise ValueError("Pairing source evidence receipt is absent")
    seen = set()
    for master in source_list(catalog["masters"], "pairing masters", True):
        fields_with_optional(master, ("productId", "dongleId", "bleId", "claimInterface", "canPairTwoDevices"), ("supports8KHzPollingRate",), "pairing master")
        pid = usb_product(master["productId"])
        if pid in seen:
            raise ValueError("Repeated pairing source master")
        seen.add(pid)
        usb_product(master["dongleId"])
        usb_product(master["bleId"], allow_zero=True)
        bounded(master["claimInterface"], 0xFF, "master claim interface")
        boolean(master["canPairTwoDevices"], "canPairTwoDevices")
        if "supports8KHzPollingRate" in master:
            boolean(master["supports8KHzPollingRate"], "supports8KHzPollingRate")
    seen = set()
    for device in source_list(catalog["devices"], "pairing devices", True):
        fields_with_optional(device, ("DeviceDonglePid", "ProductInfo", "DevicePairingBuddies", "DeviceType"), ("DeviceSiblings", "DeviceFamily"), "pairing device")
        pid = bounded(device["DeviceDonglePid"], 0xFFFE, "pairing radio", 1)
        if pid in seen:
            raise ValueError("Repeated pairing source radio")
        seen.add(pid)
        editions = set()
        for edition in source_list(device["ProductInfo"], "product editions"):
            exact_fields(edition, ("EID", "Name", "CHSName"), "product edition")
            eid = bounded(edition["EID"], 0xFFFF, "edition ID")
            if eid in editions:
                raise ValueError("Repeated pairing source edition")
            editions.add(eid)
            rust_string(edition["Name"])
            rust_string(edition["CHSName"])
        product_ids(device["DevicePairingBuddies"], "pairing buddies")
        if "DeviceSiblings" in device:
            product_ids(device["DeviceSiblings"], "device siblings")
        if device["DeviceType"] not in PAIRING_CATEGORIES:
            raise ValueError("Unknown pairing device category")
        if "DeviceFamily" in device:
            rust_string(device["DeviceFamily"])
    for product in source_list(catalog["available"], "available products", True):
        fields_with_optional(product, ("productId",), ("isNotChromaDevice", "dongleId", "bleId", "wiredId", "psModeIds", "monitorPortIds", "xBoxId"), "available pairing product")
        usb_product(product["productId"])
        if "isNotChromaDevice" in product:
            boolean(product["isNotChromaDevice"], "isNotChromaDevice")
        for field in ("dongleId", "bleId", "wiredId", "psModeIds", "monitorPortIds", "xBoxId"):
            if field in product:
                value = product[field]
                if isinstance(value, list):
                    product_ids(value, field, True)
                else:
                    usb_product(value, field)
    return catalog


def render_pairing_source_catalog(raw):
    catalog = validate_pairing_source_catalog(raw)
    lines = generated_header(raw, "Fixed pairing compatibility and presentation records.", [
        "use crate::receiver_pairing::{CatalogProductIds, PairingAvailableProduct, PairingCatalog, PairingCatalogDevice, PairingDeviceCategory, PairingMaster, PairingProductEdition};",
    ])
    lines.extend(["pub(crate) fn catalog() -> PairingCatalog {", "    PairingCatalog {", "        schema_version: 1,", "        masters: vec!["])
    for master in catalog["masters"]:
        lines.extend(["            PairingMaster {",
                      f"                product_id: {master['productId']}, dongle_id: {master['dongleId']}, ble_id: {master['bleId']}, claim_interface: {master['claimInterface']},",
                      "                supports_eight_khz: " + option_bool(master, "supports8KHzPollingRate") + ",",
                      "                can_pair_two_devices: " + boolean(master["canPairTwoDevices"], "canPairTwoDevices") + ",", "            },"])
    lines.extend(["        ],", "        devices: vec!["])
    for device in catalog["devices"]:
        lines.extend(["            PairingCatalogDevice {", f"                dongle_id: {device['DeviceDonglePid']},", "                editions: vec!["])
        for edition in device["ProductInfo"]:
            lines.append("                    PairingProductEdition { edition_id: " + str(edition["EID"]) + ", name: " + rust_string(edition["Name"]) + ".into(), chinese_name: " + rust_string(edition["CHSName"]) + ".into() },")
        lines.extend(["                ],", "                siblings: " + option_expr(device, "DeviceSiblings", rust_vec) + ",",
                      "                pairing_buddies: " + rust_vec(device["DevicePairingBuddies"]) + ",",
                      "                category: PairingDeviceCategory::" + PAIRING_CATEGORIES[device["DeviceType"]] + ",",
                      "                family: " + option_expr(device, "DeviceFamily", lambda family: rust_string(family) + ".into()") + ",", "            },"])
    lines.extend(["        ],", "        available: vec!["])
    for product in catalog["available"]:
        radio = option_expr(product, "dongleId", lambda ids: "CatalogProductIds::Many(" + rust_vec(ids) + ")" if isinstance(ids, list) else f"CatalogProductIds::One({ids})")
        lines.append("            PairingAvailableProduct { product_id: " + str(product["productId"]) + ", dongle_id: " + radio + " },")
    lines.extend(["        ],", "    }", "}", ""])
    return "\n".join(lines).encode("utf-8")


def prepare_local_protocol_tables(reads=READ_INPUT, identity=IDENTITY_INPUT, queries=QUERY_INPUT,
                                  pairing=PAIR_CAP_INPUT, catalog=PAIR_CATALOG_INPUT, check=False):
    import subprocess
    definitions = [(reads, READ_OUTPUT, render_read_catalog), (identity, IDENTITY_OUTPUT, render_identity_catalog),
                   (queries, QUERY_OUTPUT, render_receiver_query_catalog), (pairing, PAIR_CAP_OUTPUT, render_pairing_capabilities),
                   (catalog, PAIR_CATALOG_OUTPUT, render_pairing_source_catalog)]
    outputs = []
    for source, name, renderer in definitions:
        rendered = renderer(local_path(source).read_bytes())
        formatted = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
                                   input=rendered, capture_output=True, check=True)
        outputs.append((name, formatted.stdout))
    for name, raw in outputs:
        output = local_path(name)
        if check:
            if output.read_bytes() != raw:
                raise ValueError("Typed local protocol table differs from its maintenance input: " + name)
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(raw)
    return {"paths": [name for name, _ in outputs], "passed": True,
            "scope": "fixed local protocol data; no runtime JSON, network or device operations"}


def fixed_tables():
    return {
        ("razer-device", "DEVICE_WRITE_CAPABILITIES"): (WRITE_INPUT, WRITE_OUTPUT, render_write_catalog, False),
        ("razer-device", "DEVICE_READ_CAPABILITIES"): (READ_INPUT, READ_OUTPUT, render_read_catalog, True),
        ("razer-device", "DEVICE_IDENTITY_CATALOG"): (IDENTITY_INPUT, IDENTITY_OUTPUT, render_identity_catalog, True),
        ("razer-device", "RECEIVER_QUERY_CAPABILITIES"): (QUERY_INPUT, QUERY_OUTPUT, render_receiver_query_catalog, True),
        ("razer-device", "RECEIVER_PAIRING_CAPABILITIES"): (PAIR_CAP_INPUT, PAIR_CAP_OUTPUT, render_pairing_capabilities, True),
        ("razer-device", "RECEIVER_PAIRING_CATALOG"): (PAIR_CATALOG_INPUT, PAIR_CATALOG_OUTPUT, render_pairing_source_catalog, True),
        ("razer-storage", "RECEIVER_PAIRING_CATALOG"): (PAIRING_INPUT, PAIRING_OUTPUT, render_pairing_storage, False),
        ("razer-storage", "RECEIVER_RESET_DEFAULTS"): (RESET_INPUT, RESET_OUTPUT, render_reset_storage, False),
    }


def regenerate_input(path, *, root=None):
    import subprocess
    root = (root or ROOT).resolve()
    for source, output, renderer, formatted in fixed_tables().values():
        if (root / source).resolve() != Path(path).resolve():
            continue
        content = renderer(Path(path).read_bytes())
        if formatted:
            content = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
                                     input=content, capture_output=True, check=True).stdout
        target = root / output
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content)


def add_commands(commands):
    p = commands.add_parser("prepare-local-protocol-tables", help="Generate typed local read, identity and receiver tables")
    p.add_argument("--reads-json", default=READ_INPUT)
    p.add_argument("--identity-json", default=IDENTITY_INPUT)
    p.add_argument("--queries-json", default=QUERY_INPUT)
    p.add_argument("--pairing-json", default=PAIR_CAP_INPUT)
    p.add_argument("--catalog-json", default=PAIR_CATALOG_INPUT)
    p.add_argument("--check", action="store_true")
    p = commands.add_parser("prepare-device-write-table", help="Generate typed Rust capabilities from explicit maintenance JSON")
    p.add_argument("--from-json", default=WRITE_INPUT, help="Source-derived maintenance input; never read by Cargo or runtime")
    p.add_argument("--check", action="store_true", help="Verify generated Rust without rewriting")
    p = commands.add_parser("prepare-storage-tables", help="Generate typed local receiver defaults and catalog projection")
    p.add_argument("--pairing-json", default=PAIRING_INPUT)
    p.add_argument("--reset-json", default=RESET_INPUT)
    p.add_argument("--check", action="store_true")


def run(args):
    if args.command == "prepare-local-protocol-tables":
        return prepare_local_protocol_tables(args.reads_json, args.identity_json, args.queries_json,
                                              args.pairing_json, args.catalog_json, args.check)
    if args.command == "prepare-device-write-table":
        return prepare_write_table(args.from_json, args.check)
    if args.command == "prepare-storage-tables":
        return prepare_receiver_storage(args.pairing_json, args.reset_json, args.check)
    raise ValueError("Unknown typed-table command")

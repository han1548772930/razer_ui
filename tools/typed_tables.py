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
MAPPING_OUTPUT = "crates/razer-model/src/mapping_requirements/mouse_capabilities.rs"
TRANSFER_OUTPUT = "crates/razer-device/src/profile_transfer/targets.rs"
BATTERY_PEER_OUTPUT = "crates/razer-device/src/receiver/receiver_battery_peers_generated.rs"
BATTERY_PEER_PRODUCTS = (214, 221, 226, 231, 235, 45066)
BATTERY_PEER_FACTORIES = {name: name[0].upper() + name[1:] for name in (
    "rzDevice25", "rzDevice25DualLinkMouse", "rzDevice30", "rzDevice25Krakoff")}
BATTERY_PEER_GEOMETRY_FIELDS = {
    "Version": "version", "VID": "vendor_id", "PID": "product_id", "EID": "edition",
    "Layout": "layout", "MatrixMaxRow": "matrix_max_row", "MatrixMaxCol": "matrix_max_col",
    "DeviceMaxRow": "device_max_row", "DeviceMaxCol": "device_max_col", "LedWidth": "led_width",
    "LedHeight": "led_height", "LedWidthOffset": "led_width_offset", "LedHeightOffset": "led_height_offset",
    "MaxRegion": "max_region", "RegionMaxRow": "region_max_row", "RegionMaxCol": "region_max_col",
}
TRANSFER_CATEGORIES = {
    "ACCESSORY": "Accessory", "AUDIO": "Audio", "BROADCASTER": "Broadcaster",
    "CASE": "Case", "CHAIR": "Chair", "EGPU": "Egpu", "GAMEPAD": "Gamepad",
    "HUE": "Hue", "IOT": "Iot", "KEYBOARD": "Keyboard", "KEYPAD": "Keypad",
    "MONITOR": "Monitor", "MOUSE": "Mouse", "MOUSEMAT": "Mousemat",
    "MOUSEPLUSMAT": "Mouseplusmat", "SYSTEM": "System",
}
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


def check_receiver_reset_storage():
    source, output = local_path(RESET_INPUT), local_path(RESET_OUTPUT)
    raw = source.read_bytes()
    generated = render_reset_storage(raw)
    if output.read_bytes() != generated:
        raise ValueError("Typed receiver reset storage differs from complete source defaults")
    return {"source": RESET_INPUT, "source_sha256": hashlib.sha256(raw).hexdigest(),
            "output": RESET_OUTPUT, "output_sha256": hashlib.sha256(generated).hexdigest(),
            "products": [row["product_id"] for row in json.loads(raw)["products"]],
            "complete_generated_bytes_equal": True, "passed": True}



READ_INPUT = ".analysis/inputs/typed-tables/razer-device/device-read-capabilities.json"
IDENTITY_AVAILABLE_URL = "https://apps.razer.com/synapse/dashboard/AvailableDevices.json"
IDENTITY_DEVELOPMENT_URL = "https://apps.razer.com/synapse/dashboard/inDevelopmentDevices.json"
IDENTITY_HOST_URL = "https://apps.razer.com/background-manager/"
QUERY_INPUT = ".analysis/inputs/typed-tables/razer-device/receiver-query-capabilities.json"
PAIR_CAP_INPUT = ".analysis/inputs/typed-tables/razer-device/receiver-pairing-capabilities.json"
RECEIVER_179_PAIRING_NAMESPACES = {
    "MOUSE": {"scan_pair_prefix": 0, "unpair_prefix": 128, "cancel_prefix": 0, "metadata_prefix": 0},
    "KEYBOARD": {"scan_pair_prefix": 128, "unpair_prefix": 0, "cancel_prefix": 0, "metadata_prefix": 128},
}
PAIR_CATALOG_INPUT = ".analysis/inputs/typed-tables/razer-device/receiver-pairing-catalog.json"
READ_OUTPUT = "crates/razer-device/src/transport/device_read_capabilities.rs"
ONBOARD_KEY_OUTPUT = "crates/razer-device/src/keyboard/keyboard_onboard_inventory.rs"
ONBOARD_MAPPING_OUTPUT = "crates/razer-device/src/keyboard/keyboard_onboard_mapping_source.rs"
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
IDENTITY_CATEGORIES = TRANSFER_CATEGORIES
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


IDENTITY_CATALOG_FIELDS = {
    "productId": "product_id", "repId": "rep_id", "dongleId": "dongle_id", "bleId": "ble_id",
    "xBoxId": "xbox_id", "wiredId": "wired_id", "psModeIds": "ps_mode_ids",
    "monitorPortIds": "monitor_port_ids", "isNotChromaDevice": "is_not_chroma_device",
}
IDENTITY_DEFINITION_FIELDS = {"productId", "dongleId", "bleId", "vendorId", "category", "claimInterface"}


def validate_identity_catalog(raw, *, allow_empty=False):
    """Decode the actual ordered host JSON; repeated rows remain distinct."""
    products = source_list(json.loads(raw, object_pairs_hook=duplicate_free), "identity catalog", not allow_empty)
    result = []
    for product in products:
        fields_with_optional(product, ("productId",), IDENTITY_CATALOG_FIELDS.keys() - {"productId"}, "host identity row")
        usb_product(product["productId"])
        if "repId" in product:
            usb_product(product["repId"], allow_zero=True)
        for field in ("dongleId", "bleId", "xBoxId", "wiredId"):
            if field in product:
                value = product[field]
                if isinstance(value, list):
                    product_ids(value, field, True)
                else:
                    usb_product(value, field)
        for field in ("psModeIds", "monitorPortIds"):
            if field in product:
                product_ids(product[field], field, True)
        if "isNotChromaDevice" in product:
            boolean(product["isNotChromaDevice"], "isNotChromaDevice")
        result.append({IDENTITY_CATALOG_FIELDS[key]: value for key, value in product.items()})
    return result


def validate_identity_definition(projection):
    fields_with_optional(projection, {"productId", "category"},
                         IDENTITY_DEFINITION_FIELDS - {"productId", "category"}, "DeviceInfo identity")
    for field in ("productId", "vendorId", "dongleId", "bleId"):
        if field in projection:
            usb_product(projection[field], field, allow_zero=field in {"dongleId", "bleId"})
    if not isinstance(projection["category"], str) or projection["category"] not in IDENTITY_CATEGORIES:
        raise ValueError("Unknown DeviceInfo identity category")
    if "claimInterface" in projection:
        value = projection["claimInterface"]
        if isinstance(value, dict):
            if not value:
                raise ValueError("DeviceInfo claimInterface has no physical product branches")
            for physical, interface in value.items():
                if not isinstance(physical, str) or not re.fullmatch(r"[1-9][0-9]*", physical):
                    raise ValueError("DeviceInfo claimInterface has an invalid physical product key")
                usb_product(int(physical), "claimInterface physical product")
                bounded(interface, 0xFF, "claimInterface physical interface")
        else:
            bounded(value, 0xFF, "claimInterface uniform interface")
    return projection


def identity_definition_projection(body, *, modules=None):
    projection, evidence = _profile_transfer_static_fields(
        body, "DeviceInfo", IDENTITY_DEFINITION_FIELDS, modules or {})
    return validate_identity_definition(projection), evidence


def identity_definition_semantics(projection):
    if projection is None:
        return None
    validate_identity_definition(projection)
    interface = projection.get("claimInterface")
    return {
        "identity": {"product_id": projection["productId"], "dongle_id": projection.get("dongleId"),
                     "ble_id": projection.get("bleId"), "vendor_id": projection.get("vendorId")},
        "category": IDENTITY_CATEGORIES[projection["category"]],
        "claim_interface": ({"kind": "ByProduct", "value": [
            {"physical_product_id": int(pid), "interface": number} for pid, number in interface.items()]}
            if isinstance(interface, dict) else {"kind": "Uniform", "value": interface} if interface is not None else None),
    }


def identity_catalog_semantics(products):
    return [{field: product.get(field, False if field == "is_not_chroma_device" else None)
             for field in IDENTITY_CATALOG_FIELDS.values()} for product in products]


def render_identity_catalog(products, definitions, development_products):
    for row in source_list(definitions, "source identity definitions", True):
        fields_with_optional(row, {"mounted_product_id", "ui", "middleware"}, {"sources"}, "source identity definitions")
        usb_product(row["mounted_product_id"], "mounted identity product")
    source_list(development_products, "in-development identity products")
    mounted = [row["mounted_product_id"] for row in definitions]
    if not products or mounted != sorted(set(mounted)) or not {row["product_id"] for row in products} <= set(mounted):
        raise ValueError("Current identity definitions must cover the catalog once in mounted product order")
    reverse_fields = {value: key for key, value in IDENTITY_CATALOG_FIELDS.items()}
    validate_identity_catalog(json.dumps([{reverse_fields[key]: value for key, value in row.items()} for row in products]))
    for product_id in development_products:
        usb_product(product_id, "in-development identity")
    lines = [
        "//! Current host catalog rows and independently declared UI/MW identities.",
        "//! Regenerate with workspace.py prepare-local-protocol-tables --current-identities.",
        "use crate::device_identity::{", "    CatalogProduct, ClaimInterface, DeviceCategory, DeviceInfoDefinition, Ids,",
        "    PhysicalInterface, ProductDefinitions, SourceProductIdentity,", "};", "",
        "pub(crate) static PRODUCTS: &[CatalogProduct] = &[",
    ]
    for product in products:
        lines.extend(["    CatalogProduct {", f"        product_id: {product['product_id']},",
                      "        rep_id: " + option_expr(product, "rep_id") + ","])
        for field in ("dongle_id", "ble_id", "xbox_id", "wired_id"):
            lines.append(f"        {field}: " + option_expr(product, field, lambda ids:
                "Ids::Many(&" + str(ids) + ")" if isinstance(ids, list) else f"Ids::One({ids})") + ",")
        for field in ("ps_mode_ids", "monitor_port_ids"):
            lines.append(f"        {field}: " + option_expr(product, field, lambda ids: "&" + str(ids)) + ",")
        lines.extend(["        is_not_chroma_device: " + boolean(product.get("is_not_chroma_device", False), "is_not_chroma_device") + ",",
                      "    },"])
    lines.extend(["];", "", "pub(crate) static IN_DEVELOPMENT: &[u32] = &" + str(development_products) + ";", "",
                  "pub(crate) static DEFINITIONS: &[ProductDefinitions] = &["])
    for row in definitions:
        lines.extend(["    ProductDefinitions {", f"        mounted_product_id: {row['mounted_product_id']},"])
        for role in ("ui", "middleware"):
            value = identity_definition_semantics(row[role])
            if value is None:
                lines.append(f"        {role}: None,")
                continue
            lines.extend([f"        {role}: Some(DeviceInfoDefinition {{", "            identity: SourceProductIdentity {"])
            lines.append(f"                product_id: {value['identity']['product_id']},")
            for field in ("dongle_id", "ble_id", "vendor_id"):
                item = value["identity"][field]
                lines.append(f"                {field}: " + ("None" if item is None else f"Some({item})") + ",")
            lines.extend(["            },", f"            category: DeviceCategory::{value['category']},"])
            interface = value["claim_interface"]
            if interface is None:
                lines.append("            claim_interface: None,")
            elif interface["kind"] == "Uniform":
                lines.append(f"            claim_interface: Some(ClaimInterface::Uniform({interface['value']})),")
            else:
                lines.append("            claim_interface: Some(ClaimInterface::ByProduct(&[")
                for branch in interface["value"]:
                    lines.append("                PhysicalInterface { physical_product_id: " +
                        str(branch["physical_product_id"]) + ", interface: " + str(branch["interface"]) + " },")
                lines.append("            ])),")
            lines.append("        }),")
        lines.append("    },")
    lines.extend(["];", ""])
    return "\n".join(lines).encode("utf-8")


def verify_identity_constructor(content, products, definitions, development_products):
    """Read every fixed Rust field, source row, duplicate and interface branch back."""
    tree = receiver_table_tree(content)
    structs = {"CatalogProduct": set(IDENTITY_CATALOG_FIELDS.values()),
               "ProductDefinitions": {"mounted_product_id", "ui", "middleware"},
               "DeviceInfoDefinition": {"identity", "category", "claim_interface"},
               "SourceProductIdentity": {"product_id", "dongle_id", "ble_id", "vendor_id"},
               "PhysicalInterface": {"physical_product_id", "interface"}}

    def decode(node):
        if node.type == "reference_expression" and len(node.named_children) == 1:
            return decode(node.named_children[0])
        if node.type == "array_expression":
            return [decode(child) for child in node.named_children if child.type not in {"line_comment", "block_comment"}]
        if node.type in {"integer_literal", "boolean_literal"}:
            return json.loads(node.text)
        if node.type == "identifier" and node.text == b"None":
            return None
        if node.type == "scoped_identifier" and node.text.startswith(b"DeviceCategory::"):
            category = node.text.decode().removeprefix("DeviceCategory::")
            if category in IDENTITY_CATEGORIES.values():
                return category
        if node.type == "call_expression":
            function = node.child_by_field_name("function").text.decode()
            args = node.child_by_field_name("arguments").named_children
            if len(args) == 1 and function in {"Some", "Ids::One", "Ids::Many"}:
                value = decode(args[0])
                if function == "Ids::One" and type(value) is not int or function == "Ids::Many" and not isinstance(value, list):
                    raise ValueError("Generated identity lost a scalar/array branch")
                return value
            if len(args) == 1 and function in {"ClaimInterface::Uniform", "ClaimInterface::ByProduct"}:
                return {"kind": function.split("::")[1], "value": decode(args[0])}
        if node.type == "struct_expression":
            name = node.child_by_field_name("name").text.decode()
            if name not in structs:
                raise ValueError("Generated identity has an unrecognized constructor")
            values = {}
            for child in node.child_by_field_name("body").named_children:
                if child.type in {"line_comment", "block_comment"}:
                    continue
                if child.type != "field_initializer":
                    raise ValueError("Generated identity fields must be explicit")
                field = child.child_by_field_name("field").text.decode()
                if field in values:
                    raise ValueError("Generated identity repeats a field")
                values[field] = decode(child.child_by_field_name("value"))
            exact_fields(values, structs[name], name)
            return values
        raise ValueError("Generated identity has a nonliteral value: " + node.type)

    actual = {}
    for node in tree.root_node.named_children:
        if node.type == "static_item":
            name = node.child_by_field_name("name").text.decode()
            if name not in {"PRODUCTS", "DEFINITIONS", "IN_DEVELOPMENT"} or name in actual:
                raise ValueError("Generated identity has an unknown or repeated fixed table")
            actual[name] = decode(node.child_by_field_name("value"))
        elif node.type not in {"use_declaration", "line_comment", "block_comment"}:
            raise ValueError("Generated identity has an unverified top-level owner")
    expected = {"PRODUCTS": identity_catalog_semantics(products), "IN_DEVELOPMENT": development_products,
                "DEFINITIONS": [{"mounted_product_id": row["mounted_product_id"],
                                 **{role: identity_definition_semantics(row[role]) for role in ("ui", "middleware")}}
                                for row in definitions]}
    if json.dumps(actual, sort_keys=True) != json.dumps(expected, sort_keys=True):
        raise ValueError("Generated identity fields, row order, duplicates or physical branches differ from current source")
    return {"catalog_rows": len(products), "mounted_products": len(definitions),
            "all_fields_order_duplicates_and_physical_branches_equal": True}


def identity_discovery_contract(raw):
    """Verify the host's catalog projections in their complete owning functions."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    parser = Parser(Language(tree_sitter_javascript.language()))
    tree = parser.parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Current identity host has a JavaScript AST error")

    def walk(node):
        yield node
        for child in node.named_children:
            yield from walk(child)

    def one(rows, name):
        values = list(rows)
        if len(values) != 1:
            raise ValueError("Current identity host has no unique " + name)
        return values[0]

    def signature(node):
        names = {}

        def visit(value):
            if value.type == "parenthesized_expression":
                return visit(one(value.named_children, "parenthesized expression"))
            if value.type == "identifier":
                return (value.type, names.setdefault(value.text, len(names)))
            return (value.type, tuple(visit(child) for child in value.children if child.type != "comment")
                    if value.children else value.text)
        return visit(node)

    def expected(expression):
        parsed = parser.parse(("const identity_projection=" + expression + ";").encode())
        if parsed.root_node.has_error:
            raise ValueError("Identity contract expression has an AST error")
        return signature(parsed.root_node.named_children[0].named_children[0].child_by_field_name("value"))

    def member(node, object_name, property_name):
        return (node is not None and node.type == "member_expression"
                and node.child_by_field_name("object").text == object_name
                and node.child_by_field_name("property").text == property_name)

    fields = {}
    all_nodes = list(walk(tree.root_node))
    for name in ("getLatestDeviceData", "_checkUSBDetail"):
        field = one((node for node in all_nodes if node.type == "field_definition"
                     and node.child_by_field_name("property").text.decode() == name), name)
        fields[name] = field
    loader = fields["getLatestDeviceData"]
    checker = fields["_checkUSBDetail"]
    checker_nodes = list(walk(checker))
    projections = {
        "host_candidates": "this.available.filter(e=>e.productId===a||HD(e.dongleId,a)||HD(e.bleId,a)||e.repId===a||HD(e.xBoxId,a)||HD(e.wiredId,a)||e.psModeIds?.includes(a))",
        "monitor_first": "this.available.find(t=>t.monitorPortIds&&t.monitorPortIds.includes(e.productId))",
        "single_route": "r.productId=t[0].repId||t[0].productId",
        "single_dongle": "r.isDongle=HD(t[0].dongleId,r.realProductId)",
        "single_ble": "r.isBle=HD(t[0].bleId,r.realProductId)",
        "single_xbox": "r.isXBox=HD(t[0].xBoxId,r.realProductId)",
        "single_playstation": "r.isPlaystation=t[0].psModeIds?.includes(r.realProductId)",
        "multiple_identity": '{productId:a||t,deviceContainerId:n,type:"available",realProductId:e.productId,baseProductName:i||e.productName,isDongle:!0}',
    }
    selected = {}
    for name, expression in projections.items():
        required = expected(expression)
        selected[name] = one((node for node in checker_nodes
                              if node.type != "parenthesized_expression" and signature(node) == required), name)
    filters = [node for node in checker_nodes if node.type == "call_expression"
               and member(node.child_by_field_name("function"), b"this.available", b"filter")]
    if filters != [selected["host_candidates"]]:
        raise ValueError("Current host has an additional catalog candidate filter")
    callback = selected["host_candidates"].child_by_field_name("arguments").named_children[0]
    helpers = {node.child_by_field_name("function").text for node in walk(callback)
               if node.type == "call_expression" and node.child_by_field_name("function").type == "identifier"}
    helper_name = one(helpers, "scalar/array alias matcher")
    helper = one((node for node in all_nodes if node.type == "function_declaration"
                  and node.child_by_field_name("name").text == helper_name), "alias matcher owner")
    helper_body = helper.child_by_field_name("body")
    returned = one((node for node in helper_body.named_children if node.type == "return_statement"), "alias result")
    if signature(one(returned.named_children, "alias return expression")) != expected("null!=e&&(Array.isArray(e)?e.includes(t):e===t)"):
        raise ValueError("Current host alias matching no longer has strict scalar/array semantics")
    assignments = list(walk(loader))
    available = one((node for node in assignments if node.type == "assignment_expression"
                     and member(node.child_by_field_name("left"), b"this", b"available")), "available assignment")
    if signature(available.child_by_field_name("right")) != expected("a.filter(e=>!c.find(t=>t.productId===e.productId))"):
        raise ValueError("Current host development exclusion differs")
    filtered = available.child_by_field_name("right")
    available_name = filtered.child_by_field_name("function").child_by_field_name("object").text
    development_find = one((node for node in walk(filtered) if node.type == "call_expression"
                            and node.child_by_field_name("function").type == "member_expression"
                            and node.child_by_field_name("function").child_by_field_name("property").text == b"find"), "development lookup")
    development_name = development_find.child_by_field_name("function").child_by_field_name("object").text
    results = one((node for node in assignments if node.type == "variable_declarator"
                   and node.child_by_field_name("name").type == "array_pattern"), "ordered catalog fetch results")
    fetched = results.child_by_field_name("name").named_children
    if len(fetched) != 7 or fetched[0].text != available_name or fetched[-1].text != development_name:
        raise ValueError("Host available/development directories no longer use their declared fetch positions")
    value_call = one((node for node in assignments if node.type == "call_expression"
                     and member(node.child_by_field_name("function"), b"Object", b"values")), "ordered directory source")
    directory_name = one(value_call.child_by_field_name("arguments").named_children, "directory table").text
    directory = one((node for node in all_nodes if node.type == "variable_declarator"
                     and node.child_by_field_name("name").text == directory_name), "directory table owner")
    directory_expression = '{AVAILABLE_DEVICE:`${window.location.origin}/synapse/dashboard/AvailableDevices.json`,WINDOW_10_DEVICE:`${window.location.origin}/synapse/dashboard/Windows10Devices.json`,WINDOW_8_DEVICE:null,SYNAPSE_2_DEVICE:`${window.location.origin}/synapse/dashboard/Synapse2Devices.json`,XBOX_HEADSETS:`${window.location.origin}/synapse/dashboard/XboxHeadsets.json`,XBOX_CONTROLLERS:`${window.location.origin}/synapse/dashboard/XboxDevices.json`,IN_DEVELOPMENT_DEVICES:`${window.location.origin}/synapse/dashboard/inDevelopmentDevices.json`}'
    if signature(directory.child_by_field_name("value")) != expected(directory_expression):
        raise ValueError("Current host directory fields or ordering differ")

    def capture(node):
        return {"byte_start": node.start_byte, "byte_end": node.end_byte,
                "body_sha256": hashlib.sha256(node.text).hexdigest(),
                "complete_body_retained_in_original": True}
    return {"owners": {**{key: capture(value) for key, value in fields.items()},
                       "alias_matcher": capture(helper), "directory_table": capture(directory)},
            "projections": {key: capture(value) for key, value in selected.items()},
            "catalog_order_and_duplicates_retained": True, "middleware_aliases_are_not_host_candidates": True,
            "monitor_first_match": True, "development_exclusion_uses_primary_id": True,
            "dynamic_aio_additions": "Separate local observations; not folded into the installed fixed table"}




def _capture_identity_product(item):
    import frontend
    pid, routes, observations = item
    definition = {"mounted_product_id": pid, "sources": {}}
    inputs = []
    for endpoint_role, role in (("ui", "ui"), ("mw", "middleware")):
        route = f"https://apps.razer.com/synapse/products/{pid}/{endpoint_role}/"
        source = frontend.current_product_source(
            pid, endpoint_role, candidates=routes.get(route, []), observations=observations)
        audit = source.pop("audit")
        source.pop("acquired")
        if source["status"] == "official_source_unavailable":
            definition[role] = None
            definition["sources"][role] = source
            inputs.extend(source["inputs"])
            continue
        source.pop("files")
        entry = source["entry"]
        manifest, main = source["manifest"], source["main"]
        owners, modules = profile_transfer_source_owners(audit.read(main))
        if len(owners) != 1:
            raise ValueError(f"Current identity PID {pid} {role} has no unique complete DeviceInfo owner")
        projection, evidence = identity_definition_projection(owners[0]["body"], modules=modules)
        definition[role] = projection
        definition["sources"][role] = {"status": "verified", "acquisition": source["acquisition"],
            "selection": source["selection"], "freshness": source["freshness"],
            "entry": {key: entry[key] for key in ("path", "bytes", "sha256")},
            "manifest": {key: manifest[key] for key in ("path", "bytes", "sha256")},
            "main": {key: main[key] for key in ("path", "bytes", "sha256")},
            "complete_owner": {key: value for key, value in owners[0].items() if key != "body"},
            "projection_evidence": evidence}
        inputs.extend(audit.artifacts.values())
    return definition, inputs


def prepare_current_identity_table(*, check=False, workers=4):
    import concurrent.futures
    import subprocess
    import sys
    import analysis
    import families
    import frontend
    import host
    from importlib.metadata import version
    if type(workers) is not int or not 2 <= workers <= 16:
        raise ValueError("Current identity workers must be between two and sixteen")
    inputs = [families.identity(local_path(name)) for name in (
        "tools/typed_tables.py", "tools/frontend.py", "tools/families.py", "tools/evidence.py",
        "tools/host.py", "tools/workspace.py", "tools/analysis.py")]
    routes = frontend.current_acquisition_routes()
    audit = host.HostLocalAudit()
    observations = frontend.latest_deployment_observations(audit)
    inputs.extend(audit.artifacts.values())
    directories = {}
    for role, url in (("available", IDENTITY_AVAILABLE_URL), ("development", IDENTITY_DEVELOPMENT_URL)):
        source = frontend.current_deployment_source(url, candidates=routes.get(url, []), observations=observations)
        source_audit = source.pop("audit")
        if source["status"] != "verified":
            raise ValueError("Current identity directory is unavailable: " + url)
        acquired = source.pop("acquired")
        source.pop("files")
        source["freshness"] = frontend.verify_latest_deployment_observations(
            acquired, [source["entry"]], source_audit, observations=observations)
        source["products"] = validate_identity_catalog(source_audit.read(source["entry"]), allow_empty=role == "development")
        directories[role] = source
        inputs.extend(source_audit.artifacts.values())
    host_source = frontend.current_deployment_source(
        IDENTITY_HOST_URL, candidates=routes.get(IDENTITY_HOST_URL, []), observations=observations)
    host_audit = host_source.pop("audit")
    if host_source["status"] != "verified":
        raise ValueError("Current identity host source is unavailable")
    host_acquired = host_source.pop("acquired")
    host_files = host_source.pop("files")
    host_mains = [row for row in host_source["selection"]["deployment"]["sources"]
                  if row["identity_verified"] and row["deployment_role"] == "html_include"
                  and re.fullmatch(re.escape(IDENTITY_HOST_URL) + r"assets/index-[\w]+\.js", row["url"])]
    if len(host_mains) != 1:
        raise ValueError("Current identity host has no unique mounted main")
    host_main = host_files[host_mains[0]["url"]]
    host_source["freshness"] = frontend.verify_latest_deployment_observations(
        host_acquired, [host_source["entry"], host_main], host_audit, observations=observations)
    host_source["contract"] = identity_discovery_contract(host_audit.read(host_main))
    host_source["main"] = host_main
    inputs.extend(host_audit.artifacts.values())
    products = directories["available"]["products"]
    development = [row["product_id"] for row in directories["development"]["products"]]
    mounted = {row["product_id"] for row in products}
    for route in routes:
        match = re.fullmatch(r"https://apps\.razer\.com/synapse/products/([1-9][0-9]*)/(ui|mw)/", route)
        if match:
            mounted.add(usb_product(int(match[1]), "mounted identity endpoint"))
    completed, failures = {}, []
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {pool.submit(_capture_identity_product, (pid, routes, observations)): pid for pid in sorted(mounted)}
        for count, future in enumerate(concurrent.futures.as_completed(futures), 1):
            pid = futures[future]
            try:
                definition, original_inputs = future.result()
                completed[pid] = definition
                inputs.extend(original_inputs)
            except (ValueError, OSError, KeyError, TypeError) as error:
                failures.append({"mounted_product_id": pid, "error": str(error)})
            if count % 32 == 0 or count == len(futures):
                print(f"Current identity sources: {count}/{len(futures)}; failed: {len(failures)}", file=sys.stderr, flush=True)
    definitions = [completed[pid] for pid in sorted(completed)]
    distinct_inputs = {}
    for row in inputs:
        if row["path"] in distinct_inputs and row != distinct_inputs[row["path"]]:
            raise ValueError("Identity generation observed conflicting input bytes: " + row["path"])
        distinct_inputs[row["path"]] = row
    inputs = list(distinct_inputs.values())
    report = {"directories": directories, "host": host_source, "definitions": definitions, "inputs": inputs,
              "parsers": {name: version(name) for name in ("tree-sitter", "tree-sitter-javascript", "tree-sitter-rust")},
              "failures": sorted(failures, key=lambda row: row["mounted_product_id"]),
              "catalog_rows": len(products), "catalog_primary_products": len({row["product_id"] for row in products}),
              "mounted_products": len(mounted), "check_only": check,
              "catalog_repeated_primaries": [{"product_id": pid,
                  "row_indices": [index for index, row in enumerate(products) if row["product_id"] == pid]}
                  for pid in sorted({row["product_id"] for row in products})
                  if sum(row["product_id"] == pid for row in products) > 1],
              "declared_endpoint_aliases": [{"mounted_product_id": row["mounted_product_id"], "role": role,
                  "declared_product_id": row[role]["productId"]}
                  for row in definitions for role in ("ui", "middleware")
                  if row[role] is not None and row[role]["productId"] != row["mounted_product_id"]],
              "physical_interface_branch_products": {role: [row["mounted_product_id"] for row in definitions
                  if row[role] is not None and isinstance(row[role].get("claimInterface"), dict)]
                  for role in ("ui", "middleware")},
              "unavailable_sources": [{"mounted_product_id": row["mounted_product_id"], "role": role, "source": row["sources"][role]}
                                      for row in definitions for role in ("ui", "middleware") if row[role] is None],
              "all_product_sources_complete": not failures and all(row[role] is not None for row in definitions for role in ("ui", "middleware")),
              "application_execution": False, "vendor_execution": False, "runtime_acceptance": "unverified"}
    if not failures:
        rendered = render_identity_catalog(products, definitions, development)
        rendered = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
                                  input=rendered, capture_output=True, check=True).stdout
        report["constructor_equivalence"] = verify_identity_constructor(rendered, products, definitions, development)
        if not all(families.valid(row) for row in inputs):
            raise ValueError("Current identity inputs changed during generation")
        output = local_path(IDENTITY_OUTPUT)
        if check:
            if not output.is_file() or output.read_bytes() != rendered:
                raise ValueError("Typed current identities differ; regenerate through workspace.py")
            verify_identity_constructor(output.read_bytes(), products, definitions, development)
        else:
            import os
            import tempfile
            output.parent.mkdir(parents=True, exist_ok=True)
            temporary = None
            try:
                with tempfile.NamedTemporaryFile(dir=output.parent, prefix=".identity-", suffix=".rs", delete=False) as stream:
                    temporary = Path(stream.name)
                    stream.write(rendered)
                    stream.flush()
                    os.fsync(stream.fileno())
                verify_identity_constructor(temporary.read_bytes(), products, definitions, development)
                os.replace(temporary, output)
            finally:
                if temporary is not None and temporary.exists():
                    temporary.unlink()
        report.update(output=families.identity(output), inputs_unchanged=True, published=not check,
                      fixed_table_verified=True, publication_status="verified_existing" if check else "published")
    else:
        report["published"] = False
    destination = ROOT / ".analysis/results/typed-data.json"
    with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
        previous["current_device_identities"] = report
        analysis.save(destination, previous)
    if failures:
        raise ValueError("Current identity source chains failed: " + json.dumps(report["failures"]))
    return {"output": IDENTITY_OUTPUT, "catalog_rows": len(products), "mounted_products": len(definitions),
            "unavailable_sources": len(report["unavailable_sources"]),
            "all_product_sources_complete": report["all_product_sources_complete"], "passed": True, "check_only": check}


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
    lines.append(indent + "    source_class: " + rust_string(value["source_class"]) + ".into(),")
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
        if pid == 179 and any(capability[field] != value
                              for field, value in RECEIVER_179_PAIRING_NAMESPACES[category].items()):
            raise ValueError("PID179 pairing namespaces differ from the current category factories and primary Cancel")
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


def prepare_local_protocol_tables(reads=READ_INPUT, queries=QUERY_INPUT,
                                  pairing=PAIR_CAP_INPUT, catalog=PAIR_CATALOG_INPUT, check=False,
                                  current_reads=False, current_identities=False, workers=4):
    import subprocess
    import concurrent.futures
    if type(workers) is not int or not 2 <= workers <= 16:
        raise ValueError("Local protocol table workers must be between two and sixteen")
    if current_reads and current_identities:
        raise ValueError("Select one current protocol table projection")
    if current_reads:
        return prepare_current_read_table(check=check)
    if current_identities:
        return prepare_current_identity_table(check=check, workers=workers)
    definitions = [(reads, READ_OUTPUT, render_read_catalog),
                   (queries, QUERY_OUTPUT, render_receiver_query_catalog), (pairing, PAIR_CAP_OUTPUT, render_pairing_capabilities),
                   (catalog, PAIR_CATALOG_OUTPUT, render_pairing_source_catalog)]

    def render_one(item):
        source, name, renderer = item
        rendered = renderer(local_path(source).read_bytes())
        if name == READ_OUTPUT:
            rendered, _ = current_read_table_projection(rendered, current_read_sources()[0], update=True)
        formatted = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
                                   input=rendered, capture_output=True, check=True)
        return name, formatted.stdout

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(2, min(workers, len(definitions)))) as pool:
        outputs = list(pool.map(render_one, definitions))
    identities = prepare_current_identity_table(check=check, workers=workers)
    for name, raw in outputs:
        output = local_path(name)
        if check:
            if output.read_bytes() != raw:
                raise ValueError("Typed local protocol table differs from its maintenance input: " + name)
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(raw)
    return {"paths": [name for name, _ in outputs] + [IDENTITY_OUTPUT], "identities": identities, "passed": True,
            "scope": "fixed local protocol data; no runtime JSON, network or device operations"}


def current_read_decoder_contract(method, body, *, decoder_only=False):
    """Check the payload and observed fields against the current typed decoder."""
    import sys
    sys.path.insert(0, str(Path(__file__).resolve().parent / "analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    tree = Parser(Language(tree_sitter_javascript.language())).parse(body.encode())
    if tree.root_node.has_error:
        raise ValueError("Current observation helper has a JavaScript AST error")

    def walk(node):
        yield node
        for child in node.named_children:
            yield from walk(child)

    def one(nodes, label):
        rows = list(nodes)
        if len(rows) != 1:
            raise ValueError("Current observation has absent or ambiguous " + label)
        return rows[0]

    def member(node, name):
        return (node is not None and node.type == "member_expression"
                and node.child_by_field_name("property").text == name)

    nodes = list(walk(tree.root_node))
    payload = []
    if not decoder_only:
        send = one((node for node in nodes if node.type == "call_expression"
                    and member(node.child_by_field_name("function"), b"sendCommand")), "GET sendCommand")
        args = send.child_by_field_name("arguments").named_children
        if method == "getExtendedFirmwareVersion":
            if len(args) != 2:
                raise ValueError("Current firmware getter has an unexpected payload")
        else:
            if len(args) != 3 or args[2].type != "identifier":
                raise ValueError("Current getter has a nonliteral payload owner")
            payload_name = args[2].text
            declaration = one((node for node in nodes if node.type == "variable_declarator"
                and node.child_by_field_name("name").text == payload_name), "GET payload declaration")
            value = declaration.child_by_field_name("value")
            if (value.type != "new_expression" or value.child_by_field_name("constructor").text != b"Uint8Array"):
                raise ValueError("Current GET payload is not zero-initialized")
            allocation = value.child_by_field_name("arguments").named_children
            if (len(allocation) != 1 or allocation[0].type != "subscript_expression"
                    or allocation[0].child_by_field_name("object").text != args[0].text
                    or allocation[0].child_by_field_name("index").text != b"0"):
                raise ValueError("Current GET payload length differs from its command")
            writes = [node for node in nodes if node.type == "assignment_expression"
                      and node.child_by_field_name("left").type == "subscript_expression"
                      and node.child_by_field_name("left").child_by_field_name("object").text == payload_name]
            if method in ("getBatteryLevel", "getChargingStatus"):
                arrow = one((node for node in tree.root_node.named_children[0].named_children[0].named_children
                             if node.type == "arrow_function"), "battery getter")
                parameters = arrow.child_by_field_name("parameters").named_children
                if (len(parameters) != 2 or len(writes) != 1
                        or writes[0].child_by_field_name("left").child_by_field_name("index").text != b"0"
                        or writes[0].child_by_field_name("right").text != parameters[1].text):
                    raise ValueError("Current battery selector is not payload byte zero")
                payload = [0]
            elif method != "getTimeToSleep" or writes:
                raise ValueError("Current idle getter payload is not empty")
        if method == "getTimeToSleep":
            return {"payload": payload, "response_decoder": "separate current idle decoder"}

    assignment = one((node for node in nodes if node.type == "assignment_expression"
                      and member(node.child_by_field_name("left"), b"jsonData")), "observed jsonData")
    result_name = assignment.child_by_field_name("left").child_by_field_name("object").text
    result = assignment.child_by_field_name("right")
    if result.type != "object":
        raise ValueError("Current observation is not a literal result")

    def expression(node):
        if node.type == "number":
            return int(node.text)
        if node.type == "subscript_expression":
            obj, index = node.child_by_field_name("object"), node.child_by_field_name("index")
            if (member(obj, b"data") and obj.child_by_field_name("object").text == result_name
                    and index.type == "number"):
                return ["byte", int(index.text)]
        if node.type == "binary_expression":
            return [node.child_by_field_name("operator").text.decode(),
                    expression(node.child_by_field_name("left")), expression(node.child_by_field_name("right"))]
        if node.type == "call_expression":
            fn = node.child_by_field_name("function")
            args = node.child_by_field_name("arguments").named_children
            if fn.text == b"Math.floor" and len(args) == 1:
                return ["floor", expression(args[0])]
            if fn.type == "member_expression" and len(args) == 2:
                if method == "getChargingStatus" and member(args[0], b"Jz"):
                    return ["charging_enum", expression(args[1])]
                if method == "getTimeToSleep":
                    return ["big_endian", expression(args[0]), expression(args[1])]
        raise ValueError("Current observation has an unresolved decoder expression")

    observed = {}
    for pair in result.named_children:
        key, value = pair.child_by_field_name("key"), pair.child_by_field_name("value")
        if pair.type != "pair" or key is None or key.type != "property_identifier" or key.text.decode() in observed:
            raise ValueError("Current observation contains repeated or dynamic result fields")
        observed[key.text.decode()] = expression(value)
    expected = {
        "getExtendedFirmwareVersion": {"major": ["byte", 0], "minor": ["byte", 1],
                                       "internal": ["byte", 2], "reserved": ["byte", 3]},
        "getBatteryLevel": {"batteryId": ["byte", 0],
                            "batteryLevel": ["floor", ["*", ["/", ["byte", 1], 255], 100]]},
        "getChargingStatus": {"batteryId": ["byte", 0], "chargingStatus": ["byte", 1],
                              "chargingStatusEnum": ["charging_enum", ["byte", 1]]},
        "getTimeToSleep": {"timeToSleep": ["big_endian", ["byte", 0], ["byte", 1]]},
    }
    if observed != expected.get(method):
        raise ValueError("Current observation differs from its typed response decoder: " + method)
    return {"payload": payload, "observed_fields": observed}


def current_read_key_inventory(buttons, mapping):
    """Project the mounted normal-mode UI keys into their firmware identities."""
    def fields(node):
        if node.type != "object":
            raise ValueError("Current keyboard inventory requires literal objects")
        result = {}
        for pair in node.named_children:
            key, value = pair.child_by_field_name("key"), pair.child_by_field_name("value")
            if pair.type != "pair" or key is None or key.type != "property_identifier" or key.text in result:
                raise ValueError("Current keyboard inventory has dynamic or repeated fields")
            result[key.text] = value
        return result

    if buttons.type != "array":
        raise ValueError("Current keyboard UI inventory is not a fixed array")
    defaults = fields(mapping)
    rows = {}
    for button in buttons.named_children:
        row = fields(button)
        shift = row.get(b"isHyperShift")
        if shift is not None:
            if shift.text not in (b"!0", b"!1", b"true", b"false"):
                raise ValueError("Current keyboard UI inventory has a dynamic mode")
            if shift.text in (b"!0", b"true"):
                continue
        key = row.get(b"inputID")
        if key is None or key.type != "string":
            raise ValueError("Current keyboard UI key has no literal identity")
        key_name = json.loads(key.text)
        if key_name.encode() not in defaults:
            raise ValueError("Current keyboard UI key has no firmware mapping: " + key_name)
        default = fields(defaults[key_name.encode()])
        firmware = default.get(b"fwID")
        identity = default.get(b"inputID")
        if (firmware is None or firmware.type != "number" or identity is None or identity.type != "string"
                or json.loads(identity.text) != key_name):
            raise ValueError("Current keyboard firmware identity is absent, dynamic or mismatched")
        firmware_id = bounded(int(firmware.text), 254, "keyboard firmware key ID", 1)
        ignored = default.get(b"ignoreInObm")
        if ignored is not None and ignored.text not in (b"!1", b"false"):
            raise ValueError("Current keyboard normal-mode ignore branch requires an explicit audit")
        previous = rows.get(firmware_id)
        if previous is not None and previous != key_name:
            raise ValueError("Current keyboard firmware key ID is ambiguous")
        rows[firmware_id] = key_name
    if not rows:
        raise ValueError("Current keyboard firmware inventory is empty")
    return [{"firmware_key_id": key, "input_id": rows[key]} for key in sorted(rows)]


def current_onboard_key_inventory(buttons, generic, overrides, mouse):
    """Apply the source's per-mode shallow overrides and shared ignore lists."""
    def fields(node):
        if node.type != "object":
            raise ValueError("Current onboard key inventory requires literal objects")
        result = {}
        for pair in node.named_children:
            key, value = pair.child_by_field_name("key"), pair.child_by_field_name("value")
            if pair.type != "pair" or key is None or key.type != "property_identifier" or key.text in result:
                raise ValueError("Current onboard key inventory has dynamic or repeated fields")
            result[key.text] = value
        return result

    def boolean(node):
        if node is None:
            return False
        if node.text not in (b"!0", b"!1", b"true", b"false"):
            raise ValueError("Current onboard key inventory has a dynamic boolean")
        return node.text in (b"!0", b"true")

    def identity(name, node):
        row = fields(node)
        key = row.get(b"inputID")
        if key is None or key.type != "string" or json.loads(key.text).encode() != name:
            raise ValueError("Current onboard key inventory has a mismatched identity")
        firmware = row.get(b"fwID")
        if firmware is not None:
            if firmware.type != "number":
                raise ValueError("Current onboard key inventory has a dynamic firmware ID")
            firmware = bounded(int(firmware.text), 254, "onboard firmware key ID", 1)
        return firmware, boolean(row.get(b"ignoreInObm"))

    generic_modes, override_modes, mouse_modes = fields(generic), fields(overrides), fields(mouse)
    if set(generic_modes) != {b"NormalMode", b"HypershiftMode"} or set(mouse_modes) != set(generic_modes):
        raise ValueError("Current onboard key inventory has unknown modes")
    if set(override_modes) - set(generic_modes):
        raise ValueError("Current onboard key inventory has an unknown override mode")
    modes, ignored = {}, {}
    for mode in generic_modes:
        merged = fields(generic_modes[mode])
        if mode in override_modes:
            merged.update(fields(override_modes[mode]))
        modes[mode] = {name: identity(name, node) for name, node in merged.items()}
        mouse_rows = [identity(name, node) for name, node in fields(mouse_modes[mode]).items()]
        ignored[mode] = {firmware for firmware, skip in [*modes[mode].values(), *mouse_rows] if skip}
        if None in ignored[mode]:
            raise ValueError("Current onboard ignore list contains an absent firmware ID")

    if buttons.type != "array":
        raise ValueError("Current onboard UI inventory is not a fixed array")
    rows = {}
    for button in buttons.named_children:
        row = fields(button)
        if boolean(row.get(b"isHyperShift")):
            continue
        key = row.get(b"inputID")
        if key is None or key.type != "string":
            raise ValueError("Current onboard UI key has no literal identity")
        name = json.loads(key.text)
        normal = modes[b"NormalMode"].get(name.encode())
        if normal is None or normal[0] is None:
            raise ValueError("Current onboard UI key has no firmware mapping: " + name)
        firmware = normal[0]
        hyper_matches = [key for key, value in modes[b"HypershiftMode"].items() if value[0] == firmware]
        if hyper_matches != [name.encode()]:
            raise ValueError("Current onboard key has an ambiguous or mismatched Hypershift identity")
        projected = {"input_id": name, "firmware_key_id": firmware,
                     "ignore_normal": firmware in ignored[b"NormalMode"],
                     "ignore_hypershift": firmware in ignored[b"HypershiftMode"]}
        if firmware in rows and rows[firmware] != projected:
            raise ValueError("Current onboard physical firmware key ID is ambiguous")
        rows[firmware] = projected
    if not rows:
        raise ValueError("Current onboard physical key inventory is empty")
    return [rows[key] for key in sorted(rows)]


def render_current_onboard_inventory(inventory):
    """Render every source-selected physical key and its two independent filters."""
    exact_fields(inventory, {"inventories", "uk_layout_ids"}, "current onboard inventory")
    exact_fields(inventory["inventories"], {"BUTTON_LIST", "BUTTON_LIST_UK"}, "current onboard layouts")
    layouts = inventory["uk_layout_ids"]
    if not isinstance(layouts, list) or not layouts or len(layouts) != len(set(layouts)):
        raise ValueError("Current onboard UK layout identities are empty or repeated")
    for layout in layouts:
        bounded(layout, 0xffffffff, "onboard layout ID")
    lines = ["//! Source-selected physical keys and onboard mode filters.",
        "//! Generated by tools/workspace.py prepare-local-protocol-tables --current-reads.", "",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "#[non_exhaustive]", "pub struct Key {",
        "    pub input_id: &'static str,", "    pub firmware_key_id: u8,",
        "    pub ignore_normal: bool,", "    pub ignore_hypershift: bool,", "}", "",
        "impl Key {", "    const fn new(input_id: &'static str, firmware_key_id: u8, ignore_normal: bool, ignore_hypershift: bool) -> Self {",
        "        Self { input_id, firmware_key_id, ignore_normal, ignore_hypershift }", "    }", "}", ""]
    for source, target in (("BUTTON_LIST", "KEYS"), ("BUTTON_LIST_UK", "UK_KEYS")):
        rows = inventory["inventories"][source]
        if not isinstance(rows, list) or not rows:
            raise ValueError("Current onboard key table is empty")
        firmware_ids, input_ids = [], []
        lines.append("const " + target + ": &[Key] = &[")
        for row in rows:
            exact_fields(row, {"input_id", "firmware_key_id", "ignore_normal", "ignore_hypershift"}, "onboard physical key")
            if not isinstance(row["input_id"], str) or not row["input_id"]:
                raise ValueError("Current onboard key identity is absent")
            bounded(row["firmware_key_id"], 254, "onboard physical firmware ID", 1)
            if type(row["ignore_normal"]) is not bool or type(row["ignore_hypershift"]) is not bool:
                raise ValueError("Current onboard key filter is not boolean")
            firmware_ids.append(row["firmware_key_id"]); input_ids.append(row["input_id"])
            lines.append("    Key::new(" + json.dumps(row["input_id"]) + ", " + str(row["firmware_key_id"]) + ", "
                         + str(row["ignore_normal"]).lower() + ", " + str(row["ignore_hypershift"]).lower() + "),")
        if firmware_ids != sorted(set(firmware_ids)) or len(input_ids) != len(set(input_ids)):
            raise ValueError("Current onboard physical key identities repeat or are out of order")
        lines.extend([" ];", ""])
    lines.extend(["/// Selects the mounted source inventory for the device's observed layout.",
        "pub fn current_679_inventory(layout_id: u32) -> &'static [Key] {",
        "    if [" + ", ".join(map(str, layouts)) + "].contains(&layout_id) {", "        UK_KEYS", "    } else {",
        "        KEYS", "    }", "}", ""])
    return "\n".join(lines).encode()


def current_onboard_literal(node):
    """Decode closed AST literals without executing a vendor expression."""
    if node.has_error:
        raise ValueError("Onboard mapping contains a JavaScript AST error")
    if node.type == "object":
        result = {}
        for pair in node.named_children:
            if pair.type == "comment":
                continue
            key, value = pair.child_by_field_name("key"), pair.child_by_field_name("value")
            if pair.type != "pair" or key is None or value is None:
                raise ValueError("Onboard mapping contains a dynamic object member")
            if key.type == "property_identifier":
                name = key.text.decode()
            elif key.type == "string":
                name = json.loads(key.text)
            elif key.type == "number" and re.fullmatch(rb"0|[1-9][0-9]*", key.text):
                name = key.text.decode()
            else:
                raise ValueError("Onboard mapping contains a dynamic object key")
            if name in result:
                raise ValueError("Onboard mapping repeats a literal field: " + name)
            result[name] = current_onboard_literal(value)
        return result
    if node.type == "array":
        result, expects_value = [], True
        for child in node.children:
            if child.type in {"[", "]", "comment"}:
                continue
            if child.type == ",":
                if expects_value:
                    raise ValueError("Onboard mapping contains an array hole")
                expects_value = True
            else:
                if not expects_value or child.type == "spread_element":
                    raise ValueError("Onboard mapping contains a dynamic array")
                result.append(current_onboard_literal(child))
                expects_value = False
        return result
    if node.type == "string":
        return json.loads(node.text)
    if node.type == "number" and re.fullmatch(rb"0|[1-9][0-9]*", node.text):
        return int(node.text)
    if node.text in (b"!0", b"true"):
        return True
    if node.text in (b"!1", b"false"):
        return False
    raise ValueError("Onboard mapping is not a closed typed literal: " + node.type)


def render_current_onboard_mapping(projection):
    """Generate source enums and complete firmware/HID default lookup rows."""
    exact_fields(projection, {"keyboard", "mouse", "encoding"}, "onboard mapping projection")
    encoding = projection["encoding"]
    expected = {"function_ids", "modifier_bits", "modifier_names", "media_codes", "media_names",
                "power_codes", "razer_codes", "shortcut_codes", "shortcut_names", "mouse_names", "profile_names"}
    exact_fields(encoding, expected, "onboard mapping encoding")

    def variant(value):
        if not isinstance(value, str) or re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", value) is None:
            raise ValueError("Onboard mapping enum identity is invalid")
        return "".join(part[:1].upper() + part[1:] for part in value.split("_"))

    def number(value, maximum=255):
        if isinstance(value, str) and value.isdecimal():
            value = int(value)
        return bounded(value, maximum, "onboard mapping number")

    def enum_values(values):
        values = list(values)
        if not values:
            raise ValueError("Onboard mapping enum is empty")
        names = [variant(value) for value in values]
        if len(names) != len(set(names)):
            raise ValueError("Onboard mapping enum variants collide")
        return values

    for field in expected:
        if not isinstance(encoding[field], dict) or not encoding[field]:
            raise ValueError("Onboard mapping encoding is not a nonempty table: " + field)
    for codes, names in (("media_codes", "media_names"), ("shortcut_codes", "shortcut_names"),
                         ("modifier_bits", "modifier_names")):
        if set(encoding[codes]) != set(encoding[names]):
            raise ValueError("Onboard mapping wire codes and source names differ: " + codes)

    lines = ["//! Current 679 onboard output and default mapping tables.",
             "//! Generated by tools/workspace.py prepare-local-protocol-tables --current-reads.",
             "use super::{DefaultMapping, ExtendedKey, HidKey, KeyboardOutput, Mode, Output, ProfileNavigation};",
             "use serde::{Deserialize, Serialize};", ""]

    def enum(name, values):
        lines.extend(["#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]", "pub enum " + name + " {"])
        for value in enum_values(values):
            lines.extend(["    #[serde(rename = " + json.dumps(value) + ")]", "    " + variant(value) + ","])
        lines.extend(["}", ""])

    def codes_function(name, enum_name, values, maximum=255):
        values = {key: number(value, maximum) for key, value in values.items()}
        if len(values.values()) != len(set(values.values())):
            raise ValueError("Onboard mapping wire codes repeat")
        lines.extend(["pub(super) fn " + name + "(code: u16) -> Option<" + enum_name + "> {", "    match code {"])
        for key, value in values.items():
            lines.append("        " + str(value) + " => Some(" + enum_name + "::" + variant(key) + "),")
        lines.extend(["        _ => None,", "    }", "}", ""])

    enum("FunctionId", encoding["function_ids"])
    codes_function("function_id", "FunctionId", encoding["function_ids"])
    enum("MediaAction", encoding["media_names"].values())
    codes_function("media_action", "MediaAction", {encoding["media_names"][key]: value for key, value in encoding["media_codes"].items()}, 65535)
    enum("PowerAction", encoding["power_codes"])
    codes_function("power_action", "PowerAction", encoding["power_codes"])
    enum("RazerAction", encoding["razer_codes"])
    codes_function("razer_action", "RazerAction", encoding["razer_codes"])
    enum("ShortcutAction", encoding["shortcut_names"].values())
    codes_function("shortcut_action", "ShortcutAction", {encoding["shortcut_names"][key]: value for key, value in encoding["shortcut_codes"].items()})
    enum("MouseAction", encoding["mouse_names"].values())
    enum("ProfileAction", encoding["profile_names"].values())

    def action(family, value, values):
        if value not in values:
            raise ValueError("Onboard mapping selects an unknown " + family)
        return family + "::" + variant(value)

    modifier_bits = encoding["modifier_bits"]
    modifier_names = encoding["modifier_names"]
    for bit in modifier_bits.values():
        bounded(bit, 255, "onboard modifier bit", 1)
    enum_values(modifier_names.values())
    if (list(modifier_bits.values()) != [1, 2, 4, 8, 16, 32, 64, 128]
            or set(modifier_bits) != set(modifier_names)):
        raise ValueError("Onboard modifier bit order or names changed")
    lines.extend(["pub(super) const MODIFIER_NAMES: &[(u8, &str)] = &["])
    for key, bit in modifier_bits.items():
        lines.append("    (" + str(bit) + ", " + json.dumps(modifier_names[key]) + "),")
    lines.extend([
        "];", "",
    ])

    def output(row):
        kind = row["outputType"]
        group = row.get(kind)
        if kind == "disableGroup":
            if group is not None:
                raise ValueError("Onboard disabled default adds a group")
            return "Output::Disabled"
        if kind == "hyperShiftGroup":
            if group is not None:
                raise ValueError("Onboard Hypershift default adds a group")
            return "Output::Hypershift"
        if kind == "keyboardGroup":
            if not isinstance(group, dict) or set(group) - {"key", "modifiers", "extendedKey"} or not {"key", "modifiers"} <= set(group):
                raise ValueError("Onboard keyboard group fields changed")
            exact_fields(group["key"], {"HID", "flag", "pageID"}, "onboard keyboard key")
            mask, extended = 0, False
            if not isinstance(group["modifiers"], list):
                raise ValueError("Onboard modifiers are not a list")
            for value in group["modifiers"]:
                if value == "Extended_Key":
                    if extended:
                        raise ValueError("Onboard extended modifier repeats")
                    extended = True
                else:
                    matches = [modifier_bits[key] for key, name in modifier_names.items() if name == value]
                    if len(matches) != 1 or mask & matches[0]:
                        raise ValueError("Onboard physical modifier is unknown or repeated")
                    mask |= matches[0]
            extension = group.get("extendedKey")
            if extension not in (None, "Extended_00"):
                raise ValueError("Onboard extended keyboard group changed")
            return ("Output::Keyboard(KeyboardOutput { key: HidKey { hid: " + str(number(group["key"]["HID"], 65535))
                + ", flag: " + str(number(group["key"]["flag"])) + ", page_id: " + str(number(group["key"]["pageID"], 65535))
                + " }, modifiers: " + str(mask) + ", extended_modifier: " + str(extended).lower()
                + ", extended_key: " + ("Some(ExtendedKey::Extended00)" if extension else "None") + ", turbo: None })")
        if kind in {"multimediaGroup", "powerKeyGroup", "razerKeyGroup", "win8ShortcutsGroup"}:
            field, family, values, result = {
                "multimediaGroup": ("multimediaAssignment", "MediaAction", encoding["media_names"].values(), "Media"),
                "powerKeyGroup": ("powerKeyAssignment", "PowerAction", encoding["power_codes"], "Power"),
                "razerKeyGroup": ("razerKeyAssignment", "RazerAction", encoding["razer_codes"], "Razer"),
                "win8ShortcutsGroup": ("windowsShortcutAssignment", "ShortcutAction", encoding["shortcut_names"].values(), "WindowsShortcut"),
            }[kind]
            exact_fields(group, {field}, "onboard " + kind)
            return "Output::" + result + "(" + action(family, group[field], values) + ")"
        if kind == "profileNavigationGroup":
            exact_fields(group, {"profileNavigationAssignment"}, "onboard profile default")
            return "Output::Profile(ProfileNavigation { action: " + action("ProfileAction", group["profileNavigationAssignment"], encoding["profile_names"].values()) + ", slot_id: None })"
        raise ValueError("Onboard keyboard default output requires a typed implementation: " + kind)

    exact_fields(projection["keyboard"], {"NormalMode", "HypershiftMode"}, "onboard keyboard modes")

    def input_row(name, row, mouse=False):
        if not isinstance(name, str) or not name or not isinstance(row, dict):
            raise ValueError("Onboard default mapping has no source identity")
        base = {"inputID", "inputType", "outputType"}
        kind = row.get("outputType")
        inputs = {"KeyInput": "keyInput", "DKMInput": "dkmInput"}
        if mouse:
            inputs["MouseInput"] = "mouseInput"
            base.add("fwID")
        input_field = inputs.get(row.get("inputType"))
        if input_field is None:
            raise ValueError("Onboard default input type changed")
        base.add(input_field)
        optional = {"fwID", "isHyperShift", "ignoreInObm"}
        if not isinstance(kind, str):
            raise ValueError("Onboard default output type is absent")
        if kind not in {"disableGroup", "hyperShiftGroup"}:
            base.add(kind)
        if set(row) - base - optional or not base <= set(row) or row["inputID"] != name:
            raise ValueError("Onboard default mapping fields or identity changed")
        firmware_id = row.get("fwID")
        if "fwID" in row:
            number(firmware_id)
        if input_field == "keyInput":
            exact_fields(row["keyInput"], {"HID", "pageID"}, "onboard input key")
            number(row["keyInput"]["pageID"], 65535)
            input_hid = number(row["keyInput"]["HID"], 65535)
        else:
            if row[input_field] != name:
                raise ValueError("Onboard default input identity changed")
            input_hid = None
        if any(type(row[field]) is not bool for field in ("isHyperShift", "ignoreInObm") if field in row):
            raise ValueError("Onboard default mode or filter is not boolean")
        return firmware_id, input_hid

    lines.append("pub(super) const KEYBOARD: &[DefaultMapping] = &[")
    for mode, rows in projection["keyboard"].items():
        ids = set()
        if not isinstance(rows, dict) or not rows:
            raise ValueError("Onboard keyboard mapping rows are empty")
        for name, row in rows.items():
            firmware_id, input_hid = input_row(name, row)
            if firmware_id is not None:
                number(firmware_id)
                if firmware_id in ids:
                    raise ValueError("Onboard default firmware identity repeats")
                ids.add(firmware_id)
            hid = "None" if input_hid is None else "Some(" + str(input_hid) + ")"
            lines.append("    DefaultMapping { input_id: " + json.dumps(name) + ", mode: Mode::"
                + ("Normal" if mode == "NormalMode" else "Hypershift") + ", firmware_key_id: "
                + ("None" if firmware_id is None else "Some(" + str(firmware_id) + ")")
                + ", input_hid: " + hid + ", output: " + output(row) + " },")
    lines.extend([
        "];", "", "pub(super) const MOUSE: &[(Mode, u8, MouseAction)] = &["])
    exact_fields(projection["mouse"], {"NormalMode", "HypershiftMode"}, "onboard mouse modes")
    for mode, rows in projection["mouse"].items():
        ids = set()
        if not isinstance(rows, dict) or not rows:
            raise ValueError("Onboard mouse mapping rows are empty")
        for name, row in rows.items():
            firmware_id, _ = input_row(name, row, mouse=True)
            if firmware_id in ids:
                raise ValueError("Onboard mouse firmware identity repeats")
            ids.add(firmware_id)
            kind = row["outputType"]
            if kind in {"keyboardGroup", "profileNavigationGroup"}:
                output(row)
                continue
            if kind == "sensitivityGroup":
                group = row[kind]
                if (not isinstance(group, dict) or "sensitivityAssignment" not in group
                        or set(group) - {"sensitivityAssignment", "x", "y"}
                        or group["sensitivityAssignment"] not in {"DPI_Up", "DPI_Down", "DPI_CycleUp", "DPI_Clutch"}):
                    raise ValueError("Onboard unused mouse sensitivity fields changed")
                for field in ("x", "y"):
                    if field in group:
                        number(group[field], 65535)
                continue
            if kind == "scrollingGroup":
                exact_fields(row[kind], {"scrollingAssignment"}, "onboard unused mouse scrolling")
                if row[kind]["scrollingAssignment"] != "SCROLL_CycleUp":
                    raise ValueError("Onboard unused mouse scrolling action changed")
                continue
            if kind != "mouseGroup":
                raise ValueError("Onboard mouse default output requires a typed implementation: " + kind)
            group = row["mouseGroup"]
            if not isinstance(group, dict) or set(group) - {"mouseAssignment", "turboMode"} or "mouseAssignment" not in group:
                raise ValueError("Onboard mouse assignment fields changed")
            if "turboMode" in group:
                exact_fields(group["turboMode"], {"isTurbo", "keysPerSecond"}, "onboard mouse default turbo")
                if group["turboMode"] != {"isTurbo": True, "keysPerSecond": 7}:
                    raise ValueError("Onboard mouse default turbo changed")
            lines.append("    (Mode::" + ("Normal" if mode == "NormalMode" else "Hypershift") + ", " + str(firmware_id)
                + ", " + action("MouseAction", group["mouseAssignment"], encoding["mouse_names"].values()) + "),")
    lines.extend([" ];", ""])
    return "\n".join(lines).encode()


def current_read_inventory_selection_contract(selection, merging):
    """Reject changed layout or override semantics before projecting the key tables."""
    import sys
    sys.path.insert(0, str(Path(__file__).resolve().parent / "analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    parser = Parser(Language(tree_sitter_javascript.language()))

    def syntax(body):
        tree = parser.parse(body.encode())
        if tree.root_node.has_error:
            raise ValueError("Current keyboard inventory selection has a JavaScript AST error")

        def shape(node):
            children = [child for child in node.children if child.type != "comment"]
            return (node.type, tuple(shape(child) for child in children)) if children else (node.type, node.text)
        return shape(tree.root_node)

    expected_selection = ('w=()=>{let e=[];return e=o.Yi0.includes(m.A.runtimeData.layoutId)&&n.BUTTON_LIST_UK?'
        '[...n.BUTTON_LIST_UK]:m.A.runtimeData.layoutId===_.Z7j.Japanese&&n.BUTTON_LIST_JP?'
        '[...n.BUTTON_LIST_JP]:[...n.BUTTON_LIST],e}')
    expected_merging = ('C=()=>{var e,t;if(O)return O;let i={};'
        'const{inputMapping:n,inputMappingJP:r,inputMappingUK:s}=p.A.getFeatureParam("isObm","yes");'
        'return i=m.A.runtimeData.layoutId===_.Z7j.Japanese&&r?r:o.Yi0.includes(m.A.runtimeData.layoutId)&&s?s:n,'
        'i?(O={NormalMode:Object.assign(Object.assign({},a.Bx.NormalMode),'
        '(null===(e=i.keyboard)||void 0===e?void 0:e.NormalMode)||{}),'
        'HypershiftMode:Object.assign(Object.assign({},a.Bx.HypershiftMode),'
        '(null===(t=i.keyboard)||void 0===t?void 0:t.HypershiftMode)||{})},O):(O=a.Bx,O)}')
    if syntax(selection) != syntax(expected_selection) or syntax(merging) != syntax(expected_merging):
        raise ValueError("Current keyboard layout inventory or firmware override semantics changed")
    return {"layout_and_override_selection_equal": True}


def verify_current_read_key_fixtures(expected, rust_raw, python_raw):
    """Keep read-only probe allowlists equal to the source-derived key inventory."""
    import ast
    tree = receiver_table_tree(rust_raw)
    declarations, pending = [], [tree.root_node]
    while pending:
        node = pending.pop(); pending.extend(node.named_children)
        if node.type == "const_item" and node.child_by_field_name("name").text == b"KEYBOARD_679_LAYOUT_1_KEYS":
            declarations.append(node)
    if len(declarations) != 1 or declarations[0].parent.type != "source_file":
        raise ValueError("Keyboard probe has no unique module-owned Rust key inventory")
    value = declarations[0].child_by_field_name("value")
    if value.type != "reference_expression" or len(value.named_children) != 1 or value.named_children[0].type != "array_expression":
        raise ValueError("Keyboard probe Rust inventory is not a literal slice")
    array = value.named_children[0]
    if any(node.type != "integer_literal" for node in array.named_children):
        raise ValueError("Keyboard probe Rust key inventory is dynamic")
    rust_keys = [int(node.text) for node in array.named_children]
    declarations = []
    for node in ast.parse(python_raw).body:
        targets = node.targets if isinstance(node, ast.Assign) else [node.target] if isinstance(node, ast.AnnAssign) else []
        if any(isinstance(target, ast.Name) and target.id == "_KEYBOARD_679_LAYOUT_1_KEYS" for target in targets):
            declarations.append(node)
    if len(declarations) != 1:
        raise ValueError("Keyboard probe has no unique module-owned Python key inventory")
    value = declarations[0].value
    if (not isinstance(value, ast.Call) or not isinstance(value.func, ast.Name) or value.func.id != "frozenset"
            or len(value.args) != 1 or value.keywords or not isinstance(value.args[0], (ast.Tuple, ast.List, ast.Set))
            or any(not isinstance(node, ast.Constant) or type(node.value) is not int for node in value.args[0].elts)):
        raise ValueError("Keyboard probe Python inventory is not a literal immutable set")
    python_keys = [node.value for node in value.args[0].elts]
    for owner, actual in (("Rust", rust_keys), ("Python", python_keys)):
        if actual != expected or len(actual) != len(set(actual)):
            raise ValueError("Keyboard probe " + owner + " key inventory differs from current source")
    return {"keys": len(expected), "rust_and_python_key_order_equal": True}


def current_read_sources():
    """Resolve observation getters through each current mounted Protocol25 class."""
    import analysis
    import host
    audit = host.HostLocalAudit()
    indexed = json.loads(audit.artifact(ROOT / ".analysis/inputs.json"))
    physical = analysis.feature_report_callers(audit.parser, {row["path"]: row for row in indexed["inputs"]},
        selected_capabilities=[{"product_id": 190}, {"product_id": 679}])
    capabilities, products = [], []

    def binding_value(source, binding):
        if binding["resolution"] != "declaration":
            raise ValueError("Current observation dependency has no unique declaration")
        node = audit.tree(source).root_node.descendant_for_byte_range(binding["start_byte"], binding["end_byte"] - 1)
        if node.type != "variable_declarator":
            raise ValueError("Current observation dependency is not a literal binding")
        return node.child_by_field_name("value")

    def imported(source, member, files, role):
        if member.type != "member_expression":
            raise ValueError("Current observation dependency is not an imported member")
        obj, prop = member.child_by_field_name("object"), member.child_by_field_name("property")
        if obj.type != "identifier" or prop.type != "property_identifier":
            raise ValueError("Current observation dependency has a dynamic member")
        binding = audit.binding(role + "_import", source, obj.text.decode(), member.start_byte)
        value = binding_value(source, binding)
        if value.type != "call_expression" or value.child_by_field_name("function").type != "identifier":
            raise ValueError("Current observation import is not a static webpack require")
        args = value.child_by_field_name("arguments").named_children
        if len(args) != 1 or args[0].type != "number":
            raise ValueError("Current observation import ID is not a literal")
        return audit.webpack_export(role, files, int(args[0].text), prop.text.decode())

    def calls(root, method):
        found, pending = [], [root]
        while pending:
            node = pending.pop(); pending.extend(reversed(node.named_children))
            if node.type != "call_expression":
                continue
            fn = node.child_by_field_name("function")
            if fn.type == "member_expression" and fn.child_by_field_name("property").text == method:
                found.append(node)
        return found

    for factory in physical["products"]:
        pid = factory["product_id"]
        if factory["failures"] or not factory["source_owners_complete"]:
            raise ValueError("Current observation factory is unresolved: " + str(pid) + " " + str(factory["failures"]))
        source_set, files = audit.source_set(factory["namespace"])
        owners = [(source, owner) for source in factory["sources"] for owner in source["owners"]
                  if owner["role"] == "complete_protocol_class" and 'this.name="rzDevice25"' in owner["body"]]
        source_record, owner = min(owners, key=lambda row: len(row[1]["body"]))
        source = source_record["original"]
        root = audit.tree(source).root_node.descendant_for_byte_range(owner["byte_start"], owner["byte_end"] - 1)
        class_owner = audit.retain_range("current_read_class_" + str(pid), source,
            owner["byte_start"], owner["byte_end"], root.type)
        branches = factory["physical_transports_by_pid"]
        cap = {key: value for key, value in branches[str(pid)].items() if key != "is_duallink"}
        cap.update(product_id=pid, vendor_id=5426, direct_pids=sorted(map(int, branches)), report_bytes=91,
                   physical_transports_by_pid=branches,
                   claim_interfaces_by_pid={key: value["claim_interface"] for key, value in branches.items()},
                   queries=[], polling_codes={}, charging_codes={})
        methods = ["getExtendedFirmwareVersion"]
        if pid == 190:
            methods += ["getBatteryLevel", "getChargingStatus", "getTimeToSleep"]
        getters = []
        for method in methods:
            member, member_owner = audit.callable_member(str(pid) + "_" + method, source, root, method)
            delegation = []
            pending = [member]
            while pending:
                node = pending.pop(); pending.extend(reversed(node.named_children))
                if node.type != "call_expression":
                    continue
                args = node.child_by_field_name("arguments").named_children
                fn = node.child_by_field_name("function")
                if args and args[0].type == "this" and fn.type == "member_expression":
                    delegation.append(fn)
            if len(delegation) != 1:
                raise ValueError("Current getter helper is unresolved: " + method)
            helper = imported(source, delegation[0], files, str(pid) + "_" + method + "_helper")
            helper_source, helper_binding = helper["original"], helper["binding"]
            helper_node = binding_value(helper_source, helper_binding)
            sends = calls(helper_node, b"sendCommand")
            if len(sends) != 1:
                raise ValueError("Current observation getter selects multiple commands")
            args = sends[0].child_by_field_name("arguments").named_children
            constant = imported(helper_source, args[0], files, str(pid) + "_" + method + "_command")
            value = binding_value(constant["original"], constant["binding"])
            values = value.child_by_field_name("arguments").named_children if value.type == "new_expression" else []
            if (value.type != "new_expression" or value.child_by_field_name("constructor").text != b"Uint8Array"
                    or len(values) != 1 or values[0].type != "array"
                    or any(item.type != "number" for item in values[0].named_children)):
                raise ValueError("Current observation command is not a fixed byte array")
            command = [int(item.text) for item in values[0].named_children]
            contract = READ_METHODS[method]
            if command != contract[2]:
                raise ValueError("Current observation command differs from the typed decoder: " + method)
            cap["queries"].append({"name": contract[1], "method": method, "command": command,
                "payload": contract[3], "min_response_bytes": contract[4]})
            getters.append({"method": method, "member_owner": member_owner,
                            "helper": helper, "command": constant,
                            "decoder": current_read_decoder_contract(method, helper_binding["body"])})
        products.append({"product_id": pid, "deployment": source_set,
                         "factory": factory, "class_owner": class_owner, "getters": getters})
        capabilities.append(cap)

    mouse = next(row for row in products if row["product_id"] == 190)
    _, files = audit.source_set("mouse-mw-190-current")
    source = next(row for row in files if Path(row["path"]).name.startswith("6120."))
    battery = audit.retain("current_battery_caller", source, "t.getChargingStatus(e)", {"variable_declarator"})
    if len(battery) != 1:
        raise ValueError("Current battery caller is ambiguous")
    caller = audit.owners[battery[0]]
    selector_call = re.search(r'const ([\w$]+)=yield ([\w$]+)\(\);if\(\1<0\)', caller["body"])
    if selector_call is None:
        raise ValueError("Current battery selector caller is unresolved")
    selector = audit.binding("current_battery_id", source, selector_call[2], caller["byte_start"])
    if not re.search(r'else ([\w$]+)=0;return \1', selector["body"]) or '"rzDevice30"' not in selector["body"]:
        raise ValueError("Current Protocol25 battery selector is not zero")
    enum_owner = audit.webpack_export("current_charging_codes", files, 91458, "Jz")
    enum_value = binding_value(enum_owner["original"], enum_owner["binding"])
    if enum_value.type != "object":
        raise ValueError("Current charging enum is not a literal object")
    codes = {}
    for pair in enum_value.named_children:
        key, value = pair.child_by_field_name("key"), pair.child_by_field_name("value")
        if pair.type != "pair" or key.type != "property_identifier" or value.type != "number" or key.text.decode() in codes:
            raise ValueError("Current charging enum contains dynamic or repeated fields")
        codes[key.text.decode()] = int(value.text)
    next(row for row in capabilities if row["product_id"] == 190)["charging_codes"] = codes
    mouse.update(battery_caller=battery[0], battery_selector=selector, charging_codes=enum_owner)
    decoder = audit.binding("current_idle_decoder", source, "g", next(
        row["helper"]["binding"]["start_byte"] for row in mouse["getters"] if row["method"] == "getTimeToSleep"))
    decoder_value = binding_value(source, decoder)
    decoder_calls = []
    pending = [decoder_value]
    while pending:
        node = pending.pop(); pending.extend(node.named_children)
        if node.type == "call_expression":
            decoder_calls.append(node.child_by_field_name("function"))
    if len(decoder_calls) != 1:
        raise ValueError("Current idle decoder calls an unresolved helper")
    number = imported(source, decoder_calls[0], files, "current_idle_big_endian")
    if not re.fullmatch(r'[\w$]+=\(([\w$]+),([\w$]+)\)=>\(255&\1\)<<8\|255&\2', number["binding"]["body"]):
        raise ValueError("Current idle byte order differs from the typed decoder")
    mouse.update(idle_decoder=decoder, idle_big_endian=number,
                 idle_decoder_contract=current_read_decoder_contract("getTimeToSleep", decoder["body"], decoder_only=True),
                 firmware_target="logical mouse currentFWVersion; dongle currentDockFWVersion is a separate getter")
    firmware = audit.retain("current_firmware_target_consumer", source,
        "function Je(e=o.razerWindow.rzDevice,t=f.default.runtimeData)", {"function_declaration"})
    mouse["firmware_consumer"] = firmware
    keyboard = next(row for row in products if row["product_id"] == 679)
    _, files = audit.source_set("keyboard-mw-679-current")
    generic = audit.webpack_export("current_679_firmware_mapping", files, 32906, "Bx")
    generic_node = binding_value(generic["original"], generic["binding"])
    normal = next(pair.child_by_field_name("value") for pair in generic_node.named_children
                  if pair.child_by_field_name("key").text == b"NormalMode")
    inventories = {}
    for export in ("BUTTON_LIST", "BUTTON_LIST_UK"):
        owner = audit.webpack_export("current_679_" + export, files, 26061, export)
        inventories[export] = {"export": owner, "keys": current_read_key_inventory(
            binding_value(owner["original"], owner["binding"]), normal)}
    selection = audit.webpack_export("current_679_layout_key_selection", files, 26192, "NA")
    merging = audit.webpack_export("current_679_firmware_mapping_selection", files, 26192, "Aq")
    selection_contract = current_read_inventory_selection_contract(selection["binding"]["body"], merging["binding"]["body"])
    for name, module_id in (("n", 26061), ("a", 32906), ("o", 11469), ("_", 93490)):
        dependency = audit.binding("current_679_inventory_dependency_" + name, selection["original"], name,
                                   selection["binding"]["start_byte"])
        value = binding_value(selection["original"], dependency)
        if (value.type != "call_expression" or len(value.child_by_field_name("arguments").named_children) != 1
                or value.child_by_field_name("arguments").named_children[0].text != str(module_id).encode()):
            raise ValueError("Current keyboard inventory dependency no longer selects its actual module")
    uk = audit.webpack_export("current_679_uk_layouts", files, 11469, "Yi0")
    layout = audit.webpack_export("current_679_layout_enum", files, 93490, "Z7j")
    layout_node = binding_value(layout["original"], layout["binding"])
    layout_codes = {pair.child_by_field_name("key").text.decode(): int(pair.child_by_field_name("value").text)
                    for pair in layout_node.named_children if pair.type == "pair"}
    uk_node = binding_value(uk["original"], uk["binding"])
    if uk_node.type != "array" or any(node.type != "member_expression" for node in uk_node.named_children):
        raise ValueError("Current keyboard UK layout selection is not a fixed enum list")
    uk_layout_ids = [layout_codes[node.child_by_field_name("property").text.decode()]
                     for node in uk_node.named_children]
    main = inventories["BUTTON_LIST"]["export"]["original"]
    feature = audit.retain("current_679_obm_registration", main, 'useFeature("isObm","yes"', {"call_expression"})
    if len(feature) != 1:
        raise ValueError("Current keyboard OBM registration is ambiguous")
    feature_owner = audit.owners[feature[0]]
    feature_node = audit.tree(main).root_node.descendant_for_byte_range(feature_owner["byte_start"], feature_owner["byte_end"] - 1)
    arguments = feature_node.child_by_field_name("arguments").named_children
    if len(arguments) != 3 or arguments[2].type != "object":
        raise ValueError("Current keyboard OBM registration is not a literal feature")
    options = {pair.child_by_field_name("key").text: pair.child_by_field_name("value") for pair in arguments[2].named_children
               if pair.type == "pair"}
    if b"inputMappingJP" in options or b"inputMappingUK" in options or options[b"engineType"].text != b'"obmEngineKeyboardCalMaxMem"':
        raise ValueError("Current keyboard OBM registration adds an unaudited layout or engine override")
    mapping = options[b"inputMapping"]
    keyboard_mapping = next(pair.child_by_field_name("value") for pair in mapping.named_children
                            if pair.child_by_field_name("key").text == b"keyboard")
    normal_override = next(pair.child_by_field_name("value") for pair in keyboard_mapping.named_children
                           if pair.child_by_field_name("key").text == b"NormalMode")
    if normal_override.type != "object" or normal_override.named_children:
        raise ValueError("Current keyboard adds an unaudited normal-mode firmware key override")
    mouse_mapping = audit.webpack_export("current_679_mount_mouse_mapping", files, 32906, "InputIDToMapping_Mouse_Generic")
    onboard_inventories = {
        name: current_onboard_key_inventory(
            binding_value(item["export"]["original"], item["export"]["binding"]), generic_node,
            keyboard_mapping, binding_value(mouse_mapping["original"], mouse_mapping["binding"]))
        for name, item in inventories.items()
    }
    if "BUTTON_LIST_JP" in audit.owners[inventories["BUTTON_LIST"]["export"]["complete_factory_owner"]]["body"]:
        raise ValueError("Current keyboard adds a Japanese inventory requiring its own source projection")
    factory = audit.webpack_export("current_679_obm_engine", files, 17317, "default")
    factory_root = binding_value(factory["original"], factory["binding"])
    callers = {}
    for method in ("init", "getButtonIdFromUIMappingList", "getAllButtonAssignmentInfo", "getButtonAssignmentListDetail",
                   "createButtonMappingFromFnDataByte"):
        _, owner = audit.callable_member("current_679_" + method, factory["original"], factory_root, method,
                                          receiver="this")
        callers[method] = owner
    engine_source = next(row for row in files if Path(row["path"]).name.startswith("obmEngineKeyboard."))
    encoding, encoding_owners = {}, {}
    for field, module, export in (
            ("function_ids", 36475, "rA"), ("modifier_bits", 39674, "rw"),
            ("modifier_names", 68214, "_A"), ("media_codes", 39674, "w0"),
            ("media_names", 68214, "tC"), ("power_codes", 39674, "ox"),
            ("razer_codes", 68214, "ZI"), ("shortcut_codes", 39674, "V7"),
            ("shortcut_names", 68214, "wo"), ("mouse_names", 68214, "AX"),
            ("profile_names", 68214, "E7")):
        owner = audit.webpack_export("current_679_mapping_" + field,
                                    [engine_source] if module == 39674 else files, module, export)
        encoding[field] = current_onboard_literal(binding_value(owner["original"], owner["binding"]))
        encoding_owners[field] = owner
    defaults = current_onboard_literal(generic_node)
    overrides = current_onboard_literal(keyboard_mapping)
    for mode, rows in overrides.items():
        if mode not in defaults:
            raise ValueError("Current onboard mapping adds an unaudited mode")
        defaults[mode].update(rows)
    keyboard["onboard_mapping"] = {
        "projection": {"keyboard": defaults, "mouse": current_onboard_literal(
            binding_value(mouse_mapping["original"], mouse_mapping["binding"])), "encoding": encoding},
        "encoding_owners": encoding_owners,
        "decoder": callers["createButtonMappingFromFnDataByte"],
        "raw_observation_preserved": True,
        "unknown_output_preserved": True,
    }
    subclass = audit.webpack_export("current_679_calculated_memory_engine", files, 65711, "default")
    mounting_source = selection["original"]
    mounting = audit.retain("current_679_obm_mounting", mounting_source, "obmEngine.init(",
                            {"function_expression", "function_declaration", "arrow_function"})
    keyboard["analog_key_inventory"] = {"inventories": inventories, "normal_firmware_mapping": generic,
        "onboard_physical_keys": {"inventories": onboard_inventories, "uk_layout_ids": uk_layout_ids},
        "shared_mouse_mapping": mouse_mapping,
        "selection_contract": selection_contract,
        "layout_selection": selection, "firmware_mapping_selection": merging, "uk_layout_codes": uk,
        "layout_enum": layout, "uk_layout_ids": uk_layout_ids, "feature_registration": feature[0],
        "engine": factory, "calculated_memory_subclass": subclass, "callers": callers, "mounting": mounting,
        "japanese_specific_ui_inventory": "absent; source selector uses BUTTON_LIST when BUTTON_LIST_JP is absent",
        "other_layouts": "source selector uses BUTTON_LIST for every layout outside uk_layout_ids",
        "profile_id_source": "obmJson.profiles[e].id from actual profile inventory; caller passes profile ID",
        "memory_type": 0, "actuation_batch_limit": 15, "rapid_trigger_batch_limit": 13,
        "key_identity": "firmware fwID from mounted NormalMode mapping; not analogKeyID or HID scan code"}
    fixture_rust = ROOT / "crates/razer-hid/tests/connected_devices.rs"
    fixture_python = ROOT / "tools/native_runtime.py"
    expected_keys = [row["firmware_key_id"] for row in inventories["BUTTON_LIST"]["keys"]]
    equality = verify_current_read_key_fixtures(expected_keys, fixture_rust.read_bytes(), fixture_python.read_bytes())
    import families
    keyboard["analog_key_inventory"]["probe_fixture_equivalence"] = {**equality,
        "rust": families.identity(fixture_rust), "python": families.identity(fixture_python)}
    validate_read_catalog(json.dumps({"schema_version": 1, "products": capabilities}).encode())
    if audit.final_snapshot():
        raise ValueError("Current observation source bytes changed during extraction")
    return capabilities, {"products": products, "complete_owners": list(audit.owners.values()),
                          "inputs": list(audit.artifacts.values()), "parser_versions": audit.parser_versions,
                          "target_execution": False, "runtime_acceptance": "unverified"}


def read_table_constructors(raw):
    """Resolve every literal capability under the production products array."""
    tree = receiver_table_tree(raw)
    found, pending = {}, [tree.root_node]
    while pending:
        node = pending.pop(); pending.extend(reversed(node.named_children))
        if node.type != "struct_expression" or node.child_by_field_name("name").text != b"DeviceReadCapability":
            continue
        fields = {}
        for field in node.child_by_field_name("body").named_children:
            if field.type in {"line_comment", "block_comment"}:
                continue
            if field.type != "field_initializer":
                raise ValueError("Read constructor requires explicit fields")
            key = field.child_by_field_name("field").text.decode()
            if key in fields:
                raise ValueError("Read constructor repeats a field")
            fields[key] = field.child_by_field_name("value")
        pid = fields.get("product_id")
        if pid is None or pid.type != "integer_literal" or int(pid.text) in found:
            raise ValueError("Read constructor has missing or repeated product identity")
        found[int(pid.text)] = node
    if found:
        owner = next(iter(found.values())).parent
        if owner.type != "array_expression" or any(node.parent != owner for node in found.values()):
            raise ValueError("Read table production constructors do not share one array owner")
        function = owner.parent.parent if owner.parent is not None else None
        if (function is None or function.type != "function_item"
                or function.child_by_field_name("name").text != b"products"
                or function.parent.type != "source_file"):
            raise ValueError("Read table constructors are not owned by the production products function")
        values = [node for node in owner.named_children if node.type not in {"line_comment", "block_comment"}]
        if values != list(found.values()):
            raise ValueError("Read table array contains an unrecognized capability expression")
    return tree, found


def current_read_table_projection(content, capabilities, *, update=False):
    """Replace only verified product constructors; preserve the rest of the typed table."""
    expected_raw = render_read_catalog(json.dumps({"schema_version": 1, "products": capabilities}).encode())


    def shape(node):
        return (node.type, tuple(shape(child) for child in node.children
                               if child.type not in {"line_comment", "block_comment"})) if node.children else (node.type, node.text)

    expected_tree, expected = read_table_constructors(expected_raw)
    tree, existing = read_table_constructors(content)
    if not existing:
        raise ValueError("Read table has no production constructors")
    edits, missing = [], []
    for pid, target in expected.items():
        current = existing.get(pid)
        if current is None:
            if not update:
                raise ValueError("Read table omits current product " + str(pid))
            missing.append(expected_raw[target.start_byte:target.end_byte])
        elif shape(current) != shape(target):
            if not update:
                raise ValueError("Read table differs from current source product " + str(pid))
            edits.append((current.start_byte, current.end_byte, expected_raw[target.start_byte:target.end_byte]))
    if missing:
        owner = next(iter(existing.values())).parent
        children = [node for node in owner.children[:-1] if node.type not in {"line_comment", "block_comment"}]
        position = children[-1].end_byte
        separator = b"" if children[-1].type == "," else b","
        replacement = separator + b"\n" + b"\n".join(b"        " + raw + b"," for raw in missing)
        edits.append((position, position, replacement))
    for start, end, replacement in sorted(edits, reverse=True):
        content = content[:start] + replacement + content[end:]
    receiver_table_tree(content)
    return content, sorted(expected)


def prepare_current_read_table(*, check=False, record=True):
    import analysis
    import families
    import subprocess
    capabilities, report = current_read_sources()
    path = ROOT / READ_OUTPUT
    original = path.read_bytes()
    rendered, products = current_read_table_projection(original, capabilities, update=not check)
    if not check and rendered != original:
        rendered = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
            input=rendered, capture_output=True, check=True).stdout
        path.write_bytes(rendered)
    verified, verified_products = current_read_table_projection(path.read_bytes(), capabilities)
    if verified != path.read_bytes() or products != verified_products:
        raise ValueError("Current observation constructor verification differs")
    keyboard = next(row for row in report["products"] if row["product_id"] == 679)
    inventory = keyboard["analog_key_inventory"]["onboard_physical_keys"]
    inventory_path = ROOT / ONBOARD_KEY_OUTPUT
    inventory_bytes = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
        input=render_current_onboard_inventory(inventory), capture_output=True, check=True).stdout
    receiver_table_tree(inventory_bytes)
    if not check:
        inventory_path.write_bytes(inventory_bytes)
    if inventory_path.read_bytes() != inventory_bytes:
        raise ValueError("Current onboard physical key inventory differs from source keys or mode filters")
    mapping_path = ROOT / ONBOARD_MAPPING_OUTPUT
    mapping_bytes = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
        input=render_current_onboard_mapping(keyboard["onboard_mapping"]["projection"]),
        capture_output=True, check=True).stdout
    receiver_table_tree(mapping_bytes)
    if not check:
        mapping_path.write_bytes(mapping_bytes)
    if mapping_path.read_bytes() != mapping_bytes:
        raise ValueError("Current onboard output mapping differs from its source enums or complete defaults")
    report.update(tool=families.identity(Path(__file__)), output=families.identity(path), check_only=check,
                  constructors_verified=products, onboard_key_output=families.identity(inventory_path),
                  onboard_mapping_output=families.identity(mapping_path))
    if record:
        destination = ROOT / ".analysis/results/typed-data.json"
        with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
            previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
            previous["current_local_read_table"] = report
            analysis.save(destination, previous)
    return {"products": products, "queries": {row["product_id"]: [query["name"] for query in row["queries"]]
            for row in capabilities}, "source_factories_and_getters_equal": True, "passed": True,
            "onboard_key_counts": {name: len(rows) for name, rows in inventory["inventories"].items()},
            "check_only": check, "runtime_acceptance": "unverified"}


def mapping_source_projection(body):
    """Project consumed fields from a complete factory using static JS ASTs."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    import json5
    prefix = b"const CAPABILITY_MODULE={"
    raw = prefix + body.encode("utf-8") + b"};"
    tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Current mapping capability factory has a JavaScript AST error")
    module_object = tree.root_node.named_children[0].named_children[0].child_by_field_name("value")
    factory = module_object.named_children[0].child_by_field_name("value")
    statements = factory.child_by_field_name("body")
    bindings = {}
    for statement in statements.named_children:
        if statement.type not in ("lexical_declaration", "variable_declaration"):
            continue
        for declarator in statement.named_children:
            if declarator.type == "variable_declarator":
                name = declarator.child_by_field_name("name")
                if name.type == "identifier":
                    bindings[raw[name.start_byte:name.end_byte].decode()] = declarator
    retained, holes, duplicates, exported = {}, [], [], {}
    def source(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8")
    def find_exports(node):
        if node.type == "call_expression":
            function = node.child_by_field_name("function")
            arguments = node.child_by_field_name("arguments").named_children
            if (function.type == "member_expression" and source(function.child_by_field_name("property")) == "d"
                    and len(arguments) == 2 and arguments[1].type == "object"):
                for pair in arguments[1].named_children:
                    if pair.type != "pair":
                        continue
                    getter = pair.child_by_field_name("value")
                    if getter.type != "arrow_function":
                        continue
                    result = getter.child_by_field_name("body")
                    if result.type == "identifier":
                        name = source(pair.child_by_field_name("key"))
                        if name in exported:
                            raise ValueError("Ambiguous current mapping export: " + name)
                        exported[name] = source(result)
        if node.type not in ("arrow_function", "function_expression", "function_declaration", "method_definition"):
            for child in node.named_children:
                find_exports(child)
    for statement in statements.named_children:
        if statement.type == "expression_statement":
            find_exports(statement)
    def capture(node, name):
        literal = raw[node.start_byte:node.end_byte]
        retained[name] = {"name": name, "byte_start": node.start_byte - len(prefix),
            "byte_end": node.end_byte - len(prefix), "body": literal.decode("utf-8"),
            "body_sha256": hashlib.sha256(literal).hexdigest()}
    def value(node, chain=()):
        if node.type == "identifier":
            name = source(node)
            if name in chain or name not in bindings:
                raise ValueError("Unresolved current mapping capability binding: " + name)
            declaration = bindings[name]
            capture(declaration, name)
            return value(declaration.child_by_field_name("value"), (*chain, name))
        if node.type in ("string", "number", "true", "false", "null"):
            return json5.loads(source(node))
        if node.type == "unary_expression" and source(node) in ("!0", "!1"):
            return source(node) == "!0"
        if node.type == "array":
            output, item_pending = [], True
            for child in node.children[1:-1]:
                if child.type == "comment":
                    continue
                if child.type == ",":
                    if item_pending:
                        holes.append({"byte_start": child.start_byte - len(prefix),
                            "array_body": source(node), "array_index": len(output)})
                        output.append({"source_array_hole": True})
                    item_pending = True
                else:
                    output.append(value(child, chain))
                    item_pending = False
            return output
        raise ValueError("Unsupported consumed JavaScript capability node: " + node.type + " " + source(node))
    def properties(node):
        if node.type != "object":
            raise ValueError("Expected a current capability object, found " + source(node))
        output = {}
        for item in node.named_children:
            if item.type == "comment":
                continue
            if item.type != "pair":
                raise ValueError("Nonliteral capability object property: " + source(item))
            key = item.child_by_field_name("key")
            name = json5.loads(source(key)) if key.type == "string" else source(key)
            if name in output:
                duplicates.append({"name": name, "earlier": source(output[name]),
                    "effective": source(item.child_by_field_name("value")),
                    "semantics": "JavaScript object literal uses the last property with the same key"})
            output[name] = item.child_by_field_name("value")
        return output
    def export(name):
        if name not in exported:
            return None
        if exported[name] not in bindings:
            raise ValueError("Current capability export binding is absent: " + name)
        declaration = bindings[exported[name]]
        capture(declaration, name)
        return declaration.child_by_field_name("value")
    info = export("DeviceInfo")
    if info is None:
        raise ValueError("Current mapping factory does not export DeviceInfo")
    fields = properties(info)
    projected = {}
    for key in ("productId", "category", "isOBMDevice", "isXenaT2Series", "controllerAssignmentsWithoutSynapse"):
        if key in fields:
            if key == "controllerAssignmentsWithoutSynapse":
                projected[key] = source(fields[key]) != "null" and source(fields[key]) != "void 0"
                capture(fields[key], key)
            else:
                projected[key] = value(fields[key])
    if "OBMSpecs" in fields:
        obm = properties(fields["OBMSpecs"])
        if "noFirmwareInputId" in obm:
            projected["noFirmwareInputId"] = value(obm["noFirmwareInputId"])
        if "supportedMappings" in obm:
            supported = properties(obm["supportedMappings"])
            projected["supportedMappings"] = {key: value(supported[key]) for key in
                ("controllerGroup", "macroGroup", "mouseGroup", "scrollingGroup", "sensitivityGroup", "profileNavigationGroup") if key in supported}
    if "ble" in fields:
        ble = properties(fields["ble"])
        if "obmMacrosAvailable" in ble:
            projected["bleMacrosAvailable"] = value(ble["obmMacrosAvailable"])
    for key in ("MOUSE_FUNC_REQUIRED_SYNAPSE_VALUES", "MOUSE_TURBO_FUNC_REQUIRED_SYNAPSE_VALUES"):
        node = export(key)
        if node is not None:
            projected[key] = value(node)
    return projected, {"bindings": list(retained.values()), "array_holes": holes, "duplicate_properties": duplicates,
        "scope": "Exact consumed fields; assignment arrays project includes(valid string) membership, preserving source order and recording omitted holes"}


def render_mapping_capabilities(products):
    categories = {"MOUSE": "Mouse", "MOUSEPLUSMAT": "MousePlusMat", "KEYBOARD": "Keyboard", "SYSTEM": "System"}
    def vector(values, *, holes=False):
        if not isinstance(values, list):
            raise ValueError("Expected a current mapping string array")
        entries = []
        for value in values:
            if holes and value == {"source_array_hole": True}:
                continue
            if not isinstance(value, str):
                raise ValueError("Nonstring current mapping assignment: " + repr(value))
            entries.append(json.dumps(value, ensure_ascii=False) + ".into()")
        return "vec![" + ", ".join(entries) + "]"
    def optional(source, name, renderer):
        return "None" if name not in source else "Some(" + renderer(source[name]) + ")"
    def boolean(value):
        if type(value) is not bool:
            raise ValueError("Expected a current mapping boolean")
        return str(value).lower()
    def support(value):
        return "MappingSupport::Enabled(" + boolean(value) + ")" if type(value) is bool else "MappingSupport::Assignments(" + vector(value, holes=True) + ")"
    lines = ["//! Generated by tools/workspace.py prepare-mapping-capabilities from current complete product factories.",
        "use super::{FirmwareMappingSupport, MappingCapabilities, MappingCategory, MappingSupport};", "",
        "pub(super) fn current_mouse_capabilities() -> Vec<(u32, MappingCapabilities)> {", "    vec!["]
    for row in products:
        info = row["projection"]
        category = "MappingCategory::" + categories[info["category"]] if info["category"] in categories else "MappingCategory::Other(" + json.dumps(info["category"]) + ".into())"
        lines += [f"        ({row['product_id']}, MappingCapabilities {{", "            category: " + category + ",",
            "            is_obm_device: " + optional(info, "isOBMDevice", boolean) + ",",
            "            is_xena_t2_series: " + optional(info, "isXenaT2Series", boolean) + ",",
            "            no_firmware_input_ids: " + optional(info, "noFirmwareInputId", vector) + ",",
            "            supported: FirmwareMappingSupport {"]
        mappings = info.get("supportedMappings", {})
        for field, key in (("controller", "controllerGroup"), ("macros", "macroGroup"), ("mouse", "mouseGroup"),
                           ("scrolling", "scrollingGroup"), ("sensitivity", "sensitivityGroup"), ("profile_navigation", "profileNavigationGroup")):
            lines.append("                " + field + ": " + optional(mappings, key, support) + ",")
        lines += ["            },", "            ble_macros_available: " + optional(info, "bleMacrosAvailable", boolean) + ",",
            "            has_controller_assignments_without_synapse: " + boolean(info.get("controllerAssignmentsWithoutSynapse", False)) + ",",
            "            required_mouse: " + optional(info, "MOUSE_FUNC_REQUIRED_SYNAPSE_VALUES", vector) + ",",
            "            required_turbo_mouse: " + optional(info, "MOUSE_TURBO_FUNC_REQUIRED_SYNAPSE_VALUES", vector) + ",",
            "        }),"]
    return ("\n".join([*lines, "    ]", "}", ""])).encode("utf-8")


def verify_mapping_constructor(content, products):
    """Read every generated field back through the Rust AST and compare semantics."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_rust
    import catalog_data
    parser = Parser(Language(tree_sitter_rust.language()))
    tree = parser.parse(content)
    if tree.root_node.has_error:
        raise ValueError("Generated mapping capability Rust has an AST error")
    def text(node):
        return node.text.decode("utf-8")
    def decode(node):
        if node.type == "block":
            if len(node.named_children) != 1:
                raise ValueError("Generated mapping owner must contain one constructor expression")
            return decode(node.named_children[0])
        if node.type in ("integer_literal", "boolean_literal", "string_literal"):
            return json.loads(text(node))
        if node.type == "identifier" and text(node) == "None":
            return None
        if node.type == "scoped_identifier" and text(node).startswith("MappingCategory::"):
            return text(node).removeprefix("MappingCategory::")
        if node.type == "macro_invocation" and text(node.child_by_field_name("macro")) == "vec":
            token = node.named_children[-1]
            return decode(catalog_data.parse_expression(parser, "[" + text(token)[1:-1] + "]"))
        if node.type in ("array_expression", "tuple_expression"):
            return [decode(child) for child in node.named_children]
        if node.type == "call_expression":
            function = node.child_by_field_name("function")
            arguments = node.child_by_field_name("arguments").named_children
            if function.type == "field_expression" and text(function.child_by_field_name("field")) == "into" and not arguments:
                return decode(function.child_by_field_name("value"))
            if text(function) == "Some" and len(arguments) == 1:
                return decode(arguments[0])
            if text(function) in ("MappingSupport::Enabled", "MappingSupport::Assignments") and len(arguments) == 1:
                return {"kind": text(function).split("::")[-1], "value": decode(arguments[0])}
        if node.type == "struct_expression":
            fields = {}
            for field in node.child_by_field_name("body").named_children:
                if field.type != "field_initializer":
                    raise ValueError("Generated mapping record has an indirect field")
                name = text(field.child_by_field_name("field"))
                if name in fields:
                    raise ValueError("Generated mapping record has a duplicate field")
                fields[name] = decode(field.child_by_field_name("value"))
            return fields
        raise ValueError("Generated mapping constructor is indirect: " + text(node)[:100])
    roots = [node for node in tree.root_node.named_children if node.type == "function_item"
             and text(node.child_by_field_name("name")) == "current_mouse_capabilities"]
    if len(roots) != 1:
        raise ValueError("Generated mapping constructor owner is ambiguous")
    actual = decode(roots[0].child_by_field_name("body"))
    expected = []
    for row in products:
        info = row["projection"]
        support = {}
        for field, source in (("controller", "controllerGroup"), ("macros", "macroGroup"), ("mouse", "mouseGroup"),
                              ("scrolling", "scrollingGroup"), ("sensitivity", "sensitivityGroup"), ("profile_navigation", "profileNavigationGroup")):
            value = info.get("supportedMappings", {}).get(source)
            if value is None:
                support[field] = None
            elif type(value) is bool:
                support[field] = {"kind": "Enabled", "value": value}
            else:
                support[field] = {"kind": "Assignments", "value": [item for item in value if isinstance(item, str)]}
        expected.append([row["product_id"], {
            "category": {"MOUSE": "Mouse", "MOUSEPLUSMAT": "MousePlusMat", "KEYBOARD": "Keyboard", "SYSTEM": "System"}[info["category"]],
            "is_obm_device": info.get("isOBMDevice"), "is_xena_t2_series": info.get("isXenaT2Series"),
            "no_firmware_input_ids": info.get("noFirmwareInputId"), "supported": support,
            "ble_macros_available": info.get("bleMacrosAvailable"),
            "has_controller_assignments_without_synapse": info.get("controllerAssignmentsWithoutSynapse", False),
            "required_mouse": info.get("MOUSE_FUNC_REQUIRED_SYNAPSE_VALUES"),
            "required_turbo_mouse": info.get("MOUSE_TURBO_FUNC_REQUIRED_SYNAPSE_VALUES"),
        }])
    if actual != expected:
        raise ValueError("Complete generated mapping capability fields, product branches or order differ from current source projection")
    return {"products": len(actual), "all_fields_and_order_verified": True,
        "constructor_sha256": hashlib.sha256(content).hexdigest()}


def prepare_mapping_capabilities(*, check=False, workers=4):
    import subprocess
    import concurrent.futures
    import sys
    import urllib.parse
    from importlib.metadata import version
    import families
    import analysis
    if type(workers) is not int or not 2 <= workers <= 16:
        raise ValueError("Mapping capability workers must be between two and sixteen")
    registry = ROOT / ".analysis/inputs/typed-tables/razer-catalog/product_registry.json"
    registered = json.loads(registry.read_bytes())["products"]
    tool = families.identity(Path(__file__))
    products, inputs = [], [families.identity(registry), tool,
        families.identity(ROOT / "tools/families.py"), families.identity(ROOT / "tools/workspace.py")]
    selected_products = [product for product in registered
                         if any(category in ("MOUSE", "MOUSEPLUSMAT") for category in product["categories"])]

    # Each mounted mouse input is independent.  Workers return product records
    # and input identities; the coordinator extends both lists in registry order.
    def mapping_product(product):
        pid = product["id"]
        receipt = ROOT / f".analysis/inputs/frontend/mouse-{pid}-current/acquisition.json"
        acquired = json.loads(receipt.read_bytes())
        product_inputs = [families.identity(receipt)]
        files = {row["url"]: row for row in acquired["files"]}
        entry = files[acquired["entry_url"]]
        manifest = files[urllib.parse.urljoin(entry["url"], "manifest.json")]
        for original in (entry, manifest):
            stable = next((row for row in acquired.get("stability", []) if row["url"] == original["url"]), {})
            if (not families.valid(original) or not acquired.get("source_current_during_acquisition")
                    or not stable.get("unchanged") or stable.get("before_sha256") != original["sha256"]
                    or stable.get("after_sha256") != original["sha256"]):
                raise ValueError(f"Current mounted mouse {pid} entry/manifest does not verify")
            product_inputs.append({key: original[key] for key in ("path", "bytes", "sha256")})
        scripts = re.findall(r'<script\b[^>]*\bsrc=["\']([^"\']*\bmain\.[\w]+\.js)["\']', (ROOT / entry["path"]).read_text(encoding="utf-8"))
        if len(scripts) != 1:
            raise ValueError(f"Current mouse {pid} must mount one main bundle")
        main = files[urllib.parse.urljoin(entry["url"], scripts[0])]
        if not families.valid(main):
            raise ValueError(f"Current mouse {pid} main bytes do not verify")
        if main.get("kind") == "installed_cached_official_resource":
            attempts = main.get("current_http_revalidation", {}).get("attempts", [])
            if len(attempts) != 2 or not all(row.get("status") == 200 and row.get("matches_retained")
                    and row.get("bytes") == main["bytes"] and row.get("sha256") == main["sha256"] for row in attempts):
                raise ValueError(f"Current cached mouse {pid} main needs exact official HTTP revalidation")
        product_inputs.append({key: main[key] for key in ("path", "bytes", "sha256")})
        raw = (ROOT / main["path"]).read_bytes()
        owners = [owner for owner in families.keyboard_source_modules(raw.decode("utf-8"))
                  if isinstance(owner["module_id"], int) and "DeviceInfo:()=>" in owner["body"]]
        if len(owners) != 1:
            raise ValueError(f"Current mouse {pid} complete DeviceInfo factory is unresolved")
        owner = owners[0]
        if raw[owner["byte_start"]:owner["byte_end"]] != owner["body"].encode():
            raise ValueError("Current complete mouse factory boundaries differ")
        projection, evidence = mapping_source_projection(owner["body"])
        declared = projection["productId"]
        if type(declared) is not int or (declared != pid and (pid, declared) != (105, 104)):
            raise ValueError(f"Current mouse mounted identity differs: {pid}/{declared}")
        return {"product_id": pid, "declared_product_id": declared,
            "identity_branch": "current endpoint 105 shares declared physical DeviceInfo104" if declared != pid else "exact",
            "main": main, "complete_owner": owner, "projection": projection, "projection_evidence": evidence}, product_inputs
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(2, min(workers, len(selected_products)))) as pool:
        for product, product_inputs in pool.map(mapping_product, selected_products):
            products.append(product)
            inputs.extend(product_inputs)
    content = render_mapping_capabilities(products)
    formatted = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"], input=content, capture_output=True, check=True).stdout
    equivalence = verify_mapping_constructor(formatted, products)
    unchanged = all(families.valid(row) for row in inputs)
    if not unchanged:
        raise ValueError("Current mapping capability input bytes changed during extraction")
    output = ROOT / MAPPING_OUTPUT
    if check:
        if not output.is_file() or output.read_bytes() != formatted:
            raise ValueError("Typed current mouse mapping capabilities differ; regenerate through workspace.py")
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(formatted)
    report = {"tool": tool, "parser": {"tree_sitter": version("tree-sitter"),
        "tree_sitter_javascript": version("tree-sitter-javascript")}, "inputs": inputs,
        "products": products, "output": families.identity(output), "inputs_unchanged": unchanged,
        "product_count": len(products), "constructor_equivalence": equivalence, "fixed_rust_constructor_verified": True, "check_only": check,
        "application_execution": False, "vendor_execution": False, "runtime_acceptance": "unverified"}
    destination = ROOT / ".analysis/results/typed-data.json"
    with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
        previous["current_mouse_mapping_capabilities"] = report
        analysis.save(destination, previous)
    return {"output": MAPPING_OUTPUT, "products": len(products), "bytes": len(formatted), "passed": True, "check_only": check}


def profile_transfer_source_owners(raw):
    """Locate exact exporting factories, including complete concatenated entries."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    import families
    tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Current profile transfer source has a JavaScript AST error")

    def source(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8")

    def capture(node, module_id):
        return {"module_id": module_id, "byte_start": node.start_byte, "byte_end": node.end_byte,
                "body": source(node), "body_sha256": hashlib.sha256(raw[node.start_byte:node.end_byte]).hexdigest()}

    owners, modules, pending = {}, {}, [tree.root_node]
    while pending:
        node = pending.pop()
        if node.type in {"pair", "method_definition"}:
            key = node.child_by_field_name("key" if node.type == "pair" else "name")
            value = node.child_by_field_name("value") if node.type == "pair" else node
            if key is not None and key.type == "number" and value is not None and value.type in {
                    "arrow_function", "function_expression", "method_definition"}:
                module_id = families.javascript_integer_literal(source(key))
                if module_id in modules:
                    raise ValueError("Duplicate current transfer module identity: " + str(module_id))
                modules[module_id] = capture(node, module_id)
        if node.type == "call_expression":
            function = node.child_by_field_name("function")
            arguments = node.child_by_field_name("arguments").named_children
            if (function.type == "member_expression" and source(function.child_by_field_name("property")) == "d"
                    and len(arguments) == 2 and arguments[1].type == "object"
                    and any(pair.type == "pair" and source(pair.child_by_field_name("key")) == "DeviceInfo"
                            for pair in arguments[1].named_children)):
                owner = node.parent
                while owner is not None and owner.type not in {
                        "arrow_function", "function_expression", "method_definition", "function_declaration"}:
                    owner = owner.parent
                if owner is None:
                    raise ValueError("Current DeviceInfo export has no complete owner")
                module_id = "concatenated-entry@" + str(owner.start_byte)
                if owner.parent is not None and owner.parent.type == "pair":
                    key = owner.parent.child_by_field_name("key")
                    if key is not None and key.type == "number":
                        owner, module_id = owner.parent, families.javascript_integer_literal(source(key))
                elif owner.type == "method_definition":
                    key = owner.child_by_field_name("name")
                    if key is not None and key.type == "number":
                        module_id = families.javascript_integer_literal(source(key))
                owners[(owner.start_byte, owner.end_byte)] = capture(owner, module_id)
        pending.extend(reversed(node.named_children))
    return list(owners.values()), modules


def _profile_transfer_static_fields(body, export_name, consumed, modules, import_chain=(), *, allow_absent=False):
    """Resolve fixed exported fields or a complete value without executing JavaScript."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    import json5
    import families
    numeric_factory = re.match(r"\s*" + families.JAVASCRIPT_INTEGER_LITERAL + r"\s*(?::|\()", body) is not None
    prefix = b"const TRANSFER_MODULE={" if numeric_factory else b"const TRANSFER_MODULE=("
    raw = prefix + body.encode("utf-8") + (b"};" if numeric_factory else b");")
    tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Current profile transfer factory has a JavaScript AST error")
    container = tree.root_node.named_children[0].named_children[0].child_by_field_name("value")
    if len(container.named_children) != 1:
        raise ValueError("Current profile transfer source is not one complete factory")
    member = container.named_children[0]
    factory = member.child_by_field_name("value") if member.type == "pair" else member
    statements = factory.child_by_field_name("body")
    if statements is None or statements.type != "statement_block":
        raise ValueError("Current profile transfer factory has no complete statement body")
    bindings, exported, retained, dependencies, duplicates, sequences = {}, {}, {}, [], [], []
    parameters = factory.child_by_field_name("parameters")
    parameter_names = [raw[node.start_byte:node.end_byte].decode("utf-8") for node in parameters.named_children] if parameters else []
    require_name = parameter_names[2] if len(parameter_names) == 3 else None

    def source(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8")

    def capture(node, name):
        literal = raw[node.start_byte:node.end_byte]
        retained[(node.start_byte, node.end_byte)] = {
            "name": name, "byte_start": node.start_byte - len(prefix),
            "byte_end": node.end_byte - len(prefix), "body": literal.decode("utf-8"),
            "body_sha256": hashlib.sha256(literal).hexdigest(),
        }

    for statement in statements.named_children:
        if statement.type not in ("lexical_declaration", "variable_declaration"):
            continue
        for declaration in statement.named_children:
            if declaration.type != "variable_declarator":
                continue
            name = declaration.child_by_field_name("name")
            if name.type == "identifier":
                key = source(name)
                if key in bindings:
                    raise ValueError("Repeated profile transfer source binding: " + key)
                bindings[key] = declaration

    def find_exports(node):
        if node.type == "call_expression":
            function = node.child_by_field_name("function")
            arguments = node.child_by_field_name("arguments").named_children
            if (function.type == "member_expression"
                    and source(function.child_by_field_name("property")) == "d"
                    and len(arguments) == 2 and arguments[1].type == "object"):
                for pair in arguments[1].named_children:
                    if pair.type != "pair":
                        continue
                    key = pair.child_by_field_name("key")
                    name = json5.loads(source(key)) if key.type == "string" else source(key)
                    if name != export_name:
                        continue
                    getter = pair.child_by_field_name("value")
                    if getter.type not in ("arrow_function", "function_expression"):
                        raise ValueError("DeviceInfo export is not a fixed getter")
                    result = getter.child_by_field_name("body")
                    if result.type == "statement_block":
                        returns = [child for child in result.named_children if child.type != "comment"]
                        if (len(returns) != 1 or returns[0].type != "return_statement"
                                or len(returns[0].named_children) != 1):
                            raise ValueError("DeviceInfo getter has an unresolved statement body")
                        result = returns[0].named_children[0]
                    if result.type != "identifier" or name in exported:
                        raise ValueError("DeviceInfo export is ambiguous or dynamic")
                    exported[name] = source(result)
                    capture(pair, export_name + "_export")
        if node.type not in ("arrow_function", "function_expression", "function_declaration", "method_definition"):
            for child in node.named_children:
                find_exports(child)

    for statement in statements.named_children:
        if statement.type == "expression_statement":
            find_exports(statement)
    if not exported and allow_absent:
        return None, {"export_observed": False, "export_name": export_name,
                      "complete_factory_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest(),
                      "scope": "Export is absent from the complete parsed factory"}
    if set(exported) != {export_name}:
        raise ValueError("Current profile transfer factory does not export " + export_name)

    def resolve(node, chain=()):
        while node.type == "parenthesized_expression" and len(node.named_children) == 1:
            node = node.named_children[0]
        if node.type == "sequence_expression":
            operands = node.named_children
            def passive(operand):
                return (operand.type == "identifier" or operand.type == "member_expression"
                        and passive(operand.child_by_field_name("object"))
                        and operand.child_by_field_name("property").type == "property_identifier")
            if not operands or not all(passive(operand) for operand in operands[:-1]):
                raise ValueError("DeviceInfo sequence has an unresolved leading operation")
            sequences.append({"expression": source(node), "result": source(operands[-1])})
            return resolve(operands[-1], chain)
        if node.type not in {"identifier", "shorthand_property_identifier"}:
            return node
        name = source(node)
        if name in chain or name not in bindings:
            raise ValueError("Unresolved profile transfer source binding: " + name)
        declaration = bindings[name]
        value = declaration.child_by_field_name("value")
        if value is None:
            raise ValueError("Uninitialized profile transfer source binding: " + name)
        capture(declaration, name)
        return resolve(value, (*chain, name))

    def value(node, value_chain=()):
        node = resolve(node)
        identity = (node.start_byte, node.end_byte)
        if identity in value_chain:
            raise ValueError("Cyclic static source value: " + source(node))
        value_chain = (*value_chain, identity)
        if node.type in ("string", "number", "true", "false", "null"):
            return json5.loads(source(node))
        if node.type == "unary_expression" and source(node) in ("!0", "!1"):
            return source(node) == "!0"
        if node.type == "object":
            return {key: value(field, value_chain) for key, field in properties(node, value_chain=value_chain).items()}
        if node.type == "array":
            elements, expect_element = [], True
            for child in node.children[1:-1]:
                if child.type == "comment":
                    continue
                if child.type == ",":
                    if expect_element:
                        raise ValueError("Static source array has an unobserved element")
                    expect_element = True
                else:
                    if not expect_element or child.type == "spread_element":
                        raise ValueError("Static source array has an unresolved element")
                    elements.append(value(child, value_chain))
                    expect_element = False
            return elements
        if node.type == "member_expression":
            path, root = [], node
            while root.type == "member_expression":
                key = root.child_by_field_name("property")
                if key.type != "property_identifier":
                    raise ValueError("Computed transfer field member is unresolved")
                path.insert(0, source(key))
                root = root.child_by_field_name("object")
            if root.type == "identifier" and source(root) in bindings:
                binding = bindings[source(root)]
                imported = binding.child_by_field_name("value")
                if imported is not None and imported.type == "call_expression":
                    arguments = imported.child_by_field_name("arguments").named_children
                    if (require_name is not None and source(imported.child_by_field_name("function")) == require_name
                            and len(arguments) == 1 and arguments[0].type == "number" and len(path) == 2):
                        module_id = families.javascript_integer_literal(source(arguments[0]))
                        marker = (module_id, *path)
                        if marker in import_chain or module_id not in modules:
                            raise ValueError("Imported transfer field module is absent or cyclic: " + str(module_id))
                        capture(binding, source(root))
                        dependency = modules[module_id]
                        fields, evidence = _profile_transfer_static_fields(
                            dependency["body"], path[0], {path[1]}, modules, (*import_chain, marker))
                        dependencies.append({"owner": dependency, "export": path[0], "field": path[1], "evidence": evidence})
                        return fields[path[1]]
            key = source(node.child_by_field_name("property"))
            object_fields = properties(node.child_by_field_name("object"), observed={key}, value_chain=value_chain)
            if key not in object_fields:
                raise ValueError("Static transfer object member is absent: " + key)
            return value(object_fields[key], value_chain)
        raise ValueError("Nonliteral profile transfer field: " + source(node))

    def properties(node, spread_chain=(), *, observed=None, value_chain=()):
        node = resolve(node)
        if node.type != "object" or node.start_byte in spread_chain:
            raise ValueError("DeviceInfo must resolve to one fixed object")
        output = {}
        for item in node.named_children:
            if item.type == "comment":
                continue
            if item.type == "pair":
                key = item.child_by_field_name("key")
                if key.type not in ("property_identifier", "string", "number"):
                    raise ValueError("DeviceInfo has a computed field identity: " + source(key))
                name = (json5.loads(source(key)) if key.type == "string" else
                        str(families.javascript_integer_literal(source(key))) if key.type == "number" else source(key))
                incoming = {name: item.child_by_field_name("value")}
            elif item.type == "shorthand_property_identifier":
                incoming = {source(item): item}
            elif item.type == "spread_element" and len(item.named_children) == 1:
                incoming = properties(item.named_children[0], (*spread_chain, node.start_byte),
                                      observed=observed, value_chain=value_chain)
            else:
                raise ValueError("DeviceInfo has an unresolved object property: " + source(item))
            for key, item_value in incoming.items():
                if key in output and (observed is None or key in observed):
                    if value(output[key], value_chain) != value(item_value, value_chain):
                        raise ValueError("Conflicting profile transfer DeviceInfo field: " + key)
                    duplicates.append({"field": key, "previous": source(output[key]), "effective": source(item_value),
                                       "equal_fixed_values": True, "semantics": "last object property wins"})
                output[key] = item_value
        return output

    declaration = bindings.get(exported[export_name])
    if declaration is None:
        raise ValueError("Exported static binding is absent: " + export_name)
    capture(declaration, export_name)
    absent_optional_fields = []
    if consumed is None:
        projection = value(declaration.child_by_field_name("value"))
    else:
        fields = properties(declaration.child_by_field_name("value"), observed=consumed)
        required = {"productId", "category"} if export_name == "DeviceInfo" else consumed
        missing = required - fields.keys()
        if missing:
            raise ValueError("Required profile transfer DeviceInfo fields are absent: " + ", ".join(sorted(missing)))
        projection = {}
        for name in sorted(consumed & fields.keys()):
            node = fields[name]
            # Shorthand object keys bind the same top-level identifier.
            if node.type == "shorthand_property_identifier":
                if name not in bindings:
                    raise ValueError("Unresolved DeviceInfo shorthand: " + name)
                node = bindings[name].child_by_field_name("value")
                capture(bindings[name], name)
            capture(node, name)
            projection[name] = value(node)
        absent_optional_fields = sorted(consumed - required - fields.keys())
    return projection, {
        "export_observed": True,
        "exported_binding": exported[export_name],
        "bindings": sorted(retained.values(), key=lambda row: (row["byte_start"], row["byte_end"])),
        "absent_optional_fields": absent_optional_fields,
        "equal_duplicate_properties": duplicates, "sequence_results": sequences, "imported_fixed_fields": dependencies,
        "scope": "Exact exported fixed fields; unrelated product defaults stay with their product owners",
    }


def registry_identity_projection(body, *, modules=None):
    """Read manufacturer identity and optional presentation exports independently."""
    modules = modules or {}
    projection, evidence = _profile_transfer_static_fields(
        body, "DeviceInfo", IDENTITY_DEFINITION_FIELDS | {"deviceName"}, modules)
    validate_identity_definition({key: value for key, value in projection.items()
                                  if key in IDENTITY_DEFINITION_FIELDS})
    name = projection.get("deviceName")
    name_evidence = {"source": "DeviceInfo.deviceName", "export_observed": "deviceName" in projection}
    if "deviceName" not in projection:
        name, name_evidence = _profile_transfer_static_fields(
            body, "DEVICE_NAME", None, modules, allow_absent=True)
        name_evidence["source"] = "DEVICE_NAME"
    if name_evidence["export_observed"] and (type(name) is not str or not name.strip()):
        raise ValueError("Current registry manufacturer name is not a nonempty string")
    editions, edition_evidence = _profile_transfer_static_fields(
        body, "AVAILABLE_EDITION_ID", None, modules, allow_absent=True)
    if edition_evidence["export_observed"]:
        source_list(editions, "current registry edition IDs", True)
        for edition in editions:
            bounded(edition, 0xFFFFFFFF, "current registry edition ID")
        if len(set(editions)) != len(editions):
            raise ValueError("Current registry repeats an edition ID")
    return {"device_info": projection, "manufacturer_name": name, "edition_ids": editions}, {
        "device_info": evidence, "manufacturer_name": name_evidence, "edition_ids": edition_evidence}


def profile_transfer_source_projection(body, *, modules=None):
    """Read only the exported DeviceInfo object through its complete factory AST."""
    projection, evidence = _profile_transfer_static_fields(body, "DeviceInfo",
        {"productId", "dongleId", "bleId", "category", "is8kAnalogDevice", "deviceName"}, modules or {})
    for name in ("productId", "dongleId", "bleId"):
        if name in projection:
            bounded(projection[name], 0xFFFFFFFF, name, 1 if name == "productId" else 0)
    if projection["category"] not in TRANSFER_CATEGORIES:
        raise ValueError("Unknown current profile transfer category: " + str(projection["category"]))
    if "deviceName" in projection and (type(projection["deviceName"]) is not str or not projection["deviceName"].strip()):
        raise ValueError("Current profile transfer deviceName is empty or not a string")
    if "is8kAnalogDevice" in projection:
        boolean(projection["is8kAnalogDevice"], "is8kAnalogDevice")
    return projection, evidence


def profile_transfer_semantics(projection):
    return {
        "product_id": projection["productId"], "dongle_id": projection.get("dongleId"),
        "ble_id": projection.get("bleId"), "category": TRANSFER_CATEGORIES[projection["category"]],
        "is_eight_k_analog_device": projection.get("is8kAnalogDevice", False),
    }


def profile_export_payload_semantics(body):
    """Verify the observed writer projection and selected-row failure semantics."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    raw = b"const writer=" + body.encode("utf-8") + b";"
    tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Current export payload has an invalid complete function")
    owner = tree.root_node.named_children[0].named_children[0].child_by_field_name("value")

    def source(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8")

    def unwrapped(node):
        while node.type == "parenthesized_expression" and len(node.named_children) == 1:
            node = node.named_children[0]
        return node

    def descendants(node):
        pending = [node]
        while pending:
            current = pending.pop()
            yield current
            pending.extend(reversed(current.named_children))

    def fields(node):
        output = {}
        for pair in node.named_children:
            if pair.type == "comment":
                continue
            if pair.type != "pair" or pair.child_by_field_name("key").type != "property_identifier":
                raise ValueError("Export payload has an unresolved object property")
            key = source(pair.child_by_field_name("key"))
            if key in output:
                raise ValueError("Export payload has a repeated field: " + key)
            output[key] = pair.child_by_field_name("value")
        return output

    nodes = list(descendants(owner))
    files = [fields(node) for node in nodes if node.type == "object"
             and re.search(r"\bproductId\s*:", source(node))
             and {"productId", "deviceName", "category", "profiles"} <= fields(node).keys()]
    if len(files) != 1:
        raise ValueError("Export payload file object is unresolved")
    document = files[0]
    has_macros = "macros" in document
    exact_fields(document, {"productId", "deviceName", "category", "profiles", *(('macros',) if has_macros else ())}, "export file")
    if document["profiles"].type != "identifier" or has_macros and document["macros"].type != "identifier":
        raise ValueError("Export arrays must have explicit local owners")
    profile_array = source(document["profiles"])
    macro_array = source(document["macros"]) if has_macros else None
    copies = [node for node in nodes if node.type == "variable_declarator"
              and node.child_by_field_name("value") is not None
              and re.fullmatch(r"JSON\.parse\(JSON\.stringify\([\w$]+\.find\(.+\)\)\)", source(node.child_by_field_name("value")))]
    if len(copies) != 1:
        raise ValueError("Export selected profile must be cloned from exactly one GUID lookup")
    profile_copy = copies[0]
    profile = source(profile_copy.child_by_field_name("name"))
    deletes = [source(node) for node in nodes if node.type == "unary_expression" and source(node).startswith("delete ")]
    if deletes != ["delete " + profile + ".appEngine"]:
        raise ValueError("Export profile must remove only appEngine")
    callback = profile_copy.parent
    while callback is not None and callback.type != "arrow_function":
        callback = callback.parent
    if callback is None:
        raise ValueError("Export selected profile has no row callback")
    parameters = callback.child_by_field_name("parameters")
    parameter = callback.child_by_field_name("parameter")
    if parameter is None and parameters is not None and len(parameters.named_children) == 1:
        parameter = parameters.named_children[0]
    if parameter is None or parameter.type != "identifier":
        raise ValueError("Export selected profile callback must have one row identity")
    row = source(parameter)
    copy_text = source(profile_copy.child_by_field_name("value"))
    if re.search(r"([\w$]+)\.guid===" + re.escape(row) + r"\.id", copy_text) is None:
        raise ValueError("Export selected profile lookup does not use the row GUID")
    profile_iteration = callback.parent
    while profile_iteration is not None and profile_iteration.type != "call_expression":
        profile_iteration = profile_iteration.parent
    if profile_iteration is None or re.fullmatch(r"[\w$]+\.forEach", source(profile_iteration.child_by_field_name("function"))) is None:
        raise ValueError("Export profile selection must preserve input forEach order")
    for node in descendants(callback):
        if node.type in {"assignment_expression", "augmented_assignment_expression", "update_expression"}:
            target = node.child_by_field_name("left") or node.child_by_field_name("argument")
            if target is not None and re.match(re.escape(profile) + r"(?:\.|\[|$)", source(target)):
                raise ValueError("Export cloned profile has an unverified mutation")

    def envelope(array, value_name, display_owner):
        pushes = [node for node in nodes if node.type == "call_expression"
                  and source(node.child_by_field_name("function")) == array + ".push"]
        if len(pushes) != 1:
            raise ValueError("Export payload array must have one ordered envelope push")
        args = pushes[0].child_by_field_name("arguments").named_children
        if len(args) != 1 or args[0].type != "object":
            raise ValueError("Export envelope is not a fixed object")
        projected = fields(args[0])
        exact_fields(projected, {"name", "payload", "hash"}, "export envelope")
        if source(projected["name"]) != display_owner + ".name" or projected["payload"].type != "identifier":
            raise ValueError("Export envelope name/payload owner differs")
        hash_call = projected["hash"]
        if (hash_call.type != "call_expression" or [source(arg) for arg in hash_call.child_by_field_name("arguments").named_children] != [value_name]):
            raise ValueError("Export hash must consume the exact projected value")
        scope = pushes[0].parent
        while scope is not None and scope.type != "statement_block":
            scope = scope.parent
        declarations = [node for statement in scope.named_children
            if statement.type in {"lexical_declaration", "variable_declaration"}
            for node in statement.named_children if node.type == "variable_declarator"]
        values = {source(node.child_by_field_name("name")): source(node.child_by_field_name("value")) for node in declarations}
        serialized = [key for key, value in values.items() if value == "JSON.stringify(" + value_name + ")"]
        payload = source(projected["payload"])
        if len(serialized) != 1 or payload not in values or re.fullmatch(
                r"[\w$]+(?:\.Buffer)?\.from\(" + re.escape(serialized[0]) + r"\)\.toString\([\"']base64[\"']\)", values[payload]) is None:
            raise ValueError("Export payload must base64-encode the exact JSON projection")
        return {"push": source(pushes[0]), "hash_expression": source(hash_call.child_by_field_name("function"))}

    profile_envelope = envelope(profile_array, profile, profile)
    macro_projections = [(node, fields(node)) for node in nodes if node.type == "object"
                         and {"guid", "macroEvents", "macroPlaybackOption"} <= fields(node).keys()]
    if len(macro_projections) != (1 if has_macros else 0):
        raise ValueError("Export macro projection count differs from the file schema")
    result = {"export_rule": "ProfilesOnly", "profile": {"copy": source(profile_copy),
        "delete": deletes[0], "selection": source(profile_iteration), "envelope": profile_envelope},
        "missing_profile": "fail entire export", "macro": None}
    if has_macros:
        projection, projected = macro_projections[0]
        exact_fields(projected, {"guid", "macroEvents", "macroPlaybackOption"}, "export macro")
        match = re.fullmatch(r"([\w$]+)\.guid", source(projected["guid"]))
        if match is None or any(source(projected[key]) != match[1] + "." + key for key in projected):
            raise ValueError("Export macro fields do not share their original owner")
        macro = match[1]
        declaration = projection.parent
        if declaration.type != "variable_declarator":
            raise ValueError("Export macro projection has no stable local binding")
        projected_name = source(declaration.child_by_field_name("name"))
        macro_envelope = envelope(macro_array, projected_name, macro)
        unique_if = declaration.parent
        while unique_if is not None and unique_if.type != "if_statement":
            unique_if = unique_if.parent
        if unique_if is None:
            raise ValueError("Export macro projection has no duplicate guard")
        unique_condition = unwrapped(unique_if.child_by_field_name("condition"))
        # Compare grammar tokens after removing optional parenthesized arrows.
        dedup_text = re.sub(r"\s+", "", source(unique_condition)).replace("(", "").replace(")", "")
        unique_suffix = re.escape(macro_array) + r"\.findIndex([\w$]+)=>\1\.guid===" + re.escape(macro) + r"\.guid"
        if re.fullmatch(r"(?:" + re.escape(macro) + r"&&)?-1===" + unique_suffix, dedup_text) is None:
            raise ValueError("Export macros must be deduplicated by original GUID")
        combined_skip = dedup_text.startswith(macro + "&&")
        siblings = unique_if.parent.named_children
        preceding = [node for node in siblings if node.end_byte <= unique_if.start_byte]
        separate_skip = any(node.type == "if_statement" and re.sub(r"\s+", "", source(node)) == "if(!" + macro + ")return;" for node in preceding)
        selected_if = unique_if.parent
        while selected_if is not None and selected_if.type != "if_statement":
            selected_if = selected_if.parent
        if selected_if is None:
            raise ValueError("Export macro selection guard is absent")
        selected = source(unwrapped(selected_if.child_by_field_name("condition")))
        selected_match = re.fullmatch(r"([\w$]+)\.isMacroSelected", selected)
        if selected_match is None:
            raise ValueError("Export macros do not use the selected row flag")
        selected_row = selected_match[1]
        lookup = [node for node in preceding if node.type in {"lexical_declaration", "variable_declaration"}
                  for node in node.named_children if node.type == "variable_declarator"
                  and source(node.child_by_field_name("name")) == macro]
        if len(lookup) != 1 or re.fullmatch(r"[\w$]+\.find\(\(?([\w$]+)=>\1\.guid===" + re.escape(selected_row) + r"\.guid\)?\)", source(lookup[0].child_by_field_name("value"))) is None:
            raise ValueError("Export selected macro lookup is not the original GUID")
        macro_iteration = selected_if.parent
        while macro_iteration is not None and macro_iteration.type != "arrow_function":
            macro_iteration = macro_iteration.parent
        macro_iteration = macro_iteration.parent if macro_iteration is not None else None
        while macro_iteration is not None and macro_iteration.type != "call_expression":
            macro_iteration = macro_iteration.parent
        if macro_iteration is None or source(macro_iteration.child_by_field_name("function")) != row + ".listItems.forEach":
            raise ValueError("Export macros must follow each selected profile's row order")
        skip = combined_skip or separate_skip
        result["export_rule"] = "ProfilesWithSelectedMacros" if skip else "ProfilesWithRequiredSelectedMacros"
        result["macro"] = {"projection": {key: source(value) for key, value in projected.items()},
            "selection": source(macro_iteration), "envelope": macro_envelope,
            "missing_selected_macro": "skip" if skip else "fail entire export", "deduplication": "original GUID"}
    return result


def profile_export_name_source(raw, *, device_info_owner=None, device_info_binding=None, verify_payload=False):
    """Retain complete export functions and classify their exact device-name producer."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    import families
    tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Current profile export source has a JavaScript AST error")

    def source(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8")

    function_kinds = {"function_declaration", "function_expression", "arrow_function", "generator_function", "generator_function_declaration"}
    candidates, pending = {}, [tree.root_node]
    while pending:
        node = pending.pop()
        if node.type == "string" and source(node) in ('"taskMakerExportProfile"', "'taskMakerExportProfile'"):
            owner = node.parent
            while owner is not None:
                if owner.type in function_kinds:
                    candidates[(owner.start_byte, owner.end_byte)] = owner
                owner = owner.parent
        pending.extend(reversed(node.named_children))
    captures = []
    accepted_ranges = []
    for owner in sorted(candidates.values(), key=lambda node: node.end_byte - node.start_byte):
        if any(owner.start_byte <= begin and end <= owner.end_byte for begin, end in accepted_ranges):
            continue
        pending, exports = [owner], []
        while pending:
            node = pending.pop()
            if node.type == "object":
                fields = {}
                for child in node.named_children:
                    if child.type == "pair":
                        key = child.child_by_field_name("key")
                        if key.type == "property_identifier":
                            fields[source(key)] = child.child_by_field_name("value")
                if {"productId", "deviceName", "category", "profiles"} <= fields.keys():
                    exports.append(fields)
            pending.extend(reversed(node.named_children))
        if len(exports) != 1:
            continue
        fields = exports[0]
        outer_fields = {"productId", "deviceName", "category", "profiles"}
        if fields.keys() not in (outer_fields, outer_fields | {"macros"}):
            raise ValueError("Current profile export file has unresolved fields: " + str(sorted(fields)))
        export_rule = "ProfilesWithSelectedMacros" if "macros" in fields else "ProfilesOnly"
        name = source(fields["deviceName"])
        product, category = source(fields["productId"]), source(fields["category"])
        direct = re.fullmatch(r"([\w$]+(?:\.DeviceInfo)?)\.productId", product)
        if direct is None or category != direct[1] + ".category":
            raise ValueError("Current export product/category do not share their DeviceInfo owner")
        product_owner = direct[1]
        if name == product_owner + ".deviceName":
            mode = "middleware_device_info"
        elif re.fullmatch(r"[\w$]+(?:\.[\w$]+)*\.getCurLangNameData\(\)\.productName", name):
            mode = "current_localized_product_name"
        else:
            raise ValueError("Unresolved current profile export device-name producer: " + name)
        pending, deletions, macro_payloads, profile_copies = [owner], [], [], []
        while pending:
            node = pending.pop()
            if node.type == "unary_expression" and source(node).startswith("delete "):
                deletions.append(source(node))
            if node.type == "variable_declarator":
                initializer = node.child_by_field_name("value")
                if initializer is not None and re.fullmatch(r"JSON\.parse\(JSON\.stringify\(.+\.find\(.+\)\)\)", source(initializer)):
                    profile_copies.append(source(node))
            if node.type == "object":
                projected = {source(pair.child_by_field_name("key")): source(pair.child_by_field_name("value"))
                    for pair in node.named_children if pair.type == "pair"}
                if {"guid", "macroEvents", "macroPlaybackOption"} <= projected.keys():
                    macro_payloads.append(projected)
            pending.extend(reversed(node.named_children))
        identifiers, properties = {}, {}
        def normalized(node):
            if node.type == "comment":
                return None
            if node.type == "identifier":
                key = source(node)
                return [node.type, identifiers.setdefault(key, len(identifiers))]
            if node.type == "property_identifier":
                key = source(node)
                # Retain domain fields, built-in operations and enum members;
                # alpha-renaming only short minified import/export aliases.
                return [node.type, properties.setdefault(key, len(properties)) if len(key) <= 3 and key not in {"get", "set", "id"} else key]
            if not node.children:
                return [node.type, source(node)]
            return [node.type, [item for child in node.children if (item := normalized(child)) is not None]]
        structure = json.dumps(normalized(owner), separators=(",", ":")).encode()
        verified_payload = profile_export_payload_semantics(source(owner)) if verify_payload else None
        if verified_payload is not None:
            export_rule = verified_payload["export_rule"]
        module, enclosing_functions = owner.parent, []
        while module is not None:
            if module.type in ("pair", "method_definition"):
                key = module.child_by_field_name("key" if module.type == "pair" else "name")
                if key is not None and key.type == "number":
                    break
            if module.type in function_kinds:
                enclosing_functions.append(module)
            module = module.parent
        if module is None:
            matching = [node for node in enclosing_functions if device_info_owner is not None
                        and hashlib.sha256(raw[node.start_byte:node.end_byte]).hexdigest() == device_info_owner["body_sha256"]]
            if len(matching) != 1:
                raise ValueError("Current profile export function is outside a complete verified owner")
            module = matching[0]
            module_id = device_info_owner["module_id"]
        else:
            module_id = families.javascript_integer_literal(source(module.child_by_field_name("key" if module.type == "pair" else "name")))
        linkage = None
        if device_info_owner is not None:
            if product_owner.endswith(".DeviceInfo"):
                factory = module.child_by_field_name("value") if module.type == "pair" else module
                parameters = factory.child_by_field_name("parameters").named_children
                if len(parameters) != 3 or not isinstance(device_info_owner["module_id"], int):
                    raise ValueError("Current export DeviceInfo import has no verified module boundary")
                require_name = source(parameters[2])
                alias = product_owner.removesuffix(".DeviceInfo")
                declarations = [declaration for statement in factory.child_by_field_name("body").named_children
                    if statement.type in {"variable_declaration", "lexical_declaration"}
                    for declaration in statement.named_children if declaration.type == "variable_declarator"
                    and source(declaration.child_by_field_name("name")) == alias]
                if len(declarations) != 1:
                    raise ValueError("Current export DeviceInfo import binding is unresolved")
                imported = declarations[0].child_by_field_name("value")
                arguments = imported.child_by_field_name("arguments") if imported.type == "call_expression" else None
                if (arguments is None or source(imported.child_by_field_name("function")) != require_name
                        or len(arguments.named_children) != 1 or arguments.named_children[0].type != "number"
                        or families.javascript_integer_literal(source(arguments.named_children[0])) != device_info_owner["module_id"]):
                    raise ValueError("Current export DeviceInfo import does not resolve to the verified writer")
                linkage = {"kind": "webpack_require", "module_id": device_info_owner["module_id"], "binding": source(declarations[0])}
            elif (product_owner == device_info_binding and module_id == device_info_owner["module_id"]
                    and hashlib.sha256(raw[module.start_byte:module.end_byte]).hexdigest() == device_info_owner["body_sha256"]):
                linkage = {"kind": "same_complete_owner", "binding": product_owner}
            else:
                raise ValueError("Current export DeviceInfo binding does not resolve to the verified writer")
        accepted_ranges.append((owner.start_byte, owner.end_byte))
        captures.append({"name_source": mode, "device_name_expression": name,
            "export_rule": export_rule, "file_fields": sorted(fields),
            "verified_payload_semantics": verified_payload,
            "processor_observation": {"deletions": deletions, "profile_copies": profile_copies,
                "macro_payloads": macro_payloads, "alpha_renamed_ast_sha256": hashlib.sha256(structure).hexdigest(),
                "scope": "Complete writer function structure and observed operations; helper call equivalence is not implied"},
            "product_expression": product, "category_expression": category,
            "verified_device_info_linkage": linkage,
            "function": {"byte_start": owner.start_byte, "byte_end": owner.end_byte,
                         "body": source(owner), "sha256": hashlib.sha256(raw[owner.start_byte:owner.end_byte]).hexdigest()},
            "module": {"byte_start": module.start_byte, "byte_end": module.end_byte,
                       "module_id": module_id,
                       "complete_body_retained_in_original": True,
                       "sha256": hashlib.sha256(raw[module.start_byte:module.end_byte]).hexdigest()}})
    if len(captures) > 1:
        raise ValueError("Current profile export has ambiguous name producers")
    return captures[0] if captures else None


def render_profile_transfer_products(products):
    ids = [row["mounted_product_id"] for row in products]
    if not ids or ids != sorted(set(ids)):
        raise ValueError("Current profile transfer products must be complete, unique and sorted")
    lines = [
        "//! Fixed capabilities from current mounted DeviceInfo factories.",
        "//! Regenerate with tools/workspace.py prepare-profile-transfer-table.",
        "use super::{ProfileExportRule, ProfileTransferProduct, TransferCategory, TransferTarget};", "",
        "pub(super) static PRODUCTS: &[ProfileTransferProduct] = &[",
    ]
    for row in products:
        lines.extend([
            "    ProfileTransferProduct {",
            f"        mounted_product_id: {row['mounted_product_id']},",
        ])
        for field, projection in (("target", row["projection"]), ("export_target", row["export_projection"])):
            target = profile_transfer_semantics(projection)
            lines.extend([
                f"        {field}: TransferTarget {{",
                f"            product_id: {target['product_id']},",
                f"            dongle_id: {'None' if target['dongle_id'] is None else 'Some(' + str(target['dongle_id']) + ')'},",
                f"            ble_id: {'None' if target['ble_id'] is None else 'Some(' + str(target['ble_id']) + ')'},",
                f"            category: TransferCategory::{target['category']},",
                f"            is_eight_k_analog_device: {str(target['is_eight_k_analog_device']).lower()},",
                "        },",
            ])
        lines.extend([
            f"        export_rule: ProfileExportRule::{row['export_rule']},",
            f"        device_name: {'None' if row.get('fixed_export_name') is None else 'Some(' + rust_string(row['fixed_export_name']) + ')'},",
            "    },",
        ])
    return ("\n".join([*lines, "];", ""])).encode("utf-8")


def verify_profile_transfer_constructor(content, products):
    """Compare every generated Rust field and row position with the original AST projection."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_rust
    import catalog_data
    parser = Parser(Language(tree_sitter_rust.language()))
    tree = parser.parse(content)
    if tree.root_node.has_error:
        raise ValueError("Generated profile transfer Rust has an AST error")

    def text(node):
        return node.text.decode("utf-8")

    def decode(node):
        if node.type == "reference_expression":
            if len(node.named_children) != 1:
                raise ValueError("Generated transfer table reference is ambiguous")
            return decode(node.named_children[0])
        if node.type == "array_expression":
            return [decode(child) for child in node.named_children]
        if node.type in ("integer_literal", "boolean_literal"):
            return json.loads(text(node))
        if node.type == "string_literal":
            return catalog_data.unstring(text(node))
        if node.type == "identifier" and text(node) == "None":
            return None
        if node.type == "call_expression" and text(node.child_by_field_name("function")) == "Some":
            arguments = node.child_by_field_name("arguments").named_children
            if len(arguments) == 1:
                return decode(arguments[0])
        if node.type == "scoped_identifier" and text(node).startswith("TransferCategory::"):
            result = text(node).removeprefix("TransferCategory::")
            if result in TRANSFER_CATEGORIES.values():
                return result
        if node.type == "scoped_identifier" and text(node) in {
                "ProfileExportRule::ProfilesOnly", "ProfileExportRule::ProfilesWithSelectedMacros",
                "ProfileExportRule::ProfilesWithRequiredSelectedMacros"}:
            return text(node).removeprefix("ProfileExportRule::")
        if node.type == "struct_expression":
            kind = text(node.child_by_field_name("name"))
            if kind not in {"ProfileTransferProduct", "TransferTarget"}:
                raise ValueError("Unexpected generated transfer struct: " + kind)
            fields = node.child_by_field_name("body")
            result = {}
            for field in fields.named_children:
                if field.type != "field_initializer":
                    raise ValueError("Generated transfer fields must be explicit")
                name = text(field.child_by_field_name("field"))
                if name in result:
                    raise ValueError("Duplicate generated transfer field: " + name)
                result[name] = decode(field.child_by_field_name("value"))
            expected = ({"mounted_product_id", "target", "export_target", "export_rule", "device_name"} if kind == "ProfileTransferProduct" else
                        {"product_id", "dongle_id", "ble_id", "category", "is_eight_k_analog_device"})
            exact_fields(result, expected, kind)
            return result
        raise ValueError("Nonliteral generated transfer constructor: " + node.type)

    tables = [node for node in tree.root_node.named_children if node.type == "static_item"
              and text(node.child_by_field_name("name")) == "PRODUCTS"]
    if len(tables) != 1:
        raise ValueError("Generated transfer table must have one PRODUCTS owner")
    actual = decode(tables[0].child_by_field_name("value"))
    expected = [{"mounted_product_id": row["mounted_product_id"],
                 "target": profile_transfer_semantics(row["projection"]),
                 "export_target": profile_transfer_semantics(row["export_projection"]),
                 "export_rule": row["export_rule"],
                 "device_name": row.get("fixed_export_name")} for row in products]
    if actual != expected:
        raise ValueError("Generated profile transfer fields, order or product branches differ from current source")
    return {"products": len(actual), "fields_per_product": 13, "all_fields_and_order_equal": True}


def profile_transfer_file_inputs(original):
    import families
    if not families.valid(original):
        raise ValueError("Current profile transfer original bytes do not verify")
    inputs = [{key: original[key] for key in ("path", "bytes", "sha256")}]
    if original.get("kind") == "installed_cached_official_resource":
        attempts = original.get("current_http_revalidation", {}).get("attempts", [])
        if len(attempts) != 2 or not all(row.get("status") == 200 and row.get("matches_retained")
                and row.get("bytes") == original["bytes"] and row.get("sha256") == original["sha256"]
                and families.valid(row) for row in attempts):
            raise ValueError("Current cached transfer original lacks exact retained official HTTP revalidation")
        inputs.extend({key: row[key] for key in ("path", "bytes", "sha256")} for row in attempts)
    return inputs


def _capture_profile_transfer_product(item):
    import urllib.parse
    import families
    pid, acquisitions = item
    inputs = []
    captures = []
    for role in ("ui", "mw"):
        receipt_path, receipt = acquisitions[(pid, role)]
        receipt_identity = families.identity(receipt_path)
        inputs.append(receipt_identity)
        files = {row["url"]: row for row in receipt["files"]}
        entry = files.get(receipt["entry_url"])
        if entry is None:
            raise ValueError(f"Current PID {pid} {role} entry is absent from its acquisition")
        manifest = files.get(urllib.parse.urljoin(entry["url"], "manifest.json"))
        if manifest is None:
            raise ValueError(f"Current PID {pid} {role} manifest is absent from its acquisition")
        for original in (entry, manifest):
            stable = next((row for row in receipt.get("stability", []) if row["url"] == original["url"]), {})
            if (not families.valid(original) or not receipt.get("source_current_during_acquisition")
                    or not stable.get("unchanged") or stable.get("before_sha256") != original["sha256"]
                    or stable.get("after_sha256") != original["sha256"]):
                raise ValueError(f"Current PID {pid} {role} entry/manifest bytes or deployment do not verify")
            inputs.append({key: original[key] for key in ("path", "bytes", "sha256")})
        scripts = re.findall(r'<script\b[^>]*\bsrc=["\']([^"\']*\bmain\.[\w]+\.js)["\']',
                             (ROOT / entry["path"]).read_text(encoding="utf-8"))
        if len(scripts) != 1:
            raise ValueError(f"Current PID {pid} {role} must mount one main bundle")
        main = files.get(urllib.parse.urljoin(entry["url"], scripts[0]))
        if main is None or not families.valid(main):
            raise ValueError(f"Current PID {pid} {role} mounted main bytes do not verify")
        inputs.extend(profile_transfer_file_inputs(main))
        raw = (ROOT / main["path"]).read_bytes()
        owners, modules = profile_transfer_source_owners(raw)
        if len(owners) != 1:
            raise ValueError(f"Current PID {pid} {role} complete DeviceInfo factory is unresolved")
        owner = owners[0]
        if raw[owner["byte_start"]:owner["byte_end"]] != owner["body"].encode("utf-8"):
            raise ValueError("Complete DeviceInfo factory boundaries differ from current original bytes")
        try:
            projection, evidence = profile_transfer_source_projection(owner["body"], modules=modules)
        except ValueError as error:
            raise ValueError(f"Current PID {pid} {role}: {error}") from error
        captures.append({"role": role, "acquisition": receipt_identity,
            "entry": entry, "manifest": manifest, "original_main": main,
            "complete_owner": owner, "projection": projection, "projection_evidence": evidence})
        if role == "mw":
            exports = []
            for original in receipt["files"]:
                if ("path" not in original or not original["path"].endswith(".js")
                        or f"/synapse/products/{pid}/mw/" not in original["path"]):
                    continue
                export_raw = (ROOT / original["path"]).read_bytes()
                if b'"taskMakerExportProfile"' not in export_raw and b"'taskMakerExportProfile'" not in export_raw:
                    continue
                if len(export_raw) != original["bytes"] or hashlib.sha256(export_raw).hexdigest() != original["sha256"]:
                    raise ValueError(f"Current PID {pid} export source bytes do not verify")
                export = profile_export_name_source(export_raw, device_info_owner=owner,
                        device_info_binding=evidence["exported_binding"], verify_payload=True)
                if export is not None:
                    inputs.extend(profile_transfer_file_inputs(original))
                    exports.append({"original": original, **export})
            if len(exports) != 1:
                raise ValueError(f"Current PID {pid} profile export name source is absent or ambiguous")
            captures[-1]["profile_export"] = exports[0]
    reader, writer = captures[0]["projection"], captures[1]["projection"]
    export = captures[1]["profile_export"]
    fixed_name = writer.get("deviceName") if export["name_source"] == "middleware_device_info" else None
    if export["name_source"] == "middleware_device_info" and fixed_name is None:
        raise ValueError(f"Current PID {pid} export reads an absent DeviceInfo deviceName")
    product = {"mounted_product_id": pid, "projection": reader, "export_projection": writer, "sources": captures,
        "fixed_export_name": fixed_name,
        "export_rule": export["export_rule"],
        "device_name_owner": export["name_source"],
        "ui_middleware_target_differs": profile_transfer_semantics(reader) != profile_transfer_semantics(writer),
        "ui_device_name_differs": reader.get("deviceName") != writer.get("deviceName")}
    return product, inputs


def prepare_profile_transfer_table(*, check=False, workers=4):
    import subprocess
    import urllib.parse
    from importlib.metadata import version
    import analysis
    import families
    registry_path = ROOT / ".analysis/inputs/typed-tables/razer-catalog/product_registry.json"
    registered = {row["id"] for row in json.loads(registry_path.read_bytes())["products"]}
    acquisitions = {}
    for path in sorted((ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json")):
        document = json.loads(path.read_bytes(), object_pairs_hook=duplicate_free)
        match = re.fullmatch(r"https://apps\.razer\.com/synapse/products/(\d+)/(ui|mw)/(?:index\.html)?", document.get("entry_url", ""))
        if match is None:
            continue
        key = (int(match[1]), match[2])
        if key in acquisitions:
            raise ValueError("Duplicate mounted profile transfer acquisition: " + str(key))
        acquisitions[key] = (path, document)
    mounted = {pid for pid, role in acquisitions if role == "ui"}
    if not registered <= mounted or mounted != {pid for pid, role in acquisitions if role == "mw"}:
        raise ValueError("Current profile transfer UI/MW acquisition coverage is incomplete")
    inputs = [families.identity(registry_path), families.identity(Path(__file__)),
              families.identity(ROOT / "tools/families.py"), families.identity(ROOT / "tools/workspace.py")]
    import concurrent.futures
    if type(workers) is not int or not 2 <= workers <= 16:
        raise ValueError("Profile transfer workers must be between two and sixteen")
    items = [(pid, {key: acquisitions[key] for key in ((pid, "ui"), (pid, "mw"))}) for pid in sorted(mounted)]
    products, failures = [], []
    completed_results = {}
    failed_results = {}
    import sys
    # Keep even a one-product batch on the same bounded thread-pool path.  The
    # worker count is a policy boundary, while the coordinator owns ordered
    # publication and failure aggregation.
    pool_workers = max(2, min(workers, max(2, len(items))))
    with concurrent.futures.ThreadPoolExecutor(max_workers=pool_workers) as pool:
        futures = {pool.submit(_capture_profile_transfer_product, item): item[0] for item in items}
        for completed, future in enumerate(concurrent.futures.as_completed(futures), 1):
            try:
                product, source_inputs = future.result()
                completed_results[futures[future]] = (product, source_inputs)
            except Exception as error:
                failed_results[futures[future]] = {"mounted_product_id": futures[future], "error": str(error)}
            if completed % 32 == 0 or completed == len(items):
                print(f"Profile transfer sources: {completed}/{len(items)} products; unresolved: {len(failed_results)}", file=sys.stderr, flush=True)
    for product_id in sorted(completed_results):
        product, source_inputs = completed_results[product_id]
        products.append(product)
        inputs.extend(source_inputs)
    failures.extend(failed_results[product_id] for product_id in sorted(failed_results))
    analysis.save(ROOT / ".work/profile-transfer-source-captures.json", {
        "tool": families.identity(Path(__file__)), "products": products, "inputs": inputs,
        "failures": failures, "application_execution": False, "vendor_execution": False})
    if failures:
        failures.sort(key=lambda row: row["mounted_product_id"])
        scratch = ROOT / ".work/profile-transfer-extraction-errors.json"
        scratch.parent.mkdir(parents=True, exist_ok=True)
        analysis.save(scratch, {"tool": families.identity(Path(__file__)), "failures": failures,
                                "application_execution": False, "vendor_execution": False})
        raise ValueError("Current profile transfer source chains are unresolved: " + json.dumps(failures))
    products.sort(key=lambda row: row["mounted_product_id"])
    content = render_profile_transfer_products(products)
    formatted = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
                               input=content, capture_output=True, check=True).stdout
    equivalence = verify_profile_transfer_constructor(formatted, products)
    if not all(families.valid(row) for row in inputs):
        raise ValueError("Current profile transfer input bytes changed during extraction")
    output = ROOT / TRANSFER_OUTPUT
    if check:
        if not output.is_file() or output.read_bytes() != formatted:
            raise ValueError("Typed profile transfer products differ; regenerate through workspace.py")
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(formatted)
    report = {
        "tool": families.identity(Path(__file__)),
        "parsers": {"tree_sitter": version("tree-sitter"), "tree_sitter_javascript": version("tree-sitter-javascript"),
                    "tree_sitter_rust": version("tree-sitter-rust")},
        "inputs": inputs, "products": products, "output": families.identity(output),
        "product_count": len(products), "current_registered_products": len(registered),
        "ui_middleware_target_difference_count": sum(row["ui_middleware_target_differs"] for row in products),
        "export_rules": {rule: [row["mounted_product_id"] for row in products if row["export_rule"] == rule]
            for rule in ("ProfilesOnly", "ProfilesWithSelectedMacros", "ProfilesWithRequiredSelectedMacros")},
        "writer_structure_groups": [
            {"alpha_renamed_ast_sha256": key,
             "mounted_product_ids": [row["mounted_product_id"] for row in products
                if row["sources"][1]["profile_export"]["processor_observation"]["alpha_renamed_ast_sha256"] == key],
             "payload_projection_verified_for_every_product": True}
            for key in sorted({row["sources"][1]["profile_export"]["processor_observation"]["alpha_renamed_ast_sha256"] for row in products})],
        "constructor_equivalence": equivalence, "inputs_unchanged": True, "check_only": check,
        "source_scope": "All retained current official product UI and MW acquisitions; exact mounted main DeviceInfo factories",
        "application_execution": False, "vendor_execution": False, "runtime_acceptance": "unverified",
    }
    destination = ROOT / ".analysis/results/typed-data.json"
    with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
        previous["current_profile_transfer_capabilities"] = report
        analysis.save(destination, previous)
    return {"output": TRANSFER_OUTPUT, "products": len(products), "bytes": len(formatted),
            "passed": True, "check_only": check}


def receiver_battery_peer_default_effect(quick_effects):
    """Convert the mounted MW profile default independently of device exit state."""
    if not isinstance(quick_effects, dict) or type(quick_effects.get("selectedEffectId")) is not int:
        raise ValueError("Battery peer profile quick effect is unresolved")
    if quick_effects == {"selectedEffectId": 3}:
        return "PeerProfileEffect::Spectrum"
    if quick_effects.get("selectedEffectId") == 1:
        exact_fields(quick_effects, ("selectedEffectId", "selectedEffectSetting"), "peer profile effect")
        settings = quick_effects["selectedEffectSetting"]
        exact_fields(settings, ("color1",), "peer profile effect settings")
        if isinstance(settings["color1"], str) and re.fullmatch(r"#[0-9a-fA-F]{6}", settings["color1"]):
            return {"_type": "PeerProfileEffect::Static", "color": list(bytes.fromhex(settings["color1"][1:]))}
    raise ValueError("Battery peer profile quick effect has no current typed conversion")


def receiver_battery_peer_projection(document, source_product_id, edition, profile_quick_effect):
    """Project the actual manifest branch, including its distinct declared PID."""
    required = {"productId", "vendorId", "wiredProductId", "dongleProductId", "category",
                "rzDeviceConfig", "supportBatteryLevelMonitoring", "ledConfig", "quickEffectOnExit"}
    if not isinstance(document, dict) or not required <= document.keys():
        raise ValueError("Battery peer manifest is missing required fields")
    if document["category"] != "MOUSE" or document["vendorId"] != 5426:
        raise ValueError("Battery peer category or vendor is unresolved")
    config = document["rzDeviceConfig"]
    exact_fields(config, ("claimInterface", "maxRetryIn", "maxRetryOut", "rzDeviceName",
                         "sleepTimeBetweenIn", "sleepTimeBetweenOut", "sleepTimeBetweenOutIn"), "peer transport")
    if config["rzDeviceName"] not in BATTERY_PEER_FACTORIES:
        raise ValueError("Battery peer factory has no typed branch")
    support = {"source_product_id": bounded(source_product_id, 65535, "source PID", 1),
               "manifest_product_id": bounded(document["productId"], 65535, "manifest PID", 1),
               "edition": bounded(edition, 255, "edition"),
               "vendor_id": bounded(document["vendorId"], 65535, "vendor", 1),
               "wired_product_id": bounded(document["wiredProductId"], 65535, "wired PID"),
               "dongle_product_id": bounded(document["dongleProductId"], 65535, "dongle PID"),
               "ble_product_id": bounded(document["bleProductId"], 65535, "BLE PID") if "bleProductId" in document else None,
               "category": "PeerCategory::Mouse",
               "rz_device_name": "PeerDeviceFactory::" + BATTERY_PEER_FACTORIES[config["rzDeviceName"]]}
    for source, target, maximum in (("claimInterface", "claim_interface", 255),
            ("maxRetryIn", "max_retry_in", 255), ("maxRetryOut", "max_retry_out", 255),
            ("sleepTimeBetweenIn", "sleep_time_between_in_ms", 65535),
            ("sleepTimeBetweenOut", "sleep_time_between_out_ms", 65535),
            ("sleepTimeBetweenOutIn", "sleep_time_between_out_in_ms", 65535)):
        support[target] = bounded(config[source], maximum, source)
    for source, target in (("supportBatteryLevelMonitoring", "support_battery_monitoring"),
            ("supportDockingStation", "support_docking_station"), ("supportDockingStationV2", "support_docking_v2"),
            ("isDualLink", "is_dual_link"), ("skipRegionsForChargingEffect", "skip_regions_for_charging_effect"),
            ("useParentContainerIdToSetDeviceHandle", "use_parent_container_id_to_set_device_handle")):
        boolean(document.get(source, False), source)
        support[target] = document.get(source, False)
    for source, target in (("createPreviewEngineMainDevice", "create_preview_engine_main_device"),
                           ("sendChromaFramesToDock", "send_chroma_frames_to_dock")):
        if source in document:
            boolean(document[source], source)
        support[target] = document.get(source)
    override = document.get("overwriteDeviceConfig", {})
    if not isinstance(override, dict) or set(override) - {"ignoreSetDeviceModeTimeoutError"}:
        raise ValueError("Battery peer device override contains an unresolved field")
    ignore = override.get("ignoreSetDeviceModeTimeoutError", False)
    boolean(ignore, "ignoreSetDeviceModeTimeoutError")
    support["ignore_set_device_mode_timeout_error"] = ignore
    exit_effect = document["quickEffectOnExit"]
    exact_fields(exit_effect, ("effectId", "params"), "peer exit effect")
    if exit_effect == {"effectId": 3, "params": {}}:
        support["quick_effect_on_exit"] = "PeerExitEffect::Spectrum"
    elif (exit_effect["effectId"] == 1 and isinstance(exit_effect["params"], dict)
          and set(exit_effect["params"]) == {"color"}
          and isinstance(exit_effect["params"]["color"], str)
          and re.fullmatch(r"#[0-9a-fA-F]{6}", exit_effect["params"]["color"])):
        color = bytes.fromhex(exit_effect["params"]["color"][1:])
        support["quick_effect_on_exit"] = {"_type": "PeerExitEffect::Static", "color": list(color)}
    else:
        raise ValueError("Battery peer exit effect has no current typed conversion")
    support["default_profile_effect"] = receiver_battery_peer_default_effect(profile_quick_effect)
    led_configs = document["ledConfig"]
    if not isinstance(led_configs, dict) or not led_configs:
        raise ValueError("Battery peer has no LED geometry")
    geometries = []
    for key, geometry in led_configs.items():
        match = re.fullmatch(r"ledConfig_([0-9]+)_([0-9]+)", key)
        if match is None or int(match[1]) != edition:
            raise ValueError("Battery peer LED key does not match its acquired edition")
        exact_fields(geometry, (*BATTERY_PEER_GEOMETRY_FIELDS, "LedMatrix", "LedRegion", "LedInputMap"), "peer geometry")
        value = {target: bounded(geometry[source], 65535 if source in ("VID", "PID") else 255, source)
                 for source, target in BATTERY_PEER_GEOMETRY_FIELDS.items()}
        if value["edition"] != edition or value["layout"] != int(match[2]):
            raise ValueError("Battery peer LED key and declared metadata differ")
        value["led_matrix"], value["led_regions"], value["led_input_map"] = [], [], []

        def pair(raw, maximum, owner):
            if not isinstance(raw, list) or len(raw) != 2:
                raise ValueError("Battery peer position is not a pair: " + owner)
            return [bounded(item, maximum, owner) for item in raw]

        for mapping in geometry["LedMatrix"]:
            exact_fields(mapping, ("MatrixPos", "DevicePos"), "peer LED mapping")
            value["led_matrix"].append({"matrix_pos": pair(mapping["MatrixPos"], 255, "LED matrix"),
                                         "device_pos": pair(mapping["DevicePos"], 255, "device matrix")})
        for region in geometry["LedRegion"]:
            exact_fields(region, ("RegionId", "NumLed", "MaxRow", "MaxCol", "RegionMatrix"), "peer LED region")
            entry = {"region_id": bounded(region["RegionId"], 255, "region ID"),
                     "num_led": bounded(region["NumLed"], 65535, "region LEDs"),
                     "max_row": bounded(region["MaxRow"], 255, "region rows"),
                     "max_col": bounded(region["MaxCol"], 255, "region cols"), "region_matrix": []}
            for position in region["RegionMatrix"]:
                exact_fields(position, ("MatrixPos",), "peer region position")
                entry["region_matrix"].append({"matrix_pos": pair(position["MatrixPos"], 65535, "region matrix")})
            value["led_regions"].append(entry)
        input_types = {"dkm": "Dkm", "mou": "Mouse", "kbd": "Keyboard"}
        for input_map in geometry["LedInputMap"]:
            exact_fields(input_map, ("InputType", "InputData", "MatrixPos"), "peer input map")
            if input_map["InputType"] not in input_types:
                raise ValueError("Battery peer input event type is unresolved")
            value["led_input_map"].append({"input_type": "LedInputType::" + input_types[input_map["InputType"]],
                "input_data": pair(input_map["InputData"], 65535, "input data"),
                "matrix_pos": pair(input_map["MatrixPos"], 65535, "input matrix")})
        geometries.append(value)
    effects = {}
    for source, target in (("batteryLevelEffect", "battery_level_effects"), ("chargingEffect", "charging_effects")):
        effects[target] = None
        if source not in document:
            continue
        exact_fields(document[source], ("full", "high", "mid", "low", "critical"), source)
        effects[target] = {}
        for level, effect in document[source].items():
            exact_fields(effect, ("min", "max", "effectId", "effectParams"), source + "/" + level)
            exact_fields(effect["effectParams"], ("color",), source + "/" + level + "/params")
            minimum, maximum = bounded(effect["min"], 100, "battery minimum"), bounded(effect["max"], 100, "battery maximum")
            if minimum > maximum:
                raise ValueError("Battery peer effect has a reversed level range")
            effects[target][level] = {"minimum": minimum, "maximum": maximum,
                "effect_id": bounded(effect["effectId"], 65535, "battery effect"),
                "color": bounded(effect["effectParams"]["color"], 0xFFFFFFFF, "battery color")}
    return {"support": support, "geometries": sorted(geometries, key=lambda value: value["layout"]), **effects}


def receiver_battery_peer_inputs():
    import families
    import host
    audit = host.HostLocalAudit()
    peers = families.receiver_battery_peer_sources(audit)
    rows, inputs, seen = [], [], set()
    for pid, peer in zip(BATTERY_PEER_PRODUCTS, peers["source_sets"], strict=True):
        source_set = peer["source_set"]
        acquired = json.loads(local_path(source_set["acquisition"]["path"]).read_bytes(), object_pairs_hook=duplicate_free)
        if (source_set["name"] != f"mouse-mw-{pid}-current"
                or acquired.get("entry_url") != f"https://apps.razer.com/synapse/products/{pid}/mw/index.html"):
            raise ValueError("Battery peer acquisition has a different mounted product")
        inputs.append(source_set["acquisition"])
        exported = peer["default_profile_export"]
        source, binding = exported["original"], exported["binding"]
        inputs.append(source)
        node = audit.tree(source).root_node.descendant_for_byte_range(binding["start_byte"], binding["end_byte"] - 1)
        while node is not None and node.type != "variable_declarator":
            node = node.parent
        if node is None or node.start_byte != binding["start_byte"] or node.end_byte != binding["end_byte"]:
            raise ValueError("Battery peer default profile declaration is unresolved")
        value = node.child_by_field_name("value")
        if value.type != "object":
            raise ValueError("Battery peer default profile is not a complete object")
        quick = [item.child_by_field_name("value") for item in value.named_children
                 if item.type == "pair" and item.child_by_field_name("key").text == b"quickEffects"]
        if len(quick) != 1:
            raise ValueError("Battery peer default profile quick effect owner is absent or ambiguous")
        profile_quick_effect = families.keyboard_static_literal(quick[0].text.decode())
        for item in peer["manifests"]:
            original = item["original"]
            name = Path(original["path"]).name
            match = re.fullmatch(r"DeviceManifest_([0-9]+)_([0-9]+)\.json", name)
            if match is None:
                raise ValueError("Battery peer source edition is unresolved")
            document = json.loads(audit.read(original), object_pairs_hook=duplicate_free)
            identity = (int(match[1]), int(match[2]))
            if identity in seen:
                raise ValueError("Duplicate battery peer edition")
            seen.add(identity)
            row = receiver_battery_peer_projection(document, *identity, profile_quick_effect)
            row["default_profile_export"] = exported
            rows.append(row)
            inputs.append({key: original[key] for key in ("path", "bytes", "sha256")})
    if len(rows) != 25:
        raise ValueError("Current battery peer edition coverage changed; audit all acquired branches")
    if audit.final_snapshot():
        raise ValueError("Battery peer source bytes changed during generation")
    return sorted(rows, key=lambda row: (row["support"]["source_product_id"], row["support"]["edition"])), inputs


def render_receiver_battery_peers(rows):
    """Share equal geometry with ordinary constructors; metadata remains per branch."""
    def literal(value, kind=None):
        if type(value) is bool:
            return str(value).lower()
        if type(value) is int:
            return str(value)
        if value is None:
            return "None"
        if isinstance(value, str):
            return value
        if isinstance(value, list):
            return "[" + ", ".join(literal(item) for item in value) + "]"
        if isinstance(value, dict):
            fields = [name + ": " + literal(item) for name, item in value.items() if name != "_type"]
            return value.get("_type", kind) + " { " + ", ".join(fields) + " }"
        raise ValueError("Unexpected generated battery peer value")

    lines = ["//! Current battery-monitor peer manifests. Regenerate with workspace.py prepare-receiver-battery-peers.",
        "use super::{PeerBatteryEffect, PeerBatteryEffects, PeerCategory, PeerDeviceFactory, PeerExitEffect, PeerProfileEffect, PeerSupport};",
        "use crate::lighting_engine::{LedConfiguration, LedInputMap, LedInputType, LedMapping, LedRegion, RegionMatrix};",
        "pub(super) const IDENTITIES: &[(u16, u8)] = &[" + ",".join(
            f"({row['support']['source_product_id']},{row['support']['edition']})" for row in rows) + "];",
        "pub(super) fn support(source_product_id: u16, edition: u8) -> Option<PeerSupport> {",
        "match (source_product_id,edition) {"]
    for row in rows:
        support = row["support"]
        fields = [name + ": " + ("Some(" + literal(value) + ")" if name in {"ble_product_id", "create_preview_engine_main_device", "send_chroma_frames_to_dock"} and value is not None else literal(value))
                  for name, value in support.items()]
        lines.append(f"({support['source_product_id']},{support['edition']}) => Some(PeerSupport {{" + ",".join(fields) + "}),")
    lines.extend(["_ => None,", "}", "}"])
    for name in ("battery_level_effects", "charging_effects"):
        lines.extend([f"pub(super) fn {name}(source_product_id:u16,edition:u8)->Option<PeerBatteryEffects> {{",
                      "match (source_product_id,edition) {"])
        for row in rows:
            if row[name] is not None:
                effects = ",".join(level + ": " + literal(effect, "PeerBatteryEffect") for level, effect in row[name].items())
                lines.append(f"({row['support']['source_product_id']},{row['support']['edition']}) => Some(PeerBatteryEffects {{" + effects + "}),")
        lines.extend(["_ => None,", "}", "}"])
    lines.extend([
        "pub(super) fn led_configuration(source_product_id: u16, edition: u8, layout: u8) -> Option<LedConfiguration> {",
        "match (source_product_id,edition,layout) {"])
    shapes = []
    metadata = ("vendor_id", "product_id", "edition", "layout")
    for row in rows:
        for geometry in row["geometries"]:
            shape = {key: value for key, value in geometry.items() if key not in metadata}
            if shape not in shapes:
                shapes.append(shape)
            arguments = ",".join(str(geometry[key]) for key in metadata)
            lines.append(f"({row['support']['source_product_id']},{row['support']['edition']},{geometry['layout']}) => Some(geometry_{shapes.index(shape)}({arguments})),")
    lines.extend(["_ => None,", "}", "}"])
    for index, shape in enumerate(shapes):
        lines.append(f"fn geometry_{index}(vendor_id: u16, product_id: u16, edition: u8, layout: u8) -> LedConfiguration {{")
        lines.append("LedConfiguration { vendor_id: vendor_id, product_id: product_id, edition: edition, layout: layout,")
        for name, value in shape.items():
            if name == "led_matrix":
                expression = "vec![" + ",".join(literal(item, "LedMapping") for item in value) + "]"
            elif name == "led_input_map":
                expression = "vec![" + ",".join(literal(item, "LedInputMap") for item in value) + "]"
            elif name == "led_regions":
                regions = []
                for region in value:
                    fields = [field + ": " + literal(item) for field, item in region.items() if field != "region_matrix"]
                    fields.append("region_matrix: vec![" + ",".join(literal(item, "RegionMatrix") for item in region["region_matrix"]) + "]")
                    regions.append("LedRegion {" + ",".join(fields) + "}")
                expression = "vec![" + ",".join(regions) + "]"
            else:
                expression = literal(value)
            lines.append(name + ": " + expression + ",")
        lines.extend(["}", "}"])
    return "\n".join(lines).encode("utf-8")


def verify_receiver_battery_peers(content, rows):
    """Decode the complete Rust constructors and compare every field and array position."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_rust
    parser = Parser(Language(tree_sitter_rust.language()))
    tree = parser.parse(content)
    if tree.root_node.has_error:
        raise ValueError("Generated battery peer Rust has an AST error")
    functions = {node.child_by_field_name("name").text.decode(): node
                 for node in tree.root_node.named_children if node.type == "function_item"}

    def decode(node, environment=None):
        environment = environment or {}
        text = node.text.decode()
        if node.type in ("integer_literal", "boolean_literal"):
            return json.loads(text)
        if node.type == "identifier":
            if text == "None":
                return None
            if text in environment:
                return environment[text]
        if node.type == "scoped_identifier" and text.startswith(("PeerCategory::", "PeerDeviceFactory::", "PeerExitEffect::", "PeerProfileEffect::", "LedInputType::")):
            return text
        if node.type == "reference_expression":
            return decode(node.named_children[0], environment)
        if node.type in ("array_expression", "tuple_expression"):
            return [decode(child, environment) for child in node.named_children]
        if node.type in {"block", "expression_statement"} and len(node.named_children) == 1:
            return decode(node.named_children[0], environment)
        if node.type == "macro_invocation" and node.child_by_field_name("macro").text == b"vec":
            tokens = next(child for child in node.named_children if child.type == "token_tree").text
            wrapped = parser.parse(b"fn values(){" + tokens + b"}")
            expression = wrapped.root_node.named_children[0].child_by_field_name("body").named_children[0]
            return decode(expression, environment)
        if node.type == "call_expression":
            name = node.child_by_field_name("function").text.decode()
            values = [decode(child, environment) for child in node.child_by_field_name("arguments").named_children]
            if name == "Some" and len(values) == 1:
                return values[0]
            if name.startswith("geometry_") and name in functions:
                function = functions[name]
                names = [param.child_by_field_name("pattern").text.decode()
                         for param in function.child_by_field_name("parameters").named_children]
                if len(names) == len(values):
                    return decode(function.child_by_field_name("body"), dict(zip(names, values)))
        if node.type == "struct_expression":
            name = node.child_by_field_name("name").text.decode()
            if name not in {"PeerSupport", "PeerBatteryEffect", "PeerBatteryEffects", "LedConfiguration", "LedMapping", "LedRegion", "RegionMatrix", "LedInputMap", "PeerExitEffect::Static", "PeerProfileEffect::Static"}:
                raise ValueError("Unexpected generated battery peer struct")
            value = {"_type": name} if name in {"PeerExitEffect::Static", "PeerProfileEffect::Static"} else {}
            for field in node.child_by_field_name("body").named_children:
                if field.type == "shorthand_field_initializer" and field.text.decode() in environment:
                    key = field.text.decode()
                    if key in value:
                        raise ValueError("Duplicate generated battery peer field")
                    value[key] = environment[key]
                    continue
                if field.type != "field_initializer":
                    raise ValueError("Battery peer fields must be explicit")
                key = field.child_by_field_name("field").text.decode()
                if key in value:
                    raise ValueError("Duplicate generated battery peer field")
                value[key] = decode(field.child_by_field_name("value"), environment)
            return value
        raise ValueError("Nonliteral generated battery peer constructor: " + node.type + " " + text[:80])

    def branches(name):
        body = functions[name].child_by_field_name("body")
        expression = body.named_children[0] if len(body.named_children) == 1 else None
        if expression is not None and expression.type == "expression_statement" and len(expression.named_children) == 1:
            expression = expression.named_children[0]
        if expression is None or expression.type != "match_expression":
            raise ValueError("Battery peer entry must contain one complete match")
        arms = expression.child_by_field_name("body").named_children
        values, fallback = {}, 0
        for arm in arms:
            pattern, value = arm.child_by_field_name("pattern"), arm.child_by_field_name("value")
            if pattern.text == b"_":
                if decode(value) is not None:
                    raise ValueError("Battery peer fallback must preserve absence")
                fallback += 1
                continue
            # match_pattern wraps the concrete tuple pattern.
            concrete = pattern.named_children[0] if pattern.type == "match_pattern" else pattern
            if concrete.type != "tuple_pattern":
                raise ValueError("Battery peer branch identity is not a tuple")
            key = tuple(json.loads(child.text) for child in concrete.named_children)
            if key in values:
                raise ValueError("Duplicate generated battery peer branch")
            values[key] = decode(value)
        if fallback != 1:
            raise ValueError("Battery peer match must preserve one unknown branch")
        return values

    expected_support = {(row["support"]["source_product_id"], row["support"]["edition"]): row["support"] for row in rows}
    expected_geometry = {(row["support"]["source_product_id"], row["support"]["edition"], geometry["layout"]): geometry
                         for row in rows for geometry in row["geometries"]}
    identities = [node for node in tree.root_node.named_children if node.type == "const_item"
                  and node.child_by_field_name("name").text == b"IDENTITIES"]
    if (len(identities) != 1 or decode(identities[0].child_by_field_name("value")) != [list(key) for key in expected_support]
            or branches("support") != expected_support or branches("led_configuration") != expected_geometry):
        raise ValueError("Generated battery peer fields, branches or array order differ from current manifests")
    for name in ("battery_level_effects", "charging_effects"):
        expected = {(row["support"]["source_product_id"], row["support"]["edition"]): row[name]
                    for row in rows if row[name] is not None}
        if branches(name) != expected:
            raise ValueError("Generated peer battery effect fields or branches differ from current manifests")
    return {"identities": len(expected_support), "geometries": len(expected_geometry), "all_fields_and_order_equal": True}


def prepare_receiver_battery_peers(*, check=False, record=True):
    import subprocess
    import analysis
    import families
    rows, inputs = receiver_battery_peer_inputs()
    content = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
                             input=render_receiver_battery_peers(rows), capture_output=True, check=True).stdout
    output = local_path(BATTERY_PEER_OUTPUT)
    if check:
        if not output.is_file() or output.read_bytes() != content:
            raise ValueError("Typed battery peers differ from the complete current manifests; regenerate them")
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(content)
    equivalence = verify_receiver_battery_peers(output.read_bytes(), rows)
    if not all(families.valid(item) for item in inputs):
        raise ValueError("Battery peer inputs changed during generation")
    default_profiles = {row["default_profile_export"]["original"]["path"]: row["default_profile_export"] for row in rows}
    report = {"tool": families.identity(Path(__file__)), "inputs": inputs,
              "default_profile_exports": list(default_profiles.values()),
              "output": families.identity(output), "constructor_equivalence": equivalence,
              "check_only": check, "target_execution": False, "runtime_acceptance": "unverified"}
    if record:
        destination = ROOT / ".analysis/results/typed-data.json"
        with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
            previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
            previous["receiver_battery_peers"] = report
            analysis.save(destination, previous)
    return {"output": BATTERY_PEER_OUTPUT, **equivalence, "passed": True, "check_only": check}


def receiver_pairing_namespace_contract(bodies):
    """Derive the selected UMA factories from complete current JavaScript owners."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    parser = Parser(Language(tree_sitter_javascript.language()))

    def parse(text):
        tree = parser.parse(text.encode())
        if tree.root_node.has_error:
            raise ValueError("Receiver namespace source has a JavaScript AST error")
        return tree.root_node

    def walk(node):
        yield node
        for child in node.named_children:
            yield from walk(child)

    def shape(node):
        children = [child for child in node.children if child.type != "comment"]
        return (node.type, tuple(shape(child) for child in children)) if children else (node.type, node.text)

    def expression(text):
        return parse("(" + text + ");").named_children[0].named_children[0].named_children[0]

    def only(nodes, label):
        nodes = list(nodes)
        if len(nodes) != 1:
            raise ValueError("Receiver namespace source has an absent or ambiguous " + label)
        return nodes[0]

    def member(node, name):
        return (node.type == "member_expression" and node.child_by_field_name("object").type == "this"
                and node.child_by_field_name("property").text == name.encode())

    def class_node(role):
        return expression(bodies[role])

    common = class_node("protocol_common_device")
    initial = only((node for node in walk(common) if node.type == "assignment_expression"
                    and member(node.child_by_field_name("left"), "transactionId")), "initial transaction counter")
    if initial.child_by_field_name("right").text != b"0":
        raise ValueError("Receiver category factories no longer start their own counter at zero")

    def factory(role):
        node = class_node(role)
        method_nodes = [child for child in node.child_by_field_name("body").named_children if child.type == "method_definition"]
        methods = {child.child_by_field_name("name").text.decode(): child for child in method_nodes}
        if len(methods) != len(method_nodes) or not {"_getTransactionId", "sendCommand"} <= set(methods):
            raise ValueError("Receiver namespace factory has absent or repeated transport methods")
        transaction = methods["_getTransactionId"].child_by_field_name("body").named_children
        if len(transaction) != 1 or transaction[0].type != "return_statement":
            raise ValueError("Receiver transaction method must retain one complete return")
        sequence = transaction[0].named_children[0]
        if (sequence.type != "sequence_expression" or sequence.named_child_count != 2
                or shape(sequence.named_children[0]) != shape(expression("31===this.transactionId&&(this.transactionId=0)"))):
            raise ValueError("Receiver transaction counter reset or modulus changed")
        result = sequence.named_children[1]
        prefix = 0
        if result.type == "binary_expression":
            left = result.child_by_field_name("left")
            if left.type != "number" or result.child_by_field_name("operator").text != b"|":
                raise ValueError("Receiver transaction prefix is not a fixed bitwise namespace")
            prefix, result = int(left.text), result.child_by_field_name("right")
        if shape(result) != shape(expression("this.transactionId++")):
            raise ValueError("Receiver transaction method changed its persistent sequence")
        sender = methods["sendCommand"]
        header = sender.child_by_field_name("parameters").named_children[0].text.decode()
        rewrite = only((child for child in walk(sender) if child.type == "augmented_assignment_expression"
                        and child.child_by_field_name("left").type == "subscript_expression"
                        and child.child_by_field_name("left").child_by_field_name("index").text == b"1"),
                       "command transaction rewrite")
        target = rewrite.child_by_field_name("left").child_by_field_name("object")
        value, operator = rewrite.child_by_field_name("right"), rewrite.child_by_field_name("operator")
        if target.type != "identifier" or value.type != "number" or operator.text not in {b"&=", b"|="}:
            raise ValueError("Receiver command transaction rewrite is not a fixed mask")
        condition = rewrite.parent.parent
        if condition.type != "binary_expression":
            raise ValueError("Receiver namespace rewrite lost its command guard")
        guard = condition.child_by_field_name("left")
        comparisons = guard.named_children
        if guard.type != "binary_expression" or len(comparisons) != 2:
            raise ValueError("Receiver namespace rewrite has an unresolved command guard")
        command = comparisons[0].child_by_field_name("right")
        if command is None or command.type != "subscript_expression":
            raise ValueError("Receiver namespace rewrite lost its protocol table")
        table = command.child_by_field_name("object")
        if table.type != "member_expression" or table.child_by_field_name("object").type != "identifier":
            raise ValueError("Receiver namespace command table is unresolved")
        alias, packet = table.child_by_field_name("object").text.decode(), target.text.decode()
        expected = f"{header}[2]!=={alias}.pJ[2]&&{header}[2]!=={alias}.fr[2]||({packet}[1]{operator.text.decode()}{int(value.text)})"
        if shape(condition) != shape(expression(expected)):
            raise ValueError("Receiver namespace rewrite must apply only to scan and pair commands")
        declaration = only((child for child in walk(sender) if child.type == "variable_declarator"
                            and child.child_by_field_name("name").text == target.text), "rewritten command frame")
        call = declaration.child_by_field_name("value")
        if (call.type != "call_expression" or not member(call.child_by_field_name("function"), "_createDataSend")
                or call.child_by_field_name("arguments").named_children[0].text != header.encode()):
            raise ValueError("Receiver namespace rewrite does not target the constructed command")
        scan_pair = prefix & int(value.text) if operator.text == b"&=" else prefix | int(value.text)
        return {"normal_prefix": prefix, "scan_pair_prefix": scan_pair}

    factories = {14770: factory("protocol_pairing_mouse_category"),
                 48320: factory("protocol_pairing_keyboard_category")}
    primary = factory("linker_factory")
    aliases = {}
    for role in ("pairing_factory_V", "pairing_factory_z"):
        declaration = parse("const " + bodies[role] + ";").named_children[0].named_children[0]
        call = declaration.child_by_field_name("value")
        arguments = call.child_by_field_name("arguments") if call.type == "call_expression" else None
        if arguments is None or arguments.named_child_count != 1 or arguments.named_children[0].type != "number":
            raise ValueError("Receiver category factory import is unresolved")
        name = declaration.child_by_field_name("name").text.decode()
        if name in aliases:
            raise ValueError("Receiver category factory repeats an import")
        aliases[name] = int(arguments.named_children[0].text)
    selection = parse("const " + bodies["mw_pairing_factory_selection"] + ";")
    maps = []
    for node in walk(selection):
        if node.type == "object" and {child.child_by_field_name("key").text for child in node.named_children
                                      if child.type == "pair"} == {b"true", b"false"}:
            maps.append(node)
    mapping = only(maps, "forward and metadata category map")
    factory_function = selection.named_children[0].named_children[0].child_by_field_name("value")
    if factory_function.type != "arrow_function":
        raise ValueError("Receiver category selector is not the complete current arrow function")
    parameters = factory_function.child_by_field_name("parameters").named_children
    if len(parameters) != 2 or parameters[0].type != "identifier" or parameters[1].type != "assignment_pattern":
        raise ValueError("Receiver category selector has unresolved category or mode parameters")
    category_parameter = parameters[0].text.decode()
    mode_parameter = parameters[1].child_by_field_name("left").text.decode()
    indexed = mapping.parent
    assignment = indexed.parent
    if indexed.type != "subscript_expression" or assignment.type != "assignment_expression":
        raise ValueError("Receiver category mapping no longer feeds the selected factory")
    temporary = assignment.child_by_field_name("left")
    if temporary.type != "identifier":
        raise ValueError("Receiver category mapping has an unresolved intermediate owner")
    declaration = assignment.parent
    while declaration is not None and declaration.type != "variable_declarator":
        declaration = declaration.parent
    if declaration is None:
        raise ValueError("Receiver category selection lost its retained factory declaration")
    temporary_name = temporary.text.decode()
    expected_selection = (f"null===({temporary_name}={mapping.text.decode()}[String({mode_parameter})])"
                          f"||void 0==={temporary_name}?void 0:{temporary_name}[{category_parameter}]")
    if shape(declaration.child_by_field_name("value")) != shape(expression(expected_selection)):
        raise ValueError("Receiver category map is not selected by the supplied category and mode")
    selected_factory = declaration.child_by_field_name("name").text
    only((node for node in walk(factory_function) if node.type == "new_expression"
          and node.child_by_field_name("constructor").text == selected_factory), "fresh selected category instance")
    if len(mapping.named_children) != 2:
        raise ValueError("Receiver category map repeats a mode branch")
    branches = {}
    for branch in mapping.named_children:
        value = branch.child_by_field_name("value")
        if value.type != "object":
            raise ValueError("Receiver category map is not an explicit object")
        categories = {}
        for pair in value.named_children:
            key, factory_value = pair.child_by_field_name("key"), pair.child_by_field_name("value")
            if (key.type != "computed_property_name" or key.named_child_count != 1
                    or key.named_children[0].type != "member_expression" or factory_value.type != "member_expression"
                    or factory_value.child_by_field_name("property").text != b"default"):
                raise ValueError("Receiver category map has an unresolved factory")
            category = key.named_children[0].child_by_field_name("property").text.decode()
            alias = factory_value.child_by_field_name("object").text.decode()
            if category in categories or aliases.get(alias) not in factories:
                raise ValueError("Receiver category map repeats a branch or selects an unknown module")
            categories[category] = aliases[alias]
        if set(categories) != {"MOUSE", "KEYBOARD"}:
            raise ValueError("Receiver category map is incomplete")
        branches[branch.child_by_field_name("key").text.decode()] = categories
    rows = {category: {"scan_pair_prefix": factories[module]["scan_pair_prefix"],
                       "unpair_prefix": factories[module]["normal_prefix"],
                       "cancel_prefix": primary["scan_pair_prefix"],
                       "metadata_prefix": factories[branches["false"][category]]["normal_prefix"]}
            for category, module in branches["true"].items()}
    if rows != RECEIVER_179_PAIRING_NAMESPACES or primary["normal_prefix"] != 224:
        raise ValueError("Current PID179 source namespaces changed; review every primary and category owner")
    return {"product_id": 179, "primary_prefix": primary["normal_prefix"],
            "category_modules": branches, "categories": rows}


def receiver_table_tree(content):
    """Parse typed constructors, exposing vec elements without changing byte offsets."""
    import sys
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_rust
    parser = Parser(Language(tree_sitter_rust.language()))
    tree = parser.parse(content)
    if tree.root_node.has_error:
        raise ValueError("Receiver sender table has a Rust AST error")
    pending, masks = [tree.root_node], []
    while pending:
        node = pending.pop(); pending.extend(node.named_children)
        if node.type == "macro_invocation" and node.child_by_field_name("macro").text == b"vec":
            tokens = next(child for child in node.named_children if child.type == "token_tree")
            if tokens.text[:1] != b"[":
                raise ValueError("Receiver sender vec macro requires an array constructor")
            masks.append((node.start_byte, tokens.start_byte))
    expanded = content
    for start, end in masks:
        expanded = expanded[:start] + b" " * (end-start) + expanded[end:]
    tree = parser.parse(expanded)
    if tree.root_node.has_error:
        raise ValueError("Receiver sender vector expansion has a Rust AST error")
    return tree


def receiver_evidence_path(product_id):
    """Resolve the captured source family, independent of the physical receiver PID."""
    usb_product(product_id, "receiver source product")
    candidates = list((ROOT / ".analysis/results/frontend").glob(f"*-mw-{product_id}-current/analysis.json"))
    if len(candidates) != 1:
        raise ValueError(f"Receiver source {product_id} requires exactly one current capture")
    path = candidates[0]
    capture = json.loads(path.read_bytes(), object_pairs_hook=duplicate_free)
    source = ".analysis/inputs/frontend/" + path.parent.name
    if capture.get("input") != source or not (ROOT / source).is_dir() or not capture.get("original_inputs"):
        raise ValueError(f"Receiver source {product_id} capture does not identify its current originals")
    return path.relative_to(ROOT).as_posix()


def receiver_evidence_table_projection(content):
    """Keep source provenance in the maintenance receipt, never in runtime tables."""
    tree = receiver_table_tree(content)
    pending, rows = [tree.root_node], []
    while pending:
        node = pending.pop(); pending.extend(node.named_children)
        if node.type != "struct_expression" or node.child_by_field_name("name").text != b"ReceiverCapability":
            continue
        fields = {}
        for field in node.child_by_field_name("body").named_children:
            if field.type != "field_initializer":
                raise ValueError("Receiver evidence requires explicit capability fields")
            name = field.child_by_field_name("field").text.decode()
            if name in fields:
                raise ValueError("Receiver evidence capability repeats a field")
            fields[name] = field.child_by_field_name("value")
        pid = fields.get("source_product_id")
        if "evidence_path" in fields:
            raise ValueError("Receiver evidence paths belong only in maintenance receipts")
        if pid is None or pid.type != "integer_literal":
            raise ValueError("Receiver evidence requires an explicit source identity")
        expected = receiver_evidence_path(int(pid.text))
        rows.append({"source_product_id": int(pid.text), "evidence_path": expected})
    if not rows:
        raise ValueError("Receiver evidence table has no capability constructors")
    return content, sorted(rows, key=lambda row: row["source_product_id"])


def receiver_sender_table_projection(content, bindings, *, update=False):
    """Read actual typed receiver constructors; preserve every other field and caller."""
    tree = receiver_table_tree(content)
    pending, edits, found, verified = [tree.root_node], [], set(), []
    while pending:
        node = pending.pop(); pending.extend(node.named_children)
        if node.type != "struct_expression" or node.child_by_field_name("name").text != b"ReceiverCapability":
            continue
        fields = {}
        for field in node.child_by_field_name("body").named_children:
            if field.type != "field_initializer":
                raise ValueError("Receiver capability requires explicit typed fields")
            name = field.child_by_field_name("field").text.decode()
            if name in fields:
                raise ValueError("Receiver capability repeats a field")
            fields[name] = field.child_by_field_name("value")
        identity = {}
        for name in ("source_product_id", "product_id"):
            value = fields.get(name)
            if value is None or value.type != "integer_literal":
                raise ValueError("Receiver capability identity is not an integer")
            identity[name] = int(value.text)
        pid = identity["source_product_id"]
        if pid not in bindings:
            continue
        if identity["product_id"] != pid:
            raise ValueError("Current Linker sender table changed its physical branch")
        expected = bindings[pid]
        source = fields.get("source_class")
        if source is None or source.type != "call_expression":
            raise ValueError("Current Linker sender source class must be explicit")
        function = source.child_by_field_name("function")
        if (function.type != "field_expression" or function.child_by_field_name("field").text != b"into"
                or source.child_by_field_name("arguments").named_children
                or function.child_by_field_name("value").type != "string_literal"
                or json.loads(function.child_by_field_name("value").text) != expected["source_class"]):
            raise ValueError("Current Linker table source class differs from its selected adapter")
        for name in ("claim_interface", "report_id"):
            value = fields.get(name)
            if value is None or value.type != "integer_literal":
                raise ValueError("Current Linker selector is not an integer")
            if int(value.text) != expected[name]:
                if not update:
                    raise ValueError("Current Linker " + name + " differs from its mounted sender")
                edits.append((value.start_byte, value.end_byte, str(expected[name]).encode()))
        found.add(pid)
        verified.append({**identity, "source_class": expected["source_class"],
                         "claim_interface": expected["claim_interface"], "report_id": expected["report_id"]})
    if not found:
        raise ValueError("Receiver sender table contains no current mounted Linker constructor")
    for start, end, replacement in sorted(edits, reverse=True):
        content = content[:start] + replacement + content[end:]
    return content, sorted(verified, key=lambda row: (row["source_product_id"], row["product_id"]))


def receiver_pairing_namespace_table_projection(content, contract, *, update=False):
    tree = receiver_table_tree(content)
    pending, edits, found = [tree.root_node], [], {}
    while pending:
        node = pending.pop(); pending.extend(node.named_children)
        if node.type != "struct_expression" or node.child_by_field_name("name").text != b"PairingCapability":
            continue
        fields = {}
        for field in node.child_by_field_name("body").named_children:
            if field.type != "field_initializer":
                raise ValueError("Pairing capability requires explicit typed fields")
            name = field.child_by_field_name("field").text.decode()
            if name in fields:
                raise ValueError("Pairing capability repeats a field")
            fields[name] = field.child_by_field_name("value")
        pid = fields.get("product_id")
        if pid is None or pid.type != "integer_literal":
            raise ValueError("Pairing capability identity is not an integer")
        if int(pid.text) != contract["product_id"]:
            continue
        category = fields.get("category")
        if category is None or category.type != "call_expression":
            raise ValueError("Pairing category must be an explicit string")
        function = category.child_by_field_name("function")
        if (function.type != "field_expression" or function.child_by_field_name("field").text != b"into"
                or category.child_by_field_name("arguments").named_children
                or function.child_by_field_name("value").type != "string_literal"):
            raise ValueError("Pairing category must retain its typed string conversion")
        category = json.loads(function.child_by_field_name("value").text)
        if category not in contract["categories"] or category in found:
            raise ValueError("PID179 pairing table has an unknown or repeated category")
        expected = contract["categories"][category]
        for name, value in expected.items():
            actual = fields.get(name)
            if actual is None or actual.type != "integer_literal":
                raise ValueError("Pairing namespace is not an explicit integer")
            if int(actual.text) != value:
                if not update:
                    raise ValueError("PID179 pairing " + category + " " + name + " differs from its current source owner")
                edits.append((actual.start_byte, actual.end_byte, str(value).encode()))
        found[category] = {"product_id": contract["product_id"], "category": category, **expected}
    if set(found) != set(contract["categories"]):
        raise ValueError("PID179 pairing table is missing a category branch")
    for start, end, replacement in sorted(edits, reverse=True):
        content = content[:start] + replacement + content[end:]
    return content, [found[key] for key in sorted(found)]


def prepare_receiver_sender_tables(*, check=False, record=True):
    import analysis
    import families
    import host
    audit = host.HostLocalAudit()
    sources = [families.receiver_sender_sources(audit, pid) for pid in (164, 179, 241)]
    bindings = {row["product_id"]: row for row in sources}
    pairing = families.receiver_179_local_sources(audit)
    roles = ("protocol_common_device", "protocol_pairing_mouse_category", "protocol_pairing_keyboard_category",
             "linker_factory", "pairing_factory_V", "pairing_factory_z", "mw_pairing_factory_selection")
    namespace_owners = {}
    for role in roles:
        keys = pairing["roles"][role]
        if len(keys) != 1:
            raise ValueError("PID179 namespace source owner is ambiguous: " + role)
        namespace_owners[role] = audit.owners[keys[0]]
    for role, module in (("protocol_pairing_mouse_category", 14770), ("protocol_pairing_keyboard_category", 48320)):
        owner = namespace_owners[role]
        enclosing = [audit.owners[row["owner"]] for row in pairing["complete_factories"] if row["module_id"] == module]
        if not any(row["original"] == owner["original"] and row["byte_start"] <= owner["byte_start"]
                   and owner["byte_end"] <= row["byte_end"] for row in enclosing):
            raise ValueError("PID179 category class differs from its imported complete factory")
    namespaces = receiver_pairing_namespace_contract({role: owner["body"] for role, owner in namespace_owners.items()})
    if (pairing["cancel_contract"]["target"] != "primary_runtime_device"
            or pairing["cancel_contract"]["scan_status"] != 4 or pairing["cancel_contract"]["category_parameter"]):
        raise ValueError("PID179 Cancel no longer belongs to the primary runtime owner")
    tables = []
    for name in (QUERY_OUTPUT, PAIR_CAP_OUTPUT):
        path = local_path(name)
        original = path.read_bytes()
        rendered, rows = receiver_sender_table_projection(original, bindings, update=not check)
        rendered, evidence_rows = receiver_evidence_table_projection(rendered)
        namespace_rows = []
        if name == PAIR_CAP_OUTPUT:
            rendered, namespace_rows = receiver_pairing_namespace_table_projection(rendered, namespaces, update=not check)
        if not check and rendered != original:
            path.write_bytes(rendered)
        verified, verified_rows = receiver_sender_table_projection(path.read_bytes(), bindings)
        if verified != path.read_bytes() or rows != verified_rows:
            raise ValueError("Receiver sender typed constructor verification differs")
        verified, verified_evidence = receiver_evidence_table_projection(path.read_bytes())
        if verified != path.read_bytes() or evidence_rows != verified_evidence:
            raise ValueError("Receiver evidence typed constructor verification differs")
        if name == PAIR_CAP_OUTPUT:
            verified, verified_namespaces = receiver_pairing_namespace_table_projection(path.read_bytes(), namespaces)
            if verified != path.read_bytes() or namespace_rows != verified_namespaces:
                raise ValueError("Receiver pairing namespace verification differs")
        tables.append({"output": families.identity(path), "complete_selector_constructors": rows,
                       "complete_pairing_namespaces": namespace_rows, "source_capture_paths": evidence_rows})
    if audit.final_snapshot():
        raise ValueError("Current receiver sender source bytes changed during generation")
    report = {"tool": families.identity(Path(__file__)), "source_extractor": families.identity(Path(families.__file__)),
              "sources": sources, "complete_owners": list(audit.owners.values()),
              "inputs": list(audit.artifacts.values()), "pairing_namespaces": namespaces,
              "namespace_source_owners": {role: row["id"] for role, row in namespace_owners.items()},
              "primary_cancel_contract": pairing["cancel_contract"], "tables": tables, "check_only": check,
              "target_execution": False, "runtime_acceptance": "unverified"}
    if record:
        destination = ROOT / ".analysis/results/typed-data.json"
        with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
            previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
            previous["receiver_sender_tables"] = report
            analysis.save(destination, previous)
    return {"products": sorted(bindings), "typed_constructors": sum(len(row["complete_selector_constructors"]) for row in tables),
            "pairing_namespace_constructors": sum(len(row["complete_pairing_namespaces"]) for row in tables),
            "source_selectors_equal": True, "source_pairing_namespaces_equal": True, "passed": True, "check_only": check}


def fixed_tables():
    return {
        ("razer-device", "DEVICE_WRITE_CAPABILITIES"): (WRITE_INPUT, WRITE_OUTPUT, render_write_catalog, False),
        ("razer-device", "DEVICE_READ_CAPABILITIES"): (READ_INPUT, READ_OUTPUT, render_read_catalog, True),
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
        if output == READ_OUTPUT:
            content, _ = current_read_table_projection(content, current_read_sources()[0], update=True)
        if formatted:
            content = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
                                     input=content, capture_output=True, check=True).stdout
        target = root / output
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content)


def add_commands(commands):
    p = commands.add_parser("prepare-receiver-senders", help="Verify typed Linker query and pairing selectors against current mounted senders")
    p.add_argument("--check", action="store_true")
    p = commands.add_parser("prepare-receiver-battery-peers", help="Generate typed current battery peer transport and complete LED geometry")
    p.add_argument("--check", action="store_true")
    p = commands.add_parser("prepare-profile-transfer-table", help="Generate UI reader and MW writer transfer capabilities from current mounted originals")
    p.add_argument("--check", action="store_true")
    p.add_argument("--workers", type=int, choices=range(2, 17), default=4,
                   help="Bounded worker threads for independent mounted product readers")
    p = commands.add_parser("prepare-mapping-producers", help="Generate current mouse assignment producer metadata from complete mounted factories")
    p.add_argument("--check", action="store_true")
    p.add_argument("--workers", type=int, choices=range(2, 17), default=4,
                   help="bounded worker threads for independent mounted producer factories")
    p = commands.add_parser("prepare-mapping-capabilities", help="Generate typed mouse mapping capabilities from current mounted original factories")
    p.add_argument("--check", action="store_true")
    p.add_argument("--workers", type=int, choices=range(2, 17), default=4,
                   help="bounded worker threads for independent mounted mouse factories")
    p = commands.add_parser("prepare-local-protocol-tables", help="Generate typed local read, identity and receiver tables")
    p.add_argument("--reads-json", default=READ_INPUT)
    p.add_argument("--queries-json", default=QUERY_INPUT)
    p.add_argument("--pairing-json", default=PAIR_CAP_INPUT)
    p.add_argument("--catalog-json", default=PAIR_CATALOG_INPUT)
    p.add_argument("--check", action="store_true")
    p.add_argument("--workers", type=int, choices=range(2, 17), default=4,
                   help="bounded worker threads for independent protocol table renderers")
    p.add_argument("--current-reads", action="store_true",
                   help="Prepare source-verified current observation constructors without retired maintenance JSON")
    p.add_argument("--current-identities", action="store_true",
                   help="Prepare current host catalog and independent UI/MW identities from verified official sources")
    p = commands.add_parser("prepare-device-write-table", help="Generate typed Rust capabilities from explicit maintenance JSON")
    p.add_argument("--from-json", default=WRITE_INPUT, help="Source-derived maintenance input; never read by Cargo or runtime")
    p.add_argument("--check", action="store_true", help="Verify generated Rust without rewriting")
    p = commands.add_parser("prepare-storage-tables", help="Generate typed local receiver defaults and catalog projection")
    p.add_argument("--pairing-json", default=PAIRING_INPUT)
    p.add_argument("--reset-json", default=RESET_INPUT)
    p.add_argument("--check", action="store_true")


def run(args):
    if args.command == "prepare-receiver-senders":
        return prepare_receiver_sender_tables(check=args.check)
    if args.command == "prepare-receiver-battery-peers":
        return prepare_receiver_battery_peers(check=args.check)
    if args.command == "prepare-profile-transfer-table":
        return prepare_profile_transfer_table(check=args.check, workers=args.workers)
    if args.command == "prepare-mapping-producers":
        import mapping_producers
        return mapping_producers.prepare(check=args.check, workers=args.workers)
    if args.command == "prepare-mapping-capabilities":
        return prepare_mapping_capabilities(check=args.check, workers=args.workers)
    if args.command == "prepare-local-protocol-tables":
        return prepare_local_protocol_tables(args.reads_json, args.queries_json,
                                              args.pairing_json, args.catalog_json, args.check,
                                              current_reads=args.current_reads, current_identities=args.current_identities,
                                              workers=args.workers)
    if args.command == "prepare-device-write-table":
        return prepare_write_table(args.from_json, args.check)
    if args.command == "prepare-storage-tables":
        return prepare_receiver_storage(args.pairing_json, args.reset_json, args.check)
    raise ValueError("Unknown typed-table command")

"""Retain current host Map/action receipts without importing vendor code."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs/re/host-storage-current-evidence.json"


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def generate():
    sources = []
    for relative, actions in [
        ("keyStorage.js", ["setItem", "getKeys", "getItem", "removeItem", "registerEvent", "unregisterEvent"]),
        ("modules/memory_storage/index.js", ["setMemoryStorageItem", "setMemoryStorageItemNoEvent", "getMemoryStorageKeys", "getMemoryStorageItem", "removeMemoryStorageItem", "clearMemoryStorage", "getMemoryStorage", "registerMemoryStorageEvent", "unRegisterMemoryStorageEvent", "resetMemoryStorage"]),
        ("modules/window_storage/index.js", ["setWindowStorageItem", "getWindowStorageKeys", "getWindowStorageItem", "removeWindowStorageItem", "clearWindowStorage", "getWindowStorage", "registerWindowStorageEvent", "unRegisterWindowStorageEvent", "resetWindowStorage"]),
    ]:
        path = ".ref/host-4.0.827/electron/" + relative
        raw = (ROOT / path).read_bytes()
        # Byte ranges deliberately refer to the current original file.
        receipts = []
        for action in actions:
            token = ('case"' + action + '":').encode()
            assert raw.count(token) == 1, (path, action)
            start = raw.index(token)
            end = raw.find(b'case"', start + len(token))
            if end < 0:
                end = raw.index(b"default:", start)
            receipts.append({"action": action, "offset": start, "end": end,
                             "sha256": sha(raw[start:end])})
        sources.append({"path": path, "sha256": sha(raw), "actions": receipts})
    consumers = []
    for path, anchors in [
        ("crates/razer-storage/src/host.rs", ["fn validate_map_key", "fn same_map_key", "fn js_truthy", "pub fn close_view", "fn emit"]),
        ("crates/razer-storage/src/host_tests.rs", ["fn key_map_retains_primitive_types", "fn url_maps_preserve_null_keys", "fn memory_targets_keep_duplicate_matches", "fn same_url_subscription_survives_close"]),
        ("crates/razer-service/src/runtime/mod.rs", ["ServiceRequest::HostStorageView", "ServiceRequest::HostStorageCall", "ServiceRequest::HostStorageEvents", "ServiceRequest::HostStorageClose"]),
    ]:
        raw = (ROOT / path).read_bytes()
        text = raw.decode("utf-8")
        for anchor in anchors:
            assert text.count(anchor) == 1, (path, anchor)
        consumers.append({"path": path, "sha256": sha(raw), "anchors": anchors})
    return {
        "method": "Current host static action byte receipts and reviewed Rust consumers; no vendor execution",
        "sources": sources,
        "consumers": consumers,
        "semantics": {
            "keys": "JS Map primitive keys retain type; numbers use SameValueZero. KeyStorage rejects null keys; Memory/Window accept null keys. Object identity and undefined are not representable by the JSON wire.",
            "events": "Key first set is changed; URL first set is created. URL subscriptions survive view destruction; closed queues do not. Memory target substrings send once per match; no-event writes use sender URL.",
            "values": "Key rejects null writes. URL stores accept null/false; aggregate getters retain JS truthiness including empty objects/arrays.",
        },
        "gaps": ["Actual host window and page storage consumers remain unconnected.",
                 "JS object-identity keys, undefined values and non-JSON types remain unsupported by the JSON wire."],
        "runtime_acceptance": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = generate()
    if args.check:
        assert json.loads(OUTPUT.read_text("utf-8")) == result, "Host storage receipts changed"
    else:
        OUTPUT.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print("Host storage: 25 current action receipts; primitive Map keys, event lifecycle and explicit wire gaps")


if __name__ == "__main__":
    main()

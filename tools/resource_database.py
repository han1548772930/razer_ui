"""Prepare the single packaged SQLite catalog; never run vendor code or fetch media."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sqlite3

ROOT = Path(__file__).resolve().parent.parent
INSTALLATION = "crates/razer-resource-db/installation.json"


def installation_policy(root=None):
    import re
    policy = json.loads(((root or ROOT) / INSTALLATION).read_bytes())
    if (type(policy) is not dict or set(policy) != {"schema_version", "catalog_source", "catalog_filename", "cache_relative"}
            or policy["schema_version"] != 1):
        raise ValueError("Invalid resource installation policy")
    for key in ("catalog_source", "catalog_filename", "cache_relative"):
        value = policy[key]
        if (type(value) is not str or not re.fullmatch(r"[A-Za-z0-9_./-]+", value)
                or Path(value).is_absolute() or any(part in {"", ".", ".."} for part in value.split("/"))):
            raise ValueError("Invalid installation path: " + key)
    if "/" in policy["catalog_filename"]:
        raise ValueError("Installed catalog must be a basename")
    return policy


OUTPUT = installation_policy()["catalog_source"]
SCHEMA = "crates/razer-resource-db/src/schema.sql"
APPLICATION_ID = 0x525A5549
VERSION = 2
INPUTS = {
    "mapping": ".analysis/inputs/typed-tables/razer-pages/keyboard_mapping_text_emoji_data.json",
    "mouse": ".analysis/inputs/typed-tables/razer-pages/mouse_mapping_text_current_data.json",
    "shortcuts": ".analysis/inputs/typed-tables/razer-pages/shortcuts_current.json",
    "macro": ".analysis/inputs/typed-tables/razer-app-pages/text_data.json",
}
SELECTORS = {
    "product_image_selector": (("product-images.rs", "armory-product-images.rs"), ("u32", "u32", "u32", "part", "text")),
    "dashboard_image_selector": (("dashboard-images.rs", "armory-dashboard-images.rs"), ("u32", "u32", "u32", "text")),
    "dashboard_plugin_image_selector": (("current-dashboard-images.rs",), ("u32", "u32", "u32", "u32", "bool", "density", "text")),
    "pairing_image_selector": (("multipairing-product-images.rs",), ("u32", "u32", "u32", "text")),
    "gamepad_calibration_svg_selector": (("gamepad-calibration-svg-sources.rs",), ("u32", "u32", "kind", "text")),
}


def migrate_resource_tables():
    import analysis
    import asset_release
    source_root = (ROOT / "assets/synapse/tables").resolve()
    destination_root = (ROOT / asset_release.TABLES).resolve()
    destination_root.mkdir(parents=True, exist_ok=True)
    moved = []
    for source in sorted(source_root.glob("*.rs")):
        destination = asset_release.table_path(source.name)
        if (source.is_symlink() or source.resolve().parent != source_root
                or destination.resolve().parent != destination_root):
            raise ValueError("Resource maintenance cleanup escaped its owned directories")
        raw = source.read_bytes()
        if destination.exists() and destination.read_bytes() != raw:
            raise ValueError("Conflicting resource maintenance input: " + source.name)
        destination.write_bytes(raw)
        if source.read_bytes() != destination.read_bytes():
            raise ValueError("Resource table changed during maintenance migration")
        moved.append({"original": source.relative_to(ROOT).as_posix(), "path": destination.relative_to(ROOT).as_posix(),
                      "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
        source.unlink()
    if source_root.exists() and not any(source_root.iterdir()):
        source_root.rmdir()
    lock = asset_release.load_lock()
    retained = [row for row in lock["git_files"] if not row["path"].startswith("assets/synapse/tables/")]
    if retained != lock["git_files"]:
        lock["git_files"] = retained
        lock["source_sha256"] = asset_release.content_hash({"groups": lock["groups"], "git_files": retained})
        lock["tag"] = "assets-" + lock["source_sha256"][:20]
        asset_release.validate_lock(lock)
        analysis.save(ROOT / asset_release.LOCK, lock)
    if moved:
        path = ROOT / ".analysis/results/typed-data.json"
        report = json.loads(path.read_bytes()) if path.is_file() else {"schema_version": 1}
        report["resource_table_maintenance_migration"] = {"files": moved, "byte_identity_verified": True}
        analysis.save(path, report)
    return moved


def selector_rows(manifest, input_rows=None):
    import re
    import asset_release
    aliases = {row["key"] for row in manifest["entries"]}
    token = re.compile(r'\s*(\d+|true|false|DeviceImage::[A-Za-z]+|"(?:[^"\\]|\\.)*")\s*')
    tables = {}
    for table, (sources, types) in SELECTORS.items():
        rows = []
        for filename in sources:
            path = asset_release.table_path(filename)
            raw = path.read_bytes()
            if input_rows is not None:
                input_rows.append({"path": path.relative_to(ROOT).as_posix(), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
            body = re.sub(r"(?m)^\s*//[^\n]*", "", raw.decode("utf-8")).strip()
            if not body.startswith("&[") or not body.endswith("]"):
                raise ValueError("Selector source must be one complete array: " + filename)
            cursor = 2
            while body[cursor:].strip() != "]":
                start = re.match(r"\s*\(\s*", body[cursor:])
                if start is None:
                    raise ValueError("Unparsed selector row: " + filename)
                cursor += start.end()
                fields = []
                for index, typ in enumerate(types):
                    match = token.match(body, cursor)
                    if match is None:
                        raise ValueError("Unparsed selector field: " + filename)
                    literal = match[1]
                    cursor = match.end()
                    if typ in {"text", "kind"}:
                        value = json.loads(literal)
                        if type(value) is not str or (typ == "kind" and value not in {"prd", "ActuationDevice"}):
                            raise ValueError("Invalid selector text")
                    elif typ == "part":
                        value = literal.removeprefix("DeviceImage::")
                        if value not in {"Product", "MouseBottom", "KeyboardWrist", "KeyboardDial", "Streamer"}:
                            raise ValueError("Unknown device image part")
                    elif typ == "bool":
                        if literal not in {"true", "false"}:
                            raise ValueError("Invalid selector wrist flag")
                        value = int(literal == "true")
                    else:
                        if not literal.isdecimal() or not 0 <= int(literal) < 2**32:
                            raise ValueError("Invalid selector identity")
                        value = int(literal)
                        if typ == "density" and value not in {1, 3}:
                            raise ValueError("Invalid selector density")
                    fields.append(value)
                    if index < len(types) - 1:
                        if body[cursor:cursor + 1] != ",":
                            raise ValueError("Incomplete selector fields")
                        cursor += 1
                end = re.match(r"\s*,?\s*\)\s*,?\s*", body[cursor:])
                if end is None or fields[-1] not in aliases or fields[0] <= 0:
                    raise ValueError("Invalid selector closure or unregistered asset: " + filename)
                cursor += end.end()
                rows.append((len(rows), *fields))
        if not rows:
            raise ValueError("Empty maintained selector table: " + table)
        tables[table] = rows
    return tables


def fill_selectors(connection, tables):
    for name, rows in tables.items():
        placeholders = ",".join("?" for _ in rows[0])
        connection.executemany("INSERT INTO " + name + " VALUES (" + placeholders + ")", rows)
        restored = connection.execute("SELECT * FROM " + name + " ORDER BY position").fetchall()
        if restored != rows:
            raise ValueError("Selector fields, order or duplicates changed: " + name)


def emoji_datasets():
    import rust_data
    data = {name: json.loads((ROOT / path).read_bytes()) for name, path in INPUTS.items()}
    mapping = data["mapping"]
    alternate = data["mouse"]["datasets"]
    if len(alternate) != 2 or alternate[0] != mapping or data["shortcuts"]["emoji"] != mapping:
        raise ValueError("Reaudit emoji consumer branches: shared mapping data changed")
    datasets = {"mapping": mapping, "mapping_without_exclusions": alternate[1], "macro": data["macro"]}
    _, search = rust_data.emoji_search_input()
    for name, value in datasets.items():
        macro = name == "macro"
        exclusion = "excluded_windows_11" if macro else "excluded_without_windows_10"
        if set(value) != {"groups", "tabs", "search", "variants", exclusion}:
            raise ValueError("Unknown emoji dataset fields: " + name)
        if value["search"] != search or len(value["groups"]) != len(value["tabs"]) or not value["groups"]:
            raise ValueError("Emoji search or category branches differ: " + name)
        for group in value["groups"]:
            if set(group) != ({"label", "name", "values"} if macro else {"values"}):
                raise ValueError("Unknown emoji group fields")
            if not isinstance(group["values"], list) or not all(type(v) is str for v in group["values"]):
                raise ValueError("Invalid ordered emoji group")
            if macro and not all(type(group[k]) is str for k in ("label", "name")):
                raise ValueError("Invalid macro category label")
        for tab in value["tabs"]:
            if set(tab) != ({"emo", "name", "pos"} if macro else {"emo", "name"}):
                raise ValueError("Unknown emoji tab fields")
            if not all(type(tab[k]) is str for k in ("emo", "name")):
                raise ValueError("Invalid emoji tab")
            if macro and (type(tab["pos"]) is not int or not 0 <= tab["pos"] < len(value["groups"])):
                raise ValueError("Invalid macro tab position")
        if (type(value["variants"]) is not dict
                or any(type(k) is not str or type(v) is not list or not v
                       or any(type(s) is not str for s in v) for k, v in value["variants"].items())
                or type(value[exclusion]) is not list or any(type(s) is not str for s in value[exclusion])):
            raise ValueError("Invalid emoji variants/exclusions")
    return datasets, search


def emoji_dataset_identity(value):
    datasets, _ = emoji_datasets()
    names = {"mapping": "Mapping", "mapping_without_exclusions": "MappingWithoutExclusions", "macro": "Macro"}
    for name, dataset in datasets.items():
        if value == dataset:
            return names[name]
    raise ValueError("Emoji dataset does not match a complete audited database profile")


def fill(connection, manifest, datasets, search):
    connection.executescript((ROOT / SCHEMA).read_text(encoding="utf-8"))
    connection.execute("PRAGMA application_id = " + str(APPLICATION_ID))
    connection.execute("PRAGMA user_version = " + str(VERSION))
    storage = manifest["storage"]
    connection.execute("INSERT INTO resource_storage VALUES (1, ?, ?, ?, ?)",
                       (manifest["schema_version"], storage["provider"], storage["repository"], storage["revision"]))
    files = {}
    for position, row in enumerate(manifest["files"]):
        path = row["path"]
        identity = (row["bytes"], row["sha256"])
        if path in files and files[path][1] != identity:
            raise ValueError("Conflicting resource identity: " + path)
        if path not in files:
            file_id = len(files)
            files[path] = (file_id, identity)
            connection.execute("INSERT INTO resource_file VALUES (?, ?, ?, ?)", (file_id, path, *identity))
        connection.execute("INSERT INTO resource_file_order VALUES (?, ?)", (position, files[path][0]))
    for position, row in enumerate(manifest["entries"]):
        file_id, identity = files[row["path"]]
        if identity != (row["bytes"], row["sha256"]):
            raise ValueError("Resource alias identity differs from file")
        connection.execute("INSERT INTO resource_alias VALUES (?, ?, ?, ?, ?, ?)",
                           (position, row["key"], file_id, row.get("view_box"), row.get("width"), row.get("height")))
    connection.executemany("INSERT INTO resource_list VALUES (?, ?, ?)",
                           ((i, row["key"], row["provider"]) for i, row in enumerate(manifest["list_entries"])))
    connection.executemany("INSERT INTO resource_font VALUES (?, ?)",
                           ((i, files[row["path"]][0]) for i, row in enumerate(manifest["fonts"])))
    unique = {}
    for position, row in enumerate(search):
        entry = (row["emo"], row["name"])
        if entry not in unique:
            unique[entry] = len(unique)
            connection.execute("INSERT INTO emoji_entry VALUES (?, ?, ?)", (unique[entry], *entry))
        connection.execute("INSERT INTO emoji_search VALUES (?, ?)", (position, unique[entry]))
    groups, variants, exclusions = {}, {}, {}
    for name, value in datasets.items():
        macro = name == "macro"
        variant_key = tuple((key, tuple(items)) for key, items in sorted(value["variants"].items()))
        if variant_key not in variants:
            variant_id = len(variants)
            variants[variant_key] = variant_id
            connection.execute("INSERT INTO emoji_variant_set VALUES (?)", (variant_id,))
            connection.executemany("INSERT INTO emoji_variant VALUES (?, ?, ?, ?)",
                                   ((variant_id, base, i, item) for base, items in variant_key for i, item in enumerate(items)))
        exclusion_key = tuple(value["excluded_windows_11" if macro else "excluded_without_windows_10"])
        if exclusion_key not in exclusions:
            exclusion_id = len(exclusions)
            exclusions[exclusion_key] = exclusion_id
            connection.execute("INSERT INTO emoji_exclusion_set VALUES (?)", (exclusion_id,))
            connection.executemany("INSERT INTO emoji_exclusion VALUES (?, ?, ?)",
                                   ((exclusion_id, i, item) for i, item in enumerate(exclusion_key)))
        connection.execute("INSERT INTO emoji_dataset VALUES (?, ?, ?, ?)",
                           (name, variants[variant_key], exclusions[exclusion_key],
                            "windows_11" if macro else "without_windows_10"))
        for position, group in enumerate(value["groups"]):
            group_key = tuple(group["values"])
            if group_key not in groups:
                groups[group_key] = len(groups)
                connection.execute("INSERT INTO emoji_group_list VALUES (?)", (groups[group_key],))
                connection.executemany("INSERT INTO emoji_group_value VALUES (?, ?, ?)",
                                       ((groups[group_key], i, item) for i, item in enumerate(group_key)))
            connection.execute("INSERT INTO emoji_group VALUES (?, ?, ?, ?, ?)",
                               (name, position, groups[group_key], group.get("label"), group.get("name")))
        connection.executemany("INSERT INTO emoji_tab VALUES (?, ?, ?, ?, ?)",
                               ((name, i, tab["emo"], tab["name"], tab.get("pos")) for i, tab in enumerate(value["tabs"])))
    connection.commit()


def verify_rows(connection, manifest, datasets, search):
    """Full roundtrip comparison, preserving every duplicate and source order."""
    row = connection.execute("SELECT schema_version, provider, repository, revision FROM resource_storage").fetchone()
    restored = {"schema_version": row[0], "storage": dict(zip(("provider", "repository", "revision"), row[1:]))}
    files = "f.path, f.bytes, f.sha256"
    restored["files"] = [dict(zip(("path", "bytes", "sha256"), row)) for row in connection.execute(
        "SELECT " + files + " FROM resource_file_order o JOIN resource_file f ON f.id=o.file_id ORDER BY o.position")]
    restored["fonts"] = [dict(zip(("path", "bytes", "sha256"), row)) for row in connection.execute(
        "SELECT " + files + " FROM resource_font o JOIN resource_file f ON f.id=o.file_id ORDER BY o.position")]
    restored["entries"] = []
    for row in connection.execute("SELECT a.key, " + files + ", a.view_box, a.width, a.height "
                                  "FROM resource_alias a JOIN resource_file f ON f.id=a.file_id ORDER BY a.position"):
        restored["entries"].append({key: item for key, item in zip(
            ("key", "path", "bytes", "sha256", "view_box", "width", "height"), row) if item is not None})
    restored["list_entries"] = [dict(zip(("key", "provider"), row)) for row in connection.execute(
        "SELECT key, provider FROM resource_list ORDER BY position")]
    if restored != manifest:
        raise ValueError("Complete database resource index differs from source semantics")
    recovered = [dict(zip(("emo", "name"), row)) for row in connection.execute(
        "SELECT e.emoji, e.name FROM emoji_search s JOIN emoji_entry e ON e.id=s.entry_id ORDER BY s.position")]
    if recovered != search:
        raise ValueError("Database emoji vocabulary changed order, duplicates or text")
    for name, original in datasets.items():
        variant_set, exclusion_set, rule = connection.execute(
            "SELECT variant_set, exclusion_set, exclusion_rule FROM emoji_dataset WHERE id=?", (name,)).fetchone()
        macro = rule == "windows_11"
        groups = []
        for values_id, label, label_key in connection.execute(
                "SELECT values_id, label, name FROM emoji_group WHERE dataset_id=? ORDER BY position", (name,)):
            group = {"values": [row[0] for row in connection.execute(
                "SELECT value FROM emoji_group_value WHERE list_id=? ORDER BY position", (values_id,))]}
            if macro:
                group.update(label=label, name=label_key)
            groups.append(group)
        tabs = [{key: item for key, item in zip(("emo", "name", "pos"), row) if item is not None}
                for row in connection.execute("SELECT emoji, name, target_position FROM emoji_tab "
                                               "WHERE dataset_id=? ORDER BY position", (name,))]
        variants = {}
        for base, item in connection.execute("SELECT base, value FROM emoji_variant WHERE set_id=? ORDER BY base, position", (variant_set,)):
            variants.setdefault(base, []).append(item)
        value = {"groups": groups, "tabs": tabs, "search": recovered, "variants": variants,
                 "excluded_windows_11" if macro else "excluded_without_windows_10": [row[0] for row in connection.execute(
                     "SELECT value FROM emoji_exclusion WHERE set_id=? ORDER BY position", (exclusion_set,))]}
        if value != original:
            raise ValueError("Complete database emoji profile differs: " + name)
    if connection.execute("PRAGMA integrity_check").fetchall() != [("ok",)] or connection.execute("PRAGMA foreign_key_check").fetchall():
        raise ValueError("Resource database integrity/foreign keys failed")


def prepare(*, check=False, sources=None, check_local=False, migrate_data=False):
    import analysis
    import asset_release
    import rust_data
    import catalog_data
    if migrate_data:
        if check:
            raise ValueError("A migration cannot run in check-only mode")
        migrate_resource_tables()
    registry = ROOT / "tools/typed-data.json"
    recipes = json.loads(registry.read_bytes())
    documents, adapters, typed_reports = catalog_data.prepare_documents(recipes, migrate=migrate_data, check=check)
    sources = sources or recipes.get("emoji_search", {}).get("original_sources", [])
    if not sources:
        raise ValueError("Register retained original emoji JS with --source")
    datasets, search = emoji_datasets()
    evidence = rust_data.verify_emoji_search_sources(sources, search)
    table_inputs = []
    manifest = asset_release.runtime_metadata(check_local=check_local, input_rows=table_inputs)
    selectors = selector_rows(manifest, table_inputs)
    with sqlite3.connect(":memory:") as connection:
        connection.execute("PRAGMA page_size=4096")
        connection.execute("PRAGMA foreign_keys=ON")
        fill(connection, manifest, datasets, search)
        fill_selectors(connection, selectors)
        connection.executemany("INSERT INTO typed_catalog VALUES (?, ?, ?, ?)",
                               ((doc["id"], doc["codec"], hashlib.sha256(doc["raw"]).hexdigest(), doc["raw"]) for doc in documents))
        connection.commit()
        restored_documents = connection.execute("SELECT id, codec, sha256, payload FROM typed_catalog ORDER BY id").fetchall()
        expected_documents = sorted((doc["id"], doc["codec"], hashlib.sha256(doc["raw"]).hexdigest(), doc["raw"]) for doc in documents)
        if restored_documents != expected_documents:
            raise ValueError("Complete typed catalog byte roundtrip differs")
        verify_rows(connection, manifest, datasets, search)
        connection.execute("VACUUM")
        raw = connection.serialize()
    catalog_data.publish_adapters(adapters, recipes, check=check)
    output = ROOT / OUTPUT
    if check:
        if output.read_bytes() != raw:
            raise ValueError("Packaged resource database differs; regenerate through workspace.py")
    elif not output.is_file() or output.read_bytes() != raw:
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary = output.with_suffix(".sqlite3.partial")
        try:
            temporary.write_bytes(raw)
            temporary.replace(output)
        finally:
            temporary.unlink(missing_ok=True)
    report = {"output": OUTPUT, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
              "schema_version": VERSION, "sqlite_generator": sqlite3.sqlite_version,
              "emoji_search_entries": len(search), "emoji_unique_entries": len({(r['emo'], r['name']) for r in search}),
              "emoji_profiles": list(datasets), "aliases": len(manifest["entries"]),
              "list_entries": len(manifest["list_entries"]), "files": len(manifest["files"]),
              "fonts": len(manifest["fonts"]), "original_sources": sources,
              "typed_catalogs": typed_reports,
              "selectors": {name: len(rows) for name, rows in selectors.items()},
              "inputs": [{"path": path, "bytes": (ROOT / path).stat().st_size,
                          "sha256": hashlib.sha256((ROOT / path).read_bytes()).hexdigest()}
                         for path in [SCHEMA, INSTALLATION, rust_data.EMOJI_SEARCH_INPUT, *INPUTS.values(), asset_release.LOCK,
                                      "tools/resource_database.py", "tools/asset_release.py", "tools/rust_data.py", "tools/catalog_data.py",
                                      *[item["source"] for item in recipes["tables"] if catalog_data.selected(item)]]]
                        + table_inputs,
              "full_ordered_roundtrip_verified": True, "passed": True}
    if not check:
        with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
            recipes["emoji_search"] = {"source": rust_data.EMOJI_SEARCH_INPUT, "output": OUTPUT,
                                        "original_sources": sources, "passed": True}
            analysis.save(registry, recipes)
        result = ROOT / ".analysis/results/typed-data.json"
        previous = json.loads(result.read_bytes()) if result.is_file() else {"schema_version": 1}
        previous["emoji_search_sources"] = evidence
        previous["resource_database"] = report
        analysis.save(result, previous)
    return {**report, "check_only": check}


def add_commands(commands):
    command = commands.add_parser("prepare-resource-database", help="Prepare/verify packaged emoji profiles and resource index without media downloads")
    command.add_argument("--check", action="store_true")
    command.add_argument("--migrate-data", action="store_true", help="Recover and verify complete current typed constructors before replacing them with SQLite adapters")


def run(args):
    return prepare(check=args.check, migrate_data=args.migrate_data)

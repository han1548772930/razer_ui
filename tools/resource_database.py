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
VERSION = 4
LOCALES = ".analysis/inputs/typed-tables/razer-i18n/locales.json"
LOCALE_BASELINE = ".analysis/inputs/typed-tables/razer-i18n/baseline"
LOCALE_REPORT = ".analysis/results/locale-preparation.json"
PRODUCT_REGISTRY = ".analysis/inputs/typed-tables/razer-catalog/product_registry.json"
PRODUCT_REGISTRY_EVIDENCE = ".analysis/results/registry-source-evidence.json"
PRODUCT_HELP = ".analysis/inputs/typed-tables/razer-pages/source_help_data.json"
INPUTS = {
    "mapping": ".analysis/inputs/typed-tables/razer-pages/keyboard_mapping_text_emoji_data.json",
    "mouse": ".analysis/inputs/typed-tables/razer-pages/mouse_mapping_text_current_data.json",
    "shortcuts": ".analysis/inputs/typed-tables/razer-pages/shortcuts_current.json",
    "macro": ".analysis/inputs/typed-tables/razer-app-pages/text_data.json",
}


def unique_json(raw):
    """Maintenance files must not discard a repeated field during JSON parsing."""
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("Duplicate locale maintenance field: " + key)
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs)


def flatten_locale(document):
    """Preserve the locked i18n dotted namespace representation of legacy words."""
    if type(document) is not dict or not document:
        raise ValueError("Locale dictionary is not a nonempty object")
    result = {}

    def visit(value, prefix):
        if type(value) is str:
            if not prefix or prefix in result:
                raise ValueError("Empty or colliding locale key: " + prefix)
            result[prefix] = value
        elif type(value) is dict and value:
            for key in sorted(value):
                if type(key) is not str or not key:
                    raise ValueError("Invalid locale namespace key")
                visit(value[key], prefix + "." + key if prefix else key)
        else:
            raise ValueError("Locale words must be string leaves in nonempty namespaces")

    visit(document, "")
    return dict(sorted(result.items()))


def validate_locales(document):
    import re
    if (type(document) is not dict or set(document) != {"schema_version", "locales"}
            or type(document["schema_version"]) is not int or document["schema_version"] != 1
            or type(document["locales"]) is not list or not 0 < len(document["locales"]) <= 128):
        raise ValueError("Invalid locale maintenance schema")
    identities = []
    for row in document["locales"]:
        if (type(row) is not dict or set(row) != {"locale", "messages"}
                or type(row["locale"]) is not str or len(row["locale"]) > 64
                or not re.fullmatch(r"[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*", row["locale"])
                or type(row["messages"]) is not dict or not 0 < len(row["messages"]) <= 1_000_000):
            raise ValueError("Invalid locale identity or dictionary")
        identities.append(row["locale"])
        for key, value in row["messages"].items():
            if (type(key) is not str or not 0 < len(key.encode("utf-8")) <= 4096
                    or type(value) is not str or len(value.encode("utf-8")) > 1024 * 1024):
                raise ValueError("Invalid locale key or value")
        if list(row["messages"]) != sorted(row["messages"]):
            raise ValueError("Locale key order is not canonical: " + row["locale"])
    if identities != sorted(set(identities)) or "en" not in identities:
        raise ValueError("Locale order is repeated, noncanonical or lacks English")
    return document["locales"]


def localization_document(path=None):
    return validate_locales(unique_json((path or ROOT / LOCALES).read_bytes()))


def verify_locales(connection, locales):
    restored_locales = connection.execute("SELECT id, position FROM ui_locale ORDER BY position").fetchall()
    expected_locales = [(row["locale"], position) for position, row in enumerate(locales)]
    expected_messages = [(row["locale"], position, key, value) for row in locales
                         for position, (key, value) in enumerate(row["messages"].items())]
    restored_messages = connection.execute(
        "SELECT locale, position, key, value FROM ui_translation ORDER BY locale, position").fetchall()
    if restored_locales != expected_locales or restored_messages != expected_messages:
        raise ValueError("Complete locale fields, namespace, order or text differ")


def fill_locales(connection, locales):
    connection.executemany("INSERT INTO ui_locale VALUES (?, ?)",
                           ((row["locale"], position) for position, row in enumerate(locales)))
    connection.executemany("INSERT INTO ui_translation VALUES (?, ?, ?, ?)",
                           ((row["locale"], position, key, value) for row in locales
                            for position, (key, value) in enumerate(row["messages"].items())))
    verify_locales(connection, locales)
    return {row["locale"]: len(row["messages"]) for row in locales}


def product_registry(path=None):
    """Validated maintenance source; the installed product only reads SQLite."""
    document = json.loads((path or ROOT / PRODUCT_REGISTRY).read_bytes())
    return validate_product_registry(document)


def validate_product_registry(document):
    if type(document) is not dict or set(document) != {"schema_version", "products"} or document["schema_version"] != 1:
        raise ValueError("Invalid product registry maintenance schema")
    products = document["products"]
    if type(products) is not list or not products:
        raise ValueError("Empty product registry")
    ids, navigation_keys, page_keys = set(), set(), set()
    for product in products:
        if (type(product) is not dict or set(product) != {"id", "name", "categories", "edition_ids", "navigations"}
                or type(product["id"]) is not int or not 0 < product["id"] <= 0xFFFFFFFF
                or product["id"] in ids or type(product["name"]) is not str or not product["name"]):
            raise ValueError("Invalid or duplicate product registry identity")
        ids.add(product["id"])
        for key, typ in (("categories", str), ("edition_ids", int)):
            values = product[key]
            if (type(values) is not list or not values or any(type(value) is not typ for value in values)
                    or len(values) != len(set(values)) or (typ is str and any(not value for value in values))
                    or (typ is int and any(not 0 <= value <= 0xFFFFFFFF for value in values))):
                raise ValueError("Invalid product registry " + key)
        if type(product["navigations"]) is not list or not product["navigations"]:
            raise ValueError("Product has no navigation branches")
        primary_count = 0
        for navigation in product["navigations"]:
            if (type(navigation) is not dict or set(navigation) != {"key", "display_mode", "primary", "pages"}
                    or type(navigation["key"]) is not str or not navigation["key"] or navigation["key"] in navigation_keys
                    or type(navigation["display_mode"]) is not str or not navigation["display_mode"]
                    or type(navigation["primary"]) is not bool):
                raise ValueError("Invalid or duplicate product navigation")
            navigation_keys.add(navigation["key"])
            primary_count += navigation["primary"]
            if type(navigation["pages"]) is not list or not navigation["pages"]:
                raise ValueError("Navigation branch has no pages")
            for page in navigation["pages"]:
                if (type(page) is not dict or set(page) != {"key", "kind", "role", "help_selector"}
                        or type(page["key"]) is not str or not page["key"] or page["key"] in page_keys
                        or type(page["kind"]) is not str or not page["kind"]
                        or page["role"] not in {"Page", "Help", "StandaloneMode"}
                        or type(page["help_selector"]) is not int or not 0 <= page["help_selector"] <= 0x7FFFFFFFFFFFFFFF):
                    raise ValueError("Invalid or duplicate product page")
                page_keys.add(page["key"])
        if primary_count != 1:
            raise ValueError("Product must retain exactly one primary navigation")
    if [row["id"] for row in products] != sorted(ids):
        raise ValueError("Product registry order is not strictly increasing")
    return products


def validate_product_help_bindings(products, help_records):
    """Help selectors address retained local presentation records, not JS offsets."""
    selectors = {}
    for record in help_records:
        pid = record["product_id"]
        values = [page["offset"] for page in record["pages"]]
        if pid in selectors or len(values) != len(set(values)):
            raise ValueError("Ambiguous product Help records")
        selectors[pid] = set(values)
    count = 0
    for product in products:
        for navigation in product["navigations"]:
            for page in navigation["pages"]:
                if page["role"] != "Help":
                    continue
                if page["help_selector"] not in selectors.get(product["id"], set()):
                    raise ValueError(f"Product {product['id']} Help selector has no local presentation record: {page['key']}")
                count += 1
    return count


def product_registry_tables(products):
    tables = {name: [] for name in ("ui_product", "ui_product_category", "ui_product_edition", "ui_navigation", "ui_page")}
    for product in products:
        pid = product["id"]
        tables["ui_product"].append((pid, product["name"]))
        tables["ui_product_category"].extend((pid, index, value) for index, value in enumerate(product["categories"]))
        tables["ui_product_edition"].extend((pid, index, value) for index, value in enumerate(product["edition_ids"]))
        for index, navigation in enumerate(product["navigations"]):
            tables["ui_navigation"].append((navigation["key"], pid, index, navigation["display_mode"], int(navigation["primary"])))
            tables["ui_page"].extend((page["key"], navigation["key"], position, page["kind"], page["role"], page["help_selector"])
                                     for position, page in enumerate(navigation["pages"]))
    return tables


def fill_product_registry(connection, products):
    tables = product_registry_tables(products)
    orders = {"ui_product": "id", "ui_product_category": "product_id, position", "ui_product_edition": "product_id, position",
              "ui_navigation": "product_id, position", "ui_page": "navigation_key, position"}
    for name, rows in tables.items():
        connection.executemany("INSERT INTO " + name + " VALUES (" + ",".join("?" for _ in rows[0]) + ")", rows)
        restored = connection.execute("SELECT * FROM " + name + " ORDER BY " + orders[name]).fetchall()
        expected = sorted(rows, key=(lambda row: (row[1], row[2])) if name == "ui_page" else
                          (lambda row: (row[1], row[2])) if name == "ui_navigation" else (lambda row: row))
        if restored != expected:
            raise ValueError("Complete product registry fields/order/branches differ: " + name)
    return {name: len(rows) for name, rows in tables.items()}


def selector_rows(manifest, input_rows=None):
    import asset_release
    mappings = asset_release.load_resource_mappings(input_rows=input_rows)
    aliases = {row["key"] for row in manifest["entries"]}
    tables = {}
    for table, columns in asset_release.SELECTOR_FIELDS.items():
        rows = []
        for position, row in enumerate(mappings["selectors"][table]):
            if row["key"] not in aliases:
                raise ValueError("Selector references an unregistered asset: " + row["key"])
            rows.append((position, *(int(row[column]) if column == "wrist" else row[column] for column in columns)))
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
    products = product_registry()
    locales = localization_document()
    help_bindings = validate_product_help_bindings(products, json.loads((ROOT / PRODUCT_HELP).read_bytes()))
    if migrate_data:
        if check:
            raise ValueError("A migration cannot run in check-only mode")
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
        locale_counts = fill_locales(connection, locales)
        product_counts = fill_product_registry(connection, products)
        fill_selectors(connection, selectors)
        connection.executemany("INSERT INTO typed_catalog VALUES (?, ?, ?, ?)",
                               ((doc["id"], doc["codec"], hashlib.sha256(doc["raw"]).hexdigest(), doc["raw"]) for doc in documents))
        connection.commit()
        restored_documents = connection.execute("SELECT id, codec, sha256, payload FROM typed_catalog ORDER BY id").fetchall()
        expected_documents = sorted((doc["id"], doc["codec"], hashlib.sha256(doc["raw"]).hexdigest(), doc["raw"]) for doc in documents)
        if restored_documents != expected_documents:
            raise ValueError("Complete typed catalog byte roundtrip differs")
        verify_rows(connection, manifest, datasets, search)
        verify_locales(connection, locales)
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
              "localization": {"source": LOCALES, "languages": locale_counts,
                               "full_field_namespace_and_order_roundtrip_verified": True},
              "product_registry": {"source": PRODUCT_REGISTRY, "tables": product_counts,
                                    "full_field_and_branch_roundtrip_verified": True,
                                    "help_presentation_bindings_verified": help_bindings},
              "selectors": {name: len(rows) for name, rows in selectors.items()},
              "inputs": [{"path": path, "bytes": (ROOT / path).stat().st_size,
                          "sha256": hashlib.sha256((ROOT / path).read_bytes()).hexdigest()}
                         for path in [SCHEMA, INSTALLATION, LOCALES, PRODUCT_REGISTRY, rust_data.EMOJI_SEARCH_INPUT, *INPUTS.values(), asset_release.LOCK,
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

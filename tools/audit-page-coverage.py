"""Report product pages that still render a local placeholder.

Static parsing only. For every generated product family this script compares the
page keys a product declares in its own bundle data with the keys the local
renderer actually dispatches:

* mouse / keyboard / gamepad / system / accessory_system dispatch on
  `self.page.as_str()`; a key without an arm falls through to the placeholder.
* audio renders generically from its generated spec, so a page without any
  section (and without a demo view) is the placeholder case.

Usage:
    python tools/audit-page-coverage.py            # table
    python tools/audit-page-coverage.py --json     # machine-readable, same data
    python tools/audit-page-coverage.py --self-test  # parser and guard-order rules
"""
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# `--self-test` drives the parser over synthetic dispatch bodies instead of the
# repository sources, so the guard-order rule has a receipt that does not need to
# rewrite a source file to prove it can fail.
SELF_TEST_BODIES = [
    # Implemented pages before the descriptor guard: the expected shape.
    (
        "implemented-first",
        """match self.page.as_str() {
            (3858, "TAB_GAMING") => self.monitor_gaming(cx),
            (3921, "TAB_CUSTOMIZE") => self.corex(cx),
            _ if !supports_page(self.spec.product_id, &self.page) => {
                surface::note("no descriptor", cx)
            }
            _ => self.fallback(cx),
        }""",
        [],
        [],
        [],
    ),
    # The guard hides every implemented page behind a note: must be reported.
    (
        "guard-first",
        """match self.page.as_str() {
            _ if !supports_page(self.spec.product_id, &self.page) => {
                surface::note("no descriptor", cx)
            }
            (3858, "TAB_GAMING") => self.monitor_gaming(cx),
            _ => self.fallback(cx),
        }""",
        [],
        ["TAB_GAMING"],
        [],
    ),
    # An arm that only renders a placeholder note is not a renderer.
    (
        "placeholder-arm",
        """match self.page.as_str() {
            "TAB_PAIRING" => surface::note("配对界面尚未接入。", cx),
            _ => self.fallback(cx),
        }""",
        [],
        [],
        ["TAB_PAIRING"],
    ),
    # A key with no arm at all stays on the placeholder.
    (
        "missing-arm",
        """match self.page.as_str() {
            "TAB_LIGHTING" => self.lighting(cx),
            _ => self.placeholder(cx),
        }""",
        ["TAB_OLED"],
        [],
        [],
    ),
]


def keys_in_arms(body: str):
    if body is None:
        return set()
    return {
        key for pattern, _ in arms(body) for key in re.findall(r'"([A-Za-z_0-9]+)"', pattern)
    }


def self_test() -> int:
    failures = 0
    for name, code, expected_unwired, expected_blocked, expected_placeholder in SELF_TEST_BODIES:
        body = match_body(code, r"self\.page\.as_str\(\)")
        handled, placeholder = handled_keys(code)
        blocked = guard_order_problems(body) if body else []
        declared = keys_in_arms(body) | set(expected_unwired)
        reported = sorted(key for key in declared if key not in handled and key not in placeholder)
        wanted_blocked = sorted(expected_blocked)
        blocked_matches = blocked == [wanted_blocked] if wanted_blocked else blocked == []
        ok = (reported == sorted(expected_unwired) and blocked_matches
              and sorted(placeholder) == sorted(expected_placeholder))
        if not ok:
            failures += 1
        print(
            f"[{'ok' if ok else 'FAIL'}] {name}: handled={sorted(handled)} "
            f"placeholder={sorted(placeholder)} blocked-behind-guard={blocked} "
            f"unwired={reported} expected unwired={sorted(expected_unwired)} "
            f"blocked={wanted_blocked}"
        )
    print(f"self-test failures: {failures} / {len(SELF_TEST_BODIES)}")
    return 1 if failures else 0

FAMILIES = {
    "mouse": ("crates/razer-pages/src/features/mouse_products.rs", "crates/razer-pages/src/features/mouse_products_data.json"),
    "keyboard": ("crates/razer-pages/src/features/keyboard_products.rs", "crates/razer-pages/src/features/keyboard_products_data.json"),
    "gamepad": ("crates/razer-pages/src/features/gamepad_products.rs", "crates/razer-pages/src/features/gamepad_products_data.json"),
    "system": ("crates/razer-pages/src/features/system_products.rs", "crates/razer-pages/src/features/system_products_data.json"),
    "accessory_system": (
        "crates/razer-pages/src/features/accessory_system_products.rs",
        "crates/razer-pages/src/features/accessory_system_products_data.json",
    ),
    "audio": ("crates/razer-pages/src/features/audio_products.rs", "crates/razer-pages/src/features/audio_products_data.json"),
}


def match_body(code: str, anchor: str):
    """Text inside the match block that starts after `anchor` (brace matched)."""
    match = re.search(anchor, code)
    if not match:
        return None
    brace = code.find("{", match.end())
    if brace < 0:
        return None
    depth = 0
    index = brace
    while index < len(code):
        if code[index] == "{":
            depth += 1
        elif code[index] == "}":
            depth -= 1
            if depth == 0:
                break
        index += 1
    return code[brace + 1:index]


def arms(body: str):
    """Split a match body into (pattern, arm body) pairs at depth 0.

    A block-bodied arm (`pat => { .. }`) ends at its own closing brace and needs
    no trailing comma, so the comma scan must not run past that brace.
    """
    result = []
    depth = 0
    index = 0
    previous_end = 0
    while index < len(body):
        character = body[index]
        if character in "([{":
            depth += 1
        elif character in ")]}":
            depth -= 1
        elif character == "=" and body[index:index + 2] == "=>" and depth == 0:
            pattern = body[previous_end:index]
            cursor = index + 2
            while cursor < len(body) and body[cursor] in " \t\r\n":
                cursor += 1
            if cursor < len(body) and body[cursor] == "{":
                inner = 0
                while cursor < len(body):
                    current = body[cursor]
                    if current == "{":
                        inner += 1
                    elif current == "}":
                        inner -= 1
                        if inner == 0:
                            cursor += 1
                            break
                    cursor += 1
            else:
                inner = 0
                while cursor < len(body):
                    current = body[cursor]
                    if current in "([{":
                        inner += 1
                    elif current in ")]}":
                        inner -= 1
                    elif current == "," and inner == 0:
                        break
                    cursor += 1
            result.append((pattern, body[index + 2:cursor]))
            previous_end = cursor
            index = cursor
        index += 1
    return result


PLACEHOLDER = re.compile(r"尚未接入|仍在接入|没有本地描述符|尚未完成|不可用")


def handled_keys(code: str, anchor: str = r"self\.page\.as_str\(\)"):
    """Page keys whose dispatch arm renders real content.

    Keys whose arm body is only a placeholder note are reported separately by
    `placeholder_keys`, so both states stay visible.
    """
    body = match_body(code, anchor)
    if body is None:
        return set(), set()
    handled, placeholder = set(), set()
    for pattern, arm_body in arms(body):
        keys = set(re.findall(r'"([A-Za-z_0-9]+)"', pattern))
        if not keys:
            continue
        (placeholder if PLACEHOLDER.search(arm_body) else handled).update(keys)
    return handled, placeholder


def guard_order_problems(body: str):
    """Key groups dispatched after a descriptor guard, so never reachable.

    A descriptor guard (`_ if !supports_page(..)`) must follow every implemented
    arm: the monitor and cooling pages have renderers without generated
    descriptors, so a leading guard would hide them behind a note.
    """
    problems = []
    seen_guard = False
    for pattern, _ in arms(body):
        keys = set(re.findall(r'"([A-Za-z_0-9]+)"', pattern))
        if "!supports_page" in pattern:
            seen_guard = True
        elif keys and seen_guard:
            problems.append(sorted(keys))
    return problems


def pages_of(product: dict):
    pages = []
    for entry in product.get("pages", []):
        if isinstance(entry, str):
            pages.append({"key": entry, "sections": None})
        elif isinstance(entry, dict):
            pages.append(
                {"key": entry.get("kind") or entry.get("key"), "sections": entry.get("sections")}
            )
    return pages


def standalone_pages() -> dict:
    """Page kinds that only exist inside a non-primary standalone navigation.

    The source registers these under a `displayMode` navigation
    (e.g. `multiDevicePairing`), so they never appear in the product tab strip and
    never reach the family renderer: the standalone root implements them. Both
    facts come from the generated registry.
    """
    text = (ROOT / "crates/razer-catalog/src/registry_data.rs").read_text(encoding="utf-8")
    navigation = re.compile(
        r'ProductNavigation \{ key: "(?P<key>[^"]+)", owner: "(?P<owner>[^"]*)", '
        r'display_mode: "(?P<mode>[^"]*)", source: "(?P<source>[^"]*)", '
        r'source_sha256: "(?P<sha>[^"]*)", offset: (?P<offset>\d+), '
        r'primary: (?P<primary>true|false), reachability: "(?P<reach>(?:[^"\\]|\\.)*)"',
        re.S,
    )
    page = re.compile(r'kind: ProductPageKind\("(?P<kind>[^"]*)"\), role: ProductPageRole::(?P<role>\w+)')
    matches = list(navigation.finditer(text))
    standalone = {}
    for index, match in enumerate(matches):
        row = match.groupdict()
        if row["primary"] == "true" or row["mode"] == "default":
            continue
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        product_id = re.search(r"product-(\d+)-", row["key"])
        if not product_id:
            continue
        for entry in page.finditer(text[match.end():end]):
            if entry.group("role") == "StandaloneMode":
                standalone.setdefault(entry.group("kind"), set()).add(int(product_id.group(1)))
    return standalone


STANDALONE = standalone_pages()


def supplement_pages() -> dict:
    """(product id, page key) pairs the shared control supplement renders.

    `SourceProductWorkspace::use_supplement_for` routes a page to the supplement
    entity before the family renderer when the supplement has descriptors for it:
    keyboards only for `OLED`, audio and accessory-system products for any page
    the family does not implement itself.
    """
    supported = defaultdict(set)
    for name in (
        "source_controls_data.json",
        "keyboard_oled_data.json",
        "accessory_controls_data.json",
    ):
        data = json.loads((ROOT / "src/features" / name).read_text(encoding="utf-8"))
        for product in data:
            for page in product.get("pages", []):
                if page.get("sections"):
                    supported[product["product_id"]].add(page.get("key"))
    return supported


SUPPLEMENT = supplement_pages()
AUDIO_DEMOS = {
    product["product_id"]
    for product in json.loads(
        (ROOT / "crates/razer-pages/src/features/audio_demo_data.json").read_text(encoding="utf-8")
    )
}


def supplement_handles(family: str, pid: int, key: str, family_handled: bool) -> bool:
    if key not in SUPPLEMENT.get(pid, set()):
        return False
    if family == "keyboard":
        return key == "OLED"
    if family in ("audio", "accessory_system"):
        return not family_handled
    return False


if "--self-test" in sys.argv:
    raise SystemExit(self_test())

report = []
placeholder_report = []
order_problems = []
checked_slots = 0
checked_products = 0
for family, (code_path, data_path) in FAMILIES.items():
    code = (ROOT / code_path).read_text(encoding="utf-8")
    handled, placeholder = handled_keys(code)
    body = match_body(code, r"self\.page\.as_str\(\)")
    if body:
        order_problems += [
            f"{code_path}: {problem} after the descriptor guard"
            for problem in guard_order_problems(body)
        ]
    data = json.loads((ROOT / data_path).read_text(encoding="utf-8"))
    unwired = defaultdict(list)
    for product in data:
        checked_products += 1
        for page in pages_of(product):
            key = page["key"]
            if key is None:
                continue
            # HELP pages never reach the family renderer: the workspace routes
            # every Help-role page to the shared help view before dispatch.
            if key == "HELP":
                continue
            checked_slots += 1
            if family == "audio":
                family_handled = bool(page.get("sections")) or (
                    key == "TAB_DEMO" and product["product_id"] in AUDIO_DEMOS
                )
            else:
                family_handled = key in handled
            if family_handled:
                continue
            if supplement_handles(family, product["product_id"], key, family_handled):
                continue
            unwired[key].append(product["product_id"])
            if key in placeholder:
                placeholder_report.append((family, key, product["product_id"]))

    for key, ids in sorted(unwired.items(), key=lambda item: (-len(item[1]), item[0])):
        standalone = STANDALONE.get(key, set())
        pages = sorted(ids)
        report.append(
            {
                "family": family,
                "page": key,
                "products": pages,
                "count": len(pages),
                # Products whose only use of this key is the standalone mode root.
                "standalone_products": [pid for pid in pages if pid in standalone],
                "placeholder_products": pages if key in placeholder else [],
            }
        )

report.sort(key=lambda row: (-row["count"], row["family"], row["page"]))
if "--json" in sys.argv:
    print(
        json.dumps(
            {
                "schema_version": 1,
                "checked_slots": checked_slots,
                "checked_products": checked_products,
                "gaps": report,
                "guard_order_problems": order_problems,
            },
            indent=2,
            ensure_ascii=False,
        )
    )
else:
    for problem in order_problems:
        print(f"guard order: {problem}")
    total = 0
    for row in report:
        note = ""
        if row["standalone_products"]:
            note = f"  standalone-mode only: {row['standalone_products']}"
        if row["placeholder_products"]:
            note += f"  arm renders a placeholder: {row['placeholder_products']}"
        print(
            f"{row['family']:18} {row['page']:22} products={row['count']:4} "
            f"{row['products'][:8]}{note}"
        )
        total += len(
            [
                pid
                for pid in row["products"]
                if pid not in row["standalone_products"] and pid not in row["placeholder_products"]
            ]
        )
    print(
        f"\nchecked {checked_slots} page slots on {checked_products} products; "
        f"slots with no renderer at all: {total} across {len(report)} page kinds; "
        f"placeholder-only slots: {len(placeholder_report)}; guard-order problems: {len(order_problems)}"
    )
if order_problems:
    raise SystemExit(1)

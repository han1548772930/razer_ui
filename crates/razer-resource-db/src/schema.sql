PRAGMA foreign_keys = ON;
CREATE TABLE ui_locale (
    id TEXT PRIMARY KEY CHECK(length(id)>0), position INTEGER NOT NULL UNIQUE CHECK(position>=0)
) STRICT;
CREATE TABLE ui_translation (
    locale TEXT NOT NULL REFERENCES ui_locale(id), position INTEGER NOT NULL CHECK(position>=0),
    key TEXT NOT NULL CHECK(length(key)>0), value TEXT NOT NULL,
    PRIMARY KEY(locale, key), UNIQUE(locale, position)
) STRICT;
CREATE TABLE typed_catalog (
    id TEXT PRIMARY KEY, codec TEXT NOT NULL CHECK(length(codec)=64),
    sha256 TEXT NOT NULL CHECK(length(sha256)=64), payload BLOB NOT NULL
) STRICT;

CREATE TABLE product_image_selector (
    position INTEGER PRIMARY KEY CHECK(position>=0), product_id INTEGER NOT NULL CHECK(product_id>0),
    edition_id INTEGER NOT NULL CHECK(edition_id>=0), layout_id INTEGER NOT NULL CHECK(layout_id>=0),
    part TEXT NOT NULL CHECK(part IN ('Product','MouseBottom','KeyboardWrist','KeyboardDial','Streamer')),
    asset TEXT NOT NULL
) STRICT;
CREATE TABLE dashboard_image_selector (
    position INTEGER PRIMARY KEY CHECK(position>=0), product_id INTEGER NOT NULL CHECK(product_id>0),
    edition_id INTEGER NOT NULL CHECK(edition_id>=0), layout_id INTEGER NOT NULL CHECK(layout_id>=0), asset TEXT NOT NULL
) STRICT;
CREATE TABLE dashboard_plugin_image_selector (
    position INTEGER PRIMARY KEY CHECK(position>=0), product_id INTEGER NOT NULL CHECK(product_id>0),
    edition_id INTEGER NOT NULL CHECK(edition_id>=0), filename_edition_id INTEGER NOT NULL CHECK(filename_edition_id>=0),
    layout_id INTEGER NOT NULL CHECK(layout_id>=0), wrist INTEGER NOT NULL CHECK(wrist IN (0,1)),
    density INTEGER NOT NULL CHECK(density IN (1,3)), asset TEXT NOT NULL
) STRICT;
CREATE TABLE pairing_image_selector (
    position INTEGER PRIMARY KEY CHECK(position>=0), product_id INTEGER NOT NULL CHECK(product_id>0),
    edition_id INTEGER NOT NULL CHECK(edition_id>=0), layout_id INTEGER NOT NULL CHECK(layout_id>=0), asset TEXT NOT NULL
) STRICT;
CREATE TABLE gamepad_calibration_svg_selector (
    position INTEGER PRIMARY KEY CHECK(position>=0), product_id INTEGER NOT NULL CHECK(product_id>0),
    edition_id INTEGER NOT NULL CHECK(edition_id>=0),
    kind TEXT NOT NULL CHECK(kind IN ('prd','ActuationDevice')), asset TEXT NOT NULL
) STRICT;
CREATE TABLE resource_storage (
    id INTEGER PRIMARY KEY CHECK(id=1), schema_version INTEGER NOT NULL CHECK(schema_version=1),
    provider TEXT NOT NULL, repository TEXT NOT NULL, revision TEXT
) STRICT;
CREATE TABLE ui_product (
    id INTEGER PRIMARY KEY CHECK(id>0 AND id<=4294967295), name TEXT NOT NULL CHECK(length(name)>0)
) STRICT;
CREATE TABLE ui_product_category (
    product_id INTEGER NOT NULL REFERENCES ui_product(id), position INTEGER NOT NULL CHECK(position>=0),
    value TEXT NOT NULL CHECK(length(value)>0), PRIMARY KEY(product_id, position), UNIQUE(product_id, value)
) STRICT;
CREATE TABLE ui_product_edition (
    product_id INTEGER NOT NULL REFERENCES ui_product(id), position INTEGER NOT NULL CHECK(position>=0),
    edition_id INTEGER NOT NULL CHECK(edition_id>=0 AND edition_id<=4294967295),
    PRIMARY KEY(product_id, position), UNIQUE(product_id, edition_id)
) STRICT;
CREATE TABLE ui_navigation (
    key TEXT PRIMARY KEY CHECK(length(key)>0), product_id INTEGER NOT NULL REFERENCES ui_product(id),
    position INTEGER NOT NULL CHECK(position>=0), display_mode TEXT NOT NULL CHECK(length(display_mode)>0),
    is_primary INTEGER NOT NULL CHECK(is_primary IN (0,1)), UNIQUE(product_id, position)
) STRICT;
CREATE UNIQUE INDEX ui_navigation_primary ON ui_navigation(product_id) WHERE is_primary=1;
CREATE TABLE ui_page (
    key TEXT PRIMARY KEY CHECK(length(key)>0), navigation_key TEXT NOT NULL REFERENCES ui_navigation(key),
    position INTEGER NOT NULL CHECK(position>=0), kind TEXT NOT NULL CHECK(length(kind)>0),
    role TEXT NOT NULL CHECK(role IN ('Page','Help','StandaloneMode')),
    help_selector INTEGER NOT NULL CHECK(help_selector>=0), UNIQUE(navigation_key, position)
) STRICT;
CREATE TABLE resource_file (
    id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE,
    bytes INTEGER NOT NULL CHECK(bytes>=0), sha256 TEXT NOT NULL CHECK(length(sha256)=64)
) STRICT;
CREATE TABLE resource_file_order (
    position INTEGER PRIMARY KEY CHECK(position>=0), file_id INTEGER NOT NULL REFERENCES resource_file(id)
) STRICT;
CREATE TABLE resource_alias (
    position INTEGER PRIMARY KEY CHECK(position>=0), key TEXT NOT NULL,
    file_id INTEGER NOT NULL REFERENCES resource_file(id), view_box TEXT,
    width INTEGER CHECK(width>0), height INTEGER CHECK(height>0)
) STRICT;
CREATE INDEX resource_alias_key ON resource_alias(key, position);
CREATE TABLE resource_list (
    position INTEGER PRIMARY KEY CHECK(position>=0), key TEXT NOT NULL, provider TEXT NOT NULL
) STRICT;
CREATE TABLE resource_font (
    position INTEGER PRIMARY KEY CHECK(position>=0), file_id INTEGER NOT NULL REFERENCES resource_file(id)
) STRICT;
CREATE TABLE emoji_entry (
    id INTEGER PRIMARY KEY, emoji TEXT NOT NULL, name TEXT NOT NULL, UNIQUE(emoji, name)
) STRICT;
CREATE TABLE emoji_search (
    position INTEGER PRIMARY KEY CHECK(position>=0), entry_id INTEGER NOT NULL REFERENCES emoji_entry(id)
) STRICT;
CREATE TABLE emoji_group_list (id INTEGER PRIMARY KEY) STRICT;
CREATE TABLE emoji_group_value (
    list_id INTEGER NOT NULL REFERENCES emoji_group_list(id), position INTEGER NOT NULL CHECK(position>=0),
    value TEXT NOT NULL, PRIMARY KEY(list_id, position)
) STRICT;
CREATE TABLE emoji_variant_set (id INTEGER PRIMARY KEY) STRICT;
CREATE TABLE emoji_variant (
    set_id INTEGER NOT NULL REFERENCES emoji_variant_set(id), base TEXT NOT NULL,
    position INTEGER NOT NULL CHECK(position>=0), value TEXT NOT NULL, PRIMARY KEY(set_id, base, position)
) STRICT;
CREATE TABLE emoji_exclusion_set (id INTEGER PRIMARY KEY) STRICT;
CREATE TABLE emoji_exclusion (
    set_id INTEGER NOT NULL REFERENCES emoji_exclusion_set(id), position INTEGER NOT NULL CHECK(position>=0),
    value TEXT NOT NULL, PRIMARY KEY(set_id, position)
) STRICT;
CREATE TABLE emoji_dataset (
    id TEXT PRIMARY KEY CHECK(id IN ('mapping', 'mapping_without_exclusions', 'macro')),
    variant_set INTEGER NOT NULL REFERENCES emoji_variant_set(id),
    exclusion_set INTEGER NOT NULL REFERENCES emoji_exclusion_set(id),
    exclusion_rule TEXT NOT NULL CHECK(exclusion_rule IN ('windows_11', 'without_windows_10'))
) STRICT;
CREATE TABLE emoji_group (
    dataset_id TEXT NOT NULL REFERENCES emoji_dataset(id), position INTEGER NOT NULL CHECK(position>=0),
    values_id INTEGER NOT NULL REFERENCES emoji_group_list(id), label TEXT, name TEXT,
    PRIMARY KEY(dataset_id, position)
) STRICT;
CREATE TABLE emoji_tab (
    dataset_id TEXT NOT NULL REFERENCES emoji_dataset(id), position INTEGER NOT NULL CHECK(position>=0),
    emoji TEXT NOT NULL, name TEXT NOT NULL, target_position INTEGER CHECK(target_position>=0),
    PRIMARY KEY(dataset_id, position)
) STRICT;

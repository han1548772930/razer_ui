PRAGMA foreign_keys = ON;
CREATE TABLE resource_storage (
    id INTEGER PRIMARY KEY CHECK(id=1), schema_version INTEGER NOT NULL CHECK(schema_version=1),
    provider TEXT NOT NULL, repository TEXT NOT NULL, revision TEXT
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

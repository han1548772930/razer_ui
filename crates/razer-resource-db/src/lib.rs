//! Packaged, immutable SQLite resources. Media and user configuration have separate owners.
use anyhow::{Context as _, ensure};
use rusqlite::{Connection, OpenFlags};
use std::sync::{Mutex, OnceLock};

mod document;
mod installation;
mod publication;
pub use document::{CatalogId, NodeReader, initialize, read_document};
pub mod catalogs;
pub use installation::InstallationPaths;
pub use publication::StagedResource;

mod packaged {
    include!(concat!(env!("OUT_DIR"), "/installation.rs"));
}
pub use packaged::CACHE_RELATIVE;

const APPLICATION_ID: u32 = 0x525A5549;
const SCHEMA_VERSION: u32 = 3;

fn open(path: &std::path::Path) -> anyhow::Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .context("Cannot open the installed resource database")?;
    validate(&connection)?;
    Ok(connection)
}

fn validate(connection: &Connection) -> anyhow::Result<()> {
    connection.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;")?;
    let application: u32 =
        connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    ensure!(
        application == APPLICATION_ID,
        "Invalid resource database identity"
    );
    ensure!(
        version == SCHEMA_VERSION,
        "Unsupported resource database version {version}"
    );
    let integrity: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    ensure!(
        integrity == "ok",
        "Resource database integrity check failed: {integrity}"
    );
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    ensure!(
        statement.query([])?.next()?.is_none(),
        "Resource database references are invalid"
    );
    Ok(())
}

/// Deserialization boundary for the typed catalog and resource-index owners.
/// Both owners retain validated snapshots; rendering never executes SQL or file I/O.
pub fn read_catalog<T>(read: impl FnOnce(&Connection) -> anyhow::Result<T>) -> anyhow::Result<T> {
    static DATABASE: OnceLock<Result<Mutex<Connection>, String>> = OnceLock::new();
    let database = DATABASE
        .get_or_init(|| {
            (|| -> anyhow::Result<_> {
                let paths = InstallationPaths::current()?;
                let path = paths.catalog()?;
                Ok(Mutex::new(open(&path)?))
            })()
            .map_err(|error| format!("{error:#}"))
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let connection = database
        .lock()
        .map_err(|_| anyhow::anyhow!("Resource database lock was poisoned"))?;
    read(&connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_catalog_is_readonly_and_has_no_media_payload_table() {
        read_catalog(|connection| {
            let count: u32 =
                connection.query_row("SELECT count(*) FROM emoji_search", [], |row| row.get(0))?;
            assert_eq!(count, 1936);
            assert!(connection.execute("DELETE FROM emoji_search", []).is_err());
            let blobs: u32 = connection.query_row(
                "SELECT count(*) FROM pragma_table_info('resource_file') WHERE type='BLOB'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(blobs, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn foreign_database_and_unknown_schema_are_rejected() {
        let connection = Connection::open_in_memory().unwrap();
        assert!(
            validate(&connection)
                .unwrap_err()
                .to_string()
                .contains("identity")
        );
        connection.pragma_update(None, "query_only", false).unwrap();
        connection
            .pragma_update(None, "application_id", APPLICATION_ID)
            .unwrap();
        connection
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        assert!(
            validate(&connection)
                .unwrap_err()
                .to_string()
                .contains("version")
        );
    }
}

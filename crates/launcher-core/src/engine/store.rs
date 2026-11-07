use rusqlite::{Connection, OptionalExtension, Row};
use serde::Serialize;

use super::version::compare_desc;
use crate::{Database, Result};

/// One row of `installed_engines`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledEngine {
    pub version: String,
    pub status: String,
    pub os: String,
    pub architecture: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub installed_at: String,
}

fn from_row(row: &Row<'_>) -> rusqlite::Result<InstalledEngine> {
    let size: i64 = row.get("size_bytes")?;
    Ok(InstalledEngine {
        version: row.get("version")?,
        status: row.get("status")?,
        os: row.get("os")?,
        architecture: row.get("architecture")?,
        sha256: row.get("sha256")?,
        size_bytes: u64::try_from(size).unwrap_or(0),
        installed_at: row.get("installed_at")?,
    })
}

const COLUMNS: &str = "version, status, os, architecture, sha256, size_bytes, installed_at";

fn select_one(conn: &Connection, version: &str) -> Result<Option<InstalledEngine>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM installed_engines WHERE version = ?1"),
            [version],
            from_row,
        )
        .optional()?)
}

pub fn get(db: &Database, version: &str) -> Result<Option<InstalledEngine>> {
    db.with_conn(|conn| select_one(conn, version))
}

/// Newest version first.
pub fn list(db: &Database) -> Result<Vec<InstalledEngine>> {
    let mut engines = db.with_conn(|conn| {
        let mut statement = conn.prepare(&format!("SELECT {COLUMNS} FROM installed_engines"))?;
        let rows = statement.query_map([], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })?;
    engines.sort_by(|a, b| compare_desc(&a.version, &b.version));
    Ok(engines)
}

/// Records a finished install (replacing an older row of the same version) and returns it as
/// stored, including the database's `installed_at`.
pub fn insert(db: &Database, engine: &InstalledEngine) -> Result<InstalledEngine> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO installed_engines
                 (version, status, os, architecture, sha256, size_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                engine.version,
                engine.status,
                engine.os,
                engine.architecture,
                engine.sha256,
                i64::try_from(engine.size_bytes).unwrap_or(i64::MAX),
            ],
        )?;
        Ok(select_one(conn, &engine.version)?.unwrap_or_else(|| engine.clone()))
    })
}

pub fn delete(db: &Database, version: &str) -> Result<()> {
    db.with_conn(|conn| {
        conn.execute(
            "DELETE FROM installed_engines WHERE version = ?1",
            [version],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(version: &str) -> InstalledEngine {
        InstalledEngine {
            version: version.to_owned(),
            status: "stable".to_owned(),
            os: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
            sha256: "a".repeat(64),
            size_bytes: 1234,
            installed_at: String::new(),
        }
    }

    #[test]
    fn insert_get_list_delete() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(list(&db).unwrap(), Vec::<InstalledEngine>::new());

        let stored = insert(&db, &engine("1.2.0")).unwrap();
        assert_eq!(stored.size_bytes, 1234);
        assert_ne!(stored.installed_at, "");
        insert(&db, &engine("1.10.0")).unwrap();
        insert(&db, &engine("0.9.0")).unwrap();

        let versions: Vec<_> = list(&db).unwrap().into_iter().map(|e| e.version).collect();
        assert_eq!(versions, ["1.10.0", "1.2.0", "0.9.0"]);
        assert!(get(&db, "1.2.0").unwrap().is_some());

        delete(&db, "1.2.0").unwrap();
        assert!(get(&db, "1.2.0").unwrap().is_none());
    }

    #[test]
    fn inserting_the_same_version_again_replaces_it() {
        let db = Database::open_in_memory().unwrap();
        insert(&db, &engine("1.0.0")).unwrap();
        let mut again = engine("1.0.0");
        again.size_bytes = 99;
        insert(&db, &again).unwrap();
        assert_eq!(list(&db).unwrap().len(), 1);
        assert_eq!(get(&db, "1.0.0").unwrap().unwrap().size_bytes, 99);
    }
}

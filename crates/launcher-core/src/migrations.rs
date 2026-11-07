use rusqlite::Connection;

use crate::{Error, Result};

/// Ordered list of schema migrations. Migration number `n` (1-based) is stored in SQLite's
/// `user_version` once applied. Never edit or reorder an existing entry, only append.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_engines.sql"),
];

/// Brings the database up to the latest schema.
pub fn run(conn: &mut Connection) -> Result<()> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let supported = u32::try_from(MIGRATIONS.len()).unwrap_or(u32::MAX);
    if current > supported {
        return Err(Error::DatabaseTooNew {
            found: current,
            supported,
        });
    }

    let applied = usize::try_from(current).unwrap_or(usize::MAX);
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(applied) {
        let version = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        tracing::info!(version, "applied database migration");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(conn: &Connection) -> u32 {
        conn.pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn applies_all_migrations_to_a_fresh_database() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn).unwrap();
        assert_eq!(
            user_version(&conn),
            u32::try_from(MIGRATIONS.len()).unwrap()
        );
        conn.execute("INSERT INTO settings (key, value) VALUES ('k', 'v')", [])
            .unwrap();
    }

    #[test]
    fn running_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn).unwrap();
        run(&mut conn).unwrap();
        assert_eq!(
            user_version(&conn),
            u32::try_from(MIGRATIONS.len()).unwrap()
        );
    }

    #[test]
    fn refuses_a_database_from_a_newer_launcher() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        let error = run(&mut conn).unwrap_err();
        assert!(matches!(error, Error::DatabaseTooNew { found: 99, .. }));
    }
}

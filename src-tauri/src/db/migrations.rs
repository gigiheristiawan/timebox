use rusqlite::Connection;

/// Forward-only, numbered migrations. Never edit a migration that has shipped —
/// add a new one. Each runs inside a transaction together with the bookkeeping
/// row, so a crash mid-migration leaves the schema at its previous version.
struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: include_str!("migrations/001_initial_schema.sql"),
    },
    Migration {
        version: 2,
        name: "away_and_first_run",
        sql: include_str!("migrations/002_away_and_first_run.sql"),
    },
    Migration {
        version: 3,
        name: "working_window",
        sql: include_str!("migrations/003_working_window.sql"),
    },
];

pub fn run(conn: &mut Connection) -> crate::error::AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at TEXT NOT NULL
        )",
    )?;

    let applied: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;

    for m in MIGRATIONS.iter().filter(|m| m.version > applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at)
             VALUES (?1, ?2, datetime('now'))",
            rusqlite::params![m.version, m.name],
        )?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_numbered_contiguously_from_one() {
        for (i, m) in MIGRATIONS.iter().enumerate() {
            assert_eq!(m.version, i as i64 + 1, "migration versions must be 1,2,3…");
        }
    }

    #[test]
    fn running_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn).unwrap();
        let first: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        run(&mut conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(first, MIGRATIONS.len() as i64);
        assert_eq!(count, MIGRATIONS.len() as i64, "migrations must not re-apply");
    }

    #[test]
    fn a_database_already_at_version_1_upgrades_without_losing_rows() {
        // The shipped database is at version 1. An upgrade that dropped or
        // rewrote rows would lose a real day's work, so this walks the actual
        // path a user's file takes rather than only the fresh-install one.
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL)")
            .unwrap();
        conn.execute_batch(MIGRATIONS[0].sql).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (1, 'initial_schema', '')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, title, status, block_duration_ms, created_at)
             VALUES ('t1', 'yesterday', 'IN_PROGRESS', 1800000, 0)",
            [],
        )
        .unwrap();

        run(&mut conn).unwrap();

        let version: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);

        let title: String = conn
            .query_row("SELECT title FROM tasks WHERE id = 't1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "yesterday", "existing rows must survive the upgrade");

        // The new columns must be usable, and default for pre-existing rows.
        let away: i64 = conn
            .query_row("SELECT COALESCE(SUM(away_ms), 0) FROM time_blocks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(away, 0);
        let first_run: i64 = conn
            .query_row("SELECT first_run_done FROM settings WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(first_run, 0, "an existing install has not seen the panel either");

        // Migration 003: the working window defaults to Mon-Fri 09:00-18:00 for
        // an existing install too, so idle is measurable from the first launch
        // after the upgrade rather than only once the user visits settings.
        let (start, end, days): (i64, i64, i64) = conn
            .query_row(
                "SELECT work_start_minutes, work_end_minutes, working_weekdays FROM settings WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((start, end, days), (540, 1080, 31));
    }

    #[test]
    fn schema_has_the_four_expected_tables() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn).unwrap();
        for t in ["tasks", "time_blocks", "app_state", "settings", "idle_spans"] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "missing table {t}");
        }
    }

    #[test]
    fn singleton_rows_are_seeded_and_cannot_multiply() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn).unwrap();
        for t in ["app_state", "settings"] {
            let n: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 1, "{t} should be seeded with exactly one row");
        }
        // id is CHECK-constrained to 1, so a second row is rejected.
        assert!(conn
            .execute("INSERT INTO app_state (id, timer_state) VALUES (2, 'IDLE')", [])
            .is_err());
    }
}

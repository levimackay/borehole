//! SQLite schema. This is real (not a stub) — the table shapes are part of
//! the frozen contract other crates are written against via `Index`'s
//! methods, so they're pinned here rather than left for whoever implements
//! the query methods to invent ad hoc.

use rusqlite::Connection;

pub const SCHEMA_VERSION: i64 = 1;

pub fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS files (
            id            INTEGER PRIMARY KEY,
            path          TEXT NOT NULL UNIQUE,
            language      TEXT,
            content_hash  TEXT NOT NULL,
            size_bytes    INTEGER NOT NULL,
            indexed_at    INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS symbols (
            id             INTEGER PRIMARY KEY,
            file_id        INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            name           TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            kind           TEXT NOT NULL,
            language       TEXT NOT NULL,
            start_line     INTEGER NOT NULL,
            start_col      INTEGER NOT NULL,
            end_line       INTEGER NOT NULL,
            end_col        INTEGER NOT NULL,
            start_byte     INTEGER NOT NULL,
            end_byte       INTEGER NOT NULL,
            is_exported    INTEGER NOT NULL,
            doc_comment    TEXT,
            signature      TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
        CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
        CREATE INDEX IF NOT EXISTS idx_symbols_qualified_name ON symbols(qualified_name);

        CREATE TABLE IF NOT EXISTS refs (
            id              INTEGER PRIMARY KEY,
            from_file_id    INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            from_symbol_id  INTEGER REFERENCES symbols(id) ON DELETE SET NULL,
            from_start_line INTEGER NOT NULL,
            from_start_col  INTEGER NOT NULL,
            from_end_line   INTEGER NOT NULL,
            from_end_col    INTEGER NOT NULL,
            from_start_byte INTEGER NOT NULL,
            from_end_byte   INTEGER NOT NULL,
            to_symbol_id    INTEGER REFERENCES symbols(id) ON DELETE SET NULL,
            to_name         TEXT NOT NULL,
            kind            TEXT NOT NULL,
            confidence      TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_refs_to_symbol ON refs(to_symbol_id);
        CREATE INDEX IF NOT EXISTS idx_refs_from_symbol ON refs(from_symbol_id);
        CREATE INDEX IF NOT EXISTS idx_refs_to_name ON refs(to_name);
        CREATE INDEX IF NOT EXISTS idx_refs_from_file ON refs(from_file_id);

        CREATE TABLE IF NOT EXISTS imports (
            id             INTEGER PRIMARY KEY,
            file_id        INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            local_name     TEXT NOT NULL,
            source_module  TEXT NOT NULL,
            imported_name  TEXT NOT NULL,
            start_line     INTEGER NOT NULL,
            start_col      INTEGER NOT NULL,
            end_line       INTEGER NOT NULL,
            end_col        INTEGER NOT NULL,
            start_byte     INTEGER NOT NULL,
            end_byte       INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_imports_file ON imports(file_id);
        CREATE INDEX IF NOT EXISTS idx_imports_local_name ON imports(local_name);
        "#,
    )?;

    conn.execute(
        "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [SCHEMA_VERSION.to_string()],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_creates_cleanly_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap(); // must not error on re-run

        let version: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION.to_string());
    }

    #[test]
    fn foreign_keys_cascade_on_file_delete() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO files(id, path, language, content_hash, size_bytes, indexed_at)
             VALUES (1, 'a.rs', 'rust', 'h', 10, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO symbols(id, file_id, name, qualified_name, kind, language,
                start_line, start_col, end_line, end_col, start_byte, end_byte, is_exported)
             VALUES (1, 1, 'foo', 'foo', 'function', 'rust', 1,1,1,1,0,3,1)",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM files WHERE id = 1", []).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "symbols should cascade-delete with their file");
    }
}

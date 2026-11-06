use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;

        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            totp_secret TEXT,
            totp_enabled INTEGER NOT NULL DEFAULT 0,
            totp_last_step INTEGER NOT NULL DEFAULT 0,
            is_admin INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions (
            token_hash TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            created_at INTEGER NOT NULL,
            last_seen INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            user_agent TEXT
        );
        CREATE TABLE IF NOT EXISTS app_tokens (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            token_hash TEXT UNIQUE NOT NULL,
            created_at INTEGER NOT NULL,
            last_used INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS recovery_codes (
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            code_hash TEXT NOT NULL,
            PRIMARY KEY (user_id, code_hash)
        );
        CREATE TABLE IF NOT EXISTS hash_cache (
            user_id INTEGER NOT NULL,
            path TEXT NOT NULL,
            size INTEGER NOT NULL,
            mtime INTEGER NOT NULL,
            hash TEXT NOT NULL,
            PRIMARY KEY (user_id, path)
        );
        CREATE TABLE IF NOT EXISTS trash (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            orig_path TEXT NOT NULL,
            stored_name TEXT NOT NULL,
            is_dir INTEGER NOT NULL,
            size INTEGER NOT NULL,
            deleted_at INTEGER NOT NULL
        );
        "#,
    )?;
    Ok(conn)
}

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub struct Ws { pub id: String, pub path: String, pub last_used: i64, pub created_at: i64, pub ttl_secs: i64 }

pub fn open(db: &Path) -> Result<Connection> {
    let c = Connection::open(db)?;
    c.execute_batch("PRAGMA journal_mode=WAL;")?;
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS workspaces (
           id TEXT PRIMARY KEY, name TEXT, path TEXT NOT NULL UNIQUE,
           created_at INTEGER NOT NULL, last_used INTEGER NOT NULL,
           ttl_secs INTEGER NOT NULL, base_hash TEXT, state TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS idx_ws_last_used ON workspaces(last_used);",
    )?;
    Ok(c)
}

pub fn insert(c: &Connection, id: &str, name: Option<&str>, path: &str, ttl: i64) -> Result<()> {
    let t = now();
    c.execute(
        "INSERT INTO workspaces (id,name,path,created_at,last_used,ttl_secs,state) VALUES (?1,?2,?3,?4,?4,?5,'active')",
        params![id, name, path, t, ttl],
    )?;
    Ok(())
}

pub fn list(c: &Connection, state: &str) -> Result<Vec<Ws>> {
    let mut st = c.prepare("SELECT id,path,last_used,created_at,ttl_secs FROM workspaces WHERE state=?1 ORDER BY last_used ASC")?;
    let rows = st.query_map([state], |r| Ok(Ws { id: r.get(0)?, path: r.get(1)?, last_used: r.get(2)?, created_at: r.get(3)?, ttl_secs: r.get(4)? }))?;
    let out: Vec<Ws> = rows.filter_map(|r| r.ok()).collect();
    Ok(out)
}

pub fn set_state(c: &Connection, id: &str, state: &str) -> Result<()> {
    c.execute("UPDATE workspaces SET state=?2 WHERE id=?1", params![id, state])?;
    Ok(())
}
pub fn delete(c: &Connection, id: &str) -> Result<()> {
    c.execute("DELETE FROM workspaces WHERE id=?1", [id])?;
    Ok(())
}
pub fn touch(c: &Connection, id: &str) -> Result<()> {
    c.execute("UPDATE workspaces SET last_used=?2 WHERE id=?1", params![id, now()])?;
    Ok(())
}

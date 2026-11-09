use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub struct Ws {
    pub id: String,
    pub name: Option<String>,
    pub path: String,
    pub last_used: i64,
    pub created_at: i64,
    pub ttl_secs: i64,
    pub base_hash: Option<String>,
}

impl Ws {
    /// Unix time at which this workspace expires by TTL.
    pub fn expires_at(&self) -> i64 {
        self.created_at + self.ttl_secs
    }
}

pub struct BaseInfo {
    pub built_at: i64,
    pub head: Option<String>,
    pub lockfile_hash: Option<String>,
    /// Repo URL or path the base was built from. `None` for bases built before v0.1.
    pub repo: Option<String>,
    /// Setup command the base was built with. `None` for bases built before v0.1.
    pub setup_cmd: Option<String>,
}

pub fn open(db: &Path) -> Result<Connection> {
    let c = Connection::open(db)?;
    c.execute_batch("PRAGMA journal_mode=WAL;")?;
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS workspaces (
           id TEXT PRIMARY KEY, name TEXT, path TEXT NOT NULL UNIQUE,
           created_at INTEGER NOT NULL, last_used INTEGER NOT NULL,
           ttl_secs INTEGER NOT NULL, base_hash TEXT, state TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS idx_ws_last_used ON workspaces(last_used);
         CREATE TABLE IF NOT EXISTS base (
           id INTEGER PRIMARY KEY CHECK (id = 1),
           built_at INTEGER NOT NULL,
           head TEXT,
           lockfile_hash TEXT);",
    )?;
    migrate_base(&c)?;
    Ok(c)
}

/// Add the `repo` and `setup_cmd` columns to databases created before v0.1.
fn migrate_base(c: &Connection) -> Result<()> {
    let mut cols = Vec::new();
    let mut st = c.prepare("PRAGMA table_info(base)")?;
    let rows = st.query_map([], |r| r.get::<_, String>(1))?;
    for name in rows {
        cols.push(name?);
    }
    for col in ["repo", "setup_cmd"] {
        if !cols.iter().any(|c| c == col) {
            c.execute_batch(&format!("ALTER TABLE base ADD COLUMN {col} TEXT;"))?;
        }
    }
    Ok(())
}

pub fn insert(
    c: &Connection,
    id: &str,
    name: Option<&str>,
    path: &str,
    ttl: i64,
    base_hash: Option<String>,
) -> Result<()> {
    let t = now();
    c.execute(
        "INSERT INTO workspaces (id,name,path,created_at,last_used,ttl_secs,base_hash,state) VALUES (?1,?2,?3,?4,?4,?5,?6,'active')",
        params![id, name, path, t, ttl, base_hash],
    )?;
    Ok(())
}

const WS_COLUMNS: &str = "id,name,path,last_used,created_at,ttl_secs,base_hash";

fn row_to_ws(r: &rusqlite::Row<'_>) -> rusqlite::Result<Ws> {
    Ok(Ws {
        id: r.get(0)?,
        name: r.get(1)?,
        path: r.get(2)?,
        last_used: r.get(3)?,
        created_at: r.get(4)?,
        ttl_secs: r.get(5)?,
        base_hash: r.get(6)?,
    })
}

/// Workspaces in a given state, least recently used first.
pub fn list(c: &Connection, state: &str) -> Result<Vec<Ws>> {
    let mut st = c.prepare(&format!(
        "SELECT {WS_COLUMNS} FROM workspaces WHERE state=?1 ORDER BY last_used ASC"
    ))?;
    let rows = st.query_map([state], row_to_ws)?;
    let out: Vec<Ws> = rows.filter_map(|r| r.ok()).collect();
    Ok(out)
}

/// One active or deleting workspace by id.
pub fn find(c: &Connection, id: &str) -> Result<Option<Ws>> {
    let mut st = c.prepare(&format!("SELECT {WS_COLUMNS} FROM workspaces WHERE id=?1"))?;
    Ok(st.query_row([id], row_to_ws).optional()?)
}

pub fn set_state(c: &Connection, id: &str, state: &str) -> Result<()> {
    c.execute(
        "UPDATE workspaces SET state=?2 WHERE id=?1",
        params![id, state],
    )?;
    Ok(())
}
pub fn delete(c: &Connection, id: &str) -> Result<()> {
    c.execute("DELETE FROM workspaces WHERE id=?1", [id])?;
    Ok(())
}
pub fn touch(c: &Connection, id: &str) -> Result<()> {
    c.execute(
        "UPDATE workspaces SET last_used=?2 WHERE id=?1",
        params![id, now()],
    )?;
    Ok(())
}

pub fn set_base_info(
    c: &Connection,
    built_at: i64,
    head: Option<String>,
    lockfile_hash: Option<String>,
    repo: Option<String>,
    setup_cmd: Option<String>,
) -> Result<()> {
    c.execute(
        "INSERT OR REPLACE INTO base (id, built_at, head, lockfile_hash, repo, setup_cmd)
         VALUES (1, ?1, ?2, ?3, ?4, ?5)",
        params![built_at, head, lockfile_hash, repo, setup_cmd],
    )?;
    Ok(())
}

pub fn get_base_info(c: &Connection) -> Result<Option<BaseInfo>> {
    let mut st =
        c.prepare("SELECT built_at, head, lockfile_hash, repo, setup_cmd FROM base WHERE id = 1")?;
    let mut rows = st.query_map([], |r| {
        Ok(BaseInfo {
            built_at: r.get(0)?,
            head: r.get(1)?,
            lockfile_hash: r.get(2)?,
            repo: r.get(3)?,
            setup_cmd: r.get(4)?,
        })
    })?;
    Ok(rows.next().transpose()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn base_info_roundtrip() {
        let d = tempdir().unwrap();
        let db = open(&d.path().join("test.db")).unwrap();

        // Initially no base info
        assert!(get_base_info(&db).unwrap().is_none());

        set_base_info(
            &db,
            12345,
            Some("abc123".to_string()),
            Some("lockhash".to_string()),
            Some("/repo".to_string()),
            Some("npm ci".to_string()),
        )
        .unwrap();

        let info = get_base_info(&db).unwrap().unwrap();
        assert_eq!(info.built_at, 12345);
        assert_eq!(info.head, Some("abc123".to_string()));
        assert_eq!(info.lockfile_hash, Some("lockhash".to_string()));
        assert_eq!(info.repo, Some("/repo".to_string()));
        assert_eq!(info.setup_cmd, Some("npm ci".to_string()));

        // Update base info
        set_base_info(
            &db,
            67890,
            Some("def456".to_string()),
            Some("newhash".to_string()),
            None,
            None,
        )
        .unwrap();
        let info = get_base_info(&db).unwrap().unwrap();
        assert_eq!(info.built_at, 67890);
        assert_eq!(info.head, Some("def456".to_string()));
        assert_eq!(info.lockfile_hash, Some("newhash".to_string()));
        assert_eq!(info.repo, None);
    }

    #[test]
    fn find_and_expiry() {
        let d = tempdir().unwrap();
        let db = open(&d.path().join("test.db")).unwrap();
        insert(&db, "abc", Some("feat"), "/ws/abc", 3600, None).unwrap();
        let ws = find(&db, "abc").unwrap().unwrap();
        assert_eq!(ws.name.as_deref(), Some("feat"));
        assert_eq!(ws.expires_at(), ws.created_at + 3600);
        assert!(find(&db, "missing").unwrap().is_none());
    }

    #[test]
    fn migrates_old_base_table() {
        let d = tempdir().unwrap();
        let path = d.path().join("old.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE base (id INTEGER PRIMARY KEY CHECK (id = 1),
                   built_at INTEGER NOT NULL, head TEXT, lockfile_hash TEXT);
                 INSERT INTO base VALUES (1, 5, 'h', 'l');",
            )
            .unwrap();
        }
        let db = open(&path).unwrap();
        let info = get_base_info(&db).unwrap().unwrap();
        assert_eq!(info.built_at, 5);
        assert_eq!(info.repo, None);
    }
}

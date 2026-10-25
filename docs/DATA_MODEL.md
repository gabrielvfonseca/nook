# Data Model

SQLite, WAL mode, file `$NOOK_ROOT/nook.db`.

```sql
CREATE TABLE workspaces (
  id         TEXT PRIMARY KEY,      -- short random id
  name       TEXT,
  path       TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,      -- unix secs
  last_used  INTEGER NOT NULL,
  ttl_secs   INTEGER NOT NULL,
  base_hash  TEXT,                  -- lockfile/HEAD hash at clone time
  state      TEXT NOT NULL          -- active | deleting
);
CREATE INDEX idx_ws_last_used ON workspaces(last_used);

CREATE TABLE base (
  id            INTEGER PRIMARY KEY CHECK (id = 1),
  built_at      INTEGER NOT NULL,
  head          TEXT,
  lockfile_hash TEXT
);
```

## State machine
`(none) -> active -> deleting -> (row removed)`

On startup, any `deleting` row is retried (crash recovery). A directory under `ws/`
with no row is an orphan; `gc` reports and removes it after a grace period.

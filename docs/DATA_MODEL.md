# Data model

SQLite in WAL mode, at `$REPLICA_ROOT/replica.db`. Schema changes go in
`store::open`, and they must be idempotent so that an older database upgrades on
first use.

```sql
CREATE TABLE workspaces (
  id         TEXT PRIMARY KEY,      -- 8 hex chars, from util::new_id()
  name       TEXT,                  -- optional label from --name
  path       TEXT NOT NULL UNIQUE,  -- $REPLICA_ROOT/ws/<id>
  created_at INTEGER NOT NULL,      -- unix seconds
  last_used  INTEGER NOT NULL,      -- unix seconds; set on new and by `touch`
  ttl_secs   INTEGER NOT NULL,      -- lifetime from --ttl
  base_hash  TEXT,                  -- lockfile hash of the base when cloned
  state      TEXT NOT NULL          -- 'active' or 'deleting'
);
CREATE INDEX idx_ws_last_used ON workspaces(last_used);

CREATE TABLE base (
  id            INTEGER PRIMARY KEY CHECK (id = 1),  -- at most one base row
  built_at      INTEGER NOT NULL,   -- unix seconds
  head          TEXT,               -- git commit hash at build time
  lockfile_hash TEXT,               -- SHA-256 over the lockfiles present
  repo          TEXT,               -- repo URL or path, used by `base refresh`
  setup_cmd     TEXT                -- setup command, used by `base refresh`
);
```

Expiry is `created_at + ttl_secs`. Expired means `expires_at < now`.

## State machine

```
(none) --new--> active --gc / rm--> deleting --(files gone)--> (row removed)
```

A workspace moves to `deleting` before its files are removed. If the process
dies in between, the next `replica gc` retries the removal, and it deletes the
row only after the files are gone. A failed removal leaves the row in
`deleting`, and `rm` reports it.

Not yet implemented: detecting orphan directories under `ws/` that have no row.
See [ROADMAP](ROADMAP.md).

## Migrations

| Version | Change | How it is applied |
|---|---|---|
| v1 | Initial `workspaces` table | `CREATE TABLE IF NOT EXISTS` |
| v2 | `workspaces.base_hash`, `base` table | `CREATE TABLE IF NOT EXISTS` for `base`. `base_hash` was added with `ALTER TABLE` |
| v3 | `base.repo`, `base.setup_cmd` | `migrate_base` checks `PRAGMA table_info(base)` and runs `ALTER TABLE base ADD COLUMN` for each missing column |

Rows from before v3 have `repo` and `setup_cmd` set to `NULL`. For those bases,
`replica base refresh` tells you to rebuild once with `base build`.

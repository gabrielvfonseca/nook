# Nook Spec

## Problem
Agent runs need an isolated project tree. Today: `git clone`/checkout + dependency
install, or `cp -r`. On macOS this creates tens of thousands of small files; file
creation/deletion is the slowest fs operation. Setup costs seconds to minutes per run,
cleanup is slow, and leftover build output and temp files fill the disk.

## Goals
- G1: Create a ready workspace by cloning a prebuilt base (clone cost scales with
  file/dir count, not bytes).
- G2: Isolation: changes in one workspace never affect the base or other workspaces.
- G3: Bounded disk use: a runaway workspace cannot fill the system disk.
- G4: Automatic cleanup via TTL and LRU eviction, with fast parallel deletion.
- G5: Honest accounting of disk use (clones make `du` lie).

## Non-goals
- A new filesystem (no FUSE/FSKit).
- Speeding up in-workspace reads/writes.
- Linux/Windows support (reflink support may come later; out of scope).
- Following base updates in existing workspaces (they are snapshots).

## Requirements
| ID | Requirement |
|---|---|
| R1 | `nook new` clones base to `$NOOK_ROOT/ws/<id>` using `clonefile(2)`; errors if not same APFS volume. |
| R2 | Sets per-workspace `TMPDIR`, `NOOK_WS`, cache env vars via `nook env <id>`. |
| R3 | Metadata persisted in SQLite; crash-safe (WAL). |
| R4 | `gc` deletes expired workspaces, then evicts LRU until volume usage < low-water mark. |
| R5 | Deletion is parallel and symlink-safe. |
| R6 | Base rebuild is atomic (build in `base.new`, rename swap). |
| R7 | `stats` reports volume free space from `statvfs`, not `du`. |
| R8 | Concurrent `new`/`gc` safe via SQLite transactions + per-workspace state. |

## Success criteria
Measured on the user's machine (see BENCHMARKS.md): clone is >= 5-10x faster than the
current setup path. If not, the bottleneck is elsewhere and Nook should not be adopted.

## Risks
- Clone time still scales with file count.
- Space reclaimed on delete only when no other clone shares the blocks.
- Security tools may still scan cloned files on first access.
- Tools that hardlink or use absolute paths inside the tree may misbehave when cloned.

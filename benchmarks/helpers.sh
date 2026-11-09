#!/usr/bin/env bash
# Untimed setup/cleanup steps for hyperfine. Called from run.sh; not meant to be run by hand.
# Each subcommand is a single, quiet action so it can sit in --prepare without skewing timings.
set -euo pipefail

case "${1:?subcommand required}" in
  new-path)   # create a workspace and record its path
    replica new --ttl 1h >"$2" 2>/dev/null ;;
  new-id)     # create a workspace and record its id
    p=$(replica new --ttl 1h 2>/dev/null)
    basename "$p" >"$2" ;;
  rm-recorded) # remove the workspace recorded in a file (if any), then forget it
    if [ -s "$2" ]; then
      if [ "${3:-}" = "path" ]; then
        replica rm "$(basename "$(cat "$2")")" >/dev/null 2>&1 || true
      else
        replica rm "$(cat "$2")" >/dev/null 2>&1 || true
      fi
    fi
    rm -f "$2" ;;
  git-clean)  # drop a detached worktree and its directory
    git -C "$2" worktree remove --force "$3" >/dev/null 2>&1 || true
    git -C "$2" worktree prune >/dev/null 2>&1 || true
    rm -rf "$3" ;;
  rm-tree)    # remove a directory tree created by the benchmark
    rm -rf "$2" ;;
  *)
    echo "helpers.sh: unknown subcommand $1" >&2
    exit 2 ;;
esac

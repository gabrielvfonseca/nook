---
name: replica-release
description: Cut a Replica release. Covers the version bump, CHANGELOG, building bin/ for both macOS architectures, tagging vX.Y.Z, and publishing the GitHub release with binaries and sha256sums.txt via scripts/release.sh. Use when preparing a version or running the release script. Homebrew is out of scope for now (see HOMEBREW.md).
---

# Replica release

A release is a git tag plus a GitHub release with three binaries and
`sha256sums.txt`. The installer script (`scripts/get-replica.sh`) downloads from
the latest release and checks the checksum. Homebrew is not part of the release
yet; see `HOMEBREW.md`.

## Steps

1. **Start clean.** `git status` must be empty, and `make check` must pass.
2. **Bump the version** in `Cargo.toml`. Add a `## X.Y.Z - YYYY-MM-DD` section to
   `CHANGELOG.md`. The release script requires that heading.
3. **Merge** the release commit to `main` through a PR, as usual.
4. **Dry run** from the merged `main`. It writes nothing except `bin/`:
   ```sh
   scripts/release.sh X.Y.Z
   ```
   It checks the clean tree, that Cargo.toml and CHANGELOG match, and that the tag
   does not exist. It runs `make check` and `make dist`, and prints the sha256 values.
5. **Publish**:
   ```sh
   scripts/release.sh X.Y.Z --apply
   ```
   This creates annotated tag `vX.Y.Z`, pushes it, and creates the GitHub release
   with the CHANGELOG notes and the `bin/` artifacts plus `sha256sums.txt`.
6. **Verify** from a clean shell:
   ```sh
   PREFIX=$(mktemp -d) sh scripts/get-replica.sh
   "$PREFIX/bin/replica" --version && "$PREFIX/bin/replica" help
   ```

## Details

- `bin/replica-darwin-arm64`, `bin/replica-darwin-x86_64`, and
  `bin/replica-darwin-universal` come from `make dist`. The release uploads all
  three, so the installer can pick either architecture.
- `make dist` needs rustup's cargo on `PATH` to build the x86_64 target. Use
  `PATH=$HOME/.cargo/bin:$PATH` if Homebrew's cargo is first.

## Troubleshooting

- **Tag already exists**: pick the next version. Do not delete a published tag.
- **Dirty tree**: commit or stash. The script will not publish from uncommitted state.
- **Checksum mismatch in the installer**: the release assets changed after `sha256sums.txt` was made. Cut a new version.

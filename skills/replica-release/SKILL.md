---
name: replica-release
description: Cut a Replica release. Covers the version bump, CHANGELOG, building bin/ for both macOS architectures, tagging vX.Y.Z, publishing the GitHub release with binaries and sha256sums.txt, and updating the Homebrew formula in homebrew/ (which is the tap, inside this repository). Use when preparing a version, running scripts/release.sh, editing the formula, or when a user asks how to install Replica with Homebrew.
---

# Replica release and Homebrew

The formula is `homebrew/Formula/replica.rb`. It is not installable with
`brew` yet: Homebrew needs a tap with `Formula/` at a repository root, and this
repository has none by design. See `homebrew/README.md` for the options.

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
   This creates annotated tag `vX.Y.Z`, pushes it, creates the GitHub release with the
   CHANGELOG notes and the `bin/` artifacts plus `sha256sums.txt`, and writes the
   checksums into `homebrew/Formula/replica.rb`. Those checksums come from the same
   local artifacts that were uploaded.
6. **Commit the formula** to `main` (PR), so `brew` users get the new checksums.
7. **Verify** from a clean shell:
   ```sh
   # brew: not available until a tap layout is chosen (see homebrew/README.md)
   replica --version && replica help
   ```

## Formula details

- Stable installs the prebuilt binary for the machine's architecture. `--HEAD` builds from `main` with `cargo install`.
- `service` runs `replica gc` every 600 seconds. It uses `REPLICA_ROOT=$HOME/replica`, because launchd does not read the shell profile.
- `test do` checks `replica --version` and that `replica help` prints `USAGE`.
- `license "Apache-2.0"` matches `Cargo.toml`.

## Troubleshooting

- **Tag already exists**: pick the next version, or delete the local and remote tag only after confirming nobody installed it.
- **Dirty tree**: commit or stash. The script will not publish from uncommitted state.
- **`replica setup --homebrew`** writes a local formula to `~/homebrew/replica.rb` with placeholder checksums. Prefer the tap for installs.

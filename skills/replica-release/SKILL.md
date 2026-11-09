---
name: replica-release
description: Cut a Replica release and publish it through the Homebrew tap (gabrielvfonseca/homebrew-tap). Use when preparing a version bump, tagging vX.Y.Z, updating CHANGELOG.md, rendering the Homebrew formula with scripts/release.sh, or when a user asks how to install Replica with brew. Covers the two-phase flow (dry run before the tag, --apply after) and the steps that only a maintainer should run.
---

# Replica release and Homebrew tap

Users install Replica with:

```sh
brew tap gabrielvfonseca/tap
brew install replica
```

`brew tap gabrielvfonseca/tap` resolves to the repo `gabrielvfonseca/homebrew-tap`.
The formula builds from the tagged source tarball with `cargo install`, so
users need no Rust toolchain of their own (brew installs `rust` as a build
dependency).

The tap formula is `Formula/replica.rb` in the tap repo. `scripts/release.sh`
writes its `url` and `sha256`. The formula source of truth is
`templates/replica.rb.tmpl`, used by `replica setup --homebrew` for local
installs. Keep the two consistent.

## Who runs what

Anything that publishes (tags, pushes, GitHub releases, pushing the tap) is
outward-facing. Confirm with the maintainer before running it. The script never
publishes on its own, and it never creates tags or releases.

## Steps

1. **Check the tree.** `git status` must be clean, and `make check` must pass.
2. **Bump the version** in `Cargo.toml`, then update `CHANGELOG.md`. Add a dated
   section for the new version. List user-visible changes only, and cite
   benchmark numbers only from `benchmarks/results/`.
3. **Commit** the release changes on `main`.
4. **Dry run** the release plan (writes nothing):
   ```sh
   scripts/release.sh 0.1.0
   ```
   It refuses if the tree is dirty or the tag exists. It prints a preview
   sha256 from `git archive`. That value is not the one to publish, because
   GitHub's tag tarball can differ.
5. **Tag and publish** (maintainer runs these):
   ```sh
   git tag -a v0.1.0 -m "replica 0.1.0"
   git push origin v0.1.0
   gh release create v0.1.0 --title "replica 0.1.0" --notes-file CHANGELOG.md
   ```
6. **Render the formula** from the published tag tarball:
   ```sh
   scripts/release.sh 0.1.0 --apply
   ```
   This downloads `https://github.com/gabrielvfonseca/replica/archive/refs/tags/v0.1.0.tar.gz`,
   hashes it, and rewrites `url` and `sha256` in the tap's formula.
   Set `TAP_DIR` if the tap is not at `../homebrew-tap`.
7. **Check the formula** in the tap:
   ```sh
   cd ../homebrew-tap
   ruby -c Formula/replica.rb
   brew style Formula/replica.rb
   brew audit --strict --online Formula/replica.rb    # needs the tap to be installed or named
   ```
8. **Push the tap** (first time, create the repo `gabrielvfonseca/homebrew-tap` on GitHub):
   ```sh
   git add Formula/replica.rb && git commit -m "replica 0.1.0"
   git remote add origin git@github.com:gabrielvfonseca/homebrew-tap.git   # first time only
   git push -u origin main
   ```
9. **Verify from a clean machine or shell:**
   ```sh
   brew tap gabrielvfonseca/tap
   brew install replica
   replica --version
   replica help
   ```

## The formula

- `url` and `sha256` change with each release. `head` tracks `main` for `brew install --HEAD replica`.
- `service` runs `replica gc` every 600 seconds. `brew services start replica` enables it. It uses `REPLICA_ROOT=$HOME/replica`, because launchd does not read the shell profile. Tell users to change this if their workspaces live elsewhere.
- `test do` checks `replica --version` and that `replica help` prints `USAGE`. Keep that text in sync with `src/help.rs`.
- `license "Apache-2.0"` matches `Cargo.toml`.

## Troubleshooting

- **`tag vX.Y.Z is not on origin yet`**: push the tag first (step 5).
- **`working tree is not clean`**: commit or stash. The script will not render a formula from uncommitted state.
- **`brew style` flags `sha256`**: the placeholder `REPLACE_ON_RELEASE` is still there. Run step 6.
- **Users get the old version**: `brew update` before `brew upgrade replica`.
- **`replica setup --homebrew`** writes a local formula to `~/homebrew/replica.rb`. It uses placeholder `sha256` values (`REPLACE_WITH_ACTUAL_SHA256_*`) and points at prebuilt binaries from the release workflow, so it does not install until those values are filled in. Prefer the tap for installs.

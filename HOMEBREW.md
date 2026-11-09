# Homebrew

Replica is **not available through Homebrew yet**. Install it with the
installer script or from source (see [README](README.md#install)).

## Why `brew install` does not work

Homebrew installs formulae from *taps*, and a tap is a repository with a
`Formula/` directory at its root. This repository has no such directory, so
`brew install` has nothing to read. Checked on Homebrew 7.0.12:

- `brew install <formula URL>` fails with `Invalid tap name`
- `brew install ./path/to/replica.rb` (relative or absolute) fails with `Invalid tap name`
- A tap whose root has `Formula/` works. A local test with a `Formula` symlink
  pointing at a formula directory resolved the formula after `brew trust`.

## Where the formula is

The formula was removed from the working tree, so it is not in `main` now. It is
still in history: the last commit that contains it is **`16ad02e`**. Its
checksums match the published v0.1.0 release artifacts.

```sh
git show 16ad02e:homebrew/Formula/replica.rb > replica.rb    # inspect or restore it
```

The formula installs the prebuilt binary for the machine's architecture
(`replica-darwin-arm64` or `replica-darwin-x86_64`) from the GitHub release, and
has a `--HEAD` build from source with Rust.

## To enable Homebrew later

Pick one layout and tell the release process about it:

1. **A root `Formula/` in this repository.** Restore the formula there (for
   example, `git show 16ad02e:homebrew/Formula/replica.rb > Formula/replica.rb`).
   Users then run:
   ```sh
   brew tap gabrielvfonseca/replica https://github.com/gabrielvfonseca/replica
   brew trust gabrielvfonseca/replica
   brew install gabrielvfonseca/replica/replica
   ```
   `brew trust` is needed on Homebrew 7 for third-party taps.
2. **A separate `homebrew-tap` repository.** Users run
   `brew tap gabrielvfonseca/tap` and `brew install replica`. This needs a
   new public repository.

Either way, the formula's `version`, `url`, and `sha256` values must match the
release. Copy them from `bin/sha256sums.txt` after `scripts/release.sh X.Y.Z`
has produced the artifacts.

## Other ways to install

```sh
# Installer script: downloads the binary for your architecture and checks it against sha256sums.txt
curl -fsSL https://raw.githubusercontent.com/gabrielvfonseca/replica/main/scripts/get-replica.sh | sh

# From source (needs Rust: https://rustup.rs)
git clone https://github.com/gabrielvfonseca/replica.git && cd replica
make release && PREFIX=$HOME/.local make install
```

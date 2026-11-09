# Homebrew formula for Replica

This directory is the Homebrew tap for Replica. There is no separate tap
repository: the project root is the tap, and the root `Formula` symlink points
at `homebrew/Formula/`, which is where Homebrew looks for formulae.

Install (one time):

```sh
brew tap gabrielvfonseca/replica https://github.com/gabrielvfonseca/replica
brew trust gabrielvfonseca/replica          # Homebrew asks you to trust third-party taps
brew install gabrielvfonseca/replica/replica
```

- `brew install` fetches the prebuilt binary for your architecture from the
  GitHub release (`replica-darwin-arm64` or `replica-darwin-x86_64`).
- `brew install --HEAD gabrielvfonseca/replica/replica` builds from `main` with Rust.
- Upgrade: `brew update && brew upgrade replica`.
- Background GC every 10 minutes: `brew services start replica`.

Formula source: [`Formula/replica.rb`](Formula/replica.rb). The release script
([`scripts/release.sh`](../scripts/release.sh)) writes the checksums from the
release artifacts, so do not edit them by hand.

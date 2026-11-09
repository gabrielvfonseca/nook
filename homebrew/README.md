# Homebrew tap for Replica

This directory is the source of the `gabrielvfonseca/homebrew-tap` repository.
It is published with `git subtree split` (see the release skill), so the tap
repo always matches this directory.

```sh
brew tap gabrielvfonseca/tap
brew install replica
```

`brew install replica` installs the prebuilt, universal-compatible binary for
your architecture from the GitHub release (`replica-darwin-arm64` or
`replica-darwin-x86_64`). `brew install --HEAD replica` builds from `main` with
Rust instead.

Optional background GC every 10 minutes:

```sh
brew services start replica
```

Upgrade: `brew update && brew upgrade replica`.

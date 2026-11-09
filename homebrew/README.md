# Homebrew formula for Replica

The formula lives at [`Formula/replica.rb`](Formula/replica.rb) in this directory.
It is installed directly from its URL. There is no tap repository and no
symlink at the project root.

```sh
brew install https://raw.githubusercontent.com/gabrielvfonseca/replica/main/homebrew/Formula/replica.rb
```

- The formula installs the prebuilt binary for your architecture from the GitHub
  release (`replica-darwin-arm64` or `replica-darwin-x86_64`).
- For a build from `main`, install the formula from its URL with `--HEAD`.
- Upgrade: reinstall from the same URL after a new release is published.
- Background GC every 10 minutes: `brew services start replica`.

The release script ([`scripts/release.sh`](../scripts/release.sh)) writes the
checksums from the release artifacts, so do not edit them by hand.

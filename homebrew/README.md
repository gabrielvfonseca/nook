# Homebrew formula for Replica (not yet installable)

The formula is at [`Formula/replica.rb`](Formula/replica.rb). Its checksums match
the published v0.1.0 release artifacts.

**Status: `brew install` does not work yet.** Homebrew installs formulae from taps,
which need a `Formula/` directory at the root of a repository. This repository
does not have one, by design: the formula stays under `homebrew/`, and the root
`Formula` symlink was removed on request. Checked on Homebrew 7.0.12: installing
from a URL, a relative path, or an absolute path all fail with `Invalid tap name`.

Until a layout is chosen, install with the installer script
([`scripts/get-replica.sh`](../scripts/get-replica.sh)) or from source.

Options to make Homebrew work from this repository:

1. Restore a root `Formula/` (a symlink to `homebrew/Formula`, or a real directory),
   then users run `brew tap gabrielvfonseca/replica https://github.com/gabrielvfonseca/replica`,
   `brew trust gabrielvfonseca/replica`, and `brew install gabrielvfonseca/replica/replica`.
   This was verified locally.
2. Publish a separate `homebrew-tap` repository (the formula is ready for it).

# Benchmarks (plan; NO RESULTS YET)

Measure before building further. Record machine, macOS version, file count.

## Baselines (on your real project)
```sh
time cp -R proj /tmp/a          # copy
time cp -c -R proj /tmp/b       # clonefile (same volume only)
time rm -rf /tmp/a /tmp/b       # delete
time (git clone <url> c && cd c && npm ci)   # current setup path
```

## Where is time really going?
```sh
sudo fs_usage -w -f filesys <agent-pid> | head -n 500
sample <pid> 5
```
If time is dominated by process spawn or security scanning, Nook will not help much.

## Nook metrics
| Metric | Method |
|---|---|
| `nook new` latency | `hyperfine 'nook new --ttl 1m'` (cold + warm) |
| `nook rm` latency | compare vs `rm -rf` |
| Extra disk per clone | `nook stats` before/after |
| Disk after 100 runs + gc | `nook stats` |

## Decision rule
Adopt if clone+delete beats current setup+cleanup by >= 5-10x. Otherwise investigate
Spotlight/XProtect first.

## Results
_(empty)_

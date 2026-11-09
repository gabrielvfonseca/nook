# Replica skills

Agent skills for working with Replica. Each directory holds one `SKILL.md`,
which the skills.sh CLI and most coding agents can read.

| Skill | Use it when |
|---|---|
| [replica](replica/SKILL.md) | You need an isolated, ready-to-run workspace for a task |
| [replica-base](replica-base/SKILL.md) | Building, checking, or refreshing the golden base tree |
| [replica-maintenance](replica-maintenance/SKILL.md) | Disk is filling up, or gc is removing the wrong things |
| [replica-dev](replica-dev/SKILL.md) | Changing the Replica codebase |
| [replica-benchmarks](replica-benchmarks/SKILL.md) | Running benchmarks or publishing performance numbers |
| [replica-release](replica-release/SKILL.md) | Cutting a release and updating the Homebrew formula in `homebrew/` |

## Install

```sh
npx skills add gabrielvfonseca/replica            # all skills, from GitHub
npx skills add ./skills                            # from a local checkout
```

`replica setup` also installs the `replica` skill for Claude Code and Cursor.

## Check

```sh
make skills-check        # frontmatter: name matches the directory, description present
```

Each skill's `name` must match its directory, and its `description` says when to
use it. Keep both accurate, because agents choose skills from the description.

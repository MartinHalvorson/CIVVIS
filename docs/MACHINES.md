# Machine registry

Every branch in this repository names the machine that made it:
`agent/<machine>/<agent>/<task>-...`. That name is only useful if one
physical computer uses one name, and the fleet has never managed it: across
2,317 pull requests the branch prefix has taken **21 distinct values** for
six physical computers. That ambiguity is exactly what made the
cross-machine overwrite investigation of 2026-08-05 harder than it needed to
be — ownership tracking cannot work when the owner has no stable name.

`civvis_collab.py check-pr` reads this file and posts a **notice** (never a
failure) on any PR whose branch names a machine not listed here. To add a
machine, add a row; the notice tells you to do exactly that. Keep the ID
lowercase, hyphenated, and descriptive enough that a human can tell which
physical box it is (`martin-desktop`, not something like computer-2 —
anything in backticks in this file is treated as a registered ID, so keep
non-examples out of them).

## What actually resolves an alias

The join is already in the history and nobody had used it. A branch prefix is
chosen once and then copied forward; the `Computer:` line in a pull request's
ownership block is written from `scutil --get ComputerName` on the machine
that ran the work. One row of a PR body therefore carries both halves, and
they can simply be counted:

```bash
gh pr list -R MartinHalvorson/CIVVIS --state all --limit 3000 \
  --json headRefName,body --jq \
  '.[] | [(.headRefName | capture("^agent/(?<m>[^/]+)/").m),
          (.body | capture("(?m)^\\s*[-*]?\\s*Computer:?\\s*`?(?<c>[^`\n]+?)`?\\s*$").c)]
   | @tsv' | sort | uniq -c | sort -rn
```

That is how the table below was filled in on 2026-08-23, and it overturned
this document's own earlier guess: `martbot-mac` was listed as "likely the
same laptop" as `mbp-martin`, and 32 of its pull requests say
`Computer: mbp-m5-pro-64's MacBook Pro` — a different box entirely. Guessing
from how a name *looks* is what produced that error; counting the trailer is
what corrected it.

Two cautions, both learned by trying them:

- **`Computer: MacBook Pro` decides nothing on its own.** It is the macOS
  default name. 197 of the 199 pull requests carrying it come from IDs
  otherwise resolved to `mbp-m5-max-128`, which makes it suggestive for that
  box and useless everywhere else.
- **A shared agent ID proves nothing at all.** The agent IDs codex-root and
  codex ran
  on every machine in the fleet, so "these two prefixes share an agent" votes
  for whichever box was busiest. Only a date-stamped, single-purpose agent ID
  is worth anything, and it is still weaker than the trailer.

## Canonical machines

One row per physical computer. The **canonical ID is the name new work must
use**; it is what `python3 tools/civvis_collab.py start --machine <id>`
should be given, and the launcher then records it in repository-local Git
config (`git config civvis.machine`) so later tasks on that box inherit it.
`Aliases` are IDs the same machine has used before. They stay registered so
old branches keep parsing and the policy check stays quiet about them — a
historical branch name must remain readable — but nothing new should be
started under one.

| Canonical ID | Hardware | Aliases seen in history | Evidence |
| --- | --- | --- | --- |
| `martin-desktop` | Windows 11 desktop (RTX Pro 6000) | — | 7 PRs, `Computer: martin-desktop` |
| `martbot-9985` | AMD Ryzen Threadripper PRO 9985WX (64 cores / 128 threads) | — | 7 PRs, `Computer: martbot-9985` |
| `martbot-mbp-m4-max-128gb` | MacBook Pro M4 Max 128 GB | `mbp-m4-max-128-1`, `martbot-m4-max`, `martbot-mbp-m4` | 32 + 19 + 1 PRs name `Computer: mbp-m4-max-128-1`; `martbot-mbp-m4` is a truncation and the registry has one M4 machine |
| `mbp-m5-max-128` | MacBook Pro M5 Max 128 GB | `martbot-mbp-m5-max-128`, `martin-mbp`, `mbp-martin`, `macbook-pro`, `martbot-mbp-m5-max` | 42 + 37 PRs name `Computer: mbp-m5-max-128`; the box's own ComputerName was `Martbot-MBP-M5-Max-128` before it was renamed, and it is the only M5 Max in the fleet |
| `mbp-m5-pro-64` | MacBook Pro M5 Pro 64 GB | `martbot-mac`, `martbot` | 32 + 1 PRs name `Computer: mbp-m5-pro-64's MacBook Pro` |
| `martins-m4-air` | MacBook Air M4 16 GB | — | 8 PRs, `Computer: Martin's M4 Air` |

`martbot-mbp-m4-max-128gb` is the one canonical ID that is not its machine's
device name (`mbp-m4-max-128-1` is). It is left as it is deliberately: the ID
is already recorded in that machine's Git config and on 99 branches, and
renaming a canonical ID is that machine owner's call, not a passing sweep's.

## Unresolved aliases — operator to consolidate

⚠ **This list is derived, not remembered.** It was hand-written until
2026-08-23 and had gone stale in both directions: it named five IDs that the
trailer count resolves immediately, and it was missing eight IDs that were in
active use — one of them, `martbot`, used the same morning the list was
audited. A hand-written list of names is complete the day it is written.
Regenerate it instead:

```bash
gh pr list -R MartinHalvorson/CIVVIS --state all --limit 3000 \
  --json headRefName --jq '.[].headRefName' \
  | grep -oE '^agent/[^/]+' | sed 's|agent/||' | sort -u
```

Anything that command prints and this file does not contain is a new
unresolved alias. As of 2026-08-23 that leaves four, and every one of them is
unresolved for the same reason: **not one of their pull request bodies
carries a `Computer:` line**, because they predate the trailer becoming
routine. Nothing weaker than that line is worth registering a machine on.

| Alias | PRs | Window | Why it is still open |
| --- | --- | --- | --- |
| `martbot-mbp-m5` | 29 | 2026-07-24 → 2026-08-22 | A truncation that fits **two** rows — the M5 Max and the M5 Pro |
| `martbot-mbp` | 22 | 2026-07-31 → 2026-08-03 | Fits every MacBook Pro in the fleet |
| `martin-mac` | 14 | 2026-07-27 → 2026-08-05 | No trailer, and no agent ID it shares with a resolved row |
| `mac-martin` | 14 | 2026-08-03 → 2026-08-04 | No trailer. Lead, not proof: the dated agent opus5-loop-20260803 used this prefix and `mbp-m5-max-128` on the same day |
| `mbp` | 1 | 2026-08-03 | One PR, agent codex, no trailer |

If one of these is your machine, claim it: move it into the `Aliases` column
of your row above, with the evidence, in the same PR as your next task.
Unresolved aliases still pass the check — the goal is a ratchet toward one
name per box, not a wall in front of tonight's work.

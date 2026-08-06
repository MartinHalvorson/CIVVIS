# Machine registry

Every branch in this repository names the machine that made it:
`agent/<machine>/<agent>/<task>-...`. That name is only useful if one
physical computer uses one name. As of 2026-08-05 the history shows at
least ten machine IDs for what is probably four or five physical machines —
`martin-mac`, `mac-martin`, `mbp-martin`, and `martbot-mac` are likely the
same laptop introducing itself four ways — and that ambiguity is exactly
what made the cross-machine overwrite investigation of 2026-08-05 harder
than it needed to be: ownership tracking cannot work when the owner has no
stable name.

`civvis_collab.py check-pr` reads this file and posts a **notice** (never a
failure) on any PR whose branch names a machine not listed here. To add a
machine, add a row; the notice tells you to do exactly that. Keep the ID
lowercase, hyphenated, and descriptive enough that a human can tell which
physical box it is (`martin-desktop`, not something like computer-2 —
anything in backticks in this file is treated as a registered ID, so keep
non-examples out of them).

## Canonical machines

One row per physical computer. `Aliases` are IDs that machine has used
before; they stay valid so old branches stay parseable, but new work should
use the canonical ID.

| Canonical ID | Hardware | Aliases seen in history |
| --- | --- | --- |
| `martin-desktop` | Windows 11 desktop (RTX Pro 6000) | — |
| `martbot-mbp-m4-max-128gb` | MacBook Pro M4 Max 128 GB | `mbp-m4-max-128-1` |
| `mbp-m5-max-128` | MacBook Pro M5 Max 128 GB | `martbot-mbp-m5-max-128` |
| `mbp-m5-pro-64` | MacBook Pro M5 Pro 64 GB | — |

## Unresolved aliases — operator to consolidate

These IDs appear in branch history but have not been tied to a canonical
row. If one of these is your machine, claim it: move it to a canonical row
above (or add it as an alias of one) in the same PR as your next task.

- `martin-mac`
- `mac-martin`
- `mbp-martin`
- `martin-mbp`
- `martbot-mac`
- `martbot-mbp-m5-max-128` (listed above as an alias; shown here until its
  machine confirms)

Unresolved aliases still pass the check — the goal is a ratchet toward one
name per box, not a wall in front of tonight's work.

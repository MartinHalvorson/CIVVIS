# Two changes claimed the same ledger version

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

Not a treatment. While shipping #2079, `collaboration-policy` refused it: the
branch collided with #2070 on the same lines of `src/elo.rs` and `src/main.rs`.
Both were re-pinning the frozen anchor, and **both were setting
`ELO_PROTOCOL_VERSION = 15`** — for two independent changes to what
`advanced_v1` plays (barbarian scout raids; a Builder that can walk to a
pillaged tile).

So: what would have happened if they had not touched the same lines?

## How it was measured

By reading what each guard actually recomputes, and then planting the faults.

**The anchor pins are self-guarding.** `advanced_v1_plays_the_same_game_it_always_did`
recomputes the decision count and the fingerprint from the merged tree, so a
value carried over from before the other change landed fails in CI. That is why
the two constants are safe: they are *derived*, and the test derives them again.

**The version is not.** It is a number somebody types. A merge that keeps either
side's `15` is green — the constant is 15, the doc says 15, every test passes —
and two different rulesets then share one ledger identity. That is precisely
what the version exists to prevent: `docs/EVAL.md`'s rows are only comparable
within a version.

⚠ Note what saved this one: the two changes happened to edit the same *lines*.
`collaboration-policy` is a line-overlap check, not a semantic one. Two PRs
bumping the version while touching different parts of `elo.rs` would not have
collided at all.

## What was decided

**Shipped: the changelog above the constant is checked.** Its entries have been
the record of what each version means since v5, and three properties make a
duplicate impossible to land quietly:

| property | what it catches |
|---|---|
| every version named exactly once | two changes claiming the same number |
| no gaps between the first and last | a version whose meaning is unrecoverable |
| the newest entry equals the constant | a bump with no entry, or an entry with no bump |

All three were confirmed against planted faults rather than asserted:

```
duplicate v15  → these ledger versions are documented more than once, so two
                 rulesets share one identity ...: [15]
bump, no entry → the newest documented version is v15 and the code reports v16
gap            → these ledger versions are ... described nowhere: [11, 16, ...]
```

A census guard is included too: fewer than ten entries fails, because a scrape
that stops matching would otherwise pass an undocumented ledger.

## What this does not do

It does not stop two branches from *choosing* the same number — it stops the
second one from merging without noticing. That is the achievable guarantee: the
numbers are typed by people working in parallel, and the check is what turns a
silent collision into a red gate.

⚠ It also does not check `docs/ELO_REPINS.md`, which carries headings for only
some versions (v14 has none — v13's heading describes it). Requiring one there
would fail today for reasons that are not defects, and loosening the check until
it passed would be the wrong direction.

## Also underway, and not concluded here

Roadmap objective 3 records **35 shipped live treatments never named in any
round**. A treatment that never fires is dead code that can be removed without a
strength gate, because removing something provably inert is behaviour-preserving
— so the cheap question is which of them fire at all.

A screen of 33 `live_without_*` arms is running at a deliberately cheap profile
(4p 40x26, 150 turns, 10 pairs) as a *candidate filter*: an arm that fires there
is certainly not dead, and one that does not needs confirmation at the
deployment shape before anything is concluded. First result:
`live_without_district_coverage` reports #2003's "nothing differed". That is a
candidate and nothing more — the profile is too small to conclude inertness
from, which is the same trap #2042 recorded.

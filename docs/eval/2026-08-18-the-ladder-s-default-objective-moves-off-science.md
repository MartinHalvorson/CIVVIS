# the ladder's default objective moves off science

_2026-08-18 · `no new games`_

## What was asked

Two questions, and only the second needed a decision. First: which lane should
the Civilization VI ladder aim at, now that all six are reachable from the live
seat? Second, found while answering it: **how many places state that answer, and
do they agree?**

## How it was measured

Nothing new was run. This round spends the evidence already on the ledger:

- **Completion**, `victory_eval --games 8 --players 6 --turns 250 --speed
  online`, 96 games on two disjoint seed streams (`docs/EVAL.md`, 2026-08-17).
- **Strength**, `ai_eval <lane> advanced_target_science --deployment-comparison
  --players 6 --turns 250 --speed online`, 24 pairs per lane (same entry).
- **The host's own census**, 199 terminal events across 307 real
  Settler/Small/Online games (`docs/CIV6_LADDER.md`).

The audit half was a read of the four launchers and the two `tools/ops/`
supervisors that invoke them.

## What it measured

| lane | completes | vs the science-targeted incumbent | host terminals |
|---|---|---|---|
| diplomatic | **14/16** | +669 CONFIRMED (97.9%, 23-0-1) | **41** |
| culture | 12/16 | +137 INCONCLUSIVE | 24 |
| religious | 8/16 | +417 CONFIRMED | 5 |
| domination | 2/16 | +172 PASS | 3 |
| **science** | **0/16** | — (it is the incumbent) | 1 |

⚠ **The margins measure science's floor, not the winner's strength**, and that
is demonstrated rather than inferred: diplomatic vs religious — the fair fight
between two lanes that both finish — is 47.9%, −14 Elo (CI −150..+121),
p=1.0000, INCONCLUSIVE. Promoted effects on this ledger run +30..+40, so a +669
is a broken incumbent, not a discovery.

The audit found the second answer: the **list** of lanes was collapsed to one
source of truth in #1871; the **default** was not.

| where | what it said |
|---|---|
| `tools/civ6_play.py` | `science` |
| `tools/civ6_civvis_climb.py` | `science` |
| `tools/civ6_brain.py` | `science` |
| `tools/ops/civvis-batch-loop.sh` | `civvis` (untargeted), in three places |
| `tools/ops/civvis-chain-status.sh` | `civvis` |
| `tools/ops/civvis-game-supervisor.sh` | nothing — inherited `science` in silence |

The last row is the loop installed as a launchd service, so **the two production
supervisors were running two different experiments into one ledger**, and the
one that produces the ladder was aiming at the lane that completes 0/16.

## What was decided

The default moves to `diplomatic` and now has exactly one home,
`civ6_play.DEFAULT_CIVVIS_VICTORY`, with the evidence beside it. The other two
launchers import it; no `tools/ops/` script names a lane at all — the batch loop
and the status probe ask the tree they are about to play, which is the idiom the
batch loop already used for `--strategy auto`; and the supervisor gained a
`CIVVIS_VICTORY` knob so an objective it does not inherit is one it states.

Diplomacy is chosen among the lanes that land on the **host's** census, not on
the +669. This moves the aim off a lane that cannot finish; it does **not**
claim the new one is strong, and it is not a host prediction — sim→host transfer
of strength is CONFIRMED negative (`docs/AI_GAPS.md`).

`tools/test_ops_ladder_objective.py` holds it: the three launchers must resolve
to the same object, no `tools/ops/*.sh` may write a lane by hand (discovered by
glob, failing if the glob is empty), and the supervisor must keep the knob that
lets it speak. The guard was run against the shipped defect first and fails on
it.

⚠ Rows either side of this are **not** comparable and `code_rev` is what
separates them. Every one of the 307 attempts before it was aimed at a lane this
screen cannot make land, with three of Civilization VI's five victory conditions
priced at −10_000 in its own production valuation.

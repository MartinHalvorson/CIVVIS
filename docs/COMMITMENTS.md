# Commitments: decisions tracked to completion

Operator directive, 2026-08-27:

> As a whole, when we play CIVVIS we should be making decisions more
> decisively. Sometimes decisions are made and are slow to be carried out, or
> are forgotten. We should be making the best decisions, executing them to
> completion as fast as possible. Systematize this as solidly as possible.

This document is the system that answers it. It has three parts, in the order
they must exist: an **instrument** that counts every decision the controller
makes and what became of it; a **ledger** in the controller that turns each
multi-turn decision into a commitment with an owner, a target, a made-at
turn, an ETA and a per-turn progress check; and **genes** that act on the
ledger to close each failure class the instrument exposes.

## 1. The class of defect, as already observed

Every one of these was found and fixed *locally* by the loop over
2026-08-18…27. They are one class: a decision existed, and the carrying-out
was slow, lost, or silently reversed.

| where | the decision | what became of it | source |
|---|---|---|---|
| Settlers | settle site X | mean 7.3 turns build→found, p90 16, max 93; **19% of settlers lost**; 20% re-target; walked 1.57× the straight line | `settler_walk_census`, `docs/…` #2256 |
| Builders | improve tile T | pinned on an unreachable tile for 30 turns; Builders idle on 24.9% of their own turns; 25 Builders holding 73 charges at t180 | #2480 |
| Any walker | step to B | 2,015 walked round trips in 29,390 walked unit-turns (native); 10% of live movement walked back to its start | #2113 |
| Besiegers | take city C | arrival spread 7–11 turns — a 520-material train fed to a 165-material garrison a unit at a time; 4 sieges to 180–190/200 hp, 0 captures | #2614 |
| Live bridge | `[Move, Attack]` | one order per unit per turn; the strike was deferred and the turn released on the apply tick, so follow-ups lived only while the host refused end-turn | #2107, #2257 |
| Live bridge | found city at S | founded one hex short (where the settler stood) | #2257 |
| Grand strategy | posture P | ~4.1 switches per seat-game, 2.36 of them **unanchored** (57.7%) — no visible trigger | `docs/EVAL.md` 2026-07-29 |
| Great People | recruit G | pile up as blocked points (Engineers 14× price, Writers/Musicians with no slot) | #2248 |
| Wonders | build W | diplomatic lane finishes 1 wonder in 32 games; 31 of 53 wonders gated on an adjacent district that was never planned | wonder reachability census |

## 2. Prior art, and what it rules out

The repository has already tried **commitment as a lock** and it lost:

- 2026-07-29 (`docs/EVAL.md`, "what actually changes the adaptive
  strategy?"): generic hysteresis on the grand strategy was preregistered and
  then *not* promoted — "adding a generic cooldown now could suppress an
  urgent counter-plan while claiming to cure churn". Trigger-scoped
  commitment "reliably reduced churn and pointed +26/+35" but was
  inconclusive at 20 maps.
- 2026-07-31 (`docs/AI_GAPS.md`): "Strategy commitment regressed over 120
  maps, and champion-weight commitment regressed sharply."
- `settler_commit` **+30 Elo** (60/95, p=0.0061) — kept. The one commitment
  mechanism that paid, and it is a unit-level one.
- `siege-commitment` unresolved (+0.19pp, z +0.49 at 19,080 pairs);
  `unit-objective-memory` null; `settler-target-hysteresis` out of the genome.

So the lesson is not "commit harder". A lock on the *choice* was measured and
lost; the operator's complaint is about the *carrying-out*. The two are
separable, and only the second is this document's subject:

- **Choice** stays free to change every turn, as it does today. A change with
  a reason is adaptation; the instrument records the reason and moves on.
- **Execution** is what gets systematized: once a decision is made, the
  committed unit/city/army acts on it *this turn*, with all its movement, and
  keeps acting on it every turn until it completes — or until the controller
  *explicitly* resolves it (retarget, abandon), with the reason written down.
  A decision that simply stops being acted on, with its owner alive and its
  target still valid, is the defect ("forgotten"). A decision whose owner
  acts on it but makes no progress for k turns is the other defect
  ("stalled"). Both are counted, and each count has a gene whose job is to
  drive it to zero.

## 3. Principles

1. **A counter, not a clock.** The instrument counts decisions made /
   completed / stalled / forgotten / reversed / lost, and turns from decision
   to completion against the ETA the decision carried. It runs in every
   evaluator and on the live bridge, and it is not a gene.
2. **Explicit resolution.** No commitment ends without a recorded outcome.
   The ledger's `reconcile()` runs at the end of every turn; anything open
   whose owner did not act this turn is "forgotten" *by construction*, which
   is what makes the count honest and the fix local.
3. **Tempo is a first-class term.** The ETA is priced when the decision is
   made (walk turns, build turns), and every turn over it is a cost the
   planner sees — the same shape as `settle-sooner`'s turn price, generalized.
4. **Genes, opt-in, screened.** Every behaviour change is a gene per
   `docs/GENOME.md`; the ledger and its counters are infrastructure and ship
   on. The whole-game screen is the ship decision; the censuses are the
   evidence that a gene did what it claims.

## 4. The ledger (`src/ai/advanced/commitments.rs`)

The ledger is an **observer at the turn boundary**. `AdvancedAi` already
keeps the decisions as maps — `settler_targets` (unit → site),
`builder_targets` (unit → tile), the war plan's `objective_city`, and the
grand strategy's `target_city` under Conquest. At the end of every acting
turn, after the unit pass and before `EndTurn` (so `Unit::acted` still says
what each unit did), `reconcile_commitments` compares those maps with what
it saw the turn before and classifies every decision:

| ending | meaning |
|---|---|
| `completed` | our city stands on the site / the target tile's improvement changed / the target city is ours |
| `retargeted` | the same owner now points at a different target — split into *before moving* and *en route* (it had already got closer) |
| `dropped` | the owner is alive and the decision is gone (a target drop, a stand-down) |
| `settled elsewhere` | the settler vanished and a city of ours that is new this turn stands within 3 of where it stood |
| `lost` | the owner died or was captured with the decision open |
| `stood down` | the empire no longer means to take that city |

and, for every decision still open, three per-turn readings:

| reading | definition |
|---|---|
| **forgotten** | the owner had movement to spend and did not act (`!acted && moves_left > 0`); for a declared war, no own military unit within 3 hexes of the objective |
| **stalled** | acted, but no better progress reading (hexes to walk; phase-then-hit-points for a capture) for `STALL_TURNS` = 3 turns — the same limit as `SETTLER_STALL_LIMIT` |
| **late** | past the ETA priced when the decision was made: a walk at two hexes a turn, the war plan's own research + production + march estimate, or `CONQUEST_ETA_TURNS` = 20 (`CAMPAIGN_PATIENCE`) for a bare Conquest target |

Every forgotten turn is also filed under the first hold the observer can see
(`forgotten_why`): waiting for escort, threat forecast on the site, hostile
within two, stall counted, in a city, at the tile with the build refused,
walk refused or not attempted, unexplained. That split is what names the
gene a class wants.

**Where it surfaces.** Three places, so nothing has to be re-plumbed:
`players[pid].counters` under `commit:<kind>:<field>` and summed
`commit:<field>`, which `gene_screen` lifts onto every seat row as
`commit_*` (so every future screen carries the decisiveness of every
genome); `audit` prints one `commitments <kind> …` line per kind per game
and pooled; and the `#[ignore]` census `commitment_census` (`cargo test
--profile ci commitment_census -- --ignored --nocapture`, eight 6-player
60×38 Online maps at the deployment genome, `CIVVIS_CENSUS_OPT_INS=tag,tag`
switches opt-in genes on) prints the pooled reading with the endings and
the forgotten-why split.

**It changes no decision.** Proof: the three-game `gene_screen` block at seeds
98600000–98600002 played by the exact base commit and by this branch gives
**19 of 19 seat rows identical** apart from the new `commit_*` columns, the
build stamp and wall-clock seconds.

## 5. The first reading (2026-08-27, 8 maps, deployment genome)

```
settle  made 687  done 201 (29%)  retargeted 309  dropped 151  lost 17
        open turns 3206: forgotten 784 (24%)  stalled 592 (18%)  late 1525 (47%)
        done in 4.1 turns v eta 1.9
improve made 4258 done 2679 (64%) retargeted 650  dropped 568  lost 240
        open turns 14908: forgotten 3975 (26%) stalled 1763 (11%) late 7260 (48%)
        done in 3.7 turns v eta 1.9
capture made 263  done 10 (4%)    retargeted 65   stood down 168
        open turns 2044: forgotten 698 (34%)  stalled 290 (14%)  late 279 (13%)
        done in 18.1 turns v eta 20.0
```

Retargets: settle 159 before moving / **150 en route**; improve 313 / 337.

Forgotten, by hold:

| kind | hold | turns |
|---|---|---:|
| settle | hostile within two | **577** |
| settle | unexplained | 99 |
| settle | stall counted (route refused) | 87 |
| settle | threat forecast on the site | 19 |
| settle | in a city | 2 |
| improve | hostile within two | **2107** |
| improve | walk refused or not attempted | **1713** |
| improve | in a city | 125 |
| improve | at the tile, build refused | 30 |
| capture | nobody at the objective | **698** |

## 6. What the reading says

1. **The conquest decision is the least executed decision in the game.**
   263 times an empire's grand strategy named a city to take; 10 of those
   cities fell. 168 decisions were stood down and 65 re-aimed, and on a third
   of the turns a target was held, no unit of ours was within three hexes of
   it. This is the class `docs/EVAL.md` (2026-07-29) measured as ~4 strategy
   switches per seat-game, now seen from the objective's side: the choice
   changes because nothing was ever sent, not the other way round.
2. **Civilians freeze near a hostile.** 577 settler-turns and 2,107
   Builder-turns — about 45 unit-turns per seat-game — a unit with a target
   and movement stood still because a hostile (barbarians included) was
   within two hexes. Standing still next to a raider is the one option that
   is never right: the settler-hold comment in `advanced_settler_step`
   records two live settlers taken exactly that way.
3. **Builders hold a pin they never walk to** — 1,713 turns. This is the
   #2480 defect (`builder_targets` survives its own refused route) and its
   gene, `builder-tries-the-next-tile`, ships off.
4. **Half of settler retargets happen after the walk has started.** 150 of
   309. A decision reversed mid-execution costs the walk already sunk, and
   the settle ranking does not see that cost.
5. **Everything runs at half its priced pace.** Settle and improve complete
   in ~4 turns against a 1.9-turn flat-ground price; half of all open
   commitment-turns are past their ETA.

The settler holds (items 2 and 4 on the settler side) are the subject of
PR #2655, which owns `advanced_settler_step`; this document records the
numbers and leaves that lane alone.

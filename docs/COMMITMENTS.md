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

## 4. The ledger

*(filled in as it lands — see the sections below for the structure, the
census columns, the genes and the screens.)*

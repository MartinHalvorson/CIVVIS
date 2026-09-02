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
| `stood down` | the empire no longer means to take that city — split by whether anyone ever went (`nobody ever went` / `always present` / `went then left`) and, once there, whether the siege ever pushed the city below where it stood when the decision was made (`engaged` / `never scratched it`) |

and, for every decision still open, three per-turn readings:

| reading | definition |
|---|---|
| **forgotten** | the owner had movement to spend and did not act (`!acted && moves_left > 0`); for a declared war, no own military unit within 3 hexes of the objective |
| **stalled** | acted, but no better progress reading (hexes to walk; phase-then-hit-points for a capture) for `STALL_TURNS` = 3 turns — the same limit as `SETTLER_STALL_LIMIT`. A commitment also carries `stalled_streak` (consecutive stalled readings, reset by a new low or a forgotten turn) and `engaged` (the reading has been below its initial value) |
| **late** | past the ETA priced when the decision was made: the terrain walk in turns (`settle_sooner_walk_costs` over `step_cost`, at the unit's movement allowance; a hex a turn beyond `WALK_PRICE_RADIUS` = 16), the war plan's own research + production + march estimate, or `CONQUEST_ETA_TURNS` = 20 (`CAMPAIGN_PATIENCE`) for a bare Conquest target. Until 2026-08-27 the walk was priced at two hexes a turn; §5's `late` and ETA figures are on that price |

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

## 7. Existing genes, priced on this axis

Each opt-in gene was switched on for every major (`CIVVIS_CENSUS_OPT_INS`)
on the same eight maps. These are **not paired** — a gene that changes any
decision changes the game's whole trajectory — so only large moves read.

| gene | what moved | what did not |
|---|---|---|
| `civilian-out-of-reach` | nothing: **byte-identical** to the baseline — it is already ON in the deployment genome | the 2,684 freeze turns happen *with* it on; "waits outside the raider's reach" **is** the freeze |
| `settler-target-hysteresis-2` | nothing: byte-identical — also already ON | the 309 settler retargets happen with it on |
| `builder-tries-the-next-tile` (#2480, off) | improve made 5,790 (+36%), dropped 1,955 (3.4×, by design), done in 3.1 turns (v 3.7); walk-not-attempted 1,469 (v 1,713) | completions 2,830 (v 2,679); forgotten share 27% |
| `city-campaign` (off) | capture made 394 (+50%); nobody-at-the-objective 20% of turns (v 34%) | captures **10** either way |
| `unit-objective-memory` (off) | — | nothing on any class |

**No existing gene closes any of the classes in §6.** That is the finding
that justifies new genes rather than a default flip.

## 8. Genes that act on the ledger

### `capture-go-or-stand-down` (opt-in, ships off)

The first. A declared war's objective that no unit of ours has been within
`CAPTURE_PRESENCE_RADIUS` = 3 hexes of for `CAPTURE_GO_TURNS` = 6
consecutive turns is **stood down explicitly**: the city leaves the target
ranking for `CAPTURE_STAND_DOWN_TURNS` = 20 turns (never a home emergency),
`plan_stale` re-assesses the strategy next turn instead of on the five-turn
cadence, a journal line names the city and the streak, and
`commit:capture:gene_stand_downs` counts it. Six turns is longer than any
march the war plan prices for a target it appointed, so a streak that long
is a target no army is going to. Untreated, the same target is held until
the cadence happens to drop it — 168 of 263 decisions ended that way.

Fires on two disjoint 6-game probe blocks (seeds 99400000 and 99400100):
win +14.8 pp and +7.4 pp, share +2.3 and −1.0 — probe-sized, not a
measurement. The standard screen prices it, on wins and on the two columns
below.

### `commitment-patience` (opt-in, ships off)

The follow-up that closes §6's items 2–4 with one mechanism. The reading
said the same thing three ways: a target survives no hold. A passing raider
within two hexes freezes the unit (2,684 turns), and — through the settler's
two threat drop reasons ("step risk above the limit", "deferred for a
visible threat") and the Builder's reach filter — also *drops the target*,
so the unit re-ranks, picks a site elsewhere, and walks again: 150 of 309
settler retargets and 337 of 650 Builder retargets came after the owner had
already got closer. And a pin the Builder cannot reach was held for ever
(1,713 turns).

With the gene on, **a target is a commitment with bounded patience**: the
threat reasons no longer drop it (the unit holds or retreats exactly as
before — nothing walks into a raider's reach), and the ledger retires any
settle or improve commitment forgotten for `COMMITMENT_PATIENCE` = 3
consecutive turns, parking the site for the hysteresis window
(`settler_dead_sites` for a settler; a Builder's `builder_avoid` tile joins
its `reserved` set) so the next pick is a different one. A raider that moves
on inside three turns costs nothing; a camp guard that does not costs three
turns and then a new decision. Journal line, `commit:<kind>:retired`
counters, and a `retired` ending in the census.

**Same eight maps, gene off → on** (same seeds; a gene that changes a
decision changes the game, so these are magnitudes, not a paired test):

| | forgotten turns | stalled | late | retargets (en route) | lost | done | turns to complete |
|---|---:|---:|---:|---:|---:|---:|---:|
| settle, off | 1,077 (30%) | 611 | 41% | 339 (175) | 21 | 212 (29%) | 4.1 |
| settle, on | **269 (11%)** | **160** | **15%** | 219 (126) | **6** | 216 (31%) | 3.7 |
| improve, off | 4,175 (26%) | 1,784 | 36% | 644 (310) | 256 | 2,870 (64%) | 3.8 |
| improve, on | **1,964 (16%)** | 1,296 | 27% | **102 (35)** | 188 | 2,579 (67%) | 3.4 |

Forgotten by hold, settle: hostile within two 806 → 240, unexplained
212 → 13, stall-counted 47 → 15. Improve: pin never walked to 1,914 →
**433**, hostile within two 2,142 → 1,478. Retired: 42 settle, 397 improve.

Probe (two disjoint 6-game blocks, seeds 99500000 and 99500100, the
decisiveness columns of §9): forgotten-turn share **−7.7 ±3.0 pp and
−6.4 ±0.8 pp**, decisions completed +10.1 ±3.7 / −0.4 ±2.1 pp, score share
+3.9 and +3.1 pp (merged +3.4 ±1.5, z +2.2), win Δ +20 / −21 pp — the
probe-sized noise a 36-seat block always has. The standard screen prices it
on wins; the columns say it does what it claims.

### The ETA is now the terrain walk

Item 4. A new decision is priced once, when it is made, at
`settle_sooner_walk_costs` (movement points over `step_cost`) divided by the
unit's allowance — a hex a turn beyond `WALK_PRICE_RADIUS` = 16 — instead of
two hexes a turn. On the same maps the settle ETA moved 1.9 → 3.1 turns
against 4.1 actual and the improve ETA 1.9 → 2.8 against 3.8, so `late`
now reads indecision rather than hills: 41% / 36% of open turns before the
gene, 15% / 27% with it.

### `capture-go-or-stand-down-2` (opt-in, ships off; one version of the family plays)

The capture side, one level deeper. The first ledger split said 137 of 182
failed conquests had bodies at the objective throughout; the question was
what those bodies were doing. Every commitment now carries `engaged` — the
reading has been below its initial value at least once, i.e. the siege
pushed the city lower than when the decision was made — and
`stalled_streak`, consecutive stalled readings with the army present. The
capture endings split on both (2026-08-28, eight maps, gene off):

| how a failed conquest ended | count |
|---|---:|
| nobody ever went | 49 |
| always present, **never scratched it** | **91** |
| always present, engaged | 11 |
| went then left, never scratched it | 78 |
| went then left, engaged | 15 |

**169 of 244 failed conquests never pushed the city below its starting
strength.** The army arrives and does not dent the city; only 26 sieges
that failed were ever winning. That is not a commitment defect at the
boundary — the decision was made, staffed and held — it is the assault
that never starts: force-group posture (`hold_weak` below
`LOCAL_SUPERIORITY_FLOOR`) and siege composition (walls without a siege
unit). The next instrument is a split of "never scratched it" by whether a
siege-class unit was present and by the group's posture at the objective;
the arena (`the_storming`) is where the answer gets priced.

Version 2 adds the second trigger the split allows: a siege with bodies at
the objective that has not pushed the city to a new low for
`CAPTURE_STALL_TURNS` = 6 consecutive stalled readings (nine turns without
a new low) is stood down the same way version 1 stands down an objective
nobody went to — out of the ranking for twenty turns, strategy re-assessed
now, `commit:capture:stall_stand_downs`. Same eight maps, off → on: captures
4 → 8, stalled capture-turns 220 → 139, nobody-at-the-objective turns
624 → 503, failed sieges resolved in 6.5 turns instead of 17; "went then
left, never scratched it" 78 → 60. "Always present, never scratched it"
91 → 84 — a stand-down cannot start an assault. Probe on two disjoint
blocks: win −5.4 / −2.0 pp, share −3.5 / +2.5 — probe noise; the standard
screen prices it.

### `commitment-owner-acts` (opt-in, ships off)

The class the two civilian genes above leave standing, re-measured on
2026-09-01 at the deployment genome as it then shipped (`commitment-patience`
and `capture-go-or-stand-down-2` on; `civilian-out-of-reach` off), twelve
6-player 60×38 Online maps, seeds 98500000–98500011:

```
settle  made 2064 done 311 (15%) retargeted 784 dropped 939 lost 14 · open turns 6394: forgotten 1481 (23%) stalled 233 (3%) late 711 (11%) · done in 3.4 turns v eta 2.8
improve made 6106 done 4173 (69%) retargeted 176 dropped 1387 lost 234 · open turns 18105: forgotten 2873 (15%) stalled 1514 (8%) late 4366 (24%) · done in 3.4 turns v eta 2.8
capture made 366 done 4 (1%)    retargeted 115 dropped 217 lost 0   · open turns 2185: forgotten 501 (22%)  stalled 146 (6%)  late 90 (4%)    · done in 13.8 turns v eta 20.0
```

Forgotten, by hold: settle — hostile within two **1,372**, stall counted 50,
unexplained 34, threat forecast 19, in a city 6; improve — hostile within
two **2,400**, walk refused or not attempted 442, at the tile 16, in a city
15; capture — nobody at the objective 501. Endings: settle retired 340,
improve retired 690 (`commitment-patience`); capture stood down 217.

The forgotten turn is still the largest open class on both civilian kinds,
and 87% of it is one hold: **a hostile within two hexes and the owner does
not act** — `commitment-patience` retires the target after three such turns
(1,030 retirements), and for those three turns the owner stands still with
movement to spend and an order it was given but never carried out. The
remainder is a pin the Builder never walks to and the settler holds the
observer cannot name.

With the gene on, **every open settle or improve commitment whose owner the
unit pass left with movement and no order acts on it, or resolves it, the
same turn.** After `advanced_units` and before the ledger's reading, each
such owner gets the commitment's own next step:

- standing on the target: the city is founded (under the same step-risk and
  loyalty bars the unit pass applies) or the best improvement that fits the
  tile is laid;
- otherwise one route step toward the target, when no hostile can reach that
  step (`barbarian_reach` / `civilian_safe_at`, the civilian-safety envelope)
  and, for a settler, the step is under `SETTLER_STEP_RISK_LIMIT`;
- or the commitment is **released** with a recorded reason: the step is
  illegal (no route step, a refused order, a site that cannot be founded, a
  tile nothing fits) or the owner itself stands inside a hostile's reach —
  where the civilian-safety flee keeps priority and a decision must not pull
  the unit deeper in. A released site is parked for the hysteresis window
  the way `commitment-patience` parks a retired one.

The one hold it leaves standing is a next step that is itself inside a
hostile's reach or over the settler risk limit: a passing raider is a wait,
not a new decision, and `commitment-patience` prices how long. Journal line
per act (`… acts on the decision the unit pass left standing`, `… releases
the decision it was not acting on`), `commit:<kind>:owner_{stepped,
completed,held,released}` counters, and a `released` ending in the census.

**Same twelve maps, gene off → on** (same seeds; a gene that changes a
decision changes the game, so these are magnitudes, not a paired test):

| | made | done | retargets (en route) | dropped | lost | forgotten turns | stalled | late | turns to complete (v eta) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| settle, off | 2,064 | 311 (15%) | 784 (419) | 939 | 14 | 1,481 (23%) | 233 | 711 (11%) | 3.4 (v 2.8) |
| settle, on | 1,875 | 301 (16%) | 708 (366) | 835 | 22 | **847 (15%)** | 242 | 595 (11%) | 3.6 (v 2.9) |
| improve, off | 6,106 | 4,173 (69%) | 176 (46) | 1,387 | 234 | 2,873 (15%) | 1,514 | 4,366 (24%) | 3.4 (v 2.8) |
| improve, on | 5,954 | 4,147 (71%) | 160 (67) | 1,303 | 181 | **1,846 (11%)** | 1,197 | 3,910 (23%) | 3.4 (v 2.7) |

Forgotten, by hold (off → on): settle — hostile within two 1,372 → **782**,
stall counted 50 → 31, unexplained 34 → 25, threat forecast 19 → 3; improve
— hostile within two 2,400 → **1,684**, walk refused or not attempted
442 → **155**, at the tile 16 → 7, in a city 15 → 0. Endings: released 159
settle / 187 improve; retired (`commitment-patience`) 340 → 157 settle,
690 → 427 improve — the patience gene now retires what the owner could not
act on, not what it was never asked to. Civilian forgotten turns
4,354 → 2,693 (−38%); civilians lost 248 → 203; done 15% → 16% settle,
69% → 71% improve; the completion pace is flat (3.4 → 3.6 / 3.4 → 3.4
turns) — the class that closed was the standing still, and what remains is
the wait the gene leaves on purpose: the next step is inside a hostile's
reach (782 + 1,684 of the 2,693). The capture kind is untouched by the gene
(done 4 → 13 is trajectory noise).

What the gene did on those maps, its own counters pooled over every major
(`commitment_census` now prints every `commit:` counter the census itself
does not export): settle — 32 route steps, 1 founding, 536 holds (the next
step inside a hostile's reach or over the settler risk limit), 283 releases;
improve — 633 route steps, 93 improvements laid, 184 holds, 304 releases.
A settler the unit pass leaves standing is almost always one the step-risk
guard refused, so the gene's settle work is the explicit resolution — a
release, or a hold with its reason in the journal; the Builder work is
actuation, 726 orders the pass never issued.

### Still open

- **The assault that never starts** (above): 169 of 244 failed conquests
  never scratched the city. Posture and siege composition at the objective,
  measured by the arena, not by this ledger.
- The genes ship off until the standard screen prices them; the continuous
  batch re-prices their decisiveness every rotation (§9).

## 9. The standing measurement

Every seat row a screen writes now carries `commit_*`, and
`gene_screen --analyze` prices every gene's on−off Δ on **decisions
completed** (`done_delta_pp`) and **commitment-turns forgotten**
(`forgotten_delta_pp`) beside its win and share Δ, in the JSON and as a
`decisiveness` block after the table. The continuous batch
(`docs/GENE_SCREEN.md`) therefore re-prices the decisiveness of the whole
genome every rotation without anyone asking. `commitment_census` is
registered in `docs/CENSUS.md`, so the scheduled census run holds the
reading against drift.

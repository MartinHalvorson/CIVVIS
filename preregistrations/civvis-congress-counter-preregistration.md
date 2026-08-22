# Pre-registration — aiming the World Congress at the empire about to win

Written 2026-07-29, before any `ai_eval` run of these arms. PR #536, worktree
`/Users/martin/civvis-diplo-counter-e677`.

## What is being tested

`AdvancedAi::congress_choice` resolves every leader-targeting term to
`diplomatic_leader` — the empire holding the most Diplomatic Victory Points.
`congress_census` (this PR) measures that target, over congress sessions of
decided games:

| profile | dvp leader is the winner | base rate | score leader is the winner |
|---|---|---|---|
| 4p 60×38, 6 CS, 214 sessions | **24.8%** | 25.0% | 61.2% |
| 6p 74×46, 9 CS, 181 sessions | **14.4%** | 16.7% | 60.8% |

At or below chance on both. `advanced_congress_counter` points the three
resolutions that carry a *targeted penalty* — `trade_policy` B (total trade
embargo), `migration_treaty` B (−20% growth; scores 0.0 against any rival
today, so the penalty can never be aimed at anybody), `border_control_treaty` B
(no tile annexation from border growth) — at the empire `victory_denial`
already names. `world_leader` is deliberately unchanged.

`advanced_congress_votes` is the decomposition arm: same aim as shipped, but a
ballot opposing the named empire is backed with the second and third vote.

## Why this is not the axis `docs/COUNTERING_LEADERS.md` closed

That document's seven arms all measured null, and at deployment scale the
shipped response *costs terminal score* (44 map-directions to 65, p=0.055)
while holding 18% more gold. The mechanism it settled on: "nothing is recovered
by not fighting because the alternative use of the resources is not being made
either."

A ballot is not a resource reallocation. `Game::resolve_congress` refunds a
losing vote **in full** and a right-outcome/wrong-target vote at **half**, and
Favor has no sink but votes and deals. So this treatment can be wrong at no
cost, which is not true of any arm in that document.

## Predictions, registered before the run

1. **Wins: null.** Paired-map score for `advanced` in **48–52%**, sign p > 0.10.
   This repo's base rate for response-side counter-leader treatments is seven
   nulls out of seven, and the intervention is rare: ~49% of sessions offer a
   penalty resolution at all, the denial layer has to be naming somebody, and
   outcome B has to carry the vote — I expect **1–3 landed penalties per game**.
2. **Terminal score: null**, 48–52%. If anything moves, I expect it here rather
   than in wins, because the mechanism is an economic penalty on one rival.
3. **The fires-check WILL separate the arms.** `--arm counter` must raise
   "ballots naming this voter's own denial target" and must raise "targeted
   penalties landing on the eventual winner" above the `ship` arm's base rate.
   If it does not, the eval is measuring nothing and must not be run.
4. **`advanced_congress_votes` moves less than `advanced_congress_counter`.**
   The census finds opposition on `world_leader` already unanimous and winning
   98.5% of the time, so extra votes mostly buy an outcome already won.

## What would refute a positive reading

- A 120-pair discovery run is **not** evidence. `docs/COUNTERING_LEADERS.md`
  produced two false positives at 120 pairs, one at p=0.0225, both refuted by
  their own 360-pair confirmations at disjoint seeds. Any positive here must be
  confirmed at **360 pairs on a disjoint seed**; regression to 49–51% refutes.
- Pooling the discovery seed with its confirmation is illegitimate and will not
  be done.

## Runs

Deployment density, not the `ai_eval` 24×16 default
([[civvis-eval-defaults-are-not-the-deployment]]):

```
ai_eval advanced advanced_congress_counter --players 4 --city-states 6 \
    --width 60 --height 38 --pairs 120 --turns 400 --seed 990000
```
→ `/Users/martin/eval-congress-counter-120.log`

## ADDENDUM 2026-07-29 — the fires-check, run before the eval

12 maps, 4p 60×38, 6 city-states, seed 983000,
`congress_census --arm ship|counter|votes|hard`
(`/Users/martin/congress-fires-check.log`):

| arm | ballots naming own denial target | ballots with a bought vote | targeted penalties passed | landed on the eventual winner |
|---|---|---|---|---|
| `ship` | 2.5% (19/773) | 0.6% (5) | 7 — 0.58/game | 4/7 — 57.1% |
| **`counter`** | **7.3%** (58/794) | 0.8% (6) | **17 — 1.42/game** | **12/17 — 70.6%** |
| `votes` | 2.5% (19/773) | **1.9%** (15) | 7 — 0.58/game | 4/7 — 57.1% |
| `hard` | 7.7% (60/776) | **7.2%** (56) | 17 — 1.42/game | 12/17 — 70.6% |

Base rate 25.0%.

**Registered prediction 3 holds**: `counter` nearly triples the aimed ballots
*and* the penalties that actually pass, and lands them on the eventual winner
70.6% of the time. Not a silent no-op; the eval is worth running.

**Registered prediction 4 holds, and more strongly than I wrote it.** I
predicted `advanced_congress_votes` would "move less". It moves *nothing*:
against `ship` it triples the bought votes and reproduces the aimed-ballot
count, the penalty count and the landing rate **exactly** (19/773, 7, 4/7).
`hard` against `counter` is the same story — 9× the bought votes, identical
17 and 12/17. **Extra votes flip no resolution in either aim.**

Mechanism: the bandwagon term (`observed(choice) * 35.0`) makes ballots
converge, so winning margins are wide and one voter's extra two votes cannot
carry an outcome. **`advanced_congress_votes` and
`advanced_congress_counter_hard` are therefore retired here rather than
evaluated** — an inert arm's eval measures nothing. Only
`advanced_congress_counter` goes to `ai_eval`.

## ADDENDUM 2 — 2026-07-29, after the first eval. I retired an arm too early.

`advanced_congress_counter` returned an **exact dead heat**: 50.0% paired,
50.0% terminal score, both sign p=1.0000, wins resting on 4 of 120 maps
(`/Users/martin/eval-congress-counter-120.log`). Registered prediction 1 was
"null, 48–52%, p>0.10". It landed at 50.0%.

**But the harness gave the mechanism half the votes.** `ai_eval` runs a
mirrored 2v2, so two treated seats vote B on the leader and two control seats
vote A on themselves — outcome A and outcome B tie, and `resolve_congress`
breaks a tie toward A. Measured rather than asserted, with a new
`congress_census --arm mixed` that treats every other seat:

| arm | aimed ballots | bought votes | penalties passed | landed on winner |
|---|---|---|---|---|
| `ship` | 2.5% (19/773) | 0.6% (5) | 7 — 0.58/game | 4/7 — 57.1% |
| `counter` (all seats) | 7.3% (58/794) | 0.8% (6) | 17 — 1.42/game | 12/17 — 70.6% |
| **`mixed` (half seats)** | 4.6% (36/776) | 0.4% (3) | **7 — 0.58/game** | 6/7 — 85.7% |
| **`mixed_hard`** | 6.1% (47/776) | **3.7% (29)** | **14 — 1.17/game** | **14/14 — 100%** |

`mixed` passes **exactly the control's penalty count** — the treated half aims
and never carries a vote. That is the signature of a mechanism that did not
fire, and it is why the eval was a dead heat.

**★ My "the vote-weight lever is inert" conclusion was wrong, and wrong for a
specific reason I can name:** I measured it in the all-seats arm, where the
opposition already carried every vote it cared about, and in the shipped arm,
where the aim was wrong. Vote-buying only matters *at the margin* — and a
mirrored 2v2 is exactly the margin. At half the table it doubles the penalties
that pass and lands **all fourteen on the eventual winner**.

### Registered prediction for `advanced_congress_counter_hard`

Run: `ai_eval advanced advanced_congress_counter_hard --players 4
--city-states 6 --width 60 --height 38 --pairs 120 --turns 400 --seed 991000`
→ `/Users/martin/eval-congress-counter-hard-120.log`

1. **Modal expectation is still null**, `advanced` paired score **48–52%**,
   sign p > 0.10. Seven prior nulls on this axis and one just now; 1.17
   landed penalties per game is a thin intervention even aimed perfectly.
2. If anything moves it should be **terminal score before wins**, because the
   mechanism is an economic penalty rather than a kill.
3. Direction if it works: `advanced`'s score goes **below** 50%.
4. **A reading below 47% would be the first non-null on this axis in eight
   attempts and I will not claim it from this run.** Confirmation at 360 pairs
   on disjoint seed 992000 is required; regression to 49–51% refutes. Pooling
   discovery with confirmation is illegitimate and will not be done.

## RESULT 2 — 2026-07-29. Null again, and this time the mechanism fired.

`/Users/martin/eval-congress-counter-hard-120.log`:

| reading | value |
|---|---|
| game-win share | 121/240 vs 119/240 |
| paired score for `advanced` | **50.4%** (Wilson 41.6–59.2), Elo **+3** |
| paired direction | 3 / 115 / 2, sign **p=1.0000** |
| terminal score | **50.1%**, 19 / 89 / 12, **p=0.2810** |
| resolution | wins on 5 of 120 maps, terminal score on 31 |
| gate | INCONCLUSIVE |

**All four registered predictions held.** (1) null at 48–52%: landed 50.4%.
(2) terminal score would move before wins: it did, 19–12, and still null.
(3) direction if it worked: `advanced` below 50% — it is *above*. (4) nothing
below 47%, so nothing to confirm and no 360-pair run is owed.

**This is the arm that closes the question**, because it is the one where the
mechanism was measured firing in this exact seating: 14 landed penalties over
12 games, all fourteen on the eventual winner. The null therefore cannot be
attributed to mis-aiming, and cannot be attributed to cost, since a losing vote
is refunded in full.

Stated to the repo's own standard: **worth less than this run can resolve**,
not zero. 1.2 penalties per game is thin and wins rest on 5 of 120 maps. Sizing
it needs the oracle-immunity grant handed off in `docs/COUNTERING_LEADERS.md`,
which bounds the value of *all* Congress penalties instead of the AI's ability
to aim them.

# Recovering captive Spies with an executable cash quote

Status: **preregistered; no evaluator implementation exists and no focal seed
has been generated or read**.

## Exploratory production evidence

The frozen prevalence census is the latest 50 completed production saves
through
`20260729T152436.680936Z-seed-129245755-turn-246-instance-30801`. These are
rolling live revisions, not a homogeneous experiment. They contain 400 major
civilization endpoints at a mean terminal turn of 271.3:

| endpoint observation | count |
|---|---:|
| Spies owned by majors | 1,589 |
| captive Spies | **751 (47.3%)** |
| seats with at least one captive | 350 / 400 |
| seats with at least two captives | 241 / 400 |
| seats with Spies but no usable non-captive Spy | 16 / 400 |
| captives held by another living major at peace | **626** |
| total levels / promotions on captives | 1,101 / 846 |

Winners held more Spies and more captives than non-winners. That is expected
from longer survival, technology, and capacity and is not evidence that capture
helps or that recovery wins games. The archive establishes prevalence only.

A source-identical diagnostic loaded a moving latest-50 window after the
census. Twenty-two old endpoints did not deserialize under that head; across
the remaining 378 major-seat endpoints, `Game::quick_deals` offered **zero**
`recover_spy` quotes. One inspected eight-major endpoint held 20 captive Spies,
all between majors then at peace. Seven owners could legally recover a captive
by rounding the mutually beneficial midpoint up to whole lump Gold; the owner
with zero Gold could not. The ordinary market exposed no recovery quote to any
of the eight.

These reads happened before this preregistration and are hypothesis-generating.
No treatment outcome or focal seed was inspected.

## Exact source mechanism

Captive recovery and release already execute through ordinary `Trade` actions.
The owner values an occupied, unusable Spy-capacity slot at

`325 + 70 * level + 35 * promotions`,

while the captor's release cost is

`90 + 30 * level + 15 * promotions`.

The fair midpoint is therefore always

`207.5 + 50 * level + 25 * promotions`.

`quoted_payment` floors the lump payment. Even when the owner can pay the
whole midpoint from cash, the remaining 0.5 becomes 0.1 Gold per turn. The
quote is then rejected if a conservative liquidity proxy—four Gold per city
plus one percent of treasury, less unit and infrastructure maintenance—is
zero. Large late-game armies commonly drive that proxy to zero despite a large
positive live treasury and positive actual income. Because every captive-Spy
midpoint ends in 0.5, the same fractional tail recurs for every recovery.

The AI cannot build around the failure. Captive Spies still count against
`spy_capacity`, `legal_spy_actions` returns no action for them, and producing a
replacement is rejected while the occupied capacity slot remains. Basic and
Advanced diplomacy already compare mutually beneficial Quick Deals, but a
recovery quote removed before that comparison can never be selected.

## Frozen hypothesis and treatment

> On the stock diplomacy cadence, spending real lump Gold to recover the most
> experienced affordable captive will restore working espionage capacity and
> improve terminal strategic strength without harming wins.

The evaluator changes no engine rule and no shipped AI default. For each focal
treatment turn it:

1. clones the current game and focal `AdvancedAi`, runs the complete stock
   turn, and retains the resulting controller state and successful action log;
2. replays every successful stock action in order except the final `EndTurn`;
3. only when `turn % 6 == focal_seat % 6`, the game is still live, and the
   focal seat still owns the turn, enumerates its captive Spies held by living
   non-minor, non-barbarian civilizations that it has met and is not at war
   with;
4. computes the frozen midpoint above, rounds it **up** to whole lump Gold,
   and keeps only captives affordable above the stock cash reserve
   `min(30% of treasury, 40 Gold)` after every stock action has already spent;
5. chooses highest Spy level, then most promotions, then lowest Spy id, and
   applies one ordinary `Trade` requesting that Spy for the rounded lump Gold;
   and
6. defers `EndTurn` until the recovery attempt finishes, then resumes ordinary
   play with the exact stock-produced controller state.

The treatment never creates Gold, waives a price, uses Gold per turn, releases
a wartime or city-state captive, changes capture odds, increases Spy capacity,
changes missions, adds movement, changes information, or displaces a stock
trade or any later stock action. The captor receives the real Gold. One extra
trade on a six-turn cadence is legal under the engine's action economy; the
stock throttle is an AI policy/computation limit, not a game action limit.

Running the recovery only after stock replay is deliberate. Inserting it
before stock spending could invalidate purchases and would test a different
budget allocation. Replacing stock's chosen trade would conflate recovery with
the resource, Great Work, or Open Borders deal displaced. This treatment asks
whether using the still-available legal action and remaining treasury is worth
it. A pass permits only the same end-of-turn policy in a separate gameplay PR.

## Exact null and focused contract

Before treatment data, a four-map null at seed `10019999` uses the same wrapper
with recovery disabled. For both focal seats on all four maps, ordinary stock
and null replay must have identical focal results, census, and serialized
terminal `Game`: eight exact cells or **STOP**.

Focused tests must prove:

- every level/promotion midpoint ends in 0.5 and the rounded payment is exactly
  one-half Gold above it;
- the stock reserve blocks a payment that would cross it;
- wartime, dead, minor, barbarian, unmet, self-held, and unaffordable captives
  are ineligible;
- ordering is level, promotions, then stable Spy id;
- a legal recovery transfers exactly the rounded Gold and releases the Spy;
- at most one recovery occurs and only on the frozen cadence;
- all stock actions and controller state survive null replay exactly; and
- the screen and holdout gates reject every individual harm or missing-
  mechanism condition.

## Deployment population and endpoints

The evaluator targets the unattended production rollover population with the
same deterministic 126-profile cycle used by the registered Spaceport,
horizon, recon, and repair studies. For zero-based map offset `i`:

- players are `[4, 6, 8, 10, 5, 7, 9][i mod 7]`;
- scripts are `[land_only, water_world, continents, true_start_earth, lakes,
  inland_sea, pangaea, small_continents, islands][i mod 9]`; and
- topology is Flat for even `i`, Planet for odd `i`.

`MapSize::for_players` supplies dimensions and city-state counts. Civilizations
are randomized; Poles, Online speed, and Science/Culture/Domination victories
are fixed. `Game.max_turns` stays 250 while the unchanged stateful agents are
observed externally through turn 320. The runner must assert that the
policy-visible horizon never changes.

Each map is played four times: focal seat 0 and the final major seat, each as
ordinary stock and treatment. Every other major is stock `AdvancedAi`; minors
retain their stock paths. The two focal seats are aggregated inside their map,
and the map is the only inference unit.

The evaluator reports:

- peace-held and affordable opportunities, successful recoveries, recovered
  levels/promotions, Gold paid, seat-game coverage, and any failed application;
- subsequent `AssignSpy`, `PromoteSpy`, and `SpyMission` actions by recovered
  Spy ids, including seat-game coverage;
- total trade actions, terminal captive and non-captive Spies, active missions,
  Spy levels/promotions, and terminal Gold;
- wins and victory types, finish turn, terminal score, cities, technologies,
  civics, Science-project progress, lifetime Culture and Tourism, and military
  power; and
- paired map win score, paired terminal-score share, complete
  favorable/neutral/adverse map directions, and exact two-sided sign tests.

## Fixed development screen

After the exact null, the one allowed screen is 18 maps / 72 games:

```text
captive_spy_recovery_eval --deployment-mix --maps 18 --turns 250 \
  --observe-through 320 --speed online --poles poles --randomize-civs \
  --victories science,culture,domination --seed 10020000 --jobs 6
```

It advances only if every term holds:

1. treatment completes at least 18 recoveries across at least 12 of 36 focal
   seat-games, with zero failed recovery applications;
2. recovered Spies subsequently issue at least 18 Spy actions across at least
   eight treatment seat-games;
3. treatment terminal captives are at most 75% of control and treatment has at
   least as many terminal non-captive Spies;
4. terminal-score favorable map directions outnumber adverse directions, the
   exact two-sided sign-test p-value is at most 0.20, paired terminal-score
   share is at least 50%, and mean treatment score is not lower;
5. paired map win score is at least 50% and treatment has no fewer total focal
   wins; and
6. treatment has no fewer Science, Culture, or Domination wins than control.

Any failed term means **STOP**: retain the stock policy, record the negative
result, do not tune the price, cadence, target ordering, gate, sample size, or
seed, and do not inspect the holdout.

## Disjoint holdout

A complete screen pass earns one unchanged 63-map holdout at seed `10021000`
(252 games). It must retain the full mechanism gate, terminal captives at most
75% of control, at least as many non-captive Spies, paired terminal-score share
at least 50.5%, a positive mean score difference, more favorable than adverse
score maps with exact two-sided sign-test `p < 0.05`, paired map win score at
least 50%, and no loss of total or per-type wins. Only that conjunction permits
a separate gameplay-integration PR; this evaluator cannot ship the policy.

Undefined ratios pass only when both corresponding counts are zero. No pooled
rescue, seed retry, sample extension, subgroup promotion, or post-result
treatment change is allowed.

## Resource and integration order

The exact null, screen, and any earned holdout use at most six jobs and run
alone in the shared simulator slot. They are queued behind every older active
registered job, including #561, #567, #570, #574, #579, #584, #589, #591,
#592, #593, and #597. Studies that stop or land release their place; this task
never jumps a still-live older batch.

The implementation, latest-main merge, focused checks, and full locked CI suite
must precede the exact null. Exact commands, source commit, wall time, and all
results will be recorded before this evaluator leaves draft.

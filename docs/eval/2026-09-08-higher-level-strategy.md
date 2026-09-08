# Five development weaknesses holding back the higher difficulty ladder

## What the evidence supports

The current task's `docs/civ6_ladder.json` records 873 attempts and a King
Science victory at turn 234: `civvis-20260901T033000Z-cont8`, revision
`fcbcd573d9dc159d9ef72432d483c8d6d44958d1`. The host's
`~/civvis-civ6-runs/control/ladder.json`, read 2026-09-08, has 818 attempt
records and agrees on that win. Neither records an Emperor win. Attempt
counts include restarts and continuations; they are not an independent-game
win-rate denominator. The separate initial `~/CIVVIS` checkout had an older
650-attempt snapshot; it is not the source for the current rung assessment.

The available recent run directories are a much smaller sample. Running
`python3 tools/civ6_run_report.py --aggregate ~/civvis-civ6-runs/control`
reported six completed runs, zero wins, and nine without a terminal event.
All three reaching turn 200 built a Spaceport and launched, but none led the
science field; two launched 1/4 projects and one 2/4. The two completed runs
with turn-60 observations already held four and five cities. Expansion is
necessary groundwork, not a sufficient victory strategy. Recent Emperor
terminal ledger rows include `civvis-20260908T191919Z-cont4` (turn 213,
549 score against 1,797) and `civvis-20260908T204713Z-cont2` (turn 177,
540 against 1,381). Score diagnoses the gap; the King Science win despite
lower score is itself a warning not to optimize score in place of victory.

The following is a prioritized engineering assessment, not a statistically
identified ranking of five causal effects. Older King measurements explain
specific code paths; they do not prove the same frequencies under today's
Emperor genome. The new genes are hypotheses and ship as independently
screenable opt-ins, with no change to the measured deployment selection.

## 1. Expansion is still tied too closely to the capital's availability

`advanced/expansion_schedule.rs` records an older 218-run corpus with all nine
wins inside the four-to-six-city turn-60 band. It also records a live capital
building Archers, Campus and Walls while a second Settler was allowed.
`rapid-city-expansion-2` repairs the excessive first version but reserves only
an idle capital. A safe secondary city can therefore have an empty queue while
the empire falls behind the opening schedule.

**`expansion-best-idle-city`** reserves the fastest eligible idle city's
Settler while founded cities trail the existing five-city pace. It requires
zero active or queued Settlers, the ordinary settlement target and production
admission, and a site passing the existing walker-aware gate. It ends at the
opening band. This tests launch timing, not a larger empire target or a wider
pipeline. Risk: taking secondary-city production can delay that city's own
development; the narrow window and one-walker limit bound that cost.

## 2. Income repair arrives after the empire has already lost useful turns

`advanced/yield_floors.rs` documents fifteen older King runs with no Markets or
Lighthouses, one trade slot, and six bankrupt for 31–73 turns. The existing
gold floor adds a price bonus, while the first-route reserve fills a slot that
already exists. Neither guarantees that a completed Hub gets its capacity
building before the income crisis.

**`trade-building-before-bankruptcy`** reserves one legal Market-family or
Lighthouse-family building when net income is below two Gold per city and all
existing route capacity is occupied or committed. It does not build another
district, reserve every Trader, or buy the second copy of a city's shared
Market/Lighthouse capacity tier. The normal recovery governor retains a severe
crisis. Risk: capacity still needs a subsequent Trader to realize its value.

## 3. Culture catch-up is a bonus competing with the same bids that caused the deficit

The same King evidence records culture at 16–62 against the best rival's
71–133, no Amphitheatres, and governments stuck at Classical Republic in all
fifteen turn-100 observations. The existing `culture-floor` lifts the slot
veto and adds value, but a legal useful building can still lose its city queue.
A Science plan also needs culture for its civic and government economy.

**`culture-building-catchup`** reserves one legal culture-yielding building
when output trails 70% of the strongest contacted major's observed output.
It selects the best printed culture per completion turn across safe idle
cities, services at most one outstanding culture-building debt, and becomes
inert at the floor. It uses observed host yield adjustments as the existing
floor does. Risk: printed yields omit some multipliers, and matching a
culture specialist may divert too much production from the winning lane.

## 4. Science completion starts with too little science behind it

The recent three late games demonstrate that reaching a Spaceport is no
longer the only missing link. `advanced/science_scaling.rs` records the older
19-run building funnel: 50% Campus, 39% Library, 20% University, 3% Research
Lab. The current named Science reservation already services this chain after
specialization; the opening and adaptive governors still need an independent
catch-up response before that point.

**`research-building-catchup`** reserves one legal science-yielding Campus
building while research output trails 70% of the strongest contacted major.
It completes installed capacity without buying another Campus and ranks by
printed science per completion turn. Victory-project reservations run first.
Risk: a late research investment may not accelerate the final technology
soon enough; the completion-and-use window filters the latest investments.

## 5. Builder supply treats exhausted capacity as an ordinary low-value bid

`production_value` prices a Builder at roughly 260 plus a small headcount
shortfall, while capital expansion and district chains command much larger
raw bids. The first-Builder rewrite is deliberately an opening receipt: its
predecessor repeatedly interpreted zero Builders as a first Builder and
pre-empted queues. That mistake does not mean replacement is never needed;
it means replacement needs its own decision and a concrete job.

**`builder-workforce-recovery`** reserves one Builder only with two or more
cities, no active or queued Builder, expansion on pace or a Settler committed,
and legal improvement work on an unimproved tile owned by the launch city.
It cannot pre-empt a queue. This is primarily a code-supported hypothesis;
there is no fresh measured Emperor frequency for this shortfall. Risk:
replacement can still consume too much production if Builders die or their
jobs prove unsafe. Local launch safety is checked; future job safety remains
the existing Builder controller's responsibility.

## Shared implementation and evaluation boundary

The pass runs after the opening book and the existing victory reservations,
before either generic governor. It issues at most one order per call, in debt
priority order (trade, expansion, culture, research, Builder), selecting across
cities rather than accepting city iteration order. A queued answer suppresses
another answer of that type. Active queues, recently attacked cities, local
barbarian alarms, named threatened cities, military requisitions, active war
plans and Recovery posture are excluded. A finite clock must leave twenty
standard turns after completion to use the asset. All five disabled is an
immediate no-op. Each successful order names its gene in the decision journal.

Validation and screen results are recorded below after execution. Small
single-gene screens establish measurement reach only. Future regular screens
must judge the genes under the existing deployment rule; a higher difficulty
win improvement requires actual live evidence. Equal native difficulty bonuses
for all seats do not reproduce the live player's asymmetric handicap.

## Validation

- `cargo test --profile ci --locked`: 3,197 passed, zero failed, 50 ignored
  (including four documentation examples). Repeated successfully after merging
  the current tournament ledger from `origin/main`.
- Seven focused tests cover registration, each gene's legal production order,
  queued answers, income recovery, opening expiry, threats, Recovery, active
  queues and the finite-clock cutoff.
- `cargo fmt --check` and `git diff --check origin/main...` passed.
- `cargo clippy --profile ci --locked --lib --message-format=json`: passed;
  no diagnostics in the changed AI files.
- `python3 tools/genes.py check` and
  `python3 tools/eval_manifest.py --check`: current after regeneration.
- Five independent single-gene screens were pre-sized at 12 games / 72 seats
  each, two workers each, Emperor majors, ordinary standard-screen settings
  otherwise. Seed windows are 108090000–108090011 (Builder),
  108091000–108091011 (culture), 108092000–108092011 (expansion),
  108093000–108093011 (research), 108094000–108094011 (trade).
  Each binary reports clean source commit
  `d42d2cd63127ea3b3b3ee0f358456c591c804066`. The later merge changes the
  tournament-selected background, so these are explicitly reach probes of
  that recorded source, not strength evidence for the refreshed deployment.

## Completed reach probes

All five fixed-size probes completed: 60 games / 360 player seats in total.
The following are exploratory on-minus-off estimates in percentage points;
SE is clustered by game. These small samples do not authorize promotion.

| Gene | On / off seats | Win delta ± SE (pp) | Analyzer reading |
|---|---:|---:|---|
| `builder-workforce-recovery` | 9 / 63 | -6.35 ± 12.70 | ~ |
| `culture-building-catchup` | 11 / 61 | +1.79 ± 13.88 | ~ |
| `expansion-best-idle-city` | 17 / 55 | -14.12 ± 5.97 | hurts * (thin) |
| `research-building-catchup` | 21 / 51 | +3.36 ± 10.83 | ~ |
| `trade-building-before-bankruptcy` | 20 / 52 | -2.31 ± 9.67 | ~ |

The expansion arm carries a negative early signal, not a successful fix.
It stays off and should receive particular scrutiny before any promotion;
the other probes are inconclusive. The five mechanism tests prove concrete
order changes, and the committed summaries satisfy the repository reach
gate. Their intervals and small on-seat counts remain visible. No source
was entered into the deployment ledger and no gene was promoted.

Artifacts: `docs/gene_screens/fires/2026-09-08-higher-level-*.json`. Each
retains the analyzer's reproducibility block and clean build stamp. Raw rows
remain in `/tmp/civvis-higher-level-<tag>.jsonl` on this host and are
reproducible using the recorded source, options and seed windows.

Final tooling validation: all 194 gene-tool tests and all 14 registry-append
tests passed. The reach ratchet reports 275 registered genes with evidence,
zero waived and zero unproven.

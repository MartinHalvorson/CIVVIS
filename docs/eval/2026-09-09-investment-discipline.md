# Higher difficulty: five investment mistakes to test next

This is a second assessment following #3239, not a claim that its five new
reservations already improved the live player. The first expansion probe was
negative (-14.12 ± 5.97 percentage points, clustered SE); the other four were
inconclusive. All five remain opt-ins. Preserve their identities and add five
version-two hypotheses so the regular evaluator can compare the families.

## Evidence and scope

On 2026-09-08, `python3 tools/civ6_run_report.py --aggregate
/Users/martbot-mbp-m5-max-128/civvis-civ6-runs/control` reported 23 retained run
directories, seven terminal losses and sixteen without terminal events. These
are retained segments, not an independent-game win-rate denominator. Four
reached turn 200: all four had a Spaceport and a launch, none led the field in
science at the end, and their completed launch counts were 1, 1, 2 and 3 of 4.
The two terminal runs with turn-60 city observations had four and five cities.
Missing early observations in continuations do not establish an early loss.

We are reaching the race but still falling behind. More reservations can make
that worse if they spend production at the wrong time or for too little value.
These are the five highest-priority admission weaknesses in the earlier
investment pass, based on source inspection and its exploratory screen. They
are not a statistically established ranking of every cause of Emperor losses.
Military survival, rival victory denial and diplomatic execution also matter;
this change isolates the investment decisions rather than mixing those axes.

## 1. Starting a Settler is counted as fixing an opening deadline

The first version checks the turn when production starts, not when the Settler
can leave. It can therefore reserve one on turn 59 of a 60-turn opening band.
It also takes a small secondary city's population. The negative first probe
makes this a priority hypothesis, but does not identify these mechanisms as its
cause.

**`expansion-best-idle-city-2`** requires population at least four and enough
opening time for construction plus ten standard turns of travel/founding
allowance. It retains the real-site gate, one-walker cap and existing empire
size target. The allowance is a conservative heuristic, not a pathfinding ETA;
long or unsafe journeys can still exceed it. It may also reject a useful late
city, which is why it is a separate experiment.

## 2. Trade capacity is purchased before its whole chain is affordable

The first reservation sees a Market/Lighthouse and low income. A new route
also needs a Trader. A treasury can empty during that second construction, or
the building can finish with too little clock left for the route to work.

**`trade-building-before-bankruptcy-2`** estimates sequential building and
Trader completion in the selected city, includes the building's maintenance
once completed, and requires nonnegative projected treasury at both points.
The complete chain must leave twenty standard turns on a finite clock. It
credits no future route income. This is admission, not a promise that the
ordinary controller will construct the Trader next. It excludes uncertain new
building yields from its budget, so the estimate can be conservative.

## 3. Culture catch-up pursues the specialist instead of the field

The first version uses 70% of the strongest contacted rival's culture as its
floor. One tourism specialist can raise that target far beyond what a Science
empire needs and keep culture ahead of research in the reservation ordering.

**`culture-building-catchup-2`** uses 70% of the median contacted living major's
culture. All observations retain host yield adjustments. With one rival this
is unchanged; with an even number it uses the mean of the two central values.
It still repairs a broad deficit and ignores uncontacted players. The median
is a spending control, not a cultural-victory defence; it can underinvest when
one specialist is the actual immediate threat.

## 4. Catch-up research can occupy the launch city's idle window

Existing launch reservations act first, but a gap between unlocked projects
leaves an idle Spaceport city eligible for the generic research catch-up pass.
The next project may unlock while its Library/University is being built.

**`research-building-catchup-2`** excludes cities with a standing Spaceport
from this reservation. Other cities can still complete their Campus chains.
It leaves the launch city to the existing science and production controller;
it does not idle the city or ban its ordinary research-building choice. This
can cost science when the pad is in the best research city or the next launch
is far away. The new test proves the reservation changes, not that such a ban
is universally better.

## 5. A whole replacement Builder can be reserved for one tile

The first version requires one legal local job. That proves the unit has
something to do, but does not justify spending a city's queue on its full
charge capacity. A single remaining farm can trigger repeated replacement.

**`builder-workforce-recovery-2`** requires three distinct owned tiles with
legal Builder improvements. Three alternative improvements on one tile count
as one job. Existing or queued Builders, expansion commitments, local threats
and active queues retain their guards. The test does not guarantee jobs remain
safe or are high-value worked tiles; these remain limitations. It also defers
one exceptionally valuable luxury job to the existing first-Builder and normal
production logic.

## Implementation and evaluation

Each version is a standalone opt-in. Enabling either family member clears the
other. Version one selected alone preserves its eligibility and scoring. The
new flags append to the registry without changing any existing positional
index. The existing common pass still reaches both governors, issues at most
one order, protects military/recovery/active queues and runs after the opening
and victory reservations. Decision journal entries identify the selected version.

The validation section below will record executed checks and fixed-size reach
probes. These are hypotheses about better spending, not claims of improved win
rates. No deployment gene is promoted by this change. These probes use `--difficulty emperor --rivals firaxis-mix --handicap rivals
--rival-chairs 3 --p-on 0.5`: three measured seats without the Emperor bonuses
against three boosted rival seats. This exercises an asymmetric handicap,
though it is not the live one-player/five-rival shape or the Firaxis AI itself. A positive small probe must not be used as proof
of a higher-level live improvement.


Before execution, fix six games per gene (30 games total), two workers per
probe, seed windows 109090000–109090005 (Builder), 109091000–109091005
(culture), 109092000–109092005 (expansion), 109093000–109093005 (research),
and 109094000–109094005 (trade). Each uses only its one new gene in `--genes`;
other genes retain the evaluator baseline. Each game contributes three measured
seats and three excluded rival seats. This is a reach screen, not a promotion
trial, and the sample size will not be increased to obtain a favorable sign.

## Completed validation

- Full Rust suite after merging current main: **3,221 passed, zero failed,
  50 ignored**. The pre-merge suite also passed (3,220 tests).
- Thirteen focused tests pass. Six existing scenarios now exercise both
  versions; additional tests cover each new rejection and family toggling.
- Changed-file clippy including tests, formatting, and diff whitespace checks
  pass. All 307 pre-existing registry indices remain in order; five appended.
- Five fixed-size probes completed: 30 games, 90 measured seats and 90 excluded
  rival seats. Every summary records clean source
  `1df55a1365691024ed7135887a79adaa03d66e03`. The later merge adds upstream
  movement calculation caching and host popup handling; the source stamp
  identifies the actual screened revision.

The initial probe startup was stopped before any game completed because its
source stamp reflected a pending test-only lint edit. After committing that
edit, all five restarted with the same reserved seeds and clean stamps. The
completed artifacts below, not that aborted startup, are the evidence.

| Gene | On / off seats | Win delta ± clustered SE (pp) | Reading |
|---|---:|---:|---|
| `builder-workforce-recovery-2` | 13 / 5 | -4.62 ± 20.48 | Inconclusive |
| `culture-building-catchup-2` | 12 / 6 | +8.33 ± 20.18 | Inconclusive |
| `expansion-best-idle-city-2` | 8 / 10 | -17.50 ± 19.20 | Inconclusive |
| `research-building-catchup-2` | 7 / 11 | +5.19 ± 16.56 | Inconclusive |
| `trade-building-before-bankruptcy-2` | 8 / 10 | -17.50 ± 22.98 | Inconclusive |

All five readings are inconclusive. The expansion point estimate is negative
again; this is not evidence that the new guards rescued its strength. The two
versions used different source revisions, handicap settings and seeds, so
subtracting their point estimates is not a version-two treatment effect. No
source is added to the deployment ledger and no new version is enabled by
default. Family inventory entries are generated with no selected head.

Artifacts are the complete analyzer outputs in
`docs/gene_screens/fires/2026-09-09-investment-*.json`; raw rows remain under
`/tmp/civvis-investment-<tag>.jsonl` on this host.

Future selection should use the regular randomized-genome screens and a
separate held-out set with five handicapped rivals. Track founding turns,
insolvency turns, civic progress, launch timing and Builder work alongside the
primary win result to distinguish a better decision from a lucky draw. Any
higher-level live improvement still needs retained Civilization VI outcomes.

Tool validation: all 194 gene-tool tests and all 14 registry-append tests
passed. `gene_fires.py --max 0` reports 280/280 genes evidenced, no waivers
and no missing evidence. `genes.py check` and `eval_manifest.py --check` pass.

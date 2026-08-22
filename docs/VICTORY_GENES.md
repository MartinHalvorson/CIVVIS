# A gene for every victory lane

The controller is treated as a genome (`docs/GENE_SCREEN.md`): every behaviour
is a gene, on or off, priced regularly by a randomised screen. This file asks
a question that ranking never asks — **which victory is each gene for?** — and
records what the answer says about the lanes the agent can and cannot win in.

The operator's brief: *when it makes sense to go for a particular victory, the
optimal genes for that victory should be available.* That is two claims. The
first is coverage: every lane needs genes of its own. The second is reach: the
lane the empire is racing has to arrive at the decisions that serve it.

## 1. Which lanes land at all

`victory_eval --games 8 --players 6 --width 74 --height 46 --turns 250
--speed online --start-seed 51000000`, one run a lane, at the deployment
shape (`docs/EVAL.md`). Every seat carries the same target, so the table is
about reachability, not about beating a field.

| lane | lands | wonders built, all majors | what the winner had at the end |
|---|---:|---:|---|
| score | **8/8** | 132 | the tally at turn 250 |
| culture | **8/8** | 37 | 66–93 visiting tourists against an 63–91 target; turns 165–230 |
| religious | 5/8 | 4 | — |
| diplomatic | **1/8** | **0** | dvp 20, 18, 16, 16, 13, 12, 10, 8 — **needs 20** |
| science | **0/8** | **0** | 50–57 techs of 77, **2 launch projects of 4**, 0 light-years travelled |
| domination | **0/8** | 0 | every game to the clock, every major alive |

Three readings, in descending order of how much they should change what gets
built next:

1. **Diplomacy is a near miss, not a gap.** Seven of eight games end with the
   leading seat between 8 and 18 of the 20 points it needed. A lane that
   finishes 2 points short is a lane a handful more points wins; that is a
   very different repair from a lane that is not in the race.
2. **Science is out of reach at this clock**, and by a wide margin: two of
   four launch projects, twenty-odd techs short, and the fifty light-years
   never started. `score_horizon` correctly stops paying for it. Whether the
   *economy* can be made to reach it is a different question, under study in
   #2264; nothing in this file claims to move it.
3. **Zero wonders in the science and diplomatic games.** For Diplomacy that is
   7 of the 20 points, and it is **not** a construction gap:
   `docs/eval/2026-08-18-the-wonder-a-city-cannot-start-pays-its-prerequisites.md`
   established that Mahabodhi is religion-gated, the Statue of Liberty
   civic-gated and the Potala Palace priced out by the cost floor on purpose.
   More production credit is the wrong lever there. The other thirteen points
   are.

## 2. Which genes each lane already has

Classification of the **101** rows that stood in `LIVE_TREATMENTS`,
`PRODUCTION_TREATMENTS` and `PRODUCTION_OPT_INS` before the six below, by the
victory they serve. It is a judgement, not a
field in the table — a gene that helps the empire generally helps every lane —
so the useful column is the *specific* one: genes that only exist because of
one victory condition.

| lane | genes specific to it | examples |
|---|---:|---|
| domination | ~31 | `war-economy`, `siege-tracks-wall`, `siege-commitment`, `blind-objective-strength`, `war-patience`, `relief-targets-the-siege`, `contact-posture` |
| score / expansion | ~23 | `wide-map-capacity`, `parallel-settlers`, `settle-sooner`, `land-grab`, `era-paced-expansion`, `stranded-settler-discount` |
| religion | 10 | `founder-temple`, `inquisition-on-threat`, `apostle-promotion-by-role`, `theology-for-founders`, `idle-faith-patronage`, `religion-sues-peace` |
| culture | 4 | `tally-culture`, `culture-building-debt`, `culture-coverage`, `slot-kind-tiebreak` |
| **diplomacy** | **3** | `bank-envoys` *(host-only)*, `stock-denial-lead-time`, `projected-stock-denial` |
| **science** | **1** | `one-launch-pad` |

The two lanes with the fewest genes of their own are the two that never land.
Domination has thirty-one and never lands either — so gene count is plainly
not sufficient — but the converse still holds: **the lanes with nothing built
for them are not lanes the agent can be said to have tried.** Three of the
Diplomacy genes are the Congress-denial family, which exists to stop a *rival*
winning; only `bank-envoys` is about winning it ourselves, and it is host-only
(`FIRAXIS_ONLY_TREATMENTS`), so it is inert on a native board.

## 3. The reach problem, measured

`assess` produces one `StrategicPlan::strategy`, and `take_turn_inner` hands
it to every lane-shaped subsystem of the turn: the World Congress ballot,
Great Person patronage, the policy deck, the Culture Faith pass, the space
race. Two of those are `if plan.strategy == <lane>` gates, so the lane's whole
machinery is off whenever the plan says something else.

The plan says something else more often than the design suggests. `assess`
returns `Expansion` for an assigned lane while the empire is short of cities
("the assigned lane can still afford to expand first"), and for an adaptive
seat whenever it is short of cities with land still open. Measured at the
deployment shape (six games a lane, seeds 52000000.. and 53000000..;
per-seat-turn shares of `AdvancedAi::strategy_census`):

| seat | Expansion | Conquest | Recovery | on a victory lane |
|---|---:|---:|---:|---:|
| `--target culture` | 20.6% | 0.0% | 0.0% | 79.4% |
| `--target diplomatic` | 19.7% | 0.0% | 0.0% | 80.3% |
| `--target science` | 18.9% | 0.0% | 0.0% | 81.1% |
| `--target religious` | 5.2% | 0.0% | 0.0% | 94.8% |
| **adaptive (no target)** | 15.0% | 22.1% | 10.9% | **52.0%** |

A targeted seat spends about a fifth of the game with no victory content in
its plan, and those are the **opening** turns, which the live ladder measures
as the part that decides the game: `tools/civ6_opening_census.py` records
`cities_at_60` as a gate — **0 wins in 22 recorded games below four cities** —
and two thirds of the losses already behind by turn 75. The adaptive seat —
which is what production ships and what every seat in a native `gene_screen`
game is — spends **48% of its seat-turns** under Expansion, Conquest or
Recovery.

`victory_focus` already answers "which victory is this empire racing" for both
kinds of seat: the assigned target when there is one, the best-progress lane
when there is not. `take_turn_inner` already uses exactly that resolution for
city dispositions, and `strategic_wonder_value` already uses the target-first
half of it, with the reason stated in its own comment: *a targeted agent whose
plan has swung to Conquest under pressure is still playing for the target.*
The genes below extend it, one decider at a time.

## 4. The genes

Seven ship **off**, live in `src/ai/advanced/victory_lane.rs`, and are rows
in `PRODUCTION_OPT_INS` — so `gene_screen` discovers them without being told,
and `victory_eval --with <tag>` can seat one by name in the lane it exists
for. Each is one sentence and one call site.

| gene | lane | what it changes |
|---|---|---|
| `lane-congress-ballot` | diplomacy | the World Congress ballot is **scored** for the raced lane — which outcome and target this seat names |
| `lane-congress-favor` | diplomacy | the Favor **stake** behind that ballot. Split from the row above by §8's reading; naming the right outcome is free, buying it is not |
| `lane-great-people` | all four | Great Person patronage **and the Great Person points a project earns** rank classes by the raced lane. The one gene here that overrides a war plan — see §7 for the fires-check that chose that scope |
| `lane-policy-deck` | all four | the policy cards are chosen for the raced lane |
| `lane-culture-spending` | culture | the Naturalist and the touring Rock Bands, and the Faith reserve that keeps them affordable, follow the race rather than the plan |
| `lane-space-race` | science | a Science **racer** is treated as a Science **target** by the space race: three pads for the parallel laser race instead of one, a launch project may claim a city with something else queued, and the pass opens at all |
| `competition-victory-points` | diplomacy | a scored competition's first place is priced by the Diplomatic Victory Points it pays |

And three that are **not new behaviour** — they already existed, off in
production and reachable only as named `elo.rs` arms, so `gene_screen` could
not see them and the genome instrument has never priced any of them (§8):

| gene | lane | what it already did |
|---|---|---|
| `congress-banks-decided` | diplomacy | answer an already-decided resolution with the one free vote on its settled winner. Its own field doc carries the measurement and it has sat unused: **26 of 192 ballot decisions already settled, ~1.4 free points a seat a game** against the twenty |
| `congress-counter-votes` | diplomacy | back a ballot aimed at the empire closest to winning with everything the treasury can spare — a losing vote is refunded in full, so a failed opposition costs no Favor |
| `envoy-infrastructure` | diplomacy | price the Consulate and Chancery by the envoys their influence stream will actually produce before the turn limit |

### Two scopes, and the fires-check that forced them apart

The first draft scoped all six to an `Expansion` plan. A paired fires-check —
`victory_eval` on the same four seeds with and without each gene, seed
54000000.. — said that was right for three of them and inert for three:

| gene | games of 4 that diverged, `Expansion`-scoped |
|---|---:|
| `lane-policy-deck` | **4/4** culture, 2/4 diplomatic |
| `lane-congress-ballot` | **3/4** diplomatic |
| `lane-culture-spending` | 0/4 |
| `lane-space-race` | 0/4 |
| `competition-victory-points` | 0/4 |
| `lane-great-people` | 0/4 |

The three zeros are structural, not noise. A targeted seat's expansion window
shuts at `standard_duration(175)` — turn ~87 at Online — and none of those
three can act that early: a National Park needs `conservation`, a Rock Band
`cold_war`, a Spaceport `rocketry`, and every scored competition is gated on
world era 5 to 8. Restricted to the settling turns they were **strictly
inert**, which is exactly the trap #2264's own body warns about: *check a
gene's branch is reachable under the baseline the screen runs before spending
games on it.*

So the scope now follows the kind of decider, and each kind is stated:

1. **A choice among options the empire is making anyway** — the ballot, the
   patronage ranking, the policy deck. `AdvancedAi::raced_lane` answers `None`
   for every plan that is not `Expansion`: a `Conquest` or `Recovery` plan is
   a *deliberate* refusal of the economic lanes, and overriding it is a much
   riskier claim than filling in a posture that has no victory content yet.
   The war case is otherwise a separate gene and a separate screen — the one
   exception is `lane-great-people`, which §7 explains and which is where that
   claim is actually tested. `raced_lane` also answers `None` when the raced
   lane is Conquest or the score tally, because there is no economic lane to
   substitute.
2. **A whole pass switched off unless the plan names the lane** —
   `culture_spending`, `space_race_production`. What is missing here is not
   the settling turns: an **adaptive** seat holds the Culture plan for 5.0% of
   its turns and the Science plan for 25.3%, so those passes are all but
   unreachable without an assigned target. These follow the race under any
   posture short of `Recovery`.
3. **Pricing a currency** — `competition-victory-points` — needs no posture
   test at all. It asks only whether this empire is racing Diplomacy.

### `competition-victory-points`, and the regime it is live in

Thirteen of the twenty points a diplomatic victory needs come from the World
Congress and its scored competitions. `Game::NATIVE_COMPETITIONS` pays first
place **2** points for the Climate Accords, Send Aid and Send Military Aid,
and **1** for the World Games, the World's Fair and the International Space
Station. `host_competition_score_value` prices the competition's own *score*,
its deadline and the lead swing — and prices the victory points **at zero**,
at the same rate for a Conquest seat as for a Diplomacy one.

⚠ **This gene is inert in a default native game, and that is a property of
the rules rather than of the gene.** `Game::native_competitions` ships **off**
— its own doc says why: turning it on changes what every participant faces and
moves the frozen rating anchor, so it is an arm to be priced
(`--native-competitions`, `docs/ELO_REPINS.md`), not a silent rules change.
With it off, `open_native_competition` returns immediately and no competition
is ever seated, so **`gene_screen` cannot price this gene** — it would read
exactly +0.0, which is what an unreachable branch reads. Its two live regimes
are the `--native-competitions` arm and the **live Civilization VI bridge**,
whose mirror supplies real Gathering Storm competitions in
`Game::host_competitions` — and the live ladder is where the diplomatic lane
matters most (`docs/CIV6_LADDER.md`: index 6 is the commonest terminal event
in 199 recorded games). `an_open_competition_pays_its_points_to_a_diplomacy_racer`
seats one the way both regimes do, so the branch is proved rather than
assumed.

This is the same absence `strategic_wonder_value` closed for wonders in #2061,
in the other half of the same lane, and it is closed the same way and with the
same denominator: one point is
`STRATEGIC_WONDER_VICTORY_VALUE / DIPLOMATIC_VICTORY_POINTS`, 700 in the units
`production_value` ranks in — the number calibrated so that the Statue of
Liberty's four points read ~2 800 and outbid a Settler, and one point reads
700 and outbids nothing. It is paid only where the points are collectable: the
raced lane is Diplomacy, the competition is open, and this completion would
put the seat at or in front of the current leader.

Per `docs/GENOME.md`'s standing prior — actuation repairs pay, valuation tunes
do not — five of these six are actuation (a decider that could not see the
lane now sees it) and the sixth is absence-class (a currency that was not
priced at all), which is the class #2061 and #2082 were licensed under.

## 5. What is deliberately not here

- **Domination.** `docs/eval` records the pin as converting and costing −319
  Elo; the lane is closed and this file does not reopen it.
- **The science economy.** Science lands 0/8 because the empire reaches 50–57
  of 77 techs, not because the Spaceport pass is gated wrong. `lane-space-race`
  removes one gate; it does not claim to move the lane. #2264 is the economy
  half.
- **Diplomatic wonders.** Named as a construction gap by the lane table and
  established as a religion/civic gating one by the 2026-08-18 census. More
  production credit is the wrong lever and is not applied.
- **A war plan that outlives the lane.** The larger half of §3 — 33% of an
  adaptive seat's turns under Conquest or Recovery — is left for its own gene.

## 6. How to price them

Both instruments, because they answer different questions:

```sh
# 1. The native screen: every seat adaptive, every gene on in exactly one arm
#    of every pair, all six priced from one batch beside the rest of the genome.
target/ci/gene_screen --genes lane-congress-ballot,lane-great-people,\
lane-policy-deck,lane-culture-spending,lane-space-race,competition-victory-points \
  --pairs 3000 --all-seats --randomize-civs --players 6 --jobs 8 --out lanes.jsonl

# 2. The lane instrument: does the lane the gene is for land more often?
target/ci/victory_eval --target diplomatic --with lane-congress-ballot \
  --with competition-victory-points --games 24 --players 6 --width 74 --height 46 \
  --turns 250 --speed online --start-seed 51000000
```

The screen ranks and directs; `ai_eval` remains the ship decision for any
promotion (`docs/GENE_SCREEN.md`). A `*` here is a candidate for a dedicated
arm, never a promotion.

⚠ **The all-lanes screen cannot price the Diplomacy genes, and its own header
says why.** Its census of how the games ended reads *religious 58%, score 25%,
culture 17%* — **no diplomatic victory at all**, and none expected: the native
regime is the one `docs/GENE_SCREEN.md` records as decided two thirds of the
time by conversion. A gene whose whole purpose is to win the Congress can only
show up there as score share. That is what `--victories` is for, and it is the
same correction the war genes needed (`--victories domination,score` gave "the
31 war and siege genes a game that does not end by conversion at turn 149").
So the lane genes are priced twice: once in the native regime, for the average
marginal effect on the games production actually plays, and once in
`--victories diplomatic,score` and `--victories culture,score`, for the
question the gene was written to answer.

`lane-space-race` gets a third: `--turns 600`, because its lane does not
finish in 250 (§1, §7).

## 7. The fires-check ledger

Before any of these is worth screen games, it has to change a game at all.
Two paired instruments, both on the same seeds with and without one gene:
`victory_eval` (every seat carries the lane's target) and `gene_screen
--genes <one>` (every seat adaptive, the regime production ships).

| gene | targeted, 4 games | native adaptive, seat-pairs | read |
|---|---:|---:|---|
| `lane-policy-deck` | **4/4** culture, 2/4 diplomatic | — | fires |
| `lane-congress-ballot` | **3/4** diplomatic | — | fires |
| `lane-culture-spending` | 0/4 | **5/36** at 250 turns | fires, and only where it was supposed to |
| `lane-great-people` | 0/4 | 0/36 Expansion-scoped → **19/36** at the war scope | the scope was chosen by this row; see below |
| `lane-space-race` | 0/4 at 250 turns, 0/4 at its own 1,300-turn clock | 0/36 at 250 turns → **13/24** at 600 turns | live exactly where the space race can finish |
| `competition-victory-points` | 0/4 | not applicable | `Game::native_competitions` ships off, so no competition is ever seated |

Three of those rows are findings in their own right.

**`lane-great-people` was inert at the Expansion scope and could not be
rescued by widening the decider.** Reaching the Great Person points a
*project* earns as well as patronage did not help: 0 of 36 either way. The
reason is not the decider, it is the window — patronage needs a bank the
opening rarely has and a district project needs districts the opening has not
built, so *everything this gene ranks exists only after the settling turns are
over*. It therefore asks its question where the question is live and genuinely
contestable: a `Conquest` plan ranks Generals and Admirals at 2.3 and the class
the empire's own race needs at 0.85. That is the war-scope claim §4 otherwise
defers, made at deliberately the cheapest and most reversible decider — no
production committed, no card slotted, only a ranking among people the empire
is competing for anyway. `Recovery` still keeps its own strategy. 19 of 36.

**`lane-space-race` cannot be checked on a targeted seat at all, by
construction.** `--target science` sets `victory_target`, which is exactly what
all three of `science_production`'s gates already test — so the gene adds
nothing there whatever the clock, which is what 0/4 at 250 turns *and* 0/4 at
1,300 turns says. Its subject is the **adaptive** seat, which has no target and
is therefore held to one launch pad and to spaceport cities with empty queues.
At the deployment clock it is still inert (0/36) for the reason §1 gives —
science does not finish in 250 turns. Give the same adaptive seats 600 turns,
where science victories land at turns 254 and 260, and it moves **13 of 24**
seat-pairs. The gene is live exactly in the games its lane can win, and the
deployment profile is not one of them.

**`competition-victory-points` is inert in a default native game**, and that is
a property of the rules rather than of the gene — see the section above.

## 8. First readings — under-powered, and recorded as such

Five runs now, and the honest summary first: **the only gene to reach a screen
flag is `lane-great-people`, on disjoint seeds; the largest negative of the
first window did not replicate; and one gene is a measured null.** Every
reading below is a screening flag at |z| ≥ 2, never a promotion; nothing here
changes a default.

⚠ **Read §8.4 before §8.1.** The first window's two loudest readings are both
contradicted by the disjoint-seed replication that follows it. They are left
in place, unedited, because what was believed and why is evidence — but
neither of them is a current conclusion.

### 8.1 The regime production plays — all six victory conditions

`docs/gene_screens/2026-08-22-v1-victory-lane-genes-native-6p-allseats-486-pairs.json`.
Command, so it can be extended with `--append` and a disjoint `--start-seed`:

```sh
target/ci/gene_screen --genes lane-congress-ballot,lane-policy-deck,\
lane-culture-spending,lane-great-people,congress-banks-decided,\
congress-counter-votes,envoy-infrastructure \
  --pairs 1200 --anchor-pairs 0 --players 6 --all-seats --randomize-civs \
  --jobs 6 --start-seed 62000000 --out lanes.jsonl
```

**486 complete pairs — 2,916 seat-pairs — which resolves a win Δ of ±4.5 pp at
80% power. Nothing here is resolved, and every row reads `~`.** The run was cut
short deliberately: the box is shared with eight other agent sessions and was
running at load 34 on 18 cores. What the rows are good for is direction and a
starting point, not a verdict.

| gene | on% | off% | win Δpp | z | share Δpp | z |
|---|---:|---:|---:|---:|---:|---:|
| `lane-congress-ballot` | 17.1% | 16.3% | +0.8 | +0.50 | +0.06 | +0.37 |
| `envoy-infrastructure` | 17.1% | 16.3% | +0.8 | +0.50 | −0.08 | −0.47 |
| `lane-policy-deck` | 16.9% | 16.5% | +0.4 | +0.26 | +0.07 | +0.43 |
| `congress-counter-votes` | 16.9% | 16.5% | +0.4 | +0.26 | +0.36 | +1.78 |
| `congress-banks-decided` | 16.5% | 16.9% | −0.4 | −0.26 | −0.14 | −1.01 |
| `lane-great-people` | 15.8% | 17.5% | −1.6 | −1.16 | +0.06 | +0.56 |
| `lane-culture-spending` | 15.4% | 17.9% | −2.5 | −1.75 | −0.20 | −1.19 |

The one row that looked worth a sentence was the bottom one:
`lane-culture-spending` is the largest reading in the table and it is
**negative**, in a regime whose own census reads religious 58% — the reading
that suggests an empire spending Faith on a Naturalist and a touring Rock Band
in a world decided by conversion has spent it on the wrong thing.

⚠ **That sentence did not survive its replication.** On a disjoint seed window
the same gene reads **+1.7 pp** — the sign flipped. See §8.4. The paragraph is
left here as written because the point of the replication is that this is what
a z −1.75 reading is worth, and editing the evidence away would hide it.

### 8.2 The lane's own regime — `--victories diplomatic,score`

`docs/gene_screens/2026-08-22-v2-victory-lane-diplomacy-regime-6p-allseats-120-pairs.json`,
seed 64000000. **174 pairs, resolving ±7.1 pp at 80% power**, so the flag
below is a candidate for a dedicated arm and not a verdict; the family-wise
bar for four genes is |z| ≥ 2.50 and nothing clears it.

| gene | on% | off% | win Δpp | z | share Δpp | z | read |
|---|---:|---:|---:|---:|---:|---:|---|
| `congress-banks-decided` | 18.4% | 14.9% | +3.4 | +1.36 | +0.02 | +0.05 | ~ |
| `envoy-infrastructure` | 16.1% | 17.2% | −1.1 | −0.44 | +0.03 | +0.10 | ~ |
| `congress-counter-votes` | 16.1% | 17.2% | −1.1 | −0.57 | −0.37 | −1.55 | ~ |
| `lane-congress-ballot` | 14.9% | 18.4% | −3.4 | −1.36 | **−0.51** | **−2.07** | **share hurts \*** |

Two things happened as this run grew from 120 pairs to 174, and both are worth
recording because they are what a screen is for:

- **`congress-banks-decided` was the strongest reading in this whole body of
  work at 120 pairs (+6.7 pp, z +2.18, `helps *`) and had fallen to +3.4 pp
  (z +1.36, unresolved) by 174.** That is the ordinary behaviour of a
  discovery-sized flag and the reason `docs/GENE_SCREEN.md` says a `*` is a
  candidate for a dedicated arm rather than a promotion. It remains the gene
  worth pointing the next batch at — it is not one this work invented, but one
  that existed, off, reachable only as an `elo.rs` arm, with the number that
  motivates it sitting unused in its own doc comment (26 of 192 ballot
  decisions already settled, ~1.4 free points a seat a game). Registering it
  as a gene cost a toggle pair and a row; §9 is about the other thirty.
- **`lane-congress-ballot`'s negative persisted** across both windows
  (−0.61 z −2.33 at 120, −0.51 z −2.07 at 174), which is why it was split
  rather than left alone.
- **`lane-congress-ballot` reads negative on score share, and has been split
  because of it.** The plausible mechanism is entirely on one side: naming the
  right outcome and target costs nothing, while `congress_affordable_votes`
  empties the treasury behind the ballot and a **winning** ballot is not
  refunded — so a seat still settling pays for a resolution it would have named
  correctly for free. That is two claims, so it is now two genes:
  `lane-congress-ballot` keeps the **scoring** half and `lane-congress-favor`
  takes the **stake**. ⚠ **The reading above belongs to the composite**, which
  is what the screened binary carried; it is attributed to the composite and
  to neither half. The halves are unscreened, and pricing them apart is the
  first thing the next batch should do.

⚠ **Even here the lane barely lands.** The regime census reads *score 94%
(median t250), diplomatic 6% (median t233)* — restricting the game to two
victory conditions does not make the Diplomacy lane reachable, which is the
same finding §1 records at 1/8 and is the reason the near miss is worth
chasing at all.

### 8.3 `lane-space-race` at 600 turns

`docs/gene_screens/2026-08-22-v3-lane-space-race-600-turns-6p-allseats.json`,
seed 63000000. **312 pairs, resolving ±1.8 pp — and the answer is +0.0 pp
(z 0.00), score share −0.02 (z −0.26).**

That is a **null**, not an inert gene, and the distinction is measurable here:
the fires-check (§7) shows the gene moving 13 of 24 seat-pairs, so the games
genuinely differ. They differ without the seat winning more. The gate it
corrects is real — an adaptive Science racer is held to one launch pad and to
spaceport cities with empty queues — and correcting it is worth nothing at
this size. Read it as: *the space race's target-vs-race inconsistency is not
where the science lane is lost.*

⚠ Nothing in this file promotes anything. Every gene remains `default:off`, and
`docs/GENE_SCREEN.md`'s rule stands: the screen ranks and directs, `ai_eval` is
the ship decision.

### 8.4 The same seven genes on a disjoint seed window

`docs/gene_screens/2026-08-22-v5-victory-lane-genes-native-disjoint-seeds-6p-allseats.json`,
seed **66000000** — the same command as §8.1 with a disjoint seed window, so
the genomes drawn are disjoint too. **480 pairs, resolving ±3.2 pp.**

| gene | §8.1 (seed 62000000, 486 pairs) | §8.4 (seed 66000000, 480 pairs) | agree? |
|---|---:|---:|---|
| `lane-great-people` | −1.6 (z −1.16) | **+3.3 (z +2.04) `helps *`** | **no — sign flipped** |
| `lane-culture-spending` | **−2.5 (z −1.75)** | +1.7 (z +0.94) | **no — sign flipped** |
| `congress-banks-decided` | −0.4 (z −0.26) | +2.1 (z +1.30) | no |
| `congress-counter-votes` | +0.4 (z +0.26) | +2.9 (z +1.47) | yes, both positive |
| `lane-policy-deck` | +0.4 (z +0.26) | +1.2 (z +0.62) | yes |
| `envoy-infrastructure` | +0.8 (z +0.50) | −0.4 (z −0.24) | no |
| `lane-congress-ballot` | +0.8 (z +0.50) | −0.4 (z −0.26) | no |

**Five of seven genes changed sign between two windows of the same experiment.**
Neither window resolves anything (±4.5 pp and ±3.2 pp against readings of 0.4
to 3.3), so this is not a contradiction — it is what unresolved means, shown
rather than asserted. `docs/GENE_SCREEN.md` says a pooled flag is a screening
result and consistent direction across chronological windows is the extra
evidence to demand; **this file now has a worked example of why.**

Two specific corrections follow, and both are corrections to things stated
earlier in this same document:

1. **The `lane-culture-spending` sentence in §8.1 is withdrawn.** Its
   motivating reading did not replicate.
2. **`lane-great-people` is the only gene here to reach `helps *`**, at +3.3 pp
   (z +2.04) — and it is the gene that was inert at its first scope, nearly
   dropped, and only kept because the fires-check forced it to the war posture
   (§7). It is the first candidate for a dedicated `ai_eval` arm. One flag in
   one window is not a promotion.

⚠ **A process trap this run found.** `--append` onto an existing rows file is
only valid while the **genome order** is unchanged. `lane-congress-favor` was
added between §8.1 and this run, which shifted every gene's bit position, and
`--analyze` correctly refused to merge the two sections rather than silently
mixing two experiments. Split the file at its second `kind: header` line and
analyse the sections separately — or re-run both halves against one build.

### 8.5 The two ballot halves, priced apart

`docs/gene_screens/2026-08-22-v4-ballot-halves-diplomacy-regime-6p-allseats.json`,
`--victories diplomatic,score`, seed 65000000. **570 pairs, resolving ±2.8 pp**
— the best-powered run in this file, and still nothing clears |z| ≥ 2.

| gene | on% | off% | win Δpp | z | share Δpp | z |
|---|---:|---:|---:|---:|---:|---:|
| `lane-congress-favor` (the stake) | 17.4% | 16.0% | **+1.4** | +1.42 | −0.10 | −0.72 |
| `congress-banks-decided` | 17.2% | 16.1% | +1.1 | +1.14 | +0.10 | +0.81 |
| `lane-congress-ballot` (the naming) | 15.8% | 17.5% | **−1.8** | −1.30 | −0.16 | −1.23 |

**The halves separate, and they separate the wrong way round from the reason
given for splitting them.** §8.2 argued the harm was on the staking side —
that `congress_affordable_votes` empties a treasury a winning ballot does not
refund. Priced apart, the stake is the **positive** half in all four windows
(+3.7 at 108 pairs, +4.2 at 144, +2.0 at 498, +1.4 at 570) and **naming** the
ballot for the raced lane is the half carrying the negative (−1.9, +0.0, −2.0,
−1.8).

So the mechanism in that paragraph, and in `congress_lane`'s doc comment, was
a plausible story that the split itself refuted. What the ordering suggests
instead — and it is a suggestion, at z −1.30 — is that scoring a ballot for
the lane the empire is *racing* rather than the posture it is *in* names the
wrong outcome: a seat still settling has different interests in a resolution
than the diplomat it intends to become, and voting as the diplomat costs it
the Favor refund a losing ballot would have paid. **The splitting was right;
the reason given for it was not.**

## 9. What the genome instrument cannot see

`gene_screen` discovers its genome from `LIVE_TREATMENTS`,
`PRODUCTION_TREATMENTS` and `PRODUCTION_OPT_INS`. A behaviour with no row in
any of the three is invisible to it however real it is — and a lot of
behaviour has no row:

```sh
python3 - <<'EOF'
import re
adv = open('src/ai/advanced.rs').read()
tre = open('src/ai/advanced/treatments.rs').read()
off = set(re.findall(r'^\s+([a-z0-9_]+): false,\s*$', adv, re.M))
rows = set(re.findall(r'\("([a-z0-9_]+)",', tre)) | set(
    re.findall(r'AdvancedAi::(?:en|dis)able_([a-z0-9_]+)\)', tre))
body = adv[adv.index('fn promoted_policy_envoy'):]
body = body[:body.index('\n    pub fn ')]
on = set(re.findall(r'ai\.([a-z0-9_]+) = true;', body)) | set(
    re.findall(r'ai\.enable_([a-z0-9_]+)\(\)', body))
print(sorted(off - rows - on))
EOF
```

At the time of writing that prints **33** fields that are off in the shipped
agent and named in no table — `coordinated_finish`, `coupled_expansion`,
`early_rush`, `diplomatic_opening`, `city_strategy`, `fog_honest`,
`great_work_veto_by_district`, `price_the_suzerainty`, `timed_war` and two
dozen more. Each is reachable as an `elo.rs` arm and therefore priceable one
batch at a time by `ai_eval`; none of them is priceable by the screen, which
is the instrument `docs/GENE_SCREEN.md` says should run "after each batch of
landed treatments".

The count is a heuristic — it matches field names against table entries, and
an arm whose published identity differs from its field spelling can slip
through either way — so read it as "at least 33", not as a census.

Three of the 33 are the Diplomacy rows added above, chosen because the lane
was the thinnest and because the first of them has a measured number sitting
unused in its own doc comment. The remaining thirty are a standing backlog,
not a claim that any of them helps: **making a behaviour screenable is cheap
(a toggle pair and a row) and says nothing about whether it should ship.**

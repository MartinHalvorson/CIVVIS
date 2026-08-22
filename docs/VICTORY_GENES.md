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

## 4. The six genes

All six ship **off**, live in `src/ai/advanced/victory_lane.rs`, and are rows
in `PRODUCTION_OPT_INS` — so `gene_screen` discovers them without being told,
and `victory_eval --with <tag>` can seat one by name in the lane it exists
for. Each is one sentence and one call site.

| gene | lane | what it changes |
|---|---|---|
| `lane-congress-ballot` | diplomacy | the World Congress ballot is scored, and backed with Favor, by the raced lane |
| `lane-great-people` | all four | Great Person patronage ranks classes by the raced lane |
| `lane-policy-deck` | all four | the policy cards are chosen for the raced lane |
| `lane-culture-spending` | culture | the Naturalist and the touring Rock Bands, and the Faith reserve that keeps them affordable, follow the race rather than the plan |
| `lane-space-race` | science | a Science **racer** is treated as a Science **target** by the space race: three pads for the parallel laser race instead of one, a launch project may claim a city with something else queued, and the pass opens at all |
| `competition-victory-points` | diplomacy | a scored competition's first place is priced by the Diplomatic Victory Points it pays |

And three that are **not new behaviour** — they already existed, off in
production and reachable only as named `elo.rs` arms, so `gene_screen` could
not see them and the genome instrument has never priced any of them (§7):

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
   The war case is a separate gene and a separate screen, not made here.
   `raced_lane` also answers `None` when the raced lane is Conquest or the
   score tally, because there is no economic lane to substitute.
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

## 7. What the genome instrument cannot see

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

# AI Development Guide

The engine exists so you can develop advanced AI strategies against a
Civ-6-like game without a UI in the loop.

## Runtime status

CIVVIS does not use a language model at runtime. Its game agents are local Rust
controllers, and the observer's plan/reasoning feed is a deterministic journal
of their decisions rather than generated prose.

Keep deployment, builtins, and evaluator arms separate when describing the AI:

| surface | controller in a normal checkout |
|---|---|
| major civilization, no seated league | stock `AdvancedAi` |
| supervised exhibition | rank-weighted sample from the current table size's top three conservative outright winners; leader/civilization placement rating breaks equal win bounds |
| city-state or barbarian | `BasicAi` |
| human-seat auto-play | selected live roster entry, with scripted builtin fallbacks |
| `neural` / `policy` | `BasicAi` / champion-weight `AdvancedAi` fallback because no value net ships |
| `strategic` | champion-weight `StrategicAi` using score-share rollouts; offline `league_only` anchor, not an exhibition seat |

The shipped tree contains and embeds `data/evolved/best.json`; it contains no
`valuenet.json`. `builtin_provenance` and `ai_eval` report the effective
controller after those fallbacks. The exhibition supervisor uses `--league
auto`, seeds a mutable runtime roster from `data/league`, and excludes
`league_only` entries from live seating. Without a league, the server uses
stock `AdvancedAi` for every non-human major.

There are seven concrete `Ai` implementations: `RandomAi`, `BasicAi`,
`AdvancedAi`, `NeuralAi`, `StrategicAi`, `PolicyAi`, and
`ProductionSearchAi`. They are not a linear strength ladder.
`ProductionSearchAi` is a retained negative result, while the generic
`Oracle<A>` wrapper is diagnostic-only and cannot be selected as a rated
entrant.

The real Civilization VI tooling does not transplant this controller whole.
`tools/civ6_strategy.py` exports the economic subset of a parameterized league
strategy while Firaxis' AI handles the rest. `tools/civ6_control` is a separate
Lua heuristic seat controller. Its generated ladder currently contains no
completed attempt; neither bridge is external strength evidence for
`AdvancedAi`.

## In-process Rust agents

Implement the `Ai` trait; you get the full `Game` API (`legal_actions`,
`apply`, all queries):

```rust
use civvis::{ai::{Ai, run_game, AdvancedAi}, game::{Action, Game}};

struct MyBot;
impl Ai for MyBot {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        // inspect g, apply actions...
        let _ = g.apply(pid, &Action::EndTurn);
    }
}

let mut g = Game::new(4, 28, 18, 1, 250, 2);
let mut ais = AdvancedAi::fleet(&g);
run_game(&mut g, &mut ais);
```

`AdvancedAi` is the default major-civilization agent when no league strategy
is seated. It maintains persistent grand strategy, victory, campaign,
force-group, settlement, builder, and threat
state; coordinates research, civics, policies, governments, Secret Societies,
diplomacy, production, spending, religion, trade, and unit orders; and falls
back to the stable city governor for routine production. `advanced_v1`
preserves the pre-upgrade agent as a frozen regression control. `BasicAi` is
the deterministic lightweight agent used by city-states and barbarians. All
three read full state (cheat on fog); fair-play agents should restrict
themselves to `civvis::obs::observation(&game, pid)`.

Default strategic planning also reads the public victory-race information for
every rival. An imminent science or score win becomes a military-denial target,
a culture lead triggers defensive Culture and Tourism investment, a religious
lead is met with theological pressure (or military denial when no religion is
available), a Diplomatic Victory lead redirects Favor and envoys, and captured
foreign Capitals force a recovery posture. This urgency can override a nearer,
weaker distraction, while an explicit benchmark victory target remains fixed.
Economic plans normally persist for five turns to avoid strategic thrashing,
but a surprise major war, a newly threatened city, or an imminent rival
victory interrupts that window and triggers an immediate reassessment.
Incoming diplomatic proposals are priced against their Gold transfer,
grievances, current strategy, alliance type, war position, and campaign
fatigue; the agent no longer accepts an exploitative friendship payment or a
non-peace pact with the rival it is trying to deny.
Advanced agents proactively seek Research, Cultural, Religious, Military, or
Economic Alliances according to their victory plan, reject imminent victory
rivals, and prefer partners whose technology, Tourism, military, religion, or
city-state network complements the plan. Both AI tiers prioritize the first
bilateral route that accelerates Alliance experience; Advanced Traders also
price the exact route yield and Cultural Alliance Great Person benefit. Trader
production is capped by reachable, empire-wide unreserved destinations, and an
idle Trader relocates when another origin can use the remaining route capacity.
Advanced envoy placement values each named suzerain bonus against the active
victory plan, prices the next 1/3/6 threshold from the empire's actual active
building tiers and production queues on a per-Envoy basis, avoids contesting a
Level 3 Economic partner whose bonus is already shared, and uses actual
Valletta Faith prices when converting reserves into immediate infrastructure
or defenses.
Congress ballots follow the same plan: Diplomatic agents back themselves for
World Leader, other strategies steer target rewards toward the civilization
furthest from victory, and competition votes predict the strongest public
candidate instead of mechanically voting for the current player. Military and
City-State Emergency votes additionally price the target's victory pressure,
Grievances, relative military strength, and the voter's city-state interest.
Supporting agents accept the mandated coalition war, retarget their campaign
at the captured objective, and prefer liberation over keeping or razing it.
Espionage follows the same plan. Advanced agents reserve a home Counterspy for
Science or recovery when a Capital or Spaceport needs protection; other agents
establish sources, select mission promotions, and target rival technologies,
Spaceports, Great Works, city-state Envoys, Governors, production, or Loyalty
according to victory strategy and campaign target. Operation ordering combines
mission value with the live success probability after sources, policies,
buildings, Governors, and defending Spies. The lightweight AI uses the same
legal mission model but maximizes general expected value, and both agents can
recover captured operatives through valued bilateral trades.
District production is family-aware: unique replacements inherit the strategic
role of their standard district, while candidate sites are compared using
their actual adjacency, specialist and Great Person yields, housing, amenities,
Loyalty, air capacity, defenses, appeal, and one-time effects. Housing value is
derived from the post-build city state, including Aqueduct water access and the
appeal bands for Neighborhoods and Preserves; non-specialty districts are not
subject to an invented AI population cap. Search sees each district's
progress-scaled, underbuilt-discounted cost; once a site is selected, that
foundation is offered for resumption ahead of fresh sites and retains its
locked cost through later research. Production search evaluates incremental
remaining cost, so item-specific paused progress and usable overflow act like
cached search work instead of being mistaken for a fresh build. When an
Aqueduct, Dam, or Canal is under construction, a Military Engineer routes to
its foundation and spends charges there instead of being absorbed into an
army's support screen.

## Victory targeting and full-game validation

Every major can be assigned an explicit victory objective. Targeted agents
coordinate research, civics, policy cards, production, diplomacy, spending,
and unit orders around that objective; city-states and barbarians continue to
use the lightweight agent.

The six pipelines are concrete rather than score labels. Science reserves a
Spaceport and completes the launch chain; Culture builds a Theater Square
network, recruits cultural Great People, trains and routes capacity-aware
Archaeologists, reaches the Conservation/Professional Sports tourism unlocks,
improves tourism tiles, connects every rival with a Tourism-amplifying Trade
Route before duplicating links, buys the direction of Open Borders that boosts
its own pressure, buys compatible Great Works from civilizations with genuine
duplicates while preserving its own collection, and sends promoted Rock Bands
to the best risk-adjusted foreign venues. It also matches an available Tier
3/4 government used by the leading Culture defender to remove the
conflicting-government penalty;
Religion founds, enhances, defends,
and spreads its faith while reconverting its own core first;
Diplomacy prioritizes Favor, envoys, alliances, city-state liberation, and
strategic World Congress voting. Congress choices score both the A/B outcome
and its target, coordinate with visible ballots, contest a rival DVP leader,
and spend additional Favor when pursuing a diplomatic victory; Domination
coordinates production and force objectives, then reserves one reachable land
unit per ungarrisoned occupied city in ascending Loyalty order; Score
balances expansion and near-term empire value. Society choice supports the
same goal: Hermetic Order for Science, Voidsingers for Culture/Religion, and
Owls of Minerva for economic, diplomatic, and conquest plans.

```rust
use civvis::ai::{run_game, AdvancedAi, VictoryTarget};
use civvis::game::Game;

let mut game = Game::new(4, 28, 18, 7, 1_200, 0);
let mut ais = AdvancedAi::fleet_targeting(&game, VictoryTarget::Science);
run_game(&mut game, &mut ais);
assert_eq!(game.victory_type.as_deref(), Some("science"));
```

`victory_eval` runs the real game loop without injecting progress or invoking
victory checks directly. It exits nonzero if the resulting victory type does
not exactly match the requested target:

```bash
cargo run --release --bin victory_eval -- --target all --games 3 \
  --start-seed 9000 --players 2
```

`--target` accepts `science`, `culture`, `religion`, `diplomacy`,
`domination`, `score`, a comma-separated subset, or `all`. Per-condition turn
limits reflect the length of each race; `--turns` overrides them for bounded
diagnostics. Map dimensions can be overridden with `--width` and `--height`.

### Validated regression baseline (2026-07-22)

The current engine passes exact, unassisted full-game victories for every
target on two independent seeds. On seeds 20000 and 20001 respectively, the
winning turns were Science 1021/940, Culture 175/385, Religion 79/177,
Diplomacy 395/395, Domination 82/136, and Score 301/301. The diplomatic turns
reflect the stock two-stage resolution model rather than the former
target-only ballots.

Against the frozen `advanced_v1` control on mirrored current-engine maps,
Advanced v2 won 61–39 across 100 four-player games and 26–24 across 50
eight-player games: 87–63 combined (58.0%). Use these as regression baselines,
not universal strength claims; repeat them when rules or evaluation settings
change.

## Coordinated forces

During a war, `AdvancedAi` partitions military and support units by movement
domain and command distance. Each resulting `ForceGroup` is an inspectable army
or fleet order with a common anchor, campaign objective, focus-fire target,
readiness, local strength ratio, and one of five postures: muster, advance,
engage, hold, or recover. Movement then scores the order as a whole: melee
screens ranged and siege units, roles keep useful engagement depth, weak local
odds discourage unsupported advances, and stragglers rejoin their group.
Orders are recomputed before every combat-unit step, so a kill, retreat, newly
opened line, or local-power swing immediately changes the remaining force's
focus and movement instead of waiting for the next turn.
Campaign selection evaluates major civilizations and exposed city-states in
the same distance, strength, development, and victory-pressure frame. A
city-state is discounted as a target when the attacker has invested Envoys,
can secure it immediately with free Envoys, already controls it as Suzerain, or
would discard a valuable type bonus. Major wars prefer an available low-cost
casus belli; otherwise the planner denounces and waits for Formal War, except
when an imminent rival victory makes the five-turn delay strategically fatal.
Within the selected rival, city move ordering combines live Garrison and wall
health, approach width, staged local force, reinforcement distance, post-capture
Loyalty pressure, development, liberation value, Spaceport denial, and the
Domination value of an original Capital. A breached front can therefore be
taken before a poisoned Capital, while the Capital becomes the principal
objective as soon as its defenses and approach make the line forcing.
Positional ties favor taking at least one useful step each turn; remaining in
place is reserved for recovery, attacks, explicit defensive/muster positions,
or cases where every legal move is materially worse. At peace, troops that
have finished exploring rotate among persistent frontier patrol posts instead
of accumulating indefinitely at the capital.

Military units also follow class-specific doctrine rather than sharing one
generic policy. Recon units keep exploring during wars unless a clearly good
attack is available; assault and high-strength units accept thinner combat
advantages; mobile and naval-raider units exploit pillage opportunities;
ranged units preserve firing depth; siege prioritizes cities and districts;
support stays close to the line; fighters compare exact strikes against
interception patrol value, while bombers search cloned strike results and
useful rebasing rather than statically preferring any city. Aircraft are kept
out of ground-force readiness and local-superiority totals. Production reserves
one logistics capability for a real land force (and a second for a large one):
Rams/Towers only for eligible wall eras, Balloons/Drones when siege can exploit
them, and Medic/Convoy support for wounded or mobility-constrained armies. If no recon unit exists, one
ordinary combat unit explores at peace instead of sending the whole army.
When hostile aircraft actually exist, the same reserve searches for the
strongest available Anti-Air Gun or Mobile SAM and sizes coverage to the air
threat instead of counting support weapons as frontline land strength. Before
committing an ordinary melee or ranged attack, the principal evaluator makes
every candidate on a clone and values the seeded damage, kills, survival,
wall/Encampment damage, and actual city transfer. A bounded quiescence search
then orders the opponent's forcing replies and extends the four strongest
branches through a second focus-fire action. This catches poisoned captures,
selects melee city finishes for hybrid units, and distinguishes high-value
kills that static exchanges score as ties without expanding quiet movement
into a full turn-tree search.

```rust
for force in ai.force_groups() {
    println!("{:?} {:?}: {:?}", force.domain, force.posture, force.units);
}
```

The planner is domain-generic: fleets intercept ships and embarked enemies,
screen embarked settlers, and choose adjacent coastal approaches so ranged
ships can reduce defenses before naval melee captures. Coastal empires treat
Sailing, Shipbuilding, Celestial Navigation, and Cartography as a capability
chain, keep a role-balanced exploration/escort fleet, and pursue current naval
upgrades during maritime wars. Settlers retain globally scored, route-checked
colony targets across multiple turns and linked ships lead them over water.
New domains can use the same group/order pipeline instead of adding another
independent-unit AI.

## Genetic strategy evolution

`Weights::to_vec` exposes a 40-gene search surface for part of the advanced
agent. It is not a complete encoding of the policy: research ordering, city
roles, many strategic gates, policy-deck mode, and dedication choice remain
hand-written or live outside the vector. Alongside economy, diplomacy,
production, and exchange evaluation, the vector includes command radius,
muster radius/readiness, cohesion, focus fire, screening, role spacing,
objective pressure, local-superiority caution, and withdraw/rejoin thresholds.

```bash
cargo run --release -- evolve --generations 100 --pop 24 --games 12 \
  --players 4 --threads 8 --dir evolved
civvis tournament --ais evolved,advanced,advanced_v1,basic --games 80
```

Every genome plays the real `AdvancedAi` against the reigning champion on
shared map seeds and rotating seats. Multiplayer training tables also draw from
`archive.json`, a hall of fame of prior champions, to reduce cyclic strategies
and catastrophic forgetting. Fitness combines final score share with a smaller
kill/capture signal so battlefield doctrine can learn before it decides a whole
game. Elites survive; fitter genomes are crossed and mutated within per-gene
bounds. A candidate only replaces `best.json` after a sequential match confirms
a higher win rate and it does not regress on a generation-independent,
fixed-seed holdout benchmark. `population.json` resumes the run, `history.csv`
records training and holdout progress, and `dataset.csv` feeds value training.
Old checkpoints load with defaults for newly introduced genes and validation
metadata.

The continuous league and live exhibition have a different **selection**
contract from the placement ladder. Glicko continues to rate full placement
because that is useful for matchmaking and display, but genome parents, niche
elites, retirement, and live seeded seating are ordered first by 95% Wilson
bounds on **outright wins at the current table size**. A
leader/civilization's placement rating breaks an equal live win bound. A
two-player win rate is not compared as though its parity rate matched a six-
or eight-player table. The roster checkpoints games and wins by seat count;
old rosters recover the retained raw portion from `matches.csv`, preserve
unreconstructable prior games only in the all-history totals, and show both in
`--standings`. Live seating requires enough exact evidence to fill its entire
top-three sample; a new or unmigrated roster otherwise retains the old
placement-only policy. Once that evidence bar is met, seating exhausts the
exact-profile entrants before falling back to unprofiled placement candidates,
so removing the first pick cannot switch the next seat back to a different
objective. This prevents a safe second-place specialist from breeding or being
exhibited because placement compressed who won, a duel-heavy entrant from
winning selection because its wins were easier to obtain, and a two-game lucky
streak from outranking a settled winner.
The `strategic_deep_league` transfer control uses the same conservative
outright-win objective on the committed snapshot; the fixed-profile tournament
below remains the instrument for a fully standardized longitudinal comparison.
The standalone `evolve` fitness above still uses its cheaper score/combat proxy;
its separate promotion gate remains the point where wins decide shipment.

## Elo tournaments

```bash
civvis tournament \
  --ais advanced-20260731-settlement=advanced,advanced_v1,basic-20260731-settlement=basic,random-20260730=random \
  --games 40 --players 4
civvis tournament --standings          # verify and print without playing
```

The CLI checkpoints every completed game to the tracked
`data/elo_ratings.json` ledger (override it with `--ratings path`). Its online
rating key is the **player** — a human account or named AI strategy — accumulated
across every leader and civilization it draws. Separate
`player × leader × civilization` rows remain as matchup diagnostics. A newly
seen combination inherits that player's current Elo instead of silently
starting the established player over at the base rating.

An entrant may be written as `rating-identity=controller`. The left side is
what the ledger measures; the right side is the builtin the game constructs.
Use a new immutable identity whenever a mutable controller changes, for example
`advanced-20260815=advanced`. Otherwise a row named only `advanced` is a
lifetime average of several implementations and will dilute improvements over
time. The default command dates every mutable controller; only the deliberately
configuration-pinned `advanced_v1` keeps its bare identity and anchors successive versions on
one connected scale. After every update the ledger translates every rating by
the same amount to keep that contract-pinned control at exactly 1500. Pairwise gaps
and win expectations are unchanged, while fresh weak identities can no longer
inflate later generations relative to inactive older ones. Custom tournaments
can select another entrant with `--anchor identity`; `--anchor none` leaves a
one-off pool floating. The CLI also refuses two identities that resolve to the
same effective controller and refuses a learned entrant that silently degraded
because a definitional artifact is absent.

`advanced_v1` is not a copied historical implementation: it freezes the
victory-planning configuration but shares the underlying `BasicAi` and
`AdvancedAi` code. CI pins both source files under an explicit anchor contract.
If an edit affects the legacy path, the Elo protocol must change and the run
must use a new ledger; an edit proved to be gated away from that path still
requires an explicit review and re-pin. This guard prevents the word “frozen”
from hiding a moving control, while allowing candidate-only code to evolve.

The fog-honest city-pressure repair changed that shared legacy path, so the
complete protocol-v1 baseline is preserved, read-only, at
`data/elo_ratings_v1.json`. The island-settlement repair also changes the
shared path: a passable natural wonder is no longer an unbuildable settler
target. It therefore starts protocol v3 with a new replay-verified 40-game
ledger at `data/elo_ratings.json`; the complete protocol-v2 baseline is
preserved, read-only, at `data/elo_ratings_v2.json`. Historical ledgers remain
useful evidence but must not be extended or mixed with a later protocol.
The protocol-v3 baseline rates `advanced-20260731-settlement` at 1615.0 online
Elo and 1643.2 direct Elo against `advanced_v1`, from a 28/40 pair score (95%
54.6–81.9%, or 1531.8–1762.5 direct Elo). This is a fresh measurement record
for the corrected controller, not an effect-size claim for the settlement fix.

Schema 3 binds a ledger to the complete rating profile: an explicit experiment
protocol version, a deterministic fingerprint of the fully merged rules JSON,
a readable contract for every fixed lobby default, ordered controller roster,
table size, dimensions, turn limit, city-state count, speed, map
script/shape/poles, active mods, and K. The setup contract records Civ6 rules,
an Ancient start, Prince difficulty, barbarians, disaster intensity 2, no game
modes, the Civ6 leader pool, deterministic stock civilization fill, no humans,
free-for-all teams, and all six victory types. The readable mod names say what
was loaded; the fingerprint binds their actual content, so editing a mod in
place cannot reuse its old scale. A later run with any different field is
rejected with a request to use another `--ratings` path. Versioned player
identities may change while their ordered controller roles remain `advanced,
advanced_v1, basic, random`; this is what lets a new challenger join without
quietly changing its multiplayer controls. Bump the protocol when engine
behavior, implicit setup defaults, or scoring semantics change enough to
define a different contest; rules-data changes are detected automatically.
The preserved protocol-v1 ledger is a canonical 40-game,
1500-centred baseline bound to the CLI's stock 4-player Standard game (rules
`fnv1a64:3423bd46da2b8cd7`, 60×38, 500 turns, six city-states, Pangaea,
flat/poles, no mods, K=24). Protocol-v2 uses the same fixed setup with a new
40-game evidence set: `advanced-20260731` is 1570.5 online Elo and 1623.6
direct Elo against `advanced_v1`, from a 27/40 pair score (95% 52.0–79.9%, or
1514.0–1739.9 direct Elo). The historical frozen July 30 run rates
`advanced-20260730` at 1589 and the
`advanced_v1` anchor at 1500, an +89-point online gap. The order-independent
direct result is 1708, from a 31/40 pair score whose 95% interval is
62.5–87.7%, or 1589–1841 after the same monotone Elo transform. Future dated
challengers can therefore be compared through the unchanged control, with both
effect and uncertainty visible. This prevents a short smoke test, a mod, or another map size from
quietly changing what its Elo scale measures. Settings control the experiment;
versioned identities control what player generated each observation. Both are
required for a longitudinal number.

The ledger retains games and wins at both levels. If fewer entrants than seats
would cause an AI to occupy several seats, a persistent run refuses: controlling
twice as much of the map changes the contest even if the arithmetic deduplicates
the comparisons. In-memory/manual pools still defensively count a cloned player
once per world.

Every fresh schema-3 ledger also retains the raw scored table for every game,
not only the resulting point estimates. On load, the aggregates are replayed
and checked against that evidence, so a hand-edited or corrupted Elo cannot
pass as a result. Persistent events are keyed by run seed, game index, map seed,
and ordered identities. Repeating a run is idempotent; replaying the same key
with a different outcome is an error that points back to a reused mutable
identity. Writes are atomic and briefly locked per game, and keyed events are
sorted before replay, so concurrent workers preserve every result *and* finish
with the same ratings regardless of lock-acquisition order. Migrated schema-1/2
aggregates cannot recover games that were never stored and say so explicitly in
the leaderboard; start a fresh path for a fully auditable baseline. A keyed
history also has one canonical order. Every raw event must contain exactly the
profile's table size with distinct player identities, and its K must match the
bound profile; replay-consistent but structurally truncated evidence is still
rejected.

The leaderboard additionally derives a **direct performance Elo** for every
player co-seated with the fixed anchor. It converts that player's aggregate
pair score into the usual 400-point logistic scale, with a Jeffreys half-result
on each side so finite undefeated samples remain finite. This diagnostic is
order-independent and is recomputed from raw evidence. Its printed 95% Wilson
interval appears both on the observed pair-score scale and after the same
monotone Elo transform, so a short run cannot masquerade as a settled Elo gap.
An interval touching 0% or 100% correctly has an infinite Elo endpoint. Use
this direct-anchor result at a fixed game count as the longitudinal baseline;
use the incremental K-factor Elo as the continuously updated
matchmaking/leaderboard state.
On a migrated schema-1/2 pool, the heading explicitly says “post-migration” and
“retained raw games only”: that direct slice is valid for those new games, but
it excludes the unreconstructable aggregate prior and is not the standardized
full-history baseline.

Entrants use a seeded round-robin seat schedule instead of independent random
sampling. Across one complete cycle, every fixed civilization seat sees every
configured entrant exactly once. Score ties are Elo draws; an actual victory
outranks score even when the winner has fewer score points.

For lower-variance two-player measurement, the paired evaluator runs every map
twice with seats swapped and reports outcome plus economy/army diagnostics:

```bash
cargo run --release --bin ai_eval -- advanced basic --pairs 100 --seed 4000
```

```rust
use civvis::elo::{
    builtin_ai, leaderboard, run_persistent_tournament, TourneyCfg,
    DEFAULT_RATINGS_PATH,
};
let names = ["mybot", "advanced_v1", "basic", "random"]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
let pool = run_persistent_tournament(&names, |name, seed| match name {
    "mybot" => Box::new(MyBot),
    other => builtin_ai(other, seed),
}, &TourneyCfg::default(), DEFAULT_RATINGS_PATH)?;
println!("{}", leaderboard(&pool));
# Ok::<(), std::io::Error>(())
```

Multiplayer games score as simultaneous pairwise Elo results by final placement
(K/(n-1) scaling). `run_tournament` remains available for an in-memory,
non-persisted evaluation. Game generation and seating are deterministic given
`cfg.seed`; persistent ratings also depend on the ledger's prior state.

Non-tournament callers can rate human and AI players through the same API by
constructing each result with `RatedPlayer::new(player, leader, civilization,
score, won)` and passing the finished table to `EloPool::record_game`.

Use the shared ledger to prioritize low-rated player/leader/civilization rows
with meaningful sample counts, then rerun the same evaluation battery after a
strategy change. Missing rows are unmeasured, not evidence of parity.

## External agents over HTTP (any language)

`civvis play --no-open --port 8765` exposes the JSON protocol:

- `GET /state` — observation for player 0 (fog applied) + `legal_actions`.
  A currently visible district tile also carries `district_yields` and an
  `adjacency` ledger (one entry per source: `source`, `count`, `percent`,
  `yields`, and the pre-rounding `raw`), and a tile where one of your own
  cities is building a district carries the same figures under
  `planned_district`. Both read the live neighborhood, so they are absent
  under fog
- `POST /action` body `{"action": {"type": "end_turn"}}` — applies, runs the
  AI opponents, returns the new state (+`error` string on illegal actions)
- `GET /rules` — the full ruleset (techs, units, costs, ...)
- `POST /new` body `{"seed": 7, "num_players": 4}` — fresh game; selecting a
  player count also applies its full stock Civ VI map profile: 2 = Duel
  (44×26/3 city-states), 4 = Tiny (60×38/6), 6 = Small (74×46/9), 8 = Standard
  (84×54/12), 10 = Large (96×60/15), and 12 = Huge (106×66/18). Explicit
  `width`, `height`, or `num_city_states` fields override individual profile
  values.

A searching agent does not read fogged-tile memory between the root and the
leaf, so `game.set_fog_memory(false)` stops maintaining it: explored ground and
Natural-Wonder discovery are kept (they change the game), while the remembered
map of fogged tiles and cities — which only feeds observations — is left alone.
Outcomes are identical; a clone-and-move drops about a fifth and ending a turn
about a sixth. Turn it back on at a node you intend to observe. `civvis
rollouts` reports both, for a move and for a turn boundary.

An agent that searches spends its time cloning a position and stepping it
forward, not playing whole games, so `civvis rollouts` times that directly:
cloning, cloning plus an ordinary move, and cloning plus ending a turn. Ending
a turn is roughly eight times the cost of a move — it settles every city's
yields and refreshes every seat's map — so a search that ends turns deep in a
line costs far more per node than one that does not.

Actions are plain JSON dicts identical to what `legal_actions` returns, so an
external client can feed them to LLM tool-calling or an RL policy. No in-tree
agent currently does so. One process per concurrent game; in-process Rust
agents remain the fast path for self-play at scale. On an Apple M5 Max, the
current release Advanced-v2 workload measured
1,173 turns/sec for `benchmark --games 100 --turns 100 --jobs 1` (two players,
20×14, one game at a time). A batch that leaves `--jobs` alone plays across
every core and reaches several times that. Throughput varies materially with
map size, era, player count, and agent; older tens-of-thousands figures
describe a much smaller historical rules workload.

## Machine-learning surfaces

- `civvis::obs_tensor::obs_tensor(&game, pid)` renders a fog-honest spatial
  observation: 25 named `f32` feature planes over the wrapped map plus named
  public/global scalars. `civvis selfplay` exports these tensors and outcome
  labels, and `tools/train_spatial.py` trains a wrap-aware PyTorch model.
  Nothing in the Rust runtime loads that checkpoint yet.
- `evolve::features` is a different representation: 25 full-state empire
  aggregates. `tools/train_valuenet.py` can fit a 25→64→32→1 state-value MLP
  and write `evolved/valuenet.json`. That artifact is local/generated, not
  committed or embedded.
- `decision_features` widens the scalar input to 34 terms and raises measured
  action visibility from 44.5% to 86.1%. `action_space` supplies stable IDs for
  all 77 action variants plus contextual destination features. These are
  useful offline data surfaces; neither is a live learned policy.
- `NeuralAi` uses a 25-wide value net only to compare scripted war rollouts.
  `PolicyAi` applies legal tactical actions to clones and greedily scores the
  resulting states. `StrategicAi` rolls out adaptive and victory-lane branches,
  blending a compatible net at 25% when one exists and using score share when
  it does not.

### Current evidence, with scope

No agent has a profile-independent “strongest” result. The exhibition rotates
through 4–10 seats and matching stock map sizes, while much of the research was
run at four players on a 24×16 Standard map.

| comparison | measured profile | result | interpretation |
|---|---|---|---|
| `advanced` vs `advanced_v1` | 6p, 74×46, 6 city-states, Online | **+207**, gate PASS | robust scripted improvement on that recorded comparison profile |
| `advanced` vs `advanced_v1` | 6p, 74×46, 9 city-states, Online, randomized civilizations | −17, inconclusive (40 maps) | the ordering is not established on the matrix deployment profile |
| `advanced_evolved` vs `advanced` | 6p recorded profile above | −9, inconclusive | small-profile genome gain did not transfer |
| `strategic` vs `advanced` | 6p recorded profile above, 60 maps | −47, wins inconclusive | open; the planned 300-map confirmation did not finish |
| `strategic_cheap` vs `advanced` | 6p recorded profile above | **−63**, retain `advanced` | cheap search regressed |
| `strategic_deep` vs `strategic` | 4p, 24×16, Standard | **+45**, gate PASS | deeper search won on its source benchmark only |
| `policy_wide` vs `advanced` | 4p benchmark | **−313**, 14.2% | outcome correlation was harmful when greedily maximized |
| `production` vs `advanced` | 4p benchmark | 45.0%, sign p=0.0428 | scripted governor retained |

The learned-policy diagnosis is causal, not just a bad score. The original
25-wide policy was unchanged by 96% of tactical candidates. The 34-wide policy
could distinguish them, but selected moves that increased enemy contact—a
correlate of strong attacking positions in the training games—while losing
material. Freezing those two contact inputs restored exact parity. A predictive
state value is therefore not an action value, and calibration alone does not
license an argmax over interventions.

Search remains valuable as a counterfactual laboratory, and one searching seat
was measured at about 6.4× the game-turn cost of an all-scripted six-seat fleet
on a 74×46 map with nine city-states (three early-game seeds). Its live strength
across the rotating exhibition profiles is unmeasured, which is why the shipped
roster keeps `strategic` as an offline-only anchor.
- Ranked AI-strength roadmap and current status: `docs/AI_GAPS.md`.
  Recorded eval baselines and the regression battery: `docs/EVAL.md`.

## Evaluation tips

- Fix multiple seed sets; report paired win rate vs `basic` plus multiplayer Elo.
- Use the promotion matrix for any claim that a challenger should replace
  `advanced`:

  ```sh
  cargo run --release --bin ai_eval -- challenger advanced \
    --matrix --pairs 120 --jobs 12 --seed 12000000
  ```

  It runs the compact Standard safety profile and the six-player Online
  deployment concurrently. Deployment must PASS; compact must have enough
  evidence and must not RETAIN `advanced`. Matrix mode rejects profile-shaping
  flags, so the command cannot silently test a different game under the same
  label.
- Keep mechanism controls separate. `advanced_policy_live_control`,
  `advanced_envoy_policy`, `advanced_envoy_infrastructure`, and
  `advanced_envoy_economy` decompose policy-deck, influence-card, and production
  effects. `advanced_strategic_commitment` is the default-off soft-replan
  treatment. None is production behavior unless a later matrix gate promotes
  it.
- Use `ai_eval` to catch regressions hidden by wins (stalled settlers, obsolete
  armies, unfinished queues, or weak science/culture output).
- Keep `random` in the pool as a sanity floor.
- `soak` flags anomalies (no tech progress, minor winners) across seeds.

### 2026-07-31 full-prefix status

Production remains `advanced`, with every new behavior below default-off. This
is a failed-replacement conclusion, not a profile-independent strength claim.

| comparison | matrix sample | compact | deployment | production consequence |
|---|---:|---:|---:|---|
| live policy deck vs `advanced` | 300 maps/profile | 52.3%, +16 | 54.3%, +30 | Wilson gate inconclusive; no promotion |
| direct envoy production vs `advanced` | 120 maps/profile | 48.3%, −12 | 54.4%, +30 | gate inconclusive; no promotion |
| commitment vs `advanced` | 120 maps/profile | 51.0%, +7 | 46.5%, −25 | reject treatment |
| `advanced_evolved` vs `advanced` | 120 maps/profile | 57.3%, +51 | 45.6%, −30 | reject transfer |
| live policy deck vs `advanced_v1` | 300 maps/profile | 50.5%, +3 | 53.7%, +26 | gate inconclusive; anchor retained |

`advanced_envoy_priority` is the direct mechanism arm. After the opening book,
it can place the first incomplete empire-unique Diplomatic Quarter, Consulate,
or Chancery stage into an idle adaptive-production queue. It requires a met and
contestable city-state plus useful remaining envoy stream, preserves existing
queues, and yields to Recovery, local danger, active rushes, and major war.
`advanced_envoy_infrastructure` remains the valuation-only control. Do not infer
direct production behavior from that older arm.

Evaluator-only fallbacks now report what they actually build: champion-backed
netless policy arms are `advanced_evolved`, and champion-backed netless neural
is `basic_evolved`. Direct evaluation refuses degraded artifacts by default;
`--allow-degraded` is a diagnostic opt-in and is forbidden in `--matrix` mode.
The matrix also uses sample-size-independent profile seeds, so extending a
prefix preserves both profiles' earlier maps.

The next strength experiment should be a single pre-registered composite of the
live policy deck and direct envoy production on fresh stable prefixes. Treat it
as a new hypothesis: the two favorable directions may overlap, and higher envoy
income may simply move production cost elsewhere.

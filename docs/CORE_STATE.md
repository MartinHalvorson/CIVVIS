# The irreducible game state

What a CIVVIS world must carry for the rules to play on, measured rather than
argued, and how `Game` was reduced to carry only that beside it. Measured
with `civvis core-probe` (`src/core_probe.rs`) and `tools/core_probe.py` on
PR #5749, 2026-11-24; reduced in #5761-#5770 ("The reduction, done"). The
per-field classification is `docs/core_state_classes.json`.

## What "irreducible" means here

The rules are deterministic: `apply(state, seat, action) -> state`, with the
RNG inside the state. Two saves are equivalent if every future sequence of
actions gets the same response from both: the same legal actions, the same
refusals, the same world. The irreducible state is the smallest description
that keeps equivalent saves together and inequivalent ones apart (the
Myhill–Nerode state of the game as a machine). Everything else in a save is
carried but never consulted by the rules.

There are two minimal forms, trading space for time:

- **History**: setup options, seed, and the action log. Replaying it
  reproduces the game (verified below, with one leak), but reaching turn N
  costs running turns 1 to N-1. No shortcut to turn N exists except running
  the turns, which is what computational irreducibility means here.
- **Snapshot**: the Markov state at turn N. This is what a clone, a search
  branch, a debugger timeline node, a hash and a save should hold.

## The answer

A save holds **404 fields** at the depth measured here (every top-level field
of `Game`, and every field of every player, city, unit, spy, trade route,
deal and tile). Pooled over 15 games:

| Layer | Fields | Value bytes, fog memory off | Fog memory on | Where it belongs |
|---|---:|---:|---:|---|
| **Core**: blanking it changes the game | 175 | 189 KB (32%) | 306 KB (5%) | the world |
| **Presumed core**: rule code reads it, no tested window exercised it | 84 | 37 KB (6%) | 52 KB (1%) | the world, until a test exercises it |
| Map geometry fixed at generation | 7 | 233 KB (40%) | 322 KB (5%) | regenerated from the seed |
| Setup fixed for the whole game | 36 | 2 KB | 2 KB | once per game, shared |
| Last-seen memory per seat | 4 | 54 KB (9%) | 5.6 MB (88%) | the seat's observer |
| History | 39 | 69 KB (12%) | 99 KB (2%) | an append-only record beside the world |
| Derived, rebuilt by the next turn boundary | 11 | 2 KB | 4 KB | computed |
| Mirror of a live Civilization VI game | 48 | 0 in self-play | 0 | one optional block |

Beyond the save, `Game` holds ~45 more mirror fields (`host_*`, `blocked_*`)
that saves already drop, and 28 runtime caches and indexes.

**The world is 259 fields**: core plus presumed core. That is 39% of a save's
value bytes with fog memory off and under 6% with it on. Measured on a
2-major game at turn 120, the world compresses to 12–16 KB with the map stored
as an overlay on the seed-generated map (only 208 of 1,144 tiles differ from
turn 0). The full JSON save is 798 KB (42 KB gzipped), and the game's action
log is 54 KB gzipped.

### Proof

- **Sufficient**: a save with every non-core field blanked at once (36–54
  fields per game) replays the recorded window to the identical world in all
  18 tries with a non-empty window (15 games over two rounds, and 3 more
  keeping the schema version): no other field differs, no action is
  refused, and every seat's
  legal action set at the start of the first and middle round is unchanged.
- **Necessary, per field**: each core field, blanked alone, changed another
  rule-bearing field, got an action refused, changed a seat's legal actions,
  or stopped the save loading or replaying. Twelve of them changed **only**
  the legal action set, in every game where they mattered: the two
  denouncement timers, friendship expiry, diplomatic favor, incidents,
  missions, victory points, governor titles spent, a spy's ready turn, a
  pending deal's sender, a unit's fortified flag, and the map's cliff edges
  (where a unit may embark). A replay of the actions the agents happened to
  take cannot see those.
- **The agents do not need the rest either**: fresh stock agents played 20
  turns from a save are deterministic (two runs from one save, identical),
  and play the same game with history, last-seen memory, derived, expiring,
  untested and mirror fields blanked (3 games). What changes their play is
  only setup and presumed-core fields the rules ignored in those windows
  (barbarian difficulty, victory settings, peace treaties, ...), which stay in
  the world.

### The core, by container

- **Clock and randomness**: `turn`, `rng`, `next_id`, `next_deal_id`,
  `world_era`, `world_era_since`, `world_era_countdown_end`,
  `meteor_strikes`, `retired_great_people`, `great_person_offer_costs`.
- **Map overlay** (per tile, over the generated geometry): `feature`,
  `resource`, `improvement`, `district`, `district_foundation`, `wonder`,
  `owner_city`, `road`, `pillaged`, `drought`, `disaster_food`,
  `disaster_production`, `disaster_faith`.
- **Seats** (54): research and civics (sets, current choice, progress,
  overflow, boosts), treasury (`gold`, `faith`, `strategic_resources`,
  `bankruptcy_amenity_penalty`, `co2_emissions`), `government`, `policies`,
  religion (`pantheon`, `religion`, `religion_beliefs`, `holy_city`), great
  people (`gpp`, `gp_claimed`, `great_people`), ages (`era_score`,
  `era_triggers`, `dedications`, `age`, `past_dark_ages`,
  `past_golden_ages`), diplomacy (`met`, `grievances`, `alliances`,
  `friends_until`, `open_borders_until`, `denounced_since`,
  `denounced_until`, `diplomatic_missions`, `diplomatic_incidents`,
  `diplomatic_favor`, `dvp`, `envoys`, `envoys_free`, `influence`,
  `quests`), governors (`governors`, `governor_roster`,
  `governor_titles_spent`), `culture_lifetime`, `tourism_lifetime`,
  `explored`, `discovered_natural_wonders`, `alive`, and `counters` (mixed:
  see the leaks).
- **Cities** (26): `id`, `pos`, `owner`, `original_owner`, `is_capital`,
  `occupied_from`, `occupation_grievance`, `pop`, `food`, `production`,
  `production_progress`, `queue`, `strategic_resource_commitments`,
  `buildings`, `pillaged_buildings`, `districts`, `wonders`, `owned_tiles`,
  `border_culture`, `hp`, `wall_hp`, `encampment_wall_hp`, `loyalty`,
  `pressure`, `atheist_pressure`, `great_person_foreign_route_gold`.
- **Units** (21): `id`, `type`, `owner`, `pos`, `hp`, `moves_left`,
  `attacks_left`, `acted`, `charges`, `xp`, `level`, `promotions`,
  `xp_bonus_pct`, `formation`, `linked_to`, `religion`, `fortified`,
  `fortify_turns`, `free_upkeep`, `levied_from`, `levied_until`.
- **Between seats**: `wars`, `at_war`, spies (9 fields), trade routes,
  pending and active deals, Congress resolutions and effects, emergencies,
  barbarian camps, scouts, raiders, targets and alerts, storms, droughts,
  fires.

**Presumed core** (84): never exercised by a tested window, read by rule code,
so kept. Among them `current`, `winner`, `peace_treaties`, `war_weariness`,
the age thresholds, `past_governments`, `tourism_pressure`,
`great_work_pieces`, the space-race and mass-driver state, citizen locks and
focus, defensive pacts, promises, anarchy, competitions, flood and fallout
state, and the per-turn strike flags. The pooled games show why the rule
holds: `coastal_lowland` looked inert until the latest game reached sea-level
rise.

## Leaks the probe found

1. **The action log is not a complete record.** Replaying the log from setup,
   or a window from a save, reproduces every field of the played game except
   `players[*].counters`, in every game tested. Stock-agent modules write their
   own bookkeeping straight into the game state: `commit:*`, `raid_prize:*`,
   `raid_wars` (`src/ai/advanced/commitments.rs`, `diplomacy.rs`,
   `opportunistic_war.rs`, `city_campaign.rs`,
   `city_state_before_the_first_capital.rs`). The engine uses about 15
   literal keys of the same map for rule tallies; the agents use 47.
   **Closed by #5767**: `Player::counters` is a `Counters` that stores the
   rules' tallies and the agents' notes apart (`is_agent_note`), and the
   world-boundary test requires every rules tally to replay exactly.
2. **Loading a save can return favor a seat already spent.** The loader
   migrates pre-trade saves by copying `counters["diplomatic_favor"]` into
   `diplomatic_favor` whenever the balance is exactly `0.0`
   (`impl From<GameSer> for Game`), but the engine still adds every grant to
   that counter (`boosts.rs`, `actions/diplomacy.rs`), so it holds lifetime
   earnings. Seen in seed 207 in two rounds. **Fixed by #5763**: only a save
   with no favor field migrates.
3. **Narration lives inside rule state.** `wars` carries war highlights and
   theater names, so renaming a city changes `wars`; `counters` mixes rule
   tallies with agents' notes. `counters` is split (#5767); the narration
   in `wars` is not yet, so a world hash would still see a renamed city.
4. **Load is not the identity on `wars`** when the narrated war ledger is
   stale (headless play runs with `set_war_ledger(false)`): seed 204. Open.

## What it costs to carry the rest

Median of 9 batches on the CI runner, 3 games:

| | Full save | Stripped to the world |
|---|---:|---:|
| `Game::clone` | 10.3–12.5 µs | 2.2–4.8 µs |
| First write to one seat | 2.6–3.2 µs | 1.6–2.5 µs |
| First write to the map | 7.7–12.3 µs | unchanged |
| First write to the unit table | 1.6–2.7 µs | unchanged |
| JSON save | 1.5–5.1 ms | 1.3–2.0 ms |

Most of the clone saving is history kept in plain `Vec`s
(`concluded_wars`, `unit_move_trails`), which every clone deep-copies.

The rules themselves (replaying the whole log, no agents, JSON load and save
included) take 18–27% of a self-play game's time, 39% with last-seen memory
tracked. The agents take the rest.

## The reduction, done

The game model is now the world plus blocks beside it, each one shared
pointer that a fork copies only when it writes there:

```
Game                          the world: the irreducible state, read and
│                             written directly (95 fields, from 183)
├── chronicle                 everything the game records about itself:
│                             event log, ended wars, nuclear strikes, siege
│                             census, turn stats, unit trails       #5761 #5769
├── host: Arc<HostMirror>     84 facts a live Civilization VI host
│                             supplied; read through Deref           #5762
└── players[i]: Player        the seat's rules state (98 fields)
    ├── aside: Arc<PlayerAside>   what the seat saw, did and was told
    │                         (16 fields); read through Deref         #5764
    └── counters              the rules' tallies | the agents' notes,
                              stored apart                            #5767
```

Every step left play byte-identical (a container moved; no rule computes
anything differently) and saves unchanged: the moved fields are still written
under their old names at their old places. `Deref` on `Game` and `Player`
means every one of the ~2,000 host-fact reads and writes kept its spelling.

The boundary is held by `src/game/world_boundary_tests.rs` (#5765, #5770):
a save with everything outside the world blanked replays the same turns to the
same world with the same legal actions; the full save replays to the world
that was played, every rules tally included; and every saved field must have
a layer in `docs/core_state_classes.json`.

| | Before | After |
|---|---:|---:|
| `Game` direct fields | 183 | 95, plus 3 shared blocks |
| `Player` fields | 114 | 98, plus 1 shared block |
| Fields outside the world copied per fork | 88 per game, 16 per seat written | one pointer per block |
{MEASURED}

### Kept on purpose

- **The action log** stays on `Game`. It is already a shared chain (a fork
  pays a pointer), it is the game's other minimal form, the history form,
  and the agents read it.
- **Values rebuilt at each turn boundary** (`gold_per_turn`, a unit's ZoC
  flags, a city's strike flag, ...) stay stored. They are derived *at a turn
  boundary*, but between two actions of one turn they are state: a city that
  has struck cannot strike again. A fork taken mid-turn, which is what a
  search branch is, needs them.
- **Setup and map geometry** stay on `Game`. The rules read them, and they
  are already cheap to fork: the tile grid is shared until written. Only a
  save needs them reduced, to seed, options and an overlay; that is the one
  step of the original plan not taken, and the measured size it would buy is
  above (12–16 KB compressed for a 2-major game at turn 120).
- **Per-record leftovers**: a unit's lifetime damage and a host's
  embarkation reading, a tile's host plot readings, a city's name, and the
  narration inside `wars`.

## Limits

Evidence, not proof. Rounds 2 and 3 pooled: 15 usable games (2, 4, 6 and 8
majors, Online speed, Continents, stock agents, two with fog memory forced
on), 40-turn windows from turns 60 to 205, legal sets sampled in two rounds
per window. A field called inert here is inert in these windows; rule code
that reads it keeps it presumed core. Not covered: human seats, simultaneous
turns, game modes, mods, Tactics battles, and the mirror of a live
Civilization VI game, which is what the mirror fields exist for.

## Rerun

```
cargo build --profile ci --bin civvis
python3 tools/core_probe.py --civvis target/ci/civvis --budget-minutes 30
python3 tools/core_probe.py --civvis target/ci/civvis --quick docs/core_state_classes.json
```

The first plays 8 games and classifies every field (about 18 minutes on 4
cores); the second reuses a classification to bisect what the agents read and
time clones (about a minute). Every row prints as a `CPROBE {json}` line.

# The irreducible game state

What a CIVVIS world must carry for the rules to play on, measured rather than
argued, and the path from today's `Game` to a world that carries only that.
Measured with `civvis core-probe` (`src/core_probe.rs`) and
`tools/core_probe.py` on PR #5749, 2026-11-24.

## The answer

A save at turn T holds **404 fields** at the depth this report reads
(every top-level field of `Game`, and every field of every player, city,
unit, spy, trade route, deal and tile). Classified by what the rules do with
them:

| Layer | Fields | Share of save value bytes | Where it belongs |
|---|---:|---:|---|
| **Core**: blanking it changes the game | 175 | 10.7% | the world |
| **Presumed core**: the rules read it, no tested window exercised it | 85 | 2.0% | the world, until a test exercises it |
| Setup fixed for the whole game (options, seat identities) | 35 | 0.1% | once per game, `Arc`-shared |
| Map geometry fixed at generation (terrain, hills, rivers, cliffs, continents, lowlands) | 6 | 12.5% | regenerated from the seed |
| Last-seen memory per seat (`remembered_tiles`, `remembered_cities`, `turn_units`, `turn_visible`) | 5 | 70.9% with fog tracking on | the seat's observer, not the world |
| History (events, turn stats, war chronicle, combat ledger, unit lifetimes, move trails) | 39 | 3.7% | an append-only record beside the world |
| Derived (rebuilt by the next turn boundary) | 11 | 0.1% | computed, not stored |
| Mirror of a real Civilization VI game (`observed_*`, `live_*`, host facts) | 48 | 0.0% in self-play | one optional mirror block |

On top of those, `Game` carries ~45 more mirror fields (`host_*`,
`blocked_*`) that saves already drop, and 28 runtime caches and indexes
(`#[serde(skip)]`, `occ`, `city_by_pos`, `log`).

**Sufficiency is proven on the games tested.** A save with every non-core
field blanked at once (30–48 fields per game) replays the recorded actions
to the identical world in all 8 games of round 2: no field of the rest of
the state differs, no action is refused, and every seat's legal action set
at the start of the first and middle round of the window is unchanged.

**Necessity is per field.** Each of the 175 core fields, blanked alone,
changed some other rule-bearing field, got an action refused, changed a
seat's legal actions, or stopped the save from loading or replaying. Ten of
them (denouncement timers, friendship and open-border expiries, diplomatic
favor, incidents, missions, victory points, governor titles, a spy's ready
turn, a pending deal's sender) changed **only** the legal action set: a pure
replay of the actions the agents took could not see them.

### The core, by container

- **World clock and randomness**: `turn`, `rng`, `next_id`, `next_deal_id`,
  `world_era`, `world_era_since`, `world_era_countdown_end`,
  `meteor_strikes`, `retired_great_people`, `great_person_offer_costs`.
- **Map overlay** (per tile, over the generated geometry): `feature`,
  `resource`, `improvement`, `district`, `district_foundation`, `wonder`,
  `owner_city`, `road`, `pillaged`, `drought`, `disaster_food`,
  `disaster_production`, `disaster_faith`.
- **Seats** (54 fields): research and civics (sets, current choice,
  progress, overflow, boosts), treasury (`gold`, `faith`,
  `strategic_resources`, `bankruptcy_amenity_penalty`, `co2_emissions`),
  government and policies, religion (`pantheon`, `religion`,
  `religion_beliefs`, `holy_city`), great people (`gpp`, `gp_claimed`,
  `great_people`), ages (`era_score`, `era_triggers`, `dedications`, `age`,
  `past_dark_ages`, `past_golden_ages`), diplomacy (`met`, `grievances`,
  `alliances`, `friends_until`, `open_borders_until`, `denounced_since`,
  `denounced_until`, `diplomatic_missions`, `diplomatic_incidents`,
  `diplomatic_favor`, `dvp`, `envoys`, `envoys_free`, `influence`,
  `quests`), governors (`governors`, `governor_roster`,
  `governor_titles_spent`), culture (`culture_lifetime`, `tourism_lifetime`),
  exploration (`explored`, `discovered_natural_wonders`), `alive`, and
  `counters` (see the leaks below: it mixes engine tallies with agents'
  notes).
- **Cities** (26): identity and ownership (`id`, `pos`, `owner`,
  `original_owner`, `is_capital`, `occupied_from`, `occupation_grievance`),
  growth (`pop`, `food`), production (`production`, `production_progress`,
  `queue`, `strategic_resource_commitments`), what stands
  (`buildings`, `pillaged_buildings`, `districts`, `wonders`, `owned_tiles`,
  `border_culture`), defence (`hp`, `wall_hp`, `encampment_wall_hp`),
  loyalty and faith (`loyalty`, `pressure`, `atheist_pressure`),
  `great_person_foreign_route_gold`.
- **Units** (21): `id`, `type`, `owner`, `pos`, `hp`, `moves_left`,
  `attacks_left`, `acted`, `charges`, `xp`, `level`, `promotions`,
  `xp_bonus_pct`, `formation`, `linked_to`, `religion`, `fortified`,
  `fortify_turns`, `free_upkeep`, `levied_from`, `levied_until`.
- **Everything between seats**: `wars`, `at_war`, spies (9 fields), trade
  routes, pending and active deals, Congress resolutions and their effects,
  emergencies, barbarian camps and their scouts, raiders, targets and alerts,
  storms, droughts, fires.

### Presumed core

85 fields were never exercised by a tested window (empty in every save, or
untouched by the rules in 40 turns) but are read by rule code, so they stay
in the world until a test exercises them: among them `peace_treaties`
(war-declaration legality), `war_weariness`, the age thresholds,
`past_governments`, `tourism_pressure`, `great_work_pieces`, the space-race
and mass-driver state, citizen locks and focus, defensive pacts, promises,
anarchy, competitions, flood and fallout state, and the per-turn strike
flags. A seat-local game mode, a human seat or a simultaneous-turn world
exercises fields this probe never saw.

## Leaks the probe found

1. **The action log is not a complete record.** Replaying the log from setup,
   or the window from a save, reproduces every field of the played game
   except `players[*].counters`, in every game tested. Stock-agent modules
   write their own bookkeeping straight into the game state: `commit:*`
   (commitments made, completed, abandoned, late), `raid_prize:*`,
   `raid_wars` (`src/ai/advanced/commitments.rs`, `diplomacy.rs`,
   `opportunistic_war.rs`, `city_campaign.rs`,
   `city_state_before_the_first_capital.rs`). The engine uses about 15
   literal keys of the same map for rule tallies. Agents' notes belong in the
   agent.
2. **Loading a save can hand a player the favor they already spent.** The
   loader migrates pre-trade saves by copying `counters["diplomatic_favor"]`
   into `diplomatic_favor` whenever the balance is exactly `0.0`
   (`impl From<GameSer> for Game`). The engine still adds every grant to
   that counter (`boosts.rs`, `actions/diplomacy.rs`), so it is a lifetime
   total. A seat that spent all its favor gets its lifetime earnings back on
   load (seen in seed 207). The migration needs a save-version gate, like
   `visibility_memory_version`.
3. **Load is not the identity on `wars`** when the narrated war ledger is
   stale (headless play runs with `set_war_ledger(false)`): seed 204's `wars`
   differ between the save and the loaded game.

## How we get there

Each step keeps the game byte-identical, and the probe is the gate.

1. **Lock the boundary.** Promote the probe to tests: a save replays to the
   played game; the save with every non-core field blanked replays to the
   same world; a manifest names each field's layer, and a field missing from
   it fails the test, so a new field must declare whether the rules read it.
   Fix the three leaks.
2. **Move the mirror out.** The ~100 `observed_*`, `host_*`, `blocked_*`
   and `live_*` fields become one `Option<Box<HostMirror>>`, `None` in
   self-play.
3. **Move history out.** Events, turn stats, the war chronicle, combat
   ledgers, unit lifetimes and move trails become an append-only `Chronicle`
   the engine emits into and observers read; the world never reads it back.
4. **Move last-seen memory to the seat.** Fog memory is each seat's belief
   about the world, the input to its observation function, not world state.
   It is 71% of a save's value bytes when tracked.
5. **Compute the derived fields.** Rebuilt every turn boundary, so they
   need no storage or save field.
6. **Take setup out of the clone.** Options, seat identities, rules and the
   generated map geometry go in one `Arc<Setup>`; tiles carry only the
   dynamic overlay; a save is the seed, the options and the overlay.
7. **What is left is `World`**: the core plus presumed core, the only thing a
   clone, a search branch, a debugger timeline node, a hash or a save needs
   to hold.

## Limits

Evidence, not proof. Round 2: 7 usable games (2, 4, 6 and 8 majors, Online
speed, Continents, stock agents, two with fog memory forced on), 40-turn
windows from turns 60 to 205, legal sets sampled at two rounds per window. A
field called inert here is inert in these windows; rule code that reads it
keeps it presumed core. Untested systems include human seats, simultaneous
turns, game modes, mods, and the mirror of a live Civilization VI game,
which is what most of the mirror fields exist for.

## Rerun

```
cargo build --profile ci --bin civvis
python3 tools/core_probe.py --civvis target/ci/civvis --budget-minutes 30
```

Every row is printed as a `CPROBE {json}` line.

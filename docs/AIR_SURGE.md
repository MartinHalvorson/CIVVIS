# The air surge

`air-surge` is an opt-in heuristic gene: beeline Advanced Flight from three
technologies out, raise an Aerodrome and a bomber wing, and take a named rival
city with the cavalry behind it. Off in production
(`PRODUCTION_OPT_INS`); `gene_screen --genes air-surge` prices it.

Implementation: `src/ai/advanced/air_surge.rs`. Tests:
`src/ai/advanced/tests.rs`, the `air_surge` block.

## The gap it fills

The controller had no route into the air layer at all, and the reason is
structural rather than accidental:

- `choose_war_plan` — the one place that appoints a package around a coming
  technology — filters its candidates to `is_melee_capable()` units and
  explicitly skips `domain == "air"`. No air unit can be an assault body or a
  breach element there.
- The same function ranks its unlocks by **cheapest remaining research first**.
  It can therefore appoint the *next* technology and never a chain, so a
  three-step beeline is not expressible in it.
- Nothing else asked for an Aerodrome. `src/ai.rs` scored one at 135 — but
  that is the *air-pillage target* table, valuing a rival's airfield as
  something to bomb. No production path ever built one.

Meanwhile the engine implements the whole layer: `Action::AirStrike`,
`Action::AirPillage`, `Action::AirRebase`, interception
(`resolve_air_interceptions`), and `air_slots` on both the City Center (1) and
the Aerodrome (2). The tactical layer already flies what it is given —
`UnitDoctrine::AirStrike`, `ForceRole::AirStrike`, `air_strike_value`. Every
missing piece was upstream of the first Bomber.

## The chain

Read off `data/`, not chosen:

| step | unlocks | note |
|---|---|---|
| `flight` (1250, era 5) | Aerodrome | `industrialization` + `scientific_theory` |
| `radio` (1370, era 5) | **reveals Aluminum** | requires `flight` |
| `advanced_flight` (1480, era 6) | Bomber, Fighter | requires `radio` |

The Bomber is `bombard_strength` 110 at range 10, `siege: true` — full damage
against walls — and costs 1 Aluminum to train and 1 a turn to keep. Two
consequences follow directly:

- **Aluminum is revealed exactly one technology before the unit that needs
  it**, so the appointment cannot price the metal up front. It waits
  `AIR_SURGE_ALUMINUM_GRACE` standard turns after the breakthrough and then
  gives the wing up. Mining needs no new code: `BasicAi::builder_step` already
  takes an unopened strategic deposit ahead of every other tile at any
  distance, and `best_improvement` prefers the improvement that works a tile's
  resource.
- **A Bomber cannot take a city, only empty one.** The capture bodies are the
  light and heavy cavalry lines by preference: 4–5 movement turns an emptied
  city into a captured one inside the window the wing keeps it emptied, and
  `cavalry: true` ignores the zone of control that the melee package
  negotiates tile by tile. With neither Horses nor Iron the surge falls back to
  the strongest melee body it can build and records that it did.

## ⚠ Reachability: measured, and it is the headline

`air_surge_census_at_deployment_scale` plays the shipped shape (six players,
74×46, 300 turns) with the gene on and samples the distance to Advanced Flight
every turn. **The seat never gets there.**

| seed | closest approach | at turn | techs at end | affordable? |
|---|---|---|---|---|
| 941200 | 7 techs | 258 | 33 | never |
| 941201 | 6 techs | 285 | 34 | never |
| 941202 | 9 techs | 179 | 26 | never |
| 941203 | 15 techs | 147 | 18 | never |

Every seed is short of the same branch — `industrialization`,
`mass_production`, `square_rigging`, `steam_power`, then `flight`, `radio` —
not of the air chain. At the closest approach the remaining chain still cost
**79 to 278 turns of research** at the seat's own science, against a 300-turn
game. The gene is therefore **inert at the shipped deployment shape**, and it
is inert for the right reason: `air_surge_affordable` refuses an appointment
that cannot finish, rather than spending the last forty turns of a game
researching a unit it can never build.

This is a science result, not an air result. It belongs with the recorded
science-lag lane — the empire peaks at three to five cities, and Advanced
Flight sits twenty-five technologies deep on its own branch. **Do not read a
flat screen as "the wing does not help".** Run the census first: it separates
"the gene lost" from "the gene never fired", which a paired screen cannot.

### The same census at 600 turns

`CIVVIS_AIR_SURGE_TURNS=600` gives the seat long enough to reach the air, and
on the one seed that does, the whole pipeline runs:

| seed | closest | appointments | breakthroughs | peak field / Al / bombers | declared | captured | end |
|---|---|---|---|---|---|---|---|
| 941200 | **0** at t341 | 4 | 3 | 1 / 2 / **3** | 1 | **1** | 9 cities, score 690 |
| 941201 | 16 at t156 | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 4 cities, score 270 |
| 941202 | 7 at t172 | 1 | 0 | 0 / 0 / 0 | 0 | 0 | 6 cities, score 283 |
| 941203 | 8 at t330 | 1 | 0 | 0 / 0 / 0 | 0 | 0 | 0 cities, score 232 |

Seed 941200 is the end-to-end proof: beeline → Aerodrome → two connected
Aluminium → **three Bombers** → declaration → the appointed city taken. It also
finished with the best score of the four. The other three never reached the
breakthrough at all (`breakthroughs 0`), so the wing was never possible there
— the same science ceiling as above, not an air defect.

⚠ Two things this census does **not** say, and a screen must:

- Seed 941203 ended with **no cities**. It held one appointment and never got
  the technology. Whether the beeline contributed to that collapse, or the seat
  would have been wiped anyway, is unmeasured — there is no control arm here.
  A diverted research plan is exactly the cost a paired screen exists to price.
- The 941200 seat ended with **zero** Bombers despite peaking at three. A
  Bomber costs 1 Aluminium a turn to keep and can be shot down; end-state
  counts cannot tell a wing that was never built from one that was spent. Read
  the peak, not the end.

## Lifecycle

One appointment at a time, on one objective city.

| phase | owns |
|---|---|
| `Beeline` | research forced along the chain |
| `Arm` | Aerodrome, then `AIR_SURGE_BOMBERS` bombers, then `AIR_SURGE_BODIES` cavalry; denounce runs alongside |
| `Strike` | the declaration |
| `Exploit` | the war, pressed on the appointed city |

The research goal is keyed off **the technology, not the phase** — a counter
(below) is in `Exploit` from its first turn and is exactly the appointment that
most needs the beeline.

The grand strategy is taken only from `Strike`: three technologies of Conquest
posture would pay for the wing with the economy that has to build it. A home
`Recovery` still outranks the appointment.

## Two ways in

- **The elective attack.** At peace, within three technologies of the Bomber,
  against the best campaign-valued city of a legal major that a Bomber based at
  home can strike and a land body can walk to.
- **The counter.** With exactly one major already at war with us, the surge
  arms against *that* civilization, has no declaration to make
  (`opened_at_war`), and skips straight to `Exploit`. This is the operator's
  "bombers are also useful for countering other civs": the empire that most
  needs a wing is the one already being invaded.

Two fronts shuts the window — that is a war the empire is losing, and a
three-technology beeline is not the answer to it.

## Stand-downs

Objective changed owner (a capture is recorded, not an abort), target dead,
diplomacy made the war illegal, someone else opened the war, peace closed it,
the launch slipped past `AIR_SURGE_ENDGAME_RESERVE`, no base left within the
wing's reach, home Recovery persisted for two reviews, no Aluminum after the
grace, victory denial superseded the surge.

⚠ **A stand-down sets a cooldown** (`AIR_SURGE_ABORT_COOLDOWN`). The lifecycle
ends by appointing whenever none is live, so without it an abort whose cause is
still true is undone by the call that recorded it and re-recorded every turn —
the census would count hundreds of stand-downs for one situation.

## Bounds, and what this is not

`air-surge` is a **capability**, priced on its own. It does not change which
victory the planner aims at, and it must not be read as reopening the
domination lane: `docs/eval/` records a pinned `live_target_domination` at
−319 Elo across 120 pairs, while converting 22 domination victories against
adaptive's 1. Conquest is mechanically reachable; *aiming* at it is what cost
the Elo. This gene adds the machinery and leaves the aim alone.

It also never competes with the melee appointment: `air_surge_open` refuses
while a `war_plan` is live and `may_form_war_plan` refuses while a surge is,
because two packages would bid for the same idle queues and the same
declaration. Both halves of that cross-gate are exact no-ops while the gene is
off.

## Measuring it

```bash
cargo run --release --bin gene_screen -- --genes air-surge
# the lane's own census, on four deployment-shape games:
cargo test --lib --profile ci air_surge_census_at_deployment_scale -- --ignored --nocapture
# and on games long enough to reach the air at all:
CIVVIS_AIR_SURGE_TURNS=600 cargo test --lib --profile ci \
    air_surge_census_at_deployment_scale -- --ignored --nocapture
```

The census prints appointments, breakthroughs, declarations, captures and the
abort histogram per seed, which is the fastest way to tell "the gene lost" from
"the gene never fired".

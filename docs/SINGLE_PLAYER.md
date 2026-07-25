# Single player

CIVVIS was built AI-simulator-first, and its browser client grew up around the
spectator: an exhibition of eight agents playing themselves, watched. The human
seat came along for the ride. `civvis play` has always seated a person at seat
0, and the engine has never been the limit — `Game::legal_actions(0)` already
enumerates policy cards, pantheons, governors, envoys, spies, trade routes,
Great People, casus belli and peace offers for that seat, and `observation()`
already ships the state each of those decisions needs.

What was missing was the client: a person could see almost everything and
decide almost nothing. This document is the plan for closing that, and the
record of the conventions the work follows so the pieces keep agreeing with
each other.

The reference point is Civilization VI, because that is the game this engine
models. Where Civ 6 and CIVVIS disagree on mechanics, `docs/MECHANICS.md` and
`docs/FIDELITY.md` win; this file is only about how a person *drives* them.

## The shape of a Civ 6 turn

A Civ 6 turn is a loop around one button. The player is never asked "what would
you like to do?" — the game tells them what it is waiting on, in priority
order, and the End Turn button is the thing that says it:

1. Something demands an answer (a captured city's fate, a proposed deal, a
   World Congress vote, an age dedication).
2. Something is unchosen (research, civic, production, a policy, a pantheon).
3. Something is unmoved (a unit that still has movement and no standing order).
4. Nothing is waiting — the button turns green and says **Next Turn**.

Pressing the button when it is *not* green does not end the turn; it takes the
player to whatever is blocking. That single rule is most of what makes Civ 6
playable without a manual, and it is what CIVVIS's turn loop implements.

Around that button sit notifications: a vertical rail of small, clickable
chips. A notification is not a log line. A log line records that something
happened; a notification is a thing the player may still want to *act on*, and
clicking it goes there.

### Blocker priority

The order the client resolves blockers in, highest first. It is the order the
End Turn button announces them, and the order `Enter` walks them.

| # | Blocker | Resolved by |
|---|---------|-------------|
| 1 | A captured city awaits its fate | keep / raze / liberate modal |
| 2 | A rival has proposed a deal | accept / reject in Diplomacy |
| 3 | The World Congress is voting | vote buttons in Government |
| 4 | A new age needs its dedication | dedication buttons in Government |
| 5 | A pantheon may be founded | Religion panel |
| 6 | A policy slot is empty | Government panel |
| 7 | No research is selected | Science card / tech tree |
| 8 | No civic is selected | Culture card / civics tree |
| 9 | A city is producing nothing | City command panel |
| 10 | A unit has moves and no orders | select the unit; move, fortify, skip or sleep |

Blockers 1–4 and 7–10 are engine-legal actions the client can already resolve;
5 and 6 arrive with the panels that resolve them, and a blocker is never shown
before the UI that answers it exists. A blocker is skippable — `Shift`+`Enter`,
or shift-clicking the button, ends the turn regardless — because a rule that
cannot be overridden becomes a trap the first time a player disagrees with it.

### Unit orders

A unit "needs orders" when it belongs to the player, has movement left, and has
neither an engine standing order (fortified, or sleeping through an air
patrol) nor a client-side one. Two client-side orders exist, matching Civ 6:

- **Skip** (`Space`) — done for this turn only. Cleared when the turn advances.
- **Sleep** (`Z`) — done until something changes: an enemy comes within two
  tiles, the unit takes damage, or the player selects it again.

Fortify (`F`) is an engine action and outlives both. The client never invents
engine state: a skipped unit is simply one the client stops asking about.

Turn start selects the first unit that needs orders and centres the camera on
it. Spending a unit's last movement point advances to the next one. `Tab`
cycles manually.

## What the client covers

The decision surface, grouped the way Civ 6's launch bar groups it. This table
is the ledger for the build-out; a row is "yes" only when a person can perform
the action from the UI without a debugger.

| Area | Engine actions | Client |
|------|----------------|--------|
| Turn loop | `end_turn` | yes — blockers, notifications, unit cycling |
| Unit orders | `move`, `move_to`, `attack`, `ranged`, `fortify`, `promote`, `upgrade`, `improve`, `pillage`, `repair_improvement`, air ops, religious ops | yes — contextual unit bar |
| Research | `research`, `civic` | yes — cards and both trees |
| Cities | `produce`, `buy` | partial — production and unit purchase; no queue, no building/district purchase, no district siting |
| Government | `government`, `slot_policy`, `unslot_policy` | yes — Empire ▸ Government |
| Religion | `choose_pantheon`, `found_religion`, `evangelize_belief` | yes — Empire ▸ Religion |
| Great People | `recruit_great_person`, `patronize_great_person` | yes — Empire ▸ Great People, though it cannot name the person on offer |
| Governors | `appoint_governor`, `assign_governor`, `reassign_governor`, `promote_governor` | yes — Empire ▸ Governors |
| City-states | `send_envoy`, `levy_military` | yes — Empire ▸ City-States |
| Trade | `trade_route`, `found_corporation`, `move_product` | partial — routes and corporations; `move_product` is still a unit order |
| Espionage | `assign_spy`, `spy_mission`, `promote_spy` | yes — Empire ▸ Espionage |
| Diplomacy | `declare_war`, `declare_war_with_casus_belli`, `make_peace`, `denounce`, `propose_deal`, `trade` | partial — Quick Deals and deal replies; no leader screen |
| World Congress | `congress_vote` | yes — Government panel |
| Ages | `choose_dedication`, `choose_secret_society` | yes — Government panel |
| Conquest | `keep_city`, `raze_city`, `liberate_city` | yes — modal |
| Setup | difficulty, leader choice | no — CLI flags only |

Rows marked "no" or "partial" are the remaining work, in roughly that order of
value to a player. The largest of them is diplomacy: a person still cannot
declare war, sue for peace or denounce anyone, so a whole victory path is
closed to them by the client rather than by the rules.

## The Empire panel

Civ 6 keeps its standing decisions on a launch bar. So does this: a rail of
icons down the map's inner edge, mirroring the notification rail on the other
side, each badged with the number of decisions waiting behind it. `G` opens
and closes it; `Escape` closes it.

Every screen behind that bar is a labelling layer and nothing more. It reads
`legal_actions`, names what it finds using `/rules`, and posts the action back
byte-for-byte. No screen constructs an action of its own, so no screen can
disagree with the engine about what is legal — which is also what keeps the
client inside the JSON-protocol rule in `CONTRIBUTING.md`.

The one thing a screen must never do is guess. The Great People screen does
not name the person each kind is currently offering, because that is a world
fact — it depends on which people every civilization has retired — and the
observation does not carry it. It shows the kind, the points and the count,
and names only the Great People already in your service.

## Keys

Chosen to match Civ 6 where Civ 6 has an opinion, and to leave the existing
camera and spectator keys alone.

| Key | Action |
|-----|--------|
| `Enter` | Resolve the next blocker, or end the turn |
| `Shift`+`Enter` | End the turn regardless of blockers |
| `Space` | Skip the selected unit's turn (or end the turn with nothing selected) |
| `Z` | Sleep the selected unit |
| `F` | Fortify the selected unit — with nothing selected, toggle the command deck |
| `Tab` | Select the next unit needing orders |
| `Escape` | Clear the selection |
| `P` | Civilopedia |
| `D` | Quick Deals |
| `A` | Hand the seat to the agent for a turn (`Shift` for ten) |
| `Y` | Tile yields |
| `1` `2` `3` | Next unit · appeal lens · tack a marker |

## Notifications

Each notification is `{kind, icon, tone, title, detail, pos, act}`. Blockers are
notifications too — they are simply the ones that also gate the button, and
they are pinned to the top of the rail with a gold ring.

Tones: `action` (gold, wants a decision), `good` (green), `warn` (amber),
`bad` (red). Categories map onto the engine's own event categories so the rail
and the event log never disagree about what happened.

Standing notifications are derived fresh from state on every render, so they
disappear the moment the thing they point at is resolved. Event-derived ones
are captured when the turn advances and live until dismissed or until eight
turns have passed, whichever comes first.

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
- **Travel** — clicking a tile outside this turn's movement is an order to go
  there, and the unit walks it over as many turns as it takes.

Travel cannot be built on `move_to`: `path_to` seeds its search with the
unit's *remaining* movement, so anything further is `"unreachable"`. `/route`
exposes `Game::route_step`, the long-range router the AI has always used —
it plans across future turns around mountains and choke points and returns the
first step, and the client sends a normal Move for that step, so the engine
stays the authority on whether the move is legal now.

A refused step does not end a journey. A unit with one movement point left
cannot enter a two-cost forest, and next turn it can; giving up on the first
refusal stranded units one tile short of where they were sent. The order ends
when the router has nothing to offer — arrival, or no route at all — or after
three turns with no progress at all.

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
| Cities | `produce`, `buy`, `buy_building`, `buy_district` | yes — city screen; there is no production *queue* because the engine has none (`do_produce` replaces `city.queue`) |
| Government | `government`, `slot_policy`, `unslot_policy` | yes — Empire ▸ Government |
| Religion | `choose_pantheon`, `found_religion`, `evangelize_belief` | yes — Empire ▸ Religion |
| Great People | `recruit_great_person`, `patronize_great_person` | yes — Empire ▸ Great People, naming the person on offer and why they cannot be claimed |
| Governors | `appoint_governor`, `assign_governor`, `reassign_governor`, `promote_governor` | yes — Empire ▸ Governors |
| City-states | `send_envoy`, `levy_military` | yes — Empire ▸ City-States |
| Trade | `trade_route`, `found_corporation`, `move_product` | partial — routes and corporations; `move_product` is still a unit order |
| Espionage | `assign_spy`, `spy_mission`, `promote_spy` | yes — Empire ▸ Espionage |
| Diplomacy | `declare_war`, `declare_war_with_casus_belli`, `make_peace`, `denounce`, `propose_deal`, `trade` | yes — Diplomacy screen (`L`), plus Quick Deals |
| World Congress | `congress_vote` | yes — Government panel |
| Ages | `choose_dedication`, `choose_secret_society` | yes — Government panel |
| Conquest | `keep_city`, `raze_city`, `liberate_city` | yes — modal |
| Setup | difficulty, leader choice, save and load | yes — Game settings ▸ Single player, with a leader, a difficulty and the server's saves |

Rows marked "no" or "partial" are the remaining work, in roughly that order of
value to a player. What is left is a production *queue*, which is an engine
gap rather than a client one: `do_produce` sets `city.queue = vec![item]`.

## The Empire panel

Civ 6 keeps its standing decisions on a launch bar. So does this: a rail of
icons down the map's inner edge, mirroring the notification rail on the other
side, each badged with the number of decisions waiting behind it. `G` opens
and closes it; `Escape` closes it.

It opens on **Cities**, because that is what a wide empire needs first: one
row per city with its six yields, what it is building and how long that has
left, when it next grows, and anything wrong with it — under attack, short of
amenities, out of housing, loyalty slipping. Cities waiting on an order sort
to the top and badge the tab, so the screen says how much is waiting without
being opened. Past three or four cities, clicking each one on the map to learn
the same things stops being navigation and becomes a chore.

Every screen behind that bar is a labelling layer and nothing more. It reads
`legal_actions`, names what it finds using `/rules`, and posts the action back
byte-for-byte. No screen constructs an action of its own, so no screen can
disagree with the engine about what is legal — which is also what keeps the
client inside the JSON-protocol rule in `CONTRIBUTING.md`.

The one thing a screen must never do is guess. Which named person each kind
is offering is a world fact — it depends on which people every civilization
has retired — so the client cannot derive it and must not try. The engine
ships it instead, in `me.great_person_offers`: the name, era, points
threshold, patronage prices, effects, and `blocked`, the reason it cannot be
claimed if it cannot. That last field matters more than it sounds — enough
points is not enough on its own (a Great Scientist wants a Campus, a Great
Writer wants an open Great Work slot), and a card showing 268 of 240 points
with no Recruit button reads as a bug until it says "requires 3 open art
Great Work slots".

## Setup

The browser's Game settings panel used to offer one mode — an AI-only
simulation — with "Single player · later" greyed out beside it. Single player
is now the default mode there, and it asks the two things a Civ 6 lobby asks
that this one could not: which leader you are, and how hard the rivals play.
Both selects are filled from the live ruleset, so a mod that adds a
civilization or a difficulty appears in them without a client change. Neither
is offered for an AI-only world, because there is nobody to hand them to.

Below them sit the saves this server is holding — every autosave and every
named one, newest turn first, each with the turn, leader and difficulty it
was written at. A build whose server has no save endpoints hides the group
rather than showing one that cannot work.

## Diplomacy

Quick Deals compares every trade the rivals would accept at once. The
Diplomacy screen (`L`) is the other half: one card per leader with their
agenda, their opinion, the grievances in both directions, and the four things
you can do that are not a trade — declare war (plain or with a casus belli),
sue for peace, denounce, and offer a pact. Proposals a rival has put to you
sit at the top, because they are the only part of the screen with a clock on
it.

City-states are listed below the majors with the meters that actually apply to
them — envoys, suzerain — and their own war and peace. Leaving them out made a
war with a city-state startable and never endable.

Barbarians are permanently at war with everyone and are not a power; they are
not listed and are not counted in the "at war with N" line.
## The city screen

The sidebar panel is the glance; the city screen is the desk. Left: what the
city *is* — six yields, growth, housing, amenities, loyalty, its districts,
buildings, wonders and citizens. Right: everything it *could* start, grouped
Districts, Buildings, Units, Wonders, Projects, each with its cost, its turns
at the current production, and a Produce or Buy button where the engine allows
one.

Two things it does deliberately:

- **A district names its tiles.** Where a district goes is most of what it is
  worth, so an item with more than one candidate site shows the sites rather
  than one arbitrary one.
- **It describes what it sells.** The ruleset carries no prose for units,
  buildings or districts, so the note is synthesised from the numbers —
  strength and movement, yields and housing, upkeep. A production list without
  that is a list of prices.

There is no production queue, and that is an engine fact rather than a client
gap: `do_produce` sets `city.queue = vec![item]`. Adding a real queue is engine
work.

## Endings

A game can end two ways for the person at the keyboard, and both used to
leave them on a live-looking map with an End Turn button that earned a red
error toast when pressed.

- **Somebody wins.** The button says the game is over and is disabled; the
  finale names the victor and the victory.
- **You are eliminated.** The world plays on without you — the engine answers
  your `end_turn` with "not your turn" — so the button says your civilization
  has fallen and the finale says so too.

Both offer the one thing still useful: another game, started from the setup
panel. A spectated finale keeps its countdown instead, because the supervisor
owns that handoff.

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
| Click a far tile | Travel there over as many turns as it takes |
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

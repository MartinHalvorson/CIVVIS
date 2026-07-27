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
| Auto-play | `POST /autoplay` | yes — a league strategy and a turn count under the End Turn button |

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
is no longer "later", and the modes are listed in the order this project
values them:

1. **AI-only simulation**, the default. It is what the engine exists for and
   what the exhibition deployment runs, so it is what the panel opens on.
2. **Single player**, a person in seat 0 against the agents.
3. **Multiplayer · later**, still greyed out.

A world already on screen overrides that default: open the page on a running
`civvis play` and the select reads Single player, because a person playing a
game should not have to notice that the button beside their empire has quietly
started offering to replace it with a simulation.

Single player asks the two things a Civ 6 lobby asks that this one could not:
which leader you are, and how hard the rivals play. Both selects are filled
from the live ruleset, so a mod that adds a civilization or a difficulty
appears in them without a client change. Neither is offered for an AI-only
world, because there is nobody to hand them to.

One control on screen starts the next world, and which one it is follows the
world you are looking at rather than the mode you have just picked: the
specbar's restart button while a simulation is playing itself, the sidebar's
**Start new game** in a game somebody is playing. Choosing Single player
therefore renames the specbar control **Start Single Player Game** — pressing
it opens that game rather than another simulation. On the supervised exhibition
that is not a restart: every simulation is a fresh process on freshly built
code, but a human game takes the running process over in place, so sitting down
to play does not wait out a process handoff, and the supervisor then leaves
that game alone until it is over.

Below them sit the saves this server is holding — every autosave and every
named one, newest turn first, each with the turn, leader and difficulty it
was written at. A build whose server has no save endpoints hides the group
rather than showing one that cannot work.

## Auto-play

Unciv has an AutoPlay button and it earns its keep in two places: skipping a
stretch of a game that has already been decided, and watching how an agent
would play the position you are in. CIVVIS has a third, because CIVVIS is a
strategy laboratory before it is a game — you can hand your seat to a *named*
strategy off the league leaderboard and watch that one play it.

So auto-play is two choices rather than a modifier key, and both sit under the
button:

- **Agent** — every entrant still competing in the league roster, strongest
  first, by the handle the leaderboards give it and the rating it is
  defending. An entrant that has not finished a rated game is marked unrated
  rather than shown as an authoritative 1500. The seat's current agent is
  preselected, so pressing the button without touching this changes nothing
  about who plays; a choice is remembered for the next game.
- **Turns** — 1, 2, 3, 4, 5, 10, 20, 30, 40, 50, 100, 150, 200, 250, or All.
  "All" is the turns this game has left, not an unbounded loop.

`POST /autoplay` takes `{turns, strategy}`; `turns` may be the string `"all"`,
and the server bounds any count by the turns remaining. The roster comes from
`GET /rules` as `strategies`, alongside `seat_strategy`, the name of whoever
holds seat 0 now. A name the roster does not have is an error rather than a
quiet fallback — a player who picked a strategy and got a different one has
been lied to.

Two things the client does deliberately:

- **It plays one turn per request, and draws every one of them.** Somebody who
  asked for 250 turns wants to watch them happen, and a batch is turns nobody
  watches: the engine plays every turn in it, but only the state *after the
  last one* ever comes back, so a batch of ten draws one frame and discards
  nine. Each turn now gets its own request and its own `requestAnimationFrame`,
  because two states rendered inside one display refresh are composited into a
  single frame — the same one-turn-per-presented-frame rule the exhibition
  keeps. Throughput survives it: the next turn is requested *before* the
  current one is painted, so the engine plays turn N+1 while the browser draws
  turn N. Measured on this machine, that costs about a fifth of the raw turn
  rate late in a game (18 against 22 turns/s at turn 200) and buys every turn
  a frame instead of one in ten. Pressing the button again stops after the
  turn already in flight, which is drawn rather than dropped.
- **It says the seat is on loan.** While an agent plays, End Turn is disabled
  and reads "An agent is playing", and both selects lock. A lit control that
  quietly does nothing is worse than a disabled one that explains itself.

The roster is the committed league snapshot under `data/league` unless this
game is already being rated against another one, in which case it is that one
and the ratings shown are the ratings in play. Reading it is a labelling
concern only: nothing about auto-play seats a rival differently.

The agents the picker offers are agents. A person registered in the same
roster (below) is a player in it but never an entrant, so a seat can never be
handed to somebody who is not at a keyboard.

## You are a new player

Starting a single player game **registers a new player**. The seat is not one
of the agents already in the league: `League::active` — the entrants a league
schedules, breeds, retires and seats — leaves people out, `Session::ai_fleet`
skips every human seat when it deals entrants to civilizations, and
`league::register_player` appends a fresh row with its own handle, provisional
at the base rating until the game is decided.

It was the other way around, and both halves of that were wrong. A person wore
an entrant's handle and elo in the HUD, which was an identity nobody at the
keyboard had earned; and when the game finished, `record_league_result` filed
the result under that entrant, so an agent that never played the game
collected its rating. A league is only worth reading if the name beside a
result is the one that produced it.

What the person sees: their seat's row carries `player_username` (the handle),
`player_rated` (whether the registration reached a roster on disk),
`player_elo`/`player_elo_rd` and `player_games`. Every other seat keeps the
`ai_*` fields it always had. A player with no finished game reads *Unrated*
rather than an authoritative 1500, the same way the leaderboards mark a
provisional entrant.

Two boundaries worth knowing:

- **Only a game that is being rated writes anything.** Without
  `--league <dir> --league-record` there is no roster to join, so the handle is
  minted against whatever roster is loaded, names the seat for this game, and
  goes no further. `register_player` also refuses a roster a distributed league
  round has already snapshotted — a manifest is an immutable promise about who
  is playing that round — and that game then goes unrated rather than
  invalidating jobs already running on other machines.
- **A save carries the world, not the person.** Reloading one registers
  another new player; the save format has no room for an identity, and
  inventing one on load would be a guess about who picked the file up.

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

Both offer the one thing still useful: another game, on the settings currently
in the setup panel. **Start another game** counts itself down from ten seconds
and then starts, so a finished game does not sit on its result screen forever
waiting for a click; the button carries the count. Any click, key press or
scroll stops the countdown, because somebody who is still deciding has just
proved they are there — as does choosing one of the ways to keep the world
below. A spectated finale keeps the supervisor's countdown instead, because
the supervisor owns that handoff.

### Continue after victory

A finale with a victor also offers three ways to keep the same map, seed, and
empires from the turn on which victory was declared:

- **Take a look around** returns the world under the next-victory rule but
  pauses the exhibition before another AI turn can run. Resume whenever you
  are ready from the normal simulation controls. A human game already waits
  for its player's next action.
- **Continue** resumes immediately and stops when a civilization earns a result
  other than the exact result on the finale.
- **To infinity and beyond** resumes immediately and ignores every later
  victory.

The two resuming choices are named for what the person wants, not for the
stopping rule they select; each rule is spelled out on its button's tooltip.

All three choices remove the turn cap from the continued world.

- The result is not thrown away. It is recorded as the game's verdict, the
  turn readout gains a *Playing on* line, and the league rating that was
  written when the victory landed is never written again.
- The exact winner and victory path shown on the finale cannot immediately
  repeat. A genuinely later result can end the next-victory continuation;
  indefinite play suppresses all of them.

The offer is real on the exhibition too. Every result a browser can see is held
for ten seconds — the same countdown a single-player finale runs, and not
configurable in either place —
the countdown is published on the same state that first carries the winner, and
the supervisor re-reads the world after its cooldown: a continued world is not
retired. Continuing is not a setup setting, so its uncapped turn rule is never
carried into the next game.
Headless simulation — `civvis sim`, soaks, the league — has no result screen and
no viewer, and waits for nothing.

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
| `A` | Auto-play the selected agent for the selected turns — again to stop |
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

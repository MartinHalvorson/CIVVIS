# Single player

CIVVIS was built AI-simulator-first, and its browser client grew up around the
spectator: agents playing themselves, watched. The supervised exhibition now
rotates through 4–10 major seats and matching stock map sizes. The human seat
came along for the ride. `civvis play` has always seated a person at seat 0,
and the engine has never been the limit — `Game::legal_actions(0)` already
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
2. Something is unchosen (research, civic, production, a government, a policy).
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
End Turn button announces them, and the order `1` — Civ 6's own EndTurn
key — walks them.

| # | Blocker | Resolved by |
|---|---------|-------------|
| 1 | A captured city awaits its fate | keep / raze / liberate modal |
| 2 | A rival has proposed a deal | accept / reject in Diplomacy |
| 3 | The World Congress is voting | vote buttons in Government |
| 4 | A new age needs its dedication | dedication buttons in Government |
| 5 | No government is adopted and one is available | Government panel |
| 6 | A policy slot is empty and a legal card fits | Government panel |
| 7 | No research is selected | Science card / tech tree |
| 8 | No civic is selected | Culture card / civics tree |
| 9 | A city is producing nothing | the city on the map ▸ production panel |
| 10 | A unit has moves and no orders | select the unit; move, fortify, skip or sleep |

Government and policy prompts open the existing Government panel. They use
engine-legal actions, so an empty slot with no compatible card does not block
the turn. Pantheons remain clickable standing notifications in the Religion
panel. A blocker is skippable — `Shift`+`1`,
or shift-clicking the button, ends the turn regardless — because a rule that
cannot be overridden becomes a trap the first time a player disagrees with it.

### Unit orders

A unit "needs orders" when it belongs to the player, has movement left, and has
neither an engine standing order (fortified, or sleeping through an air
patrol) nor a client-side one. Three client-side orders exist, matching Civ 6:

- **Skip** (`Space`) — done for this turn only. Cleared when the turn advances.
- **Sleep** (`Z`) — done until something changes: an enemy comes within two
  tiles, the unit takes damage, or the player selects it again.
- **Travel** — secondary-clicking a tile outside this turn's movement is an
  order to go there, and the unit walks it over as many turns as it takes.

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
it. Spending a unit's last movement point advances to the next one. `.` and
`,` cycle the roster manually, and `N` takes the nearest one still waiting.

## What the client covers

The decision surface, grouped the way Civ 6's launch bar groups it. This table
is the ledger for the build-out; a row is "yes" only when a person can perform
the action from the UI without a debugger.

| Area | Engine actions | Client |
|------|----------------|--------|
| Turn loop | `end_turn` | yes — blockers, notifications, unit cycling |
| Unit orders | `move`, `move_to`, `attack`, `ranged`, `fortify`, `promote`, `upgrade`, `improve`, `pillage`, `repair_improvement`, air ops, religious ops | yes — contextual unit bar |
| Research | `research`, `civic` | yes — cards and both trees |
| Cities | `produce`, `buy`, `buy_building`, `buy_district` | yes — the city's panels on the map, with prices on purchases, a production queue the page keeps, a yield focus (`POST /city-focus`) and citizens placed by hand (`POST /city-citizen`) |
| Government | `government`, `slot_policy`, `unslot_policy` | yes — Empire ▸ Government |
| Religion | `choose_pantheon`, `found_religion`, `evangelize_belief` | yes — Empire ▸ Religion |
| Great People | `recruit_great_person`, `patronize_great_person` | yes — Empire ▸ Great People, naming the person on offer and why they cannot be claimed |
| Governors | `appoint_governor`, `assign_governor`, `reassign_governor`, `promote_governor` | yes — Empire ▸ Governors |
| City-states | `send_envoy`, `levy_military` | yes — Empire ▸ City-States |
| Trade | `trade_route`, `found_corporation`, `move_product` | partial — routes and corporations; `move_product` is still a unit order |
| Espionage | `assign_spy`, `spy_mission`, `promote_spy` | yes — Empire ▸ Espionage |
| Diplomacy | `declare_war`, `declare_war_with_casus_belli`, `make_peace`, `denounce`, `send_delegation`, `send_embassy`, `propose_defensive_pact`, `propose_joint_war`, `request_promise`, `demand_gold`, `propose_deal`, `trade` | yes — Diplomacy screen (`L`), plus Quick Deals |
| World Congress | `congress_vote` | yes — Government panel |
| Ages | `choose_dedication`, `choose_secret_society` | yes — Government panel |
| Conquest | `keep_city`, `raze_city`, `liberate_city` | yes — modal |
| Setup | shared world setup, difficulty, leader choice, save and load | yes — Game setup ▸ Single player, with the same world settings as an AI simulation plus a leader, a difficulty and the server's saves |
| Auto-play | `POST /autoplay` | yes — a built-in agent and a turn count under the End Turn button |

Rows marked "no" or "partial" are the remaining work, in roughly that order of
value to a player. Nothing a Civilization VI player decides in a city is
missing from the page now.

## The arrangement

Everything above is about *what* a person can decide. This is about where the
controls are, which turned out to matter just as much: the client grew up
around the spectator, so a person who sat down to play met a laboratory. An
Elo table ran across the top of the map, the arena's stats ran down the right,
and End Turn was the third control in a column of simulation settings on the
left. Every decision in the table above was reachable, and almost none of it
was where a Civilization VI player's hand already goes.

A world with somebody in seat 0 now wears that game's own arrangement. The
anchors are not remembered; they are read off the installed game's interface
definitions under `Base/Assets/UI`, and the code says which file each one
comes from:

| Civ 6 | Its anchor | Here |
|-------|-----------|------|
| `TopPanel.xml` | one 29px strip, yields left, turn and menu right | `#civtop` |
| `LaunchBar.xml` | horizontal, upper left, under the standings masthead: tech tree, civics tree, Government, Religion, Great People, Great Works, Climate, Governors, History | `#launchbar` |
| `PartialScreenHooks.xml` | horizontal, upper right, read right to left: Reports, Era progress, Trade routes, City-States, Espionage — then Diplomacy, Quick Deals and ☗ | `#hooksbar` |
| `WorldTracker.xml` | under the launch bar: research, then civics | `#worldtracker` |
| `MinimapPanel.xml` | `Anchor="L,B"` | `.minimap-frame` |
| `ActionPanel.xml` | `Anchor="R,B"`: End Turn, with what it is waiting on and the choices that settle it | `#actionpanel` |
| `UnitPanel.xml` | `Anchor="R,B" Offset="172,0"`, inboard of the corner | `#ubar` |
| `NotificationPanel.xml` | `Anchor="R,B"`, stack growth up out of the corner | `#notify` |

The switch is one class, `body.civ6-frame`, set from the engine's own
`spectate` flag so it cannot drift from what the server thinks; everything
else is stylesheet. Nothing in the layer decides anything — every control
posts exactly the action it posted before, so the rule that no screen
constructs an action of its own still holds.

One seat reaches it: `body.playing-solo`, a person holding seat 0. A watched
simulation keeps the laboratory — the section below this one is why.

Four choices in it are worth their reasons.

**The top strip is this empire's yields, not the standings.** Science and
Culture carry a rate alone, Faith and Gold a balance with its rate beside it,
and Tourism appears only once there is some — which is exactly how
`TopPanel.lua`'s `RefreshYields` divides them, down to `YieldButton_SingleLabel`
against `YieldButton_DoubleLabel`. Then trade routes against capacity, unspent
envoys, Diplomatic Favor, and the resources this empire actually controls. The
turn, the era and its score sit on the right with the Civilopedia and the
menu, where Civ 6's `RightContents` stack puts them.

**The two trees lead the launch bar, each ringed by its meter.** They are the
first two hooks in `LaunchBar.xml` and they were the two screens this client
had no hook for at all: research was a `<select>` in a settings drawer. The
ring is the fraction of the current study that is done, so a glance at the bar
answers "how far into this tech am I" without opening anything.

**A played world opens on the map.** Rankings (☗) opens the standings and
arena statistics; the menu opens the command deck with setup, saved games,
auto-play and display settings. End Turn, unit orders, research and the
launch bar stay on the map. A watched simulation keeps its reports visible.
This follows the shipped `Base/Assets/UI/InGame.xml:52`, where
`WorldRankings` starts with `Hidden="1"`.

Explicit layout choices are remembered. The old rankings preference was
also written during initialization, so its default "open" could not be told
apart from a person's choice. The new rankings preference starts fresh and
is written only on a toggle. The deck's existing explicit preference survives.

**The turn is played from the corner.** End Turn stands in
`ActionPanel.xml`'s bottom-right corner, and above it the corner says what
the turn is waiting on and lays out the choices that would settle it: the
techs that could be researched, the civics, what an idle city could build,
the units still wanting orders, a proposal's Accept and Reject, a Congress
vote, an age's dedications. Every one of them is one of the engine's own
legal actions, posted exactly as the tree, the city screen and the deck
already post it — the corner constructs nothing and can decide nothing — and
the last row opens the whole screen for the choices that did not fit. The
selected unit stands inboard of the corner, where `UnitPanel.xml` offsets it,
and the notification rail climbs from the corner to the foot of the bars. The
deck no longer carries End Turn at all: "Choose research" is read off the map,
where the research is.

**The launch bar and the hooks are that game's two bars.** The launch bar
runs the nine screens `LaunchBar.xml` runs — the two trees, Government,
Religion, Great People, Great Works, Climate, Governors, History — and the
hooks bar runs `PartialScreenHooks.xml`'s five, read right to left as that
game reads them: Reports (the Cities report `F8` already opened), Era
progress, Trade routes, City-States, Espionage, with Diplomacy, Quick Deals
and ☗ beyond them. Four of those screens did not exist before this: Great
Works (every piece the empire holds, and every slot its buildings and
wonders carry, by city — the observation now ships `great_work_pieces`),
Climate (the world's phase and points, this empire's share of the carbon,
and the storms and droughts in sight), History (the seat's own event log as
a timeline, newest turn first), and Era Progress (the age, the era score
against both thresholds, dedications taken and the one a new age is waiting
for). All four are tabs of the Empire panel, so they share its cards, its
badges and its keyboard.

☗ still folds the report away for a look at the map, and ☰ still folds the
deck. Both answers are kept, under keys of their own — `civvis-solo-rankings-v1`
and `civvis-solo-deck-v1` — and neither may touch `civvis-map-overlays-v1`,
which is the whole reason the report is a class rather than an overlay flag.

**End Turn says what is blocking it, in Civ 6's words.** The blocker priority
table above has always been right; what the button said about it was this
project's own phrasing. It now reads `CHOOSE RESEARCH`, `UNIT NEEDS ORDERS`,
`CHOOSE PRODUCTION`, `KEEP CITY?` — the `LOC_ACTION_PANEL_*` strings from
`Base/Assets/Text/en_US/InGameText.xml` — and `NEXT TURN` when nothing is
waiting. What CIVVIS knows and Civ 6 does not, which city is idle and how many
units are waiting, goes on a second line rather than displacing the phrase a
player already recognises. The notification chips keep the longer sentences:
a rail is read deliberately, and there the specific one is better.

Two smaller corrections came out of the same reading. The selected unit leads
with its name and the four numbers a player decides on — health, combat
strength, ranged strength, and the movement left of the movement it has —
where it used to read `warrior hp 100 · mp 2`, the same facts in the register
of a debugger. And a city opens on what it can build: the plot market was the
first group in that column, seventeen cards deep, and pushed Districts and
Buildings off the bottom of the screen, so the one thing a person opens a city
to do was the one thing they had to scroll for. It is a fold at the foot of
the column now, which is where Civ 6 keeps buying land.

**Every node says what would boost it.** The ruleset carries
`boost: {trigger, count}` on every tech and civic, and the client had been
showing the bolt only *after* the boost landed — the one moment the
information is worthless. Both trees and the world tracker now print the
requirement in Civ 6's own register: "Kill 3 barbarians", "Build a University
next to a mountain", "Meet another continent". The 108 triggers the ruleset
ships are a handful of prefixed families over a short list of bare ones; a
trigger added to the engine tomorrow falls through to its own name and its
count rather than disappearing, and a test fails if any shipped boost lands
there.

A battlefield keeps none of this. An arena has no empire behind it — the
economy is a fixed grant, nothing is built or worshipped — so the strip keeps
the turn and the era, which a battle does have, and drops the rest rather than
printing six zeroes about a civilization that is not there; the world tracker
and the two tree hooks go with them. It is the same test the standings columns
already use, `worldStandingsInPlay()`.

Auto-play stays in the deck, under Start new game. It is this project's own
addition and a real one — handing your seat to a *named* agent is most of why
a laboratory has a play mode at all — but it is not a thing a player reaches
for every turn, so it does not compete with End Turn for the corner. Its
button says how long the loan is, "Auto-play 10 turns", and follows the
picker beside it; "All" is the turns the game has left, and a game with no
cap runs until stopped.

## The watched world keeps the laboratory

The arrangement above is a *seat's*. A simulation being watched does not get
it, and #2382 — which gave the watcher the same frame and put the standings
masthead and the arena rail behind ☗ with it — is reverted.

The reason is what the two seats are for. A person holding seat 0 is playing a
game, and Civilization VI's answer to where the controls go is the right one
for them. Somebody watching a simulation is reading an experiment: the
standings table, the victory tracker and the arena's records *are* what they
opened the page for. Putting them behind a button did not tidy the screen; it
hid the instrument, and the map it made room for is the one thing on that page
that was never in short supply.

So a watched world wears what it wore before: the standings masthead across
the top, the arena stats and victory tracker down the right, the command deck
open on the left with the transport in it, and the map in what those three
leave. `civ6Frame()` is `playingSolo()`, and everything that was the watcher's
alone has gone with the class it hung off — `#diploribbon` and its leader
discs, `arrangeWatchHud`, and the two remembered answers under
`civvis-watch-*`. `a_watched_simulation_keeps_the_laboratory` is the test, and
what it asserts is absence: a spectator-only element that nothing paints is
worse than no element, because it reads as a feature to whoever finds it next.

One thing the ribbon did that the masthead had better keep doing: it was how a
watcher took a seat. It still is — the standings table's own **Watch as**
button on each row, which is the action the discs were posting.

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

The browser's Game setup panel used to offer one mode — an AI-only
simulation — with "Single player · later" greyed out beside it. Single player
is no longer "later", and the public modes are listed in the order this
project values them:

1. **AI-only simulation**, the default. It is what the engine exists for and
   what the exhibition deployment runs, so it is what the panel opens on.
2. **Single player - WIP**, a person in seat 0 against the agents.

Multiplayer is not listed for now.

The bridge to a real Civilization VI game is retained as a hidden `civ6` option
for browser verification; it is not a public left-panel choice. When
verification selects it, it starts a game on the computer serving the page and
hands the human seat to `tools/civ6_play.py`. It re-points the panel at the
other game's settings, and every row it does not carry is hidden rather than
shown and dropped. See [CIV6_GAME_MODE.md](CIV6_GAME_MODE.md).

A world already on screen overrides that default: open the page on a running
`civvis play` and the select reads Single player, because a person playing a
game should not have to notice that the button beside their empire has quietly
started offering to replace it with a simulation.

Single player starts from the same world setup as an AI-only simulation:
ruleset, leader pool, teams, world shape and climate, map, size, city-state
count, start and future eras, speed, optional turn cap, victory conditions and
an optional reproducible map seed. Leaving city-states or the turn cap blank
uses the normal value for the selected size or speed; leaving the seed blank
asks for a fresh map. The running world's seed is shown back in the field, so
the ordinary Restart action can reproduce it exactly; clearing it deliberately
asks for a new roll. Those are world facts, so changing mode never drops them.
They are hidden only for the separate Firaxis-Civ-6 control mode, where that
game cannot carry them.

On top of those shared choices, single player asks the two things a Civ 6 lobby
asks that this one could not: which leader you are, and how hard the rivals
play. Both selects are filled from the live ruleset, so a mod that adds a
civilization or a difficulty appears in them without a client change. Neither
is offered for an AI-only world, because there is nobody to hand them to.

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
built-in agent and watch that one play it.

So auto-play is two choices rather than a modifier key, and both sit under the
button:

- **Agent** — the built-in agents, each by its handle. Nothing rates them
  (the league left in #2357). The seat's current agent is preselected, so
  pressing the button without touching this changes nothing about who plays;
  a choice is remembered for the next game.
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

The roster is the built-in agent list compiled into the binary, so it is the
same wherever the program was started from. Reading it is a labelling concern
only: nothing about auto-play seats a rival differently.

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
agenda, directional relationship state, grievances in both directions,
missions, visibility, and defensive-pact expiry. It exposes Delegations,
Resident Embassies, promises, Gold demands, Defensive Pacts, Joint Wars,
denouncements, peace, ordinary deals, and every available casus belli.
Gold demands appear only when the relationship is unfriendly or either leader
has denounced the other. A promise appears only after the counterpart has
settled nearby, converted one of your cities, spied on you, or attacked a
city-state you suzerain.
Proposals a rival has put to you sit at the top, because they are the only
part of the screen with a clock on them.

City-states are listed below the majors with the meters that actually apply to
them — envoys, suzerain — and their own war and peace. Leaving them out made a
war with a city-state startable and never endable.

Barbarians are permanently at war with everyone and are not a power; they are
not listed and are not counted in the "at war with N" line.
## The combat forecast

Every attack is a decision about two numbers, so hovering what the selected
unit can strike shows what the blow would do before it is struck: both
strengths, the health each side would keep, the range the roll can land in, and
a plain verdict. `POST /forecast` reads the exchange the resolver itself uses
(`Game::combat_forecast`), so a forecast cannot drift from the blow. It answers
only for a strike `legal_actions` offers the seat right now, which is what stops
it reading a unit through the fog, and it answers nothing for an Encampment
rather than something approximate.

## The city on the map

Civilization VI opens a city on the map, not on a screen over it, and so does
this. Selecting a city docks two panels at the edge of a world that stays in
view and in play:

- **The production panel** runs down the right edge above End Turn
  (`ProductionPanel.xml`). Everything the city could start is one column of
  rows grouped Districts, Buildings, Wonders, Units, Projects, each with its
  turns at the current production and its cost. The whole row is the button.
  What the treasury can buy outright is a second page of the same panel, and a
  third once Faith can buy something here, each purchase with its price.
- **The city panel** stands where a selected unit's panel does
  (`CityPanel.xml`): the six yields, growth and production meters, housing,
  amenities, loyalty and the governor, with what already stands in the city
  behind a City details fold. The arrows step to the previous and next city.

While a city is open its own tiles show their yields, with the tiles its
citizens work ringed, whether or not the yield icons are on for the whole map.

Three things it does deliberately:

- **An order closes the panels; a purchase leaves them up.** Choosing
  production answers the city and the turn moves on. A treasury is often spent
  on more than one thing.
- **Anything chosen by where it goes is chosen on the map.** A district or
  wonder with more than one possible tile, a district bought outright, and a
  plot of land: the panels step aside, the candidate tiles are outlined, and a
  plot carries its price and what it would yield. Buying land stays up for the
  next tile. Escape goes back to the city.
- **It describes what it sells.** The ruleset carries no prose for units,
  buildings or districts, so the note is synthesised from the numbers —
  strength and movement, yields and housing, upkeep. A production list without
  that is a list of prices.

The city is not a modal: the map's keys work under it, Escape or a press on
open country puts it down, and selecting a unit takes the corner back. A stage
too narrow to dock against keeps one stacked sheet.

**The queue.** Civilization VI lets a city be told what to build after what it
is building. The engine holds one item per city (`do_produce` replaces the
queue), so the rest of the list is a standing order the page keeps and
re-issues, the way a travel order is. With Add to queue on, or Shift held, a
row goes behind the current build instead of replacing it; a district chosen
for the queue keeps the tile it was given. At the top of a turn an idle city is
ordered the next thing on its list that the engine still offers, and an item it
no longer offers is dropped. The list is kept with the browser's saves for the
one game it belongs to.

**The focus.** Each yield on the city panel is also its focus: press it to
favour it, again to disfavour it, again to leave it to the governor. The
governor is the engine's own (`citizen_strategy`); `POST /city-focus` writes
the seat's wish into `Player::citizen_focus` and the citizens are reassigned
at once. That is kept apart from the per-city directive an agent writes,
which an agent rebuilds on every turn it plays: a focus set by hand outlives
handing the seat to auto-play. A favoured yield outranks every appetite the
governor reaches by itself, the panel says what each press did, and a focus
cannot starve a city.

**Placing a citizen.** Manage citizens, on the city panel, shows the city's
land with a citizen on every worked tile. Pressing a tile places a citizen
there and keeps them there (a white ring marks it); pressing it again
releases them. The engine seats placed citizens first and its governor
arranges everyone else around them (`Player::citizen_locks`, written only by
`POST /city-citizen`). A placed citizen is not moved even to cover
consumption, as in Civilization VI, and a city cannot have more placed than
it has citizens.

A district with specialist slots is part of the same mode. It shows how many
of its slots are filled, and each press seats one more specialist there and
holds them (`Player::specialist_locks`); a press on a full district releases
them. Placed citizens and seated specialists together cannot outnumber the
city's citizens.

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
in the setup panel. The observer settings' **Between-game countdown** offers
**None**, **3s**, **5s**, and **10s** (the default). **Start another game**
uses that interval, so a finished game does not sit on its result screen
forever waiting for a click; choosing **None** starts immediately. Any click,
key press or scroll stops a running human countdown, because somebody who is
still deciding has just proved they are there — as does choosing one of the
ways to keep the world below. A spectated finale keeps the supervisor's
countdown instead, because the supervisor owns that handoff.

The public site does neither. A result there is somebody reading how their game
ended, so it holds until they move on, shows their own score, rank, cities,
population, technologies and wonders, and **Start another game** goes to Create
Game.

### Continue after victory

At the end of a simulation, the supervisor countdown remains in charge and
the result screen has two choices:

- **One more turn** resumes the same map, seed, and empires with no turn limit
  until only one civilization remains. Every other victory is ignored, every
  civilization left is at war with every other, peace is refused, and every AI
  plays for conquest as a total war: none falls back to a defensive plan, each
  aims at the nearest rival city, and every idle city trains soldiers while its
  income carries them. City-states and Free Cities do not count, and a team
  left together counts as one. **Restart sim** still ends it early. The button is
  not offered when one civilization is already left.
- **New Game** starts the next world immediately. Leaving the screen alone
  keeps the existing countdown behavior and starts that next world when it
  reaches zero.

A human finale retains three ways to keep the same map, seed, and empires from
the turn on which victory was declared:

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

The offer is real on the exhibition too. The selected between-game interval is
published on the same state that first carries the winner, and the supervisor
re-reads the world after its countdown: a continued world is not retired. The
browser remembers the selection and reapplies it after a supervised successor
starts on a fresh process. Continuing is not a setup setting, so its uncapped
turn rule is never carried into the next game.
Headless simulation — `civvis sim`, soaks, the league — has no result screen and
no viewer, and waits for nothing.

## Keys

The map is Civilization VI's own, read out of that game's `InputSettings.json`
rather than chosen: `1` ends the turn, `Space` skips a unit, `Z` sleeps it, `V`
puts it on alert, `F` fortifies it, `B` founds a city, `E` sends it exploring,
`.` and `,` walk the units that still want orders, `]` and `[` walk the cities,
`T` and `C` open the trees, `D` opens Quick Deals, and the screens Civ 6 keeps
on function keys keep them. The lenses run `2` through `0` exactly as they do
there. [CIV6_KEYBINDINGS.md](CIV6_KEYBINDINGS.md) is the whole table, and the places
the map cannot be that game's: `A` is Attack there and CIVVIS attacks by
pointing, so Alert sits on `V` where that game has it; F5, F6, F11 and F12
belong to the browser; and Tab is how somebody navigating by keyboard reaches
every control on the page, so the board does not take it.

Two rules the table does not show:

- `1` resolves the next blocker rather than ending the turn, in the priority
  order above. `Shift`+`1` ends it regardless, because a rule that cannot be
  overridden becomes a trap the first time a player disagrees with it.
- Secondary-clicking a tile outside this turn's movement is an order to travel
  there over as many turns as it takes.

This section used to promise `Enter`, `Space`, `Z`, `Tab`, `P`, `D`, `L` and
`Y`, and an `A` that meant auto-play. **None of those keys were bound to
anything**: the client's map had five entries and the ones a player would
actually reach for were not among them, though every handler behind them
already existed. They are bound now, and this document and
`CIV6_KEYBINDINGS.md` describe the same map.

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

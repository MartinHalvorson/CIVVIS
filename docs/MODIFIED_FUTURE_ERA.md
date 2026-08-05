# The Modified Future Era

Status: **shipped, off by default**. The lobby's Future Era setting selects it;
`--future-era modified` selects it from the command line; and
`--mods mods/modified-future-era` loads the same content as an ordinary mod
folder. A game that does not ask for it is byte-for-byte the game it was
before, and that is the first property this document is about.

## What it changes

The classic Future Era ends the space programme the way Gathering Storm does.
The Moon is a milestone: `launch_moon_landing` pays a one-off Culture bonus,
deepens the exoplanet survey, and is then over. Nothing about the Moon is a
*place* — it is drawn out there in the sky view at true scale, tiled like every
other solid body, and none of it is anything a civilization can reach into.

This era makes it one. The Moon spawns with ore on it the way the Earth does,
and a **mass driver** on its surface throws that ore down onto a tile you name.

| | |
|---|---|
| `mass_driver` | 1200 Production · Spaceport · Offworld Mission · after the Moon landing · repeatable |
| lunar aluminum | 200–320 units |
| lunar iron | 140–240 units |
| lunar uranium | 50–90 units |

## One Moon

The piles are rolled once per game, from the map seed, and they are **shared**.
Not a deposit per civilization: one body, one set of numbers, drawn down by
whoever gets a driver over it first. What a rival takes out is gone, and how
much is left is reported to everybody who can see the Moon at all — the race is
only a race if you can see it being lost.

That is the whole design. A private lunar deposit per civilization would be a
production bonus wearing a spacesuit; a shared one is a commons, and the reason
to build the second driver is that somebody else is building their first.

Two consequences fall out of it:

- **Depletion is permanent.** An exhausted pile pays nothing for the rest of
  the game and stops being offered as somewhere to aim. Uranium is the thin
  one on purpose: 50–90 units is a handful of Giant Death Robots' worth of
  upkeep, not an endless supply.
- **Nothing is drawn that cannot be held.** A civilization at its stockpile
  ceiling stops mining rather than spilling ore onto a full warehouse. Wasting
  a shared, finite resource on a cap check would make the ceiling into a way to
  deny it to everyone else by accident.

The ore does **not** scale with game speed. Everything a civilization buys with
a stockpiled yield does, and this deliberately does not: the Moon is a physical
quantity of rock, so a Quick game is a shorter race for the same Moon rather
than a smaller one.

## Aiming, and the two things a slug can be

`AimMassDriver { site, ore }` is a standing order, not a shot. It names one ore
and one landing site inside your own borders, and it holds until it is given
again. Until it is given, nothing falls — drivers with no orders are drivers
with no orders.

Each turn, every driver draws one unit of the aimed ore off the Moon and lands
it at the site, where it goes into the ordinary strategic stockpile under the
ordinary ceiling. Ground that stops being yours — a landing site taken with the
city that owned it — catches nothing until the drivers are aimed somewhere
else.

`MassDriverStrike { target }` throws the same slug at a tile instead. It is
bounded by the two things the delivery is bounded by and nothing else:

- **one shot per driver per turn**, and
- **one unit of the aimed ore out of the stockpile per shot** — the slug *is*
  the metal.

So a strike is a turn's cargo spent on a target rather than a separate
magazine, which is why a driver that fired is not punished twice: the next turn
it delivers as usual. There is no range. A rock leaving lunar orbit reaches
anywhere on a world this civilization has revealed, and a launcher on the
ground would be modelling the wrong end of the machine.

What it does when it lands is one hexagon of nuclear ground zero — 100 damage
to everything standing there, the improvement or district pillaged, a struck
city's Outer Defenses gone and a point of population with them — with no blast
radius and **no fallout**. It is a rock, not a device. It earns 50 grievances
and 2 war weariness where a detonation earns 150 and 10, and it is held to the
same legality rule a detonation is: you cannot drop one on a civilization you
are at peace with.

## How it is put together

The content is a **mod folder**, `mods/modified-future-era/`, holding the two
overlay files a mod would hold: `resources.json` gives three strategic
resources a `lunar` deposit range, and `projects.json` adds the project. The
engine embeds those exact bytes (`FUTURE_ERA_MODIFIED_FILES` in `src/rules.rs`)
so the lobby setting and the `--mods` path cannot drift apart; a test asserts
they agree.

`Rules::for_game` resolves the setting, merging the overlay onto
`Rules::active_values` — the shipped data *with any installed mod already in
it* — so a game played with both a mod and this era gets both. The result is
per-game and lives on the save, because the era selects the ruleset itself and
a match must reload on the rules it was started under.

Three things follow from doing it this way, and each is worth stating because
each was a way to get it wrong:

- **The shipped ruleset does not move.** `data/` is untouched, so
  `Rules::source_fingerprint` is unchanged for a classic game and no rated
  game or Elo binding is invalidated by any of this.
- **The world does not move.** The deposits are rolled after map generation
  and only for resources the ruleset gives a lunar range, so a classic game
  draws no randomness for the Moon and a seed makes exactly the map it always
  made. There is a test that walks every tile of the two to say so.
- **A classic save loads unchanged.** Every new field is `#[serde(default)]`,
  and an absent Future Era is the classic one, which is what every save written
  before this was played on.

## What is not here yet

- **The AI does not build one.** `AdvancedAi`'s space programme is the four
  victory projects; it will not queue a mass driver, aim one, or fire one, so
  an all-agent game in this era plays like a classic game with an untouched
  Moon. The actions are enumerated in `legal_actions` and the mechanic is
  exercised by tests, so this is a strategy gap rather than a missing rule.
- **The sky view does not draw it.** `moon_deposits` and the driver's aim are
  on the observation, so a client can show what is left of each pile and where
  the slugs are landing; nothing in `web/index.html` reads them yet.
- **The landing does not show as an event.** Deliveries are counted in
  `mass_driver_landings` but say nothing in the log; only strikes announce
  themselves.
- **A rated game cannot be played in it, and that is load-bearing.**
  `tournament_setup_contract` in `src/elo.rs` is the string an Elo ledger binds
  a rated game to, and it does not name the Future Era. That is correct today
  and only because `TourneyCfg` cannot ask for one: every tournament game is
  built from `GameOptions::new`, which is the classic era. The moment a
  tournament can select this era, that string has to gain the field — two games
  played under different rules must not pool into one rating — and adding it
  will move the contract for every existing record, which is a ledger migration
  rather than a one-line change.

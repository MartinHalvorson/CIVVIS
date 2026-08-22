# Builder safety in a Barbarian capture envelope

_2026-08-21 · `builder-barbarian-safety`_

## What failed

Live run `civvis-20260821T204930Z` gave a Builder the ordinary movement
path even though a visible Barbarian could take its destination on the next
hostile phase. On turn 17 the Builder and its Warrior stood together at
`(26,18)`; on turn 18 a Barbarian broke the Warrior and captured the Builder.
A replacement Builder was refused an improvement near the same raider on turn
22, then routed toward work and captured before it could complete the job.

`barbarian-hunt` made military units answer nearby raiders, but it did not
make Builder target selection or movement wait for that answer. Settlers
already had a capture-aware route check; Builders did not.

## The gene

`builder-barbarian-safety` is a native production opt-in, default off. For a
visible Barbarian military unit it reads `Game::threat_reach`, the engine's
fresh-turn, terrain- and zone-of-control-aware movement envelope. It excludes
air and engine-managed recon units, and it never turns an ordinary major-war
unit into a Builder embargo.

An opted-in Builder:

- retreats before spending a charge if it is already in direct capture reach;
- refuses a route step that enters that envelope, retaining its job while a
  responder clears the raider; and
- does not credit an unbound guard: it may be killed or take its own turn and
  leave before the Barbarian acts.

That restriction is the live ordering: the t18 guard died before the Builder
could be captured, and the ordinary unit order also gives an unbound guard a
turn after the Builder. A generic escort discount would simply recreate the
failure.

## Demonstrated benefit

The focused regression
`ai::advanced::tests::builder_barbarian_safety_rejects_and_escapes_a_barbarian_capture_envelope`
constructs a real Builder job one legal Barbarian move away.

| controller | result after the same Barbarian move |
|---|---|
| stock | Builder is captured |
| gene on | Builder remains owned by its civilization with all 3 charges |

It also proves that a major-war unit does not trigger the gene and that a
future-tile guard cannot falsely make the tile safe. This is the positive
result the gene is intended to deliver: one prevented capture and three
preserved improvement charges in the previously failing position.

## Whole-game screen

This safety proof is not a claim that every map scores better. I ran the
foldover screen in the Barbarian-response profile: 6 players, 60x38 Pangaea,
Online speed, 150 turns, 6 city-states, shuffled civilizations, all seats,
`domination,score`, repairs baseline and repairs field.

- The independent exact-envelope confirmation at seeds `73013400..=73013423`
  (24 map pairs / 48 games) read **−1.4pp win, z −0.44** and **−0.48pp score
  share, z −0.97**.

The result is unresolved by the screen's own threshold. Therefore the gene is
not promoted into the global default or the gene ledger. It is available by
name for Barbarian-exposed runs, where the causal regression above establishes
its specific safety improvement without pretending that the broad score
evidence is already decisive.

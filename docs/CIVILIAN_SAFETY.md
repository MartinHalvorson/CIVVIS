# `civilian-out-of-reach`: settlers and builders stay out of a barbarian's reach

One opt-in gene (`Kind::OptIn`, off until the screen prices it) with one
rule and three consequences.

**The rule.** Every turn, the seat draws the **barbarian reach**: the set of
tiles every known barbarian military unit could end its next move on — a
one-turn movement flood from where it stands, at its own movement points,
over the terrain it would pay for. A civilian standing on a reach tile at
the end of our turn, without a military unit on its tile, can be captured
before it moves again. So:

1. **Never step into reach.** A settler or builder choosing its next tile
   refuses any tile inside the reach — including its own target site when
   the site is inside it. It waits one tile out, or takes the safe detour.
2. **Flee when already inside.** A civilian whose current tile is inside the
   reach, with no guard stacked on it, moves before anything else: to the
   reachable tile outside the reach that keeps the most progress toward its
   goal (its site, its job, or home); failing any safe tile, to the tile
   farthest from the nearest barbarian and closest to friendly cover.
3. **The escort stacks.** A settler with a guard is safe on any tile — a
   military unit sharing the tile blocks capture outright — so the guard's
   job is to be *on* the settler's tile whenever the settler is in or beside
   the reach, not merely nearby. A threatened settler that cannot stay out
   of the reach *summons* the nearest healthy land military unit that can
   reach its tile this turn (a routed `MoveTo`; a guard that gets under way
   is bound too and finishes the walk on its own step), pulls the stacked
   guard along with every step, and the guard's own turn keeps it on the
   settler's tile. The bond is released when no raider is within eight
   tiles or the settler is gone. The doorstep case — arriving beside the
   site with no moves left, alone, surviving a barbarian turn before
   founding — is answered two ways: the step onto a site inside the reach is
   allowed only when the settler keeps movement to found this same turn
   (the city stands before the raider moves), and otherwise it waits one
   tile out or enters with its guard.

**Visibility.** The reach is drawn from the raiders inside the turn-start
vision frame, as every other civilian-risk path reads the board — never
through fog. The live bridge only ever exports what the seat sees, so this is
also what it can act on; and a zone of control counts: a raider that would be
stopped beside one of our units cannot reach past it, and `threat_reach`
says so.

## Why this and not more of what exists

`settler-threat-detour` (ships on) prices a dodge into the settler's step
score: a retreat is *allowed* when a tile is much safer. `builder-barbarian-safety`
holds a builder near threat. Neither draws the reach the barbarian actually
has, so a settler can be walked to a tile two hexes from a horseman with
four moves and the score never sees the capture coming. The live record is
the motivation: the doorstep captures of run `civvis-20260815T081505Z`, the
eight settlers taken in 104 turns on `civvis-20260821T130446Z`, and the
finding that runs which lose settlers end with half the cities.

The reach is the fact the operator named: *we know where the barbarians
are*. This gene turns that knowledge into a hard constraint on where a
civilian may stand, instead of a soft term it can be outbid on.

## Vocabulary

The **gene pool** is every gene, on or off (the registry,
`src/ai/advanced/genes.rs`); a **genome** is one player's set of on genes.
`civilian-out-of-reach` enters the pool off and is screened beside every
other gene (`docs/GENE_SCREEN.md`).

## What it does not do

- It does not change site selection or the settler's target.
- It does not send military units hunting barbarians (`barbarian-hunt`,
  `camp-party` own that).
- Off, every touched path is unchanged.

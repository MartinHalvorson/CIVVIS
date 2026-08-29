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

## What the live seat learned (2026-08-28): twenty-four captures in ten runs

The operator's standing rule, 2026-08-28: *every* settler a barbarian takes is
reconstructed turn by turn and the mechanism is fixed. The first day under the
rule read all twenty-four captures of the day's ten Civilization VI runs
(`tools/civ6_settler_captures.py` now writes the dossiers; run
`civvis-20260829T000643Z` alone lost eight). They fell into six mechanisms, and
two **host-only** treatments answer five of them. Host-only because the native
barbarian seat's recon cannot capture and its raids are already priced by the
screens: both are on for the Civilization VI seat through
`enable_live_bridge_universe`, inert on a native board, so no screened gene's
behaviour changes under them.

| captures | mechanism | answer |
|---:|---|---|
| 11 | the site chosen was beside a nest that had already taken a settler — "takes a site the preferred search refused"; a dead site was retired per settler, six turns | `live-settler-capture-lessons` (1): a settler that leaves the board with no city of ours within two tiles of where it last stood was **taken**; every tile within `SETTLER_CAPTURE_SCAR_RADIUS` (3) of that ground is dead for **every** settler for `SETTLER_DEAD_SITE_AVOID_TURNS` (30 standard turns) — `settler_site_is_dead` reads the scar, so the ranking, the exhaustion fallback and the target validation all refuse it, and the journal says "A settler was lost at …" |
| 6 | a barbarian **scout** two tiles away, exempt from every capture model on the claim that Firaxis' scouts never capture (`barbarian-scouts-are-scouts`); run `civvis-20260828T122324Z` shows one scout standing on our captured settler and taking three more | `live-barbarian-scouts-capture`: a barbarian recon unit counts in `barbarian_reach`, the turn-start hostiles, the unstacked-step capture test, the guard release and the route scorer's capture threats. The graded `settlement_tile_risk` keeps the exemption so the fourteen-turn freeze that gene fixed does not return |
| 6 | marching alone into fog: "The settler's guard stands down — no visible hostile within 8 tiles" fired the turn before a raider walked out of the fog, six times beside a camp the board knew about | `live-settler-capture-lessons` (4): the guard is not released while a known barbarian camp is within `SETTLER_ESCORT_THREAT_RADIUS` |
| 3 | a wounded or outclassed guard counted as protection (a 15-strength archer bound over a warrior; a guard at 12 HP "stands with its settler") | `live-settler-capture-lessons` (3): of the guards that can reach the settler this turn the **strongest** is summoned, not the nearest |
| 2 | "holds inside a barbarian's reach — no reachable tile is better": standing still beside a skirmisher, two tiles from a full-health archer (run `civvis-20260829T022749Z`, t78) | `live-settler-capture-lessons` (2): with no safe tile, a tile one of our military units holds outranks every bare tile — the settler walks onto it and binds that unit as its guard — and standing still loses to any tile farther from the nearest raider |
| 2 | "flees … out of reach" to a tile the raider reached anyway | open: the mirror's movement flood disagreed with the host's; the dossiers carry both positions for the next reading |

Tests: `a_barbarian_scout_is_a_capture_threat_on_the_live_seat`,
`a_settler_with_no_safe_tile_flees_onto_a_friendly_stack_under_the_lessons`,
`a_lost_settler_retires_the_ground_around_it_for_every_settler`,
`the_strongest_guard_that_can_reach_the_settler_is_summoned_under_the_lessons`.

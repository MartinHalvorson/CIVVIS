# Movement: what a step is allowed to do

This document exists because `docs/MECHANICS.md` carried movement as a single
green row — *"MP paid up front, min-1-tile, river +2 MP ✅"* — and one of the
rules that row does not name was wrong for the whole life of the project.

Every rule below names the test that pins it. A rule with no test is not a
rule here; it is a sentence with good posture.

## The rule that was wrong

**Civilization VI checks the stacking layer at the END of a move, not at every
step.** A unit may walk *through* a tile held by its own unit of the same
layer, as long as it has the Movement to leave again. It may not *finish* its
move there.

CIVVIS answered both questions with one boolean, `Game::can_enter`, and that
boolean gated every intermediate step of every flood, path and route in the
engine. A friendly unit was therefore a wall: no column filed through a defile,
no front line rotated, no route was planned through the army that was standing
on it, and the reachability overlay stopped at our own units.

Reference basis: the in-game
[Civilopedia "Movement" entry](https://www.civilopedia.net/en-US/standard-rules/concepts/movement_3/);
the shipped end-turn blocker `ENDTURN_BLOCKING_STACKED_UNITS`, which this
repository's own bridge already handles (`docs/CIV6_COMPUTER_CONTROL.md`) and
which exists precisely because units are allowed to be stacked in the middle of
a turn.

⚠ It was found by a person playing the Tactics arena, not by any test, screen,
ladder or bench. See [Why nothing caught it](#why-nothing-caught-it).

## The rules

| Rule | Where it lives | Pinned by |
|---|---|---|
| A unit may **cross** a tile held by its own unit of the same stacking layer | `Game::entry_at` → `Entry::Pass` | `a_unit_passes_through_its_own_military_with_movement_to_spare` |
| A unit may **not finish** a move on one | `Game::can_stop` | `a_unit_cannot_end_on_its_own_unit` |
| Crossing needs the Movement to leave again; a crossing with nothing left is not a move | `Game::flow_past`, `Game::path_to` | `passing_through_needs_the_movement_to_get_out` |
| The one-free-step allowance never lands a unit on its own | `Game::can_pay_step` + `can_stop` | `the_first_free_step_never_lands_on_a_friend` |
| Entering enemy zone of control ends movement, so a friendly tile inside one is a dead end and not a crossing | `Game::formation_enters_enemy_zoc` | `enemy_zone_of_control_on_a_friends_tile_is_a_dead_end` |
| Every **foreign** unit blocks the step itself — other civilizations, allies, city-states — except the capture cases | `Game::entry_at` → `Entry::Blocked` | `foreign_units_still_block_the_step_itself` |
| Layers: land military, naval military, civilian, support, religious, air (a slot, not a layer). Two units contend only within one layer | `Game::shares_stacking_layer` | `the_stacking_layers_decide_who_crosses_whom` |
| A walk never leaves two of one player's units contending for a tile, however it ends | `Game::do_move_to` | `a_walk_never_leaves_two_units_stacked` |
| Two adjacent friendly units of one layer may **swap**, and both pay the entry cost of the tile they take | `Game::do_swap` | `adjacent_friendly_units_swap_and_both_pay_the_step` and the three refusals beside it |
| The threat/attack envelopes read through *everything* and are a reading, never a permission | `Game::threat_reach`, `Game::attack_reach` | `the_threat_reading_still_passes_everything` |

All of these live in `src/game/movement_rule_tests.rs`.

## What this means for callers

There are now three questions where there was one, and picking the wrong one is
how this defect comes back:

- **`can_enter` / `can_move`** — may this unit step here **and stay**? This is
  what a single `Action::Move` asks, and it is the right question for a
  destination.
- **`can_pass`** — may this unit cross this tile on the way somewhere else?
  Every flood and path expands on this.
- **`can_stop`** — may this unit be left standing here? A fact about the
  destination alone, so a flood decides it once per tile.

Consequences a caller has to know:

- **`Game::reachable` is not the flood's key set.** The flood
  (`flow`/`flow_past`) returns every tile the movement can occupy *or cross*;
  `reachable` filters it by `can_stop`. Anything that offers a destination to a
  player, a client or a planner wants `reachable`.
- **A path returned by `path_to` or `approach_reach` may cross our own units,
  so it is one order and not a list of stopping places.** Replay it with
  `Action::MoveTo`. Walking it with one `Action::Move` per step stops dead on
  the first friendly tile — that is what `AdvancedAi::walk_to_stand` used to do.
- **`Action::MoveTo` is the only action that may execute a crossing.** It
  enforces the never-end-stacked invariant itself: it anchors on the last tile
  the unit could legally be left on, so a walk that stops short for any reason
  lands there rather than on top of one of ours.
- **`route_step` still returns a tile the unit may stand on.** Its 80-odd
  callers were not changed. When a march is blocked by nothing but our own
  column, `BasicAi::step_toward_range` reaches its pass-through fallback, which
  fires only after every ordinary step has been refused — it replaces a hold,
  never a march anybody already chose.

## Swap

`Action::Swap { unit, other }` exchanges two adjacent units of one player.
Both move, so both pay the entry cost of the tile they take, both lose
fortification, and both are subject to zone of control on arrival. It is
refused for units on different stacking layers (they may already share a tile,
so the mover can simply move), for a linked escort, for a tile carrying based
aircraft, and for anything belonging to another player.

## What is not modelled

Stated exactly, because a half-shipped affordance described as shipped is how
the defect above survived for so long.

- **Swap is engine and protocol only.** `Action::Swap` is legal, tested and
  refused correctly, and any client or controller may send it. It is **not**
  enumerated by `Game::legal_actions`, so learned agents never see it; it has
  no button in the browser. **The `swap-rotation` gene (2026-08-26,
  `src/ai/advanced/swap_rotation.rs`, opt-in and off) is the first controller
  to choose one**: a unit in contact and at or below `withdraw_hp` trades
  places with an adjacent melee friend 25 hp healthier and further from the
  enemy. Measured positive on four instruments and significant on none; see
  `docs/LIVE_TACTICS.md` §18, which also records that the value is in not
  opening the line rather than in the healing.
- **The host's own swap operation** is not driven over the Civilization VI
  bridge, so a swap is a CIVVIS-side decision only. Two `MOVE_TO` orders cannot
  emulate one: the first is refused.
- **Formation swaps.** A linked escort moves as one unit with its charge;
  exchanging half of a formation is refused rather than approximated.
- **`route_step` still plans around our own units on its first step.** Its
  eighty-odd callers keep the old contract — the tile it returns is one the
  unit may stand on. The crossing case is answered by
  `Game::pass_through_destination` instead, which both the controller and
  `/route` reach only when the ordinary router has nothing.

## Why nothing caught it

Recorded here because the causes are structural and will produce the next
defect of this class if they are left alone.

1. **One predicate answered two questions, and its doc comment stated the
   wrong scope as settled law.** Every later reader — three performance PRs, a
   flood cache, a −12.34% profile result — read that function for cost and
   trusted the comment for meaning.
2. **The mechanics checklist's granularity hid it.** One green row covered
   several rules and named none of the missing ones. A ✅ row is never
   re-opened by a reader who does not already know what is wrong with it.
3. **Fidelity tooling is data-only.** `tools/civ6_fidelity.py` compares the
   shipped database; `docs/MECHANICS.md` marks behavioural rules "DLL —
   behavioural only", which in practice means *no tool checks them at all*.
   `tools/live_divergence.py` is projection-only: "the live record carries
   order COUNTS, not orders with targets, so nothing is replayed."
4. **Legality was verified in one direction.** The bridge only ever sends the
   host what CIVVIS already believes is legal, so "Civilization VI allows
   something CIVVIS refuses" produces no signal anywhere.
5. **Every A/B plays both arms under the same rules.** Gene screens, the Elo
   ladder and the Tactics bench cannot see a rule error, and the whole
   `docs/TACTICS.md` programme was tuned inside the wall.
6. **Attention followed the profiler, not the rulebook.** This was the
   most-studied function in the engine and had no conformance test.

The fixes that follow from these are tracked in `docs/FIDELITY.md` and
`docs/AI_GAPS.md`: one rule per checklist row with a source citation and a
named test; recording live orders **with targets** so divergence can replay
legality in both directions; and bucketing refused movement by cause, so
"refused because one of ours was standing there" is a number somebody can see
rather than a shape nobody was looking for.

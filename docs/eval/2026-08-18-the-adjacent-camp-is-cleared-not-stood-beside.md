# The adjacent camp is cleared, not stood beside

_2026-08-18 · `codex-root` (implementation) + `claude-fable-campclear` (gate + arm)_

## What was asked

Operator directive from a live run: a Slinger ended its turn beside an
EMPTY barbarian camp, declined to enter it, and the camp then spawned the
archer that killed the Slinger. Camps are captured by movement, not by an
attack, so no attack scan ever offers the clear: `tactical_step` holds a
melee unit one tile from its target, the peacetime empty-enemies branch
hands the unit to staging or the Basic wander, and a free 50-gold,
spawn-denying clear goes untaken. Fix the behavior.

## What shipped

`BasicAi::clear_adjacent_empty_barbarian_camp`: enter a **visible,
undefended** camp that is **one legal step** away, by a direct engine
`Move`. Hooked ahead of village, escort, staging, patrol, and explore
assignments in both the Basic and Advanced steps, and behind combat,
retreat, and city-garrison claims. No claim, no march, no exchange gate —
it fires only when the clear is immediate, which is exactly the case the
retained `camp_bounty` errand (2026-08-18, "The camp errand fires, moves
its metrics, and does not pay") never covered: the bounty priced marches
to camps and lost pooled 41.3%; this treatment spends at most the step the
unit was already going to waste standing next to the camp. Fogged,
defended, or occupied camps are never targets, and the live seat is
covered because the mirror rebuilds `barb_camps` from visible tile
improvements on every apply.

## Why the ledger stands

The first cut was unconditional and reached the frozen rating anchor —
`advanced_v1_plays_the_same_game_it_always_did` failed (18,572 → 18,471
decisions) and the draft plan was a v15 re-pin, which restarts the entire
Elo ledger. Rejected: the anchor test's own contract and the `naval_recon`
precedent take the gate route instead. `adjacent_camp_clear` is default-ON
in `BasicAi::new()` / `with_weights()`, explicitly OFF in
`AdvancedAi::legacy()`, and minors keep their own behaviour — the anchor
fingerprint does not move, `ELO_PROTOCOL_VERSION` stays at v14, and
`the_adjacent_camp_clear_cannot_reach_the_frozen_anchor` pins the gate.

## How it stays priceable

Withhold arm `advanced_without_adjacent_camp_clear` (axis
`adjacent-camp-clear-withheld`), per the price-by-withholding discipline:
the treatment ships on mechanism proof and operator directive, and its Elo
price is measurable at any time as `advanced` vs the withhold arm on
disjoint seeds. No screen was run before shipping — the mechanism pins
(warrior and scout enter the camp on the barbarian-response path; the
Basic peacetime step clears before movement assignments) discriminate from
the wander's accidental clears by being single-turn, and the failure mode
being repaired was observed on the live seat, where camps raid and spawn
(unlike the passive native world that priced the bounty down).

Related: the camp-bounty rematch in a raiding world stays open; if the
environment activation lands, both treatments should be re-priced there.

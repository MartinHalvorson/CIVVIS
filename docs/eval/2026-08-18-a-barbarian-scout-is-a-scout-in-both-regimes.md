# A barbarian scout is a scout in both regimes

_2026-08-18 · `claude-fable-barbs`_

## What was asked

`barbarian_scouts_are_scouts` (settlement risk and `recon_flight` skip a
barbarian-owned recon unit) was scoped Firaxis-only on the recorded claim
that "CIVVIS's own barbarian scouts DO capture (`capture_adjacent_civilian`
runs for them)". Is that claim true, and if not, what does carrying the
exemption natively cost or pay?

## How it was measured

The claim first, by reading the path: the barbarian seat's military units
are returned to `fortify_or_stop` by the minor-seat gate before any capture
step (the seat is a minor with no home city), and the engine's own scout
phase moves its recon by `relocate`, which has no capture hook. A native
barbarian scout can neither attack nor capture — the claim is false, and
the exemption is true in both regimes (Firaxis' barbarian scouts neither
attack nor capture either; that was always the treatment's premise).

Then the promotion screen: `advanced_without_barbarian_scouts_are_scouts`
(the new withhold) vs `advanced` (now carrying the exemption), 20 map pairs
per seed on two disjoint seeds (`230000000`, `240000000`), deployment
shape — 6p 74×46, 9 city-states, online, 250 turns, all six victories.
`ai_eval` gained a `cities@t60` sample (the opening-tempo correlate the
live ladder records) as the goal metric.

## What it measured

The arms diverge (the exemption fires natively), and everything lands
within noise:

- Seed 230000000: withhold 47.5% (Elo −17, CI −286..+221), direction 4/5/11.
- Seed 240000000: withhold 60.0% (+70, CI −103..+418), direction 5/1/14.
- Pooled 40 pairs: withhold 43/80 games (53.8%), direction 9 withhold / 6
  stock / 25 neutral — null, no consistent direction.
- `cities@t60`: 3.58 vs 3.58 and 3.62 vs 3.58 — flat. Reveal, villages,
  meets, era score, camps: all flat.

The proof explains the null: a unit that cannot act cannot punish boldness,
so ignoring it cannot lose anything natively — and in the current passive
world (see "The world where the barbarians raid") the settler encounters
that would show a tempo gain are rare.

## What was decided

Shipped default-ON, on the proof and on parity, not on Elo: the Firaxis-only
scoping rested on a false claim; the live bundle already carries the
exemption, so the promotion removes a regime split rather than adding one;
the native record above is a measured null at 40 pairs with the frozen
anchors unchanged. The tag moves to the engine-repair war half (the
recon-quartet precedent) and `advanced_without_barbarian_scouts_are_scouts`
stays registered so any future regression is priceable. The live-side value
(one barbarian Scout froze a whole opening, `civvis-20260816T151716Z`,
t15–t35) already ships in the live bundle and is unchanged.

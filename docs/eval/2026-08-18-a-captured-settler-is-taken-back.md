# A captured settler is taken back

_2026-08-18 · `claude-fable-rescue`_

## What was asked

Operator heuristic: "if we can capture a barbarian settler — especially one
of our settlers that was captured — we should always do that." It arose on
the live seat, run `civvis-20260818T222844Z`, turns ~24–41.

## The live episode

Reconstructed from `events.jsonl` + `orders.sqlite`: our settler was
captured at t24 one tile from its stacked guard; we killed the barbarian
escort at t26; the now-unguarded settler then walked past four of our units
for seven turns on its way to a naval camp near (30,15). The two times a
unit stood adjacent at decision time — the scout at t27, the slinger at
t33 — `decline_settlers` in `BasicAi::military_step` keyed on "we already
own a settler / no practical settle site" and ordered a **fortify** (its
freeze branch; origin martbot commit `9cb45510`, one comment line of
rationale, never measured). The pursuing slinger meanwhile targeted the
tile *beside* the settler every turn — ranged attack positioning, and the
engine rejects `Attack` on an undefended civilian — so the stern chase
could never convert. The settler was lost.

## What changed

`BasicAi::civilian_rescue` (treatment `civilian-rescue`, default off so the
frozen native controllers keep their history; on in the production
constructor, `enable_live_bridge`, and the engine-repair war half):

1. A barbarian-held settler (`barb_pid`-owned, not `is_barbarian` — Free
   Cities carry that flag) is never declined and outranks every other
   adjacent capture.
2. `pursue_capturable_civilian`: the best capturable settler/builder within
   this turn's movement reach is walked onto — one step per unit-loop call,
   the adjacent branch finishes the capture the same turn. A unit standing
   on or beside one of our own settlers never pursues (escort safety).
3. Major-owned settlers keep the decline guard unchanged.

Withhold arms: `advanced_without_civilian_rescue` (axis
`civilian-rescue-withheld`), `live_without_civilian_rescue`.

## How it was measured

Mechanism pinned by unit test: the two-tile walk-on capture converts on a
flattened board (also proving `route_step` paths onto an enemy-civilian
tile), the rescue fires despite a duplicate own settler, the frozen-off arm
still declines, and a major's settler is still declined under rescue.

Promotion screen: `advanced_without_civilian_rescue` vs `advanced`, 20 map
pairs per seed on seeds `250000000` and `260000000`, deployment shape (6p
74×46, 9 city-states, online, 250 turns, all six victories). Note the world
now raids natively (`civ-lost` ≈ 6.2/seat-game in both arms), so barbarian-
held civilians exist in-regime; the reach-pursuit also prices in major wars.

## What it measured

- Seed 250000000: withhold 45.0% (Elo −35, CI −271..+129), direction 0
  withhold / 18 neutral / 2 stock, p=0.50. Gate INCONCLUSIVE.
- Seed 260000000: withhold 57.5% (+53, CI −123..+344), direction 4 / 15 / 1,
  p=0.375. Gate INCONCLUSIVE.
- Pooled 80 games: withhold 41/80 (51.2%) — the two seeds flip sign; null,
  no consistent direction. Cities, cleared camps, kills, `civ-lost`, and
  `camps standing` are flat between arms (`civ-lost` counts the capture
  event and is not decremented by a later rescue, so it cannot show the
  treatment).

## What was decided

Shipped default-ON on the proof and the operator heuristic, not on Elo —
the same footing as `barbarian-scouts-are-scouts` (#1987): the mechanism is
pinned by test, the native screen is a measured two-seed null with the
frozen anchors unchanged, and the live value is the episode above, where
either of two one-move rescues would have returned a full settler. No
rescue counter exists in the engine ledger, so native fire frequency is
unmeasured; the live verification is the why-journal line "marches to
rescue a capturable civilian" in post-merge ladder runs.

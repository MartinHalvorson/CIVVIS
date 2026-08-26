# A barbarian-held settler is taken back

_2026-08-26 · `claude-fable-recapture`_

## What was asked

Operator, on the live seat: "why are we not capturing a barbarian settler in
our territory that is clearly capturable? big mistake. free settler that was
taken from us" — then "add back in a better capture barbarian settler gene".
Run `civvis-20260826T194422Z` (King), turns 65–92.

## The live episode

Reconstructed from `events.jsonl` + `orders.sqlite`: our settler 1900545 was
taken at (11,8) on t65 and, as barbarian unit 9895965, wandered UNGUARDED two
to four tiles from the capital from t72 on. Heavy Chariots with full movement
stood adjacent at t76, t82, t87 and t88; every one of those frames produced a
`FORTIFY`. No order in the run ever targeted the settler's tile.

Three causes, stacked:

1. **#2075's rescue was cut.** `civilian-rescue` left in #2509 (2026-08-25,
   operator directive), so the seat plays the July guard.
2. **The July guard is nearly always on for an expansion seat.**
   `decline_settlers = counts().settlers > 0 || !has_practical_settle_site`
   (`AdvancedAi::advance_units`), and `counts()` also counts a settler at the
   head of a build queue. `capture_adjacent_civilian` then returns nothing for
   a settler, `unwanted_settler_adjacent` sends the unit to `fortify_or_stop`,
   and `pursue_capturable_civilian` was barbarian-only — a major never walked
   toward a free civilian.
3. **#2075 never captured live either.** `Action::Move` crossed as a bare
   `MOVE_TO`; the mod's own note on `attackModifiers` and Firaxis's
   `Civ6Common.lua:152` say a plain MOVE_TO "will not enter a plot an enemy
   is standing on — walk next to it and stop". Over the 273 live runs that
   carried #2075 (08-18 → 08-25): 278 unit-turns adjacent to an unguarded
   barbarian-held settler with movement left, 65 `MOVE_TO` orders aimed at
   its tile, **zero captures**; all 64 "captures" in the window were kills
   that advanced onto a GUARDED settler. The screen that priced
   `civilian-rescue` null was pricing a mechanism the host never executed.

## What changed

Gene `barbarian-settler-capture` (`Kind::Repair(Axis::War)`, flag
`BasicAi::barbarian_settler_capture`, pinned on in
`tools/genes.py::OPERATOR_DEFAULT_ON` at the operator's request):

1. A barbarian-held settler (`barb_pid`-owned) is exempt from the
   duplicate/unusable-settler guard in `BasicAi::military_step` and from
   `AdvancedAi`'s `unwanted_settler_adjacent`, and outranks every other
   adjacent capture. Major-owned settlers keep the guard.
2. `pursue_capturable_civilian` is reachable by a major and is called from
   `AdvancedAi::advanced_military_step_with_decline` (#2075 only enabled the
   flag and never called it from the deployed path). A barbarian-held
   settler is chased out to `BARBARIAN_SETTLER_PURSUIT_RADIUS` (4 tiles);
   everything else stays within this turn's reach; a unit on or beside one
   of our own settlers never pursues; the only soldier in one of our cities
   chases only what it can take this turn. The barbarian hunter's own
   pursuit is unchanged.
3. **The host leg.** `civvis_orders::translate` emits a move onto an enemy
   civilian as the `CAPTURE` verb; the mod requests the same MOVE_TO with
   the attack modifier and no strike ledger; `verify_unit_order` reads the
   unit standing on the tile, or the civilian gone from it, as the capture.
   Ungated: a bare MOVE_TO onto an occupied enemy tile can never succeed.

## How it was measured

Mechanism pinned by unit test: the rescue despite a duplicate settler
(`BasicAi` and the deployed `AdvancedAi` path), the frozen-off decline
preserved, a major's settler still declined, the two-tile walk-on capture,
the four-tile approach, the lone-garrison hold, the `CAPTURE` translation
and its verdict.

Fires probe: `gene_screen --games 24 --jobs 6 --difficulty emperor --genes
barbarian-settler-capture --start-seed 270000000`, the standard shape.

## What it measured

Fires probe, 24 games / 144 seats, seed 270000000, Emperor, 6p 74×46, 9
city-states, online, 250 turns: `barbarian-settler-capture` − off **win
+8.3 pp ± 6.7** (z +1.25), share +0.35 pp ± 0.63. A fires check — the gene
acts and the row exists — not a verdict; win resolves only ±8 pp at this
size. The large standard-shape batches price it like every other gene.

The number that matters is the host leg, and it is on the live seat, not in
the sim: the next barbarian-held settler beside one of our soldiers should
produce a `CAPTURE` row in `orders.sqlite` and a `unit:CAPTURE` verdict the
frame after — the join that showed 65 `MOVE_TO` / 0 captures for #2075.

## Follow-ups

- Read the first live `CAPTURE` verdicts; `not_captured` on an adjacent,
  unguarded civilian means the host refused the attack-modifier move and the
  mod must fall back to `UnitOperationMoveModifiers.NONE` after a refusal.
- The three `decline_settlers` clauses in `coordinated_tactical_step` still
  refuse to step onto a barbarian-held settler during group tactics; the
  capture and pursuit steps run before them, so nothing waits on this.

# Sea answers: pinned, registered, and natively non-firing

- Date: 2026-08-18
- Arms: `advanced_sea_answers` vs `advanced`
- Shape: 6p 74×46, 9 city-states, online, 250 turns

Two repairs behind `sea_answers`: a barbarian raider on water counts toward
`major_naval_war` (the exploration arm excluded barbarians while
`desired_navy`'s own war test always counted them), and
`home_defense_objective` admits ships to the responder pool while matching
responder domain to the threat's tile — a galley offshore used to recruit
land units that could neither reach it nor be reached by it (the threat
list has no domain filter; responders were land-only).

The mechanism is pinned by
`a_galley_offshore_is_answered_by_a_galley_not_a_spearman` (blind: the
spearman is recalled to the water threat while our galley idles; aware: the
galley takes the raider, the spearman keeps its job).

**Fires-check, 20 pairs, seed 170000000: the arms are byte-identical —
every metric equal, terminal direction 0/0/20 neutral.** Natively the
treatment cannot fire: `home_defense` is a bridge treatment (off in native
production), and the barbarian-second-hull coincidence (exactly one hull, a
barb ship on water, the arm check live) did not occur once in 40 games. No
screen was run on a non-firing arm.

**Where it matters is the live bridge**, which runs `home_defense` and
where a barbarian galley took two settlers in one run
(`civvis-20260815T233405Z`). The honest next step is a `LIVE_TREATMENTS`
row + `enable_live_bridge` adoption priced on the live ladder (~152
games/arm), not a native promotion.

# The barbarian ledger

_2026-08-18 · `claude-fable-barbs`_

## What was asked

What do barbarians currently cost and yield the deployed native controller?
Nothing barbarian was measured before this round: `ai_eval` read neither
existing engine counter (`camps` cleared, `barbs_killed`), and the engine had
no counter at all for the loss side. This round ships the instrument (PR
#1976) and takes the first reading, as the baseline for the barbarian
improvement series.

## How it was measured

`advanced` vs `advanced_v1` (the frozen anchor; both sides get a Barbarians
row), 20 map pairs / 40 games, seed prefix `210000000..=210000019`, deployment
shape: 6p 74×46, 9 city-states, online, 250 turns, continents/planet/poles,
civilizations randomized, all six victories, `--deployment-comparison`. The
new instrument: camps cleared and barbs killed (existing engine counters),
`lost_to_barbarians` and `civilians_lost_to_barbarians` (new victim-side
counters, Free Cities excluded), camps standing within six tiles of the
seat's cities sampled at t50, and camps standing at game end.

## What it measured

| arm | cleared | kills | lost | civ-lost | camps≤6@t50 | standing at end |
|---|---|---|---|---|---|---|
| `advanced` | 3.34 | 19.32 | 0.47 | 0.00 | 2.49 | 17.10 |
| `advanced_v1` | 3.42 | 23.40 | 0.11 | 0.00 | 2.34 | 17.10 |

(Environmental reading per seat over 40 games each; the head-to-head itself
was `advanced` 77.5%, Wilson CI 55.7%..90.4% — the anchor gap, not this
round's question.)

Three facts frame the axis:

- **Native barbarians take nothing.** 0.00 civilians captured in 240
  seat-games, 0.47 units lost per game (deaths while attacking barbs, not
  raids — the engine's barbarian military units never move: `BasicAi`'s
  minor-seat gate returns every one of them to `fortify_or_stop` because the
  barbarian seat has no home city). The live regime is the opposite — a
  barbarian galley took two settlers in one run (`civvis-20260815T233405Z`),
  eight of fourteen settlers captured in another (`civvis-20260816T155856Z`).
  Native losses are near-zero because native barbs are passive, not because
  defense is good.
- **Camps go uncleared.** 17.1 of the world's ~18-camp target stand at game
  end; ~2.5 stand within six tiles of the empire's own cities at t50. Each
  clear pays 50 gold, 2–3 era score (Ancient–Medieval), Military Tradition
  boost progress at one camp, Bronze Working at three kills. The ~19
  barbarian kills per game say armies fight barbs incidentally (the Basic
  tactical fallback's enemy list carries no barbarian filter); the ~3.3
  clears say nobody runs the errand deliberately — the military step's enemy list admits the barbarian
  seat only behind `home_defense`, which native production ships OFF.
- **The two regimes disagree**, so native-null defensive treatments
  (sea-answers precedent) stay honest live-bundle candidates, while the
  native-measurable axis is the clearing economics.

## What was decided

Instrument shipped (PR #1976); baseline recorded here. The improvement
series opens with `advanced_camp_bounty` (PR #1977): deliberate,
exchange-gated peacetime camp clearing near home, screened against this
baseline on this seed and a disjoint one.

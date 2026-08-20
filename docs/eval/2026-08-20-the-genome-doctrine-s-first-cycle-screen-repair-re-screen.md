# The genome doctrine's first cycle: screen, repair, re-screen

_2026-08-20 · `agent/mbp-m5-pro-64/claude-fable-tactics` · PRs #2191, #2193_

## What was asked

The operator's directive, verbatim intent: treat the whole controller as a
genome — every feature a gene — and test the genes regularly in very large
randomized runs, hundreds of games, every player its own test. **Which flags
have the highest impact on winning, which flags cause more losses than
baseline when activated, and can the harmful ones be repaired so they help?**

## How it was measured

`gene_screen` (`docs/GENE_SCREEN.md`), random-genome foldover screens, shuffled
civs everywhere, 4p 60×38 Pangaea Online-250 throughout. Five runs, disjoint
seed windows:

| run | design | regime | size | seeds |
|---|---|---|---|---|
| 1 | classic (one treated seat v production field) | all six lanes | 4,000 pairs + 200 anchors (8,400 games) | 40000000.. |
| 2 | **all-seats** (every major its own genome; errors clustered by game pair) | `domination,score` | 3,204 seat-pairs read (1,602 games; stopped early — the two questions it existed for were decisive) | 41000000.. |
| 3R | all-seats, **repaired code**, six repaired genes only | all six | 4,800 seat-pairs (2,400 games) | 43000000.. |
| 3b | all-seats, repaired code, six genes | `domination,score` | 2,000 seat-pairs (1,000 games + 50 anchor pairs) | 44000000.. |
| 3c | classic, repaired code (v2 gates), six genes — the exact phase-1 design | all six | 2,000 pairs (4,000 games) | 45000000.. |

Runs 1–2 price the pre-repair genome; 3b and 3c are the paired post-repair
reads (3b against run 2's design and regime, 3c against run 1's). Run 3R is
recorded for what it taught about designs: it verified all-seats against a
treated field while run 1 was classic against production, and the two are not
the same estimand — its numbers are kept but no repair verdict is read across
that boundary.

## What it measured

**Run 1 — the native all-six verdict (resolution ±2.1 pp win, ±0.41 pp share
at 80% power; family-wise bar |z| ≥ 3.35).** No gene helps past the bar.
The losing side:

| gene | win Δ | share Δ |
|---|---|---|
| `war-economy` | **−7.2 pp [−8.7, −5.7], z −9.6** | −2.26 pp, z −16.0 |
| `wide-map-capacity` | **−3.4 [−4.9, −1.9], z −4.5** | **+2.33, z +16.5** |
| `governor-every-lane` | **−2.8 [−4.3, −1.3], z −3.7** | −4.45, z −34.8 |
| `campus-every-city` | **−2.8 [−4.3, −1.3], z −3.7** | −0.19, z −1.3 |
| `housing-research` | −2.2 [−3.7, −0.8], z −3.0 | −0.67, z −4.6 ** |
| `garrison-walls` | −2.1 [−3.5, −0.6], z −2.7 | −0.51, z −3.5 ** |

Candidates below the bar: `ranged-line-of-sight` −2.2 (a Firaxis-fidelity
self-restriction — the native engine enforces line of sight for nobody, so
requiring it of ourselves is a pure native handicap and correct live
behaviour; recorded, not repaired), `siege-muster` −2.1, `housing-cards` −1.8,
`settler-stack-discipline` −1.8, `civilian-rescue` −1.8. Anchors: the whole
repair bundle all-on won **7.5%** of its games against **27%** all-off — the
bundle's native cost is not one gene's.

**Run 2 — the war regime (all-seats; resolution ±3.2 pp win).**
`war-economy` **−26.7 pp [−28.5, −24.8], z −29** — catastrophic in the very
regime it was built for, so its defect is the routing, not the lane.
`wide-map-capacity` **+19.2 pp [+17.2, +21.3], z +18.5** — the single
highest-impact gene measured anywhere in this cycle, positive exactly where
conversion cannot end the game. `governor-every-lane` −9.7 (z −8.7) and
`campus-every-city` −4.1 (z −3.6) hurt here too. The per-civ split
(`--by-civ`): `war-economy` hurts all 22+ civilizations (z −3.9 to −5.8 each)
— a mechanism defect, not a civ-strategy mismatch; `wide-map-capacity` helps
broadly. The new opt-ins lean positive, unresolved: `arrival-waves` +1.6
(z +1.4), `joint-tactics` +1.4 (z +1.3).

**The repairs (PR #2193), each giving a gate back the premise it claimed:**

- `war-economy`: the Conquest production routing now requires the war with
  the plan's target to be **declared**; peacetime staging returns to the
  baseline governor; the appointed timed war keeps its own routing.
- `wide-map-capacity`: the wide city target stands until a religious victory
  is enabled AND a rival religion exists ("conversion race live", ~t40–70) —
  the regime split of run 1's own rows (+9.0 pp in the 511 non-conversion
  pairs, −7.5 pp in the 1,825 conversion pairs) is the mechanism. A first-cut
  gate (a third of cities converted) measured **inert** on 1,200 verification
  pairs — its signal arrives ~t120, after the settling window; the paired
  cities delta did not move (+2.68 → +2.78). Kept here as the cautionary
  half of the round: a repair is a hypothesis until the screen says otherwise.
- `governor-every-lane`: the lane routing keeps the baseline's trader
  reservation as a hard preemption (the recorded census fingerprint: traders
  0.70×, gold 0.71× of control).
- `campus-every-city`: beyond the half-empire cliff, the coverage exemption
  asks only in cities of pop ≥ 7 — the towns that can staff the Library the
  funnel argument rests on.
- `housing-research`: the tech goal needs the cap to bind the empire (≥2
  capped cities, ≥ half) and to be tech-bound (no capped city can already
  produce housing relief).
- `garrison-walls`: the walls doctrine needs a declared major war or a
  visible hostile within 8 tiles; a quiet map keeps its production.

**Run 3R — all-seats, all-six, repaired code (resolution ±2.3 pp win).**
`housing-research` −0.9 (z −1.1, share +0.02) and `garrison-walls` −1.0
(z −1.2) no longer resolve as harmful; `campus-every-city` −2.1 (z −2.6),
`war-economy` −4.5 (z −5.7), `wide-map-capacity` −6.6 (v1 gate, measured
inert, superseded), `governor-every-lane` −6.7 (z −8.4, share z −40). Read
with the design caveat above; the clean verdicts are 3b and 3c.

**Run 3b — war regime, post-repair (against run 2's −26.7 / +19.2; read at
1,060 seat-pairs and stopped — the two questions it existed for were
resolved, and the all-repairs-on seats it screens cost ~2 games/min):**

| gene | pre-repair (run 2) | post-repair (3b) | verdict |
|---|---|---|---|
| `war-economy` | −26.7 [−28.5, −24.8] | **−18.1 [−21.4, −14.8], z −10.8** | the declared-war gate helped — the intervals are disjoint — and the gene is STILL the worst on the board |
| `wide-map-capacity` | +19.2 [+17.2, +21.3] | **+14.5 [+11.2, +17.8], z +8.7** | the repair did not cost the regime where the gene shines |
| `governor-every-lane` | −9.7 [−11.9, −7.5] | −9.1 [−12.6, −5.6], z −5.1 | the trader preemption is inert in this regime |
| `campus-every-city` | −4.1 [−6.3, −1.8] | −2.5 [−5.9, +1.0] | consistent with mild improvement, unresolved |
| `arrival-waves` | +1.6 [−0.6, +3.9] | −3.0 [−6.7, +0.6] | no reliable effect either read; stays an opt-in |
| `joint-tactics` | +1.4 [−0.8, +3.7] | +2.1 [−1.7, +5.8] | leans positive both reads, unresolved |

**Run 3c — classic all-six, post-repair (against run 1's row for each gene):**
<!-- 3C_RESULTS -->

## What was decided

<!-- DECISIONS: filled with 3b/3c -->

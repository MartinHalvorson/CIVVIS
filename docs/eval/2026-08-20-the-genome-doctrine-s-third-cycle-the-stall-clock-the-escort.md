# The genome doctrine's third cycle: the stall clock, the escorts, and what the screens kept

_2026-08-20 · `agent/mbp-m5-pro-64/claude-fable-tactics` · PR #2199_

## What was asked

The 6p whole-genome ranking (13,386 game-pairs, seeds 46M) and cycle two's
war-regime residual left four genes on the losing side: `war-economy` (war
−7.4 after two repairs), `settler-stack-discipline` (−2.3), `stacked-escort`
(−2.1), `campus-every-city` (−1.9) — plus `governor-every-lane`'s −4.6 pp
score-share drag. The operator's standing rule: fix what can be fixed, and
**remove the code that cannot**. What did cycle three's repairs clear, and
what did the knife take?

## How it was measured

Four repairs (each studied first; PR #2199), then re-pricing on disjoint
seeds against each gene's matched baseline:

- **6a**: 6p all-seats, 6,000 seat-pairs, the four genes vs the phase-4
  design (seeds 49000000..; resolution ±1.7 pp win / ±0.25 share).
- **6b**: classic 4p `domination,score`, 800 pairs, `war-economy` +
  `wide-map-capacity` vs 5b (seeds 50000000..).
- **6c**: classic 4p all-lanes, 2,000 pairs, `governor-every-lane`'s priced
  completion vs 3c's trader-only +0.8 win / −4.63 share (seeds 51000000..).

## What it measured

**6a — native, repaired code vs the phase-4 baseline:**

| gene | pre (phase 4) | post (6a) | verdict |
|---|---|---|---|
| `settler-stack-discipline` | −2.3 [−3.1, −1.5] | **−0.9 [−2.0, +0.2]** | threat-scoped escorting clears the win harm; share −0.38 residual |
| `stacked-escort` | −2.1 [−2.9, −1.2] | −2.2 [−3.4, −1.0] | unchanged — the same gate that cleared its twin did nothing here; cycle four's lever is engagement hysteresis (bind on threat, keep for the march) |
| `war-economy` | −3.8 [−4.7, −3.0] | −2.3 [−3.5, −1.2] · share **+0.41, z +4.6** | the stall clock improved it again; the war regime decides below |
| `campus-every-city` | −1.9 [−2.7, −1.0] | **−2.8 [−4.0, −1.6]** | the conversion-race gate measured WORSE and was **reverted the same day** with its number in the code — at six players a rival religion exists by ~t40, so the gate turned coverage off wholesale, and a Campus is not fuel for the rival's clock the way a settled city is. The pop floor stays; the unmeasured next lever is pricing the ask by remaining game length (`campus_payback_horizon`'s shape) |

**6b — the war regime, and the knife.** `wide-map-capacity` **+10.2
[+7.0, +13.5] (z +6.3)** — the cycle-one repair's upside intact on a third
disjoint seed window. `war-economy` **−6.8 [−10.0, −3.5] (z −4.1)** —
statistically unmoved from 5b's −7.4. That was the third repair (declared-war
gate −26.7 → −18.1; front bound → −7.4; stall clock → −6.8), so per the rule
**the Conquest routing was REMOVED**, its full trail in a removal note where
it stood. The flag keeps its protective halves (recovery-when-broke, the
maintenance-emergency policy cards — they prevent the recorded bankruptcy
disbands); the appointed timed war keeps its own `war_plan` routing; the
pins now keep the routing OUT. The next whole-genome screen prices the
residual.

**6c — the governor's priced completion: reverted, like the preemption.**
Against 3c's trader-only baseline (+0.8 [−1.3, +2.9] win · −4.63 share), the
priced completion measured **−2.9 [−4.8, −0.9] win (z −2.9) · −4.52 share
(z −34)** over 2,000 pairs. Building-first is wrong for the lanes in either
form — preempted (−3.6, cycle two) or priced (−2.9, this cycle) — and the
−4.5 pp share drag survives every building-side lever. The trader preemption
stands as the composite's one verified repair; the note in the valuation
table records both failures and hands the gate the question the 2026-08-18
bisect already priced: whether the victory-lanes half carries its weight at
all (−70..−80 Elo; PR #1955).

## What was decided

- **Kept**: threat-scoped escorting (clears `settler-stack-discipline`; a
  guard is released to the army on quiet ground — the recorded live captures
  all had a visible hostile in sight, so the live seat keeps its protection);
  the `war-economy` stall clock's native gains ride into the removal note.
- **Removed**: `war-economy`'s Conquest routing — the genome's first code
  removal under the doctrine. Three repairs, three regimes, one number that
  never moved.
- **Reverted, numbers recorded**: the campus conversion-race gate (this
  cycle) joins the governor building preemption (cycle two) — the screen has
  now caught three plausible regressions before they shipped.
- **Cycle four's docket**: `stacked-escort` hysteresis; `campus-every-city`
  payback-horizon pricing; the sub-bar 6p repeats (`siege-is-progress` −1.2,
  `loyalty-policy-defence` −1.1, `apostle-promotion-by-role` −1.1) once a
  fresh whole-genome screen on post-cycle-three code re-ranks everything;
  and a study of `garrison-under-fire` — the genome's first native helper —
  for what a defensive gene that pays actually does.

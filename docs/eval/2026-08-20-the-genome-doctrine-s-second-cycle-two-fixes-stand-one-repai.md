# The genome doctrine's second cycle: two fixes stand, one repair is reverted by its own number

_2026-08-20 · `agent/mbp-m5-pro-64/claude-fable-tactics` · PR #2194_

## What was asked

Cycle one (`docs/eval/2026-08-20-the-genome-doctrine-s-first-cycle…`) left
`war-economy` (−4.1 native / −18.1 war) and `garrison-walls` (−3.1) resolving
harmful after one repair each, and `governor-every-lane` with its −4.6 pp
score-share drag. The operator's standing directive: study where genes could
be working better and improve the code; drop only what is very far off.
**Do the second-cut repairs clear them?**

## How it was measured

Studies first, then repairs, then re-pricing on disjoint seeds in the same
design as each baseline (`gene_screen`, classic 4p 60×38 Online-250, shuffled
civs, field production `advanced`):

- **Studies.** A six-game paired probe showed the ordinary build order walls
  nearly every city by game end with `garrison-walls` OFF — the flag's whole
  marginal action is walling *early*; and the cycle-one gate was open almost
  always (the standing barbarian war makes `is_at_war` true from turn one,
  and a wandering scout counted as a visible threat). The 6p whole-genome
  screen (13,386 game-pairs, seeds 46000000..; its own section below) had
  meanwhile replicated `war-economy` at −3.8 [−4.7, −3.0] on a third design.
- **Repairs.** `war-economy`: the adaptive war routing became spatial —
  cities within `WAR_ECONOMY_FRONT_RADIUS` (12) of the campaign objective
  are the war machine; the interior keeps the baseline governor (skipped
  queues fall to `delegated_cities`); timed wars and victory-targeted seats
  unchanged. `garrison-walls`: the gate prices actual siege risk — the
  capital at a declared MAJOR war or under a raid party (≥2 visible
  non-recon hostiles within 5); a frontier town only with the war's enemy
  near (visible non-recon hostile within 12) or under a raid party.
  `governor-every-lane`: the lane routing ordered the cheapest FIRST
  building of any specialty district before the strategic argmax (the
  recorded census fingerprint: buildings 0.81× of control).
- **Run 5a**: 2,000 pairs, seeds 47000000.., the four genes above plus
  `campus-every-city` unchanged as a stability read — against cycle one's
  3c (2,000 pairs, seeds 45000000..). Resolution ±2.9 pp win / ±0.53 share.
- **Run 5b**: 800 pairs, seeds 48000000.., `domination,score`,
  `war-economy` and `wide-map-capacity` — against 3b's post-cycle-one war
  reads.

## What it measured

| gene | cycle-1 code (3c) | cycle-2 code (5a) | verdict |
|---|---|---|---|
| `war-economy` | −4.1 [−6.2, −2.0], z −3.9 | **−0.8 [−2.8, +1.2], z −0.8** | the front bound clears it: from −7.2 (unrepaired) to a null in two cycles |
| `garrison-walls` | −3.1 [−5.2, −1.0], z −2.9 | **−0.1 [−2.1, +1.9], z −0.1** | the risk-priced gate clears it |
| `governor-every-lane` | +0.8 [−1.3, +2.9] win · −4.63 share (z −33) | **−3.6 [−5.6, −1.6] win (z −3.5) · −5.39 share (z −37)** | the building preemption made BOTH axes worse and was **reverted the same day**, its number recorded in the code |
| `campus-every-city` | −1.7 [−3.8, +0.4] | −1.8 [−3.8, +0.2] | stable; unchanged by this cycle, on cycle three's docket |

**Run 5b (war regime):**
<!-- 5B_RESULTS -->

**The 6p whole-genome screen** (operator specification: six players, every
seat its own genome; 13,386 game-pairs / 26,898 seat-observations at seeds
46000000.. before the process died quietly; the surviving read resolves
±1.2 pp win / ±0.20 share, family-wise bar |z| ≥ 3.36, cycle-1 code):

- **The genome's first native helper**: `garrison-under-fire` +1.3
  [+0.5, +2.1] (z +3.2; +1.5, z +3.4 at the 12k-pair read — at the bar).
  Candidates behind it: `barbarian-scouts-are-scouts` +1.1,
  `siege-tracks-wall` +1.0, `war-reinforcement` +1.0; share helpers
  `loyalty-rate-alarm` (z +6.9) and `wide-map-capacity` (z +4.7 — cycle
  one's repair holding at six players).
- **Past the bar on the losing side**: `war-economy` −3.8 (third design,
  same answer), `settler-stack-discipline` −2.3 (z −5.4, NEW),
  `stacked-escort` −2.1 (z −4.8, NEW), `campus-every-city` −1.9 (persists).
  Sub-bar repeats: `siege-is-progress` −1.2, `loyalty-policy-defence` −1.1,
  `apostle-promotion-by-role` −1.1, `governor-every-lane` −1.1 with share
  z −51.

## What was decided

- **Shipped: the two fixes that measured** — `war-economy`'s front bound and
  `garrison-walls`' risk-priced gate, each with tests pinning the repaired
  contract and the study that motivated it in the doc comment.
- **Shipped: the revert** of the governor's building preemption, with its
  number in the code so the naive form is not retried. The −4.6 share drag
  stays an open problem; the note names the unmeasured next levers (scope
  the completion to the district the routing itself just raised, or price —
  never preempt — the first building into the strategic table).
- **Cycle three's docket, from the 6p ranking**: `settler-stack-discipline`,
  `stacked-escort`, a deeper `campus-every-city` cut, `siege-is-progress`,
  `apostle-promotion-by-role`; and on the helper side, study what
  `garrison-under-fire` does right — it is the only gene the native regime
  has ever rewarded past the bar.
- The methods note from this cycle: **a screen-driven revert is a result** —
  cycle two would have shipped a plausible-sounding regression without run
  5a, exactly the way cycle one's inert wide-map gate would have shipped
  without its cities-delta check.

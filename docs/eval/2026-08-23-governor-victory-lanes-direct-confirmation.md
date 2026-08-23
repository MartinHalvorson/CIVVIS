# `governor-victory-lanes` at the deployment shape: −4.8 pp, and the default turns off

_2026-08-23 · single-gene direct arm · seeds 150000000–150000599 · analysis
`docs/gene_screens/2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json`_

## Why this arm was run

`governor-victory-lanes` — *"half the composite: the governor under the four
victory lanes only"* — shipped **default on**, ranked 6th at **+46** wins/10k
seats. It was promoted on a single legacy column under the provisional
"one column above +20" clause (#2294).

Three independent readings disagreed with that promotion, and none of them had
been put beside the others:

1. **A decomposition predicted it, and its confirming arm was never run.**
   `docs/eval/2026-08-18-pricing-the-governor-s-routing-and-the-settling-asymmetry.md`
   priced the governor composite at **−95**, cleared the Expansion half
   (−20/−16, RETAIN), and concluded by subtraction that *"the four victory lanes
   carry roughly −70 to −80"*. It named `advanced_governor_victory_lanes`
   (seed 29000000) as *"the open end"*. That arm had not run.
2. **The ledger already flagged it.** P10 recorded `verdict: unresolved`,
   **`conflict: true`**, read `helps * · share HURTS **` — win z **+2.46**
   against score-share z **−15.92**. The default rule reads the win axis only,
   so it shipped over its own conflict marker.
3. **The first standard-shape whole-genome screen contradicted the win axis
   too**: −4.73 pp at win z −15.37 over 23,622 matched seat comparisons
   (#2323), the worst of 99 genes.

## The design, pre-registered before the run

Written down before the batch started, and reproduced here unchanged: 600
complete map pairs on seeds **150000000–150000599** — deliberately **disjoint**
from the whole-genome screen's 141000000–141003936, so this is new evidence and
not a re-read of the same maps — one gene varied, every other deployment gene
held fixed, bare `gene_screen` defaults (6 majors, 74×46 Continents, 9
city-states, Online to its own 250-turn clock, all six lanes, best-genome
baseline, all-seats foldover, shuffled civs).

The decision rule, also fixed in advance: **confirmed harmful** if the on−off win
Δ is negative with |win z| ≥ 3; **not confirmed** if |z| < 3 whatever the sign —
in which case the whole-genome screen's largest reading did not survive a direct
arm and that bears on every conclusion drawn from it; **contradicted** if
positive at z ≥ +3. Reporting actual-vs-intended pairs was mandatory either way.

## The result

**600 of 600 pre-registered map pairs completed — 3,600 matched seat
comparisons, no early stop.**

| | on | off |
|---|---:|---:|
| win rate | **14.28%** | **19.06%** |

- **win Δ −4.78 pp, z −6.11**, 95% CI **[−6.3, −3.2]**
- score-share Δ **−2.73 pp, z −23.76**
- read: **`HURTS ** · share HURTS **`**, past this run's family-wise bar
- this run resolves a win Δ of ±2.2 pp at 80% power, so the effect is well
  outside its own resolution
- compute cost −0.05 ± 0.36 % per completed turn: the gene is not paying for
  itself in speed either

**It confirms.** The point estimate matches the whole-genome screen almost
exactly — **−4.78 pp here against −4.73 pp there** — on disjoint maps, with an
independent design that holds the rest of the genome fixed rather than
randomising it. The score-share axis agrees at z −23.76, as P10's share axis had
already said a day before the win axis caught up.

## What changed, and what did not

Entering this as a ledger source moves **exactly one default**:
`governor-victory-lanes` **on → off**. The deployment genome goes **31 → 30**.
No other gene's default moved, and no game rule changed.

Both clauses of the published rule agree, so this does not rest on a marginal
veto: the win columns read **−239 / +46** (the newest screen is negative, and
the two-column average is −96.5, far below the +15 bar), *and* the pooled `Diff`
is **−0.057 pp**, which vetoes independently.

⚠ **This is a native-screen effect. On the live bridge the gene is inert** —
#2335 replayed 768 recorded live turns through two builds differing only in this
flag and got **0 turns different, byte-identical**, because
`src/ai/advanced.rs:31550` reads `… || active_victory_target.is_some() || … ||
every_lane` and the live bridge always passes `--victory <lane>`, so the gate is
already open. Turning the default off therefore costs the live seat nothing and
changes only the headless deployment genome.

## Provenance, and one honest gap

The batch was played by a `gene_screen` built from **`17a27004`**, which predates
#2331's build stamp (`8162f7a9`), so the artefact carries no in-band `build`
block and the ledger refuses it without an explicit escape. Rather than discard
the run, the equivalent was recorded by hand while the binary was still on disk
and the tree still clean:

- commit `17a27004d04936bf737cbada42137024e8020d44`
- `binary_sha256` `bc57c795ed1a6c968932614c56acd51f8be7d9d737b9bfa40a96e58d77bc11b5`
- build tree clean (0 dirty files) at build and at capture

It is therefore entered with `--unverified-build` and that reason, which the
ledger records beside the source. This is the documented escape used as
intended — not a silent pass — and the next arm on this machine should be built
from a commit containing `8162f7a9` so it stamps itself.

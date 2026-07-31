# The empire's city target and the governor that decides

**Verdict: the treatment is a recorded null, refuted by its own fires-check
before any outcome seed was read. The mechanism it documents is real and
corrects `docs/GENOME.md`.** Read the fires-check section for why the obvious
repair makes the empire smaller, and for the measurement that matters more
than the treatment did: at deployment the empire is *target*-limited (4.83
cities against its own target of 5.00) and at the compact eval profile it is
*execution*-limited (2.17 against 3.83), so the expansion axis has two regimes
and one profile cannot judge it.

## The disagreement

`AdvancedAi::assess` computes how many cities this empire wants **on this
map**:

```rust
let map_capacity = (2 + land / 55).clamp(3, 9);
let city_cadence = g.standard_duration(90).max(1) as usize;
let desired_cities = (self.city_target_floor + g.turn as usize / city_cadence)
    .min(map_capacity)
    .min(6);
```

It is land-aware, speed-aware and time-aware: three cities at the opening,
about one more per era, never more than the map can hold.

Nothing on the adaptive path consumes it. `docs/OPENINGS.md` §19 records the
dispatch, and the code still reads that way: after the four-build opening an
untargeted adaptive empire enters `advanced_production` only for `Recovery`;
every other adaptive plan ends in `BasicAi::cities`. That governor's Settler
gate is

```rust
((n_cities + settlers) as f64) < self.w.city_target   // src/ai.rs
```

— a **flat gene that cannot see the map**. `ai_eval` reports stock `advanced`
at *100% adaptive plans, 0.00 switches*, so this is the governor deciding
every evaluated game. That is not a property of stock alone: on the
deployment-profile screen recorded in `docs/EVAL.md`, **all four genome arms
report `{"adaptive": 240}` — 240 of 240 seat-games each** — so every arm
compared there took this path.

So the empire computes one city target and expands to a different one, and the
number that decides is the one that knows least about the map.

## Why the gap is not academic

| agent | `city_target` | what the baseline governor stops at |
|---|---:|---:|
| stock `advanced` (`Weights::default`) | 4.0 | 4 cities, every map size, every speed |
| shipped gen-14 champion (`data/evolved/best.json`) | **2.408** | 3 |
| live-league leader `g28-28` (round 3217) | **9.681** | its own site supply |
| `assess`, deployment profile, turn 135+ | — | **6** |

The champion's value sits *below the opening floor of three*. It was bred at
4p 24×16 = 96 tiles per player; the promotion gate's deployment profile is
6p 74×46 = 567 tiles per player. `docs/EVAL.md`'s matrix table records that
champion at 120 maps per profile as **+51 on compact and −30 at deployment,
verdict retain stock** (`docs/AI_GAPS.md` §5 has the same shape as +58/−9 on an
older comparison).

Meanwhile the league — which plays six-player games continuously — has its
current leader at `city_target = 9.681` after 1011 six-player games (19.7%
outright wins against stock `advanced`'s 16.4%). ⚠ That is one entrant, not a
trend: the next-best-evidenced league genome, `g44-41`, sits at exactly the
stock 4.0. The defensible reading is only that six-player selection has not
pushed this gene *down* the way the compact breeding profile did.

`docs/GENOME.md`'s `gene_leverage` independently reports **expansion as the
one block where replacing the shipped values with uniform draws from their own
bounds *helped*** (−0.0305 cost, 1.8 SE), and names `city_target` explicitly:
"shipped `city_target = 4` against a 2..12 range whose random mean is 7 → the
AI may simply under-expand."

## The correction, and what it is not

`docs/GENOME.md` and `AdvancedAi::city_target_floor`'s doc comment both say
the gene "is only reached through `unwrap_or_else` when there is no plan — a
live `AdvancedAi` reads `plan.desired_cities` and never consults it."

**That is true of `advanced_production` and false of the path an adaptive
empire actually takes.** The `unwrap_or_else` site is real, and so is the
`BasicAi::cities` gate, and the second one is where the settlers come from.
Any sweep of this gene that concluded "identical to four decimal places" was
reading the fallback; that is why every value above six looked the same.

## The treatment

`AdvancedAi::plan_city_target` (default off, arm `advanced_plan_city_target`)
makes the delegation carry the plan. For the duration of the delegated call
only, the baseline governor is handed `plan.desired_cities`; its own gene is
restored before the turn returns.

It changes nothing else. The population gate, the production costs, the
one-settler-at-a-time rule, `settler_stop_turn`, the site search and the
opening-book branch are all untouched, and the substitution is invisible to
every other consumer of `w.city_target` — asserted by
`plan_city_target_substitutes_only_inside_the_delegated_call`.

This is deliberately *not* a new number. Every value it introduces is one the
agent already computed and already acts on elsewhere, which is the property
the retuning attempts recorded in `docs/GENOME.md` lacked.

## ⚠ REFUTED BY ITS OWN FIRES-CHECK — no outcome seed was read

```sh
cargo test --profile ci plan_city_target_fires -- --ignored --nocapture
```

Six maps at each of 4p 24×16 and 6p 74×46, flag off and on, reporting **cities
at end** beside the plan's own target. The criterion was fixed as the outcome,
not a mechanism bucket, for the reason `city_target_floor_fires` gives: a
bucket the treatment cannot move is not falsifiable in the helpful direction.

```text
[eval 4p 24x16]       plan_city_target=false  cities 2.17 / plan target 3.83   score 147
[eval 4p 24x16]       plan_city_target=true   cities 1.50 / plan target 3.67   score 122
[deployment 6p 74x46] plan_city_target=false  cities 4.83 / plan target 5.00   score 373
[deployment 6p 74x46] plan_city_target=true   cities 4.33 / plan target 5.00   score 329
```

**Cities fall in both profiles, and so does score.** The mechanism fires and it
fires the wrong way, so the pre-registered `ai_eval --matrix` at seed
65,000,000 was never run and the arm stays default-off with the null recorded,
on the `advanced_parallel_settlers` precedent.

### Why, and it is the useful part

The ramp *starts below the gene*. `desired_cities` opens at
`city_target_floor = 3`; stock's `city_target` gene is **4.0**. So for the
whole early game — the part that compounds — substituting the plan makes the
governor *more* restrictive, not less, and the late rungs of the ramp arrive
after the settler window has largely closed. The two numbers do disagree, and
the plan is not uniformly the larger one.

The census also prices the axis, which is worth more than the treatment was:

- **At deployment the empire reaches 4.83 cities against its own target of
  5.00.** It is target-limited, not execution-limited, which independently
  reproduces #569's recorded figure.
- **At the compact eval profile it reaches 2.17 against a target of 3.83.**
  There it is execution-limited. The same treatment therefore cannot be judged
  at one scale, and any expansion result read only at 4p 24×16 is reading the
  other regime.

A `max(gene, plan.desired_cities)` variant would not have this failure mode.
It is not proposed here: it was visible only after reading this census, and
adopting it now would be choosing the knob after seeing the result — the
failure `docs/GENOME.md` records for the opening-book sweep. If it is worth
testing it needs its own pre-registration and its own fires-check.

## What survives

The mechanism note above stands and is independent of the treatment: the
adaptive path's Settler gate is the flat gene in `BasicAi::cities`, and
`docs/GENOME.md`'s "`city_target` is only reached through `unwrap_or_else`" is
true of `advanced_production` and false of the path an adaptive empire takes.
Any future sweep of that gene must move it on this path or it is sweeping the
fallback again.

⚠ **And it does not generalise.** The obvious next question is whether the four
settle-site genes are live for the same reason. They are not: there are two
`settle_value` implementations — `BasicAi`'s reads the genes, `AdvancedAi`'s
uses hard-coded ring weights — and the Advanced settler path calls its own at
every site choice and every re-target, while the gene-reading one is reached
only from `BasicAi`'s own settler movement. `docs/GENOME.md`'s dead-gene
reading of the settle block stands. `city_target` crosses over through
*production*, not through siting, and this correction should not be extended
past the one gene it was measured on.

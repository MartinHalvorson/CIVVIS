# A forty-map screen cannot see a forty-Elo change

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

Three screens in this session came back with a positive point estimate and the
verdict `INCONCLUSIVE`:

| arm | maps | score | Elo-equivalent | verdict |
|---|---:|---:|---:|---|
| `advanced_engine_faith_price` | 24 | 56.2% | +44 | INCONCLUSIVE |
| `advanced_open_water_navy` | 40 | 56.2% | +44 | INCONCLUSIVE |
| `advanced` vs `advanced_v1` | 24 | 64.6% | +104 | INCONCLUSIVE |

`docs/EVAL.md` is full of the same shape going back months. The obvious reading
is that these treatments are close to parity. The question nobody had asked is
the other one: **at these map counts, what is the smallest edge this gate could
have promoted at all?**

## How it was measured

The gate itself now measures it. Simulate the whole gate — both conjuncts, the
`PROMOTION_MIN_MAPS` floor, the nonstationarity veto — on synthetic runs of a
given length whose maps break at a given rate, and bisect for the smallest true
edge it promotes 80% of the time. The break rate comes from the run being
reported, which is the only estimate available.

The power search wants the verdict hundreds of thousands of times and nothing
else, so it uses `gate_would_promote` rather than `paired_inference`: inverting
the betting test over a grid of candidate means costs about a thousand times
more, and the two agree by construction — the interval's `low > 0.5` is the
same statement as the challenger direction rejecting parity. A test holds them
to that on 200 random score vectors.

## What it measured

At the 28% break rate these deployment screens actually produce. The following
thresholds are **DISCOVERY ESTIMATES** from synthetic power searches (not
observed treatment effects): the first is for **40 maps**, and the second for
**200 maps**.

| maps | smallest edge promoted at 80% power |
|---:|---:|
| 40 | **+97 Elo-equivalent** |
| 200 | **+47 Elo-equivalent** |

**A 40-map screen needs +97 Elo to promote.** Every one of the three runs above
was measuring an edge less than half that. They were not close calls and they
were not weak evidence — they were **unasked questions**. `advanced_open_water_
navy` could not have been promoted by that run whether its true effect was +44,
0, or −44.

This also prices the honest next step, and it is not encouraging: even 200 maps
resolves only +47, which is barely under the +44 estimate it would be testing.
Screening a +40-class change to a decision is a several-hundred-map job, not a
forty-map one.

## What was decided

**Shipped.** Every verdict now carries a `resolution:` line naming the map
count, the break rate, and the edge that run could resolve. Printed on `PASS`
and `RETAIN` too, not only `INCONCLUSIVE`: a promotion earned on a run that
could barely resolve the edge it reports is also worth knowing about.

This changes no controller and promotes nothing. What it changes is what an
`INCONCLUSIVE` means to whoever reads it next, and the standing advice that
follows: **do not open a deployment screen at 40 maps expecting an answer about
a +40 change.** Two 200-map screens (`advanced_open_water_navy`,
`advanced_builder_survey`) were opened on this evidence and are recorded
separately.

⚠ The number is a scale and not a certificate. The break rate is taken from the
observed run and is itself noisy on a short one, which is precisely when the
line matters most. It is meant to stop `INCONCLUSIVE` being read as "no
effect", not to price the next experiment to the Elo.

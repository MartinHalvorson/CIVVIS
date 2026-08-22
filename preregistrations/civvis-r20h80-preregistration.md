# Pre-registration 2: strategic_r20h80 (strategic_deep) at n=300

Written BEFORE the run. Prior evidence, all reported regardless of outcome:

- seed 70000, 120 maps: 29 for / 7 against, gate PASS (Wilson low 50.2%,
  cleared by 0.2pp)
- seed 90000, 120 maps: 24 for / 8 against, gate INCONCLUSIVE (Wilson low
  47.7%). The effect replicated; the PASS did not.
- pooled: 240 maps, 53 for / 15 against, sign p = 4.1e-06

Power analysis, not data-peeking: at the pooled effect (~57.9%) the Wilson
lower bound clears parity from about n=200. n=300 is fixed here for margin.

- Challenger `strategic_r20h80` (review_every 20, horizon 80) vs incumbent
  `strategic`; `--pairs 300 --players 4 --seed 100000 --turns 200`.
  Seed 100000 is disjoint from 41000/52000/60000/70000/80000/90000.
- Decision rule: whatever `promotion gate:` prints at n=300 stands. A PASS
  supports promoting `strategic_deep` as a new builtin. Anything else does
  not, and is reported.
- No further seeds if this fails. The pooled sign-test evidence is already
  reported separately and does not depend on this run.

Note the companion failure being reported alongside: pre-registered
`strategic_r20` at n=400 reached e=1.97e4 and 46-12 on maps and STILL read
INCONCLUSIVE, because its smaller 54.2% effect needs about n=540 for the
Wilson bound to clear.

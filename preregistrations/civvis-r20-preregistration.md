# Pre-registration: strategic_r20 promotion attempt

Written 2026-07-26 BEFORE running, because the promotion gate's Wilson
interval is a fixed-n interval and stopping when it happens to clear would
be optional stopping on a statistic that does not permit it.

- Challenger: `strategic_r20` (review_every 20, everything else identical
  to `strategic`).
- Incumbent: `strategic`.
- Settings: `--pairs 400 --players 4 --seed 80000 --turns 200`.
  Seed 80000 is disjoint from every set used so far (41000, 52000, 60000,
  70000).
- n = 400 maps, fixed in advance. Chosen because Wilson clears parity at the
  observed 55.4% effect for n > ~325; 400 gives margin without being chosen
  after seeing the data.
- Decision rule: whatever `promotion gate:` prints at n=400 is the result.
  PASS promotes `review_every = 20` as the default. Anything else does not,
  and the run is reported either way.
- No re-runs on other seeds to find a better answer. If this fails, it is
  reported as a failure.

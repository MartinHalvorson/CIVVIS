# Turn-structure semantics A/B — preregistration (2026-08-06)

Question: does the simultaneous turn structure (PR #1280) distort play
relative to sequential, and is the drop rate healthy at scale?

- Design: 20 paired seeds (7312000–7312019), 6 majors, auto map for 6
  (74×46), 9 city-states, online speed, 150 turns, `AdvancedAi` fleet,
  ci-profile binary at PR #1280 head, serial driver, `soak --jobs` default.
- Metrics, per arm: winner score distribution (median/IQR), all-major score
  sum, cities and techs of the leader, victory kinds, NO-WINNER count; for
  the simultaneous arm: drop rate per game (dropped/planned), aborts.
- Comparisons: paired per-seed winner-score delta (median, sign test
  direction), distribution overlap; seat-0 win share vs 1/6 in each arm
  (commit-order advantage check).
- Decision rule: the mode is usable for throughput work if no game aborts,
  drop rate stays under ~5% median, and score distributions overlap broadly
  (no wholesale collapse). Elo/league stay sequential regardless (regime
  mixing is out of scope by design).
- One seed is never a result; 20 paired seeds is a first health check, not
  a final regime-equivalence claim.

## Results (2026-08-06, PR #1280 head + cursor fix, ci binary, serial)

- 20/20 paired games completed both arms; **0 aborts**.
- Drop rate: **median 4.5%** (3.6–5.7%) — inside the ~5% rule.
- World development indistinguishable: cities median 47.5 (IQR 45–52)
  sequential vs 48.0 (46–51) simultaneous; majors_alive median 6 in both.
- Victories: sequential 18 score + 2 religious; simultaneous 20 score.
  The two religious closes disappear — long missionary sequences are the
  most drop-vulnerable plans. Real regime difference; note for anyone
  running religion experiments in this mode.
- Seat-order advantage: none detected — seat 0 (Rome) won 3/20
  simultaneous vs 4/20 sequential; leader spread (Greece 7/20) is within
  n=20 noise.
- Wall clock (serial driver): median 36.4s → 41.6s per game (+14%); the
  parallel planning phase is the follow-up that repays this.
- **Decision: usable for throughput work** per the pre-registered rule;
  Elo/league/live remain sequential by design.


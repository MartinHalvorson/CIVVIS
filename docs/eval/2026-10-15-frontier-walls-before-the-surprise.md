# Earlier frontier fortification: preregistered experiment

Computer: MacBook Pro

The existing early-wall policy watches only the original capital. Native
Guayaquil in `civvis-20261015T164637Z` had an empty queue at turn 47, a stronger
Maya staging group nearby, and actual Ancient Walls offered for 40 Production
in six turns. It chose a Granary and then a Spearman; its first wall order
arrived at 67/200 health on turn 54 and the city disappeared at 55. The
modeled Walls/plaza journal churn did not cross native production transport.

Two later diagnostics restored the same archived turn-18 board, including
identical native seat, state, tiles and settlement-pressure exports. Baseline
`1d770be4c` lost to Culture at 134; pressure-aware `4f6c8e869` retired under the
unchanged production-rank rule at 150. Neither captured a city or won. Both
controllers and every captured input prefix were audited; this one pair does
not establish a strength gain. Guayaquil disappeared at 53 with near-zero
Loyalty and no damage in the baseline, and at 41 after combat damage with
healthy Loyalty in the candidate. These are different failure modes.

Walls first became available at turn 36 in both the original and candidate
histories. The candidate's thirteen-turn estimate was already too slow for
its turn-41 loss. The experiment therefore covers earlier warning, Masonry,
and construction, rather than assuming a late wall order saves the city.
Existing `border-parity-3` already implements a local garrison purchase. It
was off in the 282-tag profile; adding only that flag to exact frozen turn-47
and turn-36 requests produced identical complete order arrays. A nearby
garrison and the policy's reserve constrain it. No global policy is changed.

The new arm remains default-off. Baseline `d0567d312` registers the arm with
explicit no-op frontier methods. GitHub run `38785853149` reaches the intended
Masonry-goal and legal-Walls assertions after their fixture preconditions and
fails both: `None` instead of Masonry, and Builder instead of Walls. It stops
early after 1,087 other passing tests; its PR merge checkout includes the
concurrent airbase change, so this is not a full-suite result for the head.

The repair tracks each (owned non-capital city, peaceful rival) separately.
A visible non-recon land soldier within eight tiles and at least twice our
military power arms early Masonry and legal first Walls. A sighting lasts ten
Online turns. Friends, allies, minors, barbarians and active wars do not arm
the warning. An invested wall finishes after its warning expires; a one-turn
queue head or Settler within two turns finishes first. Final queue claims run
after ordinary production and paused-district claims. The original-capital
policy and garrison purchases retain their existing behavior.

Twelve focused controls cover the two baseline cases, the actual research
picker and observed-turn production caller, visibility and power limits,
unit classes, diplomatic exclusions, separate city memories, stale/future
sightings, the original capital, invested Walls and nearly finished Settlers.
Exact integrated source `89d1f01bb` passes all fifteen focused tests: twelve
frontier controls and three original-capital controls. Its full local suite
passes 6,080 tests with `RUST_TEST_THREADS=1`, 51 existing library ignores and
four ignored documentation examples. Source hashes remain unchanged. The
first parallel run passed 5,742 library tests and failed the unchanged restart
timing control at 55.8 ms against its 50 ms limit; that control passed alone
before the successful full serial run. GitHub `38787949153` independently
passes all 6,080 Rust tests; the job then fails only the missing firing-evidence
gate, which the following recorded block addresses.

The preregistered six-game firing screen, seeds `40630001..40630006`, completes
all 36 seats on clean source `89d1f01bb`. Its binary SHA-256 is
`dc1418d6721d3a1939c8c0f1166c791c8b90f43c8ff86c3b9c3187c8341c57d9`.
The actual header reads six majors, 74x46 Continents, nine city-states, Online,
250 turns, Emperor majors and Deity barbarians, with all victories. Only the
frontier gene varies; the other 624 genome bits are identical. Fourteen seats
draw it on and 22 off. The actual analyzer reports `~` (unresolved): win
contrast +7.79 percentage points with 9.21 points standard error, and score
share contrast +1.94 points with 1.47 points standard error. This satisfies the
repository firing-evidence gate; it does not establish a strength gain and is
not a native or paired-game result. The JSONL and analyzer output are committed
under `docs/gene_screens/fires/2026-10-15-frontier-walls.*`.

Native artifact validation, frozen order comparisons and execution remain
pending. The existing deployment rules, all 141 selected genes, their
retained-selection rationale and the compute-cost record remain byte-identical.
No survival benefit is claimed.

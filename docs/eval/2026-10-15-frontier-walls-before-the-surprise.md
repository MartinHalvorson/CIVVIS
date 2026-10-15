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
All targets compile, but these controls, full validation, firing evidence and
native execution remain pending. The existing deployment rules, all 141
selected genes, their retained-selection rationale and the compute-cost
record remain byte-identical. No survival benefit is claimed.

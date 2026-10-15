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

The new arm remains default-off. Its first checkpoint registers the arm and
two intentionally failing production-path controls: a visible stronger major
near a non-capital city must bring Masonry forward and claim legal first
walls. The original-capital policy remains a separate control. Implementation,
negative controls, fire evidence, full validation and native comparisons are
pending; no repaired native execution or survival benefit is claimed.

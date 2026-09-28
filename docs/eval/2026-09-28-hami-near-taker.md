# Unwalled Hami held by an exact staging ring

The King Online Gran Colombia game `civvis-20260928T193527Z` named Mongolia's
Hami as its first conquest objective. At turn 70 it had 375 military power
against Mongolia's 105 and several ranged units five tiles from Hami, but
`campaign_staged_for_war` held the declaration because the Man-at-Arms was
seven tiles away. Mongolia opened the war on turn 94. Hami was unwalled
through turn 104, yet our delayed siege did not capture it; walls appeared
on turn 105 and reached 200 HP by turn 113.

For committed Domination only, an unwalled objective may now pass the final
staging check with at least three ranged units on the normal ring, a healthy
30-strength melee taker six to eight tiles away, and a local strength ratio
of at least 1.60. The board's city-strength bill, defensive requisitions,
affordability, and diplomatic gates still apply. Standing walls retain the
ordinary requirement for a taker already on the ring.

The focused regression checks this opening, rejects it outside committed
Domination and when the city has walls, then checks the ordinary fully staged
declaration. This is a launch-readiness improvement, not evidence of a live
capture or victory; the next game must measure those outcomes.

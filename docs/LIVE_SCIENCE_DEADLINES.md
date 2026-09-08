# Science development deadlines

The retained Emperor game `civvis-20260903T131248Z` used a 650-turn cap and
lost to rival science at turn 206. Its capital learned Rocketry at 148,
completed a Spaceport at 177, and completed the Moon Landing at 205. Extending
the cap provides more observation time; it does not slow down the opponents.

The shared development clock now takes the earlier of the configured cap and
500 Standard-speed turns. Online specialization therefore starts at turn 125
even with a 650-turn cap, while a deliberately shortened game still specializes
earlier. Unlimited games keep the existing Industrial-era rule. This fixes a
cap-dependent delay in expansion, district valuation, and victory development;
it is not evidence that the whole launch delay is eliminated.

`chase-every-boost-2` also prices the active study's remaining research as the
maximum possible boost saving, and includes the candidate item's production
multiplier when checking whether its trigger can finish in time. Its bounded
production premium, current-study restriction, and cross-city claim guard stay
in force. No tournament seating, sampling, or default gene selection changes.

The ordinary production valuation also includes each item's multiplier in its
completion time and score divisor. Colonization, Ilkum, Veterancy, and launch
project bonuses therefore affect the choice they accelerate, including whether
it fits before the deadline. Previously those builds were priced at the city's
unmodified rate even though the engine produces them faster. The frozen
pre-victory-planning controller retains its historical calculation.

Gold purchase scoring cancels that build-time adjustment before comparing
purchase candidates, so a production bonus does not inflate willingness to
spend gold on the same item. The separate purchase-policy treatment retains
its existing behavior.

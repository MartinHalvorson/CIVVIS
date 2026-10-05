# Guarded productive Builder jobs

This opt-in prototype reserves an existing healthy guard for one currently
worked, production-improving land job inside a raider's reach. The ordinary
emergency Builder support planner runs first, and existing Settler guards
remain reserved. No new Builder, military unit, construction quota, engine
cost/yield, protected pin or deployed genome is introduced.

The support plan validates a guard walk, a Builder walk and the actual
improvement together on one exact native clone. Both units must arrive this
turn and the improvement must remain legal. The job must add net Production
without losing Food. A final own-city military garrison is retained, a threatened
job city is excluded, and the guard must satisfy the existing HP and expected
strike survival bars. Survival is checked again after the improvement so a
changed feature cannot leave the guard protected only by its old cover.

Only one additional productive pair is reserved per frame. Existing emergency
pairs keep priority; guards already promised to a civilian cannot be borrowed.
The ordinary military pre-passes see the same guard reservation. Actual joint
movement and improvement use the already checked action sequence, and a failed
revalidation cannot send one half of the pair onward. The existing support
planner's old pairs carry no productive job and keep their behavior.

The first compile caught two Name-value type errors before any tests or games.
Its own compiler was stopped after those reported errors; this is not a test
validation. A corrected suite includes actual improved-city Production, the
full unit driver, Settler binding and wounded-guard refusal, final-garrison
preservation, and invalidated-pair atomicity. Results remain pending until the
command completes. The feedback suite uses opt-level zero only for fixtures;
all game comparisons will use a separately frozen normal optimized build.

The protocol assigns eight already-consumed Emperor/Deity maps. It requires
accepted and actually completed escorted work before fresh evaluation. No
fresh pilot or confirmation has been played. The native model alone cannot
establish Firaxis parity or host citizen assignment behavior.

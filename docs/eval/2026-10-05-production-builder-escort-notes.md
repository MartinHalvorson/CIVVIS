# Guarded productive Builder jobs

This opt-in prototype reserves an existing healthy guard for one currently
worked, production-improving land job inside a raider's reach. The ordinary
emergency Builder support planner runs first, and existing Settler guards
remain reserved. No new Builder, military unit, construction quota, engine
cost/yield, protected pin or deployed genome is introduced.

The support plan validates a guard walk, a Builder walk and the actual
improvement together on one exact native clone. Both units must arrive this
turn. The improvement executes immediately when legal. If a hill spends the
Builder's final movement, a separate native model copy checks the operation
with a fresh allowance; the real sequence executes only the legal joint walk
and the next frame must revalidate the job and guard before improving. The job must add net Production
without losing Food. A final own-city military garrison is retained, a threatened
job city is excluded, and the guard must satisfy the existing HP and expected
strike survival bars. Survival is checked again after the improvement so a
changed feature cannot leave the guard protected only by its old cover.

Only one additional productive pair is reserved per frame. Existing emergency
pairs keep priority; guards already promised to a civilian cannot be borrowed.
The ordinary military pre-passes see the same guard reservation. Actual joint
movement and any immediate improvement use the already checked action sequence, and a failed
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


The first complete focused run passed thirteen tests and failed three new
positive-route tests; no comparative games were played. The roadless-hill
fixture exhausted the Builder's movement before improvement. The policy now
permits only the checked joint walk in that case, retains the guard under the
same survival bar, and requires a new frame to check and complete the job. A
separate road fixture covers immediate completion, and an actual native turn
boundary covers the two-step operation. The failed run is retained separately.


The first checkpoint's Cargo CI failed the same three positive-route fixtures.
Its collaboration gate also caught the private flag filed in the wrong
alphabetic append range. The field is now named builder_productive_escort in
the existing b range; public method names stay the same. The second local fixture run completed with fifteen passes and two failed
immediate-completion expectations; its roadless-hill turn-boundary test passed.
The normal-library build was stopped before completion to correct the known
append-range policy failure before freezing. The subsequent fixture compile
was stopped before completing to correct those unchanged expectations. Neither
stopped command is passing validation.
No comparison games have run.


The immediate-operation fixture now starts the Builder on its worked job and
checks that the newly arriving guard protects its legal improvement. The full
unit-driver fixture checks both real walks and, when movement is exhausted,
the actual next native turn before requiring the mine. A road alone did not
establish sufficient remaining movement in the previous fixture. The policy
continues to demand engine-legal operations; expectations now cover both timing
cases. This cfg(test)-only fixture edit is not an input to the concurrently
running non-test optimized library build. Runtime source remains unchanged.

The corrected focused feedback suite completed successfully: 17 passed, zero
failed, 4,356 filtered out. The separate append-point policy suite passed all
14 tests. These fixture results do not establish a production gain. The normal
optimized library is still compiling; no comparative games have started.

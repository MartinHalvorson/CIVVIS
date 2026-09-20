# Use an earned Prophet outside the religious lane

Native King domination game `civvis-20260920T085522Z` lost to Babylon's religious
victory on turn 168. Gran Colombia held Great Prophet 2424852 from turn 77 to
the end with `prophet_pending=true`, but issued no religion-founding order.
The Prophet reached Popayán's Holy Site at (18,33) on turn 106 and stayed there.
The turn-112 snapshot confirms the completed, unpillaged Holy Site underneath
it. Only two religions were founded in this four-player game.

`AdvancedAi` clears `pursue_religion` for an explicit domination target.
`BasicAi::research` also used that flag to gate spending an already-earned
Prophet. That discarded the opportunity after paying its acquisition cost.
Founding now depends on the pending Prophet and the existing game legality
checks. Religious investment, missionary spending, and the victory target keep
their existing controls.

The regression disables religious pursuit and entry into the Prophet race,
then supplies an already-earned Prophet and an available belief pair. It failed
before the change and passes afterward, without changing either strategy flag.
A second test checks that a seat without a Prophet does not found. The existing
test for exhausting preferred belief pairs remains covered by the shared fixture.

Validation:

- All 18 Prophet-related tests pass.
- Full `cargo test --profile ci --locked`: 3,673 library and 204 binary tests
  pass; 49 library and four documentation tests are ignored.
- Same-base replay of all 479 recorded decision frames: the baseline emits no
  religion orders; the patched build first emits `BELIEF_WORK_ETHIC,BELIEF_TITHE`
  at turn 102 and also emits it at turn 112 with the Prophet on the Holy Site.
- Both replay processes exit successfully. The frozen host observations do not
  execute the new orders, so this proves order generation, not native founding,
  religious defense, or a counterfactual win. Existing host legality checks can
  reject the earlier request before the Prophet arrives.
- No engine mechanics or control-mod API changed; an engine soak is not
  applicable to this decision gate. Live founding remains to be observed in a
  later native game after deployment.

Local evidence: `/tmp/civvis-earned-prophet-replay/` contains the immutable event
snapshot, both order streams, reasoning logs, and `comparison.json`. The native
run's `unused-prophet-review.json` records the original Prophet trajectory.

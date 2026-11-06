# Read-back controls for order queue timing studies

The launcher can optionally set the mod's existing idle-probe, no-op-probe and grace thresholds. The running mod publishes the effective values in its `seat` event. An explicit override qualifies only when the receiver reports a numeric matching value and the order queue is enabled. Missing readback from an older mod is insufficient for an explicitly requested timing experiment; without these overrides, older seat reports remain compatible.

| Optional flag | Mod config key | Effective seat field | Existing mod default |
| --- | --- | --- | ---: |
| `--order-queue-idle-probe-ticks` | `OrderQueueIdleProbeTicks` | `order_queue_idle_probe_ticks` |2|
| `--order-queue-noop-probe-ticks` | `OrderQueueNoopProbeTicks` | `order_queue_noop_probe_ticks` |8|
| `--order-queue-grace-ticks` | `OrderQueueGraceTicks` | `order_queue_grace_ticks` |30|

All three launcher defaults are `None`: absent overrides are omitted from the baked mod config. The mod's movement consumers and their fallback values are unchanged. Flags require positive integers and an enabled queue; invalid CLI requests fail before `play` is called. Direct config callers receive the same validation. Requested values are retained in fresh and attached run summaries, alongside the raw receiver seat report. An attach checks the loaded mod's settings; it does not change them.

Fresh and attach qualification use the existing `seat_matches_requested` contract. Explicit mismatches, missing fields, strings or booleans cannot establish a timing treatment. The existing fresh-run seat refusal stops a mismatched attempt, and summaries cannot label either flow as configured on these inputs. Numeric integral receiver values such as `8.0` agree with an integer request of8. No override means no new queue-timing qualification requirement.

The underlying queue uses the host path and activity reads. The shipped `Base/Assets/UI/WorldInput.lua:961` reads `local pathInfo : table = UnitManager.GetMoveToPathEx( kUnit, endPlotId );`; its busy-core guard is at884. This change adds no game operation, path rule or asynchronous-settlement claim. `CivvisQueue.drain` and `CivvisBoard.moveNoop` were checked byte-identical before and after the readback addition. The new fields use the same `tonumber` and fallback contract as those consumers.

The need for a controlled experiment comes from saved native Indonesia observations: in one University-policy pair, frame18/1 had different movement readback before the first logged policy activation. An offline mocked-host reproduction of the shipped queue showed the two-tick idle probe releasing an opening move watch before a supplied later arrival, while the existing setting8 held until that arrival. Neither that example nor the broader callback census proves an optimal threshold or a native strength gain. A reduction in the `move_noop` label alone would not establish better gameplay.

For a future approved pilot, an explicit baseline would request2/8/30 and a candidate8/8/30 for idle/no-op/grace respectively, with a frozen controller, mod, genotype and all other settings. The receiver values must be verified. Measure actual next-state movement, stale-state replans, dependent action results, wall time and natural outcomes, rather than treating early no-op records as terminal failures. This change supplies the configuration and qualification path; it does not run that experiment or choose a new production default.

Validation: all6 new focused Python tests and the full324-test `test_civ6_play.py` suite pass. The Lua5.1 test drives the actual exported `CivvisSurvey` function from the shipped agent for defaults, explicit overrides and numeric-fallback cases. Tests do not launch or attach to Civilization VI, acquire its game lock or write installed mod settings.

No native harness, lock waiter, saved continuation, lane setting, force-file or halt flag was changed. Native testing still requires the operator's explicit approval.

# Siege resource purchase

In native match `civvis-20260921T233717Z` (`48e2388ce284d46fd3cae23bbdef5f0bff0df2b0`), Metal Casting was known by turn 100, but Niter income stayed zero until turn 140. The deposit at Civ VI (35,16), adjacent to Bogotá's territory and within its purchase radius, remained unowned until turn 146 and was mined at 154. Turn 140's first income instead came from a minor's deposit. The match ended in a Culture loss at 199; this change has no native victory result.

During a Domination conquest, the planner can now buy a legal first deposit required by an existing siege unit's immediate researched upgrade. It requires zero stock and income, a connectable resource, a charged owned Builder within four hexes and four traversable steps, and the existing civilian destination risk check. Both the plan and current board must report no threatened home city. It preserves 40 Gold plus six turns of any deficit. An owned connectable or already installed matching mine, including a pillaged mine, prevents a redundant purchase. Emergency and parity purchases still run first.

This deliberately permits spending below the ordinary reserve and 200-Gold surplus requirement. The route check establishes reachability; the destination check does not prove every path tile safe. The heuristic buys a plot, not a mine or an upgraded army. The native bridge retains its command legality checks.

## Paired native recording

The first prototype incorrectly required a target city at the purchase phase, where the recorded planner had none, and changed zero actions. Removing that unrelated requirement produced the intended order while retaining the conquest and home-defense guards. Diagnostic execution established that at turn 111's actual purchase phase the bank was 156 Gold, the quote 100, and income 17.2031; earlier estimates from the raw snapshot were not the planner's price or treasury.

The final integrated comparison replays the same 446 turn/frame boundaries through turn 160, using the recorded forced genome and fixed input. Baseline source is `23360191c16810b4ce4f00eac66af446e41286e1`; candidate is `1b49bbafaeb17cc3ca7bce6a15e74cbf8890dbf8`.

- Input SHA-256: `0a2fa8078238cc4380c5564ce7bbad847bbd33a91c7842949c892dbd6fec3dde`.
- Baseline binary SHA-256: `4fdd8c727df3c4f9714da5c5095ad65e20ed841187f868828135855e41270a2e`.
- Candidate binary SHA-256: `2a542c9d66cc1f3b01ad3dcbad27d8554aca7241b0e5360c1f6d316c66eccdeb`.

There are two changed exported frames and one changed internal-action frame. At 111/0, the candidate adds Bogotá's `buy_plot` for (35,16), modeled cost 100. At 112/0, the unchanged recorded future reports `plot_not_owned` for that counterfactual proposal and adjusts receipt counts. No other exported or internal actions change. This reproduces the corrected prototype's comparison exactly. It does not establish host acceptance, an earlier mine, an upgrade, a capture, or a prevented loss.

## Validation

- Full integrated Rust suite: 4,150 passed, zero failed, 53 ignored.
- Four focused tests pass, exercising the actual spending planner, absent target city, purchase vetoes, and an owned pillaged mine.
- Fourteen treatment append-integrity tests pass.
- Eight integrated four-player smoke games complete, seeds 370500–370507, using `--turns 180 --start-seed 370500`. These are stability checks, not Domination win-rate evidence.
- Changed-line Rust quality passes against exact integrated baseline `23360191c`; scoped changes pass whitespace checks.
- Integrated CI paired cost: -1.43% per completed turn, IQR 0.72 percentage points, resolution ±0.48%; within the +8% budget. Final ready-head checks remain enforced by shipping.

Reproduction artifacts are local under `/tmp/civvis-siege-resource-replay`; the final comparator output is `/tmp/civvis-3705-integrated-comparison.json`. No runtime recording or diagnostic instrumentation is committed.

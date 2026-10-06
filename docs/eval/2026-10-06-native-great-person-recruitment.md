# Native Great Person recruitment and activation

The native run `civvis-20261006T011806Z` offered
`GREAT_PERSON_INDIVIDUAL_MASARU_IBUKA` at turn 226 frame 0. Its saved
baseline decision and complete emitted orders were independently reproduced
exactly at that frame in the earlier income-policy investigation. The first
recorded action recruited a Merchant. Reconstructing and applying that action
raised modeled Gold from 200 to 380 by retiring the unrelated local offer,
Marcus Licinius Crassus (three 60-Gold charges). The actual native game recruited
Masaru Ibuka as a physical unit with one charge, initially unable to activate.
The subsequent snapshot showed 60 Gold after a 140-Gold Anti-Air Gun upgrade,
without the invented 180-Gold reward.

The modeled reward crossed the 350-Gold wartime reserve in that frame and let
Total War displace Economic Union. This supports the recruitment error and its
recorded budget consequence; it does not establish a counterfactual outcome.
The earlier income-policy candidate's first changed frame failed its native
fidelity gate and was rejected. This correction addresses the underlying
invented reward rather than changing that policy guard.

Primary local sources distinguish recruitment from activation:

- Shipped `Base/Assets/UI/Popups/GreatPeoplePopup.lua:891`:
  `UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.RECRUIT_GREAT_PERSON, kParameters);`
- Repository controller `tools/civ6_control/mod/CivvisControlAgent.lua:12397`
  requests the same operation for the actual available timeline individual.
- The controller's separate unit activation path at line 14451 calls
  `commandUnit(unit, CMD["UNITCOMMAND_ACTIVATE_GREAT_PERSON"])`.
- Its unit export at lines 7507–7511 reads remaining action charges and
  `UnitManager.CanStartCommand` for activation independently of recruitment.

The intended change applies only when the player carries an authoritative
`live_great_person_offers` set. A successful recruit or patronage purchase
consumes its modeled points/currency and the currently exported class offer,
and increments the recruitment count.
It preserves the exported named individual and does not retire a different
local person or apply activation effects, historic moments, district growth,
or religion pressure. Later native observations remain authoritative about
actual recruitment and activation results. Ordinary headless games and older
exports without this field retain the existing immediate-retirement model.

This deliberately retains existing live offer blockers and patronage pricing.
It does not add a physical Great Person to the simulator, replace the native
activation/movement controller, or repair a live class whose entire local
roster is exhausted. Unknown native individuals in supported classes remain
recruitable through the existing legality and observed-cost paths.

Twelve new focused cases pass. On the original production code, the initial
ten-case gate had two passing controls and eight expected failures: phantom
Gold, unactivated eurekas/promotions, and repeated purchases of one native
offer. The first full-suite run exposed four failures caused by omitting the
existing recruitment counter. That count is now retained; the four existing
tests remain unchanged. Final full validation is in progress on main
`6e6b32deb` plus the corrected native guard.

An isolated copy of the exact recorded private source `32c265964` was checked
against all 2,322 archive files; only `src/game.rs` differed by this guard.
An action-level probe on the actual 226f0 prefix confirms recruitment remains
legal (671 points, cost 660), Gold remains 200, and the 350-Gold reserve stays
in force. The provisional frozen-history replay used the original saved native
binary and all 159 original forced tags. Its first changed frame, 42f0, fails
the complete-order fidelity gate: the original entire decision matches the
saved native decision, but an extra native FORTIFY order is absent from replay.
This is a real emitted-order discrepancy and is retained as a failed gate.

The independent 226f0 frame does match the entire saved original decision and
complete orders. On that matched frame the provisional candidate avoids
Economic Union removal and the emitted Total War deck transaction. These are
conditional frozen-history findings, not native counterfactual outcomes. The
provisional replay lacks the later recruitment-counter correction; it is
retained separately, and final exact-source replay is in progress. No candidate
native action or win-rate improvement has been established.

Evidence artifacts:
`~/civvis-tactics-results/2026-10-05/culture-building-reservation/native-merchant-phantom-gold-226f0.json`
and `~/civvis-tactics-results/2026-10-06/native-great-person-recruitment/`.

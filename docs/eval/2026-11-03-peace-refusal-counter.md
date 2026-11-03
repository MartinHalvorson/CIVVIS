# Keep a refused peace session for its counter

**Interactive peace refusals now use a bounded counter wait.** Previously, the peace branch closed its `MAKE_DEAL` session immediately after the rival refused. A following statement in that conversation then had no registered session and could not reach the shared counter handler. The alliance branch already retained ownership for that sequence.

The peace branch now records the refusal, holds the existing session for four seconds by default, and sends `REJECTED` before closing if the same rival's counter arrives. Wrong-session, wrong-statement and our own statements cannot consume the wait. If no counter arrives, the existing closer releases it as `counter_not_sent` without counting an unanswered offer or holding the next turn. `PeaceCounterHoldSeconds = 0` restores immediate close.

This does not change accepted-peace equality checks or the host `at_war` readback used to prove enactment. It does not enable interactive peace sessions under the operator's current `trade` mode.

## Native observation and limits

The actual play log for Immortal Culture `civvis-20261103T002240Z` records `deal_sessions=trade`. Two peace requests, at 97 and 102, were reported submitted with zero concession, but no peace session opened and neither got a `peace_response`. The next-turn order checks reported `no_peace_response`. Sumeria's war was first observed at 89 and remained observed through the final turn 182. Trade sessions with another rival opened and answered normally nearby.

Those observations identify the direct transport used in that game; they do not show that an interactive peace offer would have been accepted. The historical October 3 peace-freeze CSV cited in source commentary was unavailable here and was not newly verified. This patch has **offline callback-sequence evidence**, not a native peace-session reproduction, an enacted treaty or a win-rate result. Preserve the operator transport policy while studying that path.

Shipped basis: `Base/Assets/UI/DiplomacyDealView.lua:607` defines `OnRefuseDeal`; line 627 sends `DealManager.SendWorkingDeal(DealProposalAction.REJECTED, localPlayerID, otherPlayerID)`. The change reuses the existing registered-session counter handler and closer rather than treating a refusal as an accepted peace.

## Validation and artifacts

The unchanged extended session test produced seven failing checks against the original handler. The candidate passed all 172 checks, including refusal/counter order, correlation, no-counter cleanup, zero-hold compatibility, direct trade-only peace, accepted/adjusted peace readback and the closer. All 114 discovered `*_test.lua` files passed under separate `lupa.lua51` processes; all 139 installer tests passed.

The initial isolated fixture copy omitted `CivvisControlAutoClose.lua`: the candidate passed its first 144 checks and then could not load that helper. That delivery failure and both logs remain retained. The corrected comparison copied every discovered production Lua helper and reproduced the original seven failures versus the candidate's 172 passes.

The adjacent JSON binds the archive hash and member count; its internal `SHA256SUMS.json` binds every other member. The archive retains original/candidate handlers and tests, red/green and all discovered Lua logs, installer output, the patch/source manifest, shipped-rule lines, and hash-bound native slices. Native observation extraction is read-only and makes no game commands.

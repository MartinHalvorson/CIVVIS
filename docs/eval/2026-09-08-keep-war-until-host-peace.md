# A peace offer must not erase the live battlefield

In `civvis-20260908T204713Z-cont2`, turn 175 frame 0, the science controller
logged "Ending the defensive war with India" before ordering crossbowman
6225944 out of Ostia (31,12) onto the coastal Harbor (30,12), and ordering
bombard 7864340 to fortify on the Commercial Hub at (29,12). Both died that
turn. Tank 10682372 was already visible at (30,13), 87 HP and combat 85.
The bombard had 100 HP, combat 45 and two moves.

A one-shot replay of the recorded opening frame reproduced both orders.
Adding `strike-reach` preserved both orders: that gene cannot repair a
threat field whose opponent is no longer considered at war. The replay's
active treatment header matched the original, including battle-planner-2.

The actual host submitted peace on turn 174; its `peace_request` row says
`submitted: true`, not accepted. No new peace order was sent on turn 175.
The turn-176 opening state still reports rival player 1 at war. The planning
model nevertheless called `Game::apply(MakePeace)` on turn 175 and cleared
its war before the military phase.

On the mirrored seat, the diplomacy pass now records its peace offer and
continues fighting until an authoritative host frame ends the war. The
existing `PlanReport::peace_offers` path in `civvis_orders` already converts
that intent into an outbound host peace request, including retry cooldown
and tribute handling. The native simulation retains its existing peace
behavior. The pending offer does not set a speculative peace cooldown or
clear the controller's war history.

The regression checks the offer remains available to the bridge, war and
attack envelopes remain active, repeated unaccepted plans keep defending,
and an authoritative accepted state stops further offers. The existing
native-science peace regression remains the control.

Validation uses a controlled live-seat regression, with native peace as a
control. Restoring the old behavior makes the new regression fail because
war is cleared. The fixed test also checks the public `plan_report()`
contains the outbound peace intent.

Replay limitation: the historical runtime reproduced the bad orders, but
current main (`65df99458`) already chooses different orders on this saved
frame. With and without this guard on that same base, the orders are
identical: the crossbow fires from Ostia and the bombard retreats to Ravenna
(28,14). Neither current-base replay chooses peace, so neither emits a peace
order. These safer orders are not evidence of this patch's effect. The
regression specifically isolates the still-present offer/acceptance defect;
no live survival improvement or gene promotion is claimed.

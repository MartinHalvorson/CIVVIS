# Recovering after live order refusals

An explicitly refused production request releases its same-turn queue lease
for a different item the host has not refused. Successful or still-pending
queues retain the first request, and the replacement becomes the owned queue.
The mirror already excludes `refused_production` items from its legal menu;
the bridge now lets that alternative reach the host promptly.

A MOVE_TO that the host reports as `host_noop_no_path` while the unit remains
at its starting tile gets individually planned steps on the following turn.
Known terrain is not marked impassable by this retry. The existing exact fog
probe remains the separate mechanism for learning unknown impassable plots.
The retry expires after one turn or ceases to apply when the unit has moved.

Buy and sell share one working deal per rival, so at most one is emitted per
rival per turn, including the first frame. A different item or asking price
does not replace the pending request. Other rivals remain independent. Host
cooldown, expiration and actual deal acceptance retain their own authority;
an emitted request is not reported as income or an accepted peace treaty.

The contract is visible in `CivvisControlAgent.lua`'s sell and buy branches:
both consult `CivvisTrade.pending[subject]`, then `trade.asked[subject]`, before
building an outgoing working deal. Its shipped reference is
`DiplomacyDealView_Expansion2.lua`, `OnClickAvailableResource` and
`OnClickAvailableOneTimeFavor`. This change only schedules bridge requests.

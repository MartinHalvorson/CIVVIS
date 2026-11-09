# Russia: a Loyalty policy request needs an occupied seat

Native Russia run `civvis-20261109T210123Z` missed a confirmed free policy
window at turn 132 while its only city, Yaroslavl, had 80.1875 Loyalty and
was losing 9.125 per turn. The city had an established Magnus and a completed
Lavra. Its six-card Monarchy deck kept Merchant Confederation in the
diplomatic seat. The later saved-game diagnostic
`civvis-20261109T211656Z` resumed this campaign at turn 140, observed the
city fall from 22.8125 to 6.8125 Loyalty at -8 per turn, and recorded our
defeat at turn 143. This diagnostic is a continuation, not another new game.
The original run's operator retirement at 140 remains censored.

## Captured-read counterexample

The original t132 decision captured 18 identical reads of snapshot 279.
The reconstructed input is 17,472,564 bytes, SHA-256
`8b8d66dfd0a50aadf56048439c1490e744dc6cbf7cd774f5cf80cf234044df9c`,
and is an exact prefix of the retained native event ledger. Replaying the
same frozen binary with the recorded forced genome, once without and once
with `--with loyalty-rate-alarm`, confirmed the treatment readback changed.
Both runs nevertheless produced the same political actions: promote Amani,
remove Colonization and Discipline, and slot Limes and Urban Planning.
The original binary's stamped source revision `39374cfea82c` and this task's
base have byte-identical `src/ai/advanced/policies.rs`.
These resumed one-shot arms do not reproduce persistent controller history;
their complete native action lists match each other but differ from the
original live decision.

The on-arm journal put Praetorium first in the religion portfolio. Its
ordinary replacement rule protected every wanted card, including Merchant
Confederation, so the request could not use the occupied diplomatic seat.
The prior regression had free capacity and did not cover this case.

At turn 140, a separate fixed-input replay with the same repair enabled
produced no policy actions. The host quoted a 65-Gold unlock, rather than a
free change. The bridge correctly requires a confirmed free window and does
not implement paid unlocks. Removing that gate would be incorrect:
`Base/Assets/UI/Screens/GovernmentScreen.lua:1589-1592` explicitly requests
`PlayerOperations.UNLOCK_POLICIES`, whereas directly requesting policy
changes can bypass the game's payment gate. This change uses the existing
free-window path only.

## Change and limits

When the existing `loyalty-rate-alarm` repair requests Praetorium for a
governed city within its existing 20-turn measured runway, the policy pass
may replace a wanted diplomatic card. Other typed cards and their wildcard
capacity remain protected. Culture-defense cards, a timed Nobel Peace
Favor card, Communications Office, and a paying card needed by an active
bankruptcy drain are not donors. Ordinary card availability, deck-fit and
host-window checks still apply. The replacement journal now explains the
emergency rather than calling a wanted donor unwanted.

The installed Gathering Storm source
`DLC/Expansion2/Data/Expansion1_Policies.xml:27` gives Praetorium a
diplomatic slot and Recorded History prerequisite. Lines 171-175 bind its
Loyalty modifier to cities with assigned governors; lines 661-665 set its
Amount to 2. This is a +2 Loyalty buffer, insufficient by itself to stop the
recorded -9.125 or -8 decline. Neither the fixed-input replay nor unit tests
establish that the city survives or that religious win rate improves.

The repair remains default-off; no experiment slot or live arm is changed.
The narrow fix does not promote the broader Loyalty repair, which has
other settlement, production and governor consumers.

Regression coverage exercises a completely occupied native-shaped deck,
retention against the paying envoy card on a second pass, the off arm,
stable/recovering/distant decline, absence of a governor or the prerequisite,
a closed host window, occupied wildcard capacity in Oligarchy, and a
bankruptcy drain that must keep its paying card.

Local diagnostic artifacts are under
`~/civvis-tactics-results/2026-11-09/russia-last-city-loyalty/`, including the
sealed original turn-140 read and `t132/` replay inputs, outputs, readback
and journals. They are evidence files, not committed runtime artifacts.

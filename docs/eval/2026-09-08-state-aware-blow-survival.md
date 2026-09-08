# Survival after a coordinated strike sequence

`doomed-blow-veto-2` is an opt-in alternative to the repaired original veto.
The original classifies each shooter on the opening board. This version
recalculates every planned striker's expected incoming damage after each
friendly kill, refunds exposure costs for enemies removed later, and selects
only terminal sequences whose expected replies spare all strikers.

An unsafe intermediate sequence remains searchable: a later friendly strike
may remove the retaliator. Directly fatal melee exchanges are refused, and
the existing wounded-finisher rule remains. The verifier then replays only
accepted blows on a fresh clone and checks their resulting positions again.
A dropped rescuing strike therefore cannot leave an unsafe accepted prefix.
Unselected shooters with no individually surviving strike retain the retreat
handoff. The versions are mutually exclusive and both remain opt-in.

Focused evidence covers a two-strike combination that rescues both attackers,
refund of the first striker's obsolete exposure penalty, refusal of an
incomplete unsafe plan, refusal of a directly fatal profitable sacrifice,
and refusal of an unsafe high-value choice despite another survivable choice.
The full battle-pass tests also cover the retreat handoff. The verifier test
shows the old behavior retaining an unsafe prefix when the rescuing strike
is dropped; version two rejects that prefix.

The saved cont3 turn-121 pikeman frame retreats to (12,23) and fortifies with
version two, matching the repaired version one's protection of that case.
This is a one-shot replay, not proof that the host executes the retreat.
Expected native damage, finite beam width and host refusal remain limits;
this change does not guarantee survival under every random roll or partial
host execution.

Initial comparisons were run on base5652bda8f. Before final evaluation,
#3243's movement-risk changes are being integrated and the comparisons
repeated on that shared baseline. Final tables and validation follow below.

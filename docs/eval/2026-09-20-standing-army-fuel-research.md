# Standing-army fuel research

## Native evidence

The King / Gran Colombia / Domination verification continuation
`civvis-20260920T122005Z-cont4`, pinned to `d9ee52e8754e5204ccbfda74a6194b4cd7b92776`,
ended at turn 239 in a loss to Sumeria. Its summary records zero cities taken
and four lost. This is a native game, separate from the simulated spectator.

A granted Giant Death Robot appears at turn 190. It has no upgrade successor,
so the existing wartime modernization goal cannot help it. At turns 210–239 it
has 35 HP; several frames show it fortified on owned territory, while the
controller repeatedly expects healing. There is no positive Uranium stockpile
in the examined exports. The exporter queries every strategic resource, without
a revelation-tech filter (`CivvisControlAgent.lua:8955–8975`).

The shipped Gathering Storm `Expansion2_Units.xml:436` specifies
`ResourceMaintenanceType="RESOURCE_URANIUM" ResourceMaintenanceAmount="3"`
for `UNIT_GIANT_DEATH_ROBOT`. `Expansion2_Moments.xml:180–188` supplies three
Uranium per turn for the Automaton golden-age dedication, which this seat has.
Thus no reserve does **not** prove that upkeep went unpaid. Nor does the frozen
trace isolate the exact cause of failed healing. No healing rule is changed.

The candidate makes revelation of an exhausted maintenance resource a research
priority for an already-fielded Domination army during a major war. It stops
when the reveal technology is known; connection, trade and healing remain
separate problems. A resource-consuming unit received before its normal unlock
is a concrete demand, even if it is the only such unit. The candidate retains
urgent defense and appointed land/air breakthrough priority and permits only
legal prerequisites on the selected supply path outside the normal era window.

## Measurement

Pending focused tests and a complete frozen-observation comparison of cont4.
Alternative requests on frozen observations do not establish host execution,
resource acquisition, restored healing, a city capture, or victory.

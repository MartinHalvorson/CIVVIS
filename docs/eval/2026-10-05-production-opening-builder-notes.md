# Public opening production experiments

The first prototype swaps the public Settler and Builder opening slots. Both
arms use `targeting(Domination)` and `enable_live_bridge`, then reweight a clone
of the public current weights, with all settings read back. No private pin,
weight bundle, controller, engine costs or yield rules are changed.

All eight consumed maps were replayed with sixteen zero-exit executions. The
first Emperor map completed a Builder on turn 14 versus 21 and a mine on turn
21 versus 40. Across the whole diagnostic, however, scored turn-75 Production
fell 38.05% at Emperor and 28.63% at Deity. The fixed death/early-end zero rule
counts one candidate Emperor game ending on turn 64 as zero, though the focal
player was alive. Deity candidate survival at turn 75 also fell from four to
three. Science and empire size fell at both difficulties. These consumed maps
are diagnosis, not fresh strength evidence. The earlier Builder is rejected.

A second independent opening variant retains Scout repair, the first Settler,
and the existing Builder, replacing only slot three's Monument with a Settler.
Both variants and their controls will remain separately archived. No new fresh
screen or confirmation is authorized by the first prototype's outcome; fresh
seeds will be frozen only if the next consumed replay merits continuing.

The optimized frozen native library is from #3938's runtime source 95b1ee7e4;
its source difference to final d78d97fb4 is only a cfg(test) fixture. The probe's
first compile failed before any play because ActionLog is not indexable; using
its iterator corrected that observation code. The successful compiler command,
source and dependency hashes were frozen before all assigned executions.
All drained planning thought IDs are contiguous, with no reset or turn
truncation. Every assigned map is included. No Firaxis parity is established.

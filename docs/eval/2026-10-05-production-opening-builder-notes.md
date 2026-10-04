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


The second variant also completed every assigned pair with sixteen zero-exit
executions. Scored turn-75 Production fell 14.02% at Emperor and 1.34% at Deity;
turn-75 survival was unchanged, but Deity Science fell more than 10%. End-game
Deity survival was also lower. It is rejected, with no fresh pilot or
confirmation played. Production costs, yields, protected live configuration,
registry and deployment weights remain unchanged.

Each variant's control action logs and final worlds are byte-identical to the
other variant's control on all eight maps. Complete per-turn focal actions,
Builder states and city queues support the timing diagnosis. All raw files,
frozen dependencies, binaries, protocols and compiler hashes are preserved in
`civvis-production-evidence/2026-10-04/opening-order`. The historical probes
compile against the frozen public library and can reproduce both rejected
experiments. The final PR changes only evaluation documentation and probes.

The first expansion analysis was attempted before both Deity processes had
finished and stopped on a missing final-world file. That incomplete invocation
produced no final result file. The complete invocation ran only after all four
batch processes reported exit zero and includes all eight pairs.

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

Both binaries were built with `cargo build --profile ci --locked --bin
civvis_orders`: baseline `152a392a0`, candidate `5f328ae7d`. The replay starts a
fresh controller at the beginning of cont4, retains it across frames, and feeds
all events before each unique `(turn, frame)` decision. Both use the native
Gran Colombia / Domination configuration and the same 19 forced genes.

Both completed all 375 frames: baseline 127.61 seconds, candidate 136.40 seconds.
These concurrent local runs are functional measurements, not a performance
benchmark. After excluding `order_failed`, `order_verified`, and `turn_verified`
telemetry, exactly six frames change exported orders and internal native
research actions. Every other exported order is identical.

| Turn / frame | Baseline research | Candidate research |
| --- | --- | --- |
| 191 / 0 | Sanitation | Printing |
| 197 / 0 | Replaceable Parts | Printing |
| 199 / 0 | Chemistry | Printing |
| 213 / 0 | Steel | Refining |
| 216 / 0 | Advanced Ballistics | Refining |
| 239 / 0 | Plastics | Combined Arms |

Printing repeats because this is a frozen trace: the host observations still
follow the original research history. These are requests at observed research
slots, **not** a claim that the alternative technologies finished on those
turns. Refining and Steel are both needed on the Combined Arms path; the
candidate chooses the cheaper legal prerequisite. The material tradeoff is
earlier supply research at the expense of sanitation, infantry/anti-air
modernization, Chemistry, and Plastics. Existing appointed military packages
remain ahead of this priority.

Eight focused tests pass. Two behavioral regressions failed before the
implementation (Banking instead of legal Combined Arms or its missing
Combustion prerequisite); the in-progress-research control passed. The tests
also cover stock, revelation, ownership, war, victory-target and planning gates,
free upkeep, construction-only resources, queued versus fielded demand,
aggregated combat value, and land/air commitment precedence.

`cargo test --profile ci --locked`: 3,726 library tests plus 205 other tests pass;
49 library tests and four doc tests are ignored. `cargo fmt --all -- --check`
and `git diff --check origin/main...` pass. This changes AI research selection
only, so an engine crash soak is not applicable. The branch was checked against
current `origin/main` (`152a392a0`) before final validation.

Local reproducibility artifacts: `/tmp/civvis-army-fuel-frame-replay.py`,
`/tmp/civvis-army-fuel-replay/{baseline,candidate}-orders.jsonl`, corresponding
binaries and why logs, `/tmp/civvis-3631-comparison.json`, and
`/tmp/civvis-3631-{red,focused,full}.log`.
Alternative requests on frozen observations do not establish host execution,
resource acquisition, restored healing, a city capture, or victory.

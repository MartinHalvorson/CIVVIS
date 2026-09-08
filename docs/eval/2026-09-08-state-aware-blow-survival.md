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
It removes rejected or unsafe strikes and repeats until stable, preserving
independent safe shots. A dropped rescuing strike therefore cannot leave an
unsafe accepted prefix.
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
Expected native damage, finite beam width and host refusal remain limits.
The beam updates killed enemies and our post-strike HP, while surviving
enemies' wounds and friendly support positions retain the frame estimate;
the final verifier replays accepted actions on the actual resulting model
board. Existing per-strike tactical approval remains in force. This does
not guarantee survival under every random roll or partial host execution.

## Validation and comparison

The final implementation is 496e3a33b78964fdfe731a94f4010a2d519ce877, including
main 0f5c7c2ca's movement-risk changes. All 38 battle-planner tests pass.
An earlier full advanced-AI run passed 1,132 tests with 37 ignored; focused
checks were repeated after integration and the verification-pruning fix.
The independent-shot regression fails against the blanket-rejection
prototype and passes with the iterative pruning. Incremental Rust quality
checks pass. Full final-head CI is required before merge.

The `ci`/`developer-tools` binary compares
`advanced+battle-planner-2+strike-reach+doomed-blow-veto-2` against both
`advanced+battle-planner-2+strike-reach+doomed-blow-veto` and
`advanced+battle-planner-2+strike-reach`. Each comparison uses 100 seeds,
both seatings, 24 turns, 28x20 maps, separation 6 and 2 workers. The identical-agent
control uses 12 seeds starting 99130800 and is exactly zero on every seed.
The table reports version two's mean material advantage ± standard error.
The four seed blocks are reused against the two opponents; they are not
800 independent map seeds.

| Army | First seed | Versus version one | Versus no veto |
| --- | ---: | ---: | ---: |
| 2 warriors, spearman, 2 archers, horseman | 99130900 | -4.60 ±26.98 | +49.90 ±37.50 |
| 2 warriors, 4 archers | 99131100 | +21.80 ±12.49 | -0.20 ±33.27 |
| 4 warriors, 2 spearmen | 99131300 | -5.80 ±6.10 | +8.20 ±13.92 |
| 2 musketmen, 2 crossbowmen, bombard, knight | 99131500 | -41.70 ±43.36 | +80.70 ±85.04 |

None reaches p<0.05 even before accounting for multiple comparisons. This
version is not established as an overall improvement over version one or
no veto. Keep it opt-in. It provides an independently screenable hypothesis
about coordinated survival, with targeted behavior verified, rather than a
promotion recommendation. These skirmishes do not establish city protection,
campaign outcomes or performance in the host's full games.

Raw final outputs are `survival2-final-{v1,base}-*-100.txt`, with
`survival2-final-control.txt` and `survival2-final-provenance.json`, under the
verification host's `civvis-tactical-evidence-20260908` directory. The bench
binary SHA-256 is
`8623913ff28c204ec4d93321c18ed9a34867b552fbc766be1cee4b776c505003`;
the replay binary SHA-256 is
`b34bc2208807102320d339dd7610445dbd9c704f0f7383cffbcc4a23bcb8a8f6`.
Earlier files without `final` are preliminary evidence and are not the
numbers reported here.

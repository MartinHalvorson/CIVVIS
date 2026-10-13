# Critically wounded siege recovery investigation

Status: **withheld from deployment** after the four-seed Prince pilot.
The candidate fixes a reproduced vacated-corridor danger defect, but native
survival and playing-strength improvement remain unproven and every pilot
candidate game ended earlier. Keep the source checkpoint for further diagnosis.

Native evidence: `civvis-20260926T211003Z`, pinned `72b0d8bee` (before
the linked-support recovery fix in #3778). Bombard `7208992` and Siege
Tower `3080215` were linked. Coordinates below are native offset coordinates.

| Turn | Start HP | Start | Native result |
| --- | ---: | --- | --- |
| 157 | 100 | (5,25) | Advanced to (6,26); Isin at (7,25) dealt 55 damage. |
| 158 | 45 | (6,26) | Moved to (5,26); Cuirassier `3997696` dealt 44 damage. |
| 159 | 1 | (5,26) | Moved to (5,25). |
| 160 | 1 | (5,25) | Moved to (6,25); Field Cannon `4325386` killed it. |

The recovery owner's frozen baseline and #3778 candidate both reproduce the
turn-160 `MOVE_TO (6,25)` on the same archived frame-0 board. This is a
negative diagnostic for #3778, not evidence that its earlier 31-HP Trebuchet
regression failed. That earlier case and this second case must stay separate.

The archived turn-160 input has SHA-256
`18234122da576c7a954ed0e1165843c11cad42a79835a931076b80672d0de753`.
Local evidence is under
`civvis-war-evidence-20260926/linked-recovery-replay/followup-211003-t160/`.

Investigate the actual danger field, legal retreat tiles, formation eligibility,
and last-shot alternatives before selecting a repair. If every turn-160 option
is lethal, replacing its final move with a hold does not establish that the
unit could have survived. Earlier turns must then be checked separately.

The temporary ignored diagnostic was removed after investigation. Its source
is retained in checkpoint `78c1bb634`; the shipped regression tests are
self-contained. No native victory or capture is attributable to this task.

## Probe results

The turn-160 mirror confirms a valid linked support pair on a nongarrison
tile. The danger reading at its current tile is 147.56 damage; its only two
reachable alternatives each read 152.49. `least_danger_stand` returns `None`
under the existing deliberate lethal-stand rule. This is not an omitted
linked-carrier eligibility check, and a final hold is not a demonstrated save.

At turn 158, however, the retreat to (5,26) reads zero danger and a 20-HP heal
rate. The observed Cuirassier `3997696` starts embarked at (7,26), lands at
(6,26), and actually hits the retreating Bombard for 44. Rebuilding the field
after relocating only the Bombard does not change the zero reading. The linked
peer was not relocated in that first probe. Repeating with both members moved
still reads zero: the modeled Cuirassier has four movement points and reaches
the landing with none remaining. The host exported no rival `max_moves`, even
though it exports that fact for own and barbarian units. A native Great General
is visible at (7,25), but its exact movement contribution is not established.

A deliberately hypothetical six-point allowance makes the vacated landing
reachable and prices the retreat at 50.46 damage. Six is **not** an observed
native allowance. This probe shows why the missing fact matters, not that this
change alone fixes the turn-158 loss. Exporting actual visible-rival movement
allowances is a separate required follow-up; no movement rule or allowance is
invented in this candidate.

A sequential replay of all 462 observed frames through turn 160 keeps AI
history in each arm. Baseline reproduces the native turn-157 advance to (6,26);
the #3778 candidate instead orders (5,26) and Fortify. Both order (5,26) and
Fortify at 158, (4,26) at 159, and (6,25) at 160. The future frames are the
recorded observations, not consequences of the candidate's different orders.
No survival or outcome claim follows from this replay.

## Independently reproduced corridor defect

An isolated three-tile land corridor gives the threat calculation a unit on
the middle tile and an enemy on one end. The unit retreats to the other end.
The old field reads **zero** danger because its precomputed enemy routes still
see the retreating unit blocking the middle tile. A field rebuilt after the
same move reads **14.03** damage. The failing-first regression records exactly
that mismatch; the candidate passes it.

With strike-reach enabled, the field now moves a valid reciprocal formation
on its private probe and tests missing enemy reach on that proposed board.
It retains the old reach as a conservative floor. New reach is cached separately
from damage by unit, destination, and enemy, so different hypothetical HP
values reuse the route calculation without reusing the wrong damage price.
Both formation members and their unit state are restored after each query.
The legacy movement-flood mode is unchanged.

Self-contained tests cover linked and unlinked retreats, query-state
restoration, HP-sensitive damage caching, and an initially blocked landing
whose enemy had no original strike reach. This establishes a planner
correctness defect, not a native domination-strength promotion.

These are historical King observations. The user's subsequent request sets
future games and prospective strength evaluations to Prince, retaining Simón,
four-player Pangaea, and the domination objective.

## Validation and decision cost

Four focused regressions passed. The complete Rust suite passed before
integration (4332 passed, 53 existing ignored), then passed again after
merging `3d8d41e43` (4334 passed, 53 ignored). Changed-Rust quality passed
on both revisions. Two standard-controller Prince simulation smoke games,
seeds **0 and 1**, completed; these were not focal domination comparisons.

Frozen parent `54d5786b5` and candidate `cc0fa26d6` replayed the same archived
King turn-158 and turn-160 frame-zero inputs three times per arm, alternating
execution order. All orders were identical across arms and repetitions. The
Bombard still moves to (5,26) at 158 and (6,25) at 160. Median child CPU seconds
were 0.5621 → 0.5929 at 158 and 0.5607 → 0.6005 at 160 (about +5.5% and +7.1%).
These are small diagnostic samples under concurrent host load, not isolated
throughput benchmarks. CI's paired-cost check also passed.

Raw plan, frozen external harness executables, both complete trial streams,
and replay orders/timing provenance are retained on this host under
`civvis-tactics-results/2026-09-26/vacated-retreat-threats/`.
Candidate CLI SHA-256:
`18eeb42bc5ab6183c707977b26ee58c1c7e6ecaa2e63aba365497fd4b06a9657`.
Parent CLI SHA-256:
`1631dc6c41f55ee04e3e7ccaf5bb6b49e956b5fb6f434357343acf7860bff860`.

## Preregistered Prince pilot: no promotion

Seeds 37840000–37840003 were fixed before either arm ran. Compare the **on**
rows between libraries, not the harness's within-binary off/on totals.
Both arms use the identical frozen Prince-capable harness from
`9dda6319d413cb11bc5fdda55dfa487a6c1f4c11`, the same forced policy bundle,
Gran Colombia focal domination target, four majors, 60×38 Pangaea, six city
states, Online speed, natural 250-turn cap, and all victories enabled.
Both player and barbarian difficulty are Prince. Opponents are adaptive
CIVVIS controllers, not Firaxis; the library correction can affect their
strike-reach calculations too. This is not a focal-only treatment.

| Seed | Parent outcome | Candidate outcome | Major cities observed held, parent → candidate | Foreign capitals held at end |
| --- | --- | --- | ---: | ---: |
| 37840000 | Score win, t250 | Religious loss, t227 | 3 → 0 | 0 → 0 |
| 37840001 | Science loss, t249 | Science loss, t235 | 0 → 1 | 0 → 0 |
| 37840002 | Science loss, t235 | Religious loss, t207 | 0 → 0 | 0 → 0 |
| 37840003 | Science loss, t244 | Culture loss, t196 | 4 → 9 | 1 → 1 |

Domination wins are 0/4 in both arms. The extra cities on the last seed do
not cancel out four earlier endings or establish a stronger domination AI.
Four seeds cannot prove a general regression, but these results, additional
decision cost, and unchanged archived orders do not justify live promotion.
No seeds were removed, replaced, or selected after seeing their result.

Full baseline JSONL SHA-256:
`89aaeeafcb6463de4d3450c5bd3094bfae9d4af80f5bba0a128f14cb90a99099`.
Full candidate JSONL SHA-256:
`d8db4767351b2bfc776d98e8577c1750924fbd4d1bfcf6365e2c08235008f289`.

Next useful evidence is actual rival maximum movement from the independent
export correction (#3788), followed by a grounded native retreat replay.
Do not ship this candidate merely because its synthetic regression passes.

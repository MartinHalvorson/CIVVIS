# Bomber generation continuity: native defect and King regression screen

Status: **PROMOTE AS A CORRECTNESS FIX**. The registered functional and regression
checks pass. This does not establish a strength gain or a native Domination win.

## Native trigger

Native `civvis-20260927T110717Z` lost to Science on turn 239. Gran Colombia kept
its own original capital, held one foreign major city from turn 231, and held no
foreign original capitals. Radio and Aluminum income appeared on turn 172,
Advanced Flight on 181, two usable airfields on 191, the first Bomber on 203,
and two aircraft on 228.

The first aircraft upgraded to a Jet Bomber by turn 214. On turns 224–226 the
active surge nevertheless reported `0/4 bombers`, despite observing that Jet.
It compared aircraft and queues against the original Advanced Flight unit name.
The omission affects package counts, launch estimates, quota accounting,
production priority and ordering. After Stealth Technology, the old Bomber is
obsolete and the producer cannot request the legal Jet successor.

The 25-turn gap between the first and second aircraft has additional causes.
Maracaibo's Bomber queue automatically became the costlier Jet Bomber on turn
204, preserving progress. It reached 337/350 production on 217, but its airfield
was pillaged and the queue disappeared on 218. Popayán then trained the second
Jet through 227. Correcting package bookkeeping does not establish that those
native delays disappear or that the game would be won.

## Candidate

Count every air-domain `air_bomber` in the standing and queued package, excluding
fighters. Preserve an existing successor queue's priority by subtracting its own
commitment from the candidate quota. Choose the strongest unlocked, nonobsolete
Bomber from the civilization's actual catalog; retain the Advanced Flight unit
as the planning anchor before it unlocks. The research goal remains Advanced
Flight. Cost, range and upkeep planning use the selected generation.

Before two aircraft stand, range planning uses the trainable generation. Once a
launch wing stands, at least two aircraft must individually have enough range
from an owned city base to the objective. A single Jet cannot lend its longer
range to an older partner. Legal production menus and the existing resource,
treasury, threat and quota guards still apply. This does not change the early
alternative-airfield prototype withheld in PR #3825.

## Registration

Registered at `2026-09-27T12:29:44.673357+00:00`, before candidate native replay or
fresh games. Evidence root:
`~/civvis-war-evidence-20260927/bomber-generation-continuity/`.

Both frozen sources replay all 700 observed native frames, including same-turn
replans, with Domination, deployed air-surge-2 ON and all 21 forced genes.
Both must finish every frame with identical treatments. Baseline must reproduce
at least one undercount of observed Jet Bombers. Candidate must produce an
active package report with upgraded aircraft and no count mismatch against its
corresponding observation. Fixed future boards establish bookkeeping and
proposals, not counterfactual conquest or victory.

Eight fresh source pairs use seeds `38260000`–`38260007`. The profile is four
players, focal Gran Colombia with Domination target, Pangaea 60×38, six city
states, Online, 250 turns, King players and barbarians, focal human handicap
exemption and all victory conditions enabled. Keep the 21 deployed forced
genes, SHA-256
`439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`.

Four independent two-seed workers alternate source order by seed. The existing
single-seed harness runs focal OFF then ON for each source; ON across sources is
primary and OFF is secondary. Freeze binaries before launch, inspect outcomes
only when every worker is terminal, and do not tune or replace seeds. Source
changes also affect rival CIVVIS controllers. The simulator is not native
Firaxis difficulty equivalence or an actor-isolated causal comparison.

Promote as a correctness fix only if generation, queue, range and quota tests,
full required checks and native replay pass, and the eight primary source pairs
show no decrease in aggregate Domination wins or final foreign original capitals
and no increase in own original capital losses. Invalid or incomplete profile
or provenance withholds promotion. This is a regression screen; any strength
claim requires a separately registered larger replication.

## Validation and results

Baseline focused tests reproduce four assertion failures: upgraded package
count, legal successor production, two-Jet range, and mixed wing count. The
original four production-queue regressions pass on baseline. The candidate's
nine focused tests pass, including successor queue preservation through the
production governor and no third aircraft when two Jets fill the affordable
launch quota. Intermediate compilation and fixture errors were corrected
before binary freeze; no policy tuning occurred after fresh launch.

`cargo test --profile ci --locked` passed 4,423 tests, zero failures and 53
ignored. `python3 tools/rust_quality.py --base 9d430ac585b91744cb5f959e400b1599bbb285eb
--head 01194a859` reports the changed lines formatted and warning-free.
`python3 -m unittest discover -s tools -p 'test_docs*.py'` passed 12 tests.

The baseline binaries were compiled from
`0583bcbb4544ca43608ef4f46a8982693f38dea9`. All thirteen changed files through
baseline `9d430ac58` are docs or Python tools; Rust, compiled data, Cargo inputs,
gene ledger and the forced bundle are identical. The equivalence patch and its
hash are retained. Candidate binaries were compiled in this task worktree's own
target directory from `01194a85963b8301b4b90c93f23b7f8e8fed5906` and frozen before
replay and fresh launch.

| Binary | Baseline SHA-256 | Candidate SHA-256 |
| --- | --- | --- |
| `civvis_orders` | `23e797618293c0e41b215517db8a77585042f0990c42308a619a70b856b19a95` | `a9d4a76cd57b3e6369302356faa2919c10b708c5868c2924ac75302751b8f79a` |
| `victory_eval` | `afade64f074882b49a46ee5add09898ce7b74eb70573ecf8d9dbdd7768ed7264` | `aa03f1994f8715b8a69a8eeb8d5ecefd35457a46f9f0d988a1c2c03b133dfead` |

Both persistent native replays completed all 700 frames. Their actual genomes
have Domination, air-surge-2 ON, v1 OFF, identical treatment sets and all 21
requested deployed policies active. Frame-indexed reasoning offsets align each
package report with the corresponding observed state. Baseline has twelve
reports, including four mismatches: zero instead of one Jet on turns 224–226
and zero instead of two Jets on 233. Candidate has 68 reports, 57 with observed
Jets, and zero mismatches. It also proposes legal Jet Bomber reinforcement
queues. Different report counts reflect changed internal appointments and
proposals, not additional native matches. This passes the registered native
preflight; it does not show a counterfactual capture or victory.

All four fresh workers finished at `2026-09-27T12:51:21.584220+00:00`; outcome
inspection began at `2026-09-27T12:51:55.482846+00:00`. All sixteen harness
invocations returned zero. All 32 games, eight fixed source pairs, profiles,
forced-policy lists, focal civilization/target, paired civilization rosters and
frozen binary hashes were validated. No seed replacement or policy tuning
occurred after launch.

| Metric, eight primary ON games per source | Baseline | Candidate |
| --- | ---: | ---: |
| Domination wins | 0 | 0 |
| Final foreign original capitals | 1 | 1 |
| Foreign original capitals ever held | 1 | 1 |
| Own original capital losses | 3 | 3 |
| Foreign major cities ever held | 8 | 8 |
| Major declarations | 4 | 4 |

| Seed | Baseline finish | Candidate finish | Major cities ever, baseline/candidate | Final foreign capitals, baseline/candidate | Own capital held, both sources |
| --- | --- | --- | ---: | ---: | --- |
| 38260000 | science 195 | science 195 | 7/7 | 1/1 | yes |
| 38260001 | science 211 | science 211 | 0/0 | 0/0 | yes |
| 38260002 | science 195 | science 196 | 0/0 | 0/0 | yes |
| 38260003 | science 196 | science 196 | 0/0 | 0/0 | no |
| 38260004 | science 191 | science 191 | 1/1 | 0/0 | yes |
| 38260005 | science 213 | science 213 | 0/0 | 0/0 | no |
| 38260006 | science 193 | science 193 | 0/0 | 0/0 | no |
| 38260007 | science 202 | science 202 | 0/0 | 0/0 | yes |

The secondary OFF arm has zero Domination wins, zero final foreign capitals and
three home-capital losses in both sources. Equal aggregate counts do not imply
identical play: some leg summaries and action counts differ, and one primary
game finishes a turn later. The harness does not record Bomber-upgrade exposure.
This small screen cannot exclude a regression or establish a strength gain.
Source changes also affect rivals. The registered decision is to promote the
reproduced correctness fix because its functional checks pass and none of the
three pilot guards worsens. No strength replication or ledger change is claimed.

## Automatic native observation readback

While this task ran, the existing recovery supervisor completed the separate
native game `civvis-20260927T115203Z` through `-cont2`. The automatic terminal
ladder row from observer source `307fdf2` includes air supply and an explicitly
validated recovered game path. It uses root turns 1–83 and continuation turns
84–181, discarding the superseded first continuation. These are one game.

The rival won Culture on turn 182. Our original capital remained held, no
foreign major cities or original capitals were held, and the two Bombers were
first observed on 176 and 180. The terminal observation has Aluminum income
five, stock 25 and two usable airfields. This independently verifies automatic
air-supply collection on a newly completed native game; it is not a candidate
result or evidence of stronger play. The next native batch remains on the
deployed source and the requested four-player King Gran Colombia profile.

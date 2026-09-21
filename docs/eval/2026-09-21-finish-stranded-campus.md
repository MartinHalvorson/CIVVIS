# Finishing a stranded Campus: investigation

Native run `civvis-20260921T214546Z`, pinned to `82741ed94`, placed Popayán's
Campus around turn 62 and still had it unfinished at turn 172. Native offers
continued to allow completion: 12 turns at turn90, six at120, and two at150
and172, at a locked 70 production cost. The city produced 4.5 science while
that Campus remained unfinished. At162 the nine-city empire produced58.91
science against rivals at108.91–144.70.

The research catch-up planner previously admitted only Campus buildings,
excluding the foundation that makes those buildings possible. The experiment
admits an already placed Campus foundation for Domination under the existing
public-science shortfall gate. It retains idle-queue, home-emergency, Recovery,
war-plan, and affordability checks. The existing production commitment also
preserves that new reservation before its first hammer. New sites and other
district families do not enter this path. Yield accounting credits the
Campus's own yield, not a hypothetical future Library.

A queued foundation does not satisfy the legacy global research-building
reservation: a completed Campus elsewhere can still reserve its Library.
A first replay caught that regression in the prototype; a failing Library
regression test and the corrected replay verify the fix.

## Validation

- Final source: `6ee81ea01cbbd32eb20ebf62e2cddf1640d76b26`.
- Parent: `4fafd3e059b88a4be39c0810ad103e57e9a522cf`.
- Full Rust suite: 4,134 passed, zero failed, 53 ignored across six suites.
- Thirteen research commitment tests include four new tests: foundation
  reservation and production-review survival; new-site/other-lane/caught-up
  exclusions; threat/Recovery/busy-queue safeguards; queued-foundation versus
  Library regression. Both causal regressions failed before their fixes.
- Fourteen append-integrity tests passed; changed-line Rust quality passed.
- Eight four-player, 180-turn smoke games completed (seeds369800–369807).
  These are stability checks, not evidence of native Domination wins.

## Paired recorded-game replay

Both final binaries processed all493 frames through turn172. Input SHA256:
`778b5f296b6ec9cc441bbc87a0f391267efdfe2d6dbf12b827b101df0f2e97d7`.
Baseline binary SHA256:
`2f697aceaa40f69d8cb6f534f5810b437ee032817e29d5a1c84d06158cd0eb2f`.
Candidate binary SHA256:
`8e5a5573981ffad1d5859adedb9cbbf12e24551c51ef77923b0f4a43c6d5c467`.

Twenty-five exported frames and45 internal-action frames differ. All exports
before138 are identical, including Cartagena's turn60 Library. At138 Popayán
resumes its Campus instead of a Workshop. It also requests Campus completion
at164–167 and170–172 instead of Entertainment/Builder or subsequent military
production. All changed production commands belong to Popayán; later
Natural Philosophy policy choices also reflect the changed plan.

The recorded future still contains the original Workshop and unfinished
Campus. Consequently the replay includes later Walls/Knight substitutions,
removed military next-production requests, and four added failure receipts
against the original Workshop. These are counterfactual mismatches, not
observed native execution failures. Likewise its seven Campus verification
receipts do not establish seven actual completions. The replay proves the
new completion requests and preservation of the early Library, not faster
research, a completed Campus, captured capitals, or victory. It trades some
local production/military/amenity work for science under existing safeguards.

Artifacts remain outside the worktree in `/tmp/civvis-campus-finish-replay/`
(binaries, provenance, input, replies and why logs), with comparison at
`/tmp/civvis-3698-final-comparison.json`. Replay durations91.94/91.35seconds
are not controlled performance measurements; CI provides the speed gate.

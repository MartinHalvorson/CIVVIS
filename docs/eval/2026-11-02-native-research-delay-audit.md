# Native research delay audit, 2026-11-02

The next research experiment should protect the first Science launch's
technology chain through a temporary Recovery posture and optional army
modernization. Earlier science infrastructure is a separate experiment.

## Evidence and coverage

The accompanying JSON was produced from the live ladder and retained native
logs, starting at `2026-11-01`. Of 72 configured attempts in that snapshot,
33 have retained research logs and 39 are explicitly excluded for missing
`why.log`. Every included run also has a hashed native `events.jsonl` witness.
This is a retained-log sample, not an unbiased estimate of fleet performance.
The active run is absent until it appears in the ladder's attempts.

Thirty included attempts selected Rocketry. Thirteen recorded at least one
military-attributed selection between Rocketry and Satellites that is outside
the **model** Satellites prerequisite closure. Repeated same-turn frame
decisions count once. An unobserved Satellites selection is censored, never
treated as a zero interval. Abandoned attempts have separate summary groups.
Selection logs record proposed decisions; the separate native witness records
first observed ownership of each milestone.

Across the retained runs, native ownership medians were Writing t33 (33 runs),
Education t92 (31), and Rocketry t185 (29). These are different observed
cohorts; their differences are not paired estimates of research duration.

## Emperor Science case

`civvis-20261102T161612Z`, controller revision
`6cb7bed2d6f95ff074e183a17c3372d9144c50ab`, lost to a Technology victory at
t208. It first owned Rocketry at t171, established its Spaceport at t195
(ladder field), and first owned Satellites at t196.

| Turn selected | Technology | Logged reason |
| --- | --- | --- |
| 171 | Ballistics | Nearby barbarian army |
| 173 | Siege Tactics | Nearby barbarian army |
| 175 | Electricity | Ordinary Recovery argmax |
| 178 | Advanced Flight | Decisive military window |
| 182 | Printing | Standing army modernization |
| 183 | Military Science | Standing army modernization |
| 185 | Rifling | Standing army modernization |
| 186 | Steel | Standing army modernization |
| 188 | Advanced Ballistics | Standing army modernization |
| 192 | Satellites | Science lane |

Advanced Flight is required for Satellites in `data/techs.json`, so its
military label does **not** make it an unrelated detour. The other seven
military-labelled selections are outside that model prerequisite closure.
The 29 turns between selecting Rocketry and Satellites include researching
Rocketry itself, prerequisites, and other decisions. They are not 29 turns
that a new treatment can claim to save. The native ownership interval is 25
turns and also includes necessary research.

## Mechanism in current source

At audit base `27927468d`, `advanced_research` places wartime modernization
ahead of the ordinary Science goal. The stronger
`science_endgame_research_preempts_wartime` protects Nanotechnology, Smart
Materials and Offworld Mission only after a Moon Landing is queued or owned.
It does not cover the first Satellite's prerequisites. The log's Recovery
argmax also shows why changing only wartime priority would leave another hole.
The recent `satellite-holds-the-pad` gene protects production at the pad;
this report investigates the earlier technology path.

## Next treatment and acceptance evidence

Implement an opt-in treatment for a seat committed to Science that already
owns Rocketry, keeping the next unowned first-launch technology goal through
temporary posture changes. Place it ahead of optional wartime modernization
and ordinary Recovery scoring, while preserving immediate city defence and
genuine war-breakthrough decisions. Test the Recovery and wartime cases
separately, plus a non-Science control, completed-Satellites control, and the
Advanced Flight prerequisite case.

Use the documented six-player Emperor screen and disjoint confirmation seeds
before a default promotion. Native follow-up should measure first owned
Satellites, first Spaceport, first launch, survival/cities lost, and actual
wins. Faster research alone is insufficient: a pad still under construction
may hide the benefit, and foregoing an upgrade could lose a city. An earlier
Spaceport and a protected research chain may interact; retain each treatment's
identity rather than crediting the combined result to one change.

The early compounding question remains open: Education at a retained median
t92 is late enough to warrant a separate look at research ordering and actual
Library availability. Do not bundle that intervention into this launch-chain
experiment.

## Reproduce

```sh
python3 tools/civ6_research_delay_audit.py \
  /Users/martbot-mbp-m5-max-128/civvis-civ6-runs/control \
  --since 2026-11-01 --out /tmp/native-research-delay-audit.json
python3 -m unittest discover -s tools -p test_civ6_research_delay_audit.py
```

The ladder continues changing, so a fresh run can include additional attempts.
The committed JSON pins the source hashes and revision metadata for this audit.
The prerequisite annotation is explicitly from the hashed model tech graph;
it is not an independent validation of the installed native ruleset.

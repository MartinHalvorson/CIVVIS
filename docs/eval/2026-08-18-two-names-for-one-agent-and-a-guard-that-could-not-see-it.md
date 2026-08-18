# Two names for one agent, and a guard that could not see it

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

#2002 shipped `every_distinct_same_family_arm_declares_a_semantic_axis` under
the title *"No evaluator arm may quietly be another arm"*. It rejects an
`"implementation"` axis, so an arm that differs in a way nobody named fails.

It says nothing about the opposite. An arm that names **no** axis at all passes
it. So: **do any arms declare nothing, and is that deliberate?**

## How it was measured

A census over all 226 registered names — `BUILTIN_AIS` chained with
`EVAL_ONLY_AIS` — grouping by `AgentSpec::differing_axes` being empty, then
reading each group's constructor and its entry in the artifact effective-alias
table.

A second, behavioural census was run first and **discarded as too weak to
conclude from**: 226 arms on a 24-turn 2-player 16x12 probe collapse to 36
distinct action logs, which mostly says that congress, war and governors do not
happen by turn 24. Reading that as "these arms are identical" would have been
the same error as #2049.

## What it measured

**212 distinct specs among 226 names, in four collapsed groups.**

| group | names | why |
|---|---:|---|
| `advanced` and nine others | 10 | the alias table declares each `ArmKind::Advanced` — treatments retired from production, names kept so a recorded result keeps its identity |
| `advanced_evolved`, `evolved`, `advanced_banking_dedication` | 3 | all resolve to `AdvancedEvolved` once the champion loads; the third is declared `advanced_fallback` in the same table |
| `strategic`, `strategic_score`, `strategic_warm` | 3 | all resolve to `strategic_score` with no value net present — the documented degraded fallback |
| `strategic_deep` with itself | 2 | **the name is registered twice** |

**Three of the four are deliberate and already declared.** `artifact_effective_alias_from`
names them explicitly, with the comment *"these three withhold arms now build
the production controller… declared effectively `advanced` so the pairs fail
closed as self-play"*. The registry is coherent.

**The fourth is a real defect.** `strategic_deep` is in `BUILTIN_AIS` *and*
`EVAL_ONLY_AIS`. Those lists are not interchangeable: `EVAL_ONLY_AIS`'s own doc
says keeping a control out of `BUILTIN_AIS` *"prevents a control factory from
being pooled into the same player/leader rating key as its treatment"*, and
`BUILTIN_AIS` is what gates a tournament entrant. A name in both is
evaluator-only and rated at once.

Git says exactly how it happened. `3c543665` — **"Add strategic_deep, and
decline to promote it"** — put it in `EVAL_ONLY_AIS` at 07:16. `dc6f661e` —
**"Promote strategic_deep: pre-registered PASS at 300 maps"** — added it to
`BUILTIN_AIS` at 08:29 and did not remove the first entry. It has been in both
for three weeks, and `docs/EVAL_STATUS.md` has been counting it twice.

## What was decided

**The `EVAL_ONLY_AIS` entry is removed**, because the promotion commit is the
later statement of intent and the one that made it a tournament entrant.
`no_agent_name_is_registered_twice` now fails on any name in both lists or
repeated within one, and was confirmed against the restored duplicate:

```
a name is registered more than once; a builtin is rated and an evaluator-only
control is deliberately not, so it cannot be both:
["strategic_deep in [\"BUILTIN_AIS\", \"EVAL_ONLY_AIS\"]"]
```

**A second guard was written and not shipped.** It would have allowed an empty
axis list exactly when the two names resolve to the same effective agent, and
failed otherwise — the natural mirror of #2002. It passes today, and it also
passes when the collapse is *planted*: removing an alias declaration, and
blanking a real arm's axis list, both leave it green. `builtin_provenance`'s
effective name does not separate those cases, so the guard was checking less
than its assertion claims.

⚠ **A guard nobody can show biting is the thing this repository keeps paying
for**, so it is recorded here instead of shipped. The census above is the
finding; the check that would keep it true needs an equivalence the registry
does not currently expose, and inventing one inside a cleanup would be the
wrong place to do it.

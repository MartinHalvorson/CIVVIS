# A failed capture must stay stood down during its cooldown

`capture-go-or-stand-down` can mark a stalled siege objective unavailable and
make its plan stale immediately. The ranking honors that cooldown. With
`siege-commitment` also enabled, however, the cached target was selected ahead
of the filtered ranking, putting the same stood-down city straight back into
the plan. Reassessment could therefore keep restoring the rejected objective.

The cached commitment now consults the same `capture_stood_down_holds`
predicate as the ordinary ranking. Both stand-down versions apply; an expired
cooldown stops excluding the objective. An active commitment with no stand-down
still holds. The existing home-emergency priority remains in place.

This is an interaction correction, not a new gene or a change to deployment
selection. A separate eight-seed exploration and forty-seed holdout found that
simply enabling siege commitment did not reliably improve Domination completion;
those observations motivated inspection but do not prove this interaction caused
the outcome difference. The regression directly exercises the conflicting plan
selection paths. The full suite passed 3,590 tests (52 ignored), including
both stand-down versions and cooldown expiry in the integration regression.
Clippy produced no diagnostics; formatting and diff checks passed.

## Matched native replay

A predeclared 16-seed Prince replay (109109000–109109015; three players,
36×22, Online 250, no barbarians or city-states, all majors targeting
Domination) compared base `7fdb08a18262` with patched `297eeaa15`.
Both arms enabled `domination-lane-hands-over`, `siege-commitment` and
`capture-go-or-stand-down-2`, and disabled the first stand-down version.
Only the cached-target filter changed between the two source trees.

The unpatched arm completed Domination in **4/16** worlds; the patched arm
completed **2/16**. One paired seed gained a completion and three lost one;
ten terminal records were identical. This small sample provides no evidence
of improved completion rate. The repair enforces the enabled policy's explicit
cooldown; it does not justify enabling the combined policy or claim that the
current stand-down thresholds are optimal. No deployment selection changed.

The existing evaluator can reproduce terminal outcomes at each revision:

```sh
cargo run --profile ci --locked --features developer-tools --bin victory_eval -- \
  --target domination --games 16 --players 3 --width 36 --height 22 \
  --turns 250 --speed online --start-seed 109109000 \
  --with domination-lane-hands-over --with siege-commitment \
  --with capture-go-or-stand-down-2 --without capture-go-or-stand-down
```

The observer replay's paired terminal records below retain raw engine turns
(the evaluator reports the score boundary as turn 250 instead of raw 251).

| Seed | Unpatched: victory / winner / raw turn | Patched: victory / winner / raw turn |
|---|---|---|
| 109109000 | domination / 1 / 224 | score / 1 / 251 |
| 109109001 | score / 2 / 251 | score / 2 / 251 |
| 109109002 | domination / 2 / 128 | domination / 2 / 241 |
| 109109003 | score / 0 / 251 | score / 0 / 251 |
| 109109004 | domination / 0 / 169 | score / 0 / 251 |
| 109109005 | score / 0 / 251 | score / 0 / 251 |
| 109109006 | score / 0 / 251 | score / 0 / 251 |
| 109109007 | domination / 0 / 205 | score / 0 / 251 |
| 109109008 | score / 2 / 251 | score / 2 / 251 |
| 109109009 | score / 0 / 251 | score / 0 / 251 |
| 109109010 | score / 0 / 251 | score / 0 / 251 |
| 109109011 | score / 0 / 251 | score / 0 / 251 |
| 109109012 | culture / 0 / 194 | domination / 0 / 184 |
| 109109013 | score / 0 / 251 | score / 0 / 251 |
| 109109014 | score / 0 / 251 | score / 2 / 251 |
| 109109015 | score / 0 / 251 | score / 0 / 251 |

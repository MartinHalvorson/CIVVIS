# Pre-registration — `max(city_target gene, plan.desired_cities)` for the base settler gate

Written 2026-08-03 **before the treatment exists in code and before any map has
been run on it.** Agent `opus5-loop`.

## Why this specific knob, and why it is not a second bite at #684

`AdvancedAi::plan_city_target` (arm `advanced_plan_city_target`, merged as a
recorded null) substitutes `plan.desired_cities` for the `city_target` gene in
`BasicAi::cities`'s settler gate. It was measured and it made the empire
**smaller** in both profiles:

```
[eval 4p 24x16]       off  cities 2.17 / plan target 3.83   score 147
[eval 4p 24x16]       on   cities 1.50 / plan target 3.67   score 122
[deployment 6p 74x46] off  cities 4.83 / plan target 5.00   score 373
[deployment 6p 74x46] on   cities 4.33 / plan target 5.00   score 329
```

The recorded cause is mechanical: the ramp is
`desired_cities = (city_target_floor + turn/cadence).min(map_capacity).min(6)`
and it **opens at 3 while stock's gene is 4.0**, so through the whole compounding
early game the plan is the *more restrictive* number.

`civvis-city-target-is-live-on-the-adaptive-path` names the variant that does not
have this failure mode — `max(gene, plan.desired_cities)` — and explicitly says
it **needs its own pre-registration** because it became visible only after that
census. This is that pre-registration. The knob was chosen from the recorded
cause, not from a favourable read: I have looked at no outcome on this variant.

## The mechanical claim being tested

`max` can only ever raise the settler ceiling, never lower it:

| turn | gene | plan ramp | stock gate | treatment gate |
|---|---|---|---|---|
| opening | 4.0 | 3 | 4.0 | **4.0** (unchanged) |
| mid | 4.0 | 5 | 4.0 | **5** |
| late | 4.0 | 6 | 4.0 | **6** |

So the treatment is a strict superset of stock's expansion appetite and **cannot
reproduce #684's shrinkage**. If city count does not move at all, the knob is
inert and that is a null, not a win.

## Treatment

New flag `AdvancedAi::plan_city_target_max`, default off, reachable as
`advanced_plan_city_target_max`, paired against `advanced`. It substitutes
`gene.max(plan.desired_cities)` for the duration of the one delegated
`BasicAi::cities` call and restores the gene afterwards — the same seam
`plan_city_target` already uses, so nothing else changes. The existing
`plan_city_target` arm is left exactly as it is; its recorded null stays valid.

## The runs, fixed now

⚠ Both profiles are required. `civvis-city-target-is-live-on-the-adaptive-path`
established that the expansion axis has two regimes and **one profile cannot
judge it**: deployment 6p 74×46 is TARGET-limited (4.83 cities against its own
5.00), compact 4p 24×16 is EXECUTION-limited (2.17 against 3.83). A result read
only at the compact profile would be measuring the wrong constraint.

```sh
# A — deployment regime (PRIMARY)
ai_eval advanced_plan_city_target_max advanced --pairs 120 --jobs 4 \
        --seed 91000000 --players 6 --width 74 --height 46 --turns 250

# B — compact regime (GUARD)
ai_eval advanced_plan_city_target_max advanced --pairs 300 --jobs 4 \
        --seed 92000000 --players 4 --width 24 --height 16 --turns 140
```

Seeds are declared here and will not be changed. Both exceed `ai_eval`'s own
promotion gate of 20 independent maps.

## Decision rule, declared before the first map

1. **Fires-check, run first.** The `cities` column must differ between arms on
   profile A. If it does not, the knob is inert: record a null and stop. Do not
   read any outcome number before this check — that is what killed #684's
   treatment and it is what fires-checks are for.
2. **SHIP** only if, on profile A: paired-map score favours the treatment, and
   `cities` is not lower, and profile B shows no significant score regression
   (treatment-favoured or inconclusive direction, i.e. not an `advanced`-favoured
   sign test at p<0.05).
3. **RECORD A NULL and do not ship** if profile A is inconclusive or negative, or
   if profile B regresses significantly.
4. The `ai_eval` promotion gate's own verdict is reported verbatim either way. I
   will not substitute a different statistic after seeing the output, and I will
   not re-run either profile on a fresh seed to obtain a better read.

## What this cannot show

Both profiles are the headless engine. `civvis-headless-and-live-are-different-regimes`
applies: the live Civ 6 ladder reaches 4–5 cities and an army of one, and neither
profile reproduces that. A pass here is a reason to enable the flag on the live
bridge and then read `code_rev` rows in `~/civvis-civ6-runs/civvis_ladder.jsonl`;
it is not evidence the ladder improved.

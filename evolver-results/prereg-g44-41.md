# Pre-registration — the one genome with a well-powered positive deployment reading

Written 2026-07-31 **before this run existed**, and before `r2`/`r3`/`r1` of
the genome-halves screen returned anything. Agent `claude-evolver`.

## Why this candidate, and why it is not a fishing expedition

Every genome measured today at the matrix's deployment profile sits at or below
its deck-matched baseline: the champion 42.5%, `g28-28` 40.0% (10 maps), the
deck-only rung `r0` 53.1%. The repo's own record contains exactly one genome
with a **well-powered positive** deployment reading:
`docs/LIVE_GENOME_TRANSFER.md` measured **`g44-41` at 51.9% (+13)** over 40
pairs on an 8p 84×54 / 12-city-state / flat Online profile — `INCONCLUSIVE`,
11 directions for and 8 against, failing only its own pre-declared 52.5% screen
term. It was never measured on the matrix's own 6p 74×46 deployment profile.

It is also the genome my mechanism predicts should carry. The deficit measured
today is attributed to the champion's **yield** genes, and `g44-41` sits close
to stock on exactly those:

| gene | `g44-41` | champion | stock |
|---|---:|---:|---:|
| `city_target` | 4.000 | 2.408 | 4.000 |
| `settler_min_pop` | 2.365 | 4.457 | 2.000 |
| `builder_per_city` | 0.414 | 0.200 | 0.500 |
| `wonder_min_bld` | 2.664 | 1.164 | 3.000 |
| `faith_builder` | 150.5 | 350.4 | 120.0 |

So this is a **prediction**, not a search: the genome whose yield block is near
stock should not carry the champion's deployment deficit, and its non-yield
genes have 1,799 league games behind them.

## The run, fixed now

```sh
ai_eval advanced_evolved advanced --pairs 40 --jobs 4 --seed 68000000 \
  --players 6 --width 74 --height 46 --city-states 9 --turns 250 \
  --speed online --map continents --shape planet --poles poles \
  --randomize-civs --victories science,culture,domination
```

from a working directory staging `g44-41` at `evolved/best.json`. Seed
68,000,000 is disjoint from 61/62/63/66,000,000 and from
`docs/LIVE_GENOME_TRANSFER.md`'s 9,958,000.

## The rule, fixed now

`g44-41` is nominated for a full `ai_eval --matrix --pairs 120` gate at seed
69,000,000 **only if its paired-map score exceeds the deck-matched baseline
`r0 = 53.1%`** on this run. Any other outcome — including a score above 50% but
at or below 53.1% — is recorded as "does not beat a stock genome carrying the
same policy deck" and nothing is gated. No gene, seed, sample size or profile
flag will be chosen after seeing this result, and a failure will not be pooled
with the `9958000` maps or retried on another seed.

⚠ If the `r0` re-run on the final binary does not reproduce 53.1%, this rule's
threshold moves to that re-run's value, which is fixed before this run starts.

✅ **RESOLVED: the threshold stays 53.1%.** The `r0` re-run on the final binary
returned output **byte-identical** to the original — same paired score, same
Wilson interval, same every diagnostic column — so the contingency above does
not fire.

⚠ **MAP OVERLAP NOTED before the run, not after.** `MATRIX_PROFILE_SEED_STRIDE`
is 1,000,000, so the `r3` gate launched at seed 67,000,000 puts its
**deployment** child on seed **68,000,000** — the same prefix this screen uses.
This screen's first forty maps are therefore the `r3` gate's first forty
deployment maps.

That is not a confound for *this* decision, whose rule compares `g44-41`
against `r0` at seed 66,000,000, a disjoint set. It does mean `g44-41` and the
`r3` gate's deployment arm are **paired on shared terrain** rather than
independent, so any later comparison *between those two* must be read as paired
and neither may be treated as an independent replication of the other. The seed
is not being changed after the fact; it is being labelled.

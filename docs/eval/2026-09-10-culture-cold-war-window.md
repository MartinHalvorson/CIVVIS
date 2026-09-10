# Test an earlier Rock Band window for Culture

`culture-cold-war-window` is an opt-in strategy experiment. It keeps Humanism
and Conservation first, then asks for Cold War before Professional Sports and
Cultural Heritage. The existing path reaches Cold War later as a prerequisite
of Space Race. Earlier government and Great Person goals keep their priority.
The band purchase pass, its Faith prices, and its active-band cap are unchanged.

The hypothesis is that opening a spendable Faith-to-tourism path earlier can
finish a Culture race sooner than the stadium and museum-tourism detours. It
can also lose: earlier Cold War consumes culture that could improve persistent
tourism, and a seat with little Faith or poor venues may get no useful concerts.
This is a choice to measure, not a correction that should silently become the
default.

The host unlock is grounded in
`DLC/Expansion2/Data/Expansion2_Units.xml:42`, whose Rock Band row names
`PurchaseYield="YIELD_FAITH"` and `PrereqCivic="CIVIC_COLD_WAR"`. The engine's
`data/civics.json` makes Cold War and Professional Sports siblings under
Ideology. No rules, purchase costs, or prerequisites are altered here.

Regression coverage checks the legal civic order through `advanced_research`,
including an explicit Culture target while the temporary plan is Expansion.
The control selects Professional Sports; the treatment selects Cold War.
The Science target is unchanged, the early Humanism/Conservation sequence
remains intact, and disabled Culture victory falls back to the old order.

## Evaluation protocol

Use one recorded binary and identical seeds for an all-Culture reachability
comparison with `victory_eval --target culture --deployment`, enabling only
this gene in the treatment. This evaluator targets every major at Culture,
uses no barbarians or city-states for that target, and does not accept a
difficulty flag. Its result is a simulator reachability comparison, not a
Pericles-versus-Emperor win-rate estimate.

Also run a standard-shape `gene_screen` batch at Emperor, varying this gene
among the deployed background's adaptive seats. Record the binary hash,
seeds, shape, exposure counts, complete outcomes and uncertainty. A small
screen is diagnostic and does not justify promotion. Do not pool it with
live Pericles outcomes or the all-Culture reachability comparison.

## All-Culture comparison

Both arms used clean source revision `bec42aae732a296acb2c2077a406317972768168`,
the same `ci`-profile binary, six players, 74×46, Online speed, a 250-turn cap,
and the deployment profile. Only the experimental gene differed. Each arm
completed all four games, and every game ended in a Culture victory.

| Seed | Control finish | Earlier Cold War finish | Treatment minus control |
| --- | ---: | ---: | ---: |
| 910131000 | 175 | 183 | +8 |
| 910131001 | 176 | 180 | +4 |
| 910131002 | 200 | 211 | +11 |
| 910131003 | 202 | 197 | −5 |

The experimental order finished later on three of four maps and averaged
4.5 turns slower. This small all-Culture comparison provides no case for
changing the default. Because every major uses the same arm, these numbers
measure when the field produces a culture winner, not an individual's win
probability against unchanged rivals.

Commands (the process returns success only when all requested Culture games
finish as Culture):

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen --bin victory_eval
target/ci/victory_eval --target culture --deployment --games 4 \
  --players 6 --width 74 --height 46 --turns 250 --speed online \
  --start-seed 910131000 --without culture-cold-war-window
target/ci/victory_eval --target culture --deployment --games 4 \
  --players 6 --width 74 --height 46 --turns 250 --speed online \
  --start-seed 910131000 --with culture-cold-war-window
```

The measured `victory_eval` SHA-256 is
`7631f9d6d5374dda8addb98b0f2380735598ae18517213599186e000c74ca03b`.
Raw logs and provenance are retained in the host's
`civvis-culture-evidence-20260910/cold-war-window-bec42aae7` directory.

## Emperor adaptive-seat screen

All twelve games completed (72 seats): 17 enabled and 55 disabled. Enabled
seats won 2/17 versus 10/55, a −6.42 percentage-point contrast with a reported
standard error of 8.25 points. Score share differed by −2.58 points (reported
SE 1.13). The small sample and seats sharing games do not establish a reliable
win-rate effect. Ten games ended by Science, one by Religion, and one by
Culture. This is not a dedicated Pericles Culture evaluation.

The screen used six majors, 74×46 Continents, nine city-states, Online speed,
a 250-turn cap, Emperor, shuffled civilizations, the deployed background and
only this gene varied at probability 0.25. Its clean source revision is the
same checkpoint as the all-Culture comparison; its binary SHA-256 is
`9c2093ead5c0ae21e8da9d8f067f0ed4d95385851013d02a0e690278ee492d81`.

```sh
target/ci/gene_screen --games 12 --jobs 2 --genes culture-cold-war-window \
  --difficulty emperor --start-seed 910130000 --out screen.jsonl --quiet
target/ci/gene_screen --analyze screen.jsonl \
  --json docs/gene_screens/fires/culture-cold-war-window.json
```

The committed analysis records complete batch coverage and provenance.
This pilot is a firing artifact, not a new source for the pooled deployment
ledger; the generated ranking therefore still labels it unmeasured there.
The matched games independently demonstrate changed outcomes. Neither test
supports promotion: the gene remains off by default.

## Separate live prerequisite verification

The fresh Pericles run `civvis-20260910T130546Z` uses deployed revision
`482136653f4d6874a2ecab1598fb8eebd28c4eee`, not this experimental strategy.
Its host events record Li Bai activating at turn 69, moving to a tile with
`open_slot: true`, then activating again at turn 70. Subsequent state readback
reports two Great Works and 4 tourism per turn. This verifies the Great Work
slot correction from #3380 in a real game, but is not a Cold War treatment
result or a demonstrated win-rate increase. The evidence is retained as
`civvis-culture-evidence-20260910/post-fix-first-writer.json` with the run's
binary provenance.

The original segment and first continuation stalled at turn 72. The managed
supervisor recovered from the turn-68 autosave on its second continuation,
which passed turn 105 with eight cities and 12 tourism per turn. These are
segments of the same game family, not independent trials. No completed
post-fix outcome is available at this checkpoint.

At exact turn 150, the same continuation reported eight cities, eleven Great
Works, 18 tourism per turn, 122.461 culture per turn, and zero foreign
tourists. This confirms better conversion of recruited people into works;
it does not establish competitive tourism or a victory. The raw state is
retained in `civvis-culture-evidence-20260910/post-fix-turn150-state.json`.

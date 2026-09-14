# Patronage that actually closes the era

`age-closer` lifts the ordinary Great Person purchase restriction whenever a
seat is one to four points below its Normal Age threshold. A purchase does
not always supply those points: `Game::claim_great_person` awards one for a
normal recruitment, or three when patronage buys more than half the points.
The historic-moment catalogue, Taj Mahal, Dedications and the person's
activation can change the final result. The original also ignores when the
era will end.

The separate `age-closer-2` arm requires a known, unexpired era deadline no
more than five Standard turns away, and before the game ends. For each
otherwise eligible, affordable purchase it applies the actual patronage
action to a disposable game branch. The emergency exception opens only if
that result reaches the current Normal Age threshold. Among valid purchases,
a verified closer takes priority; the ordinary affinity and price score
breaks ties. Gold and Faith reserves, host offers, activation requirements,
Culture work slots and the existing one-purchase-per-turn limit remain.

Version one is retained. Version two does not forecast a free recruitment or
other future era-score events, does not chase Golden Ages, and cannot use the
emergency exception when the era deadline is unknown. These are limitations
of the candidate, not evidence of a strength improvement.

## Fixed evaluation plan

Registered on 2026-09-14 before any candidate games:

- Twelve family-probe games, seeds 914357500–914357511, naming `age-closer`
  and `age-closer-2` together. Other genes stay at the evaluator baseline.
  This artifact supplies reach evidence; its sample is too small for a
  strength claim.
- Thirty-six whole-registry games, seeds 914359000–914359035, using the normal
  independent seat genomes. Compare v2 with both off and v1 and report the
  errors clustered by game. Do not replace a deployment default from an
  unresolved screen or silently turn this sample into a broad batch publish.

Both runs use the normal six-player, 74×46 Continents, nine-city-state,
Online 250-turn Emperor shape. The executable records the actual target mix,
player contract, source commit and binary fingerprint. Three workers run the
family probe; four run the whole-registry screen. Raw rows remain outside
the repository. The analyzed evidence is committed under
`docs/gene_screens/fires/age-closer-2*.json`.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes age-closer,age-closer-2 --games 12 \
  --start-seed 914357500 --jobs 3 --difficulty emperor --out FAMILY_ROWS
target/ci/gene_screen --analyze FAMILY_ROWS \
  --json docs/gene_screens/fires/age-closer-2.json
target/ci/gene_screen --games 36 --start-seed 914359000 --jobs 4 \
  --difficulty emperor --out SCREEN_ROWS
target/ci/gene_screen --analyze SCREEN_ROWS \
  --json docs/gene_screens/fires/age-closer-2-registry.json
```

## Validation and results

Pending. Tests exercise actual patronage, the half-price boundary, Taj Mahal,
Dedication rules, deadline and speed boundaries, reserves, host refusal,
source-state preservation and choice between an ordinary purchase and a
verified closer. The full Rust suite and repository gene gates run before
integration.

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

The implementation at `4f8addc698` passed 3,707 Rust tests, with 53 ignored
including documentation examples. Twelve focused cases exercise actual
Gold and Faith patronage, the half-price boundary, Taj Mahal, Dedication
rules, deadline and speed boundaries, reserves, host refusal, source-state
preservation and choice between an ordinary purchase and a verified closer.
They include the actual player observation contract. The generated gene
metadata and fourteen append-point tests also passed. CI identified formatting
in the modified purchase hook; that formatting is corrected before the first screen.

Before any age-closer games start, this branch imports the engine ownership
repair from PR #3578. An earlier government probe exposed a missing owning
city panic in the shared Builder improvement query. The repair rejects that
stale handle and has two passing regression tests. No age-closer games have
been discarded or replaced. The first age-closer screen started from clean, pushed source
`9d35d6c6271878ca2937294574a15bfd1898a796`, with the original seed windows
and sample sizes. All twelve age-closer cases and both Builder ownership
cases passed on that combined source, and the changed-line formatting and
Clippy gate passed. The completed binary was copied to an immutable path
and its SHA-256 verified before launch:
`25decfc8d2e0c04a491adde335b77eb3e003c61ed008e7868e226c22b58dd128`.
Each phase explicitly stamps that verified revision. The header confirms the
planned map, clock, Emperor difficulty, observed-player contract, native
competitions and all seven target lanes. The running screen does not use the
later source integration with main; completed evidence will be recorded here.

The strength comparison is pending; deployment defaults remain unchanged.

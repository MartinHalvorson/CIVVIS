# Visible identity supersedes a remembered ranged threat

Hostile-memory v3 already treats a visible identity as superseding its old
capture forecast, including after the unit changes to a friendly owner. The
ranged forecast still excluded only visible enemies. A converted ranged unit
could therefore leave a false hostile firing envelope behind.

V3 now applies the same visible-identity rule to ranged projections. Visibility
and stealth detection remain required; a hidden native unit's changed owner is
not an observation. The historical sighting is retained, so a unit missing after
a speculative kill still contributes its remembered threat. Older versions and
deployment defaults are unchanged.

Before collecting results, the complete-game check is fixed at six standard
Emperor games, seeds 914357900–914357905, two workers, with all three hostile
memory versions. Use six players, 74×46 Continents, nine city-states, Online
speed to 250 turns, all victory conditions and standard genome probabilities.
This is a regression/activation probe, not a new strength comparison; the larger
192-game v3 comparison retains its originally frozen binary and seed window.


## Completed regression probe

All six reserved games completed: 36 unique `(seed, seat)` records covering
914357900–914357905, with every genome matching the 311-gene build header.
Endings were two culture, three science and one diplomatic victory. Only one
seat enabled v3 (zero wins), versus 35 other seats (six wins). This is positive
configuration exposure, not proof that the ownership-transfer edge case fired
in a game; the three focused tests exercise that behavior directly.

The analyzer reports a raw v3 win difference of −17.14 percentage points
(SE 15.60) and score-share difference of −2.99 points (reported SE 0.09).
With only one enabled seat, the nominal share significance is not credible
strength evidence: the enabled group supplies no replication. No superiority,
regression-strength conclusion or default promotion follows from this probe.
The full unmodified analyzer output is committed alongside this note.

The frozen executable was built from clean source
`a89cbd47bb5d425189d6e795271538cc3f6e69cc`. Binary SHA-256:
`a4058ba3558b29fb37970c16318f5f9c7974761889c8ad926dac098e9ed0cd0f`.
Ordered gene-list SHA-256:
`dc790bde269f21723795d967e1054e56cddad1fd0aedaead6d8cc639692e0478`.
Raw records and the frozen executable are retained under
`/Users/martbot/civvis-runs/2026-09-14-genes-three-hours/`.

Commands, from this task worktree (the binary path below is abbreviated):

```sh
gene_screen-a89cbd47 --genes hostile-memory,hostile-memory-2,hostile-memory-3 --games 6 --start-seed 914357900 --jobs 2 --difficulty emperor --out /Users/martbot/civvis-runs/2026-09-14-genes-three-hours/hostile-identity/activation.jsonl
gene_screen-a89cbd47 --analyze /Users/martbot/civvis-runs/2026-09-14-genes-three-hours/hostile-identity/activation.jsonl --json docs/gene_screens/fires/2026-09-14-hostile-visible-identity.json
```

Three focused local regressions passed. `cargo test --profile ci --locked`
passed 3,720 tests with 53 ignored on the same feature source. Clean main merge
`6afb22fdabd2d4e6d7922a559795979ccf90a311` passed a local CI-profile type check,
generated gene/manifest/evidence checks, and all remote CI gates. The earlier
192-game comparison uses source `e994e562a` and predates this follow-up; it is
not an exact evaluation of this patch.

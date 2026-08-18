# Closed experiment executables

These binaries preserve reproducible evidence for experiments whose registered
decision is already closed. They are deliberately outside `src/bin`, so normal
development and CI do not build or run them.

They remain available only with the opt-in feature:

```sh
cargo test --profile ci --locked --features closed-experiments --bin science_parallelism_eval
cargo test --profile ci --locked --features closed-experiments --bin reactor_conversion_eval
cargo test --profile ci --locked --features closed-experiments --bin faith_conversion_eval
cargo test --profile ci --locked --features closed-experiments --bin q_override_train
cargo test --profile ci --locked --features closed-experiments --bin q_pairwise_calibrate
```

- `science_parallelism_eval` — development screen STOP; retain `AdvancedAi`.
- `reactor_conversion_eval` — registered null/STOP; retain stock `AdvancedAi`.
- `faith_conversion_eval` — development STOP; no gameplay integration.
- `q_override_train` and `q_pairwise_calibrate` — the recorded corpus cannot
  support a trustworthy selective override.
- `q_train`, `q_counterfactual`, `q_advantage_train`, `q_dataset` — the rest
  of that Q line: its dataset emitters and trainers, moved here with the
  closed decision they served.
- `ablate` — the oracle ablation harness (docs/EVAL.md); `src/oracle.rs`, its
  only consumer, rides the same feature.
- `fog_census` — docs/FOG_CENSUS.md; reproducible command lines in the doc.
- `expansion_investment` — docs/EXPANSION_INVESTMENT.md.
- `gene_objective_probe` — docs/GENE_OBJECTIVE.md.
- `terminal_faith_census` — docs/closed/TERMINAL_FAITH_OPPORTUNITIES.md; the
  religious lane is measured dead in deployment (n=142, median 0 faith buys).

Do not reopen a result by tuning, rerunning, or changing these binaries in
place. A future experiment needs a new preregistration, fresh independent data,
and its own executable or module. The associated documents retain the protocol
and recorded result.

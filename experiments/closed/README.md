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

Do not reopen a result by tuning, rerunning, or changing these binaries in
place. A future experiment needs a new preregistration, fresh independent data,
and its own executable or module. The associated documents retain the protocol
and recorded result.

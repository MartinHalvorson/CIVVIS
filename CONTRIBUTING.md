# Contributing

- Pure Rust; serde is the only dependency. Keep it that way.
- All game content changes go in `data/*.json`, not code.
- All state mutation goes through `Game::apply`; new actions need: an
  `Action` variant, a handler, `legal_actions` coverage, and a test.
- Run `cargo test --release` and a `civvis soak` before pushing.
- Determinism is sacred: any randomness must come from `game.rng` or a
  seeded AI-local `Rng`.
- The GUI (`web/index.html`) must only speak the JSON protocol — no
  engine-specific coupling beyond `/state`, `/action`, `/rules`, `/new`.

CI: the workflow file is staged in `ci/tests.yml`; copy it to
`.github/workflows/` (needs a token with workflow scope or the GitHub UI).

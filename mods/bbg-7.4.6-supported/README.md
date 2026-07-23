# BBG 7.4.6 supported modifier slice

This overlay pins the current BBG 7.4.6 SQL rows that CIVVIS' generic
modifier runtime and policy model can execute exactly today:

- Qin Shi Huang's adjacent Great Wall `+1 Gold` and `+1 Faith` rows;
- Sumeria's river-adjacent Farm `+1 Food` row;
- the December 2025 adjacency-card rework: four early cards become 50% plus
  flat city yields, Empirical Method and Master Artisans provide the
  intermediate 100% cards, and the replacement chains execute;
- the March 2026 Communism changes: its slot layout and legacy Production per
  population, plus government-exclusive Scientific Vanguard and Kolkhoz.

The overlay currently contains 15 generic modifier rows plus their policy and
government data.

It is intentionally named a **supported slice**. It does not claim to be the
complete BBG ruleset and the dated CPL tournament preset does not activate it
implicitly. Use both explicitly:

```sh
civvis simulate --mods mods/bbg-7.4.6-supported \
  --tournament-preset cpl-ffa-2026-07
```

Source: BBG release tag `7.4.6`: `sql/Base/China.sql`,
`sql/Base/Sumer.sql`, `sql/Base/Policies.sql`, and `sql/Base/Government.sql`.

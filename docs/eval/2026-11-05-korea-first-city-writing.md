# Korea's first Writing opportunity after Pottery

In the qualified fresh Deity Korea Science pilot `civvis-20261105T090547Z-forge-writing-pilot-01`, Writing was offered at 16/0 for 25 Science, with Pottery already known. The capital was the only owned city when research was chosen. The active `science-writing-before-the-luxury` option required two cities, so the planner chose Sailing toward Rocketry. It founded city two later in the same decision; the next exported state is 16/1 with two cities. Ongoing Sailing then kept its research slot, followed by urgent Archery. Writing finished at 33 and the first Campus at 38.

Extend the existing opt-in's early Writing slot to Korea with exactly one owned city and known Pottery. Its existing two-city behavior stays intact. Other civilizations' first cities, the pre-Pottery opening, disabled Science, the flag off and already-owned Writing have no new goal. The research dispatcher is unchanged: urgent military goals and ongoing research keep their priority. No new flag, registry append, default promotion or waiver is introduced.

## Retained native evidence

The neighboring `.native.tar.xz` archive has SHA-256 `8054b529d9232079ffd632d996e3ff7c6653f6eed961e929b8fa253cc78c544c`. Extract it and run `python3.12 verify-evidence.py`. It verifies the canonical native decision, all 18 identical actual captured reads, native Writing availability and the next two-city state. The original binary is source `624d0eeafe4a27e43a27ab7f828e3029dd751722`, SHA-256 `d9546d69ed4aeec45229837b22b508690a6d357fe15d0c50d7d0e3ea4259f8a6`.

Two additional original-binary controls use fresh private planners on that same frozen prefix. Both choose Sailing with Writing priority active; all 17 actual reads per control match the prefix and their full orders/decision replies are stable. These controls reproduce the research choice, not the original controller's entire private state or native execution.

The archive retains the initial proposed-new-gene protocol and its implementation amendment, written before any changed candidate build or query. The implementation reuses the existing Writing option; a future same-candidate OFF/ON comparison will therefore toggle that existing flag. No changed candidate has yet been queried at this checkpoint.

The original full pilot ended in a rival Science victory at 203, with Chemistry only at 189 and no own space projects or Rocketry. This single changing-source/background exploratory game supplies no strength estimate or claim that earlier Writing would have won it. Future performance trials must freeze a current main binary and current actual runtime profile rather than treating this historical background as current deployment.

## Validation

Scoped rustfmt and `git diff --check` pass. The library module has 11 focused tests: three new research regressions and the existing coverage, with ranged-defense, rush and ongoing-research checks extended to both the one-city and two-city cases. Rust execution, full CI, focused Mac tests and the predeclared candidate proposal comparison remain pending. Engine rules and the research dispatcher are unchanged.

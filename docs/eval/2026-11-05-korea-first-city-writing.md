# Korea's first Writing opportunity after Pottery

In the qualified fresh Deity Korea Science pilot `civvis-20261105T090547Z-forge-writing-pilot-01`, Writing was offered at 16/0 for 25 Science, with Pottery already known. The capital was the only owned city when research was chosen. The active `science-writing-before-the-luxury` option required two cities, so the planner chose Sailing toward Rocketry. It founded city two later in the same decision; the next exported state is 16/1 with two cities. Ongoing Sailing then kept its research slot, followed by urgent Archery. Writing finished at 33 and the first Campus at 38.

Extend the existing opt-in's early Writing slot to Korea with exactly one owned city and known Pottery. Its existing two-city behavior stays intact. Other civilizations' first cities, the pre-Pottery opening, disabled Science, the flag off and already-owned Writing have no new goal. The research dispatcher is unchanged: urgent military goals and ongoing research keep their priority. No new flag, registry append, default promotion or waiver is introduced.

## Retained native evidence

The neighboring `.native.tar.xz` archive has SHA-256 `8054b529d9232079ffd632d996e3ff7c6653f6eed961e929b8fa253cc78c544c`. Extract it and run `python3.12 verify-evidence.py`. It verifies the canonical native decision, all 18 identical actual captured reads, native Writing availability and the next two-city state. The original binary is source `624d0eeafe4a27e43a27ab7f828e3029dd751722`, SHA-256 `d9546d69ed4aeec45229837b22b508690a6d357fe15d0c50d7d0e3ea4259f8a6`.

Two additional original-binary controls use fresh private planners on that same frozen prefix. Both choose Sailing with Writing priority active; all 17 actual reads per control match the prefix and their full orders/decision replies are stable. These controls reproduce the research choice, not the original controller's entire private state or native execution.

The archive retains the initial proposed-new-gene protocol and its implementation amendment, written before any changed candidate build or query. The implementation reuses the existing Writing option. The predeclared same-candidate OFF/ON/OFF/ON comparison toggled only that option, with all four scheduled roles retained.

The original full pilot ended in a rival Science victory at 203, with Chemistry only at 189 and no own space projects or Rocketry. This single changing-source/background exploratory game supplies no strength estimate or claim that earlier Writing would have won it. Future performance trials must freeze a current main binary and current actual runtime profile rather than treating this historical background as current deployment.

## Validation

Scoped rustfmt and `git diff --check` pass. The library module has 11 focused tests: three new research regressions and the existing coverage, with ranged-defense, rush and ongoing-research checks extended to both the one-city and two-city cases. The source `6f742b709f59e2a0d8f70120429ab3b6d3119e52` passed all 11 focused Mac tests in hosted build [42009187946](https://github.com/MartinHalvorson/CIVVIS/actions/runs/42009187946); both native binaries passed their smoke checks. Its Linux CI passed 6,984 Rust tests, with 53 skipped. Main was merged once without changing either Writing source file; integrated CI is recorded separately in the PR. Engine rules and the research dispatcher are unchanged.

## Same-candidate native proposal comparison

The candidate archive `2026-11-05-korea-first-city-writing-candidate.native.tar.xz` is 202,128 bytes, SHA-256 `dd46509030fa3b811635822df5db83365c481e27b0ee5eee0d8e48b720336a85`. Extract it and run `python3.12 verify-candidate.py .`. Verification also passed after independent extraction. The tested candidate source is `6f742b709f59e2a0d8f70120429ab3b6d3119e52`, native `civvis_orders` SHA-256 `60e33c7364fe9f5abfd686a158444e43603d3ea520276cc9a05fa6007200d502`. The archive includes the build receipt, protocol, raw replies, actual genomes, captured reads and portable verifier; the binary is omitted.

Both OFF roles chose `TECH_SAILING`; both ON roles chose `TECH_WRITING`. Each role made 17 actual reads, all byte-identical to the preselected native 16/0 prefix. The verifier checks those bytes, exact repeated full orders/decision replies, actual Science assignment and that the actual genomes differ only in the existing Writing flag's forced/treatment membership. No later input, replacement world or retry was selected.

This establishes the intended research contrast on the captured native public input using fresh private planners. It does not establish original private-state parity, native order execution, earlier Campus completion, survival or victory strength. The option remains default-off; no performance promotion follows from this proposal check.

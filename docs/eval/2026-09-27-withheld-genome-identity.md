# Explicit native withholds must appear in startup identity

The #3806 preflight rejected a native OFF arm whose startup genome still
listed `counter-in-lane` as active. `configure_live_bridge` already applies
validated `--without` setters after ledger and force configuration. The
report instead used the ledger/forced projection without those withholds.
The defect concerns controller identity; it does not establish that the
runtime option stayed enabled.

The startup `treatments` projection now removes explicitly withheld tags,
retaining registry order. Configuration, action selection, deployment defaults,
the force bundle, and the stdout decision protocol are unchanged. The `forced`
field still records the canonical force selection; effective activity is
reported by `treatments`. No live policy is activated or withheld by this patch.

## Failed-first reproduction and controls

Registered at **2026-09-27T04:31:43.769693+00:00** on claim
`e40c2112abb5b0b4fea9772c84a11410e9fc20d6`, parent main
`59dc53c6dffc7db1248c8a6b3e9816c6acf60ff3`.

Commit `976ae1522895921bd502b2063fa75ee470958c22` routed startup reporting
through a pass-through helper and added the regression. The exact filtered
binary test ran one test and failed: under the compiled 21-option bundle,
the configured Domination controller has `counter_in_lane=false`, while
its reported treatment list contains the tag. The failure is retained in
`failed-first.log` and `failed-first.json`.

The repair is `d2898707654081159afe5d9d27f9635a80529f83`. Four focused tests
compare reported identity with the configured controller: the failing case,
unchanged default and forced genomes, multiple repeated host withholds, and
mixed host/forced-repair withholds. The last case verifies that explicit
withholding overrides a forced `siege-commitment` in both the controller and
its effective identity.

Final validation at test-only commit `c094a6626` passes all four focused tests
and the full suite (4,410 passed, zero failed, 53 ignored). The ledger and manifest checks pass;
the firing ratchet reports 326/326 shown to fire and zero waivers. Rust quality
reports the changed lines formatted and warning-free. The build retains the
pre-existing `civilian_reach_safety_on` dead-code warning. No engine or gameplay
mechanic changes require an engine soak.

## Native replay on one policy baseline

Both builds use the same `59dc53c6d` policy baseline, which includes #3802.
The before binary is built at the parent-equivalent pass-through refactor
`976ae1522`; the after binary is built at `d28987076`. These are not the older
#3806 binaries, whose baseline predates #3802. The later test-only addition
changes no production code.

The frozen history SHA-256 is
`afd1616cbcdca0d5e88a470d992807524e8dca70e53800482dd423dd01b2d778`.
The unchanged 21-option bundle SHA-256 is
`439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`;
startup readback has the same 14 canonical forced tags in both arms.
Four persistent replays (before/after × OFF/ON) complete all 202 frames, exit zero.
OFF uses `--without counter-in-lane`; ON uses the deployed host default.
All retain an assigned Domination target.

| Check | OFF | ON |
|---|---|---|
| Before/after stdout bytes and complete JSON decisions | Identical | Identical |
| Before/after issued-order changes | 0/202 | 0/202 |
| Before/after startup genome changes | Only removal of the withheld tag from `treatments` | None |
| Effective tag after repair | Absent, unforced | Present, unforced |

Before binary SHA-256:
`7e2cfc70488e23d9acff5f6785f6aaf208ba10e4115df949d25fe666f8df902e`.
After binary SHA-256:
`ec72b3ce3b0243f39f57f719ce62dedb2d64262a6953d2a52fa05e8e2bca464e`.

Evidence: `~/civvis-war-evidence-20260926/withheld-genome-identity/` contains
registration, build/test logs, frozen binaries, commands, frame provenance,
complete decisions, genome readback, and `native-replay-comparison.json`.
The original #3806 registration, incorrect reports, and preflight abort remain
preserved. Its eight reserved fresh seeds have not been run. The experiment
must register the repaired, current baseline before starting any fresh game.

Fixed archived future states cannot establish counterfactual survival, conquest,
a native victory, or level promotion. This correctness repair provides truthful
control identity; its validation is not evidence of greater playing strength.

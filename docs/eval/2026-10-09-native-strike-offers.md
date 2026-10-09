# Native strike target offers

Publication and independent candidate CI are pending. The implementation is
locally committed; no native candidate game or win-rate improvement is established.

A completed four-player Gran Colombia game recorded 26 ranged refusals with
`can_start=false,no_reasons [p4r]`. That game used private revision
`46fa42f32163160d977320b0fb21b5642bf2869a`, so its refusals do not establish a
current public planning defect. They motivate more precise native observations.

## Shipped interface and implementation

`Base/Assets/UI/WorldInput.lua:2004–2015` calls
`GetOperationTargets(pUnit, UnitOperationTypes.RANGE_ATTACK)` and includes plots
only when `modifier == UnitOperationResults.MODIFIER_IS_TARGET`. Lines 2091–2097
use the same two-argument query and paired-row filter for `AIR_ATTACK`.
Membership in the raw `PLOTS` array alone is insufficient.

The actual ranged and air unit refusal handlers now add three flat fields:

- `native_target_known`: whether the complete native result was readable;
- `native_target_offered`: whether a target-modifier row names the requested plot;
- `native_target_rows`: the count of target-modifier rows, including duplicates.

Missing APIs/enums/plots, thrown reads, invalid enums, malformed values, sparse
arrays, and misaligned rows produce `native_target_known=false`; the other two
fields are omitted. Results are assigned only after every row passes validation,
so a malformed later row cannot leave a partial positive offer.

The existing refusal event, original reason, verdict and operation behavior are
preserved. The query runs once for an existing host refusal. Accepted shots and
the tested early diplomatic refusal perform no diagnostic query. The emitted
summary contains no native target list. A known absent offer is an observation,
not an explanation of why `CanStartOperation` refused the shot.

## Retained validation

All original Lua suites and Python installation assertions remain unchanged.

| Checkpoint | Evidence |
| --- | --- |
| Unchanged public controller, `413fb480c` | All 89 existing Lua suites passed; all 21 new cases executed, with 18 missing-metadata failures and three passing controls. This is a feature baseline, not a reproduced native gameplay defect. |
| Malformed-data extension, `24e226836` | 25/26 new cases passed; an invalid target enum incorrectly reported a known result. Failure retained, then fixed. |
| First Python installation run, `7ea60c523` | 135/137 passed; two existing guards rejected the changed emitter syntax. Their assertions were preserved and the original emitter form restored. |
| Final implementation, `8d07dbe6e` | All 90 discovered Lua suites passed, including all 26 new cases; all 97 scripts across both mod roots parsed under Lua 5.1. All 137 installation tests passed. |

The 11 production/type-name tests passed. The main-chunk local-slot guard counts
182 locals; the new helper adds no file-scope local. Registered validations
freeze 648 Rust/rules/build/Lua inputs and the exact clean commit.

The complete local Rust run at `7ea60c523` used unfiltered Nextest, zero retries,
and no fail-fast: **4,834 executed, 4,816 passed, 18 failed, 51 existing skips**.
Every failure was a server test denied permission to bind a local port. There
were no compiler errors or changing inputs. Repeated immediate/final failure
output represents 18 unique cases, not 36 executions. The documentation command
exited zero with zero failures and four existing ignored examples.

All 551 registered Rust/rules/build inputs are byte-identical to actual public
trunk `4ac783905`; its independent CI run
[37983720217](https://github.com/MartinHalvorson/CIVVIS/actions/runs/37983720217)
passed all 4,834 Rust tests. The final emitter repair changes only Lua. This
source comparison does not replace candidate CI or turn the local socket
failures into passes. Engine crash soak is inapplicable to this diagnostics-only
change; no Rust gameplay rule or AI policy changes.

Local retained logs have SHA256:

| Log | SHA256 |
| --- | --- |
| Original Lua baseline | `6afa36e28f549937003ab6d5aa45aac6a16bd055a7e2c67a92768f7937a0eea8` |
| Malformed-enum baseline | `5928b2af45e17581c9a5b777245aa6602a8b7de4cd277255f5c3e95866bde556` |
| Final Lua implementation | `9be61afc0337fe5fa198fe90cec26952b60659f304b857f65f0737811f062af9` |
| Initial installation guards | `f9dd8b5548b85f652143fb5df957dc0eceedc4840499770d167e9626c4a4cd40` |
| Complete local Rust run | `92991213236ea7f197ddae46f0e71cf02c5489a04173b0883d1c6fab2e84acef` |

## Integration and native evidence limits

The branch started from `4ac783905`. A connector read confirmed that exact
GitHub main, and the one pre-ready merge reported already up to date.
Git push failed DNS resolution. The connector rejected updating this PR because
the tool requires approval and the session policy is `never`. The PR remains
draft on its initial claim until publication and candidate checks can proceed.

Read-only follow-up matched all 26 historical refusal events to their recorded
decisions. Three proposed approach moves were absent from emitted orders. The
private fork contains friendly-column retargeting absent in public main; causal
attribution and a current public regression remain unproven. Raw distance
comparisons also omit wrapping and cannot assume exported range includes
adjacent support bonuses. No extra range veto is introduced from those records.

Native pins, installed mod and verification processes have not been changed.
Mocked SDK results establish the handler contract, not native attack acceptance,
CPU cost, survival improvement or domination strength.

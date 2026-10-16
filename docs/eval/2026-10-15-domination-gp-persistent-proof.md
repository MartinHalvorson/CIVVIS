# Persistent Domination production proof

The shipped #4072 repair also changes the production-mode decision at the original turn-178 opportunity: the Spaceport order disappears and an Artillery next-production lease is proposed. Every other order keeps its content and relative order. This is an audited comparison of existing outputs, not a new game or executed host action.

The earlier cold one-shot check proposed retaining Government Culture. Those diagnostics deliberately start with empty production ownership; production's `--serve --fresh-board` path instead preserves the agent, skips the opening book on a late first observation, and adopts current host production. The persistent comparison below uses the same initialization path in both traces.

## Matched evidence

Both completed traces contain the same 41 original opening prefixes, turns 160–200 of `civvis-20261015T120650Z`, with the same 283 common Native flags and exact CLI arguments after removing only binary and storage paths. Their actual captured archives equal the shared original 61,648,701-byte stream. Turns **160–177 are identical in full**, excluding only top-level input-capture metadata. The first divergence is turn **178**, frame zero, on the original 47,876,534-byte prefix with SHA-256 `90ea6d78384a84998bb1de07fa6dd03e1c6238f353cb55869b82737e746ada14`.

| Before | After |
| --- | --- |
| `produce DISTRICT_SPACEPORT`, city 131073, (21,15) | `produce_next UNIT_ARTILLERY`, city 131073 |

The input has Maracaibo's Government Culture building at 210/220 with one turn left, and physical Stephanie Kwolek needing a completed Spaceport. The old request starts a 900-production Spaceport. The new `produce_next` request is a deferred handoff: `tools/civ6_control/mod/CivvisControlAgent.lua:14824` stores `civvisBuild[tostring(cityId) .. ":next"] = resolved;`; `driveProduction` must later consume the lease at the production blocker. It does **not** immediately add an Artillery unit or replace current production. No lease acceptance, blocker consumption, or completed Artillery is observed here.

The before source is `dc67b182ea3594f50f9a9e747b9ab397bf340ab4`, the validated #4071 integration build. The after source is `88abc8ab14f505b43c927ba2aeebbc4708f626cc`, the immutable baseline later used for #4073. Its production source matches shipped #4072 main `dd553446902c52c21b935b027f5fd012a7b924c6`; its extra refusal-reader fixtures and module declaration are `cfg(test)` only. Source inspection finds only the #4072 Basic/Advanced production-context repair as a functional change between these binaries. No rule, data, Cargo, build input, forcing, target, or mode changes explain the difference.

## Interpretation and limits

This supports the intended opportunity at the first changed decision: the Domination controller stops reserving the Spaceport for a space-only Great Person and can hand the next production choice to its existing artillery planner. It does not show that the host accepted or built the Artillery, that an army reached a city, or that Domination became more likely.

Every later difference is retained in the summary and both complete traces. They are **not a candidate host continuation**: the inputs still describe the original game's Spaceport investment after turn 178, while the repaired agent proposed a different order. Later production, resource, and victory outcomes cannot be inferred from that counterfactual feedback. This remains one historical world and no new Native or headless game.

The comparison is post hoc, after the source repairs and fixed performance studies completed. It changes no code, policy, gene, setting, forcing set, verification rule, or registered Native cohort. The separate #4073 CPU result remains its own fixed comparison; this documentation adds no timing arms.

- [Audit, exact arguments, all prefix hashes, and every later difference](2026-10-15-domination-gp-persistent-proof.json)
- [Both complete raw traces and captured-input indexes](2026-10-15-domination-gp-persistent-proof.traces.json.gz)
- [Complete source delta, including test-only support](2026-10-15-domination-gp-persistent-proof.source.patch.gz)
- [Shared original input stream](../performance/2026-10-15-native-base-world-cost.input.events.jsonl.gz)

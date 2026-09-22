# Intercept an urgent religious threat without a known enemy city

Native `civvis-20260922T051229Z-cont2` lost to Arabia's religious victory on turn 131 on source `9d8c98737c230390a97ad7c650c55b1506d296b6`. All seven of our cities followed Islam by turn 79. Arabia's cities were still undiscovered at turn 84, but its Missionaries were visible beside our army. The ordinary conquest-denial gate requires a known enemy city.

The change supplies a separate, immediate religious interception: an explicit Domination seat facing a religious match point may declare a legal war and condemn a currently visible religious unit at home. A speculative declaration, optional adjacent movement and condemnation must all succeed first. It retains affordability, home-threat and single-major-war gates, requires a healthy military unit with movement, and executes the promised interception rather than merely switching plans. No distant city siege is assumed.

## Native command evidence and a corrected interpretation

`Base/Assets/Gameplay/Data/UnitCommands.xml:47` registers `UNITCOMMAND_CONDEMN_HERETIC`; the shipped command has no target parameters. The existing bridge requests it on the military unit after co-location.

Retained strings in `Base/Assets/Text/en_US/InGameText.xml:3138–3142` describe founded-faith and majority-faith refusal reasons. An initial reading treated those strings as proof that the turn-83 interception was impossible. The actual runtime contradicts that conclusion: at turn 126, Arabian Apostle `5701640` explicitly carries `RELIGION_ISLAM`, all our cities follow Islam, and our Caravel `3145731` moves to `(44,10)`. The native event stream records `condemn_removed` at `05:41:14.875Z` and confirms `CONDEMN_HERETIC` through `order_verified` on turn 127. Therefore the unused/conditional text alone does not establish an unconditional prohibition, and no speculative simulator restriction is added here. This corrects the preliminary note committed during investigation.

## Validation

The base regression failed as intended; the negative-control test passed. Both candidate tests pass. On integrated source `933e08150759353624ec3f1737c3fed383d8aa99`, `cargo test --profile ci --locked` passes 4,186 tests with 53 ignored, changed-line Rust quality passes, both release binaries build, and eight four-player, 180-turn soak games complete from seed 372100. The treatment append checks pass 14/14.

Isolated replays compare baseline `9b1a5a49b5409334df520165dc0c3421e5df8117` against prototype `55997fbad2291aaac42ed688f9e387ffee4f393f`. The original segment completes 231 frames: nine exported-order frames change, six also change internal actions, beginning at turn 83/frame 2. The independent restarted segment completes 132 frames: five exported frames change, four also change internal actions, between turns 86 and 88. Earlier decisions are unchanged. These overlapping segments are not concatenated into a continuous counterfactual game.

The first changed frame exports a declaration against Arabia and two condemnation orders, including a movement followed by condemnation. Repeated declarations in the replay reflect the same recorded peaceful boards being restored, not multiple native wars. Effective treatments, forced flags and withheld genes match the native logs for both binaries. The integrated revision additionally includes the separately tested score-response change; the isolated replay claim remains scoped to the revisions above.

SHA-256 provenance:

- Baseline binary: `69c44b3e359d1649bfa8354bef6c99bc27e2f0c808191a62c8e3038639a6c28d`
- Candidate binary: `a8c959713b4eed8389e6c30756945ad49c0ac1d1e7c2f1d0d379f3d0b44e7b76`
- Original input: `b22d9532c346f2e94723e67b336fc13a91feeef35ff03b6f1e4c4010062035f2`
- Restarted input: `6cb232fd893271b812b07b1ad9a25f09a54310c7163f24220b58b69f4071c4df`

The verification host retains the comparisons and argument provenance under `/tmp/civvis-3721-interception-replay` and `/tmp/civvis-3721-interception-cont2-replay`. Concurrent timings are not performance evidence. A successful native condemnation supports the command pathway, while replay establishes newly proposed actions; neither proves an avoided loss or a Domination victory.

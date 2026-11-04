# First Spaceport and selected infrastructure repairs

Native Immortal Australia `civvis-20261103T070446Z` chose Geelong for its first Spaceport at turn 159 while it produced 63 per turn against the best legal city's 72. The Industrial Zone and buildings were then pillaged, and production fell to 26. At 160/frame0 the repair policy selected a Commercial Hub district repair, but the first-pad hold replaced it with the Spaceport, quoted at 44 host turns. This is a priority conflict; the selected repair is not an Industrial Zone repair.

Default-OFF `launch-pad-spares-the-repair` lets an already selected, still legal district or building repair keep that first-pad queue. It selects no new repair and requires the original pad hold. Ordinary production still yields to that hold; existing later defense and loyalty decisions retain their authority.

The searched Nov 1–3 journals contain 198 matching lines, 72 distinct run/turn/city observations in 17 runs (9 King, 3 Emperor, 5 Immortal). Replans account for repeated lines. This census establishes repeated proposals, not causal losses or repair execution.

Source `93f036e6ba14c2fa29ee8c6cc3a614d16588a5f7`, based on `2bd9ae08e752ae088867b3c4147f8456b290e384`; CI-profile executable SHA-256 `f0e328faa0a00a5ab104dce27a72e94fb7f6c4e5797967fa9055231ef4a46222`. Current main has since advanced; these results describe this source, not a later integration.

- Seven focused optimized Rust interaction tests passed.
- Full four-thread Rust suite: 6,718passed,0 failed,56ignored. Initial fixture API compilation errors are retained with their corrections.
- Original native program: two complete 386-request controls through turn160, with full payloads and actual captured read count/order/bytes matching.
- Candidate OFF: two complete 386-request controls match. ON: both repeats first change orders at 160/frame0 after 384 matching order lists, stopping at that changed request with matching read and frame identities.
- Twenty fresh queries across four captured boards: older/OFF and repeated OFF/ON full-payload controls match; one case changes orders and three do not. The changed native command is the Geelong Spaceport order becoming the Commercial Hub repair order. These fresh boards do not continue a counterfactual game.
- The first persistent OFF attempt timed out at 117/frame1 and is retained as unqualified. The unchanged-source retry passes all four controls. The first independent-verifier run lacked an imported helper; that dependency and failure log are retained.

The raw archive is `13,942,076`bytes with `194`members: `docs/eval/2026-11-03-launch-pad-repair.raw.tar.xz` (SHA-256 `019daf2f0257e1eb0f45805d505ca4514879e8d01aaca6bfb202ce0c7022d943`). Each retained member is independently hashed. `verify.py` reconstructs source snapshots, repeated replies, profiles, ordered actual reads and the journal census without executing a game.

No native repair execution, faster Spaceport completion, saved city, held-out confirmation, win-rate effect, default promotion or native treatment is claimed. Development inputs were reused. The original draft was closed during operator cleanup while this implementation remained local; source and evidence are preserved on its task branch. Review current-main integration and eligibility for a controlled native pilot before publication or promotion.

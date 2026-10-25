The first eight native games after the income-selector delivery all lost.
This follow-up traces actual construction and one interrupted Market, so a
missing district at turn 100 is not mistaken for either a rejected order or
a reason to override every competing production reservation.

All eight members are retained, including the last member's overlap with the
industrial-guard cohort. They span six source revisions. Outcomes and the
initial turn-100 census were already known before this descriptive audit;
this is not an outcome-blind, paired or causal strength trial.

Attempt stem       First income request   First complete income observation
20261025T202038Z    167                    none before terminal outcome
20261025T203421Z    105                    125, Commercial Hub
20261025T204502Z    none                   none before terminal outcome
20261025T205235Z    118                    123, Harbor
20261025T210315Z    139                    141, Commercial Hub
20261025T211349Z     91                     98, Commercial Hub
20261025T213230Z    111                    113, Commercial Hub
20261025T215348Z     95                    101, Commercial Hub

Seven games requested income production, with a median first request at 111.
Six eventually had a completed income district, with a median first observed
completion at 118 among those six. One had a completed standard Hub at the
first turn-100 state; none had a completed standard Harbor there. These are
export observations, not exact engine completion timestamps. Reissued orders
are not extra buildings. The complete requests, verification receipts,
budgets, city/type completion observations and intervening queue heads are
retained in income-eight.json.gz, alongside the frozen observer index.

At turn 100 six of eight focal empires had fewer citizens than their exported
science leader. The other two had more citizens and still less Science. The
median leader/focal Science ratio is 2.77; normalizing each by its own public
population leaves a median ratio of 1.94. This separates empire size from
yield intensity without identifying their causes or removing native leader
and difficulty differences. Rival population uses public_stats totals, not
the incomplete list of currently visible cities. City growth observations
are included for every member rather than selected from the worst games.

In the selected 215348Z history, the capital starts a Market at 101 with a
three-turn quote and first shows it complete at 118. Exact turn-102 captured
input has 1 Gold, +3.5 Gold/turn, the Market in production and a two-turn quote.
The actual root caller replacing it is claim_capital_walls_before_the_surprise;
the Knight is both transported and host-verified at 102. Later transported
capital reservations are an Aqueduct at 105, Catapults at 108 and 111, and a
University at 113. Ordinary rescoring returns to the Market at 117.

This is evidence against classifying that interruption as an unfunded
industrial reservation or automatically imposing Market-first priority. It
does not establish that the defense, housing or siege choices were optimal;
changing those priorities requires their own cost and survival evidence.

The caller diagnosis has two original-binary controls and two diagnostic
controls, each reproducing all 307 complete orders/decision payloads and all
5,336 ordered captured reads through turn 118. Both complete diagnostic traces
match: 69 root frames and 63 Produce calls. Original argv/cwd are reconstructed
from the actual brain configuration and exact launcher, not directly observed.
Their full-prefix controls qualify the reconstruction for this history.

Diagnostic source starts at exact d183ac33d5f6c37f6310eadd974cf911467fafd3.
All 830 source/input blobs are verified before a root-Game pointer/frame setter
and a track_caller Produce wrapper change only two files. Simulated clones
are excluded. The preserved private executable is identified in registration;
the seven-minute build ran under the existing CPU controls. An initial Python
indentation error happened before instrumentation writes or a build, was
corrected after reverifying every source blob, and remains in the receipts.

Root model choices must remain distinct from native orders. For example, the
capital's Aqueduct attempts at 96/frame 1 and 2 and its Knight attempts at
101/frame 1 and 2 produce no transported order. The actual Knight at 102 does.
Whole-turn journal lines or a count of root calls alone cannot establish that
the host executed an alternative choice.

qualified-market.json.gz contains registrations, source/build receipts,
complete qualification reports, every diagnostic event and the exact native
turn-102 input/order/verification witness. complete-market-traces.json.gz
retains the complete original prefix and all four complete replay replies
and stderr logs. Captured-read byte streams remain in the named external
sealed directories, with full archive/index SHA-256 receipts; these large
streams are not embedded in the compact repository archive.

The exact executed continuity script is retained with its original external
ROOT path. Reproduction needs the frozen native archives at that root. The
existing constant-read-control.py and diagnostic-replay.py in the sibling
2026-10-25-native-income-callers directory perform the complete controls;
their invocation, exact arguments and input hashes are retained here. The
diagnostic-root.patch.gz (decompressed) and source-manifest reproduce the private source change.

This change is evidence only. It makes no controller, rule, gene, native
setup, force-bundle, screening-lease or game-input change.

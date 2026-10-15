# Materials for a fundable army upgrade

The completed Emperor attempt `civvis-20261014T230322Z` lost at turn 109.
At turn 89 it still fielded ten Heavy Chariots. Their native Knight upgrade
quote was 160 Gold plus ten Iron. The seat held no Iron and earned none;
peaceful Germany and Sumeria each exported 25 tradeable Iron. There were
no recorded Iron purchase proposals. Four Olives were held, so surplus
barter is an opportunity to investigate. Exported offers do not establish
an acceptable price, accepted deal, completed upgrade or successful defense.
The raw evidence and observations remain in
`~/civvis-tactics-results/2026-10-14/capital-recon-priority/native-collapse-context.json`.

The shipped resource buyer asks for siege-gun resources and the air wing's
Aluminum. This change adds the opt-in `army-upgrade-materials` policy for
one useful, affordable land upgrade. It reads the existing exact host
successor, Gold and material bill when present. The shipped UI uses
`DLC/Expansion2/UI/Replacements/UnitPanel_Expansion2.lua:94`:
`local upgradeResource, upgradeResourceCost = pUnit:GetUpgradeResourceCost();`.
The existing `Game::unit_upgrade_resource_price` applies that bill rather
than guessing the model's twenty Iron in the posed ten-Iron native case.

The request retains the upgrade's Gold plus the existing 30-Gold upgrade
floor, increased for negative income. It chooses one healthy, unlinked,
unreserved land unit with at least a 15-strength improvement. Existing
stock or the next turn's income retires the request. An unknown host
successor is not replaced with a guessed unit; an upgrade into a fuel-using
unit requires income that covers its upkeep. The existing upgrade pass and
native permission still determine actual execution.

The trade transport retains the siege buyer's existing budget and cadence.
The army request has its own cash reservation, includes planned spending,
uses no future-income credit, and can barter only the existing bounded
surplus. Native luxury counts protect the last copy. Already committed
sales, another purchase, hostile sellers and incomplete material lots
prevent an ask. Required army materials and the existing siege/air resources
are protected from surplus sale or barter.

The new library and transport controls cover the host bill, one fundable
upgrade from a large obsolete army, disabled and other-victory cases,
reservations, existing stock, invalid successors, cash limits, surplus
barter, missing or hostile sellers, busy deals and last-copy protection.
The integrated GitHub suite on `2041debaf` passed all 5,937 Rust tests,
including five library and seven transport controls, with 51 existing skips
and four ignored documentation examples. Rust quality and paired cost checks
passed. Metadata preserves all 141 deployment tags and the operator selection
note. The 21 evaluation-manifest, 17 fires-gate and 186 gene-tooling tests
passed locally, as did `cargo check --locked --all-targets` and all 14
append-point controls.

Review found the original transport caller read the untouched authoritative
mirror after planning had spent Gold on a separate copy. A complete decision
fixture buys a Builder and Monument, then the old caller still bids 386 Gold
for Iron against its original 600-Gold treasury while reporting 190 reserved
for a Knight. The corrected caller reads the planning copy. A separate
control rejects even fully bartered material when earlier spending used the
upgrade reservation. The failed-first production-path checkpoint reached
both intended assertions (five controls passed, two failed); both regressions
and all seven transport controls pass after the repair.

The complete local command
`CARGO_TARGET_DIR=/Users/martbot-mbp-m5-max-128/civvis-safe-muster-detour-2a33/target cargo test --profile ci --locked -j4 -- --test-threads=1`
passed all 5,937 Rust tests on `2041debaf`, with 51 existing test skips and
four ignored documentation examples. The prior four-thread local run had
5,599 passes and one unrelated spectator arrival timing failure (315 ms
against its 250 ms bound); the unchanged isolated test passed. An earlier
run hit the independent two-second spectator-stream window, which also
passed unchanged in isolation. No assertion was weakened.

A complete frozen private candidate, `077188c51043ac482c76e98c0837a8f8bc3ed558`,
passed 56 selected native controls on Apple Silicon with Rust 1.97.1.
All 2,516 tracked source-file hashes and both downloaded executable hashes
matched, and both downloaded executables passed local smoke checks.

Two frozen native-input probes preserve the recorded effective policy sets;
the ON arm adds only `army-upgrade-materials`. At turn 89/frame 0 of the old
Emperor game, the ON arm correctly holds its treasury: planning upgrades an
Archer to a Crossbowman for 125 Gold, leaving only 75 of the observed 200,
below the Knight's 190-Gold reservation. The raw missing-Iron opportunity
alone therefore does not establish a fundable purchase after planning.

At turn 104/frame 0 of `civvis-20261015T035232Z`, the observed 493 Gold and
host Warrior-to-Man-at-Arms quote (60 Gold plus ten Iron) permit a request
after other planning purchases. OFF submits no resource buy; ON requests
ten Iron from seller 3 for one Ivory and two Horses, with a zero-Gold ceiling
and 90 Gold retained for the upgrade and floor. Both actual captured input
archives match the same frozen prefix byte for byte. These probes establish
planning behavior on recorded native input, not an accepted native trade,
actual resource stock, completed upgrade, or full history replay.

The fires waiver records that simulator `gene_screen` does not execute this
CLI/native trade channel. No simulator null result is entered as proof that
the policy fires. No native resource-buy execution, win-rate improvement or highest-difficulty
victory is established for this policy.

The first live component pilot, `civvis-20261015T054045Z`, used the verified
complete frozen source `048be52e8` with all 270 effective parent policy tags
preserved and only `army-upgrade-materials` added. Its actual Emperor,
four-player Gran Colombia, Tiny Pangaea, Online Gathering Storm settings
matched the intended profile. All 418 recorded decisions used the same
verified executable and reported complete input capture. The unchanged
production rule retired it at turn 150 in third place (154 production
against 494, 141 and 390); it captured no city. It submitted no army-material
request. At turn 92, Iron stock 21 with income 2 already covered the ten-Iron
Knight bills, while the 84-Gold treasury could not pay their 160-Gold price.
This verifies native adoption of the opt-in, without establishing accepted
material, an executed upgrade, or a strategy improvement.

# Native conquest accounting after autosave recovery

The King game `civvis-20260927T082417Z` first held Babylon's Borsippa at
turn 226. Its final continuation began at turn 228. Segment accounting correctly
reported that the city was already present at that continuation's first frame,
but could not retain the original acquisition observation. Adding all three
segment totals would count the same city three times and retain abandoned turns.

The climb now writes `recovery-chain.json` before launching each continuation.
It records the original run, the ordered continuation tags, frozen turns and
selected saves. Save names remain provenance: the first native ownership state
in each continuation determines the rollback boundary. A new continuation
replaces all earlier observations from that turn onward. An older second reload
can discard the entire first continuation.

`conquest` remains scoped to one run segment. `game_conquest` separately reports
the surviving observed path, including its retained and discarded segments.
Automatic recording, backfill and publication retain both scopes and the explicit
ancestry. Publication enriches its summary without changing the local raw
summary. No native orders, recovery save selection or controller policy changed.

Reconstruction requires explicit climb ancestry, every segment's events,
matching native seat configuration, matching event run identities and the same
original home-capital plot. It never infers a game family from tag suffixes.
Missing or conflicting evidence yields `available: false` with a reason while
preserving segment measurements. Missing turns and incomplete city metadata
remain visible in an available reconstruction; available means validated
lineage, not complete observation coverage. Bookkeeping failures do not prevent
the existing native recovery flow.

## Completed native validation

The actual native seat reported four players, `DIFFICULTY_KING`, Gran Colombia,
Simón Bolívar, Tiny Pangaea, Online speed, Gathering Storm, six city states and
all victories enabled. Native maximum turns was 250. The controller stayed at
`4988720f02d42518e7f1215c568e2e199d96d6d5`, binary SHA-256
`5c2e1682c7f1dbdd48e0046ec0c4363be61a971f8aced2c63efe2e065dfe90eb`.

| Input segment | Observed turns | Surviving turns |
| --- | --- | --- |
| `civvis-20260927T082417Z` | 1–232 | 1–227 |
| `civvis-20260927T082417Z-cont1` | 231–232 | discarded |
| `civvis-20260927T082417Z-cont2` | 228–248 | 228–248 |

The climb's existing final ledger row explicitly names both recoveries: first
`AutoSave_0231.Civ6Save`, then `AutoSave_0228.Civ6Save`, each after freezing at
turn 232. A historical manifest was generated from that row in an external
evidence directory. Reconstruction read the original native events without
editing their directories or summaries.

The surviving path contains all 248 observed turns without gaps. It reports one
foreign major city observed and finally held, first observed at turn 226 with
`present_at_first_frame: false`; zero foreign major original capitals; zero
minor cities; and the own original capital finally held. The final segment alone
still reports first held at turn 228 with `present_at_first_frame: true`.

The native victory event names team 3 at turn 248 with `won: false`. The native
seat's victory table maps index 5 to `VICTORY_TECHNOLOGY`: a Babylon Science loss,
not a Domination win. This bookkeeping change supplies no evidence of stronger
gameplay. The next four-player King match continued on its existing pinned
controller while validation ran.

Local evidence is under `~/civvis-war-evidence-20260927/recovered-conquest-path/`.
The preserved ledger row SHA-256 is
`3f3167aaa3250b6fab9cc233994600c5d76e374527558b7913a7c2f128e5b6a7`.
`historical-validation.json` contains the source seat, executable identity,
terminal outcome, both accounting scopes and input hashes; its SHA-256 is
`49217a484bc720e23d3be30f087c2e3f56271959660880e097277851e7e152c8`.

| Native events | SHA-256 |
| --- | --- |
| original | `c4d03000bf356bb4aee1b77d4766cfd3ca85e22d14991a3b826bf33a40bd639e` |
| continuation 1 | `41c1ec64e6a91d74c8a586209335e0a5eb991f67e7df184a2c742c13286ff507` |
| continuation 2 | `cd602d03b0f1eb4cbc7cba0daee3506492a8ee36aecbf00646c7b13ed8d0b96a` |

## Verification

Ownership tests cover retained-capture deduplication, abandoned captures and war
requests, an older second reload, eliminated major/minor identities, a lost home
capital, missing turns, compressed events, missing inputs, mismatched seats or
capital plots, traversal rejection and read-only historical CLI reconstruction.
Integration tests check metadata exists before the continuation starts, metadata
write failure still permits recovery, automatic recording/backfill and archived
publication retain both scopes, and local summaries remain unchanged.

```bash
python3 -m unittest discover -s tools -p test_civ6_conquest.py
python3 -m unittest discover -s tools -p test_civ6_ladder.py
python3 -m unittest discover -s tools -p test_civ6_civvis_climb.py
python3 -m unittest discover -s tools -p test_docs_commands.py
cargo test --profile ci --locked
git diff --check
```

The focused Python suites passed: 28 conquest, 201 ladder and 194 climb tests.
The full Rust suite passed: 4,418 passed, zero failed, 53 ignored. The documentation
command checks and diff whitespace checks also passed. No engine behavior changed,
so no simulation performance soak was needed.

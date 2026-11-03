# Read-only native Artifact activation audit

`tools/civ6_artifact_activation_audit.py` keeps production proposals, observed
Archaeologists, occupied Artifact slots and relevant event rows separate. It
issues no native commands and rejects output inside its input run directory.

```sh
python3 tools/civ6_artifact_activation_audit.py \
  --run-dir /path/to/civvis-run --out /tmp/artifact-audit.json
```

The tool requires complete city work queries and valid occupied-slot records
before reporting zero Artifacts. It deduplicates identical slots and treats
conflicting or missing data as unknown. Missing unit exports, menu quotes and
production hashes also remain unknown. Only an explicit zero production hash
means an idle city. The Museum spelling is Firaxis's
`BUILDING_MUSEUM_ARTIFACT`.

Each selected state/event/proposal carries its original line number and hash.
Whole-file hashes cover the bytes read; they do not freeze an actively growing
log. Runtime and genome histories retain their original context. Event selection
is documented and does not prove that an unselected activation event is absent.
An excavation verdict or disappearing unit does not establish an acquired work.

## Three selected historical Immortal Culture games

| Native run, Nov 3 UTC | State rows | Relevant observations | Idle Museum with legal digger offer | Archaeologist proposals / observed units |
| --- | ---: | ---: | ---: | ---: |
| `002240Z` | 452 | 109 | 11 | 0 / 0 |
| `012701Z` | 567 | 0 | 0 | 0 / 0 |
| `024543Z` | 503 | 5 | 1 | 0 / 0 |

In `002240Z`, all 109 relevant readings confirm zero Artifacts while the owned
Mary Leakey requires one. This reproduces the manual audit used for the separate
default-off activation candidate. The `012701Z` stream has no matching person,
Museum, digger or positive Artifact observation. In `024543Z`, the Museum first
appears at 186 and the game ends at 187; it has no physical Artifact-requiring
person. A Museum observation alone does not establish the candidate's trigger.

Each game has one startup state with unknown work inventory, retained as unknown.
All three end in rival Science victories. These selected histories, repeated
state rows and differing source programs/profiles are not treatment arms or a
win-rate comparison. No experiment, flag or default was changed.

## Validation and reproduction

Ten focused tests passed, including actual CLI execution, unknown versus zero,
slot conflicts, partial tails, preserved input files and separation of proposals,
verdicts, units and works. The shared reader's five tests also passed.

The [adjudication](2026-11-03-artifact-execution-audit.json) binds the three
reports and source hashes. The [archive](2026-11-03-artifact-execution-audit.raw.tar.xz)
contains all five input files for each closed run, reports, tool sources, logs,
`reproduce.py` and the per-member manifest: 29 members, 2,819,196 bytes, SHA-256
`fbc3c559cd2be4a1767a707788482b0f5aca630a4f7be6adee45503d366c41d2`.
Extraction followed by `python3 reproduce.py` regenerates all three complete
reports exactly, including original line and whole-file hashes, without a game
installation. Packaging verified every original input against its report hash
and ran that reproduction successfully.

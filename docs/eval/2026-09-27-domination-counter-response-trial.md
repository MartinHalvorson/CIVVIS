# Host counter response under an assigned Domination target

Registered at **2026-09-27T04:19:12.542267+00:00**, before any fresh game.
Frozen source `1001b24ee7bb7cd8b979d5fc33866ceff3c785ae` has the same tracked tree as
parent main `8fcba544aa92a61c72235e154f58ac9f95cb7071`. This includes #3803 and #3804;
#3805 is an evidence-only report and changes no policy.

Eight fresh seeds **37925000–37925007** compare the existing HostOnly
`counter-in-lane` option. **ON is the deployed host control; OFF is the
experimental ablation.** There is no new controller, gene, deployment-default,
or evaluator change. The live host setting remains ON.

Both arms use Gran Colombia in seat zero, an assigned Domination target,
four majors, Tiny 60×38 Pangaea, six city-states, Online speed, all victories,
the natural 250-turn clock, and Prince players and barbarians. Only the focal
option changes; rivals remain fixed. The existing 21-option force bundle has
SHA-256 `439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0` and stays fixed.
Four two-pair segments alternate execution order; no partial outcomes are read.

The host option changes responses to **both Science and score pressure**.
This trial cannot attribute any result to Science alone. Earlier broad
counter-war evidence includes severe losses, so every ending, city/capital
retention metric, home-capital loss, and applied-action comparison is retained.
Absent-contrast exit-2 rejections remain rejections.

Before fresh games, build and freeze binary/source hashes, run appropriate
existing tests, and replay all 202 archived native frames. Read back the actual
option and Domination target. Native OFF uses `--without counter-in-lane`;
ON uses the existing host default and does not pass an unsupported HostOnly
`--with` argument. Fixed archived future states cannot establish survival,
conquest, a native win, or promotion.

A promising pilot requires more OFF Domination wins than ON, no increase in
home-capital losses, and no reduction in capitals retained. That result would
require a separately registered 16-pair replication before considering any
activation. No tuning follows fresh outcomes without a new registration.

Evidence root: `~/civvis-war-evidence-20260926/prince-counter-response/`.
`preregistration.json` records the complete design. Build, native replay, and
fresh results are pending. No fresh game has started.

## Native preflight rejected before fresh games

The registered source passed the full suite (4,390 passed, zero failed,
53 ignored), 60 existing counter tests, and 124 Domination tests. A probe of
the exact focal evaluator constructor reads `counter_in_lane=false` in OFF,
`true` in ON, and `Some(Domination)` in both arms. The unchanged ledger,
manifest, and firing gate pass (326/326 shown to fire, zero waivers).

Both persistent native replays completed all 202 frames, exit zero, but startup
metadata lists `counter-in-lane` as active in **both** arms. The OFF command
does include `--without counter-in-lane`. Source inspection shows
`configure_live_bridge` applies explicit withholds after force configuration;
the startup `treatments` field instead projects the ledger/forced list without
subtracting those withholds. This is a reporting defect, not proof that the
runtime setter failed.

The required native identity assertion rejected the preflight. No pilot freeze
or start receipt was written. Four external runners subsequently failed to
read the absent start receipt before invoking any simulation binary; no fresh
trial game started and no outcome was inspected. `preflight-abort.json` retains
the abort. A separate repair, #3808, verifies the startup identity against the
configured controller. The original registration, binaries, constructor
readback, and both native replays remain preserved. Before fresh games, record
the corrected source and any newer-main changes in a new registration receipt;
retain the same reserved seeds and fixed-rival comparison. The host setting
remains ON.

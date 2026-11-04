# Spaceport above the sea: native proposal preregistration

The interrupted Science win family `civvis-20261102T171007Z` provides a
concrete coastal failure. It built its first Spaceport at t156 and launched
Earth Satellite at t165. The selected tile (49,31) had been exported as
coastal-lowland band 1 since t26. At the original t139 decision, the native
Spaceport menu offered five sites; the other four were outside coastal
lowlands (`cl=-1`). The original pad's tile became flooded/pillaged at t176
and submerged/coast at t191. The subsequent Moon Landing appears at t210.
These observations do not estimate turns a different site would save.

The parent ends at t180; its continuation resumes at t179 and wins Science
at t227. They share game/map seeds 509223680/509223681 and the same city IDs
and positions at the overlap. This is one resumed game, with repeated turns,
not two independent outcomes or an uninterrupted-run claim.

The host exports `TerrainManager.GetCoastalLowlandType(plot)` and direct
`IsFlooded`/`IsSubmerged` readings. The shipped
`DLC/Expansion2/UI/Replacements/PlotTooltip_Expansion2.lua:26` uses that same
TerrainManager accessor. This policy does not invent a climate forecast:
it prefers an offered site outside coastal lowlands when starting a pad.
Such a site can still suffer other disasters, including river floods.

The opt-in AI method considers only a new proposed Spaceport in the same
owned city. It preserves an already-safe site, an active completed Flood
Barrier, every standing or queued Spaceport, and the original choice when
no other legal site exists. Safe alternatives use the existing positional
tie-break. The native bridge applies the returned position to `produce` or
`produce_next`; it changes no other order fields or production priorities.
Both controllers leave the gene off by default.

This version acts through the native bridge. A simulator run that never
calls this site selector cannot establish its strength. The separate
first-launch research screen and the active native fuel screen are not
evidence for this candidate. No default promotion is proposed.

The fixed captured-input query cases are frame 0 of t139, t140 and t176 in
the motivating parent, plus frame 0 of t166 in `civvis-20261102T185903Z`
(the first Spaceport order in the later unscreened Science win). Each query
uses a private immutable captured read, a Science controller, and the same
snapshot of the standing force-on bundle, represented by the actually
observed forced-tag list in the later unscreened run's genome. Retain that
profile source; the reproducer does not edit the standing file or silently
drop a refused tag. Only this new tag differs between
arms. Every control is repeated. This is a one-shot query on captured boards,
not replay of the original persistent controller or commands sent to a host.
The later source run was assigned Culture; the explicitly Science query is
therefore a counterfactual controller on its captured board.

Validation requires stable controls, unchanged non-position order content,
and each changed site present in the actual source's native offer with a
non-lowland tile reading. Record complete inputs, observed genomes, source
and binary hashes, and raw responses. Native execution and complete outcome
validation remain separate gates.

The initial seven focused Rust tests passed in the local unoptimized CI profile, covering
the opt-in, safe/legal alternatives, fallbacks, Flood Barrier state, existing
construction, ownership and invalid originals. Fourteen append-convention
tests and formatting checks pass. This was preregistration validation;
the final observations below supersede the pending CLI/query status.

The first query attempt passed the bundle as one comma-separated CLI flag.
The decider refused it before planning (exit 2, no genome or orders). Keep
that failed invocation with the evidence; the corrected call supplies one
`--with` argument per played tag. No candidate result came from that attempt.

The initial valid four-case query changed the site in the t139 and t140
reads, with all other order content equal. The actual captured t140/frame0
read still showed a Research Lab and no placed pad, even though later t140
observations show construction. Do not label that stale pre-placement read
as a construction control. Before the final query, add t140/frame1, whose
captured input explicitly includes the placed, incomplete Spaceport. Retain
the four original cases. An additional behavior test covers a foundation
that remains placed while another item occupies its queue.

## Final proposal validation

All eight focused tests pass, including the placed-foundation regression.
The native CLI builds in the unoptimized CI profile. All five final pairs
and repeated controls pass with only the tested gene changed in the observed
genomes. At t139/frame0 and t140/frame0, Ternate's proposed placement changes
from (49,31) to the native-offered non-lowland (48,27). All other order
content is identical. The placed t140/frame1 pad, the pillaged t176 pad,
and the later winner's already non-lowland t166 placement stay unchanged.

The archive retains immutable input reads, raw responses, source/binary
hashes and patch, the exact profile source, the failed CLI invocation, and
raw host lines documenting the original tile's flooding and submersion.
Its five-case firing receipt passes `gene_fires.py --max 0`; generated
evaluation metadata is current. This is proposal activation and host
observation, not execution of the safer placement, saved turns or a win gain.

The captured binary is pinned to prototype `c0322cc12`. Current main's
independent `old-techs-before-the-beeline` opt-in was then integrated; it
defaults off and is absent from the query profile. The selector and bridge
file hashes still match the archive. All eight behavior tests, 14 convention
checks and `cargo check --tests --locked` pass on the combined tree. The
current firing, ledger, evaluation-metadata and formatting checks also pass.

# Urgent Culture denial with a ready air sortie

Native run `civvis-20260921T221854Z-cont1`, pinned to `7e6ed5e4`,
redirected toward Brazil after peace with Macedon at171. Brazil had60 foreign
tourists against the leading opponent domestic count70 at170, rising to67
versus77 at175. The threat was real. Native decisions repeatedly held war
because the ground army had not finished staging, despite ready Bombers and
visible Brazilian Theater Squares within range.

An urgent Culture-denial opening now accepts a healthy Bomber's actual
positive-value Theater Square pillage choice on a cloned post-declaration
board, provided the modeled aircraft survives with at least60HP. Existing
war legality, treasury, peace deadline, single-front and geographic checks
remain in the caller. Other victory lanes, nonurgent threats, home emergencies,
spent or wounded aircraft, and unavailable missions do not get this exception.
The declaration explanation identifies the air sortie instead of claiming
that the ground army is already staged.

## Validation

Final production source: `a647309e3a8f96b2d2bc662ce6803b459ca20cb4`.
Integrated parent: `48e2388ce284d46fd3cae23bbdef5f0bff0df2b0`.

- Full Rust suite:4,143 passed, zero failed,53 ignored across six suites.
- Three focused tests pass, covering actual diplomacy, unchanged forecast
  board, home/lane/urgency guards, spent and wounded aircraft, and unavailable
  Theater missions. Disabling only the staging exception makes the actual
  declaration assertion fail. The positive fixture includes a visible,
  defended city; its earlier missing visibility was corrected before validation.
- Fourteen append-integrity tests pass; changed-line Rust quality passes.
- Eight four-player180-turn smoke games complete, seeds370000–370007.

## Recorded-game replay

Both final binaries process all166 frames from the native reload boundary
through175. The candidate changes six exported frames and four internal-action
frames, exactly matching the initial prototype differences after integration.
At172/frame1 it adds a declaration against Brazil and five air attacks,
including the Theater at36,18. At175/frames1 and2 it again proposes a declaration
and six attacks, including Theaters45,18 and37,14. The recorded future remains
at peace, so repeated declarations are counterfactual proposals, not three
actual wars. Likewise17 added air-attack exports are not17 executed sorties.

Other changes include existing prewar liquidation of Dyes, ground movement,
fortification and trade-route decisions. Later `not_at_war`, `target_unharmed`
and receipt differences compare new proposals with the unchanged old future;
they do not establish native host refusal. This opens a war before ground
capture readiness and accepts that strategic risk to begin Culture denial.
The replay does not establish tourism reduction, capture or victory. The
recorded match ultimately lost by Religion to Cree at176, so this change alone
must not be presented as preventing that loss.

The initial baseline was4f, but the task actually started on132464691, which
also contained the wall-defense change. Final evaluation instead uses exact48.
The4f and48 baselines were additionally verified to produce identical internal
and exported actions in all166 frames; neither intervening change accounts
for the prototype's observed differences.

Artifacts: `/tmp/civvis-urgent-air-denial-investigation/` contains input,
arguments, exact binaries, provenance, replies and reasoning logs.
`/tmp/civvis-3700-final-comparison.json` contains the full paired differences.
SHA256 values:

- Input:`3586d8d174d325b9c080093b12ecda1c43d6e6d0b0b123ef98cdc567c4683006`.
- Baseline:`1f13cf41a1d7cfcf395d22f72e9310460d8c637fd08bdbcdcf661898e014beaa`.
- Candidate:`757f7b04791f2d1f7e9459fede5a3230a6066612f9f32fe007e3800efe0c0319`.

Replay durations37.03/37.95seconds are not controlled performance measurements.
The required CI paired-cost gate is the performance evidence.
